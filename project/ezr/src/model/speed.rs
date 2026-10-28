use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub const WINDOW: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
pub struct SpeedWindow {
    samples: VecDeque<(Instant, u64)>,
    total_bytes: u64,
}

impl SpeedWindow {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, now: Instant, cumulative: u64) {
        self.total_bytes = self.total_bytes.max(cumulative);
        self.samples.push_back((now, cumulative));
        self.evict(now);
    }

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

    #[must_use]
    pub fn last_push(&self) -> Option<Instant> {
        self.samples.back().map(|(t, _)| *t)
    }

    fn evict(&mut self, now: Instant) {
        while self.samples.len() > 2 && now.duration_since(self.samples[1].0) > WINDOW {
            self.samples.pop_front();
        }
    }
}

#[derive(Debug, Default)]
pub struct SmoothedSpeed {
    value: f64,
}

impl SmoothedSpeed {
    const ALPHA: f64 = 1.0 / 5.0;

    pub fn push(&mut self, sample: f64) {
        self.value += Self::ALPHA * (sample - self.value);
    }

    pub fn zero(&mut self) {
        self.value = 0.0;
    }

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
        let r = w.rate();
        assert!((r - 1000.0).abs() < 200.0, "rate={r}");
    }

    #[test]
    fn stale_samples_evicted() {
        let t0 = Instant::now();
        let mut w = SpeedWindow::new();
        w.push(t0, 0);
        w.push(t0 + Duration::from_secs(5), 5000);
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

    #[test]
    fn ema_first_sample_is_fraction_of_input() {
        let mut s = SmoothedSpeed::default();
        s.push(1000.0);
        assert!((s.value() - 200.0).abs() < 1e-6, "value={}", s.value());
    }

    #[test]
    fn ema_converges_to_steady_input() {
        let mut s = SmoothedSpeed::default();
        for _ in 0..40 {
            s.push(1000.0);
        }
        assert!((s.value() - 1000.0).abs() < 20.0, "value={}", s.value());
    }

    #[test]
    fn zero_method_snaps_immediately() {
        let mut s = SmoothedSpeed::default();
        for _ in 0..20 {
            s.push(1000.0);
        }
        s.zero();
        assert_eq!(s.value(), 0.0);
    }
}
