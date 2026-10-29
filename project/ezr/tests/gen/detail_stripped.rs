use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::text::{break_links_deep, fmt_size, link_break_spans, pad_right, truncate, w};
use super::{
    panel_border_style, panel_title_style, state_color, DIM, FG, GREEN, LIGHT_BLUE, RED, YELLOW,
};
use crate::app::{App, DetailField, Panel};
use crate::model::chunk::fmt_block_size;
use crate::model::{Task, TaskState};

fn dim_label(s: &str) -> Span<'static> {
    Span::styled(pad_right(s, 9), Style::default().fg(DIM))
}

fn link_label(s: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            s.to_string(),
            Style::default()
                .fg(LIGHT_BLUE)
                .add_modifier(Modifier::UNDERLINED),
        ),
        Span::raw(" ".repeat(9 - w(s))),
    ]
}

fn with_underline(s: Style, on: bool) -> Style {
    if on {
        s.add_modifier(Modifier::UNDERLINED)
    } else {
        s
    }
}

fn queued_value(p: Option<usize>, used: usize, max: usize) -> String {
    match p {
        Some(pos) => format!("排队第 {pos} 位（{used}/{max}）· Space 暂停"),
        None => "已获得下载槽位，即将开始下载".to_string(),
    }
}

fn checksum_value_short(value: &str) -> String {
    if value.chars().count() > 10 {
        format!("{}…", value.chars().take(10).collect::<String>())
    } else {
        value.to_string()
    }
}

fn verify_status(verify_ok: Option<bool>, state: TaskState) -> (&'static str, Color) {
    match (verify_ok, state) {
        (Some(true), _) => ("（已通过）", GREEN),
        (Some(false), _) => ("（未通过）", RED),
        (None, TaskState::Verifying) => ("（校验中）", LIGHT_BLUE),
        _ => ("（待校验）", DIM),
    }
}

fn chunk_row(t: &Task) -> Vec<Span<'static>> {
    let state_c = state_color(t.state);
    let (x, y, plen) = t.chunk_info();
    let mut l = vec![Span::raw("  "), dim_label("分块")];
    if t.total == 0 || y == 0 || !t.supports_pieces() {
        l.push(Span::styled("-".to_string(), Style::default().fg(DIM)));
    } else {
        l.push(Span::styled(
            format!("{x}/{y}"),
            Style::default().fg(state_c).add_modifier(Modifier::BOLD),
        ));
        l.push(Span::styled(
            format!(" · {}/块", fmt_block_size(plen)),
            Style::default().fg(FG),
        ));
    }
    l
}

pub(super) fn draw_detail(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Panel::Detail && !app.overlay_open();
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(focused))
        .title(Span::styled(" 任务详情 ", panel_title_style(focused)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    app.detail_area = Some(area);

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

    let val = |s: String| Span::styled(s, Style::default().fg(FG));

    let hoverable = !app.overlay_open();
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
                let hv = hoverable
                    && app.pointer.is_some_and(|(px, py)| {
                        px >= r.x
                            && px < r.x.saturating_add(r.width)
                            && py >= r.y
                            && py < r.y.saturating_add(r.height)
                    });
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
        let vtxt = queued_value(app.queue_pos_of(t.id), app.used_slots(), app.max_slots);
        lines.push(Line::from(vec![
            Span::raw("  "),
            dim_label("排队"),
            Span::styled(
                truncate(&vtxt, (inner.width as usize).saturating_sub(13)),
                Style::default().fg(YELLOW),
            ),
        ]));
    }
    lines.push(Line::from(vec![
        Span::raw("  "),
        dim_label("ID"),
        Span::styled(format!("{id}", id = t.id), Style::default().fg(DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  "),
        dim_label("创建"),
        Span::styled(t.created.clone(), Style::default().fg(DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  "),
        dim_label("类型"),
        Span::styled(
            t.protocol.label().to_string(),
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ),
        {
            let resume_txt = if t.protocol.is_bt() || t.resumable {
                "支持断点续传"
            } else {
                "不支持断点续传"
            };
            val(format!(" · {resume_txt}"))
        },
    ]));
    let mut ck_line_idx: Option<usize> = None;
    if let Some(ck) = t.checksum.as_ref().filter(|_| !t.protocol.is_bt()) {
        let vshort = checksum_value_short(&ck.value);
        let (st, stc) = verify_status(t.verify_ok, t.state);
        ck_line_idx = Some(lines.len());
        let vw = w(ck.algo) + 3 + w(&vshort) + w(st);
        let hv = val_hover(lines.len(), vw, DetailField::Checksum);
        let mut ck_line = vec![Span::raw("  ")];
        ck_line.extend(link_label("校验"));
        ck_line.push(Span::styled(
            ck.algo.to_string(),
            with_underline(Style::default().fg(YELLOW).add_modifier(Modifier::BOLD), hv),
        ));
        ck_line.push(Span::styled(
            format!(" · {vshort}"),
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
        dim_label("大小"),
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
        fail_line.extend(link_label("失败"));
        fail_line.push(Span::styled(
            err_s,
            with_underline(Style::default().fg(RED), hv),
        ));
        lines.push(Line::from(fail_line));
    }
    let save_line_idx = lines.len();
    let save_s = truncate(&t.target_path(), (inner.width as usize).saturating_sub(13));
    let hv = val_hover(save_line_idx, w(&save_s), DetailField::SavePath);
    let mut save_line = vec![Span::raw("  ")];
    save_line.extend(link_label("路径"));
    save_line.push(Span::styled(
        save_s,
        with_underline(Style::default().fg(FG), hv),
    ));
    lines.push(Line::from(save_line));
    let url_line_idx = lines.len();
    let url_s = break_links_deep(&truncate(&t.url, (inner.width as usize).saturating_sub(13)));
    let hv = val_hover(url_line_idx, w(&url_s), DetailField::Url);
    let mut url_line = vec![Span::raw("  ")];
    url_line.extend(link_label("URL"));
    url_line.extend(link_break_spans(
        &url_s,
        with_underline(Style::default().fg(FG), hv),
    ));
    lines.push(Line::from(url_line));

    lines.push(Line::from(chunk_row(t)));

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

#[cfg(test)]
mod bt_detail_tests {

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;
    use crate::model::bt::{BtFile, BtTaskData};
    use crate::model::{Checksum, Protocol, TaskState};

    fn bt_task() -> Task {
        let mut t = crate::model::sample_task();
        t.protocol = Protocol::Bt;
        t.state = TaskState::Downloading;
        t.probed = true;
        t.total = 3_000_000;
        t.url = "/tmp/ubuntu.torrent".to_string();
        t.bt = Some(BtTaskData {
            source: "/tmp/ubuntu.torrent".to_string(),
            trackers: vec![],
            piece_length: 1_048_576,
            files: vec![BtFile {
                path: "ubuntu.iso".to_string(),
                size: 3_000_000,
                selected: true,
            }],
        });
        t.checksum = Some(Checksum {
            algo: "SHA-256",
            value: "deadbeef".to_string(),
        });
        t
    }

    #[tokio::test]
    async fn bt_detail_type_chunk_and_no_checksum_row() {
        let mut app = crate::ui::testfx::make_app_in("ezr-udetail", "bt");
        app.tasks.push(bt_task());
        app.selected = 0;
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("BT · 支持断点续传"), "类型行: {s}");
        assert!(s.contains("0/3 · 1 MB/块"), "分块行: {s}");
        let plain: String = s.chars().filter(|&c| c != '\u{FE0E}').collect();
        assert!(plain.contains("ubuntu.torrent"), "URL 行: {plain}");
        assert!(!s.contains("校验"), "无校验字段行: {s}");
        app.shutdown().await;
    }
}
