//! dialog_keys — 对话框逐键处理（导航/Enter/退格/字符输入与下拉选择）

use crossterm::event::KeyCode;

mod routing;

// 键路由函数经本模块 re-export（pub(crate) 兼作内部使用的名字绑定，产品构建
// 由内部消费者消化、无 unused 警告）。可见性链：routing 项 pub（私有模块内
// 不外泄）→ 本模块 pub(crate) → app 门面 #[cfg(test)] pub(crate)——供 crate 根
// property_tests 触达（notes/rust.md「门面 re-export」手法，产品 API 面零增量）。
use routing::kind_add;
pub(crate) use routing::{
    dropdown_open_key, proxy_dropdown_open_key, push_hex_capped, text_backspace, text_char,
};

use super::{Dialog, DialogKind};

/// 对话框文本字段容量上限（URL 与目录共用，逐键输入与 bracketed paste 两
/// 入口单源，architect v116 防双入口容量漂移）
pub(super) const CAP_TEXT: usize = 300;
/// 并发数字段容量上限（位数；并发值域 1–64 恰两位）
pub(super) const CAP_CONNS_DIGITS: usize = 2;
/// 校验码字段容量上限（SHA-512 十六进制位数）
pub(super) const CAP_CK_HEX: usize = 128;

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
            if let Some(d) = self.dialog.as_mut() {
                push_hex_capped(d, c);
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
            if let Some(d) = self.dialog.as_mut() {
                push_hex_capped(d, c);
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

#[cfg(test)]
mod dialog_key_tests {
    use super::*;
    // 测试模块显式导入（拆分后产品路径不再经 mod.rs 转发该常量）
    use crate::app::CHECKSUM_ALGOS;
    use crate::app::{Dialog, DialogKind};
    use crate::model::config::Config;
    use crate::model::Protocol;

    fn make_app(tag: &str) -> crate::app::App {
        let dir = crate::model::testenv::uniq_tmp_dir(&format!("ezr-dlgkey-{tag}"));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        crate::app::App::new(Config::default(), reg)
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
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dlgkey-resume");
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

    fn modify_dlg() -> Dialog {
        crate::app::testutil::dialog(DialogKind::Modify, 0)
    }

    /// Modify 对话框字符输入路由（v1.5/FR-01-87）：0 并发仅数字 ≤2 位、
    /// 1 空格展开算法下拉、2 校验码仅十六进制、3 空格展开代理下拉、
    /// 4/5 空格激活确认/取消按钮
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
        // 按 m 打开修改对话框（预填并发 2）
        app.on_key(KeyCode::Char('m'), crossterm::event::KeyModifiers::NONE);
        assert_eq!(
            app.dialog.as_ref().unwrap().kind,
            DialogKind::Modify,
            "m 打开修改对话框"
        );

        // focus 0：并发仅数字、≤2 位（预填 "2" → "27"，字母与第 3 位拒绝）
        app.on_dialog_key(KeyCode::Char('7'));
        app.on_dialog_key(KeyCode::Char('x'));
        app.on_dialog_key(KeyCode::Char('8'));
        assert_eq!(app.dialog.as_ref().unwrap().conns, "27");
        assert!(app.dialog.as_ref().unwrap().conns_edited, "手动编辑置位");

        // focus 1：空格展开算法下拉
        app.on_dialog_key(KeyCode::Tab);
        app.on_dialog_key(KeyCode::Char(' '));
        assert!(app.dialog.as_ref().unwrap().ck_open, "空格展开算法下拉");
        app.on_dialog_key(KeyCode::Esc);
        assert!(!app.dialog.as_ref().unwrap().ck_open, "Esc 收起下拉");

        // focus 2：校验码仅十六进制
        app.on_dialog_key(KeyCode::Tab);
        app.on_dialog_key(KeyCode::Char('a'));
        app.on_dialog_key(KeyCode::Char('z'));
        assert_eq!(app.dialog.as_ref().unwrap().ck_value, "a", "仅十六进制接收");

        // focus 3：空格展开代理下拉
        app.on_dialog_key(KeyCode::Tab);
        app.on_dialog_key(KeyCode::Char(' '));
        assert!(app.dialog.as_ref().unwrap().proxy_open, "空格展开代理下拉");
        app.on_dialog_key(KeyCode::Esc);
        assert!(!app.dialog.as_ref().unwrap().proxy_open);

        // focus 4：空格激活确认；校验码 "a" 非法 → 不生效并聚焦回校验码
        app.on_dialog_key(KeyCode::Tab);
        app.on_dialog_key(KeyCode::Char(' '));
        assert!(app.dialog.is_some(), "非法校验码不生效");
        assert_eq!(app.dialog.as_ref().unwrap().focus, 2, "聚焦回校验码");

        // 清空校验码 → Enter 确认：并发 27 生效、对话框关闭
        app.on_dialog_key(KeyCode::Backspace);
        assert!(app.dialog.as_ref().unwrap().ck_value.is_empty());
        app.on_dialog_key(KeyCode::Enter);
        assert!(app.dialog.is_none(), "确认后关闭");
        assert_eq!(app.tasks[0].concurrency, 27, "并发立即生效");

        // focus 5：空格激活取消按钮 → 直接关闭
        app.selected = 0;
        app.on_key(KeyCode::Char('m'), crossterm::event::KeyModifiers::NONE);
        app.on_dialog_key(KeyCode::Down);
        app.on_dialog_key(KeyCode::Down);
        app.on_dialog_key(KeyCode::Down);
        app.on_dialog_key(KeyCode::Down);
        app.on_dialog_key(KeyCode::Down); // focus 0 → 5
        assert_eq!(app.dialog.as_ref().unwrap().focus, 5);
        app.on_dialog_key(KeyCode::Char(' '));
        assert!(app.dialog.is_none(), "取消按钮直接关闭");
        app.shutdown().await;
    }

    /// Modify 对话框 Enter 路由：1/3 展开对应下拉、5 取消关闭、
    /// 其余焦点走确认（dlg_confirm_modify）
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

        // focus 1 Enter：展开算法下拉
        app.on_dialog_key(KeyCode::Down);
        app.on_dialog_key(KeyCode::Enter);
        assert!(app.dialog.as_ref().unwrap().ck_open, "Enter 展开算法下拉");
        app.on_dialog_key(KeyCode::Esc);

        // focus 3 Enter：展开代理下拉
        app.on_dialog_key(KeyCode::Down);
        app.on_dialog_key(KeyCode::Down);
        app.on_dialog_key(KeyCode::Enter);
        assert!(
            app.dialog.as_ref().unwrap().proxy_open,
            "Enter 展开代理下拉"
        );
        app.on_dialog_key(KeyCode::Esc);

        // focus 0 Enter：确认 → 并发非法值钳制生效路径（空 = 保持当前 4）
        app.on_dialog_key(KeyCode::Up); // 回 focus 3
        app.on_dialog_key(KeyCode::Up);
        app.on_dialog_key(KeyCode::Up); // 回 focus 0
        app.on_dialog_key(KeyCode::Enter);
        assert!(app.dialog.is_none(), "确认后关闭");
        assert_eq!(app.tasks[0].concurrency, 4, "并发空值保持当前");

        // focus 5 Enter：取消关闭
        app.selected = 0;
        app.on_key(KeyCode::Char('m'), crossterm::event::KeyModifiers::NONE);
        for _ in 0..5 {
            app.on_dialog_key(KeyCode::Down);
        }
        app.on_dialog_key(KeyCode::Enter);
        assert!(app.dialog.is_none(), "取消焦点 Enter 关闭");
        app.shutdown().await;
    }

    /// 代理下拉展开态逐键路由（v1.5/FR-01-86）：Up/Down 环绕、Home/End 定位、
    /// Enter/Esc 关闭、Tab/BackTab 关闭并按布局跳焦点、其余键关闭、n=0 守卫
    #[test]
    fn proxy_dropdown_open_key_routes() {
        // Add 布局：Tab→6、BackTab→4
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
        assert_eq!(d.focus, 6, "Add 布局 Tab 跳确认按钮");
        d.proxy_open = true;
        proxy_dropdown_open_key(&mut d, 2, KeyCode::BackTab);
        assert!(!d.proxy_open);
        assert_eq!(d.focus, 4, "Add 布局 BackTab 跳校验码");
        d.proxy_open = true;
        proxy_dropdown_open_key(&mut d, 2, KeyCode::Char('x'));
        assert!(!d.proxy_open, "其余键仅关闭");

        // Modify 布局：Tab→4、BackTab→2
        let mut m = modify_dlg();
        m.proxy_open = true;
        proxy_dropdown_open_key(&mut m, 3, KeyCode::Tab);
        assert_eq!(m.focus, 4, "Modify 布局 Tab 跳确认按钮");
        m.proxy_open = true;
        proxy_dropdown_open_key(&mut m, 3, KeyCode::BackTab);
        assert_eq!(m.focus, 2, "Modify 布局 BackTab 跳校验码");

        // n=0 守卫：按 1 个选项处理（max(1)），End/Down 均落在 0
        let mut z = add_dlg();
        proxy_dropdown_open_key(&mut z, 0, KeyCode::End);
        assert_eq!(z.proxy_sel, 0, "n=0 守卫按 1 处理");
        proxy_dropdown_open_key(&mut z, 0, KeyCode::Down);
        assert_eq!(z.proxy_sel, 0);
    }

    /// Add 对话框字符输入路由：文本字段拒控制字符、算法/代理行空格展开下拉、
    /// 校验码十六进制 ≤128 位、确认/取消按钮空格激活
    #[tokio::test]
    async fn add_dialog_char_routes() {
        let mut app = make_app("achar");
        app.dialog = Some(add_dlg());

        // focus 0：控制字符不进 URL 字段
        app.on_dialog_key(KeyCode::Char('\u{1}'));
        assert!(app.dialog.as_ref().unwrap().url.is_empty(), "控制字符被拒");

        // focus 3：空格展开算法下拉
        app.on_dialog_key(KeyCode::Tab);
        app.on_dialog_key(KeyCode::Tab);
        app.on_dialog_key(KeyCode::Tab);
        app.on_dialog_key(KeyCode::Char(' '));
        assert!(app.dialog.as_ref().unwrap().ck_open);
        app.on_dialog_key(KeyCode::Esc);

        // focus 4：校验码仅十六进制，≤128 位
        app.on_dialog_key(KeyCode::Tab);
        app.on_dialog_key(KeyCode::Char('f'));
        app.on_dialog_key(KeyCode::Char('g'));
        assert_eq!(app.dialog.as_ref().unwrap().ck_value, "f");
        for _ in 0..200 {
            app.on_dialog_key(KeyCode::Char('a'));
        }
        assert_eq!(
            app.dialog.as_ref().unwrap().ck_value.chars().count(),
            128,
            "校验码 128 位上限"
        );
        app.dialog.as_mut().unwrap().ck_value.clear();

        // focus 5：空格展开代理下拉
        app.on_dialog_key(KeyCode::Tab);
        app.on_dialog_key(KeyCode::Char(' '));
        assert!(app.dialog.as_ref().unwrap().proxy_open);
        app.on_dialog_key(KeyCode::Esc);

        // focus 6：空格激活确认；空 URL → 拒绝保持
        app.on_dialog_key(KeyCode::Tab);
        app.on_dialog_key(KeyCode::Char(' '));
        assert!(app.dialog.is_some(), "空 URL 确认被拒");
        // 填入合法 URL 后再确认 → 建任务并关闭
        app.dialog.as_mut().unwrap().url = "http://example.com/a.bin".to_string();
        app.on_dialog_key(KeyCode::Char(' '));
        assert!(app.dialog.is_none(), "确认按钮建任务");
        assert_eq!(app.tasks.len(), 1);

        // focus 7：空格激活取消按钮 → 直接关闭
        app.dialog = Some(add_dlg());
        app.on_dialog_key(KeyCode::Up); // 环绕到 7
        app.on_dialog_key(KeyCode::Char(' '));
        assert!(app.dialog.is_none(), "取消按钮直接关闭");
        app.shutdown().await;
    }
}
