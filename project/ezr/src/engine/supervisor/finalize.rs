use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::mpsc;

use super::super::error::EngineFailure;
use super::{emit, Evt, TaskSpec};
use crate::model::chunk::{block_range, Blocks};
use crate::model::consistency::ServerStamp;
use crate::model::sidecar::{Sidecar, SidecarTask};
use crate::model::Checksum;

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
        let _ = tokio::fs::rename(dl_path, final_path).await;
        let _ = Sidecar::remove(sc_path);
    } else {
        let _ = Sidecar::save(&build_sidecar(spec, stamp, blocks, false), sc_path);
    }
    let _ = emit(evt_tx, Evt::DownloadDone { id: spec.id, total }).await;
    super::Flow::Completed
}

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

pub(super) fn block_span(b: &Blocks, idx: u32) -> (u64, u64, u64) {
    let (s, e) = block_range(b.total, b.piece, idx).unwrap_or((0, 0));
    let written = b.written.get(idx as usize).copied().unwrap_or(0);
    (s, e, written)
}

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
