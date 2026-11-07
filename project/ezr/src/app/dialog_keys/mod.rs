use crossterm::event::KeyCode;

mod routing;

use routing::kind_add;
pub(crate) use routing::{
    dropdown_open_key, insert_conns_digit, insert_limit, normalize_speed_display,
    proxy_dropdown_open_key, push_hex_capped, speed_unit_mult, text_backspace, text_char,
    text_field_at, TextField, CAP_LIMIT,
};

use super::{Dialog, DialogKind};

pub(super) const CAP_TEXT: usize = 300;
pub(super) const CAP_CONNS_DIGITS: usize = 2;
pub(super) const CAP_CK_HEX: usize = 128;

impl super::App {
    pub(super) fn sync_default_conns_after_url_change(&mut self) {
        let Some(d) = self.dialog.as_mut() else {
            return;
        };
        if d.conns_edited {
            return;
        }
        let proto = crate::model::protocol_of_url(d.url.value.trim());
        let def = if proto.is_bt() {
            20
        } else {
            self.cfg.http_concurrency
        };
        d.conns.set(def.to_string());
    }

    pub(super) fn on_dialog_key(&mut self, code: KeyCode, mods: crossterm::event::KeyModifiers) {
        let (kind, focus) = match &self.dialog {
            Some(d) => (d.kind, d.focus),
            None => return,
        };
        let nfocus = match kind {
            DialogKind::Add => 10,
            DialogKind::Modify => 7,
            DialogKind::Delete => 3,
            DialogKind::AddBt => 11,
            DialogKind::ModifyBt => 8,
        };
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
            KeyCode::Tab | KeyCode::BackTab => {
                if let Some(d) = self.dialog.as_mut() {
                    let step = if code == KeyCode::Tab { 1 } else { nfocus - 1 };
                    d.focus_field((d.focus + step) % nfocus);
                }
            }
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                let bt_special = matches!(kind, DialogKind::AddBt | DialogKind::ModifyBt);
                if bt_special
                    && self
                        .dialog
                        .as_ref()
                        .is_some_and(|d| d.focus == Dialog::bt_list_focus(kind))
                {
                    if let Some(d) = self.dialog.as_mut() {
                        match code {
                            KeyCode::Up => d.bt_list_move(-1),
                            KeyCode::Down => d.bt_list_move(1),
                            KeyCode::Left => d.bt_expand_cursor(false),
                            KeyCode::Right => d.bt_expand_cursor(true),
                            _ => {}
                        }
                    }
                    return;
                }
                if bt_special
                    && self
                        .dialog
                        .as_ref()
                        .is_some_and(|d| d.trackers_focus() == Some(d.focus))
                {
                    if let Some(d) = self.dialog.as_mut() {
                        match code {
                            KeyCode::Up => d.trackers.up(),
                            KeyCode::Down => d.trackers.down(),
                            KeyCode::Left => d.trackers.left(),
                            KeyCode::Right => d.trackers.right(),
                            _ => {}
                        }
                    }
                    return;
                }
                let on_text = self
                    .dialog
                    .as_ref()
                    .is_some_and(|d| text_field_at(d.kind, d.focus).is_some());
                if let Some(d) = self.dialog.as_mut() {
                    if on_text {
                        let field = text_field_at(d.kind, d.focus).unwrap();
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
            KeyCode::Enter
                if mods.contains(crossterm::event::KeyModifiers::ALT)
                    || mods.contains(crossterm::event::KeyModifiers::SHIFT) =>
            {
                if matches!(kind, DialogKind::AddBt | DialogKind::ModifyBt)
                    && self
                        .dialog
                        .as_ref()
                        .is_some_and(|d| d.trackers_focus() == Some(d.focus))
                {
                    self.bt_trackers_newline();
                }
            }
            KeyCode::Enter => match kind {
                DialogKind::Add => self.add_dialog_enter(focus),
                DialogKind::Modify => self.modify_dialog_enter(focus),
                DialogKind::Delete => self.dlg_activate_delete(focus),
                DialogKind::AddBt => self.add_bt_dialog_enter(focus),
                DialogKind::ModifyBt => self.modify_bt_dialog_enter(focus),
            },
            KeyCode::Backspace => {
                if let Some(d) = self.dialog.as_mut() {
                    text_backspace(kind, d, focus);
                }
                if kind == DialogKind::Add && focus == 0 {
                    self.sync_default_conns_after_url_change();
                }
            }
            KeyCode::Delete => {
                if let Some(d) = self.dialog.as_mut() {
                    if d.trackers_focus() == Some(d.focus) {
                        d.trackers.delete();
                    }
                }
            }
            KeyCode::Home => {
                if let Some(d) = self.dialog.as_mut() {
                    if d.trackers_focus() == Some(d.focus) {
                        d.trackers.to_first();
                    }
                }
            }
            KeyCode::End => {
                if let Some(d) = self.dialog.as_mut() {
                    if d.trackers_focus() == Some(d.focus) {
                        d.trackers.to_last();
                    }
                }
            }
            KeyCode::Char(c) => match kind {
                DialogKind::Add => self.add_dialog_char(focus, c),
                DialogKind::Modify => self.modify_dialog_char(focus, c),
                DialogKind::AddBt => self.add_bt_dialog_char(focus, c),
                DialogKind::ModifyBt => self.modify_bt_dialog_char(focus, c),
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

    fn add_dialog_enter(&mut self, focus: usize) {
        if focus == 4 {
            let d = self.dialog.as_mut().unwrap();
            d.ck_open = true;
            d.ck_sel = d.ck_type;
        } else if focus == 6 {
            let d = self.dialog.as_mut().unwrap();
            d.proxy_open = true;
        } else if focus == 8 {
            self.dlg_add_only();
        } else if focus == 9 {
            self.dialog = None;
        } else {
            self.dlg_confirm_add();
        }
    }

    fn modify_dialog_enter(&mut self, focus: usize) {
        if focus == 2 {
            let d = self.dialog.as_mut().unwrap();
            d.ck_open = true;
            d.ck_sel = d.ck_type;
        } else if focus == 4 {
            let d = self.dialog.as_mut().unwrap();
            d.proxy_open = true;
        } else if focus == 6 {
            self.dialog = None;
        } else {
            self.dlg_confirm_modify();
        }
    }

    fn add_dialog_char(&mut self, focus: usize, c: char) {
        if focus < 4 {
            if !c.is_control() {
                if let Some(d) = self.dialog.as_mut() {
                    text_char(kind_add(), d, focus, c);
                }
            }
            if focus == 0 {
                self.sync_default_conns_after_url_change();
            }
        } else if focus == 4 {
            if c == ' ' {
                let d = self.dialog.as_mut().unwrap();
                d.ck_open = true;
                d.ck_sel = d.ck_type;
            }
        } else if focus == 5 {
            if let Some(d) = self.dialog.as_mut() {
                push_hex_capped(d, c);
            }
        } else if c == ' ' {
            if focus == 6 {
                let d = self.dialog.as_mut().unwrap();
                d.proxy_open = true;
            } else {
                self.dlg_activate_add(focus);
            }
        }
    }

    fn modify_dialog_char(&mut self, focus: usize, c: char) {
        if focus < 2 {
            if !c.is_control() {
                if let Some(d) = self.dialog.as_mut() {
                    text_char(DialogKind::Modify, d, focus, c);
                }
            }
        } else if focus == 2 {
            if c == ' ' {
                let d = self.dialog.as_mut().unwrap();
                d.ck_open = true;
                d.ck_sel = d.ck_type;
            }
        } else if focus == 3 {
            if let Some(d) = self.dialog.as_mut() {
                push_hex_capped(d, c);
            }
        } else if c == ' ' {
            if focus == 4 {
                let d = self.dialog.as_mut().unwrap();
                d.proxy_open = true;
            } else {
                self.dlg_activate_modify(focus);
            }
        }
    }
}

#[cfg(test)]
mod dialog_key_tests {
    use super::*;
    use crate::app::{Dialog, DialogKind, CHECKSUM_ALGOS};
    use crate::model::checksum::CK_NONE;
    use crate::model::Protocol;

    fn make_app(tag: &str) -> crate::app::App {
        crate::app::testutil::make_app_in("ezr-dlgkey", tag)
    }

    fn add_dlg() -> Dialog {
        crate::app::testutil::dialog(DialogKind::Add, 0)
    }

    fn del_dlg(name: &str) -> Dialog {
        let mut d = crate::app::testutil::dialog(DialogKind::Delete, 0);
        d.ck_type = 0;
        d.ck_sel = 0;
        d.task_name = name.to_string();
        d
    }

    #[test]
    fn dropdown_open_key_navigation_and_close() {
        let mut d = add_dlg();
        d.ck_open = true;
        d.ck_sel = 0;
        dropdown_open_key(&mut d, KeyCode::Up);
        assert_eq!(d.ck_sel, CHECKSUM_ALGOS.len(), "Up 环绕到末尾（8 项）");
        assert_eq!(d.ck_type, d.ck_sel);
        d.ck_value.insert("ff", 128);
        dropdown_open_key(&mut d, KeyCode::Home);
        assert_eq!(d.ck_sel, CK_NONE, "Home 回首（NONE）");
        assert!(d.ck_value.is_empty(), "选中 NONE 实时清空校验码");
        dropdown_open_key(&mut d, KeyCode::Down);
        assert_eq!(d.ck_sel, 1, "Down 环绕回首后步进");
        dropdown_open_key(&mut d, KeyCode::End);
        assert_eq!(d.ck_sel, CHECKSUM_ALGOS.len());
        dropdown_open_key(&mut d, KeyCode::Home);
        assert_eq!(d.ck_sel, CK_NONE);
        dropdown_open_key(&mut d, KeyCode::Enter);
        assert!(!d.ck_open);
        d.ck_open = true;
        dropdown_open_key(&mut d, KeyCode::Tab);
        assert!(!d.ck_open);
        assert_eq!(d.focus, 5);
        d.ck_open = true;
        dropdown_open_key(&mut d, KeyCode::BackTab);
        assert_eq!(d.focus, 3);
        d.ck_open = true;
        dropdown_open_key(&mut d, KeyCode::Char('x'));
        assert!(!d.ck_open);
    }

    #[test]
    fn text_backspace_fields_and_gates() {
        let mut d = add_dlg();
        d.url.insert("abc", CAP_TEXT);
        assert!(text_backspace(DialogKind::Add, &mut d, 0));
        assert_eq!(d.url.value, "ab");
        assert_eq!(d.url.cursor, 2, "退格后光标随迁");
        d.url.set_cursor(1);
        assert!(text_backspace(DialogKind::Add, &mut d, 0));
        assert_eq!(d.url.value, "b");
        d.url.set_cursor(0);
        assert!(text_backspace(DialogKind::Add, &mut d, 0));
        assert_eq!(d.url.value, "b");
        d.dir.insert("xy", CAP_TEXT);
        assert!(text_backspace(DialogKind::Add, &mut d, 1));
        assert_eq!(d.dir.value, "x");
        d.conns.insert("12", CAP_CONNS_DIGITS);
        assert!(text_backspace(DialogKind::Add, &mut d, 2));
        assert_eq!(d.conns.value, "1");
        assert!(d.conns_edited);
        d.ck_value.insert("ff", CAP_CK_HEX);
        assert!(text_backspace(DialogKind::Add, &mut d, 5));
        assert_eq!(d.ck_value.value, "f");
        d.down_limit.insert("2 MB/s", 16);
        assert!(text_backspace(DialogKind::Add, &mut d, 3));
        assert_eq!(d.down_limit.value, "2 MB/", "下载限速退格");
        let mut bt = crate::app::testutil::dialog(DialogKind::AddBt, 0);
        bt.up_limit.insert("500 KB/s", 16);
        assert!(text_backspace(DialogKind::AddBt, &mut bt, 5));
        assert_eq!(bt.up_limit.value, "500 KB/", "上传限速退格（BT 框保留）");
        assert!(!text_backspace(DialogKind::Add, &mut d, 4));
    }

    #[test]
    fn text_char_caps_and_filters() {
        let mut d = add_dlg();
        for c in "abcdefghij".chars() {
            text_char(DialogKind::Add, &mut d, 0, c);
        }
        assert_eq!(d.url.len_chars(), 10);
        for c in ('a'..='z').cycle().take(400) {
            text_char(DialogKind::Add, &mut d, 0, c);
        }
        assert_eq!(d.url.len_chars(), 300);
        text_char(DialogKind::Add, &mut d, 2, '5');
        text_char(DialogKind::Add, &mut d, 2, 'x');
        text_char(DialogKind::Add, &mut d, 2, '6');
        assert_eq!(d.conns.value, "56", "数字接收、字母拒绝、容量 2 封顶");
        assert!(d.conns_edited);
        text_char(DialogKind::Add, &mut d, 2, '7');
        assert_eq!(d.conns.value, "7", "满容量续入重启录入（bug21）");
    }

    #[test]
    fn insert_conns_digit_restarts_full_prefill() {
        let mut bt = crate::app::testutil::dialog(DialogKind::AddBt, 0);
        bt.conns.set("20");
        insert_conns_digit(&mut bt, '3');
        assert_eq!(bt.conns.value, "3", "BT 预填 20 满容量后键入重启（bug①）");
        assert!(bt.conns_edited);
        insert_conns_digit(&mut bt, '5');
        assert_eq!(bt.conns.value, "35");
        let mut m = crate::app::testutil::dialog(DialogKind::ModifyBt, 0);
        m.conns.set("20");
        insert_conns_digit(&mut m, '8');
        assert_eq!(m.conns.value, "8", "ModifyBt 并发满容量重启");
        insert_conns_digit(&mut m, 'x');
        assert_eq!(m.conns.value, "8", "非数字拒收");
        let mut mid = crate::app::testutil::dialog(DialogKind::AddBt, 0);
        mid.conns.set("20");
        mid.conns.set_cursor(1);
        insert_conns_digit(&mut mid, '9');
        assert_eq!(mid.conns.value, "20", "满容量非行尾不重启不插入");
    }

    #[test]
    fn typing_full_canonical_form_with_leading_zero_prefill_stays_parseable() {
        use super::super::dialog_keys::insert_limit;
        let mut d = add_dlg();
        d.down_limit.set("0");
        d.down_limit.backspace();
        for c in "256 KiB/s".chars() {
            insert_limit(&mut d.down_limit, c);
            eprintln!("after {c:?}: {:?}", d.down_limit.value);
        }
        let v = d.down_limit.value.clone();
        assert!(
            super::super::dialogs::parse_speed_limit(&v).is_some(),
            "退格清零后键入『256 KiB/s』应可解析，实际 = {v:?}"
        );
        assert_eq!(v, "256 KiB/s");
        d.down_limit.clear();
        for c in "512 KiB/s".chars() {
            insert_limit(&mut d.down_limit, c);
        }
        let v = d.down_limit.value.clone();
        assert!(
            super::super::dialogs::parse_speed_limit(&v).is_some(),
            "空字段键入『512 KiB/s』应可解析，实际 = {v:?}"
        );
        assert_eq!(v, "512 KiB/s");
        d.up_limit.set("0");
        d.up_limit.backspace();
        for c in "2 MiB/s".chars() {
            insert_limit(&mut d.up_limit, c);
        }
        let v = d.up_limit.value.clone();
        assert!(
            super::super::dialogs::parse_speed_limit(&v).is_some(),
            "键入『2 MiB/s』应可解析，实际 = {v:?}"
        );
    }

    #[test]
    fn insert_limit_canonicalizes_binary_units() {
        use super::super::dialog_keys::insert_limit;
        let mut d = add_dlg();
        for c in "512k".chars() {
            insert_limit(&mut d.down_limit, c);
        }
        assert_eq!(d.down_limit.value, "512 KiB/s", "512k → 512 KiB/s");
        for c in "b".chars() {
            insert_limit(&mut d.down_limit, c);
        }
        assert_eq!(d.down_limit.value, "512 KiB/s", "后缀 b 吸收（512kb 同义）");
        for c in "/s".chars() {
            insert_limit(&mut d.down_limit, c);
        }
        assert_eq!(d.down_limit.value, "512 KiB/s", "/s 吸收");
        d.down_limit.clear();
        for c in "1 M".chars() {
            insert_limit(&mut d.up_limit, c);
        }
        assert_eq!(d.up_limit.value, "1 MiB/s", "1 M → 1 MiB/s");
        d.up_limit.clear();
        for c in "512 kb".chars() {
            insert_limit(&mut d.down_limit, c);
        }
        assert_eq!(d.down_limit.value, "512 KiB/s", "512 kb → 512 KiB/s");
        d.down_limit.clear();
        for c in "2048".chars() {
            insert_limit(&mut d.down_limit, c);
        }
        assert_eq!(d.down_limit.value, "2048", "纯整数原样保留");
        d.down_limit.backspace();
        assert_eq!(d.down_limit.value, "204", "退格不被规范化回写");
    }

    #[tokio::test]
    async fn confirm_add_validates_then_creates() {
        let mut app = make_app("confirm");
        app.dialog = Some(add_dlg());

        app.dlg_confirm_add();
        assert!(app.dialog.is_some(), "空 URL 拒绝，对话框保持");
        app.dialog.as_mut().unwrap().url.set("ftp://x/f.bin");
        app.dlg_confirm_add();
        assert!(app.dialog.is_some(), "非法协议拒绝");
        assert!(app.tasks.is_empty());
        let d = app.dialog.as_mut().unwrap();
        d.url.set("http://example.com/a.bin");
        d.ck_value.set("abc");
        app.dlg_confirm_add();
        assert!(app.dialog.is_some(), "校验码位数不符拒绝");
        assert_eq!(app.dialog.as_ref().unwrap().focus, 5, "聚焦回校验码");
        assert!(app.tasks.is_empty());
        let d = app.dialog.as_mut().unwrap();
        d.ck_type = CK_NONE;
        d.ck_sel = CK_NONE;
        d.ck_value.clear();
        d.conns.set("9");
        app.dlg_confirm_add();
        assert!(app.dialog.is_none(), "确认后关闭对话框");
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(app.tasks[0].name, "a.bin");
        assert_eq!(app.tasks[0].concurrency, 9);
        app.dialog = Some(add_dlg());
        app.dialog
            .as_mut()
            .unwrap()
            .url
            .set("http://example.com/a.bin");
        app.dlg_confirm_add();
        assert!(app.dialog.is_some(), "重复任务拒绝");
        assert_eq!(app.tasks.len(), 1);
        app.dlg_activate_add(9);
        assert!(app.dialog.is_none(), "取消按钮关闭对话框");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn confirm_add_resumes_from_existing_sidecar() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dlgkey-resume");
        let mut app = make_app("resume");
        app.dialog = Some(add_dlg());
        let d = app.dialog.as_mut().unwrap();
        d.url.set("http://example.com/r.bin");
        d.dir.set(dir.to_string_lossy().into_owned());
        d.ck_type = CK_NONE;
        d.ck_sel = CK_NONE;
        crate::app::testutil::preset_resume_sidecar(&dir);
        app.dlg_confirm_add();
        assert!(app.dialog.is_none(), "确认后关闭");
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(app.tasks[0].name, "r.bin", "断点接续沿用原名");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn dialog_key_navigates_and_activates_delete() {
        let mut app = make_app("dnav");
        app.dialog = Some(add_dlg());

        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::BackTab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::BackTab, crossterm::event::KeyModifiers::NONE);
        assert_eq!(app.dialog.as_ref().unwrap().focus, 1);
        app.on_dialog_key(KeyCode::BackTab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::BackTab, crossterm::event::KeyModifiers::NONE);
        assert_eq!(app.dialog.as_ref().unwrap().focus, 9, "BackTab 环绕到末位");
        app.on_dialog_key(KeyCode::Left, crossterm::event::KeyModifiers::NONE);
        assert_eq!(app.dialog.as_ref().unwrap().focus, 8, "按钮行 Left 后退");
        app.on_dialog_key(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        assert_eq!(app.dialog.as_ref().unwrap().focus, 9, "按钮行 Down 前进");

        app.on_dialog_key(KeyCode::Esc, crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "Esc 关闭对话框");

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
        app.on_dialog_key(KeyCode::Char('3'), crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "取消按钮关闭");
        assert_eq!(app.tasks.len(), 1, "取消不删任务");

        app.dialog = Some(del_dlg("del.bin"));
        app.on_dialog_key(KeyCode::Char('1'), crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "删除后关闭");
        assert!(app.tasks.is_empty(), "任务已移除");
        app.shutdown().await;
    }

    fn modify_dlg() -> Dialog {
        crate::app::testutil::dialog(DialogKind::Modify, 0)
    }

    #[tokio::test]
    async fn modify_dialog_char_routes() {
        let mut app = make_app("mchar");
        app.tasks.push(crate::model::Task::new_queued(
            1,
            "mod.bin".to_string(),
            Protocol::Http,
            "http://example.com/mod.bin".to_string(),
            "/tmp".to_string(),
            1024 * 1024,
            2,
            5,
            None,
            crate::model::config::ProxyChoice::Direct,
            0,
        ));
        app.selected = 0;
        app.on_key(KeyCode::Char('m'), crossterm::event::KeyModifiers::NONE);
        assert_eq!(
            app.dialog.as_ref().unwrap().kind,
            DialogKind::Modify,
            "m 打开修改对话框"
        );
        assert_eq!(
            app.dialog.as_ref().unwrap().ck_type,
            CK_NONE,
            "无校验任务预填 NONE（v1.32）"
        );
        app.dialog.as_mut().unwrap().ck_type = 4;
        app.dialog.as_mut().unwrap().ck_sel = 4;

        app.on_dialog_key(KeyCode::Char('7'), crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char('x'), crossterm::event::KeyModifiers::NONE);
        assert_eq!(app.dialog.as_ref().unwrap().conns.value, "27");
        assert!(app.dialog.as_ref().unwrap().conns_edited, "手动编辑置位");
        app.on_dialog_key(KeyCode::Char('8'), crossterm::event::KeyModifiers::NONE);
        assert_eq!(
            app.dialog.as_ref().unwrap().conns.value,
            "8",
            "满容量续入重启（bug21）"
        );

        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char('2'), crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char('M'), crossterm::event::KeyModifiers::NONE);
        assert_eq!(
            app.dialog.as_ref().unwrap().down_limit.value,
            "2 MiB/s",
            "限速字段接收字符并规范化"
        );

        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char(' '), crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.as_ref().unwrap().ck_open, "空格展开算法下拉");
        app.on_dialog_key(KeyCode::Esc, crossterm::event::KeyModifiers::NONE);
        assert!(!app.dialog.as_ref().unwrap().ck_open, "Esc 收起下拉");

        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char('a'), crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char('z'), crossterm::event::KeyModifiers::NONE);
        assert_eq!(
            app.dialog.as_ref().unwrap().ck_value.value,
            "a",
            "仅十六进制接收"
        );

        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char(' '), crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.as_ref().unwrap().proxy_open, "空格展开代理下拉");
        app.on_dialog_key(KeyCode::Esc, crossterm::event::KeyModifiers::NONE);
        assert!(!app.dialog.as_ref().unwrap().proxy_open);

        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char(' '), crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.is_some(), "非法校验码不生效");
        assert_eq!(app.dialog.as_ref().unwrap().focus, 3, "聚焦回校验码");

        let d = app.dialog.as_mut().unwrap();
        d.ck_type = CK_NONE;
        d.ck_value.clear();
        app.on_dialog_key(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.as_ref().unwrap().ck_value.is_empty());
        app.on_dialog_key(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "确认后关闭");
        assert_eq!(app.tasks[0].concurrency, 8, "并发立即生效");

        app.selected = 0;
        app.on_key(KeyCode::Char('m'), crossterm::event::KeyModifiers::NONE);
        for _ in 0..6 {
            app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        }
        assert_eq!(app.dialog.as_ref().unwrap().focus, 6);
        app.on_dialog_key(KeyCode::Char(' '), crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "取消按钮直接关闭");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn modify_dialog_enter_routes() {
        let mut app = make_app("menter");
        app.tasks.push(crate::model::Task::new_queued(
            1,
            "me.bin".to_string(),
            Protocol::Http,
            "http://example.com/me.bin".to_string(),
            "/tmp".to_string(),
            1024 * 1024,
            4,
            5,
            None,
            crate::model::config::ProxyChoice::Direct,
            0,
        ));
        app.selected = 0;
        app.on_key(KeyCode::Char('m'), crossterm::event::KeyModifiers::NONE);

        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.as_ref().unwrap().ck_open, "Enter 展开算法下拉");
        app.on_dialog_key(KeyCode::Esc, crossterm::event::KeyModifiers::NONE);

        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
        assert!(
            app.dialog.as_ref().unwrap().proxy_open,
            "Enter 展开代理下拉"
        );
        app.on_dialog_key(KeyCode::Esc, crossterm::event::KeyModifiers::NONE);

        for _ in 0..4 {
            app.on_dialog_key(KeyCode::BackTab, crossterm::event::KeyModifiers::NONE);
        }
        app.on_dialog_key(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "确认后关闭");
        assert_eq!(app.tasks[0].concurrency, 4, "并发空值保持当前");

        app.selected = 0;
        app.on_key(KeyCode::Char('m'), crossterm::event::KeyModifiers::NONE);
        for _ in 0..6 {
            app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        }
        app.on_dialog_key(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "取消焦点 Enter 关闭");
        app.shutdown().await;
    }

    #[test]
    fn proxy_dropdown_open_key_routes() {
        let mut d = add_dlg();
        d.proxy_open = true;
        proxy_dropdown_open_key(&mut d, 2, KeyCode::Down);
        assert_eq!(d.proxy_sel, 1, "Down 前进");
        proxy_dropdown_open_key(&mut d, 2, KeyCode::Down);
        assert_eq!(d.proxy_sel, 0, "Down 环绕回首");
        proxy_dropdown_open_key(&mut d, 2, KeyCode::Up);
        assert_eq!(d.proxy_sel, 1, "Up 环绕到末尾");
        proxy_dropdown_open_key(&mut d, 2, KeyCode::Home);
        assert_eq!(d.proxy_sel, 0);
        proxy_dropdown_open_key(&mut d, 2, KeyCode::End);
        assert_eq!(d.proxy_sel, 1);
        proxy_dropdown_open_key(&mut d, 2, KeyCode::Enter);
        assert!(!d.proxy_open, "Enter 关闭");
        d.proxy_open = true;
        proxy_dropdown_open_key(&mut d, 2, KeyCode::Tab);
        assert!(!d.proxy_open);
        assert_eq!(d.focus, 7, "Add 布局 Tab 跳确认按钮");
        d.proxy_open = true;
        proxy_dropdown_open_key(&mut d, 2, KeyCode::BackTab);
        assert!(!d.proxy_open);
        assert_eq!(d.focus, 5, "Add 布局 BackTab 跳校验码");
        d.proxy_open = true;
        proxy_dropdown_open_key(&mut d, 2, KeyCode::Char('x'));
        assert!(!d.proxy_open, "其余键仅关闭");

        let mut m = modify_dlg();
        m.proxy_open = true;
        proxy_dropdown_open_key(&mut m, 3, KeyCode::Tab);
        assert_eq!(m.focus, 5, "Modify 布局 Tab 跳确认按钮");
        m.proxy_open = true;
        proxy_dropdown_open_key(&mut m, 3, KeyCode::BackTab);
        assert_eq!(m.focus, 3, "Modify 布局 BackTab 跳校验码");

        let mut z = add_dlg();
        proxy_dropdown_open_key(&mut z, 0, KeyCode::End);
        assert_eq!(z.proxy_sel, 0, "n=0 守卫按 1 处理");
        proxy_dropdown_open_key(&mut z, 0, KeyCode::Down);
        assert_eq!(z.proxy_sel, 0);
    }

    #[tokio::test]
    async fn add_dialog_char_routes() {
        let mut app = make_app("achar");
        app.dialog = Some(add_dlg());

        app.on_dialog_key(KeyCode::Char('\u{1}'), crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.as_ref().unwrap().url.is_empty(), "控制字符被拒");
        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char(' '), crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.as_ref().unwrap().ck_open);
        app.on_dialog_key(KeyCode::Esc, crossterm::event::KeyModifiers::NONE);

        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char('f'), crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char('g'), crossterm::event::KeyModifiers::NONE);
        assert_eq!(app.dialog.as_ref().unwrap().ck_value.value, "f");
        for _ in 0..200 {
            app.on_dialog_key(KeyCode::Char('a'), crossterm::event::KeyModifiers::NONE);
        }
        assert_eq!(
            app.dialog.as_ref().unwrap().ck_value.len_chars(),
            128,
            "校验码 128 位上限"
        );
        app.dialog.as_mut().unwrap().ck_value.clear();

        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char(' '), crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.as_ref().unwrap().proxy_open);
        app.on_dialog_key(KeyCode::Esc, crossterm::event::KeyModifiers::NONE);

        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char(' '), crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.is_some(), "空 URL 确认被拒");
        let d = app.dialog.as_mut().unwrap();
        d.url.set("http://example.com/a.bin");
        d.ck_type = CK_NONE;
        d.ck_sel = CK_NONE;
        app.on_dialog_key(KeyCode::Char(' '), crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "确认按钮建任务");
        assert_eq!(app.tasks.len(), 1);

        app.dialog = Some(add_dlg());
        app.on_dialog_key(KeyCode::BackTab, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char(' '), crossterm::event::KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "取消按钮直接关闭");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn arrow_keys_move_cursor_in_text_fields() {
        let mut app = make_app("cur");
        app.dialog = Some(add_dlg());
        for c in ['a', 'b', 'c'] {
            app.on_dialog_key(KeyCode::Char(c), crossterm::event::KeyModifiers::NONE);
        }
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.url.value, "abc");
        assert_eq!(d.url.cursor, 3, "键入后光标在字段尾");

        app.on_dialog_key(KeyCode::Left, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char('X'), crossterm::event::KeyModifiers::NONE);
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.url.value, "abXc", "中位插入落在光标列");
        assert_eq!(d.url.cursor, 3);
        assert_eq!(d.focus, 0, "方向键不跳焦点");

        app.on_dialog_key(KeyCode::Right, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Right, crossterm::event::KeyModifiers::NONE);
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.url.cursor, 4, "Right 到尾钳制");
        app.on_dialog_key(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.url.cursor, 4, "Down 行尾");
        assert_eq!(d.focus, 0);
        for _ in 0..6 {
            app.on_dialog_key(KeyCode::Left, crossterm::event::KeyModifiers::NONE);
        }
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.url.cursor, 0, "Left 到头钳制");
        app.on_dialog_key(KeyCode::Up, crossterm::event::KeyModifiers::NONE);
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.url.cursor, 0, "Up 行首");
        assert_eq!(d.focus, 0);

        app.on_dialog_key(KeyCode::Right, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.url.value, "bXc", "光标 1 处退格删前字符");
        assert_eq!(d.url.cursor, 0);

        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.focus, 1);
        assert_eq!(d.dir.cursor, 0, "焦点切换单源：光标迁至新字段尾");

        app.dialog = None;
        app.tasks.push(crate::model::Task::new_queued(
            1,
            "cur.bin".to_string(),
            Protocol::Http,
            "http://example.com/cur.bin".to_string(),
            "/tmp".to_string(),
            1024 * 1024,
            2,
            5,
            None,
            crate::model::config::ProxyChoice::Direct,
            0,
        ));
        app.selected = 0;
        app.on_key(KeyCode::Char('m'), crossterm::event::KeyModifiers::NONE);
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.conns.value, "2");
        assert_eq!(d.conns.cursor, 1, "预填字段光标在尾（open_modify_dialog）");
        app.on_dialog_key(KeyCode::Left, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Char('9'), crossterm::event::KeyModifiers::NONE);
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.conns.value, "92", "预填字段中位插入");
        assert_eq!(d.focus, 0, "方向键不跳焦点（Modify）");
        app.shutdown().await;
    }

    fn bt_add_dlg(focus: usize) -> Dialog {
        crate::app::testutil::dialog(DialogKind::AddBt, focus)
    }

    #[tokio::test]
    async fn bt_dialog_trackers_edit_delete_and_navigation() {
        let mut app = make_app("trkedit");
        app.dialog = Some(bt_add_dlg(7));

        for c in "udp://a/ann".chars() {
            app.on_dialog_key(KeyCode::Char(c), crossterm::event::KeyModifiers::NONE);
        }
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(
                d.trackers.trim_lines(),
                vec!["udp://a/ann"],
                "字符插入 trackers"
            );
        }

        app.on_dialog_key(KeyCode::Enter, crossterm::event::KeyModifiers::ALT);
        for c in "udp://b/ann".chars() {
            app.on_dialog_key(KeyCode::Char(c), crossterm::event::KeyModifiers::NONE);
        }
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(
                d.trackers.trim_lines(),
                vec!["udp://a/ann", "udp://b/ann"],
                "Alt+Enter 框内换行"
            );
        }

        for _ in 0..4 {
            app.on_dialog_key(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
        }
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(
                d.trackers.trim_lines(),
                vec!["udp://a/ann", "udp://b"],
                "Backspace 删除字符（回归：不能删除字符串）"
            );
        }

        app.on_dialog_key(KeyCode::Home, crossterm::event::KeyModifiers::NONE);
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(d.trackers.row, 0, "Home 跳首行");
            assert_eq!(d.trackers.line().cursor, 0, "Home 光标行首");
        }
        app.on_dialog_key(KeyCode::End, crossterm::event::KeyModifiers::NONE);
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(d.trackers.row, 1, "End 跳末行");
        }

        app.on_dialog_key(KeyCode::Left, crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Delete, crossterm::event::KeyModifiers::NONE);
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(
                d.trackers.trim_lines(),
                vec!["udp://a/ann", "udp://"],
                "Delete 前向删除光标处字符"
            );
        }

        let n = app.dialog.as_ref().unwrap().trackers.lines[1].len_chars();
        for _ in 0..n {
            app.on_dialog_key(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
        }
        app.on_dialog_key(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(
                d.trackers.trim_lines(),
                vec!["udp://a/ann"],
                "行首退格并入上一行"
            );
            assert_eq!(d.trackers.lines.len(), 1, "空行不残留");
        }
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_dialog_backspace_routes_bt_fields() {
        let mut app = make_app("btbs");
        app.dialog = Some(bt_add_dlg(0));
        for c in "/tmp/a.torrent".chars() {
            app.on_dialog_key(KeyCode::Char(c), crossterm::event::KeyModifiers::NONE);
        }
        app.on_dialog_key(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(d.torrent.value, "/tmp/a.torren", "种子行退格");
        }

        for _ in 0..2 {
            app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        }
        app.on_dialog_key(KeyCode::Tab, crossterm::event::KeyModifiers::NONE);
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(d.focus, 3, "Tab 到并发");
        }
        app.on_dialog_key(KeyCode::Char('7'), crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(d.conns.value, "", "BT 并发行退格");
            assert!(d.conns_edited, "退格置编辑位");
        }
        app.shutdown().await;
    }

    #[tokio::test]
    async fn modify_bt_dialog_trackers_backspace() {
        let mut app = make_app("mtrk");
        app.dialog = Some(crate::app::testutil::dialog(DialogKind::ModifyBt, 5));
        app.dialog
            .as_mut()
            .unwrap()
            .trackers
            .set_text("udp://x/ann");
        for _ in 0..4 {
            app.on_dialog_key(KeyCode::Backspace, crossterm::event::KeyModifiers::NONE);
        }
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(
                d.trackers.trim_lines(),
                vec!["udp://x"],
                "ModifyBt trackers 退格（回归）"
            );
        }
        app.shutdown().await;
    }
}
