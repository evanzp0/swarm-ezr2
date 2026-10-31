use super::dialog_keys::CAP_TEXT;
use super::{App, Dialog, DialogKind};
use crate::bt::metainfo::TorrentMeta;
use crate::engine::Cmd;
use crate::model::{BtFile, BtTaskData, Protocol, Task, TaskState};
use crate::textfield::TextInput;

impl App {
    pub(crate) fn open_add_bt_dialog_with(&mut self, source: &str, meta: &TorrentMeta) {
        let files: Vec<BtFile> = meta
            .files
            .iter()
            .map(|f| BtFile {
                path: f.path.clone(),
                size: f.length,
                selected: true,
            })
            .collect();
        let trackers_prefill =
            Self::merge_trackers(&self.cfg.http_trackers, &self.cfg.udp_trackers, meta, &[])
                .join("\n");
        self.set_dialog(Dialog {
            kind: DialogKind::AddBt,
            url: TextInput::new(),
            dir: TextInput::with_value(
                self.cfg
                    .download_dir
                    .clone()
                    .unwrap_or_else(crate::model::config::default_download_dir),
            ),
            conns: TextInput::with_value(self.cfg.bt_concurrency.to_string()),
            conns_edited: false,
            ck_type: crate::model::checksum::CK_NONE,
            ck_value: TextInput::new(),
            ck_open: false,
            ck_sel: crate::model::checksum::CK_NONE,
            proxy_sel: self.default_proxy_sel,
            proxy_open: false,
            focus: 0,
            task_name: meta.name.clone(),
            task_id: None,
            torrent: TextInput::with_value(source),
            bt_tree: super::bt_tree::bt_build_tree(&files),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: {
                let mut tr = crate::multiline::MultiLineBuf::new();
                tr.set_text(&trackers_prefill);
                tr
            },
        });
    }

    pub(crate) fn open_modify_bt_dialog(&mut self) {
        let Some(t) = self.sel_task() else {
            self.set_toast("没有选中的任务");
            return;
        };
        if t.state == TaskState::Completed {
            self.set_toast("⚠ 已完成任务不可修改");
            return;
        }
        let (id, name, concurrency, proxy) = (t.id, t.name.clone(), t.concurrency, t.proxy.clone());
        let (files, trackers_text) = match t.bt.as_ref() {
            Some(b) => (b.files.clone(), b.trackers.join("\n")),
            None => (Vec::new(), String::new()),
        };

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
            kind: DialogKind::ModifyBt,
            url: TextInput::new(),
            dir: TextInput::new(),
            conns: TextInput::with_value(concurrency.to_string()),
            conns_edited: false,
            ck_type: crate::model::checksum::CK_NONE,
            ck_value: TextInput::new(),
            ck_open: false,
            ck_sel: crate::model::checksum::CK_NONE,
            proxy_sel,
            proxy_open: false,
            focus: Dialog::bt_list_focus(DialogKind::ModifyBt),
            task_name: name,
            task_id: Some(id),
            torrent: TextInput::new(),
            bt_tree: super::bt_tree::bt_build_tree(&files),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: {
                let mut tr = crate::multiline::MultiLineBuf::new();
                tr.set_text(&trackers_text);
                tr
            },
        });
    }

    pub(crate) fn modify_dialog_entry(&mut self) {
        if self
            .sel_task()
            .is_some_and(|t| t.protocol.is_bt() && t.state != TaskState::Completed)
        {
            self.open_modify_bt_dialog();
            return;
        }
        self.open_modify_dialog();
    }

    pub(crate) fn make_bt_spec(&self, t: &Task, meta: &TorrentMeta) -> crate::engine::BtSpec {
        let bt = t.bt.as_ref();
        crate::engine::BtSpec {
            id: t.id,
            source: t.url.clone(),
            meta: meta.clone(),
            save_dir: t.save_dir.clone(),
            name: t.name.clone(),
            files: bt.map(|b| b.files.clone()).unwrap_or_default(),
            trackers: bt.map(|b| b.trackers.clone()).unwrap_or_default(),
            concurrency: t.concurrency,
            proxy_endpoint: self.cfg.resolve_proxy(&t.proxy).0,
            seeding: false,
            max_speed: self.cfg.max_speed,
            tunables: crate::engine::BtTunables {
                peer_pipeline: self.cfg.peer_pipeline,
                upload_slots: self.cfg.upload_slots,
                idle_eviction_secs: self.cfg.idle_eviction_secs,
                connect_timeout_secs: self.cfg.connect_timeout_secs,
                handshake_timeout_secs: self.cfg.handshake_timeout_secs,
                request_timeout_secs: self.cfg.request_timeout_secs,
                requests_per_piece: self.cfg.requests_per_piece,
                rechoke_interval_secs: self.cfg.rechoke_interval_secs,
                optimistic_interval_secs: self.cfg.optimistic_interval_secs,
                progress_emit_min_interval_ms: self.cfg.progress_emit_min_interval_ms,
                tracker_numwant: self.cfg.tracker_numwant,
                peer_retry_base_secs: self.cfg.peer_retry_base_secs,
                peer_retry_cap_secs: self.cfg.peer_retry_cap_secs,
                tracker_backoff_start_secs: self.cfg.tracker_backoff_start_secs,
                tracker_backoff_cap_secs: self.cfg.tracker_backoff_cap_secs,
                udp_base_timeout_secs: self.cfg.udp_base_timeout_secs,
                udp_retries: self.cfg.udp_retries,
            },
        }
    }
}

impl App {
    fn bt_trackers_char(&mut self, c: char) {
        if !c.is_control() {
            if let Some(d) = self.dialog.as_mut() {
                d.trackers.insert_char(c);
            }
        }
    }

    pub(crate) fn bt_trackers_newline(&mut self) {
        if !self.dialog.as_mut().is_some_and(|d| d.trackers.newline()) {
            self.set_toast(format!(
                "⚠ trackers 最多 {} 行",
                crate::multiline::MULTI_MAX_LINES
            ));
        }
    }

    pub(super) fn add_bt_dialog_enter(&mut self, focus: usize) {
        if focus == 4 {
            let d = self.dialog.as_mut().unwrap();
            d.proxy_open = true;
        } else if focus == 6 {
            self.dlg_confirm_add_bt(false);
        } else if focus == 7 {
            self.dlg_confirm_add_bt(true);
        } else if focus == 8 {
            self.dialog = None;
        } else {
            self.dlg_confirm_add_bt(false);
        }
    }

    pub(super) fn modify_bt_dialog_enter(&mut self, focus: usize) {
        if focus == 2 {
            let d = self.dialog.as_mut().unwrap();
            d.proxy_open = true;
        } else if focus == 4 {
            self.dlg_confirm_modify_bt();
        } else if focus == 5 {
            self.dialog = None;
        } else {
            self.dlg_confirm_modify_bt();
        }
    }

    pub(super) fn add_bt_dialog_char(&mut self, focus: usize, c: char) {
        if focus < 2 {
            if !c.is_control() {
                let d = self.dialog.as_mut().unwrap();
                match focus {
                    0 => {
                        d.torrent.insert(&c.to_string(), CAP_TEXT);
                    }
                    1 => {
                        d.dir.insert(&c.to_string(), CAP_TEXT);
                    }
                    _ => {}
                }
            }
        } else if focus == 2 {
            self.bt_list_char(c);
        } else if focus == 3 {
            if c.is_ascii_digit() {
                let d = self.dialog.as_mut().unwrap();
                if d.conns
                    .insert(&c.to_string(), super::dialog_keys::CAP_CONNS_DIGITS)
                    > 0
                {
                    d.conns_edited = true;
                }
            }
        } else if focus == 5 {
            self.bt_trackers_char(c);
        } else if c == ' ' {
            if focus == 4 {
                let d = self.dialog.as_mut().unwrap();
                d.proxy_open = true;
            } else {
                self.dlg_activate_add_bt(focus);
            }
        }
    }

    pub(super) fn modify_bt_dialog_char(&mut self, focus: usize, c: char) {
        if focus == 0 {
            self.bt_list_char(c);
        } else if focus == 1 {
            if c.is_ascii_digit() {
                let d = self.dialog.as_mut().unwrap();
                if d.conns
                    .insert(&c.to_string(), super::dialog_keys::CAP_CONNS_DIGITS)
                    > 0
                {
                    d.conns_edited = true;
                }
            }
        } else if focus == 3 {
            self.bt_trackers_char(c);
        } else if c == ' ' {
            if focus == 2 {
                let d = self.dialog.as_mut().unwrap();
                d.proxy_open = true;
            } else {
                self.dlg_activate_modify_bt(focus);
            }
        }
    }

    fn bt_list_char(&mut self, c: char) {
        let Some(d) = self.dialog.as_mut() else {
            return;
        };
        match c {
            ' ' => d.bt_toggle_cursor(),
            'a' | 'A' => d.bt_select_all(true),
            'n' | 'N' => d.bt_select_all(false),
            'i' | 'I' => d.bt_invert(),
            _ => {}
        }
    }

    pub(super) fn dlg_activate_add_bt(&mut self, btn: usize) {
        match btn {
            6 => self.dlg_confirm_add_bt(false),
            7 => self.dlg_confirm_add_bt(true),
            8 => self.dialog = None,
            _ => {}
        }
    }

    pub(super) fn dlg_activate_modify_bt(&mut self, btn: usize) {
        match btn {
            4 => self.dlg_confirm_modify_bt(),
            5 => self.dialog = None,
            _ => {}
        }
    }
}

impl App {
    fn merge_trackers(
        cfg_http: &[String],
        cfg_udp: &[String],
        meta: &TorrentMeta,
        manual: &[String],
    ) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut push = |raw: &str| {
            let s = raw.trim().to_string();
            if !s.is_empty() && !out.contains(&s) {
                out.push(s);
            }
        };
        for u in cfg_http.iter().chain(cfg_udp.iter()) {
            push(u);
        }
        if let Some(a) = meta.announce.as_ref() {
            push(a);
        }
        for u in &meta.announce_list {
            push(u);
        }
        for u in manual {
            push(u);
        }
        out
    }

    fn dedup_keep_order(items: Vec<String>) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for s in items {
            if !out.contains(&s) {
                out.push(s);
            }
        }
        out
    }

    pub(crate) fn dlg_confirm_add_bt(&mut self, paused: bool) {
        let Some(d) = self.dialog.as_ref() else {
            return;
        };
        let source = d.torrent.value.trim().to_string();
        if source.is_empty() {
            self.set_toast("⚠ 请输入种子文件路径");
            if let Some(d) = self.dialog.as_mut() {
                d.focus_field(0);
            }
            return;
        }
        let (sel_cnt, total_files) = d.bt_counts();
        if sel_cnt == 0 {
            self.set_toast("⚠ 请至少选择一个要下载的文件（空格勾选）");
            if let Some(d) = self.dialog.as_mut() {
                d.focus = Dialog::bt_list_focus(DialogKind::AddBt);
            }
            return;
        }
        let files = d.bt_flatten();
        let dir_raw = d.dir.value.trim().to_string();
        let conns_raw = d.conns.value.trim().to_string();
        let proxy_sel = d.proxy_sel.min(self.proxy_options.len() - 1);

        let dir = if dir_raw.is_empty() {
            self.cfg
                .download_dir
                .clone()
                .unwrap_or_else(crate::model::config::default_download_dir)
        } else {
            dir_raw
        };
        let dir = crate::model::config::normalize_save_dir(&dir);

        if self
            .tasks
            .iter()
            .any(|t| t.url == source && t.save_dir == dir)
        {
            self.set_toast("⚠ 任务已存在");
            return;
        }

        let Ok(bytes) = std::fs::read(crate::model::config::expand_tilde(&source)) else {
            self.set_toast("⚠ 无法读取种子文件");
            return;
        };
        let Ok(meta) = TorrentMeta::parse(&bytes) else {
            self.set_toast("⚠ 无法读取种子文件");
            return;
        };

        let base_name = meta.name.clone();
        let name = {
            let mut seq = 1u32;
            let mut n = base_name.clone();
            while self.tasks.iter().any(|t| t.name == n && t.save_dir == dir) {
                n = format!("{base_name}.{seq}");
                seq += 1;
            }
            n
        };

        let conns = conns_raw
            .parse::<usize>()
            .map(|v| v.clamp(1, 64))
            .unwrap_or(self.cfg.bt_concurrency);

        let edited = Self::dedup_keep_order(d.trackers.trim_lines());
        let trackers = if edited.is_empty() {
            Self::merge_trackers(&self.cfg.http_trackers, &self.cfg.udp_trackers, &meta, &[])
        } else {
            edited
        };

        let id = self.alloc_task_id();
        let proxy = self.proxy_options[proxy_sel].clone();
        let total = files
            .iter()
            .filter(|f| f.selected)
            .map(|f| f.size)
            .sum::<u64>()
            .max(1);

        let mut t = Task::new_queued(
            id,
            name.clone(),
            Protocol::Bt,
            source.clone(),
            dir,
            u64::from(meta.piece_length),
            conns,
            self.cfg.max_retries,
            None,
            proxy,
            crate::model::unix_now(),
        );
        t.probed = true;
        t.total = total;
        t.name_locked = true;
        t.bt = Some(BtTaskData {
            source: source.clone(),
            trackers,
            piece_length: meta.piece_length,
            files,
        });
        if paused {
            t.state = TaskState::Paused;
        }
        self.tasks.push(t);
        self.dialog = None;

        self.filter = 0;
        let flen = self.filtered().len();
        if flen > 0 {
            self.selected = flen - 1;
        }

        if paused {
            self.set_toast(format!(
                "⏸ 已添加任务 #{id}: {name}（BT，{sel_cnt}/{total_files} 文件，已暂停，按空格开始）"
            ));
        } else {
            self.set_toast(format!(
                "✓ 已添加任务 #{id}: {name}（BT，已选 {sel_cnt}/{total_files} 文件）"
            ));
        }
        self.save_registry();
    }

    pub(crate) fn dlg_confirm_modify_bt(&mut self) {
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
        let (sel_cnt, _total_files) = d.bt_counts();
        if sel_cnt == 0 {
            self.set_toast("⚠ 请至少保留一个要下载的文件（空格勾选）");
            if let Some(d) = self.dialog.as_mut() {
                d.focus = Dialog::bt_list_focus(DialogKind::ModifyBt);
            }
            return;
        }
        let files = d.bt_flatten();
        let conns_raw = d.conns.value.trim().to_string();
        let trackers_lines = d.trackers.trim_lines();
        let proxy_sel = d.proxy_sel.min(self.proxy_options.len().saturating_sub(1));
        let proxy = self.proxy_options[proxy_sel].clone();

        let concurrency = if conns_raw.is_empty() {
            self.tasks[idx].concurrency
        } else {
            conns_raw.parse::<usize>().unwrap_or(0).clamp(1, 64)
        };

        let old_total = self.tasks[idx].total;
        let changed =
            new_total_of(&files) != old_total || concurrency != self.tasks[idx].concurrency;
        let state = self.tasks[idx].state;
        let source = self.tasks[idx].url.clone();
        let new_total = new_total_of(&files);
        let old_trackers = self.tasks[idx]
            .bt
            .as_ref()
            .map(|b| b.trackers.clone())
            .unwrap_or_default();

        {
            let t = &mut self.tasks[idx];
            t.concurrency = concurrency;
            t.proxy = proxy;
            if let Some(b) = t.bt.as_mut() {
                b.files = files;
                if !trackers_lines.is_empty() {
                    b.trackers = trackers_lines;
                }
            }
            t.total = new_total;
            t.downloaded = t.downloaded.min(new_total);
        }
        let new_trackers = self.tasks[idx]
            .bt
            .as_ref()
            .map(|b| b.trackers.clone())
            .unwrap_or_default();
        let trackers_changed = new_trackers != old_trackers;
        self.dialog = None;

        let (endpoint, missing) = self.cfg.resolve_proxy(&self.tasks[idx].proxy);
        if missing {
            if let crate::model::ProxyChoice::Named(nm) = &self.tasks[idx].proxy {
                self.set_toast(format!("⚠ 引用的代理「{nm}」不存在，已按直连"));
            }
        }
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

        let mut rebuilt = true;
        let mut pool_updated = false;
        if changed && matches!(state, TaskState::Downloading | TaskState::Seeding) {
            rebuilt = match std::fs::read(crate::model::config::expand_tilde(&source))
                .map_err(|_| ())
                .and_then(|b| TorrentMeta::parse(&b).map_err(|_| ()))
            {
                Ok(meta) => {
                    let spec = Box::new(self.make_bt_spec(&self.tasks[idx], &meta));
                    let engine = self.engine.clone();
                    tokio::spawn(async move {
                        engine.send(Cmd::CancelBt { id }).await;
                        engine.send(Cmd::StartBt { spec }).await;
                    });
                    pool_updated = true;
                    true
                }
                Err(_) => false,
            };
        }
        if trackers_changed
            && !pool_updated
            && matches!(state, TaskState::Downloading | TaskState::Seeding)
        {
            let engine = self.engine.clone();
            tokio::spawn(async move {
                engine
                    .send(Cmd::BtUpdateTrackers {
                        id,
                        trackers: new_trackers,
                    })
                    .await;
            });
        }

        if rebuilt {
            self.set_toast(format!(
                "✓ BT 任务已更新：{}/{} 文件 · 共 {}（立即生效）",
                sel_cnt,
                self.tasks[idx]
                    .bt
                    .as_ref()
                    .map(|b| b.files.len())
                    .unwrap_or(0),
                crate::model::sizefmt::fmt_size(self.tasks[idx].total)
            ));
        } else {
            self.set_toast("⚠ 无法读取种子文件");
        }
        self.save_registry();
    }
}

fn new_total_of(files: &[BtFile]) -> u64 {
    files
        .iter()
        .filter(|f| f.selected)
        .map(|f| f.size)
        .sum::<u64>()
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-btdlg", tag)
    }

    fn write_multi_torrent(dir: &std::path::Path, file_name: &str) -> String {
        let pieces = [0u8; 20];
        let mut buf = Vec::new();
        buf.extend_from_slice(b"d8:announce14:udp://seed/ann");
        buf.extend_from_slice(b"13:announce-listl");
        buf.extend_from_slice(b"l12:udp://l1/annee");
        buf.extend_from_slice(b"4:infod5:filesl");
        buf.extend_from_slice(b"d6:lengthi5e4:pathl3:sub5:a.txtee");
        buf.extend_from_slice(b"d6:lengthi7e4:pathl5:b.binee");
        buf.extend_from_slice(b"e4:name3:tor12:piece lengthi4e6:pieces20:");
        buf.extend_from_slice(&pieces);
        buf.push(b'e');
        buf.push(b'e');
        let path = dir.join(file_name);
        std::fs::write(&path, &buf).unwrap();
        path.to_string_lossy().to_string()
    }

    fn parse_at(path: &str) -> TorrentMeta {
        let bytes = std::fs::read(path).unwrap();
        TorrentMeta::parse(&bytes).unwrap()
    }

    #[test]
    fn merge_trackers_three_sources_dedup_in_order() {
        let meta = parse_at(&{
            let dir = crate::model::testenv::uniq_tmp_dir("ezr-btdlg-mt");
            write_multi_torrent(&dir, "m.torrent")
        });
        let merged = App::merge_trackers(
            &[
                "udp://cfg1/ann".into(),
                "udp://seed/ann".into(),
                "  ".into(),
            ],
            &["http://cfg2/ann".into()],
            &meta,
            &["http://cfg2/ann".into(), "udp://manual/ann".into()],
        );
        assert_eq!(
            merged,
            vec![
                "udp://cfg1/ann",
                "udp://seed/ann",
                "http://cfg2/ann",
                "udp://l1/ann",
                "udp://manual/ann",
            ],
            "三来源合并：config → torrent(announce → announce-list) → 手输，精确去重保序"
        );
    }

    #[tokio::test]
    async fn open_add_bt_prefills_real_metadata() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btdlg-open");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = parse_at(&path);
        let mut app = make_app("open");
        app.open_add_bt_dialog_with(&path, &meta);
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.kind, DialogKind::AddBt);
        assert_eq!(d.torrent.value, path);
        assert_eq!(d.task_name, "tor", "任务名 = info.name（D50）");
        assert_eq!(d.conns.value, "20", "并发预填 20");
        assert_eq!(
            d.dir.value,
            crate::model::config::default_download_dir(),
            "保存到预填配置默认目录（D48）"
        );
        assert_eq!(d.bt_counts(), (2, 2));
        assert_eq!(d.bt_selected_bytes(), 12);
        assert_eq!(d.bt_row_count(), 1, "根层仅 tor 目录（默认收拢）");
        assert_eq!(
            d.trackers.trim_lines(),
            vec!["udp://seed/ann", "udp://l1/ann"],
            "trackers 预填 = .torrent 解析（announce → announce-list，无 config 时）"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn open_add_bt_prefills_config_and_torrent_trackers() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btdlg-prefill");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = parse_at(&path);
        let mut app = make_app("prefill");
        app.cfg.http_trackers = vec!["http://cfg-h/ann".into()];
        app.cfg.udp_trackers = vec!["udp://cfg1/ann".into()];
        app.open_add_bt_dialog_with(&path, &meta);
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(
            d.trackers.trim_lines(),
            vec![
                "http://cfg-h/ann",
                "udp://cfg1/ann",
                "udp://seed/ann",
                "udp://l1/ann"
            ],
            "trackers 罗列 config + .torrent（config → announce → announce-list 去重保序）"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn confirm_add_builds_task_with_merged_pool_and_engine_start() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btdlg-add");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = parse_at(&path);
        let mut app = make_app("add");
        app.cfg.http_trackers = vec!["udp://cfg1/ann".into()];
        app.open_add_bt_dialog_with(&path, &meta);
        {
            let d = app.dialog.as_ref().unwrap();
            assert_eq!(
                d.trackers.trim_lines(),
                vec!["udp://cfg1/ann", "udp://seed/ann", "udp://l1/ann"],
                "打开即罗列 config + torrent trackers"
            );
        }
        app.dialog
            .as_mut()
            .unwrap()
            .trackers
            .set_text("udp://seed/ann\nudp://manual/ann");
        app.dlg_confirm_add_bt(false);
        assert!(app.dialog.is_none(), "确认后关闭");
        assert_eq!(app.tasks.len(), 1);
        let t = &app.tasks[0];
        assert_eq!(t.protocol, Protocol::Bt);
        assert_eq!(t.name, "tor", "名称 = info.name（D50）");
        assert!(t.probed && t.name_locked, "BT 无探测流程：元数据即权威");
        assert_eq!(t.total, 12, "total = 选中文件合计（D49 全选）");
        assert_eq!(t.block_size, 4, "分块 = 元数据 piece length（D46）");
        let b = t.bt.as_ref().unwrap();
        assert_eq!(b.source, path);
        assert_eq!(b.files.len(), 2);
        assert!(b.files.iter().all(|f| f.selected));
        assert_eq!(
            b.trackers,
            vec!["udp://seed/ann", "udp://manual/ann"],
            "编辑后面板内容即任务池：删掉的 config tracker 不回填、手输行纳入"
        );
        assert_eq!(t.state, TaskState::Queued, "非 paused = 等待中");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn confirm_add_paused_lands_paused_without_engine_start() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btdlg-pause");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = parse_at(&path);
        let mut app = make_app("pause");
        app.open_add_bt_dialog_with(&path, &meta);
        app.dlg_confirm_add_bt(true);
        let t = &app.tasks[0];
        assert_eq!(t.state, TaskState::Paused, "仅添加 = 已暂停（不占槽位）");
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.starts_with("⏸ 已添加任务 #")),
            "paused toast：{:?}",
            app.toast
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn confirm_add_validation_chain() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btdlg-val");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = parse_at(&path);
        let mut app = make_app("val");
        app.open_add_bt_dialog_with(&path, &meta);

        app.dialog.as_mut().unwrap().torrent.clear();
        app.dlg_confirm_add_bt(false);
        assert!(app.dialog.is_some(), "空来源不关窗");
        assert_eq!(app.dialog.as_ref().unwrap().focus, 0, "焦点回种子行");

        {
            let d = app.dialog.as_mut().unwrap();
            d.torrent.set(&path);
            d.bt_select_all(false);
        }
        app.dlg_confirm_add_bt(false);
        assert!(app.dialog.is_some());
        assert_eq!(app.dialog.as_ref().unwrap().focus, 2, "焦点回文件列表");

        app.dialog.as_mut().unwrap().bt_select_all(true);
        std::fs::remove_file(&path).unwrap();
        app.dlg_confirm_add_bt(false);
        assert!(app.dialog.is_some(), "解析失败不关窗");
        assert_eq!(app.toast.as_deref(), Some("⚠ 无法读取种子文件"));
        assert!(app.tasks.is_empty());
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn confirm_add_tilde_torrent_path_expands_home() {
        let Some(home) = crate::model::config::home_dir() else {
            return;
        };
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let dir = home.join(format!(".ezr-btdlg-tilde-{}-{ms}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = parse_at(&path);
        let dirname = dir.file_name().unwrap().to_string_lossy().to_string();
        let mut app = make_app("tilde");
        app.open_add_bt_dialog_with(&path, &meta);
        app.dialog
            .as_mut()
            .unwrap()
            .torrent
            .set(format!("~/{dirname}/a.torrent").as_str());
        app.dlg_confirm_add_bt(false);
        assert_eq!(app.tasks.len(), 1, "~/ 路径展开后确认建任务");
        assert_eq!(
            app.tasks[0].url,
            format!("~/{dirname}/a.torrent"),
            "任务来源保留输入原文"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn confirm_add_empty_trackers_falls_back_to_auto_pool() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btdlg-fb");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = parse_at(&path);
        let mut app = make_app("fallback");
        app.cfg.udp_trackers = vec!["udp://cfg1/ann".into()];
        app.open_add_bt_dialog_with(&path, &meta);
        app.dialog.as_mut().unwrap().trackers.reset();
        app.dlg_confirm_add_bt(false);
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(
            app.tasks[0].bt.as_ref().unwrap().trackers,
            vec!["udp://cfg1/ann", "udp://seed/ann", "udp://l1/ann"],
            "面板清空确认 = 回退 config + torrent 自动池（BT 任务不留空 tracker）"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn confirm_add_duplicate_rejected() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btdlg-dup");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = parse_at(&path);
        let mut app = make_app("dup");
        app.open_add_bt_dialog_with(&path, &meta);
        app.dlg_confirm_add_bt(false);
        assert_eq!(app.tasks.len(), 1);
        app.open_add_bt_dialog_with(&path, &meta);
        app.dlg_confirm_add_bt(false);
        assert!(app.dialog.is_some(), "重复任务拒绝保持对话框");
        assert_eq!(app.toast.as_deref(), Some("⚠ 任务已存在"));
        assert_eq!(app.tasks.len(), 1);
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn confirm_add_duplicate_name_gets_seq_suffix() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btdlg-seq");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = parse_at(&path);
        let mut app = make_app("seq");
        app.open_add_bt_dialog_with(&path, &meta);
        app.dlg_confirm_add_bt(false);
        let path2 = write_multi_torrent(&dir, "b.torrent");
        app.open_add_bt_dialog_with(&path2, &meta);
        app.dlg_confirm_add_bt(false);
        assert_eq!(app.tasks.len(), 2);
        assert_eq!(
            app.tasks[1].name, "tor.1",
            "重名同目录 .1 递增（demo 口径）"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn modify_dialog_entry_routes_bt_and_completed() {
        let mut app = make_app("route");
        app.modify_dialog_entry();
        assert_eq!(app.toast.as_deref(), Some("没有选中的任务"));

        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btdlg-route");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = parse_at(&path);
        app.open_add_bt_dialog_with(&path, &meta);
        app.dlg_confirm_add_bt(false);
        app.selected = 0;
        app.modify_dialog_entry();
        assert_eq!(
            app.dialog.as_ref().unwrap().kind,
            DialogKind::ModifyBt,
            "BT 任务 → BT 修改框（FR-02-04③）"
        );
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.focus, 0, "焦点落在文件列表");
        assert_eq!(d.task_name, "tor", "任务名静态行");
        assert_eq!(d.conns.value, "20", "并发预填当前值");
        assert!(
            d.trackers
                .trim_lines()
                .contains(&"udp://seed/ann".to_string()),
            "trackers 预填任务当前池"
        );
        assert_eq!(d.bt_counts(), (2, 2), "文件树勾选状态保留");
        app.dialog = None;

        app.tasks[0].state = TaskState::Completed;
        app.filter = 1;
        app.selected = 0;
        app.modify_dialog_entry();
        assert!(app.dialog.is_none());
        assert_eq!(app.toast.as_deref(), Some("⚠ 已完成任务不可修改"));
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn confirm_modify_writes_back_and_keeps_pool_on_empty() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btdlg-mod");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = parse_at(&path);
        let mut app = make_app("mod");
        app.open_add_bt_dialog_with(&path, &meta);
        app.dlg_confirm_add_bt(false);

        app.selected = 0;
        app.modify_dialog_entry();
        {
            let d = app.dialog.as_mut().unwrap();
            d.bt_select_all(false);
            d.bt_expand_cursor(true);
            d.bt_list_move(3);
            d.bt_toggle_cursor();
            d.trackers.reset();
        }
        app.dlg_confirm_modify_bt();
        assert!(app.dialog.is_none(), "确认后关闭");
        let t = &app.tasks[0];
        assert_eq!(t.total, 7, "total 重算 = 选中合计");
        assert_eq!(
            t.bt.as_ref()
                .unwrap()
                .files
                .iter()
                .filter(|f| f.selected)
                .count(),
            1
        );
        assert_eq!(
            t.bt.as_ref().unwrap().trackers,
            vec!["udp://seed/ann", "udp://l1/ann"],
            "trackers 空输入写回保留原池"
        );
        assert_eq!(app.tasks[0].downloaded, 0, "本例原 downloaded = 0");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn confirm_modify_trackers_only_sends_hot_update_no_rebuild() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btdlg-hotupd");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = parse_at(&path);
        let mut app = make_app("hotupd");
        app.open_add_bt_dialog_with(&path, &meta);
        app.dlg_confirm_add_bt(false);
        app.selected = 0;
        app.tasks[0].state = TaskState::Downloading;

        let (spy_tx, mut spy_rx) = tokio::sync::mpsc::channel::<crate::engine::Cmd>(16);
        app.engine = crate::engine::EngineHandle::from_sender(spy_tx);

        app.modify_dialog_entry();
        {
            let d = app.dialog.as_mut().unwrap();
            let mut tr = d.trackers.trim_lines().join("\n");
            tr.push_str("\nudp://added/ann");
            d.trackers.set_text(&tr);
        }
        app.dlg_confirm_modify_bt();
        assert!(app.dialog.is_none(), "确认后关闭");
        assert_eq!(
            app.tasks[0].bt.as_ref().unwrap().trackers.last(),
            Some(&"udp://added/ann".to_string()),
            "新 tracker 已写入任务池（重开预填 = 新列表）"
        );

        let mut got = None;
        for _ in 0..4 {
            let c = tokio::time::timeout(std::time::Duration::from_millis(500), spy_rx.recv())
                .await
                .expect("应发出引擎命令")
                .expect("发送端存活");
            match c {
                crate::engine::Cmd::Reconfigure { .. } => continue,
                other => {
                    got = Some(other);
                    break;
                }
            }
        }
        match got.expect("应发出 BtUpdateTrackers") {
            crate::engine::Cmd::BtUpdateTrackers { id, trackers } => {
                assert_eq!(id, app.tasks[0].id);
                assert_eq!(
                    trackers.last(),
                    Some(&"udp://added/ann".to_string()),
                    "热更新携带确认后的新列表"
                );
                assert!(trackers.contains(&"udp://seed/ann".to_string()));
            }
            other => panic!("纯 tracker 修改应发 BtUpdateTrackers，实际: {other:?}"),
        }
        let more = tokio::time::timeout(std::time::Duration::from_millis(150), spy_rx.recv()).await;
        assert!(
            more.is_err(),
            "不得重挂引擎（无 CancelBt/StartBt），实际: {:?}",
            more.ok().flatten()
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn confirm_modify_zero_selection_rejected() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btdlg-mod0");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = parse_at(&path);
        let mut app = make_app("mod0");
        app.open_add_bt_dialog_with(&path, &meta);
        app.dlg_confirm_add_bt(false);
        app.selected = 0;
        app.modify_dialog_entry();
        app.dialog.as_mut().unwrap().bt_select_all(false);
        app.dlg_confirm_modify_bt();
        assert!(app.dialog.is_some(), "0 选择拒绝保持对话框");
        assert_eq!(
            app.toast.as_deref(),
            Some("⚠ 请至少保留一个要下载的文件（空格勾选）")
        );
        assert_eq!(
            app.dialog.as_ref().unwrap().focus,
            0,
            "焦点回文件列表（ModifyBt = 0）"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn make_bt_spec_maps_fields() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btdlg-spec");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = parse_at(&path);
        let mut app = make_app("spec");
        app.cfg.max_speed = 1024;
        app.cfg.udp_trackers = vec!["udp://cfg1/ann".into()];
        app.open_add_bt_dialog_with(&path, &meta);
        app.dialog.as_mut().unwrap().conns.set("7");
        app.dlg_confirm_add_bt(false);
        let t = &app.tasks[0];
        let spec = app.make_bt_spec(t, &meta);
        assert_eq!(spec.id, t.id);
        assert_eq!(spec.source, path);
        assert_eq!(spec.name, "tor");
        assert_eq!(spec.concurrency, 7);
        assert_eq!(spec.files.len(), 2);
        assert_eq!(spec.trackers.len(), 3, "config 1 + torrent 2（去重后）");
        assert!(!spec.seeding);
        assert_eq!(spec.max_speed, 1024);
        assert!(spec.proxy_endpoint.is_none(), "直连 = None");
        assert_eq!(spec.meta.infohash, meta.infohash);
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn bt_confirm_normalizes_tilde_and_relative_dir() {
        let Some(home) = crate::model::config::home_dir() else {
            return;
        };
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btdlg-norm");
        let path = write_multi_torrent(&dir, "n.torrent");
        let meta = parse_at(&path);
        let mut app = make_app("norm");
        app.open_add_bt_dialog_with(&path, &meta);
        if let Some(d) = app.dialog.as_mut() {
            d.dir.value = format!("~/ezr-bt-tilde-{}", std::process::id());
        }
        app.dlg_confirm_add_bt(false);
        assert_eq!(app.tasks.len(), 1, "任务创建");
        let expect = home
            .join(format!("ezr-bt-tilde-{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        assert_eq!(
            app.tasks[0].save_dir, expect,
            "BT 保存到 ~ 展开为主目录绝对路径（FR-02-02 v1.4/D62）"
        );
        app.shutdown().await;

        let mut app2 = make_app("norm-rel");
        let cwd = std::env::current_dir().unwrap();
        app2.open_add_bt_dialog_with(&path, &meta);
        if let Some(d) = app2.dialog.as_mut() {
            d.dir.value = "ezr-bt-rel-test".to_string();
        }
        app2.dlg_confirm_add_bt(false);
        assert_eq!(app2.tasks.len(), 1, "任务创建");
        assert_eq!(
            app2.tasks[0].save_dir,
            cwd.join("ezr-bt-rel-test").to_string_lossy().to_string(),
            "BT 保存到相对路径按创建时 cwd 转绝对（D62）"
        );
        app2.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
