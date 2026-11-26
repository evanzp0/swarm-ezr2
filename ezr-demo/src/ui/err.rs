use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::{panel_border_style, panel_title_style, RED};
use crate::app::App;

pub(crate) const ERR_W: u16 = 66;
pub(crate) const ERR_H: u16 = 7;

pub(crate) fn draw_err(f: &mut Frame, app: &mut App, area: Rect) {
    let dlg = super::dialogs::dialog_rect_at(area, app.err_pos, ERR_W, ERR_H);
    if app.err_pos.is_none() {
        app.err_pos = Some((dlg.x, dlg.y));
    }
    app.err_area = Some(dlg);
    f.render_widget(Clear, dlg);
    super::clip_wide_at_edges(f, dlg);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" 错误 ", panel_title_style(true)));
    let inner = block.inner(dlg);
    f.render_widget(block, dlg);

    app.popup_close_rect = super::draw_close_btn(f, app, dlg, super::w(" 错误 "));
    if inner.width < 20 || inner.height < 4 {
        app.err_btn_rect = None;
        return;
    }

    let msg_rect = Rect {
        x: inner.x + 2,
        y: inner.y + 1,
        width: inner.width.saturating_sub(4),
        height: 1,
    };
    let msg = super::truncate(&app.err_msg, msg_rect.width as usize);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(msg, Style::default().fg(RED)))),
        msg_rect,
    );

    let btn_row = inner.y + 3;
    let label = "[ 确定 ]";
    let bw = super::w(label) + 2;
    let bx = inner.x as usize + ((inner.width as usize).saturating_sub(bw)) / 2;
    let btn_rect = Rect {
        x: bx as u16,
        y: btn_row,
        width: bw as u16,
        height: 1,
    };
    f.render_widget(
        Paragraph::new(Line::from(super::dialogs::button_spans("确定", false))),
        btn_rect,
    );
    app.err_btn_rect = Some(btn_rect);
}
