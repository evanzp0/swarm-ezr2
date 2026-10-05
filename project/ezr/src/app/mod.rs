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

//! 模块划分：`cli`（CLI 启动参数添加任务）/ `engine`（tick 事件消费/槽位调度/
//! 失败处理）/ `keys`（键盘输入）/ `mouse`（鼠标输入）/ `dialogs`（对话框确认流）/
//! `paste`（bracketed paste）/ `tasks`（暂停·重试·清理操作）。App 结构体与构造在此。

use std::collections::HashMap;
use std::time::Instant;

use ratatui::layout::Rect;

use crate::engine::{EngineHandle, Evt};
use crate::model::config::Config;
use crate::model::registry::Registry;
use crate::model::sidecar::Sidecar;
use crate::model::slots;
use crate::model::speed::{SmoothedSpeed, SpeedWindow};
pub use crate::model::Task;

mod cli;
mod dialog_keys;
mod dialogs;
mod engine;
mod keys;
mod mouse;
mod paste;
mod tasks;

/// 每个任务在列表中占用的行高（3 行内容 + 1 行空行分隔）
pub const ITEM_HEIGHT: u16 = 4;

/// 列表页签（仅两个）
pub const FILTERS: [&str; 2] = ["正在下载", "已完成"];

/// braille 转轮字符（校验中/后期处理等动态效果）
const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// 校验算法表（单源于 `model::checksum::CHECKSUM_ALGOS`：显示名，期望十六进制
/// 长度，伴随文件后缀）。架构评审（cleaner 批次后）收口：原 app 层另有一份
/// 2 元组同名表，双源存在漂移风险（对话框下拉与 CLI `-x`/伴随文件解析口径
/// 可能分岐），改统一 re-export，各消费点按需取字段。
pub use crate::model::checksum::CHECKSUM_ALGOS;

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
    /// 各任务上次观察到的已下载字节（会话累计账本，增量式计数用）
    session_seen: HashMap<u32, u64>,
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
    /// 展示面平滑值（FR-01-17 修订：1s 节拍采样 + EMA；生命周期与 windows 一致）
    speed_display: HashMap<u32, SmoothedSpeed>,
    /// 展示面上次采样时刻（数值每秒最多变化一次）
    last_speed_tick: Instant,
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
            session_seen: HashMap::new(),
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
            last_speed_tick: Instant::now(),
            speed_display: HashMap::new(),
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
    pub(super) fn push_hist(&mut self, global_dl: f64, global_ul: f64) {
        self.speed_hist.push((global_dl / 1024.0) as u64);
        if self.speed_hist.len() > 180 {
            self.speed_hist.remove(0);
        }
        self.up_hist.push((global_ul / 1024.0) as u64);
        if self.up_hist.len() > 180 {
            self.up_hist.remove(0);
        }
    }
}
