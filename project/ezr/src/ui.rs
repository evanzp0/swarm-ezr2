//! ui.rs — EZR Downloader TUI Demo 的界面渲染
//!
//! 布局：头部(全局统计) / 页签(正在下载|已完成) / 主区(任务列表 + 详情与速度图) / 页脚(快捷键与提示)
//! 每个任务条目 3 行：标题行(名称+协议/状态徽标+百分比)、整体进度条(不分块)、统计行。
//! 协议徽标 [HTTP/HTTPS/BT] 统一黄色；详情页分块以文字显示 x/y（已完成/总块数）与块大小 N/块。
//! 分块策略：块大小按协议写死（HTTP 1 MB / BT 256 KB），块数与并发数解耦。
//! 对话框：添加任务(URL+目录+并发+校验算法下拉+校验码) / 删除任务(仅任务|任务和文件|取消)。
#![allow(missing_docs)] // 交互层：demo 定稿基线复用，接口文档见 model/engine 层
#![allow(clippy::pedantic)] // 交互层字节/速度展示算术与 demo 基线风格豁免
#![allow(clippy::nursery)] // 同上
#![allow(clippy::cognitive_complexity, clippy::too_many_lines, clippy::too_many_arguments)]
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_wrap)]


use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Sparkline, Tabs},
    Frame,
};

use crate::app::{App, DialogKind, CHECKSUM_ALGOS, FILTERS, ITEM_HEIGHT};
use crate::model::chunk::fmt_block_size;
use crate::VERSION;
use crate::model::{FailKind, Task, TaskState};

// ---------------------------------------------------------------------------
// 调色板（语义状态色；下载中=淡蓝，暂停=白，等待=黄，失败=红，完成=绿，做种=粉）
// ---------------------------------------------------------------------------

const BORDER: Color = Color::Rgb(56, 78, 86);
const FG: Color = Color::Rgb(198, 208, 212);
const DIM: Color = Color::Rgb(126, 141, 147);
const DIM2: Color = Color::Rgb(88, 102, 108);
const SEL_BG: Color = Color::Rgb(22, 42, 48);
const ACCENT: Color = Color::Cyan;
const GREEN: Color = Color::Green;
const YELLOW: Color = Color::Rgb(228, 178, 62);
const RED: Color = Color::Rgb(226, 92, 84);
const MAGENTA: Color = Color::Rgb(214, 112, 214);
const EMPTY: Color = Color::Rgb(54, 68, 74);
/// 下载中/校验中/后期处理（淡蓝）
const LIGHT_BLUE: Color = Color::Rgb(122, 185, 242);

fn state_color(s: TaskState) -> Color {
    match s {
        TaskState::Queued => YELLOW,
        TaskState::Downloading | TaskState::Verifying | TaskState::PostProcessing => LIGHT_BLUE,
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

/// 按终端显示宽度左填充空格
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
    let bps = if bps == 0.0 { 0.0 } else { bps }; // 归一化负零（-0.0 == 0.0），避免「-0 B/s」显示
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

fn task_lines(t: &Task, sel: bool, spinner: char, width: usize, queue_pos: usize) -> Vec<Line<'static>> {
    let width = width.max(20);
    let state_c = state_color(t.state);

    // ---- 行 1：光标 + 名称 + 徽标 + 右对齐百分比 ----
    // 等待中任务显示真实进度（可能持有断点进度）
    let pct_txt = format!("{:>5.1}%", t.progress() * 100.0);
    let proto_badge = format!("[{}]", t.protocol.label());
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
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
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
                // 磁盘空间不足 / 校验失败直接停等，FR-M1-43/44/51）。
                // 瞬态失败未达上限却停等 = auto_retry=false 配置（Gherkin
                // 01-retry-backoff-10：显示「不自动重试」而非「已达上限」）
                None => match t.fail_kind {
                    Some(FailKind::Fatal) | Some(FailKind::Verify) => {
                        "不自动重试".to_string()
                    }
                    _ if t.retries >= t.max_retries => "已达上限".to_string(),
                    _ => "不自动重试".to_string(),
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
            if t.total == 0 {
                // 等待任务不预取（规格 01-download-engine-15）：探测前大小显示「未知」
                l3.push(Span::styled("未知".to_string(), Style::default().fg(DIM)));
            } else {
                l3.push(Span::styled(
                    format!("{}/{}", fmt_size(t.downloaded), fmt_size(t.total)),
                    Style::default().fg(FG),
                ));
            }
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

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
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
    let threads: usize = app
        .tasks
        .iter()
        .filter(|t| t.state == TaskState::Downloading)
        .map(|t| t.thread_count())
        .sum();
    let doing = app.tasks.iter().filter(|t| !t.state.is_done()).count();
    let done = app.tasks.len() - doing;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(Line::from(vec![
            Span::styled(" ◆ ", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
            Span::styled(
                "EZR Downloader",
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  v{VERSION}"), Style::default().fg(DIM)),
        ]));

    let line = Line::from(vec![
        Span::styled(" 全局  ", Style::default().fg(DIM)),
        Span::styled(
            format!("↓ {}", fmt_speed(dl)),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("   ↑ {}", fmt_speed(ul)),
            Style::default().fg(MAGENTA),
        ),
        Span::styled(
            format!("   并发线程 {} ", threads),
            Style::default().fg(FG),
        ),
        Span::styled(
            format!("  会话已下载 {}", fmt_size(app.session_bytes)),
            Style::default().fg(FG),
        ),
        Span::styled(
            format!("  任务 {} · 正在下载 {} · 已完成 {}", app.tasks.len(), doing, done),
            Style::default().fg(DIM),
        ),
    ]);
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(Paragraph::new(line), inner);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let doing = app.tasks.iter().filter(|t| !t.state.is_done()).count();
    let done = app.tasks.len() - doing;
    let counts = [doing, done];
    let titles: Vec<Line> = FILTERS
        .iter()
        .zip(counts.iter())
        .map(|(label, c)| Line::from(format!(" {} ({}) ", label, c)))
        .collect();

    let tabs = Tabs::new(titles)
        .select(app.filter)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::styled("│", Style::default().fg(DIM2)))
        .style(Style::default().fg(FG));

    let slots = app.used_slots();
    let slots_full = slots >= app.max_slots;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(Span::styled(
            " 任务队列 ",
            Style::default().fg(BORDER),
        ))
        // 右侧：下载槽位占用（已满时黄色提醒：等待任务需排队）
        .title(
            Line::from(Span::styled(
                format!("下载槽位 {}/{} ", slots, app.max_slots),
                Style::default()
                    .fg(if slots_full { YELLOW } else { DIM })
                    .add_modifier(if slots_full {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ))
            .alignment(Alignment::Right),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(tabs, inner);
}

fn draw_list(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(Line::from(vec![
            Span::styled(" 下载任务 ", Style::default().fg(ACCENT)),
            Span::styled("（鼠标: 点击选中 / 滚轮滚动） ", Style::default().fg(DIM2)),
        ]))
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
    let rows = inner.height as usize / ITEM_HEIGHT as usize;

    if rows == 0 || idxs.is_empty() {
        let msg = if idxs.is_empty() {
            // 空态提示（Gherkin 01-tui-display-13：显示「按 A 添加下载任务」）
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
        let row_area = Rect {
            x: inner.x,
            y: inner.y + (vi * ITEM_HEIGHT as usize) as u16,
            width: inner.width,
            height: ITEM_HEIGHT,
        };
        render_task(f, t, row_area, ti == app.selected, spinner, qpos[ti]);
    }

    app.visible_rows = max_items;
    app.list_area = Some(inner);
}

fn draw_detail(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(Span::styled(
            " 任务详情 ",
            Style::default().fg(ACCENT),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(t) = app.sel_task() else {
        f.render_widget(
            Paragraph::new(Span::styled("（未选中任务）", Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    };

    let label = |s: &str| Span::styled(pad_right(s, 9), Style::default().fg(DIM));
    let val = |s: String| Span::styled(s, Style::default().fg(FG));
    let state_c = state_color(t.state);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            truncate(&t.name, (inner.width as usize).saturating_sub(4)),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("状态"),
        Span::styled(
            format!("[{}] {:.1}%", t.state.label(), t.progress() * 100.0),
            Style::default().fg(state_c).add_modifier(Modifier::BOLD),
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
                app.max_slots
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
        label("ID"),
        Span::styled(
            format!("{id} · 添加于 {created}", id = t.id, created = t.created),
            Style::default().fg(DIM),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("类型"),
        Span::styled(
            t.protocol.label().to_string(),
            Style::default()
                .fg(YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        // 并发数仅在「下载中」与「做种中」有数值，其他状态显示 -
        {
            let conn_txt = match t.state {
                TaskState::Downloading | TaskState::Seeding => t.thread_count().to_string(),
                _ => "-".to_string(),
            };
            let resume_txt = if t.resumable {
                "支持断点续传"
            } else {
                "不支持断点续传"
            };
            val(format!(" · {} 并发 · {}", conn_txt, resume_txt))
        },
    ]));
    // 校验行（提供了校验码的任务）：算法 · 校验码前缀 + 状态
    // 前缀取 10 位、状态用短文案，保证 110 列窄面板也能完整显示
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
        lines.push(Line::from(vec![
            Span::raw(" "),
            label("校验"),
            Span::styled(
                ck.algo.to_string(),
                Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" · {}", vshort), Style::default().fg(FG)),
            Span::styled(st, Style::default().fg(stc)),
        ]));
    }
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("大小"),
        val(format!(
            "{} / {}（剩余 {}）",
            fmt_size(t.downloaded),
            fmt_size(t.total),
            fmt_size(t.total.saturating_sub(t.downloaded))
        )),
    ]));
    // 失败原因（仅已失败）
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
    // 速度行（无平均值；缺失用 -）
    let speed_line: Vec<Span<'static>> = match t.state {
        TaskState::Downloading => {
            let mut v = vec![
                Span::raw(" "),
                label("速度"),
                Span::styled(
                    format!("↓ {}", fmt_speed(t.speed)),
                    Style::default().fg(ACCENT),
                ),
            ];
            if t.protocol.is_bt() {
                v.push(Span::styled(
                    format!("  ↑ {}", fmt_speed(t.upload_speed)),
                    Style::default().fg(MAGENTA),
                ));
            }
            v.push(Span::styled(
                format!("  剩余 {}", fmt_eta(t.eta_secs())),
                Style::default().fg(DIM),
            ));
            v
        }
        TaskState::Seeding => {
            vec![
                Span::raw(" "),
                label("速度"),
                Span::styled(
                    format!("↑ {}", fmt_speed(t.upload_speed)),
                    Style::default().fg(MAGENTA),
                ),
                Span::styled(
                    format!("  剩余做种 {}", fmt_dur(t.seed_left.ceil() as u64)),
                    Style::default().fg(DIM),
                ),
            ]
        }
        TaskState::Completed => {
            vec![
                Span::raw(" "),
                label("速度"),
                Span::styled(
                    format!("—  总用时 {}", fmt_dur(t.elapsed as u64)),
                    Style::default().fg(DIM),
                ),
            ]
        }
        _ => {
            let mut v = vec![Span::raw(" "), label("速度"), Span::raw("↓ ")];
            v.push(Span::styled("-", Style::default().fg(DIM)));
            if t.protocol.is_bt() {
                v.push(Span::raw("  ↑ "));
                v.push(Span::styled("-", Style::default().fg(DIM)));
            }
            v.push(Span::raw("  剩余 "));
            v.push(Span::styled("-", Style::default().fg(DIM)));
            v
        }
    };
    lines.push(Line::from(speed_line));
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("保存"),
        val(truncate(&t.target_path(), (inner.width as usize).saturating_sub(12))),
    ]));
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("URL"),
        val(truncate(
            // 重定向后展示最终 URL（FR-01-14 / 规格 01-download-engine-08「详情 URL 显示最终 URL」）
            t.final_url.as_deref().unwrap_or(&t.url),
            (inner.width as usize).saturating_sub(12),
        )),
    ]));

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
                Style::default()
                    .fg(state_c)
                    .add_modifier(Modifier::BOLD),
            ));
            l.push(Span::styled(
                format!(" · {}/块", fmt_block_size(t.block_size)),
                Style::default().fg(FG),
            ));
        }
        lines.push(Line::from(l));
    }

    if inner.height as usize > lines.len() + 2 {
        lines.push(Line::from(Span::styled(
            " ────────── 并发分块明细 ──────────",
            Style::default().fg(DIM2),
        )));
        // 表头（紧凑列宽，适配窄面板；CJK 按显示宽度填充）
        lines.push(Line::from(vec![
            Span::styled(
                format!(
                    "{}{}{}{}  {}",
                    pad_right("#", 4),
                    pad_right("当前分块", 14),
                    pad_left("进度", 7),
                    pad_left("速度", 11),
                    "状态"
                ),
                Style::default().fg(DIM),
            ),
        ]));
        let rows_avail = (inner.height as usize).saturating_sub(lines.len() + 1);
        for (i, c) in t.connections.iter().enumerate() {
            if i >= rows_avail.max(1) {
                break;
            }
            let idle = c.cap() == 0;
            let frac = if idle { 0.0 } else { c.frac() };
            let done = !idle && frac >= 1.0;
            let st = if idle {
                Span::styled("待命", Style::default().fg(DIM2))
            } else if done {
                Span::styled("完成", Style::default().fg(GREEN))
            } else if c.speed > 0.0 {
                Span::styled("传输中", Style::default().fg(ACCENT))
            } else {
                Span::styled("挂起", Style::default().fg(DIM))
            };
            // 分块列：块号「块 k/y」（分块队列按块大小顺序领块），待命显示 —
            let chunk_col = if idle {
                "—".to_string()
            } else {
                let (_, yy, nn) = t.chunk_info();
                if nn != 0 {
                    format!("块 {}/{}", c.start.checked_div(nn).unwrap_or(0) + 1, yy)
                } else {
                    "—".to_string()
                }
            };
            lines.push(Line::from(vec![
                Span::styled(
                    pad_right(&format!("#{}", c.id), 4),
                    Style::default().fg(FG),
                ),
                Span::styled(
                    pad_right(&chunk_col, 14),
                    Style::default().fg(if idle { DIM2 } else { FG }),
                ),
                Span::styled(
                    if idle {
                        pad_left("—", 7)
                    } else {
                        pad_left(&format!("{:.1}%", frac * 100.0), 7)
                    },
                    Style::default().fg(if done { GREEN } else if idle { DIM2 } else { FG }),
                ),
                Span::styled(
                    format!(
                        "{}  ",
                        pad_left(
                            &if c.speed > 0.0 {
                                fmt_speed(c.speed)
                            } else {
                                "—".to_string()
                            },
                            11
                        )
                    ),
                    Style::default().fg(if done { DIM } else { ACCENT }),
                ),
                st,
            ]));
        }
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_chart(f: &mut Frame, app: &App, area: Rect) {
    let dl: u64 = (app
        .tasks
        .iter()
        .filter(|t| t.state == TaskState::Downloading)
        .map(|t| t.speed)
        .sum::<f64>()
        / 1024.0) as u64;
    let peak = app.speed_hist.iter().copied().max().unwrap_or(0);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(Line::from(vec![
            Span::styled(" ↓ 全局速度 ", Style::default().fg(ACCENT)),
            Span::styled(
                fmt_speed(dl as f64 * 1024.0),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .title(
            Line::from(Span::styled(
                format!(" 峰值 {} ", fmt_speed(peak as f64 * 1024.0)),
                Style::default().fg(DIM),
            ))
            .alignment(Alignment::Right),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);

    let width = (inner.width as usize).max(1);
    let start = app.speed_hist.len().saturating_sub(width);
    let data: Vec<u64> = app.speed_hist[start..].to_vec();
    f.render_widget(
        Sparkline::default()
            .data(&data)
            .style(Style::default().fg(ACCENT)),
        inner,
    );
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(inner);

    // 快捷键行
    let key = |s: &str| Span::styled(s.to_string(), Style::default().fg(ACCENT).add_modifier(Modifier::BOLD));
    let desc = |s: &str| Span::styled(s.to_string(), Style::default().fg(DIM));
    let mut l1 = vec![
        key(" ↑↓"), desc(" 选择  "),
        key("Space"), desc(" 暂停/继续  "),
        key("R"), desc(" 重试  "),
        key("A"), desc(" 添加  "),
        key("D"), desc(" 删除  "),
        key("U/J"), desc(" 上移/下移  "),
        key("C"), desc(" 清已完成  "),
        key("Tab"), desc(" 页签  "),
        key("G"), desc(" 图表  "),
        key("Q"), desc(" 退出"),
    ];
    // 窄屏精简
    if inner.width < 100 {
        l1.truncate(9);
    }
    if inner.width < 86 {
        l1.truncate(7);
    }
    if inner.width < 72 {
        l1.truncate(5);
    }
    f.render_widget(Paragraph::new(Line::from(l1)), rows[0]);

    // 鼠标提示 + toast
    let mut l2 = vec![
        Span::styled(" 鼠标 ", Style::default().fg(DIM)),
        Span::styled("点击选中 · 滚轮滚动   ", Style::default().fg(DIM)),
    ];
    if let Some(msg) = &app.toast {
        l2.push(Span::styled(
            format!("◆ {}", msg),
            Style::default().fg(YELLOW),
        ));
    }
    f.render_widget(
        Paragraph::new(Line::from(l2)),
        rows[1],
    );
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

/// 浮层边界与 CJK 宽字符的切割处理（残影 bug 的根治点）。
///
/// 浮层（对话框/下拉）居中后，左/右边界列可能恰好切在底层 CJK 宽字符的
/// 半格上，使缓冲区进入自相矛盾的状态：
/// - 左边界外一格是宽字符主格（如「待」），其显示会延伸进浮层区域的右半格，
///   而浮层又在该半格上写了边框字符——同一显示行上「宽字 + 窄字」重叠；
/// - 右边界外一格残留孤立的宽字符半格（主格已被浮层改写为边框/空格）。
///
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

/// 居中绘制对话框按钮行并回填命中区域（添加/删除对话框共用）
fn draw_button_row(
    f: &mut Frame,
    app: &mut App,
    inner: Rect,
    btn_row: u16,
    labels: &[(&str, usize)],
    focus: usize,
) {
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
}

fn draw_dialogs(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(d) = app.dialog.as_ref() else { return };
    app.dlg_btn_rects.clear();
    match d.kind {
        DialogKind::Add => draw_add_dialog(f, app, area),
        DialogKind::Delete => draw_delete_dialog(f, app, area),
    }
}

fn draw_add_dialog(f: &mut Frame, app: &mut App, area: Rect) {
    let dlg = dialog_rect(area, 74, 11);
    f.render_widget(Clear, dlg);
    clip_wide_at_edges(f, dlg);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            " 添加下载任务 ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(dlg);
    f.render_widget(block, dlg);
    if inner.width < 24 || inner.height < 9 {
        return;
    }

    let (focus, url, dir, conns, ck_type, ck_value, ck_open, ck_sel) = {
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
        )
    };
    let (algo_name, algo_need) = CHECKSUM_ALGOS[ck_type.min(CHECKSUM_ALGOS.len() - 1)];

    // 五个字段行（URL / 保存目录 / 并发数 / 校验算法 / 校验码）
    // 校验算法为下拉选择行（Enter/Space/点击展开），其余为文本输入行
    let fields: [(&str, String, String, bool); 5] = [
        ("URL", url, "https://example.com/file.zip".to_string(), false),
        ("保存到", dir, "/srv/downloads".to_string(), false),
        ("并发", conns, "4".to_string(), false),
        ("校验", algo_name.to_string(), String::new(), true),
        (
            "校验码",
            ck_value,
            format!("{} 位十六进制（可留空）", algo_need),
            false,
        ),
    ];
    let avail = (inner.width as usize).saturating_sub(12);
    let mut type_row_y = inner.y + 4;
    for (i, (lab, val, ph, is_sel)) in fields.iter().enumerate() {
        let focused = focus == i;
        let row_y = inner.y + 1 + i as u16;
        if *is_sel {
            type_row_y = row_y;
        }
        let mut spans = vec![
            Span::styled(
                pad_right(lab, 7),
                Style::default().fg(if focused { ACCENT } else { DIM }),
            ),
            Span::styled("> ".to_string(), Style::default().fg(DIM2)),
        ];
        if *is_sel {
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
                spans.push(Span::styled(
                    "  Enter 选择算法",
                    Style::default().fg(DIM2),
                ));
            }
        } else if val.is_empty() {
            spans.push(Span::styled(
                truncate(ph, avail),
                Style::default().fg(DIM2),
            ));
        } else {
            // 长校验码尾部显示（…+末尾字符），便于核对输入结尾
            let shown = if w(val) > avail {
                let keep = avail.saturating_sub(1);
                format!("…{}", val.chars().skip(val.chars().count() - keep).collect::<String>())
            } else {
                val.clone()
            };
            spans.push(Span::styled(shown, Style::default().fg(Color::White)));
        }
        if focused && !*is_sel {
            spans.push(Span::styled(
                "▏",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ));
        }
        let rect = Rect {
            x: inner.x + 1,
            y: row_y,
            width: inner.width.saturating_sub(2),
            height: 1,
        };
        f.render_widget(Paragraph::new(Line::from(spans)), rect);
        // 回填字段行命中区域（鼠标点击聚焦；校验算法行再展开下拉框）
        app.dlg_field_rects.push((rect, i));
    }

    // 按钮行：[ 确认 ] [ 取消 ]
    draw_button_row(f, app, inner, inner.y + 7, &[("确认", 5), ("取消", 6)], focus);

    // 提示行（下拉框展开时切换为列表操作提示）
    let hint = if ck_open {
        " ↑↓ 选择算法 · Enter 确认选择 · Esc 关闭列表"
    } else {
        " Enter 确认 · Tab/↑↓ 切换 · Esc 取消 · 校验码留空 = 不校验"
    };
    f.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(DIM2))),
        Rect {
            x: inner.x + 1,
            y: inner.y + 8,
            width: inner.width.saturating_sub(2),
            height: 1,
        },
    );

    // 校验算法下拉框（浮层最后绘制，覆盖对话框与下层内容）
    if ck_open {
        let items = CHECKSUM_ALGOS;
        let pw = 24u16;
        let ph = items.len() as u16 + 2;
        // 优先展开在校验算法行下方；超出终端底部则改为行上方展开
        let mut py = type_row_y + 1;
        if py + ph > area.y + area.height {
            py = type_row_y.saturating_sub(ph);
        }
        let px = (inner.x + 8)
            .min(area.x + area.width.saturating_sub(pw + 1))
            .max(area.x + 1);
        let prect = Rect {
            x: px,
            y: py.max(area.y),
            width: pw,
            height: ph,
        };
        f.render_widget(Clear, prect);
        clip_wide_at_edges(f, prect);
        let pblock = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT))
            .title(Span::styled(
                " 校验算法 ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ));
        let pinner = pblock.inner(prect);
        f.render_widget(pblock, prect);
        for (i, (name, need)) in items.iter().enumerate() {
            let sel = i == ck_sel;
            let bg = if sel { Color::DarkGray } else { Color::Reset };
            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", if sel { "▸" } else { " " }),
                    Style::default()
                        .fg(if sel { ACCENT } else { DIM2 })
                        .bg(bg),
                ),
                Span::styled(
                    pad_right(name, 9),
                    Style::default()
                        .fg(if sel { Color::White } else { FG })
                        .bg(bg)
                        .add_modifier(if sel { Modifier::BOLD } else { Modifier::empty() }),
                ),
                Span::styled(
                    format!("{} 位", need),
                    Style::default()
                        .fg(if sel { ACCENT } else { DIM2 })
                        .bg(bg),
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
}

fn draw_delete_dialog(f: &mut Frame, app: &mut App, area: Rect) {
    let dlg = dialog_rect(area, 66, 7);
    f.render_widget(Clear, dlg);
    clip_wide_at_edges(f, dlg);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(RED))
        .title(Span::styled(
            " 删除任务 ",
            Style::default().fg(RED).add_modifier(Modifier::BOLD),
        ));
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
                format!("「{}」", truncate(&task_name, (inner.width as usize).saturating_sub(22))),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
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
    draw_button_row(
        f,
        app,
        inner,
        inner.y + 3,
        &[("仅删除任务", 0), ("删除任务和文件", 1), ("取消", 2)],
        focus,
    );

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
// 总入口
// ---------------------------------------------------------------------------

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let outer = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(4),
    ])
    .split(area);

    draw_header(f, app, outer[0]);
    draw_tabs(f, app, outer[1]);

    let wide = outer[2].width >= 100;
    if wide && app.show_chart {
        let cols = Layout::horizontal([
            Constraint::Percentage(58),
            Constraint::Percentage(42),
        ])
        .split(outer[2]);
        draw_list(f, app, cols[0]);
        let right = Layout::vertical([Constraint::Percentage(58), Constraint::Min(4)])
            .split(cols[1]);
        draw_detail(f, app, right[0]);
        draw_chart(f, app, right[1]);
    } else {
        draw_list(f, app, outer[2]);
    }

    draw_footer(f, app, outer[3]);

    // 对话框最后绘制（覆盖层）
    if app.dialog.is_some() {
        draw_dialogs(f, app, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_counts_cjk_as_two() {
        assert_eq!(w("abc"), 3);
        assert_eq!(w("下载"), 4);
        assert_eq!(w("a 下载 b"), 8);
        assert_eq!(w(""), 0);
    }

    #[test]
    fn truncate_adds_ellipsis_and_respects_width() {
        assert_eq!(truncate("short", 10), "short");
        let t = truncate("very-long-name.bin", 8);
        assert_eq!(w(&t), 8);
        assert!(t.ends_with('…'));
        // CJK 不会被切成半格
        let t = truncate("下载任务名称很长", 6);
        assert!(w(&t) <= 6);
    }

    #[test]
    fn pad_respects_display_width() {
        assert_eq!(pad_right("ab", 5), "ab   ");
        assert_eq!(pad_left("ab", 5), "   ab");
        assert_eq!(pad_right("下载", 5), "下载 ");
        assert_eq!(pad_right("abc", 2), "abc"); // 超宽不截断
    }

    #[test]
    fn fmt_size_thresholds() {
        assert_eq!(fmt_size(999), "999 B");
        assert_eq!(fmt_size(1000), "1 KB");
        assert_eq!(fmt_size(1_500_000), "1.5 MB");
        assert_eq!(fmt_size(2_500_000_000), "2.50 GB");
    }

    #[test]
    fn fmt_speed_normalizes_negative_zero() {
        assert_eq!(fmt_speed(-0.0), "0 B/s");
        assert_eq!(fmt_speed(0.0), "0 B/s");
        assert_eq!(fmt_speed(1500.0), "2 KB/s");
        assert_eq!(fmt_speed(2_000_000.0), "2.0 MB/s");
    }

    #[test]
    fn fmt_dur_and_eta() {
        assert_eq!(fmt_dur(65), "01:05");
        assert_eq!(fmt_dur(3_725), "1:02:05");
        assert_eq!(fmt_eta(None), "--:--");
        assert_eq!(fmt_eta(Some(65)), "01:05");
    }

    #[test]
    fn fmt_size_pair_same_unit() {
        assert_eq!(fmt_size_pair(1_720_000_000, 4_320_000_000), "1.72/4.32 GB");
        assert_eq!(fmt_size_pair(500, 900), "500/900 B");
    }

    #[test]
    fn plain_bar_fills_and_pads() {
        let spans = plain_bar(10, 0.5, GREEN);
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 10);
        assert_eq!(spans[0].content, "█".repeat(5));
        // 空进度仍产出整条空槽
        let spans = plain_bar(4, 0.0, GREEN);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "░".repeat(4));
    }
}
