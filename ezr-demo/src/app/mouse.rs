use std::time::{Duration, Instant};

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::textfield::TextInput;

use super::*;

impl App {
    pub fn on_mouse(&mut self, m: MouseEvent) {
        self.pointer = Some((m.column, m.row));

        if self.about_open {
            match m.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    self.footer_press = None;

                    if Self::pointer_in(&self.popup_close_rect, m.column, m.row) {
                        self.x_press = true;
                        return;
                    }
                    if Self::pointer_in(&self.about_area, m.column, m.row) {
                        if let Some(r) = self.about_area {
                            self.about_drag =
                                Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                        }
                    }
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    if self.x_press {
                        self.x_press = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                    } else if let Some((dx, dy)) = self.about_drag {
                        self.about_pos =
                            Some((m.column.saturating_sub(dx), m.row.saturating_sub(dy)));
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    if self.x_press {
                        let hit = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        self.x_press = false;
                        self.sb_release();
                        if hit {
                            self.about_open = false;
                        }
                        return;
                    }
                    self.about_drag = None;
                    self.sb_release();
                }
                _ => {}
            }
            return;
        }

        if self.val_open {
            match m.kind {
                MouseEventKind::ScrollUp => self.val_scroll_by(-1),
                MouseEventKind::ScrollDown => self.val_scroll_by(1),
                MouseEventKind::Down(MouseButton::Left) => {
                    self.footer_press = None;

                    if Self::pointer_in(&self.popup_close_rect, m.column, m.row) {
                        self.x_press = true;
                        return;
                    }

                    if let Some(bar) = self.val_sb {
                        if let Some(hit) = bar.hit(m.column, m.row) {
                            self.sb_press(SbKey::Val, &bar, hit);
                            return;
                        }
                    }
                    if Self::pointer_in(&self.val_area, m.column, m.row) {
                        if let Some(r) = self.val_area {
                            self.val_drag =
                                Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                        }
                    }
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    if self.x_press {
                        self.x_press = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        return;
                    }

                    self.sb_arrow_follow(m);
                    if self.sb_drag.is_some() {
                        self.sb_drag_move(m);
                    } else if let Some((dx, dy)) = self.val_drag {
                        self.val_pos =
                            Some((m.column.saturating_sub(dx), m.row.saturating_sub(dy)));
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    if self.x_press {
                        let hit = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        self.x_press = false;
                        self.sb_release();
                        if hit {
                            self.close_val_popup();
                        }
                        return;
                    }
                    self.val_drag = None;
                    self.sb_release();
                }
                _ => {}
            }
            return;
        }

        if self.help_open {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    self.help_scroll = self.help_scroll.saturating_sub(1);
                }
                MouseEventKind::ScrollDown => self.help_scroll += 1,
                MouseEventKind::Down(MouseButton::Left) => {
                    self.footer_press = None;

                    if Self::pointer_in(&self.popup_close_rect, m.column, m.row) {
                        self.x_press = true;
                        return;
                    }

                    if let Some(bar) = self.help_sb {
                        if let Some(hit) = bar.hit(m.column, m.row) {
                            self.sb_press(SbKey::Help, &bar, hit);
                            return;
                        }
                    }
                    if Self::pointer_in(&self.help_area, m.column, m.row) {
                        if let Some(r) = self.help_area {
                            self.help_drag =
                                Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                        }
                    }
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    if self.x_press {
                        self.x_press = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        return;
                    }

                    self.sb_arrow_follow(m);
                    if self.sb_drag.is_some() {
                        self.sb_drag_move(m);
                    } else if let Some((dx, dy)) = self.help_drag {
                        self.help_pos =
                            Some((m.column.saturating_sub(dx), m.row.saturating_sub(dy)));
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    if self.x_press {
                        let hit = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        self.x_press = false;
                        self.sb_release();
                        if hit {
                            self.help_open = false;
                        }
                        return;
                    }
                    self.help_drag = None;
                    self.sb_release();
                }
                _ => {}
            }
            return;
        }

        if self.pieces_open {
            match m.kind {
                MouseEventKind::ScrollUp => self.pieces_scroll_by(-2),
                MouseEventKind::ScrollDown => self.pieces_scroll_by(2),
                MouseEventKind::Down(MouseButton::Left) => {
                    self.footer_press = None;

                    if Self::pointer_in(&self.popup_close_rect, m.column, m.row) {
                        self.x_press = true;
                        return;
                    }

                    if let Some(bar) = self.pieces_sb {
                        if let Some(hit) = bar.hit(m.column, m.row) {
                            self.sb_press(SbKey::Pieces, &bar, hit);
                            return;
                        }
                    }
                    if Self::pointer_in(&self.pieces_area, m.column, m.row) {
                        if let Some(r) = self.pieces_area {
                            self.pieces_drag =
                                Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                        }
                    }
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    if self.x_press {
                        self.x_press = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        return;
                    }

                    self.sb_arrow_follow(m);
                    if self.sb_drag.is_some() {
                        self.sb_drag_move(m);
                    } else if let Some((dx, dy)) = self.pieces_drag {
                        self.pieces_pos =
                            Some((m.column.saturating_sub(dx), m.row.saturating_sub(dy)));
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    if self.x_press {
                        let hit = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        self.x_press = false;
                        self.sb_release();
                        if hit {
                            self.close_pieces();
                        }
                        return;
                    }
                    self.pieces_drag = None;
                    self.sb_release();
                }
                _ => {}
            }
            return;
        }

        if self.dialog.is_some() {
            match m.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let ck_open = self.dialog.as_ref().is_some_and(|d| d.ck_open);
                    let proxy_open = self.dialog.as_ref().is_some_and(|d| d.proxy_open);

                    self.footer_press = None;
                    if ck_open {
                        let hit = |rects: &[(Rect, usize)]| {
                            rects
                                .iter()
                                .find(|(r, _)| {
                                    m.column >= r.x
                                        && m.column < r.x.saturating_add(r.width)
                                        && m.row >= r.y
                                        && m.row < r.y.saturating_add(r.height)
                                })
                                .map(|(_, i)| *i)
                        };
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
                        let hit = |rects: &[(Rect, usize)]| {
                            rects
                                .iter()
                                .find(|(r, _)| {
                                    m.column >= r.x
                                        && m.column < r.x.saturating_add(r.width)
                                        && m.row >= r.y
                                        && m.row < r.y.saturating_add(r.height)
                                })
                                .map(|(_, i)| *i)
                        };
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

                    if Self::pointer_in(&self.popup_close_rect, m.column, m.row) {
                        self.x_press = true;
                        return;
                    }

                    if Self::pointer_in(&self.dialog_area, m.column, m.row) {
                        if let Some(r) = self.dialog_area {
                            self.dialog_drag =
                                Some((m.column.saturating_sub(r.x), m.row.saturating_sub(r.y)));
                            self.dialog_drag_moved = false;
                        }
                    }
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    if self.x_press {
                        self.x_press = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        return;
                    }

                    if let Some((dx, dy)) = self.dialog_drag {
                        let new = (m.column.saturating_sub(dx), m.row.saturating_sub(dy));
                        if self.dialog_pos != Some(new) {
                            self.dialog_pos = Some(new);
                            self.dialog_drag_moved = true;
                        }
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    if self.x_press {
                        let hit = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        self.x_press = false;
                        self.sb_release();
                        if hit {
                            self.dialog = None;
                        }
                        return;
                    }

                    self.sb_release();
                    if self.dialog_drag.is_some() {
                        let moved = self.dialog_drag_moved;
                        self.dialog_drag = None;
                        self.dialog_drag_moved = false;
                        if !moved {
                            self.dialog_click(m);
                        }
                    }
                }
                _ => {}
            }
            return;
        }
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let in_rect = |r: Rect| {
                    m.column >= r.x
                        && m.column < r.x.saturating_add(r.width)
                        && m.row >= r.y
                        && m.row < r.y.saturating_add(r.height)
                };

                let footer_hit = self
                    .footer_cmd_rects
                    .iter()
                    .find(|(r, _)| in_rect(*r))
                    .map(|(_, i)| *i);
                if let Some(i) = footer_hit {
                    self.footer_press = Some(i);
                    self.footer_flash = Some((i, Instant::now() + Duration::from_millis(400)));
                    return;
                }
                self.footer_press = None;
                let tab_hit = self
                    .tab_rects
                    .iter()
                    .find(|(r, _)| in_rect(*r))
                    .map(|(_, i)| *i);
                if let Some(i) = tab_hit {
                    if i != self.filter {
                        self.filter = i;
                        self.scroll_to_selection();
                    }

                    self.focus = Panel::List;
                    return;
                }

                if Self::pointer_in(&self.detail_url_rect, m.column, m.row) {
                    self.focus = Panel::Detail;
                    if let Some(t) = self.sel_task() {
                        let url = t.url.clone();
                        self.copy_to_clipboard(&url);
                        self.set_toast("已复制 url");
                    }
                    return;
                }
                if Self::pointer_in(&self.detail_ck_rect, m.column, m.row) {
                    self.focus = Panel::Detail;
                    if let Some(t) = self.sel_task() {
                        if let Some(ck) = &t.checksum {
                            let v = ck.value.clone();
                            self.copy_to_clipboard(&v);
                            self.set_toast("已复制 校验码");
                        }
                    }
                    return;
                }

                if Self::pointer_in(&self.detail_fail_rect, m.column, m.row) {
                    self.focus = Panel::Detail;
                    if let Some(t) = self.sel_task() {
                        let v = t.error.clone().unwrap_or_else(|| "未知错误".to_string());
                        self.copy_to_clipboard(&v);
                        self.set_toast("已复制 失败原因");
                    }
                    return;
                }

                if Self::pointer_in(&self.detail_save_rect, m.column, m.row) {
                    self.focus = Panel::Detail;
                    if let Some(t) = self.sel_task() {
                        let v = t.save_path.clone();
                        self.copy_to_clipboard(&v);
                        self.set_toast("已复制 保存路径");
                    }
                    return;
                }

                let val_hit = self
                    .detail_val_rects
                    .iter()
                    .find(|(r, _)| in_rect(*r))
                    .map(|(_, f)| *f);
                if let Some(field) = val_hit {
                    self.focus = Panel::Detail;
                    self.open_val_popup(field);
                    return;
                }

                if Self::pointer_in(&self.detail_area, m.column, m.row) {
                    self.focus = Panel::Detail;
                    return;
                }

                for (bar, key) in [
                    (self.list_sb, SbKey::List),
                    (self.conns_v_sb, SbKey::ConnsV),
                    (self.conns_h_sb, SbKey::ConnsH),
                ] {
                    let Some(bar) = bar else { continue };
                    if let Some(hit) = bar.hit(m.column, m.row) {
                        self.focus = match key {
                            SbKey::ConnsV | SbKey::ConnsH => Panel::Conns,
                            _ => Panel::List,
                        };
                        self.sb_press(key, &bar, hit);
                        return;
                    }
                }

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
                        return;
                    }
                }
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
            MouseEventKind::ScrollUp => {
                if Self::pointer_in(&self.conns_area, m.column, m.row) {
                    self.conns_scroll = self.conns_scroll.saturating_sub(2);
                } else {
                    self.scroll = self.scroll.saturating_sub(2);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.sb_arrow_follow(m);
                if self.sb_drag.is_some() {
                    self.sb_drag_move(m);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
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
            MouseEventKind::ScrollDown => {
                if Self::pointer_in(&self.conns_area, m.column, m.row) {
                    let vis = self.visible_conns_rows.max(1);
                    let maxs = self.conn_count().saturating_sub(vis);
                    self.conns_scroll = (self.conns_scroll + 2).min(maxs);
                } else {
                    let vis = self.visible_rows.max(1);
                    let maxs = self.filtered().len().saturating_sub(vis);
                    self.scroll = (self.scroll + 2).min(maxs);
                }
            }
            _ => {}
        }
    }

    fn pointer_in(area: &Option<Rect>, x: u16, y: u16) -> bool {
        matches!(area, Some(a) if x >= a.x && x < a.x.saturating_add(a.width) && y >= a.y && y < a.y.saturating_add(a.height))
    }
}
impl App {
    pub fn footer_chip_hot(&self, i: usize) -> bool {
        if self.footer_press == Some(i) {
            return true;
        }
        matches!(
            self.footer_flash,
            Some((fi, until)) if fi == i && Instant::now() < until
        )
    }

    fn run_footer_cmd(&mut self, i: usize) {
        match i {
            0 => self.open_add_dialog(),
            1 => self.open_delete_dialog(),
            2 => self.open_modify_dialog(),
            3 => self.quit = true,
            4 => {
                self.help_open = true;
                self.help_scroll = 0;
            }
            _ => {}
        }
    }

    fn dialog_click(&mut self, m: MouseEvent) {
        let kind = self.dialog.as_ref().map(|d| d.kind);
        let hit = |rects: &[(Rect, usize)]| {
            rects
                .iter()
                .find(|(r, _)| {
                    m.column >= r.x
                        && m.column < r.x.saturating_add(r.width)
                        && m.row >= r.y
                        && m.row < r.y.saturating_add(r.height)
                })
                .map(|(_, i)| *i)
        };
        let field_rects = self.dlg_field_rects.clone();
        let btn_rects = self.dlg_btn_rects.clone();
        if let Some(i) = hit(&field_rects) {
            let d = self.dialog.as_mut().unwrap();

            d.focus_field(i);

            if let Some(win) = self.dlg_field_wins.get(i).and_then(|w| w.as_ref()) {
                let off = TextInput::click_offset(win, m.column);
                if let Some(f) = d.text_field_at(i) {
                    d.field_mut(f).set_cursor(off);
                }
            }
            let ck_i = if kind == Some(DialogKind::Add) { 3 } else { 1 };
            let proxy_i = if kind == Some(DialogKind::Add) { 5 } else { 3 };
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
                None => {}
            }
        }
    }
}
