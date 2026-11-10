use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use super::pieces::PieceMap;
use super::tracker::TrackerUrl;
use super::webrtc::{PeerTransport, RtcReady, SignalState};
use super::{peer, BtError, BtTaskCmd};
use crate::engine::{BtTunables, EngineShared, Evt, Throttle};

mod conn;
mod peerwire;
mod pieceio;
mod trackpool;

use conn::*;
use peerwire::*;
use pieceio::*;
use trackpool::*;

const PEER_SWEEP_SECS: u64 = 2;

const LEECH_UP_MIN: u64 = 1024 * 1024;

const LEECH_DOWN_MAX: u64 = 64 * 1024;

const LEECH_MIN_ALIVE_SECS: u64 = 120;

pub(crate) async fn run(
    mut spec: crate::engine::BtSpec,
    shared: Arc<EngineShared>,
    mut cmd_rx: mpsc::Receiver<BtTaskCmd>,
    evt_tx: mpsc::Sender<Evt>,
) {
    let id = spec.id;
    let tun: BtTunables = spec.tunables.clone();
    let peer_id = super::peer_id::session_peer_id();
    let mut pmap = Arc::new(PieceMap::new(&spec.files, spec.meta.piece_length));

    let mut files_handles: HashMap<usize, tokio::fs::File> = HashMap::new();
    for (i, f) in pmap.files.iter().enumerate() {
        if !f.selected {
            continue;
        }
        match open_selected_file_handle(&spec.save_dir, f).await {
            Ok(fh) => {
                files_handles.insert(i, fh);
            }
            Err(e) => {
                report_fail(&evt_tx, id, BtError::Io(e)).await;
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
                            save_bt_sidecar_full(&sidecar_path, &final_done, &partial);
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
        let sc_loaded = crate::model::BtSidecar::load_full(&sidecar_path);
        if let Some(sc) = &sc_loaded {
            restore_sidecar_partial(
                &sc.partial_pieces,
                &pmap,
                &spec.meta,
                &mut files_handles,
                &mut done,
                &mut partial,
                &mut downloaded,
            )
            .await;
        }
        save_bt_sidecar_full(&sidecar_path, &done, &partial);
    }
    let mut sidecar_saved_done: usize = done.len();

    let listener = shared.bt_listener();
    let listen_port = listener.local_addr().map(|a| a.port()).unwrap_or(6881);
    let task_dl: Arc<Throttle> = Arc::new(Throttle::with_params(
        spec.max_download_speed,
        shared.throttle_burst_secs(),
        shared.throttle_sleep_ms(),
    ));
    let task_ul: Arc<Throttle> = Arc::new(Throttle::with_params(
        spec.max_upload_speed,
        shared.throttle_burst_secs(),
        shared.throttle_sleep_ms(),
    ));
    let dl_flow = Arc::new(DlFlow {
        global: shared.dl_throttle().clone(),
        task: task_dl.clone(),
    });

    let mut trackers: Vec<TrackerUrl> = {
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
    let signals: Arc<std::sync::Mutex<SignalState>> =
        Arc::new(std::sync::Mutex::new(SignalState::default()));
    let (rtc_tx, mut rtc_rx) = mpsc::channel::<RtcReady>(16);
    let reannounce_notify = Arc::new(tokio::sync::Notify::new());
    let (mut completed_notifies, mut tr_handles) = start_tracker_pool(
        &trackers,
        spec.meta.infohash,
        peer_id,
        listen_port,
        shared.bt_tracker_client(spec.proxy_endpoint.as_ref()),
        &tun,
        &live,
        &tr_tx,
        &signals,
        &rtc_tx,
        &reannounce_notify,
    );

    let (peer_tx, mut peer_rx) = mpsc::channel::<(usize, PeerIn)>(256);
    let mut next_conn: usize = 1;
    let mut conns: HashMap<usize, ConnStat> = HashMap::new();
    let mut candidates: HashMap<(IpAddr, u16), PeerCandidate> = HashMap::new();
    let mut seeding = spec.seeding;
    let mut upload_paused = spec.upload_paused;

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
    let mut order_cache = PieceOrderCache::new();
    let mut last_emit = std::time::Instant::now();

    loop {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    BtTaskCmd::Pause | BtTaskCmd::StopSeed => {
                        if !spec.seeding {
                            save_bt_sidecar_full(&sidecar_path, &done, &partial);
                        }
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
                            for &p in st.has.iter().filter(|&p| pmap.covers(*p)).collect::<Vec<_>>() {
                                order_cache.peer_lost(p);
                            }
                            let _ = st.peer_out.send(PeerOut::Drop).await;
                        }
                    }
                    BtTaskCmd::UpdateTrackers(urls) => {
                        for h in tr_handles.drain(..) {
                            h.abort();
                        }
                        let mut seen = HashSet::new();
                        trackers = urls
                            .iter()
                            .filter_map(|s| TrackerUrl::parse(s))
                            .filter(|u| seen.insert(u.clone()))
                            .collect();
                        tracker_fail_count = 0;
                        got_peers = got_peers || !trackers.is_empty();
                        let (notifies, handles) = start_tracker_pool(
                            &trackers,
                            spec.meta.infohash,
                            peer_id,
                            listen_port,
                            shared.bt_tracker_client(spec.proxy_endpoint.as_ref()),
                            &tun,
                            &live,
                            &tr_tx,
                            &signals,
                            &rtc_tx,
                            &reannounce_notify,
                        );
                        completed_notifies = notifies;
                        tr_handles = handles;
                    }
                    BtTaskCmd::Reconfigure {
                        concurrency,
                        files,
                        proxy_endpoint,
                        max_download_speed,
                        max_upload_speed,
                    } => {
                        task_dl.set_rate(max_download_speed);
                        task_ul.set_rate(max_upload_speed);
                        let target = concurrency.clamp(1, 64);
                        let files_changed = spec.files != files;
                        let proxy_changed = spec.proxy_endpoint != proxy_endpoint;
                        spec.concurrency = target;
                        spec.proxy_endpoint = proxy_endpoint;
                        if files_changed {
                            let old_files = std::mem::take(&mut spec.files);
                            for (i, f) in files.iter().enumerate() {
                                if !f.selected {
                                    files_handles.remove(&i);
                                }
                            }
                            crate::model::bt::delete_deselected_files(
                                &spec.save_dir,
                                &old_files,
                                &files,
                            );
                            spec.files = files;
                            pmap = Arc::new(PieceMap::new(&spec.files, spec.meta.piece_length));
                            let mut avail: HashMap<u32, usize> = HashMap::new();
                            for st in conns.values() {
                                for &p in st.has.iter().filter(|&p| pmap.covers(*p)) {
                                    *avail.entry(p).or_insert(0) += 1;
                                }
                            }
                            order_cache.avail = avail;
                            order_cache.order = Vec::new();
                            order_cache.dirty = true;
                            let pl = u64::from(pmap.piece_length).max(1);
                            let mut off = 0u64;
                            let mut invalid: HashSet<u32> = HashSet::new();
                            for f in spec.files.iter() {
                                let (s, e) = (off, off + f.size);
                                off = e;
                                if !f.selected || f.size == 0 {
                                    continue;
                                }
                                let abs = file_abs_path(&spec.save_dir, &f.path);
                                if !std::path::Path::new(&abs).exists() {
                                    let p0 = (s / pl) as u32;
                                    let p1 = ((e - 1) / pl) as u32;
                                    for p in p0..=p1 {
                                        invalid.insert(p);
                                    }
                                }
                            }
                            for p in &invalid {
                                done.remove(p);
                                partial.remove(p);
                            }
                            for (i, f) in pmap.files.iter().enumerate() {
                                if !f.selected || files_handles.contains_key(&i) {
                                    continue;
                                }
                                if let Ok(fh) =
                                    open_selected_file_handle(&spec.save_dir, f).await
                                {
                                    files_handles.insert(i, fh);
                                }
                            }
                            for st in conns.values_mut() {
                                let mut dropped = Vec::new();
                                cancel_inflight_not_covered(st, &pmap, &mut dropped);
                            }
                            downloaded = recompute_downloaded(&done, &pmap);
                            {
                                let mut st = live.lock().expect("live stats 锁");
                                st.downloaded = downloaded;
                                st.left = pmap.selected_total().saturating_sub(downloaded);
                            }
                            if seeding && !all_selected_done(&done, &pmap) {
                                seeding = false;
                                upload_paused = true;
                                live.lock().expect("live stats 锁").completed = false;
                                for st in conns.values_mut() {
                                    st.am_choking = true;
                                    let _ = st
                                        .peer_out
                                        .send(PeerOut::Send(peer::PeerMsg::Choke))
                                        .await;
                                }
                            }
                            if !seeding {
                                let claimed = claimed_blocks(&conns, &partial);
                                let cids: Vec<usize> = conns.keys().copied().collect();
                                for cid in cids {
                                    if let Some(st) = conns.get_mut(&cid) {
                                        issue_requests(
                                            cid, st, &claimed, &pmap, &tun, &done, &partial,
                                            &mut order_cache,
                                        )
                                        .await;
                                    }
                                }
                            }
                        }
                        for cid in shed_excess_conns(&conns, target) {
                            if let Some(st) = conns.remove(&cid) {
                                if let Some(key) = addr_sock(&st.addr) {
                                    if let Some(c) = candidates.get_mut(&key) {
                                        c.last_try = Some(std::time::Instant::now());
                                    }
                                }
                                let _ = st.peer_out.send(PeerOut::Drop).await;
                            }
                        }
                        if !seeding && conns.len() < spec.concurrency.max(1) {
                            fill_peer_slots(
                                &spec, &peer_id, &peer_tx, shared.as_ref(), &mut conns,
                                &mut candidates, &mut next_conn, &dl_flow,
                            );
                            // 扩容/仍有缺口：全 tracker 提前 re-announce（ws 会话立即携新
                            // offers 重连 shed 的 WebRTC peer；http/udp 刷新候选池），
                            // 不再等 tracker interval（操作者 bug28③ 调优）。
                            reannounce_notify.notify_waiters();
                        }
                        if proxy_changed {
                            for h in tr_handles.drain(..) {
                                h.abort();
                            }
                            tracker_fail_count = 0;
                            let (notifies, handles) = start_tracker_pool(
                                &trackers,
                                spec.meta.infohash,
                                peer_id,
                                listen_port,
                                shared.bt_tracker_client(spec.proxy_endpoint.as_ref()),
                                &tun,
                                &live,
                                &tr_tx,
                                &signals,
                                &rtc_tx,
                                &reannounce_notify,
                            );
                            completed_notifies = notifies;
                            tr_handles = handles;
                        }
                        let views = conn_views(&mut conns, pmap.covered_pieces(), &done, optimistic_slot);
                        let _ = evt_tx.send(Evt::BtConns { id, conns: views }).await;
                    }
                }
            }
            Some(rep) = tr_rx.recv() => {
                match rep {
                    TrackerReport::Peers { url, peers, complete, incomplete, interval_secs } => {
                        tracker_fail_count = 0;
                        got_peers = true;
                        let _ = evt_tx
                            .send(Evt::BtPeers { id, complete, incomplete })
                            .await;
                        let _ = evt_tx
                            .send(Evt::BtTrackers {
                                id,
                                url,
                                working: true,
                                peers: Some(incomplete),
                                seeds: Some(complete),
                                next_in_secs: interval_secs,
                            })
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
                            &mut candidates, &mut next_conn, &dl_flow,
                        );
                    }
                    TrackerReport::Failed { url, retry_secs } => {
                        let _ = evt_tx
                            .send(Evt::BtTrackers {
                                id,
                                url,
                                working: false,
                                peers: None,
                                seeds: None,
                                next_in_secs: retry_secs,
                            })
                            .await;
                        tracker_fail_count += 1;
                        if tracker_fail_count >= trackers.len() && !got_peers && !trackers.is_empty() {
                            report_fail(&evt_tx, id, BtError::Tracker("全部 tracker 失败".into())).await;
                            return;
                        }
                    }
                }
            }
            Ok((stream, addr)) = listener.accept() => {
                let sock_key = (addr.ip(), addr.port());
                if spec.tunables.transports.allows(PeerTransport::Tcp)
                    && inbound_admitted(&conns, sock_key, spec.concurrency.max(1) + 8)
                    && !shared.is_blacklisted(addr.ip())
                {
                    spawn_incoming_peer(
                        next_conn,
                        stream,
                        addr,
                        &spec,
                        &peer_id,
                        peer_tx.clone(),
                        dl_flow.clone(),
                    );
                    let mut inc = new_conn_stat(addr.to_string());
                    inc.inbound = true;
                    conns.insert(next_conn, inc);
                    next_conn += 1;
                } else {
                    drop(stream);
                }
            }
            Some(ready) = rtc_rx.recv() => {
                if !spec.tunables.transports.allows(PeerTransport::WebRtc)
                    || conns.len() >= spec.concurrency.max(1) + 8
                {
                    continue;
                }
                let cid = next_conn;
                next_conn += 1;
                // D71⑤ 改判（操作者 20261109）：协议列已显 WebRTC，地址列只保留
                // 对端 peer_id 前 5 字节 hex 裸值，不再加 "WebRTC:" 前缀。
                let addr = super::tracker::hex_lower(&ready.remote_peer_id[..5]);
                let mut st = new_conn_stat_with(addr, PeerTransport::WebRtc);
                st.inbound = !ready.initiator;
                conns.insert(cid, st);
                signals
                    .lock()
                    .expect("signals 锁")
                    .mark_peer_alive(&ready.remote_peer_id);
                let hs_timeout = spec.tunables.handshake_timeout_secs;
                let ih: [u8; 20] = spec.meta.infohash;
                let pid: [u8; 20] = peer_id;
                let initiator = ready.initiator;
                let ptx = peer_tx.clone();
                let flow = dl_flow.clone();
                tokio::spawn(async move {
                    run_peer_loop(
                        cid,
                        ready.stream,
                        &ih,
                        &pid,
                        hs_timeout,
                        initiator,
                        ptx,
                        flow,
                    )
                    .await;
                });
            }
            Some((conn_id, ev)) = peer_rx.recv() => {
                let prev_downloaded = downloaded;
                match ev {
                    PeerIn::Connected {
                        client,
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
                            st.client = client;
                            st.peer_id = Some(peer_id);
                            st.peer_out = out;
                            if st.protocol == PeerTransport::WebRtc {
                                signals
                                    .lock()
                                    .expect("signals 锁")
                                    .mark_peer_alive(&peer_id);
                            }
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
                        let views = conn_views(&mut conns, pmap.covered_pieces(), &done, optimistic_slot);
                        let _ = evt_tx.send(Evt::BtConns { id, conns: views }).await;
                    }
                    PeerIn::Closed { reason } => {
                        if let Some(st) = conns.remove(&conn_id) {
                            for &p in st.has.iter().filter(|&p| pmap.covers(*p)).collect::<Vec<_>>() {
                                order_cache.peer_lost(p);
                            }
                            if st.protocol == PeerTransport::WebRtc {
                                if let Some(pid) = st.peer_id {
                                    // 本端主动收口（缩容 shed / 空闲置换 Drop / 自身拒绝）：
                                    // 不记失败冷却（mark_peer_idle），扩容后下一轮 offer 即可
                                    // 重连；异常断开仍走 mark_peer_closed 惩罚性退避（操作者
                                    // bug28③ 调优：shed 的健康 peer 不得被计入失败）。
                                    let local = reason == "引擎断开" || reason == "self";
                                    let mut sig = signals.lock().expect("signals 锁");
                                    if local {
                                        sig.mark_peer_idle(&pid);
                                    } else {
                                        sig.mark_peer_closed(&pid);
                                    }
                                }
                            }
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
                            &mut candidates, &mut next_conn, &dl_flow,
                        );
                    }
                    PeerIn::ProtocolViolation => {
                        ban_and_drop(
                            &mut conns,
                            &mut candidates,
                            &mut order_cache,
                            &pmap,
                            conn_id,
                            &shared,
                            &evt_tx,
                            BanKind::Protocol,
                        )
                        .await;
                        let views = conn_views(&mut conns, pmap.covered_pieces(), &done, optimistic_slot);
                        let _ = evt_tx.send(Evt::BtConns { id, conns: views }).await;
                    }
                    PeerIn::Msg(msg) => {
                        let ban = handle_peer_msg(
                            conn_id,
                            msg,
                            &mut conns,
                            &pmap,
                            &spec,
                            &tun,
                            shared.upload_read_permits(),
                            shared.ul_throttle(),
                            &task_ul,
                            &mut done,
                            &mut downloaded,
                            &mut files_handles,
                            &mut partial,
                            &mut order_cache,
                            upload_paused,
                        )
                        .await;
                        if let Some(kind) = ban {
                            ban_and_drop(
                                &mut conns,
                                &mut candidates,
                                &mut order_cache,
                                &pmap,
                                conn_id,
                                &shared,
                                &evt_tx,
                                kind,
                            )
                            .await;
                            let views = conn_views(&mut conns, pmap.covered_pieces(), &done, optimistic_slot);
                            let _ = evt_tx.send(Evt::BtConns { id, conns: views }).await;
                        }
                        if downloaded != prev_downloaded {
                            {
                                let mut st = live.lock().expect("live stats 锁");
                                st.downloaded = downloaded;
                                st.left = pmap.selected_total().saturating_sub(downloaded);
                            }
                            // 逐块事件节流：250ms 窗内的块到达不重复发 BtProgress/BtConns。
                            // 大种子下逐块克隆 done/partial/active 全集（O(P)×3）与逐块
                            // conn_views 曾随块率线性放大引擎/应用两侧开销；250ms 粒度
                            // 对进度与速度显示足够（qBittorrent 同为秒级刷新）。
                            if progress_emit_due(last_emit, std::time::Instant::now(), tun.progress_emit_min_interval_ms) {
                                last_emit = std::time::Instant::now();
                                let views = conn_views(&mut conns, pmap.covered_pieces(), &done, optimistic_slot);
                                let _ = evt_tx.send(Evt::BtProgress {
                                    id,
                                    downloaded,
                                    chunk_done: done.len() as u32,
                                    done_blocks: done.iter().copied().collect::<Vec<_>>(),
                                    partial_blocks: partial_piece_list(&partial, &done),
                                    active_blocks: inflight_piece_list(&conns, &done),
                                    uploaded: uploaded_total(&conns),
                                }).await;
                                let _ = evt_tx.send(Evt::BtConns { id, conns: views }).await;
                            }
                        }
                        if !seeding && all_selected_done(&done, &pmap) {
                            seeding = true;
                            upload_paused = false;
                            sidecar_saved_done = done.len();
                            live.lock().expect("live stats 锁").completed = true;
                            for n in &completed_notifies {
                                n.notify_one();
                            }
                            let _ = crate::model::BtSidecar::remove(&sidecar_path);
                            optimistic_slot = apply_choker(
                                &mut conns, true, false, tun.upload_slots, optimistic_slot,
                                &mut optimistic_cursor,
                            )
                            .await;
                            let _ = evt_tx.send(Evt::BtDone { id, total: pmap.selected_total() }).await;
                        }
                    }
                }
            }
            _ = tick.tick() => {
                let now = std::time::Instant::now();
                if !seeding {
                    let mut evict: Vec<usize> = Vec::new();
                    for (cid, st) in conns.iter_mut() {
                        refresh_idle(st, now);
                        if idle_evict_due(st, now, tun.idle_eviction_secs) {
                            evict.push(*cid);
                        }
                    }
                    for cid in evict {
                        if let Some(st) = conns.remove(&cid) {
                            if let Some(key) = addr_sock(&st.addr) {
                                if let Some(c) = candidates.get_mut(&key) {
                                    c.last_try = Some(std::time::Instant::now());
                                }
                            }
                            let _ = st.peer_out.send(PeerOut::Drop).await;
                        }
                    }
                    let mut leech_bans: Vec<usize> = Vec::new();
                    for (cid, st) in conns.iter() {
                        if st.cum_up >= LEECH_UP_MIN
                            && st.cum_down < LEECH_DOWN_MAX
                            && st.connected_at.elapsed().as_secs() >= LEECH_MIN_ALIVE_SECS
                        {
                            leech_bans.push(*cid);
                        }
                    }
                    for cid in leech_bans {
                        ban_and_drop(
                            &mut conns,
                            &mut candidates,
                            &mut order_cache,
                            &pmap,
                            cid,
                            &shared,
                            &evt_tx,
                            BanKind::Leech,
                        )
                        .await;
                    }
                    if conns.len() < spec.concurrency.max(1) {
                        fill_peer_slots(
                            &spec, &peer_id, &peer_tx, shared.as_ref(), &mut conns,
                            &mut candidates, &mut next_conn, &dl_flow,
                        );
                    }
                }
                let mut stale: Vec<usize> = Vec::new();
                for (cid, st) in conns.iter_mut() {
                    tick_pipeline(st, now, tun.peer_pipeline);
                    let before = st.inflight.len();
                    st.inflight
                        .retain(|_, (_, at)| now.duration_since(*at).as_secs() < tun.request_timeout_secs);
                    if st.inflight.len() != before {
                        stale.push(*cid);
                    }
                }
                for cid in stale {
                    let claimed = claimed_blocks(&conns, &partial);
                    if let Some(st) = conns.get_mut(&cid) {
                        issue_requests(
                            cid, st, &claimed, &pmap, &tun, &done, &partial, &mut order_cache,
                        )
                        .await;
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
                if !upload_paused
                    && now.duration_since(last_rechoke).as_secs() >= tun.rechoke_interval_secs
                {
                    let rotate =
                        now.duration_since(last_optimistic).as_secs() >= tun.optimistic_interval_secs;
                    if rotate {
                        last_optimistic = now;
                    }
                    optimistic_slot = apply_choker(
                        &mut conns, seeding, rotate, tun.upload_slots, optimistic_slot,
                        &mut optimistic_cursor,
                    )
                    .await;
                    last_rechoke = now;
                }
                if !seeding && all_selected_done(&done, &pmap) {
                    seeding = true;
                    upload_paused = false;
                    sidecar_saved_done = done.len();
                    live.lock().expect("live stats 锁").completed = true;
                    for n in &completed_notifies {
                        n.notify_one();
                    }
                    let _ = crate::model::BtSidecar::remove(&sidecar_path);
                    optimistic_slot = apply_choker(
                        &mut conns, true, false, tun.upload_slots, optimistic_slot,
                        &mut optimistic_cursor,
                    )
                    .await;
                    let _ = evt_tx.send(Evt::BtDone { id, total: pmap.selected_total() }).await;
                }
                last_emit = now;
                let views = conn_views(&mut conns, pmap.covered_pieces(), &done, optimistic_slot);
                let _ = evt_tx.send(Evt::BtProgress {
                    id,
                    downloaded,
                    chunk_done: done.len() as u32,
                    done_blocks: done.iter().copied().collect::<Vec<_>>(),
                    partial_blocks: partial_piece_list(&partial, &done),
                    active_blocks: inflight_piece_list(&conns, &done),
                    uploaded: uploaded_total(&conns),
                }).await;
                let _ = evt_tx.send(Evt::BtConns { id, conns: views }).await;
            }
            _ = sweep.tick() => {
                fill_peer_slots(
                    &spec, &peer_id, &peer_tx, shared.as_ref(), &mut conns,
                    &mut candidates, &mut next_conn, &dl_flow,
                );
            }
            else => break,
        }
    }
}

type PartialPieces = HashMap<u32, BTreeMap<u32, Vec<u8>>>;

fn addr_ip(addr: &str) -> Option<IpAddr> {
    addr_sock(addr).map(|(ip, _)| ip)
}

fn addr_sock(addr: &str) -> Option<(IpAddr, u16)> {
    let (host, port) = addr.rsplit_once(':')?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    Some((host.parse().ok()?, port.parse().ok()?))
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

    fn test_ul() -> std::sync::Arc<crate::engine::Throttle> {
        std::sync::Arc::new(crate::engine::Throttle::new(0))
    }
    use std::collections::BTreeMap;

    use sha1::{Digest, Sha1};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    use super::*;
    use crate::bt::bencode::{encode, BValue};
    use crate::bt::metainfo::TorrentMeta;
    use crate::bt::webrtc::PeerTransports;
    use crate::bt::REQUEST_BLOCK_LEN;
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
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
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

    async fn fake_udp_tracker_logging(
        seeder_port: u16,
        announces: Arc<std::sync::Mutex<Vec<[u8; 20]>>>,
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
                    let mut out = Vec::new();
                    out.extend_from_slice(&0u32.to_be_bytes());
                    out.extend_from_slice(&txid.to_be_bytes());
                    out.extend_from_slice(&0x1234_5678_9abc_def0u64.to_be_bytes());
                    let _ = sock.send_to(&out, from).await;
                } else if action == 1 {
                    let conn_id = u64::from_be_bytes(req[0..8].try_into().unwrap());
                    if conn_id != 0x1234_5678_9abc_def0 {
                        continue;
                    }
                    let ih: [u8; 20] = req[16..36].try_into().unwrap();
                    announces.lock().expect("announce 日志锁").push(ih);
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
    async fn engine_tracker_pool_hot_update() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..64 * 1024usize).map(|i| (i % 251) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-hotupd");
        let save_dir = dir.to_string_lossy().into_owned();
        let meta = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let seeder_port = fake_seeder(data.clone(), piece_len, meta.infohash).await;
        let log_a: Arc<std::sync::Mutex<Vec<[u8; 20]>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let log_b: Arc<std::sync::Mutex<Vec<[u8; 20]>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let port_a = fake_udp_tracker_logging(seeder_port, log_a.clone()).await;
        let port_b = fake_udp_tracker_logging(seeder_port, log_b.clone()).await;

        let spec = crate::engine::BtSpec {
            id: 91,
            source: "/tmp/hotupd.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "hotupd.bin".into(),
            files: vec![BtFile {
                path: "hotupd.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{port_a}/announce")],
            concurrency: 2,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
        };
        let (evt_tx, _evt_rx) = mpsc::channel(256);
        let (cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let shared = crate::engine::shared_for_tests();
        let engine_handle = tokio::spawn(run(spec, shared, cmd_rx, evt_tx));

        async fn wait_announces(
            log: &Arc<std::sync::Mutex<Vec<[u8; 20]>>>,
            want: usize,
            secs: u64,
        ) {
            let deadline = std::time::Instant::now() + Duration::from_secs(secs);
            while std::time::Instant::now() < deadline {
                if log.lock().expect("锁").len() >= want {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            panic!("announce 未达 {want} 次");
        }

        wait_announces(&log_a, 1, 10).await;
        assert_eq!(
            log_a.lock().unwrap()[0],
            meta.infohash,
            "tracker-A 收到该任务 infohash 的 announce"
        );
        assert!(
            log_b.lock().unwrap().is_empty(),
            "未更新的 tracker-B 尚无 announce"
        );

        cmd_tx
            .send(BtTaskCmd::UpdateTrackers(vec![format!(
                "udp://127.0.0.1:{port_b}/announce"
            )]))
            .await
            .unwrap();

        wait_announces(&log_b, 1, 10).await;
        assert_eq!(
            log_b.lock().unwrap()[0],
            meta.infohash,
            "热更新后 tracker-B 立即被 announce（同一 infohash，任务不重挂）"
        );
        let a_count = log_a.lock().unwrap().len();
        tokio::time::sleep(Duration::from_secs(3)).await;
        assert_eq!(
            log_a.lock().unwrap().len(),
            a_count,
            "被移除的 tracker-A 不再收到后续 announce（旧池已中止）"
        );
        let still_alive = cmd_tx
            .send(BtTaskCmd::UpdateTrackers(vec![format!(
                "udp://127.0.0.1:{port_b}/announce"
            )]))
            .await
            .is_ok();
        assert!(still_alive, "热更新后任务保持运行（不重挂引擎）");
        engine_handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn engine_downloads_via_udp_tracker_and_seeder() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..8 * 1024 * 1024usize).map(|i| (i % 251) as u8).collect();
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
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
        };
        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let shared = crate::engine::shared_for_tests();
        let engine_handle = tokio::spawn(run(spec, shared, cmd_rx, evt_tx));

        let mut saw_progress = false;
        let mut saw_conns = false;
        let mut done_seen = false;
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
                    done_seen = true;
                }
                Evt::BtFailed { reason, .. } => panic!("意外失败：{reason}"),
                _ => {}
            }
            // 逐块事件 250ms 节流后，小负载可能在窗内完成而看不到中间
            // 进度事件；完成后的 1s tick 必发 BtProgress，等到它再断言。
            if done_seen && saw_progress && saw_conns {
                break;
            }
        }
        assert!(done_seen, "下载完成事件在位");
        assert!(saw_progress, "进度事件有落盘量（完成后的 tick 必发）");
        assert!(saw_conns, "连接明细事件在位");
        let out = std::fs::read(format!("{save_dir}/e2e.bin")).unwrap();
        assert_eq!(out, data, "产物与 seeder 数据逐字节一致");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn engine_task_speed_limit_throttles_download() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..512 * 1024usize).map(|i| (i % 251) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-tlimit");
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
            id: 79,
            source: "/tmp/e2e-tlimit.torrent".into(),
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
            upload_paused: false,
            max_download_speed: 100_000,
            max_upload_speed: 0,
            tunables: Default::default(),
        };
        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let shared = crate::engine::shared_for_tests();
        let engine_handle = tokio::spawn(run(spec, shared, cmd_rx, evt_tx));

        let start = std::time::Instant::now();
        let mut done_seen = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            let remain = deadline.saturating_duration_since(std::time::Instant::now());
            let Some(evt) = tokio::time::timeout(remain, evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                panic!("限速下载超时（已等 {:.1}s）", start.elapsed().as_secs_f64());
            };
            match evt {
                Evt::BtDone { id, total } => {
                    assert_eq!(id, 79);
                    assert_eq!(total, data.len() as u64);
                    done_seen = true;
                }
                Evt::BtFailed { reason, .. } => panic!("意外失败：{reason}"),
                _ => {}
            }
            if done_seen {
                break;
            }
        }
        let took = start.elapsed().as_secs_f64();
        assert!(
            took >= 4.5,
            "BT 任务级限速 100KB/s 下载 512KiB 应 ≥4.5s，实际 {took:.2}s"
        );
        let out = std::fs::read(format!("{save_dir}/e2e.bin")).unwrap();
        assert_eq!(out, data, "产物与 seeder 数据逐字节一致");
        engine_handle.abort();
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
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
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

    async fn counting_seeder_any(
        data: Vec<u8>,
        piece_len: usize,
        infohash: [u8; 20],
        counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        peer_id: &[u8; 20],
    ) -> u16 {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let data = std::sync::Arc::new(data);
        let peer_id = *peer_id;
        tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = listener.accept().await else {
                    return;
                };
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let data = data.clone();
                tokio::spawn(async move {
                    serve_seeder_session_as(sock, &data, piece_len, infohash, &peer_id).await;
                });
            }
        });
        port
    }

    async fn serve_seeder_session(
        sock: TcpStream,
        data: &[u8],
        piece_len: usize,
        infohash: [u8; 20],
    ) {
        serve_seeder_session_as(sock, data, piece_len, infohash, b"-QX7777-seederpeer00").await;
    }

    async fn corrupt_seeder(
        data: Vec<u8>,
        piece_len: usize,
        infohash: [u8; 20],
        counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) -> u16 {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
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
                    let mut sock = sock;
                    let mut hs = [0u8; 68];
                    if tokio::io::AsyncReadExt::read_exact(&mut sock, &mut hs)
                        .await
                        .is_err()
                    {
                        return;
                    }
                    let mut resp = Vec::new();
                    resp.push(19);
                    resp.extend_from_slice(b"BitTorrent protocol");
                    resp.extend_from_slice(&[0u8; 8]);
                    resp.extend_from_slice(&infohash);
                    resp.extend_from_slice(b"-QX7777-corrupter000");
                    if tokio::io::AsyncWriteExt::write_all(&mut sock, &resp)
                        .await
                        .is_err()
                    {
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
                    let _ = tokio::io::AsyncWriteExt::write_all(&mut sock, &frame).await;
                    let _ = tokio::io::AsyncWriteExt::write_all(&mut sock, &[0, 0, 0, 1, 1]).await;
                    loop {
                        match peer::read_msg(&mut sock).await {
                            Ok(peer::PeerMsg::Request {
                                index,
                                begin,
                                length,
                            }) => {
                                let mut blk = vec![0u8; length as usize];
                                for (i, b) in blk.iter_mut().enumerate() {
                                    *b = (i % 251) as u8;
                                }
                                let mut f = Vec::new();
                                f.extend_from_slice(&((9 + blk.len()) as u32).to_be_bytes());
                                f.push(7);
                                f.extend_from_slice(&index.to_be_bytes());
                                f.extend_from_slice(&begin.to_be_bytes());
                                f.extend_from_slice(&blk);
                                if tokio::io::AsyncWriteExt::write_all(&mut sock, &f)
                                    .await
                                    .is_err()
                                {
                                    return;
                                }
                            }
                            Ok(_) => {}
                            Err(_) => return,
                        }
                    }
                });
            }
        });
        port
    }

    async fn serve_seeder_session_as(
        mut sock: TcpStream,
        data: &[u8],
        piece_len: usize,
        infohash: [u8; 20],
        peer_id: &[u8; 20],
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
        resp.extend_from_slice(peer_id);
        if sock.write_all(&resp).await.is_err() {
            return;
        }
        let _ = serve_seeder_stream(sock, data, piece_len).await;
    }

    async fn serve_seeder_stream(
        mut sock: TcpStream,
        data: &[u8],
        piece_len: usize,
    ) -> Result<(), BtError> {
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
                        return Ok(());
                    }
                    let blk = data[off..end].to_vec();
                    let mut f = Vec::new();
                    f.extend_from_slice(&((9 + blk.len()) as u32).to_be_bytes());
                    f.push(7);
                    f.extend_from_slice(&index.to_be_bytes());
                    f.extend_from_slice(&begin.to_be_bytes());
                    f.extend_from_slice(&blk);
                    if let Err(e) = sock.write_all(&f).await {
                        return Err(e.into());
                    }
                }
                Ok(_) => {}
                Err(e) => return Err(e),
            }
        }
    }

    #[derive(Default)]
    struct SeederProbe {
        peer_ids: Vec<[u8; 20]>,
        close_kind: Option<String>,
    }

    async fn probing_seeder(
        data: Vec<u8>,
        piece_len: usize,
        infohash: [u8; 20],
        counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        probe: std::sync::Arc<std::sync::Mutex<SeederProbe>>,
    ) -> u16 {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let data = std::sync::Arc::new(data);
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let data = data.clone();
                let probe = probe.clone();
                tokio::spawn(async move {
                    let mut hs = [0u8; 68];
                    if sock.read_exact(&mut hs).await.is_err() {
                        return;
                    }
                    let mut pid = [0u8; 20];
                    pid.copy_from_slice(&hs[48..68]);
                    probe.lock().expect("probe 锁").peer_ids.push(pid);
                    let mut resp = Vec::new();
                    resp.push(19);
                    resp.extend_from_slice(b"BitTorrent protocol");
                    resp.extend_from_slice(&[0u8; 8]);
                    resp.extend_from_slice(&infohash);
                    resp.extend_from_slice(b"-QX7777-probepeer000");
                    if sock.write_all(&resp).await.is_err() {
                        return;
                    }
                    if let Err(e) = serve_seeder_stream(sock, &data, piece_len).await {
                        probe.lock().expect("probe 锁").close_kind = Some(e.to_string());
                    }
                });
            }
        });
        port
    }

    type PeerBehavior = std::sync::Arc<
        dyn Fn(
                tokio::net::TcpStream,
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
            + Send
            + Sync,
    >;

    async fn ban_probe_peer(infohash: [u8; 20], behavior: PeerBehavior) -> u16 {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = listener.accept().await else {
                    return;
                };
                let behavior = behavior.clone();
                tokio::spawn(async move {
                    let mut sock = sock;
                    let mut hs = [0u8; 68];
                    if tokio::io::AsyncReadExt::read_exact(&mut sock, &mut hs)
                        .await
                        .is_err()
                    {
                        return;
                    }
                    let mut resp = Vec::new();
                    resp.push(19);
                    resp.extend_from_slice(b"BitTorrent protocol");
                    resp.extend_from_slice(&[0u8; 8]);
                    resp.extend_from_slice(&infohash);
                    resp.extend_from_slice(b"-QX7777-banprobe0000");
                    if tokio::io::AsyncWriteExt::write_all(&mut sock, &resp)
                        .await
                        .is_err()
                    {
                        return;
                    }
                    behavior(sock).await;
                });
            }
        });
        port
    }

    fn expect_ban_toast(evt: &Evt) -> bool {
        matches!(evt, Evt::Toast(m) if m.contains("⛔ 已封禁"))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn unknown_message_id_bans_peer_immediately() {
        use std::sync::atomic::AtomicUsize;
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..16_384usize).map(|i| (i % 7) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-unkmsg");
        let save_dir = dir.to_string_lossy().into_owned();
        let counter = std::sync::Arc::new(AtomicUsize::new(0));
        let meta0 = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let peer_port = ban_probe_peer(
            meta0.infohash,
            std::sync::Arc::new(|sock| {
                Box::pin(async move {
                    let mut sock = sock;
                    use tokio::io::AsyncWriteExt;
                    let _ = sock.write_all(&[0, 0, 0, 2, 99, 255]).await;
                    let _ = sock.flush().await;
                    tokio::time::sleep(Duration::from_secs(3)).await;
                })
            }),
        )
        .await;
        let tracker_port = fake_udp_tracker_fast(peer_port).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );
        let _ = counter;
        let shared = crate::engine::shared_for_tests();
        let spec = crate::engine::BtSpec {
            id: 61,
            source: "/tmp/e2e-unk.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "e2e.bin".into(),
            files: vec![BtFile {
                path: "e2e.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 2,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
        };
        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let task = tokio::spawn(run(spec, shared.clone(), cmd_rx, evt_tx));
        let mut banned = false;
        let mut saw_toast = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            let Some(evt) = tokio::time::timeout(Duration::from_millis(400), evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                continue;
            };
            if expect_ban_toast(&evt) {
                saw_toast = true;
            }
            if shared.is_blacklisted(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)) {
                banned = true;
            }
            if banned && saw_toast {
                break;
            }
        }
        assert!(banned, "未知消息 id 首次即封禁（协议违规）");
        assert!(saw_toast, "⛔ toast 可观察");
        task.abort();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn illegal_requests_three_strikes_ban() {
        use std::sync::atomic::AtomicUsize;
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..16_384usize).map(|i| (i % 11) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-badreq");
        let save_dir = dir.to_string_lossy().into_owned();
        let counter = std::sync::Arc::new(AtomicUsize::new(0));
        let meta0 = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let peer_port = ban_probe_peer(
            meta0.infohash,
            std::sync::Arc::new(|sock| {
                Box::pin(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut sock = sock;
                    let _ = sock.write_all(&[0, 0, 0, 1, 2]).await;
                    let _ = sock.flush().await;
                    let deadline = std::time::Instant::now() + Duration::from_secs(8);
                    while std::time::Instant::now() < deadline {
                        let mut len_buf = [0u8; 4];
                        match tokio::time::timeout(
                            Duration::from_millis(300),
                            sock.read_exact(&mut len_buf),
                        )
                        .await
                        {
                            Ok(Ok(_)) => {}
                            _ => continue,
                        }
                        let len = u32::from_be_bytes(len_buf) as usize;
                        if len == 0 {
                            continue;
                        }
                        let mut body = vec![0u8; len];
                        if sock.read_exact(&mut body).await.is_err() {
                            return;
                        }
                        if body[0] == 1 {
                            let _ = sock
                                .write_all(&[0, 0, 0, 13, 6, 0, 0, 9, 255, 0, 0, 0, 0, 0, 16, 0, 0])
                                .await;
                            let _ = sock
                                .write_all(&[0, 0, 0, 13, 6, 0, 0, 9, 255, 0, 0, 0, 0, 0, 16, 0, 0])
                                .await;
                            let _ = sock
                                .write_all(&[0, 0, 0, 13, 6, 0, 0, 9, 255, 0, 0, 0, 0, 0, 16, 0, 0])
                                .await;
                            let _ = sock.flush().await;
                            break;
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                })
            }),
        )
        .await;
        let tracker_port = fake_udp_tracker_fast(peer_port).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );
        let _ = counter;
        let shared = crate::engine::shared_for_tests();
        let spec = crate::engine::BtSpec {
            id: 62,
            source: "/tmp/e2e-badreq.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "e2e.bin".into(),
            files: vec![BtFile {
                path: "e2e.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 2,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: crate::engine::BtTunables {
                rechoke_interval_secs: 1,
                ..Default::default()
            },
        };
        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let task = tokio::spawn(run(spec, shared.clone(), cmd_rx, evt_tx));
        let mut banned = false;
        let mut saw_toast = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(12);
        while std::time::Instant::now() < deadline {
            let Some(evt) = tokio::time::timeout(Duration::from_millis(400), evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                continue;
            };
            if expect_ban_toast(&evt) {
                saw_toast = true;
            }
            if shared.is_blacklisted(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)) {
                banned = true;
            }
            if banned && saw_toast {
                break;
            }
        }
        assert!(banned, "非法区间 request 重复 3 次封禁");
        assert!(saw_toast, "⛔ toast 可观察");
        task.abort();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn corrupt_seeder_three_bad_pieces_ban() {
        use std::sync::atomic::AtomicUsize;
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..16_384usize).map(|i| (i % 13) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-badpiece");
        let save_dir = dir.to_string_lossy().into_owned();
        let counter = std::sync::Arc::new(AtomicUsize::new(0));
        let meta0 = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let seeder_port =
            corrupt_seeder(data.clone(), piece_len, meta0.infohash, counter.clone()).await;
        let tracker_port = fake_udp_tracker_fast(seeder_port).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );
        let shared = crate::engine::shared_for_tests();
        let spec = crate::engine::BtSpec {
            id: 63,
            source: "/tmp/e2e-badpiece.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "e2e.bin".into(),
            files: vec![BtFile {
                path: "e2e.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 2,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
        };
        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let task = tokio::spawn(run(spec, shared.clone(), cmd_rx, evt_tx));
        let mut banned = false;
        let mut saw_toast = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while std::time::Instant::now() < deadline {
            let Some(evt) = tokio::time::timeout(Duration::from_millis(400), evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                continue;
            };
            if expect_ban_toast(&evt) {
                saw_toast = true;
            }
            if shared.is_blacklisted(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)) {
                banned = true;
            }
            if banned && saw_toast {
                break;
            }
        }
        assert!(banned, "连续 3 次坏 piece 封禁（归属交付方）");
        assert!(saw_toast, "⛔ toast 可观察");
        task.abort();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tracker_pool_emits_per_url_status_events() {
        use std::sync::atomic::AtomicUsize;
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..16_384usize).map(|i| (i % 5) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-trksta");
        let save_dir = dir.to_string_lossy().into_owned();
        let counter = std::sync::Arc::new(AtomicUsize::new(0));
        let meta0 = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let seeder_port =
            counting_seeder(data.clone(), piece_len, meta0.infohash, counter.clone()).await;
        let tracker_port = fake_udp_tracker_fast(seeder_port).await;
        let url = format!("udp://127.0.0.1:{tracker_port}/announce");
        let meta = make_meta(&data, piece_len, &url);
        let shared = crate::engine::shared_for_tests();
        let spec = crate::engine::BtSpec {
            id: 64,
            source: "/tmp/e2e-trk.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "e2e.bin".into(),
            files: vec![BtFile {
                path: "e2e.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![url.clone()],
            concurrency: 2,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
        };
        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (_cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let task = tokio::spawn(run(spec, shared.clone(), cmd_rx, evt_tx));
        let mut got = None;
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            let Some(evt) = tokio::time::timeout(Duration::from_millis(400), evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                continue;
            };
            if let Evt::BtTrackers {
                url: u,
                working,
                seeds,
                next_in_secs,
                ..
            } = evt
            {
                if u == url && working {
                    got = Some((seeds, next_in_secs));
                    break;
                }
            }
        }
        let (seeds, interval) = got.expect("应收到 tracker 工作中状态事件");
        assert!(interval >= 1, "下轮 announce 倒计时 > 0");
        assert!(seeds.is_some(), "tracker 应答携带 swarm 计数");
        task.abort();
        std::fs::remove_dir_all(&dir).ok();
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
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
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
    async fn upload_read_plan_multi_file_reads_only_requested_slice() {
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
        let save_dir = dir.to_string_lossy().into_owned();
        let plan = upload_read_plan(&pmap, &save_dir, 1, 32, 32).expect("跨文件区间可出读盘计划");
        let out = read_upload_blocks(plan, 32).await.expect("跨文件区间可读");
        let mut expect = Vec::new();
        expect.extend_from_slice(&a[96..100]);
        expect.extend_from_slice(&b[0..28]);
        assert_eq!(out, expect, "仅读请求区间（piece1 全局 [96..128)）");
        assert!(
            upload_read_plan(&pmap, &save_dir, 1, 60, 8).is_none(),
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
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
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
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
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
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
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

    #[test]
    fn save_bt_sidecar_full_persists_done_and_partial_blocks() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btsc-full");
        let path = crate::model::namegen::sidecar_path_of(&dir.to_string_lossy(), "full.bin");
        let mut done = HashSet::new();
        done.insert(0);
        let mut partial: PartialPieces = HashMap::new();
        let mut blocks0 = BTreeMap::new();
        blocks0.insert(0u32, vec![0xAA; 16]);
        blocks0.insert(16384u32, vec![0xBB; 16]);
        partial.insert(1, blocks0);
        let mut blocks_done = BTreeMap::new();
        blocks_done.insert(0u32, vec![0xCC; 16]);
        partial.insert(0, blocks_done);
        save_bt_sidecar_full(&path, &done, &partial);
        let sc = crate::model::BtSidecar::load_full(&path).expect("sidecar 可读");
        assert_eq!(sc.done_pieces, vec![0]);
        assert_eq!(sc.partial_pieces.len(), 1, "done piece 的 partial 不导出");
        let p1 = &sc.partial_pieces[0];
        assert_eq!(p1.piece, 1);
        assert_eq!(
            p1.blocks.iter().map(|b| b.begin).collect::<Vec<_>>(),
            vec![0, 16384]
        );
        assert_eq!(p1.blocks[0].decode_data().unwrap(), vec![0xAA; 16]);
        assert_eq!(p1.blocks[1].decode_data().unwrap(), vec![0xBB; 16]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn restore_sidecar_partial_writes_verified_and_filters_invalid() {
        let piece_len = 3 * REQUEST_BLOCK_LEN as usize;
        let data: Vec<u8> = (0..2 * piece_len).map(|i| (i % 211) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bt-restore");
        let save_dir = dir.to_string_lossy().into_owned();
        let meta = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let files = vec![BtFile {
            path: "r.bin".into(),
            size: data.len() as u64,
            selected: true,
        }];
        let pmap = PieceMap::new(&files, piece_len as u32);
        let mut handles: HashMap<usize, tokio::fs::File> = HashMap::new();
        handles.insert(
            0,
            open_selected_file_handle(&save_dir, &files[0])
                .await
                .unwrap(),
        );

        let blk = |piece: u32, idx: usize| {
            crate::model::BtPartialBlock::encode_block(
                (idx * REQUEST_BLOCK_LEN as usize) as u32,
                &data[piece as usize * piece_len + idx * REQUEST_BLOCK_LEN as usize
                    ..(piece as usize + 1) * piece_len],
            )
        };
        let full0 = crate::model::BtPartialPiece {
            piece: 0,
            blocks: vec![blk(0, 0), blk(0, 1), blk(0, 2)],
        };
        let uncovered = crate::model::BtPartialPiece {
            piece: 9,
            blocks: vec![crate::model::BtPartialBlock::encode_block(0, &[1u8; 16])],
        };
        let misaligned = crate::model::BtPartialPiece {
            piece: 1,
            blocks: vec![crate::model::BtPartialBlock::encode_block(1, &[1u8; 16])],
        };
        let corrupt: crate::model::BtPartialPiece =
            serde_json::from_str(r#"{"piece":1,"blocks":[{"begin":16384,"data_b64":"!!!"}]}"#)
                .unwrap();
        let mut done = HashSet::new();
        done.insert(1);
        let mut partial = PartialPieces::new();
        let mut downloaded = 0u64;

        restore_sidecar_partial(
            &[full0, uncovered, misaligned, corrupt],
            &pmap,
            &meta,
            &mut handles,
            &mut done,
            &mut partial,
            &mut downloaded,
        )
        .await;

        assert!(done.contains(&0), "完整且校验通过 → 直接落盘转 done");
        assert_eq!(downloaded, selected_bytes_of_piece(&pmap, 0));
        assert!(partial.is_empty(), "无效条目全部滤除，不进入 partial");
        let buf = std::fs::read(format!("{save_dir}/r.bin")).unwrap();
        assert_eq!(&buf[..piece_len], &data[..piece_len], "piece 0 数据落盘");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn restore_sidecar_partial_keeps_incomplete_for_resume() {
        let piece_len = 3 * REQUEST_BLOCK_LEN as usize;
        let data: Vec<u8> = (0..piece_len).map(|i| (i % 223) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bt-restore-inc");
        let save_dir = dir.to_string_lossy().into_owned();
        let meta = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let files = vec![BtFile {
            path: "i.bin".into(),
            size: data.len() as u64,
            selected: true,
        }];
        let pmap = PieceMap::new(&files, piece_len as u32);
        let mut handles: HashMap<usize, tokio::fs::File> = HashMap::new();
        handles.insert(
            0,
            open_selected_file_handle(&save_dir, &files[0])
                .await
                .unwrap(),
        );
        let entries = vec![crate::model::BtPartialPiece {
            piece: 0,
            blocks: vec![crate::model::BtPartialBlock::encode_block(
                0,
                &data[..REQUEST_BLOCK_LEN as usize],
            )],
        }];
        let mut done = HashSet::new();
        let mut partial = PartialPieces::new();
        let mut downloaded = 7u64;

        restore_sidecar_partial(
            &entries,
            &pmap,
            &meta,
            &mut handles,
            &mut done,
            &mut partial,
            &mut downloaded,
        )
        .await;

        assert!(done.is_empty(), "不完整 piece 不转 done");
        assert_eq!(downloaded, 7, "未校验字节不计入 downloaded");
        let blocks = partial.get(&0).expect("不完整 piece 留在 partial");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks.get(&0).unwrap(), &data[..REQUEST_BLOCK_LEN as usize]);
        let buf = std::fs::read(format!("{save_dir}/i.bin")).unwrap();
        assert_eq!(
            buf.len(),
            data.len(),
            "句柄打开即预分配全量（set_len 稀疏文件）"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    async fn fake_logging_peer(
        data: Vec<u8>,
        piece_len: usize,
        infohash: [u8; 20],
        serve: std::collections::BTreeSet<(u32, u32)>,
        req_log: std::sync::Arc<std::sync::Mutex<std::collections::BTreeSet<(u32, u32)>>>,
        served_tx: tokio::sync::mpsc::UnboundedSender<(u32, u32)>,
    ) -> u16 {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let data = data.clone();
                let serve = serve.clone();
                let req_log = req_log.clone();
                let served_tx = served_tx.clone();
                tokio::spawn(async move {
                    let mut hs = [0u8; 68];
                    if sock.read_exact(&mut hs).await.is_err() {
                        return;
                    }
                    let mut resp = Vec::new();
                    resp.push(19);
                    resp.extend_from_slice(b"BitTorrent protocol");
                    resp.extend_from_slice(&[0u8; 8]);
                    resp.extend_from_slice(&infohash);
                    resp.extend_from_slice(b"-QX7777-loggingpeer0");
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
                    if sock.write_all(&frame).await.is_err() {
                        return;
                    }
                    if sock.write_all(&[0, 0, 0, 1, 1]).await.is_err() {
                        return;
                    }
                    loop {
                        match peer::read_msg(&mut sock).await {
                            Ok(peer::PeerMsg::Request {
                                index,
                                begin,
                                length,
                            }) => {
                                req_log.lock().unwrap().insert((index, begin));
                                if serve.contains(&(index, begin)) {
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
                                    let _ = served_tx.send((index, begin));
                                }
                            }
                            Ok(peer::PeerMsg::Interested) => {}
                            Ok(peer::PeerMsg::KeepAlive) => {}
                            Ok(_) => {}
                            Err(_) => return,
                        }
                    }
                });
            }
        });
        port
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pause_persists_partial_blocks_and_resume_skips_restored_requests() {
        let piece_len = 3 * REQUEST_BLOCK_LEN as usize;
        let num_pieces = 3usize;
        let data: Vec<u8> = (0..piece_len * num_pieces)
            .map(|i| (i % 199) as u8)
            .collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bt-antileech");
        let save_dir = dir.to_string_lossy().into_owned();
        let name = "al.bin";
        std::fs::write(format!("{save_dir}/{name}"), &data).unwrap();
        let meta = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let infohash = meta.infohash;
        let files = vec![BtFile {
            path: name.to_string(),
            size: data.len() as u64,
            selected: true,
        }];
        let sc_path = crate::model::namegen::sidecar_path_of(&save_dir, name);

        let (served_tx, mut served_rx) = mpsc::unbounded_channel();
        let log1 = Arc::new(std::sync::Mutex::new(std::collections::BTreeSet::new()));
        let mut serve1 = std::collections::BTreeSet::new();
        for p in 0..num_pieces as u32 {
            for b in 0..3u32 {
                serve1.insert((p, b * REQUEST_BLOCK_LEN));
            }
        }
        serve1.remove(&(0u32, REQUEST_BLOCK_LEN));
        serve1.remove(&(0u32, 2 * REQUEST_BLOCK_LEN));
        let seeder1 = fake_logging_peer(
            data.clone(),
            piece_len,
            infohash,
            serve1,
            log1.clone(),
            served_tx,
        )
        .await;
        let tracker1 = fake_udp_tracker(seeder1).await;
        let spec = crate::engine::BtSpec {
            id: 96,
            source: "/tmp/al.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: name.to_string(),
            files: files.clone(),
            trackers: vec![format!("udp://127.0.0.1:{tracker1}/announce")],
            concurrency: 2,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
        };
        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let handle = tokio::spawn(run(spec, crate::engine::shared_for_tests(), cmd_rx, evt_tx));

        let mut served00 = false;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout_at(deadline, served_rx.recv()).await {
                Ok(Some((0, 0))) => {
                    served00 = true;
                    break;
                }
                Ok(Some(_)) => continue,
                _ => break,
            }
        }
        assert!(served00, "phase1: 假 peer 已服务 piece0 块0");
        tokio::time::sleep(Duration::from_millis(1500)).await;
        cmd_tx.send(BtTaskCmd::Pause).await.unwrap();
        let mut stopped = false;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while tokio::time::Instant::now() < deadline {
            let remain = deadline.saturating_duration_since(tokio::time::Instant::now());
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
        assert!(stopped, "phase1: 暂停回报 BtStopped");
        let _ = handle.await;
        let sc = crate::model::BtSidecar::load_full(&sc_path).expect("phase1: sidecar 存在");
        assert!(
            !sc.done_pieces.contains(&0),
            "phase1: piece0 缺块不完整（serve 集排空 (0,1)(0,2)）"
        );
        let p0 = sc
            .partial_pieces
            .iter()
            .find(|p| p.piece == 0)
            .expect("phase1: partial piece0 已持久化");
        assert_eq!(p0.blocks.len(), 1);
        assert_eq!(p0.blocks[0].begin, 0);
        assert_eq!(
            p0.blocks[0].decode_data().unwrap(),
            data[..REQUEST_BLOCK_LEN as usize].to_vec(),
            "phase1: 块数据字节保真"
        );

        let log2 = Arc::new(std::sync::Mutex::new(std::collections::BTreeSet::new()));
        let (served2_tx, _served2_rx) = mpsc::unbounded_channel();
        let mut serve_all = std::collections::BTreeSet::new();
        for p in 0..num_pieces as u32 {
            for b in 0..3u32 {
                serve_all.insert((p, b * REQUEST_BLOCK_LEN));
            }
        }
        let seeder2 = fake_logging_peer(
            data.clone(),
            piece_len,
            infohash,
            serve_all,
            log2.clone(),
            served2_tx,
        )
        .await;
        let tracker2 = fake_udp_tracker(seeder2).await;
        let spec2 = crate::engine::BtSpec {
            id: 96,
            source: "/tmp/al.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: name.to_string(),
            files,
            trackers: vec![format!("udp://127.0.0.1:{tracker2}/announce")],
            concurrency: 2,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
        };
        let (evt_tx2, _evt_rx2) = mpsc::channel(256);
        let (_cmd_tx2, cmd_rx2) = mpsc::channel::<BtTaskCmd>(8);
        let handle2 = tokio::spawn(run(
            spec2,
            crate::engine::shared_for_tests(),
            cmd_rx2,
            evt_tx2,
        ));
        let mut completed = false;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        while tokio::time::Instant::now() < deadline {
            if !std::path::Path::new(&sc_path).exists() {
                completed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        assert!(completed, "phase2: 恢复后借恢复块全量完成（sidecar 移除）");
        let got = std::fs::read(format!("{save_dir}/{name}")).unwrap();
        assert_eq!(got, data, "phase2: 恢复块参与组装并完整落盘");
        assert!(
            !log2.lock().unwrap().contains(&(0, 0)),
            "phase2: 已恢复块不再重请求（反吸血关键判据：对端看不到同区间重复下载），实际请求 = {:?}",
            log2.lock().unwrap()
        );
        assert!(
            log2.lock().unwrap().contains(&(0, REQUEST_BLOCK_LEN)),
            "phase2: 缺失块正常请求"
        );
        handle2.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bt_conns_mark_full_bitfield_peer_as_seeder() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..8 * 1024 * 1024usize).map(|i| (i % 199) as u8).collect();
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
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
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
                // 逐块事件 250ms 节流后，全量 bitfield 的 seeder 标志
                // 最早也随第一个节流后的 BtConns 到达；BtDone 不提前退出，
                // 完成后的 1s tick 必发 BtConns。
                Evt::BtDone { .. } => {}
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
        let has: HashSet<u32> = HashSet::from([1, 3]);
        let done: HashSet<u32> = HashSet::new();
        let visit = refill_visit_order(&has, &done, &partial, &[1, 3]);
        assert_eq!(
            visit.first(),
            Some(&3),
            "部分到达的 piece 优先（尽管它在缓存序中靠后）"
        );
    }

    #[test]
    fn idle_conn_without_progress_evicted_after_threshold() {
        let now = std::time::Instant::now();
        let mut st = new_conn_stat("10.0.0.9:1".into());
        st.idle_check = (0, now - std::time::Duration::from_secs(61));
        assert!(
            idle_evict_due(&st, now, 60),
            "零进度 60s 的连接到置换点（同 IP 无用 peer 挡住有价值 seeder 的根因）"
        );
        st.cum_down = 4096;
        refresh_idle(&mut st, now);
        assert!(!idle_evict_due(&st, now, 60), "有字节进账的连接不置换");
        let stale_cum = st.cum_down;
        st.idle_check = (stale_cum, now - std::time::Duration::from_secs(61));
        assert!(
            idle_evict_due(&st, now, 60),
            "有历史进账但 60s 无新增的连接到置换点"
        );
        let fresh = new_conn_stat("10.0.0.9:2".into());
        assert!(!idle_evict_due(&fresh, now, 60), "新连接不置换");
    }

    #[test]
    fn inbound_admitted_blocks_same_endpoint_and_cap() {
        let mut conns: HashMap<usize, ConnStat> = HashMap::new();
        conns.insert(1, new_conn_stat("127.0.0.1:17903".into()));
        let same_ep: (IpAddr, u16) = ("127.0.0.1".parse().unwrap(), 17903);
        assert!(
            !inbound_admitted(&conns, same_ep, 28),
            "同端点已有连接时入连拒绝"
        );
        let same_ip_other_port: (IpAddr, u16) = ("127.0.0.1".parse().unwrap(), 55555);
        assert!(
            inbound_admitted(&conns, same_ip_other_port, 28),
            "同 IP 不同端口入连放行（多客户端一台机/NAT 共享出口场景）"
        );
        let other: (IpAddr, u16) = ("10.0.0.8".parse().unwrap(), 1234);
        assert!(inbound_admitted(&conns, other, 28), "不同 IP 入连放行");
        assert!(!inbound_admitted(&conns, other, 1), "容量满时入连拒绝");
        let unparseable = new_conn_stat("未知地址".into());
        conns.insert(2, unparseable);
        assert!(
            inbound_admitted(&conns, other, 28),
            "地址不可解析的既有连接不参与端点判定"
        );
    }

    #[test]
    fn picker_rarest_first_among_equally_partial() {
        let files = vec![BtFile {
            path: "a.bin".into(),
            size: 320 * 1024,
            selected: true,
        }];
        let pmap = PieceMap::new(&files, 32 * 1024);
        let mut cache = PieceOrderCache::new();
        cache.peer_gained(7);
        cache.peer_gained(9);
        cache.peer_gained(7);
        cache.peer_gained(7);
        cache.rebuild(&pmap);
        let pos = |p: u32| cache.order.iter().position(|&x| x == p).unwrap();
        assert!(
            pos(9) < pos(7),
            "稀有度低（副本少）的 piece 先选：9（1 份）在 7（3 份）前"
        );
    }

    #[test]
    fn jitter_is_deterministic_per_seed() {
        assert_eq!(jitter(7, 42), jitter(7, 42), "同 (piece, seed) 键必须恒定");
        assert_ne!(jitter(7, 1), jitter(7, 2), "不同种子应产生不同键");
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

    #[test]
    fn piece_order_cache_dirty_on_gain_and_loss() {
        let files = vec![BtFile {
            path: "a.bin".into(),
            size: 64 * 1024,
            selected: true,
        }];
        let pmap = PieceMap::new(&files, 32 * 1024);
        let mut cache = PieceOrderCache::new();
        assert!(cache.dirty, "初始即待重建");
        cache.rebuild(&pmap);
        assert!(!cache.dirty, "重建后干净");
        assert!(cache.order.is_empty(), "无人报有的 piece 不进 order");
        cache.peer_gained(1);
        assert!(cache.dirty, "Have/Bitfield 报有必须标脏");
        cache.rebuild(&pmap);
        assert_eq!(cache.order, vec![1], "重建后纳入新报有 piece");
        cache.peer_lost(1);
        assert!(cache.dirty, "连接关闭失去报有必须标脏");
        cache.rebuild(&pmap);
        assert!(cache.order.is_empty(), "失去最后报有者后退出 order");
    }

    #[test]
    fn piece_order_cache_rebuild_rarest_first() {
        let files = vec![BtFile {
            path: "a.bin".into(),
            size: 96 * 1024,
            selected: true,
        }];
        let pmap = PieceMap::new(&files, 32 * 1024);
        let mut cache = PieceOrderCache::new();
        for p in [0u32, 1, 2] {
            cache.peer_gained(p);
        }
        cache.peer_gained(1);
        cache.peer_gained(1);
        cache.rebuild(&pmap);
        let pos = |p: u32| cache.order.iter().position(|&x| x == p).unwrap();
        assert_eq!(cache.order.len(), 3, "全部报有 piece 在 order 中");
        assert!(
            pos(0) < pos(1) && pos(2) < pos(1),
            "副本最少的 piece 先于 3 副本的 piece"
        );
    }

    #[test]
    fn piece_order_cache_rebuild_is_permutation_of_covered() {
        let files = vec![BtFile {
            path: "a.bin".into(),
            size: 256 * 1024,
            selected: true,
        }];
        let pmap = PieceMap::new(&files, 32 * 1024);
        let mut cache = PieceOrderCache::new();
        for p in pmap.covered_pieces().iter().copied() {
            cache.peer_gained(p);
        }
        cache.rebuild(&pmap);
        let mut as_multiset = cache.order.clone();
        as_multiset.sort_unstable();
        assert_eq!(
            as_multiset,
            pmap.covered_pieces().to_vec(),
            "重建结果是 covered pieces 的排列（比较器 total order，大输入不 panic）"
        );
    }

    #[test]
    fn refill_visit_order_partial_first_then_cache_order() {
        let partial: PartialPieces = HashMap::from([
            (
                3u32,
                BTreeMap::from([(0u32, vec![0u8; 16]), (16384u32, vec![0u8; 16])]),
            ),
            (5u32, BTreeMap::from([(0u32, vec![0u8; 16])])),
        ]);
        let done: HashSet<u32> = HashSet::from([9]);
        let has: HashSet<u32> = HashSet::from([3, 5, 7, 9]);
        let cache_order = vec![9u32, 7, 5, 3];
        let visit = refill_visit_order(&has, &done, &partial, &cache_order);
        assert_eq!(
            visit,
            vec![3, 5, 7],
            "partial 优先（3 已收 2 块 > 5 的 1 块），缓存序跳过 partial/done"
        );
    }

    #[test]
    fn ema_target_floor_cap_and_rate_scaling() {
        assert_eq!(
            ema_target(0, 512),
            2,
            "零速率取下限 2（libtorrent min_request_queue）"
        );
        assert_eq!(ema_target(16_384, 512), 3, "3s × 16KiB/s ÷ 16KiB = 3");
        assert_eq!(
            ema_target(2_621_440, 512),
            480,
            "3s × 2.5MiB/s ÷ 16KiB = 480"
        );
        assert_eq!(ema_target(52_428_800, 512), 512, "高速率钳上限");
        assert_eq!(ema_target(2_621_440, 64), 64, "上限钳制生效");
        assert_eq!(ema_target(2_621_440, 1), 1, "极小上限不被抬升且不 panic");
    }

    #[test]
    fn tick_pipeline_slow_start_grows_and_exits_at_plateau() {
        let mut st = new_conn_stat("10.0.0.1:51413".into());
        let mut t = std::time::Instant::now();
        tick_pipeline(&mut st, t, 512);
        assert_eq!(
            st.pipe_target, 4,
            "首个 tick 无进账不退 slow-start（初始深度 = libtorrent 成员初始化 4）"
        );
        assert!(st.pipe_slow_start);
        t += Duration::from_secs(1);
        st.cum_down = 300_000;
        tick_pipeline(&mut st, t, 512);
        assert!(
            st.pipe_slow_start,
            "首秒有进账但无上一秒样本不退 slow-start"
        );
        t += Duration::from_secs(1);
        st.cum_down = 700_000;
        tick_pipeline(&mut st, t, 512);
        assert!(
            st.pipe_slow_start,
            "400KB > 300KB 仍在增长，slow-start 继续"
        );
        st.peer_choking = true;
        t += Duration::from_secs(1);
        st.cum_down = 703_000;
        tick_pipeline(&mut st, t, 512);
        assert!(
            st.pipe_slow_start,
            "choke 期间停滞不触发退出口径（对齐 libtorrent !m_peer_choked）"
        );
        st.peer_choking = false;
        t += Duration::from_secs(1);
        st.cum_down = 704_000;
        tick_pipeline(&mut st, t, 512);
        assert!(
            !st.pipe_slow_start,
            "未 choke 且增长停滞（1KB ≤ 3KB+5KB）退出 slow-start"
        );
        let ema_after = st.pipe_ema;
        t += Duration::from_secs(1);
        st.cum_down += 2_621_440;
        tick_pipeline(&mut st, t, 512);
        assert_eq!(
            st.pipe_target,
            ema_target(ema_after * 4 / 5 + 2_621_440 / 5, 512),
            "退出后目标 = 3s × EMA ÷ 16KiB（整数均值口径同 libtorrent stat_channel）"
        );
    }

    #[test]
    fn tick_pipeline_ema_stable_under_ui_snapshot_resets() {
        let mut st = new_conn_stat("10.0.0.1:51413".into());
        st.pipe_slow_start = false;
        st.pipe_ema = 2_621_440;
        let mut t = std::time::Instant::now();
        t += Duration::from_secs(1);
        st.cum_down += 2_621_440;
        tick_pipeline(&mut st, t, 512);
        st.down_window = (st.cum_down, t);
        assert_eq!(
            st.pipe_target, 480,
            "稳态 2.5MiB/s → 3s 队列深度 480；UI 快照重置 down_window 不影响管线口径"
        );
        t += Duration::from_secs(1);
        tick_pipeline(&mut st, t, 512);
        st.down_window = (st.cum_down, t);
        assert_eq!(
            st.pipe_target, 384,
            "单秒断流只衰减 1/5（480→384），不再随 UI 窗口重置塌向下限"
        );
    }

    fn rate_conn(has: HashSet<u32>, pipe_target: usize) -> ConnStat {
        let mut st = new_conn_stat("10.0.0.1:51413".into());
        st.has = has;
        st.peer_choking = false;
        st.pipe_slow_start = false;
        st.pipe_target = pipe_target;
        st
    }

    async fn drain_issue_requests(
        st: &mut ConnStat,
        pmap: &PieceMap,
        tun: &BtTunables,
        done: &HashSet<u32>,
        partial: &mut PartialPieces,
        order: &mut PieceOrderCache,
        rounds: usize,
    ) {
        for _ in 0..rounds {
            let claimed: HashSet<(u32, u32)> = st.inflight.keys().copied().collect();
            issue_requests(1, st, &claimed, pmap, tun, done, partial, order).await;
        }
    }

    #[tokio::test]
    async fn issue_requests_scales_queue_with_rate() {
        let piece_len = 262144usize;
        let data = vec![0x5Au8; 32 * piece_len];
        let _meta = make_meta(&data, piece_len, "http://127.0.0.1:1/announce");
        let files = vec![BtFile {
            path: "e2e.bin".into(),
            size: data.len() as u64,
            selected: true,
        }];
        let pmap = PieceMap::new(&files, piece_len as u32);
        let n = pmap.num_pieces();
        let has: HashSet<u32> = (0..n).collect();
        let tun = BtTunables {
            peer_pipeline: 512,
            ..BtTunables::default()
        };
        let done: HashSet<u32> = HashSet::new();
        let mut partial: PartialPieces = HashMap::new();
        let mut order = PieceOrderCache::new();

        let mut st = rate_conn(has.clone(), 480);
        for p in 0..n {
            order.peer_gained(p);
        }
        drain_issue_requests(&mut st, &pmap, &tun, &done, &mut partial, &mut order, 40).await;
        assert_eq!(
            st.inflight.len(),
            480,
            "目标深度 480（3s × 2.5MiB/s ÷ 16KiB）时在飞恰好钳在目标"
        );
        let asked_lens: HashSet<u32> = st.inflight.values().map(|(len, _)| *len).collect();
        assert!(asked_lens.iter().all(|&l| l == 16384), "整块 16KiB 请求");

        let mut st_capped = rate_conn(has, 64);
        let tun_capped = BtTunables {
            peer_pipeline: 64,
            ..BtTunables::default()
        };
        for p in 0..n {
            order.peer_gained(p);
        }
        drain_issue_requests(
            &mut st_capped,
            &pmap,
            &tun_capped,
            &done,
            &mut partial,
            &mut order,
            40,
        )
        .await;
        assert_eq!(st_capped.inflight.len(), 64, "config 上限仍然生效");
    }

    #[tokio::test]
    async fn issue_requests_fresh_connection_ramps_per_block_in_slow_start() {
        let piece_len = 262144usize;
        let data = vec![0x5Au8; 8 * piece_len];
        let _meta = make_meta(&data, piece_len, "http://127.0.0.1:1/announce");
        let files = vec![BtFile {
            path: "e2e.bin".into(),
            size: data.len() as u64,
            selected: true,
        }];
        let pmap = PieceMap::new(&files, piece_len as u32);
        let n = pmap.num_pieces();
        let has: HashSet<u32> = (0..n).collect();
        let tun = BtTunables::default();
        let done: HashSet<u32> = HashSet::new();
        let mut partial: PartialPieces = HashMap::new();
        let mut order = PieceOrderCache::new();
        let mut st = new_conn_stat("10.0.0.1:51413".into());
        st.has = has;
        st.peer_choking = false;
        for p in 0..n {
            order.peer_gained(p);
        }
        drain_issue_requests(&mut st, &pmap, &tun, &done, &mut partial, &mut order, 40).await;
        assert_eq!(
            st.inflight.len(),
            4,
            "新连接 slow-start 起步深度 = 4（libtorrent m_desired_queue_size 成员初始化）"
        );
        for _ in 0..16 {
            st.pipe_target = st.pipe_target.saturating_add(1);
        }
        drain_issue_requests(&mut st, &pmap, &tun, &done, &mut partial, &mut order, 40).await;
        assert_eq!(
            st.inflight.len(),
            20,
            "slow-start 每收一块 +1，在飞随目标线性爬坡"
        );
    }

    fn shared_for_fill_test() -> EngineShared {
        EngineShared::for_test()
    }

    fn spec_for_fill_test(concurrency: usize) -> crate::engine::BtSpec {
        let meta = make_meta(&[0u8; 262144], 262144, "http://127.0.0.1:1/announce");
        crate::engine::BtSpec {
            id: 1,
            source: "test".into(),
            meta,
            save_dir: "/tmp/ezr-fill-test".into(),
            name: "e2e.bin".into(),
            files: vec![BtFile {
                path: "e2e.bin".into(),
                size: 262144,
                selected: true,
            }],
            trackers: vec![],
            concurrency,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: BtTunables::default(),
        }
    }

    #[test]
    fn file_abs_path_single_layer_for_multi_file() {
        // bug28①：rel 首段已含 torrent name（metainfo 解析保证：单文件 = name，
        // 多文件 = name/…），落盘路径 = save_dir/rel，不得再叠加任务名（旧实现
        // 多段 rel 拼成 save_dir/name/name/… 双层）。
        assert_eq!(file_abs_path("/a", "b"), "/a/b");
        assert_eq!(file_abs_path("/a", "b/sub/f.bin"), "/a/b/sub/f.bin");
        assert_eq!(file_abs_path("/a", "b/sub/dir/f.bin"), "/a/b/sub/dir/f.bin");
    }

    /// 本地 ws announce 计数服务器：每收到一条 announce 回一条响应
    /// （interval/min interval 可配），返回 ws:// 地址供会话连接。
    async fn ws_announce_counter_server(
        count: Arc<std::sync::atomic::AtomicUsize>,
        interval: u32,
        min_interval: u32,
    ) -> String {
        use futures::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let Ok((sock, _)) = listener.accept().await else {
                return;
            };
            let Ok(mut ws) = tokio_tungstenite::accept_async(sock).await else {
                return;
            };
            while let Some(Ok(msg)) = ws.next().await {
                let Message::Text(t) = msg else {
                    continue;
                };
                let Ok(req) = serde_json::from_str::<serde_json::Value>(&t) else {
                    continue;
                };
                if req.get("action").and_then(|v| v.as_str()) != Some("announce") {
                    continue;
                }
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let resp = serde_json::json!({
                    "action": "announce",
                    "info_hash": req["info_hash"],
                    "interval": interval,
                    "min interval": min_interval,
                    "complete": 1,
                    "incomplete": 0,
                    "peers": []
                });
                if ws.send(Message::Text(resp.to_string())).await.is_err() {
                    break;
                }
            }
        });
        format!("ws://{addr}/announce")
    }

    fn ws_test_ctx(reannounce: Arc<tokio::sync::Notify>) -> AnnounceCtx {
        AnnounceCtx {
            infohash: [7u8; 20],
            peer_id: [8u8; 20],
            port: 6881,
            client: reqwest::Client::new(),
            numwant: 50,
            live: Arc::new(std::sync::Mutex::new(LiveStats::default())),
            completed_notify: Arc::new(tokio::sync::Notify::new()),
            reannounce_notify: reannounce,
            backoff_start_secs: 8,
            backoff_cap_secs: 60,
            udp_base_timeout_secs: 2,
            udp_retries: 2,
            signals: Arc::new(std::sync::Mutex::new(SignalState::default())),
            rtc_tx: mpsc::channel(4).0,
            transports: PeerTransports {
                tcp: true,
                webrtc: false,
            },
            rtc_retry_base_secs: 5,
            rtc_retry_cap_secs: 60,
        }
    }

    async fn wait_announce_count(count: &Arc<std::sync::atomic::AtomicUsize>, n: usize) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while count.load(std::sync::atomic::Ordering::SeqCst) < n {
            assert!(
                tokio::time::Instant::now() < deadline,
                "announce 计数未达 {n}"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    #[tokio::test]
    async fn ws_tracker_session_reannounces_immediately_on_grow_notify() {
        // bug28③：扩容触发 reannounce_notify → ws 会话不等 interval（120s），
        // 立即重发 announce（携带新 offers 重连 shed 的 peer）。
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let url = ws_announce_counter_server(count.clone(), 120, 0).await;
        let reannounce = Arc::new(tokio::sync::Notify::new());
        let sess = {
            let reannounce = Arc::clone(&reannounce);
            tokio::spawn(async move {
                let ctx = ws_test_ctx(reannounce);
                let (tx, _rx) = mpsc::channel::<TrackerReport>(32);
                let _ = ws_tracker_session(&url, &ctx, &tx).await;
            })
        };
        wait_announce_count(&count, 1).await;
        let t1 = tokio::time::Instant::now();
        reannounce.notify_waiters();
        wait_announce_count(&count, 2).await;
        assert!(
            t1.elapsed() < Duration::from_secs(4),
            "notify 后应立即 re-announce，实际 {:?}",
            t1.elapsed()
        );
        sess.abort();
    }

    #[tokio::test]
    async fn ws_tracker_session_reannounce_respects_min_interval() {
        // min_interval=60：即使收到 notify，也不得早于 min_interval 提前 announce。
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let url = ws_announce_counter_server(count.clone(), 120, 60).await;
        let reannounce = Arc::new(tokio::sync::Notify::new());
        let sess = {
            let reannounce = Arc::clone(&reannounce);
            tokio::spawn(async move {
                let ctx = ws_test_ctx(reannounce);
                let (tx, _rx) = mpsc::channel::<TrackerReport>(32);
                let _ = ws_tracker_session(&url, &ctx, &tx).await;
            })
        };
        wait_announce_count(&count, 1).await;
        reannounce.notify_waiters();
        tokio::time::sleep(Duration::from_millis(2500)).await;
        assert_eq!(
            count.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "min_interval 未到不得提前 announce"
        );
        sess.abort();
    }

    #[tokio::test]
    async fn fill_peer_slots_redials_immediately_when_no_connections() {
        let spec = spec_for_fill_test(4);
        let shared = shared_for_fill_test();
        let (tx, mut rx) = mpsc::channel(8);
        let peer_id = [0u8; 20];
        let dl_flow = std::sync::Arc::new(DlFlow {
            global: std::sync::Arc::new(Throttle::new(0)),
            task: std::sync::Arc::new(Throttle::new(0)),
        });
        let mut conns: HashMap<usize, ConnStat> = HashMap::new();
        let mut candidates: HashMap<(IpAddr, u16), PeerCandidate> = HashMap::new();
        candidates.insert(
            ("10.0.0.9".parse().unwrap(), 51413),
            PeerCandidate {
                fails: 5,
                last_try: Some(std::time::Instant::now() - Duration::from_millis(1100)),
            },
        );
        let mut next_conn = 0usize;
        fill_peer_slots(
            &spec,
            &peer_id,
            &tx,
            &shared,
            &mut conns,
            &mut candidates,
            &mut next_conn,
            &dl_flow,
        );
        assert_eq!(
            conns.len(),
            1,
            "零连接紧急重拨：深退避候选在 1s 后立即拨出（非紧急口径需 120s）"
        );
        let _ = rx.try_recv();
    }

    #[tokio::test]
    async fn fill_peer_slots_respects_cooldown_when_connections_remain() {
        let spec = spec_for_fill_test(4);
        let shared = shared_for_fill_test();
        let (tx, mut rx) = mpsc::channel(8);
        let peer_id = [0u8; 20];
        let dl_flow = std::sync::Arc::new(DlFlow {
            global: std::sync::Arc::new(Throttle::new(0)),
            task: std::sync::Arc::new(Throttle::new(0)),
        });
        let mut conns: HashMap<usize, ConnStat> = HashMap::new();
        conns.insert(7, new_conn_stat("10.0.0.1:51413".into()));
        let mut candidates: HashMap<(IpAddr, u16), PeerCandidate> = HashMap::new();
        candidates.insert(
            ("10.0.0.9".parse().unwrap(), 51413),
            PeerCandidate {
                fails: 5,
                last_try: Some(std::time::Instant::now()),
            },
        );
        let mut next_conn = 0usize;
        fill_peer_slots(
            &spec,
            &peer_id,
            &tx,
            &shared,
            &mut conns,
            &mut candidates,
            &mut next_conn,
            &dl_flow,
        );
        assert_eq!(
            conns.len(),
            1,
            "仍有连接时保持退避节奏，不因紧急口径提前重拨"
        );
        let _ = rx.try_recv();
    }

    #[tokio::test]
    async fn fill_peer_slots_dials_same_ip_different_endpoint() {
        let spec = spec_for_fill_test(4);
        let shared = shared_for_fill_test();
        let (tx, mut rx) = mpsc::channel(8);
        let peer_id = [0u8; 20];
        let dl_flow = std::sync::Arc::new(DlFlow {
            global: std::sync::Arc::new(Throttle::new(0)),
            task: std::sync::Arc::new(Throttle::new(0)),
        });
        let mut conns: HashMap<usize, ConnStat> = HashMap::new();
        conns.insert(7, new_conn_stat("127.0.0.1:17920".into()));
        let mut candidates: HashMap<(IpAddr, u16), PeerCandidate> = HashMap::new();
        candidates.insert(
            ("127.0.0.1".parse().unwrap(), 17903),
            PeerCandidate {
                fails: 0,
                last_try: None,
            },
        );
        let mut next_conn = 0usize;
        fill_peer_slots(
            &spec,
            &peer_id,
            &tx,
            &shared,
            &mut conns,
            &mut candidates,
            &mut next_conn,
            &dl_flow,
        );
        assert_eq!(
            conns.len(),
            2,
            "同 IP 不同端口的 seeder 候选不被已有连接阻断（反 60s 死区）"
        );
        let _ = rx.try_recv();
    }

    #[tokio::test]
    async fn fill_peer_slots_skips_connected_endpoint() {
        let spec = spec_for_fill_test(4);
        let shared = shared_for_fill_test();
        let (tx, mut rx) = mpsc::channel(8);
        let peer_id = [0u8; 20];
        let dl_flow = std::sync::Arc::new(DlFlow {
            global: std::sync::Arc::new(Throttle::new(0)),
            task: std::sync::Arc::new(Throttle::new(0)),
        });
        let mut conns: HashMap<usize, ConnStat> = HashMap::new();
        conns.insert(7, new_conn_stat("127.0.0.1:17903".into()));
        let mut candidates: HashMap<(IpAddr, u16), PeerCandidate> = HashMap::new();
        candidates.insert(
            ("127.0.0.1".parse().unwrap(), 17903),
            PeerCandidate {
                fails: 0,
                last_try: None,
            },
        );
        let mut next_conn = 0usize;
        fill_peer_slots(
            &spec,
            &peer_id,
            &tx,
            &shared,
            &mut conns,
            &mut candidates,
            &mut next_conn,
            &dl_flow,
        );
        assert_eq!(
            conns.len(),
            1,
            "同端点已有连接时不重复拨出（同一客户端单连接）"
        );
        let _ = rx.try_recv();
    }

    #[test]
    fn progress_emit_due_throttles_bursts() {
        let t0 = std::time::Instant::now();
        assert!(!progress_emit_due(t0, t0, 250), "刚发过不再发");
        assert!(
            !progress_emit_due(t0, t0 + Duration::from_millis(249), 250),
            "249ms 内节流"
        );
        assert!(
            progress_emit_due(t0, t0 + Duration::from_millis(250), 250),
            "250ms 到点放行"
        );
    }

    #[test]
    fn bt_tunables_default_from_config() {
        let t = BtTunables::default();
        let c = crate::model::config::Config::default();
        assert_eq!(
            t.peer_pipeline, c.peer_pipeline,
            "管线深度缺省 = [bt] peer_pipeline 缺省"
        );
        assert_eq!(
            t.progress_emit_min_interval_ms, c.progress_emit_min_interval_ms,
            "逐块事件节流缺省 250ms（qBittorrent UI 同为秒级刷新）"
        );
        assert_eq!(t.upload_slots, c.upload_slots);
        assert_eq!(t.idle_eviction_secs, c.idle_eviction_secs);
        assert_eq!(t.tracker_numwant, c.tracker_numwant);
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
        let views = conn_views(&mut conns, pmap.covered_pieces(), &HashSet::new(), None);
        assert_eq!(
            views.len(),
            1,
            "握手未完成的连接不得出现在并发 panel（同 IP 两行根因）"
        );
        assert_eq!(views[0].addr, "10.0.0.7:51413", "保留的是已建立连接");
    }

    fn flags_case(
        conn_id: usize,
        covered: &[u32],
        done: &HashSet<u32>,
        peer_choking: bool,
        am_choking: bool,
        peer_interested: bool,
        has: &[u32],
        inbound: bool,
        optimistic: Option<usize>,
    ) -> String {
        let mut st = new_conn_stat(format!("10.0.0.{conn_id}:6881"));
        st.peer_id = Some([conn_id as u8; 20]);
        st.peer_choking = peer_choking;
        st.am_choking = am_choking;
        st.peer_interested = peer_interested;
        st.inbound = inbound;
        for &p in has {
            st.has.insert(p);
        }
        conn_flags(&st, covered, done, optimistic, conn_id)
    }

    #[test]
    fn conn_flags_decision_table_covers_all_eight_flags() {
        let covered = [0u32, 1, 2];
        let mut done = HashSet::new();
        done.insert(0u32);
        done.insert(1u32);
        done.insert(2u32);
        let dl_done: HashSet<u32> = done.clone();

        assert_eq!(
            flags_case(
                1,
                &covered,
                &HashSet::new(),
                false,
                true,
                false,
                &[0, 1],
                false,
                None
            ),
            "D",
            "D = 未 choke ∧ 我方有未完成需求"
        );
        assert_eq!(
            flags_case(
                1,
                &covered,
                &HashSet::new(),
                true,
                true,
                false,
                &[0, 1],
                false,
                None
            ),
            "d",
            "d = choke ∧ 我方有未完成需求"
        );
        assert_eq!(
            flags_case(1, &covered, &dl_done, true, false, true, &[], false, None),
            "U",
            "U = 对方 interested ∧ 我方未 choke（对方 choke 且我方无需求，避免 K/? 并入）"
        );
        assert_eq!(
            flags_case(1, &covered, &dl_done, true, true, true, &[], false, None),
            "u",
            "u = 对方 interested ∧ 我方 choke"
        );
        assert_eq!(
            flags_case(
                1,
                &covered,
                &dl_done,
                true,
                false,
                true,
                &[],
                false,
                Some(1)
            ),
            "UO",
            "O = 乐观槽命中（池 = interested 对端，必伴随 U；qBt 图例序 U 在 O 前）"
        );
        assert_eq!(
            flags_case(1, &covered, &dl_done, true, true, false, &[], true, None),
            "I",
            "I = 入连方向"
        );
        assert_eq!(
            flags_case(1, &covered, &dl_done, false, true, false, &[], false, None),
            "K",
            "K = 未 choke ∧ 我方无未完成需求"
        );
        assert_eq!(
            flags_case(1, &covered, &dl_done, true, false, false, &[], false, None),
            "?",
            "? = 我方未 choke ∧ 对方无 interest"
        );
    }

    #[test]
    fn conn_flags_multi_flag_ordering_and_mutual_exclusions() {
        let covered = [0u32, 1, 2];
        let all_done: HashSet<u32> = [0u32, 1, 2].into_iter().collect();

        assert_eq!(
            flags_case(
                1,
                &covered,
                &HashSet::new(),
                false,
                false,
                true,
                &[0, 1],
                false,
                None
            ),
            "DU",
            "下载互惠连接 = DU（qBt 图例序）"
        );
        assert_eq!(
            flags_case(
                1,
                &covered,
                &HashSet::new(),
                false,
                false,
                true,
                &[0, 1],
                false,
                Some(1)
            ),
            "DUO",
            "乐观槽互惠连接 = DUO（O 尾接）"
        );
        assert_eq!(
            flags_case(
                1,
                &covered,
                &HashSet::new(),
                true,
                false,
                true,
                &[],
                true,
                None
            ),
            "UI",
            "做种入连上传中 = UI"
        );
        assert_eq!(
            flags_case(
                1,
                &covered,
                &HashSet::new(),
                false,
                false,
                true,
                &[0, 1],
                true,
                Some(1)
            ),
            "DUOI",
            "极值组合 = DUOI 四枚（D+U+O+I）"
        );
        assert_eq!(
            flags_case(1, &covered, &all_done, false, false, false, &[], true, None),
            "IK?",
            "做种入连互不 choke 且双方无兴趣 = IK?（qBt 图例序 I 在 K/? 前）"
        );
        assert_eq!(
            flags_case(
                1,
                &covered,
                &HashSet::new(),
                true,
                true,
                true,
                &[0, 1],
                true,
                None
            ),
            "duI",
            "被 choke 候选源入连 = duI（小写互斥对）"
        );
    }

    #[test]
    fn conn_flags_empty_renders_dash_and_independent_flags_coexist() {
        let covered = [0u32, 1, 2];
        let mut done = HashSet::new();
        done.insert(0u32);
        done.insert(1u32);
        done.insert(2u32);
        let all_done: HashSet<u32> = done;

        assert_eq!(
            flags_case(
                1,
                &covered,
                &HashSet::new(),
                true,
                true,
                false,
                &[],
                false,
                None
            ),
            "-",
            "刚建立双方均 choke 且无 interest 的新连接 = -（空值占位）"
        );
        assert_eq!(
            flags_case(
                1,
                &covered,
                &all_done,
                false,
                false,
                false,
                &[],
                false,
                None
            ),
            "K?",
            "K 与 ? 条件独立可并存（对方未 choke 我方且我方无需求 + 我方未 choke 且对方无兴趣）"
        );
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
        let slot = apply_choker(&mut conns, false, false, 4, None, &mut cursor).await;
        let unchoked: Vec<usize> = conns
            .iter()
            .filter(|(_, st)| !st.am_choking)
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(unchoked.len(), 4 + 1, "4 固定槽 + 1 乐观槽");
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
        apply_choker(&mut conns, false, false, 4, None, &mut cursor).await;
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
        let slot = apply_choker(&mut conns, true, false, 4, None, &mut cursor).await;
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
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
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
            &BtTunables::default(),
            Arc::new(tokio::sync::Semaphore::new(8)),
            &test_ul(),
            &test_ul(),
            &mut done,
            &mut downloaded,
            &mut handles,
            &mut partial,
            &mut PieceOrderCache::new(),
            false,
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
            &BtTunables::default(),
            Arc::new(tokio::sync::Semaphore::new(8)),
            &test_ul(),
            &test_ul(),
            &mut done,
            &mut downloaded,
            &mut handles,
            &mut partial,
            &mut PieceOrderCache::new(),
            false,
        )
        .await;
        let served =
            tokio::time::timeout(std::time::Duration::from_millis(2000), rx_a.recv()).await;
        match served {
            Ok(Some(PeerOut::Send(peer::PeerMsg::Block {
                index,
                begin,
                data: blk,
            }))) => {
                assert_eq!((index, begin), (0, 0));
                assert_eq!(blk, &data[..4096]);
            }
            other => panic!("unchoke 后 request 应被服务（spawn 任务送出 Block）：{other:?}"),
        }
        assert_eq!(
            conns[&1].cum_up, 4096,
            "serve 决策时计入 cum_up（读盘失败才可能多计）"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn request_serve_does_not_block_engine_loop_when_out_channel_full() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..16_384usize).map(|i| (i % 211) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-serve-nonblock");
        let save_dir = dir.to_string_lossy().into_owned();
        let files = vec![BtFile {
            path: "g.bin".into(),
            size: data.len() as u64,
            selected: true,
        }];
        std::fs::write(format!("{save_dir}/g.bin"), &data).unwrap();
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
            id: 98,
            source: "/tmp/g.torrent".into(),
            meta,
            save_dir: save_dir.clone(),
            name: "g.bin".into(),
            files,
            trackers: vec![],
            concurrency: 4,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
        };
        let pmap = PieceMap::new(&spec.files, spec.meta.piece_length);
        let mut done: HashSet<u32> = HashSet::new();
        done.insert(0);
        let mut downloaded = data.len() as u64;
        let mut partial: PartialPieces = HashMap::new();

        let mut st = new_conn_stat("10.0.0.9:999".into());
        st.am_choking = false;
        let (tx_a, rx_a) = mpsc::channel(1);
        tx_a.send(PeerOut::Drop).await.unwrap();
        st.peer_out = tx_a;
        let mut conns = HashMap::from([(1usize, st)]);
        let _keep_rx = rx_a;
        let mut handles: HashMap<usize, tokio::fs::File> = HashMap::new();
        let fh = tokio::fs::OpenOptions::new()
            .read(true)
            .open(format!("{save_dir}/g.bin"))
            .await
            .unwrap();
        handles.insert(0, fh);

        let serve = tokio::time::timeout(
            std::time::Duration::from_millis(2000),
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
                &BtTunables::default(),
                Arc::new(tokio::sync::Semaphore::new(8)),
                &test_ul(),
                &test_ul(),
                &mut done,
                &mut downloaded,
                &mut handles,
                &mut partial,
                &mut PieceOrderCache::new(),
                false,
            ),
        )
        .await;
        assert!(
            serve.is_ok(),
            "out 通道满且无人排水时 request 服务不得阻塞引擎循环（上传读盘+发送必须移出引擎任务）"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn request_serve_keeps_engine_handle_cursor_untouched() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..16_384usize).map(|i| (i % 211) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-serve-cursor");
        let save_dir = dir.to_string_lossy().into_owned();
        let files = vec![BtFile {
            path: "g.bin".into(),
            size: data.len() as u64,
            selected: true,
        }];
        std::fs::write(format!("{save_dir}/g.bin"), &data).unwrap();
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
            id: 99,
            source: "/tmp/g.torrent".into(),
            meta,
            save_dir: save_dir.clone(),
            name: "g.bin".into(),
            files,
            trackers: vec![],
            concurrency: 4,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
        };
        let pmap = PieceMap::new(&spec.files, spec.meta.piece_length);
        let mut done: HashSet<u32> = HashSet::new();
        done.insert(0);
        let mut downloaded = data.len() as u64;
        let mut partial: PartialPieces = HashMap::new();

        let mut st = new_conn_stat("10.0.0.9:999".into());
        st.am_choking = false;
        let (tx_a, mut rx_a) = mpsc::channel(8);
        st.peer_out = tx_a;
        let mut conns = HashMap::from([(1usize, st)]);
        let mut handles: HashMap<usize, tokio::fs::File> = HashMap::new();
        let fh = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("{save_dir}/g.bin"))
            .await
            .unwrap();
        handles.insert(0, fh);
        use tokio::io::AsyncSeekExt;
        let cursor_before = handles
            .get_mut(&0)
            .unwrap()
            .stream_position()
            .await
            .unwrap();

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
            &BtTunables::default(),
            Arc::new(tokio::sync::Semaphore::new(8)),
            &test_ul(),
            &test_ul(),
            &mut done,
            &mut downloaded,
            &mut handles,
            &mut partial,
            &mut PieceOrderCache::new(),
            false,
        )
        .await;
        let drained = tokio::time::timeout(std::time::Duration::from_millis(2000), rx_a.recv())
            .await
            .expect("上传任务应完成");
        assert!(matches!(
            drained,
            Some(PeerOut::Send(peer::PeerMsg::Block { .. }))
        ));
        let cursor_after = handles
            .get_mut(&0)
            .unwrap()
            .stream_position()
            .await
            .unwrap();
        assert_eq!(
            cursor_before, cursor_after,
            "上传读盘不得挪动引擎句柄的共享偏移（须独立 open file description）"
        );
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
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
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
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
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
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
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

    async fn wait_bt_conns_established(evt_rx: &mut mpsc::Receiver<Evt>, n: usize) {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        loop {
            assert!(
                std::time::Instant::now() < deadline,
                "等待 {n} 条已握手连接超时"
            );
            let Some(evt) = tokio::time::timeout(Duration::from_millis(500), evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                continue;
            };
            if let Evt::BtConns { conns, .. } = evt {
                if conns.len() >= n {
                    break;
                }
            }
        }
    }

    fn reconf_spec_conns(inflight: &[(u32, u32)], cum_down: u64) -> ConnStat {
        let (tx, _rx) = mpsc::channel::<PeerOut>(8);
        let mut st = new_conn_stat("127.0.0.9:1000".into());
        st.peer_out = tx;
        st.cum_down = cum_down;
        for (p, b) in inflight {
            st.inflight
                .insert((*p, *b), (16 * 1024, std::time::Instant::now()));
        }
        st
    }

    #[test]
    fn shed_excess_conns_prefers_uninflight_then_least_downloaded() {
        let mut conns = HashMap::new();
        conns.insert(1usize, reconf_spec_conns(&[(0, 0), (0, 16384)], 1000));
        conns.insert(2usize, reconf_spec_conns(&[], 5));
        conns.insert(3usize, reconf_spec_conns(&[(1, 0)], 50));
        let shed = shed_excess_conns(&conns, 1);
        assert_eq!(shed, vec![2usize, 3usize], "先收无在飞、再收累计下载最少");
        let shed = shed_excess_conns(&conns, 2);
        assert_eq!(shed, vec![2usize], "目标 2 只收一条");
        let shed = shed_excess_conns(&conns, 3);
        assert!(shed.is_empty(), "未超额不收");
    }

    #[test]
    fn cancel_inflight_not_covered_keeps_wanted_requests() {
        let files = vec![
            BtFile {
                path: "a.bin".into(),
                size: 200 * 1024,
                selected: true,
            },
            BtFile {
                path: "b.bin".into(),
                size: 200 * 1024,
                selected: false,
            },
        ];
        let pmap = PieceMap::new(&files, 128 * 1024);
        assert!(pmap.covers(0) && pmap.covers(1) && !pmap.covers(2) && !pmap.covers(3));
        let (tx, _rx) = mpsc::channel::<PeerOut>(8);
        let mut st = new_conn_stat("127.0.0.9:1000".into());
        st.peer_out = tx;
        for (p, b) in [(0u32, 0u32), (2u32, 0u32), (3u32, 16384u32)] {
            st.inflight
                .insert((p, b), (16 * 1024, std::time::Instant::now()));
        }
        let mut cancelled = Vec::new();
        cancel_inflight_not_covered(&mut st, &pmap, &mut cancelled);
        cancelled.sort_unstable();
        assert_eq!(
            cancelled,
            vec![(2u32, 0u32), (3u32, 16384u32)],
            "撤销集合 = 非覆盖 piece 的在飞请求（顺序不敏感）"
        );
        assert!(st.inflight.contains_key(&(0, 0)), "覆盖内请求保留");
    }

    #[test]
    fn recompute_downloaded_follows_new_selection() {
        let all = vec![
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
        let full = PieceMap::new(&all, 64);
        let done: HashSet<u32> = [0u32, 1, 2, 3].into();
        assert_eq!(
            recompute_downloaded(&done, &full),
            200,
            "全选：四块全完成 = 200"
        );
        let shrunk_files = vec![
            BtFile {
                path: "a.bin".into(),
                size: 100,
                selected: true,
            },
            BtFile {
                path: "b.bin".into(),
                size: 100,
                selected: false,
            },
        ];
        let shrunk = PieceMap::new(&shrunk_files, 64);
        assert_eq!(
            recompute_downloaded(&done, &shrunk),
            100,
            "取消勾选 b.bin：仅计入 a.bin 覆盖的字节（64+36，重算口径与完成计数同源）"
        );
    }

    async fn fake_udp_tracker_multi(peers: Vec<(std::net::IpAddr, u16)>) -> u16 {
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
                    let mut out = Vec::new();
                    out.extend_from_slice(&0u32.to_be_bytes());
                    out.extend_from_slice(&txid.to_be_bytes());
                    out.extend_from_slice(&0x1234_5678_9abc_def0u64.to_be_bytes());
                    let _ = sock.send_to(&out, from).await;
                } else if action == 1 {
                    let mut out = Vec::new();
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&txid.to_be_bytes());
                    out.extend_from_slice(&(peers.len() as u32).to_be_bytes());
                    out.extend_from_slice(&1u32.to_be_bytes());
                    out.extend_from_slice(&(peers.len() as u32).to_be_bytes());
                    for (ip, port) in &peers {
                        match ip {
                            std::net::IpAddr::V4(v4) => out.extend_from_slice(&v4.octets()),
                            std::net::IpAddr::V6(v6) => out.extend_from_slice(&v6.octets()),
                        }
                        out.extend_from_slice(&port.to_be_bytes());
                    }
                    let _ = sock.send_to(&out, from).await;
                }
            }
        });
        port
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reconfigure_grows_connection_count_without_restart() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..80_000usize).map(|i| (i % 251) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-reconf-grow");
        let save_dir = dir.to_string_lossy().into_owned();
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let seeders = [
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 2)),
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 3)),
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 4)),
        ];
        let meta0 = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let mut peer_list = Vec::new();
        for (i, ip) in seeders.iter().enumerate() {
            let mut pid = *b"-QX7777-seederpeer00";
            pid[19] = b'0' + i as u8;
            let port = counting_seeder_any(
                data.clone(),
                piece_len,
                meta0.infohash,
                counter.clone(),
                &pid,
            )
            .await;
            peer_list.push((*ip, port));
        }
        let tracker_port = fake_udp_tracker_multi(peer_list).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );

        let shared = crate::engine::shared_for_tests();
        let spec = crate::engine::BtSpec {
            id: 61,
            source: "/tmp/e2e-reconf.torrent".into(),
            meta: meta.clone(),
            save_dir,
            name: "e2e-reconf.bin".into(),
            files: vec![BtFile {
                path: "e2e-reconf.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 1,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
        };

        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let handle = tokio::spawn(run(spec.clone(), shared.clone(), cmd_rx, evt_tx));

        wait_bt_conns_established(&mut evt_rx, 1).await;

        cmd_tx
            .send(BtTaskCmd::Reconfigure {
                concurrency: 3,
                files: spec.files.clone(),
                proxy_endpoint: None,
                max_download_speed: 0,
                max_upload_speed: 0,
            })
            .await
            .unwrap();
        wait_bt_conns_established(&mut evt_rx, 3).await;
        assert!(
            !handle.is_finished(),
            "热调不得重启引擎（引擎任务必须存活）"
        );

        cmd_tx.send(BtTaskCmd::Cancel).await.unwrap();
        let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reconfigure_shrinks_connections_and_keeps_downloading() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..400_000usize).map(|i| (i % 241) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-reconf-shrink");
        let save_dir = dir.to_string_lossy().into_owned();
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let ips = [
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 5)),
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 6)),
        ];
        let meta0 = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let mut peer_list = Vec::new();
        for (i, ip) in ips.iter().enumerate() {
            let mut pid = *b"-QX7777-seederpeer00";
            pid[19] = b'0' + i as u8;
            let port = counting_seeder_any(
                data.clone(),
                piece_len,
                meta0.infohash,
                counter.clone(),
                &pid,
            )
            .await;
            peer_list.push((*ip, port));
        }
        let tracker_port = fake_udp_tracker_multi(peer_list).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );

        let shared = crate::engine::shared_for_tests();
        let spec = crate::engine::BtSpec {
            id: 62,
            source: "/tmp/e2e-shrink.torrent".into(),
            meta: meta.clone(),
            save_dir,
            name: "e2e-shrink.bin".into(),
            files: vec![BtFile {
                path: "e2e-shrink.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 2,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
        };

        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let handle = tokio::spawn(run(spec.clone(), shared.clone(), cmd_rx, evt_tx));

        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let mut established_seen = 0usize;
        let mut last_downloaded = 0u64;
        while established_seen < 2 {
            assert!(std::time::Instant::now() < deadline, "双连接未建立");
            let Some(evt) = tokio::time::timeout(Duration::from_millis(500), evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                continue;
            };
            match evt {
                Evt::BtConns { conns, .. } => {
                    established_seen = conns.len().max(established_seen);
                }
                Evt::BtProgress { downloaded, .. } => {
                    assert!(
                        downloaded >= last_downloaded,
                        "已下载字节单调不减：{last_downloaded} → {downloaded}"
                    );
                    last_downloaded = downloaded;
                }
                _ => {}
            }
        }

        cmd_tx
            .send(BtTaskCmd::Reconfigure {
                concurrency: 1,
                files: spec.files.clone(),
                proxy_endpoint: None,
                max_download_speed: 0,
                max_upload_speed: 0,
            })
            .await
            .unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut shrunk = false;
        let mut progress_after = 0usize;
        while std::time::Instant::now() < deadline {
            let Some(evt) = tokio::time::timeout(Duration::from_millis(300), evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                continue;
            };
            match evt {
                Evt::BtConns { conns, .. } => {
                    if !shrunk {
                        assert!(
                            conns.len() <= 2,
                            "热调前快照不得超过原并发，实际 {}",
                            conns.len()
                        );
                        if conns.len() <= 1 {
                            shrunk = true;
                        }
                    } else {
                        assert!(
                            conns.len() <= 1,
                            "收缩后连接数 ≤ 目标并发，实际 {}",
                            conns.len()
                        );
                    }
                }
                Evt::BtProgress { downloaded, .. } => {
                    assert!(
                        downloaded >= last_downloaded,
                        "收缩后字节单调不减：{last_downloaded} → {downloaded}"
                    );
                    last_downloaded = downloaded;
                    if shrunk {
                        progress_after += 1;
                    }
                }
                _ => {}
            }
        }
        assert!(shrunk, "收缩后应观察到 ≤1 条连接的 BtConns 快照");
        assert!(progress_after > 0, "收缩后任务保持下载（进度事件继续）");
        assert!(
            !handle.is_finished(),
            "热调不得重启引擎（引擎任务必须存活）"
        );

        cmd_tx.send(BtTaskCmd::Cancel).await.unwrap();
        let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn graceful_close_sends_fin_then_force_closes_after_grace() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let dialer = tokio::spawn(async move {
            loop {
                if let Ok(s) = TcpStream::connect(addr).await {
                    return s;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        });
        let (peer_sock, _) = listener.accept().await.unwrap();
        let ezr_sock = dialer.await.unwrap();
        let (mut pr, pw) = peer_sock.into_split();
        let (rd, wr) = ezr_sock.into_split();
        let reader = tokio::spawn(async move {
            let mut rd = rd;
            let mut buf = [0u8; 64];
            loop {
                match rd.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
        });
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();
        let peer_writer = tokio::spawn(async move {
            let mut pw = pw;
            tokio::time::sleep(Duration::from_millis(60)).await;
            let mid = pw.write_all(b"hello").await.is_ok();
            let _ = done_rx.await;
            let _ = pw.write_all(b"x").await;
            let late = pw.write_all(b"y").await.is_err();
            (mid, late)
        });
        graceful_close(wr, reader, Duration::from_millis(200)).await;
        let _ = done_tx.send(());
        let (mid_ok, late_err) = peer_writer.await.unwrap();
        assert!(mid_ok, "排水窗内对端写入应成功（读半部仍在排水）");
        let mut buf = [0u8; 8];
        let n = pr.read(&mut buf).await.unwrap();
        assert_eq!(n, 0, "对端应读到 FIN/EOF（有序关闭）");
        assert!(late_err, "排水窗后强制收口，对端写入应失败（无滞留读半部）");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pause_resume_reuses_session_peer_id() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..40_960usize).map(|i| (i % 241) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-session-pid");
        let save_dir = dir.to_string_lossy().into_owned();
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let probe = std::sync::Arc::new(std::sync::Mutex::new(SeederProbe::default()));

        let meta0 = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let seeder_port = probing_seeder(
            data.clone(),
            piece_len,
            meta0.infohash,
            counter.clone(),
            probe.clone(),
        )
        .await;
        let tracker_port = fake_udp_tracker_fast(seeder_port).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );

        let shared = crate::engine::shared_for_tests();
        let make_spec = |id: u32| crate::engine::BtSpec {
            id,
            source: "/tmp/e2e-session-pid.torrent".into(),
            meta: meta.clone(),
            save_dir: save_dir.clone(),
            name: "e2e-session-pid.bin".into(),
            files: vec![BtFile {
                path: "e2e-session-pid.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 2,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
        };

        let (evt_tx1, mut evt_rx1) = mpsc::channel(256);
        let (cmd_tx1, cmd_rx1) = mpsc::channel::<BtTaskCmd>(8);
        let run1 = tokio::spawn(run(make_spec(81), shared.clone(), cmd_rx1, evt_tx1));
        wait_bt_conns_established(&mut evt_rx1, 1).await;
        cmd_tx1.send(BtTaskCmd::Cancel).await.unwrap();
        let _ = tokio::time::timeout(Duration::from_secs(5), run1).await;

        let (evt_tx2, mut evt_rx2) = mpsc::channel(256);
        let (_cmd_tx2, cmd_rx2) = mpsc::channel::<BtTaskCmd>(8);
        let run2 = tokio::spawn(run(make_spec(82), shared.clone(), cmd_rx2, evt_tx2));
        wait_bt_conns_established(&mut evt_rx2, 1).await;
        run2.abort();

        let ids = probe.lock().expect("probe 锁").peer_ids.clone();
        assert!(ids.len() >= 2, "两轮各应有一次握手，实际 {}", ids.len());
        assert_eq!(
            ids[0], ids[1],
            "跨暂停/重启 peer_id 必须全等（FR-02-27① 会话级稳定）"
        );
        assert_eq!(&ids[0][..8], b"-EZ2000-", "握手 peer_id 前缀形态不变");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pause_closes_peer_connections_with_eof_not_reset() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..40_960usize).map(|i| (i % 241) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-graceful-eof");
        let save_dir = dir.to_string_lossy().into_owned();
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let probe = std::sync::Arc::new(std::sync::Mutex::new(SeederProbe::default()));

        let meta0 = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let seeder_port = probing_seeder(
            data.clone(),
            piece_len,
            meta0.infohash,
            counter.clone(),
            probe.clone(),
        )
        .await;
        let tracker_port = fake_udp_tracker_fast(seeder_port).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );

        let shared = crate::engine::shared_for_tests();
        let spec = crate::engine::BtSpec {
            id: 83,
            source: "/tmp/e2e-graceful.torrent".into(),
            meta: meta.clone(),
            save_dir,
            name: "e2e-graceful.bin".into(),
            files: vec![BtFile {
                path: "e2e-graceful.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 2,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
        };

        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let handle = tokio::spawn(run(spec, shared.clone(), cmd_rx, evt_tx));
        wait_bt_conns_established(&mut evt_rx, 1).await;
        cmd_tx.send(BtTaskCmd::Cancel).await.unwrap();
        let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;

        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let kind = loop {
            assert!(
                std::time::Instant::now() < deadline,
                "seeder 侧未观察到连接收口"
            );
            let k = probe.lock().expect("probe 锁").close_kind.clone();
            if let Some(k) = k {
                break k;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        assert!(
            kind.to_lowercase().contains("eof") && !kind.to_lowercase().contains("reset"),
            "暂停收口必须为有序 FIN（对端读到 EOF），不得产生 RST（FR-02-27②）；实际收口：{kind}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn shed_then_grow_respects_self_close_cooldown() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..400_000usize).map(|i| (i % 241) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-selfclose-cool");
        let save_dir = dir.to_string_lossy().into_owned();
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let ips = [
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 7)),
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 8)),
        ];
        let meta0 = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let mut peer_list = Vec::new();
        for (i, ip) in ips.iter().enumerate() {
            let mut pid = *b"-QX7777-seederpeer00";
            pid[19] = b'0' + i as u8;
            let port = counting_seeder_any(
                data.clone(),
                piece_len,
                meta0.infohash,
                counter.clone(),
                &pid,
            )
            .await;
            peer_list.push((*ip, port));
        }
        let tracker_port = fake_udp_tracker_multi(peer_list).await;
        let meta = make_meta(
            &data,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );

        let shared = crate::engine::shared_for_tests();
        let spec = crate::engine::BtSpec {
            id: 84,
            source: "/tmp/e2e-cool.torrent".into(),
            meta: meta.clone(),
            save_dir,
            name: "e2e-cool.bin".into(),
            files: vec![BtFile {
                path: "e2e-cool.bin".into(),
                size: data.len() as u64,
                selected: true,
            }],
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 2,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: crate::engine::BtTunables {
                peer_retry_base_secs: 1,
                ..Default::default()
            },
        };

        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let handle = tokio::spawn(run(spec.clone(), shared.clone(), cmd_rx, evt_tx));
        wait_bt_conns_established(&mut evt_rx, 2).await;

        for cycle in 0..3 {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            cmd_tx
                .send(BtTaskCmd::Reconfigure {
                    concurrency: 1,
                    files: spec.files.clone(),
                    proxy_endpoint: None,
                    max_download_speed: 0,
                    max_upload_speed: 0,
                })
                .await
                .unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                assert!(
                    std::time::Instant::now() < deadline,
                    "cycle {cycle}: 收缩未生效"
                );
                let Some(evt) = tokio::time::timeout(Duration::from_millis(300), evt_rx.recv())
                    .await
                    .ok()
                    .flatten()
                else {
                    continue;
                };
                if let Evt::BtConns { conns, .. } = evt {
                    if conns.len() <= 1 {
                        break;
                    }
                }
            }
            let t0 = std::time::Instant::now();
            cmd_tx
                .send(BtTaskCmd::Reconfigure {
                    concurrency: 2,
                    files: spec.files.clone(),
                    proxy_endpoint: None,
                    max_download_speed: 0,
                    max_upload_speed: 0,
                })
                .await
                .unwrap();
            let deadline = t0 + Duration::from_secs(6);
            loop {
                assert!(
                    std::time::Instant::now() < deadline,
                    "cycle {cycle}: 回补未在期限内建立"
                );
                let Some(evt) = tokio::time::timeout(Duration::from_millis(200), evt_rx.recv())
                    .await
                    .ok()
                    .flatten()
                else {
                    continue;
                };
                if let Evt::BtConns { conns, .. } = evt {
                    if conns.len() >= 2 {
                        break;
                    }
                }
            }
            let elapsed = t0.elapsed();
            assert!(
                elapsed >= Duration::from_millis(700),
                "cycle {cycle}: 回补仅耗时 {elapsed:?}——自关候选被立即重拨（FR-02-27③ 自关冷却缺失）"
            );
            assert!(
                !handle.is_finished(),
                "热调不得重启引擎（引擎任务必须存活）"
            );
        }

        cmd_tx.send(BtTaskCmd::Cancel).await.unwrap();
        let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn make_meta_files(files: &[(&str, u64)], piece_len: usize, tracker: &str) -> TorrentMeta {
        let total: u64 = files.iter().map(|(_, s)| *s).sum();
        let data: Vec<u8> = (0..total as usize).map(|i| (i % 241) as u8).collect();
        let mut pieces = Vec::new();
        for chunk in data.chunks(piece_len) {
            let mut h = Sha1::new();
            h.update(chunk);
            pieces.extend_from_slice(&h.finalize());
        }
        let mut items = Vec::new();
        for (rel, size) in files {
            let segs: Vec<BValue> = rel
                .split('/')
                .map(|s| BValue::Bytes(s.as_bytes().to_vec()))
                .collect();
            let mut d = BTreeMap::new();
            d.insert(b"length".to_vec(), BValue::Int(*size as i64));
            d.insert(b"path".to_vec(), BValue::List(segs));
            items.push(BValue::Dict(d));
        }
        let mut info = BTreeMap::new();
        info.insert(b"files".to_vec(), BValue::List(items));
        info.insert(b"name".to_vec(), BValue::Bytes(b"mftor".to_vec()));
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

    fn concat_bytes(files: &[(&str, u64)]) -> Vec<u8> {
        let total: u64 = files.iter().map(|(_, s)| *s).sum();
        (0..total as usize).map(|i| (i % 241) as u8).collect()
    }

    #[tokio::test]
    async fn request_silenced_while_upload_paused() {
        let piece_len = 16 * 1024;
        let data: Vec<u8> = (0..16_384usize).map(|i| (i % 211) as u8).collect();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-upause");
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
        let meta = make_meta(&data, piece_len, "udp://127.0.0.1:1/announce");
        let spec = crate::engine::BtSpec {
            id: 98,
            source: "/tmp/g.torrent".into(),
            meta,
            save_dir: save_dir.clone(),
            name: "g.bin".into(),
            files,
            trackers: vec![],
            concurrency: 4,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
        };
        let pmap = PieceMap::new(&spec.files, spec.meta.piece_length);
        let mut done: HashSet<u32> = HashSet::new();
        done.insert(0);
        let mut downloaded = data.len() as u64;
        let mut partial: PartialPieces = HashMap::new();

        let mut unchoked = new_conn_stat("10.0.0.9:999".into());
        unchoked.am_choking = false;
        let (tx_a, mut rx_a) = mpsc::channel(8);
        unchoked.peer_out = tx_a;
        let mut conns = HashMap::from([(1usize, unchoked)]);
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
            &BtTunables::default(),
            Arc::new(tokio::sync::Semaphore::new(8)),
            &test_ul(),
            &test_ul(),
            &mut done,
            &mut downloaded,
            &mut handles,
            &mut partial,
            &mut PieceOrderCache::new(),
            true,
        )
        .await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            rx_a.try_recv().is_err(),
            "做种回退下载态零上传（D73②）：request 不被服务、cum_up 不增长"
        );
        assert_eq!(conns.get(&1).unwrap().cum_up, 0, "上传累计必须为零");
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
            &BtTunables::default(),
            Arc::new(tokio::sync::Semaphore::new(8)),
            &test_ul(),
            &test_ul(),
            &mut done,
            &mut downloaded,
            &mut handles,
            &mut partial,
            &mut PieceOrderCache::new(),
            false,
        )
        .await;
        let served =
            tokio::time::timeout(std::time::Duration::from_millis(2000), rx_a.recv()).await;
        assert!(
            matches!(served, Ok(Some(PeerOut::Send(peer::PeerMsg::Block { .. })))),
            "解除暂停后同一 request 被服务（D58 互惠面不受影响）"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reconfigure_deselect_deletes_then_reselect_redownloads() {
        let piece_len = 16 * 1024;
        let file_list: Vec<(&str, u64)> =
            vec![("mftor/a.bin", 20_000), ("mftor/sub/b.bin", 30_000)];
        let data = concat_bytes(&file_list);
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bteng-reconf-deldisk");
        let save_dir = dir.to_string_lossy().into_owned();
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seeder_ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 7));
        let meta0 = make_meta_files(&file_list, piece_len, "udp://127.0.0.1:1/announce");
        let mut pid = *b"-QX7777-seederpeer00";
        pid[19] = b'7';
        let seeder_port = counting_seeder_any(
            data.clone(),
            piece_len,
            meta0.infohash,
            counter.clone(),
            &pid,
        )
        .await;
        let tracker_port = fake_udp_tracker_multi(vec![(seeder_ip, seeder_port)]).await;
        let meta = make_meta_files(
            &file_list,
            piece_len,
            &format!("udp://127.0.0.1:{tracker_port}/announce"),
        );

        let files = vec![
            BtFile {
                path: "mftor/a.bin".into(),
                size: 20_000,
                selected: true,
            },
            BtFile {
                path: "mftor/sub/b.bin".into(),
                size: 30_000,
                selected: true,
            },
        ];
        let shared = crate::engine::shared_for_tests();
        let spec = crate::engine::BtSpec {
            id: 99,
            source: "/tmp/e2e-deldisk.torrent".into(),
            meta,
            save_dir,
            name: "mftor".into(),
            files,
            trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
            concurrency: 2,
            proxy_endpoint: None,
            seeding: false,
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
        };

        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let handle = tokio::spawn(run(spec.clone(), shared.clone(), cmd_rx, evt_tx));

        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            assert!(std::time::Instant::now() < deadline, "首下载未在时限内完成");
            let Some(evt) = tokio::time::timeout(Duration::from_millis(500), evt_rx.recv())
                .await
                .ok()
                .flatten()
            else {
                continue;
            };
            if matches!(evt, Evt::BtDone { .. }) {
                break;
            }
        }
        let a_path = format!("{}/mftor/a.bin", spec.save_dir);
        let b_path = format!("{}/mftor/sub/b.bin", spec.save_dir);
        assert_eq!(
            std::fs::read(&a_path).unwrap(),
            data[..20_000],
            "首下载文件 A 字节正确"
        );
        assert_eq!(
            std::fs::read(&b_path).unwrap(),
            data[20_000..],
            "首下载文件 B 字节正确"
        );

        let mut deselected = spec.files.clone();
        deselected[1].selected = false;
        cmd_tx
            .send(BtTaskCmd::Reconfigure {
                concurrency: 2,
                files: deselected.clone(),
                proxy_endpoint: None,
                max_download_speed: 0,
                max_upload_speed: 0,
            })
            .await
            .unwrap();
        let del_deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::fs::read(&b_path).is_ok() {
            assert!(
                std::time::Instant::now() < del_deadline,
                "取消选中后落盘文件未被删除（FR-02-19 v1.16）"
            );
            while evt_rx.try_recv().is_ok() {}
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(
            !std::path::Path::new(&format!("{}/mftor/sub", spec.save_dir)).exists(),
            "变空目录自底向上整树清理"
        );
        assert_eq!(
            std::fs::read(&a_path).unwrap(),
            data[..20_000],
            "保留文件不受删除影响"
        );

        let mut reselected = deselected.clone();
        reselected[1].selected = true;
        cmd_tx
            .send(BtTaskCmd::Reconfigure {
                concurrency: 2,
                files: reselected,
                proxy_endpoint: None,
                max_download_speed: 0,
                max_upload_speed: 0,
            })
            .await
            .unwrap();
        let re_deadline = std::time::Instant::now() + Duration::from_secs(20);
        let expect = &data[20_000..];
        loop {
            assert!(
                std::time::Instant::now() < re_deadline,
                "勾回文件未从头补下（句柄未关闭会写入孤儿 inode，文件永不落盘）"
            );
            while evt_rx.try_recv().is_ok() {}
            tokio::time::sleep(Duration::from_millis(100)).await;
            if std::fs::read(&b_path).is_ok_and(|bytes| bytes == expect) {
                break;
            }
        }

        cmd_tx.send(BtTaskCmd::Cancel).await.unwrap();
        let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod seeding_tests {
    use std::collections::BTreeMap;

    use sha1::{Digest, Sha1};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    use super::*;
    use crate::bt::bencode::{encode, BValue};
    use crate::bt::metainfo::TorrentMeta;
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
            upload_paused: false,
            max_download_speed: 0,
            max_upload_speed: 0,
            tunables: Default::default(),
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
        let eh = peer::parse_ext_handshake(&eh[2..]).expect("ext handshake 可解析");
        assert_eq!(
            eh.metadata_size.map(|s| s as usize),
            Some(meta.info_bytes.len())
        );
        assert_eq!(eh.ut_metadata, Some(1));
        let peer_ut_id = eh.ut_metadata.expect("ut_metadata id 在位");

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
