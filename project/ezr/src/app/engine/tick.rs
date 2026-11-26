use std::collections::HashMap;
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
        self.tick_seed_countdown(dt);
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
            if self.tasks[idx].protocol == Protocol::Bt && self.bt_stopping.contains_key(&id) {
                // 旧 BT 引擎尚未确认退出（BtStopped 未到）：先归还槽位，
                // 等 BtStopped 清除登记后下一轮重新授予并启动。
                self.tasks[idx].has_slot = false;
                continue;
            }
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

    /// 做种退出评估与自动停止（09-bt-engine-13/23，QA-BE-13/23）。
    /// 未配置 [bt.seed_exit_rule] 时走 default_seed_duration_m 倒计时（逐帧 dt
    /// 累计、归零即停）；任一键配置时倒计时失效，改按 AND 评估已配置条件
    /// （share_ratio_gt 实时分享率、seeders_count_gt 实时 x1 连接种子、
    /// seed_duration_m 做种累计时长），随 tick 秒级评估、满足即时收尾。
    /// 做种累计时长仅 Seeding 态累加，跨暂停/重启经注册表续走（FR-02-22）。
    /// 归零/满足落点与空格提前停止共用 finish_seeding（仅 toast 文案不同）。
    fn tick_seed_countdown(&mut self, dt: f64) {
        let rule = self.cfg.seed_exit_rule;
        let rule_active = rule.is_some_and(|r| r.any_configured());
        let mut expired: Vec<u32> = Vec::new();
        for t in &mut self.tasks {
            if t.state != TaskState::Seeding {
                continue;
            }
            t.seed_elapsed += dt;
            if rule_active {
                let r = rule.unwrap_or(crate::model::config::SeedExitRule {
                    share_ratio_gt: None,
                    seeders_count_gt: None,
                    seed_duration_m: None,
                });
                let ratio_ok = r.share_ratio_gt.is_none_or(|a| t.share_ratio_value() >= a);
                let seeds_ok = r.seeders_count_gt.is_none_or(|b| (t.seeders as u64) >= b);
                let dur_ok = r
                    .seed_duration_m
                    .is_none_or(|c| t.seed_elapsed >= (c * 60) as f64);
                if ratio_ok && seeds_ok && dur_ok {
                    expired.push(t.id);
                }
            } else {
                t.seed_left -= dt;
                if t.seed_left <= 0.0 {
                    t.seed_left = 0.0;
                    expired.push(t.id);
                }
            }
        }
        for id in expired {
            let Some(idx) = self.tasks.iter().position(|t| t.id == id) else {
                continue;
            };
            let name = self.tasks[idx].name.clone();
            self.finish_seeding(idx, format!("✓ 做种结束: {name}"));
        }
    }

    pub(super) fn tick_speed_and_session(&mut self, now: Instant) {
        for t in self.tasks.iter_mut() {
            if t.state != TaskState::Downloading {
                t.speed = 0.0;
            }
        }
        let speed_due = now.duration_since(self.last_speed_tick) >= SPEED_TICK;
        if speed_due {
            self.last_speed_tick = now;
            // HTTP 与 BT 同口径（FR-01-102 v1.39/D63③）：任务速度一律由
            // per-conn EMA 汇总（Σ 与并发 panel 行同源），独立任务级 EMA 链废除。
            self.sample_conn_speed_display();
            self.sample_bt_conn_speed_display();
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
        let mut totals: HashMap<u32, f64> = HashMap::new();
        for t in &self.tasks {
            if t.state != TaskState::Downloading {
                for ((tid, _), d) in self.conn_speed.iter_mut() {
                    if *tid == t.id {
                        d.zero();
                    }
                }
                continue;
            }
            let mut sum = 0.0f64;
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
                sum += d.value();
            }
            totals.insert(t.id, sum);
        }
        for t in self.tasks.iter_mut() {
            if t.state == TaskState::Downloading {
                t.speed = totals.get(&t.id).copied().unwrap_or(0.0);
            }
        }
    }

    pub(super) fn sample_bt_conn_speed_display(&mut self) {
        let task_ids: Vec<u32> = self.bt_conns.keys().copied().collect();
        let mut totals: HashMap<u32, (f64, f64)> = HashMap::new();
        for tid in task_ids {
            let active = self
                .tasks
                .iter()
                .find(|t| t.id == tid)
                .is_some_and(|t| t.state.shows_conns());
            let Some(rows) = self.bt_conns.get(&tid) else {
                continue;
            };
            let conn_ids: Vec<usize> = rows.iter().map(|r| r.id).collect();
            let mut sum = (0.0f64, 0.0f64);
            for cid in conn_ids {
                let key = (tid, cid);
                if !active {
                    if let Some(st) = self.bt_conn_stats.get_mut(&key) {
                        st.down_ema.zero();
                        st.up_ema.zero();
                    }
                    continue;
                }
                if let Some(st) = self.bt_conn_stats.get_mut(&key) {
                    let (down, up) = (st.down_win.rate(), st.up_win.rate());
                    st.down_ema.push(down);
                    st.up_ema.push(up);
                    sum.0 += st.down_ema.value();
                    sum.1 += st.up_ema.value();
                }
            }
            if active {
                totals.insert(tid, sum);
            }
        }
        // 任务速度 = Σ per-conn EMA（与并发 panel 行完全同源）：单连接时
        // 任务速度 == 该行速度（构造上相等），多连接时等于各行之和。
        // 引擎逐块碎片窗口和与逐块事件 upload_speed=0 的旧口径已废除。
        for t in self.tasks.iter_mut() {
            if t.protocol.is_bt() && t.state.shows_conns() {
                match totals.get(&t.id) {
                    Some(&(down, up)) => {
                        t.upload_speed = up;
                        if t.state == TaskState::Downloading {
                            t.speed = down;
                        }
                    }
                    None => {
                        t.upload_speed = 0.0;
                        if t.state == TaskState::Downloading {
                            t.speed = 0.0;
                        }
                    }
                }
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
        // bt_stopping 兜底：正常路径 BtStopped/BtFailed 必然清除登记；
        // 引擎任务若异常挂死（不退也不报），登记滞留会让任务永远无法
        // 再启动，故超时强制放行（正常停止在秒级完成，180s 极宽裕）。
        const BT_STOPPING_FAILOPEN_SECS: u64 = 180;
        self.bt_stopping
            .retain(|_, at| now.duration_since(*at).as_secs() < BT_STOPPING_FAILOPEN_SECS);
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
            .map_or(0, |t| {
                if t.protocol.is_bt() {
                    self.bt_conns.get(&t.id).map_or(0, Vec::len)
                } else {
                    t.connections.len()
                }
            });
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
    use crate::app::testutil::{assert_no_cmd, recv_cmd};
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
            upload_paused: false,
            sequential: false,
        });
        t
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
    async fn bt_task_in_bt_stopping_waits_for_old_engine_exit() {
        // 旧引擎尚未确认退出（bt_stopping 在册）时不得拉起新引擎：
        // 双引擎同写文件与 sidecar 是「暂停→继续进度清零」的根因。
        let mut app = make_app("gate-bt-stopping");
        let (spy_tx, mut spy_rx) = mpsc::channel::<Cmd>(8);
        app.engine = EngineHandle::from_sender(spy_tx);
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btroute-gate");
        let path = write_torrent(&dir, "a.torrent");
        app.tasks.push(bt_task(1, &path));
        app.bt_stopping.insert(1, std::time::Instant::now());
        app.tick_grant_and_start().await;
        assert_no_cmd(&mut spy_rx).await;
        assert_eq!(
            app.tasks[0].state,
            TaskState::Queued,
            "登记未清除前任务保持排队等待"
        );
        assert!(
            !app.tasks[0].has_slot,
            "跳过启动时归还槽位，登记清除后下一轮重新授予"
        );
        // 登记 BtStopped 清除后，下一轮调度正常启动。
        app.bt_stopping.remove(&1);
        app.tick_grant_and_start().await;
        match recv_cmd(&mut spy_rx).await {
            Cmd::StartBt { spec } => assert_eq!(spec.id, 1),
            other => panic!("登记清除后应正常启动，实际: {other:?}"),
        }
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn bt_stopping_failsafe_expires_stale_registration() {
        // 兜底：登记滞留（引擎挂死既不退也不报）超时后放行，
        // 任务不能被永久卡死。
        let mut app = make_app("gate-bt-failsafe");
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-btroute-failsafe");
        app.tasks.push(bt_task(1, "unused.torrent"));
        let stale = std::time::Instant::now() - std::time::Duration::from_secs(600);
        app.bt_stopping.insert(1, stale);
        app.tick_housekeeping(std::time::Instant::now());
        assert!(
            !app.bt_stopping.contains_key(&1),
            "超时兜底必须清除过期登记"
        );
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

#[cfg(test)]
mod seed_countdown_tests {
    use tokio::sync::mpsc;

    use super::*;
    use crate::app::testutil::{assert_no_cmd, recv_cmd};
    use crate::engine::EngineHandle;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-seed", tag)
    }

    fn seeding_bt(id: u32) -> crate::model::Task {
        let mut t = crate::model::Task::new_queued(
            id,
            "seed.bin".to_string(),
            crate::model::Protocol::Bt,
            "/tmp/a.torrent".to_string(),
            "/tmp".to_string(),
            1 << 20,
            4,
            3,
            None,
            crate::model::ProxyChoice::Direct,
            crate::model::unix_now(),
        );
        t.state = TaskState::Seeding;
        t.upload_speed = 4096.0;
        t.uploaded = 123_456;
        t.seed_left = 1800.0;
        t
    }

    #[tokio::test]
    async fn seed_countdown_decrements_only_while_seeding() {
        let mut app = make_app("seed-dec");
        let (spy_tx, mut spy_rx) = mpsc::channel::<Cmd>(8);
        app.engine = EngineHandle::from_sender(spy_tx);
        let mut dl = seeding_bt(2);
        dl.state = TaskState::Downloading;
        dl.seed_left = 999.0;
        app.tasks.push(seeding_bt(1));
        app.tasks.push(dl);
        app.tick_seed_countdown(1.0);
        assert_eq!(app.tasks[0].seed_left, 1799.0, "做种中逐帧递减");
        assert_eq!(app.tasks[0].state, TaskState::Seeding, "未归零不转态");
        assert_eq!(
            app.tasks[1].seed_left, 999.0,
            "仅 Seeding 态递减，下载中任务不动"
        );
        assert_no_cmd(&mut spy_rx).await;
        app.shutdown().await;
    }

    #[tokio::test]
    async fn seed_countdown_zero_stops_seeding_to_completed() {
        let mut app = make_app("seed-zero");
        let (spy_tx, mut spy_rx) = mpsc::channel::<Cmd>(8);
        app.engine = EngineHandle::from_sender(spy_tx);
        let mut t = seeding_bt(1);
        t.seed_left = 1.0;
        app.tasks.push(t);
        app.tick_seed_countdown(2.0);
        let t = &app.tasks[0];
        assert_eq!(
            t.state,
            TaskState::Completed,
            "倒计时归零 → 已完成（09-bt-engine-13 归零臂）"
        );
        assert_eq!(t.seed_left, 0.0, "归零清底");
        assert_eq!(t.upload_speed, 0.0, "上传速度归零");
        assert!(!t.has_slot, "释放槽位");
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("✓ 做种结束: seed.bin")),
            "归零 toast: {:?}",
            app.toast
        );
        match recv_cmd(&mut spy_rx).await {
            Cmd::StopSeed { id } => {
                assert_eq!(id, 1, "引擎收到停止做种（tracker stopped 载体）")
            }
            other => panic!("归零应发 StopSeed，实际: {other:?}"),
        }
        // 已离做种态：再次递减不重复触发（无第二条命令、状态不再变化）
        app.tick_seed_countdown(2.0);
        assert_eq!(app.tasks[0].state, TaskState::Completed);
        assert_no_cmd(&mut spy_rx).await;
        app.shutdown().await;
    }

    #[tokio::test]
    async fn full_tick_drives_seed_countdown() {
        let mut app = make_app("seed-tick-wire");
        let (spy_tx, mut spy_rx) = mpsc::channel::<Cmd>(8);
        app.engine = EngineHandle::from_sender(spy_tx);
        app.tasks.push(seeding_bt(1));
        app.tick().await;
        assert!(
            app.tasks[0].seed_left < 1800.0,
            "主 tick 必须驱动做种倒计时递减（本轮缺口回归锁）: {}",
            app.tasks[0].seed_left
        );
        assert_eq!(app.tasks[0].state, TaskState::Seeding);
        assert_no_cmd(&mut spy_rx).await;
        app.shutdown().await;
    }

    #[tokio::test]
    async fn seed_exit_rule_share_ratio_only_arm_exits_by_ratio() {
        let mut app = make_app("seed-rule-ratio");
        let (spy_tx, mut spy_rx) = mpsc::channel::<Cmd>(8);
        app.engine = EngineHandle::from_sender(spy_tx);
        app.cfg.seed_exit_rule = Some(crate::model::config::SeedExitRule {
            share_ratio_gt: Some(1.5),
            seeders_count_gt: None,
            seed_duration_m: None,
        });
        let mut t = seeding_bt(1);
        t.seed_left = f64::INFINITY;
        t.seed_elapsed = 600.0;
        t.total = 3_000_000;
        t.downloaded = 3_000_000;
        t.uploaded = 3_000_000;
        app.tasks.push(t);
        app.tick_seed_countdown(1.0);
        assert_eq!(
            app.tasks[0].state,
            TaskState::Seeding,
            "分享率 1.0 < 1.5 不退出"
        );
        app.tasks[0].uploaded = 4_500_000;
        app.tick_seed_countdown(1.0);
        assert_eq!(
            app.tasks[0].state,
            TaskState::Completed,
            "分享率达 1.5 即退出（时长臂未配置不参与）"
        );
        match recv_cmd(&mut spy_rx).await {
            Cmd::StopSeed { id } => assert_eq!(id, 1),
            other => panic!("rule 满足应发 StopSeed，实际: {other:?}"),
        }
        app.shutdown().await;
    }

    #[tokio::test]
    async fn seed_exit_rule_and_combination_and_x1_semantics() {
        let mut app = make_app("seed-rule-and");
        let (spy_tx, mut spy_rx) = mpsc::channel::<Cmd>(8);
        app.engine = EngineHandle::from_sender(spy_tx);
        app.cfg.seed_exit_rule = Some(crate::model::config::SeedExitRule {
            share_ratio_gt: Some(0.5),
            seeders_count_gt: Some(2),
            seed_duration_m: None,
        });
        let mut t = seeding_bt(1);
        t.seed_left = f64::INFINITY;
        t.total = 1_000_000;
        t.downloaded = 1_000_000;
        t.uploaded = 1_000_000;
        t.seeders = 1;
        t.peers = 3;
        app.tasks.push(t);
        app.tick_seed_countdown(0.5);
        assert_eq!(
            app.tasks[0].state,
            TaskState::Seeding,
            "AND 判定：分享率满足但 x1 < 2 不退出"
        );
        app.tasks[0].seeders = 2;
        app.tick_seed_countdown(0.5);
        assert_eq!(
            app.tasks[0].state,
            TaskState::Completed,
            "x1（连接种子数）达 2 且分享率满足 → 退出"
        );
        match recv_cmd(&mut spy_rx).await {
            Cmd::StopSeed { id } => assert_eq!(id, 1),
            other => panic!("AND 满足应发 StopSeed，实际: {other:?}"),
        }
        app.shutdown().await;
    }

    #[tokio::test]
    async fn seed_exit_rule_duration_arm_uses_cumulative_elapsed() {
        let mut app = make_app("seed-rule-dur");
        let (spy_tx, mut spy_rx) = mpsc::channel::<Cmd>(8);
        app.engine = EngineHandle::from_sender(spy_tx);
        app.cfg.seed_exit_rule = Some(crate::model::config::SeedExitRule {
            share_ratio_gt: None,
            seeders_count_gt: None,
            seed_duration_m: Some(1),
        });
        let mut t = seeding_bt(1);
        t.seed_left = f64::INFINITY;
        t.seed_elapsed = 50.0;
        app.tasks.push(t);
        app.tick_seed_countdown(5.0);
        assert_eq!(
            app.tasks[0].state,
            TaskState::Seeding,
            "累计 55s < 60s 不退出（default_seed_duration_m 失效，无 30 分钟倒计时）"
        );
        app.tick_seed_countdown(10.0);
        assert_eq!(
            app.tasks[0].state,
            TaskState::Completed,
            "累计时长 ≥ 60s 退出（跨暂停续走由注册表持久化）"
        );
        match recv_cmd(&mut spy_rx).await {
            Cmd::StopSeed { id } => assert_eq!(id, 1),
            other => panic!("时长臂满足应发 StopSeed，实际: {other:?}"),
        }
        app.shutdown().await;
    }

    #[tokio::test]
    async fn seed_exit_rule_active_disables_default_countdown() {
        let mut app = make_app("seed-rule-no-countdown");
        let (spy_tx, mut spy_rx) = mpsc::channel::<Cmd>(8);
        app.engine = EngineHandle::from_sender(spy_tx);
        app.cfg.seed_exit_rule = Some(crate::model::config::SeedExitRule {
            share_ratio_gt: Some(9.9),
            seeders_count_gt: None,
            seed_duration_m: None,
        });
        let mut t = seeding_bt(1);
        t.seed_left = 1.0;
        t.total = 1_000_000;
        t.downloaded = 1_000_000;
        t.uploaded = 0;
        app.tasks.push(t);
        app.tick_seed_countdown(5.0);
        assert_eq!(
            app.tasks[0].state,
            TaskState::Seeding,
            "rule 生效臂不跑 default 倒计时（旧 seed_left 不驱动退出）"
        );
        assert!(app.tasks[0].seed_left > 0.0, "seed_left 不被递减");
        assert_no_cmd(&mut spy_rx).await;
        app.shutdown().await;
    }
}
