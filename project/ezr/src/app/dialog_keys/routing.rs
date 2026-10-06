//! dialog_keys 键路由纯函数子模块（保持行为拆分：变异点扫描登记该文件
//! 118 点 > 100 阈值——App 对话框处理器与自由键路由函数分为两个子模块，
//! 各自低于阈值；纯函数无副作用，供 impl 处理器与测试复用。

use crossterm::event::KeyCode;

use super::{Dialog, DialogKind};
use crate::app::CHECKSUM_ALGOS;

/// 校验码字段字符追加（Add focus 4 / Modify focus 2 共用，DRY 收敛）：
/// 仅十六进制、≤ 128 位（SHA-512 上限）
pub fn push_hex_capped(d: &mut Dialog, c: char) {
    if c.is_ascii_hexdigit() && d.ck_value.chars().count() < 128 {
        d.ck_value.push(c);
    }
}

/// Add 布局标记（text_char 复用：Add 与 Modify 共用文本字段语义，焦点映射
/// 由 kind 区分）
pub(super) const fn kind_add() -> DialogKind {
    DialogKind::Add
}

/// 下拉框展开态逐键处理（Up/Down/Home/End 选择，Enter/Esc 关闭，
/// Tab/BackTab 关闭并跳转焦点，其余键关闭下拉）。消费所有按键。
pub fn dropdown_open_key(d: &mut Dialog, code: KeyCode) {
    match code {
        KeyCode::Up => {
            d.ck_sel = (d.ck_sel + CHECKSUM_ALGOS.len() - 1) % CHECKSUM_ALGOS.len();
            d.ck_type = d.ck_sel;
        }
        KeyCode::Down => {
            d.ck_sel = (d.ck_sel + 1) % CHECKSUM_ALGOS.len();
            d.ck_type = d.ck_sel;
        }
        KeyCode::Home => {
            d.ck_sel = 0;
            d.ck_type = 0;
        }
        KeyCode::End => {
            d.ck_sel = CHECKSUM_ALGOS.len() - 1;
            d.ck_type = d.ck_sel;
        }
        KeyCode::Enter | KeyCode::Esc => d.ck_open = false,
        KeyCode::Tab | KeyCode::BackTab => {
            d.ck_open = false;
            d.focus = if code == KeyCode::Tab { 4 } else { 2 };
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
            d.focus = if d.kind == DialogKind::Add { 6 } else { 4 };
        }
        KeyCode::BackTab => {
            d.proxy_open = false;
            d.focus = if d.kind == DialogKind::Add { 4 } else { 2 };
        }
        _ => d.proxy_open = false,
    }
}

/// 文本字段退格（kind 感知焦点映射：Add 0=URL 1=目录 2=并发 4=校验码；
/// Modify 0=并发 2=校验码）
pub fn text_backspace(kind: DialogKind, d: &mut Dialog, focus: usize) -> bool {
    match (kind, focus) {
        (DialogKind::Add, 0) => {
            d.url.pop();
            true
        }
        (DialogKind::Add, 1) => {
            d.dir.pop();
            true
        }
        (DialogKind::Add, 2) | (DialogKind::Modify, 0) => {
            d.conns.pop();
            d.conns_edited = true;
            true
        }
        (DialogKind::Add, 4) | (DialogKind::Modify, 2) => {
            d.ck_value.pop();
            true
        }
        _ => false,
    }
}

/// 文本字段字符输入（kind 感知：Add 0=URL 1=目录 2=并发；Modify 0=并发。
/// URL/目录 ≤300 字符；并发仅 2 位数字）
pub fn text_char(kind: DialogKind, d: &mut Dialog, focus: usize, c: char) -> bool {
    match (kind, focus) {
        (DialogKind::Add, 0) => {
            if d.url.chars().count() < 300 {
                d.url.push(c);
            }
            true
        }
        (DialogKind::Add, 1) => {
            if d.dir.chars().count() < 300 {
                d.dir.push(c);
            }
            true
        }
        (DialogKind::Add, 2) | (DialogKind::Modify, 0) => {
            if c.is_ascii_digit() && d.conns.chars().count() < 2 {
                d.conns.push(c);
                d.conns_edited = true;
            }
            true
        }
        _ => false,
    }
}
