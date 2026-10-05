//! keys — 键盘事件：全局键路由与对话框逐键处理

use crossterm::event::{KeyCode, KeyModifiers};

use super::{App, FILTERS};

impl App {
    /// 键盘事件入口
    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) {
            if code == KeyCode::Char('c') {
                self.quit = true;
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
            KeyCode::Char('g') | KeyCode::Char('G') => self.show_chart = !self.show_chart,
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
