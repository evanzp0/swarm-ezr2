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

/// ConnView → Connection 消费侧适配（FR-01-81 修订二：进度视图字段直传，
/// 块号不进 UI 模型）。原为低层 `ConnView::to_connection` 构造方法；架构评审
/// 依 notes/rust.md「低层不得构造高层展示类型」移到消费侧（低层暴露纯数据，
/// 高层在消费侧内联构造）。
fn conn_from_view(c: &crate::engine::ConnView) -> crate::model::Connection {
    crate::model::Connection {
        id: c.id,
        start: c.start,
        end: c.end,
        done: c.done,
    }
}

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
            self.send_retry_start(*id).await;
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
            self.send_retry_start(id).await;
        }

        // 5) 速度展示（FR-01-17 修订）：数据面窗口不变；展示面 1s 节拍采样 + EMA 平滑。
        //    非下载态每 tick 立即归零（零值速断，无衰减拖尾，全局同步扣除）；
        //    下载态每秒采样一次窗口速率推入 EMA，数值每秒最多变化一次。
        //    （FR-01-81 修订二：连接级展示随明细表移除而撤销，仅任务级三展示面。）
        for t in self.tasks.iter_mut() {
            if t.state != TaskState::Downloading {
                if let Some(d) = self.speed_display.get_mut(&t.id) {
                    d.zero();
                }
                t.speed = 0.0;
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

        // 7) 注册表周期保存（5s 兜底；关键状态转换即时保存；底层原子写对
        //    内容未变化的空闲期短路跳过——零写放大）
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

    /// 重试补发 Start 并清进展标记（FR-01-41：新一轮尝试的连续性判定从零起算；
    /// 4a 到点重试与 4b 重排补发共用，未知 id 静默跳过）
    async fn send_retry_start(&mut self, id: u32) {
        let Some(idx) = self.tasks.iter().position(|t| t.id == id) else {
            return;
        };
        let spec = Self::make_spec(&self.tasks[idx]);
        self.engine.send(Cmd::Start { spec }).await;
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.made_progress = false;
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
                t.connections = conns.iter().map(conn_from_view).collect();
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

    /// 桩 HTTP 服务器：固定响应头 + 确定性内容（i%251）；返回监听地址。
    /// 线程 detach（engineering.md：无退出条件的测试线程一律 detach）。
    fn stub_server(len: u64) -> std::net::SocketAddr {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let data: Vec<u8> = (0..len as u32).map(|i| (i % 251) as u8).collect();
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
        addr
    }

    /// 泵到完成态并断言（stub 全流程下载类测试共用）
    async fn pump_to_completed(app: &mut App, deadline: std::time::Instant) {
        pump_until(app, deadline, |a| a.tasks[0].state == TaskState::Completed).await;
        assert_eq!(app.tasks[0].state, TaskState::Completed, "stub 下载应完成");
    }

    #[tokio::test]
    async fn tick_completes_download_from_stub_server() {
        // 桩服务器：固定响应头 + 确定性内容（i%251）
        let addr = stub_server(128_000);
        let mut app = make_app("done");
        let save = std::env::temp_dir().join(format!("ezr-tick-save-{}", std::process::id()));
        std::fs::create_dir_all(&save).ok();
        app.add_cli_task(
            format!("http://{addr}/stub.bin"),
            Some(save.to_string_lossy().into_owned()),
            Some(2),
            None,
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        pump_to_completed(&mut app, deadline).await;
        let out = save.join("stub.bin");
        assert_eq!(std::fs::metadata(&out).unwrap().len(), 128_000);
        app.shutdown().await;
        std::fs::remove_dir_all(&save).ok();
    }

    /// 退避到点自动重试（tick 4a → send_retry_start）：retry_in 到点转等待中、
    /// 补发 Start 并清进展标记，重试一轮后计数递增（FR-01-41）
    #[tokio::test]
    async fn retry_expiry_resends_start_and_counts_round() {
        let mut app = make_app("retry4a");
        app.add_cli_task(
            "http://127.0.0.1:1/none.bin".to_string(),
            None,
            Some(1),
            None,
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        pump_until(&mut app, deadline, |a| {
            a.tasks[0].state == TaskState::Failed
        })
        .await;
        let retries_first = app.tasks[0].retries;
        // 退避压进观察窗：到点 → Queued → 4a 补发 Start（清进展标记）→ 探测再失败
        app.tasks[0].retry_in = Some(0.05);
        app.tasks[0].made_progress = true;
        pump_until(&mut app, deadline, |a| a.tasks[0].retries > retries_first).await;
        assert!(
            app.tasks[0].retries > retries_first,
            "退避到点后自动重试新一轮（Start 补发）"
        );
        assert_eq!(app.tasks[0].state, TaskState::Failed, "重试后再次失败");
        assert!(!app.tasks[0].made_progress, "新一轮尝试进展标记已清");
        app.shutdown().await;
    }

    /// 重排后的已获槽+已探测任务重发 Start（tick 4b → send_retry_start）：
    /// 若 4b 未补发，任务永远停在等待中；补发后走完探测→下载→完成全流程
    #[tokio::test]
    async fn requeued_probed_task_gets_restart_from_slot() {
        let addr = stub_server(64_000);
        let mut app = make_app("retry4b");
        let save = std::env::temp_dir().join(format!("ezr-tick-save4b-{}", std::process::id()));
        std::fs::create_dir_all(&save).ok();
        app.add_cli_task(
            format!("http://{addr}/stub4b.bin"),
            Some(save.to_string_lossy().into_owned()),
            Some(2),
            None,
        );
        // 置于 4b 过滤态：等待中 + 已获槽 + 已探测（重排后待重发的口径）
        app.tasks[0].state = TaskState::Queued;
        app.tasks[0].has_slot = true;
        app.tasks[0].probed = true;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        pump_to_completed(&mut app, deadline).await;
        app.shutdown().await;
        std::fs::remove_dir_all(&save).ok();
    }

    /// 全局速度历史窗口上限（180 点、KB/s 采样）与 toast 过期清理
    #[tokio::test]
    async fn push_hist_caps_length_and_toast_expires() {
        let mut app = make_app("hist-toast");
        app.push_hist(1024.0, 0.0);
        assert_eq!(app.speed_hist.last(), Some(&1), "KB/s 采样换算");
        for _ in 0..300 {
            app.push_hist(0.0, 0.0);
        }
        assert_eq!(app.speed_hist.len(), 180, "历史窗口上限 180 点");
        assert_eq!(app.up_hist.len(), 180);
        app.set_toast("测试提示");
        assert!(app.toast.is_some());
        app.toast_until = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        app.tick().await;
        assert!(app.toast.is_none(), "过期 toast 清除");
        app.shutdown().await;
    }

    /// ConnView → Connection 消费侧映射锁定（FR-01-81 修订二：进度视图字段
    /// 直传，块号不进 UI 模型；原锁定测试随适配面自 engine 层同步迁入）
    #[test]
    fn conn_view_maps_progress_fields() {
        let cv = crate::engine::ConnView {
            id: 3,
            block: 1,
            start: 100,
            end: 1100,
            done: 400,
        };
        let c = conn_from_view(&cv);
        assert_eq!(c.id, 3);
        assert_eq!(c.start, 100);
        assert_eq!(c.end, 1100);
        assert_eq!(c.done, 400);
    }

    /// make_app 变体：同时返回注册表落盘路径（空闲期零写放大测试用）
    fn make_app_with_reg(tag: &str) -> (App, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("ezr-tick-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        (App::new(Config::default(), reg), dir.join("registry.json"))
    }

    /// 空闲期注册表零写放大（tick 第 7 步周期兜底）：全任务终态（无传输、无
    /// 进度推进）时，越过 5s 窗口的周期保存不得重写内容未变的注册表文件
    /// （mtime 不变为证）；状态漂移（进度推进）后下一个周期窗口必须落盘。
    #[tokio::test]
    async fn idle_tick_skips_registry_rewrite_until_state_drifts() {
        let (mut app, reg_path) = make_app_with_reg("idlesave");
        // 种子：终态任务（已完成，无引擎交互 = 空闲形态；避开会被后台自动化
        // 推进的等待/下载态，engineering.md「种子状态避开后台自动化」）
        let mut t = crate::model::sample_task();
        t.id = 1;
        t.state = TaskState::Completed;
        t.total = 200;
        t.downloaded = 100;
        app.tasks.push(t);
        app.next_id = 2;
        // 第一帧：越过 5s 窗口 → 周期保存落盘（基线）
        app.last_save = std::time::Instant::now() - std::time::Duration::from_secs(10);
        app.tick().await;
        let m1 = std::fs::metadata(&reg_path).unwrap().modified().unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        // 第二帧：仍空闲、内容未变 → 越过 5s 窗口但不得重写
        app.last_save = std::time::Instant::now() - std::time::Duration::from_secs(10);
        app.tick().await;
        let m2 = std::fs::metadata(&reg_path).unwrap().modified().unwrap();
        assert_eq!(m1, m2, "空闲期周期保存不得重写内容未变的注册表（零写放大）");
        // 第三帧：持久化字段漂移（进度推进模拟）→ 下一个周期窗口必须落盘
        app.tasks[0].downloaded = 150;
        app.last_save = std::time::Instant::now() - std::time::Duration::from_secs(10);
        app.tick().await;
        let reg = Registry::load(reg_path.to_str().unwrap()).expect("注册表应可加载");
        assert_eq!(reg.tasks[0].downloaded, 150, "内容漂移后周期保存必须落盘");
        app.shutdown().await;
        std::fs::remove_dir_all(reg_path.parent().unwrap()).ok();
    }
}
