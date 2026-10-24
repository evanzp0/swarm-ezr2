//! textfield — 单行文本输入框组件（six-pack/coder 会话产物）。
//!
//! 把此前散落四处（`ui/text.rs` 宽度工具、`ui/dialog.rs` 取窗、
//! `app/mod.rs` 窗口几何、`app/mouse.rs` 点击映射、`app/dialog_keys/routing.rs`
//! 光标编辑）的输入框实现收口为**一个可复用组件**：值 + 光标状态、光标处
//! 编辑、显示取窗、终端原生光标定位、鼠标点击列→字符偏移映射全部单源。
//! 使用方（对话框字段行 / 任何未来输入框）只依赖本模块，杜绝多实现漂移。
//!
//! 组件零外部依赖（不引 ratatui/crossterm），纯数据 + 纯函数，全部可单元
//! 测试；渲染层（spans 组装 / `Frame::set_cursor_position`）仍由 ui 层消费
//! [`TextInput::field_window`] 的产物完成。
//!
//! 语义基线（沿袭 v1.34 二/三批 bug 修订，口径不变）：
//! - 光标 = 字符偏移 `0..=字符数`，方向键移动、输入/退格/粘贴均在光标处生效；
//! - 聚焦取窗锚 = 光标（pre/post 两段拼接 = 显示内容，光标屏幕列 =
//!   值区起点 + w(pre)，由调用方发布终端原生光标）；未聚焦锚 = 字段尾；
//! - 超宽两轮不动点取窗（裁剪指示符占列扣预算重取）；CJK 原子不溢出；
//! - 点击列按累积列半开区间定位（宽字符左右半格分置、越界钳制窗口首/尾）。

/// CJK 全角区间表（显示宽度按 2 计的码点闭区间，FR-01 定稿宽度口径）
const WIDE_RANGES: [(u32, u32); 6] = [
    (0x2E80, 0xA4CF), // CJK 部首扩展 … 谚文音节前段
    (0xAC00, 0xD7A3), // Hangul 音节
    (0xF900, 0xFAFF), // CJK 兼容表意文字
    (0xFF00, 0xFF60), // 全角形式
    (0x3000, 0x303F), // CJK 符号与标点
    (0xFFE0, 0xFFE6), // 全角补足
];

/// 码点显示宽度是否为 2（查表，短路任一区间命中）
fn wide(c: u32) -> bool {
    WIDE_RANGES.iter().any(|&(lo, hi)| lo <= c && c <= hi)
}

/// 码点是否零宽（URL 反链接注入 VS15 后宽度计算不得把它当占位字符，
/// 否则热区/wrap/truncate 全部偏移）——变体选择符 U+FE00..=FE0F（含本应用
/// 使用的 VS15）、零宽空格/连接符 U+200B..=200F、词连接符 U+2060、
/// BOM U+FEFF——终端渲染均为零宽
fn zero_width(c: u32) -> bool {
    (0xFE00..=0xFE0F).contains(&c) || (0x200B..=0x200F).contains(&c) || c == 0x2060 || c == 0xFEFF
}

/// 终端显示宽度（CJK 全角查表 ×2、零宽码点 ×0、其余 ×1）——全应用单源
///（`ui::text::w` 转发本实现，历史引用路径不变）
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

/// 宽度预算内截断（超出以 … 指示符收尾，指示符占 1 列预算）
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

/// 第 n 个字符的字节偏移（前 n 字符 UTF-8 长度和；n ≤ 字符数时结果恒为
/// char 边界——String::insert/remove 安全前提）
fn char_byte_offset(s: &str, n: usize) -> usize {
    s.chars().take(n).map(char::len_utf8).sum()
}

/// 对话框文本字段窗口几何（原生终端光标 + 鼠标点击定位光标的映射单源
/// 载荷。渲染层每帧随字段命中区域平行回填，鼠标层消费——点击列 → 字符
/// 偏移不经宽度重算，与渲染显示取窗同源，杜绝双实现漂移）
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldWin {
    /// 值区起始屏幕列（字段行 rect.x + label 7 列 + "> " 2 列）
    pub value_x: u16,
    /// 左缘裁剪指示（true = 值区首列是 … 指示符，点击映射扣 1 列）
    pub left_cut: bool,
    /// 可见字符的累积显示列半开边界（cols[0] = 0；cols[k+1] = cols[k] +
    /// 第 k 个可见字符显示宽；末位 = 窗口总宽）——点击列按 (cols[k],
    /// cols[k+1]) 半开区间定位：落宽字符左半格 = 光标在其前、右半格 =
    /// 在其后（终端鼠标以格为单位的惯例语义）；越界钳制窗口首/尾
    pub cols: Vec<u16>,
    /// 可见窗口首字符在字段文本中的字符下标
    pub win_lo: usize,
    /// 可见窗口尾字符下标（不含）
    pub win_hi: usize,
}

/// 取窗产物（[`TextInput::field_window`] 返回）：pre/post 在锚点切分，
/// 二段拼接 = 显示内容；win = 点击列→字符偏移映射与光标定位共用几何。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldDisplay {
    /// 锚点前段（聚焦 = 光标前，可能带左缘 … 指示符；未聚焦恒空）
    pub pre: String,
    /// 锚点后段（聚焦 = 光标后；未聚焦 = 整窗，可能带 … 指示符）
    pub post: String,
    /// 窗口几何（渲染层回填 value_x 后发布给鼠标层）
    pub win: FieldWin,
}

/// 单行文本输入框组件：可编辑文本值 + 光标（字符偏移）。
///
/// 状态纯净（无渲染属性——占位提示等由调用方传入 [`TextInput::field_window`]），
/// 编辑全部在光标处生效、方向键移动全程越界钳制。容量上限按字符数计
///（不切分多字节字符），超容量截断式取前缀。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextInput {
    /// 字段值
    pub value: String,
    /// 光标（字符偏移 0..=value 字符数；始终落在 char 边界）
    pub cursor: usize,
}

impl TextInput {
    /// 空输入框
    pub fn new() -> Self {
        Self::default()
    }

    /// 以给定初值构造，光标迁文本尾（输入框惯例：新获得焦点的字段从尾
    /// 继续输入）
    pub fn with_value(value: impl Into<String>) -> Self {
        let mut t = Self {
            value: value.into(),
            cursor: 0,
        };
        t.cursor_end();
        t
    }

    /// 字符数（光标 End 语义的长度读取单源）
    pub fn len_chars(&self) -> usize {
        self.value.chars().count()
    }

    /// 值是否为空
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// 清空值并归零光标
    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    /// 整体替换值，光标迁新文本尾
    pub fn set(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor_end();
    }

    /// 光标移到文本尾（End 语义：焦点切换迁尾 / Down 键同口径）
    pub fn cursor_end(&mut self) {
        self.cursor = self.len_chars();
    }

    /// 光标移到行首（Home 语义：Up 键同口径——单行字段无纵向行）
    pub fn cursor_home(&mut self) {
        self.cursor = 0;
    }

    /// 直接置光标到指定字符偏移（点击定位覆盖迁尾值的入口；越界钳制
    /// 0..=字符数）
    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.len_chars());
    }

    /// 光标左移一格（0 处钳制）
    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// 光标右移一格（字段尾钳制）
    pub fn right(&mut self) {
        if self.cursor < self.len_chars() {
            self.cursor += 1;
        }
    }

    /// 光标处插入文本（容量按字符数计，不切分多字节字符）；返回插入的
    /// 字符数（容量不足时截断式取前缀）。光标推进插入量。
    pub fn insert(&mut self, text: &str, cap: usize) -> usize {
        let room = cap.saturating_sub(self.len_chars());
        let taken: Vec<char> = text.chars().take(room).collect();
        let n = taken.len();
        if n > 0 {
            // 逐字符递进字节偏移插入（同偏移循环插入会倒序）
            let mut off = char_byte_offset(&self.value, self.cursor);
            for c in taken {
                self.value.insert(off, c);
                off += c.len_utf8();
            }
            self.cursor += n;
        }
        n
    }

    /// 光标处删除前一字符（cursor = 0 时无操作——行首退格钳制）
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let off = char_byte_offset(&self.value, self.cursor - 1);
            self.value.remove(off);
            self.cursor -= 1;
        }
    }

    /// 字段行取窗与点击/光标映射单源（原 ui/dialog.rs field_display 与
    /// cursor_field_display 两同构实现收口）。返回 [`FieldDisplay`]：
    /// - 聚焦：锚 = 光标（[`TextInput::cursor`]），pre/post 在锚处切分，
    ///   光标屏幕列 = 值区起点 + w(pre)，由调用方发布（终端原生光标，
    ///   移动零位移、可闪烁；内联「▏」字形占列导致后续文本随光标整体
    ///   平移，退役）；
    /// - 未聚焦：锚 = 字段尾，pre 恒空、post = 整窗。
    ///
    /// 预算内整值显示；超宽以锚点取窗（先向右取满、余量向左回退；裁剪侧 …
    /// 指示占列，预算扣减重取至不动点；budget < 3 放弃指示符）；空值显示
    /// 占位提示（截断，窗口空：cols = [0]，点击任意处光标 0）。
    ///
    /// FieldWin.cols/win_lo/win_hi = 点击列→字符偏移映射与光标定位共用几何。
    pub fn field_window(&self, ph: &str, budget: usize, focused: bool) -> FieldDisplay {
        let budget = budget.max(1);
        let mut win = FieldWin {
            value_x: 0,
            left_cut: false,
            cols: Vec::new(),
            win_lo: 0,
            win_hi: 0,
        };
        // cols 累积列回填（字符宽度原子累加，末位 = 窗口总宽）
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
        // 超宽取窗：两轮不动点（指示符占列 → 扣减预算重取，直至裁剪侧数稳定）
        let mut reserve = 0usize;
        loop {
            let b = budget.saturating_sub(reserve).max(1);
            // 向右取满，余量向左回退（字符原子：宽字符放不下整列则不取）
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

    /// 点击列 → 字符偏移映射（原 app/mouse.rs click_cursor_offset）：
    /// cols 半开区间 (cols[k], cols[k+1]) 定位字符——落宽字符左半格 = 光标
    /// 在其前、右半格 = 在其后（终端鼠标以格为单位，两侧半格分置惯例）；
    /// rel ≤ 0 → 窗口首（win_lo）；越过窗口末（含尾部 … 指示/右侧 padding）
    /// → win_hi；空字段（cols = [0]）恒 0。
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

    // ---- 宽度工具（自 ui/text.rs 迁移，口径不变） ----

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

    // ---- 编辑状态机（自 app/dialog_keys/routing.rs 迁移） ----

    #[test]
    fn insert_at_cursor_and_capacity() {
        let mut t = TextInput::with_value("abc");
        assert_eq!(t.cursor, 3);
        t.left();
        assert_eq!(t.insert("X", 10), 1);
        assert_eq!(t.value, "abXc");
        assert_eq!(t.cursor, 3, "光标推进插入量");
        // 容量截断式取前缀（现值 4、cap 5 → 只进 1 字符；插在光标 3 处）
        assert_eq!(t.insert("123456", 5), 1);
        assert_eq!(t.value, "abX1c");
        assert_eq!(t.cursor, 4);
        // 多字节字符不切分
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

    // ---- 取窗单源（自 ui/dialog.rs 迁移：三态窗口/cols 自洽/CJK 原子/
    //      未聚焦尾窗） ----

    #[test]
    fn field_window_windows_and_splits() {
        // 预算内整值：pre/post 在光标处切分，窗口覆盖整值
        let mut t = TextInput::with_value("abcdef");
        t.set_cursor(2);
        let fd = t.field_window("", 10, true);
        assert_eq!(fd.pre, "ab");
        assert_eq!(fd.post, "cdef");
        assert_eq!(fd.win.win_lo, 0);
        assert_eq!(fd.win.win_hi, 6);
        assert_eq!(fd.win.cols, vec![0, 1, 2, 3, 4, 5, 6]);
        assert!(!fd.win.left_cut);

        // 未聚焦预算内：pre 空、post 整值（原 field_display 语义，锚 = 尾
        // 与光标位置无关）
        let fd = t.field_window("", 10, false);
        assert_eq!(fd.pre, "");
        assert_eq!(fd.post, "abcdef");

        // 超宽聚焦：光标 8 靠右，窗口锚定光标（左 … 指示）
        let val = "abcdefghijklmnopqrstuvwxyz";
        let mut t = TextInput::with_value(val);
        t.set_cursor(8);
        let fd = t.field_window("", 6, true);
        // 取窗 = 先向右取满、余量向左回退：两轮不动点后右窗 = 12（b=4）
        assert_eq!(fd.win.win_hi, 12, "右窗 = 预算内向右取满");
        assert_eq!(w(&fd.pre) + w(&fd.post), 6, "pre/post 拼接恰满预算");
        assert!(fd.pre.starts_with('…'));
        assert_eq!(fd.win.value_x, 0);
        // cols 末位 = 窗口内容总宽（双缘 … 指示各占 1 列，不在 cols 内）
        let total_cols = *fd.win.cols.last().unwrap() as usize;
        assert_eq!(total_cols, 4, "窗口内容 = 4 窄字符（b=4 不动点）");

        // 聚焦超宽左端（光标 16，右 … 指示）
        let mut t = TextInput::with_value(val);
        t.set_cursor(16);
        let fd = t.field_window("", 6, true);
        assert_eq!(fd.win.win_lo, 16, "锚 = 光标：左侧只到锚");
        assert!(fd.post.ends_with('…'));

        // 光标 0 聚焦：无左裁剪
        let mut t = TextInput::with_value(val);
        t.set_cursor(0);
        let fd = t.field_window("", 6, true);
        assert_eq!(fd.win.win_lo, 0);
        assert!(!fd.win.left_cut);
    }

    #[test]
    fn field_window_cjk_atomic_window() {
        // CJK 原子：预算 3 放不下第二个宽字（2+2 > 3），窗口只含首字
        let mut t = TextInput::with_value("中文中文");
        t.set_cursor(4);
        let fd = t.field_window("", 3, true);
        assert_eq!(fd.win.win_lo, 3);
        assert_eq!(fd.win.win_hi, 4);
        assert_eq!(fd.pre, "…文");
        assert_eq!(fd.post, "");
        // cols 与宽字原子一致（每字符 +2）
        let t = TextInput::with_value("中文");
        let fd = t.field_window("", 10, true);
        assert_eq!(fd.win.cols, vec![0, 2, 4]);
    }

    #[test]
    fn field_window_unfocused_takes_tail_window() {
        // 未聚焦超宽：尾窗语义（= 原 field_display：… + 预算内尾部；
        // 锚 = 字段尾 → 右裁不发生、post 无 …）
        let val = "abcdefghijklmnopqrstuvwxyz";
        let t = TextInput::with_value(val);
        let fd = t.field_window("", 6, false);
        assert_eq!(fd.pre, "…");
        assert_eq!(fd.post, "vwxyz");
        assert_eq!(fd.win.win_lo, 21);
        assert_eq!(fd.win.win_hi, 26);
        // 未聚焦超宽 ASCII 同值窄预算：指示符在 pre、窗口内容在 post
        //（锚 = 字段尾 → 右裁不发生、post 无 …）
        let t = TextInput::with_value("abcdef1234567890");
        let fd = t.field_window("", 6, false);
        assert_eq!(fd.pre, "…");
        assert_eq!(fd.post, "67890");
        assert_eq!((fd.win.win_lo, fd.win.win_hi), (11, 16));
        assert!(fd.win.left_cut);
        // 未聚焦超宽 CJK：宽度口径（按字符数计会溢出）——预算 6 尾窗取
        // 2 个汉字（宽 4 + … 1 = 5 ≤ 6），不取第 3 个
        let t = TextInput::with_value("一二三四五");
        let fd = t.field_window("", 6, false);
        assert_eq!(fd.pre, "…");
        assert_eq!(fd.post, "四五", "CJK 原子尾窗（右裁不发生，无 …）");
        assert_eq!((fd.win.win_lo, fd.win.win_hi), (3, 5));
        // 未聚焦空值：占位提示整窗
        let t = TextInput::new();
        let fd = t.field_window("占位提示", 20, false);
        assert!(fd.pre.is_empty());
        assert_eq!(fd.post, truncate("占位提示", 20));
    }

    #[test]
    fn field_window_clamps_stale_cursor_and_tail_anchor() {
        // 光标越界钳制（陈旧偏移防御）：按字段长取尾
        let t = TextInput::with_value("abc");
        let mut t2 = t.clone();
        t2.cursor = 99;
        let fd = t2.field_window("", 10, true);
        assert_eq!(fd.pre, "abc");
        assert!(fd.post.is_empty());
        assert_eq!(fd.win.win_hi, 3);
        // 聚焦超宽（预算 6，值宽 16）双缘裁剪 + cols 长度自洽 + 光标可见
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
        // 光标在尾：窗贴右（无尾部 …）
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

    // ---- 点击映射（自 app/mouse.rs 迁移：窄字符/宽字半格/左缘指示/
    //      越界/空字段 全分支） ----

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
        // 窄字符：value_x=35，窗口 [0,3)，cols = 0/1/2/3
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

        // 宽字符：cols = 0/2/4，左半格 = 前、右半格 = 后
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

        // 左缘 … 指示：映射扣 1 列
        let cut = win(35, true, vec![0, 1, 2], 5, 7);
        assert_eq!(TextInput::click_offset(&cut, 35), 5, "指示符列 → 窗口首");
        assert_eq!(TextInput::click_offset(&cut, 36), 5, "首字符左半 → win_lo");
        assert_eq!(
            TextInput::click_offset(&cut, 37),
            6,
            "首字符右半 → 下一偏移"
        );

        // 空字段：恒 0
        let empty = win(35, false, vec![0], 0, 0);
        assert_eq!(TextInput::click_offset(&empty, 40), 0);
    }

    #[test]
    fn field_window_geometry_feeds_click_offset_roundtrip() {
        // 取窗产物（回填 value_x 后）与点击映射闭环：窗口几何可直接消费
        let mut t = TextInput::with_value("abc中文def");
        t.set_cursor(5);
        let mut fd = t.field_window("", 10, true);
        fd.win.value_x = 40;
        // 点击「中」左半格（值区起点列 40 + pre 宽 3）→ 光标落其前
        assert_eq!(TextInput::click_offset(&fd.win, 43), 3);
        // 右半格 → 其后
        assert_eq!(TextInput::click_offset(&fd.win, 44), 4);
    }
}
