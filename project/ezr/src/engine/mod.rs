//! engine — 下载引擎（tokio 任务集：探测、分块下载、校验、限速、事件上报）
//!
//! 架构：App（UI 线程）持 [`EngineHandle`] 下发 [`Cmd`]；引擎主循环 spawn 每
//! 任务一个 supervisor（见 [`supervisor`]），supervisor 经 [`Evt`] 上报探测、
//! 进度、完成、失败与暂停。任务上下文（URL/路径/并发等）由 App 在
//! `Cmd::Start` 时以 [`supervisor::TaskSpec`] 一次性携带，引擎不持有任务模型，
//! 避免跨线程锁竞争。
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_wrap)]
// 字节/速度/时间算术在 u64-f64 间转换是下载器领域固有；边界由调用方保证
#![allow(clippy::missing_const_for_fn)] // nursery 误报为主（含 trait impl 场景）
#![allow(clippy::doc_markdown, clippy::doc_lazy_continuation)] // 中文文档中英文术语不强制反引号
#![allow(clippy::float_cmp)] // 速度/时间为 0 的语义判断使用精确比较
#![allow(clippy::map_unwrap_or, clippy::option_if_let_else, clippy::unnested_or_patterns)]
#![allow(clippy::cognitive_complexity, clippy::too_many_lines)] // 分块计算/状态机逻辑固有复杂度


pub mod error;
pub mod supervisor;
pub mod throttle;

use std::collections::HashMap;
use std::sync::Arc;

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
#[allow(clippy::large_enum_variant)] // Failed 携带断点快照（块表）属业务必需
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
        /// Content-Disposition 文件名（FR-01-02 首优先级；引擎内消费，App 以 name 同步）
        #[allow(dead_code)]
        cd_name: Option<String>,
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
        /// 块表快照
        blocks: Vec<u64>,
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

/// 连接视图快照（事件上报给 App 的展示形态）
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

impl ConnView {
    /// 转 UI 连接模型（速度字段由 App 滑窗回填）
    #[must_use]
    pub fn to_connection(&self) -> crate::model::Connection {
        crate::model::Connection {
            id: self.id,
            start: self.start,
            end: self.end,
            done: self.done,
            speed: 0.0,
        }
    }
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
    pub throttle: Arc<throttle::Throttle>,
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

/// 引擎主循环：接收 Cmd、管理 supervisor 生命周期
async fn main_loop(
    shared: Arc<EngineShared>,
    mut cmd_rx: mpsc::Receiver<Cmd>,
    evt_tx: mpsc::Sender<Evt>,
) {
    let mut tasks: HashMap<u32, mpsc::Sender<TaskCmd>> = HashMap::new();
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            Cmd::Start { spec } => {
                let (tx, rx) = mpsc::channel::<TaskCmd>(8);
                tasks.insert(spec.id, tx.clone());
                tokio::spawn(supervisor::run(spec, shared.clone(), rx, evt_tx.clone()));
            }
            Cmd::Pause { id } => {
                if let Some(h) = tasks.get(&id) {
                    let _ = h.send(TaskCmd::Pause).await;
                }
            }
            Cmd::Cancel { id } => {
                match tasks.remove(&id) {
                    Some(h) => {
                        if h.send(TaskCmd::Cancel).await.is_err() {
                            // 句柄已死（任务早已结束：完成/暂停/失败后 supervisor
                            // 退出但 map 条目保留）：直接回报停止确认，让 App 的
                            // 「删除任务和文件」延迟删除得以执行（FR-01-25）
                            let _ = evt_tx.send(Evt::Cancelled { id }).await;
                        }
                    }
                    None => {
                        // 未知 id（未在运行）：同上，直接回报停止确认
                        let _ = evt_tx.send(Evt::Cancelled { id }).await;
                    }
                }
            }
            Cmd::Verify { spec } => {
                let tx = evt_tx.clone();
                tokio::spawn(async move {
                    supervisor::verify(spec, tx).await;
                });
            }
            Cmd::Shutdown => {
                for (_, h) in tasks.drain() {
                    let _ = h.send(TaskCmd::Cancel).await;
                }
                break;
            }
        }
    }
}
