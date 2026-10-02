//! detail — 任务详情面板与速度图

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Sparkline};
use ratatui::Frame;

use super::text::{fmt_dur, fmt_eta, fmt_size, fmt_speed, pad_left, pad_right, truncate};
use super::{state_color, ACCENT, BORDER, DIM, DIM2, FG, GREEN, LIGHT_BLUE, MAGENTA, RED, YELLOW};
use crate::app::App;
use crate::model::chunk::fmt_block_size;
use crate::model::{Task, TaskState};

/// 排队详情文案（紧凑：保证「Space 暂停」提示在窄面板不被截断）
fn queued_value(p: Option<usize>, used: usize, max: usize) -> String {
    match p {
        Some(pos) => format!("排队第 {pos} 位（{used}/{max}）· Space 暂停"),
        None => "已获得下载槽位，即将开始下载".to_string(),
    }
}

/// 校验码详情展示：超过 10 位取前 10 位加省略号（窄面板适配）
fn checksum_value_short(value: &str) -> String {
    if value.chars().count() > 10 {
        format!("{}…", value.chars().take(10).collect::<String>())
    } else {
        value.to_string()
    }
}

/// 校验状态文案与颜色（按通过/未通过/校验中/待校验）
fn verify_status(verify_ok: Option<bool>, state: TaskState) -> (&'static str, Color) {
    match (verify_ok, state) {
        (Some(true), _) => ("（已通过）", GREEN),
        (Some(false), _) => ("（未通过）", RED),
        (None, TaskState::Verifying) => ("（校验中）", LIGHT_BLUE),
        _ => ("（待校验）", DIM),
    }
}

/// 速度行（无平均值；缺失用 -；BT 双向、做种、完成态各有版式）
fn speed_row(t: &Task) -> Vec<Span<'static>> {
    let label = |s: &str| Span::styled(pad_right(s, 9), Style::default().fg(DIM));
    match t.state {
        TaskState::Downloading => {
            let mut v = vec![
                Span::raw(" "),
                label("速度"),
                Span::styled(
                    format!("↓ {}", fmt_speed(t.speed)),
                    Style::default().fg(ACCENT),
                ),
            ];
            if t.protocol.is_bt() {
                v.push(Span::styled(
                    format!("  ↑ {}", fmt_speed(t.upload_speed)),
                    Style::default().fg(MAGENTA),
                ));
            }
            v.push(Span::styled(
                format!("  剩余 {}", fmt_eta(t.eta_secs())),
                Style::default().fg(DIM),
            ));
            v
        }
        TaskState::Seeding => vec![
            Span::raw(" "),
            label("速度"),
            Span::styled(
                format!("↑ {}", fmt_speed(t.upload_speed)),
                Style::default().fg(MAGENTA),
            ),
            Span::styled(
                format!("  剩余做种 {}", fmt_dur(t.seed_left.ceil() as u64)),
                Style::default().fg(DIM),
            ),
        ],
        TaskState::Completed => vec![
            Span::raw(" "),
            label("速度"),
            Span::styled(
                format!("—  总用时 {}", fmt_dur(t.elapsed as u64)),
                Style::default().fg(DIM),
            ),
        ],
        _ => {
            let mut v = vec![Span::raw(" "), label("速度"), Span::raw("↓ ")];
            v.push(Span::styled("-", Style::default().fg(DIM)));
            if t.protocol.is_bt() {
                v.push(Span::raw("  ↑ "));
                v.push(Span::styled("-", Style::default().fg(DIM)));
            }
            v.push(Span::raw("  剩余 "));
            v.push(Span::styled("-", Style::default().fg(DIM)));
            v
        }
    }
}

/// 分块行：x/y（x=已完成分块数，y=总分块数）与块大小「N/块」；未探测/零块显示 -
fn chunk_row(t: &Task) -> Vec<Span<'static>> {
    let state_c = state_color(t.state);
    let (x, y, _) = t.chunk_info();
    let mut l = vec![
        Span::raw(" "),
        Span::styled(pad_right("分块", 9), Style::default().fg(DIM)),
    ];
    if t.total == 0 || y == 0 {
        l.push(Span::styled("-".to_string(), Style::default().fg(DIM)));
    } else {
        l.push(Span::styled(
            format!("{x}/{y}"),
            Style::default().fg(state_c).add_modifier(Modifier::BOLD),
        ));
        l.push(Span::styled(
            format!(" · {}/块", fmt_block_size(t.block_size)),
            Style::default().fg(FG),
        ));
    }
    l
}

/// 连接明细状态列（待命/完成/传输中/挂起）
fn conn_status(idle: bool, done: bool, speed: f64) -> Span<'static> {
    if idle {
        Span::styled("待命", Style::default().fg(DIM2))
    } else if done {
        Span::styled("完成", Style::default().fg(GREEN))
    } else if speed > 0.0 {
        Span::styled("传输中", Style::default().fg(ACCENT))
    } else {
        Span::styled("挂起", Style::default().fg(DIM))
    }
}

/// 分块列文案：块号「块 k/y」（分块队列按块大小顺序领块），待命显示 —
fn chunk_col_text(idle: bool, start: u64, nn: u64, yy: u64) -> String {
    if idle {
        "—".to_string()
    } else if nn != 0 {
        format!("块 {}/{}", start.checked_div(nn).unwrap_or(0) + 1, yy)
    } else {
        "—".to_string()
    }
}

/// 单连接明细行（# / 分块 / 进度 / 速度 / 状态）
fn conn_row(i: usize, t: &Task) -> Line<'static> {
    let c = &t.connections[i];
    let idle = c.cap() == 0;
    let frac = if idle { 0.0 } else { c.frac() };
    let done = !idle && frac >= 1.0;
    let st = conn_status(idle, done, c.speed);
    let (_, yy, nn) = t.chunk_info();
    let col = chunk_col_text(idle, c.start, nn, yy);
    Line::from(vec![
        Span::styled(pad_right(&format!("#{}", c.id), 4), Style::default().fg(FG)),
        Span::styled(
            pad_right(&col, 14),
            Style::default().fg(if idle { DIM2 } else { FG }),
        ),
        Span::styled(
            if idle {
                pad_left("—", 7)
            } else {
                pad_left(&format!("{:.1}%", frac * 100.0), 7)
            },
            Style::default().fg(if done {
                GREEN
            } else if idle {
                DIM2
            } else {
                FG
            }),
        ),
        Span::styled(
            format!(
                "{}  ",
                pad_left(
                    &if c.speed > 0.0 {
                        fmt_speed(c.speed)
                    } else {
                        "—".to_string()
                    },
                    11
                )
            ),
            Style::default().fg(if done { DIM } else { ACCENT }),
        ),
        st,
    ])
}

/// 并发分块明细表（表头 + 至多 rows_avail 行）
fn conn_table(t: &Task, rows_avail: usize) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![Span::styled(
        format!(
            "{}{}{}{}  {}",
            pad_right("#", 4),
            pad_right("当前分块", 14),
            pad_left("进度", 7),
            pad_left("速度", 11),
            "状态"
        ),
        Style::default().fg(DIM),
    )])];
    for (i, _) in t.connections.iter().enumerate() {
        if i >= rows_avail.max(1) {
            break;
        }
        lines.push(conn_row(i, t));
    }
    lines
}

pub(super) fn draw_detail(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(Span::styled(" 任务详情 ", Style::default().fg(ACCENT)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(t) = app.sel_task() else {
        f.render_widget(
            Paragraph::new(Span::styled("（未选中任务）", Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    };

    let label = |s: &str| Span::styled(pad_right(s, 9), Style::default().fg(DIM));
    let val = |s: String| Span::styled(s, Style::default().fg(FG));
    let state_c = state_color(t.state);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            truncate(&t.name, (inner.width as usize).saturating_sub(4)),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("状态"),
        Span::styled(
            format!("[{}] {:.1}%", t.state.label(), t.progress() * 100.0),
            Style::default().fg(state_c).add_modifier(Modifier::BOLD),
        ),
    ]));
    // 等待中任务：显示槽位排队详情（位次按列表顺序从上往下；已获槽位 = 重试排队即将开始）
    if t.state == TaskState::Queued {
        let vtxt = queued_value(app.queue_pos_of(t.id), app.used_slots(), app.max_slots);
        lines.push(Line::from(vec![
            Span::raw(" "),
            label("排队"),
            Span::styled(
                truncate(&vtxt, (inner.width as usize).saturating_sub(12)),
                Style::default().fg(YELLOW),
            ),
        ]));
    }
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("ID"),
        Span::styled(
            format!("{id} · 添加于 {created}", id = t.id, created = t.created),
            Style::default().fg(DIM),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("类型"),
        Span::styled(
            t.protocol.label().to_string(),
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ),
        // 并发数仅在「下载中」与「做种中」有数值，其他状态显示 -
        {
            let conn_txt = match t.state {
                TaskState::Downloading | TaskState::Seeding => t.thread_count().to_string(),
                _ => "-".to_string(),
            };
            let resume_txt = if t.resumable {
                "支持断点续传"
            } else {
                "不支持断点续传"
            };
            val(format!(" · {} 并发 · {}", conn_txt, resume_txt))
        },
    ]));
    // 校验行（提供了校验码的任务）：算法 · 校验码前缀 + 状态
    // 前缀取 10 位、状态用短文案，保证 110 列窄面板也能完整显示
    if let Some(ck) = &t.checksum {
        let vshort = checksum_value_short(&ck.value);
        let (st, stc) = verify_status(t.verify_ok, t.state);
        lines.push(Line::from(vec![
            Span::raw(" "),
            label("校验"),
            Span::styled(
                ck.algo.to_string(),
                Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" · {}", vshort), Style::default().fg(FG)),
            Span::styled(st, Style::default().fg(stc)),
        ]));
    }
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("大小"),
        val(format!(
            "{} / {}（剩余 {}）",
            fmt_size(t.downloaded),
            fmt_size(t.total),
            fmt_size(t.total.saturating_sub(t.downloaded))
        )),
    ]));
    // 失败原因（仅已失败）
    if t.state == TaskState::Failed {
        lines.push(Line::from(vec![
            Span::raw(" "),
            label("失败原因"),
            Span::styled(
                truncate(
                    t.error.as_deref().unwrap_or("未知错误"),
                    (inner.width as usize).saturating_sub(12),
                ),
                Style::default().fg(RED),
            ),
        ]));
    }
    // 速度行（无平均值；缺失用 -）
    lines.push(Line::from(speed_row(t)));
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("保存"),
        val(truncate(
            &t.target_path(),
            (inner.width as usize).saturating_sub(12),
        )),
    ]));
    lines.push(Line::from(vec![
        Span::raw(" "),
        label("URL"),
        val(truncate(
            // 重定向后展示最终 URL（FR-01-14 / 规格 01-download-engine-08「详情 URL 显示最终 URL」）
            t.final_url.as_deref().unwrap_or(&t.url),
            (inner.width as usize).saturating_sub(12),
        )),
    ]));

    // 分块：文字显示 x/y 与块大小「N/块」（块大小按协议写死）
    lines.push(Line::from(chunk_row(t)));

    if inner.height as usize > lines.len() + 2 {
        lines.push(Line::from(Span::styled(
            " ────────── 并发分块明细 ──────────",
            Style::default().fg(DIM2),
        )));
        // 表头（紧凑列宽，适配窄面板；CJK 按显示宽度填充）
        lines.push(Line::from(vec![Span::styled(
            format!(
                "{}{}{}{}  {}",
                pad_right("#", 4),
                pad_right("当前分块", 14),
                pad_left("进度", 7),
                pad_left("速度", 11),
                "状态"
            ),
            Style::default().fg(DIM),
        )]));
        let rows_avail = (inner.height as usize).saturating_sub(lines.len() + 1);
        lines.extend(conn_table(t, rows_avail));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

pub(super) fn draw_chart(f: &mut Frame, app: &App, area: Rect) {
    let dl: u64 = (app
        .tasks
        .iter()
        .filter(|t| t.state == TaskState::Downloading)
        .map(|t| t.speed)
        .sum::<f64>()
        / 1024.0) as u64;
    let peak = app.speed_hist.iter().copied().max().unwrap_or(0);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(Line::from(vec![
            Span::styled(" ↓ 全局速度 ", Style::default().fg(ACCENT)),
            Span::styled(
                fmt_speed(dl as f64 * 1024.0),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .title(
            Line::from(Span::styled(
                format!(" 峰值 {} ", fmt_speed(peak as f64 * 1024.0)),
                Style::default().fg(DIM),
            ))
            .alignment(Alignment::Right),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);

    let width = (inner.width as usize).max(1);
    let start = app.speed_hist.len().saturating_sub(width);
    let data: Vec<u64> = app.speed_hist[start..].to_vec();
    f.render_widget(
        Sparkline::default()
            .data(&data)
            .style(Style::default().fg(ACCENT)),
        inner,
    );
}
