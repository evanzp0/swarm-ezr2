//! val_popup — 任务详情字段值弹窗（v1.34/FR-01-113，对齐 ezr-demo 重构版
//! draw_val_popup）：详情 校验/URL/失败/路径 四字段值点击打开的 48×12 模态
//! 小窗（z-order 最上层 L6），完整值 wrap 展示、仅垂直滚动条、Y 复制全部
//! 内容 + toast、拖拽移动位置关闭即清、Esc/q/Q/点 [✗] 关闭、点击外部无操作。
//!
//! - 固定 48×12（内框 46×10；内容区 = 内框再收阅读内边距 VAL_PAD_X=2/Y=1
//!   → 42×8，S2 基准先例 FR-01-115）；绿色加粗边框；标题 ` <字段名> `
//!   （`DetailField::title` 单源）；打开居中、位置关闭即清（同 pieces 口径）；
//! - 内容 = 打开时快照的完整字段值（wrap_text CJK 感知按内容区宽折行，
//!   整段完整展示无截断）；每行经 break_links_deep 逐字符注入零宽 VS15
//!   深度断链 + link_break_spans 影子色分段渲染（URL 等值不再被终端叠加
//!   默认链接下划线——悬停/常驻同断，比 scheme 锚点断链更强的 v1.34
//!   bug 修订；复制语义走原值）；
//! - 内容为**纯静态文本**：悬停不加下划线、无任何悬停反馈（与详情面板字段值
//!   热区「悬停 = 值下划线」区分）；弹窗内鼠标仅用于拖拽移动/滚动条交互，
//!   点击内容不触发复制；
//! - 右缘**仅垂直滚动条**（通用组件 FR-01-109 ScrollablePanel Vertical 实例，
//!   内容超窗才显示、滑块恒绿）；无水平滚动条（wrap 保证任意宽度完整可见）。

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use super::btn::{clip_wide_at_edges, dialog_rect, draw_close_btn};
use super::scrollable_panel::{ScrollablePanel, ScrollbarMode};
use super::text::{break_links_deep, link_break_spans, w, wrap_text};
use super::{panel_border_style, panel_title_style, FG, FOCUS_BORDER};
use crate::app::{App, SbKey};

/// 字段值弹窗固定尺寸（宽×高，含边框；v1.34：demo 首版 60×12 → 48×12 收窄）
const VAL_W: u16 = 48;
/// 字段值弹窗固定高度（含边框）
const VAL_H: u16 = 12;

/// 弹窗内容区阅读内边距（S2 基准先例，FR-01-115）：左右各 2 列、上下各 1 行
/// ——文本不贴边框，长文案阅读更舒适（极小窗口按内框一半钳制，不 panic）
const VAL_PAD_X: u16 = 2;
const VAL_PAD_Y: u16 = 1;

/// 字段值弹窗渲染（模态浮层；z-order 最上层 L6，draw() 末位绘制）。
pub(super) fn draw_val_popup(f: &mut Frame, app: &mut App, area: Rect) {
    // 可拖拽定位：记忆位置优先（钳制在终端内），首帧居中并回填
    let rect = match app.val_pos {
        Some((px, py)) => {
            let dw = VAL_W.min(area.width);
            let dh = VAL_H.min(area.height);
            Rect {
                x: px.min(area.width.saturating_sub(dw)),
                y: py.min(area.height.saturating_sub(dh)),
                width: dw,
                height: dh,
            }
        }
        None => {
            let r = dialog_rect(area, VAL_W, VAL_H);
            app.val_pos = Some((r.x, r.y));
            r
        }
    };
    f.render_widget(Clear, rect);
    // 浮层边界 CJK 宽字符切割处理（残影根治点，与对话框同口径）
    clip_wide_at_edges(f, rect);
    let title = format!(" {} ", app.val_field.title());
    let title_w = w(&title);
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(ratatui::text::Span::styled(title, panel_title_style(true)));
    // 弹窗框架 = ScrollablePanel（Vertical：右边框列 ↑│║│↓；仅垂直滚动条）
    let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
    sb.thumb(FOCUS_BORDER)
        .pressed_v(app.sb_arrow_pressed(SbKey::Val));
    let inner = sb.render_block(f, rect, block);
    app.val_area = Some(rect);
    // 右上角 [✗]（FR-01-114：全部弹窗统一；原地松开 = 关闭字段值弹窗）
    app.popup_close_rect = draw_close_btn(f, app, rect, title_w);

    // 内容：完整字段值 wrap 成行（每行 ≤ 内容区宽），FG 常规色、纯静态样式
    //（无悬停反馈）；内容区 = 内框再收阅读内边距（VAL_PAD_X/Y）；内框过窄时
    // 按 1 列折行兜底（只留边框的极小窗口不 panic）
    let pad_x = VAL_PAD_X.min(inner.width / 2);
    let pad_y = VAL_PAD_Y.min(inner.height / 2);
    let content = Rect {
        x: inner.x + pad_x,
        y: inner.y + pad_y,
        width: inner.width.saturating_sub(pad_x * 2),
        height: inner.height.saturating_sub(pad_y * 2),
    };
    let view_h = content.height as usize;
    // 每行经 break_links_deep 深度断链（逐字符 VS15）——字段值弹窗内容不再
    // 被终端叠加 http(s) 默认链接下划线（v1.34 bug 修订：scheme 锚点断链
    // 不足以拦下归一化后匹配的终端识别，逐字符断链后行内无任何连续裸串）；
    // wrap 先按原值折行、断链注入零宽不改变行宽与折行结果；经 link_break_spans
    // 分段渲染兼容 pyte 类模拟器（文档见该函数）
    let lines: Vec<Line> = wrap_text(&app.val_text, content.width.max(1) as usize)
        .into_iter()
        .map(|s| {
            Line::from(link_break_spans(
                &break_links_deep(&s),
                Style::default().fg(FG),
            ))
        })
        .collect();
    let total = lines.len();
    let max_scroll = total.saturating_sub(view_h);
    app.val_scroll = app.val_scroll.min(max_scroll);
    let start = app.val_scroll;
    let shown: Vec<Line> = lines.into_iter().skip(start).take(view_h).collect();
    f.render_widget(Paragraph::new(shown), content);

    // 通用滚动条·垂直（内容超窗才绘制——显示条件在组件内收口）
    sb.vertical(total, view_h, start);
    app.val_scrollbar = sb.render_bars(f, rect).vertical;
    app.val_max_scroll = total.saturating_sub(view_h.max(1));
}

#[cfg(test)]
mod tests {
    //! v1.34 bug 修订：字段值弹窗内容深度断链 + 纯静态渲染（无悬停下划线）。
    //! 弹窗内容 = 终端默认链接下划线的唯一历史残留面（详情 URL 值为设计内
    //! 悬停下划线）；本组以缓冲区级断言锁定：①内容逐字符 VS15 断链；
    //! ②弹窗区域无任何 UNDERLINED 修饰（静态样式）；③悬停不变（指针落
    //! 内容区重绘后仍无下划线、断链不回退）。

    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::Modifier;
    use ratatui::Terminal;

    use super::super::btn::dialog_rect;
    use super::super::testfx;
    use super::*;
    use crate::app::DetailField;
    use crate::model::TaskState;

    fn app_with_url_popup(url: &str) -> App {
        let mut app = testfx::make_app_in("ezr-valpopup", "deep-break");
        let mut t = crate::model::sample_task();
        t.url = url.to_string();
        t.state = TaskState::Paused;
        app.tasks.push(t);
        app.selected = 0;
        app.open_val_popup(DetailField::Url);
        app
    }

    /// 弹窗内容逐字符 VS15 深度断链：URL 文本所在行每个可见字符格都携带
    /// VS15（行内不存在任何连续裸串——终端无论按裸串还是归一化匹配都无
    /// 「https://」可识）；点击复制的原始值不经注入（app.val_text 原值）。
    #[tokio::test]
    async fn popup_content_deep_broken_and_value_unaffected() {
        let url = "https://example.com/media-release.zip";
        let mut app = app_with_url_popup(url);
        assert_eq!(app.val_text, url, "复制语义走原值（快照不注入）");
        let term = testfx::render_full(&mut app, 100, 24);
        let buf = term.backend().buffer();
        let rect = dialog_rect(Rect::new(0, 0, 100, 24), VAL_W, VAL_H);
        // 内容区（内框收 S2 padding，与渲染同口径）
        let content = Rect {
            x: rect.x + 3,
            y: rect.y + 2,
            width: VAL_W - 6,
            height: VAL_H - 4,
        };
        let mut visible_chars = 0usize;
        let mut broken_chars = 0usize;
        for y in content.y..content.y + content.height {
            for x in content.x..content.x + content.width {
                let sym = buf.cell((x, y)).unwrap().symbol();
                if sym == " " {
                    continue;
                }
                visible_chars += 1;
                if sym.contains('\u{FE0E}') {
                    broken_chars += 1;
                }
            }
        }
        assert_eq!(
            broken_chars, visible_chars,
            "内容区每个可见字符格都携带 VS15（{visible_chars} 格）"
        );
        assert!(visible_chars >= url.chars().count(), "URL 完整可见");
    }

    /// 弹窗区域（含边框/标题/滚动条）无任何 UNDERLINED 修饰——纯静态文本；
    /// 指针悬停内容区重绘后依旧（悬停不加下划线，与详情面板值热区区分）
    #[tokio::test]
    async fn popup_has_no_underline_even_on_hover() {
        let mut app = app_with_url_popup("https://example.com/media-release.zip");
        let term = testfx::render_full(&mut app, 100, 24);
        let rect = dialog_rect(Rect::new(0, 0, 100, 24), VAL_W, VAL_H);
        let no_underline = |term: &Terminal<TestBackend>| {
            let buf = term.backend().buffer();
            for y in rect.y..rect.y + rect.height {
                for x in rect.x..rect.x + rect.width {
                    let m = buf.cell((x, y)).unwrap().modifier;
                    assert!(
                        !m.contains(Modifier::UNDERLINED),
                        "弹窗区域 ({x},{y}) 出现下划线修饰"
                    );
                }
            }
        };
        no_underline(&term);
        // 指针悬停内容区（URL 文本行中部）重绘：仍无下划线
        app.pointer = Some((rect.x + 20, rect.y + 3));
        let term = testfx::render_full(&mut app, 100, 24);
        no_underline(&term);
    }
}
