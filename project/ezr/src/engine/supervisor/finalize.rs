//! supervisor::finalize — 完成收尾单源与 sidecar 记账
//!
//! 职责（architect v145 拆分，纯移动）：完成收尾（finalize_completed，多块
//! 聚合完成臂/单流全块台账直判臂共用）、sidecar 构建/落盘（build/flush）、
//! 失败臂台账（failed_flow/failed_ledger）与块区间视图（block_span）。

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::mpsc;

use super::super::error::EngineFailure;
use super::{emit, Evt, TaskSpec};
use crate::model::chunk::{block_range, Blocks};
use crate::model::consistency::ServerStamp;
use crate::model::sidecar::{Sidecar, SidecarTask};
use crate::model::Checksum;

/// 完成收尾单源（多块聚合完成臂 / 单流全块台账直判臂共用，PMD CPD 同构收敛）：
/// 无校验值 → 改名清台账直判完成（FR-01-24/20）；有校验值 → 落全块完成
/// 终态台账（校验等待窗中断重启按台账直判收尾，v1.18 修订 A），再发
/// DownloadDone。两臂差异仅台账变量与 total 来源，行为由既有测试锁定。
pub(super) async fn finalize_completed(
    spec: &TaskSpec,
    stamp: &ServerStamp,
    blocks: &Blocks,
    sc_path: &str,
    dl_path: &Path,
    final_path: &Path,
    total: u64,
    evt_tx: &mpsc::Sender<Evt>,
) -> super::Flow {
    if spec.expected_algo.is_none() {
        // 无校验值直接完成：收尾（FR-01-24/20）
        let _ = tokio::fs::rename(dl_path, final_path).await;
        let _ = Sidecar::remove(sc_path);
    } else {
        // 校验等待窗：落全块完成终态台账（与多块完成臂同款），
        // 由 App 校验流程收尾
        let _ = Sidecar::save(&build_sidecar(spec, stamp, blocks, false), sc_path);
    }
    let _ = emit(evt_tx, Evt::DownloadDone { id: spec.id, total }).await;
    super::Flow::Completed
}

/// blocks 快照构建并落盘 sidecar（周期落盘与完成前终态落盘共用同一收口：
/// 加锁 → 全块口径构建 → 解锁后写盘，与两侧原内联序列逐句等价）
pub(super) async fn flush_sidecar(
    spec: &TaskSpec,
    stamp: &ServerStamp,
    blocks_shared: &tokio::sync::Mutex<Blocks>,
    sc_path: &str,
) {
    let b = blocks_shared.lock().await;
    let sidecar = build_sidecar(spec, stamp, &b, false);
    drop(b);
    let _ = Sidecar::save(&sidecar, sc_path);
}

/// 失败臂共用尾段：blocks 快照构建 sidecar，连同失败信息回报 Failed 流
/// （监督循环失败臂与大小校验失败臂两处同构收敛，PMD CPD 66tok）
pub(super) fn failed_flow(
    spec: &TaskSpec,
    stamp: &ServerStamp,
    blocks: &Blocks,
    f: EngineFailure,
    total_written: &AtomicU64,
    sc_path: &str,
) -> super::Flow {
    let sidecar = build_sidecar(spec, stamp, blocks, false);
    super::Flow::Failed(
        f,
        total_written.load(Ordering::Relaxed) > 0,
        Some((sidecar, sc_path.to_string())),
    )
}

/// 单流失败臂的断点台账（206 可续传流型折叠块内进度；无 Range 流型按
/// FR-01-12 不可续传不落账——与暂停臂口径一致）
pub(super) fn failed_ledger(
    spec: &TaskSpec,
    stamp: &ServerStamp,
    blocks: &Blocks,
    multi_ok: bool,
    downloaded: u64,
    sc_path: &str,
) -> Option<(Sidecar, String)> {
    if !multi_ok || downloaded == 0 {
        return None;
    }
    let mut b = blocks.clone();
    b.mark_progress(0, downloaded);
    Some((build_sidecar(spec, stamp, &b, false), sc_path.to_string()))
}

/// 块区间与已写字节视图（block_worker 重领臂与新领臂两处同构收敛，
/// PMD CPD 52tok；`block_range` 无解时区间落 (0, 0)、已写量落 0）
pub(super) fn block_span(b: &Blocks, idx: u32) -> (u64, u64, u64) {
    let (s, e) = block_range(b.total, b.piece, idx).unwrap_or((0, 0));
    let written = b.written.get(idx as usize).copied().unwrap_or(0);
    (s, e, written)
}

/// 构建 sidecar（下载中/暂停/失败共用）
#[allow(clippy::needless_pass_by_value)]
pub(super) fn build_sidecar(
    spec: &TaskSpec,
    stamp: &ServerStamp,
    blocks: &Blocks,
    non_resumable: bool,
) -> Sidecar {
    let expected = spec
        .expected_algo
        .as_ref()
        .zip(spec.expected_value.as_ref())
        .map(|(a, v)| Checksum {
            algo: a,
            value: v.clone(),
        });
    Sidecar::build(
        stamp,
        blocks.total,
        blocks.piece,
        &blocks.written,
        non_resumable,
        expected.as_ref(),
        SidecarTask {
            id: spec.id,
            added_at: spec.added_at,
            save_dir: spec.save_dir.clone(),
            concurrency: spec.concurrency,
            protocol: spec.protocol,
        },
    )
}
