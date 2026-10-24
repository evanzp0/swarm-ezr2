//! dialog — 对话框与浮层（添加/删除、校验算法下拉、宽字符边界处理）

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

/// 字段行取窗与点击/光标映射单源（v1.34 三批 bug 修订：field_display 与
/// cursor_field_display 两同构实现收口）。返回 (光标前段, 光标后段, 窗口几何)：
/// 二段拼接 = 显示内容，取窗以锚点为准——
/// - 聚焦：锚 = 光标（[`crate::app::Dialog::cursor`]），pre/post 在锚处切分，
///   光标屏幕列 = 值区起点 + w(pre)，由调用方发布
///   [`crate::app::App::dlg_cursor_screen`]（终端原生光标，移动零位移、可
///   闪烁；原内联「▏」字形占列导致后续文本随光标整体平移，退役）；
/// - 未聚焦：锚 = 字段尾（= 原 field_display 尾窗语义，宽度口径修正为
///   CJK 原子不溢出），pre 恒空、post = 整窗。
///
/// 预算内整值显示；超宽以锚点取窗（先向右取满、余量向左回退；裁剪侧 …
/// 指示占列，预算扣减重取至不动点；budget < 3 放弃指示符）；空值显示
/// 占位提示（截断，窗口空：cols = [0]，点击任意处光标 0）。
///
/// FieldWin.cols/win_lo/win_hi = 点击列→字符偏移映射与光标定位共用几何。
fn field_window(
    val: &str,
    ph: &str,
    budget: usize,
    cursor: usize,
    focused: bool,
) -> (String, String, FieldWin) {
    let budget = budget.max(1);
    let mut win = FieldWin {
        value_x: 0,
        left_cut: false,
        cols: Vec::new(),
        win_lo: 0,
        win_hi: 0,
    };
    // cols 累积列回填（字符宽度原子累加，末位 = 窗口总宽）
    fn fill_cols(win: &mut FieldWin, widths: &[usize]) {
        win.cols.push(0);
        let mut acc = 0u16;
        for &cw in widths {
            acc += cw as u16;
            win.cols.push(acc);
        }
    }
    if val.is_empty() {
        win.cols.push(0);
        return (String::new(), truncate(ph, budget), win);
    }
    let chars: Vec<char> = val.chars().collect();
    let widths: Vec<usize> = chars.iter().map(|c| w(&c.to_string())).collect();
    let total: usize = widths.iter().sum();
    let anchor = if focused {
        cursor.min(chars.len())
    } else {
        chars.len()
    };
    if total <= budget {
        win.win_lo = 0;
        win.win_hi = chars.len();
        fill_cols(&mut win, &widths);
        return if focused {
            (
                chars[..anchor].iter().collect(),
                chars[anchor..].iter().collect(),
                win,
            )
        } else {
            (String::new(), val.to_string(), win)
        };
    }
    // 超宽取窗：两轮不动点（指示符占列 → 扣减预算重取，直至裁剪侧数稳定）
    let mut reserve = 0usize;
    loop {
        let b = budget.saturating_sub(reserve).max(1);
        // 向右取满，余量向左回退（字符原子：宽字符放不下整列则不取）
        let mut acc = 0usize;
        let mut hi = anchor;
        while hi < chars.len() && acc + widths[hi] <= b {
            acc += widths[hi];
            hi += 1;
        }
        let mut lo = anchor;
        while lo > 0 && acc + widths[lo - 1] <= b {
            lo -= 1;
            acc += widths[lo];
        }
        let left_cut = lo > 0;
        let right_cut = hi < chars.len();
        let need = usize::from(left_cut) + usize::from(right_cut);
        if need == reserve || budget < 3 {
            win.win_lo = lo;
            win.win_hi = hi;
            win.left_cut = left_cut;
            fill_cols(&mut win, &widths[lo..hi]);
            let pre = if focused {
                if left_cut {
                    format!("…{}", chars[lo..anchor].iter().collect::<String>())
                } else {
                    chars[lo..anchor].iter().collect()
                }
            } else if left_cut {
                "…".to_string()
            } else {
                String::new()
            };
            let post = if focused {
                if right_cut {
                    format!("{}…", chars[anchor..hi].iter().collect::<String>())
                } else {
                    chars[anchor..hi].iter().collect()
                }
            } else if right_cut {
                format!("{}…", chars[lo..hi].iter().collect::<String>())
            } else {
                chars[lo..hi].iter().collect()
            };
            return (pre, post, win);
        }
        reserve = need;
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
    // v1.34/FR-01-115：px = 内框 + 9 = 字段行左 2 列 padding 后「> 」指示符所在列
    //（与字段行相对对齐随 S2 同步 +1，原 +8）
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
    let pblock = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(title, panel_title_style(true)));
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
        // v1.34/FR-01-115 ⑤ 特例表：下拉浮层选项行仅执行 S2 左 2 列前缀缩进
        //（高度「项数 + 2」紧凑特例不变；原 1 列前缀 +2）
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

/// 下拉选择行值片段（cleaner v129 收口：校验/代理两同构臂共用——黄色加粗值 +
/// ▾ 指针（聚焦黄/失焦灰）；渲染字节与原两处内联形态一致）
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
    app.dlg_field_wins.clear();
    app.dlg_cursor_screen = None;
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

/// 添加/修改任务对话框统一渲染（v1.5/FR-01-86/87；v1.15/FR-01-103 三钮；
/// v1.30 修订；v1.34 padding 统一 FR-01-115）：字段行 + 按钮行 + 算法/代理下拉浮层。
/// v1.34：对话框 74×12（Add）/ 74×10（Modify）（内框 = 顶 1 + 字段行 + 空 1 +
/// 按钮 1 + 底 1——字段行/按钮行 y 不变、仅底部补齐 1 行 padding；字段行 x+2
/// 宽 −4，S2）；右上角 [✗] 关闭按钮（FR-01-114，原地松开 = 取消）；模态浮层
/// 恒定绿色加粗边框。
///
/// * Add —— URL/保存到/并发/校验/校验码/代理（立即下载 6 仅添加 7 取消 8）
/// * Modify —— 并发/校验/校验码/代理（确定 4 取消 5）
fn draw_task_dialog(f: &mut Frame, app: &mut App, area: Rect, kind: DialogKind) {
    let is_add = kind == DialogKind::Add;
    // v1.32/FR-01-01/FR-01-87 修订（对齐 ezr-demo m3）：标题更名——
    // ` 添加下载任务 ` → ` 添加任务：HTTP(S) `、` 修改任务 ` →
    // ` 修改任务：HTTP(S) `；v1.34/FR-01-115：尺寸 74×12 / 74×10
    //（padding 统一：内框高 +1 底部补齐，字段行/按钮行 y 不变）
    let (title, dw, dh) = if is_add {
        (" 添加任务：HTTP(S) ", 74u16, 12u16)
    } else {
        (" 修改任务：HTTP(S) ", 74, 10)
    };
    let dlg = {
        // v133/D38：拖拽位置优先（钳制终端内），首帧居中并记忆；区域热区
        // 同步回填（鼠标非交互区拖拽起点命中测试）
        let r = overlay_rect(area, dw, dh, app.dlg_pos);
        if app.dlg_pos.is_none() {
            app.dlg_pos = Some((r.x, r.y));
        }
        app.dlg_area = Some(r);
        r
    };
    f.render_widget(Clear, dlg);
    clip_wide_at_edges(f, dlg);
    // 模态浮层恒定绿色加粗边框/标题（与主面板聚焦色统一，v1.30/FR-01-108）
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(title, panel_title_style(true)));
    let inner = block.inner(dlg);
    f.render_widget(block, dlg);
    // 右上角 [✗]（v1.34/FR-01-114：全部弹窗统一；原地松开 = 取消对话框，
    // Esc 同口径；先于下拉浮层绘制，展开下拉时被覆盖、点击先收起下拉）
    app.popup_close_rect = draw_close_btn(f, app, dlg, w(title));
    let min_h = if is_add { 10 } else { 8 };
    if inner.width < 24 || inner.height < min_h {
        return;
    }

    let (focus, url, dir, conns, ck_type, ck_value, ck_open, ck_sel, proxy_sel, proxy_open, cursor) = {
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
            d.cursor,
        )
    };
    // 校验下拉当前项（v1.32：下拉 8 项含首行 NONE——（显示名，期望位数）
    // 由 model::checksum 单源派生；期望位数仅校验码占位提示使用，占位提示
    // 已随 v1.32/P3 删除，此处仅取显示名）
    let entries = checksum_dropdown();
    let (algo_name, _algo_need) = entries[ck_type.min(entries.len() - 1)];
    let proxy_label = app
        .proxy_labels
        .get(proxy_sel)
        .cloned()
        .unwrap_or_else(|| "直连".to_string());

    // 字段行构建（Add 六行 / Modify 四行；校验与代理为下拉选择行）。
    // v1.32/P3（提示信息删除，对齐 ezr-demo m3）：URL 字段占位提示、校验码
    // 字段占位提示（原「{N} 位十六进制（可留空）」）全部移除（交互说明收敛
    // 帮助窗口）；保存到/并发占位提示保留（demo 同口径）
    let mut rows: Vec<Row> = Vec::new();
    if is_add {
        rows.push(Row {
            label: "URL",
            value: url,
            placeholder: String::new(),
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
        placeholder: String::new(),
        kind: RowKind::Text,
    });
    rows.push(Row {
        label: "代理",
        value: String::new(),
        placeholder: String::new(),
        kind: RowKind::ProxySel,
    });

    // 值显示宽预算：字段行 rect = x+2 宽−4（v1.34/FR-01-115 S2）内再扣
    // label 7 列 + 指示符 "> " 2 列 → inner.width − 13（原宽−2 时代为 −12，
    // padding 改版后同步 −1 防溢出 rect 右缘 1 列被 Paragraph 截尾）
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
        // 文本行取窗单源（field_window）：聚焦/未聚焦同源；窗口几何与光标位
        // 在 rect 求得后回填（字段热区平行表 + 原生光标屏幕位）
        let mut text_win: Option<FieldWin> = None;
        let mut pre_cols = 0u16;
        match row.kind {
            RowKind::CkSel => {
                push_selector_spans(&mut spans, algo_name, focused);
                // v1.32/P3：校验行聚焦尾注 `  Enter 选择算法` 删除
                //（交互说明收敛帮助窗口，对齐 ezr-demo m3）
            }
            RowKind::ProxySel => {
                push_selector_spans(&mut spans, &proxy_label, focused);
            }
            RowKind::Text => {
                // 原生光标渲染（v1.34 三批 bug 修订）：pre/post 在光标处
                // 切分直接拼接显示（不再插入「▏」字形占列——后续文本不再
                // 随光标移动平移），终端原生光标由 ui::draw 收口绘制
                let (pre, post, win) =
                    field_window(&row.value, &row.placeholder, avail, cursor, focused);
                let base = Style::default().fg(if row.value.is_empty() {
                    DIM2
                } else {
                    Color::White
                });
                if focused {
                    pre_cols = w(&pre) as u16;
                    spans.push(Span::styled(pre, base));
                    spans.push(Span::styled(post, base));
                } else {
                    spans.push(Span::styled(format!("{pre}{post}"), base));
                }
                text_win = Some(win);
            }
        }
        let rect = Rect {
            // v1.34/FR-01-115 S2：字段行 x+2、宽 −4（内容区左右各 2 列）
            x: inner.x + 2,
            y: row_y,
            width: inner.width.saturating_sub(4),
            height: 1,
        };
        f.render_widget(Paragraph::new(Line::from(spans)), rect);
        // 回填字段行命中区域（鼠标点击聚焦/定位光标；下拉行点击展开）与
        // 文本字段窗口几何（点击列→字符偏移映射单源载荷；value_x = rect.x
        // + label 7 列 + "> " 2 列）；聚焦行另发布原生光标屏幕位
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
    // 按钮行（FR-01-103）：Add = [ 立即下载 ] [ 仅添加 ] [ 取消 ]（焦点 6/7/8）；
    // Modify = [ 确定 ] [ 取消 ]（焦点 4/5）
    let buttons: Vec<(&str, usize)> = if is_add {
        vec![("立即下载", 6), ("仅添加", 7), ("取消", 8)]
    } else {
        vec![("确定", 4), ("取消", 5)]
    };
    draw_button_row(f, app, inner, inner.y + n_rows + 2, &buttons, focus);

    // v1.30/FR-01-107：底部快捷键提示行删除（P3 原则——原提示行位于
    // n_rows + 3；下拉框操作提示同步移除，交互说明统一收敛 H 帮助窗口）

    // 校验算法下拉框（浮层最后绘制，覆盖对话框与下层内容；架构轮单源的
    // dropdown_frame/dropdown_item + architect v116 整体渲染收口 draw_dropdown）。
    // v1.32/FR-01-01：下拉 8 项 = 首行 NONE（右列 — 占位，不校验）+ 7 算法
    //（各右列显示期望位数）
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

    /// v1.34 三批：field_window 单源取窗三态（不超宽/超宽取窗/空值占位）
    /// ——聚焦/未聚焦锚点、拼接 = 显示内容、光标恒在窗内、CJK 原子不切半、
    /// cols 累积列与窗口边界自洽
    #[test]
    fn field_window_windows_and_splits() {
        // 聚焦不超宽：整值显示，pre 宽 = 光标列，窗口 = 全值
        let (pre, post, win) = field_window("abcdef", "", 10, 2, true);
        assert_eq!(pre, "ab");
        assert_eq!(post, "cdef");
        assert_eq!(w(&pre) + w(&post), 6);
        assert_eq!(win.win_lo, 0);
        assert_eq!(win.win_hi, 6);
        assert!(!win.left_cut);
        assert_eq!(win.cols, vec![0, 1, 2, 3, 4, 5, 6], "累积列含末位总宽");
        // 未聚焦同值：锚在字段尾 → 整窗、pre 恒空
        let (pre, post, win) = field_window("abcdef", "", 10, 0, false);
        assert!(pre.is_empty() && post == "abcdef");
        assert_eq!((win.win_lo, win.win_hi), (0, 6));
        // 聚焦超宽（预算 6，值宽 16）：光标 8 为锚 → 窗内含光标、双缘 … 指示
        let val = "abcdefghijklmnop";
        let (pre, post, win) = field_window(val, "", 6, 8, true);
        assert!(
            pre.starts_with('…') && post.ends_with('…'),
            "双缘裁剪指示: {pre}|{post}"
        );
        assert_eq!(w(&pre) + w(&post), 6, "总宽 = 预算");
        assert!(win.left_cut, "左缘裁剪指示发布");
        assert_eq!(
            win.cols.len(),
            win.win_hi - win.win_lo + 1,
            "cols = 窗内字符数 + 1"
        );
        // 光标可见性：pre+post 还原后含 val[8]（'i'），且非占位空窗
        let joined = format!("{pre}{post}");
        assert!(joined.contains('i'), "窗内含光标字符: {joined}");
        // 光标在尾：窗贴右（无尾部 …）
        let (pre, post, win) = field_window(val, "", 6, 16, true);
        assert!(!post.ends_with('…'), "光标在尾右缘不裁: {pre}|{post}");
        assert!(pre.starts_with('…'));
        assert_eq!(win.win_hi, 16);
        // 光标在 0：窗贴左（无首部 …）
        let (pre, post, win) = field_window(val, "", 6, 0, true);
        assert!(!pre.starts_with('…'), "光标在首左缘不裁: {pre}|{post}");
        assert!(post.ends_with('…'));
        assert_eq!(win.win_lo, 0);
        // CJK 原子：宽 4 预算 3 → 单汉字 2 列可取，不切半
        let (pre, post, _win) = field_window("中文中文", "", 3, 4, true);
        assert_eq!(
            w(&pre) + w(&post),
            3,
            "CJK 原子取窗（汉字 2 列 + … 1 列 = 预算）: {pre}|{post}"
        );
        // CJK cols 累积：宽字符占 2 列
        let (_, _, win) = field_window("中文", "", 10, 2, true);
        assert_eq!(win.cols, vec![0, 2, 4], "CJK 宽字符累积列");
        // 未聚焦超宽：尾窗语义（= 原 field_display：… + 预算内尾部）——
        // 指示符在 pre、窗口内容在 post，拼接 = 原显示形态
        let (pre, post, win) = field_window("abcdef1234567890", "", 6, 0, false);
        assert_eq!(pre, "…");
        assert_eq!(post, "67890");
        assert_eq!((win.win_lo, win.win_hi), (11, 16));
        assert!(win.left_cut);
        // 未聚焦超宽 CJK：宽度口径（原 field_display 按字符数计会溢出）——
        // 预算 6 尾窗取 2 个汉字（宽 4 + … 1 = 5 ≤ 6），不取第 3 个
        let (pre, post, win) = field_window("一二三四五", "", 6, 0, false);
        assert_eq!(pre, "…");
        assert_eq!(post, "四五", "CJK 原子尾窗: {pre}{post}");
        assert_eq!((win.win_lo, win.win_hi), (3, 5));
        // 空值：占位提示在 post（光标 0 列在占位提示之前），窗口空 cols = [0]
        let (pre, post, win) = field_window("", "4", 6, 0, true);
        assert!(pre.is_empty() && post == "4");
        assert_eq!(win.cols, vec![0]);
        assert_eq!((win.win_lo, win.win_hi), (0, 0));
        // 光标越界钳制（陈旧偏移防御）：按字段长取尾
        let (pre, post, win) = field_window("abc", "", 10, 99, true);
        assert_eq!(pre, "abc");
        assert!(post.is_empty());
        assert_eq!(win.win_hi, 3);
    }

    #[test]
    fn field_window_unfocused_placeholder_and_normal() {
        // 未聚焦空值：占位提示整窗
        let (pre, post, _) = field_window("", "占位提示", 20, 0, false);
        assert!(pre.is_empty());
        assert_eq!(post, truncate("占位提示", 20));
        // 未聚焦正常宽度：原样
        let (pre, post, _) = field_window("abc", "", 6, 0, false);
        assert!(pre.is_empty() && post == "abc");
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
        assert_eq!(r.x, 23); // inner.x + 9（v1.34 S2：字段行左 2 列 padding 后指示符列）
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

#[cfg(test)]
mod tests {
    //! v133/D38 对话框拖拽渲染：首帧居中并记忆（dlg_pos/dlg_area 回填）、
    //! 拖拽位置优先渲染与终端钳制（位置记忆保留原始拖拽点，绘制矩形钳制）。
    //! v1.34 三批：原生光标渲染集成（字形退役 / 光标屏幕位 / TestBackend
    //! 收到 set_cursor_position / 窗口几何回填）。

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::super::testfx;

    /// v1.34 三批：原生光标渲染集成——①无内联「▏」字形（后续文本不因
    /// 光标存在平移）；②聚焦文本行发布光标屏幕位（值区起点 + pre 宽）；
    /// ③Terminal 收口把光标位下发后端（TestBackend 可读回）；④字段窗口
    /// 几何与热区平行回填（点击映射单源载荷在位）
    #[tokio::test]
    async fn dialog_native_cursor_published_and_positioned() {
        let mut app = testfx::make_app_in("ezr-dlgcur", "cur");
        app.open_add_dialog();
        {
            let d = app.dialog.as_mut().unwrap();
            d.url = "https://ezr.dev/".to_string(); // 16 字符
            d.focus_field(0);
            d.cursor = 8; // "https://|ezr.dev/"
        }
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        // 120×40：dlg=(23,14,74,12)、inner=(24,15,72,10)；URL 行 y=16、
        // rect.x=26、值区起点 x=35；pre="https://" 宽 8 → 光标列 43
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
        // 字形退役：整屏无「▏」；文本原样显示（光标不占列）
        let s = term.backend().to_string();
        assert!(!s.contains('▏'), "内联字形光标退役: {s}");
        assert!(s.contains("https://ezr.dev/"), "URL 原样显示: {s}");
        // 窗口几何回填：URL 行 win 在位且 value_x = 35（点击映射单源载荷）
        let win = app.dlg_field_wins[0].as_ref().expect("文本行 win 回填");
        assert_eq!(win.value_x, 35);
        assert_eq!((win.win_lo, win.win_hi), (0, 16));
        assert_eq!(win.cols.last().copied(), Some(16), "cols 末位 = 窗口总宽");
        // 非文本行 win = None（校验 = 行 3、代理 = 行 5）
        assert!(app.dlg_field_wins[3].is_none(), "下拉行无窗口几何");
        // 光标移动到中段：屏幕位随动（零位移——显示内容不变，仅光标位变）
        app.dialog.as_mut().unwrap().cursor = 2;
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
        // 74×12 居中于 100×30：x=(100−74)/2=13、y=(30−12)/2=9
        assert_eq!(app.dlg_pos, Some((13, 9)), "首帧居中并记忆");
        let a = app.dlg_area.expect("dlg_area 回填");
        assert_eq!((a.x, a.y, a.width, a.height), (13, 9, 74, 12));
        app.shutdown().await;
    }

    #[tokio::test]
    async fn dialog_dragged_position_wins_and_clamps() {
        let mut app = testfx::make_app_in("ezr-dlgp", "drag");
        app.open_add_dialog();
        // 越界拖拽点（90, 26）：绘制矩形钳制在终端内（74×12 → x≤26、y≤18），
        // 位置记忆保留原始拖拽点（与 pieces 渲染同口径）
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
        assert!(s.contains("添加下载任务"), "拖拽位置渲染: {s}");
        app.shutdown().await;
    }
}
