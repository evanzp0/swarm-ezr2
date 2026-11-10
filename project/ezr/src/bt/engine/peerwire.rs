use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

use super::conn::{ConnStat, DlFlow, PeerCandidate, PeerIn, PeerOut, PieceOrderCache};
use super::pieceio::{
    claimed_blocks, issue_requests, on_block, read_upload_blocks, upload_read_plan,
};
use super::{addr_ip, addr_sock, PartialPieces};
use crate::bt::pieces::PieceMap;
use crate::bt::{peer, BtError};
use crate::engine::{BtTunables, EngineShared, Evt, Throttle};

pub(super) const UPLOAD_BLOCK: usize = 16 * 1024;

pub(super) const BAD_PIECE_BAN_THRESHOLD: u32 = 3;

pub(super) const INVALID_REQUEST_BAN_THRESHOLD: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BanKind {
    Leech,
    BadPiece,
    Protocol,
}

pub(super) async fn run_peer_loop<S>(
    conn_id: usize,
    mut sock: S,
    infohash: &[u8; 20],
    peer_id: &[u8; 20],
    handshake_timeout_secs: u64,
    initiator: bool,
    tx: mpsc::Sender<(usize, PeerIn)>,
    dl_flow: Arc<DlFlow>,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (out_tx, mut out_rx) = mpsc::channel::<PeerOut>(64);

    let hs = peer::handshake(
        &mut sock,
        infohash,
        peer_id,
        initiator,
        handshake_timeout_secs,
    )
    .await;
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
            let client = crate::bt::peer_id::client_display(None, &ok.peer_id);
            let ext_metadata = ok.reserved[5] & 0x10 != 0;
            if tx
                .send((
                    conn_id,
                    PeerIn::Connected {
                        client,
                        peer_id: ok.peer_id,
                        out: out_tx,
                        ext_metadata,
                    },
                ))
                .await
                .is_err()
            {
                return;
            }
            peer_pump(conn_id, sock, &mut out_rx, tx, dl_flow).await;
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

pub(super) async fn peer_pump<S>(
    conn_id: usize,
    sock: S,
    out_rx: &mut mpsc::Receiver<PeerOut>,
    tx: mpsc::Sender<(usize, PeerIn)>,
    dl_flow: Arc<DlFlow>,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut rd, mut wr) = tokio::io::split(sock);
    let tx_read = tx.clone();
    let reader = tokio::spawn(async move {
        loop {
            match peer::read_msg(&mut rd).await {
                Ok(m) => {
                    if let peer::PeerMsg::Block { data, .. } = &m {
                        dl_flow.global.acquire(data.len() as u64).await;
                        dl_flow.task.acquire(data.len() as u64).await;
                    }
                    if tx_read.send((conn_id, PeerIn::Msg(m))).await.is_err() {
                        return;
                    }
                }
                Err(e) => {
                    if matches!(e, BtError::Peer(_)) {
                        let _ = tx_read.send((conn_id, PeerIn::ProtocolViolation)).await;
                    } else {
                        let _ = tx_read
                            .send((
                                conn_id,
                                PeerIn::Closed {
                                    reason: e.to_string(),
                                },
                            ))
                            .await;
                    }
                    return;
                }
            }
        }
    });
    loop {
        // recv 是 channel 操作（取消安全），timeout 到期仅代表本窗内无出站帧，
        // 不存在半帧消费问题；读半部仍由独立 reader 任务独占。
        match tokio::time::timeout(
            std::time::Duration::from_secs(PEER_KEEPALIVE_SECS),
            out_rx.recv(),
        )
        .await
        {
            Ok(Some(PeerOut::Send(m))) => {
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
            Ok(Some(PeerOut::Drop)) | Ok(None) => {
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
            Err(_idle) => {
                // 线路空闲超阈值：发 keep-alive（0 长帧）防对端按 ~2 分钟静默超时断连。
                // timeout 每轮重置，仅连续空闲达阈值才触发；被置换连接先收
                // PeerOut::Drop（优先级高于保活），二者不竞争。
                let frame = peer::encode_msg(&peer::PeerMsg::KeepAlive);
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
        }
    }
    graceful_close(wr, reader, Duration::from_secs(GRACE_CLOSE_SECS)).await;
}

pub(super) const GRACE_CLOSE_SECS: u64 = 5;

/// 线路空闲超过该秒数发送 keep-alive（0 长帧）：对端普遍按 ~2 分钟静默断连（BEP 3 惯例），
/// 90s 低于该阈值、高于空闲置换窗（默认 60s）——被置换连接先收 PeerOut::Drop，
/// keep-alive 不与之竞争。做种任务不参与空闲置换（!seeding 门控），
/// 其长闲置连接依赖本保活维持（遗留受限项「ezr 不发 KeepAlive」的修复）。
pub(super) const PEER_KEEPALIVE_SECS: u64 = 90;

pub(super) async fn graceful_close<W>(
    mut wr: W,
    reader: tokio::task::JoinHandle<()>,
    grace: Duration,
) where
    W: AsyncWrite + Unpin,
{
    let _ = wr.shutdown().await;
    drop(wr);
    let mut reader = reader;
    let _ = tokio::time::timeout(grace, &mut reader).await;
    reader.abort();
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_peer_msg(
    conn_id: usize,
    msg: peer::PeerMsg,
    conns: &mut HashMap<usize, ConnStat>,
    pmap: &PieceMap,
    spec: &crate::engine::BtSpec,
    tun: &BtTunables,
    permits: Arc<tokio::sync::Semaphore>,
    ul: &Arc<Throttle>,
    task_ul: &Arc<Throttle>,
    done: &mut HashSet<u32>,
    downloaded: &mut u64,
    files: &mut HashMap<usize, tokio::fs::File>,
    partial: &mut PartialPieces,
    order: &mut PieceOrderCache,
    upload_paused: bool,
) -> Option<BanKind> {
    let st = conns.get_mut(&conn_id)?;
    match msg {
        peer::PeerMsg::Bitfield(bits) => {
            for (i, b) in bits.iter().enumerate() {
                for bit in 0..8u32 {
                    if b & (0x80 >> bit) != 0 {
                        let p = (i as u32) * 8 + bit;
                        if st.has.insert(p) && pmap.covers(p) {
                            order.peer_gained(p);
                        }
                    }
                }
            }
            let want = st.has.iter().any(|p| !done.contains(p) && pmap.covers(*p));
            if want {
                send(conn_id, st, peer::PeerMsg::Interested).await;
            }
        }
        peer::PeerMsg::Have(p) => {
            if st.has.insert(p) && pmap.covers(p) {
                order.peer_gained(p);
            }
            let want = !done.contains(&p) && pmap.covers(p);
            if want {
                send(conn_id, st, peer::PeerMsg::Interested).await;
                if !st.peer_choking {
                    // 按需计算：claimed 只在确实要补请求时构建，
                    // 控制消息（KeepAlive/Request/Cancel/…）零开销直达。
                    let claimed = claimed_blocks(conns, partial);
                    if let Some(st) = conns.get_mut(&conn_id) {
                        issue_requests(conn_id, st, &claimed, pmap, tun, done, partial, order)
                            .await;
                    }
                }
            }
        }
        peer::PeerMsg::Unchoke => {
            st.peer_choking = false;
            let claimed = claimed_blocks(conns, partial);
            if let Some(st) = conns.get_mut(&conn_id) {
                issue_requests(conn_id, st, &claimed, pmap, tun, done, partial, order).await;
            }
        }
        peer::PeerMsg::Choke => {
            st.peer_choking = true;
            st.inflight.clear();
        }
        peer::PeerMsg::Block { index, begin, data } => {
            return on_block(
                conn_id, index, begin, data, conns, pmap, spec, done, downloaded, files, partial,
                order,
            )
            .await;
        }
        peer::PeerMsg::Request {
            index,
            begin,
            length,
        } => {
            let allowed = !st.am_choking && !upload_paused;
            let have = done.contains(&index);
            let valid_len = length > 0 && length as usize <= UPLOAD_BLOCK * 2;
            if allowed && have && valid_len {
                if let Some(plan) =
                    upload_read_plan(pmap, &spec.save_dir, index, begin, length as usize)
                {
                    st.cum_up += u64::from(length);
                    let out = st.peer_out.clone();
                    let ul = ul.clone();
                    let task_ul = task_ul.clone();
                    tokio::spawn(async move {
                        let data = {
                            let _permit = permits.acquire().await;
                            read_upload_blocks(plan, length as usize).await
                        };
                        if let Some(data) = data {
                            ul.acquire(data.len() as u64).await;
                            task_ul.acquire(data.len() as u64).await;
                            let _ = out
                                .send(PeerOut::Send(peer::PeerMsg::Block { index, begin, data }))
                                .await;
                        }
                    });
                } else {
                    return count_invalid_request(st);
                }
            } else if allowed && (!have || !valid_len) {
                return count_invalid_request(st);
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
            if ext_id == 0 {
                if let Some(eh) = peer::parse_ext_handshake(&payload) {
                    if let Some(v) = eh.v {
                        let disp = crate::bt::peer_id::client_display(
                            Some(&v),
                            &st.peer_id.unwrap_or([0u8; 20]),
                        );
                        if st.client != disp {
                            st.client = disp;
                        }
                    }
                }
            } else if ext_id == 1 {
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
        peer::PeerMsg::Unknown(..) => return Some(BanKind::Protocol),
    }
    None
}

pub(super) fn count_invalid_request(st: &mut ConnStat) -> Option<BanKind> {
    st.invalid_requests = st.invalid_requests.saturating_add(1);
    (st.invalid_requests >= INVALID_REQUEST_BAN_THRESHOLD).then_some(BanKind::Protocol)
}

pub(super) async fn ban_and_drop(
    conns: &mut HashMap<usize, ConnStat>,
    candidates: &mut HashMap<(IpAddr, u16), PeerCandidate>,
    order: &mut PieceOrderCache,
    pmap: &PieceMap,
    conn_id: usize,
    shared: &Arc<EngineShared>,
    evt_tx: &mpsc::Sender<Evt>,
    kind: BanKind,
) {
    let Some(st) = conns.remove(&conn_id) else {
        return;
    };
    if let Some(ip) = addr_ip(&st.addr) {
        let reason = match kind {
            BanKind::Leech => "leech",
            BanKind::BadPiece => "bad_piece",
            BanKind::Protocol => "protocol",
        };
        shared.blacklist_add_with(ip, reason);
        let toast = match kind {
            BanKind::Leech => format!("⛔ 已封禁吸血对端 {ip}（只下载不上传）"),
            BanKind::BadPiece => format!("⛔ 已封禁对端 {ip}（连续提供坏块）"),
            BanKind::Protocol => format!("⛔ 已封禁对端 {ip}（协议违规）"),
        };
        let _ = evt_tx.send(Evt::Toast(toast)).await;
    }
    for &p in st
        .has
        .iter()
        .filter(|&p| pmap.covers(*p))
        .collect::<Vec<_>>()
    {
        order.peer_lost(p);
    }
    if let Some(key) = addr_sock(&st.addr) {
        candidates.remove(&key);
    }
    let _ = st.peer_out.send(PeerOut::Drop).await;
}

pub(super) async fn send(_conn_id: usize, st: &mut ConnStat, m: peer::PeerMsg) {
    let _ = st.peer_out.send(PeerOut::Send(m)).await;
}

pub(super) fn assemble_contiguous(entry: &BTreeMap<u32, Vec<u8>>, plen: usize) -> Option<Vec<u8>> {
    let mut cursor = 0usize;
    let mut buf: Vec<u8> = Vec::with_capacity(plen);
    for (off, blk) in entry.iter() {
        if *off as usize != cursor || cursor + blk.len() > plen {
            return None;
        }
        buf.extend_from_slice(blk);
        cursor += blk.len();
    }
    if cursor == plen {
        Some(buf)
    } else {
        None
    }
}
