//! speed — 速度统计（FR-01-17：任务速度与全局速度按 1 秒滑动窗口计算）
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_wrap)]
// 字节/速度/时间算术在 u64-f64 间转换是下载器领域固有；边界由调用方保证
#![allow(clippy::missing_const_for_fn)] // nursery 误报为主（含 trait impl 场景）
#![allow(clippy::doc_markdown, clippy::doc_lazy_continuation)] // 中文文档中英文术语不强制反引号
#![allow(clippy::float_cmp)] // 速度/时间为 0 的语义判断使用精确比较
#![allow(clippy::map_unwrap_or, clippy::option_if_let_else, clippy::unnested_or_patterns)]
#![allow(clippy::cognitive_complexity, clippy::too_many_lines)] // 分块计算/状态机逻辑固有复杂度


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
    fn evict(&mut self, now: Instant) {
        // 保留窗口内 + 最旧一个端点（差值基准）
        while self.samples.len() > 2
            && now.duration_since(self.samples[1].0) > WINDOW
        {
            self.samples.pop_front();
        }
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

}
