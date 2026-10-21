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
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::chart::draw_chart;
use super::text::{fmt_size, fmt_speed};
use super::{
    bordered_block, overlay_open, ACCENT, BORDER, DIM, DIM2, FG, LIGHT_BLUE, MAGENTA, YELLOW,
};
use crate::app::{App, FILTERS};

pub(super) fn draw_header(f: &mut Frame, app: &mut App, area: Rect, with_chart: bool) {
    // 头部聚合口径单源 App 访问器（architect v116：自渲染路径提取为可无头单测的
    // 应用层策略——全局 ↓↑ 合计与并发 N 总数原内联在本函数）
    let dl = app.global_dl_speed();
    let ul = app.global_ul_speed();
    // 会话内峰值（头部「峰值 ↓↑」口径；architect v145：提取为 App 访问器，
    // 与全局 ↓↑ 同法——KB/s 采样历史的 B/s 还原收口应用层）与全局并发连接数
    // （下载中/做种中任务的连接总数，FR-01-94 ②/FR-01-96 数据面口径）
    let dlp_val = app.peak_dl_speed();
    let ulp_val = app.peak_ul_speed();
    let conns_n = app.conns_display_total();

    // v1.32/FR-01-94①（对齐 ezr-demo m3）：标题左对齐（Block 默认锚点，
    // 不居中）、标题单元 " ◆ EZR Downloader "（名称前后各 1 列空白，前后
    // padding 距离相等）；**版本号移除**（原「随期号递进」退役，版本展示面
    // 迁移关于窗口 FR-01-111/D39）；峰值不写在标题行边框上
    let block = bordered_block(BORDER).title(Line::from(vec![
        Span::styled(
            " ◆ ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "EZR Downloader ",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // 右侧内嵌流量图：│ 分隔符 + 图区（v1.32/FR-01-94⑤ 修订，对齐
    // ezr-demo m3：图区紧贴分隔符与右边框——左右 0 列空白，与上下 padding
    // 一致）；图宽 = 内容区 20%，并保证字段区至少 92 列（不足时压缩图宽，
    // v1.32：clamp 上限改 内容宽 − 93）；占内容区全高
    let chart_w = if with_chart && inner.height >= 3 && inner.width >= 49 {
        (inner.width / 5).clamp(6, inner.width.saturating_sub(93).max(6))
    } else {
        0
    };
    let fields_w = if chart_w > 0 {
        inner.width - chart_w - 1
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
    let dlp = format!("↓ {}", fmt_speed(dlp_val));
    let ulp = format!("↑ {}", fmt_speed(ulp_val));
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
    // 已完成 (n) │ 下载槽位 x/y（槽位内联左对齐；已满黄色加粗：等待任务需排队）。
    // v1.30/FR-01-108：页签高亮跟随列表焦点——列表聚焦且无浮层 → 黑字青底加粗；
    // 否则（焦点在其他面板 / 浮层打开）青字加粗无底色，非当前页签 FG。
    // 每帧回填页签命中矩形 tab_rects（CJK 计宽），供鼠标点击切换（FR-01-80 v1.30）
    if inner.height >= 3 {
        // 页签计数口径在应用层访问器（architect v126 提取，无头单测面）
        let counts = app.tab_counts();
        let row_y = inner.y + 2;
        let mut spans: Vec<Span> = Vec::new();
        let mut tabs_w: u16 = 0;
        // 页签高亮：列表聚焦且无浮层时反色底，否则青字加粗
        let tab_active = !overlay_open(app) && app.focus == crate::app::Panel::List;
        app.tab_rects.clear();
        for (i, label) in FILTERS.iter().enumerate() {
            let text = format!(" {} ({}) ", label, counts[i]);
            let tab_x = inner.x + tabs_w;
            // 显示宽经 Span::width 计算（CJK 计宽；unicode-width 非直接依赖）
            let text_w = Span::raw(text.as_str()).width() as u16;
            let span = Span::styled(
                text.clone(),
                if i == app.filter {
                    if tab_active {
                        Style::default()
                            .fg(Color::Black)
                            .bg(ACCENT)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
                    }
                } else {
                    Style::default().fg(FG)
                },
            );
            tabs_w += text_w;
            spans.push(span);
            // 回填页签命中矩形（显示宽经 Span::width CJK 计宽）
            app.tab_rects.push((
                Rect {
                    x: tab_x,
                    y: row_y,
                    width: text_w,
                    height: 1,
                },
                i,
            ));
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
        // v1.32/FR-01-94⑤：图区紧贴分隔符与右边框（分隔符右侧 0 列空白、
        // 图区右缘不另留白；无边框、占全高）
        let chart_area = Rect {
            x: inner.x + fields_w + 1,
            width: chart_w,
            ..inner
        };
        draw_chart(f, app, chart_area);
    }
}
