use std::time::{Duration, Instant};

use crate::app::App;
use crate::engine::Cmd;
use crate::model::speed::SpeedWindow;
use crate::model::{slots, FailKind, Protocol, TaskState};

const SPEED_TICK: Duration = Duration::from_secs(1);

impl App {
    pub async fn tick(&mut self) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_tick)
            .as_secs_f64()
            .clamp(0.001, 0.5);
        self.last_tick = now;
        self.frame = self.frame.wrapping_add(1);

        self.tick_consume_events().await;
        self.tick_grant_and_start().await;
        self.tick_retry_countdown(dt).await;
        self.tick_speed_and_session(now);
        self.tick_housekeeping(now);
        self.tick_clamp_selection(now);
    }

    async fn tick_consume_events(&mut self) {
        loop {
            match self.evt_rx.try_recv() {
                Ok(evt) => self.on_evt(evt).await,
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
            }
        }
    }

    async fn tick_grant_and_start(&mut self) {
        let granted = {
            let tasks = &mut self.tasks;
            slots::allocate(tasks, self.max_slots)
        };
        for id in granted {
            let Some(idx) = self.tasks.iter().position(|t| t.id == id) else {
                continue;
            };
            if !self.disk_precheck(idx) {
                continue;
            }
            self.tasks[idx].probed = false;
            if self.tasks[idx].protocol == Protocol::Bt {
                self.start_bt_from_slot(idx).await;
                continue;
            }
            let choice = self.tasks[idx].proxy.clone();
            let endpoint = self.resolve_endpoint(&choice);
            let spec = self.make_spec(&self.tasks[idx], endpoint);
            self.engine.send(Cmd::Start { spec }).await;
        }
    }

    async fn start_bt_from_slot(&mut self, idx: usize) {
        let source = self.tasks[idx]
            .bt
            .as_ref()
            .map(|b| b.source.clone())
            .unwrap_or_else(|| self.tasks[idx].url.clone());
        let meta = std::fs::read(crate::model::config::expand_tilde(&source))
            .ok()
            .and_then(|b| crate::bt::metainfo::TorrentMeta::parse(&b).ok());
        let Some(meta) = meta else {
            let t = &mut self.tasks[idx];
            t.state = TaskState::Failed;
            t.fail_kind = Some(FailKind::Fatal);
            t.retry_in = None;
            t.has_slot = false;
            t.error = Some(format!("无法读取种子文件：{source}"));
            let name = t.name.clone();
            self.set_toast(format!(
                "✘ 无法读取种子文件：{name}，不自动重试，按 R 手动重试"
            ));
            return;
        };
        self.tasks[idx].state = TaskState::Downloading;
        let spec = Box::new(self.make_bt_spec(&self.tasks[idx], &meta));
        self.engine.send(Cmd::StartBt { spec }).await;
    }

    async fn tick_retry_countdown(&mut self, dt: f64) {
        let mut expired: Vec<u32> = Vec::new();
        for t in &mut self.tasks {
            if t.state == TaskState::Failed {
                if let Some(left) = t.retry_in.as_mut() {
                    *left -= dt;
                    if *left <= 0.0 {
                        t.retry_in = None;
                        t.state = TaskState::Queued;
                        t.error = None;
                        expired.push(t.id);
                    }
                }
            }
        }
        for id in &expired {
            self.send_retry_start(*id).await;
        }
        let retry_starts: Vec<u32> = self
            .tasks
            .iter()
            .filter(|t| {
                t.state == TaskState::Queued && t.has_slot && t.probed && !expired.contains(&t.id)
            })
            .map(|t| t.id)
            .collect();
        for id in retry_starts {
            self.send_retry_start(id).await;
        }
    }

    fn tick_speed_and_session(&mut self, now: Instant) {
        for t in self.tasks.iter_mut() {
            if t.state != TaskState::Downloading {
                if let Some(d) = self.speed_display.get_mut(&t.id) {
                    d.zero();
                }
                t.speed = 0.0;
            }
        }
        let speed_due = now.duration_since(self.last_speed_tick) >= SPEED_TICK;
        if speed_due {
            self.last_speed_tick = now;
            for t in self.tasks.iter_mut() {
                if t.state != TaskState::Downloading {
                    continue;
                }
                let sample = self.windows.get_mut(&t.id).map(|w| w.rate()).unwrap_or(0.0);
                let d = self.speed_display.entry(t.id).or_default();
                d.push(sample);
                t.speed = d.value();
            }
            self.sample_conn_speed_display();
        }
        let global_dl = self.global_dl_speed();
        let mut gained: u64 = 0;
        for t in self.tasks.iter() {
            match self.session_seen.get(&t.id).copied() {
                None => {
                    self.session_seen.insert(t.id, t.downloaded);
                }
                Some(prev) if t.downloaded > prev => {
                    gained += t.downloaded - prev;
                    self.session_seen.insert(t.id, t.downloaded);
                }
                Some(_) => {}
            }
        }
        self.session_bytes += gained;
        if speed_due {
            self.push_hist(global_dl, 0.0);
        }
    }

    pub(super) fn sample_conn_speed_display(&mut self) {
        for t in &self.tasks {
            if t.state != TaskState::Downloading {
                for ((tid, _), d) in self.conn_speed.iter_mut() {
                    if *tid == t.id {
                        d.zero();
                    }
                }
                continue;
            }
            for c in &t.connections {
                let key = (t.id, c.id);
                let active = crate::model::conn_transferring(c.cap(), c.done);
                let d = self.conn_speed.entry(key).or_default();
                if !active {
                    d.zero();
                    continue;
                }
                let sample = self
                    .conn_windows
                    .get_mut(&key)
                    .map(SpeedWindow::rate)
                    .unwrap_or(0.0);
                d.push(sample);
            }
        }
    }

    fn tick_housekeeping(&mut self, now: Instant) {
        if let Some(u) = self.toast_until {
            if now > u {
                self.toast = None;
                self.toast_until = None;
            }
        }
        if self.last_save.elapsed().as_secs() >= 5 {
            self.save_registry();
        }
    }

    fn tick_clamp_selection(&mut self, now: Instant) {
        let flen = self.filtered().len();
        if flen == 0 {
            self.selected = 0;
            self.scroll = 0;
        } else {
            if self.selected >= flen {
                self.selected = flen - 1;
            }
            let selection_changed = self.last_selected != self.selected;
            let vis = self.visible_rows.max(1);
            if selection_changed {
                if self.scroll > self.selected {
                    self.scroll = self.selected;
                }
                if self.selected >= self.scroll + vis {
                    self.scroll = self.selected + 1 - vis;
                }
            }
            let max_scroll = flen.saturating_sub(vis);
            self.scroll = self.scroll.min(max_scroll);
        }
        self.last_selected = self.selected;
        if self.conns_scroll_for != self.selected {
            self.conns_scroll_for = self.selected;
            self.conns_scroll = 0;
            self.conns_sel = 0;
            self.conns_hscroll = 0;
        }
        let clen = self
            .sel_task()
            .filter(|t| t.state.shows_conns())
            .map_or(0, |t| t.connections.len());
        if self.conns_sel >= clen.max(1) {
            self.conns_sel = clen.saturating_sub(1);
        }
        let cvis = self.visible_conns_rows.max(1);
        self.conns_scroll = self.conns_scroll.min(clen.saturating_sub(cvis));
        self.conns_hscroll = self.conns_hscroll.min(self.conns_max_hscroll);
        self.sb_hold_tick(now);
    }

    async fn send_retry_start(&mut self, id: u32) {
        let Some(idx) = self.tasks.iter().position(|t| t.id == id) else {
            return;
        };
        if self.tasks[idx].protocol == Protocol::Bt {
            if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                t.made_progress = false;
                t.probed = false;
            }
            self.start_bt_from_slot(idx).await;
            return;
        }
        let choice = self.tasks[idx].proxy.clone();
        let endpoint = self.resolve_endpoint(&choice);
        let spec = self.make_spec(&self.tasks[idx], endpoint);
        self.engine.send(Cmd::Start { spec }).await;
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.made_progress = false;
            t.probed = false;
        }
    }
}

#[cfg(test)]
mod bt_route_tests {
    use tokio::sync::mpsc;

    use super::*;
    use crate::engine::EngineHandle;
    use crate::model::BtTaskData;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-btroute", tag)
    }

    fn write_torrent(dir: &std::path::Path, file_name: &str) -> String {
        let pieces = [0u8; 20];
        let mut buf = Vec::new();
        buf.extend_from_slice(b"d8:announce14:udp://seed/ann");
        buf.extend_from_slice(b"13:announce-listl");
        buf.extend_from_slice(b"l12:udp://l1/annee");
        buf.extend_from_slice(b"4:infod6:lengthi12e4:name3:tor12:piece lengthi4e6:pieces20:");
        buf.extend_from_slice(&pieces);
        buf.push(b'e');
        buf.push(b'e');
        let path = dir.join(file_name);
        std::fs::write(&path, &buf).unwrap();
        path.to_string_lossy().to_string()
    }

    fn bt_task(id: u32, source: &str) -> crate::model::Task {
        let mut t = crate::model::Task::new_queued(
            id,
            "tor".to_string(),
            crate::model::Protocol::Bt,
            source.to_string(),
            "/dl".to_string(),
            4,
            20,
            3,
            None,
            crate::model::ProxyChoice::Direct,
            crate::model::unix_now(),
        );
        t.probed = true;
        t.total = 12;
        t.bt = Some(BtTaskData {
            source: source.to_string(),
            trackers: vec!["udp://seed/ann".to_string()],
            piece_length: 4,
            files: vec![crate::model::BtFile {
                path: "tor".to_string(),
                size: 12,
                selected: true,
            }],
            done_pieces: Vec::new(),
        });
        t
    }

    async fn recv_cmd(rx: &mut mpsc::Receiver<Cmd>) -> Cmd {
        tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("引擎命令应在超时前到达")
            .expect("发送端存活，命令应为 Some")
    }

    async fn assert_no_cmd(rx: &mut mpsc::Receiver<Cmd>) {
        let got = tokio::time::timeout(std::time::Duration::from_millis(120), rx.recv()).await;
        assert!(
            got.is_err(),
            "不应向引擎发送命令，实际收到: {:?}",
            got.ok().flatten()
        );
    }

    #[tokio::test]
    async fn bt_task_granted_slot_starts_bt_engine_not_http() {
        let mut app = make_app("grant-bt");
        let (spy_tx, mut spy_rx) = mpsc::channel::<Cmd>(8);
        app.engine = EngineHandle::from_sender(spy_tx);
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btroute-grant");
        let path = write_torrent(&dir, "a.torrent");
        app.tasks.push(bt_task(1, &path));
        app.tick_grant_and_start().await;
        match recv_cmd(&mut spy_rx).await {
            Cmd::StartBt { spec } => {
                assert_eq!(spec.id, 1);
                assert_eq!(spec.source, path, "BT 启动必须携带种子来源");
            }
            other => panic!("BT 任务获槽应发 StartBt，实际: {other:?}"),
        }
        assert_eq!(
            app.tasks[0].state,
            TaskState::Downloading,
            "BT 无探测流程：获槽启动即转下载中"
        );
        assert!(app.tasks[0].has_slot);
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn http_task_granted_slot_keeps_http_start() {
        let mut app = make_app("grant-http");
        let (spy_tx, mut spy_rx) = mpsc::channel::<Cmd>(8);
        app.engine = EngineHandle::from_sender(spy_tx);
        app.tasks.push(crate::model::sample_task());
        app.tick_grant_and_start().await;
        match recv_cmd(&mut spy_rx).await {
            Cmd::Start { spec } => {
                assert_eq!(spec.id, 1);
                assert_eq!(spec.url, "http://x/f.bin", "HTTP 任务获槽仍走 HTTP 引擎");
            }
            other => panic!("HTTP 任务获槽应发 Start，实际: {other:?}"),
        }
        assert_eq!(app.tasks[0].state, TaskState::Queued, "HTTP 等探测事件转态");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_task_with_missing_torrent_fails_fatal_without_engine_cmd() {
        let mut app = make_app("grant-bt-missing");
        let (spy_tx, mut spy_rx) = mpsc::channel::<Cmd>(8);
        app.engine = EngineHandle::from_sender(spy_tx);
        app.tasks.push(bt_task(1, "/nonexistent/dir/a.torrent"));
        app.tick_grant_and_start().await;
        assert_no_cmd(&mut spy_rx).await;
        let t = &app.tasks[0];
        assert_eq!(t.state, TaskState::Failed, "种子不可读 = 启动失败");
        assert_eq!(t.fail_kind, Some(FailKind::Fatal));
        assert!(!t.has_slot, "失败释放槽位");
        assert!(
            t.error
                .as_deref()
                .is_some_and(|m| m.contains("无法读取种子文件")),
            "错误文案: {:?}",
            t.error
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_retry_start_routes_to_bt_engine() {
        let mut app = make_app("retry-bt");
        let (spy_tx, mut spy_rx) = mpsc::channel::<Cmd>(8);
        app.engine = EngineHandle::from_sender(spy_tx);
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btroute-retry");
        let path = write_torrent(&dir, "r.torrent");
        let mut t = bt_task(7, &path);
        t.state = TaskState::Queued;
        t.has_slot = true;
        app.tasks.push(t);
        app.send_retry_start(7).await;
        match recv_cmd(&mut spy_rx).await {
            Cmd::StartBt { spec } => assert_eq!(spec.id, 7, "BT 自动重试必须走 BT 引擎"),
            other => panic!("BT 重试应发 StartBt，实际: {other:?}"),
        }
        assert_eq!(app.tasks[0].state, TaskState::Downloading);
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn confirm_add_bt_defers_engine_start_to_slot_grant() {
        let mut app = make_app("add-bt-defer");
        let (spy_tx, mut spy_rx) = mpsc::channel::<Cmd>(8);
        app.engine = EngineHandle::from_sender(spy_tx);
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btroute-add");
        let path = write_torrent(&dir, "a.torrent");
        let bytes = std::fs::read(&path).unwrap();
        let meta = crate::bt::metainfo::TorrentMeta::parse(&bytes).unwrap();
        app.open_add_bt_dialog_with(&path, &meta);
        app.dlg_confirm_add_bt(false);
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(
            app.tasks[0].state,
            TaskState::Queued,
            "确认后任务入等待队列（与 HTTP 同一排队口径）"
        );
        assert_no_cmd(&mut spy_rx).await;
        app.tick_grant_and_start().await;
        match recv_cmd(&mut spy_rx).await {
            Cmd::StartBt { spec } => assert_eq!(spec.id, app.tasks[0].id),
            other => panic!("获槽后应发 StartBt，实际: {other:?}"),
        }
        assert_eq!(app.tasks[0].state, TaskState::Downloading);
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
