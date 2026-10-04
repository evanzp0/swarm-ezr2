//! 加固测试入口（six-pack/hardender）：变异存活体杀灭套件，独立成套。
//! 布局：ezr 为 bin-only crate（无 lib），tests/ 无法导入二进制 crate 内部模块；
//! 以 `#[path]` 源包含方式挂载被测产品模块（mutants.out 口径：产品树零改动）。
//! 入口命令：`cargo test --test hardening`
//!
//! `crate::model` 挂载壳（真实树形 model::chunk::{plan,lease,blocks}）挂在 crate
//! 根供全部子模块解析 `crate::model::...`（blocks/lease 的源码路径依赖）。
#[path = "hardening/model/mod.rs"]
mod model;
#[path = "hardening/config_fallback.rs"]
mod config_fallback;
#[path = "hardening/speed_window.rs"]
mod speed_window;
#[path = "hardening/lease_spread.rs"]
mod lease_spread;
#[path = "hardening/namegen_hard.rs"]
mod namegen_hard;
#[path = "hardening/timefmt_hard.rs"]
mod timefmt_hard;
#[path = "hardening/blocks_hard.rs"]
mod blocks_hard;
#[path = "hardening/speed_evict.rs"]
mod speed_evict;
