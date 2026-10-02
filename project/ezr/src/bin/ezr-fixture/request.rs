//! request — fixture 侧 HTTP/1.1 请求解析与访问日志

use std::io::{BufRead, BufReader, Read, Write};

/// 请求行 + 头（手工 HTTP/1.1 解析，httparse）
pub struct Request {
    pub method: String,
    pub target: String,
    pub range: Option<String>,
    pub accept_encoding: Option<String>,
}

/// 按头名（大小写不敏感）取第一个头的值
fn header_find(headers: &[httparse::Header<'_>], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .and_then(|h| std::str::from_utf8(h.value).ok())
        .map(str::to_string)
}

/// 读取请求头块剩余字节（首行已被调用方消费；读到 `\r\n\r\n` 或流结束）
fn read_head_rest<R: Read>(stream: &mut BufReader<R>) -> std::io::Result<Vec<u8>> {
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
    Ok(rest)
}

/// 读取并解析一个请求（对端关闭 → Ok(None)）
pub fn read_request<R: Read>(stream: &mut BufReader<R>) -> std::io::Result<Option<Request>> {
    let mut line = String::new();
    if stream.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut req = httparse::Request::new(&mut headers);
    // httparse 需要完整的头块：把首行与其余头一起喂入
    let rest = read_head_rest(stream)?;
    let mut head = line.clone().into_bytes();
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
    let range = header_find(req.headers, "range");
    let accept_encoding = header_find(req.headers, "accept-encoding");
    Ok(Some(Request {
        method,
        target,
        range,
        accept_encoding,
    }))
}

/// 访问日志（JSONL；字段：ts/method/path/range/accept_encoding）
pub struct AccessLog {
    pub(crate) file: Option<std::sync::Mutex<std::fs::File>>,
}

impl AccessLog {
    pub fn open(path: Option<&str>) -> AccessLog {
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

    pub fn log(&self, method: &str, path: &str, range: Option<&str>, ae: Option<&str>) {
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn parse_all(input: &'static [u8]) -> std::io::Result<Option<Request>> {
        let mut r = BufReader::new(Cursor::new(input));
        read_request(&mut r)
    }

    #[test]
    fn read_request_headers_and_eof() {
        let req = parse_all(
            b"GET /a.bin?x=1 HTTP/1.1\r\nRange: bytes=5-\r\nAccept-Encoding: identity\r\n\r\n",
        )
        .unwrap()
        .unwrap();
        assert_eq!(req.method, "GET");
        assert_eq!(req.target, "/a.bin?x=1");
        assert_eq!(req.range.as_deref(), Some("bytes=5-"));
        assert_eq!(req.accept_encoding.as_deref(), Some("identity"));
        // 对端关闭（空输入）→ None
        assert!(parse_all(b"").unwrap().is_none());
        // 无 Range 头 → None
        let req = parse_all(b"GET /a HTTP/1.1\r\n\r\n").unwrap().unwrap();
        assert!(req.range.is_none());
    }

    #[test]
    fn read_request_headless_body_is_error() {
        // 只有半截头块 → partial → InvalidData
        let r = parse_all(b"GET /a HTTP/1.1\r\nRange: bytes=0-");
        assert!(r.is_err());
    }

    #[test]
    fn access_log_jsonl_shape() {
        let dir = std::env::temp_dir().join(format!("ezr-fx-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.jsonl");
        {
            let log = AccessLog::open(p.to_str());
            log.log("GET", "/f.bin", Some("bytes=0-9"), None);
        }
        let content = std::fs::read_to_string(&p).unwrap();
        assert!(content.contains("\"method\":\"GET\""));
        assert!(content.contains("\"path\":\"/f.bin\""));
        assert!(content.contains("\"range\":\"bytes=0-9\""));
        assert!(content.contains("\"accept_encoding\":null"));
        assert!(content.trim_end().ends_with('}'));
        std::fs::remove_dir_all(&dir).ok();
    }
}
