//! 加固测试：`timefmt`（src/model/timefmt.rs 变异幸存体杀灭）。
//! 靶点（放量轮 timefmt.rs 幸存 6）：
//! - `27:33` yoe 分子 `+ doe/36524` → `-`：翻转点 days=59（1970-03-01）；
//! - `27:48` `- doe/146096` → `+`（days=11016 即 2000-02-29 翻转）与 → `/`
//!   （days=0 时 `doe/146096` 为 0，`doe/36524 / doe/146096` 除零 panic）；
//! - `29:42` doy 的 `yoe/4 - yoe/100` → `+`：days=0 翻转；
//! - `32:29` `mp + 3` → `*`：mp=0 时输出月份 0（days=59）；
//! - `32:45` `mp - 9` → `/`：mp=11 时 11/9=1 错报一月（days=31，1970-02-01）；
//! - `39:5` `unix_now → 0`：必须返回当前 Unix 秒（> 2023-01-01 且 ≤ 当前+容差）。
//! 期望值以 Howard Hinnant civil_from_days 独立实现（Python 参照真实日历）逐点锚定。

#![allow(missing_docs)]
#[path = "../../src/model/timefmt.rs"]
mod timefmt;

use timefmt::{fmt_created, unix_now};

/// 靶 `27:48(/)`（doe=0 除零）与 `29:42`（yoe/4 与 yoe/100 之间 `-`→`+`）：epoch 锚点。
#[test]
fn epoch_anchor() {
    assert_eq!(fmt_created(0), "1970-01-01 08:00");
}

/// 靶 `27:33`（yoe 分子 `+doe/36524`→`-`）与 `32:29`（`mp+3`→`mp*3`，mp=0 → 月 0）。
#[test]
fn march_anchor() {
    assert_eq!(fmt_created(5_097_600), "1970-03-01 08:00");
}

/// 靶 `32:45`（`mp-9`→`mp/9`，mp=11 → 11/9=1）：1970-02-01（mp=11 → 二月）。
#[test]
fn february_anchor_kills_mp_div() {
    assert_eq!(fmt_created(2_678_400), "1970-02-01 08:00");
}

/// 靶 `27:48(+)`（doe=146096 时 `doe/146096`=1，`-`→`+` 使 yoe 进位翻转）：
/// 2000-02-29（era 末闰日）。
#[test]
fn leap_era_tail_anchor() {
    assert_eq!(fmt_created(951_782_400), "2000-02-29 08:00");
}

/// 跨日界对照：23:59:59 UTC 落在东八区次日 07:59。
#[test]
fn day_boundary_crossing() {
    assert_eq!(fmt_created(86_399), "1970-01-02 07:59");
}

/// 靶 `39:5`（`unix_now → 0`）：返回值必须是当前 Unix 秒。
#[test]
fn unix_now_tracks_wall_clock() {
    let now = unix_now();
    assert!(now > 1_672_531_200, "unix_now={} 恒 0 变体", now); // > 2023-01-01
    let sys = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(now <= sys + 5, "unix_now={} 不得超前系统时钟", now);
}
