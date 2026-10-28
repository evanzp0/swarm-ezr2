use crate::textfield::{w, wrap_segments, TextInput};

pub const MULTI_MAX_LINES: usize = 64;

pub type VisualPos = (Vec<(usize, usize, usize)>, usize, usize);

#[derive(Debug)]
pub struct MultiLineBuf {
    pub lines: Vec<TextInput>,

    pub row: usize,

    pub view: usize,

    pub view_x: usize,

    pub follow: bool,

    pub max_scroll: usize,

    pub max_scroll_x: usize,

    pub text_w: usize,

    pub wrap: bool,
}

impl Default for MultiLineBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiLineBuf {
    #[must_use]
    pub fn new() -> Self {
        Self {
            lines: vec![TextInput::new()],
            row: 0,
            view: 0,
            view_x: 0,
            follow: true,
            max_scroll: 0,
            max_scroll_x: 0,
            text_w: 0,
            wrap: true,
        }
    }

    pub fn reset(&mut self) {
        self.lines = vec![TextInput::new()];
        self.row = 0;
        self.view = 0;
        self.view_x = 0;
        self.follow = true;
        self.max_scroll = 0;
        self.max_scroll_x = 0;
        self.text_w = 0;
        self.wrap = true;
    }

    pub fn set_text(&mut self, text: &str) {
        self.reset();
        self.insert_str(text);
        self.follow = false;
        self.view = 0;
    }

    pub fn line(&self) -> &TextInput {
        &self.lines[self.row.min(self.lines.len() - 1)]
    }

    pub fn line_mut(&mut self) -> &mut TextInput {
        let row = self.row.min(self.lines.len() - 1);
        &mut self.lines[row]
    }

    #[must_use]
    pub fn is_buffer_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].value.is_empty()
    }

    #[must_use]
    pub fn trim_lines(&self) -> Vec<String> {
        self.lines
            .iter()
            .map(|l| l.value.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    }

    pub fn newline(&mut self) -> bool {
        if self.lines.len() >= MULTI_MAX_LINES {
            return false;
        }
        let rest = self.line_mut().split_at_cursor();
        self.lines.insert(self.row + 1, rest);
        self.row += 1;
        self.follow = true;
        true
    }

    pub fn backspace(&mut self) {
        self.follow = true;
        if self.row == 0 {
            self.line_mut().backspace();
            return;
        }
        let at_start = self.line().cursor == 0;
        if at_start {
            let cur = self.lines.remove(self.row);
            self.row -= 1;
            self.lines[self.row].cursor_end();
            self.lines[self.row].insert_at_cursor(&cur.value);
        } else {
            self.line_mut().backspace();
        }
    }

    pub fn delete(&mut self) {
        self.follow = true;
        let at_end = self.line().cursor >= self.line().len_chars();
        if at_end && self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.line_mut().insert_at_cursor(&next.value);
        } else {
            self.line_mut().delete_at_cursor();
        }
    }

    pub fn left(&mut self) {
        self.follow = true;
        self.line_mut().left();
    }

    pub fn right(&mut self) {
        self.follow = true;
        self.line_mut().right();
    }

    pub fn up(&mut self) {
        self.follow = true;
        let (rows, ri, vcol) = self.visual_pos();
        if ri == 0 {
            return;
        }
        self.move_to_visual_row(&rows, ri - 1, vcol);
    }

    pub fn down(&mut self) {
        self.follow = true;
        let (rows, ri, vcol) = self.visual_pos();
        if ri + 1 >= rows.len() {
            return;
        }
        self.move_to_visual_row(&rows, ri + 1, vcol);
    }

    fn visual_pos(&self) -> VisualPos {
        if self.wrap {
            let tw = self.text_w.max(1);
            let rows = self.wrap_rows(tw);
            let (ri, vcol) = self.cursor_visual(tw);
            (rows, ri, vcol as usize)
        } else {
            let rows = self.nowrap_rows();
            let (ri, vcol) = self.cursor_visual_nowrap();
            (rows, ri, vcol)
        }
    }

    fn move_to_visual_row(&mut self, rows: &[(usize, usize, usize)], target: usize, vcol: usize) {
        let Some((li, s, n)) = rows.get(target).copied() else {
            return;
        };
        let chars: Vec<char> = self.lines[li].value.chars().collect();
        let mut acc = 0usize;
        let mut off = 0usize;
        for k in 0..n.min(chars.len().saturating_sub(s)) {
            let cw = w(&chars[s + k].to_string());
            if vcol < acc + cw.div_ceil(2) {
                off = k;
                break;
            }
            acc += cw;
            off = k + 1;
        }
        self.row = li;
        self.lines[li].set_cursor(s + off);
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn to_first(&mut self) {
        self.follow = true;
        self.row = 0;
        self.line_mut().cursor_home();
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn to_last(&mut self) {
        self.follow = true;
        self.row = self.lines.len() - 1;
        self.line_mut().cursor_end();
    }

    pub fn insert_char(&mut self, c: char) {
        self.follow = true;
        self.line_mut().insert_at_cursor(&c.to_string());
    }

    pub fn insert_str(&mut self, text: &str) {
        self.follow = true;
        let mut first = true;
        for raw in text.split('\n') {
            let seg = raw.strip_suffix('\r').unwrap_or(raw);
            if !first {
                if self.lines.len() >= MULTI_MAX_LINES {
                    break;
                }
                let rest = self.line_mut().split_at_cursor();
                self.lines.insert(self.row + 1, rest);
                self.row += 1;
            }
            let clean: String = seg.chars().filter(|c| !c.is_control()).collect();
            self.line_mut().insert_at_cursor(&clean);
            first = false;
        }
    }

    #[must_use]
    pub fn wrap_rows(&self, text_w: usize) -> Vec<(usize, usize, usize)> {
        let mut rows = Vec::new();
        for (li, l) in self.lines.iter().enumerate() {
            for (s, n) in wrap_segments(&l.value, text_w.max(1)) {
                rows.push((li, s, n));
            }
        }
        if rows.is_empty() {
            rows.push((0, 0, 0));
        }
        rows
    }

    #[must_use]
    pub fn cursor_visual(&self, text_w: usize) -> (usize, u16) {
        let rows = self.wrap_rows(text_w);
        let total = rows.len();
        let cur_line = self.row.min(self.lines.len() - 1);
        let cur_col = self.lines[cur_line].cursor;
        let mut first_of_line = None;
        let mut found = None;
        for (ri, (li, s, n)) in rows.iter().enumerate() {
            if *li == cur_line {
                if first_of_line.is_none() {
                    first_of_line = Some(ri);
                }
                let (s, n) = (*s, *n);
                if cur_col >= s && cur_col <= s + n {
                    found = Some(ri);
                }
            }
        }
        let ri = found.or(first_of_line).unwrap_or(total.saturating_sub(1));
        let vcol = match rows.get(ri) {
            Some((_, s, n)) => {
                let (s, n) = (*s, *n);
                let upto = cur_col.min(s + n);
                let pre: String = self.lines[cur_line]
                    .value
                    .chars()
                    .skip(s)
                    .take(upto - s)
                    .collect();
                (w(&pre) as u16).min(text_w.max(1) as u16 - 1)
            }
            None => 0,
        };
        (ri, vcol)
    }

    pub fn scroll_follow(&mut self, total_rows: usize, visible: usize, cursor_row: usize) {
        let vis = visible.max(1);
        if cursor_row < self.view {
            self.view = cursor_row;
        }
        if cursor_row >= self.view + vis {
            self.view = cursor_row + 1 - vis;
        }
        let max = total_rows.saturating_sub(vis);
        self.view = self.view.min(max);
    }

    pub fn clamp_view(&mut self, total_rows: usize, visible: usize) {
        let vis = visible.max(1);
        self.view = self.view.min(total_rows.saturating_sub(vis));
    }

    pub fn scroll_by(&mut self, delta: i32) {
        self.follow = false;
        let max = self.max_scroll as i64;
        self.view = (self.view as i64 + delta as i64).clamp(0, max.max(0)) as usize;
    }

    pub fn scroll_to(&mut self, pos: usize) {
        self.follow = false;
        self.view = pos.min(self.max_scroll);
    }

    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_by(-(n as i32));
    }

    pub fn scroll_down(&mut self, n: usize) {
        self.scroll_by(n as i32);
    }

    #[must_use]
    pub fn nowrap_rows(&self) -> Vec<(usize, usize, usize)> {
        self.lines
            .iter()
            .enumerate()
            .map(|(li, l)| (li, 0, l.len_chars()))
            .collect()
    }

    #[must_use]
    pub fn nowrap_content_w(&self) -> usize {
        self.lines.iter().map(|l| w(&l.value)).max().unwrap_or(0)
    }

    #[must_use]
    pub fn cursor_visual_nowrap(&self) -> (usize, usize) {
        let cur = self.row.min(self.lines.len() - 1);
        let pre: String = self.lines[cur]
            .value
            .chars()
            .take(self.lines[cur].cursor)
            .collect();
        (cur, w(&pre))
    }

    pub fn scroll_follow_nowrap(
        &mut self,
        total_rows: usize,
        visible: usize,
        cursor_row: usize,
        text_w: usize,
        cursor_col: usize,
    ) {
        let vis = visible.max(1);
        if cursor_row < self.view {
            self.view = cursor_row;
        }
        if cursor_row >= self.view + vis {
            self.view = cursor_row + 1 - vis;
        }
        let max = total_rows.saturating_sub(vis);
        self.view = self.view.min(max);
        let tw = text_w.max(1);
        if cursor_col < self.view_x {
            self.view_x = cursor_col;
        }
        if cursor_col >= self.view_x + tw {
            self.view_x = cursor_col + 1 - tw;
        }
        self.view_x = self.view_x.min(self.max_scroll_x);
    }

    pub fn scroll_by_x(&mut self, delta: i32) {
        self.follow = false;
        let max = self.max_scroll_x as i64;
        self.view_x = (self.view_x as i64 + delta as i64).clamp(0, max.max(0)) as usize;
    }

    pub fn scroll_x_by(&mut self, n: usize) {
        self.scroll_by_x(n as i32);
    }

    pub fn scroll_x_to(&mut self, pos: usize) {
        self.follow = false;
        self.view_x = pos.min(self.max_scroll_x);
    }

    pub fn click_at_nowrap(&mut self, rel_row: usize, col: u16, x: u16) {
        let ri = self.view + rel_row;
        if ri >= self.lines.len() {
            return;
        }
        self.row = ri;
        let chars: Vec<char> = self.lines[ri].value.chars().collect();
        let base = self.view_x as i64 + col as i64 - x as i64;
        let mut acc = 0i64;
        let mut off = 0usize;
        for (k, c) in chars.iter().enumerate() {
            let cw = w(&c.to_string()) as i64;
            if base < acc + (cw + 1) / 2 {
                off = k;
                break;
            }
            acc += cw;
            off = k + 1;
        }
        self.lines[ri].set_cursor(off);
        self.follow = true;
    }

    pub fn click_at(&mut self, text_w: usize, rel_row: usize, col: u16, x: u16) {
        let rows = self.wrap_rows(text_w);
        let ri = self.view + rel_row;
        let Some((li, s, n)) = rows.get(ri.min(rows.len().saturating_sub(1))).copied() else {
            return;
        };
        self.row = li;
        let val = self.lines[li].value.clone();
        let chars: Vec<char> = val.chars().collect();
        let mut acc = 0u16;
        let mut off = 0usize;
        for k in 0..n.min(chars.len().saturating_sub(s)) {
            let cw = w(&chars[s + k].to_string()) as u16;
            if col < x.saturating_add(acc + cw.div_ceil(2)) {
                off = k;
                break;
            }
            acc += cw;
            off = k + 1;
        }
        self.lines[li].set_cursor(s + off);
        self.follow = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf_with(lines: &[&str], wrap: bool, text_w: usize) -> MultiLineBuf {
        let mut b = MultiLineBuf::new();
        b.wrap = wrap;
        b.text_w = text_w;
        for (i, l) in lines.iter().enumerate() {
            if i > 0 {
                b.newline();
            }
            b.insert_str(l);
        }
        b
    }

    #[test]
    fn nowrap_up_clamps_to_end_of_short_line() {
        let mut b = buf_with(&["short", "a-very-long-tracker-url"], false, 40);
        b.row = 1;
        b.lines[1].set_cursor(10);
        b.up();
        assert_eq!(b.row, 0);
        assert_eq!(b.lines[0].cursor, 5);
    }

    #[test]
    fn nowrap_down_preserves_column_exactly() {
        let mut b = buf_with(&["abcdef", "xyz"], false, 40);
        b.row = 0;
        b.lines[0].set_cursor(2);
        b.down();
        assert_eq!(b.row, 1);
        assert_eq!(b.lines[1].cursor, 2);
        b.up();
        assert_eq!(b.lines[0].cursor, 2);
    }

    #[test]
    fn nowrap_up_empty_target_line_lands_at_home() {
        let mut b = buf_with(&["abcdef", "", "xyz"], false, 40);
        b.row = 2;
        b.lines[2].set_cursor(1);
        b.up();
        assert_eq!(b.row, 1);
        assert_eq!(b.lines[1].cursor, 0);
    }

    #[test]
    fn nowrap_up_down_wide_char_column() {
        let mut b = buf_with(&["中文", "abc"], false, 40);
        b.lines[1].set_cursor(2);
        b.up();
        assert_eq!(b.lines[0].cursor, 1);
    }

    #[test]
    fn nowrap_at_edges_is_noop() {
        let mut b = buf_with(&["one", "two"], false, 40);
        b.row = 0;
        b.lines[0].set_cursor(1);
        b.up();
        assert_eq!(b.row, 0);
        assert_eq!(b.lines[0].cursor, 1);
        b.row = 1;
        b.down();
        assert_eq!(b.row, 1);
    }

    #[test]
    fn wrap_up_moves_across_wrapped_segment() {
        let mut b = buf_with(&["aaa bbb ccc ddd", "zz"], true, 10);
        b.row = 0;
        b.lines[0].set_cursor(10);
        b.up();
        assert_eq!(b.row, 0);
        assert_eq!(b.lines[0].cursor, 2);
    }

    #[test]
    fn wrap_down_returns_across_wrapped_segment() {
        let mut b = buf_with(&["aaa bbb ccc ddd", "zz"], true, 10);
        b.row = 0;
        b.lines[0].set_cursor(2);
        b.down();
        assert_eq!(b.lines[0].cursor, 10);
    }

    #[test]
    fn wrap_up_from_line_end_lands_at_segment_end() {
        let mut b = buf_with(&["aaa bbb ccc ddd"], true, 10);
        b.lines[0].cursor_end();
        b.up();
        assert_eq!(b.lines[0].cursor, 7);
    }

    #[test]
    fn wrap_down_from_first_segment_to_second_line() {
        let mut b = buf_with(&["aaa bbb ccc ddd", "xyz"], true, 10);
        b.row = 0;
        b.lines[0].set_cursor(3);
        b.down();
        assert_eq!(b.lines[0].cursor, 11);
        b.down();
        assert_eq!(b.row, 1);
        assert_eq!(b.lines[1].cursor, 3);
    }

    #[test]
    fn wrap_single_row_edges_are_noop() {
        let mut b = buf_with(&["only one line"], true, 40);
        b.lines[0].set_cursor(3);
        b.up();
        assert_eq!(b.lines[0].cursor, 3);
        b.down();
        assert_eq!(b.lines[0].cursor, 3);
    }

    #[test]
    fn newline_caps_at_multi_max_lines() {
        let mut b = MultiLineBuf::new();
        for _ in 1..MULTI_MAX_LINES {
            assert!(b.newline());
        }
        assert_eq!(b.lines.len(), MULTI_MAX_LINES);
        assert!(!b.newline(), "达上限后换行被拒绝");
    }

    #[test]
    fn backspace_at_line_start_merges_with_previous() {
        let mut b = buf_with(&["abc", "def"], true, 40);
        b.row = 1;
        b.line_mut().set_cursor(0);
        b.backspace();
        assert_eq!(b.lines.len(), 1);
        assert_eq!(b.lines[0].value, "abcdef");
        assert_eq!(b.lines[0].cursor, 6);
    }

    #[test]
    fn delete_at_line_end_swallows_next_line() {
        let mut b = buf_with(&["abc", "def"], true, 40);
        b.row = 0;
        b.lines[0].cursor_end();
        b.delete();
        assert_eq!(b.lines.len(), 1);
        assert_eq!(b.lines[0].value, "abcdef");
    }

    #[test]
    fn trim_lines_filters_empty_and_trims() {
        let b = buf_with(&["  http://a  ", "", "x"], true, 40);
        assert_eq!(b.trim_lines(), vec!["http://a", "x"]);
        assert!(!b.is_buffer_empty());
        assert!(MultiLineBuf::new().is_buffer_empty(), "单空行 = 空缓冲");
    }

    #[test]
    fn set_text_prefills_and_keeps_view_at_top() {
        let mut b = MultiLineBuf::new();
        b.set_text("a\nb\nc\nd\ne\nf");
        assert_eq!(b.lines.len(), 6);
        assert_eq!(b.row, 5, "光标停在末行");
        assert_eq!(b.view, 0, "视口展示首行（不跟随到底）");
        assert!(!b.follow, "预填不触发跟随回贴");
        b.reset();
        assert!(b.is_buffer_empty() && b.row == 0 && b.view == 0);
    }

    #[test]
    fn scroll_primitives_clamp_and_unlock_follow() {
        let mut b = buf_with(&["a", "b", "c", "d"], true, 40);
        b.max_scroll = 2;
        b.follow = true;
        b.scroll_down(5);
        assert_eq!(b.view, 2, "下滚钳制到 max_scroll");
        assert!(!b.follow, "滚动解除视口跟随");
        b.scroll_up(5);
        assert_eq!(b.view, 0, "上滚钳制到 0");
        b.scroll_to(1);
        assert_eq!(b.view, 1);
        assert!(!b.follow, "绝对定位同样解除跟随");
        let mut t = buf_with(&["long-tracker-url"], false, 8);
        t.max_scroll_x = 7;
        t.scroll_x_by(3);
        assert_eq!(t.view_x, 3);
        t.scroll_x_by(99);
        assert_eq!(t.view_x, 7, "水平滚动钳制 max_scroll_x");
        t.scroll_x_to(2);
        assert_eq!(t.view_x, 2);
    }
}
