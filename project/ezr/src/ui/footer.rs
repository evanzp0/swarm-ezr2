//! footer — 页脚快捷键提示与 toast 行（自 header.rs 按渲染区块拆出，
//! architect v116：头部仪表面板 / 流量图 / 页脚三区块各自成模块）

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use super::{ACCENT, BORDER, DIM, YELLOW};
use crate::app::App;

pub(super) fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER));
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
