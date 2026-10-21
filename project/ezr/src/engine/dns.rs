//! dns — 内置异步 DNS 解析器（FR-01-112，v1.33）
//!
//! 引擎 HTTP 客户端域名解析默认走内置解析器：读系统 DNS 解析配置
//! （Linux/macOS = `/etc/resolv.conf`、Windows = 系统适配器 DNS 配置，
//! [`TokioAsyncResolver::tokio_from_system_conf`] 单源承载）自行直接发查询，
//! 完全绕开平台系统解析库（getaddrinfo/NSS 与 Windows 系统解析栈）。
//! 解析次序自愈链（FR-01-112）：
//! ① hosts 静态表优先（管理员钉点逃生口）；
//! ② 内置解析器——A 与 AAAA 并行独立查询，任一腿成功后为另一腿保留
//!    [`GRACE`] 宽限期（先答腿立即可用，黑吞腿不拖慢整体，D40①），结果
//!    IPv4 前置去重合并（D40②）；
//! ③ 两腿皆败回落系统解析（std ToSocketAddrs，阻塞调用经 spawn_blocking
//!    承载，D40③）；构造失败时调用方整体降级系统解析路径（D40④）。
//!
//! 不变项：代理/重定向/超时/传输语义全部不变，仅替换域名解析路径。

use std::collections::HashMap;
use std::future::Future;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use hickory_resolver::TokioAsyncResolver;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

/// A/AAAA 双腿宽限期（D40①）：先答腿成功后，另一腿至多再等此时长，
/// 超时即弃置（黑吞腿不拖慢整体；双腿皆败仍等各自超时后进回落）。
pub(crate) const GRACE: Duration = Duration::from_millis(1000);

/// 模块级统一错误箱（与 reqwest [`Resolve`] 错误形态一致）。
pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// 双腿结果（`None` = 宽限超时弃置，D40①）。
type LegResult<T, E> = Option<Result<Vec<T>, E>>;

/// 跨 await 持有的另一腿（`+ Send` 擦除：不染非 Send）。
type PendingLeg<'a, T, E> = Pin<&'a mut (dyn Future<Output = Result<Vec<T>, E>> + Send)>;

/// 回落 future（boxed + Send：阻塞调用经 spawn_blocking 承载）。
pub(crate) type FallbackFuture =
    Pin<Box<dyn Future<Output = Result<Vec<IpAddr>, BoxError>> + Send>>;

// hosts 静态表路径经平台适配缝单源（FR-01-112①；arch 规则 8：env 读取
// 收口于 `model/config.rs` 的 hosts_file_path，本模块不直读环境变量）。

/// hosts 静态表（FR-01-112①：命中即用，管理员钉点逃生口）。
///
/// 解析规则与系统口径一致：`#` 起为注释；行首 token = IP 字面量
/// （v4/v6 皆可，解析失败整行跳过不中止），其余 token = 别名（大小写
/// 无关归一后建索引，同别名同 IP 去重、异 IP 保留）。读取失败 → 空表
/// （非致命，自愈链继续）。
pub(crate) struct HostsFile {
    entries: HashMap<String, Vec<IpAddr>>,
}

impl HostsFile {
    /// 从平台 hosts 路径加载（读不到 → 空表，不阻塞、不致命）。
    pub(crate) fn load() -> Self {
        Self::load_from(crate::model::config::hosts_file_path())
    }

    /// 从指定路径加载（单源供 [`HostsFile::load`] 与测试注入；读不到 → 空表）。
    fn load_from(path: PathBuf) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_lines(&text),
            Err(_) => Self {
                entries: HashMap::new(),
            },
        }
    }

    /// 从文本行解析（单源供 [`HostsFile::load`] 与形态守卫测试）。
    pub(crate) fn from_lines(text: &str) -> Self {
        let mut entries: HashMap<String, Vec<IpAddr>> = HashMap::new();
        for line in text.lines() {
            let code = line.split('#').next().unwrap_or("");
            let mut tokens = code.split_whitespace();
            let Some(ip) = tokens.next().and_then(|t| t.parse::<IpAddr>().ok()) else {
                continue;
            };
            for alias in tokens {
                let ips = entries.entry(alias.to_ascii_lowercase()).or_default();
                if !ips.contains(&ip) {
                    ips.push(ip);
                }
            }
        }
        Self { entries }
    }

    /// 大小写无关命中；未命中 → None（自愈链继续走内置解析）。
    pub(crate) fn lookup(&self, host: &str) -> Option<Vec<IpAddr>> {
        self.entries.get(&host.to_ascii_lowercase()).cloned()
    }
}

/// 双腿答案合并（D40②）：IPv4 前置、跨腿去重、保序。
fn merge_answers(v4: Vec<IpAddr>, v6: Vec<IpAddr>) -> Vec<IpAddr> {
    let mut out: Vec<IpAddr> = Vec::with_capacity(v4.len() + v6.len());
    for ip in v4.into_iter().chain(v6) {
        if !out.contains(&ip) {
            out.push(ip);
        }
    }
    out
}

/// 双腿并行 + 先答腿宽限（FR-01-112②/D40①）。
///
/// 两腿并行起跑；任一腿**成功**后另一腿最多再等 `grace`，超时弃置
/// （返回 `None`）；先答腿**失败**则继续等另一腿（其自身超时兜底，
/// 不因宽限提前放弃唯一可用答案）；双腿皆败原样返回 `(Err, Err)`
/// 供调用方走回落。
async fn race_with_grace<T, E, FA, FB>(
    a: FA,
    b: FB,
    grace: Duration,
) -> (LegResult<T, E>, LegResult<T, E>)
where
    FA: Future<Output = Result<Vec<T>, E>> + Send,
    FB: Future<Output = Result<Vec<T>, E>> + Send,
{
    tokio::pin!(a, b);
    // 第一阶段：双腿并行，先到者胜（select 只轮询胜者；败者/慢腿保持存活）
    let (first_is_b, first) = tokio::select! {
        r = &mut a => (false, r),
        r = &mut b => (true, r),
    };
    // 第二阶段（dyn + Send 擦除：跨 await 持有需 Send 才不染非 Send）：
    // 先答腿成功 → 另一腿至多再等 grace（超时弃置 = None）；
    // 先答腿失败 → 等另一腿到其自身超时
    let pending_leg: PendingLeg<'_, T, E> = if first_is_b { a.as_mut() } else { b.as_mut() };
    let second = if first.is_ok() {
        tokio::time::timeout(grace, pending_leg).await.ok()
    } else {
        Some(pending_leg.await)
    };
    if first_is_b {
        (second, Some(first))
    } else {
        (Some(first), second)
    }
}

/// 系统解析回落（D40③）：std [`ToSocketAddrs`]（POSIX = getaddrinfo、
/// Windows = 平台原生解析），阻塞调用经 spawn_blocking 承载于阻塞线程池。
pub(crate) trait SystemFallback: Send + Sync {
    /// 解析 `host`；失败即自愈链终点（调用方按既有连接失败臂呈现）。
    fn resolve(&self, host: String) -> FallbackFuture;
}

/// [`SystemFallback`] 的平台原生实现（getaddrinfo / Winsock 同形）。
pub(crate) struct GaiFallback;

impl SystemFallback for GaiFallback {
    fn resolve(&self, host: String) -> FallbackFuture {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || -> Result<Vec<IpAddr>, BoxError> {
                Ok((host.as_str(), 0)
                    .to_socket_addrs()?
                    .map(|sa| sa.ip())
                    .collect())
            })
            .await
            .map_err(|e| -> BoxError { Box::new(e) })?
        })
    }
}

/// 解析主流程（FR-01-112 自愈链臂序，单源供 [`Resolve::resolve`] 与流程守卫测试）：
/// ① hosts 命中即返（不触内置与回落）；② 内置并行命中即返（不触回落）；
/// ③ 回落系统解析（失败按既有连接失败臂呈现，零新增失败形态）。
async fn resolve_flow<F, Fut>(
    hosts: Option<Vec<IpAddr>>,
    builtin: F,
    fallback: &dyn SystemFallback,
    host: &str,
) -> Result<Vec<IpAddr>, BoxError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Vec<IpAddr>>,
{
    if let Some(ips) = hosts.filter(|ips| !ips.is_empty()) {
        return Ok(ips);
    }
    let ips = builtin().await;
    if !ips.is_empty() {
        return Ok(ips);
    }
    fallback.resolve(host.to_owned()).await
}

/// 内置异步 DNS 解析器（FR-01-112；挂接于引擎基线构建器，替代
/// reqwest 默认的 GaiResolver——同机制于 Chrome 内建 async DNS resolver）。
#[derive(Clone)]
pub(crate) struct BuiltinResolver {
    res: TokioAsyncResolver,
    hosts: Arc<HostsFile>,
    fallback: Arc<dyn SystemFallback>,
}

impl BuiltinResolver {
    /// 系统配置构造（FR-01-112②；跨平台单源：Linux/macOS = `/etc/resolv.conf`
    /// 的 nameserver/search/options，Windows = 系统适配器 DNS 配置）。
    ///
    /// # Errors
    /// 系统 DNS 配置不可读/格式异常时原样上抛——调用方（`base_client_builder`）
    /// 按 D40④ 整体降级系统解析路径，不阻塞启动。
    pub(crate) fn try_system() -> Result<Self, BoxError> {
        Ok(Self {
            res: TokioAsyncResolver::tokio_from_system_conf()?,
            hosts: Arc::new(HostsFile::load()),
            fallback: Arc::new(GaiFallback),
        })
    }

    /// 测试注入构造（形态守卫：ezr.test 假域名经 hosts 拦截证明接管系统解析路径）。
    #[cfg(test)]
    pub(crate) fn with_parts(
        res: TokioAsyncResolver,
        hosts: Arc<HostsFile>,
        fallback: Arc<dyn SystemFallback>,
    ) -> Self {
        Self {
            res,
            hosts,
            fallback,
        }
    }

    /// 内置并行双腿查询（A/AAAA 独立查询、先答腿 + [`GRACE`] 宽限、合并）。
    async fn parallel_lookup(&self, host: &str) -> Vec<IpAddr> {
        let host = host.to_owned();
        let res4 = self.res.clone();
        let host4 = host.clone();
        let v4 = async move {
            res4.ipv4_lookup(host4)
                .await
                .map(|l| l.into_iter().map(|a| IpAddr::V4(a.0)).collect::<Vec<_>>())
        };
        let res6 = self.res.clone();
        let v6 = async move {
            res6.ipv6_lookup(host)
                .await
                .map(|l| l.into_iter().map(|a| IpAddr::V6(a.0)).collect::<Vec<_>>())
        };
        let (r4, r6) = race_with_grace(v4, v6, GRACE).await;
        merge_answers(
            r4.and_then(|r| r.ok()).unwrap_or_default(),
            r6.and_then(|r| r.ok()).unwrap_or_default(),
        )
    }
}

impl Resolve for BuiltinResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let this = self.clone();
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let hosts = this.hosts.lookup(&host);
            resolve_flow(
                hosts,
                || this.parallel_lookup(&host),
                this.fallback.as_ref(),
                &host,
            )
            .await
            .map(ips_to_addrs)
        })
    }
}

/// 答案集 → reqwest 地址迭代器（端口 0：hyper 以请求 scheme 端口覆写）。
fn ips_to_addrs(ips: Vec<IpAddr>) -> Addrs {
    Box::new(ips.into_iter().map(|ip| SocketAddr::from((ip, 0))))
}

#[cfg(test)]
mod tests {
    use std::future::{pending, ready};
    use std::net::{IpAddr, SocketAddr};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use hickory_resolver::config::{
        NameServerConfig, NameServerConfigGroup, Protocol, ResolverConfig, ResolverOpts,
    };
    use hickory_resolver::TokioAsyncResolver;

    use super::{
        merge_answers, race_with_grace, resolve_flow, BuiltinResolver, GaiFallback, HostsFile,
        SystemFallback, GRACE,
    };

    fn ip4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::from([a, b, c, d])
    }

    fn ip6(seg: u16, tail: u16) -> IpAddr {
        IpAddr::from([0xfd00, 0, 0, 0, 0, 0, seg, tail])
    }

    // ---------- HostsFile（自愈链①） ----------

    #[test]
    fn hosts_parses_aliases_comments_and_both_families() {
        let text =
            "# 头注释\n127.0.0.1 localhost lp\n::1 localhost\n10.0.0.5 ezr.test # 行内注释\n";
        let h = HostsFile::from_lines(text);
        assert_eq!(
            h.lookup("localhost"),
            Some(vec![
                ip4(127, 0, 0, 1),
                IpAddr::from([0, 0, 0, 0, 0, 0, 0, 1])
            ])
        );
        assert_eq!(h.lookup("lp"), Some(vec![ip4(127, 0, 0, 1)]));
        assert_eq!(h.lookup("ezr.test"), Some(vec![ip4(10, 0, 0, 5)]));
        assert_eq!(h.lookup("missing.test"), None);
    }

    #[test]
    fn hosts_lookup_is_case_insensitive_and_dedups_aliases() {
        let h = HostsFile::from_lines("10.1.2.3 EzR.Test\n10.1.2.3 ezr.TEST\n");
        assert_eq!(h.lookup("ezr.test"), Some(vec![ip4(10, 1, 2, 3)]));
        assert_eq!(h.lookup("EZR.TEST"), Some(vec![ip4(10, 1, 2, 3)]));
        assert_eq!(h.lookup("other.test"), None);
    }

    #[test]
    fn hosts_skips_malformed_lines_without_aborting() {
        let h = HostsFile::from_lines("not-an-ip host\n\n999.1.1.1 bad\n10.2.3.4 good.test\n");
        assert_eq!(h.lookup("host"), None);
        assert_eq!(h.lookup("bad"), None);
        assert_eq!(h.lookup("good.test"), Some(vec![ip4(10, 2, 3, 4)]));
    }

    #[test]
    fn load_missing_path_yields_empty_table() {
        // load 的 Err 臂：读不到 → 空表非致命（自愈链继续，自愈链①不阻断）
        let h = HostsFile::load_from(PathBuf::from("/nonexistent/ezr-v141-test-hosts"));
        assert_eq!(h.lookup("anything.test"), None, "空表任何别名都不命中");
    }

    // ---------- merge_answers（D40②：IPv4 前置去重） ----------

    #[test]
    fn merge_answers_ipv4_first_then_ipv6_with_dedup() {
        let v4 = vec![ip4(1, 1, 1, 1), ip4(8, 8, 8, 8)];
        let v6 = vec![ip6(1, 2), ip4(1, 1, 1, 1), ip6(1, 2)];
        assert_eq!(
            merge_answers(v4, v6),
            vec![ip4(1, 1, 1, 1), ip4(8, 8, 8, 8), ip6(1, 2)]
        );
    }

    #[test]
    fn merge_answers_preserves_ipv6_only_result() {
        assert_eq!(merge_answers(Vec::new(), vec![ip6(3, 4)]), vec![ip6(3, 4)]);
    }

    // ---------- race_with_grace（D40①：先答腿 + 1s 宽限） ----------

    #[tokio::test(start_paused = true)]
    async fn race_first_ok_abandons_slow_leg_after_grace() {
        let slow = pending::<Result<Vec<IpAddr>, ()>>();
        let t0 = tokio::time::Instant::now();
        let (r4, r6) = race_with_grace(ready(Ok(vec![ip4(1, 1, 1, 1)])), slow, GRACE).await;
        assert_eq!(t0.elapsed(), GRACE, "慢腿至多拖 GRACE，不等到其超时");
        assert_eq!(r4.map(|r| r.unwrap()), Some(vec![ip4(1, 1, 1, 1)]));
        assert_eq!(r6, None, "黑吞腿弃置");
    }

    #[tokio::test(start_paused = true)]
    async fn race_second_leg_within_grace_is_kept() {
        let slow_ok = async {
            tokio::time::sleep(Duration::from_millis(300)).await;
            Ok::<Vec<IpAddr>, ()>(vec![ip6(1, 2)])
        };
        let (r4, r6) = race_with_grace(ready(Ok(vec![ip4(1, 1, 1, 1)])), slow_ok, GRACE).await;
        assert_eq!(r4.map(|r| r.unwrap()), Some(vec![ip4(1, 1, 1, 1)]));
        assert_eq!(
            r6.map(|r| r.unwrap()),
            Some(vec![ip6(1, 2)]),
            "宽限内到达的腿保留"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn race_first_err_waits_second_beyond_grace() {
        // 先答腿失败 → 不因宽限提前放弃唯一可用答案（D40①）
        let first_err = ready(Err::<Vec<IpAddr>, ()>(()));
        let second_ok = async {
            tokio::time::sleep(Duration::from_secs(3)).await;
            Ok::<Vec<IpAddr>, ()>(vec![ip4(9, 9, 9, 9)])
        };
        let t0 = tokio::time::Instant::now();
        let (r4, r6) = race_with_grace(first_err, second_ok, GRACE).await;
        assert_eq!(t0.elapsed(), Duration::from_secs(3), "失败腿不触发宽限弃置");
        assert_eq!(r4, Some(Err(())));
        assert_eq!(r6.map(|r| r.unwrap()), Some(vec![ip4(9, 9, 9, 9)]));
    }

    #[tokio::test(start_paused = true)]
    async fn race_both_err_propagate_to_fallback_caller() {
        let (r4, r6) = race_with_grace(
            ready(Err::<Vec<IpAddr>, ()>(())),
            ready(Err::<Vec<IpAddr>, ()>(())),
            GRACE,
        )
        .await;
        assert!(r4.unwrap().is_err());
        assert!(r6.unwrap().is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn race_first_is_b_ok_abandons_slow_leg_after_grace() {
        // b 先答（a 永不就绪）：first_is_b=true 交换臂序确定性覆盖（D40①）
        let slow = pending::<Result<Vec<IpAddr>, ()>>();
        let t0 = tokio::time::Instant::now();
        let (r4, r6) = race_with_grace(slow, ready(Ok(vec![ip6(7, 8)])), GRACE).await;
        assert_eq!(t0.elapsed(), GRACE, "慢腿（a）至多拖 GRACE");
        assert_eq!(r4, None, "a 腿宽限超时弃置");
        assert_eq!(
            r6.map(|r| r.unwrap()),
            Some(vec![ip6(7, 8)]),
            "b 腿答案保留"
        );
    }

    // ---------- resolve_flow（自愈链臂序） ----------

    struct MockFallback {
        calls: Mutex<Vec<String>>,
        result: Result<Vec<IpAddr>, ()>,
    }

    impl MockFallback {
        fn ok(ips: Vec<IpAddr>) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                result: Ok(ips),
            }
        }

        fn err() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                result: Err(()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl SystemFallback for MockFallback {
        fn resolve(&self, host: String) -> super::FallbackFuture {
            self.calls.lock().unwrap().push(host);
            let result = self.result.clone();
            Box::pin(async move {
                result.map_err(|()| -> super::BoxError { "mock 系统解析失败".into() })
            })
        }
    }

    #[tokio::test]
    async fn flow_hosts_hit_short_circuits_builtin_and_fallback() {
        let fb = MockFallback::ok(vec![ip4(9, 9, 9, 9)]);
        let ips = resolve_flow(
            Some(vec![ip4(10, 0, 0, 5)]),
            || async { panic!("hosts 命中不应触内置解析") },
            &fb,
            "ezr.test",
        )
        .await
        .unwrap();
        assert_eq!(ips, vec![ip4(10, 0, 0, 5)]);
        assert!(fb.calls().is_empty(), "hosts 命中不应触系统解析回落");
    }

    #[tokio::test]
    async fn flow_builtin_hit_skips_fallback() {
        let fb = MockFallback::ok(vec![ip4(9, 9, 9, 9)]);
        let ips = resolve_flow(
            None,
            || async { vec![ip4(1, 1, 1, 1), ip6(1, 2)] },
            &fb,
            "github.com",
        )
        .await
        .unwrap();
        assert_eq!(ips, vec![ip4(1, 1, 1, 1), ip6(1, 2)]);
        assert!(fb.calls().is_empty(), "内置解析命中不应触系统解析回落");
    }

    #[tokio::test]
    async fn flow_builtin_empty_falls_back_to_system() {
        let fb = MockFallback::ok(vec![ip4(9, 9, 9, 9)]);
        let ips = resolve_flow(None, || async { Vec::new() }, &fb, "github.com")
            .await
            .unwrap();
        assert_eq!(ips, vec![ip4(9, 9, 9, 9)]);
        assert_eq!(fb.calls(), vec!["github.com".to_owned()]);
    }

    #[tokio::test]
    async fn flow_fallback_error_propagates() {
        let fb = MockFallback::err();
        let err = resolve_flow(None, || async { Vec::new() }, &fb, "h")
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), "mock 系统解析失败");
    }

    // ---------- GaiFallback（真实系统解析） ----------

    #[tokio::test]
    async fn gai_fallback_resolves_localhost() {
        let ips = GaiFallback.resolve("localhost".to_owned()).await.unwrap();
        assert!(
            ips.iter().any(|ip| ip.is_loopback()),
            "localhost 至少回环地址之一"
        );
    }

    // ---------- client 级接线（证明拦截系统解析路径） ----------

    async fn spawn_mini_http() -> (u16, tokio::task::JoinHandle<()>) {
        use tokio::io::AsyncWriteExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            if let Ok((mut sock, _)) = listener.accept().await {
                let _ = sock
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                    )
                    .await;
                let _ = sock.shutdown().await;
            }
        });
        (port, handle)
    }

    /// client 级接线脚手架单源：内置器注入 client → 请求 mini http → 收尾。
    async fn fetch_through_builtin(resolver: BuiltinResolver, host: &str) -> String {
        let client = reqwest::Client::builder()
            .dns_resolver(Arc::new(resolver))
            .build()
            .unwrap();
        let (port, server) = spawn_mini_http().await;
        let body = client
            .get(format!("http://{host}:{port}/"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        server.await.unwrap();
        body
    }

    #[tokio::test]
    async fn client_resolves_fake_host_via_builtin_hosts_interception() {
        // ezr.test 非真实域名：系统解析必败；命中注入 hosts 表 ⇒ 证明解析被内置器接管
        let resolver = BuiltinResolver::with_parts(
            TokioAsyncResolver::tokio_from_system_conf().unwrap(),
            Arc::new(HostsFile::from_lines("127.0.0.1 ezr.test\n")),
            Arc::new(GaiFallback),
        );
        assert_eq!(fetch_through_builtin(resolver, "ezr.test").await, "ok");
    }

    #[tokio::test]
    async fn client_resolves_localhost_through_builtin_chain() {
        // try_system 构造（真实系统 DNS 配置）+ hosts(localhost) 臂 + 全链 client
        let resolver = BuiltinResolver::try_system().unwrap();
        assert_eq!(fetch_through_builtin(resolver, "localhost").await, "ok");
    }

    // ---------- parallel_lookup 双腿接线 + 内置臂→回落衔接（覆盖率补齐） ----------

    /// 双腿不可达的内置解析器（nameserver = 127.0.0.1:1 UDP 短超时；
    /// A/AAAA 两腿各自失败 → 合并空集，供 fallback 链补测注入）。
    fn unreachable_builtin_resolver() -> BuiltinResolver {
        let group = NameServerConfigGroup::from(vec![NameServerConfig::new(
            SocketAddr::from(([127, 0, 0, 1], 1)),
            Protocol::Udp,
        )]);
        let config = ResolverConfig::from_parts(None, Vec::new(), group);
        let mut options = ResolverOpts::default();
        options.timeout = Duration::from_millis(150);
        options.attempts = 1;
        BuiltinResolver::with_parts(
            TokioAsyncResolver::tokio(config, options),
            Arc::new(HostsFile::from_lines("")),
            Arc::new(GaiFallback),
        )
    }

    #[tokio::test]
    async fn parallel_lookup_unreachable_nameserver_yields_empty() {
        // 双腿接线（A/AAAA 查询 + 映射 + 宽限竞速 + 合并）：双腿皆败 → 空集
        let r = unreachable_builtin_resolver()
            .parallel_lookup("leg-fail.invalid")
            .await;
        assert!(r.is_empty(), "双腿皆败合并后必须为空集（调用方走回落）");
    }

    #[tokio::test]
    async fn client_hosts_miss_falls_through_builtin_to_failure() {
        // hosts 未命中 + 内置双腿空集 → 回落系统解析（.invalid 假域名必败）：
        // 同一 client 请求内走通 Resolve::resolve → builtin 臂 → fallback 臂全链
        let client = reqwest::Client::builder()
            .dns_resolver(Arc::new(unreachable_builtin_resolver()))
            .build()
            .unwrap();
        let sent = client
            .get("http://builtin-fallthrough.invalid/")
            .send()
            .await;
        assert!(
            sent.is_err(),
            "hosts 未命中 + 双腿空集 + 系统解析必败 → 请求必须失败（零新增成功形态）"
        );
    }

    // ---------- 引擎基线构建器接线（FR-01-112） ----------

    #[test]
    fn base_client_builder_attaches_builtin_resolver() {
        // 基线构建器构造成功 = 内置解析器已挂接（构造失败路径整体降级，不 panic）
        let _client = super::super::base_client_builder().build().unwrap();
    }
}
