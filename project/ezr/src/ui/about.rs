use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::btn::{clip_wide_at_edges, dialog_rect, draw_close_btn};
use super::{panel_border_style, panel_title_style};
use crate::app::App;

const ABOUT_W: u16 = 40;
const ABOUT_H: u16 = 7;

pub(super) fn draw_about(f: &mut Frame, app: &mut App, area: Rect) {
    let rect = match app.about_pos {
        Some((px, py)) => {
            let dw = ABOUT_W.min(area.width);
            let dh = ABOUT_H.min(area.height);
            Rect {
                x: px.min(area.width.saturating_sub(dw)),
                y: py.min(area.height.saturating_sub(dh)),
                width: dw,
                height: dh,
            }
        }
        None => {
            let r = dialog_rect(area, ABOUT_W, ABOUT_H);
            app.about_pos = Some((r.x, r.y));
            r
        }
    };
    f.render_widget(Clear, rect);
    clip_wide_at_edges(f, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" About ", panel_title_style(true)));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    app.about_area = Some(rect);
    app.popup_close_rect = draw_close_btn(f, app, rect, super::text::w(" About "));
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let line = Line::from(Span::styled(
        "EZR Download v1.0.0",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));
    f.render_widget(
        Paragraph::new(line).alignment(Alignment::Center),
        Rect {
            x: inner.x,
            y: inner.y + inner.height / 2,
            width: inner.width,
            height: 1,
        },
    );
}

#[cfg(test)]
mod tests {

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::super::testfx;
    use crate::app::App;

    fn render_about(app: &mut App, w: u16, h: u16) -> Terminal<TestBackend> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        app.about_open = true;
        term.draw(|f| crate::ui::draw(f, app)).unwrap();
        term
    }

    #[tokio::test]
    async fn about_geometry_centered_and_version_line() {
        let mut app = testfx::make_app_in("ezr-about", "geom");
        let term = render_about(&mut app, 120, 40);
        let s = term.backend().to_string();
        assert!(s.contains(" About "), "标题 About: {s}");
        assert!(s.contains("EZR Download v1.0.0"), "版本行 D39: {s}");
        assert_eq!(
            app.about_area,
            Some(ratatui::layout::Rect::new(40, 16, 40, 7)),
            "区域回填"
        );
        assert_eq!(app.about_pos, Some((40, 16)), "首帧居中并记忆");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn about_position_memory_across_reopens() {
        let mut app = testfx::make_app_in("ezr-about", "mem");
        app.about_open = true;
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        assert!(app.about_pos.is_some(), "首帧回填位置");
        app.close_about();
        app.about_pos = Some((3, 5));
        app.open_about();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        assert_eq!(app.about_pos, Some((3, 5)), "位置跨次记忆（FR-01-111）");
        assert_eq!(
            app.about_area,
            Some(ratatui::layout::Rect::new(3, 5, 40, 7))
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn about_clamps_in_small_terminal() {
        let mut app = testfx::make_app_in("ezr-about", "clamp");
        let term = render_about(&mut app, 30, 5);
        let s = term.backend().to_string();
        assert!(s.contains(" About "), "钳制后仍渲染标题: {s}");
        assert_eq!(
            app.about_area,
            Some(ratatui::layout::Rect::new(0, 0, 30, 5)),
            "窗口钳制到终端尺寸"
        );
        app.shutdown().await;
    }
}
