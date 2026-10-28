pub mod bt;
pub mod checksum;
pub mod chunk;
pub mod config;
pub mod consistency;
pub mod idgen;
pub mod namegen;
pub mod registry;
pub mod retry;
pub mod sidecar;
pub mod sizefmt;
pub mod slots;
pub mod speed;

#[cfg(test)]
pub(crate) mod testenv;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Protocol {
    Http,
    Https,
    #[allow(dead_code)]
    Bt,
}

impl Protocol {
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Protocol::Http => "HTTP",
            Protocol::Https => "HTTPS",
            Protocol::Bt => "BT",
        }
    }

    #[must_use]
    pub fn is_bt(&self) -> bool {
        matches!(self, Protocol::Bt)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum TaskState {
    Queued,
    Downloading,
    Paused,
    Verifying,
    #[allow(dead_code)]
    PostProcessing,
    Completed,
    Failed,
    #[allow(dead_code)]
    Seeding,
}

impl TaskState {
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            TaskState::Queued => "等待中",
            TaskState::Downloading => "下载中",
            TaskState::Paused => "已暂停",
            TaskState::Verifying => "校验中",
            TaskState::PostProcessing => "后期处理中",
            TaskState::Completed => "已完成",
            TaskState::Failed => "已失败",
            TaskState::Seeding => "做种中",
        }
    }

    #[must_use]
    pub fn is_done(&self) -> bool {
        matches!(self, TaskState::Completed | TaskState::Seeding)
    }

    #[must_use]
    pub fn shows_conns(&self) -> bool {
        matches!(self, TaskState::Downloading | TaskState::Seeding)
    }
}

#[must_use]
pub fn is_http_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

#[must_use]
pub fn protocol_of_url(s: &str) -> Protocol {
    if s.starts_with("https://") {
        Protocol::Https
    } else {
        Protocol::Http
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum FailKind {
    Transient,
    Fatal,
    Verify,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checksum {
    pub algo: &'static str,
    pub value: String,
}

mod task;
mod timefmt;

pub use bt::{BtFile, BtTaskData};
#[allow(unused_imports)]
pub use config::{ProxyChoice, ProxyEndpoint};
#[allow(unused_imports)]
pub(crate) use task::conn_transferring;
#[allow(unused_imports)]
pub use task::{ConnPhase, Connection, PieceState, Task};
#[allow(unused_imports)]
pub use timefmt::fmt_created;
pub use timefmt::unix_now;

pub(crate) fn save_json_atomic<T: serde::Serialize>(value: &T, path: &str) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    if std::fs::read_to_string(path).is_ok_and(|old| old == json) {
        return Ok(());
    }
    let tmp = format!("{path}.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn sample_task() -> Task {
    Task {
        id: 1,
        name: "f.bin".to_string(),
        protocol: Protocol::Http,
        url: "http://x/f.bin".to_string(),
        name_locked: false,
        save_dir: "/dl".to_string(),
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
        block_size: 1_048_576,
        concurrency: 4,
        proxy: config::ProxyChoice::Direct,
        retries: 0,
        max_retries: 5,
        made_progress: false,
        retry_in: None,
        fail_kind: None,
        error: None,
        invalidation_streak: 0,
        checksum: None,
        verify_ok: None,
        etag: None,
        last_modified: None,
        has_slot: false,
        upload_speed: 0.0,
        uploaded: 0,
        seeders: 0,
        peers: 0,
        seed_left: 0.0,
        created: "2026-02-10 10:00".to_string(),
        added_at: 0,
        bt: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_labels_match_phase01() {
        assert_eq!(TaskState::Queued.label(), "等待中");
        assert_eq!(TaskState::Downloading.label(), "下载中");
        assert_eq!(TaskState::Paused.label(), "已暂停");
        assert_eq!(TaskState::Verifying.label(), "校验中");
        assert_eq!(TaskState::PostProcessing.label(), "后期处理中");
        assert_eq!(TaskState::Completed.label(), "已完成");
        assert_eq!(TaskState::Failed.label(), "已失败");
        assert_eq!(TaskState::Seeding.label(), "做种中");
    }

    #[test]
    fn done_tab_contains_completed_and_seeding() {
        assert!(TaskState::Completed.is_done());
        assert!(TaskState::Seeding.is_done());
        assert!(!TaskState::Failed.is_done());
        assert!(!TaskState::Queued.is_done());
        assert!(!TaskState::Downloading.is_done());
        assert!(!TaskState::Paused.is_done());
        assert!(!TaskState::Verifying.is_done());
        assert!(!TaskState::PostProcessing.is_done());
    }

    #[test]
    fn thread_counts_active_conns_by_data_plane() {
        let mut t = sample_task();
        let conn = |id: usize, start: u64, end: u64, done: u64| Connection {
            id,
            start,
            end,
            done,
        };
        t.connections = vec![
            conn(1, 0, 1_000, 400),
            conn(2, 1_000, 1_000, 0),
            conn(3, 1_000, 2_000, 1_000),
            conn(4, 2_000, 3_000, 100),
        ];
        t.state = TaskState::Downloading;
        assert_eq!(t.thread_count(), 2, "下载中 = 持有未完成块的连接数");
        t.state = TaskState::Paused;
        assert_eq!(t.thread_count(), 3, "非下载态 = 有效连接数（cap>0）");
    }

    #[test]
    fn chunk_info_uses_block_size() {
        let mut t = sample_task();
        t.total = 2_500_000;
        t.block_size = 1_000_000;
        t.chunk_done = 2;
        let (x, y, n) = t.chunk_info();
        assert_eq!((x, y, n), (2, 3, 1_000_000));
    }

    #[test]
    fn progress_clamps() {
        let mut t = sample_task();
        t.total = 100;
        t.downloaded = 250;
        assert!((t.progress() - 1.0).abs() < 1e-9);
        t.downloaded = 50;
        assert!((t.progress() - 0.5).abs() < 1e-9);
        t.total = 0;
        assert!(t.progress() == 0.0);
    }

    #[test]
    fn eta_none_when_not_downloading() {
        let mut t = sample_task();
        t.state = TaskState::Paused;
        t.speed = 100.0;
        assert!(t.eta_secs().is_none());
        t.state = TaskState::Downloading;
        t.total = 1000;
        t.downloaded = 0;
        assert_eq!(t.eta_secs(), Some(10));
    }

    #[test]
    fn paths_follow_naming_rules() {
        let mut t = sample_task();
        t.save_dir = "/dl".to_string();
        t.name = "a.bin".to_string();
        assert_eq!(t.target_path(), "/dl/a.bin");
        assert_eq!(t.downloading_path(), "/dl/a.bin.downloading");
        assert_eq!(t.sidecar_path(), "/dl/a.bin.ezr");
    }

    #[test]
    fn fmt_created_known_instant() {
        assert_eq!(fmt_created(1_767_225_600), "2026-01-01 08:00");
    }

    #[test]
    fn new_queued_initial_state() {
        let t = Task::new_queued(
            7,
            "f.bin".to_string(),
            Protocol::Http,
            "http://x/f.bin".to_string(),
            "/dl".to_string(),
            1_000_000,
            8,
            3,
            None,
            ProxyChoice::Direct,
            1_767_225_600,
        );
        assert_eq!(t.id, 7);
        assert_eq!(t.state, TaskState::Queued);
        assert_eq!(
            (t.total, t.downloaded, t.chunk_done, t.retries),
            (0, 0, 0, 0)
        );
        assert!(t.resumable && !t.probed && !t.has_slot && !t.made_progress);
        assert!(t.connections.is_empty());
        assert_eq!(
            (t.concurrency, t.block_size, t.max_retries),
            (8, 1_000_000, 3)
        );
        assert!(t.checksum.is_none() && t.verify_ok.is_none() && t.error.is_none());
        assert_eq!(t.created, "2026-01-01 08:00");
        assert_eq!(t.added_at, 1_767_225_600);
    }
}
