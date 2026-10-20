//! ui.rs — EZR Downloader TUI Demo 的界面渲染
//!
//! 布局：顶部区 = 头部仪表面板（通栏全宽 5 行：标题边框行 + 字段行/分割线/页签行
//! 3 内容行 + 底边框，右缘与任务详情面板右缘对齐）。字段行单行排列、顺序固定：
//! 并发、会话已下载、全局 ↓↑、峰值 ↓↑（峰值写在面板内容里而非边框线上，峰值 ↓
//! 与流量图曲线同色兼作图例）；分割线仅贯通字段区宽；页签行 = 原「任务队列」面板
//! 内容并入：正在下载 (n) │ 已完成 (n) │ 下载槽位 x/y（槽位内联左对齐、紧跟
//! 已完成 页签并以 │ 分隔）。内容区右侧 = │ 竖线分隔符 + 左右各 1 列空白 + 内嵌
//! 无边框流量图（占内容区全高 3 行、宽为内容区 20%，仅绘制下行流量，上传流量
//! 不绘制）。主区：左列 58% = 任务列表；右列 42% = 任务详情
//! （10 行内容 = 12 行定高，v1.20：任务 ID 与创建时间拆两行）及并发连接面板
//! （占右列余下全部高度）。页脚 = 双行：行 1 常用命令图例（A 添加 / D 删除 /
//! M 修改 / Q 退出 / H 帮助；鼠标左键可点击——按压反色高亮 = 点击效果、原地
//! 松开执行对应命令，v0.2 页脚双行修订），行 2 toast 提示。
//! 流量图：单色面积图——逐列自底向上填充，顶线以 ▁▂▃▄▅▆▇█ 八级部分块呈现
//! 1/8 格亚字符精度的平滑曲线，全图统一一种颜色（下载淡蓝，REQ-8.5）；
//! 序列经双向 EMA 平滑 + 窗口 min–max 自适应缩放，每列取双子样本较大值保尖峰
//! ——面积首尾相接、无断列、无间隔、无盲文点阵。
//! 每个任务条目 3 行：标题行(名称+协议/状态徽标+百分比)、整体进度条(不分块)、统计行。
//! 协议徽标 [HTTP/HTTPS/BT] 统一黄色；详情页分块以文字显示 x/y（已完成/总块数）与块大小 N/块。
//! pieces 分块弹窗（P）支持鼠标左键拖拽移动（按住弹窗内空白处拖动；
//! 右缘滚动条上的拖拽 = 滚动网格内容，与主界面面板同一通用滚动条组件）。
//! 分块策略：块大小按协议写死（HTTP 1 MB / BT 256 KB），块数与并发数解耦。
//! 并发连接面板：BT 任务列序 = 对端 IP(掩码)/下载速度/累计下载/上传速度/
//! 累计上传/客户端/协议(TCP|uTP)（m3 新增 客户端/协议 两列置于末尾，内容超宽时底部
//! 出现通用横向滚动条 ←═→，←/→ 键横向滚动）；HTTP 任务列序 = 序号/下载速度/
//! 累计下载（无上传两列）；BT 首列对端 IP 以
//! 「IPv4 首段.*.末端（如 192.*.100）/ IPv6 首段:*:末端（如 2001:*:7334）」掩码显示；
//! 客户端 = 对端客户端两字母缩写（QB/TR/LT/UT/DE/AR/RT/TX/AZ/XL 演示池）。
//! 聚焦本面板后 ↑/↓ 选中行高亮（lazygit 式统一列表选择），B 断开选中连接。
//! 累计均为当前任务本次开始下载后的累计量（重新排队/重试时清零）；
//! 仅「下载中」与「做种中」状态显示并发明细，其余状态（等待中/已暂停/校验中/
//! 后期处理中/已失败/已完成）明细为空、活跃 0/0。
//! 对话框：添加任务(URL+目录+并发+校验算法下拉+校验码+代理下拉) /
//! 修改任务(并发+校验+校验码+代理，v1.5/FR-01-87 同步) / 删除任务(仅任务|任务和文件|取消)。
//! pieces 分块弹窗（P）：分块（piece）状态一览——🟩 已下载 / 🟦 正在下载（分配了
//! 并发下载）/ 🟨 部分下载 / ⬜ 未下载；同一块「部分下载」且「正在下载」时只显示
//! 🟦；仅支持分块下载的任务可打开（BT / 支持断点续传的 HTTP(S)）。

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{
    chunk_size_label, App, DialogKind, FailKind, Panel, PieceState, SbKey, Task, TaskState,
    CHECKSUM_ALGOS, FILTERS, ITEM_HEIGHT, MAX_DOWNLOAD_SLOTS,
};
use crate::scrollbar;

// ---------------------------------------------------------------------------
// 调色板（语义状态色；下载中=淡蓝，暂停=白，等待=黄，失败=红，完成=绿，做种=粉）
// ---------------------------------------------------------------------------

const BORDER: Color = Color::Rgb(56, 78, 86);
const FG: Color = Color::Rgb(198, 208, 212);
/// 两级弱化文字：标签、占位符、次级提示（滚动条箭头/轨道同用，§4.1.3）
pub(crate) const DIM: Color = Color::Rgb(126, 141, 147);
pub(crate) const DIM2: Color = Color::Rgb(88, 102, 108);
const SEL_BG: Color = Color::Rgb(22, 42, 48);
/// 强调色（页脚命令键、对话框按钮共用，§4.1.3；滚动条箭头按压高亮
/// 不再用 ACCENT——改用滑块同款前景色，见 scrollbar::render）
pub(crate) const ACCENT: Color = Color::Cyan;
const GREEN: Color = Color::Green;
const YELLOW: Color = Color::Rgb(228, 178, 62);
const RED: Color = Color::Rgb(226, 92, 84);
const MAGENTA: Color = Color::Rgb(214, 112, 214);
const EMPTY: Color = Color::Rgb(54, 68, 74);
/// 下载中/校验中/后期处理（淡蓝；亦为流量图唯一用色——REQ-8.5 单色面积图，
/// 与「峰值 ↓」图例同色）
const LIGHT_BLUE: Color = Color::Rgb(122, 185, 242);

// ---------------------------------------------------------------------------
// lazygit 式聚焦显示：当前面板边框高亮 + 非当前面板暗边框
// ---------------------------------------------------------------------------

/// 聚焦面板边框色（lazygit 主题默认 activeBorderColor = green + bold）
const FOCUS_BORDER: Color = Color::Green;

/// 是否存在浮层（对话框/pieces 弹窗/帮助窗口）：浮层打开时底层面板全部
/// 降级为暗边框，焦点面板不再高亮（视觉焦点让位给模态浮层）
fn overlay_open(app: &App) -> bool {
    app.dialog.is_some() || app.pieces_open || app.help_open
}

/// 面板边框样式：聚焦 = 绿色加粗（lazygit activeBorderColor 口径），
/// 未聚焦/浮层打开 = 暗色（inactiveBorderColor 口径）
fn panel_border_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(FOCUS_BORDER)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(BORDER)
    }
}

/// 面板标题样式：聚焦 = 绿色加粗，未聚焦/浮层打开 = 暗灰
fn panel_title_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(FOCUS_BORDER)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DIM)
    }
}

/// 通用滚动条渲染入口（垂直 ↑║↓ / 水平 ←═→，全系统唯一实现见
/// `scrollbar.rs`）：几何（滑块吸附/取整）、字形、命中测试全部由组件提供，
/// 各 panel 只是回填 `total/page/pos` 与面板矩形——保证不同 panel 获得完全
/// 相同的行为（滑块拖拽 / 箭头点击·按住连发 / 轨道翻页 / 方向键，§4.1.3）。
///
/// * `panel`：面板完整区域（含边框）——垂直画右边框列、水平画下边框行；
/// * `total`/`page`：内容总单位数 / 可视单位数（total <= page 时不绘制）；
/// * `position`：滚动偏移（组件内部钳制到 0..=total-page）；
/// * `thumb`：滑块颜色（跟随便属面板边框色：聚焦面板/弹窗均为高亮绿）；
/// * `pressed`：按压中的箭头（App 层按压/余辉态，None = 常规）——按下/
///   按住 = 前景色换成滑块同款（加粗、无背景色，§4.1.3）。
///
/// 返回本次渲染的滚动条几何（供 App 层命中测试；调用方回填到对应字段）。
fn draw_scrollbar(
    f: &mut Frame,
    panel: Rect,
    vertical: bool,
    total: usize,
    page: usize,
    position: usize,
    thumb: Color,
    pressed: Option<scrollbar::Arrow>,
) -> scrollbar::Bar {
    let bar = if vertical {
        scrollbar::Bar::vertical(panel, total, page, position)
    } else {
        scrollbar::Bar::horizontal(panel, total, page, position)
    };
    scrollbar::render(f, &bar, thumb, pressed);
    bar
}

fn state_color(s: TaskState) -> Color {
    match s {
        TaskState::Queued => YELLOW,
        TaskState::Downloading | TaskState::Verifying | TaskState::PostProcessing => LIGHT_BLUE,
        // v1.22/v1.23 展示与数据统一同步：失败挂起由 Paused 承载（色同白），
        // 徽标文案不细分——展示与普通「已暂停」一致（生产 v1.22 直判同步）
        TaskState::Paused => Color::White,
        TaskState::Completed => GREEN,
        TaskState::Failed => RED,
        TaskState::Seeding => MAGENTA,
    }
}

// ---------------------------------------------------------------------------
// 文本工具（CJK 宽度感知）
// ---------------------------------------------------------------------------

/// 估算终端显示宽度（CJK 等宽字符按 2 计）
fn w(s: &str) -> usize {
    s.chars()
        .map(|c| {
            let u = c as u32;
            if (0x2E80..=0xA4CF).contains(&u)
                || (0xAC00..=0xD7A3).contains(&u)
                || (0xF900..=0xFAFF).contains(&u)
                || (0xFF00..=0xFF60).contains(&u)
                || (0x3000..=0x303F).contains(&u)
                || (0xFFE0..=0xFFE6).contains(&u)
            {
                2
            } else {
                1
            }
        })
        .sum()
}

fn truncate(s: &str, max: usize) -> String {
    if w(s) <= max {
        return s.to_string();
    }
    let budget = max.saturating_sub(1);
    let mut out = String::new();
    let mut acc = 0;
    for ch in s.chars() {
        let cw = w(&ch.to_string());
        if acc + cw > budget {
            break;
        }
        acc += cw;
        out.push(ch);
    }
    out.push('…');
    out
}

/// 按终端显示宽度右填充空格（Rust format! 的宽度按字符数计，对 CJK 不准）
fn pad_right(s: &str, width: usize) -> String {
    let cur = w(s);
    if cur >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - cur))
    }
}

/// 按终端显示宽度左填充空格（数字/表格列右对齐用）
fn pad_left(s: &str, width: usize) -> String {
    let cur = w(s);
    if cur >= width {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(width - cur), s)
    }
}

fn fmt_size(bytes: u64) -> String {
    const MB: f64 = 1_000_000.0;
    const GB: f64 = 1_000_000_000.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= 1000.0 {
        format!("{:.0} KB", b / 1000.0)
    } else {
        format!("{} B", bytes)
    }
}

fn fmt_speed(bps: f64) -> String {
    const MB: f64 = 1_000_000.0;
    if bps >= MB {
        format!("{:.1} MB/s", bps / MB)
    } else if bps >= 1000.0 {
        format!("{:.0} KB/s", bps / 1000.0)
    } else {
        format!("{:.0} B/s", bps)
    }
}

fn fmt_eta(secs: Option<u64>) -> String {
    match secs {
        None => "--:--".to_string(),
        Some(s) => fmt_dur(s),
    }
}

fn fmt_dur(s: u64) -> String {
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, sec)
    } else {
        format!("{:02}:{:02}", m, sec)
    }
}

/// 同单位大小对（BT 行更紧凑）：1.72/4.32 GB
fn fmt_size_pair(a: u64, b: u64) -> String {
    const KB: f64 = 1_000.0;
    const MB: f64 = 1_000_000.0;
    const GB: f64 = 1_000_000_000.0;
    let (x, y) = (a as f64, b as f64);
    let unit = if y >= GB {
        ("GB", GB)
    } else if y >= MB {
        ("MB", MB)
    } else if y >= KB {
        ("KB", KB)
    } else {
        ("B", 1.0)
    };
    let (u, div) = unit;
    if u == "GB" {
        format!("{:.2}/{:.2} {}", x / div, y / div, u)
    } else if u == "MB" {
        format!("{:.1}/{:.1} {}", x / div, y / div, u)
    } else {
        format!("{:.0}/{:.0} {}", x / div, y / div, u)
    }
}

// ---------------------------------------------------------------------------
// 进度条
// ---------------------------------------------------------------------------

/// 列表整体进度条（不展示分块）
fn plain_bar(width: usize, frac: f64, fill: Color) -> Vec<Span<'static>> {
    let width = width.max(1);
    let filln = ((width as f64) * frac.clamp(0.0, 1.0)).round() as usize;
    let mut spans = Vec::new();
    if filln > 0 {
        spans.push(Span::styled("█".repeat(filln), Style::default().fg(fill)));
    }
    if filln < width {
        spans.push(Span::styled(
            "░".repeat(width - filln),
            Style::default().fg(EMPTY),
        ));
    }
    spans
}

// ---------------------------------------------------------------------------
// 任务条目（3 行）
// ---------------------------------------------------------------------------

fn task_lines(
    t: &Task,
    sel: bool,
    spinner: char,
    width: usize,
    queue_pos: usize,
) -> Vec<Line<'static>> {
    let width = width.max(20);
    let state_c = state_color(t.state);

    // ---- 行 1：光标 + 名称 + 徽标 + 右对齐百分比 ----
    // 等待中任务显示真实进度（可能持有断点进度）
    let pct_txt = format!("{:>5.1}%", t.progress() * 100.0);
    let proto_badge = format!("[{}]", t.protocol.label());
    // v1.22 展示统一同步：徽标文案 = 状态名单源，失败挂起不细分
    let state_badge = format!("[{}]", t.state.label());
    let cursor_w = 2usize;
    let badges_w = w(&proto_badge) + 1 + w(&state_badge) + 1;
    let pct_w = w(&pct_txt);
    let name_budget = width
        .saturating_sub(cursor_w + badges_w + pct_w + 1)
        .clamp(8, 64);
    let name_txt = truncate(&t.name, name_budget);
    let used = cursor_w + w(&name_txt) + 1 + badges_w;
    let pad = width.saturating_sub(used + pct_w);

    let l1 = vec![
        Span::styled(
            if sel { "▌ " } else { "  " }.to_string(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            name_txt,
            if sel {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG)
            },
        ),
        Span::raw(" "),
        Span::styled(
            proto_badge,
            // 协议徽标统一黄色
            Style::default().fg(YELLOW),
        ),
        Span::raw(" "),
        Span::styled(state_badge, Style::default().fg(state_c)),
        Span::raw(" ".repeat(pad)),
        Span::styled(
            pct_txt,
            Style::default().fg(state_c).add_modifier(Modifier::BOLD),
        ),
    ];

    // ---- 行 2：整体进度条（不分块）----
    let bar_w = width.saturating_sub(4);
    let mut l2 = vec![Span::raw("  ")];
    l2.extend(plain_bar(bar_w, t.progress(), state_c));
    // 行内动态提示（非状态文案，仅校验/后期处理的活动提示）
    match t.state {
        TaskState::Verifying => {
            l2.push(Span::raw("  "));
            let algo = t.checksum.as_ref().map(|c| c.algo).unwrap_or("SHA-256");
            l2.push(Span::styled(
                format!("{} 分块校验 {}", algo, spinner),
                Style::default().fg(LIGHT_BLUE),
            ));
        }
        TaskState::PostProcessing => {
            l2.push(Span::raw("  "));
            l2.push(Span::styled(
                format!("后期处理中 {}", spinner),
                Style::default().fg(LIGHT_BLUE),
            ));
        }
        _ => {}
    }

    // ---- 行 3：统计信息（不显示状态文案；缺失数值用 -）----
    let dash = Span::styled("-", Style::default().fg(DIM));
    let mut l3: Vec<Span<'static>> = vec![Span::raw("  ")];
    match t.state {
        TaskState::Failed => {
            // 已重试次数/上限 · 重试倒计时 · 已下载/需要下载
            let retry = format!("重试 {}/{}", t.retries, t.max_retries);
            let countdown = match t.retry_in {
                Some(s) => format!("{}s 后重试", s.ceil() as u64),
                // 无倒计时：区分「已达上限」与「不自动重试」（语义性 4xx /
                // 磁盘空间不足 / 校验失败直接停等，FR-M1-43/44/51）
                None => match t.fail_kind {
                    Some(FailKind::Fatal) | Some(FailKind::Verify) => "不自动重试".to_string(),
                    _ => "已达上限".to_string(),
                },
            };
            l3.push(Span::styled(retry, Style::default().fg(RED)));
            l3.push(Span::styled(" · ", Style::default().fg(RED)));
            l3.push(Span::styled(countdown, Style::default().fg(RED)));
            l3.push(Span::styled(" · ", Style::default().fg(RED)));
            l3.push(Span::styled(
                format!("{}/{}", fmt_size(t.downloaded), fmt_size(t.total)),
                Style::default().fg(RED),
            ));
        }
        TaskState::Queued => {
            // 槽位排队信息：位次按列表顺序从上往下（已获槽位的重试任务显示即将开始）
            if t.has_slot {
                l3.push(Span::styled(
                    "已获得下载槽位 · 即将开始".to_string(),
                    Style::default().fg(YELLOW),
                ));
            } else if queue_pos > 0 {
                l3.push(Span::styled(
                    format!("排队第 {} 位 · 等待空闲下载槽位", queue_pos),
                    Style::default().fg(YELLOW),
                ));
            }
            l3.push(Span::styled(" · ", Style::default().fg(DIM)));
            l3.push(Span::styled(
                format!("{}/{}", fmt_size(t.downloaded), fmt_size(t.total)),
                Style::default().fg(FG),
            ));
        }
        TaskState::Completed => {
            // 校验情况（按算法显示）+ 文件大小
            match (&t.checksum, t.verify_ok) {
                (Some(ck), Some(true)) => l3.push(Span::styled(
                    format!("{} 校验成功", ck.algo),
                    Style::default().fg(GREEN),
                )),
                (Some(ck), Some(false)) => l3.push(Span::styled(
                    format!("{} 校验失败", ck.algo),
                    Style::default().fg(RED),
                )),
                _ => l3.push(Span::styled("无校验".to_string(), Style::default().fg(DIM))),
            }
            l3.push(Span::styled(" · ", Style::default().fg(DIM)));
            l3.push(Span::styled(fmt_size(t.total), Style::default().fg(FG)));
        }
        TaskState::Seeding => {
            // ↑ 速度 · 种子/节点 · 已上传/需要下载 · 剩余做种时间
            l3.push(Span::styled(
                format!("↑ {}", fmt_speed(t.upload_speed)),
                Style::default().fg(MAGENTA),
            ));
            l3.push(Span::styled(
                format!("  {}/{}", t.seeders, t.peers),
                Style::default().fg(MAGENTA),
            ));
            l3.push(Span::styled(
                format!("  已上传 {}", fmt_size_pair(t.uploaded, t.total)),
                Style::default().fg(FG),
            ));
            l3.push(Span::styled(
                format!("  剩余做种 {}", fmt_dur(t.seed_left.ceil() as u64)),
                Style::default().fg(DIM),
            ));
        }
        _ if t.protocol.is_bt() => {
            // BT 其余状态：同下载中版式，缺失用 -
            let downloading = t.state == TaskState::Downloading;
            if downloading {
                l3.push(Span::styled(
                    format!("↓ {}", fmt_speed(t.speed)),
                    Style::default().fg(ACCENT),
                ));
                l3.push(Span::styled(
                    format!(" ↑ {}", fmt_speed(t.upload_speed)),
                    Style::default().fg(MAGENTA),
                ));
            } else {
                // 非下载中（暂停/排队等）：速度缺失用 -，但 ↓/↑ 箭头保留
                l3.push(Span::raw("↓ "));
                l3.push(dash.clone());
                l3.push(Span::raw(" ↑ "));
                l3.push(dash.clone());
            }
            if downloading {
                l3.push(Span::styled(
                    format!("  {}/{}", t.seeders, t.peers),
                    Style::default().fg(FG),
                ));
            } else {
                l3.push(Span::raw("  -/-"));
            }
            l3.push(Span::styled(
                format!("  {}", fmt_size_pair(t.downloaded, t.total)),
                Style::default().fg(FG),
            ));
            if downloading {
                l3.push(Span::styled(
                    format!("  剩余 {}", fmt_eta(t.eta_secs())),
                    Style::default().fg(DIM),
                ));
            } else {
                l3.push(Span::raw("  剩余 "));
                l3.push(dash.clone());
            }
        }
        _ => {
            // HTTP 其余状态：同下载中版式，缺失用 -
            let downloading = t.state == TaskState::Downloading;
            if downloading {
                l3.push(Span::styled(
                    format!("↓ {}", fmt_speed(t.speed)),
                    Style::default().fg(ACCENT),
                ));
            } else {
                l3.push(Span::raw("↓ "));
                l3.push(dash.clone());
            }
            l3.push(Span::styled(
                format!("  {}/{}", fmt_size(t.downloaded), fmt_size(t.total)),
                Style::default().fg(FG),
            ));
            if downloading {
                l3.push(Span::styled(
                    format!("  剩余 {}", fmt_eta(t.eta_secs())),
                    Style::default().fg(DIM),
                ));
            } else {
                l3.push(Span::raw("  剩余 "));
                l3.push(dash.clone());
            }
        }
    }

    vec![
        Line::from(l1),
        Line::from(l2),
        Line::from(l3),
        Line::from(vec![Span::raw("")]),
    ]
}

fn render_task(f: &mut Frame, t: &Task, area: Rect, sel: bool, spinner: char, queue_pos: usize) {
    let text = task_lines(t, sel, spinner, area.width as usize, queue_pos);
    let para = Paragraph::new(text).style(if sel {
        Style::default().bg(SEL_BG)
    } else {
        Style::default()
    });
    f.render_widget(para, area);
}

// ---------------------------------------------------------------------------
// 各区块渲染
// ---------------------------------------------------------------------------

fn draw_header(f: &mut Frame, app: &mut App, area: Rect, with_chart: bool) {
    let dl: f64 = app
        .tasks
        .iter()
        .filter(|t| t.state == TaskState::Downloading)
        .map(|t| t.speed)
        .sum();
    let ul: f64 = app
        .tasks
        .iter()
        .filter(|t| t.upload_speed > 0.0)
        .map(|t| t.upload_speed)
        .sum();
    // 会话内峰值（速度历史为 KB/s）与全局并发连接数（下载中/做种中任务的连接总数）
    let peak_dl = app.speed_hist.iter().copied().max().unwrap_or(0);
    let peak_ul = app.up_hist.iter().copied().max().unwrap_or(0);
    let conns_n: usize = app
        .tasks
        .iter()
        .filter(|t| t.state.shows_conns())
        .map(|t| t.connections.len())
        .sum();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(Line::from(vec![
            Span::styled(
                " ◆ ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "EZR Downloader",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  v0.1.0-m1", Style::default().fg(DIM)),
        ]));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // 右侧内嵌流量图：│ 分隔符 + 左右各 1 列空白 + 图宽 = 内容区 20%（第七轮
    // 较此前 40% 减半），并保证字段区至少 92 列（不足时压缩图宽）；占内容区全高
    let chart_w = if with_chart && inner.height >= 3 && inner.width >= 49 {
        (inner.width / 5).clamp(6, inner.width.saturating_sub(95).max(6))
    } else {
        0
    };
    let fields_w = if chart_w > 0 {
        inner.width - chart_w - 3
    } else {
        inner.width
    };

    // 字段着色：标签暗灰；全局 ↓ 青(加粗)/↑ 品红；峰值 ↓ 淡蓝（与曲线同色即图例）/↑ 品红
    let lab = |s: &str| Span::styled(s.to_string(), Style::default().fg(DIM));
    let val = |s: &str| Span::styled(s.to_string(), Style::default().fg(FG));
    let dl_now = |s: &str| {
        Span::styled(
            s.to_string(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )
    };
    let ul_now = |s: &str| Span::styled(s.to_string(), Style::default().fg(MAGENTA));
    let dl_peak = |s: &str| Span::styled(s.to_string(), Style::default().fg(LIGHT_BLUE));

    let dls = format!("↓ {}", fmt_speed(dl));
    let uls = format!("↑ {}", fmt_speed(ul));
    let dlp = format!("↓ {}", fmt_speed(peak_dl as f64 * 1024.0));
    let ulp = format!("↑ {}", fmt_speed(peak_ul as f64 * 1024.0));
    let sess = fmt_size(app.session_bytes);

    // 字段行 1：单行排列，顺序固定：并发 → 会话已下载 → 全局 → 峰值（第七轮合并为一行）
    let fields = Line::from(vec![
        lab(" 并发 "),
        val(&conns_n.to_string()),
        lab("   会话已下载 "),
        val(&sess),
        lab("   全局 "),
        dl_now(&dls),
        ul_now(&format!(" {}", uls)),
        lab("   峰值 "),
        dl_peak(&dlp),
        ul_now(&format!(" {}", ulp)),
    ]);
    f.render_widget(
        Paragraph::new(fields),
        Rect {
            x: inner.x,
            y: inner.y,
            width: fields_w,
            height: 1,
        },
    );

    // 行 2：横向分割线（贯通字段区宽、止于 │ 分隔符；图区让位给波形不画线）
    if inner.height >= 2 {
        let sep = "─".repeat(fields_w as usize);
        f.render_widget(
            Paragraph::new(Span::styled(sep, Style::default().fg(BORDER))),
            Rect {
                x: inner.x,
                y: inner.y + 1,
                width: fields_w,
                height: 1,
            },
        );
    }

    // 行 3：页签行（原「任务队列」面板内容并入）：正在下载 (n) │ 已完成 (n) │
    // 下载槽位 x/y（槽位内联左对齐；已满黄色加粗：等待任务需排队）
    // 页签高亮跟随任务列表面板焦点（lazygit 式）：列表聚焦时当前页签
    // 黑字青底加粗；焦点在别的面板/浮层打开时页签全部不高亮（当前页签
    // 仅青字加粗保留可辨识度）
    if inner.height >= 3 {
        let doing = app.tasks.iter().filter(|t| !t.state.is_done()).count();
        let done = app.tasks.len() - doing;
        let counts = [doing, done];
        let row_y = inner.y + 2;
        let list_hot = app.focus == Panel::List && !overlay_open(app);
        // 页签可点击区域（需求修订：鼠标点击切换页签）：每帧重建，
        // 记录各页签 span 的命中矩形（不含分隔符）
        app.tab_rects.clear();
        let mut spans: Vec<Span> = Vec::new();
        let mut tabs_w: u16 = 0;
        for (i, label) in FILTERS.iter().enumerate() {
            let text = format!(" {} ({}) ", label, counts[i]);
            let tab_rect = Rect {
                x: inner.x + tabs_w,
                y: row_y,
                width: w(&text) as u16,
                height: 1,
            };
            let span = Span::styled(
                text,
                if i == app.filter {
                    if list_hot {
                        Style::default()
                            .fg(Color::Black)
                            .bg(ACCENT)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(ACCENT)
                            .add_modifier(Modifier::BOLD)
                    }
                } else {
                    Style::default().fg(FG)
                },
            );
            tabs_w += span.width() as u16;
            spans.push(span);
            app.tab_rects.push((tab_rect, i));
            if i + 1 < FILTERS.len() {
                let dv = Span::styled("│", Style::default().fg(DIM2));
                tabs_w += dv.width() as u16;
                spans.push(dv);
            }
        }
        // 下载槽位：内联左对齐，紧跟「已完成」页签之后并以 │ 分隔（第八轮：
        // 由行尾右对齐改为随页签同行排列）
        let slots = app.used_slots();
        let slots_full = slots >= MAX_DOWNLOAD_SLOTS;
        let slot = Span::styled(
            format!(" 下载槽位 {}/{}", slots, MAX_DOWNLOAD_SLOTS),
            Style::default()
                .fg(if slots_full { YELLOW } else { DIM })
                .add_modifier(if slots_full {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        );
        if fields_w as usize >= tabs_w as usize + 1 + slot.width() {
            spans.push(Span::styled("│", Style::default().fg(DIM2)));
            spans.push(slot);
        }
        f.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect {
                x: inner.x,
                y: row_y,
                width: fields_w,
                height: 1,
            },
        );
    }

    // │ 分隔符：字段区与图区之间，贯通头部内容区全高（第八轮）
    if chart_w > 0 {
        let sep_x = inner.x + fields_w;
        for y in 0..inner.height {
            f.render_widget(
                Paragraph::new(Span::styled("│", Style::default().fg(BORDER))),
                Rect {
                    x: sep_x,
                    y: inner.y + y,
                    width: 1,
                    height: 1,
                },
            );
        }
        // 图：分隔符右侧留 1 列空白、图区右缘再留 1 列空白（无边框、占全高）
        let chart_area = Rect {
            x: inner.x + fields_w + 2,
            width: chart_w,
            ..inner
        };
        draw_chart(f, app, chart_area);
    }
}

fn draw_list(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Panel::List && !overlay_open(app);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(focused))
        .title(Span::styled(" 下载任务 ", panel_title_style(focused)))
        .title(
            Line::from(Span::styled(
                format!(" {}/{} 项 ", app.selected + 1, app.filtered().len()),
                Style::default().fg(DIM),
            ))
            .alignment(Alignment::Right),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);

    let idxs = app.filtered();
    // 可见任务块数：完整块 = 3 行内容 + 1 行块间空行（ITEM_HEIGHT = 4）；
    // 尾部余量 ≥ 3 行（缺的只是空行）时额外渲染一个末尾部分块——
    // Paragraph 渲染进 3 行区域自动裁掉末尾空行，不吞边框
    let ih = ITEM_HEIGHT as usize;
    let rows = inner.height as usize / ih + usize::from(inner.height as usize % ih >= ih - 1);

    if rows == 0 || idxs.is_empty() {
        let msg = if idxs.is_empty() {
            // 空态提示（Gherkin 01-tui-display-13 同步：显示「按 A 添加下载任务」）
            "（此页签下没有任务，按 A 添加下载任务）"
        } else {
            "（区域过小）"
        };
        f.render_widget(
            Paragraph::new(Span::styled(msg, Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        app.visible_rows = 0;
        app.list_area = Some(inner);
        app.list_sb = None; // 空态无滚动条（命中几何同步清空）
        return;
    }

    let spinner = app.spinner();
    let max_items = rows.min(idxs.len().saturating_sub(app.scroll));
    // 排队位次：按列表顺序从上往下，等待槽位的任务依次编号（1 基；已获槽位/非等待 = 0）
    let mut waiting = 0usize;
    let qpos: Vec<usize> = idxs
        .iter()
        .map(|&ti| {
            let t = &app.tasks[ti];
            if t.state == TaskState::Queued && !t.has_slot {
                waiting += 1;
                waiting
            } else {
                0
            }
        })
        .collect();
    for vi in 0..max_items {
        let ti = app.scroll + vi;
        let t = &app.tasks[idxs[ti]];
        let y = inner.y + (vi * ih) as u16;
        let row_area = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: (ih as u16).min(inner.bottom().saturating_sub(y)),
        };
        render_task(f, t, row_area, ti == app.selected, spinner, qpos[ti]);
    }

    // 通用滚动条（任务数超出一屏时；画在面板右边框列，↑↓ 箭头 + │轨道 + ║滑块）。
    // 几何回填给 App：滑块拖拽 / 箭头点击·按住连发 / 轨道翻页全组件化（§4.1.3）
    if idxs.len() > rows {
        let thumb = if focused { FOCUS_BORDER } else { BORDER };
        let bar = draw_scrollbar(
            f,
            area,
            true,
            idxs.len(),
            rows,
            app.scroll,
            thumb,
            app.sb_arrow_pressed(SbKey::List),
        );
        app.list_sb = Some(bar);
    } else {
        app.list_sb = None;
    }

    app.visible_rows = max_items;
    app.list_area = Some(inner);
}

fn draw_detail(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Panel::Detail && !overlay_open(app);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(focused))
        .title(Span::styled(" 任务详情 ", panel_title_style(focused)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    // 详情面板内框（鼠标点击聚焦本面板；需求修订-1）
    app.detail_area = Some(inner);

    let Some(t) = app.sel_task() else {
        // 无选中任务：字段名热区清空（修订-5）
        app.detail_url_rect = None;
        app.detail_ck_rect = None;
        f.render_widget(
            Paragraph::new(Span::styled("（未选中任务）", Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    };

    let label = |s: &str| Span::styled(pad_right(s, 9), Style::default().fg(DIM));
    // 修订-5：可点击复制的字段名（URL/校验）= 蓝色链接样式；点击行为见 on_mouse。
    // 修订-6：下划线仅覆盖字段名文字部分（pad_right 填充空格不带下划线），
    // 热区仍为整段 9 列（与 label 对齐块一致，便于点击）
    let label_link = |s: &str| -> Vec<Span<'static>> {
        vec![
            Span::styled(
                s.to_string(),
                Style::default()
                    .fg(LIGHT_BLUE)
                    .add_modifier(Modifier::UNDERLINED),
            ),
            Span::raw(" ".repeat(9 - w(s))),
        ]
    };
    let val = |s: String| Span::styled(s, Style::default().fg(FG));
    let state_c = state_color(t.state);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            truncate(&t.name, (inner.width as usize).saturating_sub(4)),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    // 等待中任务：显示槽位排队详情（位次按列表顺序从上往下；已获槽位 = 重试排队即将开始）
    if t.state == TaskState::Queued {
        let vtxt = match app.queue_pos_of(t.id) {
            // 紧凑文案：详情面板较窄，保证「Space 暂停」提示不被截断
            Some(p) => format!(
                "排队第 {} 位（{}/{}）· Space 暂停",
                p,
                app.used_slots(),
                MAX_DOWNLOAD_SLOTS
            ),
            None => "已获得下载槽位，即将开始下载".to_string(),
        };
        lines.push(Line::from(vec![
            Span::raw(" "),
            label("排队"),
            Span::styled(
                truncate(&vtxt, (inner.width as usize).saturating_sub(12)),
                Style::default().fg(YELLOW),
            ),
        ]));
    }
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("任务 ID"),
        Span::styled(format!("{id}", id = t.id), Style::default().fg(DIM)),
    ]));
    // 创建时间单独一行（v1.20 生产同步，操作者 20261011 指令：原「ID · 添加于」
    // 合并行拆分；ID 为随机 u32，仅详情页展示）
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("创建时间"),
        Span::styled(t.created.clone(), Style::default().fg(DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("类型"),
        Span::styled(
            t.protocol.label().to_string(),
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ),
        // 修订-3：类型行不再展示并发数，仅保留协议与断点续传支持
        {
            let resume_txt = if t.resumable {
                "支持断点续传"
            } else {
                "不支持断点续传"
            };
            val(format!(" · {}", resume_txt))
        },
    ]));
    // 校验行（提供了校验码的任务）：算法 · 校验码前缀 + 状态
    // 前缀取 10 位、状态用短文案，保证 110 列窄面板也能完整显示
    let mut ck_line_idx: Option<usize> = None;
    if let Some(ck) = &t.checksum {
        let vshort: String = if ck.value.chars().count() > 10 {
            format!("{}…", ck.value.chars().take(10).collect::<String>())
        } else {
            ck.value.clone()
        };
        let (st, stc) = match (t.verify_ok, t.state) {
            (Some(true), _) => ("（已通过）", GREEN),
            (Some(false), _) => ("（未通过）", RED),
            (None, TaskState::Verifying) => ("（校验中）", LIGHT_BLUE),
            _ => ("（待校验）", DIM),
        };
        ck_line_idx = Some(lines.len());
        let mut ck_line = vec![Span::raw(" ")];
        ck_line.extend(label_link("校验"));
        ck_line.push(Span::styled(
            ck.algo.to_string(),
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ));
        ck_line.push(Span::styled(
            format!(" · {}", vshort),
            Style::default().fg(FG),
        ));
        ck_line.push(Span::styled(st, Style::default().fg(stc)));
        lines.push(Line::from(ck_line));
    }
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("大小"),
        // v1.12/FR-01-80 同步：仅「已下载/总大小」，不含「（剩余 …）」后缀
        // （进度展示由列表行承载，FR-01-17 口径不变）
        val(format!(
            "{} / {}",
            fmt_size(t.downloaded),
            fmt_size(t.total)
        )),
    ]));
    // 失败原因：仅「已失败」态展示（生产 v1.22 直判同步——失败挂起不再展示，
    // 详情与普通已暂停完全一致）
    if t.state == TaskState::Failed {
        lines.push(Line::from(vec![
            Span::raw(" "),
            label("失败原因"),
            Span::styled(
                truncate(
                    t.error.as_deref().unwrap_or("未知错误"),
                    (inner.width as usize).saturating_sub(12),
                ),
                Style::default().fg(RED),
            ),
        ]));
    }
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("保存"),
        val(truncate(
            &t.save_path,
            (inner.width as usize).saturating_sub(12),
        )),
    ]));
    // 修订-5：URL 字段名可点击复制（蓝色下划线）；记录行号供热区回填
    let url_line_idx = lines.len();
    let mut url_line = vec![Span::raw(" ")];
    url_line.extend(label_link("URL"));
    url_line.push(val(truncate(
        &t.url,
        (inner.width as usize).saturating_sub(12),
    )));
    lines.push(Line::from(url_line));

    // 分块：文字显示——x/y（x=已完成分块数，y=总分块数）与块大小「N/块」；
    // 块大小按协议写死（HTTP 1 MB / BT 256 KB），块数 = ceil(total/块大小)
    {
        let (x, y, _) = t.chunk_info();
        let mut l = vec![Span::raw(" "), label("分块")];
        if t.total == 0 || y == 0 {
            l.push(Span::styled("-".to_string(), Style::default().fg(DIM)));
        } else {
            l.push(Span::styled(
                format!("{}/{}", x, y),
                Style::default().fg(state_c).add_modifier(Modifier::BOLD),
            ));
            l.push(Span::styled(
                format!(" · {}/块", chunk_size_label(t.protocol)),
                Style::default().fg(FG),
            ));
        }
        lines.push(Line::from(l));
    }

    // （FR-01-81 修订二同步）并发分块明细表已整体移除：详情面板止于任务级字段行。

    f.render_widget(Paragraph::new(lines), inner);

    // 修订-5：回填字段名热区（点击复制）。热区 = 行首空格后的字段名整段
    // （pad_right 9 列，与下划线视觉一致）；y = 内框顶 + 行号，超出内框则不设
    let hot = |idx: usize| {
        let y = inner.y + idx as u16;
        (y < inner.y + inner.height).then(|| Rect {
            x: inner.x + 1,
            y,
            width: 9,
            height: 1,
        })
    };
    app.detail_url_rect = hot(url_line_idx);
    app.detail_ck_rect = ck_line_idx.and_then(hot);
}

/// 流量图（内嵌于头部面板右侧、**无边框**；第八轮定稿：单色面积图，REQ-8.5）。
/// 逐列自底向上填充，每字符行按 1/8 分级（▁▂▃▄▅▆▇█ 八级部分块）→ 亚字符精度的
/// 平滑顶部曲线；**全图统一一种颜色**（下载淡蓝 LIGHT_BLUE，与「峰值 ↓」图例同色）。
/// 序列取最近 2×(列数+1) 个样本、双向 EMA 平滑（前向消突刺、反向消相位滞后），
/// 窗口 min–max 自适应缩放（上下呼吸边距）纵贯图区，每列取双子样本较大值保尖峰
/// ——面积首尾相接、无断列、无间隔、无盲文点阵。
fn draw_chart(f: &mut Frame, app: &App, area: Rect) {
    if area.width < 2 || area.height < 1 || app.speed_hist.len() < 2 {
        return;
    }
    let w = area.width as usize;
    let h = area.height as usize;
    // 亚字符精度：每字符行 8 级部分块（▁▂▃▄▅▆▇█），纵向总分辨率 = h×8
    const PARTIAL: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let levels = h * 8;

    // 取最近 2×(列数+1) 个样本（每列两个子采样），双向 EMA 平滑
    let n = (((w + 1) * 2).min(app.speed_hist.len())).max(2);
    let start = app.speed_hist.len() - n;
    let raw: Vec<f64> = app.speed_hist[start..].iter().map(|&v| v as f64).collect();
    const ALPHA: f64 = 0.75;
    let mut sm = raw.clone();
    for i in 1..sm.len() {
        sm[i] = sm[i - 1] + (raw[i] - sm[i - 1]) * ALPHA;
    }
    for i in (0..sm.len() - 1).rev() {
        sm[i] += (sm[i + 1] - sm[i]) * ALPHA;
    }

    // 窗口 min–max 自适应缩放（上下呼吸边距）→ 波形纵贯图区；
    // 边距下限取窗口峰值 5%，保证稳态抖动在图上仍有可见起伏
    let hi = sm.iter().copied().fold(f64::MIN, f64::max);
    let lo = sm.iter().copied().fold(f64::MAX, f64::min);
    let pad = ((hi - lo) * 0.15).max(hi * 0.05).max(1.0);
    let y_min = (lo - pad).max(0.0);
    let span = (hi + pad - y_min).max(1.0);
    let last = sm.len() - 1;

    // 逐列取两个子采样的较大值（保尖峰）→ 归一化高度 t ∈ [0,1] → 亚像素高度
    let px: Vec<usize> = (0..w)
        .map(|c| {
            let mut m = 0.0f64;
            for k in 0..2 {
                let s = ((c * 2 + k) as f64 + 0.5) * last as f64 / (w * 2) as f64;
                let i = (s as usize).min(last);
                let v = sm[i] + (sm[(i + 1).min(last)] - sm[i]) * (s - i as f64);
                let t = ((v - y_min) / span).clamp(0.0, 1.0);
                m = m.max(t);
            }
            ((m * levels as f64).round() as usize).min(levels)
        })
        .collect();

    // 逐行渲染（REQ-8.5：全图仅一种颜色，统一下载淡蓝，不再按深度渐变）：
    // rem = 列亚像素高度 − 本行亚像素带下界 → ≤0 空格 / ≥8 全块 █ / 其余部分块
    for row in 0..h {
        let band0 = (h - 1 - row) * 8; // 本行覆盖的亚像素带 [band0, band0+8)
        let line: String = px
            .iter()
            .map(|&p| {
                let rem = p as isize - band0 as isize;
                if rem <= 0 {
                    ' '
                } else {
                    PARTIAL[(rem.min(8) - 1) as usize]
                }
            })
            .collect();
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                line,
                Style::default().fg(LIGHT_BLUE),
            ))),
            Rect {
                x: area.x,
                y: area.y + row as u16,
                width: area.width,
                height: 1,
            },
        );
    }
}

/// 并发连接面板（任务详情下方）：每个连接一行——
/// BT 任务列序 = 对端 IP(掩码)/下载速度/累计下载/上传速度/累计上传/客户端/协议
/// （m3 新增 客户端/协议 两列置于末尾；宽度超内框 → 底部横向滚动条 ←═→，←/→ 键与
/// 滚轮横向滚动），按连接建立顺序；
/// HTTP 任务列序 = 序号/下载速度/累计下载（无上传两列），按序号（1 基）升序。
/// 对端 IP 掩码：IPv4「首段.*.末端」（如 192.*.100）、IPv6「首段:*:末端」（如 2001:*:7334）。
/// 客户端 = 对端客户端两字母缩写（QB=qBittorrent / TR=Transmission / LT=libtorrent …），
/// 协议 = 传输协议 TCP / uTP（BT 专用列，演示数据确定性生成）。
/// 累计上传/下载均为当前任务本次开始下载后的累计量（重试/重新排队开新一次下载时清零）；
/// 仅「下载中」与「做种中」状态的任务有明细，其余状态明细为空、活跃 0/0
/// （面板为空时滚轮穿透滚任务列表，Ctrl+↑↓/Ctrl+B 给出状态提示 toast）。
/// ↑/↓ 选中行高亮（跟随滚动），B 断开选中连接。
/// 连接数超出面板高度时纵向滚动（指针悬停于本面板时滚轮滚动这里而非任务列表）。
fn draw_conns(f: &mut Frame, app: &mut App, area: Rect) {
    // 通用滚动条命中几何与横向滚动上限：先按空面板清底，有明细时再回填
    app.conns_v_sb = None;
    app.conns_h_sb = None;
    app.conns_max_hscroll = 0;
    // 修订-3：标题仅展示活跃并发数（原「活跃 x/y」中的上限 y 已移除）
    let (is_bt, _total_n, active, empty) = match app.sel_task() {
        Some(t) => {
            // 仅「下载中」与「做种中」任务显示并发明细：
            // 其余状态（等待中/已暂停/校验中/后期处理中/已失败/已完成）按空面板处理
            let shows = t.state.shows_conns();
            (
                t.protocol.is_bt(),
                if shows { t.connections.len() } else { 0 },
                if shows {
                    t.connections
                        .iter()
                        .filter(|c| c.speed > 0.0 || c.up_speed > 0.0)
                        .count()
                } else {
                    0
                },
                !shows || t.connections.is_empty(),
            )
        }
        None => (false, 0, 0, true),
    };
    // 聚焦状态（并发连接面板边框/标题/选中行高亮跟随；滚轮滚动不受影响）
    let focused = app.focus == Panel::Conns && !overlay_open(app);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(focused))
        .title(Span::styled(" 并发连接 ", panel_title_style(focused)))
        .title(
            Line::from(Span::styled(
                format!("活跃 {} ", active),
                Style::default().fg(DIM),
            ))
            .alignment(Alignment::Right),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);
    app.conns_area = Some(inner);

    // 面板结构：表头 1 行 + 数据行（提示行已删除，全部空间让给数据）
    let rows = (inner.height as usize).saturating_sub(1);

    let Some(t) = app.sel_task() else {
        app.visible_conns_rows = 0;
        // 无选中任务：面板不接收滚轮（指针悬停时滚轮滚任务列表）
        app.conns_area = None;
        f.render_widget(
            Paragraph::new(Span::styled("（未选中任务）", Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    };
    let conn_total = t.connections.len(); // 预提取（后续写回 app 需先结束 t 借用）
    if empty {
        app.visible_conns_rows = 0;
        // 明细为空：面板不接收滚轮（指针悬停时滚轮滚任务列表）
        app.conns_area = None;
        f.render_widget(
            Paragraph::new(Span::styled("（无并发连接）", Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    // 并发表列宽（显示列，单空格分隔）——表头/数据/内容总宽共用同一份定义，
    // 保证横向滚动上限与实际渲染宽度永远一致：
    // BT   = IP(11) 下载速度(8) 累计下载(8) 上传速度(8) 累计上传(8) 客户端(6) 协议(4)
    // HTTP = 序号(4) 下载速度(8) 累计下载(8)
    let colw: Vec<usize> = if is_bt {
        vec![11, 8, 8, 8, 8, 6, 4]
    } else {
        vec![4, 8, 8]
    };
    let content_w = colw.iter().sum::<usize>() + colw.len().saturating_sub(1);

    let dim = |s: String, wd: usize| Span::styled(pad_left(&s, wd), Style::default().fg(DIM));
    // 表头（m3：BT 新增 客户端/协议 两列；IP 掩码列宽 11：IPv4「192.*.100」
    // 最长 9、IPv6「2001:*:7334」最长 11；客户端列宽 6 容纳 CJK 表头）
    let head = if is_bt {
        vec![
            Span::styled(pad_right("IP", 11), Style::default().fg(DIM)),
            Span::raw(" "),
            dim("下载速度".to_string(), 8),
            Span::raw(" "),
            dim("累计下载".to_string(), 8),
            Span::raw(" "),
            dim("上传速度".to_string(), 8),
            Span::raw(" "),
            dim("累计上传".to_string(), 8),
            Span::raw(" "),
            dim("客户端".to_string(), 6),
            Span::raw(" "),
            dim("协议".to_string(), 4),
        ]
    } else {
        vec![
            dim("序号".to_string(), 4),
            Span::raw(" "),
            dim("下载速度".to_string(), 8),
            Span::raw(" "),
            dim("累计下载".to_string(), 8),
        ]
    };
    let mut lines = vec![Line::from(head)];

    // 起点钳制到「最后一个满窗」起点（渲染层兜底）：即使滚动态短暂越界
    // （连接数收缩的过渡帧），窗口也完整覆盖尾部数据行，不产生底部空白行
    //（滚到底闪烁修复的第二道防线；顺带消除 len==0 的潜在减法恐慌）。
    let start = app
        .conns_scroll
        .min(t.connections.len().saturating_sub(rows));
    let data_rows = rows;
    let end = (start + data_rows).min(t.connections.len());

    for (i, c) in t.connections[start..end].iter().enumerate() {
        let i = start + i;
        // 选中行高亮仅在本面板聚焦时显示（lazygit 式：非聚焦面板不显示
        // 内部选中态，视觉焦点唯一）；任务选中不受影响（主列表恒定高亮）
        let sel = focused && i == app.conns_sel;
        // 选中行：深青背景高亮（同任务列表选中色）
        let bg = if sel { SEL_BG } else { Color::Reset };
        let up_on = c.up_speed > 0.0;
        let dl_on = c.speed > 0.0;
        let cell = |s: String, wd: usize, col: Color| {
            Span::styled(pad_left(&s, wd), Style::default().fg(col).bg(bg))
        };
        let spans = if is_bt {
            // 首列 = 对端 IP（掩码显示，左对齐）；选中行白字高亮
            vec![
                Span::styled(
                    pad_right(&c.ip_masked(), 11),
                    Style::default()
                        .fg(if sel { Color::White } else { FG })
                        .bg(bg),
                ),
                Span::styled(" ", Style::default().bg(bg)),
                // 下载=淡蓝（同头部 ↓/详情速度配色），待命显示 -
                cell(
                    if dl_on {
                        fmt_speed(c.speed)
                    } else {
                        "-".to_string()
                    },
                    8,
                    if dl_on { LIGHT_BLUE } else { DIM2 },
                ),
                Span::styled(" ", Style::default().bg(bg)),
                cell(fmt_size(c.cum_down), 8, FG),
                Span::styled(" ", Style::default().bg(bg)),
                // 上传=品红（同头部 ↑ 配色）；待命显示 -
                cell(
                    if up_on {
                        fmt_speed(c.up_speed)
                    } else {
                        "-".to_string()
                    },
                    8,
                    if up_on { MAGENTA } else { DIM2 },
                ),
                Span::styled(" ", Style::default().bg(bg)),
                cell(fmt_size(c.cum_up), 8, FG),
                Span::styled(" ", Style::default().bg(bg)),
                // m3 新增 客户端（末二列）：对端客户端两字母缩写（DIM，演示数据）
                cell(c.client.to_string(), 6, DIM),
                Span::styled(" ", Style::default().bg(bg)),
                // m3 新增 协议（末列）：TCP 直连 / uTP 微传输协议（DIM）
                cell(c.transport.label().to_string(), 4, DIM),
            ]
        } else {
            vec![
                cell(format!("{}", i + 1), 4, if sel { Color::White } else { FG }),
                Span::styled(" ", Style::default().bg(bg)),
                // 下载=淡蓝（同头部 ↓/详情速度配色），待命显示 -
                cell(
                    if dl_on {
                        fmt_speed(c.speed)
                    } else {
                        "-".to_string()
                    },
                    8,
                    if dl_on { LIGHT_BLUE } else { DIM2 },
                ),
                Span::styled(" ", Style::default().bg(bg)),
                cell(fmt_size(c.cum_down), 8, FG),
            ]
        };
        lines.push(Line::from(spans));
    }

    // 横向滚动偏移（钳制在渲染层兜底 + tick 事件内双防线）：
    // Paragraph.scroll 的水平分量按显示列跳过行首内容，表头随数据同步平移
    let hscroll = app.conns_hscroll.min(content_w.saturating_sub(inner.width as usize)) as u16;
    app.conns_max_hscroll = content_w.saturating_sub(inner.width as usize);
    app.visible_conns_rows = data_rows;
    f.render_widget(Paragraph::new(lines).scroll((0, hscroll)), inner);

    let thumb = if focused { FOCUS_BORDER } else { BORDER };
    // 通用滚动条·垂直（连接数超出一屏时；右边框列 ↑│║│↓）
    if conn_total > data_rows {
        let bar = draw_scrollbar(
            f,
            area,
            true,
            conn_total,
            data_rows,
            app.conns_scroll,
            thumb,
            app.sb_arrow_pressed(SbKey::ConnsV),
        );
        app.conns_v_sb = Some(bar);
    }
    // 通用滚动条·水平（m3 新增：BT 七列内容宽超出内框时；下边框行 ←─═══─→）
    if content_w > inner.width as usize {
        let bar = draw_scrollbar(
            f,
            area,
            false,
            content_w,
            inner.width as usize,
            app.conns_hscroll,
            thumb,
            app.sb_arrow_pressed(SbKey::ConnsH),
        );
        app.conns_h_sb = Some(bar);
    }
}

fn draw_footer(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // 双行页脚（v0.2 修订）：
    // 行 1 = 常用命令图例 chips：A 添加 / D 删除 / M 修改 / Q 退出 / H 帮助
    //   （鼠标左键可点击——按压反色高亮 = 点击效果、原地松开执行对应命令，
    //   与键盘快捷键同口径；完整交互说明仍收敛于 H 帮助窗口，P3 原则）；
    // 行 2 = toast 提示（◆ 黄色消息，§7.4）。
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    // 左右内边距（修订）：各留 1 列空白，与头部面板右侧留白同距（图区右缘
    // 到右边框 1 列）——内容不再顶格贴边框，行 1 chips 与行 2 toast 起止一致。
    const PAD: u16 = 1;
    let row_x = inner.x + PAD;
    let row_w = inner.width.saturating_sub(PAD * 2);
    let row1 = Rect {
        x: row_x,
        y: inner.y,
        width: row_w,
        height: 1,
    };
    let row2 = Rect {
        x: row_x,
        y: inner.y + 1,
        width: row_w,
        height: 1,
    };

    // ---- 行 1：常用命令 chips（每帧重建命中矩形，不含分隔符）----
    app.footer_cmd_rects.clear();
    let cmds = [
        ("A", "添加"),
        ("D", "删除"),
        ("M", "修改"),
        ("Q", "退出"),
        ("H", "帮助"),
    ];
    let mut spans: Vec<Span> = Vec::new();
    let mut x = row1.x;
    let right = row1.x.saturating_add(row1.width);
    for (i, (key, label)) in cmds.iter().enumerate() {
        // chip 显示宽 = 键 1 列 + 空格 1 列 + 标签（CJK 计 2）
        let chip_w = 2 + w(label) as u16;
        let sep_w = 3u16; // 分隔符 " · "
        let placed = spans.is_empty();
        let need = if placed { chip_w } else { sep_w + chip_w };
        if x.saturating_add(need) > right {
            break; // P7 小终端：放不下的 chip 整个不渲染（不越界、不截断）
        }
        if !placed {
            spans.push(Span::styled(" · ", Style::default().fg(DIM2)));
            x += sep_w;
        }
        // 点击效果：按压中或 400ms 余辉内整片反色高亮（同对话框按钮聚焦态色）
        let hot = app.footer_chip_hot(i);
        let (key_style, label_style) = if hot {
            (
                Style::default()
                    .fg(Color::Black)
                    .bg(ACCENT)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::Black).bg(ACCENT),
            )
        } else {
            (
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                Style::default().fg(FG),
            )
        };
        let key_span = Span::styled(format!("{} ", key), key_style);
        let label_span = Span::styled((*label).to_string(), label_style);
        app.footer_cmd_rects.push((
            Rect {
                x,
                y: row1.y,
                width: chip_w,
                height: 1,
            },
            i,
        ));
        x += chip_w;
        spans.push(key_span);
        spans.push(label_span);
    }
    f.render_widget(Paragraph::new(Line::from(spans)), row1);

    // ---- 行 2：toast 提示 ----
    if let Some(msg) = &app.toast {
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                format!("◆ {}", msg),
                Style::default().fg(YELLOW),
            )])),
            row2,
        );
    }
}

// ---------------------------------------------------------------------------
// 对话框
// ---------------------------------------------------------------------------

fn dialog_rect(area: Rect, dw: u16, dh: u16) -> Rect {
    let dw = dw.min(area.width);
    let dh = dh.min(area.height);
    Rect {
        x: area.x + (area.width.saturating_sub(dw)) / 2,
        y: area.y + (area.height.saturating_sub(dh)) / 2,
        width: dw,
        height: dh,
    }
}

/// 对话框定位：有记忆位置（拖拽过/首帧已回填）则用之（钳制在终端内），
/// 否则居中。同 pieces 弹窗口径——打开时居中并记忆，拖拽后位置保持。
fn dialog_rect_at(area: Rect, pos: Option<(u16, u16)>, dw: u16, dh: u16) -> Rect {
    match pos {
        Some((px, py)) => {
            let dw = dw.min(area.width);
            let dh = dh.min(area.height);
            Rect {
                x: px.min(area.width.saturating_sub(dw)),
                y: py.min(area.height.saturating_sub(dh)),
                width: dw,
                height: dh,
            }
        }
        None => dialog_rect(area, dw, dh),
    }
}

/// 浮层边界与 CJK 宽字符的切割处理（残影 bug 的根治点）。
///
/// 浮层（对话框/下拉）居中后，左/右边界列可能恰好切在底层 CJK 宽字符的
/// 半格上，使缓冲区进入自相矛盾的状态：
/// - 左边界外一格是宽字符主格（如「待」），其显示会延伸进浮层区域的右半格，
///   而浮层又在该半格上写了边框字符——同一显示行上「宽字 + 窄字」重叠；
/// - 右边界外一格残留孤立的宽字符半格（主格已被浮层改写为边框/空格）。
/// 两种矛盾下，无论增量 diff 还是清屏全量重绘，终端都可能把宽字渲染在
/// 浮层边框之上（残影），或使后续字符整体错位一格（宽字打印后游标推进
/// 两格，与缓冲区期望的格位不再对齐）。
/// 因此浮层清屏后立刻：把左边界外一格的宽字符主格截断为空格（该字本就
/// 有一半被浮层盖住，整字不显示是正确视觉），把右边界外一格的孤立半格
/// 补成空格——保证缓冲区内字符的显示占用永不与浮层内容跨界重叠。
fn clip_wide_at_edges(f: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let buf = f.buffer_mut();
    let x0 = area.x;
    let xr = area.right() - 1; // 浮层最右列
    for y in area.y..area.bottom() {
        if x0 > 0 {
            let i = buf.index_of(x0 - 1, y);
            // 宽字符主格（显示占两列、右半侵入浮层）→ 整格截断为空格
            if w(buf.content[i].symbol()) > 1 {
                buf.content[i].set_char(' ');
            }
        }
        if xr + 1 < buf.area.width {
            let i = buf.index_of(xr + 1, y);
            // 孤立半格（主格在浮层内、已被浮层改写）→ 补成空格
            if buf.content[i].symbol().is_empty() {
                buf.content[i].set_char(' ');
            }
        }
    }
}

fn button_spans(txt: &str, focused: bool) -> Vec<Span<'static>> {
    let label = format!("[ {} ]", txt);
    if focused {
        vec![Span::styled(
            format!(" {} ", label),
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        )]
    } else {
        vec![
            Span::styled(" ", Style::default()),
            Span::styled(label, Style::default().fg(FG)),
            Span::styled(" ", Style::default()),
        ]
    }
}

fn draw_dialogs(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(d) = app.dialog.as_ref() else { return };
    app.dlg_btn_rects.clear();
    app.dlg_field_rects.clear();
    app.dlg_ck_rects.clear();
    app.dlg_proxy_rects.clear();
    match d.kind {
        DialogKind::Add => draw_task_dialog(f, app, area, DialogKind::Add),
        DialogKind::Modify => draw_task_dialog(f, app, area, DialogKind::Modify),
        DialogKind::Delete => draw_delete_dialog(f, app, area),
    }
}

/// 字段行显示值：空值显示占位提示；超宽显示尾部（…+末尾字符，便于核对校验码结尾）
fn field_display(val: &str, ph: &str, avail: usize) -> String {
    if val.is_empty() {
        truncate(ph, avail)
    } else if w(val) > avail {
        let keep = avail.saturating_sub(1);
        format!(
            "…{}",
            val.chars()
                .skip(val.chars().count() - keep)
                .collect::<String>()
        )
    } else {
        val.to_string()
    }
}

/// 下拉浮层定位：优先展开在选择行下方；超出终端底部则改为行上方；
/// 横向钳制在终端内（左右各留 1 列边距）
fn dropdown_rect(area: Rect, inner: Rect, row_y: u16, pw: u16, ph: u16) -> Rect {
    let mut py = row_y + 1;
    if py + ph > area.y + area.height {
        py = row_y.saturating_sub(ph);
    }
    let px = (inner.x + 8)
        .min(area.x + area.width.saturating_sub(pw + 1))
        .max(area.x + 1);
    Rect {
        x: px,
        y: py.max(area.y),
        width: pw,
        height: ph,
    }
}

/// 任务对话框行种类（文本输入 / 校验算法下拉 / 代理下拉）
enum RowKind {
    Text,
    CkSel,
    ProxySel,
}

struct Row {
    label: &'static str,
    value: String,
    placeholder: String,
    kind: RowKind,
}

/// 添加/修改任务对话框统一渲染（v1.5/FR-01-86/87 同步）：字段行 + 按钮行
/// （需求修订：底部快捷键提示行已删除，高度相应缩短 1 行；完整说明见 H 帮助）
/// + 算法/代理下拉浮层。
///
/// * Add —— URL/保存到/并发/校验/校验码/代理（立即下载 6 · 仅添加 7 · 取消 8）
/// * Modify —— 并发/校验/校验码/代理（确定 4 取消 5）
fn draw_task_dialog(f: &mut Frame, app: &mut App, area: Rect, kind: DialogKind) {
    let is_add = kind == DialogKind::Add;
    let (title, dw, dh) = if is_add {
        (" 添加下载任务 ", 74u16, 11u16)
    } else {
        (" 修改任务 ", 74, 9)
    };
    // 可拖拽定位：记忆位置优先（钳制在终端内），首帧居中并回填位置/热区
    let dlg = dialog_rect_at(area, app.dialog_pos, dw, dh);
    if app.dialog_pos.is_none() {
        app.dialog_pos = Some((dlg.x, dlg.y));
    }
    app.dialog_area = Some(dlg);
    f.render_widget(Clear, dlg);
    clip_wide_at_edges(f, dlg);
    // 弹窗高亮边框与主面板聚焦色统一（绿+粗；一致性修复）
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(title, panel_title_style(true)));
    let inner = block.inner(dlg);
    f.render_widget(block, dlg);
    let min_h = if is_add { 9 } else { 7 };
    if inner.width < 24 || inner.height < min_h {
        return;
    }

    let (focus, url, dir, conns, ck_type, ck_value, ck_open, ck_sel, proxy_sel, proxy_open) = {
        let d = app.dialog.as_ref().unwrap();
        (
            d.focus,
            d.url.clone(),
            d.dir.clone(),
            d.conns.clone(),
            d.ck_type,
            d.ck_value.clone(),
            d.ck_open,
            d.ck_sel,
            d.proxy_sel,
            d.proxy_open,
        )
    };
    let (algo_name, algo_need) = CHECKSUM_ALGOS[ck_type.min(CHECKSUM_ALGOS.len() - 1)];
    let proxy_label = app
        .proxy_labels
        .get(proxy_sel)
        .cloned()
        .unwrap_or_else(|| "直连".to_string());

    // 字段行构建（Add 六行 / Modify 四行；校验与代理为下拉选择行）
    let mut rows: Vec<Row> = Vec::new();
    if is_add {
        rows.push(Row {
            label: "URL",
            value: url,
            placeholder: "https://example.com/file.zip".to_string(),
            kind: RowKind::Text,
        });
        rows.push(Row {
            label: "保存到",
            value: dir,
            placeholder: "/srv/downloads".to_string(),
            kind: RowKind::Text,
        });
    }
    rows.push(Row {
        label: "并发",
        value: conns,
        placeholder: "4".to_string(),
        kind: RowKind::Text,
    });
    rows.push(Row {
        label: "校验",
        value: String::new(),
        placeholder: String::new(),
        kind: RowKind::CkSel,
    });
    rows.push(Row {
        label: "校验码",
        value: ck_value,
        placeholder: format!("{} 位十六进制（可留空）", algo_need),
        kind: RowKind::Text,
    });
    rows.push(Row {
        label: "代理",
        value: String::new(),
        placeholder: String::new(),
        kind: RowKind::ProxySel,
    });

    let avail = (inner.width as usize).saturating_sub(12);
    let mut ck_row_y = inner.y;
    let mut proxy_row_y = inner.y;
    for (i, row) in rows.iter().enumerate() {
        let focused = focus == i;
        let row_y = inner.y + 1 + i as u16;
        match row.kind {
            RowKind::CkSel => ck_row_y = row_y,
            RowKind::ProxySel => proxy_row_y = row_y,
            RowKind::Text => {}
        }
        let mut spans = vec![
            Span::styled(
                pad_right(row.label, 7),
                Style::default().fg(if focused { ACCENT } else { DIM }),
            ),
            Span::styled("> ".to_string(), Style::default().fg(DIM2)),
        ];
        match row.kind {
            RowKind::CkSel => {
                // 下拉选择行：算法名黄色加粗 + ▾ 指示
                spans.push(Span::styled(
                    format!("{} ", algo_name),
                    Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    "▾",
                    Style::default().fg(if focused { YELLOW } else { DIM }),
                ));
                if focused {
                    spans.push(Span::styled("  Enter 选择算法", Style::default().fg(DIM2)));
                }
            }
            RowKind::ProxySel => {
                spans.push(Span::styled(
                    format!("{} ", proxy_label),
                    Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    "▾",
                    Style::default().fg(if focused { YELLOW } else { DIM }),
                ));
            }
            RowKind::Text => {
                spans.push(Span::styled(
                    field_display(&row.value, &row.placeholder, avail),
                    Style::default().fg(if row.value.is_empty() {
                        DIM2
                    } else {
                        Color::White
                    }),
                ));
                if focused {
                    spans.push(Span::styled(
                        "▏",
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                    ));
                }
            }
        }
        let rect = Rect {
            x: inner.x + 1,
            y: row_y,
            width: inner.width.saturating_sub(2),
            height: 1,
        };
        f.render_widget(Paragraph::new(Line::from(spans)), rect);
        // 回填字段行命中区域（鼠标点击聚焦；下拉行点击展开）
        app.dlg_field_rects.push((rect, i));
    }

    let n_rows = rows.len() as u16;
    // 按钮行（v1.15/FR-01-103 同步）：Add = [ 立即下载 ] [ 仅添加 ] [ 取消 ]
    //（焦点 6/7/8）；Modify = [ 确定 ] [ 取消 ]（焦点 4/5）
    let btn_row = inner.y + n_rows + 2;
    let buttons: Vec<(&str, usize)> = if is_add {
        vec![("立即下载", 6), ("仅添加", 7), ("取消", 8)]
    } else {
        vec![("确定", 4), ("取消", 5)]
    };
    let btn_ws: Vec<usize> = buttons
        .iter()
        .map(|(txt, _)| w(&format!("[ {} ]", txt)) + 2)
        .collect();
    let total_w: usize = btn_ws.iter().sum::<usize>() + 2 * (buttons.len() - 1);
    let mut bx = inner.x as usize + ((inner.width as usize).saturating_sub(total_w)) / 2;
    let inner_right = inner.x as usize + inner.width as usize;
    let buf_right = f.area().width as usize;
    for ((txt, idx), bw) in buttons.iter().zip(btn_ws.iter()) {
        let focused = focus == *idx;
        let line = Line::from(button_spans(txt, focused));
        // 右缘钳制（与生产 ui/btn.rs 同纪律）：矩形右缘不超过内框右缘与
        // 缓冲区右缘（越界截断 / 整只跳过），窄终端下三钮不溢出面板
        let width = (*bw)
            .min(inner_right.saturating_sub(bx))
            .min(buf_right.saturating_sub(bx));
        if width > 0 {
            let rect = Rect {
                x: bx as u16,
                y: btn_row,
                width: width as u16,
                height: 1,
            };
            f.render_widget(Paragraph::new(line), rect);
            app.dlg_btn_rects.push((rect, *idx));
        }
        bx += bw + 2;
    }

    // 需求修订：底部快捷键提示行已删除（原提示行位于 n_rows + 3；
    // 下拉框操作提示同步移除，交互说明统一收敛到 H 帮助窗口）

    // 校验算法下拉框（浮层最后绘制，覆盖对话框与下层内容）
    if ck_open {
        let items = CHECKSUM_ALGOS;
        let pw = 24u16;
        let ph = items.len() as u16 + 2;
        let prect = dropdown_rect(area, inner, ck_row_y, pw, ph);
        f.render_widget(Clear, prect);
        clip_wide_at_edges(f, prect);
        let pblock = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(panel_border_style(true))
            .title(Span::styled(" 校验算法 ", panel_title_style(true)));
        let pinner = pblock.inner(prect);
        f.render_widget(pblock, prect);
        for (i, (name, need)) in items.iter().enumerate() {
            let sel = i == ck_sel;
            let bg = if sel { Color::DarkGray } else { Color::Reset };
            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", if sel { "▸" } else { " " }),
                    Style::default().fg(if sel { ACCENT } else { DIM2 }).bg(bg),
                ),
                Span::styled(
                    pad_right(name, 9),
                    Style::default()
                        .fg(if sel { Color::White } else { FG })
                        .bg(bg)
                        .add_modifier(if sel {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::styled(
                    format!("{} 位", need),
                    Style::default().fg(if sel { ACCENT } else { DIM2 }).bg(bg),
                ),
            ]);
            let irect = Rect {
                x: pinner.x,
                y: pinner.y + i as u16,
                width: pinner.width,
                height: 1,
            };
            f.render_widget(Paragraph::new(line).style(Style::default().bg(bg)), irect);
            // 回填下拉选项命中区域（鼠标点击选择）
            app.dlg_ck_rects.push((irect, i));
        }
    }

    // 代理下拉框（v1.5/FR-01-86：直连 + 命名条目，显示名含类型标注，不含认证信息）
    if proxy_open {
        let items: Vec<String> = app.proxy_labels.clone();
        let pw = 26u16;
        let ph = items.len() as u16 + 2;
        let prect = dropdown_rect(area, inner, proxy_row_y, pw, ph);
        f.render_widget(Clear, prect);
        clip_wide_at_edges(f, prect);
        let pblock = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(panel_border_style(true))
            .title(Span::styled(" 选择代理 ", panel_title_style(true)));
        let pinner = pblock.inner(prect);
        f.render_widget(pblock, prect);
        for (i, name) in items.iter().enumerate() {
            let sel = i == proxy_sel;
            let bg = if sel { Color::DarkGray } else { Color::Reset };
            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", if sel { "▸" } else { " " }),
                    Style::default().fg(if sel { ACCENT } else { DIM2 }).bg(bg),
                ),
                Span::styled(
                    truncate(name, (pw as usize).saturating_sub(6)),
                    Style::default()
                        .fg(if sel { Color::White } else { FG })
                        .bg(bg)
                        .add_modifier(if sel {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
            ]);
            let irect = Rect {
                x: pinner.x,
                y: pinner.y + i as u16,
                width: pinner.width,
                height: 1,
            };
            f.render_widget(Paragraph::new(line).style(Style::default().bg(bg)), irect);
            // 回填代理选项命中区域（鼠标点击选择）
            app.dlg_proxy_rects.push((irect, i));
        }
    }
}

fn draw_delete_dialog(f: &mut Frame, app: &mut App, area: Rect) {
    // 可拖拽定位：记忆位置优先（钳制在终端内），首帧居中并回填位置/热区
    let dlg = dialog_rect_at(area, app.dialog_pos, 66, 7);
    if app.dialog_pos.is_none() {
        app.dialog_pos = Some((dlg.x, dlg.y));
    }
    app.dialog_area = Some(dlg);
    f.render_widget(Clear, dlg);
    clip_wide_at_edges(f, dlg);
    // 删除框边框同样统一为高亮绿（危险语义由正文警示文案/按钮色承载）
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" 删除任务 ", panel_title_style(true)));
    let inner = block.inner(dlg);
    f.render_widget(block, dlg);
    if inner.width < 40 || inner.height < 5 {
        return;
    }

    let (focus, task_name) = {
        let d = app.dialog.as_ref().unwrap();
        (d.focus, d.task_name.clone())
    };

    // 提示信息
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" 删除任务 ", Style::default().fg(DIM)),
            Span::styled(
                format!(
                    "「{}」",
                    truncate(&task_name, (inner.width as usize).saturating_sub(22))
                ),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ？", Style::default().fg(DIM)),
        ])),
        Rect {
            x: inner.x + 1,
            y: inner.y + 1,
            width: inner.width.saturating_sub(2),
            height: 1,
        },
    );

    // 按钮行：[ 仅删除任务 ] [ 删除任务和文件 ] [ 取消 ]
    let btn_row = inner.y + 3;
    let labels: [(&str, usize); 3] = [("仅删除任务", 0), ("删除任务和文件", 1), ("取消", 2)];
    let btn_ws: Vec<usize> = labels
        .iter()
        .map(|(txt, _)| w(&format!("[ {} ]", txt)) + 2)
        .collect();
    let total_w: usize = btn_ws.iter().sum::<usize>() + 2 * (labels.len() - 1);
    let mut bx = inner.x as usize + ((inner.width as usize).saturating_sub(total_w)) / 2;
    for ((txt, idx), bw) in labels.iter().zip(btn_ws.iter()) {
        let focused = focus == *idx;
        let line = Line::from(button_spans(txt, focused));
        let rect = Rect {
            x: bx as u16,
            y: btn_row,
            width: *bw as u16,
            height: 1,
        };
        f.render_widget(Paragraph::new(line), rect);
        app.dlg_btn_rects.push((rect, *idx));
        bx += bw + 2;
    }

    // 提示行
    f.render_widget(
        Paragraph::new(Span::styled(
            " 1/2/3 快捷选择 · Enter 确认 · Esc 取消",
            Style::default().fg(DIM2),
        )),
        Rect {
            x: inner.x + 1,
            y: inner.y + 4,
            width: inner.width.saturating_sub(2),
            height: 1,
        },
    );
}

// ---------------------------------------------------------------------------
// pieces 分块弹窗（P 快捷键；仅支持分块下载的任务）
// ---------------------------------------------------------------------------

/// piece 状态色块 span：同状态连续 n 块合并为一个 span（色块字符 emoji 自带
/// 语义色，fg 兜底供无彩色 emoji 字体的终端降级显示）
fn piece_span(st: PieceState, n: usize) -> Span<'static> {
    let (ch, color) = match st {
        PieceState::Done => ('🟩', GREEN),
        PieceState::Active => ('🟦', LIGHT_BLUE),
        PieceState::Partial => ('🟨', YELLOW),
        PieceState::Empty => ('⬜', EMPTY),
    };
    Span::styled(ch.to_string().repeat(n), Style::default().fg(color))
}

/// pieces 弹窗固定尺寸（宽×高，含边框；高度较初版缩短 2 行：
/// 底部快捷键提示行改为图例行，网格区域同步收紧）
const PIECES_W: u16 = 64;
const PIECES_H: u16 = 19;

/// pieces 分块弹窗（P）：展示选中任务分块（piece）状态一览。
///
/// 展示口径（色块字符 = 状态，见 app::PieceState）：
/// 🟩 已下载 · 🟦 正在下载（分配了并发下载，仅「下载中」任务出现）·
/// 🟨 部分下载（状态下沉冻结的持块连接内已落盘零头）· ⬜ 未下载；
/// 同一块「部分下载」且「正在下载」时只显示 🟦。
///
/// 布局：任务名 / 元信息（协议 · 块大小 · x/y 块 · 并发）/
/// 分块网格（每块一个宽 2 列的色块字符；块数超出可视范围时
/// 滚动翻看 + 右缘滚动条）/ 图例行（四色说明，需求修订：替代原快捷键提示行）。
/// 弹窗支持鼠标左键拖拽移动（按住弹窗内任意位置拖动，位置钳制在终端内；
/// 打开时居中，位置记忆至关闭）。打开时定位到下载前沿（第一块未完成块
/// 所在行滚动至网格垂直居中），此后滚动完全由用户控制，不自动跟随。
fn draw_pieces(f: &mut Frame, app: &mut App, area: Rect) {
    // 先取快照（owned）再回填 app（滚动定位/热区），避免借用冲突
    let (name, proto, conc, x, y, states) = {
        let snap = app.sel_task().map(|t| {
            let (x, y, _piece) = t.chunk_info();
            (t.name.clone(), t.protocol, t.concurrency, x, y, t.pieces())
        });
        let Some(snap) = snap else {
            app.close_pieces();
            return;
        };
        snap
    };
    let y = y as usize;

    // 弹窗尺寸 64×19（内框 62×17：名称/元信息/图例各 1 行 + 网格 14 行；
    // 每行 31 块 → 一屏 434 块）；位置优先取记忆的拖拽位置（钳制在终端内），
    // 打开首帧居中并记忆；小终端钳制尺寸，网格行数随内框收缩
    let rect = match app.pieces_pos {
        Some((px, py)) => {
            let dw = PIECES_W.min(area.width);
            let dh = PIECES_H.min(area.height);
            Rect {
                x: px.min(area.width.saturating_sub(dw)),
                y: py.min(area.height.saturating_sub(dh)),
                width: dw,
                height: dh,
            }
        }
        None => {
            let r = dialog_rect(area, PIECES_W, PIECES_H);
            app.pieces_pos = Some((r.x, r.y));
            r
        }
    };
    f.render_widget(Clear, rect);
    // 浮层边界 CJK 宽字符切割处理（残影根治点，与对话框同口径）
    clip_wide_at_edges(f, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        // 模态弹窗恒定高亮边框（与主面板聚焦色统一的绿+粗；
        // 与底层面板降级暗边框形成对比）
        .border_style(panel_border_style(true))
        .title(Span::styled(" 分块状态 ", panel_title_style(true)));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    app.pieces_area = Some(rect);

    let cols = (inner.width as usize / 2).max(1); // 每块占 2 列
    let rows = Layout::vertical([
        Constraint::Length(1), // 任务名
        Constraint::Length(1), // 元信息
        Constraint::Min(1),    // 分块网格
        Constraint::Length(1), // 图例行（四色说明；需求修订：替代快捷键提示）
    ])
    .split(inner);
    let grid = rows[2];
    let grid_h = grid.height as usize;

    let total_rows = y.div_ceil(cols);
    // 打开首帧：以第一块未完成块所在行为中心定位（下载前沿可见：
    // 上方 🟩、前沿 🟦/🟨、下方 ⬜，一眼读出整体进度形态）
    if app.pieces_follow {
        app.pieces_follow = false;
        let frontier = x as usize / cols;
        app.pieces_scroll = frontier.saturating_sub(grid_h / 2);
    }
    app.pieces_scroll = app
        .pieces_scroll
        .min(total_rows.saturating_sub(grid_h));
    let start_row = app.pieces_scroll;

    // 行 1：任务名（超宽截尾）
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {}", truncate(&name, inner.width as usize)),
            Style::default().fg(FG).add_modifier(Modifier::BOLD),
        ))),
        rows[0],
    );

    // 行 2：元信息 = 协议 · 块大小 · x/y 块（x=已下载块数，与详情面板分块行
    // 同口径）· 并发上限
    let meta = Span::styled(
        format!(
            " {} · 块大小 {} · {}/{} 块 · 并发 {}",
            proto.label(),
            chunk_size_label(proto),
            x,
            y,
            conc
        ),
        Style::default().fg(DIM),
    );
    f.render_widget(Paragraph::new(Line::from(vec![meta])), rows[1]);

    // 行 3：分块网格（仅构建可视行；同状态连续块合并为单 span）
    let mut lines: Vec<Line> = Vec::with_capacity(grid_h);
    for r in 0..grid_h {
        let s = (start_row + r) * cols;
        if s >= y {
            break;
        }
        let e = (s + cols).min(y);
        let mut spans: Vec<Span> = Vec::new();
        let mut run = states[s];
        let mut run_at = s;
        for (off, st) in states[s..e].iter().enumerate() {
            let i = s + off;
            if *st != run {
                spans.push(piece_span(run, i - run_at));
                run = *st;
                run_at = i;
            }
        }
        spans.push(piece_span(run, e - run_at));
        lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(lines), grid);

    // 行 4：图例（需求修订：底部分块状态说明，替代原快捷键提示行——
    // 色块字符与网格同源，字色兜底同 piece_span）
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled("🟩", Style::default().fg(GREEN)),
            Span::styled(" 已下载   ", Style::default().fg(FG)),
            Span::styled("🟦", Style::default().fg(LIGHT_BLUE)),
            Span::styled(" 正在下载   ", Style::default().fg(FG)),
            Span::styled("🟨", Style::default().fg(YELLOW)),
            Span::styled(" 部分下载   ", Style::default().fg(FG)),
            Span::styled("⬜", Style::default().fg(EMPTY)),
            Span::styled(" 未下载", Style::default().fg(FG)),
        ])),
        rows[3],
    );

    // 通用滚动条·垂直（块数超出一屏时；↑↓ 箭头 + │轨道 + ║滑块，画在弹窗
    // 右边框列，不与网格内容重叠）。几何回填给 App：滑块拖拽 = 滚动网格
    // （修复：此前拖拽按窗口移动处理）、箭头点击/按住连发、轨道翻页（§4.1.3）
    if total_rows > grid_h {
        let bar = draw_scrollbar(
            f,
            rect,
            true,
            total_rows,
            grid_h,
            start_row,
            FOCUS_BORDER,
            app.sb_arrow_pressed(SbKey::Pieces),
        );
        app.pieces_sb = Some(bar);
    } else {
        app.pieces_sb = None;
    }
    app.pieces_max_scroll = total_rows.saturating_sub(grid_h);
}

// ---------------------------------------------------------------------------
// 帮助窗口（H 快捷键）
// ---------------------------------------------------------------------------

/// 帮助窗口固定尺寸（宽×高，含边框）
const HELP_W: u16 = 70;
const HELP_H: u16 = 24;

/// 分类帮助内容（需求修订：按 panel / 窗口分组的交互说明）。
/// 每帧重建（行数小、无状态）；键名列用 CJK 感知宽度对齐。
fn help_lines() -> Vec<Line<'static>> {
    /// 分组标题
    fn sec(t: &str) -> Line<'static> {
        Line::from(Span::styled(
            format!(" 【{}】", t),
            Style::default()
                .fg(FOCUS_BORDER)
                .add_modifier(Modifier::BOLD),
        ))
    }
    /// 键位 + 说明（键列 16 显示宽，CJK 感知对齐）
    fn kd(k: &str, d: &str) -> Line<'static> {
        Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled(
                pad_right(k, 16),
                Style::default()
                    .fg(ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(d.to_string(), Style::default().fg(FG)),
        ])
    }
    /// 补充说明（暗色）
    fn note(s: &str) -> Line<'static> {
        Line::from(Span::styled(s.to_string(), Style::default().fg(DIM)))
    }
    vec![
        sec("全局"),
        kd("Tab / BackTab", "切换面板（任务列表 → 详情 → 并发连接）"),
        kd("鼠标左键", "点击面板聚焦；点击任务/连接选中明细"),
        kd("页脚命令", "A/D/M/Q/H 图例可点击：按压高亮，松开执行"),
        kd("H", "打开 / 关闭本帮助窗口"),
        kd("G", "收起/展开 任务详情 + 并发连接面板"),
        kd("Esc", "关闭弹窗"),
        kd("Q", "退出程序（主界面；弹窗内先关窗）"),
        sec("页签"),
        kd("] / [", "右/左切换页签（正在下载 ⇄ 已完成）"),
        kd("鼠标点击页签", "切换到该页签"),
        kd("T", "循环切换页签（与 ] 等效）"),
        sec("任务列表"),
        kd("↑ / ↓", "选择上/下一个任务"),
        kd("PgUp / PgDn", "翻页选择（Home / End 跳首/尾）"),
        kd("鼠标滚轮", "滚动列表（视口自由浏览全表）"),
        kd("A / M / D", "添加 / 修改 / 删除任务"),
        kd("Space / R", "暂停·继续 / 重试"),
        kd("U / J", "上移 / 下移任务"),
        kd("C", "清除已完成任务"),
        kd("P", "查看分块状态（仅分块下载任务）"),
        sec("任务详情"),
        kd("鼠标点击", "URL / 校验 字段名复制对应值"),
        sec("并发连接"),
        kd("↑ / ↓", "选择连接明细（视口跟随滚动）"),
        kd("← / →", "横向滚动明细列（客户端/协议等超宽列）"),
        kd("鼠标滚轮", "悬停并发面板 = 滚动连接明细（±2/格）"),
        kd("B", "阻塞并移除选中连接（仅 BT 任务）"),
        sec("滚动条（通用组件）"),
        kd("拖拽滑块 ═║", "按住双线滑块拖动 = 比例滚动内容（浮层内同样滚内容）"),
        kd("点击 ↑↓←→", "步进滚动 1 格；按住不放自动连发"),
        kd("点击轨道", "朝点击方向整页滚动（可按住连发）"),
        kd("方向键", "面板聚焦时 ↑↓ 选择/纵向滚动、←→ 横向滚动（并发）"),
        sec("弹窗（添加/修改/删除/pieces/帮助）"),
        kd("Tab / ↑↓", "切换字段/焦点（下拉框内选择选项）"),
        kd("Enter", "确认"),
        kd("Esc", "取消/关闭"),
        kd("鼠标拖拽", "按住弹窗内空白处拖动移动窗口（滑块/边框除外）"),
        sec("pieces 分块弹窗"),
        kd("↑↓ / PgUp·PgDn", "滚动分块网格（Home / End 跳首/尾）"),
        kd("Esc / P", "关闭弹窗"),
        note(""),
        note(" ↑↓/滚轮 滚动本页 · Esc/H 关闭"),
    ]
}

/// 帮助窗口（H）：模态浮层——打开时底层面板降级暗框；绿色高亮边框（与
/// 主面板聚焦色统一）；分类交互说明；内容超窗时 ↑↓/PgUp/PgDn/滚轮滚动 +
/// 右缘 lazygit 式滚动条；支持鼠标拖拽移动（位置记忆）、点击外部关闭。
fn draw_help(f: &mut Frame, app: &mut App, area: Rect) {
    // 可拖拽定位：记忆位置优先（钳制在终端内），首帧居中并回填
    let rect = match app.help_pos {
        Some((px, py)) => {
            let dw = HELP_W.min(area.width);
            let dh = HELP_H.min(area.height);
            Rect {
                x: px.min(area.width.saturating_sub(dw)),
                y: py.min(area.height.saturating_sub(dh)),
                width: dw,
                height: dh,
            }
        }
        None => {
            let r = dialog_rect(area, HELP_W, HELP_H);
            app.help_pos = Some((r.x, r.y));
            r
        }
    };
    f.render_widget(Clear, rect);
    // 浮层边界 CJK 宽字符切割处理（残影根治点，与对话框同口径）
    clip_wide_at_edges(f, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" 帮助 ", panel_title_style(true)));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    app.help_area = Some(rect);

    let lines = help_lines();
    let view_h = inner.height as usize;
    let max_scroll = lines.len().saturating_sub(view_h);
    app.help_scroll = app.help_scroll.min(max_scroll);
    let start = app.help_scroll;
    let shown: Vec<Line> = lines.iter().skip(start).take(view_h).cloned().collect();
    f.render_widget(Paragraph::new(shown), inner);

    // 通用滚动条·垂直（内容超窗时；↑↓ 箭头 + │轨道 + ║滑块；滑块拖拽 =
    // 滚动内容、箭头点击/按住连发、轨道翻页——与主界面面板同一组件同一行为）
    if lines.len() > view_h && view_h > 0 {
        let bar = draw_scrollbar(
            f,
            rect,
            true,
            lines.len(),
            view_h,
            start,
            FOCUS_BORDER,
            app.sb_arrow_pressed(SbKey::Help),
        );
        app.help_sb = Some(bar);
    } else {
        app.help_sb = None;
    }
    app.help_max_scroll = lines.len().saturating_sub(view_h.max(1));
}

// ---------------------------------------------------------------------------
// 总入口
// ---------------------------------------------------------------------------

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let outer = Layout::vertical([
        Constraint::Length(5), // 顶部区：头部仪表面板（标题边框 + 字段/分割线/页签 3 内容行 + 底边框）
        Constraint::Min(8),   // 主区：任务列表 | 任务详情 + 并发连接面板
        Constraint::Length(4), // 页脚（双行：常用命令图例 chips + toast）
    ])
    .split(area);

    let wide = area.width >= 100;
    // 焦点钳制：详情/并发面板未渲染（紧凑布局 G / 窄终端 <100 列）时
    // 焦点只允许落在任务列表（与 G 键处理同口径，双保险）
    if !wide || !app.show_panes {
        app.focus = Panel::List;
    }
    if wide {
        // 头部仪表面板：通栏全宽（右缘 = 屏幕右缘 = 任务详情面板右缘）；字段行 +
        // 分割线 + 页签行，右侧内嵌无边框像素级流量图（占内容区全高、20% 宽）。
        // 修订-1：流量图始终显示，不再随 G 键隐藏
        draw_header(f, app, outer[0], true);

        if app.show_panes {
            // 主区：左列 58% = 任务列表（「任务队列」面板已并入头部，列表自头部正下方
            // 起、占满左列全高）；右列 42% = 任务详情（10 行内容 + 边框 = 12 行；
            // v1.20 生产同步：任务 ID 与创建时间拆分两行）+
            // 并发连接面板（占余下全部高度）
            let cols = Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
                .split(outer[1]);
            draw_list(f, app, cols[0]);
            let right =
                Layout::vertical([Constraint::Length(12), Constraint::Min(6)]).split(cols[1]);
            draw_detail(f, app, right[0]);
            draw_conns(f, app, right[1]);
        } else {
            // G 键紧凑布局：流量图保留在头部，仅收起任务详情 / 并发连接面板，
            // 任务列表占满整行
            app.conns_area = None;
            app.detail_url_rect = None;
            app.detail_ck_rect = None;
            draw_list(f, app, outer[1]);
        }
    } else {
        // 窄终端（< 100 列）：头部面板无图（分割线与页签行贯通全宽），任务列表占满整行
        app.conns_area = None;
        app.detail_url_rect = None;
        app.detail_ck_rect = None;
        draw_header(f, app, outer[0], false);
        draw_list(f, app, outer[1]);
    }

    draw_footer(f, app, outer[2]);

    // 对话框最后绘制（覆盖层）
    if app.dialog.is_some() {
        draw_dialogs(f, app, area);
    }

    // pieces 分块弹窗（最上层覆盖；与对话框互斥——按键层已保证）
    if app.pieces_open {
        draw_pieces(f, app, area);
    }

    // 帮助窗口（最上层；H 打开，模态：打开时底层面板暗框）
    if app.help_open {
        draw_help(f, app, area);
    }
}
