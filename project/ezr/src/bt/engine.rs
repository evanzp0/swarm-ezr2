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

enum PeerIn {
    Connected {
        abbrev: String,
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
    has: HashSet<u32>,
    inflight: HashMap<(u32, u32), (u32, std::time::Instant)>,
    peer_choking: bool,
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
    let mut downloaded: u64 = 0;
    let sidecar_path = crate::model::namegen::sidecar_path_of(&spec.save_dir, &spec.name);
    let mut candidates: Vec<u32> = spec.done_pieces.clone();
    if let Some(sc) = crate::model::BtSidecar::load(&sidecar_path) {
        for p in sc.data.done_pieces {
            if !candidates.contains(&p) {
                candidates.push(p);
            }
        }
    }
    for &p in &candidates {
        if !pmap.covered_pieces().contains(&p) {
            continue;
        }
        match read_piece_from_disk(&files_handles, &pmap, p).await {
            Ok(buf) if verify_piece(&buf, &spec.meta, p) => {
                done.insert(p);
                downloaded += selected_bytes_of_piece(&pmap, p);
            }
            _ => {}
        }
    }
    save_bt_sidecar(&spec, &sidecar_path, &done);
    let mut sidecar_saved_done: usize = done.len();

    let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) => {
            report_fail(&evt_tx, id, BtError::Io(e.to_string())).await;
            return;
        }
    };
    let listen_port = listener.local_addr().map(|a| a.port()).unwrap_or(6881);

    let mut seen = HashSet::new();
    let trackers: Vec<TrackerUrl> = spec
        .trackers
        .iter()
        .filter_map(|s| TrackerUrl::parse(s))
        .filter(|u| seen.insert(u.clone()))
        .collect();
    let live: Arc<std::sync::Mutex<LiveStats>> = Arc::new(std::sync::Mutex::new(LiveStats {
        uploaded: 0,
        downloaded,
        left: pmap.selected_total() - downloaded,
        completed: false,
        completed_sent: false,
    }));
    let (tr_tx, mut tr_rx) = mpsc::channel::<TrackerReport>(32);
    for (ti, url) in trackers.iter().enumerate() {
        spawn_tracker(
            id,
            ti,
            url.clone(),
            AnnounceCtx {
                infohash: spec.meta.infohash,
                peer_id,
                port: listen_port,
                client: shared.bt_tracker_client(spec.proxy_endpoint.as_ref()),
                numwant: 50,
                live: live.clone(),
            },
            tr_tx.clone(),
        );
    }
    drop(tr_tx);

    let (peer_tx, mut peer_rx) = mpsc::channel::<(usize, PeerIn)>(256);
    let mut next_conn: usize = 1;
    let mut conns: HashMap<usize, ConnStat> = HashMap::new();
    let mut tried: HashSet<(IpAddr, u16)> = HashSet::new();
    let mut seeding = spec.seeding;

    let mut tick = tokio::time::interval(Duration::from_secs(1));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut sweep = tokio::time::interval(Duration::from_secs(PEER_SWEEP_SECS));
    sweep.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let done_at_start = done.len();
    let _ = done_at_start;

    let mut tracker_fail_count = 0usize;
    let mut got_peers = !trackers.is_empty();
    let mut partial: PartialPieces = HashMap::new();

    loop {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    BtTaskCmd::Pause | BtTaskCmd::StopSeed => {
                        save_bt_sidecar(&spec, &sidecar_path, &done);
                        spawn_stopped_announce(id, &trackers, &spec, &peer_id, listen_port,
                            &shared, seeding);
                        report_stopped(&evt_tx, id, &pmap, &done, downloaded).await;
                        return;
                    }
                    BtTaskCmd::Cancel => {
                        report_stopped(&evt_tx, id, &pmap, &done, downloaded).await;
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
                            if tried.insert((p.ip, p.port)) && conns.len() < spec.concurrency {
                                spawn_outgoing_peer(next_conn, p, &spec, &peer_id, peer_tx.clone());
                                conns.insert(next_conn, new_conn_stat(format!("{}:{}", p.ip, p.port)));
                                next_conn += 1;
                            }
                        }
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
                    PeerIn::Connected { abbrev, out, ext_metadata } => {
                        if let Some(st) = conns.get_mut(&conn_id) {
                            st.abbrev = abbrev;
                            st.peer_out = out;
                            let bitfield = bitfield_bytes(&done, pmap.num_pieces());
                            if !bitfield.is_empty() {
                                let _ = st.peer_out.send(PeerOut::Send(peer::PeerMsg::Bitfield(bitfield))).await;
                            }
                            if seeding {
                                st.peer_choking = false;
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
                        let _ = reason;
                        conns.remove(&conn_id);
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
                            seeding,
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
                                partial_blocks: partial_pieces(&conns, &done),
                                uploaded: uploaded_total(&conns),
                                upload_speed: 0.0,
                            }).await;
                            let _ = evt_tx.send(Evt::BtConns { id, conns: views }).await;
                        }
                        if !seeding && done.len() >= pmap.covered_pieces().len()
                            && !pmap.covered_pieces().is_empty() {
                            seeding = true;
                            live.lock().expect("live stats 锁").completed = true;
                            let _ = crate::model::BtSidecar::remove(&sidecar_path);
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
                    if let Some(st) = conns.get_mut(&cid) {
                        issue_requests(cid, st, &claimed, &pmap, &done).await;
                    }
                }
                let views = conn_views(&mut conns, pmap.covered_pieces());
                let sum_up_speed: f64 = views.iter().map(|v| v.up_speed).sum();
                if done.len() != sidecar_saved_done {
                    save_bt_sidecar(&spec, &sidecar_path, &done);
                    sidecar_saved_done = done.len();
                }
                let _ = evt_tx.send(Evt::BtProgress {
                    id,
                    downloaded,
                    chunk_done: done.len() as u32,
                    done_blocks: done.iter().copied().collect::<Vec<_>>(),
                    partial_blocks: partial_pieces(&conns, &done),
                    uploaded: uploaded_total(&conns),
                    upload_speed: sum_up_speed,
                }).await;
                let _ = evt_tx.send(Evt::BtConns { id, conns: views }).await;
            }
            _ = sweep.tick() => {
                conns.retain(|_, _| true);
            }
            else => break,
        }
    }
    let _ = got_peers;
}

type PartialPieces = HashMap<u32, BTreeMap<u32, Vec<u8>>>;

struct AnnounceCtx {
    infohash: [u8; 20],
    peer_id: [u8; 20],
    port: u16,
    client: reqwest::Client,
    numwant: u32,
    live: Arc<std::sync::Mutex<LiveStats>>,
}

#[derive(Default)]
struct LiveStats {
    uploaded: u64,
    downloaded: u64,
    left: u64,
    completed: bool,
    completed_sent: bool,
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

fn spawn_tracker(
    id: u32,
    ti: usize,
    url: TrackerUrl,
    ctx: AnnounceCtx,
    tx: mpsc::Sender<TrackerReport>,
) {
    tokio::spawn(async move {
        let mut delay = FIRST_ANNOUNCE_DELAY;
        let mut min_interval_secs: u32 = 0;
        let mut first = true;
        loop {
            let wait = delay.max(min_interval_secs as u64);
            tokio::time::sleep(Duration::from_secs(wait)).await;
            let first_now = first;
            first = false;
            let (up, down, left, ev) = {
                let mut st = ctx.live.lock().expect("live stats 锁");
                if st.completed && !st.completed_sent {
                    st.completed_sent = true;
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
                    let _ = ti;
                    let _ = id;
                }
            }
        }
    });
}

fn spawn_stopped_announce(
    id: u32,
    trackers: &[TrackerUrl],
    spec: &crate::engine::BtSpec,
    peer_id: &[u8; 20],
    port: u16,
    shared: &Arc<EngineShared>,
    seeding: bool,
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
                uploaded: 0,
                downloaded: 0,
                left: 0,
                event: Some(TrackerEvent::Stopped),
                numwant: 0,
            };
            let _ = id;
            let _ = seeding;
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
            Duration::from_secs(super::HANDSHAKE_TIMEOUT_SECS),
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
    let host = addr.rsplit_once(':').map_or(addr, |(h, _)| h);
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.parse().ok()
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
            let abbrev = super::peer_id::client_abbrev(&ok.peer_id);
            let ext_metadata = ok.reserved[5] & 0x10 != 0;
            if tx
                .send((
                    conn_id,
                    PeerIn::Connected {
                        abbrev,
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
        has: HashSet::new(),
        inflight: HashMap::new(),
        peer_choking: true,
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
    seeding: bool,
) {
    let claimed = claimed_blocks(conns, partial);
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
            let want = st
                .has
                .iter()
                .any(|p| !done.contains(p) && pmap.covered_pieces().contains(p));
            if want {
                send(conn_id, st, peer::PeerMsg::Interested).await;
            }
        }
        peer::PeerMsg::Have(p) => {
            st.has.insert(p);
            let want = !done.contains(&p) && pmap.covered_pieces().contains(&p);
            if want {
                send(conn_id, st, peer::PeerMsg::Interested).await;
                if !st.peer_choking {
                    issue_requests(conn_id, st, &claimed, pmap, done).await;
                }
            }
        }
        peer::PeerMsg::Unchoke => {
            st.peer_choking = false;
            issue_requests(conn_id, st, &claimed, pmap, done).await;
        }
        peer::PeerMsg::Choke => {
            st.peer_choking = true;
            st.inflight.clear();
        }
        peer::PeerMsg::Block { index, begin, data } => {
            on_block(
                conn_id, index, begin, data, conns, pmap, spec, done, downloaded, files, partial,
                seeding,
            )
            .await;
        }
        peer::PeerMsg::Request {
            index,
            begin,
            length,
        } => {
            let allowed = seeding || !st.peer_choking;
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
        peer::PeerMsg::Interested
        | peer::PeerMsg::KeepAlive
        | peer::PeerMsg::NotInterested
        | peer::PeerMsg::Port(_) => {}
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

async fn issue_requests(
    conn_id: usize,
    st: &mut ConnStat,
    claimed: &HashSet<(u32, u32)>,
    pmap: &PieceMap,
    done: &HashSet<u32>,
) {
    if st.peer_choking || st.inflight.len() >= PEER_PIPELINE {
        return;
    }
    for &piece in pmap.covered_pieces() {
        if st.inflight.len() >= PEER_PIPELINE {
            return;
        }
        if done.contains(&piece) || !st.has.contains(&piece) {
            continue;
        }
        let plen = pmap.piece_len(piece) as usize;
        let mut asked = 0usize;
        let mut begin = 0usize;
        while begin < plen && asked < REQUESTS_PER_PIECE && st.inflight.len() < PEER_PIPELINE {
            let key = (piece, begin as u32);
            if claimed.contains(&key) {
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
    }
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
    seeding: bool,
) {
    let mut st = match conns.remove(&conn_id) {
        Some(s) => s,
        None => return,
    };
    st.inflight.remove(&(index, begin));
    st.cum_down += data.len() as u64;

    if done.contains(&index) {
        conns.insert(conn_id, st);
        let claimed = claimed_blocks(conns, partial);
        if let Some(st) = conns.get_mut(&conn_id) {
            issue_requests(conn_id, st, &claimed, pmap, done).await;
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
    if let Some(st) = conns.get_mut(&conn_id) {
        issue_requests(conn_id, st, &claimed, pmap, done).await;
    }
    let _ = seeding;
}

fn file_abs_path(save_dir: &str, task_name: &str, rel: &str) -> String {
    let p = std::path::Path::new(rel);
    if p.parent().is_none() || p.components().count() == 1 {
        return format!("{save_dir}/{rel}");
    }
    format!("{save_dir}/{task_name}/{rel}")
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

fn save_bt_sidecar(spec: &crate::engine::BtSpec, path: &str, done: &HashSet<u32>) {
    let mut pieces: Vec<u32> = done.iter().copied().collect();
    pieces.sort_unstable();
    let sc = crate::model::BtSidecar::build(
        spec.id,
        &spec.name,
        &spec.save_dir,
        spec.concurrency,
        crate::model::BtTaskData {
            source: spec.source.clone(),
            trackers: spec.trackers.clone(),
            piece_length: spec.meta.piece_length,
            files: spec.files.clone(),
            done_pieces: pieces,
        },
    );
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

fn partial_pieces(conns: &HashMap<usize, ConnStat>, done: &HashSet<u32>) -> Vec<u32> {
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
    pmap: &PieceMap,
    done: &HashSet<u32>,
    downloaded: u64,
) {
    let _ = pmap;
    let _ = evt_tx
        .send(Evt::BtStopped {
            id,
            downloaded,
            done_pieces: done.iter().copied().collect::<Vec<_>>(),
            partial_pieces: Vec::new(),
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
            done_pieces: vec![],
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
            done_pieces: vec![],
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
                Ok(peer::PeerMsg::Interested) | Ok(peer::PeerMsg::KeepAlive) => {}
                _ => return,
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
            done_pieces: vec![],
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
            done_pieces: vec![],
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
        let sc = crate::model::BtSidecar::load(&sc_path).expect("sidecar 可解析");
        assert_eq!(sc.version, crate::model::bt::BT_SIDECAR_VERSION);
        assert_eq!(sc.id, 91);
        assert_eq!(sc.name, "sc.bin");
        assert_eq!(
            sc.save_dir, save_dir,
            "sidecar 路径口径 = save_dir/name.ezr"
        );
        assert_eq!(sc.concurrency, 2);
        assert_eq!(
            sc.data.trackers,
            vec![format!("udp://127.0.0.1:{seeder_port}/announce")]
        );
        assert_eq!(sc.data.piece_length, piece_len as u32);
        assert_eq!(sc.data.files.len(), 1);
        assert_eq!(sc.data.source, "/tmp/sc.torrent");
        assert!(sc.data.done_pieces.is_empty(), "启动时无已完成 piece");

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
        crate::model::BtSidecar::build(
            93,
            "sc2.bin",
            &save_dir,
            4,
            crate::model::BtTaskData {
                source: "/tmp/sc2.torrent".into(),
                trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
                piece_length: piece_len as u32,
                files: vec![BtFile {
                    path: "sc2.bin".into(),
                    size: data.len() as u64,
                    selected: true,
                }],
                done_pieces: (0..num_pieces).collect(),
            },
        )
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
            done_pieces: vec![],
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
            done_pieces: vec![],
            seeding: false,
            max_speed: 0,
        };
        let sc_path = crate::model::namegen::sidecar_path_of(&save_dir, "sc3.bin");
        std::fs::write(format!("{save_dir}/sc3.bin"), &data).unwrap();
        crate::model::BtSidecar::build(
            94,
            "sc3.bin",
            &save_dir,
            4,
            crate::model::BtTaskData {
                source: "/tmp/sc3.torrent".into(),
                trackers: vec![format!("udp://127.0.0.1:{tracker_port}/announce")],
                piece_length: piece_len as u32,
                files: vec![BtFile {
                    path: "sc3.bin".into(),
                    size: data.len() as u64,
                    selected: true,
                }],
                done_pieces: (0..half).collect(),
            },
        )
        .save(&sc_path)
        .unwrap();

        let (evt_tx, mut evt_rx) = mpsc::channel(256);
        let (cmd_tx, cmd_rx) = mpsc::channel::<BtTaskCmd>(8);
        let shared = crate::engine::shared_for_tests();
        let handle = tokio::spawn(run(spec, shared, cmd_rx, evt_tx));
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
        let sc = crate::model::BtSidecar::load(&sc_path).expect("暂停后 sidecar 在盘");
        assert_eq!(
            sc.data.done_pieces.len(),
            half as usize,
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
            done_pieces: vec![],
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
        let num_pieces = data.len().div_ceil(piece_len) as u32;

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
            done_pieces: (0..num_pieces).collect(),
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
