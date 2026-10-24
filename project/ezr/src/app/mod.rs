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
use std::time::{Duration, Instant};

use ratatui::layout::Rect;

use crate::engine::{EngineHandle, Evt};
use crate::model::config::Config;
use crate::model::registry::Registry;
use crate::model::sidecar::Sidecar;
use crate::model::speed::{SmoothedSpeed, SpeedWindow};
use crate::model::{slots, Connection, Task};
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
mod pieces;
mod tasks;

// v1.32/FR-01-109：通用滚动条引擎几何类型（Bar/Hit/Arrow）经根叶子模块引入
// （渲染与命中几何单一来源 crate::scrollbar；app 侧交互状态机在本文件；
// 引擎为根叶子层，app → scrollbar 不违反组装根分层——arch 规则 13 只约束 ui）
use crate::scrollbar::{Arrow as SbArrow, Bar as SbBar, Hit as SbHit};
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
    text_field_at, TextField,
};

/// 每个任务在列表中占用的行高（3 行内容 + 1 行空行分隔）
pub const ITEM_HEIGHT: u16 = 4;

// ---------------------------------------------------------------------------
// 通用滚动条交互状态机（v1.32/FR-01-109 全面重写，对齐 ezr-demo m3 app.rs）
// ---------------------------------------------------------------------------

/// 滚动条实例标识（每实例一个；[`SbKey::bar`] 从 App 回填几何取本帧 [`SbBar`]）
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
    /// 字段值弹窗（垂直，v1.34/FR-01-113）
    Val,
}

impl SbKey {
    /// 箭头单击/按住连发的步长（内容单位：任务条目/连接行/内容列/网格行/文本行；
    /// 水平以 2 列为单位——单列步进肉眼难辨，FR-01-109 ⑥）
    #[must_use]
    pub fn step(self) -> i32 {
        match self {
            SbKey::ConnsH => 2,
            _ => 1,
        }
    }

    /// 本实例当前帧的滚动条几何（ui 层每帧回填；None = 本帧未绘制）
    pub(crate) fn bar(self, app: &App) -> Option<SbBar> {
        match self {
            SbKey::List => app.list_scrollbar,
            SbKey::ConnsV => app.conns_v_scrollbar,
            SbKey::ConnsH => app.conns_h_scrollbar,
            SbKey::Pieces => app.pieces_scrollbar,
            SbKey::Help => app.help_scrollbar,
            SbKey::Val => app.val_scrollbar,
        }
    }
}

/// 滚动条箭头/轨道按住连发状态（Down 记录、Up 取消、tick 驱动重复）
#[derive(Clone, Copy, Debug)]
pub struct SbHold {
    /// 哪个滚动条实例
    pub key: SbKey,
    /// 方向：-1 = 朝头箭头（↑/←），+1 = 朝尾箭头（↓/→）
    pub dir: i32,
    /// true = 轨道翻页（按 page 步长），false = 箭头步进（按 step 步长）
    pub page: bool,
    /// 按下时刻（起振延迟基准）
    pub since: Instant,
    /// 上次重复时刻
    pub last: Instant,
}

impl SbHold {
    /// 首次重复的延迟（按住起振时间，FR-01-109 ⑥：0.4s）
    const FIRST_DELAY: Duration = Duration::from_millis(400);
    /// 后续重复周期（0.12s）
    const REPEAT: Duration = Duration::from_millis(120);

    /// 下次重复到期时刻（首次按起振延迟，之后按周期）
    fn due(&self) -> Instant {
        if self.since == self.last {
            self.since + Self::FIRST_DELAY
        } else {
            self.last + Self::REPEAT
        }
    }
}

// ---------------------------------------------------------------------------
// 面板焦点（lazygit 式聚焦显示，v1.30/FR-01-108，对齐 ezr-demo 重构版）
// ---------------------------------------------------------------------------

/// 可聚焦面板：Tab/BackTab 循环切换；当前面板边框绿色高亮、
/// 其余面板暗边框（lazygit activeBorderColor/inactiveBorderColor 口径）。
/// 默认 = 任务列表（应用启动与焦点钳制基线）。
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Panel {
    /// 任务列表（头部页签随本面板聚焦而高亮；默认聚焦）
    #[default]
    List,
    /// 任务详情
    Detail,
    /// 并发连接
    Conns,
}

impl Panel {
    /// Tab：下一个面板（列表 → 详情 → 并发 → 列表）
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Panel::List => Panel::Detail,
            Panel::Detail => Panel::Conns,
            Panel::Conns => Panel::List,
        }
    }

    /// BackTab：上一个面板
    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Panel::List => Panel::Conns,
            Panel::Detail => Panel::List,
            Panel::Conns => Panel::Detail,
        }
    }
}

/// 列表页签（仅两个）
pub const FILTERS: [&str; 2] = ["正在下载", "已完成"];

/// 任务详情可弹窗查看完整值的字段（v1.34/FR-01-113，对齐 ezr-demo 重构版
/// DetailField）：字段名点击仍是复制（FR-01-101 v1.34 四项），字段值点击
/// 打开完整值弹窗。字段与详情面板蓝色下划线链接字段一一对应；
/// `title`/`copy_label` 为弹窗标题与复制 toast 文案的单源（D42）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DetailField {
    /// 校验（完整校验码）
    Checksum,
    /// URL（完整下载地址）
    Url,
    /// 失败（完整错误文案；字段名原「失败原因」）
    Failed,
    /// 路径（完整保存路径；字段名原「保存」）
    SavePath,
}

impl DetailField {
    /// 弹窗标题（= 详情面板字段名，FR-01-80 v1.34 精简口径单源）
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            DetailField::Checksum => "校验",
            DetailField::Url => "URL",
            DetailField::Failed => "失败",
            DetailField::SavePath => "路径",
        }
    }

    /// 复制 toast 文案（与字段名点击复制同 label 单源——描述被复制的值
    /// 而非字段名显示文本，D42 钦定文案）
    #[must_use]
    pub fn copy_label(self) -> &'static str {
        match self {
            DetailField::Checksum => "校验码",
            DetailField::Url => "url",
            DetailField::Failed => "失败原因",
            DetailField::SavePath => "保存路径",
        }
    }
}

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
    /// Add: 0=URL 1=目录 2=并发 3=算法 4=校验码 5=代理 6=立即下载 7=仅添加 8=取消
    /// （v1.15/FR-01-103 三钮；原 6=确认 7=取消 两钮序废止）
    /// Modify: 0=并发 1=算法 2=校验码 3=代理 4=确定 5=取消
    /// Delete: 0=仅删除任务 1=删除任务和文件 2=取消
    pub focus: usize,
    /// 文本字段光标（v1.34 二批 bug 修订：当前焦点文本字段内的**字符偏移**
    /// 0..=字段长——方向键在输入框内移动光标不再跳焦点；输入/退格/粘贴均
    /// 在光标处生效。约定：焦点切换随 [`Dialog::focus_field`] 把光标迁到新
    /// 字段文本尾；非文本焦点（下拉行/按钮）不消费该值）
    pub cursor: usize,
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
    /// 当前聚焦面板（lazygit 式，v1.30/FR-01-108；窄终端/紧凑模式钳制 List）
    pub focus: Panel,
    /// 全局下载速度历史（KB/s 采样）
    pub speed_hist: Vec<u64>,
    /// 全局上传速度历史（KB/s 采样；01 恒 0，BT 二期启用）
    pub up_hist: Vec<u64>,
    /// 本次会话累计下载字节（FR-01-81 头部统计）
    pub session_bytes: u64,
    /// 各任务上次观察到的已下载字节（会话累计账本，增量式计数用）
    session_seen: HashMap<u32, u64>,
    /// 连接级账本（FR-01-99 ②累计 + FR-01-102 数据面，App 消费侧计量不回写引擎）：
    /// (任务, 连接) → 上一观测 (块号, 块内已写字节)——增量与换块判定依据
    conn_prev: HashMap<(u32, usize), (u32, u64)>,
    /// (任务, 连接) → 本次开始下载起累计落盘字节（新一次下载清零、续传保留）
    conn_cum: HashMap<(u32, usize), u64>,
    /// (任务, 连接) → 连接速度展示面平滑器（EMA α=1/5，FR-01-102 展示面；
    /// 采样每秒一次在 tick ⑤，待命/非下载态零值速断）
    conn_speed: HashMap<(u32, usize), SmoothedSpeed>,
    /// (任务, 连接) → 连接速度滑窗（1s，FR-01-102 数据面：与任务级 windows 同构，
    /// 以连接级累计读数推进；生命周期随连接账本，clear_conn_stats 一并清理）
    conn_windows: HashMap<(u32, usize), SpeedWindow>,
    /// 连接级账本待重锚任务集（04-ui-conns-09 R 臂，QA v138 缺陷修复）：
    /// 手动重试 = 新的一次下载、累计从零重计——账本清零后首个 Progress 观测
    /// 的块内 done 含续传检查点偏移，直接入账会把偏移当本轮落盘量；置重锚
    /// 标记后由 Progress 消费侧以当前观测重锚 conn_prev（该次增量 = 0）。
    conn_rebase: std::collections::HashSet<u32>,
    /// 帧计数（转轮动画）
    pub frame: u64,
    /// 退出标志
    pub quit: bool,
    /// toast 文案
    pub toast: Option<String>,
    toast_until: Option<Instant>,
    /// 上一帧选中下标（tick 兜底跟随的变化检测：仅选中项真的变化才拉回
    /// 视口——滚轮自由滚动不被逐帧归位，滚动条不锁死，v1.30/FR-01-109 ⑤）
    last_selected: usize,
    /// 帮助窗口（H 快捷键，v1.30/FR-01-107）是否打开
    pub help_open: bool,
    /// 帮助窗口内容滚动偏移（以「行」为单位；渲染层按内容高度钳制）
    pub help_scroll: usize,
    /// 帮助窗口左上角位置（鼠标拖拽移动记忆；None = 打开时居中。
    /// 帮助窗口位置跨次打开记忆——关闭不清除，与 pieces 关闭即清不同）
    pub help_pos: Option<(u16, u16)>,
    /// 帮助窗口拖拽中的抓取偏移（指针相对窗口左上角）
    pub help_drag: Option<(u16, u16)>,
    /// 帮助窗口区域（由 ui 层每帧回填；拖拽/滚轮/点击外部关闭的命中测试）
    pub help_area: Option<Rect>,
    /// 任务列表滚动条命中几何（v1.32/FR-01-109：ui 层每帧回填；None = 本帧未绘制）
    pub list_scrollbar: Option<SbBar>,
    /// 并发连接面板垂直滚动条命中几何（v1.32/FR-01-109）
    pub conns_v_scrollbar: Option<SbBar>,
    /// 并发连接面板水平滚动条命中几何（v1.32/FR-01-109：唯一 Both 实例）
    pub conns_h_scrollbar: Option<SbBar>,
    /// pieces 弹窗滚动条命中几何（v1.32/FR-01-109）
    pub pieces_scrollbar: Option<SbBar>,
    /// 帮助窗口滚动条命中几何（v1.32/FR-01-109）
    pub help_scrollbar: Option<SbBar>,
    /// 字段值弹窗滚动条命中几何（v1.34/FR-01-113：仅垂直，内容超窗才显示）
    pub val_scrollbar: Option<SbBar>,
    /// pieces 网格滚动上限（总行数 − 网格行数；ui 层每帧回填，与渲染层钳制同源）
    pub pieces_max_scroll: usize,
    /// 帮助窗口滚动上限（内容总行数 − 内框行数；ui 层每帧回填）
    pub help_max_scroll: usize,
    /// 通用滚动条滑块拖拽中：(哪个滚动条, 抓取点在滑块内的格偏移)
    /// ——Down 命中 Thumb 记录、Drag 随动反解、任意 Up 结束（FR-01-109 ⑥）
    pub sb_drag: Option<(SbKey, usize)>,
    /// 通用滚动条箭头/轨道按压连发状态（Down 记录、Up 取消、tick 驱动重复）
    pub sb_hold: Option<SbHold>,
    /// 通用滚动条箭头按压高亮态（Some = (哪个滚动条, 哪个箭头)；按住期间拖出
    /// 箭头 = 取消高亮连发不中断、拖回恢复，FR-01-109 ⑥）
    pub sb_pressed: Option<(SbKey, SbArrow)>,
    /// 箭头点击高亮余辉（Down 即置 400ms；与页脚 chips 的 footer_flash 同机制）
    pub sb_flash: Option<(SbKey, SbArrow, Instant)>,
    /// 关于窗口（O 快捷键，v1.32/FR-01-111）是否打开
    pub about_open: bool,
    /// 关于窗口左上角位置（鼠标拖拽移动记忆；None = 打开时居中。
    /// 位置跨次打开记忆——关闭不清除，与帮助窗口同口径）
    pub about_pos: Option<(u16, u16)>,
    /// 关于窗口拖拽中的抓取偏移（指针相对窗口左上角）
    pub about_drag: Option<(u16, u16)>,
    /// 关于窗口区域（由 ui 层每帧回填；拖拽的命中测试）
    pub about_area: Option<Rect>,
    /// 头部页签命中矩形（ui 层每帧回填；鼠标点击页签切换，v1.30/FR-01-80）
    pub tab_rects: Vec<(Rect, usize)>,
    /// 页脚命令 chips 命中矩形（v1.31/FR-01-107①：ui 层每帧重建，不含分隔符；
    /// 鼠标点击执行与键盘同口径）
    pub footer_cmd_rects: Vec<(Rect, usize)>,
    /// 页脚 chip 按压中下标（v1.31/FR-01-107①：Down 记录、原地松开执行、
    /// 拖出取消、浮层接管清除）
    pub footer_press: Option<usize>,
    /// 页脚 chip 点击余辉（v1.31/FR-01-107①：(下标, 余辉截止时刻)，
    /// 400ms——快速点按在两帧之间也可见）
    pub footer_flash: Option<(usize, Instant)>,
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
    /// 并发连接面板滚动偏移（连接行单位；滚轮 / ↑↓（面板聚焦时）滚动，FR-01-96/97）
    pub conns_scroll: usize,
    /// 并发连接面板当前选中行（连接下标 0 基；↑↓ 移动（面板聚焦时），B 断开目标，
    /// FR-01-97 v1.30）
    pub conns_sel: usize,
    /// 并发连接面板内框区域（ui 层回填；滚轮命中；空明细时 None = 穿透）
    pub conns_area: Option<Rect>,
    /// 任务详情面板「URL」字段名热区（ui 层每帧回填；点击复制 url 展示值，FR-01-101）
    pub detail_url_rect: Option<Rect>,
    /// 任务详情面板「校验」字段名热区（ui 层每帧回填；点击复制校验码值，FR-01-101）
    pub detail_ck_rect: Option<Rect>,
    /// 任务详情面板「失败」字段名热区（ui 层每帧回填；点击复制失败原因，
    /// v1.34/FR-01-101 四项扩展）
    pub detail_fail_rect: Option<Rect>,
    /// 任务详情面板「路径」字段名热区（ui 层每帧回填；点击复制保存路径，
    /// v1.34/FR-01-101 四项扩展）
    pub detail_save_rect: Option<Rect>,
    /// 任务详情面板字段值热区（ui 层每帧回填：悬停下划线 + 点击打开字段值
    /// 弹窗，v1.34/FR-01-113；浮层打开时不响应）
    pub detail_val_rects: Vec<(Rect, DetailField)>,
    /// 任务详情面板完整区域（ui 层每帧回填；点击面板任意处聚焦，v1.30/FR-01-108）
    pub detail_area: Option<Rect>,
    /// 指针位置（v1.34/FR-01-113 悬停下划线判定：任意鼠标事件均携带坐标，
    /// 供详情字段值悬停下划线与 [✗] 悬停高亮判定；TUI 无法感知指针离开
    /// 终端，悬停态以最后一次上报位置为准）
    pub pointer: Option<(u16, u16)>,
    /// 弹窗右上角 [✗] 热区（v1.34/FR-01-114：ui 层每帧由顶层弹窗绘制回填，
    /// 无弹窗打开 = None；热区 = [✗] 自身 3 列）
    pub popup_close_rect: Option<Rect>,
    /// [✗] 按压中（Down 记录、Drag 随动拖出取消、原地松开关闭弹窗，FR-01-114）
    pub x_press: bool,
    /// 字段值弹窗（v1.34/FR-01-113）是否打开
    pub val_open: bool,
    /// 字段值弹窗字段（弹窗标题与 Y 复制 toast 文案依据）
    pub val_field: DetailField,
    /// 字段值弹窗内容快照（打开时的完整字段值）
    pub val_text: String,
    /// 字段值弹窗内容滚动偏移（渲染层按内容行数钠制）
    pub val_scroll: usize,
    /// 字段值弹窗滚动上限（内容总行数 − 可视行数；ui 层每帧回填）
    pub val_max_scroll: usize,
    /// 字段值弹窗左上角位置（拖拽记忆；None = 打开时居中；关闭即清，
    /// 同 pieces 口径）
    pub val_pos: Option<(u16, u16)>,
    /// 字段值弹窗拖拽中的抓取偏移（指针相对窗口左上角）
    pub val_drag: Option<(u16, u16)>,
    /// 字段值弹窗区域（ui 层每帧回填；拖拽/滚轮/点击外部命中测试）
    pub val_area: Option<Rect>,
    /// 系统剪贴板实例（FR-01-101：懒初始化并复用，避免每次点击重建桌面连接）
    clipboard: Option<arboard::Clipboard>,
    /// 最近一次复制的内容（FR-01-101 诊断与测试断言复制内容用）
    pub(crate) last_copied: Option<String>,
    /// 并发连接面板可视数据行数（ui 层回填，滚动钳制）
    pub visible_conns_rows: usize,
    /// 明细滚动所属的列表选中序号（切换任务时重置滚动与选择，FR-01-97）
    conns_scroll_for: usize,
    /// 并发连接面板横向滚动偏移（v1.32/FR-01-96/109：以「内容列」为单位，
    /// ←/→ 键步进 ±2；上限 = 内容宽 − 内框宽，ui 层每帧回填钳制）
    pub conns_hscroll: usize,
    /// 并发连接面板横向最大滚动偏移（内容宽 − 内框宽，ui 层每帧回填）
    pub conns_max_hscroll: usize,
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
    /// 对话框完整区域（ui 层每帧回填；拖拽起点命中测试，v133/D38）
    pub dlg_area: Option<Rect>,
    /// 对话框左上角位置（鼠标拖拽移动记忆；None = 打开时居中，渲染层
    /// 首帧回填；关闭不保留，再次打开重新居中，v133/D38 与 pieces 同口径）
    pub dlg_pos: Option<(u16, u16)>,
    /// 对话框拖拽中的抓取偏移（指针相对窗口左上角；Some = 左键拖拽进行中）
    pub dlg_drag: Option<(u16, u16)>,
    /// pieces 分块弹窗（`P` 快捷键，FR-01-106）是否打开
    pub pieces_open: bool,
    /// pieces 弹窗网格滚动偏移（网格行单位；上限由渲染层按实际总行数钳制）
    pub pieces_scroll: usize,
    /// 打开 pieces 弹窗时定位到下载前沿（由 ui 层按实际列数消费一次，
    /// 打开后滚动完全由用户控制、不自动跟随下载推进）
    pub pieces_follow: bool,
    /// pieces 弹窗区域（ui 层每帧回填；滚轮滚动/点击关闭/拖拽的命中测试）
    pub pieces_area: Option<Rect>,
    /// pieces 弹窗左上角位置（鼠标拖拽移动记忆；None = 打开时居中，渲染层
    /// 首帧回填；关闭不保留，再次打开重新居中）
    pub pieces_pos: Option<(u16, u16)>,
    /// pieces 弹窗拖拽中的抓取偏移（指针相对弹窗左上角；Some = 左键拖拽进行中）
    pub pieces_drag: Option<(u16, u16)>,
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
        // （v1.20：任务 ID 随快照保留，无 next_id 派生）
        let tasks = Self::restore(&registry_path);
        let mut app = App {
            tasks,
            selected: 0,
            scroll: 0,
            filter: 0,
            focus: Panel::List,
            last_selected: 0,
            speed_hist: vec![0; 90],
            up_hist: vec![0; 90],
            session_bytes: 0,
            session_seen: HashMap::new(),
            conn_prev: HashMap::new(),
            conn_cum: HashMap::new(),
            conn_speed: HashMap::new(),
            conn_windows: HashMap::new(),
            conn_rebase: std::collections::HashSet::new(),
            frame: 0,
            quit: false,
            // 启动欢迎语 4 秒（v1.30/FR-01-107：toast 一般 3 秒、欢迎语 4 秒）
            toast: Some("EZR Downloader 就绪".to_string()),
            toast_until: Some(Instant::now() + std::time::Duration::from_secs(4)),
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
            detail_fail_rect: None,
            detail_save_rect: None,
            detail_val_rects: Vec::new(),
            detail_area: None,
            pointer: None,
            popup_close_rect: None,
            x_press: false,
            val_open: false,
            val_field: DetailField::Url,
            val_text: String::new(),
            val_scroll: 0,
            val_max_scroll: 0,
            val_pos: None,
            val_drag: None,
            val_area: None,
            clipboard: None,
            last_copied: None,
            visible_conns_rows: 8,
            conns_scroll_for: 0,
            conns_hscroll: 0,
            conns_max_hscroll: 0,
            list_scrollbar: None,
            conns_v_scrollbar: None,
            conns_h_scrollbar: None,
            pieces_scrollbar: None,
            help_scrollbar: None,
            val_scrollbar: None,
            pieces_max_scroll: 0,
            help_max_scroll: 0,
            sb_drag: None,
            sb_hold: None,
            sb_pressed: None,
            sb_flash: None,
            about_open: false,
            about_pos: None,
            about_drag: None,
            about_area: None,
            dialog: None,
            dlg_btn_rects: Vec::new(),
            dlg_field_rects: Vec::new(),
            dlg_ck_rects: Vec::new(),
            dlg_proxy_rects: Vec::new(),
            dlg_area: None,
            dlg_pos: None,
            dlg_drag: None,
            pieces_open: false,
            pieces_scroll: 0,
            pieces_follow: false,
            pieces_area: None,
            pieces_pos: None,
            pieces_drag: None,
            help_open: false,
            help_scroll: 0,
            help_pos: None,
            help_drag: None,
            help_area: None,
            tab_rects: Vec::new(),
            footer_cmd_rects: Vec::new(),
            footer_press: None,
            footer_flash: None,
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
    fn restore(path: &str) -> Vec<Task> {
        let Some(reg) = Registry::load(path) else {
            return Vec::new();
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
        tasks
    }

    /// 分配新任务 ID（v1.20，操作者 20261011 指令）：u32 随机数 + 任务全集
    /// 查重（含已完成/失败历史），0 保留——取代顺序自增 `next_id`，注册表
    /// 簿记、空列表不回退、`u32::MAX` 溢出回绕三个问题一并消除（口径与
    /// 唯一性论证见 `model::idgen`）
    #[must_use]
    pub fn alloc_task_id(&self) -> u32 {
        crate::model::idgen::alloc_task_id(|cand| self.tasks.iter().any(|t| t.id == cand))
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

    /// 是否存在浮层（对话框/pieces 弹窗/帮助窗口/关于窗口/字段值弹窗）。
    /// 浮层打开时底层面板全部降级暗边框、页签高亮不再使用反色底。
    /// v1.32/FR-01-108 注记：浮层集合含关于窗口（FR-01-111）；
    /// v1.34/FR-01-113：浮层集合含字段值弹窗。
    #[must_use]
    pub fn overlay_open(&self) -> bool {
        self.dialog.is_some()
            || self.pieces_open
            || self.help_open
            || self.about_open
            || self.val_open
    }

    /// 打开关于窗口（O 快捷键，v1.32/FR-01-111）：居中显示 EZR Download v1.0.0
    pub fn open_about(&mut self) {
        self.about_open = true;
        // about_pos 保留上次拖拽位置（跨次记忆）；None = 渲染层首帧居中并回填
    }

    /// 关闭关于窗口（位置记忆保留，与帮助窗口同口径）
    pub fn close_about(&mut self) {
        self.about_open = false;
        self.about_area = None;
        self.about_drag = None;
    }

    // -----------------------------------------------------------------------
    // 字段值弹窗（v1.34/FR-01-113，对齐 ezr-demo 重构版 open/close_val_popup）
    // -----------------------------------------------------------------------

    /// 打开任务详情字段值弹窗（模态，L6 最上层）：快照对应字段的**完整值**
    /// （完整校验码 / 完整 URL / 完整错误文案 / 完整保存路径），标题 = 字段名
    ///（[`DetailField::title`] 单源）。字段不可用时（如未提供校验码）不打开；
    /// 窗口内无提示（P10），操作说明收敛于 H 帮助窗口【字段值弹窗】组；
    /// 位置关闭即清（同 pieces 口径，与帮助/关于跨次记忆不同）。
    pub fn open_val_popup(&mut self, field: DetailField) {
        let Some(t) = self.sel_task() else {
            return;
        };
        let text = match field {
            DetailField::Checksum => {
                let Some(ck) = t.checksum.as_ref() else {
                    return;
                };
                ck.value.clone()
            }
            DetailField::Url => t.url.clone(),
            DetailField::Failed => t.error.clone().unwrap_or_else(|| "未知错误".to_string()),
            DetailField::SavePath => t.target_path(),
        };
        self.val_open = true;
        self.val_field = field;
        self.val_text = text;
        self.val_scroll = 0;
        self.val_pos = None; // 打开时居中（渲染层首帧回填）
        self.val_drag = None;
        self.val_area = None;
        self.val_scrollbar = None;
        self.focus = Panel::Detail; // 同时聚焦详情面板（FR-01-113 ①）
    }

    /// 关闭字段值弹窗（位置即清：val_pos 重置，同 pieces 口径）
    pub fn close_val_popup(&mut self) {
        self.val_open = false;
        self.val_area = None;
        self.val_pos = None;
        self.val_drag = None;
        self.val_scrollbar = None;
    }

    // -----------------------------------------------------------------------
    // 通用滚动条交互（v1.32/FR-01-109 ⑥：滑块拖拽/箭头步进连发/轨道翻页/
    // 按压高亮余辉；对齐 ezr-demo m3 app.rs sb_* 方法族）
    // -----------------------------------------------------------------------

    /// 滚动条滚动上限（按实例取各自的 total − page；ConnsH = 横向内容差）
    fn sb_max(&self, key: SbKey) -> usize {
        match key {
            SbKey::List => self
                .filtered()
                .len()
                .saturating_sub(self.visible_rows.max(1)),
            SbKey::ConnsV => self
                .conn_count()
                .saturating_sub(self.visible_conns_rows.max(1)),
            SbKey::ConnsH => self.conns_max_hscroll,
            SbKey::Pieces => self.pieces_max_scroll,
            SbKey::Help => self.help_max_scroll,
            SbKey::Val => self.val_max_scroll,
        }
    }

    /// 滚动偏移写入（按实例写回各自的滚动字段；事件内立即钳制 0..=max_pos，
    /// 末页即停——FR-01-109 ⑥）
    fn sb_scroll(&mut self, key: SbKey, delta: i32) {
        let max = self.sb_max(key);
        let clamp = |v: i64| v.clamp(0, max as i64) as usize;
        match key {
            SbKey::List => self.scroll = clamp(self.scroll as i64 + delta as i64),
            SbKey::ConnsV => self.conns_scroll = clamp(self.conns_scroll as i64 + delta as i64),
            SbKey::ConnsH => self.conns_hscroll = clamp(self.conns_hscroll as i64 + delta as i64),
            SbKey::Pieces => self.pieces_scroll = clamp(self.pieces_scroll as i64 + delta as i64),
            SbKey::Help => self.help_scroll = clamp(self.help_scroll as i64 + delta as i64),
            SbKey::Val => self.val_scroll = clamp(self.val_scroll as i64 + delta as i64),
        }
    }

    /// 滚动偏移绝对写入（滑块拖拽反解结果；钳制到 0..=max_pos）
    fn sb_set(&mut self, key: SbKey, pos: usize) {
        let max = self.sb_max(key);
        let v = pos.min(max);
        match key {
            SbKey::List => self.scroll = v,
            SbKey::ConnsV => self.conns_scroll = v,
            SbKey::ConnsH => self.conns_hscroll = v,
            SbKey::Pieces => self.pieces_scroll = v,
            SbKey::Help => self.help_scroll = v,
            SbKey::Val => self.val_scroll = v,
        }
    }

    /// 轨道翻页页长（本实例一屏可视单位数；几何缺失时退化为 1）
    fn sb_page(&self, key: SbKey) -> i32 {
        key.bar(self).map_or(1, |b| b.page.max(1) as i32)
    }

    /// 滚动条 Down 事件分派（FR-01-109 ⑥）：滑块 = 开始拖拽；箭头 = 立即
    /// 步进一次 + 按压高亮 + 400ms 余辉 + 记连发；轨道 = 朝点击方向整页滚动
    /// + 记连发。同时聚焦所属面板（主界面实例）。
    fn sb_press(&mut self, key: SbKey, bar: &SbBar, hit: SbHit) {
        match hit {
            SbHit::Thumb(grab) => {
                self.sb_drag = Some((key, grab));
            }
            SbHit::Head => {
                let step = key.step();
                self.sb_scroll(key, -step);
                self.sb_pressed = Some((key, SbArrow::Head));
                self.sb_flash = Some((
                    key,
                    SbArrow::Head,
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
            SbHit::Tail => {
                let step = key.step();
                self.sb_scroll(key, step);
                self.sb_pressed = Some((key, SbArrow::Tail));
                self.sb_flash = Some((
                    key,
                    SbArrow::Tail,
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
            SbHit::Track(t) => {
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

    /// 滑块拖拽随动（Drag 事件）：指针在轨道内的下标 − 抓取偏移 = 滑块新位，
    /// 反解回滚动偏移（与正向显示取整严格互逆，FR-01-109 ④）
    fn sb_drag_move(&mut self, col: u16, row: u16) {
        let Some((key, grab)) = self.sb_drag else {
            return;
        };
        let Some(bar) = key.bar(self) else {
            return;
        };
        let t = bar.track_index_at(col, row);
        let pos = bar.pos_from_thumb(t.saturating_sub(grab));
        self.sb_set(key, pos);
    }

    /// 松开左键：结束拖拽/连发/按压高亮（任何位置松开都调用）
    fn sb_release(&mut self) {
        self.sb_drag = None;
        self.sb_hold = None;
        self.sb_pressed = None;
    }

    /// tick 驱动的按住连发（FR-01-109 ⑥：0.4s 起振、0.12s 周期，松开即停）
    pub(crate) fn sb_hold_tick(&mut self, now: Instant) {
        let Some(h) = self.sb_hold else {
            return;
        };
        if now < h.due() {
            return;
        }
        let delta = if h.page {
            self.sb_page(h.key)
        } else {
            h.key.step()
        };
        self.sb_scroll(h.key, h.dir * delta);
        if let Some(hh) = self.sb_hold.as_mut() {
            hh.last = now;
        }
    }

    /// 渲染查询：箭头按压高亮态（按压中优先；否则 400ms 余辉内可见——
    /// 快速单击也有可见点击效果，FR-01-109 ⑥）
    #[must_use]
    pub fn sb_arrow_pressed(&self, key: SbKey) -> Option<SbArrow> {
        if let Some((k, a)) = self.sb_pressed {
            if k == key {
                return Some(a);
            }
        }
        match self.sb_flash {
            Some((k, a, until)) if k == key && Instant::now() < until => Some(a),
            _ => None,
        }
    }

    /// 按住箭头期间的拖出/拖回高亮跟随：指针在按住的箭头上 = 恢复高亮，
    /// 拖出 = 取消高亮（连发不中断，FR-01-109 ⑥）
    pub(crate) fn sb_arrow_follow(&mut self, col: u16, row: u16) {
        let Some(h) = self.sb_hold else {
            return;
        };
        if h.page {
            return; // 轨道翻页无箭头高亮
        }
        let key = h.key;
        let arrow = if h.dir < 0 {
            SbArrow::Head
        } else {
            SbArrow::Tail
        };
        let on_arrow = key
            .bar(self)
            .and_then(|b| b.hit(col, row))
            .is_some_and(|hit| match hit {
                SbHit::Head => arrow == SbArrow::Head,
                SbHit::Tail => arrow == SbArrow::Tail,
                _ => false,
            });
        if on_arrow {
            self.sb_pressed = Some((key, arrow));
        } else if self.sb_pressed == Some((key, arrow)) {
            self.sb_pressed = None;
        }
    }

    /// 主界面滚动条 Down 命中分派（FR-01-109 ⑥：按下滚动条同时聚焦所属面板）。
    /// 依实例序检查命中（列表 → 并发垂直 → 并发水平）；命中即消费。
    pub(crate) fn sb_press_at(&mut self, col: u16, row: u16) -> bool {
        for (key, focus) in [
            (SbKey::List, Panel::List),
            (SbKey::ConnsV, Panel::Conns),
            (SbKey::ConnsH, Panel::Conns),
        ] {
            if let Some(bar) = key.bar(self) {
                if let Some(hit) = bar.hit(col, row) {
                    self.focus = focus;
                    self.sb_press(key, &bar, hit);
                    return true;
                }
            }
        }
        false
    }

    /// 页脚命令 chip 是否处于点击高亮（v1.31/FR-01-107①：按压中，或
    /// 400ms 余辉内——快速点按在两帧之间也可见）
    #[must_use]
    pub fn footer_chip_hot(&self, i: usize) -> bool {
        if self.footer_press == Some(i) {
            return true;
        }
        matches!(
            self.footer_flash,
            Some((fi, until)) if fi == i && Instant::now() < until
        )
    }

    /// 页脚命令 chip 执行（v1.31/FR-01-107①）：与同名键盘快捷键完全同口径
    /// —— A 添加 / D 删除 / M 修改 / Q 退出 / H 帮助（含「已完成任务不可
    /// 修改」等既有校验与 toast 反馈）。仅主界面态可达（模态浮层打开时
    /// 鼠标事件先交给浮层，P2 模态优先级）。
    pub(crate) fn run_footer_cmd(&mut self, i: usize) {
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

    /// 头部「并发」口径（FR-01-94 ②/FR-01-96 v1.34 活跃口径）：显示并发明细
    /// 的任务的**活跃并发**连接总数（正在连接 + 已连接，[`ConnPhase::is_active`]
    /// 单源——与并发连接面板明细行数、右上角徽标 x 严格同源；FR-01-81 v1.34：
    /// 原「连接总数」口径退役，待命/已断开不再计入）
    #[must_use]
    pub fn conns_display_total(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.state.shows_conns())
            .map(|t| {
                t.connections
                    .iter()
                    .filter(|c| self.conn_is_active(t.id, c))
                    .count()
            })
            .sum()
    }

    /// 并发面板「x/y 项」的 x（FR-01-96 v1.34 活跃口径）：明细状态下活跃
    /// 并发数（正在连接 + 已连接，ConnPhase 单源）；非明细状态恒 0
    #[must_use]
    pub fn active_conn_count(&self, t: &Task) -> usize {
        if !t.state.shows_conns() {
            return 0;
        }
        t.connections
            .iter()
            .filter(|c| self.conn_is_active(t.id, c))
            .count()
    }

    /// 连接活跃判定（v1.34 ConnPhase 数据面派生的 App 消费口）：以本轮累计
    /// 落盘字节（FR-01-99 ② 连接账本）派生连接阶段并判活跃
    pub(crate) fn conn_is_active(&self, task_id: u32, c: &Connection) -> bool {
        let cum = self.conn_cum.get(&(task_id, c.id)).copied().unwrap_or(0);
        c.conn_phase(cum).is_active()
    }

    /// 当前选中任务的活跃并发明细（显示行 → 连接 id 映射，v1.34/FR-01-96 ⑦：
    /// 明细只显示活跃并发——仅「下载中/做种中」任务，且连接处于 正在连接/
    /// 已连接 活跃阶段；面板渲染行序、↑↓ 选择、B 断开映射与本表同源，
    /// 按 id 升序 = 连接池槽位号序）
    pub(crate) fn active_conn_ids(&self) -> Vec<usize> {
        self.sel_task()
            .filter(|t| t.state.shows_conns())
            .map(|t| {
                let mut ids: Vec<usize> = t
                    .connections
                    .iter()
                    .filter(|c| self.conn_is_active(t.id, c))
                    .map(|c| c.id)
                    .collect();
                ids.sort_unstable();
                ids
            })
            .unwrap_or_default()
    }

    /// 页签计数（FR-01-94 ④ 页签行「正在下载 (n) │ 已完成 (n)」口径；
    /// architect v126 自渲染路径提取——原 header 内联聚合无直接单测面）：
    /// 下标对齐 [`FILTERS`]（0 = 未完成态计数，1 = `is_done` 态计数，
    /// 两项之和恒等于任务总数）。
    #[must_use]
    pub fn tab_counts(&self) -> [usize; 2] {
        let doing = self.tasks.iter().filter(|t| !t.state.is_done()).count();
        [doing, self.tasks.len() - doing]
    }

    /// 会话下载峰值（B/s；头部「峰值 ↓」口径：速度历史窗口内的最大采样，
    /// architect v145 自渲染路径提取——原 header 内联 max 聚合无直接单测面，
    /// 与「全局 ↓↑」提取同法）。速度历史存储口径为 KB/s 采样（见
    /// `speed_hist` 写入侧换算），此处负责 KB/s → B/s 还原，展示层零换算。
    #[must_use]
    pub fn peak_dl_speed(&self) -> f64 {
        self.speed_hist.iter().copied().max().unwrap_or(0) as f64 * 1024.0
    }

    /// 会话上传峰值（B/s；头部「峰值 ↑」口径，同 [`App::peak_dl_speed`]；
    /// 01 期 BT 未启用上传，历史恒 0）
    #[must_use]
    pub fn peak_ul_speed(&self) -> f64 {
        self.up_hist.iter().copied().max().unwrap_or(0) as f64 * 1024.0
    }

    /// 清空指定任务的连接级账本（新一次下载开始 / 任务移除 / 收尾时调用，FR-01-99）
    pub(super) fn clear_conn_stats(&mut self, task_id: u32) {
        self.conn_prev.retain(|(tid, _), _| *tid != task_id);
        self.conn_cum.retain(|(tid, _), _| *tid != task_id);
        self.conn_speed.retain(|(tid, _), _| *tid != task_id);
        self.conn_windows.retain(|(tid, _), _| *tid != task_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskState;

    fn make_app(tag: &str) -> App {
        let dir = crate::model::testenv::uniq_tmp_dir(&format!("ezr-app-{tag}"));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        App::new(Config::default(), reg)
    }

    fn task(id: u32, name: &str, state: TaskState) -> Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t.state = state;
        t
    }

    /// 页签计数访问器（architect v126 提取）：口径与页签行一致——
    /// 0 = 未完成态计数、1 = is_done 态计数，两项之和恒等于任务总数
    #[tokio::test]
    async fn tab_counts_split_by_done_state() {
        let mut app = make_app("tab");
        assert_eq!(app.tab_counts(), [0, 0], "空表双零");
        app.tasks.push(task(1, "a.bin", TaskState::Downloading));
        app.tasks.push(task(2, "b.bin", TaskState::Queued));
        app.tasks.push(task(3, "c.bin", TaskState::Paused));
        app.tasks.push(task(4, "d.bin", TaskState::Completed));
        app.tasks.push(task(5, "e.bin", TaskState::Seeding));
        let counts = app.tab_counts();
        assert_eq!(
            counts[0], 3,
            "未完成态 = 下载中/等待/暂停（含挂起，v1.23 数据同普通暂停）"
        );
        assert_eq!(counts[1], 2, "is_done = 已完成/做种");
        assert_eq!(counts[0] + counts[1], app.tasks.len(), "两页签不重不漏");
        // 状态翻转 → 计数随动（校验中属未完成页签）
        app.tasks[2].state = TaskState::Completed;
        assert_eq!(app.tab_counts(), [2, 3]);
        app.shutdown().await;
    }

    /// 会话峰值访问器（architect v145 提取）：速度历史窗口内最大采样的
    /// B/s 还原口径——空/零历史恒 0，写入侧 KB/s 采样 ×1024 还原
    #[tokio::test]
    async fn peak_speeds_max_of_history_in_bytes() {
        let mut app = make_app("peak");
        assert_eq!(app.peak_dl_speed(), 0.0, "零历史峰值 0");
        assert_eq!(app.peak_ul_speed(), 0.0, "上传历史恒 0（01 期无 BT）");
        app.speed_hist = vec![0, 12, 340, 5];
        app.up_hist = vec![0, 0, 0];
        assert_eq!(
            app.peak_dl_speed(),
            340.0 * 1024.0,
            "最大采样 ×1024 还原 B/s"
        );
        assert_eq!(app.peak_ul_speed(), 0.0);
        // 历史回缩（tick 窗口滑动剔除旧峰）→ 峰值随动（展示口径随窗口）
        app.speed_hist = vec![1, 2];
        assert_eq!(app.peak_dl_speed(), 2.0 * 1024.0);
        app.shutdown().await;
    }
}

#[cfg(test)]
mod sb_interaction_tests {
    //! v1.32/FR-01-109 通用滚动条交互状态机单测：箭头步进+按压高亮+连发、
    //! 轨道翻页方向、滑块拖拽反解、松开收尾、←→ 横向钳制与 About 模态。

    use crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::layout::Rect;

    use super::*;
    use crate::model::config::Config;
    use crate::model::TaskState;

    fn make_app(tag: &str) -> App {
        let dir = crate::model::testenv::uniq_tmp_dir(&format!("ezr-sb-{tag}"));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        App::new(Config::default(), reg)
    }

    fn dl_task(id: u32, name: &str) -> crate::model::Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t.state = TaskState::Downloading;
        t.probed = true;
        t.total = 5_000_000;
        t
    }

    /// 40×20 面板的垂直滚动条几何（total=30、page=5 → max 25，与下方
    /// 测试播种的 30 个任务一致——回填几何与真实滚动上限同源，模拟 ui 层
    /// 每帧回填的真实契约）
    fn vbar() -> SbBar {
        SbBar::vertical(Rect::new(0, 0, 40, 20), 30, 5, 0)
    }

    /// 以当前 scroll 重建回填几何（模拟下一帧 ui 层回填：pos 字段随帧刷新）
    fn refill(app: &mut App) -> SbBar {
        let bar = SbBar {
            pos: app.scroll,
            ..app.list_scrollbar.unwrap()
        };
        app.list_scrollbar = Some(bar);
        bar
    }

    #[tokio::test]
    async fn arrow_press_steps_and_focuses_panel() {
        let mut app = make_app("arrow");
        app.list_scrollbar = Some(vbar());
        app.visible_rows = 5;
        for _ in 0..30 {
            let mut t = dl_task(1, "t.bin");
            t.state = TaskState::Queued;
            app.tasks.push(t);
        }
        app.focus = Panel::Detail;
        // 尾箭头命中（几何：高 20 → 轨道 [2,17]；Tail 在行 18）
        let bar = app.list_scrollbar.unwrap();
        let hit = bar.hit(39, 18);
        assert!(matches!(hit, Some(SbHit::Tail)));
        app.sb_press(SbKey::List, &bar, hit.unwrap());
        assert_eq!(app.scroll, 1, "尾箭头步进 1");
        assert!(app.sb_hold.is_some(), "记录连发");
        assert_eq!(
            app.sb_pressed,
            Some((SbKey::List, SbArrow::Tail)),
            "按压高亮"
        );
        assert!(app.sb_flash.is_some(), "400ms 余辉");
        // 主界面按下滚动条同时聚焦所属面板（sb_press_at 入口语义）
        app.focus = Panel::Detail;
        assert!(app.sb_press_at(39, 18), "尾箭头再次命中");
        assert_eq!(app.focus, Panel::List, "按下滚动条聚焦所属面板");
        // 渲染查询：按压态可见
        assert_eq!(app.sb_arrow_pressed(SbKey::List), Some(SbArrow::Tail));
        // 头箭头：scroll 回 0 后再按被钳 0
        app.scroll = 0;
        let hit = bar.hit(39, 1);
        assert!(matches!(hit, Some(SbHit::Head)));
        app.sb_press(SbKey::List, &bar, hit.unwrap());
        assert_eq!(app.scroll, 0, "事件内立即钳制（末页/首页即停）");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn track_press_pages_toward_click_side() {
        let mut app = make_app("track");
        app.list_scrollbar = Some(vbar());
        app.visible_rows = 5;
        for _ in 0..30 {
            let mut t = dl_task(1, "t.bin");
            t.state = TaskState::Queued;
            app.tasks.push(t);
        }
        let bar = app.list_scrollbar.unwrap();
        // 首屏（pos 0）：滑块在轨道头部 → 轨道下半段点击 = 朝尾翻页
        let hit = bar.hit(39, 10);
        assert!(matches!(hit, Some(SbHit::Track(_))));
        app.sb_press(SbKey::List, &bar, hit.unwrap());
        assert_eq!(app.scroll, 5, "整页翻页（page = 可视 5）");
        assert!(app.sb_hold.as_ref().is_some_and(|h| h.page), "轨道连发按页");
        // 滑块拖到末页后（下一帧回填 pos）：轨道上半段点击 = 朝头翻页
        app.sb_set(SbKey::List, 25);
        let bar = refill(&mut app);
        let hit = bar.hit(39, 3);
        if let Some(SbHit::Track(t)) = hit {
            assert!(t < bar.thumb_pos(), "点击点在滑块上方");
            app.sb_press(SbKey::List, &bar, hit.unwrap());
            assert_eq!(app.scroll, 20, "朝头整页翻页（25 − page 5）");
        } else {
            panic!("轨道上半段应命中 Track");
        }
        app.shutdown().await;
    }

    #[tokio::test]
    async fn thumb_drag_inverse_and_release() {
        let mut app = make_app("thumb");
        app.list_scrollbar = Some(vbar());
        app.visible_rows = 5;
        for _ in 0..30 {
            let mut t = dl_task(1, "t.bin");
            t.state = TaskState::Queued;
            app.tasks.push(t);
        }
        let bar = app.list_scrollbar.unwrap();
        // 滑块命中（首屏滑块在轨道首格：行 2）→ 抓取偏移 0
        let hit = bar.hit(39, 2);
        assert!(matches!(hit, Some(SbHit::Thumb(0))));
        app.sb_press(SbKey::List, &bar, hit.unwrap());
        assert_eq!(app.sb_drag, Some((SbKey::List, 0)));
        // 拖到轨道中部（行 10 → 轨道下标 8）→ 反解滚动偏移
        app.sb_drag_move(39, 10);
        let expected = bar.pos_from_thumb(8);
        assert_eq!(app.scroll, expected, "拖拽反解与显示取整互逆");
        // 松开 = 结束拖拽/连发/按压
        app.sb_release();
        assert!(app.sb_drag.is_none() && app.sb_hold.is_none() && app.sb_pressed.is_none());
        app.shutdown().await;
    }

    #[tokio::test]
    async fn hold_autorepeat_first_delay_then_period() {
        let mut app = make_app("hold");
        app.list_scrollbar = Some(vbar());
        app.visible_rows = 5;
        for _ in 0..30 {
            let mut t = dl_task(1, "t.bin");
            t.state = TaskState::Queued;
            app.tasks.push(t);
        }
        let bar = app.list_scrollbar.unwrap();
        app.sb_press(SbKey::List, &bar, SbHit::Tail);
        assert_eq!(app.scroll, 1, "按下立即步进一次");
        // 起振前（<400ms）tick 不重复
        app.sb_hold_tick(Instant::now());
        assert_eq!(app.scroll, 1);
        // 起振（>400ms）→ 重复一次
        let later = Instant::now() + std::time::Duration::from_millis(450);
        app.sb_hold_tick(later);
        assert_eq!(app.scroll, 2, "0.4s 起振后重复");
        // 周期（0.12s 内）不重复
        app.sb_hold_tick(later + std::time::Duration::from_millis(60));
        assert_eq!(app.scroll, 2);
        // 周期到期 → 再重复
        app.sb_hold_tick(later + std::time::Duration::from_millis(130));
        assert_eq!(app.scroll, 3, "0.12s 周期重复");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn conns_horizontal_keys_clamp_to_zero_for_http() {
        let mut app = make_app("hkey");
        app.tasks.push(dl_task(1, "h.bin"));
        app.selected = 0;
        app.focus = Panel::Conns;
        // HTTP 内容 22 列常态不超宽 → conns_max_hscroll = 0（ui 回填）
        app.conns_max_hscroll = 0;
        app.on_key(KeyCode::Right, KeyModifiers::NONE);
        app.on_key(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(app.conns_hscroll, 0, "←/→ 无横向内容不响应（钳制归零）");
        app.on_key(KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(app.conns_hscroll, 0);
        // 非 Conns 聚焦不响应
        app.focus = Panel::List;
        app.conns_max_hscroll = 10;
        app.on_key(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(app.conns_hscroll, 0, "列表聚焦时 ←→ 不滚并发明细");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn about_modal_lifecycle_and_priority() {
        let mut app = make_app("about");
        // O 打开；位置记忆跨次保留
        app.on_key(KeyCode::Char('o'), KeyModifiers::NONE);
        assert!(app.about_open);
        assert!(app.overlay_open(), "浮层集合含关于窗口（FR-01-108 注记）");
        app.about_pos = Some((7, 9));
        app.close_about();
        assert!(!app.about_open && !app.overlay_open());
        app.open_about();
        assert_eq!(app.about_pos, Some((7, 9)), "位置跨次记忆");
        // 模态：主界面快捷键全部消费为无动作；Esc/O/q/Q 关闭不退出
        app.on_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "About 模态消费 A");
        app.on_key(KeyCode::Char('h'), KeyModifiers::NONE);
        assert!(!app.help_open, "About 模态消费 H");
        app.on_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.focus, Panel::List, "About 模态消费 Tab");
        app.on_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(!app.about_open && !app.quit, "q 先关窗不退出");
        // 帮助内 O：About 叠于帮助之上，关闭后回帮助
        app.help_open = true;
        app.on_key(KeyCode::Char('O'), KeyModifiers::NONE);
        assert!(app.about_open && app.help_open, "帮助内打开 About");
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.about_open && app.help_open, "关闭后回帮助");
        // 帮助内 Esc/H/q/Q 仍关闭帮助
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.help_open);
        app.shutdown().await;
    }
}
