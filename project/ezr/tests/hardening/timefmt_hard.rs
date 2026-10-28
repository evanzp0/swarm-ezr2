#![allow(missing_docs)]
#[path = "../../src/model/timefmt.rs"]
mod timefmt;

use timefmt::{fmt_created, unix_now};

#[test]
fn epoch_anchor() {
    assert_eq!(fmt_created(0), "1970-01-01 08:00");
}

#[test]
fn march_anchor() {
    assert_eq!(fmt_created(5_097_600), "1970-03-01 08:00");
}

#[test]
fn february_anchor_kills_mp_div() {
    assert_eq!(fmt_created(2_678_400), "1970-02-01 08:00");
}

#[test]
fn leap_era_tail_anchor() {
    assert_eq!(fmt_created(951_782_400), "2000-02-29 08:00");
}

#[test]
fn day_boundary_crossing() {
    assert_eq!(fmt_created(86_399), "1970-01-02 07:59");
}

#[test]
fn unix_now_tracks_wall_clock() {
    let now = unix_now();
    assert!(now > 1_672_531_200, "unix_now={} 恒 0 变体", now);
    let sys = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(now <= sys + 5, "unix_now={} 不得超前系统时钟", now);
}
