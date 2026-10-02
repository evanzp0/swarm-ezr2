//! ezr-proxy — 本地 HTTP 代理 fixture（QA 基建，AC-8 / qa/01-throttle-proxy）
//!
//! 能力：
//! - 普通 http 请求（绝对 URI 形态）：转发并回传，逐条记录经手请求
//! - CONNECT 隧道（HTTPS）：转发字节流，逐条记录隧道目标
//! - 双代理区分记录：各实例 `--name` 标识 + 独立 `--log` 日志
//!
//! 用法：`ezr-proxy serve --port 8766 --name proxy-a [--log proxy-a.jsonl]`
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

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

/// 访问日志（JSONL）
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

/// 解析已读头部 → 连接目标 → 原样转发头 → 双向拷贝
///
/// `head_bytes` = accept 分发线程已从流中消费的完整请求头
/// （请求行 + 头部 + 空行）；分发线程需先读首行判定 CONNECT 与否，
/// 故头部必须自此传入，严禁再从流上重复读取（重复读取将永久阻塞）。
fn handle_http(mut client: TcpStream, log: Arc<ProxyLog>, head_bytes: Vec<u8>) {
    let head = String::from_utf8_lossy(&head_bytes).into_owned();
    let Some((request_line, headers)) = parse_head(&head) else {
        return;
    };
    let Some(target) = request_target(&request_line) else {
        return;
    };
    log.log("http", &target);
    let Some((hostport, path)) = split_abs_uri(&target) else {
        let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n");
        return;
    };
    let (hostname, port) = split_host_port(&hostport, 80);
    let Ok(mut upstream) = TcpStream::connect((hostname.as_str(), port)) else {
        let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n");
        return;
    };
    let fwd = build_forward(&path, &headers);
    if upstream.write_all(fwd.as_bytes()).is_err() {
        return;
    }
    // 双向拷贝至一方关闭（头部已被分发线程消费，客户端侧直接用原始流拷贝）
    let mut cl2 = client.try_clone().expect("clone 失败");
    let mut up2 = upstream.try_clone().expect("clone 失败");
    let a = std::thread::spawn(move || {
        let mut up = upstream;
        let _ = std::io::copy(&mut up, &mut client);
    });
    let b = std::thread::spawn(move || {
        let _ = std::io::copy(&mut cl2, &mut up2);
    });
    let _ = a.join();
    let _ = b.join();
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

/// 处理 CONNECT 隧道（HTTPS）：`target` = 请求行第二段（host:port）
fn handle_connect(mut client: TcpStream, target: String, log: Arc<ProxyLog>) {
    log.log("connect", &target);
    let (hostname, port) = split_host_port(&target, 443);
    let Ok(upstream) = TcpStream::connect((hostname.as_str(), port)) else {
        let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n");
        return;
    };
    let _ = client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n");
    let mut up2 = upstream.try_clone().expect("clone 失败");
    let mut cl2 = client.try_clone().expect("clone 失败");
    let a = std::thread::spawn(move || {
        let mut up = upstream;
        let _ = std::io::copy(&mut up, &mut cl2);
    });
    let b = std::thread::spawn(move || {
        let _ = std::io::copy(&mut client, &mut up2);
    });
    let _ = a.join();
    let _ = b.join();
}

/// accept 后逐连接分派：CONNECT 隧道 / 普通 http
///
/// 分发线程先读首行判定 CONNECT 与否；普通 http 在此续读头部到空行
/// 组装完整头（严禁从流上重复读取——重复读取将永久阻塞）。
fn dispatch_conn(s: TcpStream, log: Arc<ProxyLog>) {
    let mut reader = BufReader::new(s.try_clone().expect("clone 失败"));
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.is_empty() {
        return;
    }
    let upper = line.to_uppercase();
    if upper.starts_with("CONNECT") {
        // 取请求行第二段（host:port）作为隧道目标
        let Some(target) = request_target(&line) else {
            return;
        };
        drop(reader);
        handle_connect(s, target, log);
    } else {
        // 普通 http：首行已读（判定非 CONNECT），续读头部到空行后交 handle_http
        let full = collect_head(&mut reader, &line);
        drop(reader);
        handle_http(s, log, full);
    }
}

/// 从首行之后续读头部到空行（或流结束），返回完整请求头字节（首行 + 头部 + 空行）
fn collect_head(reader: &mut BufReader<TcpStream>, first_line: &str) -> Vec<u8> {
    let mut full = first_line.as_bytes().to_vec();
    loop {
        let mut h = String::new();
        match reader.read_line(&mut h) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                full.extend_from_slice(h.as_bytes());
                if h.trim().is_empty() {
                    break;
                }
            }
        }
    }
    full
}

/// CLI 参数（默认值：port 8766 / name "proxy"）
struct ProxyArgs {
    cmd: String,
    port: u16,
    name: String,
    log_path: Option<String>,
}

/// 解析 `serve --port <n> --name <名> [--log <路径>]`（未知参数忽略）
fn parse_proxy_args(args: &[String]) -> ProxyArgs {
    let mut out = ProxyArgs {
        cmd: String::new(),
        port: 8766,
        name: String::from("proxy"),
        log_path: None,
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
            _ => {}
        }
        i += 1;
    }
    out
}

/// accept 循环：逐连接孵化分派线程（accept 错误记录后继续，不失去服务能力）
fn serve(listener: TcpListener, log: Arc<ProxyLog>) {
    for conn in listener.incoming() {
        match conn {
            Ok(s) => {
                let log = Arc::clone(&log);
                std::thread::spawn(move || dispatch_conn(s, log));
            }
            Err(e) => {
                eprintln!("ezr-proxy: accept 错误（{e}）");
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let a = parse_proxy_args(&args);
    if a.cmd != "serve" {
        eprintln!("用法: ezr-proxy serve --port 8766 --name proxy-a [--log proxy-a.jsonl]");
        std::process::exit(2);
    }
    let listener = TcpListener::bind(("127.0.0.1", a.port)).expect("绑定失败");
    println!("ezr-proxy [{}] listening on 127.0.0.1:{}", a.name, a.port);
    let log = Arc::new(ProxyLog::open(a.log_path.as_deref(), &a.name));
    serve(listener, log);
}

#[cfg(test)]
mod proxy_tests {
    use std::io::{Read, Write};

    use super::*;

    fn log_quiet() -> Arc<ProxyLog> {
        Arc::new(ProxyLog::open(None, "t"))
    }

    /// 本地桩上游：accept 一个连接，读一次后回固定响应，返回收到的请求文本
    fn stub_upstream(response: &'static [u8]) -> (u16, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
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

    /// 配置读超时 → 发请求 → 关写侧（让代理 client→upstream 拷贝得到 EOF）→ 读到 EOF
    fn send_and_read(mut client: TcpStream, req: String) -> Vec<u8> {
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        client.write_all(req.as_bytes()).unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let mut resp = Vec::new();
        let _ = client.read_to_end(&mut resp);
        resp
    }

    #[test]
    fn dispatch_http_forwards_rewrites_and_relays() {
        let (up_port, upstream) =
            stub_upstream(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello");
        let (client, server) = tcp_pair();
        let log = log_quiet();
        let h = std::thread::spawn(move || dispatch_conn(server, log));
        // 关写侧：代理的 client→upstream 拷贝得到 EOF 后才能收尾
        let resp = send_and_read(
            client,
            format!("GET http://127.0.0.1:{up_port}/f.bin HTTP/1.1\r\nHost: x\r\n\r\n"),
        );
        let _ = h.join();
        let req = upstream.join().unwrap();
        assert!(
            req.starts_with("GET /f.bin HTTP/1.1\r\n"),
            "重写为路径形式: {req}"
        );
        assert!(req.contains("host: x"));
        let resp = String::from_utf8_lossy(&resp);
        assert!(resp.contains("200 OK"));
        assert!(resp.ends_with("hello"));
    }

    #[test]
    fn dispatch_connect_establishes_and_tunnels() {
        let (up_port, upstream) = stub_upstream(b"tunnel-ack");
        let (client, server) = tcp_pair();
        let log = log_quiet();
        let h = std::thread::spawn(move || dispatch_conn(server, log));
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
        let _ = h.join();
        let req = upstream.join().unwrap();
        // 桩单次 read 收到的就是隧道首笔数据（原始字节转发）
        assert!(req.starts_with("ping"), "tunnel payload: {req:?}");
        assert_eq!(String::from_utf8_lossy(&rest), "tunnel-ack");
    }

    #[test]
    fn dispatch_non_http_target_is_400() {
        let (client, server) = tcp_pair();
        let log = log_quiet();
        let h = std::thread::spawn(move || dispatch_conn(server, log));
        let resp = send_and_read(client, "GET ftp://h/x HTTP/1.1\r\n\r\n".to_string());
        let _ = h.join();
        assert!(String::from_utf8_lossy(&resp).contains("400 Bad Request"));
    }

    #[test]
    fn dispatch_unreachable_upstream_is_502() {
        let (client, server) = tcp_pair();
        let log = log_quiet();
        let h = std::thread::spawn(move || dispatch_conn(server, log));
        let resp = send_and_read(
            client,
            "GET http://127.0.0.1:1/x HTTP/1.1\r\n\r\n".to_string(),
        );
        let _ = h.join();
        assert!(String::from_utf8_lossy(&resp).contains("502 Bad Gateway"));
    }

    #[test]
    fn serve_loop_handles_multiple_conns() {
        let (up_port, upstream) = stub_upstream(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let log = log_quiet();
        std::thread::spawn(move || serve(listener, log));
        let mut c1 = TcpStream::connect(addr).unwrap();
        c1.write_all(format!("GET http://127.0.0.1:{up_port}/1 HTTP/1.1\r\n\r\n").as_bytes())
            .unwrap();
        c1.shutdown(std::net::Shutdown::Write).unwrap();
        let mut buf = String::new();
        let _ = c1.read_to_string(&mut buf);
        assert!(buf.contains("200 OK"));
        let _ = upstream.join();
    }

    /// 建立一对已连接的回环 TCP 流（模拟客户端 ↔ 代理入口）
    fn tcp_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let c = TcpStream::connect(addr).unwrap();
        let (s, _) = listener.accept().unwrap();
        (c, s)
    }

    #[test]
    fn parse_proxy_args_defaults_and_overrides() {
        let a = parse_proxy_args(&[]);
        assert_eq!(a.cmd, "");
        assert_eq!(a.port, 8766);
        assert_eq!(a.name, "proxy");
        assert!(a.log_path.is_none());

        let a = parse_proxy_args(&[
            "ezr-proxy".to_string(),
            "serve".to_string(),
            "--port".to_string(),
            "9000".to_string(),
            "--name".to_string(),
            "p-b".to_string(),
            "--log".to_string(),
            "x.jsonl".to_string(),
        ]);
        assert_eq!(a.cmd, "serve");
        assert_eq!(a.port, 9000);
        assert_eq!(a.name, "p-b");
        assert_eq!(a.log_path.as_deref(), Some("x.jsonl"));

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
