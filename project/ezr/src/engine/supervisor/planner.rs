use std::time::{Duration, Instant};

use crate::model::chunk::chunk_total;

pub(super) type WorkerId = usize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Segment {
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) cursor: u64,
    pub(super) reserved: u64,
    pub(super) worker: Option<WorkerId>,
    pub(super) receiving: bool,
    pub(super) given_at: Option<Instant>,
}

impl Segment {
    fn new(start: u64, end: u64) -> Segment {
        Segment {
            start,
            end,
            cursor: start,
            reserved: start,
            worker: None,
            receiving: false,
            given_at: None,
        }
    }
}

pub(super) struct Assignment {
    pub(super) from: u64,
}

pub(super) enum Claim {
    Granted { n: u64, seg_end: u64 },
    Lost,
}

pub(super) struct Planner {
    #[allow(dead_code)]
    total: u64,
    segs: Vec<Segment>,
    retired: u64,
    pre_committed: u64,
    min_split: u64,
    cut_align: u64,
    preempt_after: Duration,
}

pub(super) const MIN_PREEMPT: Duration = Duration::from_millis(20);
pub(super) const SPLIT_ALIGN: u64 = 4096;

fn align_down(x: u64, align: u64) -> u64 {
    match x.checked_div(align) {
        Some(q) => q * align,
        None => x,
    }
}

impl Planner {
    #[must_use]
    pub(super) fn new(
        total: u64,
        min_split: u64,
        cut_align: u64,
        preempt_after: Duration,
    ) -> Planner {
        let segs = if total > 0 {
            vec![Segment::new(0, total)]
        } else {
            Vec::new()
        };
        Planner {
            total,
            segs,
            retired: 0,
            pre_committed: 0,
            min_split: min_split.max(1),
            cut_align: cut_align.max(1),
            preempt_after,
        }
    }

    #[must_use]
    pub(super) fn from_gaps(
        total: u64,
        gaps: &[(u64, u64)],
        min_split: u64,
        cut_align: u64,
        preempt_after: Duration,
    ) -> Planner {
        let mut segs: Vec<Segment> = gaps
            .iter()
            .filter(|(s, e)| e > s)
            .map(|(s, e)| Segment::new(*s, *e))
            .collect();
        segs.sort_by_key(|s| s.start);
        let gap_len: u64 = segs.iter().map(|s| s.end - s.start).sum();
        Planner {
            total,
            segs,
            retired: 0,
            pre_committed: total.saturating_sub(gap_len),
            min_split: min_split.max(1),
            cut_align: cut_align.max(1),
            preempt_after,
        }
    }

    pub(super) fn assign(&mut self, w: WorkerId, after: Option<u64>) -> Option<Assignment> {
        if let Some(idx) = self.pick_unassigned(after) {
            let seg = &mut self.segs[idx];
            seg.worker = Some(w);
            seg.given_at = Some(Instant::now());
            seg.receiving = false;
            return Some(Assignment { from: seg.cursor });
        }
        if let Some(idx) = self.pick_stale() {
            let seg = &mut self.segs[idx];
            seg.reserved = seg.cursor;
            seg.worker = Some(w);
            seg.given_at = Some(Instant::now());
            seg.receiving = false;
            return Some(Assignment { from: seg.cursor });
        }
        self.try_split(w)
    }
    fn pick_unassigned(&self, after: Option<u64>) -> Option<usize> {
        let cands: Vec<usize> = self
            .segs
            .iter()
            .enumerate()
            .filter(|(_, s)| s.worker.is_none() && s.cursor < s.end)
            .map(|(i, _)| i)
            .collect();
        let pick = after.and_then(|a| {
            cands
                .iter()
                .copied()
                .find(|i| self.segs[*i].start == a && self.segs[*i].cursor <= a)
        });
        pick.or_else(|| {
            cands
                .iter()
                .copied()
                .min_by_key(|i| (self.segs[*i].start, self.segs[*i].cursor))
        })
    }

    fn pick_stale(&self) -> Option<usize> {
        let now = Instant::now();
        self.segs.iter().position(|s| {
            s.worker.is_some()
                && !s.receiving
                && s.given_at
                    .is_some_and(|t| now.duration_since(t) >= self.preempt_after)
        })
    }

    fn try_split(&mut self, w: WorkerId) -> Option<Assignment> {
        loop {
            if let Some((owner_idx, cut)) = self.find_split() {
                let owner = &mut self.segs[owner_idx];
                let new_end = owner.end;
                owner.end = cut;
                let mut seg = Segment::new(cut, new_end);
                seg.worker = Some(w);
                seg.given_at = Some(Instant::now());
                self.segs.push(seg);
                return Some(Assignment { from: cut });
            }
            if self.min_split > 1 {
                self.min_split = (self.min_split / 2).max(1);
            } else {
                return None;
            }
        }
    }

    fn find_split(&self) -> Option<(usize, u64)> {
        let mut best: Option<(usize, u64, u64)> = None;
        for (i, s) in self.segs.iter().enumerate() {
            if s.done() || s.worker.is_none() {
                continue;
            }
            let claim_end = s.reserved.max(s.cursor);
            let spare = s.end.checked_sub(claim_end)?;
            if spare < 2 * self.min_split {
                continue;
            }
            let Some(cut) = self.cut_point(claim_end, s.end) else {
                continue;
            };
            if best.is_none_or(|(_, _, bs)| spare > bs) {
                best = Some((i, cut, spare));
            }
        }
        best.map(|(i, cut, _)| (i, cut))
    }

    fn cut_point(&self, claim_end: u64, end: u64) -> Option<u64> {
        let spare = end.checked_sub(claim_end)?;
        let ideal = claim_end + spare / 2;
        let mut cands: Vec<u64> = Vec::new();
        let push = |v: &mut Vec<u64>, b: u64| {
            if b > claim_end && b < end && !v.contains(&b) {
                v.push(b);
            }
        };
        let fl = ideal / self.cut_align * self.cut_align;
        push(&mut cands, fl);
        if fl < ideal {
            if let Some(c) = fl.checked_add(self.cut_align) {
                push(&mut cands, c);
            }
        }
        let lo = claim_end.saturating_add(self.min_split);
        let hi = end.saturating_sub(self.min_split);
        if lo <= hi {
            let lf = lo / self.cut_align * self.cut_align;
            if lf < lo {
                if let Some(c) = lf.checked_add(self.cut_align) {
                    push(&mut cands, c);
                }
            } else {
                push(&mut cands, lf);
            }
            push(&mut cands, hi / self.cut_align * self.cut_align);
        }
        let feasible = |b: u64| {
            b >= claim_end.saturating_add(self.min_split) && b <= end.saturating_sub(self.min_split)
        };
        if let Some(b) = cands
            .iter()
            .copied()
            .filter(|&b| feasible(b))
            .min_by_key(|&b| b.abs_diff(ideal))
        {
            return Some(b);
        }
        if let Some(b) = cands.iter().copied().min_by_key(|&b| b.abs_diff(ideal)) {
            return Some(b);
        }
        if chunk_total(self.total, self.cut_align) <= 1 {
            let half = align_down(spare / 2, SPLIT_ALIGN);
            if half >= self.min_split {
                return Some(claim_end + half);
            }
        }
        None
    }

    pub(super) fn claim(&mut self, w: WorkerId, pos: u64, len: u64) -> Claim {
        let Some(idx) = self
            .segs
            .iter()
            .position(|s| s.worker == Some(w) && pos >= s.start && pos < s.end)
        else {
            return Claim::Lost;
        };
        let seg = &mut self.segs[idx];
        if pos < seg.cursor {
            return Claim::Lost;
        }
        let grant = len.min(seg.end - pos);
        if grant == 0 {
            return Claim::Lost;
        }
        seg.reserved = seg.reserved.max(pos + grant);
        Claim::Granted {
            n: grant,
            seg_end: seg.end,
        }
    }

    pub(super) fn commit(&mut self, w: WorkerId, upto: u64) -> Option<u64> {
        let idx = self.segs.iter().position(|s| s.worker == Some(w))?;
        let seg = &mut self.segs[idx];
        if upto < seg.start || upto > seg.end {
            return None;
        }
        seg.cursor = seg.cursor.max(upto);
        seg.reserved = seg.reserved.max(seg.cursor);
        seg.receiving = true;
        if seg.cursor >= seg.end {
            seg.worker = None;
            Some(seg.end)
        } else {
            None
        }
    }

    pub(super) fn release(&mut self, w: WorkerId) {
        for s in self.segs.iter_mut() {
            if s.worker == Some(w) {
                s.reserved = s.cursor;
                s.worker = None;
                s.receiving = false;
                s.given_at = None;
            }
        }
    }

    pub(super) fn absorb_next(&mut self, w: WorkerId, at: u64) -> Option<u64> {
        let idx = self.segs.iter().position(|s| {
            s.start == at && s.cursor == s.start && s.cursor < s.end && s.worker.is_none()
        })?;
        let seg = &mut self.segs[idx];
        seg.worker = Some(w);
        seg.cursor = seg.start;
        seg.reserved = seg.start;
        seg.receiving = true;
        seg.given_at = Some(Instant::now());
        Some(seg.end)
    }

    pub(super) fn completed_bytes(&self) -> u64 {
        self.pre_committed
            + self.retired
            + self
                .segs
                .iter()
                .map(|s| s.cursor.min(s.end) - s.start)
                .sum::<u64>()
    }

    /// 各段已提交字节区间 [start, cursor)：cursor 只在字节推入 writer 后前进，
    /// 因此本集合 = 已接收并排队待落盘的区间（列表 downloaded 的同源覆盖口径，
    /// 领先于 writer 刷盘区间）。retired 段已并入 flushed、pre_committed 并入
    /// resume 区间，均不在此重复。
    pub(super) fn committed_ranges(&self) -> Vec<(u64, u64)> {
        self.segs
            .iter()
            .filter(|s| s.cursor > s.start)
            .map(|s| (s.start, s.cursor.min(s.end)))
            .collect()
    }

    #[cfg(test)]
    #[allow(dead_code)] // hardening 测试以非 test cfg 包含本文件时会误报死代码
    pub(super) fn reserved_total(&self) -> u64 {
        self.pre_committed
            + self.retired
            + self
                .segs
                .iter()
                .map(|s| s.reserved.min(s.end) - s.start)
                .sum::<u64>()
    }

    #[cfg(test)]
    pub(super) fn segments(&self) -> &[Segment] {
        &self.segs
    }

    #[cfg(test)]
    #[allow(dead_code)] // 同上：非 test cfg 包含时误报
    pub(super) fn min_split(&self) -> u64 {
        self.min_split
    }

    pub(super) fn gc<F>(&mut self, flushed: F) -> usize
    where
        F: Fn(u64, u64) -> u64,
    {
        let before = self.segs.len();
        let mut retired = 0u64;
        self.segs.retain(|s| {
            let fully_flushed = flushed(s.start, s.end) >= s.end && s.cursor >= s.end;
            if fully_flushed {
                retired += s.end - s.start;
            }
            !fully_flushed
        });
        self.retired += retired;
        before - self.segs.len()
    }

    #[cfg(test)]
    pub(super) fn check_invariants(&self) -> Result<(), String> {
        let mut sorted: Vec<&Segment> = self.segs.iter().collect();
        sorted.sort_by_key(|s| s.start);
        for pair in sorted.windows(2) {
            if pair[0].end > pair[1].start {
                return Err(format!(
                    "段区间重叠: [{}, {}) vs [{}, {})",
                    pair[0].start, pair[0].end, pair[1].start, pair[1].end
                ));
            }
        }
        for s in &self.segs {
            if s.start >= s.end {
                return Err(format!("空段或倒序段: [{}, {})", s.start, s.end));
            }
            if s.cursor < s.start || s.cursor > s.end {
                return Err(format!(
                    "cursor 越界: {} ∉ [{}, {})",
                    s.cursor, s.start, s.end
                ));
            }
            if s.reserved < s.cursor || s.reserved > s.end {
                return Err(format!(
                    "reserved 违反 cursor ≤ reserved ≤ end: cursor={} reserved={} end={}",
                    s.cursor, s.reserved, s.end
                ));
            }
        }
        let covered = self.pre_committed
            + self.retired
            + self.segs.iter().map(|s| s.end - s.start).sum::<u64>();
        if covered != self.total {
            return Err(format!("Σ段长 {covered} ≠ 文件大小 {}", self.total));
        }
        Ok(())
    }
}

impl Segment {
    pub(super) fn done(&self) -> bool {
        self.worker.is_none() && self.cursor >= self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_planner_spans_whole_file() {
        let p = Planner::new(10_000, 256 * 1024, 4096, MIN_PREEMPT);
        assert_eq!(p.completed_bytes(), 0);
        assert_eq!(p.segments().len(), 1);
        assert_eq!(p.segments()[0].start, 0);
        assert_eq!(p.segments()[0].end, 10_000);
        p.check_invariants().unwrap();
    }

    #[test]
    fn assign_claim_commit_drive_to_completion() {
        let mut p = Planner::new(10_000, 2_048, 4096, MIN_PREEMPT);
        let a = p.assign(1, None).unwrap();
        assert_eq!(a.from, 0);
        let mut at = 0u64;
        loop {
            match p.claim(1, at, 1_000) {
                Claim::Granted { n, .. } => {
                    at += n;
                    if let Some(_end) = p.commit(1, at) {
                        break;
                    }
                }
                Claim::Lost => panic!("无竞争不应丢失"),
            }
        }
        assert_eq!(p.completed_bytes(), 10_000);
        p.check_invariants().unwrap();
        assert!(p.assign(2, None).is_none(), "全部提交后无活可领");
    }

    #[test]
    fn split_gives_tail_to_second_worker() {
        let mut p = Planner::new(10_000, 2_048, 4096, MIN_PREEMPT);
        let a0 = p.assign(1, None).unwrap();
        assert_eq!(a0.from, 0);
        let a1 = p.assign(2, None).unwrap();
        assert!(a1.from > 0, "第二 worker 应从切分点领取");
        assert_eq!(p.segments().len(), 2);
        let seg0 = &p.segments()[0];
        let seg1 = &p.segments()[1];
        assert_eq!(seg0.end, a1.from, "切分点 = 原段新 end");
        assert!(seg1.end - seg1.start >= 2_048, "新段不小于 min_split");
        assert!(seg0.end >= seg0.reserved, "切分不越过原段已认领字节");
        p.check_invariants().unwrap();
        let covered: u64 = p.segments().iter().map(|s| s.end - s.start).sum();
        assert_eq!(covered, 10_000);
    }

    #[test]
    fn split_respects_claimed_region() {
        let mut p = Planner::new(100_000, 4_096, 4096, MIN_PREEMPT);
        p.assign(1, None).unwrap();
        assert!(matches!(p.claim(1, 0, 50_000), Claim::Granted { .. }));
        let reserved_before = p.segments()[0].reserved;
        let a2 = p.assign(2, None).unwrap();
        assert!(a2.from >= reserved_before, "切分点绝不越过他人已认领字节");
        let seg0 = &p.segments()[0];
        assert!(seg0.end >= reserved_before);
        p.check_invariants().unwrap();
    }

    #[test]
    fn claim_refunds_after_segment_shortened() {
        let mut p = Planner::new(10_000, 2_048, 4096, MIN_PREEMPT);
        p.assign(1, None).unwrap();
        p.assign(2, None).unwrap();
        let seg1_end = p.segments()[1].end;
        let n = match p.claim(1, 0, 10_000) {
            Claim::Granted { n, .. } => n,
            Claim::Lost => panic!("持有段内认领应授予"),
        };
        assert!(n <= seg1_end.min(10_000), "授予不超过段尾");
        p.commit(1, n);
        p.check_invariants().unwrap();
    }

    #[test]
    fn claim_lost_when_stolen_or_empty() {
        let mut p = Planner::new(4_096, 2_048, 4096, MIN_PREEMPT);
        p.assign(1, None).unwrap();
        match p.claim(2, 0, 100) {
            Claim::Lost => {}
            Claim::Granted { .. } => panic!("他人持有段认领应丢失"),
        }
        let mut p2 = Planner::new(100, 64, 4096, MIN_PREEMPT);
        p2.assign(1, None).unwrap();
        p2.commit(1, 100);
        assert!(matches!(p2.claim(1, 100, 10), Claim::Lost), "段外认领丢失");
    }

    #[test]
    fn release_returns_unclaimed_and_keeps_cursor() {
        let mut p = Planner::new(10_000, 2_048, 4096, MIN_PREEMPT);
        p.assign(1, None).unwrap();
        assert!(matches!(p.claim(1, 0, 5_000), Claim::Granted { .. }));
        p.commit(1, 3_000);
        p.release(1);
        let s = &p.segments()[0];
        assert_eq!(s.cursor, 3_000, "已提交游标保留");
        assert_eq!(s.reserved, 3_000, "未提交认领退款");
        assert!(s.worker.is_none());
        p.check_invariants().unwrap();
        let a = p.assign(2, None).unwrap();
        assert_eq!(a.from, 3_000, "续接从已提交游标开始");
    }

    #[test]
    fn preempt_steals_silent_assignment() {
        let mut p = Planner::new(10_000, 2_048, 4096, Duration::ZERO);
        let a1 = p.assign(1, None).unwrap();
        assert_eq!(a1.from, 0);
        let a2 = p.assign(2, None).unwrap();
        let cut = a2.from;
        let a3 = p.assign(3, None).unwrap();
        assert!(a3.from >= cut, "抢占或再切分仍推进");
        p.check_invariants().unwrap();
        match p.claim(1, 0, 10) {
            Claim::Lost => {}
            Claim::Granted { .. } => panic!("被抢占后原 worker 认领应丢失"),
        }
    }

    #[test]
    fn preempt_respects_receiving() {
        let mut p = Planner::new(10_000, 2_048, 4096, Duration::ZERO);
        p.assign(1, None).unwrap();
        assert!(matches!(p.claim(1, 0, 100), Claim::Granted { .. }));
        p.commit(1, 100);
        let a2 = p.assign(2, None).unwrap();
        assert!(a2.from >= 2_048, "已在接收的段不被抢占（走切分）");
        let s0 = &p.segments()[0];
        assert_eq!(s0.worker, Some(1), "活跃段所有权不变");
    }

    #[test]
    fn absorb_merges_adjacent_unclaimed() {
        let mut p = Planner::new(12_000, 4_096, 4096, MIN_PREEMPT);
        p.assign(1, None).unwrap();
        let a2 = p.assign(2, None).unwrap();
        let cut = a2.from;
        p.release(2);
        assert!(
            p.absorb_next(1, cut).is_some(),
            "相邻无人认领段可被整文件流吸收"
        );
        p.check_invariants().unwrap();
        let s = p.segments().iter().find(|s| s.start == cut).unwrap();
        assert_eq!(s.worker, Some(1), "吸收后 worker 1 持有相邻段");
        assert!(p.absorb_next(1, cut).is_none(), "重复吸收无相邻段返回 None");
    }

    #[test]
    fn min_split_halves_under_tail_pressure() {
        let mut p = Planner::new(10_240, 8_192, 4096, MIN_PREEMPT);
        p.assign(1, None).unwrap();
        let a2 = p.assign(2, None).unwrap();
        assert!(a2.from > 0, "尾段小于 2×min_split 时自适应降半仍可切分");
        p.check_invariants().unwrap();
    }

    #[test]
    fn from_gaps_resume_starts_inside_partial_block_residues() {
        let mb = 1_048_576u64;
        let mut p = Planner::from_gaps(
            8 * mb,
            &[
                (246_000, mb),
                (mb + 246_000, 2 * mb),
                (2 * mb + 246_000, 3 * mb),
            ],
            64 * 1024,
            4096,
            MIN_PREEMPT,
        );
        let a1 = p.assign(1, None).unwrap();
        assert_eq!(
            a1.from, 246_000,
            "首分配 = 首个缺口起点（黄块残留字节，块 0 内部）"
        );
        let a2 = p.assign(2, None).unwrap();
        assert_eq!(a2.from, mb + 246_000, "次分配 = 次缺口起点（块 1 残留）");
        let a3 = p.assign(3, None).unwrap();
        assert_eq!(a3.from, 2 * mb + 246_000, "第三分配 = 块 2 残留");
        p.check_invariants().unwrap();
    }

    #[test]
    fn from_gaps_reconstructs_resume_state() {
        let mut p = Planner::from_gaps(
            10_000,
            &[(2_000, 5_000), (6_000, 10_000)],
            2_048,
            4096,
            MIN_PREEMPT,
        );
        assert_eq!(p.completed_bytes(), 3_000, "续传起步 = sidecar 已提交字节");
        let a = p.assign(1, None).unwrap();
        assert_eq!(a.from, 2_000, "首个缺口起点 = 首个未提交字节");
        let covered: u64 = p.segments().iter().map(|s| s.end - s.start).sum();
        assert_eq!(covered, 7_000, "段覆盖 = 未下载缺口总量");
        p.check_invariants().unwrap();
    }

    #[test]
    fn zero_size_planner_has_no_segments() {
        let mut p = Planner::new(0, 2_048, 4096, MIN_PREEMPT);
        assert_eq!(p.completed_bytes(), 0);
        assert!(p.segments().is_empty());
        assert!(p.assign(1, None).is_none());
    }

    #[test]
    fn committed_ranges_covers_partial_and_done_segments() {
        let mut p = Planner::new(10_000, 2_048, 4096, MIN_PREEMPT);
        assert!(p.committed_ranges().is_empty(), "无提交无区间");
        p.assign(1, None).unwrap();
        let a2 = p.assign(2, None).unwrap();
        let cut = a2.from;
        assert!(matches!(p.claim(1, 0, cut), Claim::Granted { .. }));
        p.commit(1, cut);
        assert!(matches!(p.claim(2, cut, 3_000), Claim::Granted { .. }));
        p.commit(2, cut + 3_000);
        assert_eq!(
            p.committed_ranges(),
            vec![(0, cut), (cut, cut + 3_000)],
            "整段提交 + 段内部分提交各自成区间"
        );
        let expected_sum: u64 = p.committed_ranges().iter().map(|(a, b)| b - a).sum();
        assert_eq!(
            expected_sum,
            cut + 3_000,
            "区间字节数和 = completed_bytes 活段部分"
        );
        p.check_invariants().unwrap();
    }

    #[test]
    fn gc_removes_fully_flushed_done_segments() {
        let mut p = Planner::new(10_000, 2_048, 4096, MIN_PREEMPT);
        p.assign(1, None).unwrap();
        assert!(matches!(p.claim(1, 0, 10_000), Claim::Granted { .. }));
        p.commit(1, 10_000);
        assert_eq!(p.completed_bytes(), 10_000);
        let removed = p.gc(|_s, _e| 0);
        assert_eq!(removed, 0, "未 flush 的完成段保留（sidecar 仍需其区间）");
        let removed = p.gc(|_s, e| e);
        assert_eq!(removed, 1, "全 flush 的完成段可回收");
        assert_eq!(p.completed_bytes(), 10_000, "回收后聚合游标不变");
        p.check_invariants().unwrap();
    }

    #[test]
    fn split_point_alignment() {
        let mut p = Planner::new(1_000_000, 1_024, 4096, MIN_PREEMPT);
        p.assign(1, None).unwrap();
        let a2 = p.assign(2, None).unwrap();
        assert_eq!(a2.from % 4096, 0, "切分点按 4096 对齐");
        p.check_invariants().unwrap();
    }

    #[test]
    fn commit_rejects_out_of_range() {
        let mut p = Planner::new(1_000, 512, 4096, MIN_PREEMPT);
        p.assign(1, None).unwrap();
        assert!(p.commit(1, 2_000).is_none(), "越过段尾提交拒绝");
        assert!(p.commit(2, 10).is_none(), "非持有者提交拒绝");
        p.check_invariants().unwrap();
    }

    #[test]
    fn split_cut_lands_on_block_boundary_when_available() {
        let mb = 1_048_576u64;
        let mut p = Planner::new(8 * mb, 256 * 1024, mb, MIN_PREEMPT);
        p.assign(1, None).unwrap();
        assert!(matches!(p.claim(1, 0, 4_900_000), Claim::Granted { .. }));
        p.commit(1, 4_900_000);
        let a2 = p.assign(2, None).unwrap();
        assert_eq!(a2.from % mb, 0, "切分点必须落在块边界，后继租约块对齐");
        assert_eq!(a2.from, 6 * mb, "取理想切点最近的可行块边界");
        p.check_invariants().unwrap();
    }

    #[test]
    fn split_without_internal_boundary_is_refused() {
        let mb = 1_048_576u64;
        let mut p = Planner::new(3 * mb + 512 * 1024, 256 * 1024, mb, MIN_PREEMPT);
        p.assign(1, None).unwrap();
        assert!(matches!(p.claim(1, 0, 3 * mb), Claim::Granted { .. }));
        p.commit(1, 3 * mb);
        let a2 = p.assign(2, None);
        assert!(
            a2.is_none(),
            "段内无块边界时不得产生块中切分（尾段单 worker 收尾）"
        );
        p.check_invariants().unwrap();
    }

    #[test]
    fn absorb_next_skips_partially_committed_segment() {
        let mut p = Planner::new(12_000, 4_096, 4096, MIN_PREEMPT);
        p.assign(1, None).unwrap();
        let a2 = p.assign(2, None).unwrap();
        let cut = a2.from;
        assert!(matches!(p.claim(2, cut, 1_000), Claim::Granted { .. }));
        p.commit(2, cut + 600);
        p.release(2);
        assert!(
            p.absorb_next(1, cut).is_none(),
            "部分提交段不得被吸收重置（已提交字节必须续传）"
        );
        let a = p.assign(1, Some(cut)).unwrap();
        assert_eq!(a.from, cut + 600, "改走分配路径从已提交游标续传");
        p.check_invariants().unwrap();
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::*;

    #[derive(Debug, Clone)]
    enum Op {
        Assign(WorkerId, Option<u64>),
        Claim(WorkerId, u64, u64),
        Commit(WorkerId, u64),
        Release(WorkerId),
        Absorb(WorkerId, u64),
    }

    fn op_strategy() -> impl Strategy<Value = Op> {
        prop_oneof![
            (1usize..8, proptest::option::of(0u64..200_000u64))
                .prop_map(|(w, after)| Op::Assign(w, after)),
            (1usize..8, 0u64..200_000u64, 1u64..40_000u64)
                .prop_map(|(w, pos, len)| Op::Claim(w, pos, len)),
            (1usize..8, 0u64..200_000u64).prop_map(|(w, upto)| Op::Commit(w, upto)),
            (1usize..8).prop_map(Op::Release),
            (1usize..8, 0u64..200_000u64).prop_map(|(w, at)| Op::Absorb(w, at)),
        ]
    }

    proptest! {
        #[test]
        fn prop_planner_invariants_under_random_ops(
            total in 1u64..200_000u64,
            min_split in 1u64..64_000u64,
            ops in prop::collection::vec(op_strategy(), 0..120),
        ) {
            let mut p = Planner::new(total, min_split, 4096, Duration::ZERO);
            p.check_invariants().unwrap();
            for op in &ops {
                match op {
                    Op::Assign(w, after) => {
                        let _ = p.assign(*w, *after);
                    }
                    Op::Claim(w, pos, len) => {
                        let _ = p.claim(*w, *pos, *len);
                    }
                    Op::Commit(w, upto) => {
                        let _ = p.commit(*w, *upto);
                    }
                    Op::Release(w) => p.release(*w),
                    Op::Absorb(w, at) => {
                        let _ = p.absorb_next(*w, *at);
                    }
                }
                if let Err(e) = p.check_invariants() {
                    prop_assert!(false, "total={total} min_split={min_split} op={op:?}: {e}");
                }
            }
            prop_assert!(p.completed_bytes() <= total, "聚合游标不越过文件大小");
        }
    }

    proptest! {
        #[test]
        fn prop_planner_completion_liveness(
            total in 1u64..100_000u64,
            min_split in 512u64..32_000u64,
            n_workers in 1usize..6usize,
            chunk_seed in 1u64..100u64,
        ) {
            let mut p = Planner::new(total, min_split, 4096, MIN_PREEMPT);
            let mut ats: Vec<Option<u64>> = vec![None; n_workers];
            let mut exited = vec![false; n_workers];
            let mut steps = 0usize;
            while p.completed_bytes() < total && steps < 50_000 {
                steps += 1;
                let w = (steps % n_workers) + 1;
                if exited[w - 1] {
                    continue;
                }
                if ats[w - 1].is_none() {
                    match p.assign(w, None) {
                        Some(a) => ats[w - 1] = Some(a.from),
                        None => {
                            exited[w - 1] = true;
                            continue;
                        }
                    }
                }
                let at = ats[w - 1].unwrap();
                let chunk = 1 + (chunk_seed * steps as u64 % 8_192);
                match p.claim(w, at, chunk) {
                    Claim::Granted { n, .. } => {
                        let upto = at + n;
                        if let Some(end) = p.commit(w, upto) {
                            if p.absorb_next(w, end).is_none() {
                                ats[w - 1] = None;
                            } else {
                                ats[w - 1] = Some(end);
                            }
                        } else {
                            ats[w - 1] = Some(upto);
                        }
                    }
                    Claim::Lost => {
                        ats[w - 1] = None;
                    }
                }
            }
            prop_assert_eq!(
                p.completed_bytes(),
                total,
                "公平轮转下聚合游标应收敛到文件大小"
            );
        }
    }
}
