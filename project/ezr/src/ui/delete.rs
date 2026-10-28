use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use super::btn::{clip_wide_at_edges, draw_button_row, draw_close_btn, overlay_rect};
use super::text::{truncate, w};
use super::{panel_border_style, panel_title_style, DIM};
use crate::app::App;

pub(super) fn draw_delete_dialog(f: &mut Frame, app: &mut App, area: Rect) {
    let dlg = {
        let r = overlay_rect(area, 66, 7, app.dlg_pos);
        if app.dlg_pos.is_none() {
            app.dlg_pos = Some((r.x, r.y));
        }
        app.dlg_area = Some(r);
        r
    };
    f.render_widget(Clear, dlg);
    clip_wide_at_edges(f, dlg);
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" 删除任务 ", panel_title_style(true)));
    let inner = block.inner(dlg);
    f.render_widget(block, dlg);
    app.popup_close_rect = draw_close_btn(f, app, dlg, w(" 删除任务 "));
    if inner.width < 40 || inner.height < 5 {
        return;
    }

    let (focus, task_name) = {
        let d = app.dialog.as_ref().unwrap();
        (d.focus, d.task_name.clone())
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("删除任务 ", Style::default().fg(DIM)),
            Span::styled(
                format!(
                    "「{}」",
                    truncate(&task_name, (inner.width as usize).saturating_sub(22))
                ),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("？", Style::default().fg(DIM)),
        ])),
        Rect {
            x: inner.x + 2,
            y: inner.y + 1,
            width: inner.width.saturating_sub(4),
            height: 1,
        },
    );

    draw_button_row(
        f,
        app,
        inner,
        inner.y + 3,
        &[("仅删除任务", 0), ("删除任务和文件", 1), ("取消", 2)],
        focus,
    );
}

#[cfg(test)]
mod tests {

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::super::testfx;

    #[tokio::test]
    async fn delete_dialog_green_border_and_hint_retired() {
        let mut app = testfx::make_app_in("ezr-del", "green");
        app.open_add_dialog();
        let t = testfx::task_in(9, "todelete.bin", crate::model::TaskState::Paused);
        app.tasks.push(t);
        app.open_delete_dialog();
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("删除任务"), "删除对话框: {s}");
        assert!(s.contains("仅删除任务"), "按钮1: {s}");
        assert!(s.contains("删除任务和文件"), "按钮2: {s}");
        assert!(s.contains("取消"), "按钮3: {s}");
        assert!(!s.contains("1/2/3 快捷选择"), "提示行已删除（P10）: {s}");
        let buf = term.backend().buffer();
        let x_sym = buf.cell((79, 11)).unwrap();
        assert_eq!(x_sym.symbol(), "✗", "[✗] 按钮: {s}");
        assert_eq!(buf.cell((78, 11)).unwrap().symbol(), "[", "左括号");
        assert_eq!(buf.cell((80, 11)).unwrap().symbol(), "]", "右括号");
        let c = buf.cell((17, 11)).unwrap();
        assert_eq!(c.fg, super::super::FOCUS_BORDER, "删除框边框聚焦绿");
        assert!(
            c.modifier.contains(ratatui::style::Modifier::BOLD),
            "边框加粗"
        );
        app.shutdown().await;
    }
}
