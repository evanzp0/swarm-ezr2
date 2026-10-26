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
    match d.kind {
        DialogKind::Add => draw_task_dialog(f, app, area, DialogKind::Add),
        DialogKind::Modify => draw_task_dialog(f, app, area, DialogKind::Modify),
        DialogKind::Delete => draw_delete_dialog(f, app, area),
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
