//! chunk — AIR2 分块模型（计划数学见 [`plan`]，运行时租约见 [`blocks`]）

mod blocks;
mod lease;
mod plan;

pub use blocks::Blocks;
pub use lease::lease_snapshot;
#[allow(unused_imports)] // 02 期 BT 常量（01 保留对外 API）
pub use plan::BT_CHUNK_SIZE;
pub use plan::{block_range, chunk_total, fmt_block_size, HTTP_CHUNK_SIZE};
