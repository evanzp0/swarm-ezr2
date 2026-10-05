//! model — EZR Downloader 领域模型层（纯逻辑，全部可单元测试）
//!
//! 包含：任务/状态机基础模型（`Task`）、AIR2 分块计算（`chunk`）、重试策略
//! （`retry`）、速度滑窗（`speed`）、校验算法表（`checksum`）、sidecar 元数据
//! （`sidecar`）、续传一致性检查（`consistency`）、配置（`config`）、文件名
//! 推导（`namegen`）、槽位调度（`slots`）与任务注册表（`registry`）。
//!
//! 状态机口径（phase-01 FR-01-30）：01 实际出现 6 态——等待中/下载中/已暂停/
//! 校验中/已失败/已完成；「后期处理中」与「做种中」为后续期预留（枚举与配色
//! 保留、01 内无触发路径）。不存在「连接中」状态。

pub mod checksum;
pub mod chunk;
pub mod config;
pub mod consistency;
pub mod namegen;
pub mod registry;
pub mod retry;
pub mod sidecar;
pub mod slots;
pub mod speed;

/// 传输协议（01 仅 HTTP/HTTPS；`Bt` 为 phase-02 预留枚举，01 无触发路径）
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Protocol {
    /// HTTP
    Http,
    /// HTTPS
    Https,
    /// BT（phase-02 预留，01 不出现）
    #[allow(dead_code)]
    Bt,
}

impl Protocol {
    /// 协议徽标显示名（UI 黄色徽标用）
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Protocol::Http => "HTTP",
            Protocol::Https => "HTTPS",
            Protocol::Bt => "BT",
        }
    }

    /// 是否 BT（01 恒为 false）
    #[must_use]
    pub fn is_bt(&self) -> bool {
        matches!(self, Protocol::Bt)
    }
}

/// 任务状态（phase-01 FR-01-30 六态 + 两预留态）
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum TaskState {
    /// 排队等待（含失败重试后的重新排队）
    Queued,
    /// 下载中
    Downloading,
    /// 已暂停
    Paused,
    /// 完整性校验中（占下载槽位，FR-01-31/D12）
    Verifying,
    /// 后期处理（02+ 预留，01 无触发路径）
    #[allow(dead_code)]
    PostProcessing,
    /// 已完成
    Completed,
    /// 已失败
    Failed,
    /// 已暂停（失败）——失败任务按空格挂起自动重试（v1.6/FR-01-92/D25）；
    /// 错误信息与失败类型保留可见，不占下载槽位；空格/R 恢复 = 重新排队
    FailedPaused,
    /// 做种（BT，02+ 预留，01 无触发路径）
    #[allow(dead_code)]
    Seeding,
}

impl TaskState {
    /// 状态显示名（列表徽标与详情字段）
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
            TaskState::FailedPaused => "已暂停（失败）",
            TaskState::Seeding => "做种中",
        }
    }

    /// 是否属于「已完成」页签
    #[must_use]
    pub fn is_done(&self) -> bool {
        matches!(self, TaskState::Completed | TaskState::Seeding)
    }
}

/// 失败类别（FR-01-40/43/44/51/22）：决定自动重试行为与列表行文案
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum FailKind {
    /// 临时性错误（网络错误 / HTTP 408·429·5xx / 文件大小不符）：
    /// 按指数退避自动重试，`Retry-After` 存在时优先采用（R2：无 60s 上限）
    Transient,
    /// 语义性错误（HTTP 4xx 除 408/429 / 磁盘空间不足 / 服务器内容持续变化）：
    /// 不自动重试，直接停等（「已达上限」式），仅可 R 手动重试
    Fatal,
    /// 完整性校验失败：不自动重试；按 R 重新校验（断点保留，D10）
    Verify,
}

/// 任务的完整性校验期望（添加对话框显式提供 / CLI `-x` / 伴随文件，FR-01-50）
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checksum {
    /// 算法显示名（`checksum::CHECKSUM_ALGOS` 中的名称）
    pub algo: &'static str,
    /// 十六进制校验码（统一小写）
    pub value: String,
}

/// 单个并发连接（分块）状态：一段连续的 byte range 由一个 worker 负责
mod task;
mod timefmt;

#[allow(unused_imports)] // ProxyConfig 为 v1.5 对外 API 面（配置项类型）
pub use config::{ProxyChoice, ProxyConfig, ProxyEndpoint};
pub use task::{Connection, Task};
#[allow(unused_imports)] // 对外保留（01 期已暴露）
pub use timefmt::fmt_created;
pub use timefmt::unix_now;

/// JSON 原子写（先写 `<path>.tmp` 再 rename；任意时刻断电不损坏，FR-01-20）。
/// 注册表与 sidecar 持久化共用（DRY 单源）。
/// 内容幂等短路：序列化结果与盘上现有文件字节一致时跳过写盘——周期兜底保存
/// （如注册表 5s 兜底）在无变更的空闲期不再产生写放大（无谓 temp+rename 与
/// mtime 抖动）；内容有差异（含文件缺失/损坏）时照常原子落盘。
///
/// # Errors
/// 序列化或临时文件写入/rename 失败时返回 IO 错误。
pub(crate) fn save_json_atomic<T: serde::Serialize>(value: &T, path: &str) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    // 盘上现有内容与本次序列化结果字节一致 → 跳过写盘（读失败视为不一致照常写）
    if std::fs::read_to_string(path).is_ok_and(|old| old == json) {
        return Ok(());
    }
    let tmp = format!("{path}.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// 测试夹具：默认任务（各模块单测共用）
#[cfg(test)]
pub(crate) fn sample_task() -> Task {
    Task {
        id: 1,
        name: "f.bin".to_string(),
        protocol: Protocol::Http,
        url: "http://x/f.bin".to_string(),
        final_url: None,
        save_dir: "/dl".to_string(),
        total: 0,
        downloaded: 0,
        speed: 0.0,
        state: TaskState::Queued,
        resumable: true,
        probed: false,
        connections: vec![],
        chunk_done: 0,
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
        elapsed: 0.0,
        created: "2026-02-10 10:00".to_string(),
        added_at: 0,
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

    /// FR-01-81 修订二：并发线程数（活跃连接数）数据面口径——
    /// 下载中 = 持有未完成块的连接数（cap>0 且 done<cap）；其余态 = 有效连接数（cap>0）。
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
            conn(1, 0, 1_000, 400),       // 传输中：计入
            conn(2, 1_000, 1_000, 0),     // 待命空连接：不计入
            conn(3, 1_000, 2_000, 1_000), // 已完成当前块：不计入
            conn(4, 2_000, 3_000, 100),   // 传输中：计入
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
        // 2026-01-01 00:00:00 UTC = 1767225600；东八区 → 2026-01-01 08:00
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
