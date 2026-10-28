pub const HTTP_CHUNK_SIZE: u64 = 1024 * 1024;

pub const MIN_HTTP_BLOCK_SIZE: u64 = 1024 * 1024;

#[allow(dead_code)]
pub const BT_CHUNK_SIZE: u64 = 256 * 1024;

pub fn chunk_total(total: u64, piece: u64) -> u32 {
    if total == 0 || piece == 0 {
        0
    } else {
        u32::try_from(total.div_ceil(piece)).unwrap_or(u32::MAX)
    }
}

#[must_use]
pub fn block_range(total: u64, piece: u64, i: u32) -> Option<(u64, u64)> {
    let y = chunk_total(total, piece);
    if i >= y {
        return None;
    }
    let start = u64::from(i) * piece;
    let end = if u64::from(i + 1) == u64::from(y) {
        total
    } else {
        start + piece
    };
    Some((start, end))
}

#[must_use]
pub fn fmt_block_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    if bytes == MB {
        "1 MB".to_string()
    } else if bytes == BT_CHUNK_SIZE {
        "256 KB".to_string()
    } else if bytes >= MB && bytes.is_multiple_of(MB) {
        format!("{} MB", bytes / MB)
    } else if bytes >= KB && bytes.is_multiple_of(KB) {
        format!("{} KB", bytes / KB)
    } else {
        format!("{bytes} B")
    }
}
