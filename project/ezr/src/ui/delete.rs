//! delete — 删除任务对话框（三选按钮；v1.30：绿色边框——危险语义由
//! 正文与按钮承载，不用红色边框，对齐 demo 重构版；v1.34/FR-01-25 修订：
//! 底部提示行退役——P10 原则定版「窗口不需要有提示信息」，数字直选/Space
//! 交互语义不变，快捷键说明收敛帮助窗口【弹窗】组；FR-01-115：正文行 x+2
//! 宽 −4（S2），尺寸 66×7 维持（v1.30 已补齐：内框 64×5 = 顶 1 + 正文 1 +
//! 空 1 + 按钮 1 + 底 1）；FR-01-114：右上角 [✗]（原地松开 = 取消））

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
    // v133/D38：拖拽位置优先（钳制终端内），首帧居中并记忆；区域热区同步
    // 回填（鼠标非交互区拖拽起点命中测试）——与任务对话框同口径；
    // 66×7（v1.30 padding 统一后口径不变：内框 64×5）
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
    // 右上角 [✗]（v1.34/FR-01-114：全部弹窗统一；原地松开 = 取消删除对话框）
    app.popup_close_rect = draw_close_btn(f, app, dlg, w(" 删除任务 "));
    if inner.width < 40 || inner.height < 5 {
        return;
    }

    let (focus, task_name) = {
        let d = app.dialog.as_ref().unwrap();
        (d.focus, d.task_name.clone())
    };

    // 提示信息（正文行；v1.34/FR-01-115 S2：内容区左 2 列 padding，x+2 宽 −4；
    // 文本不再自带前导空格）
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

    // 按钮行：[ 仅删除任务 ] [ 删除任务和文件 ] [ 取消 ]（y 不变，仅底部补齐 padding）
    draw_button_row(
        f,
        app,
        inner,
        inner.y + 3,
        &[("仅删除任务", 0), ("删除任务和文件", 1), ("取消", 2)],
        focus,
    );

    // v1.34/FR-01-25（P10 原则定版）：底部提示行 ` 1/2/3 快捷选择 · Enter 确认
    // · Esc 取消` 删除——原「全部浮层中唯一保留的提示行」例外条款随此退役
    //（至此全部 panel/弹窗内容区无提示信息；数字直选/Space=取消/Enter 交互
    // 语义不变，快捷键说明收敛帮助窗口【弹窗】组）
}

#[cfg(test)]
mod tests {
    //! v1.34/FR-01-25 删除对话框渲染金样：绿框、三钮、提示行退役（反向断言）；
    //! 数字键 1/2/3 直选与 Space=取消见 app/dialog_keys 既有单测（行为不变）。

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
        // v1.34/FR-01-25：提示行退役（反向断言）
        assert!(!s.contains("1/2/3 快捷选择"), "提示行已删除（P10）: {s}");
        // 右上角 [✗]（FR-01-114：66×7 居中 → 100×30 下 x=17、y=11；按钮在
        // 顶边框行右端 = x 17+66-5=78 起 3 列）
        let buf = term.backend().buffer();
        // [✗] 热区 = bx 起 3 列：bx = x + 宽 − 5 = 17+66−5 = 78（[ 在 78、✗ 79、] 80）
        let x_sym = buf.cell((79, 11)).unwrap();
        assert_eq!(x_sym.symbol(), "✗", "[✗] 按钮: {s}");
        assert_eq!(buf.cell((78, 11)).unwrap().symbol(), "[", "左括号");
        assert_eq!(buf.cell((80, 11)).unwrap().symbol(), "]", "右括号");
        // 边框绿色加粗
        let c = buf.cell((17, 11)).unwrap();
        assert_eq!(c.fg, super::super::FOCUS_BORDER, "删除框边框聚焦绿");
        assert!(
            c.modifier.contains(ratatui::style::Modifier::BOLD),
            "边框加粗"
        );
        app.shutdown().await;
    }
}
