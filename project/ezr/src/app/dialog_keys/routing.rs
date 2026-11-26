use crossterm::event::KeyCode;

use super::{Dialog, DialogKind, CAP_CK_HEX, CAP_CONNS_DIGITS, CAP_TEXT};
use crate::app::CHECKSUM_ALGOS;
use crate::model::checksum::CK_NONE;
use crate::textfield::TextInput;

const CK_DROPDOWN_LEN: usize = CHECKSUM_ALGOS.len() + 1;

pub const CAP_LIMIT: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextField {
    Url,
    Dir,
    Conns,
    DownLimit,
    UpLimit,
    CkValue,
}

pub fn text_field_at(kind: DialogKind, focus: usize) -> Option<TextField> {
    match (kind, focus) {
        (DialogKind::Add, 0) => Some(TextField::Url),
        (DialogKind::Add, 1) => Some(TextField::Dir),
        (DialogKind::Add, 2)
        | (DialogKind::Modify, 0)
        | (DialogKind::AddBt, 3)
        | (DialogKind::ModifyBt, 1) => Some(TextField::Conns),
        (DialogKind::Add, 3)
        | (DialogKind::AddBt, 4)
        | (DialogKind::Modify, 1)
        | (DialogKind::ModifyBt, 2) => Some(TextField::DownLimit),
        (DialogKind::AddBt, 5) | (DialogKind::ModifyBt, 3) => Some(TextField::UpLimit),
        (DialogKind::Add, 5) | (DialogKind::Modify, 3) => Some(TextField::CkValue),
        _ => None,
    }
}

impl Dialog {
    pub fn field_mut(&mut self, f: TextField) -> &mut TextInput {
        match f {
            TextField::Url => &mut self.url,
            TextField::Dir => &mut self.dir,
            TextField::Conns => &mut self.conns,
            TextField::DownLimit => &mut self.down_limit,
            TextField::UpLimit => &mut self.up_limit,
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

pub fn insert_conns_digit(d: &mut Dialog, c: char) {
    if !c.is_ascii_digit() {
        return;
    }
    if d.conns.len_chars() >= CAP_CONNS_DIGITS && d.conns.cursor >= d.conns.len_chars() {
        d.conns.clear();
    }
    if d.conns.insert(&c.to_string(), CAP_CONNS_DIGITS) > 0 {
        d.conns_edited = true;
    }
}

fn speed_unit_label(unit: &str) -> Option<&'static str> {
    match unit {
        "" => Some(""),
        "b" | "bps" => Some("B/s"),
        "k" | "kb" | "kib" | "kbps" => Some("KiB/s"),
        "m" | "mb" | "mib" | "mbps" => Some("MiB/s"),
        "g" | "gb" | "gib" | "gbps" => Some("GiB/s"),
        _ => None,
    }
}

pub fn speed_unit_mult(unit: &str) -> Option<f64> {
    match unit {
        "" | "b" | "bps" => Some(1.0),
        "k" | "kb" | "kib" | "kbps" => Some(1024.0),
        "m" | "mb" | "mib" | "mbps" => Some(1_048_576.0),
        "g" | "gb" | "gib" | "gbps" => Some(1_073_741_824.0),
        _ => None,
    }
}

pub fn canonical_speed_form(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let split = s
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.'))
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    let (num, unit_raw) = s.split_at(split);
    if num.is_empty() || num.matches('.').count() > 1 {
        return None;
    }
    let n: f64 = num.parse().ok()?;
    if !n.is_finite() || n < 0.0 {
        return None;
    }
    let unit = unit_raw.trim().trim_end_matches("/s").trim().to_lowercase();
    let label = speed_unit_label(&unit)?;
    let num_disp = if num.contains('.') {
        num.to_string()
    } else {
        let t = num.trim_start_matches('0');
        if t.is_empty() {
            "0".to_string()
        } else {
            t.to_string()
        }
    };
    if label.is_empty() {
        return Some(num_disp);
    }
    Some(format!("{num_disp} {label}"))
}

fn speed_display_absorbs(buf: &TextInput, c: char) -> bool {
    if buf.cursor < buf.len_chars() {
        return false;
    }
    if !matches!(c, 'b' | 'B' | 'i' | 'I' | 's' | 'S' | '/') {
        return false;
    }
    match canonical_speed_form(&buf.value) {
        Some(canon) => canon == buf.value && canon.ends_with("/s"),
        None => false,
    }
}

pub fn normalize_speed_display(buf: &mut TextInput) {
    if let Some(canon) = canonical_speed_form(&buf.value) {
        if canon != buf.value {
            buf.set(canon);
        }
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
        KeyCode::Tab => {
            d.ck_open = false;
            d.focus_field(if d.kind == DialogKind::Add { 5 } else { 3 });
        }
        KeyCode::BackTab => {
            d.ck_open = false;
            d.focus_field(if d.kind == DialogKind::Add { 3 } else { 1 });
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
            let f = match d.kind {
                DialogKind::Add => 7,
                DialogKind::AddBt => 7,
                DialogKind::ModifyBt => 5,
                _ => 5,
            };
            d.focus_field(f);
        }
        KeyCode::BackTab => {
            d.proxy_open = false;
            let f = match d.kind {
                DialogKind::Add => 5,
                DialogKind::AddBt => 5,
                DialogKind::ModifyBt => 3,
                _ => 3,
            };
            d.focus_field(f);
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
        (DialogKind::Add, 3)
        | (DialogKind::AddBt, 4)
        | (DialogKind::Modify, 1)
        | (DialogKind::ModifyBt, 2) => {
            d.down_limit.backspace();
            true
        }
        (DialogKind::AddBt, 5) | (DialogKind::ModifyBt, 3) => {
            d.up_limit.backspace();
            true
        }
        (DialogKind::Add, 5) | (DialogKind::Modify, 3) => {
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
        (DialogKind::AddBt, 8) | (DialogKind::ModifyBt, 6) => {
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
            insert_conns_digit(d, c);
            true
        }
        (DialogKind::Add, 3)
        | (DialogKind::AddBt, 4)
        | (DialogKind::Modify, 1)
        | (DialogKind::ModifyBt, 2) => {
            insert_limit(&mut d.down_limit, c);
            true
        }
        (DialogKind::AddBt, 5) | (DialogKind::ModifyBt, 3) => {
            insert_limit(&mut d.up_limit, c);
            true
        }
        _ => false,
    }
}

pub fn insert_limit(buf: &mut TextInput, c: char) {
    if !(c.is_ascii_alphanumeric() || c == ' ' || c == '/' || c == '.') {
        return;
    }
    if speed_display_absorbs(buf, c) {
        return;
    }
    buf.insert(&c.to_string(), CAP_LIMIT);
    normalize_speed_display(buf);
}
