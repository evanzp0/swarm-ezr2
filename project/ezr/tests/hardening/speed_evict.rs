#![allow(missing_docs)]
#[path = "../../src/model/speed.rs"]
mod speed;

use std::thread::sleep;
use std::time::{Duration, Instant};

use speed::{SpeedWindow, WINDOW};

#[test]
fn evict_keeps_two_sample_baseline_when_len_eq_two() {
    let base = Instant::now();
    let mut w = SpeedWindow::new();
    w.push(base, 0);
    w.push(base, 100);
    sleep(WINDOW + Duration::from_millis(200));
    let r = w.rate();
    assert!(
        r > 0.0,
        "len==2 时必须保留差分基准（mutant >= 2 提前剪枝 → rate 退化为 0）；实际 {r}"
    );
}
