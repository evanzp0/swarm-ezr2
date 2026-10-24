//! detail — 任务详情面板（定高 12 行 = 10 行内容 + 上下边框，FR-01-94 ⑥；
//! v1.20 修订，操作者 20261011 指令：任务 ID 与创建时间拆分两行）
//!
//! v1.14（FR-01-100/101，对齐 ezr-demo 源码修订-3/5/6/7）：类型行去并发数；
//! 字段名 = 淡蓝下划线链接样式 + 热区每帧回填（点击复制见 app/mouse）。
//! v1.34（FR-01-80/101/113/116，对齐 ezr-demo 重构版）：①字段名精简——
//! 「任务 ID」→「ID」、「创建时间」→「创建」、「失败原因」→「失败」、
//! 「保存」→「路径」（`DetailField::title` 单源）；②行首 2 列 padding
//! （S2，FR-01-115）——字段名热区 x = 内框 +2、字段值热区 x = 内框 +11、
//! 截断预算 −13；③可点击复制字段名扩展四项（校验/URL/失败/路径）；
//! ④字段值悬停下划线 + 点击打开字段值弹窗（FR-01-113，值热区每帧回填
//! detail_val_rects）；⑤URL 字段值 break_links_deep 逐字符深度断链显示
//!（v1.34 二批 bug 修订：原 break_links 仅 scheme 锚点注 VS15，终端对 www.
//! 形态 URL 的识别在归一化后仍命中——www 起始域名悬停仍叠加终端默认下划线
//! 形成双线；逐字符注入后任何 ≥2 字符连续裸串均不存在，任意终端识别形态
//! 全断）；⑥大小行总大小未知（不支持续传的 HTTP(S) 流式下载）显示
//! `<已下载> / ? B`（FR-01-116）；⑦「分块」行对不支持分块下载的任务恒显示
//! `-`（supports_pieces 同口径）。

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

/// 悬停下划线开关（FR-01-113：字段值悬停 = 下划线；浮层打开时不响应）
fn with_underline(s: Style, on: bool) -> Style {
    if on {
        s.add_modifier(Modifier::UNDERLINED)
    } else {
        s
    }
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

/// 分块行：x/y（x=已完成分块数，y=总分块数）与块大小「N/块」；未探测/零块/
/// 不支持分块下载的任务（不支持续传的 HTTP(S)，与 P 功能门槛 supports_pieces
/// 同口径——流式下载无块概念）恒显示 -（v1.34/FR-01-116 ②）
fn chunk_row(t: &Task) -> Vec<Span<'static>> {
    let state_c = state_color(t.state);
    let (x, y, _) = t.chunk_info();
    let mut l = vec![Span::raw("  "), dim_label("分块")];
    if t.total == 0 || y == 0 || !t.supports_pieces() {
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
    // 聚焦边框（v1.30/FR-01-108）：详情聚焦且无浮层 → 绿色加粗，否则暗色
    let focused = app.focus == Panel::Detail && !app.overlay_open();
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(focused))
        .title(Span::styled(" 任务详情 ", panel_title_style(focused)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    // 详情面板完整区域（点击任意处聚焦，v1.30/FR-01-108 ⑤）
    app.detail_area = Some(area);

    let Some(t) = app.sel_task() else {
        // 无选中任务：字段名/字段值热区清空（FR-01-101/113）
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

    // v1.34/FR-01-113：字段值悬停 = 下划线、点击 = 打开完整值弹窗。
    // 悬停判定仅在无浮层时生效（浮层打开时详情面板处于模态降级态）；
    // 值热区 = 行首 2 列 padding + 字段名 9 列之后、值文本实际显示宽 × 1 行，
    // 由 ui 层每帧回填（detail_val_rects），点击行为见 on_mouse。
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
    // 登记值热区并返回悬停态（idx = 该行在 lines 中的下标、vw = 值显示宽）
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
    // 行首 2 列 padding（v1.34/FR-01-115 S2，原 1 列；字段名/值热区 x 同步 +1）
    lines.push(Line::from(vec![
        Span::raw("  "),
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
            Span::raw("  "),
            dim_label("排队"),
            Span::styled(
                truncate(&vtxt, (inner.width as usize).saturating_sub(13)),
                Style::default().fg(YELLOW),
            ),
        ]));
    }
    // 任务 ID 单独一行（v1.20 修订拆行；v1.34/FR-01-80：字段名精简「任务 ID」→「ID」）
    lines.push(Line::from(vec![
        Span::raw("  "),
        dim_label("ID"),
        Span::styled(format!("{id}", id = t.id), Style::default().fg(DIM)),
    ]));
    // 创建时间单独一行（v1.20 修订拆行；v1.34/FR-01-80：字段名精简「创建时间」→「创建」）
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
    // 前缀取 10 位、状态用短文案，保证 110 列窄面板也能完整显示；
    // v1.34/FR-01-113：值部分悬停下划线 + 点击打开完整值弹窗
    let mut ck_line_idx: Option<usize> = None;
    if let Some(ck) = &t.checksum {
        let vshort = checksum_value_short(&ck.value);
        let (st, stc) = verify_status(t.verify_ok, t.state);
        ck_line_idx = Some(lines.len());
        // 值显示宽 = 算法 + " · " + 前缀 + 状态（CJK 感知）
        let vw = w(ck.algo) + 3 + w(&vshort) + w(st);
        let hv = val_hover(lines.len(), vw, DetailField::Checksum);
        let mut ck_line = vec![Span::raw("  ")];
        // FR-01-101：字段名链接样式（淡蓝下划线，点击复制校验码值）
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
        // v1.12/FR-01-80 修订：仅「已下载/总大小」，不含「（剩余 …）」后缀；
        // v1.34/FR-01-116：总大小未知（不支持续传的 HTTP(S) 流式下载，bug 修正）
        // 以 `? B` 占位（流结束后恢复实际大小）
        val(if t.size_known() {
            format!("{} / {}", fmt_size(t.downloaded), fmt_size(t.total))
        } else {
            format!("{} / ? B", fmt_size(t.downloaded))
        }),
    ]));
    // 失败：仅「已失败」态展示（v1.6/FR-01-92；v1.22/v1.23 挂起态不再展示）；
    // v1.34/FR-01-80：字段名「失败原因」→「失败」；FR-01-101 v1.34：蓝色下划线
    // 链接（点击复制）+ 值悬停下划线、点击打开完整值弹窗
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
    // 路径：v1.34/FR-01-80：字段名原「保存」再简化为「路径」；FR-01-101 v1.34：
    // 蓝色下划线链接（点击复制），值悬停下划线 + 点击打开完整值弹窗
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
    // URL 字段名可点击复制（淡蓝下划线）；v1.34/FR-01-113：值悬停下划线 +
    // 点击打开完整值弹窗；记录行号供热区回填；FR-01-113 ④：显示值经
    // break_links_deep 逐字符注入 VS15 深度断链——终端对 http(s) 与 www.
    // 形态的 URL 自动识别（含归一化后匹配）全断，不再叠加默认链接下划线
    //（悬停时只剩 ezr 自身的 SGR 下划线，www 起始域名不再双下划线）；经
    // link_break_spans 分段渲染兼容 pyte 类模拟器（文档见该函数）
    let url_line_idx = lines.len();
    let url_s = break_links_deep(&truncate(
        // v1.28：详情 URL = 原始 URL 单值（任务级最终 URL 已退役，FR-01-14/80/D27
        // v1.28「详情 URL 显示原始 URL」，规格 01-download-engine-08 修订）
        &t.url,
        (inner.width as usize).saturating_sub(13),
    ));
    let hv = val_hover(url_line_idx, w(&url_s), DetailField::Url);
    let mut url_line = vec![Span::raw("  ")];
    url_line.extend(link_label("URL"));
    url_line.extend(link_break_spans(
        &url_s,
        with_underline(Style::default().fg(FG), hv),
    ));
    lines.push(Line::from(url_line));

    // 分块：文字显示 x/y 与块大小「N/块」（块大小按协议写死）；不支持分块
    // 下载的任务恒显示 -（v1.34/FR-01-116 ②）
    lines.push(Line::from(chunk_row(t)));

    // （FR-01-81 修订二）并发分块明细表已整体移除：详情面板止于任务级字段行；
    // 逐连接明细自 v1.13 起由独立「并发连接」面板承载（FR-01-96）。

    f.render_widget(Paragraph::new(lines), inner);

    // 回填字段名热区（点击复制，FR-01-101）。热区 = 行首 2 列 padding 后的
    // 字段名整段（pad_right 9 列，与下划线视觉一致；x = 内框 +2，v1.34）；
    // y = 内框顶 + 行号，超出内框则不设
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
    // v1.34：失败/路径 字段名热区 + 字段值热区（悬停下划线/点击弹窗）
    app.detail_fail_rect = fail_line_idx.and_then(hot);
    app.detail_save_rect = hot(save_line_idx);
    app.detail_val_rects = val_rects;
}
