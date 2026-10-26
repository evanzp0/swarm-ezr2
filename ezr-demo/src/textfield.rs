const WIDE_RANGES: [(u32, u32); 6] = [
    (0x2E80, 0xA4CF),
    (0xAC00, 0xD7A3),
    (0xF900, 0xFAFF),
    (0xFF00, 0xFF60),
    (0x3000, 0x303F),
    (0xFFE0, 0xFFE6),
];

fn wide(c: u32) -> bool {
    WIDE_RANGES.iter().any(|&(lo, hi)| lo <= c && c <= hi)
}

fn zero_width(c: u32) -> bool {
    (0xFE00..=0xFE0F).contains(&c) || (0x200B..=0x200F).contains(&c) || c == 0x2060 || c == 0xFEFF
}

pub fn w(s: &str) -> usize {
    s.chars()
        .map(|c| {
            let u = c as u32;
            if zero_width(u) {
                0
            } else if wide(u) {
                2
            } else {
                1
            }
        })
        .sum()
}

pub fn truncate(s: &str, max: usize) -> String {
    if w(s) <= max {
        return s.to_string();
    }
    let budget = max.saturating_sub(1);
    let mut out = String::new();
    let mut acc = 0;
    for ch in s.chars() {
        let cw = w(&ch.to_string());
        if acc + cw > budget {
            break;
        }
        acc += cw;
        out.push(ch);
    }
    out.push('…');
    out
}

fn char_byte_offset(s: &str, n: usize) -> usize {
    s.chars().take(n).map(char::len_utf8).sum()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldWin {
    pub value_x: u16,

    pub left_cut: bool,

    pub cols: Vec<u16>,

    pub win_lo: usize,

    pub win_hi: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldDisplay {
    pub pre: String,

    pub post: String,

    pub win: FieldWin,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextInput {
    pub value: String,

    pub cursor: usize,
}

impl TextInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_value(value: impl Into<String>) -> Self {
        let mut t = Self {
            value: value.into(),
            cursor: 0,
        };
        t.cursor_end();
        t
    }

    pub fn len_chars(&self) -> usize {
        self.value.chars().count()
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    pub fn set(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor_end();
    }

    pub fn cursor_end(&mut self) {
        self.cursor = self.len_chars();
    }

    pub fn cursor_home(&mut self) {
        self.cursor = 0;
    }

    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.len_chars());
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        if self.cursor < self.len_chars() {
            self.cursor += 1;
        }
    }

    pub fn insert(&mut self, text: &str, cap: usize) -> usize {
        let room = cap.saturating_sub(self.len_chars());
        let taken: Vec<char> = text.chars().take(room).collect();
        let n = taken.len();
        if n > 0 {
            let mut off = char_byte_offset(&self.value, self.cursor);
            for c in taken {
                self.value.insert(off, c);
                off += c.len_utf8();
            }
            self.cursor += n;
        }
        n
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let off = char_byte_offset(&self.value, self.cursor - 1);
            self.value.remove(off);
            self.cursor -= 1;
        }
    }

    pub fn field_window(&self, ph: &str, budget: usize, focused: bool) -> FieldDisplay {
        let budget = budget.max(1);
        let mut win = FieldWin {
            value_x: 0,
            left_cut: false,
            cols: Vec::new(),
            win_lo: 0,
            win_hi: 0,
        };

        fn fill_cols(win: &mut FieldWin, widths: &[usize]) {
            win.cols.push(0);
            let mut acc = 0u16;
            for &cw in widths {
                acc += cw as u16;
                win.cols.push(acc);
            }
        }
        if self.value.is_empty() {
            win.cols.push(0);
            return FieldDisplay {
                pre: String::new(),
                post: truncate(ph, budget),
                win,
            };
        }
        let chars: Vec<char> = self.value.chars().collect();
        let widths: Vec<usize> = chars.iter().map(|c| w(&c.to_string())).collect();
        let total: usize = widths.iter().sum();
        let anchor = if focused {
            self.cursor.min(chars.len())
        } else {
            chars.len()
        };
        if total <= budget {
            win.win_lo = 0;
            win.win_hi = chars.len();
            fill_cols(&mut win, &widths);
            return if focused {
                FieldDisplay {
                    pre: chars[..anchor].iter().collect(),
                    post: chars[anchor..].iter().collect(),
                    win,
                }
            } else {
                FieldDisplay {
                    pre: String::new(),
                    post: self.value.clone(),
                    win,
                }
            };
        }

        let mut reserve = 0usize;
        loop {
            let b = budget.saturating_sub(reserve).max(1);

            let mut acc = 0usize;
            let mut hi = anchor;
            while hi < chars.len() && acc + widths[hi] <= b {
                acc += widths[hi];
                hi += 1;
            }
            let mut lo = anchor;
            while lo > 0 && acc + widths[lo - 1] <= b {
                lo -= 1;
                acc += widths[lo];
            }
            let left_cut = lo > 0;
            let right_cut = hi < chars.len();
            let need = usize::from(left_cut) + usize::from(right_cut);
            if need == reserve || budget < 3 {
                win.win_lo = lo;
                win.win_hi = hi;
                win.left_cut = left_cut;
                fill_cols(&mut win, &widths[lo..hi]);
                let pre = if focused {
                    if left_cut {
                        format!("…{}", chars[lo..anchor].iter().collect::<String>())
                    } else {
                        chars[lo..anchor].iter().collect()
                    }
                } else if left_cut {
                    "…".to_string()
                } else {
                    String::new()
                };
                let post = if focused {
                    if right_cut {
                        format!("{}…", chars[anchor..hi].iter().collect::<String>())
                    } else {
                        chars[anchor..hi].iter().collect()
                    }
                } else if right_cut {
                    format!("{}…", chars[lo..hi].iter().collect::<String>())
                } else {
                    chars[lo..hi].iter().collect()
                };
                return FieldDisplay { pre, post, win };
            }
            reserve = need;
        }
    }

    pub fn click_offset(win: &FieldWin, col: u16) -> usize {
        let rel = col
            .saturating_sub(win.value_x)
            .saturating_sub(u16::from(win.left_cut));
        let mut k = 0usize;
        while k + 1 < win.cols.len() {
            let (lo_c, hi_c) = (win.cols[k], win.cols[k + 1]);
            if rel >= lo_c && rel < hi_c {
                return win.win_lo + k + usize::from(rel > lo_c);
            }
            k += 1;
        }
        win.win_hi
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn w_counts_cjk_double_and_zero_width_as_zero() {
        assert_eq!(w("ab"), 2);
        assert_eq!(w("中文"), 4);
        assert_eq!(w("a\u{FE0F}"), 1, "VS15 零宽");
        assert_eq!(w("\u{200B}"), 0, "零宽空格");
        assert_eq!(w(""), 0);
    }

    #[test]
    fn truncate_respects_display_width_budget() {
        assert_eq!(truncate("abc", 10), "abc");
        assert_eq!(truncate("中文中文", 5), "中文…");
        assert_eq!(truncate("abcdef", 0), "…");
    }

    #[test]
    fn insert_at_cursor_and_capacity() {
        let mut t = TextInput::with_value("abc");
        assert_eq!(t.cursor, 3);
        t.left();
        assert_eq!(t.insert("X", 10), 1);
        assert_eq!(t.value, "abXc");
        assert_eq!(t.cursor, 3, "光标推进插入量");

        assert_eq!(t.insert("123456", 5), 1);
        assert_eq!(t.value, "abX1c");
        assert_eq!(t.cursor, 4);

        let mut t2 = TextInput::new();
        assert_eq!(t2.insert("中文", 10), 2);
        assert_eq!(t2.value, "中文");
        assert_eq!(t2.cursor, 2);
        assert_eq!(t2.insert("字", 2), 0, "容量已满");
    }

    #[test]
    fn backspace_clamps_and_moves_cursor() {
        let mut t = TextInput::with_value("abc");
        t.backspace();
        assert_eq!(t.value, "ab");
        assert_eq!(t.cursor, 2);
        t.cursor_home();
        t.backspace();
        assert_eq!(t.value, "ab", "行首退格无操作");
        assert_eq!(t.cursor, 0);
        let mut t2 = TextInput::with_value("中文");
        t2.backspace();
        assert_eq!(t2.value, "中", "多字节整字符删除");
    }

    #[test]
    fn cursor_movement_clamps_both_ends() {
        let mut t = TextInput::with_value("abc");
        t.right();
        assert_eq!(t.cursor, 3, "右移到尾钳制");
        t.left();
        t.left();
        assert_eq!(t.cursor, 1);
        t.left();
        t.left();
        assert_eq!(t.cursor, 0, "左移到头钳制");
        t.cursor_end();
        assert_eq!(t.cursor, 3);
        t.set_cursor(99);
        assert_eq!(t.cursor, 3, "set_cursor 越界钳尾");
        t.set_cursor(1);
        assert_eq!(t.cursor, 1);
    }

    #[test]
    fn set_and_clear_reset_cursor() {
        let mut t = TextInput::with_value("abc");
        t.clear();
        assert!(t.is_empty());
        assert_eq!(t.cursor, 0);
        t.set("中文");
        assert_eq!(t.cursor, 2, "set 光标迁尾");
    }

    #[test]
    fn field_window_windows_and_splits() {
        let mut t = TextInput::with_value("abcdef");
        t.set_cursor(2);
        let fd = t.field_window("", 10, true);
        assert_eq!(fd.pre, "ab");
        assert_eq!(fd.post, "cdef");
        assert_eq!(fd.win.win_lo, 0);
        assert_eq!(fd.win.win_hi, 6);
        assert_eq!(fd.win.cols, vec![0, 1, 2, 3, 4, 5, 6]);
        assert!(!fd.win.left_cut);

        let fd = t.field_window("", 10, false);
        assert_eq!(fd.pre, "");
        assert_eq!(fd.post, "abcdef");

        let val = "abcdefghijklmnopqrstuvwxyz";
        let mut t = TextInput::with_value(val);
        t.set_cursor(8);
        let fd = t.field_window("", 6, true);

        assert_eq!(fd.win.win_hi, 12, "右窗 = 预算内向右取满");
        assert_eq!(w(&fd.pre) + w(&fd.post), 6, "pre/post 拼接恰满预算");
        assert!(fd.pre.starts_with('…'));
        assert_eq!(fd.win.value_x, 0);

        let total_cols = *fd.win.cols.last().unwrap() as usize;
        assert_eq!(total_cols, 4, "窗口内容 = 4 窄字符（b=4 不动点）");

        let mut t = TextInput::with_value(val);
        t.set_cursor(16);
        let fd = t.field_window("", 6, true);
        assert_eq!(fd.win.win_lo, 16, "锚 = 光标：左侧只到锚");
        assert!(fd.post.ends_with('…'));

        let mut t = TextInput::with_value(val);
        t.set_cursor(0);
        let fd = t.field_window("", 6, true);
        assert_eq!(fd.win.win_lo, 0);
        assert!(!fd.win.left_cut);
    }

    #[test]
    fn field_window_cjk_atomic_window() {
        let mut t = TextInput::with_value("中文中文");
        t.set_cursor(4);
        let fd = t.field_window("", 3, true);
        assert_eq!(fd.win.win_lo, 3);
        assert_eq!(fd.win.win_hi, 4);
        assert_eq!(fd.pre, "…文");
        assert_eq!(fd.post, "");

        let t = TextInput::with_value("中文");
        let fd = t.field_window("", 10, true);
        assert_eq!(fd.win.cols, vec![0, 2, 4]);
    }

    #[test]
    fn field_window_unfocused_takes_tail_window() {
        let val = "abcdefghijklmnopqrstuvwxyz";
        let t = TextInput::with_value(val);
        let fd = t.field_window("", 6, false);
        assert_eq!(fd.pre, "…");
        assert_eq!(fd.post, "vwxyz");
        assert_eq!(fd.win.win_lo, 21);
        assert_eq!(fd.win.win_hi, 26);

        let t = TextInput::with_value("abcdef1234567890");
        let fd = t.field_window("", 6, false);
        assert_eq!(fd.pre, "…");
        assert_eq!(fd.post, "67890");
        assert_eq!((fd.win.win_lo, fd.win.win_hi), (11, 16));
        assert!(fd.win.left_cut);

        let t = TextInput::with_value("一二三四五");
        let fd = t.field_window("", 6, false);
        assert_eq!(fd.pre, "…");
        assert_eq!(fd.post, "四五", "CJK 原子尾窗（右裁不发生，无 …）");
        assert_eq!((fd.win.win_lo, fd.win.win_hi), (3, 5));

        let t = TextInput::new();
        let fd = t.field_window("占位提示", 20, false);
        assert!(fd.pre.is_empty());
        assert_eq!(fd.post, truncate("占位提示", 20));
    }

    #[test]
    fn field_window_clamps_stale_cursor_and_tail_anchor() {
        let t = TextInput::with_value("abc");
        let mut t2 = t.clone();
        t2.cursor = 99;
        let fd = t2.field_window("", 10, true);
        assert_eq!(fd.pre, "abc");
        assert!(fd.post.is_empty());
        assert_eq!(fd.win.win_hi, 3);

        let mut t = TextInput::with_value("abcdefghijklmnop");
        t.set_cursor(8);
        let fd = t.field_window("", 6, true);
        assert!(
            fd.pre.starts_with('…') && fd.post.ends_with('…'),
            "双缘裁剪指示"
        );
        assert_eq!(w(&fd.pre) + w(&fd.post), 6, "总宽 = 预算");
        assert!(fd.win.left_cut, "左缘裁剪指示发布");
        assert_eq!(
            fd.win.cols.len(),
            fd.win.win_hi - fd.win.win_lo + 1,
            "cols = 窗内字符数 + 1"
        );
        assert!(
            format!("{}{}", fd.pre, fd.post).contains('i'),
            "窗内含光标字符"
        );

        let t = TextInput::with_value("abcdefghijklmnop");
        let fd = t.field_window("", 6, true);
        assert!(!fd.post.ends_with('…'), "光标在尾右缘不裁");
        assert!(fd.pre.starts_with('…'));
        assert_eq!(fd.win.win_hi, 16);
    }

    #[test]
    fn field_window_empty_shows_placeholder() {
        let t = TextInput::new();
        let fd = t.field_window("/srv/downloads", 6, true);
        assert_eq!(fd.pre, "");
        assert_eq!(fd.post, "/srv/…");
        assert_eq!(fd.win.cols, vec![0], "空字段窗口空：点击任意处光标 0");
    }

    fn win(value_x: u16, left_cut: bool, cols: Vec<u16>, win_lo: usize, win_hi: usize) -> FieldWin {
        FieldWin {
            value_x,
            left_cut,
            cols,
            win_lo,
            win_hi,
        }
    }

    #[test]
    fn click_offset_maps_columns_to_char_indices() {
        let narrow = win(35, false, vec![0, 1, 2, 3], 0, 3);
        assert_eq!(TextInput::click_offset(&narrow, 35), 0, "首字符列 → 前");
        assert_eq!(TextInput::click_offset(&narrow, 36), 1, "第 2 字符列 → 前");
        assert_eq!(TextInput::click_offset(&narrow, 37), 2, "第 3 字符列 → 前");
        assert_eq!(TextInput::click_offset(&narrow, 38), 3, "越末 → win_hi");
        assert_eq!(
            TextInput::click_offset(&narrow, 60),
            3,
            "右侧 padding → win_hi"
        );
        assert_eq!(TextInput::click_offset(&narrow, 34), 0, "label 区 → 窗口首");

        let wide = win(35, false, vec![0, 2, 4], 0, 2);
        assert_eq!(TextInput::click_offset(&wide, 35), 0, "宽字左半格 → 其前");
        assert_eq!(TextInput::click_offset(&wide, 36), 1, "宽字右半格 → 其后");
        assert_eq!(
            TextInput::click_offset(&wide, 37),
            1,
            "第 2 宽字左半格 → 其前"
        );
        assert_eq!(
            TextInput::click_offset(&wide, 38),
            2,
            "第 2 宽字右半格 → win_hi"
        );

        let cut = win(35, true, vec![0, 1, 2], 5, 7);
        assert_eq!(TextInput::click_offset(&cut, 35), 5, "指示符列 → 窗口首");
        assert_eq!(TextInput::click_offset(&cut, 36), 5, "首字符左半 → win_lo");
        assert_eq!(
            TextInput::click_offset(&cut, 37),
            6,
            "首字符右半 → 下一偏移"
        );

        let empty = win(35, false, vec![0], 0, 0);
        assert_eq!(TextInput::click_offset(&empty, 40), 0);
    }

    #[test]
    fn field_window_geometry_feeds_click_offset_roundtrip() {
        let mut t = TextInput::with_value("abc中文def");
        t.set_cursor(5);
        let mut fd = t.field_window("", 10, true);
        fd.win.value_x = 40;

        assert_eq!(TextInput::click_offset(&fd.win, 43), 3);

        assert_eq!(TextInput::click_offset(&fd.win, 44), 4);
    }
}
