//! 加固测试：`SpeedWindow`（src/model/speed.rs 变异幸存体杀灭）。
//! 靶点（校准轮 12 幸存中的 7 个）：
//! - `evict` 整体删除（61:9）——窗口不剪枝，速率退化为全程平均；
//! - `61:34` 长度卫 `> 2` → `== 2` / `>= 2`；
//! - `61:79` 窗口比较 `> WINDOW` → `== / < / >=`（含恰好 1 秒边界）；
//! - `54:48` 速率除法 `dt` → 乘法（`dt = 1.0` 时乘除巧合，需非 1 秒跨度探针）。
//!
//! 确定性手法：未来时间戳（`base = now + 1h`）——`rate()` 内部 `evict(Instant::now())`
//! 对未来样本 `duration_since` 饱和为 0，剪枝只由 `push` 的合成时间驱动，速率断言
//! 不依赖测试机快慢；`>= 2` 长度卫的真实时钟差异用短 sleep 相位单列一测。

#![allow(missing_docs)]
#[path = "../../src/model/speed.rs"]
mod speed;

use std::time::{Duration, Instant};

use speed::{SpeedWindow, WINDOW};

/// 未来基准：保证 `rate()` 的真实时钟剪枝为无操作（duration_since 饱和为 0）
fn future_base() -> Instant {
    Instant::now() + Duration::from_secs(3600)
}

fn at(base: Instant, millis: u64) -> Instant {
    base + Duration::from_millis(millis)
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-4
}

/// 靶 `evict`→()`、`61:34 > → ==`：不剪枝时速率退化为全程平均（700），正确剪枝为窗口末段速率（100）。
/// 前段快（1000 B/s）后段慢（100 B/s），窗口剪枝后末段读数只反映慢段。
#[test]
fn evict_prunes_stale_front_rate_tracks_recent_window() {
    let base = future_base();
    let mut w = SpeedWindow::new();
    w.push(at(base, 0), 0);
    for i in 1..=20 {
        w.push(at(base, 500 * i), 500 * i); // 快段：1000 B/s
    }
    for i in 21..=30 {
        w.push(at(base, 500 * i), 10_000 + 50 * (i - 20)); // 慢段：100 B/s
    }
    // 正确语义：front 保留在窗口内（13500ms，累计 10350），back=15000ms（累计 10500）
    let r = w.rate();
    assert!(close(r, 100.0), "rate={r}（不剪枝/长度卫失守时≈700）");
}

/// 靶 `54:48 / → *`：`dt = 0.5s` 时 500/0.5=1000，乘法变体得 250。
#[test]
fn rate_divides_by_time_span() {
    let base = future_base();
    let mut w = SpeedWindow::new();
    w.push(at(base, 0), 0);
    w.push(at(base, 500), 500);
    let r = w.rate();
    assert!(close(r, 1000.0), "rate={r}（乘法变体=250）");
}

/// 靶 `61:79 > → >= / == / <`：恰好 1 秒旧的内部样本必须保留（差值基准）。
/// 正确语义：push(3000) 时逐出 (0) 后 samples[1]=(2000) 恰满 WINDOW 停止 →
/// front=(1000,1000) → 速率 (5000−1000)/2.0 = 2000；
/// `>=` 驱逐恰满窗口样本 → front=(2000) → 4000；`==`/`<` 不逐 (0) → front=(0,0) → ≈1666.7。
#[test]
fn boundary_sample_exactly_one_window_old_is_retained() {
    assert_eq!(WINDOW, Duration::from_secs(1)); // 靶前提：窗口常量未被变异
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

/// 靶 `61:79 > → <`：比较方向反转时会驱逐"年轻"内部样本，速率塌缩为 0/末段差值。
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

/// 靶 `61:34 > → >=`：仅两样本且均过期时，正确语义保留旧端点作差值基准（速率 1000），
/// `>= 2` 会连基准一并驱逐（速率 0）。此差异只在真实时钟（rate 时 back 已 >1s 旧）可观察。
#[test]
fn stale_pair_rate_preserved_with_two_samples() {
    let t0 = Instant::now();
    let mut w = SpeedWindow::new();
    w.push(t0, 0);
    w.push(t0 + Duration::from_millis(100), 100);
    std::thread::sleep(Duration::from_millis(1200)); // 让 back 亦 >WINDOW 旧
    let r = w.rate();
    assert!(close(r, 1000.0), "rate={r}（>= 2 长度卫驱逐基准 → 0）");
}
