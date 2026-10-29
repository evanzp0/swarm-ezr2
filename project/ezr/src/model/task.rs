use super::config::ProxyChoice;
use super::namegen::{sidecar_path_of, DOWNLOADING_EXT};
use super::timefmt::fmt_created;
use super::{chunk, namegen, Checksum, FailKind, Protocol, TaskState};

#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
    pub id: usize,
    pub start: u64,
    pub end: u64,
    pub done: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnPhase {
    Connecting,
    Connected,
    Closed,
}

impl ConnPhase {
    #[must_use]
    pub fn is_active(self) -> bool {
        matches!(self, ConnPhase::Connecting | ConnPhase::Connected)
    }
}

impl Connection {
    #[must_use]
    pub fn cap(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub fn conn_phase(&self, cum_down: u64) -> ConnPhase {
        if self.cap() == 0 {
            if self.done > 0 {
                ConnPhase::Connected
            } else {
                ConnPhase::Closed
            }
        } else if cum_down > 0 {
            ConnPhase::Connected
        } else {
            ConnPhase::Connecting
        }
    }
}

#[must_use]
pub(crate) fn conn_transferring(cap: u64, done: u64) -> bool {
    if cap == 0 {
        done > 0
    } else {
        done < cap
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PieceState {
    Empty,
    Done,
    Partial,
    Active,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub id: u32,
    pub name: String,
    pub protocol: Protocol,
    pub url: String,
    pub name_locked: bool,
    pub save_dir: String,
    pub total: u64,
    pub downloaded: u64,
    pub speed: f64,
    pub state: TaskState,
    pub resumable: bool,
    pub probed: bool,
    pub connections: Vec<Connection>,
    pub chunk_done: u32,
    pub done_blocks: Vec<u32>,
    pub partial_blocks: Vec<u32>,
    pub active_blocks: Vec<u32>,
    pub block_size: u64,
    pub concurrency: usize,
    pub proxy: ProxyChoice,
    pub retries: u32,
    pub max_retries: u32,
    pub made_progress: bool,
    pub retry_in: Option<f64>,
    pub fail_kind: Option<FailKind>,
    pub error: Option<String>,
    pub invalidation_streak: u32,
    pub checksum: Option<Checksum>,
    pub verify_ok: Option<bool>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub has_slot: bool,
    #[allow(dead_code)]
    pub upload_speed: f64,
    #[allow(dead_code)]
    pub uploaded: u64,
    #[allow(dead_code)]
    pub seeders: usize,
    #[allow(dead_code)]
    pub peers: usize,
    #[allow(dead_code)]
    pub seed_left: f64,
    pub created: String,
    pub added_at: u64,
    pub bt: Option<super::bt::BtTaskData>,
}

impl Task {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new_queued(
        id: u32,
        name: String,
        protocol: Protocol,
        url: String,
        save_dir: String,
        block_size: u64,
        concurrency: usize,
        max_retries: u32,
        checksum: Option<Checksum>,
        proxy: ProxyChoice,
        added_at: u64,
    ) -> Task {
        Task {
            id,
            name,
            protocol,
            url,
            name_locked: false,
            save_dir,
            total: 0,
            downloaded: 0,
            speed: 0.0,
            state: TaskState::Queued,
            resumable: true,
            probed: false,
            connections: vec![],
            chunk_done: 0,
            done_blocks: Vec::new(),
            partial_blocks: Vec::new(),
            active_blocks: Vec::new(),
            block_size,
            concurrency,
            proxy,
            retries: 0,
            max_retries,
            made_progress: false,
            retry_in: None,
            fail_kind: None,
            error: None,
            invalidation_streak: 0,
            checksum,
            verify_ok: None,
            etag: None,
            last_modified: None,
            has_slot: false,
            upload_speed: 0.0,
            uploaded: 0,
            seeders: 0,
            peers: 0,
            seed_left: 0.0,
            created: fmt_created(added_at),
            added_at,
            bt: None,
        }
    }

    #[must_use]
    pub fn progress(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.downloaded as f64 / self.total as f64).clamp(0.0, 1.0)
        }
    }

    #[must_use]
    pub fn size_known(&self) -> bool {
        if self.protocol.is_bt() {
            return true;
        }
        if !matches!(self.protocol, Protocol::Http | Protocol::Https) {
            return true;
        }
        if self.resumable {
            return true;
        }
        matches!(
            self.state,
            TaskState::Verifying | TaskState::PostProcessing | TaskState::Completed
        )
    }

    #[must_use]
    pub fn eta_secs(&self) -> Option<u64> {
        if !self.size_known() {
            return None;
        }
        if self.speed <= 0.0 || self.state != TaskState::Downloading {
            return None;
        }
        let remain = self.total.saturating_sub(self.downloaded) as f64;
        Some((remain / self.speed) as u64)
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn thread_count(&self) -> usize {
        if self.state == TaskState::Downloading {
            self.connections
                .iter()
                .filter(|c| c.cap() > 0 && c.done < c.cap())
                .count()
        } else {
            self.connections.iter().filter(|c| c.cap() > 0).count()
        }
    }

    #[must_use]
    pub fn chunk_info(&self) -> (u32, u64, u64) {
        if let Some(bt) = &self.bt {
            let pl = u64::from(bt.piece_length).max(1);
            let total_all: u64 = bt.files.iter().map(|f| f.size).sum();
            let y = total_all.div_ceil(pl).max(1);
            return (self.chunk_done.min(y as u32), y, pl);
        }
        let y = chunk::chunk_total(self.total, self.block_size);
        (self.chunk_done.min(y), u64::from(y), self.block_size)
    }

    #[must_use]
    pub fn supports_pieces(&self) -> bool {
        if self.protocol.is_bt() {
            return true;
        }
        self.resumable
    }

    #[must_use]
    pub fn pieces(&self) -> Vec<PieceState> {
        let y = if let Some(bt) = &self.bt {
            let pl = u64::from(bt.piece_length).max(1);
            let total_all: u64 = bt.files.iter().map(|f| f.size).sum();
            (total_all.div_ceil(pl).max(1)) as usize
        } else {
            usize::try_from(chunk::chunk_total(self.total, self.block_size)).unwrap_or(0)
        };
        if y == 0 {
            return Vec::new();
        }
        let mut v = vec![PieceState::Empty; y];
        if self.done_blocks.is_empty() {
            let x = (self.chunk_done as usize).min(y);
            for s in v.iter_mut().take(x) {
                *s = PieceState::Done;
            }
        } else {
            for &i in &self.done_blocks {
                if let Some(idx) = usize::try_from(i).ok().filter(|&idx| idx < y) {
                    v[idx] = PieceState::Done;
                }
            }
        }
        let downloading = self.state == TaskState::Downloading;
        let partial_authoritative = !self.partial_blocks.is_empty();
        if self.bt.is_some() {
            // BT：在途/部分集合来自引擎真实状态（active_blocks/partial_blocks），
            // 不用 HTTP 式连续 lease 伪装——否则在途分块被画成从前沿连成一片。
            if downloading {
                for &i in &self.active_blocks {
                    if let Some(idx) = usize::try_from(i).ok().filter(|&idx| idx < y) {
                        if v[idx] != PieceState::Done {
                            v[idx] = PieceState::Active;
                        }
                    }
                }
            }
        } else {
            for c in &self.connections {
                if c.cap() == 0 {
                    continue;
                }
                let idx = (c.start / self.block_size) as usize;
                if idx >= y {
                    continue;
                }
                let s = if c.done >= c.cap() {
                    PieceState::Done
                } else if downloading {
                    PieceState::Active
                } else if partial_authoritative {
                    PieceState::Empty
                } else if c.done > 0 {
                    PieceState::Partial
                } else {
                    PieceState::Empty
                };
                if v[idx] != PieceState::Done || s == PieceState::Done {
                    v[idx] = s;
                }
            }
        }
        if partial_authoritative {
            for &i in &self.partial_blocks {
                if let Some(idx) = usize::try_from(i).ok().filter(|&idx| idx < y) {
                    if v[idx] == PieceState::Empty {
                        v[idx] = PieceState::Partial;
                    }
                }
            }
        }
        v
    }

    #[must_use]
    pub fn target_path(&self) -> String {
        namegen::join_path(&self.save_dir, &self.name)
    }

    #[must_use]
    pub fn downloading_path(&self) -> String {
        format!("{}{DOWNLOADING_EXT}", self.target_path())
    }

    #[must_use]
    pub fn sidecar_path(&self) -> String {
        sidecar_path_of(&self.save_dir, &self.name)
    }

    pub fn apply_chunk_snapshot(&mut self, downloaded: u64, chunk_done: u32) {
        let (conns, snap) =
            chunk::lease_snapshot(self.total, downloaded, self.concurrency, self.block_size);
        self.connections = conns;
        let multi_block = self.resumable && chunk::chunk_total(self.total, self.block_size) > 1;
        self.chunk_done = if multi_block {
            chunk_done
        } else {
            snap.max(chunk_done)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piece_state_covers_four_states() {
        let states = [
            PieceState::Empty,
            PieceState::Done,
            PieceState::Partial,
            PieceState::Active,
        ];
        for (i, a) in states.iter().enumerate() {
            for (j, b) in states.iter().enumerate() {
                assert_eq!(a == b, i == j, "四态互异：{a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn supports_pieces_bt_always_http_needs_resume() {
        let mut t = crate::model::sample_task();
        t.protocol = Protocol::Bt;
        t.resumable = false;
        assert!(t.supports_pieces(), "BT 恒支持（resumable 与门槛无关）");
        t.protocol = Protocol::Http;
        assert!(!t.supports_pieces(), "HTTP 不可续传 → 不支持分块");
        t.resumable = true;
        assert!(t.supports_pieces(), "HTTP 可续传 → 支持");
        t.protocol = Protocol::Https;
        assert!(t.supports_pieces(), "HTTPS 可续传 → 支持");
    }

    #[test]
    fn pieces_empty_file_returns_empty_vec() {
        let mut t = crate::model::sample_task();
        t.total = 0;
        t.probed = true;
        t.state = TaskState::Downloading;
        assert!(t.pieces().is_empty(), "total=0 无分块");
    }

    #[test]
    fn pieces_done_prefix_rest_empty_when_not_downloading() {
        let mut t = crate::model::sample_task();
        t.total = 6_000_000;
        t.block_size = 1_000_000;
        t.chunk_done = 2;
        t.state = TaskState::Paused;
        t.probed = true;
        let v = t.pieces();
        assert_eq!(v.len(), 6);
        assert_eq!(v[0], PieceState::Done);
        assert_eq!(v[1], PieceState::Done);
        assert!(v[2..].iter().all(|s| *s == PieceState::Empty));
        assert!(!v.contains(&PieceState::Active), "非下载中不出现 🟦");
    }

    #[test]
    fn pieces_active_when_downloading_assigned_even_zero_progress() {
        let mut t = crate::model::sample_task();
        t.total = 4_000_000;
        t.block_size = 1_000_000;
        t.chunk_done = 1;
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![
            Connection {
                id: 1,
                start: 1_000_000,
                end: 2_000_000,
                done: 0,
            },
            Connection {
                id: 2,
                start: 3_000_000,
                end: 4_000_000,
                done: 0,
            },
        ];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Done, "块 0 已完成");
        assert_eq!(v[1], PieceState::Active, "块 1 被持有（done=0 也算）");
        assert_eq!(v[2], PieceState::Empty, "块 2 无连接持有");
        assert_eq!(v[3], PieceState::Active, "块 3 被持有");
    }

    #[test]
    fn pieces_active_wins_over_partial_and_freezes_on_pause() {
        let mut t = crate::model::sample_task();
        t.total = 3_000_000;
        t.block_size = 1_000_000;
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![Connection {
            id: 1,
            start: 1_000_000,
            end: 2_000_000,
            done: 300_000,
        }];
        let v = t.pieces();
        assert_eq!(v[1], PieceState::Active, "🟦 优先于 🟨（下载中）");
        t.state = TaskState::Paused;
        let v = t.pieces();
        assert_eq!(v[1], PieceState::Partial, "非下载中：有零头 → 🟨");
        t.connections[0].done = 0;
        let v = t.pieces();
        assert_eq!(v[1], PieceState::Empty, "非下载中：零头 0 → ⬜");
    }

    #[test]
    fn pieces_completed_lease_shows_done() {
        let mut t = crate::model::sample_task();
        t.total = 4_000_000;
        t.block_size = 1_000_000;
        t.chunk_done = 1;
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![Connection {
            id: 2,
            start: 2_000_000,
            end: 3_000_000,
            done: 1_000_000,
        }];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Done, "前缀块 0");
        assert_eq!(v[1], PieceState::Empty, "块 1 无连接持有（跳过）");
        assert_eq!(v[2], PieceState::Done, "块 2 乱序写满 → 🟩");
        assert_eq!(v[3], PieceState::Empty);
    }

    #[test]
    fn pieces_out_of_order_done_set_shows_green() {
        let mut t = crate::model::sample_task();
        t.total = 6_000_000;
        t.block_size = 1_000_000;
        t.chunk_done = 2;
        t.done_blocks = vec![2, 5];
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![
            Connection {
                id: 1,
                start: 0,
                end: 1_000_000,
                done: 400_000,
            },
            Connection {
                id: 2,
                start: 1_000_000,
                end: 2_000_000,
                done: 0,
            },
            Connection {
                id: 3,
                start: 3_000_000,
                end: 4_000_000,
                done: 700_000,
            },
            Connection {
                id: 4,
                start: 4_000_000,
                end: 5_000_000,
                done: 100_000,
            },
        ];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Active);
        assert_eq!(v[1], PieceState::Active);
        assert_eq!(v[2], PieceState::Done, "乱序完成块 2：连接已移走仍须 🟩");
        assert_eq!(v[3], PieceState::Active);
        assert_eq!(v[4], PieceState::Active);
        assert_eq!(v[5], PieceState::Done, "乱序完成块 5：连接已移走仍须 🟩");
    }

    #[test]
    fn pieces_done_set_not_downgraded_by_frozen_lease() {
        let mut t = crate::model::sample_task();
        t.total = 3_000_000;
        t.block_size = 1_000_000;
        t.chunk_done = 1;
        t.done_blocks = vec![0, 2];
        t.state = TaskState::Paused;
        t.probed = true;
        t.connections = vec![Connection {
            id: 1,
            start: 2_000_000,
            end: 3_000_000,
            done: 300_000,
        }];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Done);
        assert_eq!(v[2], PieceState::Done, "已完成块不被冻结租约降级为 🟨");
    }

    #[test]
    fn pieces_failed_partial_set_shows_yellow() {
        let mut t = crate::model::sample_task();
        t.total = 5_000_000;
        t.block_size = 1_000_000;
        t.done_blocks = vec![0];
        t.partial_blocks = vec![1, 3];
        t.state = TaskState::Failed;
        t.probed = true;
        t.connections = vec![Connection {
            id: 1,
            start: 2_000_000,
            end: 3_000_000,
            done: 300_000,
        }];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Done, "块 0 已完成");
        assert_eq!(v[1], PieceState::Partial, "投影外的真部分块 1 → 🟨");
        assert_eq!(v[2], PieceState::Empty, "投影租约假零头不虚标（集合权威）");
        assert_eq!(v[3], PieceState::Partial, "投影外的真部分块 3 → 🟨");
        assert_eq!(v[4], PieceState::Empty, "块 4 无字节");
    }

    #[test]
    fn pieces_paused_partial_set_not_downgraded_by_idle_lease() {
        let mut t = crate::model::sample_task();
        t.total = 3_000_000;
        t.block_size = 1_000_000;
        t.done_blocks = vec![0, 2];
        t.partial_blocks = vec![1];
        t.state = TaskState::Paused;
        t.probed = true;
        t.connections = vec![Connection {
            id: 1,
            start: 1_000_000,
            end: 2_000_000,
            done: 0,
        }];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Done);
        assert_eq!(v[1], PieceState::Partial, "零头租约不把集合真值降级为 ⬜");
        assert_eq!(v[2], PieceState::Done);
    }

    #[test]
    fn pieces_downloading_partial_set_stays_active() {
        let mut t = crate::model::sample_task();
        t.total = 3_000_000;
        t.block_size = 1_000_000;
        t.done_blocks = vec![0];
        t.partial_blocks = vec![1];
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![Connection {
            id: 1,
            start: 1_000_000,
            end: 2_000_000,
            done: 400_000,
        }];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Done);
        assert_eq!(v[1], PieceState::Active, "下载中 🟦 口径不变");
        assert_eq!(v[2], PieceState::Empty);
    }

    #[test]
    fn pieces_downloading_unleased_partial_blocks_show_yellow() {
        let mut t = crate::model::sample_task();
        t.total = 6_000_000;
        t.block_size = 1_000_000;
        t.partial_blocks = vec![0, 1, 2, 3];
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![
            Connection {
                id: 1,
                start: 0,
                end: 1_000_000,
                done: 400_000,
            },
            Connection {
                id: 2,
                start: 1_000_000,
                end: 2_000_000,
                done: 0,
            },
        ];
        let v = t.pieces();
        assert_eq!(v.len(), 6);
        assert_eq!(v[0], PieceState::Active, "有租约的部分块 0 仍 🟦");
        assert_eq!(v[1], PieceState::Active, "有租约的块 1 🟦（分配即算）");
        assert_eq!(v[2], PieceState::Partial, "缩容回收的无租约部分块 2 → 🟨");
        assert_eq!(v[3], PieceState::Partial, "缩容回收的无租约部分块 3 → 🟨");
        assert_eq!(v[4], PieceState::Empty, "块 4 无字节");
        assert_eq!(v[5], PieceState::Empty, "块 5 无字节");
    }

    #[test]
    fn pieces_downloading_unleased_partial_not_downgrading_done() {
        let mut t = crate::model::sample_task();
        t.total = 5_000_000;
        t.block_size = 1_000_000;
        t.done_blocks = vec![2];
        t.partial_blocks = vec![1, 2, 3];
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![Connection {
            id: 1,
            start: 0,
            end: 1_000_000,
            done: 100_000,
        }];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Active);
        assert_eq!(v[1], PieceState::Partial, "无租约部分块 1 → 🟨");
        assert_eq!(v[2], PieceState::Done, "🟩 不被集合降级");
        assert_eq!(v[3], PieceState::Partial, "无租约部分块 3 → 🟨");
        assert_eq!(v[4], PieceState::Empty);
    }

    #[test]
    fn pieces_paused_empty_partial_set_falls_back_to_lease() {
        let mut t = crate::model::sample_task();
        t.total = 3_000_000;
        t.block_size = 1_000_000;
        t.done_blocks = Vec::new();
        t.partial_blocks = Vec::new();
        t.chunk_done = 1;
        t.state = TaskState::Paused;
        t.probed = true;
        t.connections = vec![Connection {
            id: 1,
            start: 2_000_000,
            end: 3_000_000,
            done: 300_000,
        }];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Done, "空集回退计数前缀");
        assert_eq!(v[2], PieceState::Partial, "回退租约零头口径（v129 行为）");
    }

    #[test]
    fn pieces_idle_and_out_of_range_leases_ignored() {
        let mut t = crate::model::sample_task();
        t.total = 2_000_000;
        t.block_size = 1_000_000;
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![
            Connection {
                id: 1,
                start: 0,
                end: 0,
                done: 0,
            },
            Connection {
                id: 2,
                start: 9_000_000,
                end: 10_000_000,
                done: 100,
            },
        ];
        let v = t.pieces();
        assert!(v.iter().all(|s| *s == PieceState::Empty), "两条连接均不计");
    }

    #[test]
    fn pieces_completed_task_all_done() {
        let mut t = crate::model::sample_task();
        t.total = 2_500_000;
        t.block_size = 1_000_000;
        t.chunk_done = 3;
        t.state = TaskState::Completed;
        t.probed = true;
        let v = t.pieces();
        assert_eq!(v.len(), 3);
        assert!(v.iter().all(|s| *s == PieceState::Done));
        t.chunk_done = 99;
        assert_eq!(t.pieces().len(), 3);
    }

    fn bt_pieces_task(state: TaskState) -> Task {
        use super::super::bt::{BtFile, BtTaskData};
        let mut t = crate::model::sample_task();
        t.protocol = Protocol::Bt;
        t.state = state;
        t.probed = true;
        t.total = 1_000;
        t.block_size = 100;
        t.bt = Some(BtTaskData {
            source: "/tmp/a.torrent".to_string(),
            trackers: vec![],
            piece_length: 100,
            files: vec![BtFile {
                path: "a.bin".to_string(),
                size: 1_000,
                selected: true,
            }],
        });
        t
    }

    #[test]
    fn bt_pieces_real_sets_scattered_not_contiguous() {
        let mut t = bt_pieces_task(TaskState::Downloading);
        t.done_blocks = vec![0, 5, 9];
        t.active_blocks = vec![2, 7];
        t.partial_blocks = vec![4];
        let v = t.pieces();
        let expect = [
            PieceState::Done,
            PieceState::Empty,
            PieceState::Active,
            PieceState::Empty,
            PieceState::Partial,
            PieceState::Done,
            PieceState::Empty,
            PieceState::Active,
            PieceState::Empty,
            PieceState::Done,
        ];
        assert_eq!(
            v, expect,
            "BT 分块 = 引擎真实 done/active/partial 三集合，离散分布不连片"
        );
        t.connections = vec![Connection {
            id: 1,
            start: 300,
            end: 1_000,
            done: 0,
        }];
        assert_eq!(
            t.pieces(),
            expect,
            "HTTP lease 不得参与 BT 上色（在途块不得连成一片）"
        );
    }

    #[test]
    fn bt_pieces_active_only_while_downloading() {
        let mut t = bt_pieces_task(TaskState::Paused);
        t.done_blocks = vec![0];
        t.active_blocks = vec![2];
        t.partial_blocks = vec![4];
        let v = t.pieces();
        assert_eq!(
            v,
            vec![
                PieceState::Done,
                PieceState::Empty,
                PieceState::Empty,
                PieceState::Empty,
                PieceState::Partial,
                PieceState::Empty,
                PieceState::Empty,
                PieceState::Empty,
                PieceState::Empty,
                PieceState::Empty,
            ],
            "暂停态不画在途蓝块，部分块保持黄"
        );
    }

    #[test]
    fn chunk_snapshot_multi_block_event_truth_beats_prefix_phantom() {
        let mut t = crate::model::sample_task();
        t.total = 15_000_000;
        t.block_size = 1_000_000;
        t.resumable = true;
        let downloaded = 11_000_000 + 600_000 + 1_000_000 + 500_000;
        t.apply_chunk_snapshot(downloaded, 12);
        assert_eq!(
            t.chunk_done, 12,
            "多块路径事件逐块真值权威，前缀幻影不得盖过"
        );
        let (x, y, piece) = t.chunk_info();
        assert_eq!((x, y, piece), (12, 15, 1_000_000));
        let (conns, _) = chunk::lease_snapshot(t.total, downloaded, t.concurrency, t.block_size);
        assert_eq!(t.connections, conns);
    }

    #[test]
    fn chunk_snapshot_single_stream_keeps_prefix_fallback() {
        let mut t = crate::model::sample_task();
        t.total = 15_000_000;
        t.block_size = 1_000_000;
        t.resumable = false;
        t.apply_chunk_snapshot(13_100_000, 0);
        assert_eq!(t.chunk_done, 13, "单流路径前缀快照精确，维持较大值兜底");
    }

    #[test]
    fn chunk_snapshot_single_block_keeps_prefix_fallback() {
        let mut t = crate::model::sample_task();
        t.total = 1_000_000;
        t.block_size = 1_000_000;
        t.resumable = true;
        t.apply_chunk_snapshot(600_000, 0);
        assert_eq!(t.chunk_done, 0);
        t.apply_chunk_snapshot(1_000_000, 0);
        assert_eq!(t.chunk_done, 1);
    }

    fn conn(id: usize, start: u64, end: u64, done: u64) -> Connection {
        Connection {
            id,
            start,
            end,
            done,
        }
    }

    #[test]
    fn conn_phase_standby_is_closed() {
        let c = conn(1, 0, 0, 0);
        assert_eq!(c.conn_phase(0), ConnPhase::Closed);
        assert!(!c.conn_phase(0).is_active());
        assert_eq!(c.conn_phase(100), ConnPhase::Closed);
    }

    #[test]
    fn conn_phase_streaming_single_thread_is_connected() {
        let c = conn(1, 0, 0, 4096);
        assert_eq!(c.conn_phase(0), ConnPhase::Connected);
        assert_eq!(c.conn_phase(4096), ConnPhase::Connected);
        assert!(c.conn_phase(4096).is_active());
        assert_eq!(conn(1, 0, 0, 0).conn_phase(4096), ConnPhase::Closed);
        assert_eq!(
            conn(2, 0, 1_000_000, 0).conn_phase(0),
            ConnPhase::Connecting
        );
    }

    #[test]
    fn conn_phase_holding_block_without_bytes_is_connecting() {
        let c = conn(2, 0, 1_000_000, 0);
        assert_eq!(c.conn_phase(0), ConnPhase::Connecting);
        let r = conn(3, 0, 1_000_000, 400_000);
        assert_eq!(r.conn_phase(0), ConnPhase::Connecting);
        assert!(r.conn_phase(0).is_active());
    }

    #[test]
    fn conn_phase_with_bytes_is_connected_and_stable() {
        let c = conn(4, 0, 1_000_000, 100);
        assert_eq!(c.conn_phase(1), ConnPhase::Connected);
        assert!(c.conn_phase(1).is_active());
        assert_eq!(c.conn_phase(1_000_000), ConnPhase::Connected);
    }

    #[test]
    fn conn_phase_is_active_exhaustive() {
        assert!(ConnPhase::Connecting.is_active());
        assert!(ConnPhase::Connected.is_active());
        assert!(!ConnPhase::Closed.is_active());
    }

    #[test]
    fn conn_transferring_gate_all_arms() {
        assert!(conn_transferring(0, 1));
        assert!(conn_transferring(0, 4096));
        assert!(!conn_transferring(0, 0));
        assert!(conn_transferring(1_000_000, 0));
        assert!(conn_transferring(1_000_000, 400_000));
        assert!(!conn_transferring(1_000_000, 1_000_000));
        assert!(!conn_transferring(1_000_000, 1_500_000));
        assert!(!conn_transferring(0, 0));
    }

    #[test]
    fn size_known_streaming_http_unknown_until_stream_ends() {
        let mut t = crate::model::sample_task();
        t.protocol = Protocol::Http;
        t.resumable = false;
        t.state = TaskState::Downloading;
        assert!(!t.size_known(), "流式下载期间大小未知");
        t.state = TaskState::Queued;
        assert!(!t.size_known(), "排队（重探窗口）同样未知");
        t.state = TaskState::Paused;
        assert!(!t.size_known(), "暂停未知");
        t.state = TaskState::Failed;
        assert!(!t.size_known(), "失败未知");
        for st in [
            TaskState::Verifying,
            TaskState::PostProcessing,
            TaskState::Completed,
        ] {
            t.state = st;
            assert!(t.size_known(), "流结束态 {st:?} 大小已知");
        }
    }

    #[test]
    fn size_known_resumable_and_bt_always_known_and_eta_gated() {
        let mut t = crate::model::sample_task();
        t.protocol = Protocol::Http;
        t.resumable = true;
        t.state = TaskState::Downloading;
        assert!(t.size_known());
        t.protocol = Protocol::Bt;
        t.resumable = false;
        assert!(t.size_known(), "BT 恒已知");
        t.protocol = Protocol::Http;
        t.resumable = true;
        t.state = TaskState::Downloading;
        t.total = 3_000_000;
        t.downloaded = 1_000_000;
        t.speed = 1_000_000.0;
        assert!(t.eta_secs().is_some(), "已知大小时 ETA 正常");
        t.resumable = false;
        assert!(t.eta_secs().is_none(), "未知大小时 ETA 无从估计");
    }
}
