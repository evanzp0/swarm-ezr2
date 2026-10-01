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
#![allow(clippy::cognitive_complexity, clippy::too_many_lines, clippy::too_many_arguments)]
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_wrap)]


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
            let line =
                format!("{{\"ts\":{ts},\"proxy\":\"{}\",\"kind\":\"{kind}\",\"target\":\"{target}\"}}\n", self.name);
            let _ = f.lock().unwrap().write_all(line.as_bytes());
        }
        println!("[{}] {kind} {target}", self.name);
    }
}

/// 处理普通 http 请求（绝对 URI）：解析已读头部 → 连接目标 → 原样转发头 → 双向拷贝
///
/// `head_bytes` = accept 分发线程已从流中消费的完整请求头
/// （请求行 + 头部 + 空行）；分发线程需先读首行判定 CONNECT 与否，
/// 故头部必须自此传入，严禁再从流上重复读取（重复读取将永久阻塞）。
fn handle_http(mut client: TcpStream, log: Arc<ProxyLog>, head_bytes: Vec<u8>) {
    let head = String::from_utf8_lossy(&head_bytes).into_owned();
    let mut lines = head.lines();
    let Some(request_line) =
        lines.next().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string)
    else {
        return;
    };
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
    // 请求行第二段 = 绝对 URI
    let Some(target) = request_line.split_whitespace().nth(1).map(str::to_string) else { return };
    log.log("http", &target);
    let Some(rest) = target.strip_prefix("http://") else {
        let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n");
        return;
    };
    let (host, path) = match rest.split_once('/') {
        Some((h, p)) => (h.to_string(), format!("/{p}")),
        None => (rest.to_string(), "/".to_string()),
    };
    let port = host.split_once(':').map(|(_, p)| p.parse::<u16>().unwrap_or(80)).unwrap_or(80);
    let hostname = host.split(':').next().unwrap_or(&host).to_string();
    let Ok(mut upstream) = TcpStream::connect((hostname.as_str(), port)) else {
        let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n");
        return;
    };
    // 重写请求行（绝对 URI → 路径）+ 透传头
    let mut fwd = format!("GET {path} HTTP/1.1\r\n");
    for (k, v) in &headers {
        if k != "proxy-connection" && k != "proxy-authorization" {
            fwd.push_str(&format!("{k}: {v}\r\n"));
        }
    }
    fwd.push_str("Connection: close\r\n\r\n");
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

/// 处理 CONNECT 隧道（HTTPS）：`target` = 请求行第二段（host:port）
fn handle_connect(mut client: TcpStream, target: String, log: Arc<ProxyLog>) {
    log.log("connect", &target);
    let (hostname, port) = match target.split_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(443)),
        None => (target.to_string(), 443),
    };
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut cmd = String::new();
    let mut port: u16 = 8766;
    let mut name = String::from("proxy");
    let mut log_path = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "serve" => cmd = args[i].clone(),
            "--port" => {
                i += 1;
                port = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(port);
            }
            "--name" => {
                i += 1;
                name = args.get(i).cloned().unwrap_or(name);
            }
            "--log" => {
                i += 1;
                log_path = args.get(i).cloned();
            }
            _ => {}
        }
        i += 1;
    }
    if cmd != "serve" {
        eprintln!("用法: ezr-proxy serve --port 8766 --name proxy-a [--log proxy-a.jsonl]");
        std::process::exit(2);
    }
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("绑定失败");
    println!("ezr-proxy [{name}] listening on 127.0.0.1:{port}");
    let log = Arc::new(ProxyLog::open(log_path.as_deref(), &name));
    for conn in listener.incoming() {
        match conn {
            Ok(s) => {
                let log = Arc::clone(&log);
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(s.try_clone().expect("clone 失败"));
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() || line.is_empty() {
                        return;
                    }
                    let upper = line.to_uppercase();
                    if upper.starts_with("CONNECT") {
                        // 取请求行第二段（host:port）作为隧道目标
                        let Some(target) =
                            line.split_whitespace().nth(1).map(str::to_string)
                        else {
                            return;
                        };
                        drop(reader);
                        handle_connect(s, target, log);
                    } else {
                        // 普通 http：首行已读（判定非 CONNECT），续读头部到空行，
                        // 组装完整头交 handle_http 解析（不得从流上重复读取）
                        let mut full = line.clone().into_bytes();
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
                        drop(reader);
                        handle_http(s, log, full);
                    }
                });
            }
            Err(e) => {
                eprintln!("ezr-proxy: accept 错误（{e}）");
            }
        }
    }
}
