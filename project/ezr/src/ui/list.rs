//! list — 任务列表区渲染（条目三行 + 列表块；v1.30/FR-01-108 聚焦边框 +
//! v1.32/FR-01-109 通用滚动条组件 ScrollablePanel Vertical 实例）

use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::scrollable_panel::{ScrollablePanel, ScrollbarMode};
use super::task_lines::task_lines;
use super::{panel_border_style, panel_title_style, BORDER, DIM, FOCUS_BORDER, SEL_BG};
use crate::app::{App, Panel, SbKey, ITEM_HEIGHT};
use crate::model::{slots, Task};

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
    // 聚焦边框（v1.30/FR-01-108）：列表聚焦且无浮层 → 绿色加粗，否则暗色
    let focused = app.focus == Panel::List && !app.overlay_open();
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(focused))
        .title(Line::from(vec![Span::styled(
            " 下载任务 ",
            panel_title_style(focused),
        )]))
        .title(
            Line::from(Span::styled(
                format!(" {}/{} 项 ", app.selected + 1, app.filtered().len()),
                Style::default().fg(DIM),
            ))
            .alignment(Alignment::Right),
        );
    // v1.32/FR-01-109：ScrollablePanel 一体化组件（任务列表 = Vertical 实例）。
    // 滑块颜色 = 面板边框色（聚焦绿加粗/未聚焦暗）；箭头按压高亮随 App 态。
    let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
    sb.thumb(if focused { FOCUS_BORDER } else { BORDER })
        .pressed_v(app.sb_arrow_pressed(SbKey::List));
    let inner = sb.render_block(f, area, block);

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
    // 排队位次：按列表顺序从上往下，等待槽位的任务依次编号（1 基；已获槽位/非等待 = 0）。
    // 编号规则单源 model::slots::queue_positions（architect v116：原渲染层内联重实现）
    let qpos = slots::queue_positions(&app.tasks, &idxs);
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

    // 右缘滚动条（v1.32/FR-01-109：ScrollablePanel 组件收口——显示条件
    // （total > rows）由组件内判定；滑块色随面板焦点）
    let total = idxs.len();
    sb.vertical(total, rows, app.scroll);
    app.list_scrollbar = sb.render_bars(f, area).vertical;
}
