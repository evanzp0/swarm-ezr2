//! keys — 键盘事件：全局键路由与对话框逐键处理
//!
//! v1.30/FR-01-108 键位改版（对齐 ezr-demo 重构版 app.rs on_key）：
//! - `Tab`/`BackTab` = 面板焦点循环（列表 → 详情 → 并发），原切页签语义
//!   改由页签键 `]`/`[`（含 `}`/`{`）与 `T` 承载；
//! - `↑`/`↓`（含 PgUp/PgDn/Home/End）按焦点面板分发：列表 = 选任务、
//!   并发 = 选连接明细（原 `Ctrl+↑↓` 退役）、详情 = 无操作；
//! - `B` 断开选中连接（原 `Ctrl+B`，不限焦点面板）；
//! - `H` 打开帮助窗口（FR-01-107）；`Esc` 主界面无操作（仅用于关闭浮层，
//!   防误触退出）；`Ctrl` 组合键仅保留 `Ctrl+C` 退出（D37）。

use crossterm::event::{KeyCode, KeyModifiers};

use super::{App, Panel, SbKey};

impl App {
    /// 键盘事件入口
    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) {
            if code == KeyCode::Char('c') {
                self.quit = true;
            }
            // 其余 Ctrl 组合键不再占用（原 Ctrl+↑↓/Ctrl+B 并发面板快捷键已
            // 统一进面板焦点模型：聚焦面板后 ↑↓ 选择、B 断开，v1.30/D37）
            return;
        }
        // 字段值弹窗最上层模态（v1.34/FR-01-113）：Y 复制窗口全部内容 + toast、
        // 滚动翻阅、Esc/q/Q 关闭（先关窗不退出）；Ctrl+C 退出优先级不变
        if self.val_open {
            match code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => self.close_val_popup(),
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    // Y 复制窗口全部内容（打开时快照的完整字段值）+ toast
                    //（label 单源 DetailField::copy_label，与字段名点击同口径，D42）
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
        // 对话框打开时，按键优先交给对话框
        if self.dialog.is_some() {
            self.on_dialog_key(code);
            return;
        }
        // 关于窗口最上层模态（v1.32/FR-01-111）：Esc/O/q/Q 关闭（先关窗
        // 不退出），其余键忽略——模态优先级 关于 > 帮助 > pieces > 对话框
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
        // pieces 分块弹窗打开时进入模态（FR-01-106 ⑥）：按键仅用于滚动与关闭
        //（q/Q 同为关闭而非退出——弹窗内先关窗，避免误触直接退出应用）
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
                KeyCode::End => self.pieces_scroll = usize::MAX, // 渲染层钳制到末页
                _ => {}
            }
            return;
        }
        // 帮助窗口模态（v1.30/FR-01-107）：滚动翻阅分类帮助；Esc/H/q/Q 关闭
        //（q 同 pieces 口径：先关窗不退出）；O 打开关于窗口（v1.32/FR-01-111：
        // About 叠于帮助之上，关闭后回帮助）
        if self.help_open {
            match code {
                KeyCode::Esc
                | KeyCode::Char('h')
                | KeyCode::Char('H')
                | KeyCode::Char('q')
                | KeyCode::Char('Q') => self.help_open = false,
                KeyCode::Char('o') | KeyCode::Char('O') => self.open_about(),
                KeyCode::Up => self.help_scroll = self.help_scroll.saturating_sub(1),
                KeyCode::Down => self.help_scroll += 1, // 渲染层钳制
                KeyCode::PageUp => self.help_scroll = self.help_scroll.saturating_sub(8),
                KeyCode::PageDown => self.help_scroll += 8,
                KeyCode::Home => self.help_scroll = 0,
                KeyCode::End => self.help_scroll = usize::MAX,
                _ => {}
            }
            return;
        }
        match code {
            // Q/q：退出程序（v1.30：Esc 仅用于关闭浮层，主界面按 Esc 无操作
            // ——避免想关弹窗时误触退出，FR-01-80 v1.30 修订）
            KeyCode::Char('q') | KeyCode::Char('Q') => self.quit = true,
            // Tab/BackTab：面板焦点循环（v1.30/FR-01-108，lazygit 式聚焦；
            // 当前面板边框高亮 + 页签高亮跟随 + ↑↓ 作用于本面板列表）
            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::BackTab => self.focus = self.focus.prev(),
            // T：页签循环（正在下载 ⇄ 已完成；与 ] 等效，保留兼容）。
            // 切换后 filtered 列表已变，显式将视口定位到选中项（可见）
            KeyCode::Char('t') | KeyCode::Char('T') => self.cycle_filter(1),
            // ] / [：页签右/左切换（含 Shift 变体 } / {，FR-01-80 v1.30）
            KeyCode::Char(']') | KeyCode::Char('}') => self.cycle_filter(1),
            KeyCode::Char('[') | KeyCode::Char('{') => self.cycle_filter(-1),
            // H：帮助窗口（分类交互说明；Esc/H/q/Q 关闭，FR-01-107）
            KeyCode::Char('h') | KeyCode::Char('H') => {
                self.help_open = true;
                self.help_scroll = 0;
                // help_pos 保留上次拖拽位置（跨次记忆）；None = 渲染层首帧居中并回填
            }
            // O：关于窗口（v1.32/FR-01-111：40×7，EZR Download v1.0.0；
            // Esc/O/q/Q 关闭，位置跨次记忆）
            KeyCode::Char('o') | KeyCode::Char('O') => self.open_about(),
            // ←/→（v1.32/FR-01-109 ⑦ m3 新增）：并发连接面板聚焦时横向滚动
            // 明细列（±2 列/步，SbKey::ConnsH）；无横向内容的面板不响应
            //（HTTP 22 列不超宽 → 上限 0 钳制，事件内末页即停）
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
            // ↑/↓：统一列表选择交互——聚焦任务列表 = 选任务；聚焦并发连接 =
            // 选连接明细（原 Ctrl+↑↓ 语义，v1.30/FR-01-97 修订：移除 Ctrl 方式）；
            // 聚焦详情 = 无操作
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
            // B：阻塞并移除（断开）选中的并发连接——仅 BT 任务（原 Ctrl+B 语义
            // 同构改单键，不限焦点面板，v1.30/FR-01-97 修订）
            KeyCode::Char('b') | KeyCode::Char('B') => self.disconnect_conn(),
            KeyCode::Char(' ') => self.toggle_pause(),
            KeyCode::Char('r') | KeyCode::Char('R') => self.retry(),
            // P：查看选中任务的 pieces 分块弹窗（仅支持分块下载的任务）
            KeyCode::Char('p') | KeyCode::Char('P') => self.open_pieces(),
            KeyCode::Char('a') | KeyCode::Char('A') => self.open_add_dialog(),
            KeyCode::Char('m') | KeyCode::Char('M') => self.open_modify_dialog(),
            KeyCode::Char('d') | KeyCode::Char('D') => self.open_delete_dialog(),
            KeyCode::Char('c') | KeyCode::Char('C') => self.clear_completed(),
            // FR-01-98：G 仅切换任务详情 / 并发连接面板（紧凑布局），流量图常显；
            // 收起后详情/并发面板不可见，焦点强制回任务列表（v1.30 注记）
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

    /// 当前选中任务的并发明细条数（仅「下载中」/「做种中」任务有明细）
    pub(crate) fn conn_count(&self) -> usize {
        self.active_conn_ids().len()
    }

    /// `↑`/`↓`（并发面板聚焦，v1.30/FR-01-97）：在并发连接明细中上下移动
    /// 选中行（越界自动跟随滚动）。状态门槛先于连接数判断：仅「下载中/做种中」
    /// 可选，否则 toast 提示。
    pub(crate) fn move_conn_sel(&mut self, delta: i32) {
        let Some(idx) = self.sel_idx() else {
            return;
        };
        if !self.tasks[idx].state.shows_conns() {
            self.set_toast("当前状态无并发明细（仅下载中/做种中可选）");
            return;
        }
        // 选择序号与展示序号同序（v1.34：面板只显示活跃并发，按连接 id 升序
        // 渲染 = 连接池槽位号序，与 [`App::active_conn_ids`] 同源，FR-01-96 ⑦）
        let ids = self.active_conn_ids();
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

    /// `B`（v1.30/FR-01-97 修订，原 `Ctrl+B`/D26）：断开选中连接——仅 BT 任务，
    /// 不限焦点面板。守卫链顺序固定：状态门槛 → 协议门槛 → 空明细。
    /// v1.34/FR-01-96 ⑦：明细只显示活跃并发，选中行先经活跃映射
    ///（[`App::active_conn_ids`]，显示行序 = 活跃 id 升序）映射回真实连接
    /// 再移除——01 期无 BT 任务，协议门槛后的实际断开（连接移除 + 分块补完）
    /// 属 phase-02 范围，此处为不可达臂；活跃映射已就位，phase-02 落地时
    /// 按 `ids[conns_sel]` 取真实连接 id 发引擎断开命令。
    pub(crate) fn disconnect_conn(&mut self) {
        let Some(idx) = self.sel_idx() else {
            return;
        };
        if !self.tasks[idx].state.shows_conns() {
            self.set_toast("当前状态无可断开的并发连接（仅下载中/做种中）");
            return;
        }
        if !self.tasks[idx].protocol.is_bt() {
            // v1.30：拒绝文案键名随键改 `（B）`（原 `（Ctrl+B）` 退役）
            self.set_toast("仅 BT 任务支持断开并发连接（B）");
            return;
        }
        // 活跃映射（v1.34）：显示行 → 真实连接 id（选中行越界或空明细 = 无可断开）
        let ids = self.active_conn_ids();
        if self.conns_sel >= ids.len() {
            self.set_toast("当前任务没有可断开的并发连接");
            return;
        }
        let _conn_id = ids[self.conns_sel]; // phase-02：真实连接 id → 引擎断开命令
    }

    fn move_sel(&mut self, delta: i32) {
        let flen = self.filtered().len();
        if flen == 0 {
            return;
        }
        let cur = self.selected as i32;
        self.selected = cur.saturating_add(delta).clamp(0, flen as i32 - 1) as usize;
        // 视口即时跟随选中项（不依赖下一帧 tick，避免一帧延迟；v1.30/FR-01-109）
        self.scroll_to_selection();
    }

    /// 页签切换（delta = ±1，环形）；切换后视口定位到选中项
    ///（v1.30/FR-01-80：页签键 `]`/`[`/`T` 承载原 Tab 切页签语义）
    pub(crate) fn cycle_filter(&mut self, delta: i32) {
        let n = super::FILTERS.len() as i32;
        self.filter = ((self.filter as i32 + delta).rem_euclid(n)) as usize;
        self.scroll_to_selection();
    }

    /// 将视口滚动到选中项可见（选中项变化 / 页签切换后调用；
    /// 鼠标滚动不调用——视口自由移动正是滚动条不锁死的前提，v1.30/FR-01-109）
    pub(crate) fn scroll_to_selection(&mut self) {
        let vis = self.visible_rows.max(1);
        if self.scroll > self.selected {
            self.scroll = self.selected;
        }
        if self.selected >= self.scroll + vis {
            self.scroll = self.selected + 1 - vis;
        }
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
mod pieces_key_tests {
    //! FR-01-106 ⑥ 弹窗模态与关闭单测：模态消费主界面按键、q/Q 先关窗不
    //! 退出、滚动键逐行/按页/Home/End。

    use super::*;
    use crate::app::testutil::dl_task_mb;
    use crate::model::TaskState;

    /// 夹具单源委托（app 树 App 夹具族收口 testutil，architect v153）
    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-pkey", tag)
    }

    fn dl_task(id: u32, name: &str) -> crate::model::Task {
        // 分块数学族口径（block_size = 1 MB）单源 testutil（architect v153）
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
        // 模态：主界面快捷键全部被消费为无动作
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
        // q/Q 先关窗不退出（FR-01-106 ⑥）
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
    //! v1.30/FR-01-107 帮助窗口键盘单测：H 开关、模态消费、滚动键、
    //! q/Q/Esc/H 先关窗不退出。

    use super::*;

    /// 夹具单源委托（app 树 App 夹具族收口 testutil，architect v153）
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
        // 大写 H 同口径
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
        // 主界面快捷键全部被消费（模态，按键优先级帮助窗口最高）
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
    //! v1.30/FR-01-97 修订并发明细交互单测：`↑`/`↓`（并发面板聚焦时）选择、
    //! `B` 断开（不限焦点）、拒绝 toast 文案 `…（B）`、Ctrl 组合键退役、
    //! Esc 主界面无操作。

    use super::*;
    use crate::app::testutil::task_with_conns;
    use crate::model::TaskState;

    /// 夹具单源委托（app 树 App 夹具族收口 testutil，architect v153）
    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-ck", tag)
    }

    // task_with_conns 夹具收敛至共享模块（与 mouse 测试同源单点，v152-clean）

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

    /// 列表聚焦时 ↑↓ 选任务、明细选择不动（焦点分发，FR-01-108）
    #[tokio::test]
    async fn updown_dispatches_by_focus() {
        let mut app = make_app("dispatch");
        app.tasks
            .push(task_with_conns(1, "a.bin", TaskState::Downloading, 3));
        app.tasks
            .push(task_with_conns(2, "b.bin", TaskState::Downloading, 3));
        app.selected = 0;
        // 列表聚焦（默认）：↓ 选任务，明细选择不动
        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.selected, 1, "列表聚焦 ↓ 选任务");
        assert_eq!(app.conns_sel, 0, "明细选择不动");
        // 详情聚焦：↑↓ 无操作
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
        // v1.30：B 不限焦点面板（列表聚焦也响应）
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
        // 已暂停的 HTTP 任务：状态门槛先于协议判断（FR-01-97 ②）
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

    /// v1.30/D37：Ctrl 组合键方式整体退役——Ctrl+↑↓/Ctrl+B 不再作用于明细，
    /// Ctrl+C 退出优先级不变
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

    /// v1.30/FR-01-80：Esc 主界面无操作（仅用于关闭浮层）
    #[tokio::test]
    async fn esc_is_noop_on_main_view() {
        let mut app = make_app("esc");
        app.tasks
            .push(task_with_conns(1, "e.bin", TaskState::Downloading, 3));
        app.selected = 0;
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.quit, "Esc 主界面不退出");
        assert!(!app.help_open && !app.pieces_open && app.dialog.is_none());
        // q 仍退出
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
        // 切换选中任务（tick 钳制臂负责复位）
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
    //! v1.30 页签键与面板焦点键单测：Tab/BackTab 移动面板焦点（不切页签）、
    //! ]/[/T（含 }/{）切页签且视口跟随。

    use super::*;
    use crate::model::TaskState;

    /// 夹具单源委托（app 树 App 夹具族收口 testutil，architect v153）
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
        // ] 右切
        app.on_key(KeyCode::Char(']'), KeyModifiers::NONE);
        assert_eq!(app.filter, 1, "] 到已完成页签");
        assert_eq!(app.focus, Panel::List, "页签键不动面板焦点");
        // } 等效 ]
        app.on_key(KeyCode::Char('}'), KeyModifiers::NONE);
        assert_eq!(app.filter, 0, "}} 环回");
        // [ 左切（环形回绕）
        app.on_key(KeyCode::Char('['), KeyModifiers::NONE);
        assert_eq!(app.filter, 1, r#"[ 反向环绕"#);
        // { 等效 [
        app.on_key(KeyCode::Char('{'), KeyModifiers::NONE);
        assert_eq!(app.filter, 0);
        // T/t 循环（与 ] 等效）
        app.on_key(KeyCode::Char('T'), KeyModifiers::NONE);
        assert_eq!(app.filter, 1, "T 循环切换");
        app.on_key(KeyCode::Char('t'), KeyModifiers::NONE);
        assert_eq!(app.filter, 0, "t 同效");
        app.shutdown().await;
    }
}
