//! registry — 任务注册表持久化（FR-01-70：`~/.ezr/state/`，原子写，崩溃恢复）
//!
//! 全部任务（含已完成/已失败历史）跨会话保留；启动时恢复任务列表与历史，
//! 「下载中」任务重启后回到「等待中」按序排队（FR-01-23）。

use serde::{Deserialize, Serialize};

use super::sidecar::ChecksumCompat;
use super::{Checksum, FailKind, Protocol, Task, TaskState};

/// 注册表文件格式版本
pub const REGISTRY_VERSION: u32 = 1;

/// 注册表（JSON 持久化；原子写同 sidecar）
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Registry {
    /// 格式版本
    pub version: u32,
    /// 下一个任务 ID
    pub next_id: u32,
    /// 任务快照（含已完成/已失败历史）
    pub tasks: Vec<TaskSnapshot>,
}

/// 任务快照（注册表持久化形态；运行时 `Task` 的可序列化投影）
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskSnapshot {
    /// 任务 ID
    pub id: u32,
    /// 文件名
    pub name: String,
    /// 协议
    pub protocol: Protocol,
    /// 原始 URL
    pub url: String,
    /// 最终 URL
    pub final_url: Option<String>,
    /// 保存目录
    pub save_dir: String,
    /// 总大小
    pub total: u64,
    /// 已下载
    pub downloaded: u64,
    /// 状态
    pub state: TaskState,
    /// 是否支持续传
    pub resumable: bool,
    /// 是否已探测
    pub probed: bool,
    /// 块大小
    pub block_size: u64,
    /// 并发数
    pub concurrency: usize,
    /// 重试计数
    pub retries: u32,
    /// 自动重试上限
    pub max_retries: u32,
    /// 一致性失效连击
    pub invalidation_streak: u32,
    /// 校验期望
    pub checksum: Option<ChecksumCompat>,
    /// 校验结果
    pub verify_ok: Option<bool>,
    /// ETag
    pub etag: Option<String>,
    /// Last-Modified
    pub last_modified: Option<String>,
    /// 失败原因
    pub error: Option<String>,
    /// 添加时间显示串
    pub created: String,
    /// 添加时间 Unix 秒
    pub added_at: u64,
}

impl From<&Task> for TaskSnapshot {
    fn from(t: &Task) -> Self {
        TaskSnapshot {
            id: t.id,
            name: t.name.clone(),
            protocol: t.protocol,
            url: t.url.clone(),
            final_url: t.final_url.clone(),
            save_dir: t.save_dir.clone(),
            total: t.total,
            downloaded: t.downloaded,
            state: t.state,
            resumable: t.resumable,
            probed: t.probed,
            block_size: t.block_size,
            concurrency: t.concurrency,
            retries: t.retries,
            max_retries: t.max_retries,
            invalidation_streak: t.invalidation_streak,
            checksum: t.checksum.as_ref().map(ChecksumCompat::from),
            verify_ok: t.verify_ok,
            etag: t.etag.clone(),
            last_modified: t.last_modified.clone(),
            error: t.error.clone(),
            created: t.created.clone(),
            added_at: t.added_at,
        }
    }
}

impl From<&TaskSnapshot> for Task {
    fn from(s: &TaskSnapshot) -> Self {
        Task {
            id: s.id,
            name: s.name.clone(),
            protocol: s.protocol,
            url: s.url.clone(),
            final_url: s.final_url.clone(),
            save_dir: s.save_dir.clone(),
            total: s.total,
            downloaded: s.downloaded,
            speed: 0.0,
            state: s.state,
            resumable: s.resumable,
            probed: s.probed,
            connections: vec![],
            chunk_done: 0,
            block_size: s.block_size,
            concurrency: s.concurrency,
            retries: s.retries,
            max_retries: s.max_retries,
            made_progress: false,
            retry_in: None,
            fail_kind: s.error.as_deref().map(|_| FailKind::Fatal),
            error: s.error.clone(),
            invalidation_streak: s.invalidation_streak,
            checksum: s
                .checksum
                .as_ref()
                .and_then(|c| TryInto::<Checksum>::try_into(c).ok()),
            verify_ok: s.verify_ok,
            etag: s.etag.clone(),
            last_modified: s.last_modified.clone(),
            has_slot: false,
            upload_speed: 0.0,
            uploaded: 0,
            seeders: 0,
            peers: 0,
            seed_left: 0.0,
            elapsed: 0.0,
            created: s.created.clone(),
            added_at: s.added_at,
        }
    }
}

impl Registry {
    /// 空注册表
    #[must_use]
    pub fn new() -> Registry {
        Registry {
            version: REGISTRY_VERSION,
            next_id: 1,
            tasks: Vec::new(),
        }
    }

    /// 从运行时任务列表构建（`next_id` 取最大 id + 1）
    #[must_use]
    pub fn from_tasks(tasks: &[Task], next_id: u32) -> Registry {
        Registry {
            version: REGISTRY_VERSION,
            next_id: next_id.max(tasks.iter().map(|t| t.id + 1).max().unwrap_or(1)),
            tasks: tasks.iter().map(TaskSnapshot::from).collect(),
        }
    }

    /// 崩溃恢复投影（FR-01-23）：快照 → 运行时任务；
    /// 「下载中 / 校验中」重启后回「等待中」按序排队（has_slot 清空、清空连接视图）；
    /// 已完成/已失败历史原样保留（失败任务不自动重试，需手动 R）。
    #[must_use]
    pub fn restore_tasks(&self) -> Vec<Task> {
        self.tasks
            .iter()
            .map(Task::from)
            .map(|mut t| {
                if t.state == TaskState::Downloading || t.state == TaskState::Verifying {
                    t.state = TaskState::Queued;
                    t.has_slot = false;
                    t.connections.clear();
                    t.speed = 0.0;
                    // 重启时校验结果视为未定（重新校验/续传后重判）
                    if t.verify_ok != Some(true) {
                        t.verify_ok = None;
                    }
                }
                t
            })
            .collect()
    }

    /// 原子写（temp + rename，FR-01-20；实现见 `save_json_atomic`）；
    /// 内容与盘上一致时跳过写盘（幂等，空闲期周期保存零写放大）
    ///
    /// # Errors
    /// 写临时文件或 rename 失败时返回 IO 错误。
    pub fn save(&self, path: &str) -> std::io::Result<()> {
        super::save_json_atomic(self, path)
    }

    /// 读取（文件缺失/损坏/版本不符 → None，按全新注册表处理）
    #[must_use]
    pub fn load(path: &str) -> Option<Registry> {
        let text = std::fs::read_to_string(path).ok()?;
        let r: Registry = serde_json::from_str(&text).ok()?;
        if r.version != REGISTRY_VERSION {
            return None;
        }
        Some(r)
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{sample_task, unix_now};

    #[test]
    fn roundtrip_and_next_id() {
        let mut t = sample_task();
        t.id = 5;
        t.state = TaskState::Completed;
        t.checksum = Some(Checksum {
            algo: "MD5",
            value: "d".repeat(32),
        });
        let reg = Registry::from_tasks(&[t.clone()], 6);
        let dir = std::env::temp_dir().join(format!("ezr-reg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("registry.json");
        reg.save(p.to_str().unwrap()).unwrap();
        let loaded = Registry::load(p.to_str().unwrap()).unwrap();
        assert_eq!(loaded.next_id, 6);
        assert_eq!(loaded.tasks.len(), 1);
        let back = loaded.restore_tasks();
        assert_eq!(back[0].name, t.name);
        assert_eq!(back[0].state, TaskState::Completed);
        assert_eq!(back[0].checksum.as_ref().map(|c| c.algo), Some("MD5"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn crash_recovery_requeues_inflight() {
        let mut a = sample_task();
        a.id = 1;
        a.state = TaskState::Downloading;
        let mut b = sample_task();
        b.id = 2;
        b.state = TaskState::Verifying;
        let c = sample_task();
        let reg = Registry::from_tasks(&[a, b, c], 4);
        let tasks = reg.restore_tasks();
        assert_eq!(tasks[0].state, TaskState::Queued);
        assert_eq!(tasks[1].state, TaskState::Queued);
        assert!(!tasks[0].has_slot);
        assert!(tasks[0].connections.is_empty());
    }

    #[test]
    fn missing_or_corrupt_registry_is_none() {
        assert!(Registry::load("/nonexistent/registry.json").is_none());
        let dir = std::env::temp_dir().join(format!("ezr-reg2-{}", unix_now()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("r.json");
        std::fs::write(&p, "garbage{").unwrap();
        assert!(Registry::load(p.to_str().unwrap()).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn atomic_save_no_tmp_left() {
        let reg = Registry::new();
        let dir = std::env::temp_dir().join(format!("ezr-reg3-{}", unix_now()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("registry.json");
        reg.save(p.to_str().unwrap()).unwrap();
        assert!(!dir.join("registry.json.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// 内容未变化时保存不重写文件（空闲期零写放大）：周期兜底保存反复触发时，
    /// 字节一致的序列化结果必须短路跳过 temp+rename（mtime 不变为证）
    #[test]
    fn save_skips_rewrite_when_content_unchanged() {
        let mut t = sample_task();
        t.id = 1;
        t.state = TaskState::Completed;
        t.total = 100;
        t.downloaded = 100;
        let reg = Registry::from_tasks(&[t], 2);
        let dir = std::env::temp_dir().join(format!("ezr-reg4-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("registry.json");
        reg.save(p.to_str().unwrap()).unwrap();
        let m1 = std::fs::metadata(&p).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        reg.save(p.to_str().unwrap()).unwrap();
        let m2 = std::fs::metadata(&p).unwrap().modified().unwrap();
        assert_eq!(
            m1, m2,
            "内容未变化时保存不得重写文件（空闲期周期保存零写放大）"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// 内容变化时保存仍须写盘（防过度跳过）：跳过短路只对字节一致内容生效
    #[test]
    fn save_writes_when_content_changes() {
        let mut t = sample_task();
        t.id = 1;
        t.total = 200;
        t.downloaded = 100;
        let dir = std::env::temp_dir().join(format!("ezr-reg5-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("registry.json");
        Registry::from_tasks(&[t.clone()], 2)
            .save(p.to_str().unwrap())
            .unwrap();
        t.downloaded = 150;
        Registry::from_tasks(&[t], 2)
            .save(p.to_str().unwrap())
            .unwrap();
        let loaded = Registry::load(p.to_str().unwrap()).expect("注册表应可加载");
        assert_eq!(loaded.tasks[0].downloaded, 150, "内容漂移后保存必须落盘");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
