//! 加固测试入口（six-pack/hardender）：变异存活体杀灭套件，独立成套。
//! 布局：ezr 为 bin-only crate（无 lib），tests/ 无法导入二进制 crate 内部模块；
//! 以 `#[path]` 源包含方式挂载被测产品模块（mutants.out 口径：产品树零改动）。
//! 入口命令：`cargo test --test hardening`
//!
//! `crate::model` 挂载壳（真实树形 model::chunk::{plan,lease,blocks}）挂在 crate
//! 根供全部子模块解析 `crate::model::...`（blocks/lease 的源码路径依赖）。
// lint 姿态与产品 bin（src/main.rs crate 级 allow）对齐：#[path] 收编的产品源
// 在本测试 crate 内沿用产品面的豁免口径（产品面 clippy 0 的同一合同）。
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
// 测试挂载机制固有豁免：产品源经 #[path] 部分收编进独立测试 crate，部分产物项
// 在本 crate 上下文无消费者或被重复挂载（产品全量编译下均非死代码）。
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
