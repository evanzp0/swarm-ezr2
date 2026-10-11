//! task — 任务实体与单连接状态（模型层核心类型）

use super::config::ProxyChoice;
use super::namegen::{sidecar_path_of, DOWNLOADING_EXT};
use super::timefmt::fmt_created;
use super::{chunk, namegen, Checksum, FailKind, Protocol, TaskState};

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
}

impl Connection {
    /// 该连接当前持有块的总字节
    #[must_use]
    pub fn cap(&self) -> u64 {
        self.end.saturating_sub(self.start)
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
    /// 任务级代理选择（v1.5/FR-01-86，D18 三态；旧注册表缺省 = Global）
    pub proxy: ProxyChoice,
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
        proxy: ProxyChoice,
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

    /// 并发线程数（展示用）：下载中 = 活跃连接数（数据面口径，FR-01-81 修订二：
    /// 持有未完成块的连接），其余状态 = 有效连接数（排除已待命的空连接）
    /// FR-01-100（详情类型行去并发数）后产品面暂无调用点；数据面语义测试保留，
    /// phase-02 BT（做种/上传速度展示）预期复用
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

    /// 下载中的盘上文件路径（追加 `.downloading` 扩展名，FR-01-24；扩展名
    /// 单源于 [`namegen::DOWNLOADING_EXT`]，architect v116）
    #[must_use]
    pub fn downloading_path(&self) -> String {
        format!("{}{DOWNLOADING_EXT}", self.target_path())
    }

    /// sidecar 元数据文件路径（`<目标文件>.ezr`，D1；委托
    /// [`namegen::sidecar_path_of`] 单源——与 (目录, 文件名) 散点同口径）
    #[must_use]
    pub fn sidecar_path(&self) -> String {
        sidecar_path_of(&self.save_dir, &self.name)
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
