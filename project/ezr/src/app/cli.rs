//! cli — CLI 启动参数添加任务（FR-01-01；入口 shell 与应用层的边界归位）
//!
//! `add_cli_task` 原位于 main.rs 的 `impl App` 块；架构评审（cleaner 批次后）依
//! notes/rust.md「入口（main.rs）不扩展应用主类型的 impl」条款移入应用层——
//! main.rs 只保留 CLI 解析（parse_cli）与调用，任务创建逻辑与对话框添加流
//! （dialogs.rs）同层同权。

use super::App;
use crate::model::{unix_now, Checksum, Protocol, Task};

impl App {
    /// CLI 启动参数直接添加任务（FR-01-01；与对话框共用校验与命名规则）
    pub fn add_cli_task(
        &mut self,
        url: String,
        dir: Option<String>,
        conns: Option<usize>,
        checksum: Option<Checksum>,
    ) {
        // URL 校验（FR-01-05）
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            self.set_toast(format!("⚠ 仅支持 http:// 或 https:// 链接（{url}）"));
            return;
        }
        let dir = dir
            .or_else(|| self.cfg.download_dir.clone())
            .unwrap_or_else(crate::model::config::default_download_dir)
            .trim_end_matches('/')
            .to_string();
        let base_name = crate::model::namegen::derive_name(None, None, &url);
        // 断点自动接续（FR-01-26）：同 URL 同路径的既有 sidecar → 沿用原名；
        // 否则重名检测（任务表 + 盘上）自动追加序号（与对话框添加同用
        // namegen::dedupe，口径一致）
        let resumed = crate::model::sidecar::Sidecar::load(&format!("{dir}/{base_name}.ezr"))
            .is_some_and(|s| s.url == url);
        let name = if resumed {
            base_name.clone()
        } else {
            let tasks = &self.tasks;
            crate::model::namegen::dedupe(&base_name, |n| {
                tasks.iter().any(|t| t.name == n && t.save_dir == dir)
                    || crate::model::namegen::exists_on_disk(&dir, n)
            })
        };
        let protocol = if url.starts_with("https://") {
            Protocol::Https
        } else {
            Protocol::Http
        };
        let ts = unix_now();
        let id = self.next_id;
        let t = Task::new_queued(
            id,
            name.clone(),
            protocol,
            url,
            dir,
            self.cfg.block_size_http,
            conns.unwrap_or(self.cfg.default_concurrency).clamp(1, 64),
            self.cfg.max_retries,
            checksum,
            ts,
        );
        self.next_id += 1;
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
        let dir = std::env::temp_dir().join(format!("ezr-cli-add-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut app = App::new(Config::default(), reg);
        let url = "http://example.com/f.bin".to_string();
        let save = dir.to_string_lossy().to_string();
        app.add_cli_task(url.clone(), Some(save.clone()), None, None);
        app.add_cli_task(url, Some(save), None, None);
        assert_eq!(app.tasks.len(), 2);
        assert_eq!(app.tasks[0].name, "f.bin");
        // 同目录同名：第二个任务经 namegen::dedupe（任务表+盘上口径）追加序号
        assert_eq!(app.tasks[1].name, "f.bin.1");
        assert_eq!(app.next_id, 3);
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
