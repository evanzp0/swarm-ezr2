use crossterm::event::KeyCode;

use crate::textfield::TextInput;

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DialogKind {
    Add,

    Delete,

    Modify,
}

pub struct Dialog {
    pub kind: DialogKind,

    pub url: TextInput,

    pub dir: TextInput,

    pub conns: TextInput,

    pub conns_edited: bool,

    pub ck_type: usize,

    pub ck_value: TextInput,

    pub ck_open: bool,

    pub ck_sel: usize,

    pub proxy_sel: usize,

    pub proxy_open: bool,

    pub focus: usize,

    pub task_name: String,

    pub task_id: Option<u32>,
}

impl Dialog {
    pub fn text_field_at(&self, focus: usize) -> Option<TextField> {
        match (self.kind, focus) {
            (DialogKind::Add, 0) => Some(TextField::Url),
            (DialogKind::Add, 1) => Some(TextField::Dir),
            (DialogKind::Add, 2) | (DialogKind::Modify, 0) => Some(TextField::Conns),
            (DialogKind::Add, 4) | (DialogKind::Modify, 2) => Some(TextField::CkValue),
            _ => None,
        }
    }

    pub fn field_mut(&mut self, f: TextField) -> &mut TextInput {
        match f {
            TextField::Url => &mut self.url,
            TextField::Dir => &mut self.dir,
            TextField::Conns => &mut self.conns,
            TextField::CkValue => &mut self.ck_value,
        }
    }

    pub fn focus_field(&mut self, f: usize) {
        self.focus = f;
        if let Some(t) = self.text_field_at(f) {
            self.field_mut(t).cursor_end();
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextField {
    Url,

    Dir,

    Conns,

    CkValue,
}

pub(super) fn proxy_dropdown_open_key(d: &mut Dialog, n: usize, code: KeyCode) {
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

pub(super) fn ck_dropdown_open_key(d: &mut Dialog, code: KeyCode) {
    match code {
        KeyCode::Up => {
            d.ck_sel = (d.ck_sel + CHECKSUM_ALGOS.len() - 1) % CHECKSUM_ALGOS.len();
            d.ck_type = d.ck_sel;
            if d.ck_type == CK_NONE {
                d.ck_value.clear();
            }
        }
        KeyCode::Down => {
            d.ck_sel = (d.ck_sel + 1) % CHECKSUM_ALGOS.len();
            d.ck_type = d.ck_sel;
            if d.ck_type == CK_NONE {
                d.ck_value.clear();
            }
        }
        KeyCode::Home => {
            d.ck_sel = 0;
            d.ck_type = 0;
            d.ck_value.clear();
        }
        KeyCode::End => {
            d.ck_sel = CHECKSUM_ALGOS.len() - 1;
            d.ck_type = d.ck_sel;
        }
        KeyCode::Enter | KeyCode::Esc => d.ck_open = false,
        KeyCode::Tab | KeyCode::BackTab => {
            d.ck_open = false;
            d.focus_field(if code == KeyCode::Tab { 4 } else { 2 });
        }
        _ => d.ck_open = false,
    }
}

pub(super) fn text_backspace(kind: DialogKind, d: &mut Dialog, focus: usize) -> bool {
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
            d.ck_value.backspace();
            true
        }
        _ => false,
    }
}

pub(super) fn paste_sanitize(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

pub(super) fn dlg_apply_paste(kind: DialogKind, d: &mut Dialog, focus: usize, text: &str) -> bool {
    let clean = paste_sanitize(text);
    match (kind, focus) {
        (DialogKind::Add, 0) => d.url.insert(&clean, 300) > 0,
        (DialogKind::Add, 1) => d.dir.insert(&clean, 300) > 0,
        (DialogKind::Add, 2) | (DialogKind::Modify, 0) => {
            let digits: String = clean.chars().filter(|c| c.is_ascii_digit()).collect();
            let inserted = d.conns.insert(&digits, 2) > 0;
            d.conns_edited |= inserted;
            inserted
        }
        (DialogKind::Add, 4) | (DialogKind::Modify, 2) => {
            if d.ck_type == CK_NONE {
                return false;
            }
            let hex: String = clean.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            d.ck_value.insert(&hex, 128) > 0
        }
        _ => false,
    }
}
