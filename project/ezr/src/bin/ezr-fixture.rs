//! ezr-fixture — 本地 fixture 服务器（QA 基建 + 交付物，AC-10）
//!
//! 能力清单（对应 qa/ 各套件环境前置节）：
//! - Range 支持/关闭（`?norange=1`）、隐藏 Content-Length（`streamy.bin` chunked）
//! - 状态码注入（`?status=503`）+ Retry-After（`?retry_after=10`）
//! - 传输中途断连（`?disconnect=N`：传 N 字节后断）、慢速门控（`?speed=B/s`）
//! - 重定向链（`?redirect=N`：再跳 N 次）
//! - 一致性头控制：`?etag=X` 覆盖 / `?noheaders=1` 不返回 ETag/Last-Modified
//! - Content-Disposition（`?cd=name`）
//! - 同 URL 换内容（`?swapsize=N`）
//! - 请求头记录（`--log <jsonl>`：method/path/range/accept-encoding）
//! - 文件魔法名：`ghost-404.bin` → 404；`streamy.bin` → chunked 无 Content-Length
//!
//! 用法：
//! ```text
//! ezr-fixture gen --root <dir>            # 生成标准文件集（含 100MB 稀疏）
//! ezr-fixture serve --root <dir> --port 8765 [--log access.jsonl]
//! ```
//!
//! 文件内容确定性：字节模式 `i % 251`，供校验值预计算。
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

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 简单查询参数解析（k=v&k=v；值不解码——fixture 路径约定为 ASCII）
fn parse_query(url: &str) -> (String, Vec<(String, String)>) {
    let (path, q) = match url.split_once('?') {
        Some((p, q)) => (p.to_string(), q),
        None => return (url.to_string(), Vec::new()),
    };
    let mut kv = Vec::new();
    for pair in q.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            kv.push((k.to_string(), v.to_string()));
        } else if !pair.is_empty() {
            kv.push((pair.to_string(), String::new()));
        }
    }
    (path, kv)
}

fn qget<'a>(kv: &'a [(String, String)], k: &str) -> Option<&'a str> {
    kv.iter().find(|(key, _)| key == k).map(|(_, v)| v.as_str())
}

/// 请求行 + 头（手工 HTTP/1.1 解析，httparse）
struct Request {
    method: String,
    target: String,
    range: Option<String>,
    accept_encoding: Option<String>,
}

fn read_request(stream: &mut BufReader<TcpStream>) -> std::io::Result<Option<Request>> {
    let mut line = String::new();
    if stream.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut req = httparse::Request::new(&mut headers);
    // httparse 需要完整的头块：把首行与其余头一起喂入
    let mut rest = Vec::new();
    loop {
        let mut buf = [0u8; 1];
        let n = stream.read(&mut buf)?;
        if n == 0 {
            break;
        }
        rest.push(buf[0]);
        if rest.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let full = line.clone().into_bytes();
    let mut head = full;
    head.extend_from_slice(&rest);
    let status = req
        .parse(&head)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    if status.is_partial() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "partial request",
        ));
    }
    let method = req.method.unwrap_or("GET").to_string();
    let target = req.path.unwrap_or("/").to_string();
    let range = req
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("range"))
        .and_then(|h| std::str::from_utf8(h.value).ok())
        .map(str::to_string);
    let accept_encoding = req
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("accept-encoding"))
        .and_then(|h| std::str::from_utf8(h.value).ok())
        .map(str::to_string);
    Ok(Some(Request {
        method,
        target,
        range,
        accept_encoding,
    }))
}

/// 访问日志（JSONL；字段：ts/method/path/range/accept_encoding）
struct AccessLog {
    file: Option<std::sync::Mutex<std::fs::File>>,
}

impl AccessLog {
    fn open(path: Option<&str>) -> AccessLog {
        AccessLog {
            file: path.map(|p| {
                std::sync::Mutex::new(
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(p)
                        .expect("打开日志失败"),
                )
            }),
        }
    }

    fn log(&self, method: &str, path: &str, range: Option<&str>, ae: Option<&str>) {
        if let Some(f) = &self.file {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let line = format!(
                "{{\"ts\":{ts},\"method\":\"{method}\",\"path\":\"{path}\",\"range\":{},\"accept_encoding\":{}}}\n",
                range.map_or("null".to_string(), |r| format!("\"{r}\"")),
                ae.map_or("null".to_string(), |r| format!("\"{r}\"")),
            );
            let _ = f.lock().unwrap().write_all(line.as_bytes());
        }
    }
}

/// 生成标准文件集（qa/ 环境前置节文件清单；内容确定性）
fn gen_files(root: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    let put = |name: &str, size: u64, sparse: bool| -> std::io::Result<()> {
        let p = root.join(name);
        if p.exists() {
            return Ok(());
        }
        let f = std::fs::File::create(&p)?;
        if sparse {
            f.set_len(size)?;
        } else {
            let mut f = f;
            let mut buf = vec![0u8; 64 * 1024];
            let mut remaining = size;
            let mut i: u64 = 0;
            while remaining > 0 {
                let n = buf.len().min(remaining as usize);
                for b in buf[..n].iter_mut() {
                    *b = (i % 251) as u8;
                    i += 1;
                }
                f.write_all(&buf[..n])?;
                remaining -= n as u64;
            }
        }
        Ok(())
    };
    put("five-m.bin", 5 * 1024 * 1024, false)?;
    put("three-m.bin", 3 * 1024 * 1024, false)?;
    put("eight-m.bin", 8 * 1024 * 1024, false)?;
    put("twelve-m.bin", 12 * 1024 * 1024, false)?;
    put("two-m.bin", 2 * 1024 * 1024, false)?;
    put("one-m.bin", 1024 * 1024, false)?;
    put("half-m.bin", 512 * 1024, false)?;
    put("ten-m.bin", 10 * 1024 * 1024, false)?;
    put("small.bin", 1024, false)?;
    put("big-100m.bin", 100 * 1024 * 1024, true)?;
    put("streamy.bin", 4 * 1024 * 1024, false)?;
    put("ghost-404.bin", 0, false)?;
    println!("文件集已生成: {}", root.display());
    Ok(())
}

/// 写入响应头
fn write_head(
    w: &mut impl Write,
    status: u16,
    reason: &str,
    headers: &[(&str, String)],
) -> std::io::Result<()> {
    let mut head =
        format!("HTTP/1.1 {status} {reason}\r\nServer: ezr-fixture\r\nConnection: close\r\n");
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    w.write_all(head.as_bytes())
}

/// 单请求处理
fn handle(mut stream: TcpStream, root: Arc<PathBuf>, log: Arc<AccessLog>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone 失败"));
    let req = read_request(&mut reader).ok().flatten();
    let Some(req) = req else { return };
    let (path_raw, kv) = parse_query(&req.target);
    log.log(
        &req.method,
        &req.target,
        req.range.as_deref(),
        req.accept_encoding.as_deref(),
    );

    // 状态码注入（探测必失败用例）
    if let Some(status) = qget(&kv, "status") {
        let code = status.parse::<u16>().unwrap_or(500);
        let ra = qget(&kv, "retry_after").map(|s| s.to_string());
        let mut head = vec![("Content-Length".to_string(), "0".to_string())];
        if let Some(ra) = &ra {
            head.push(("Retry-After".to_string(), ra.clone()));
        }
        let head_refs: Vec<(&str, String)> =
            head.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        let _ = write_head(&mut stream, code, "Injected", &head_refs);
        return;
    }

    // 重定向链（FR-01-14：Location 逐跳递减）
    if let Some(n) = qget(&kv, "redirect").and_then(|s| s.parse::<u32>().ok()) {
        if n > 0 {
            let next = req
                .target
                .replace(&format!("redirect={n}"), &format!("redirect={}", n - 1));
            let _ = write_head(
                &mut stream,
                302,
                "Found",
                &[("Location", next), ("Content-Length", "0".to_string())],
            );
            return;
        }
    }

    let file_name = path_raw.trim_start_matches('/');
    let base = root.join(file_name);

    // 魔法名：ghost-404
    if file_name.starts_with("ghost-404") {
        let _ = write_head(
            &mut stream,
            404,
            "Not Found",
            &[("Content-Length", "0".to_string())],
        );
        return;
    }

    let Ok(data) = std::fs::read(&base) else {
        let _ = write_head(
            &mut stream,
            404,
            "Not Found",
            &[("Content-Length", "0".to_string())],
        );
        return;
    };

    // 同 URL 换内容（swapsize=N）
    let data = match qget(&kv, "swapsize").and_then(|s| s.parse::<u64>().ok()) {
        Some(n) => {
            let mut d = data.clone();
            d.resize(n as usize, 0xAB);
            d
        }
        None => data,
    };

    let total = data.len() as u64;
    // 一致性头（noheaders=1 → 不返回 ETag/Last-Modified；etag=X → 覆盖）
    let etag = match qget(&kv, "etag") {
        Some(e) => Some(e.to_string()),
        None => Some(format!(
            "\"{}-{total}\"",
            file_name.replace(['/', '?', '&', '='], "_")
        )),
    }
    .filter(|_| qget(&kv, "noheaders").is_none());
    let last_modified = qget(&kv, "noheaders")
        .is_none()
        .then(|| "Mon, 09 Feb 2026 08:00:00 GMT".to_string());

    let mut common: Vec<(String, String)> = Vec::new();
    if let Some(e) = &etag {
        common.push(("ETag".to_string(), e.clone()));
    }
    if let Some(lm) = &last_modified {
        common.push(("Last-Modified".to_string(), lm.clone()));
    }
    if let Some(cd) = qget(&kv, "cd") {
        common.push((
            "Content-Disposition".to_string(),
            format!("attachment; filename={cd}"),
        ));
    }
    if qget(&kv, "norange").is_none() {
        common.push(("Accept-Ranges".to_string(), "bytes".to_string()));
    }

    // 慢速门控（speed=B/s）与中途断连（disconnect=N）
    let speed = qget(&kv, "speed")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let disconnect_at = qget(&kv, "disconnect").and_then(|s| s.parse::<u64>().ok());

    // streamy.bin：chunked 无 Content-Length（FR-01-12 无 Content-Length 分支）
    let no_length = file_name.starts_with("streamy");

    // Range 判定（norange=1 → 忽略 Range 返回 200 全量，AC-4）
    let range = req
        .range
        .as_deref()
        .and_then(parse_range)
        .filter(|_| qget(&kv, "norange").is_none());
    match range {
        Some((start, end)) if start < total => {
            let end = end.min(total - 1);
            let slice = &data[start as usize..=(end as usize)];
            common.push((
                "Content-Range".to_string(),
                format!("bytes {start}-{end}/{total}"),
            ));
            let refs: Vec<(&str, String)> = common
                .iter()
                .map(|(k, v)| (k.as_str(), v.clone()))
                .chain([("Content-Length", slice.len().to_string())])
                .collect();
            if write_head(&mut stream, 206, "Partial Content", &refs).is_ok() {
                emit_body(
                    &mut stream,
                    slice,
                    speed,
                    disconnect_at.map(|n| n.min(slice.len() as u64) as usize),
                );
            }
        }
        Some(_) => {
            // Range 起点越界：416
            let _ = write_head(
                &mut stream,
                416,
                "Range Not Satisfiable",
                &[("Content-Length", "0".to_string())],
            );
        }
        None => {
            // 200 全量（含 norange 与无 Range 头）
            let refs: Vec<(&str, String)> = if no_length {
                common
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.clone()))
                    .chain([("Transfer-Encoding", "chunked".to_string())])
                    .collect()
            } else {
                common
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.clone()))
                    .chain([("Content-Length", total.to_string())])
                    .collect()
            };
            if write_head(&mut stream, 200, "OK", &refs).is_ok() {
                if no_length {
                    emit_chunked(&mut stream, &data, speed);
                } else {
                    emit_body(
                        &mut stream,
                        &data,
                        speed,
                        disconnect_at.map(|n| (n.min(total)) as usize),
                    );
                }
            }
        }
    }
}

/// bytes=a-b / bytes=a- 解析
fn parse_range(v: &str) -> Option<(u64, u64)> {
    let v = v.strip_prefix("bytes=")?;
    let (a, b) = v.split_once('-')?;
    let start = a.trim().parse::<u64>().ok()?;
    let end = if b.trim().is_empty() {
        u64::MAX
    } else {
        b.trim().parse::<u64>().ok()?
    };
    Some((start, end))
}

/// 全量 body 输出（慢速门控/中途断连注入）
fn emit_body(w: &mut TcpStream, data: &[u8], speed: u64, disconnect_at: Option<usize>) {
    let limit = disconnect_at.unwrap_or(data.len());
    if speed == 0 {
        let _ = w.write_all(&data[..limit]);
        let _ = w.flush();
    } else {
        let chunk_ms = 100;
        let per_tick = ((speed as u128 * chunk_ms) / 1000).max(1) as usize;
        let mut sent = 0usize;
        for chunk in data[..limit].chunks(per_tick.max(1)) {
            if w.write_all(chunk).is_err() || w.flush().is_err() {
                return;
            }
            sent += chunk.len();
            if sent >= limit {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(chunk_ms as u64));
        }
    }
    if disconnect_at.is_some() {
        // 中途断连：直接关闭（不发 FIN 前的剩余数据）
        let _ = w.shutdown(std::net::Shutdown::Both);
    }
}

/// chunked 输出（无 Content-Length）
fn emit_chunked(w: &mut TcpStream, data: &[u8], speed: u64) {
    let per = if speed == 0 {
        64 * 1024
    } else {
        ((speed as usize * 100) / 1000).max(1)
    };
    for chunk in data.chunks(per.max(1)) {
        if writeln!(w, "{:x}\r", chunk.len()).is_err()
            || w.write_all(chunk).is_err()
            || w.write_all(b"\r\n").is_err()
        {
            return;
        }
        let _ = w.flush();
        if speed > 0 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    let _ = w.write_all(b"0\r\n\r\n");
    let _ = w.flush();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut root = String::from(".");
    let mut port: u16 = 8765;
    let mut log_path = None;
    let mut cmd = String::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "gen" | "serve" => cmd = args[i].clone(),
            "--root" => {
                i += 1;
                root = args.get(i).cloned().unwrap_or(root);
            }
            "--port" => {
                i += 1;
                port = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(port);
            }
            "--log" => {
                i += 1;
                log_path = args.get(i).cloned();
            }
            _ => {}
        }
        i += 1;
    }
    let root_path: PathBuf = root.into();
    match cmd.as_str() {
        "gen" => {
            if let Err(e) = gen_files(&root_path) {
                eprintln!("ezr-fixture: 生成失败（{e}）");
                std::process::exit(1);
            }
        }
        "serve" => {
            let listener = TcpListener::bind(("127.0.0.1", port)).expect("绑定失败");
            println!(
                "ezr-fixture listening on http://127.0.0.1:{port} root={}",
                root_path.display()
            );
            let log = Arc::new(AccessLog::open(log_path.as_deref()));
            let root = Arc::new(root_path);
            for conn in listener.incoming() {
                match conn {
                    Ok(s) => {
                        let root = Arc::clone(&root);
                        let log = Arc::clone(&log);
                        // fixture 线程：detach（测试进程退出自然回收，engineering.md 纪律）
                        std::thread::spawn(move || handle(s, root, log));
                    }
                    // accept 瞬时错误：记录并 continue（engineering.md 网络 fixture 条款）
                    Err(e) => {
                        eprintln!("ezr-fixture: accept 错误（{e}）");
                    }
                }
            }
        }
        _ => {
            eprintln!(
                "用法:\n  ezr-fixture gen --root <dir>\n  ezr-fixture serve --root <dir> --port 8765 [--log access.jsonl]"
            );
            std::process::exit(2);
        }
    }
}
