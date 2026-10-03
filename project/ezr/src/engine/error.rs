//! error — 失败分类与原因文案（FR-01-40：失败分类并在详情/toast 展示）
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
// 字节/速度/时间算术在 u64-f64 间转换是下载器领域固有；边界由调用方保证
#![allow(clippy::missing_const_for_fn)] // nursery 误报为主（含 trait impl 场景）
#![allow(clippy::doc_markdown, clippy::doc_lazy_continuation)] // 中文文档中英文术语不强制反引号
#![allow(clippy::float_cmp)] // 速度/时间为 0 的语义判断使用精确比较
#![allow(
    clippy::map_unwrap_or,
    clippy::option_if_let_else,
    clippy::unnested_or_patterns
)]
#![allow(clippy::cognitive_complexity, clippy::too_many_lines)] // 分块计算/状态机逻辑固有复杂度

use crate::model::FailKind;

/// 引擎失败：类别 + 面向用户的原因文案 + Retry-After 秒
#[derive(Debug, Clone)]
pub struct EngineFailure {
    /// 失败类别（决定自动重试行为）
    pub kind: FailKind,
    /// 失败原因（中文，展示用）
    pub reason: String,
    /// 服务器 Retry-After 秒（存在时优先于退避）
    pub retry_after: Option<f64>,
}

impl EngineFailure {
    /// 网络错误（连接失败/超时/重置）
    #[must_use]
    pub fn network(reason: impl Into<String>) -> EngineFailure {
        EngineFailure {
            kind: FailKind::Transient,
            reason: reason.into(),
            retry_after: None,
        }
    }

    /// HTTP 状态码失败（附状态码）
    #[must_use]
    pub fn http(status: u16, retry_after: Option<f64>) -> EngineFailure {
        EngineFailure {
            kind: crate::model::retry::classify_http_status(status),
            reason: format!("HTTP {status} {}", status_text(status)),
            retry_after,
        }
    }

    /// 语义性失败（停等：证书错误/磁盘不足/内容持续变化等）
    #[must_use]
    pub fn fatal(reason: impl Into<String>) -> EngineFailure {
        EngineFailure {
            kind: FailKind::Fatal,
            reason: reason.into(),
            retry_after: None,
        }
    }
}

/// 状态码短语表（常见码中文标注；未收录码显示 Standard 原因短语占位）
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

/// 状态码短语（常见码中文标注；未收录码显示 Standard 原因短语占位）
#[must_use]
pub fn status_text(status: u16) -> &'static str {
    STATUS_TEXTS
        .iter()
        .find(|(code, _)| *code == status)
        .map_or("（服务器错误响应）", |(_, text)| text)
}

/// 证书/TLS 关键字（错误链中出现任一即判证书失败，FR-01-15）
const CERT_KEYWORDS: [&str; 5] = [
    "certificate",
    "tls",
    "UnknownIssuer",
    "CertExpired",
    "NotValidFor",
];

/// 分类决策输入（从 `reqwest::Error` 抽取的可判定特征集合——
/// 决策核纯函数化后，单测无需构造真实 reqwest 错误对象）
struct FailureFeatures<'a> {
    msg_chain: &'a str,
    status: Option<u16>,
    is_redirect: bool,
    is_builder: bool,
    is_timeout: bool,
    is_connect: bool,
    is_body_or_decode: bool,
}

/// builder 错误且重定向目标为非 http(s) 协议 → 停等（FR-01-43）
fn builder_bad_protocol(is_builder: bool, msg_chain: &str) -> bool {
    is_builder && redirect_target_scheme(msg_chain).is_some_and(|s| s != "http" && s != "https")
}

/// 网络类失败的原因文案（timeout / connect / body·decode / 兼底）
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

/// 决策核：按既有优先序（证书 → 状态码 → 重定向 → builder 协议 → 网络原因）分类
fn classify_features(f: FailureFeatures<'_>) -> EngineFailure {
    // 证书/TLS 错误：语义不可恢复 → 停等（FR-01-15 不提供跳过校验开关）
    if CERT_KEYWORDS.iter().any(|k| f.msg_chain.contains(k)) {
        return EngineFailure::fatal(format!("HTTPS 证书校验失败（{}）", f.msg_chain));
    }
    if let Some(status) = f.status {
        return EngineFailure::http(status, parse_retry_after_header(None));
    }
    // 重定向跟随失败：超过次数上限（Policy::limited(10)，FR-01-14）→ 停等（FR-01-43 语义不可恢复）
    if f.is_redirect {
        return EngineFailure::fatal("重定向次数超限".to_string());
    }
    // 重定向至非 http(s) 协议（reqwest 以 builder error 呈现，消息含目标 URL）→ 停等
    if builder_bad_protocol(f.is_builder, f.msg_chain) {
        return EngineFailure::fatal("不支持的重定向协议".to_string());
    }
    EngineFailure::network(network_reason(&f))
}

/// 把 `reqwest::Error` 映射为 [`EngineFailure`]（FR-01-40 网络错误 / FR-01-15 证书错误）；
/// 分类决策见 [`classify_features`]（纯函数），本函数只做特征抽取
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

/// 从错误消息链中提取重定向目标 URL 的 scheme（形如 `url (ftp://…）`；无则 None）
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

/// 解析 `Retry-After` 头（秒数形式；HTTP-date 形式不支持→None。R2：无 60s 上限）
#[must_use]
pub fn parse_retry_after_header(value: Option<&str>) -> Option<f64> {
    value
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|s| *s >= 0.0)
}

/// 错误链拼接（reqwest 错误的 source 链，截断到 160 字符）
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
        assert_eq!(out.kind, FailKind::Transient); // 429 → Transient（状态码优先）
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

        // builder 但目标是 http(s) → 不触发停等，落到网络原因
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
        // 截断边界：到首个 ':' 或 ')' 为止；字符集合法即 Some（大小写归一）
        assert_eq!(redirect_target_scheme("url (:bad)"), None);
        assert_eq!(redirect_target_scheme("url ()"), None);
        assert_eq!(
            redirect_target_scheme("url (no-scheme-here)"),
            Some("no-scheme-here".to_string())
        );
        assert_eq!(redirect_target_scheme("url (has space)"), None);
        assert_eq!(redirect_target_scheme("nothing"), None);
    }

    /// network / fatal 构造器：类别与 Retry-After 语义（无服务器指示时恒 None）
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
