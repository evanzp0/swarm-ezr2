//! supervisor::single — 单流下载降级路径
//!
//! 职责（architect v145 拆分，纯移动）：不支持 Range 或单块计划时的单流
//! 下载（FR-01-12 降级）、206 可续传流型的块内偏移 Range 续传（v1.26
//! 断点保全）、暂停/完成/失败收尾与进度节流上报。

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

/// 单流下载路径（不支持 Range 或单块计划；FR-01-12 降级）。
/// v1.26 断点保全：单流 ≠ 不可续传——服务器支持 Range（206）时单块计划
/// 同样按块内偏移 Range 续传（FR-01-13）；仅当服务器本次未响应 Range 而
/// 台账已有进度时按暂态失败保留断点（不再静默截断重下——操作者实证：
/// 几次连接超时后分块/字节/进度清零）。
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

    // 恢复臂 A：服务器本次未支持 Range 且台账有进度 → 暂态失败保留断点
    // （不截断 .downloading、不作废台账；FR-01-22 作废语义仅限内容确变）
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

    // 恢复臂 B：单块计划 + Range 可用 + 有块内进度 → Range 续传
    let resume = multi_ok && single_block && resumed > 0;
    if resume && resumed >= blocks.total {
        // 全块完成台账（校验等待窗中断等）：直判文件实况收尾（D34 口径）
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
        // 台账与磁盘脱节：作废断点从头重下（FR-01-22 同源语义）
        return Flow::Invalidated;
    }

    // 降级提示仅对真实不可续传（无 Range）流型发（206 单块计划可续传，不误报）
    if !multi_ok {
        let _ = emit(
            evt_tx,
            Evt::Toast(format!("⚠ 服务器不支持断点续传，单线程下载: {name}")),
        )
        .await;
    }
    let mut resp = if resume {
        // 续传需带块内偏移的新请求：探测连接（bytes=0-）丢弃；v1.28（FR-01-14
        // ②③）：续传请求同样从原始 URL 发起并独立跟跳转（不复用探测落点，
        // 失败重试回滚原始 URL 重新跟跳转同口径）
        drop(resp);
        let client = shared.client_for(spec.proxy_endpoint.as_ref());
        let range = format!("bytes={}-{}", resumed, blocks.total - 1);
        match req_checked(&client, &spec.url, Some(range)).await {
            Ok(r) => r,
            // v1.27：续传请求臂失败不得弃账（旧实现 None → 失败事件 0/0，
            // 台账块内进度在 UI 双失真）；按失败台账口径回报块内进度
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
        // 恢复臂保留已写前缀；全新流截断重建
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
        // 续传写位 = 块内已写偏移（FR-01-13；与分块 worker seek 语义一致）
        let _ = file.seek(std::io::SeekFrom::Start(resumed)).await;
    }
    // 单流进度上报节流（FR-01-17/81：不支持续传的任务同样需要真实进度/速度/连接数）
    let mut last_emit = std::time::Instant::now();
    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => match cmd {
                Some(TaskCmd::Pause) => {
                    // v1.26：206 可续传流型折叠块内进度（旧实现恒落全零台账
                    // + 不可续传标记，恢复必从头重下）；无 Range 流型维持
                    // FR-01-12 不可续传口径
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
                // 单流降级路径不支持热调（在途响应绑定探测连接）；修改在
                // 下次重试/续传生效（spec 由任务当前值构建）
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
                    // 单流连接视图：仅 1 个活跃连接（FR-01-12），周期上报进度
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
                            // 单流无逐块记账（块边界即完成点，FR-01-12 口径）
                            done_blocks: Vec::new(),
                            partial_blocks: Vec::new(),
                        }).await;
                    }
                }
                Ok(None) => {
                    // 完成：大小校验（FR-01-52；total 未知时以实际为准）
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
                    // v1.26：可续传流型折叠块内进度（终态台账与收尾口径一致）
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
