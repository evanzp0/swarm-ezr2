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

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{mpsc, watch};

use super::error::{classify_reqwest, parse_retry_after_header, EngineFailure};
use super::{ConnView, Evt, TaskCmd};
use crate::model::chunk::{block_range, chunk_total, Blocks, MIN_HTTP_BLOCK_SIZE};
use crate::model::config::ProxyEndpoint;
use crate::model::consistency::{self, Consistency, ServerStamp};
use crate::model::namegen::DOWNLOADING_EXT;
use crate::model::sidecar::{Sidecar, SidecarTask};
use crate::model::{checksum, namegen, Checksum, FailKind, Protocol};

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
    /// 沿用既有任务名（探测后的重启：失效重下/续传）；false = 首次启动，
    /// 按规则推导并盘上去重（FR-01-02）。失效重启若重新去重，会因残留的
    /// `.downloading` 文件漂移到新名（m.bin → m.bin.1…），导致 sidecar 落盘
    /// 名与任务名错位、下轮续传一致性检查被跳过（FR-01-22 违例），故必须钉住。
    pub keep_name: bool,
    /// 任务添加时间（Unix 秒）
    pub added_at: u64,
    /// 任务级代理端点（v1.5/FR-01-86/D18；None = 直连。App 已解析三态选择；
    /// url 合法性由引擎在 start_task 时校验，非法回退直连 + toast）
    pub proxy_endpoint: Option<ProxyEndpoint>,
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
            let (downloaded, chunk_done) = match &sidecar {
                Some((sc, _)) => (sc.downloaded, completed_of(sc)),
                None => (0, 0),
            };
            let _ = evt_tx
                .send(Evt::Failed {
                    id: spec.id,
                    kind: f.kind,
                    reason: f.reason,
                    retry_after: f.retry_after,
                    made_progress,
                    downloaded,
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
        .filter(|(w, i)| block_range(sc.size, sc.block_size, *i).is_some_and(|(s, e)| **w >= e - s))
        .count() as u32
}

/// sidecar 路径推导（委托 model 单源 namegen::sidecar_path_of，architect v116：
/// 原 `.ezr` 后缀手拼与模型 Task 方法重复）
fn sidecar_path(save_dir: &str, name: &str) -> String {
    namegen::sidecar_path_of(save_dir, name)
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

/// 下载请求构建（公共头：禁用内容压缩，FR-01-16）。client 由调用方按当前
/// 任务代理端点解析（v1.5/FR-01-86：client 缓存池按端点查池）。
fn req_with_identity(
    client: &reqwest::Client,
    url: &str,
    range: Option<String>,
) -> reqwest::RequestBuilder {
    let mut rb = client
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
        etag: h
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string),
        last_modified: h
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string),
        cd_name: h
            .get("content-disposition")
            .and_then(|v| v.to_str().ok())
            .and_then(namegen::from_content_disposition),
        retry_after: parse_retry_after_header(h.get("retry-after").and_then(|v| v.to_str().ok())),
    }
}

/// 停止全部 worker（stop 标志 + watch 广播 + 收割句柄）；监督循环各退出路径共用
async fn stop_workers(
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

/// 收割已结束 worker 句柄（待命退出/配额下调退出；监督循环 tick 与
/// Reconfigure 时调用，保持存活集合与槽位号准确）
fn prune_finished(handles: &mut Vec<(usize, tokio::task::JoinHandle<()>)>) {
    handles.retain(|(_, h)| !h.is_finished());
}

/// 补足 worker 至目标配额 q（v1.5/FR-01-87 扩容 spawn 单源；v1.16/D30
/// 进度 tick 兜底对账复用——缩容侧 worker 退出收割与扩容 spawn 之间的
/// 瞬时缺口由下一进度帧自愈）。槽位号取最小空闲编号（退出可复用）
fn fill_workers(
    handles: &mut Vec<(usize, tokio::task::JoinHandle<()>)>,
    deps: &WorkerDeps,
    shared: &Arc<super::EngineShared>,
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

/// 下载主流程
async fn download(
    spec: &TaskSpec,
    shared: &Arc<super::EngineShared>,
    cmd_rx: &mut mpsc::Receiver<TaskCmd>,
    evt_tx: &mpsc::Sender<Evt>,
) -> Flow {
    // ---- 探测（FR-01-10；探测与下载同走任务级代理端点，FR-01-86）----
    // v1.25：探测请求与任务命令同 select（修复：探测在途不消费命令）——
    // 慢探测（连接超时 15s）期间 Pause/Cancel/替换即时生效；重复 Start
    // 替换时存活旧 supervisor 立即停机（句柄替换后全部 sender 丢弃，
    // recv 返回 None），探测失败不再补发迟到 Failed（App 侧重试计数被
    // 幽灵事件重复推进、冲破上限的根因），探测成功不再孵化幽灵 worker
    // 与新 supervisor 并发写盘（进度回退根因）。探测期无已写进度，
    // 按 Cancelled 收尾时盘上 sidecar 维持前值（暂停恢复语义不变）。
    let probe_client = shared.client_for(spec.proxy_endpoint.as_ref());
    let resp = loop {
        tokio::select! {
            r = req_with_identity(&probe_client, &spec.url, Some("bytes=0-".into())).send() => match r {
                Ok(r) => break r,
                // v1.27：探测失败 = 未核验 ≠ 断点消失——spec 携带的台账是唯一
                // 持久化事实，随失败事件回报（0/0 会让 App 的事件口径覆盖
                // 分块视图 →「已下载 5.6 MB / 分块 0/11」自相矛盾，操作者
                // 实证缺陷）；台账原样重存（内容不变），核验留给下次探测
                Err(e) => {
                    return Flow::Failed(
                        classify_reqwest(&e),
                        false,
                        spec.sidecar.clone().zip(spec.sidecar_path.clone()),
                    )
                }
            },
            // Reconfigure 探测期无热调语义（spec 已快照，并发/代理改动随
            // 下次 Start 生效）：继续探测；Pause/Cancel/通道关闭（替换或
            // 删除）→ 一律按取消收尾
            cmd = cmd_rx.recv() => match cmd {
                Some(TaskCmd::Reconfigure { .. }) => {}
                _ => return Flow::Cancelled,
            },
        }
    };
    let head = probe_head(&resp);
    if !(200..=299).contains(&head.status) {
        // v1.27：同探测网络错误——状态错误未核验内容，台账原样回报
        return Flow::Failed(
            EngineFailure::http(head.status, head.retry_after),
            false,
            spec.sidecar.clone().zip(spec.sidecar_path.clone()),
        );
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
    let name = if spec.sidecar.is_some() || spec.keep_name {
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
    // sc_path 提前推导（D3 守卫与后续单流/分块路径共用单源）
    let sc_path = sidecar_path(&spec.save_dir, &name);
    let sc = spec.sidecar.clone().filter(|_| {
        name == spec.name || spec.sidecar.as_ref().is_some_and(|s| s.task.id == spec.id)
    });
    let blocks = if let Some(sidecar) = &sc {
        // v1.26（操作者实证「.ezr 中途被删 + 进度清零」）：探测响应缺失
        // Content-Length（超时型/降级服务器的典型形态）≠「大小变化」确证
        // ——probed.size=None 与台账比对恒 false 会被误判 Invalidated 删账
        // 重下。台账已知大小（size>0）→ 按探测不可靠处理：暂态失败保留
        // 断点；未知大小台账（size=0）维持作废重下（不可核验，v115
        // unknown_size_resume_restarts 锁定口径）。
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
        // v1.16/FR-01-104（D31）：块计划入口生效值下限 1 MB——配置/注册表遗留
        // 小块值（含极端 1B × 超大文件的 u32 块数越域面）一律钳到下限；
        // 续传分支用 sidecar 块大小，不在此限（一致性优先）
        Blocks::new(head.total, spec.block_size.max(MIN_HTTP_BLOCK_SIZE))
    };

    let total_blocks = blocks.count();
    let multi_ok = head.status == 206 && head.total > 0;
    // 探测完成（App 更新 final_url/total/resumable/probed 与大小显示）
    if !emit(
        evt_tx,
        Evt::Probed {
            id: spec.id,
            name: name.clone(),
            final_url: resp.url().as_str().to_string(),
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
    // 保存目录自动创建（Gherkin 01-add-task-11「目录不存在自动创建」：
    // 引擎打开目标文件前确保父目录存在，覆盖默认下载目录与用户新输入目录）
    let _ = tokio::fs::create_dir_all(&dir).await;
    let dl_path = dir.join(format!("{name}{DOWNLOADING_EXT}"));
    let final_path = dir.join(&name);

    // 单流路径：不支持 Range（200）或单块（total_blocks <= 1）
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

    // ---- 分块下载（FR-01-11/13）----
    // 探测响应体（bytes=0- = 整文件流）在多块路径不再需要：取定最终 URL 后
    // 立即释放探测连接——否则未读响应体钉死该连接（服务端阻塞在写体/连接
    // 闲置），与 worker 的新建连接争用。单流路径（上方）仍移交 resp 续读。
    let worker_url = resp.url().as_str().to_string();
    drop(resp);
    // 预分配稀疏文件（续传时文件可能已存在：open 不截断，set_len 保持内容）
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
                    // v1.27：致命错不抹断点视图——台账原样回报
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
    // 任务级代理 + 并发热调通道（v1.5/FR-01-86/87，D19；下调臂 v1.16/D30 改判
    // 即时收缩——quota 臂见 block_worker 内层 select）：
    // quota = 目标 worker 数（下调时超额 worker 立即中止在传块后退出）；
    // endpoint = 当前任务代理端点（worker 每次领块解析 client——切换后
    // 新连接即新代理，在途请求按旧 client 完成，不中断传输）
    let (quota_tx, quota_rx) = watch::channel(spec.concurrency.clamp(1, 64));
    let (ep_tx, ep_rx) = watch::channel(Arc::new(spec.proxy_endpoint.clone()));
    let blocks_shared = Arc::new(tokio::sync::Mutex::new(blocks.clone()));
    // v1.16/FR-01-105：任务级实时读数基线 = 运行起点磁盘侧记账（含 sidecar
    // 恢复字节）；Progress 事件 downloaded = 基线 + 本轮 total_written，
    // 块内随字节落盘即时推进，不再等待块边界（消除任务/全局速度块内平台期）
    let base_downloaded = blocks.downloaded();
    let lease_next = Arc::new(AtomicU32::new(0));
    // v1.16/D30：配额中止块再领队列（与 lease_next 同源的领块面扩展）
    let requeue = Arc::new(tokio::sync::Mutex::<Vec<u32>>::new(Vec::new()));
    let total_written = Arc::new(AtomicU64::new(0));
    let failure = Arc::new(tokio::sync::Mutex::<Option<EngineFailure>>::new(None));
    let conns = Arc::new(tokio::sync::Mutex::<Vec<ConnView>>::new(Vec::new()));

    // worker 池：按设置并发数生成 worker（FR-01-11）；块数不足时多余 worker
    // 立即转「待命」（明细表可见），活跃传输数仍 = min(并发数, 未完成块数)
    // spawn 依赖单源（架构轮：12 参 spawn 表收敛为 WorkerDeps 参数对象——
    // 初始池与 Reconfigure 扩容两个调用点共用同一份克隆清单，防两处漂移）
    let deps = WorkerDeps {
        url: worker_url.clone(),
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

    // 监督循环：进度事件 + sidecar 周期落盘 + 完成/暂停/取消判定
    let mut flush_deadline = tokio::time::Instant::now() + SIDECAR_FLUSH;
    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => match cmd {
                Some(TaskCmd::Reconfigure { concurrency, endpoint }) => {
                    // v1.5/FR-01-87（D19 立即生效；下调即时收缩 v1.16/D30）：并发与代理热调
                    let q = concurrency.clamp(1, 64);
                    let _ = ep_tx.send(Arc::new(endpoint));
                    let _ = quota_tx.send(q);
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
            _ = tokio::time::sleep(PROGRESS_TICK) => {
                prune_finished(&mut handles);
                // v1.16/D30 兜底对账：缩容退出延迟/收割时序导致的瞬时缺口
                // （handles.len() < quota）在下一进度帧补足
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
                let conns_now = conns.lock().await.clone();
                drop(b);
                // v1.16/FR-01-105：任务级实时口径（基线 + 本轮累计写入）；
                // 块边界记账仅留收尾/落盘（完成时在传清零，两口径在完成点恒等，
                // FR-01-52 大小校验语义不变）
                let downloaded = base_downloaded + total_written.load(Ordering::Relaxed);
                if !emit(evt_tx, Evt::Progress {
                    id: spec.id,
                    downloaded,
                    conns: conns_now,
                    chunk_done,
                }).await {
                    return Flow::Cancelled;
                }
                if chunk_done == total_blocks && total_blocks > 0 {
                    // 全块完成 → 大小校验（FR-01-52）
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
                        // 无校验值直接完成：此处收尾——去 `.downloading` 扩展名 +
                        // 删 sidecar（FR-01-24/20）；有校验值时由 verify() 收尾
                        let _ = tokio::fs::rename(&dl_path, &final_path).await;
                        let _ = Sidecar::remove(&sc_path);
                    } else {
                        // v1.18 修订 A（FR-01-51 v1.18）：有校验值时完成前即时落盘
                        // 全块完成口径的终态 sidecar——原仅 SIDECAR_FLUSH 周期落盘，
                        // 完成点与最后落盘间的尾部字节不在台账（≤2s 内完成的快速
                        // 下载甚至从未落过盘）；「校验中」退出应用重启按过期台账
                        // 恢复会回退重传尾巴。verify() 通过后仍删此 sidecar（收尾
                        // 单源不破坏）；校验失败路径保留台账——全块完成口径下
                        // 引擎恢复零重传（FR-01-22/51）
                        flush_sidecar(spec, &probed_stamp, &blocks_shared, &sc_path).await;
                    }
                    let _ = emit(evt_tx, Evt::DownloadDone {
                        id: spec.id,
                        total,
                    }).await;
                    return Flow::Completed;
                }
                // sidecar 周期落盘
                if tokio::time::Instant::now() >= flush_deadline {
                    flush_deadline = tokio::time::Instant::now() + SIDECAR_FLUSH;
                    flush_sidecar(spec, &probed_stamp, &blocks_shared, &sc_path).await;
                }
            }
        }
    }
}

/// blocks 快照构建并落盘 sidecar（周期落盘与完成前终态落盘共用同一收口：
/// 加锁 → 全块口径构建 → 解锁后写盘，与两侧原内联序列逐句等价）
async fn flush_sidecar(
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
fn failed_flow(
    spec: &TaskSpec,
    stamp: &ServerStamp,
    blocks: &Blocks,
    f: EngineFailure,
    total_written: &AtomicU64,
    sc_path: &str,
) -> Flow {
    let sidecar = build_sidecar(spec, stamp, blocks, false);
    Flow::Failed(
        f,
        total_written.load(Ordering::Relaxed) > 0,
        Some((sidecar, sc_path.to_string())),
    )
}

/// 单流失败臂的断点台账（206 可续传流型折叠块内进度；无 Range 流型按
/// FR-01-12 不可续传不落账——与暂停臂口径一致）
fn failed_ledger(
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
fn block_span(b: &Blocks, idx: u32) -> (u64, u64, u64) {
    let (s, e) = block_range(b.total, b.piece, idx).unwrap_or((0, 0));
    let written = b.written.get(idx as usize).copied().unwrap_or(0);
    (s, e, written)
}

/// 构建 sidecar（下载中/暂停/失败共用）
#[allow(clippy::needless_pass_by_value)]
fn build_sidecar(
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

/// 单流下载路径（不支持 Range 或单块计划；FR-01-12 降级）。
/// v1.26 断点保全：单流 ≠ 不可续传——服务器支持 Range（206）时单块计划
/// 同样按块内偏移 Range 续传（FR-01-13）；仅当服务器本次未响应 Range 而
/// 台账已有进度时按暂态失败保留断点（不再静默截断重下——操作者实证：
/// 几次连接超时后分块/字节/进度清零）。
#[allow(clippy::too_many_arguments)]
async fn single_stream(
    spec: &TaskSpec,
    shared: &Arc<super::EngineShared>,
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
            if spec.expected_algo.is_none() {
                let _ = tokio::fs::rename(dl_path, final_path).await;
                let _ = Sidecar::remove(sc_path);
            } else {
                // 校验等待窗：落全块完成终态台账（与多块完成臂同款），
                // 由 App 校验流程收尾
                let _ = Sidecar::save(&build_sidecar(spec, stamp, &blocks, false), sc_path);
            }
            let _ = emit(
                evt_tx,
                Evt::DownloadDone {
                    id: spec.id,
                    total: blocks.total,
                },
            )
            .await;
            return Flow::Completed;
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
        // 续传需带块内偏移的新请求：探测连接（bytes=0-）丢弃；最终 URL
        // 与分块 worker 同源（探测响应实长 URL，免重定向再跟一跳）
        let worker_url = resp.url().as_str().to_string();
        drop(resp);
        let client = shared.client_for(spec.proxy_endpoint.as_ref());
        let range = format!("bytes={}-{}", resumed, blocks.total - 1);
        match req_with_identity(&client, &worker_url, Some(range))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let status = r.status().as_u16();
                let ra = parse_retry_after_header(
                    r.headers().get("retry-after").and_then(|v| v.to_str().ok()),
                );
                let _ = r.bytes().await;
                // v1.27：续传请求臂失败不得弃账（旧实现 None → 失败事件 0/0，
                // 台账块内进度在 UI 双失真）；按失败台账口径回报块内进度
                return Flow::Failed(
                    EngineFailure::http(status, ra),
                    false,
                    failed_ledger(spec, stamp, &blocks, multi_ok, resumed, sc_path),
                );
            }
            Err(e) => {
                return Flow::Failed(
                    classify_reqwest(&e),
                    false,
                    failed_ledger(spec, stamp, &blocks, multi_ok, resumed, sc_path),
                )
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
                    if spec.expected_algo.is_none() {
                        // 无校验值直接完成：收尾（FR-01-24/20）
                        let _ = tokio::fs::rename(dl_path, final_path).await;
                        let _ = Sidecar::remove(sc_path);
                    } else {
                        // v1.18 修订 A 对齐（多块完成臂同款）：完成前落全块
                        // 完成终态台账——「校验中」退出重启按台账直判收尾
                        let _ = Sidecar::save(
                            &build_sidecar(spec, stamp, &ledger, false),
                            sc_path,
                        );
                    }
                    let _ = emit(evt_tx, Evt::DownloadDone {
                        id: spec.id,
                        total: final_total,
                    }).await;
                    return Flow::Completed;
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

/// 校验（App 转校验中后下发；流式哈希不整载内存，FR-01-51）
pub(crate) async fn verify(spec: VerifySpec, evt_tx: mpsc::Sender<Evt>) {
    let path = spec.path.clone();
    // 规范名回查单源 checksum::algo_index_exact_or_default（architect v116：
    // 原内联 exact-match + unwrap_or(3) 与修改对话框预填重复）
    let algo_idx = checksum::algo_index_exact_or_default(spec.algo);
    let computed = tokio::task::spawn_blocking(move || checksum::digest_file(&path, algo_idx))
        .await
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
                .send(Evt::VerifyDone {
                    id: spec.id,
                    ok: false,
                    computed: e,
                    expected: spec.expected,
                })
                .await;
        }
    }
}

/// worker 共享依赖包（spawn 参数对象）：分块下载路径的全部跨 worker 共享
/// 句柄与任务上下文。依赖清单唯一登记于本结构体定义；`spawn` 是唯一 spawn
/// 调用点——初始 worker 池与 Reconfigure 扩容共用（架构轮：原两处 12 参
/// 调用表收敛，防漂移单源）。字段全部 Clone，逐 worker 克隆后转移所有权。
#[derive(Clone)]
struct WorkerDeps {
    /// 下载 URL（探测重定向后的最终地址）
    url: String,
    /// `.downloading` 目标文件路径
    file_path: PathBuf,
    /// 共享块视图（领块与写盘记账）
    blocks: Arc<tokio::sync::Mutex<Blocks>>,
    /// 全局块租约序号（原子领块指针）
    lease_next: Arc<AtomicU32>,
    /// 配额中止块再领队列（v1.16/D30：单调租约游标不回退，被中止在传块
    /// 经此队列交还后续 worker 从块内偏移接续；pop 单领由互斥保证）
    requeue: Arc<tokio::sync::Mutex<Vec<u32>>>,
    /// 停止信号（暂停/取消/失败收尾）
    stop_rx: watch::Receiver<bool>,
    /// 目标 worker 数（下调时多余 worker 完成当前块后自行退出）
    quota_rx: watch::Receiver<usize>,
    /// 任务代理端点（每次领块解析 client——切换后新连接即新代理）
    ep_rx: watch::Receiver<Arc<Option<ProxyEndpoint>>>,
    /// 全任务累计写字节（进度上报）
    total_written: Arc<AtomicU64>,
    /// 首个致命失败（任一 worker 登记后监督循环收尾）
    failure: Arc<tokio::sync::Mutex<Option<EngineFailure>>>,
    /// 活跃连接视图（进度事件携带）
    conns: Arc<tokio::sync::Mutex<Vec<ConnView>>>,
}

impl WorkerDeps {
    /// 克隆本依赖包产出第 `wid` 号 worker（依赖集单一来源：WorkerDeps 定义）
    fn spawn(&self, wid: usize, shared: Arc<super::EngineShared>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(block_worker(wid, shared, self.clone()))
    }
}

/// 块 worker：领块 → Range 下载 → 写盘；无块可领或停止信号时退出
async fn block_worker(wid: usize, shared: Arc<super::EngineShared>, deps: WorkerDeps) {
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
        // 并发下调（v1.5/FR-01-87；下调臂 v1.16/D30 改判即时收缩——内层
        // select 配额臂中止在传块，块交还再领队列）：超出目标配额的 worker
        // 立即退出；槽位 freed 后由 Reconfigure/进度 tick 在扩容时复用
        if wid > *quota_rx.borrow_and_update() {
            return;
        }
        // 领块：优先消费配额中止块的再领队列（v1.16/D30——单调游标不回退，
        // 被中止的在传块经队列交还，从块内已写偏移接续；pop 单领语义由
        // 互斥保证，无双重租约）；队列空则从 lease_next 顺序扫描第一个未完成块
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
            // 队列已空：连接转待命（FR-01-11）——保留空连接条目（cap=0）供
            // 明细表「待命」展示；块队列单调递减，worker 不会复活，条目存活至任务收尾
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
        // 任务级代理（v1.5/FR-01-86，D19）：每次领块按当前端点解析 client——
        // 代理切换后新连接即新代理；在途请求持旧 client 自然完成
        let ep = ep_rx.borrow_and_update().clone();
        let client = shared.client_for((*ep).as_ref());
        let resp = req_with_identity(&client, &url, Some(range)).send().await;
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
        // v1.16/FR-01-87 修订臂（D30）：配额下调中止（区别于停止信号，
        // 退出前需清除自身连接行）
        let mut quota_aborted = false;
        loop {
            tokio::select! {
                changed = stop_rx.changed() => {
                    if changed.is_err() || *stop_rx.borrow() {
                        aborted = true;
                    }
                }
                _ = quota_rx.changed() => {
                    // v1.16/FR-01-87 修订臂（D30 下调即时收缩）：配额与停止
                    // 信号同层监听——超额 worker 的在传块立即中止（不再等待
                    // 块边界，块内已写字节经 mark_progress 保留）；上调/
                    // 他人配额变化不打断本 worker
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
        if aborted {
            if quota_aborted {
                // v1.16/D30：被中止块交还再领队列（后续 worker 从块内偏移接续）
                // + 连接行即刻消失（面板反映目标并发）
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

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    use super::*;

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
        // 连接序号计数（droponce 门控：仅首个连接半途断连，后续正常——
        // 同一 URL 内构造「首轮断连、重试恢复」的用户故障形态）
        let conn_seq = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        std::thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                let seq = conn_seq.fetch_add(1, Ordering::Relaxed);
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
                // v1.27 回归门控：dropseq = 第 N 个连接不回任何响应直接断开
                // （构造「探测成功、续传 Range 请求即断」的请求臂失败形态）
                if opts
                    .iter()
                    .find(|(k, _)| k == "dropseq")
                    .and_then(|(_, v)| v.parse::<u32>().ok())
                    .is_some_and(|n| seq == n)
                {
                    continue;
                }
                let norange = opts.iter().any(|(k, _)| k == "norange");
                // v1.26 回归门控：noclen = 响应缺 Content-Length（超时型/降级
                // 服务器的典型形态）；drophalf = 传输半途断连（连接超时故障形态）
                let noclen = opts.iter().any(|(k, _)| k == "noclen");
                let drophalf = opts.iter().any(|(k, _)| k == "drophalf");
                let droponce = opts.iter().any(|(k, _)| k == "droponce");
                let (start, end, status) = match range_header
                    .as_deref()
                    .and_then(|v| v.strip_prefix("bytes="))
                {
                    Some(rv) if !norange => {
                        let (a, b) = rv.split_once('-').unwrap();
                        let s = a.parse::<u64>().unwrap_or(0);
                        let e = if b.is_empty() {
                            total - 1
                        } else {
                            b.parse::<u64>().unwrap_or(total - 1).min(total - 1)
                        };
                        (s, e, 206)
                    }
                    _ => (0, total - 1, 200),
                };
                let len = end - start + 1;
                let cl = if noclen {
                    String::new()
                } else {
                    format!("Content-Length: {len}\r\n")
                };
                let _ = write!(
                    w,
                    "HTTP/1.1 {status} OK\r\nAccept-Ranges: bytes\r\nETag: \"mock-{total}\"\r\nLast-Modified: Mon, 09 Feb 2026 08:00:00 GMT\r\n{cl}Connection: close\r\n\r\n"
                );
                let slow = opts.iter().any(|(k, _)| k == "slow");
                // 挂起门控（v1.17 计划外对齐，同轮扩面）：头部照发、正文永不应答
                // （阻塞读等客户端断开），对该连接的**一切请求**生效——探测
                // （开区间 `bytes=0-`）只需头部（Probed 事件照常），分块 worker
                // （有界区间）与单流（复用探测响应直接流 body、无独立 Range
                // 请求）全部阻塞在首字节读上，取消竞态测试中取消必然先于任何
                // 数据/进度/完成事件被处理。
                // 背景两段：①微文件瞬时完成与取消处理存在调度赛跑，workspace
                // 并行负载下偶发 DownloadDone 先于 Evt::Cancelled（原测试无门控
                // 靠时序幸运，硬定时非确定性）；②首轮门控只拦有界区间，漏了
                // v1.16/FR-01-104 1MB 下限把微文件块计划收敛成单块后的单流
                // 路径（total_blocks <= 1 走 single_stream 流探测响应 body）——
                // 门控对它失明，赛跑复现（fail-fast 全量实测 1/4 挂载点引爆
                // 「取消后不应完成下载」）。
                let hold = opts.iter().any(|(k, _)| k == "hold");
                if hold {
                    let mut sink = [0u8; 1];
                    let _ = std::io::Read::read(&mut r, &mut sink);
                    continue;
                }
                let mut buf = vec![0u8; 8192];
                let mut pos = start;
                while pos <= end {
                    let n = (end - pos + 1).min(8192) as usize;
                    fill(&mut buf[..n], pos);
                    if w.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    pos += n as u64;
                    if slow {
                        // 慢速门控：8 KB/15ms ≈ 0.53 MB/s，1 MB 块约 1.9 s——
                        // 构造跨多个进度帧的块内观察窗（FR-01-105/FR-01-87 测试用）
                        std::thread::sleep(std::time::Duration::from_millis(15));
                    }
                    if (drophalf || (droponce && seq == 0)) && pos - start >= len / 2 {
                        // 半途断连：读写句柄随循环作用域关闭（abrupt close，
                        // 客户端读到截断体 → 传输中断错误）；droponce 仅首个连接
                        break;
                    }
                }
            }
        });
        format!("http://{addr}/f.bin")
    }

    fn spec_for(
        id: u32,
        url: &str,
        dir: &str,
        block: u64,
        conc: usize,
        expected: Option<(&'static str, String)>,
    ) -> TaskSpec {
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
            keep_name: false,
            added_at: 0,
            proxy_endpoint: None,
        }
    }

    fn throttle_free() -> Arc<super::super::EngineShared> {
        Arc::new(super::super::EngineShared {
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            clients: std::sync::Mutex::new(std::collections::HashMap::new()),
            throttle: Arc::new(super::super::throttle::Throttle::new(0)),
        })
    }

    /// 构建 sidecar 接续 spec（断点测试夹具共用）
    fn sidecar_spec(id: u32, url: &str, dir: &std::path::Path, conc: usize) -> TaskSpec {
        let mut spec = spec_for(id, url, dir.to_str().unwrap(), 4096, conc, None);
        spec.sidecar = Some(Sidecar::load(dir.join("f.bin.ezr").to_str().unwrap()).unwrap());
        spec.sidecar_path = Some(dir.join("f.bin.ezr").to_str().unwrap().to_string());
        spec
    }

    /// 启动一次 run()：返回 (命令句柄 keep-alive，事件接收端，10s 超时截止)。
    /// cmd_tx 必须由调用方持有到断言结束：所有 sender drop 会让 supervisor 的
    /// cmd_rx.recv() 立即得到 None → Flow::Cancelled，下载永远完不成。
    #[allow(clippy::type_complexity)]
    fn launch_spec(
        spec: TaskSpec,
    ) -> (
        mpsc::Sender<TaskCmd>,
        mpsc::Receiver<Evt>,
        std::time::Instant,
    ) {
        let (tx, rx) = mpsc::channel::<Evt>(64);
        let (cmd_tx, cmd_rx) = mpsc::channel::<TaskCmd>(4);
        tokio::spawn(run(spec, throttle_free(), cmd_rx, tx));
        (
            cmd_tx,
            rx,
            std::time::Instant::now() + std::time::Duration::from_secs(10),
        )
    }

    /// 泵事件至 DownloadDone（返回 true）或 deadline（返回 false）；
    /// Failed 即 panic（两个 keep_name 重启用例共用的事件泵）。
    async fn wait_download_done(
        rx: &mut mpsc::Receiver<Evt>,
        deadline: std::time::Instant,
    ) -> bool {
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::DownloadDone { .. })) => return true,
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                _ => {}
            }
        }
        false
    }

    /// v1.5/FR-01-87（D19）：Reconfigure 并发热调——1→4 上调后新 worker 加入
    /// （活跃连接数峰值 ≥2），任务完成且无失败（代理热调同通道，None=直连）。
    /// v1.16/FR-01-104 对齐：块计划入口下限 1 MB 后，测试规格改用 1 MB 块
    /// （2 块文件 + 慢速门控，保证多帧观测窗与上调后新 worker 领块）
    #[tokio::test]
    async fn reconfigure_hot_adjusts_concurrency_and_completes() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-hot");
        let total: u64 = 2 * 1024 * 1024; // 2 块 × 1 MB（下限领域；慢速维持观测窗）
        let url = mock_server(total, vec![("slow".into(), String::new())]);
        let spec = spec_for(
            1,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            1,
            None,
        );
        let (cmd_tx, mut rx, deadline) = launch_spec(spec);
        let mut hot = false;
        let mut max_conns = 0usize;
        let mut done = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Probed { .. })) => {
                    // 探测完成即上调并发：1 → 4（直连）
                    cmd_tx
                        .send(TaskCmd::Reconfigure {
                            concurrency: 4,
                            endpoint: None,
                        })
                        .await
                        .unwrap();
                    hot = true;
                }
                Ok(Some(Evt::Progress { conns, .. })) => {
                    max_conns = max_conns.max(conns.len());
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(hot, "Reconfigure 应在探测完成后发出");
        assert!(done, "热调后任务应完成");
        assert!(max_conns >= 2, "上调后活跃连接数应上升（峰值 {max_conns}）");
        let f = dir.join("f.bin");
        let meta = std::fs::metadata(&f).expect("目标文件在位");
        assert_eq!(meta.len(), total);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// v1.16/FR-01-105：任务级 Progress 读数实时化——块内（chunk_done 未增长）
    /// downloaded 随字节落盘即时推进且严格单调（旧实现 = blocks.downloaded()
    /// 块边界粒度，块内恒为基线值 → 本测试红）
    #[tokio::test]
    async fn progress_task_bytes_live_within_block() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-live");
        let total: u64 = 1024 * 1024; // 1 块（1 MB），慢速门控跨多个进度帧
        let url = mock_server(total, vec![("slow".into(), String::new())]);
        let spec = spec_for(
            1,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            1,
            None,
        );
        let (_cmd_tx, mut rx, deadline) = launch_spec(spec);
        let mut in_block_samples: Vec<u64> = Vec::new();
        let mut done = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Progress {
                    downloaded,
                    chunk_done,
                    ..
                })) => {
                    if chunk_done == 0 && downloaded > 0 {
                        in_block_samples.push(downloaded);
                    }
                }
                Ok(Some(Evt::DownloadDone { total: t, .. })) => {
                    assert_eq!(t, total);
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(done, "任务应完成");
        assert!(
            in_block_samples.len() >= 3,
            "块内应出现多个非零实时读数（got {}）",
            in_block_samples.len()
        );
        assert!(
            in_block_samples.windows(2).all(|w| w[0] < w[1]),
            "块内读数应严格递增: {in_block_samples:?}"
        );
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        assert_eq!(data.len() as u64, total);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// v1.16/FR-01-104（D31）：块计划入口生效值下限 1 MB——spec.block_size = 1
    /// 的新一次下载按 1 MB 块计划（连接区间 = 1 MB；旧实现 1 字节块 → 本测试红）；
    /// 任务完成且文件正确（钳制不破坏下载链路）
    #[tokio::test]
    async fn block_plan_clamps_to_one_mb_floor() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-floor");
        let total: u64 = 2 * 1024 * 1024; // 钳制后 = 2 块 × 1 MB
        let url = mock_server(total, vec![("slow".into(), String::new())]);
        let spec = spec_for(2, &url, dir.to_str().unwrap(), 1, 1, None); // 遗留 1 字节块值
        let (_cmd_tx, mut rx, deadline) = launch_spec(spec);
        let mut first_range: Option<(u64, u64)> = None;
        let mut done = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Progress { conns, .. })) => {
                    if first_range.is_none() {
                        if let Some(c) = conns.first() {
                            first_range = Some((c.start, c.end));
                        }
                    }
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(done, "钳制后任务应完成");
        let (s, e) = first_range.expect("应观测到连接块区间");
        assert_eq!(
            e - s,
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            "块计划应按 1 MB 下限钳制（got {s}..{e}）"
        );
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect, "钳制路径文件内容应逐字节一致");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// v1.16/FR-01-87 修订臂（D30）：并发下调即时收缩——2→1 确定后约 1 秒内
    /// 多余连接的在传块被中止（不再等待块边界，旧实现块级过渡延迟 → 本测试红）、
    /// 连接行消失；被中断块块内进度保留（最终文件逐字节正确、无回退）
    #[tokio::test]
    async fn reconfigure_scale_down_interrupts_inflight_immediately() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-scaledown");
        let total: u64 = 2 * 1024 * 1024; // 2 块 × 1 MB（慢速 1.9 s/块）
        let url = mock_server(total, vec![("slow".into(), String::new())]);
        let spec = spec_for(
            3,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            2,
            None,
        );
        let (cmd_tx, mut rx, deadline) = launch_spec(spec);
        let mut downgraded_at: Option<std::time::Instant> = None;
        let mut max_conns_after = 0usize;
        let mut last_downloaded: u64 = 0;
        let mut done = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Probed { .. })) => {
                    // 探测完成即下调并发：2 → 1（两 worker 均已在途）
                    cmd_tx
                        .send(TaskCmd::Reconfigure {
                            concurrency: 1,
                            endpoint: None,
                        })
                        .await
                        .unwrap();
                    downgraded_at = Some(std::time::Instant::now());
                }
                Ok(Some(Evt::Progress {
                    downloaded, conns, ..
                })) => {
                    assert!(
                        downloaded >= last_downloaded,
                        "已下载字节单调不减: {last_downloaded} -> {downloaded}"
                    );
                    last_downloaded = downloaded;
                    if let Some(t0) = downgraded_at {
                        if t0.elapsed() >= std::time::Duration::from_millis(600) {
                            max_conns_after = max_conns_after.max(conns.len());
                        }
                    }
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(downgraded_at.is_some(), "下调命令应已发出");
        assert!(done, "下调后任务应完成");
        assert!(
            max_conns_after <= 1,
            "下调 600ms 后活跃连接数应 ≤ 1（got {max_conns_after}）——旧实现块级过渡延迟会超窗"
        );
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect, "被中断块的块内进度应保留（文件逐字节一致）");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn end_to_end_resumable_download_completes() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e");
        let total: u64 = 3 * 4096 + 1111; // 4 块（块 4KB），末块吸收余数
        let url = mock_server(total, vec![]);
        let (_cmd_tx, mut rx, deadline) =
            launch_spec(spec_for(1, &url, dir.to_str().unwrap(), 4096, 4, None));
        let mut done = false;
        let mut probed = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::DownloadDone { total: t, .. })) => {
                    assert_eq!(t, total);
                    done = true;
                    break;
                }
                Ok(Some(Evt::Probed {
                    total: t,
                    resumable: true,
                    ..
                })) => {
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
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e-cancel");
        let total: u64 = 3 * 4096 + 1111;
        // v1.17 计划外对齐：hold 门控（有界区间请求正文永不应答）——微文件瞬时
        // 完成与取消处理存在调度赛跑，workspace 并行负载下偶发 DownloadDone
        // 先于 Evt::Cancelled（非确定性失败两次复现于基线复跑）；门控后取消
        // 必然先于任何数据/完成事件被处理，测试转确定性
        let url = mock_server(total, vec![("hold".into(), String::new())]);
        let (tx, mut rx) = mpsc::channel::<Evt>(64);
        let shared = throttle_free();
        let (cmd_tx, cmd_rx) = mpsc::channel::<TaskCmd>(4);
        let spec = spec_for(9, &url, dir.to_str().unwrap(), 4096, 4, None);
        // 取消命令先于 run 入队：v1.25 探测 select 消费命令后，取消在探测
        // 在途即命中（即时停机）或在探测完成后的首个监督轮询命中——两者
        // 顺序均合法（loopback 头部到达与取消命令的就绪竞速为 50/50），
        // 本测试不锁定 Probed 是否先到，只锁定取消语义：Cancelled 必到、
        // 无幽灵进度、无完成、无产物
        cmd_tx.send(TaskCmd::Cancel).await.unwrap();
        tokio::spawn(run(spec, shared, cmd_rx, tx));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut probed = false;
        let mut cancelled = false;
        let mut ghost = false;
        let mut done = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Probed {
                    resumable: true, ..
                })) => probed = true,
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
        let _ = probed; // 探测事件与取消事件的先后不锁定（见上）
        assert!(cancelled, "未收到取消确认事件（Evt::Cancelled）");
        // Flow::Cancelled 路径不 finalize、不写 sidecar：最终文件与 .ezr 都不应出现
        assert!(!dir.join("f.bin").exists(), "取消不应产出最终文件");
        assert!(!dir.join("f.bin.ezr").exists(), "取消路径不应写 sidecar");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn norange_server_falls_back_single_stream() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e2");
        let total: u64 = 9000;
        let url = mock_server(total, vec![("norange".to_string(), "1".to_string())]);
        let (_cmd_tx, mut rx, deadline) =
            launch_spec(spec_for(2, &url, dir.to_str().unwrap(), 4096, 4, None));
        let mut done = false;
        let mut non_resumable = false;
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

    /// 失效重启（keep_name=true）必须沿用既有任务名：残留 `.downloading` 文件
    /// 不得触发去重改名（否则 sidecar 落盘名与任务名错位，FR-01-22 一致性
    /// 检查在下轮续传被跳过）；keep_name=false（首次启动）仍按规则去重。
    #[tokio::test]
    async fn keep_name_restart_skips_dedupe() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e4");
        let total: u64 = 9000;
        let url = mock_server(total, vec![]);
        // 残留的部分下载文件（失效重启现场）：f.bin.downloading + 旧 sidecar 名
        std::fs::write(dir.join("f.bin.downloading"), b"stale").unwrap();

        // keep_name=true：沿用 f.bin（写入 f.bin.downloading，不漂移新名）
        let mut spec = spec_for(4, &url, dir.to_str().unwrap(), 4096, 2, None);
        spec.keep_name = true;
        let (cmd_tx, mut rx, deadline) = launch_spec(spec);
        let done = wait_download_done(&mut rx, deadline).await;
        assert!(done, "keep_name 重启应完成");
        assert!(
            dir.join("f.bin").exists(),
            "keep_name 重启应写回原名目标文件"
        );
        assert!(
            !dir.join("f.bin.1").exists() && !dir.join("f.bin.1.downloading").exists(),
            "keep_name 重启不得去重改名"
        );
        drop(cmd_tx);
        std::fs::remove_dir_all(&dir).ok();

        // keep_name=false（首次启动）：残留文件存在时按规则去重
        let dir2 = crate::model::testenv::uniq_tmp_dir("ezr-e2e7");
        std::fs::write(dir2.join("f.bin.downloading"), b"stale").unwrap();
        let (_cmd_tx, mut rx, deadline) =
            launch_spec(spec_for(5, &url, dir2.to_str().unwrap(), 4096, 2, None));
        let done = wait_download_done(&mut rx, deadline).await;
        assert!(done, "首次启动应完成");
        assert!(
            dir2.join("f.bin.1").exists(),
            "首次启动遇残留文件应去重到 f.bin.1"
        );
        std::fs::remove_dir_all(&dir2).ok();
    }

    #[tokio::test]
    async fn status_503_fails_transient() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e3");
        let url = mock_server(1000, vec![("status".to_string(), "503".to_string())]);
        let (_cmd_tx, mut rx, deadline) =
            launch_spec(spec_for(3, &url, dir.to_str().unwrap(), 4096, 2, None));
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
        // 目录名唯一化（原与 keep_name_restart_skips_dedupe 同用 ezr-e2e4-{pid}，
        // 并行测试下先完成者的 remove_dir_all 会删除另一测试的工作目录）
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e8");
        let total: u64 = 2 * 4096;
        let url = mock_server(total, vec![]);
        // 先下载（无校验值）生成文件
        let (_cmd_tx, mut rx, deadline) =
            launch_spec(spec_for(4, &url, dir.to_str().unwrap(), 4096, 2, None));
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
            Some(Evt::VerifyDone {
                ok: true, computed, ..
            }) => assert_eq!(computed, digest),
            other => panic!("校验应通过: {other:?}"),
        }
        assert!(final_path.exists(), "通过后应 rename 到最终名");
        assert!(!dl.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn sidecar_resume_continues_from_breakpoint() {
        // 断点接续：预置 sidecar（前 1.5 块已写）→ 探测一致 → 续传 → 完成
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e5");
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
            SidecarTask {
                id: 5,
                added_at: 0,
                save_dir: dir.to_str().unwrap().to_string(),
                concurrency: 2,
                protocol: Protocol::Http,
            },
        );
        sc.save(dir.join("f.bin.ezr").to_str().unwrap()).unwrap();
        let (_cmd_tx, mut rx, deadline) = launch_spec(sidecar_spec(5, &url, &dir, 2));
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
        assert!(
            resumed_progress.unwrap_or(0) >= 6000,
            "应从断点续传: {resumed_progress:?}"
        );
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn full_sidecar_no_checksum_finalizes_without_transfer() {
        // FR-01-51 v1.17③/D34 引擎链路：sidecar 全块完成 + 无期望值 → Start 后
        // 不发起任何传输，直接收尾「已完成」（rename + 删 sidecar）。用盘上
        // 特征字节证明无重传：预写内容与 mock 供数模式（fill）不同，若引擎误
        // 重传，内容会被覆盖为 fill 模式、断言失败。
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e-full-sc");
        let total: u64 = 4 * 4096;
        let url = mock_server(total, vec![]);
        std::fs::write(dir.join("f.bin.downloading"), vec![0xAB; total as usize]).unwrap();
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
            &[4096, 4096, 4096, 4096],
            false,
            None,
            SidecarTask {
                id: 6,
                added_at: 0,
                save_dir: dir.to_str().unwrap().to_string(),
                concurrency: 2,
                protocol: Protocol::Http,
            },
        );
        sc.save(dir.join("f.bin.ezr").to_str().unwrap()).unwrap();
        let (_cmd_tx, mut rx, deadline) = launch_spec(sidecar_spec(6, &url, &dir, 2));
        loop {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
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
        // 收尾形态：最终文件存在且内容为预写特征字节（无重传证据）、.downloading
        // 与 sidecar 均已清除
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        assert_eq!(
            data,
            vec![0xAB; total as usize],
            "预写内容必须原样保留（误重传会被 fill 模式覆盖）"
        );
        assert!(!dir.join("f.bin.downloading").exists());
        assert!(!dir.join("f.bin.ezr").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn changed_etag_invalidates_sidecar() {
        // 一致性失效：sidecar ETag 与探测不符 → Invalidated
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e6");
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
            SidecarTask {
                id: 6,
                added_at: 0,
                save_dir: dir.to_str().unwrap().to_string(),
                concurrency: 1,
                protocol: Protocol::Http,
            },
        );
        sc.save(dir.join("f.bin.ezr").to_str().unwrap()).unwrap();
        let (_cmd_tx, mut rx, deadline) = launch_spec(sidecar_spec(6, &url, &dir, 1));
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Invalidated { .. })) => return,
                Ok(Some(Evt::Failed { reason, .. })) => panic!("不应失败: {reason}"),
                _ => {}
            }
        }
        panic!("应回报一致性失效");
    }

    /// v1.25 缺陷回归（引擎单写者不变式；操作者实证：重试 10/5 / 进度清零 /
    /// 已失败红条增长）：同任务 id 重复 Start 时，引擎必须先取消存活旧
    /// supervisor 再孵化新 supervisor。历史缺陷：旧句柄被静默覆盖、双
    /// supervisor 并发写同一 `.downloading` 文件与 sidecar——重复 Failed
    /// 计数冲破重试上限、进度回退、已失败态进度条被幽灵事件推高。
    /// 观察点：第二个 Start 后旧 supervisor 被取消（收到 Evt::Cancelled），
    /// 替换后的新 supervisor 独自跑完下载（恰好一个 DownloadDone）。
    #[tokio::test]
    async fn engine_start_replaces_live_supervisor() {
        let total: u64 = 3 * 1024 * 1024; // 3 块 × 1 MB；slow 门控拉长在传窗口
        let url = mock_server(total, vec![("slow".into(), String::new())]);
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-replace");
        let shared = throttle_free();
        let mut tasks: std::collections::HashMap<u32, mpsc::Sender<TaskCmd>> =
            std::collections::HashMap::new();
        let (evt_tx, mut evt_rx) = mpsc::channel::<Evt>(256);
        let mut spec = spec_for(
            7,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            1,
            None,
        );
        // 钉住任务名（比照真实重试路径 make_spec 的 keep_name 口径）
        spec.keep_name = true;
        crate::engine::start_task(&mut tasks, spec.clone(), &shared, &evt_tx);
        // 首个 supervisor 越过探测进入在传窗口（slow ≈ 1.9s/块）
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        crate::engine::start_task(&mut tasks, spec, &shared, &evt_tx);
        // 旧 supervisor 必须被取消（Cancelled 事件）；deadline 内未到即失败
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut cancelled = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), evt_rx.recv()).await {
                Ok(Some(Evt::Cancelled { id: 7 })) => {
                    cancelled = true;
                    break;
                }
                Ok(Some(_)) => {}
                _ => break,
            }
        }
        assert!(
            cancelled,
            "同任务 id 重复 Start：存活旧 supervisor 必须被替换取消"
        );
        // 替换后的新 supervisor 独自跑完下载：恰好一个 DownloadDone
        let done_deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut done_count = 0usize;
        while std::time::Instant::now() < done_deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), evt_rx.recv()).await {
                Ok(Some(Evt::DownloadDone { id: 7, .. })) => done_count += 1,
                Ok(Some(Evt::Failed { reason, .. })) => panic!("替换后不应失败: {reason}"),
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(_) => {} // 静默窗：继续等待
            }
        }
        assert_eq!(
            done_count, 1,
            "替换后恰好一个 supervisor 完成（无双开双完成）"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 探测期专用服务器：首个连接延迟应答 503（慢探测失败注入），后续连接
    /// 按 Range 正常 206 服务。线程 detach（engineering.md 口径）。
    fn slow_first_fail_server(total: u64, delay_ms: u64) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            let mut first = true;
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
                if first {
                    first = false;
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    let _ = write!(
                        w,
                        "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    continue;
                }
                let (start, end) = match range_header
                    .as_deref()
                    .and_then(|v| v.strip_prefix("bytes="))
                {
                    Some(rv) => {
                        let (a, b) = rv.split_once('-').unwrap();
                        let s = a.parse::<u64>().unwrap_or(0);
                        let e = if b.is_empty() {
                            total - 1
                        } else {
                            b.parse::<u64>().unwrap_or(total - 1).min(total - 1)
                        };
                        (s, e)
                    }
                    _ => (0, total - 1),
                };
                let len = end - start + 1;
                let _ = write!(
                    w,
                    "HTTP/1.1 206 Partial Content\r\nAccept-Ranges: bytes\r\nETag: \"mock-{total}\"\r\nLast-Modified: Mon, 09 Feb 2026 08:00:00 GMT\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
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

    /// v1.25 缺陷回归（探测期替换；操作者实证「重试 10/5」的引擎侧根因）：
    /// 历史 supervisor 探测阶段不消费任务命令——重复 Start 替换时，慢探测的
    /// 存活旧 supervisor 不受句柄替换影响，探测失败后补发**迟到 Failed**
    ///（App 侧重试计数被幽灵事件重复推进，冲破 5 次上限观测 10/5）；探测
    /// 成功则孵化幽灵 worker 与新 supervisor 并发写盘（进度回退/已失败态
    /// 红条增长）。修复后探测请求与命令同 select：替换即停机（Cancelled
    /// 事件），不再有迟到 Failed，新 supervisor 独自完成。
    #[tokio::test]
    async fn probe_phase_replace_never_yields_late_failed() {
        let total: u64 = 256 * 1024;
        let url = slow_first_fail_server(total, 2_000);
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-probe-replace");
        let shared = throttle_free();
        let mut tasks: std::collections::HashMap<u32, mpsc::Sender<TaskCmd>> =
            std::collections::HashMap::new();
        let (evt_tx, mut evt_rx) = mpsc::channel::<Evt>(256);
        let mut spec = spec_for(
            9,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            1,
            None,
        );
        spec.keep_name = true;
        // Start#1：旧 supervisor 进入 2s 慢探测（连接 1 排队 503 注入）
        crate::engine::start_task(&mut tasks, spec.clone(), &shared, &evt_tx);
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        // Start#2：替换——旧 supervisor 必须立即停机（探测 select 消费命令）
        crate::engine::start_task(&mut tasks, spec, &shared, &evt_tx);
        // 泵事件至新 supervisor 完成（连接 2 起正常服务）；迟到 Failed 即违例
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let (mut cancelled, mut done) = (false, false);
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), evt_rx.recv()).await {
                Ok(Some(Evt::Cancelled { id: 9 })) => cancelled = true,
                Ok(Some(Evt::DownloadDone { id: 9, .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { id: 9, reason, .. })) => {
                    panic!("替换后的存活旧 supervisor 不得补发迟到 Failed: {reason}")
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(_) => {} // 静默窗（慢探测注入期）：继续等待
            }
        }
        assert!(done, "替换后的新 supervisor 应完成下载");
        assert!(
            cancelled,
            "探测期替换必须即时取消旧 supervisor（Cancelled 事件）"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// sidecar 已完成块数：部分写与零块不计入（Failed 事件汇总口径）
    #[test]
    fn completed_of_counts_full_blocks_only() {
        let url = "http://127.0.0.1:1/x.bin".to_string();
        let stamp = ServerStamp {
            final_url: Some(url.clone()),
            etag: None,
            last_modified: None,
            size: Some(4 * 4096),
        };
        let sc = Sidecar::build(
            &url,
            &stamp,
            4 * 4096,
            4096,
            &[4096, 4096, 1904, 0],
            false,
            None,
            SidecarTask {
                id: 1,
                added_at: 0,
                save_dir: crate::model::testenv::uniq_tmp_dir("ezr-sup-sd")
                    .to_string_lossy()
                    .into_owned(),
                concurrency: 2,
                protocol: Protocol::Http,
            },
        );
        assert_eq!(completed_of(&sc), 2, "部分写块与零块不计入完成");
    }

    // -----------------------------------------------------------------------
    // v1.26（操作者 20261014 二次报告）：单流路径三缺陷 —— 「几次连接超时后
    // 分块/字节/进度全部清零重新下载 + .ezr 控制文件中途被删」。三缺陷：
    //   ① 单流失败臂不落 sidecar（206 可续传流型的块内进度全弃，重试从零）；
    //   ② 单流入口无视恢复进度一律截断重下（单块计划 + 206 本可 Range 续传）；
    //   ③ 带断点重试的探测缺 Content-Length 被误判 Invalidated → 删 .ezr。
    // -----------------------------------------------------------------------

    /// 预置 sidecar 落盘（stamp 与 mock 服务器响应头严格一致）
    fn seed_sidecar(dir: &std::path::Path, url: &str, total: u64, piece: u64, written: &[u64]) {
        let stamp = ServerStamp {
            final_url: Some(url.to_string()),
            etag: Some(format!("\"mock-{total}\"")),
            last_modified: Some("Mon, 09 Feb 2026 08:00:00 GMT".to_string()),
            size: Some(total),
        };
        let sc = Sidecar::build(
            url,
            &stamp,
            total,
            piece,
            written,
            false,
            None,
            SidecarTask {
                id: 1,
                added_at: 0,
                save_dir: dir.to_string_lossy().into_owned(),
                concurrency: 4,
                protocol: Protocol::Http,
            },
        );
        sc.save(dir.join("f.bin.ezr").to_str().unwrap()).unwrap();
    }

    /// ① 单流（单块计划 + 206 可续传）半途断连：失败必须落可续传台账
    /// （旧实现失败臂 sidecar=None，块内进度全弃 → 重试从零）
    #[tokio::test]
    async fn single_stream_failure_saves_resumable_sidecar() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-fail");
        let total: u64 = 512 * 1024; // ≤ 1MB 下限 → 单块计划 → 单流路径
        let url = mock_server(total, vec![("drophalf".into(), String::new())]);
        let spec = spec_for(
            1,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            4,
            None,
        );
        let (_cmd_tx, mut rx, deadline) = launch_spec(spec);
        let mut failed: Option<u64> = None;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Failed { downloaded, .. })) => {
                    failed = Some(downloaded);
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("断连应按失败而非作废收尾"),
                _ => {}
            }
        }
        let downloaded = failed.expect("半途断连应回报 Failed");
        assert!(
            downloaded > 0 && downloaded < total,
            "失败事件应携带真实进度（{downloaded}/{total}）"
        );
        let sc = Sidecar::load(dir.join("f.bin.ezr").to_str().unwrap())
            .expect("断点台账应落盘（旧实现失败臂 sidecar=None 全弃）");
        assert_eq!(sc.downloaded, downloaded, "台账字节 = 失败事件字节");
        assert!(!sc.non_resumable, "206 流型失败台账可续传");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ② 断点续传全程：断连失败 → 台账 → 恢复 Range 续传不回退直至完成
    /// （同一 URL 内构造「首轮断连、重试恢复」；旧实现恢复即截断重下）
    #[tokio::test]
    async fn single_stream_failure_then_retry_resumes_without_restart() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-resume");
        let total: u64 = 512 * 1024;
        // droponce：首个连接（探测+复用流）半途断连，后续连接正常
        let url = mock_server(total, vec![("droponce".into(), String::new())]);
        let spec1 = spec_for(
            1,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            4,
            None,
        );
        let (_tx, mut rx, deadline) = launch_spec(spec1);
        let mut resumed_at = 0u64;
        while std::time::Instant::now() < deadline {
            if let Ok(Some(Evt::Failed { downloaded, .. })) =
                tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await
            {
                resumed_at = downloaded;
                break;
            }
        }
        assert!(
            resumed_at > 0 && resumed_at < total,
            "第一轮应半途失败（{resumed_at}/{total}）"
        );
        // 第二轮：同一 URL 重试 + 断点接续
        let spec2 = sidecar_spec(1, &url, &dir, 4);
        let (_tx2, mut rx2, deadline2) = launch_spec(spec2);
        let mut done = false;
        let mut first_progress = true;
        while std::time::Instant::now() < deadline2 {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx2.recv()).await {
                Ok(Some(Evt::Progress { downloaded, .. })) => {
                    if first_progress {
                        assert!(
                            downloaded >= resumed_at,
                            "续传首个进度不得回退（{downloaded} < {resumed_at}；旧实现截断重下）"
                        );
                        first_progress = false;
                    }
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("一致台账不应作废"),
                Ok(Some(Evt::Failed { reason, .. })) => panic!("续传不应失败: {reason}"),
                _ => {}
            }
        }
        assert!(done, "断点续传应完成");
        let data = std::fs::read(dir.join("f.bin")).expect("完成文件在位");
        assert_eq!(data.len() as u64, total);
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect, "续传后文件字节完整（不重不漏）");
        assert!(!dir.join("f.bin.ezr").exists(), "完成后台账删除");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ②-补：服务器本次未支持 Range 且台账有进度 → 暂态失败保留断点
    /// （不得静默截断重下；.downloading 与 .ezr 均保持原样）
    #[tokio::test]
    async fn resume_entry_without_range_preserves_progress() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-norange");
        let total: u64 = 2 * 1024 * 1024; // 2 块 × 1MB
        let url = mock_server(total, vec![("norange".into(), String::new())]);
        seed_sidecar(&dir, &url, total, 1024 * 1024, &[1024 * 1024, 200_000]);
        std::fs::write(dir.join("f.bin.downloading"), vec![0u8; total as usize]).unwrap();
        let spec = sidecar_spec(1, &url, &dir, 4);
        let (_tx, mut rx, deadline) = launch_spec(spec);
        let mut reason: Option<String> = None;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Failed { reason: r, .. })) => {
                    reason = Some(r);
                    break;
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    panic!("有断点时 200 探测不得静默截断重下（旧实现行为）");
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("服务端戳一致不得作废"),
                _ => {}
            }
        }
        let reason = reason.expect("应暂态失败保留断点");
        assert!(
            reason.contains("Range"),
            "失败原因应说明 Range 未支持: {reason}"
        );
        assert!(dir.join("f.bin.ezr").exists(), "台账保留");
        assert_eq!(
            std::fs::metadata(dir.join("f.bin.downloading"))
                .unwrap()
                .len(),
            total,
            "半成品文件不得被截断"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ③ 带断点重试的探测缺 Content-Length → 暂态失败保留断点
    /// （不得判 Invalidated 删 .ezr——操作者实证「.ezr 中途被删」根因）
    #[tokio::test]
    async fn resume_probe_without_content_length_keeps_sidecar() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-noclen");
        let total: u64 = 1024 * 1024;
        let url = mock_server(total, vec![("noclen".into(), String::new())]);
        seed_sidecar(&dir, &url, total, 1024 * 1024, &[400_000]);
        let spec = sidecar_spec(1, &url, &dir, 4);
        let (_tx, mut rx, deadline) = launch_spec(spec);
        let mut failed = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Failed { .. })) => {
                    failed = true;
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => {
                    panic!("缺 Content-Length 的降级探测不得作废断点（旧实现删 .ezr）");
                }
                _ => {}
            }
        }
        assert!(failed, "缺 Content-Length 探测应按暂态失败保留断点");
        assert!(dir.join("f.bin.ezr").exists(), "台账保留");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ④ v1.27 回归（操作者实证：详情「分块 0/11」而磁盘 .ezr 台账 2 块全 +
    /// 1 部分块）：带断点台账的任务在**探测阶段**网络错误（连接超时/拒连——
    /// 重试的每次尝试都先探测）时，失败事件不得把台账弃成 None 上报 0/0——
    /// 多块路径事件口径权威，App apply_chunk_snapshot 会用 0 覆盖分块视图
    /// （字节因 >0 守卫保留，详情页「已下载 2.2 MB / 分块 0/3」自相矛盾，
    /// 注册表随之持久化错误快照）。探测失败 = 未核验 ≠ 断点消失：上报台账
    /// 的持久化状态（不确变、不删账）。
    #[tokio::test]
    async fn probe_conn_error_reports_persisted_breakpoint() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-probe-conn");
        let total: u64 = 3 * 1024 * 1024; // 3 块 × 1MB
                                          // 拒连端口：bind 后立即 drop（探测请求必网络错误，无响应可用）
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        drop(l);
        let url = format!("http://{addr}/f.bin");
        seed_sidecar(
            &dir,
            &url,
            total,
            1024 * 1024,
            &[1_048_576, 1_048_576, 200_000],
        );
        let spec = sidecar_spec(1, &url, &dir, 4);
        let (_tx, mut rx, deadline) = launch_spec(spec);
        let mut got: Option<(u64, u32)> = None;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Failed {
                    downloaded,
                    chunk_done,
                    ..
                })) => {
                    got = Some((downloaded, chunk_done));
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("无响应探测不得作废断点"),
                _ => {}
            }
        }
        assert_eq!(
            got,
            Some((1_048_576 + 1_048_576 + 200_000, 2)),
            "失败事件应携带台账持久化状态（旧实现上报 0/0 → 详情分块清零）"
        );
        assert!(dir.join("f.bin.ezr").exists(), "台账保留");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ⑤ 同 ④：探测期 HTTP 状态错误（503）同口径——失败事件携带台账状态。
    #[tokio::test]
    async fn probe_http_error_reports_persisted_breakpoint() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-probe-http");
        let total: u64 = 3 * 1024 * 1024;
        let url = mock_server(total, vec![("status".into(), "503".into())]);
        seed_sidecar(
            &dir,
            &url,
            total,
            1024 * 1024,
            &[1_048_576, 1_048_576, 200_000],
        );
        let spec = sidecar_spec(1, &url, &dir, 4);
        let (_tx, mut rx, deadline) = launch_spec(spec);
        let mut got: Option<(u64, u32)> = None;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Failed {
                    downloaded,
                    chunk_done,
                    ..
                })) => {
                    got = Some((downloaded, chunk_done));
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("状态错误探测不得作废断点"),
                _ => {}
            }
        }
        assert_eq!(
            got,
            Some((1_048_576 + 1_048_576 + 200_000, 2)),
            "失败事件应携带台账持久化状态（旧实现上报 0/0）"
        );
        assert!(dir.join("f.bin.ezr").exists(), "台账保留");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ⑥ 单流续传请求臂（Range 续传请求本身网络错误）同口径：单块台账
    /// 块内部分进度（无完整块 → chunk_done=0 属实），失败事件必须携带台账
    /// 字节（旧实现 None → 「已下载」清零观感，字节与分块双失真）。
    #[tokio::test]
    async fn single_stream_resume_request_error_keeps_ledger() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-resume-err");
        let total: u64 = 512 * 1024; // 单块计划 → 单流路径
                                     // dropseq=1：连接 0 = 探测（206 全量头），连接 1 = Range 续传请求 → 直接断开
        let url = mock_server(total, vec![("dropseq".into(), "1".into())]);
        seed_sidecar(&dir, &url, total, 1024 * 1024, &[200_000]);
        let spec = sidecar_spec(1, &url, &dir, 4);
        let (_tx, mut rx, deadline) = launch_spec(spec);
        let mut got: Option<u64> = None;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Failed { downloaded, .. })) => {
                    got = Some(downloaded);
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("一致台账不得作废"),
                _ => {}
            }
        }
        assert_eq!(
            got,
            Some(200_000),
            "续传请求失败应携带台账字节（旧实现上报 0 → 已下载清零观感）"
        );
        assert!(dir.join("f.bin.ezr").exists(), "台账保留");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ②-补2：全块完成台账（校验等待窗中断）恢复 → 直判收尾不重传
    /// （旧实现截断后阻塞在 hold 体上；台账与磁盘脱节才作废重下）
    #[tokio::test]
    async fn full_ledger_resume_finalizes_without_retransfer() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-full");
        let total: u64 = 512 * 1024;
        // hold 门控：有头无体——全块台账直判收尾不需要响应体
        let url = mock_server(total, vec![("hold".into(), String::new())]);
        seed_sidecar(&dir, &url, total, 1024 * 1024, &[total]);
        std::fs::write(dir.join("f.bin.downloading"), vec![7u8; total as usize]).unwrap();
        let spec = sidecar_spec(1, &url, &dir, 4);
        let (_tx, mut rx, deadline) = launch_spec(spec);
        let mut done = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::DownloadDone { total: t, .. })) => {
                    assert_eq!(t, total);
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("全块台账应直判收尾: {reason}"),
                _ => {}
            }
        }
        assert!(
            done,
            "全块完成台账恢复应直接收尾（旧实现截断后阻塞在 hold 体上）"
        );
        assert!(!dir.join("f.bin.ezr").exists(), "收尾后台账删除");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ①-补：206 单流暂停臂折叠块内进度（旧实现恒落全零台账 + 不可续传
    /// 标记，恢复必从头重下）；恢复 Range 接续至完成
    #[tokio::test]
    async fn single_block_pause_folds_progress_into_ledger() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-pause");
        let total: u64 = 1024 * 1024; // 1 块（慢速门控留出暂停窗）
        let url = mock_server(total, vec![("slow".into(), String::new())]);
        let spec = spec_for(
            1,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            1,
            None,
        );
        let (cmd_tx, mut rx, deadline) = launch_spec(spec);
        let mut paused_at = 0u64;
        let mut pause_sent = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Progress { downloaded, .. }))
                    if !pause_sent && downloaded >= 64 * 1024 =>
                {
                    let _ = cmd_tx.send(TaskCmd::Pause).await;
                    pause_sent = true;
                }
                Ok(Some(Evt::PausedDone { downloaded, .. })) => {
                    paused_at = downloaded;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("慢速流不应失败: {reason}"),
                _ => {}
            }
        }
        assert!(
            paused_at > 0 && paused_at < total,
            "暂停应折叠真实进度（{paused_at}/{total}）"
        );
        let sc = Sidecar::load(dir.join("f.bin.ezr").to_str().unwrap()).expect("暂停台账落盘");
        assert!(!sc.non_resumable, "206 单流暂停台账可续传（旧实现恒 true）");
        assert_eq!(sc.downloaded, paused_at, "台账字节 = PausedDone 字节");
        // 恢复：同一 URL Range 接续至完成
        let spec2 = sidecar_spec(1, &url, &dir, 1);
        let (_tx2, mut rx2, deadline2) = launch_spec(spec2);
        let mut done = false;
        let mut first = true;
        while std::time::Instant::now() < deadline2 {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx2.recv()).await {
                Ok(Some(Evt::Progress { downloaded, .. })) => {
                    if first {
                        assert!(downloaded >= paused_at, "恢复进度不得回退");
                        first = false;
                    }
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("恢复不应失败: {reason}"),
                _ => {}
            }
        }
        assert!(done, "暂停恢复应完成");
        let data = std::fs::read(dir.join("f.bin")).expect("完成文件在位");
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect, "恢复后文件字节完整");
        std::fs::remove_dir_all(&dir).ok();
    }
}
