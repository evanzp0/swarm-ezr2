#[cfg(test)]
use super::lease::{lease_snapshot, spread_bytes};
use super::plan::{block_range, chunk_total};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blocks {
    pub total: u64,
    pub piece: u64,
    pub written: Vec<u64>,
}

impl Blocks {
    #[must_use]
    pub fn new(total: u64, piece: u64) -> Self {
        let y = chunk_total(total, piece);
        Blocks {
            total,
            piece,
            written: vec![0; y as usize],
        }
    }

    #[must_use]
    pub fn count(&self) -> u32 {
        self.written.len() as u32
    }

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

    #[must_use]
    pub fn done_indices(&self) -> Vec<u32> {
        self.written
            .iter()
            .enumerate()
            .filter(|(i, w)| {
                block_range(self.total, self.piece, *i as u32).is_some_and(|(s, e)| **w >= e - s)
            })
            .map(|(i, _)| i as u32)
            .collect()
    }

    #[must_use]
    pub fn partial_indices(&self) -> Vec<u32> {
        self.written
            .iter()
            .enumerate()
            .filter(|(i, w)| {
                block_range(self.total, self.piece, *i as u32)
                    .is_some_and(|(s, e)| **w > 0 && **w < e - s)
            })
            .map(|(i, _)| i as u32)
            .collect()
    }

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

    pub fn mark_done(&mut self, i: u32) {
        if let Some((s, e)) = block_range(self.total, self.piece, i) {
            if let Some(w) = self.written.get_mut(i as usize) {
                *w = e - s;
            }
        }
    }

    pub fn mark_progress(&mut self, i: u32, done: u64) {
        if let Some((s, e)) = block_range(self.total, self.piece, i) {
            if let Some(w) = self.written.get_mut(i as usize) {
                *w = done.min(e - s);
            }
        }
    }
}

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
    fn done_indices_sorted_matches_completed() {
        let mut b = Blocks::new(5_000_000, 1_000_000);
        assert!(b.done_indices().is_empty(), "全空无完成块");
        b.mark_done(2);
        b.mark_done(0);
        b.mark_progress(1, 300_000);
        let idx = b.done_indices();
        assert_eq!(idx, vec![0, 2], "升序列出已完成块号");
        assert_eq!(idx.len() as u32, b.completed(), "与 completed() 计数一致");
    }

    #[test]
    fn partial_indices_sorted_excludes_done_and_empty() {
        let mut b = Blocks::new(5_000_000, 1_000_000);
        assert!(b.partial_indices().is_empty(), "全空无部分块");
        b.mark_done(0);
        b.mark_progress(1, 400_000);
        b.mark_progress(2, 1_000_000);
        b.mark_progress(4, 1);
        let idx = b.partial_indices();
        assert_eq!(idx, vec![1, 4], "升序列出部分下载块号");
        let done = b.done_indices();
        assert!(
            idx.iter().all(|i| !done.contains(i)),
            "部分集合与完成集合互斥"
        );
    }

    #[test]
    fn snapshot_idle_task_has_no_conns() {
        let (conns, x) = lease_snapshot(5 * MB, 0, 4, MB);
        assert_eq!(conns.len(), 4);
        assert_eq!(x, 0);
        assert_eq!(conns[0].start, 0);
        assert_eq!(conns[0].end, MB);
    }

    #[test]
    fn snapshot_partial_prefix() {
        let (conns, x) = lease_snapshot(5 * MB, 2 * MB + 300_000, 4, MB);
        assert_eq!(x, 2);
        assert_eq!(conns.len(), 3);
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
        assert_eq!(v.iter().sum::<u64>(), 400);
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
