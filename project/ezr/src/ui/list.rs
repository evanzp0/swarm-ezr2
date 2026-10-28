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
    anim: u64,
) {
    let text = task_lines(
        t,
        sel,
        spinner,
        (area.width as usize).saturating_sub(2),
        queue_pos,
        anim,
    );
    let para = Paragraph::new(text).style(if sel {
        Style::default().bg(SEL_BG)
    } else {
        Style::default()
    });
    f.render_widget(para, area);
}

pub(super) fn draw_list(f: &mut Frame, app: &mut App, area: Rect) {
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
    let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
    sb.thumb(if focused { FOCUS_BORDER } else { BORDER })
        .pressed_v(app.sb_arrow_pressed(SbKey::List));
    let inner = sb.render_block(f, area, block);

    let idxs = app.filtered();
    let rows = inner.height as usize / ITEM_HEIGHT as usize;

    if rows == 0 || idxs.is_empty() {
        let msg = if idxs.is_empty() {
            "（此页签下没有任务，按 A 打开命令窗口添加任务）"
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
        render_task(
            f,
            t,
            row_area,
            ti == app.selected,
            spinner,
            qpos[ti],
            app.frame,
        );
    }

    app.visible_rows = max_items;
    app.list_area = Some(inner);

    let total = idxs.len();
    sb.vertical(total, rows, app.scroll);
    app.list_scrollbar = sb.render_bars(f, area).vertical;
}
