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
    let dl = app.global_dl_speed();
    let ul = app.global_ul_speed();
    let dlp_val = app.peak_dl_speed();
    let ulp_val = app.peak_ul_speed();
    let conns_n = app.conns_display_total();

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

    let fields = Line::from(vec![
        Span::raw(" "),
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

    if inner.height >= 3 {
        let counts = app.tab_counts();
        let row_y = inner.y + 2;
        let mut spans: Vec<Span> = Vec::new();
        let mut tabs_w: u16 = 0;
        let tab_active = !overlay_open(app) && app.focus == crate::app::Panel::List;
        app.tab_rects.clear();
        for (i, label) in FILTERS.iter().enumerate() {
            let text = format!(" {} ({}) ", label, counts[i]);
            let tab_x = inner.x + 1 + tabs_w;
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
                x: inner.x + 1,
                y: row_y,
                width: fields_w.saturating_sub(1),
                height: 1,
            },
        );
    }

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
        let chart_area = Rect {
            x: inner.x + fields_w + 1,
            width: chart_w,
            ..inner
        };
        draw_chart(f, app, chart_area);
    }
}
