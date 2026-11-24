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
    queue_pos: usize,
    anim: u64,
) {
    let text = task_lines(
        t,
        sel,
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

    if app.selected >= idxs.len() {
        app.selected = idxs.len() - 1;
        app.scroll_to_selection();
    }
    let max_scroll = idxs.len().saturating_sub(rows);
    if app.scroll > max_scroll {
        app.scroll = max_scroll;
    }
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
        render_task(f, t, row_area, ti == app.selected, qpos[ti], app.frame);
    }

    app.visible_rows = max_items;
    app.list_area = Some(inner);

    let total = idxs.len();
    sb.vertical(total, rows, app.scroll);
    app.list_scrollbar = sb.render_bars(f, area).vertical;
}

#[cfg(test)]
mod draw_list_tests {
    use ratatui::backend::TestBackend;
    use ratatui::prelude::Rect;
    use ratatui::Terminal;

    use super::*;
    use crate::app::testutil::{dl_task, make_app_in};

    #[tokio::test]
    async fn draw_list_tiny_area_renders_area_too_small_hint() {
        let mut app = make_app_in("ezr-list", "tiny");
        app.tasks.push(dl_task(1, "t.bin"));
        app.selected = 0;
        let mut term = Terminal::new(TestBackend::new(50, 5)).unwrap();
        let area = Rect::new(0, 0, 50, 5);
        term.draw(|f| draw_list(f, &mut app, area)).unwrap();
        let s = term.backend().to_string();
        assert!(
            s.contains("（区域过小）"),
            "行高不足 4 的区域渲染过小提示: {s}"
        );
        assert_eq!(app.visible_rows, 0, "过小区域可见行数为 0");
        assert!(app.list_area.is_some(), "过小区域仍回填列表区域");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn draw_list_clamps_selected_out_of_range() {
        let mut app = make_app_in("ezr-list", "clamp");
        app.tasks.push(dl_task(1, "a.bin"));
        app.tasks.push(dl_task(2, "b.bin"));
        app.selected = 9;
        let mut term = Terminal::new(TestBackend::new(60, 14)).unwrap();
        let area = Rect::new(0, 0, 60, 14);
        term.draw(|f| draw_list(f, &mut app, area)).unwrap();
        assert_eq!(
            app.selected, 1,
            "选择越界时钳制到末项并滚动跟随（L78-79 臂）"
        );
        assert!(app.visible_rows > 0, "正常区域可见行数为正");
        assert!(
            term.backend().to_string().contains("b.bin"),
            "钳制后渲染末项"
        );
        app.shutdown().await;
    }
}
