//! dialog_keys — 对话框逐键处理（导航/Enter/退格/字符输入与下拉选择）

use crossterm::event::KeyCode;

use super::{Dialog, DialogKind};
use crate::app::CHECKSUM_ALGOS;

impl super::App {
    pub(super) fn on_dialog_key(&mut self, code: KeyCode) {
        let (kind, focus) = match &self.dialog {
            Some(d) => (d.kind, d.focus),
            None => return,
        };
        let nfocus = match kind {
            DialogKind::Add => 8,
            DialogKind::Modify => 6,
            DialogKind::Delete => 3,
        };
        // 校验算法 / 代理下拉框展开时：整块交给下拉键处理（消费所有键）
        if self.dialog.as_ref().is_some_and(|d| d.ck_open) {
            if let Some(d) = self.dialog.as_mut() {
                dropdown_open_key(d, code);
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
            KeyCode::Tab | KeyCode::Down | KeyCode::Right => {
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = (d.focus + 1) % nfocus;
                }
            }
            KeyCode::BackTab | KeyCode::Up | KeyCode::Left => {
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = (d.focus + nfocus - 1) % nfocus;
                }
            }
            KeyCode::Enter => match kind {
                DialogKind::Add => self.add_dialog_enter(focus),
                DialogKind::Modify => self.modify_dialog_enter(focus),
                DialogKind::Delete => self.dlg_activate_delete(focus),
            },
            KeyCode::Backspace => {
                if let Some(d) = self.dialog.as_mut() {
                    text_backspace(kind, d, focus);
                }
            }
            KeyCode::Char(c) => match kind {
                DialogKind::Add => self.add_dialog_char(focus, c),
                DialogKind::Modify => self.modify_dialog_char(focus, c),
                DialogKind::Delete => match c {
                    '1' => self.dlg_activate_delete(0),
                    '2' => self.dlg_activate_delete(1),
                    '3' | ' ' => self.dlg_activate_delete(2),
                    _ => {}
                },
            },
            _ => {}
        }
    }

    /// Add 对话框 Enter：算法/代理行展开下拉、取消按钮关闭、其余确认
    fn add_dialog_enter(&mut self, focus: usize) {
        if focus == 3 {
            let d = self.dialog.as_mut().unwrap();
            d.ck_open = true;
            d.ck_sel = d.ck_type;
        } else if focus == 5 {
            let d = self.dialog.as_mut().unwrap();
            d.proxy_open = true;
        } else if focus == 7 {
            self.dialog = None;
        } else {
            self.dlg_confirm_add();
        }
    }

    /// Modify 对话框 Enter（v1.5/FR-01-87）：0=并发 1=算法 2=校验码 3=代理
    /// 4=确认 5=取消
    fn modify_dialog_enter(&mut self, focus: usize) {
        if focus == 1 {
            let d = self.dialog.as_mut().unwrap();
            d.ck_open = true;
            d.ck_sel = d.ck_type;
        } else if focus == 3 {
            let d = self.dialog.as_mut().unwrap();
            d.proxy_open = true;
        } else if focus == 5 {
            self.dialog = None;
        } else {
            self.dlg_confirm_modify();
        }
    }

    /// Add 对话框字符输入：0-2 文本字段、3 空格展开算法下拉、4 校验码、
    /// 5 空格展开代理下拉、6/7 空格激活按钮
    fn add_dialog_char(&mut self, focus: usize, c: char) {
        if focus < 3 {
            if !c.is_control() {
                if let Some(d) = self.dialog.as_mut() {
                    text_char(kind_add(), d, focus, c);
                }
            }
        } else if focus == 3 {
            if c == ' ' {
                let d = self.dialog.as_mut().unwrap();
                d.ck_open = true;
                d.ck_sel = d.ck_type;
            }
        } else if focus == 4 {
            // 校验码：仅十六进制，最多 128 位（SHA-512）
            if c.is_ascii_hexdigit() && self.dialog.as_ref().unwrap().ck_value.chars().count() < 128
            {
                self.dialog.as_mut().unwrap().ck_value.push(c);
            }
        } else if c == ' ' {
            if focus == 5 {
                let d = self.dialog.as_mut().unwrap();
                d.proxy_open = true;
            } else {
                self.dlg_activate_add(focus);
            }
        }
    }

    /// Modify 对话框字符输入（v1.5/FR-01-87）：0 并发数字、1 空格展开算法
    /// 下拉、2 校验码、3 空格展开代理下拉、4/5 空格激活按钮
    fn modify_dialog_char(&mut self, focus: usize, c: char) {
        if focus == 0 {
            if c.is_ascii_digit() && self.dialog.as_ref().unwrap().conns.chars().count() < 2 {
                let d = self.dialog.as_mut().unwrap();
                d.conns.push(c);
                d.conns_edited = true;
            }
        } else if focus == 1 {
            if c == ' ' {
                let d = self.dialog.as_mut().unwrap();
                d.ck_open = true;
                d.ck_sel = d.ck_type;
            }
        } else if focus == 2 {
            if c.is_ascii_hexdigit() && self.dialog.as_ref().unwrap().ck_value.chars().count() < 128
            {
                self.dialog.as_mut().unwrap().ck_value.push(c);
            }
        } else if c == ' ' {
            if focus == 3 {
                let d = self.dialog.as_mut().unwrap();
                d.proxy_open = true;
            } else {
                self.dlg_activate_modify(focus);
            }
        }
    }
}

/// Add 布局标记（text_char 复用：Add 与 Modify 共用文本字段语义，焦点映射
/// 由 kind 区分）
const fn kind_add() -> DialogKind {
    DialogKind::Add
}

/// 下拉框展开态逐键处理（Up/Down/Home/End 选择，Enter/Esc 关闭，
/// Tab/BackTab 关闭并跳转焦点，其余键关闭下拉）。消费所有按键。
fn dropdown_open_key(d: &mut Dialog, code: KeyCode) {
    match code {
        KeyCode::Up => {
            d.ck_sel = (d.ck_sel + CHECKSUM_ALGOS.len() - 1) % CHECKSUM_ALGOS.len();
            d.ck_type = d.ck_sel;
        }
        KeyCode::Down => {
            d.ck_sel = (d.ck_sel + 1) % CHECKSUM_ALGOS.len();
            d.ck_type = d.ck_sel;
        }
        KeyCode::Home => {
            d.ck_sel = 0;
            d.ck_type = 0;
        }
        KeyCode::End => {
            d.ck_sel = CHECKSUM_ALGOS.len() - 1;
            d.ck_type = d.ck_sel;
        }
        KeyCode::Enter | KeyCode::Esc => d.ck_open = false,
        KeyCode::Tab | KeyCode::BackTab => {
            d.ck_open = false;
            d.focus = if code == KeyCode::Tab { 4 } else { 2 };
        }
        _ => d.ck_open = false,
    }
}

/// 代理下拉框展开态逐键处理（v1.5/FR-01-86；Up/Down/Home/End 选择，
/// Enter/Esc 关闭，Tab/BackTab 关闭并跳转焦点，其余键关闭）。消费所有按键。
fn proxy_dropdown_open_key(d: &mut Dialog, n: usize, code: KeyCode) {
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
            d.focus = if d.kind == DialogKind::Add { 6 } else { 4 };
        }
        KeyCode::BackTab => {
            d.proxy_open = false;
            d.focus = if d.kind == DialogKind::Add { 4 } else { 2 };
        }
        _ => d.proxy_open = false,
    }
}

/// 文本字段退格（kind 感知焦点映射：Add 0=URL 1=目录 2=并发 4=校验码；
/// Modify 0=并发 2=校验码）
fn text_backspace(kind: DialogKind, d: &mut Dialog, focus: usize) -> bool {
    match (kind, focus) {
        (DialogKind::Add, 0) => {
            d.url.pop();
            true
        }
        (DialogKind::Add, 1) => {
            d.dir.pop();
            true
        }
        (DialogKind::Add, 2) | (DialogKind::Modify, 0) => {
            d.conns.pop();
            d.conns_edited = true;
            true
        }
        (DialogKind::Add, 4) | (DialogKind::Modify, 2) => {
            d.ck_value.pop();
            true
        }
        _ => false,
    }
}

/// 文本字段字符输入（kind 感知：Add 0=URL 1=目录 2=并发；Modify 0=并发。
/// URL/目录 ≤300 字符；并发仅 2 位数字）
fn text_char(kind: DialogKind, d: &mut Dialog, focus: usize, c: char) -> bool {
    match (kind, focus) {
        (DialogKind::Add, 0) => {
            if d.url.chars().count() < 300 {
                d.url.push(c);
            }
            true
        }
        (DialogKind::Add, 1) => {
            if d.dir.chars().count() < 300 {
                d.dir.push(c);
            }
            true
        }
        (DialogKind::Add, 2) | (DialogKind::Modify, 0) => {
            if c.is_ascii_digit() && d.conns.chars().count() < 2 {
                d.conns.push(c);
                d.conns_edited = true;
            }
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod dialog_key_tests {
    use super::*;
    use crate::app::{Dialog, DialogKind};
    use crate::model::config::Config;
    use crate::model::Protocol;

    fn make_app(tag: &str) -> crate::app::App {
        let dir = std::env::temp_dir().join(format!("ezr-dlgkey-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        crate::app::App::new(Config::default(), reg)
    }

    fn add_dlg() -> Dialog {
        let mut d = Dialog {
            kind: DialogKind::Add,
            url: String::new(),
            dir: String::new(),
            conns: String::new(),
            conns_edited: false,
            ck_type: 3,
            ck_value: String::new(),
            ck_open: false,
            ck_sel: 3,
            proxy_sel: 0,
            proxy_open: false,
            focus: 0,
            task_name: String::new(),
            task_id: None,
        };
        d.ck_sel = 3;
        d
    }

    fn del_dlg(name: &str) -> Dialog {
        Dialog {
            kind: DialogKind::Delete,
            url: String::new(),
            dir: String::new(),
            conns: String::new(),
            conns_edited: false,
            ck_type: 0,
            ck_value: String::new(),
            ck_open: false,
            ck_sel: 0,
            proxy_sel: 0,
            proxy_open: false,
            focus: 0,
            task_name: name.to_string(),
            task_id: None,
        }
    }

    #[test]
    fn dropdown_open_key_navigation_and_close() {
        let mut d = add_dlg();
        d.ck_open = true;
        d.ck_sel = 0;
        dropdown_open_key(&mut d, KeyCode::Up);
        assert_eq!(d.ck_sel, CHECKSUM_ALGOS.len() - 1, "Up 环绕到末尾");
        assert_eq!(d.ck_type, d.ck_sel);
        dropdown_open_key(&mut d, KeyCode::Down);
        assert_eq!(d.ck_sel, 0, "Down 环绕回首");
        dropdown_open_key(&mut d, KeyCode::End);
        assert_eq!(d.ck_sel, CHECKSUM_ALGOS.len() - 1);
        dropdown_open_key(&mut d, KeyCode::Home);
        assert_eq!(d.ck_sel, 0);
        dropdown_open_key(&mut d, KeyCode::Enter);
        assert!(!d.ck_open);
        // Tab：关闭下拉并跳到校验码字段
        d.ck_open = true;
        dropdown_open_key(&mut d, KeyCode::Tab);
        assert!(!d.ck_open);
        assert_eq!(d.focus, 4);
        d.ck_open = true;
        dropdown_open_key(&mut d, KeyCode::BackTab);
        assert_eq!(d.focus, 2);
        // 其余键：仅关闭
        d.ck_open = true;
        dropdown_open_key(&mut d, KeyCode::Char('x'));
        assert!(!d.ck_open);
    }

    #[test]
    fn text_backspace_fields_and_gates() {
        let mut d = add_dlg();
        d.url.push_str("abc");
        assert!(text_backspace(DialogKind::Add, &mut d, 0));
        assert_eq!(d.url, "ab");
        d.dir.push_str("xy");
        assert!(text_backspace(DialogKind::Add, &mut d, 1));
        assert_eq!(d.dir, "x");
        d.conns.push_str("12");
        assert!(text_backspace(DialogKind::Add, &mut d, 2));
        assert_eq!(d.conns, "1");
        assert!(d.conns_edited);
        d.ck_value.push_str("ff");
        assert!(text_backspace(DialogKind::Add, &mut d, 4));
        assert_eq!(d.ck_value, "f");
        // 焦点 3（算法下拉）/5/6（按钮）不消费
        assert!(!text_backspace(DialogKind::Add, &mut d, 3));
        assert!(!text_backspace(DialogKind::Add, &mut d, 5));
    }

    #[test]
    fn text_char_caps_and_filters() {
        let mut d = add_dlg();
        for c in "abcdefghij".chars() {
            text_char(DialogKind::Add, &mut d, 0, c);
        }
        assert_eq!(d.url.chars().count(), 10);
        // 300 上限
        for c in ('a'..='z').cycle().take(400) {
            text_char(DialogKind::Add, &mut d, 0, c);
        }
        assert_eq!(d.url.chars().count(), 300);
        // 并发：仅数字、2 位
        text_char(DialogKind::Add, &mut d, 2, '5');
        text_char(DialogKind::Add, &mut d, 2, 'x'); // 非数字拒绝
        text_char(DialogKind::Add, &mut d, 2, '6');
        text_char(DialogKind::Add, &mut d, 2, '7'); // 第 3 位拒绝
        assert_eq!(d.conns, "56");
        assert!(d.conns_edited);
    }

    /// 确认添加主链路（FR-01-01/03/04/05/16）：空 URL/非法协议/校验码位数不符
    /// 逐一拒绝并保持对话框；合法输入建任务并关闭；重复任务拒绝；取消按钮直接关闭
    #[tokio::test]
    async fn confirm_add_validates_then_creates() {
        let mut app = make_app("confirm");
        app.dialog = Some(add_dlg());

        // 空 URL → 拒绝
        app.dlg_confirm_add();
        assert!(app.dialog.is_some(), "空 URL 拒绝，对话框保持");
        // 非 http(s) → 拒绝
        app.dialog.as_mut().unwrap().url = "ftp://x/f.bin".to_string();
        app.dlg_confirm_add();
        assert!(app.dialog.is_some(), "非法协议拒绝");
        assert!(app.tasks.is_empty());
        // 校验码位数不符 → 聚焦回校验码字段（focus 4）
        let d = app.dialog.as_mut().unwrap();
        d.url = "http://example.com/a.bin".to_string();
        d.ck_value = "abc".to_string();
        app.dlg_confirm_add();
        assert!(app.dialog.is_some(), "校验码位数不符拒绝");
        assert_eq!(app.dialog.as_ref().unwrap().focus, 4, "聚焦回校验码");
        assert!(app.tasks.is_empty());
        // 合法完整输入（并发显式 9）→ 建任务、关闭对话框、页签回下载视图
        let d = app.dialog.as_mut().unwrap();
        d.ck_value.clear();
        d.conns = "9".to_string();
        app.dlg_confirm_add();
        assert!(app.dialog.is_none(), "确认后关闭对话框");
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(app.tasks[0].name, "a.bin");
        assert_eq!(app.tasks[0].concurrency, 9);
        // 重复任务（同 URL 同目录）→ 拒绝并保持对话框
        app.dialog = Some(add_dlg());
        app.dialog.as_mut().unwrap().url = "http://example.com/a.bin".to_string();
        app.dlg_confirm_add();
        assert!(app.dialog.is_some(), "重复任务拒绝");
        assert_eq!(app.tasks.len(), 1);
        // 取消按钮（btn 7）→ 直接关闭
        app.dlg_activate_add(7);
        assert!(app.dialog.is_none(), "取消按钮关闭对话框");
        app.shutdown().await;
    }

    /// 断点自动接续（FR-01-26）：保存目录存在同 URL 的 sidecar → 沿用推导原名，
    /// toast 提示接续（不再追加去重序号）
    #[tokio::test]
    async fn confirm_add_resumes_from_existing_sidecar() {
        let dir = std::env::temp_dir().join(format!("ezr-dlgkey-resume-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let mut app = make_app("resume");
        app.dialog = Some(add_dlg());
        let d = app.dialog.as_mut().unwrap();
        d.url = "http://example.com/r.bin".to_string();
        d.dir = dir.to_string_lossy().into_owned();
        // 预置同 URL sidecar（断点视图：模拟既有下载残留）
        let sc = crate::model::sidecar::Sidecar::build(
            "http://example.com/r.bin",
            &crate::model::consistency::ServerStamp {
                final_url: None,
                etag: None,
                last_modified: None,
                size: Some(1000),
            },
            1000,
            1024 * 1024,
            &[0],
            false,
            None,
            crate::model::sidecar::SidecarTask {
                id: 99,
                added_at: 0,
                save_dir: dir.to_string_lossy().into_owned(),
                concurrency: 4,
                protocol: Protocol::Http,
            },
        );
        sc.save(&dir.join("r.bin.ezr").to_string_lossy()).ok();
        app.dlg_confirm_add();
        assert!(app.dialog.is_none(), "确认后关闭");
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(app.tasks[0].name, "r.bin", "断点接续沿用原名");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 对话框键盘导航与 Delete 对话框激活：焦点环绕、Esc 关闭、
    /// 数字 1/3 触发删除/取消（FR-01-25 删除三选）
    #[tokio::test]
    async fn dialog_key_navigates_and_activates_delete() {
        let mut app = make_app("dnav");
        app.dialog = Some(add_dlg());

        // 焦点前进/后退与环绕（Add nfocus=8：v1.5 增代理行）
        app.on_dialog_key(KeyCode::Down); // 1
        app.on_dialog_key(KeyCode::Right); // 2
        app.on_dialog_key(KeyCode::Tab); // 3
        app.on_dialog_key(KeyCode::Up); // 2
        app.on_dialog_key(KeyCode::Left); // 1
        app.on_dialog_key(KeyCode::BackTab); // 0
        app.on_dialog_key(KeyCode::Up); // 环绕到 7
        assert_eq!(app.dialog.as_ref().unwrap().focus, 7, "Up 环绕到末位");

        // Esc 关闭
        app.on_dialog_key(KeyCode::Esc);
        assert!(app.dialog.is_none(), "Esc 关闭对话框");

        // Delete 对话框：数字 3 = 取消按钮 → 关闭且任务保留
        app.tasks.push(crate::model::Task::new_queued(
            7,
            "del.bin".to_string(),
            Protocol::Http,
            "http://example.com/del.bin".to_string(),
            "/tmp".to_string(),
            1024 * 1024,
            4,
            3,
            None,
            crate::model::config::ProxyChoice::Direct,
            0,
        ));
        app.dialog = Some(del_dlg("del.bin"));
        app.on_dialog_key(KeyCode::Char('3'));
        assert!(app.dialog.is_none(), "取消按钮关闭");
        assert_eq!(app.tasks.len(), 1, "取消不删任务");

        // 数字 1 = 仅删除任务：任务移除
        app.dialog = Some(del_dlg("del.bin"));
        app.on_dialog_key(KeyCode::Char('1'));
        assert!(app.dialog.is_none(), "删除后关闭");
        assert!(app.tasks.is_empty(), "任务已移除");
        app.shutdown().await;
    }
}
