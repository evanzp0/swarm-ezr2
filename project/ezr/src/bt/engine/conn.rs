use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use super::{addr_sock, PartialPieces};
use crate::bt::pieces::PieceMap;
use crate::bt::webrtc::PeerTransport;
use crate::bt::{peer, REQUEST_BLOCK_LEN};
use crate::engine::{BtConnView, Throttle};

pub(super) const PIPELINE_QUEUE_TIME_SECS: u64 = 3;

pub(super) const PIPELINE_MIN: usize = 2;

pub(super) const PIPELINE_SLOW_START_SLACK_BYTES: u64 = 5000;

pub(super) fn ema_target(ema_rate_bps: u64, cap: usize) -> usize {
    let want = (PIPELINE_QUEUE_TIME_SECS * ema_rate_bps / u64::from(REQUEST_BLOCK_LEN)) as usize;
    want.max(PIPELINE_MIN).min(cap).max(1)
}

pub(super) fn tick_pipeline(st: &mut ConnStat, now: std::time::Instant, cap: usize) {
    let dt_ms = now.duration_since(st.pipe_sample.1).as_millis() as u64;
    let dt_ms = dt_ms.max(1);
    let sample = st
        .cum_down
        .saturating_sub(st.pipe_sample.0)
        .saturating_mul(1000)
        / dt_ms;
    st.pipe_ema = st.pipe_ema * 4 / 5 + sample / 5;
    st.pipe_sample = (st.cum_down, now);
    if st.pipe_slow_start {
        let cur = st.cum_down.saturating_sub(st.tick_prev_down);
        if !st.peer_choking
            && st.pipe_last_second > 0
            && st.pipe_last_second + PIPELINE_SLOW_START_SLACK_BYTES >= cur
        {
            st.pipe_slow_start = false;
        }
        st.pipe_last_second = cur;
    } else {
        st.pipe_target = ema_target(st.pipe_ema, cap);
    }
    st.tick_prev_down = st.cum_down;
}

pub(super) struct DlFlow {
    pub(super) global: Arc<Throttle>,
    pub(super) task: Arc<Throttle>,
}

pub(super) enum PeerIn {
    Connected {
        client: String,
        peer_id: [u8; 20],
        out: mpsc::Sender<PeerOut>,
        ext_metadata: bool,
    },
    Msg(peer::PeerMsg),
    Closed {
        reason: String,
    },
    ProtocolViolation,
}

#[derive(Debug)]
pub(super) enum PeerOut {
    Send(peer::PeerMsg),
    Drop,
}

pub(super) struct ConnStat {
    pub(super) peer_out: mpsc::Sender<PeerOut>,
    pub(super) addr: String,
    pub(super) protocol: PeerTransport,
    pub(super) client: String,
    pub(super) peer_id: Option<[u8; 20]>,
    pub(super) has: HashSet<u32>,
    pub(super) inflight: HashMap<(u32, u32), (u32, std::time::Instant)>,
    pub(super) peer_choking: bool,
    pub(super) am_choking: bool,
    pub(super) peer_interested: bool,
    pub(super) inbound: bool,
    pub(super) tit_prev: (u64, std::time::Instant),
    pub(super) cum_down: u64,
    pub(super) cum_up: u64,
    pub(super) down_window: (u64, std::time::Instant),
    pub(super) up_window: (u64, std::time::Instant),
    pub(super) idle_check: (u64, std::time::Instant),
    pub(super) pipe_target: usize,
    pub(super) pipe_slow_start: bool,
    pub(super) pipe_ema: u64,
    pub(super) pipe_sample: (u64, std::time::Instant),
    pub(super) tick_prev_down: u64,
    pub(super) pipe_last_second: u64,
    pub(super) connected_at: std::time::Instant,
    pub(super) bad_pieces: u32,
    pub(super) invalid_requests: u32,
}

impl ConnStat {
    pub(super) fn speed(cur: &(u64, std::time::Instant), cum: u64) -> f64 {
        let dt = cur.1.elapsed().as_secs_f64();
        if dt <= 0.0 {
            return 0.0;
        }
        (cum.saturating_sub(cur.0)) as f64 / dt
    }
}

pub(super) struct PeerCandidate {
    pub(super) fails: u32,
    pub(super) last_try: Option<std::time::Instant>,
}

/// 全引擎共享的选片缓存：增量维护「多少 peer 报有」并按稀有度缓存
/// 排序结果。逐块 refill 只走缓存 O(小)，不再每块全量重算
/// availability（O(Σ|has|)）+ 全排序（O(P log P)）——真实大种子
/// （数万 piece）下曾把引擎任务 CPU 打满，socket 读取被饿死，速度
/// 塌到 100KB/s 以下（loopback 小 payload 基准测不出）。
pub(super) struct PieceOrderCache {
    pub(super) avail: HashMap<u32, usize>,
    pub(super) order: Vec<u32>,
    pub(super) dirty: bool,
    pub(super) sequential: bool,
}

impl PieceOrderCache {
    pub(super) fn new() -> Self {
        Self {
            avail: HashMap::new(),
            order: Vec::new(),
            dirty: true,
            sequential: false,
        }
    }

    pub(super) fn peer_gained(&mut self, p: u32) {
        *self.avail.entry(p).or_insert(0) += 1;
        self.dirty = true;
    }

    pub(super) fn peer_lost(&mut self, p: u32) {
        if let Some(c) = self.avail.get_mut(&p) {
            *c = c.saturating_sub(1);
            self.dirty = true;
        }
    }

    pub(super) fn rebuild(&mut self, pmap: &PieceMap) {
        let mut v: Vec<u32> = pmap
            .covered_pieces()
            .iter()
            .copied()
            .filter(|&p| self.avail.get(&p).copied().unwrap_or(0) > 0)
            .collect();
        if self.sequential {
            v.sort_unstable();
        } else {
            static ROUND: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let seed = ROUND.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            v.sort_by_key(|&p| (self.avail.get(&p).copied().unwrap_or(0), jitter(p, seed)));
        }
        self.order = v;
        self.dirty = false;
    }
}

/// 逐块 refill 的访问序：partial piece（按已收块数降序）优先，
/// 之后按缓存 order（稀有度+jitter）；done/无报有/已在 partial 阶段
/// 覆盖的 piece 跳过。与旧 order_wanted 的 partial-first 项同义。
pub(super) fn refill_visit_order(
    st_has: &HashSet<u32>,
    done: &HashSet<u32>,
    partial: &PartialPieces,
    cache_order: &[u32],
) -> Vec<u32> {
    let mut partials: Vec<u32> = partial
        .keys()
        .copied()
        .filter(|p| !done.contains(p) && st_has.contains(p))
        .collect();
    partials.sort_by_key(|&p| std::cmp::Reverse(partial.get(&p).map_or(0, |b| b.len())));
    partials
        .into_iter()
        .chain(
            cache_order
                .iter()
                .copied()
                .filter(|p| !partial.contains_key(p) && !done.contains(p) && st_has.contains(p)),
        )
        .collect()
}

pub(super) fn progress_emit_due(
    last_emit: std::time::Instant,
    now: std::time::Instant,
    min_interval_ms: u64,
) -> bool {
    now.duration_since(last_emit) >= Duration::from_millis(min_interval_ms)
}

pub(super) fn refresh_idle(st: &mut ConnStat, now: std::time::Instant) {
    if st.cum_down != st.idle_check.0 {
        st.idle_check = (st.cum_down, now);
    }
}

pub(super) fn idle_evict_due(st: &ConnStat, now: std::time::Instant, idle_secs: u64) -> bool {
    st.cum_down == st.idle_check.0 && now.duration_since(st.idle_check.1).as_secs() >= idle_secs
}

pub(super) fn inbound_admitted(
    conns: &HashMap<usize, ConnStat>,
    addr: (IpAddr, u16),
    cap: usize,
) -> bool {
    conns.len() < cap && !conns.values().any(|st| addr_sock(&st.addr) == Some(addr))
}

/// piece 的伪随机权重：同一 (piece, seed) 恒定不变。
/// 排序键在单次排序内必须稳定，否则 sort_by_key 的 total order
/// 前提被破坏（panic: user-provided comparison function does not
/// correctly implement a total order）。
pub(super) fn jitter(piece: u32, seed: u64) -> u64 {
    let mut x = u64::from(piece).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ seed.rotate_left(17);
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    x
}

impl PeerCandidate {
    pub(super) fn cooldown_ok(&self, base_secs: u64, cap_secs: u64) -> bool {
        match self.last_try {
            None => true,
            Some(t) => {
                let backoff = (base_secs << self.fails.min(5)).min(cap_secs);
                std::time::Instant::now().duration_since(t).as_secs() >= backoff
            }
        }
    }
}

pub(super) fn new_conn_stat(addr: String) -> ConnStat {
    new_conn_stat_with(addr, PeerTransport::Tcp)
}

pub(super) fn new_conn_stat_with(addr: String, protocol: PeerTransport) -> ConnStat {
    let (dummy_tx, _dummy_rx) = mpsc::channel(1);
    ConnStat {
        peer_out: dummy_tx,
        addr,
        protocol,
        client: String::new(),
        peer_id: None,
        has: HashSet::new(),
        inflight: HashMap::new(),
        peer_choking: true,
        am_choking: true,
        peer_interested: false,
        inbound: false,
        tit_prev: (0, std::time::Instant::now()),
        cum_down: 0,
        cum_up: 0,
        down_window: (0, std::time::Instant::now()),
        up_window: (0, std::time::Instant::now()),
        idle_check: (0, std::time::Instant::now()),
        pipe_target: 4,
        pipe_slow_start: true,
        pipe_ema: 0,
        pipe_sample: (0, std::time::Instant::now()),
        tick_prev_down: 0,
        pipe_last_second: 0,
        connected_at: std::time::Instant::now(),
        bad_pieces: 0,
        invalid_requests: 0,
    }
}

pub(super) fn conn_flags(
    st: &ConnStat,
    covered: &[u32],
    done: &HashSet<u32>,
    optimistic_slot: Option<usize>,
    conn_id: usize,
) -> String {
    let we_want = covered
        .iter()
        .any(|&p| !done.contains(&p) && st.has.contains(&p));
    let download = match (we_want, st.peer_choking) {
        (true, false) => Some('D'),
        (true, true) => Some('d'),
        _ => None,
    };
    let upload = match (st.peer_interested, st.am_choking) {
        (true, false) => Some('U'),
        (true, true) => Some('u'),
        _ => None,
    };
    let idle = match (we_want, st.peer_choking) {
        (false, false) => Some('K'),
        _ => None,
    };
    let uninterested = match (st.am_choking, st.peer_interested) {
        (false, false) => Some('?'),
        _ => None,
    };
    let flags: String = [
        download,
        upload,
        (optimistic_slot == Some(conn_id)).then_some('O'),
        st.inbound.then_some('I'),
        idle,
        uninterested,
    ]
    .into_iter()
    .flatten()
    .collect();
    if flags.is_empty() {
        "-".to_string()
    } else {
        flags
    }
}

/// 种子健康度（FR-02-37）：qBt Availability / libtorrent distributed_copies
/// 同源。公式（D74 钦定口径）：每 piece 持有者计数 = 连接的非种子 peer 持有
/// 记 1 + 本端已有记 1；健康度 = 计数最小值 + 连接种子数 +（计数超出最小值
/// 的 piece 数 ÷ 总 piece 数）。连接 = 已握手连接（位图未知不计入，与并发
/// panel 显示口径一致）。
pub(super) fn distributed_copies(
    conns: &HashMap<usize, ConnStat>,
    covered: &[u32],
    done: &HashSet<u32>,
) -> f64 {
    if covered.is_empty() {
        return 0.0;
    }
    let known: Vec<(usize, &ConnStat)> = conns
        .iter()
        .filter(|(_, st)| st.peer_id.is_some())
        .map(|(cid, st)| (*cid, st))
        .collect();
    let seeds: HashSet<usize> = known
        .iter()
        .filter(|(_, st)| {
            st.has.len() >= covered.len() && covered.iter().all(|p| st.has.contains(p))
        })
        .map(|(cid, _)| *cid)
        .collect();
    let mut counts = vec![0usize; covered.len()];
    for (i, p) in covered.iter().enumerate() {
        let mut c = usize::from(done.contains(p));
        for (cid, st) in &known {
            if !seeds.contains(cid) && st.has.contains(p) {
                c += 1;
            }
        }
        counts[i] = c;
    }
    let min_c = counts.iter().copied().min().unwrap_or(0);
    let extra = counts.iter().filter(|c| **c > min_c).count();
    (min_c + seeds.len()) as f64 + extra as f64 / covered.len() as f64
}

pub(super) fn conn_views(
    conns: &mut HashMap<usize, ConnStat>,
    covered: &[u32],
    done: &HashSet<u32>,
    optimistic_slot: Option<usize>,
) -> (Vec<BtConnView>, f64) {
    let now = std::time::Instant::now();
    let mut views = Vec::with_capacity(conns.len());
    // 本端缺失 piece 集（FR-02-36 相关度分母）：covered − done。
    let missing: HashSet<u32> = covered
        .iter()
        .copied()
        .filter(|p| !done.contains(p))
        .collect();
    for (cid, st) in conns.iter_mut() {
        // 握手未完成（peer_id 未知）的连接不进并发 panel：对方交叉拨入
        // （我们出连的同时对方也拨入）在握手完成前会以同 IP 挂在表里，
        // 渲染出来就是「同一 peer 两行」，且客户端会周期性重拨使该窗口
        // 反复出现。并发 panel 只展示已建立（握手完成）的连接。
        if st.peer_id.is_none() {
            continue;
        }
        let seeder = !covered.is_empty()
            && st.has.len() >= covered.len()
            && covered.iter().all(|p| st.has.contains(p));
        // 相关度（FR-02-36，qBt PeerInfo::calcRelevance 同源）=
        // （对端拥有 ∧ 本端缺失的 piece 数）÷（本端缺失 piece 总数）；
        // 本端无缺失（做种中/已完成下载）时恒 0。
        let relevance = if missing.is_empty() {
            0.0
        } else {
            let overlap = st.has.iter().filter(|p| missing.contains(*p)).count();
            overlap as f64 / missing.len() as f64
        };
        let down_speed = ConnStat::speed(&st.down_window, st.cum_down);
        let up_speed = ConnStat::speed(&st.up_window, st.cum_up);
        st.down_window = (st.cum_down, now);
        st.up_window = (st.cum_up, now);
        views.push(BtConnView {
            id: *cid,
            addr: st.addr.clone(),
            protocol: st.protocol.label().to_string(),
            down_speed,
            cum_down: st.cum_down,
            up_speed,
            cum_up: st.cum_up,
            client: st.client.clone(),
            seeder,
            flags: conn_flags(st, covered, done, optimistic_slot, *cid),
            relevance,
        });
    }
    views.sort_unstable_by_key(|v| v.id);
    let availability = distributed_copies(conns, covered, done);
    (views, availability)
}
