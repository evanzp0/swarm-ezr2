//! dialog — 对话框与浮层（添加/删除、校验算法下拉、宽字符边界处理）

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::btn::{clip_wide_at_edges, dialog_rect, draw_button_row};
use super::delete::draw_delete_dialog;
use super::text::{pad_right, truncate, w};
use super::{ACCENT, DIM, DIM2, FG, YELLOW};
use crate::app::{App, DialogKind, CHECKSUM_ALGOS};

/// 字段行显示值：空值显示占位提示；超宽显示尾部（…+末尾字符，便于核对校验码结尾）
fn field_display(val: &str, ph: &str, avail: usize) -> String {
    if val.is_empty() {
        truncate(ph, avail)
    } else if w(val) > avail {
        let keep = avail.saturating_sub(1);
        format!(
            "…{}",
            val.chars()
                .skip(val.chars().count() - keep)
                .collect::<String>()
        )
    } else {
        val.to_string()
    }
}

/// 校验算法下拉浮层定位：优先展开在校验算法行下方；超出终端底部则改为行上方；
/// 横向钳制在终端内（左右各留 1 列边距）；宽/高最终钳制在终端内——窄终端
/// （宽 ≤ pw）时 px 触下限钳后右缘会越界、浮层高于终端时下缘同理，
/// 不钳制则 ratatui Clear/Block 渲染越界 panic（coder v116 缺陷修复），
/// 钳制只截去越界部分、常态几何不变。
fn dropdown_rect(area: Rect, inner: Rect, type_row_y: u16, pw: u16, ph: u16) -> Rect {
    let mut py = type_row_y + 1;
    if py + ph > area.y + area.height {
        py = type_row_y.saturating_sub(ph);
    }
    let px = (inner.x + 8)
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

/// 下拉浮层框架（架构轮单源：算法/代理下拉共用）：定位 + Clear + 宽字符
/// 边缘裁剪 + 圆角边框块。返回选项内框，逐行渲染由调用方按各自行内容继续。
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
    let pblock = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            title,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
    let pinner = pblock.inner(prect);
    f.render_widget(pblock, prect);
    pinner
}

/// 下拉选项行渲染（架构轮单源）：选中高亮 + ▸ 指针 + 主体文本 + 可选后缀，
/// 逐行回填命中区域（鼠标点击选择）。渲染字节与原两处内联形态一致。
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
            format!(" {} ", if selected { "▸" } else { " " }),
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

/// 下拉浮层整体渲染（architect v116 收口：校验算法/代理两处仅 (选项集, 宽,
/// 标题, 选中项, 命中区域表) 不同——框架定位 + 逐项渲染 + 钳短裁剪收口为
/// 单一函数，选项内容由调用方以 (主体文本, 可选后缀) 对传入）
fn draw_dropdown(
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
    // 浮层被终端钳短时只渲染可见行（coder v116：选项行不得越出钳后内框）
    for (i, (main, suffix)) in items.iter().enumerate().take(pinner.height as usize) {
        dropdown_item(f, pinner, i, sel, main.clone(), suffix.clone(), rects);
    }
}

pub(super) fn draw_dialogs(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(d) = app.dialog.as_ref() else { return };
    app.dlg_btn_rects.clear();
    app.dlg_field_rects.clear();
    app.dlg_ck_rects.clear();
    app.dlg_proxy_rects.clear();
    match d.kind {
        DialogKind::Add => draw_task_dialog(f, app, area, DialogKind::Add),
        DialogKind::Modify => draw_task_dialog(f, app, area, DialogKind::Modify),
        DialogKind::Delete => draw_delete_dialog(f, app, area),
    }
}

/// 任务对话框行种类（文本输入 / 校验算法下拉 / 代理下拉）
enum RowKind {
    Text,
    CkSel,
    ProxySel,
}

struct Row {
    label: &'static str,
    value: String,
    placeholder: String,
    kind: RowKind,
}

/// 添加/修改任务对话框统一渲染（v1.5/FR-01-86/87）：字段行 + 按钮行 + 提示行
/// + 算法/代理下拉浮层。
///
/// * Add —— URL/保存到/并发/校验/校验码/代理（确认 6 取消 7）
/// * Modify —— 并发/校验/校验码/代理（确认 4 取消 5）
fn draw_task_dialog(f: &mut Frame, app: &mut App, area: Rect, kind: DialogKind) {
    let is_add = kind == DialogKind::Add;
    let (title, dw, dh, btn_confirm, btn_cancel) = if is_add {
        (" 添加下载任务 ", 74u16, 12u16, 6usize, 7usize)
    } else {
        (" 修改任务 ", 74, 10, 4, 5)
    };
    let dlg = dialog_rect(area, dw, dh);
    f.render_widget(Clear, dlg);
    clip_wide_at_edges(f, dlg);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            title,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(dlg);
    f.render_widget(block, dlg);
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
    let (algo_name, algo_need, _) = CHECKSUM_ALGOS[ck_type.min(CHECKSUM_ALGOS.len() - 1)];
    let proxy_label = app
        .proxy_labels
        .get(proxy_sel)
        .cloned()
        .unwrap_or_else(|| "直连".to_string());

    // 字段行构建（Add 六行 / Modify 四行；校验与代理为下拉选择行）
    let mut rows: Vec<Row> = Vec::new();
    if is_add {
        rows.push(Row {
            label: "URL",
            value: url,
            placeholder: "https://example.com/file.zip".to_string(),
            kind: RowKind::Text,
        });
        rows.push(Row {
            label: "保存到",
            value: dir,
            placeholder: "/srv/downloads".to_string(),
            kind: RowKind::Text,
        });
    }
    rows.push(Row {
        label: "并发",
        value: conns,
        placeholder: "4".to_string(),
        kind: RowKind::Text,
    });
    rows.push(Row {
        label: "校验",
        value: String::new(),
        placeholder: String::new(),
        kind: RowKind::CkSel,
    });
    rows.push(Row {
        label: "校验码",
        value: ck_value,
        placeholder: format!("{} 位十六进制（可留空）", algo_need),
        kind: RowKind::Text,
    });
    rows.push(Row {
        label: "代理",
        value: String::new(),
        placeholder: String::new(),
        kind: RowKind::ProxySel,
    });

    let avail = (inner.width as usize).saturating_sub(12);
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
                if focused {
                    spans.push(Span::styled("  Enter 选择算法", Style::default().fg(DIM2)));
                }
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
                spans.push(Span::styled(
                    field_display(&row.value, &row.placeholder, avail),
                    Style::default().fg(if row.value.is_empty() {
                        DIM2
                    } else {
                        Color::White
                    }),
                ));
                if focused {
                    spans.push(Span::styled(
                        "▏",
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                    ));
                }
            }
        }
        let rect = Rect {
            x: inner.x + 1,
            y: row_y,
            width: inner.width.saturating_sub(2),
            height: 1,
        };
        f.render_widget(Paragraph::new(Line::from(spans)), rect);
        // 回填字段行命中区域（鼠标点击聚焦；下拉行点击展开）
        app.dlg_field_rects.push((rect, i));
    }

    let n_rows = rows.len() as u16;
    // 按钮行：[ 确认 ] [ 取消 ]
    draw_button_row(
        f,
        app,
        inner,
        inner.y + n_rows + 2,
        &[
            (if is_add { "确认" } else { "确定" }, btn_confirm),
            ("取消", btn_cancel),
        ],
        focus,
    );

    // 提示行（下拉框展开时切换为列表操作提示）
    let hint = if ck_open {
        " ↑↓ 选择算法 · Enter 确认选择 · Esc 关闭列表"
    } else if proxy_open {
        " ↑↓ 选择代理 · Enter 确认选择 · Esc 关闭列表"
    } else if is_add {
        " Enter 确认 · Tab/↑↓ 切换 · Esc 取消 · 校验码留空 = 不校验"
    } else {
        " Enter 确定 · Tab/↑↓ 切换 · Esc 取消 · 确定后立即生效"
    };
    f.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(DIM2))),
        Rect {
            x: inner.x + 1,
            y: inner.y + n_rows + 3,
            width: inner.width.saturating_sub(2),
            height: 1,
        },
    );

    // 校验算法下拉框（浮层最后绘制，覆盖对话框与下层内容；架构轮单源的
    // dropdown_frame/dropdown_item + architect v116 整体渲染收口 draw_dropdown）
    if ck_open {
        let items: Vec<(String, Option<String>)> = CHECKSUM_ALGOS
            .iter()
            .map(|(name, need, _)| (pad_right(name, 9), Some(format!("{need} 位"))))
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

    // 代理下拉框（v1.5/FR-01-86：直连 + 默认代理 + 命名条目；不含认证信息）
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
    fn field_display_empty_long_and_normal() {
        assert_eq!(field_display("", "占位提示", 20), truncate("占位提示", 20));
        // 超宽：尾部省略（… + 末尾 keep 个字符）
        let long = "abcdef1234567890";
        let shown = field_display(long, "", 6);
        assert!(shown.starts_with('…'));
        assert!(shown.contains("890"));
        // 正常宽度：原样
        assert_eq!(field_display("abc", "", 6), "abc");
    }

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
        // 下方放得下：type_row_y + 1
        let r = dropdown_rect(area, inner, 15, 24, 9);
        assert_eq!(r.y, 16);
        assert_eq!(r.x, 22); // inner.x + 8
                             // 底部放不下：改为上方展开
        let r = dropdown_rect(area, inner, 26, 24, 9);
        assert_eq!(r.y, 17); // 26 - 9
                             // 横向钳制：窄终端
        let small = Rect {
            x: 0,
            y: 0,
            width: 30,
            height: 30,
        };
        let r = dropdown_rect(small, inner, 15, 24, 9);
        assert_eq!(r.x, 5); // max(area.x + 1)
                            // y 不低于浮层顶
        assert!(r.y >= area.y);
    }

    /// 缺陷回归（coder v116）：浮层 Rect 必须整体钳制在终端 area 内。
    /// ①26 列 + 代理下拉（pw=26）：px 触下限钳后右缘 27>26，Clear 渲染
    /// panic——宽钳为 area 右缘内（25）；②浮层高于终端（多代理 ph>高）：
    /// 上移展开后下缘越界——高钳为 area 底缘内。
    #[test]
    fn dropdown_rect_clamps_within_area() {
        // ① 横向：26 列终端，pw=26 → 宽钳为 25，右缘不越界
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
        // ② 纵向：ph=15 > 终端高 12 → 高钳为 12，下缘不越界
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
        // 常态不受钳制影响：宽高原样保留
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
