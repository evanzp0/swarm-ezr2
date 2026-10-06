//! `model` 挂载壳：为 blocks/lease 的 `crate::model::...` 路径提供真实解析点。
//! `Connection` 与 src/model/task.rs 同构（名义类型按结构兼容挂载）。

#![allow(missing_docs)]

/// 收编产品侧测试环境工具（src/model/config.rs 等收编源引用
/// `crate::model::testenv::uniq_tmp_dir`，挂载壳须提供同路径解析点）
#[path = "../../../src/model/testenv.rs"]
pub mod testenv;

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
