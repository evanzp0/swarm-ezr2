use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::watch;

use super::super::error::{classify_reqwest, EngineFailure};
use super::super::throttle::Throttle;
use super::super::{ConnActivity, ConnView};
use super::finalize::block_span;
use super::http::req_checked;
use crate::model::chunk::Blocks;
use crate::model::config::ProxyEndpoint;

pub(super) const WBUF_SIZE: usize = 256 * 1024;

const MAX_INPLACE_ATTEMPTS: u32 = 5;

fn inplace_backoff(attempt: u32) -> Duration {
    match attempt {
        0 | 1 => Duration::from_millis(0),
        2 => Duration::from_secs(1),
        3 => Duration::from_secs(2),
        4 => Duration::from_secs(4),
        _ => Duration::from_secs(8),
    }
}

pub(super) struct WBuf {
    buf: Vec<u8>,
    pos: u64,
    flushed: u64,
}

impl WBuf {
    pub(super) fn new() -> Self {
        WBuf {
            buf: Vec::with_capacity(WBUF_SIZE),
            pos: 0,
            flushed: 0,
        }
    }

    pub(super) fn flushed_total(&self) -> u64 {
        self.flushed
    }

    async fn flush(&mut self, file: &mut tokio::fs::File) -> std::io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        file.seek(std::io::SeekFrom::Start(self.pos)).await?;
        let out = std::mem::take(&mut self.buf);
        file.write_all(&out).await?;
        self.flushed = self.pos + out.len() as u64;
        self.buf = Vec::with_capacity(WBUF_SIZE);
        Ok(())
    }

    pub(super) async fn push(
        &mut self,
        at: u64,
        data: &[u8],
        file: &mut tokio::fs::File,
    ) -> std::io::Result<()> {
        if !self.buf.is_empty() && at != self.pos + self.buf.len() as u64 {
            self.flush(file).await?;
        }
        if self.buf.is_empty() {
            self.pos = at;
        }
        self.buf.extend_from_slice(data);
        if self.buf.len() >= WBUF_SIZE {
            self.flush(file).await?;
        }
        Ok(())
    }

    pub(super) async fn flush_now(&mut self, file: &mut tokio::fs::File) -> std::io::Result<()> {
        self.flush(file).await
    }
}

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
        handles.push((next, deps.spawn(next, Arc::clone(shared), None)));
        live.insert(next);
        next += 1;
    }
}

pub(super) type InplaceAttempts = Arc<std::sync::Mutex<std::collections::HashMap<u32, u32>>>;

fn bump_attempt(map: &InplaceAttempts, idx: u32) -> u32 {
    let mut m = map.lock().unwrap();
    let a = m.entry(idx).or_insert(0);
    *a += 1;
    *a
}

fn clear_attempt(map: &InplaceAttempts, idx: u32) {
    map.lock().unwrap().remove(&idx);
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
    pub(super) task_dl: Arc<Throttle>,
    pub(super) speed_rx: watch::Receiver<u64>,
    pub(super) total_written: Arc<AtomicU64>,
    pub(super) failure: Arc<tokio::sync::Mutex<Option<EngineFailure>>>,
    pub(super) conns: Arc<tokio::sync::Mutex<Vec<ConnView>>>,
    pub(super) inplace: InplaceAttempts,
}

impl WorkerDeps {
    pub(super) fn spawn(
        &self,
        wid: usize,
        shared: Arc<super::super::EngineShared>,
        primed: Option<reqwest::Response>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(block_worker(wid, shared, self.clone(), primed))
    }
}

const OPEN_RETRY_WINDOW: std::time::Duration = std::time::Duration::from_millis(1000);
const OPEN_RETRY_POLL: std::time::Duration = std::time::Duration::from_millis(25);

async fn open_target_file(file_path: &std::path::Path) -> std::io::Result<tokio::fs::File> {
    let deadline = tokio::time::Instant::now() + OPEN_RETRY_WINDOW;
    loop {
        match tokio::fs::OpenOptions::new()
            .write(true)
            .open(file_path)
            .await
        {
            Ok(f) => return Ok(f),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if tokio::time::Instant::now() >= deadline {
                    return Err(e);
                }
            }
            Err(e) => return Err(e),
        }
        tokio::time::sleep(OPEN_RETRY_POLL).await;
    }
}

enum Transient {
    Retry,
    Escalate(EngineFailure),
}

fn escalate_or_retry(f: &EngineFailure) -> Transient {
    match f.kind {
        crate::model::FailKind::Transient => Transient::Retry,
        _ => Transient::Escalate(f.clone()),
    }
}

async fn block_worker(
    wid: usize,
    shared: Arc<super::super::EngineShared>,
    deps: WorkerDeps,
    primed: Option<reqwest::Response>,
) {
    let WorkerDeps {
        url,
        file_path,
        blocks,
        lease_next,
        requeue,
        mut stop_rx,
        mut quota_rx,
        mut ep_rx,
        task_dl,
        mut speed_rx,
        total_written,
        failure,
        conns,
        inplace,
    } = deps;
    let mut primed = primed;
    let mut file = match open_target_file(&file_path).await {
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
                activity: ConnActivity::Connecting,
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
            activity: ConnActivity::Connecting,
        });
        let primed_fits = primed.is_some() && start + written == 0;
        let mut resp = if primed_fits {
            primed.take().unwrap()
        } else {
            primed = None;
            let range = format!("bytes={}-{}", start + written, end - 1);
            let ep = ep_rx.borrow_and_update().clone();
            let client = shared.client_for((*ep).as_ref());
            match req_checked(&client, &url, Some(range)).await {
                Ok(r) => r,
                Err(f) => {
                    let wait = match escalate_or_retry(&f) {
                        Transient::Escalate(ef) => {
                            *failure.lock().await = Some(ef);
                            return;
                        }
                        Transient::Retry => {
                            let att = bump_attempt(&inplace, idx);
                            if att > MAX_INPLACE_ATTEMPTS {
                                *failure.lock().await = Some(f);
                                return;
                            }
                            inplace_backoff(att)
                        }
                    };
                    requeue.lock().await.push(idx);
                    tokio::select! {
                        _ = tokio::time::sleep(wait) => {}
                        _ = stop_rx.changed() => {
                            if *stop_rx.borrow() {
                                return;
                            }
                        }
                        _ = quota_rx.changed() => {
                            if wid > *quota_rx.borrow_and_update() {
                                conns.lock().await.retain(|c| c.id != wid);
                                return;
                            }
                        }
                    }
                    continue;
                }
            }
        };
        let _ = file.seek(std::io::SeekFrom::Start(start + written)).await;
        let mut done_here = written;
        let mut aborted = false;
        let mut quota_aborted = false;
        let mut retry_wait: Option<Duration> = None;
        let mut wbuf = WBuf::new();
        let mut flushed_mark = start + written;
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
                _ = speed_rx.changed() => {
                    task_dl.set_rate(*speed_rx.borrow_and_update());
                }
                chunk = resp.chunk() => {
                    match chunk {
                        Ok(Some(bytes)) => {
                            shared.dl_throttle().acquire(bytes.len() as u64).await;
                            task_dl.acquire(bytes.len() as u64).await;
                            let room = end - start - done_here;
                            let take = (bytes.len() as u64).min(room);
                            if wbuf.push(
                                start + done_here,
                                &bytes[..take as usize],
                                &mut file,
                            ).await.is_err() {
                                *failure.lock().await = Some(EngineFailure::fatal("磁盘写入失败"));
                                aborted = true;
                                break;
                            }
                            done_here += take;
                            total_written.fetch_add(take, Ordering::Relaxed);
                            let new_flushed = wbuf.flushed_total();
                            if new_flushed > flushed_mark {
                                flushed_mark = new_flushed;
                                if let Some(cv) = conns.lock().await.iter_mut().find(|c| c.id == wid) {
                                    cv.done = flushed_mark - start;
                                }
                            }
                            if take < bytes.len() as u64 || done_here >= end - start {
                                break;
                            }
                        }
                        Ok(None) => {
                            if done_here < end - start {
                                let att = bump_attempt(&inplace, idx);
                                if att > MAX_INPLACE_ATTEMPTS {
                                    *failure.lock().await = Some(EngineFailure::network("服务器提前结束响应"));
                                    aborted = true;
                                } else {
                                    retry_wait = Some(inplace_backoff(att));
                                }
                            }
                            break;
                        }
                        Err(e) => {
                            let f = classify_reqwest(&e);
                            match escalate_or_retry(&f) {
                                Transient::Escalate(ef) => {
                                    *failure.lock().await = Some(ef);
                                    aborted = true;
                                }
                                Transient::Retry => {
                                    let att = bump_attempt(&inplace, idx);
                                    if att > MAX_INPLACE_ATTEMPTS {
                                        *failure.lock().await = Some(f);
                                        aborted = true;
                                    } else {
                                        retry_wait = Some(inplace_backoff(att));
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
            }
            if aborted {
                break;
            }
        }
        if wbuf.flush_now(&mut file).await.is_err() {
            *failure.lock().await = Some(EngineFailure::fatal("磁盘写入失败"));
            aborted = true;
        }
        if wbuf.flushed_total() > flushed_mark {
            if let Some(cv) = conns.lock().await.iter_mut().find(|c| c.id == wid) {
                cv.done = wbuf.flushed_total() - start;
            }
        }
        blocks.lock().await.mark_progress(idx, done_here);
        if let Some(wait) = retry_wait {
            requeue.lock().await.push(idx);
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                _ = stop_rx.changed() => {
                    if *stop_rx.borrow() {
                        return;
                    }
                }
                _ = quota_rx.changed() => {
                    if wid > *quota_rx.borrow_and_update() {
                        conns.lock().await.retain(|c| c.id != wid);
                        return;
                    }
                }
            }
            continue;
        }
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
        if done_here >= end - start {
            blocks.lock().await.mark_done(idx);
            clear_attempt(&inplace, idx);
        } else {
            requeue.lock().await.push(idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn tmp_file(name: &str) -> (tokio::fs::File, std::path::PathBuf) {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-wbuf");
        let p = dir.join(name);
        let f = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&p)
            .await
            .unwrap();
        (f, p)
    }

    #[tokio::test]
    async fn wbuf_coalesces_contiguous_chunks_into_one_write() {
        let (mut file, p) = tmp_file("a.bin").await;
        let mut w = WBuf::new();
        w.push(0, &[1u8; 100], &mut file).await.unwrap();
        w.push(100, &[2u8; 100], &mut file).await.unwrap();
        assert_eq!(w.flushed_total(), 0, "不足 256 KiB 不落盘");
        w.flush_now(&mut file).await.unwrap();
        file.flush().await.unwrap();
        assert_eq!(w.flushed_total(), 200);
        let meta = std::fs::metadata(&p).unwrap();
        assert_eq!(meta.len(), 200);
        std::fs::remove_dir_all(p.parent().unwrap()).ok();
    }

    #[tokio::test]
    async fn wbuf_discontinuous_push_flushes_then_repositions() {
        let (mut file, p) = tmp_file("b.bin").await;
        let mut w = WBuf::new();
        w.push(0, &[7u8; 50], &mut file).await.unwrap();
        w.push(200, &[9u8; 50], &mut file).await.unwrap();
        assert_eq!(w.flushed_total(), 50, "断点触发先落前段");
        w.flush_now(&mut file).await.unwrap();
        file.flush().await.unwrap();
        assert_eq!(w.flushed_total(), 250);
        let bytes = std::fs::read(&p).unwrap();
        assert_eq!(bytes.len(), 250);
        assert!(bytes[..50].iter().all(|&b| b == 7));
        assert!(bytes[50..200].iter().all(|&b| b == 0), "空洞保持零");
        assert!(bytes[200..].iter().all(|&b| b == 9));
        std::fs::remove_dir_all(p.parent().unwrap()).ok();
    }

    #[tokio::test]
    async fn wbuf_auto_flushes_at_boundary() {
        let (mut file, p) = tmp_file("c.bin").await;
        let mut w = WBuf::new();
        w.push(0, &[1u8; WBUF_SIZE - 10], &mut file).await.unwrap();
        assert_eq!(w.flushed_total(), 0);
        w.push(WBUF_SIZE as u64 - 10, &[2u8; 20], &mut file)
            .await
            .unwrap();
        assert_eq!(
            w.flushed_total(),
            WBUF_SIZE as u64 + 10,
            "满 256 KiB 即落盘（连同越界余量整批写）"
        );
        w.flush_now(&mut file).await.unwrap();
        file.flush().await.unwrap();
        let meta = std::fs::metadata(&p).unwrap();
        assert_eq!(meta.len(), WBUF_SIZE as u64 + 10);
        std::fs::remove_dir_all(p.parent().unwrap()).ok();
    }

    #[test]
    fn inplace_backoff_ladder_matches_d86() {
        assert_eq!(inplace_backoff(1), Duration::from_secs(0), "第 1 次立即");
        assert_eq!(inplace_backoff(2), Duration::from_secs(1));
        assert_eq!(inplace_backoff(3), Duration::from_secs(2));
        assert_eq!(inplace_backoff(4), Duration::from_secs(4));
        assert_eq!(inplace_backoff(5), Duration::from_secs(8));
        assert_eq!(inplace_backoff(9), Duration::from_secs(8), "封顶 8s");
    }

    #[test]
    fn fatal_failure_escalates_transient_retries() {
        let f = EngineFailure::fatal("URL 失效");
        assert!(matches!(escalate_or_retry(&f), Transient::Escalate(_)));
        let t = EngineFailure::network("连接被重置");
        assert!(matches!(escalate_or_retry(&t), Transient::Retry));
        let h = EngineFailure::http(503, None);
        assert!(matches!(escalate_or_retry(&h), Transient::Retry));
        let nf = EngineFailure::http(404, None);
        assert!(matches!(escalate_or_retry(&nf), Transient::Escalate(_)));
    }
}
