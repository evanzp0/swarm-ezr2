use crate::model::bt::BtFile;

#[derive(Clone, Debug)]
pub struct PieceMap {
    pub piece_length: u32,
    pub files: Vec<BtFile>,
    file_offsets: Vec<u64>,
    covered: Vec<u32>,
    total_all: u64,
}

impl PieceMap {
    #[must_use]
    pub fn new(files: &[BtFile], piece_length: u32) -> PieceMap {
        let pl = u64::from(piece_length).max(1);
        let mut file_offsets = Vec::with_capacity(files.len());
        let mut acc = 0u64;
        let mut covered_set = std::collections::BTreeSet::new();
        for f in files {
            file_offsets.push(acc);
            if f.selected && f.size > 0 {
                let first = acc / pl;
                let last = (acc + f.size - 1) / pl;
                for p in first..=last {
                    covered_set.insert(p as u32);
                }
            }
            acc += f.size;
        }
        let total_all = acc;
        PieceMap {
            piece_length,
            files: files.to_vec(),
            file_offsets,
            covered: covered_set.into_iter().collect(),
            total_all,
        }
    }

    #[must_use]
    pub fn covered_pieces(&self) -> &[u32] {
        &self.covered
    }

    #[must_use]
    pub fn selected_total(&self) -> u64 {
        self.files
            .iter()
            .filter(|f| f.selected)
            .map(|f| f.size)
            .sum()
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn total_all(&self) -> u64 {
        self.total_all
    }

    #[must_use]
    pub fn num_pieces(&self) -> u32 {
        let pl = u64::from(self.piece_length).max(1);
        (self.total_all.div_ceil(pl).max(1)) as u32
    }

    #[must_use]
    pub fn piece_len(&self, piece: u32) -> u64 {
        let pl = u64::from(self.piece_length).max(1);
        let start = u64::from(piece) * pl;
        (self.total_all - start).min(pl)
    }

    #[must_use]
    pub fn write_plan(&self, piece: u32) -> Vec<(usize, u64, u64, u64)> {
        let pl = u64::from(self.piece_length).max(1);
        let p_start = u64::from(piece) * pl;
        let p_end = p_start + self.piece_len(piece);
        let mut out = Vec::new();
        for (i, f) in self.files.iter().enumerate() {
            if !f.selected || f.size == 0 {
                continue;
            }
            let f_start = self.file_offsets[i];
            let f_end = f_start + f.size;
            let lo = p_start.max(f_start);
            let hi = p_end.min(f_end);
            if lo < hi {
                out.push((i, lo - f_start, lo - p_start, hi - lo));
            }
        }
        out
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn pieces_of_file(&self, file_idx: usize) -> (u32, u32) {
        let pl = u64::from(self.piece_length).max(1);
        let start = self.file_offsets.get(file_idx).copied().unwrap_or(0);
        let size = self.files.get(file_idx).map_or(0, |f| f.size);
        if size == 0 {
            return (0, 0);
        }
        let first = start / pl;
        let last = (start + size - 1) / pl;
        (first as u32, last as u32)
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn recompute_covered(&self) -> Vec<u32> {
        PieceMap::new(&self.files, self.piece_length).covered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bf(path: &str, size: u64, selected: bool) -> BtFile {
        BtFile {
            path: path.to_string(),
            size,
            selected,
        }
    }

    fn layout() -> Vec<BtFile> {
        vec![
            bf("t/f0", 100, true),
            bf("t/f1", 150, true),
            bf("t/f2", 50, false),
            bf("t/f3", 100, true),
        ]
    }

    #[test]
    fn covered_pieces_and_totals() {
        let m = PieceMap::new(&layout(), 100);
        assert_eq!(m.covered_pieces(), &[0, 1, 2, 3]);
        assert_eq!(m.selected_total(), 350);
        assert_eq!(m.total_all(), 400);
        assert_eq!(m.num_pieces(), 4);
    }

    #[test]
    fn unselected_middle_piece_excluded() {
        let files = vec![bf("a", 100, true), bf("b", 100, false), bf("c", 100, true)];
        let m = PieceMap::new(&files, 100);
        assert_eq!(m.covered_pieces(), &[0, 2]);
        assert!(m.write_plan(1).is_empty());
    }

    #[test]
    fn write_plan_skips_unselected_bytes() {
        let m = PieceMap::new(&layout(), 100);
        let plan = m.write_plan(2);
        assert_eq!(plan, vec![(1, 100, 0, 50)], "未选文件不写入");
        assert_eq!(m.write_plan(3), vec![(3, 0, 0, 100)]);
    }

    #[test]
    fn last_piece_absorbs_remainder() {
        let m = PieceMap::new(&layout(), 100);
        assert_eq!(m.piece_len(3), 100);
        let files = vec![bf("a", 250, true)];
        let m2 = PieceMap::new(&files, 100);
        assert_eq!(m2.num_pieces(), 3);
        assert_eq!(m2.piece_len(2), 50);
    }

    #[test]
    fn selection_change_recompute() {
        let mut files = layout();
        let m = PieceMap::new(&files, 100);
        assert_eq!(m.covered_pieces(), &[0, 1, 2, 3]);
        files[1].selected = false;
        let m2 = PieceMap::new(&files, 100);
        assert_eq!(m2.covered_pieces(), &[0, 3]);
        assert_eq!(m2.selected_total(), 200);
    }

    #[test]
    fn pieces_of_file_range() {
        let m = PieceMap::new(&layout(), 100);
        assert_eq!(m.pieces_of_file(0), (0, 0));
        assert_eq!(m.pieces_of_file(1), (1, 2));
        assert_eq!(m.pieces_of_file(3), (3, 3));
    }
}
