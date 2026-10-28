use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, SbKey};
use crate::ui::btn::{button_spans, clip_wide_at_edges, dialog_rect, draw_close_btn};
use crate::ui::multiline::draw_multi_line_input;
use crate::ui::text::w;
use crate::ui::{panel_border_style, panel_title_style};

pub(crate) const CMD_W: u16 = 78;
pub(crate) const CMD_H: u16 = 16;

#[allow(dead_code)]
pub(crate) fn draw_cmd(f: &mut Frame, app: &mut App, area: Rect) {
    let dw = CMD_W.min(area.width);
    let dh = CMD_H.min(area.height);
    let rect = match app.cmd_pos {
        Some((px, py)) => Rect {
            x: px.min(area.width.saturating_sub(dw)),
            y: py.min(area.height.saturating_sub(dh)),
            width: dw,
            height: dh,
        },
        None => {
            let r = dialog_rect(area, CMD_W, CMD_H);
            app.cmd_pos = Some((r.x, r.y));
            r
        }
    };
    f.render_widget(Clear, rect);
    clip_wide_at_edges(f, rect);

    let title = " 请输入命令或链接 ";
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(title, panel_title_style(true)));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    app.popup_close_rect = draw_close_btn(f, app, rect, w(title));
    if inner.width < 24 || inner.height < 6 {
        app.cmd_area = Some(rect);
        app.cmd_text_area = None;
        app.cmd_sb = None;
        app.cmd_btn_rect = None;
        app.cmd_cursor_screen = None;
        return;
    }

    let box_h = inner.height.saturating_sub(3);
    let box_rect = Rect {
        x: inner.x + 2,
        y: inner.y + 1,
        width: inner.width.saturating_sub(4),
        height: box_h,
    };
    let focused_box = app.cmd_focus == 0;
    let rendered = draw_multi_line_input(
        f,
        box_rect,
        None,
        "",
        focused_box,
        app.sb_arrow_pressed(SbKey::CmdText),
        None,
        true,
        &mut app.cmd,
    );
    app.cmd_area = Some(rect);
    app.cmd_text_area = Some(rendered.inner);
    app.cmd_sb = rendered.sb;
    app.cmd_cursor_screen = rendered.cursor;

    let btn_row = inner.y + inner.height - 2;
    let label = "[ 确定 ]";
    let bw = w(label) + 2;
    let bx = inner.x as usize + ((inner.width as usize).saturating_sub(bw)) / 2;
    let btn_rect = Rect {
        x: bx as u16,
        y: btn_row,
        width: bw as u16,
        height: 1,
    };
    let focused_btn = app.cmd_focus == 1;
    f.render_widget(
        Paragraph::new(Line::from(button_spans("确定", focused_btn))),
        btn_rect,
    );
    app.cmd_btn_rect = Some(btn_rect);
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-uicmd", tag)
    }

    #[tokio::test]
    async fn renders_window_button_and_backfills_geometry() {
        let mut app = make_app("gold");
        app.open_cmd_window();
        app.cmd.set_text("http://example.com/f.bin");
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| draw_cmd(f, &mut app, f.area())).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("请输入命令或链接"), "标题: {s}");
        assert!(s.contains("确定"), "确定按钮: {s}");
        assert!(s.contains("http://example.com/f.bin"), "文本内容: {s}");
        assert!(s.contains("✗"), "右上角关闭按钮: {s}");
        let a = app.cmd_area.expect("cmd_area 回填");
        assert_eq!((a.x, a.y, a.width, a.height), (11, 7, 78, 16));
        let ta = app.cmd_text_area.expect("文本框内容区回填");
        assert_eq!((ta.width, ta.height), (70, 9), "文本框 72×11 去边框 2");
        let b = app.cmd_btn_rect.expect("按钮热区回填");
        assert_eq!(b.y, 7 + 1 + 12, "按钮行 = 内框 y + 内框高 − 2");
        assert_eq!(b.width, 10, "[ 确定 ] + 两侧 padding = 10 列");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn tiny_terminal_degrades_without_hotspots() {
        let mut app = make_app("tiny");
        app.open_cmd_window();
        app.cmd_btn_rect = Some(Rect::new(0, 0, 5, 1));
        app.cmd_text_area = Some(Rect::new(0, 0, 5, 1));
        let mut term = Terminal::new(TestBackend::new(20, 8)).unwrap();
        term.draw(|f| draw_cmd(f, &mut app, f.area())).unwrap();
        assert!(app.cmd_area.is_some(), "窗口区域仍回填");
        assert!(app.cmd_text_area.is_none() && app.cmd_btn_rect.is_none());
        app.shutdown().await;
    }
}
