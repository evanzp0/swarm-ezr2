//! mouse — 鼠标事件：点击选中/滚轮/对话框按钮命中

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::{App, DialogKind, ITEM_HEIGHT};

impl App {
    /// 鼠标事件入口
    pub fn on_mouse(&mut self, m: MouseEvent) {
        if self.dialog.is_some() {
            if let MouseEventKind::Down(MouseButton::Left) = m.kind {
                let kind = self.dialog.as_ref().map(|d| d.kind);
                let ck_open = self.dialog.as_ref().is_some_and(|d| d.ck_open);
                let proxy_open = self.dialog.as_ref().is_some_and(|d| d.proxy_open);
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
                if ck_open {
                    let ck_rects = self.dlg_ck_rects.clone();
                    if let Some(i) = hit(&ck_rects) {
                        let d = self.dialog.as_mut().unwrap();
                        d.ck_type = i;
                        d.ck_sel = i;
                        d.ck_open = false;
                    } else if let Some(d) = self.dialog.as_mut() {
                        d.ck_open = false;
                    }
                    return;
                }
                // 代理下拉（v1.5/FR-01-86）：点击选项选择；点外关闭
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
                let field_rects = self.dlg_field_rects.clone();
                let btn_rects = self.dlg_btn_rects.clone();
                if let Some(i) = hit(&field_rects) {
                    // 下拉行下标：Add 校验=3 / Modify 校验=1；代理行恒为末二
                    //（Add 5 / Modify 3）
                    let ck_i = if kind == Some(DialogKind::Add) { 3 } else { 1 };
                    let proxy_i = if kind == Some(DialogKind::Add) { 5 } else { 3 };
                    let d = self.dialog.as_mut().unwrap();
                    d.focus = i;
                    if i == ck_i {
                        d.ck_open = true;
                        d.ck_sel = d.ck_type;
                    } else if i == proxy_i {
                        d.proxy_open = true;
                    }
                } else if let Some(btn) = hit(&btn_rects) {
                    if let Some(d) = self.dialog.as_mut() {
                        d.focus = btn;
                    }
                    match kind {
                        Some(DialogKind::Add) => self.dlg_activate_add(btn),
                        Some(DialogKind::Modify) => self.dlg_activate_modify(btn),
                        Some(DialogKind::Delete) => self.dlg_activate_delete(btn),
                        None => {}
                    }
                }
            }
            return;
        }
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(a) = self.list_area {
                    let in_x = m.column >= a.x && m.column < a.x.saturating_add(a.width);
                    let in_y = m.row >= a.y && m.row < a.y.saturating_add(a.height);
                    if in_x && in_y {
                        let idx = (m.row - a.y) as usize / ITEM_HEIGHT as usize;
                        let ti = self.scroll + idx;
                        if ti < self.filtered().len() {
                            self.selected = ti;
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                self.scroll = self.scroll.saturating_sub(2);
            }
            MouseEventKind::ScrollDown => {
                let vis = self.visible_rows.max(1);
                let maxs = self.filtered().len().saturating_sub(vis);
                self.scroll = (self.scroll + 2).min(maxs);
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // 任务操作（Space/R/D/C 语义沿用 demo + 真实引擎动作）
    // -----------------------------------------------------------------------
}
