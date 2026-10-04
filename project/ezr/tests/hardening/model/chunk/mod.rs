//! `model::chunk` 挂载壳：按真实树形挂载 plan/lease/blocks 三模块，
//! 满足 blocks.rs 的 `super::{lease, plan}` 与内嵌 tests 的
//! `crate::model::chunk::plan`、lease.rs 的 `crate::model::Connection` 解析。

#![allow(missing_docs)]
#[path = "../../../../src/model/chunk/plan.rs"]
pub mod plan;
#[path = "../../../../src/model/chunk/lease.rs"]
pub mod lease;
#[path = "../../../../src/model/chunk/blocks.rs"]
pub mod blocks;
