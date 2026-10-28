use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, DialogKind, FieldWin, SbKey};
use crate::textfield::TextInput;
use crate::ui::btn::{button_spans, clip_wide_at_edges, draw_close_btn, overlay_rect};
use crate::ui::dialog::draw_dropdown;
use crate::ui::multiline::draw_multi_line_input;
use crate::ui::text::{fmt_size, pad_right, truncate, w};
use crate::ui::tree::{draw_tree_panel, TreeRow};
use crate::ui::{panel_border_style, panel_title_style, ACCENT, DIM, DIM2, YELLOW};

#[allow(dead_code)]
pub(crate) fn draw_bt_dialog(f: &mut Frame, app: &mut App, area: Rect) {
    let kind = app
        .dialog
        .as_ref()
        .map(|d| d.kind)
        .unwrap_or(DialogKind::AddBt);
    let is_add = kind == DialogKind::AddBt;
    let (title, dw, dh) = if is_add {
        (" 添加任务：BT ", 78u16, 26u16)
    } else {
        (" 修改任务：BT ", 78, 24)
    };
    let nfocus = if is_add { 9usize } else { 6usize };
    let list_focus = if is_add { 2usize } else { 0usize };
    let trackers_focus = if is_add { 5usize } else { 3usize };

    app.dlg_btfile_rects.clear();
    app.dlg_btbtn_rects.clear();
    app.dlg_bt_sb = None;
    app.dlg_bt_area = None;
    app.dlg_trackers_sb = None;
    app.dlg_trackers_h_sb = None;
    app.dlg_trackers_area = None;

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

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(title, panel_title_style(true)));
    let inner = block.inner(dlg);
    f.render_widget(block, dlg);

    app.popup_close_rect = draw_close_btn(f, app, dlg, w(title));
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

    let list_y = if is_add { inner.y + 3 } else { inner.y + 2 };
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
        if d.bt_follow {
            d.bt_scroll_follow(list_rows);
            d.bt_follow = false;
        }
        let max = d.bt_row_count().saturating_sub(list_rows.max(1));
        d.bt_scroll = d.bt_scroll.min(max);
    }
    let (tree_rows, bt_cursor, bt_scroll, sel_cnt, sel_bytes) = {
        let d = app.dialog.as_ref().unwrap();
        let rows: Vec<TreeRow> = d
            .bt_rows()
            .iter()
            .map(|(_, n)| TreeRow {
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
    let tree_title = " 选择下载文件 ";
    let tree_caption = format!("已选 {} 个文件，共 {}", sel_cnt, fmt_size(sel_bytes));
    let tree = draw_tree_panel(
        f,
        box_rect,
        tree_title,
        &tree_caption,
        list_focused,
        app.sb_arrow_pressed(SbKey::BtFiles),
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

    {
        let right = box_rect.x + box_rect.width.saturating_sub(1);
        let none_x = right.saturating_sub(8);
        let all_x = none_x.saturating_sub(1 + 6);
        for (txt, bx, bw, idx) in [
            ("全选", all_x, 6u16, 0usize),
            ("全不选", none_x, 8u16, 1usize),
        ] {
            if bx <= box_rect.x + 1 {
                continue;
            }
            let r = Rect {
                x: bx,
                y: box_rect.y,
                width: bw,
                height: 1,
            };
            let hot = app.pointer.is_some_and(|(px, py)| {
                px >= r.x
                    && px < r.x.saturating_add(r.width)
                    && py >= r.y
                    && py < r.y.saturating_add(r.height)
            });
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

    let trackers_rect = Rect {
        x: inner.x + 2,
        y: list_y + box_rect.height + 2,
        width: inner.width.saturating_sub(4),
        height: 7,
    };
    let trackers_focused = focus == trackers_focus;
    let trackers_hint = "可输入多行 traker，使用 ALT+ENTER 换行";
    let trackers_pressed_v = app.sb_arrow_pressed(SbKey::BtTrackers);
    let trackers_pressed_h = app.sb_arrow_pressed(SbKey::BtTrackersH);
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

    if proxy_open {
        let items: Vec<(String, Option<String>)> = app
            .proxy_labels
            .iter()
            .map(|name| (truncate(name, 20), None))
            .collect();
        draw_dropdown(
            f,
            area,
            inner,
            list_y + box_rect.height + 1,
            26,
            " 选择代理 ",
            proxy_sel,
            &items,
            &mut app.dlg_proxy_rects,
        );
    }
}

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

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;
    use crate::bt::metainfo::TorrentMeta;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-uibtdlg", tag)
    }

    fn write_multi_torrent(dir: &std::path::Path, name: &str) -> String {
        let pieces = [0u8; 20];
        let mut buf = Vec::new();
        buf.extend_from_slice(b"d8:announce14:udp://seed/ann");
        buf.extend_from_slice(b"13:announce-listl");
        buf.extend_from_slice(b"l12:udp://l1/annee");
        buf.extend_from_slice(b"4:infod5:filesl");
        buf.extend_from_slice(b"d6:lengthi5e4:pathl3:sub5:a.txtee");
        buf.extend_from_slice(b"d6:lengthi7e4:pathl5:b.binee");
        buf.extend_from_slice(b"e4:name3:tor12:piece lengthi4e6:pieces20:");
        buf.extend_from_slice(&pieces);
        buf.push(b'e');
        buf.push(b'e');
        let path = dir.join(name);
        std::fs::write(&path, &buf).unwrap();
        path.to_string_lossy().to_string()
    }

    #[tokio::test]
    async fn add_bt_dialog_renders_and_backfills() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-uibtdlg-add");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = TorrentMeta::parse(&std::fs::read(&path).unwrap()).unwrap();
        let mut app = make_app("add");
        app.open_add_bt_dialog_with(&path, &meta);
        let mut term = Terminal::new(TestBackend::new(110, 36)).unwrap();
        term.draw(|f| crate::ui::dialog::draw_dialogs(f, &mut app, f.area()))
            .unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("添加任务：BT"), "标题: {s}");
        assert!(s.contains("种子"), "种子行: {s}");
        assert!(s.contains("保存到"), "保存到行: {s}");
        assert!(s.contains("选择下载文件"), "文件列表标题: {s}");
        assert!(s.contains("已选 2 个文件，共 12 B"), "动态统计: {s}");
        assert!(s.contains("trackers"), "trackers 标题: {s}");
        assert!(
            s.contains("立即下载") && s.contains("仅添加") && s.contains("取消"),
            "按钮行: {s}"
        );
        assert!(s.contains("全选") && s.contains("全不选"), "右上按钮: {s}");
        assert!(s.contains("tor/"), "目录行（默认收拢仅根层）: {s}");
        assert_eq!(app.dlg_btfile_rects.len(), 1, "收拢态 1 可见行热区");
        assert_eq!(app.dlg_btbtn_rects.len(), 2, "全选/全不选热区");
        assert_eq!(app.dlg_bt_visible, 7, "Add 列表 7 可视行");
        assert!(app.dlg_bt_sb.is_none(), "1 行不超页无滚动条");
        assert!(app.dlg_trackers_area.is_some(), "trackers 内容区回填");
        assert!(app.dlg_bt_area.is_some());
        let field_focuses: Vec<usize> = app.dlg_field_rects.iter().map(|(_, i)| *i).collect();
        assert_eq!(field_focuses, vec![0, 1, 2, 3, 4, 5], "焦点表矩形回填");
        assert_eq!(app.dlg_btn_rects.len(), 3, "三按钮热区");
        let a = app.dlg_area.unwrap();
        assert_eq!((a.x, a.y, a.width, a.height), (16, 5, 78, 26));
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn modify_bt_dialog_renders_and_backfills() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-uibtdlg-mod");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = TorrentMeta::parse(&std::fs::read(&path).unwrap()).unwrap();
        let mut app = make_app("mod");
        app.open_add_bt_dialog_with(&path, &meta);
        app.dlg_confirm_add_bt(false);
        app.selected = 0;
        app.modify_dialog_entry();
        assert_eq!(app.dialog.as_ref().unwrap().kind, DialogKind::ModifyBt);
        let mut term = Terminal::new(TestBackend::new(110, 36)).unwrap();
        term.draw(|f| crate::ui::dialog::draw_dialogs(f, &mut app, f.area()))
            .unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("修改任务：BT"), "标题: {s}");
        assert!(s.contains("任务 "), "任务名静态行: {s}");
        assert!(s.contains("tor"), "任务名值: {s}");
        assert!(s.contains("确定") && s.contains("取消"), "两按钮: {s}");
        assert!(!s.contains("立即下载"), "修改框无立即下载: {s}");
        assert_eq!(app.dlg_bt_visible, 6, "Modify 列表 6 可视行");
        let field_focuses: Vec<usize> = app.dlg_field_rects.iter().map(|(_, i)| *i).collect();
        assert_eq!(field_focuses, vec![0, 1, 2, 3], "焦点表 0–3 矩形回填");
        assert_eq!(app.dlg_btn_rects.len(), 2);
        assert!(s.contains("udp://seed/ann"), "trackers 预填渲染: {s}");
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn proxy_dropdown_overlay_renders() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-uibtdlg-dd");
        let path = write_multi_torrent(&dir, "a.torrent");
        let meta = TorrentMeta::parse(&std::fs::read(&path).unwrap()).unwrap();
        let mut app = make_app("dd");
        app.open_add_bt_dialog_with(&path, &meta);
        app.dialog.as_mut().unwrap().proxy_open = true;
        let mut term = Terminal::new(TestBackend::new(110, 36)).unwrap();
        term.draw(|f| crate::ui::dialog::draw_dialogs(f, &mut app, f.area()))
            .unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("选择代理"), "下拉标题: {s}");
        assert!(s.contains("直连"), "代理选项: {s}");
        assert_eq!(
            app.dlg_proxy_rects.len(),
            app.proxy_labels.len(),
            "选项热区"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
