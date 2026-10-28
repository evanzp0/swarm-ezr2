mod dns;
mod error;
mod supervisor;
mod throttle;

use std::collections::HashMap;
use std::sync::Arc;

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use dns::{merge_answers, HostsFile};
#[cfg(test)]
#[allow(unused_imports)]
pub use error::classify_reqwest;
#[cfg(test)]
#[allow(unused_imports)]
pub use error::parse_retry_after_header;
pub use supervisor::{BtSpec, TaskSpec, VerifySpec};
#[cfg(test)]
#[allow(unused_imports)]
pub use throttle::Throttle;
#[cfg(test)]
#[allow(unused_imports)]
pub use throttle::{bucket_step, BURST_SECS, SLEEP_MS};
use tokio::sync::mpsc;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Cmd {
    Start {
        spec: supervisor::TaskSpec,
    },
    Pause {
        id: u32,
    },
    Cancel {
        id: u32,
    },
    Verify {
        spec: supervisor::VerifySpec,
    },
    Reconfigure {
        id: u32,
        concurrency: usize,
        endpoint: Option<crate::model::ProxyEndpoint>,
    },
    StartBt {
        spec: Box<supervisor::BtSpec>,
    },
    PauseBt {
        id: u32,
    },
    CancelBt {
        id: u32,
    },
    StopSeed {
        id: u32,
    },
    BtDisconnect {
        id: u32,
        conn: usize,
    },
    Shutdown,
}

#[derive(Debug)]
pub enum Evt {
    Probed {
        id: u32,
        name: String,
        total: u64,
        resumable: bool,
        etag: Option<String>,
        last_modified: Option<String>,
    },
    Progress {
        id: u32,
        downloaded: u64,
        conns: Vec<ConnView>,
        chunk_done: u32,
        done_blocks: Vec<u32>,
        partial_blocks: Vec<u32>,
    },
    PausedDone {
        id: u32,
        downloaded: u64,
        chunk_done: u32,
        partial_blocks: Vec<u32>,
    },
    Cancelled {
        id: u32,
    },
    Failed {
        id: u32,
        kind: crate::model::FailKind,
        reason: String,
        retry_after: Option<f64>,
        made_progress: bool,
        downloaded: u64,
        chunk_done: u32,
        partial_blocks: Vec<u32>,
    },
    Invalidated {
        id: u32,
    },
    DownloadDone {
        id: u32,
        total: u64,
    },
    VerifyDone {
        id: u32,
        ok: bool,
        computed: String,
        expected: String,
    },
    Toast(String),
    BtPeers {
        id: u32,
        complete: u32,
        incomplete: u32,
    },
    BtConns {
        id: u32,
        conns: Vec<BtConnView>,
    },
    BtProgress {
        id: u32,
        downloaded: u64,
        chunk_done: u32,
        done_blocks: Vec<u32>,
        partial_blocks: Vec<u32>,
        uploaded: u64,
        upload_speed: f64,
    },
    BtDone {
        id: u32,
        total: u64,
    },
    BtStopped {
        id: u32,
        downloaded: u64,
        done_pieces: Vec<u32>,
        partial_pieces: Vec<u32>,
    },
    BtFailed {
        id: u32,
        kind: crate::model::FailKind,
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConnView {
    pub id: usize,
    pub block: u32,
    pub start: u64,
    pub end: u64,
    pub done: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BtConnView {
    pub id: usize,
    pub addr: String,
    pub down_speed: f64,
    pub cum_down: u64,
    pub up_speed: f64,
    pub cum_up: u64,
    pub client: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskCmd {
    Pause,
    Cancel,
    Reconfigure {
        concurrency: usize,
        endpoint: Option<crate::model::ProxyEndpoint>,
    },
}

pub(crate) struct EngineShared {
    pub client: reqwest::Client,
    clients: std::sync::Mutex<HashMap<String, reqwest::Client>>,
    throttle: Arc<throttle::Throttle>,
}

impl EngineShared {
    pub(crate) fn bt_tracker_client(
        &self,
        ep: Option<&crate::model::ProxyEndpoint>,
    ) -> reqwest::Client {
        self.client_for(ep)
    }

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

fn proxy_fingerprint(ep: &crate::model::ProxyEndpoint) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}",
        ep.url,
        ep.username.as_deref().unwrap_or(""),
        ep.password.as_deref().unwrap_or("")
    )
}

pub(crate) fn endpoint_valid(ep: &crate::model::ProxyEndpoint) -> bool {
    reqwest::Proxy::all(&ep.url).is_ok()
}

fn socks5_auth_url(base: &str, user: &str, pass: &str) -> Option<String> {
    let mut u = url::Url::parse(base).ok()?;
    if u.scheme() != "socks5" {
        return None;
    }
    u.set_username(user).ok()?;
    u.set_password(Some(pass)).ok()?;
    Some(u.to_string())
}

fn build_endpoint_client(ep: &crate::model::ProxyEndpoint) -> Option<reqwest::Client> {
    let px = match ep.kind {
        crate::model::config::ProxyKind::Socks5 => {
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

fn base_client_builder() -> reqwest::ClientBuilder {
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .no_proxy()
        .connect_timeout(std::time::Duration::from_secs(15))
        .read_timeout(std::time::Duration::from_secs(30));
    if let Ok(resolver) = dns::BuiltinResolver::try_system() {
        builder = builder.dns_resolver(Arc::new(resolver));
    }
    builder
}

#[derive(Clone)]
pub struct EngineHandle {
    cmd_tx: mpsc::Sender<Cmd>,
}

impl EngineHandle {
    pub fn start(cfg: &crate::model::config::Config, evt_tx: mpsc::Sender<Evt>) -> EngineHandle {
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

    pub async fn send(&self, cmd: Cmd) {
        let _ = self.cmd_tx.send(cmd).await;
    }
}

async fn main_loop(
    shared: Arc<EngineShared>,
    mut cmd_rx: mpsc::Receiver<Cmd>,
    evt_tx: mpsc::Sender<Evt>,
) {
    let mut tasks: HashMap<u32, mpsc::Sender<TaskCmd>> = HashMap::new();
    let mut bt_tasks: HashMap<u32, mpsc::Sender<crate::bt::BtTaskCmd>> = HashMap::new();
    while let Some(cmd) = cmd_rx.recv().await {
        if handle_cmd(&mut tasks, &mut bt_tasks, cmd, &shared, &evt_tx).await {
            break;
        }
    }
}

async fn handle_cmd(
    tasks: &mut HashMap<u32, mpsc::Sender<TaskCmd>>,
    bt_tasks: &mut HashMap<u32, mpsc::Sender<crate::bt::BtTaskCmd>>,
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
        Cmd::StartBt { spec } => {
            start_bt_task(bt_tasks, *spec, shared.clone(), evt_tx.clone());
            false
        }
        Cmd::PauseBt { id } => bt_send(bt_tasks, id, crate::bt::BtTaskCmd::Pause),
        Cmd::CancelBt { id } => bt_send(bt_tasks, id, crate::bt::BtTaskCmd::Cancel),
        Cmd::StopSeed { id } => bt_send(bt_tasks, id, crate::bt::BtTaskCmd::StopSeed),
        Cmd::BtDisconnect { id, conn } => {
            bt_send(bt_tasks, id, crate::bt::BtTaskCmd::Disconnect(conn));
            false
        }
        Cmd::Shutdown => {
            shutdown_all(tasks).await;
            shutdown_all_bt(bt_tasks).await;
            true
        }
    }
}

fn start_bt_task(
    bt_tasks: &mut HashMap<u32, mpsc::Sender<crate::bt::BtTaskCmd>>,
    spec: supervisor::BtSpec,
    shared: Arc<EngineShared>,
    evt_tx: mpsc::Sender<Evt>,
) {
    let (tx, rx) = mpsc::channel::<crate::bt::BtTaskCmd>(8);
    bt_tasks.insert(spec.id, tx);
    tokio::spawn(crate::bt::engine::run(spec, shared, rx, evt_tx));
}

fn bt_send(
    bt_tasks: &mut HashMap<u32, mpsc::Sender<crate::bt::BtTaskCmd>>,
    id: u32,
    cmd: crate::bt::BtTaskCmd,
) -> bool {
    if let Some(tx) = bt_tasks.get(&id) {
        let _ = tx.try_send(cmd);
    }
    false
}

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

async fn pause_task(tasks: &HashMap<u32, mpsc::Sender<TaskCmd>>, id: u32) {
    if let Some(h) = tasks.get(&id) {
        let _ = h.send(TaskCmd::Pause).await;
    }
}

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

fn spawn_verify(spec: supervisor::VerifySpec, evt_tx: &mpsc::Sender<Evt>) {
    let tx = evt_tx.clone();
    tokio::spawn(async move {
        supervisor::verify(spec, tx).await;
    });
}

async fn shutdown_all(tasks: &mut HashMap<u32, mpsc::Sender<TaskCmd>>) {
    for (_, h) in tasks.drain() {
        let _ = h.send(TaskCmd::Cancel).await;
    }
}

async fn shutdown_all_bt(bt_tasks: &mut HashMap<u32, mpsc::Sender<crate::bt::BtTaskCmd>>) {
    for (_, h) in bt_tasks.drain() {
        let _ = h.send(crate::bt::BtTaskCmd::Cancel).await;
    }
}

#[cfg(test)]
pub(crate) fn shared_for_tests() -> Arc<EngineShared> {
    let client = base_client_builder()
        .build()
        .expect("测试 HTTP 客户端构建失败");
    Arc::new(EngineShared {
        client,
        clients: std::sync::Mutex::new(HashMap::new()),
        throttle: Arc::new(throttle::Throttle::new(0)),
    })
}

#[cfg(test)]
mod main_loop_tests {
    use super::*;

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

    #[tokio::test]
    async fn reconfigure_unknown_id_is_noop() {
        let mut tasks: HashMap<u32, mpsc::Sender<TaskCmd>> = HashMap::new();
        let mut bt_tasks: HashMap<u32, mpsc::Sender<crate::bt::BtTaskCmd>> = HashMap::new();
        let shared = Arc::new(shared_free());
        let (tx, _rx) = mpsc::channel::<Evt>(8);
        let exited = handle_cmd(
            &mut tasks,
            &mut bt_tasks,
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

    #[tokio::test]
    async fn reconfigure_invalid_endpoint_falls_back_to_direct() {
        let mut tasks: HashMap<u32, mpsc::Sender<TaskCmd>> = HashMap::new();
        let mut bt_tasks: HashMap<u32, mpsc::Sender<crate::bt::BtTaskCmd>> = HashMap::new();
        let shared = Arc::new(shared_free());
        let (tx, mut rx) = mpsc::channel::<Evt>(8);
        let (ttx, mut trx) = mpsc::channel::<TaskCmd>(8);
        tasks.insert(7u32, ttx);
        let exited = handle_cmd(
            &mut tasks,
            &mut bt_tasks,
            Cmd::Reconfigure {
                id: 7,
                concurrency: 2,
                endpoint: Some(crate::model::ProxyEndpoint {
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
        drop(rx);
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
        assert!(evt_rx.try_recv().is_err());
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn pause_forwards_only_to_live_handle() {
        let mut tasks = HashMap::new();
        let (h, mut rx) = mpsc::channel::<TaskCmd>(1);
        tasks.insert(9u32, h);
        pause_task(&tasks, 9).await;
        assert_eq!(rx.recv().await, Some(TaskCmd::Pause));
        pause_task(&tasks, 404).await;
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
        let mut bt_tasks: HashMap<u32, mpsc::Sender<crate::bt::BtTaskCmd>> = HashMap::new();

        assert!(
            !handle_cmd(
                &mut tasks,
                &mut bt_tasks,
                Cmd::Start {
                    spec: spec_bogus(11)
                },
                &shared,
                &evt_tx
            )
            .await
        );
        assert!(tasks.contains_key(&11));

        assert!(
            !handle_cmd(
                &mut tasks,
                &mut bt_tasks,
                Cmd::Pause { id: 11 },
                &shared,
                &evt_tx
            )
            .await
        );
        assert!(
            !handle_cmd(
                &mut tasks,
                &mut bt_tasks,
                Cmd::Cancel { id: 999 },
                &shared,
                &evt_tx
            )
            .await
        );
        assert!(matches!(evt_rx.recv().await, Some(Evt::Cancelled { id }) if id == 999));
        let vspec = supervisor::VerifySpec {
            id: 11,
            path: "/nonexistent/a.downloading".to_string(),
            final_path: "/nonexistent/a.bin".to_string(),
            sidecar_path: "/nonexistent/a.sidecar".to_string(),
            algo: "MD5",
            expected: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
        };
        assert!(
            !handle_cmd(
                &mut tasks,
                &mut bt_tasks,
                Cmd::Verify { spec: vspec },
                &shared,
                &evt_tx
            )
            .await
        );

        assert!(handle_cmd(&mut tasks, &mut bt_tasks, Cmd::Shutdown, &shared, &evt_tx).await);
        assert!(tasks.is_empty());
    }
}

#[cfg(test)]
mod proxy_auth_tests {
    use super::*;
    use crate::model::config::ProxyKind;

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

    #[test]
    fn socks5_auth_url_unicode_and_space_roundtrip() {
        let out =
            socks5_auth_url("socks5://h:1", "用 户", "密 码").expect("非 ASCII 凭证应构造成功");
        let u = url::Url::parse(&out).expect("认证 url 应可解析");
        assert_eq!(u.host_str(), Some("h"));
        assert_eq!(pct_decode(u.username()), "用 户");
        assert_eq!(pct_decode(u.password().unwrap_or_default()), "密 码");
    }

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

    #[test]
    fn build_client_socks5_without_credentials_defensive() {
        let e = ep(ProxyKind::Socks5, "socks5://127.0.0.1:1080", None, None);
        assert!(
            build_endpoint_client(&e).is_some(),
            "socks5 无凭证防御 → Some"
        );
    }

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
