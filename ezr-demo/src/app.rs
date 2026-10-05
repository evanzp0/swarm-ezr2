//! app.rs — EZR Downloader TUI Demo 的应用状态、任务模型与伪造数据模拟
//!
//! Demo 说明：所有下载任务均为伪造数据，速度/进度由本地模拟器驱动，
//! 用于验证界面布局、状态流转与交互体验（状态切换规则口径对齐 mission/phase-01.md 第一期需求）。
//!
//! 页签口径：正在下载 = 等待/下载/暂停/校验/后期处理/失败；
//!           已完成   = 已完成 / 做种中。
//!
//! 状态机说明：不存在「连接中」状态——任务从等待中直接进入下载；
//! 已失败任务重试时先回到「等待中」队列；
//! 「等待中」任务按 Space 会暂停（退出等待队列，→ 已暂停）。
//!
//! 失败与重试规则（FR-M1-40~44 / 51 / 22）：
//! - 自动重试类（网络错误 / HTTP 408·429·5xx / 文件大小不符）：指数退避
//!   8s → 16s → 32s → 60s 封顶；响应带 Retry-After（≤60s）时优先采用；
//! - 不自动重试类（语义性 HTTP 4xx / 磁盘空间不足 / SHA-256 校验失败）：
//!   直接停等（「已达上限」式），仅可 R 手动重试；
//!   其中校验失败按 R 时清除断点、从头重新下载（数据损坏时断点无意义）；
//! - 续传一致性失效（服务器内容已更新）：断点作废、清零进度、从头重新下载；
//! - 重试计数连续性：连续失败（无进展）→ 累加、有进展 → 重置为 1；
//!   达上限（默认 5）→ 停止自动重试、释放槽位、显示「已达上限」。
//!
//! 下载槽位：同时可下载的任务数写死为 5（MAX_DOWNLOAD_SLOTS）。
//! 「下载中」与「已失败（未达重试上限、将自动重试）」的任务占用槽位；
//! 「等待中」任务须等槽位空出，按列表顺序（从上往下）依次获得槽位后开始下载。
//! 开始/续传前进行磁盘空间预检（演示：tensorflow 首次获得槽位时预检失败一次）。

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

/// 每个任务在列表中占用的行高（3 行内容 + 1 行空行分隔）
pub const ITEM_HEIGHT: u16 = 4;

/// 列表页签（仅两个）
pub const FILTERS: [&str; 2] = ["正在下载", "已完成"];

/// 下载槽位上限（demo 写死）：「正在下载」中同时可下载的任务数。
/// 等待中任务须等槽位空出，按列表顺序（从上往下）依次获得槽位后开始下载。
pub const MAX_DOWNLOAD_SLOTS: usize = 5;

/// braille 转轮字符（校验中/后期处理等动态效果）
const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

// ---------------------------------------------------------------------------
// 基础模型
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Protocol {
    Http,
    Https,
    Bt,
    /// 预留（暂未在 Demo 数据中使用）
    #[allow(dead_code)]
    Ftp,
}

impl Protocol {
    pub fn label(&self) -> &'static str {
        match self {
            Protocol::Http => "HTTP",
            Protocol::Https => "HTTPS",
            Protocol::Bt => "BT",
            Protocol::Ftp => "FTP",
        }
    }

    pub fn is_bt(&self) -> bool {
        matches!(self, Protocol::Bt)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskState {
    /// 排队等待（含失败重试后的重新排队）
    Queued,
    /// 下载中
    Downloading,
    /// 已暂停
    Paused,
    /// 分块校验（SHA-256）
    Verifying,
    /// 后期处理（解包/归档等）
    PostProcessing,
    /// 已完成
    Completed,
    /// 已失败
    Failed,
    /// 做种（BT）
    Seeding,
}

/// 失败类别（FR-M1-40/43/44/51/22 演示口径）：决定自动重试行为与列表行文案
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FailKind {
    /// 临时性错误（网络错误 / HTTP 408·429·5xx / 文件大小不符）：
    /// 按指数退避自动重试，Retry-After（≤60s）优先
    Transient,
    /// 语义性错误（HTTP 4xx 除 408/429 / 磁盘空间不足）：
    /// 不自动重试，直接停等（「已达上限」式），仅可 R 手动重试
    Fatal,
    /// 完整性校验失败：不自动重试；按 R 清除断点、从头重新下载
    Verify,
    /// 续传一致性失效（服务器内容已更新）：自动重试，但断点已作废、从头下载
    Invalidated,
}

impl TaskState {
    pub fn label(&self) -> &'static str {
        match self {
            TaskState::Queued => "等待中",
            TaskState::Downloading => "下载中",
            TaskState::Paused => "已暂停",
            TaskState::Verifying => "校验中",
            TaskState::PostProcessing => "后期处理中",
            TaskState::Completed => "已完成",
            TaskState::Failed => "已失败",
            TaskState::Seeding => "做种中",
        }
    }

    /// 是否属于「已完成」页签
    pub fn is_done(&self) -> bool {
        matches!(self, TaskState::Completed | TaskState::Seeding)
    }
}

/// 单个并发连接（分块）状态：一段连续的 byte range 由一个线程负责
#[derive(Clone)]
pub struct Connection {
    /// 分块起始字节（含）
    pub start: u64,
    /// 分块结束字节（不含）
    pub end: u64,
    /// 已下载字节
    pub done: u64,
    /// 当前速度 B/s
    pub speed: f64,
}

impl Connection {
    pub fn cap(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }
}

/// 下载任务
#[derive(Clone)]
pub struct Task {
    pub id: u32,
    pub name: String,
    pub protocol: Protocol,
    pub url: String,
    pub save_path: String,
    pub total: u64,
    pub downloaded: u64,
    /// 当前速度 B/s（聚合）
    pub speed: f64,
    /// 标称速度（模拟抖动基准）
    pub base_speed: f64,
    pub state: TaskState,
    /// 服务器是否支持断点续传（不支持时继续/重试只能从头下载，
    /// 且无法使用多线程分块下载）
    pub resumable: bool,
    /// 最大并发数（添加对话框可改 1–64；修改对话框可热调，v1.5/FR-01-87 同步）
    pub concurrency: usize,
    /// 并发分块连接（多线程下载一个任务）
    pub connections: Vec<Connection>,
    /// AIR2（固定块大小）：已完成分块数 x（由 tick 维护，HTTP 与 BT 同模型）
    pub chunk_done: u32,
    /// BT：可连接的做种数（seeders）
    pub seeders: usize,
    /// BT：邻居节点数（peers）
    pub peers: usize,
    /// BT 上传速度 B/s（做种/下载时分享）
    pub upload_speed: f64,
    /// BT 累计上传字节
    pub uploaded: u64,
    /// BT 剩余做种时间（秒）
    pub seed_left: f64,
    /// 完成后是否需要后期处理（按扩展名判定）
    pub post_process: bool,
    post_started: Option<Instant>,
    /// 已自动重试次数（连续失败时累加；非连续失败重置为 1；R 手动重试重置为 1）
    pub retries: u32,
    /// 自动重试上限
    pub max_retries: u32,
    /// 累计失败次数（仅用于失败原因轮换展示）
    pub fail_count: u32,
    /// 本次尝试（自上次失败/开始以来）是否下载到过数据。
    /// 重试计数规则：连续失败（无进展）→ 累加；非连续失败（有进展）→ 重置为 1
    pub made_progress: bool,
    /// 距下次自动重试的倒计时（秒）；None = 不再自动重试
    pub retry_in: Option<f64>,
    /// 最近一次失败的类别（决定「不自动重试/已达上限」行文案与 R 重试语义）
    pub fail_kind: Option<FailKind>,
    /// 演示用：该任务会周期性下载失败（用于展示失败/重试流转）
    pub flaky: bool,
    /// 演示用：首次完整性校验失败一次（FR-M1-51 演示；R 后重下校验通过）
    pub verify_fail_once: bool,
    /// 演示用：首次获得槽位开始下载前磁盘空间预检失败一次
    /// （FR-M1-44 演示；不自动重试，R 后预检通过）
    pub precheck_fail_once: bool,
    running_since: Option<Instant>,
    /// 「等待中」→ 开始下载的等待秒数（获得槽位后 2s 模拟建连 / 重试 3s / 新任务 2s）
    pub start_delay: f64,
    queued_since: Option<Instant>,
    /// 是否持有下载槽位（仅对「等待中」任务有独立意义：
    /// 下载中/待自动重试的失败任务恒持有；其余状态恒不持有，由 tick 槽位调度维护）
    pub has_slot: bool,
    verify_started: Option<Instant>,
    pub error: Option<String>,
    /// 完整性校验码（None = 未提供，完成后不校验）
    pub checksum: Option<Checksum>,
    /// 校验结果；None = 未校验（无校验码或尚未完成）
    pub verify_ok: Option<bool>,
    /// 代理选择（v1.5/FR-01-86 UI 同步；demo 仅承载选择，不影响模拟）
    pub proxy: ProxyChoice,
    pub created: String,
}

impl Task {
    pub fn progress(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.downloaded as f64 / self.total as f64).clamp(0.0, 1.0)
        }
    }

    pub fn eta_secs(&self) -> Option<u64> {
        if self.speed <= 0.0 || self.state != TaskState::Downloading {
            return None;
        }
        let remain = self.total.saturating_sub(self.downloaded) as f64;
        Some((remain / self.speed) as u64)
    }

    /// 并发线程数（展示用）：下载中 = 活跃连接数（有速度且未下载完的分块），
    /// 其余状态 = 有效连接数（排除已待命的空连接）
    pub fn thread_count(&self) -> usize {
        if self.state == TaskState::Downloading {
            self.connections
                .iter()
                .filter(|c| c.speed > 0.0 && c.done < c.cap())
                .count()
        } else {
            self.connections.iter().filter(|c| c.cap() > 0).count()
        }
    }

    /// 分块信息 (x=已完成块数, y=总块数, n=块大小字节)：
    /// 块大小按协议写死（HTTP 1 MB / BT 256 KB），块数 = ceil(total/块大小)，
    /// 与并发数解耦；x 由 tick 维护（分块队列动态领块，HTTP 与 BT 同模型）
    pub fn chunk_info(&self) -> (u32, u32, u64) {
        let piece = chunk_size(self.protocol);
        let y = chunk_total(self.total, piece);
        (self.chunk_done.min(y), y, piece)
    }
}

// ---------------------------------------------------------------------------
// 简易 xorshift 随机源（避免引入 rand 依赖，保证 demo 确定性可控）
// ---------------------------------------------------------------------------

pub struct Rng {
    s: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { s: seed | 1 }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.s;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.s = x;
        x
    }

    pub fn f01(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / 9_007_199_254_740_992.0
    }

    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f01()
    }
}

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

/// 任务的完整性校验码（添加对话框可选填）
#[derive(Clone, Debug)]
pub struct Checksum {
    /// 算法显示名（CHECKSUM_ALGOS 中的名称）
    pub algo: &'static str,
    /// 十六进制校验码（确认时统一小写）
    pub value: String,
}

/// 代理选择（v1.5/FR-01-86 UI 同步；demo 仅承载选择本身，不影响模拟器）
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ProxyChoice {
    /// 直连
    Direct,
    /// 命名代理（按名称引用演示代理表）
    Named(String),
}

/// 演示命名代理表（伪造数据；下拉显示名带类型标注，v1.9 三值口径）
fn demo_proxies() -> Vec<(&'static str, &'static str)> {
    vec![("办公网代理", "http"), ("本地 SOCKS5", "socks5")]
}

// ---------------------------------------------------------------------------
// 对话框（添加任务 / 删除任务）
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DialogKind {
    /// 添加任务
    Add,
    /// 删除任务
    Delete,
    /// 修改任务（v1.5/FR-01-87 同步：并发/校验算法/校验码/代理，确定立即生效）
    Modify,
}

pub struct Dialog {
    pub kind: DialogKind,
    pub url: String,
    pub dir: String,
    /// Add: 并发数输入缓冲（数字字符串；HTTP 默认 4 / BT 默认 20，可修改）
    pub conns: String,
    /// Add: 用户是否手动修改过并发数（未修改时随 URL 类型自动同步默认值）
    pub conns_edited: bool,
    /// Add: 校验算法在 CHECKSUM_ALGOS 中的下标（默认 SHA-256 = 3）
    pub ck_type: usize,
    /// Add: 校验码输入缓冲（十六进制，可留空 = 不校验）
    pub ck_value: String,
    /// Add: 校验算法下拉框是否展开
    pub ck_open: bool,
    /// Add/Modify: 下拉框当前高亮项（↑↓ 导航，实时同步 ck_type）
    pub ck_sel: usize,
    /// Add/Modify: 代理下拉在 [`App::proxy_options`] 中的下标（v1.5/FR-01-86）
    pub proxy_sel: usize,
    /// Add/Modify: 代理下拉框是否展开
    pub proxy_open: bool,
    /// Add:    0=URL 1=保存目录 2=并发数 3=校验算法 4=校验码 5=代理 6=确认 7=取消
    /// Modify: 0=并发数 1=校验算法 2=校验码 3=代理 4=确定 5=取消
    /// Delete: 0=仅删除任务  1=删除任务和文件  2=取消
    pub focus: usize,
    /// Delete 用：待删除任务名
    pub task_name: String,
    /// Modify 用：待修改任务 id（None = Add/Delete）
    pub task_id: Option<u32>,
}

// ---------------------------------------------------------------------------
// 应用状态
// ---------------------------------------------------------------------------

pub struct App {
    pub tasks: Vec<Task>,
    /// 当前页签视图中的选中序号
    pub selected: usize,
    /// 列表滚动偏移（以"任务条目"为单位）
    pub scroll: usize,
    /// 页签下标（对应 FILTERS）
    pub filter: usize,
    /// 全局下载速度历史（KB/s 采样）
    pub speed_hist: Vec<u64>,
    /// 全局上传速度历史（KB/s 采样）
    pub up_hist: Vec<u64>,
    /// 本次会话累计下载字节
    pub session_bytes: u64,
    pub rng: Rng,
    pub frame: u64,
    pub quit: bool,
    pub toast: Option<String>,
    toast_until: Option<Instant>,
    pub next_id: u32,
    last_tick: Instant,
    /// 列表可视条目数（由 ui 层回填，用于滚动钳制）
    pub visible_rows: usize,
    /// 列表内框区域（由 ui 层回填，用于鼠标命中测试）
    pub list_area: Option<Rect>,
    /// 是否显示右侧速度图表面板
    pub show_chart: bool,
    /// 当前打开的对话框（None = 无）
    pub dialog: Option<Dialog>,
    /// 对话框按钮的可点击区域（由 ui 层每帧回填）
    pub dlg_btn_rects: Vec<(Rect, usize)>,
    /// 对话框输入行/选择行的可点击区域（由 ui 层每帧回填；点击聚焦，
    /// 校验算法行再点击可展开下拉框）
    pub dlg_field_rects: Vec<(Rect, usize)>,
    /// 校验算法下拉框各选项的可点击区域（由 ui 层每帧回填，仅展开时有效）
    pub dlg_ck_rects: Vec<(Rect, usize)>,
    /// 代理下拉选项命中区域（v1.5/FR-01-86；鼠标点击选择）
    pub dlg_proxy_rects: Vec<(Rect, usize)>,
    /// 代理选项表（v1.5/FR-01-86 同步；演示数据：直连 + 命名条目）
    pub proxy_options: Vec<ProxyChoice>,
    /// 代理选项显示名（与 proxy_options 一一对应；命名条目含类型标注，不含认证信息）
    pub proxy_labels: Vec<String>,
    /// 添加对话框代理默认选中下标（恒直连 = 0）
    pub default_proxy_sel: usize,
}

impl App {
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E3779B97F4A7C15);
        // 代理选项表（v1.5/FR-01-86 同步）：直连恒在 + 演示命名条目（显示名带类型标注）
        let mut proxy_options = vec![ProxyChoice::Direct];
        let mut proxy_labels = vec!["直连".to_string()];
        for (name, kind) in demo_proxies() {
            proxy_options.push(ProxyChoice::Named(name.to_string()));
            proxy_labels.push(format!("{}（{}）", name, kind));
        }
        App {
            tasks: demo_tasks(),
            selected: 0,
            scroll: 0,
            filter: 0,
            speed_hist: vec![0; 90],
            up_hist: vec![0; 90],
            session_bytes: 0,
            rng: Rng::new(seed),
            frame: 0,
            quit: false,
            toast: Some("EZR Downloader Demo 就绪 · 数据为模拟数据".to_string()),
            toast_until: Some(Instant::now() + Duration::from_secs(4)),
            next_id: 100,
            last_tick: Instant::now(),
            visible_rows: 6,
            list_area: None,
            show_chart: true,
            dialog: None,
            dlg_btn_rects: Vec::new(),
            dlg_field_rects: Vec::new(),
            dlg_ck_rects: Vec::new(),
            dlg_proxy_rects: Vec::new(),
            proxy_options,
            proxy_labels,
            default_proxy_sel: 0,
        }
    }

    pub fn spinner(&self) -> char {
        SPINNER[(self.frame as usize) % SPINNER.len()]
    }

    /// 当前占用下载槽位的任务数（下载中 / 待自动重试的已失败 / 已获槽位的等待中）
    pub fn used_slots(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| match t.state {
                TaskState::Downloading => true,
                // 已失败且仍会自动重试（未达上限）→ 继续占用槽位；
                // 达上限停止自动重试的失败任务不占用
                TaskState::Failed => t.retry_in.is_some(),
                TaskState::Queued => t.has_slot,
                _ => false,
            })
            .count()
    }

    /// 等待任务在队列中的位次（1 基，按列表顺序从上往下）：None = 未在排队。
    /// 已持有槽位的等待中任务（重试排队）不算队列位次。
    pub fn queue_pos_of(&self, task_id: u32) -> Option<usize> {
        let mut pos = 0;
        for t in &self.tasks {
            if t.state == TaskState::Queued && !t.has_slot {
                pos += 1;
                if t.id == task_id {
                    return Some(pos);
                }
            }
        }
        None
    }

    /// 下载槽位调度（每 tick 开头执行）：
    /// 1) 槽位不变式维护：下载中/待自动重试的失败任务恒持有槽位，其余状态（除等待中）恒不持有；
    /// 2) 空闲槽位按列表顺序（从上往下）分配给最靠前的未持有槽位的「等待中」任务。
    fn enforce_slots(&mut self) {
        for t in self.tasks.iter_mut() {
            match t.state {
                TaskState::Downloading => t.has_slot = true,
                TaskState::Failed => t.has_slot = t.retry_in.is_some(),
                TaskState::Queued => {}
                _ => t.has_slot = false,
            }
        }
        let mut used = self.used_slots();
        if used >= MAX_DOWNLOAD_SLOTS {
            return;
        }
        for t in self.tasks.iter_mut() {
            if used >= MAX_DOWNLOAD_SLOTS {
                break;
            }
            if t.state == TaskState::Queued && !t.has_slot {
                t.has_slot = true;
                used += 1;
            }
        }
    }

    /// 过滤后的任务下标视图（页签 0 = 正在下载，页签 1 = 已完成）
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

    pub fn sel_idx(&self) -> Option<usize> {
        self.filtered().get(self.selected).copied()
    }

    pub fn sel_task(&self) -> Option<&Task> {
        self.sel_idx().and_then(|i| self.tasks.get(i))
    }

    fn set_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some(msg.into());
        self.toast_until = Some(Instant::now() + Duration::from_secs(3));
    }

    fn move_sel(&mut self, delta: i32) {
        let flen = self.filtered().len();
        if flen == 0 {
            return;
        }
        let cur = self.selected as i32;
        self.selected = cur.saturating_add(delta).clamp(0, flen as i32 - 1) as usize;
    }

    /// 上移/下移选中任务（在当前页签内交换位置）
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
            if delta < 0 {
                "↑ 已上移"
            } else {
                "↓ 已下移"
            },
            name
        ));
    }

    // -----------------------------------------------------------------------
    // 主循环 tick：推进模拟、状态机、历史曲线与视图钳制
    // -----------------------------------------------------------------------

    pub fn tick(&mut self) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_tick)
            .as_secs_f64()
            .clamp(0.001, 0.5);
        self.last_tick = now;
        self.frame = self.frame.wrapping_add(1);

        // 下载槽位调度：先回收/分配槽位，再进行状态推进
        self.enforce_slots();

        let rng = &mut self.rng;
        let mut global_dl = 0.0f64;
        let mut global_ul = 0.0f64;
        let mut notices: Vec<String> = Vec::new();

        for t in self.tasks.iter_mut() {
            match t.state {
                TaskState::Downloading => {
                    // 演示：首次失败之后的重试模拟「连接卡死」——速度为 0、无数据下载，
                    // 用于展示「连续失败（无进展）累加重试次数」规则
                    let stalled = t.flaky && t.fail_count > 1;
                    // 聚合速度抖动 ±18%
                    t.speed = if stalled {
                        0.0
                    } else {
                        t.base_speed * rng.range(0.82, 1.18)
                    };
                    // 分块推进：每个连接独立抖动
                    let n = t.connections.len().max(1) as f64;
                    for c in t.connections.iter_mut() {
                        if c.cap() > 0 && c.done < c.cap() {
                            let seg = (t.speed / n) * rng.range(0.55, 1.45);
                            c.speed = seg;
                            c.done = (c.done as f64 + seg * dt).min(c.cap() as f64) as u64;
                        } else {
                            c.speed = 0.0;
                        }
                    }
                    // 分块队列（HTTP 与 BT 同模型，仅块大小不同）：完成一块立即领取下一块，
                    // 队列临近结束时剩余连接自然转入待命（start=end=0）
                    let piece = chunk_size(t.protocol);
                    let y = chunk_total(t.total, piece) as u64;
                    let k = t
                        .connections
                        .iter()
                        .filter(|c| c.cap() > 0 && c.done >= c.cap())
                        .count() as u32;
                    if k > 0 {
                        let l = t.connections.iter().filter(|c| c.cap() > 0).count() as u32;
                        let mut next = (t.chunk_done + l) as u64;
                        for c in t.connections.iter_mut() {
                            if c.cap() > 0 && c.done >= c.cap() {
                                if next < y {
                                    c.start = next * piece;
                                    c.end = if next + 1 == y {
                                        t.total
                                    } else {
                                        c.start + piece
                                    };
                                    c.done = 0;
                                    c.speed = 0.0;
                                    next += 1;
                                } else {
                                    // 队列已空：连接待命
                                    c.start = 0;
                                    c.end = 0;
                                    c.done = 0;
                                    c.speed = 0.0;
                                }
                            }
                        }
                        t.chunk_done += k;
                    }
                    // 已下载 = 已完成块 × 块大小 + 在传块内进度（末块差额在全部完成时归位）
                    let inflight: u64 = t
                        .connections
                        .iter()
                        .filter(|c| c.cap() > 0 && c.done < c.cap())
                        .map(|c| c.done)
                        .sum();
                    let mut agg = t.chunk_done as u64 * piece + inflight;
                    if t.chunk_done as u64 >= y {
                        agg = t.total;
                    }
                    t.downloaded = agg.min(t.total);
                    // 本次尝试已有下载进展（用于重试计数的连续性判定）
                    if !stalled {
                        t.made_progress = true;
                    }
                    global_dl += t.speed;
                    // BT 边下边传
                    if t.protocol.is_bt() {
                        t.upload_speed = 420_000.0 * rng.range(0.5, 1.7);
                        t.uploaded = (t.uploaded as f64 + t.upload_speed * dt) as u64;
                        global_ul += t.upload_speed;
                    }
                    // 演示用：周期性失败（展示失败分类/退避倒计时/自动重试流转，FR-M1-40~43）；
                    // 卡死重试 2.0s 即失败，正常尝试 6s 后失败
                    if t.flaky {
                        let fail_after = if t.fail_count > 1 { 2.0 } else { 6.0 };
                        let mut re_fail = false;
                        if let Some(rs) = t.running_since {
                            if now.duration_since(rs) > Duration::from_secs_f64(fail_after) {
                                re_fail = true;
                            }
                        }
                        if re_fail {
                            t.state = TaskState::Failed;
                            t.running_since = None;
                            t.speed = 0.0;
                            t.upload_speed = 0.0;
                            t.fail_count += 1;
                            let (msg, kind, retry_after) = fail_case(t.fail_count);
                            t.error = Some(msg.to_string());
                            t.fail_kind = Some(kind);
                            // 重试计数规则：连续失败（自上次失败后未下载到任何数据）→ 累加；
                            // 非连续失败（重试期间有下载进展）→ 重置为 1（FR-M1-41）
                            let reset = t.made_progress;
                            t.retries = if reset { 1 } else { t.retries + 1 };
                            t.made_progress = false;
                            let rule = if reset {
                                format!(
                                    "非连续失败，重试次数重置为 {}/{}",
                                    t.retries, t.max_retries
                                )
                            } else {
                                format!("连续失败，重试次数累加至 {}/{}", t.retries, t.max_retries)
                            };
                            // 自动重试倒计时：指数退避 8→16→32→60s 封顶；
                            // 响应带 Retry-After（≤60s）时优先采用（FR-M1-42/43）
                            let secs = retry_after.unwrap_or_else(|| backoff_secs(t.retries));
                            let auto_kind =
                                matches!(kind, FailKind::Transient | FailKind::Invalidated);
                            if auto_kind && t.retries < t.max_retries {
                                // 未达最大重试次数：倒计时后自动重试，期间继续占用下载槽位
                                t.retry_in = Some(secs);
                                t.has_slot = true;
                                if kind == FailKind::Invalidated {
                                    // 续传一致性失效（FR-M1-22）：sidecar 作废、断点清零，
                                    // 重试时从头重新下载
                                    let total = t.total;
                                    let n = t.connections.len().max(1);
                                    let piece = chunk_size(t.protocol);
                                    t.downloaded = 0;
                                    t.chunk_done = 0;
                                    t.connections = make_chunk_conns(total, 0, n, piece).0;
                                    notices.push(format!(
                                        "⚠ 服务器内容已更新，断点已作废，{}s 后从头重新下载: {}",
                                        secs as u64, t.name
                                    ));
                                } else {
                                    notices.push(format!(
                                        "✘ 下载失败（{}），{}s 后自动重试: {}",
                                        rule, secs as u64, t.name
                                    ));
                                }
                            } else if auto_kind {
                                // 已达最大重试次数：停止自动重试并释放槽位（按 R 手动重试）
                                t.retry_in = None;
                                t.has_slot = false;
                                notices.push(format!(
                                    "✘ 已达最大重试次数（{}/{}），停止自动重试，按 R 手动重试: {}",
                                    t.retries, t.max_retries, t.name
                                ));
                            } else {
                                // 语义性 4xx / 磁盘空间不足：不自动重试，
                                // 直接进入「已达上限」式停等，释放槽位（FR-M1-43/44）
                                t.retry_in = None;
                                t.has_slot = false;
                                notices.push(format!(
                                    "✘ 下载失败（{}）：不自动重试，按 R 手动重试: {}",
                                    msg, t.name
                                ));
                            }
                            continue;
                        }
                    }
                    if t.downloaded >= t.total {
                        t.speed = 0.0;
                        t.upload_speed = 0.0;
                        t.running_since = None;
                        t.has_slot = false; // 下载阶段结束，释放下载槽位
                                            // 有校验码 → 进入「校验中」；无校验码 → 直接完成/后期处理
                        let algo = t.checksum.as_ref().map(|c| c.algo);
                        if let Some(algo) = algo {
                            t.state = TaskState::Verifying;
                            t.verify_started = Some(now);
                            notices.push(format!("✓ 下载完成，开始{}校验: {}", algo, t.name));
                        } else if t.post_process {
                            t.state = TaskState::PostProcessing;
                            t.post_started = Some(now);
                            notices.push(format!("✓ 下载完成，开始后期处理: {}", t.name));
                        } else {
                            t.state = TaskState::Completed;
                            notices.push(format!("✓ 下载完成: {}", t.name));
                        }
                    }
                }
                TaskState::Verifying => {
                    if let Some(s) = t.verify_started {
                        if now.duration_since(s) > Duration::from_millis(2600) {
                            let algo = t.checksum.as_ref().map(|c| c.algo).unwrap_or("SHA-256");
                            t.verify_started = None;
                            t.speed = 0.0;
                            t.upload_speed = 0.0;
                            // 校验失败路径（FR-M1-51）：不自动重试；按 R 清除断点从头重新下载
                            //（演示：gpt4all 首次校验失败，R 重下后校验通过）
                            if t.verify_fail_once {
                                t.verify_fail_once = false;
                                t.verify_ok = Some(false);
                                t.state = TaskState::Failed;
                                t.fail_kind = Some(FailKind::Verify);
                                t.retry_in = None;
                                t.has_slot = false;
                                t.error = Some(format!("{} 校验失败：内容与校验值不符", algo));
                                notices.push(format!(
                                    "✘ {} 校验失败：不自动重试，按 R 清除断点从头下载: {}",
                                    algo, t.name
                                ));
                            } else {
                                t.verify_ok = Some(true);
                                if t.post_process {
                                    t.state = TaskState::PostProcessing;
                                    t.post_started = Some(now);
                                    notices.push(format!(
                                        "✓ {} 校验通过，开始后期处理: {}",
                                        algo, t.name
                                    ));
                                } else {
                                    t.state = TaskState::Completed;
                                    notices.push(format!("✓ {} 校验通过: {}", algo, t.name));
                                }
                            }
                        }
                    }
                }
                TaskState::PostProcessing => {
                    if let Some(s) = t.post_started {
                        if now.duration_since(s) > Duration::from_millis(3400) {
                            t.state = TaskState::Completed;
                            t.post_started = None;
                            t.speed = 0.0;
                            t.upload_speed = 0.0;
                            notices.push(format!("✓ 后期处理完成: {}", t.name));
                        }
                    }
                }
                TaskState::Seeding => {
                    t.upload_speed = 1_900_000.0 * rng.range(0.6, 1.5);
                    t.uploaded = (t.uploaded as f64 + t.upload_speed * dt) as u64;
                    global_ul += t.upload_speed;
                    t.seed_left = (t.seed_left - dt).max(0.0);
                    if t.seed_left <= 0.0 {
                        t.state = TaskState::Completed;
                        t.upload_speed = 0.0;
                        notices.push(format!("✓ 做种结束: {}", t.name));
                    }
                }
                TaskState::Queued => {
                    // 未持有槽位的等待任务继续排队，等 enforce_slots 按列表顺序分配槽位
                    if !t.has_slot {
                        continue;
                    }
                    if let Some(q) = t.queued_since {
                        if now.duration_since(q) > Duration::from_secs_f64(t.start_delay.max(0.5)) {
                            // 磁盘空间预检（FR-M1-44）：开始/续传前检查保存目录可用空间，
                            // 小于剩余需下载量 → 失败（不自动重试，R 后预检通过）。
                            //（演示：tensorflow 首次获得槽位开始前预检失败一次）
                            if t.precheck_fail_once {
                                t.precheck_fail_once = false;
                                t.state = TaskState::Failed;
                                t.fail_kind = Some(FailKind::Fatal);
                                t.retry_in = None;
                                t.has_slot = false; // 预检失败释放槽位
                                t.queued_since = None;
                                t.error = Some(
                                    "磁盘空间不足（保存目录剩余空间小于待下载量）".to_string(),
                                );
                                notices.push(format!(
                                    "✘ 磁盘空间不足：开始前预检失败，不自动重试，按 R 手动重试: {}",
                                    t.name
                                ));
                                continue;
                            }
                            // 已持有槽位且等待结束：进入下载（无「连接中」状态）；
                            // 不支持断点续传的任务重试/重新排队后只能从头开始
                            if !t.resumable {
                                let total = t.total;
                                let n = t.connections.len().max(1);
                                let piece = chunk_size(t.protocol);
                                t.downloaded = 0;
                                t.chunk_done = 0;
                                t.connections = make_chunk_conns(total, 0, n, piece).0;
                            }
                            t.state = TaskState::Downloading;
                            t.queued_since = None;
                            t.running_since = Some(now);
                            t.made_progress = false; // 新尝试开始：清空进展标记
                            let name = t.name.clone();
                            if t.fail_count > 0 {
                                notices.push(format!(
                                    "↻ 自动重试开始（{}/{}）: {}",
                                    t.retries, t.max_retries, name
                                ));
                            } else {
                                notices.push(format!("▶ 槽位空闲，开始下载: {}", name));
                            }
                        }
                    }
                }
                TaskState::Failed => {
                    if let Some(left) = t.retry_in.as_mut() {
                        *left -= dt;
                        if *left <= 0.0 {
                            t.retry_in = None;
                            // 进入「等待中」重新排队；重试计数已在失败时按连续性规则更新，
                            // 槽位保持占用（未达最大重试次数的失败任务继续持有槽位）
                            t.state = TaskState::Queued;
                            t.error = None;
                            t.queued_since = Some(now);
                            t.start_delay = 3.0;
                            notices.push(format!(
                                "↻ 自动重试（{}/{}）→ 等待中: {}",
                                t.retries, t.max_retries, t.name
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        // 会话统计与速度历史
        self.session_bytes += (global_dl * dt) as u64;
        self.speed_hist.push((global_dl / 1024.0) as u64);
        if self.speed_hist.len() > 180 {
            self.speed_hist.remove(0);
        }
        self.up_hist.push((global_ul / 1024.0) as u64);
        if self.up_hist.len() > 180 {
            self.up_hist.remove(0);
        }

        if let Some(last) = notices.pop() {
            self.set_toast(last);
        }
        if let Some(u) = self.toast_until {
            if now > u {
                self.toast = None;
                self.toast_until = None;
            }
        }

        // 选中项 / 滚动钳制
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

    // -----------------------------------------------------------------------
    // 键盘交互
    // -----------------------------------------------------------------------

    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) {
            if code == KeyCode::Char('c') {
                self.quit = true;
            }
            return;
        }
        // 对话框打开时，按键优先交给对话框
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
            KeyCode::Char('m') | KeyCode::Char('M') => self.open_modify_dialog(),
            KeyCode::Char('d') | KeyCode::Char('D') => self.open_delete_dialog(),
            KeyCode::Char('c') | KeyCode::Char('C') => self.clear_completed(),
            KeyCode::Char('g') | KeyCode::Char('G') => self.show_chart = !self.show_chart,
            KeyCode::Char('u') | KeyCode::Char('U') => self.move_task(-1),
            KeyCode::Char('j') | KeyCode::Char('J') => self.move_task(1),
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // 对话框键盘处理
    // -----------------------------------------------------------------------

    fn on_dialog_key(&mut self, code: KeyCode) {
        let (kind, focus) = match &self.dialog {
            Some(d) => (d.kind, d.focus),
            None => return,
        };
        let nfocus = match kind {
            DialogKind::Add => 8,
            DialogKind::Modify => 6,
            DialogKind::Delete => 3,
        };
        // 校验算法 / 代理下拉框展开时：整块交给下拉键处理（消费所有键）
        if self.dialog.as_ref().is_some_and(|d| d.ck_open) {
            if let Some(d) = self.dialog.as_mut() {
                ck_dropdown_open_key(d, code);
            }
            return;
        }
        if self.dialog.as_ref().is_some_and(|d| d.proxy_open) {
            let n = self.proxy_options.len().max(1);
            if let Some(d) = self.dialog.as_mut() {
                proxy_dropdown_open_key(d, n, code);
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
                DialogKind::Add => self.add_dialog_enter(focus),
                DialogKind::Modify => self.modify_dialog_enter(focus),
                DialogKind::Delete => self.dlg_activate_delete(focus),
            },
            KeyCode::Backspace => {
                if let Some(d) = self.dialog.as_mut() {
                    text_backspace(kind, d, focus);
                    // URL 变化时同步默认并发数（用户未手动修改过时，demo 体验保留）
                    if kind == DialogKind::Add && focus == 0 && !d.conns_edited {
                        let (p, _) = parse_url_task(d.url.trim());
                        d.conns = default_conns(p).to_string();
                    }
                }
            }
            KeyCode::Char(c) => match kind {
                DialogKind::Add => self.add_dialog_char(focus, c),
                DialogKind::Modify => self.modify_dialog_char(focus, c),
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

    /// Add 对话框 Enter：算法/代理行展开下拉、取消按钮关闭、其余确认
    fn add_dialog_enter(&mut self, focus: usize) {
        if focus == 3 {
            let d = self.dialog.as_mut().unwrap();
            d.ck_open = true;
            d.ck_sel = d.ck_type;
        } else if focus == 5 {
            let d = self.dialog.as_mut().unwrap();
            d.proxy_open = true;
        } else if focus == 7 {
            self.dialog = None;
        } else {
            self.dlg_confirm_add();
        }
    }

    /// Modify 对话框 Enter（v1.5/FR-01-87）：0=并发 1=算法 2=校验码 3=代理
    /// 4=确定 5=取消
    fn modify_dialog_enter(&mut self, focus: usize) {
        if focus == 1 {
            let d = self.dialog.as_mut().unwrap();
            d.ck_open = true;
            d.ck_sel = d.ck_type;
        } else if focus == 3 {
            let d = self.dialog.as_mut().unwrap();
            d.proxy_open = true;
        } else if focus == 5 {
            self.dialog = None;
        } else {
            self.dlg_confirm_modify();
        }
    }

    /// Add 对话框字符输入：0-2 文本字段、3 空格展开算法下拉、4 校验码、
    /// 5 空格展开代理下拉、6/7 空格激活按钮
    fn add_dialog_char(&mut self, focus: usize, c: char) {
        if focus < 3 {
            if !c.is_control() {
                let d = self.dialog.as_mut().unwrap();
                match focus {
                    0 => {
                        if d.url.chars().count() < 300 {
                            d.url.push(c);
                            // URL 变化时同步默认并发数（用户未手动修改过时）
                            if !d.conns_edited {
                                let (p, _) = parse_url_task(d.url.trim());
                                d.conns = default_conns(p).to_string();
                            }
                        }
                    }
                    1 => {
                        if d.dir.chars().count() < 300 {
                            d.dir.push(c);
                        }
                    }
                    // 并发数：仅接受数字，最多 2 位（提交时钳制 1-64）
                    _ => {
                        if c.is_ascii_digit() && d.conns.chars().count() < 2 {
                            d.conns.push(c);
                            d.conns_edited = true;
                        }
                    }
                }
            }
        } else if focus == 3 {
            // 校验算法行：Space 同样展开下拉框，其余字符忽略
            if c == ' ' {
                let d = self.dialog.as_mut().unwrap();
                d.ck_open = true;
                d.ck_sel = d.ck_type;
            }
        } else if focus == 4 {
            // 校验码：仅接受十六进制字符，最多 128 位（SHA-512）
            if c.is_ascii_hexdigit() && self.dialog.as_ref().unwrap().ck_value.chars().count() < 128
            {
                self.dialog.as_mut().unwrap().ck_value.push(c);
            }
        } else if c == ' ' {
            if focus == 5 {
                let d = self.dialog.as_mut().unwrap();
                d.proxy_open = true;
            } else {
                self.dlg_activate_add(focus);
            }
        }
    }

    /// Modify 对话框字符输入（v1.5/FR-01-87）：0 并发数字、1 空格展开算法
    /// 下拉、2 校验码、3 空格展开代理下拉、4/5 空格激活按钮
    fn modify_dialog_char(&mut self, focus: usize, c: char) {
        if focus == 0 {
            if c.is_ascii_digit() && self.dialog.as_ref().unwrap().conns.chars().count() < 2 {
                let d = self.dialog.as_mut().unwrap();
                d.conns.push(c);
                d.conns_edited = true;
            }
        } else if focus == 1 {
            if c == ' ' {
                let d = self.dialog.as_mut().unwrap();
                d.ck_open = true;
                d.ck_sel = d.ck_type;
            }
        } else if focus == 2 {
            if c.is_ascii_hexdigit() && self.dialog.as_ref().unwrap().ck_value.chars().count() < 128
            {
                self.dialog.as_mut().unwrap().ck_value.push(c);
            }
        } else if c == ' ' {
            if focus == 3 {
                let d = self.dialog.as_mut().unwrap();
                d.proxy_open = true;
            } else {
                self.dlg_activate_modify(focus);
            }
        }
    }

    fn dlg_activate_add(&mut self, btn: usize) {
        match btn {
            6 => self.dlg_confirm_add(),
            7 => self.dialog = None,
            _ => {}
        }
    }

    /// 修改任务按钮激活（v1.5/FR-01-87：4=确定 5=取消）
    fn dlg_activate_modify(&mut self, btn: usize) {
        match btn {
            4 => self.dlg_confirm_modify(),
            5 => self.dialog = None,
            _ => {}
        }
    }

    /// 确认修改（v1.5/FR-01-87，D19/D20 同步）：并发（空=保持，非法钳制 1–64）、
    /// 校验（空=清除，非法聚焦提示不生效）、代理（下拉选择）；demo 立即写任务字段
    fn dlg_confirm_modify(&mut self) {
        let Some(d) = self.dialog.as_ref() else {
            return;
        };
        let Some(id) = d.task_id else {
            return;
        };
        let Some(idx) = self.tasks.iter().position(|t| t.id == id) else {
            self.dialog = None;
            return;
        };
        let conns_raw = d.conns.trim().to_string();
        let ck_raw = d.ck_value.trim().to_string();
        let ck_type = d.ck_type;
        let proxy_sel = d.proxy_sel.min(self.proxy_options.len().saturating_sub(1));
        let proxy = self.proxy_options[proxy_sel].clone();

        // 并发：空 = 保持当前；非空按 1–64 钳制
        let concurrency = if conns_raw.is_empty() {
            self.tasks[idx].concurrency
        } else {
            conns_raw.parse::<usize>().unwrap_or(0).clamp(1, 64)
        };

        // 校验：空 = 清除；非空同添加对话框合法性口径，非法 → 不生效 + 聚焦校验码字段
        let checksum = if ck_raw.is_empty() {
            None
        } else {
            let (algo, need) = CHECKSUM_ALGOS[ck_type.min(CHECKSUM_ALGOS.len() - 1)];
            let v = ck_raw.to_lowercase();
            if !v.chars().all(|c| c.is_ascii_hexdigit()) {
                self.set_toast(format!("⚠ {algo} 校验码只能是十六进制字符（0-9a-f）"));
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = 2;
                }
                return;
            }
            let len = v.chars().count();
            if len != need {
                self.set_toast(format!(
                    "⚠ {algo} 校验码需为 {need} 位十六进制（当前 {len} 位）"
                ));
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = 2;
                }
                return;
            }
            Some(Checksum { algo, value: v })
        };

        {
            let t = &mut self.tasks[idx];
            let changed = t.concurrency != concurrency;
            t.concurrency = concurrency;
            t.checksum = checksum;
            t.proxy = proxy;
            if changed {
                // 立即生效（v1.5/FR-01-87 demo 语义）：按新并发重建分块连接
                //（沿用 make_chunk_conns，已下载字节按块进度重分配，不丢进度）
                let piece = chunk_size(t.protocol);
                let (conns_new, x) = make_chunk_conns(t.total, t.downloaded, concurrency, piece);
                t.connections = conns_new;
                t.chunk_done = x;
            }
        }
        self.dialog = None;
        self.set_toast("✓ 任务参数已更新，立即生效");
    }

    fn dlg_confirm_add(&mut self) {
        // 先取出全部需要的值（避免后续可变借用冲突）
        let Some(d) = self.dialog.as_ref() else {
            return;
        };
        let url = d.url.trim().to_string();
        if url.is_empty() {
            self.set_toast("⚠ 请输入下载 URL");
            return;
        }
        let dir_raw = d.dir.trim().to_string();
        let conns_raw = d.conns.trim().to_string();
        let ck_raw = d.ck_value.trim().to_string();
        let ck_type = d.ck_type;

        // 校验码验证：非空时必须是纯十六进制且长度与算法匹配
        let checksum = if ck_raw.is_empty() {
            None
        } else {
            let (algo, need) = CHECKSUM_ALGOS[ck_type.min(CHECKSUM_ALGOS.len() - 1)];
            let v = ck_raw.to_lowercase();
            if !v.chars().all(|c| c.is_ascii_hexdigit()) {
                self.set_toast(format!("⚠ {} 校验码只能是十六进制字符（0-9a-f）", algo));
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = 4;
                }
                return;
            }
            let len = v.chars().count();
            if len != need {
                self.set_toast(format!(
                    "⚠ {} 校验码需为 {} 位十六进制（当前 {} 位）",
                    algo, need, len
                ));
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = 4;
                }
                return;
            }
            Some(Checksum { algo, value: v })
        };

        let dir = if dir_raw.is_empty() {
            "/srv/downloads".to_string()
        } else {
            dir_raw.trim_end_matches('/').to_string()
        };

        let (proto, mut name) = parse_url_task(&url);
        // 目标文件重名：自动追加序号 .1 / .2 …（不覆盖既有条目，FR-M1-02）
        let base = name.clone();
        let mut seq = 1u32;
        while self.tasks.iter().any(|t| t.name == name) {
            name = format!("{}.{}", base, seq);
            seq += 1;
        }
        // 并发数：取对话框输入（钳制 1-64）；留空或非法时用协议默认（HTTP 4 / BT 20）
        let conns = conns_raw
            .parse::<usize>()
            .map(|v| v.clamp(1, 64))
            .unwrap_or_else(|_| default_conns(proto));
        let id = self.next_id;
        self.next_id += 1;
        let proxy = self.proxy_options[d.proxy_sel.min(self.proxy_options.len() - 1)].clone();
        let total = pseudo_total(&name);
        let save_path = format!("{}/{}", dir, name);
        let base = 5_000_000.0 + (id as f64 * 1.37e6) % 7_000_000.0;
        let mut t = mk(
            id,
            &name,
            proto,
            &url,
            &save_path,
            total,
            TaskState::Queued,
            0,
            conns,
            base,
            "刚刚",
        );
        t.post_process = is_archive(&name);
        t.queued_since = Some(Instant::now());
        t.start_delay = 2.0;
        t.checksum = checksum;
        t.proxy = proxy;
        self.tasks.push(t);
        self.dialog = None;
        // 切到「正在下载」页签并选中新任务
        self.filter = 0;
        let flen = self.filtered().len();
        if flen > 0 {
            self.selected = flen - 1;
        }
        self.set_toast(format!("✓ 已添加任务 #{}: {}", id, name));
        if self.used_slots() >= MAX_DOWNLOAD_SLOTS {
            self.set_toast(format!(
                "✓ 已添加任务 #{}: {}（槽位已满 {}/{}，进入等待队列）",
                id,
                name,
                self.used_slots(),
                MAX_DOWNLOAD_SLOTS
            ));
        }
    }

    fn dlg_activate_delete(&mut self, btn: usize) {
        let Some(idx) = self.sel_idx() else {
            self.dialog = None;
            return;
        };
        let name = self.tasks[idx].name.clone();
        match btn {
            0 => {
                self.tasks.remove(idx);
                self.dialog = None;
                self.set_toast(format!("🗑 已删除任务（保留文件）: {}", name));
            }
            1 => {
                self.tasks.remove(idx);
                self.dialog = None;
                self.set_toast(format!("🗑 已删除任务及本地文件: {}", name));
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

    pub fn open_add_dialog(&mut self) {
        let prefill = ADD_POOL[(self.next_id as usize) % ADD_POOL.len()];
        // 默认并发数随预填 URL 的协议类型：HTTP 4 / BT 20（对话框中可修改）
        let (proto, _) = parse_url_task(prefill);
        self.dialog = Some(Dialog {
            kind: DialogKind::Add,
            url: prefill.to_string(),
            dir: "/srv/downloads".to_string(),
            conns: default_conns(proto).to_string(),
            conns_edited: false,
            // 校验算法默认 SHA-256，校验码留空 = 不校验
            ck_type: 3,
            ck_value: String::new(),
            ck_open: false,
            ck_sel: 3,
            proxy_sel: self.default_proxy_sel,
            proxy_open: false,
            focus: 0,
            task_name: String::new(),
            task_id: None,
        });
    }

    /// 打开修改任务对话框（v1.5/FR-01-87 同步）：四字段预填当前值；
    /// 已完成任务拒绝（D20，toast 提示）；无选中任务 toast 提示
    pub fn open_modify_dialog(&mut self) {
        let Some(t) = self.sel_task() else {
            self.set_toast("没有选中的任务");
            return;
        };
        if t.state == TaskState::Completed {
            self.set_toast("⚠ 已完成任务不可修改");
            return;
        }
        let (id, name, concurrency, checksum, proxy) = (
            t.id,
            t.name.clone(),
            t.concurrency,
            t.checksum.clone(),
            t.proxy.clone(),
        );
        let ck_type = CHECKSUM_ALGOS
            .iter()
            .position(|(n, _)| checksum.as_ref().is_some_and(|c| c.algo == *n))
            .unwrap_or(3);
        // 选项表未收录时追加兜底项保证回显一致（正常流都在表内）
        let proxy_sel = match self.proxy_options.iter().position(|o| o == &proxy) {
            Some(i) => i,
            None => {
                self.proxy_labels.push(match &proxy {
                    ProxyChoice::Named(n) => n.clone(),
                    ProxyChoice::Direct => "直连".to_string(),
                });
                self.proxy_options.push(proxy.clone());
                self.proxy_options.len() - 1
            }
        };
        self.dialog = Some(Dialog {
            kind: DialogKind::Modify,
            url: String::new(),
            dir: String::new(),
            conns: concurrency.to_string(),
            conns_edited: false,
            ck_type,
            ck_value: checksum.map(|c| c.value).unwrap_or_default(),
            ck_open: false,
            ck_sel: ck_type,
            proxy_sel,
            proxy_open: false,
            focus: 0,
            task_name: name,
            task_id: Some(id),
        });
    }

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
            proxy_sel: 0,
            proxy_open: false,
            focus: 0,
            task_name: t.name.clone(),
            task_id: None,
        });
    }

    // -----------------------------------------------------------------------
    // 鼠标交互：左键选中任务 / 对话框按钮，滚轮滚动列表
    // -----------------------------------------------------------------------

    pub fn on_mouse(&mut self, m: MouseEvent) {
        // 对话框优先：下拉框展开时只认选项点击（点外部收起）；否则字段行聚焦 / 按钮激活
        if self.dialog.is_some() {
            if let MouseEventKind::Down(MouseButton::Left) = m.kind {
                let kind = self.dialog.as_ref().map(|d| d.kind);
                let ck_open = self.dialog.as_ref().is_some_and(|d| d.ck_open);
                let proxy_open = self.dialog.as_ref().is_some_and(|d| d.proxy_open);
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
                        // 点击下拉框外部：仅收起下拉框
                        d.ck_open = false;
                    }
                    return;
                }
                // 代理下拉（v1.5/FR-01-86）：点击选项选择；点外关闭
                if proxy_open {
                    let proxy_rects = self.dlg_proxy_rects.clone();
                    if let Some(i) = hit(&proxy_rects) {
                        let d = self.dialog.as_mut().unwrap();
                        d.proxy_sel = i;
                        d.proxy_open = false;
                    } else if let Some(d) = self.dialog.as_mut() {
                        d.proxy_open = false;
                    }
                    return;
                }
                let field_rects = self.dlg_field_rects.clone();
                let btn_rects = self.dlg_btn_rects.clone();
                if let Some(i) = hit(&field_rects) {
                    // 点击输入/选择行：聚焦；下拉行（Add 校验=3/Modify 校验=1、
                    // 代理恒为末二：Add 5/Modify 3）再次点击展开下拉框
                    let ck_i = if kind == Some(DialogKind::Add) { 3 } else { 1 };
                    let proxy_i = if kind == Some(DialogKind::Add) { 5 } else { 3 };
                    let d = self.dialog.as_mut().unwrap();
                    d.focus = i;
                    if i == ck_i {
                        d.ck_open = true;
                        d.ck_sel = d.ck_type;
                    } else if i == proxy_i {
                        d.proxy_open = true;
                    }
                } else if let Some(btn) = hit(&btn_rects) {
                    if let Some(d) = self.dialog.as_mut() {
                        d.focus = btn;
                    }
                    match kind {
                        Some(DialogKind::Add) => self.dlg_activate_add(btn),
                        Some(DialogKind::Modify) => self.dlg_activate_modify(btn),
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

    /// bracketed paste 事件入口（FR-01-06 同步）：仅 Add 对话框的文本字段接收
    /// 粘贴；下拉框展开、删除/修改对话框、无对话框时忽略（避免粘贴触发按钮/导航）
    pub fn on_paste(&mut self, text: &str) {
        let Some(d) = self.dialog.as_mut() else {
            return;
        };
        if d.kind != DialogKind::Add || d.ck_open {
            return;
        }
        let focus = d.focus;
        dlg_apply_paste(d, focus, text);
    }

    // -----------------------------------------------------------------------
    // 任务操作
    // -----------------------------------------------------------------------

    pub fn toggle_pause(&mut self) {
        let Some(idx) = self.sel_idx() else { return };
        let name = self.tasks[idx].name.clone();
        match self.tasks[idx].state {
            TaskState::Downloading => {
                self.tasks[idx].state = TaskState::Paused;
                self.tasks[idx].speed = 0.0;
                self.tasks[idx].upload_speed = 0.0;
                self.tasks[idx].running_since = None;
                self.tasks[idx].has_slot = false; // 暂停后释放下载槽位
                for c in self.tasks[idx].connections.iter_mut() {
                    c.speed = 0.0;
                }
                let toast = if self.tasks[idx].resumable {
                    format!("⏸ 已暂停，释放下载槽位（断点已保存）: {}", name)
                } else {
                    format!(
                        "⏸ 已暂停（服务器不支持断点续传，继续时将从头下载）: {}",
                        name
                    )
                };
                self.set_toast(toast);
            }
            TaskState::Paused => {
                // 继续下载：需要空闲下载槽位；槽位已满时转入「等待中」队列排队
                if self.used_slots() >= MAX_DOWNLOAD_SLOTS {
                    let used = self.used_slots();
                    self.tasks[idx].state = TaskState::Queued;
                    self.tasks[idx].has_slot = false;
                    self.tasks[idx].queued_since = Some(Instant::now());
                    self.tasks[idx].start_delay = 1.5;
                    self.set_toast(format!(
                        "⏳ 无空闲下载槽位（{}/{}），已进入等待队列: {}",
                        used, MAX_DOWNLOAD_SLOTS, name
                    ));
                    return;
                }
                // 继续下载：从断点直接恢复传输（无「连接中」状态）；
                // 服务器不支持断点续传时，只能从头开始下载
                let restart = !self.tasks[idx].resumable;
                if restart {
                    let total = self.tasks[idx].total;
                    let n = self.tasks[idx].connections.len().max(1);
                    let piece = chunk_size(self.tasks[idx].protocol);
                    self.tasks[idx].downloaded = 0;
                    self.tasks[idx].chunk_done = 0;
                    self.tasks[idx].connections = make_chunk_conns(total, 0, n, piece).0;
                }
                self.tasks[idx].state = TaskState::Downloading;
                self.tasks[idx].has_slot = true;
                self.tasks[idx].queued_since = None;
                self.tasks[idx].running_since = Some(Instant::now());
                self.tasks[idx].made_progress = false; // 新尝试开始：清空进展标记
                let toast = if restart {
                    format!("⚠ 服务器不支持断点续传，已从头开始下载: {}", name)
                } else {
                    format!("▶ 继续下载（从断点恢复）: {}", name)
                };
                self.set_toast(toast);
            }
            TaskState::Queued => {
                // 等待中任务按 Space：暂停并退出等待队列（→ 已暂停）。
                // 已持有槽位的（重试排队）任务暂停后释放槽位，
                // 由 enforce_slots 分配给后续等待任务
                let held = self.tasks[idx].has_slot;
                self.tasks[idx].state = TaskState::Paused;
                self.tasks[idx].has_slot = false;
                self.tasks[idx].queued_since = None;
                self.tasks[idx].running_since = None;
                let toast = if held {
                    format!("⏸ 已暂停（退出等待队列，释放下载槽位）: {}", name)
                } else {
                    format!("⏸ 已暂停（退出等待队列）: {}", name)
                };
                self.set_toast(toast);
            }
            TaskState::Failed => {
                // 重新排队 → 已重试次数重置为 1，进入「等待中」队列；
                // 校验失败的任务断点无意义 → 清除断点从头下载（FR-M1-51）；
                // 原本持有槽位的（未达重试上限的）失败任务继续占用槽位
                self.requeue_failed(idx, false);
            }
            TaskState::Seeding => {
                // 做种中 → 暂停做种，进入「已完成」
                self.tasks[idx].state = TaskState::Completed;
                self.tasks[idx].upload_speed = 0.0;
                self.set_toast(format!("⏸ 已停止做种（进入已完成）: {}", name));
            }
            TaskState::Completed => self.set_toast("该任务已完成，无需操作".to_string()),
            _ => self.set_toast("当前状态下不可暂停/继续".to_string()),
        }
    }

    pub fn retry(&mut self) {
        let Some(idx) = self.sel_idx() else { return };
        if self.tasks[idx].state == TaskState::Failed {
            // 手动重试 → 已重试次数重置为 1，进入「等待中」队列（无「连接中」状态）；
            // 校验失败的任务清除断点从头下载（FR-M1-51）；
            // 原本持有槽位的（未达重试上限的）失败任务继续占用槽位，
            // 已达上限/停等释放槽位的任务重新排队后等待新的空闲槽位
            self.requeue_failed(idx, true);
        } else {
            self.set_toast("仅「已失败」的任务可以重试".to_string());
        }
    }

    /// 失败任务重新排队（R 手动重试 / Space）：重试计数重置为 1、回到等待队列；
    /// 校验失败（fail_kind = Verify）时断点无意义 → 清除断点从头下载（FR-M1-51）；
    /// 其余失败沿用断点续传（已下载块的字节保持有效）
    fn requeue_failed(&mut self, idx: usize, manual: bool) {
        let from_scratch = self.tasks[idx].fail_kind == Some(FailKind::Verify);
        if from_scratch {
            let total = self.tasks[idx].total;
            let n = self.tasks[idx].connections.len().max(1);
            let piece = chunk_size(self.tasks[idx].protocol);
            self.tasks[idx].downloaded = 0;
            self.tasks[idx].chunk_done = 0;
            self.tasks[idx].connections = make_chunk_conns(total, 0, n, piece).0;
        }
        self.tasks[idx].state = TaskState::Queued;
        self.tasks[idx].error = None;
        self.tasks[idx].retry_in = None;
        self.tasks[idx].fail_kind = None;
        self.tasks[idx].retries = 1;
        self.tasks[idx].queued_since = Some(Instant::now());
        self.tasks[idx].start_delay = 3.0;
        let name = self.tasks[idx].name.clone();
        let toast = if from_scratch {
            format!(
                "↻ {}（已清除断点，从头下载）: {}",
                if manual {
                    "手动重试"
                } else {
                    "重新排队"
                },
                name
            )
        } else {
            format!(
                "↻ {}（等待中）: {}",
                if manual {
                    "手动重试"
                } else {
                    "重新排队"
                },
                name
            )
        };
        self.set_toast(toast);
    }

    pub fn clear_completed(&mut self) {
        let before = self.tasks.len();
        self.tasks.retain(|t| t.state != TaskState::Completed);
        let n = before - self.tasks.len();
        if n > 0 {
            self.set_toast(format!("已清理 {} 个已完成任务", n));
        } else {
            self.set_toast("没有可清理的已完成任务".to_string());
        }
    }
}

/// 代理下拉框展开态逐键处理（v1.5/FR-01-86 同步；Up/Down/Home/End 选择，
/// Enter/Esc 关闭，Tab/BackTab 关闭并跳转焦点，其余键关闭）。消费所有按键。
fn proxy_dropdown_open_key(d: &mut Dialog, n: usize, code: KeyCode) {
    let n = n.max(1);
    match code {
        KeyCode::Up => {
            d.proxy_sel = (d.proxy_sel + n - 1) % n;
        }
        KeyCode::Down => {
            d.proxy_sel = (d.proxy_sel + 1) % n;
        }
        KeyCode::Home => d.proxy_sel = 0,
        KeyCode::End => d.proxy_sel = n - 1,
        KeyCode::Enter | KeyCode::Esc => d.proxy_open = false,
        KeyCode::Tab => {
            d.proxy_open = false;
            d.focus = if d.kind == DialogKind::Add { 6 } else { 4 };
        }
        KeyCode::BackTab => {
            d.proxy_open = false;
            d.focus = if d.kind == DialogKind::Add { 4 } else { 2 };
        }
        _ => d.proxy_open = false,
    }
}

/// 校验算法下拉框展开态逐键处理（Up/Down/Home/End 选择，Enter/Esc 关闭，
/// Tab/BackTab 关闭并跳转焦点，其余键关闭）。消费所有按键。
fn ck_dropdown_open_key(d: &mut Dialog, code: KeyCode) {
    match code {
        KeyCode::Up => {
            d.ck_sel = (d.ck_sel + CHECKSUM_ALGOS.len() - 1) % CHECKSUM_ALGOS.len();
            d.ck_type = d.ck_sel;
        }
        KeyCode::Down => {
            d.ck_sel = (d.ck_sel + 1) % CHECKSUM_ALGOS.len();
            d.ck_type = d.ck_sel;
        }
        KeyCode::Home => {
            d.ck_sel = 0;
            d.ck_type = 0;
        }
        KeyCode::End => {
            d.ck_sel = CHECKSUM_ALGOS.len() - 1;
            d.ck_type = d.ck_sel;
        }
        KeyCode::Enter | KeyCode::Esc => d.ck_open = false,
        KeyCode::Tab | KeyCode::BackTab => {
            d.ck_open = false;
            d.focus = if code == KeyCode::Tab { 4 } else { 2 };
        }
        _ => d.ck_open = false,
    }
}

/// 文本字段退格（kind 感知焦点映射：Add 0=URL 1=目录 2=并发 4=校验码；
/// Modify 0=并发 2=校验码）
fn text_backspace(kind: DialogKind, d: &mut Dialog, focus: usize) -> bool {
    match (kind, focus) {
        (DialogKind::Add, 0) => {
            d.url.pop();
            true
        }
        (DialogKind::Add, 1) => {
            d.dir.pop();
            true
        }
        (DialogKind::Add, 2) | (DialogKind::Modify, 0) => {
            d.conns.pop();
            d.conns_edited = true;
            true
        }
        (DialogKind::Add, 4) | (DialogKind::Modify, 2) => {
            d.ck_value.pop();
            true
        }
        _ => false,
    }
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
/// 校验码仅十六进制 ≤128 位；按钮焦点不接收。
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

// ---------------------------------------------------------------------------
// 工具函数
// ---------------------------------------------------------------------------

/// 从 URL 解析协议与文件名
fn parse_url_task(url: &str) -> (Protocol, String) {
    if let Some(rest) = url.strip_prefix("magnet:") {
        let dn = rest
            .split('&')
            .find_map(|p| p.strip_prefix("dn="))
            .filter(|s| !s.is_empty())
            .unwrap_or("bt-task");
        return (Protocol::Bt, dn.to_string());
    }
    let proto = if url.starts_with("https://") {
        Protocol::Https
    } else {
        Protocol::Http
    };
    let path = url.split("://").nth(1).unwrap_or(url);
    let seg = path
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('/')
        .find(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            // 均无法推导时用 download-<时间戳>（FR-M1-02）
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!("download-{}", ts)
        });
    (proto, seg)
}

/// 由文件名哈希推导伪大小（确定性）
fn pseudo_total(name: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    80_000_000 + h % 5_000_000_000
}

/// 按扩展名判断是否需要后期处理
fn is_archive(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".zip")
        || n.ends_with(".tar.gz")
        || n.ends_with(".tar.xz")
        || n.ends_with(".tar.zst")
        || n.ends_with(".tgz")
        || n.ends_with(".whl")
        || n.ends_with(".7z")
        || n.ends_with(".rar")
}

/// 指数退避序列（FR-M1-42）：8s → 16s → 32s → 60s 封顶
fn backoff_secs(retries: u32) -> f64 {
    match retries {
        1 => 8.0,
        2 => 16.0,
        3 => 32.0,
        _ => 60.0,
    }
}

/// 演示用失败场景轮换（按累计失败次数循环）——覆盖 FR-M1-40 的失败分类：
/// 网络错误（超时/重置）、HTTP 5xx（附状态码与 Retry-After）、文件大小不符、
/// 续传一致性失效；语义性 4xx 与磁盘错误经预检路径演示（FR-M1-44）、
/// SHA-256 校验失败走校验路径（FR-M1-51）。
/// 返回 (失败原因, 失败类别, Retry-After 秒)
fn fail_case(n: u32) -> (&'static str, FailKind, Option<f64>) {
    match n % 5 {
        1 => ("连接超时 (ETIMEDOUT)", FailKind::Transient, None),
        2 => ("连接被重置 (ECONNRESET)", FailKind::Transient, None),
        3 => (
            "文件大小不符（已接收 3.9 GB ≠ Content-Length 4.2 GB）",
            FailKind::Transient,
            None,
        ),
        4 => (
            "HTTP 503 Service Unavailable（Retry-After: 10s）",
            FailKind::Transient,
            Some(10.0),
        ),
        _ => (
            "续传一致性失效（ETag 已变化，服务器内容已更新）",
            FailKind::Invalidated,
            None,
        ),
    }
}

/// A 键对话框的示例 URL 池（预填，可直接回车添加；含一条 BT 磁力链演示默认并发 20）
const ADD_POOL: [&str; 6] = [
    "https://developer.download.nvidia.com/compute/cuda/12.8.0/cuda_12.8.0_linux.tar.xz",
    "https://dl.k8s.io/v1.32.2/kubernetes-server-linux-amd64.tar.gz",
    "https://huggingface.co/deepseek-ai/DeepSeek-R1-Distill-Qwen-7B/resolve/main/model-q4.gguf",
    "https://github.com/Kitware/CMake/releases/download/v3.31.6/cmake-3.31.6-linux-x86_64.tar.gz",
    "https://mirrors.edge.kernel.org/fedora/releases/42/Workstation/x86_64/iso/Fedora-Workstation-Live-x86_64-42.iso",
    "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=ubuntu-24.04.2-desktop-amd64.iso",
];

/// 新任务的默认最大并发数：HTTP 4 · BT 20（可在添加对话框中修改）
fn default_conns(p: Protocol) -> usize {
    if p.is_bt() {
        20
    } else {
        4
    }
}

// ---------------------------------------------------------------------------
// 伪造数据构造
// ---------------------------------------------------------------------------

/// 分块策略：块大小按协议写死（AIR2 口径）——HTTP 1 MB/块 · BT 256 KB/块，
/// 与并发数解耦；总块数 = ceil(total / 块大小)，末块不足整块时吸收余数。
const HTTP_CHUNK_SIZE: u64 = 1024 * 1024; // 1 MB
const BT_CHUNK_SIZE: u64 = 256 * 1024; // 256 KB

/// 协议对应的分块大小（字节）
fn chunk_size(p: Protocol) -> u64 {
    if p.is_bt() {
        BT_CHUNK_SIZE
    } else {
        HTTP_CHUNK_SIZE
    }
}

/// 分块大小的展示文字（固定常量直出，保证显示恰为 "1 MB" / "256 KB"）
pub fn chunk_size_label(p: Protocol) -> &'static str {
    if p.is_bt() {
        "256 KB"
    } else {
        "1 MB"
    }
}

/// 总块数 y = ceil(total / 块大小)；空文件无分块
fn chunk_total(total: u64, piece: u64) -> u32 {
    if total == 0 || piece == 0 {
        0
    } else {
        ((total + piece - 1) / piece) as u32
    }
}

/// 权重表：把一段剩余字节散落到多个连接，呈现真实的非均匀进度观感
const CONN_WEIGHTS: [f64; 16] = [
    0.95, 0.45, 0.10, 0.00, 0.75, 0.40, 0.85, 0.15, 0.55, 0.20, 0.55, 0.00, 0.70, 0.25, 0.80, 0.35,
];

/// 按权重把 `amount` 字节拆成 `n` 份（钳制到 [0, cap_i]，差额回补，保证总和精确）
fn spread_bytes(amount: u64, n: usize, cap: u64) -> Vec<u64> {
    let mut v = vec![0u64; n];
    if amount == 0 || n == 0 {
        return v;
    }
    let wsum: f64 = (0..n).map(|i| CONN_WEIGHTS[i % 16]).sum();
    let mut assigned = 0u64;
    for i in 0..n {
        let raw = (amount as f64 * (CONN_WEIGHTS[i % 16] / wsum)).round() as u64;
        let d = raw.min(cap);
        v[i] = d;
        assigned += d;
    }
    let mut diff = amount as i64 - assigned as i64;
    let mut k = 0;
    while diff != 0 && k < 8 * n {
        let i = k % n;
        if diff > 0 && v[i] < cap {
            let add = (diff as u64).min(cap - v[i]);
            v[i] += add;
            diff -= add as i64;
        } else if diff < 0 && v[i] > 0 {
            let sub = ((-diff) as u64).min(v[i]);
            v[i] -= sub;
            diff += sub as i64;
        }
        k += 1;
    }
    v
}

/// 分块队列：为任务构建"当前持块"的连接列表（HTTP 与 BT 同模型，仅块大小不同）。
///
/// 策略：
/// - 块大小按协议写死（HTTP 1 MB / BT 256 KB），块数 y = ceil(total / 块大小)，
///   与并发数解耦；
/// - 连接从块队列 [x, x+active) 连续领块，完成一块立即领取下一块；
/// - 实际并发 = min(最大并发, 未完成块数)，队列临近结束时连接自然转入待命；
/// - done_total ≥ total（校验/完成/做种）时展示最后 min(n, y) 个满块。
///
/// 返回 (连接列表, 已完成块数 x)
fn make_chunk_conns(total: u64, done_total: u64, n: usize, piece: u64) -> (Vec<Connection>, u32) {
    let y = chunk_total(total, piece);
    if y == 0 {
        return (Vec::new(), 0);
    }
    let y64 = y as u64;
    let piece_end = |i: u64| if i + 1 == y64 { total } else { (i + 1) * piece };
    let done = done_total.min(total);
    let (x, lease_from, active, rem) = if done >= total {
        let a = (n.max(1) as u64).min(y64);
        (y64, y64 - a, a, 0u64)
    } else {
        let x = done / piece;
        let a = (n.max(1) as u64).min(y64 - x);
        (x, x, a, done - x * piece)
    };
    let partials = spread_bytes(rem, active as usize, piece);
    let conns = (0..active as usize)
        .map(|i| {
            let idx = lease_from + i as u64;
            let start = idx * piece;
            let end = piece_end(idx);
            Connection {
                start,
                end,
                done: if done >= total {
                    end - start
                } else {
                    partials[i].min(end - start)
                },
                speed: 0.0,
            }
        })
        .collect();
    (conns, x as u32)
}

#[allow(clippy::too_many_arguments)]
fn mk(
    id: u32,
    name: &str,
    protocol: Protocol,
    url: &str,
    path: &str,
    total: u64,
    state: TaskState,
    done: u64,
    conns: usize,
    base_speed: f64,
    created: &str,
) -> Task {
    let piece = chunk_size(protocol);
    let (connections, chunk_done) = make_chunk_conns(total, done, conns, piece);
    let downloaded = done.min(total);
    Task {
        id,
        name: name.to_string(),
        protocol,
        url: url.to_string(),
        save_path: path.to_string(),
        total,
        downloaded,
        speed: 0.0,
        base_speed,
        resumable: true,
        concurrency: conns,
        connections,
        chunk_done,
        seeders: 0,
        peers: 0,
        upload_speed: 0.0,
        uploaded: 0,
        seed_left: 0.0,
        post_process: false,
        post_started: None,
        retries: 0,
        max_retries: 5,
        fail_count: 0,
        made_progress: false,
        retry_in: None,
        fail_kind: None,
        flaky: false,
        verify_fail_once: false,
        precheck_fail_once: false,
        running_since: None,
        state,
        start_delay: 2.0,
        queued_since: None,
        has_slot: matches!(state, TaskState::Downloading),
        verify_started: None,
        error: None,
        checksum: None,
        verify_ok: None,
        proxy: ProxyChoice::Direct,
        created: created.to_string(),
    }
}

fn demo_tasks() -> Vec<Task> {
    let now = Instant::now();
    let mut v = vec![
        mk(
            1,
            "ubuntu-24.04.2-desktop-amd64.iso",
            Protocol::Https,
            "https://releases.ubuntu.com/24.04/ubuntu-24.04.2-desktop-amd64.iso",
            "/srv/downloads/ubuntu-24.04.2-desktop-amd64.iso",
            6_210_000_000,
            TaskState::Downloading,
            2_810_000_000,
            8,
            14_500_000.0,
            "2026-02-10 09:32",
        ),
        // 示例：HTTP 服务器不支持断点续传（仅能单线程下载，继续/重试只能从头开始）
        mk(
            2,
            "media-assets-2026-pack.zip",
            Protocol::Http,
            "http://cdn.example-media.net/get.php?file=media-assets-2026-pack.zip",
            "/srv/downloads/media-assets-2026-pack.zip",
            812_000_000,
            TaskState::Downloading,
            316_000_000,
            1,
            2_400_000.0,
            "2026-02-10 10:26",
        ),
        mk(
            3,
            "archlinux-x86_64-2026.02.01.iso",
            Protocol::Bt,
            "magnet:?xt=urn:btih:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b&dn=archlinux",
            "/srv/downloads/archlinux-x86_64-2026.02.01.iso",
            4_320_000_000,
            TaskState::Seeding,
            4_320_000_000,
            1,
            0.0,
            "2026-02-08 21:04",
        ),
        mk(
            4,
            "rust-toolchain-nightly.tar.zst",
            Protocol::Https,
            "https://static.rust-lang.org/dist/2026-02-09/rust-nightly-x86_64-unknown-linux-gnu.tar.zst",
            "/srv/downloads/rust-nightly.tar.zst",
            1_240_000_000,
            // 等待中：初始 5 个下载槽位已被占满（ubuntu/media/llama3/sintel/win11），
            // 位于队首，槽位空出后第一个获得槽位并从断点（421MB）继续下载
            TaskState::Queued,
            421_000_000,
            4,
            5_800_000.0,
            "2026-02-10 10:02",
        ),
        mk(
            5,
            "blender-4.5-linux-x64.tar.xz",
            Protocol::Http,
            "https://download.blender.org/release/Blender4.5/blender-4.5-linux-x64.tar.xz",
            "/srv/downloads/blender-4.5.tar.xz",
            328_000_000,
            TaskState::Paused,
            123_000_000,
            4,
            8_600_000.0,
            "2026-02-10 08:47",
        ),
        mk(
            6,
            "llama3-8b-instruct-q4_k_m.gguf",
            Protocol::Https,
            "https://huggingface.co/meta-llama/Meta-Llama-3-8B-Instruct/resolve/main/llama3-8b-q4_k_m.gguf",
            "/srv/downloads/llama3-8b-q4_k_m.gguf",
            4_920_000_000,
            TaskState::Downloading,
            601_000_000,
            16,
            9_200_000.0,
            "2026-02-10 09:58",
        ),
        mk(
            7,
            "imagenet-mini-dataset.zip",
            Protocol::Http,
            "http://datasets.example-mirror.com/vision/imagenet-mini.zip",
            "/srv/downloads/imagenet-mini.zip",
            2_260_000_000,
            TaskState::Queued,
            0,
            8,
            6_000_000.0,
            "2026-02-10 10:15",
        ),
        mk(
            8,
            "ffmpeg-master-amd64.AppImage",
            Protocol::Https,
            "https://github.com/eugeneware/ffmpeg-static/releases/latest/ffmpeg-amd64",
            "/srv/downloads/ffmpeg-master-amd64.AppImage",
            96_400_000,
            TaskState::Completed,
            96_400_000,
            1,
            0.0,
            "2026-02-09 14:20",
        ),
        mk(
            9,
            "godot-4.4-stable-linux.tar.gz",
            Protocol::Http,
            "https://github.com/godotengine/godot/releases/download/4.4-stable/godot-4.4-stable-linux.tar.gz",
            "/srv/downloads/godot-4.4.tar.gz",
            61_800_000,
            TaskState::Completed,
            61_800_000,
            1,
            0.0,
            "2026-02-09 16:02",
        ),
        mk(
            10,
            "win11-24h2-x64.iso",
            Protocol::Https,
            "https://dl.example-microsoft.com/software-download/win11-24h2-x64.iso",
            "/srv/downloads/win11-24h2-x64.iso",
            5_710_000_000,
            TaskState::Failed,
            2_160_000_000,
            8,
            11_000_000.0,
            "2026-02-10 07:12",
        ),
        // 排队任务：tensorflow 首次获得槽位开始前磁盘空间预检失败一次
        //（FR-M1-44 演示；排在 gpt4all 之前保证首屏可见）
        mk(
            12,
            "tensorflow-cuda-12.8.whl",
            Protocol::Http,
            "http://pypi.example-mirror.org/packages/tf/tensorflow-cuda-12.8-cp312.whl",
            "/srv/downloads/tensorflow-cuda-12.8.whl",
            712_000_000,
            TaskState::Queued,
            0,
            4,
            5_000_000.0,
            "2026-02-10 10:20",
        ),
        mk(
            11,
            "gpt4all-models-bundle.bin",
            Protocol::Https,
            "https://gpt4all.example-models.io/models/gpt4all-models-bundle.bin",
            "/srv/downloads/gpt4all-models-bundle.bin",
            3_340_000_000,
            TaskState::Verifying,
            3_340_000_000,
            8,
            0.0,
            "2026-02-10 10:18",
        ),
        mk(
            13,
            "neovim-0.11.2-linux-x86_64.tar.gz",
            Protocol::Https,
            "https://github.com/neovim/neovim/releases/download/v0.11.2/neovim-0.11.2-linux-x86_64.tar.gz",
            "/srv/downloads/neovim-0.11.2-linux-x86_64.tar.gz",
            23_400_000,
            TaskState::PostProcessing,
            23_400_000,
            2,
            0.0,
            "2026-02-10 10:21",
        ),
        mk(
            14,
            "sintel-4k-2160p-x265.mkv",
            Protocol::Bt,
            "magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1366a6b6&dn=sintel-4k",
            "/srv/downloads/sintel-4k-2160p-x265.mkv",
            4_190_000_000,
            TaskState::Downloading,
            1_280_000_000,
            12,
            2_800_000.0,
            "2026-02-10 09:47",
        ),
    ];

    // 示例任务：HTTP 不支持断点续传
    v[1].resumable = false;

    // 示例任务：ubuntu 经「办公网代理」下载（v1.5/FR-01-86 UI 同步演示：
    // m 修改对话框代理字段可回显命名代理）
    v[0].proxy = ProxyChoice::Named("办公网代理".to_string());

    // BT 做种任务附加信息（做种剩余 30 分钟，用于演示倒计时）
    v[2].seeders = 12;
    v[2].peers = 35;
    v[2].upload_speed = 1_900_000.0;
    v[2].uploaded = 1_712_000_000;
    v[2].seed_left = 1_800.0;
    v[2].verify_ok = Some(true);
    // arch（做种中）：SHA-1 校验通过（展示多算法校验结果）
    v[2].checksum = Some(Checksum {
        algo: "SHA-1",
        value: "a9993e364706816aba3e25717850c26c9cd0d89d".to_string(),
    });
    // media（HTTP 不支持续传）：SHA-224，完成后校验
    v[1].checksum = Some(Checksum {
        algo: "SHA-224",
        value: "d14a028c2a3a2bc9476102bb288234c415a2b01f828ea62ac5b3e42f".to_string(),
    });

    // BT 下载中任务附加信息
    v[13].seeders = 8;
    v[13].peers = 23;
    v[13].upload_speed = 420_000.0;
    v[13].uploaded = 96_000_000;

    // 失败任务：校验失败 + 倒计时自动重试 + 周期性再失败（演示状态流转）。
    // 失败原因按 fail_case 轮换：网络错误 → 重置 → 大小不符 → 503(Retry-After) →
    // 续传一致性失效（断点作废从头下载）；
    // 重试计数真实递增：未达上限时自动重试（指数退避 8→16→32→60s）并继续占用槽位；
    // 达到上限后停止自动重试并释放槽位（按 R 手动重试重置为 1）
    v[9].error = Some(fail_case(1).0.to_string());
    v[9].retries = 1;
    v[9].fail_count = 1;
    v[9].retry_in = Some(8.0); // backoff_secs(1)：退避序列第一档
    v[9].fail_kind = Some(FailKind::Transient);
    v[9].flaky = true;

    // 校验中任务启动计时（带 SHA-256 校验码，完成后展示「SHA-256 校验通过」）；
    // 演示：首次校验失败（FR-M1-51）——不自动重试，R 清除断点从头下载。
    // 计时推后 8s：首屏保持「校验中」可见，10.6s 后转「已失败（不自动重试）」
    //（gpt4all 交换到 v[11]：让 tensorflow 排在前保证首屏可见排队行）
    v[11].verify_started = Some(now + Duration::from_secs(8));
    v[11].verify_fail_once = true;
    v[11].checksum = Some(Checksum {
        algo: "SHA-256",
        value: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
    });
    // 排队任务：tensorflow 首次获得槽位开始前磁盘空间预检失败一次
    //（FR-M1-44 演示：预检失败释放槽位、不自动重试，R 手动重试后预检通过）
    v[10].precheck_fail_once = true;
    // 后期处理中任务（进行到一半）
    v[12].post_started = Some(now - Duration::from_millis(1200));
    v[12].verify_ok = Some(true);
    v[12].checksum = Some(Checksum {
        algo: "SHA-512",
        value: "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce".to_string()
            + "47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e",
    });
    v[12].post_process = true;

    // 排队任务入队计时（获得空闲槽位后才开始下载，演示槽位队列流转）
    v[3].queued_since = Some(now);
    v[6].queued_since = Some(now);
    v[10].queued_since = Some(now);
    // 压缩包类任务完成后进入后期处理
    v[2].post_process = true;
    v[3].post_process = true;
    v[6].post_process = true;
    v[11].post_process = true;

    // 已完成任务：godot MD5 校验通过；ffmpeg 无校验（verify_ok 保持 None）
    v[8].verify_ok = Some(true);
    v[8].checksum = Some(Checksum {
        algo: "MD5",
        value: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
    });

    v
}
