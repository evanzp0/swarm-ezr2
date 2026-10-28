use crossterm::event::{KeyCode, KeyModifiers};

use super::{App, Panel, SbKey};
use crate::engine::Cmd;

impl App {
    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) {
            if code == KeyCode::Char('c') {
                self.quit = true;
                return;
            }
            if self.cmd_open && (code == KeyCode::Char('j') || code == KeyCode::Enter) {
                self.on_cmd_key(code, mods);
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
                    let label = self.val_field.copy_label();
                    let text = self.val_text.clone();
                    self.copy_to_clipboard(&text);
                    self.set_toast(format!("已复制 {label}"));
                }
                KeyCode::Up => self.sb_scroll(SbKey::Val, -1),
                KeyCode::Down => self.sb_scroll(SbKey::Val, 1),
                KeyCode::PageUp => self.sb_scroll(SbKey::Val, -8),
                KeyCode::PageDown => self.sb_scroll(SbKey::Val, 8),
                KeyCode::Home => self.sb_set(SbKey::Val, 0),
                KeyCode::End => self.sb_set(SbKey::Val, usize::MAX),
                _ => {}
            }
            return;
        }
        if self.err_open {
            self.on_err_key(code);
            return;
        }
        if self.dialog.is_some() {
            self.on_dialog_key(code, mods);
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
            KeyCode::Char('m') | KeyCode::Char('M') => self.modify_dialog_entry(),
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

    pub(crate) fn conn_count(&self) -> usize {
        if let Some(t) = self.sel_task().filter(|t| t.protocol.is_bt()) {
            return self.bt_conns.get(&t.id).map_or(0, Vec::len);
        }
        self.active_conn_ids().len()
    }

    pub(crate) fn move_conn_sel(&mut self, delta: i32) {
        let Some(idx) = self.sel_idx() else {
            return;
        };
        if !self.tasks[idx].state.shows_conns() {
            self.set_toast("当前状态无并发明细（仅下载中/做种中可选）");
            return;
        }
        let n = self.conn_count();
        if n == 0 {
            self.set_toast("当前任务没有并发连接明细");
            return;
        }
        let new = (self.conns_sel as i32 + delta).clamp(0, n as i32 - 1) as usize;
        self.conns_sel = new;
        let vis = self.visible_conns_rows.max(1);
        if new < self.conns_scroll {
            self.conns_scroll = new;
        } else if new >= self.conns_scroll + vis {
            self.conns_scroll = new + 1 - vis;
        }
    }

    pub(crate) fn disconnect_conn(&mut self) {
        let Some(idx) = self.sel_idx() else {
            return;
        };
        if !self.tasks[idx].state.shows_conns() {
            self.set_toast("当前状态无可断开的并发连接（仅下载中/做种中）");
            return;
        }
        if !self.tasks[idx].protocol.is_bt() {
            self.set_toast("仅 BT 任务支持断开并发连接（B）");
            return;
        }
        let id = self.tasks[idx].id;
        let rows = self.bt_conns.get(&id).cloned().unwrap_or_default();
        if self.conns_sel >= rows.len() {
            self.set_toast("当前任务没有可断开的并发连接");
            return;
        }
        self.conns_sel = self.conns_sel.min(rows.len() - 1);
        let conn = rows[self.conns_sel].id;
        let ip = rows[self.conns_sel]
            .addr
            .rsplit_once(':')
            .map_or(rows[self.conns_sel].addr.as_str(), |(ip, _)| ip);
        let left = rows.len() - 1;
        let engine = self.engine.clone();
        tokio::spawn(async move {
            engine.send(Cmd::BtDisconnect { id, conn }).await;
        });
        self.set_toast(format!("✂ 已断开连接 {ip}（剩余 {left} 条）"));
    }

    fn move_sel(&mut self, delta: i32) {
        let flen = self.filtered().len();
        if flen == 0 {
            return;
        }
        let cur = self.selected as i32;
        self.selected = cur.saturating_add(delta).clamp(0, flen as i32 - 1) as usize;
        self.scroll_to_selection();
    }

    pub(crate) fn cycle_filter(&mut self, delta: i32) {
        let n = super::FILTERS.len() as i32;
        self.filter = ((self.filter as i32 + delta).rem_euclid(n)) as usize;
        self.scroll_to_selection();
    }

    pub(crate) fn scroll_to_selection(&mut self) {
        let vis = self.visible_rows.max(1);
        if self.scroll > self.selected {
            self.scroll = self.selected;
        }
        if self.selected >= self.scroll + vis {
            self.scroll = self.selected + 1 - vis;
        }
    }

    pub fn move_task(&mut self, delta: i32) {
        let fl = self.filtered();
        if fl.is_empty() {
            return;
        }
        let new = self.selected as i32 + delta;
        if new < 0 {
            self.set_toast("已在顶部");
            return;
        }
        if new >= fl.len() as i32 {
            self.set_toast("已在底部");
            return;
        }
        let new = new as usize;
        let (a, b) = (fl[self.selected], fl[new]);
        if a != b {
            self.tasks.swap(a, b);
        }
        self.selected = new;
        let name = self.tasks[fl[new]].name.clone();
        self.set_toast(format!(
            "{} 任务: {}",
            if delta < 0 {
                "↑ 已上移"
            } else {
                "↓ 已下移"
            },
            name
        ));
    }
}

#[cfg(test)]
mod pieces_key_tests {

    use super::*;
    use crate::app::testutil::dl_task_mb;
    use crate::model::TaskState;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-pkey", tag)
    }

    fn dl_task(id: u32, name: &str) -> crate::model::Task {
        dl_task_mb(id, name)
    }

    #[tokio::test]
    async fn p_opens_pieces_popup() {
        let mut app = make_app("open");
        app.tasks.push(dl_task(1, "p.bin"));
        app.selected = 0;
        app.on_key(KeyCode::Char('p'), KeyModifiers::NONE);
        assert!(app.pieces_open, "P 打开分块弹窗");
        assert!(!app.quit);
        app.shutdown().await;
    }

    #[tokio::test]
    async fn modal_keys_consumed_q_closes_without_quit() {
        let mut app = make_app("modal");
        app.tasks.push(dl_task(1, "p.bin"));
        app.selected = 0;
        app.on_key(KeyCode::Char('p'), KeyModifiers::NONE);
        assert!(app.pieces_open);
        app.on_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "A 不打开添加对话框");
        app.on_key(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(app.tasks[0].state, TaskState::Downloading, "Space 不暂停");
        app.on_key(KeyCode::Char('d'), KeyModifiers::NONE);
        assert_eq!(app.tasks.len(), 1, "D 不打开删除对话框");
        app.on_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.filter, 0, "Tab 不切页签（页签键语义改版）");
        assert_eq!(
            app.focus,
            super::Panel::List,
            "Tab 不移动面板焦点（弹窗模态）"
        );
        app.on_key(KeyCode::Char(']'), KeyModifiers::NONE);
        assert_eq!(app.filter, 0, "] 不切页签（弹窗模态）");
        app.on_key(KeyCode::Char('g'), KeyModifiers::NONE);
        assert!(app.show_panes, "G 不收起面板");
        app.on_key(KeyCode::Char('h'), KeyModifiers::NONE);
        assert!(!app.help_open, "H 不打开帮助（pieces 模态优先）");
        app.on_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(!app.pieces_open, "q 关闭弹窗");
        assert!(!app.quit, "q 不退出应用");
        app.on_key(KeyCode::Char('p'), KeyModifiers::NONE);
        app.on_key(KeyCode::Char('Q'), KeyModifiers::NONE);
        assert!(!app.pieces_open && !app.quit, "Q 同口径");
        app.on_key(KeyCode::Char('p'), KeyModifiers::NONE);
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.pieces_open && !app.quit, "Esc 关闭不退出");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn modal_scroll_keys_step_line_and_page() {
        let mut app = make_app("pscroll");
        app.tasks.push(dl_task(1, "p.bin"));
        app.selected = 0;
        app.on_key(KeyCode::Char('p'), KeyModifiers::NONE);
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.pieces_scroll, 2, "↓ 逐行下滚");
        assert_eq!(app.selected, 0, "任务列表选择不动");
        app.on_key(KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(app.pieces_scroll, 10, "PgDn 按页 8 行");
        app.on_key(KeyCode::PageUp, KeyModifiers::NONE);
        assert_eq!(app.pieces_scroll, 2, "PgUp 按页 8 行");
        app.on_key(KeyCode::End, KeyModifiers::NONE);
        assert_eq!(app.pieces_scroll, usize::MAX, "End 交给渲染层钳制末页");
        app.on_key(KeyCode::Home, KeyModifiers::NONE);
        assert_eq!(app.pieces_scroll, 0, "Home 回首行");
        app.on_key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(app.pieces_scroll, 0, "↑ 顶部钳制");
        app.shutdown().await;
    }
}

#[cfg(test)]
mod help_key_tests {

    use super::*;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-hkey", tag)
    }

    #[tokio::test]
    async fn h_opens_and_closes_help() {
        let mut app = make_app("toggle");
        app.on_key(KeyCode::Char('h'), KeyModifiers::NONE);
        assert!(app.help_open, "H 打开帮助窗口");
        assert_eq!(app.help_scroll, 0, "打开时滚动复位首页");
        app.on_key(KeyCode::Char('h'), KeyModifiers::NONE);
        assert!(!app.help_open, "H 再按关闭");
        app.on_key(KeyCode::Char('H'), KeyModifiers::NONE);
        assert!(app.help_open);
        app.on_key(KeyCode::Char('H'), KeyModifiers::NONE);
        assert!(!app.help_open);
        app.shutdown().await;
    }

    #[tokio::test]
    async fn help_modal_consumes_main_keys() {
        let mut app = make_app("modal");
        app.on_key(KeyCode::Char('h'), KeyModifiers::NONE);
        app.on_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "A 不打开对话框");
        app.on_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(!app.quit && !app.help_open, "q 先关窗不退出");
        app.on_key(KeyCode::Char('h'), KeyModifiers::NONE);
        app.on_key(KeyCode::Char('Q'), KeyModifiers::NONE);
        assert!(!app.quit && !app.help_open, "Q 同口径");
        app.on_key(KeyCode::Char('h'), KeyModifiers::NONE);
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.quit && !app.help_open, "Esc 关闭不退出");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn help_scroll_keys_line_page_and_home_end() {
        let mut app = make_app("scroll");
        app.on_key(KeyCode::Char('h'), KeyModifiers::NONE);
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.help_scroll, 2, "↓ 逐行下滚");
        assert_eq!(app.selected, 0, "任务列表选择不动");
        app.on_key(KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(app.help_scroll, 10, "PgDn 按页 8 行");
        app.on_key(KeyCode::PageUp, KeyModifiers::NONE);
        assert_eq!(app.help_scroll, 2, "PgUp 按页 8 行");
        app.on_key(KeyCode::End, KeyModifiers::NONE);
        assert_eq!(app.help_scroll, usize::MAX, "End 交给渲染层钳制末页");
        app.on_key(KeyCode::Home, KeyModifiers::NONE);
        assert_eq!(app.help_scroll, 0, "Home 回首行");
        app.on_key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(app.help_scroll, 0, "↑ 顶部钳制");
        app.shutdown().await;
    }
}

#[cfg(test)]
mod conn_key_tests {

    use super::*;
    use crate::app::testutil::task_with_conns;
    use crate::model::TaskState;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-ck", tag)
    }

    #[tokio::test]
    async fn plain_updown_moves_when_conns_focused() {
        let mut app = make_app("sel");
        app.tasks
            .push(task_with_conns(1, "a.bin", TaskState::Downloading, 4));
        app.selected = 0;
        app.focus = Panel::Conns;
        for _ in 0..5 {
            app.on_key(KeyCode::Down, KeyModifiers::NONE);
        }
        assert_eq!(app.conns_sel, 3, "下移钳制到末行（4 条 → 3）");
        for _ in 0..10 {
            app.on_key(KeyCode::Up, KeyModifiers::NONE);
        }
        assert_eq!(app.conns_sel, 0, "上移钳制到首行");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn plain_down_follows_scroll() {
        let mut app = make_app("follow");
        app.tasks
            .push(task_with_conns(1, "b.bin", TaskState::Downloading, 4));
        app.selected = 0;
        app.focus = Panel::Conns;
        app.visible_conns_rows = 2;
        for _ in 0..3 {
            app.on_key(KeyCode::Down, KeyModifiers::NONE);
        }
        assert_eq!(app.conns_sel, 3);
        assert_eq!(app.conns_scroll, 2, "选中越出可视窗 → 跟随滚动");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn updown_dispatches_by_focus() {
        let mut app = make_app("dispatch");
        app.tasks
            .push(task_with_conns(1, "a.bin", TaskState::Downloading, 3));
        app.tasks
            .push(task_with_conns(2, "b.bin", TaskState::Downloading, 3));
        app.selected = 0;
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.selected, 1, "列表聚焦 ↓ 选任务");
        assert_eq!(app.conns_sel, 0, "明细选择不动");
        app.focus = Panel::Detail;
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.selected, 1, "详情聚焦 ↓ 无操作");
        assert_eq!(app.conns_sel, 0);
        app.shutdown().await;
    }

    #[tokio::test]
    async fn conns_updown_state_gate_toast() {
        let mut app = make_app("gate");
        app.tasks
            .push(task_with_conns(1, "p.bin", TaskState::Paused, 3));
        app.selected = 0;
        app.focus = Panel::Conns;
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.conns_sel, 0, "非下载态不移动选择");
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("当前状态无并发明细（仅下载中/做种中可选）")),
            "状态门槛 toast: {:?}",
            app.toast
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn b_key_http_rejects_with_single_key_toast() {
        let mut app = make_app("bhttp");
        app.tasks
            .push(task_with_conns(1, "h.bin", TaskState::Downloading, 3));
        app.selected = 0;
        app.focus = Panel::List;
        app.on_key(KeyCode::Char('b'), KeyModifiers::NONE);
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("仅 BT 任务支持断开并发连接（B）")),
            "HTTP 拒绝 toast（键名（B））: {:?}",
            app.toast
        );
        assert_eq!(app.tasks[0].connections.len(), 3, "明细行数不变");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn b_key_state_gate_precedes_protocol() {
        let mut app = make_app("bgate");
        app.tasks
            .push(task_with_conns(1, "g.bin", TaskState::Paused, 3));
        app.selected = 0;
        app.on_key(KeyCode::Char('b'), KeyModifiers::NONE);
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("当前状态无可断开的并发连接（仅下载中/做种中）")),
            "状态门槛 toast 先于协议 toast: {:?}",
            app.toast
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn ctrl_combos_retired_except_quit() {
        let mut app = make_app("ctrlret");
        app.tasks
            .push(task_with_conns(1, "c.bin", TaskState::Downloading, 3));
        app.selected = 0;
        app.on_key(KeyCode::Down, KeyModifiers::CONTROL);
        assert_eq!(app.conns_sel, 0, "Ctrl+↓ 退役：不作用于明细");
        app.on_key(KeyCode::Char('b'), KeyModifiers::CONTROL);
        assert!(
            !app.toast.as_deref().is_some_and(|m| m.contains("仅 BT")),
            "Ctrl+B 退役：不触发协议 toast"
        );
        app.on_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(app.quit, "Ctrl+C 退出优先级不变");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn esc_is_noop_on_main_view() {
        let mut app = make_app("esc");
        app.tasks
            .push(task_with_conns(1, "e.bin", TaskState::Downloading, 3));
        app.selected = 0;
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.quit, "Esc 主界面不退出");
        assert!(!app.help_open && !app.pieces_open && app.dialog.is_none());
        app.on_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(app.quit, "q 退出");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn selection_resets_when_task_switches() {
        let mut app = make_app("reset");
        app.tasks
            .push(task_with_conns(1, "t1.bin", TaskState::Downloading, 4));
        app.tasks
            .push(task_with_conns(2, "t2.bin", TaskState::Downloading, 4));
        app.selected = 0;
        app.focus = Panel::Conns;
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.conns_sel, 1);
        app.focus = Panel::List;
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.selected, 1, "列表选中切到下一任务");
        app.tick().await;
        assert_eq!(app.conns_sel, 0, "切换任务后明细选择复位（FR-01-97）");
        assert_eq!(app.conns_scroll, 0, "切换任务后明细滚动复位");
        app.shutdown().await;
    }
}

#[cfg(test)]
mod tab_key_tests {

    use super::*;
    use crate::model::TaskState;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-tkey", tag)
    }

    fn task(id: u32, name: &str, state: TaskState) -> crate::model::Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t.state = state;
        t
    }

    #[tokio::test]
    async fn tab_cycles_panel_focus_not_filter() {
        let mut app = make_app("panelfocus");
        app.tasks.push(task(1, "k1.bin", TaskState::Downloading));
        app.on_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.focus, Panel::Detail, "Tab → 详情");
        assert_eq!(app.filter, 0, "Tab 不切页签（v1.30 改版）");
        app.on_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.focus, Panel::Conns, "Tab → 并发");
        app.on_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.focus, Panel::List, "Tab 环回列表");
        app.on_key(KeyCode::BackTab, KeyModifiers::NONE);
        assert_eq!(app.focus, Panel::Conns, "BackTab 反向");
        app.on_key(KeyCode::BackTab, KeyModifiers::NONE);
        assert_eq!(app.focus, Panel::Detail);
        app.on_key(KeyCode::BackTab, KeyModifiers::NONE);
        assert_eq!(app.focus, Panel::List);
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bracket_keys_switch_tabs() {
        let mut app = make_app("brackets");
        app.tasks.push(task(1, "k1.bin", TaskState::Completed));
        app.on_key(KeyCode::Char(']'), KeyModifiers::NONE);
        assert_eq!(app.filter, 1, "] 到已完成页签");
        assert_eq!(app.focus, Panel::List, "页签键不动面板焦点");
        app.on_key(KeyCode::Char('}'), KeyModifiers::NONE);
        assert_eq!(app.filter, 0, "}} 环回");
        app.on_key(KeyCode::Char('['), KeyModifiers::NONE);
        assert_eq!(app.filter, 1, r#"[ 反向环绕"#);
        app.on_key(KeyCode::Char('{'), KeyModifiers::NONE);
        assert_eq!(app.filter, 0);
        app.on_key(KeyCode::Char('T'), KeyModifiers::NONE);
        assert_eq!(app.filter, 1, "T 循环切换");
        app.on_key(KeyCode::Char('t'), KeyModifiers::NONE);
        assert_eq!(app.filter, 0, "t 同效");
        app.shutdown().await;
    }
}

#[cfg(test)]
mod bt_key_tests {

    use super::*;
    use crate::app::{BtConnRow, Panel};
    use crate::model::{Protocol, TaskState};

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-btkey", tag)
    }

    fn bt_task(id: u32, name: &str, state: TaskState) -> crate::model::Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t.protocol = Protocol::Bt;
        t.state = state;
        t
    }

    fn row(id: usize, addr: &str) -> BtConnRow {
        BtConnRow {
            id,
            addr: addr.to_string(),
            down_speed: 0.0,
            cum_down: 0,
            up_speed: 0.0,
            cum_up: 0,
            client: "QB".to_string(),
        }
    }

    #[tokio::test]
    async fn a_opens_cmd_window_not_add_dialog() {
        let mut app = make_app("a");
        app.tasks.push(bt_task(1, "t.bin", TaskState::Downloading));
        app.selected = 0;
        app.on_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(app.cmd_open, "A 打开命令窗口");
        assert!(app.dialog.is_none(), "A 不再直开添加对话框");
        app.on_key(KeyCode::Char('d'), KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "命令窗口模态消费主界面按键");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn m_routes_bt_to_bt_modify_dialog() {
        let mut app = make_app("m");
        app.tasks.push(bt_task(1, "bt.bin", TaskState::Downloading));
        app.selected = 0;
        app.on_key(KeyCode::Char('m'), KeyModifiers::NONE);
        assert!(
            app.dialog
                .as_ref()
                .is_some_and(|d| d.kind == super::super::DialogKind::ModifyBt),
            "BT 任务 M 开修改任务：BT 对话框: {:?}",
            app.dialog.as_ref().map(|d| d.kind)
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn b_disconnects_selected_bt_conn_row() {
        let mut app = make_app("b-bt");
        app.tasks.push(bt_task(1, "bt.bin", TaskState::Downloading));
        app.selected = 0;
        app.bt_conns.insert(
            1,
            vec![
                row(1, "1.2.3.4:51400"),
                row(2, "5.6.7.8:51401"),
                row(3, "5.6.7.8:51402"),
            ],
        );
        app.conns_sel = 1;
        app.focus = Panel::Conns;
        app.on_key(KeyCode::Char('b'), KeyModifiers::NONE);
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("✂ 已断开连接 5.6.7.8（剩余 2 条）")),
            "断开 toast = 选中行 IP（去 :port）: {:?}",
            app.toast
        );
        assert_eq!(app.bt_conns.get(&1).map(Vec::len), Some(3));
        assert_eq!(app.conns_sel, 1);
        app.shutdown().await;
    }

    #[tokio::test]
    async fn b_bt_empty_rows_toast() {
        let mut app = make_app("b-empty");
        app.tasks.push(bt_task(1, "bt.bin", TaskState::Downloading));
        app.selected = 0;
        app.on_key(KeyCode::Char('b'), KeyModifiers::NONE);
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("当前任务没有可断开的并发连接")),
            "空明细 toast: {:?}",
            app.toast
        );
        app.shutdown().await;
    }
}
