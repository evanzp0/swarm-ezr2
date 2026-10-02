//! engine — 主循环 tick：消费引擎事件 → 槽位调度 → 启动/重试推进 → 统计

use std::time::{Duration, Instant};

use super::App;
use crate::engine::{Cmd, Evt};
use crate::model::registry::Registry;
use crate::model::sidecar::Sidecar;
use crate::model::speed::SpeedWindow;
use crate::model::{checksum, namegen, slots, Checksum, FailKind, Task, TaskState};

/// 展示面采样节拍（FR-01-17 修订：数值每秒最多变化一次）
const SPEED_TICK: Duration = Duration::from_secs(1);

impl App {
    /// 校验期望解析（FR-01-50/D3）：显式提供优先；否则查保存目录伴随文件
    /// `<目标文件>.<算法后缀>`（算法表顺序取先，位数不符视为无效并提示）。
    pub(super) fn resolve_checksum(
        save_dir: &str,
        name: &str,
        explicit: Option<Checksum>,
    ) -> Option<Checksum> {
        if explicit.is_some() {
            return explicit;
        }
        checksum::find_companion(save_dir, name).map(|sc| Checksum {
            algo: checksum::CHECKSUM_ALGOS[sc.algo_idx].0,
            value: sc.value,
        })
    }

    /// 构建任务启动规格（Start 前置：读 sidecar 断点、磁盘预检在获槽时）
    #[allow(clippy::too_many_arguments)]
    pub(super) fn make_spec(t: &Task) -> crate::engine::supervisor::TaskSpec {
        let sidecar = Sidecar::load(&t.sidecar_path());
        let sc_path = (sidecar.is_some()).then(|| t.sidecar_path());
        crate::engine::supervisor::TaskSpec {
            id: t.id,
            url: t.url.clone(),
            save_dir: t.save_dir.clone(),
            name: t.name.clone(),
            concurrency: t.concurrency,
            block_size: t.block_size,
            protocol: t.protocol,
            expected_algo: t.checksum.as_ref().map(|c| c.algo),
            expected_value: t.checksum.as_ref().map(|c| c.value.clone()),
            sidecar,
            sidecar_path: sc_path,
            added_at: t.added_at,
        }
    }

    /// 磁盘空间预检（FR-01-44）：可用空间 < 剩余需下载量 → 失败停等
    pub(super) fn disk_precheck(&mut self, idx: usize) -> bool {
        let t = &self.tasks[idx];
        let need = t.total.saturating_sub(t.downloaded);
        if need == 0 || !t.probed {
            return true; // 未探测（无大小）时跳过预检，大小校验兜底
        }
        let dir = std::path::Path::new(&t.save_dir);
        let avail = std::fs::create_dir_all(dir)
            .ok()
            .and_then(|()| fs2::available_space(dir).ok())
            .unwrap_or(u64::MAX);
        if avail < need {
            let t = &mut self.tasks[idx];
            t.state = TaskState::Failed;
            t.fail_kind = Some(FailKind::Fatal);
            t.retry_in = None;
            t.has_slot = false;
            t.error = Some("磁盘空间不足（保存目录剩余空间小于待下载量）".to_string());
            let name = t.name.clone();
            self.set_toast(format!(
                "✘ 磁盘空间不足：开始前预检失败，不自动重试，按 R 手动重试: {name}"
            ));
            return false;
        }
        true
    }

    // -----------------------------------------------------------------------
    // 主循环 tick：消费引擎事件 → 槽位调度 → 启动/重试推进 → 统计
    // -----------------------------------------------------------------------

    /// 推进一帧（每 100ms；消费引擎事件、调度槽位、推进重试倒计时）
    pub async fn tick(&mut self) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_tick)
            .as_secs_f64()
            .clamp(0.001, 0.5);
        self.last_tick = now;
        self.frame = self.frame.wrapping_add(1);

        // 1) 消费引擎事件（本轮全部）
        loop {
            match self.evt_rx.try_recv() {
                Ok(evt) => self.on_evt(evt).await,
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
            }
        }

        // 2) 槽位不变式 + 空闲分配（按列表顺序，FR-01-31/32）
        let granted = {
            let tasks = &mut self.tasks;
            slots::allocate(tasks, self.max_slots)
        };

        // 3) 新获槽位的等待任务 → 启动（磁盘预检 → 发 Start）
        for id in granted {
            let Some(idx) = self.tasks.iter().position(|t| t.id == id) else {
                continue;
            };
            if !self.disk_precheck(idx) {
                continue;
            }
            self.tasks[idx].probed = false;
            let spec = Self::make_spec(&self.tasks[idx]);
            self.engine.send(Cmd::Start { spec }).await;
        }

        // 4) 失败倒计时推进（自动重试：到点 → 等待中重排，槽位保持占用）
        let mut expired: Vec<u32> = Vec::new();
        for t in &mut self.tasks {
            if t.state == TaskState::Failed {
                if let Some(left) = t.retry_in.as_mut() {
                    *left -= dt;
                    if *left <= 0.0 {
                        t.retry_in = None;
                        t.state = TaskState::Queued;
                        t.error = None;
                        expired.push(t.id);
                    }
                }
            }
        }
        // 4a) 到点任务补发 Start（FR-01-41 自动重试）：探针失败型任务
        //     probed=false，下方 4b 的 probed 过滤会永久漏掉它们——故到点
        //     即发，覆盖全部失败类型
        for id in &expired {
            if let Some(idx) = self.tasks.iter().position(|t| t.id == *id) {
                let spec = Self::make_spec(&self.tasks[idx]);
                self.engine.send(Cmd::Start { spec }).await;
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == *id) {
                    // 重试新一轮尝试：清进展标记（连续性判定基准，FR-01-41）
                    t.made_progress = false;
                }
            }
        }
        // 4b) 已获槽位的等待任务重排后需要（重新）Start——Failed→Queued 的重试
        //     由上面 retry_in 到点转 Queued（has_slot 保持）且已由 4a 补发；
        //     本帧仅补发其他来源的 probed 任务（排除 4a 已发者防双开）。
        let retry_starts: Vec<u32> = self
            .tasks
            .iter()
            .filter(|t| {
                t.state == TaskState::Queued && t.has_slot && t.probed && !expired.contains(&t.id)
            })
            .map(|t| t.id)
            .collect();
        for id in retry_starts {
            if let Some(idx) = self.tasks.iter().position(|t| t.id == id) {
                let spec = Self::make_spec(&self.tasks[idx]);
                self.engine.send(Cmd::Start { spec }).await;
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                    // 重试新一轮尝试：清进展标记（连续性判定基准，FR-01-41）
                    t.made_progress = false;
                }
            }
        }

        // 5) 速度展示（FR-01-17 修订）：数据面窗口不变；展示面 1s 节拍采样 + EMA 平滑。
        //    非下载态每 tick 立即归零（零值速断，无衰减拖尾，全局同步扣除）；
        //    下载态每秒采样一次窗口速率推入 EMA，数值每秒最多变化一次。
        //    连接级与任务级同拍同口径（FR-01-81 增补）：非下载态连接行立即归零；
        //    下载态同一节拍内采样各连接窗口速率推入各自的 EMA，明细表速度列
        //    与「传输中/挂起」状态判定同源（均取平滑值）。
        for t in self.tasks.iter_mut() {
            if t.state != TaskState::Downloading {
                if let Some(d) = self.speed_display.get_mut(&t.id) {
                    d.zero();
                }
                t.speed = 0.0;
                for c in &mut t.connections {
                    c.speed = 0.0;
                }
                for ((tid, _), d) in self.conn_speed_display.iter_mut() {
                    if *tid == t.id {
                        d.zero();
                    }
                }
            }
        }
        let speed_due = now.duration_since(self.last_speed_tick) >= SPEED_TICK;
        if speed_due {
            self.last_speed_tick = now;
            for t in self.tasks.iter_mut() {
                if t.state != TaskState::Downloading {
                    continue;
                }
                let sample = self.windows.get_mut(&t.id).map(|w| w.rate()).unwrap_or(0.0);
                let d = self.speed_display.entry(t.id).or_default();
                d.push(sample);
                t.speed = d.value();
                for c in &mut t.connections {
                    let key = (t.id, c.id);
                    let raw = self
                        .conn_windows
                        .get_mut(&key)
                        .map(|w| w.rate())
                        .unwrap_or(0.0);
                    let cd = self.conn_speed_display.entry(key).or_default();
                    cd.push(raw);
                    c.speed = cd.value();
                }
            }
        }
        let global_dl: f64 = self.tasks.iter().map(|t| t.speed).sum();
        self.session_bytes += (global_dl * dt) as u64;
        if speed_due {
            self.push_hist(global_dl, 0.0);
        }

        // 6) toast 过期
        if let Some(u) = self.toast_until {
            if now > u {
                self.toast = None;
                self.toast_until = None;
            }
        }

        // 7) 注册表周期保存（5s 兜底；关键状态转换即时保存）
        if self.last_save.elapsed().as_secs() >= 5 {
            self.save_registry();
        }

        // 8) 选中项 / 滚动钳制
        let flen = self.filtered().len();
        if flen == 0 {
            self.selected = 0;
            self.scroll = 0;
        } else {
            if self.selected >= flen {
                self.selected = flen - 1;
            }
            let vis = self.visible_rows.max(1);
            if self.scroll > self.selected {
                self.scroll = self.selected;
            }
            if self.selected >= self.scroll + vis {
                self.scroll = self.selected + 1 - vis;
            }
            let max_scroll = flen.saturating_sub(vis);
            self.scroll = self.scroll.min(max_scroll);
        }
    }

    /// 引擎事件处理（状态机权威转换点）
    async fn on_evt(&mut self, evt: Evt) {
        match evt {
            Evt::Probed {
                id,
                name,
                final_url,
                total,
                resumable,
                etag,
                last_modified,
                cd_name: _,
            } => {
                // CD 命名回写需重新去重（Gherkin 01-add-task-10：目标被占时保留
                // 原推导名，防止下载覆盖既有文件——先算定名再可变借用）
                let rename_to = {
                    let cur = self.tasks.iter().find(|t| t.id == id);
                    match cur {
                        Some(t0) if t0.name != name && t0.downloaded == 0 => {
                            let taken = |n: &str| {
                                self.tasks
                                    .iter()
                                    .any(|x| x.id != id && x.name == n && x.save_dir == t0.save_dir)
                                    || namegen::exists_on_disk(&t0.save_dir, n)
                            };
                            Some(namegen::dedupe(&name, taken))
                        }
                        _ => None,
                    }
                };
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                    if let Some(nn) = rename_to {
                        t.name = nn;
                    }
                    t.final_url = Some(final_url);
                    t.total = total;
                    t.resumable = resumable;
                    t.probed = true;
                    t.etag = etag;
                    t.last_modified = last_modified;
                    if t.state == TaskState::Queued {
                        t.state = TaskState::Downloading;
                        t.made_progress = false;
                    }
                }
            }
            Evt::Progress {
                id,
                downloaded,
                conns,
                chunk_done,
            } => {
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
                    // 已删除/未知任务的幽灵进度：不建滑窗、不聚合（删除后引擎停止确认前的兜底）
                    self.windows.remove(&id);
                    self.conn_windows.retain(|(tid, _), _| *tid != id);
                    self.conn_speed_display.retain(|(tid, _), _| *tid != id);
                    self.speed_display.remove(&id);
                    return;
                };
                if let Some(w) = self.windows.get_mut(&id) {
                    w.push(Instant::now(), downloaded);
                } else {
                    let mut w = SpeedWindow::new();
                    w.push(Instant::now(), downloaded);
                    self.windows.insert(id, w);
                }
                t.downloaded = downloaded;
                t.chunk_done = chunk_done;
                t.connections = conns.iter().map(|c| c.to_connection()).collect();
                // 展示值回填：重建的连接视图速度默认 0（ConnView 无速度字段），若不回填，
                // 节拍间隔内（~10Hz 事件流）明细表速度列与状态列会被清零、与 1Hz 节拍写出的
                // 平滑值高频交替（缺陷：明细表"一闪一闪"）。回填后 c.speed 始终持平滑值，
                // 展示面唯一数据源口径不变（FR-01-81）；新连接无平滑器条目时保持 0，下拍起写。
                for c in &mut t.connections {
                    if let Some(d) = self.conn_speed_display.get(&(id, c.id)) {
                        c.speed = d.value();
                    }
                }
                // 每连接数据面：按连接累计字节进 1s 滑窗（速率基准，FR-01-81）。
                // 展示值不再在此裸写——统一由 tick 第 5 步按 1s 节拍经 EMA 写出
                // （FR-01-17 修订：连接级与任务级同拍平滑、状态列同源）。
                let now = Instant::now();
                for c in &conns {
                    self.conn_windows
                        .entry((id, c.id))
                        .or_default()
                        .push(now, c.done);
                }
                if downloaded > 0 {
                    t.made_progress = true;
                }
            }
            Evt::PausedDone {
                id,
                downloaded,
                chunk_done,
            } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                    t.downloaded = downloaded.max(t.downloaded);
                    t.speed = 0.0;
                    t.has_slot = false;
                    // 断点视图快照（详情页分块表）；含被覆盖的 t.chunk_done 中间写
                    t.apply_chunk_snapshot(downloaded, chunk_done);
                }
                self.windows.remove(&id);
                self.conn_windows.retain(|(tid, _), _| *tid != id);
                self.conn_speed_display.retain(|(tid, _), _| *tid != id);
            }
            Evt::Failed {
                id,
                kind,
                reason,
                retry_after,
                made_progress,
                downloaded,
                chunk_done,
            } => {
                self.handle_failure(
                    id,
                    kind,
                    reason,
                    retry_after,
                    made_progress,
                    downloaded,
                    chunk_done,
                );
            }
            Evt::Invalidated { id } => {
                // 续传一致性失效（FR-01-22）：断点作废、从头重下、不计失败；
                // 连续 3 次转停等失败（防循环）
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
                    return;
                };
                // 防循环计数（FR-01-22）：连续 3 次失效转停等失败
                let (streak, stop_wait) =
                    crate::model::retry::invalidate_streak(t.invalidation_streak);
                t.invalidation_streak = streak;
                let name = t.name.clone();
                t.downloaded = 0;
                t.chunk_done = 0;
                t.connections.clear();
                t.speed = 0.0;
                self.windows.remove(&id);
                self.conn_windows.retain(|(tid, _), _| *tid != id);
                self.conn_speed_display.retain(|(tid, _), _| *tid != id);
                if stop_wait {
                    t.state = TaskState::Failed;
                    t.fail_kind = Some(FailKind::Fatal);
                    t.retry_in = None;
                    t.has_slot = false;
                    t.error = Some("服务器内容持续变化".to_string());
                    self.set_toast(format!(
                        "✘ 连续 3 次一致性失效，停止自动重试（按 R 手动重试）: {name}"
                    ));
                } else {
                    t.state = TaskState::Queued;
                    t.probed = false;
                    // 引擎任务已随 Flow::Invalidated 结束：必须释放槽位，
                    // 调度器才能重新分配并重启（FR-01-22「从头重新下载」；
                    // slots::allocate 只补 !has_slot 的等待任务，不释放则永久卡等）
                    t.has_slot = false;
                    self.set_toast(format!(
                        "⚠ 服务器内容已更新，断点已作废，从头重新下载: {name}"
                    ));
                }
            }
            Evt::DownloadDone {
                id,
                total,
                has_checksum,
            } => {
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
                    return;
                };
                t.downloaded = total;
                t.total = total;
                t.speed = 0.0;
                self.windows.remove(&id);
                self.conn_windows.retain(|(tid, _), _| *tid != id);
                self.conn_speed_display.retain(|(tid, _), _| *tid != id);
                let name = t.name.clone();
                if has_checksum {
                    let algo = t.checksum.as_ref().map_or("SHA-256", |c| c.algo);
                    // 校验中占槽位（D12：不释放）
                    t.state = TaskState::Verifying;
                    t.has_slot = true;
                    let expected = t
                        .checksum
                        .as_ref()
                        .map_or(String::new(), |c| c.value.clone());
                    let spec = crate::engine::supervisor::VerifySpec {
                        id,
                        path: t.downloading_path(),
                        final_path: t.target_path(),
                        sidecar_path: t.sidecar_path(),
                        algo,
                        expected,
                    };
                    self.engine.send(Cmd::Verify { spec }).await;
                    self.set_toast(format!("✓ 下载完成，开始校验: {name}"));
                } else {
                    // 无校验值直接完成（引擎已收尾：改名 + 删 sidecar）
                    t.state = TaskState::Completed;
                    t.has_slot = false;
                    t.verify_ok = None;
                    t.connections.clear();
                    self.set_toast(format!("✓ 下载完成: {name}（无校验）"));
                }
                self.save_registry();
            }
            Evt::VerifyDone {
                id,
                ok,
                computed,
                expected,
            } => {
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
                    return;
                };
                let algo = t.checksum.as_ref().map_or("SHA-256", |c| c.algo);
                let name = t.name.clone();
                t.speed = 0.0;
                if ok {
                    t.verify_ok = Some(true);
                    t.state = TaskState::Completed;
                    t.has_slot = false;
                    t.error = None;
                    t.connections.clear();
                    self.set_toast(format!("✓ {algo} 校验通过: {name}"));
                } else {
                    t.verify_ok = Some(false);
                    t.state = TaskState::Failed;
                    t.fail_kind = Some(FailKind::Verify);
                    t.retry_in = None;
                    t.has_slot = false; // 校验失败释放槽位（FR-01-51）
                    t.error = Some(format!("{algo} 校验失败：内容与校验值不符"));
                    self.set_toast(format!(
                        "✘ {algo} 校验失败：不自动重试，按 R 重新校验（computed={computed} expected={expected}）: {name}"
                    ));
                }
                self.save_registry();
            }
            Evt::Cancelled { id } => {
                // 引擎已确认停止：执行延迟文件删除（「删除任务和文件」），未知 id 忽略
                if let Some((dir, name)) = self.pending_deletes.remove(&id) {
                    Self::delete_task_files(&dir, &name);
                }
            }
            Evt::Toast(msg) => self.set_toast(msg),
        }
    }

    /// 失败统一处理（FR-01-41/42/43：连续性规则 + 退避 + 停等分类）
    #[allow(clippy::too_many_arguments)]
    fn handle_failure(
        &mut self,
        id: u32,
        kind: FailKind,
        reason: String,
        retry_after: Option<f64>,
        made_progress: bool,
        downloaded: u64,
        chunk_done: u32,
    ) {
        let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
            return;
        };
        if downloaded > 0 {
            t.downloaded = downloaded.max(t.downloaded);
        }
        t.speed = 0.0;
        // 重试决策（FR-01-41/42/43）：连续性规则 + 退避/Retry-After + 分类，
        // 策略唯一来源在 model::retry（DRY）
        let (auto_retry, max_retries) = (self.cfg.auto_retry, t.max_retries);
        let (new_retries, decision) = crate::model::retry::decide(
            kind,
            made_progress,
            t.retries,
            max_retries,
            retry_after,
            auto_retry,
        );
        t.retries = new_retries;
        t.made_progress = false;
        t.state = TaskState::Failed;
        t.fail_kind = Some(kind);
        t.error = Some(reason.clone());
        let name = t.name.clone();
        // max_retries 已在决策前读定（处理中不变），此处仅取更新后的计数供文案
        let retries = t.retries;
        let toast = match decision {
            crate::model::retry::RetryDecision {
                auto: true,
                delay_secs,
                ..
            } => {
                t.retry_in = Some(delay_secs);
                t.has_slot = true; // 待自动重试：继续占用槽位
                format!(
                    "✘ 下载失败（{reason}），{delay_secs:.0}s 后自动重试（{retries}/{max_retries}）: {name}"
                )
            }
            crate::model::retry::RetryDecision { at_limit: true, .. } => {
                t.retry_in = None;
                t.has_slot = false;
                format!(
                    "✘ 已达最大重试次数（{retries}/{max_retries}），停止自动重试，按 R 手动重试: {name}"
                )
            }
            _ => {
                t.retry_in = None;
                t.has_slot = false; // 停等：释放槽位
                format!("✘ {reason}：不自动重试，按 R 手动重试: {name}")
            }
        };
        // 断点视图快照
        t.apply_chunk_snapshot(t.downloaded, chunk_done);
        // t（可变借用）在此作用域结束后自然释放
        self.windows.remove(&id);
        self.conn_windows.retain(|(tid, _), _| *tid != id);
        self.conn_speed_display.retain(|(tid, _), _| *tid != id);
        self.set_toast(toast);
        self.save_registry();
    }

    /// 保存注册表（原子写；状态转换与周期兜底共用）
    pub(crate) fn save_registry(&mut self) {
        self.last_save = Instant::now();
        if let Some(dir) = std::path::Path::new(&self.registry_path).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let reg = Registry::from_tasks(&self.tasks, self.next_id);
        let _ = reg.save(&self.registry_path);
    }

    // -----------------------------------------------------------------------
    // 键盘交互（快捷键全集沿用 demo，FR-01-80）
    // -----------------------------------------------------------------------

    /// 优雅退出收尾：暂停全部传输（引擎落盘 sidecar）→ 保存注册表（FR-01-73）
    pub async fn shutdown(&mut self) {
        let downloading: Vec<u32> = self
            .tasks
            .iter()
            .filter(|t| t.state == TaskState::Downloading)
            .map(|t| t.id)
            .collect();
        for id in downloading {
            self.engine.send(Cmd::Pause { id }).await;
        }
        // 给引擎 sidecar 落盘留出窗口（最多 2s 周期 + 暂停即时落盘）
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        // 引擎已全量取消（Shutdown → TaskCmd::Cancel）：补做未确认的延迟文件删除
        for (_, (dir, name)) in self.pending_deletes.drain() {
            Self::delete_task_files(&dir, &name);
        }
        self.save_registry();
        self.engine.send(Cmd::Shutdown).await;
    }
}

#[cfg(test)]
mod tick_tests {
    use super::*;
    use crate::model::config::Config;
    use crate::model::{sample_task, Connection};

    /// 下载态任务夹具：两连接各持一块（明细表两行传输中）
    fn dl_task_with_conns() -> Task {
        let mut t = sample_task();
        t.id = 7;
        t.state = TaskState::Downloading;
        t.probed = true;
        t.has_slot = true;
        t.total = 2_000_000;
        t.downloaded = 400_000;
        t.connections = vec![
            Connection {
                id: 1,
                start: 0,
                end: 1_000_000,
                done: 250_000,
                speed: 0.0,
            },
            Connection {
                id: 2,
                start: 1_000_000,
                end: 2_000_000,
                done: 150_000,
                speed: 0.0,
            },
        ];
        t
    }

    /// 数据面窗口预置：1 秒间距两个累计读数 → rate ≈ bytes B/s
    fn feed_window(w: &mut SpeedWindow, bytes: u64) {
        let t0 = Instant::now();
        w.push(t0 - Duration::from_secs(1), 0);
        w.push(t0, bytes);
    }

    /// 强制下一 tick 触发 1s 展示节拍
    fn arm_speed_tick(app: &mut App) {
        app.last_speed_tick = Instant::now() - Duration::from_secs(2);
    }

    fn make_app(tag: &str) -> App {
        let dir = std::env::temp_dir().join(format!("ezr-tick-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        App::new(Config::default(), reg)
    }

    /// 若干轮 tick：deadline 前反复推进帧；`done` 返回 true 时提前收束
    async fn pump_until(app: &mut App, deadline: std::time::Instant, done: impl Fn(&App) -> bool) {
        while std::time::Instant::now() < deadline {
            app.tick().await;
            if done(app) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    #[tokio::test]
    async fn tick_drives_failed_state_from_bogus_url() {
        let mut app = make_app("fail");
        let url = "http://127.0.0.1:1/none.bin".to_string();
        let dir = std::env::temp_dir().join("ezr-tick-dl");
        app.add_cli_task(url, Some(dir.to_string_lossy().into_owned()), Some(2), None);
        assert_eq!(app.tasks.len(), 1);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        pump_until(&mut app, deadline, |a| {
            a.tasks[0].state == TaskState::Failed
        })
        .await;
        assert_eq!(app.tasks[0].state, TaskState::Failed, "连接拒绝 → Failed");
        assert!(app.tasks[0].error.is_some());
        app.shutdown().await;
    }

    #[tokio::test]
    async fn tick_completes_download_from_stub_server() {
        // 桩服务器：固定响应头 + 确定性内容（i%251）
        let data: Vec<u8> = (0..128_000u32).map(|i| (i % 251) as u8).collect();
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                let data = data.clone();
                std::thread::spawn(move || {
                    use std::io::{BufRead, BufReader, Write};
                    let mut c = conn;
                    let mut r = BufReader::new(c.try_clone().unwrap());
                    let mut line = String::new();
                    while r.read_line(&mut line).unwrap_or(0) > 0 {
                        if line == "\r\n" {
                            break;
                        }
                        line.clear();
                    }
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        data.len()
                    );
                    let _ = c.write_all(head.as_bytes());
                    let _ = c.write_all(&data);
                    let _ = c.flush();
                });
            }
        });
        let mut app = make_app("done");
        let save = std::env::temp_dir().join(format!("ezr-tick-save-{}", std::process::id()));
        std::fs::create_dir_all(&save).ok();
        app.add_cli_task(
            format!("http://127.0.0.1:{port}/stub.bin"),
            Some(save.to_string_lossy().into_owned()),
            Some(2),
            None,
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        pump_until(&mut app, deadline, |a| {
            a.tasks[0].state == TaskState::Completed
        })
        .await;
        assert_eq!(app.tasks[0].state, TaskState::Completed, "stub 下载应完成");
        let out = save.join("stub.bin");
        assert_eq!(std::fs::metadata(&out).unwrap().len(), 128_000);
        app.shutdown().await;
        std::fs::remove_dir_all(&save).ok();
    }

    /// FR-01-17（修订）/FR-01-81：连接级展示值与任务级同拍平滑——同一 1s 节拍内，
    /// 任务速度与各连接速度都从数据面窗口经 EMA 写出（首拍 = α 份额，非裸差分直传）。
    #[tokio::test]
    async fn tick_smooths_connection_speeds_on_same_beat() {
        let mut app = make_app("conn-smooth");
        app.tasks.push(dl_task_with_conns());
        feed_window(app.windows.entry(7).or_default(), 100_000);
        feed_window(app.conn_windows.entry((7, 1)).or_default(), 60_000);
        feed_window(app.conn_windows.entry((7, 2)).or_default(), 40_000);
        arm_speed_tick(&mut app);
        app.tick().await;
        let t = &app.tasks[0];
        assert!(t.speed > 0.0, "任务速度随节拍写出");
        let raw1 = app
            .conn_windows
            .get_mut(&(7, 1))
            .map(|w| w.rate())
            .unwrap_or(0.0);
        let c1 = t.connections.iter().find(|c| c.id == 1).unwrap();
        let c2 = t.connections.iter().find(|c| c.id == 2).unwrap();
        assert!(c1.speed > 0.0 && c2.speed > 0.0, "连接速度随同一节拍写出");
        assert!(
            c1.speed < raw1 * 0.9,
            "连接展示值是 EMA 份额（首拍 α=1/5），不是裸差分直传：{} vs raw {raw1}",
            c1.speed
        );
        // 同拍门控：1s 内再次 tick 不产生新数值
        let (s_t, s_c1) = (t.speed, c1.speed);
        app.tick().await;
        let t = &app.tasks[0];
        assert_eq!(t.speed, s_t, "节拍未到任务数值不变");
        assert_eq!(
            t.connections.iter().find(|c| c.id == 1).unwrap().speed,
            s_c1,
            "节拍未到连接数值不变（同拍，无独立变拍）"
        );
        app.shutdown().await;
    }

    /// 零值速断（FR-01-17 修订）：任务进入非下载态，连接行展示值立即归零，
    /// 状态列（传输中/挂起）与数字同源，不再残留传输中。
    #[tokio::test]
    async fn tick_zeroes_connection_displays_when_not_downloading() {
        let mut app = make_app("conn-zero");
        app.tasks.push(dl_task_with_conns());
        feed_window(app.windows.entry(7).or_default(), 100_000);
        feed_window(app.conn_windows.entry((7, 1)).or_default(), 60_000);
        feed_window(app.conn_windows.entry((7, 2)).or_default(), 40_000);
        arm_speed_tick(&mut app);
        app.tick().await;
        assert!(
            app.tasks[0].connections.iter().all(|c| c.speed > 0.0),
            "前置：连接展示值已平滑写出"
        );
        app.tasks[0].state = TaskState::Paused;
        app.tick().await;
        let t = &app.tasks[0];
        assert_eq!(t.speed, 0.0, "任务速度立即归零");
        assert!(
            t.connections.iter().all(|c| c.speed == 0.0),
            "连接行速度立即归零（无 EMA 拖尾）"
        );
        app.shutdown().await;
    }

    /// 归零后恢复下载：EMA 从 0 重新爬升（首拍仍为 α 份额），证明归零清除了
    /// 平滑器状态而非冻结旧值——对应场景 16「恢复后各行平滑爬升」。
    #[tokio::test]
    async fn tick_restarts_connection_climb_after_zero_cut() {
        let mut app = make_app("conn-resume");
        app.tasks.push(dl_task_with_conns());
        feed_window(app.windows.entry(7).or_default(), 100_000);
        feed_window(app.conn_windows.entry((7, 1)).or_default(), 60_000);
        feed_window(app.conn_windows.entry((7, 2)).or_default(), 40_000);
        arm_speed_tick(&mut app);
        app.tick().await;
        app.tasks[0].state = TaskState::Paused;
        app.tick().await;
        // 恢复下载：续传 Start 清旧窗（tasks.rs 重启路径），新窗从 0 重新累积
        app.tasks[0].state = TaskState::Downloading;
        app.windows.remove(&7);
        app.conn_windows.retain(|(tid, _), _| *tid != 7);
        feed_window(app.windows.entry(7).or_default(), 100_000);
        feed_window(app.conn_windows.entry((7, 1)).or_default(), 60_000);
        arm_speed_tick(&mut app);
        app.tick().await;
        let raw1 = app
            .conn_windows
            .get_mut(&(7, 1))
            .map(|w| w.rate())
            .unwrap_or(0.0);
        let c1 = app.tasks[0].connections.iter().find(|c| c.id == 1).unwrap();
        assert!(
            c1.speed > 0.0 && c1.speed < raw1 * 0.9,
            "恢复后从 0 平滑爬升（首拍 α 份额）：{} vs raw {raw1}",
            c1.speed
        );
        app.shutdown().await;
    }

    /// 缺陷修复（明细表闪烁）：`Evt::Progress` 每次事件都会整表重建连接视图
    /// （`ConnView::to_connection` 速度默认 0），重建不得清掉 1Hz 节拍写出的
    /// 平滑展示值——否则节拍间隔内（~10Hz 事件流）明细表速度列与状态列在
    /// 「数值/传输中」与「—/挂起」之间高频交替（操作者报告的"一闪一闪"）。
    #[tokio::test]
    async fn progress_rebuild_keeps_smoothed_connection_speeds() {
        use crate::engine::ConnView;
        let mut app = make_app("conn-rebuild");
        app.tasks.push(dl_task_with_conns());
        feed_window(app.windows.entry(7).or_default(), 100_000);
        feed_window(app.conn_windows.entry((7, 1)).or_default(), 60_000);
        feed_window(app.conn_windows.entry((7, 2)).or_default(), 40_000);
        arm_speed_tick(&mut app);
        app.tick().await;
        let speeds = |a: &App| {
            let t = &a.tasks[0];
            (
                t.connections.iter().find(|c| c.id == 1).unwrap().speed,
                t.connections.iter().find(|c| c.id == 2).unwrap().speed,
            )
        };
        let (v1, v2) = speeds(&app);
        assert!(v1 > 0.0 && v2 > 0.0, "前置：节拍已写出连接平滑值");
        // 节拍间隔内到达的 Progress（重建连接视图，速度默认 0）不得清掉展示值
        app.on_evt(Evt::Progress {
            id: 7,
            downloaded: 400_001,
            conns: vec![
                ConnView {
                    id: 1,
                    block: 0,
                    start: 0,
                    end: 1_000_000,
                    done: 250_001,
                },
                ConnView {
                    id: 2,
                    block: 1,
                    start: 1_000_000,
                    end: 2_000_000,
                    done: 150_001,
                },
            ],
            chunk_done: 0,
        })
        .await;
        let (w1, w2) = speeds(&app);
        assert_eq!(w1, v1, "Progress 重建后连接 1 展示值保持平滑值（不被清零）");
        assert_eq!(w2, v2, "Progress 重建后连接 2 展示值保持平滑值（不被清零）");
        // 下一节拍从既有平滑值继续推进（EMA 收敛，而非清零重启）
        arm_speed_tick(&mut app);
        app.tick().await;
        let (n1, _) = speeds(&app);
        assert!(n1 >= v1, "下一节拍延续既有平滑值爬升：{n1} ≥ {v1}");
        app.shutdown().await;
    }
}
