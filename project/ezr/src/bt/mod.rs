pub mod bencode;
pub mod engine;
pub mod metainfo;
pub mod peer;
pub mod peer_id;
pub mod pieces;
pub mod tracker;
pub mod webrtc;

use crate::model::BtFile;

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

impl std::error::Error for BtError {}

impl From<std::io::Error> for BtError {
    fn from(e: std::io::Error) -> Self {
        BtError::Io(e.to_string())
    }
}

// bencode 模块自包含化（mktorrent bencode 合规修订轮，操作者授权越界）：decode 族
// 返回模块局部 `BencodeError`，经本映射升格为门面类型——消息串逐条保留，
// Display 输出字节级不变；产品调用点 `?` 自动转换零改动（metainfo/tracker），
// `.ok()`/`is_ok()` 消费点类型无关（peer.rs）。
impl From<bencode::BencodeError> for BtError {
    fn from(e: bencode::BencodeError) -> Self {
        BtError::Bencode(e.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BtTaskCmd {
    Pause,
    Cancel,
    StopSeed,
    Disconnect(usize),
    UpdateTrackers(Vec<String>),
    Reconfigure {
        concurrency: usize,
        files: Vec<BtFile>,
        proxy_endpoint: Option<crate::model::ProxyEndpoint>,
        max_download_speed: u64,
        max_upload_speed: u64,
        sequential: bool,
    },
}

pub const REQUEST_BLOCK_LEN: u32 = 16 * 1024;
