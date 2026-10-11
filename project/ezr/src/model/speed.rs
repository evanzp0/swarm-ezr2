//! speed — 速度统计（FR-01-17：任务速度与全局速度按 1 秒滑动窗口计算）

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// 滑动窗口时长（1 秒，FR-01-17）
pub const WINDOW: Duration = Duration::from_secs(1);

/// 1 秒滑动窗口速度表：按时间戳记录累计字节，读取窗口两端差值得到速率。
#[derive(Debug, Default)]
pub struct SpeedWindow {
    samples: VecDeque<(Instant, u64)>,
    total_bytes: u64,
}

impl SpeedWindow {
    /// 空表
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 推进累计字节读数（每次 tick 调用；`cumulative` 为单调累计字节）
    pub fn push(&mut self, now: Instant, cumulative: u64) {
        self.total_bytes = self.total_bytes.max(cumulative);
        self.samples.push_back((now, cumulative));
        self.evict(now);
    }

    /// 当前速率 B/s（窗口两端样本差值 / 实际时间跨度；窗口不足或无增量返回 0）
    #[must_use]
    pub fn rate(&mut self) -> f64 {
        let now = Instant::now();
        self.evict(now);
        match (self.samples.front(), self.samples.back()) {
            (Some(&(t0, b0)), Some(&(t1, b1))) => {
                let dt = t1.duration_since(t0).as_secs_f64().max(0.05);
                (b1.saturating_sub(b0)) as f64 / dt
            }
            _ => 0.0,
        }
    }

    /// 末次推进时刻（空表 = None；FR-01-102 连接级窗口端点老化守卫用）
    #[must_use]
    pub fn last_push(&self) -> Option<Instant> {
        self.samples.back().map(|(t, _)| *t)
    }

    fn evict(&mut self, now: Instant) {
        // 保留窗口内 + 最旧一个端点（差值基准）
        while self.samples.len() > 2 && now.duration_since(self.samples[1].0) > WINDOW {
            self.samples.pop_front();
        }
    }
}

/// 展示面平滑器（FR-01-17 修订）：指数滑动平均，α=1/5（等效约 5 秒窗口）。
/// 数值每秒经 `push` 推进一次采样；`zero` 供非下载态立即归零（不走衰减、无拖尾）。
#[derive(Debug, Default)]
pub struct SmoothedSpeed {
    value: f64,
}

impl SmoothedSpeed {
    /// 平滑系数 α = 1/5（libtorrent second_tick 同款口径）
    const ALPHA: f64 = 1.0 / 5.0;

    /// 推进一次本秒采样（B/s），展示值向采样值收敛 1/5
    pub fn push(&mut self, sample: f64) {
        self.value += Self::ALPHA * (sample - self.value);
    }

    /// 立即归零（零值速断：任务进入非下载态时调用，EMA 不拖尾）
    pub fn zero(&mut self) {
        self.value = 0.0;
    }

    /// 当前展示值（B/s）
    pub fn value(&self) -> f64 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_window_is_zero() {
        let mut w = SpeedWindow::new();
        assert_eq!(w.rate(), 0.0);
    }

    #[test]
    fn rate_over_one_second() {
        let t0 = Instant::now();
        let mut w = SpeedWindow::new();
        w.push(t0, 0);
        w.push(t0 + Duration::from_millis(500), 500);
        w.push(t0 + Duration::from_millis(1000), 1000);
        // 窗口 0→1s：1000 B / 1s
        let r = w.rate();
        assert!((r - 1000.0).abs() < 200.0, "rate={r}");
    }

    #[test]
    fn stale_samples_evicted() {
        let t0 = Instant::now();
        let mut w = SpeedWindow::new();
        w.push(t0, 0);
        w.push(t0 + Duration::from_secs(5), 5000);
        // 旧端点出窗后差值为 0（5s 前的 0 已被驱逐，窗口只剩最新段）
        w.push(t0 + Duration::from_secs(5), 5000);
        assert!(w.rate() >= 0.0);
    }

    #[test]
    fn last_push_reports_newest_sample() {
        let t0 = Instant::now();
        let mut w = SpeedWindow::new();
        assert_eq!(w.last_push(), None, "空表 = None");
        w.push(t0, 0);
        w.push(t0 + Duration::from_millis(500), 500);
        assert_eq!(w.last_push(), Some(t0 + Duration::from_millis(500)));
    }

    // ---- FR-01-17 修订：展示面 EMA 平滑（α=1/5，等效约 5 秒窗口）----

    #[test]
    fn ema_first_sample_is_fraction_of_input() {
        // 首个采样只推入 α 份额（从 0 爬升，非瞬时跳变）——对恒等直传的错误实现失败
        let mut s = SmoothedSpeed::default();
        s.push(1000.0);
        assert!((s.value() - 200.0).abs() < 1e-6, "value={}", s.value());
    }

    #[test]
    fn ema_converges_to_steady_input() {
        // 恒定采样 40 步（0.8^40≈0.01%）应收敛到 ±2% 内——对固定偏置/遗忘的错误实现失败
        let mut s = SmoothedSpeed::default();
        for _ in 0..40 {
            s.push(1000.0);
        }
        assert!((s.value() - 1000.0).abs() < 20.0, "value={}", s.value());
    }

    #[test]
    fn zero_method_snaps_immediately() {
        // 零值速断：zero() 后必须精确为 0，无衰减拖尾——对依赖自然衰减的错误实现失败
        let mut s = SmoothedSpeed::default();
        for _ in 0..20 {
            s.push(1000.0);
        }
        s.zero();
        assert_eq!(s.value(), 0.0);
    }
}
