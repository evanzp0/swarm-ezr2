use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::App;

use super::*;

pub(crate) fn draw_footer(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    const PAD: u16 = 2;
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

    app.footer_cmd_rects.clear();
    let cmds = [
        ("A", "命令"),
        ("D", "删除"),
        ("M", "修改"),
        ("Q", "退出"),
        ("H", "帮助"),
    ];
    let mut spans: Vec<Span> = Vec::new();
    let mut x = row1.x;
    let right = row1.x.saturating_add(row1.width);
    for (i, (key, label)) in cmds.iter().enumerate() {
        let chip_w = 2 + w(label) as u16;
        let sep_w = 3u16;
        let placed = spans.is_empty();
        let need = if placed { chip_w } else { sep_w + chip_w };
        if x.saturating_add(need) > right {
            break;
        }
        if !placed {
            spans.push(Span::styled(" · ", Style::default().fg(DIM2)));
            x += sep_w;
        }

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
