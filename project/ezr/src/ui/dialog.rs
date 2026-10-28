use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use super::btn::{clip_wide_at_edges, draw_button_row, draw_close_btn, overlay_rect};
use super::delete::draw_delete_dialog;
use super::text::{pad_right, truncate, w};
use super::{panel_border_style, panel_title_style, ACCENT, DIM, DIM2, FG, YELLOW};
use crate::app::{App, DialogKind, FieldWin};
use crate::model::checksum::checksum_dropdown;
use crate::textfield::TextInput;

fn dropdown_rect(area: Rect, inner: Rect, type_row_y: u16, pw: u16, ph: u16) -> Rect {
    let mut py = type_row_y + 1;
    if py + ph > area.y + area.height {
        py = type_row_y.saturating_sub(ph);
    }
    let px = (inner.x + 9)
        .min(area.x + area.width.saturating_sub(pw + 1))
        .max(area.x + 1);
    let y = py.max(area.y);
    Rect {
        x: px,
        y,
        width: pw.min(area.x + area.width.saturating_sub(px)),
        height: ph.min(area.y + area.height.saturating_sub(y)),
    }
}

fn dropdown_frame(
    f: &mut Frame,
    area: Rect,
    inner: Rect,
    row_y: u16,
    pw: u16,
    ph: u16,
    title: &str,
) -> Rect {
    let prect = dropdown_rect(area, inner, row_y, pw, ph);
    f.render_widget(Clear, prect);
    clip_wide_at_edges(f, prect);
    let pblock = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(title, panel_title_style(true)));
    let pinner = pblock.inner(prect);
    f.render_widget(pblock, prect);
    pinner
}

fn dropdown_item(
    f: &mut Frame,
    pinner: Rect,
    i: usize,
    sel: usize,
    main: String,
    suffix: Option<String>,
    rects: &mut Vec<(Rect, usize)>,
) {
    let selected = i == sel;
    let bg = if selected {
        Color::DarkGray
    } else {
        Color::Reset
    };
    let mut spans = vec![
        Span::styled(
            format!("  {} ", if selected { "▸" } else { " " }),
            Style::default()
                .fg(if selected { ACCENT } else { DIM2 })
                .bg(bg),
        ),
        Span::styled(
            main,
            Style::default()
                .fg(if selected { Color::White } else { FG })
                .bg(bg)
                .add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ),
    ];
    if let Some(sfx) = suffix {
        spans.push(Span::styled(
            sfx,
            Style::default()
                .fg(if selected { ACCENT } else { DIM2 })
                .bg(bg),
        ));
    }
    let irect = Rect {
        x: pinner.x,
        y: pinner.y + i as u16,
        width: pinner.width,
        height: 1,
    };
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
        irect,
    );
    rects.push((irect, i));
}

fn push_selector_spans(spans: &mut Vec<Span<'static>>, label: &str, focused: bool) {
    spans.push(Span::styled(
        format!("{label} "),
        Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled(
        "▾",
        Style::default().fg(if focused { YELLOW } else { DIM }),
    ));
}

pub(super) fn draw_dropdown(
    f: &mut Frame,
    area: Rect,
    inner: Rect,
    anchor_y: u16,
    pw: u16,
    title: &str,
    sel: usize,
    items: &[(String, Option<String>)],
    rects: &mut Vec<(Rect, usize)>,
) {
    let ph = items.len() as u16 + 2;
    let pinner = dropdown_frame(f, area, inner, anchor_y, pw, ph, title);
    for (i, (main, suffix)) in items.iter().enumerate().take(pinner.height as usize) {
        dropdown_item(f, pinner, i, sel, main.clone(), suffix.clone(), rects);
    }
}

pub(super) fn draw_dialogs(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(d) = app.dialog.as_ref() else { return };
    app.dlg_btn_rects.clear();
    app.dlg_field_rects.clear();
    app.dlg_field_wins.clear();
    app.dlg_cursor_screen = None;
    app.dlg_ck_rects.clear();
    app.dlg_proxy_rects.clear();
    match d.kind {
        DialogKind::Add => draw_task_dialog(f, app, area, DialogKind::Add),
        DialogKind::Modify => draw_task_dialog(f, app, area, DialogKind::Modify),
        DialogKind::Delete => draw_delete_dialog(f, app, area),
        DialogKind::AddBt | DialogKind::ModifyBt => super::bt_dialog::draw_bt_dialog(f, app, area),
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

fn draw_task_dialog(f: &mut Frame, app: &mut App, area: Rect, kind: DialogKind) {
    let is_add = kind == DialogKind::Add;
    let (title, dw, dh) = if is_add {
        (" 添加任务：HTTP(S) ", 74u16, 12u16)
    } else {
        (" 修改任务：HTTP(S) ", 74, 10)
    };
    let dlg = {
        let r = overlay_rect(area, dw, dh, app.dlg_pos);
        if app.dlg_pos.is_none() {
            app.dlg_pos = Some((r.x, r.y));
        }
        app.dlg_area = Some(r);
        r
    };
    f.render_widget(Clear, dlg);
    clip_wide_at_edges(f, dlg);
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
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
    let entries = checksum_dropdown();
    let (algo_name, _algo_need) = entries[ck_type.min(entries.len() - 1)];
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
                push_selector_spans(&mut spans, algo_name, focused);
            }
            RowKind::ProxySel => {
                push_selector_spans(&mut spans, &proxy_label, focused);
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
    let buttons: Vec<(&str, usize)> = if is_add {
        vec![("立即下载", 6), ("仅添加", 7), ("取消", 8)]
    } else {
        vec![("确定", 4), ("取消", 5)]
    };
    draw_button_row(f, app, inner, inner.y + n_rows + 2, &buttons, focus);

    if ck_open {
        let items: Vec<(String, Option<String>)> = checksum_dropdown()
            .iter()
            .map(|(name, need)| {
                (
                    pad_right(name, 9),
                    Some(if *need == 0 {
                        "—".to_string()
                    } else {
                        format!("{need} 位")
                    }),
                )
            })
            .collect();
        draw_dropdown(
            f,
            area,
            inner,
            ck_row_y,
            24,
            " 校验算法 ",
            ck_sel,
            &items,
            &mut app.dlg_ck_rects,
        );
    }

    if proxy_open {
        let pw = 26u16;
        let items: Vec<(String, Option<String>)> = app
            .proxy_labels
            .iter()
            .map(|name| (truncate(name, (pw as usize).saturating_sub(6)), None))
            .collect();
        draw_dropdown(
            f,
            area,
            inner,
            proxy_row_y,
            pw,
            " 选择代理 ",
            proxy_sel,
            &items,
            &mut app.dlg_proxy_rects,
        );
    }
}

#[cfg(test)]
mod helper_tests {
    use super::*;

    #[test]
    fn dropdown_rect_prefers_below_and_clamps() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 30,
        };
        let inner = Rect {
            x: 14,
            y: 10,
            width: 70,
            height: 9,
        };
        let r = dropdown_rect(area, inner, 15, 24, 9);
        assert_eq!(r.y, 16);
        assert_eq!(r.x, 23);
        let r = dropdown_rect(area, inner, 26, 24, 9);
        assert_eq!(r.y, 17);
        let small = Rect {
            x: 0,
            y: 0,
            width: 30,
            height: 30,
        };
        let r = dropdown_rect(small, inner, 15, 24, 9);
        assert_eq!(r.x, 5);
        assert!(r.y >= area.y);
    }

    #[test]
    fn dropdown_rect_clamps_within_area() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 26,
            height: 40,
        };
        let inner = Rect {
            x: 1,
            y: 14,
            width: 24,
            height: 10,
        };
        let r = dropdown_rect(area, inner, 21, 26, 3);
        assert_eq!(r.x, 1, "px 仍触下限钳");
        assert_eq!(r.width, 25, "宽钳为 area.width - 1");
        assert!(r.right() <= area.right(), "右缘不得越界");
        let small = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 12,
        };
        let inner2 = Rect {
            x: 4,
            y: 1,
            width: 72,
            height: 10,
        };
        let r = dropdown_rect(small, inner2, 7, 26, 15);
        assert_eq!(r.y, 0, "上移展开后顶在 area.y");
        assert_eq!(r.height, 12, "高钳为终端高");
        assert!(r.bottom() <= small.bottom(), "下缘不得越界");
        let wide = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 30,
        };
        let r = dropdown_rect(wide, inner, 15, 24, 9);
        assert_eq!((r.width, r.height), (24, 9));
    }
}

#[cfg(test)]
mod tests {

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::super::testfx;

    #[tokio::test]
    async fn dialog_native_cursor_published_and_positioned() {
        let mut app = testfx::make_app_in("ezr-dlgcur", "cur");
        app.open_add_dialog();
        {
            let d = app.dialog.as_mut().unwrap();
            d.url.set("https://ezr.dev/");
            d.focus_field(0);
            d.url.set_cursor(8);
        }
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        assert_eq!(
            app.dlg_cursor_screen,
            Some((43, 16)),
            "聚焦文本行光标屏幕位（值区起点 + pre 宽）"
        );
        let pos = term.get_cursor_position().unwrap();
        assert_eq!(
            (pos.x, pos.y),
            (43, 16),
            "Terminal 收口把原生光标位下发后端"
        );
        let s = term.backend().to_string();
        assert!(!s.contains('▏'), "内联字形光标退役: {s}");
        assert!(s.contains("https://ezr.dev/"), "URL 原样显示: {s}");
        let win = app.dlg_field_wins[0].as_ref().expect("文本行 win 回填");
        assert_eq!(win.value_x, 35);
        assert_eq!((win.win_lo, win.win_hi), (0, 16));
        assert_eq!(win.cols.last().copied(), Some(16), "cols 末位 = 窗口总宽");
        assert!(app.dlg_field_wins[3].is_none(), "下拉行无窗口几何");
        app.dialog.as_mut().unwrap().url.set_cursor(2);
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        assert_eq!(app.dlg_cursor_screen, Some((37, 16)), "光标列随 cursor");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn dialog_first_frame_centers_and_backfills() {
        let mut app = testfx::make_app_in("ezr-dlgc", "center");
        app.open_add_dialog();
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        assert_eq!(app.dlg_pos, Some((13, 9)), "首帧居中并记忆");
        let a = app.dlg_area.expect("dlg_area 回填");
        assert_eq!((a.x, a.y, a.width, a.height), (13, 9, 74, 12));
        app.shutdown().await;
    }

    #[tokio::test]
    async fn dialog_dragged_position_wins_and_clamps() {
        let mut app = testfx::make_app_in("ezr-dlgp", "drag");
        app.open_add_dialog();
        app.dlg_pos = Some((90, 26));
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        assert_eq!(app.dlg_pos, Some((90, 26)), "位置记忆保留原始拖拽点");
        let a = app.dlg_area.expect("dlg_area 回填");
        assert_eq!(
            (a.x, a.y, a.width, a.height),
            (26, 18, 74, 12),
            "绘制矩形钳制终端内"
        );
        let s = term.backend().to_string();
        assert!(s.contains("添加任务：HTTP(S)"), "拖拽位置渲染: {s}");
        app.shutdown().await;
    }
}
