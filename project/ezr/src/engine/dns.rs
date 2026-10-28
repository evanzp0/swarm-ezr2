use std::collections::HashMap;
use std::future::Future;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use hickory_resolver::TokioAsyncResolver;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

pub(crate) const GRACE: Duration = Duration::from_millis(1000);

pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;

type LegResult<T, E> = Option<Result<Vec<T>, E>>;

type PendingLeg<'a, T, E> = Pin<&'a mut (dyn Future<Output = Result<Vec<T>, E>> + Send)>;

pub(crate) type FallbackFuture =
    Pin<Box<dyn Future<Output = Result<Vec<IpAddr>, BoxError>> + Send>>;

pub(crate) struct HostsFile {
    entries: HashMap<String, Vec<IpAddr>>,
}

impl HostsFile {
    pub(crate) fn load() -> Self {
        Self::load_from(crate::model::config::hosts_file_path())
    }

    fn load_from(path: PathBuf) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_lines(&text),
            Err(_) => Self {
                entries: HashMap::new(),
            },
        }
    }

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

    pub(crate) fn lookup(&self, host: &str) -> Option<Vec<IpAddr>> {
        self.entries.get(&host.to_ascii_lowercase()).cloned()
    }
}

pub(crate) fn merge_answers(v4: Vec<IpAddr>, v6: Vec<IpAddr>) -> Vec<IpAddr> {
    let mut out: Vec<IpAddr> = Vec::with_capacity(v4.len() + v6.len());
    for ip in v4.into_iter().chain(v6) {
        if !out.contains(&ip) {
            out.push(ip);
        }
    }
    out
}

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
    let (first_is_b, first) = tokio::select! {
        r = &mut a => (false, r),
        r = &mut b => (true, r),
    };
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

pub(crate) trait SystemFallback: Send + Sync {
    fn resolve(&self, host: String) -> FallbackFuture;
}

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

#[derive(Clone)]
pub(crate) struct BuiltinResolver {
    res: TokioAsyncResolver,
    hosts: Arc<HostsFile>,
    fallback: Arc<dyn SystemFallback>,
}

impl BuiltinResolver {
    pub(crate) fn try_system() -> Result<Self, BoxError> {
        Ok(Self {
            res: TokioAsyncResolver::tokio_from_system_conf()?,
            hosts: Arc::new(HostsFile::load()),
            fallback: Arc::new(GaiFallback),
        })
    }

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
        let h = HostsFile::load_from(PathBuf::from("/nonexistent/ezr-v141-test-hosts"));
        assert_eq!(h.lookup("anything.test"), None, "空表任何别名都不命中");
    }

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

    #[tokio::test]
    async fn gai_fallback_resolves_localhost() {
        let ips = GaiFallback.resolve("localhost".to_owned()).await.unwrap();
        assert!(
            ips.iter().any(|ip| ip.is_loopback()),
            "localhost 至少回环地址之一"
        );
    }

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
        let resolver = BuiltinResolver::with_parts(
            TokioAsyncResolver::tokio_from_system_conf().unwrap(),
            Arc::new(HostsFile::from_lines("127.0.0.1 ezr.test\n")),
            Arc::new(GaiFallback),
        );
        assert_eq!(fetch_through_builtin(resolver, "ezr.test").await, "ok");
    }

    #[tokio::test]
    async fn client_resolves_localhost_through_builtin_chain() {
        let resolver = BuiltinResolver::try_system().unwrap();
        assert_eq!(fetch_through_builtin(resolver, "localhost").await, "ok");
    }

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
        let r = unreachable_builtin_resolver()
            .parallel_lookup("leg-fail.invalid")
            .await;
        assert!(r.is_empty(), "双腿皆败合并后必须为空集（调用方走回落）");
    }

    #[tokio::test]
    async fn client_hosts_miss_falls_through_builtin_to_failure() {
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

    #[test]
    fn base_client_builder_attaches_builtin_resolver() {
        let _client = super::super::base_client_builder().build().unwrap();
    }
}
