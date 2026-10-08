//! btn — 对话框公共构件（居中矩形/宽字符边界截断/按钮行渲染）

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::text::w;
use super::{ACCENT, FG};
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

/// 居中绘制对话框按钮行并回填命中区域（添加/删除对话框共用）
pub(super) fn draw_button_row(
    f: &mut Frame,
    app: &mut App,
    inner: Rect,
    btn_row: u16,
    labels: &[(&str, usize)],
    focus: usize,
) {
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
