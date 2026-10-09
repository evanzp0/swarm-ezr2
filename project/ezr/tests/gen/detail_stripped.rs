
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use super::text::{fmt_size, pad_right, truncate, w};
use super::{state_color, ACCENT, BORDER, DIM, FG, GREEN, LIGHT_BLUE, RED, YELLOW};
use crate::app::App;
use crate::model::chunk::fmt_block_size;
use crate::model::{Task, TaskState};

/// 详情面板字段行标签 Span（pad_right 9 列 + DIM 前景色；各字段行共用）
fn dim_label(s: &str) -> Span<'static> {
    Span::styled(pad_right(s, 9), Style::default().fg(DIM))
}

/// 链接样式字段名（FR-01-101，demo 修订-5/6）：文字淡蓝 + 下划线（仅覆盖文字部分），
/// pad_right 对齐填充空格不带下划线；热区仍为整段 9 列（与 label 对齐块一致便于点击）
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

/// 分块行：x/y（x=已完成分块数，y=总分块数）与块大小「N/块」；未探测/零块显示 -
fn chunk_row(t: &Task) -> Vec<Span<'static>> {
    let state_c = state_color(t.state);
    let (x, y, _) = t.chunk_info();
    let mut l = vec![Span::raw(" "), dim_label("分块")];
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

pub(super) fn draw_detail(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(Span::styled(" 任务详情 ", Style::default().fg(ACCENT)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(t) = app.sel_task() else {
        // 无选中任务：字段名热区清空（FR-01-101）
        app.detail_url_rect = None;
        app.detail_ck_rect = None;
        f.render_widget(
            Paragraph::new(Span::styled("（未选中任务）", Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    };

    let val = |s: String| Span::styled(s, Style::default().fg(FG));

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
    // 等待中任务：显示槽位排队详情（位次按列表顺序从上往下；已获槽位 = 重试排队即将开始）
    if t.state == TaskState::Queued {
        let vtxt = queued_value(app.queue_pos_of(t.id), app.used_slots(), app.max_slots);
        lines.push(Line::from(vec![
            Span::raw(" "),
            dim_label("排队"),
            Span::styled(
                truncate(&vtxt, (inner.width as usize).saturating_sub(12)),
                Style::default().fg(YELLOW),
            ),
        ]));
    }
    lines.push(Line::from(vec![
        Span::raw(" "),
        dim_label("ID"),
        Span::styled(
            format!("{id} · 添加于 {created}", id = t.id, created = t.created),
            Style::default().fg(DIM),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw(" "),
        dim_label("类型"),
        Span::styled(
            t.protocol.label().to_string(),
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ),
        // FR-01-100（demo 修订-3）：类型行不再展示并发数，仅保留协议与断点续传支持
        //（并发信息由头部「并发」字段与并发连接面板承载）
        {
            let resume_txt = if t.resumable {
                "支持断点续传"
            } else {
                "不支持断点续传"
            };
            val(format!(" · {resume_txt}"))
        },
    ]));
    // 校验行（提供了校验码的任务）：算法 · 校验码前缀 + 状态
    // 前缀取 10 位、状态用短文案，保证 110 列窄面板也能完整显示
    let mut ck_line_idx: Option<usize> = None;
    if let Some(ck) = &t.checksum {
        let vshort = checksum_value_short(&ck.value);
        let (st, stc) = verify_status(t.verify_ok, t.state);
        ck_line_idx = Some(lines.len());
        let mut ck_line = vec![Span::raw(" ")];
        // FR-01-101：字段名链接样式（淡蓝下划线，点击复制校验码值）
        ck_line.extend(link_label("校验"));
        ck_line.push(Span::styled(
            ck.algo.to_string(),
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ));
        ck_line.push(Span::styled(
            format!(" · {vshort}"),
            Style::default().fg(FG),
        ));
        ck_line.push(Span::styled(st, Style::default().fg(stc)));
        lines.push(Line::from(ck_line));
    }
    lines.push(Line::from(vec![
        Span::raw(" "),
        dim_label("大小"),
        // v1.12/FR-01-80 修订（操作者第九批指令）：仅「已下载/总大小」，
        // 不含「（剩余 …）」后缀（进度展示由列表行承载，FR-01-17 口径不变）
        val(format!(
            "{} / {}",
            fmt_size(t.downloaded),
            fmt_size(t.total)
        )),
    ]));
    // 失败原因（已失败与已暂停（失败）均保留可见，v1.6/FR-01-92）
    if matches!(t.state, TaskState::Failed | TaskState::FailedPaused) {
        lines.push(Line::from(vec![
            Span::raw(" "),
            dim_label("失败原因"),
            Span::styled(
                truncate(
                    t.error.as_deref().unwrap_or("未知错误"),
                    (inner.width as usize).saturating_sub(12),
                ),
                Style::default().fg(RED),
            ),
        ]));
    }
    lines.push(Line::from(vec![
        Span::raw(" "),
        dim_label("保存"),
        val(truncate(
            &t.target_path(),
            (inner.width as usize).saturating_sub(12),
        )),
    ]));
    // FR-01-101：URL 字段名可点击复制（淡蓝下划线）；记录行号供热区回填
    let url_line_idx = lines.len();
    let mut url_line = vec![Span::raw(" ")];
    url_line.extend(link_label("URL"));
    url_line.push(val(truncate(
        // 重定向后展示最终 URL（FR-01-14 / 规格 01-download-engine-08「详情 URL 显示最终 URL」）
        t.final_url.as_deref().unwrap_or(&t.url),
        (inner.width as usize).saturating_sub(12),
    )));
    lines.push(Line::from(url_line));

    // 分块：文字显示 x/y 与块大小「N/块」（块大小按协议写死）
    lines.push(Line::from(chunk_row(t)));

    // （FR-01-81 修订二）并发分块明细表已整体移除：详情面板止于任务级字段行；
    // 逐连接明细自 v1.13 起由独立「并发连接」面板承载（FR-01-96）。

    f.render_widget(Paragraph::new(lines), inner);

    // FR-01-101：回填字段名热区（点击复制）。热区 = 行首空格后的字段名整段
    // （pad_right 9 列，与下划线视觉一致）；y = 内框顶 + 行号，超出内框则不设
    app.detail_url_rect = hot_rect(inner, url_line_idx);
    app.detail_ck_rect = ck_line_idx.and_then(|idx| hot_rect(inner, idx));
}

/// FR-01-101 热区矩形：行首空格后的字段名整段 9 列（x = 内框左 + 1、宽 9、高 1）；
/// 行号超出内框高度时不设（None）
fn hot_rect(inner: Rect, line_idx: usize) -> Option<Rect> {
    let y = inner.y + line_idx as u16;
    (y < inner.y + inner.height).then_some(Rect {
        x: inner.x + 1,
        y,
        width: 9,
        height: 1,
    })
}
