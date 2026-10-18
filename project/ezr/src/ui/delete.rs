//! delete — 删除任务对话框（三选按钮 + 提示；v1.30：绿色边框——危险语义由
//! 正文与按钮承载，不用红色边框，对齐 demo 重构版）

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use super::btn::{clip_wide_at_edges, draw_button_row, overlay_rect};
use super::text::truncate;
use super::{panel_border_style, panel_title_style, DIM, DIM2};
use crate::app::App;

pub(super) fn draw_delete_dialog(f: &mut Frame, app: &mut App, area: Rect) {
    // v133/D38：拖拽位置优先（钳制终端内），首帧居中并记忆；区域热区同步
    // 回填（鼠标非交互区拖拽起点命中测试）——与任务对话框同口径
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
    // 删除框边框统一为高亮绿（模态浮层与主面板聚焦色统一，v1.30/FR-01-108；
    // 危险语义由正文警示文案/按钮色承载）
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" 删除任务 ", panel_title_style(true)));
    let inner = block.inner(dlg);
    f.render_widget(block, dlg);
    if inner.width < 40 || inner.height < 5 {
        return;
    }

    let (focus, task_name) = {
        let d = app.dialog.as_ref().unwrap();
        (d.focus, d.task_name.clone())
    };

    // 提示信息
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" 删除任务 ", Style::default().fg(DIM)),
            Span::styled(
                format!(
                    "「{}」",
                    truncate(&task_name, (inner.width as usize).saturating_sub(22))
                ),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ？", Style::default().fg(DIM)),
        ])),
        Rect {
            x: inner.x + 1,
            y: inner.y + 1,
            width: inner.width.saturating_sub(2),
            height: 1,
        },
    );

    // 按钮行：[ 仅删除任务 ] [ 删除任务和文件 ] [ 取消 ]
    draw_button_row(
        f,
        app,
        inner,
        inner.y + 3,
        &[("仅删除任务", 0), ("删除任务和文件", 1), ("取消", 2)],
        focus,
    );

    // 提示行（v1.30/FR-01-25：全部浮层中唯一保留的操作提示行——本弹窗操作
    // 为一次性确认，无需记忆；文案逐字 ` 1/2/3 快捷选择 · Enter 确认 · Esc 取消`）
    f.render_widget(
        Paragraph::new(Span::styled(
            " 1/2/3 快捷选择 · Enter 确认 · Esc 取消",
            Style::default().fg(DIM2),
        )),
        Rect {
            x: inner.x + 1,
            y: inner.y + 4,
            width: inner.width.saturating_sub(2),
            height: 1,
        },
    );
}

#[cfg(test)]
mod tests {
    //! v1.30/FR-01-25 删除对话框渲染金样：绿框、三钮、提示行逐字（唯一保留）；
    //! 数字键 1/2/3 直选与 Space=取消见 app/dialog_keys 既有单测（行为不变）。

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::super::testfx;

    #[tokio::test]
    async fn delete_dialog_green_border_and_hint() {
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
        // 提示行逐字（唯一保留）
        assert!(
            s.contains("1/2/3 快捷选择 · Enter 确认 · Esc 取消"),
            "提示行: {s}"
        );
        // 边框绿色加粗（66×7 居中 → 100×30 下 x=17、y=11）
        let buf = term.backend().buffer();
        let c = buf.cell((17, 11)).unwrap();
        assert_eq!(c.fg, super::super::FOCUS_BORDER, "删除框边框聚焦绿");
        assert!(
            c.modifier.contains(ratatui::style::Modifier::BOLD),
            "边框加粗"
        );
        app.shutdown().await;
    }
}
