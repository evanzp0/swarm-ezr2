use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, watch};

use super::super::error::{classify_reqwest, EngineFailure};
use super::super::throttle::Throttle;
use super::super::{ConnActivity, ConnView};
use super::http::{probe_head, req_checked, req_with_identity};
use super::planner::{Claim, Planner, MIN_PREEMPT};
use super::single::single_stream;
use super::writer::Writer;
use super::{emit, Evt, Flow, PausedState, TaskCmd, TaskSpec};
use crate::model::chunk::{
    merge_byte_ranges, project_ranges, refine_http_grid, Blocks, MIN_HTTP_BLOCK_SIZE,
};
use crate::model::config::ProxyEndpoint;
use crate::model::consistency::{self, Consistency, ServerStamp};
use crate::model::namegen::DOWNLOADING_EXT;
use crate::model::sidecar::{Sidecar, SidecarTask};
use crate::model::{Checksum, FailKind};

const MAX_INPLACE_ATTEMPTS: u32 = 5;
const CONN_VIEW_PERIOD: Duration = Duration::from_millis(100);

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
    let mut sorted: Vec<(u64, u64)> = segments.iter().filter(|(s, c)| c > s).copied().collect();
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
    planner: Arc<std::sync::Mutex<Planner>>,
    writer: Arc<Writer>,
    stop_rx: watch::Receiver<bool>,
    quota_rx: watch::Receiver<usize>,
    ep_rx: watch::Receiver<Arc<Option<ProxyEndpoint>>>,
    task_dl: Arc<Throttle>,
    speed_rx: watch::Receiver<u64>,
    failure: Arc<std::sync::Mutex<Option<EngineFailure>>>,
    conns: Arc<std::sync::Mutex<Vec<ConnView>>>,
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
            planner.lock().expect("planner 锁").release(wid);
            conns.lock().expect("conns 锁").retain(|c| c.id != wid);
            return;
        }
        if wid > *quota_rx.borrow_and_update() {
            planner.lock().expect("planner 锁").release(wid);
            conns.lock().expect("conns 锁").retain(|c| c.id != wid);
            return;
        }
        let assignment = planner.lock().expect("planner 锁").assign(wid, after);
        let Some(a) = assignment else {
            conns.lock().expect("conns 锁").retain(|c| c.id != wid);
            return;
        };
        after = None;
        let mut cur_start = a.from;
        let mut last_cv_at = Instant::now();
        {
            let mut g = conns.lock().expect("conns 锁");
            g.retain(|c| c.id != wid);
            g.push(ConnView {
                id: wid,
                block: 0,
                start: a.from,
                end: total,
                done: 0,
                activity: ConnActivity::Connecting,
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
                        planner.lock().expect("planner 锁").release(wid);
                        if resume {
                            *failure.lock().expect("failure 锁") = Some(EngineFailure::network(
                                "服务器本次未支持断点续传（Range），保留断点稍后重试",
                            ));
                        }
                        conns.lock().expect("conns 锁").retain(|c| c.id != wid);
                        return;
                    }
                    r
                }
                Err(f) => {
                    let f = f.into_block_retry();
                    let wait = match f.kind {
                        FailKind::Transient => {
                            inplace += 1;
                            if inplace > MAX_INPLACE_ATTEMPTS {
                                *failure.lock().expect("failure 锁") = Some(f);
                                return;
                            }
                            if let Some(cv) = conns
                                .lock()
                                .expect("conns 锁")
                                .iter_mut()
                                .find(|c| c.id == wid)
                            {
                                cv.activity = ConnActivity::Retrying(f.reason.clone());
                            }
                            inplace_backoff(inplace)
                        }
                        _ => {
                            *failure.lock().expect("failure 锁") = Some(f);
                            return;
                        }
                    };
                    planner.lock().expect("planner 锁").release(wid);
                    tokio::select! {
                        _ = tokio::time::sleep(wait) => {}
                        _ = stop_rx.changed() => {
                            if *stop_rx.borrow() {
                                return;
                            }
                        }
                        _ = quota_rx.changed() => {
                            if wid > *quota_rx.borrow_and_update() {
                                conns.lock().expect("conns 锁").retain(|c| c.id != wid);
                                return;
                            }
                        }
                    }
                    continue;
                }
            }
        };
        if let Some(cv) = conns
            .lock()
            .expect("conns 锁")
            .iter_mut()
            .find(|c| c.id == wid)
        {
            cv.activity = ConnActivity::Receiving;
        }
        let mut at = a.from;
        let mut carry: Vec<u8> = Vec::new();
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
                            let data = if carry.is_empty() {
                                bytes
                            } else {
                                let mut buf = std::mem::take(&mut carry);
                                buf.extend_from_slice(&bytes);
                                bytes::Bytes::from(buf)
                            };
                            let claim = planner
                                .lock()
                                .expect("planner 锁")
                                .claim(wid, at, data.len() as u64);
                            match claim {
                                Claim::Granted { n, seg_end } => {
                                    if n > 0 {
                                        let sealed = match writer.push(at, &data[..n as usize]) {
                                            Ok(s) => s,
                                            Err(_) => {
                                                *failure.lock().expect("failure 锁") =
                                                    Some(EngineFailure::fatal("磁盘写入失败"));
                                                aborted = true;
                                                break;
                                            }
                                        };
                                        if sealed || writer.pending_bytes() >= hold_cap {
                                            if let Err(e) = writer.step(hold_cap).await {
                                                *failure.lock().expect("failure 锁") = Some(
                                                    EngineFailure::fatal(format!(
                                                        "磁盘写入失败（{e}）"
                                                    )),
                                                );
                                                aborted = true;
                                                break;
                                            }
                                        }
                                        at += n;
                                    }
                                    let granted = n as usize;
                                    if granted < data.len() {
                                        carry.extend_from_slice(&data[granted..]);
                                    }
                                    let (absorbed, rotate_after) = {
                                        let mut p = planner.lock().expect("planner 锁");
                                        match p.commit(wid, at) {
                                            Some(end) => match p.absorb_next(wid, end) {
                                                Some(new_end) => (Some((end, new_end)), None),
                                                None => (None, Some(end)),
                                            },
                                            None => (None, None),
                                        }
                                    };
                                    if n > 0 && last_cv_at.elapsed() >= CONN_VIEW_PERIOD {
                                        last_cv_at = Instant::now();
                                        if let Some(cv) = conns
                                            .lock()
                                            .expect("conns 锁")
                                            .iter_mut()
                                            .find(|c| c.id == wid)
                                        {
                                            cv.start = cur_start;
                                            cv.end = seg_end;
                                            cv.done = at - cur_start;
                                        }
                                    }
                                    if let Some((end, new_end)) = absorbed {
                                        cur_start = end;
                                        last_cv_at = Instant::now();
                                        if let Some(cv) = conns
                                            .lock()
                                            .expect("conns 锁")
                                            .iter_mut()
                                            .find(|c| c.id == wid)
                                        {
                                            cv.start = cur_start;
                                            cv.end = new_end;
                                            cv.done = 0;
                                        }
                                    }
                                    if let Some(end) = rotate_after {
                                        after = Some(end);
                                        clean_rotation = true;
                                        break;
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
                            planner.lock().expect("planner 锁").release(wid);
                            inplace += 1;
                            if inplace > MAX_INPLACE_ATTEMPTS {
                                *failure.lock().expect("failure 锁") =
                                    Some(EngineFailure::network("服务器提前结束响应"));
                                aborted = true;
                            } else {
                                if let Some(cv) = conns
                                    .lock()
                                    .expect("conns 锁")
                                    .iter_mut()
                                    .find(|c| c.id == wid)
                                {
                                    cv.activity =
                                        ConnActivity::Retrying("服务器提前结束响应".to_string());
                                }
                                retry_wait = Some(inplace_backoff(inplace));
                            }
                            break;
                        }
                        Err(e) => {
                            let f = classify_reqwest(&e);
                            match f.kind {
                                FailKind::Transient => {
                                    planner.lock().expect("planner 锁").release(wid);
                                    inplace += 1;
                                    if inplace > MAX_INPLACE_ATTEMPTS {
                                        *failure.lock().expect("failure 锁") = Some(f);
                                        aborted = true;
                                    } else {
                                        if let Some(cv) = conns
                                            .lock()
                                            .expect("conns 锁")
                                            .iter_mut()
                                            .find(|c| c.id == wid)
                                        {
                                            cv.activity =
                                                ConnActivity::Retrying(f.reason.clone());
                                        }
                                        retry_wait = Some(inplace_backoff(inplace));
                                    }
                                }
                                _ => {
                                    *failure.lock().expect("failure 锁") = Some(f);
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
                        conns.lock().expect("conns 锁").retain(|c| c.id != wid);
                        return;
                    }
                }
            }
            continue;
        }
        if aborted {
            planner.lock().expect("planner 锁").release(wid);
            if quota_aborted {
                conns.lock().expect("conns 锁").retain(|c| c.id != wid);
            }
            return;
        }
        if failure.lock().expect("failure 锁").is_some() {
            planner.lock().expect("planner 锁").release(wid);
            return;
        }
        if clean_rotation {
            inplace = 0;
        }
    }
}

fn flushed_complete(writer: &Writer, base_flushed: u64, total: u64) -> bool {
    writer.global_flushed_prefix() >= total || base_flushed + writer.total_flushed() == total
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
    base_ranges: &[(u64, u64)],
    writer: &Writer,
    total: u64,
) -> Sidecar {
    let mut ranges: Vec<(u64, u64)> = base_ranges.to_vec();
    ranges.extend(writer.flushed_ranges());
    merge_byte_ranges(&mut ranges);
    Sidecar::build_dynamic(
        stamp,
        total,
        &ranges,
        expected_checksum(spec).as_ref(),
        sidecar_task(spec),
    )
}

fn stop_dyn_workers(
    stop: &std::sync::atomic::AtomicBool,
    stop_tx: &watch::Sender<bool>,
    handles: &mut Vec<(usize, tokio::task::JoinHandle<()>)>,
) {
    stop.store(true, Ordering::Relaxed);
    let _ = stop_tx.send(true);
    for (_, h) in handles.drain(..) {
        h.abort();
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
    let resp = loop {
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
                    Some((
                        sidecar.clone(),
                        spec.sidecar_path.clone().unwrap_or_default(),
                    )),
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
    let cut_align = refine_http_grid(total, spec.block_size, spec.concurrency);
    let planner = match sc.as_ref() {
        Some(sidecar) if sidecar.is_dynamic() => {
            let gaps = gaps_of(&sidecar.resume_segments(), sidecar.size);
            Planner::from_gaps(total, &gaps, min_split, cut_align, preempt_after)
        }
        _ => Planner::new(total, min_split, cut_align, preempt_after),
    };
    let resume_ranges: Vec<(u64, u64)> = sc
        .as_ref()
        .filter(|s| s.is_dynamic())
        .map(|s| s.resume_segments())
        .unwrap_or_default();
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
            let sidecar =
                dynamic_sidecar_snapshot(spec, &probed_stamp, &resume_ranges, &writer, total);
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
    let planner = Arc::new(std::sync::Mutex::new(planner));
    let failure = Arc::new(std::sync::Mutex::<Option<EngineFailure>>::new(None));
    let conns = Arc::new(std::sync::Mutex::<Vec<ConnView>>::new(Vec::new()));
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

    let mut primed_resp: PrimedResponse = Some((resp, None));
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
                    deps.conns.lock().expect("conns 锁").retain(|c| c.id <= q);
                    handles.retain(|(_, h)| !h.is_finished());
                    while handles.len() < q {
                        let next = (1..=q).find(|i| !handles.iter().any(|(s, _)| s == i)).unwrap_or(q + 1);
                        handles.push((next, deps.spawn(next, Arc::clone(shared), None)));
                    }
                }
                Some(TaskCmd::Pause) | Some(TaskCmd::Cancel) => {
                    let cancelled = cmd == Some(TaskCmd::Cancel);
                    stop_dyn_workers(&stop, &stop_tx, &mut handles);
                    let finish_res = writer.finish().await;
                    if cancelled {
                        return Flow::Cancelled;
                    }
                    if let Err(e) = finish_res {
                        let sidecar =
                            dynamic_sidecar_snapshot(spec, &probed_stamp, &resume_ranges, &writer, total);
                        return Flow::Failed(
                            EngineFailure::fatal(format!("磁盘写入失败（{e}）")),
                            writer.total_pushed() > 0,
                            Some((sidecar, sc_path.clone())),
                        );
                    }
                    let sidecar =
                        dynamic_sidecar_snapshot(spec, &probed_stamp, &resume_ranges, &writer, total);
                    return Flow::Paused(PausedState {
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
                    stop_dyn_workers(&stop, &stop_tx, &mut handles);
                    let _ = writer.finish().await;
                    stop.store(false, Ordering::Relaxed);
                    let _ = stop_tx.send(false);
                    *planner.lock().expect("planner 锁") = Planner::new(total, min_split, cut_align, preempt_rescue);
                    let _ = quota_tx.send(1);
                    let rescue_client = shared.client_for(spec.proxy_endpoint.as_ref());
                    let rescue_resp = match req_with_identity(&rescue_client, &spec.url, None).send().await {
                        Ok(r) if r.status().is_success() => Some((r, None)),
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
                if failure.lock().expect("failure 锁").is_some() {
                    stop_dyn_workers(&stop, &stop_tx, &mut handles);
                    let _ = writer.finish().await;
                    let f = failure.lock().expect("failure 锁").clone().unwrap_or_else(|| {
                        EngineFailure::network("传输中断")
                    });
                    let sidecar =
                        dynamic_sidecar_snapshot(spec, &probed_stamp, &resume_ranges, &writer, total);
                    return Flow::Failed(
                        f,
                        base_flushed + writer.total_pushed() > 0,
                        Some((sidecar, sc_path.clone())),
                    );
                }
                let (completed, committed) = {
                    let p = planner.lock().expect("planner 锁");
                    (p.completed_bytes(), p.committed_ranges())
                };
                let conns_now = conns.lock().expect("conns 锁").clone();
                let downloaded = completed;
                // pieces 网格投影：resume 区间 ∪ 本会话落盘区间 ∪ 本会话已提交区间
                // → Done/Partial 集合。刷盘回收由 chunk 步进驱动（absorb 只在
                // step/finish），停包期间滞留队列与在途作业不回收，落盘区间滞后于
                // 聚合游标：worker 移出块后其字节已全部提交，若只投影落盘区间，
                // 已收满的块会被误判部分覆盖（🟨）——与列表 downloaded 口径脱节。
                // 已提交区间与 downloaded 同源，保证网格绿块与百分比一致。
                let mut ranges = resume_ranges.clone();
                ranges.extend(writer.flushed_ranges());
                ranges.extend(committed);
                let (chunk_done_proj, done_blocks, partial_blocks) =
                    project_ranges(total, cut_align, &ranges);
                if !emit(evt_tx, Evt::Progress {
                    id: spec.id,
                    downloaded,
                    conns: conns_now,
                    chunk_done: chunk_done_proj,
                    done_blocks,
                    partial_blocks,
                }).await {
                    return Flow::Cancelled;
                }
                if completed >= total && total > 0 {
                    stop_dyn_workers(&stop, &stop_tx, &mut handles);
                    if let Err(e) = writer.finish().await {
                        let sidecar = dynamic_sidecar_snapshot(
                            spec,
                            &probed_stamp,
                            &resume_ranges,
                            &writer,
                            total,
                        );
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
                        let sidecar = dynamic_sidecar_snapshot(
                            spec,
                            &probed_stamp,
                            &resume_ranges,
                            &writer,
                            total,
                        );
                        return Flow::Failed(f, true, Some((sidecar, sc_path.clone())));
                    }
                    let has_checksum = spec.expected_algo.is_some();
                    if !has_checksum {
                        let _ = tokio::fs::rename(&dl_path, &final_path).await;
                        let _ = Sidecar::remove(&sc_path);
                    } else {
                        let sidecar = dynamic_sidecar_snapshot(
                            spec,
                            &probed_stamp,
                            &resume_ranges,
                            &writer,
                            total,
                        );
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
                    let _ = writer.step(u64::MAX).await;
                    {
                        let mut p = planner.lock().expect("planner 锁");
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
                    let sidecar =
                        dynamic_sidecar_snapshot(spec, &probed_stamp, &resume_ranges, &writer, total);
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
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    use super::*;
    use crate::engine::supervisor::Flow;
    use crate::engine::EngineShared;
    use crate::model::chunk::block_range;
    use crate::model::Protocol;

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

    fn burst_server(total: u64, burst: usize, pause_ms: u64) -> String {
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
                        "HTTP/1.1 200 OK\r\nAccept-Ranges: bytes\r\nContent-Length: {total}\r\nETag: \"mock-{total}\"\r\nLast-Modified: Mon, 09 Feb 2026 08:00:00 GMT\r\nConnection: close\r\n\r\n"
                    );
                    let _ = c.write_all(head.as_bytes());
                    let mut pos = 0usize;
                    while pos < data.len() {
                        let n = burst.min(data.len() - pos);
                        let _ = c.write_all(&data[pos..pos + n]);
                        let _ = c.flush();
                        pos += n;
                        if pos < data.len() {
                            std::thread::sleep(std::time::Duration::from_millis(pause_ms));
                        }
                    }
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
        tokio::spawn(crate::engine::supervisor::run(
            spec,
            free_shared(),
            cmd_rx,
            tx,
        ));
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
            diffs
                .first()
                .map(|i| &expect[*i..(*i + 8).min(expect.len())])
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

    #[tokio::test(flavor = "multi_thread")]
    async fn dynamic_progress_carries_done_and_partial_blocks() {
        // pieces 窗口数据源回归：Progress 必须携带真实 Done/Partial 集合，
        // 否则分块网格只剩 🟦 与 ⬜（已下载块全白）。
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dyn-pieces");
        let total: u64 = 8 * 1024 * 1024;
        let url = super::super::tests::mock_server(total, vec![("slow".into(), String::new())]);
        let mut spec = dyn_spec(1, &url, &dir, 4);
        spec.task_max_download_speed = 2 * 1024 * 1024; // 限速 2 MiB/s → ≥4s 可观察
        let (_cmd_tx, mut rx, deadline) = launch(spec).await;
        let mut saw_done_growth = false;
        let mut saw_partial = false;
        let mut last_done: u32 = 0;
        let mut completed = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Progress {
                    chunk_done,
                    done_blocks,
                    partial_blocks,
                    ..
                })) => {
                    assert_eq!(
                        done_blocks.len() as u32,
                        chunk_done,
                        "chunk_done 与 done_blocks 长度一致"
                    );
                    assert_eq!(
                        *done_blocks.last().unwrap_or(&0),
                        *done_blocks.iter().max().unwrap_or(&0),
                        "done_blocks 升序"
                    );
                    if chunk_done > last_done {
                        last_done = chunk_done;
                        saw_done_growth = true;
                    }
                    if !partial_blocks.is_empty() {
                        saw_partial = true;
                    }
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    completed = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("限速动态下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(completed, "限速动态任务应完成");
        assert!(saw_done_growth, "下载过程中应观察到 done 块增长（🟩）");
        assert!(saw_partial, "下载过程中应观察到部分块（🟨）");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dynamic_done_blocks_track_committed_prefix_when_worker_advances() {
        // 操作者报告回归：蓝块移入下一块后，原蓝块闪 🟨（部分下载）而非 🟩。
        // 根因：Progress 投影只并集落盘区间（flushed），而刷盘回收由 chunk 步进驱动
        // （absorb 只在 step/finish），停包期间滞留队列与在途作业不回收 →
        // 聚合游标（= 列表 downloaded 同源口径）已覆盖整块，投影却仍判部分覆盖。
        // 口径（conc=1 时 downloaded = 已提交前缀）：downloaded 覆盖的整块
        // 必须当场 ∈ done_blocks 且 ∉ partial_blocks——worker 移出块的瞬间原块即 🟩。
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dyn-committed");
        let total: u64 = 4 * 1024 * 1024;
        let block = 1024 * 1024u64;
        let url = burst_server(total, 1024 * 1024, 120);
        let mut spec = dyn_spec(1, &url, &dir, 1);
        spec.block_size = block;
        let (_cmd_tx, mut rx, deadline) = launch(spec).await;
        let y = 4u32;
        let mut saw_meaningful = false;
        let mut completed = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Progress {
                    downloaded,
                    done_blocks,
                    partial_blocks,
                    ..
                })) => {
                    if downloaded >= block {
                        saw_meaningful = true;
                    }
                    for b in 0..y {
                        let bend = ((u64::from(b) + 1) * block).min(total);
                        if bend <= downloaded {
                            assert!(
                                done_blocks.contains(&b),
                                "downloaded={downloaded} 已提交覆盖整块 {b}，必须当场 🟩（done={done_blocks:?} partial={partial_blocks:?}）"
                            );
                            assert!(
                                !partial_blocks.contains(&b),
                                "downloaded={downloaded} 已提交覆盖整块 {b}，不得 🟨（partial={partial_blocks:?}）"
                            );
                        }
                    }
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    completed = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("突发服务器动态下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(completed, "突发服务器任务应完成");
        assert!(
            saw_meaningful,
            "应观察到 downloaded ≥ 1 块的 Progress 帧（不变式断言非空转）"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dynamic_vacated_blocks_turn_green_when_frontier_moves_on() {
        // 操作者报告回归：并发下载中蓝色“正在下载”块移入下一块后，原块显示 🟨
        // 而非 🟩。根因：try_split 切分点只按 4096 对齐，后继租约从块中间起步，
        // 块头字节仍在前任租约未完成区域——后继移过该块时投影判部分覆盖。
        // 修复口径：切分点落在块边界；任一连接租约内已移过的整块必须当场
        // ∈ done_blocks 且 ∉ partial_blocks。
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dyn-vacated");
        let total: u64 = 8 * 1024 * 1024;
        let block: u64 = 1024 * 1024;
        let url = burst_server(total, 64 * 1024, 5);
        let mut spec = dyn_spec(1, &url, &dir, 2);
        spec.block_size = block;
        spec.task_max_download_speed = 2 * 1024 * 1024;
        let (_cmd_tx, mut rx, deadline) = launch(spec).await;
        let mut saw_vacated = false;
        let mut completed = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Progress {
                    conns,
                    done_blocks,
                    partial_blocks,
                    ..
                })) => {
                    for c in &conns {
                        if c.end <= c.start {
                            continue;
                        }
                        let pos = c.start + c.done;
                        let mut b = c.start / block;
                        while let Some((_, be)) = block_range(total, block, b as u32) {
                            if be > pos {
                                break;
                            }
                            saw_vacated = true;
                            assert!(
                                done_blocks.contains(&(b as u32)),
                                "连接 lease_start={} pos={pos} 已移过块 {b}，必须当场 🟩（done={done_blocks:?} partial={partial_blocks:?}）",
                                c.start
                            );
                            assert!(
                                !partial_blocks.contains(&(b as u32)),
                                "连接已移过块 {b} 不得 🟨（partial={partial_blocks:?}）"
                            );
                            b += 1;
                        }
                    }
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    completed = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("并发动态下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(completed, "并发动态任务应完成");
        assert!(
            saw_vacated,
            "应观察到连接移过整块的前沿推进（不变式断言非空转）"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dynamic_straddling_chunk_across_absorbed_segment_keeps_stream_aligned() {
        // 潜在 bug 回归：chunk 跨越段边界时 claim 只授予到段尾，剩余字节若被丢弃，
        // absorb 接续同一条 HTTP 流后下一 chunk 会写到错误偏移（字节丢失/错位）。
        // 修复口径：跨段剩余字节进 carry 缓冲，absorb 续流后先认领 carry。
        // 服务器按 100 KiB 步进发送，250 KiB 切分点（非 16 KiB 对齐）必被某 chunk 跨越。
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dyn-straddle");
        let total: u64 = 600 * 1024;
        let url = burst_server(total, 100 * 1024, 30);
        let mut spec = dyn_spec(1, &url, &dir, 2);
        spec.block_size = 250 * 1024;
        let (cmd_tx, mut rx, deadline) = launch(spec).await;
        cmd_tx
            .send(TaskCmd::Reconfigure {
                concurrency: 1,
                endpoint: None,
                max_download_speed: 0,
            })
            .await
            .unwrap();
        let mut completed = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::DownloadDone { total: t, .. })) => {
                    assert_eq!(t, total);
                    completed = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("跨段续流下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(completed, "跨段续流任务应完成");
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        assert_eq!(data.len() as u64, total, "文件长度完整");
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
            "跨段字节流必须对齐: diffs={} first={} got={:?} want={:?}",
            diffs.len(),
            diffs.first().unwrap_or(&0),
            diffs.first().map(|i| &data[*i..(*i + 8).min(data.len())]),
            diffs
                .first()
                .map(|i| &expect[*i..(*i + 8).min(expect.len())])
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dynamic_refined_grid_restores_full_concurrency() {
        // 速度回退回归：块对齐切分把并行度封顶在 UI 网格块数上（2 MiB 文件 1 MiB 网格
        // 只能 2 连接）。修复 = refine_http_grid 收细网格保证块数 ≥ 并发；本例 2 MiB +
        // 并发 4 → 512 KiB 网格 4 块 → 4 条块对齐并行连接。
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dyn-conc");
        let total: u64 = 2 * 1024 * 1024;
        let url = super::super::tests::mock_server(total, vec![("slow".into(), String::new())]);
        let mut spec = dyn_spec(1, &url, &dir, 4);
        spec.block_size = 1024 * 1024;
        spec.task_max_download_speed = 512 * 1024;
        let (_cmd_tx, mut rx, deadline) = launch(spec).await;
        let mut peak_conns = 0usize;
        let mut completed = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Progress { conns, .. })) => {
                    peak_conns = peak_conns.max(conns.len());
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    completed = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("细化网格下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(completed, "细化网格任务应完成");
        assert!(
            peak_conns >= 4,
            "块数收细后必须达到满并发: peak={peak_conns}（切分封锁并行度回归）"
        );
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        assert_eq!(data.len() as u64, total, "文件长度完整");
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert!(data == expect, "细化网格并行下载字节完整");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dynamic_refined_grid_vacated_blocks_turn_green() {
        // bug9 绿块不变式在细化网格场景的守护：2.8 MB + 并发 4 → 网格收细 512 KiB，
        // 租约起点仍块对齐，任一连接移过的整块必当场 🟩 且不落 🟨。
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dyn-vacated-refined");
        let total: u64 = 2_800_000;
        let block = refine_http_grid(total, 1024 * 1024, 4);
        assert_eq!(block, 512 * 1024, "2.8 MB + 并发 4 → 512 KiB 网格");
        let url = super::super::tests::mock_server(total, vec![("slow".into(), String::new())]);
        let mut spec = dyn_spec(1, &url, &dir, 4);
        spec.block_size = 1024 * 1024;
        spec.task_max_download_speed = 512 * 1024;
        let (_cmd_tx, mut rx, deadline) = launch(spec).await;
        let mut saw_spanning = false;
        let mut frames = 0usize;
        let mut completed = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Progress {
                    conns,
                    done_blocks,
                    partial_blocks,
                    ..
                })) => {
                    frames += 1;
                    for c in &conns {
                        if c.end <= c.start {
                            continue;
                        }
                        if c.end - c.start > block {
                            saw_spanning = true;
                        }
                        let pos = c.start + c.done;
                        let mut b = c.start / block;
                        while let Some((_, be)) = block_range(total, block, b as u32) {
                            if be > pos {
                                break;
                            }
                            assert!(
                                done_blocks.contains(&(b as u32)),
                                "连接 lease_start={} pos={pos} 已移过块 {b}（512 KiB 网格），必须当场 🟩（done={done_blocks:?} partial={partial_blocks:?}）",
                                c.start
                            );
                            assert!(
                                !partial_blocks.contains(&(b as u32)),
                                "连接已移过块 {b} 不得 🟨（partial={partial_blocks:?}）"
                            );
                            b += 1;
                        }
                    }
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    completed = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("细化网格限速下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(completed, "细化网格任务应完成");
        assert!(
            saw_spanning,
            "细化网格下应存在跨块租约（鸽笼：块数 > 并发数）——布局未达可测前提"
        );
        assert!(
            frames >= 10,
            "进度帧采样不足（frames={frames}），不变式校验空转"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn dynamic_sidecar_snapshot_covers_gc_retired_ranges() {
        // pieces 暂停→续传蓝块 bug 根因回归：gc 退役的完全落盘段必须进快照。
        // 丢失时暂停 sidecar 缺已下载字节区间 → 续传把已下载块重下（🟦"正在下载"）
        // 且 Progress 投影丢绿块（🟩→⬜）。
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dyn-snap");
        let fp = dir.join("snap.bin");
        std::fs::File::create(&fp).unwrap();
        let w = Writer::open(&fp).unwrap();
        let total: u64 = 2 * 1024 * 1024;
        let mut p = Planner::new(total, 256 * 1024, 4096, MIN_PREEMPT);
        p.assign(1, None).unwrap();
        p.assign(2, None).unwrap();
        let cut = p.segments()[1].start;
        assert!(matches!(p.claim(1, 0, cut), Claim::Granted { .. }));
        p.commit(1, cut);
        assert!(matches!(p.claim(2, cut, 512 * 1024), Claim::Granted { .. }));
        p.commit(2, cut + 512 * 1024);
        w.push(0, &vec![0u8; cut as usize]).unwrap();
        w.push(cut, &vec![0u8; 512 * 1024]).unwrap();
        w.finish().await.unwrap();
        assert_eq!(
            p.gc(|s, e| w.flushed_extent_from(s).min(e)),
            1,
            "完全落盘段被 gc 退役"
        );
        p.check_invariants().unwrap();
        let flushed_end = cut + 512 * 1024;

        let spec = dyn_spec(1, "http://127.0.0.1/f.bin", &dir, 4);
        let stamp = ServerStamp {
            etag: None,
            last_modified: None,
            size: Some(total),
        };
        let sc = dynamic_sidecar_snapshot(&spec, &stamp, &[], &w, total);
        assert_eq!(
            sc.resume_segments(),
            vec![(0, flushed_end)],
            "快照必须覆盖 gc 退役区间（与 pieces 投影同源口径）"
        );
        assert_eq!(sc.downloaded, flushed_end, "sidecar 字节 = 真实落盘字节");
        let (chunk_done, done_blocks, _) =
            project_ranges(total, 1024 * 1024, &sc.resume_segments());
        assert_eq!(chunk_done, 1, "退役段整块应投影为 Done（🟩）");
        assert_eq!(done_blocks, vec![0], "续传投影不得丢已下载块");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn dynamic_resume_keeps_done_blocks_from_before_pause() {
        // 操作者报告回归：HTTP 任务暂停→继续后 pieces 窗口已下载块变 🟦/⬜。
        // 口径：sidecar 字节 = PausedDone 真实落盘字节；续传后任一 Progress 的
        // done_blocks ⊇ 暂停前集合（已下载块恒 🟩）、downloaded 不回退。
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dyn-resume-pieces");
        let total: u64 = 8 * 1024 * 1024;
        let url = super::super::tests::mock_server(total, vec![("slow".into(), String::new())]);
        let mut spec = dyn_spec(1, &url, &dir, 4);
        spec.task_max_download_speed = 2 * 1024 * 1024;
        let (cmd_tx, mut rx, deadline) = launch(spec).await;
        let mut pre_done: Vec<u32> = Vec::new();
        let mut paused_downloaded = 0u64;
        let mut pause_sent = false;
        let mut paused = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(300), rx.recv()).await {
                Ok(Some(Evt::Progress { done_blocks, .. })) => {
                    if done_blocks.len() >= 2 && !pause_sent {
                        pause_sent = true;
                        pre_done = done_blocks.clone();
                        cmd_tx.send(TaskCmd::Pause).await.unwrap();
                    }
                }
                Ok(Some(Evt::PausedDone { downloaded, .. })) => {
                    paused_downloaded = downloaded;
                    paused = true;
                    break;
                }
                Ok(Some(Evt::DownloadDone { .. })) => panic!("应在暂停前截停"),
                Ok(Some(Evt::Failed { reason, .. })) => panic!("暂停前失败: {reason}"),
                _ => {}
            }
        }
        assert!(paused, "应收到 PausedDone");
        assert!(pre_done.len() >= 2, "暂停前应观察到 ≥2 个绿块");
        let sc_path = dir.join("f.bin.ezr");
        let sc = Sidecar::load(sc_path.to_str().unwrap()).expect("暂停台账落盘");
        assert_eq!(
            sc.downloaded, paused_downloaded,
            "暂停 sidecar 字节必须等于真实落盘字节（{} ≠ {}）",
            sc.downloaded, paused_downloaded
        );

        let mut spec2 = dyn_spec(1, &url, &dir, 4);
        spec2.sidecar = Some(sc);
        spec2.sidecar_path = Some(sc_path.to_string_lossy().into_owned());
        spec2.keep_name = true;
        let (_cmd_tx2, mut rx2, deadline2) = launch(spec2).await;
        let mut done = false;
        let mut checked = false;
        while std::time::Instant::now() < deadline2 {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx2.recv()).await {
                Ok(Some(Evt::Progress {
                    downloaded,
                    done_blocks,
                    ..
                })) => {
                    assert!(
                        downloaded >= paused_downloaded,
                        "续传 downloaded 不得回退（{downloaded} < {paused_downloaded}）"
                    );
                    if !done_blocks.is_empty() {
                        let missing: Vec<u32> = pre_done
                            .iter()
                            .filter(|b| !done_blocks.contains(b))
                            .copied()
                            .collect();
                        assert!(
                            missing.is_empty(),
                            "暂停前已下载块续传后必须仍为 Done（🟩），丢失: {missing:?}"
                        );
                        checked = true;
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
        assert!(checked, "续传过程应观察到 done 集合");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[allow(dead_code)]
    fn flow_shape(f: Flow) -> bool {
        matches!(f, Flow::Completed)
    }
}
