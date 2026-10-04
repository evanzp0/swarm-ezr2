//! 加固测试：`SpeedWindow::evict` 长度卫（src/model/speed.rs 变异幸存体杀灭）。
//! 靶点（放量轮 speed.rs 幸存 1）：
//! - `61:34` `samples.len() > 2` → `>= 2`：恰好两个样本、末样本真实过期（> 1s 窗口）
//!   时，正确行为保留两端差分基准（rate 按两样本差值），mutant 提前剪枝只剩一个
//!   样本 → rate 退化为 0。
//! 确定性手法：两个样本同刻合成（dt=0 → rate 上限 0.05s 钳制 = 2000 B/s 读数），
//! 真实时钟只用于 rate() 内部的 evict 剪枝判断（短 sleep 越过 WINDOW）。

#![allow(missing_docs)]
#[path = "../../src/model/speed.rs"]
mod speed;

use speed::{SpeedWindow, WINDOW};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// 靶 `61:34`：len==2 且末样本真实过期时不得剪枝（mutant `>= 2` 剪到 1 个样本 → rate=0）。
#[test]
fn evict_keeps_two_sample_baseline_when_len_eq_two() {
    let base = Instant::now();
    let mut w = SpeedWindow::new();
    w.push(base, 0);
    w.push(base, 100); // 同刻第二样本：len=2，samples[1]=base
    sleep(WINDOW + Duration::from_millis(200)); // 真实时钟越过窗口
    let r = w.rate();
    assert!(
        r > 0.0,
        "len==2 时必须保留差分基准（mutant >= 2 提前剪枝 → rate 退化为 0）；实际 {r}"
    );
}
