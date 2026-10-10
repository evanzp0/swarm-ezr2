//! tasks — 任务操作：暂停/继续、重试、重排、清理

use super::App;
use crate::engine::Cmd;
use crate::model::sidecar::Sidecar;
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
                let choice = self.tasks[idx].proxy.clone();
                let id = self.tasks[idx].id;
                self.windows.remove(&id);
                let engine = self.engine.clone();
                let was_resumable = self.tasks[idx].resumable;
                if !was_resumable {
                    // 不支持续传 = 从头下载 = 新的一次下载：连接级累计清零（FR-01-99 ②）
                    self.clear_conn_stats(id);
                }
                let resume_toast = if was_resumable {
                    format!("▶ 继续下载（从断点恢复）: {name}")
                } else {
                    format!("⚠ 服务器不支持断点续传，已从头开始下载: {name}")
                };
                // 恢复提示先行；引用缺失提醒（resolve_endpoint 内）在其后发出
                // 时覆盖本提示 —— 缺失提醒必须可见（02-named-proxy-05：
                // toast 提醒一次；toast 槽位单一，后发覆盖先发）
                self.set_toast(resume_toast);
                let endpoint = self.resolve_endpoint(&choice);
                let spec = self.make_spec(&self.tasks[idx], endpoint);
                tokio::spawn(async move {
                    engine.send(Cmd::Start { spec }).await;
                });
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
                // v1.6/FR-01-92/D25：失败任务按空格 = 暂停（挂起自动重试）——
                // 清倒计时、转「已暂停（失败）」；错误信息与失败类型保留可见
                let id = self.tasks[idx].id;
                self.tasks[idx].state = TaskState::FailedPaused;
                self.tasks[idx].retry_in = None;
                self.tasks[idx].has_slot = false;
                let engine = self.engine.clone();
                tokio::spawn(async move {
                    // 引擎侧如有已排队重试任务（retry 到点前），一并取消
                    engine.send(Cmd::Cancel { id }).await;
                });
                let name = self.tasks[idx].name.clone();
                self.set_toast(format!(
                    "⏸ 已暂停（失败），不再自动重试；按空格或 R 恢复: {name}"
                ));
            }
            TaskState::FailedPaused => {
                // 已暂停（失败）→ 恢复 = 重新排队（计数重置、断点续传口径同 R；
                // 校验失败型仍走重新校验路径）
                self.requeue_failed(idx, false);
            }
            TaskState::Completed => self.set_toast("该任务已完成，无需操作"),
            _ => self.set_toast("当前状态下不可暂停/继续"),
        }
    }

    /// R：失败任务手动重试（FR-01-34/D10：计数重置 1、断点续传、重新排队）；
    /// v1.6/FR-01-92：「已暂停（失败）」同样可 R（= 恢复重新排队）
    pub fn retry(&mut self) {
        let Some(idx) = self.sel_idx() else { return };
        if matches!(
            self.tasks[idx].state,
            TaskState::Failed | TaskState::FailedPaused
        ) {
            self.requeue_failed(idx, true);
        } else {
            self.set_toast("仅「已失败」的任务可以重试");
        }
    }

    /// 失败任务重新排队（R/Space）：计数重置 1、回等待队列、断点续传；
    /// 校验失败（Verify）按 R = 重新校验（块全满、无数据重传，D10）。
    fn requeue_failed(&mut self, idx: usize, manual: bool) {
        let verify_fail = self.tasks[idx].fail_kind == Some(FailKind::Verify);
        let label = if manual {
            "手动重试"
        } else {
            "重新排队"
        };
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
            self.requeue_verify_failed(idx, label, id);
            return;
        }
        let name = self.tasks[idx].name.clone();
        self.set_toast(format!("↻ {label}（等待中）: {name}"));
    }

    /// 校验失败重排三分支（D10；v1.17 三分支修订，FR-01-51/D34）：期望值解析
    /// ①伴随文件存在 → 以伴随现值为准（D10 重查语义：若失败源于期望值写错
    ///   AC-6，修正伴随文件后按 R 即可通过）；
    /// ②伴随缺失、任务显式校验值在 → 按显式值重校验（D3 显式优先）；
    /// ③伴随缺失、显式值也已清空（03-modify-task-05）→ **不校验**：不进
    ///   「校验中」、不发校验指令、不重传——原实现③仍发空期望 Verify
    ///   （VerifySpec 落 algo 默认/expected 空串，空串比对恒败），「已失败」R
    ///   后仍显示校验失败，废止（操作者 20261009 第十四批指令，缺陷 B）。
    /// 分支①②进 [`Self::start_reverify`]；分支③落
    /// [`Self::direct_finalize_or_discard`] 文件完整性直判。
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

    /// 分支①②：期望值已解析 → 进「校验中」下发校验指令（占槽位，D12）。
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

    /// 分支③ v1.18 修订 B（FR-01-51 v1.18；D34「不重传直接收尾」实现精化）：
    /// 文件完整性直判——校验失败时全块已完成、文件已 100% 落盘，按文件实况
    /// 判定而非经引擎恢复（旧路径按过期 sidecar 恢复会回退重传尾巴，快速下载
    /// 无 sidecar 时整体重下）：
    /// ① 文件在且大小与 total 一致 → 不经引擎直接收尾「已完成/无校验」；
    /// ② 文件缺失/大小不符（外部改动，台账与磁盘脱节）→ 作废断点从头重下
    ///    （FR-01-22 作废语义同源，不信任与磁盘实况脱节的全块完成台账）。
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
            // 直判命中：收尾以文件实际位置为准（引擎先行改名历史形态，比照
            // evt.rs DownloadDone 校验臂）；命中 `.downloading` 则改名，
            // 命中目标则免改名
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
        // fallback：文件缺失/大小不符 → 作废断点从头重下（state 保持 Queued，
        // 重下字节全属本次运行，比照 Invalidated 臂重置会话基线）
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

    /// C：清理已完成任务
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
