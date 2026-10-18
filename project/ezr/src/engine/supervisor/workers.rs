//! supervisor::workers — 分块下载 worker 池
//!
//! 职责（architect v145 拆分，纯移动）：worker 共享依赖包（WorkerDeps 参数
//! 对象）、块 worker 主循环（领块 → Range 下载 → 写盘）、池管理三件套
//! （stop 全停 / prune 收割 / fill 补足）。

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

/// 停止全部 worker（stop 标志 + watch 广播 + 收割句柄）；监督循环各退出路径共用
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

/// 收割已结束 worker 句柄（待命退出/配额下调退出；监督循环 tick 与
/// Reconfigure 时调用，保持存活集合与槽位号准确）
pub(super) fn prune_finished(handles: &mut Vec<(usize, tokio::task::JoinHandle<()>)>) {
    handles.retain(|(_, h)| !h.is_finished());
}

/// 补足 worker 至目标配额 q（v1.5/FR-01-87 扩容 spawn 单源；v1.16/D30
/// 进度 tick 兜底对账复用——缩容侧 worker 退出收割与扩容 spawn 之间的
/// 瞬时缺口由下一进度帧自愈）。槽位号取最小空闲编号（退出可复用）
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

/// worker 共享依赖包（spawn 参数对象）：分块下载路径的全部跨 worker 共享
/// 句柄与任务上下文。依赖清单唯一登记于本结构体定义；`spawn` 是唯一 spawn
/// 调用点——初始 worker 池与 Reconfigure 扩容共用（架构轮：原两处 12 参
/// 调用表收敛，防漂移单源）。字段全部 Clone，逐 worker 克隆后转移所有权。
#[derive(Clone)]
pub(super) struct WorkerDeps {
    /// 下载起点 URL（v1.28：原始输入 URL——各 worker 每个请求独立跟跳转，
    /// 不复用探测落点，FR-01-14 ②）
    pub(super) url: String,
    /// `.downloading` 目标文件路径
    pub(super) file_path: PathBuf,
    /// 共享块视图（领块与写盘记账）
    pub(super) blocks: Arc<tokio::sync::Mutex<Blocks>>,
    /// 全局块租约序号（原子领块指针）
    pub(super) lease_next: Arc<AtomicU32>,
    /// 配额中止块再领队列（v1.16/D30：单调租约游标不回退，被中止在传块
    /// 经此队列交还后续 worker 从块内偏移接续；pop 单领由互斥保证）
    pub(super) requeue: Arc<tokio::sync::Mutex<Vec<u32>>>,
    /// 停止信号（暂停/取消/失败收尾）
    pub(super) stop_rx: watch::Receiver<bool>,
    /// 目标 worker 数（下调时多余 worker 完成当前块后自行退出）
    pub(super) quota_rx: watch::Receiver<usize>,
    /// 任务代理端点（每次领块解析 client——切换后新连接即新代理）
    pub(super) ep_rx: watch::Receiver<Arc<Option<ProxyEndpoint>>>,
    /// 全任务累计写字节（进度上报）
    pub(super) total_written: Arc<AtomicU64>,
    /// 首个致命失败（任一 worker 登记后监督循环收尾）
    pub(super) failure: Arc<tokio::sync::Mutex<Option<EngineFailure>>>,
    /// 活跃连接视图（进度事件携带）
    pub(super) conns: Arc<tokio::sync::Mutex<Vec<ConnView>>>,
}

impl WorkerDeps {
    /// 克隆本依赖包产出第 `wid` 号 worker（依赖集单一来源：WorkerDeps 定义）
    pub(super) fn spawn(
        &self,
        wid: usize,
        shared: Arc<super::super::EngineShared>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(block_worker(wid, shared, self.clone()))
    }
}

/// 块 worker：领块 → Range 下载 → 写盘；无块可领或停止信号时退出
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
        // 并发下调（v1.5/FR-01-87；下调臂 v1.16/D30 改判即时收缩——内层
        // select 配额臂中止在传块，块交还再领队列）：超出目标配额的 worker
        // 立即退出；槽位 freed 后由 Reconfigure/进度 tick 在扩容时复用。
        // v133（操作者实证：缩容后面板残留无效连接行）：块边界退出也须
        // 清自身连接行——上一块租约遗留的行仍留在共享视图，不清则面板
        // 多出超额槽位的僵死条目
        if wid > *quota_rx.borrow_and_update() {
            conns.lock().await.retain(|c| c.id != wid);
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
        let mut resp = match req_checked(&client, &url, Some(range)).await {
            Ok(r) => r,
            Err(f) => {
                *failure.lock().await = Some(f);
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
