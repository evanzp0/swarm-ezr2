#![allow(missing_docs)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BtError {
    Bencode(String),
    Meta(String),
    Tracker(String),
    Peer(String),
    Io(String),
}

impl std::fmt::Display for BtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BtError::Bencode(m) => write!(f, "bencode 解析失败：{m}"),
            BtError::Meta(m) => write!(f, "种子元数据无效：{m}"),
            BtError::Tracker(m) => write!(f, "tracker 错误：{m}"),
            BtError::Peer(m) => write!(f, "peer 协议错误：{m}"),
            BtError::Io(m) => write!(f, "IO 错误：{m}"),
        }
    }
}

impl From<std::io::Error> for BtError {
    fn from(e: std::io::Error) -> Self {
        BtError::Io(e.to_string())
    }
}

#[path = "../../src/bt/bencode.rs"]
pub mod bencode;
#[path = "../../src/bt/tracker.rs"]
pub mod tracker;
#[path = "../../src/bt/webrtc.rs"]
pub mod webrtc;
