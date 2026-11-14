use super::App;
use crate::model::{FailKind, TaskState};

impl App {
    pub(super) fn on_bt_progress(
        &mut self,
        id: u32,
        downloaded: u64,
        chunk_done: u32,
        done_blocks: Vec<u32>,
        partial_blocks: Vec<u32>,
        active_blocks: Vec<u32>,
        uploaded: u64,
    ) {
        let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
            return;
        };
        if !matches!(t.state, TaskState::Downloading | TaskState::Seeding) {
            return;
        }
        t.downloaded = downloaded;
        t.uploaded = uploaded;
        t.probed = true;
        if t.protocol.is_bt() {
            // BT：分块状态全部来自引擎真实报告（done/partial/active 三集合），
            // 不得走 HTTP 式 apply_chunk_snapshot 连续 lease——否则 pieces
            // 窗口把在途分块画成从前沿连成一片，与 libtorrent 随机选块相悖。
            t.chunk_done = chunk_done;
            t.connections.clear();
        } else {
            t.apply_chunk_snapshot(downloaded, chunk_done);
        }
        t.done_blocks = done_blocks;
        t.partial_blocks = partial_blocks;
        t.active_blocks = active_blocks;
    }

    pub(super) fn on_bt_done(&mut self, id: u32, total: u64) {
        self.bt_stopping.remove(&id);
        let name = match self.tasks.iter().find(|t| t.id == id) {
            Some(t) => t.name.clone(),
            None => return,
        };
        let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
            return;
        };
        if t.protocol.is_bt() && matches!(t.state, TaskState::Downloading | TaskState::Queued) {
            t.state = TaskState::Seeding;
            t.downloaded = total.max(t.downloaded);
            t.total = if t.total > 0 { t.total } else { total };
            t.speed = 0.0;
            let rule = self.cfg.seed_exit_rule;
            let rule_active = rule.is_some_and(|r| r.any_configured());
            if rule_active {
                t.seed_left = f64::INFINITY;
            } else {
                t.seed_left = (self.cfg.default_seed_duration_m * 60) as f64;
            }
            if rule.is_some_and(|r| r.seed_duration_m.is_some()) {
                t.seed_elapsed = 0.0;
            }
            if let Some(b) = t.bt.as_mut() {
                b.upload_paused = false;
            }
            self.bt_conns.remove(&id);
            self.clear_bt_conn_stats(id);
            self.set_toast(format!("✓ 下载完成: {name}"));
            self.save_registry();
        }
    }

    pub(super) fn on_bt_stopped(
        &mut self,
        id: u32,
        downloaded: u64,
        done_pieces: Vec<u32>,
        partial_pieces: Vec<u32>,
    ) {
        self.bt_stopping.remove(&id);
        let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
            return;
        };
        if !t.protocol.is_bt() {
            return;
        }
        // 引擎终态回报（Pause/StopSeed/Cancel）。
        match t.state {
            TaskState::Paused => {
                // 正常暂停流：app 在 toggle_pause 已乐观置 Paused（暂停目标态），
                // 落地最终 done/partial 集合与下载量，pieces 窗口立即还原真实
                // 状态（不靠重启后的 sidecar 回读）。
                if !done_pieces.is_empty() || !partial_pieces.is_empty() {
                    t.done_blocks = done_pieces;
                    t.partial_blocks = partial_pieces;
                    t.chunk_done = t.done_blocks.len() as u32;
                }
                t.downloaded = downloaded;
                t.active_blocks.clear();
                t.speed = 0.0;
                t.upload_speed = 0.0;
                t.has_slot = false;
                self.bt_conns.remove(&id);
                self.clear_bt_conn_stats(id);
            }
            TaskState::Completed => {
                // 停止做种流：状态已置 Completed，仅清理连接与速度显示。
                t.active_blocks.clear();
                t.speed = 0.0;
                t.upload_speed = 0.0;
                t.has_slot = false;
                self.bt_conns.remove(&id);
                self.clear_bt_conn_stats(id);
            }
            _ => {
                // Queued/Downloading/Seeding：旧引擎滞后退出的竞态回报，
                // 新引擎已接管任务——陈旧集合与 downloaded 不得覆盖新引擎
                // 进度，状态也不得回落 Paused，整体忽略。
            }
        }
    }

    pub(super) fn on_bt_failed(&mut self, id: u32, kind: FailKind, reason: String) {
        self.bt_stopping.remove(&id);
        self.handle_failure(id, kind, reason, None, true, 0, 0, Vec::new());
    }
}

#[cfg(test)]
mod bt_progress_speed_tests {
    use super::super::App;
    use crate::model::{FailKind, PieceState, TaskState};

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-bt-speed", tag)
    }

    fn bt_task(id: u32, state: TaskState) -> crate::model::Task {
        crate::app::testutil::bt_task(id, state)
    }

    #[tokio::test]
    async fn bt_task_speed_equals_conn_row_single_conn() {
        let mut app = make_app("bt-speed-single");
        app.tasks.push(bt_task(1, TaskState::Downloading));
        app.on_evt(crate::engine::Evt::BtConns {
            id: 1,
            conns: vec![bt_view(1, 1000, false)],
            availability: 0.0,
        })
        .await;
        app.on_evt(crate::engine::Evt::BtConns {
            id: 1,
            conns: vec![bt_view(1, 3000, false)],
            availability: 0.0,
        })
        .await;
        app.sample_bt_conn_speed_display();
        let (row_down, row_up) = app.bt_conn_speeds_of(1, 1);
        assert!(row_down > 0.0, "行速度来自滑窗: {row_down}");
        let t = &app.tasks[0];
        assert_eq!(
            t.speed, row_down,
            "单连接任务下载速度必须等于该行速度（同源 Σ）"
        );
        assert_eq!(
            t.upload_speed, row_up,
            "单连接任务上传速度必须等于该行上传速度（引擎碎片窗口和与逐块清零已废除）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_task_speed_sums_conn_rows_multi_conn() {
        let mut app = make_app("bt-speed-multi");
        app.tasks.push(bt_task(1, TaskState::Downloading));
        app.on_evt(crate::engine::Evt::BtConns {
            id: 1,
            conns: vec![bt_view(1, 1000, false), bt_view(2, 0, false)],
            availability: 0.0,
        })
        .await;
        app.on_evt(crate::engine::Evt::BtConns {
            id: 1,
            conns: vec![bt_view(1, 5000, false), bt_view(2, 2000, false)],
            availability: 0.0,
        })
        .await;
        app.sample_bt_conn_speed_display();
        let (d1, u1) = app.bt_conn_speeds_of(1, 1);
        let (d2, u2) = app.bt_conn_speeds_of(1, 2);
        let t = &app.tasks[0];
        assert!(
            (t.speed - (d1 + d2)).abs() < 1e-6,
            "任务下载速度 = Σ 行速度: {} vs {}+{}",
            t.speed,
            d1,
            d2
        );
        assert!(
            (t.upload_speed - (u1 + u2)).abs() < 1e-6,
            "任务上传速度 = Σ 行上传速度"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_seeding_upload_speed_from_conn_rows() {
        let mut app = make_app("bt-seed-speed");
        app.tasks.push(bt_task(1, TaskState::Seeding));
        app.on_evt(crate::engine::Evt::BtConns {
            id: 1,
            conns: vec![bt_view(1, 0, false)],
            availability: 0.0,
        })
        .await;
        app.on_evt(crate::engine::Evt::BtConns {
            id: 1,
            conns: vec![bt_view(1, 0, false)],
            availability: 0.0,
        })
        .await;
        app.on_evt(crate::engine::Evt::BtConns {
            id: 1,
            conns: vec![bt_view(1, 0, true)],
            availability: 0.0,
        })
        .await;
        app.sample_bt_conn_speed_display();
        let (_, row_up) = app.bt_conn_speeds_of(1, 1);
        let t = &app.tasks[0];
        assert_eq!(
            t.upload_speed, row_up,
            "做种任务上传速度与并发行同源（shows_conns 含 Seeding）"
        );
        assert_eq!(t.speed, 0.0, "做种任务无下载速度");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_progress_sets_real_piece_sets_without_lease_fabrication() {
        let mut app = make_app("bt-sets");
        app.tasks.push(bt_task(1, TaskState::Downloading));
        app.on_bt_progress(1, 8, 2, vec![0, 2], vec![], vec![1], 0);
        let t = &app.tasks[0];
        assert!(
            t.connections.is_empty(),
            "BT 不得构造 HTTP lease（在途块不得连成一片）"
        );
        assert_eq!(t.done_blocks, vec![0, 2]);
        assert_eq!(t.partial_blocks, Vec::<u32>::new());
        assert_eq!(t.active_blocks, vec![1]);
        let v = t.pieces();
        assert_eq!(
            v,
            vec![PieceState::Done, PieceState::Active, PieceState::Done],
            "已下载绿 + 在途蓝离散分布（3 piece：0/2 done，1 active）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_progress_partial_piece_shows_yellow_not_empty() {
        let mut app = make_app("bt-partial");
        app.tasks.push(bt_task(1, TaskState::Downloading));
        app.on_bt_progress(1, 4, 1, vec![0], vec![1], vec![], 0);
        let v = app.tasks[0].pieces();
        assert_eq!(
            v,
            vec![PieceState::Done, PieceState::Partial, PieceState::Empty],
            "已收块未验完的 piece 显示部分下载（不得显示成未下载）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_stopped_persists_final_piece_sets() {
        let mut app = make_app("bt-stop-sets");
        // 真实暂停流：app 在 toggle_pause 已乐观置 Paused，BtStopped 到达时
        // 任务处于 Paused（暂停目标态），终态集合与 downloaded 得以落地。
        app.tasks.push(bt_task(1, TaskState::Paused));
        app.on_bt_stopped(1, 8, vec![0, 1], vec![]);
        let t = &app.tasks[0];
        assert_eq!(t.state, TaskState::Paused);
        assert_eq!(t.done_blocks, vec![0, 1]);
        assert_eq!(t.chunk_done, 2);
        assert_eq!(t.downloaded, 8);
        assert!(t.active_blocks.is_empty(), "暂停后在途集合清空");
        let v = t.pieces();
        assert_eq!(
            v,
            vec![PieceState::Done, PieceState::Done, PieceState::Empty],
            "暂停后 pieces 窗口立即还原真实状态"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_stopped_while_restarting_does_not_clobber_new_engine_progress() {
        // 旧引擎滞后退出的竞态：BtStopped 到达时任务已被重新调度为
        // Downloading（新引擎在跑），陈旧终态不得覆盖新引擎进度与集合。
        let mut app = make_app("bt-stop-race");
        app.tasks.push(bt_task(1, TaskState::Downloading));
        app.on_bt_progress(1, 4096, 2, vec![0, 1], vec![], vec![], 0);
        app.on_bt_stopped(1, 8, vec![0, 1], vec![]);
        let t = &app.tasks[0];
        assert_eq!(
            t.state,
            TaskState::Downloading,
            "新引擎运行中不得回落 Paused"
        );
        assert_eq!(t.downloaded, 4096, "陈旧 downloaded 不得覆盖新引擎进度");
        assert_eq!(t.done_blocks, vec![0, 1], "新引擎已报的 done 集合保持");
        assert_eq!(t.chunk_done, 2);
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_stopped_clears_bt_stopping_registration() {
        let mut app = make_app("bt-stop-gate");
        app.tasks.push(bt_task(1, TaskState::Paused));
        app.bt_stopping.insert(1, std::time::Instant::now());
        app.on_bt_stopped(1, 0, vec![], vec![]);
        assert!(
            !app.bt_stopping.contains_key(&1),
            "BtStopped 确认旧引擎退出后必须清除 bt_stopping 登记（恢复放行）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_failed_clears_bt_stopping_registration() {
        let mut app = make_app("bt-fail-gate");
        app.tasks.push(bt_task(1, TaskState::Paused));
        app.bt_stopping.insert(1, std::time::Instant::now());
        app.on_bt_failed(1, FailKind::Fatal, "引擎任务中止".into());
        assert!(
            !app.bt_stopping.contains_key(&1),
            "BtFailed 后必须清除 bt_stopping 登记（避免任务永久卡在等待）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_done_resets_seed_left_and_clears_upload_pause() {
        let mut app = make_app("bt-done-d73");
        app.cfg.default_seed_duration_m = 30;
        let mut t = bt_task(1, TaskState::Downloading);
        t.seed_left = 1700.0;
        t.seed_elapsed = 900.0;
        t.bt.as_mut().unwrap().upload_paused = true;
        app.tasks.push(t);
        app.on_bt_done(1, 12);
        let t = &app.tasks[0];
        assert_eq!(t.state, TaskState::Seeding, "再次全部完成回做种态");
        assert_eq!(
            t.seed_left, 1800.0,
            "D73③/D79③：exit rule 未配置时 seed_left 重置 default_seed_duration_m（30 分钟）"
        );
        assert!(
            t.seed_elapsed > 0.0,
            "rule 未配置且无 seed_duration_m 时累计时长不动"
        );
        assert!(
            !t.bt.as_ref().unwrap().upload_paused,
            "D73②：再次全部完成上传恢复做种形态"
        );
        assert_eq!(
            app.toast.as_deref(),
            Some("✓ 下载完成: tor"),
            "D73⑤：再次完成 toast 照常出现"
        );
        app.shutdown().await;
    }

    fn bt_view(id: usize, cum: u64, seeder: bool) -> crate::engine::BtConnView {
        crate::engine::BtConnView {
            id,
            addr: format!("10.0.0.{id}:6881"),
            protocol: "TCP".into(),
            down_speed: 0.0,
            cum_down: cum,
            up_speed: 0.0,
            cum_up: cum / 2,
            client: "QB".into(),
            seeder,
            flags: "-".into(),
            relevance: 0.0,
        }
    }

    #[tokio::test]
    async fn bt_conns_feed_sliding_windows_and_seeder_flag() {
        let mut app = make_app("btwin");
        app.tasks.push(bt_task(1, TaskState::Downloading));
        app.on_evt(crate::engine::Evt::BtConns {
            id: 1,
            conns: vec![bt_view(1, 1000, true), bt_view(2, 0, false)],
            availability: 0.0,
        })
        .await;
        app.on_evt(crate::engine::Evt::BtConns {
            id: 1,
            conns: vec![bt_view(1, 3000, true), bt_view(2, 0, false)],
            availability: 0.0,
        })
        .await;
        assert_eq!(
            app.bt_conns.get(&1).map(|v| v[0].seeder),
            Some(true),
            "seeder 标志贯通到并发行"
        );
        app.sample_bt_conn_speed_display();
        let (down, up) = app.bt_conn_speeds_of(1, 1);
        assert!(
            down > 0.0,
            "每连接下载速度来自 1s 滑窗而非引擎瞬时值: {down}"
        );
        assert!(up > 0.0, "每连接上传速度同口径: {up}");
        let (down2, _) = app.bt_conn_speeds_of(1, 2);
        assert_eq!(down2, 0.0, "零累计连接速度为 0");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_conn_stats_pruned_when_conn_leaves_and_cleared_on_stop() {
        let mut app = make_app("btwin-prune");
        app.tasks.push(bt_task(1, TaskState::Downloading));
        app.on_evt(crate::engine::Evt::BtConns {
            id: 1,
            conns: vec![bt_view(1, 1000, false), bt_view(2, 1000, false)],
            availability: 0.0,
        })
        .await;
        app.on_evt(crate::engine::Evt::BtConns {
            id: 1,
            conns: vec![bt_view(1, 2000, false)],
            availability: 0.0,
        })
        .await;
        assert!(
            app.bt_conn_speeds_of(1, 2) == (0.0, 0.0),
            "离开的连接其速度统计被回收"
        );
        app.sample_bt_conn_speed_display();
        let (d1, _) = app.bt_conn_speeds_of(1, 1);
        assert!(d1 > 0.0, "仍在的连接统计保留: {d1}");

        // 真实暂停流：暂停先乐观置 Paused，随后 BtStopped 到达清理统计。
        app.tasks[0].state = TaskState::Paused;
        app.on_bt_stopped(1, 0, vec![], vec![]);
        assert_eq!(
            app.bt_conn_speeds_of(1, 1),
            (0.0, 0.0),
            "BtStopped 后该任务连接速度统计清零"
        );
        app.shutdown().await;
    }
}
