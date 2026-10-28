use serde::{Deserialize, Serialize};

use super::config::ProxyChoice;
use super::sidecar::ChecksumCompat;
use super::{Checksum, FailKind, Protocol, Task, TaskState};

pub const REGISTRY_VERSION: u32 = 1;

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
            "direct" | "global" => ProxyChoice::Direct,
            s => s
                .strip_prefix("named:")
                .map(|n| ProxyChoice::Named(n.to_string()))
                .unwrap_or(ProxyChoice::Direct),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Registry {
    pub version: u32,
    pub tasks: Vec<TaskSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskSnapshot {
    pub id: u32,
    pub name: String,
    pub protocol: Protocol,
    pub url: String,
    #[serde(default)]
    pub name_locked: bool,
    pub save_dir: String,
    pub total: u64,
    pub downloaded: u64,
    pub state: TaskState,
    pub resumable: bool,
    pub probed: bool,
    pub block_size: u64,
    pub concurrency: usize,
    pub retries: u32,
    pub max_retries: u32,
    pub invalidation_streak: u32,
    pub checksum: Option<ChecksumCompat>,
    #[serde(default)]
    pub proxy: ProxyChoiceCompat,
    pub verify_ok: Option<bool>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub error: Option<String>,
    pub created: String,
    pub added_at: u64,
    #[serde(default)]
    pub uploaded: u64,
    #[serde(default)]
    pub seed_left: f64,
    #[serde(default)]
    pub bt: Option<super::bt::BtTaskData>,
}

impl From<&Task> for TaskSnapshot {
    fn from(t: &Task) -> Self {
        TaskSnapshot {
            id: t.id,
            name: t.name.clone(),
            protocol: t.protocol,
            url: t.url.clone(),
            name_locked: t.name_locked,
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
            uploaded: t.uploaded,
            seed_left: t.seed_left,
            bt: t.bt.clone(),
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
            name_locked: s.name_locked,
            save_dir: s.save_dir.clone(),
            total: s.total,
            downloaded: s.downloaded,
            speed: 0.0,
            state: s.state,
            resumable: s.resumable,
            probed: s.probed,
            connections: vec![],
            chunk_done: 0,
            done_blocks: Vec::new(),
            partial_blocks: Vec::new(),
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
            uploaded: s.uploaded,
            seeders: 0,
            peers: 0,
            seed_left: s.seed_left,
            created: s.created.clone(),
            added_at: s.added_at,
            bt: s.bt.clone(),
        }
    }
}

impl Registry {
    #[must_use]
    pub fn new() -> Registry {
        Registry {
            version: REGISTRY_VERSION,
            tasks: Vec::new(),
        }
    }

    #[must_use]
    pub fn from_tasks(tasks: &[Task]) -> Registry {
        Registry {
            version: REGISTRY_VERSION,
            tasks: tasks.iter().map(TaskSnapshot::from).collect(),
        }
    }

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
                    if t.verify_ok != Some(true) {
                        t.verify_ok = None;
                    }
                }
                t
            })
            .collect()
    }

    pub fn save(&self, path: &str) -> std::io::Result<()> {
        super::save_json_atomic(self, path)
    }

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

    #[test]
    fn legacy_registry_with_next_id_field_still_loads() {
        let mut t = sample_task();
        t.id = 5;
        let reg = Registry::from_tasks(&[t]);
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reg-legacy");
        let p = dir.join("registry.json");
        reg.save(p.to_str().unwrap()).unwrap();
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

    #[test]
    fn legacy_final_url_field_is_ignored_and_not_written() {
        let mut t = sample_task();
        t.id = 5;
        let reg = Registry::from_tasks(&[t]);
        let raw = serde_json::to_string(&reg).unwrap();
        let legacy = raw.replace(
            "\"url\":\"http://x/f.bin\"",
            "\"url\":\"http://x/f.bin\",\"final_url\":\"http://old.example/fin.bin\"",
        );
        assert!(legacy.contains("final_url"), "旧形态注入应生效");
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reg-fu-legacy");
        let p = dir.join("registry.json");
        std::fs::write(&p, legacy).unwrap();
        let loaded =
            Registry::load(p.to_str().unwrap()).expect("旧注册表（含 final_url）应兼容加载");
        let p2 = dir.join("rewritten.json");
        loaded.save(p2.to_str().unwrap()).unwrap();
        let text = std::fs::read_to_string(&p2).unwrap();
        assert!(
            !text.contains("final_url"),
            "新序列化不得含 final_url 键: {text}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn legacy_failed_paused_variant_is_rejected() {
        let mut t = sample_task();
        t.state = TaskState::Paused;
        t.error = Some("连接超时".to_string());
        let reg = Registry::from_tasks(&[t]);
        let raw = serde_json::to_string(&reg).unwrap();
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

    #[test]
    fn proxy_choice_compat_default_is_direct() {
        assert_eq!(ProxyChoiceCompat::default().0, "direct");
    }

    #[test]
    fn proxy_choice_compat_legacy_global_maps_to_direct() {
        assert_eq!(
            ProxyChoice::from(ProxyChoiceCompat("global".to_string())),
            ProxyChoice::Direct
        );
    }

    #[test]
    fn proxy_choice_compat_unknown_falls_back_to_direct() {
        assert_eq!(
            ProxyChoice::from(ProxyChoiceCompat("bogus".to_string())),
            ProxyChoice::Direct
        );
        assert_eq!(
            ProxyChoice::from(ProxyChoiceCompat("namedx".to_string())),
            ProxyChoice::Direct
        );
    }

    #[test]
    fn registry_default_is_empty() {
        assert!(Registry::default().tasks.is_empty());
    }

    #[test]
    fn load_rejects_version_mismatch() {
        let reg = Registry::from_tasks(&[sample_task()]);
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-reg-version");
        let p = dir.join("registry.json");
        reg.save(p.to_str().unwrap()).unwrap();
        let mut v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        v["version"] = serde_json::json!(REGISTRY_VERSION + 1);
        std::fs::write(&p, serde_json::to_string(&v).unwrap()).unwrap();
        assert!(
            Registry::load(p.to_str().unwrap()).is_none(),
            "版本不符应返回 None"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn restore_resets_unverified_checksum_result() {
        let mut t = sample_task();
        t.state = TaskState::Downloading;
        t.verify_ok = Some(false);
        let restored = Registry::from_tasks(&[t]).restore_tasks();
        assert_eq!(restored[0].state, TaskState::Queued);
        assert_eq!(restored[0].verify_ok, None);
    }
}
