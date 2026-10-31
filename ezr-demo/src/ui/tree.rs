use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::scrollable_panel::{ScrollablePanel, ScrollbarMode};
use crate::scrollbar;
use crate::ui::fmt_size;

use super::*;

/// 文件树行快照（渲染与树状态解耦：调用方从树模型产出一帧快照）
pub struct TreeRow {
    /// 目录层级（0 = 根；行首缩进 = depth × 2 列）
    pub depth: usize,

    /// 目录行（三态复选框 + ▾/▸ 展开·收拢箭头；文件行无箭头）
    pub is_dir: bool,

    pub name: String,

    /// 字节数（目录 = 子树聚合），右对齐显示
    pub size: u64,

    pub selected: bool,

    /// 目录部分勾选（[-] 黄色）
    pub partial: bool,

    /// 目录展开状态（▸ 收拢 / ▾ 展开）
    pub expanded: bool,
}

/// `draw_tree_panel` 的渲染产物：调用方据此回填命中映射与滚动条几何。
pub struct TreePanelRendered {
    /// 内容区（不含边框与滚动条列）
    pub inner: Rect,

    /// 垂直滚动条几何（拖拽/点击轨道/箭头；无溢出时 None）
    pub sb: Option<scrollbar::Bar>,

    /// 可点击行热区（行号 = 可见行绝对下标 scroll + vi；复选框/目录展开
    /// 判定由调用方按 depth 换算）
    pub row_rects: Vec<(Rect, usize)>,
}

/// 可复用文件树 panel（TreePanel）。
///
/// 「添加/修改 BT 任务」下载文件选择列表使用：
/// - 圆角边框（焦点绿 / 非焦点暗）+ 左上标题（`title`）；
/// - 底边框右侧动态统计（`caption`，右下角；不覆盖右下角框角字符）；
/// - 目录 ▾/▸ 展开·收拢，三态勾选 `[x]`/`[-]`/`[ ]`，文件 `[x]`/`[ ]`；
///   行首缩进体现层级，右对齐大小（目录 = 子树聚合）；
/// - 内容区执行 §4.3 S2 padding：行首 2 列 + 行尾 2 列（与任务列表
///   「左 2 光标槽 / 右 2 收窄」同构），光标行 SEL_BG 满行背景（含 padding 列）；
/// - 内容超出可视行时，垂直滚动条由 ScrollablePanel 绘制在右边框上
///   （与任务列表 ScrollPanel 同构：同箭头/轨道/内边距几何，panel 层级，
///   不占用内容列）。
pub fn draw_tree_panel(
    f: &mut Frame,
    rect: Rect,
    title: &str,
    caption: &str,
    focused: bool,
    pressed_v: Option<scrollbar::Arrow>,
    rows: &[TreeRow],
    cursor: usize,
    scroll: usize,
    visible: usize,
) -> TreePanelRendered {
    // panel = ScrollablePanel（与任务列表 ScrollPanel 同构）：垂直滚动条
    // 绘制在列表框右边框上，箭头/轨道/内边距几何与主列表完全一致
    let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
    sb.thumb(if focused { FOCUS_BORDER } else { BORDER })
        .pressed_v(pressed_v);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(panel_border_style(focused))
        .title(Span::styled(title.to_string(), panel_title_style(focused)));
    let inner = sb.render_block(f, rect, block);

    // 右下角动态统计（caption）：底边框行右对齐，左侧与右下角框角各留 1 列
    let cap_text = format!(" {caption} ");
    let cap_w = crate::ui::w(&cap_text) as u16;
    if rect.width > cap_w + 2 {
        let cap_rect = Rect {
            x: rect.x + rect.width - 1 - cap_w,
            y: rect.y + rect.height - 1,
            width: cap_w,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(cap_text, Style::default().fg(DIM)))),
            cap_rect,
        );
    }

    // 文件树行（占满内容区全宽——滚动条在列表框右边框上；光标行整行 SEL_BG）
    let mut row_rects = Vec::new();
    for vi in 0..visible.min(inner.height as usize) {
        let i = scroll + vi;
        let Some(snap) = rows.get(i) else { break };
        let y = inner.y + vi as u16;
        let is_cursor = i == cursor;
        let row_rect = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };
        // §4.3 S2 内容区 padding：行首 2 列 + 行尾 2 列（与任务列表「左 2 光标槽 /
        // 右 2 收窄」同构；SEL_BG 满行覆盖含 padding 列）；head_w 计入 mark(4)/
        // 空格(1)/目录箭头(2)，保证行宽恰好 = 内容区宽（无溢出裁剪）
        let content_w = (row_rect.width as usize).saturating_sub(4);
        let indent = "  ".repeat(snap.depth.min(8));
        let (mark, mark_color) = if snap.is_dir {
            if snap.selected {
                ("[x]", GREEN)
            } else if snap.partial {
                ("[-]", YELLOW)
            } else {
                ("[ ]", DIM2)
            }
        } else if snap.selected {
            ("[x]", GREEN)
        } else {
            ("[ ]", DIM2)
        };
        let head_w = crate::ui::w(&indent) + 4 + 1 + usize::from(snap.is_dir) * 2;
        let size_str = fmt_size(snap.size);
        let name_disp = if snap.is_dir {
            format!("{}/", snap.name)
        } else {
            snap.name.clone()
        };
        let name_budget = content_w
            .saturating_sub(head_w + crate::ui::w(&size_str))
            .max(4);
        let name = crate::ui::truncate(&name_disp, name_budget);
        let pad = content_w.saturating_sub(head_w + crate::ui::w(&name) + crate::ui::w(&size_str));
        let mut spans = Vec::new();
        spans.push(Span::raw("  "));
        spans.push(Span::styled(indent.clone(), Style::default().fg(DIM2)));
        spans.push(Span::styled(
            mark.to_string(),
            Style::default().fg(mark_color).add_modifier(if is_cursor {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
        ));
        spans.push(Span::styled(" ", Style::default()));
        if snap.is_dir {
            spans.push(Span::styled(
                if snap.expanded { "▾ " } else { "▸ " },
                Style::default().fg(if is_cursor { Color::White } else { ACCENT }),
            ));
        }
        spans.push(Span::styled(
            name.clone(),
            Style::default()
                .fg(if is_cursor { Color::White } else { FG })
                .add_modifier(if snap.is_dir || is_cursor {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ));
        spans.push(Span::styled(
            format!("{}{}", " ".repeat(pad), size_str),
            Style::default().fg(if is_cursor { Color::White } else { DIM }),
        ));
        let mut para = Paragraph::new(Line::from(spans));
        if is_cursor {
            // 整行背景（含行尾空区），不再前置空格位移内容
            para = para.style(Style::default().bg(SEL_BG));
        }
        f.render_widget(para, row_rect);
        row_rects.push((row_rect, i));
    }

    // 垂直滚动条：ScrollablePanel 绘制在列表框右边框上（与任务列表 ScrollPanel
    // 同构——箭头在首/末内容行，轨道居中；panel 层级，不占内容列）
    sb.vertical(rows.len(), visible.max(1), scroll);
    let bar = sb.render_bars(f, rect).vertical;

    TreePanelRendered {
        inner,
        sb: bar,
        row_rects,
    }
}
