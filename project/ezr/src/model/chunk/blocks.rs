//! blocks — 运行时分块状态与租约快照（Blocks 队列、连接分配）

#[cfg(test)]
use super::lease::{lease_snapshot, spread_bytes};
/// 总块数 y = ⌈total / piece⌉；空文件或零块大小无分块
use super::plan::{block_range, chunk_total};

/// 分块运行时状态表：每块 Done（全部完成）或 Partial（块内已写 `done` 字节）。
/// 下载中断后已写盘字节保持有效（FR-01-13），续传从 `start + done` 继续 Range。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blocks {
    /// 文件总大小
    pub total: u64,
    /// 块大小
    pub piece: u64,
    /// 各块已写字节（值 == 块长表示完成；0 表示未开始）
    pub written: Vec<u64>,
}

impl Blocks {
    /// 构建全空块表
    #[must_use]
    pub fn new(total: u64, piece: u64) -> Self {
        let y = chunk_total(total, piece);
        Blocks {
            total,
            piece,
            written: vec![0; y as usize],
        }
    }

    /// 总块数
    #[must_use]
    pub fn count(&self) -> u32 {
        self.written.len() as u32
    }

    /// 已完成块数 x（written == 块长）
    #[must_use]
    pub fn completed(&self) -> u32 {
        self.written
            .iter()
            .enumerate()
            .filter(|(i, w)| {
                block_range(self.total, self.piece, *i as u32).is_some_and(|(s, e)| **w >= e - s)
            })
            .count() as u32
    }

    /// 已下载字节合计（完成块全量 + 部分块块内字节）
    #[must_use]
    pub fn downloaded(&self) -> u64 {
        self.written
            .iter()
            .enumerate()
            .map(
                |(i, w)| match block_range(self.total, self.piece, i as u32) {
                    Some((s, e)) => (*w).min(e - s),
                    None => 0,
                },
            )
            .sum()
    }

    /// 第 `i` 块是否已完成
    #[must_use]
    pub fn is_done_block(&self, i: u32) -> bool {
        match (
            block_range(self.total, self.piece, i),
            self.written.get(i as usize),
        ) {
            (Some((s, e)), Some(w)) => *w >= e - s,
            _ => false,
        }
    }

    /// 标记第 `i` 块完成
    pub fn mark_done(&mut self, i: u32) {
        if let Some((s, e)) = block_range(self.total, self.piece, i) {
            if let Some(w) = self.written.get_mut(i as usize) {
                *w = e - s;
            }
        }
    }

    /// 标记第 `i` 块块内进度（`done` 字节；越界钳制到块长）
    pub fn mark_progress(&mut self, i: u32, done: u64) {
        if let Some((s, e)) = block_range(self.total, self.piece, i) {
            if let Some(w) = self.written.get_mut(i as usize) {
                *w = done.min(e - s);
            }
        }
    }
}

/// 连接展示快照（demo `make_chunk_conns` 同构语义）：按「已下载 done_bytes 的
/// 连续前缀」把 min(n, 未完成块数) 个块分配给 n 个连接；`done_bytes >= total`
/// （校验/完成）时展示最后 min(n, y) 个满块。
///
/// 仅用于 UI 快照与恢复基线；下载中的真实连接视图由引擎逐连接上报。
/// 返回 (连接列表, 已完成块数 x)。
#[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::chunk::plan::{fmt_block_size, BT_CHUNK_SIZE};

    const MB: u64 = 1_048_576;

    #[test]
    fn chunk_total_counts_ceil() {
        assert_eq!(chunk_total(0, MB), 0);
        assert_eq!(chunk_total(1, MB), 1);
        assert_eq!(chunk_total(MB, MB), 1);
        assert_eq!(chunk_total(MB + 1, MB), 2);
        assert_eq!(chunk_total(3 * MB, MB), 3);
        assert_eq!(chunk_total(5_000_000, 1_000_000), 5);
        assert_eq!(chunk_total(5_000_001, 1_000_000), 6);
    }

    #[test]
    fn chunk_total_saturates_on_huge() {
        // 块大小极小时块数封顶 u32::MAX，不 panic 不回绕
        assert_eq!(chunk_total(u64::MAX, 1), u32::MAX);
    }

    #[test]
    fn block_range_last_block_absorbs_remainder() {
        let total = 2_500_000;
        let piece = 1_000_000;
        assert_eq!(block_range(total, piece, 0), Some((0, 1_000_000)));
        assert_eq!(block_range(total, piece, 1), Some((1_000_000, 2_000_000)));
        assert_eq!(block_range(total, piece, 2), Some((2_000_000, 2_500_000)));
        assert_eq!(block_range(total, piece, 3), None);
    }

    #[test]
    fn blocks_completed_and_downloaded() {
        let mut b = Blocks::new(2_500_000, 1_000_000);
        assert_eq!(b.count(), 3);
        assert_eq!(b.completed(), 0);
        assert_eq!(b.downloaded(), 0);
        b.mark_done(0);
        b.mark_progress(1, 400_000);
        assert_eq!(b.completed(), 1);
        assert_eq!(b.downloaded(), 1_400_000);
        assert_eq!(b.count() - b.completed(), 2);
        b.mark_done(1);
        b.mark_done(2);
        assert_eq!(b.completed(), b.count());
        assert_eq!(b.downloaded(), 2_500_000);
    }

    #[test]
    fn blocks_progress_clamps_to_block_len() {
        let mut b = Blocks::new(1_000_000, 1_000_000);
        b.mark_progress(0, 9_999_999);
        assert!(b.is_done_block(0));
        assert_eq!(b.downloaded(), 1_000_000);
    }

    #[test]
    fn snapshot_idle_task_has_no_conns() {
        let (conns, x) = lease_snapshot(5 * MB, 0, 4, MB);
        assert_eq!(conns.len(), 4);
        assert_eq!(x, 0);
        // done=0 时第一连接持有第 0 块、块内进度按权重散布
        assert_eq!(conns[0].start, 0);
        assert_eq!(conns[0].end, MB);
    }

    #[test]
    fn snapshot_partial_prefix() {
        let (conns, x) = lease_snapshot(5 * MB, 2 * MB + 300_000, 4, MB);
        assert_eq!(x, 2);
        // 实际并发 = min(4, 剩余未完成块 3) = 3（FR-01-11）
        assert_eq!(conns.len(), 3);
        // 已完成块（0、1）不占连接；三个连接分别持有块 2/3/4，
        // 块内进度按权重散布且总和精确等于块内余量 300 KB
        assert_eq!(conns[0].start, 2 * MB);
        assert_eq!(conns[1].start, 3 * MB);
        assert_eq!(conns[2].start, 4 * MB);
        let sum: u64 = conns.iter().map(|c| c.done).sum();
        assert_eq!(sum, 300_000);
        for c in &conns {
            assert!(c.done <= c.cap());
        }
    }

    #[test]
    fn snapshot_done_shows_last_full_blocks() {
        let (conns, x) = lease_snapshot(5 * MB, 5 * MB, 2, MB);
        assert_eq!(x, 5);
        assert_eq!(conns.len(), 2);
        for c in &conns {
            assert_eq!(c.done, c.cap());
        }
    }

    #[test]
    fn snapshot_active_never_exceeds_pending() {
        // 剩余 1 块时实际并发 1（FR-01-11）
        let (conns, _) = lease_snapshot(5 * MB, 4 * MB, 4, MB);
        assert_eq!(conns.len(), 1);
    }

    #[test]
    fn spread_bytes_sums_exact() {
        let v = spread_bytes(1_234_567, 4, MB);
        assert_eq!(v.iter().sum::<u64>(), 1_234_567);
        let v = spread_bytes(0, 4, MB);
        assert!(v.iter().all(|&x| x == 0));
        let v = spread_bytes(999, 4, 100);
        assert_eq!(v.iter().sum::<u64>(), 400); // 钳制到 cap
    }

    #[test]
    fn fmt_block_size_matches_demo_labels() {
        assert_eq!(fmt_block_size(MB), "1 MB");
        assert_eq!(fmt_block_size(BT_CHUNK_SIZE), "256 KB");
        assert_eq!(fmt_block_size(2 * MB), "2 MB");
        assert_eq!(fmt_block_size(512 * 1024), "512 KB");
        assert_eq!(fmt_block_size(123), "123 B");
    }
}
