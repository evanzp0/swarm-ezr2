//! timefmt — 时间显示工具（Unix 秒 → 展示字段；东八区固定偏移口径）

use std::time::{SystemTime, UNIX_EPOCH};

/// 不引入 chrono 依赖：按 Unix 秒手工换算（闰年/月长表），误差仅与时区
/// 固定偏移有关；显示用字段，不参与任何一致性判定。
#[must_use]
pub fn fmt_created(unix: u64) -> String {
    // 东八区偏移；显示口径与 demo 一致（本地时间）
    const CST: i64 = 8 * 3600;
    let t = unix as i64 + CST;
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60
    )
}

/// 天数 → (年, 月, 日)（Howard Hinnant civil_from_days 算法，公历）
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 当前时刻的 Unix 秒（注册表/文件名时间戳兜底用）
#[must_use]
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
