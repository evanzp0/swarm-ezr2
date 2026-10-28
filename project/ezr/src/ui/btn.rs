use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::text::w;
use super::{panel_border_style, ACCENT, DIM2, FG, RED};
use crate::app::App;

pub(super) fn dialog_rect(area: Rect, dw: u16, dh: u16) -> Rect {
    let dw = dw.min(area.width);
    let dh = dh.min(area.height);
    Rect {
        x: area.x + (area.width.saturating_sub(dw)) / 2,
        y: area.y + (area.height.saturating_sub(dh)) / 2,
        width: dw,
        height: dh,
    }
}

pub(super) fn overlay_rect(area: Rect, dw: u16, dh: u16, pos: Option<(u16, u16)>) -> Rect {
    let w = dw.min(area.width);
    let h = dh.min(area.height);
    match pos {
        Some((px, py)) => Rect {
            x: px.min(area.width.saturating_sub(w)),
            y: py.min(area.height.saturating_sub(h)),
            width: w,
            height: h,
        },
        None => dialog_rect(area, dw, dh),
    }
}

pub(super) fn button_spans(txt: &str, focused: bool) -> Vec<Span<'static>> {
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

pub(super) fn draw_button_row(
    f: &mut Frame,
    app: &mut App,
    inner: Rect,
    btn_row: u16,
    labels: &[(&str, usize)],
    focus: usize,
) {
    let area = f.area();
    let btn_ws: Vec<usize> = labels
        .iter()
        .map(|(txt, _)| w(&format!("[ {} ]", txt)) + 2)
        .collect();
    let total_w: usize = btn_ws.iter().sum::<usize>() + 2 * (labels.len() - 1);
    let mut bx = inner.x as usize + ((inner.width as usize).saturating_sub(total_w)) / 2;
    let inner_right = inner.x as usize + inner.width as usize;
    for ((txt, idx), bw) in labels.iter().zip(btn_ws.iter()) {
        let focused = focus == *idx;
        let line = Line::from(button_spans(txt, focused));
        let width = (*bw)
            .min(inner_right.saturating_sub(bx))
            .min(area.width as usize);
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
}

fn clip_left_edge(buf: &mut ratatui::buffer::Buffer, x: u16, y: u16) {
    if x == 0 {
        return;
    }
    let i = buf.index_of(x - 1, y);
    if w(buf.content[i].symbol()) > 1 {
        buf.content[i].set_char(' ');
    }
}

fn clip_right_edge(buf: &mut ratatui::buffer::Buffer, xr: u16, y: u16) {
    if xr + 1 >= buf.area.width {
        return;
    }
    let i = buf.index_of(xr + 1, y);
    if buf.content[i].symbol().is_empty() {
        buf.content[i].set_char(' ');
    }
}

pub(super) fn clip_wide_at_edges(f: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let buf = f.buffer_mut();
    let x0 = area.x;
    let xr = area.right() - 1;
    for y in area.y..area.bottom() {
        clip_left_edge(buf, x0, y);
        clip_right_edge(buf, xr, y);
    }
}

pub(super) fn draw_close_btn(
    f: &mut Frame,
    app: &App,
    popup: Rect,
    title_w: usize,
) -> Option<Rect> {
    if popup.width < 12 || popup.height < 3 {
        return None;
    }
    let bx = popup.x + popup.width - 5;
    if (bx as usize) < popup.x as usize + 1 + title_w + 2 {
        return None;
    }
    let r = Rect {
        x: bx,
        y: popup.y,
        width: 3,
        height: 1,
    };
    let hovered = app.pointer.is_some_and(|(px, py)| {
        px >= r.x
            && px < r.x.saturating_add(r.width)
            && py >= r.y
            && py < r.y.saturating_add(r.height)
    });
    let hot = app.x_press || hovered;
    let bracket = panel_border_style(true);
    let x_st = if hot {
        Style::default().fg(RED).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DIM2).remove_modifier(Modifier::BOLD)
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("[", bracket),
            Span::styled("✗", x_st),
            Span::styled("]", bracket),
        ])),
        r,
    );
    Some(r)
}
