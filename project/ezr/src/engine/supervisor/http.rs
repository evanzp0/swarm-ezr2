use super::super::error::{classify_reqwest, parse_retry_after_header, EngineFailure};
use crate::model::namegen;

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

pub(super) struct ProbeHead {
    pub(super) status: u16,
    pub(super) total: u64,
    pub(super) etag: Option<String>,
    pub(super) last_modified: Option<String>,
    pub(super) cd_name: Option<String>,
    pub(super) retry_after: Option<f64>,
}

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
