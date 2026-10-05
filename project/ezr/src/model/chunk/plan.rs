//! plan — 分块计划数学（块数计算/块区间/块大小格式化，纯函数）

/// HTTP 协议默认块大小（phase-01 定案：1 MB/块，块数与并发数解耦；demo D13 默认值）
pub const HTTP_CHUNK_SIZE: u64 = 1024 * 1024;

/// BT 协议默认块大小（256 KB；02 期启用，01 保留常量）
#[allow(dead_code)]
pub const BT_CHUNK_SIZE: u64 = 256 * 1024;

pub fn chunk_total(total: u64, piece: u64) -> u32 {
    if total == 0 || piece == 0 {
        0
    } else {
        u32::try_from(total.div_ceil(piece)).unwrap_or(u32::MAX)
    }
}

/// 第 `i` 块（0 基）的字节区间 `[start, end)`；`i` 越界返回 `None`
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

/// 块大小展示文字（固定口径直出，保证显示恰为 `1 MB` / `256 KB` 等形式）
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
