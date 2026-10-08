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
use crate::model::speed::{SmoothedSpeed, SpeedWindow};
use crate::model::{slots, Task};
// architect v116：原 `pub use crate::model::Task;` 死 re-export（crate::app::Task
// 零消费者，各层均直用 crate::model::Task）降为模块内私有导入

mod cli;
mod clipboard;
mod dialog_keys;
mod dialogs;
mod engine;
mod keys;
mod mouse;
mod paste;
mod tasks;
#[cfg(test)]
pub(crate) mod testutil;

// 测试可达性门面：property_tests 属性测试需触达 dialog_keys/routing 键路由
// 纯函数（对话框状态机不变量：字段容量上限、下拉选择同步与越界钳制）。
// 经 cfg(test) 门控 re-export（notes/rust.md「门面 re-export」手法）；测试
// 构建由 crate 根 property_tests 消费、产品构建不产生该路径，外部 API 面
// 零增量（挂载测试 crate 的 unused 属机制固有豁免，三分法 ①）。
#[cfg(test)]
pub(crate) use dialog_keys::{
    dropdown_open_key, proxy_dropdown_open_key, push_hex_capped, text_backspace, text_char,
};

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
    /// 修改任务（v1.5/FR-01-87：并发/校验算法/校验码/代理，确定立即生效）
    Modify,
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
    /// Add/Modify: 代理下拉在 [`App::proxy_options`] 中的下标（v1.5/FR-01-86）
    pub proxy_sel: usize,
    /// Add/Modify: 代理下拉框是否展开
    pub proxy_open: bool,
    /// Add: 0=URL 1=目录 2=并发 3=算法 4=校验码 5=代理 6=确认 7=取消
    /// Modify: 0=并发 1=算法 2=校验码 3=代理 4=确认 5=取消
    /// Delete: 0=仅删除任务 1=删除任务和文件 2=取消
    pub focus: usize,
    /// Delete 用：待删除任务名
    pub task_name: String,
    /// Modify 用：待修改任务 id（None = Add/Delete）
    pub task_id: Option<u32>,
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
    /// 连接级账本（FR-01-99，App 消费侧计量不回写引擎）：
    /// (任务, 连接) → 上一观测 (块号, 块内已写字节, 时刻)——增量与换块判定依据
    conn_prev: HashMap<(u32, usize), (u32, u64, Instant)>,
    /// (任务, 连接) → 本次开始下载起累计落盘字节（新一次下载清零、续传保留）
    conn_cum: HashMap<(u32, usize), u64>,
    /// (任务, 连接) → 连接速度展示面平滑器（EMA；待命零值速断）
    conn_speed: HashMap<(u32, usize), SmoothedSpeed>,
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
    /// 代理选项表（v1.5/FR-01-86；v1.6/D18 两态：直连 + 命名条目）；
    /// 添加/修改对话框下拉的选项源与 proxy_sel 下标依据
    pub proxy_options: Vec<crate::model::ProxyChoice>,
    /// 代理选项显示名（与 proxy_options 一一对应；命名条目含类型标注，不含认证信息）
    pub proxy_labels: Vec<String>,
    /// 添加对话框代理默认选中下标（v1.6：恒直连 = 0）
    pub default_proxy_sel: usize,
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
    /// 是否显示右栏两面板（任务详情 / 并发连接面板；`G` 切换，FR-01-98；
    /// 头部流量图常显不受影响）
    pub show_panes: bool,
    /// 并发连接面板滚动偏移（连接行单位；滚轮 / Ctrl+↑↓ 滚动，FR-01-96）
    pub conns_scroll: usize,
    /// 并发连接面板当前选中行（连接下标 0 基；Ctrl+↑↓ 移动，Ctrl+B 断开目标，FR-01-97）
    pub conns_sel: usize,
    /// 并发连接面板内框区域（ui 层回填；滚轮命中；空明细时 None = 穿透）
    pub conns_area: Option<Rect>,
    /// 任务详情面板「URL」字段名热区（ui 层每帧回填；点击复制 url 展示值，FR-01-101）
    pub detail_url_rect: Option<Rect>,
    /// 任务详情面板「校验」字段名热区（ui 层每帧回填；点击复制校验码值，FR-01-101）
    pub detail_ck_rect: Option<Rect>,
    /// 系统剪贴板实例（FR-01-101：懒初始化并复用，避免每次点击重建桌面连接）
    clipboard: Option<arboard::Clipboard>,
    /// 最近一次复制的内容（FR-01-101 诊断与测试断言复制内容用）
    pub(crate) last_copied: Option<String>,
    /// 并发连接面板可视数据行数（ui 层回填，滚动钳制）
    pub visible_conns_rows: usize,
    /// 明细滚动所属的列表选中序号（切换任务时重置滚动与选择，FR-01-97）
    conns_scroll_for: usize,
    /// 当前打开的对话框
    pub dialog: Option<Dialog>,
    /// 对话框按钮可点击区域（ui 层每帧回填）
    pub dlg_btn_rects: Vec<(Rect, usize)>,
    /// 对话框输入/选择行可点击区域（ui 层每帧回填）
    pub dlg_field_rects: Vec<(Rect, usize)>,
    /// 校验算法下拉框选项可点击区域（ui 层每帧回填）
    pub dlg_ck_rects: Vec<(Rect, usize)>,
    /// 代理下拉选项命中区域（v1.5/FR-01-86；鼠标点击选择）
    pub dlg_proxy_rects: Vec<(Rect, usize)>,
    /// 待延迟文件删除（引擎 Cancelled 确认后执行）：id → (保存目录, 文件名)
    pending_deletes: HashMap<u32, (String, String)>,
}

impl App {
    /// 构建应用：加载配置与注册表（崩溃恢复），接入引擎。
    #[must_use]
    pub fn new(cfg: Config, registry_path: String) -> App {
        let (evt_tx, evt_rx) = tokio::sync::mpsc::channel::<Evt>(256);
        let engine = EngineHandle::start(&cfg, evt_tx);
        // 代理选项表（v1.5/FR-01-86；v1.6/FR-01-90 两态）：直连恒在 +
        // 按配置顺序列出全部命名条目（显示名带类型标注）；默认选中 = 直连
        let mut proxy_options = vec![crate::model::ProxyChoice::Direct];
        let mut proxy_labels = vec!["直连".to_string()];
        for p in &cfg.proxies {
            proxy_options.push(crate::model::ProxyChoice::Named(p.name.clone()));
            proxy_labels.push(format!("{}（{}）", p.name, p.kind.label()));
        }
        let default_proxy_sel = 0;
        // 崩溃恢复：注册表 → 任务列表；逐任务合并 sidecar 断点（FR-01-23）
        let (tasks, next_id) = Self::restore(&registry_path);
        let mut app = App {
            tasks,
            selected: 0,
            scroll: 0,
            filter: 0,
            speed_hist: vec![0; 90],
            up_hist: vec![0; 90],
            session_bytes: 0,
            session_seen: HashMap::new(),
            conn_prev: HashMap::new(),
            conn_cum: HashMap::new(),
            conn_speed: HashMap::new(),
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
            show_panes: true,
            conns_scroll: 0,
            conns_sel: 0,
            conns_area: None,
            detail_url_rect: None,
            detail_ck_rect: None,
            clipboard: None,
            last_copied: None,
            visible_conns_rows: 8,
            conns_scroll_for: 0,
            dialog: None,
            dlg_btn_rects: Vec::new(),
            dlg_field_rects: Vec::new(),
            dlg_ck_rects: Vec::new(),
            dlg_proxy_rects: Vec::new(),
            proxy_options,
            proxy_labels,
            default_proxy_sel,
            pending_deletes: HashMap::new(),
        };
        // 配置加载警告（v1.5/FR-01-86：非法代理条目）→ 启动 toast 提醒一次；
        // 多条合并为一条（toast 槽位单一，逐条会被后发覆盖）
        if !app.cfg.warnings.is_empty() {
            let msg = app.cfg.warnings.join("；");
            app.set_toast(format!("⚠ {msg}"));
        }
        app
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

    /// 全局速度历史采样（UI 流量图用，最大 180 点）
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

    /// 连接级展示面：指定连接的平滑速度（B/s；无记录 = 0 待命）
    pub(crate) fn conn_speed_of(&self, task_id: u32, conn_id: usize) -> f64 {
        self.conn_speed
            .get(&(task_id, conn_id))
            .map_or(0.0, SmoothedSpeed::value)
    }

    /// 连接级展示面：指定连接自本次开始下载起累计落盘字节（无记录 = 0）
    pub(crate) fn conn_cum_of(&self, task_id: u32, conn_id: usize) -> u64 {
        self.conn_cum.get(&(task_id, conn_id)).copied().unwrap_or(0)
    }

    /// 全局下载速度合计（B/s；头部仪表面板「全局 ↓」与 tick 流量图采样共用
    /// 口径：仅合计「下载中」任务速度。architect v116 自渲染路径提取——
    /// 原头部内联聚合无直接单测面；tick 侧原合计全任务（非下载态速度已被
    /// 同帧归零，数值恒等）。
    #[must_use]
    pub fn global_dl_speed(&self) -> f64 {
        self.tasks
            .iter()
            .filter(|t| t.state == crate::model::TaskState::Downloading)
            .map(|t| t.speed)
            .sum()
    }

    /// 全局上传速度合计（B/s；头部「全局 ↑」口径：仅合计有上传的任务；
    /// 01 期 BT 未启用恒 0，FR-01-94 ③）
    #[must_use]
    pub fn global_ul_speed(&self) -> f64 {
        self.tasks
            .iter()
            .filter(|t| t.upload_speed > 0.0)
            .map(|t| t.upload_speed)
            .sum()
    }

    /// 头部「并发 N」口径（FR-01-94 ②/FR-01-96 数据面）：显示并发明细的任务
    /// 连接总数（architect v116 自渲染路径提取）
    #[must_use]
    pub fn conns_display_total(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.state.shows_conns())
            .map(|t| t.connections.len())
            .sum()
    }

    /// 并发面板「活跃 x」计数（FR-01-96）：明细状态下速度 > 0 的连接数；
    /// 非明细状态恒 0（architect v116 自渲染路径提取，无头单测面）
    #[must_use]
    pub fn active_conn_count(&self, t: &Task) -> usize {
        if !t.state.shows_conns() {
            return 0;
        }
        t.connections
            .iter()
            .filter(|c| c.cap() > 0 && self.conn_speed_of(t.id, c.id) > 0.0)
            .count()
    }

    /// 清空指定任务的连接级账本（新一次下载开始 / 任务移除 / 收尾时调用，FR-01-99）
    pub(super) fn clear_conn_stats(&mut self, task_id: u32) {
        self.conn_prev.retain(|(tid, _), _| *tid != task_id);
        self.conn_cum.retain(|(tid, _), _| *tid != task_id);
        self.conn_speed.retain(|(tid, _), _| *tid != task_id);
    }
}
