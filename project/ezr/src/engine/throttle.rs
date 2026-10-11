//! throttle — 全局限速令牌桶（FR-01-60：生效范围为全部下载任务合计速度）

use std::sync::Mutex;
use std::time::Instant;

/// 全局限速器：令牌桶（1 token = 1 字节；桶容量 = 速率 × 突发窗口）。
/// `rate = 0` 表示不限速（acquire 直通）。
pub struct Throttle {
    /// 限速 B/s（0 = 不限）
    rate: Mutex<f64>,
    /// 当前可用令牌（字节）
    tokens: Mutex<f64>,
    /// 上次补充时刻
    last: Mutex<Instant>,
}

/// 桶容量秒数（突发窗口；过大导致低速时段粒度差）
const BURST_SECS: f64 = 0.5;
/// 单次睡眠粒度（毫秒）
const SLEEP_MS: u64 = 20;

impl Throttle {
    /// 构建限速器（`rate_bps` = 0 表示不限速）
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

    /// 获取 `n` 字节配额（阻塞直到配额全部就绪；不限速立即返回）。
    /// `n` 可大于桶容量（分批取走，逐窗等待）。
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
                *tokens = (*tokens + elapsed * rate).min(rate * BURST_SECS);
                let take = remaining.min(*tokens);
                if take > 0.0 {
                    *tokens -= take;
                    remaining -= take;
                }
                if remaining <= 0.0 {
                    0
                } else {
                    let deficit = remaining;
                    (deficit / rate * 1000.0).max(SLEEP_MS as f64) as u64
                }
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
        let t = Throttle::new(1_000_000); // 1 MB/s
        let start = std::time::Instant::now();
        // 2 MB @ 1 MB/s ≈ 2s（突发窗口 0.5s 已预存）
        t.acquire(2_000_000).await;
        let took = start.elapsed().as_secs_f64();
        assert!(took > 1.0, "took={took}");
        assert!(took < 4.0, "took={took}");
    }

    #[tokio::test]
    async fn small_acquire_within_burst_is_fast() {
        let t = Throttle::new(10_000_000);
        let start = std::time::Instant::now();
        t.acquire(100_000).await; // 远小于突发容量
        assert!(start.elapsed().as_millis() < 100);
    }
}
