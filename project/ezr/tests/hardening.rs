#![allow(missing_docs)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]
#![allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::too_many_arguments
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
#![allow(dead_code)]
#![allow(clippy::duplicate_mod)]
#![allow(unused_imports)]
#[path = "hardening/blocks_hard.rs"]
mod blocks_hard;
#[path = "hardening/checksum_hard.rs"]
mod checksum_hard;
#[path = "hardening/config_fallback.rs"]
mod config_fallback;
#[path = "hardening/lease_spread.rs"]
mod lease_spread;
#[path = "hardening/model/mod.rs"]
mod model;
#[path = "hardening/namegen_hard.rs"]
mod namegen_hard;
#[path = "hardening/speed_evict.rs"]
mod speed_evict;
#[path = "hardening/speed_window.rs"]
mod speed_window;
#[path = "hardening/timefmt_hard.rs"]
mod timefmt_hard;
