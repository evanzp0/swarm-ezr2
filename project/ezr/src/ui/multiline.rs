use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use super::{panel_border_style, panel_title_style, DIM2, FOCUS_BORDER};
use crate::multiline::MultiLineBuf;
use crate::scrollbar::{Arrow as SbArrow, Bar as SbBar};
use crate::textfield::disp_window;
use crate::ui::scrollable_panel::{ScrollablePanel, ScrollbarMode};

pub struct MultiLineRendered {
    pub inner: Rect,

    pub sb: Option<SbBar>,

    pub h_sb: Option<SbBar>,

    pub cursor: Option<(u16, u16)>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_multi_line_input(
    f: &mut Frame,
    rect: Rect,
    title: Option<&str>,
    placeholder: &str,
    focused: bool,
    pressed_v: Option<SbArrow>,
    pressed_h: Option<SbArrow>,
    wrap: bool,
    buf: &mut MultiLineBuf,
) -> MultiLineRendered {
    let mode = if wrap {
        ScrollbarMode::Vertical
    } else {
        ScrollbarMode::Both
    };
    let mut sb = ScrollablePanel::new(mode);
    sb.thumb(if focused { FOCUS_BORDER } else { super::BORDER })
        .pressed_v(pressed_v)
        .pressed_h(pressed_h);
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(focused));
    if let Some(t) = title {
        block = block.title(Span::styled(t.to_string(), panel_title_style(focused)));
    }
    let inner = sb.render_block(f, rect, block);

    let text_w = inner.width as usize;
    let view_h = inner.height as usize;

    buf.text_w = text_w;
    buf.wrap = wrap;

    let (rows, total_rows, cursor_vrow, cursor_vcol) = if wrap {
        let rows = buf.wrap_rows(text_w);
        let total = rows.len();
        let (r, c) = buf.cursor_visual(text_w);
        (rows, total, r, c as usize)
    } else {
        let rows = buf.nowrap_rows();
        let total = rows.len();
        let (r, c) = buf.cursor_visual_nowrap();
        (rows, total, r, c)
    };

    let content_w = if wrap { 0 } else { buf.nowrap_content_w() };
    if !wrap {
        buf.max_scroll_x = content_w.saturating_sub(text_w);
        buf.view_x = buf.view_x.min(buf.max_scroll_x);
    }

    if buf.follow {
        if wrap {
            buf.scroll_follow(total_rows, view_h, cursor_vrow);
        } else {
            buf.scroll_follow_nowrap(total_rows, view_h, cursor_vrow, text_w, cursor_vcol);
        }
        buf.follow = false;
    } else {
        buf.clamp_view(total_rows, view_h);
    }
    buf.max_scroll = total_rows.saturating_sub(view_h);
    let start = buf.view;

    let empty = buf.is_buffer_empty();
    let mut lines: Vec<Line> = Vec::with_capacity(view_h);
    for vi in 0..view_h {
        let ri = start + vi;
        let span = if empty && vi == 0 && !placeholder.is_empty() {
            Span::styled(placeholder.to_string(), Style::default().fg(DIM2))
        } else {
            match rows.get(ri) {
                Some((li, s, n)) => {
                    let text: String = if wrap {
                        buf.lines[*li].value.chars().skip(*s).take(*n).collect()
                    } else {
                        disp_window(&buf.lines[*li].value, buf.view_x, text_w)
                    };
                    Span::styled(text, Style::default().fg(Color::White))
                }
                None => Span::styled(String::new(), Style::default()),
            }
        };
        lines.push(Line::from(span));
    }
    f.render_widget(Paragraph::new(lines), inner);

    let cursor = if focused
        && cursor_vrow >= start
        && cursor_vrow < start + view_h
        && cursor_vcol >= buf.view_x
        && cursor_vcol <= buf.view_x + text_w.max(1)
    {
        let disp_col = (cursor_vcol - buf.view_x).min(text_w.saturating_sub(1)) as u16;
        Some((inner.x + disp_col, inner.y + (cursor_vrow - start) as u16))
    } else {
        None
    };

    sb.vertical(total_rows, view_h, start);
    if !wrap {
        sb.horizontal(content_w, text_w, buf.view_x);
    }
    let bars = sb.render_bars(f, rect);

    MultiLineRendered {
        inner,
        sb: bars.vertical,
        h_sb: bars.horizontal,
        cursor,
    }
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;

    #[test]
    fn publishes_geometry_and_consumes_follow() {
        let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
        let mut buf = MultiLineBuf::new();
        buf.set_text("a\nb\nc\nd\ne\nf\ng\nh");
        buf.wrap = false;
        buf.follow = true;
        term.draw(|f| {
            let r = draw_multi_line_input(
                f,
                Rect::new(2, 2, 40, 5),
                Some(" trackers "),
                "hint",
                true,
                None,
                None,
                false,
                &mut buf,
            );
            assert_eq!(r.inner.width, 38, "内框不含边框");
            assert_eq!(r.inner.height, 3, "内框不含边框");
            assert!(r.cursor.is_some(), "聚焦发布原生光标位");
            assert!(r.sb.is_some(), "垂直滚动条几何");
        })
        .unwrap();
        assert_eq!(buf.max_scroll, 5, "max_scroll = 总行 − 可视行");
        assert!(!buf.follow, "跟随被渲染层消费一次后复位");
        assert_eq!(buf.max_scroll_x, 0);
    }
}
