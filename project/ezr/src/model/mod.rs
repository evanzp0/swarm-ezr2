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
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_wrap)]
// 字节/速度/时间算术在 u64-f64 间转换是下载器领域固有；边界由调用方保证
#![allow(clippy::missing_const_for_fn)] // nursery 误报为主（含 trait impl 场景）
#![allow(clippy::doc_markdown, clippy::doc_lazy_continuation)] // 中文文档中英文术语不强制反引号
#![allow(clippy::float_cmp)] // 速度/时间为 0 的语义判断使用精确比较
#![allow(clippy::map_unwrap_or, clippy::option_if_let_else, clippy::unnested_or_patterns)]
#![allow(clippy::cognitive_complexity, clippy::too_many_lines)] // 分块计算/状态机逻辑固有复杂度


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

use std::time::{SystemTime, UNIX_EPOCH};

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
#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
    /// 连接序号（1 基）
    pub id: usize,
    /// 分块起始字节（含）
    pub start: u64,
    /// 分块结束字节（不含）
    pub end: u64,
    /// 已下载字节
    pub done: u64,
    /// 当前速度 B/s（UI 层由滑窗计算回填）
    pub speed: f64,
}

impl Connection {
    /// 该连接当前持有块的总字节
    #[must_use]
    pub fn cap(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// 块内完成比例
    #[must_use]
    pub fn frac(&self) -> f64 {
        let c = self.cap();
        if c == 0 {
            1.0
        } else {
            f64::from(self.done as u32) / f64::from(c as u32)
        }
    }
}

/// 下载任务（UI 权威状态；真实进度由引擎事件驱动更新）
#[derive(Clone, Debug)]
pub struct Task {
    /// 任务 ID（会话内自增，注册表持久化）
    pub id: u32,
    /// 目标文件名（最终名；下载中盘上文件追加 `.downloading`，FR-01-24）
    pub name: String,
    /// 协议
    pub protocol: Protocol,
    /// 原始 URL
    pub url: String,
    /// 重定向后的最终 URL（探测后填；续传一致性比较用，FR-01-14/22）
    pub final_url: Option<String>,
    /// 保存目录（不含文件名）
    pub save_dir: String,
    /// 文件总大小（字节；探测前为 0，等待行显示「未知」）
    pub total: u64,
    /// 已下载字节
    pub downloaded: u64,
    /// 当前速度 B/s（1s 滑窗，FR-01-17）
    pub speed: f64,
    /// 状态
    pub state: TaskState,
    /// 服务器是否支持断点续传（不支持时单并发整体下载，FR-01-12）
    pub resumable: bool,
    /// 是否已完成探测（等待中未探测时大小显示「未知」）
    pub probed: bool,
    /// 并发分块连接（下载中 = 活跃分块；其余状态 = 空或最后快照）
    pub connections: Vec<Connection>,
    /// AIR2：已完成分块数 x
    pub chunk_done: u32,
    /// 块大小（字节；HTTP 默认 1 MB 可配置，添加时按当时配置定死，D13）
    pub block_size: u64,
    /// 并发数（1–64）
    pub concurrency: usize,
    /// 已自动重试次数（连续失败时累加；有进展重置为 1；R 手动重试重置为 1）
    pub retries: u32,
    /// 自动重试上限（配置 `max_retries`，默认 5）
    pub max_retries: u32,
    /// 本次尝试（自上次失败/开始以来）是否下载到过数据（重试计数连续性判定）
    pub made_progress: bool,
    /// 距下次自动重试的倒计时（秒）；None = 不再自动重试
    pub retry_in: Option<f64>,
    /// 最近一次失败的类别（决定「不自动重试/已达上限」行文案与 R 重试语义）
    pub fail_kind: Option<FailKind>,
    /// 最近一次失败原因（详情「失败原因」行）
    pub error: Option<String>,
    /// 连续「续传一致性失效」次数（FR-01-22 防循环：连续 3 次转已失败停等）
    pub invalidation_streak: u32,
    /// 完整性校验期望（None = 无校验值，完成后不校验）
    pub checksum: Option<Checksum>,
    /// 校验结果；None = 未校验（无校验码或尚未完成）
    pub verify_ok: Option<bool>,
    /// 服务器 ETag（探测后填；一致性检查用）
    pub etag: Option<String>,
    /// 服务器 Last-Modified（探测后填；一致性检查用）
    pub last_modified: Option<String>,
    /// 是否持有下载槽位
    pub has_slot: bool,
    /// BT 上传速度 B/s（01 恒 0；FR-01-81 头部 ↑ 字段保留，BT 二期启用）
    #[allow(dead_code)]
    pub upload_speed: f64,
    /// BT 累计上传字节（02 预留）
    #[allow(dead_code)]
    pub uploaded: u64,
    /// BT 可连接做种数（02 预留，01 详情不出现）
    #[allow(dead_code)]
    pub seeders: usize,
    /// BT 邻居节点数（02 预留）
    #[allow(dead_code)]
    pub peers: usize,
    /// BT 剩余做种时间秒（02 预留）
    #[allow(dead_code)]
    pub seed_left: f64,
    /// 累计下载用时（秒，仅统计下载中时段）
    pub elapsed: f64,
    /// 添加时间（显示与 sidecar/注册表记录）
    pub created: String,
    /// 添加时刻的 Unix 秒（注册表用）
    pub added_at: u64,
}

impl Task {
    /// 新建排队中的下载任务（添加对话框与 CLI 添加共用初值口径，FR-01-01/03/04）：
    /// 进度/连接/重试等运行时字段全部取初值，`created` 由 `added_at` 换算。
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
        added_at: u64,
    ) -> Task {
        Task {
            id,
            name,
            protocol,
            url,
            final_url: None,
            save_dir,
            total: 0,
            downloaded: 0,
            speed: 0.0,
            state: TaskState::Queued,
            resumable: true,
            probed: false,
            connections: vec![],
            chunk_done: 0,
            block_size,
            concurrency,
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
            elapsed: 0.0,
            created: fmt_created(added_at),
            added_at,
        }
    }

    /// 进度比例 0.0–1.0
    #[must_use]
    pub fn progress(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.downloaded as f64 / self.total as f64).clamp(0.0, 1.0)
        }
    }

    /// 剩余时间（秒；速度为 0 或非下载态返回 None）
    #[must_use]
    pub fn eta_secs(&self) -> Option<u64> {
        if self.speed <= 0.0 || self.state != TaskState::Downloading {
            return None;
        }
        let remain = self.total.saturating_sub(self.downloaded) as f64;
        Some((remain / self.speed) as u64)
    }

    /// 并发线程数（展示用）：下载中 = 活跃连接数（有速度且未下载完的分块），
    /// 其余状态 = 有效连接数（排除已待命的空连接）
    #[must_use]
    pub fn thread_count(&self) -> usize {
        if self.state == TaskState::Downloading {
            self.connections
                .iter()
                .filter(|c| c.speed > 0.0 && c.done < c.cap())
                .count()
        } else {
            self.connections.iter().filter(|c| c.cap() > 0).count()
        }
    }

    /// 分块信息 (x=已完成块数, y=总块数, n=块大小字节)
    #[must_use]
    pub fn chunk_info(&self) -> (u32, u64, u64) {
        let y = chunk::chunk_total(self.total, self.block_size);
        (self.chunk_done.min(y), u64::from(y), self.block_size)
    }

    /// 目标文件完整路径（保存目录 + 文件名）
    #[must_use]
    pub fn target_path(&self) -> String {
        namegen::join_path(&self.save_dir, &self.name)
    }

    /// 下载中的盘上文件路径（追加 `.downloading` 扩展名，FR-01-24）
    #[must_use]
    pub fn downloading_path(&self) -> String {
        format!("{}.downloading", self.target_path())
    }

    /// sidecar 元数据文件路径（`<目标文件>.ezr`，D1）
    #[must_use]
    pub fn sidecar_path(&self) -> String {
        format!("{}.ezr", self.target_path())
    }

    /// 暂停/失败收尾的断点视图快照（详情页分块表）：按 total/downloaded/
    /// concurrency/block_size 推导连接视图，`chunk_done` 取快照与事件口径较大值。
    /// `downloaded` 由调用方显式传入（暂停/失败事件对「已下载」口径不同）。
    pub fn apply_chunk_snapshot(&mut self, downloaded: u64, chunk_done: u32) {
        let (conns, snap) =
            chunk::lease_snapshot(self.total, downloaded, self.concurrency, self.block_size);
        self.connections = conns;
        self.chunk_done = snap.max(chunk_done);
    }
}

/// 当前时刻的 Unix 秒（注册表/文件名时间戳兜底用）
#[must_use]
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// JSON 原子写（先写 `<path>.tmp` 再 rename；任意时刻断电不损坏，FR-01-20）。
/// 注册表与 sidecar 持久化共用（DRY 单源）。
///
/// # Errors
/// 序列化或临时文件写入/rename 失败时返回 IO 错误。
pub(crate) fn save_json_atomic<T: serde::Serialize>(value: &T, path: &str) -> std::io::Result<()> {
    let tmp = format!("{path}.tmp");
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// 当前时刻的 `%Y-%m-%d %H:%M` 本地时间字符串（任务 `created` 列显示）。
/// 不引入 chrono 依赖：按 Unix 秒手工换算（闰年/月长表），误差仅与时区
/// 固定偏移有关；显示用字段，不参与任何一致性判定。
#[must_use]
pub fn fmt_created(unix: u64) -> String {
    // 东八区偏移；显示口径与 demo 一致（本地时间）
    const CST: i64 = 8 * 3600;
    let t = unix as i64 + CST;
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", secs / 3600, (secs % 3600) / 60)
}

/// 天数 → (年, 月, 日)（Howard Hinnant civil_from_days 算法，公历）
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
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

    #[test]
    fn connection_frac_zero_cap_is_full() {
        let c = Connection { id: 1, start: 0, end: 0, done: 0, speed: 0.0 };
        assert!((c.frac() - 1.0).abs() < 1e-9);
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
            1_767_225_600,
        );
        assert_eq!(t.id, 7);
        assert_eq!(t.state, TaskState::Queued);
        assert_eq!((t.total, t.downloaded, t.chunk_done, t.retries), (0, 0, 0, 0));
        assert!(t.resumable && !t.probed && !t.has_slot && !t.made_progress);
        assert!(t.connections.is_empty());
        assert_eq!((t.concurrency, t.block_size, t.max_retries), (8, 1_000_000, 3));
        assert!(t.checksum.is_none() && t.verify_ok.is_none() && t.error.is_none());
        assert_eq!(t.created, "2026-01-01 08:00");
        assert_eq!(t.added_at, 1_767_225_600);
    }
}
