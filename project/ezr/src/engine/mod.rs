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
    /// 任务参数热调（v1.5/FR-01-87，D19：并发=下一调度周期、代理=新连接）。
    /// 未知/已停任务静默忽略（排队/暂停任务的修改在下次 Start 按新 spec 生效）
    Reconfigure {
        id: u32,
        concurrency: usize,
        endpoint: Option<crate::model::ProxyEndpoint>,
    },
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskCmd {
    /// 暂停（停传写 sidecar）
    Pause,
    /// 取消（删除任务；不写 sidecar）
    Cancel,
    /// 任务参数热调（v1.5/FR-01-87：并发/代理；supervisor 监督循环消费）
    Reconfigure {
        concurrency: usize,
        endpoint: Option<crate::model::ProxyEndpoint>,
    },
}

/// 引擎运行时（跨任务共享）
pub(crate) struct EngineShared {
    /// 直连 HTTP 客户端（no_proxy，FR-01-61 不读环境代理解析；任务代理
    /// 选择解析为 None 时的基线 client，v1.5/FR-01-86）
    pub client: reqwest::Client,
    /// 代理客户端池（v1.5/FR-01-86：key = 端点指纹 url\u{1f}user\u{1f}password；
    /// reqwest 代理是 client 级，任务级代理经查池实现按连接切换——见
    /// packs/_common/notes/rust.md 沉淀条）
    clients: std::sync::Mutex<HashMap<String, reqwest::Client>>,
    /// 全局限速器
    throttle: Arc<throttle::Throttle>,
}

impl EngineShared {
    /// 按任务代理端点解析客户端（None = 直连基线；Some = 查池 get_or_build，
    /// 构建失败回退直连——合法性由调用方 [`endpoint_valid`] 预检并 toast）
    pub(crate) fn client_for(&self, ep: Option<&crate::model::ProxyEndpoint>) -> reqwest::Client {
        let Some(ep) = ep else {
            return self.client.clone();
        };
        let key = proxy_fingerprint(ep);
        let mut map = self.clients.lock().expect("代理客户端池锁");
        if let Some(c) = map.get(&key) {
            return c.clone();
        }
        let client = build_endpoint_client(ep).unwrap_or_else(|| self.client.clone());
        map.insert(key, client.clone());
        client
    }
}

/// 代理端点指纹（client 池 key；账号纳入指纹避免同 url 不同凭据撞池；
/// 仅驻内存，不写日志不显界面——FR-01-86 认证口径）
fn proxy_fingerprint(ep: &crate::model::ProxyEndpoint) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}",
        ep.url,
        ep.username.as_deref().unwrap_or(""),
        ep.password.as_deref().unwrap_or("")
    )
}

/// 端点代理 url 可构建性预检（reqwest::Proxy::all；非法时调用方 toast + 直连）
pub(crate) fn endpoint_valid(ep: &crate::model::ProxyEndpoint) -> bool {
    reqwest::Proxy::all(&ep.url).is_ok()
}

/// socks5 认证 url 构造（v1.8/FR-01-93 ③）：把无凭证 base url 与凭证编码为
/// `socks5://user:pass@host:port`——reqwest 解析 socks url 的 userinfo 时
/// percent-decode 后走 RFC 1929 user/pass 子协商（`Proxy::basic_auth` 仅产生
/// HTTP Proxy-Authorization 头，对 socks 代理无效）。percent-encode 由 url
/// crate 完成（特殊字符/非 ASCII 凭证往返无损）。base 非 socks5 scheme 或
/// 解析失败 → None。认证 url 仅在引擎构建 client 时内部构造，不落盘不显示。
fn socks5_auth_url(base: &str, user: &str, pass: &str) -> Option<String> {
    let mut u = url::Url::parse(base).ok()?;
    if u.scheme() != "socks5" {
        return None;
    }
    u.set_username(user).ok()?;
    u.set_password(Some(pass)).ok()?;
    Some(u.to_string())
}

/// 按端点构建客户端（重定向/超时口径与直连基线一致）。凭证传递按类型分级
/// （v1.8/FR-01-93 ③；v1.9 https 与 http 同级）：socks5 型凭证齐备 → 内部构造
/// 认证 url（userinfo percent-decode → RFC 1929 user/pass 握手），凭证缺失
/// （防御路径，config 校验链已保证不出现）→ 匿名 socks5；http 与 https 型
/// （v1.9/FR-01-89：https = 代理自身走 TLS 的 HTTP 代理，`Proxy::all` 原生支持）
/// 成对凭证 → `basic_auth`——TLS 会话内 Proxy-Authorization 照常生效，
/// 无需按 scheme 分支（packs/_common/notes/rust.md 口径）。
fn build_endpoint_client(ep: &crate::model::ProxyEndpoint) -> Option<reqwest::Client> {
    let px = match ep.kind {
        crate::model::config::ProxyKind::Socks5 => {
            // 认证 url 构造意外失败（base 已带合法 socks5 scheme，实践中不可达）
            // → 退回无凭证 url，与 endpoint_valid 预检同一来源，避免静默直连
            let target = match (&ep.username, &ep.password) {
                (Some(u), Some(pw)) => {
                    socks5_auth_url(&ep.url, u, pw).unwrap_or_else(|| ep.url.clone())
                }
                _ => ep.url.clone(),
            };
            reqwest::Proxy::all(&target).ok()?
        }
        crate::model::config::ProxyKind::Http | crate::model::config::ProxyKind::Https => {
            let mut px = reqwest::Proxy::all(&ep.url).ok()?;
            if let (Some(u), Some(pw)) = (&ep.username, &ep.password) {
                px = px.basic_auth(u, pw);
            }
            px
        }
    };
    base_client_builder().proxy(px).build().ok()
}

/// 客户端策略单一事实来源（DRY 收敛：直连基线与端点 client 共用）：
/// 重定向上限 10（FR-01-14）、不读环境变量代理（FR-01-61）、
/// 连接 15s / 读 30s 超时
fn base_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        // FR-01-14：最多跟随 10 次重定向
        .redirect(reqwest::redirect::Policy::limited(10))
        // FR-01-61：代理仅配置文件，不读取环境变量（no_proxy 关闭系统代理解析）
        .no_proxy()
        .connect_timeout(std::time::Duration::from_secs(15))
        .read_timeout(std::time::Duration::from_secs(30))
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
        // 直连基线 client（v1.5/FR-01-86：全局/命名代理一律走端点解析 +
        // client 池；全局 proxy 键语义 = 「默认代理」由 App 按 D18 解析）
        let client = base_client_builder().build().expect("HTTP 客户端构建失败");
        let shared = Arc::new(EngineShared {
            client,
            clients: std::sync::Mutex::new(HashMap::new()),
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
        Cmd::Reconfigure {
            id,
            concurrency,
            endpoint,
        } => {
            // 端点合法性预检（v1.5/FR-01-86 口径：非法 toast 一次 + 直连回退）
            let endpoint = match &endpoint {
                Some(ep) if !endpoint_valid(ep) => {
                    let tx = evt_tx.clone();
                    let url = ep.url.clone();
                    tokio::spawn(async move {
                        let _ = tx
                            .send(Evt::Toast(format!(
                                "⚠ 配置的代理地址无效（{url}），已按直连处理"
                            )))
                            .await;
                    });
                    None
                }
                other => other.clone(),
            };
            reconfigure_task(tasks, id, concurrency, endpoint).await;
            false
        }
        Cmd::Shutdown => {
            shutdown_all(tasks).await;
            true
        }
    }
}

/// Start：为任务建命令通道并孵化 supervisor。spec 携带的代理端点先做
/// 合法性预检（v1.5/FR-01-86：非法 toast 一次 + 直连回退，与全局代理
/// 既有口径一致）
fn start_task(
    tasks: &mut HashMap<u32, mpsc::Sender<TaskCmd>>,
    mut spec: supervisor::TaskSpec,
    shared: &Arc<EngineShared>,
    evt_tx: &mpsc::Sender<Evt>,
) {
    if spec
        .proxy_endpoint
        .as_ref()
        .is_some_and(|ep| !endpoint_valid(ep))
    {
        let url = spec
            .proxy_endpoint
            .as_ref()
            .map(|ep| ep.url.clone())
            .unwrap_or_default();
        spec.proxy_endpoint = None;
        let tx = evt_tx.clone();
        tokio::spawn(async move {
            let _ = tx
                .send(Evt::Toast(format!(
                    "⚠ 配置的代理地址无效（{url}），已按直连处理"
                )))
                .await;
        });
    }
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

/// Reconfigure：向存活任务句柄转发参数热调（未知/已亡句柄静默忽略——
/// 排队/暂停任务的修改在下次 Start 按 make_spec 的新值生效）
async fn reconfigure_task(
    tasks: &HashMap<u32, mpsc::Sender<TaskCmd>>,
    id: u32,
    concurrency: usize,
    endpoint: Option<crate::model::ProxyEndpoint>,
) {
    if let Some(h) = tasks.get(&id) {
        let _ = h
            .send(TaskCmd::Reconfigure {
                concurrency,
                endpoint,
            })
            .await;
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
            clients: std::sync::Mutex::new(HashMap::new()),
            throttle: Arc::new(throttle::Throttle::new(0)),
        }
    }

    fn spec_bogus(id: u32) -> supervisor::TaskSpec {
        supervisor::TaskSpec {
            id,
            url: "http://127.0.0.1:1/ezr-ml.bin".to_string(),
            save_dir: crate::model::testenv::uniq_tmp_dir("ezr-ml")
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
            proxy_endpoint: None,
        }
    }

    /// v1.5/FR-01-87：Reconfigure 对未知 id 静默忽略（排队/已停任务不在
    /// 句柄表；不 panic、不退出主循环）
    #[tokio::test]
    async fn reconfigure_unknown_id_is_noop() {
        let mut tasks: HashMap<u32, mpsc::Sender<TaskCmd>> = HashMap::new();
        let shared = Arc::new(shared_free());
        let (tx, _rx) = mpsc::channel::<Evt>(8);
        let exited = handle_cmd(
            &mut tasks,
            Cmd::Reconfigure {
                id: 999,
                concurrency: 4,
                endpoint: None,
            },
            &shared,
            &tx,
        )
        .await;
        assert!(!exited, "Reconfigure 不退出主循环");
    }

    /// v1.5/FR-01-86：非法代理端点在 Reconfigure 时 toast + 按直连转发
    #[tokio::test]
    async fn reconfigure_invalid_endpoint_falls_back_to_direct() {
        let mut tasks: HashMap<u32, mpsc::Sender<TaskCmd>> = HashMap::new();
        let shared = Arc::new(shared_free());
        let (tx, mut rx) = mpsc::channel::<Evt>(8);
        // 注册一个任务句柄通道，捕获转发后的 endpoint
        let (ttx, mut trx) = mpsc::channel::<TaskCmd>(8);
        tasks.insert(7u32, ttx);
        let exited = handle_cmd(
            &mut tasks,
            Cmd::Reconfigure {
                id: 7,
                concurrency: 2,
                endpoint: Some(crate::model::ProxyEndpoint {
                    // ":://" 为 URI 解析必然失败形态（reqwest Proxy::all Err）
                    url: ":://bad".to_string(),
                    kind: crate::model::config::ProxyKind::Http,
                    username: None,
                    password: None,
                }),
            },
            &shared,
            &tx,
        )
        .await;
        assert!(!exited);
        match trx.recv().await {
            Some(TaskCmd::Reconfigure {
                concurrency,
                endpoint,
            }) => {
                assert_eq!(concurrency, 2);
                assert!(endpoint.is_none(), "非法端点按直连转发");
            }
            other => panic!("应转发 Reconfigure：{other:?}"),
        }
        // toast 提醒一次
        let mut warned = false;
        while let Ok(Some(ev)) =
            tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await
        {
            if let Evt::Toast(m) = ev {
                if m.contains("代理地址无效") {
                    warned = true;
                }
            }
        }
        assert!(warned, "应 toast 代理地址无效");
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

// ===== socks5 认证引擎语义（v1.8/FR-01-93 ③：凭证经 userinfo 走 RFC 1929）=====

#[cfg(test)]
mod proxy_auth_tests {
    use super::*;
    use crate::model::config::ProxyKind;

    /// 测试用最小 percent-decode（reqwest 消费 socks url userinfo 的同款语义：
    /// 合法 %XX 序列解码、非法序列原样通过）——凭证经 percent-encode 往返无损
    fn pct_decode(s: &str) -> String {
        let b = s.as_bytes();
        let mut out: Vec<u8> = Vec::with_capacity(b.len());
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%'
                && i + 2 < b.len()
                && b[i + 1].is_ascii_hexdigit()
                && b[i + 2].is_ascii_hexdigit()
            {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 3;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        String::from_utf8(out).expect("userinfo 解码应为 UTF-8")
    }

    fn ep(
        kind: ProxyKind,
        url: &str,
        user: Option<&str>,
        pass: Option<&str>,
    ) -> crate::model::ProxyEndpoint {
        crate::model::ProxyEndpoint {
            url: url.to_string(),
            kind,
            username: user.map(str::to_string),
            password: pass.map(str::to_string),
        }
    }

    /// 常规凭证：认证 url 保留 scheme/host/port，userinfo 经编码可解出原凭证
    #[test]
    fn socks5_auth_url_basic() {
        let out =
            socks5_auth_url("socks5://127.0.0.1:1080", "u", "p").expect("socks5 应构造认证 url");
        let u = url::Url::parse(&out).expect("认证 url 应可解析");
        assert_eq!(u.scheme(), "socks5");
        assert_eq!(u.host_str(), Some("127.0.0.1"));
        assert_eq!(u.port(), Some(1080));
        assert_eq!(pct_decode(u.username()), "u");
        assert_eq!(pct_decode(u.password().unwrap_or_default()), "p");
    }

    /// 特殊字符凭证（`p@ss:wo/rd` / `100%`）：percent-encode 后 host/port 仍解析
    /// 正确（凭证中的 @ : / % 不破坏 authority），reqwest percent-decode 往返无损
    #[test]
    fn socks5_auth_url_special_chars_roundtrip() {
        let out =
            socks5_auth_url("socks5://h:1", "p@ss:wo/rd", "100%").expect("特殊字符凭证应构造成功");
        let u = url::Url::parse(&out).expect("认证 url 应可解析");
        assert_eq!(u.host_str(), Some("h"), "凭证中的 @ 不破坏 authority");
        assert_eq!(u.port(), Some(1), "凭证中的 : 不破坏端口解析");
        assert_eq!(pct_decode(u.username()), "p@ss:wo/rd");
        assert_eq!(pct_decode(u.password().unwrap_or_default()), "100%");
    }

    /// 非 ASCII 与空格凭证：UTF-8 percent-encode 往返无损
    #[test]
    fn socks5_auth_url_unicode_and_space_roundtrip() {
        let out =
            socks5_auth_url("socks5://h:1", "用 户", "密 码").expect("非 ASCII 凭证应构造成功");
        let u = url::Url::parse(&out).expect("认证 url 应可解析");
        assert_eq!(u.host_str(), Some("h"));
        assert_eq!(pct_decode(u.username()), "用 户");
        assert_eq!(pct_decode(u.password().unwrap_or_default()), "密 码");
    }

    /// 非 socks5 scheme / 解析失败 → None（`Proxy::basic_auth` 对 socks 无效，
    /// 认证 url 构造只服务 socks5 型）
    #[test]
    fn socks5_auth_url_rejects_non_socks5() {
        assert_eq!(socks5_auth_url("http://127.0.0.1:8080", "u", "p"), None);
        assert_eq!(socks5_auth_url("https://127.0.0.1:8443", "u", "p"), None);
        assert_eq!(
            socks5_auth_url(":://bad", "u", "p"),
            None,
            "解析失败 → None"
        );
    }

    /// 场景 15 引擎层：socks5 带凭证端点可构建 client（认证 url 引擎内部构造）
    #[test]
    fn build_client_socks5_with_credentials() {
        let e = ep(
            ProxyKind::Socks5,
            "socks5://127.0.0.1:1080",
            Some("u"),
            Some("p"),
        );
        assert!(build_endpoint_client(&e).is_some(), "socks5 带凭证 → Some");
    }

    /// 防御路径：socks5 无凭证（config 校验链已保证不出现）→ 匿名 socks5 client
    #[test]
    fn build_client_socks5_without_credentials_defensive() {
        let e = ep(ProxyKind::Socks5, "socks5://127.0.0.1:1080", None, None);
        assert!(
            build_endpoint_client(&e).is_some(),
            "socks5 无凭证防御 → Some"
        );
    }

    /// http 型回归不变：无凭证与成对凭证均可构建（basic_auth 路径不动）
    #[test]
    fn build_client_http_regression() {
        let anon = ep(ProxyKind::Http, "http://127.0.0.1:8080", None, None);
        assert!(build_endpoint_client(&anon).is_some(), "http 匿名 → Some");
        let auth = ep(
            ProxyKind::Http,
            "http://127.0.0.1:8080",
            Some("u"),
            Some("p"),
        );
        assert!(
            build_endpoint_client(&auth).is_some(),
            "http 成对凭证 → Some"
        );
    }

    /// v1.9/FR-01-89/93（场景 15 https 相位）：https 型端点 = 代理自身走 TLS 的
    /// HTTP 代理——与 http 型同路（成对凭证 basic_auth 在 TLS 会话内生效）；
    /// 匿名/成对两相位均可构建 client（回归口径）
    #[test]
    fn build_client_https_with_credentials() {
        let auth = ep(
            ProxyKind::Https,
            "https://127.0.0.1:8443",
            Some("u"),
            Some("p"),
        );
        assert!(
            build_endpoint_client(&auth).is_some(),
            "https 成对凭证 → Some"
        );
        let anon = ep(ProxyKind::Https, "https://127.0.0.1:8443", None, None);
        assert!(build_endpoint_client(&anon).is_some(), "https 匿名 → Some");
    }
}
