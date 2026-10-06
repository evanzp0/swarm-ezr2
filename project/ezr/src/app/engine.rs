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

    /// 构建任务启动规格（Start 前置：读 sidecar 断点、磁盘预检在获槽时）。
    /// 任务级代理端点由调用方先经 [`App::resolve_endpoint`] 解析传入
    /// （v1.5/FR-01-86/D18：三态选择 → 端点；失效引用 → 直连 + toast 一次）
    pub(super) fn make_spec(
        &self,
        t: &Task,
        proxy_endpoint: Option<crate::model::ProxyEndpoint>,
    ) -> crate::engine::TaskSpec {
        let sidecar = Sidecar::load(&t.sidecar_path());
        let sc_path = (sidecar.is_some()).then(|| t.sidecar_path());
        crate::engine::TaskSpec {
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
            // 任务名已随首次探测定稿（final_url 已知 → 命名规则已走完）：
            // 之后的一切重启（失效重下/续传/重试）沿用既有名。失效重启若重新
            // 去重，会因残留 .downloading 文件漂移新名（m.bin → m.bin.1…），
            // 使 sidecar 落盘名与任务名错位、下轮续传一致性检查被跳过
            // （FR-01-22 违例），故必须钉住。
            keep_name: t.final_url.is_some(),
            added_at: t.added_at,
            proxy_endpoint,
        }
    }

    /// 任务级代理选择 → 端点（v1.5/FR-01-86/D18）。命名引用失效 → 直连 +
    /// toast 一次（Probed 前发出，用户可见；Global 未配置全局 = 直连不提示）
    pub(super) fn resolve_endpoint(
        &mut self,
        choice: &crate::model::ProxyChoice,
    ) -> Option<crate::model::ProxyEndpoint> {
        let (endpoint, missing) = self.cfg.resolve_proxy(choice);
        if missing {
            if let crate::model::ProxyChoice::Named(name) = choice {
                self.set_toast(format!("⚠ 引用的代理「{name}」不存在，已按直连"));
            }
        }
        endpoint
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
            let choice = self.tasks[idx].proxy.clone();
            let endpoint = self.resolve_endpoint(&choice);
            let spec = self.make_spec(&self.tasks[idx], endpoint);
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
        // 会话已下载（FR-01-81「本次运行累计下载字节」）：按各任务真实已下载
        // 字节的运行内增量累计（首次观察到任务时以其当前进度为基线，sidecar
        // 恢复的既有进度不计入）。不积分展示面 EMA 速度——EMA 爬升期的少计
        // 会随任务结束固化（3 MB 短任务实测少计约一半）。
        let mut gained: u64 = 0;
        for t in self.tasks.iter() {
            match self.session_seen.get(&t.id).copied() {
                None => {
                    self.session_seen.insert(t.id, t.downloaded);
                }
                Some(prev) if t.downloaded > prev => {
                    gained += t.downloaded - prev;
                    self.session_seen.insert(t.id, t.downloaded);
                }
                Some(_) => {}
            }
        }
        self.session_bytes += gained;
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
        let choice = self.tasks[idx].proxy.clone();
        let endpoint = self.resolve_endpoint(&choice);
        let spec = self.make_spec(&self.tasks[idx], endpoint);
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
                // CD 命名回写：引擎侧已在建文件前对盘上去重（derive→dedupe 先于
                // 文件创建），App 只需对其它任务同名去重。此处不得再查盘：事件
                // 异步处理时引擎可能已建 <名字>.downloading，盘查会把自己当
                // 占用者而漂移新名（m4.bin → m4.bin.1），使 sidecar 落盘名与
                // 任务名错位、下轮续传一致性检查被跳过（FR-01-22 违例）。
                let rename_to = {
                    let cur = self.tasks.iter().find(|t| t.id == id);
                    match cur {
                        Some(t0) if t0.name != name && t0.downloaded == 0 => {
                            let taken = |n: &str| {
                                self.tasks
                                    .iter()
                                    .any(|x| x.id != id && x.name == n && x.save_dir == t0.save_dir)
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
                // 会话累计账本同步重置基线：失效重下的全部字节都属本次运行
                // 下载（若保留旧基线，重下进度超过旧值前会被漏计）
                self.session_seen.insert(id, 0);
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
                    let spec = crate::engine::VerifySpec {
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
        // v1.6/FR-01-92：用户已手动挂起（FailedPaused）→ 迟到失败事件仅更新
        // 错误信息，不得转回失败态或恢复自动重试（否则挂起语义被竞态覆盖）
        if t.state == TaskState::FailedPaused {
            t.error = Some(reason.clone());
            return;
        }
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

#[cfg(test)]
mod evt_tests {
    //! on_evt arm 级单测（QA 轮登记的单测缺口补齐）：不经引擎/网络，直接构造
    //! `Evt` 驱动状态机权威转换点，锁定每个 arm 与关键内部分支的可观察效果。
    //! 事件全流程（探测→下载→完成→校验）由 tick_tests 与 PTY e2e 套件覆盖。

    use super::*;
    use crate::engine::ConnView;
    use crate::model::config::Config;

    fn make_app(tag: &str) -> App {
        let dir = std::env::temp_dir().join(format!("ezr-evt-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        App::new(Config::default(), reg)
    }

    /// 种子任务：模型层 sample_task 改 id/名字（终态避免被 tick 自动推进；
    /// 本模块不经 tick，直接驱动 on_evt，状态播种即为确定起点）
    fn seed_task(id: u32, name: &str) -> Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t
    }

    fn conn_view(id: usize, start: u64, end: u64, done: u64) -> ConnView {
        ConnView {
            id,
            block: 0,
            start,
            end,
            done,
        }
    }

    // ===== Evt::Probed =====

    #[tokio::test]
    async fn probed_unknown_id_is_noop() {
        let mut app = make_app("probe-ghost");
        app.on_evt(Evt::Probed {
            id: 9,
            name: "x.bin".to_string(),
            final_url: "http://x/x.bin".to_string(),
            total: 10,
            resumable: true,
            etag: None,
            last_modified: None,
        })
        .await;
        assert!(app.tasks.is_empty(), "未知 id：无任务被创建或修改");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn probed_renames_with_dedupe_when_name_taken_in_same_dir() {
        let mut app = make_app("probe-dedupe");
        let mut t1 = seed_task(1, "a.bin");
        t1.save_dir = "/dl".to_string();
        let mut t2 = seed_task(2, "b.bin");
        t2.save_dir = "/dl".to_string();
        app.tasks.push(t1);
        app.tasks.push(t2);
        app.on_evt(Evt::Probed {
            id: 1,
            name: "b.bin".to_string(),
            final_url: "http://x/b.bin".to_string(),
            total: 10,
            resumable: true,
            etag: None,
            last_modified: None,
        })
        .await;
        assert_eq!(app.tasks[0].name, "b.bin.1", "同名任务占用 → 追加 .1 去重");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn probed_renames_plainly_when_name_only_taken_in_other_dir() {
        let mut app = make_app("probe-otherdir");
        let mut t1 = seed_task(1, "a.bin");
        t1.save_dir = "/dl".to_string();
        let mut t2 = seed_task(2, "b.bin");
        t2.save_dir = "/other".to_string();
        app.tasks.push(t1);
        app.tasks.push(t2);
        app.on_evt(Evt::Probed {
            id: 1,
            name: "b.bin".to_string(),
            final_url: "http://x/b.bin".to_string(),
            total: 10,
            resumable: true,
            etag: None,
            last_modified: None,
        })
        .await;
        assert_eq!(app.tasks[0].name, "b.bin", "不同保存目录的同名不算占用");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn probed_skips_rename_when_task_already_has_progress() {
        let mut app = make_app("probe-resumed");
        let mut t1 = seed_task(1, "a.bin");
        t1.save_dir = "/dl".to_string();
        t1.downloaded = 100; // 断点续传进行中
        let mut t2 = seed_task(2, "b.bin");
        t2.save_dir = "/dl".to_string();
        app.tasks.push(t1);
        app.tasks.push(t2);
        app.on_evt(Evt::Probed {
            id: 1,
            name: "b.bin".to_string(),
            final_url: "http://x/b.bin".to_string(),
            total: 10,
            resumable: true,
            etag: None,
            last_modified: None,
        })
        .await;
        assert_eq!(
            app.tasks[0].name, "a.bin",
            "已下载 > 0 时不得改名（FR-01-22 断点一致性）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn probed_fills_fields_and_starts_queued_task() {
        let mut app = make_app("probe-fields");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Queued;
        t.has_slot = true;
        app.tasks.push(t);
        app.on_evt(Evt::Probed {
            id: 1,
            name: "f.bin".to_string(),
            final_url: "http://final/f.bin".to_string(),
            total: 1234,
            resumable: false,
            etag: Some("\"e1\"".to_string()),
            last_modified: Some("yesterday".to_string()),
        })
        .await;
        let t = &app.tasks[0];
        assert!(t.probed, "探测标记置位");
        assert_eq!(t.final_url.as_deref(), Some("http://final/f.bin"));
        assert_eq!(t.total, 1234);
        assert!(!t.resumable);
        assert_eq!(t.etag.as_deref(), Some("\"e1\""));
        assert_eq!(t.last_modified.as_deref(), Some("yesterday"));
        assert_eq!(
            t.state,
            TaskState::Downloading,
            "等待中 + 探测完成 → 下载中"
        );
        assert!(!t.made_progress, "新一轮尝试进展标记从零起算");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn probed_keeps_nonqueued_state_but_marks_probed() {
        let mut app = make_app("probe-paused");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Paused;
        app.tasks.push(t);
        app.on_evt(Evt::Probed {
            id: 1,
            name: "f.bin".to_string(),
            final_url: "http://final/f.bin".to_string(),
            total: 5,
            resumable: true,
            etag: None,
            last_modified: None,
        })
        .await;
        let t = &app.tasks[0];
        assert!(t.probed);
        assert_eq!(t.state, TaskState::Paused, "非等待中状态不被探测改写");
        app.shutdown().await;
    }

    // ===== Evt::Progress =====

    #[tokio::test]
    async fn progress_ghost_cleans_speed_state_without_touching_tasks() {
        let mut app = make_app("progress-ghost");
        app.windows.insert(9, SpeedWindow::new());
        app.speed_display.entry(9).or_default();
        app.on_evt(Evt::Progress {
            id: 9,
            downloaded: 100,
            conns: vec![conn_view(1, 0, 100, 100)],
            chunk_done: 1,
        })
        .await;
        assert!(
            !app.windows.contains_key(&9) && !app.speed_display.contains_key(&9),
            "已删除/未知任务的幽灵进度：速度状态一并清除"
        );
        assert!(app.tasks.is_empty());
        app.shutdown().await;
    }

    #[tokio::test]
    async fn progress_seeds_window_and_updates_task_view() {
        let mut app = make_app("progress-seed");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        app.tasks.push(t);
        app.on_evt(Evt::Progress {
            id: 1,
            downloaded: 500,
            conns: vec![conn_view(2, 0, 1000, 500)],
            chunk_done: 3,
        })
        .await;
        assert!(app.windows.contains_key(&1), "首次进度为任务建速度滑窗");
        let t = &app.tasks[0];
        assert_eq!(t.downloaded, 500);
        assert_eq!(t.chunk_done, 3);
        assert_eq!(t.connections.len(), 1, "连接视图按 ConnView 直传适配");
        assert_eq!(t.connections[0].id, 2);
        assert_eq!(t.connections[0].done, 500);
        assert!(t.made_progress, "downloaded > 0 → 连续性进展标记");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn progress_zero_bytes_does_not_mark_progress() {
        let mut app = make_app("progress-zero");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        app.tasks.push(t);
        app.on_evt(Evt::Progress {
            id: 1,
            downloaded: 0,
            conns: vec![],
            chunk_done: 0,
        })
        .await;
        assert!(!app.tasks[0].made_progress, "零字节进度不算进展");
        assert!(app.windows.contains_key(&1), "窗口仍然建立（观测后续速率）");
        app.shutdown().await;
    }

    // ===== Evt::PausedDone =====

    #[tokio::test]
    async fn paused_done_snapshots_breakpoint_and_clears_window() {
        let mut app = make_app("paused-snap");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.total = 1000;
        t.downloaded = 30;
        t.has_slot = true;
        t.speed = 999.0;
        app.tasks.push(t);
        app.windows.insert(1, SpeedWindow::new());
        app.on_evt(Evt::PausedDone {
            id: 1,
            downloaded: 50,
            chunk_done: 2,
        })
        .await;
        let t = &app.tasks[0];
        assert_eq!(t.downloaded, 50, "事件口径较大值生效（30 → 50）");
        assert_eq!(t.speed, 0.0);
        assert!(!t.has_slot, "暂停完成释放槽位");
        assert_eq!(
            t.chunk_done, 2,
            "断点视图快照含事件块数（snap=0 → max 取 2）"
        );
        assert_eq!(t.connections.len(), 1, "快照推导连接视图（详情页分块表）");
        assert!(!app.windows.contains_key(&1), "速度窗口随暂停清除");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn paused_done_keeps_larger_local_downloaded() {
        let mut app = make_app("paused-max");
        let mut t = seed_task(1, "f.bin");
        t.downloaded = 80;
        app.tasks.push(t);
        app.on_evt(Evt::PausedDone {
            id: 1,
            downloaded: 50,
            chunk_done: 0,
        })
        .await;
        assert_eq!(
            app.tasks[0].downloaded, 80,
            "本地已下载较大时不得回退（事件滞后兜底）"
        );
        app.shutdown().await;
    }

    // ===== Evt::Failed =====

    #[tokio::test]
    async fn failed_transient_schedules_auto_retry_and_keeps_slot() {
        let mut app = make_app("failed-transient");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.downloaded = 100;
        t.has_slot = true;
        app.tasks.push(t);
        app.windows.insert(1, SpeedWindow::new());
        app.on_evt(Evt::Failed {
            id: 1,
            kind: FailKind::Transient,
            reason: "连接被重置".to_string(),
            retry_after: None,
            made_progress: false,
            downloaded: 400,
            chunk_done: 1,
        })
        .await;
        let t = &app.tasks[0];
        assert_eq!(t.state, TaskState::Failed);
        assert_eq!(t.retries, 1, "无进展失败 → 计数累加（FR-01-41）");
        assert_eq!(t.retry_in, Some(8.0), "首轮退避 8s（FR-01-42）");
        assert!(t.has_slot, "待自动重试继续占槽位");
        assert_eq!(t.downloaded, 400, "失败时点进度取较大值落账");
        assert_eq!(t.error.as_deref(), Some("连接被重置"));
        assert_eq!(t.speed, 0.0);
        assert!(!app.windows.contains_key(&1), "失败清除速度窗口");
        let toast = app.toast.clone().unwrap_or_default();
        assert!(
            toast.contains("8s 后自动重试") && toast.contains("1/5"),
            "toast={toast}"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn failed_fatal_releases_slot_and_stops_auto_retry() {
        let mut app = make_app("failed-fatal");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.has_slot = true;
        app.tasks.push(t);
        app.on_evt(Evt::Failed {
            id: 1,
            kind: FailKind::Fatal,
            reason: "HTTP 403".to_string(),
            retry_after: None,
            made_progress: true,
            downloaded: 0,
            chunk_done: 0,
        })
        .await;
        let t = &app.tasks[0];
        assert_eq!(t.state, TaskState::Failed);
        assert_eq!(t.retries, 1, "有进展 → 计数重置为 1");
        assert!(t.retry_in.is_none(), "停等类别不设自动重试倒计时");
        assert!(!t.has_slot, "停等释放槽位");
        let toast = app.toast.clone().unwrap_or_default();
        assert!(
            toast.contains("不自动重试") && toast.contains("按 R 手动重试"),
            "toast={toast}"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn failed_unknown_id_is_noop() {
        let mut app = make_app("failed-ghost");
        app.on_evt(Evt::Failed {
            id: 42,
            kind: FailKind::Transient,
            reason: "ghost".to_string(),
            retry_after: None,
            made_progress: false,
            downloaded: 0,
            chunk_done: 0,
        })
        .await;
        assert!(app.tasks.is_empty());
        app.shutdown().await;
    }

    // ===== Evt::Invalidated =====

    #[tokio::test]
    async fn invalidated_unknown_id_is_noop() {
        let mut app = make_app("inval-ghost");
        app.on_evt(Evt::Invalidated { id: 7 }).await;
        assert!(app.tasks.is_empty(), "未知 id 直接忽略");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn invalidated_first_streak_requeues_and_resets_session_baseline() {
        let mut app = make_app("inval-requeue");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.downloaded = 500;
        t.chunk_done = 1;
        t.has_slot = true;
        t.probed = true;
        t.speed = 42.0;
        t.connections = vec![crate::model::Connection {
            id: 1,
            start: 0,
            end: 100,
            done: 50,
        }];
        app.tasks.push(t);
        app.windows.insert(1, SpeedWindow::new());
        app.session_seen.insert(1, 500);
        app.on_evt(Evt::Invalidated { id: 1 }).await;
        let t = &app.tasks[0];
        assert_eq!(t.invalidation_streak, 1, "连击计数推进（0 → 1）");
        assert_eq!(t.state, TaskState::Queued, "从头重下 → 等待中重排");
        assert!(!t.probed, "重排后需重新探测");
        assert!(
            !t.has_slot,
            "引擎任务已结束，槽位释放供调度器重分（FR-01-22）"
        );
        assert_eq!(t.downloaded, 0);
        assert_eq!(t.chunk_done, 0);
        assert!(t.connections.is_empty());
        assert_eq!(t.speed, 0.0);
        assert!(!app.windows.contains_key(&1));
        assert_eq!(
            app.session_seen.get(&1),
            Some(&0),
            "会话累计账本基线同步归零（重下字节全属本次运行）"
        );
        let toast = app.toast.clone().unwrap_or_default();
        assert!(toast.contains("从头重新下载"), "toast={toast}");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn invalidated_third_streak_fails_and_stops_auto() {
        let mut app = make_app("inval-stop");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.invalidation_streak = 2;
        t.has_slot = true;
        t.retry_in = Some(8.0);
        app.tasks.push(t);
        app.on_evt(Evt::Invalidated { id: 1 }).await;
        let t = &app.tasks[0];
        assert_eq!(t.invalidation_streak, 3);
        assert_eq!(
            t.state,
            TaskState::Failed,
            "连续 3 次失效 → 停等失败（防循环）"
        );
        assert_eq!(t.fail_kind, Some(FailKind::Fatal));
        assert!(t.retry_in.is_none());
        assert!(!t.has_slot);
        assert_eq!(t.error.as_deref(), Some("服务器内容持续变化"));
        let toast = app.toast.clone().unwrap_or_default();
        assert!(toast.contains("连续 3 次一致性失效"), "toast={toast}");
        app.shutdown().await;
    }

    // ===== Evt::DownloadDone =====

    #[tokio::test]
    async fn download_done_without_checksum_completes_and_clears() {
        let mut app = make_app("done-nocheck");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.has_slot = true;
        t.connections = vec![crate::model::Connection {
            id: 1,
            start: 0,
            end: 100,
            done: 100,
        }];
        app.tasks.push(t);
        app.windows.insert(1, SpeedWindow::new());
        app.on_evt(Evt::DownloadDone {
            id: 1,
            total: 1000,
            has_checksum: false,
        })
        .await;
        let t = &app.tasks[0];
        assert_eq!(t.state, TaskState::Completed, "无校验值直接完成");
        assert_eq!(t.downloaded, 1000);
        assert_eq!(t.total, 1000);
        assert!(!t.has_slot);
        assert!(t.verify_ok.is_none(), "未校验 → 结果 None");
        assert!(t.connections.is_empty(), "连接视图随完成清空");
        assert!(!app.windows.contains_key(&1));
        let toast = app.toast.clone().unwrap_or_default();
        assert!(toast.contains("无校验"), "toast={toast}");
        assert!(
            std::path::Path::new(&app.registry_path).exists(),
            "完成态即时落盘注册表"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn download_done_with_checksum_moves_to_verifying_and_keeps_slot() {
        let mut app = make_app("done-check");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.has_slot = true;
        t.checksum = Some(Checksum {
            algo: "MD5",
            value: "aa".to_string(),
        });
        app.tasks.push(t);
        app.on_evt(Evt::DownloadDone {
            id: 1,
            total: 64,
            has_checksum: true,
        })
        .await;
        let t = &app.tasks[0];
        assert_eq!(
            t.state,
            TaskState::Verifying,
            "有校验值 → 校验中（D12 占槽位）"
        );
        assert!(t.has_slot, "校验中不释放槽位（D12）");
        assert_eq!(t.speed, 0.0);
        let toast = app.toast.clone().unwrap_or_default();
        assert!(toast.contains("开始校验"), "toast={toast}");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn download_done_unknown_id_is_noop() {
        let mut app = make_app("done-ghost");
        app.on_evt(Evt::DownloadDone {
            id: 11,
            total: 1,
            has_checksum: false,
        })
        .await;
        assert!(app.tasks.is_empty());
        app.shutdown().await;
    }

    // ===== Evt::VerifyDone =====

    #[tokio::test]
    async fn verify_done_ok_completes_and_clears_error() {
        let mut app = make_app("verify-ok");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Verifying;
        t.has_slot = true;
        t.error = Some("历史错误残留".to_string());
        t.checksum = Some(Checksum {
            algo: "MD5",
            value: "aa".to_string(),
        });
        t.connections = vec![crate::model::Connection {
            id: 1,
            start: 0,
            end: 10,
            done: 10,
        }];
        app.tasks.push(t);
        app.on_evt(Evt::VerifyDone {
            id: 1,
            ok: true,
            computed: "aa".to_string(),
            expected: "aa".to_string(),
        })
        .await;
        let t = &app.tasks[0];
        assert_eq!(t.state, TaskState::Completed);
        assert_eq!(t.verify_ok, Some(true));
        assert!(t.error.is_none(), "校验通过清除错误行");
        assert!(!t.has_slot);
        assert!(t.connections.is_empty());
        let toast = app.toast.clone().unwrap_or_default();
        assert!(toast.contains("MD5 校验通过"), "toast={toast}");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn verify_done_mismatch_fails_without_auto_retry() {
        let mut app = make_app("verify-bad");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Verifying;
        t.has_slot = true;
        t.checksum = Some(Checksum {
            algo: "MD5",
            value: "aa".to_string(),
        });
        app.tasks.push(t);
        app.on_evt(Evt::VerifyDone {
            id: 1,
            ok: false,
            computed: "bb".to_string(),
            expected: "aa".to_string(),
        })
        .await;
        let t = &app.tasks[0];
        assert_eq!(t.state, TaskState::Failed);
        assert_eq!(t.fail_kind, Some(FailKind::Verify));
        assert_eq!(t.verify_ok, Some(false));
        assert!(t.retry_in.is_none(), "校验失败不自动重试（FR-01-51）");
        assert!(!t.has_slot, "校验失败释放槽位");
        assert_eq!(t.error.as_deref(), Some("MD5 校验失败：内容与校验值不符"));
        let toast = app.toast.clone().unwrap_or_default();
        assert!(
            toast.contains("computed=bb") && toast.contains("expected=aa"),
            "toast 带实测/期望摘要：{toast}"
        );
        app.shutdown().await;
    }

    // ===== Evt::Cancelled =====

    #[tokio::test]
    async fn cancelled_executes_pending_delete_then_unknown_id_ignored() {
        let mut app = make_app("cancelled-del");
        let dir = std::env::temp_dir().join(format!("ezr-evt-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let base = dir.to_string_lossy().trim_end_matches('/').to_string();
        for f in ["f.bin", "f.bin.downloading", "f.bin.ezr"] {
            std::fs::write(format!("{base}/{f}"), b"x").unwrap();
        }
        app.pending_deletes
            .insert(5, (base.clone(), "f.bin".to_string()));
        // 相位一：未知 id → 不触碰延迟删除表（负向探测单发口径）
        app.on_evt(Evt::Cancelled { id: 99 }).await;
        assert!(
            std::path::Path::new(&format!("{base}/f.bin")).exists(),
            "未知 id 的停止确认不触发删除"
        );
        assert!(app.pending_deletes.contains_key(&5));
        // 相位二：确认 id → 三类本地文件删除、登记清除
        app.on_evt(Evt::Cancelled { id: 5 }).await;
        for f in ["f.bin", "f.bin.downloading", "f.bin.ezr"] {
            assert!(
                !std::path::Path::new(&format!("{base}/{f}")).exists(),
                "{f} 应被删除"
            );
        }
        assert!(app.pending_deletes.is_empty(), "已执行的登记应清除");
        std::fs::remove_dir_all(&dir).ok();
        app.shutdown().await;
    }

    // ===== Evt::Toast =====

    #[tokio::test]
    async fn toast_sets_message_with_fresh_expiry() {
        let mut app = make_app("toast");
        app.on_evt(Evt::Toast("引擎提示".to_string())).await;
        assert_eq!(app.toast.as_deref(), Some("引擎提示"));
        assert!(app.toast_until.is_some(), "toast 附带过期时刻");
        app.shutdown().await;
    }

    // ===== 磁盘预检（disk_precheck）=====

    /// 开始前磁盘预检三分支：未探测/无余量跳过、需求 ≤ 可用空间通过、
    /// 需求超可用空间转 Fatal 失败并提示（不自动重试）
    #[tokio::test]
    async fn disk_precheck_skips_passes_and_fails_overneed() {
        let dir = std::env::temp_dir().join(format!("ezr-evt-disk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let mut app = make_app("disk");

        // 未探测（probed=false）→ 跳过预检返回 true
        let mut t = seed_task(1, "d1.bin");
        t.probed = false;
        t.total = u64::MAX;
        t.save_dir = dir.to_string_lossy().into_owned();
        app.tasks.push(t);
        app.selected = 0;
        assert!(app.disk_precheck(0), "未探测时跳过预检");
        assert_eq!(app.tasks[0].state, crate::model::TaskState::Queued);

        // 已探测但需求为 0 → 通过
        app.tasks[0].probed = true;
        app.tasks[0].total = 0;
        assert!(app.disk_precheck(0), "无待下载量时通过");
        assert_eq!(app.tasks[0].state, crate::model::TaskState::Queued);

        // 已探测、需求 ≤ 可用空间 → 通过
        app.tasks[0].total = 1024;
        assert!(app.disk_precheck(0), "小需求通过预检");

        // 需求超可用空间（u64::MAX 需求必超）→ Fatal 失败 + 提示
        app.tasks[0].total = u64::MAX;
        assert!(!app.disk_precheck(0), "超量需求预检失败");
        assert_eq!(app.tasks[0].state, crate::model::TaskState::Failed);
        assert_eq!(
            app.tasks[0].fail_kind,
            Some(crate::model::FailKind::Fatal),
            "预检失败为 Fatal 不自动重试"
        );
        assert!(app.tasks[0].retry_in.is_none());
        assert!(!app.tasks[0].has_slot);
        assert!(
            app.tasks[0]
                .error
                .as_deref()
                .is_some_and(|m| m.contains("磁盘空间不足")),
            "错误信息: {:?}",
            app.tasks[0].error
        );
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("磁盘空间不足")),
            "toast 提示"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
