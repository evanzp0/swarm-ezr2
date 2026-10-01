//! supervisor — 单任务下载监督者（探测 → 分块下载 → 完成/失败收尾 → 校验）
//!
//! 生命周期：`Cmd::Start` 携带 [`TaskSpec`] 触发 spawn。流程：
//! 1. 探测（`GET` + `Range: bytes=0-`）拿 Content-Length / ETag /
//!    Last-Modified / Content-Disposition（FR-01-10）；
//! 2. 续传一致性检查（spec 带 sidecar 时）：失效 → 作废断点回报
//!    [`Evt::Invalidated`]（不计失败、不占重试计数，FR-01-22/D11）；
//!    一致 → 恢复块表续传；
//! 3. 分块下载（支持 Range 且多块）：worker 池动态领块、乱序完成、
//!    实际并发 = min(并发数, 未完成块数)（FR-01-11）；单块或不支持 Range →
//!    单流降级（FR-01-12）；
//! 4. 完成：全块 + 大小校验（FR-01-52）→ [`Evt::DownloadDone`]；有校验值时
//!    App 转「校验中」并下发 `Cmd::Verify`，通过后 finalize
//!    （去 `.downloading` 扩展名 + 删 sidecar，FR-01-24/20）；
//! 5. 暂停/失败：停 worker → 块表快照写 sidecar（原子写）→ 回报事件。
//!
//! 停止信号用 `tokio::sync::watch`；worker 在流读取 select 中响应，无忙轮询。
//! sidecar 周期落盘（2s）+ 暂停/失败即时落盘（NFR-2：kill -9 不损坏、最多丢
//! 一个窗口的进度）。
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_wrap)]
// 字节/速度/时间算术在 u64-f64 间转换是下载器领域固有；边界由调用方保证
#![allow(clippy::missing_const_for_fn)] // nursery 误报为主（含 trait impl 场景）
#![allow(clippy::doc_markdown, clippy::doc_lazy_continuation)] // 中文文档中英文术语不强制反引号
#![allow(clippy::float_cmp)] // 速度/时间为 0 的语义判断使用精确比较
#![allow(clippy::map_unwrap_or, clippy::option_if_let_else, clippy::unnested_or_patterns)]
#![allow(clippy::cognitive_complexity, clippy::too_many_lines)] // 分块计算/状态机逻辑固有复杂度


use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{mpsc, watch};

use crate::model::chunk::{block_range, chunk_total, Blocks};
use crate::model::checksum;
use crate::model::checksum::CHECKSUM_ALGOS;
use crate::model::consistency::{self, Consistency, ServerStamp};
use crate::model::namegen;
use crate::model::sidecar::{Sidecar, SidecarTask};
use crate::model::{Checksum, FailKind, Protocol};

use super::error::{classify_reqwest, parse_retry_after_header, EngineFailure};
use super::{ConnView, Evt, TaskCmd};

/// 进度事件聚合间隔（与 UI tick 同频）
const PROGRESS_TICK: Duration = Duration::from_millis(100);
/// sidecar 周期落盘间隔（NFR-2）
const SIDECAR_FLUSH: Duration = Duration::from_secs(2);
/// 任务运行上下文（App 在 `Cmd::Start` 时构建携带）
#[derive(Clone, Debug)]
pub struct TaskSpec {
    /// 任务 ID
    pub id: u32,
    /// 原始 URL
    pub url: String,
    /// 保存目录
    pub save_dir: String,
    /// 文件名（添加时由 URL 推导；探测后 CD 优先改名）
    pub name: String,
    /// 并发数
    pub concurrency: usize,
    /// 块大小
    pub block_size: u64,
    /// 协议
    pub protocol: Protocol,
    /// 校验期望算法显示名（如有）
    pub expected_algo: Option<&'static str>,
    /// 校验期望值（十六进制小写，如有）
    pub expected_value: Option<String>,
    /// sidecar 内容（App 预读；None = 无断点）
    pub sidecar: Option<Sidecar>,
    /// sidecar 盘上路径（作废断点时删除用；sidecar 存在时必填）
    pub sidecar_path: Option<String>,
    /// 任务添加时间（Unix 秒）
    pub added_at: u64,
}

/// 校验请求上下文（App 在 `Cmd::Verify` 时构建携带）
#[derive(Clone, Debug)]
pub struct VerifySpec {
    /// 任务 ID
    pub id: u32,
    /// 待校验文件（`.downloading` 形态）
    pub path: String,
    /// 最终目标路径（通过后 rename 到此）
    pub final_path: String,
    /// sidecar 路径（通过后删除）
    pub sidecar_path: String,
    /// 算法显示名（`CHECKSUM_ALGOS`）
    pub algo: &'static str,
    /// 期望摘要（十六进制小写）
    pub expected: String,
}

/// supervisor 入口：跑下载流程并按终态收尾（落盘 sidecar + 回报事件）
pub(crate) async fn run(
    spec: TaskSpec,
    shared: Arc<super::EngineShared>,
    mut cmd_rx: mpsc::Receiver<TaskCmd>,
    evt_tx: mpsc::Sender<Evt>,
) {
    match download(&spec, &shared, &mut cmd_rx, &evt_tx).await {
        Flow::Completed => {}
        Flow::Paused(state) => {
            let _ = Sidecar::save(&state.sidecar, &state.sidecar_path);
            let _ = evt_tx
                .send(Evt::PausedDone {
                    id: spec.id,
                    downloaded: state.blocks.downloaded(),
                    chunk_done: state.blocks.completed(),
                })
                .await;
        }
        Flow::Cancelled => {
            // 回报停止确认：App 据此执行延迟文件删除（删除任务和文件）
            let _ = evt_tx.send(Evt::Cancelled { id: spec.id }).await;
        }
        Flow::Failed(f, made_progress, sidecar) => {
            if let Some((sc, sc_path)) = &sidecar {
                let _ = Sidecar::save(sc, sc_path);
            }
            let (downloaded, blocks, chunk_done) = match &sidecar {
                Some((sc, _)) => (sc.downloaded, sc.blocks.clone(), completed_of(sc)),
                None => (0, vec![], 0),
            };
            let _ = evt_tx
                .send(Evt::Failed {
                    id: spec.id,
                    kind: f.kind,
                    reason: f.reason,
                    retry_after: f.retry_after,
                    made_progress,
                    downloaded,
                    blocks,
                    chunk_done,
                })
                .await;
        }
        Flow::Invalidated => {
            // 作废断点：删 sidecar、回报失效（不计失败，FR-01-22）
            if let Some(p) = &spec.sidecar_path {
                let _ = Sidecar::remove(p);
            }
            let _ = evt_tx.send(Evt::Invalidated { id: spec.id }).await;
        }
    }
}

/// sidecar 中已完成块数（Failed 事件汇总用）
fn completed_of(sc: &Sidecar) -> u32 {
    sc.blocks
        .iter()
        .zip(0u32..)
        .filter(|(w, i)| {
            block_range(sc.size, sc.block_size, *i).is_some_and(|(s, e)| **w >= e - s)
        })
        .count() as u32
}

/// sidecar 路径推导
fn sidecar_path(save_dir: &str, name: &str) -> String {
    format!("{}.ezr", namegen::join_path(save_dir, name))
}

/// 下载流程结果
enum Flow {
    /// 完成（DownloadDone 已报）
    Completed,
    /// 暂停（sidecar 已构建待落盘）
    Paused(PausedState),
    /// 取消（删除任务，不落盘）
    Cancelled,
    /// 失败（失败信息 + 是否有进展 + 可选 sidecar 待落盘）
    Failed(EngineFailure, bool, Option<(Sidecar, String)>),
    /// 一致性失效（FR-01-22）
    Invalidated,
}

/// 暂停态
struct PausedState {
    blocks: Blocks,
    sidecar: Sidecar,
    sidecar_path: String,
}

/// 事件发送辅助（App 退出后静默）
async fn emit(evt: &mpsc::Sender<Evt>, e: Evt) -> bool {
    evt.send(e).await.is_ok()
}

/// 下载请求构建（公共头：禁用内容压缩，FR-01-16）
fn req_with_identity(
    shared: &super::EngineShared,
    url: &str,
    range: Option<String>,
) -> reqwest::RequestBuilder {
    let mut rb = shared
        .client
        .get(url)
        .header("Accept-Encoding", "identity")
        .header("User-Agent", concat!("ezr/", env!("CARGO_PKG_VERSION")));
    if let Some(r) = range {
        rb = rb.header("Range", r);
    }
    rb
}

/// 探测响应头快照
struct ProbeHead {
    status: u16,
    total: u64,
    etag: Option<String>,
    last_modified: Option<String>,
    cd_name: Option<String>,
    retry_after: Option<f64>,
}

/// 解析探测响应头（GET Range: bytes=0-）
fn probe_head(r: &reqwest::Response) -> ProbeHead {
    let h = r.headers();
    ProbeHead {
        status: r.status().as_u16(),
        total: h
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(0),
        etag: h.get("etag").and_then(|v| v.to_str().ok()).map(str::to_string),
        last_modified: h.get("last-modified").and_then(|v| v.to_str().ok()).map(str::to_string),
        cd_name: h
            .get("content-disposition")
            .and_then(|v| v.to_str().ok())
            .and_then(namegen::from_content_disposition),
        retry_after: parse_retry_after_header(h.get("retry-after").and_then(|v| v.to_str().ok())),
    }
}

/// 下载主流程
async fn download(
    spec: &TaskSpec,
    shared: &Arc<super::EngineShared>,
    cmd_rx: &mut mpsc::Receiver<TaskCmd>,
    evt_tx: &mpsc::Sender<Evt>,
) -> Flow {
    // ---- 探测（FR-01-10）----
    let resp = match req_with_identity(shared, &spec.url, Some("bytes=0-".into())).send().await {
        Ok(r) => r,
        Err(e) => return Flow::Failed(classify_reqwest(&e), false, None),
    };
    let head = probe_head(&resp);
    if !(200..=299).contains(&head.status) {
        return Flow::Failed(EngineFailure::http(head.status, head.retry_after), false, None);
    }
    let probed_stamp = ServerStamp {
        final_url: Some(resp.url().as_str().to_string()),
        etag: head.etag.clone(),
        last_modified: head.last_modified.clone(),
        size: (head.total > 0).then_some(head.total),
    };

    // 文件名定稿（FR-01-02：CD → 最终 URL 末段 → 原始 URL 末段 → 时间戳）；
    // 断点接续（spec 带 sidecar）沿用既有任务名，避免另存新文件（FR-01-26）。
    // 非接续路径定稿前做盘上去重（Gherkin 01-add-task-10：目标已存在时追加
    // 序号，防止下载覆盖既有文件；与 App 添加期去重同口径、结果一致）
    let name = if spec.sidecar.is_some() {
        spec.name.clone()
    } else {
        let base = namegen::derive_name(
            head.cd_name.as_deref(),
            Some(resp.url().as_str()),
            &spec.url,
        );
        namegen::dedupe(&base, |n| namegen::exists_on_disk(&spec.save_dir, n))
    };

    // ---- 续传一致性检查（FR-01-22/D11；sidecar 名与推导名一致才可接续）----
    let sc = spec
        .sidecar
        .clone()
        .filter(|_| name == spec.name || spec.sidecar.as_ref().is_some_and(|s| s.task.id == spec.id));
    let blocks = if let Some(sidecar) = &sc {
        match consistency::check(&sidecar.stamp(), &probed_stamp) {
            Consistency::Valid => {
                let y = chunk_total(sidecar.size, sidecar.block_size);
                if y == sidecar.block_count && sidecar.size == head.total {
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
        Blocks::new(head.total, spec.block_size)
    };

    let y = blocks.count();
    let multi_ok = head.status == 206 && head.total > 0;
    // 探测完成（App 更新 final_url/total/resumable/probed 与大小显示）
    if !emit(evt_tx, Evt::Probed {
        id: spec.id,
        name: name.clone(),
        final_url: resp.url().as_str().to_string(),
        total: head.total,
        resumable: multi_ok,
        etag: head.etag.clone(),
        last_modified: head.last_modified.clone(),
        cd_name: head.cd_name.clone(),
    })
    .await
    {
        return Flow::Cancelled;
    }

    let dir = PathBuf::from(&spec.save_dir);
    // 保存目录自动创建（Gherkin 01-add-task-11「目录不存在自动创建」：
    // 引擎打开目标文件前确保父目录存在，覆盖默认下载目录与用户新输入目录）
    let _ = tokio::fs::create_dir_all(&dir).await;
    let dl_path = dir.join(format!("{name}.downloading"));
    let final_path = dir.join(&name);
    let sc_path = sidecar_path(&spec.save_dir, &name);

    // 单流路径：不支持 Range（200）或单块（y <= 1）
    if !multi_ok || y <= 1 {
        return single_stream(
            spec,
            shared,
            resp,
            blocks,
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

    // ---- 分块下载（FR-01-11/13）----
    // 预分配稀疏文件（续传时文件可能已存在：open 不截断，set_len 保持内容）
    match tokio::fs::OpenOptions::new().write(true).create(true).truncate(false).open(&dl_path).await {
        Ok(f) => {
            if let Err(e) = f.set_len(head.total).await {
                return Flow::Failed(
                    EngineFailure::fatal(format!("无法预分配文件（{e}）")),
                    false,
                    None,
                );
            }
        }
        Err(e) => {
            return Flow::Failed(
                EngineFailure::fatal(format!("无法创建目标文件（{e}）")),
                false,
                None,
            )
        }
    }

    let total = head.total;
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (stop_tx, stop_rx) = watch::channel(false);
    let blocks_shared = Arc::new(tokio::sync::Mutex::new(blocks.clone()));
    let lease_next = Arc::new(AtomicU32::new(0));
    let total_written = Arc::new(AtomicU64::new(0));
    let failure = Arc::new(tokio::sync::Mutex::<Option<EngineFailure>>::new(None));
    let conns = Arc::new(tokio::sync::Mutex::<Vec<ConnView>>::new(Vec::new()));

    // worker 池：按设置并发数生成 worker（FR-01-11）；块数不足时多余 worker
    // 立即转「待命」（明细表可见），活跃传输数仍 = min(并发数, 未完成块数)
    let n_workers = spec.concurrency.clamp(1, 64);
    let mut handles = Vec::with_capacity(n_workers);
    for wid in 1..=n_workers {
        handles.push(tokio::spawn(block_worker(
            wid,
            Arc::clone(shared),
            resp.url().as_str().to_string(),
            dl_path.clone(),
            blocks_shared.clone(),
            lease_next.clone(),
            stop_rx.clone(),
            total_written.clone(),
            failure.clone(),
            conns.clone(),
        )));
    }

    // 监督循环：进度事件 + sidecar 周期落盘 + 完成/暂停/取消判定
    let mut flush_deadline = tokio::time::Instant::now() + SIDECAR_FLUSH;
    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => match cmd {
                Some(TaskCmd::Pause) | Some(TaskCmd::Cancel) => {
                    let cancelled = cmd == Some(TaskCmd::Cancel);
                    stop.store(true, Ordering::Relaxed);
                    let _ = stop_tx.send(true);
                    for h in handles.drain(..) {
                        let _ = h.await;
                    }
                    let blocks = blocks_shared.lock().await.clone();
                    if cancelled {
                        return Flow::Cancelled;
                    }
                    let sidecar =
                        build_sidecar(spec, &name, &probed_stamp, &blocks, false);
                    return Flow::Paused(PausedState {
                        blocks,
                        sidecar,
                        sidecar_path: sc_path.clone(),
                    });
                }
                _ => return Flow::Cancelled,
            },
            _ = tokio::time::sleep(PROGRESS_TICK) => {
                if failure.lock().await.is_some() {
                    stop.store(true, Ordering::Relaxed);
                    let _ = stop_tx.send(true);
                    for h in handles.drain(..) {
                        let _ = h.await;
                    }
                    let f = failure.lock().await.clone().unwrap_or_else(|| {
                        EngineFailure::network("传输中断")
                    });
                    let blocks = blocks_shared.lock().await.clone();
                    let sidecar = build_sidecar(spec, &name, &probed_stamp, &blocks, false);
                    return Flow::Failed(
                        f,
                        total_written.load(Ordering::Relaxed) > 0,
                        Some((sidecar, sc_path.clone())),
                    );
                }
                let b = blocks_shared.lock().await;
                let downloaded = b.downloaded();
                let chunk_done = b.completed();
                let conns_now = conns.lock().await.clone();
                drop(b);
                if !emit(evt_tx, Evt::Progress {
                    id: spec.id,
                    downloaded,
                    conns: conns_now,
                    chunk_done,
                }).await {
                    return Flow::Cancelled;
                }
                if chunk_done == y && y > 0 {
                    // 全块完成 → 大小校验（FR-01-52）
                    if downloaded != total {
                        let f = EngineFailure {
                            kind: FailKind::Transient,
                            reason: format!(
                                "文件大小不符（已接收 {downloaded} B ≠ Content-Length {total} B）"
                            ),
                            retry_after: None,
                        };
                        stop.store(true, Ordering::Relaxed);
                        let _ = stop_tx.send(true);
                        for h in handles.drain(..) {
                            let _ = h.await;
                        }
                        let blocks = blocks_shared.lock().await.clone();
                        let sidecar = build_sidecar(spec, &name, &probed_stamp, &blocks, false);
                        return Flow::Failed(
                            f,
                            total_written.load(Ordering::Relaxed) > 0,
                            Some((sidecar, sc_path.clone())),
                        );
                    }
                    stop.store(true, Ordering::Relaxed);
                    let _ = stop_tx.send(true);
                    for h in handles.drain(..) {
                        let _ = h.await;
                    }
                    let has_checksum = spec.expected_algo.is_some();
                    if !has_checksum {
                        // 无校验值直接完成：此处收尾——去 `.downloading` 扩展名 +
                        // 删 sidecar（FR-01-24/20）；有校验值时由 verify() 收尾
                        let _ = tokio::fs::rename(&dl_path, &final_path).await;
                        let _ = Sidecar::remove(&sc_path);
                    }
                    let _ = emit(evt_tx, Evt::DownloadDone {
                        id: spec.id,
                        total,
                        has_checksum,
                    }).await;
                    return Flow::Completed;
                }
                // sidecar 周期落盘
                if tokio::time::Instant::now() >= flush_deadline {
                    flush_deadline = tokio::time::Instant::now() + SIDECAR_FLUSH;
                    let b2 = blocks_shared.lock().await;
                    let sidecar = build_sidecar(spec, &name, &probed_stamp, &b2, false);
                    drop(b2);
                    let _ = Sidecar::save(&sidecar, &sc_path);
                }
            }
        }
    }
}

/// 构建 sidecar（下载中/暂停/失败共用）
#[allow(clippy::needless_pass_by_value)]
fn build_sidecar(
    spec: &TaskSpec,
    _name: &str,
    stamp: &ServerStamp,
    blocks: &Blocks,
    non_resumable: bool,
) -> Sidecar {
    let expected = spec
        .expected_algo
        .as_ref()
        .zip(spec.expected_value.as_ref())
        .map(|(a, v)| Checksum { algo: a, value: v.clone() });
    Sidecar::build(
        &spec.url,
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

/// 单流下载路径（不支持 Range 或单块；FR-01-12 降级：从头整体下载）
#[allow(clippy::too_many_arguments)]
async fn single_stream(
    spec: &TaskSpec,
    shared: &Arc<super::EngineShared>,
    resp: reqwest::Response,
    blocks: Blocks,
    dl_path: &Path,
    final_path: &Path,
    sc_path: &str,
    name: &str,
    stamp: &ServerStamp,
    cmd_rx: &mut mpsc::Receiver<TaskCmd>,
    evt_tx: &mpsc::Sender<Evt>,
) -> Flow {
    let _ = emit(evt_tx, Evt::Toast(format!("⚠ 服务器不支持断点续传，单线程下载: {name}"))).await;
    let mut file = match tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(dl_path)
        .await
    {
        Ok(f) => f,
        Err(e) => return Flow::Failed(EngineFailure::fatal(format!("无法创建目标文件（{e}）")), false, None),
    };
    let mut downloaded: u64 = 0;
    let mut resp = resp;
    // 单流进度上报节流（FR-01-17/81：不支持续传的任务同样需要真实进度/速度/连接数）
    let mut last_emit = std::time::Instant::now();
    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => match cmd {
                Some(TaskCmd::Pause) => {
                    let sidecar = build_sidecar(spec, name, stamp, &blocks, true);
                    return Flow::Paused(PausedState {
                        blocks,
                        sidecar,
                        sidecar_path: sc_path.to_string(),
                    });
                }
                Some(TaskCmd::Cancel) | None => return Flow::Cancelled,
            },
            chunk = resp.chunk() => match chunk {
                Ok(Some(bytes)) => {
                    shared.throttle.acquire(bytes.len() as u64).await;
                    if file.write_all(&bytes).await.is_err() {
                        return Flow::Failed(EngineFailure::fatal("磁盘写入失败"), downloaded > 0, None);
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
                            None,
                        );
                    }
                    let final_total = if blocks.total == 0 { downloaded } else { blocks.total };
                    if spec.expected_algo.is_none() {
                        // 无校验值直接完成：收尾（FR-01-24/20）
                        let _ = tokio::fs::rename(dl_path, final_path).await;
                        let _ = Sidecar::remove(sc_path);
                    }
                    let _ = emit(evt_tx, Evt::DownloadDone {
                        id: spec.id,
                        total: final_total,
                        has_checksum: spec.expected_algo.is_some(),
                    }).await;
                    return Flow::Completed;
                }
                Err(e) => {
                    return Flow::Failed(classify_reqwest(&e), downloaded > 0, None);
                }
            },
        }
    }
}

/// 校验（App 转校验中后下发；流式哈希不整载内存，FR-01-51）
pub(crate) async fn verify(
    spec: VerifySpec,
    evt_tx: mpsc::Sender<Evt>,
) {
    let path = spec.path.clone();
    let algo_idx = CHECKSUM_ALGOS
        .iter()
        .position(|(disp, _, _)| *disp == spec.algo)
        .unwrap_or(3);
    let computed = tokio::task::spawn_blocking(move || checksum::digest_file(&path, algo_idx)).await
        .unwrap_or_else(|e| Err(format!("校验任务失败（{e}）")));
    match computed {
        Ok(hex) => {
            let ok = hex.eq_ignore_ascii_case(&spec.expected);
            if ok {
                // 收尾：去 `.downloading` 扩展名 + 删 sidecar（FR-01-24/20）
                let _ = tokio::fs::rename(&spec.path, &spec.final_path).await;
                let _ = Sidecar::remove(&spec.sidecar_path);
            }
            let _ = evt_tx
                .send(Evt::VerifyDone {
                    id: spec.id,
                    ok,
                    computed: hex,
                    expected: spec.expected,
                })
                .await;
        }
        Err(e) => {
            // 文件不可读（被外部删除等）：按校验失败停等
            let _ = evt_tx
                .send(Evt::VerifyDone { id: spec.id, ok: false, computed: e, expected: spec.expected })
                .await;
        }
    }
}

/// 块 worker：领块 → Range 下载 → 写盘；无块可领或停止信号时退出
#[allow(clippy::too_many_arguments)]
async fn block_worker(
    wid: usize,
    shared: Arc<super::EngineShared>,
    url: String,
    file_path: PathBuf,
    blocks: Arc<tokio::sync::Mutex<Blocks>>,
    lease_next: Arc<AtomicU32>,
    stop_rx: watch::Receiver<bool>,
    total_written: Arc<AtomicU64>,
    failure: Arc<tokio::sync::Mutex<Option<EngineFailure>>>,
    conns: Arc<tokio::sync::Mutex<Vec<ConnView>>>,
) {
    let file = match tokio::fs::OpenOptions::new().write(true).open(&file_path).await {
        Ok(f) => f,
        Err(_) => {
            *failure.lock().await = Some(EngineFailure::fatal("无法写入目标文件"));
            return;
        }
    };
    let mut file = file;
    let mut stop_rx = stop_rx;
    loop {
        if *stop_rx.borrow() {
            return;
        }
        // 领块：从 lease_next 顺序扫描第一个未完成块
        let lease = {
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
                let (s, e) = block_range(b.total, b.piece, idx).unwrap_or((0, 0));
                let written = b.written.get(idx as usize).copied().unwrap_or(0);
                (idx, s, e, written)
            })
        };
        let Some((idx, start, end, written)) = lease else {
            // 队列已空：连接转待命（FR-01-11）——保留空连接条目（cap=0）供
            // 明细表「待命」展示；块队列单调递减，worker 不会复活，条目存活至任务收尾
            let mut g = conns.lock().await;
            g.retain(|c| c.id != wid);
            g.push(ConnView { id: wid, block: 0, start: 0, end: 0, done: 0 });
            drop(g);
            return;
        };
        conns.lock().await.retain(|c| c.id != wid);
        conns.lock().await.push(ConnView { id: wid, block: idx, start, end, done: written });
        let range = format!("bytes={}-{}", start + written, end - 1);
        let resp = req_with_identity(&shared, &url, Some(range)).send().await;
        let mut resp = match resp {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let status = r.status().as_u16();
                let ra = parse_retry_after_header(
                    r.headers().get("retry-after").and_then(|v| v.to_str().ok()),
                );
                let _ = r.bytes().await;
                *failure.lock().await = Some(EngineFailure::http(status, ra));
                return;
            }
            Err(e) => {
                *failure.lock().await = Some(classify_reqwest(&e));
                return;
            }
        };
        // 流式写盘（seek 到块内偏移；每 worker 独立句柄，FR-01-13）
        let _ = file.seek(std::io::SeekFrom::Start(start + written)).await;
        let mut done_here = written;
        let mut aborted = false;
        loop {
            tokio::select! {
                changed = stop_rx.changed() => {
                    if changed.is_err() || *stop_rx.borrow() {
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
                                // 连接视图即时回填（FR-01-81）：每连接进度/速度滑窗依赖
                                // ConnView.done 在传输中持续增长，否则明细表恒显「挂起」
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
        // 记录块内进度（部分块保留已写字节，FR-01-13）
        blocks.lock().await.mark_progress(idx, done_here);
        if aborted || failure.lock().await.is_some() {
            return;
        }
        blocks.lock().await.mark_done(idx);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    /// 确定性内容（与 fixture 同模式）
    fn fill(buf: &mut [u8], start: u64) {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = ((start as usize + i) % 251) as u8;
        }
    }

    /// mock HTTP 服务器：支持 Range/状态码注入/慢速；返回 (地址, 关闭标志)。
    /// 线程 detach（engineering.md：无退出条件的测试线程一律 detach）。
    fn mock_server(total: u64, opts: Vec<(String, String)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                let mut w = conn.try_clone().unwrap();
                let mut r = BufReader::new(conn);
                let mut line = String::new();
                if r.read_line(&mut line).is_err() || line.is_empty() {
                    continue;
                }
                let mut range_header = None;
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).is_err() || h.trim().is_empty() {
                        break;
                    }
                    if let Some((k, v)) = h.split_once(':') {
                        if k.trim().eq_ignore_ascii_case("range") {
                            range_header = Some(v.trim().to_string());
                        }
                    }
                }
                let want_status = opts
                    .iter()
                    .find(|(k, _)| k == "status")
                    .and_then(|(_, v)| v.parse::<u16>().ok());
                if let Some(code) = want_status {
                    let _ = write!(w, "HTTP/1.1 {code} Injected\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    continue;
                }
                let norange = opts.iter().any(|(k, _)| k == "norange");
                let (start, end, status) = match range_header.as_deref().and_then(|v| v.strip_prefix("bytes=")) {
                    Some(rv) if !norange => {
                        let (a, b) = rv.split_once('-').unwrap();
                        let s = a.parse::<u64>().unwrap_or(0);
                        let e = if b.is_empty() { total - 1 } else { b.parse::<u64>().unwrap_or(total - 1).min(total - 1) };
                        (s, e, 206)
                    }
                    _ => (0, total - 1, 200),
                };
                let len = end - start + 1;
                let _ = write!(
                    w,
                    "HTTP/1.1 {status} OK\r\nAccept-Ranges: bytes\r\nETag: \"mock-{total}\"\r\nLast-Modified: Mon, 09 Feb 2026 08:00:00 GMT\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
                );
                let mut buf = vec![0u8; 8192];
                let mut pos = start;
                while pos <= end {
                    let n = (end - pos + 1).min(8192) as usize;
                    fill(&mut buf[..n], pos);
                    if w.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    pos += n as u64;
                }
            }
        });
        format!("http://{addr}/f.bin")
    }

    fn spec_for(id: u32, url: &str, dir: &str, block: u64, conc: usize, expected: Option<(&'static str, String)>) -> TaskSpec {
        TaskSpec {
            id,
            url: url.to_string(),
            save_dir: dir.to_string(),
            name: "f.bin".to_string(),
            concurrency: conc,
            block_size: block,
            protocol: Protocol::Http,
            expected_algo: expected.as_ref().map(|(a, _)| *a),
            expected_value: expected.as_ref().map(|(_, v)| v.clone()),
            sidecar: None,
            sidecar_path: None,
            added_at: 0,
        }
    }

    fn throttle_free() -> Arc<super::super::EngineShared> {
        Arc::new(super::super::EngineShared {
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            throttle: Arc::new(super::super::throttle::Throttle::new(0)),
        })
    }

    #[tokio::test]
    async fn end_to_end_resumable_download_completes() {
        let dir = std::env::temp_dir().join(format!("ezr-e2e-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let total: u64 = 3 * 4096 + 1111; // 4 块（块 4KB），末块吸收余数
        let url = mock_server(total, vec![]);
        let (tx, mut rx) = mpsc::channel::<Evt>(64);
        let shared = throttle_free();
        let (_cmd_tx, cmd_rx) = mpsc::channel::<TaskCmd>(4);
        let spec = spec_for(1, &url, dir.to_str().unwrap(), 4096, 4, None);
        tokio::spawn(run(spec, shared, cmd_rx, tx));
        let mut done = false;
        let mut probed = false;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::DownloadDone { total: t, has_checksum: false, .. })) => {
                    assert_eq!(t, total);
                    done = true;
                    break;
                }
                Ok(Some(Evt::Probed { total: t, resumable: true, .. })) => {
                    assert_eq!(t, total);
                    probed = true;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                Ok(Some(_)) => {}
                _ => {}
            }
        }
        assert!(probed, "未收到探测事件");
        assert!(done, "未收到完成事件");
        // 文件字节一致（.downloading 改名后为 f.bin；无校验引擎已 finalize）
        let path = dir.join("f.bin");
        let data = std::fs::read(&path).unwrap();
        assert_eq!(data.len() as u64, total);
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect, "文件内容应逐字节一致");
        // sidecar 已删（完成收尾）
        assert!(!dir.join("f.bin.ezr").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn cancel_emits_cancelled_and_no_ghost_progress() {
        let dir = std::env::temp_dir().join(format!("ezr-e2e-cancel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let total: u64 = 3 * 4096 + 1111;
        let url = mock_server(total, vec![]);
        let (tx, mut rx) = mpsc::channel::<Evt>(64);
        let shared = throttle_free();
        let (cmd_tx, cmd_rx) = mpsc::channel::<TaskCmd>(4);
        let spec = spec_for(9, &url, dir.to_str().unwrap(), 4096, 4, None);
        // 取消命令先于 run 入队：探测后首个 select 轮询即命中取消
        //（当前线程 runtime 下 worker 尚未被轮询，杜绝完成/进度竞态）
        cmd_tx.send(TaskCmd::Cancel).await.unwrap();
        tokio::spawn(run(spec, shared, cmd_rx, tx));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut probed = false;
        let mut cancelled = false;
        let mut ghost = false;
        let mut done = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Probed { resumable: true, .. })) => probed = true,
                Ok(Some(Evt::Cancelled { .. })) => {
                    cancelled = true;
                    break;
                }
                Ok(Some(Evt::Progress { .. })) => ghost = true,
                Ok(Some(Evt::DownloadDone { .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                Ok(Some(_)) => {}
                _ => {}
            }
        }
        assert!(!done, "取消后不应完成下载");
        assert!(!ghost, "取消后不应再有进度事件");
        assert!(probed, "未收到探测事件");
        assert!(cancelled, "未收到取消确认事件（Evt::Cancelled）");
        // Flow::Cancelled 路径不 finalize、不写 sidecar：最终文件与 .ezr 都不应出现
        assert!(!dir.join("f.bin").exists(), "取消不应产出最终文件");
        assert!(!dir.join("f.bin.ezr").exists(), "取消路径不应写 sidecar");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn norange_server_falls_back_single_stream() {
        let dir = std::env::temp_dir().join(format!("ezr-e2e2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let total: u64 = 9000;
        let url = mock_server(total, vec![("norange".to_string(), "1".to_string())]);
        let (tx, mut rx) = mpsc::channel::<Evt>(64);
        let shared = throttle_free();
        let (_cmd_tx, cmd_rx) = mpsc::channel::<TaskCmd>(4);
        let spec = spec_for(2, &url, dir.to_str().unwrap(), 4096, 4, None);
        tokio::spawn(run(spec, shared, cmd_rx, tx));
        let mut done = false;
        let mut non_resumable = false;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::DownloadDone { total: t, .. })) => {
                    assert_eq!(t, total);
                    done = true;
                    break;
                }
                Ok(Some(Evt::Probed { resumable, .. })) => {
                    if !resumable {
                        non_resumable = true;
                    }
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(non_resumable, "应探测为不支持续传");
        assert!(done, "单流应完成");
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        assert_eq!(data.len() as u64, total);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn status_503_fails_transient() {
        let dir = std::env::temp_dir().join(format!("ezr-e2e3-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let url = mock_server(1000, vec![("status".to_string(), "503".to_string())]);
        let (tx, mut rx) = mpsc::channel::<Evt>(64);
        let shared = throttle_free();
        let (_cmd_tx, cmd_rx) = mpsc::channel::<TaskCmd>(4);
        let spec = spec_for(3, &url, dir.to_str().unwrap(), 4096, 2, None);
        tokio::spawn(run(spec, shared, cmd_rx, tx));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Failed { kind, reason, .. })) => {
                    assert_eq!(kind, FailKind::Transient, "503 应为可自动重试类: {reason}");
                    return;
                }
                Ok(Some(Evt::Toast(_))) => {}
                _ => {}
            }
        }
        panic!("503 应产生失败事件");
    }

    #[tokio::test]
    async fn verify_computes_matching_digest() {
        // 校验通过路径：expected = 真实摘要（模式填充内容）
        let dir = std::env::temp_dir().join(format!("ezr-e2e4-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let total: u64 = 2 * 4096;
        let url = mock_server(total, vec![]);
        // 先下载（无校验值）生成文件
        let (tx, mut rx) = mpsc::channel::<Evt>(64);
        let shared = throttle_free();
        let (_cmd_tx, cmd_rx) = mpsc::channel::<TaskCmd>(4);
        let spec = spec_for(4, &url, dir.to_str().unwrap(), 4096, 2, None);
        tokio::spawn(run(spec, shared, cmd_rx, tx));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::DownloadDone { .. })) => break,
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                Err(_) => panic!("超时"),
                _ => {}
            }
            if std::time::Instant::now() > deadline {
                panic!("超时");
            }
        }
        // 模拟校验中状态：文件在下载完成后被 rename；重命名为 .downloading 以走 VerifySpec
        let final_path = dir.join("f.bin");
        let dl = dir.join("f.bin.downloading");
        std::fs::rename(&final_path, &dl).unwrap();
        let digest = checksum::digest_file(dl.to_str().unwrap(), 3).unwrap();
        let vspec = VerifySpec {
            id: 4,
            path: dl.to_str().unwrap().to_string(),
            final_path: final_path.to_str().unwrap().to_string(),
            sidecar_path: dir.join("f.bin.ezr").to_str().unwrap().to_string(),
            algo: "SHA-256",
            expected: digest.clone(),
        };
        let (tx2, mut rx2) = mpsc::channel::<Evt>(8);
        verify(vspec, tx2).await;
        match rx2.recv().await {
            Some(Evt::VerifyDone { ok: true, computed, .. }) => assert_eq!(computed, digest),
            other => panic!("校验应通过: {other:?}"),
        }
        assert!(final_path.exists(), "通过后应 rename 到最终名");
        assert!(!dl.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn sidecar_resume_continues_from_breakpoint() {
        // 断点接续：预置 sidecar（前 1.5 块已写）→ 探测一致 → 续传 → 完成
        let dir = std::env::temp_dir().join(format!("ezr-e2e5-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let total: u64 = 4 * 4096; // 4 块
        let url = mock_server(total, vec![]);
        // 盘上预写 .downloading（真实内容前 6000 字节）+ sidecar（块 0 完成、块 1 部分写 1904）
        let mut content = vec![0u8; total as usize];
        fill(&mut content, 0);
        std::fs::write(dir.join("f.bin.downloading"), &content).unwrap();
        let stamp = ServerStamp {
            final_url: Some(url.clone()),
            etag: Some(format!("\"mock-{total}\"")),
            last_modified: Some("Mon, 09 Feb 2026 08:00:00 GMT".to_string()),
            size: Some(total),
        };
        let sc = Sidecar::build(
            &url,
            &stamp,
            total,
            4096,
            &[4096, 1904, 0, 0],
            false,
            None,
            SidecarTask { id: 5, added_at: 0, save_dir: dir.to_str().unwrap().to_string(), concurrency: 2, protocol: Protocol::Http },
        );
        sc.save(dir.join("f.bin.ezr").to_str().unwrap()).unwrap();
        let (tx, mut rx) = mpsc::channel::<Evt>(64);
        let shared = throttle_free();
        let (_cmd_tx, cmd_rx) = mpsc::channel::<TaskCmd>(4);
        let mut spec = spec_for(5, &url, dir.to_str().unwrap(), 4096, 2, None);
        spec.sidecar = Some(Sidecar::load(dir.join("f.bin.ezr").to_str().unwrap()).unwrap());
        spec.sidecar_path = Some(dir.join("f.bin.ezr").to_str().unwrap().to_string());
        tokio::spawn(run(spec, shared, cmd_rx, tx));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut resumed_progress = None;
        loop {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Progress { downloaded, .. })) => {
                    resumed_progress.get_or_insert(downloaded);
                }
                Ok(Some(Evt::DownloadDone { total: t, .. })) => {
                    assert_eq!(t, total);
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("一致快照不应失效"),
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                Err(_) => panic!("超时"),
                _ => {}
            }
            if std::time::Instant::now() > deadline {
                panic!("超时");
            }
        }
        // 续传起点应 ≥ sidecar 已写字节（6000），证明从断点续传（未从头）
        assert!(resumed_progress.unwrap_or(0) >= 6000, "应从断点续传: {resumed_progress:?}");
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn changed_etag_invalidates_sidecar() {
        // 一致性失效：sidecar ETag 与探测不符 → Invalidated
        let dir = std::env::temp_dir().join(format!("ezr-e2e6-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let total: u64 = 2 * 4096;
        let url = mock_server(total, vec![]);
        let stamp = ServerStamp {
            final_url: Some(url.clone()),
            etag: Some("\"stale\"".to_string()),
            last_modified: None,
            size: Some(total),
        };
        let sc = Sidecar::build(
            &url,
            &stamp,
            total,
            4096,
            &[4096, 0],
            false,
            None,
            SidecarTask { id: 6, added_at: 0, save_dir: dir.to_str().unwrap().to_string(), concurrency: 1, protocol: Protocol::Http },
        );
        sc.save(dir.join("f.bin.ezr").to_str().unwrap()).unwrap();
        let (tx, mut rx) = mpsc::channel::<Evt>(64);
        let shared = throttle_free();
        let (_cmd_tx, cmd_rx) = mpsc::channel::<TaskCmd>(4);
        let mut spec = spec_for(6, &url, dir.to_str().unwrap(), 4096, 1, None);
        spec.sidecar = Some(Sidecar::load(dir.join("f.bin.ezr").to_str().unwrap()).unwrap());
        spec.sidecar_path = Some(dir.join("f.bin.ezr").to_str().unwrap().to_string());
        tokio::spawn(run(spec, shared, cmd_rx, tx));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Invalidated { .. })) => return,
                Ok(Some(Evt::Failed { reason, .. })) => panic!("不应失败: {reason}"),
                _ => {}
            }
        }
        panic!("应回报一致性失效");
    }
}
