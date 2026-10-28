use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{App, SbKey};

use super::*;

pub(crate) const CMD_W: u16 = 78;
pub(crate) const CMD_H: u16 = 16;

/// 命令窗口（A 唤起）：标题「请输入命令或链接」+ 大多行文本框 + 确定按钮。
///
/// - 文本框 = 可复用多行文本输入 panel 组件（`ui::multiline::draw_multi_line_input`，
///   title = None 无标题，placeholder = 空 即不显示任何输入提示）：基于
///   ScrollablePanel（与任务列表同构：垂直滚动条绘制在文本框右边框上——
///   panel 层级，内容区不预留滚动条列，含 ↑↓ 箭头 / 轨道点击 / 滑块拖拽）；
/// - 内容 word wrap 自动换行（按显示宽，空格优先断行），超出可视行时滚动；
///   视口跟随为按需制：光标移动/编辑时拉回光标，滚动条/滚轮滚动后停留原处
///   （每帧强制回贴会让滚动条失效）；空缓冲不显示任何输入提示；
/// - Shift+Enter / Alt+Enter 换行，Enter 确认（= 触发 [确定] 按钮 click）；
///   确定按钮居中，Tab 在文本框/按钮间切换焦点，按钮焦点下 Enter 确认；
/// - 光标 = 终端原生光标（渲染层发布 `cmd_cursor_screen`，draw 收口）；
/// - 顶边框行右端 [✗] 关闭按钮（全部弹窗同契约）。
pub(crate) fn draw_cmd(f: &mut Frame, app: &mut App, area: Rect) {
    let dw = CMD_W.min(area.width);
    let dh = CMD_H.min(area.height);
    let rect = match app.cmd_pos {
        Some((px, py)) => Rect {
            x: px.min(area.width.saturating_sub(dw)),
            y: py.min(area.height.saturating_sub(dh)),
            width: dw,
            height: dh,
        },
        None => {
            let r = dialog_rect(area, CMD_W, CMD_H);
            app.cmd_pos = Some((r.x, r.y));
            r
        }
    };
    f.render_widget(Clear, rect);
    clip_wide_at_edges(f, rect);

    let title = " 请输入命令或链接 ";
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(title, panel_title_style(true)));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    app.popup_close_rect = draw_close_btn(f, app, rect, w(title));
    if inner.width < 24 || inner.height < 6 {
        app.cmd_area = Some(rect);
        return;
    }

    // 文本框 = 可复用多行文本输入 panel 组件（无标题；空缓冲无占位提示）；
    // 高度 = 内框 - 按钮行 1 - 按钮上下空行 2
    let box_h = inner.height.saturating_sub(3);
    let box_rect = Rect {
        x: inner.x + 2,
        y: inner.y + 1,
        width: inner.width.saturating_sub(4),
        height: box_h,
    };
    let focused_box = app.cmd_focus == 0;
    let rendered = draw_multi_line_input(
        f,
        box_rect,
        None,
        "",
        focused_box,
        app.sb_arrow_pressed(SbKey::CmdText),
        None,
        true,
        &mut app.cmd,
    );
    app.cmd_area = Some(rect);
    app.cmd_text_area = Some(rendered.inner);
    app.cmd_sb = rendered.sb;
    app.cmd_cursor_screen = rendered.cursor;

    // 确定按钮（居中，按钮行下空 1 行）
    let btn_row = inner.y + inner.height - 2;
    let label = "[ 确定 ]";
    let bw = w(label) + 2;
    let bx = inner.x as usize + ((inner.width as usize).saturating_sub(bw)) / 2;
    let btn_rect = Rect {
        x: bx as u16,
        y: btn_row,
        width: bw as u16,
        height: 1,
    };
    let focused_btn = app.cmd_focus == 1;
    f.render_widget(
        Paragraph::new(Line::from(button_spans("确定", focused_btn))),
        btn_rect,
    );
    app.cmd_btn_rect = Some(btn_rect);
}
