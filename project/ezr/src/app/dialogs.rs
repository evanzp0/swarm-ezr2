use super::{App, Dialog, DialogKind, CHECKSUM_ALGOS};
use crate::engine::Cmd;
use crate::model::checksum::CK_NONE;
use crate::model::namegen::{DOWNLOADING_EXT, SIDECAR_EXT};
use crate::model::sidecar::Sidecar;
use crate::model::{
    checksum, is_http_url, namegen, protocol_of_url, unix_now, Checksum, Task, TaskState,
};
use crate::textfield::TextInput;

impl App {
    pub(crate) fn set_dialog(&mut self, d: Dialog) {
        self.dialog = Some(d);
        self.dlg_pos = None;
        self.dlg_drag = None;
        self.dlg_area = None;
    }
    fn checksum_from_input(
        &mut self,
        ck_type: usize,
        ck_raw: &str,
        ck_focus: usize,
    ) -> Option<Option<Checksum>> {
        let entries = checksum::checksum_dropdown();
        let (algo, need) = entries[ck_type.min(entries.len() - 1)];
        if ck_type == CK_NONE {
            return Some(None);
        }
        if ck_raw.is_empty() {
            self.set_toast(format!("⚠ {algo} 校验码不可为空（需 {need} 位十六进制）"));
            if let Some(d) = self.dialog.as_mut() {
                d.focus_field(ck_focus);
            }
            return None;
        }
        let algo_idx = ck_type - 1;
        match checksum::validate_value(algo_idx, ck_raw) {
            Ok(v) => Some(Some(Checksum {
                algo: CHECKSUM_ALGOS[algo_idx].0,
                value: v,
            })),
            Err(e) => {
                let algo = CHECKSUM_ALGOS[algo_idx].0;
                self.set_toast(format!("⚠ {algo} {e}"));
                if let Some(d) = self.dialog.as_mut() {
                    d.focus_field(ck_focus);
                }
                None
            }
        }
    }

    pub(super) fn dlg_activate_add(&mut self, btn: usize) {
        match btn {
            6 => self.dlg_confirm_add(),
            7 => self.dlg_add_only(),
            8 => self.dialog = None,
            _ => {}
        }
    }

    pub(super) fn dlg_activate_modify(&mut self, btn: usize) {
        match btn {
            4 => self.dlg_confirm_modify(),
            5 => self.dialog = None,
            _ => {}
        }
    }

    pub(super) fn dlg_confirm_modify(&mut self) {
        let Some(d) = self.dialog.as_ref() else {
            return;
        };
        let Some(id) = d.task_id else {
            return;
        };
        let Some(idx) = self.tasks.iter().position(|t| t.id == id) else {
            self.dialog = None;
            return;
        };
        let conns_raw = d.conns.value.trim().to_string();
        let ck_raw = d.ck_value.value.trim().to_string();
        let ck_type = d.ck_type;
        let proxy_sel = d.proxy_sel.min(self.proxy_options.len().saturating_sub(1));
        let proxy = self.proxy_options[proxy_sel].clone();

        let concurrency = if conns_raw.is_empty() {
            self.tasks[idx].concurrency
        } else {
            conns_raw.parse::<usize>().unwrap_or(0).clamp(1, 64)
        };

        let Some(checksum) = self.checksum_from_input(ck_type, &ck_raw, 2) else {
            return;
        };

        let (endpoint, missing) = self.cfg.resolve_proxy(&proxy);
        if missing {
            if let crate::model::ProxyChoice::Named(name) = &proxy {
                self.set_toast(format!("⚠ 引用的代理「{name}」不存在，已按直连"));
            }
        }

        {
            let t = &mut self.tasks[idx];
            t.concurrency = concurrency;
            t.checksum = checksum;
            t.proxy = proxy;
        }
        self.dialog = None;
        let engine = self.engine.clone();
        tokio::spawn(async move {
            engine
                .send(Cmd::Reconfigure {
                    id,
                    concurrency,
                    endpoint,
                })
                .await;
        });
        self.set_toast("✓ 任务参数已更新，立即生效");
        self.save_registry();
    }

    pub(super) fn dlg_confirm_add(&mut self) {
        self.dlg_finish_add(false);
    }

    pub(super) fn dlg_add_only(&mut self) {
        self.dlg_finish_add(true);
    }

    fn dlg_finish_add(&mut self, paused: bool) {
        let Some(d) = self.dialog.as_ref() else {
            return;
        };
        let url = d.url.value.trim().to_string();
        if url.is_empty() {
            self.set_toast("⚠ 请输入下载 URL");
            return;
        }
        if !is_http_url(&url) {
            self.set_toast("⚠ 仅支持 http:// 或 https:// 链接");
            return;
        }
        let dir_raw = d.dir.value.trim().to_string();
        let conns_raw = d.conns.value.trim().to_string();
        let ck_raw = d.ck_value.value.trim().to_string();
        let ck_type = d.ck_type;
        let proxy = self.proxy_options[d.proxy_sel.min(self.proxy_options.len() - 1)].clone();

        let Some(checksum) = self.checksum_from_input(ck_type, &ck_raw, 4) else {
            return;
        };

        let dir = if dir_raw.is_empty() {
            self.cfg
                .download_dir
                .clone()
                .unwrap_or_else(crate::model::config::default_download_dir)
        } else {
            dir_raw
        };
        let dir = dir.trim_end_matches('/').to_string();

        let protocol = protocol_of_url(&url);
        if self.tasks.iter().any(|t| t.url == url && t.save_dir == dir) {
            self.set_toast("⚠ 任务已存在");
            return;
        }
        let base_name = namegen::derive_name(None, None, &url);
        let id = self.alloc_task_id();
        let resumed = Sidecar::load(&format!("{dir}/{base_name}{SIDECAR_EXT}")).is_some()
            || self.tasks.iter().any(|t| {
                t.url == url
                    && t.save_dir == dir
                    && t.name == base_name
                    && t.state != TaskState::Completed
            });
        let name = if resumed {
            base_name.clone()
        } else {
            let tasks = &self.tasks;
            namegen::dedupe(&base_name, |n| {
                tasks.iter().any(|t| t.name == n && t.save_dir == dir)
                    || namegen::exists_on_disk(&dir, n)
            })
        };
        let conns = match conns_raw.parse::<usize>() {
            Ok(v) if (1..=64).contains(&v) => v,
            _ => self.cfg.http_concurrency,
        };
        let checksum = Self::resolve_checksum(&dir, &name, checksum);
        let ts = unix_now();
        let mut t = Task::new_queued(
            id,
            name.clone(),
            protocol,
            url.clone(),
            dir,
            self.cfg.block_size_http,
            conns,
            self.cfg.max_retries,
            checksum,
            proxy,
            ts,
        );
        if paused {
            t.state = TaskState::Paused;
        }
        if resumed {
            t.name_locked = true;
        }
        self.tasks.push(t);
        self.dialog = None;
        self.filter = 0;
        let flen = self.filtered().len();
        if flen > 0 {
            self.selected = flen - 1;
        }
        if paused {
            if resumed {
                self.set_toast(format!(
                    "⏸ 已添加任务 #{id}: {name}（发现有效断点，已暂停，按空格继续）"
                ));
            } else {
                self.set_toast(format!(
                    "⏸ 已添加任务 #{id}: {name}（已暂停，按空格开始下载）"
                ));
            }
        } else if resumed {
            self.set_toast(format!(
                "✓ 已添加任务 #{id}: {name}（发现有效断点，将自动接续）"
            ));
        } else {
            self.set_toast(format!("✓ 已添加任务 #{id}: {name}"));
        }
        self.save_registry();
    }

    pub(super) fn dlg_activate_delete(&mut self, btn: usize) {
        let Some(idx) = self.sel_idx() else {
            self.dialog = None;
            return;
        };
        let (id, name) = {
            let t = &self.tasks[idx];
            (t.id, t.name.clone())
        };
        match btn {
            0 => {
                self.remove_task(idx, id, false);
                self.set_toast(format!("🗑 已删除任务（保留文件）: {name}"));
                self.dialog = None;
            }
            1 => {
                self.remove_task(idx, id, true);
                self.set_toast(format!("🗑 已删除任务，正在停止引擎并清理本地文件: {name}"));
                self.dialog = None;
            }
            _ => {
                self.dialog = None;
            }
        }
        let flen = self.filtered().len();
        if self.selected >= flen {
            self.selected = flen.saturating_sub(1);
        }
    }

    pub(super) fn remove_task(&mut self, idx: usize, id: u32, delete_files: bool) {
        let (save_dir, name) = {
            let t = &self.tasks[idx];
            (t.save_dir.clone(), t.name.clone())
        };
        self.tasks.remove(idx);
        self.windows.remove(&id);
        self.clear_conn_stats(id);
        self.clear_bt_conn_stats(id);
        if delete_files {
            self.pending_deletes.insert(id, (save_dir, name));
        }
        let engine = self.engine.clone();
        tokio::spawn(async move {
            engine.send(Cmd::CancelBt { id }).await;
            engine.send(Cmd::Cancel { id }).await;
        });
        self.save_registry();
    }

    pub(super) fn delete_task_files(save_dir: &str, name: &str) {
        let base = save_dir.trim_end_matches('/');
        for path in [
            format!("{base}/{name}"),
            format!("{base}/{name}{DOWNLOADING_EXT}"),
            format!("{base}/{name}{SIDECAR_EXT}"),
        ] {
            let _ = std::fs::remove_file(&path);
        }
    }

    #[allow(dead_code)]
    pub fn open_add_dialog(&mut self) {
        self.set_dialog(Dialog {
            kind: DialogKind::Add,
            url: TextInput::new(),
            dir: TextInput::new(),
            conns: TextInput::new(),
            conns_edited: false,
            ck_type: CK_NONE,
            ck_value: TextInput::new(),
            ck_open: false,
            ck_sel: CK_NONE,
            proxy_sel: self.default_proxy_sel,
            proxy_open: false,
            focus: 0,
            task_name: String::new(),
            task_id: None,
            torrent: TextInput::new(),
            bt_tree: Vec::new(),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: crate::multiline::MultiLineBuf::new(),
        });
    }

    pub fn open_modify_dialog(&mut self) {
        let Some(t) = self.sel_task() else {
            self.set_toast("没有选中的任务");
            return;
        };
        if t.state == TaskState::Completed {
            self.set_toast("⚠ 已完成任务不可修改");
            return;
        }
        let (id, name, concurrency, checksum, proxy) = (
            t.id,
            t.name.clone(),
            t.concurrency,
            t.checksum.clone(),
            t.proxy.clone(),
        );
        let ck_type = checksum
            .as_ref()
            .map(|c| checksum::algo_index_exact_or_default(c.algo) + 1)
            .unwrap_or(CK_NONE);
        let proxy_sel = match self.proxy_options.iter().position(|o| o == &proxy) {
            Some(i) => i,
            None => {
                self.proxy_options.push(proxy.clone());
                if let crate::model::ProxyChoice::Named(n) = &proxy {
                    self.proxy_labels.push(n.clone());
                }
                self.proxy_options.len() - 1
            }
        };
        self.set_dialog(Dialog {
            kind: DialogKind::Modify,
            url: TextInput::new(),
            dir: TextInput::new(),
            conns: TextInput::with_value(concurrency.to_string()),
            conns_edited: false,
            ck_type,
            ck_value: TextInput::with_value(checksum.map(|c| c.value).unwrap_or_default()),
            ck_open: false,
            ck_sel: ck_type,
            proxy_sel,
            proxy_open: false,
            focus: 0,
            task_name: name,
            task_id: Some(id),
            torrent: TextInput::new(),
            bt_tree: Vec::new(),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: crate::multiline::MultiLineBuf::new(),
        });
    }

    pub fn open_delete_dialog(&mut self) {
        let Some(t) = self.sel_task() else {
            self.set_toast("没有选中的任务");
            return;
        };
        self.set_dialog(Dialog {
            kind: DialogKind::Delete,
            url: TextInput::new(),
            dir: TextInput::new(),
            conns: TextInput::new(),
            conns_edited: false,
            ck_type: 0,
            ck_value: TextInput::new(),
            ck_open: false,
            ck_sel: 0,
            proxy_sel: 0,
            proxy_open: false,
            focus: 0,
            task_name: t.name.clone(),
            task_id: None,
            torrent: TextInput::new(),
            bt_tree: Vec::new(),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: crate::multiline::MultiLineBuf::new(),
        });
    }
}

#[cfg(test)]
mod add_dialog_flow_tests {
    use crossterm::event::{KeyCode, KeyModifiers};

    use super::*;
    use crate::model::config::Config;
    use crate::model::Protocol;

    #[tokio::test]
    async fn add_dialog_confirm_creates_task_with_normalized_dir() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-app-dlg");
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut app = App::new(Config::default(), reg);

        app.open_add_dialog();
        assert!(app.dialog.is_some());
        for c in "http://example.com/f.bin".chars() {
            app.on_key(KeyCode::Char(c), KeyModifiers::NONE);
        }
        app.on_key(KeyCode::Tab, KeyModifiers::NONE);
        for c in format!("{}/", dir.to_string_lossy()).chars() {
            app.on_key(KeyCode::Char(c), KeyModifiers::NONE);
        }
        app.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(app.dialog.is_none());
        assert_eq!(app.tasks.len(), 1);
        let t = &app.tasks[0];
        assert_eq!(t.name, "f.bin");
        assert_eq!(t.save_dir, dir.to_string_lossy());
        assert_eq!(t.state, TaskState::Queued);
        assert_eq!(
            t.concurrency,
            crate::model::config::DEFAULT_HTTP_CONCURRENCY
        );
        assert!(app
            .toast
            .as_deref()
            .is_some_and(|m| m.contains("已添加任务")));
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn add_only_button_creates_paused_task() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dlg-addonly");
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut app = App::new(Config::default(), reg);
        app.dialog = Some(Dialog {
            kind: DialogKind::Add,
            url: TextInput::with_value("http://example.com/paused.bin"),
            dir: TextInput::with_value(dir.to_string_lossy().to_string()),
            conns: TextInput::new(),
            conns_edited: false,
            ck_type: CK_NONE,
            ck_value: TextInput::new(),
            ck_open: false,
            ck_sel: CK_NONE,
            proxy_sel: 0,
            proxy_open: false,
            focus: 7,
            task_name: String::new(),
            task_id: None,
            torrent: TextInput::new(),
            bt_tree: Vec::new(),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: crate::multiline::MultiLineBuf::new(),
        });
        app.dlg_activate_add(7);
        assert!(app.dialog.is_none(), "仅添加后关闭对话框");
        assert_eq!(app.tasks.len(), 1);
        let t = &app.tasks[0];
        assert_eq!(t.name, "paused.bin");
        assert_eq!(t.state, TaskState::Paused, "仅添加 → 已暂停（D28）");
        assert!(!t.has_slot, "不占下载槽位");
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("已添加任务") && m.contains("已暂停，按空格开始下载")),
            "toast 钦定文案（D29）：{:?}",
            app.toast
        );
        app.selected = 0;
        app.toggle_pause();
        assert_eq!(
            app.tasks[0].state,
            TaskState::Queued,
            "空格继续 = 重新排队（统一路径，不检测空闲槽位）"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn add_only_with_sidecar_resume_keeps_paused() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dlg-addonly-sc");
        let mut app = App::new(
            Config::default(),
            dir.join("registry.json").to_string_lossy().into_owned(),
        );
        app.dialog = Some(Dialog {
            kind: DialogKind::Add,
            url: TextInput::with_value("http://example.com/r.bin"),
            dir: TextInput::with_value(dir.to_string_lossy().into_owned()),
            conns: TextInput::new(),
            conns_edited: false,
            ck_type: CK_NONE,
            ck_value: TextInput::new(),
            ck_open: false,
            ck_sel: CK_NONE,
            proxy_sel: 0,
            proxy_open: false,
            focus: 7,
            task_name: String::new(),
            task_id: None,
            torrent: TextInput::new(),
            bt_tree: Vec::new(),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: crate::multiline::MultiLineBuf::new(),
        });
        crate::app::testutil::preset_resume_sidecar(&dir);
        app.dlg_activate_add(7);
        assert!(app.dialog.is_none(), "仅添加后关闭");
        assert_eq!(app.tasks[0].name, "r.bin", "断点接续沿用原名");
        assert!(
            app.tasks[0].name_locked,
            "接续任务名随接续定稿（失效重下钉名）"
        );
        assert_eq!(app.tasks[0].state, TaskState::Paused, "接续任务同样落暂停");
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("发现有效断点") && m.contains("已暂停，按空格继续")),
            "接续 toast 钦定文案（D29）：{:?}",
            app.toast
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn confirm_add_resumes_by_path_even_with_different_url() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dlg-pathbase");
        let mut app = App::new(
            Config::default(),
            dir.join("registry.json").to_string_lossy().into_owned(),
        );
        app.dialog = Some(Dialog {
            kind: DialogKind::Add,
            url: TextInput::with_value("http://example.com/alias.bin"),
            dir: TextInput::with_value(dir.to_string_lossy().into_owned()),
            conns: TextInput::new(),
            conns_edited: false,
            ck_type: CK_NONE,
            ck_value: TextInput::new(),
            ck_open: false,
            ck_sel: CK_NONE,
            proxy_sel: 0,
            proxy_open: false,
            focus: 7,
            task_name: String::new(),
            task_id: None,
            torrent: TextInput::new(),
            bt_tree: Vec::new(),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: crate::multiline::MultiLineBuf::new(),
        });
        let legacy = serde_json::json!({
            "version": 1,
            "url": "http://other.example/alias.bin",
            "final_url": null,
            "size": 1000,
            "etag": null,
            "last_modified": null,
            "block_size": 1_048_576,
            "block_count": 1,
            "blocks": [0],
            "downloaded": 0,
            "non_resumable": false,
            "expected": null,
            "task": {
                "id": 99,
                "added_at": 0,
                "save_dir": dir.to_string_lossy(),
                "concurrency": 4,
                "protocol": "Http"
            }
        });
        std::fs::write(dir.join("alias.bin.ezr"), legacy.to_string()).unwrap();
        app.dlg_confirm_add();
        assert!(app.dialog.is_none(), "确认后关闭");
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(
            app.tasks[0].name, "alias.bin",
            "路径基接续沿用推导原名（不做同 URL 校验，v1.28/FR-01-26）"
        );
        assert!(
            app.tasks[0].name_locked,
            "接续任务名随接续定稿（首探前失效重下不得去重漂移，QA-RS-14）"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn add_only_rejects_invalid_url_same_as_confirm() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dlg-addonly-bad");
        let mut app = App::new(
            Config::default(),
            dir.join("registry.json").to_string_lossy().into_owned(),
        );
        app.dialog = Some(Dialog {
            kind: DialogKind::Add,
            url: TextInput::with_value("ftp://x/f.bin"),
            dir: TextInput::new(),
            conns: TextInput::new(),
            conns_edited: false,
            ck_type: CK_NONE,
            ck_value: TextInput::new(),
            ck_open: false,
            ck_sel: CK_NONE,
            proxy_sel: 0,
            proxy_open: false,
            focus: 7,
            task_name: String::new(),
            task_id: None,
            torrent: TextInput::new(),
            bt_tree: Vec::new(),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: crate::multiline::MultiLineBuf::new(),
        });
        app.dlg_activate_add(7);
        assert!(app.dialog.is_some(), "非法 URL 拒绝，对话框保持");
        assert!(app.tasks.is_empty(), "无任务创建");
        assert!(app
            .toast
            .as_deref()
            .is_some_and(|m| m.contains("仅支持 http:// 或 https://")));
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn add_dialog_proxy_options_two_states() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dlg-pdef");
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut cfg = Config::from_toml(
            "[[common.proxy]]\nname = \"a\"\ntype = \"socks5\"\nip = \"a\"\nport = 2\nusername = \"u\"\npassword = \"p\"\n[[common.proxy]]\nname = \"b\"\ntype = \"http\"\nip = \"b\"\nport = 3\n[[common.proxy]]\nname = \"c\"\ntype = \"https\"\nip = \"c\"\nport = 4\n",
        );
        cfg.warnings.clear();
        let mut app = App::new(cfg, reg);
        assert_eq!(
            app.proxy_options.len(),
            4,
            "直连 + 命名 a + 命名 b + 命名 c"
        );
        assert_eq!(app.proxy_labels[0], "直连");
        assert_eq!(
            app.proxy_labels[1], "a（socks5）",
            "命名条目显示名带类型标注（FR-01-89）"
        );
        assert_eq!(app.proxy_labels[2], "b（http）");
        assert_eq!(
            app.proxy_labels[3], "c（https）",
            "https 类型标注（v1.9 三值）"
        );
        app.open_add_dialog();
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.proxy_sel, 0, "默认选中恒「直连」（v1.6 两态）");
        app.dialog
            .as_mut()
            .unwrap()
            .url
            .set("http://example.com/p.bin");
        app.dlg_confirm_add();
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(
            app.tasks[0].proxy,
            crate::model::ProxyChoice::Direct,
            "默认直连写入任务"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();

        let reg2 = dir.join("registry2.json").to_string_lossy().to_string();
        let mut app2 = App::new(Config::default(), reg2);
        app2.open_add_dialog();
        assert_eq!(app2.proxy_options.len(), 1, "仅直连");
        assert_eq!(app2.dialog.as_ref().unwrap().proxy_sel, 0);
        app2.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn modify_dialog_prefills_applies_and_rejects_completed() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-dlg-mod");
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut cfg = Config::from_toml(
            "[[common.proxy]]\nname = \"a\"\ntype = \"http\"\nip = \"a\"\nport = 2\n[[common.proxy]]\nname = \"b\"\ntype = \"http\"\nip = \"b\"\nport = 3\n",
        );
        cfg.warnings.clear();
        let mut app = App::new(cfg, reg);
        app.tasks.push(crate::model::Task::new_queued(
            1,
            "m.bin".to_string(),
            Protocol::Http,
            "http://example.com/m.bin".to_string(),
            "/tmp".to_string(),
            1024 * 1024,
            2,
            5,
            Some(Checksum {
                algo: "SHA-256",
                value: "a".repeat(64),
            }),
            crate::model::ProxyChoice::Direct,
            0,
        ));
        app.selected = 0;
        app.on_key(KeyCode::Char('m'), KeyModifiers::NONE);
        let d = app.dialog.as_ref().expect("m 打开修改对话框");
        assert_eq!(d.kind, DialogKind::Modify);
        assert_eq!(d.conns.value, "2");
        assert_eq!(d.ck_type, 4, "SHA-256 下拉下标（NONE 首行 + 1）");
        assert_eq!(d.ck_value.value, "a".repeat(64));
        assert_eq!(d.proxy_sel, 0, "任务为 Direct → 预选直连");
        let d = app.dialog.as_mut().unwrap();
        d.conns.set("6");
        d.ck_type = CK_NONE;
        d.ck_sel = CK_NONE;
        d.ck_value.clear();
        d.proxy_sel = 1;
        app.dlg_confirm_modify();
        assert!(app.dialog.is_none());
        let t = &app.tasks[0];
        assert_eq!(t.concurrency, 6, "并发立即生效");
        assert!(t.checksum.is_none(), "选 NONE 确定 = 清除校验（v1.32）");
        assert_eq!(t.proxy, crate::model::ProxyChoice::Named("a".into()));
        assert!(app
            .toast
            .as_deref()
            .is_some_and(|m| m.contains("任务参数已更新")));
        assert!(dir.join("registry.json").exists(), "注册表已落盘");
        app.shutdown().await;

        let reg2 = dir.join("registry2.json").to_string_lossy().to_string();
        let mut app2 = App::new(Config::default(), reg2);
        app2.tasks.push(crate::model::Task::new_queued(
            1,
            "c.bin".to_string(),
            Protocol::Http,
            "http://example.com/c.bin".to_string(),
            "/tmp".to_string(),
            1024 * 1024,
            4,
            5,
            None,
            crate::model::ProxyChoice::Direct,
            0,
        ));
        app2.tasks[0].state = TaskState::Completed;
        app2.selected = 0;
        app2.filter = 1;
        app2.on_key(KeyCode::Char('m'), KeyModifiers::NONE);
        assert!(app2.dialog.is_none(), "已完成任务不弹对话框");
        assert!(app2
            .toast
            .as_deref()
            .is_some_and(|m| m.contains("不可修改")));
        app2.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
