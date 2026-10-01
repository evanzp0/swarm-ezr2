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


use std::io::{BufRead, BufReader, Read, Write};
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

/// 请求行 + 头列表
type ReqHead = (String, Vec<(String, String)>);

/// 读取并解析请求头块
fn read_head(stream: &mut BufReader<TcpStream>) -> std::io::Result<Option<ReqHead>> {
    let mut line = String::new();
    if stream.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let request_line = line.trim().to_string();
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        if stream.read_line(&mut h)? == 0 {
            break;
        }
        let t = h.trim();
        if t.is_empty() {
            break;
        }
        if let Some((k, v)) = t.split_once(':') {
            headers.push((k.trim().to_lowercase(), v.trim().to_string()));
        }
    }
    Ok(Some((request_line, headers)))
}

/// 处理普通 http 请求（绝对 URI）：读完整头 → 连接目标 → 原样转发头 → 双向拷贝
fn handle_http(mut client: TcpStream, log: Arc<ProxyLog>, header_extra: Vec<u8>) {
    let mut reader = BufReader::new(client.try_clone().expect("clone 失败"));
    let Ok(Some((request_line, headers))) = read_head(&mut reader) else { return };
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
    // 客户端可能已发的 body 片段（fixture 场景基本为 GET，忽略）
    let _ = header_extra;
    // 双向拷贝至一方关闭
    let mut up2 = upstream.try_clone().expect("clone 失败");
    let a = std::thread::spawn(move || {
        let mut up = upstream;
        let _ = std::io::copy(&mut up, &mut client);
    });
    let b = std::thread::spawn(move || {
        let _ = std::io::copy(&mut reader, &mut up2);
    });
    let _ = a.join();
    let _ = b.join();
}

/// 处理 CONNECT 隧道（HTTPS）
fn handle_connect(mut client: TcpStream, target: String, log: Arc<ProxyLog>) {
    log.log("connect", &target);
    let Some((hostport, _)) = target.split_once(char::is_whitespace) else { return };
    let (hostname, port) = match hostport.split_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(443)),
        None => (hostport.to_string(), 443),
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
                        let target = line.trim().to_string();
                        drop(reader);
                        handle_connect(s, target, log);
                    } else {
                        // 普通 http：把已读的首行与后续头一起处理
                        let mut rest = Vec::new();
                        // 读到头结束
                        let mut buf = [0u8; 1];
                        loop {
                            match reader.read(&mut buf) {
                                Ok(0) | Err(_) => break,
                                Ok(_) => {
                                    rest.push(buf[0]);
                                    if rest.ends_with(b"\r\n\r\n") || rest.ends_with(b"\n\n") {
                                        break;
                                    }
                                }
                            }
                        }
                        drop(reader);
                        let mut full = line.clone().into_bytes();
                        full.extend_from_slice(&rest);
                        let _ = full;
                        handle_http(s, log, Vec::new());
                    }
                });
            }
            Err(e) => {
                eprintln!("ezr-proxy: accept 错误（{e}）");
            }
        }
    }
}
