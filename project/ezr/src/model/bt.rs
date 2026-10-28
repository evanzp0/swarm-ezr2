use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BtFile {
    pub path: String,
    pub size: u64,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BtTaskData {
    pub source: String,
    pub trackers: Vec<String>,
    pub piece_length: u32,
    pub files: Vec<BtFile>,
    pub done_pieces: Vec<u32>,
}
