use std::path::Path;
use std::sync::Arc;

use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::mpsc;

use super::super::error::{classify_reqwest, EngineFailure};
use super::super::ConnView;
use super::finalize::{build_sidecar, failed_ledger, finalize_completed};
use super::http::req_checked;
use super::{emit, Evt, Flow, PausedState, TaskCmd, TaskSpec};
use crate::model::chunk::{block_range, Blocks};
use crate::model::consistency::ServerStamp;
use crate::model::FailKind;

#[allow(clippy::too_many_arguments)]
pub(super) async fn single_stream(
    spec: &TaskSpec,
    shared: &Arc<super::super::EngineShared>,
    resp: reqwest::Response,
    blocks: Blocks,
    multi_ok: bool,
    dl_path: &Path,
    final_path: &Path,
    sc_path: &str,
    name: &str,
    stamp: &ServerStamp,
    cmd_rx: &mut mpsc::Receiver<TaskCmd>,
    evt_tx: &mpsc::Sender<Evt>,
) -> Flow {
    let resumed = blocks.written.first().copied().unwrap_or(0);
    let has_progress = blocks.written.iter().any(|&w| w > 0);
    let single_block = block_range(blocks.total, blocks.piece, 1).is_none();

    if !multi_ok && has_progress {
        return Flow::Failed(
            EngineFailure::network("服务器本次未支持断点续传（Range），保留断点稍后重试"),
            false,
            Some((
                build_sidecar(spec, stamp, &blocks, false),
                sc_path.to_string(),
            )),
        );
    }

    let resume = multi_ok && single_block && resumed > 0;
    if resume && resumed >= blocks.total {
        if std::fs::metadata(dl_path).is_ok_and(|m| m.len() == blocks.total) {
            return finalize_completed(
                spec,
                stamp,
                &blocks,
                sc_path,
                dl_path,
                final_path,
                blocks.total,
                evt_tx,
            )
            .await;
        }
        return Flow::Invalidated;
    }

    if !multi_ok {
        let _ = emit(
            evt_tx,
            Evt::Toast(format!("⚠ 服务器不支持断点续传，单线程下载: {name}")),
        )
        .await;
    }
    let mut resp = if resume {
        drop(resp);
        let client = shared.client_for(spec.proxy_endpoint.as_ref());
        let range = format!("bytes={}-{}", resumed, blocks.total - 1);
        match req_checked(&client, &spec.url, Some(range)).await {
            Ok(r) => r,
            Err(f) => {
                return Flow::Failed(
                    f,
                    false,
                    failed_ledger(spec, stamp, &blocks, multi_ok, resumed, sc_path),
                );
            }
        }
    } else {
        resp
    };
    let mut downloaded: u64 = if resume { resumed } else { 0 };
    let mut file = match tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(!resume)
        .open(dl_path)
        .await
    {
        Ok(f) => f,
        Err(e) => {
            return Flow::Failed(
                EngineFailure::fatal(format!("无法创建目标文件（{e}）")),
                false,
                failed_ledger(spec, stamp, &blocks, multi_ok, resumed, sc_path),
            )
        }
    };
    if resume {
        let _ = file.seek(std::io::SeekFrom::Start(resumed)).await;
    }
    let mut last_emit = std::time::Instant::now();
    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => match cmd {
                Some(TaskCmd::Pause) => {
                    let mut ledger = blocks.clone();
                    if multi_ok {
                        ledger.mark_progress(0, downloaded);
                    }
                    let sidecar = build_sidecar(spec, stamp, &ledger, !multi_ok);
                    return Flow::Paused(PausedState {
                        blocks: ledger,
                        sidecar,
                        sidecar_path: sc_path.to_string(),
                    });
                }
                Some(TaskCmd::Cancel) | None => return Flow::Cancelled,
                Some(TaskCmd::Reconfigure { .. }) => {}
            },
            chunk = resp.chunk() => match chunk {
                Ok(Some(bytes)) => {
                    shared.throttle.acquire(bytes.len() as u64).await;
                    if file.write_all(&bytes).await.is_err() {
                        return Flow::Failed(
                            EngineFailure::fatal("磁盘写入失败"),
                            downloaded > 0,
                            failed_ledger(spec, stamp, &blocks, multi_ok, downloaded, sc_path),
                        );
                    }
                    downloaded += bytes.len() as u64;
                    if last_emit.elapsed() >= std::time::Duration::from_millis(200) {
                        last_emit = std::time::Instant::now();
                        let conns = vec![ConnView {
                            id: 1,
                            block: 0,
                            start: 0,
                            end: blocks.total,
                            done: downloaded,
                        }];
                        let _ = emit(evt_tx, Evt::Progress {
                            id: spec.id,
                            downloaded,
                            conns,
                            chunk_done: 0,
                            done_blocks: Vec::new(),
                            partial_blocks: Vec::new(),
                        }).await;
                    }
                }
                Ok(None) => {
                    if blocks.total > 0 && downloaded != blocks.total {
                        return Flow::Failed(
                            EngineFailure {
                                kind: FailKind::Transient,
                                reason: format!(
                                    "文件大小不符（已接收 {downloaded} B ≠ Content-Length {} B）",
                                    blocks.total
                                ),
                                retry_after: None,
                            },
                            downloaded > 0,
                            failed_ledger(spec, stamp, &blocks, multi_ok, downloaded, sc_path),
                        );
                    }
                    let final_total = if blocks.total == 0 { downloaded } else { blocks.total };
                    let mut ledger = blocks.clone();
                    if multi_ok {
                        ledger.mark_progress(0, downloaded);
                    }
                    return finalize_completed(
                        spec,
                        stamp,
                        &ledger,
                        sc_path,
                        dl_path,
                        final_path,
                        final_total,
                        evt_tx,
                    )
                    .await;
                }
                Err(e) => {
                    return Flow::Failed(
                        classify_reqwest(&e),
                        downloaded > 0,
                        failed_ledger(spec, stamp, &blocks, multi_ok, downloaded, sc_path),
                    );
                }
            },
        }
    }
}
