//! engine::tick — 主循环 tick（消费引擎事件 → 槽位调度 → 启动/重试推进 → 统计）
//!
//! 自单文件按职责拆出（architect v116，纯移动）。tick 原为 9 步单函数
//! （CRAP 21），本轮已先做粗粒度保持行为拆分为六段管线（段内局部变量不跨段）；
//! 更细粒度拆分需参数化跨段局部量，属不安全形态，留 hardender 裁决。

use std::time::{Duration, Instant};

use crate::app::App;
use crate::engine::Cmd;
use crate::model::{slots, TaskState};

/// 展示面采样节拍（FR-01-17 修订：数值每秒最多变化一次）
const SPEED_TICK: Duration = Duration::from_secs(1);

impl App {
    // -----------------------------------------------------------------------
    // 主循环 tick：消费引擎事件 → 槽位调度 → 启动/重试推进 → 统计
    // -----------------------------------------------------------------------

    /// 推进一帧（每 100ms；消费引擎事件、调度槽位、推进重试倒计时）。
    /// architect v116 粗粒度保持行为拆分：原 9 步单函数（CRAP 21）按段内
    /// 局部变量不跨段切成六个私有方法，顺序与原实现完全一致（分发核边界
    /// 裁决——更细粒度拆分需参数化跨段局部量，属不安全形态，留 hardender）。
    pub async fn tick(&mut self) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_tick)
            .as_secs_f64()
            .clamp(0.001, 0.5);
        self.last_tick = now;
        self.frame = self.frame.wrapping_add(1);

        self.tick_consume_events().await;
        self.tick_grant_and_start().await;
        self.tick_retry_countdown(dt).await;
        self.tick_speed_and_session(now);
        self.tick_housekeeping(now);
        self.tick_clamp_selection();
    }

    /// tick ① 消费引擎事件（本轮全部）
    async fn tick_consume_events(&mut self) {
        loop {
            match self.evt_rx.try_recv() {
                Ok(evt) => self.on_evt(evt).await,
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
            }
        }
    }

    /// tick ②③ 槽位不变式 + 空闲分配（按列表顺序，FR-01-31/32）→ 新获槽位
    /// 的等待任务启动（磁盘预检 → 发 Start）
    async fn tick_grant_and_start(&mut self) {
        let granted = {
            let tasks = &mut self.tasks;
            slots::allocate(tasks, self.max_slots)
        };
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
    }

    /// tick ④ 失败倒计时推进（自动重试：到点 → 等待中重排，槽位保持占用）
    /// + 4a 到点补发 Start + 4b 已获槽位等待任务重排补发
    async fn tick_retry_countdown(&mut self, dt: f64) {
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
    }

    /// tick ⑤ 速度展示（FR-01-17 修订）：数据面窗口不变；展示面 1s 节拍采样 +
    /// EMA 平滑。非下载态每 tick 立即归零（零值速断，无衰减拖尾，全局同步扣除）；
    /// 下载态每秒采样一次窗口速率推入 EMA，数值每秒最多变化一次。
    /// （FR-01-81 修订二：连接级展示随明细表移除而撤销，仅任务级三展示面。）
    /// 含会话已下载累计（FR-01-81）与流量图历史采样。
    fn tick_speed_and_session(&mut self, now: Instant) {
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
        // 全局 ↓（architect v116：口径单源 App::global_dl_speed——与本帧前已将
        // 非下载态速度归零的合计数值恒等）
        let global_dl = self.global_dl_speed();
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
    }

    /// tick ⑥⑦ toast 过期 + 注册表周期保存（5s 兜底；关键状态转换即时保存；
    /// 底层原子写对内容未变化的空闲期短路跳过——零写放大）
    fn tick_housekeeping(&mut self, now: Instant) {
        if let Some(u) = self.toast_until {
            if now > u {
                self.toast = None;
                self.toast_until = None;
            }
        }
        if self.last_save.elapsed().as_secs() >= 5 {
            self.save_registry();
        }
    }

    /// tick ⑧⑨ 选中项 / 滚动钳制 + 并发明细选择 / 滚动钳制（FR-01-97：
    /// 切换选中任务后复位，选中行越界钳制到有效范围）
    fn tick_clamp_selection(&mut self) {
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
        if self.conns_scroll_for != self.selected {
            self.conns_scroll_for = self.selected;
            self.conns_scroll = 0;
            self.conns_sel = 0;
        }
        let clen = self
            .sel_task()
            .filter(|t| t.state.shows_conns())
            .map_or(0, |t| t.connections.len());
        if self.conns_sel >= clen.max(1) {
            self.conns_sel = clen.saturating_sub(1);
        }
        let cvis = self.visible_conns_rows.max(1);
        self.conns_scroll = self.conns_scroll.min(clen.saturating_sub(cvis));
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
}
