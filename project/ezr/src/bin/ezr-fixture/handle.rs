//! handle — 单请求处理（注入判定 → 魔法名 → 文件服务）

use std::io::BufReader;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;

use crate::query::parse_query;
use crate::request::{read_request, AccessLog};
use crate::respond::{respond_inject, respond_redirect, serve_file, write_head};

/// 单请求处理
pub fn handle(mut stream: TcpStream, root: Arc<PathBuf>, log: Arc<AccessLog>) {
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
    if respond_inject(&mut stream, &kv) {
        return;
    }

    // 重定向链（FR-01-14：Location 逐跳递减）
    if respond_redirect(&mut stream, &req.target, &kv) {
        return;
    }

    let file_name = path_raw.trim_start_matches('/').to_string();
    let base = root.join(&file_name);

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

    let _ = serve_file(&mut stream, &kv, &base, &file_name, req.range.as_deref());
}

#[cfg(test)]
mod handle_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// 起一个临时 root 的 fixture，返回 (端口, root 路径)
    fn spawn_fixture() -> (u16, PathBuf) {
        let dir = std::env::temp_dir().join(format!("ezr-fx-h-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // 3000 字节 i%251 模式文件
        let data: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(dir.join("f.bin"), &data).unwrap();
        std::fs::write(dir.join("streamy.bin"), &data).unwrap();
        // 100MB 稀疏文件（磁盘开销可忽略，engineering.md 稀疏 fixture 条款）
        let big = std::fs::File::create(dir.join("big-100m.bin")).unwrap();
        big.set_len(100 * 1024 * 1024).unwrap();
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let log = Arc::new(AccessLog::open(None));
        let root_dir = dir.clone();
        std::thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                let root = Arc::new(root_dir.clone());
                let log = Arc::clone(&log);
                std::thread::spawn(move || handle(conn, root, log));
            }
        });
        (port, dir)
    }

    fn get(port: u16, target: &str, extra: &[&str]) -> Vec<u8> {
        let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
        c.set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut req = format!("GET {target} HTTP/1.1\r\nHost: t\r\n");
        for e in extra {
            req.push_str(e);
            req.push_str("\r\n");
        }
        req.push_str("\r\n");
        c.write_all(req.as_bytes()).unwrap();
        let mut buf = Vec::new();
        let _ = c.read_to_end(&mut buf);
        buf
    }

    fn body_of(resp: &[u8]) -> Vec<u8> {
        resp.windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|p| resp[p + 4..].to_vec())
            .unwrap_or_default()
    }

    fn head_of(resp: &[u8]) -> String {
        String::from_utf8_lossy(
            resp.split_once_str(b"\r\n\r\n")
                .map(|(h, _)| h)
                .unwrap_or(resp),
        )
        .into_owned()
    }

    trait SplitOnce {
        fn split_once_str(&self, sep: &[u8]) -> Option<(&[u8], &[u8])>;
    }
    impl SplitOnce for [u8] {
        fn split_once_str(&self, sep: &[u8]) -> Option<(&[u8], &[u8])> {
            (0..=self.len().saturating_sub(sep.len()))
                .find(|&i| &self[i..i + sep.len()] == sep)
                .map(|i| (&self[..i], &self[i + sep.len()..]))
        }
    }

    #[test]
    fn full_200_bytes_exact() {
        let (port, dir) = spawn_fixture();
        let resp = get(port, "/f.bin", &[]);
        assert!(head_of(&resp).starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(head_of(&resp).contains("Content-Length: 3000"));
        assert!(head_of(&resp).contains("Accept-Ranges: bytes"));
        assert!(head_of(&resp).contains("ETag: \"f.bin-3000\""));
        let expected: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
        assert_eq!(body_of(&resp), expected);
        drop(dir);
    }

    #[test]
    fn range_206_bytes_exact() {
        let (port, dir) = spawn_fixture();
        let resp = get(port, "/f.bin", &["Range: bytes=100-199"]);
        let head = head_of(&resp);
        assert!(
            head.starts_with("HTTP/1.1 206 Partial Content\r\n"),
            "{head}"
        );
        assert!(head.contains("Content-Range: bytes 100-199/3000"));
        assert!(head.contains("Content-Length: 100"));
        let expected: Vec<u8> = (100..200u32).map(|i| (i % 251) as u8).collect();
        assert_eq!(body_of(&resp), expected);
        // 开放末端：bytes=2990- → 到 2999
        let resp = get(port, "/f.bin", &["Range: bytes=2990-"]);
        assert!(head_of(&resp).contains("Content-Range: bytes 2990-2999/3000"));
        assert_eq!(body_of(&resp).len(), 10);
        drop(dir);
    }

    #[test]
    fn range_out_of_bounds_is_416() {
        let (port, dir) = spawn_fixture();
        let resp = get(port, "/f.bin", &["Range: bytes=3000-"]);
        assert!(head_of(&resp).starts_with("HTTP/1.1 416 Range Not Satisfiable\r\n"));
        drop(dir);
    }

    #[test]
    fn norange_ignores_range_header() {
        let (port, dir) = spawn_fixture();
        let resp = get(port, "/f.bin?norange=1", &["Range: bytes=0-9"]);
        let head = head_of(&resp);
        assert!(head.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(!head.contains("Accept-Ranges"));
        assert_eq!(body_of(&resp).len(), 3000);
        drop(dir);
    }

    #[test]
    fn ghost_404_and_missing_file() {
        let (port, dir) = spawn_fixture();
        let resp = get(port, "/ghost-404.bin", &[]);
        assert!(head_of(&resp).starts_with("HTTP/1.1 404 Not Found\r\n"));
        let resp = get(port, "/no-such.bin", &[]);
        assert!(head_of(&resp).starts_with("HTTP/1.1 404 Not Found\r\n"));
        drop(dir);
    }

    #[test]
    fn status_injection_and_redirect_chain() {
        let (port, dir) = spawn_fixture();
        let resp = get(port, "/f.bin?status=503&retry_after=10", &[]);
        let head = head_of(&resp);
        assert!(head.starts_with("HTTP/1.1 503 Injected\r\n"), "{head}");
        assert!(head.contains("Retry-After: 10"));
        let resp = get(port, "/f.bin?redirect=2", &[]);
        let head = head_of(&resp);
        assert!(head.starts_with("HTTP/1.1 302 Found\r\n"));
        assert!(head.contains("Location: /f.bin?redirect=1"));
        // redirect=0 → 正常文件响应
        let resp = get(port, "/f.bin?redirect=0", &[]);
        assert!(head_of(&resp).starts_with("HTTP/1.1 200 OK\r\n"));
        drop(dir);
    }

    #[test]
    fn swapsize_truncates_and_extends() {
        let (port, dir) = spawn_fixture();
        // 截断到 100 字节
        let resp = get(port, "/f.bin?swapsize=100", &[]);
        let head = head_of(&resp);
        assert!(head.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(head.contains("Content-Length: 100"));
        assert!(head.contains("ETag: \"f.bin-100\""));
        let expected: Vec<u8> = (0..100u32).map(|i| (i % 251) as u8).collect();
        assert_eq!(body_of(&resp), expected);
        // 扩到 3010：尾部补 0xAB
        let resp = get(port, "/f.bin?swapsize=3010", &[]);
        let body = body_of(&resp);
        assert_eq!(body.len(), 3010);
        assert!(body[3000..].iter().all(|&b| b == 0xAB));
        drop(dir);
    }

    #[test]
    fn streamy_chunked_no_content_length_and_range_falls_to_200() {
        let (port, dir) = spawn_fixture();
        let resp = get(port, "/streamy.bin", &[]);
        let head = head_of(&resp);
        assert!(head.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(!head.contains("Content-Length:"));
        assert!(head.contains("Transfer-Encoding: chunked"));
        // 解 chunked 帧重组，字节与源一致
        let body = body_of(&resp);
        let mut data = Vec::new();
        let mut i = 0;
        while i < body.len() {
            let line_end = body[i..]
                .windows(2)
                .position(|w| w == b"\r\n")
                .expect("chunk size line")
                + i;
            let size =
                usize::from_str_radix(std::str::from_utf8(&body[i..line_end]).unwrap().trim(), 16)
                    .unwrap();
            if size == 0 {
                break;
            }
            data.extend_from_slice(&body[line_end + 2..line_end + 2 + size]);
            i = line_end + 2 + size + 2;
        }
        let expected: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
        assert_eq!(data, expected);
        // streamy 的 Range 请求 → 200 全量 chunked（不带 Content-Range）
        let resp = get(port, "/streamy.bin", &["Range: bytes=0-9"]);
        let head = head_of(&resp);
        assert!(head.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(!head.contains("Content-Range"));
        drop(dir);
    }

    #[test]
    fn disconnect_cuts_connection_mid_body() {
        let (port, dir) = spawn_fixture();
        let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
        c.set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        c.write_all(b"GET /f.bin?disconnect=100 HTTP/1.1\r\nHost: t\r\n\r\n")
            .unwrap();
        let mut buf = Vec::new();
        let _ = c.read_to_end(&mut buf);
        // 动态定位头块边界：断连后 body 只有 100 字节（远小于 Content-Length）
        let (h, b) = buf.split_once_str(b"\r\n\r\n").unwrap();
        assert_eq!(b.len(), 100);
        let head = String::from_utf8_lossy(h).into_owned();
        assert!(head.starts_with("HTTP/1.1 200 OK\r\n"), "{head}");
        assert!(head.contains("Content-Length: 3000"));
        drop(dir);
    }

    #[test]
    fn etag_override_and_noheaders_and_cd() {
        let (port, dir) = spawn_fixture();
        let resp = get(port, "/f.bin?etag=XYZ", &[]);
        assert!(head_of(&resp).contains("ETag: XYZ"));
        let resp = get(port, "/f.bin?noheaders=1", &[]);
        let head = head_of(&resp);
        assert!(!head.contains("ETag:"));
        assert!(!head.contains("Last-Modified:"));
        let resp = get(port, "/f.bin?cd=renamed.bin", &[]);
        assert!(head_of(&resp).contains("Content-Disposition: attachment; filename=renamed.bin"));
        drop(dir);
    }

    #[test]
    fn big_sparse_file_streams_without_materializing() {
        let (port, dir) = spawn_fixture();
        // 100MB 稀疏文件：只读响应头 + 少量字节即断开（不消费全量）
        let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
        c.set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        c.write_all(b"GET /big-100m.bin HTTP/1.1\r\nHost: t\r\n\r\n")
            .unwrap();
        let mut buf = vec![0u8; 8192];
        let n = c.read(&mut buf).unwrap();
        let head = String::from_utf8_lossy(&buf[..n.min(2048)]).into_owned();
        assert!(head.contains("Content-Length: 104857600"), "{head}");
        drop(c);
        drop(dir);
    }
}
