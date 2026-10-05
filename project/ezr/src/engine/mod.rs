//! engine — 下载引擎（tokio 任务集：探测、分块下载、校验、限速、事件上报）
//!
//! 架构：App（UI 线程）持 [`EngineHandle`] 下发 [`Cmd`]；引擎主循环 spawn 每
//! 任务一个 supervisor，supervisor 经 [`Evt`] 上报探测、
//! 进度、完成、失败与暂停。任务上下文（URL/路径/并发等）由 App 在
//! `Cmd::Start` 时以 [`TaskSpec`] 一次性携带，引擎不持有任务模型，
//! 避免跨线程锁竞争。
//!
//! 窄接口：本模块门面只导出 [`Cmd`]/[`Evt`]/[`ConnView`]/[`EngineHandle`]
//! 与载荷类型 [`TaskSpec`]/[`VerifySpec`]；supervisor/error/throttle 为实现
//! 子模块（私有，app/ui 不可达，防越层 reach-in）。

mod error;
mod supervisor;
mod throttle;

use std::collections::HashMap;
use std::sync::Arc;

// 测试可达性门面：#[path] 挂载的加固测试需直接测实现子模块（throttle/error），
// 经 cfg(test) 门控在门面 re-export（notes/rust.md「门面 re-export」手法）；
// 产品构建（cfg(test)=off）不产生该路径，窄接口不受影响。allow 依据：产品
// crate 自身的测试编译不消费该路径（消费方是独立编译的挂载测试 crate），
// unused_imports 属挂载机制固有告警（三分法 ①，notes/rust.md）。
#[cfg(test)]
#[allow(unused_imports)]
pub use error::classify_reqwest;
/// 窄接口载荷：`Cmd::Start`/`Cmd::Verify` 携带的任务上下文（单源于 supervisor
/// 定义，门面 re-export 供 app 构造；app 不得再 reach 进 supervisor 内部）。
pub use supervisor::{TaskSpec, VerifySpec};
#[cfg(test)]
#[allow(unused_imports)]
pub use throttle::Throttle;
use tokio::sync::mpsc;

/// 引擎命令（App → 引擎）
#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // Start 携带任务上下文（TaskSpec）属业务必需
pub enum Cmd {
    /// 启动任务（探测 + 下载；断点接续由 spec 携带的 sidecar 决定）
    Start { spec: supervisor::TaskSpec },
    /// 暂停任务（停传、写 sidecar，回报 [`Evt::PausedDone`]）
    Pause { id: u32 },
    /// 取消任务运行（删除任务前调用；不写 sidecar）
    Cancel { id: u32 },
    /// 校验请求（下载完成后 App 转校验中并下发）
    Verify { spec: supervisor::VerifySpec },
    /// 关闭引擎（退出）
    Shutdown,
}

/// 引擎事件（引擎 → App，App 每 tick 消费）
#[derive(Debug)]
pub enum Evt {
    /// 探测完成（获槽后；等待中任务此时才获得大小显示）
    Probed {
        /// 任务 ID
        id: u32,
        /// 探测后定稿的文件名（Content-Disposition 优先，FR-01-02）
        name: String,
        /// 重定向后最终 URL
        final_url: String,
        /// 文件总大小（无 Content-Length 时 0）
        total: u64,
        /// 是否支持断点续传
        resumable: bool,
        /// ETag
        etag: Option<String>,
        /// Last-Modified
        last_modified: Option<String>,
    },
    /// 进度快照（每 tick 聚合；App 据此更新 downloaded/连接视图/块数）
    Progress {
        /// 任务 ID
        id: u32,
        /// 已下载字节（权威值）
        downloaded: u64,
        /// 活跃连接视图
        conns: Vec<ConnView>,
        /// 已完成块数
        chunk_done: u32,
    },
    /// 暂停完成（sidecar 已写；App 转已暂停并更新断点视图）
    PausedDone {
        /// 任务 ID
        id: u32,
        /// 已下载字节
        downloaded: u64,
        /// 已完成块数
        chunk_done: u32,
    },
    /// 任务已停止运行（取消：删除任务/排队取消/引擎关闭；不写 sidecar）。
    /// App 收到后执行已登记的延迟文件删除；未知 id 直接忽略。
    Cancelled {
        /// 任务 ID
        id: u32,
    },
    /// 下载失败
    Failed {
        /// 任务 ID
        id: u32,
        /// 失败类别
        kind: crate::model::FailKind,
        /// 失败原因
        reason: String,
        /// Retry-After 秒
        retry_after: Option<f64>,
        /// 本次尝试是否有进展（重试计数连续性判定，FR-01-41）
        made_progress: bool,
        /// 已下载字节（失败时点）
        downloaded: u64,
        /// 已完成块数
        chunk_done: u32,
    },
    /// 续传一致性失效（FR-01-22：sidecar 作废、从头重下、不计失败）
    Invalidated {
        /// 任务 ID
        id: u32,
    },
    /// 下载完成（全块完成 + 大小校验通过）
    DownloadDone {
        /// 任务 ID
        id: u32,
        /// 文件实际大小
        total: u64,
        /// 是否需要校验（false = 无校验值，引擎已完成收尾：改名+删 sidecar）
        has_checksum: bool,
    },
    /// 校验完成（ok = true 时引擎已完成收尾：改名+删 sidecar）
    VerifyDone {
        /// 任务 ID
        id: u32,
        /// 是否通过
        ok: bool,
        /// 实际摘要（十六进制小写）
        computed: String,
        /// 期望摘要
        expected: String,
    },
    /// 用户提示（toast）
    Toast(String),
}

/// 连接视图快照（事件上报给 App 的展示形态；纯数据字段——消费侧适配为
/// 领域连接模型，低层不构造高层类型）
#[derive(Clone, Debug, PartialEq)]
pub struct ConnView {
    /// 连接序号（1 基）
    pub id: usize,
    /// 块号（0 基）
    pub block: u32,
    /// 块起始
    pub start: u64,
    /// 块结束
    pub end: u64,
    /// 已写字节
    pub done: u64,
}

/// 任务运行控制信号（supervisor 内部）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskCmd {
    /// 暂停（停传写 sidecar）
    Pause,
    /// 取消（删除任务；不写 sidecar）
    Cancel,
}

/// 引擎运行时（跨任务共享）
pub(crate) struct EngineShared {
    /// HTTP 客户端（代理/重定向/TLS 配置一次成型）
    pub client: reqwest::Client,
    /// 全局限速器
    throttle: Arc<throttle::Throttle>,
}

/// 引擎句柄（App 持有；命令下发与事件消费的边界；Clone 以便任务内发送）
#[derive(Clone)]
pub struct EngineHandle {
    /// 命令通道
    cmd_tx: mpsc::Sender<Cmd>,
}

impl EngineHandle {
    /// 启动引擎主循环。
    ///
    /// # Panics
    /// HTTP 客户端构建失败时 panic（非法代理等；代理地址合法性应在配置期校验）。
    pub fn start(cfg: &crate::model::config::Config, evt_tx: mpsc::Sender<Evt>) -> EngineHandle {
        let mut cb = reqwest::Client::builder()
            // FR-01-14：最多跟随 10 次重定向
            .redirect(reqwest::redirect::Policy::limited(10))
            // FR-01-61：代理仅配置文件，不读取环境变量（no_proxy 关闭系统代理解析）
            .no_proxy()
            .connect_timeout(std::time::Duration::from_secs(15))
            .read_timeout(std::time::Duration::from_secs(30));
        if let Some(p) = &cfg.proxy {
            match reqwest::Proxy::all(p) {
                Ok(px) => {
                    cb = cb.proxy(px);
                }
                Err(_) => {
                    let tx = evt_tx.clone();
                    let p = p.clone();
                    tokio::spawn(async move {
                        let _ = tx
                            .send(Evt::Toast(format!("⚠ 配置的代理地址无效（{p}），已忽略")))
                            .await;
                    });
                }
            }
        }
        let client = cb.build().expect("HTTP 客户端构建失败");
        let shared = Arc::new(EngineShared {
            client,
            throttle: Arc::new(throttle::Throttle::new(cfg.max_speed)),
        });
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>(64);
        tokio::spawn(main_loop(shared, cmd_rx, evt_tx));
        EngineHandle { cmd_tx }
    }

    /// 下发命令（异步）
    pub async fn send(&self, cmd: Cmd) {
        let _ = self.cmd_tx.send(cmd).await;
    }
}

/// 引擎主循环：接收 Cmd、管理 supervisor 生命周期（逐命令决策见 [`handle_cmd`]）
async fn main_loop(
    shared: Arc<EngineShared>,
    mut cmd_rx: mpsc::Receiver<Cmd>,
    evt_tx: mpsc::Sender<Evt>,
) {
    let mut tasks: HashMap<u32, mpsc::Sender<TaskCmd>> = HashMap::new();
    while let Some(cmd) = cmd_rx.recv().await {
        if handle_cmd(&mut tasks, cmd, &shared, &evt_tx).await {
            break;
        }
    }
}

/// 处理单条命令（main_loop 的逐命令决策核；返回 `true` 表示 Shutdown 退出）
async fn handle_cmd(
    tasks: &mut HashMap<u32, mpsc::Sender<TaskCmd>>,
    cmd: Cmd,
    shared: &Arc<EngineShared>,
    evt_tx: &mpsc::Sender<Evt>,
) -> bool {
    match cmd {
        Cmd::Start { spec } => {
            start_task(tasks, spec, shared, evt_tx);
            false
        }
        Cmd::Pause { id } => {
            pause_task(tasks, id).await;
            false
        }
        Cmd::Cancel { id } => {
            cancel_task(tasks, id, evt_tx).await;
            false
        }
        Cmd::Verify { spec } => {
            spawn_verify(spec, evt_tx);
            false
        }
        Cmd::Shutdown => {
            shutdown_all(tasks).await;
            true
        }
    }
}

/// Start：为任务建命令通道并孵化 supervisor
fn start_task(
    tasks: &mut HashMap<u32, mpsc::Sender<TaskCmd>>,
    spec: supervisor::TaskSpec,
    shared: &Arc<EngineShared>,
    evt_tx: &mpsc::Sender<Evt>,
) {
    let (tx, rx) = mpsc::channel::<TaskCmd>(8);
    tasks.insert(spec.id, tx.clone());
    tokio::spawn(supervisor::run(spec, shared.clone(), rx, evt_tx.clone()));
}

/// Pause：向存活任务句柄转发暂停命令（未知/已亡句柄静默忽略）
async fn pause_task(tasks: &HashMap<u32, mpsc::Sender<TaskCmd>>, id: u32) {
    if let Some(h) = tasks.get(&id) {
        let _ = h.send(TaskCmd::Pause).await;
    }
}

/// Cancel：移除句柄并转发取消；句柄已死（任务早已结束：完成/暂停/失败后
/// supervisor 退出但 map 条目保留）或未知 id（未在运行）→ 直接回报停止确认，
/// 让 App 的「删除任务和文件」延迟删除得以执行（FR-01-25）
async fn cancel_task(
    tasks: &mut HashMap<u32, mpsc::Sender<TaskCmd>>,
    id: u32,
    evt_tx: &mpsc::Sender<Evt>,
) {
    match tasks.remove(&id) {
        Some(h) => {
            if h.send(TaskCmd::Cancel).await.is_err() {
                let _ = evt_tx.send(Evt::Cancelled { id }).await;
            }
        }
        None => {
            let _ = evt_tx.send(Evt::Cancelled { id }).await;
        }
    }
}

/// Verify：孵化一次性校验流程
fn spawn_verify(spec: supervisor::VerifySpec, evt_tx: &mpsc::Sender<Evt>) {
    let tx = evt_tx.clone();
    tokio::spawn(async move {
        supervisor::verify(spec, tx).await;
    });
}

/// Shutdown：向全部存活任务广播取消后退出主循环
async fn shutdown_all(tasks: &mut HashMap<u32, mpsc::Sender<TaskCmd>>) {
    for (_, h) in tasks.drain() {
        let _ = h.send(TaskCmd::Cancel).await;
    }
}

#[cfg(test)]
mod main_loop_tests {
    use super::*;

    /// 与 supervisor 测试同口径：无限速 + 关闭环境代理解析
    fn shared_free() -> EngineShared {
        EngineShared {
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            throttle: Arc::new(throttle::Throttle::new(0)),
        }
    }

    fn spec_bogus(id: u32) -> supervisor::TaskSpec {
        supervisor::TaskSpec {
            id,
            url: "http://127.0.0.1:1/ezr-ml.bin".to_string(),
            save_dir: std::env::temp_dir()
                .join("ezr-ml")
                .to_string_lossy()
                .into_owned(),
            name: format!("ezr-ml-{id}.bin"),
            concurrency: 2,
            block_size: 1024 * 1024,
            protocol: crate::model::Protocol::Http,
            expected_algo: None,
            expected_value: None,
            sidecar: None,
            sidecar_path: None,
            keep_name: false,
            added_at: 0,
        }
    }

    #[tokio::test]
    async fn cancel_unknown_id_reports_cancelled() {
        let (evt_tx, mut evt_rx) = mpsc::channel::<Evt>(8);
        let mut tasks = HashMap::new();
        cancel_task(&mut tasks, 7, &evt_tx).await;
        assert!(matches!(evt_rx.recv().await, Some(Evt::Cancelled { id }) if id == 7));
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn cancel_dead_handle_reports_cancelled() {
        let (evt_tx, mut evt_rx) = mpsc::channel::<Evt>(8);
        let mut tasks = HashMap::new();
        let (h, rx) = mpsc::channel::<TaskCmd>(1);
        tasks.insert(3u32, h);
        drop(rx); // supervisor 已退出 → 句柄发送必失败
        cancel_task(&mut tasks, 3, &evt_tx).await;
        assert!(matches!(evt_rx.recv().await, Some(Evt::Cancelled { id }) if id == 3));
    }

    #[tokio::test]
    async fn cancel_live_handle_forwards_without_event() {
        let (evt_tx, mut evt_rx) = mpsc::channel::<Evt>(8);
        let mut tasks = HashMap::new();
        let (h, mut rx) = mpsc::channel::<TaskCmd>(1);
        tasks.insert(5u32, h);
        cancel_task(&mut tasks, 5, &evt_tx).await;
        assert_eq!(rx.recv().await, Some(TaskCmd::Cancel));
        assert!(evt_rx.try_recv().is_err()); // 存活句柄不补发确认
        assert!(tasks.is_empty()); // 句柄已移除
    }

    #[tokio::test]
    async fn pause_forwards_only_to_live_handle() {
        let mut tasks = HashMap::new();
        let (h, mut rx) = mpsc::channel::<TaskCmd>(1);
        tasks.insert(9u32, h);
        pause_task(&tasks, 9).await;
        assert_eq!(rx.recv().await, Some(TaskCmd::Pause));
        pause_task(&tasks, 404).await; // 未知 id：静默
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn shutdown_all_drains_and_broadcasts_cancel() {
        let mut tasks = HashMap::new();
        let (h1, mut r1) = mpsc::channel::<TaskCmd>(1);
        let (h2, mut r2) = mpsc::channel::<TaskCmd>(1);
        tasks.insert(1u32, h1);
        tasks.insert(2u32, h2);
        shutdown_all(&mut tasks).await;
        assert_eq!(r1.recv().await, Some(TaskCmd::Cancel));
        assert_eq!(r2.recv().await, Some(TaskCmd::Cancel));
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn handle_cmd_dispatches_and_shutdown_reports_exit() {
        let shared = Arc::new(shared_free());
        let (evt_tx, mut evt_rx) = mpsc::channel::<Evt>(64);
        let mut tasks = HashMap::new();

        // Start：孵化 supervisor（bogus URL 异步失败，事件被丢弃不阻塞）
        assert!(
            !handle_cmd(
                &mut tasks,
                Cmd::Start {
                    spec: spec_bogus(11)
                },
                &shared,
                &evt_tx
            )
            .await
        );
        assert!(tasks.contains_key(&11));

        // Pause / Cancel(未知) / Verify：均不退出
        assert!(!handle_cmd(&mut tasks, Cmd::Pause { id: 11 }, &shared, &evt_tx).await);
        assert!(!handle_cmd(&mut tasks, Cmd::Cancel { id: 999 }, &shared, &evt_tx).await);
        assert!(matches!(evt_rx.recv().await, Some(Evt::Cancelled { id }) if id == 999));
        let vspec = supervisor::VerifySpec {
            id: 11,
            path: "/nonexistent/a.downloading".to_string(),
            final_path: "/nonexistent/a.bin".to_string(),
            sidecar_path: "/nonexistent/a.sidecar".to_string(),
            algo: "MD5",
            expected: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
        };
        assert!(!handle_cmd(&mut tasks, Cmd::Verify { spec: vspec }, &shared, &evt_tx).await);

        // Shutdown：广播取消并报告退出
        assert!(handle_cmd(&mut tasks, Cmd::Shutdown, &shared, &evt_tx).await);
        assert!(tasks.is_empty());
    }
}
