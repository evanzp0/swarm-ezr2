pub mod bencode;
pub mod engine;
pub mod metainfo;
pub mod peer;
pub mod peer_id;
pub mod pieces;
pub mod tracker;

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

#[allow(dead_code)]
pub const BT_DEFAULT_CONCURRENCY: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BtTaskCmd {
    Pause,
    Cancel,
    StopSeed,
    Disconnect(usize),
}

pub const SEED_DURATION_SECS: f64 = 1800.0;

/// 每 peer 在飞请求块数上限（×16KiB = 1MiB 窗口）。
/// 对齐 libtorrent request_queue_time=3s 的动态深窗精神（队列 = 3×下载速率，
/// cap 500）：固定窗需能吸收 piece 校验/落盘造成的引擎 refill 延迟尖峰，
/// 32 块（512KiB）在真实大种子上不够；64 块留足余量且内存可忽略。
pub const PEER_PIPELINE: usize = 64;

pub const REQUEST_BLOCK_LEN: u32 = 16 * 1024;

pub const HANDSHAKE_TIMEOUT_SECS: u64 = 10;

/// 出连 TCP 拨号超时：公网 swarm 死 peer 占比高，10s 级拨号会把
/// 并发槽位成片占满、拖垮连接建立速度（libtorrent 连接超时为秒级）。
pub const CONNECT_TIMEOUT_SECS: u64 = 3;
