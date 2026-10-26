use std::time::Instant;

use super::*;

impl App {
    pub(super) fn move_conn_sel(&mut self, delta: i32) {
        let Some(idx) = self.sel_idx() else { return };

        if !self.tasks[idx].state.shows_conns() {
            self.set_toast("当前状态无并发明细（仅下载中/做种中可选）");
            return;
        }
        let n = self.tasks[idx]
            .connections
            .iter()
            .filter(|c| c.is_active())
            .count();
        if n == 0 {
            self.set_toast("当前任务没有并发连接明细");
            return;
        }
        let new = (self.conns_sel as i32 + delta).clamp(0, n as i32 - 1) as usize;
        self.conns_sel = new;

        let vis = self.visible_conns_rows.max(1);
        if new < self.conns_scroll {
            self.conns_scroll = new;
        } else if new >= self.conns_scroll + vis {
            self.conns_scroll = new + 1 - vis;
        }
    }

    pub(super) fn disconnect_conn(&mut self) {
        let Some(idx) = self.sel_idx() else { return };

        if !self.tasks[idx].state.shows_conns() {
            self.set_toast("当前状态无可断开的并发连接（仅下载中/做种中）");
            return;
        }
        if !self.tasks[idx].protocol.is_bt() {
            self.set_toast("仅 BT 任务支持断开并发连接（B）");
            return;
        }

        let active_idx: Vec<usize> = self.tasks[idx]
            .connections
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_active())
            .map(|(i, _)| i)
            .collect();
        if active_idx.is_empty() {
            self.set_toast("当前任务没有可断开的并发连接");
            return;
        }
        let disp = self.conns_sel.min(active_idx.len() - 1);
        self.conns_sel = disp;
        let left = active_idx.len() - 1;
        let piece = chunk_size(self.tasks[idx].protocol);
        let masked = {
            let t = &mut self.tasks[idx];
            let c = t.connections.remove(active_idx[disp]);
            if c.cap() > 0 && c.done < c.cap() {
                let y = chunk_total(t.total, piece);
                t.chunk_done = (t.chunk_done + 1).min(y);
            }
            c.ip_masked()
        };

        if self.conns_sel >= left {
            self.conns_sel = left.saturating_sub(1);
        }
        self.set_toast(format!("✂ 已断开连接 {}（剩余 {} 条）", masked, left));
    }

    pub fn toggle_pause(&mut self) {
        let Some(idx) = self.sel_idx() else { return };
        let name = self.tasks[idx].name.clone();
        match self.tasks[idx].state {
            TaskState::Downloading => {
                self.tasks[idx].state = TaskState::Paused;
                self.tasks[idx].speed = 0.0;
                self.tasks[idx].upload_speed = 0.0;
                self.tasks[idx].running_since = None;
                self.tasks[idx].has_slot = false;
                for c in self.tasks[idx].connections.iter_mut() {
                    c.speed = 0.0;
                }
                let toast = if self.tasks[idx].resumable {
                    format!("⏸ 已暂停，释放下载槽位（断点已保存）: {}", name)
                } else {
                    format!(
                        "⏸ 已暂停（服务器不支持断点续传，继续时将从头下载）: {}",
                        name
                    )
                };
                self.set_toast(toast);
            }
            TaskState::Paused => {
                let resumable = self.tasks[idx].resumable;
                let verify_fail = self.tasks[idx].fail_kind == Some(FailKind::Verify);
                self.requeue_reset(idx);
                self.tasks[idx].queued_since = Some(Instant::now());
                self.tasks[idx].start_delay = 1.5;
                if verify_fail {
                    self.requeue_verify_branch(idx);
                    return;
                }
                let toast = if resumable {
                    format!("▶ 继续下载（从断点恢复，已进入等待队列）: {name}")
                } else {
                    format!("⚠ 服务器不支持断点续传，将从头下载（已进入等待队列）: {name}")
                };
                self.set_toast(toast);
            }
            TaskState::Queued => {
                let held = self.tasks[idx].has_slot;
                self.tasks[idx].state = TaskState::Paused;
                self.tasks[idx].has_slot = false;
                self.tasks[idx].queued_since = None;
                self.tasks[idx].running_since = None;
                let toast = if held {
                    format!("⏸ 已暂停（退出等待队列，释放下载槽位）: {}", name)
                } else {
                    format!("⏸ 已暂停（退出等待队列）: {}", name)
                };
                self.set_toast(toast);
            }
            TaskState::Failed => {
                self.tasks[idx].state = TaskState::Paused;
                self.tasks[idx].error = None;
                self.tasks[idx].retries = 1;
                self.tasks[idx].retry_in = None;
                self.tasks[idx].has_slot = false;
                self.set_toast(format!(
                    "⏸ 已暂停（失败），不再自动重试；按空格恢复: {name}"
                ));
            }
            TaskState::Seeding => {
                self.tasks[idx].state = TaskState::Completed;
                self.tasks[idx].upload_speed = 0.0;
                self.tasks[idx].connections.clear();
                self.set_toast(format!("⏸ 已停止做种（进入已完成）: {}", name));
            }
            TaskState::Completed => self.set_toast("该任务已完成，无需操作".to_string()),
            _ => self.set_toast("当前状态下不可暂停/继续".to_string()),
        }
    }

    pub fn retry(&mut self) {
        let Some(idx) = self.sel_idx() else { return };
        if self.tasks[idx].state == TaskState::Failed {
            self.requeue_failed(idx);
        } else {
            self.set_toast("仅「已失败」的任务可以重试".to_string());
        }
    }

    fn requeue_reset(&mut self, idx: usize) {
        self.tasks[idx].state = TaskState::Queued;
        self.tasks[idx].error = None;
        self.tasks[idx].retry_in = None;
        self.tasks[idx].fail_kind = None;
        self.tasks[idx].retries = 1;
        self.tasks[idx].made_progress = false;
        self.tasks[idx].has_slot = false;
    }

    fn requeue_verify_branch(&mut self, idx: usize) {
        if self.tasks[idx].checksum.is_some() {
            self.tasks[idx].state = TaskState::Verifying;
            self.tasks[idx].has_slot = true;
            self.tasks[idx].verify_started = Some(Instant::now());
            let name = self.tasks[idx].name.clone();
            self.set_toast(format!("↻ 重新校验（无块重传）: {name}"));
            return;
        }

        let name = self.tasks[idx].name.clone();
        self.tasks[idx].state = TaskState::Completed;
        self.tasks[idx].verify_ok = None;
        self.tasks[idx].speed = 0.0;
        self.tasks[idx].connections.clear();
        self.set_toast(format!("✓ 文件已完整，直接完成（无校验）: {name}"));
    }

    fn requeue_failed(&mut self, idx: usize) {
        let verify_fail = self.tasks[idx].fail_kind == Some(FailKind::Verify);
        self.requeue_reset(idx);
        if verify_fail {
            self.requeue_verify_branch(idx);
            return;
        }
        self.tasks[idx].queued_since = Some(Instant::now());
        self.tasks[idx].start_delay = 3.0;
        let name = self.tasks[idx].name.clone();
        self.set_toast(format!("↻ 手动重试（等待中）: {name}"));
    }

    pub fn clear_completed(&mut self) {
        let before = self.tasks.len();
        self.tasks.retain(|t| t.state != TaskState::Completed);
        let n = before - self.tasks.len();
        if n > 0 {
            self.set_toast(format!("已清理 {} 个已完成任务", n));
        } else {
            self.set_toast("没有可清理的已完成任务".to_string());
        }
    }
}
