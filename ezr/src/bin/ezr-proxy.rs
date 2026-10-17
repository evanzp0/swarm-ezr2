//! ezr-proxy — 本地代理 fixture（QA 基建，AC-8 / qa/01-throttle-proxy、qa/02-named-proxy）
//!
//! 能力（v140 扩展后）：
//! - scheme=http（缺省）：http 匿名代理（绝对 URI 转发 + CONNECT 隧道），逐条记录
//! - scheme=http/https + `--user/--password`：认证强制模式——无/错凭证回 407 质询
//!   （`Proxy-Authenticate: Basic`），正确凭证放行；日志记认证事件（ok/denied）但
//!   **不落凭证明文**（NP-04 判据：全日志无 username/password）
//! - scheme=https：代理自身 TLS 监听（v1.9/FR-01-89 的 https 型 = 代理走 TLS 的
//!   HTTP 代理）。证书为内嵌自签（SAN：IP 127.0.0.1 / IP ::1 / DNS localhost，
//!   有效期至 2126）；客户端信任链：reqwest `rustls-tls-native-roots` 经
//!   rustls-native-certs 0.8（openssl-probe）读取 `SSL_CERT_FILE` 环境变量，
//!   套件以 `ca.pem` 注入，产品零改动
//! - scheme=socks5：SOCKS5 + RFC 1929 user/pass 认证（凭证必填，与规格 FR-01-93
//!   「socks5 不支持匿名」口径一致；错凭证回 0x01 失败并记 denied 事件）
//! - `--bind`：监听地址（缺省 127.0.0.1；NP-18 用 `::1` IPv6 回环）
//!
//! 用法：`ezr-proxy serve --port 8766 --name proxy-a [--log proxy-a.jsonl]
//!       [--scheme http|https|socks5] [--user U --password W] [--bind ADDR]`
//!
//! 日志 JSONL 字段口径不变：`{"ts","proxy","kind","target"}`（套件 NP-13 锁四键）；
//! kind：http / connect / socks5 / http-auth-ok / http-auth-denied /
//! socks5-auth-ok / socks5-auth-denied。
#![allow(missing_docs)] // 交互层：demo 定稿基线复用，接口文档见 model/engine 层
#![allow(clippy::pedantic)] // 交互层字节/速度展示算术与 demo 基线风格豁免
#![allow(clippy::nursery)] // 同上
#![allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::too_many_arguments
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use std::io::Write as _;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// 内嵌自签证书（DER）：SAN = IP 127.0.0.1 / IP ::1 / DNS localhost。
/// PEM 形态 `ca.pem`（同目录）供套件以 SSL_CERT_FILE 注入产品进程。
const TLS_CERT_DER: &[u8] = include_bytes!("ezr-proxy-certs/server.der");
const TLS_KEY_DER: &[u8] = include_bytes!("ezr-proxy-certs/server-key.der");

/// 访问日志（JSONL）；认证事件不含凭证明文
struct ProxyLog {
    file: Option<std::sync::Mutex<std::fs::File>>,
    name: String,
}

impl ProxyLog {
    fn open(path: Option<&str>, name: &str) -> ProxyLog {
        ProxyLog {
            file: path.map(|p| {
                std::sync::Mutex::new(
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(p)
                        .expect("打开日志失败"),
                )
            }),
            name: name.to_string(),
        }
    }

    fn log(&self, kind: &str, target: &str) {
        if let Some(f) = &self.file {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let line = format!(
                "{{\"ts\":{ts},\"proxy\":\"{}\",\"kind\":\"{kind}\",\"target\":\"{target}\"}}\n",
                self.name
            );
            let _ = f.lock().unwrap().write_all(line.as_bytes());
        }
        println!("[{}] {kind} {target}", self.name);
    }
}

/// host:port 拆分（无端口用默认值；端口解析失败也用默认值）
fn split_host_port(hostport: &str, default_port: u16) -> (String, u16) {
    match hostport.split_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(default_port)),
        None => (hostport.to_string(), default_port),
    }
}

/// 由请求行提取目标（第二段）；无第二段 → None
fn request_target(request_line: &str) -> Option<String> {
    request_line.split_whitespace().nth(1).map(str::to_string)
}

/// 绝对 URI → (hostport, path)；非 http:// → None（调用方回 400）
fn split_abs_uri(target: &str) -> Option<(String, String)> {
    let rest = target.strip_prefix("http://")?;
    Some(match rest.split_once('/') {
        Some((h, p)) => (h.to_string(), format!("/{p}")),
        None => (rest.to_string(), "/".to_string()),
    })
}

/// 组装转发请求（重写请求行为 `GET {path}` + 透传头（剔除代理专用头）+ Connection: close）
fn build_forward(path: &str, headers: &[(String, String)]) -> String {
    let mut fwd = format!("GET {path} HTTP/1.1\r\n");
    for (k, v) in headers {
        if k != "proxy-connection" && k != "proxy-authorization" {
            fwd.push_str(&format!("{k}: {v}\r\n"));
        }
    }
    fwd.push_str("Connection: close\r\n\r\n");
    fwd
}

/// 极简 base64 编码（标准字母表 + padding；仅用于构造期望的 Basic 头值
/// 与客户端头做字符串比对，无需解码依赖）
fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// 认证要求（`--user/--password` 齐备时启用；http/https 生效）
type Auth = Option<(String, String)>;

/// 期望的 `Proxy-Authorization` 头值（`Basic b64(user:pass)`）
fn expected_basic(auth: &Auth) -> Option<String> {
    auth.as_ref()
        .map(|(u, p)| format!("Basic {}", b64(format!("{u}:{p}").as_bytes())))
}

/// 校验请求头中的代理凭证；`expected` 为 None（匿名模式）恒通过。
/// 通过 → Ok；失败 → 回 407 + `Proxy-Authenticate`（写失败仅静默，连接即弃）。
async fn check_http_auth<S: AsyncWrite + Unpin>(
    s: &mut S,
    headers: &[(String, String)],
    expected: &Auth,
    log: &ProxyLog,
) -> bool {
    let Some(want) = expected_basic(expected) else {
        return true;
    };
    let got = headers
        .iter()
        .find(|(k, _)| k == "proxy-authorization")
        .map(|(_, v)| v.clone());
    if got.as_deref() == Some(want.as_str()) {
        log.log("http-auth-ok", "authenticated");
        true
    } else {
        log.log("http-auth-denied", "challenge-407");
        let body = "HTTP/1.1 407 Proxy Authentication Required\r\n\
                    Proxy-Authenticate: Basic realm=\"ezr-proxy\"\r\n\
                    Content-Length: 0\r\n\r\n";
        let _ = s.write_all(body.as_bytes()).await;
        let _ = s.flush().await;
        false
    }
}

/// 监听形态（scheme × 认证）
enum Mode {
    /// http 匿名/认证（`--scheme http` 缺省）
    PlainHttp(Auth),
    /// https：先 TLS 握手再按 http 处理（`--scheme https`）
    TlsHttp(Auth),
    /// socks5 + RFC 1929 认证（凭证必填，main 已校验）
    Socks5 { user: String, pass: String },
}

/// TLS acceptor（内嵌证书；进程默认 ring provider 单一存在）
fn tls_acceptor() -> tokio_rustls::TlsAcceptor {
    let cert = rustls::pki_types::CertificateDer::from(TLS_CERT_DER.to_vec());
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(TLS_KEY_DER.to_vec());
    let cfg = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key.into())
        .expect("内嵌证书/私钥非法");
    tokio_rustls::TlsAcceptor::from(Arc::new(cfg))
}

/// 已读头部 → 连接目标 → 原样转发头 → 双向拷贝
///
/// `head_bytes` = 分派层已从流中消费的完整请求头（请求行 + 头部 + 空行）；
/// 分派层需先读首部判定 CONNECT 与否，故头部必须自此传入，严禁再从流上
/// 重复读取（重复读取将永久阻塞）。
async fn handle_http<S>(mut client: S, log: Arc<ProxyLog>, head_bytes: Vec<u8>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let head = String::from_utf8_lossy(&head_bytes).into_owned();
    let Some((request_line, headers)) = parse_head(&head) else {
        return;
    };
    let Some(target) = request_target(&request_line) else {
        return;
    };
    log.log("http", &target);
    let Some((hostport, path)) = split_abs_uri(&target) else {
        let _ = client
            .write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n")
            .await;
        return;
    };
    let (hostname, port) = split_host_port(&hostport, 80);
    let Ok(mut upstream) = TcpStream::connect((hostname.as_str(), port)).await else {
        let _ = client
            .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
            .await;
        return;
    };
    let fwd = build_forward(&path, &headers);
    if upstream.write_all(fwd.as_bytes()).await.is_err() {
        return;
    }
    // 双向拷贝：客户端头部后余量（GET 无 body）⇄ 上游响应；上游按注入的
    // Connection: close 关闭 ⇒ 上游向 EOF 收敛，套件生命周期内自然结束
    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
}

/// 处理 CONNECT 隧道（HTTPS 目标）：`target` = 请求行第二段（host:port）
async fn handle_connect<S>(mut client: S, target: String, log: Arc<ProxyLog>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    log.log("connect", &target);
    let (hostname, port) = split_host_port(&target, 443);
    let Ok(mut upstream) = TcpStream::connect((hostname.as_str(), port)).await else {
        let _ = client
            .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
            .await;
        return;
    };
    let _ = client
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await;
    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
}

/// 读请求头到空行（\r\n\r\n；上限 64KB）；EOF/超限 → None（头部非空且有内容
/// 时按已收头部返回，与旧 collect_head 的 EOF 容错口径一致）
async fn read_head<S: AsyncRead + Unpin>(s: &mut S) -> Option<Vec<u8>> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let n = s.read(&mut chunk).await.ok()?;
        if n == 0 {
            return if buf.is_empty() { None } else { Some(buf) };
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.ends_with(b"\r\n\r\n") || buf.ends_with(b"\n\n") {
            return Some(buf);
        }
        if buf.len() > 64 * 1024 {
            return None;
        }
    }
}

/// 已解析头部：（请求行，小写键头部列表）
type HeadParts = (String, Vec<(String, String)>);

/// 解析已消费的头部文本 →（请求行，小写键头部列表）；空头部 → None
fn parse_head(head: &str) -> Option<HeadParts> {
    let mut lines = head.lines();
    let request_line = lines
        .next()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)?;
    let mut headers = Vec::new();
    for l in lines {
        let t = l.trim();
        if t.is_empty() {
            break;
        }
        if let Some((k, v)) = t.split_once(':') {
            headers.push((k.trim().to_lowercase(), v.trim().to_string()));
        }
    }
    Some((request_line, headers))
}

/// accept 后逐连接分派：CONNECT 隧道 / 普通 http（TLS 与否由调用方先行处理）
async fn dispatch_http<S>(mut stream: S, auth: &Auth, log: Arc<ProxyLog>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let Some(head_bytes) = read_head(&mut stream).await else {
        return;
    };
    let head = String::from_utf8_lossy(&head_bytes).into_owned();
    let Some((request_line, headers)) = parse_head(&head) else {
        return;
    };
    let Some(target) = request_target(&request_line) else {
        return;
    };
    if !check_http_auth(&mut stream, &headers, auth, &log).await {
        return;
    }
    if request_line.to_uppercase().starts_with("CONNECT") {
        dispatch_connect(stream, target, log).await;
    } else {
        handle_http(stream, log, head_bytes).await;
    }
}

/// CONNECT 分派（认证通过后；独立函数便于日志/复用口径清晰）
async fn dispatch_connect<S>(stream: S, target: String, log: Arc<ProxyLog>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    handle_connect(stream, target, log).await;
}

/// SOCKS5 服务（RFC 1928 + RFC 1929 user/pass；认证必填）
async fn handle_socks5<S>(mut s: S, user: &str, pass: &str, log: Arc<ProxyLog>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // 1) 方法协商：仅接受 0x02（user/pass）；探活连接（连后即断）在此自然退出
    let mut hdr = [0u8; 2];
    if s.read_exact(&mut hdr).await.is_err() {
        return;
    }
    if hdr[0] != 5 || hdr[1] == 0 {
        return;
    }
    let mut methods = vec![0u8; usize::from(hdr[1])];
    if s.read_exact(&mut methods).await.is_err() {
        return;
    }
    if !methods.contains(&0x02) {
        log.log("socks5-auth-denied", "no-userpass-method");
        let _ = s.write_all(&[5, 0xFF]).await;
        return;
    }
    if s.write_all(&[5, 0x02]).await.is_err() {
        return;
    }
    // 2) RFC 1929 user/pass 子协商（ver(1) ulen(1) uname plen(1) passwd）；
    // 凭证比对失败 → 0x01 + 断开（不落明文）
    let mut ver = [0u8; 1];
    if s.read_exact(&mut ver).await.is_err() || ver[0] != 1 {
        return;
    }
    let mut ulen = [0u8; 1];
    if s.read_exact(&mut ulen).await.is_err() {
        return;
    }
    let mut uname = vec![0u8; usize::from(ulen[0])];
    if s.read_exact(&mut uname).await.is_err() {
        return;
    }
    let mut plen = [0u8; 1];
    if s.read_exact(&mut plen).await.is_err() {
        return;
    }
    let mut passwd = vec![0u8; usize::from(plen[0])];
    if s.read_exact(&mut passwd).await.is_err() {
        return;
    }
    if uname != user.as_bytes() || passwd != pass.as_bytes() {
        log.log("socks5-auth-denied", "bad-credentials");
        let _ = s.write_all(&[1, 0x01]).await;
        return;
    }
    if s.write_all(&[1, 0x00]).await.is_err() {
        return;
    }
    log.log("socks5-auth-ok", "authenticated");
    // 3) 请求：仅 CONNECT（cmd=1）
    let mut pre = [0u8; 3];
    if s.read_exact(&mut pre).await.is_err() {
        return;
    }
    if pre[0] != 5 || pre[1] != 0x01 {
        let _ = s.write_all(&[5, 0x07, 0, 1, 0, 0, 0, 0, 0, 0]).await;
        return;
    }
    let mut atyp = [0u8; 1];
    if s.read_exact(&mut atyp).await.is_err() {
        return;
    }
    let host = match atyp[0] {
        0x01 => {
            let mut o = [0u8; 4];
            if s.read_exact(&mut o).await.is_err() {
                return;
            }
            format!("{}.{}.{}.{}", o[0], o[1], o[2], o[3])
        }
        0x03 => {
            let mut l = [0u8; 1];
            if s.read_exact(&mut l).await.is_err() {
                return;
            }
            let mut d = vec![0u8; usize::from(l[0])];
            if s.read_exact(&mut d).await.is_err() {
                return;
            }
            String::from_utf8_lossy(&d).into_owned()
        }
        0x04 => {
            let mut o = [0u8; 16];
            if s.read_exact(&mut o).await.is_err() {
                return;
            }
            std::net::Ipv6Addr::new(
                u16::from_be_bytes([o[0], o[1]]),
                u16::from_be_bytes([o[2], o[3]]),
                u16::from_be_bytes([o[4], o[5]]),
                u16::from_be_bytes([o[6], o[7]]),
                u16::from_be_bytes([o[8], o[9]]),
                u16::from_be_bytes([o[10], o[11]]),
                u16::from_be_bytes([o[12], o[13]]),
                u16::from_be_bytes([o[14], o[15]]),
            )
            .to_string()
        }
        _ => {
            let _ = s.write_all(&[5, 0x08, 0, 1, 0, 0, 0, 0, 0, 0]).await;
            return;
        }
    };
    let mut pbuf = [0u8; 2];
    if s.read_exact(&mut pbuf).await.is_err() {
        return;
    }
    let port = u16::from_be_bytes(pbuf);
    let target = format!("{host}:{port}");
    log.log("socks5", &target);
    let Ok(mut upstream) = TcpStream::connect((host.as_str(), port)).await else {
        let _ = s.write_all(&[5, 0x05, 0, 1, 0, 0, 0, 0, 0, 0]).await;
        return;
    };
    // 回复：ver5 rep0 rsv0 atyp1(IPv4) bind=0.0.0.0:0
    let _ = s.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await;
    let _ = tokio::io::copy_bidirectional(&mut s, &mut upstream).await;
}

/// accept 循环：逐连接孵化分派任务（accept 错误记录后继续，不失去服务能力）
async fn serve(listener: TcpListener, mode: Mode, log: Arc<ProxyLog>) {
    loop {
        match listener.accept().await {
            Ok((s, _)) => {
                let log = Arc::clone(&log);
                let mode = match &mode {
                    Mode::PlainHttp(a) => Mode::PlainHttp(a.clone()),
                    Mode::TlsHttp(a) => Mode::TlsHttp(a.clone()),
                    Mode::Socks5 { user, pass } => Mode::Socks5 {
                        user: user.clone(),
                        pass: pass.clone(),
                    },
                };
                tokio::spawn(async move {
                    dispatch_conn(s, mode, log).await;
                });
            }
            Err(e) => {
                eprintln!("ezr-proxy: accept 错误（{e}）");
            }
        }
    }
}

/// 单连接分派：按形态接 TLS / 走 http / 走 socks5
///
/// TLS 握手失败（探活连接连后即断等）静默丢弃——不属服务错误。
async fn dispatch_conn(s: TcpStream, mode: Mode, log: Arc<ProxyLog>) {
    match mode {
        Mode::PlainHttp(auth) => dispatch_http(s, &auth, log).await,
        Mode::TlsHttp(auth) => {
            let acceptor = tls_acceptor();
            // 探活/主动关闭握手失败：静默（套件 start_proxy 以裸 TCP 连接探活）
            if let Ok(tls) = acceptor.accept(s).await {
                dispatch_http(tls, &auth, log).await;
            }
        }
        Mode::Socks5 { user, pass } => handle_socks5(s, &user, &pass, log).await,
    }
}

/// CLI 参数（默认值：port 8766 / name "proxy" / scheme http / bind 127.0.0.1）
struct ProxyArgs {
    cmd: String,
    port: u16,
    name: String,
    log_path: Option<String>,
    scheme: String,
    user: Option<String>,
    password: Option<String>,
    bind: String,
}

/// 解析 `serve --port <n> --name <名> [--log <路径>] [--scheme <http|https|socks5>]
/// [--user <U>] [--password <W>] [--bind <ADDR>]`（未知参数忽略）
fn parse_proxy_args(args: &[String]) -> ProxyArgs {
    let mut out = ProxyArgs {
        cmd: String::new(),
        port: 8766,
        name: String::from("proxy"),
        log_path: None,
        scheme: String::from("http"),
        user: None,
        password: None,
        bind: String::from("127.0.0.1"),
    };
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "serve" => out.cmd = args[i].clone(),
            "--port" => {
                i += 1;
                out.port = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(out.port);
            }
            "--name" => {
                i += 1;
                out.name = args.get(i).cloned().unwrap_or_else(|| out.name.clone());
            }
            "--log" => {
                i += 1;
                out.log_path = args.get(i).cloned();
            }
            "--scheme" => {
                i += 1;
                out.scheme = args.get(i).cloned().unwrap_or_else(|| out.scheme.clone());
            }
            "--user" => {
                i += 1;
                out.user = args.get(i).cloned();
            }
            "--password" => {
                i += 1;
                out.password = args.get(i).cloned();
            }
            "--bind" => {
                i += 1;
                out.bind = args.get(i).cloned().unwrap_or_else(|| out.bind.clone());
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// accept 循环入口：按参数装配监听形态（同步函数，供 main 与测试复用）
async fn run_proxy(a: ProxyArgs) {
    let listener = TcpListener::bind((a.bind.as_str(), a.port))
        .await
        .expect("绑定失败");
    println!(
        "ezr-proxy [{}] listening on {}:{} ({})",
        a.name, a.bind, a.port, a.scheme
    );
    let log = Arc::new(ProxyLog::open(a.log_path.as_deref(), &a.name));
    let mode = match a.scheme.as_str() {
        "socks5" => {
            // socks5 凭证必填（FR-01-93 口径）；main 已校验，此处防御性回退匿名禁用
            Mode::Socks5 {
                user: a.user.unwrap_or_default(),
                pass: a.password.unwrap_or_default(),
            }
        }
        "https" => Mode::TlsHttp(auth_pair(a.user, a.password)),
        _ => Mode::PlainHttp(auth_pair(a.user, a.password)),
    };
    serve(listener, mode, log).await;
}

/// 认证对：user/password 齐备才启用（单侧缺失视作配置不全 → 匿名）
fn auth_pair(user: Option<String>, password: Option<String>) -> Auth {
    match (user, password) {
        (Some(u), Some(p)) => Some((u, p)),
        _ => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let a = parse_proxy_args(&args);
    if a.cmd != "serve" {
        eprintln!(
            "用法: ezr-proxy serve --port 8766 --name proxy-a [--log proxy-a.jsonl] \
             [--scheme http|https|socks5] [--user U --password W] [--bind ADDR]"
        );
        std::process::exit(2);
    }
    // socks5 凭证必填校验（FR-01-93：不支持匿名 socks5；缺任一 → 拒绝启动）
    if a.scheme == "socks5" && (a.user.is_none() || a.password.is_none()) {
        eprintln!("ezr-proxy: socks5 监听需 --user 与 --password（不支持匿名 socks5）");
        std::process::exit(2);
    }
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    rt.block_on(run_proxy(a));
}

#[cfg(test)]
mod proxy_tests {
    use std::io::{Read, Write};

    use super::*;

    fn log_quiet() -> Arc<ProxyLog> {
        Arc::new(ProxyLog::open(None, "t"))
    }

    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        // multi_thread：测试客户端侧用阻塞 std I/O，不得饿死分派任务
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(f)
    }

    /// std 流 → tokio 流（新版 tokio 要求注册前显式转非阻塞）
    fn to_tokio(s: std::net::TcpStream) -> tokio::net::TcpStream {
        s.set_nonblocking(true).unwrap();
        tokio::net::TcpStream::from_std(s).unwrap()
    }

    /// 本地桩上游：accept 一个连接，读一次后回固定响应，返回收到的请求文本
    fn stub_upstream(response: &'static [u8]) -> (u16, std::thread::JoinHandle<String>) {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let h = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).into_owned();
            let _ = s.write_all(response);
            let _ = s.flush();
            req
        });
        (port, h)
    }

    #[test]
    fn split_host_port_defaults_and_parse() {
        assert_eq!(split_host_port("h.example", 80), ("h.example".into(), 80));
        assert_eq!(
            split_host_port("h.example:8080", 80),
            ("h.example".into(), 8080)
        );
        assert_eq!(
            split_host_port("h.example:bad", 443),
            ("h.example".into(), 443)
        );
    }

    #[test]
    fn request_target_second_token() {
        assert_eq!(
            request_target("GET http://a/b HTTP/1.1"),
            Some("http://a/b".to_string())
        );
        assert_eq!(request_target("GET"), None);
        assert_eq!(request_target(""), None);
    }

    #[test]
    fn split_abs_uri_path_and_root() {
        assert_eq!(
            split_abs_uri("http://h:8080/a/b?q=1"),
            Some(("h:8080".to_string(), "/a/b?q=1".to_string()))
        );
        assert_eq!(
            split_abs_uri("http://h"),
            Some(("h".to_string(), "/".to_string()))
        );
        assert_eq!(split_abs_uri("https://h/x"), None);
    }

    #[test]
    fn build_forward_filters_proxy_headers() {
        let headers = vec![
            ("host".to_string(), "h".to_string()),
            ("proxy-connection".to_string(), "keep-alive".to_string()),
            ("Proxy-Authorization".to_string(), "Basic x".to_string()),
            ("accept".to_string(), "*/*".to_string()),
        ];
        let fwd = build_forward("/f.bin", &headers);
        assert!(fwd.starts_with("GET /f.bin HTTP/1.1\r\n"));
        assert!(fwd.contains("host: h\r\n"));
        assert!(fwd.contains("accept: */*\r\n"));
        assert!(!fwd.contains("proxy-connection"));
        assert!(!fwd.contains("proxy-authorization"));
        assert!(fwd.ends_with("Connection: close\r\n\r\n"));
    }

    #[test]
    fn parse_head_request_line_and_headers() {
        let (rl, hs) = parse_head("GET http://h/x HTTP/1.1\r\nHost: H\r\n\r\n").unwrap();
        assert_eq!(rl, "GET http://h/x HTTP/1.1");
        assert_eq!(hs, vec![("host".to_string(), "H".to_string())]);
        assert!(parse_head("\r\n").is_none());
        assert!(parse_head("").is_none());
    }

    #[test]
    fn b64_rfc4648_vectors_and_basic_pair() {
        assert_eq!(b64(b""), "");
        assert_eq!(b64(b"f"), "Zg==");
        assert_eq!(b64(b"fo"), "Zm8=");
        assert_eq!(b64(b"foo"), "Zm9v");
        assert_eq!(b64(b"foob"), "Zm9vYg==");
        assert_eq!(b64(b"fooba"), "Zm9vYmE=");
        assert_eq!(b64(b"foobar"), "Zm9vYmFy");
        assert_eq!(
            expected_basic(&Some(("bob".into(), "pw".into()))).unwrap(),
            format!("Basic {}", b64(b"bob:pw"))
        );
        assert_eq!(expected_basic(&None), None);
    }

    /// 建立一对已连接的回环 TCP 流（模拟客户端 ↔ 代理入口）
    fn tcp_pair() -> (std::net::TcpStream, std::net::TcpStream) {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let c = std::net::TcpStream::connect(addr).unwrap();
        let (s, _) = listener.accept().unwrap();
        (c, s)
    }

    #[test]
    fn dispatch_http_forwards_rewrites_and_relays() {
        block_on(async {
            let (up_port, upstream) =
                stub_upstream(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello");
            let (client, server) = tcp_pair();
            let log = log_quiet();
            let server = to_tokio(server);
            let no_auth: Auth = None;
            let h = tokio::spawn(async move { dispatch_http(server, &no_auth, log).await });
            // 关写侧：代理的 client→upstream 拷贝得到 EOF 后才能收尾
            let mut client = client;
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            client
                .write_all(
                    format!("GET http://127.0.0.1:{up_port}/f.bin HTTP/1.1\r\nHost: x\r\n\r\n")
                        .as_bytes(),
                )
                .unwrap();
            client.shutdown(std::net::Shutdown::Write).unwrap();
            let mut resp = Vec::new();
            let _ = client.read_to_end(&mut resp);
            let _ = h.await;
            let req = upstream.join().unwrap();
            assert!(
                req.starts_with("GET /f.bin HTTP/1.1\r\n"),
                "重写为路径形式: {req}"
            );
            assert!(req.contains("host: x"));
            let resp = String::from_utf8_lossy(&resp);
            assert!(resp.contains("200 OK"));
            assert!(resp.ends_with("hello"));
        });
    }

    #[test]
    fn dispatch_connect_establishes_and_tunnels() {
        block_on(async {
            let (up_port, upstream) = stub_upstream(b"tunnel-ack");
            let (client, server) = tcp_pair();
            let log = log_quiet();
            let server = to_tokio(server);
            let no_auth: Auth = None;
            let h = tokio::spawn(async move { dispatch_http(server, &no_auth, log).await });
            let mut client = client;
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            client
                .write_all(format!("CONNECT 127.0.0.1:{up_port} HTTP/1.1\r\n\r\n").as_bytes())
                .unwrap();
            // 读到 Established 头之后，经隧道发一笔数据让桩完成其单次 read
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") {
                let n = client.read(&mut byte).unwrap();
                assert!(n > 0, "连接在 Established 前被关闭");
                head.extend_from_slice(&byte[..n]);
            }
            assert!(String::from_utf8_lossy(&head).contains("200 Connection Established"));
            client.write_all(b"ping").unwrap();
            client.shutdown(std::net::Shutdown::Write).unwrap();
            let mut rest = Vec::new();
            let _ = client.read_to_end(&mut rest);
            let _ = h.await;
            let req = upstream.join().unwrap();
            // 桩单次 read 收到的就是隧道首笔数据（原始字节转发）
            assert!(req.starts_with("ping"), "tunnel payload: {req:?}");
            assert_eq!(String::from_utf8_lossy(&rest), "tunnel-ack");
        });
    }

    #[test]
    fn dispatch_non_http_target_is_400() {
        block_on(async {
            let (client, server) = tcp_pair();
            let log = log_quiet();
            let server = to_tokio(server);
            let no_auth: Auth = None;
            let h = tokio::spawn(async move { dispatch_http(server, &no_auth, log).await });
            let mut client = client;
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            client.write_all(b"GET ftp://h/x HTTP/1.1\r\n\r\n").unwrap();
            client.shutdown(std::net::Shutdown::Write).unwrap();
            let mut resp = Vec::new();
            let _ = client.read_to_end(&mut resp);
            let _ = h.await;
            assert!(String::from_utf8_lossy(&resp).contains("400 Bad Request"));
        });
    }

    #[test]
    fn dispatch_unreachable_upstream_is_502() {
        block_on(async {
            let (client, server) = tcp_pair();
            let log = log_quiet();
            let server = to_tokio(server);
            let no_auth: Auth = None;
            let h = tokio::spawn(async move { dispatch_http(server, &no_auth, log).await });
            let mut client = client;
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            client
                .write_all(b"GET http://127.0.0.1:1/x HTTP/1.1\r\n\r\n")
                .unwrap();
            client.shutdown(std::net::Shutdown::Write).unwrap();
            let mut resp = Vec::new();
            let _ = client.read_to_end(&mut resp);
            let _ = h.await;
            assert!(String::from_utf8_lossy(&resp).contains("502 Bad Gateway"));
        });
    }

    #[test]
    fn http_auth_mode_challenges_missing_and_wrong_credentials() {
        block_on(async {
            let (up_port, upstream) =
                stub_upstream(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
            let auth = Some(("bob".to_string(), "pw-secret".to_string()));
            // ① 无凭证 → 407 + Proxy-Authenticate
            let (c1, s1) = tcp_pair();
            let s1 = to_tokio(s1);
            let log = log_quiet();
            let value = auth.clone();
            let h1 = tokio::spawn(async move { dispatch_http(s1, &value, log).await });
            let mut c1 = c1;
            c1.set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            c1.write_all(format!("GET http://127.0.0.1:{up_port}/f HTTP/1.1\r\n\r\n").as_bytes())
                .unwrap();
            let mut resp1 = String::new();
            let _ = c1.read_to_string(&mut resp1);
            let _ = h1.await;
            assert!(resp1.contains("407"), "无凭证应 407: {resp1}");
            assert!(resp1.contains("Proxy-Authenticate: Basic"), "应有质询头");
            // ② 错凭证 → 407
            let (c2, s2) = tcp_pair();
            let s2 = to_tokio(s2);
            let log = log_quiet();
            let value = auth.clone();
            let h2 = tokio::spawn(async move { dispatch_http(s2, &value, log).await });
            let mut c2 = c2;
            c2.set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let wrong = format!(
                "GET http://127.0.0.1:{up_port}/f HTTP/1.1\r\n\
                 Proxy-Authorization: Basic {}\r\n\r\n",
                b64(b"bob:wrong")
            );
            c2.write_all(wrong.as_bytes()).unwrap();
            let mut resp2 = String::new();
            let _ = c2.read_to_string(&mut resp2);
            let _ = h2.await;
            assert!(resp2.contains("407"), "错凭证应 407: {resp2}");
            // ③ 正凭证 → 放行（200 经上游回传）
            let (c3, s3) = tcp_pair();
            let s3 = to_tokio(s3);
            let log = log_quiet();
            let value = auth.clone();
            let h3 = tokio::spawn(async move { dispatch_http(s3, &value, log).await });
            let mut c3 = c3;
            c3.set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let right = format!(
                "GET http://127.0.0.1:{up_port}/f HTTP/1.1\r\n\
                 Proxy-Authorization: Basic {}\r\n\r\n",
                b64(b"bob:pw-secret")
            );
            c3.write_all(right.as_bytes()).unwrap();
            c3.shutdown(std::net::Shutdown::Write).unwrap();
            let mut resp3 = Vec::new();
            let _ = c3.read_to_end(&mut resp3);
            let _ = h3.await;
            let _ = upstream.join();
            assert!(
                String::from_utf8_lossy(&resp3).contains("200 OK"),
                "正凭证放行"
            );
        });
    }

    /// SOCKS5 桩客户端：走完整握手，返回 (auth 状态, 隧道后收到的上游数据)
    #[test]
    fn socks5_handshake_auth_and_tunnel() {
        block_on(async {
            let (up_port, upstream) = stub_upstream(b"socks-ok");
            let (client, server) = tcp_pair();
            let server = to_tokio(server);
            let log = log_quiet();
            let h = tokio::spawn(async move { handle_socks5(server, "u5", "p5", log).await });
            let mut client = client;
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            // greeting: 要求 user/pass 方法
            client.write_all(&[5, 1, 0x02]).unwrap();
            let mut m = [0u8; 2];
            client.read_exact(&mut m).unwrap();
            assert_eq!(&m, &[5, 0x02], "应选定 user/pass 方法");
            // RFC 1929 正确凭证：ver(1) ulen(2) uname("u5") plen(2) passwd("p5")
            client
                .write_all(&[1, 2, b'u', b'5', 2, b'p', b'5'])
                .unwrap();
            let mut a = [0u8; 2];
            client.read_exact(&mut a).unwrap();
            assert_eq!(&a, &[1, 0], "认证应成功");
            // CONNECT 127.0.0.1:up_port
            client.write_all(&[5, 1, 0, 1, 127, 0, 0, 1]).unwrap();
            client.write_all(&up_port.to_be_bytes()).unwrap();
            let mut r = [0u8; 10];
            client.read_exact(&mut r).unwrap();
            assert_eq!(&r[..3], &[5, 0, 0], "CONNECT 应成功");
            client.write_all(b"ping").unwrap();
            client.shutdown(std::net::Shutdown::Write).unwrap();
            let mut rest = Vec::new();
            let _ = client.read_to_end(&mut rest);
            let _ = h.await;
            assert_eq!(upstream.join().unwrap().as_str(), "ping");
            assert_eq!(String::from_utf8_lossy(&rest), "socks-ok");
        });
    }

    #[test]
    fn socks5_bad_credentials_rejected_with_0x01() {
        block_on(async {
            let (client, server) = tcp_pair();
            let server = to_tokio(server);
            let log = log_quiet();
            let h = tokio::spawn(async move { handle_socks5(server, "u5", "p5", log).await });
            let mut client = client;
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            client.write_all(&[5, 1, 0x02]).unwrap();
            let mut m = [0u8; 2];
            client.read_exact(&mut m).unwrap();
            // 错凭证：uname="ux" passwd="yz"（非 u5/p5）
            client
                .write_all(&[1, 2, b'u', b'x', 2, b'y', b'z'])
                .unwrap();
            let mut a = [0u8; 2];
            client.read_exact(&mut a).unwrap();
            assert_eq!(&a, &[1, 0x01], "错凭证应 0x01 失败");
            let _ = h.await;
        });
    }

    #[test]
    fn serve_loop_handles_multiple_conns() {
        block_on(async {
            let (up_port, upstream) =
                stub_upstream(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
                .await
                .unwrap();
            let addr = listener.local_addr().unwrap();
            let log = log_quiet();
            tokio::spawn(async move { serve(listener, Mode::PlainHttp(None), log).await });
            let mut c1 = tokio::net::TcpStream::connect(addr).await.unwrap();
            c1.write_all(format!("GET http://127.0.0.1:{up_port}/1 HTTP/1.1\r\n\r\n").as_bytes())
                .await
                .unwrap();
            c1.shutdown().await.unwrap();
            let mut buf = Vec::new();
            let _ = c1.read_to_end(&mut buf).await;
            assert!(String::from_utf8_lossy(&buf).contains("200 OK"));
            let _ = upstream.join();
        });
    }

    #[test]
    fn parse_proxy_args_defaults_and_overrides() {
        let a = parse_proxy_args(&[]);
        assert_eq!(a.cmd, "");
        assert_eq!(a.port, 8766);
        assert_eq!(a.name, "proxy");
        assert!(a.log_path.is_none());
        assert_eq!(a.scheme, "http");
        assert_eq!(a.bind, "127.0.0.1");
        assert!(a.user.is_none() && a.password.is_none());

        let a = parse_proxy_args(&[
            "ezr-proxy".to_string(),
            "serve".to_string(),
            "--port".to_string(),
            "9000".to_string(),
            "--name".to_string(),
            "p-b".to_string(),
            "--log".to_string(),
            "x.jsonl".to_string(),
            "--scheme".to_string(),
            "socks5".to_string(),
            "--user".to_string(),
            "u".to_string(),
            "--password".to_string(),
            "w".to_string(),
            "--bind".to_string(),
            "::1".to_string(),
        ]);
        assert_eq!(a.cmd, "serve");
        assert_eq!(a.port, 9000);
        assert_eq!(a.name, "p-b");
        assert_eq!(a.log_path.as_deref(), Some("x.jsonl"));
        assert_eq!(a.scheme, "socks5");
        assert_eq!(a.user.as_deref(), Some("u"));
        assert_eq!(a.password.as_deref(), Some("w"));
        assert_eq!(a.bind, "::1");

        // 非法 port 沿用默认；--name 缺值保持原名；未知参数忽略
        let a = parse_proxy_args(&[
            "ezr-proxy".into(),
            "--port".into(),
            "zz".into(),
            "--name".into(),
        ]);
        assert_eq!(a.port, 8766);
        assert_eq!(a.name, "proxy");
    }
}
