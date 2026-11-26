use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::{chunk_size_label, App, DetailField, Panel, TaskState, MAX_DOWNLOAD_SLOTS};
use crate::scrollable_panel::{ScrollablePanel, ScrollbarMode};

use super::*;

pub(crate) fn draw_detail(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Panel::Detail && !overlay_open(app);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(focused))
        .title(Span::styled(" 任务详情 ", panel_title_style(focused)));

    let sb = ScrollablePanel::new(ScrollbarMode::None);
    let inner = sb.render_block(f, area, block);

    app.detail_area = Some(inner);

    let Some(t) = app.sel_task() else {
        app.detail_url_rect = None;
        app.detail_ck_rect = None;
        app.detail_fail_rect = None;
        app.detail_save_rect = None;
        app.detail_val_rects = Vec::new();
        f.render_widget(
            Paragraph::new(Span::styled("（未选中任务）", Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    };

    let label = |s: &str| Span::styled(pad_right(s, 9), Style::default().fg(DIM));

    let label_link = |s: &str| -> Vec<Span<'static>> {
        vec![
            Span::styled(
                s.to_string(),
                Style::default()
                    .fg(LIGHT_BLUE)
                    .add_modifier(Modifier::UNDERLINED),
            ),
            Span::raw(" ".repeat(9 - w(s))),
        ]
    };
    let val = |s: String| Span::styled(s, Style::default().fg(FG));
    let state_c = state_color(t.state);

    let hoverable = !overlay_open(app);
    let val_rect = |idx: usize, vw: usize| -> Option<Rect> {
        let y = inner.y + idx as u16;
        (y < inner.y + inner.height).then(|| Rect {
            x: inner.x + 11,
            y,
            width: vw.min(inner.width.saturating_sub(11) as usize) as u16,
            height: 1,
        })
    };
    let mut val_rects: Vec<(Rect, DetailField)> = Vec::new();

    let mut val_hover = |idx: usize, vw: usize, field: DetailField| -> bool {
        match val_rect(idx, vw) {
            Some(r) => {
                let hv = hoverable && hovered(app.pointer, r);
                val_rects.push((r, field));
                hv
            }
            None => false,
        }
    };

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            truncate(&t.name, (inner.width as usize).saturating_sub(4)),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    if t.state == TaskState::Queued {
        let vtxt = match app.queue_pos_of(t.id) {
            Some(p) => format!(
                "排队第 {} 位（{}/{}）· Space 暂停",
                p,
                app.used_slots(),
                MAX_DOWNLOAD_SLOTS
            ),
            None => "已获得下载槽位，即将开始下载".to_string(),
        };
        lines.push(Line::from(vec![
            Span::raw("  "),
            label("排队"),
            Span::styled(
                truncate(&vtxt, (inner.width as usize).saturating_sub(13)),
                Style::default().fg(YELLOW),
            ),
        ]));
    }
    lines.push(Line::from(vec![
        Span::raw("  "),
        label("ID"),
        Span::styled(format!("{id}", id = t.id), Style::default().fg(DIM)),
    ]));

    lines.push(Line::from(vec![
        Span::raw("  "),
        label("创建"),
        Span::styled(t.created.clone(), Style::default().fg(DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  "),
        label("类型"),
        Span::styled(
            t.protocol.label().to_string(),
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ),
        {
            let resume_txt = if t.resumable {
                "支持断点续传"
            } else {
                "不支持断点续传"
            };
            val(format!(" · {}", resume_txt))
        },
    ]));

    let mut ck_line_idx: Option<usize> = None;
    if let Some(ck) = &t.checksum {
        let vshort: String = if ck.value.chars().count() > 10 {
            format!("{}…", ck.value.chars().take(10).collect::<String>())
        } else {
            ck.value.clone()
        };
        let (st, stc) = match (t.verify_ok, t.state) {
            (Some(true), _) => ("（已通过）", GREEN),
            (Some(false), _) => ("（未通过）", RED),
            (None, TaskState::Verifying) => ("（校验中）", LIGHT_BLUE),
            _ => ("（待校验）", DIM),
        };
        ck_line_idx = Some(lines.len());

        let algo_s = ck.algo.to_string();
        let vw = w(&algo_s) + 3 + w(&vshort) + w(st);
        let hv = val_hover(lines.len(), vw, DetailField::Checksum);
        let mut ck_line = vec![Span::raw("  ")];
        ck_line.extend(label_link("校验"));
        ck_line.push(Span::styled(
            algo_s,
            with_underline(Style::default().fg(YELLOW).add_modifier(Modifier::BOLD), hv),
        ));
        ck_line.push(Span::styled(
            format!(" · {}", vshort),
            with_underline(Style::default().fg(FG), hv),
        ));
        ck_line.push(Span::styled(
            st,
            with_underline(Style::default().fg(stc), hv),
        ));
        lines.push(Line::from(ck_line));
    }
    lines.push(Line::from(vec![
        Span::raw("  "),
        label("大小"),
        val(if t.size_known() {
            format!("{} / {}", fmt_size(t.downloaded), fmt_size(t.total))
        } else {
            format!("{} / ? B", fmt_size(t.downloaded))
        }),
    ]));

    let mut fail_line_idx: Option<usize> = None;
    if t.state == TaskState::Failed {
        fail_line_idx = Some(lines.len());
        let err_s = truncate(
            t.error.as_deref().unwrap_or("未知错误"),
            (inner.width as usize).saturating_sub(13),
        );
        let hv = val_hover(lines.len(), w(&err_s), DetailField::Failed);
        let mut fail_line = vec![Span::raw("  ")];
        fail_line.extend(label_link("失败"));
        fail_line.push(Span::styled(
            err_s,
            with_underline(Style::default().fg(RED), hv),
        ));
        lines.push(Line::from(fail_line));
    }

    let save_line_idx = lines.len();
    let save_s = truncate(&t.save_path, (inner.width as usize).saturating_sub(13));
    let hv = val_hover(save_line_idx, w(&save_s), DetailField::SavePath);
    let mut save_line = vec![Span::raw("  ")];
    save_line.extend(label_link("路径"));
    save_line.push(Span::styled(
        save_s,
        with_underline(Style::default().fg(FG), hv),
    ));
    lines.push(Line::from(save_line));

    let url_line_idx = lines.len();
    let url_s = break_links(&truncate(&t.url, (inner.width as usize).saturating_sub(13)));
    let hv = val_hover(url_line_idx, w(&url_s), DetailField::Url);
    let mut url_line = vec![Span::raw("  ")];
    url_line.extend(label_link("URL"));
    url_line.extend(link_break_spans(
        &url_s,
        with_underline(Style::default().fg(FG), hv),
    ));
    lines.push(Line::from(url_line));

    {
        let (x, y, _) = t.chunk_info();
        let mut l = vec![Span::raw("  "), label("分块")];
        if t.total == 0 || y == 0 || !t.supports_pieces() {
            l.push(Span::styled("-".to_string(), Style::default().fg(DIM)));
        } else {
            l.push(Span::styled(
                format!("{}/{}", x, y),
                Style::default().fg(state_c).add_modifier(Modifier::BOLD),
            ));
            l.push(Span::styled(
                format!(" · {}/块", chunk_size_label(t.protocol)),
                Style::default().fg(FG),
            ));
        }
        lines.push(Line::from(l));
    }

    f.render_widget(Paragraph::new(lines), inner);

    let hot = |idx: usize| {
        let y = inner.y + idx as u16;
        (y < inner.y + inner.height).then(|| Rect {
            x: inner.x + 2,
            y,
            width: 9,
            height: 1,
        })
    };
    app.detail_url_rect = hot(url_line_idx);
    app.detail_ck_rect = ck_line_idx.and_then(hot);

    app.detail_fail_rect = fail_line_idx.and_then(hot);
    app.detail_save_rect = hot(save_line_idx);
    app.detail_val_rects = val_rects;
}
