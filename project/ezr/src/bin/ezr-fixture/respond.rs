//! respond — 响应构造与 body 输出（状态注入/重定向/一致性头/Range/流式 body）

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

use crate::query::qget;
use crate::range::parse_range;

/// 写入响应头
pub fn write_head(
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

/// `?status=<code>[&retry_after=<s>]` 注入（探测必失败用例）；命中 → 已响应
pub fn respond_inject(w: &mut TcpStream, kv: &[(String, String)]) -> bool {
    let Some(status) = qget(kv, "status") else {
        return false;
    };
    let code = status.parse::<u16>().unwrap_or(500);
    let mut head = vec![("Content-Length".to_string(), "0".to_string())];
    if let Some(ra) = qget(kv, "retry_after") {
        head.push(("Retry-After".to_string(), ra.to_string()));
    }
    let head_refs: Vec<(&str, String)> =
        head.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let _ = write_head(w, code, "Injected", &head_refs);
    true
}

/// `?redirect=N`：Location 逐跳递减（N>0 才响应；命中 → 已响应）
pub fn respond_redirect(w: &mut TcpStream, target: &str, kv: &[(String, String)]) -> bool {
    match qget(kv, "redirect").and_then(|s| s.parse::<u32>().ok()) {
        Some(n) if n > 0 => {
            let next = target.replace(&format!("redirect={n}"), &format!("redirect={}", n - 1));
            let _ = write_head(
                w,
                302,
                "Found",
                &[("Location", next), ("Content-Length", "0".to_string())],
            );
            true
        }
        _ => false,
    }
}

/// ETag 口径：`?etag=X` 覆盖；否则 `"名字规范化-{total}"`（名字中 /?&= → `_`）；
/// `?noheaders=1` 时不返回
fn etag_of(kv: &[(String, String)], file_name: &str, total: u64) -> Option<String> {
    match qget(kv, "etag") {
        Some(e) => Some(e.to_string()),
        None => Some(format!(
            "\"{}-{total}\"",
            file_name.replace(['/', '?', '&', '='], "_")
        )),
    }
    .filter(|_| qget(kv, "noheaders").is_none())
}

/// 公共一致性头（ETag / Last-Modified / Content-Disposition / Accept-Ranges）
fn common_headers(kv: &[(String, String)], file_name: &str, total: u64) -> Vec<(String, String)> {
    let mut common: Vec<(String, String)> = Vec::new();
    if let Some(e) = etag_of(kv, file_name, total) {
        common.push(("ETag".to_string(), e));
    }
    if qget(kv, "noheaders").is_none() {
        common.push((
            "Last-Modified".to_string(),
            "Mon, 09 Feb 2026 08:00:00 GMT".to_string(),
        ));
    }
    if let Some(cd) = qget(kv, "cd") {
        common.push((
            "Content-Disposition".to_string(),
            format!("attachment; filename={cd}"),
        ));
    }
    if qget(kv, "norange").is_none() {
        common.push(("Accept-Ranges".to_string(), "bytes".to_string()));
    }
    common
}

/// body 数据源：swapsize 物化缓冲优先（从 start 切片），否则按需 seek 文件
/// （QA 大文件路径不整载内存——每请求内存占用恒定）
fn body_source<'a>(
    base: &Path,
    materialized: &'a Option<Vec<u8>>,
    start: u64,
) -> Box<dyn Read + 'a> {
    if let Some(d) = materialized {
        let s = (start as usize).min(d.len());
        return Box::new(&d[s..]);
    }
    let mut f = std::fs::File::open(base).expect("文件已确认存在");
    if start > 0 {
        use std::io::Seek;
        f.seek(std::io::SeekFrom::Start(start)).expect("seek 失败");
    }
    Box::new(f)
}

/// 流式输出 body：从 `src` 读 `remaining` 字节分块写出。
/// speed=0 → 64 KiB 连发；speed>0 → 100ms 节流分块（沿用既有每请求限速口径）；
/// disconnect_at=Some(n) → 只发 min(n, remaining) 字节后强制断连
pub fn emit_stream(
    w: &mut TcpStream,
    src: &mut impl Read,
    remaining: u64,
    speed: u64,
    disconnect_at: Option<u64>,
) {
    let limit = disconnect_at.map_or(remaining, |n| n.min(remaining));
    if speed == 0 {
        copy_n_quiet(w, src, limit);
    } else {
        emit_throttled(w, src, limit, speed);
    }
    if disconnect_at.is_some() {
        // 中途断连：直接关闭（不发 FIN 前的剩余数据）
        let _ = w.shutdown(std::net::Shutdown::Both);
    }
}

/// 不限速拷贝至多 `remaining` 字节（源提前耗尽即止；写失败静默放弃）
fn copy_n_quiet(w: &mut TcpStream, src: &mut impl Read, mut remaining: u64) {
    let mut buf = vec![0u8; 64 * 1024];
    while remaining > 0 {
        let want = buf.len().min(remaining as usize);
        match src.read(&mut buf[..want]) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if w.write_all(&buf[..n]).is_err() {
                    return;
                }
                remaining -= n as u64;
            }
        }
    }
    let _ = w.flush();
}

/// 慢速门控输出：每 100ms 发一批（沿用既有 `speed*100/1000` 口径）
fn emit_throttled(w: &mut TcpStream, src: &mut impl Read, limit: u64, speed: u64) {
    let chunk_ms = 100u64;
    let per_tick = ((speed as u128 * chunk_ms as u128) / 1000).max(1) as usize;
    let mut buf = vec![0u8; per_tick.max(1)];
    let mut sent: u64 = 0;
    while sent < limit {
        let want = buf.len().min((limit - sent) as usize);
        match src.read(&mut buf[..want]) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if w.write_all(&buf[..n]).is_err() || w.flush().is_err() {
                    return;
                }
                sent += n as u64;
                if sent >= limit {
                    break;
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(chunk_ms));
    }
}

/// 单个 chunk 帧（长度行 + 数据 + CRLF）
fn write_chunk(w: &mut TcpStream, chunk: &[u8]) -> std::io::Result<()> {
    writeln!(w, "{:x}\r", chunk.len())?;
    w.write_all(chunk)?;
    w.write_all(b"\r\n")?;
    Ok(())
}

/// chunked 输出（无 Content-Length；streamy.bin），流式读盘
pub fn emit_chunked(w: &mut TcpStream, src: &mut impl Read, speed: u64) {
    let per = if speed == 0 {
        64 * 1024
    } else {
        ((speed as usize * 100) / 1000).max(1)
    };
    let mut buf = vec![0u8; per.max(1)];
    loop {
        match src.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if write_chunk(w, &buf[..n]).is_err() {
                    return;
                }
                let _ = w.flush();
                if speed > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    }
    let _ = w.write_all(b"0\r\n\r\n");
    let _ = w.flush();
}

/// 文件响应：打开 → swapsize 注入 → 一致性头 → Range 判定 → 流式 body。
/// 返回 `false` = 文件不存在（已回 404）。
///
/// Range 口径（FR-01-12，QA-DE-02）：`?norange=1` 忽略 Range 返回 200 全量；
/// `streamy.bin`（无 Content-Length 形态）对 Range 同样返回 200 全量 chunked——
/// 隐藏 Content-Length 的服务端不应在 206 中携带 Content-Length/Content-Range，
/// 否则探测（Range: bytes=0-）会误判为支持续传。
#[allow(clippy::too_many_lines)] // 分支表即规格（状态注入顺序与 QA 口径一一对应）
pub fn serve_file(
    w: &mut TcpStream,
    kv: &[(String, String)],
    base: &Path,
    file_name: &str,
    range_header: Option<&str>,
) -> bool {
    let Ok(file) = std::fs::File::open(base) else {
        let _ = write_head(w, 404, "Not Found", &[("Content-Length", "0".to_string())]);
        return false;
    };
    let Ok(meta) = file.metadata() else {
        let _ = write_head(w, 404, "Not Found", &[("Content-Length", "0".to_string())]);
        return false;
    };
    if !meta.is_file() {
        let _ = write_head(w, 404, "Not Found", &[("Content-Length", "0".to_string())]);
        return false;
    }
    let file_size = meta.len();

    // 同 URL 换内容（swapsize=N）：物化到内存并 resize（扩容补 0xAB）；
    // 读取失败与原口径一致 → 404（QA 约定 swapsize 仅用于小文件）
    let swap_n = qget(kv, "swapsize").and_then(|s| s.parse::<u64>().ok());
    let materialized: Option<Vec<u8>> = match swap_n {
        Some(n) => match std::fs::read(base) {
            Ok(mut d) => {
                d.resize(n as usize, 0xAB);
                Some(d)
            }
            // 与原口径一致：读取失败 → 404
            Err(_) => {
                let _ = write_head(w, 404, "Not Found", &[("Content-Length", "0".to_string())]);
                return false;
            }
        },
        None => None,
    };
    let total = swap_n.unwrap_or(file_size);

    let mut common = common_headers(kv, file_name, total);
    let no_length = file_name.starts_with("streamy");
    let range = range_header
        .and_then(parse_range)
        .filter(|_| qget(kv, "norange").is_none())
        .filter(|_| !no_length);
    let speed = qget(kv, "speed")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let disconnect_at = qget(kv, "disconnect").and_then(|s| s.parse::<u64>().ok());

    match range {
        Some((start, end)) if start < total => {
            let end = end.min(total - 1);
            let slice_len = end
                .checked_sub(start)
                .expect("反向 Range（end<start）：沿用既有切片语义")
                + 1;
            common.push((
                "Content-Range".to_string(),
                format!("bytes {start}-{end}/{total}"),
            ));
            let refs: Vec<(&str, String)> = common
                .iter()
                .map(|(k, v)| (k.as_str(), v.clone()))
                .chain([("Content-Length", slice_len.to_string())])
                .collect();
            if write_head(w, 206, "Partial Content", &refs).is_ok() {
                let mut src = body_source(base, &materialized, start);
                emit_stream(
                    w,
                    &mut src,
                    slice_len,
                    speed,
                    disconnect_at.map(|n| n.min(slice_len)),
                );
            }
        }
        Some(_) => {
            // Range 起点越界：416
            let _ = write_head(
                w,
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
            if write_head(w, 200, "OK", &refs).is_ok() {
                let mut src = body_source(base, &materialized, 0);
                if no_length {
                    emit_chunked(w, &mut src, speed);
                } else {
                    emit_stream(
                        w,
                        &mut src,
                        total,
                        speed,
                        disconnect_at.map(|n| n.min(total)),
                    );
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn etag_override_and_noheaders() {
        let none = kv(&[]);
        assert_eq!(etag_of(&none, "f.bin", 100), Some("\"f.bin-100\"".into()));
        let over = kv(&[("etag", "X1")]);
        assert_eq!(etag_of(&over, "f.bin", 100), Some("X1".into()));
        let nh = kv(&[("noheaders", "1")]);
        assert_eq!(etag_of(&nh, "f.bin", 100), None);
        // 名字规范化：/?&= → _
        let none = kv(&[]);
        assert_eq!(
            etag_of(&none, "a/b?c&d=e", 7),
            Some("\"a_b_c_d_e-7\"".into())
        );
    }

    #[test]
    fn common_headers_respects_cd_and_norange() {
        let none = kv(&[]);
        let h = common_headers(&none, "f.bin", 10);
        assert!(h.contains(&("ETag".into(), "\"f.bin-10\"".into())));
        assert!(h.contains(&(
            "Last-Modified".into(),
            "Mon, 09 Feb 2026 08:00:00 GMT".into()
        )));
        assert!(h.contains(&("Accept-Ranges".into(), "bytes".into())));
        assert!(!h.iter().any(|(k, _)| k == "Content-Disposition"));

        let with = kv(&[("cd", "name.bin"), ("norange", "1"), ("noheaders", "1")]);
        let h = common_headers(&with, "f.bin", 10);
        assert!(!h.iter().any(|(k, _)| k == "ETag"));
        assert!(!h.iter().any(|(k, _)| k == "Last-Modified"));
        assert!(!h.iter().any(|(k, _)| k == "Accept-Ranges"));
        assert!(h.contains(&(
            "Content-Disposition".into(),
            "attachment; filename=name.bin".into()
        )));
    }

    #[test]
    fn respond_inject_and_redirect_gate() {
        // 无 status/redirect → 不响应
        let (c, mut s) = tcp_pair();
        assert!(!respond_inject(&mut s, &kv(&[])));
        drop(s);
        drop(c);
    }

    /// 建立一对已连接的回环 TCP 流
    fn tcp_pair() -> (TcpStream, TcpStream) {
        use std::net::TcpListener;
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let c = TcpStream::connect(addr).unwrap();
        let (s, _) = listener.accept().unwrap();
        (c, s)
    }

    fn read_all(c: &mut TcpStream) -> String {
        c.set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut buf = Vec::new();
        let _ = c.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    }

    #[test]
    fn status_injection_with_retry_after() {
        let (mut c, mut s) = tcp_pair();
        assert!(respond_inject(
            &mut s,
            &kv(&[("status", "503"), ("retry_after", "10")])
        ));
        drop(s);
        let resp = read_all(&mut c);
        assert!(resp.starts_with("HTTP/1.1 503 Injected\r\n"));
        assert!(resp.contains("Retry-After: 10"));
        assert!(resp.contains("Content-Length: 0"));
        // 非法 status → 500；无 retry_after → 无该头
        let (mut c, mut s) = tcp_pair();
        assert!(respond_inject(&mut s, &kv(&[("status", "zzz")])));
        drop(s);
        let resp = read_all(&mut c);
        assert!(resp.starts_with("HTTP/1.1 500 Injected\r\n"));
        assert!(!resp.contains("Retry-After"));
    }

    #[test]
    fn redirect_decrements_and_zero_falls_through() {
        let (mut c, mut s) = tcp_pair();
        assert!(respond_redirect(
            &mut s,
            "/f.bin?redirect=3&x=1",
            &kv(&[("redirect", "3"), ("x", "1")])
        ));
        drop(s);
        let resp = read_all(&mut c);
        assert!(resp.starts_with("HTTP/1.1 302 Found\r\n"));
        assert!(resp.contains("Location: /f.bin?redirect=2&x=1"));

        // redirect=0 → 不响应（落到文件服务）
        let (c, mut s) = tcp_pair();
        assert!(!respond_redirect(
            &mut s,
            "/f.bin?redirect=0",
            &kv(&[("redirect", "0")])
        ));
        drop(s);
        drop(c);
        // 非法 redirect → 不响应
        let (c, mut s) = tcp_pair();
        assert!(!respond_redirect(
            &mut s,
            "/f.bin",
            &kv(&[("redirect", "zz")])
        ));
        drop(s);
        drop(c);
    }
}
