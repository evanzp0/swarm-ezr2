//! dialog_keys 键路由纯函数子模块（保持行为拆分：变异点扫描登记该文件
//! 118 点 > 100 阈值——App 对话框处理器与自由键路由函数分为两个子模块，
//! 各自低于阈值；纯函数无副作用，供 impl 处理器与测试复用。
//!
//! 文本字段已升级为输入框组件实例（[`TextInput`]，值 + 光标状态内聚）：
//! 编辑/光标移动全部走组件单源（原 cursor_insert/cursor_backspace/
//! cursor_left/cursor_right 自由函数退役进组件方法），本模块只保留
//! 字段映射（(kind, focus) → 字段）与按字段分流的薄路由。消费者（本模块
//! 路由、App 键路由、paste）共用映射，防矩阵漂移。

use crossterm::event::KeyCode;

use super::{Dialog, DialogKind, CAP_CK_HEX, CAP_CONNS_DIGITS, CAP_TEXT};
use crate::app::CHECKSUM_ALGOS;
use crate::model::checksum::CK_NONE;
use crate::textfield::TextInput;

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

impl Dialog {
    /// 字段组件可变访问（编辑路由单源借用入口）
    pub fn field_mut(&mut self, f: TextField) -> &mut TextInput {
        match f {
            TextField::Url => &mut self.url,
            TextField::Dir => &mut self.dir,
            TextField::Conns => &mut self.conns,
            TextField::CkValue => &mut self.ck_value,
        }
    }

    /// 焦点切换单源（光标随迁——置到新焦点字段文本尾，输入框惯例；
    /// 非文本焦点不消费光标。全部焦点赋值点（Tab/BackTab、下拉跳焦、
    /// 点击聚焦、校验失败回焦）必须走本方法，防光标滞留旧字段偏移造成
    /// 越位。迁尾实现 = 组件 [`TextInput::cursor_end`]）
    pub fn focus_field(&mut self, f: usize) {
        self.focus = f;
        if let Some(t) = text_field_at(self.kind, f) {
            self.field_mut(t).cursor_end();
        }
    }
}

/// 校验码字段字符追加（Add focus 4 / Modify focus 2 共用）：仅十六进制、
/// ≤ [`CAP_CK_HEX`] 位（SHA-512 上限），光标处插入（组件单源）。
/// v1.32/FR-01-01：算法选中 NONE 时校验码不可输入（逐键拦截，与粘贴
/// 同口径）——NONE 臂直接吞掉输入。
pub fn push_hex_capped(d: &mut Dialog, c: char) {
    if d.ck_type == CK_NONE {
        return;
    }
    if c.is_ascii_hexdigit() && d.ck_value.len_chars() < CAP_CK_HEX {
        d.ck_value.insert(&c.to_string(), CAP_CK_HEX);
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
/// Modify 0=并发 2=校验码）——删除**光标前一字符**（行首无操作，组件单源）。
/// 返回是否命中文本字段（消费口径与字段矩阵一致，由属性测试锁定）。
pub fn text_backspace(kind: DialogKind, d: &mut Dialog, focus: usize) -> bool {
    match (kind, focus) {
        (DialogKind::Add, 0) => {
            d.url.backspace();
            true
        }
        (DialogKind::Add, 1) => {
            d.dir.backspace();
            true
        }
        (DialogKind::Add, 2) | (DialogKind::Modify, 0) => {
            d.conns.backspace();
            d.conns_edited = true;
            true
        }
        (DialogKind::Add, 4) | (DialogKind::Modify, 2) => {
            // v1.32/FR-01-01：NONE 选中后校验码不可输入（退格同拦截）
            if d.ck_type == CK_NONE {
                return true;
            }
            d.ck_value.backspace();
            true
        }
        _ => false,
    }
}

/// 文本字段字符输入（kind 感知：Add 0=URL 1=目录 2=并发；Modify 0=并发。
/// URL/目录 ≤ [`CAP_TEXT`] 字符；并发仅 [`CAP_CONNS_DIGITS`] 位数字）
/// ——**光标处插入**（组件单源）。
pub fn text_char(kind: DialogKind, d: &mut Dialog, focus: usize, c: char) -> bool {
    match (kind, focus) {
        (DialogKind::Add, 0) => {
            d.url.insert(&c.to_string(), CAP_TEXT);
            true
        }
        (DialogKind::Add, 1) => {
            d.dir.insert(&c.to_string(), CAP_TEXT);
            true
        }
        (DialogKind::Add, 2) | (DialogKind::Modify, 0) => {
            if c.is_ascii_digit() {
                d.conns.insert(&c.to_string(), CAP_CONNS_DIGITS);
                d.conns_edited = true;
            }
            true
        }
        _ => false,
    }
}
