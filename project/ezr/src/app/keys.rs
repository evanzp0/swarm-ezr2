//! keys — 键盘事件：全局键路由与对话框逐键处理

use crossterm::event::{KeyCode, KeyModifiers};

use super::{App, FILTERS};

impl App {
    /// 键盘事件入口
    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) {
            if code == KeyCode::Char('c') {
                self.quit = true;
                return;
            }
            // 并发连接面板快捷键（FR-01-97，对话框打开时不响应）：
            // Ctrl+↑/↓ 上下选择连接明细，Ctrl+B 断开选中连接（仅 BT 任务）
            if self.dialog.is_none() {
                match code {
                    KeyCode::Up => self.move_conn_sel(-1),
                    KeyCode::Down => self.move_conn_sel(1),
                    KeyCode::Char('b') | KeyCode::Char('B') => self.disconnect_conn(),
                    _ => {}
                }
            }
            return;
        }
        if self.dialog.is_some() {
            self.on_dialog_key(code);
            return;
        }
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Down => self.move_sel(1),
            KeyCode::Up => self.move_sel(-1),
            KeyCode::PageDown => self.move_sel(4),
            KeyCode::PageUp => self.move_sel(-4),
            KeyCode::Home => self.selected = 0,
            KeyCode::End => {
                let flen = self.filtered().len();
                if flen > 0 {
                    self.selected = flen - 1;
                }
            }
            KeyCode::Tab => self.filter = (self.filter + 1) % FILTERS.len(),
            KeyCode::BackTab => self.filter = (self.filter + FILTERS.len() - 1) % FILTERS.len(),
            KeyCode::Char(' ') => self.toggle_pause(),
            KeyCode::Char('r') | KeyCode::Char('R') => self.retry(),
            KeyCode::Char('a') | KeyCode::Char('A') => self.open_add_dialog(),
            KeyCode::Char('m') | KeyCode::Char('M') => self.open_modify_dialog(),
            KeyCode::Char('d') | KeyCode::Char('D') => self.open_delete_dialog(),
            KeyCode::Char('c') | KeyCode::Char('C') => self.clear_completed(),
            // FR-01-98：G 仅切换任务详情 / 并发连接面板（紧凑布局），流量图常显
            KeyCode::Char('g') | KeyCode::Char('G') => self.show_panes = !self.show_panes,
            KeyCode::Char('u') | KeyCode::Char('U') => self.move_task(-1),
            KeyCode::Char('j') | KeyCode::Char('J') => self.move_task(1),
            _ => {}
        }
    }

    /// Ctrl+↑/↓（FR-01-97）：在并发连接明细中上下移动选中行（越界自动跟随滚动）。
    /// 状态门槛先于连接数判断：仅「下载中/做种中」可选，否则 toast 提示。
    pub(crate) fn move_conn_sel(&mut self, delta: i32) {
        let Some(idx) = self.sel_idx() else {
            return;
        };
        if !self.tasks[idx].state.shows_conns() {
            self.set_toast("当前状态无并发明细（仅下载中/做种中可选）");
            return;
        }
        // 选择序号与展示序号同序（面板按连接 id 升序渲染，FR-01-96/97 同一口径）
        let mut ids: Vec<usize> = self.tasks[idx].connections.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        let n = ids.len();
        if n == 0 {
            self.set_toast("当前任务没有并发连接明细");
            return;
        }
        let new = (self.conns_sel as i32 + delta).clamp(0, n as i32 - 1) as usize;
        self.conns_sel = new;
        // 选中行越出可视窗口时跟随滚动（visible_conns_rows 由 ui 层回填）
        let vis = self.visible_conns_rows.max(1);
        if new < self.conns_scroll {
            self.conns_scroll = new;
        } else if new >= self.conns_scroll + vis {
            self.conns_scroll = new + 1 - vis;
        }
    }

    /// Ctrl+B（FR-01-97/D26）：断开选中连接——仅 BT 任务。守卫链顺序固定：
    /// 状态门槛 → 协议门槛 → 空明细。01 期无 BT 任务，协议门槛后的实际断开
    /// （连接移除 + 分块补完）属 phase-02 范围，此处为不可达臂。
    pub(crate) fn disconnect_conn(&mut self) {
        let Some(idx) = self.sel_idx() else {
            return;
        };
        if !self.tasks[idx].state.shows_conns() {
            self.set_toast("当前状态无可断开的并发连接（仅下载中/做种中）");
            return;
        }
        if !self.tasks[idx].protocol.is_bt() {
            self.set_toast("仅 BT 任务支持断开并发连接（Ctrl+B）");
            return;
        }
        if self.tasks[idx].connections.is_empty() {
            self.set_toast("当前任务没有可断开的并发连接");
        }
        // phase-02 落地点（D26）：BT 引擎断开命令 + 连接视图移除 + 分块补完语义
    }

    fn move_sel(&mut self, delta: i32) {
        let flen = self.filtered().len();
        if flen == 0 {
            return;
        }
        let cur = self.selected as i32;
        self.selected = cur.saturating_add(delta).clamp(0, flen as i32 - 1) as usize;
    }

    /// 上移/下移选中任务（调整排队优先级，FR-01-32）
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

    // -----------------------------------------------------------------------
    // 对话框（结构沿用 demo）
    // -----------------------------------------------------------------------
}

#[cfg(test)]
mod conn_key_tests {
    //! FR-01-97 并发明细交互单测：Ctrl+↑/↓ 选择（钳制/跟随滚动/复位）、
    //! 状态门槛与协议门槛 toast（文案照录 demo 定稿，D26）、对话框内不响应。

    use super::*;
    use crate::model::config::Config;
    use crate::model::{Connection, TaskState};

    fn make_app(tag: &str) -> App {
        let dir = crate::model::testenv::uniq_tmp_dir(&format!("ezr-ck-{tag}"));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        App::new(Config::default(), reg)
    }

    fn task_with_conns(id: u32, name: &str, state: TaskState, n: usize) -> crate::model::Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t.state = state;
        t.connections = (1..=n)
            .map(|i| Connection {
                id: i,
                start: (i as u64 - 1) * 1000,
                end: i as u64 * 1000,
                done: 100,
            })
            .collect();
        t
    }

    fn ctrl(app: &mut App, code: KeyCode) {
        app.on_key(code, KeyModifiers::CONTROL);
    }

    #[tokio::test]
    async fn ctrl_updown_moves_and_clamps() {
        let mut app = make_app("sel");
        app.tasks
            .push(task_with_conns(1, "a.bin", TaskState::Downloading, 4));
        app.selected = 0;
        for _ in 0..5 {
            ctrl(&mut app, KeyCode::Down);
        }
        assert_eq!(app.conns_sel, 3, "下移钳制到末行（4 条 → 3）");
        for _ in 0..10 {
            ctrl(&mut app, KeyCode::Up);
        }
        assert_eq!(app.conns_sel, 0, "上移钳制到首行");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn ctrl_down_follows_scroll() {
        let mut app = make_app("follow");
        app.tasks
            .push(task_with_conns(1, "b.bin", TaskState::Downloading, 4));
        app.selected = 0;
        app.visible_conns_rows = 2;
        for _ in 0..3 {
            ctrl(&mut app, KeyCode::Down);
        }
        assert_eq!(app.conns_sel, 3);
        assert_eq!(app.conns_scroll, 2, "选中越出可视窗 → 跟随滚动");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn ctrl_updown_state_gate_toast() {
        let mut app = make_app("gate");
        app.tasks
            .push(task_with_conns(1, "p.bin", TaskState::Paused, 3));
        app.selected = 0;
        ctrl(&mut app, KeyCode::Down);
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
    async fn ctrl_down_no_conns_toast() {
        let mut app = make_app("empty");
        app.tasks
            .push(task_with_conns(1, "e.bin", TaskState::Downloading, 0));
        app.selected = 0;
        ctrl(&mut app, KeyCode::Down);
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("当前任务没有并发连接明细")),
            "空明细 toast: {:?}",
            app.toast
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn ctrl_b_http_rejects_with_protocol_toast() {
        let mut app = make_app("bhttp");
        app.tasks
            .push(task_with_conns(1, "h.bin", TaskState::Downloading, 3));
        app.selected = 0;
        ctrl(&mut app, KeyCode::Char('b'));
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("仅 BT 任务支持断开并发连接（Ctrl+B）")),
            "HTTP 拒绝 toast: {:?}",
            app.toast
        );
        assert_eq!(app.tasks[0].connections.len(), 3, "明细行数不变");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn ctrl_b_state_gate_precedes_protocol() {
        let mut app = make_app("bgate");
        // 已暂停的 HTTP 任务：状态门槛先于协议判断（FR-01-97 ②）
        app.tasks
            .push(task_with_conns(1, "g.bin", TaskState::Paused, 3));
        app.selected = 0;
        ctrl(&mut app, KeyCode::Char('b'));
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
    async fn ctrl_keys_ignored_when_dialog_open() {
        let mut app = make_app("dlg");
        app.tasks
            .push(task_with_conns(1, "d.bin", TaskState::Downloading, 3));
        app.selected = 0;
        app.open_add_dialog();
        assert!(app.dialog.is_some());
        ctrl(&mut app, KeyCode::Down);
        assert_eq!(app.conns_sel, 0, "对话框打开时 Ctrl+↓ 不作用于明细");
        ctrl(&mut app, KeyCode::Char('b'));
        assert!(
            !app.toast.as_deref().is_some_and(|m| m.contains("仅 BT")),
            "对话框打开时 Ctrl+B 不触发协议 toast"
        );
        ctrl(&mut app, KeyCode::Char('c'));
        assert!(app.quit, "Ctrl+C 退出优先级不变");
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
        ctrl(&mut app, KeyCode::Down);
        assert_eq!(app.conns_sel, 1);
        // 切换选中任务（tick 钳制臂负责复位）
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.selected, 1, "列表选中切到下一任务");
        app.tick().await;
        assert_eq!(app.conns_sel, 0, "切换任务后明细选择复位（FR-01-97）");
        assert_eq!(app.conns_scroll, 0, "切换任务后明细滚动复位");
        app.shutdown().await;
    }
}
