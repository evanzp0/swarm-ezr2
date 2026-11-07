use super::App;
use crate::model::namegen::SIDECAR_EXT;
use crate::model::{protocol_of_url, unix_now, Checksum, Task};

impl App {
    pub fn add_cli_task(
        &mut self,
        url: String,
        dir: Option<String>,
        conns: Option<usize>,
        checksum: Option<Checksum>,
    ) {
        if !crate::model::is_http_url(&url) {
            self.set_toast(format!("⚠ 仅支持 http:// 或 https:// 链接（{url}）"));
            return;
        }
        // bug27：任务按 URL 唯一——同 URL 已存在（无论保存目录异同）拒绝重复添加。
        if self.tasks.iter().any(|t| t.url == url) {
            self.set_toast("⚠ 该链接的任务已存在");
            return;
        }
        let dir = dir
            .or_else(|| self.cfg.download_dir.clone())
            .unwrap_or_else(crate::model::config::default_download_dir);
        let dir = crate::model::config::normalize_save_dir(&dir);
        let base_name = crate::model::namegen::derive_name(None, None, &url);
        let occupied = self.tasks.iter().any(|t| {
            t.save_dir == dir
                && t.name == base_name
                && t.state != crate::model::TaskState::Completed
        });
        let resumed = !occupied
            && crate::model::sidecar::Sidecar::load(&format!("{dir}/{base_name}{SIDECAR_EXT}"))
                .is_some();
        let name = if resumed {
            base_name.clone()
        } else {
            let tasks = &self.tasks;
            crate::model::namegen::dedupe(&base_name, |n| {
                tasks.iter().any(|t| t.name == n && t.save_dir == dir)
                    || crate::model::namegen::exists_on_disk(&dir, n)
            })
        };
        let protocol = protocol_of_url(&url);
        let ts = unix_now();
        let id = self.alloc_task_id();
        let mut t = Task::new_queued(
            id,
            name.clone(),
            protocol,
            url,
            dir,
            self.cfg.block_size_http,
            conns.unwrap_or(self.cfg.http_concurrency).clamp(1, 64),
            self.cfg.max_retries,
            checksum,
            crate::model::config::ProxyChoice::Direct,
            ts,
        );
        if resumed {
            t.name_locked = true;
            t.resume_orphan = true;
        }
        t.max_download_speed = self.cfg.task_max_download_speed;
        self.tasks.push(t);
        if resumed {
            self.set_toast(format!(
                "✓ 已添加任务 #{id}: {name}（发现有效断点，将自动接续）"
            ));
        } else {
            self.set_toast(format!("✓ 已添加任务 #{id}: {name}"));
        }
        self.save_registry();
    }
}

#[cfg(test)]
mod cli_add_tests {
    use super::*;
    use crate::model::config::Config;

    #[tokio::test]
    async fn cli_add_applies_config_default_task_speed_limit() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-cli-tlimit");
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let cfg = Config {
            task_max_download_speed: 524_288,
            ..Config::default()
        };
        let mut app = App::new(cfg, reg);
        let save = dir.to_string_lossy().to_string();
        app.add_cli_task(
            "http://example.com/limit.bin".to_string(),
            Some(save),
            None,
            None,
        );
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(
            app.tasks[0].max_download_speed, 524_288,
            "CLI 新建任务应回退 config 任务级默认下载限速（FR-02-29 默认值口径）"
        );
        assert_eq!(
            app.tasks[0].max_upload_speed, 0,
            "HTTP 无上传语义，任务上传限速恒 0"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn cli_add_dedupes_duplicate_names() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-cli-add");
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut app = App::new(Config::default(), reg);
        let save = dir.to_string_lossy().to_string();
        app.add_cli_task(
            "http://example.com/f.bin".to_string(),
            Some(save.clone()),
            None,
            None,
        );
        app.add_cli_task(
            "http://example.com/other/f.bin".to_string(),
            Some(save),
            None,
            None,
        );
        assert_eq!(
            app.tasks.len(),
            2,
            "异 URL 同推导名各自建任务（bug27 口径）"
        );
        assert_eq!(app.tasks[0].name, "f.bin");
        assert_eq!(app.tasks[1].name, "f.bin.1");
        assert_ne!(app.tasks[0].id, app.tasks[1].id, "随机分配的 ID 互不相同");
        assert_ne!(app.tasks[0].id, 0);
        assert_ne!(app.tasks[1].id, 0);
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn cli_add_same_url_rejected_as_duplicate() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-cli-dup");
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut app = App::new(Config::default(), reg);
        let url = "http://example.com/f.bin".to_string();
        let save = dir.to_string_lossy().to_string();
        app.add_cli_task(url.clone(), Some(save.clone()), None, None);
        assert_eq!(app.tasks.len(), 1);
        let other = crate::model::config::normalize_save_dir("ezr-cli-dup-other");
        app.add_cli_task(url, Some(other), None, None);
        assert_eq!(
            app.toast.as_deref(),
            Some("⚠ 该链接的任务已存在"),
            "同 URL 二次添加拒绝（bug27：任务按 URL 唯一，与保存目录无关）"
        );
        assert_eq!(app.tasks.len(), 1, "判重命中不新增任务");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn cli_add_rejects_non_http_url_with_toast() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-cli-bad-url");
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut app = App::new(Config::default(), reg);
        app.add_cli_task("ftp://example.com/f.bin".to_string(), None, None, None);
        assert!(app.tasks.is_empty(), "非法 URL 不建任务");
        let toast = app.toast.as_deref().unwrap_or_default();
        assert!(
            toast.contains("仅支持 http:// 或 https://"),
            "toast 提示: {toast}"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn cli_add_resumes_by_path_sidecar_keeps_name() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-cli-resume");
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let save = dir.to_string_lossy().to_string();
        crate::app::testutil::preset_resume_sidecar(&dir);
        let mut app = App::new(Config::default(), reg);
        app.add_cli_task(
            "http://example.com/r.bin".to_string(),
            Some(save),
            None,
            None,
        );
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(app.tasks[0].name, "r.bin", "路径基接续沿用推导原名");
        assert!(
            app.tasks[0].name_locked,
            "接续任务名随接续定稿（首探前失效重下不得去重漂移，QA-RS-14）"
        );
        let toast = app.toast.as_deref().unwrap_or_default();
        assert!(toast.contains("自动接续"), "接续 toast: {toast}");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn cli_add_guard_blocks_resume_when_path_occupied() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-cli-guard");
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let save = dir.to_string_lossy().to_string();
        crate::app::testutil::preset_resume_sidecar(&dir);
        let mut app = App::new(Config::default(), reg);
        let mut occupied = crate::model::sample_task();
        occupied.id = 9;
        occupied.name = "r.bin".to_string();
        occupied.save_dir = save.clone();
        occupied.url = "http://other.example/r.bin".to_string();
        occupied.state = crate::model::TaskState::Downloading;
        app.tasks.push(occupied);
        app.add_cli_task(
            "http://example.com/r.bin".to_string(),
            Some(save),
            None,
            None,
        );
        assert_eq!(app.tasks.len(), 2);
        assert_eq!(
            app.tasks[1].name, "r.bin.1",
            "CLI 同口径（v1.45 4A）：占用 → 序号改道"
        );
        assert!(
            !app.tasks[1].resume_orphan,
            "守卫臂不接续存活任务的 sidecar"
        );
        let toast = app.toast.as_deref().unwrap_or_default();
        assert!(!toast.contains("断点"), "守卫臂静默: {toast}");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn cli_add_normalizes_relative_config_download_dir() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-cli-norm");
        let cfg = Config {
            download_dir: Some("ezr-cli-rel-dl".to_string()),
            ..Config::default()
        };
        let cwd = std::env::current_dir().unwrap();
        let mut app = App::new(
            cfg,
            dir.join("registry.json").to_string_lossy().into_owned(),
        );
        app.add_cli_task("http://example.com/n.bin".to_string(), None, None, None);
        assert_eq!(app.tasks.len(), 1, "任务创建");
        assert_eq!(
            app.tasks[0].save_dir,
            cwd.join("ezr-cli-rel-dl").to_string_lossy().to_string(),
            "配置 download_dir 相对路径按创建时 cwd 转绝对（D62）"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
