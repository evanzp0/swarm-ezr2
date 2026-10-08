//! header — 头部仪表面板（通栏 5 行定稿布局）、单色面积流量图与页脚快捷键提示
//!
//! 定稿口径（FR-01-94/95，对齐 ezr-demo REQ-2.1/5.2/6.1/6.2/6.3/7.1/7.3/8.1/8.3/8.4/8.5）：
//! 5 行 = 标题边框行 + 字段行/分割线/页签行 3 内容行 + 底边框；字段单行顺序固定
//! 并发 → 会话已下载 → 全局 ↓↑ → 峰值 ↓↑（峰值写在面板内容里，峰值 ↓ 淡蓝与曲线
//! 同色兼作图例）；内容区右侧 = │ 竖线分隔符（贯通全高）+ 左右各 1 列空白 +
//! 无边框单色面积流量图（占内容区全高、宽 20%，仅绘下行流量）。原「任务队列」
//! 面板撤销，页签行并入内容行 3（槽位内联左对齐、紧跟「已完成」页签以 │ 分隔）。

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use super::text::{fmt_size, fmt_speed};
use super::{ACCENT, BORDER, DIM, DIM2, FG, LIGHT_BLUE, MAGENTA, YELLOW};
use crate::app::{App, FILTERS};

pub(super) fn draw_header(f: &mut Frame, app: &App, area: Rect, with_chart: bool) {
    let dl: f64 = app
        .tasks
        .iter()
        .filter(|t| t.state == crate::model::TaskState::Downloading)
        .map(|t| t.speed)
        .sum();
    let ul: f64 = app
        .tasks
        .iter()
        .filter(|t| t.upload_speed > 0.0)
        .map(|t| t.upload_speed)
        .sum();
    // 会话内峰值（速度历史为 KB/s）与全局并发连接数（下载中/做种中任务的连接总数，
    // FR-01-94 ②/FR-01-96 数据面口径）
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

/// 单色面积流量图（FR-01-95，定稿口径）：逐列自底向上填充，顶线以
/// ▁▂▃▄▅▆▇█ 八级部分块呈现 1/8 格亚字符精度的平滑曲线；全图统一一种颜色
/// （下载淡蓝 RGB(122,185,242)，REQ-8.5）；序列经双向 EMA 平滑（α=0.75）+
/// 窗口 min–max 自适应缩放（上下 ≈15% 呼吸边距，下限取窗口峰值 5%），
/// 每列取双子样本较大值（保尖峰）——面积首尾相接、无断列、无间隔、无盲文点阵。
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

    // 逐行渲染（REQ-8.5：全图仅一种颜色，统一下载淡蓝）：
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

pub(super) fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(inner);

    // 快捷键行
    let key = |s: &str| {
        Span::styled(
            s.to_string(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )
    };
    let desc = |s: &str| Span::styled(s.to_string(), Style::default().fg(DIM));
    let mut l1 = vec![
        key(" ↑↓"),
        desc(" 选择  "),
        key("Space"),
        desc(" 暂停/继续  "),
        key("R"),
        desc(" 重试  "),
        key("A"),
        desc(" 添加  "),
        key("D"),
        desc(" 删除  "),
        key("U/J"),
        desc(" 上移/下移  "),
        key("C"),
        desc(" 清已完成  "),
        key("Tab"),
        desc(" 页签  "),
        key("G"),
        // FR-01-98：G 切换右栏两面板（详情/并发明细），流量图常显 → 提示「面板」
        desc(" 面板  "),
        key("Q"),
        desc(" 退出"),
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
    f.render_widget(Paragraph::new(Line::from(l2)), rows[1]);
}
