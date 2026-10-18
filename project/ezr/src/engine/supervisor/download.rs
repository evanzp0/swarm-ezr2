//! supervisor::download — 多块监督主流程
//!
//! 职责（architect v145 拆分，纯移动）：探测 select（命令在途即响应）、
//! 续传一致性检查、文件名定稿、文件预分配、worker 池装配与监督循环
//! （命令臂 / 进度 tick 臂 / 完成判定 / sidecar 周期落盘）。

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

/// 进度事件聚合间隔（与 UI tick 同频）
const PROGRESS_TICK: Duration = Duration::from_millis(100);
/// sidecar 周期落盘间隔（NFR-2）
const SIDECAR_FLUSH: Duration = Duration::from_secs(2);

/// 下载主流程
pub(super) async fn download(
    spec: &TaskSpec,
    shared: &Arc<super::super::EngineShared>,
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
        etag: head.etag.clone(),
        last_modified: head.last_modified.clone(),
        size: (head.total > 0).then_some(head.total),
    };

    // 文件名定稿（FR-01-02：CD → 探测请求跳转落点末段（该次连接当前值）→
    // 原始 URL 末段 → 时间戳）；断点接续（spec 带 sidecar）沿用既有任务名，
    // 避免另存新文件（FR-01-26）。
    // 非接续路径定稿前做盘上去重（Gherkin 01-add-task-10：目标已存在时追加
    // 序号，防止下载覆盖既有文件；与 App 添加期去重同口径、结果一致）
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
    // 探测完成（App 更新 total/resumable/probed/首探定名标记与大小显示；
    // v1.28：事件不再携带最终 URL——任务级 URL 权威恒为原始 URL）
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
    // 探测响应体（bytes=0- = 整文件流）在多块路径不再需要：立即释放探测连接
    // ——否则未读响应体钉死该连接（服务端阻塞在写体/连接闲置），与 worker
    // 的新建连接争用。单流路径（上方）仍移交 resp 续读。
    // v1.28（FR-01-14 ②）：worker 不再共享探测捕获的落点——各请求均从原始
    // URL（spec.url）发起并独立跟跳转（reqwest Policy::limited(10) 按请求链
    // 跳转），落点不缓存不复用。
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
                let done_blocks = b.done_indices();
                let partial_blocks = b.partial_indices();
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
                    done_blocks,
                    partial_blocks,
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
