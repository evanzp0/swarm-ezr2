use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, watch};

use super::super::error::{classify_reqwest, EngineFailure};
use super::super::throttle::Throttle;
use super::super::ConnView;
use super::http::{probe_head, req_checked, req_with_identity};
use super::planner::{Claim, Planner, MIN_PREEMPT};
use super::single::single_stream;
use super::writer::Writer;
use super::{emit, Flow, PausedState, TaskCmd, TaskSpec, Evt};
use crate::model::chunk::{Blocks, MIN_HTTP_BLOCK_SIZE};
use crate::model::config::ProxyEndpoint;
use crate::model::consistency::{self, Consistency, ServerStamp};
use crate::model::namegen::DOWNLOADING_EXT;
use crate::model::sidecar::Sidecar;
use crate::model::{Checksum, FailKind};
use crate::model::sidecar::SidecarTask;

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

fn gaps_of(segments: &[(u64, u64)], total: u64) -> Vec<(u64, u64)> {
    let mut sorted: Vec<(u64, u64)> = segments
        .iter()
        .filter(|(s, c)| c > s)
        .copied()
        .collect();
    sorted.sort_by_key(|(s, _)| *s);
    let mut gaps = Vec::new();
    let mut pos = 0u64;
    for (s, c) in sorted {
        if s > pos {
            gaps.push((pos, s.min(total)));
        }
        pos = pos.max(c);
        if pos >= total {
            break;
        }
    }
    if pos < total {
        gaps.push((pos, total));
    }
    gaps.retain(|(a, b)| b > a);
    gaps
}

struct DynDeps {
    url: String,
    name: String,
    total: u64,
    resume: bool,
    primed_start: u64,
    planner: Arc<tokio::sync::Mutex<Planner>>,
    writer: Arc<Writer>,
    stop_rx: watch::Receiver<bool>,
    quota_rx: watch::Receiver<usize>,
    ep_rx: watch::Receiver<Arc<Option<ProxyEndpoint>>>,
    task_dl: Arc<Throttle>,
    speed_rx: watch::Receiver<u64>,
    failure: Arc<tokio::sync::Mutex<Option<EngineFailure>>>,
    conns: Arc<tokio::sync::Mutex<Vec<ConnView>>>,
    hold_cap: u64,
    range_ignored: Arc<AtomicBool>,
}

pub(super) type PrimedResponse = Option<(reqwest::Response, Option<bytes::Bytes>)>;

impl DynDeps {
    fn spawn(
        &self,
        wid: usize,
        shared: Arc<super::super::EngineShared>,
        primed: PrimedResponse,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(dynamic_worker(wid, shared, self.clone(), primed))
    }
}

impl Clone for DynDeps {
    fn clone(&self) -> Self {
        DynDeps {
            url: self.url.clone(),
            name: self.name.clone(),
            total: self.total,
            resume: self.resume,
            primed_start: self.primed_start,
            planner: Arc::clone(&self.planner),
            writer: Arc::clone(&self.writer),
            stop_rx: self.stop_rx.clone(),
            quota_rx: self.quota_rx.clone(),
            ep_rx: self.ep_rx.clone(),
            task_dl: Arc::clone(&self.task_dl),
            speed_rx: self.speed_rx.clone(),
            failure: Arc::clone(&self.failure),
            conns: Arc::clone(&self.conns),
            hold_cap: self.hold_cap,
            range_ignored: Arc::clone(&self.range_ignored),
        }
    }
}

async fn dynamic_worker(
    wid: usize,
    shared: Arc<super::super::EngineShared>,
    deps: DynDeps,
    primed: PrimedResponse,
) {
    let DynDeps {
        url,
        total,
        resume,
        primed_start,
        planner,
        writer,
        mut stop_rx,
        mut quota_rx,
        mut ep_rx,
        task_dl,
        mut speed_rx,
        failure,
        conns,
        hold_cap,
        range_ignored,
        ..
    } = deps;
    let mut primed = primed;
    let mut inplace: u32 = 0;
    let mut after: Option<u64> = None;
    loop {
        if *stop_rx.borrow() {
            planner.lock().await.release(wid);
            conns.lock().await.retain(|c| c.id != wid);
            return;
        }
        if wid > *quota_rx.borrow_and_update() {
            planner.lock().await.release(wid);
            conns.lock().await.retain(|c| c.id != wid);
            return;
        }
        let assignment = planner.lock().await.assign(wid, after);
        let Some(a) = assignment else {
            conns.lock().await.retain(|c| c.id != wid);
            return;
        };
        after = None;
        let mut cur_start = a.from;
        {
            let mut g = conns.lock().await;
            g.retain(|c| c.id != wid);
            g.push(ConnView {
                id: wid,
                block: 0,
                start: a.from,
                end: total,
                done: 0,
            });
        }
        let mut primed_chunk: Option<bytes::Bytes> = None;
        let mut resp = if primed.is_some() && a.from == primed_start {
            let (r, first) = primed.take().unwrap();
            primed_chunk = first;
            r
        } else {
            primed = None;
            let range = format!("bytes={}-", a.from);
            let ep = ep_rx.borrow_and_update().clone();
            let client = shared.client_for((*ep).as_ref());
            match req_checked(&client, &url, Some(range)).await {
                Ok(r) => {
                    if r.status().as_u16() == 200 {
                        range_ignored.store(true, Ordering::Relaxed);
                        planner.lock().await.release(wid);
                        if resume {
                            *failure.lock().await = Some(EngineFailure::network(
                                "服务器本次未支持断点续传（Range），保留断点稍后重试",
                            ));
                        }
                        conns.lock().await.retain(|c| c.id != wid);
                        return;
                    }
                    r
                }
                Err(f) => {
                    let wait = match f.kind {
                        FailKind::Transient => {
                            inplace += 1;
                            if inplace > MAX_INPLACE_ATTEMPTS {
                                *failure.lock().await = Some(f);
                                return;
                            }
                            inplace_backoff(inplace)
                        }
                        _ => {
                            *failure.lock().await = Some(f);
                            return;
                        }
                    };
                    planner.lock().await.release(wid);
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
        let mut at = a.from;
        let mut aborted = false;
        let mut quota_aborted = false;
        let mut retry_wait: Option<Duration> = None;
        let mut clean_rotation = false;
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
                chunk = async {
                    match primed_chunk.take() {
                        Some(b) => Ok(Some(b)),
                        None => resp.chunk().await,
                    }
                } => {
                    match chunk {
                        Ok(Some(bytes)) => {
                            shared.dl_throttle().acquire(bytes.len() as u64).await;
                            task_dl.acquire(bytes.len() as u64).await;
                            let claim = planner.lock().await.claim(wid, at, bytes.len() as u64);
                            match claim {
                                Claim::Granted { n, seg_end } => {
                                    if n > 0 {
                                        if writer.push(at, &bytes[..n as usize]).is_err() {
                                            *failure.lock().await = Some(EngineFailure::fatal("磁盘写入失败"));
                                            aborted = true;
                                            break;
                                        }
                                        if let Err(e) = writer.step(hold_cap).await {
                                            *failure.lock().await =
                                                Some(EngineFailure::fatal(format!("磁盘写入失败（{e}）")));
                                            aborted = true;
                                            break;
                                        }
                                        at += n;
                                        if let Some(cv) = conns.lock().await.iter_mut().find(|c| c.id == wid) {
                                            cv.start = cur_start;
                                            cv.end = seg_end;
                                            cv.done = at - cur_start;
                                        }
                                    }
                                    let exhausted = planner.lock().await.commit(wid, at);
                                    if let Some(end) = exhausted {
                                        match planner.lock().await.absorb_next(wid, end) {
                                            Some(new_end) => {
                                                cur_start = end;
                                                if let Some(cv) = conns.lock().await.iter_mut().find(|c| c.id == wid) {
                                                    cv.start = cur_start;
                                                    cv.end = new_end;
                                                    cv.done = 0;
                                                }
                                            }
                                            None => {
                                                after = Some(end);
                                                clean_rotation = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                                Claim::Lost => {
                                    clean_rotation = true;
                                    break;
                                }
                            }
                            if at >= total {
                                clean_rotation = true;
                                break;
                            }
                        }
                        Ok(None) => {
                            planner.lock().await.release(wid);
                            inplace += 1;
                            if inplace > MAX_INPLACE_ATTEMPTS {
                                *failure.lock().await = Some(EngineFailure::network("服务器提前结束响应"));
                                aborted = true;
                            } else {
                                retry_wait = Some(inplace_backoff(inplace));
                            }
                            break;
                        }
                        Err(e) => {
                            let f = classify_reqwest(&e);
                            match f.kind {
                                FailKind::Transient => {
                                    planner.lock().await.release(wid);
                                    inplace += 1;
                                    if inplace > MAX_INPLACE_ATTEMPTS {
                                        *failure.lock().await = Some(f);
                                        aborted = true;
                                    } else {
                                        retry_wait = Some(inplace_backoff(inplace));
                                    }
                                }
                                _ => {
                                    *failure.lock().await = Some(f);
                                    aborted = true;
                                }
                            }
                            break;
                        }
                    }
                }
            }
            if aborted || retry_wait.is_some() || clean_rotation {
                break;
            }
        }
        if let Some(wait) = retry_wait {
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
            planner.lock().await.release(wid);
            if quota_aborted {
                conns.lock().await.retain(|c| c.id != wid);
            }
            return;
        }
        if failure.lock().await.is_some() {
            planner.lock().await.release(wid);
            return;
        }
        if clean_rotation {
            inplace = 0;
        }
    }
}

fn flushed_complete(writer: &Writer, base_flushed: u64, total: u64) -> bool {
    writer.global_flushed_prefix() >= total
        || base_flushed + writer.total_flushed() == total
}

fn expected_checksum(spec: &TaskSpec) -> Option<Checksum> {
    spec.expected_algo
        .as_ref()
        .zip(spec.expected_value.as_ref())
        .map(|(a, v)| Checksum {
            algo: a,
            value: v.clone(),
        })
}

fn sidecar_task(spec: &TaskSpec) -> SidecarTask {
    SidecarTask {
        id: spec.id,
        added_at: spec.added_at,
        save_dir: spec.save_dir.clone(),
        concurrency: spec.concurrency,
        protocol: spec.protocol,
    }
}

fn dynamic_sidecar_snapshot(
    spec: &TaskSpec,
    stamp: &ServerStamp,
    planner: &Planner,
    writer: &Writer,
    total: u64,
) -> Sidecar {
    let segs: Vec<(u64, u64)> = planner
        .sidecar_segments()
        .into_iter()
        .map(|(s, c)| (s, writer.flushed_extent_from(s).min(c)))
        .filter(|(s, c)| c > s)
        .collect();
    Sidecar::build_dynamic(
        stamp,
        total,
        &segs,
        expected_checksum(spec).as_ref(),
        sidecar_task(spec),
    )
}

async fn stop_dyn_workers(
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

pub(super) async fn download_dynamic(
    spec: &TaskSpec,
    shared: &Arc<super::super::EngineShared>,
    cmd_rx: &mut mpsc::Receiver<TaskCmd>,
    evt_tx: &mpsc::Sender<Evt>,
) -> Flow {
    let probed_sidecar = spec.sidecar.clone().zip(spec.sidecar_path.clone());
    let (resume, _gaps, resume_from) = match &probed_sidecar {
        Some((sidecar, _)) if sidecar.is_dynamic() => {
            let gaps = gaps_of(&sidecar.resume_segments(), sidecar.size);
            let first = gaps.first().map(|g| g.0);
            (true, gaps, first)
        }
        _ => (false, Vec::new(), None),
    };

    let probe_client = shared.client_for(spec.proxy_endpoint.as_ref());
    let range_req = resume_from.map(|from| format!("bytes={from}-"));
    let ttfb_start = Instant::now();
    let mut resp = loop {
        tokio::select! {
            r = req_with_identity(&probe_client, &spec.url, range_req.clone()).send() => match r {
                Ok(r) => break r,
                Err(e) => {
                    return Flow::Failed(classify_reqwest(&e), false, probed_sidecar);
                }
            },
            cmd = cmd_rx.recv() => match cmd {
                Some(TaskCmd::Reconfigure { .. }) => {}
                _ => return Flow::Cancelled,
            },
        }
    };
    let head = probe_head(&resp);
    let first_chunk = match resp.chunk().await {
        Ok(Some(b)) => Some(b),
        Ok(None) => None,
        Err(e) => {
            return Flow::Failed(classify_reqwest(&e), false, probed_sidecar);
        }
    };
    let ttfb = ttfb_start.elapsed();
    if !(200..=299).contains(&head.status) {
        return Flow::Failed(
            EngineFailure::http(head.status, head.retry_after),
            false,
            probed_sidecar,
        );
    }
    let probed_stamp = ServerStamp {
        etag: head.etag.clone(),
        last_modified: head.last_modified.clone(),
        size: (head.total > 0).then_some(head.total),
    };

    if resume {
        if head.status == 200 {
            return Flow::Failed(
                EngineFailure::network("服务器本次未支持断点续传（Range），保留断点稍后重试"),
                false,
                probed_sidecar,
            );
        }
        if let Some((sidecar, _)) = &probed_sidecar {
            if head.total == 0 && sidecar.size > 0 {
                return Flow::Failed(
                    EngineFailure::network(
                        "探测响应缺少 Content-Length，暂无法核验断点，保留断点稍后重试",
                    ),
                    false,
                    Some((sidecar.clone(), spec.sidecar_path.clone().unwrap_or_default())),
                );
            }
            match consistency::check(&sidecar.stamp(), &probed_stamp) {
                Consistency::Valid => {
                    if sidecar.size != head.total {
                        return if spec.resume_orphan {
                            Flow::OrphanRedirect
                        } else {
                            Flow::Invalidated
                        };
                    }
                }
                Consistency::Invalidated | Consistency::MissingStamp => {
                    return if spec.resume_orphan {
                        Flow::OrphanRedirect
                    } else {
                        Flow::Invalidated
                    };
                }
            }
        }
    }

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

    let sc_path = super::sidecar_path(&spec.save_dir, &name);
    let sc = spec.sidecar.clone().filter(|_| {
        name == spec.name || spec.sidecar.as_ref().is_some_and(|s| s.task.id == spec.id)
    });
    let total = head.total;

    if !emit(
        evt_tx,
        Evt::Probed {
            id: spec.id,
            name: name.clone(),
            total,
            resumable: total > 0,
            etag: head.etag.clone(),
            last_modified: head.last_modified.clone(),
            dynamic: true,
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

    if total == 0 {
        let blocks = Blocks::new(0, spec.block_size.max(MIN_HTTP_BLOCK_SIZE));
        return single_stream(
            spec,
            shared,
            resp,
            blocks,
            false,
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

    let std_file = match tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&dl_path)
        .await
    {
        Ok(f) => f.into_std().await,
        Err(e) => {
            return Flow::Failed(
                EngineFailure::fatal(format!("无法创建目标文件（{e}）")),
                false,
                sc.clone().map(|s| (s, sc_path.clone())),
            )
        }
    };
    if let Err(e) = super::sparse::preallocate(&std_file, &dl_path, total) {
        return Flow::Failed(
            EngineFailure::fatal(format!("无法预分配文件（{e}）")),
            false,
            sc.clone().map(|s| (s, sc_path.clone())),
        );
    }
    let writer = match Writer::open(&dl_path) {
        Ok(w) => Arc::new(w),
        Err(e) => {
            return Flow::Failed(
                EngineFailure::fatal(format!("无法写入目标文件（{e}）")),
                false,
                sc.clone().map(|s| (s, sc_path.clone())),
            )
        }
    };

    let preempt_after = std::cmp::max(
        Duration::from_secs_f64(ttfb.as_secs_f64() * 2.0),
        MIN_PREEMPT,
    );
    let min_split = spec.min_split.max(1);
    let planner = match sc.as_ref() {
        Some(sidecar) if sidecar.is_dynamic() => {
            let gaps = gaps_of(&sidecar.resume_segments(), sidecar.size);
            Planner::from_gaps(total, &gaps, min_split, preempt_after)
        }
        _ => Planner::new(total, min_split, preempt_after),
    };
    let base_flushed = sc
        .as_ref()
        .filter(|s| s.is_dynamic())
        .map(|s| s.downloaded)
        .unwrap_or(0);
    let flushed_total = || base_flushed + writer.total_flushed();
    if planner.completed_bytes() >= total {
        drop(resp);
        if let Err(e) = writer.finish().await {
            return Flow::Failed(
                EngineFailure::fatal(format!("磁盘写入失败（{e}）")),
                base_flushed > 0,
                sc.clone().map(|s| (s, sc_path.clone())),
            );
        }
        if !flushed_complete(&writer, base_flushed, total) {
            return Flow::Failed(
                EngineFailure {
                    kind: FailKind::Transient,
                    reason: format!(
                        "文件大小不符（已落盘 {} B ≠ Content-Length {total} B）",
                        flushed_total()
                    ),
                    retry_after: None,
                },
                base_flushed > 0,
                sc.clone().map(|s| (s, sc_path.clone())),
            );
        }
        let has_checksum = spec.expected_algo.is_some();
        if !has_checksum {
            let _ = tokio::fs::rename(&dl_path, &final_path).await;
            let _ = Sidecar::remove(&sc_path);
        } else {
            let sidecar = dynamic_sidecar_snapshot(spec, &probed_stamp, &planner, &writer, total);
            let _ = Sidecar::save(&sidecar, &sc_path);
        }
        let _ = emit(evt_tx, Evt::DownloadDone { id: spec.id, total }).await;
        return Flow::Completed;
    }

    let stop = Arc::new(AtomicBool::new(false));
    let (stop_tx, stop_rx) = watch::channel(false);
    let n_workers = spec.concurrency.clamp(1, 128);
    let (quota_tx, quota_rx) = watch::channel(n_workers);
    let (ep_tx, ep_rx) = watch::channel(Arc::new(spec.proxy_endpoint.clone()));
    let (speed_tx, speed_rx) = watch::channel(spec.task_max_download_speed);
    let task_dl = Arc::new(Throttle::with_params(
        spec.task_max_download_speed,
        shared.throttle_burst_secs(),
        shared.throttle_sleep_ms(),
    ));
    let planner = Arc::new(tokio::sync::Mutex::new(planner));
    let failure = Arc::new(tokio::sync::Mutex::<Option<EngineFailure>>::new(None));
    let conns = Arc::new(tokio::sync::Mutex::<Vec<ConnView>>::new(Vec::new()));
    let range_ignored = Arc::new(AtomicBool::new(false));
    let deps = DynDeps {
        url: spec.url.clone(),
        name: name.clone(),
        total,
        resume: sc.as_ref().is_some_and(Sidecar::is_dynamic),
        primed_start: resume_from.unwrap_or(0),
        planner: Arc::clone(&planner),
        writer: Arc::clone(&writer),
        stop_rx: stop_rx.clone(),
        quota_rx: quota_rx.clone(),
        ep_rx: ep_rx.clone(),
        task_dl,
        speed_rx: speed_rx.clone(),
        failure: Arc::clone(&failure),
        conns: conns.clone(),
        hold_cap: super::writer::HOLD_MAX / n_workers.max(1) as u64,
        range_ignored: Arc::clone(&range_ignored),
    };

    let mut primed_resp: PrimedResponse = Some((resp, first_chunk));
    let mut handles: Vec<(usize, tokio::task::JoinHandle<()>)> = Vec::with_capacity(n_workers);
    for wid in 1..=n_workers {
        let primed = if wid == 1 { primed_resp.take() } else { None };
        handles.push((wid, deps.spawn(wid, Arc::clone(shared), primed)));
    }

    let progress_tick = Duration::from_millis(spec.progress_tick_ms.max(1));
    let sidecar_flush = Duration::from_secs(spec.sidecar_flush_secs.max(1));
    let mut flush_deadline = tokio::time::Instant::now() + sidecar_flush;
    let mut last_sync = Instant::now();
    let mut last_sync_flushed = 0u64;
    let mut range_toast_done = false;
    let mut rescue_done = false;
    let preempt_rescue = preempt_after;
    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => match cmd {
                Some(TaskCmd::Reconfigure { concurrency, endpoint, max_download_speed }) => {
                    let q = concurrency.clamp(1, 128);
                    let _ = ep_tx.send(Arc::new(endpoint));
                    let _ = quota_tx.send(q);
                    let _ = speed_tx.send(max_download_speed);
                    deps.conns.lock().await.retain(|c| c.id <= q);
                    handles.retain(|(_, h)| !h.is_finished());
                    while handles.len() < q {
                        let next = (1..=q).find(|i| !handles.iter().any(|(s, _)| s == i)).unwrap_or(q + 1);
                        handles.push((next, deps.spawn(next, Arc::clone(shared), None)));
                    }
                }
                Some(TaskCmd::Pause) | Some(TaskCmd::Cancel) => {
                    let cancelled = cmd == Some(TaskCmd::Cancel);
                    stop_dyn_workers(&stop, &stop_tx, &mut handles).await;
                    let finish_res = writer.finish().await;
                    if cancelled {
                        return Flow::Cancelled;
                    }
                    if let Err(e) = finish_res {
                        let planner_now = planner.lock().await;
                        let sidecar =
                            dynamic_sidecar_snapshot(spec, &probed_stamp, &planner_now, &writer, total);
                        drop(planner_now);
                        return Flow::Failed(
                            EngineFailure::fatal(format!("磁盘写入失败（{e}）")),
                            writer.total_pushed() > 0,
                            Some((sidecar, sc_path.clone())),
                        );
                    }
                    let planner_now = planner.lock().await;
                    let sidecar =
                        dynamic_sidecar_snapshot(spec, &probed_stamp, &planner_now, &writer, total);
                    drop(planner_now);
                    return Flow::Paused(PausedState {
                        blocks: Blocks::new(0, 1),
                        downloaded: flushed_total(),
                        sidecar,
                        sidecar_path: sc_path.clone(),
                    });
                }
                _ => return Flow::Cancelled,
            },
            _ = tokio::time::sleep(progress_tick) => {
                handles.retain(|(_, h)| !h.is_finished());
                let want = *quota_tx.borrow();
                if handles.len() < want {
                    for next in 1..=want {
                        if handles.len() >= want { break; }
                        if !handles.iter().any(|(s, _)| *s == next) {
                            handles.push((next, deps.spawn(next, Arc::clone(shared), None)));
                        }
                    }
                }
                if !range_toast_done && range_ignored.load(Ordering::Relaxed) {
                    range_toast_done = true;
                    let _ = emit(evt_tx, Evt::Toast(format!(
                        "⚠ 服务器不支持断点续传，单线程下载: {name}"
                    ))).await;
                }
                if !rescue_done && !deps.resume && range_ignored.load(Ordering::Relaxed) {
                    rescue_done = true;
                    stop_dyn_workers(&stop, &stop_tx, &mut handles).await;
                    let _ = writer.finish().await;
                    stop.store(false, Ordering::Relaxed);
                    let _ = stop_tx.send(false);
                    *planner.lock().await = Planner::new(total, min_split, preempt_rescue);
                    let _ = quota_tx.send(1);
                    let rescue_client = shared.client_for(spec.proxy_endpoint.as_ref());
                    let rescue_resp = match req_with_identity(&rescue_client, &spec.url, None).send().await {
                        Ok(r) if r.status().is_success() => {
                            let mut r = r;
                            let rescue_first = match r.chunk().await {
                                Ok(b) => b,
                                Err(e) => {
                                    return Flow::Failed(
                                        classify_reqwest(&e),
                                        writer.total_pushed() > 0,
                                        sc.clone().map(|s| (s, sc_path.clone())),
                                    );
                                }
                            };
                            Some((r, rescue_first))
                        }
                        Ok(r) => {
                            let st = r.status().as_u16();
                            let ra = super::super::error::parse_retry_after_header(
                                r.headers().get("retry-after").and_then(|v| v.to_str().ok()),
                            );
                            let _ = r.bytes().await;
                            return Flow::Failed(
                                EngineFailure::http(st, ra),
                                writer.total_pushed() > 0,
                                sc.clone().map(|s| (s, sc_path.clone())),
                            );
                        }
                        Err(e) => {
                            return Flow::Failed(
                                classify_reqwest(&e),
                                writer.total_pushed() > 0,
                                sc.clone().map(|s| (s, sc_path.clone())),
                            );
                        }
                    };
                    handles.clear();
                    handles.push((1, deps.spawn(1, Arc::clone(shared), rescue_resp)));
                }
                if failure.lock().await.is_some() {
                    stop_dyn_workers(&stop, &stop_tx, &mut handles).await;
                    let _ = writer.finish().await;
                    let f = failure.lock().await.clone().unwrap_or_else(|| {
                        EngineFailure::network("传输中断")
                    });
                    let planner_now = planner.lock().await;
                    let sidecar =
                        dynamic_sidecar_snapshot(spec, &probed_stamp, &planner_now, &writer, total);
                    drop(planner_now);
                    return Flow::Failed(
                        f,
                        base_flushed + writer.total_pushed() > 0,
                        Some((sidecar, sc_path.clone())),
                    );
                }
                let completed = planner.lock().await.completed_bytes();
                let conns_now = conns.lock().await.clone();
                let downloaded = completed;
                if !emit(evt_tx, Evt::Progress {
                    id: spec.id,
                    downloaded,
                    conns: conns_now,
                    chunk_done: 0,
                    done_blocks: Vec::new(),
                    partial_blocks: Vec::new(),
                }).await {
                    return Flow::Cancelled;
                }
                if completed >= total && total > 0 {
                    stop_dyn_workers(&stop, &stop_tx, &mut handles).await;
                    if let Err(e) = writer.finish().await {
                        let planner_now = planner.lock().await;
                        let sidecar =
                            dynamic_sidecar_snapshot(spec, &probed_stamp, &planner_now, &writer, total);
                        drop(planner_now);
                        return Flow::Failed(
                            EngineFailure::fatal(format!("磁盘写入失败（{e}）")),
                            true,
                            Some((sidecar, sc_path.clone())),
                        );
                    }
                    let flushed = flushed_total();
                    if !flushed_complete(&writer, base_flushed, total) {
                        let f = EngineFailure {
                            kind: FailKind::Transient,
                            reason: format!(
                                "文件大小不符（已落盘 {flushed} B ≠ Content-Length {total} B）"
                            ),
                            retry_after: None,
                        };
                        let planner_now = planner.lock().await;
                        let sidecar =
                            dynamic_sidecar_snapshot(spec, &probed_stamp, &planner_now, &writer, total);
                        drop(planner_now);
                        return Flow::Failed(f, true, Some((sidecar, sc_path.clone())));
                    }
                    let has_checksum = spec.expected_algo.is_some();
                    if !has_checksum {
                        let _ = tokio::fs::rename(&dl_path, &final_path).await;
                        let _ = Sidecar::remove(&sc_path);
                    } else {
                        let planner_now = planner.lock().await;
                        let sidecar =
                            dynamic_sidecar_snapshot(spec, &probed_stamp, &planner_now, &writer, total);
                        drop(planner_now);
                        let _ = Sidecar::save(&sidecar, &sc_path);
                    }
                    let _ = emit(evt_tx, Evt::DownloadDone {
                        id: spec.id,
                        total,
                    }).await;
                    return Flow::Completed;
                }
                if tokio::time::Instant::now() >= flush_deadline {
                    flush_deadline = tokio::time::Instant::now() + sidecar_flush;
                    {
                        let mut p = planner.lock().await;
                        let w = Arc::clone(&writer);
                        p.gc(|start, end| w.flushed_extent_from(start).min(end));
                    }
                    let flushed_now = writer.total_flushed();
                    let need = std::cmp::max(1024 * 1024, total / 16);
                    let sync_due = flushed_now.saturating_sub(last_sync_flushed) >= need
                        || last_sync.elapsed() >= Duration::from_secs(30);
                    if sync_due && writer.sync_data().await.is_ok() {
                        last_sync = Instant::now();
                        last_sync_flushed = flushed_now;
                    }
                    let planner_now = planner.lock().await;
                    let sidecar =
                        dynamic_sidecar_snapshot(spec, &probed_stamp, &planner_now, &writer, total);
                    drop(planner_now);
                    let _ = Sidecar::save(&sidecar, &sc_path);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaps_of_complements_committed_regions() {
        let gaps = gaps_of(&[(0, 2_000_000), (3_000_000, 3_500_000)], 10_000_000);
        assert_eq!(gaps, vec![(2_000_000, 3_000_000), (3_500_000, 10_000_000)]);
        let gaps = gaps_of(&[], 1_000);
        assert_eq!(gaps, vec![(0, 1_000)], "空段列表 = 全文件缺口");
        let gaps = gaps_of(&[(0, 1_000)], 1_000);
        assert!(gaps.is_empty(), "全覆盖无缺口");
        let gaps = gaps_of(&[(500, 900)], 1_000);
        assert_eq!(gaps, vec![(0, 500), (900, 1_000)]);
        let gaps = gaps_of(&[(0, 2_000), (1_000, 3_000)], 4_000);
        assert_eq!(gaps, vec![(3_000, 4_000)], "重叠已提交区间合并后求补");
    }

    #[test]
    fn inplace_ladder_matches_d86() {
        assert_eq!(inplace_backoff(1), Duration::from_secs(0));
        assert_eq!(inplace_backoff(2), Duration::from_secs(1));
        assert_eq!(inplace_backoff(3), Duration::from_secs(2));
        assert_eq!(inplace_backoff(4), Duration::from_secs(4));
        assert_eq!(inplace_backoff(5), Duration::from_secs(8));
        assert_eq!(inplace_backoff(6), Duration::from_secs(8), "封顶 8s");
    }
}

#[cfg(test)]
mod e2e_tests {
    use super::*;
    use crate::engine::{EngineShared, supervisor::Flow};
    use crate::model::Protocol;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    fn fill(buf: &mut [u8], pos: u64) {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = ((pos as usize + i) % 251) as u8;
        }
    }

    #[allow(dead_code)]
    fn norange_server(total: u64) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let data: Vec<u8> = (0..total as usize).map(|i| (i % 251) as u8).collect();
            for conn in listener.incoming().flatten() {
                let data = data.clone();
                std::thread::spawn(move || {
                    let mut c = conn;
                    let mut r = BufReader::new(c.try_clone().unwrap());
                    let mut line = String::new();
                    while r.read_line(&mut line).unwrap_or(0) > 0 {
                        if line == "\r\n" {
                            break;
                        }
                        line.clear();
                    }
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"mock-{total}\"\r\nConnection: close\r\n\r\n",
                        data.len()
                    );
                    let _ = c.write_all(head.as_bytes());
                    let _ = c.write_all(&data);
                    let _ = c.flush();
                });
            }
        });
        format!("http://{addr}/f.bin")
    }

    fn dyn_spec(id: u32, url: &str, dir: &std::path::Path, conc: usize) -> TaskSpec {
        TaskSpec {
            id,
            url: url.to_string(),
            save_dir: dir.to_string_lossy().into_owned(),
            name: "f.bin".to_string(),
            concurrency: conc,
            block_size: 1024 * 1024,
            scheduler: crate::model::config::HttpScheduler::Dynamic,
            min_split: 64 * 1024,
            protocol: Protocol::Http,
            expected_algo: None,
            expected_value: None,
            sidecar: None,
            sidecar_path: None,
            keep_name: false,
            resume_orphan: false,
            added_at: 0,
            proxy_endpoint: None,
            progress_tick_ms: 50,
            sidecar_flush_secs: 1,
            task_max_download_speed: 0,
        }
    }

    fn free_shared() -> Arc<EngineShared> {
        Arc::new(EngineShared::for_test())
    }

    async fn launch(
        spec: TaskSpec,
    ) -> (
        tokio::sync::mpsc::Sender<TaskCmd>,
        tokio::sync::mpsc::Receiver<Evt>,
        std::time::Instant,
    ) {
        let (tx, rx) = mpsc::channel::<Evt>(64);
        let (cmd_tx, cmd_rx) = mpsc::channel::<TaskCmd>(4);
        tokio::spawn(crate::engine::supervisor::run(spec, free_shared(), cmd_rx, tx));
        (
            cmd_tx,
            rx,
            std::time::Instant::now() + std::time::Duration::from_secs(30),
        )
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dynamic_new_task_completes_multi_segment() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dyn-new");
        let total: u64 = 512 * 1024;
        let url = super::super::tests::mock_server(total, vec![("slow".into(), String::new())]);
        let spec = dyn_spec(1, &url, &dir, 4);
        let (_cmd_tx, mut rx, deadline) = launch(spec).await;
        let mut done = false;
        let mut dynamic_probed = false;
        let mut toast = String::new();
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Probed { dynamic, .. })) => {
                    dynamic_probed = dynamic;
                }
                Ok(Some(Evt::DownloadDone { total: t, .. })) => {
                    assert_eq!(t, total);
                    done = true;
                    break;
                }
                Ok(Some(Evt::Toast(s))) => toast = s,
                Ok(Some(Evt::Failed { reason, .. })) => panic!("动态下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(done, "动态任务应完成");
        assert!(dynamic_probed, "Probed 事件应携带 dynamic 标记");
        assert!(toast.is_empty(), "Range 服务器不应触发降级 toast");
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        assert_eq!(data.len() as u64, total);
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        let diffs: Vec<usize> = data
            .iter()
            .zip(expect.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect();
        assert!(
            diffs.is_empty(),
            "动态引擎字节完整: diffs={} first={} got={:?} want={:?}",
            diffs.len(),
            diffs.first().unwrap_or(&0),
            diffs.first().map(|i| &data[*i..(*i + 8).min(data.len())]),
            diffs.first().map(|i| &expect[*i..(*i + 8).min(expect.len())])
        );
        assert!(!dir.join("f.bin.ezr").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn dynamic_pause_saves_dynamic_sidecar_then_resume_completes() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dyn-pause");
        let total: u64 = 2 * 1024 * 1024;
        let url = super::super::tests::mock_server(total, vec![("slow".into(), String::new())]);
        let spec = dyn_spec(1, &url, &dir, 4);
        let (cmd_tx, mut rx, deadline) = launch(spec).await;
        let mut paused = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(300), rx.recv()).await {
                Ok(Some(Evt::Progress { downloaded, .. })) => {
                    if downloaded > 256 * 1024 {
                        cmd_tx.send(TaskCmd::Pause).await.unwrap();
                    }
                }
                Ok(Some(Evt::PausedDone { downloaded, .. })) => {
                    assert!(downloaded > 0, "暂停应携带真实进度 {downloaded}");
                    paused = true;
                    break;
                }
                Ok(Some(Evt::DownloadDone { .. })) => panic!("应在暂停前截停"),
                Ok(Some(Evt::Failed { reason, .. })) => panic!("暂停前失败: {reason}"),
                _ => {}
            }
        }
        assert!(paused, "应收到 PausedDone");
        let sc_path = dir.join("f.bin.ezr");
        let sc = Sidecar::load(sc_path.to_str().unwrap()).expect("暂停台账落盘");
        assert!(sc.is_dynamic(), "动态任务暂停台账 = 动态格式（D89）");
        assert!(sc.downloaded > 0);
        let resumed_before = sc.downloaded;

        let mut spec2 = dyn_spec(1, &url, &dir, 4);
        spec2.sidecar = Some(sc);
        spec2.sidecar_path = Some(sc_path.to_string_lossy().into_owned());
        spec2.keep_name = true;
        let (_cmd_tx2, mut rx2, deadline2) = launch(spec2).await;
        let mut done = false;
        let mut first = true;
        while std::time::Instant::now() < deadline2 {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx2.recv()).await {
                Ok(Some(Evt::Progress { downloaded, .. })) => {
                    if first {
                        assert!(
                            downloaded >= resumed_before,
                            "续传进度不得回退（{downloaded} < {resumed_before}）"
                        );
                        first = false;
                    }
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("一致动态台账不应作废"),
                Ok(Some(Evt::Failed { reason, .. })) => panic!("动态续传失败: {reason}"),
                _ => {}
            }
        }
        assert!(done, "动态续传应完成");
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        assert_eq!(data.len() as u64, total);
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect, "续传后字节完整");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn dynamic_range_ignored_rescues_to_single_stream() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dyn-norange");
        let total: u64 = 512 * 1024;
        let url = norange_server(total);
        let spec = dyn_spec(1, &url, &dir, 4);
        let (_cmd_tx, mut rx, deadline) = launch(spec).await;
        let mut done = false;
        let mut toast_seen = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Toast(s))) => {
                    if s.contains("不支持断点续传") {
                        toast_seen = true;
                    }
                }
                Ok(Some(Evt::DownloadDone { total: t, .. })) => {
                    assert_eq!(t, total);
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("降级下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(done, "Range 失效应降级单流完成（D87）");
        assert!(toast_seen, "应触发既有「不支持断点续传」toast");
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect, "降级单流字节完整");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn dynamic_sidecar_format_is_engine_on_resume() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dyn-fmt");
        let total: u64 = 2 * 1024 * 1024;
        let url = super::super::tests::mock_server(total, vec![("slow".into(), String::new())]);
        let spec = dyn_spec(1, &url, &dir, 4);
        let (cmd_tx, mut rx, deadline) = launch(spec).await;
        let mut paused = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(300), rx.recv()).await {
                Ok(Some(Evt::Progress { downloaded, .. })) => {
                    if downloaded > 256 * 1024 {
                        cmd_tx.send(TaskCmd::Pause).await.unwrap();
                    }
                }
                Ok(Some(Evt::PausedDone { .. })) => {
                    paused = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(paused);
        let sc_path = dir.join("f.bin.ezr");
        let text = std::fs::read_to_string(&sc_path).unwrap();
        assert!(
            text.contains("\"scheduler\"") && text.contains("\"dynamic\""),
            "动态 sidecar 含 scheduler 字段: {text}"
        );
        let sc = Sidecar::load(sc_path.to_str().unwrap()).unwrap();

        let mut spec2 = dyn_spec(1, &url, &dir, 4);
        spec2.scheduler = crate::model::config::HttpScheduler::Dynamic;
        spec2.sidecar = Some(sc);
        spec2.sidecar_path = Some(sc_path.to_string_lossy().into_owned());
        spec2.keep_name = true;
        let (_cmd_tx2, mut rx2, deadline2) = launch(spec2).await;
        let mut done = false;
        while std::time::Instant::now() < deadline2 {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx2.recv()).await {
                Ok(Some(Evt::DownloadDone { .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("格式即引擎续传失败: {reason}"),
                _ => {}
            }
        }
        assert!(done, "动态 sidecar 应由动态引擎接续");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[allow(dead_code)]
    fn flow_shape(f: Flow) -> bool {
        matches!(f, Flow::Completed)
    }
}
