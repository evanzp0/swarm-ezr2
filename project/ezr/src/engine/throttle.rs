use std::sync::Mutex;
use std::time::Instant;

pub struct Throttle {
    rate: Mutex<f64>,
    tokens: Mutex<f64>,
    last: Mutex<Instant>,
}

pub const BURST_SECS: f64 = 0.5;
pub const SLEEP_MS: u64 = 20;

pub fn bucket_step(rate: f64, tokens: f64, elapsed_secs: f64, remaining: f64) -> (f64, f64, u64) {
    let clamped = (tokens + elapsed_secs * rate).min(rate * BURST_SECS);
    let take = remaining.min(clamped);
    let tokens = clamped - take;
    let left = remaining - take;
    let wait_ms = if left <= 0.0 {
        0
    } else {
        (left / rate * 1000.0).max(SLEEP_MS as f64) as u64
    };
    (tokens, take, wait_ms)
}

impl Throttle {
    #[must_use]
    pub fn new(rate_bps: u64) -> Self {
        Throttle {
            rate: Mutex::new(rate_bps as f64),
            tokens: Mutex::new(if rate_bps == 0 {
                f64::INFINITY
            } else {
                rate_bps as f64 * BURST_SECS
            }),
            last: Mutex::new(Instant::now()),
        }
    }

    pub async fn acquire(&self, n: u64) {
        let mut remaining = n as f64;
        while remaining > 0.0 {
            let wait = {
                let rate = *self.rate.lock().unwrap();
                if rate <= 0.0 {
                    return;
                }
                let mut tokens = self.tokens.lock().unwrap();
                let mut last = self.last.lock().unwrap();
                let now = Instant::now();
                let elapsed = now.duration_since(*last).as_secs_f64();
                *last = now;
                let (tokens_after, taken, wait) = bucket_step(rate, *tokens, elapsed, remaining);
                *tokens = tokens_after;
                remaining -= taken;
                wait
            };
            if wait > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(wait.min(200))).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unlimited_is_instant() {
        let t = Throttle::new(0);
        let start = std::time::Instant::now();
        t.acquire(10_000_000).await;
        assert!(start.elapsed().as_millis() < 50);
    }

    #[tokio::test]
    async fn limited_throttles_bulk() {
        let t = Throttle::new(1_000_000);
        let start = std::time::Instant::now();
        t.acquire(2_000_000).await;
        let took = start.elapsed().as_secs_f64();
        assert!(took > 1.0, "took={took}");
        assert!(took < 4.0, "took={took}");
    }

    #[tokio::test]
    async fn small_acquire_within_burst_is_fast() {
        let t = Throttle::new(10_000_000);
        let start = std::time::Instant::now();
        t.acquire(100_000).await;
        assert!(start.elapsed().as_millis() < 100);
    }

    #[test]
    fn bucket_step_headless_decisions() {
        let (tokens, taken, wait) = bucket_step(1_000.0, 0.0, 0.0, 500.0);
        assert_eq!(taken, 0.0, "零流逝空桶无币可取");
        assert_eq!(tokens, 0.0);
        assert!(wait > 0, "未取足应给出等待");
        let (tokens, taken, wait) = bucket_step(1_000.0, 500.0, 0.0, 250.0);
        assert_eq!(taken, 250.0);
        assert_eq!(tokens, 250.0);
        assert_eq!(wait, 0, "取足收束");
        let (tokens, taken, _) = bucket_step(1_000.0, 400.0, 100.0, 0.0);
        assert_eq!(tokens, 500.0, "钳制到 rate*BURST_SECS");
        assert_eq!(taken, 0.0, "remaining=0 不取");
        let (tokens, taken, wait) = bucket_step(2_000.0, 1_000.0, 0.0, 4_000.0);
        assert_eq!(taken, 1_000.0);
        assert_eq!(tokens, 0.0);
        assert_eq!(wait, 1_500, "deficit 3000B @ 2000B/s = 1500ms");
        let (_, _, wait) = bucket_step(1_000_000.0, 0.0, 0.0, 1.0);
        assert_eq!(wait, SLEEP_MS, "防忙轮询下限");
    }
}
