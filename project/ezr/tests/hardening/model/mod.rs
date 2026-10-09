//! `model` 挂载壳：为 blocks/lease/checksum 的 `crate::model::...` 路径提供
//! 真实解析点。`Connection` 与 src/model/task.rs 同构（名义类型按结构兼容挂载）。

#![allow(missing_docs)]

/// 收编产品侧测试环境工具（src/model/config.rs 等收编源引用
/// `crate::model::testenv::uniq_tmp_dir`，挂载壳须提供同路径解析点）
#[path = "../../../src/model/testenv.rs"]
pub mod testenv;

/// 收编产品侧 checksum 模块（v117 加固轮：checksum_hard.rs 靶
/// `validate_value` 越界钳位与 `Display for ChecksumError`；
/// 其内嵌 tests 引用 `crate::model::testenv`，经本壳解析）
#[path = "../../../src/model/checksum.rs"]
pub mod checksum;

#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub id: usize,
    pub start: u64,
    pub end: u64,
    pub done: u64,
}

impl Connection {
    /// 与 task.rs::Connection::cap 同构（块持有总字节）。
    pub fn cap(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }
}

pub mod chunk;
