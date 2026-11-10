use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;

use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use super::conn::{
    new_conn_stat, refill_visit_order, ConnStat, DlFlow, PeerCandidate, PeerIn, PeerOut,
    PieceOrderCache,
};
use super::peerwire::{assemble_contiguous, send, BanKind, BAD_PIECE_BAN_THRESHOLD};
use super::trackpool::spawn_outgoing_peer;
use super::{addr_sock, PartialPieces};
use crate::bt::metainfo::TorrentMeta;
use crate::bt::pieces::PieceMap;
use crate::bt::tracker::PeerAddr;
use crate::bt::webrtc::PeerTransport;
use crate::bt::{peer, BtError, REQUEST_BLOCK_LEN};
use crate::engine::{BtTunables, EngineShared};

pub(super) async fn write_piece_to_files(
    files: &mut HashMap<usize, tokio::fs::File>,
    pmap: &PieceMap,
    index: u32,
    buf: &[u8],
) -> bool {
    use tokio::io::{AsyncSeekExt, SeekFrom};
    for (fi, off_in_file, off_in_piece, len) in pmap.write_plan(index) {
        if let Some(fh) = files.get_mut(&fi) {
            let seek_ok = fh.seek(SeekFrom::Start(off_in_file)).await.is_ok();
            let write_ok = seek_ok
                && fh
                    .write_all(&buf[off_in_piece as usize..(off_in_piece + len) as usize])
                    .await
                    .is_ok();
            let flush_ok = write_ok && fh.flush().await.is_ok();
            if !flush_ok {
                return false;
            }
        }
    }
    true
}

pub(super) async fn restore_sidecar_partial(
    entries: &[crate::model::BtPartialPiece],
    pmap: &PieceMap,
    meta: &TorrentMeta,
    files_handles: &mut HashMap<usize, tokio::fs::File>,
    done: &mut HashSet<u32>,
    partial: &mut PartialPieces,
    downloaded: &mut u64,
) {
    for entry in entries {
        let piece = entry.piece;
        if done.contains(&piece) || !pmap.covers(piece) {
            continue;
        }
        let plen = pmap.piece_len(piece) as usize;
        let mut blocks: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
        let mut next_begin = 0u64;
        for b in &entry.blocks {
            let Some(data) = b.decode_data() else {
                continue;
            };
            let begin = u64::from(b.begin);
            if data.is_empty()
                || begin % u64::from(REQUEST_BLOCK_LEN) != 0
                || begin < next_begin
                || begin + data.len() as u64 > plen as u64
            {
                continue;
            }
            next_begin = begin + data.len() as u64;
            blocks.insert(b.begin, data);
        }
        if blocks.is_empty() {
            continue;
        }
        if let Some(buf) = assemble_contiguous(&blocks, plen) {
            if verify_piece(&buf, meta, piece)
                && write_piece_to_files(files_handles, pmap, piece, &buf).await
            {
                done.insert(piece);
                *downloaded += selected_bytes_of_piece(pmap, piece);
            }
            continue;
        }
        partial.insert(piece, blocks);
    }
}

pub(super) fn claimed_blocks(
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

pub(super) fn endgame_active(
    claimed: &HashSet<(u32, u32)>,
    pmap: &PieceMap,
    done: &HashSet<u32>,
) -> bool {
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

pub(super) async fn issue_requests(
    conn_id: usize,
    st: &mut ConnStat,
    claimed: &HashSet<(u32, u32)>,
    pmap: &PieceMap,
    tun: &BtTunables,
    done: &HashSet<u32>,
    partial: &PartialPieces,
    order: &mut PieceOrderCache,
) {
    let target = st.pipe_target;
    if st.peer_choking || st.inflight.len() >= target {
        return;
    }
    if order.dirty {
        order.rebuild(pmap);
    }
    let visit = refill_visit_order(&st.has, done, partial, &order.order);
    if visit.is_empty() {
        return;
    }
    let mut issued = 0usize;
    for &piece in &visit {
        issued += request_blocks_of_piece(
            conn_id, st, claimed, pmap, tun, partial, piece, false, target,
        )
        .await;
        if st.inflight.len() >= target {
            return;
        }
    }
    if issued == 0 && endgame_active(claimed, pmap, done) {
        for &piece in &visit {
            request_blocks_of_piece(
                conn_id, st, claimed, pmap, tun, partial, piece, true, target,
            )
            .await;
            if st.inflight.len() >= target {
                return;
            }
        }
    }
}

pub(super) async fn request_blocks_of_piece(
    conn_id: usize,
    st: &mut ConnStat,
    claimed: &HashSet<(u32, u32)>,
    pmap: &PieceMap,
    tun: &BtTunables,
    partial: &PartialPieces,
    piece: u32,
    endgame: bool,
    target: usize,
) -> usize {
    let plen = pmap.piece_len(piece) as usize;
    let mut asked = 0usize;
    let mut begin = 0usize;
    while begin < plen && asked < tun.requests_per_piece && st.inflight.len() < target {
        let key = (piece, begin as u32);
        let already_have = partial
            .get(&piece)
            .is_some_and(|m| m.contains_key(&(begin as u32)));
        if st.inflight.contains_key(&key)
            || (!endgame && claimed.contains(&key))
            || (endgame && already_have)
        {
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

pub(super) fn shed_excess_conns(conns: &HashMap<usize, ConnStat>, target: usize) -> Vec<usize> {
    let excess = conns.len().saturating_sub(target.max(1));
    if excess == 0 {
        return Vec::new();
    }
    let mut order: Vec<(usize, usize, u64)> = conns
        .iter()
        .map(|(cid, st)| (*cid, st.inflight.len(), st.cum_down))
        .collect();
    order.sort_by_key(|&(_, inflight_n, cum)| (inflight_n, cum));
    order
        .into_iter()
        .take(excess)
        .map(|(cid, _, _)| cid)
        .collect()
}

pub(super) fn cancel_inflight_not_covered(
    st: &mut ConnStat,
    pmap: &PieceMap,
    cancelled: &mut Vec<(u32, u32)>,
) {
    let drop_keys: Vec<(u32, u32)> = st
        .inflight
        .keys()
        .copied()
        .filter(|(p, _)| !pmap.covers(*p))
        .collect();
    for key in drop_keys {
        if let Some((len, _)) = st.inflight.remove(&key) {
            cancelled.push(key);
            let _ = st.peer_out.try_send(PeerOut::Send(peer::PeerMsg::Cancel {
                index: key.0,
                begin: key.1,
                length: len,
            }));
        }
    }
}

pub(super) fn recompute_downloaded(done: &HashSet<u32>, pmap: &PieceMap) -> u64 {
    done.iter().map(|&p| selected_bytes_of_piece(pmap, p)).sum()
}

pub(super) async fn open_selected_file_handle(
    save_dir: &str,
    f: &crate::model::BtFile,
) -> Result<tokio::fs::File, String> {
    let abs = file_abs_path(save_dir, &f.path);
    if let Some(parent) = std::path::Path::new(&abs).parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let fh = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .read(true)
        .open(&abs)
        .await
        .map_err(|e| e.to_string())?;
    if f.size > 0 {
        let _ = fh.set_len(f.size).await;
    }
    Ok(fh)
}

pub(super) fn fill_peer_slots(
    spec: &crate::engine::BtSpec,
    peer_id: &[u8; 20],
    peer_tx: &mpsc::Sender<(usize, PeerIn)>,
    shared: &EngineShared,
    conns: &mut HashMap<usize, ConnStat>,
    candidates: &mut HashMap<(IpAddr, u16), PeerCandidate>,
    next_conn: &mut usize,
    dl_flow: &Arc<DlFlow>,
) {
    if !spec.tunables.transports.allows(PeerTransport::Tcp) {
        return;
    }
    // 端点级（ip:port）去重：候选 = tracker/PEX 给出的 ip:listen，连接 = 出连
    // ip:listen 或入连 ip:ephemeral。同 IP 不同端口的多个 peer（多客户端一台机/
    // NAT 共享出口）都必须可达；同一客户端实例的出连+入连双通道由握手后
    // peer_id 重复裁决收口（libtorrent duplicate_peer_id 口径），
    // 不再按裸 IP 拒绝——libtorrent peer_list::new_connection 只在
    // 同地址且有活连接时按方向裁决，连接死亡条目即刻可复用，
    // 永不因「同 IP 已有连接」拒绝不同端口的 peer。
    let connected_eps: HashSet<(IpAddr, u16)> = conns
        .values()
        .filter_map(|st| addr_sock(&st.addr))
        .collect();
    let emergency = conns.is_empty();
    let retry_base = if emergency {
        1
    } else {
        spec.tunables.peer_retry_base_secs
    };
    let retry_cap = if emergency {
        1
    } else {
        spec.tunables.peer_retry_cap_secs
    };
    while conns.len() < spec.concurrency.max(1) {
        let Some(key) = candidates
            .iter()
            .filter(|(k, c)| {
                !connected_eps.contains(k)
                    && !shared.is_blacklisted(k.0)
                    && c.cooldown_ok(retry_base, retry_cap)
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
            dl_flow.clone(),
        );
        conns.insert(*next_conn, new_conn_stat(format!("{ip}:{port}")));
        *next_conn += 1;
    }
}

pub(super) async fn apply_choker(
    conns: &mut HashMap<usize, ConnStat>,
    seeding: bool,
    rotate_optimistic: bool,
    upload_slots: usize,
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
        .take(upload_slots)
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
pub(super) async fn on_block(
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
    order: &mut PieceOrderCache,
) -> Option<BanKind> {
    let mut st = match conns.remove(&conn_id) {
        Some(s) => s,
        None => return None,
    };
    st.inflight.remove(&(index, begin));
    if st.pipe_slow_start {
        st.pipe_target = st
            .pipe_target
            .saturating_add(1)
            .min(spec.tunables.peer_pipeline.max(1));
    }
    if !pmap.covers(index) {
        conns.insert(conn_id, st);
        let claimed = claimed_blocks(conns, partial);
        if let Some(st) = conns.get_mut(&conn_id) {
            issue_requests(
                conn_id,
                st,
                &claimed,
                pmap,
                &spec.tunables,
                done,
                partial,
                order,
            )
            .await;
        }
        return None;
    }
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
        if let Some(st) = conns.get_mut(&conn_id) {
            issue_requests(
                conn_id,
                st,
                &claimed,
                pmap,
                &spec.tunables,
                done,
                partial,
                order,
            )
            .await;
        }
        return None;
    }

    let plen = pmap.piece_len(index) as usize;
    let assembled = {
        let entry = partial.entry(index).or_default();
        entry.insert(begin, data);
        assemble_contiguous(entry, plen)
    };

    if let Some(buf) = assembled {
        if verify_piece(&buf, &spec.meta, index) {
            if !write_piece_to_files(files, pmap, index, &buf).await {
                conns.insert(conn_id, st);
                return None;
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
            st.bad_pieces = st.bad_pieces.saturating_add(1);
            if st.bad_pieces >= BAD_PIECE_BAN_THRESHOLD {
                let kind = BanKind::BadPiece;
                conns.insert(conn_id, st);
                let claimed = claimed_blocks(conns, partial);
                if let Some(st) = conns.get_mut(&conn_id) {
                    issue_requests(
                        conn_id,
                        st,
                        &claimed,
                        pmap,
                        &spec.tunables,
                        done,
                        partial,
                        order,
                    )
                    .await;
                }
                return Some(kind);
            }
        }
    }
    conns.insert(conn_id, st);
    let claimed = claimed_blocks(conns, partial);
    if let Some(st) = conns.get_mut(&conn_id) {
        issue_requests(
            conn_id,
            st,
            &claimed,
            pmap,
            &spec.tunables,
            done,
            partial,
            order,
        )
        .await;
    }
    None
}

/// rel 已含 torrent name 首段（单文件 = name；多文件 = name/…，metainfo
/// 解析保证），落盘路径恒为 save_dir/rel——与任务详情展示的
/// target_path = save_dir/name 一致（bug28①：多文件不得再叠一层 name）。
pub(super) fn file_abs_path(save_dir: &str, rel: &str) -> String {
    format!("{save_dir}/{rel}")
}

/// 从盘上读一块 piece 并做 SHA-1 校验（启动重校验用）。
/// 独立成 async fn 以便在 select! 中与 Pause/Cancel 命令竞争，
/// 借用面只覆盖 files/pmap/meta，不与 done/partial 交叉。
pub(super) async fn verify_piece_from_disk(
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

pub(super) async fn read_piece_from_disk(
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

pub(super) fn save_bt_sidecar(path: &str, done: &HashSet<u32>) {
    let sc = crate::model::BtSidecar::build(done.iter().copied());
    let _ = sc.save(path);
}

pub(super) fn save_bt_sidecar_full(path: &str, done: &HashSet<u32>, partial: &PartialPieces) {
    let pieces = partial
        .iter()
        .filter(|(p, _)| !done.contains(p))
        .map(|(piece, blocks)| crate::model::BtPartialPiece {
            piece: *piece,
            blocks: blocks
                .iter()
                .map(|(begin, data)| crate::model::BtPartialBlock::encode_block(*begin, data))
                .collect(),
        })
        .collect();
    let sc = crate::model::BtSidecar::build_full(done.iter().copied(), pieces);
    let _ = sc.save(path);
}

pub(super) fn bitfield_bytes(done: &HashSet<u32>, num_pieces: u32) -> Vec<u8> {
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

pub(super) type UploadSeg = (String, u64, usize, usize);

pub(super) fn upload_read_plan(
    pmap: &PieceMap,
    save_dir: &str,
    piece: u32,
    begin: u32,
    len: usize,
) -> Option<Vec<UploadSeg>> {
    let plen = pmap.piece_len(piece) as usize;
    let begin = begin as usize;
    if begin + len > plen {
        return None;
    }
    let p_start = u64::from(piece) * u64::from(pmap.piece_length);
    let lo = p_start + begin as u64;
    let hi = lo + len as u64;
    let mut plan = Vec::new();
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
        let rel = &pmap.files.get(fi)?.path;
        plan.push((
            file_abs_path(save_dir, rel),
            off_in_file + (o_lo - seg_lo),
            dst,
            n,
        ));
    }
    Some(plan)
}

pub(super) async fn read_upload_blocks(plan: Vec<UploadSeg>, len: usize) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; len];
    let mut filled = 0usize;
    for (path, off, dst, n) in plan {
        let mut fh = tokio::fs::File::open(&path).await.ok()?;
        use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
        fh.seek(SeekFrom::Start(off)).await.ok()?;
        fh.read_exact(&mut buf[dst..dst + n]).await.ok()?;
        filled += n;
    }
    if filled == len {
        Some(buf)
    } else {
        None
    }
}

pub(super) fn verify_piece(buf: &[u8], meta: &TorrentMeta, piece: u32) -> bool {
    meta.pieces.get(piece as usize).is_some_and(|expect| {
        let mut h = Sha1::new();
        h.update(buf);
        h.finalize().as_slice() == expect
    })
}

pub(super) fn selected_bytes_of_piece(pmap: &PieceMap, piece: u32) -> u64 {
    pmap.write_plan(piece).iter().map(|(.., len)| *len).sum()
}

pub(super) fn partial_piece_list(partial: &PartialPieces, done: &HashSet<u32>) -> Vec<u32> {
    // 黄色集合：已收到部分块、尚未验完的 piece（引擎 partial 内存表）。
    let mut v: Vec<u32> = partial
        .keys()
        .filter(|p| !done.contains(p))
        .copied()
        .collect();
    v.sort_unstable();
    v
}

pub(super) fn inflight_piece_list(
    conns: &HashMap<usize, ConnStat>,
    done: &HashSet<u32>,
) -> Vec<u32> {
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

pub(super) fn uploaded_total(conns: &HashMap<usize, ConnStat>) -> u64 {
    conns.values().map(|s| s.cum_up).sum()
}

pub(super) fn all_selected_done(done: &HashSet<u32>, pmap: &PieceMap) -> bool {
    !pmap.covered_pieces().is_empty() && pmap.covered_pieces().iter().all(|p| done.contains(p))
}
