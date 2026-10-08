//! header — 头部仪表面板（通栏 5 行定稿布局）、单色面积流量图与页脚快捷键提示
//!
//! 定稿口径（FR-01-94/95，对齐 ezr-demo REQ-2.1/5.2/6.1/6.2/6.3/7.1/7.3/8.1/8.3/8.4/8.5）：
//! 5 行 = 标题边框行 + 字段行/分割线/页签行 3 内容行 + 底边框；字段单行顺序固定
//! 并发 → 会话已下载 → 全局 ↓↑ → 峰值 ↓↑（峰值写在面板内容里，峰值 ↓ 淡蓝与曲线
//! 同色兼作图例）；内容区右侧 = │ 竖线分隔符（贯通全高）+ 左右各 1 列空白 +
//! 无边框单色面积流量图（占内容区全高、宽 20%，仅绘下行流量）。原「任务队列」
//! 面板撤销，页签行并入内容行 3（槽位内联左对齐、紧跟「已完成」页签以 │ 分隔）。

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use super::chart::draw_chart;
use super::text::{fmt_size, fmt_speed};
use super::{ACCENT, BORDER, DIM, DIM2, FG, LIGHT_BLUE, MAGENTA, YELLOW};
use crate::app::{App, FILTERS};

pub(super) fn draw_header(f: &mut Frame, app: &App, area: Rect, with_chart: bool) {
    // 头部聚合口径单源 App 访问器（architect v116：自渲染路径提取为可无头单测的
    // 应用层策略——全局 ↓↑ 合计与并发 N 总数原内联在本函数）
    let dl = app.global_dl_speed();
    let ul = app.global_ul_speed();
    // 会话内峰值（速度历史为 KB/s）与全局并发连接数（下载中/做种中任务的连接总数，
    // FR-01-94 ②/FR-01-96 数据面口径）
    let peak_dl = app.speed_hist.iter().copied().max().unwrap_or(0);
    let peak_ul = app.up_hist.iter().copied().max().unwrap_or(0);
    let conns_n = app.conns_display_total();

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
            Span::styled(format!("  v{}", crate::VERSION), Style::default().fg(DIM)),
        ]));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // 右侧内嵌流量图：│ 分隔符 + 左右各 1 列空白 + 图宽 = 内容区 20%，并保证
    // 字段区至少 92 列（不足时压缩图宽）；占内容区全高（FR-01-95）
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

    // 字段行（内容行 1）：单行排列，顺序固定：并发 → 会话已下载 → 全局 → 峰值
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

    // 内容行 2：横向分割线（贯通字段区宽、止于 │ 分隔符；图区让位给波形不画线）
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

    // 内容行 3：页签行（原「任务队列」面板并入，FR-01-94 ④）：正在下载 (n) │
    // 已完成 (n) │ 下载槽位 x/y（槽位内联左对齐；已满黄色加粗：等待任务需排队）
    if inner.height >= 3 {
        let doing = app.tasks.iter().filter(|t| !t.state.is_done()).count();
        let done = app.tasks.len() - doing;
        let counts = [doing, done];
        let row_y = inner.y + 2;
        let mut spans: Vec<Span> = Vec::new();
        let mut tabs_w: u16 = 0;
        for (i, label) in FILTERS.iter().enumerate() {
            let span = Span::styled(
                format!(" {} ({}) ", label, counts[i]),
                if i == app.filter {
                    Style::default()
                        .fg(Color::Black)
                        .bg(ACCENT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(FG)
                },
            );
            tabs_w += span.width() as u16;
            spans.push(span);
            if i + 1 < FILTERS.len() {
                let dv = Span::styled("│", Style::default().fg(DIM2));
                tabs_w += dv.width() as u16;
                spans.push(dv);
            }
        }
        // 下载槽位：内联左对齐，紧跟「已完成」页签之后并以 │ 分隔（REQ-8.1）
        let slots = app.used_slots();
        let slots_full = slots >= app.max_slots;
        let slot = Span::styled(
            format!(" 下载槽位 {}/{}", slots, app.max_slots),
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

    // │ 分隔符：字段区与图区之间，贯通头部内容区全高（FR-01-94 ⑤）
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
