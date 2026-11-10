use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::mpsc;

use super::conn::{DlFlow, PeerIn};
use super::peerwire::run_peer_loop;
use crate::bt::tracker::{
    announce_http, announce_udp, announce_ws, build_announce_json, build_answer_announce_json,
    classify_ws_message, udp_host_port, AnnounceParams, PeerAddr, TrackerEvent, TrackerUrl,
    WsAnnounceResponse, WsInbound, WsOffer, WsSession, WsSignal,
};
use crate::bt::webrtc::{
    PeerTransports, PendingOffer, RtcReady, SignalState, OFFERS_PER_ANNOUNCE, PENDING_OFFER_CAP,
};
use crate::bt::BtError;
use crate::engine::{BtTunables, EngineShared};

pub(super) const FIRST_ANNOUNCE_DELAY: u64 = 2;

pub(super) struct AnnounceCtx {
    pub(super) infohash: [u8; 20],
    pub(super) peer_id: [u8; 20],
    pub(super) port: u16,
    pub(super) client: reqwest::Client,
    pub(super) numwant: u32,
    pub(super) live: Arc<std::sync::Mutex<LiveStats>>,
    pub(super) completed_notify: Arc<tokio::sync::Notify>,
    // 任务扩容（修改并发）触发的提前 re-announce（bug28③ 调优）
    pub(super) reannounce_notify: Arc<tokio::sync::Notify>,
    pub(super) backoff_start_secs: u64,
    pub(super) backoff_cap_secs: u64,
    pub(super) udp_base_timeout_secs: u64,
    pub(super) udp_retries: u32,
    pub(super) signals: Arc<std::sync::Mutex<SignalState>>,
    pub(super) rtc_tx: mpsc::Sender<RtcReady>,
    pub(super) transports: PeerTransports,
    pub(super) rtc_retry_base_secs: u64,
    pub(super) rtc_retry_cap_secs: u64,
}

#[derive(Default)]
pub(super) struct LiveStats {
    pub(super) uploaded: u64,
    pub(super) downloaded: u64,
    pub(super) left: u64,
    pub(super) completed: bool,
}

pub(super) enum TrackerReport {
    Peers {
        url: String,
        peers: Vec<PeerAddr>,
        complete: u32,
        incomplete: u32,
        interval_secs: u64,
    },
    Failed {
        url: String,
        retry_secs: u64,
    },
}

pub(super) fn spawn_tracker(
    url: TrackerUrl,
    ctx: AnnounceCtx,
    tx: mpsc::Sender<TrackerReport>,
) -> tokio::task::AbortHandle {
    tokio::spawn(async move {
        let mut delay = FIRST_ANNOUNCE_DELAY;
        let mut min_interval_secs: u32 = 0;
        let mut first = true;
        let mut completed_sent = false;
        let mut last_announce = tokio::time::Instant::now();
        loop {
            let wait = delay.max(min_interval_secs as u64);
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
                _ = ctx.completed_notify.notified() => {}
                _ = ctx.reannounce_notify.notified() => {
                    // 扩容触发的提前 announce（bug28③）：尊重 tracker min_interval，
                    // 未到最早许可时刻则补足剩余等待后再发。
                    let wait_more = u64::from(min_interval_secs)
                        .saturating_sub(last_announce.elapsed().as_secs());
                    if wait_more > 0 {
                        tokio::time::sleep(Duration::from_secs(wait_more)).await;
                    }
                }
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
            last_announce = tokio::time::Instant::now();
            let res: Result<crate::bt::tracker::TrackerResponse, BtError> = match &url {
                TrackerUrl::Http(u) => announce_http(u, &params, &ctx.client).await,
                TrackerUrl::Ws(u) => {
                    // 持久会话：连接保持打开、推送即时处理；会话结束（断开/
                    // 被拒）返回后由统一失败路径退避重连。
                    let _ = ws_tracker_session(u, &ctx, &tx).await;
                    Err(BtError::Tracker("ws tracker 会话结束".into()))
                }
                TrackerUrl::Udp(u) => {
                    let Some((host, port)) = udp_host_port(u) else {
                        break;
                    };
                    match tokio::net::UdpSocket::bind("0.0.0.0:0").await {
                        Ok(sock) => {
                            announce_udp(
                                sock,
                                &host,
                                port,
                                &params,
                                ctx.udp_base_timeout_secs,
                                ctx.udp_retries,
                            )
                            .await
                        }
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
                            url: announce_url(&url),
                            peers: resp.peers,
                            complete: resp.complete,
                            incomplete: resp.incomplete,
                            interval_secs: delay,
                        })
                        .await;
                }
                Err(e) => {
                    let _ = e;
                    delay = delay
                        .saturating_mul(2)
                        .clamp(ctx.backoff_start_secs, ctx.backoff_cap_secs);
                    let _ = tx
                        .send(TrackerReport::Failed {
                            url: announce_url(&url),
                            retry_secs: delay,
                        })
                        .await;
                }
            }
        }
    })
    .abort_handle()
}

pub(super) fn start_tracker_pool(
    trackers: &[TrackerUrl],
    infohash: [u8; 20],
    peer_id: [u8; 20],
    port: u16,
    client: reqwest::Client,
    tun: &BtTunables,
    live: &Arc<std::sync::Mutex<LiveStats>>,
    tr_tx: &mpsc::Sender<TrackerReport>,
    signals: &Arc<std::sync::Mutex<SignalState>>,
    rtc_tx: &mpsc::Sender<RtcReady>,
    reannounce_notify: &Arc<tokio::sync::Notify>,
) -> (Vec<Arc<tokio::sync::Notify>>, Vec<tokio::task::AbortHandle>) {
    let mut notifies = Vec::with_capacity(trackers.len());
    let mut handles = Vec::with_capacity(trackers.len());
    for url in trackers {
        let completed_notify = Arc::new(tokio::sync::Notify::new());
        let ctx = AnnounceCtx {
            infohash,
            peer_id,
            port,
            client: client.clone(),
            numwant: tun.tracker_numwant,
            live: live.clone(),
            completed_notify: completed_notify.clone(),
            reannounce_notify: Arc::clone(reannounce_notify),
            backoff_start_secs: tun.tracker_backoff_start_secs,
            backoff_cap_secs: tun.tracker_backoff_cap_secs,
            udp_base_timeout_secs: tun.udp_base_timeout_secs,
            udp_retries: tun.udp_retries,
            signals: Arc::clone(signals),
            rtc_tx: rtc_tx.clone(),
            transports: tun.transports,
            rtc_retry_base_secs: tun.peer_retry_base_secs,
            rtc_retry_cap_secs: tun.peer_retry_cap_secs,
        };
        handles.push(spawn_tracker(url.clone(), ctx, tr_tx.clone()));
        notifies.push(completed_notify);
    }
    (notifies, handles)
}

pub(super) fn spawn_stopped_announce(
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
    let udp_base = spec.tunables.udp_base_timeout_secs;
    let udp_retries = spec.tunables.udp_retries;
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
                TrackerUrl::Ws(u) => {
                    let _ = announce_ws(u, &params, &[], None).await;
                }
                TrackerUrl::Udp(u) => {
                    if let Some((host, p)) = udp_host_port(u) {
                        if let Ok(sock) = tokio::net::UdpSocket::bind("0.0.0.0:0").await {
                            let _ =
                                announce_udp(sock, &host, p, &params, udp_base, udp_retries).await;
                        }
                    }
                }
            }
        }
    });
}

pub(super) fn live_snapshot(live: &std::sync::Mutex<LiveStats>) -> (u64, u64, u64) {
    let st = live.lock().expect("live stats 锁");
    (st.uploaded, st.downloaded, st.left)
}

pub(super) const DEFAULT_WS_ANNOUNCE_INTERVAL_SECS: u64 = 30;

/// ws tracker 持久会话（真实 WebTorrent 语义）：连接保持打开，announce 周期
/// 重发（interval 由服务器给出），offer/answer 推送即时处理。返回 Err 表示
/// 会话结束（断开/被拒），由外层退避后重连。
pub(super) async fn ws_tracker_session(
    url: &str,
    ctx: &AnnounceCtx,
    tx: &mpsc::Sender<TrackerReport>,
) -> Result<(), BtError> {
    let mut sess = tokio::time::timeout(
        Duration::from_secs(crate::bt::tracker::WS_CONNECT_TIMEOUT_SECS),
        WsSession::connect(url),
    )
    .await
    .map_err(|_| BtError::Tracker("ws tracker 连接超时".into()))??;
    let url_owned = TrackerUrl::Ws(url.to_string());
    let mut interval_secs: u64 = DEFAULT_WS_ANNOUNCE_INTERVAL_SECS;
    let mut min_interval_secs: u64 = 0;
    let mut first = true;
    let mut completed_sent = false;
    loop {
        let ev = {
            let st = ctx.live.lock().expect("live stats 锁");
            if st.completed && !completed_sent {
                completed_sent = true;
                Some(TrackerEvent::Completed)
            } else if first {
                Some(TrackerEvent::Started)
            } else {
                None
            }
        };
        first = false;
        let (up, down, left) = {
            let st = ctx.live.lock().expect("live stats 锁");
            (st.uploaded, st.downloaded, st.left)
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
        // completed 不携带 offers（webtorrent 口径）；started/周期性 announce 携带
        let offers = if ev == Some(TrackerEvent::Completed) {
            Vec::new()
        } else {
            ws_prepare_offers(ctx).await
        };
        sess.send_json(&build_announce_json(&params, &offers, None))
            .await?;
        let last_announce = tokio::time::Instant::now();
        let mut next_announce = last_announce + Duration::from_secs(interval_secs);
        loop {
            let wait = next_announce.saturating_duration_since(tokio::time::Instant::now());
            tokio::select! {
                _ = ctx.completed_notify.notified() => {
                    if !completed_sent {
                        completed_sent = true;
                        let (up, down, left) = {
                            let st = ctx.live.lock().expect("live stats 锁");
                            (st.uploaded, st.downloaded, st.left)
                        };
                        let p = AnnounceParams {
                            event: Some(TrackerEvent::Completed),
                            uploaded: up,
                            downloaded: down,
                            left,
                            ..params
                        };
                        sess.send_json(&build_announce_json(&p, &[], None)).await?;
                    }
                    next_announce =
                        tokio::time::Instant::now() + Duration::from_secs(interval_secs);
                }
                _ = ctx.reannounce_notify.notified() => {
                    // 扩容触发的提前 announce（携带新 offers，bug28③）：尊重
                    // min_interval，未到最早许可时刻则补足剩余等待后重发。
                    let wait_more =
                        min_interval_secs.saturating_sub(last_announce.elapsed().as_secs());
                    if wait_more > 0 {
                        tokio::time::sleep(Duration::from_secs(wait_more)).await;
                    }
                    break;
                }
                inbound = sess.recv_json(wait) => match inbound {
                    Err(e) => return Err(e),
                    Ok(None) => break,
                    Ok(Some(v)) => match classify_ws_message(&v) {
                        WsInbound::Failure(fr) => {
                            return Err(BtError::Tracker(format!("tracker 拒绝：{fr}")));
                        }
                        WsInbound::Response(r) => {
                            if r.interval > 0 {
                                interval_secs = u64::from(r.interval);
                            }
                            min_interval_secs = u64::from(r.min_interval.unwrap_or(0));
                            let _ = tx
                                .send(TrackerReport::Peers {
                                    url: announce_url(&url_owned),
                                    peers: r.peers.clone(),
                                    complete: r.complete,
                                    incomplete: r.incomplete,
                                    interval_secs,
                                })
                                .await;
                            ws_process_response(ctx, &mut sess, &r).await;
                            next_announce = tokio::time::Instant::now()
                                + Duration::from_secs(interval_secs);
                        }
                        WsInbound::Offer(sig) => {
                            ws_handle_offer(ctx, &mut sess, &sig).await;
                        }
                        WsInbound::Answer(sig) => {
                            ws_handle_answer(ctx, &sig).await;
                        }
                        WsInbound::Other => {}
                    },
                },
            }
        }
    }
}

/// 组装本轮 announce 携带的 offers：已知 swarm peer（响应含 peer_id 列表，
/// 如 harness/mini tracker 场景）按角色分工定向发；swarm 未知（真实 tracker
/// 不返回 peers 列表）则盲发，由 tracker 抽样转发。
pub(super) async fn ws_prepare_offers(ctx: &AnnounceCtx) -> Vec<WsOffer> {
    if !ctx.transports.webrtc {
        return Vec::new();
    }
    {
        let mut st = ctx.signals.lock().expect("signals 锁");
        st.prune_expired();
    }
    let targets = {
        let st = ctx.signals.lock().expect("signals 锁");
        st.plan_offer_targets(
            &ctx.peer_id,
            OFFERS_PER_ANNOUNCE,
            ctx.rtc_retry_base_secs,
            ctx.rtc_retry_cap_secs,
        )
    };
    let blind = {
        let st = ctx.signals.lock().expect("signals 锁");
        st.swarm_peers.is_empty()
    };
    let mut offers = Vec::new();
    for target in targets {
        let pc = match crate::bt::webrtc::new_peer_connection().await {
            Ok(pc) => pc,
            Err(_) => {
                ctx.signals
                    .lock()
                    .expect("signals 锁")
                    .mark_peer_fail(&target);
                continue;
            }
        };
        match crate::bt::webrtc::create_data_channel_offer(
            &pc,
            &ctx.peer_id,
            Some(target),
            ctx.rtc_tx.clone(),
        )
        .await
        {
            Ok(bundle) => {
                let accepted = {
                    let mut st = ctx.signals.lock().expect("signals 锁");
                    let ok =
                        st.pending_offers.len() < PENDING_OFFER_CAP && !st.is_busy_with(&target);
                    if ok {
                        st.pending_offers.insert(
                            bundle.offer_id.clone(),
                            PendingOffer {
                                to_peer_id: target,
                                pc: Arc::clone(&bundle.pc),
                                answered: false,
                                created: std::time::Instant::now(),
                                remote_slot: Arc::clone(&bundle.remote_slot),
                            },
                        );
                        st.pcs.push(Arc::clone(&bundle.pc));
                    }
                    ok
                };
                if accepted {
                    offers.push(WsOffer {
                        offer_id: bundle.offer_id,
                        sdp: bundle.sdp,
                        to_peer_id: target,
                    });
                } else {
                    let _ = bundle.pc.close().await;
                }
            }
            Err(_) => {
                let _ = pc.close().await;
                ctx.signals
                    .lock()
                    .expect("signals 锁")
                    .mark_peer_fail(&target);
            }
        }
    }
    if blind {
        while offers.len() < OFFERS_PER_ANNOUNCE {
            let room = {
                let st = ctx.signals.lock().expect("signals 锁");
                st.pending_offers.len() < PENDING_OFFER_CAP
            };
            if !room {
                break;
            }
            let pc = match crate::bt::webrtc::new_peer_connection().await {
                Ok(pc) => pc,
                Err(_) => break,
            };
            match crate::bt::webrtc::create_data_channel_offer(
                &pc,
                &ctx.peer_id,
                None,
                ctx.rtc_tx.clone(),
            )
            .await
            {
                Ok(bundle) => {
                    let accepted = {
                        let mut st = ctx.signals.lock().expect("signals 锁");
                        let ok = st.pending_offers.len() < PENDING_OFFER_CAP;
                        if ok {
                            st.pending_offers.insert(
                                bundle.offer_id.clone(),
                                PendingOffer {
                                    to_peer_id: [0u8; 20],
                                    pc: Arc::clone(&bundle.pc),
                                    answered: false,
                                    created: std::time::Instant::now(),
                                    remote_slot: Arc::clone(&bundle.remote_slot),
                                },
                            );
                            st.pcs.push(Arc::clone(&bundle.pc));
                        }
                        ok
                    };
                    if accepted {
                        offers.push(WsOffer {
                            offer_id: bundle.offer_id,
                            sdp: bundle.sdp,
                            to_peer_id: [0u8; 20],
                        });
                    } else {
                        let _ = bundle.pc.close().await;
                        break;
                    }
                }
                Err(_) => {
                    let _ = pc.close().await;
                    break;
                }
            }
        }
    }
    offers
}

/// 处理收到的 offer：建立 answer 并沿会话 socket 立即回发
/// （真实服务器对 answer 类 announce 不回响应，不能等待）。
pub(super) async fn ws_handle_offer(ctx: &AnnounceCtx, sess: &mut WsSession, sig: &WsSignal) {
    if !ctx.transports.webrtc {
        return;
    }
    let busy = ctx
        .signals
        .lock()
        .expect("signals 锁")
        .is_busy_with(&sig.peer_id);
    if busy || sig.peer_id == ctx.peer_id {
        return;
    }
    match crate::bt::webrtc::create_data_channel_answer(sig, ctx.rtc_tx.clone()).await {
        Ok((pc, ans)) => {
            {
                let mut st = ctx.signals.lock().expect("signals 锁");
                st.pcs.push(Arc::clone(&pc));
            }
            let body = build_answer_announce_json(&ctx.infohash, &ctx.peer_id, &ans);
            if sess.send_json(&body).await.is_err() {
                ctx.signals
                    .lock()
                    .expect("signals 锁")
                    .mark_peer_fail(&sig.peer_id);
            }
        }
        Err(_) => {
            ctx.signals
                .lock()
                .expect("signals 锁")
                .mark_peer_fail(&sig.peer_id);
        }
    }
}

/// 处理收到的 answer：匹配 pending offer，接受后回填远端真实 peer_id
/// （盲 offer 发出时对端身份未知，answer 推送携带真实 id）。
pub(super) async fn ws_handle_answer(ctx: &AnnounceCtx, sig: &WsSignal) {
    let entry = ctx
        .signals
        .lock()
        .expect("signals 锁")
        .pending_offers
        .remove(&sig.offer_id);
    let Some(po) = entry else { return };
    match crate::bt::webrtc::accept_answer(&po.pc, sig).await {
        Ok(()) => {
            if let Ok(mut slot) = po.remote_slot.lock() {
                *slot = Some(sig.peer_id);
            }
            let mut st = ctx.signals.lock().expect("signals 锁");
            st.pending_offers.insert(
                sig.offer_id.clone(),
                PendingOffer {
                    to_peer_id: sig.peer_id,
                    pc: Arc::clone(&po.pc),
                    answered: true,
                    created: po.created,
                    remote_slot: Arc::clone(&po.remote_slot),
                },
            );
        }
        Err(_) => {
            let pc = Arc::clone(&po.pc);
            tokio::spawn(async move {
                let _ = pc.close().await;
            });
            ctx.signals
                .lock()
                .expect("signals 锁")
                .mark_peer_fail(&sig.peer_id);
        }
    }
}

/// 处理 announce 响应：注册 swarm peers + 处理捎带数组（harness/mini tracker
/// 契约；真实 tracker 的响应不含这些字段，推送走会话循环的 Offer/Answer 分支）。
pub(super) async fn ws_process_response(
    ctx: &AnnounceCtx,
    sess: &mut WsSession,
    resp: &WsAnnounceResponse,
) {
    {
        let mut st = ctx.signals.lock().expect("signals 锁");
        for pid in &resp.peer_ids {
            st.swarm_peers.insert(*pid);
        }
        st.swarm_peers.remove(&ctx.peer_id);
    }
    if !ctx.transports.webrtc {
        return;
    }
    for a in &resp.answers {
        ws_handle_answer(ctx, a).await;
    }
    for sig in &resp.offers_to_answer {
        ws_handle_offer(ctx, sess, sig).await;
    }
}

pub(super) fn spawn_outgoing_peer(
    conn_id: usize,
    p: PeerAddr,
    spec: &crate::engine::BtSpec,
    peer_id: &[u8; 20],
    tx: mpsc::Sender<(usize, PeerIn)>,
    dl_flow: Arc<DlFlow>,
) {
    let meta = spec.meta.clone();
    let pid = *peer_id;
    let connect_timeout = spec.tunables.connect_timeout_secs;
    let handshake_timeout = spec.tunables.handshake_timeout_secs;
    tokio::spawn(async move {
        let addr = format!("{}:{}", p.ip, p.port);
        let sock = match tokio::time::timeout(
            Duration::from_secs(connect_timeout),
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
        run_peer_loop(
            conn_id,
            sock,
            &meta.infohash,
            &pid,
            handshake_timeout,
            true,
            tx,
            dl_flow,
        )
        .await;
    });
}

pub(super) fn spawn_incoming_peer(
    conn_id: usize,
    sock: TcpStream,
    addr: std::net::SocketAddr,
    spec: &crate::engine::BtSpec,
    peer_id: &[u8; 20],
    tx: mpsc::Sender<(usize, PeerIn)>,
    dl_flow: Arc<DlFlow>,
) {
    let meta = spec.meta.clone();
    let pid = *peer_id;
    let hs_timeout = spec.tunables.handshake_timeout_secs;
    tokio::spawn(async move {
        let _ = sock.set_nodelay(true);
        run_peer_loop(
            conn_id,
            sock,
            &meta.infohash,
            &pid,
            hs_timeout,
            false,
            tx,
            dl_flow,
        )
        .await;
        let _ = addr;
    });
}

pub(super) fn announce_url(u: &TrackerUrl) -> String {
    match u {
        TrackerUrl::Http(s) => s.clone(),
        TrackerUrl::Ws(s) => s.clone(),
        TrackerUrl::Udp(s) => s.clone(),
    }
}
