//! about — 关于窗口（O 快捷键，v1.32/FR-01-111）渲染层
//!
//! 对齐 ezr-demo m3 draw_about：40×7 圆角模态浮层（小终端钳制）、绿色加粗
//! 边框（与主面板聚焦色统一）、标题 ` About `；内容 = 内框垂直居中一行
//! `EZR Download v1.0.0`（白色加粗水平居中；文案照录 demo 钦定，D39——
//! 不随期号变体，期号/产品版本号差异化策略留操作者裁决）；无滚动条、
//! 无滚轮内容；支持鼠标拖拽移动（位置跨次记忆）；z-order 最上层（叠于
//! 帮助之上）。

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::btn::{clip_wide_at_edges, dialog_rect, draw_close_btn};
use super::{panel_border_style, panel_title_style};
use crate::app::App;

/// 关于窗口固定尺寸（宽×高，含边框；FR-01-111 ①）
const ABOUT_W: u16 = 40;
/// 关于窗口固定高度（含边框）
const ABOUT_H: u16 = 7;

/// 关于窗口（O）：模态浮层——绿色高亮边框（与主面板聚焦色统一）；内容区
/// 垂直居中一行 `EZR Download v1.0.0`（白色加粗）；支持鼠标拖拽移动（位置
/// 记忆跨次保留）；Esc/O/q/Q 关闭；点击窗口外部 = 无任何操作（模态统一
/// 口径，FR-01-108 v1.32 注记）。
pub(super) fn draw_about(f: &mut Frame, app: &mut App, area: Rect) {
    // 可拖拽定位：记忆位置优先（钳制在终端内），首帧居中并回填
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
    // 浮层边界 CJK 宽字符切割处理（残影根治点，与对话框同口径）
    clip_wide_at_edges(f, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" About ", panel_title_style(true)));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    app.about_area = Some(rect);
    // 右上角 [✗]（v1.34/FR-01-114：全部弹窗统一；原地松开 = 关闭关于窗口）
    app.popup_close_rect = draw_close_btn(f, app, rect, super::text::w(" About "));
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    // 内容：居中一行版本标识（D39：文案照录 demo 钦定 `EZR Download v1.0.0`）
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
    //! About 窗口渲染单测（FR-01-111）：40×7 几何与居中、文案 D39、
    //! 位置跨次记忆、小终端钳制。

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
        // 40×7 居中：120 宽 → x=40、40 高 → y=16（(40−7)/2 = 16）
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
        // 模拟拖拽到新位置（关闭不清位置记忆）
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
