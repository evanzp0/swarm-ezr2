use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use super::{
    panel_border_style, panel_title_style, ACCENT, DIM, DIM2, FG, FOCUS_BORDER, GREEN, SEL_BG,
    YELLOW,
};
use crate::scrollbar::{Arrow as SbArrow, Bar as SbBar};
use crate::ui::scrollable_panel::{ScrollablePanel, ScrollbarMode};
use crate::ui::text::{fmt_size, truncate, w};

pub struct TreeRow {
    pub depth: usize,

    pub is_dir: bool,

    pub name: String,

    pub size: u64,

    pub selected: bool,

    pub partial: bool,

    pub expanded: bool,
}

pub struct TreePanelRendered {
    pub inner: Rect,

    pub sb: Option<SbBar>,

    pub row_rects: Vec<(Rect, usize)>,
}

pub(crate) fn draw_tree_panel(
    f: &mut Frame,
    rect: Rect,
    title: &str,
    caption: &str,
    focused: bool,
    pressed_v: Option<SbArrow>,
    rows: &[TreeRow],
    cursor: usize,
    scroll: usize,
    visible: usize,
) -> TreePanelRendered {
    let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
    sb.thumb(if focused { FOCUS_BORDER } else { super::BORDER })
        .pressed_v(pressed_v);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(focused))
        .title(Span::styled(title.to_string(), panel_title_style(focused)));
    let inner = sb.render_block(f, rect, block);

    let cap_text = format!(" {caption} ");
    let cap_w = w(&cap_text) as u16;
    if rect.width > cap_w + 2 {
        let cap_rect = Rect {
            x: rect.x + rect.width - 1 - cap_w,
            y: rect.y + rect.height - 1,
            width: cap_w,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(cap_text, Style::default().fg(DIM)))),
            cap_rect,
        );
    }

    let mut row_rects = Vec::new();
    for vi in 0..visible.min(inner.height as usize) {
        let i = scroll + vi;
        let Some(snap) = rows.get(i) else { break };
        let y = inner.y + vi as u16;
        let is_cursor = i == cursor;
        let row_rect = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };
        let content_w = (row_rect.width as usize).saturating_sub(4);
        let indent = "  ".repeat(snap.depth.min(8));
        let (mark, mark_color) = if snap.is_dir {
            if snap.selected {
                ("[x]", GREEN)
            } else if snap.partial {
                ("[-]", YELLOW)
            } else {
                ("[ ]", DIM2)
            }
        } else if snap.selected {
            ("[x]", GREEN)
        } else {
            ("[ ]", DIM2)
        };
        let head_w = w(&indent) + 4 + 1 + usize::from(snap.is_dir) * 2;
        let size_str = fmt_size(snap.size);
        let name_disp = if snap.is_dir {
            format!("{}/", snap.name)
        } else {
            snap.name.clone()
        };
        let name_budget = content_w.saturating_sub(head_w + w(&size_str)).max(4);
        let name = truncate(&name_disp, name_budget);
        let pad = content_w.saturating_sub(head_w + w(&name) + w(&size_str));
        let mut spans = Vec::new();
        spans.push(Span::raw("  "));
        spans.push(Span::styled(indent.clone(), Style::default().fg(DIM2)));
        spans.push(Span::styled(
            mark.to_string(),
            Style::default().fg(mark_color).add_modifier(if is_cursor {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
        ));
        spans.push(Span::styled(" ", Style::default()));
        if snap.is_dir {
            spans.push(Span::styled(
                if snap.expanded { "▾ " } else { "▸ " },
                Style::default().fg(if is_cursor { Color::White } else { ACCENT }),
            ));
        }
        spans.push(Span::styled(
            name.clone(),
            Style::default()
                .fg(if is_cursor { Color::White } else { FG })
                .add_modifier(if snap.is_dir || is_cursor {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ));
        spans.push(Span::styled(
            format!("{}{}", " ".repeat(pad), size_str),
            Style::default().fg(if is_cursor { Color::White } else { DIM }),
        ));
        let mut para = Paragraph::new(Line::from(spans));
        if is_cursor {
            para = para.style(Style::default().bg(SEL_BG));
        }
        f.render_widget(para, row_rect);
        row_rects.push((row_rect, i));
    }

    sb.vertical(rows.len(), visible.max(1), scroll);
    let bar = sb.render_bars(f, rect).vertical;

    TreePanelRendered {
        inner,
        sb: bar,
        row_rects,
    }
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;

    fn row(name: &str, depth: usize, is_dir: bool, size: u64, selected: bool) -> TreeRow {
        TreeRow {
            depth,
            is_dir,
            name: name.to_string(),
            size,
            selected,
            partial: false,
            expanded: false,
        }
    }

    #[test]
    fn renders_marks_caption_and_row_rects() {
        let rows = vec![
            row("tor", 0, true, 12, false),
            row("a.txt", 1, false, 5, true),
        ];
        let mut term = Terminal::new(TestBackend::new(60, 20)).unwrap();
        let mut rendered_cell: Option<TreePanelRendered> = None;
        term.draw(|f| {
            rendered_cell = Some(draw_tree_panel(
                f,
                Rect::new(2, 2, 40, 7),
                " 选择下载文件 ",
                "已选 1 个文件，共 5 B",
                true,
                None,
                &rows,
                1,
                0,
                5,
            ));
        })
        .unwrap();
        let rendered = rendered_cell.expect("渲染产物");
        let s = term.backend().to_string();
        assert!(s.contains("选择下载文件"), "左上标题: {s}");
        assert!(s.contains("已选 1 个文件，共 5 B"), "右下统计: {s}");
        assert!(s.contains("tor/"), "目录名带尾斜杠: {s}");
        assert!(s.contains("a.txt"));
        assert!(s.contains("[x]"), "选中标记: {s}");
        assert!(s.contains("[ ]"), "未选标记: {s}");
        assert_eq!(rendered.row_rects.len(), 2, "逐行热区");
        assert!(rendered.sb.is_none(), "2 行不超 5 可视行无滚动条");
    }
}
