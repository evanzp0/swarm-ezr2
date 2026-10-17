//! supervisor::http — 请求构建与探测头解析（贴近 IO 的 HTTP 请求面）
//!
//! 职责（architect v145 拆分，纯移动）：identity 头请求构建（FR-01-16）、
//! 请求臂失败分类收口（req_checked：多块 worker / 单流续传共用）、探测
//! 响应头快照解析（GET Range: bytes=0-）。

use super::super::error::{classify_reqwest, parse_retry_after_header, EngineFailure};
use crate::model::namegen;

/// 下载请求构建（公共头：禁用内容压缩，FR-01-16）。client 由调用方按当前
/// 任务代理端点解析（v1.5/FR-01-86：client 缓存池按端点查池）。
pub(super) fn req_with_identity(
    client: &reqwest::Client,
    url: &str,
    range: Option<String>,
) -> reqwest::RequestBuilder {
    let mut rb = client
        .get(url)
        .header("Accept-Encoding", "identity")
        .header("User-Agent", concat!("ezr/", env!("CARGO_PKG_VERSION")));
    if let Some(r) = range {
        rb = rb.header("Range", r);
    }
    rb
}

/// 请求臂失败分类收口（多块 worker / 单流续传共用，PMD CPD 同构收敛）：
/// 2xx → 响应；非 2xx → 读尽响应体后 `EngineFailure::http`（含 Retry-After
/// 解析）；传输错误 → `classify_reqwest`。失败分类单源，v1.27「失败回报
/// 携带持久化事实」的口径由调用方各自接续。
pub(super) async fn req_checked(
    client: &reqwest::Client,
    url: &str,
    range: Option<String>,
) -> Result<reqwest::Response, EngineFailure> {
    match req_with_identity(client, url, range).send().await {
        Ok(r) if r.status().is_success() => Ok(r),
        Ok(r) => {
            let status = r.status().as_u16();
            let ra = parse_retry_after_header(
                r.headers().get("retry-after").and_then(|v| v.to_str().ok()),
            );
            let _ = r.bytes().await;
            Err(EngineFailure::http(status, ra))
        }
        Err(e) => Err(classify_reqwest(&e)),
    }
}

/// 探测响应头快照
pub(super) struct ProbeHead {
    pub(super) status: u16,
    pub(super) total: u64,
    pub(super) etag: Option<String>,
    pub(super) last_modified: Option<String>,
    pub(super) cd_name: Option<String>,
    pub(super) retry_after: Option<f64>,
}

/// 解析探测响应头（GET Range: bytes=0-）
pub(super) fn probe_head(r: &reqwest::Response) -> ProbeHead {
    let h = r.headers();
    ProbeHead {
        status: r.status().as_u16(),
        total: h
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(0),
        etag: h
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string),
        last_modified: h
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string),
        cd_name: h
            .get("content-disposition")
            .and_then(|v| v.to_str().ok())
            .and_then(namegen::from_content_disposition),
        retry_after: parse_retry_after_header(h.get("retry-after").and_then(|v| v.to_str().ok())),
    }
}
