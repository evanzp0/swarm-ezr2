use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{chunk_size_label, App, PieceState, SbKey};
use crate::scrollable_panel::{ScrollablePanel, ScrollbarMode};

use super::*;

fn piece_span(st: PieceState, n: usize) -> Span<'static> {
    let (ch, color) = match st {
        PieceState::Done => ('🟩', GREEN),
        PieceState::Active => ('🟦', LIGHT_BLUE),
        PieceState::Partial => ('🟨', YELLOW),
        PieceState::Empty => ('⬜', EMPTY),
    };
    Span::styled(ch.to_string().repeat(n), Style::default().fg(color))
}

const PIECES_W: u16 = 68;
const PIECES_H: u16 = 20;

pub(crate) fn draw_pieces(f: &mut Frame, app: &mut App, area: Rect) {
    let (name, proto, conc, x, y, states) = {
        let snap = app.sel_task().map(|t| {
            let (x, y, _piece) = t.chunk_info();
            (t.name.clone(), t.protocol, t.concurrency, x, y, t.pieces())
        });
        let Some(snap) = snap else {
            app.close_pieces();
            return;
        };
        snap
    };
    let y = y as usize;

    let rect = match app.pieces_pos {
        Some((px, py)) => {
            let dw = PIECES_W.min(area.width);
            let dh = PIECES_H.min(area.height);
            Rect {
                x: px.min(area.width.saturating_sub(dw)),
                y: py.min(area.height.saturating_sub(dh)),
                width: dw,
                height: dh,
            }
        }
        None => {
            let r = dialog_rect(area, PIECES_W, PIECES_H);
            app.pieces_pos = Some((r.x, r.y));
            r
        }
    };
    f.render_widget(Clear, rect);

    clip_wide_at_edges(f, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" 分块状态 ", panel_title_style(true)));

    let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
    sb.thumb(FOCUS_BORDER)
        .pressed_v(app.sb_arrow_pressed(SbKey::Pieces));
    let inner = sb.render_block(f, rect, block);
    app.pieces_area = Some(rect);

    app.popup_close_rect = draw_close_btn(f, app, rect, w(" 分块状态 "));

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);
    let grid = rows[4];
    let grid_h = grid.height as usize;

    let grid_view = Rect {
        x: grid.x + 2,
        y: grid.y,
        width: grid.width.saturating_sub(4),
        height: grid.height,
    };

    let cols = (grid_view.width as usize / 2).max(1);

    let total_rows = y.div_ceil(cols);

    if app.pieces_follow {
        app.pieces_follow = false;
        let frontier = x as usize / cols;
        app.pieces_scroll = frontier.saturating_sub(grid_h / 2);
    }
    app.pieces_scroll = app.pieces_scroll.min(total_rows.saturating_sub(grid_h));
    let start_row = app.pieces_scroll;

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(
                "  {}",
                truncate(&name, inner.width.saturating_sub(4) as usize)
            ),
            Style::default().fg(FG).add_modifier(Modifier::BOLD),
        ))),
        rows[1],
    );

    let meta = Span::styled(
        format!(
            "  {} · 块大小 {} · {}/{} 块 · 并发 {}",
            proto.label(),
            chunk_size_label(proto),
            x,
            y,
            conc
        ),
        Style::default().fg(DIM),
    );
    f.render_widget(Paragraph::new(Line::from(vec![meta])), rows[2]);

    let mut lines: Vec<Line> = Vec::with_capacity(grid_h);
    for r in 0..grid_h {
        let s = (start_row + r) * cols;
        if s >= y {
            break;
        }
        let e = (s + cols).min(y);
        let mut spans: Vec<Span> = Vec::new();
        let mut run = states[s];
        let mut run_at = s;
        for (off, st) in states[s..e].iter().enumerate() {
            let i = s + off;
            if *st != run {
                spans.push(piece_span(run, i - run_at));
                run = *st;
                run_at = i;
            }
        }
        spans.push(piece_span(run, e - run_at));
        lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(lines), grid_view);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("🟩", Style::default().fg(GREEN)),
            Span::styled(" 已下载   ", Style::default().fg(FG)),
            Span::styled("🟦", Style::default().fg(LIGHT_BLUE)),
            Span::styled(" 正在下载   ", Style::default().fg(FG)),
            Span::styled("🟨", Style::default().fg(YELLOW)),
            Span::styled(" 部分下载   ", Style::default().fg(FG)),
            Span::styled("⬜", Style::default().fg(EMPTY)),
            Span::styled(" 未下载", Style::default().fg(FG)),
        ])),
        rows[6],
    );

    sb.vertical(total_rows, grid_h, start_row);
    app.pieces_sb = sb.render_bars(f, rect).vertical;
    app.pieces_max_scroll = total_rows.saturating_sub(grid_h);
}

const HELP_W: u16 = 70;
const HELP_H: u16 = 24;

pub(crate) fn help_lines() -> Vec<Line<'static>> {
    fn sec(t: &str) -> Line<'static> {
        Line::from(Span::styled(
            format!("  【{}】", t),
            Style::default()
                .fg(FOCUS_BORDER)
                .add_modifier(Modifier::BOLD),
        ))
    }

    fn kd(k: &str, d: &str) -> Line<'static> {
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                pad_right(k, 16),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(d.to_string(), Style::default().fg(FG)),
        ])
    }
    vec![
        sec("全局"),
        kd("Tab / BackTab", "切换面板（任务列表 → 详情 → 并发连接）"),
        kd("鼠标左键", "点击面板聚焦；点击任务/连接选中明细"),
        kd("页脚命令", "A/D/M/Q/H 图例可点击：按压高亮，松开执行"),
        kd("H", "打开 / 关闭本帮助窗口"),
        kd("O", "打开 / 关闭关于窗口（About）"),
        kd("G", "收起/展开 任务详情 + 并发连接面板"),
        kd("Esc", "关闭弹窗"),
        kd("Q", "退出程序（主界面；弹窗内先关窗）"),
        sec("页签"),
        kd("] / [", "右/左切换页签（正在下载 ⇄ 已完成）"),
        kd("鼠标点击页签", "切换到该页签"),
        kd("T", "循环切换页签（与 ] 等效）"),
        sec("任务列表"),
        kd("↑ / ↓", "选择上/下一个任务"),
        kd("PgUp / PgDn", "翻页选择（Home / End 跳首/尾）"),
        kd("鼠标滚轮", "滚动列表（视口自由浏览全表）"),
        kd("A / M / D", "添加 / 修改 / 删除任务"),
        kd("Space / R", "暂停·继续 / 重试"),
        kd("U / J", "上移 / 下移任务"),
        kd("C", "清除已完成任务"),
        kd("P", "查看分块状态（仅分块下载任务）"),
        sec("任务详情"),
        kd("鼠标点击字段名", "校验 / URL / 失败 / 路径 复制对应值"),
        kd("鼠标点击字段值", "打开完整值弹窗（悬停显示下划线）"),
        sec("字段值弹窗"),
        kd("↑↓ / PgUp·PgDn", "滚动内容（Home / End 跳首/尾）"),
        kd("Y", "复制窗口全部内容（完整字段值）"),
        kd("鼠标拖拽 / 滚轮", "移动窗口 / 滚动内容"),
        kd("鼠标点击 [✗]", "关闭窗口（右上角按钮，悬停高亮）"),
        kd("Esc / q", "关闭窗口"),
        sec("并发连接"),
        kd("↑ / ↓", "选择连接明细（视口跟随滚动）"),
        kd("← / →", "横向滚动明细列（客户端/协议等超宽列）"),
        kd("鼠标滚轮", "悬停并发面板 = 滚动连接明细（±2/格）"),
        kd("B", "阻塞并移除选中连接（仅 BT 任务）"),
        sec("滚动条（通用组件）"),
        kd(
            "拖拽滑块 ═║",
            "按住双线滑块拖动 = 比例滚动内容（浮层内同样滚内容）",
        ),
        kd("点击 ↑↓←→", "步进滚动 1 格；按住不放自动连发"),
        kd("点击轨道", "朝点击方向整页滚动（可按住连发）"),
        kd("方向键", "面板聚焦时 ↑↓ 选择/纵向滚动、←→ 横向滚动（并发）"),
        sec("弹窗（添加/修改/删除/pieces/帮助/关于/字段值）"),
        kd("Tab / ↑↓", "切换字段/焦点（下拉框内选择选项）"),
        kd("Enter", "确认"),
        kd("Esc", "取消/关闭"),
        kd("鼠标拖拽", "按住弹窗内空白处拖动移动窗口（滑块/边框除外）"),
        kd("鼠标点击 [✗]", "关闭当前弹窗（右上角按钮，悬停高亮）"),
        sec("pieces 分块弹窗"),
        kd("↑↓ / PgUp·PgDn", "滚动分块网格（Home / End 跳首/尾）"),
        kd("Esc / P / [✗]", "关闭弹窗（右上角按钮可点击，悬停高亮）"),
    ]
}

pub(crate) fn draw_help(f: &mut Frame, app: &mut App, area: Rect) {
    let rect = match app.help_pos {
        Some((px, py)) => {
            let dw = HELP_W.min(area.width);
            let dh = HELP_H.min(area.height);
            Rect {
                x: px.min(area.width.saturating_sub(dw)),
                y: py.min(area.height.saturating_sub(dh)),
                width: dw,
                height: dh,
            }
        }
        None => {
            let r = dialog_rect(area, HELP_W, HELP_H);
            app.help_pos = Some((r.x, r.y));
            r
        }
    };
    f.render_widget(Clear, rect);

    clip_wide_at_edges(f, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" 帮助 ", panel_title_style(true)));

    let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
    sb.thumb(FOCUS_BORDER)
        .pressed_v(app.sb_arrow_pressed(SbKey::Help));
    let inner = sb.render_block(f, rect, block);
    app.help_area = Some(rect);

    app.popup_close_rect = draw_close_btn(f, app, rect, w(" 帮助 "));

    let lines = help_lines();

    let content = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };
    let view_h = content.height as usize;
    let max_scroll = lines.len().saturating_sub(view_h);
    app.help_scroll = app.help_scroll.min(max_scroll);
    let start = app.help_scroll;
    let shown: Vec<Line> = lines.iter().skip(start).take(view_h).cloned().collect();
    f.render_widget(Paragraph::new(shown), content);

    sb.vertical(lines.len(), view_h, start);
    app.help_sb = sb.render_bars(f, rect).vertical;
    app.help_max_scroll = lines.len().saturating_sub(view_h.max(1));
}

const ABOUT_W: u16 = 40;
const ABOUT_H: u16 = 7;

pub(crate) fn draw_about(f: &mut Frame, app: &mut App, area: Rect) {
    let rect = match app.about_pos {
        Some((px, py)) => {
            let dw = ABOUT_W.min(area.width);
            let dh = ABOUT_H.min(area.height);
            Rect {
                x: px.min(area.width.saturating_sub(dw)),
                y: py.min(area.height.saturating_sub(dh)),
                width: dw,
                height: dh,
            }
        }
        None => {
            let r = dialog_rect(area, ABOUT_W, ABOUT_H);
            app.about_pos = Some((r.x, r.y));
            r
        }
    };
    f.render_widget(Clear, rect);

    clip_wide_at_edges(f, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" About ", panel_title_style(true)));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    app.about_area = Some(rect);

    app.popup_close_rect = draw_close_btn(f, app, rect, w(" About "));
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let line = Line::from(Span::styled(
        "EZR Download v1.0.0",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));
    f.render_widget(
        Paragraph::new(line).alignment(Alignment::Center),
        Rect {
            x: inner.x,
            y: inner.y + inner.height / 2,
            width: inner.width,
            height: 1,
        },
    );
}

const VAL_W: u16 = 48;
const VAL_H: u16 = 12;

const VAL_PAD_X: u16 = 2;
const VAL_PAD_Y: u16 = 1;

pub(crate) fn wrap_text(s: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0usize;
    for ch in s.chars() {
        let cw = w(&ch.to_string());
        if cur_w + cw > width {
            out.push(std::mem::take(&mut cur));
            cur_w = 0;
        }
        cur.push(ch);
        cur_w += cw;
    }
    out.push(cur);
    out
}

pub(crate) fn draw_val_popup(f: &mut Frame, app: &mut App, area: Rect) {
    let rect = match app.val_pos {
        Some((px, py)) => {
            let dw = VAL_W.min(area.width);
            let dh = VAL_H.min(area.height);
            Rect {
                x: px.min(area.width.saturating_sub(dw)),
                y: py.min(area.height.saturating_sub(dh)),
                width: dw,
                height: dh,
            }
        }
        None => {
            let r = dialog_rect(area, VAL_W, VAL_H);
            app.val_pos = Some((r.x, r.y));
            r
        }
    };
    f.render_widget(Clear, rect);

    clip_wide_at_edges(f, rect);
    let title = format!(" {} ", app.val_title);
    let title_w = w(&title);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(title, panel_title_style(true)));

    let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
    sb.thumb(FOCUS_BORDER)
        .pressed_v(app.sb_arrow_pressed(SbKey::Val));
    let inner = sb.render_block(f, rect, block);
    app.val_area = Some(rect);

    app.popup_close_rect = draw_close_btn(f, app, rect, title_w);

    let pad_x = VAL_PAD_X.min(inner.width / 2);
    let pad_y = VAL_PAD_Y.min(inner.height / 2);
    let content = Rect {
        x: inner.x + pad_x,
        y: inner.y + pad_y,
        width: inner.width - pad_x * 2,
        height: inner.height - pad_y * 2,
    };
    let view_h = content.height as usize;

    let lines: Vec<Line<'static>> = wrap_text(&app.val_text, content.width as usize)
        .into_iter()
        .map(|s| Line::from(link_break_spans(&break_links(&s), Style::default().fg(FG))))
        .collect();
    let total = lines.len();
    let max_scroll = total.saturating_sub(view_h);
    app.val_scroll = app.val_scroll.min(max_scroll);
    let start = app.val_scroll;
    let shown: Vec<Line> = lines.into_iter().skip(start).take(view_h).collect();
    f.render_widget(Paragraph::new(shown), content);

    sb.vertical(total, view_h, start);
    app.val_sb = sb.render_bars(f, rect).vertical;
    app.val_max_scroll = total.saturating_sub(view_h.max(1));
}
