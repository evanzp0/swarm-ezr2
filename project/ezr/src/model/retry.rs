//! retry — 重试策略（FR-01-41 连续性规则 / FR-01-42 指数退避 / FR-01-43 可重试性分类）
//!
//! 定案口径：
//! - 重试计数连续性：失败时本次尝试**有下载进展** → 计数重置为 1；
//!   **无进展**（连接卡死即失败）→ 计数累加 +1；达上限停止自动重试；
//! - 指数退避：8s → 16s → 32s → 60s 封顶；响应带 `Retry-After` 时优先采用
//!   （R2 修订：即使 >60s 也采用）；
//! - 可重试性：408/429/5xx/网络错误/文件大小不符 → 自动重试；
//!   4xx（除 408/429）/完整性校验失败/磁盘空间不足/内容持续变化 → 停等。
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_wrap)]
// 字节/速度/时间算术在 u64-f64 间转换是下载器领域固有；边界由调用方保证
#![allow(clippy::missing_const_for_fn)] // nursery 误报为主（含 trait impl 场景）
#![allow(clippy::doc_markdown, clippy::doc_lazy_continuation)] // 中文文档中英文术语不强制反引号
#![allow(clippy::float_cmp)] // 速度/时间为 0 的语义判断使用精确比较
#![allow(clippy::map_unwrap_or, clippy::option_if_let_else, clippy::unnested_or_patterns)]
#![allow(clippy::cognitive_complexity, clippy::too_many_lines)] // 分块计算/状态机逻辑固有复杂度


use super::FailKind;

/// 指数退避序列（FR-01-42）：按已计重试次数取倒计时，`retries ≥ 4` 封顶 60s
#[must_use]
pub fn backoff_secs(retries: u32) -> f64 {
    match retries {
        1 => 8.0,
        2 => 16.0,
        3 => 32.0,
        _ => 60.0,
    }
}

/// HTTP 状态码 → 失败类别（FR-01-43）。
/// 408/429/5xx 可自动重试；其余 4xx 语义性错误停等；1xx/3xx 不构成失败。
#[must_use]
pub fn classify_http_status(status: u16) -> FailKind {
    match status {
        408 | 429 => FailKind::Transient,
        s if (500..600).contains(&s) => FailKind::Transient,
        _ => FailKind::Fatal,
    }
}

/// 一次失败的重试决策结果
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetryDecision {
    /// 失败类别
    pub kind: FailKind,
    /// 是否自动重试（Transient 且未达上限且 `auto_retry` 开启）
    pub auto: bool,
    /// 自动重试倒计时秒（`auto = true` 时有效；`Retry-After` 优先于退避）
    pub delay_secs: f64,
    /// 已达上限（显示「已达上限」，释放槽位）
    pub at_limit: bool,
}

/// 失败时的重试决策（FR-01-41/42/43 汇合点）。
///
/// * `made_progress` —— 本次尝试是否下载到过数据（连续性规则）；
/// * `retries` —— 累计失败前的重试计数值（失败时按规则更新后参与判定）；
/// * `max_retries` —— 上限（默认 5）；
/// * `retry_after` —— 服务器 `Retry-After` 秒（存在即优先，无 60s 上限，R2）；
/// * `auto_retry` —— 配置开关（`max_retries` 同为配置项）。
#[must_use]
pub fn decide(
    kind: FailKind,
    made_progress: bool,
    retries: u32,
    max_retries: u32,
    retry_after: Option<f64>,
    auto_retry: bool,
) -> (u32, RetryDecision) {
    // 连续性规则：有进展重置为 1，无进展累加 +1（FR-01-41）
    let new_retries = if made_progress { 1 } else { retries + 1 };
    match kind {
        FailKind::Transient => {
            if new_retries >= max_retries {
                (
                    new_retries,
                    RetryDecision { kind, auto: false, delay_secs: 0.0, at_limit: true },
                )
            } else {
                let secs = retry_after.unwrap_or_else(|| backoff_secs(new_retries));
                (
                    new_retries,
                    RetryDecision { kind, auto: auto_retry, delay_secs: secs, at_limit: false },
                )
            }
        }
        // 4xx / 校验失败 / 磁盘不足 / 内容持续变化：不自动重试，停等（可 R）
        FailKind::Fatal | FailKind::Verify => (
            new_retries,
            RetryDecision { kind, auto: false, delay_secs: 0.0, at_limit: false },
        ),
    }
}

/// 一致性失效（FR-01-22）的独立计数推进：不计失败、不占重试计数；
/// 连续 3 次失效转「已失败（服务器内容持续变化）」停等。
/// 返回 (新的失效连击数, 是否转为停等失败)。
#[must_use]
pub fn invalidate_streak(streak: u32) -> (u32, bool) {
    let s = streak + 1;
    (s, s >= 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_sequence_8_16_32_60_cap() {
        assert_eq!(backoff_secs(1), 8.0);
        assert_eq!(backoff_secs(2), 16.0);
        assert_eq!(backoff_secs(3), 32.0);
        assert_eq!(backoff_secs(4), 60.0);
        assert_eq!(backoff_secs(9), 60.0);
    }

    #[test]
    fn http_status_classification() {
        assert_eq!(classify_http_status(403), FailKind::Fatal);
        assert_eq!(classify_http_status(404), FailKind::Fatal);
        assert_eq!(classify_http_status(408), FailKind::Transient);
        assert_eq!(classify_http_status(429), FailKind::Transient);
        assert_eq!(classify_http_status(500), FailKind::Transient);
        assert_eq!(classify_http_status(503), FailKind::Transient);
        assert_eq!(classify_http_status(418), FailKind::Fatal);
    }

    #[test]
    fn no_progress_increments() {
        let (r, d) = decide(FailKind::Transient, false, 1, 5, None, true);
        assert_eq!(r, 2);
        assert!(d.auto);
        assert_eq!(d.delay_secs, 16.0);
    }

    #[test]
    fn progress_resets_to_one() {
        let (r, d) = decide(FailKind::Transient, true, 4, 5, None, true);
        assert_eq!(r, 1);
        assert!(d.auto);
        assert_eq!(d.delay_secs, 8.0);
    }

    #[test]
    fn reaches_limit_stops_auto() {
        let (r, d) = decide(FailKind::Transient, false, 4, 5, None, true);
        assert_eq!(r, 5);
        assert!(!d.auto);
        assert!(d.at_limit);
    }

    #[test]
    fn retry_after_overrides_backoff_without_cap() {
        // R2 修订：Retry-After 即使 >60s 也优先采用
        let (_, d) = decide(FailKind::Transient, false, 1, 5, Some(90.0), true);
        assert_eq!(d.delay_secs, 90.0);
        let (_, d) = decide(FailKind::Transient, true, 3, 5, Some(5.0), true);
        assert_eq!(d.delay_secs, 5.0);
    }

    #[test]
    fn auto_retry_disabled_config() {
        let (_, d) = decide(FailKind::Transient, true, 0, 5, None, false);
        assert!(!d.auto);
        assert!(!d.at_limit);
    }

    #[test]
    fn fatal_and_verify_never_auto() {
        let (r, d) = decide(FailKind::Fatal, true, 2, 5, Some(10.0), true);
        assert_eq!(r, 1); // Fatal 也按连续性规则更新计数
        assert!(!d.auto);
        assert!(!d.at_limit);
        let (_, d) = decide(FailKind::Verify, false, 0, 5, None, true);
        assert!(!d.auto);
    }

    #[test]
    fn invalidation_streak_trips_at_three() {
        assert_eq!(invalidate_streak(0), (1, false));
        assert_eq!(invalidate_streak(1), (2, false));
        assert_eq!(invalidate_streak(2), (3, true));
    }
}
