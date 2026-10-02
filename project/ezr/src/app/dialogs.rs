//! dialogs — 对话框确认流（添加/删除按钮激活、任务创建、文件删除）

use super::{App, Dialog, DialogKind, CHECKSUM_ALGOS};
use crate::engine::Cmd;
use crate::model::sidecar::Sidecar;
use crate::model::{checksum, namegen, unix_now, Checksum, Protocol, Task, TaskState};

impl App {
    pub(super) fn dlg_activate_add(&mut self, btn: usize) {
        match btn {
            5 => self.dlg_confirm_add(),
            6 => self.dialog = None,
            _ => {}
        }
    }

    /// 确认添加（FR-01-01/03/04/05/26：URL 校验、目录默认、并发钳制、校验码校验、断点接续）
    pub(super) fn dlg_confirm_add(&mut self) {
        let Some(d) = self.dialog.as_ref() else {
            return;
        };
        let url = d.url.trim().to_string();
        if url.is_empty() {
            self.set_toast("⚠ 请输入下载 URL");
            return;
        }
        // FR-01-05：仅接受 http:// / https://
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            self.set_toast("⚠ 仅支持 http:// 或 https:// 链接");
            return;
        }
        let dir_raw = d.dir.trim().to_string();
        let conns_raw = d.conns.trim().to_string();
        let ck_raw = d.ck_value.trim().to_string();
        let ck_type = d.ck_type;

        // 校验码（可留空 = 不校验；输入统一小写，位数与算法匹配）
        let checksum = if ck_raw.is_empty() {
            None
        } else {
            match checksum::validate_value(ck_type, &ck_raw) {
                Ok(v) => Some(Checksum {
                    algo: CHECKSUM_ALGOS[ck_type].0,
                    value: v,
                }),
                Err(e) => {
                    let algo = CHECKSUM_ALGOS[ck_type].0;
                    self.set_toast(format!("⚠ {algo} {e}"));
                    if let Some(d) = self.dialog.as_mut() {
                        d.focus = 4;
                    }
                    return;
                }
            }
        };

        // 保存目录（FR-01-03）：留空 = 配置 download_dir；缺省 = ~/Downloads；
        // 统一去尾斜杠（trim 幂等，归一一次全函数共用）
        let dir = if dir_raw.is_empty() {
            self.cfg
                .download_dir
                .clone()
                .unwrap_or_else(crate::model::config::default_download_dir)
        } else {
            dir_raw
        };
        let dir = dir.trim_end_matches('/').to_string();

        let protocol = if url.starts_with("https://") {
            Protocol::Https
        } else {
            Protocol::Http
        };
        // 重复任务拒绝（Gherkin 01-add-task-16）：同 URL 同保存目录已在列表 →
        // toast「任务已存在」，不创建新任务（对话框保持打开，与其他校验失败一致）
        if self.tasks.iter().any(|t| t.url == url && t.save_dir == dir) {
            self.set_toast("⚠ 任务已存在");
            return;
        }
        let base_name = namegen::derive_name(None, None, &url);
        let id = self.next_id;
        // FR-01-26 断点自动接续：既有 sidecar 同 URL 同路径 → 接续原名
        let resumed = Sidecar::load(&format!("{dir}/{base_name}.ezr"))
            .is_some_and(|sc| sc.url == url)
            || self.tasks.iter().any(|t| {
                t.url == url
                    && t.save_dir == dir
                    && t.name == base_name
                    && t.state != TaskState::Completed
            });
        let name = if resumed {
            base_name.clone()
        } else {
            // 重名检测：任务表 + 盘上（文件/.downloading/sidecar）→ 自动追加序号
            let tasks = &self.tasks;
            namegen::dedupe(&base_name, |n| {
                tasks.iter().any(|t| t.name == n && t.save_dir == dir)
                    || namegen::exists_on_disk(&dir, n)
            })
        };
        // 并发数（FR-01-04）：非 1–64 范围（留空/0/65/非数字）一律回退配置默认
        // （Gherkin 01-add-task-06：留空/0/65/abc → 均回退默认 4，非钳制）
        let conns = match conns_raw.parse::<usize>() {
            Ok(v) if (1..=64).contains(&v) => v,
            _ => self.cfg.default_concurrency,
        };
        self.next_id += 1;
        // 校验值来源②（FR-01-50/D14）：未显式提供时查保存目录伴随文件
        let checksum = Self::resolve_checksum(&dir, &name, checksum);
        let ts = unix_now();
        let t = Task::new_queued(
            id,
            name.clone(),
            protocol,
            url.clone(),
            dir,
            self.cfg.block_size_http,
            conns,
            self.cfg.max_retries,
            checksum,
            ts,
        );
        self.tasks.push(t);
        self.dialog = None;
        self.filter = 0;
        let flen = self.filtered().len();
        if flen > 0 {
            self.selected = flen - 1;
        }
        if resumed {
            self.set_toast(format!(
                "✓ 已添加任务 #{id}: {name}（发现有效断点，将自动接续）"
            ));
        } else {
            self.set_toast(format!("✓ 已添加任务 #{id}: {name}"));
        }
        self.save_registry();
    }

    pub(super) fn dlg_activate_delete(&mut self, btn: usize) {
        let Some(idx) = self.sel_idx() else {
            self.dialog = None;
            return;
        };
        let (id, name) = {
            let t = &self.tasks[idx];
            (t.id, t.name.clone())
        };
        match btn {
            0 => {
                // 仅删除任务：文件与 sidecar 保留（FR-01-25）；引擎任务取消（停写 sidecar）
                self.remove_task(idx, id, false);
                self.set_toast(format!("🗑 已删除任务（保留文件）: {name}"));
                self.dialog = None; // 操作完成即关闭对话框
            }
            1 => {
                // 删除任务和文件：记录即时删除；文件在引擎 Cancelled 确认后删除（防重建竞态）
                self.remove_task(idx, id, true);
                self.set_toast(format!("🗑 已删除任务，正在停止引擎并清理本地文件: {name}"));
                self.dialog = None; // 操作完成即关闭对话框
            }
            _ => {
                self.dialog = None;
            }
        }
        let flen = self.filtered().len();
        if self.selected >= flen {
            self.selected = flen.saturating_sub(1);
        }
    }

    /// 从任务表移除并取消引擎任务（删除对话框两路共用）。
    /// `delete_files` = true 时登记延迟删除：引擎回报 [`Evt::Cancelled`]（supervisor
    /// 已完全退出、不再写盘）后再删目标文件/.downloading/.ezr，避免与引擎在途
    /// 写入竞态导致 sidecar/文件被重新创建。
    pub(super) fn remove_task(&mut self, idx: usize, id: u32, delete_files: bool) {
        let (save_dir, name) = {
            let t = &self.tasks[idx];
            (t.save_dir.clone(), t.name.clone())
        };
        self.tasks.remove(idx);
        self.windows.remove(&id);
        self.conn_windows.retain(|(tid, _), _| *tid != id);
        if delete_files {
            self.pending_deletes.insert(id, (save_dir, name));
        }
        let engine = self.engine.clone();
        tokio::spawn(async move {
            engine.send(Cmd::Cancel { id }).await;
        });
        self.save_registry();
    }

    /// 删除任务的三类本地文件：目标文件、.downloading、.ezr sidecar（FR-01-25「删除任务和文件」）
    pub(super) fn delete_task_files(save_dir: &str, name: &str) {
        let base = save_dir.trim_end_matches('/');
        for path in [
            format!("{base}/{name}"),
            format!("{base}/{name}.downloading"),
            format!("{base}/{name}.ezr"),
        ] {
            let _ = std::fs::remove_file(&path);
        }
    }

    /// 打开添加对话框（空预填；URL/目录留空由用户输入）
    pub fn open_add_dialog(&mut self) {
        self.dialog = Some(Dialog {
            kind: DialogKind::Add,
            url: String::new(),
            dir: String::new(),
            conns: String::new(),
            conns_edited: false,
            ck_type: 3,
            ck_value: String::new(),
            ck_open: false,
            ck_sel: 3,
            focus: 0,
            task_name: String::new(),
        });
    }

    /// 打开删除对话框（三选：仅任务/任务和文件/取消）
    pub fn open_delete_dialog(&mut self) {
        let Some(t) = self.sel_task() else {
            self.set_toast("没有选中的任务");
            return;
        };
        self.dialog = Some(Dialog {
            kind: DialogKind::Delete,
            url: String::new(),
            dir: String::new(),
            conns: String::new(),
            conns_edited: false,
            ck_type: 0,
            ck_value: String::new(),
            ck_open: false,
            ck_sel: 0,
            focus: 0,
            task_name: t.name.clone(),
        });
    }

    // -----------------------------------------------------------------------
    // 鼠标交互（沿用 demo：点击选中 / 滚轮 / 对话框按钮与字段）
    // -----------------------------------------------------------------------
}

#[cfg(test)]
mod add_dialog_flow_tests {
    use crossterm::event::{KeyCode, KeyModifiers};

    use super::*;
    use crate::model::config::Config;

    /// 添加对话框全流程：输入 URL/目录 → Enter 确认 → 任务入列；
    /// 保存目录尾斜杠归一（FR-01-03，目录归一单点化的行为锚定）
    #[tokio::test]
    async fn add_dialog_confirm_creates_task_with_normalized_dir() {
        let dir = std::env::temp_dir().join(format!("ezr-app-dlg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut app = App::new(Config::default(), reg);

        // A 打开添加对话框
        app.on_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(app.dialog.is_some());
        // 输入 URL（focus 0）
        for c in "http://example.com/f.bin".chars() {
            app.on_key(KeyCode::Char(c), KeyModifiers::NONE);
        }
        // Tab 到目录字段，输入带尾斜杠的目录
        app.on_key(KeyCode::Tab, KeyModifiers::NONE);
        for c in format!("{}/", dir.to_string_lossy()).chars() {
            app.on_key(KeyCode::Char(c), KeyModifiers::NONE);
        }
        // Enter（非算法/取消焦点）= 确认添加
        app.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(app.dialog.is_none());
        assert_eq!(app.tasks.len(), 1);
        let t = &app.tasks[0];
        assert_eq!(t.name, "f.bin");
        assert_eq!(t.save_dir, dir.to_string_lossy());
        assert_eq!(t.state, TaskState::Queued);
        assert_eq!(t.concurrency, crate::model::config::DEFAULT_CONCURRENCY);
        assert!(app
            .toast
            .as_deref()
            .is_some_and(|m| m.contains("已添加任务")));
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
