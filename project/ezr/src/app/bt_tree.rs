use super::Dialog;
use crate::model::BtFile;

#[derive(Clone, Debug)]
pub struct BtNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub depth: usize,
    pub expanded: bool,
    pub selected: bool,
    pub partial: bool,
    pub children: Vec<BtNode>,
}

#[must_use]
pub fn bt_build_tree(files: &[BtFile]) -> Vec<BtNode> {
    let mut roots: Vec<BtNode> = Vec::new();
    for f in files {
        let segs: Vec<&str> = f.path.split('/').filter(|s| !s.is_empty()).collect();
        bt_tree_insert(&mut roots, &segs, 0, "", f.size, f.selected);
    }
    bt_sync_dir_states(&mut roots);
    roots
}

fn bt_tree_insert(
    nodes: &mut Vec<BtNode>,
    segs: &[&str],
    depth: usize,
    prefix: &str,
    size: u64,
    selected: bool,
) {
    let Some(name) = segs.first() else {
        return;
    };
    let path = if prefix.is_empty() {
        (*name).to_string()
    } else {
        format!("{prefix}/{name}")
    };
    if segs.len() == 1 {
        nodes.push(BtNode {
            name: (*name).to_string(),
            path,
            is_dir: false,
            size,
            depth,
            expanded: false,
            selected,
            partial: false,
            children: Vec::new(),
        });
        return;
    }
    if let Some(dir) = nodes.iter_mut().find(|n| n.is_dir && n.name == *name) {
        bt_tree_insert(
            &mut dir.children,
            &segs[1..],
            depth + 1,
            &path,
            size,
            selected,
        );
    } else {
        let mut dir = BtNode {
            name: (*name).to_string(),
            path: path.clone(),
            is_dir: true,
            size: 0,
            depth,
            expanded: false,
            selected: false,
            partial: false,
            children: Vec::new(),
        };
        bt_tree_insert(
            &mut dir.children,
            &segs[1..],
            depth + 1,
            &path,
            size,
            selected,
        );
        nodes.push(dir);
    }
}

pub(super) fn bt_sync_dir_states(nodes: &mut [BtNode]) {
    for n in nodes.iter_mut() {
        if n.is_dir {
            bt_sync_dir_states(&mut n.children);
            n.size = n.children.iter().map(|c| c.size).sum();
            let (sel, total) = bt_count_files(&n.children);
            n.selected = total > 0 && sel == total;
            n.partial = sel > 0 && sel < total;
        }
    }
}

pub(super) fn bt_set_all(nodes: &mut [BtNode], on: bool) {
    for n in nodes.iter_mut() {
        if n.is_dir {
            bt_set_all(&mut n.children, on);
        } else {
            n.selected = on;
        }
    }
}

pub(super) fn bt_invert_all(nodes: &mut [BtNode]) {
    for n in nodes.iter_mut() {
        if n.is_dir {
            bt_invert_all(&mut n.children);
        } else {
            n.selected = !n.selected;
        }
    }
}

pub(super) fn bt_toggle_node(nodes: &mut [BtNode], path: &[usize]) {
    let Some(&idx) = path.first() else {
        return;
    };
    let Some(n) = nodes.get_mut(idx) else {
        return;
    };
    if path.len() == 1 {
        if n.is_dir {
            let on = !(n.selected && !n.partial);
            bt_set_all(&mut n.children, on);
        } else {
            n.selected = !n.selected;
        }
    } else {
        bt_toggle_node(&mut n.children, &path[1..]);
    }
}

pub(super) fn bt_set_expanded(nodes: &mut [BtNode], path: &[usize], expanded: bool) {
    let Some(&idx) = path.first() else {
        return;
    };
    let Some(n) = nodes.get_mut(idx) else {
        return;
    };
    if path.len() == 1 {
        if n.is_dir {
            n.expanded = expanded;
        }
    } else {
        bt_set_expanded(&mut n.children, &path[1..], expanded);
    }
}

pub(super) fn bt_visible_rows<'a>(
    nodes: &'a [BtNode],
    prefix: &mut Vec<usize>,
    out: &mut Vec<(Vec<usize>, &'a BtNode)>,
) {
    for (i, n) in nodes.iter().enumerate() {
        prefix.push(i);
        out.push((prefix.clone(), n));
        if n.is_dir && n.expanded {
            bt_visible_rows(&n.children, prefix, out);
        }
        prefix.pop();
    }
}

#[must_use]
pub fn bt_row_count(nodes: &[BtNode]) -> usize {
    nodes
        .iter()
        .map(|n| {
            1 + if n.is_dir && n.expanded {
                bt_row_count(&n.children)
            } else {
                0
            }
        })
        .sum()
}

#[must_use]
pub fn bt_count_files(nodes: &[BtNode]) -> (usize, usize) {
    let mut sel = 0usize;
    let mut total = 0usize;
    for n in nodes {
        if n.is_dir {
            let (s, t) = bt_count_files(&n.children);
            sel += s;
            total += t;
        } else {
            total += 1;
            if n.selected {
                sel += 1;
            }
        }
    }
    (sel, total)
}

#[must_use]
pub fn bt_selected_bytes(nodes: &[BtNode]) -> u64 {
    nodes
        .iter()
        .map(|n| {
            if n.is_dir {
                bt_selected_bytes(&n.children)
            } else if n.selected {
                n.size
            } else {
                0
            }
        })
        .sum()
}

pub(super) fn bt_tree_files(nodes: &[BtNode], out: &mut Vec<BtFile>) {
    for n in nodes {
        if n.is_dir {
            bt_tree_files(&n.children, out);
        } else {
            out.push(BtFile {
                path: n.path.clone(),
                size: n.size,
                selected: n.selected,
            });
        }
    }
}

impl Dialog {
    #[must_use]
    pub(crate) fn bt_rows(&self) -> Vec<(Vec<usize>, &BtNode)> {
        let mut out = Vec::new();
        let mut prefix = Vec::new();
        bt_visible_rows(&self.bt_tree, &mut prefix, &mut out);
        out
    }

    #[must_use]
    pub fn bt_row_count(&self) -> usize {
        bt_row_count(&self.bt_tree)
    }

    pub(super) fn bt_list_move(&mut self, delta: i32) {
        self.bt_follow = true;
        let n = self.bt_row_count() as i32;
        if n <= 0 {
            self.bt_cursor = 0;
            return;
        }
        self.bt_cursor = (self.bt_cursor as i32 + delta).clamp(0, n - 1) as usize;
    }

    pub(crate) fn bt_scroll_follow(&mut self, visible: usize) {
        let vis = visible.max(1);
        let n = self.bt_row_count();
        if self.bt_cursor < self.bt_scroll {
            self.bt_scroll = self.bt_cursor;
        }
        if self.bt_cursor >= self.bt_scroll + vis {
            self.bt_scroll = self.bt_cursor + 1 - vis;
        }
        let max = n.saturating_sub(vis);
        self.bt_scroll = self.bt_scroll.min(max);
    }

    fn bt_path_at_cursor(&self) -> Option<Vec<usize>> {
        self.bt_rows().get(self.bt_cursor).map(|(p, _)| p.clone())
    }

    pub(super) fn bt_toggle_cursor(&mut self) {
        if let Some(path) = self.bt_path_at_cursor() {
            bt_toggle_node(&mut self.bt_tree, &path);
            bt_sync_dir_states(&mut self.bt_tree);
        }
    }

    pub(super) fn bt_toggle_expand_at_cursor(&mut self) {
        let Some(path) = self.bt_path_at_cursor() else {
            return;
        };
        let expanded = self
            .bt_rows()
            .get(self.bt_cursor)
            .map(|(_, n)| n.expanded)
            .unwrap_or(false);
        bt_set_expanded(&mut self.bt_tree, &path, !expanded);
        self.bt_cursor = self.bt_cursor.min(self.bt_row_count().saturating_sub(1));
        self.bt_follow = true;
    }

    pub(super) fn bt_expand_cursor(&mut self, expand: bool) {
        if let Some(path) = self.bt_path_at_cursor() {
            bt_set_expanded(&mut self.bt_tree, &path, expand);
            self.bt_cursor = self.bt_cursor.min(self.bt_row_count().saturating_sub(1));
            self.bt_follow = true;
        }
    }

    pub(super) fn bt_select_all(&mut self, on: bool) {
        bt_set_all(&mut self.bt_tree, on);
        bt_sync_dir_states(&mut self.bt_tree);
    }

    pub(super) fn bt_invert(&mut self) {
        bt_invert_all(&mut self.bt_tree);
        bt_sync_dir_states(&mut self.bt_tree);
    }

    #[must_use]
    pub(crate) fn bt_counts(&self) -> (usize, usize) {
        bt_count_files(&self.bt_tree)
    }

    #[must_use]
    pub(crate) fn bt_selected_bytes(&self) -> u64 {
        bt_selected_bytes(&self.bt_tree)
    }

    #[must_use]
    pub(super) fn bt_flatten(&self) -> Vec<BtFile> {
        let mut out = Vec::new();
        bt_tree_files(&self.bt_tree, &mut out);
        out
    }

    #[must_use]
    pub(crate) fn bt_list_focus(kind: super::DialogKind) -> usize {
        match kind {
            super::DialogKind::AddBt => 2,
            super::DialogKind::ModifyBt => 0,
            _ => 0,
        }
    }

    #[must_use]
    pub(crate) fn trackers_focus(&self) -> Option<usize> {
        match self.kind {
            super::DialogKind::AddBt => Some(5),
            super::DialogKind::ModifyBt => Some(3),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(path: &str, size: u64, selected: bool) -> BtFile {
        BtFile {
            path: path.to_string(),
            size,
            selected,
        }
    }

    #[test]
    fn build_tree_aggregates_dirs_and_defaults_collapsed() {
        let tree = bt_build_tree(&[
            f("tor/sub/a.txt", 5, true),
            f("tor/sub/b.bin", 7, false),
            f("tor/c.iso", 11, true),
            f("loose.dat", 3, true),
        ]);
        assert_eq!(tree.len(), 2);
        let tor = &tree[0];
        assert!(tor.is_dir && tor.name == "tor" && tor.depth == 0);
        assert_eq!(tor.size, 23, "目录 size = 子树合计");
        assert!(!tor.selected && tor.partial, "部分选择 → [-]");
        assert!(!tor.expanded, "默认收拢");
        let sub = tor
            .children
            .iter()
            .find(|n| n.name == "sub")
            .expect("sub 目录");
        assert_eq!(sub.size, 12);
        assert_eq!(bt_row_count(&tree), 2, "收拢态只投影根层");
        assert_eq!(bt_count_files(&tree), (3, 4));
        assert_eq!(bt_selected_bytes(&tree), 19, "已选 5+11+3（收拢同样计入）");
    }

    #[test]
    fn toggle_and_expand_semantics() {
        let mut tree = bt_build_tree(&[f("d/a", 1, false), f("d/b", 2, false), f("c", 4, true)]);
        bt_set_expanded(&mut tree, &[0], true);
        assert_eq!(bt_row_count(&tree), 4, "d + a + b + c");
        let rows: Vec<(Vec<usize>, String)> = {
            let mut prefix: Vec<usize> = Vec::new();
            let mut refs: Vec<(Vec<usize>, &crate::app::bt_tree::BtNode)> = Vec::new();
            bt_visible_rows(&tree, &mut prefix, &mut refs);
            refs.into_iter()
                .map(|(path, node)| (path, node.name.clone()))
                .collect()
        };
        assert_eq!(rows[1].1, "a", "展开后子节点按深度优先投影");
        bt_toggle_node(&mut tree, &rows[1].0);
        bt_sync_dir_states(&mut tree);
        assert!(tree[0].partial, "1/2 选中 = 部分");
        bt_toggle_node(&mut tree, &[0]);
        bt_sync_dir_states(&mut tree);
        assert!(tree[0].selected && !tree[0].partial, "部分 → 全选");
        let (sel, _total) = bt_count_files(&tree);
        assert_eq!(sel, 3, "a/b 全部选中 + c");
        bt_toggle_node(&mut tree, &[0]);
        bt_sync_dir_states(&mut tree);
        assert_eq!(bt_count_files(&tree).0, 1, "目录全不选后仅剩 c");
        bt_invert_all(&mut tree);
        bt_sync_dir_states(&mut tree);
        assert_eq!(bt_count_files(&tree).0, 2, "反选 → a/b");
        bt_set_all(&mut tree, false);
        bt_sync_dir_states(&mut tree);
        assert_eq!(bt_count_files(&tree).0, 0);
    }

    #[test]
    fn flatten_preserves_insertion_order_and_merges_dirs() {
        let mut tree = bt_build_tree(&[f("x/a", 1, true), f("x/b", 2, false), f("x/a", 0, true)]);
        bt_set_expanded(&mut tree, &[0], true);
        bt_set_all(&mut tree, true);
        bt_sync_dir_states(&mut tree);
        let mut out = Vec::new();
        bt_tree_files(&tree, &mut out);
        assert_eq!(out.len(), 3, "插入序摊平（不去重，种子元数据保唯一）");
        assert_eq!(out[0].path, "x/a");
        assert_eq!(out[1].path, "x/b");
        assert!(out.iter().all(|ff| ff.selected), "全选后摊平全 true");
    }
}
