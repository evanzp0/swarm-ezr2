#![allow(missing_docs)]
#[path = "../../src/model/speed.rs"]
mod speed;

use std::time::{Duration, Instant};

use speed::{SpeedWindow, WINDOW};

fn future_base() -> Instant {
    Instant::now() + Duration::from_secs(3600)
}

fn at(base: Instant, millis: u64) -> Instant {
    base + Duration::from_millis(millis)
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-4
}

#[test]
fn evict_prunes_stale_front_rate_tracks_recent_window() {
    let base = future_base();
    let mut w = SpeedWindow::new();
    w.push(at(base, 0), 0);
    for i in 1..=20 {
        w.push(at(base, 500 * i), 500 * i);
    }
    for i in 21..=30 {
        w.push(at(base, 500 * i), 10_000 + 50 * (i - 20));
    }
    let r = w.rate();
    assert!(close(r, 100.0), "rate={r}（不剪枝/长度卫失守时≈700）");
}

#[test]
fn rate_divides_by_time_span() {
    let base = future_base();
    let mut w = SpeedWindow::new();
    w.push(at(base, 0), 0);
    w.push(at(base, 500), 500);
    let r = w.rate();
    assert!(close(r, 1000.0), "rate={r}（乘法变体=250）");
}

#[test]
fn boundary_sample_exactly_one_window_old_is_retained() {
    assert_eq!(WINDOW, Duration::from_secs(1));
    let base = future_base();
    let mut w = SpeedWindow::new();
    w.push(at(base, 0), 0);
    w.push(at(base, 1000), 1000);
    w.push(at(base, 2000), 1000);
    w.push(at(base, 3000), 5000);
    let r = w.rate();
    assert!(
        close(r, 2000.0),
        "rate={r}（>= 变体=4000；==/< 变体≈1666.7）"
    );
}

#[test]
fn young_interior_samples_are_not_evicted() {
    let base = future_base();
    let mut w = SpeedWindow::new();
    w.push(at(base, 0), 0);
    w.push(at(base, 250), 1000);
    w.push(at(base, 500), 1500);
    let r = w.rate();
    assert!(
        close(r, 3000.0),
        "rate={r}（< 变体驱逐年轻样本 → 2000 或 0）"
    );
}

#[test]
fn stale_pair_rate_preserved_with_two_samples() {
    let t0 = Instant::now();
    let mut w = SpeedWindow::new();
    w.push(t0, 0);
    w.push(t0 + Duration::from_millis(100), 100);
    std::thread::sleep(Duration::from_millis(1200));
    let r = w.rate();
    assert!(close(r, 1000.0), "rate={r}（>= 2 长度卫驱逐基准 → 0）");
}
