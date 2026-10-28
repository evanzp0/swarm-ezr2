use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{App, DialogKind, CHECKSUM_ALGOS};

use crate::textfield::{FieldWin, TextInput};

use super::*;

pub(crate) fn dialog_rect(area: Rect, dw: u16, dh: u16) -> Rect {
    let dw = dw.min(area.width);
    let dh = dh.min(area.height);
    Rect {
        x: area.x + (area.width.saturating_sub(dw)) / 2,
        y: area.y + (area.height.saturating_sub(dh)) / 2,
        width: dw,
        height: dh,
    }
}

pub(crate) fn dialog_rect_at(area: Rect, pos: Option<(u16, u16)>, dw: u16, dh: u16) -> Rect {
    match pos {
        Some((px, py)) => {
            let dw = dw.min(area.width);
            let dh = dh.min(area.height);
            Rect {
                x: px.min(area.width.saturating_sub(dw)),
                y: py.min(area.height.saturating_sub(dh)),
                width: dw,
                height: dh,
            }
        }
        None => dialog_rect(area, dw, dh),
    }
}

pub(crate) fn button_spans(txt: &str, focused: bool) -> Vec<Span<'static>> {
    let label = format!("[ {} ]", txt);
    if focused {
        vec![Span::styled(
            format!(" {} ", label),
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        )]
    } else {
        vec![
            Span::styled(" ", Style::default()),
            Span::styled(label, Style::default().fg(FG)),
            Span::styled(" ", Style::default()),
        ]
    }
}

pub(crate) fn draw_dialogs(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(d) = app.dialog.as_ref() else { return };
    app.dlg_btn_rects.clear();
    app.dlg_field_rects.clear();
    app.dlg_field_wins.clear();
    app.dlg_ck_rects.clear();
    app.dlg_proxy_rects.clear();
    app.dlg_btfile_rects.clear();
    app.dlg_bt_area = None;
    app.dlg_bt_visible = 0;
    match d.kind {
        DialogKind::Add => draw_task_dialog(f, app, area, DialogKind::Add),
        DialogKind::Modify => draw_task_dialog(f, app, area, DialogKind::Modify),
        DialogKind::Delete => draw_delete_dialog(f, app, area),
        DialogKind::AddBt => draw_bt_dialog(f, app, area, DialogKind::AddBt),
        DialogKind::ModifyBt => draw_bt_dialog(f, app, area, DialogKind::ModifyBt),
    }
}

pub(crate) fn dropdown_rect(area: Rect, inner: Rect, row_y: u16, pw: u16, ph: u16) -> Rect {
    let mut py = row_y + 1;
    if py + ph > area.y + area.height {
        py = row_y.saturating_sub(ph);
    }
    let px = (inner.x + 9)
        .min(area.x + area.width.saturating_sub(pw + 1))
        .max(area.x + 1);
    Rect {
        x: px,
        y: py.max(area.y),
        width: pw,
        height: ph,
    }
}

enum RowKind {
    Text,
    CkSel,
    ProxySel,
}

struct Row {
    label: &'static str,

    input: TextInput,
    placeholder: String,
    kind: RowKind,
}

pub(crate) fn draw_task_dialog(f: &mut Frame, app: &mut App, area: Rect, kind: DialogKind) {
    let is_add = kind == DialogKind::Add;
    let (title, dw, dh) = if is_add {
        (" 添加任务：HTTP(S) ", 74u16, 12u16)
    } else {
        (" 修改任务：HTTP(S) ", 74, 10)
    };

    let dlg = dialog_rect_at(area, app.dialog_pos, dw, dh);
    if app.dialog_pos.is_none() {
        app.dialog_pos = Some((dlg.x, dlg.y));
    }
    app.dialog_area = Some(dlg);
    f.render_widget(Clear, dlg);
    clip_wide_at_edges(f, dlg);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(title, panel_title_style(true)));
    let inner = block.inner(dlg);
    f.render_widget(block, dlg);

    app.popup_close_rect = draw_close_btn(f, app, dlg, w(title));
    let min_h = if is_add { 10 } else { 8 };
    if inner.width < 24 || inner.height < min_h {
        return;
    }

    let (focus, url, dir, conns, ck_type, ck_value, ck_open, ck_sel, proxy_sel, proxy_open) = {
        let d = app.dialog.as_ref().unwrap();
        (
            d.focus,
            d.url.clone(),
            d.dir.clone(),
            d.conns.clone(),
            d.ck_type,
            d.ck_value.clone(),
            d.ck_open,
            d.ck_sel,
            d.proxy_sel,
            d.proxy_open,
        )
    };
    let (algo_name, _algo_need) = CHECKSUM_ALGOS[ck_type.min(CHECKSUM_ALGOS.len() - 1)];
    let proxy_label = app
        .proxy_labels
        .get(proxy_sel)
        .cloned()
        .unwrap_or_else(|| "直连".to_string());

    let mut rows: Vec<Row> = Vec::new();
    if is_add {
        rows.push(Row {
            label: "URL",
            input: url,

            placeholder: String::new(),
            kind: RowKind::Text,
        });
        rows.push(Row {
            label: "保存到",
            input: dir,
            placeholder: "/srv/downloads".to_string(),
            kind: RowKind::Text,
        });
    }
    rows.push(Row {
        label: "并发",
        input: conns,
        placeholder: "4".to_string(),
        kind: RowKind::Text,
    });
    rows.push(Row {
        label: "校验",
        input: TextInput::new(),
        placeholder: String::new(),
        kind: RowKind::CkSel,
    });
    rows.push(Row {
        label: "校验码",
        input: ck_value,

        placeholder: String::new(),
        kind: RowKind::Text,
    });
    rows.push(Row {
        label: "代理",
        input: TextInput::new(),
        placeholder: String::new(),
        kind: RowKind::ProxySel,
    });

    let avail = (inner.width as usize).saturating_sub(13);
    let mut ck_row_y = inner.y;
    let mut proxy_row_y = inner.y;
    for (i, row) in rows.iter().enumerate() {
        let focused = focus == i;
        let row_y = inner.y + 1 + i as u16;
        match row.kind {
            RowKind::CkSel => ck_row_y = row_y,
            RowKind::ProxySel => proxy_row_y = row_y,
            RowKind::Text => {}
        }
        let mut spans = vec![
            Span::styled(
                pad_right(row.label, 7),
                Style::default().fg(if focused { ACCENT } else { DIM }),
            ),
            Span::styled("> ".to_string(), Style::default().fg(DIM2)),
        ];

        let mut text_win: Option<FieldWin> = None;
        let mut pre_cols = 0u16;
        match row.kind {
            RowKind::CkSel => {
                spans.push(Span::styled(
                    format!("{} ", algo_name),
                    Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    "▾",
                    Style::default().fg(if focused { YELLOW } else { DIM }),
                ));
            }
            RowKind::ProxySel => {
                spans.push(Span::styled(
                    format!("{} ", proxy_label),
                    Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    "▾",
                    Style::default().fg(if focused { YELLOW } else { DIM }),
                ));
            }
            RowKind::Text => {
                let fd = row.input.field_window(&row.placeholder, avail, focused);
                let base = Style::default().fg(if row.input.is_empty() {
                    DIM2
                } else {
                    Color::White
                });
                if focused {
                    pre_cols = w(&fd.pre) as u16;
                    spans.push(Span::styled(fd.pre, base));
                    spans.push(Span::styled(fd.post, base));
                } else {
                    spans.push(Span::styled(format!("{}{}", fd.pre, fd.post), base));
                }
                text_win = Some(fd.win);
            }
        }
        let rect = Rect {
            x: inner.x + 2,
            y: row_y,
            width: inner.width.saturating_sub(4),
            height: 1,
        };
        f.render_widget(Paragraph::new(Line::from(spans)), rect);

        let mut win = text_win.take();
        if let Some(w0) = win.as_mut() {
            w0.value_x = rect.x + 9;
            if focused {
                app.dlg_cursor_screen = Some((rect.x + 9 + pre_cols, row_y));
            }
        }
        app.dlg_field_rects.push((rect, i));
        app.dlg_field_wins.push(win);
    }

    let n_rows = rows.len() as u16;

    let btn_row = inner.y + n_rows + 2;
    let buttons: Vec<(&str, usize)> = if is_add {
        vec![("立即下载", 6), ("仅添加", 7), ("取消", 8)]
    } else {
        vec![("确定", 4), ("取消", 5)]
    };
    let btn_ws: Vec<usize> = buttons
        .iter()
        .map(|(txt, _)| w(&format!("[ {} ]", txt)) + 2)
        .collect();
    let total_w: usize = btn_ws.iter().sum::<usize>() + 2 * (buttons.len() - 1);
    let mut bx = inner.x as usize + ((inner.width as usize).saturating_sub(total_w)) / 2;
    let inner_right = inner.x as usize + inner.width as usize;
    let buf_right = f.area().width as usize;
    for ((txt, idx), bw) in buttons.iter().zip(btn_ws.iter()) {
        let focused = focus == *idx;
        let line = Line::from(button_spans(txt, focused));

        let width = (*bw)
            .min(inner_right.saturating_sub(bx))
            .min(buf_right.saturating_sub(bx));
        if width > 0 {
            let rect = Rect {
                x: bx as u16,
                y: btn_row,
                width: width as u16,
                height: 1,
            };
            f.render_widget(Paragraph::new(line), rect);
            app.dlg_btn_rects.push((rect, *idx));
        }
        bx += bw + 2;
    }

    if ck_open {
        let items = CHECKSUM_ALGOS;
        let pw = 24u16;
        let ph = items.len() as u16 + 2;
        let prect = dropdown_rect(area, inner, ck_row_y, pw, ph);
        f.render_widget(Clear, prect);
        clip_wide_at_edges(f, prect);
        let pblock = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(panel_border_style(true))
            .title(Span::styled(" 校验算法 ", panel_title_style(true)));
        let pinner = pblock.inner(prect);
        f.render_widget(pblock, prect);
        for (i, (name, need)) in items.iter().enumerate() {
            let sel = i == ck_sel;
            let bg = if sel { Color::DarkGray } else { Color::Reset };
            let line = Line::from(vec![
                Span::styled(
                    format!("  {} ", if sel { "▸" } else { " " }),
                    Style::default().fg(if sel { ACCENT } else { DIM2 }).bg(bg),
                ),
                Span::styled(
                    pad_right(name, 9),
                    Style::default()
                        .fg(if sel { Color::White } else { FG })
                        .bg(bg)
                        .add_modifier(if sel {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::styled(
                    if *need == 0 {
                        "—".to_string()
                    } else {
                        format!("{} 位", need)
                    },
                    Style::default().fg(if sel { ACCENT } else { DIM2 }).bg(bg),
                ),
            ]);
            let irect = Rect {
                x: pinner.x,
                y: pinner.y + i as u16,
                width: pinner.width,
                height: 1,
            };
            f.render_widget(Paragraph::new(line).style(Style::default().bg(bg)), irect);

            app.dlg_ck_rects.push((irect, i));
        }
    }

    if proxy_open {
        let items: Vec<String> = app.proxy_labels.clone();
        let pw = 26u16;
        let ph = items.len() as u16 + 2;
        let prect = dropdown_rect(area, inner, proxy_row_y, pw, ph);
        f.render_widget(Clear, prect);
        clip_wide_at_edges(f, prect);
        let pblock = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(panel_border_style(true))
            .title(Span::styled(" 选择代理 ", panel_title_style(true)));
        let pinner = pblock.inner(prect);
        f.render_widget(pblock, prect);
        for (i, name) in items.iter().enumerate() {
            let sel = i == proxy_sel;
            let bg = if sel { Color::DarkGray } else { Color::Reset };
            let line = Line::from(vec![
                Span::styled(
                    format!("  {} ", if sel { "▸" } else { " " }),
                    Style::default().fg(if sel { ACCENT } else { DIM2 }).bg(bg),
                ),
                Span::styled(
                    truncate(name, (pw as usize).saturating_sub(6)),
                    Style::default()
                        .fg(if sel { Color::White } else { FG })
                        .bg(bg)
                        .add_modifier(if sel {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
            ]);
            let irect = Rect {
                x: pinner.x,
                y: pinner.y + i as u16,
                width: pinner.width,
                height: 1,
            };
            f.render_widget(Paragraph::new(line).style(Style::default().bg(bg)), irect);

            app.dlg_proxy_rects.push((irect, i));
        }
    }
}

pub(crate) fn draw_delete_dialog(f: &mut Frame, app: &mut App, area: Rect) {
    let dlg = dialog_rect_at(area, app.dialog_pos, 66, 7);
    if app.dialog_pos.is_none() {
        app.dialog_pos = Some((dlg.x, dlg.y));
    }
    app.dialog_area = Some(dlg);
    f.render_widget(Clear, dlg);
    clip_wide_at_edges(f, dlg);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" 删除任务 ", panel_title_style(true)));
    let inner = block.inner(dlg);
    f.render_widget(block, dlg);

    app.popup_close_rect = draw_close_btn(f, app, dlg, w(" 删除任务 "));
    if inner.width < 40 || inner.height < 5 {
        return;
    }

    let (focus, task_name) = {
        let d = app.dialog.as_ref().unwrap();
        (d.focus, d.task_name.clone())
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("删除任务 ", Style::default().fg(DIM)),
            Span::styled(
                format!(
                    "「{}」",
                    truncate(&task_name, (inner.width as usize).saturating_sub(22))
                ),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("？", Style::default().fg(DIM)),
        ])),
        Rect {
            x: inner.x + 2,
            y: inner.y + 1,
            width: inner.width.saturating_sub(4),
            height: 1,
        },
    );

    let btn_row = inner.y + 3;
    let labels: [(&str, usize); 3] = [("仅删除任务", 0), ("删除任务和文件", 1), ("取消", 2)];
    let btn_ws: Vec<usize> = labels
        .iter()
        .map(|(txt, _)| w(&format!("[ {} ]", txt)) + 2)
        .collect();
    let total_w: usize = btn_ws.iter().sum::<usize>() + 2 * (labels.len() - 1);
    let mut bx = inner.x as usize + ((inner.width as usize).saturating_sub(total_w)) / 2;
    for ((txt, idx), bw) in labels.iter().zip(btn_ws.iter()) {
        let focused = focus == *idx;
        let line = Line::from(button_spans(txt, focused));
        let rect = Rect {
            x: bx as u16,
            y: btn_row,
            width: *bw as u16,
            height: 1,
        };
        f.render_widget(Paragraph::new(line), rect);
        app.dlg_btn_rects.push((rect, *idx));
        bx += bw + 2;
    }
}

/// BT 文本字段行渲染（种子/保存到/并发）：返回取窗几何回填给命中映射。
fn bt_text_row(
    f: &mut Frame,
    app: &mut App,
    focus: usize,
    idx: usize,
    label: &str,
    input: &TextInput,
    ph: &str,
    rect: Rect,
    avail: usize,
) -> Option<FieldWin> {
    let focused = focus == idx;
    let mut spans = vec![
        Span::styled(
            pad_right(label, 7),
            Style::default().fg(if focused { ACCENT } else { DIM }),
        ),
        Span::styled("> ".to_string(), Style::default().fg(DIM2)),
    ];
    let fd = input.field_window(ph, avail, focused);
    let base = Style::default().fg(if input.is_empty() { DIM2 } else { Color::White });
    if focused {
        let pre_cols = w(&fd.pre) as u16;
        spans.push(Span::styled(fd.pre.clone(), base));
        spans.push(Span::styled(fd.post.clone(), base));
        app.dlg_cursor_screen = Some((rect.x + 9 + pre_cols, rect.y));
    } else {
        spans.push(Span::styled(format!("{}{}", fd.pre, fd.post), base));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), rect);
    let mut win = fd.win;
    win.value_x = rect.x + 9;
    Some(win)
}

/// BT 下拉行渲染（代理）：黄色值 + ▾。
fn bt_dropdown_row(f: &mut Frame, focus: usize, idx: usize, label: &str, value: &str, rect: Rect) {
    let focused = focus == idx;
    let spans = vec![
        Span::styled(
            pad_right(label, 7),
            Style::default().fg(if focused { ACCENT } else { DIM }),
        ),
        Span::styled("> ".to_string(), Style::default().fg(DIM2)),
        Span::styled(
            format!("{value} "),
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ),
        Span::styled("▾", Style::default().fg(if focused { YELLOW } else { DIM })),
    ];
    f.render_widget(Paragraph::new(Line::from(spans)), rect);
}

/// BT 对话框（添加 / 修改任务：BT）：种子来源 + 内部文件树勾选列表 + 并发/代理
/// + trackers 多行输入。
///
/// 布局（§6.9/§6.10，§4.3 S2 padding 规范）：
/// - AddBt 78×26：顶 padding 1 行（首行内容 = 内框 y+1）/ 种子 / 保存到 /
///   「选择下载文件」列表框（7 可视行）/ 并发 / 代理 / trackers 输入框
///   （5 可视行）/ 按钮前空行 1 行 / 按钮行 / 底 padding 1 行；
/// - ModifyBt 78×24：顶 padding 1 行 / 任务名 / 文件列表框（6 可视行）/ 并发 /
///   代理 / trackers 输入框（5 可视行）/ 按钮前空行 / 按钮行 / 底 padding
///   （小终端列表框按剩余高度收缩，不低于 1 行）；
/// - 文件列表框 = 可复用 TreePanel 组件（`ui::tree::draw_tree_panel`）：
///   左上标题固定「选择下载文件」，右下角动态统计「已选 <x> 个文件，共 <z>」
///   （x = 已选文件数，z = 已选文件字节合计，收拢目录下的已选文件同样计入，
///   绘制在底边框右侧，不覆盖右下角框角字符）；右上角 [全选] [全不选] 按钮
///   （悬停黄色高亮，无下划线）由对话框层绘制；目录 ▾/▸ 展开·收拢（默认收拢），
///   三态勾选 `[x]`/`[-]`/`[ ]`，行首缩进体现层级，右对齐大小（目录 = 子树聚合），
///   内容区行首/行尾各 2 列 padding，光标行 SEL_BG 满行；
/// - trackers 输入框 = 可复用多行文本输入 panel 组件
///   （`ui::multiline::draw_multi_line_input`，wrap = false 无 wrap 模式）：
///   左上标题「trackers」，空缓冲显示占位提示「可输入多行 traker，使用
///   ALT+ENTER 换行」；5 可视行，逻辑行 1:1 渲染不做 word wrap，行内超宽
///   内容由水平滚动条（下边框 ←→/轨道/拖拽 + 水平滚轮）承接，溢出 5 行由
///   垂直滚动条（右边框）承接；
///   Alt+Enter（任何终端）/ Shift+Enter（kitty 协议终端）换行，
///   Enter = 确认（与按钮 click 同义）；
/// - 内容超出可视行/可视列时，滚动条由 ScrollablePanel 绘制在列表框/输入框
///   右边框与下边框上（与任务列表 ScrollPanel 同构：同箭头/轨道/内边距几何，
///   panel 层级，不占用内容列）；视口跟随为按需制：光标移动/树结构变化时拉回，
///   滚动条/滚轮滚动后停留原处（光标停在末行也可自由上滚）。
pub(crate) fn draw_bt_dialog(f: &mut Frame, app: &mut App, area: Rect, kind: DialogKind) {
    let is_add = kind == DialogKind::AddBt;
    let (title, dw, dh) = if is_add {
        (" 添加任务：BT ", 78u16, 26u16)
    } else {
        (" 修改任务：BT ", 78, 24)
    };
    let nfocus = if is_add { 9usize } else { 6usize };
    let list_focus = if is_add { 2usize } else { 0usize };
    let trackers_focus = if is_add { 5usize } else { 3usize };

    let dlg = dialog_rect_at(area, app.dialog_pos, dw, dh);
    if app.dialog_pos.is_none() {
        app.dialog_pos = Some((dlg.x, dlg.y));
    }
    app.dialog_area = Some(dlg);
    f.render_widget(Clear, dlg);
    clip_wide_at_edges(f, dlg);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(title, panel_title_style(true)));
    let inner = block.inner(dlg);
    f.render_widget(block, dlg);

    app.popup_close_rect = draw_close_btn(f, app, dlg, w(title));
    // 布局最低需求：Add = 顶 padding 1 + 2 字段行 + 1 列表行 + 并发/代理 2
    // + trackers 框 7 + 按钮前空行 1 + 按钮 1 + 底 padding 1 + 边框 2（自留 17）；
    // Modify = 顶 padding 1 + 1 任务名 + 其余同上（自留 16）。低于此值不渲染。
    let min_h = if is_add { 18 } else { 17 };
    if inner.width < 30 || inner.height < min_h {
        return;
    }

    let (focus, torrent, dir, conns, proxy_sel, proxy_open, task_name) = {
        let d = app.dialog.as_ref().unwrap();
        (
            d.focus,
            d.torrent.clone(),
            d.dir.clone(),
            d.conns.clone(),
            d.proxy_sel,
            d.proxy_open,
            d.task_name.clone(),
        )
    };
    let proxy_label = app
        .proxy_labels
        .get(proxy_sel)
        .cloned()
        .unwrap_or_else(|| "直连".to_string());

    let row_rect = |y: u16| Rect {
        x: inner.x + 2,
        y,
        width: inner.width.saturating_sub(4),
        height: 1,
    };
    let avail = (inner.width as usize).saturating_sub(13);

    let mut field_rects: Vec<Option<Rect>> = vec![None; nfocus];
    let mut field_wins: Vec<Option<FieldWin>> = vec![None; nfocus];

    // ---------- 文件列表框（TreePanel 组件） ----------
    // 顶 padding 1 行（§4.3 S2）：首行内容 = 内框 y+1。
    // Add = padding + 种子 + 保存到 两字段行；Modify = padding + 任务名一行
    let list_y = if is_add { inner.y + 3 } else { inner.y + 2 };
    // 可视行数：标准 7 行（Modify 6；phase2.10 由 5/4 加高 2 行，窗口同步
    // 78×26/78×24）；小终端按剩余高度收缩（Add 需 17 行自留，Modify 需 16）
    let list_rows = ((inner.height as usize).saturating_sub(if is_add { 17 } else { 16 }))
        .min(if is_add { 7 } else { 6 })
        .max(1);
    let box_rect = Rect {
        x: inner.x + 2,
        y: list_y,
        width: inner.width.saturating_sub(4),
        height: (list_rows + 2) as u16,
    };
    {
        let d = app.dialog.as_mut().unwrap();
        // 视口跟随（按需）：光标移动/树结构变化时拉回一次；滚动条/滚轮滚动后
        // 不强制回贴——否则光标停在末行时视口被钉死在底部，滚动条无法上滚
        if d.bt_follow {
            d.bt_scroll_follow(list_rows);
            d.bt_follow = false;
        }
        let max = d.bt_row_count().saturating_sub(list_rows.max(1));
        d.bt_scroll = d.bt_scroll.min(max);
    }
    // 行数据快照（避免与 app 可变借用冲突）
    let (tree_rows, bt_cursor, bt_scroll, sel_cnt, sel_bytes) = {
        let d = app.dialog.as_ref().unwrap();
        let rows: Vec<crate::ui::tree::TreeRow> = d
            .bt_rows()
            .iter()
            .map(|(_, n)| crate::ui::tree::TreeRow {
                depth: n.depth,
                is_dir: n.is_dir,
                name: n.name.clone(),
                size: n.size,
                selected: n.selected,
                partial: n.partial,
                expanded: n.expanded,
            })
            .collect();
        let sel = d.bt_counts().0;
        let bytes = d.bt_selected_bytes();
        (rows, d.bt_cursor, d.bt_scroll, sel, bytes)
    };
    let list_focused = focus == list_focus;
    // 左上标题固定；动态统计移至 panel 右下角（底边框右侧）
    let tree_title = " 选择下载文件 ";
    let tree_caption = format!("已选 {} 个文件，共 {}", sel_cnt, fmt_size(sel_bytes));
    let tree = draw_tree_panel(
        f,
        box_rect,
        tree_title,
        &tree_caption,
        list_focused,
        app.sb_arrow_pressed(crate::app::SbKey::BtFiles),
        &tree_rows,
        bt_cursor,
        bt_scroll,
        list_rows,
    );
    app.dlg_bt_sb = tree.sb;
    app.dlg_bt_area = Some(Rect {
        width: tree.inner.width + 1,
        ..tree.inner
    });
    app.dlg_bt_visible = list_rows;
    app.dlg_btfile_rects = tree.row_rects;
    field_rects[list_focus] = Some(box_rect);

    // 右上标题：[全选] [全不选] 按钮（悬停高亮；点击作用于文件树）
    // 显示宽：[全选] = 1+2+2+1 = 6；[全不选] = 1+2+2+2+1 = 8
    app.dlg_btbtn_rects.clear();
    {
        let right = box_rect.x + box_rect.width.saturating_sub(1); // 右角列（┐）
        let none_x = right.saturating_sub(8);
        let all_x = none_x.saturating_sub(1 + 6);
        for (txt, bx, bw, idx) in
            [("全选", all_x, 6u16, 0usize), ("全不选", none_x, 8u16, 1usize)]
        {
            if bx <= box_rect.x + 1 {
                continue;
            }
            let r = Rect {
                x: bx,
                y: box_rect.y,
                width: bw,
                height: 1,
            };
            let hot = hovered(app.pointer, r);
            // 悬停 = 黄色加亮（不加下划线）
            let st = if hot {
                Style::default().fg(YELLOW).add_modifier(Modifier::BOLD)
            } else if list_focused {
                Style::default().fg(DIM)
            } else {
                Style::default().fg(DIM2)
            };
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("[", st),
                    Span::styled(txt.to_string(), st),
                    Span::styled("]", st),
                ])),
                r,
            );
            app.dlg_btbtn_rects.push((r, idx));
        }
    }

    // ---------- 字段行 ----------
    if is_add {
        field_rects[0] = Some(row_rect(inner.y + 1));
        field_wins[0] = bt_text_row(
            f,
            app,
            focus,
            0,
            "种子",
            &torrent,
            "",
            row_rect(inner.y + 1),
            avail,
        );
        field_rects[1] = Some(row_rect(inner.y + 2));
        field_wins[1] = bt_text_row(
            f,
            app,
            focus,
            1,
            "保存到",
            &dir,
            "/srv/downloads",
            row_rect(inner.y + 2),
            avail,
        );
        field_rects[3] = Some(row_rect(list_y + box_rect.height));
        field_wins[3] = bt_text_row(
            f,
            app,
            focus,
            3,
            "并发",
            &conns,
            "20",
            row_rect(list_y + box_rect.height),
            avail,
        );
        field_rects[4] = Some(row_rect(list_y + box_rect.height + 1));
        bt_dropdown_row(
            f,
            focus,
            4,
            "代理",
            &proxy_label,
            row_rect(list_y + box_rect.height + 1),
        );
    } else {
        // 任务名行（静态；顶 padding 1 行）
        let name_rect = Rect {
            x: inner.x + 2,
            y: inner.y + 1,
            width: inner.width.saturating_sub(4),
            height: 1,
        };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("任务 ", Style::default().fg(DIM)),
                Span::styled(
                    truncate(&task_name, (name_rect.width as usize).saturating_sub(10)),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ])),
            name_rect,
        );
        field_rects[1] = Some(row_rect(list_y + box_rect.height));
        field_wins[1] = bt_text_row(
            f,
            app,
            focus,
            1,
            "并发",
            &conns,
            "20",
            row_rect(list_y + box_rect.height),
            avail,
        );
        field_rects[2] = Some(row_rect(list_y + box_rect.height + 1));
        bt_dropdown_row(
            f,
            focus,
            2,
            "代理",
            &proxy_label,
            row_rect(list_y + box_rect.height + 1),
        );
    }

    // ---------- trackers 多行输入框（可复用组件，无 wrap 模式） ----------
    // 高度 7 = 边框 2 + 5 可视行：逻辑行 1:1 渲染不做 word wrap，行内超宽由
    // 水平滚动条（下边框）承接，溢出 5 行由垂直滚动条（右边框）承接
    let trackers_rect = Rect {
        x: inner.x + 2,
        y: list_y + box_rect.height + 2,
        width: inner.width.saturating_sub(4),
        height: 7,
    };
    let trackers_focused = focus == trackers_focus;
    let trackers_hint = "可输入多行 traker，使用 ALT+ENTER 换行";
    let trackers_pressed_v = app.sb_arrow_pressed(crate::app::SbKey::BtTrackers);
    let trackers_pressed_h = app.sb_arrow_pressed(crate::app::SbKey::BtTrackersH);
    let tr = {
        let d = app.dialog.as_mut().unwrap();
        draw_multi_line_input(
            f,
            trackers_rect,
            Some(" trackers "),
            trackers_hint,
            trackers_focused,
            trackers_pressed_v,
            trackers_pressed_h,
            false,
            &mut d.trackers,
        )
    };
    app.dlg_trackers_sb = tr.sb;
    app.dlg_trackers_h_sb = tr.h_sb;
    app.dlg_trackers_area = Some(tr.inner);
    if let Some((cx, cy)) = tr.cursor {
        app.dlg_cursor_screen = Some((cx, cy));
    }
    field_rects[trackers_focus] = Some(trackers_rect);

    for i in 0..nfocus {
        if let Some(r) = field_rects[i] {
            app.dlg_field_rects.push((r, i));
            app.dlg_field_wins.push(field_wins[i].take());
        }
    }

    // ---------- 按钮行 ----------
    let btn_row = inner.y + inner.height - 2;
    let buttons: Vec<(&str, usize)> = if is_add {
        vec![("立即下载", 6), ("仅添加", 7), ("取消", 8)]
    } else {
        vec![("确定", 4), ("取消", 5)]
    };
    let btn_ws: Vec<usize> = buttons
        .iter()
        .map(|(txt, _)| w(&format!("[ {} ]", txt)) + 2)
        .collect();
    let total_w: usize = btn_ws.iter().sum::<usize>() + 2 * (buttons.len() - 1);
    let mut bx = inner.x as usize + ((inner.width as usize).saturating_sub(total_w)) / 2;
    let inner_right = inner.x as usize + inner.width as usize;
    let buf_right = f.area().width as usize;
    for ((txt, idx), bw) in buttons.iter().zip(btn_ws.iter()) {
        let focused = focus == *idx;
        let line = Line::from(button_spans(txt, focused));
        let width = (*bw)
            .min(inner_right.saturating_sub(bx))
            .min(buf_right.saturating_sub(bx));
        if width > 0 {
            let rect = Rect {
                x: bx as u16,
                y: btn_row,
                width: width as u16,
                height: 1,
            };
            f.render_widget(Paragraph::new(line), rect);
            app.dlg_btn_rects.push((rect, *idx));
        }
        bx += bw + 2;
    }

    // ---------- 代理下拉浮层 ----------
    if proxy_open {
        let items: Vec<String> = app.proxy_labels.clone();
        let pw = 26u16;
        let ph = items.len() as u16 + 2;
        let prect = dropdown_rect(area, inner, list_y + box_rect.height + 1, pw, ph);
        f.render_widget(Clear, prect);
        clip_wide_at_edges(f, prect);
        let pblock = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(panel_border_style(true))
            .title(Span::styled(" 选择代理 ", panel_title_style(true)));
        let pinner = pblock.inner(prect);
        f.render_widget(pblock, prect);
        for (i, name) in items.iter().enumerate() {
            let sel = i == proxy_sel;
            let bg = if sel { Color::DarkGray } else { Color::Reset };
            let line = Line::from(vec![
                Span::styled(
                    format!("  {} ", if sel { "▸" } else { " " }),
                    Style::default().fg(if sel { ACCENT } else { DIM2 }).bg(bg),
                ),
                Span::styled(
                    truncate(name, (pw as usize).saturating_sub(6)),
                    Style::default()
                        .fg(if sel { Color::White } else { FG })
                        .bg(bg)
                        .add_modifier(if sel {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
            ]);
            let irect = Rect {
                x: pinner.x,
                y: pinner.y + i as u16,
                width: pinner.width,
                height: 1,
            };
            f.render_widget(Paragraph::new(line).style(Style::default().bg(bg)), irect);
            app.dlg_proxy_rects.push((irect, i));
        }
    }
}
