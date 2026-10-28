use crate::model::FailKind;

#[derive(Debug, Clone)]
pub struct EngineFailure {
    pub kind: FailKind,
    pub reason: String,
    pub retry_after: Option<f64>,
}

impl EngineFailure {
    #[must_use]
    pub fn network(reason: impl Into<String>) -> EngineFailure {
        EngineFailure {
            kind: FailKind::Transient,
            reason: reason.into(),
            retry_after: None,
        }
    }

    #[must_use]
    pub fn http(status: u16, retry_after: Option<f64>) -> EngineFailure {
        EngineFailure {
            kind: crate::model::retry::classify_http_status(status),
            reason: format!("HTTP {status} {}", status_text(status)),
            retry_after,
        }
    }

    #[must_use]
    pub fn fatal(reason: impl Into<String>) -> EngineFailure {
        EngineFailure {
            kind: FailKind::Fatal,
            reason: reason.into(),
            retry_after: None,
        }
    }
}

const STATUS_TEXTS: [(u16, &str); 12] = [
    (400, "Bad Request（请求非法）"),
    (401, "Unauthorized（需要认证）"),
    (403, "Forbidden（拒绝访问）"),
    (404, "Not Found（资源不存在）"),
    (408, "Request Timeout（请求超时）"),
    (410, "Gone（资源已移除）"),
    (416, "Range Not Satisfiable（区间不可满足）"),
    (429, "Too Many Requests（请求过多）"),
    (500, "Internal Server Error（服务器内部错误）"),
    (502, "Bad Gateway（网关错误）"),
    (503, "Service Unavailable（服务不可用）"),
    (504, "Gateway Timeout（网关超时）"),
];

#[must_use]
pub fn status_text(status: u16) -> &'static str {
    STATUS_TEXTS
        .iter()
        .find(|(code, _)| *code == status)
        .map_or("（服务器错误响应）", |(_, text)| text)
}

const CERT_KEYWORDS: [&str; 5] = [
    "certificate",
    "tls",
    "UnknownIssuer",
    "CertExpired",
    "NotValidFor",
];

struct FailureFeatures<'a> {
    msg_chain: &'a str,
    status: Option<u16>,
    is_redirect: bool,
    is_builder: bool,
    is_timeout: bool,
    is_connect: bool,
    is_body_or_decode: bool,
}

fn builder_bad_protocol(is_builder: bool, msg_chain: &str) -> bool {
    is_builder && redirect_target_scheme(msg_chain).is_some_and(|s| s != "http" && s != "https")
}

fn network_reason(f: &FailureFeatures<'_>) -> String {
    if f.is_timeout {
        "连接超时".to_string()
    } else if f.is_connect {
        format!("连接失败（{}）", f.msg_chain)
    } else if f.is_body_or_decode {
        format!("传输中断（{}）", f.msg_chain)
    } else {
        format!("网络错误（{}）", f.msg_chain)
    }
}

fn classify_features(f: FailureFeatures<'_>) -> EngineFailure {
    if CERT_KEYWORDS.iter().any(|k| f.msg_chain.contains(k)) {
        return EngineFailure::fatal(format!("HTTPS 证书校验失败（{}）", f.msg_chain));
    }
    if let Some(status) = f.status {
        return EngineFailure::http(status, parse_retry_after_header(None));
    }
    if f.is_redirect {
        return EngineFailure::fatal("重定向次数超限".to_string());
    }
    if builder_bad_protocol(f.is_builder, f.msg_chain) {
        return EngineFailure::fatal("不支持的重定向协议".to_string());
    }
    EngineFailure::network(network_reason(&f))
}

#[must_use]
pub fn classify_reqwest(err: &reqwest::Error) -> EngineFailure {
    classify_features(FailureFeatures {
        msg_chain: &error_chain_text(err),
        status: err.status().map(|s| s.as_u16()),
        is_redirect: err.is_redirect(),
        is_builder: err.is_builder(),
        is_timeout: err.is_timeout(),
        is_connect: err.is_connect(),
        is_body_or_decode: err.is_body() || err.is_decode(),
    })
}

fn redirect_target_scheme(msg_chain: &str) -> Option<String> {
    let idx = msg_chain.find("url (")? + "url (".len();
    let rest = &msg_chain[idx..];
    let end = rest.find([':', ')'])?;
    let scheme = rest[..end].trim();
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
    {
        return None;
    }
    Some(scheme.to_ascii_lowercase())
}

#[must_use]
pub fn parse_retry_after_header(value: Option<&str>) -> Option<f64> {
    value
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|s| *s >= 0.0)
}

fn error_chain_text(err: &dyn std::error::Error) -> String {
    let mut msg = err.to_string();
    let mut src = err.source();
    while let Some(e) = src {
        msg.push_str(&format!(": {e}"));
        src = e.source();
    }
    if msg.len() > 160 {
        msg.truncate(160);
    }
    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_failure_classification_by_status() {
        let f = EngineFailure::http(404, None);
        assert_eq!(f.kind, FailKind::Fatal);
        assert!(f.reason.contains("404"));
        let f = EngineFailure::http(503, Some(10.0));
        assert_eq!(f.kind, FailKind::Transient);
        assert_eq!(f.retry_after, Some(10.0));
        let f = EngineFailure::http(408, None);
        assert_eq!(f.kind, FailKind::Transient);
    }

    #[test]
    fn retry_after_parsing() {
        assert_eq!(parse_retry_after_header(Some("10")), Some(10.0));
        assert_eq!(parse_retry_after_header(Some(" 90 ")), Some(90.0));
        assert_eq!(parse_retry_after_header(Some("abc")), None);
        assert_eq!(parse_retry_after_header(Some("-1")), None);
        assert_eq!(parse_retry_after_header(None), None);
    }

    #[test]
    fn status_text_common_codes() {
        assert!(status_text(404).contains("不存在"));
        assert!(status_text(503).contains("不可用"));
    }

    #[test]
    fn status_text_all_known_codes_and_fallback() {
        for (code, text) in STATUS_TEXTS {
            assert_eq!(status_text(code), text, "code {code}");
        }
        assert_eq!(status_text(599), "（服务器错误响应）");
        assert_eq!(status_text(200), "（服务器错误响应）");
    }

    fn features(chain: &str) -> FailureFeatures<'_> {
        FailureFeatures {
            msg_chain: chain,
            status: None,
            is_redirect: false,
            is_builder: false,
            is_timeout: false,
            is_connect: false,
            is_body_or_decode: false,
        }
    }

    #[test]
    fn classify_cert_keywords_each_hits_fatal() {
        for kw in CERT_KEYWORDS {
            let f = classify_features(features(&format!("error: {kw} detail")));
            assert_eq!(f.kind, FailKind::Fatal, "kw {kw}");
            assert!(f.reason.contains("证书校验失败"));
        }
    }

    #[test]
    fn classify_status_precedes_redirect_and_reasons() {
        let mut f = features("connection reset");
        f.status = Some(429);
        f.is_redirect = true;
        f.is_timeout = true;
        let out = classify_features(f);
        assert_eq!(out.kind, FailKind::Transient);
        assert!(out.reason.contains("HTTP 429"));
    }

    #[test]
    fn classify_redirect_limit_is_fatal() {
        let mut f = features("too many redirects");
        f.is_redirect = true;
        let out = classify_features(f);
        assert_eq!(out.kind, FailKind::Fatal);
        assert_eq!(out.reason, "重定向次数超限");
    }

    #[test]
    fn classify_builder_bad_protocol_and_http_fallthrough() {
        let mut ftp = features("builder error url (ftp://h/f)");
        ftp.is_builder = true;
        let out = classify_features(ftp);
        assert_eq!(out.kind, FailKind::Fatal);
        assert_eq!(out.reason, "不支持的重定向协议");

        let mut http = features("builder error url (http://h/f)");
        http.is_builder = true;
        http.is_connect = true;
        let out = classify_features(http);
        assert_eq!(out.kind, FailKind::Transient);
        assert!(out.reason.contains("连接失败"));
    }

    #[test]
    fn classify_network_reason_branches() {
        let mut t = features("deadline");
        t.is_timeout = true;
        assert_eq!(classify_features(t).reason, "连接超时");

        let mut c = features("dns error");
        c.is_connect = true;
        assert!(classify_features(c).reason.contains("连接失败"));

        let mut b = features("incomplete");
        b.is_body_or_decode = true;
        assert!(classify_features(b).reason.contains("传输中断"));

        let out = classify_features(features("weird"));
        assert!(out.reason.contains("网络错误"));
    }

    #[test]
    fn builder_bad_protocol_table() {
        assert!(!builder_bad_protocol(false, "url (ftp://h/)"));
        assert!(builder_bad_protocol(true, "url (ftp://h/)"));
        assert!(!builder_bad_protocol(true, "url (https://h/)"));
        assert!(!builder_bad_protocol(true, "no url here"));
    }

    #[test]
    fn redirect_target_scheme_edges() {
        assert_eq!(
            redirect_target_scheme("builder url (FTP://h/f)"),
            Some("ftp".to_string())
        );
        assert_eq!(redirect_target_scheme("url (:bad)"), None);
        assert_eq!(redirect_target_scheme("url ()"), None);
        assert_eq!(
            redirect_target_scheme("url (no-scheme-here)"),
            Some("no-scheme-here".to_string())
        );
        assert_eq!(redirect_target_scheme("url (has space)"), None);
        assert_eq!(redirect_target_scheme("nothing"), None);
    }

    #[test]
    fn network_and_fatal_constructors() {
        let f = EngineFailure::network("连接被拒绝");
        assert_eq!(f.kind, FailKind::Transient);
        assert_eq!(f.reason, "连接被拒绝");
        assert_eq!(f.retry_after, None);
        let f = EngineFailure::fatal("证书校验失败");
        assert_eq!(f.kind, FailKind::Fatal);
        assert_eq!(f.reason, "证书校验失败");
        assert_eq!(f.retry_after, None);
    }
}
