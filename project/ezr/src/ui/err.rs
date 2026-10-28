use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::ui::btn::{button_spans, clip_wide_at_edges, draw_close_btn, overlay_rect};
use crate::ui::text::{truncate, w};
use crate::ui::{panel_border_style, panel_title_style, RED};

pub(crate) const ERR_W: u16 = 66;
pub(crate) const ERR_H: u16 = 7;

#[allow(dead_code)]
pub(crate) fn draw_err(f: &mut Frame, app: &mut App, area: Rect) {
    let dlg = {
        let r = overlay_rect(area, ERR_W, ERR_H, app.err_pos);
        if app.err_pos.is_none() {
            app.err_pos = Some((r.x, r.y));
        }
        app.err_area = Some(r);
        r
    };
    f.render_widget(Clear, dlg);
    clip_wide_at_edges(f, dlg);

    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" 错误 ", panel_title_style(true)));
    let inner = block.inner(dlg);
    f.render_widget(block, dlg);

    app.popup_close_rect = draw_close_btn(f, app, dlg, w(" 错误 "));
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
    let msg = truncate(&app.err_msg, msg_rect.width as usize);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(msg, Style::default().fg(RED)))),
        msg_rect,
    );

    let btn_row = inner.y + 3;
    let label = "[ 确定 ]";
    let bw = w(label) + 2;
    let bx = inner.x as usize + ((inner.width as usize).saturating_sub(bw)) / 2;
    let btn_rect = Rect {
        x: bx as u16,
        y: btn_row,
        width: bw as u16,
        height: 1,
    };
    f.render_widget(
        Paragraph::new(Line::from(button_spans("确定", false))),
        btn_rect,
    );
    app.err_btn_rect = Some(btn_rect);
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-uierr", tag)
    }

    #[tokio::test]
    async fn renders_error_window_and_backfills_geometry() {
        let mut app = make_app("gold");
        app.open_error_window("暂不支持该类型的链接");
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| draw_err(f, &mut app, f.area())).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("错误"), "标题: {s}");
        assert!(s.contains("暂不支持该类型的链接"), "消息行: {s}");
        assert!(s.contains("确定"), "确定按钮: {s}");
        assert!(s.contains("✗"), "右上角关闭按钮: {s}");
        let a = app.err_area.expect("err_area 回填");
        assert_eq!((a.x, a.y, a.width, a.height), (17, 11, 66, 7));
        assert_eq!(app.err_pos, Some((17, 11)), "首帧位置记忆");
        let b = app.err_btn_rect.expect("按钮热区回填");
        assert_eq!(b.y, 11 + 1 + 3, "按钮行 = 内框 y+3");
        assert_eq!(b.width, 10, "[ 确定 ] + 两侧 padding");
        assert_eq!(b.x, 17 + 1 + (64 - 10) / 2, "按钮水平居中（内框 64 宽）");
        app.err_pos = Some((3, 3));
        term.draw(|f| draw_err(f, &mut app, f.area())).unwrap();
        let a2 = app.err_area.unwrap();
        assert_eq!((a2.x, a2.y), (3, 3), "拖拽位置优先");
        app.close_error_window();
        assert!(!app.err_open && app.err_area.is_none() && app.err_pos.is_none());
    }
}
