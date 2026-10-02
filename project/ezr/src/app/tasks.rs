//! tasks — 任务操作：暂停/继续、重试、重排、清理

use super::App;
use crate::engine::Cmd;
use crate::model::{checksum, slots, FailKind, TaskState};

impl App {
    /// Space：暂停/继续（FR-01-33）
    pub fn toggle_pause(&mut self) {
        let Some(idx) = self.sel_idx() else { return };
        let name = self.tasks[idx].name.clone();
        match self.tasks[idx].state {
            TaskState::Downloading => {
                // 下载中 → 暂停（引擎停传写 sidecar；UI 即时转已暂停）。
                // 校验中不可暂停：落至下方兜底臂提示（与原「两变体+永假守卫」行为一致）
                self.tasks[idx].state = TaskState::Paused;
                self.tasks[idx].speed = 0.0;
                self.tasks[idx].has_slot = false;
                self.windows.remove(&self.tasks[idx].id);
                let id = self.tasks[idx].id;
                let engine = self.engine.clone();
                tokio::spawn(async move {
                    engine.send(Cmd::Pause { id }).await;
                });
                let toast = if self.tasks[idx].resumable {
                    format!("⏸ 已暂停，释放下载槽位（断点已保存）: {name}")
                } else {
                    format!("⏸ 已暂停（服务器不支持断点续传，继续时将从头下载）: {name}")
                };
                self.set_toast(toast);
            }
            TaskState::Paused => {
                // 已暂停 → 继续：需空闲槽位；满则进等待队列（FR-01-33）
                if slots::used(&self.tasks) >= self.max_slots {
                    let used = slots::used(&self.tasks);
                    self.tasks[idx].state = TaskState::Queued;
                    self.tasks[idx].has_slot = false;
                    let n = self.max_slots;
                    self.set_toast(format!(
                        "⏳ 无空闲下载槽位（{used}/{n}），已进入等待队列: {name}"
                    ));
                    return;
                }
                // 直接继续（探测已做过；sidecar 断点由引擎加载）
                self.tasks[idx].state = TaskState::Downloading;
                self.tasks[idx].has_slot = true;
                self.tasks[idx].made_progress = false;
                let spec = Self::make_spec(&self.tasks[idx]);
                let id = self.tasks[idx].id;
                self.windows.remove(&id);
                self.conn_windows.retain(|(tid, _), _| *tid != id);
                self.conn_speed_display.retain(|(tid, _), _| *tid != id);
                let engine = self.engine.clone();
                let was_resumable = self.tasks[idx].resumable;
                tokio::spawn(async move {
                    engine.send(Cmd::Start { spec }).await;
                });
                let toast = if was_resumable {
                    format!("▶ 继续下载（从断点恢复）: {name}")
                } else {
                    format!("⚠ 服务器不支持断点续传，已从头开始下载: {name}")
                };
                self.set_toast(toast);
            }
            TaskState::Queued => {
                // 等待中 → 暂停并退出等待队列；已获槽位者取消引擎任务
                let held = self.tasks[idx].has_slot;
                let id = self.tasks[idx].id;
                self.tasks[idx].state = TaskState::Paused;
                self.tasks[idx].has_slot = false;
                if held {
                    let engine = self.engine.clone();
                    tokio::spawn(async move {
                        engine.send(Cmd::Cancel { id }).await;
                    });
                }
                let toast = if held {
                    format!("⏸ 已暂停（退出等待队列，释放下载槽位）: {name}")
                } else {
                    format!("⏸ 已暂停（退出等待队列）: {name}")
                };
                self.set_toast(toast);
            }
            TaskState::Failed => {
                self.requeue_failed(idx, false);
            }
            TaskState::Completed => self.set_toast("该任务已完成，无需操作"),
            _ => self.set_toast("当前状态下不可暂停/继续"),
        }
    }

    /// R：失败任务手动重试（FR-01-34/D10：计数重置 1、断点续传、重新排队）
    pub fn retry(&mut self) {
        let Some(idx) = self.sel_idx() else { return };
        if self.tasks[idx].state == TaskState::Failed {
            self.requeue_failed(idx, true);
        } else {
            self.set_toast("仅「已失败」的任务可以重试");
        }
    }

    /// 失败任务重新排队（R/Space）：计数重置 1、回等待队列、断点续传；
    /// 校验失败（Verify）按 R = 重新校验（块全满、无数据重传，D10）。
    fn requeue_failed(&mut self, idx: usize, manual: bool) {
        let verify_fail = self.tasks[idx].fail_kind == Some(FailKind::Verify);
        let id = self.tasks[idx].id;
        self.tasks[idx].state = TaskState::Queued;
        self.tasks[idx].error = None;
        self.tasks[idx].retry_in = None;
        self.tasks[idx].fail_kind = None;
        self.tasks[idx].retries = 1;
        self.tasks[idx].made_progress = false;
        // 失效计数重置（Gherkin 01-resume-sidecar-09「按 R 重置失效计数」：
        // 手动重试代表操作者已知悉内容变化，重新以最新内容为基准计数）
        self.tasks[idx].invalidation_streak = 0;
        self.tasks[idx].has_slot = false;
        if verify_fail {
            // 重新校验（D10）：期望值重查伴随文件——若失败源于期望值写错（AC-6），
            // 修正伴随文件后按 R 即可通过。伴随文件存在时以伴随现值为准（D10
            // 重查语义）；伴随缺失时保留原期望值（显式提供场景，D3 显式优先）。
            let (save_dir, name) = (
                self.tasks[idx].save_dir.clone(),
                self.tasks[idx].name.clone(),
            );
            if checksum::find_companion(&save_dir, &name).is_some() {
                self.tasks[idx].checksum = Self::resolve_checksum(&save_dir, &name, None);
            } else {
                self.tasks[idx].checksum =
                    Self::resolve_checksum(&save_dir, &name, self.tasks[idx].checksum.clone());
            }
            self.tasks[idx].state = TaskState::Verifying;
            self.tasks[idx].has_slot = true;
            let spec = crate::engine::supervisor::VerifySpec {
                id,
                path: self.tasks[idx].downloading_path(),
                final_path: self.tasks[idx].target_path(),
                sidecar_path: self.tasks[idx].sidecar_path(),
                algo: self.tasks[idx]
                    .checksum
                    .as_ref()
                    .map_or("SHA-256", |c| c.algo),
                expected: self.tasks[idx]
                    .checksum
                    .as_ref()
                    .map_or(String::new(), |c| c.value.clone()),
            };
            let engine = self.engine.clone();
            tokio::spawn(async move {
                engine.send(Cmd::Verify { spec }).await;
            });
            let name = self.tasks[idx].name.clone();
            self.set_toast(format!("↻ 重新校验（无块重传）: {name}"));
            return;
        }
        let name = self.tasks[idx].name.clone();
        let label = if manual {
            "手动重试"
        } else {
            "重新排队"
        };
        self.set_toast(format!("↻ {label}（等待中）: {name}"));
    }

    /// C：清理已完成任务
    pub fn clear_completed(&mut self) {
        let before = self.tasks.len();
        self.tasks.retain(|t| t.state != TaskState::Completed);
        let n = before - self.tasks.len();
        if n > 0 {
            self.set_toast(format!("已清理 {n} 个已完成任务"));
        } else {
            self.set_toast("没有可清理的已完成任务");
        }
        self.save_registry();
    }
}
