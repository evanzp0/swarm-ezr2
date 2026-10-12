//! registry — 任务注册表持久化（FR-01-70：`~/.ezr/state/`，原子写，崩溃恢复）
//!
//! 全部任务（含已完成/已失败历史）跨会话保留；启动时恢复任务列表与历史，
//! 「下载中」任务重启后回到「等待中」按序排队（FR-01-23）。
//!
//! 兼容性（v1.23，操作者 20261012 指令）：`has_error` 快照字段退役——失败挂起
//! 在数据层与普通暂停完全一致（挂起时即重置 error=None、retries=1）；v1.21
//! 产物注册表的多余 `has_error` 字段反序列化时被忽略（零迁移，Paused 任务
//! 按普通暂停处理）。
//! 兼容性（v1.21，操作者 20261012 指令）：FailedPaused 独立状态退役并入
//! Paused——**旧版注册表不兼容**：含 `"FailedPaused"` 状态值的旧 JSON 反序列化
//! 失败 → 按全新注册表处理（操作者钦定不迁移）。
//! 兼容性（v1.20，操作者 20261011 指令）：任务 ID 改随机 u32 分配
//! （`model::idgen`），注册表不再持久化 `next_id`——旧版 JSON 的多余
//! `next_id` 字段反序列化时被忽略（零迁移）；新文件缺该字段，旧版程序加载
//! 会因缺字段失败 → 按全新注册表处理（回滚降级可接受）。格式版本保持 1
//! （字段集随版本演进不漂移，版本闸门语义不变）。

use serde::{Deserialize, Serialize};

use super::config::ProxyChoice;
use super::sidecar::ChecksumCompat;
use super::{Checksum, FailKind, Protocol, Task, TaskState};

/// 注册表文件格式版本
pub const REGISTRY_VERSION: u32 = 1;

/// 任务级代理选择的注册表持久化形态（v1.5/FR-01-86；v1.6 两态）。
/// 字符串编码：`direct` / `named:<name>`（写入）；读取兼容旧 `global` 值映射
/// `direct`（FR-01-90，零迁移）；反序列化遇未知形态一律回退 `direct`
/// （容错口径同注册表其余字段）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyChoiceCompat(pub String);

impl Default for ProxyChoiceCompat {
    fn default() -> Self {
        Self("direct".to_string())
    }
}

impl From<&ProxyChoice> for ProxyChoiceCompat {
    fn from(c: &ProxyChoice) -> Self {
        match c {
            ProxyChoice::Direct => Self("direct".to_string()),
            ProxyChoice::Named(n) => Self(format!("named:{n}")),
        }
    }
}

impl From<ProxyChoiceCompat> for ProxyChoice {
    fn from(c: ProxyChoiceCompat) -> Self {
        match c.0.as_str() {
            // v1.6/FR-01-90：旧 "global" 值（全局键已退役）映射直连（零迁移）
            "direct" | "global" => ProxyChoice::Direct,
            s => s
                .strip_prefix("named:")
                .map(|n| ProxyChoice::Named(n.to_string()))
                .unwrap_or(ProxyChoice::Direct),
        }
    }
}

/// 注册表（JSON 持久化；原子写同 sidecar）
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Registry {
    /// 格式版本
    pub version: u32,
    /// 任务快照（含已完成/已失败历史；任务 ID 随快照保留，随机 u32）
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
    /// 任务级代理选择（v1.5/FR-01-86；旧注册表缺该字段 → 反序列化默认 = Global，
    /// v1.6 两态（direct/named:<name>；旧 "global" 值加载映射 direct，FR-01-90））
    #[serde(default)]
    pub proxy: ProxyChoiceCompat,
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
            proxy: ProxyChoiceCompat::from(&t.proxy),
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
            proxy: s.proxy.clone().into(),
            verify_ok: s.verify_ok,
            etag: s.etag.clone(),
            last_modified: s.last_modified.clone(),
            has_slot: false,
            upload_speed: 0.0,
            uploaded: 0,
            seeders: 0,
            peers: 0,
            seed_left: 0.0,
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
            tasks: Vec::new(),
        }
    }

    /// 从运行时任务列表构建（v1.20：ID 改随机分配，不再持久化 `next_id`——
    /// 任务 ID 唯一性由分配期查重保证（`model::idgen`），恢复侧无需派生）
    #[must_use]
    pub fn from_tasks(tasks: &[Task]) -> Registry {
        Registry {
            version: REGISTRY_VERSION,
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
    use crate::model::sample_task;

    /// v1.20 兼容：旧版注册表 JSON 含 next_id 字段 → 反序列化忽略未知字段
    /// 照常加载（零迁移；任务 ID 随快照保留）
    #[test]
    fn legacy_registry_with_next_id_field_still_loads() {
        let mut t = sample_task();
        t.id = 5;
        let reg = Registry::from_tasks(&[t]);
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reg-legacy");
        let p = dir.join("registry.json");
        reg.save(p.to_str().unwrap()).unwrap();
        // 注入旧版 next_id 字段（模拟 v1.19 及更早产物）
        let legacy = std::fs::read_to_string(&p)
            .unwrap()
            .replacen('{', "{ \"next_id\": 23,", 1);
        std::fs::write(&p, legacy).unwrap();
        let loaded = Registry::load(p.to_str().unwrap()).expect("旧注册表应兼容加载");
        assert_eq!(loaded.tasks[0].id, 5, "任务 ID 随快照保留");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn roundtrip_preserves_fields() {
        let mut t = sample_task();
        t.id = 5;
        t.state = TaskState::Completed;
        t.checksum = Some(Checksum {
            algo: "MD5",
            value: "d".repeat(32),
        });
        let reg = Registry::from_tasks(&[t.clone()]);
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reg");
        let p = dir.join("registry.json");
        reg.save(p.to_str().unwrap()).unwrap();
        let loaded = Registry::load(p.to_str().unwrap()).unwrap();
        assert_eq!(loaded.tasks[0].id, 5, "任务 ID 随快照往返保留");
        assert_eq!(loaded.tasks.len(), 1);
        let back = loaded.restore_tasks();
        assert_eq!(back[0].name, t.name);
        assert_eq!(back[0].state, TaskState::Completed);
        assert_eq!(back[0].checksum.as_ref().map(|c| c.algo), Some("MD5"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// v1.23（操作者 20261012 直判）：`has_error` 快照字段退役——新序列化不再
    /// 写出该键；v1.21 产物注册表的多余 `has_error` 字段反序列化被忽略（零迁移，
    /// Paused 任务按普通暂停处理，error/retries 随快照保留、恢复时重置）
    #[test]
    fn legacy_has_error_field_is_ignored() {
        let mut t = sample_task();
        t.state = TaskState::Paused;
        t.retries = 3;
        t.error = Some("连接被重置".to_string());
        t.retry_in = None;
        let reg = Registry::from_tasks(&[t]);
        let raw = serde_json::to_string(&reg).unwrap();
        assert!(
            !raw.contains("has_error"),
            "新序列化不再写 has_error 键: {raw}"
        );
        // 模拟 v1.21 产物：同一快照注入多余 has_error 字段
        let v121 = raw.replace(
            "\"state\":\"Paused\"",
            "\"state\":\"Paused\",\"has_error\":true",
        );
        assert!(v121.contains("has_error"), "v1.21 形态注入应生效");
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reg-he-legacy");
        let p = dir.join("registry.json");
        std::fs::write(&p, v121).unwrap();
        let loaded = Registry::load(p.to_str().unwrap())
            .expect("v1.21 产物（含多余 has_error 字段）仍可加载");
        let back = loaded.restore_tasks();
        assert_eq!(
            back[0].state,
            TaskState::Paused,
            "挂起态跨会话保留（按普通暂停处理）"
        );
        assert_eq!(
            back[0].error.as_deref(),
            Some("连接被重置"),
            "error 随快照保留（恢复时重置；展示层仅 Failed 态消费）"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// v1.21（操作者钦定不迁移）：含旧版 FailedPaused 状态值的注册表反序列化
    /// 失败 → `Registry::load` 返回 None → 按全新注册表处理（FR-01-70 崩溃恢复
    /// 降级口径，与文件损坏同路径）
    #[test]
    fn legacy_failed_paused_variant_is_rejected() {
        let mut t = sample_task();
        t.state = TaskState::Paused;
        t.error = Some("连接超时".to_string());
        let reg = Registry::from_tasks(&[t]);
        let raw = serde_json::to_string(&reg).unwrap();
        // 模拟 v1.20 及更早产物：状态值回写为旧枚举名
        let legacy = raw.replace("\"state\":\"Paused\"", "\"state\":\"FailedPaused\"");
        assert!(legacy.contains("FailedPaused"), "旧值注入应生效");
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reg-fp-legacy");
        let p = dir.join("registry.json");
        std::fs::write(&p, legacy).unwrap();
        assert!(
            Registry::load(p.to_str().unwrap()).is_none(),
            "含 FailedPaused 的旧注册表不兼容（按全新注册表处理）"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// v1.6/FR-01-90：旧注册表 "global" 代理值加载映射直连（零迁移）。
    /// 构造方式：现行快照序列化文本中把 proxy 值替换回旧 "global"（模拟 v1.5
    /// 产物注册表，字段集随版本演进不漂移）
    #[test]
    fn legacy_global_proxy_value_maps_to_direct() {
        let mut t = sample_task();
        t.proxy = ProxyChoice::Direct;
        let reg = Registry::from_tasks(&[t]);
        let raw = serde_json::to_string(&reg).unwrap();
        assert!(
            raw.contains("\"proxy\":\"direct\""),
            "现行编码为 direct: {raw}"
        );
        let legacy = raw.replace("\"proxy\":\"direct\"", "\"proxy\":\"global\"");
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reg-global");
        let p = dir.join("registry.json");
        std::fs::write(&p, legacy).unwrap();
        let loaded = Registry::load(p.to_str().unwrap()).unwrap();
        let back = loaded.restore_tasks();
        assert_eq!(
            back[0].proxy,
            ProxyChoice::Direct,
            "旧 global 值 → 直连（FR-01-90）"
        );
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
        let reg = Registry::from_tasks(&[a, b, c]);
        let tasks = reg.restore_tasks();
        assert_eq!(tasks[0].state, TaskState::Queued);
        assert_eq!(tasks[1].state, TaskState::Queued);
        assert!(!tasks[0].has_slot);
        assert!(tasks[0].connections.is_empty());
    }

    #[test]
    fn missing_or_corrupt_registry_is_none() {
        assert!(Registry::load("/nonexistent/registry.json").is_none());
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reg2");
        let p = dir.join("r.json");
        std::fs::write(&p, "garbage{").unwrap();
        assert!(Registry::load(p.to_str().unwrap()).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn atomic_save_no_tmp_left() {
        let reg = Registry::new();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reg3");
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
        let reg = Registry::from_tasks(&[t]);
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reg4");
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
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reg5");
        let p = dir.join("registry.json");
        Registry::from_tasks(&[t.clone()])
            .save(p.to_str().unwrap())
            .unwrap();
        t.downloaded = 150;
        Registry::from_tasks(&[t])
            .save(p.to_str().unwrap())
            .unwrap();
        let loaded = Registry::load(p.to_str().unwrap()).expect("注册表应可加载");
        assert_eq!(loaded.tasks[0].downloaded, 150, "内容漂移后保存必须落盘");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
