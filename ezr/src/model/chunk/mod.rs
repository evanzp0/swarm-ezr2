//! chunk — AIR2 分块模型（计划数学见 [`plan`]，运行时租约见 [`blocks`]）

mod blocks;
mod lease;
mod plan;

pub use blocks::Blocks;
pub use lease::lease_snapshot;
// pub(crate) 而非 pub：spread_bytes 是快照内部散布算法，仅对 crate 内属性
// 测试开放（property_tests 守恒性验证），不进对外 API；测试专用 → cfg(test) 门控
#[cfg(test)]
pub(crate) use lease::spread_bytes;
#[allow(unused_imports)] // 02 期 BT 常量（01 保留对外 API）
pub use plan::BT_CHUNK_SIZE;
pub use plan::{block_range, chunk_total, fmt_block_size, HTTP_CHUNK_SIZE, MIN_HTTP_BLOCK_SIZE};
