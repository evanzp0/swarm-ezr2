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
//! 失败与重试规则（FR-M1-40~44 / 51 / 22；R 语义已同步生产 v1.16/v1.17）：
//! - 自动重试类（网络错误 / HTTP 408·429·5xx / 文件大小不符）：指数退避
//!   8s → 16s → 32s → 60s 封顶；响应带 Retry-After（≤60s）时优先采用；
//! - 不自动重试类（语义性 HTTP 4xx / 磁盘空间不足 / SHA-256 校验失败）：
//!   直接停等（「已达上限」式），仅可 R 手动重试；
//!   校验失败按 R = 重新校验（无块重传，D10；生产 v1.17/FR-01-51 同步——
//!   原演示口径「清除断点从头重下」已废止）；
//!   校验码已清空（修改对话框清码）时按 R = 不校验：文件完整性直判不经
//!   引擎零排队零重传直接收尾「已完成」显示「无校验」
//!   （生产 v1.18/FR-01-51 + D34 钦定同步）；
//! - 速度展示面（v1.15/FR-01-102 同步）：连接/任务/全局速度同一管线——
//!   数据面逐帧推进窗口字节，展示面每秒采样 + EMA α=1/5，数值每秒最多
//!   变化一次；非下载态逐帧零值速断，下载中任务的待命连接采样点归零
//!   （D33：最坏 1s 时延）。
//! - 续传一致性失效（服务器内容已更新）：断点作废、清零进度、从头重新下载；
//! - 重试计数连续性：连续失败（无进展）→ 累加、有进展 → 重置为 1；
//!   达上限（默认 5）→ 停止自动重试、释放槽位、显示「已达上限」。
//!
//! 失败任务空格暂停（生产 v1.6/FR-01-92 + v1.19 同步 + v1.22/v1.23 展示与数据统一）：失败任务按空格 =
//! 暂停（挂起自动重试）——清倒计时、释放槽位、转「已暂停」并即时重置 error/retries（v1.23：
//! 数据层与普通暂停完全一致）；再次按空格 = 恢复重新排队（断点续传，校验失败型走重新校验路径）。
//! R 仅适用「已失败」态，空格为挂起态唯一恢复键
//! （v1.19 收窄；挂起态按 R 提示「仅『已失败』的任务可以重试」）。
//!
//! 下载槽位：同时可下载的任务数写死为 5（MAX_DOWNLOAD_SLOTS）。
//! 「下载中」与「已失败（未达重试上限、将自动重试）」的任务占用槽位；
//! 「等待中」任务须等槽位空出，按列表顺序（从上往下）依次获得槽位后开始下载。
//! 开始/续传前进行磁盘空间预检（演示：tensorflow 首次获得槽位时预检失败一次）。

use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::scrollbar;

/// 每个任务在列表中占用的行高（3 行内容 + 1 行空行分隔）
pub const ITEM_HEIGHT: u16 = 4;

// ---------------------------------------------------------------------------
// 通用滚动条交互状态（§4.1.3：滑块拖拽 / 箭头点击·按住连发 / 轨道翻页）
// ---------------------------------------------------------------------------

/// 通用滚动条标识——每个使用滚动条的 panel/弹窗一枚；行为（步进/翻页/写回
/// 哪个滚动状态）由 [`App`] 的 sb_* 方法按此分发，全部实例同一交互契约。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SbKey {
    /// 任务列表（垂直）
    List,
    /// 并发连接面板（垂直）
    ConnsV,
    /// 并发连接面板（水平，m3 新增）
    ConnsH,
    /// pieces 分块弹窗（垂直）
    Pieces,
    /// 帮助窗口（垂直）
    Help,
    /// 任务详情字段值弹窗（垂直；本轮需求修订新增）
    Val,
}

impl SbKey {
    /// 箭头单击/按住连发的步长（内容单位：任务条目/连接行/内容列/网格行/文本行；
    /// 水平以 2 列为单位——单列步进肉眼难辨）
    pub fn step(self) -> i32 {
        match self {
            SbKey::ConnsH => 2,
            _ => 1,
        }
    }

    fn bar(&self, app: &App) -> Option<scrollbar::Bar> {
        match self {
            SbKey::List => app.list_sb,
            SbKey::ConnsV => app.conns_v_sb,
            SbKey::ConnsH => app.conns_h_sb,
            SbKey::Pieces => app.pieces_sb,
            SbKey::Help => app.help_sb,
            SbKey::Val => app.val_sb,
        }
    }
}

/// 箭头/轨道按压连发状态：按下即触发一次，随后 first_delay 起振、
/// repeat_interval 周期重复（App::tick 驱动；Up 取消）
#[derive(Clone, Copy, Debug)]
pub struct SbHold {
    pub key: SbKey,
    /// 方向：-1 = 朝头箭头（↑/←），+1 = 朝尾箭头（↓/→）
    pub dir: i32,
    /// true = 轨道翻页（按 page 步长），false = 箭头步进（按 step 步长）
    pub page: bool,
    pub since: Instant,
    pub last: Instant,
}

impl SbHold {
    /// 首次重复的延迟（按住起振时间）
    const FIRST_DELAY: Duration = Duration::from_millis(400);
    /// 后续重复周期
    const REPEAT: Duration = Duration::from_millis(120);

    fn due(&self) -> Instant {
        if self.since == self.last {
            self.since + Self::FIRST_DELAY
        } else {
            self.last + Self::REPEAT
        }
    }
}

/// 列表页签（仅两个）
pub const FILTERS: [&str; 2] = ["正在下载", "已完成"];

/// 下载槽位上限（demo 写死）：「正在下载」中同时可下载的任务数。
/// 等待中任务须等槽位空出，按列表顺序（从上往下）依次获得槽位后开始下载。
pub const MAX_DOWNLOAD_SLOTS: usize = 5;

/// braille 转轮字符（校验中/后期处理等动态效果）
const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// 速度展示面采样节拍（v1.15/FR-01-102 同步）：1s 窗口 + 每秒采样 +
/// EMA α=1/5——数值每秒最多变化一次，与生产任务/连接速度同一管线
const SPEED_TICK: Duration = Duration::from_secs(1);
/// 展示面 EMA 平滑系数（α = 1/5，与生产 SpeedDisplay 同口径）
const SPEED_EMA_ALPHA: f64 = 0.2;

// ---------------------------------------------------------------------------
// 面板焦点（lazygit 式聚焦显示）
// ---------------------------------------------------------------------------

/// 可聚焦面板：Tab/BackTab 循环切换；当前面板边框绿色高亮、
/// 其余面板暗边框（lazygit activeBorderColor/inactiveBorderColor 口径）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Panel {
    /// 任务列表（头部页签随本面板聚焦而高亮）
    List,
    /// 任务详情
    Detail,
    /// 并发连接
    Conns,
}

impl Default for Panel {
    fn default() -> Self {
        Panel::List
    }
}

/// 任务详情可弹窗查看完整值的字段（本轮需求修订：点击字段值打开小窗口，
/// 字段名点击仍是复制）。字段与详情面板蓝色下划线链接字段一一对应。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DetailField {
    /// 校验（完整校验码）
    Checksum,
    /// URL（完整下载地址）
    Url,
    /// 失败（完整错误文案；字段名原「失败原因」）
    Failed,
    /// 路径（完整保存路径；字段名原「保存」，本轮需求修订再简化）
    SavePath,
}

impl DetailField {
    /// 弹窗标题（= 详情面板字段名）
    pub fn title(self) -> &'static str {
        match self {
            DetailField::Checksum => "校验",
            DetailField::Url => "URL",
            DetailField::Failed => "失败",
            DetailField::SavePath => "路径",
        }
    }

    /// 复制 toast 文案（与字段名点击复制同口径）
    pub fn copy_label(self) -> &'static str {
        match self {
            DetailField::Checksum => "校验码",
            DetailField::Url => "url",
            DetailField::Failed => "失败原因",
            DetailField::SavePath => "保存路径",
        }
    }
}

impl Panel {
    /// Tab：下一个面板（列表 → 详情 → 并发 → 列表）
    pub fn next(self) -> Self {
        match self {
            Panel::List => Panel::Detail,
            Panel::Detail => Panel::Conns,
            Panel::Conns => Panel::List,
        }
    }

    /// BackTab：上一个面板
    pub fn prev(self) -> Self {
        match self {
            Panel::List => Panel::Conns,
            Panel::Detail => Panel::List,
            Panel::Conns => Panel::Detail,
        }
    }
}

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

    /// 是否显示并发连接明细：仅「下载中」与「做种中」任务持有活跃连接。
    /// 其余状态（等待中/已暂停/校验中/后期处理中/已失败/已完成）无并发明细，
    /// 面板为空、活跃 0/0（演示口径：连接随任务开始下载建立、随任务离开
    /// 下载/做种状态断开；面板数据在状态下沉时冻结隐藏，恢复下载时原样续用，
    /// 保证暂停/断点续传的累计量口径不受影响）。
    pub fn shows_conns(&self) -> bool {
        matches!(self, TaskState::Downloading | TaskState::Seeding)
    }
}

/// 分块（piece）四态——pieces 弹窗（P 快捷键）展示口径，色块字符与状态一一对应：
/// Empty ⬜ 未下载 · Done 🟩 已下载 · Partial 🟨 部分下载 · Active 🟦 正在下载。
/// Active 仅当任务处于「下载中」且该块已分配并发连接时出现（「分配了并发下载」
/// 即算，块内零头多少不影响）；同一块「部分下载」且「正在下载」时只显示
/// Active（🟦 优先于 🟨）——任务非「下载中」时任何块都不会出现 🟦。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PieceState {
    /// 未下载（⬜）
    Empty,
    /// 已下载（🟩）
    Done,
    /// 部分下载（🟨）：块内有已落盘零头，但未分配并发下载
    /// （暂停/等待/失败等状态下沉时冻结的持块连接）
    Partial,
    /// 正在下载（🟦）：已分配并发连接（仅「下载中」任务）
    Active,
}

/// BT 对端传输协议（m3 并发面板新增「协议」列；演示数据确定性生成）：
/// TCP = 传统直连；uTP = 微传输协议（BT over UDP，拥塞友好）
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Tcp,
    Utp,
}

impl Transport {
    pub fn label(self) -> &'static str {
        match self {
            Transport::Tcp => "TCP",
            Transport::Utp => "uTP",
        }
    }
}

/// 演示用对端客户端缩写池（两个大写字母，并发面板「客户端」列）：
/// qBittorrent / Transmission / libtorrent / µTorrent / Deluge / aria2 /
/// rTorrent / Tixati / Azureus(Vuze) / Xunlei
const BT_CLIENTS: [&str; 10] = ["QB", "TR", "LT", "UT", "DE", "AR", "RT", "TX", "AZ", "XL"];

/// 由 (任务种子, 连接槽位) 确定性生成演示用对端客户端缩写（两字母）
fn pseudo_client(seed: u64, idx: usize) -> &'static str {
    let mix = |mut x: u64| {
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
        x ^= x >> 33;
        x
    };
    let h = mix(seed ^ mix((idx as u64 + 3).wrapping_mul(0x9E37_79B9_7F4A_7C15)));
    BT_CLIENTS[(h % BT_CLIENTS.len() as u64) as usize]
}

/// 由 (任务种子, 连接槽位) 确定性生成 BT 传输协议：约 1/3 uTP（BT over UDP），
/// 其余 TCP 直连（与客户端缩写用不同常数混排，避免两列强相关）
fn pseudo_transport(seed: u64, idx: usize) -> Transport {
    let mix = |mut x: u64| {
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
        x ^= x >> 33;
        x
    };
    let h = mix(seed ^ mix((idx as u64 + 11).wrapping_mul(0x9E37_79B9_7F4A_7C15)));
    if h % 3 == 0 {
        Transport::Utp
    } else {
        Transport::Tcp
    }
}

/// 单个并发连接（分块）状态：一段连续的 byte range 由一个线程负责
#[derive(Clone)]
pub struct Connection {
    /// 对端地址（演示数据：BT 任务界面以掩码显示首末段，HTTP 不显示）
    pub ip: String,
    /// 对端客户端缩写（两个大写字母；m3 并发面板新增列，BT 演示数据）
    pub client: &'static str,
    /// BT 传输协议（TCP / uTP；m3 并发面板新增列，HTTP 不显示）
    pub transport: Transport,
    /// 当前采样窗口内累计的下载字节（展示面每秒采样后清零；v1.15/FR-01-102）
    pub win: u64,
    /// 当前采样窗口内累计的上传字节（展示面每秒采样后清零；BT）
    pub win_up: u64,
    /// 分块起始字节（含）
    pub start: u64,
    /// 分块结束字节（不含）
    pub end: u64,
    /// 已下载字节
    pub done: u64,
    /// 当前速度 B/s
    pub speed: f64,
    /// 当前上传速度 B/s（演示：BT 任务边下边传按连接均摊，HTTP 恒 0）
    pub up_speed: f64,
    /// 本次开始下载后的累计下载字节（重新排队/重试开新一次下载时清零，
    /// 断点续传的已完成量不受影响；块完成后领新块也不清零）
    pub cum_down: u64,
    /// 本次开始下载后的累计上传字节
    pub cum_up: u64,
}

impl Connection {
    pub fn cap(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// 对端地址掩码显示（隐私口径）：IPv4「首段.*.末端」、IPv6「首段:*:末端」
    pub fn ip_masked(&self) -> String {
        mask_ip(&self.ip)
    }
}

/// 对端地址掩码：IPv4 a.b.c.d → 「a.*.d」，IPv6 g0:g1:…:g7 → 「g0:*:g7」
/// （只保留首末段、中间段以 * 隐藏，兼顾可辨识与隐私）
fn mask_ip(ip: &str) -> String {
    if ip.contains(':') {
        let first = ip.split(':').next().unwrap_or("");
        let last = ip.rsplit(':').next().unwrap_or("");
        format!("{}:*:{}", first, last)
    } else {
        let first = ip.split('.').next().unwrap_or("");
        let last = ip.rsplit('.').next().unwrap_or("");
        format!("{}.*.{}", first, last)
    }
}

/// 由 (任务种子, 连接槽位) 确定性生成演示用对端地址：约 1/5 为 IPv6，其余 IPv4。
/// 同一任务同一槽位的地址稳定（重建分块/断点续传时同槽位对端不变，演示口径）。
fn pseudo_ip(seed: u64, idx: usize) -> String {
    let mix = |mut x: u64| {
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
        x ^= x >> 33;
        x
    };
    let h = mix(seed ^ mix((idx as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)));
    if h % 5 == 0 {
        // IPv6：全形 8 组 4 位十六进制（如 2001:0db8:…:7334，不做 :: 压缩，
        // 便于按「首段:*:末端」掩码显示）
        let g = |k: u64| (mix(h ^ k.wrapping_mul(0x9E37_79B9_7F4A_7C15)) & 0xffff) as u16;
        (0..8)
            .map(|k| format!("{:04x}", g(k)))
            .collect::<Vec<_>>()
            .join(":")
    } else {
        // IPv4：首段 1..=223（避开 0/组播/保留段），末段 1..=254
        let o = |k: u64, m: u64| mix(h ^ k.wrapping_mul(0x9E37_79B9_7F4A_7C15)) % m;
        format!(
            "{}.{}.{}.{}",
            1 + o(1, 223),
            o(2, 256),
            o(3, 256),
            1 + o(4, 254)
        )
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

    /// 分块信息 (x=已完成块数, y=总块数, n=块大小字节)：
    /// 块大小按协议写死（HTTP 1 MB / BT 256 KB），块数 = ceil(total/块大小)，
    /// 与并发数解耦；x 由 tick 维护（分块队列动态领块，HTTP 与 BT 同模型）
    pub fn chunk_info(&self) -> (u32, u32, u64) {
        let piece = chunk_size(self.protocol);
        let y = chunk_total(self.total, piece);
        (self.chunk_done.min(y), y, piece)
    }

    /// 是否支持分块下载（pieces 弹窗 P 快捷键的功能门槛）：
    /// BT 恒分块；HTTP(S) 需服务器支持断点续传（Range）——不支持续传时只能
    /// 单线程整段下载、无分块概念；FTP 同不支持。
    pub fn supports_pieces(&self) -> bool {
        if self.protocol.is_bt() {
            return true;
        }
        !matches!(self.protocol, Protocol::Ftp) && self.resumable
    }

    /// 全量分块状态（pieces 弹窗展示口径，块序 = 字节序）。
    ///
    /// 口径（与 [`PieceState`] 文档一致）：
    /// - 块号 < chunk_done → Done（已下载）；
    /// - 块被并发连接持有（分块队列按块边界整块领块，连接 start 恒对齐块边界）：
    ///   任务「下载中」→ Active（分配了并发下载；即使块内进度为 0 也算——
    ///   「部分下载」且「正在下载」时只显示 Active）；任务非「下载中」
    ///   （暂停/等待/失败等状态下沉冻结的持块连接）→ 有零头 Partial、
    ///   零头为 0 → Empty；
    /// - 其余 → Empty（未下载）。
    ///
    /// 空/极小文件：y=0（无分块）返回空表。
    pub fn pieces(&self) -> Vec<PieceState> {
        let piece = chunk_size(self.protocol);
        let y = chunk_total(self.total, piece) as usize;
        if y == 0 {
            return Vec::new();
        }
        let mut v = vec![PieceState::Empty; y];
        let x = (self.chunk_done as usize).min(y);
        for s in v.iter_mut().take(x) {
            *s = PieceState::Done;
        }
        let downloading = self.state == TaskState::Downloading;
        for c in &self.connections {
            if c.cap() == 0 {
                continue; // 待命连接（start=end=0）不持块
            }
            let idx = (c.start / piece) as usize;
            if idx >= y {
                continue; // 防御：越界租约不计
            }
            v[idx] = if c.done >= c.cap() {
                PieceState::Done
            } else if downloading {
                PieceState::Active
            } else if c.done > 0 {
                PieceState::Partial
            } else {
                PieceState::Empty
            };
        }
        v
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

// ---------------------------------------------------------------------------
// 任务 ID 随机分配（v1.20 生产同步：u32 随机数 + 任务全集查重、0 保留，
// 取代顺序自增 next_id；与模拟器 rng 隔离——独立熵源不扰动伪造数据序列；
// 口径与唯一性论证同生产 model::idgen）
// ---------------------------------------------------------------------------

/// 进程内调用序号（同纳秒多次调用熵隔离）
static ID_SEQ: AtomicU32 = AtomicU32::new(0);

/// SplitMix64 黄金比例增量常数
const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

/// 分配一个与既有任务不冲突的随机任务 ID（SplitMix64；种子 = 系统时钟纳秒 ^
/// 进程号 ^ 栈地址 ^ 调用序号；0 保留哨兵，冲突自动重试）
fn alloc_task_id(tasks: &[Task]) -> u32 {
    loop {
        let stack_probe = 0u8;
        // 取引用地址仅作熵源（不解引用；安全转换）
        let addr = &stack_probe as *const u8 as usize as u64;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let seq = ID_SEQ.fetch_add(1, Ordering::Relaxed) as u64;
        let mut g =
            (nanos ^ ((u64::from(std::process::id())) << 32) ^ addr ^ seq.wrapping_mul(GOLDEN)) | 1;
        g = g.wrapping_add(GOLDEN);
        let mut z = g;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        let cand = ((z ^ (z >> 31)) >> 32) as u32;
        let cand = if cand == 0 { 1 } else { cand };
        if !tasks.iter().any(|t| t.id == cand) {
            return cand;
        }
    }
}

/// 校验算法表（添加对话框下拉选择）：（显示名，期望十六进制长度）。
/// 首行 NONE = 不校验（期望长度 0，仅作下拉占位显示）；其余算法校验码必填。
pub const CHECKSUM_ALGOS: [(&str, usize); 8] = [
    ("NONE", 0),
    ("MD5", 32),
    ("SHA-1", 40),
    ("SHA-224", 56),
    ("SHA-256", 64),
    ("SHA-384", 96),
    ("SHA-512", 128),
    ("Adler-32", 8),
];

/// 校验算法下拉中「NONE（不校验）」的下标（默认选中项）
pub const CK_NONE: usize = 0;

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
    /// Add: 校验算法在 CHECKSUM_ALGOS 中的下标（默认 NONE = 0，不校验）
    pub ck_type: usize,
    /// Add: 校验码输入缓冲（十六进制；算法为 NONE 时清空且不可输入，
    /// 其余算法必填）
    pub ck_value: String,
    /// Add: 校验算法下拉框是否展开
    pub ck_open: bool,
    /// Add/Modify: 下拉框当前高亮项（↑↓ 导航，实时同步 ck_type）
    pub ck_sel: usize,
    /// Add/Modify: 代理下拉在 [`App::proxy_options`] 中的下标（v1.5/FR-01-86）
    pub proxy_sel: usize,
    /// Add/Modify: 代理下拉框是否展开
    pub proxy_open: bool,
    /// Add:    0=URL 1=保存目录 2=并发数 3=校验算法 4=校验码 5=代理 6=立即下载 7=仅添加 8=取消
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
    /// 上一帧选中序号：用于区分「选中项变化」（需视口跟随）与
    /// 「鼠标滚动」（仅移视口不动选中项，若同样跟随则滚动条被锁死）
    last_selected: usize,
    /// 列表滚动偏移（以"任务条目"为单位）
    pub scroll: usize,
    /// 页签下标（对应 FILTERS）
    pub filter: usize,
    /// 全局下载速度历史（KB/s 采样）
    pub speed_hist: Vec<u64>,
    /// 共享带宽波动相位（秒）：多尺度正弦叠加模拟真实共享带宽起伏，
    /// 使全局速度曲线连续波动（流量图波形丰富）
    pub bw_t: f64,
    /// 全局上传速度历史（KB/s 采样）
    pub up_hist: Vec<u64>,
    /// 本次会话累计下载字节
    pub session_bytes: u64,
    pub rng: Rng,
    pub frame: u64,
    pub quit: bool,
    pub toast: Option<String>,
    toast_until: Option<Instant>,
    /// 添加对话框 URL 预填轮换序号（v1.20：沿用原 next_id 推进口径——
    /// 每次确认添加 +1，首值 100 保持 verify_ezr.py 场景 C 首开 = Fedora；
    /// 任务 ID 本身已改随机分配，本序号仅用于预填池轮换）
    pub prefill_seq: u32,
    last_tick: Instant,
    /// 速度展示面上次采样时刻（每 SPEED_TICK=1s 采样一次；v1.15/FR-01-102）
    last_speed_tick: Instant,
    /// 列表可视条目数（由 ui 层回填，用于滚动钳制）
    pub visible_rows: usize,
    /// 列表内框区域（由 ui 层回填，用于鼠标命中测试）
    pub list_area: Option<Rect>,
    /// 当前聚焦面板（Tab/BackTab 循环；边框高亮与 ↑↓ 列表选择跟随）
    pub focus: Panel,
    /// 帮助窗口（H 快捷键）是否打开
    pub help_open: bool,
    /// 关于窗口（O 快捷键）是否打开
    pub about_open: bool,
    /// 关于窗口左上角位置（鼠标拖拽移动记忆；None = 打开时居中）
    pub about_pos: Option<(u16, u16)>,
    /// 关于窗口拖拽中的抓取偏移（指针相对窗口左上角）
    pub about_drag: Option<(u16, u16)>,
    /// 关于窗口区域（由 ui 层每帧回填；拖拽的命中测试）
    pub about_area: Option<Rect>,
    /// 帮助窗口内容滚动偏移（以“行”为单位；渲染层按内容高度馴制）
    pub help_scroll: usize,
    /// 帮助窗口左上角位置（鼠标拖拽移动记忆；None = 打开时居中）
    pub help_pos: Option<(u16, u16)>,
    /// 帮助窗口拖拽中的抓取偏移（指针相对窗口左上角）
    pub help_drag: Option<(u16, u16)>,
    /// 帮助窗口区域（由 ui 层每帧回填；拖拽/滚轮的命中测试）
    pub help_area: Option<Rect>,
    /// 头部页签可点击区域（由 ui 层每帧回填；鼠标点击切换页签）
    pub tab_rects: Vec<(Rect, usize)>,
    /// 任务详情面板内框区域（由 ui 层回填；鼠标点击聚焦本面板）
    pub detail_area: Option<Rect>,
    /// 并发连接面板滚动偏移（以"连接行"为单位，滚轮/↑↓ 滚动）
    pub conns_scroll: usize,
    /// 并发连接面板当前选中行（以连接序号 0 基下标；聚焦本面板后 ↑↓ 移动，B 断开）
    pub conns_sel: usize,
    /// 并发连接面板内框区域（由 ui 层回填，用于鼠标滚轮命中测试）
    pub conns_area: Option<Rect>,
    /// 任务详情面板「URL」字段名热区（由 ui 层每帧回填；点击复制 url 值，修订-5）
    pub detail_url_rect: Option<Rect>,
    /// 任务详情面板「校验」字段名热区（由 ui 层每帧回填；点击复制校验码值，修订-5）
    pub detail_ck_rect: Option<Rect>,
    /// 任务详情面板「失败」字段名热区（由 ui 层每帧回填；点击复制失败原因值，
    /// 本轮需求修订：仅「已失败」态存在）
    pub detail_fail_rect: Option<Rect>,
    /// 任务详情面板「路径」字段名热区（由 ui 层每帧回填；点击复制保存路径值，
    /// 本轮需求修订新增；字段名原「保存」）
    pub detail_save_rect: Option<Rect>,
    /// 任务详情字段值热区（由 ui 层每帧回填；悬停 = 值下划线反馈、
    /// 点击 = 打开完整值弹窗，本轮需求修订新增）
    pub detail_val_rects: Vec<(Rect, DetailField)>,
    /// 指针最近位置（任意鼠标事件回填；详情字段值悬停下划线判定用，
    /// 本轮需求修订新增）
    pub pointer: Option<(u16, u16)>,
    /// 并发连接面板可视行数（由 ui 层回填，用于滚动钳制）
    pub visible_conns_rows: usize,
    /// 并发连接面板滚动所属的选中序号（选中变化时重置滚动）
    conns_scroll_for: usize,
    /// 并发连接面板横向滚动偏移（m3：以"内容列"为单位；BT 七列内容宽超出
    /// 内框时滚动，上限由 ui 层回填到 conns_max_hscroll）
    pub conns_hscroll: usize,
    /// 并发连接面板横向最大滚动偏移（内容宽 − 内框宽，由 ui 层每帧回填）
    pub conns_max_hscroll: usize,
    /// 任务列表滚动条命中几何（通用滚动条组件，ui 层每帧回填；None = 未绘制）
    pub list_sb: Option<scrollbar::Bar>,
    /// 并发连接面板垂直滚动条命中几何（ui 层每帧回填）
    pub conns_v_sb: Option<scrollbar::Bar>,
    /// 并发连接面板水平滚动条命中几何（m3 新增，ui 层每帧回填）
    pub conns_h_sb: Option<scrollbar::Bar>,
    /// pieces 弹窗滚动条命中几何（ui 层每帧回填）
    pub pieces_sb: Option<scrollbar::Bar>,
    /// 帮助窗口滚动条命中几何（ui 层每帧回填）
    pub help_sb: Option<scrollbar::Bar>,
    /// pieces 网格最大滚动偏移（总行数 − 网格行数，ui 层每帧回填；滚动钳制用）
    pub pieces_max_scroll: usize,
    /// 帮助窗口最大滚动偏移（内容行数 − 可视行数，ui 层每帧回填）
    pub help_max_scroll: usize,
    /// 通用滚动条滑块拖拽中：((哪个滚动条, 抓取点在滑块内的格偏移))。
    /// 拖拽期间 Drag 事件滚动内容而非移动窗口（浮层同口径——拖滑块滚内容、
    /// 拖空白处移窗口的分歧点就在本字段）
    pub sb_drag: Option<(SbKey, usize)>,
    /// 通用滚动条箭头/轨道按压连发状态（Down 记录、Up 取消、tick 驱动重复）
    pub sb_hold: Option<SbHold>,
    /// 通用滚动条箭头按压高亮态（Some = (哪个滚动条, 哪个箭头)）：按下即亮、
    /// 按住保持、拖出取消（连发不中断）、松开清除——与页脚 chips 按压反色
    /// 同口径（§4.1.3）
    pub sb_pressed: Option<(SbKey, scrollbar::Arrow)>,
    /// 箭头点击高亮余辉（Down 即置 400ms）：快速单击松开后仍有一段可见的
    /// 点击反馈，与页脚 chips 的 footer_flash 同机制
    pub sb_flash: Option<(SbKey, scrollbar::Arrow, Instant)>,
    /// 是否显示右侧任务详情 / 并发连接面板（`G` 切换；头部流量图始终显示、不受影响）
    pub show_panes: bool,
    /// pieces 分块弹窗（P 快捷键）是否打开
    pub pieces_open: bool,
    /// pieces 弹窗网格滚动偏移（以“网格行”为单位，每行 cols 块；
    /// 上限由渲染层按实际总行数钳制）
    pub pieces_scroll: usize,
    /// 打开 pieces 弹窗时定位到下载前沿（由 ui 层按实际列数消费一次，
    /// 此后滚动完全由用户控制，不自动跟随）
    pub pieces_follow: bool,
    /// pieces 弹窗区域（由 ui 层每帧回填；滚轮滚动/拖拽的命中测试）
    pub pieces_area: Option<Rect>,
    /// pieces 弹窗左上角位置（鼠标拖拽移动记忆；None = 打开时居中，
    /// 由渲染层首帧回填；渲染时钳制在终端内）
    pub pieces_pos: Option<(u16, u16)>,
    /// pieces 弹窗拖拽中的抓取偏移（指针相对弹窗左上角；Some = 左键拖拽进行中）
    pub pieces_drag: Option<(u16, u16)>,
    /// 任务详情字段值弹窗（点击详情字段值打开；本轮需求修订新增）是否打开
    pub val_open: bool,
    /// 弹窗对应字段（Y 复制 toast 文案区分）
    pub val_field: DetailField,
    /// 弹窗标题（= 字段名：校验 / URL / 失败 / 路径）
    pub val_title: String,
    /// 弹窗内容（打开时快照的完整字段值；wrap text 显示）
    pub val_text: String,
    /// 弹窗内容滚动偏移（wrap 后行数；上限由渲染层按实际内容行数钳制）
    pub val_scroll: usize,
    /// 弹窗内容最大滚动偏移（内容行数 − 可视行数，ui 层每帧回填）
    pub val_max_scroll: usize,
    /// 弹窗区域（由 ui 层每帧回填；滚轮滚动/拖拽的命中测试）
    pub val_area: Option<Rect>,
    /// 弹窗左上角位置（鼠标拖拽移动记忆；None = 打开时居中，
    /// 由渲染层首帧回填；渲染时钳制在终端内）
    pub val_pos: Option<(u16, u16)>,
    /// 弹窗拖拽中的抓取偏移（指针相对弹窗左上角；Some = 左键拖拽进行中）
    pub val_drag: Option<(u16, u16)>,
    /// 字段值弹窗滚动条命中几何（ui 层每帧回填）
    pub val_sb: Option<scrollbar::Bar>,
    /// 弹窗右上角 [✗] 关闭按钮热区（由 ui 层每帧回填；顶层弹窗为准，
    /// 无弹窗 = None）。Down 按下 = 按压高亮、原地松开 = 关闭当前弹窗
    /// （页脚 chips 同纪律；本轮需求修订：全部弹窗统一右上角 [✗]——
    /// ✗ = U+2717 BALLOT X，中括号内紧凑无 padding，与边框线左右各留
    /// 1 列 padding；热区仅 [✗] 自身 3 列）
    pub popup_close_rect: Option<Rect>,
    /// [✗] 按压态（Down 记录、Up 消费；拖出热区取消——渲染层据此仅前景色
    /// 红+粗高亮，无背景色/无反色）
    pub x_press: bool,
    /// 当前打开的对话框（None = 无）
    pub dialog: Option<Dialog>,
    /// 对话框左上角位置（鼠标拖拽移动记忆；None = 打开时居中，
    /// 由渲染层首帧回填；渲染时钳制在终端内）
    pub dialog_pos: Option<(u16, u16)>,
    /// 对话框拖拽中的抓取偏移（指针相对弹窗左上角；Some = 左键拖拽进行中）
    pub dialog_drag: Option<(u16, u16)>,
    /// 对话框拖拽期间指针是否实际移动过（Up 时区分「点击」与「拖拽」：
    /// 未移动 = 点击 → 聚焦字段行/激活按钮；移动过 = 拖拽 → 不触发点击）
    pub dialog_drag_moved: bool,
    /// 对话框区域（由 ui 层每帧回填；拖拽抓取与点击的命中测试）
    pub dialog_area: Option<Rect>,
    /// 对话框按钮的可点击区域（由 ui 层每帧回填）
    pub dlg_btn_rects: Vec<(Rect, usize)>,
    /// 对话框输入行/选择行的可点击区域（由 ui 层每帧回填；点击聚焦，
    /// 校验算法行再点击可展开下拉框）
    pub dlg_field_rects: Vec<(Rect, usize)>,
    /// 校验算法下拉框各选项的可点击区域（由 ui 层每帧回填，仅展开时有效）
    pub dlg_ck_rects: Vec<(Rect, usize)>,
    /// 代理下拉选项命中区域（v1.5/FR-01-86；鼠标点击选择）
    pub dlg_proxy_rects: Vec<(Rect, usize)>,
    /// 页脚命令 chips 可点击区域（v0.2 页脚双行修订；由 ui 层每帧回填：
    /// 行 1 常用命令图例 A/D/M/Q/H，左键按压高亮 = 点击效果、原地松开执行对应命令）
    pub footer_cmd_rects: Vec<(Rect, usize)>,
    /// 页脚命令 chip 按压态（Down 记录、Up 原地松开消费；渲染层据此反色高亮）
    pub footer_press: Option<usize>,
    /// 页脚命令 chip 点击高亮余辉（Down 点亮 400ms；快速点按在两帧之间也能看到点击效果）
    pub footer_flash: Option<(usize, Instant)>,
    /// 代理选项表（v1.5/FR-01-86 同步；演示数据：直连 + 命名条目）
    pub proxy_options: Vec<ProxyChoice>,
    /// 代理选项显示名（与 proxy_options 一一对应；命名条目含类型标注，不含认证信息）
    pub proxy_labels: Vec<String>,
    /// 添加对话框代理默认选中下标（恒直连 = 0）
    pub default_proxy_sel: usize,
    /// 系统剪贴板实例（修订-7：懒初始化并复用，避免每次点击重建 X11/桌面连接）
    clipboard: Option<arboard::Clipboard>,
    /// 模拟盘 sidecar 台账（v1.28 生产同步轮八，FR-01-26 路径基接续）：
    /// 键 = (保存目录, 推导名)，代表模拟盘上存在的 `<目录>/<名>.ezr` 有效断点。
    /// demo 无真实文件系统，由删除对话框「保留文件」臂登记残留（生产 FR-01-25：
    /// 仅删任务时文件与 sidecar 全保留）；「删除文件」臂移除；任务转「已完成」
    /// 后其名下条目在下次添加判定前同步清除（生产 FR-01-20/24：完成即删 sidecar）。
    /// 接续判定仅按路径匹配、不做同 URL 校验（v1.28：异 URL 同推导名也接续，
    /// 误接续由生产续传一致性检查兜底，demo 无一致性环节故直接演示接续）。
    pub sim_sidecars: HashSet<(String, String)>,
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
            last_selected: 0,
            scroll: 0,
            filter: 0,
            // 速度历史不预填（每秒采样一个样本，暖机 ~2s 后图区开始绘制；
            // 生产同口径逐秒推进）
            speed_hist: Vec::new(),
            bw_t: 0.0,
            up_hist: Vec::new(),
            session_bytes: 0,
            rng: Rng::new(seed),
            frame: 0,
            quit: false,
            help_open: false,
            about_open: false,
            about_pos: None,
            about_drag: None,
            about_area: None,
            help_scroll: 0,
            help_pos: None,
            help_drag: None,
            help_area: None,
            tab_rects: Vec::new(),
            detail_area: None,
            toast: Some("EZR Downloader Demo 就绪 · 数据为模拟数据".to_string()),
            toast_until: Some(Instant::now() + Duration::from_secs(4)),
            prefill_seq: 100,
            sim_sidecars: HashSet::new(),
            last_tick: Instant::now(),
            last_speed_tick: Instant::now(),
            visible_rows: 6,
            list_area: None,
            focus: Panel::List,
            conns_scroll: 0,
            conns_sel: 0,
            conns_area: None,
            detail_url_rect: None,
            detail_ck_rect: None,
            detail_fail_rect: None,
            detail_save_rect: None,
            detail_val_rects: Vec::new(),
            pointer: None,
            visible_conns_rows: 8,
            conns_scroll_for: 0,
            conns_hscroll: 0,
            conns_max_hscroll: 0,
            list_sb: None,
            conns_v_sb: None,
            conns_h_sb: None,
            pieces_sb: None,
            help_sb: None,
            pieces_max_scroll: 0,
            help_max_scroll: 0,
            sb_drag: None,
            sb_hold: None,
            sb_pressed: None,
            sb_flash: None,
            show_panes: true,
            pieces_open: false,
            pieces_scroll: 0,
            pieces_follow: false,
            pieces_area: None,
            pieces_pos: None,
            pieces_drag: None,
            val_open: false,
            popup_close_rect: None,
            x_press: false,
            val_field: DetailField::Url,
            val_title: String::new(),
            val_text: String::new(),
            val_scroll: 0,
            val_max_scroll: 0,
            val_area: None,
            val_pos: None,
            val_drag: None,
            val_sb: None,
            dialog: None,
            dialog_pos: None,
            dialog_drag: None,
            dialog_drag_moved: false,
            dialog_area: None,
            dlg_btn_rects: Vec::new(),
            dlg_field_rects: Vec::new(),
            dlg_ck_rects: Vec::new(),
            dlg_proxy_rects: Vec::new(),
            footer_cmd_rects: Vec::new(),
            footer_press: None,
            footer_flash: None,
            proxy_options,
            proxy_labels,
            default_proxy_sel: 0,
            clipboard: None,
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
        // 视口即时跟随选中项（不依赖下一帧 tick，避免一帧延迟）
        self.scroll_to_selection();
    }

    /// 页签切换（delta = ±1，环形）；切换后视口定位到选中项
    fn cycle_filter(&mut self, delta: i32) {
        let n = FILTERS.len() as i32;
        self.filter = ((self.filter as i32 + delta).rem_euclid(n)) as usize;
        self.scroll_to_selection();
    }

    /// 将视口滚动到选中项可见（选中项变化 / 页签切换后调用；
    /// 鼠标滚动不调用——视口自由移动正是滚动条不锁死的前提）
    fn scroll_to_selection(&mut self) {
        let vis = self.visible_rows.max(1);
        if self.scroll > self.selected {
            self.scroll = self.selected;
        }
        if self.selected >= self.scroll + vis {
            self.scroll = self.selected + 1 - vis;
        }
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
        // 共享带宽波动相位推进（多尺度正弦叠加：≈6.5s 主周期 + ≈2.1s 次周期 +
        // ≈1.1s 纹波，合成 ±19% 连续起伏 —— 模拟真实共享带宽波动，让流量图呈现
        // 连续多尺度波形）
        self.bw_t += dt;
        let bw = 1.0
            + 0.11 * (std::f64::consts::TAU * self.bw_t / 6.5).sin()
            + 0.05 * (std::f64::consts::TAU * self.bw_t / 2.1 + 1.7).sin()
            + 0.03 * (std::f64::consts::TAU * self.bw_t / 1.1 + 4.0).sin();

        let rng = &mut self.rng;
        let mut notices: Vec<String> = Vec::new();
        // 本帧实际落盘字节（会话已下载累计改用实际字节而非展示 EMA 积分，
        // 与生产 FR-01-81 口径一致：EMA 爬升期少计会随任务结束固化）
        let mut frame_dl_bytes: u64 = 0u64;

        for t in self.tasks.iter_mut() {
            match t.state {
                TaskState::Downloading => {
                    // 演示：首次失败之后的重试模拟「连接卡死」——无数据下载，
                    // 用于展示「连续失败（无进展）累加重试次数」规则
                    let stalled = t.flaky && t.fail_count > 1;
                    // ---- 数据面（逐帧）：瞬时速率仅推进字节与采样窗口累计，
                    // 不再直接写展示速度——展示面统一由每秒采样 + EMA α=1/5
                    // 驱动（见下方采样块，v1.15/FR-01-102 同步）；连接全部
                    // 被断开时视为无传输（无字节进窗）
                    let task_inst = if stalled || t.connections.is_empty() {
                        0.0
                    } else {
                        t.base_speed * bw * rng.range(0.92, 1.08)
                    };
                    let n = t.connections.len().max(1) as f64;
                    for c in t.connections.iter_mut() {
                        if c.cap() > 0 && c.done < c.cap() {
                            let rate = (task_inst / n) * rng.range(0.88, 1.12);
                            let nd = ((c.done as f64 + rate * dt).min(c.cap() as f64) as u64)
                                .saturating_sub(c.done);
                            c.win += nd;
                            c.cum_down = c.cum_down.saturating_add(nd);
                            c.done += nd;
                            frame_dl_bytes += nd;
                        }
                        // 待命/空连接：展示速度不在逐帧路径上触碰——下一采样点
                        // 零值速断（D33 锚定的最坏 1s 时延）
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
                                    // 速度沿用上一块末速（EMA 自然收敛），
                                    // 避免领块瞬间归零造成明细速度闪烁
                                    next += 1;
                                } else {
                                    // 队列已空：连接待命（展示速度不动——下一采样点
                                    // 零值速断，D33 最坏 1s 时延；数据面无字节进窗）
                                    c.start = 0;
                                    c.end = 0;
                                    c.done = 0;
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
                    // BT 边下边传（数据面）：上传瞬时速率按活跃连接均摊抖动，
                    // 推进累计与采样窗口；展示面由下方每秒采样驱动
                    if t.protocol.is_bt() {
                        let up_inst = if stalled || t.connections.is_empty() {
                            0.0
                        } else {
                            420_000.0 * rng.range(0.5, 1.7)
                        };
                        let un = t.connections.len().max(1) as f64;
                        for c in t.connections.iter_mut() {
                            if c.cap() > 0 && c.done < c.cap() {
                                let rate = (up_inst / un) * rng.range(0.88, 1.12);
                                let nb = (rate * dt) as u64;
                                c.win_up += nb;
                                c.cum_up = c.cum_up.saturating_add(nb);
                            }
                        }
                        t.uploaded = t.uploaded.saturating_add((up_inst * dt) as u64);
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
                                    t.connections =
                                        make_chunk_conns(total, 0, n, piece, t.id as u64).0;
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
                            // 已完成任务不保留并发连接明细（面板为空、活跃 0/0）
                            t.connections.clear();
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
                            // 校验失败路径（FR-M1-51；生产 v1.17 同步）：不自动重试；
                            // 按 R = 重新校验（无块重传，D10）——校验码清空后按 R
                            // 则不校验直接完成「无校验」（D34）
                            if t.verify_fail_once {
                                t.verify_fail_once = false;
                                t.verify_ok = Some(false);
                                t.state = TaskState::Failed;
                                t.fail_kind = Some(FailKind::Verify);
                                t.retry_in = None;
                                t.has_slot = false;
                                t.error = Some(format!("{} 校验失败：内容与校验值不符", algo));
                                notices.push(format!(
                                    "✘ {} 校验失败：不自动重试，按 R 重新校验: {}",
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
                                    // 已完成任务不保留并发连接明细（面板为空、活跃 0/0）
                                    t.connections.clear();
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
                            // 已完成任务不保留并发连接明细（面板为空、活跃 0/0）
                            t.connections.clear();
                            notices.push(format!("✓ 后期处理完成: {}", t.name));
                        }
                    }
                }
                TaskState::Seeding => {
                    // 上传数据面：瞬时速率按持有块的连接均摊抖动，推进累计与
                    // 采样窗口；展示面由下方每秒采样驱动（同下载口径）
                    let up_inst = 1_900_000.0 * rng.range(0.6, 1.5);
                    let un = t.connections.iter().filter(|c| c.cap() > 0).count().max(1) as f64;
                    for c in t.connections.iter_mut() {
                        if c.cap() > 0 {
                            let rate = (up_inst / un) * rng.range(0.88, 1.12);
                            let nb = (rate * dt) as u64;
                            c.win_up += nb;
                            c.cum_up = c.cum_up.saturating_add(nb);
                        }
                    }
                    t.uploaded = t.uploaded.saturating_add((up_inst * dt) as u64);
                    t.seed_left = (t.seed_left - dt).max(0.0);
                    if t.seed_left <= 0.0 {
                        t.state = TaskState::Completed;
                        t.upload_speed = 0.0;
                        // 已完成任务不保留并发连接明细（面板为空、活跃 0/0）
                        t.connections.clear();
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
                                t.connections = make_chunk_conns(total, 0, n, piece, t.id as u64).0;
                            }
                            t.state = TaskState::Downloading;
                            t.queued_since = None;
                            t.running_since = Some(now);
                            t.made_progress = false; // 新尝试开始：清空进展标记
                                                     // 本次开始下载：各连接的累计上传/下载量清零
                                                     //（重新排队/重试/新任务均经此路径；断点续传的已完成量不受影响）
                            for c in t.connections.iter_mut() {
                                c.cum_down = 0;
                                c.cum_up = 0;
                                c.up_speed = 0.0;
                            }
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

        // -------------------------------------------------------------------
        // 速度展示面（v1.15/FR-01-102 同步，与生产同管线）：
        // ① 非下载/做种展示任务逐帧零值速断（无衰减拖尾）；
        // ② 下载/做种展示值每秒采样一次（1s 窗口字节速率 → EMA α=1/5），
        //    数值每秒最多变化一次；
        // ③ 下载中任务的待命/空连接在采样点立即归零（D33：最坏 1s 时延）；
        //    在传连接即使窗口速率为 0 也持续平滑（避免 `-` 闪烁）。
        // -------------------------------------------------------------------
        let speed_due = now.duration_since(self.last_speed_tick) >= SPEED_TICK;
        if speed_due {
            self.last_speed_tick = now;
        }
        for t in self.tasks.iter_mut() {
            match t.state {
                TaskState::Downloading => {
                    if !speed_due {
                        continue;
                    }
                    if t.connections.is_empty() {
                        // 连接全部被断开：视为无传输，展示速度立即归零
                        t.speed = 0.0;
                        continue;
                    }
                    // 任务级：1s 窗口字节速率 → EMA α=1/5（与连接同一节拍）
                    let win: u64 = t.connections.iter().map(|c| c.win).sum();
                    let sample = win as f64 / SPEED_TICK.as_secs_f64();
                    t.speed += (sample - t.speed) * SPEED_EMA_ALPHA;
                    for c in t.connections.iter_mut() {
                        if c.cap() == 0 || c.done >= c.cap() {
                            // 待命/空连接：零值速断（无拖尾）
                            c.speed = 0.0;
                        } else {
                            // 在传连接：即使窗口速率为 0 也持续平滑
                            let s = c.win as f64 / SPEED_TICK.as_secs_f64();
                            c.speed += (s - c.speed) * SPEED_EMA_ALPHA;
                        }
                    }
                    // 上传展示采样（BT 下载中）
                    if t.protocol.is_bt() {
                        let win_up: u64 = t.connections.iter().map(|c| c.win_up).sum();
                        let up = win_up as f64 / SPEED_TICK.as_secs_f64();
                        t.upload_speed += (up - t.upload_speed) * SPEED_EMA_ALPHA;
                        for c in t.connections.iter_mut() {
                            if c.cap() == 0 || c.done >= c.cap() {
                                c.up_speed = 0.0;
                            } else {
                                let s = c.win_up as f64 / SPEED_TICK.as_secs_f64();
                                c.up_speed += (s - c.up_speed) * SPEED_EMA_ALPHA;
                            }
                        }
                    }
                    // 窗口清零（下一采样窗口重新累计）
                    for c in t.connections.iter_mut() {
                        c.win = 0;
                        c.win_up = 0;
                    }
                }
                TaskState::Seeding => {
                    if !speed_due {
                        continue;
                    }
                    let win_up: u64 = t.connections.iter().map(|c| c.win_up).sum();
                    let up = win_up as f64 / SPEED_TICK.as_secs_f64();
                    t.upload_speed += (up - t.upload_speed) * SPEED_EMA_ALPHA;
                    for c in t.connections.iter_mut() {
                        // 下载列待命（做种无下行）；上传按持有块连接采样
                        c.speed = 0.0;
                        if c.cap() == 0 {
                            c.up_speed = 0.0;
                        } else {
                            let s = c.win_up as f64 / SPEED_TICK.as_secs_f64();
                            c.up_speed += (s - c.up_speed) * SPEED_EMA_ALPHA;
                        }
                        c.win = 0;
                        c.win_up = 0;
                    }
                }
                _ => {
                    // 非下载/做种态：逐帧零值速断（无拖尾，与生产 tick ⑤一致）
                    t.speed = 0.0;
                    t.upload_speed = 0.0;
                    for c in t.connections.iter_mut() {
                        c.speed = 0.0;
                        c.up_speed = 0.0;
                    }
                }
            }
        }

        // 会话统计与速度历史（历史/峰值仅在采样点推进：每秒一个样本，
        // 与生产 push_hist 同节拍；会话累计用实际落盘字节）
        self.session_bytes = self.session_bytes.saturating_add(frame_dl_bytes);
        if speed_due {
            let global_dl: f64 = self
                .tasks
                .iter()
                .filter(|t| t.state == TaskState::Downloading)
                .map(|t| t.speed)
                .sum();
            let global_ul: f64 = self
                .tasks
                .iter()
                .filter(|t| t.upload_speed > 0.0)
                .map(|t| t.upload_speed)
                .sum();
            self.speed_hist.push((global_dl / 1024.0) as u64);
            if self.speed_hist.len() > 180 {
                self.speed_hist.remove(0);
            }
            self.up_hist.push((global_ul / 1024.0) as u64);
            if self.up_hist.len() > 180 {
                self.up_hist.remove(0);
            }
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
            // 仅当选中项变化时保持其可见（键盘选择 / 末尾钳制后视口跟随）；
            // 鼠标滚动只移视口不动选中项，若同样跟随则视口每帧被拉回
            // 选中行——滚动条锁死无法浏览全表（滚动锁死修复）
            if self.last_selected != self.selected {
                self.scroll_to_selection();
            }
            let max_scroll = flen.saturating_sub(self.visible_rows.max(1));
            self.scroll = self.scroll.min(max_scroll);
        }
        self.last_selected = self.selected;

        // 并发连接面板：选中任务变化时重置滚动与选中行，并按连接数与可视行数钳制
        if self.conns_scroll_for != self.selected {
            self.conns_scroll_for = self.selected;
            self.conns_scroll = 0;
            self.conns_sel = 0;
            self.conns_hscroll = 0; // 横向滚动同步归零（新任务列结构重新浏览）
        }
        // 仅「下载中」/「做种中」状态存在可视并发明细：
        // 其余状态按 0 条处理（选中行与滚动自然归零）
        let conns_visible = self
            .sel_task()
            .map(|t| t.state.shows_conns())
            .unwrap_or(false);
        let clen = if conns_visible {
            self.sel_task().map(|t| t.connections.len()).unwrap_or(0)
        } else {
            0
        };
        let cvis = self.visible_conns_rows.max(1);
        let cmax = clen.saturating_sub(cvis);
        self.conns_scroll = self.conns_scroll.min(cmax);
        // 横向滚动钳制（上限 = 内容宽 − 内框宽，ui 层每帧回填；
        // 协议切换 BT↔HTTP 时内容宽变化，由本钳制兜底）
        self.conns_hscroll = self.conns_hscroll.min(self.conns_max_hscroll);
        if self.conns_sel >= clen.max(1) {
            self.conns_sel = clen.saturating_sub(1);
        }

        // 通用滚动条箭头/轨道按住连发（Down 记录、Up 取消、tick 驱动重复：
        // 起振 0.4s、周期 0.12s，§4.1.3）
        self.sb_hold_tick(now);
    }

    // -----------------------------------------------------------------------
    // 键盘交互
    // -----------------------------------------------------------------------

    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) {
            if code == KeyCode::Char('c') {
                self.quit = true;
            }
            // 其余 Ctrl 组合键不再占用（原 Ctrl+↑↓/Ctrl+B 并发面板快捷键已
            // 统一进面板焦点模型：聚焦面板后 ↑↓ 选择、B 断开）
            return;
        }
        // 关于窗口最上层模态：Esc/O/q/Q 关闭（先关窗不退出），其余键忽略
        if self.about_open {
            match code {
                KeyCode::Esc
                | KeyCode::Char('o')
                | KeyCode::Char('O')
                | KeyCode::Char('q')
                | KeyCode::Char('Q') => self.close_about(),
                _ => {}
            }
            return;
        }
        // 任务详情字段值弹窗模态（本轮需求修订）：Esc/q/Q 关闭（先关窗不退出）；
        // Y 复制窗口全部内容（完整字段值）；↑↓/PgUp/PgDn/Home/End 滚动内容。
        // 窗口内无任何操作提示——交互说明收敛于 H 帮助窗口（P3 原则）
        if self.val_open {
            match code {
                KeyCode::Esc
                | KeyCode::Char('q')
                | KeyCode::Char('Q') => self.close_val_popup(),
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let text = self.val_text.clone();
                    self.copy_to_clipboard(&text);
                    let label = self.val_field.copy_label();
                    self.set_toast(format!("已复制 {label}"));
                }
                KeyCode::Up => self.val_scroll_by(-1),
                KeyCode::Down => self.val_scroll_by(1),
                KeyCode::PageUp => self.val_scroll_by(-8),
                KeyCode::PageDown => self.val_scroll_by(8),
                KeyCode::Home => self.val_scroll = 0,
                KeyCode::End => self.val_scroll = usize::MAX, // 渲染层钳制到末页
                _ => {}
            }
            return;
        }
        // 对话框打开时，按键优先交给对话框
        if self.dialog.is_some() {
            self.on_dialog_key(code);
            return;
        }
        //（q/Q 同为关闭而非退出——弹窗内先关窗，避免误触直接退出应用）
        if self.pieces_open {
            match code {
                KeyCode::Esc
                | KeyCode::Char('p')
                | KeyCode::Char('P')
                | KeyCode::Char('q')
                | KeyCode::Char('Q') => self.close_pieces(),
                KeyCode::Up => self.pieces_scroll_by(-1),
                KeyCode::Down => self.pieces_scroll_by(1),
                KeyCode::PageUp => self.pieces_scroll_by(-8),
                KeyCode::PageDown => self.pieces_scroll_by(8),
                KeyCode::Home => self.pieces_scroll = 0,
                KeyCode::End => self.pieces_scroll = usize::MAX, // 渲染层钳制到末页
                _ => {}
            }
            return;
        }
        // 帮助窗口模态：滚动翻阅分类帮助；Esc/H/q 关闭（q 同 pieces 口径：
        // 先关窗不退出）；O 打开关于窗口（About 叠于帮助之上，z-order 最上层）
        if self.help_open {
            match code {
                KeyCode::Esc
                | KeyCode::Char('h')
                | KeyCode::Char('H')
                | KeyCode::Char('q')
                | KeyCode::Char('Q') => self.help_open = false,
                KeyCode::Char('o') | KeyCode::Char('O') => self.open_about(),
                KeyCode::Up => self.help_scroll = self.help_scroll.saturating_sub(1),
                KeyCode::Down => self.help_scroll += 1, // 渲染层钳制
                KeyCode::PageUp => self.help_scroll = self.help_scroll.saturating_sub(8),
                KeyCode::PageDown => self.help_scroll += 8,
                KeyCode::Home => self.help_scroll = 0,
                KeyCode::End => self.help_scroll = usize::MAX,
                _ => {}
            }
            return;
        }
        match code {
            // Q/q：退出程序（需求修订：退出键统一为 Q，Esc 仅用于关闭弹窗，
            // 主界面下按 Esc 无操作——避免想关弹窗时误触退出）
            KeyCode::Char('q') | KeyCode::Char('Q') => self.quit = true,
            // Tab/BackTab：面板焦点循环（lazygit 式聚焦；当前面板边框高亮 +
            // 页签高亮 + ↑↓ 作用于本面板列表）。页签循环已挪到 T 键。
            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::BackTab => self.focus = self.focus.prev(),
            // T：页签循环（正在下载 ⇄ 已完成；与 ] 等效，保留兼容）。
            // 切换后 filtered 列表已变，显式将视口定位到选中项（可见）
            KeyCode::Char('t') | KeyCode::Char('T') => self.cycle_filter(1),
            // ] / [：页签右/左切换（需求修订：中括号键 + 鼠标点击两种方式）
            KeyCode::Char(']') | KeyCode::Char('}') => self.cycle_filter(1),
            KeyCode::Char('[') | KeyCode::Char('{') => self.cycle_filter(-1),
            // H：帮助窗口（分类交互说明；Esc/H/q 关闭）
            KeyCode::Char('h') | KeyCode::Char('H') => {
                self.help_open = true;
                self.help_scroll = 0;
                // help_pos 保留上次拖拽位置；None = 渲染层首帧居中并回填
            }
            // O：关于窗口（About，显示 EZR Download v1.0.0；Esc/O/q 关闭）
            KeyCode::Char('o') | KeyCode::Char('O') => self.open_about(),
            // ↑/↓：统一列表选择交互——聚焦任务列表 = 选任务；聚焦并发连接 =
            // 选连接明细（原 Ctrl+↑↓ 语义，修订：移除 Ctrl 组合键方式）。
            // A 形态列表的选择即纵向滚动（视口即时跟随，§4.1.6）
            KeyCode::Down => match self.focus {
                Panel::List => self.move_sel(1),
                Panel::Conns => self.move_conn_sel(1),
                Panel::Detail => {}
            },
            KeyCode::Up => match self.focus {
                Panel::List => self.move_sel(-1),
                Panel::Conns => self.move_conn_sel(-1),
                Panel::Detail => {}
            },
            // ←/→（m3）：并发连接面板聚焦时横向滚动明细列（BT 七列超宽、
            // 底部横向滚动条 ↔ 键盘双通道等价）；其余面板无横向内容不响应
            KeyCode::Left => {
                if self.focus == Panel::Conns {
                    self.sb_scroll(SbKey::ConnsH, -SbKey::ConnsH.step());
                }
            }
            KeyCode::Right => {
                if self.focus == Panel::Conns {
                    self.sb_scroll(SbKey::ConnsH, SbKey::ConnsH.step());
                }
            }
            KeyCode::PageDown => match self.focus {
                Panel::Conns => self.move_conn_sel(self.visible_conns_rows.max(1) as i32),
                _ => self.move_sel(4),
            },
            KeyCode::PageUp => match self.focus {
                Panel::Conns => self.move_conn_sel(-(self.visible_conns_rows.max(1) as i32)),
                _ => self.move_sel(-4),
            },
            KeyCode::Home => match self.focus {
                Panel::Conns => {
                    let n = self.conn_count();
                    if n > 0 {
                        self.move_conn_sel(-(n as i32));
                    }
                }
                _ => self.selected = 0,
            },
            KeyCode::End => match self.focus {
                Panel::Conns => {
                    let n = self.conn_count();
                    if n > 0 {
                        self.move_conn_sel(n as i32);
                    }
                }
                _ => {
                    let flen = self.filtered().len();
                    if flen > 0 {
                        self.selected = flen - 1;
                    }
                }
            },
            // B：阻塞并移除（断开）选中的并发连接——仅 BT 任务（原 Ctrl+B 语义）
            KeyCode::Char('b') | KeyCode::Char('B') => self.disconnect_conn(),
            KeyCode::Char(' ') => self.toggle_pause(),
            KeyCode::Char('r') | KeyCode::Char('R') => self.retry(),
            // P：查看选中任务的 pieces 分块弹窗（仅支持分块下载的任务）
            KeyCode::Char('p') | KeyCode::Char('P') => self.open_pieces(),
            KeyCode::Char('a') | KeyCode::Char('A') => self.open_add_dialog(),
            KeyCode::Char('m') | KeyCode::Char('M') => self.open_modify_dialog(),
            KeyCode::Char('d') | KeyCode::Char('D') => self.open_delete_dialog(),
            KeyCode::Char('c') | KeyCode::Char('C') => self.clear_completed(),
            // 修订-1：G 仅切换任务详情 / 并发连接面板（紧凑布局），流量图始终显示；
            // 收起后详情/并发面板不可见，焦点强制回任务列表
            KeyCode::Char('g') | KeyCode::Char('G') => {
                self.show_panes = !self.show_panes;
                if !self.show_panes {
                    self.focus = Panel::List;
                }
            }
            KeyCode::Char('u') | KeyCode::Char('U') => self.move_task(-1),
            KeyCode::Char('j') | KeyCode::Char('J') => self.move_task(1),
            _ => {}
        }
    }

    /// 当前选中任务的并发明细条数（仅「下载中」/「做种中」任务有明细）
    fn conn_count(&self) -> usize {
        self.sel_task()
            .filter(|t| t.state.shows_conns())
            .map(|t| t.connections.len())
            .unwrap_or(0)
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
            DialogKind::Add => 9,
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

    /// Add 对话框 Enter：算法/代理行展开下拉；7=仅添加 8=取消（v1.15/FR-01-103）；
    /// 其余（含非按钮焦点 0/1/2/4）= 立即下载（D32：非按钮焦点 Enter 同确认）
    fn add_dialog_enter(&mut self, focus: usize) {
        if focus == 3 {
            let d = self.dialog.as_mut().unwrap();
            d.ck_open = true;
            d.ck_sel = d.ck_type;
        } else if focus == 5 {
            let d = self.dialog.as_mut().unwrap();
            d.proxy_open = true;
        } else if focus == 7 {
            self.dlg_add_only();
        } else if focus == 8 {
            self.dialog = None;
        } else {
            self.dlg_confirm_add(false);
        }
    }

    /// 仅添加（v1.15/FR-01-103，D28）：与立即下载同校验、同建任务链路，
    /// 唯一差别 = 任务落「已暂停」态，不占下载槽位、不被自动调度启动
    fn dlg_add_only(&mut self) {
        self.dlg_confirm_add(true);
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
    /// 5 空格展开代理下拉、6/7/8 空格激活按钮（立即下载/仅添加/取消）
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
            // 校验码：仅接受十六进制字符，最多 128 位（SHA-512）；
            // 算法为 NONE（不校验）时不可输入（需求：NONE 下校验码清空禁输入）
            if self.dialog.as_ref().unwrap().ck_type != CK_NONE
                && c.is_ascii_hexdigit()
                && self.dialog.as_ref().unwrap().ck_value.chars().count() < 128
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
            // 校验码：仅接受十六进制字符，最多 128 位（SHA-512）；
            // 算法为 NONE（不校验）时不可输入（需求：NONE 下校验码清空禁输入）
            if self.dialog.as_ref().unwrap().ck_type != CK_NONE
                && c.is_ascii_hexdigit()
                && self.dialog.as_ref().unwrap().ck_value.chars().count() < 128
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
        // v1.15/FR-01-103：6=立即下载 7=仅添加 8=取消
        match btn {
            6 => self.dlg_confirm_add(false),
            7 => self.dlg_add_only(),
            8 => self.dialog = None,
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

        // 校验：NONE = 不校验（清除校验码）；其余算法同添加对话框合法性口径
        //（校验码必填，非空同添加对话框合法性），非法 → 不生效 + 聚焦校验码字段
        let checksum = if ck_type == CK_NONE {
            None
        } else if ck_raw.is_empty() {
            let (algo, need) = CHECKSUM_ALGOS[ck_type.min(CHECKSUM_ALGOS.len() - 1)];
            self.set_toast(format!(
                "⚠ {} 校验码不可为空（需 {} 位十六进制）",
                algo, need
            ));
            if let Some(d) = self.dialog.as_mut() {
                d.focus = 2;
            }
            return;
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
                // 立即生效（v1.5/FR-01-87 demo 语义；v1.16/D30 同步）：按新并发
                // 重建分块连接。v1.11/REQ-15.1（生产 coder v139 缺陷修复同步，
                // 操作者 20261013 指令）：已完成块数以 tick 逐块记账（chunk_done）
                // 为权威，不做「已下载字节→前缀块数」投影——多连接乱序/部分块
                // 在传时投影会把在传零头折成幻影已完成块，改并发瞬间分块 x
                // 跳变（与生产修复前「error 13 → 恢复 12」同源：启发式投影
                // 不得写入权威计数）。在传零头字节按新并发重摊到续传窗口
                // [chunk_done, chunk_done+active)；并发收缩且零头超过新窗口
                // 容量时，超出部分随队尾块回队重下（与中止块再领队列同口径）。
                // 幸存槽位展示速度沿用旧值（同槽位同 IP，收缩时行数立即减少
                // 且幸存行速度不闪烁——D30 下调即时收缩观感）
                let old_speeds: Vec<f64> = t.connections.iter().map(|c| c.speed).collect();
                let piece = chunk_size(t.protocol);
                // 先按 tick 领块同口径结清「已完成未领块」的满块连接（暂停/改
                // 并发可冻结在块完成与下一 tick 领块之间），再汇总在传零头；
                // 待命连接（start=end=0）不持块不入账
                let mut done_blocks = t.chunk_done as u64;
                let mut rem: u64 = 0;
                for c in t.connections.iter() {
                    if c.cap() == 0 {
                        continue;
                    }
                    if c.done >= c.cap() {
                        done_blocks += 1;
                    } else {
                        rem += c.done;
                    }
                }
                let y = chunk_total(t.total, piece) as u64;
                let done_blocks = done_blocks.min(y);
                let (mut conns_new, x) = make_chunk_conns_lease(
                    t.total,
                    done_blocks as u32,
                    rem,
                    concurrency,
                    piece,
                    t.id as u64,
                );
                for (i, c) in conns_new.iter_mut().enumerate() {
                    if let Some(s) = old_speeds.get(i) {
                        c.speed = *s;
                    }
                }
                t.connections = conns_new;
                t.chunk_done = x;
                // 顶部读数同帧重述（ = x×块大小 + 在传零头，与 tick agg 同式）：
                // 无容量溢出时字节守恒不变；收缩溢出时按模型口径显式回落，
                // 避免恢复下载时 tick 重算出现二次跳变
                let rem_new: u64 = t
                    .connections
                    .iter()
                    .filter(|c| c.cap() > 0 && c.done < c.cap())
                    .map(|c| c.done)
                    .sum();
                t.downloaded = (x as u64 * piece + rem_new).min(t.total);
            }
        }
        self.dialog = None;
        self.set_toast("✓ 任务参数已更新，立即生效");
    }

    /// 确认添加（paused=false 立即下载 / true 仅添加）。校验与创建链路两路
    /// 完全一致（FR-01-103）；唯一差别 = D28：仅添加任务落「已暂停」态，
    /// 不占下载槽位、不被槽位调度自动启动
    fn dlg_confirm_add(&mut self, paused: bool) {
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
        // 代理选择一并先读出（借用收窄：后续 sync_sim_sidecars 需要 &mut self）
        let proxy_sel = d.proxy_sel.min(self.proxy_options.len() - 1);

        // 校验码验证：NONE = 不校验（无需校验码）；其余算法校验码必填，
        // 非空时必须是纯十六进制且长度与算法匹配
        let checksum = if ck_type == CK_NONE {
            None
        } else if ck_raw.is_empty() {
            let (algo, need) = CHECKSUM_ALGOS[ck_type.min(CHECKSUM_ALGOS.len() - 1)];
            self.set_toast(format!(
                "⚠ {} 校验码不可为空（需 {} 位十六进制）",
                algo, need
            ));
            if let Some(d) = self.dialog.as_mut() {
                d.focus = 4;
            }
            return;
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

        // v1.28 生产同步（生产 01-add-task-16 同构）：重复任务拒绝——同 URL 同保存
        // 目录已在列表 → toast 提示，不创建新任务（对话框保持打开，与其他校验失败一致）
        if self
            .tasks
            .iter()
            .any(|t| t.url == url && save_dir_of(&t.save_path) == dir)
        {
            self.set_toast("⚠ 任务已存在");
            return;
        }
        self.sync_sim_sidecars();
        let (proto, base_name) = parse_url_task(&url);
        // v1.28/FR-01-26 断点自动接续（路径基）：保存目录存在与「原始 URL 路径
        // 推导名」同名的有效 sidecar（模拟盘：删除任务「保留文件」臂登记的残留，
        // 生产 FR-01-25）即接续——沿用推导原名（不追加序号），且**不做同 URL
        // 校验**（v1.28：异 URL 同推导名也接续；任务列表内同 URL 同目录同名
        // 非完成腿与生产同构保留，实际多被前置的重复拒绝短路，防御性）；
        // demo 无续传一致性环节，误接续（异 URL 同名）由生产 FR-01-22 兜底，
        // 此处直接演示路径基接续的可观察面：同名接续 + 断点进度 + 钦定 toast
        let resumed = self.sim_sidecars.contains(&(dir.clone(), base_name.clone()))
            || self.tasks.iter().any(|t| {
                t.url == url
                    && save_dir_of(&t.save_path) == dir
                    && t.name == base_name
                    && !matches!(t.state, TaskState::Completed | TaskState::Seeding)
            });
        let name = if resumed {
            base_name.clone()
        } else {
            // 重名检测：同目录域内自动追加序号 .1 / .2 …（与生产 namegen::dedupe
            // 口径对齐：以保存目录为域判「任务表 + 盘上」重名，FR-M1-02）
            let mut seq = 1u32;
            let mut n = base_name.clone();
            while self
                .tasks
                .iter()
                .any(|t| t.name == n && save_dir_of(&t.save_path) == dir)
            {
                n = format!("{}.{}", base_name, seq);
                seq += 1;
            }
            n
        };
        // 并发数：取对话框输入（钳制 1-64）；留空或非法时用协议默认（HTTP 4 / BT 20）
        let conns = conns_raw
            .parse::<usize>()
            .map(|v| v.clamp(1, 64))
            .unwrap_or_else(|_| default_conns(proto));
        // v1.20 生产同步：任务 ID = u32 随机数 + 任务全集查重，不再顺序自增
        let id = alloc_task_id(&self.tasks);
        self.prefill_seq = self.prefill_seq.wrapping_add(1);
        let proxy = self.proxy_options[proxy_sel].clone();
        let total = pseudo_total(&name);
        let save_path = format!("{}/{}", dir, name);
        // 路径基接续（v1.28）：模拟盘 sidecar 中的已下载字节以确定性部分进度
        // （按任务 ID 取 25%–59% 档）预置到新任务——「接续」的可观察观感 =
        // 列表/详情直接从断点进度继续，而非从零开始
        let done = if resumed { resumed_done(id, total) } else { 0 };
        let base = 5_000_000.0 + (id as f64 * 1.37e6) % 7_000_000.0;
        let mut t = mk(
            id,
            &name,
            proto,
            &url,
            &save_path,
            total,
            if paused {
                TaskState::Paused
            } else {
                TaskState::Queued
            },
            done,
            conns,
            base,
            "刚刚",
        );
        t.post_process = is_archive(&name);
        if paused {
            // FR-01-103/D28：仅添加 → 已暂停（不占下载槽位、不被槽位调度
            // 自动启动；不设 queued_since——按空格继续时走 Paused 链路）
        } else {
            t.queued_since = Some(Instant::now());
            t.start_delay = 2.0;
        }
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
        // toast（D29 钦定文案 + v1.28 接续两臂）：立即下载/仅添加 × 有无断点
        // 四态；接续两臂文案与生产 dlg_finish_add 逐字对齐
        if paused {
            if resumed {
                self.set_toast(format!(
                    "⏸ 已添加任务 #{}: {}（发现有效断点，已暂停，按空格继续）",
                    id, name
                ));
            } else {
                self.set_toast(format!(
                    "⏸ 已添加任务 #{}: {}（已暂停，按空格开始下载）",
                    id, name
                ));
            }
        } else if resumed {
            self.set_toast(format!(
                "✓ 已添加任务 #{}: {}（发现有效断点，将自动接续）",
                id, name
            ));
        } else {
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
    }

    /// 模拟盘 sidecar 台账同步（v1.28 生产同步轮八）：任务转「已完成/做种」
    /// 后其名下 (目录, 名) 从模拟盘移除（生产 FR-01-20/24：任务转入「已完成」
    /// 时删除对应 sidecar——无论有无校验值）。添加链路做接续判定前调用，
    /// 保证路径基匹配读到与生产等价的磁盘口径。
    fn sync_sim_sidecars(&mut self) {
        let live: HashSet<(String, String)> = self
            .tasks
            .iter()
            .filter(|t| matches!(t.state, TaskState::Completed | TaskState::Seeding))
            .map(|t| (save_dir_of(&t.save_path), t.name.clone()))
            .collect();
        self.sim_sidecars.retain(|k| !live.contains(k));
    }

    fn dlg_activate_delete(&mut self, btn: usize) {
        let Some(idx) = self.sel_idx() else {
            self.dialog = None;
            return;
        };
        let (name, dir, has_sidecar) = {
            let t = &self.tasks[idx];
            (
                t.name.clone(),
                save_dir_of(&t.save_path),
                // 生产 FR-01-20/24：sidecar 存续于非完成任务（转「已完成」即删）
                !matches!(t.state, TaskState::Completed | TaskState::Seeding),
            )
        };
        match btn {
            0 => {
                // v1.28 生产同步（FR-01-25 同构）：仅删除任务——文件与 sidecar 全保留。
                // 非完成任务在模拟盘登记 sidecar 残留：之后添加任何推导出同名文件的
                // URL（异 URL 亦可）到同目录，即路径基断点接续（v1.28/FR-01-26）
                if has_sidecar {
                    self.sim_sidecars.insert((dir, name.clone()));
                }
                self.tasks.remove(idx);
                self.dialog = None;
                self.set_toast(format!("🗑 已删除任务（保留文件）: {}", name));
            }
            1 => {
                // 删除任务和文件：目标文件/.downloading/.ezr 一并清理（FR-01-25）——
                // 模拟盘 sidecar 残留同步移除，之后再添加同名 URL 不再接续
                self.sim_sidecars.remove(&(dir, name.clone()));
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
        // 预填轮换沿用原 next_id 推进口径（prefill_seq：每次确认添加 +1，
        // 首值 100 = verify_ezr.py 场景 C 首开 ADD_POOL[100 % 6] = Fedora）
        let prefill = ADD_POOL[(self.prefill_seq as usize) % ADD_POOL.len()];
        // 默认并发数随预填 URL 的协议类型：HTTP 4 / BT 20（对话框中可修改）
        let (proto, _) = parse_url_task(prefill);
        // 打开时居中（位置不跨次记忆；拖拽位置由渲染层首帧回填）
        self.dialog_pos = None;
        self.dialog_drag = None;
        self.dialog_drag_moved = false;
        self.dialog = Some(Dialog {
            kind: DialogKind::Add,
            url: prefill.to_string(),
            dir: "/srv/downloads".to_string(),
            conns: default_conns(proto).to_string(),
            conns_edited: false,
            // 校验算法默认 NONE（不校验，下拉首行；校验码清空且不可输入）
            ck_type: CK_NONE,
            ck_value: String::new(),
            ck_open: false,
            ck_sel: CK_NONE,
            proxy_sel: self.default_proxy_sel,
            proxy_open: false,
            focus: 0,
            task_name: String::new(),
            task_id: None,
        });
    }

    /// 打开关于窗口（O 快捷键）：居中显示 EZR Download v1.0.0
    pub fn open_about(&mut self) {
        self.about_open = true;
        // about_pos 保留上次拖拽位置；None = 渲染层首帧居中并回填
    }

    /// 关闭关于窗口
    pub fn close_about(&mut self) {
        self.about_open = false;
        self.about_area = None;
        self.about_drag = None;
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
            .unwrap_or(CK_NONE); // 无校验码 → NONE（不校验）
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
        // 打开时居中（同 open_add_dialog）
        self.dialog_pos = None;
        self.dialog_drag = None;
        self.dialog_drag_moved = false;
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
        // 先克隆任务名并结束借用（后续要写 dialog_pos 等字段）
        let name = match self.sel_task() {
            Some(t) => t.name.clone(),
            None => {
                self.set_toast("没有选中的任务");
                return;
            }
        };
        // 打开时居中（同 open_add_dialog）
        self.dialog_pos = None;
        self.dialog_drag = None;
        self.dialog_drag_moved = false;
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
            task_name: name,
            task_id: None,
        });
    }

    // -----------------------------------------------------------------------
    // 鼠标交互：左键选中任务 / 对话框按钮，滚轮滚动列表
    // -----------------------------------------------------------------------

    pub fn on_mouse(&mut self, m: MouseEvent) {
        // 指针位置跟踪（本轮需求修订：任意鼠标事件均携带坐标，
        // 供详情字段值悬停下划线判定；TUI 无法感知指针离开终端，
        // 悬停态以最后一次上报位置为准）
        self.pointer = Some((m.column, m.row));
        // 关于窗口最上层（模态）：左键按住窗口内任意位置拖拽移动；点击窗口
        // 外部 = 无任何操作（全部浮层统一口径，§4.2.5）；无滚动条、无滚轮内容
        if self.about_open {
            match m.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // 模态接管：清除页脚 chip 按压态（模态期页脚不可达）
                    self.footer_press = None;
                    // 右上角 [✗]：按下高亮（原地松开关闭窗口，页脚 chip 同纪律）
                    if Self::pointer_in(&self.popup_close_rect, m.column, m.row) {
                        self.x_press = true;
                        return;
                    }
                    if Self::pointer_in(&self.about_area, m.column, m.row) {
                        // 开始拖拽：记录指针相对窗口左上角的抓取偏移
                        if let Some(r) = self.about_area {
                            self.about_drag = Some((
                                m.column.saturating_sub(r.x),
                                m.row.saturating_sub(r.y),
                            ));
                        }
                    }
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    // [✗] 按压中：拖出热区取消按压、不移动窗口（原地松开才关闭）
                    if self.x_press {
                        self.x_press = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                    } else if let Some((dx, dy)) = self.about_drag {
                        self.about_pos = Some((
                            m.column.saturating_sub(dx),
                            m.row.saturating_sub(dy),
                        ));
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    // [✗] 原地松开 = 关闭关于窗口（拖出取消；页脚 chip 同纪律）
                    if self.x_press {
                        let hit = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        self.x_press = false;
                        self.sb_release();
                        if hit {
                            self.about_open = false;
                        }
                        return;
                    }
                    self.about_drag = None;
                    self.sb_release();
                }
                _ => {}
            }
            return;
        }
        // 任务详情字段值弹窗最上层（模态，本轮需求修订新增）：滚轮滚动内容；
        // 左键按住窗口内任意位置拖拽移动（滚动条上按下除外——拖滑块 = 滚动
        // 内容，§4.1.3）；点击窗口外部 = 无任何操作（全部浮层统一口径，§4.2.5；
        // Esc/q/Q 才是关闭通道，Y 复制内容见 on_key）
        if self.val_open {
            match m.kind {
                MouseEventKind::ScrollUp => self.val_scroll_by(-1),
                MouseEventKind::ScrollDown => self.val_scroll_by(1),
                MouseEventKind::Down(MouseButton::Left) => {
                    // 模态接管：清除页脚 chip 按压态（模态期页脚不可达）
                    self.footer_press = None;
                    // 右上角 [✗]：按下高亮（原地松开关闭窗口，页脚 chip 同纪律）
                    if Self::pointer_in(&self.popup_close_rect, m.column, m.row) {
                        self.x_press = true;
                        return;
                    }
                    // 通用滚动条优先于窗口拖拽：滑块按下 = 内容拖拽滚动准备，
                    // 箭头/轨道按下 = 步进/翻页（与帮助/pieces 同口径）
                    if let Some(bar) = self.val_sb {
                        if let Some(hit) = bar.hit(m.column, m.row) {
                            self.sb_press(SbKey::Val, &bar, hit);
                            return;
                        }
                    }
                    if Self::pointer_in(&self.val_area, m.column, m.row) {
                        // 开始拖拽：记录指针相对窗口左上角的抓取偏移
                        if let Some(r) = self.val_area {
                            self.val_drag = Some((
                                m.column.saturating_sub(r.x),
                                m.row.saturating_sub(r.y),
                            ));
                        }
                    }
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    // [✗] 按压中：拖出热区取消按压、不移动窗口（原地松开才关闭）
                    if self.x_press {
                        self.x_press = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        return;
                    }
                    // 滑块拖拽进行中 = 滚动内容；否则 = 移动窗口。
                    // （箭头按压高亮跟随指针：拖出取消、拖回恢复）
                    self.sb_arrow_follow(m);
                    if self.sb_drag.is_some() {
                        self.sb_drag_move(m);
                    } else if let Some((dx, dy)) = self.val_drag {
                        self.val_pos = Some((
                            m.column.saturating_sub(dx),
                            m.row.saturating_sub(dy),
                        ));
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    // [✗] 原地松开 = 关闭字段值弹窗（拖出取消；页脚 chip 同纪律）
                    if self.x_press {
                        let hit = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        self.x_press = false;
                        self.sb_release();
                        if hit {
                            self.close_val_popup();
                        }
                        return;
                    }
                    self.val_drag = None;
                    self.sb_release();
                }
                _ => {}
            }
            return;
        }
        // 帮助窗口最上层（模态）：滚轮滚动帮助内容；左键按住窗口内任意
        // 位置拖拽移动（滚动条上的按下除外——拖滑块 = 滚动内容，§4.1.3）；
        // 点击窗口外部 = 无任何操作（全部浮层统一口径，§4.2.5）
        if self.help_open {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    self.help_scroll = self.help_scroll.saturating_sub(1);
                }
                MouseEventKind::ScrollDown => self.help_scroll += 1, // 渲染层馴制
                MouseEventKind::Down(MouseButton::Left) => {
                    // 模态接管：清除页脚 chip 按压态（模态期页脚不可达）
                    self.footer_press = None;
                    // 右上角 [✗]：按下高亮（原地松开关闭窗口，页脚 chip 同纪律）
                    if Self::pointer_in(&self.popup_close_rect, m.column, m.row) {
                        self.x_press = true;
                        return;
                    }
                    // 通用滚动条优先于窗口拖拽：滑块按下 = 内容拖拽滚动准备，
                    // 箭头/轨道按下 = 步进/翻页（修复：拖滚动条误移窗口）
                    if let Some(bar) = self.help_sb {
                        if let Some(hit) = bar.hit(m.column, m.row) {
                            self.sb_press(SbKey::Help, &bar, hit);
                            return;
                        }
                    }
                    if Self::pointer_in(&self.help_area, m.column, m.row) {
                        // 开始拖拽：记录指针相对窗口左上角的抓取偏移
                        if let Some(r) = self.help_area {
                            self.help_drag = Some((
                                m.column.saturating_sub(r.x),
                                m.row.saturating_sub(r.y),
                            ));
                        }
                    }
                    // 点击窗口外部 = 无任何操作（模态统一口径：不关闭、不
                    // 穿透到底层面板；Esc/H/q/Q 才是关闭通道，§4.2.5）
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    // [✗] 按压中：拖出热区取消按压、不移动窗口（原地松开才关闭）
                    if self.x_press {
                        self.x_press = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        return;
                    }
                    // 滑块拖拽进行中 = 滚动内容；否则 = 移动窗口。
                    // （箭头按压高亮跟随指针：拖出取消、拖回恢复）
                    self.sb_arrow_follow(m);
                    if self.sb_drag.is_some() {
                        self.sb_drag_move(m);
                    } else if let Some((dx, dy)) = self.help_drag {
                        self.help_pos = Some((
                            m.column.saturating_sub(dx),
                            m.row.saturating_sub(dy),
                        ));
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    // [✗] 原地松开 = 关闭帮助窗口（拖出取消；页脚 chip 同纪律）
                    if self.x_press {
                        let hit = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        self.x_press = false;
                        self.sb_release();
                        if hit {
                            self.help_open = false;
                        }
                        return;
                    }
                    self.help_drag = None;
                    self.sb_release();
                }
                _ => {}
            }
            return;
        }
        // pieces 弹窗最上层（模态）：滚轮滚动分块网格；左键按住弹窗内空白处
        // 拖拽移动窗口（滚动条上按下除外——拖滑块 = 滚动网格，§4.1.3）；
        // 点击弹窗外部 = 无任何操作（全部浮层统一口径，§4.2.5）
        if self.pieces_open {
            match m.kind {
                MouseEventKind::ScrollUp => self.pieces_scroll_by(-2),
                MouseEventKind::ScrollDown => self.pieces_scroll_by(2),
                MouseEventKind::Down(MouseButton::Left) => {
                    // 模态接管：清除页脚 chip 按压态
                    self.footer_press = None;
                    // 右上角 [✗]：按下高亮（原地松开关闭弹窗，页脚 chip 同纪律）
                    if Self::pointer_in(&self.popup_close_rect, m.column, m.row) {
                        self.x_press = true;
                        return;
                    }
                    // 通用滚动条优先于窗口拖拽（修复：拖滚动条误移窗口）
                    if let Some(bar) = self.pieces_sb {
                        if let Some(hit) = bar.hit(m.column, m.row) {
                            self.sb_press(SbKey::Pieces, &bar, hit);
                            return;
                        }
                    }
                    if Self::pointer_in(&self.pieces_area, m.column, m.row) {
                        // 开始拖拽：记录指针相对弹窗左上角的抓取偏移
                        if let Some(r) = self.pieces_area {
                            self.pieces_drag = Some((
                                m.column.saturating_sub(r.x),
                                m.row.saturating_sub(r.y),
                            ));
                        }
                    }
                    // 点击弹窗外部 = 无任何操作（模态统一口径：不关闭、不
                    // 穿透到底层面板；Esc/P/q/Q 才是关闭通道，§4.2.5）
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    // [✗] 按压中：拖出热区取消按压、不移动窗口（原地松开才关闭）
                    if self.x_press {
                        self.x_press = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        return;
                    }
                    // 滑块拖拽进行中 = 滚动网格；否则移动窗口：弹窗左上角 =
                    // 指针 - 抓取偏移（渲染层钳制在终端内）。
                    // （箭头按压高亮跟随指针：拖出取消、拖回恢复）
                    self.sb_arrow_follow(m);
                    if self.sb_drag.is_some() {
                        self.sb_drag_move(m);
                    } else if let Some((dx, dy)) = self.pieces_drag {
                        self.pieces_pos = Some((
                            m.column.saturating_sub(dx),
                            m.row.saturating_sub(dy),
                        ));
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    // [✗] 原地松开 = 关闭分块弹窗（拖出取消；页脚 chip 同纪律）
                    if self.x_press {
                        let hit = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        self.x_press = false;
                        self.sb_release();
                        if hit {
                            self.close_pieces();
                        }
                        return;
                    }
                    self.pieces_drag = None;
                    self.sb_release();
                }
                _ => {}
            }
            return;
        }
        // 对话框优先：支持左键拖拽移动窗口（按住弹窗内任意位置拖动，位置
        // 钳制在终端内）；下拉框展开时只认选项点击（点外部收起）；未拖动时
        // 松开左键视为点击（聚焦字段行 / 激活按钮，修订：点击在 Up 触发，
        // 与拖拽共用左键互不干扰）
        if self.dialog.is_some() {
            match m.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let ck_open = self.dialog.as_ref().is_some_and(|d| d.ck_open);
                    let proxy_open = self.dialog.as_ref().is_some_and(|d| d.proxy_open);
                    // 模态接管：清除页脚 chip 按压态
                    self.footer_press = None;
                    if ck_open {
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
                        let ck_rects = self.dlg_ck_rects.clone();
                        if let Some(i) = hit(&ck_rects) {
                            let d = self.dialog.as_mut().unwrap();
                            d.ck_type = i;
                            d.ck_sel = i;
                            // 选中 NONE（不校验）：校验码清空且不可输入
                            if d.ck_type == CK_NONE {
                                d.ck_value.clear();
                            }
                            d.ck_open = false;
                        } else if let Some(d) = self.dialog.as_mut() {
                            // 点击下拉框外部：仅收起下拉框
                            d.ck_open = false;
                        }
                        return;
                    }
                    if proxy_open {
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
                    // 右上角 [✗]：按下高亮（原地松开 = 取消对话框，Esc 同口径）
                    if Self::pointer_in(&self.popup_close_rect, m.column, m.row) {
                        self.x_press = true;
                        return;
                    }
                    // 弹窗内按下：开始拖拽（记录抓取偏移；是否算点击在 Up 判定）；
                    // 弹窗外按下：忽略
                    if Self::pointer_in(&self.dialog_area, m.column, m.row) {
                        if let Some(r) = self.dialog_area {
                            self.dialog_drag = Some((
                                m.column.saturating_sub(r.x),
                                m.row.saturating_sub(r.y),
                            ));
                            self.dialog_drag_moved = false;
                        }
                    }
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    // [✗] 按压中：拖出热区取消按压、不移动窗口（原地松开才关闭）
                    if self.x_press {
                        self.x_press = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        return;
                    }
                    // 拖拽移动：弹窗左上角 = 指针 - 抓取偏移（渲染层钳制在终端内）
                    if let Some((dx, dy)) = self.dialog_drag {
                        let new = (m.column.saturating_sub(dx), m.row.saturating_sub(dy));
                        if self.dialog_pos != Some(new) {
                            self.dialog_pos = Some(new);
                            self.dialog_drag_moved = true;
                        }
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    // [✗] 原地松开 = 取消对话框（与 Esc/取消按钮同口径；拖出取消）
                    if self.x_press {
                        let hit = Self::pointer_in(&self.popup_close_rect, m.column, m.row);
                        self.x_press = false;
                        self.sb_release();
                        if hit {
                            self.dialog = None;
                        }
                        return;
                    }
                    // 对话框无滚动条；此处兜底清理按压/拖拽态（如按住滚动条
                    // 箭头期间经键盘打开了对话框）
                    self.sb_release();
                    if self.dialog_drag.is_some() {
                        let moved = self.dialog_drag_moved;
                        self.dialog_drag = None;
                        self.dialog_drag_moved = false;
                        if !moved {
                            // 未拖动 = 点击：聚焦字段行 / 激活按钮（原有 Down 语义）
                            self.dialog_click(m);
                        }
                    }
                }
                _ => {}
            }
            return;
        }
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // 鼠标点击选择面板（需求修订）：点击头部页签切换页签并聚焦
                // 任务列表；点击详情/并发/列表任意处聚焦该面板；点任务/连接
                // 行同时选中明细
                let in_rect = |r: Rect| {
                    m.column >= r.x
                        && m.column < r.x.saturating_add(r.width)
                        && m.row >= r.y
                        && m.row < r.y.saturating_add(r.height)
                };
                // 页脚命令 chips（v0.2 页脚双行修订）：Down 按压反色高亮
                //（点击效果 + 400ms 余辉），原地松开执行对应命令（与键盘
                // 快捷键同口径）；拖到别处松开 = 取消
                let footer_hit = self
                    .footer_cmd_rects
                    .iter()
                    .find(|(r, _)| in_rect(*r))
                    .map(|(_, i)| *i);
                if let Some(i) = footer_hit {
                    self.footer_press = Some(i);
                    self.footer_flash = Some((i, Instant::now() + Duration::from_millis(400)));
                    return;
                }
                self.footer_press = None;
                let tab_hit = self
                    .tab_rects
                    .iter()
                    .find(|(r, _)| in_rect(*r))
                    .map(|(_, i)| *i);
                if let Some(i) = tab_hit {
                    if i != self.filter {
                        self.filter = i;
                        self.scroll_to_selection();
                    }
                    // 页签归属任务列表面板：点击页签同时聚焦列表
                    self.focus = Panel::List;
                    return;
                }
                // 修订-5/7：任务详情面板点击字段名复制——URL → url 值、校验 → 校验码值；
                // 本轮需求修订：扩展到失败 → 失败原因值、路径 → 保存路径值。
                // 系统剪贴板（arboard）优先，无图形会话回退 OSC 52；
                // 反馈仅在底部提示（toast），面板本身无提示信息
                // （点击复制同时聚焦详情面板）
                if Self::pointer_in(&self.detail_url_rect, m.column, m.row) {
                    self.focus = Panel::Detail;
                    if let Some(t) = self.sel_task() {
                        let url = t.url.clone();
                        self.copy_to_clipboard(&url);
                        self.set_toast("已复制 url");
                    }
                    return;
                }
                if Self::pointer_in(&self.detail_ck_rect, m.column, m.row) {
                    self.focus = Panel::Detail;
                    if let Some(t) = self.sel_task() {
                        if let Some(ck) = &t.checksum {
                            let v = ck.value.clone();
                            self.copy_to_clipboard(&v);
                            self.set_toast("已复制 校验码");
                        }
                    }
                    return;
                }
                // 本轮需求修订：「失败」字段名点击 = 复制失败原因值
                if Self::pointer_in(&self.detail_fail_rect, m.column, m.row) {
                    self.focus = Panel::Detail;
                    if let Some(t) = self.sel_task() {
                        let v = t.error.clone().unwrap_or_else(|| "未知错误".to_string());
                        self.copy_to_clipboard(&v);
                        self.set_toast("已复制 失败原因");
                    }
                    return;
                }
                // 本轮需求修订：「路径」字段名点击 = 复制保存路径值
                if Self::pointer_in(&self.detail_save_rect, m.column, m.row) {
                    self.focus = Panel::Detail;
                    if let Some(t) = self.sel_task() {
                        let v = t.save_path.clone();
                        self.copy_to_clipboard(&v);
                        self.set_toast("已复制 保存路径");
                    }
                    return;
                }
                // 本轮需求修订：点击字段值 = 打开完整值弹窗（模态；
                // 悬停下划线反馈见 ui 层，热区随帧回填）
                let val_hit = self
                    .detail_val_rects
                    .iter()
                    .find(|(r, _)| in_rect(*r))
                    .map(|(_, f)| *f);
                if let Some(field) = val_hit {
                    self.focus = Panel::Detail;
                    self.open_val_popup(field);
                    return;
                }
                // 详情面板任意处点击 = 聚焦
                if Self::pointer_in(&self.detail_area, m.column, m.row) {
                    self.focus = Panel::Detail;
                    return;
                }
                // 通用滚动条（列表垂直 / 并发垂直+水平）：滑块拖拽、箭头点击·
                // 按住连发、轨道翻页——全部实例同一组件同一行为（§4.1.3）。
                // 命中滚动条同时聚焦所属面板（点击面板聚焦口径的自然延伸）
                for (bar, key) in [
                    (self.list_sb, SbKey::List),
                    (self.conns_v_sb, SbKey::ConnsV),
                    (self.conns_h_sb, SbKey::ConnsH),
                ] {
                    let Some(bar) = bar else { continue };
                    if let Some(hit) = bar.hit(m.column, m.row) {
                        self.focus = match key {
                            SbKey::ConnsV | SbKey::ConnsH => Panel::Conns,
                            _ => Panel::List,
                        };
                        self.sb_press(key, &bar, hit);
                        return;
                    }
                }
                // 并发连接面板点击 = 聚焦 + 选行（表头 1 行之下为数据行）
                if let Some(a) = self.conns_area {
                    let in_x = m.column >= a.x && m.column < a.x.saturating_add(a.width);
                    let in_y = m.row >= a.y && m.row < a.y.saturating_add(a.height);
                    if in_x && in_y {
                        self.focus = Panel::Conns;
                        let data_top = a.y + 1; // 表头行之下
                        if m.row >= data_top {
                            let idx = (m.row - data_top) as usize;
                            if idx < self.visible_conns_rows {
                                let n = self.conn_count();
                                let ci = self.conns_scroll + idx;
                                if ci < n {
                                    self.conns_sel = ci;
                                }
                            }
                        }
                        return;
                    }
                }
                if let Some(a) = self.list_area {
                    let in_x = m.column >= a.x && m.column < a.x.saturating_add(a.width);
                    let in_y = m.row >= a.y && m.row < a.y.saturating_add(a.height);
                    if in_x && in_y {
                        // 点击列表面板 = 聚焦列表 + 选中任务
                        self.focus = Panel::List;
                        let idx = (m.row - a.y) as usize / ITEM_HEIGHT as usize;
                        let ti = self.scroll + idx;
                        if ti < self.filtered().len() {
                            self.selected = ti;
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                // 指针在并发连接面板上：滚动该面板；否则滚动任务列表
                if Self::pointer_in(&self.conns_area, m.column, m.row) {
                    self.conns_scroll = self.conns_scroll.saturating_sub(2);
                } else {
                    self.scroll = self.scroll.saturating_sub(2);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // 通用滚动条滑块拖拽 = 比例滚动内容（主界面无窗口拖拽语义，
                // 滑块之外的主界面 Drag 事件本就无操作）；箭头按压高亮跟随
                // 指针：按住箭头拖出 = 取消高亮、拖回恢复
                self.sb_arrow_follow(m);
                if self.sb_drag.is_some() {
                    self.sb_drag_move(m);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                // 通用滚动条：松开结束滑块拖拽/箭头·轨道连发
                self.sb_release();
                // 页脚命令 chip：按压后原地松开 = 执行对应命令（与键盘快捷键
                // 同口径）；拖到别处松开 = 取消。其余 Up 事件主界面无语义。
                if let Some(pressed) = self.footer_press.take() {
                    let still_on = self.footer_cmd_rects.iter().any(|(r, i)| {
                        *i == pressed
                            && m.column >= r.x
                            && m.column < r.x.saturating_add(r.width)
                            && m.row >= r.y
                            && m.row < r.y.saturating_add(r.height)
                    });
                    if still_on {
                        self.run_footer_cmd(pressed);
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                // 指针在并发连接面板上：滚动该面板；否则滚动任务列表
                if Self::pointer_in(&self.conns_area, m.column, m.row) {
                    // 到底即停（滚到底闪烁修复）：事件内按可视行数立即钳制
                    // （同任务列表口径）。此前 +2 后要等 tick 同步钳制，中间帧
                    // 渲染出底部空白行又被钳回——窗口反复变化即闪烁。
                    let vis = self.visible_conns_rows.max(1);
                    let maxs = self.conn_count().saturating_sub(vis);
                    self.conns_scroll = (self.conns_scroll + 2).min(maxs);
                } else {
                    let vis = self.visible_rows.max(1);
                    let maxs = self.filtered().len().saturating_sub(vis);
                    self.scroll = (self.scroll + 2).min(maxs);
                }
            }
            _ => {}
        }
    }

    /// 指针是否位于给定矩形内
    fn pointer_in(area: &Option<Rect>, x: u16, y: u16) -> bool {
        matches!(area, Some(a) if x >= a.x && x < a.x.saturating_add(a.width) && y >= a.y && y < a.y.saturating_add(a.height))
    }

    // -----------------------------------------------------------------------
    // 通用滚动条交互（§4.1.3：全部 panel/弹窗同一契约）
    //
    //   滑块按住拖拽   = 比例滚动内容（浮层内同样滚内容——修复「拖滚动条
    //                    移动窗口」的旧分歧；窗口拖拽仅限空白处/边框外）
    //   箭头单击/按住  = 步进 1 单位（按住 0.4s 起振、0.12s 连发，tick 驱动）
    //   轨道单击/按住  = 朝点击方向整页滚动
    //   键盘方向键     = ↑↓ 纵向（A 列表 = 选择跟随）、←→ 横向（并发面板）
    // -----------------------------------------------------------------------

    /// 滚动条滚动上限（按实例取各自的 total − page）
    fn sb_max(&self, key: SbKey) -> usize {
        match key {
            // 任务列表：条目数 − 可视块数（visible_rows 由 ui 层回填）
            SbKey::List => self.filtered().len().saturating_sub(self.visible_rows.max(1)),
            // 并发垂直：连接行数 − 可视行数（末页不空滚，同滚轮钳制口径）
            SbKey::ConnsV => self.conn_count().saturating_sub(self.visible_conns_rows.max(1)),
            // 并发水平：内容列宽 − 内框宽（ui 层每帧回填）
            SbKey::ConnsH => self.conns_max_hscroll,
            SbKey::Pieces => self.pieces_max_scroll,
            SbKey::Help => self.help_max_scroll,
            SbKey::Val => self.val_max_scroll,
        }
    }

    /// 滚动条滚动步进（delta 正 = 向下/向右；事件内立即钳制，末页即停）
    fn sb_scroll(&mut self, key: SbKey, delta: i32) {
        let max = self.sb_max(key);
        match key {
            SbKey::List => {
                self.scroll = clamp_offset(self.scroll as i64 + delta as i64, max);
            }
            SbKey::ConnsV => {
                self.conns_scroll = clamp_offset(self.conns_scroll as i64 + delta as i64, max);
            }
            SbKey::ConnsH => {
                self.conns_hscroll = clamp_offset(self.conns_hscroll as i64 + delta as i64, max);
            }
            SbKey::Pieces => {
                self.pieces_scroll = clamp_offset(self.pieces_scroll as i64 + delta as i64, max);
            }
            SbKey::Help => {
                self.help_scroll = clamp_offset(self.help_scroll as i64 + delta as i64, max);
            }
            SbKey::Val => {
                self.val_scroll = clamp_offset(self.val_scroll as i64 + delta as i64, max);
            }
        }
    }

    /// 滑块拖拽的绝对定位（pos 已由 Bar::pos_from_thumb 反解，直接钳制写回）
    fn sb_set(&mut self, key: SbKey, pos: usize) {
        let max = self.sb_max(key);
        let pos = pos.min(max);
        match key {
            SbKey::List => self.scroll = pos,
            SbKey::ConnsV => self.conns_scroll = pos,
            SbKey::ConnsH => self.conns_hscroll = pos,
            SbKey::Pieces => self.pieces_scroll = pos,
            SbKey::Help => self.help_scroll = pos,
            SbKey::Val => self.val_scroll = pos,
        }
    }

    /// 轨道点击翻页步长 = 可视单位数（整页；与滚轮/键盘翻页互不影响）
    fn sb_page(&self, key: SbKey) -> i32 {
        key.bar(self).map(|b| b.page.max(1) as i32).unwrap_or(1)
    }

    /// 滚动条 Down 事件分发（滑块 → 开始拖拽；箭头/轨道 → 步进/翻页并进入连发）
    fn sb_press(&mut self, key: SbKey, bar: &scrollbar::Bar, hit: scrollbar::Hit) {
        match hit {
            scrollbar::Hit::Thumb(grab) => {
                self.sb_drag = Some((key, grab));
            }
            scrollbar::Hit::Head => {
                let step = key.step();
                self.sb_scroll(key, -step);
                self.sb_pressed = Some((key, scrollbar::Arrow::Head));
                self.sb_flash = Some((
                    key,
                    scrollbar::Arrow::Head,
                    Instant::now() + Duration::from_millis(400),
                ));
                self.sb_hold = Some(SbHold {
                    key,
                    dir: -1,
                    page: false,
                    since: Instant::now(),
                    last: Instant::now(),
                });
            }
            scrollbar::Hit::Tail => {
                let step = key.step();
                self.sb_scroll(key, step);
                self.sb_pressed = Some((key, scrollbar::Arrow::Tail));
                self.sb_flash = Some((
                    key,
                    scrollbar::Arrow::Tail,
                    Instant::now() + Duration::from_millis(400),
                ));
                self.sb_hold = Some(SbHold {
                    key,
                    dir: 1,
                    page: false,
                    since: Instant::now(),
                    last: Instant::now(),
                });
            }
            scrollbar::Hit::Track(t) => {
                // 朝点击方向整页滚动：点击滑块上方/左侧 = 向头翻页，反之为向尾
                let dir = if t < bar.thumb_pos() { -1 } else { 1 };
                let page = self.sb_page(key);
                self.sb_scroll(key, dir * page);
                self.sb_hold = Some(SbHold {
                    key,
                    dir,
                    page: true,
                    since: Instant::now(),
                    last: Instant::now(),
                });
            }
        }
    }

    /// 滚动条滑块拖拽（Drag 事件）：指针轨道位置 − 抓取偏移 → 反解滚动偏移。
    /// 修复点：浮层（pieces/帮助）里拖滚动条此前会按「窗口拖拽」移动窗口，
    /// 现在拖滑块 = 滚动内容，拖空白处 = 移动窗口，两种拖拽互不干扰。
    fn sb_drag_move(&mut self, m: MouseEvent) {
        let Some((key, grab)) = self.sb_drag else { return };
        let Some(bar) = key.bar(self) else { return };
        let t = bar.track_index_at(m.column, m.row);
        let pos = bar.pos_from_thumb(t.saturating_sub(grab));
        self.sb_set(key, pos);
    }

    /// 左键松开：结束滑块拖拽与箭头/轨道连发，并清除箭头按压高亮
    fn sb_release(&mut self) {
        self.sb_drag = None;
        self.sb_hold = None;
        self.sb_pressed = None;
    }

    /// Drag 事件中箭头按压高亮的跟随：按住箭头期间指针仍在该箭头上 =
    /// 保持高亮，拖出 = 取消高亮（连发不中断，既有手感不变），拖回恢复。
    /// 滑块拖拽中不高亮箭头；轨道翻页按压（page）无箭头高亮语义。
    fn sb_arrow_follow(&mut self, m: MouseEvent) {
        if self.sb_drag.is_some() {
            self.sb_pressed = None;
            return;
        }
        let Some(h) = self.sb_hold else {
            return;
        };
        if h.page {
            return;
        }
        let arrow = if h.dir < 0 {
            scrollbar::Arrow::Head
        } else {
            scrollbar::Arrow::Tail
        };
        let expect = match arrow {
            scrollbar::Arrow::Head => scrollbar::Hit::Head,
            scrollbar::Arrow::Tail => scrollbar::Hit::Tail,
        };
        let on = h.key.bar(self).and_then(|b| b.hit(m.column, m.row)) == Some(expect);
        self.sb_pressed = if on { Some((h.key, arrow)) } else { None };
    }

    /// 渲染层查询：某滚动条实例的箭头当前是否处于按压/余辉高亮态
    ///（渲染层每帧调用，把结果传给 `scrollbar::render`）
    pub fn sb_arrow_pressed(&self, key: SbKey) -> Option<scrollbar::Arrow> {
        if let Some((k, a)) = self.sb_pressed {
            if k == key {
                return Some(a);
            }
        }
        if let Some((k, a, until)) = self.sb_flash {
            if k == key && Instant::now() < until {
                return Some(a);
            }
        }
        None
    }

    /// tick 驱动的按住连发（100ms tick；起振 0.4s、周期 0.12s）
    fn sb_hold_tick(&mut self, now: Instant) {
        let Some(h) = self.sb_hold else { return };
        if now < h.due() {
            return;
        }
        let step = if h.page { self.sb_page(h.key) } else { h.key.step() };
        self.sb_scroll(h.key, h.dir * step);
        if let Some(h) = &mut self.sb_hold {
            h.last = now;
        }
    }

    /// 页脚命令 chip 是否处于点击高亮（按压中，或 400ms 余辉内）
    pub fn footer_chip_hot(&self, i: usize) -> bool {
        if self.footer_press == Some(i) {
            return true;
        }
        matches!(
            self.footer_flash,
            Some((fi, until)) if fi == i && Instant::now() < until
        )
    }

    /// 页脚命令 chip 执行（v0.2 页脚双行修订）：与同名键盘快捷键完全同口径——
    /// A 添加 / D 删除 / M 修改 / Q 退出 / H 帮助。仅主界面态可达（模态浮层
    /// 打开时鼠标事件先交给浮层，P2 模态优先级）。
    fn run_footer_cmd(&mut self, i: usize) {
        match i {
            0 => self.open_add_dialog(),
            1 => self.open_delete_dialog(),
            2 => self.open_modify_dialog(),
            3 => self.quit = true,
            4 => {
                self.help_open = true;
                self.help_scroll = 0;
                // help_pos 保留上次拖拽位置；None = 渲染层首帧居中并回填
            }
            _ => {}
        }
    }

    /// 对话框左键点击（拖拽未移动的 Up 事件）：字段行聚焦 / 按钮激活。
    /// 下拉行（Add 校验=3/Modify 校验=1、代理恒为末二：Add 5/Modify 3）
    /// 点击时展开下拉框。
    fn dialog_click(&mut self, m: MouseEvent) {
        let kind = self.dialog.as_ref().map(|d| d.kind);
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
        let field_rects = self.dlg_field_rects.clone();
        let btn_rects = self.dlg_btn_rects.clone();
        if let Some(i) = hit(&field_rects) {
            let d = self.dialog.as_mut().unwrap();
            d.focus = i;
            let ck_i = if kind == Some(DialogKind::Add) { 3 } else { 1 };
            let proxy_i = if kind == Some(DialogKind::Add) { 5 } else { 3 };
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

    /// bracketed paste 事件入口（FR-01-06 v1.17 同步）：粘贴覆盖 Add 与
    /// Modify 两对话框的文本字段（Add：URL/目录/并发/校验码；Modify：并发/
    /// 校验码），与逐键输入同口径；算法/代理下拉展开、Delete 对话框、
    /// 无对话框时忽略（避免粘贴触发按钮/导航；proxy 门为 belt-and-braces——
    /// 展开时焦点在下拉行，字段路由天然 no-op，显式拦截防回归）
    pub fn on_paste(&mut self, text: &str) {
        let Some(d) = self.dialog.as_mut() else {
            return;
        };
        if !matches!(d.kind, DialogKind::Add | DialogKind::Modify) || d.ck_open || d.proxy_open {
            return;
        }
        let (kind, focus) = (d.kind, d.focus);
        dlg_apply_paste(kind, d, focus, text);
    }

    /// ↑/↓（并发连接面板聚焦时）：在并发明细中上下移动选中行（跟随滚动保持可见）
    fn move_conn_sel(&mut self, delta: i32) {
        let Some(idx) = self.sel_idx() else { return };
        // 仅「下载中」/「做种中」任务有并发明细可选
        if !self.tasks[idx].state.shows_conns() {
            self.set_toast("当前状态无并发明细（仅下载中/做种中可选）");
            return;
        }
        let n = self.tasks[idx].connections.len();
        if n == 0 {
            self.set_toast("当前任务没有并发连接明细");
            return;
        }
        let new = (self.conns_sel as i32 + delta).clamp(0, n as i32 - 1) as usize;
        self.conns_sel = new;
        // 选中行越出可视窗口时跟随滚动（visible_conns_rows 由 ui 层回填）
        let vis = self.visible_conns_rows.max(1);
        if new < self.conns_scroll {
            self.conns_scroll = new;
        } else if new >= self.conns_scroll + vis {
            self.conns_scroll = new + 1 - vis;
        }
    }

    /// B：断开（阻塞并移除）当前选中的并发连接（仅 BT 任务），明细行从屏幕移除。
    /// 断开时未传完的块按「由其他节点补完」记入已完成块数（演示口径，
    /// 保证块队列持续推进、任务仍可正常完成；进度前跳不超过一个块）。
    fn disconnect_conn(&mut self) {
        let Some(idx) = self.sel_idx() else { return };
        // 仅「下载中」/「做种中」任务有可断开的活跃连接（其余状态明细为空）
        if !self.tasks[idx].state.shows_conns() {
            self.set_toast("当前状态无可断开的并发连接（仅下载中/做种中）");
            return;
        }
        if !self.tasks[idx].protocol.is_bt() {
            self.set_toast("仅 BT 任务支持断开并发连接（B）");
            return;
        }
        if self.tasks[idx].connections.is_empty() {
            self.set_toast("当前任务没有可断开的并发连接");
            return;
        }
        if self.conns_sel >= self.tasks[idx].connections.len() {
            self.conns_sel = self.tasks[idx].connections.len() - 1;
        }
        let piece = chunk_size(self.tasks[idx].protocol);
        let (masked, left) = {
            let t = &mut self.tasks[idx];
            let c = t.connections.remove(self.conns_sel);
            if c.cap() > 0 && c.done < c.cap() {
                let y = chunk_total(t.total, piece);
                t.chunk_done = (t.chunk_done + 1).min(y);
            }
            if self.conns_sel >= t.connections.len() {
                self.conns_sel = t.connections.len().saturating_sub(1);
            }
            (c.ip_masked(), t.connections.len())
        };
        self.set_toast(format!("✂ 已断开连接 {}（剩余 {} 条）", masked, left));
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
                // 生产 v1.21/FR-01-33 修订同步（操作者 20261012 指令）：继续 =
                // 一律重新排队——「有空闲槽直接开跑」路径废除，统一经等待队列
                // 由槽位调度启动（磁盘预检/列表序公平分配/清零口径随队列单源）；
                // v1.23 同步：失败挂起在挂起时即已重置 error/retries，恢复路径
                // 数据上与普通暂停无差别；不支持续传的任务获槽启动时由 tick
                // 从头下载（既有路径承接）
                let resumable = self.tasks[idx].resumable;
                let verify_fail = self.tasks[idx].fail_kind == Some(FailKind::Verify);
                self.requeue_reset(idx);
                self.tasks[idx].queued_since = Some(Instant::now());
                self.tasks[idx].start_delay = 1.5;
                if verify_fail {
                    // 校验失败挂起的恢复：与 R 同分支（重新校验/直判）
                    self.requeue_verify_branch(idx);
                    return;
                }
                let toast = if resumable {
                    format!("▶ 继续下载（从断点恢复，已进入等待队列）: {name}")
                } else {
                    format!("⚠ 服务器不支持断点续传，将从头下载（已进入等待队列）: {name}")
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
                // 生产 v1.6/FR-01-92 + v1.21 合并口径 + v1.22/v1.23 展示与数据统一同步：
                // 失败任务按空格 = 暂停（挂起自动重试）——清倒计时、释放槽位、转「已暂停」
                // 并即时重置 error=None、retries=1（v1.23 操作者 20261012 bug 报告直判：
                // 数据层与普通暂停完全一致）；（原「空格 = 立即重新排队」口径废止）
                self.tasks[idx].state = TaskState::Paused;
                self.tasks[idx].error = None;
                self.tasks[idx].retries = 1;
                self.tasks[idx].retry_in = None;
                self.tasks[idx].has_slot = false;
                self.set_toast(format!(
                    "⏸ 已暂停（失败），不再自动重试；按空格恢复: {name}"
                ));
            }
            TaskState::Seeding => {
                // 做种中 → 暂停做种，进入「已完成」（并发连接明细清空、活跃 0/0）
                self.tasks[idx].state = TaskState::Completed;
                self.tasks[idx].upload_speed = 0.0;
                self.tasks[idx].connections.clear();
                self.set_toast(format!("⏸ 已停止做种（进入已完成）: {}", name));
            }
            TaskState::Completed => self.set_toast("该任务已完成，无需操作".to_string()),
            _ => self.set_toast("当前状态下不可暂停/继续".to_string()),
        }
    }

    pub fn retry(&mut self) {
        let Some(idx) = self.sel_idx() else { return };
        if self.tasks[idx].state == TaskState::Failed {
            // 仅「已失败」可 R；挂起态（Paused）不可 R（生产 v1.19/
            // FR-01-92 收窄 + v1.21 合并口径同步：空格为唯一恢复键，挂起态按 R
            // 落兜底提示）。手动重试 → 已重试次数重置为 1；校验失败任务按
            // 重校验/不校验分支处理（生产 v1.17/FR-01-51 + D34 同步）
            self.requeue_failed(idx);
        } else {
            self.set_toast("仅「已失败」的任务可以重试".to_string());
        }
    }

    /// 重新排队公共字段重置（R 手动重试与空格恢复共用，生产 v1.21 同步）：
    /// 转等待中、错误/倒计时/失败类别清除、计数重置、让槽
    fn requeue_reset(&mut self, idx: usize) {
        self.tasks[idx].state = TaskState::Queued;
        self.tasks[idx].error = None;
        self.tasks[idx].retry_in = None;
        self.tasks[idx].fail_kind = None;
        self.tasks[idx].retries = 1;
        self.tasks[idx].made_progress = false;
        self.tasks[idx].has_slot = false;
    }

    /// 校验失败重排分支（R 与空格恢复共用，生产 v1.16/D10 + v1.17/FR-01-51
    /// 三分支同步；demo 无伴随文件 → 两分支）：
    /// ① 任务显式校验值在 → **重新校验（无块重传）**；
    /// ② 校验码已清空 → v1.18 文件完整性直判（生产 FR-01-51 v1.18 同步）。
    fn requeue_verify_branch(&mut self, idx: usize) {
        // 分支①：显式校验值在 → 重新校验（无块重传）：不重置 downloaded/
        // chunk_done/connections（生产 D10：全块已完成，重传只会丢弃
        // 完好的已下载数据），直接落「校验中」占槽位
        if self.tasks[idx].checksum.is_some() {
            self.tasks[idx].state = TaskState::Verifying;
            self.tasks[idx].has_slot = true;
            self.tasks[idx].verify_started = Some(Instant::now());
            let name = self.tasks[idx].name.clone();
            self.set_toast(format!("↻ 重新校验（无块重传）: {name}"));
            return;
        }
        // 分支②（校验码 = None）：v1.18 文件完整性直判——校验失败时全块
        // 已完成、文件已 100% 落盘 → 直接收尾「已完成/无校验」。demo 无外部
        // 文件改动面，直判恒命中（生产侧文件缺失/大小不符时作废断点从头
        // 重下，demo 不可达不建模）
        let name = self.tasks[idx].name.clone();
        self.tasks[idx].state = TaskState::Completed;
        self.tasks[idx].verify_ok = None;
        self.tasks[idx].speed = 0.0;
        self.tasks[idx].connections.clear();
        self.set_toast(format!("✓ 文件已完整，直接完成（无校验）: {name}"));
    }

    /// 失败任务手动重试（R，FR-01-34/D10；旧「空格恢复」入参形态已随生产
    /// v1.21 统一路径并入 toggle_pause 的 Paused 臂）。其余失败沿用断点续传
    /// （已下载块的字节保持有效）；原演示口径「校验失败清除断点从头重下」
    /// 已废止（生产 v1.17 改判）。
    fn requeue_failed(&mut self, idx: usize) {
        let verify_fail = self.tasks[idx].fail_kind == Some(FailKind::Verify);
        self.requeue_reset(idx);
        if verify_fail {
            self.requeue_verify_branch(idx);
            return;
        }
        self.tasks[idx].queued_since = Some(Instant::now());
        self.tasks[idx].start_delay = 3.0;
        let name = self.tasks[idx].name.clone();
        self.set_toast(format!("↻ 手动重试（等待中）: {name}"));
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

    // -----------------------------------------------------------------------
    // pieces 分块弹窗（P 快捷键）
    // -----------------------------------------------------------------------

    /// 打开选中任务的 pieces 分块弹窗（P）。
    ///
    /// 功能门槛（仅支持分块下载的任务才有）：BT 恒分块；HTTP(S) 需服务器
    /// 支持断点续传（Range）——不支持续传时只能单线程整段下载、无分块概念，
    /// toast 拒绝（demo 中 media-assets 即此类）。空内容（total=0）无分块，
    /// 同样不打开。
    pub fn open_pieces(&mut self) {
        let Some(t) = self.sel_task() else {
            self.set_toast("未选中任务，无法查看分块");
            return;
        };
        if !t.supports_pieces() {
            self.set_toast("该任务不支持分块下载（服务器不支持断点续传），无 pieces 信息");
            return;
        }
        if t.total == 0 {
            self.set_toast("任务内容为空，无分块信息");
            return;
        }
        self.pieces_open = true;
        self.pieces_follow = true; // 首帧由渲染层按实际列数定位到下载前沿
        self.pieces_scroll = 0;
        self.pieces_pos = None; // 打开时居中（渲染层首帧回填）
        self.pieces_drag = None;
    }

    /// 关闭 pieces 弹窗
    pub fn close_pieces(&mut self) {
        self.pieces_open = false;
        self.pieces_area = None;
        self.pieces_pos = None;
        self.pieces_drag = None;
    }

    /// pieces 网格滚动（行单位；上限由渲染层按实际总行数钳制）
    fn pieces_scroll_by(&mut self, delta: i32) {
        if delta < 0 {
            self.pieces_scroll = self
                .pieces_scroll
                .saturating_sub(delta.unsigned_abs() as usize);
        } else {
            self.pieces_scroll = self.pieces_scroll.saturating_add(delta as usize);
        }
    }

    // -----------------------------------------------------------------------
    // 任务详情字段值弹窗（点击详情字段值打开；本轮需求修订新增）
    // -----------------------------------------------------------------------

    /// 打开任务详情字段值弹窗（模态）：快照对应字段的**完整值**
    /// （校验码 / URL / 失败原因 / 保存路径），标题 = 字段名。
    /// 字段不可用时（如未提供校验码）不打开；窗口操作说明收敛于
    /// H 帮助窗口【字段值弹窗】组，窗口内无提示（P3 原则）。
    pub fn open_val_popup(&mut self, field: DetailField) {
        let Some(t) = self.sel_task() else {
            return;
        };
        let text = match field {
            DetailField::Checksum => {
                let Some(ck) = &t.checksum else { return };
                ck.value.clone()
            }
            DetailField::Url => t.url.clone(),
            DetailField::Failed => t.error.clone().unwrap_or_else(|| "未知错误".to_string()),
            DetailField::SavePath => t.save_path.clone(),
        };
        self.val_open = true;
        self.val_field = field;
        self.val_title = field.title().to_string();
        self.val_text = text;
        self.val_scroll = 0;
        self.val_pos = None; // 打开时居中（渲染层首帧回填）
        self.val_drag = None;
    }

    /// 关闭字段值弹窗
    pub fn close_val_popup(&mut self) {
        self.val_open = false;
        self.val_area = None;
        self.val_pos = None;
        self.val_drag = None;
        self.val_sb = None;
    }

    /// 弹窗内容滚动（行单位；上限由渲染层按实际内容行数钳制）
    fn val_scroll_by(&mut self, delta: i32) {
        if delta < 0 {
            self.val_scroll = self
                .val_scroll
                .saturating_sub(delta.unsigned_abs() as usize);
        } else {
            self.val_scroll = self.val_scroll.saturating_add(delta as usize);
        }
    }
}

/// 通用滚动条滚动偏移钳制：i64 中间量 + [0, max] 夹取（负步进到顶即停、
/// 正步进到底即停；max=0 时恒为 0）
fn clamp_offset(v: i64, max: usize) -> usize {
    v.clamp(0, max as i64) as usize
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
/// 选中 NONE（不校验）时实时清空校验码（选中即改值口径；NONE 下不可输入）。
fn ck_dropdown_open_key(d: &mut Dialog, code: KeyCode) {
    match code {
        KeyCode::Up => {
            d.ck_sel = (d.ck_sel + CHECKSUM_ALGOS.len() - 1) % CHECKSUM_ALGOS.len();
            d.ck_type = d.ck_sel;
            if d.ck_type == CK_NONE {
                d.ck_value.clear();
            }
        }
        KeyCode::Down => {
            d.ck_sel = (d.ck_sel + 1) % CHECKSUM_ALGOS.len();
            d.ck_type = d.ck_sel;
            if d.ck_type == CK_NONE {
                d.ck_value.clear();
            }
        }
        KeyCode::Home => {
            d.ck_sel = 0;
            d.ck_type = 0;
            d.ck_value.clear();
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

/// 将粘贴文本按 (kind, focus) 矩阵路由到对话框字段（FR-01-06 v1.17：
/// Add 与 Modify 同权）。过滤规则与逐键输入一致：URL/目录 ≤300 字符；
/// 并发仅数字 ≤2 位；校验码仅十六进制 ≤128 位；下拉行/按钮焦点不接收。
fn dlg_apply_paste(kind: DialogKind, d: &mut Dialog, focus: usize, text: &str) -> bool {
    let clean = paste_sanitize(text);
    match (kind, focus) {
        (DialogKind::Add, 0) => push_capped(&mut d.url, &clean, 300),
        (DialogKind::Add, 1) => push_capped(&mut d.dir, &clean, 300),
        (DialogKind::Add, 2) | (DialogKind::Modify, 0) => {
            let digits: String = clean.chars().filter(|c| c.is_ascii_digit()).collect();
            let inserted = push_capped(&mut d.conns, &digits, 2);
            d.conns_edited |= inserted;
            inserted
        }
        (DialogKind::Add, 4) | (DialogKind::Modify, 2) => {
            // NONE（不校验）下校验码不可输入（与逐键输入同口径）
            if d.ck_type == CK_NONE {
                return false;
            }
            let hex: String = clean.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            push_capped(&mut d.ck_value, &hex, 128)
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// 工具函数
// ---------------------------------------------------------------------------

/// 由 save_path 取保存目录（demo save_path 恒为 `<目录>/<名>`，文件名不含 '/'；
/// 生产侧为独立字段 Task.save_dir，demo 以路径反提保持同口径比较）
fn save_dir_of(save_path: &str) -> String {
    match save_path.rsplit_once('/') {
        Some((d, _)) => d.to_string(),
        None => String::new(),
    }
}

/// 断点接续的模拟已下载字节（v1.28 生产同步轮八）：按任务 ID 取 25%–59%
/// 确定性档位，模拟 sidecar 中记录的断点进度（生产按 sidecar 实际字节续传）
fn resumed_done(id: u32, total: u64) -> u64 {
    let pct = 25 + (id as u64) % 35;
    total * pct / 100
}

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
/// v1.11/REQ-15.1（生产 coder v139 同步）：拆成双层——本函数是「字节→前缀
/// 投影」薄壳，仅限**无逐块记账来源**的初始造数路径（`mk` 种子任务、断点
/// 清零重建）兜底；有权威块记账的路径（修改任务并发热重建）一律走
/// [`make_chunk_conns_lease`]，以 tick 逐块记账为口径，避免启发式投影把
/// 在传零头折成幻影已完成块写入计数（生产 v139 修复的缺陷模式）。
///
/// 策略：
/// - 块大小按协议写死（HTTP 1 MB / BT 256 KB），块数 y = ceil(total / 块大小)，
///   与并发数解耦；
/// - 连接从块队列 [x, x+active) 连续领块，完成一块立即领取下一块；
/// - 实际并发 = min(最大并发, 未完成块数)，队列临近结束时连接自然转入待命；
/// - done_total ≥ total（校验/完成/做种）时展示最后 min(n, y) 个满块。
///
/// 返回 (连接列表, 已完成块数 x)
fn make_chunk_conns(
    total: u64,
    done_total: u64,
    n: usize,
    piece: u64,
    seed: u64,
) -> (Vec<Connection>, u32) {
    let done = done_total.min(total);
    make_chunk_conns_lease(total, (done / piece) as u32, done % piece, n, piece, seed)
}

/// 分块队列构建（权威口径内核）：已完成块数 `completed` 与在传零头 `rem`
/// 由调用方按记账来源给出（见 [`make_chunk_conns`] 分层说明）。
///
/// 策略与投影薄壳一致：连接从块队列 [completed, completed+active) 连续领块，
/// 实际并发 = min(最大并发, 未完成块数)；已完成字节达 total（校验/完成/做种）
/// 时展示最后 min(n, y) 个满块。`rem` 超过新窗口容量（active×块大小）时
/// [`spread_bytes`] 按块容量钳制，超出零头视为随队尾块回队重下（演示口径）。
///
/// 返回 (连接列表, 已完成块数 x)
fn make_chunk_conns_lease(
    total: u64,
    completed: u32,
    rem: u64,
    n: usize,
    piece: u64,
    seed: u64,
) -> (Vec<Connection>, u32) {
    let y = chunk_total(total, piece);
    if y == 0 {
        return (Vec::new(), 0);
    }
    let y64 = y as u64;
    let piece_end = |i: u64| if i + 1 == y64 { total } else { (i + 1) * piece };
    let x0 = (completed as u64).min(y64);
    let done = (x0 * piece + rem).min(total);
    let (x, lease_from, active, rem) = if done >= total {
        let a = (n.max(1) as u64).min(y64);
        (y64, y64 - a, a, 0u64)
    } else {
        let a = (n.max(1) as u64).min(y64 - x0);
        (x0, x0, a, rem)
    };
    let partials = spread_bytes(rem, active as usize, piece);
    let conns = (0..active as usize)
        .map(|i| {
            let idx = lease_from + i as u64;
            let start = idx * piece;
            let end = piece_end(idx);
            let partial = if done >= total {
                end - start
            } else {
                partials[i].min(end - start)
            };
            Connection {
                ip: pseudo_ip(seed, i),
                // 客户端缩写 / 传输协议（确定性演示数据；仅 BT 任务渲染这两列，
                // HTTP 行不显示——见 ui::draw_conns 列宽定义）
                client: pseudo_client(seed, i),
                transport: pseudo_transport(seed, i),
                win: 0,
                win_up: 0,
                start,
                end,
                done: partial,
                speed: 0.0,
                up_speed: 0.0,
                // 累计下载量以当前持有块内的进度为初值（演示任务从中途开始）
                cum_down: partial,
                cum_up: 0,
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
    let (mut connections, chunk_done) = make_chunk_conns(total, done, conns, piece, id as u64);
    if state == TaskState::Completed {
        // 已完成任务不保留并发连接明细（面板为空、活跃 0/0）
        connections.clear();
    }
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
    // 演示：首次校验失败（生产 v1.17/FR-01-51 同步）——不自动重试，R 重新
    // 校验（无块重传）→ 校验通过完成；校验码清空后 R 则不校验直接完成
    // 「无校验」（D34）。
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

// ---------------------------------------------------------------------------
// 修订-5/7：剪贴板复制（点击任务详情面板字段名触发）
// ---------------------------------------------------------------------------

impl App {
    /// 将文本复制到剪贴板（修订-7）：
    /// 1) 优先 arboard 写系统剪贴板（Windows / macOS / Linux X11 & Wayland，
    ///    不依赖终端支持，iTerm2/gnome-terminal 等不开 OSC 52 也能复制）；
    ///    实例懒初始化并常驻复用（首次建连后，后续点击零建连开销）；
    /// 2) 无图形会话（SSH/纯控制台）或写入失败时，回退 OSC 52 终端转义序列
    ///    （ESC ]52;c;<base64> BEL，由终端代写剪贴板，主流终端多数支持）。
    fn copy_to_clipboard(&mut self, text: &str) {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        if let Some(cb) = self.clipboard.as_mut() {
            if cb.set_text(text.to_string()).is_ok() {
                return; // 系统剪贴板写入成功
            }
            self.clipboard = None; // 实例失效：下次点击重建，本次回退
        }
        copy_osc52(text);
    }
}

/// OSC 52 终端转义序列回退：终端原生方案，无需 X11/Wayland 会话与外部依赖；
/// 写入失败时静默忽略。
fn copy_osc52(text: &str) {
    use std::io::Write;
    let seq = format!("\x1b]52;c;{}\x07", b64_encode(text.as_bytes()));
    let mut out = std::io::stdout();
    let _ = out.write_all(seq.as_bytes());
    let _ = out.flush();
}

/// 极简 base64 编码（标准字母表 + '=' 填充），仅供 OSC 52 使用，避免引入外部 crate
fn b64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = ((*chunk.first().unwrap_or(&0) as u32) << 16)
            | ((*chunk.get(1).unwrap_or(&0) as u32) << 8)
            | (*chunk.get(2).unwrap_or(&0) as u32);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}
