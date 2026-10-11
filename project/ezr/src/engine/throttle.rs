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
pub const BURST_SECS: f64 = 0.5;
/// 单次睡眠粒度（毫秒）
pub const SLEEP_MS: u64 = 20;

/// 令牌桶单步决策（纯函数，architect v134 自 `acquire` 内联数学逐行等价提取：
/// 限速策略与「读时钟 + 睡眠」IO 外壳分离，账本数学可无头单测与属性测试）。
/// 输入：速率（> 0，unlimited 由 `acquire` 提前短路，见属性测试生成域注记）、
/// 桶内余量、自上次决策起流逝秒数、本次请求字节。行为：先补币并钳制到突发
/// 容量（`tokens + elapsed*rate` 上取 `rate*BURST_SECS`），再取走
/// `min(remaining, 补币后可用)`。返回 `(取走后桶余量, 本次取走量, 等待毫秒)`：
/// 取足时等待为 0（收束），未取足为 `deficit/rate` 换算毫秒且不低于
/// [`SLEEP_MS`]（防忙轮询）。`pub` 于私有模块内（挂载测试 crate 经门面
/// re-export 消费，notes/rust.md「门面 re-export」；engine 外不可达）。
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
    /// `n` 可大于桶容量（分批取走，逐窗等待）。决策数学收口于
    /// [`bucket_step`]（纯函数），本函数只保留「读时钟 + 睡眠」IO 外壳。
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

    /// bucket_step 无头单测（architect v134 策略提取的决策锚点：确定性输入
    /// 逐步核对补币钳制、取走、等待换算——时序类测试只锁端到端时长，
    /// 账本数学由本测试 + 属性测试锁定）
    #[test]
    fn bucket_step_headless_decisions() {
        // 空桶 + 零流逝：不补币，取走预存突发的一半
        let (tokens, taken, wait) = bucket_step(1_000.0, 0.0, 0.0, 500.0);
        assert_eq!(taken, 0.0, "零流逝空桶无币可取");
        assert_eq!(tokens, 0.0);
        assert!(wait > 0, "未取足应给出等待");
        // 预存突发（new 语义：tokens = rate * 0.5）：半量请求即时满足
        let (tokens, taken, wait) = bucket_step(1_000.0, 500.0, 0.0, 250.0);
        assert_eq!(taken, 250.0);
        assert_eq!(tokens, 250.0);
        assert_eq!(wait, 0, "取足收束");
        // 补币钳制：超量流逝不越过突发容量
        let (tokens, taken, _) = bucket_step(1_000.0, 400.0, 100.0, 0.0);
        assert_eq!(tokens, 500.0, "钳制到 rate*BURST_SECS");
        assert_eq!(taken, 0.0, "remaining=0 不取");
        // 大请求分批：首窗取走全部可用并给 deficit 等待
        let (tokens, taken, wait) = bucket_step(2_000.0, 1_000.0, 0.0, 4_000.0);
        assert_eq!(taken, 1_000.0);
        assert_eq!(tokens, 0.0);
        assert_eq!(wait, 1_500, "deficit 3000B @ 2000B/s = 1500ms");
        // 等待下限：deficit 换算低于 SLEEP_MS 时钳到 SLEEP_MS
        let (_, _, wait) = bucket_step(1_000_000.0, 0.0, 0.0, 1.0);
        assert_eq!(wait, SLEEP_MS, "防忙轮询下限");
    }
}
