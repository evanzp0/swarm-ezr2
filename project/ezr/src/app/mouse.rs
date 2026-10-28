use std::time::Instant;

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::dialog_keys::text_field_at;
use super::{App, Dialog, DialogKind, Panel, SbKey, ITEM_HEIGHT};
use crate::model::checksum::CK_NONE;
use crate::textfield::TextInput;

impl App {
    pub fn on_mouse(&mut self, m: MouseEvent) {
        self.pointer = Some((m.column, m.row));
        if self.val_open {
            self.on_mouse_val(m);
            return;
        }
        if self.about_open {
            self.on_mouse_about(m);
            return;
        }
        if self.help_open {
            self.on_mouse_help(m);
            return;
        }
        if self.pieces_open {
            self.on_mouse_pieces(m);
            return;
        }
        if self.err_open {
            self.on_mouse_err(m);
            return;
        }
        if self.cmd_open {
            self.on_mouse_cmd(m);
            return;
        }
        if self.dialog.is_some() {
            self.on_mouse_dialog(m);
        } else {
            self.on_mouse_main(m);
        }
    }

    fn on_mouse_about(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.footer_press = None;
                if pointer_in(&self.popup_close_rect, m.column, m.row) {
                    self.x_press = true;
                    return;
                }
                if pointer_in(&self.about_area, m.column, m.row) {
                    if let Some(r) = self.about_area {
                        self.about_drag =
                            Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some((dx, dy)) = self.about_drag {
                    self.about_pos = Some((m.column.saturating_sub(dx), m.row.saturating_sub(dy)));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.about_drag = None;
                if self.x_press {
                    let hit = pointer_in(&self.popup_close_rect, m.column, m.row);
                    self.x_press = false;
                    if hit {
                        self.close_about();
                    }
                }
                self.sb_release();
            }
            _ => {}
        }
    }

    fn on_mouse_help(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::ScrollUp => {
                self.help_scroll = self.help_scroll.saturating_sub(1);
            }
            MouseEventKind::ScrollDown => self.help_scroll += 1,
            MouseEventKind::Down(MouseButton::Left) => {
                self.footer_press = None;
                if pointer_in(&self.popup_close_rect, m.column, m.row) {
                    self.x_press = true;
                    return;
                }
                if let Some(bar) = SbKey::Help.bar(self) {
                    if let Some(hit) = bar.hit(m.column, m.row) {
                        self.sb_press(SbKey::Help, &bar, hit);
                        return;
                    }
                }
                if pointer_in(&self.help_area, m.column, m.row) {
                    if let Some(r) = self.help_area {
                        self.help_drag =
                            Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.sb_arrow_follow(m.column, m.row);
                if self.sb_drag.is_some() {
                    self.sb_drag_move(m.column, m.row);
                } else if let Some((dx, dy)) = self.help_drag {
                    self.help_pos = Some((m.column.saturating_sub(dx), m.row.saturating_sub(dy)));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.help_drag = None;
                if self.x_press {
                    let hit = pointer_in(&self.popup_close_rect, m.column, m.row);
                    self.x_press = false;
                    if hit {
                        self.help_open = false;
                    }
                }
                self.sb_release();
            }
            _ => {}
        }
    }

    fn on_mouse_pieces(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::ScrollUp => self.pieces_scroll_by(-2),
            MouseEventKind::ScrollDown => self.pieces_scroll_by(2),
            MouseEventKind::Down(MouseButton::Left) => {
                self.footer_press = None;
                if pointer_in(&self.popup_close_rect, m.column, m.row) {
                    self.x_press = true;
                    return;
                }
                if let Some(bar) = SbKey::Pieces.bar(self) {
                    if let Some(hit) = bar.hit(m.column, m.row) {
                        self.sb_press(SbKey::Pieces, &bar, hit);
                        return;
                    }
                }
                if pointer_in(&self.pieces_area, m.column, m.row) {
                    if let Some(r) = self.pieces_area {
                        self.pieces_drag =
                            Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.sb_arrow_follow(m.column, m.row);
                if self.sb_drag.is_some() {
                    self.sb_drag_move(m.column, m.row);
                } else if let Some((dx, dy)) = self.pieces_drag {
                    self.pieces_pos = Some((m.column.saturating_sub(dx), m.row.saturating_sub(dy)));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.pieces_drag = None;
                if self.x_press {
                    let hit = pointer_in(&self.popup_close_rect, m.column, m.row);
                    self.x_press = false;
                    if hit {
                        self.close_pieces();
                    }
                }
                self.sb_release();
            }
            _ => {}
        }
    }

    fn on_mouse_val(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::ScrollUp => self.sb_scroll(SbKey::Val, -1),
            MouseEventKind::ScrollDown => self.sb_scroll(SbKey::Val, 1),
            MouseEventKind::Down(MouseButton::Left) => {
                self.footer_press = None;
                if let Some(bar) = SbKey::Val.bar(self) {
                    if let Some(hit) = bar.hit(m.column, m.row) {
                        self.sb_press(SbKey::Val, &bar, hit);
                        return;
                    }
                }
                if pointer_in(&self.popup_close_rect, m.column, m.row) {
                    self.x_press = true;
                    return;
                }
                if pointer_in(&self.val_area, m.column, m.row) {
                    if let Some(r) = self.val_area {
                        self.val_drag =
                            Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.sb_arrow_follow(m.column, m.row);
                if self.sb_drag.is_some() {
                    self.sb_drag_move(m.column, m.row);
                } else if let Some((dx, dy)) = self.val_drag {
                    self.val_pos = Some((m.column.saturating_sub(dx), m.row.saturating_sub(dy)));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.val_drag = None;
                if self.x_press {
                    let hit = pointer_in(&self.popup_close_rect, m.column, m.row);
                    self.x_press = false;
                    if hit {
                        self.close_val_popup();
                    }
                }
                self.sb_release();
            }
            _ => {}
        }
    }

    fn on_mouse_cmd(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::ScrollUp => self.sb_scroll(SbKey::CmdText, -1),
            MouseEventKind::ScrollDown => self.sb_scroll(SbKey::CmdText, 1),
            MouseEventKind::Down(MouseButton::Left) => {
                self.footer_press = None;
                if pointer_in(&self.popup_close_rect, m.column, m.row) {
                    self.x_press = true;
                    return;
                }
                if let Some(bar) = SbKey::CmdText.bar(self) {
                    if let Some(hit) = bar.hit(m.column, m.row) {
                        self.sb_press(SbKey::CmdText, &bar, hit);
                        return;
                    }
                }
                if pointer_in(&self.cmd_area, m.column, m.row) {
                    if let Some(r) = self.cmd_area {
                        self.cmd_drag =
                            Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                        self.cmd_drag_moved = false;
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.x_press {
                    self.x_press = pointer_in(&self.popup_close_rect, m.column, m.row);
                    return;
                }
                if self.sb_drag.is_some() {
                    self.sb_drag_move(m.column, m.row);
                    return;
                }
                if let Some((dx, dy)) = self.cmd_drag {
                    let new = (m.column.saturating_sub(dx), m.row.saturating_sub(dy));
                    if self.cmd_pos != Some(new) {
                        self.cmd_pos = Some(new);
                        self.cmd_drag_moved = true;
                    }
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if self.x_press {
                    let hit = pointer_in(&self.popup_close_rect, m.column, m.row);
                    self.x_press = false;
                    self.sb_release();
                    if hit {
                        self.close_cmd_window();
                    }
                    return;
                }
                self.sb_release();
                if self.cmd_drag.is_some() {
                    let moved = self.cmd_drag_moved;
                    self.cmd_drag = None;
                    self.cmd_drag_moved = false;
                    if !moved {
                        if pointer_in(&self.cmd_btn_rect, m.column, m.row) {
                            self.cmd_focus = 1;
                            self.cmd_confirm();
                        } else if pointer_in(&self.cmd_text_area, m.column, m.row) {
                            self.cmd_focus = 0;
                            if let Some(a) = self.cmd_text_area {
                                self.cmd.click_at(
                                    a.width as usize,
                                    (m.row - a.y) as usize,
                                    m.column,
                                    a.x,
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn on_mouse_err(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.footer_press = None;
                if pointer_in(&self.popup_close_rect, m.column, m.row) {
                    self.x_press = true;
                    return;
                }
                if pointer_in(&self.err_btn_rect, m.column, m.row) {
                    self.close_error_window();
                    return;
                }
                if pointer_in(&self.err_area, m.column, m.row) {
                    if let Some(r) = self.err_area {
                        self.err_drag =
                            Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.x_press {
                    self.x_press = pointer_in(&self.popup_close_rect, m.column, m.row);
                    return;
                }
                if let Some((dx, dy)) = self.err_drag {
                    self.err_pos = Some((m.column.saturating_sub(dx), m.row.saturating_sub(dy)));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.err_drag = None;
                if self.x_press {
                    let hit = pointer_in(&self.popup_close_rect, m.column, m.row);
                    self.x_press = false;
                    if hit {
                        self.close_error_window();
                    }
                }
            }
            _ => {}
        }
    }

    fn on_mouse_dialog(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::ScrollUp => {
                if pointer_in(&self.dlg_trackers_area, m.column, m.row) {
                    self.sb_scroll(SbKey::BtTrackers, -1);
                    return;
                }
                if pointer_in(&self.dlg_bt_area, m.column, m.row) {
                    if let Some(d) = self.dialog.as_mut() {
                        d.bt_follow = false;
                    }
                    self.sb_scroll(SbKey::BtFiles, -1);
                    return;
                }
            }
            MouseEventKind::ScrollDown => {
                if pointer_in(&self.dlg_trackers_area, m.column, m.row) {
                    self.sb_scroll(SbKey::BtTrackers, 1);
                    return;
                }
                if pointer_in(&self.dlg_bt_area, m.column, m.row) {
                    if let Some(d) = self.dialog.as_mut() {
                        d.bt_follow = false;
                    }
                    self.sb_scroll(SbKey::BtFiles, 1);
                    return;
                }
            }
            MouseEventKind::ScrollLeft => {
                if pointer_in(&self.dlg_trackers_area, m.column, m.row) {
                    self.sb_scroll(SbKey::BtTrackersH, -1);
                }
                return;
            }
            MouseEventKind::ScrollRight => {
                if pointer_in(&self.dlg_trackers_area, m.column, m.row) {
                    self.sb_scroll(SbKey::BtTrackersH, 1);
                }
                return;
            }
            _ => {}
        }
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let kind = self.dialog.as_ref().map(|d| d.kind);
                let ck_open = self.dialog.as_ref().is_some_and(|d| d.ck_open);
                let proxy_open = self.dialog.as_ref().is_some_and(|d| d.proxy_open);
                self.footer_press = None;
                if (ck_open || proxy_open) && pointer_in(&self.popup_close_rect, m.column, m.row) {
                    if let Some(d) = self.dialog.as_mut() {
                        d.ck_open = false;
                        d.proxy_open = false;
                    }
                    return;
                }
                if pointer_in(&self.popup_close_rect, m.column, m.row) {
                    self.x_press = true;
                    return;
                }
                let hit = |rects: &[(Rect, usize)]| rect_hit(rects, m.column, m.row);
                if ck_open {
                    let ck_rects = self.dlg_ck_rects.clone();
                    if let Some(i) = hit(&ck_rects) {
                        let d = self.dialog.as_mut().unwrap();
                        d.ck_type = i;
                        d.ck_sel = i;
                        if d.ck_type == CK_NONE {
                            d.ck_value.clear();
                        }
                        d.ck_open = false;
                    } else if let Some(d) = self.dialog.as_mut() {
                        d.ck_open = false;
                    }
                    return;
                }
                if proxy_open {
                    let proxy_rects = self.dlg_proxy_rects.clone();
                    if let Some(i) = hit(&proxy_rects) {
                        let d = self.dialog.as_mut().unwrap();
                        d.proxy_sel = i;
                        d.proxy_open = false;
                    } else if let Some(d) = self.dialog.as_mut() {
                        d.proxy_open = false;
                    }
                    return;
                }
                for (key, bar) in [
                    (SbKey::BtFiles, self.dlg_bt_sb),
                    (SbKey::BtTrackers, self.dlg_trackers_sb),
                    (SbKey::BtTrackersH, self.dlg_trackers_h_sb),
                ] {
                    if let Some(bar) = bar {
                        if let Some(hit) = bar.hit(m.column, m.row) {
                            self.sb_press(key, &bar, hit);
                            return;
                        }
                    }
                }
                let btbtn_rects = self.dlg_btbtn_rects.clone();
                if let Some(i) = hit(&btbtn_rects) {
                    if let Some(d) = self.dialog.as_mut() {
                        d.focus = Dialog::bt_list_focus(d.kind);
                    }
                    match i {
                        0 => {
                            if let Some(d) = self.dialog.as_mut() {
                                d.bt_select_all(true);
                            }
                        }
                        1 => {
                            if let Some(d) = self.dialog.as_mut() {
                                d.bt_select_all(false);
                            }
                        }
                        _ => {}
                    }
                    return;
                }
                let btfile_rects = self.dlg_btfile_rects.clone();
                if let Some((rect, i)) = btfile_rects
                    .iter()
                    .find(|(r, _)| {
                        m.column >= r.x
                            && m.column < r.x.saturating_add(r.width)
                            && m.row >= r.y
                            && m.row < r.y.saturating_add(r.height)
                    })
                    .map(|(r, i)| (*r, *i))
                {
                    let list_focus = match kind {
                        Some(DialogKind::AddBt) => 2,
                        Some(DialogKind::ModifyBt) => 0,
                        _ => 0,
                    };
                    let meta = self
                        .dialog
                        .as_ref()
                        .and_then(|d| d.bt_rows().get(i).map(|(_, n)| (n.is_dir, n.depth)));
                    if let Some(d) = self.dialog.as_mut() {
                        d.focus = list_focus;
                        d.bt_cursor = i;
                        d.bt_follow = true;
                        let (is_dir, depth) = meta.unwrap_or((false, 0));
                        let cb_lo = rect.x + 2 + (depth.min(8) * 2) as u16;
                        let cb_hi = cb_lo.saturating_add(3);
                        let on_checkbox = m.column >= cb_lo && m.column < cb_hi;
                        if is_dir && !on_checkbox {
                            d.bt_toggle_expand_at_cursor();
                        } else {
                            d.bt_toggle_cursor();
                        }
                    }
                    return;
                }
                let field_rects = self.dlg_field_rects.clone();
                let btn_rects = self.dlg_btn_rects.clone();
                if let Some(i) = hit(&field_rects) {
                    let ck_i = match kind {
                        Some(DialogKind::Add) => 3,
                        Some(DialogKind::Modify) => 1,
                        _ => usize::MAX,
                    };
                    let proxy_i = match kind {
                        Some(DialogKind::Add) => 5,
                        Some(DialogKind::Modify) => 3,
                        Some(DialogKind::AddBt) => 4,
                        Some(DialogKind::ModifyBt) => 2,
                        _ => usize::MAX,
                    };
                    let is_trackers = self
                        .dialog
                        .as_ref()
                        .is_some_and(|d| d.trackers_focus() == Some(i));
                    let trackers_area = self.dlg_trackers_area;
                    let d = self.dialog.as_mut().unwrap();
                    d.focus_field(i);
                    if is_trackers {
                        if let Some(a) = trackers_area {
                            if m.row >= a.y && m.row < a.y.saturating_add(a.height) {
                                d.trackers
                                    .click_at_nowrap((m.row - a.y) as usize, m.column, a.x);
                            }
                        }
                    } else if let Some(win) = self.dlg_field_wins.get(i).and_then(|w| w.as_ref()) {
                        if let Some(f) = text_field_at(d.kind, i) {
                            d.field_mut(f)
                                .set_cursor(TextInput::click_offset(win, m.column));
                        }
                    }
                    if i == ck_i {
                        d.ck_open = true;
                        d.ck_sel = d.ck_type;
                    } else if i == proxy_i {
                        d.proxy_open = true;
                    }
                } else if let Some(btn) = hit(&btn_rects) {
                    if let Some(d) = self.dialog.as_mut() {
                        d.focus_field(btn);
                    }
                    match kind {
                        Some(DialogKind::Add) => self.dlg_activate_add(btn),
                        Some(DialogKind::Modify) => self.dlg_activate_modify(btn),
                        Some(DialogKind::Delete) => self.dlg_activate_delete(btn),
                        Some(DialogKind::AddBt) => self.dlg_activate_add_bt(btn),
                        Some(DialogKind::ModifyBt) => self.dlg_activate_modify_bt(btn),
                        None => {}
                    }
                } else if pointer_in(&self.dlg_area, m.column, m.row) {
                    if let Some(r) = self.dlg_area {
                        self.dlg_drag =
                            Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some((dx, dy)) = self.dlg_drag {
                    self.dlg_pos = Some((m.column.saturating_sub(dx), m.row.saturating_sub(dy)));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.dlg_drag = None;
                if self.x_press {
                    let hit = pointer_in(&self.popup_close_rect, m.column, m.row);
                    self.x_press = false;
                    if hit {
                        self.dialog = None;
                    }
                }
            }
            _ => {}
        }
    }

    fn on_mouse_main(&mut self, m: MouseEvent) {
        let over_conns = pointer_in(&self.conns_area, m.column, m.row);
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => self.on_mouse_main_down(&m),
            MouseEventKind::Up(MouseButton::Left) => self.on_mouse_main_up(&m),
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                self.on_mouse_main_scroll(&m, over_conns);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.sb_arrow_follow(m.column, m.row);
                self.sb_drag_move(m.column, m.row);
            }
            _ => {}
        }
    }

    fn on_mouse_main_down(&mut self, m: &MouseEvent) {
        if self.main_down_footer(m) {
            return;
        }
        if self.main_down_tab(m) {
            return;
        }
        if self.main_down_detail_hits(m) {
            return;
        }
        if self.sb_press_at(m.column, m.row) {
            return;
        }
        if self.main_down_conns(m) {
            return;
        }
        self.main_down_list(m);
    }

    fn main_down_footer(&mut self, m: &MouseEvent) -> bool {
        if let Some(i) = rect_hit(&self.footer_cmd_rects, m.column, m.row) {
            self.footer_press = Some(i);
            self.footer_flash = Some((i, Instant::now() + std::time::Duration::from_millis(400)));
            return true;
        }
        self.footer_press = None;
        false
    }

    fn main_down_tab(&mut self, m: &MouseEvent) -> bool {
        let tab_rects = self.tab_rects.clone();
        if let Some(i) = rect_hit(&tab_rects, m.column, m.row) {
            if i != self.filter {
                self.filter = i;
                self.scroll_to_selection();
            }
            self.focus = Panel::List;
            return true;
        }
        false
    }

    fn main_down_detail_hits(&mut self, m: &MouseEvent) -> bool {
        if pointer_in(&self.detail_url_rect, m.column, m.row) {
            let v = self.sel_task().map(|t| t.url.clone());
            self.copy_detail_value(v, "已复制 url");
            return true;
        }
        if pointer_in(&self.detail_ck_rect, m.column, m.row) {
            let v = self
                .sel_task()
                .and_then(|t| t.checksum.as_ref().map(|c| c.value.clone()));
            self.copy_detail_value(v, "已复制 校验码");
            return true;
        }
        if pointer_in(&self.detail_fail_rect, m.column, m.row) {
            let v = self
                .sel_task()
                .map(|t| t.error.clone().unwrap_or_else(|| "未知错误".to_string()));
            self.copy_detail_value(v, "已复制 失败原因");
            return true;
        }
        if pointer_in(&self.detail_save_rect, m.column, m.row) {
            let v = self.sel_task().map(|t| t.target_path());
            self.copy_detail_value(v, "已复制 保存路径");
            return true;
        }
        if let Some(field) = rect_hit(&self.detail_val_rects, m.column, m.row) {
            self.open_val_popup(field);
            return true;
        }
        if pointer_in(&self.detail_area, m.column, m.row) {
            self.focus = Panel::Detail;
            return true;
        }
        false
    }

    fn copy_detail_value(&mut self, v: Option<String>, toast: &str) {
        self.focus = Panel::Detail;
        if let Some(v) = v {
            self.copy_to_clipboard(&v);
            self.set_toast(toast);
        }
    }

    fn main_down_conns(&mut self, m: &MouseEvent) -> bool {
        if let Some(a) = self.conns_area {
            let in_x = m.column >= a.x && m.column < a.x.saturating_add(a.width);
            let in_y = m.row >= a.y && m.row < a.y.saturating_add(a.height);
            if in_x && in_y {
                self.focus = Panel::Conns;
                let data_top = a.y + 1;
                if m.row >= data_top {
                    let idx = (m.row - data_top) as usize;
                    if idx < self.visible_conns_rows {
                        let n = self.conn_count();
                        let ci = self.conns_scroll + idx;
                        if ci < n {
                            self.conns_sel = ci;
                        }
                    }
                }
                return true;
            }
        }
        false
    }

    fn main_down_list(&mut self, m: &MouseEvent) {
        if let Some(a) = self.list_area {
            let in_x = m.column >= a.x && m.column < a.x.saturating_add(a.width);
            let in_y = m.row >= a.y && m.row < a.y.saturating_add(a.height);
            if in_x && in_y {
                self.focus = Panel::List;
                let idx = (m.row - a.y) as usize / ITEM_HEIGHT as usize;
                let ti = self.scroll + idx;
                if ti < self.filtered().len() {
                    self.selected = ti;
                }
            }
        }
    }

    fn on_mouse_main_up(&mut self, m: &MouseEvent) {
        self.sb_release();
        if let Some(pressed) = self.footer_press.take() {
            let still_on = self.footer_cmd_rects.iter().any(|(r, i)| {
                *i == pressed
                    && m.column >= r.x
                    && m.column < r.x.saturating_add(r.width)
                    && m.row >= r.y
                    && m.row < r.y.saturating_add(r.height)
            });
            if still_on {
                self.run_footer_cmd(pressed);
            }
        }
    }

    fn on_mouse_main_scroll(&mut self, m: &MouseEvent, over_conns: bool) {
        match m.kind {
            MouseEventKind::ScrollUp if over_conns => {
                self.conns_scroll = self.conns_scroll.saturating_sub(2);
            }
            MouseEventKind::ScrollDown if over_conns => {
                let vis = self.visible_conns_rows.max(1);
                let maxlen = self.conn_count();
                let maxs = maxlen.saturating_sub(vis);
                self.conns_scroll = (self.conns_scroll + 2).min(maxs);
            }
            MouseEventKind::ScrollUp => {
                self.scroll = self.scroll.saturating_sub(2);
            }
            _ => {
                let vis = self.visible_rows.max(1);
                let maxs = self.filtered().len().saturating_sub(vis);
                self.scroll = (self.scroll + 2).min(maxs);
            }
        }
    }
}

fn pointer_in(area: &Option<Rect>, x: u16, y: u16) -> bool {
    area.is_some_and(|a| {
        x >= a.x && x < a.x.saturating_add(a.width) && y >= a.y && y < a.y.saturating_add(a.height)
    })
}

fn rect_hit<T: Copy>(rects: &[(Rect, T)], x: u16, y: u16) -> Option<T> {
    rects
        .iter()
        .find(|(r, _)| {
            x >= r.x
                && x < r.x.saturating_add(r.width)
                && y >= r.y
                && y < r.y.saturating_add(r.height)
        })
        .map(|(_, v)| *v)
}

#[cfg(test)]
mod pieces_mouse_tests {

    use crossterm::event::KeyModifiers;
    use ratatui::layout::Rect;

    use super::*;
    use crate::app::testutil::dl_task_mb;

    fn make_app(tag: &str) -> crate::app::App {
        crate::app::testutil::make_app_in("ezr-pmouse", tag)
    }

    fn dl_task(id: u32, name: &str) -> crate::model::Task {
        dl_task_mb(id, name)
    }

    fn popup_area() -> Rect {
        Rect {
            x: 10,
            y: 5,
            width: 64,
            height: 21,
        }
    }

    fn mev(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[tokio::test]
    async fn wheel_scrolls_grid_not_task_list() {
        let mut app = make_app("wheel");
        app.tasks.push(dl_task(1, "p.bin"));
        app.selected = 0;
        app.open_pieces();
        app.pieces_area = Some(popup_area());
        app.on_mouse(mev(MouseEventKind::ScrollDown, 30, 10));
        assert_eq!(app.pieces_scroll, 2, "滚轮下滚 2 行（FR-01-106 ⑦）");
        app.on_mouse(mev(MouseEventKind::ScrollDown, 30, 10));
        assert_eq!(app.pieces_scroll, 4);
        app.on_mouse(mev(MouseEventKind::ScrollUp, 30, 10));
        assert_eq!(app.pieces_scroll, 2, "滚轮上滚 2 行");
        app.on_mouse(mev(MouseEventKind::ScrollUp, 30, 10));
        app.on_mouse(mev(MouseEventKind::ScrollUp, 30, 10));
        assert_eq!(app.pieces_scroll, 0, "上滚钳制 0");
        assert_eq!(app.scroll, 0, "任务列表不滚动");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn drag_inside_moves_window_up_ends_drag() {
        let mut app = make_app("drag");
        app.tasks.push(dl_task(1, "p.bin"));
        app.selected = 0;
        app.open_pieces();
        app.pieces_pos = Some((10, 5));
        app.pieces_area = Some(popup_area());
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 20, 8));
        assert_eq!(app.pieces_drag, Some((10, 3)), "记抓取偏移");
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 18, 6));
        assert_eq!(app.pieces_pos, Some((8, 3)), "拖拽随动");
        assert!(app.pieces_open, "拖拽不关闭弹窗");
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 18, 6));
        assert_eq!(app.pieces_drag, None);
        assert!(app.pieces_open);
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 40, 20));
        assert_eq!(app.pieces_pos, Some((8, 3)), "无抓取偏移不随动");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn click_outside_is_noop_without_task_selection() {
        let mut app = make_app("out");
        app.tasks.push(dl_task(1, "p.bin"));
        app.tasks.push(dl_task(2, "q.bin"));
        app.selected = 0;
        app.open_pieces();
        app.pieces_area = Some(popup_area());
        app.list_area = Some(Rect {
            x: 0,
            y: 5,
            width: 40,
            height: 9,
        });
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 9));
        assert!(
            app.pieces_open,
            "点外不关闭弹窗（v1.32 修订，原「点外关闭」退役）"
        );
        assert_eq!(app.selected, 0, "该次点击不选中任何任务");
        assert_eq!(app.pieces_drag, None, "窗外按下不起拖拽");
        app.on_mouse(mev(MouseEventKind::ScrollDown, 3, 9));
        assert_eq!(app.pieces_scroll, 2, "滚轮被弹窗分支消费");
        assert_eq!(app.scroll, 0, "任务列表滚动不动");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn pieces_branch_consumes_all_events() {
        let mut app = make_app("consume");
        app.tasks.push(dl_task(1, "p.bin"));
        app.selected = 0;
        app.open_pieces();
        app.pieces_area = Some(popup_area());
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Right), 30, 10));
        assert!(app.pieces_open);
        assert_eq!(app.pieces_drag, None, "右键不开始拖拽");
        app.on_mouse(mev(MouseEventKind::Moved, 30, 10));
        assert!(app.pieces_open);
        let _ = KeyModifiers::NONE;
        app.shutdown().await;
    }
}

#[cfg(test)]
mod mouse_tests {
    use crossterm::event::KeyModifiers;

    use super::*;
    use crate::app::{DetailField, FieldWin};

    fn mev(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn make_app(tag: &str) -> crate::app::App {
        crate::app::testutil::make_app_in("ezr-mouse", tag)
    }

    fn task(id: u32, name: &str) -> crate::model::Task {
        crate::model::Task::new_queued(
            id,
            name.to_string(),
            crate::model::Protocol::Http,
            format!("http://example.com/{name}"),
            "/tmp".to_string(),
            1024 * 1024,
            4,
            5,
            None,
            crate::model::config::ProxyChoice::Direct,
            0,
        )
    }

    fn dlg(kind: DialogKind, focus: usize) -> super::super::Dialog {
        crate::app::testutil::dialog(kind, focus)
    }

    #[test]
    fn click_cursor_offset_maps_columns_to_char_indices() {
        let narrow = FieldWin {
            value_x: 35,
            left_cut: false,
            cols: vec![0, 1, 2, 3],
            win_lo: 0,
            win_hi: 3,
        };
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
        let wide = FieldWin {
            value_x: 35,
            left_cut: false,
            cols: vec![0, 2, 4],
            win_lo: 0,
            win_hi: 2,
        };
        assert_eq!(TextInput::click_offset(&wide, 35), 0, "宽字左半格 → 其前");
        assert_eq!(TextInput::click_offset(&wide, 36), 1, "宽字右半格 → 其后");
        assert_eq!(
            TextInput::click_offset(&wide, 37),
            1,
            "第 2 宽字左半格 → 其前"
        );
        let cut = FieldWin {
            value_x: 35,
            left_cut: true,
            cols: vec![0, 1, 2, 3],
            win_lo: 7,
            win_hi: 10,
        };
        assert_eq!(TextInput::click_offset(&cut, 35), 7, "… 指示列 → 窗口首");
        assert_eq!(TextInput::click_offset(&cut, 36), 7, "窗口首字符列");
        assert_eq!(
            TextInput::click_offset(&cut, 37),
            8,
            "第 2 可见字符列 → 7+1"
        );
        let empty = FieldWin {
            value_x: 35,
            left_cut: false,
            cols: vec![0],
            win_lo: 0,
            win_hi: 0,
        };
        assert_eq!(TextInput::click_offset(&empty, 35), 0, "空字段 → 0");
        assert_eq!(TextInput::click_offset(&empty, 50), 0, "空字段任意列 → 0");
    }

    #[tokio::test]
    async fn dialog_field_click_positions_cursor() {
        let mut app = make_app("dlgcur");
        app.dialog = Some(dlg(DialogKind::Add, 2));
        {
            let d = app.dialog.as_mut().unwrap();
            d.url.set("https://example.com/path");
            d.conns.set("4");
            d.focus_field(2);
        }
        let win_url = FieldWin {
            value_x: 35,
            left_cut: false,
            cols: (0..=24).collect(),
            win_lo: 0,
            win_hi: 24,
        };
        let win_conns = FieldWin {
            value_x: 35,
            left_cut: false,
            cols: vec![0, 1],
            win_lo: 0,
            win_hi: 1,
        };
        app.dlg_field_wins = vec![Some(win_url), None, Some(win_conns), None, None, None];
        app.dlg_field_rects = vec![
            (
                Rect {
                    x: 26,
                    y: 16,
                    width: 68,
                    height: 1,
                },
                0,
            ),
            (
                Rect {
                    x: 26,
                    y: 18,
                    width: 68,
                    height: 1,
                },
                2,
            ),
        ];

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 40, 16));
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.focus, 0, "点击聚焦 URL 行");
        assert_eq!(d.url.cursor, 5, "光标落点击列对应字符偏移（: 前）");

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 90, 16));
        assert_eq!(app.dialog.as_ref().unwrap().url.cursor, 24, "越末 → 字段尾");

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 35, 18));
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.focus, 2, "点击聚焦并发行");
        assert_eq!(d.conns.cursor, 0, "点击处映射覆盖迁尾（值首列 = 0）");
    }

    #[tokio::test]
    async fn detail_label_click_copies_and_toasts() {
        let mut app = make_app("copy");
        let mut t = task(1, "copy.bin");
        t.checksum = Some(crate::model::Checksum {
            algo: "SHA-256",
            value: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
        });
        app.tasks.push(t);
        app.detail_url_rect = Some(Rect {
            x: 1,
            y: 7,
            width: 9,
            height: 1,
        });
        app.detail_ck_rect = Some(Rect {
            x: 1,
            y: 5,
            width: 9,
            height: 1,
        });

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 7));
        assert_eq!(app.toast.as_deref(), Some("已复制 url"), "URL 点击 toast");
        assert_eq!(
            app.last_copied.as_deref(),
            Some("http://example.com/copy.bin"),
            "复制原始 URL 展示值（D27 v1.28 单值收敛）"
        );

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 5));
        assert_eq!(
            app.toast.as_deref(),
            Some("已复制 校验码"),
            "校验点击 toast"
        );
        assert_eq!(
            app.last_copied.as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
            "复制完整校验码"
        );

        app.tasks.push(task(2, "nock.bin"));
        app.selected = 1;
        app.detail_ck_rect = None;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 5));
        assert_eq!(
            app.last_copied.as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
            "无热区点击不改变复制内容"
        );
        assert_eq!(app.toast.as_deref(), Some("已复制 校验码"), "toast 不变");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn detail_url_click_copies_original_url_single_source() {
        let mut app = make_app("copyorig");
        app.tasks.push(task(1, "orig.bin"));
        app.detail_url_rect = Some(Rect {
            x: 1,
            y: 7,
            width: 9,
            height: 1,
        });
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 7));
        assert_eq!(app.toast.as_deref(), Some("已复制 url"), "URL 点击 toast");
        assert_eq!(
            app.last_copied.as_deref(),
            Some("http://example.com/orig.bin"),
            "复制原始 URL（v1.28 单值收敛，final_url 优先臂退役）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn detail_click_guards() {
        let mut app = make_app("copyg");
        app.tasks.push(task(1, "g.bin"));
        app.detail_url_rect = Some(Rect {
            x: 1,
            y: 7,
            width: 9,
            height: 1,
        });

        app.detail_url_rect = None;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 7));
        assert!(app.last_copied.is_none(), "热区 None 点击无复制");

        app.detail_url_rect = Some(Rect {
            x: 1,
            y: 7,
            width: 9,
            height: 1,
        });
        app.dialog = Some(dlg(DialogKind::Add, 0));
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 7));
        assert!(app.last_copied.is_none(), "对话框内点击不触发复制");
        assert!(app.dialog.is_some(), "对话框不被详情点击关闭");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn click_selects_and_scroll_bounded() {
        let mut app = make_app("list");
        app.list_area = Some(Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 9,
        });
        app.tasks.push(task(1, "a.bin"));
        app.tasks.push(task(2, "b.bin"));
        app.visible_rows = 6;

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 1));
        assert_eq!(app.selected, 0, "点击首条选中");
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 5));
        assert_eq!(app.selected, 1, "点击次条选中");
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 0));
        assert_eq!(app.selected, 1, "列表外点击不选中");
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 50, 5));
        assert_eq!(app.selected, 1, "列外点击不选中");

        app.on_mouse(mev(MouseEventKind::ScrollDown, 5, 5));
        assert_eq!(app.scroll, 0, "无可滚动余量");
        app.on_mouse(mev(MouseEventKind::ScrollUp, 5, 5));
        assert_eq!(app.scroll, 0, "顶部上滚钳制");

        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 5, 5));
        assert_eq!(app.selected, 1, "拖拽事件忽略");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn dialog_click_routes() {
        let mut app = make_app("dlgclick");
        app.dialog = Some(dlg(DialogKind::Add, 0));
        app.dlg_field_rects = vec![
            (
                Rect {
                    x: 2,
                    y: 2,
                    width: 30,
                    height: 1,
                },
                0,
            ),
            (
                Rect {
                    x: 2,
                    y: 5,
                    width: 30,
                    height: 1,
                },
                3,
            ),
            (
                Rect {
                    x: 2,
                    y: 7,
                    width: 30,
                    height: 1,
                },
                5,
            ),
        ];
        app.dlg_btn_rects = vec![
            (
                Rect {
                    x: 2,
                    y: 12,
                    width: 10,
                    height: 1,
                },
                6,
            ),
            (
                Rect {
                    x: 14,
                    y: 12,
                    width: 10,
                    height: 1,
                },
                7,
            ),
            (
                Rect {
                    x: 26,
                    y: 12,
                    width: 10,
                    height: 1,
                },
                8,
            ),
        ];

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 5));
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.focus, 3, "算法行点击聚焦");
        assert!(d.ck_open, "算法行点击展开下拉");

        app.dlg_ck_rects = vec![(
            Rect {
                x: 2,
                y: 6,
                width: 30,
                height: 1,
            },
            1,
        )];
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 6));
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.ck_type, 1, "点选算法项");
        assert_eq!(d.ck_sel, 1);
        assert!(!d.ck_open, "选择后收起");

        app.dialog.as_mut().unwrap().ck_open = true;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 20));
        let d = app.dialog.as_ref().unwrap();
        assert!(!d.ck_open, "点外收起");
        assert_eq!(d.ck_type, 1, "未改变选择");

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 7));
        assert!(app.dialog.as_ref().unwrap().proxy_open);
        app.dlg_proxy_rects = vec![(
            Rect {
                x: 2,
                y: 8,
                width: 30,
                height: 1,
            },
            0,
        )];
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 8));
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.proxy_sel, 0, "点选代理项");
        assert!(!d.proxy_open, "选择后收起");

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 2));
        assert_eq!(app.dialog.as_ref().unwrap().focus, 0, "URL 行点击聚焦");
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 20));
        assert_eq!(app.dialog.as_ref().unwrap().focus, 0, "空白点击无动作");

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 16, 12));
        assert!(app.dialog.is_some(), "仅添加钮（空 URL）被校验拒绝不关闭");
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 28, 12));
        assert!(app.dialog.is_none(), "取消按钮关闭");

        app.dialog = Some(dlg(DialogKind::Add, 0));
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Right), 5, 2));
        assert!(app.dialog.is_some(), "右键不触发路由");
        app.on_mouse(mev(MouseEventKind::ScrollUp, 5, 2));
        assert!(app.dialog.is_some(), "滚轮被对话框分支消费");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn dialog_click_modify_delete_buttons() {
        let mut app = make_app("mdbtn");

        app.dialog = Some(dlg(DialogKind::Modify, 4));
        app.dlg_btn_rects = vec![(
            Rect {
                x: 14,
                y: 12,
                width: 10,
                height: 1,
            },
            5,
        )];
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 16, 12));
        assert!(app.dialog.is_none(), "Modify 取消按钮关闭");

        app.dialog = Some(dlg(DialogKind::Modify, 4));
        app.dlg_btn_rects = vec![(
            Rect {
                x: 2,
                y: 12,
                width: 10,
                height: 1,
            },
            4,
        )];
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 4, 12));
        assert!(app.dialog.is_some(), "确认不生效（无任务上下文）");

        app.dialog = Some(dlg(DialogKind::Delete, 0));
        app.dlg_btn_rects = vec![(
            Rect {
                x: 20,
                y: 12,
                width: 10,
                height: 1,
            },
            2,
        )];
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 22, 12));
        assert!(app.dialog.is_none(), "Delete 取消按钮关闭");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn dialog_drag_moves_window() {
        let mut app = make_app("dlgdrag");
        app.open_add_dialog();
        app.dlg_area = Some(Rect {
            x: 3,
            y: 5,
            width: 74,
            height: 11,
        });
        app.dlg_field_rects = vec![(
            Rect {
                x: 4,
                y: 6,
                width: 30,
                height: 1,
            },
            0,
        )];
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 10, 6));
        assert_eq!(app.dlg_drag, None, "字段行按下不起拖");

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 10, 5));
        assert_eq!(app.dlg_drag, Some((7, 0)), "非交互区按下起拖");

        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 13, 3));
        assert_eq!(app.dlg_pos, Some((6, 3)), "拖拽随动窗口左上角");
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 20, 5));
        assert_eq!(app.dlg_pos, Some((13, 5)));

        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 20, 5));
        assert_eq!(app.dlg_drag, None, "松开结束拖拽");
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 30, 15));
        assert_eq!(app.dlg_pos, Some((13, 5)), "非拖拽态 Drag 不移动");

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 0, 0));
        assert_eq!(app.dlg_drag, None, "窗外按下无动作");
        assert!(app.dialog.is_some(), "对话框不因点外关闭");

        app.dialog = None;
        app.open_add_dialog();
        assert_eq!(app.dlg_pos, None, "再次打开重新居中");
        assert_eq!(app.dlg_drag, None);
        app.shutdown().await;
    }

    fn open_about_with_rects(app: &mut crate::app::App) {
        app.open_about();
        app.about_area = Some(Rect {
            x: 30,
            y: 8,
            width: 40,
            height: 7,
        });
        app.popup_close_rect = Some(Rect {
            x: 66,
            y: 8,
            width: 3,
            height: 1,
        });
    }

    #[tokio::test]
    async fn about_window_mouse_paths() {
        let mut app = make_app("abmouse");
        open_about_with_rects(&mut app);
        app.footer_press = Some(0);

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 35, 10));
        assert_eq!(app.about_drag, Some((5, 2)), "窗口内按下起拖");
        assert_eq!(app.footer_press, None, "模态接管清除页脚按压态");

        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 38, 12));
        assert_eq!(app.about_pos, Some((33, 10)), "拖拽随动窗口左上角");

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 67, 8));
        assert!(app.x_press, "[✗] 按下高亮");
        assert_eq!(app.about_drag, Some((5, 2)), "[✗] 按下不改拖拽态");

        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 67, 8));
        assert!(!app.about_open, "[✗] 原地松开关闭");
        assert!(!app.x_press, "松开复位按压态");

        open_about_with_rects(&mut app);
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 67, 8));
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 40, 20));
        assert!(!app.x_press, "拖出热区松开复位按压");
        assert!(app.about_open, "拖出热区松开不关闭");
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 2, 2));
        assert!(app.about_open, "点外不关闭（模态统一口径）");
        assert_eq!(app.about_drag, None, "窗外按下不起拖");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn val_popup_mouse_paths() {
        let mut app = make_app("valmouse");
        app.tasks.push(task(1, "val.bin"));
        app.selected = 0;
        app.open_val_popup(DetailField::Url);
        assert!(app.val_open, "有选中任务时 URL 字段弹窗打开");
        app.val_area = Some(Rect {
            x: 20,
            y: 6,
            width: 48,
            height: 12,
        });
        app.popup_close_rect = Some(Rect {
            x: 64,
            y: 6,
            width: 3,
            height: 1,
        });

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 30, 10));
        assert_eq!(app.val_drag, Some((10, 4)), "窗口内按下起拖");
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 25, 8));
        assert_eq!(app.val_pos, Some((15, 4)), "拖拽随动窗口左上角");

        app.on_mouse(mev(MouseEventKind::ScrollDown, 30, 10));
        app.on_mouse(mev(MouseEventKind::ScrollUp, 30, 10));
        assert_eq!(app.val_scroll, 0, "无内容滚动钳制 0");

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 65, 6));
        assert!(app.x_press, "[✗] 按下高亮");
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 65, 6));
        assert!(!app.val_open, "[✗] 原地松开关闭");
        assert_eq!(app.val_pos, None, "关闭位置即清");
        assert_eq!(app.val_drag, None);

        app.open_val_popup(DetailField::Url);
        app.val_area = Some(Rect {
            x: 20,
            y: 6,
            width: 48,
            height: 12,
        });
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 2, 2));
        assert!(app.val_open, "点外不关闭（模态统一口径）");
        assert_eq!(app.val_drag, None, "窗外按下不起拖");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn help_window_mouse_paths() {
        let mut app = make_app("helpmouse");
        app.help_open = true;
        app.help_area = Some(Rect {
            x: 25,
            y: 3,
            width: 70,
            height: 20,
        });
        app.popup_close_rect = Some(Rect {
            x: 89,
            y: 3,
            width: 3,
            height: 1,
        });

        app.on_mouse(mev(MouseEventKind::ScrollDown, 50, 10));
        assert_eq!(app.help_scroll, 1, "下滚 1 行");
        app.on_mouse(mev(MouseEventKind::ScrollUp, 50, 10));
        assert_eq!(app.help_scroll, 0, "上滚 1 行");
        app.on_mouse(mev(MouseEventKind::ScrollUp, 50, 10));
        assert_eq!(app.help_scroll, 0, "上滚钳制 0");

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 30, 8));
        assert_eq!(app.help_drag, Some((5, 5)), "窗口内按下起拖");
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 28, 6));
        assert_eq!(app.help_pos, Some((23, 1)), "拖拽随动窗口左上角");

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 90, 3));
        assert!(app.x_press, "[✗] 按下高亮");
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 90, 3));
        assert!(!app.help_open, "[✗] 原地松开关闭");

        app.help_open = true;
        app.popup_close_rect = Some(Rect {
            x: 89,
            y: 3,
            width: 3,
            height: 1,
        });
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 90, 3));
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 50, 10));
        assert!(!app.x_press, "拖出热区松开复位按压");
        assert!(app.help_open, "拖出热区松开不关闭");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn dialog_close_btn_cancel_paths() {
        let mut app = make_app("dlgx");
        app.dialog = Some(dlg(DialogKind::Add, 0));
        app.popup_close_rect = Some(Rect {
            x: 70,
            y: 5,
            width: 3,
            height: 1,
        });

        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 71, 5));
        assert!(app.x_press, "[✗] 按下高亮");
        assert!(app.dialog.is_some(), "按下不关闭");
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 71, 5));
        assert!(app.dialog.is_none(), "[✗] 原地松开取消对话框");
        assert!(!app.x_press, "松开复位按压态");

        app.dialog = Some(dlg(DialogKind::Add, 0));
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 71, 5));
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 30, 9));
        assert!(!app.x_press, "拖出热区复位按压");
        assert!(app.dialog.is_some(), "拖出热区松开不关闭");

        app.dialog = Some(dlg(DialogKind::Add, 3));
        app.dialog.as_mut().unwrap().ck_open = true;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 71, 5));
        assert!(!app.dialog.as_ref().unwrap().ck_open, "下拉先收起");
        assert!(!app.x_press, "守卫臂不进入 [✗] 按压");
        assert!(app.dialog.is_some());
        app.shutdown().await;
    }

    #[tokio::test]
    async fn main_down_dispatch_chain_priority() {
        let mut app = make_app("dispatch");
        app.tasks.push(task(1, "chain.bin"));
        app.selected = 0;
        app.footer_cmd_rects = vec![(
            Rect {
                x: 2,
                y: 2,
                width: 4,
                height: 1,
            },
            0,
        )];
        app.list_area = Some(Rect {
            x: 2,
            y: 2,
            width: 20,
            height: 10,
        });
        app.focus = Panel::Conns;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 3, 2));
        assert_eq!(app.footer_press, Some(0), "页脚命中：按压记录");
        assert!(app.footer_flash.is_some(), "页脚命中：400ms 余辉");
        assert_eq!(app.focus, Panel::Conns, "页脚命中不改变焦点");
        assert_eq!(app.selected, 0, "页脚命中不选中任务");
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 30, 5));
        assert_eq!(app.footer_press, None, "未命中页脚：按压清除");
        app.detail_url_rect = Some(Rect {
            x: 0,
            y: 0,
            width: 6,
            height: 1,
        });
        app.selected = 99;
        let toast_before = app.toast.clone();
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 2, 0));
        assert_eq!(app.focus, Panel::Detail, "字段名命中聚焦详情");
        assert!(app.last_copied.is_none(), "无值不复制");
        assert_eq!(app.toast, toast_before, "无值不 toast（启动 toast 不变）");
        app.tab_rects = vec![(
            Rect {
                x: 0,
                y: 0,
                width: 5,
                height: 1,
            },
            1,
        )];
        app.selected = 0;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 2, 0));
        assert_eq!(app.filter, 1, "页签切换（命中链先于详情热区）");
        assert_eq!(app.focus, Panel::List, "页签命中聚焦列表");
        app.tasks.push(task(2, "chain2.bin"));
        app.filter = 0;
        app.selected = 0;
        app.focus = Panel::Conns;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 6));
        assert_eq!(app.focus, Panel::List, "列表点击聚焦");
        assert_eq!(app.selected, 1, "列表点击按行选中任务");
        app.shutdown().await;
    }

    fn seed_conns_panel(app: &mut crate::app::App, visible: usize) {
        app.tasks.push(crate::app::testutil::task_with_conns(
            1,
            "a.bin",
            crate::model::TaskState::Downloading,
            3,
        ));
        app.selected = 0;
        app.conns_area = Some(Rect {
            x: 60,
            y: 10,
            width: 20,
            height: 8,
        });
        app.visible_conns_rows = visible;
    }

    #[tokio::test]
    async fn conns_click_focuses_and_selects_data_row() {
        let mut app = make_app("conns-sel");
        seed_conns_panel(&mut app, 8);
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 65, 10));
        assert_eq!(app.focus, Panel::Conns, "命中面板即聚焦");
        assert_eq!(app.conns_sel, 0, "表头行不选行");
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 65, 13));
        assert_eq!(app.conns_sel, 2, "数据行选中（ci = scroll + idx）");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn conns_click_clamps_selection_and_misses_outside() {
        let mut app = make_app("conns-clamp");
        app.focus = Panel::Detail;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 65, 12));
        assert_eq!(app.focus, Panel::Detail, "面板缺省点击不聚焦并发面板");
        seed_conns_panel(&mut app, 2);
        app.focus = Panel::List;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 80, 12));
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 65, 9));
        assert_eq!(app.focus, Panel::List, "矩形外点击不聚焦");
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 65, 13));
        assert_eq!(app.focus, Panel::Conns, "界内点击聚焦");
        assert_eq!(app.conns_sel, 0, "越可见行数钳制不选");
        app.conns_scroll = 2;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 65, 12));
        assert_eq!(app.conns_sel, 0, "越连接数钳制不选");
        app.conns_scroll = 0;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 65, 12));
        assert_eq!(app.conns_sel, 1, "回落界内正常选中");
        app.shutdown().await;
    }
}

#[cfg(test)]
mod bt_mouse_tests {

    use crossterm::event::{KeyModifiers, MouseButton};
    use ratatui::layout::Rect;

    use super::*;
    use crate::app::{testutil, BtConnRow};

    fn make_app(tag: &str) -> crate::app::App {
        testutil::make_app_in("ezr-btmouse", tag)
    }

    fn mev(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn cmd_area() -> Rect {
        Rect {
            x: 21,
            y: 9,
            width: 78,
            height: 16,
        }
    }

    #[tokio::test]
    async fn cmd_wheel_scrolls_text_area() {
        let mut app = make_app("wheel");
        app.open_cmd_window();
        app.cmd_area = Some(cmd_area());
        app.cmd.max_scroll = 5;
        app.on_mouse(mev(MouseEventKind::ScrollDown, 40, 12));
        assert_eq!(app.cmd.view, 1, "滚轮下滚 1 行（CmdText 步长 1）");
        app.on_mouse(mev(MouseEventKind::ScrollUp, 40, 12));
        assert_eq!(app.cmd.view, 0, "滚轮上滚钳制 0");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn cmd_close_via_x_and_drag_moves() {
        let mut app = make_app("x-drag");
        app.open_cmd_window();
        app.cmd_area = Some(cmd_area());
        app.cmd_pos = Some((21, 9));
        app.popup_close_rect = Some(Rect {
            x: 95,
            y: 9,
            width: 3,
            height: 1,
        });
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 96, 9));
        assert!(app.x_press, "[✗] 按下高亮");
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 96, 9));
        assert!(!app.cmd_open, "[✗] 原地松开关闭命令窗口");
        app.open_cmd_window();
        app.cmd_area = Some(cmd_area());
        app.cmd_pos = Some((21, 9));
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 30, 12));
        assert_eq!(app.cmd_drag, Some((9, 3)), "记抓取偏移");
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 28, 10));
        assert_eq!(app.cmd_pos, Some((19, 7)), "拖拽随动");
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 28, 10));
        assert_eq!(app.cmd_drag, None, "松开结束拖拽");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn cmd_click_ok_button_confirms() {
        let mut app = make_app("ok");
        app.open_cmd_window();
        app.cmd_area = Some(cmd_area());
        app.cmd_btn_rect = Some(Rect {
            x: 88,
            y: 22,
            width: 8,
            height: 1,
        });
        app.cmd.lines[0].set("http://example.com/a.bin");
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 30, 12));
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 90, 22));
        assert!(!app.cmd_open, "[确定] 确认后窗口关闭（cmd_confirm 收口）");
        assert!(
            app.dialog.is_some(),
            "http(s) 输入分流打开添加对话框: {:?}",
            app.dialog.as_ref().map(|d| d.kind)
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn err_window_confirm_and_drag() {
        let mut app = make_app("err");
        app.open_error_window("暂不支持该类型的链接");
        app.err_area = Some(Rect {
            x: 27,
            y: 13,
            width: 66,
            height: 7,
        });
        app.err_btn_rect = Some(Rect {
            x: 56,
            y: 17,
            width: 8,
            height: 1,
        });
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 60, 17));
        assert!(!app.err_open, "[确定] 关闭错误窗口");
        app.open_error_window("无法读取种子文件");
        app.err_area = Some(Rect {
            x: 27,
            y: 13,
            width: 66,
            height: 7,
        });
        app.err_pos = Some((27, 13));
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 40, 15));
        assert_eq!(app.err_drag, Some((13, 2)), "记抓取偏移");
        app.on_mouse(mev(MouseEventKind::Drag(MouseButton::Left), 38, 14));
        assert_eq!(app.err_pos, Some((25, 12)), "拖拽随动");
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 38, 14));
        assert!(app.err_open, "松开不关闭（仅 [确定]/Esc 通道）");
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 5));
        assert!(app.err_open, "点外不关闭不穿透");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_conns_click_selects_bt_row() {
        let mut app = make_app("btconns");
        let mut t = testutil::task_with_conns(1, "bt.bin", crate::model::TaskState::Downloading, 4);
        t.protocol = crate::model::Protocol::Bt;
        t.connections.clear();
        app.tasks.push(t);
        app.selected = 0;
        app.bt_conns.insert(
            1,
            vec![
                BtConnRow {
                    id: 1,
                    addr: "1.2.3.4:51400".to_string(),
                    down_speed: 0.0,
                    cum_down: 0,
                    up_speed: 0.0,
                    cum_up: 0,
                    client: "QB".to_string(),
                },
                BtConnRow {
                    id: 2,
                    addr: "5.6.7.8:51401".to_string(),
                    down_speed: 0.0,
                    cum_down: 0,
                    up_speed: 0.0,
                    cum_up: 0,
                    client: "TR".to_string(),
                },
            ],
        );
        app.conns_area = Some(Rect {
            x: 60,
            y: 10,
            width: 20,
            height: 8,
        });
        app.visible_conns_rows = 8;
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 65, 12));
        assert_eq!(app.focus, Panel::Conns, "点击聚焦并发面板");
        assert_eq!(app.conns_sel, 1, "BT 行选择集 = bt_conns 行序");
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 65, 14));
        assert_eq!(app.conns_sel, 1, "越行数钳制不选");
        app.shutdown().await;
    }
}
