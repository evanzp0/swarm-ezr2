//! btn — 对话框公共构件（居中矩形/宽字符边界截断/按钮行渲染）

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::text::w;
use super::{panel_border_style, ACCENT, DIM2, FG, RED};
use crate::app::App;

pub(super) fn dialog_rect(area: Rect, dw: u16, dh: u16) -> Rect {
    let dw = dw.min(area.width);
    let dh = dh.min(area.height);
    Rect {
        x: area.x + (area.width.saturating_sub(dw)) / 2,
        y: area.y + (area.height.saturating_sub(dh)) / 2,
        width: dw,
        height: dh,
    }
}

/// 浮层矩形（v133/D38 对话框拖拽）：拖拽位置优先（钳制终端内），None =
/// 居中。调用方负责首帧位置记忆回填（None → 居中后写回）与区域热区回填
///（与 pieces/help 渲染同口径）。
pub(super) fn overlay_rect(area: Rect, dw: u16, dh: u16, pos: Option<(u16, u16)>) -> Rect {
    let w = dw.min(area.width);
    let h = dh.min(area.height);
    match pos {
        Some((px, py)) => Rect {
            x: px.min(area.width.saturating_sub(w)),
            y: py.min(area.height.saturating_sub(h)),
            width: w,
            height: h,
        },
        None => dialog_rect(area, dw, dh),
    }
}

/// 浮层边界与 CJK 宽字符的切割处理（残影 bug 的根治点）。
///
/// 浮层（对话框/下拉）居中后，左/右边界列可能恰好切在底层 CJK 宽字符的
/// 半格上，使缓冲区进入自相矛盾的状态：
/// - 左边界外一格是宽字符主格（如「待」），其显示会延伸进浮层区域的右半格，
///   而浮层又在该半格上写了边框字符——同一显示行上「宽字 + 窄字」重叠；
/// - 右边界外一格残留孤立的宽字符半格（主格已被浮层改写为边框/空格）。
///
/// 两种矛盾下，无论增量 diff 还是清屏全量重绘，终端都可能把宽字渲染在
/// 浮层边框之上（残影），或使后续字符整体错位一格（宽字打印后游标推进
/// 两格，与缓冲区期望的格位不再对齐）。
/// 因此浮层清屏后立刻：把左边界外一格的宽字符主格截断为空格（该字本就
/// 有一半被浮层盖住，整字不显示是正确视觉），把右边界外一格的孤立半格
/// 补成空格——保证缓冲区内字符的显示占用永不与浮层内容跨界重叠。
pub(super) fn button_spans(txt: &str, focused: bool) -> Vec<Span<'static>> {
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

/// 居中绘制对话框按钮行并回填命中区域（添加/删除对话框共用）。
/// 窄终端钳制（v1.15/FR-01-103 三钮后按钮组最宽 40 列，<46 列对话框内框放不下；
/// coder v116 下拉浮层同款纪律：钳制只截去越界部分、常态几何不变）——按钮矩形
/// 右缘截到内框/缓冲区内，越界按钮整只跳过（无命中区域、点击无动作）。
pub(super) fn draw_button_row(
    f: &mut Frame,
    app: &mut App,
    inner: Rect,
    btn_row: u16,
    labels: &[(&str, usize)],
    focus: usize,
) {
    let area = f.area();
    let btn_ws: Vec<usize> = labels
        .iter()
        .map(|(txt, _)| w(&format!("[ {} ]", txt)) + 2)
        .collect();
    let total_w: usize = btn_ws.iter().sum::<usize>() + 2 * (labels.len() - 1);
    let mut bx = inner.x as usize + ((inner.width as usize).saturating_sub(total_w)) / 2;
    let inner_right = inner.x as usize + inner.width as usize;
    for ((txt, idx), bw) in labels.iter().zip(btn_ws.iter()) {
        let focused = focus == *idx;
        let line = Line::from(button_spans(txt, focused));
        // 钳制：矩形右缘不超过内框右缘与缓冲区右缘（越界截断 / 整只跳过）
        let width = (*bw)
            .min(inner_right.saturating_sub(bx))
            .min(area.width as usize);
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
}

/// 左缘相邻列：宽字符主格（显示占两列、右半侵入浮层）→ 整格截断为空格
fn clip_left_edge(buf: &mut ratatui::buffer::Buffer, x: u16, y: u16) {
    if x == 0 {
        return;
    }
    let i = buf.index_of(x - 1, y);
    if w(buf.content[i].symbol()) > 1 {
        buf.content[i].set_char(' ');
    }
}

/// 右缘外一列：孤立半格（主格在浮层内、已被浮层改写）→ 补成空格
fn clip_right_edge(buf: &mut ratatui::buffer::Buffer, xr: u16, y: u16) {
    if xr + 1 >= buf.area.width {
        return;
    }
    let i = buf.index_of(xr + 1, y);
    if buf.content[i].symbol().is_empty() {
        buf.content[i].set_char(' ');
    }
}

pub(super) fn clip_wide_at_edges(f: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let buf = f.buffer_mut();
    let x0 = area.x;
    let xr = area.right() - 1; // 浮层最右列
    for y in area.y..area.bottom() {
        clip_left_edge(buf, x0, y);
        clip_right_edge(buf, xr, y);
    }
}

// ---------------------------------------------------------------------------
// 弹窗右上角 [✗] 关闭按钮（v1.34/FR-01-114，对齐 ezr-demo 重构版
// draw_close_btn：全部弹窗统一；[✗] 中括号内紧凑无 padding、中括号与
// 边框线之间左右各留 1 列 padding；中括号与弹窗边框线同色、不随悬停变化，
// 仅 ✗ 前景色高亮——D41 配色拆分）
// ---------------------------------------------------------------------------

/// 弹窗右上角 [✗] 关闭按钮：画在弹窗顶边框行右端——右角符内侧先留 1 列
/// 边框 padding，再画 `[✗]`（3 列宽，**中括号内紧凑、不留空格**），`[✗]`
/// 左侧再留 1 列边框 padding（不覆盖边框线；padding 取 1 列 = 与面板标题
/// 「 任务详情 」自身首尾 1 空格 padding 同构，整体占用 5 列）；配色 =
/// **中括号 `[`/`]` 与弹窗边框线同色**（panel_border_style(true) 绿+粗，
/// 融入边框、不随悬停/按压变化），**高亮的只有 ✗**——常态 DIM2，指针
/// 悬停或按压 = 仅前景色红+粗（无背景色/无反色，D41）；按下/松开判定与
/// 关闭行为在 App 层（popup_close_rect / x_press，各弹窗鼠标分支各自关闭
/// 自身）。返回命中热区（None = 弹窗过窄或与标题重叠，放弃绘制不设热区；
/// 热区 = `[✗]` 自身 3 列，前后 padding 列保持边框线原样、不参与点击）。
/// 按钮符号 = "[✗]"（✗ = U+2717 BALLOT X，单列宽）。
pub(super) fn draw_close_btn(
    f: &mut Frame,
    app: &App,
    popup: Rect,
    title_w: usize,
) -> Option<Rect> {
    // 极小弹窗不绘制，避免挤掉标题/内容
    if popup.width < 12 || popup.height < 3 {
        return None;
    }
    let bx = popup.x + popup.width - 5; // [✗] 3 列：右 padding 1 列 + 右角符
    if (bx as usize) < popup.x as usize + 1 + title_w + 2 {
        return None; // 左 padding 列压到标题：放弃绘制
    }
    let r = Rect {
        x: bx,
        y: popup.y,
        width: 3,
        height: 1,
    };
    let hovered = app.pointer.is_some_and(|(px, py)| {
        px >= r.x
            && px < r.x.saturating_add(r.width)
            && py >= r.y
            && py < r.y.saturating_add(r.height)
    });
    let hot = app.x_press || hovered;
    // 中括号 = 弹窗边框线同色（绿+粗），不随悬停/按压变化；
    // 仅 ✗ 高亮：常态纯 DIM2（显式清除按钮覆盖在边框单元格上继承的 BOLD，
    // 避免常态 ✗ 偏粗），悬停/按压 = 前景红+粗（无背景色/无反色）
    let bracket = panel_border_style(true);
    let x_st = if hot {
        Style::default().fg(RED).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DIM2).remove_modifier(Modifier::BOLD)
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("[", bracket),
            Span::styled("✗", x_st),
            Span::styled("]", bracket),
        ])),
        r,
    );
    Some(r)
}
