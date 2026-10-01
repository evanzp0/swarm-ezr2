//! error — 失败分类与原因文案（FR-01-40：失败分类并在详情/toast 展示）
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_wrap)]
// 字节/速度/时间算术在 u64-f64 间转换是下载器领域固有；边界由调用方保证
#![allow(clippy::missing_const_for_fn)] // nursery 误报为主（含 trait impl 场景）
#![allow(clippy::doc_markdown, clippy::doc_lazy_continuation)] // 中文文档中英文术语不强制反引号
#![allow(clippy::float_cmp)] // 速度/时间为 0 的语义判断使用精确比较
#![allow(clippy::map_unwrap_or, clippy::option_if_let_else, clippy::unnested_or_patterns)]
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
        EngineFailure { kind: FailKind::Transient, reason: reason.into(), retry_after: None }
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
        EngineFailure { kind: FailKind::Fatal, reason: reason.into(), retry_after: None }
    }
}

/// 状态码短语（常见码中文标注；未收录码显示 Standard 原因短语占位）
#[must_use]
pub fn status_text(status: u16) -> &'static str {
    match status {
        400 => "Bad Request（请求非法）",
        401 => "Unauthorized（需要认证）",
        403 => "Forbidden（拒绝访问）",
        404 => "Not Found（资源不存在）",
        408 => "Request Timeout（请求超时）",
        410 => "Gone（资源已移除）",
        416 => "Range Not Satisfiable（区间不可满足）",
        429 => "Too Many Requests（请求过多）",
        500 => "Internal Server Error（服务器内部错误）",
        502 => "Bad Gateway（网关错误）",
        503 => "Service Unavailable（服务不可用）",
        504 => "Gateway Timeout（网关超时）",
        _ => "（服务器错误响应）",
    }
}

/// 把 `reqwest::Error` 映射为 [`EngineFailure`]（FR-01-40 网络错误 / FR-01-15 证书错误）
#[must_use]
pub fn classify_reqwest(err: &reqwest::Error) -> EngineFailure {
    // 证书/TLS 错误：语义不可恢复 → 停等（FR-01-15 不提供跳过校验开关）
    let msg_chain = error_chain_text(err);
    if msg_chain.contains("certificate")
        || msg_chain.contains("tls")
        || msg_chain.contains("UnknownIssuer")
        || msg_chain.contains("CertExpired")
        || msg_chain.contains("NotValidFor")
    {
        return EngineFailure::fatal(format!(
            "HTTPS 证书校验失败（{msg_chain}）"
        ));
    }
    if let Some(status) = err.status() {
        return EngineFailure::http(status.as_u16(), parse_retry_after_header(None));
    }
    // 重定向跟随失败：超过次数上限（Policy::limited(10)，FR-01-14）→ 停等（FR-01-43 语义不可恢复）
    if err.is_redirect() {
        return EngineFailure::fatal("重定向次数超限".to_string());
    }
    // 重定向至非 http(s) 协议（reqwest 以 builder error 呈现，消息含目标 URL）→ 停等
    if err.is_builder() {
        if let Some(scheme) = redirect_target_scheme(&msg_chain) {
            if scheme != "http" && scheme != "https" {
                return EngineFailure::fatal("不支持的重定向协议".to_string());
            }
        }
    }
    let reason = if err.is_timeout() {
        "连接超时".to_string()
    } else if err.is_connect() {
        format!("连接失败（{msg_chain}）")
    } else if err.is_body() || err.is_decode() {
        format!("传输中断（{msg_chain}）")
    } else {
        format!("网络错误（{msg_chain}）")
    };
    EngineFailure::network(reason)
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
    value.and_then(|v| v.trim().parse::<f64>().ok()).filter(|s| *s >= 0.0)
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
}
