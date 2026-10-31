//! 可复用多行文本缓冲（MultiLineBuf）。
//!
//! 「命令窗口」文本框与「添加/修改 BT 任务」trackers 输入共用同一编辑模型：
//! 逻辑行 × [`TextInput`] + 光标行 + 视口起始 + 按需视口跟随开关。渲染层见
//! `ui::multiline::draw_multi_line_input`（多行文本输入 panel 组件，支持
//! title 与空缓冲占位提示）。
//!
//! 两种视口模式（由渲染层按调用参数选择，编辑模型本身双模式可用）：
//! - word wrap（命令窗口）：`wrap_rows` / `cursor_visual` / `scroll_follow`，
//!   仅垂直滚动；
//! - 无 wrap（trackers 输入）：`nowrap_rows` / `cursor_visual_nowrap` /
//!   `scroll_follow_nowrap`，逻辑行 1:1 渲染，超宽内容水平滚动
//!   （`view_x` 显示列视口 + `max_scroll_x`，滚动条/滚轮走 `scroll_by_x` /
//!   `scroll_x_to`，同样解除按需跟随）。
//!
//! 视口跟随语义（与任务列表「跟随与防锁死」契约同构）：
//! - 编辑/光标移动类操作把 `follow` 置 true（渲染层消费一次后复位）；
//! - 滚动类操作（`scroll_by` / `scroll_to`）把 `follow` 置 false——否则每帧
//!   强制回贴光标会让滚动条"失效"（尤其光标停在最后一行时无法上滚）。

use crate::textfield::{w, wrap_segments, TextInput};

/// 行数上限（demo 口径：防止无限增长）
pub const MULTI_MAX_LINES: usize = 64;

#[derive(Debug)]
pub struct MultiLineBuf {
    /// 逻辑行（每行一个单行 TextInput，复用其光标/插入/删除原语）
    pub lines: Vec<TextInput>,

    /// 光标所在逻辑行（0 基）
    pub row: usize,

    /// 视口起始（可视段行，word wrap 后的行号）
    pub view: usize,

    /// 水平视口起始（显示宽列；仅无 wrap 模式使用，wrap 模式恒 0）
    pub view_x: usize,

    /// 视口跟随开关：编辑/光标移动置 true（渲染层消费一次后复位），
    /// 滚动条/滚轮滚动置 false
    pub follow: bool,

    /// 最大可视行滚动量（渲染层每帧发布；滚轮/拖拽用）
    pub max_scroll: usize,

    /// 最大水平列滚动量（渲染层每帧发布；无 wrap 模式水平滚动条/滚轮用）
    pub max_scroll_x: usize,

    /// 渲染层每帧发布的内容区宽度（显示宽列）——↑↓ 可视行导航（wrap 分段）
    /// 与当前列计算依赖；键处理前必有一帧渲染（面板先绘制才可聚焦），
    /// 初始 0 仅在无渲染时退化，wrap_rows 内部有 max(1) 兜底
    pub text_w: usize,

    /// 渲染层每帧发布的视口模式：true = word wrap（命令窗口），false =
    /// 无 wrap 逻辑行 1:1（trackers）。↑↓ 据此选择可视行口径
    pub wrap: bool,
}

impl Default for MultiLineBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiLineBuf {
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

    /// 重置为单空行（打开窗口/对话框时调用）
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

    /// 预填多行文本（\n 分隔；用于对话框演示数据）
    pub fn set_text(&mut self, text: &str) {
        self.reset();
        self.insert_str(text);
        // 展示首行（不跟随光标到底部）
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

    /// 是否为空缓冲（单空行）——渲染层用于显示占位提示
    pub fn is_buffer_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].value.is_empty()
    }

    /// 各逻辑行 trim 后的非空行（确认分流用）
    pub fn trim_lines(&self) -> Vec<String> {
        self.lines
            .iter()
            .map(|l| l.value.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    }

    /// 在当前光标处换行（光标后内容移到新行）；超过行数上限返回 false
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

    /// Backspace：光标前删除；行首则与上一行合并
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

    /// Delete：光标处删除；行尾则吞并下一行
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

    /// ↑：上一可视行——wrap 模式含 word wrap 分段行，无 wrap 模式即上一
    /// 逻辑行（1:1）。
    ///
    /// 列保持口径：目标行当前列有字符 → 光标移到该列字符位置处；当前列
    /// 超出目标行内容（无字符）→ 光标落在目标行末尾。
    pub fn up(&mut self) {
        self.follow = true;
        let (rows, ri, vcol) = self.visual_pos();
        if ri == 0 {
            return;
        }
        self.move_to_visual_row(&rows, ri - 1, vcol);
    }

    /// ↓：下一可视行（口径同 [`Self::up`]）
    pub fn down(&mut self) {
        self.follow = true;
        let (rows, ri, vcol) = self.visual_pos();
        if ri + 1 >= rows.len() {
            return;
        }
        self.move_to_visual_row(&rows, ri + 1, vcol);
    }

    /// 当前可视位置：(可视行布局, 光标可视行, 光标显示列)。
    ///
    /// - wrap 模式：按 text_w 显示宽分段，列为段内相对显示宽；
    /// - 无 wrap 模式：逻辑行 1:1，列为全文绝对显示宽（段起点恆 0，口径一致）。
    fn visual_pos(&self) -> (Vec<(usize, usize, usize)>, usize, usize) {
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

    /// 光标移到目标可视行：保持当前显示列 `vcol`；该列无字符（超出目标行
    /// 内容显示宽）时落在行末。
    ///
    /// 显示宽走查 + 宽字符中点归属（左半格 = 停其前、右半格 = 停其后），
    /// 与 `click_at` / `click_at_nowrap` 同约定。
    fn move_to_visual_row(&mut self, rows: &[(usize, usize, usize)], target: usize, vcol: usize) {
        let Some((li, s, n)) = rows.get(target).copied() else {
            return;
        };
        let chars: Vec<char> = self.lines[li].value.chars().collect();
        let mut acc = 0usize;
        let mut off = 0usize;
        for k in 0..n.min(chars.len().saturating_sub(s)) {
            let cw = w(&chars[s + k].to_string());
            if vcol < acc + (cw + 1) / 2 {
                off = k;
                break;
            }
            acc += cw;
            off = k + 1;
        }
        self.row = li;
        self.lines[li].set_cursor(s + off);
    }

    /// Home：全文首（首行行首）
    pub fn to_first(&mut self) {
        self.follow = true;
        self.row = 0;
        self.line_mut().cursor_home();
    }

    /// End：全文尾（末行行尾）
    pub fn to_last(&mut self) {
        self.follow = true;
        self.row = self.lines.len() - 1;
        self.line_mut().cursor_end();
    }

    /// 光标处插入单字符
    pub fn insert_char(&mut self, c: char) {
        self.follow = true;
        self.line_mut().insert_at_cursor(&c.to_string());
    }

    /// 光标处插入文本：\n 分段换行（段内控制字符剔除）——粘贴同径
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

    /// 全部逻辑行按 text_w 显示宽 wrap 后的可视段行：(逻辑行号, 字符起点, 字符长)。
    /// 渲染与鼠标点击命中共用同一布局，保证可视行号一致。
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

    /// 光标可视行 + 列（段内按显示宽累计）；渲染层据此发布原生光标位
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

    /// 视口跟随光标可视行 + 钳制（渲染前调用）
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

    /// 非跟随帧钳制视口（渲染前调用）
    pub fn clamp_view(&mut self, total_rows: usize, visible: usize) {
        let vis = visible.max(1);
        self.view = self.view.min(total_rows.saturating_sub(vis));
    }

    /// 滚轮滚动：解除视口跟随（滚动条拖拽/箭头/轨道走 `scroll_to`，同样解除跟随）
    pub fn scroll_by(&mut self, delta: i32) {
        self.follow = false;
        let max = self.max_scroll as i64;
        self.view = (self.view as i64 + delta as i64).clamp(0, max.max(0)) as usize;
    }

    /// 滚动条拖拽/轨道/箭头定位：解除视口跟随
    pub fn scroll_to(&mut self, pos: usize) {
        self.follow = false;
        self.view = pos.min(self.max_scroll);
    }

    // ---------- 无 wrap 模式（逻辑行 1:1 + 水平滚动；trackers 输入） ----------

    /// 无 wrap 模式可视行：逻辑行 1:1，每行 = (逻辑行号, 0, 全行字符数)。
    /// 渲染与鼠标点击命中共用同一布局；空缓冲返回单空段占位（保证占一行）。
    pub fn nowrap_rows(&self) -> Vec<(usize, usize, usize)> {
        self.lines
            .iter()
            .enumerate()
            .map(|(li, l)| (li, 0, l.len_chars()))
            .collect()
    }

    /// 无 wrap 模式内容显示宽 = 最宽逻辑行（水平滚动条 total 用）
    pub fn nowrap_content_w(&self) -> usize {
        self.lines.iter().map(|l| w(&l.value)).max().unwrap_or(0)
    }

    /// 光标可视行 + 显示宽列（无 wrap 模式；列可为全文绝对列，渲染层结合
    /// view_x 判定可见性并换算屏幕位）
    pub fn cursor_visual_nowrap(&self) -> (usize, usize) {
        let cur = self.row.min(self.lines.len() - 1);
        let pre: String = self.lines[cur]
            .value
            .chars()
            .take(self.lines[cur].cursor)
            .collect();
        (cur, w(&pre))
    }

    /// 视口跟随光标（无 wrap 模式，渲染前调用）：垂直回贴可视行 + 水平回贴
    /// 光标显示列（进入视口左/右缘内侧），并按 max_scroll_x 钳制
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

    /// 水平滚轮滚动：解除视口跟随（水平滚动条拖拽/轨道/箭头走 `scroll_x_to`）
    pub fn scroll_by_x(&mut self, delta: i32) {
        self.follow = false;
        let max = self.max_scroll_x as i64;
        self.view_x = (self.view_x as i64 + delta as i64).clamp(0, max.max(0)) as usize;
    }

    /// 水平滚动条拖拽/轨道/箭头定位：解除视口跟随
    pub fn scroll_x_to(&mut self, pos: usize) {
        self.follow = false;
        self.view_x = pos.min(self.max_scroll_x);
    }

    /// 点击文本区定位光标（无 wrap 模式）：rel_row = 视口内相对行，
    /// (col, x) = 点击列与内容区起点 x。行 = view + rel_row（逻辑行 1:1）；
    /// 列 = view_x + (col - x) 显示宽走查：点在窄字符格上 = 光标停其前，
    /// 宽字左半格在前、右半格在后（与单行字段 `TextInput::click_offset` 同约定）。
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

    /// 点击文本区定位光标：rel_row = 视口内相对行，(col, x) = 点击列与内容区
    /// 起点 x。可视行 → (逻辑行, 字符偏移)：与渲染共用同一 wrap 布局。
    ///
    /// 列 → 段内字符偏移按显示宽走查；点在窄字符格上 = 光标停其前，宽字
    /// 左半格在前、右半格在后（与单行字段 `TextInput::click_offset` 同约定）。
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
            if col < x.saturating_add(acc + (cw + 1) / 2) {
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

    /// 构造多行缓冲：逐行 newline + insert_str，光标停在末行行尾
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

    // ---------- 无 wrap 模式（trackers）：列保持 + 短行落行尾 ----------

    #[test]
    fn nowrap_up_clamps_to_end_of_short_line() {
        let mut b = buf_with(&["short", "a-very-long-tracker-url"], false, 40);
        b.row = 1;
        b.lines[1].set_cursor(10);
        b.up();
        assert_eq!(b.row, 0);
        // 上行列 10 无字符（"short" 宽 5）→ 落上一行末尾
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
        // 无状态口径：再 ↑ 回来列仍为 2（不是记忆的原始列 2 之外的位置）
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
        // 显示列 2 = 「文」左半格 → 光标停其前（中|文）
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

    // ---------- wrap 模式（命令窗口）：可视行含 word wrap 分段 ----------

    #[test]
    fn wrap_up_moves_across_wrapped_segment() {
        // tw=10："aaa bbb ccc ddd" → 可视段 "aaa bbb"(0..7) / "ccc ddd"(8..15)
        let mut b = buf_with(&["aaa bbb ccc ddd", "zz"], true, 10);
        // 光标在逻辑行 0 偏移 10（"ccc" 中间）→ 可视行 1、段内显示列 2
        b.row = 0;
        b.lines[0].set_cursor(10);
        b.up();
        // 上一可视行 = 同一逻辑行的 wrap 段 "aaa bbb" → 列 2
        assert_eq!(b.row, 0);
        assert_eq!(b.lines[0].cursor, 2);
    }

    #[test]
    fn wrap_down_returns_across_wrapped_segment() {
        let mut b = buf_with(&["aaa bbb ccc ddd", "zz"], true, 10);
        b.row = 0;
        b.lines[0].set_cursor(2);
        b.down();
        // 下一可视行 = wrap 段 "ccc ddd" 列 2 → 字符偏移 8 + 2
        assert_eq!(b.lines[0].cursor, 10);
    }

    #[test]
    fn wrap_up_from_line_end_lands_at_segment_end() {
        let mut b = buf_with(&["aaa bbb ccc ddd"], true, 10);
        b.lines[0].cursor_end(); // 偏移 15 → 可视行 1、显示列 7
        b.up();
        // 可视列 7 超出 "aaa bbb" 内容（该列无字符）→ 落该可视行末尾（偏移 7）
        assert_eq!(b.lines[0].cursor, 7);
    }

    #[test]
    fn wrap_down_from_first_segment_to_second_line() {
        // 跨可视行：光标在首段内 → ↓ 到第二段（同逻辑行 wrap 段）→ 再 ↓ 到下一逻辑行
        let mut b = buf_with(&["aaa bbb ccc ddd", "xyz"], true, 10);
        b.row = 0;
        b.lines[0].set_cursor(3); // 首段显示列 3
        b.down();
        assert_eq!(b.lines[0].cursor, 11); // 段 (8,7) 显示列 3 → 8+3（空格前）
        b.down();
        assert_eq!(b.row, 1); // 下一可视行 = 逻辑行 1
        assert_eq!(b.lines[1].cursor, 3); // 显示列 3 = "xyz" 行尾
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
}
