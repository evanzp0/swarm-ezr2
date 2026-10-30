use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, watch};

use super::super::error::{classify_reqwest, EngineFailure};
use super::super::ConnView;
use super::finalize::{build_sidecar, failed_flow, flush_sidecar};
use super::http::{probe_head, req_with_identity};
use super::single::single_stream;
use super::workers::{fill_workers, prune_finished, stop_workers, WorkerDeps};
use super::{emit, sidecar_path, Evt, Flow, PausedState, TaskCmd, TaskSpec};
use crate::model::chunk::{chunk_total, Blocks, MIN_HTTP_BLOCK_SIZE};
use crate::model::consistency::{self, Consistency, ServerStamp};
use crate::model::namegen::DOWNLOADING_EXT;
use crate::model::sidecar::Sidecar;
use crate::model::FailKind;


pub(super) async fn download(
    spec: &TaskSpec,
    shared: &Arc<super::super::EngineShared>,
    cmd_rx: &mut mpsc::Receiver<TaskCmd>,
    evt_tx: &mpsc::Sender<Evt>,
) -> Flow {
    let probe_client = shared.client_for(spec.proxy_endpoint.as_ref());
    let resp = loop {
        tokio::select! {
            r = req_with_identity(&probe_client, &spec.url, Some("bytes=0-".into())).send() => match r {
                Ok(r) => break r,
                Err(e) => {
                    return Flow::Failed(
                        classify_reqwest(&e),
                        false,
                        spec.sidecar.clone().zip(spec.sidecar_path.clone()),
                    )
                }
            },
            cmd = cmd_rx.recv() => match cmd {
                Some(TaskCmd::Reconfigure { .. }) => {}
                _ => return Flow::Cancelled,
            },
        }
    };
    let head = probe_head(&resp);
    if !(200..=299).contains(&head.status) {
        return Flow::Failed(
            EngineFailure::http(head.status, head.retry_after),
            false,
            spec.sidecar.clone().zip(spec.sidecar_path.clone()),
        );
    }
    let probed_stamp = ServerStamp {
        etag: head.etag.clone(),
        last_modified: head.last_modified.clone(),
        size: (head.total > 0).then_some(head.total),
    };

    let name = if spec.sidecar.is_some() || spec.keep_name {
        spec.name.clone()
    } else {
        let base = super::namegen::derive_name(
            head.cd_name.as_deref(),
            Some(resp.url().as_str()),
            &spec.url,
        );
        super::namegen::dedupe(&base, |n| super::namegen::exists_on_disk(&spec.save_dir, n))
    };

    let sc_path = sidecar_path(&spec.save_dir, &name);
    let sc = spec.sidecar.clone().filter(|_| {
        name == spec.name || spec.sidecar.as_ref().is_some_and(|s| s.task.id == spec.id)
    });
    let blocks = if let Some(sidecar) = &sc {
        if head.total == 0 && sidecar.size > 0 {
            return Flow::Failed(
                EngineFailure::network(
                    "探测响应缺少 Content-Length，暂无法核验断点，保留断点稍后重试",
                ),
                false,
                Some((sidecar.clone(), sc_path.clone())),
            );
        }
        match consistency::check(&sidecar.stamp(), &probed_stamp) {
            Consistency::Valid => {
                let expected_blocks = chunk_total(sidecar.size, sidecar.block_size);
                if expected_blocks == sidecar.block_count && sidecar.size == head.total {
                    Blocks {
                        total: sidecar.size,
                        piece: sidecar.block_size,
                        written: sidecar.blocks.clone(),
                    }
                } else {
                    return Flow::Invalidated;
                }
            }
            Consistency::Invalidated | Consistency::MissingStamp => {
                return Flow::Invalidated;
            }
        }
    } else {
        Blocks::new(head.total, spec.block_size.max(MIN_HTTP_BLOCK_SIZE))
    };

    let total_blocks = blocks.count();
    let multi_ok = head.status == 206 && head.total > 0;
    if !emit(
        evt_tx,
        Evt::Probed {
            id: spec.id,
            name: name.clone(),
            total: head.total,
            resumable: multi_ok,
            etag: head.etag.clone(),
            last_modified: head.last_modified.clone(),
        },
    )
    .await
    {
        return Flow::Cancelled;
    }

    let dir = PathBuf::from(&spec.save_dir);
    let _ = tokio::fs::create_dir_all(&dir).await;
    let dl_path = dir.join(format!("{name}{DOWNLOADING_EXT}"));
    let final_path = dir.join(&name);

    if !multi_ok || total_blocks <= 1 {
        return single_stream(
            spec,
            shared,
            resp,
            blocks,
            multi_ok,
            &dl_path,
            &final_path,
            &sc_path,
            &name,
            &probed_stamp,
            cmd_rx,
            evt_tx,
        )
        .await;
    }

    drop(resp);
    match tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&dl_path)
        .await
    {
        Ok(f) => {
            if let Err(e) = f.set_len(head.total).await {
                return Flow::Failed(
                    EngineFailure::fatal(format!("无法预分配文件（{e}）")),
                    false,
                    sc.clone().map(|s| (s, sc_path.clone())),
                );
            }
        }
        Err(e) => {
            return Flow::Failed(
                EngineFailure::fatal(format!("无法创建目标文件（{e}）")),
                false,
                sc.clone().map(|s| (s, sc_path.clone())),
            )
        }
    }

    let total = head.total;
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (stop_tx, stop_rx) = watch::channel(false);
    let (quota_tx, quota_rx) = watch::channel(spec.concurrency.clamp(1, 64));
    let (ep_tx, ep_rx) = watch::channel(Arc::new(spec.proxy_endpoint.clone()));
    let blocks_shared = Arc::new(tokio::sync::Mutex::new(blocks.clone()));
    let base_downloaded = blocks.downloaded();
    let lease_next = Arc::new(AtomicU32::new(0));
    let requeue = Arc::new(tokio::sync::Mutex::<Vec<u32>>::new(Vec::new()));
    let total_written = Arc::new(AtomicU64::new(0));
    let failure = Arc::new(tokio::sync::Mutex::<Option<EngineFailure>>::new(None));
    let conns = Arc::new(tokio::sync::Mutex::<Vec<ConnView>>::new(Vec::new()));

    let deps = WorkerDeps {
        url: spec.url.clone(),
        file_path: dl_path.clone(),
        blocks: blocks_shared.clone(),
        lease_next: lease_next.clone(),
        requeue: requeue.clone(),
        stop_rx: stop_rx.clone(),
        quota_rx: quota_rx.clone(),
        ep_rx: ep_rx.clone(),
        total_written: total_written.clone(),
        failure: failure.clone(),
        conns: conns.clone(),
    };
    let n_workers = spec.concurrency.clamp(1, 64);
    let mut handles: Vec<(usize, tokio::task::JoinHandle<()>)> = Vec::with_capacity(n_workers);
    for wid in 1..=n_workers {
        handles.push((wid, deps.spawn(wid, Arc::clone(shared))));
    }

    let progress_tick = Duration::from_millis(spec.progress_tick_ms.max(1));
    let sidecar_flush = Duration::from_secs(spec.sidecar_flush_secs.max(1));
    let mut flush_deadline = tokio::time::Instant::now() + sidecar_flush;
    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => match cmd {
                Some(TaskCmd::Reconfigure { concurrency, endpoint }) => {
                    let q = concurrency.clamp(1, 64);
                    let _ = ep_tx.send(Arc::new(endpoint));
                    let _ = quota_tx.send(q);
                    deps.conns.lock().await.retain(|c| c.id <= q);
                    prune_finished(&mut handles);
                    fill_workers(&mut handles, &deps, shared, q);
                }
                Some(TaskCmd::Pause) | Some(TaskCmd::Cancel) => {
                    let cancelled = cmd == Some(TaskCmd::Cancel);
                    stop_workers(&stop, &stop_tx, &mut handles).await;
                    let blocks = blocks_shared.lock().await.clone();
                    if cancelled {
                        return Flow::Cancelled;
                    }
                    let sidecar =
                        build_sidecar(spec, &probed_stamp, &blocks, false);
                    return Flow::Paused(PausedState {
                        blocks,
                        sidecar,
                        sidecar_path: sc_path.clone(),
                    });
                }
                _ => return Flow::Cancelled,
            },
            _ = tokio::time::sleep(progress_tick) => {
                prune_finished(&mut handles);
                let want = *quota_tx.borrow();
                if handles.len() < want {
                    fill_workers(&mut handles, &deps, shared, want);
                }
                if failure.lock().await.is_some() {
                    stop_workers(&stop, &stop_tx, &mut handles).await;
                    let f = failure.lock().await.clone().unwrap_or_else(|| {
                        EngineFailure::network("传输中断")
                    });
                    let blocks = blocks_shared.lock().await.clone();
                    return failed_flow(
                        spec,
                        &probed_stamp,
                        &blocks,
                        f,
                        &total_written,
                        &sc_path,
                    );
                }
                let b = blocks_shared.lock().await;
                let chunk_done = b.completed();
                let done_blocks = b.done_indices();
                let partial_blocks = b.partial_indices();
                let conns_now = conns.lock().await.clone();
                drop(b);
                let downloaded = base_downloaded + total_written.load(Ordering::Relaxed);
                if !emit(evt_tx, Evt::Progress {
                    id: spec.id,
                    downloaded,
                    conns: conns_now,
                    chunk_done,
                    done_blocks,
                    partial_blocks,
                }).await {
                    return Flow::Cancelled;
                }
                if chunk_done == total_blocks && total_blocks > 0 {
                    if downloaded != total {
                        let f = EngineFailure {
                            kind: FailKind::Transient,
                            reason: format!(
                                "文件大小不符（已接收 {downloaded} B ≠ Content-Length {total} B）"
                            ),
                            retry_after: None,
                        };
                        stop_workers(&stop, &stop_tx, &mut handles).await;
                        let blocks = blocks_shared.lock().await.clone();
                        return failed_flow(
                            spec,
                            &probed_stamp,
                            &blocks,
                            f,
                            &total_written,
                            &sc_path,
                        );
                    }
                    stop_workers(&stop, &stop_tx, &mut handles).await;
                    let has_checksum = spec.expected_algo.is_some();
                    if !has_checksum {
                        let _ = tokio::fs::rename(&dl_path, &final_path).await;
                        let _ = Sidecar::remove(&sc_path);
                    } else {
                        flush_sidecar(spec, &probed_stamp, &blocks_shared, &sc_path).await;
                    }
                    let _ = emit(evt_tx, Evt::DownloadDone {
                        id: spec.id,
                        total,
                    }).await;
                    return Flow::Completed;
                }
                if tokio::time::Instant::now() >= flush_deadline {
                    flush_deadline = tokio::time::Instant::now() + sidecar_flush;
                    flush_sidecar(spec, &probed_stamp, &blocks_shared, &sc_path).await;
                }
            }
        }
    }
}
