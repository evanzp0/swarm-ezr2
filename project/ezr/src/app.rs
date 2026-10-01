//! app.rs — EZR Downloader 正式版应用状态（真实下载内核接入）
//!
//! 与 demo 的差异：模拟器（xorshift 抖动/定时流转/伪造任务）整体移除，
//! 速度/大小/进度/失败原因/校验结果全部来自引擎事件（FR-01-81）。
//! 状态机口径（FR-01-30/31/32）：6 态 + 槽位调度按列表顺序；「校验中」占槽位
//! （D12）；失败重试连续性规则与指数退避（FR-01-41/42）；续传一致性失效防循环
//! （FR-01-22）；等待中任务不预取大小（获槽探测后才显示）。
//!
//! 交互基线沿用 demo 定稿：对话框（添加五字段/删除三选）、快捷键全集、
//! 鼠标（点击选中/滚轮/对话框按钮）、toast 反馈。
#![allow(missing_docs)] // 交互层：demo 定稿基线复用，接口文档见 model/engine 层
#![allow(clippy::pedantic)] // 交互层字节/速度展示算术与 demo 基线风格豁免
#![allow(clippy::nursery)] // 同上
#![allow(clippy::cognitive_complexity, clippy::too_many_lines, clippy::too_many_arguments)]
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_wrap)]


use std::collections::HashMap;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::engine::{Cmd, Evt, EngineHandle};
use crate::model::config::Config;
use crate::model::namegen;
use crate::model::registry::Registry;
use crate::model::sidecar::Sidecar;
use crate::model::speed::SpeedWindow;
use crate::model::{
    checksum, slots, Checksum, Protocol, fmt_created, unix_now,
};
pub use crate::model::{FailKind, Task, TaskState};

/// 每个任务在列表中占用的行高（3 行内容 + 1 行空行分隔）
pub const ITEM_HEIGHT: u16 = 4;

/// 列表页签（仅两个）
pub const FILTERS: [&str; 2] = ["正在下载", "已完成"];

/// braille 转轮字符（校验中/后期处理等动态效果）
const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// 校验算法表（添加对话框下拉选择）：（显示名，期望十六进制长度）
pub const CHECKSUM_ALGOS: [(&str, usize); 7] = [
    ("MD5", 32),
    ("SHA-1", 40),
    ("SHA-224", 56),
    ("SHA-256", 64),
    ("SHA-384", 96),
    ("SHA-512", 128),
    ("Adler-32", 8),
];

/// 对话框种类
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DialogKind {
    /// 添加任务
    Add,
    /// 删除任务
    Delete,
}

/// 对话框状态（结构沿用 demo 定稿）
pub struct Dialog {
    /// 对话框种类
    pub kind: DialogKind,
    /// Add: URL 输入缓冲
    pub url: String,
    /// Add: 保存目录输入缓冲（空 = 配置默认）
    pub dir: String,
    /// Add: 并发数输入缓冲（数字字符串）
    pub conns: String,
    /// Add: 用户是否手动修改过并发数
    pub conns_edited: bool,
    /// Add: 校验算法在 CHECKSUM_ALGOS 中的下标（默认 SHA-256 = 3）
    pub ck_type: usize,
    /// Add: 校验码输入缓冲（十六进制，可留空 = 不校验）
    pub ck_value: String,
    /// Add: 校验算法下拉框是否展开
    pub ck_open: bool,
    /// Add: 下拉框当前高亮项
    pub ck_sel: usize,
    /// Add: 0=URL 1=目录 2=并发 3=算法 4=校验码 5=确认 6=取消
    /// Delete: 0=仅删除任务 1=删除任务和文件 2=取消
    pub focus: usize,
    /// Delete 用：待删除任务名
    pub task_name: String,
}

/// 应用状态（UI 权威；引擎经事件驱动更新）
pub struct App {
    /// 任务列表（含已完成/已失败历史）
    pub tasks: Vec<Task>,
    /// 当前页签视图中的选中序号
    pub selected: usize,
    /// 列表滚动偏移（以任务条目为单位）
    pub scroll: usize,
    /// 页签下标（对应 FILTERS）
    pub filter: usize,
    /// 全局下载速度历史（KB/s 采样）
    pub speed_hist: Vec<u64>,
    /// 全局上传速度历史（KB/s 采样；01 恒 0，BT 二期启用）
    pub up_hist: Vec<u64>,
    /// 本次会话累计下载字节（FR-01-81 头部统计）
    pub session_bytes: u64,
    /// 帧计数（转轮动画）
    pub frame: u64,
    /// 退出标志
    pub quit: bool,
    /// toast 文案
    pub toast: Option<String>,
    toast_until: Option<Instant>,
    /// 下一个任务 ID
    pub next_id: u32,
    /// 下载槽位上限（配置 `download_slots`，默认 5，D13）
    pub max_slots: usize,
    /// 引擎句柄
    engine: EngineHandle,
    /// 引擎事件接收端
    evt_rx: tokio::sync::mpsc::Receiver<Evt>,
    /// 每任务速度滑窗（1s，FR-01-17）
    windows: HashMap<u32, SpeedWindow>,
    /// 配置
    pub cfg: Config,
    /// 注册表路径
    registry_path: String,
    /// 上次注册表保存时刻
    last_save: Instant,
    /// 上次 tick 时刻
    last_tick: Instant,
    /// 列表可视条目数（ui 层回填）
    pub visible_rows: usize,
    /// 列表内框区域（ui 层回填，鼠标命中用）
    pub list_area: Option<Rect>,
    /// 是否显示右侧速度图表面板
    pub show_chart: bool,
    /// 当前打开的对话框
    pub dialog: Option<Dialog>,
    /// 对话框按钮可点击区域（ui 层每帧回填）
    pub dlg_btn_rects: Vec<(Rect, usize)>,
    /// 对话框输入/选择行可点击区域（ui 层每帧回填）
    pub dlg_field_rects: Vec<(Rect, usize)>,
    /// 校验算法下拉框选项可点击区域（ui 层每帧回填）
    pub dlg_ck_rects: Vec<(Rect, usize)>,
    /// 待延迟文件删除（引擎 Cancelled 确认后执行）：id → (保存目录, 文件名)
    pending_deletes: HashMap<u32, (String, String)>,
}

impl App {
    /// 构建应用：加载配置与注册表（崩溃恢复），接入引擎。
    #[must_use]
    pub fn new(cfg: Config, registry_path: String) -> App {
        let (evt_tx, evt_rx) = tokio::sync::mpsc::channel::<Evt>(256);
        let engine = EngineHandle::start(&cfg, evt_tx);
        // 崩溃恢复：注册表 → 任务列表；逐任务合并 sidecar 断点（FR-01-23）
        let (tasks, next_id) = Self::restore(&registry_path);
        App {
            tasks,
            selected: 0,
            scroll: 0,
            filter: 0,
            speed_hist: vec![0; 90],
            up_hist: vec![0; 90],
            session_bytes: 0,
            frame: 0,
            quit: false,
            toast: Some("EZR Downloader 就绪".to_string()),
            toast_until: Some(Instant::now() + std::time::Duration::from_secs(3)),
            next_id,
            max_slots: cfg.download_slots,
            engine,
            evt_rx,
            windows: HashMap::new(),
            cfg,
            registry_path,
            last_save: Instant::now(),
            last_tick: Instant::now(),
            visible_rows: 6,
            list_area: None,
            show_chart: true,
            dialog: None,
            dlg_btn_rects: Vec::new(),
            dlg_field_rects: Vec::new(),
            dlg_ck_rects: Vec::new(),
            pending_deletes: HashMap::new(),
        }
    }

    /// 从注册表与 sidecar 恢复任务（FR-01-23：下载中→等待中排队、断点合并）
    fn restore(path: &str) -> (Vec<Task>, u32) {
        let Some(reg) = Registry::load(path) else {
            return (Vec::new(), 1);
        };
        let mut tasks = reg.restore_tasks();
        for t in &mut tasks {
            // sidecar 断点合并：有 sidecar 的任务恢复已下载字节与块视图
            let sc = Sidecar::load(&t.sidecar_path());
            if let Some(sc) = sc {
                t.downloaded = t.downloaded.max(sc.downloaded);
                t.resumable = !sc.non_resumable;
            }
        }
        (tasks, reg.next_id)
    }

    /// braille 转轮字符
    #[must_use]
    pub fn spinner(&self) -> char {
        SPINNER[(self.frame as usize) % SPINNER.len()]
    }

    /// 当前占用下载槽位的任务数（FR-01-31：含校验中）
    #[must_use]
    pub fn used_slots(&self) -> usize {
        slots::used(&self.tasks)
    }

    /// 等待任务在队列中的位次（1 基；None = 未在排队）
    #[must_use]
    pub fn queue_pos_of(&self, task_id: u32) -> Option<usize> {
        slots::queue_pos(&self.tasks, task_id)
    }

    /// 过滤后的任务下标视图（页签 0 = 正在下载，1 = 已完成）
    #[must_use]
    pub fn filtered(&self) -> Vec<usize> {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| match self.filter {
                1 => t.state.is_done(),
                _ => !t.state.is_done(),
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// 当前选中项在任务表中的下标
    #[must_use]
    pub fn sel_idx(&self) -> Option<usize> {
        self.filtered().get(self.selected).copied()
    }

    /// 当前选中任务（引用）
    #[must_use]
    pub fn sel_task(&self) -> Option<&Task> {
        self.sel_idx().and_then(|i| self.tasks.get(i))
    }

    pub(crate) fn set_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some(msg.into());
        self.toast_until = Some(Instant::now() + std::time::Duration::from_secs(3));
    }

    /// 全局速度历史采样（UI Sparkline 用，最大 180 点）
    fn push_hist(&mut self, global_dl: f64, global_ul: f64) {
        self.speed_hist.push((global_dl / 1024.0) as u64);
        if self.speed_hist.len() > 180 {
            self.speed_hist.remove(0);
        }
        self.up_hist.push((global_ul / 1024.0) as u64);
        if self.up_hist.len() > 180 {
            self.up_hist.remove(0);
        }
    }

    /// 校验期望解析（FR-01-50/D3）：显式提供优先；否则查保存目录伴随文件
    /// `<目标文件>.<算法后缀>`（算法表顺序取先，位数不符视为无效并提示）。
    fn resolve_checksum(
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
    fn make_spec(t: &Task) -> crate::engine::supervisor::TaskSpec {
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
    fn disk_precheck(&mut self, idx: usize) -> bool {
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
        let dt = now.duration_since(self.last_tick).as_secs_f64().clamp(0.001, 0.5);
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
            let Some(idx) = self.tasks.iter().position(|t| t.id == id) else { continue };
            if !self.disk_precheck(idx) {
                continue;
            }
            self.tasks[idx].probed = false;
            let spec = Self::make_spec(&self.tasks[idx]);
            self.engine.send(Cmd::Start { spec }).await;
        }

        // 4) 失败倒计时推进（自动重试：到点 → 等待中重排，槽位保持占用）
        for t in &mut self.tasks {
            if t.state == TaskState::Failed {
                if let Some(left) = t.retry_in.as_mut() {
                    *left -= dt;
                    if *left <= 0.0 {
                        t.retry_in = None;
                        t.state = TaskState::Queued;
                        t.error = None;
                    }
                }
            }
        }
        // 4b) 已获槽位的等待任务重排后需要（重新）Start——Failed→Queued 的重试
        //     由上面 retry_in 到点转 Queued（has_slot 保持）；本帧统一补发 Start。
        let retry_starts: Vec<u32> = self
            .tasks
            .iter()
            .filter(|t| t.state == TaskState::Queued && t.has_slot && t.probed)
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

        // 5) 速度窗口 → 任务速度 + 全局统计
        let mut global_dl = 0.0f64;
        for (id, w) in self.windows.iter_mut() {
            let r = w.rate();
            if let Some(t) = self.tasks.iter_mut().find(|t| t.id == *id) {
                t.speed = r;
            }
            global_dl += r;
        }
        self.session_bytes += (global_dl * dt) as u64;
        self.push_hist(global_dl, 0.0);

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
            Evt::Probed { id, name, final_url, total, resumable, etag, last_modified, cd_name: _ } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                    if t.name != name && t.downloaded == 0 {
                        t.name = name;
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
            Evt::Progress { id, downloaded, conns, chunk_done } => {
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
                    // 已删除/未知任务的幽灵进度：不建滑窗、不聚合（删除后引擎停止确认前的兜底）
                    self.windows.remove(&id);
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
                if downloaded > 0 {
                    t.made_progress = true;
                }
            }
            Evt::PausedDone { id, downloaded, chunk_done } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                    t.downloaded = downloaded.max(t.downloaded);
                    t.chunk_done = chunk_done;
                    t.speed = 0.0;
                    t.has_slot = false;
                    // 断点视图快照（详情页分块表）
                    let (conns, x) = crate::model::chunk::lease_snapshot(
                        t.total,
                        downloaded,
                        t.concurrency,
                        t.block_size,
                    );
                    t.connections = conns;
                    t.chunk_done = x.max(chunk_done);
                }
                self.windows.remove(&id);
            }
            Evt::Failed { id, kind, reason, retry_after, made_progress, downloaded, blocks, chunk_done } => {
                self.handle_failure(
                    id, kind, reason, retry_after, made_progress, downloaded, blocks, chunk_done,
                );
            }
            Evt::Invalidated { id } => {
                // 续传一致性失效（FR-01-22）：断点作废、从头重下、不计失败；
                // 连续 3 次转停等失败（防循环）
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else { return };
                // 防循环计数（FR-01-22）：连续 3 次失效转停等失败
                let (streak, stop_wait) = crate::model::retry::invalidate_streak(t.invalidation_streak);
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
                    self.set_toast(format!(
                        "⚠ 服务器内容已更新，断点已作废，从头重新下载: {name}"
                    ));
                }
            }
            Evt::DownloadDone { id, total, has_checksum } => {
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else { return };
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
                    let expected = t.checksum.as_ref().map_or(String::new(), |c| c.value.clone());
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
            Evt::VerifyDone { id, ok, computed, expected } => {
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else { return };
                let algo = t.checksum.as_ref().map_or("SHA-256", |c| c.algo);
                let name = t.name.clone();
                let _ = expected;
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
        blocks: Vec<u64>,
        chunk_done: u32,
    ) {
        let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else { return };
        if downloaded > 0 {
            t.downloaded = downloaded.max(t.downloaded);
        }
        t.chunk_done = chunk_done;
        t.speed = 0.0;
        // 重试决策（FR-01-41/42/43）：连续性规则 + 退避/Retry-After + 分类，
        // 策略唯一来源在 model::retry（DRY）
        let (auto_retry, max_retries) = (self.cfg.auto_retry, t.max_retries);
        let (new_retries, decision) = crate::model::retry::decide(
            kind, made_progress, t.retries, max_retries, retry_after, auto_retry,
        );
        t.retries = new_retries;
        t.made_progress = false;
        t.state = TaskState::Failed;
        t.fail_kind = Some(kind);
        t.error = Some(reason.clone());
        let name = t.name.clone();
        let (retries, max_retries) = (t.retries, t.max_retries);
        let toast = match decision {
            crate::model::retry::RetryDecision { auto: true, delay_secs, .. } => {
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
        let total = t.total;
        let conc = t.concurrency;
        let piece = t.block_size;
        let dl = t.downloaded;
        let (conns, x) = crate::model::chunk::lease_snapshot(total, dl, conc, piece);
        t.connections = conns;
        t.chunk_done = x.max(chunk_done);
        // t（可变借用）在此作用域结束后自然释放
        self.windows.remove(&id);
        self.set_toast(toast);
        let _ = blocks;
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

    /// 键盘事件入口
    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) {
            if code == KeyCode::Char('c') {
                self.quit = true;
            }
            return;
        }
        if self.dialog.is_some() {
            self.on_dialog_key(code);
            return;
        }
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Down => self.move_sel(1),
            KeyCode::Up => self.move_sel(-1),
            KeyCode::PageDown => self.move_sel(4),
            KeyCode::PageUp => self.move_sel(-4),
            KeyCode::Home => self.selected = 0,
            KeyCode::End => {
                let flen = self.filtered().len();
                if flen > 0 {
                    self.selected = flen - 1;
                }
            }
            KeyCode::Tab => self.filter = (self.filter + 1) % FILTERS.len(),
            KeyCode::BackTab => self.filter = (self.filter + FILTERS.len() - 1) % FILTERS.len(),
            KeyCode::Char(' ') => self.toggle_pause(),
            KeyCode::Char('r') | KeyCode::Char('R') => self.retry(),
            KeyCode::Char('a') | KeyCode::Char('A') => self.open_add_dialog(),
            KeyCode::Char('d') | KeyCode::Char('D') => self.open_delete_dialog(),
            KeyCode::Char('c') | KeyCode::Char('C') => self.clear_completed(),
            KeyCode::Char('g') | KeyCode::Char('G') => self.show_chart = !self.show_chart,
            KeyCode::Char('u') | KeyCode::Char('U') => self.move_task(-1),
            KeyCode::Char('j') | KeyCode::Char('J') => self.move_task(1),
            _ => {}
        }
    }

    fn move_sel(&mut self, delta: i32) {
        let flen = self.filtered().len();
        if flen == 0 {
            return;
        }
        let cur = self.selected as i32;
        self.selected = cur.saturating_add(delta).clamp(0, flen as i32 - 1) as usize;
    }

    /// 上移/下移选中任务（调整排队优先级，FR-01-32）
    pub fn move_task(&mut self, delta: i32) {
        let fl = self.filtered();
        if fl.is_empty() {
            return;
        }
        let new = self.selected as i32 + delta;
        if new < 0 {
            self.set_toast("已在顶部");
            return;
        }
        if new >= fl.len() as i32 {
            self.set_toast("已在底部");
            return;
        }
        let new = new as usize;
        let (a, b) = (fl[self.selected], fl[new]);
        if a != b {
            self.tasks.swap(a, b);
        }
        self.selected = new;
        let name = self.tasks[fl[new]].name.clone();
        self.set_toast(format!(
            "{} 任务: {}",
            if delta < 0 { "↑ 已上移" } else { "↓ 已下移" },
            name
        ));
    }

    // -----------------------------------------------------------------------
    // 对话框（结构沿用 demo）
    // -----------------------------------------------------------------------

    fn on_dialog_key(&mut self, code: KeyCode) {
        let (kind, focus) = match &self.dialog {
            Some(d) => (d.kind, d.focus),
            None => return,
        };
        let nfocus = match kind {
            DialogKind::Add => 7,
            DialogKind::Delete => 3,
        };
        // 校验算法下拉框展开时（仅 Add）
        if kind == DialogKind::Add && self.dialog.as_ref().is_some_and(|d| d.ck_open) {
            match code {
                KeyCode::Up => {
                    let d = self.dialog.as_mut().unwrap();
                    d.ck_sel = (d.ck_sel + CHECKSUM_ALGOS.len() - 1) % CHECKSUM_ALGOS.len();
                    d.ck_type = d.ck_sel;
                }
                KeyCode::Down => {
                    let d = self.dialog.as_mut().unwrap();
                    d.ck_sel = (d.ck_sel + 1) % CHECKSUM_ALGOS.len();
                    d.ck_type = d.ck_sel;
                }
                KeyCode::Home => {
                    let d = self.dialog.as_mut().unwrap();
                    d.ck_sel = 0;
                    d.ck_type = 0;
                }
                KeyCode::End => {
                    let d = self.dialog.as_mut().unwrap();
                    d.ck_sel = CHECKSUM_ALGOS.len() - 1;
                    d.ck_type = d.ck_sel;
                }
                KeyCode::Enter | KeyCode::Esc => {
                    if let Some(d) = self.dialog.as_mut() {
                        d.ck_open = false;
                    }
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    let d = self.dialog.as_mut().unwrap();
                    d.ck_open = false;
                    d.focus = if code == KeyCode::Tab { 4 } else { 2 };
                }
                _ => {
                    if let Some(d) = self.dialog.as_mut() {
                        d.ck_open = false;
                    }
                }
            }
            return;
        }
        match code {
            KeyCode::Esc => self.dialog = None,
            KeyCode::Tab | KeyCode::Down | KeyCode::Right => {
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = (d.focus + 1) % nfocus;
                }
            }
            KeyCode::BackTab | KeyCode::Up | KeyCode::Left => {
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = (d.focus + nfocus - 1) % nfocus;
                }
            }
            KeyCode::Enter => match kind {
                DialogKind::Add => {
                    if focus == 3 {
                        let d = self.dialog.as_mut().unwrap();
                        d.ck_open = true;
                        d.ck_sel = d.ck_type;
                    } else if focus == 6 {
                        self.dialog = None;
                    } else {
                        self.dlg_confirm_add();
                    }
                }
                DialogKind::Delete => self.dlg_activate_delete(focus),
            },
            KeyCode::Backspace => {
                if kind == DialogKind::Add && (focus < 3 || focus == 4) {
                    let d = self.dialog.as_mut().unwrap();
                    match focus {
                        0 => {
                            d.url.pop();
                        }
                        1 => {
                            d.dir.pop();
                        }
                        2 => {
                            d.conns.pop();
                            d.conns_edited = true;
                        }
                        _ => {
                            d.ck_value.pop();
                        }
                    }
                }
            }
            KeyCode::Char(c) => match kind {
                DialogKind::Add => {
                    if focus < 3 {
                        if !c.is_control() {
                            let d = self.dialog.as_mut().unwrap();
                            match focus {
                                0 => {
                                    if d.url.chars().count() < 300 {
                                        d.url.push(c);
                                    }
                                }
                                1 => {
                                    if d.dir.chars().count() < 300 {
                                        d.dir.push(c);
                                    }
                                }
                                _ => {
                                    if c.is_ascii_digit() && d.conns.chars().count() < 2 {
                                        d.conns.push(c);
                                        d.conns_edited = true;
                                    }
                                }
                            }
                        }
                    } else if focus == 3 {
                        if c == ' ' {
                            let d = self.dialog.as_mut().unwrap();
                            d.ck_open = true;
                            d.ck_sel = d.ck_type;
                        }
                    } else if focus == 4 {
                        // 校验码：仅十六进制，最多 128 位（SHA-512）
                        if c.is_ascii_hexdigit()
                            && self.dialog.as_ref().unwrap().ck_value.chars().count() < 128
                        {
                            self.dialog.as_mut().unwrap().ck_value.push(c);
                        }
                    } else if c == ' ' {
                        self.dlg_activate_add(focus);
                    }
                }
                DialogKind::Delete => match c {
                    '1' => self.dlg_activate_delete(0),
                    '2' => self.dlg_activate_delete(1),
                    '3' | ' ' => self.dlg_activate_delete(2),
                    _ => {}
                },
            },
            _ => {}
        }
    }

    /// bracketed paste 事件入口（FR-01-06）：仅 Add 对话框的文本字段接收粘贴；
    /// 下拉框展开、删除对话框、无对话框时忽略（避免粘贴触发按钮/导航）。
    pub fn on_paste(&mut self, text: &str) {
        let Some(d) = self.dialog.as_mut() else { return };
        if !paste_accepts(d.kind, d.ck_open) {
            return;
        }
        let focus = d.focus;
        dlg_apply_paste(d, focus, text);
    }

    fn dlg_activate_add(&mut self, btn: usize) {
        match btn {
            5 => self.dlg_confirm_add(),
            6 => self.dialog = None,
            _ => {}
        }
    }

    /// 确认添加（FR-01-01/03/04/05/26：URL 校验、目录默认、并发钳制、校验码校验、断点接续）
    fn dlg_confirm_add(&mut self) {
        let Some(d) = self.dialog.as_ref() else { return };
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
                Ok(v) => Some(Checksum { algo: CHECKSUM_ALGOS[ck_type].0, value: v }),
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

        // 保存目录（FR-01-03）：留空 = 配置 download_dir；缺省 = ~/Downloads
        let dir = if dir_raw.is_empty() {
            self.cfg
                .download_dir
                .clone()
                .unwrap_or_else(crate::model::config::default_download_dir)
        } else {
            dir_raw.trim_end_matches('/').to_string()
        };

        let protocol = if url.starts_with("https://") { Protocol::Https } else { Protocol::Http };
        let base_name = namegen::derive_name(None, None, &url);
        let id = self.next_id;
        // FR-01-26 断点自动接续：既有 sidecar 同 URL 同路径 → 接续原名
        let dir_trim = dir.trim_end_matches('/').to_string();
        let resumed = Sidecar::load(&format!("{dir_trim}/{base_name}.ezr"))
            .is_some_and(|sc| sc.url == url)
            || self.tasks.iter().any(|t| {
                t.url == url
                    && t.save_dir == dir_trim
                    && t.name == base_name
                    && t.state != TaskState::Completed
            });
        let name = if resumed {
            base_name.clone()
        } else {
            // 重名检测：任务表 + 盘上（文件/.downloading/sidecar）→ 自动追加序号
            let tasks = &self.tasks;
            let dir2 = dir_trim.clone();
            namegen::dedupe(&base_name, |n| {
                tasks.iter().any(|t| t.name == n && t.save_dir == dir2)
                    || namegen::exists_on_disk(&dir2, n)
            })
        };
        // 并发数（FR-01-04）：钳制 1–64；留空/非法 → 配置默认
        let conns = conns_raw
            .parse::<usize>()
            .map(|v| v.clamp(1, 64))
            .unwrap_or(self.cfg.default_concurrency);
        self.next_id += 1;
        // 校验值来源②（FR-01-50/D14）：未显式提供时查保存目录伴随文件
        let checksum = Self::resolve_checksum(&dir_trim, &name, checksum);
        let ts = unix_now();
        let t = Task {
            id,
            name: name.clone(),
            protocol,
            url: url.clone(),
            final_url: None,
            save_dir: dir.trim_end_matches('/').to_string(),
            total: 0,
            downloaded: 0,
            speed: 0.0,
            state: TaskState::Queued,
            resumable: true,
            probed: false,
            connections: vec![],
            chunk_done: 0,
            block_size: self.cfg.block_size_http,
            concurrency: conns,
            retries: 0,
            max_retries: self.cfg.max_retries,
            made_progress: false,
            retry_in: None,
            fail_kind: None,
            error: None,
            invalidation_streak: 0,
            checksum,
            verify_ok: None,
            etag: None,
            last_modified: None,
            has_slot: false,
            upload_speed: 0.0,
            uploaded: 0,
            seeders: 0,
            peers: 0,
            seed_left: 0.0,
            elapsed: 0.0,
            created: fmt_created(ts),
            added_at: ts,
        };
        self.tasks.push(t);
        self.dialog = None;
        self.filter = 0;
        let flen = self.filtered().len();
        if flen > 0 {
            self.selected = flen - 1;
        }
        if resumed {
            self.set_toast(format!("✓ 已添加任务 #{id}: {name}（发现有效断点，将自动接续）"));
        } else {
            self.set_toast(format!("✓ 已添加任务 #{id}: {name}"));
        }
        self.save_registry();
    }

    fn dlg_activate_delete(&mut self, btn: usize) {
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
    fn remove_task(&mut self, idx: usize, id: u32, delete_files: bool) {
        let (save_dir, name) = {
            let t = &self.tasks[idx];
            (t.save_dir.clone(), t.name.clone())
        };
        self.tasks.remove(idx);
        self.windows.remove(&id);
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
    fn delete_task_files(save_dir: &str, name: &str) {
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

    /// 鼠标事件入口
    pub fn on_mouse(&mut self, m: MouseEvent) {
        if self.dialog.is_some() {
            if let MouseEventKind::Down(MouseButton::Left) = m.kind {
                let kind = self.dialog.as_ref().map(|d| d.kind);
                let ck_open = self
                    .dialog
                    .as_ref()
                    .is_some_and(|d| d.kind == DialogKind::Add && d.ck_open);
                let hit = |rects: &[(Rect, usize)]| {
                    rects
                        .iter()
                        .find(|(r, _)| {
                            m.column >= r.x
                                && m.column < r.x.saturating_add(r.width)
                                && m.row >= r.y
                                && m.row < r.y.saturating_add(r.height)
                        })
                        .map(|(_, i)| *i)
                };
                if ck_open {
                    let ck_rects = self.dlg_ck_rects.clone();
                    if let Some(i) = hit(&ck_rects) {
                        let d = self.dialog.as_mut().unwrap();
                        d.ck_type = i;
                        d.ck_sel = i;
                        d.ck_open = false;
                    } else if let Some(d) = self.dialog.as_mut() {
                        d.ck_open = false;
                    }
                    return;
                }
                let field_rects = self.dlg_field_rects.clone();
                let btn_rects = self.dlg_btn_rects.clone();
                if let Some(i) = hit(&field_rects) {
                    let d = self.dialog.as_mut().unwrap();
                    d.focus = i;
                    if kind == Some(DialogKind::Add) && i == 3 {
                        d.ck_open = true;
                        d.ck_sel = d.ck_type;
                    }
                } else if let Some(btn) = hit(&btn_rects) {
                    if let Some(d) = self.dialog.as_mut() {
                        d.focus = btn;
                    }
                    match kind {
                        Some(DialogKind::Add) => self.dlg_activate_add(btn),
                        Some(DialogKind::Delete) => self.dlg_activate_delete(btn),
                        None => {}
                    }
                }
            }
            return;
        }
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(a) = self.list_area {
                    let in_x = m.column >= a.x && m.column < a.x.saturating_add(a.width);
                    let in_y = m.row >= a.y && m.row < a.y.saturating_add(a.height);
                    if in_x && in_y {
                        let idx = (m.row - a.y) as usize / ITEM_HEIGHT as usize;
                        let ti = self.scroll + idx;
                        if ti < self.filtered().len() {
                            self.selected = ti;
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                self.scroll = self.scroll.saturating_sub(2);
            }
            MouseEventKind::ScrollDown => {
                let vis = self.visible_rows.max(1);
                let maxs = self.filtered().len().saturating_sub(vis);
                self.scroll = (self.scroll + 2).min(maxs);
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // 任务操作（Space/R/D/C 语义沿用 demo + 真实引擎动作）
    // -----------------------------------------------------------------------

    /// Space：暂停/继续（FR-01-33）
    pub fn toggle_pause(&mut self) {
        let Some(idx) = self.sel_idx() else { return };
        let name = self.tasks[idx].name.clone();
        match self.tasks[idx].state {
            TaskState::Downloading | TaskState::Verifying if self.tasks[idx].state == TaskState::Downloading => {
                // 下载中 → 暂停（引擎停传写 sidecar；UI 即时转已暂停）
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
                    self.set_toast(format!("⏳ 无空闲下载槽位（{used}/{n}），已进入等待队列: {name}"));
                    return;
                }
                // 直接继续（探测已做过；sidecar 断点由引擎加载）
                self.tasks[idx].state = TaskState::Downloading;
                self.tasks[idx].has_slot = true;
                self.tasks[idx].made_progress = false;
                let spec = Self::make_spec(&self.tasks[idx]);
                let id = self.tasks[idx].id;
                self.windows.remove(&id);
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
        self.tasks[idx].has_slot = false;
        if verify_fail {
            // 重新校验（D10）：块全满、无数据重传；期望值重查伴随文件——
            // 若失败源于期望值写错（AC-6），修正伴随文件后按 R 即可通过
            let (save_dir, name) =
                (self.tasks[idx].save_dir.clone(), self.tasks[idx].name.clone());
            self.tasks[idx].checksum =
                Self::resolve_checksum(&save_dir, &name, self.tasks[idx].checksum.clone());
            self.tasks[idx].state = TaskState::Verifying;
            self.tasks[idx].has_slot = true;
            let spec = crate::engine::supervisor::VerifySpec {
                id,
                path: self.tasks[idx].downloading_path(),
                final_path: self.tasks[idx].target_path(),
                sidecar_path: self.tasks[idx].sidecar_path(),
                algo: self.tasks[idx].checksum.as_ref().map_or("SHA-256", |c| c.algo),
                expected: self
                    .tasks[idx]
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
        let label = if manual { "手动重试" } else { "重新排队" };
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

// ---------------------------------------------------------------------------
// bracketed paste 输入路由（FR-01-06）：纯函数便于单测，过滤规则与逐键输入一致
// ---------------------------------------------------------------------------

/// 判定粘贴事件是否可作用于当前对话框状态（FR-01-06）
fn paste_accepts(kind: DialogKind, ck_open: bool) -> bool {
    kind == DialogKind::Add && !ck_open
}

/// 粘贴文本净化：剔除控制字符（剪贴板可能携带换行/制表等）
fn paste_sanitize(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

/// 截断式追加：目标字符串不超过 `cap` 个字符（按 char 计数，不切分多字节字符）
fn push_capped(dst: &mut String, src: &str, cap: usize) -> bool {
    let room = cap.saturating_sub(dst.chars().count());
    let taken: String = src.chars().take(room).collect();
    let inserted = !taken.is_empty();
    dst.push_str(&taken);
    inserted
}

/// 将粘贴文本按当前焦点路由到 Add 对话框字段（FR-01-06）。
/// 过滤规则与逐键输入一致：URL/目录 ≤300 字符；并发仅数字 ≤2 位；
/// 校验码仅十六进制 ≤128 位；按钮焦点（5=确认 6=取消）不接收。
fn dlg_apply_paste(d: &mut Dialog, focus: usize, text: &str) -> bool {
    let clean = paste_sanitize(text);
    match focus {
        0 => push_capped(&mut d.url, &clean, 300),
        1 => push_capped(&mut d.dir, &clean, 300),
        2 => {
            let digits: String = clean.chars().filter(|c| c.is_ascii_digit()).collect();
            let inserted = push_capped(&mut d.conns, &digits, 2);
            d.conns_edited |= inserted;
            inserted
        }
        4 => {
            let hex: String = clean.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            push_capped(&mut d.ck_value, &hex, 128)
        }
        _ => false,
    }
}

#[cfg(test)]
mod paste_tests {
    use super::*;

    fn add_dialog(focus: usize) -> Dialog {
        Dialog {
            kind: DialogKind::Add,
            url: String::new(),
            dir: String::new(),
            conns: String::new(),
            conns_edited: false,
            ck_type: 3,
            ck_value: String::new(),
            ck_open: false,
            ck_sel: 3,
            focus,
            task_name: String::new(),
        }
    }

    /// 01-add-task-17：粘贴 200 字符长 URL，字段完整接收
    #[test]
    fn paste_long_url_fully_inserted() {
        let mut d = add_dialog(0);
        let url = format!("http://example.com/{}", "a".repeat(180));
        assert_eq!(url.chars().count(), 199);
        assert!(dlg_apply_paste(&mut d, 0, &url));
        assert_eq!(d.url, url);
    }

    /// 粘贴携带换行/制表的剪贴板内容：控制字符被剔除
    #[test]
    fn paste_strips_control_chars() {
        let mut d = add_dialog(0);
        assert!(dlg_apply_paste(&mut d, 0, "http://example.com/f.bin\r\n"));
        assert_eq!(d.url, "http://example.com/f.bin");
    }

    /// URL 字段 300 字符上限与逐键输入一致
    #[test]
    fn paste_url_caps_at_300() {
        let mut d = add_dialog(0);
        let blob = "x".repeat(400);
        assert!(dlg_apply_paste(&mut d, 0, &blob));
        assert_eq!(d.url.chars().count(), 300);
    }

    /// 校验码字段：仅十六进制被接收，最多 128 位（SHA-512）
    #[test]
    fn paste_checksum_filters_hex_and_caps() {
        let mut d = add_dialog(4);
        assert!(dlg_apply_paste(&mut d, 4, "  zzFF88ff00!dead-beef  "));
        assert_eq!(d.ck_value, "FF88ff00deadbeef");
        let blob = "f".repeat(200);
        assert!(dlg_apply_paste(&mut d, 4, &blob));
        assert_eq!(d.ck_value.chars().count(), 128);
    }

    /// 并发数字段：仅数字，最多 2 位；edited 标记置位
    #[test]
    fn paste_conns_two_digits_only() {
        let mut d = add_dialog(2);
        assert!(dlg_apply_paste(&mut d, 2, "1a2b3"));
        assert_eq!(d.conns, "12");
        assert!(d.conns_edited);
        d.conns = "9".into();
        assert!(dlg_apply_paste(&mut d, 2, "42"));
        assert_eq!(d.conns, "94");
    }

    /// 并发字段满 2 位后再粘贴：无变化
    #[test]
    fn paste_conns_full_rejects() {
        let mut d = add_dialog(2);
        d.conns = "64".into();
        assert!(!dlg_apply_paste(&mut d, 2, "7"));
        assert_eq!(d.conns, "64");
    }

    /// 纯控制字符粘贴：无效果
    #[test]
    fn paste_all_control_is_noop() {
        let mut d = add_dialog(0);
        assert!(!dlg_apply_paste(&mut d, 0, "\r\n\t"));
        assert!(d.url.is_empty());
    }

    /// 按钮焦点（5=确认 6=取消）不接收粘贴
    #[test]
    fn paste_buttons_ignore() {
        let mut d = add_dialog(5);
        assert!(!dlg_apply_paste(&mut d, 5, "http://example.com/"));
        assert!(d.url.is_empty());
    }

    /// 粘贴仅作用于 Add 对话框文本字段：下拉展开或 Delete 对话框时忽略
    #[test]
    fn paste_accepts_only_add_text_fields() {
        assert!(paste_accepts(DialogKind::Add, false));
        assert!(!paste_accepts(DialogKind::Add, true));
        assert!(!paste_accepts(DialogKind::Delete, false));
    }

    /// 截断式追加按字符计数，不切分多字节字符
    #[test]
    fn push_capped_respects_char_boundary() {
        let mut s = String::from("下载");
        assert!(push_capped(&mut s, "器abc", 3));
        assert_eq!(s, "下载器");
        assert!(!push_capped(&mut s, "x", 3));
        assert_eq!(s, "下载器");
    }
}
