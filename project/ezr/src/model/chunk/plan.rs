pub const HTTP_CHUNK_SIZE: u64 = 1024 * 1024;

pub const MIN_HTTP_BLOCK_SIZE: u64 = 256 * 1024;

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

/// 升序合并重叠/相邻字节区间（投影前先归一，防止重复覆盖虚增字节）。
pub fn merge_byte_ranges(ranges: &mut Vec<(u64, u64)>) {
    ranges.retain(|(a, b)| b > a);
    ranges.sort_by_key(|&(a, _)| a);
    let mut i = 0;
    while i + 1 < ranges.len() {
        if ranges[i].1 >= ranges[i + 1].0 {
            let hi = ranges[i].1.max(ranges[i + 1].1);
            ranges[i].1 = hi;
            ranges.remove(i + 1);
        } else {
            i += 1;
        }
    }
}

/// 把已收字节区间投影到分块网格（piece 与 pieces 窗口同口径）：
/// 返回 (chunk_done, done_blocks, partial_blocks)。
/// 整块被覆盖 → Done；部分覆盖 → Partial；零覆盖 → 不入集合（Empty）。
#[must_use]
pub fn project_ranges(total: u64, piece: u64, ranges: &[(u64, u64)]) -> (u32, Vec<u32>, Vec<u32>) {
    let y = chunk_total(total, piece);
    if y == 0 {
        return (0, Vec::new(), Vec::new());
    }
    let mut sorted: Vec<(u64, u64)> = ranges.to_vec();
    merge_byte_ranges(&mut sorted);
    let mut done = Vec::new();
    let mut partial = Vec::new();
    let mut ri = 0usize;
    for i in 0..y {
        let Some((s, e)) = block_range(total, piece, i) else {
            break;
        };
        while ri < sorted.len() && sorted[ri].1 <= s {
            ri += 1;
        }
        let mut covered = 0u64;
        let mut j = ri;
        while j < sorted.len() && sorted[j].0 < e {
            let a = sorted[j].0.max(s);
            let b = sorted[j].1.min(e);
            if b > a {
                covered += b - a;
            }
            j += 1;
        }
        if covered >= e - s {
            done.push(i);
        } else if covered > 0 {
            partial.push(i);
        }
    }
    (done.len() as u32, done, partial)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_byte_ranges_merges_and_sorts() {
        let mut v = vec![(10, 20), (0, 5), (5, 10), (30, 40), (20, 25)];
        merge_byte_ranges(&mut v);
        assert_eq!(v, vec![(0, 25), (30, 40)], "相邻合并 + 升序");
        let mut v = vec![(0, 100), (50, 80)];
        merge_byte_ranges(&mut v);
        assert_eq!(v, vec![(0, 100)], "包含区间吸收");
        let mut v = vec![(5, 5), (0, 0)];
        merge_byte_ranges(&mut v);
        assert!(v.is_empty(), "空区间剔除");
    }

    #[test]
    fn project_ranges_prefix_done_rest_empty() {
        // 5 MB / 1 MB = 5 块，前 3 块整块落盘
        let mb = 1_048_576u64;
        let (x, done, part) = project_ranges(5 * mb, mb, &[(0, 3 * mb)]);
        assert_eq!(x, 3);
        assert_eq!(done, vec![0, 1, 2]);
        assert!(part.is_empty());
    }

    #[test]
    fn project_ranges_partial_block_shows_partial() {
        let mb = 1_048_576u64;
        let (x, done, part) = project_ranges(5 * mb, mb, &[(0, 3 * mb), (3 * mb, 3 * mb + 100)]);
        assert_eq!(x, 3);
        assert_eq!(done, vec![0, 1, 2]);
        assert_eq!(part, vec![3], "零头块 → Partial");
    }

    #[test]
    fn project_ranges_middle_hole_keeps_two_sides() {
        let mb = 1_048_576u64;
        // 块 0 完成、块 1 空洞、块 2 完成、块 3 零头
        let (x, done, part) = project_ranges(
            5 * mb,
            mb,
            &[(0, mb), (2 * mb, 3 * mb), (3 * mb, 3 * mb + 5)],
        );
        assert_eq!(x, 2);
        assert_eq!(done, vec![0, 2]);
        assert_eq!(part, vec![3]);
    }

    #[test]
    fn project_ranges_last_block_absorbs_remainder() {
        let mb = 1_048_576u64;
        // 2.5 MB / 1 MB：块 2 = [2MB, 2.5MB)，覆盖整块即 Done
        let (x, done, part) = project_ranges(2_500_000, mb, &[(2 * mb, 2_500_000)]);
        assert_eq!(x, 1);
        assert_eq!(done, vec![2]);
        assert!(part.is_empty());
    }

    #[test]
    fn project_ranges_overlapping_ranges_do_not_inflate_coverage() {
        let mb = 1_048_576u64;
        // 区间求并而非求和：块 0 全覆盖 → Done；块 1 实际只覆盖 1/2 → Partial。
        // 若按求和，块 1 覆盖 = 1/2MB + 1/2MB = 1MB 会被误判为 Done。
        let (x, done, part) = project_ranges(2 * mb, mb, &[(0, mb), (mb / 2, 3 * mb / 2)]);
        assert_eq!(x, 1);
        assert_eq!(done, vec![0]);
        assert_eq!(part, vec![1]);
    }

    #[test]
    fn project_ranges_unsorted_input_is_normalized() {
        let mb = 1_048_576u64;
        let (x, done, part) =
            project_ranges(3 * mb, mb, &[(2 * mb, 3 * mb), (0, mb), (mb, 2 * mb)]);
        assert_eq!(x, 3, "乱序输入归一后全完成");
        assert_eq!(done, vec![0, 1, 2]);
        assert!(part.is_empty());
    }

    #[test]
    fn project_ranges_empty_and_zero_total() {
        let (x, done, part) = project_ranges(0, 1_048_576, &[]);
        assert_eq!((x, done, part), (0, Vec::new(), Vec::new()));
        let mb = 1_048_576u64;
        let (x, done, part) = project_ranges(5 * mb, mb, &[]);
        assert_eq!((x, done, part), (0, Vec::new(), Vec::new()), "无区间全空");
    }
}
