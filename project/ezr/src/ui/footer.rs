//! footer — 页脚（v1.30/FR-01-107 页脚收敛，自 header.rs 按渲染区块拆出、
//! architect v116 成模块）
//!
//! 单行：` H 帮助  ·  Q 退出 `（键名青色加粗、说明暗灰）+ toast 区
//! （`◆ 消息` 黄色）。完整快捷键说明收敛 H 帮助窗口（P3 原则）——原快捷键
//! 提示行 / 鼠标提示行随页脚收敛退役（FR-01-96/98/103/106 相关条款）。

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use super::{ACCENT, BORDER, DIM, YELLOW};
use crate::app::App;

pub(super) fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(BORDER));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // 单行页脚（FR-01-107：仅保留 H 帮助 / Q 退出提示 + toast）。
    // 完整分类说明见 H 帮助窗口。
    let key = |s: &str| {
        Span::styled(
            s.to_string(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )
    };
    let desc = |s: &str| Span::styled(s.to_string(), Style::default().fg(DIM));
    let mut l1 = vec![key(" H"), desc(" 帮助  ·  "), key(" Q"), desc(" 退出  ")];
    if let Some(msg) = &app.toast {
        l1.push(Span::styled(
            format!("◆ {msg}"),
            Style::default().fg(YELLOW),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(l1)), inner);
}

#[cfg(test)]
mod tests {
    //! v1.30/FR-01-107 页脚收敛单测：只含 H 帮助 / Q 退出 + toast；
    //! 历史快捷键提示项（P 分块/G 面板/Space 暂停/鼠标提示等）全部退役。

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::super::testfx;
    use crate::app::App;

    fn footer_text(app: &App, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| {
            super::draw_footer(
                f,
                app,
                ratatui::layout::Rect {
                    x: 0,
                    y: 0,
                    width: w,
                    height: h,
                },
            )
        })
        .unwrap();
        // backend().to_string 的终端语义：宽字符占两格只印一次（续格不入文本）
        term.backend().to_string()
    }

    #[tokio::test]
    async fn footer_collapsed_to_help_quit_only() {
        let mut app = testfx::make_app_in("ezr-footer", "collapsed");
        app.set_toast("测试消息");
        let row = footer_text(&app, 120, 3);
        assert!(row.contains("H"), "H 键名: {row}");
        assert!(row.contains("帮助"), "帮助说明: {row}");
        assert!(row.contains("Q"), "Q 键名: {row}");
        assert!(row.contains("退出"), "退出说明: {row}");
        assert!(row.contains("◆ 测试消息"), "toast 区: {row}");
        // 历史项全部退役（P3 收敛，FR-01-96/98/103/106）
        for retired in [
            "分块", "面板", "暂停", "添加", "删除", "重试", "上移", "鼠标", "页签",
        ] {
            assert!(!row.contains(retired), "历史项「{retired}」应退役: {row}");
        }
        app.shutdown().await;
    }

    /// 无 toast 时仅 H 帮助 · Q 退出（◆ 前缀不出现）
    #[tokio::test]
    async fn footer_without_toast_has_no_bullet() {
        let mut app = testfx::make_app_in("ezr-footer", "notoast");
        app.toast = None;
        let row = footer_text(&app, 120, 3);
        assert!(row.contains("帮助") && row.contains("退出"));
        assert!(!row.contains('◆'), "无 toast 无 ◆: {row}");
        app.shutdown().await;
    }

    /// 窄终端不截断 H/Q 两项（收敛行长度短，80 列内完整可读）
    #[tokio::test]
    async fn footer_fits_narrow_terminal() {
        let mut app = testfx::make_app_in("ezr-footer", "narrow");
        app.set_toast("窄屏消息");
        let row = footer_text(&app, 40, 3);
        assert!(
            row.contains("帮助") && row.contains("退出"),
            "窄屏完整: {row}"
        );
        assert!(row.contains("◆ 窄屏消息"), "窄屏 toast: {row}");
        app.shutdown().await;
    }
}
