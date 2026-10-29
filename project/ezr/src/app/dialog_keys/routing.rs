use crossterm::event::KeyCode;

use super::{Dialog, DialogKind, CAP_CK_HEX, CAP_CONNS_DIGITS, CAP_TEXT};
use crate::app::CHECKSUM_ALGOS;
use crate::model::checksum::CK_NONE;
use crate::textfield::TextInput;

const CK_DROPDOWN_LEN: usize = CHECKSUM_ALGOS.len() + 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextField {
    Url,
    Dir,
    Conns,
    CkValue,
}

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
        if let Some(t) = text_field_at(self.kind, f) {
            self.field_mut(t).cursor_end();
        }
    }
}

pub fn push_hex_capped(d: &mut Dialog, c: char) {
    if d.ck_type == CK_NONE {
        return;
    }
    if c.is_ascii_hexdigit() && d.ck_value.len_chars() < CAP_CK_HEX {
        d.ck_value.insert(&c.to_string(), CAP_CK_HEX);
    }
}

pub(super) const fn kind_add() -> DialogKind {
    DialogKind::Add
}

pub fn dropdown_open_key(d: &mut Dialog, code: KeyCode) {
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
        (DialogKind::Add, 2)
        | (DialogKind::Modify, 0)
        | (DialogKind::AddBt, 3)
        | (DialogKind::ModifyBt, 1) => {
            d.conns.backspace();
            d.conns_edited = true;
            true
        }
        (DialogKind::Add, 4) | (DialogKind::Modify, 2) => {
            if d.ck_type == CK_NONE {
                return true;
            }
            d.ck_value.backspace();
            true
        }
        (DialogKind::AddBt, 0) => {
            d.torrent.backspace();
            true
        }
        (DialogKind::AddBt, 1) => {
            d.dir.backspace();
            true
        }
        (DialogKind::AddBt, 5) | (DialogKind::ModifyBt, 3) => {
            d.trackers.backspace();
            true
        }
        _ => false,
    }
}

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
