use crossterm::event::{KeyCode, KeyModifiers};

use super::*;

impl App {
    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) {
            if code == KeyCode::Char('c') {
                self.quit = true;
                return;
            }
            // 命令窗口确认键放行：Ctrl+J（\n 字节，普通终端的 Ctrl+Enter）与
            // kitty 协议的 Ctrl+Enter（CSI 13;5u → Enter+CONTROL）
            if self.cmd_open && (code == KeyCode::Char('j') || code == KeyCode::Enter) {
                self.on_cmd_key(code, mods);
            }
            return;
        }

        if self.about_open {
            match code {
                KeyCode::Esc
                | KeyCode::Char('o')
                | KeyCode::Char('O')
                | KeyCode::Char('q')
                | KeyCode::Char('Q') => self.close_about(),
                _ => {}
            }
            return;
        }

        if self.cmd_open {
            self.on_cmd_key(code, mods);
            return;
        }

        if self.val_open {
            match code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => self.close_val_popup(),
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let text = self.val_text.clone();
                    self.copy_to_clipboard(&text);
                    let label = self.val_field.copy_label();
                    self.set_toast(format!("已复制 {label}"));
                }
                KeyCode::Up => self.val_scroll_by(-1),
                KeyCode::Down => self.val_scroll_by(1),
                KeyCode::PageUp => self.val_scroll_by(-8),
                KeyCode::PageDown => self.val_scroll_by(8),
                KeyCode::Home => self.val_scroll = 0,
                KeyCode::End => self.val_scroll = usize::MAX,
                _ => {}
            }
            return;
        }

        if self.dialog.is_some() {
            self.on_dialog_key(code, mods);
            return;
        }

        if self.pieces_open {
            match code {
                KeyCode::Esc
                | KeyCode::Char('p')
                | KeyCode::Char('P')
                | KeyCode::Char('q')
                | KeyCode::Char('Q') => self.close_pieces(),
                KeyCode::Up => self.pieces_scroll_by(-1),
                KeyCode::Down => self.pieces_scroll_by(1),
                KeyCode::PageUp => self.pieces_scroll_by(-8),
                KeyCode::PageDown => self.pieces_scroll_by(8),
                KeyCode::Home => self.pieces_scroll = 0,
                KeyCode::End => self.pieces_scroll = usize::MAX,
                _ => {}
            }
            return;
        }

        if self.help_open {
            match code {
                KeyCode::Esc
                | KeyCode::Char('h')
                | KeyCode::Char('H')
                | KeyCode::Char('q')
                | KeyCode::Char('Q') => self.help_open = false,
                KeyCode::Char('o') | KeyCode::Char('O') => self.open_about(),
                KeyCode::Up => self.help_scroll = self.help_scroll.saturating_sub(1),
                KeyCode::Down => self.help_scroll += 1,
                KeyCode::PageUp => self.help_scroll = self.help_scroll.saturating_sub(8),
                KeyCode::PageDown => self.help_scroll += 8,
                KeyCode::Home => self.help_scroll = 0,
                KeyCode::End => self.help_scroll = usize::MAX,
                _ => {}
            }
            return;
        }
        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.quit = true,

            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::BackTab => self.focus = self.focus.prev(),

            KeyCode::Char('t') | KeyCode::Char('T') => self.cycle_filter(1),

            KeyCode::Char(']') | KeyCode::Char('}') => self.cycle_filter(1),
            KeyCode::Char('[') | KeyCode::Char('{') => self.cycle_filter(-1),

            KeyCode::Char('h') | KeyCode::Char('H') => {
                self.help_open = true;
                self.help_scroll = 0;
            }

            KeyCode::Char('o') | KeyCode::Char('O') => self.open_about(),

            KeyCode::Down => match self.focus {
                Panel::List => self.move_sel(1),
                Panel::Conns => self.move_conn_sel(1),
                Panel::Detail => {}
            },
            KeyCode::Up => match self.focus {
                Panel::List => self.move_sel(-1),
                Panel::Conns => self.move_conn_sel(-1),
                Panel::Detail => {}
            },

            KeyCode::Left => {
                if self.focus == Panel::Conns {
                    self.sb_scroll(SbKey::ConnsH, -SbKey::ConnsH.step());
                }
            }
            KeyCode::Right => {
                if self.focus == Panel::Conns {
                    self.sb_scroll(SbKey::ConnsH, SbKey::ConnsH.step());
                }
            }
            KeyCode::PageDown => match self.focus {
                Panel::Conns => self.move_conn_sel(self.visible_conns_rows.max(1) as i32),
                _ => self.move_sel(4),
            },
            KeyCode::PageUp => match self.focus {
                Panel::Conns => self.move_conn_sel(-(self.visible_conns_rows.max(1) as i32)),
                _ => self.move_sel(-4),
            },
            KeyCode::Home => match self.focus {
                Panel::Conns => {
                    let n = self.conn_count();
                    if n > 0 {
                        self.move_conn_sel(-(n as i32));
                    }
                }
                _ => self.selected = 0,
            },
            KeyCode::End => match self.focus {
                Panel::Conns => {
                    let n = self.conn_count();
                    if n > 0 {
                        self.move_conn_sel(n as i32);
                    }
                }
                _ => {
                    let flen = self.filtered().len();
                    if flen > 0 {
                        self.selected = flen - 1;
                    }
                }
            },

            KeyCode::Char('b') | KeyCode::Char('B') => self.disconnect_conn(),
            KeyCode::Char(' ') => self.toggle_pause(),
            KeyCode::Char('r') | KeyCode::Char('R') => self.retry(),

            KeyCode::Char('p') | KeyCode::Char('P') => self.open_pieces(),
            KeyCode::Char('a') | KeyCode::Char('A') => self.open_cmd_window(),
            KeyCode::Char('m') | KeyCode::Char('M') => self.open_modify_dialog(),
            KeyCode::Char('d') | KeyCode::Char('D') => self.open_delete_dialog(),
            KeyCode::Char('c') | KeyCode::Char('C') => self.clear_completed(),

            KeyCode::Char('g') | KeyCode::Char('G') => {
                self.show_panes = !self.show_panes;
                if !self.show_panes {
                    self.focus = Panel::List;
                }
            }
            KeyCode::Char('u') | KeyCode::Char('U') => self.move_task(-1),
            KeyCode::Char('j') | KeyCode::Char('J') => self.move_task(1),
            _ => {}
        }
    }
}
impl App {
    /// 对话框按键分发（mods 需透传：trackers 输入的 Alt+Enter / kitty 协议
    /// Shift+Enter 换行依赖修饰键区分 Enter 语义）。
    pub fn on_dialog_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        let (kind, focus) = match &self.dialog {
            Some(d) => (d.kind, d.focus),
            None => return,
        };
        let nfocus = match kind {
            DialogKind::Add => 9,
            DialogKind::Modify => 6,
            DialogKind::Delete => 3,
            DialogKind::AddBt => 9,
            DialogKind::ModifyBt => 6,
        };

        let on_bt_list = matches!(
            (kind, focus),
            (DialogKind::AddBt, 2) | (DialogKind::ModifyBt, 0)
        );
        let trackers_focused = self
            .dialog
            .as_ref()
            .is_some_and(|d| d.trackers_focus() == Some(d.focus));

        if self.dialog.as_ref().is_some_and(|d| d.ck_open) {
            if let Some(d) = self.dialog.as_mut() {
                ck_dropdown_open_key(d, code);
            }
            return;
        }
        if self.dialog.as_ref().is_some_and(|d| d.proxy_open) {
            let n = self.proxy_options.len().max(1);
            if let Some(d) = self.dialog.as_mut() {
                proxy_dropdown_open_key(d, n, code);
            }
            return;
        }
        match code {
            KeyCode::Esc => self.dialog = None,

            KeyCode::Tab | KeyCode::BackTab => {
                if let Some(d) = self.dialog.as_mut() {
                    let step = if code == KeyCode::Tab { 1 } else { nfocus - 1 };
                    d.focus_field((d.focus + step) % nfocus);
                }
            }

            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                if trackers_focused {
                    // trackers 多行输入：↑↓ 可视行导航（列保持，短行落行尾），
                    // ←→ 行内移动，不跳焦点
                    if let Some(d) = self.dialog.as_mut() {
                        match code {
                            KeyCode::Up => d.trackers.up(),
                            KeyCode::Down => d.trackers.down(),
                            KeyCode::Left => d.trackers.left(),
                            KeyCode::Right => d.trackers.right(),
                            _ => {}
                        }
                    }
                } else if on_bt_list {
                    // 文件树焦点：↑↓ 移动光标，←→ 收拢/展开目录，不跳焦点
                    if let Some(d) = self.dialog.as_mut() {
                        match code {
                            KeyCode::Up => d.bt_list_move(-1),
                            KeyCode::Down => d.bt_list_move(1),
                            KeyCode::Left => d.bt_expand_cursor(false),
                            KeyCode::Right => d.bt_expand_cursor(true),
                            _ => {}
                        }
                    }
                } else {
                    let on_text = self
                        .dialog
                        .as_ref()
                        .is_some_and(|d| d.text_field_at(d.focus).is_some());
                    if let Some(d) = self.dialog.as_mut() {
                        if on_text {
                            let field = d.text_field_at(d.focus).unwrap();
                            match code {
                                KeyCode::Left => d.field_mut(field).left(),
                                KeyCode::Right => d.field_mut(field).right(),
                                KeyCode::Up => d.field_mut(field).cursor_home(),
                                KeyCode::Down => d.field_mut(field).cursor_end(),
                                _ => {}
                            }
                        } else {
                            let step = if code == KeyCode::Up || code == KeyCode::Left {
                                nfocus - 1
                            } else {
                                1
                            };
                            d.focus_field((d.focus + step) % nfocus);
                        }
                    }
                }
            }
            KeyCode::Enter => {
                // trackers 焦点：Alt+Enter（ESC 前缀，任何终端可区分）/
                // Shift+Enter（kitty 协议 CSI 13;2u）= 文本框内换行；
                // 裸 Enter = 确认（与按钮 click 同义，与整窗口径一致）
                if trackers_focused && mods.intersects(KeyModifiers::ALT | KeyModifiers::SHIFT) {
                    if !self.dialog.as_mut().unwrap().trackers.newline() {
                        self.set_toast("⚠ trackers 最多 64 行");
                    }
                } else {
                    match kind {
                        DialogKind::Add => self.add_dialog_enter(focus),
                        DialogKind::Modify => self.modify_dialog_enter(focus),
                        DialogKind::Delete => self.dlg_activate_delete(focus),
                        DialogKind::AddBt => self.add_bt_dialog_enter(focus),
                        DialogKind::ModifyBt => self.modify_bt_dialog_enter(focus),
                    }
                }
            }
            KeyCode::Home => {
                if trackers_focused {
                    self.dialog.as_mut().unwrap().trackers.to_first();
                }
            }
            KeyCode::End => {
                if trackers_focused {
                    self.dialog.as_mut().unwrap().trackers.to_last();
                }
            }
            KeyCode::Backspace => {
                if let Some(d) = self.dialog.as_mut() {
                    text_backspace(kind, d, focus);

                    if kind == DialogKind::Add && focus == 0 && !d.conns_edited {
                        let (p, _) = parse_url_task(d.url.value.trim());
                        d.conns.set(default_conns(p).to_string());
                    }
                }
            }
            KeyCode::Delete => {
                if trackers_focused {
                    self.dialog.as_mut().unwrap().trackers.delete();
                }
            }
            KeyCode::Char(c) => {
                if trackers_focused {
                    // 字符插入在 add/modify_bt_dialog_char 内处理（控制字符忽略）
                    match kind {
                        DialogKind::AddBt => self.add_bt_dialog_char(focus, c),
                        DialogKind::ModifyBt => self.modify_bt_dialog_char(focus, c),
                        _ => {}
                    }
                } else {
                    match kind {
                        DialogKind::Add => self.add_dialog_char(focus, c),
                        DialogKind::Modify => self.modify_dialog_char(focus, c),
                        DialogKind::Delete => match c {
                            '1' => self.dlg_activate_delete(0),
                            '2' => self.dlg_activate_delete(1),
                            '3' | ' ' => self.dlg_activate_delete(2),
                            _ => {}
                        },
                        DialogKind::AddBt => self.add_bt_dialog_char(focus, c),
                        DialogKind::ModifyBt => self.modify_bt_dialog_char(focus, c),
                    }
                }
            }
            _ => {}
        }
    }
}
impl App {
    pub fn on_paste(&mut self, text: &str) {
        if self.cmd_open {
            if self.cmd_focus == 0 {
                self.cmd_paste(text);
            }
            return;
        }
        let Some(d) = self.dialog.as_mut() else {
            return;
        };
        if !matches!(
            d.kind,
            DialogKind::Add | DialogKind::Modify | DialogKind::AddBt | DialogKind::ModifyBt
        ) || d.ck_open
            || d.proxy_open
        {
            return;
        }
        let (kind, focus) = (d.kind, d.focus);
        // trackers 多行输入：粘贴保留换行分段（\n 拆行、段内过滤控制字符），
        // 与命令窗口文本框同径；单行字段仍走 dlg_apply_paste（控制字符全剔除）
        if d.trackers_focus() == Some(focus) {
            d.trackers.insert_str(text);
            return;
        }
        dlg_apply_paste(kind, d, focus, text);
    }
}
