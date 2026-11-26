use super::FailKind;

#[must_use]
pub fn backoff_secs(retries: u32) -> f64 {
    match retries {
        1 => 8.0,
        2 => 16.0,
        3 => 32.0,
        _ => 60.0,
    }
}

#[must_use]
pub fn classify_http_status(status: u16) -> FailKind {
    match status {
        408 | 429 => FailKind::Transient,
        s if (500..600).contains(&s) => FailKind::Transient,
        _ => FailKind::Fatal,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetryDecision {
    pub kind: FailKind,
    pub auto: bool,
    pub delay_secs: f64,
    pub at_limit: bool,
}

#[must_use]
pub fn decide(
    kind: FailKind,
    made_progress: bool,
    retries: u32,
    max_retries: u32,
    retry_after: Option<f64>,
    auto_retry: bool,
) -> (u32, RetryDecision) {
    let new_retries = if made_progress { 1 } else { retries + 1 };
    match kind {
        FailKind::Transient => {
            if new_retries >= max_retries {
                (
                    new_retries,
                    RetryDecision {
                        kind,
                        auto: false,
                        delay_secs: 0.0,
                        at_limit: true,
                    },
                )
            } else {
                let secs = retry_after.unwrap_or_else(|| backoff_secs(new_retries));
                (
                    new_retries,
                    RetryDecision {
                        kind,
                        auto: auto_retry,
                        delay_secs: secs,
                        at_limit: false,
                    },
                )
            }
        }
        FailKind::Fatal | FailKind::Verify => (
            new_retries,
            RetryDecision {
                kind,
                auto: false,
                delay_secs: 0.0,
                at_limit: false,
            },
        ),
    }
}

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
        assert_eq!(r, 1);
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
