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
        let dir = dir
            .or_else(|| self.cfg.download_dir.clone())
            .unwrap_or_else(crate::model::config::default_download_dir);
        let dir = crate::model::config::normalize_save_dir(&dir);
        let base_name = crate::model::namegen::derive_name(None, None, &url);
        let resumed =
            crate::model::sidecar::Sidecar::load(&format!("{dir}/{base_name}{SIDECAR_EXT}"))
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
        }
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
    async fn cli_add_dedupes_duplicate_names() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-cli-add");
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut app = App::new(Config::default(), reg);
        let url = "http://example.com/f.bin".to_string();
        let save = dir.to_string_lossy().to_string();
        app.add_cli_task(url.clone(), Some(save.clone()), None, None);
        app.add_cli_task(url, Some(save), None, None);
        assert_eq!(app.tasks.len(), 2);
        assert_eq!(app.tasks[0].name, "f.bin");
        assert_eq!(app.tasks[1].name, "f.bin.1");
        assert_ne!(app.tasks[0].id, app.tasks[1].id, "随机分配的 ID 互不相同");
        assert_ne!(app.tasks[0].id, 0);
        assert_ne!(app.tasks[1].id, 0);
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
