use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::{App, Panel, SbKey};
use crate::scrollable_panel::{ScrollablePanel, ScrollbarMode};

use super::*;

pub(crate) fn draw_conns(f: &mut Frame, app: &mut App, area: Rect) {
    app.conns_v_sb = None;
    app.conns_h_sb = None;
    app.conns_max_hscroll = 0;

    let (is_bt, rows_idx, max_conc, empty) = match app.sel_task() {
        Some(t) => {
            let shows = t.state.shows_conns();
            let idx: Vec<usize> = if shows {
                t.connections
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.is_active())
                    .map(|(i, _)| i)
                    .collect()
            } else {
                Vec::new()
            };

            let empty = idx.is_empty();
            (t.protocol.is_bt(), idx, t.concurrency, empty)
        }
        None => (false, Vec::new(), 0, true),
    };

    let focused = app.focus == Panel::Conns && !overlay_open(app);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(focused))
        .title(Span::styled(" 并发连接 ", panel_title_style(focused)))
        .title(
            Line::from(Span::styled(
                format!(" {}/{} 项 ", rows_idx.len(), max_conc),
                Style::default().fg(DIM),
            ))
            .alignment(Alignment::Right),
        );

    let mut sb = ScrollablePanel::new(ScrollbarMode::Both);
    sb.thumb(if focused { FOCUS_BORDER } else { BORDER })
        .pressed_v(app.sb_arrow_pressed(SbKey::ConnsV))
        .pressed_h(app.sb_arrow_pressed(SbKey::ConnsH));
    let inner = sb.render_block(f, area, block);
    app.conns_area = Some(inner);

    const PAD_X: u16 = 2;
    let view = Rect {
        x: inner.x.saturating_add(PAD_X),
        y: inner.y,
        width: inner.width.saturating_sub(PAD_X * 2),
        height: inner.height,
    };
    let rows = (view.height as usize).saturating_sub(1);

    let Some(t) = app.sel_task() else {
        app.visible_conns_rows = 0;

        app.conns_area = None;
        f.render_widget(
            Paragraph::new(Span::styled("（未选中任务）", Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    };
    if empty {
        app.visible_conns_rows = 0;

        app.conns_area = None;
        f.render_widget(
            Paragraph::new(Span::styled("（无并发连接）", Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    let colw: Vec<usize> = if is_bt {
        vec![11, 8, 8, 8, 8, 6, 4]
    } else {
        vec![4, 8, 8]
    };
    let content_w = colw.iter().sum::<usize>() + colw.len().saturating_sub(1);

    let dim = |s: String, wd: usize| Span::styled(pad_left(&s, wd), Style::default().fg(DIM));

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

    let total_rows = rows_idx.len();
    let start = app.conns_scroll.min(total_rows.saturating_sub(rows));
    let data_rows = rows;
    let end = (start + data_rows).min(total_rows);

    for (ri, &ci) in rows_idx[start..end].iter().enumerate() {
        let disp = start + ri;
        let c = &t.connections[ci];

        let sel = focused && disp == app.conns_sel;

        let bg = if sel { SEL_BG } else { Color::Reset };
        let up_on = c.up_speed > 0.0;
        let dl_on = c.speed > 0.0;
        let cell = |s: String, wd: usize, col: Color| {
            Span::styled(pad_left(&s, wd), Style::default().fg(col).bg(bg))
        };
        let spans = if is_bt {
            vec![
                Span::styled(
                    pad_right(&c.ip_masked(), 11),
                    Style::default()
                        .fg(if sel { Color::White } else { FG })
                        .bg(bg),
                ),
                Span::styled(" ", Style::default().bg(bg)),
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
                cell(c.client.to_string(), 6, DIM),
                Span::styled(" ", Style::default().bg(bg)),
                cell(c.transport.label().to_string(), 4, DIM),
            ]
        } else {
            vec![
                cell(
                    format!("{}", ci + 1),
                    4,
                    if sel { Color::White } else { FG },
                ),
                Span::styled(" ", Style::default().bg(bg)),
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

    let hmax = content_w.saturating_sub(view.width as usize) + 1;
    let hscroll = app.conns_hscroll.min(hmax) as u16;
    app.conns_max_hscroll = hmax;
    app.visible_conns_rows = data_rows;
    f.render_widget(Paragraph::new(lines).scroll((0, hscroll)), view);

    sb.vertical(total_rows, data_rows, app.conns_scroll)
        .horizontal(content_w, view.width as usize, app.conns_hscroll);
    let bars = sb.render_bars(f, area);
    app.conns_v_sb = bars.vertical;
    app.conns_h_sb = bars.horizontal;
}
