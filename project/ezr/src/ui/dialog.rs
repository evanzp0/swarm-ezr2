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
use crate::app::{App, DialogKind};
use crate::model::checksum::checksum_dropdown;

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

/// 聚焦文本字段的光标显示（v1.34 二批 bug 修订）：返回 (光标前串, 光标后串)，
/// 二者拼接 = 显示内容，「▏」光标字形渲染在两串之间（光标列随
/// [`crate::app::Dialog::cursor`]——原恒缀尾）。显示宽 ≤ budget：
/// - 空值：pre 空 + post = 占位提示截断（光标在 0 列，占位提示属 post）；
/// - 值宽不超预算：整值显示，pre = 前 cursor 个字符；
/// - 超宽：以光标为锚取窗（窗内恒含光标边界）——先向右取满预算、余量向左
///   回退；裁剪侧加 … 指示（指示占列，预算扣减重取至不动点；budget < 3 时
///   放弃指示符）。字符原子取窗（CJK 宽字符不切半列）。
fn cursor_field_display(val: &str, ph: &str, budget: usize, cursor: usize) -> (String, String) {
    let budget = budget.max(1);
    if val.is_empty() {
        return (String::new(), truncate(ph, budget));
    }
    let chars: Vec<char> = val.chars().collect();
    let widths: Vec<usize> = chars.iter().map(|c| w(&c.to_string())).collect();
    let total: usize = widths.iter().sum();
    let cursor = cursor.min(chars.len());
    if total <= budget {
        return (
            chars[..cursor].iter().collect(),
            chars[cursor..].iter().collect(),
        );
    }
    // 超宽取窗：两轮不动点（指示符占列 → 扣减预算重取，直至裁剪侧数稳定）
    let mut reserve = 0usize;
    loop {
        let b = budget.saturating_sub(reserve).max(1);
        // 向右取满，余量向左回退（字符原子：宽字符放不下整列则不取）
        let mut acc = 0usize;
        let mut hi = cursor;
        while hi < chars.len() && acc + widths[hi] <= b {
            acc += widths[hi];
            hi += 1;
        }
        let mut lo = cursor;
        while lo > 0 && acc + widths[lo - 1] <= b {
            lo -= 1;
            acc += widths[lo];
        }
        let left_cut = lo > 0;
        let right_cut = hi < chars.len();
        let need = usize::from(left_cut) + usize::from(right_cut);
        if need == reserve || budget < 3 {
            let pre = if left_cut {
                format!("…{}", chars[lo..cursor].iter().collect::<String>())
            } else {
                chars[lo..cursor].iter().collect()
            };
            let post = if right_cut {
                format!("{}…", chars[cursor..hi].iter().collect::<String>())
            } else {
                chars[cursor..hi].iter().collect()
            };
            return (pre, post);
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
                if focused {
                    // 光标编辑渲染（v1.34 二批 bug 修订）：「▏」落在字段内
                    // 光标列（原恒缀尾）；超宽值以光标为锚取窗 + … 指示；
                    // 预算 −1 列容纳光标字形（label 7 + "> " 2 + 值 + 字形
                    // = avail，不溢出字段行 rect 右缘）
                    let (pre, post) = cursor_field_display(
                        &row.value,
                        &row.placeholder,
                        avail.saturating_sub(1),
                        cursor,
                    );
                    let base = Style::default().fg(if row.value.is_empty() {
                        DIM2
                    } else {
                        Color::White
                    });
                    spans.push(Span::styled(pre, base));
                    spans.push(Span::styled(
                        "▏",
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::styled(post, base));
                } else {
                    spans.push(Span::styled(
                        field_display(&row.value, &row.placeholder, avail),
                        Style::default().fg(if row.value.is_empty() {
                            DIM2
                        } else {
                            Color::White
                        }),
                    ));
                }
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
        // 回填字段行命中区域（鼠标点击聚焦；下拉行点击展开）
        app.dlg_field_rects.push((rect, i));
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

    /// v1.34 二批：光标显示三态（不超宽/超宽取窗/空值占位）——拼接 = 显示
    /// 内容、光标恒在窗内、CJK 原子不切半
    #[test]
    fn cursor_field_display_windows_and_splits() {
        // 不超宽：整值显示，pre 宽 = 光标列
        let (pre, post) = cursor_field_display("abcdef", "", 10, 2);
        assert_eq!(pre, "ab");
        assert_eq!(post, "cdef");
        assert_eq!(w(&pre) + w(&post), 6);
        // 超宽（预算 6，值宽 16）：光标 8 为锚 → 窗内含光标、双缘 … 指示
        let val = "abcdefghijklmnop";
        let (pre, post) = cursor_field_display(val, "", 6, 8);
        assert!(
            pre.starts_with('…') && post.ends_with('…'),
            "双缘裁剪指示: {pre}|{post}"
        );
        assert_eq!(w(&pre) + w(&post), 6, "总宽 = 预算");
        // 光标可见性：pre+post 还原后含 val[8]（'i'），且非占位空窗
        let joined = format!("{pre}{post}");
        assert!(joined.contains('i'), "窗内含光标字符: {joined}");
        // 光标在尾：窗贴右（无尾部 …）
        let (pre, post) = cursor_field_display(val, "", 6, 16);
        assert!(!post.ends_with('…'), "光标在尾右缘不裁: {pre}|{post}");
        assert!(pre.starts_with('…'));
        // 光标在 0：窗贴左（无首部 …）
        let (pre, post) = cursor_field_display(val, "", 6, 0);
        assert!(!pre.starts_with('…'), "光标在首左缘不裁: {pre}|{post}");
        assert!(post.ends_with('…'));
        // CJK 原子：宽 4 预算 3 → 单汉字 2 列可取，不切半
        let (pre, post) = cursor_field_display("中文中文", "", 3, 4);
        assert_eq!(
            w(&pre) + w(&post),
            3,
            "CJK 原子取窗（汉字 2 列 + … 1 列 = 预算）: {pre}|{post}"
        );
        // 空值：占位提示在 post（光标 0 列在占位提示之前）
        let (pre, post) = cursor_field_display("", "4", 6, 0);
        assert!(pre.is_empty() && post == "4");
        // 光标越界钳制（陈旧偏移防御）：按字段长取尾
        let (pre, post) = cursor_field_display("abc", "", 10, 99);
        assert_eq!(pre, "abc");
        assert!(post.is_empty());
    }

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

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::super::testfx;

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
