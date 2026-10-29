use std::time::Instant;

use super::App;
use crate::bt::SEED_DURATION_SECS;
use crate::model::speed::SpeedWindow;
use crate::model::{FailKind, TaskState};

impl App {
    pub(super) fn on_bt_progress(
        &mut self,
        id: u32,
        downloaded: u64,
        chunk_done: u32,
        done_blocks: Vec<u32>,
        partial_blocks: Vec<u32>,
        uploaded: u64,
        upload_speed: f64,
    ) {
        let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
            return;
        };
        if !matches!(t.state, TaskState::Downloading | TaskState::Seeding) {
            return;
        }
        t.downloaded = downloaded;
        t.uploaded = uploaded;
        t.upload_speed = upload_speed;
        t.probed = true;
        t.apply_chunk_snapshot(downloaded, chunk_done);
        t.done_blocks = done_blocks;
        t.partial_blocks = partial_blocks;
        if t.state == TaskState::Downloading {
            if let Some(w) = self.windows.get_mut(&id) {
                w.push(Instant::now(), downloaded);
            } else {
                let mut w = SpeedWindow::new();
                w.push(Instant::now(), downloaded);
                self.windows.insert(id, w);
            }
        }
    }

    pub(super) fn on_bt_done(&mut self, id: u32, total: u64) {
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
            if t.seed_left <= 0.0 {
                t.seed_left = SEED_DURATION_SECS;
            }
            self.bt_conns.remove(&id);
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
        let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
            return;
        };
        if !t.protocol.is_bt() {
            return;
        }
        if let Some(bt) = t.bt.as_mut() {
            bt.done_pieces = done_pieces;
        }
        let _ = partial_pieces;
        t.downloaded = downloaded;
        t.speed = 0.0;
        t.upload_speed = 0.0;
        t.has_slot = false;
        self.bt_conns.remove(&id);
        if matches!(t.state, TaskState::Downloading | TaskState::Seeding) {
            t.state = TaskState::Paused;
        }
        self.windows.remove(&id);
    }

    pub(super) fn on_bt_failed(&mut self, id: u32, kind: FailKind, reason: String) {
        self.handle_failure(id, kind, reason, None, true, 0, 0, Vec::new());
    }
}

#[cfg(test)]
mod bt_progress_speed_tests {
    use std::time::{Duration, Instant};

    use super::super::App;
    use crate::model::{sample_task, BtFile, BtTaskData, Protocol, TaskState};

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-bt-speed", tag)
    }

    fn bt_task(id: u32, state: TaskState) -> crate::model::Task {
        let mut t = sample_task();
        t.id = id;
        t.protocol = Protocol::Bt;
        t.name = "tor".to_string();
        t.url = "/tmp/tor.torrent".to_string();
        t.state = state;
        t.probed = true;
        t.total = 12;
        t.bt = Some(BtTaskData {
            source: "/tmp/tor.torrent".to_string(),
            trackers: vec!["udp://seed/ann".to_string()],
            piece_length: 4,
            files: vec![BtFile {
                path: "tor".to_string(),
                size: 12,
                selected: true,
            }],
            done_pieces: Vec::new(),
        });
        t
    }

    #[tokio::test]
    async fn bt_progress_feeds_speed_window_and_task_speed() {
        let mut app = make_app("window-fed");
        app.tasks.push(bt_task(1, TaskState::Downloading));
        app.on_bt_progress(1, 4096, 1, vec![], vec![], 0, 0.0);
        app.on_bt_progress(1, 8192, 2, vec![0], vec![], 0, 0.0);
        assert!(
            app.windows.contains_key(&1),
            "BT 进度必须喂任务速度滑窗（与 HTTP Evt::Progress 同口径）"
        );
        app.last_speed_tick -= Duration::from_secs(2);
        app.tick_speed_and_session(Instant::now());
        assert!(
            app.tasks[0].speed > 0.0,
            "下载中的 BT 任务速度必须非 0，实际={}",
            app.tasks[0].speed
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_progress_while_seeding_does_not_recreate_speed_window() {
        let mut app = make_app("seed-no-window");
        app.tasks.push(bt_task(1, TaskState::Seeding));
        app.on_bt_progress(1, 8192, 2, vec![0], vec![], 0, 0.0);
        assert!(
            !app.windows.contains_key(&1),
            "做种态不重建速度滑窗（on_bt_done 已清窗，滑窗生命周期只属下载态）"
        );
        app.shutdown().await;
    }
}
