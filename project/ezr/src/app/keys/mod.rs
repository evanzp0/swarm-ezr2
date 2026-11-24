use crossterm::event::{KeyCode, KeyModifiers};

use super::{App, Panel, SbKey};

mod conns;
mod overlays;

impl App {
    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) {
            if code == KeyCode::Char('c') {
                self.quit = true;
                return;
            }
            if code == KeyCode::Char('r') {
                // Ctrl+R 配置热加载（FR-01-119）：与 Ctrl+C 同级先于浮层分派，
                // 主界面与全部浮层内可达，浮层不因热加载关闭。
                self.reload_config();
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
            self.on_val_key(code);
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
            self.on_about_key(code);
            return;
        }
        if self.pieces_open {
            self.on_pieces_key(code);
            return;
        }
        if self.trackers_open {
            self.on_trackers_key(code);
            return;
        }
        if self.help_open {
            self.on_help_key(code);
            return;
        }
        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.quit = true,
            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::BackTab => self.focus = self.focus.prev(),
            KeyCode::Char('t') | KeyCode::Char('T') => self.open_trackers_popup(),
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
        let next = ((self.filter as i32 + delta).rem_euclid(n)) as usize;
        self.set_filter(next);
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
        assert_eq!(
            app.filter, 0,
            "t/T 页签切换退役（FR-02-31）：空任务 toast 拒绝"
        );
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("未选中任务，无法查看 trackers")),
            "t 门控臂 toast: {:?}",
            app.toast
        );
        app.on_key(KeyCode::Char('t'), KeyModifiers::NONE);
        assert_eq!(app.filter, 0, "t 同效（退役后不切页签）");
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
        crate::app::testutil::bt_conn_row(id, addr)
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
            app.toast.as_deref().is_some_and(|m| {
                m.contains("✂ 已断开 5.6.7.8 并加入黑名单（剩余 2 条）")
            }),
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

    #[tokio::test]
    async fn tick_keeps_bt_peer_selection_not_reset_to_first_row() {
        let mut app = make_app("tickbt");
        app.tasks.push(bt_task(1, "bt.bin", TaskState::Downloading));
        app.selected = 0;
        app.bt_conns.insert(
            1,
            vec![
                row(1, "1.2.3.4:51400"),
                row(2, "5.6.7.8:51401"),
                row(3, "9.9.9.9:51402"),
            ],
        );
        app.conns_sel = 2;
        app.tick().await;
        assert_eq!(
            app.conns_sel, 2,
            "BT 多 peer 明细：tick 钳制与 bt_conns 行数同源，选择不被复位到首行（D62④）"
        );
        app.conns_sel = 1;
        app.tick().await;
        assert_eq!(app.conns_sel, 1, "再次 tick 仍保持");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn tick_still_clamps_bt_selection_when_rows_shrink() {
        let mut app = make_app("tickbt2");
        app.tasks.push(bt_task(1, "bt.bin", TaskState::Downloading));
        app.selected = 0;
        app.bt_conns
            .insert(1, vec![row(1, "1.2.3.4:51400"), row(2, "5.6.7.8:51401")]);
        app.conns_sel = 5;
        app.tick().await;
        assert_eq!(
            app.conns_sel, 1,
            "越界选择仍钳制到末行（行数同源口径的守卫臂保持）"
        );
        app.shutdown().await;
    }
}

#[cfg(test)]
mod reload_key_tests {
    use super::*;
    use crate::model::TaskState;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-rkey", tag)
    }

    fn press_ctrl_r(app: &mut App) {
        app.on_key(KeyCode::Char('r'), KeyModifiers::CONTROL);
    }

    #[tokio::test]
    async fn ctrl_r_fires_on_main_ui_and_every_overlay_stays_open() {
        let mut app = make_app("sweep");
        app.tasks.push(crate::app::testutil::dl_task(1, "r.bin"));
        app.selected = 0;

        press_ctrl_r(&mut app);
        assert!(app.toast.is_some(), "主界面 Ctrl+R 触发热加载");
        assert!(!app.quit, "不退出");

        app.on_key(KeyCode::Char('h'), KeyModifiers::NONE);
        assert!(app.help_open);
        press_ctrl_r(&mut app);
        assert!(app.help_open, "帮助窗口不因热加载关闭");
        assert!(app.toast.is_some(), "帮助窗口内 Ctrl+R 仍触发热加载");

        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        app.on_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(app.cmd_open, "A 打开命令窗口");
        app.cmd.set_text("https://example.com/r.bin");
        app.cmd_confirm();
        assert!(app.dialog.is_some(), "命令分流打开添加对话框");
        press_ctrl_r(&mut app);
        assert!(app.dialog.is_some(), "对话框不因热加载关闭");
        assert!(app.toast.is_some());

        app.dialog = None;
        app.on_key(KeyCode::Char('p'), KeyModifiers::NONE);
        assert!(app.pieces_open);
        press_ctrl_r(&mut app);
        assert!(app.pieces_open, "分块弹窗内 Ctrl+R 可达且弹窗不关闭");

        app.pieces_open = false;
        app.err_open = true;
        app.err_msg = "测试错误".to_string();
        press_ctrl_r(&mut app);
        assert!(app.err_open, "错误窗口内 Ctrl+R 可达且不关闭");

        app.err_open = false;
        app.val_open = true;
        press_ctrl_r(&mut app);
        assert!(app.val_open, "值弹窗内 Ctrl+R 可达且不关闭");

        app.val_open = false;
        app.about_open = true;
        press_ctrl_r(&mut app);
        assert!(app.about_open, "关于窗口内 Ctrl+R 可达且不关闭");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn ctrl_r_in_cmd_window_does_not_swallow_cmd_keys() {
        let mut app = make_app("cmdkey");
        app.open_cmd_window();
        press_ctrl_r(&mut app);
        assert!(app.cmd_open, "命令窗口不因热加载关闭");
        assert!(app.toast.is_some(), "命令窗口内 Ctrl+R 触发热加载");
        press_ctrl_r(&mut app);
        assert!(app.cmd_open, "连续 Ctrl+R 幂等");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn plain_r_keeps_retry_semantics_and_ctrl_r_only_with_control_mod() {
        let mut app = make_app("modsep");
        app.tasks.push(crate::app::testutil::dl_task(1, "m.bin"));
        app.tasks[0].state = TaskState::Failed;
        app.selected = 0;
        app.on_key(KeyCode::Char('r'), KeyModifiers::NONE);
        assert_eq!(app.tasks[0].state, TaskState::Queued, "裸 R 仍走重试语义");
        press_ctrl_r(&mut app);
        assert!(app.toast.is_some(), "Ctrl 修饰键才进热加载");
        app.shutdown().await;
    }
}

#[cfg(test)]
mod overlay_key_nav_tests {
    use super::*;
    use crate::app::testutil::{bt_task, dl_task};
    use crate::app::DetailField;
    use crate::model::TaskState;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-okey", tag)
    }

    fn row(id: usize, addr: &str) -> crate::app::BtConnRow {
        crate::app::testutil::bt_conn_row(id, addr)
    }

    #[tokio::test]
    async fn val_popup_scroll_copy_and_close_keys() {
        let mut app = make_app("valnav");
        app.tasks.push(dl_task(1, "v.bin"));
        app.selected = 0;
        app.open_val_popup(DetailField::Url);
        assert!(app.val_open, "前置：值弹窗打开");
        app.val_max_scroll = 40;

        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.val_scroll, 2, "↓ 逐行下滚");
        app.on_key(KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(app.val_scroll, 10, "PgDn 按页 8 行");
        app.on_key(KeyCode::PageUp, KeyModifiers::NONE);
        assert_eq!(app.val_scroll, 2, "PgUp 按页 8 行");
        app.on_key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(app.val_scroll, 1, "↑ 逐行上滚");
        app.on_key(KeyCode::Up, KeyModifiers::NONE);
        app.on_key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(app.val_scroll, 0, "↑ 顶部钳制");
        app.on_key(KeyCode::End, KeyModifiers::NONE);
        assert_eq!(app.val_scroll, 40, "End 钳制到滚动上限");
        app.on_key(KeyCode::Home, KeyModifiers::NONE);
        assert_eq!(app.val_scroll, 0, "Home 回首页");

        app.on_key(KeyCode::Char('y'), KeyModifiers::NONE);
        assert_eq!(
            app.last_copied.as_deref(),
            Some(app.tasks[0].url.as_str()),
            "y 复制弹窗值"
        );
        assert!(
            app.toast.as_deref().unwrap_or_default().contains("已复制"),
            "y 弹出已复制 toast，toast={:?}",
            app.toast
        );
        assert!(app.val_open, "y 不关闭弹窗");

        app.on_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(!app.val_open, "q 关闭弹窗");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn val_popup_q_y_uppercase_and_esc_close() {
        let mut app = make_app("valclose");
        app.tasks.push(dl_task(2, "v2.bin"));
        app.selected = 0;
        app.open_val_popup(DetailField::Url);
        app.on_key(KeyCode::Char('Y'), KeyModifiers::NONE);
        assert!(app.val_open, "Y 复制不关闭");
        assert!(app.last_copied.is_some(), "Y 同样复制");
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.val_open, "Esc 关闭弹窗");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn err_window_only_close_keys_reach_handler() {
        let mut app = make_app("errnav");
        app.open_error_window("boom");
        app.on_key(KeyCode::Char('x'), KeyModifiers::NONE);
        assert!(app.err_open, "非关闭键不动作");
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.err_open, "Esc 经 on_err_key 关闭");
        app.open_error_window("boom2");
        app.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(!app.err_open, "Enter 关闭");
        app.open_error_window("boom3");
        app.on_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(!app.err_open, "q 关闭");
        app.open_error_window("boom4");
        app.on_key(KeyCode::Char('Q'), KeyModifiers::NONE);
        assert!(!app.err_open && !app.quit, "Q 关闭不退出");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn trackers_popup_scroll_and_close_keys() {
        let mut app = make_app("trknav");
        app.tasks.push(bt_task(3, TaskState::Downloading));
        app.selected = 0;
        app.trackers_max_scroll = 30;

        app.on_key(KeyCode::Char('t'), KeyModifiers::NONE);
        assert!(app.trackers_open, "t 打开 tracker 弹窗");

        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.trackers_scroll, 2, "↓ 逐行下滚");
        app.on_key(KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(app.trackers_scroll, 10, "PgDn 按页 8 行");
        app.on_key(KeyCode::PageUp, KeyModifiers::NONE);
        assert_eq!(app.trackers_scroll, 2, "PgUp 按页 8 行");
        app.on_key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(app.trackers_scroll, 1, "↑ 逐行上滚");
        app.on_key(KeyCode::End, KeyModifiers::NONE);
        assert_eq!(app.trackers_scroll, usize::MAX, "End 交给渲染层钳制");
        app.on_key(KeyCode::Home, KeyModifiers::NONE);
        assert_eq!(app.trackers_scroll, 0, "Home 回首页");

        app.on_key(KeyCode::Char('t'), KeyModifiers::NONE);
        assert!(!app.trackers_open, "t 关闭弹窗");
        app.on_key(KeyCode::Char('t'), KeyModifiers::NONE);
        app.on_key(KeyCode::Char('T'), KeyModifiers::NONE);
        assert!(!app.trackers_open, "T 关闭弹窗");
        app.on_key(KeyCode::Char('t'), KeyModifiers::NONE);
        app.on_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(!app.trackers_open && !app.quit, "q 关闭不退出");
        app.on_key(KeyCode::Char('t'), KeyModifiers::NONE);
        app.on_key(KeyCode::Char('Q'), KeyModifiers::NONE);
        assert!(!app.trackers_open && !app.quit, "Q 关闭不退出");
        app.on_key(KeyCode::Char('t'), KeyModifiers::NONE);
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.trackers_open, "Esc 关闭不退出");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn conns_panel_navigate_page_home_end_and_hscroll() {
        let mut app = make_app("connav");
        app.tasks.push(bt_task(4, TaskState::Downloading));
        app.selected = 0;
        app.bt_conns.insert(
            4,
            vec![
                row(1, "1.1.1.1:51000"),
                row(2, "2.2.2.2:51001"),
                row(3, "3.3.3.3:51002"),
            ],
        );
        app.focus = Panel::Conns;
        app.conns_max_hscroll = 20;

        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.conns_sel, 1, "↓ 并发明细下移");
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.conns_sel, 2, "↓ 末行钳制");
        app.on_key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(app.conns_sel, 1, "↑ 上移");
        app.on_key(KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(app.conns_sel, 2, "PgDn 页移并钳制末行");
        app.on_key(KeyCode::PageUp, KeyModifiers::NONE);
        assert_eq!(app.conns_sel, 0, "PgUp 页移钳制首行");
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        app.on_key(KeyCode::End, KeyModifiers::NONE);
        assert_eq!(app.conns_sel, 2, "End 跳末行");
        app.on_key(KeyCode::Home, KeyModifiers::NONE);
        assert_eq!(app.conns_sel, 0, "Home 跳首行");

        app.on_key(KeyCode::Right, KeyModifiers::NONE);
        assert!(app.conns_hscroll > 0, "右滚水平滚动条");
        let before = app.conns_hscroll;
        app.on_key(KeyCode::Left, KeyModifiers::NONE);
        assert!(app.conns_hscroll < before, "左滚水平滚动条");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn detail_focus_up_down_are_noops() {
        let mut app = make_app("detaillno");
        app.tasks.push(dl_task(5, "d.bin"));
        app.selected = 0;
        app.focus = Panel::Detail;
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        app.on_key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(app.focus, Panel::Detail, "焦点不动");
        assert_eq!(app.selected, 0, "选择不动");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn ctrl_j_in_cmd_window_confirms_and_closes_empty_window() {
        let mut app = make_app("ctrlj");
        app.open_cmd_window();
        app.on_key(KeyCode::Char('j'), KeyModifiers::CONTROL);
        assert!(!app.cmd_open, "空输入 Ctrl+J 确认即关闭命令窗口");
        assert!(!app.quit, "确认不退出");
        app.shutdown().await;
    }
}
