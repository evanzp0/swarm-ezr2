//! dialog_keys 键路由纯函数子模块（保持行为拆分：变异点扫描登记该文件
//! 118 点 > 100 阈值——App 对话框处理器与自由键路由函数分为两个子模块，
//! 各自低于阈值；纯函数无副作用，供 impl 处理器与测试复用。
//!
//! v1.34 二批 bug 修订（输入光标模型）：文本字段从 append/pop 式升级为
//! **光标编辑**——`Dialog::cursor` 为当前焦点字段的字符偏移，方向键在输入
//! 框内移动光标（不再与 Tab 同臂跳焦点）、字符插入/退格/粘贴均在光标处
//! 生效。字段映射（(kind, focus) → 字段）经 [`text_field_at`] 单源收口，
//! 消费者（本模块编辑函数、App 键路由、paste）共用。

use crossterm::event::KeyCode;

use super::{Dialog, DialogKind, CAP_CK_HEX, CAP_CONNS_DIGITS, CAP_TEXT};
use crate::app::CHECKSUM_ALGOS;
use crate::model::checksum::CK_NONE;

/// 下拉项总数（v1.32/FR-01-01：8 项 = 首行 NONE + CHECKSUM_ALGOS 7 种，
/// 单源派生）
const CK_DROPDOWN_LEN: usize = CHECKSUM_ALGOS.len() + 1;

/// 对话框文本字段标识（焦点 → 字段映射单源的载荷）
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextField {
    /// Add URL 字段（focus 0）
    Url,
    /// Add 保存目录字段（focus 1）
    Dir,
    /// Add 并发（focus 2）/ Modify 并发（focus 0）
    Conns,
    /// Add 校验码（focus 4）/ Modify 校验码（focus 2）
    CkValue,
}

/// 焦点 → 文本字段映射（kind 感知，单源：Add 0=URL 1=目录 2=并发 4=校验码；
/// Modify 0=并发 2=校验码；下拉行/按钮/Delete 对话框 = None——方向键/输入
/// 的「是否文本字段」判定与编辑路由共用本映射，防矩阵漂移）
pub fn text_field_at(kind: DialogKind, focus: usize) -> Option<TextField> {
    match (kind, focus) {
        (DialogKind::Add, 0) => Some(TextField::Url),
        (DialogKind::Add, 1) => Some(TextField::Dir),
        (DialogKind::Add, 2) | (DialogKind::Modify, 0) => Some(TextField::Conns),
        (DialogKind::Add, 4) | (DialogKind::Modify, 2) => Some(TextField::CkValue),
        _ => None,
    }
}

/// 第 n 个字符的字节偏移（前 n 字符 UTF-8 长度和；n ≤ 字符数时结果恒为
/// char 边界——String::insert/remove 安全前提）
fn char_byte_offset(s: &str, n: usize) -> usize {
    s.chars().take(n).map(char::len_utf8).sum()
}

/// 光标处插入文本（容量按字符数计，不切分多字节字符）；返回插入的字符数
///（容量不足时截断式取前缀，与原 push_capped 同口径）。光标推进插入量。
pub fn cursor_insert(s: &mut String, cursor: &mut usize, text: &str, cap: usize) -> usize {
    let room = cap.saturating_sub(s.chars().count());
    let taken: Vec<char> = text.chars().take(room).collect();
    let n = taken.len();
    if n > 0 {
        // 逐字符递进字节偏移插入（同偏移循环插入会倒序）
        let mut off = char_byte_offset(s, *cursor);
        for c in taken {
            s.insert(off, c);
            off += c.len_utf8();
        }
        *cursor += n;
    }
    n
}

/// 光标处删除前一字符（cursor = 0 时无操作——行首退格钳制）
pub fn cursor_backspace(s: &mut String, cursor: &mut usize) {
    if *cursor > 0 {
        let off = char_byte_offset(s, *cursor - 1);
        s.remove(off);
        *cursor -= 1;
    }
}

/// 光标左移一格（0 处钳制）
pub fn cursor_left(cursor: &mut usize) {
    *cursor = cursor.saturating_sub(1);
}

/// 光标右移一格（字段尾钳制）
pub fn cursor_right(s: &str, cursor: &mut usize) {
    let len = s.chars().count();
    if *cursor < len {
        *cursor += 1;
    }
}

impl Dialog {
    /// 指定字段缓冲的字符长度（光标 End 语义的长度读取单源）
    fn field_len_of(&self, f: TextField) -> usize {
        match f {
            TextField::Url => self.url.chars().count(),
            TextField::Dir => self.dir.chars().count(),
            TextField::Conns => self.conns.chars().count(),
            TextField::CkValue => self.ck_value.chars().count(),
        }
    }

    /// 焦点切换单源（v1.34 二批 bug 修订：光标随迁——置到新焦点字段的
    /// 文本尾，输入框惯例；非文本焦点光标置 0 不消费）。全部焦点赋值点
    /// （Tab/BackTab、下拉跳焦、点击聚焦、校验失败回焦）必须走本方法，
    /// 防光标滞留旧字段偏移造成越位。
    pub fn focus_field(&mut self, f: usize) {
        self.focus = f;
        self.cursor = text_field_at(self.kind, f)
            .map(|t| self.field_len_of(t))
            .unwrap_or(0);
    }

    /// 光标移到指定字段文本尾（Down = 单行字段 End 语义；不切焦点）
    pub fn cursor_field_end(&mut self, f: TextField) {
        self.cursor = self.field_len_of(f);
    }
}

/// 校验码字段字符追加（Add focus 4 / Modify focus 2 共用）：仅十六进制、
/// ≤ [`CAP_CK_HEX`] 位（SHA-512 上限），光标处插入。
/// v1.32/FR-01-01：算法选中 NONE 时校验码不可输入（逐键拦截，与粘贴
/// 同口径）——NONE 臂直接吞掉输入。
pub fn push_hex_capped(d: &mut Dialog, c: char) {
    if d.ck_type == CK_NONE {
        return;
    }
    if c.is_ascii_hexdigit() && d.ck_value.chars().count() < CAP_CK_HEX {
        cursor_insert(&mut d.ck_value, &mut d.cursor, &c.to_string(), CAP_CK_HEX);
    }
}

/// Add 布局标记（text_char 复用：Add 与 Modify 共用文本字段语义，焦点映射
/// 由 kind 区分）
pub(super) const fn kind_add() -> DialogKind {
    DialogKind::Add
}

/// 下拉框展开态逐键处理（v1.32/FR-01-01 修订：8 项 = 首行 NONE + 7 算法；
/// ↑↓/Home/End 选择**选中即改值**（ck_type 同步），选中 NONE 时校验码**实时
/// 清空**且此后不可输入；Enter/Esc 关闭，Tab/BackTab 关闭并跳转焦点，
/// 其余键关闭下拉）。消费所有按键。
pub fn dropdown_open_key(d: &mut Dialog, code: KeyCode) {
    /// 选中即改值 + NONE 臂实时清空校验码（↑↓/Home/End 三臂共用）
    fn select(d: &mut Dialog, i: usize) {
        d.ck_sel = i;
        d.ck_type = i;
        if i == CK_NONE {
            d.ck_value.clear();
        }
    }
    match code {
        KeyCode::Up => select(d, (d.ck_sel + CK_DROPDOWN_LEN - 1) % CK_DROPDOWN_LEN),
        KeyCode::Down => select(d, (d.ck_sel + 1) % CK_DROPDOWN_LEN),
        KeyCode::Home => select(d, CK_NONE),
        KeyCode::End => select(d, CK_DROPDOWN_LEN - 1),
        KeyCode::Enter | KeyCode::Esc => d.ck_open = false,
        KeyCode::Tab | KeyCode::BackTab => {
            d.ck_open = false;
            d.focus_field(if code == KeyCode::Tab { 4 } else { 2 });
        }
        _ => d.ck_open = false,
    }
}

/// 代理下拉框展开态逐键处理（v1.5/FR-01-86；Up/Down/Home/End 选择，
/// Enter/Esc 关闭，Tab/BackTab 关闭并跳转焦点，其余键关闭）。消费所有按键。
pub fn proxy_dropdown_open_key(d: &mut Dialog, n: usize, code: KeyCode) {
    let n = n.max(1);
    match code {
        KeyCode::Up => {
            d.proxy_sel = (d.proxy_sel + n - 1) % n;
        }
        KeyCode::Down => {
            d.proxy_sel = (d.proxy_sel + 1) % n;
        }
        KeyCode::Home => d.proxy_sel = 0,
        KeyCode::End => d.proxy_sel = n - 1,
        KeyCode::Enter | KeyCode::Esc => d.proxy_open = false,
        KeyCode::Tab => {
            d.proxy_open = false;
            d.focus_field(if d.kind == DialogKind::Add { 6 } else { 4 });
        }
        KeyCode::BackTab => {
            d.proxy_open = false;
            d.focus_field(if d.kind == DialogKind::Add { 4 } else { 2 });
        }
        _ => d.proxy_open = false,
    }
}

/// 文本字段退格（kind 感知焦点映射：Add 0=URL 1=目录 2=并发 4=校验码；
/// Modify 0=并发 2=校验码）——v1.34 二批 bug 修订：删除**光标前一字符**
///（行首无操作），原 append/pop 式仅弹末尾字符。返回是否命中文本字段
///（消费口径与字段矩阵一致，由属性测试锁定）。
pub fn text_backspace(kind: DialogKind, d: &mut Dialog, focus: usize) -> bool {
    match (kind, focus) {
        (DialogKind::Add, 0) => {
            cursor_backspace(&mut d.url, &mut d.cursor);
            true
        }
        (DialogKind::Add, 1) => {
            cursor_backspace(&mut d.dir, &mut d.cursor);
            true
        }
        (DialogKind::Add, 2) | (DialogKind::Modify, 0) => {
            cursor_backspace(&mut d.conns, &mut d.cursor);
            d.conns_edited = true;
            true
        }
        (DialogKind::Add, 4) | (DialogKind::Modify, 2) => {
            // v1.32/FR-01-01：NONE 选中后校验码不可输入（退格同拦截）
            if d.ck_type == CK_NONE {
                return true;
            }
            cursor_backspace(&mut d.ck_value, &mut d.cursor);
            true
        }
        _ => false,
    }
}

/// 文本字段字符输入（kind 感知：Add 0=URL 1=目录 2=并发；Modify 0=并发。
/// URL/目录 ≤ [`CAP_TEXT`] 字符；并发仅 [`CAP_CONNS_DIGITS`] 位数字）
/// ——v1.34 二批 bug 修订：**光标处插入**（原 append 式恒加末尾）。
pub fn text_char(kind: DialogKind, d: &mut Dialog, focus: usize, c: char) -> bool {
    match (kind, focus) {
        (DialogKind::Add, 0) => {
            cursor_insert(&mut d.url, &mut d.cursor, &c.to_string(), CAP_TEXT);
            true
        }
        (DialogKind::Add, 1) => {
            cursor_insert(&mut d.dir, &mut d.cursor, &c.to_string(), CAP_TEXT);
            true
        }
        (DialogKind::Add, 2) | (DialogKind::Modify, 0) => {
            if c.is_ascii_digit() {
                cursor_insert(
                    &mut d.conns,
                    &mut d.cursor,
                    &c.to_string(),
                    CAP_CONNS_DIGITS,
                );
                d.conns_edited = true;
            }
            true
        }
        _ => false,
    }
}
