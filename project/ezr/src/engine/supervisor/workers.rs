use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::watch;

use super::super::error::{classify_reqwest, EngineFailure};
use super::super::ConnView;
use super::finalize::block_span;
use super::http::req_checked;
use crate::model::chunk::Blocks;
use crate::model::config::ProxyEndpoint;

pub(super) async fn stop_workers(
    stop: &std::sync::atomic::AtomicBool,
    stop_tx: &watch::Sender<bool>,
    handles: &mut Vec<(usize, tokio::task::JoinHandle<()>)>,
) {
    stop.store(true, Ordering::Relaxed);
    let _ = stop_tx.send(true);
    for (_, h) in handles.drain(..) {
        let _ = h.await;
    }
}

pub(super) fn prune_finished(handles: &mut Vec<(usize, tokio::task::JoinHandle<()>)>) {
    handles.retain(|(_, h)| !h.is_finished());
}

pub(super) fn fill_workers(
    handles: &mut Vec<(usize, tokio::task::JoinHandle<()>)>,
    deps: &WorkerDeps,
    shared: &Arc<super::super::EngineShared>,
    q: usize,
) {
    let mut live: std::collections::HashSet<usize> = handles.iter().map(|(s, _)| *s).collect();
    let mut next = 1usize;
    while handles.len() < q {
        while live.contains(&next) {
            next += 1;
        }
        handles.push((next, deps.spawn(next, Arc::clone(shared))));
        live.insert(next);
        next += 1;
    }
}

#[derive(Clone)]
pub(super) struct WorkerDeps {
    pub(super) url: String,
    pub(super) file_path: PathBuf,
    pub(super) blocks: Arc<tokio::sync::Mutex<Blocks>>,
    pub(super) lease_next: Arc<AtomicU32>,
    pub(super) requeue: Arc<tokio::sync::Mutex<Vec<u32>>>,
    pub(super) stop_rx: watch::Receiver<bool>,
    pub(super) quota_rx: watch::Receiver<usize>,
    pub(super) ep_rx: watch::Receiver<Arc<Option<ProxyEndpoint>>>,
    pub(super) total_written: Arc<AtomicU64>,
    pub(super) failure: Arc<tokio::sync::Mutex<Option<EngineFailure>>>,
    pub(super) conns: Arc<tokio::sync::Mutex<Vec<ConnView>>>,
}

impl WorkerDeps {
    pub(super) fn spawn(
        &self,
        wid: usize,
        shared: Arc<super::super::EngineShared>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(block_worker(wid, shared, self.clone()))
    }
}

async fn block_worker(wid: usize, shared: Arc<super::super::EngineShared>, deps: WorkerDeps) {
    let WorkerDeps {
        url,
        file_path,
        blocks,
        lease_next,
        requeue,
        mut stop_rx,
        mut quota_rx,
        mut ep_rx,
        total_written,
        failure,
        conns,
    } = deps;
    let mut file = match tokio::fs::OpenOptions::new()
        .write(true)
        .open(&file_path)
        .await
    {
        Ok(f) => f,
        Err(_) => {
            *failure.lock().await = Some(EngineFailure::fatal("无法写入目标文件"));
            return;
        }
    };
    loop {
        if *stop_rx.borrow() {
            return;
        }
        if wid > *quota_rx.borrow_and_update() {
            conns.lock().await.retain(|c| c.id != wid);
            return;
        }
        let lease = {
            let requeued = {
                let mut q = requeue.lock().await;
                let mut found = None;
                while let Some(i) = q.pop() {
                    let done = {
                        let b = blocks.lock().await;
                        b.is_done_block(i)
                    };
                    if !done {
                        found = Some(i);
                        break;
                    }
                }
                found
            };
            if let Some(idx) = requeued {
                let b = blocks.lock().await;
                let (s, e, written) = block_span(&b, idx);
                Some((idx, s, e, written))
            } else {
                let b = blocks.lock().await;
                let y = b.count();
                let mut found = None;
                let mut i = lease_next.load(Ordering::Relaxed);
                while i < y {
                    if !b.is_done_block(i) {
                        found = Some(i);
                        lease_next.store(i + 1, Ordering::Relaxed);
                        break;
                    }
                    i += 1;
                }
                found.map(|idx| {
                    let (s, e, written) = block_span(&b, idx);
                    (idx, s, e, written)
                })
            }
        };
        let Some((idx, start, end, written)) = lease else {
            let mut g = conns.lock().await;
            g.retain(|c| c.id != wid);
            g.push(ConnView {
                id: wid,
                block: 0,
                start: 0,
                end: 0,
                done: 0,
            });
            drop(g);
            return;
        };
        conns.lock().await.retain(|c| c.id != wid);
        conns.lock().await.push(ConnView {
            id: wid,
            block: idx,
            start,
            end,
            done: written,
        });
        let range = format!("bytes={}-{}", start + written, end - 1);
        let ep = ep_rx.borrow_and_update().clone();
        let client = shared.client_for((*ep).as_ref());
        let mut resp = match req_checked(&client, &url, Some(range)).await {
            Ok(r) => r,
            Err(f) => {
                *failure.lock().await = Some(f);
                return;
            }
        };
        let _ = file.seek(std::io::SeekFrom::Start(start + written)).await;
        let mut done_here = written;
        let mut aborted = false;
        let mut quota_aborted = false;
        loop {
            tokio::select! {
                changed = stop_rx.changed() => {
                    if changed.is_err() || *stop_rx.borrow() {
                        aborted = true;
                    }
                }
                _ = quota_rx.changed() => {
                    if wid > *quota_rx.borrow_and_update() {
                        quota_aborted = true;
                        aborted = true;
                    }
                }
                chunk = resp.chunk() => {
                    match chunk {
                        Ok(Some(bytes)) => {
                            shared.throttle.acquire(bytes.len() as u64).await;
                            if file.write_all(&bytes).await.is_err() {
                                *failure.lock().await = Some(EngineFailure::fatal("磁盘写入失败"));
                                aborted = true;
                            } else {
                                done_here += bytes.len() as u64;
                                total_written.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                                if let Some(cv) = conns.lock().await.iter_mut().find(|c| c.id == wid) {
                                    cv.done = done_here;
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            *failure.lock().await = Some(classify_reqwest(&e));
                            aborted = true;
                        }
                    }
                }
            }
            if aborted {
                break;
            }
        }
        blocks.lock().await.mark_progress(idx, done_here);
        if aborted {
            if quota_aborted {
                requeue.lock().await.push(idx);
                conns.lock().await.retain(|c| c.id != wid);
            }
            return;
        }
        if failure.lock().await.is_some() {
            return;
        }
        blocks.lock().await.mark_done(idx);
    }
}
