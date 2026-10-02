//! header — 头部全局统计、页签与页脚快捷键提示

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Tabs};
use ratatui::Frame;

use super::text::{fmt_size, fmt_speed};
use super::{ACCENT, BORDER, DIM, DIM2, FG, MAGENTA, YELLOW};
use crate::app::{App, FILTERS};
use crate::model::TaskState;
use crate::VERSION;

pub(super) fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let dl: f64 = app
        .tasks
        .iter()
        .filter(|t| t.state == TaskState::Downloading)
        .map(|t| t.speed)
        .sum();
    let ul: f64 = app
        .tasks
        .iter()
        .filter(|t| t.upload_speed > 0.0)
        .map(|t| t.upload_speed)
        .sum();
    let threads: usize = app
        .tasks
        .iter()
        .filter(|t| t.state == TaskState::Downloading)
        .map(|t| t.thread_count())
        .sum();
    let doing = app.tasks.iter().filter(|t| !t.state.is_done()).count();
    let done = app.tasks.len() - doing;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(Line::from(vec![
            Span::styled(
                " ◆ ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "EZR Downloader",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  v{VERSION}"), Style::default().fg(DIM)),
        ]));

    let line = Line::from(vec![
        Span::styled(" 全局  ", Style::default().fg(DIM)),
        Span::styled(
            format!("↓ {}", fmt_speed(dl)),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("   ↑ {}", fmt_speed(ul)),
            Style::default().fg(MAGENTA),
        ),
        Span::styled(format!("   并发线程 {} ", threads), Style::default().fg(FG)),
        Span::styled(
            format!("  会话已下载 {}", fmt_size(app.session_bytes)),
            Style::default().fg(FG),
        ),
        Span::styled(
            format!(
                "  任务 {} · 正在下载 {} · 已完成 {}",
                app.tasks.len(),
                doing,
                done
            ),
            Style::default().fg(DIM),
        ),
    ]);
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(Paragraph::new(line), inner);
}

pub(super) fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let doing = app.tasks.iter().filter(|t| !t.state.is_done()).count();
    let done = app.tasks.len() - doing;
    let counts = [doing, done];
    let titles: Vec<Line> = FILTERS
        .iter()
        .zip(counts.iter())
        .map(|(label, c)| Line::from(format!(" {} ({}) ", label, c)))
        .collect();

    let tabs = Tabs::new(titles)
        .select(app.filter)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::styled("│", Style::default().fg(DIM2)))
        .style(Style::default().fg(FG));

    let slots = app.used_slots();
    let slots_full = slots >= app.max_slots;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(Span::styled(" 任务队列 ", Style::default().fg(BORDER)))
        // 右侧：下载槽位占用（已满时黄色提醒：等待任务需排队）
        .title(
            Line::from(Span::styled(
                format!("下载槽位 {}/{} ", slots, app.max_slots),
                Style::default()
                    .fg(if slots_full { YELLOW } else { DIM })
                    .add_modifier(if slots_full {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ))
            .alignment(Alignment::Right),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(tabs, inner);
}

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
        desc(" 图表  "),
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

// ---------------------------------------------------------------------------
// 对话框
// ---------------------------------------------------------------------------
