use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::multiline::MultiLineBuf;
use crate::scrollable_panel::{ScrollablePanel, ScrollbarMode};
use crate::scrollbar;
use crate::textfield::disp_window;

use super::*;

/// `draw_multi_line_input` 的渲染产物：调用方据此回填命中映射与滚动条几何。
pub struct MultiLineRendered {
    /// 内容区（不含边框与滚动条列；点击定位/滚轮判定用）
    pub inner: Rect,

    /// 垂直滚动条几何（拖拽/点击轨道/箭头；无溢出时 None）
    pub sb: Option<scrollbar::Bar>,

    /// 水平滚动条几何（仅无 wrap 模式；内容未超宽时 None）
    pub h_sb: Option<scrollbar::Bar>,

    /// 原生光标屏幕位（仅聚焦时发布；可视行/列在视口内才显示）
    pub cursor: Option<(u16, u16)>,
}

/// 多行文本输入 panel（可复用组件）。
///
/// 「命令窗口」文本框（`wrap = true`）与「添加/修改 BT 任务」trackers 输入
/// （`wrap = false`）共用：
/// - 圆角子边框（焦点绿 / 非焦点暗）+ 可选左上标题（`title`，None = 无标题）；
/// - 空缓冲占位提示（`placeholder`，空串 = 不显示任何提示）；
/// - `wrap = true`：内容 word wrap 自动换行（按显示宽，空格优先断行），
///   超出可视行时垂直滚动；
/// - `wrap = false`：逻辑行 1:1 渲染（不做 word wrap），行内超宽内容经水平
///   视口（`view_x` 显示列）滚动，ScrollablePanel 同时绘制垂直（右边框）+
///   水平（下边框）滚动条（`ScrollbarMode::Both`，箭头/轨道/滑块拖拽）；
/// - 视口跟随为按需制（编辑拉回 / 滚动停留，语义见 `MultiLineBuf`；
///   无 wrap 模式水平方向同契约：光标移动回贴列，滚动交互后停留）；
/// - 滚动条绘制在 panel 边框上（任务列表 ScrollPanel 同构，内容区不预留
///   滚动条列）；
/// - 原生光标位通过返回值发布，由调用方收口（cmd_cursor_screen /
///   dlg_cursor_screen）。
#[allow(clippy::too_many_arguments)]
pub fn draw_multi_line_input(
    f: &mut Frame,
    rect: Rect,
    title: Option<&str>,
    placeholder: &str,
    focused: bool,
    pressed_v: Option<scrollbar::Arrow>,
    pressed_h: Option<scrollbar::Arrow>,
    wrap: bool,
    buf: &mut MultiLineBuf,
) -> MultiLineRendered {
    // panel = ScrollablePanel（与任务列表同构）：wrap = 垂直条；无 wrap = 垂直+水平
    let mode = if wrap {
        ScrollbarMode::Vertical
    } else {
        ScrollbarMode::Both
    };
    let mut sb = ScrollablePanel::new(mode);
    sb.thumb(if focused { FOCUS_BORDER } else { BORDER })
        .pressed_v(pressed_v)
        .pressed_h(pressed_h);
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(focused));
    if let Some(t) = title {
        block = block.title(Span::styled(t.to_string(), panel_title_style(focused)));
    }
    let inner = sb.render_block(f, rect, block);

    let text_w = inner.width as usize;
    let view_h = inner.height as usize;

    // 发布视口几何（↑↓ 可视行导航的 wrap 分段/列保持依赖；键处理前必有一帧渲染）
    buf.text_w = text_w;
    buf.wrap = wrap;

    // 可视行布局：wrap = 显示宽分段；无 wrap = 逻辑行 1:1
    let (rows, total_rows, cursor_vrow, cursor_vcol) = if wrap {
        let rows = buf.wrap_rows(text_w);
        let total = rows.len();
        let (r, c) = buf.cursor_visual(text_w);
        (rows, total, r, c as usize)
    } else {
        let rows = buf.nowrap_rows();
        let total = rows.len();
        let (r, c) = buf.cursor_visual_nowrap();
        (rows, total, r, c)
    };

    // 水平视口（仅无 wrap 模式）：内容宽 = 最宽逻辑行；发布 max_scroll_x 并钳制
    let content_w = if wrap { 0 } else { buf.nowrap_content_w() };
    if !wrap {
        buf.max_scroll_x = content_w.saturating_sub(text_w);
        buf.view_x = buf.view_x.min(buf.max_scroll_x);
    }

    // 视口跟随（按需）：光标移动/编辑时拉回一次；滚动条/滚轮滚动后不强制回贴
    if buf.follow {
        if wrap {
            buf.scroll_follow(total_rows, view_h, cursor_vrow);
        } else {
            buf.scroll_follow_nowrap(total_rows, view_h, cursor_vrow, text_w, cursor_vcol);
        }
        buf.follow = false;
    } else {
        buf.clamp_view(total_rows, view_h);
    }
    buf.max_scroll = total_rows.saturating_sub(view_h);
    let start = buf.view;

    // 渲染可视行（空缓冲 = 占位提示或空白）
    let empty = buf.is_buffer_empty();
    let mut lines: Vec<Line> = Vec::with_capacity(view_h);
    for vi in 0..view_h {
        let ri = start + vi;
        let span = if empty && vi == 0 && !placeholder.is_empty() {
            Span::styled(placeholder.to_string(), Style::default().fg(DIM2))
        } else {
            match rows.get(ri) {
                Some((li, s, n)) => {
                    let text: String = if wrap {
                        buf.lines[*li].value.chars().skip(*s).take(*n).collect()
                    } else {
                        disp_window(&buf.lines[*li].value, buf.view_x, text_w)
                    };
                    Span::styled(text, Style::default().fg(Color::White))
                }
                None => Span::styled(String::new(), Style::default()),
            }
        };
        lines.push(Line::from(span));
    }
    f.render_widget(Paragraph::new(lines), inner);

    // 原生光标位（聚焦文本框时发布；可视行/列在视口内才显示；
    // wrap 模式 view_x 恒 0，条件退化为与旧版一致的列界检查）。
    // 光标在内容末列之后一格（行尾插入点）时钳到最后可视列上显示——
    // 与 wrap 模式 cursor_visual 的 vcol 钳制同约定
    let cursor = if focused
        && cursor_vrow >= start
        && cursor_vrow < start + view_h
        && cursor_vcol >= buf.view_x
        && cursor_vcol <= buf.view_x + text_w.max(1)
    {
        let disp_col = (cursor_vcol - buf.view_x).min(text_w.saturating_sub(1)) as u16;
        Some((
            inner.x + disp_col,
            inner.y + (cursor_vrow - start) as u16,
        ))
    } else {
        None
    };

    // 滚动条（ScrollablePanel：垂直 = 右边框，水平 = 下边框；↑↓/←→ 箭头 +
    // 轨道 + 滑块；无溢出方向由 draw_axis 自动隐藏）
    sb.vertical(total_rows, view_h, start);
    if !wrap {
        sb.horizontal(content_w, text_w, buf.view_x);
    }
    let bars = sb.render_bars(f, rect);

    MultiLineRendered {
        inner,
        sb: bars.vertical,
        h_sb: bars.horizontal,
        cursor,
    }
}
