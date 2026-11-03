use std::time::Instant;

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::btn::{clip_wide_at_edges, dialog_rect, draw_close_btn};
use super::scrollable_panel::{ScrollablePanel, ScrollbarMode};
use super::text::{pad_left, truncate, w};
use super::{panel_border_style, panel_title_style, BORDER, FG, FOCUS_BORDER, GREEN, RED, YELLOW};
use crate::app::{App, SbKey};
use crate::model::TaskState;
use crate::scrollbar::{DIM, DIM2};

const TRACKERS_W: u16 = 90;
const TRACKERS_H: u16 = 22;

const TRACKERS_COL_URL: usize = 4;
const TRACKERS_URL_WRAP: usize = 44;
const TRACKERS_COL_STATUS: usize = 49;
const TRACKERS_COL_PEERS: usize = 62;
const TRACKERS_COL_SEEDS: usize = 69;
const TRACKERS_COL_MSG: usize = 76;

pub(super) fn break_links(s: &str) -> String {
    const VS15: &str = "\u{FE0E}";
    s.replace("https://", &format!("https{VS15}://"))
        .replace("http://", &format!("http{VS15}://"))
        .replace("www.", &format!("www{VS15}."))
}

fn fg_shadow(c: Option<Color>) -> Option<Color> {
    match c {
        Some(Color::Rgb(r, g, 0)) => Some(Color::Rgb(r, g.saturating_add(1), 1)),
        Some(Color::Rgb(r, g, b)) => Some(Color::Rgb(r, g, b.saturating_sub(1))),
        other => other,
    }
}

pub(super) fn link_break_spans(s: &str, style: Style) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut rest = s;
    let mut shadow = false;
    while let Some((i, c)) = rest.char_indices().find(|&(_, c)| c == '\u{FE0E}') {
        let (head, tail) = rest.split_at(i + c.len_utf8());
        let st = if shadow {
            Style {
                fg: fg_shadow(style.fg),
                ..style
            }
        } else {
            style
        };
        spans.push(Span::styled(head.to_string(), st));
        rest = tail;
        shadow = !shadow;
    }
    if !rest.is_empty() {
        let st = if shadow {
            Style {
                fg: fg_shadow(style.fg),
                ..style
            }
        } else {
            style
        };
        spans.push(Span::styled(rest.to_string(), st));
    }
    if spans.is_empty() {
        spans.push(Span::styled(String::new(), style));
    }
    spans
}

type TrackerStatusView = Option<(bool, Option<u32>, Option<u32>, Option<Instant>)>;

fn wrap_by_width(s: &str, max: usize) -> Vec<String> {
    if max == 0 {
        return vec![s.to_string()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut acc = 0;
    for ch in s.chars() {
        let cw = w(&ch.to_string());
        if acc + cw > max && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
            acc = 0;
        }
        cur.push(ch);
        acc += cw;
    }
    out.push(cur);
    out
}

fn push_col(buf: &mut Vec<Span<'static>>, cur: &mut usize, off: usize, s: String, st: Style) {
    if off > *cur {
        buf.push(Span::raw(" ".repeat(off - *cur)));
    }
    let cw = w(&s);
    if !s.is_empty() {
        buf.push(Span::styled(s, st));
    }
    *cur = if cw > 0 { off + cw } else { off };
}

fn push_col_spans(
    buf: &mut Vec<Span<'static>>,
    cur: &mut usize,
    off: usize,
    spans: Vec<Span<'static>>,
    text_w: usize,
) {
    if off > *cur {
        buf.push(Span::raw(" ".repeat(off - *cur)));
    }
    if !spans.is_empty() {
        buf.extend(spans);
    }
    *cur = if text_w > 0 { off + text_w } else { off };
}

pub(super) fn draw_trackers(f: &mut Frame, app: &mut App, area: Rect) {
    let (name, state, id, trackers) = {
        let snap = app.sel_task().map(|t| {
            (
                t.name.clone(),
                t.state,
                t.id,
                t.bt.as_ref()
                    .map(|b| b.trackers.clone())
                    .unwrap_or_default(),
            )
        });
        let Some(snap) = snap else {
            app.close_trackers_popup();
            return;
        };
        snap
    };
    let running = matches!(
        state,
        TaskState::Downloading
            | TaskState::Seeding
            | TaskState::Verifying
            | TaskState::PostProcessing
    );

    let rect = match app.trackers_pos {
        Some((px, py)) => {
            let dw = TRACKERS_W.min(area.width);
            let dh = TRACKERS_H.min(area.height);
            Rect {
                x: px.min(area.width.saturating_sub(dw)),
                y: py.min(area.height.saturating_sub(dh)),
                width: dw,
                height: dh,
            }
        }
        None => {
            let r = dialog_rect(area, TRACKERS_W, TRACKERS_H);
            app.trackers_pos = Some((r.x, r.y));
            r
        }
    };
    f.render_widget(Clear, rect);

    clip_wide_at_edges(f, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" Trackers ", panel_title_style(true)));
    let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
    sb.thumb(FOCUS_BORDER)
        .pressed_v(app.sb_arrow_pressed(SbKey::Trackers));
    let inner = sb.render_block(f, rect, block);
    app.trackers_area = Some(rect);

    app.popup_close_rect = draw_close_btn(f, app, rect, w(" Trackers "));
    if inner.width < 50 || inner.height < 12 {
        return;
    }

    let content_x = inner.x + 2;
    let content_w = (inner.width as usize).saturating_sub(4);
    let vis = (inner.height as usize).saturating_sub(3);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("任务 ", Style::default().fg(DIM)),
            Span::styled(
                truncate(&name, content_w.saturating_sub(10)),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ])),
        Rect {
            x: content_x,
            y: inner.y + 1,
            width: content_w as u16,
            height: 1,
        },
    );

    let mut header: Vec<Span<'static>> = Vec::new();
    let mut cur = 0usize;
    let hs = Style::default().fg(DIM2);
    push_col(&mut header, &mut cur, 0, "#".to_string(), hs);
    push_col(
        &mut header,
        &mut cur,
        TRACKERS_COL_URL,
        "URL".to_string(),
        hs,
    );
    push_col(
        &mut header,
        &mut cur,
        TRACKERS_COL_STATUS,
        "状态".to_string(),
        hs,
    );
    push_col(
        &mut header,
        &mut cur,
        TRACKERS_COL_PEERS,
        "Peers".to_string(),
        hs,
    );
    push_col(
        &mut header,
        &mut cur,
        TRACKERS_COL_SEEDS,
        "Seeds".to_string(),
        hs,
    );
    push_col(
        &mut header,
        &mut cur,
        TRACKERS_COL_MSG,
        "消息".to_string(),
        hs,
    );
    f.render_widget(
        Paragraph::new(Line::from(header)),
        Rect {
            x: content_x,
            y: inner.y + 2,
            width: content_w as u16,
            height: 1,
        },
    );

    let msg_budget = content_w.saturating_sub(TRACKERS_COL_MSG);
    let now = Instant::now();
    let status_of = |url: &str| -> TrackerStatusView {
        app.trackers_status
            .get(&id)
            .and_then(|rows| rows.iter().find(|r| r.url == url))
            .map(|r| (r.working, r.peers, r.seeds, r.next_at))
    };

    let mut flat: Vec<Vec<Span<'static>>> = Vec::new();
    let url_style = Style::default().fg(FG);
    for (i, tr) in trackers.iter().enumerate() {
        let url_lines = wrap_by_width(tr, TRACKERS_URL_WRAP);
        let (status, status_color, peers_s, seeds_s, msg, msg_color) = if !running {
            (
                "待 announce".to_string(),
                DIM2,
                "—".to_string(),
                "—".to_string(),
                "—".to_string(),
                DIM2,
            )
        } else {
            match status_of(tr) {
                Some((true, peers, seeds, next_at)) => {
                    let remaining = next_at
                        .map(|t| t.saturating_duration_since(now).as_secs())
                        .unwrap_or(0);
                    (
                        "工作中".to_string(),
                        GREEN,
                        peers.map_or_else(|| "—".to_string(), |v| v.to_string()),
                        seeds.map_or_else(|| "—".to_string(), |v| v.to_string()),
                        format!("下轮 {remaining}s"),
                        DIM,
                    )
                }
                Some((false, _, _, next_at)) => {
                    let remaining = next_at
                        .map(|t| t.saturating_duration_since(now).as_secs())
                        .unwrap_or(0);
                    (
                        "失败".to_string(),
                        RED,
                        "—".to_string(),
                        "—".to_string(),
                        format!("重试 {remaining}s"),
                        YELLOW,
                    )
                }
                None => (
                    "待 announce".to_string(),
                    DIM2,
                    "—".to_string(),
                    "—".to_string(),
                    "—".to_string(),
                    DIM2,
                ),
            }
        };
        for (li, ul) in url_lines.iter().enumerate() {
            let mut row: Vec<Span<'static>> = Vec::new();
            let mut cur = 0usize;
            if li == 0 {
                push_col(
                    &mut row,
                    &mut cur,
                    0,
                    pad_left(&format!("{}", i + 1), 2),
                    Style::default().fg(DIM2),
                );
            }
            push_col_spans(
                &mut row,
                &mut cur,
                TRACKERS_COL_URL,
                link_break_spans(&break_links(ul), url_style),
                w(ul),
            );
            if li == 0 {
                push_col(
                    &mut row,
                    &mut cur,
                    TRACKERS_COL_STATUS,
                    status.clone(),
                    Style::default().fg(status_color),
                );
                push_col(
                    &mut row,
                    &mut cur,
                    TRACKERS_COL_PEERS,
                    pad_left(&peers_s, 4),
                    Style::default().fg(if running { FG } else { DIM2 }),
                );
                push_col(
                    &mut row,
                    &mut cur,
                    TRACKERS_COL_SEEDS,
                    pad_left(&seeds_s, 4),
                    Style::default().fg(if running { FG } else { DIM2 }),
                );
                if msg_budget > 0 {
                    push_col(
                        &mut row,
                        &mut cur,
                        TRACKERS_COL_MSG,
                        truncate(&msg, msg_budget),
                        Style::default().fg(msg_color),
                    );
                }
            }
            flat.push(row);
            if li + 1 == url_lines.len() && i + 1 < trackers.len() {
                flat.push(vec![Span::styled(
                    "─".repeat(content_w),
                    Style::default().fg(BORDER),
                )]);
            }
        }
    }

    let total = flat.len();
    app.trackers_scroll = app.trackers_scroll.min(total.saturating_sub(vis));
    let start = app.trackers_scroll;
    if total == 0 {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "（无 trackers）",
                Style::default().fg(DIM2),
            ))),
            Rect {
                x: content_x,
                y: inner.y + 3,
                width: content_w as u16,
                height: 1,
            },
        );
    } else {
        let end = (start + vis).min(total);
        let lines: Vec<Line> = flat[start..end]
            .iter()
            .map(|sp| Line::from(sp.clone()))
            .collect();
        f.render_widget(
            Paragraph::new(lines),
            Rect {
                x: content_x,
                y: inner.y + 3,
                width: content_w as u16,
                height: vis as u16,
            },
        );
    }

    sb.vertical(total, vis, start);
    app.trackers_sb = sb.render_bars(f, rect).vertical;
    app.trackers_max_scroll = total.saturating_sub(vis);
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;
    use crate::model::{Protocol, Task, TaskState};

    fn bt_task_with_trackers(id: u32, trackers: &[&str]) -> Task {
        let mut t = Task::new_queued(
            id,
            format!("bt-{id}.bin"),
            Protocol::Bt,
            format!("/tmp/x{id}.torrent"),
            "/tmp".to_string(),
            16 * 1024,
            2,
            5,
            None,
            crate::model::config::ProxyChoice::Direct,
            0,
        );
        t.state = TaskState::Downloading;
        t.bt = Some(crate::model::BtTaskData {
            source: format!("/tmp/x{id}.torrent"),
            trackers: trackers.iter().map(|s| s.to_string()).collect(),
            piece_length: 16 * 1024,
            files: Vec::new(),
        });
        t
    }

    #[tokio::test]
    async fn trackers_popup_geometry_columns_and_wrapping() {
        let mut app = crate::app::testutil::make_app_in("ezr-trkpop", "geom");
        let long_url = "http://tracker.example.net/announce?k=abcdefghijklmnopqrstuvwxyz0123456789";
        app.tasks = vec![
            bt_task_with_trackers(
                1,
                &[
                    long_url,
                    "udp://tracker.example:6969/announce",
                    "https://backup.example.org/announce",
                ],
            ),
            Task::new_queued(
                2,
                "http-task.bin".to_string(),
                Protocol::Http,
                "http://x/f.bin".to_string(),
                "/tmp".to_string(),
                1024,
                2,
                5,
                None,
                crate::model::config::ProxyChoice::Direct,
                0,
            ),
        ];
        app.selected = 0;
        app.open_trackers_popup();
        assert!(app.trackers_open, "BT 任务打开弹窗");
        let mut term = Terminal::new(TestBackend::new(120, 44)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains(" Trackers "), "标题: {s}");
        assert!(
            s.contains("Peers") && s.contains("Seeds") && s.contains("消息"),
            "qBt 六列表头: {s}"
        );
        assert!(
            s.contains("工作中") || s.contains("待 announce"),
            "状态列三态渲染"
        );
        assert!(!s.contains("wss://"), "不支持协议不出现在弹窗");
        let a = app.trackers_area.expect("area 回填");
        assert_eq!((a.width, a.height), (90, 22), "窗口几何 90×22");

        let buf = term.backend().buffer();
        let row = |y: u16| -> String {
            (a.x + 2..a.right() - 2)
                .map(|x| {
                    buf.cell((x, y))
                        .map(|c| c.symbol().to_string())
                        .unwrap_or_default()
                })
                .collect()
        };
        let header = row(a.y + 3);
        assert!(header.contains('#'), "表头 # 列");
        let body = row(a.y + 4);
        assert!(
            body.trim_end().ends_with("…") || body.contains("http"),
            "URL 折行/截断渲染: {body}"
        );

        app.selected = 1;
        app.trackers_open = false;
        app.open_trackers_popup();
        assert!(!app.trackers_open, "HTTP 任务拒绝打开");
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("该任务没有 trackers")),
            "HTTP 门控 toast: {:?}",
            app.toast
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn trackers_popup_real_status_and_thin_lines() {
        let mut app = crate::app::testutil::make_app_in("ezr-trkpop2", "status");
        app.tasks = vec![bt_task_with_trackers(
            7,
            &[
                "udp://a.example/announce",
                "http://b.example/announce",
                "udp://c.example/announce",
            ],
        )];
        app.selected = 0;
        app.trackers_status.insert(
            7,
            vec![
                crate::app::TrackerRow {
                    url: "udp://a.example/announce".into(),
                    working: true,
                    peers: Some(3),
                    seeds: Some(12),
                    next_at: Some(Instant::now() + Duration::from_secs(30)),
                },
                crate::app::TrackerRow {
                    url: "http://b.example/announce".into(),
                    working: false,
                    peers: None,
                    seeds: None,
                    next_at: Some(Instant::now() + Duration::from_secs(8)),
                },
            ],
        );
        app.open_trackers_popup();
        let mut term = Terminal::new(TestBackend::new(120, 44)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("工作中"), "工作态渲染");
        assert!(
            s.contains("12") && s.contains("3"),
            "Peers/Seeds 真实计数: 片段缺失"
        );
        assert!(s.contains("下轮"), "interval 倒计时消息");
        assert!(
            s.contains("失败") && s.contains("重试"),
            "失败态 + 重试倒计时"
        );
        assert!(s.contains("待 announce"), "未回报 tracker 待态");
        let buf = term.backend().buffer();
        let a = app.trackers_area.expect("area");
        let row = |y: u16| -> String {
            (a.x + 2..a.right() - 2)
                .map(|x| {
                    buf.cell((x, y))
                        .map(|c| c.symbol().to_string())
                        .unwrap_or_default()
                })
                .collect()
        };
        let mut thin_lines = 0;
        for y in a.y + 4..a.bottom() - 1 {
            let t = row(y);
            if t.matches('─').count() >= 40 {
                thin_lines += 1;
            }
        }
        assert_eq!(
            thin_lines, 2,
            "相邻明细行之间恰 2 条细线（3 条 tracker；实测 {thin_lines}）"
        );
        app.shutdown().await;
    }
}
