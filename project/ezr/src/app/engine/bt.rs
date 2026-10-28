use super::App;
use crate::bt::SEED_DURATION_SECS;
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
            t.speed = t.speed.max(0.0);
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
