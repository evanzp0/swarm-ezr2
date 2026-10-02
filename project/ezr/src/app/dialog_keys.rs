//! dialog_keys — 对话框逐键处理（导航/Enter/退格/字符输入与下拉选择）

use crossterm::event::KeyCode;

use super::{Dialog, DialogKind};
use crate::app::CHECKSUM_ALGOS;

impl super::App {
    pub(super) fn on_dialog_key(&mut self, code: KeyCode) {
        let (kind, focus) = match &self.dialog {
            Some(d) => (d.kind, d.focus),
            None => return,
        };
        let nfocus = match kind {
            DialogKind::Add => 7,
            DialogKind::Delete => 3,
        };
        // 校验算法下拉框展开时（仅 Add）：整块交给下拉键处理
        if kind == DialogKind::Add && self.dialog.as_ref().is_some_and(|d| d.ck_open) {
            if let Some(d) = self.dialog.as_mut() {
                dropdown_open_key(d, code);
            }
            return;
        }
        match code {
            KeyCode::Esc => self.dialog = None,
            KeyCode::Tab | KeyCode::Down | KeyCode::Right => {
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = (d.focus + 1) % nfocus;
                }
            }
            KeyCode::BackTab | KeyCode::Up | KeyCode::Left => {
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = (d.focus + nfocus - 1) % nfocus;
                }
            }
            KeyCode::Enter => match kind {
                DialogKind::Add => self.add_dialog_enter(focus),
                DialogKind::Delete => self.dlg_activate_delete(focus),
            },
            KeyCode::Backspace => {
                if kind == DialogKind::Add {
                    if let Some(d) = self.dialog.as_mut() {
                        text_backspace(d, focus);
                    }
                }
            }
            KeyCode::Char(c) => match kind {
                DialogKind::Add => self.add_dialog_char(focus, c),
                DialogKind::Delete => match c {
                    '1' => self.dlg_activate_delete(0),
                    '2' => self.dlg_activate_delete(1),
                    '3' | ' ' => self.dlg_activate_delete(2),
                    _ => {}
                },
            },
            _ => {}
        }
    }

    /// Add 对话框 Enter：算法行展开下拉、取消按钮关闭、其余确认
    fn add_dialog_enter(&mut self, focus: usize) {
        if focus == 3 {
            let d = self.dialog.as_mut().unwrap();
            d.ck_open = true;
            d.ck_sel = d.ck_type;
        } else if focus == 6 {
            self.dialog = None;
        } else {
            self.dlg_confirm_add();
        }
    }

    /// Add 对话框字符输入：0-2 文本字段、3 空格展开下拉、4 校验码、5/6 空格激活按钮
    fn add_dialog_char(&mut self, focus: usize, c: char) {
        if focus < 3 {
            if !c.is_control() {
                if let Some(d) = self.dialog.as_mut() {
                    text_char(d, focus, c);
                }
            }
        } else if focus == 3 {
            if c == ' ' {
                let d = self.dialog.as_mut().unwrap();
                d.ck_open = true;
                d.ck_sel = d.ck_type;
            }
        } else if focus == 4 {
            // 校验码：仅十六进制，最多 128 位（SHA-512）
            if c.is_ascii_hexdigit() && self.dialog.as_ref().unwrap().ck_value.chars().count() < 128
            {
                self.dialog.as_mut().unwrap().ck_value.push(c);
            }
        } else if c == ' ' {
            self.dlg_activate_add(focus);
        }
    }
}

/// 下拉框展开态逐键处理（Up/Down/Home/End 选择，Enter/Esc 关闭，
/// Tab/BackTab 关闭并跳转焦点，其余键关闭下拉）。消费所有按键。
fn dropdown_open_key(d: &mut Dialog, code: KeyCode) {
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

/// 文本字段退格（focus 0=URL 1=目录 2=并发 4=校验码；其余字段 false = 未消费）
fn text_backspace(d: &mut Dialog, focus: usize) -> bool {
    match focus {
        0 => {
            d.url.pop();
            true
        }
        1 => {
            d.dir.pop();
            true
        }
        2 => {
            d.conns.pop();
            d.conns_edited = true;
            true
        }
        4 => {
            d.ck_value.pop();
            true
        }
        _ => false,
    }
}

/// 文本字段字符输入（URL/目录 ≤300 字符；并发仅 2 位数字；其余字段 false = 未消费）
fn text_char(d: &mut Dialog, focus: usize, c: char) -> bool {
    match focus {
        0 => {
            if d.url.chars().count() < 300 {
                d.url.push(c);
            }
            true
        }
        1 => {
            if d.dir.chars().count() < 300 {
                d.dir.push(c);
            }
            true
        }
        2 => {
            if c.is_ascii_digit() && d.conns.chars().count() < 2 {
                d.conns.push(c);
                d.conns_edited = true;
            }
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod dialog_key_tests {
    use super::*;
    use crate::app::{Dialog, DialogKind};

    fn add_dlg() -> Dialog {
        let mut d = Dialog {
            kind: DialogKind::Add,
            url: String::new(),
            dir: String::new(),
            conns: String::new(),
            conns_edited: false,
            ck_type: 3,
            ck_value: String::new(),
            ck_open: false,
            ck_sel: 3,
            focus: 0,
            task_name: String::new(),
        };
        d.ck_sel = 3;
        d
    }

    #[test]
    fn dropdown_open_key_navigation_and_close() {
        let mut d = add_dlg();
        d.ck_open = true;
        d.ck_sel = 0;
        dropdown_open_key(&mut d, KeyCode::Up);
        assert_eq!(d.ck_sel, CHECKSUM_ALGOS.len() - 1, "Up 环绕到末尾");
        assert_eq!(d.ck_type, d.ck_sel);
        dropdown_open_key(&mut d, KeyCode::Down);
        assert_eq!(d.ck_sel, 0, "Down 环绕回首");
        dropdown_open_key(&mut d, KeyCode::End);
        assert_eq!(d.ck_sel, CHECKSUM_ALGOS.len() - 1);
        dropdown_open_key(&mut d, KeyCode::Home);
        assert_eq!(d.ck_sel, 0);
        dropdown_open_key(&mut d, KeyCode::Enter);
        assert!(!d.ck_open);
        // Tab：关闭下拉并跳到校验码字段
        d.ck_open = true;
        dropdown_open_key(&mut d, KeyCode::Tab);
        assert!(!d.ck_open);
        assert_eq!(d.focus, 4);
        d.ck_open = true;
        dropdown_open_key(&mut d, KeyCode::BackTab);
        assert_eq!(d.focus, 2);
        // 其余键：仅关闭
        d.ck_open = true;
        dropdown_open_key(&mut d, KeyCode::Char('x'));
        assert!(!d.ck_open);
    }

    #[test]
    fn text_backspace_fields_and_gates() {
        let mut d = add_dlg();
        d.url.push_str("abc");
        assert!(text_backspace(&mut d, 0));
        assert_eq!(d.url, "ab");
        d.dir.push_str("xy");
        assert!(text_backspace(&mut d, 1));
        assert_eq!(d.dir, "x");
        d.conns.push_str("12");
        assert!(text_backspace(&mut d, 2));
        assert_eq!(d.conns, "1");
        assert!(d.conns_edited);
        d.ck_value.push_str("ff");
        assert!(text_backspace(&mut d, 4));
        assert_eq!(d.ck_value, "f");
        // 焦点 3（算法下拉）/5/6（按钮）不消费
        assert!(!text_backspace(&mut d, 3));
        assert!(!text_backspace(&mut d, 5));
    }

    #[test]
    fn text_char_caps_and_filters() {
        let mut d = add_dlg();
        for c in "abcdefghij".chars() {
            text_char(&mut d, 0, c);
        }
        assert_eq!(d.url.chars().count(), 10);
        // 300 上限
        for c in ('a'..='z').cycle().take(400) {
            text_char(&mut d, 0, c);
        }
        assert_eq!(d.url.chars().count(), 300);
        // 并发：仅数字、2 位
        text_char(&mut d, 2, '5');
        text_char(&mut d, 2, 'x'); // 非数字拒绝
        text_char(&mut d, 2, '6');
        text_char(&mut d, 2, '7'); // 第 3 位拒绝
        assert_eq!(d.conns, "56");
        assert!(d.conns_edited);
    }
}
