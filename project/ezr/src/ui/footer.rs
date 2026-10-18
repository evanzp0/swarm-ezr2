//! footer — 页脚快捷键提示与 toast 行（自 header.rs 按渲染区块拆出，
//! architect v116：头部仪表面板 / 流量图 / 页脚三区块各自成模块）

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::{bordered_block, ACCENT, BORDER, DIM, YELLOW};
use crate::app::App;

pub(super) fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let block = bordered_block(BORDER);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(inner);

    // 快捷键行
    let key = |s: &str| {
        Span::styled(
            s.to_string(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )
    };
    let desc = |s: &str| Span::styled(s.to_string(), Style::default().fg(DIM));
    let mut l1 = vec![
        key(" ↑↓"),
        desc(" 选择  "),
        key("Space"),
        desc(" 暂停/继续  "),
        key("R"),
        desc(" 重试  "),
        // v1.29/FR-01-106 ⑨：P 分块（位于「R 重试」与「A 添加」之间，
        // key 青色加粗 + 说明暗灰，与其他项同式样）
        key("P"),
        desc(" 分块  "),
        key("A"),
        desc(" 添加  "),
        key("D"),
        desc(" 删除  "),
        key("U/J"),
        desc(" 上移/下移  "),
        key("C"),
        desc(" 清已完成  "),
        key("Tab"),
        desc(" 页签  "),
        key("G"),
        // FR-01-98：G 切换右栏两面板（详情/并发明细），流量图常显 → 提示「面板」
        desc(" 面板  "),
        key("Q"),
        desc(" 退出"),
    ];
    // 窄屏精简
    if inner.width < 100 {
        l1.truncate(9);
    }
    if inner.width < 86 {
        l1.truncate(7);
    }
    if inner.width < 72 {
        l1.truncate(5);
    }
    f.render_widget(Paragraph::new(Line::from(l1)), rows[0]);

    // 鼠标提示 + toast
    let mut l2 = vec![
        Span::styled(" 鼠标 ", Style::default().fg(DIM)),
        Span::styled("点击选中 · 滚轮滚动   ", Style::default().fg(DIM)),
    ];
    if let Some(msg) = &app.toast {
        l2.push(Span::styled(
            format!("◆ {}", msg),
            Style::default().fg(YELLOW),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(l2)), rows[1]);
}

#[cfg(test)]
mod tests {
    //! FR-01-106 ⑨ 页脚 `P 分块` 项单测：位于「R 重试」与「A 添加」之间；
    //! 窄屏精简后 P 项仍在（截断位次口径不变，05-ui-pieces-08）。

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::super::testfx;
    use crate::app::App;

    fn footer_row(app: &App, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| {
            super::draw_footer(
                f,
                app,
                ratatui::layout::Rect {
                    x: 0,
                    y: 0,
                    width: w,
                    height: 4,
                },
            )
        })
        .unwrap();
        // backend().to_string 的终端语义：宽字符占两格只印一次（续格不入文本）
        term.backend().to_string()
    }

    #[tokio::test]
    async fn footer_has_p_pieces_between_retry_and_add() {
        let app = testfx::make_app_in("ezr-footer", "p");
        let row = footer_row(&app, 120, 4);
        assert!(row.contains("P 分块"), "页脚含 P 分块：{row}");
        let (ri, pi, ai) = (
            row.find("重试").expect("R 重试在位"),
            row.find("分块").expect("分块在位"),
            row.find("添加").expect("A 添加在位"),
        );
        assert!(ri < pi && pi < ai, "P 分块 位于 R 重试 与 A 添加 之间");
    }

    #[tokio::test]
    async fn footer_keeps_p_item_after_narrow_truncation() {
        let app = testfx::make_app_in("ezr-footer", "pnarrow");
        // 80 列：内框 78 < 100/< 86 档位逐级截断（位次口径不变）——P 键仍可见
        let row = footer_row(&app, 80, 4);
        assert!(row.contains('P'), "窄屏精简后 P 键仍在：{row}");
        assert!(row.contains("重试"), "截断首项 R 重试 仍可见：{row}");
        let ri = row.find("重试").unwrap();
        let pi = row.rfind('P').unwrap();
        assert!(ri < pi, "P 项在 R 重试之后（截断不越位）");
    }
}
