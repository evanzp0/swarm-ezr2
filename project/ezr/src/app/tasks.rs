use std::time::Instant;

use super::App;
use crate::engine::Cmd;
use crate::model::sidecar::Sidecar;
use crate::model::{checksum, FailKind, TaskState};

impl App {
    pub fn toggle_pause(&mut self) {
        let Some(idx) = self.sel_idx() else { return };
        let name = self.tasks[idx].name.clone();
        match self.tasks[idx].state {
            TaskState::Downloading => {
                self.tasks[idx].state = TaskState::Paused;
                self.tasks[idx].speed = 0.0;
                self.tasks[idx].has_slot = false;
                self.windows.remove(&self.tasks[idx].id);
                let id = self.tasks[idx].id;
                let is_bt = self.tasks[idx].protocol.is_bt();
                if is_bt {
                    // 引擎异步消费 Pause；恢复必须等 BtStopped 确认旧引擎退出，
                    // 否则两个引擎同写文件与 sidecar，进度互相回退。
                    self.bt_stopping.insert(id, Instant::now());
                }
                let engine = self.engine.clone();
                tokio::spawn(async move {
                    if is_bt {
                        engine.send(Cmd::PauseBt { id }).await;
                    } else {
                        engine.send(Cmd::Pause { id }).await;
                    }
                });
                let toast = if self.tasks[idx].resumable {
                    format!("⏸ 已暂停，释放下载槽位（断点已保存）: {name}")
                } else {
                    format!("⏸ 已暂停（服务器不支持断点续传，继续时将从头下载）: {name}")
                };
                self.set_toast(toast);
            }
            TaskState::Paused => {
                self.resume_paused(idx);
            }
            TaskState::Queued => {
                let held = self.tasks[idx].has_slot;
                let id = self.tasks[idx].id;
                let is_bt = self.tasks[idx].protocol.is_bt();
                self.tasks[idx].state = TaskState::Paused;
                self.tasks[idx].has_slot = false;
                if held {
                    if is_bt {
                        self.bt_stopping.insert(id, Instant::now());
                    }
                    let engine = self.engine.clone();
                    tokio::spawn(async move {
                        if is_bt {
                            engine.send(Cmd::CancelBt { id }).await;
                        } else {
                            engine.send(Cmd::Cancel { id }).await;
                        }
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
                let id = self.tasks[idx].id;
                let is_bt = self.tasks[idx].protocol.is_bt();
                self.tasks[idx].state = TaskState::Paused;
                self.tasks[idx].error = None;
                self.tasks[idx].retries = 1;
                self.tasks[idx].retry_in = None;
                self.tasks[idx].has_slot = false;
                if is_bt {
                    self.bt_stopping.insert(id, Instant::now());
                }
                let engine = self.engine.clone();
                tokio::spawn(async move {
                    if is_bt {
                        engine.send(Cmd::CancelBt { id }).await;
                    } else {
                        engine.send(Cmd::Cancel { id }).await;
                    }
                });
                let name = self.tasks[idx].name.clone();
                self.set_toast(format!(
                    "⏸ 已暂停（失败），不再自动重试；按空格恢复: {name}"
                ));
            }
            TaskState::Completed => self.set_toast("该任务已完成，无需操作"),
            TaskState::Seeding => {
                let id = self.tasks[idx].id;
                self.tasks[idx].state = TaskState::Completed;
                self.tasks[idx].upload_speed = 0.0;
                self.tasks[idx].has_slot = false;
                let engine = self.engine.clone();
                tokio::spawn(async move {
                    engine.send(Cmd::StopSeed { id }).await;
                });
                self.set_toast(format!("⏸ 已停止做种（进入已完成）: {name}"));
                self.save_registry();
            }
            _ => self.set_toast("当前状态下不可暂停/继续"),
        }
    }

    pub fn retry(&mut self) {
        let Some(idx) = self.sel_idx() else { return };
        if matches!(self.tasks[idx].state, TaskState::Failed) {
            self.requeue_failed(idx);
        } else {
            self.set_toast("仅「已失败」的任务可以重试");
        }
    }

    fn requeue_reset(&mut self, idx: usize) {
        let t = &mut self.tasks[idx];
        t.state = TaskState::Queued;
        t.error = None;
        t.retry_in = None;
        t.fail_kind = None;
        t.retries = 1;
        t.made_progress = false;
        t.has_slot = false;
    }

    fn resume_paused(&mut self, idx: usize) {
        let resumable = self.tasks[idx].resumable;
        let verify_fail = self.tasks[idx].fail_kind == Some(FailKind::Verify);
        let id = self.tasks[idx].id;
        self.requeue_reset(idx);
        if verify_fail {
            self.requeue_verify_failed(idx, "重新排队", id);
            return;
        }
        let name = self.tasks[idx].name.clone();
        let toast = if resumable {
            format!("▶ 继续下载（从断点恢复，已进入等待队列）: {name}")
        } else {
            format!("⚠ 服务器不支持断点续传，将从头下载（已进入等待队列）: {name}")
        };
        self.set_toast(toast);
    }

    fn requeue_failed(&mut self, idx: usize) {
        let verify_fail = self.tasks[idx].fail_kind == Some(FailKind::Verify);
        let id = self.tasks[idx].id;
        self.requeue_reset(idx);
        if verify_fail {
            self.requeue_verify_failed(idx, "手动重试", id);
            return;
        }
        self.conn_rebase.insert(id);
        self.clear_conn_stats(id);
        self.tasks[idx].invalidation_streak = 0;
        let name = self.tasks[idx].name.clone();
        self.set_toast(format!("↻ 手动重试（等待中）: {name}"));
    }

    fn requeue_verify_failed(&mut self, idx: usize, label: &str, id: u32) {
        let (save_dir, name) = (
            self.tasks[idx].save_dir.clone(),
            self.tasks[idx].name.clone(),
        );
        self.tasks[idx].checksum = if checksum::find_companion(&save_dir, &name).is_some() {
            Self::resolve_checksum(&save_dir, &name, None)
        } else {
            Self::resolve_checksum(&save_dir, &name, self.tasks[idx].checksum.clone())
        };
        if self.tasks[idx].checksum.is_some() {
            self.start_reverify(idx, id);
            return;
        }
        self.direct_finalize_or_discard(idx, label, id);
    }

    fn start_reverify(&mut self, idx: usize, id: u32) {
        self.tasks[idx].state = TaskState::Verifying;
        self.tasks[idx].has_slot = true;
        let spec = crate::engine::VerifySpec {
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
    }

    fn direct_finalize_or_discard(&mut self, idx: usize, label: &str, id: u32) {
        let total = self.tasks[idx].total;
        let dl_path = self.tasks[idx].downloading_path();
        let fin_path = self.tasks[idx].target_path();
        let sc_path = self.tasks[idx].sidecar_path();
        let name = self.tasks[idx].name.clone();
        let intact = |p: &str| {
            total > 0 && std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() == total)
        };
        let dl_intact = intact(&dl_path);
        let fin_intact = intact(&fin_path);
        if dl_intact || fin_intact {
            if dl_intact {
                let _ = std::fs::rename(&dl_path, &fin_path);
            }
            let _ = Sidecar::remove(&sc_path);
            {
                let t = &mut self.tasks[idx];
                t.state = TaskState::Completed;
                t.verify_ok = None;
                t.speed = 0.0;
                t.connections.clear();
            }
            self.windows.remove(&id);
            self.clear_conn_stats(id);
            self.set_toast(format!("✓ 文件已完整，直接完成（无校验）: {name}"));
            self.save_registry();
            return;
        }
        let _ = Sidecar::remove(&sc_path);
        {
            let t = &mut self.tasks[idx];
            t.downloaded = 0;
            t.chunk_done = 0;
            t.connections.clear();
            t.speed = 0.0;
        }
        self.windows.remove(&id);
        self.session_seen.insert(id, 0);
        self.clear_conn_stats(id);
        self.set_toast(format!(
            "↻ {label}（本地文件不完整，断点已作废，将从头下载）: {name}"
        ));
    }

    pub fn clear_completed(&mut self) {
        let before = self.tasks.len();
        let removed: Vec<u32> = self
            .tasks
            .iter()
            .filter(|t| t.state == TaskState::Completed)
            .map(|t| t.id)
            .collect();
        self.tasks.retain(|t| t.state != TaskState::Completed);
        for id in removed {
            self.windows.remove(&id);
            self.clear_conn_stats(id);
            self.clear_bt_conn_stats(id);
        }
        let n = before - self.tasks.len();
        if n > 0 {
            self.set_toast(format!("已清理 {n} 个已完成任务"));
        } else {
            self.set_toast("没有可清理的已完成任务");
        }
        self.save_registry();
    }
}

#[cfg(test)]
mod requeue_conn_stats_tests {

    use super::*;
    use crate::model::config::Config;
    use crate::model::{unix_now, Protocol};

    #[tokio::test]
    async fn retry_clears_conn_cum_resume_keeps() {
        let reg = std::env::temp_dir().join(format!(
            "ezr-qa-retry-cum-{}-{}.json",
            std::process::id(),
            unix_now()
        ));
        let mut a = App::new(Config::default(), reg.to_string_lossy().into_owned());
        a.tasks.push(crate::model::Task::new_queued(
            1,
            "t.bin".to_string(),
            Protocol::Http,
            "http://example.com/t.bin".to_string(),
            "/tmp".to_string(),
            1 << 20,
            4,
            3,
            None,
            crate::model::ProxyChoice::Direct,
            unix_now(),
        ));
        a.conn_cum.insert((1, 1), 123_456);
        a.conn_prev.insert((1, 1), (0, 1 << 20));
        a.selected = 0;
        a.tasks[0].state = TaskState::Failed;
        a.tasks[0].fail_kind = Some(FailKind::Fatal);
        a.retry();
        assert!(
            !a.conn_cum.contains_key(&(1, 1)),
            "R 后连接级累计应清零（重试 = 新的一次下载）"
        );
        assert!(
            !a.conn_prev.contains_key(&(1, 1)),
            "R 后连接级观测位应随账本一并清除"
        );
        assert!(
            a.conn_rebase.contains(&1),
            "R 后应置重锚标记（首个 Progress 观测以当前 done 重锚，不计检查点偏移）"
        );
        assert_eq!(
            a.tasks[0].invalidation_streak, 0,
            "R 重置失效连击（RS-09：手动重试 = 已知悉内容变化）"
        );
        a.tasks[0].invalidation_streak = 2;
        a.tasks[0].state = TaskState::Paused;
        a.resume_paused(0);
        assert_eq!(
            a.tasks[0].invalidation_streak, 2,
            "空格恢复不清失效连击（连击跨暂停→恢复累积，RS-08 口径）"
        );
        a.conn_cum.insert((1, 1), 123_456);
        a.conn_prev.insert((1, 1), (0, 1 << 20));
        a.tasks[0].state = TaskState::Paused;
        a.tasks[0].fail_kind = None;
        a.resume_paused(0);
        assert_eq!(
            a.conn_cum.get(&(1, 1)),
            Some(&123_456),
            "空格恢复连接级累计应延续（不清零臂）"
        );
        let _ = std::fs::remove_file(&reg);
    }
}

#[cfg(test)]
mod seeding_stop_tests {

    use super::*;
    use crate::model::config::Config;
    use crate::model::{unix_now, Protocol};

    fn seeding_bt(id: u32) -> crate::model::Task {
        let mut t = crate::model::Task::new_queued(
            id,
            "seed.bin".to_string(),
            Protocol::Bt,
            "/tmp/a.torrent".to_string(),
            "/tmp".to_string(),
            1 << 20,
            4,
            3,
            None,
            crate::model::ProxyChoice::Direct,
            unix_now(),
        );
        t.state = TaskState::Seeding;
        t.upload_speed = 4096.0;
        t.uploaded = 123_456;
        t.seed_left = 1800.0;
        t
    }

    #[tokio::test]
    async fn space_on_seeding_stops_seed_to_completed() {
        let reg = std::env::temp_dir().join(format!(
            "ezr-qa-seed-{}-{}.json",
            std::process::id(),
            unix_now()
        ));
        let mut a = App::new(Config::default(), reg.to_string_lossy().into_owned());
        a.tasks.push(seeding_bt(1));
        a.filter = 1;
        a.selected = 0;
        a.toggle_pause();
        assert_eq!(
            a.tasks[0].state,
            TaskState::Completed,
            "做种中空格 → 已完成"
        );
        assert_eq!(a.tasks[0].upload_speed, 0.0, "上传速度即时归零");
        assert!(
            a.toast
                .as_deref()
                .is_some_and(|m| m.contains("⏸ 已停止做种（进入已完成）: seed.bin")),
            "停止做种 toast: {:?}",
            a.toast
        );
        let _ = std::fs::remove_file(&reg);
        a.shutdown().await;
    }

    #[tokio::test]
    async fn space_on_completed_still_rejected() {
        let reg = std::env::temp_dir().join(format!(
            "ezr-qa-seed2-{}-{}.json",
            std::process::id(),
            unix_now()
        ));
        let mut a = App::new(Config::default(), reg.to_string_lossy().into_owned());
        let mut t = seeding_bt(1);
        t.state = TaskState::Completed;
        a.tasks.push(t);
        a.filter = 1;
        a.selected = 0;
        a.toggle_pause();
        assert_eq!(a.tasks[0].state, TaskState::Completed, "已完成不变");
        assert!(
            a.toast.as_deref().is_some_and(|m| m.contains("已完成")),
            "已完成 toast 口径不变: {:?}",
            a.toast
        );
        let _ = std::fs::remove_file(&reg);
        a.shutdown().await;
    }
}
