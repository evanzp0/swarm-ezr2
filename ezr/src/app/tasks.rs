//! tasks — 任务操作：暂停/继续、重试、重排、清理

use super::App;
use crate::engine::Cmd;
use crate::model::sidecar::Sidecar;
use crate::model::{checksum, FailKind, TaskState};

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
                // 已暂停 → 继续 = 一律重新排队（v1.21/FR-01-33 修订，操作者
                // 20261012 指令：「有空闲槽直接开跑（Downloading + Start）」
                // 路径废除）——统一经等待队列由槽位调度启动（FR-01-32），
                // 磁盘预检、列表序公平分配、连接级累计清零口径随队列路径单源；
                // 失败挂起与普通暂停同路径，恢复即清错误信息与
                // 计数（v1.21/FR-01-92 合并口径；v1.23 起挂起时即已重置）
                self.resume_paused(idx);
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
                // 清倒计时、转「已暂停」（v1.21 合并口径：独立 FailedPaused 状态
                // 退役）。v1.23（操作者 20261012 bug 报告直判）：挂起时即重置
                // error=None、retries=1——数据层与普通暂停完全一致（注册表快照
                // 同口径：error=null/retries=1）；fail_kind 保留仅供校验失败型
                // 恢复走重新校验分支（D10）
                let id = self.tasks[idx].id;
                self.tasks[idx].state = TaskState::Paused;
                self.tasks[idx].error = None;
                self.tasks[idx].retries = 1;
                self.tasks[idx].retry_in = None;
                self.tasks[idx].has_slot = false;
                let engine = self.engine.clone();
                tokio::spawn(async move {
                    // 引擎侧如有已排队重试任务（retry 到点前），一并取消
                    engine.send(Cmd::Cancel { id }).await;
                });
                let name = self.tasks[idx].name.clone();
                self.set_toast(format!(
                    "⏸ 已暂停（失败），不再自动重试；按空格恢复: {name}"
                ));
            }
            TaskState::Completed => self.set_toast("该任务已完成，无需操作"),
            _ => self.set_toast("当前状态下不可暂停/继续"),
        }
    }

    /// R：失败任务手动重试（FR-01-34/D10：计数重置 1、断点续传、重新排队）；
    /// 挂起态（Paused，v1.21 合并口径）不可 R——空格为唯一恢复键
    /// （v1.19/FR-01-92 收窄；v1.22 起该态展示与普通已暂停一致）
    pub fn retry(&mut self) {
        let Some(idx) = self.sel_idx() else { return };
        if matches!(self.tasks[idx].state, TaskState::Failed) {
            self.requeue_failed(idx);
        } else {
            self.set_toast("仅「已失败」的任务可以重试");
        }
    }

    /// 重新排队公共字段重置（R 手动重试与空格恢复共用，v1.21 统一口径）：
    /// 转等待中、error/fail_kind/retry_in 清空、retries 重置 1、
    /// 进展标记清零、槽位释放。
    /// 失效连击**不在**此处清（QA v138 会话缺陷修复，01-resume-sidecar-08 回归）：
    /// v1.21 统一队列路径后空格恢复也经本函数，若在此清连击，「连续 3 次一致性
    /// 失效转停等」（FR-01-22/RS-08）经暂停→恢复循环永不达成（每次恢复清零，
    /// 服务器内容持续变化时无限从头重下）。连击清零是「操作者已知悉内容变化」
    /// 语义（RS-09），归 R 手动重试路径（[`Self::requeue_failed`]）独有。
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

    /// 已暂停 → 继续（空格恢复统一路径，v1.21/FR-01-33 修订）：一律重新排队，
    /// 字段重置同 [`Self::requeue_reset`]（v1.23 起失败挂起在挂起时即已重置
    /// error/retries，恢复路径数据上与普通暂停无差别）；校验失败挂起走重新
    /// 校验分支（D10）。toast 两臂按续传支持双臂（断点恢复 / 从头下载，
    /// 均注明已进入等待队列）
    fn resume_paused(&mut self, idx: usize) {
        let resumable = self.tasks[idx].resumable;
        let verify_fail = self.tasks[idx].fail_kind == Some(FailKind::Verify);
        let id = self.tasks[idx].id;
        self.requeue_reset(idx);
        if verify_fail {
            // 校验失败挂起的恢复 = 重新校验路径（期望值解析三分支同 R 口径）
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

    /// 失败任务手动重试（R，FR-01-34/D10）：公共重置 + 校验失败重校验分支；
    /// （旧「空格恢复」入参形态已随 v1.21 统一路径并入 [`Self::resume_paused`]）
    /// R 臂连接级累计清零（04-ui-conns-09「重试开新一次下载后各连接累计量从零
    /// 重新累计」，FR-01-99 ②）：重试 = 新的一次下载。已探测任务的引擎重启
    /// 跳过探测（Probe 事件的 Queued 臂清零不触发），故在此显式清——与空格
    /// 恢复臂（累计延续，不清零）形成两臂分立口径。
    fn requeue_failed(&mut self, idx: usize) {
        let verify_fail = self.tasks[idx].fail_kind == Some(FailKind::Verify);
        let id = self.tasks[idx].id;
        self.requeue_reset(idx);
        if verify_fail {
            self.requeue_verify_failed(idx, "手动重试", id);
            return;
        }
        // 重锚标记：清账本后首个 Progress 观测以当前块内 done 重锚（增量 0），
        // 续传检查点偏移不计入新一次下载的累计（04-ui-conns-09 R 臂口径）
        self.conn_rebase.insert(id);
        self.clear_conn_stats(id);
        // 失效计数重置（Gherkin 01-resume-sidecar-09「按 R 重置失效计数」：
        // 手动重试代表操作者已知悉内容变化，重新以最新内容为基准计数）——
        // R 路径独有语义；空格恢复不清（见 requeue_reset 文档）
        self.tasks[idx].invalidation_streak = 0;
        let name = self.tasks[idx].name.clone();
        self.set_toast(format!("↻ 手动重试（等待中）: {name}"));
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

#[cfg(test)]
mod requeue_conn_stats_tests {
    //! R 臂连接级累计清零（04-ui-conns-09 R 臂 / FR-01-99 ②；QA v138 会话缺陷
    //! 修复回归锚）：已探测任务的引擎重启跳过 Probe 事件（Queued 臂清零不
    //! 触发），手动重试必须显式清连接级账本——R 后累计从零重计；空格恢复臂
    //! （resume_paused）不清零（累计延续，两臂分立口径）。

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
        // 灌连接级账本（Progress 计量产物；字段模块私有，子模块测试可直填）
        a.conn_cum.insert((1, 1), 123_456);
        a.conn_prev.insert((1, 1), (0, 1 << 20));
        a.selected = 0;
        // R 臂：非校验失败 → 手动重试清账本
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
        // 恢复臂不清连击（RS-08：连续 3 次一致性失效经暂停→恢复循环可达成）
        a.tasks[0].invalidation_streak = 2;
        a.tasks[0].state = TaskState::Paused;
        a.resume_paused(0);
        assert_eq!(
            a.tasks[0].invalidation_streak, 2,
            "空格恢复不清失效连击（连击跨暂停→恢复累积，RS-08 口径）"
        );
        // 恢复臂：空格恢复不清账本（累计延续）
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
