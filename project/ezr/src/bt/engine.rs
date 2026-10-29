use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use super::metainfo::TorrentMeta;
use super::pieces::PieceMap;
use super::tracker::{
    announce_http, announce_udp, udp_host_port, AnnounceParams, PeerAddr, TrackerEvent, TrackerUrl,
};
use super::{peer, BtError, BtTaskCmd, PEER_PIPELINE, REQUEST_BLOCK_LEN};
use crate::engine::{BtConnView, EngineShared, Evt};

const FIRST_ANNOUNCE_DELAY: u64 = 2;
const TRACKER_BACKOFF_START: u64 = 8;
const TRACKER_BACKOFF_CAP: u64 = 60;
const PEER_SWEEP_SECS: u64 = 2;
const UPLOAD_BLOCK: usize = 16 * 1024;
const REQUEST_TIMEOUT_SECS: u64 = 20;
const REQUESTS_PER_PIECE: usize = 4;
const RECHOKE_INTERVAL_SECS: u64 = 10;
const OPTIMISTIC_INTERVAL_SECS: u64 = 30;
const UPLOAD_SLOTS: usize = 4;
const PEER_RETRY_BASE_SECS: u64 = 5;
const PEER_RETRY_CAP_SECS: u64 = 120;

enum PeerIn {
    Connected {
        abbrev: String,
        peer_id: [u8; 20],
        out: mpsc::Sender<PeerOut>,
        ext_metadata: bool,
    },
    Msg(peer::PeerMsg),
    Closed {
        reason: String,
    },
}

#[derive(Debug)]
enum PeerOut {
    Send(peer::PeerMsg),
    Drop,
}

struct ConnStat {
    peer_out: mpsc::Sender<PeerOut>,
    addr: String,
    abbrev: String,
    peer_id: Option<[u8; 20]>,
    has: HashSet<u32>,
    inflight: HashMap<(u32, u32), (u32, std::time::Instant)>,
    peer_choking: bool,
    am_choking: bool,
    peer_interested: bool,
    tit_prev: (u64, std::time::Instant),
    cum_down: u64,
    cum_up: u64,
    down_window: (u64, std::time::Instant),
    up_window: (u64, std::time::Instant),
}

impl ConnStat {
    fn speed(cur: &(u64, std::time::Instant), cum: u64) -> f64 {
        let dt = cur.1.elapsed().as_secs_f64();
        if dt <= 0.0 {
            return 0.0;
        }
        (cum.saturating_sub(cur.0)) as f64 / dt
    }
}

struct PeerCandidate {
    fails: u32,
    last_try: Option<std::time::Instant>,
}

impl PeerCandidate {
    fn cooldown_ok(&self) -> bool {
        match self.last_try {
            None => true,
            Some(t) => {
                let backoff = (PEER_RETRY_BASE_SECS << self.fails.min(5)).min(PEER_RETRY_CAP_SECS);
                std::time::Instant::now().duration_since(t).as_secs() >= backoff
            }
        }
    }
}

pub(crate) async fn run(
    spec: crate::engine::BtSpec,
    shared: Arc<EngineShared>,
    mut cmd_rx: mpsc::Receiver<BtTaskCmd>,
    evt_tx: mpsc::Sender<Evt>,
) {
    let id = spec.id;
    let peer_id = super::peer_id::make_peer_id();
    let pmap = Arc::new(PieceMap::new(&spec.files, spec.meta.piece_length));

    let mut files_handles: HashMap<usize, tokio::fs::File> = HashMap::new();
    for (i, f) in pmap.files.iter().enumerate() {
        if !f.selected {
            continue;
        }
        let abs = file_abs_path(&spec.save_dir, &spec.name, &f.path);
        if let Some(parent) = std::path::Path::new(&abs).parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        match tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .read(true)
            .open(&abs)
            .await
        {
            Ok(fh) => {
                if f.size > 0 {
                    let _ = fh.set_len(f.size).await;
                }
                files_handles.insert(i, fh);
            }
            Err(e) => {
                report_fail(&evt_tx, id, BtError::Io(e.to_string())).await;
                return;
            }
        }
    }

    let mut done: HashSet<u32> = HashSet::new();
    let mut partial: PartialPieces = HashMap::new();
    let mut downloaded: u64 = 0;
    let sidecar_path = crate::model::namegen::sidecar_path_of(&spec.save_dir, &spec.name);
    let prefilled: Vec<u32> = if spec.seeding {
        pmap.covered_pieces().to_vec()
    } else {
        crate::model::BtSidecar::load_done(&sidecar_path).unwrap_or_default()
    };
    for (pi, &p) in prefilled.iter().enumerate() {
        if !pmap.covers(p) {
            continue;
        }
        // 大种子的启动重校验（逐块 SHA-1）可能持续数十秒；期间 Pause/Cancel
        // 必须立即生效。否则旧引擎迟迟不退，用户在重校验窗口里「暂停→继续」
        // 会拉起第二个引擎：两个引擎同写文件与 sidecar，进度互相回退甚至清零。
        let verified = tokio::select! {
            v = verify_piece_from_disk(&files_handles, &pmap, p, &spec.meta) => v,
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(BtTaskCmd::Pause | BtTaskCmd::StopSeed) => {
                        if !spec.seeding {
                            // 中断点语义：已校验通过 ∪ 尚未校验到的预填余量。
                            // 只存已校验子集会让 sidecar 回退（余量被误判丢失，
                            // 恢复后重下）；未校验余量下次载入时本就会被重新
                            // 校验，安全且零进度损失。校验失败的 piece 两边都
                            // 不在，自然排除。
                            let mut final_done = done.clone();
                            final_done.extend(prefilled[pi..].iter().copied());
                            save_bt_sidecar(&sidecar_path, &final_done);
                        }
                        report_stopped(&evt_tx, id, &done, &partial, downloaded).await;
                        return;
                    }
                    Some(BtTaskCmd::Cancel) => {
                        report_stopped(&evt_tx, id, &done, &partial, downloaded).await;
                        return;
                    }
                    _ => continue,
                }
            }
        };
        if verified {
            done.insert(p);
            downloaded += selected_bytes_of_piece(&pmap, p);
        }
    }
    if !spec.seeding {
        save_bt_sidecar(&sidecar_path, &done);
    }
    let mut sidecar_saved_done: usize = done.len();

    let listener = match tokio::net::TcpListener::bind("0.0.0.0:0").await {
        Ok(l) => l,
        Err(e) => {
            report_fail(&evt_tx, id, BtError::Io(e.to_string())).await;
            return;
        }
    };
    let listen_port = listener.local_addr().map(|a| a.port()).unwrap_or(6881);

    let trackers: Vec<TrackerUrl> = {
        let mut seen = HashSet::new();
        spec.trackers
            .iter()
            .filter_map(|s| TrackerUrl::parse(s))
            .filter(|u| seen.insert(u.clone()))
            .collect()
    };
    let live: Arc<std::sync::Mutex<LiveStats>> = Arc::new(std::sync::Mutex::new(LiveStats {
        uploaded: 0,
        downloaded,
        left: pmap.selected_total() - downloaded,
        completed: false,
    }));
    let (tr_tx, mut tr_rx) = mpsc::channel::<TrackerReport>(32);
    let completed_notifies: Vec<Arc<tokio::sync::Notify>> = (0..trackers.len())
        .map(|_| Arc::new(tokio::sync::Notify::new()))
        .collect();
    for (ti, url) in trackers.iter().enumerate() {
        spawn_tracker(
            url.clone(),
            AnnounceCtx {
                infohash: spec.meta.infohash,
                peer_id,
                port: listen_port,
                client: shared.bt_tracker_client(spec.proxy_endpoint.as_ref()),
                numwant: 50,
                live: live.clone(),
                completed_notify: completed_notifies[ti].clone(),
            },
            tr_tx.clone(),
        );
    }
    drop(tr_tx);

    let (peer_tx, mut peer_rx) = mpsc::channel::<(usize, PeerIn)>(256);
    let mut next_conn: usize = 1;
    let mut conns: HashMap<usize, ConnStat> = HashMap::new();
    let mut candidates: HashMap<(IpAddr, u16), PeerCandidate> = HashMap::new();
    let mut seeding = spec.seeding;

    let mut tick = tokio::time::interval(Duration::from_secs(1));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut sweep = tokio::time::interval(Duration::from_secs(PEER_SWEEP_SECS));
    sweep.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut tracker_fail_count = 0usize;
    let mut got_peers = !trackers.is_empty();
    let mut last_rechoke = std::time::Instant::now();
    let mut last_optimistic = std::time::Instant::now();
    let mut optimistic_slot: Option<usize> = None;
    let mut optimistic_cursor: usize = 0;

    loop {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    BtTaskCmd::Pause | BtTaskCmd::StopSeed => {
                        save_bt_sidecar(&sidecar_path, &done);
                        let (up, down, left) = live_snapshot(&live);
                        spawn_stopped_announce(&trackers, &spec, &peer_id, listen_port, &shared, up, down, left);
                        report_stopped(&evt_tx, id, &done, &partial, downloaded).await;
                        return;
                    }
                    BtTaskCmd::Cancel => {
                        let (up, down, left) = live_snapshot(&live);
                        spawn_stopped_announce(&trackers, &spec, &peer_id, listen_port, &shared, up, down, left);
                        report_stopped(&evt_tx, id, &done, &partial, downloaded).await;
                        return;
                    }
                    BtTaskCmd::Disconnect(conn_id) => {
                        if let Some(st) = conns.remove(&conn_id) {
                            if let Some(ip) = addr_ip(&st.addr) {
                                shared.blacklist_add(ip);
                            }
                            let _ = st.peer_out.send(PeerOut::Drop).await;
                        }
                    }
                }
            }
            Some(rep) = tr_rx.recv() => {
                match rep {
                    TrackerReport::Peers { peers, complete, incomplete } => {
                        tracker_fail_count = 0;
                        got_peers = true;
                        let _ = evt_tx
                            .send(Evt::BtPeers { id, complete, incomplete })
                            .await;
                        for p in peers {
                            if shared.is_blacklisted(p.ip) {
                                continue;
                            }
                            candidates
                                .entry((p.ip, p.port))
                                .or_insert(PeerCandidate { fails: 0, last_try: None });
                        }
                        fill_peer_slots(
                            &spec, &peer_id, &peer_tx, shared.as_ref(), &mut conns,
                            &mut candidates, &mut next_conn,
                        );
                    }
                    TrackerReport::Failed { reason } => {
                        let _ = reason;
                        tracker_fail_count += 1;
                        if tracker_fail_count >= trackers.len() && !got_peers && !trackers.is_empty() {
                            report_fail(&evt_tx, id, BtError::Tracker("全部 tracker 失败".into())).await;
                            return;
                        }
                    }
                }
            }
            Ok((stream, addr)) = listener.accept() => {
                if !shared.is_blacklisted(addr.ip()) && conns.len() < spec.concurrency.max(1) + 8 {
                    spawn_incoming_peer(next_conn, stream, addr, &spec, &peer_id, peer_tx.clone());
                    conns.insert(next_conn, new_conn_stat(addr.to_string()));
                    next_conn += 1;
                } else if shared.is_blacklisted(addr.ip()) {
                    drop(stream);
                }
            }
            Some((conn_id, ev)) = peer_rx.recv() => {
                let prev_downloaded = downloaded;
                match ev {
                    PeerIn::Connected {
                        abbrev,
                        peer_id,
                        out,
                        ext_metadata,
                    } => {
                        // 同一 peer_id（同一客户端实例）只保留一条连接：
                        // 出连 + 入连双通道并存会导致并发 panel 同 IP 两行、
                        // 速度统计分裂。与 libtorrent 单连接-per-peer 口径一致。
                        let duplicate = conns.iter().any(|(cid, st)| {
                            *cid != conn_id && st.peer_id == Some(peer_id)
                        });
                        if duplicate {
                            let dup_key = conns
                                .get(&conn_id)
                                .and_then(|st| addr_sock(&st.addr));
                            conns.remove(&conn_id);
                            let _ = out.send(PeerOut::Drop).await;
                            if let Some(key) = dup_key {
                                candidates.remove(&key);
                            }
                        } else if let Some(st) = conns.get_mut(&conn_id) {
                            st.abbrev = abbrev;
                            st.peer_id = Some(peer_id);
                            st.peer_out = out;
                            if let Some(key) = addr_sock(&st.addr) {
                                if let Some(c) = candidates.get_mut(&key) {
                                    c.fails = 0;
                                }
                            }
                            let bitfield = bitfield_bytes(&done, pmap.num_pieces());
                            if !bitfield.is_empty() {
                                let _ = st.peer_out.send(PeerOut::Send(peer::PeerMsg::Bitfield(bitfield))).await;
                            }
                            if seeding {
                                st.am_choking = false;
                                let _ = st.peer_out.send(PeerOut::Send(peer::PeerMsg::Unchoke)).await;
                            }
                            if ext_metadata {
                                let payload = peer::ext_handshake_payload(
                                    1,
                                    spec.meta.info_bytes.len() as u32,
                                );
                                let _ = st
                                    .peer_out
                                    .send(PeerOut::Send(peer::PeerMsg::Extended {
                                        ext_id: 0,
                                        payload,
                                    }))
                                    .await;
                            }
                        }
                        let views = conn_views(&mut conns, pmap.covered_pieces());
                        let _ = evt_tx.send(Evt::BtConns { id, conns: views }).await;
                    }
                    PeerIn::Closed { reason } => {
                        if let Some(st) = conns.remove(&conn_id) {
                            if let Some(key) = addr_sock(&st.addr) {
                                if reason == "self" || reason == "dup" {
                                    candidates.remove(&key);
                                } else if let Some(c) = candidates.get_mut(&key) {
                                    c.fails = c.fails.saturating_add(1);
                                }
                            }
                        }
                        fill_peer_slots(
                            &spec, &peer_id, &peer_tx, shared.as_ref(), &mut conns,
                            &mut candidates, &mut next_conn,
                        );
                    }
                    PeerIn::Msg(msg) => {
                        handle_peer_msg(
                            conn_id,
                            msg,
                            &mut conns,
                            &pmap,
                            &spec,
                            &mut done,
                            &mut downloaded,
                            &mut files_handles,
                            &mut partial,
                        )
                        .await;
                        if downloaded != prev_downloaded {
                            {
                                let mut st = live.lock().expect("live stats 锁");
                                st.downloaded = downloaded;
                                st.left = pmap.selected_total().saturating_sub(downloaded);
                            }
                            let views = conn_views(&mut conns, pmap.covered_pieces());
                            let _ = evt_tx.send(Evt::BtProgress {
                                id,
                                downloaded,
                                chunk_done: done.len() as u32,
                                done_blocks: done.iter().copied().collect::<Vec<_>>(),
                                partial_blocks: partial_piece_list(&partial, &done),
                                active_blocks: inflight_piece_list(&conns, &done),
                                uploaded: uploaded_total(&conns),
                                upload_speed: 0.0,
                            }).await;
                            let _ = evt_tx.send(Evt::BtConns { id, conns: views }).await;
                        }
                        if !seeding && all_selected_done(&done, &pmap) {
                            seeding = true;
                            sidecar_saved_done = done.len();
                            live.lock().expect("live stats 锁").completed = true;
                            for n in &completed_notifies {
                                n.notify_one();
                            }
                            let _ = crate::model::BtSidecar::remove(&sidecar_path);
                            optimistic_slot = apply_choker(
                                &mut conns, true, false, optimistic_slot, &mut optimistic_cursor,
                            )
                            .await;
                            let _ = evt_tx.send(Evt::BtDone { id, total: pmap.selected_total() }).await;
                        }
                    }
                }
            }
            _ = tick.tick() => {
                let now = std::time::Instant::now();
                let mut stale: Vec<usize> = Vec::new();
                for (cid, st) in conns.iter_mut() {
                    let before = st.inflight.len();
                    st.inflight
                        .retain(|_, (_, at)| now.duration_since(*at).as_secs() < REQUEST_TIMEOUT_SECS);
                    if st.inflight.len() != before {
                        stale.push(*cid);
                    }
                }
                for cid in stale {
                    let claimed = claimed_blocks(&conns, &partial);
                    let availability = availability_map(&conns);
                    if let Some(st) = conns.get_mut(&cid) {
                        issue_requests(cid, st, &claimed, &availability, &pmap, &done, &partial).await;
                    }
                }
                {
                    let mut st = live.lock().expect("live stats 锁");
                    st.uploaded = uploaded_total(&conns);
                    st.downloaded = downloaded;
                    st.left = pmap.selected_total().saturating_sub(downloaded);
                }
                if !seeding && done.len() != sidecar_saved_done {
                    save_bt_sidecar(&sidecar_path, &done);
                    sidecar_saved_done = done.len();
                }
                if now.duration_since(last_rechoke).as_secs() >= RECHOKE_INTERVAL_SECS {
                    let rotate = now.duration_since(last_optimistic).as_secs() >= OPTIMISTIC_INTERVAL_SECS;
                    if rotate {
                        last_optimistic = now;
                    }
                    optimistic_slot = apply_choker(
                        &mut conns, seeding, rotate, optimistic_slot, &mut optimistic_cursor,
                    )
                    .await;
                    last_rechoke = now;
                }
                if !seeding && all_selected_done(&done, &pmap) {
                    seeding = true;
                    sidecar_saved_done = done.len();
                    live.lock().expect("live stats 锁").completed = true;
                    for n in &completed_notifies {
                        n.notify_one();
                    }
                    let _ = crate::model::BtSidecar::remove(&sidecar_path);
                    optimistic_slot = apply_choker(
                        &mut conns, true, false, optimistic_slot, &mut optimistic_cursor,
                    )
                    .await;
                    let _ = evt_tx.send(Evt::BtDone { id, total: pmap.selected_total() }).await;
                }
                let views = conn_views(&mut conns, pmap.covered_pieces());
                let sum_up_speed: f64 = views.iter().map(|v| v.up_speed).sum();
                let _ = evt_tx.send(Evt::BtProgress {
                    id,
                    downloaded,
                    chunk_done: done.len() as u32,
                    done_blocks: done.iter().copied().collect::<Vec<_>>(),
                    partial_blocks: partial_piece_list(&partial, &done),
                    active_blocks: inflight_piece_list(&conns, &done),
                    uploaded: uploaded_total(&conns),
                    upload_speed: sum_up_speed,
                }).await;
                let _ = evt_tx.send(Evt::BtConns { id, conns: views }).await;
            }
            _ = sweep.tick() => {
                fill_peer_slots(
                    &spec, &peer_id, &peer_tx, shared.as_ref(), &mut conns,
                    &mut candidates, &mut next_conn,
                );
            }
            else => break,
        }
    }
}

type PartialPieces = HashMap<u32, BTreeMap<u32, Vec<u8>>>;

struct AnnounceCtx {
    infohash: [u8; 20],
    peer_id: [u8; 20],
    port: u16,
    client: reqwest::Client,
    numwant: u32,
    live: Arc<std::sync::Mutex<LiveStats>>,
    completed_notify: Arc<tokio::sync::Notify>,
}

#[derive(Default)]
struct LiveStats {
    uploaded: u64,
    downloaded: u64,
    left: u64,
    completed: bool,
}

enum TrackerReport {
    Peers {
        peers: Vec<PeerAddr>,
        complete: u32,
        incomplete: u32,
    },
    Failed {
        reason: String,
    },
}

fn spawn_tracker(url: TrackerUrl, ctx: AnnounceCtx, tx: mpsc::Sender<TrackerReport>) {
    tokio::spawn(async move {
        let mut delay = FIRST_ANNOUNCE_DELAY;
        let mut min_interval_secs: u32 = 0;
        let mut first = true;
        let mut completed_sent = false;
        loop {
            let wait = delay.max(min_interval_secs as u64);
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
                _ = ctx.completed_notify.notified() => {}
            }
            let first_now = first;
            first = false;
            let (up, down, left, ev) = {
                let st = ctx.live.lock().expect("live stats 锁");
                if st.completed && !completed_sent {
                    completed_sent = true;
                    (
                        st.uploaded,
                        st.downloaded,
                        st.left,
                        Some(TrackerEvent::Completed),
                    )
                } else if first_now {
                    (
                        st.uploaded,
                        st.downloaded,
                        st.left,
                        Some(TrackerEvent::Started),
                    )
                } else {
                    (st.uploaded, st.downloaded, st.left, None)
                }
            };
            let params = AnnounceParams {
                infohash: ctx.infohash,
                peer_id: ctx.peer_id,
                port: ctx.port,
                uploaded: up,
                downloaded: down,
                left,
                event: ev,
                numwant: ctx.numwant,
            };
            let res = match &url {
                TrackerUrl::Http(u) => announce_http(u, &params, &ctx.client).await,
                TrackerUrl::Udp(u) => {
                    let Some((host, port)) = udp_host_port(u) else {
                        break;
                    };
                    match tokio::net::UdpSocket::bind("0.0.0.0:0").await {
                        Ok(sock) => announce_udp(sock, &host, port, &params).await,
                        Err(e) => Err(BtError::Io(e.to_string())),
                    }
                }
            };
            match res {
                Ok(resp) => {
                    min_interval_secs = resp.min_interval.unwrap_or(0);
                    delay = resp.interval.max(1) as u64;
                    let _ = tx
                        .send(TrackerReport::Peers {
                            peers: resp.peers,
                            complete: resp.complete,
                            incomplete: resp.incomplete,
                        })
                        .await;
                }
                Err(e) => {
                    let _ = tx
                        .send(TrackerReport::Failed {
                            reason: e.to_string(),
                        })
                        .await;
                    delay = delay
                        .saturating_mul(2)
                        .clamp(TRACKER_BACKOFF_START, TRACKER_BACKOFF_CAP);
                }
            }
        }
    });
}

fn spawn_stopped_announce(
    trackers: &[TrackerUrl],
    spec: &crate::engine::BtSpec,
    peer_id: &[u8; 20],
    port: u16,
    shared: &Arc<EngineShared>,
    uploaded: u64,
    downloaded: u64,
    left: u64,
) {
    let client = shared.bt_tracker_client(spec.proxy_endpoint.as_ref());
    let urls = trackers.to_vec();
    let ih = spec.meta.infohash;
    let pid = *peer_id;
    tokio::spawn(async move {
        for url in urls {
            let params = AnnounceParams {
                infohash: ih,
                peer_id: pid,
                port,
                uploaded,
                downloaded,
                left,
                event: Some(TrackerEvent::Stopped),
                numwant: 0,
            };
            match &url {
                TrackerUrl::Http(u) => {
                    let _ = announce_http(u, &params, &client).await;
                }
                TrackerUrl::Udp(u) => {
                    if let Some((host, p)) = udp_host_port(u) {
                        if let Ok(sock) = tokio::net::UdpSocket::bind("0.0.0.0:0").await {
                            let _ = announce_udp(sock, &host, p, &params).await;
                        }
                    }
                }
            }
        }
    });
}

fn live_snapshot(live: &std::sync::Mutex<LiveStats>) -> (u64, u64, u64) {
    let st = live.lock().expect("live stats 锁");
    (st.uploaded, st.downloaded, st.left)
}

fn spawn_outgoing_peer(
    conn_id: usize,
    p: PeerAddr,
    spec: &crate::engine::BtSpec,
    peer_id: &[u8; 20],
    tx: mpsc::Sender<(usize, PeerIn)>,
) {
    let meta = spec.meta.clone();
    let pid = *peer_id;
    tokio::spawn(async move {
        let addr = format!("{}:{}", p.ip, p.port);
        let sock = match tokio::time::timeout(
            Duration::from_secs(super::CONNECT_TIMEOUT_SECS),
            TcpStream::connect(&addr),
        )
        .await
        {
            Ok(Ok(s)) => s,
            _ => {
                let _ = tx
                    .send((
                        conn_id,
                        PeerIn::Closed {
                            reason: "连接失败".into(),
                        },
                    ))
                    .await;
                return;
            }
        };
        let _ = sock.set_nodelay(true);
        run_peer_loop(conn_id, sock, &meta.infohash, &pid, tx).await;
    });
}

fn spawn_incoming_peer(
    conn_id: usize,
    sock: TcpStream,
    addr: std::net::SocketAddr,
    spec: &crate::engine::BtSpec,
    peer_id: &[u8; 20],
    tx: mpsc::Sender<(usize, PeerIn)>,
) {
    let meta = spec.meta.clone();
    let pid = *peer_id;
    tokio::spawn(async move {
        let _ = sock.set_nodelay(true);
        run_peer_loop(conn_id, sock, &meta.infohash, &pid, tx).await;
        let _ = addr;
    });
}

fn addr_ip(addr: &str) -> Option<IpAddr> {
    addr_sock(addr).map(|(ip, _)| ip)
}

fn addr_sock(addr: &str) -> Option<(IpAddr, u16)> {
    let (host, port) = addr.rsplit_once(':')?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    Some((host.parse().ok()?, port.parse().ok()?))
}

async fn run_peer_loop(
    conn_id: usize,
    mut sock: TcpStream,
    infohash: &[u8; 20],
    peer_id: &[u8; 20],
    tx: mpsc::Sender<(usize, PeerIn)>,
) {
    let (out_tx, mut out_rx) = mpsc::channel::<PeerOut>(64);

    let hs = peer::handshake(&mut sock, infohash, peer_id, true).await;
    match hs {
        Ok(ok) => {
            if ok.peer_id == *peer_id {
                let _ = sock.shutdown().await;
                let _ = tx
                    .send((
                        conn_id,
                        PeerIn::Closed {
                            reason: "self".into(),
                        },
                    ))
                    .await;
                return;
            }
            let abbrev = super::peer_id::client_abbrev(&ok.peer_id);
            let ext_metadata = ok.reserved[5] & 0x10 != 0;
            if tx
                .send((
                    conn_id,
                    PeerIn::Connected {
                        abbrev,
                        peer_id: ok.peer_id,
                        out: out_tx.clone(),
                        ext_metadata,
                    },
                ))
                .await
                .is_err()
            {
                return;
            }
            peer_pump(conn_id, sock, &mut out_rx, tx).await;
        }
        Err(e) => {
            let _ = tx
                .send((
                    conn_id,
                    PeerIn::Closed {
                        reason: e.to_string(),
                    },
                ))
                .await;
        }
    }
}

async fn peer_pump(
    conn_id: usize,
    sock: TcpStream,
    out_rx: &mut mpsc::Receiver<PeerOut>,
    tx: mpsc::Sender<(usize, PeerIn)>,
) {
    let (mut rd, mut wr) = sock.into_split();
    let tx_read = tx.clone();
    let reader = tokio::spawn(async move {
        loop {
            match peer::read_msg(&mut rd).await {
                Ok(m) => {
                    if tx_read.send((conn_id, PeerIn::Msg(m))).await.is_err() {
                        return;
                    }
                }
                Err(e) => {
                    let _ = tx_read
                        .send((
                            conn_id,
                            PeerIn::Closed {
                                reason: e.to_string(),
                            },
                        ))
                        .await;
                    return;
                }
            }
        }
    });
    loop {
        match out_rx.recv().await {
            Some(PeerOut::Send(m)) => {
                let frame = peer::encode_msg(&m);
                if wr.write_all(&frame).await.is_err() {
                    let _ = tx
                        .send((
                            conn_id,
                            PeerIn::Closed {
                                reason: "写入失败".into(),
                            },
                        ))
                        .await;
                    break;
                }
                let _ = wr.flush().await;
            }
            Some(PeerOut::Drop) | None => {
                let _ = tx
                    .send((
                        conn_id,
                        PeerIn::Closed {
                            reason: "引擎断开".into(),
                        },
                    ))
                    .await;
                break;
            }
        }
    }
    drop(wr);
    let _ = tokio::time::timeout(Duration::from_secs(5), reader).await;
}

fn new_conn_stat(addr: String) -> ConnStat {
    let (dummy_tx, _dummy_rx) = mpsc::channel(1);
    ConnStat {
        peer_out: dummy_tx,
        addr,
        abbrev: String::new(),
        peer_id: None,
        has: HashSet::new(),
        inflight: HashMap::new(),
        peer_choking: true,
        am_choking: true,
        peer_interested: false,
        tit_prev: (0, std::time::Instant::now()),
        cum_down: 0,
        cum_up: 0,
        down_window: (0, std::time::Instant::now()),
        up_window: (0, std::time::Instant::now()),
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_peer_msg(
    conn_id: usize,
    msg: peer::PeerMsg,
    conns: &mut HashMap<usize, ConnStat>,
    pmap: &PieceMap,
    spec: &crate::engine::BtSpec,
    done: &mut HashSet<u32>,
    downloaded: &mut u64,
    files: &mut HashMap<usize, tokio::fs::File>,
    partial: &mut PartialPieces,
) {
    let Some(st) = conns.get_mut(&conn_id) else {
        return;
    };
    match msg {
        peer::PeerMsg::Bitfield(bits) => {
            for (i, b) in bits.iter().enumerate() {
                for bit in 0..8u32 {
                    if b & (0x80 >> bit) != 0 {
                        st.has.insert((i as u32) * 8 + bit);
                    }
                }
            }
            let want = st.has.iter().any(|p| !done.contains(p) && pmap.covers(*p));
            if want {
                send(conn_id, st, peer::PeerMsg::Interested).await;
            }
        }
        peer::PeerMsg::Have(p) => {
            st.has.insert(p);
            let want = !done.contains(&p) && pmap.covers(p);
            if want {
                send(conn_id, st, peer::PeerMsg::Interested).await;
                if !st.peer_choking {
                    // 按需计算：claimed/availability 只在确实要补请求时构建，
                    // 控制消息（KeepAlive/Request/Cancel/…）零开销直达。
                    let claimed = claimed_blocks(conns, partial);
                    let availability = availability_map(conns);
                    if let Some(st) = conns.get_mut(&conn_id) {
                        issue_requests(conn_id, st, &claimed, &availability, pmap, done, partial)
                            .await;
                    }
                }
            }
        }
        peer::PeerMsg::Unchoke => {
            st.peer_choking = false;
            let claimed = claimed_blocks(conns, partial);
            let availability = availability_map(conns);
            if let Some(st) = conns.get_mut(&conn_id) {
                issue_requests(conn_id, st, &claimed, &availability, pmap, done, partial).await;
            }
        }
        peer::PeerMsg::Choke => {
            st.peer_choking = true;
            st.inflight.clear();
        }
        peer::PeerMsg::Block { index, begin, data } => {
            on_block(
                conn_id, index, begin, data, conns, pmap, spec, done, downloaded, files, partial,
            )
            .await;
        }
        peer::PeerMsg::Request {
            index,
            begin,
            length,
        } => {
            let allowed = !st.am_choking;
            let have = done.contains(&index);
            if allowed && have && length > 0 && length as usize <= UPLOAD_BLOCK * 2 {
                if let Some(buf) =
                    read_piece_range(files, pmap, index, begin, length as usize).await
                {
                    st.cum_up += buf.len() as u64;
                    send(
                        conn_id,
                        st,
                        peer::PeerMsg::Block {
                            index,
                            begin,
                            data: buf,
                        },
                    )
                    .await;
                }
            }
        }
        peer::PeerMsg::Cancel { .. } => {}
        peer::PeerMsg::Interested => {
            st.peer_interested = true;
        }
        peer::PeerMsg::NotInterested => {
            st.peer_interested = false;
            if !st.am_choking {
                st.am_choking = true;
                send(conn_id, st, peer::PeerMsg::Choke).await;
            }
        }
        peer::PeerMsg::KeepAlive | peer::PeerMsg::Port(_) => {}
        peer::PeerMsg::Extended { ext_id, payload } => {
            if ext_id == 1 {
                if let Some(um) = peer::parse_ut_metadata(&payload) {
                    match um {
                        peer::UtMetadata::Request(piece) => {
                            let total = spec.meta.info_bytes.len() as u32;
                            let start = piece as usize * 16 * 1024;
                            if start < spec.meta.info_bytes.len() {
                                let end = (start + 16 * 1024).min(spec.meta.info_bytes.len());
                                let data = spec.meta.info_bytes[start..end].to_vec();
                                let _ = total;
                                send(
                                    conn_id,
                                    st,
                                    peer::PeerMsg::Extended {
                                        ext_id: 1,
                                        payload: peer::encode_ut_metadata(
                                            &peer::UtMetadata::Data {
                                                piece,
                                                total_size: total,
                                                data,
                                            },
                                        ),
                                    },
                                )
                                .await;
                            } else {
                                send(
                                    conn_id,
                                    st,
                                    peer::PeerMsg::Extended {
                                        ext_id: 1,
                                        payload: peer::encode_ut_metadata(
                                            &peer::UtMetadata::Reject(piece),
                                        ),
                                    },
                                )
                                .await;
                            }
                        }
                        peer::UtMetadata::Data { .. } | peer::UtMetadata::Reject(_) => {}
                    }
                }
            }
        }
        peer::PeerMsg::Unknown(..) => {}
    }
}

async fn send(_conn_id: usize, st: &mut ConnStat, m: peer::PeerMsg) {
    let _ = st.peer_out.send(PeerOut::Send(m)).await;
}

fn claimed_blocks(
    conns: &HashMap<usize, ConnStat>,
    partial: &PartialPieces,
) -> HashSet<(u32, u32)> {
    let mut claimed: HashSet<(u32, u32)> = HashSet::new();
    for c in conns.values() {
        claimed.extend(c.inflight.keys().copied());
    }
    for (piece, blocks) in partial.iter() {
        for begin in blocks.keys() {
            claimed.insert((*piece, *begin));
        }
    }
    claimed
}

fn availability_map(conns: &HashMap<usize, ConnStat>) -> HashMap<u32, usize> {
    let mut m: HashMap<u32, usize> = HashMap::new();
    for st in conns.values() {
        for &p in &st.has {
            *m.entry(p).or_insert(0) += 1;
        }
    }
    m
}

/// piece 的伪随机权重：同一 (piece, seed) 恒定不变。
/// 排序键在单次排序内必须稳定，否则 sort_by_key 的 total order
/// 前提被破坏（panic: user-provided comparison function does not
/// correctly implement a total order）。
fn jitter(piece: u32, seed: u64) -> u64 {
    let mut x = u64::from(piece).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ seed.rotate_left(17);
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    x
}

fn order_wanted(
    wanted: &[u32],
    partial: &PartialPieces,
    availability: &HashMap<u32, usize>,
) -> Vec<u32> {
    // 每轮选片取一次随机种子：排序键在一轮内恒定（total order 成立），
    // 轮间换种使同优先级 piece 的次序多样化（libtorrent 随机选片思路）。
    static ROUND: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seed = ROUND.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut v = wanted.to_vec();
    v.sort_by_key(|&p| {
        (
            std::cmp::Reverse(partial.get(&p).map_or(0, |b| b.len())),
            availability.get(&p).copied().unwrap_or(0),
            jitter(p, seed),
        )
    });
    v
}

fn endgame_active(claimed: &HashSet<(u32, u32)>, pmap: &PieceMap, done: &HashSet<u32>) -> bool {
    if done.len() >= pmap.covered_pieces().len() {
        return false;
    }
    pmap.covered_pieces().iter().all(|&p| {
        if done.contains(&p) {
            return true;
        }
        let plen = pmap.piece_len(p) as usize;
        let mut begin = 0usize;
        while begin < plen {
            if !claimed.contains(&(p, begin as u32)) {
                return false;
            }
            begin += REQUEST_BLOCK_LEN as usize;
        }
        true
    })
}

async fn issue_requests(
    conn_id: usize,
    st: &mut ConnStat,
    claimed: &HashSet<(u32, u32)>,
    availability: &HashMap<u32, usize>,
    pmap: &PieceMap,
    done: &HashSet<u32>,
    partial: &PartialPieces,
) {
    if st.peer_choking || st.inflight.len() >= PEER_PIPELINE {
        return;
    }
    let wanted_all: Vec<u32> = pmap
        .covered_pieces()
        .iter()
        .copied()
        .filter(|p| !done.contains(p) && st.has.contains(p))
        .collect();
    if wanted_all.is_empty() {
        return;
    }
    let wanted = order_wanted(&wanted_all, partial, availability);
    let mut issued = 0usize;
    for &piece in &wanted {
        issued += request_blocks_of_piece(conn_id, st, claimed, pmap, piece, false).await;
        if st.inflight.len() >= PEER_PIPELINE {
            return;
        }
    }
    if issued == 0 && endgame_active(claimed, pmap, done) {
        for &piece in &wanted {
            request_blocks_of_piece(conn_id, st, claimed, pmap, piece, true).await;
            if st.inflight.len() >= PEER_PIPELINE {
                return;
            }
        }
    }
}

async fn request_blocks_of_piece(
    conn_id: usize,
    st: &mut ConnStat,
    claimed: &HashSet<(u32, u32)>,
    pmap: &PieceMap,
    piece: u32,
    endgame: bool,
) -> usize {
    let plen = pmap.piece_len(piece) as usize;
    let mut asked = 0usize;
    let mut begin = 0usize;
    while begin < plen && asked < REQUESTS_PER_PIECE && st.inflight.len() < PEER_PIPELINE {
        let key = (piece, begin as u32);
        if st.inflight.contains_key(&key) || (!endgame && claimed.contains(&key)) {
            begin += REQUEST_BLOCK_LEN as usize;
            continue;
        }
        let len = (plen - begin).min(REQUEST_BLOCK_LEN as usize) as u32;
        st.inflight.insert(key, (len, std::time::Instant::now()));
        send(
            conn_id,
            st,
            peer::PeerMsg::Request {
                index: piece,
                begin: begin as u32,
                length: len,
            },
        )
        .await;
        asked += 1;
        begin += REQUEST_BLOCK_LEN as usize;
    }
    asked
}

fn fill_peer_slots(
    spec: &crate::engine::BtSpec,
    peer_id: &[u8; 20],
    peer_tx: &mpsc::Sender<(usize, PeerIn)>,
    shared: &EngineShared,
    conns: &mut HashMap<usize, ConnStat>,
    candidates: &mut HashMap<(IpAddr, u16), PeerCandidate>,
    next_conn: &mut usize,
) {
    // 同 IP 只连一条（libtorrent allow_multiple_connections_per_ip=false 口径）：
    // tracker 给出的 ip:listen 与对端入连的 ip:ephemeral 是同 IP 不同端口，
    // 按 ip:port 去重挡不住，必须按 IP 去重。
    let connected_ips: HashSet<IpAddr> =
        conns.values().filter_map(|st| addr_ip(&st.addr)).collect();
    while conns.len() < spec.concurrency.max(1) {
        let Some(key) = candidates
            .iter()
            .filter(|(k, c)| {
                !connected_ips.contains(&k.0) && !shared.is_blacklisted(k.0) && c.cooldown_ok()
            })
            .min_by_key(|(_, c)| (c.last_try, c.fails))
            .map(|(k, _)| *k)
        else {
            break;
        };
        if let Some(c) = candidates.get_mut(&key) {
            c.last_try = Some(std::time::Instant::now());
        }
        let (ip, port) = key;
        spawn_outgoing_peer(
            *next_conn,
            PeerAddr { ip, port },
            spec,
            peer_id,
            peer_tx.clone(),
        );
        conns.insert(*next_conn, new_conn_stat(format!("{ip}:{port}")));
        *next_conn += 1;
    }
}

async fn apply_choker(
    conns: &mut HashMap<usize, ConnStat>,
    seeding: bool,
    rotate_optimistic: bool,
    mut optimistic_slot: Option<usize>,
    optimistic_cursor: &mut usize,
) -> Option<usize> {
    let now = std::time::Instant::now();
    if seeding {
        for st in conns.values_mut() {
            if st.am_choking {
                st.am_choking = false;
                let _ = st
                    .peer_out
                    .send(PeerOut::Send(peer::PeerMsg::Unchoke))
                    .await;
            }
        }
        return None;
    }
    let mut rates: Vec<(usize, f64)> = Vec::with_capacity(conns.len());
    for (cid, st) in conns.iter_mut() {
        let dt = now.duration_since(st.tit_prev.1).as_secs_f64();
        let rate = if dt > 0.0 {
            st.cum_down.saturating_sub(st.tit_prev.0) as f64 / dt
        } else {
            0.0
        };
        st.tit_prev = (st.cum_down, now);
        if st.peer_interested {
            rates.push((*cid, rate));
        }
    }
    rates.sort_by(|a, b| b.1.total_cmp(&a.1));
    let chosen: HashSet<usize> = rates
        .iter()
        .take(UPLOAD_SLOTS)
        .map(|(cid, _)| *cid)
        .collect();
    let pool: Vec<usize> = rates
        .iter()
        .map(|(cid, _)| *cid)
        .filter(|cid| !chosen.contains(cid))
        .collect();
    if pool.is_empty() {
        optimistic_slot = None;
    } else if rotate_optimistic || !optimistic_slot.is_some_and(|s| pool.contains(&s)) {
        optimistic_slot = Some(pool[*optimistic_cursor % pool.len()]);
        *optimistic_cursor = optimistic_cursor.wrapping_add(1);
    }
    for (cid, st) in conns.iter_mut() {
        let want = chosen.contains(cid) || optimistic_slot == Some(*cid);
        if want && st.am_choking {
            st.am_choking = false;
            let _ = st
                .peer_out
                .send(PeerOut::Send(peer::PeerMsg::Unchoke))
                .await;
        } else if !want && !st.am_choking {
            st.am_choking = true;
            let _ = st.peer_out.send(PeerOut::Send(peer::PeerMsg::Choke)).await;
        }
    }
    optimistic_slot
}

#[allow(clippy::too_many_arguments)]
async fn on_block(
    conn_id: usize,
    index: u32,
    begin: u32,
    data: Vec<u8>,
    conns: &mut HashMap<usize, ConnStat>,
    pmap: &PieceMap,
    spec: &crate::engine::BtSpec,
    done: &mut HashSet<u32>,
    downloaded: &mut u64,
    files: &mut HashMap<usize, tokio::fs::File>,
    partial: &mut PartialPieces,
) {
    let mut st = match conns.remove(&conn_id) {
        Some(s) => s,
        None => return,
    };
    st.inflight.remove(&(index, begin));
    st.cum_down += data.len() as u64;
    for other in conns.values_mut() {
        if let Some((len, _)) = other.inflight.remove(&(index, begin)) {
            let _ = other
                .peer_out
                .send(PeerOut::Send(peer::PeerMsg::Cancel {
                    index,
                    begin,
                    length: len,
                }))
                .await;
        }
    }

    if done.contains(&index) {
        conns.insert(conn_id, st);
        let claimed = claimed_blocks(conns, partial);
        let availability = availability_map(conns);
        if let Some(st) = conns.get_mut(&conn_id) {
            issue_requests(conn_id, st, &claimed, &availability, pmap, done, partial).await;
        }
        return;
    }

    let plen = pmap.piece_len(index) as usize;
    let assembled = {
        let entry = partial.entry(index).or_default();
        entry.insert(begin, data);
        let mut cursor = 0usize;
        let mut buf: Vec<u8> = Vec::with_capacity(plen);
        for (off, blk) in entry.iter() {
            if *off as usize != cursor || cursor + blk.len() > plen {
                cursor = usize::MAX;
                break;
            }
            buf.extend_from_slice(blk);
            cursor += blk.len();
        }
        if cursor == plen {
            Some(buf)
        } else {
            None
        }
    };

    if let Some(buf) = assembled {
        if verify_piece(&buf, &spec.meta, index) {
            for (fi, off_in_file, off_in_piece, len) in pmap.write_plan(index) {
                if let Some(fh) = files.get_mut(&fi) {
                    use tokio::io::{AsyncSeekExt, SeekFrom};
                    let seek_ok = fh.seek(SeekFrom::Start(off_in_file)).await.is_ok();
                    let write_ok = seek_ok
                        && fh
                            .write_all(&buf[off_in_piece as usize..(off_in_piece + len) as usize])
                            .await
                            .is_ok();
                    let flush_ok = write_ok && fh.flush().await.is_ok();
                    if !flush_ok {
                        conns.insert(conn_id, st);
                        return;
                    }
                }
            }
            done.insert(index);
            *downloaded += selected_bytes_of_piece(pmap, index);
            partial.remove(&index);
            for other in conns.values_mut() {
                let _ = other
                    .peer_out
                    .send(PeerOut::Send(peer::PeerMsg::Have(index)))
                    .await;
            }
        } else {
            partial.remove(&index);
        }
    }
    conns.insert(conn_id, st);
    let claimed = claimed_blocks(conns, partial);
    let availability = availability_map(conns);
    if let Some(st) = conns.get_mut(&conn_id) {
        issue_requests(conn_id, st, &claimed, &availability, pmap, done, partial).await;
    }
}

fn file_abs_path(save_dir: &str, task_name: &str, rel: &str) -> String {
    let p = std::path::Path::new(rel);
    if p.parent().is_none() || p.components().count() == 1 {
        return format!("{save_dir}/{rel}");
    }
    format!("{save_dir}/{task_name}/{rel}")
}

/// 从盘上读一块 piece 并做 SHA-1 校验（启动重校验用）。
/// 独立成 async fn 以便在 select! 中与 Pause/Cancel 命令竞争，
/// 借用面只覆盖 files/pmap/meta，不与 done/partial 交叉。
async fn verify_piece_from_disk(
    files: &HashMap<usize, tokio::fs::File>,
    pmap: &PieceMap,
    piece: u32,
    meta: &crate::bt::metainfo::TorrentMeta,
) -> bool {
    matches!(
        read_piece_from_disk(files, pmap, piece).await,
        Ok(buf) if verify_piece(&buf, meta, piece)
    )
}

async fn read_piece_from_disk(
    files: &HashMap<usize, tokio::fs::File>,
    pmap: &PieceMap,
    piece: u32,
) -> Result<Vec<u8>, BtError> {
    let plen = pmap.piece_len(piece) as usize;
    let mut buf = vec![0u8; plen];
    let mut any = false;
    for (fi, off_in_file, off_in_piece, len) in pmap.write_plan(piece) {
        if let Some(fh) = files.get(&fi) {
            use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
            let mut fh = fh
                .try_clone()
                .await
                .map_err(|e| BtError::Io(e.to_string()))?;
            fh.seek(SeekFrom::Start(off_in_file))
                .await
                .map_err(|e| BtError::Io(e.to_string()))?;
            fh.read_exact(&mut buf[off_in_piece as usize..(off_in_piece + len) as usize])
                .await
                .map_err(|e| BtError::Io(e.to_string()))?;
            any = true;
        }
    }
    if any {
        Ok(buf)
    } else {
        Err(BtError::Io("piece 无选中文件覆盖".into()))
    }
}

fn conn_views(conns: &mut HashMap<usize, ConnStat>, covered: &[u32]) -> Vec<BtConnView> {
    let now = std::time::Instant::now();
    let mut views = Vec::with_capacity(conns.len());
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
        let down_speed = ConnStat::speed(&st.down_window, st.cum_down);
        let up_speed = ConnStat::speed(&st.up_window, st.cum_up);
        st.down_window = (st.cum_down, now);
        st.up_window = (st.cum_up, now);
        views.push(BtConnView {
            id: *cid,
            addr: st.addr.clone(),
            down_speed,
            cum_down: st.cum_down,
            up_speed,
            cum_up: st.cum_up,
            client: st.abbrev.clone(),
            seeder,
        });
    }
    views.sort_unstable_by_key(|v| v.id);
    views
}

fn save_bt_sidecar(path: &str, done: &HashSet<u32>) {
    let sc = crate::model::BtSidecar::build(done.iter().copied());
    let _ = sc.save(path);
}

fn bitfield_bytes(done: &HashSet<u32>, num_pieces: u32) -> Vec<u8> {
    if num_pieces == 0 {
        return Vec::new();
    }
    let bytes = num_pieces.div_ceil(8) as usize;
    let mut v = vec![0u8; bytes];
    for &p in done {
        let (i, bit) = (p / 8, p % 8);
        if (i as usize) < bytes {
            v[i as usize] |= 0x80 >> bit;
        }
    }
    v
}

async fn read_piece_range(
    files: &HashMap<usize, tokio::fs::File>,
    pmap: &PieceMap,
    piece: u32,
    begin: u32,
    len: usize,
) -> Option<Vec<u8>> {
    let plen = pmap.piece_len(piece) as usize;
    let begin = begin as usize;
    if begin + len > plen {
        return None;
    }
    let p_start = u64::from(piece) * u64::from(pmap.piece_length);
    let lo = p_start + begin as u64;
    let hi = lo + len as u64;
    let mut buf = vec![0u8; len];
    let mut filled = 0usize;
    for (fi, off_in_file, off_in_piece, seg_len) in pmap.write_plan(piece) {
        let seg_lo = p_start + off_in_piece;
        let seg_hi = seg_lo + seg_len;
        let o_lo = lo.max(seg_lo);
        let o_hi = hi.min(seg_hi);
        if o_lo >= o_hi {
            continue;
        }
        let dst = (o_lo - lo) as usize;
        let n = (o_hi - o_lo) as usize;
        let fh = files.get(&fi)?.try_clone().await.ok()?;
        use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
        let mut fh = fh;
        fh.seek(SeekFrom::Start(off_in_file + (o_lo - seg_lo)))
            .await
            .ok()?;
        fh.read_exact(&mut buf[dst..dst + n]).await.ok()?;
        filled += n;
    }
    if filled == len {
        Some(buf)
    } else {
        None
    }
}

fn verify_piece(buf: &[u8], meta: &TorrentMeta, piece: u32) -> bool {
    meta.pieces.get(piece as usize).is_some_and(|expect| {
        let mut h = Sha1::new();
        h.update(buf);
        h.finalize().as_slice() == expect
    })
}

fn selected_bytes_of_piece(pmap: &PieceMap, piece: u32) -> u64 {
    pmap.write_plan(piece).iter().map(|(.., len)| *len).sum()
}

fn partial_piece_list(partial: &PartialPieces, done: &HashSet<u32>) -> Vec<u32> {
    // 黄色集合：已收到部分块、尚未验完的 piece（引擎 partial 内存表）。
    let mut v: Vec<u32> = partial
        .keys()
        .filter(|p| !done.contains(p))
        .copied()
        .collect();
    v.sort_unstable();
    v
}

fn inflight_piece_list(conns: &HashMap<usize, ConnStat>, done: &HashSet<u32>) -> Vec<u32> {
    // 蓝色集合：有在途请求的 piece（正在下载）。
    let mut set: HashSet<u32> = HashSet::new();
    for st in conns.values() {
        for (p, _) in st.inflight.keys() {
            if !done.contains(p) {
                set.insert(*p);
            }
        }
    }
    let mut v: Vec<u32> = set.into_iter().collect();
    v.sort_unstable();
    v
}

fn uploaded_total(conns: &HashMap<usize, ConnStat>) -> u64 {
    conns.values().map(|s| s.cum_up).sum()
}

fn all_selected_done(done: &HashSet<u32>, pmap: &PieceMap) -> bool {
    !pmap.covered_pieces().is_empty() && pmap.covered_pieces().iter().all(|p| done.contains(p))
}

async fn report_fail(evt_tx: &mpsc::Sender<Evt>, id: u32, e: BtError) {
    let _ = evt_tx
        .send(Evt::BtFailed {
            id,
            kind: crate::model::FailKind::Transient,
            reason: e.to_string(),
        })
        .await;
}

async fn report_stopped(
    evt_tx: &mpsc::Sender<Evt>,
    id: u32,
    done: &HashSet<u32>,
    partial: &PartialPieces,
    downloaded: u64,
) {
    let _ = evt_tx
        .send(Evt::BtStopped {
            id,
            downloaded,
            done_pieces: done.iter().copied().collect::<Vec<_>>(),
            partial_pieces: partial_piece_list(partial, done),
        })
        .await;
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tokio::io::AsyncReadExt;

    use super::*;
    use crate::bt::bencode::{encode, BValue};
    use crate::model::BtFile;

    fn make_meta(data: &[u8], piece_len: usize, tracker: &str) -> TorrentMeta {
        let mut pieces = Vec::new();
        for chunk in data.chunks(piece_len) {
            let mut h = Sha1::new();
            h.update(chunk);
            pieces.extend_from_slice(&h.finalize());
        }
        let mut info = BTreeMap::new();
        info.insert(b"length".to_vec(), BValue::Int(data.len() as i64));
        info.insert(b"name".to_vec(), BValue::Bytes(b"e2e.bin".to_vec()));
        info.insert(b"piece length".to_vec(), BValue::Int(piece_len as i64));
        info.insert(b"pieces".to_vec(), BValue::Bytes(pieces));
        let mut root = BTreeMap::new();
        root.insert(
            b"announce".to_vec(),
            BValue::Bytes(tracker.as_bytes().to_vec()),
        );
        root.insert(b"info".to_vec(), BValue::Dict(info));
        TorrentMeta::parse(&encode(&BValue::Dict(root))).unwrap()
    }

    async fn fake_udp_tracker(seeder_port: u16) -> u16 {
        let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let port = sock.local_addr().unwrap().port();
        tokio::spawn(async move {
            let mut buf = [0u8; 512];
            loop {
                let (n, from) = match sock.recv_from(&mut buf).await {
                    Ok(x) => x,
                    Err(_) => return,
                };
                let req = &buf[..n];
                if req.len() < 16 {
                    continue;
                }
                let action = u32::from_be_bytes(req[8..12].try_into().unwrap());
                let txid = u32::from_be_bytes(req[12..16].try_into().unwrap());
                if action == 0 {
                    let magic = u64::from_be_bytes(req[0..8].try_into().unwrap());
                    if magic != 0x0417_2710_1980 {
                        continue;
                    }
                } else if action == 1 {
                    let conn_id = u64::from_be_bytes(req[0..8].try_into().unwrap());
                    if conn_id != 0x1234_5678_9abc_def0 {
                        continue;
                    }
                } else {
                    continue;
                }
                if action == 0 {
                    let mut out = Vec::new();
                    out.extend_from_slice(&0u32.to_be_bytes());
                    out.extend_from_slice(&txid.to_be_bytes());
                    out.extend_from_slice(&0x1234_5678_9abc_def0u64.to_be_bytes());
                    let _ = sock.send_to(&out, from).await;
                } else if action == 1 {
                    let mut out = Vec::new();
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&txid.to_be_bytes());
                    out.extend_from_slice(&60u32.to_be_bytes());
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&[127, 0, 0, 1]);
                    out.extend_from_slice(&seeder_port.to_be_bytes());
                    let _ = sock.send_to(&out, from).await;
                }
            }
        });
        port
    }

    async fn fake_seeder(data: Vec<u8>, piece_len: usize, infohash: [u8; 20]) -> u16 {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut hs = [0u8; 68];
            sock.read_exact(&mut hs).await.unwrap();
            assert_eq!(&hs[1..20], b"BitTorrent protocol");
            assert_eq!(&hs[28..48], &infohash);
            let mut resp = Vec::new();
            resp.push(19);
            resp.extend_from_slice(b"BitTorrent protocol");
            resp.extend_from_slice(&[0u8; 8]);
            resp.extend_from_slice(&infohash);
            resp.extend_from_slice(b"-QX7777-seederpeer00");
            sock.write_all(&resp).await.unwrap();
            let n = data.len().div_ceil(piece_len);
            let mut bf = vec![0u8; n.div_ceil(8)];
            for i in 0..n {
                bf[i / 8] |= 0x80 >> (i % 8);
            }
            let mut frame = Vec::new();
            frame.extend_from_slice(&((1 + bf.len()) as u32).to_be_bytes());
            frame.push(5);
            frame.extend_from_slice(&bf);
            sock.write_all(&frame).await.unwrap();
            sock.write_all(&[0, 0, 0, 1, 1]).await.unwrap();
            loop {
                match peer::read_msg(&mut sock).await {
                    Ok(peer::PeerMsg::Request {
                        index,
                        begin,
                        length,
                    }) => {
                        let off = index as usize * piece_len + begin as usize;
                        let end = (off + length as usize).min(data.len());
                        let blk = data[off..end].to_vec();
                        let mut f = Vec::new();
                        f.extend_from_slice(&((9 + blk.len()) as u32).to_be_bytes());
                        f.push(7);
                        f.extend_from_slice(&index.to_be_bytes());
                        f.extend_from_slice(&begin.to_be_bytes());
                        f.extend_from_slice(&blk);
                        if sock.write_all(&f).await.is_err() {
                            return;
                        }
                    }
                    Ok(peer::PeerMsg::Interested) => {}
                    Ok(peer::PeerMsg::KeepAlive) => {}
                    Ok(_) => {}
                    Err(_) => return,
                }
            }
        });
        port
    }

    /// 复刻 fake_udp_tracker，额外把 announce 包内的 engine 监听端口
    /// （BEP 15 announce 第 96..98 字节）回报给测试，供 dup peer 拨入制造
    /// 同 peer_id 重复连接。
    async fn fake_udp_tracker_report_engine_port(
        seeder_port: u16,
        port_tx: tokio::sync::mpsc::UnboundedSender<u16>,
    ) -> u16 {
        let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let port = sock.local_addr().unwrap().port();
        tokio::spawn(async move {
            let mut buf = [0u8; 512];
            loop {
                let (n, from) = match sock.recv_from(&mut buf).await {
                    Ok(x) => x,
                    Err(_) => return,
                };
                let req = &buf[..n];
                if req.len() < 16 {
                    continue;
                }
                let action = u32::from_be_bytes(req[8..12].try_into().unwrap());
                let txid = u32::from_be_bytes(req[12..16].try_into().unwrap());
                if action == 0 {
                    let magic = u64::from_be_bytes(req[0..8].try_into().unwrap());
                    if magic != 0x0417_2710_1980 {
                        continue;
                    }
                } else if action == 1 {
                    let conn_id = u64::from_be_bytes(req[0..8].try_into().unwrap());
                    if conn_id != 0x1234_5678_9abc_def0 {
                        continue;
                    }
                } else {
                    continue;
                }
                if action == 0 {
                    let mut out = Vec::new();
                    out.extend_from_slice(&0u32.to_be_bytes());
                    out.extend_from_slice(&txid.to_be_bytes());
                    out.extend_from_slice(&0x1234_5678_9abc_def0u64.to_be_bytes());
                    let _ = sock.send_to(&out, from).await;
                } else if action == 1 {
                    if req.len() >= 98 {
                        let engine_port = u16::from_be_bytes(req[96..98].try_into().unwrap());
                        let _ = port_tx.send(engine_port);
                    }
                    let mut out = Vec::new();
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&txid.to_be_bytes());
                    out.extend_from_slice(&60u32.to_be_bytes());
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&[127, 0, 0, 1]);
                    out.extend_from_slice(&seeder_port.to_be_bytes());
                    let _ = sock.send_to(&out, from).await;
                }
            }
        });
        port
    }

    const DUP_PEER_ID: &[u8; 20] = b"-QX7777-duppeerid000";

    async fn write_seeder_hs(sock: &mut TcpStream, infohash: &[u8; 20]) {
        let mut resp = Vec::new();
        resp.push(19);
        resp.extend_from_slice(b"BitTorrent protocol");
        resp.extend_from_slice(&[0u8; 8]);
        resp.extend_from_slice(infohash);
        resp.extend_from_slice(DUP_PEER_ID);
        let _ = sock.write_all(&resp).await;
    }

    async fn serve_as_seeder(
        mut sock: TcpStream,
        data: Vec<u8>,
        piece_len: usize,
        infohash: [u8; 20],
    ) {
        let mut hs = [0u8; 68];
        if sock.read_exact(&mut hs).await.is_err() {
            return;
        }
        write_seeder_hs(&mut sock, &infohash).await;
        let n = data.len().div_ceil(piece_len);
        let mut bf = vec![0u8; n.div_ceil(8)];
        for i in 0..n {
            bf[i / 8] |= 0x80 >> (i % 8);
        }
        let mut frame = Vec::new();
        frame.extend_from_slice(&((1 + bf.len()) as u32).to_be_bytes());
        frame.push(5);
        frame.extend_from_slice(&bf);
        if sock.write_all(&frame).await.is_err() {
            return;
        }
        let _ = sock.write_all(&[0, 0, 0, 1, 1]).await;
        loop {
            match peer::read_msg(&mut sock).await {
                Ok(peer::PeerMsg::Request {
                    index,
                    begin,
                    length,
                }) => {
                    let off = index as usize * piece_len + begin as usize;
                    let end = (off + length as usize).min(data.len());
                    let blk = data[off..end].to_vec();
                    let mut f = Vec::new();
                    f.extend_from_slice(&((9 + blk.len()) as u32).to_be_bytes());
                    f.push(7);
                    f.extend_from_slice(&index.to_be_bytes());
                    f.extend_from_slice(&begin.to_be_bytes());
                    f.extend_from_slice(&blk);
                    if sock.write_all(&f).await.is_err() {
                        return;
                    }
                }
                Ok(_) => {}
                Err(_) => return,
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn engine_dedups_same_peer_id_duplicate_connection() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..40_960usize).map(|i| (i % 251) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-dup");
        let save_dir = dir.to_string_lossy().into_owned();

        let tmp_meta = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dup_port = listener.local_addr().unwrap().port();
        let (port_tx, mut port_rx) = tokio::sync::mpsc::unbounded_channel();
        let infohash = tmp_meta.infohash;
        let data_clone = data.clone();
        tokio::spawn(async move {
            // 1) 拿到 engine 监听端口后以同一 peer_id 拨入，制造重复连接
            // 2) 接受 ezr 出连（正常 seeder 会话）；两条会话均可服务下载，
            //    无论哪条在去重后幸存，下载都能完成。
            let engine_port = port_rx.recv().await.unwrap();
            tokio::time::sleep(Duration::from_millis(300)).await;
            if let Ok(dup) = TcpStream::connect(("127.0.0.1", engine_port)).await {
                tokio::spawn(serve_as_seeder(
                    dup,
                    data_clone.clone(),
                    piece_len,
                    infohash,
                ));
            }
            loop {
                let Ok((sock, _)) = listener.accept().await else {
                    return;
                };
                tokio::spawn(serve_as_seeder(
                    sock,
                    data_clone.clone(),
                    piece_len,
                    infohash,
                ));
            }
        });
        let tracker_port = fake_udp_tracker_report_engine_port(dup_port, port_tx).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );
        assert_eq!(meta.infohash, tmp_meta.infohash, "两次构造 infohash 一致");

        let spec = crate::engine::BtSpec {
            id: 79,
            source: "/tmp/dup.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "dup.bin".into(),
            files: vec![BtFile {
                path: "dup.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 4,
            proxy_endpoint: None,
            seeding: false,
            max_speed: 0,
        };
        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let shared = crate::engine::shared_for_tests();
        let engine_handle = tokio::spawn(run(spec, shared, cmd_rx, evt_tx));

        let mut last_conn_rows = 0usize;
        let mut done = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            let remain = deadline.saturating_duration_since(std::time::Instant::now());
            let Some(evt) = tokio::time::timeout(remain, evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                break;
            };
            match evt {
                Evt::BtConns { conns, .. } => last_conn_rows = conns.len(),
                Evt::BtDone { .. } => {
                    done = true;
                    break;
                }
                Evt::BtFailed { reason, .. } => panic!("意外失败：{reason}"),
                _ => {}
            }
        }
        assert!(done, "重复连接被去重后下载仍须完成");
        assert_eq!(
            last_conn_rows, 1,
            "同 peer_id 重复连接只保留一条（并发 panel 不出现同 IP 两行）"
        );
        engine_handle.abort();
        let saved = tokio::fs::read(format!("{save_dir}/dup.bin"))
            .await
            .unwrap();
        assert_eq!(saved, data, "产物字节一致");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn engine_downloads_via_udp_tracker_and_seeder() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..40_960usize).map(|i| (i % 251) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-e2e");
        let save_dir = dir.to_string_lossy().into_owned();

        let mut hasher = Sha1::new();
        hasher.update(b"placeholder");
        let _ = hasher;
        let tmp_meta = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let seeder_port = fake_seeder(data.clone(), piece_len, tmp_meta.infohash).await;
        let tracker_port = fake_udp_tracker(seeder_port).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );
        assert_eq!(meta.infohash, tmp_meta.infohash, "两次构造 infohash 一致");

        let spec = crate::engine::BtSpec {
            id: 77,
            source: "/tmp/e2e.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "e2e.bin".into(),
            files: vec![BtFile {
                path: "e2e.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 4,
            proxy_endpoint: None,
            seeding: false,
            max_speed: 0,
        };
        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let shared = crate::engine::shared_for_tests();
        let engine_handle = tokio::spawn(run(spec, shared, cmd_rx, evt_tx));

        let mut saw_progress = false;
        let mut saw_conns = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            let remain = deadline.saturating_duration_since(std::time::Instant::now());
            let Some(evt) = tokio::time::timeout(remain, evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                if engine_handle.is_finished() {
                    let e = ();
                    engine_handle.await.expect("join");
                    panic!("引擎提前退出：{e:?} saw_progress={saw_progress} saw_conns={saw_conns}");
                }
                panic!("超时未完成：saw_progress={saw_progress} saw_conns={saw_conns}");
            };
            match evt {
                Evt::BtProgress { downloaded, .. } => {
                    if downloaded > 0 {
                        saw_progress = true;
                    }
                }
                Evt::BtConns { conns, .. } => {
                    if !conns.is_empty() {
                        saw_conns = true;
                    }
                }
                Evt::BtDone { id, total } => {
                    assert_eq!(id, 77);
                    assert_eq!(total, data.len() as u64);
                    break;
                }
                Evt::BtFailed { reason, .. } => panic!("意外失败：{reason}"),
                _ => {}
            }
        }
        assert!(saw_progress, "进度事件有落盘量");
        assert!(saw_conns, "连接明细事件在位");
        let out = std::fs::read(format!("{save_dir}/e2e.bin")).unwrap();
        assert_eq!(out, data, "产物与 seeder 数据逐字节一致");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn engine_downloads_multi_block_pieces() {
        let piece_len = 64 * 1024;
        let data: Vec<u8> = (0..160_000usize).map(|i| (i % 253) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-multipiece");
        let save_dir = dir.to_string_lossy().into_owned();

        let tmp_meta = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let seeder_port = fake_seeder(data.clone(), piece_len, tmp_meta.infohash).await;
        let tracker_port = fake_udp_tracker(seeder_port).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );
        assert_eq!(meta.infohash, tmp_meta.infohash, "两次构造 infohash 一致");

        let spec = crate::engine::BtSpec {
            id: 78,
            source: "/tmp/e2e-multi.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "e2e.bin".into(),
            files: vec![BtFile {
                path: "e2e.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 4,
            proxy_endpoint: None,
            seeding: false,
            max_speed: 0,
        };
        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let shared = crate::engine::shared_for_tests();
        let engine_handle = tokio::spawn(run(spec, shared, cmd_rx, evt_tx));

        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            let remain = deadline.saturating_duration_since(std::time::Instant::now());
            let Some(evt) = tokio::time::timeout(remain, evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                if engine_handle.is_finished() {
                    engine_handle.await.expect("join");
                }
                panic!("多块 piece 未在时限内完成下载");
            };
            match evt {
                Evt::BtDone { id, total } => {
                    assert_eq!(id, 78);
                    assert_eq!(total, data.len() as u64);
                    break;
                }
                Evt::BtFailed { reason, .. } => panic!("意外失败：{reason}"),
                _ => {}
            }
        }
        let out = std::fs::read(format!("{save_dir}/e2e.bin")).unwrap();
        assert_eq!(out, data, "多块 piece 产物与 seeder 数据逐字节一致");
        let _ = std::fs::remove_dir_all(&dir);
    }

    async fn fake_udp_tracker_fast(seeder_port: u16) -> u16 {
        let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let port = sock.local_addr().unwrap().port();
        tokio::spawn(async move {
            let mut buf = [0u8; 512];
            loop {
                let (n, from) = match sock.recv_from(&mut buf).await {
                    Ok(x) => x,
                    Err(_) => return,
                };
                let req = &buf[..n];
                if req.len() < 16 {
                    continue;
                }
                let action = u32::from_be_bytes(req[8..12].try_into().unwrap());
                let txid = u32::from_be_bytes(req[12..16].try_into().unwrap());
                if action == 0 {
                    if u64::from_be_bytes(req[0..8].try_into().unwrap()) != 0x0417_2710_1980 {
                        continue;
                    }
                    let mut out = Vec::new();
                    out.extend_from_slice(&0u32.to_be_bytes());
                    out.extend_from_slice(&txid.to_be_bytes());
                    out.extend_from_slice(&0x1234_5678_9abc_def0u64.to_be_bytes());
                    let _ = sock.send_to(&out, from).await;
                } else if action == 1 {
                    if u64::from_be_bytes(req[0..8].try_into().unwrap()) != 0x1234_5678_9abc_def0 {
                        continue;
                    }
                    let mut out = Vec::new();
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&txid.to_be_bytes());
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&[127, 0, 0, 1]);
                    out.extend_from_slice(&seeder_port.to_be_bytes());
                    let _ = sock.send_to(&out, from).await;
                }
            }
        });
        port
    }

    async fn counting_seeder(
        data: Vec<u8>,
        piece_len: usize,
        infohash: [u8; 20],
        counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) -> u16 {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let data = std::sync::Arc::new(data);
        tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = listener.accept().await else {
                    return;
                };
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let data = data.clone();
                tokio::spawn(async move {
                    serve_seeder_session(sock, &data, piece_len, infohash).await;
                });
            }
        });
        port
    }

    async fn serve_seeder_session(
        mut sock: TcpStream,
        data: &[u8],
        piece_len: usize,
        infohash: [u8; 20],
    ) {
        let mut hs = [0u8; 68];
        if sock.read_exact(&mut hs).await.is_err() {
            return;
        }
        let mut resp = Vec::new();
        resp.push(19);
        resp.extend_from_slice(b"BitTorrent protocol");
        resp.extend_from_slice(&[0u8; 8]);
        resp.extend_from_slice(&infohash);
        resp.extend_from_slice(b"-QX7777-seederpeer00");
        if sock.write_all(&resp).await.is_err() {
            return;
        }
        let n = data.len().div_ceil(piece_len);
        let mut bf = vec![0u8; n.div_ceil(8)];
        for i in 0..n {
            bf[i / 8] |= 0x80 >> (i % 8);
        }
        let mut frame = Vec::new();
        frame.extend_from_slice(&((1 + bf.len()) as u32).to_be_bytes());
        frame.push(5);
        frame.extend_from_slice(&bf);
        let _ = sock.write_all(&frame).await;
        let _ = sock.write_all(&[0, 0, 0, 1, 1]).await;
        loop {
            match peer::read_msg(&mut sock).await {
                Ok(peer::PeerMsg::Request {
                    index,
                    begin,
                    length,
                }) => {
                    let off = index as usize * piece_len + begin as usize;
                    let end = (off + length as usize).min(data.len());
                    if off >= data.len() {
                        return;
                    }
                    let blk = data[off..end].to_vec();
                    let mut f = Vec::new();
                    f.extend_from_slice(&((9 + blk.len()) as u32).to_be_bytes());
                    f.push(7);
                    f.extend_from_slice(&index.to_be_bytes());
                    f.extend_from_slice(&begin.to_be_bytes());
                    f.extend_from_slice(&blk);
                    if sock.write_all(&f).await.is_err() {
                        return;
                    }
                }
                Ok(_) => {}
                Err(_) => return,
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn disconnect_blacklists_peer_and_refuses_after_restart() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..40_960usize).map(|i| (i % 241) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-blacklist");
        let save_dir = dir.to_string_lossy().into_owned();

        let counter = std::sync::Arc::new(AtomicUsize::new(0));
        let meta0 = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let seeder_port =
            counting_seeder(data.clone(), piece_len, meta0.infohash, counter.clone()).await;
        let tracker_port = fake_udp_tracker_fast(seeder_port).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );

        let shared = crate::engine::shared_for_tests();
        let make_spec = |id: u32| crate::engine::BtSpec {
            id,
            source: "/tmp/e2e-bl.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "e2e.bin".into(),
            files: vec![BtFile {
                path: "e2e.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 4,
            proxy_endpoint: None,
            seeding: false,
            max_speed: 0,
        };

        let (evt_tx1, mut evt_rx1) = mpsc::channel(256);
        let (cmd_tx1, cmd_rx1) = mpsc::channel::<BtTaskCmd>(8);
        let run1 = tokio::spawn(run(make_spec(91), shared.clone(), cmd_rx1, evt_tx1));

        let conn_id = {
            let deadline = std::time::Instant::now() + Duration::from_secs(15);
            loop {
                assert!(std::time::Instant::now() < deadline, "首轮未建立连接");
                let Some(evt) = tokio::time::timeout(Duration::from_millis(500), evt_rx1.recv())
                    .await
                    .ok()
                    .flatten()
                else {
                    continue;
                };
                if let Evt::BtConns { conns, .. } = evt {
                    if let Some(v) = conns.first() {
                        break v.id;
                    }
                }
            }
        };
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "黑名单生效前 seeder 恰被连一次"
        );

        cmd_tx1.send(BtTaskCmd::Disconnect(conn_id)).await.unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if shared.is_blacklisted(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Disconnect 应写入全局黑名单"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        cmd_tx1.send(BtTaskCmd::Cancel).await.unwrap();
        let _ = tokio::time::timeout(Duration::from_secs(5), run1).await;

        let (evt_tx2, mut evt_rx2) = mpsc::channel(256);
        let (_cmd_tx2, cmd_rx2) = mpsc::channel::<BtTaskCmd>(8);
        let run2 = tokio::spawn(run(make_spec(92), shared.clone(), cmd_rx2, evt_tx2));

        let mut saw_announce = false;
        let mut saw_conn = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(4);
        while std::time::Instant::now() < deadline {
            let Some(evt) = tokio::time::timeout(Duration::from_millis(300), evt_rx2.recv())
                .await
                .ok()
                .flatten()
            else {
                continue;
            };
            match evt {
                Evt::BtPeers { .. } => saw_announce = true,
                Evt::BtConns { conns, .. } if !conns.is_empty() => saw_conn = true,
                _ => {}
            }
        }
        assert!(saw_announce, "重启轮 tracker 正常返回 peers");
        assert!(
            !saw_conn,
            "黑名单内 peer 重启后仍被拒绝连接（不建立新连接）"
        );
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "seeder 连接计数保持 1（无重连）"
        );
        run2.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn read_piece_range_multi_file_reads_only_requested_slice() {
        let files = vec![
            BtFile {
                path: "a.bin".into(),
                size: 100,
                selected: true,
            },
            BtFile {
                path: "b.bin".into(),
                size: 100,
                selected: true,
            },
        ];
        let pmap = PieceMap::new(&files, 64);
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-rpr");
        let a: Vec<u8> = (0..100u32).map(|i| (i % 251) as u8).collect();
        let b: Vec<u8> = (100..200u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(dir.join("a.bin"), &a).unwrap();
        std::fs::write(dir.join("b.bin"), &b).unwrap();
        let mut handles = HashMap::new();
        for (i, name) in ["a.bin", "b.bin"].into_iter().enumerate() {
            let fh = tokio::fs::OpenOptions::new()
                .read(true)
                .open(dir.join(name))
                .await
                .unwrap();
            handles.insert(i, fh);
        }
        let out = read_piece_range(&handles, &pmap, 1, 32, 32)
            .await
            .expect("跨文件区间可读");
        let mut expect = Vec::new();
        expect.extend_from_slice(&a[96..100]);
        expect.extend_from_slice(&b[0..28]);
        assert_eq!(out, expect, "仅读请求区间（piece1 全局 [96..128)）");
        assert!(
            read_piece_range(&handles, &pmap, 1, 60, 8).await.is_none(),
            "越界区间拒绝"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bt_sidecar_written_at_start_with_task_shape() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..20_480usize).map(|i| (i % 199) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btsc-start");
        let save_dir = dir.to_string_lossy().into_owned();

        let seeder_port = fake_udp_tracker(1).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{seeder_port}/announce"),
        );
        let spec = crate::engine::BtSpec {
            id: 91,
            source: "/tmp/sc.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "sc.bin".into(),
            files: vec![BtFile {
                path: "sc.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{seeder_port}/announce")],
            concurrency: 2,
            proxy_endpoint: None,
            seeding: false,
            max_speed: 0,
        };
        let sc_path = crate::model::namegen::sidecar_path_of(&save_dir, "sc.bin");
        let (evt_tx, _evt_rx) = mpsc::channel(256);
        let (cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let shared = crate::engine::shared_for_tests();
        let handle = tokio::spawn(run(spec, shared, cmd_rx, evt_tx));

        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            if std::path::Path::new(&sc_path).exists() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "任务启动即写 .ezr 控制文件"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let text = std::fs::read_to_string(&sc_path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["version"], crate::model::bt::BT_SIDECAR_VERSION);
        assert_eq!(
            crate::model::BtSidecar::load_done(&sc_path),
            Some(Vec::<u32>::new()),
            "启动时无已完成 piece（进度唯一归属 .ezr，任务定义留在注册表）"
        );

        cmd_tx.send(BtTaskCmd::Cancel).await.unwrap();
        let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        assert!(
            std::path::Path::new(&sc_path).exists(),
            "Cancel（任务仍保留）不删 sidecar"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bt_sidecar_prefill_resumes_and_removed_on_completion() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..20_480usize).map(|i| (i % 199) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btsc-resume");
        let save_dir = dir.to_string_lossy().into_owned();

        let tmp_meta = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        std::fs::write(format!("{save_dir}/sc2.bin"), &data).unwrap();
        let seeder_port = fake_seeder(data.clone(), piece_len, tmp_meta.infohash).await;
        let tracker_port = fake_udp_tracker(seeder_port).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );
        assert_eq!(meta.infohash, tmp_meta.infohash);
        let num_pieces = data.len().div_ceil(piece_len) as u32;

        let sc_path = crate::model::namegen::sidecar_path_of(&save_dir, "sc2.bin");
        crate::model::BtSidecar::build(0..num_pieces)
            .save(&sc_path)
            .unwrap();

        let spec = crate::engine::BtSpec {
            id: 93,
            source: "/tmp/sc2.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "sc2.bin".into(),
            files: vec![BtFile {
                path: "sc2.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 4,
            proxy_endpoint: None,
            seeding: false,
            max_speed: 0,
        };
        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let shared = crate::engine::shared_for_tests();
        let handle = tokio::spawn(run(spec, shared, cmd_rx, evt_tx));

        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            let remain = deadline.saturating_duration_since(std::time::Instant::now());
            let Some(evt) = tokio::time::timeout(remain, evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                panic!("sidecar 全量预填应秒完成（磁盘校验通过）");
            };
            match evt {
                Evt::BtDone { id, total } => {
                    assert_eq!(id, 93);
                    assert_eq!(total, data.len() as u64);
                    break;
                }
                Evt::BtFailed { reason, .. } => panic!("意外失败：{reason}"),
                _ => {}
            }
        }
        assert!(
            !std::path::Path::new(&sc_path).exists(),
            "完成后 .ezr 控制文件删除"
        );
        handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bt_sidecar_pause_keeps_resume_state() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..1_572_864usize).map(|i| (i % 199) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btsc-pause");
        let save_dir = dir.to_string_lossy().into_owned();

        let tmp_meta = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let seeder_port = fake_seeder(data.clone(), piece_len, tmp_meta.infohash).await;
        let tracker_port = fake_udp_tracker(seeder_port).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );
        let num_pieces = data.len().div_ceil(piece_len) as u32;
        let half = num_pieces / 2;

        let spec = crate::engine::BtSpec {
            id: 94,
            source: "/tmp/sc3.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "sc3.bin".into(),
            files: vec![BtFile {
                path: "sc3.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 4,
            proxy_endpoint: None,
            seeding: false,
            max_speed: 0,
        };
        let sc_path = crate::model::namegen::sidecar_path_of(&save_dir, "sc3.bin");
        std::fs::write(format!("{save_dir}/sc3.bin"), &data).unwrap();
        crate::model::BtSidecar::build(0..half)
            .save(&sc_path)
            .unwrap();

        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let shared = crate::engine::shared_for_tests();
        let handle = tokio::spawn(run(spec, shared, cmd_rx, evt_tx));
        // 立即 Pause：无论落在启动重校验中途（保存 已校验∪未校验余量）还是
        // 重校验后的主循环（保存全量 done），sidecar 都必须保全完整预填集。
        cmd_tx.send(BtTaskCmd::Pause).await.unwrap();

        let mut stopped = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while std::time::Instant::now() < deadline {
            let remain = deadline.saturating_duration_since(std::time::Instant::now());
            let Some(evt) = tokio::time::timeout(remain, evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                break;
            };
            if matches!(evt, Evt::BtStopped { .. }) {
                stopped = true;
                break;
            }
        }
        assert!(stopped, "暂停后引擎回报 BtStopped");
        assert_eq!(
            crate::model::BtSidecar::load_done(&sc_path).map(|p| p.len()),
            Some(half as usize),
            "Pause 分支保存的 done_pieces = 磁盘校验通过量（sidecar 预填 ∪ 引擎进度）"
        );
        let _ = handle.await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bt_conns_mark_full_bitfield_peer_as_seeder() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..20_480usize).map(|i| (i % 199) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btsc-seederflag");
        let save_dir = dir.to_string_lossy().into_owned();

        let tmp_meta = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let seeder_port = fake_seeder(data.clone(), piece_len, tmp_meta.infohash).await;
        let tracker_port = fake_udp_tracker(seeder_port).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );

        let spec = crate::engine::BtSpec {
            id: 95,
            source: "/tmp/sc4.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "sc4.bin".into(),
            files: vec![BtFile {
                path: "sc4.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 4,
            proxy_endpoint: None,
            seeding: false,
            max_speed: 0,
        };
        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let shared = crate::engine::shared_for_tests();
        let handle = tokio::spawn(run(spec, shared, cmd_rx, evt_tx));

        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        let mut saw_seeder = false;
        let mut saw_leecher = false;
        loop {
            let remain = deadline.saturating_duration_since(std::time::Instant::now());
            let Some(evt) = tokio::time::timeout(remain, evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                break;
            };
            match evt {
                Evt::BtConns { conns, .. } if !conns.is_empty() => {
                    for c in &conns {
                        if c.seeder {
                            saw_seeder = true;
                        } else {
                            saw_leecher = true;
                        }
                    }
                    if saw_seeder {
                        break;
                    }
                }
                Evt::BtDone { .. } => break,
                Evt::BtFailed { reason, .. } => panic!("意外失败：{reason}"),
                _ => {}
            }
        }
        assert!(saw_seeder, "全量 bitfield peer（fake seeder）标记为 seeder");
        let _ = saw_leecher;
        handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn picker_prefers_partial_piece_closest_to_completion() {
        let mut partial: PartialPieces = HashMap::new();
        partial.insert(3, BTreeMap::from([(0u32, vec![0u8; 16])]));
        let availability = HashMap::from([(1u32, 1usize), (3u32, 5usize)]);
        let order = order_wanted(&[1, 3], &partial, &availability);
        assert_eq!(
            order.first(),
            Some(&3),
            "部分到达的 piece 优先（尽管它更常见）"
        );
    }

    #[test]
    fn picker_rarest_first_among_equally_partial() {
        let partial: PartialPieces = HashMap::new();
        let availability = HashMap::from([(7u32, 3usize), (9u32, 1usize)]);
        let order = order_wanted(&[7, 9], &partial, &availability);
        assert_eq!(order, vec![9, 7], "稀有度低（副本少）的 piece 先选");
    }

    #[test]
    fn jitter_is_deterministic_per_seed() {
        assert_eq!(jitter(7, 42), jitter(7, 42), "同 (piece, seed) 键必须恒定");
        assert_ne!(jitter(7, 1), jitter(7, 2), "不同种子应产生不同键");
    }

    #[test]
    fn order_wanted_large_input_total_order() {
        // 回归：jitter 曾用全局自增计数器作排序键，键在排序过程中漂移，
        // 触发 sort 的 total order 断言 panic（引擎内部错误→界面花屏）。
        // 大输入下排序应稳定完成，且结果是 wanted 的一个排列（不丢不重）。
        let partial: PartialPieces = HashMap::new();
        let availability: HashMap<u32, usize> = HashMap::new();
        let wanted: Vec<u32> = (0..8192u32).collect();
        let order = order_wanted(&wanted, &partial, &availability);
        let mut as_multiset = order.clone();
        as_multiset.sort_unstable();
        assert_eq!(
            as_multiset, wanted,
            "排序结果必须是 wanted 的排列（比较器满足 total order）"
        );
    }

    #[test]
    fn endgame_active_requires_every_block_claimed() {
        let files = vec![BtFile {
            path: "a.bin".into(),
            size: 64 * 1024,
            selected: true,
        }];
        let pmap = PieceMap::new(&files, 32 * 1024);
        let done: HashSet<u32> = HashSet::new();
        let claimed: HashSet<(u32, u32)> = HashSet::from([(0, 0)]);
        assert!(
            !endgame_active(&claimed, &pmap, &done),
            "存在未认领块时不进 endgame"
        );
        let full: HashSet<(u32, u32)> = [(0u32, 0u32), (0, 16 * 1024), (1, 0), (1, 16 * 1024)]
            .into_iter()
            .collect();
        assert!(
            endgame_active(&full, &pmap, &done),
            "全部块已被认领 → endgame"
        );
        let done_all: HashSet<u32> = [0u32, 1].into_iter().collect();
        assert!(
            !endgame_active(&full, &pmap, &done_all),
            "全部完成后不进 endgame"
        );
    }

    fn interested_conn(addr: &str, cum_down: u64) -> ConnStat {
        let mut st = new_conn_stat(addr.to_string());
        st.peer_interested = true;
        st.cum_down = cum_down;
        st.tit_prev = (0, std::time::Instant::now() - Duration::from_secs(10));
        st
    }

    #[test]
    fn conn_views_hides_pre_handshake_connections() {
        // 回归：对方交叉拨入（我们出连的同时对方拨入）在握手完成前以同 IP
        // 挂在 conns 里；渲染成行就是「同一 peer 两行」。并发 panel 只展示
        // 已建立（握手完成、peer_id 已知）的连接。
        let mut conns: HashMap<usize, ConnStat> = HashMap::new();
        let mut established = new_conn_stat("10.0.0.7:51413".into());
        established.peer_id = Some([7u8; 20]);
        conns.insert(1, established);
        let mut dialing = new_conn_stat("10.0.0.7:53111".into());
        dialing.peer_id = None;
        conns.insert(2, dialing);
        let files = vec![BtFile {
            path: "a.bin".into(),
            size: 64 * 1024,
            selected: true,
        }];
        let pmap = PieceMap::new(&files, 32 * 1024);
        let views = conn_views(&mut conns, pmap.covered_pieces());
        assert_eq!(
            views.len(),
            1,
            "握手未完成的连接不得出现在并发 panel（同 IP 两行根因）"
        );
        assert_eq!(views[0].addr, "10.0.0.7:51413", "保留的是已建立连接");
    }

    #[tokio::test]
    async fn choker_unchoke_top_rate_interested_and_one_optimistic() {
        let mut conns: HashMap<usize, ConnStat> = HashMap::new();
        for (i, down) in [10_000u64, 8_000, 6_000, 4_000, 2_000, 0]
            .iter()
            .enumerate()
        {
            conns.insert(
                i + 1,
                interested_conn(&format!("10.0.0.{}:1", i + 1), *down),
            );
        }
        let mut cursor = 0usize;
        let slot = apply_choker(&mut conns, false, false, None, &mut cursor).await;
        let unchoked: Vec<usize> = conns
            .iter()
            .filter(|(_, st)| !st.am_choking)
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(unchoked.len(), UPLOAD_SLOTS + 1, "4 固定槽 + 1 乐观槽");
        for id in 1..=4usize {
            assert!(
                !conns[&id].am_choking,
                "下载最快的 interested peer 获得上传槽"
            );
        }
        assert_eq!(slot, Some(5), "乐观槽在剩余 interested 池中轮转");
        assert!(!conns[&5].am_choking);
        assert!(conns[&6].am_choking, "零速率 peer 仅在轮到乐观槽时放行");
    }

    #[tokio::test]
    async fn choker_chokes_previously_unchoked_not_interested_peer() {
        let mut conns: HashMap<usize, ConnStat> = HashMap::new();
        let mut st = new_conn_stat("10.0.0.1:1".to_string());
        st.am_choking = false;
        conns.insert(1, st);
        let mut cursor = 0usize;
        apply_choker(&mut conns, false, false, None, &mut cursor).await;
        assert!(
            conns[&1].am_choking,
            "不 interested 的 peer 被回收上传槽（发 Choke）"
        );
    }

    #[tokio::test]
    async fn choker_seeding_unchoke_everyone() {
        let mut conns: HashMap<usize, ConnStat> = HashMap::new();
        for i in 1..=3usize {
            let mut st = new_conn_stat(format!("10.0.0.{i}:1"));
            st.am_choking = true;
            conns.insert(i, st);
        }
        let mut cursor = 0usize;
        let slot = apply_choker(&mut conns, true, false, None, &mut cursor).await;
        assert_eq!(slot, None, "做种态无乐观槽");
        assert!(
            conns.values().all(|st| !st.am_choking),
            "做种全量 unchoke（D58）"
        );
    }

    #[tokio::test]
    async fn request_handler_serves_only_when_not_choking() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..16_384usize).map(|i| (i % 211) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-serve-gate");
        let save_dir = dir.to_string_lossy().into_owned();
        let files = vec![BtFile {
            path: "g.bin".into(),
            size: data.len() as u64,
            selected: true,
        }];
        std::fs::write(format!("{save_dir}/g.bin"), &data).unwrap();
        let mut handles: HashMap<usize, tokio::fs::File> = HashMap::new();
        let fh = tokio::fs::OpenOptions::new()
            .read(true)
            .open(format!("{save_dir}/g.bin"))
            .await
            .unwrap();
        handles.insert(0, fh);

        let meta = {
            let mut pieces = Vec::new();
            let mut h = Sha1::new();
            h.update(&data);
            pieces.extend_from_slice(&h.finalize());
            use crate::bt::bencode::{encode, BValue};
            let mut info = BTreeMap::new();
            info.insert(b"length".to_vec(), BValue::Int(data.len() as i64));
            info.insert(b"name".to_vec(), BValue::Bytes(b"g.bin".to_vec()));
            info.insert(b"piece length".to_vec(), BValue::Int(piece_len as i64));
            info.insert(b"pieces".to_vec(), BValue::Bytes(pieces));
            let mut root = BTreeMap::new();
            root.insert(
                b"announce".to_vec(),
                BValue::Bytes(b"udp://127.0.0.1:1/announce".to_vec()),
            );
            root.insert(b"info".to_vec(), BValue::Dict(info));
            TorrentMeta::parse(&encode(&BValue::Dict(root))).unwrap()
        };
        let spec = crate::engine::BtSpec {
            id: 97,
            source: "/tmp/g.torrent".into(),
            meta,
            save_dir: save_dir.clone(),
            name: "g.bin".into(),
            files,
            trackers: vec![],
            concurrency: 4,
            proxy_endpoint: None,
            seeding: false,
            max_speed: 0,
        };
        let pmap = PieceMap::new(&spec.files, spec.meta.piece_length);
        let mut done: HashSet<u32> = HashSet::new();
        done.insert(0);
        let mut downloaded = data.len() as u64;
        let mut partial: PartialPieces = HashMap::new();

        let mut choking = new_conn_stat("10.0.0.9:999".into());
        let (tx_a, mut rx_a) = mpsc::channel(8);
        choking.peer_out = tx_a;
        let mut conns = HashMap::from([(1usize, choking)]);
        conns.get_mut(&1).unwrap().cum_up = 0;
        handle_peer_msg(
            1,
            peer::PeerMsg::Request {
                index: 0,
                begin: 0,
                length: 4096,
            },
            &mut conns,
            &pmap,
            &spec,
            &mut done,
            &mut downloaded,
            &mut handles,
            &mut partial,
        )
        .await;
        assert!(
            rx_a.try_recv().is_err(),
            "我方 choke 状态下 request 不被服务（反吸血闸门）"
        );

        if let Some(st) = conns.get_mut(&1) {
            st.am_choking = false;
        }
        handle_peer_msg(
            1,
            peer::PeerMsg::Request {
                index: 0,
                begin: 0,
                length: 4096,
            },
            &mut conns,
            &pmap,
            &spec,
            &mut done,
            &mut downloaded,
            &mut handles,
            &mut partial,
        )
        .await;
        match rx_a.try_recv() {
            Ok(PeerOut::Send(peer::PeerMsg::Block {
                index,
                begin,
                data: blk,
            })) => {
                assert_eq!((index, begin), (0, 0));
                assert_eq!(blk, &data[..4096]);
            }
            other => panic!("unchoke 后 request 应被服务：{other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    async fn flaky_then_good_seeder(
        data: Vec<u8>,
        piece_len: usize,
        infohash: [u8; 20],
        counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) -> u16 {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let data = std::sync::Arc::new(data);
        tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = listener.accept().await else {
                    return;
                };
                let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let data = data.clone();
                tokio::spawn(async move {
                    if n == 0 {
                        drop(sock);
                        return;
                    }
                    serve_seeder_session(sock, &data, piece_len, infohash).await;
                });
            }
        });
        port
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn engine_reconnects_peer_after_drop() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..40_960usize).map(|i| (i % 227) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-reconnect");
        let save_dir = dir.to_string_lossy().into_owned();

        let counter = Arc::new(AtomicUsize::new(0));
        let meta0 = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let seeder_port =
            flaky_then_good_seeder(data.clone(), piece_len, meta0.infohash, counter.clone()).await;
        let tracker_port = fake_udp_tracker_fast(seeder_port).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );
        let spec = crate::engine::BtSpec {
            id: 98,
            source: "/tmp/e2e-re.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "e2e.bin".into(),
            files: vec![BtFile {
                path: "e2e.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 4,
            proxy_endpoint: None,
            seeding: false,
            max_speed: 0,
        };
        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let shared = crate::engine::shared_for_tests();
        let handle = tokio::spawn(run(spec, shared, cmd_rx, evt_tx));

        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let mut done_evt = false;
        while std::time::Instant::now() < deadline {
            let remain = deadline.saturating_duration_since(std::time::Instant::now());
            let Some(evt) = tokio::time::timeout(remain, evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                break;
            };
            if matches!(evt, Evt::BtDone { .. }) {
                done_evt = true;
                break;
            }
        }
        assert!(done_evt, "peer 掉线后引擎从候选池重连并完成下载");
        assert_eq!(
            counter.load(Ordering::SeqCst),
            2,
            "seeder 恰被连两次（首次被断开 + 重连成功）"
        );
        let out = std::fs::read(format!("{save_dir}/e2e.bin")).unwrap();
        assert_eq!(out, data);
        handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn engine_connects_all_tracker_peers() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..20_480usize).map(|i| (i % 229) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-fanout");
        let save_dir = dir.to_string_lossy().into_owned();

        let meta0 = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let c1 = Arc::new(AtomicUsize::new(0));
        let c2 = Arc::new(AtomicUsize::new(0));
        let c3 = Arc::new(AtomicUsize::new(0));
        let p1 = counting_seeder(data.clone(), piece_len, meta0.infohash, c1.clone()).await;
        let p2 = counting_seeder(data.clone(), piece_len, meta0.infohash, c2.clone()).await;
        let p3 = counting_seeder(data.clone(), piece_len, meta0.infohash, c3.clone()).await;
        let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let tracker_port = sock.local_addr().unwrap().port();
        tokio::spawn(async move {
            let mut buf = [0u8; 512];
            loop {
                let Ok((n, from)) = sock.recv_from(&mut buf).await else {
                    return;
                };
                let req = &buf[..n];
                if req.len() < 16 {
                    continue;
                }
                let action = u32::from_be_bytes(req[8..12].try_into().unwrap());
                let txid = u32::from_be_bytes(req[12..16].try_into().unwrap());
                if action == 0 {
                    if u64::from_be_bytes(req[0..8].try_into().unwrap()) != 0x0417_2710_1980 {
                        continue;
                    }
                    let mut out = Vec::new();
                    out.extend_from_slice(&0u32.to_be_bytes());
                    out.extend_from_slice(&txid.to_be_bytes());
                    out.extend_from_slice(&0x1234_5678_9abc_def0u64.to_be_bytes());
                    let _ = sock.send_to(&out, from).await;
                } else if action == 1 {
                    if u64::from_be_bytes(req[0..8].try_into().unwrap()) != 0x1234_5678_9abc_def0 {
                        continue;
                    }
                    let mut out = Vec::new();
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&txid.to_be_bytes());
                    out.extend_from_slice(&60u32.to_be_bytes());
                    out.extend_from_slice(&0u32.to_be_bytes());
                    out.extend_from_slice(&3u32.to_be_bytes());
                    for p in [p1, p2, p3] {
                        out.extend_from_slice(&[127, 0, 0, 1]);
                        out.extend_from_slice(&p.to_be_bytes());
                    }
                    let _ = sock.send_to(&out, from).await;
                }
            }
        });

        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );
        let spec = crate::engine::BtSpec {
            id: 99,
            source: "/tmp/e2e-fan.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "e2e.bin".into(),
            files: vec![BtFile {
                path: "e2e.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 8,
            proxy_endpoint: None,
            seeding: false,
            max_speed: 0,
        };
        let (evt_tx, _evt_rx) = mpsc::channel(256);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let shared = crate::engine::shared_for_tests();
        let handle = tokio::spawn(run(spec, shared, cmd_rx, evt_tx));

        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while std::time::Instant::now() < deadline {
            if c1.load(Ordering::SeqCst) >= 1
                && c2.load(Ordering::SeqCst) >= 1
                && c3.load(Ordering::SeqCst) >= 1
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(
            c1.load(Ordering::SeqCst) >= 1
                && c2.load(Ordering::SeqCst) >= 1
                && c3.load(Ordering::SeqCst) >= 1,
            "tracker 返回的多个 peer 全部被并发连接（concurrency=8 ≥ 3）"
        );
        handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    async fn fake_udp_tracker_events(
        seeder_port: u16,
        events: std::sync::Arc<std::sync::Mutex<Vec<u32>>>,
    ) -> u16 {
        let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let port = sock.local_addr().unwrap().port();
        tokio::spawn(async move {
            let mut buf = [0u8; 512];
            loop {
                let Ok((n, from)) = sock.recv_from(&mut buf).await else {
                    return;
                };
                let req = &buf[..n];
                if req.len() < 16 {
                    continue;
                }
                let action = u32::from_be_bytes(req[8..12].try_into().unwrap());
                let txid = u32::from_be_bytes(req[12..16].try_into().unwrap());
                if action == 0 {
                    if u64::from_be_bytes(req[0..8].try_into().unwrap()) != 0x0417_2710_1980 {
                        continue;
                    }
                    let mut out = Vec::new();
                    out.extend_from_slice(&0u32.to_be_bytes());
                    out.extend_from_slice(&txid.to_be_bytes());
                    out.extend_from_slice(&0x1234_5678_9abc_def0u64.to_be_bytes());
                    let _ = sock.send_to(&out, from).await;
                } else if action == 1 {
                    if u64::from_be_bytes(req[0..8].try_into().unwrap()) != 0x1234_5678_9abc_def0 {
                        continue;
                    }
                    let ev = u32::from_be_bytes(req[80..84].try_into().unwrap());
                    events.lock().unwrap().push(ev);
                    let mut out = Vec::new();
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&txid.to_be_bytes());
                    out.extend_from_slice(&60u32.to_be_bytes());
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&[127, 0, 0, 1]);
                    out.extend_from_slice(&seeder_port.to_be_bytes());
                    let _ = sock.send_to(&out, from).await;
                }
            }
        });
        port
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tracker_completed_sent_immediately_and_stopped_with_stats() {
        use std::sync::atomic::AtomicUsize;
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..40_960usize).map(|i| (i % 233) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-completed");
        let save_dir = dir.to_string_lossy().into_owned();

        let counter = Arc::new(AtomicUsize::new(0));
        let meta0 = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let seeder_port =
            counting_seeder(data.clone(), piece_len, meta0.infohash, counter.clone()).await;
        let events: std::sync::Arc<std::sync::Mutex<Vec<u32>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let tracker_port = fake_udp_tracker_events(seeder_port, events.clone()).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );
        let spec = crate::engine::BtSpec {
            id: 100,
            source: "/tmp/e2e-cp.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "e2e.bin".into(),
            files: vec![BtFile {
                path: "e2e.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 4,
            proxy_endpoint: None,
            seeding: false,
            max_speed: 0,
        };
        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let shared = crate::engine::shared_for_tests();
        let handle = tokio::spawn(run(spec, shared, cmd_rx, evt_tx));

        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            let remain = deadline.saturating_duration_since(std::time::Instant::now());
            let Some(evt) = tokio::time::timeout(remain, evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                panic!("下载未在时限内完成");
            };
            if matches!(evt, Evt::BtDone { .. }) {
                break;
            }
        }
        let done_at = std::time::Instant::now();
        let mut saw_completed = false;
        while done_at.elapsed() < Duration::from_secs(5) {
            if events.lock().unwrap().contains(&1) {
                saw_completed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(
            saw_completed,
            "完成事件立即通知 tracker（interval=60s 内必达）: {:?}",
            events.lock().unwrap()
        );
        cmd_tx.send(BtTaskCmd::Cancel).await.unwrap();
        let mut saw_stopped = false;
        let stop_deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < stop_deadline {
            if events.lock().unwrap().contains(&3) {
                saw_stopped = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(saw_stopped, "取消（删除任务）也发送 stopped");
        let _ = handle.await;
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod seeding_tests {
    use std::collections::BTreeMap;

    use tokio::io::AsyncReadExt;

    use super::*;
    use crate::bt::bencode::{encode, BValue};
    use crate::model::BtFile;

    fn make_meta(data: &[u8], piece_len: usize, tracker: &str) -> TorrentMeta {
        let mut pieces = Vec::new();
        for chunk in data.chunks(piece_len) {
            let mut h = Sha1::new();
            h.update(chunk);
            pieces.extend_from_slice(&h.finalize());
        }
        let mut info = BTreeMap::new();
        info.insert(b"length".to_vec(), BValue::Int(data.len() as i64));
        info.insert(b"name".to_vec(), BValue::Bytes(b"seed.bin".to_vec()));
        info.insert(b"piece length".to_vec(), BValue::Int(piece_len as i64));
        info.insert(b"pieces".to_vec(), BValue::Bytes(pieces));
        let mut root = BTreeMap::new();
        root.insert(
            b"announce".to_vec(),
            BValue::Bytes(tracker.as_bytes().to_vec()),
        );
        root.insert(b"info".to_vec(), BValue::Dict(info));
        TorrentMeta::parse(&encode(&BValue::Dict(root))).unwrap()
    }

    async fn fake_udp_tracker(captured: std::sync::Arc<std::sync::Mutex<Option<u16>>>) -> u16 {
        let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let port = sock.local_addr().unwrap().port();
        tokio::spawn(async move {
            let mut buf = [0u8; 512];
            loop {
                let (n, from) = match sock.recv_from(&mut buf).await {
                    Ok(x) => x,
                    Err(_) => return,
                };
                let req = &buf[..n];
                if req.len() < 16 {
                    continue;
                }
                let action = u32::from_be_bytes(req[8..12].try_into().unwrap());
                let txid = u32::from_be_bytes(req[12..16].try_into().unwrap());
                if action == 0 {
                    if u64::from_be_bytes(req[0..8].try_into().unwrap()) != 0x0417_2710_1980 {
                        continue;
                    }
                    let mut out = Vec::new();
                    out.extend_from_slice(&0u32.to_be_bytes());
                    out.extend_from_slice(&txid.to_be_bytes());
                    out.extend_from_slice(&0x1234_5678_9abc_def0u64.to_be_bytes());
                    let _ = sock.send_to(&out, from).await;
                } else if action == 1 {
                    let announced_port = u16::from_be_bytes(req[96..98].try_into().unwrap());
                    *captured.lock().unwrap() = Some(announced_port);
                    let mut out = Vec::new();
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&txid.to_be_bytes());
                    out.extend_from_slice(&60u32.to_be_bytes());
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&1u32.to_be_bytes());
                    let _ = sock.send_to(&out, from).await;
                }
            }
        });
        port
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn engine_seeds_to_leecher_with_metadata_exchange() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..20_480usize).map(|i| (i % 199) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-seed");
        let save_dir = dir.to_string_lossy().into_owned();

        let meta = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        std::fs::write(format!("{save_dir}/seed.bin"), &data).unwrap();

        let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
        let tracker_port = fake_udp_tracker(captured.clone()).await;

        let spec = crate::engine::BtSpec {
            id: 88,
            source: "/tmp/seed.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "seed.bin".into(),
            files: vec![BtFile {
                path: "seed.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 4,
            proxy_endpoint: None,
            seeding: true,
            max_speed: 0,
        };
        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let shared = crate::engine::shared_for_tests();
        let engine_handle = tokio::spawn(run(spec, shared, cmd_rx, evt_tx));

        let listen_port = {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(p) = *captured.lock().unwrap() {
                    break p;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "超时未捕获 announce port"
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        };

        let mut sock = TcpStream::connect(format!("127.0.0.1:{listen_port}"))
            .await
            .unwrap();
        let mut hs = Vec::new();
        hs.push(19);
        hs.extend_from_slice(b"BitTorrent protocol");
        hs.extend_from_slice(&peer::reserved_bits(true));
        hs.extend_from_slice(&meta.infohash);
        hs.extend_from_slice(b"-QX7777-leecherpeer0");
        sock.write_all(&hs).await.unwrap();
        let mut their_hs = [0u8; 68];
        sock.read_exact(&mut their_hs).await.unwrap();
        assert_eq!(&their_hs[28..48], &meta.infohash, "引擎握手 infohash 回显");

        let mut head = [0u8; 4];
        sock.read_exact(&mut head).await.unwrap();
        let flen = u32::from_be_bytes(head) as usize;
        let mut fbody = vec![0u8; flen];
        sock.read_exact(&mut fbody).await.unwrap();
        assert_eq!(fbody[0], 5, "引擎首帧 = bitfield");
        let mut head2 = [0u8; 4];
        sock.read_exact(&mut head2).await.unwrap();
        assert_eq!(u32::from_be_bytes(head2), 1);
        let mut id = [0u8; 1];
        sock.read_exact(&mut id).await.unwrap();
        assert_eq!(id[0], 1, "做种全量 unchoke（D57）");

        let mut eh_head = [0u8; 4];
        sock.read_exact(&mut eh_head).await.unwrap();
        let eh_len = u32::from_be_bytes(eh_head) as usize;
        let mut eh = vec![0u8; eh_len];
        sock.read_exact(&mut eh).await.unwrap();
        assert_eq!(eh[0], 20, "extended 帧");
        assert_eq!(eh[1], 0, "ext handshake id");
        let (peer_ut_id, metadata_size) =
            peer::parse_ext_handshake(&eh[2..]).expect("ext handshake 可解析");
        assert_eq!(metadata_size as usize, meta.info_bytes.len());

        let req = peer::encode_msg(&peer::PeerMsg::Request {
            index: 0,
            begin: 0,
            length: 4096,
        });
        sock.write_all(&req).await.unwrap();
        let mut blk_head = [0u8; 4];
        sock.read_exact(&mut blk_head).await.unwrap();
        let blen = u32::from_be_bytes(blk_head) as usize;
        let mut blk = vec![0u8; blen];
        sock.read_exact(&mut blk).await.unwrap();
        assert_eq!(blk[0], 7, "piece 消息");
        let idx = u32::from_be_bytes(blk[1..5].try_into().unwrap());
        let beg = u32::from_be_bytes(blk[5..9].try_into().unwrap());
        assert_eq!((idx, beg), (0, 0));
        let payload = &blk[9..];
        assert_eq!(payload, &data[..4096], "leecher 拉到数据字节（FR-02-23②）");

        let meta_req = peer::encode_msg(&peer::PeerMsg::Extended {
            ext_id: peer_ut_id,
            payload: peer::encode_ut_metadata(&peer::UtMetadata::Request(0)),
        });
        sock.write_all(&meta_req).await.unwrap();
        let mut md_head = [0u8; 4];
        sock.read_exact(&mut md_head).await.unwrap();
        let md_len = u32::from_be_bytes(md_head) as usize;
        let mut md = vec![0u8; md_len];
        sock.read_exact(&mut md).await.unwrap();
        assert_eq!(md[0], 20);
        assert_eq!(md[1], peer_ut_id);
        let um = peer::parse_ut_metadata(&md[2..]).expect("ut_metadata data");
        match um {
            peer::UtMetadata::Data {
                piece,
                total_size,
                data: shard,
            } => {
                assert_eq!(piece, 0);
                assert_eq!(total_size as usize, meta.info_bytes.len());
                assert_eq!(shard, meta.info_bytes, "元数据分片 = info 精确字节");
                let mut h = Sha1::new();
                h.update(&shard);
                assert_eq!(h.finalize().as_slice(), &meta.infohash, "SHA-1 == infohash");
            }
            other => panic!("应为 data 响应：{other:?}"),
        }

        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        let mut uploaded_seen = 0u64;
        while std::time::Instant::now() < deadline {
            let Some(evt) = tokio::time::timeout(Duration::from_millis(1500), evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                continue;
            };
            if let Evt::BtProgress { uploaded, .. } = evt {
                uploaded_seen = uploaded.max(uploaded_seen);
                if uploaded_seen >= 4096 {
                    break;
                }
            }
        }
        assert!(uploaded_seen >= 4096, "上传统计真实增长：{uploaded_seen}");
        drop(sock);
        engine_handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
