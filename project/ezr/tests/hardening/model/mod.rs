#![allow(missing_docs)]

#[path = "../../../src/model/testenv.rs"]
pub mod testenv;

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
    pub fn cap(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }
}

pub mod chunk;
