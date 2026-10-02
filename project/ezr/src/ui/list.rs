//! list — 任务列表区渲染（条目三行 + 列表块）

use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, ITEM_HEIGHT};
use crate::model::{Task, TaskState};

use super::task_lines::task_lines;
use super::{ACCENT, BORDER, DIM, DIM2, SEL_BG};

pub(super) fn render_task(
    f: &mut Frame,
    t: &Task,
    area: Rect,
    sel: bool,
    spinner: char,
    queue_pos: usize,
) {
    let text = task_lines(t, sel, spinner, area.width as usize, queue_pos);
    let para = Paragraph::new(text).style(if sel {
        Style::default().bg(SEL_BG)
    } else {
        Style::default()
    });
    f.render_widget(para, area);
}

// ---------------------------------------------------------------------------
// 各区块渲染
// ---------------------------------------------------------------------------

pub(super) fn draw_list(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(Line::from(vec![
            Span::styled(" 下载任务 ", Style::default().fg(ACCENT)),
            Span::styled("（鼠标: 点击选中 / 滚轮滚动） ", Style::default().fg(DIM2)),
        ]))
        .title(
            Line::from(Span::styled(
                format!(" {}/{} 项 ", app.selected + 1, app.filtered().len()),
                Style::default().fg(DIM),
            ))
            .alignment(Alignment::Right),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);

    let idxs = app.filtered();
    let rows = inner.height as usize / ITEM_HEIGHT as usize;

    if rows == 0 || idxs.is_empty() {
        let msg = if idxs.is_empty() {
            // 空态提示（Gherkin 01-tui-display-13：显示「按 A 添加下载任务」）
            "（此页签下没有任务，按 A 添加下载任务）"
        } else {
            "（区域过小）"
        };
        f.render_widget(
            Paragraph::new(Span::styled(msg, Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        app.visible_rows = 0;
        app.list_area = Some(inner);
        return;
    }

    let spinner = app.spinner();
    let max_items = rows.min(idxs.len().saturating_sub(app.scroll));
    // 排队位次：按列表顺序从上往下，等待槽位的任务依次编号（1 基；已获槽位/非等待 = 0）
    let mut waiting = 0usize;
    let qpos: Vec<usize> = idxs
        .iter()
        .map(|&ti| {
            let t = &app.tasks[ti];
            if t.state == TaskState::Queued && !t.has_slot {
                waiting += 1;
                waiting
            } else {
                0
            }
        })
        .collect();
    for vi in 0..max_items {
        let ti = app.scroll + vi;
        let t = &app.tasks[idxs[ti]];
        let row_area = Rect {
            x: inner.x,
            y: inner.y + (vi * ITEM_HEIGHT as usize) as u16,
            width: inner.width,
            height: ITEM_HEIGHT,
        };
        render_task(f, t, row_area, ti == app.selected, spinner, qpos[ti]);
    }

    app.visible_rows = max_items;
    app.list_area = Some(inner);
}
