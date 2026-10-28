use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::{App, FailKind, Panel, SbKey, Task, TaskState, ITEM_HEIGHT};
use crate::scrollable_panel::{ScrollablePanel, ScrollbarMode};

use super::*;

pub(crate) fn dl_total_txt(t: &Task) -> String {
    if t.size_known() {
        format!("{}/{}", fmt_size(t.downloaded), fmt_size(t.total))
    } else {
        format!("{}/? B", fmt_size(t.downloaded))
    }
}

fn task_lines(
    t: &Task,
    sel: bool,
    spinner: char,
    width: usize,
    queue_pos: usize,
    anim: u64,
) -> Vec<Line<'static>> {
    let width = width.max(20);
    let state_c = state_color(t.state);

    let known = t.size_known();

    let pct_txt = if known {
        format!("{:>5.1}%", t.progress() * 100.0)
    } else {
        "    ?%".to_string()
    };
    let proto_badge = format!("[{}]", t.protocol.label());

    let state_badge = format!("[{}]", t.state.label());
    let cursor_w = 2usize;
    let badges_w = w(&proto_badge) + 1 + w(&state_badge) + 1;
    let pct_w = w(&pct_txt);
    let name_budget = width
        .saturating_sub(cursor_w + badges_w + pct_w + 1)
        .clamp(8, 64);
    let name_txt = truncate(&t.name, name_budget);
    let used = cursor_w + w(&name_txt) + 1 + badges_w;
    let pad = width.saturating_sub(used + pct_w);

    let l1 = vec![
        Span::styled(
            if sel { "▌ " } else { "  " }.to_string(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            name_txt,
            if sel {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG)
            },
        ),
        Span::raw(" "),
        Span::styled(proto_badge, Style::default().fg(YELLOW)),
        Span::raw(" "),
        Span::styled(state_badge, Style::default().fg(state_c)),
        Span::raw(" ".repeat(pad)),
        Span::styled(
            pct_txt,
            Style::default().fg(state_c).add_modifier(Modifier::BOLD),
        ),
    ];

    let bar_w = width.saturating_sub(4);
    let mut l2 = vec![Span::raw("  ")];
    if !known && t.state == TaskState::Downloading {
        l2.extend(marquee_bar(bar_w, anim, state_c));
    } else if known {
        l2.extend(plain_bar(bar_w, t.progress(), state_c));
    } else {
        l2.extend(plain_bar(bar_w, 0.0, state_c));
    }

    match t.state {
        TaskState::Verifying => {
            l2.push(Span::raw("  "));
            let algo = t.checksum.as_ref().map(|c| c.algo).unwrap_or("SHA-256");
            l2.push(Span::styled(
                format!("{} 分块校验 {}", algo, spinner),
                Style::default().fg(LIGHT_BLUE),
            ));
        }
        TaskState::PostProcessing => {
            l2.push(Span::raw("  "));
            l2.push(Span::styled(
                format!("后期处理中 {}", spinner),
                Style::default().fg(LIGHT_BLUE),
            ));
        }
        _ => {}
    }

    let dash = Span::styled("-", Style::default().fg(DIM));
    let mut l3: Vec<Span<'static>> = vec![Span::raw("  ")];
    match t.state {
        TaskState::Failed => {
            let retry = format!("重试 {}/{}", t.retries, t.max_retries);
            let countdown = match t.retry_in {
                Some(s) => format!("{}s 后重试", s.ceil() as u64),

                None => match t.fail_kind {
                    Some(FailKind::Fatal) | Some(FailKind::Verify) => "不自动重试".to_string(),
                    _ => "已达上限".to_string(),
                },
            };
            l3.push(Span::styled(retry, Style::default().fg(RED)));
            l3.push(Span::styled(" · ", Style::default().fg(RED)));
            l3.push(Span::styled(countdown, Style::default().fg(RED)));
            l3.push(Span::styled(" · ", Style::default().fg(RED)));
            l3.push(Span::styled(dl_total_txt(t), Style::default().fg(RED)));
        }
        TaskState::Queued => {
            if t.has_slot {
                l3.push(Span::styled(
                    "已获得下载槽位 · 即将开始".to_string(),
                    Style::default().fg(YELLOW),
                ));
            } else if queue_pos > 0 {
                l3.push(Span::styled(
                    format!("排队第 {} 位 · 等待空闲下载槽位", queue_pos),
                    Style::default().fg(YELLOW),
                ));
            }
            l3.push(Span::styled(" · ", Style::default().fg(DIM)));
            l3.push(Span::styled(dl_total_txt(t), Style::default().fg(FG)));
        }
        TaskState::Completed => {
            match (&t.checksum, t.verify_ok) {
                (Some(ck), Some(true)) => l3.push(Span::styled(
                    format!("{} 校验成功", ck.algo),
                    Style::default().fg(GREEN),
                )),
                (Some(ck), Some(false)) => l3.push(Span::styled(
                    format!("{} 校验失败", ck.algo),
                    Style::default().fg(RED),
                )),
                _ => l3.push(Span::styled("无校验".to_string(), Style::default().fg(DIM))),
            }
            l3.push(Span::styled(" · ", Style::default().fg(DIM)));
            l3.push(Span::styled(fmt_size(t.total), Style::default().fg(FG)));
        }
        TaskState::Seeding => {
            l3.push(Span::styled(
                format!("↑ {}", fmt_speed(t.upload_speed)),
                Style::default().fg(MAGENTA),
            ));
            l3.push(Span::styled(
                format!("  {}/{}", t.seeders, t.peers),
                Style::default().fg(MAGENTA),
            ));
            l3.push(Span::styled(
                format!("  已上传 {}", fmt_size_pair(t.uploaded, t.total)),
                Style::default().fg(FG),
            ));
            l3.push(Span::styled(
                format!("  剩余做种 {}", fmt_dur(t.seed_left.ceil() as u64)),
                Style::default().fg(DIM),
            ));
        }
        _ if t.protocol.is_bt() => {
            let downloading = t.state == TaskState::Downloading;
            if downloading {
                l3.push(Span::styled(
                    format!("↓ {}", fmt_speed(t.speed)),
                    Style::default().fg(ACCENT),
                ));
                l3.push(Span::styled(
                    format!(" ↑ {}", fmt_speed(t.upload_speed)),
                    Style::default().fg(MAGENTA),
                ));
            } else {
                l3.push(Span::raw("↓ "));
                l3.push(dash.clone());
                l3.push(Span::raw(" ↑ "));
                l3.push(dash.clone());
            }
            if downloading {
                l3.push(Span::styled(
                    format!("  {}/{}", t.seeders, t.peers),
                    Style::default().fg(FG),
                ));
            } else {
                l3.push(Span::raw("  -/-"));
            }
            l3.push(Span::styled(
                format!("  {}", fmt_size_pair(t.downloaded, t.total)),
                Style::default().fg(FG),
            ));
            if downloading {
                l3.push(Span::styled(
                    format!("  剩余 {}", fmt_eta(t.eta_secs())),
                    Style::default().fg(DIM),
                ));
            } else {
                l3.push(Span::raw("  剩余 "));
                l3.push(dash.clone());
            }
        }
        _ => {
            let downloading = t.state == TaskState::Downloading;
            if downloading {
                l3.push(Span::styled(
                    format!("↓ {}", fmt_speed(t.speed)),
                    Style::default().fg(ACCENT),
                ));
            } else {
                l3.push(Span::raw("↓ "));
                l3.push(dash.clone());
            }
            l3.push(Span::styled(
                format!("  {}", dl_total_txt(t)),
                Style::default().fg(FG),
            ));
            if downloading && known {
                l3.push(Span::styled(
                    format!("  剩余 {}", fmt_eta(t.eta_secs())),
                    Style::default().fg(DIM),
                ));
            } else {
                l3.push(Span::raw("  剩余 "));
                l3.push(dash.clone());
            }
        }
    }

    vec![
        Line::from(l1),
        Line::from(l2),
        Line::from(l3),
        Line::from(vec![Span::raw("")]),
    ]
}

fn render_task(
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
        area.width.saturating_sub(2) as usize,
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

pub(crate) fn draw_list(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Panel::List && !overlay_open(app);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(focused))
        .title(Span::styled(" 下载任务 ", panel_title_style(focused)))
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

    let ih = ITEM_HEIGHT as usize;
    let rows = inner.height as usize / ih + usize::from(inner.height as usize % ih >= ih - 1);

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
        app.list_sb = None;
        return;
    }

    let spinner = app.spinner();
    let max_items = rows.min(idxs.len().saturating_sub(app.scroll));

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
        let y = inner.y + (vi * ih) as u16;
        let row_area = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: (ih as u16).min(inner.bottom().saturating_sub(y)),
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

    sb.vertical(idxs.len(), rows, app.scroll);
    app.list_sb = sb.render_bars(f, area).vertical;

    app.visible_rows = max_items;
    app.list_area = Some(inner);
}
