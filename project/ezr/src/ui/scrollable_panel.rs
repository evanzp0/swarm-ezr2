//! scrollable_panel — [`ScrollablePanel`]：面板边框 + 通用滚动条一体化组件。
//!
//! v1.32/FR-01-109 ①（对齐 ezr-demo m3 scrollable_panel.rs，逐字移植）：
//! 把「画面板边框 → 画内容 → 在边框上叠画滚动条 → 回填命中几何」四步收敛
//! 为一个组件，全部带滚动条的 panel/弹窗共用（滚动条引擎仍是
//! [`super::scrollbar`]，本组件只负责组合与 z 序）：
//!
//! * [`ScrollbarMode`]：本面板的滚动条配置——[`ScrollbarMode::None`]（无）/
//!   [`ScrollbarMode::Vertical`]（仅垂直）/ [`ScrollbarMode::Horizontal`]
//!   （仅水平）/ [`ScrollbarMode::Both`]（垂直 + 水平；并发连接面板是唯一
//!   Both 实例）。
//! * [`ScrollablePanel::render_block`]：画边框（含标题/聚焦样式，`Block`
//!   由调用方组装），返回内容区内框——内容形态各异，由调用方绘制。
//! * [`ScrollablePanel::render_bars`]：内容绘制完成后调用，按 mode 把滚动
//!   条叠画在面板边框列/行上（z 序：边框 → 内容 → 滚动条，与组件化前的
//!   手工顺序一致），返回 [`PanelBars`] 命中几何供 App 层回填。
//! * **显示条件集中收口**：`page > 0 && total > page` 才绘制并返回
//!   `Some`——调用方不再各自写 if/else，App 字段恒与画面一致（杜绝
//!   陈旧几何）；轨道 ≥ 2 格等极小面板退化判定仍在 [`super::scrollbar`]
//!   的渲染/命中内部（两侧同一口径）。
//!
//! # 用法（三步，z 序由调用顺序保证）
//!
//! ```ignore
//! let mut sb = ScrollablePanel::new(ScrollbarMode::Both);
//! sb.thumb(if focused { FOCUS_BORDER } else { BORDER })
//!     .pressed_v(app.sb_arrow_pressed(SbKey::ConnsV))
//!     .pressed_h(app.sb_arrow_pressed(SbKey::ConnsH));
//! let inner = sb.render_block(f, area, block);   // 1. 边框 → 内框
//! /* ……用 inner 画内容（表格/网格/文本），可回填 app 字段…… */
//! sb.vertical(conn_total, rows, app.conns_scroll) // 2. 内容后就绪的滚动状态
//!     .horizontal(content_w, inner.width as usize, app.conns_hscroll);
//! let bars = sb.render_bars(f, area);             // 3. 叠画滚动条 + 回填几何
//! app.conns_v_scrollbar = bars.vertical;
//! app.conns_h_scrollbar = bars.horizontal;
//! ```
//!
//! 滚动状态 setter 是 `&mut self -> &mut Self`（可变 builder）：total/page
//! 往往依赖内容区内框尺寸（内框要等边框画完才有），允许在内容绘制后补设。

use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Block;
use ratatui::Frame;

use super::BORDER;
use crate::scrollbar::{self, Arrow, Bar};

/// 滚动条配置：无 / 仅垂直 / 仅水平 / 双向
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
// Horizontal 暂无实例（并发连接面板为 Both），按组件四态契约保留
#[allow(dead_code)]
pub enum ScrollbarMode {
    /// 无滚动条（纯边框面板；如任务详情）
    None,
    /// 仅垂直（右边框列 ↑│║│↓）
    Vertical,
    /// 仅水平（下边框行 ←─═══─→）
    Horizontal,
    /// 垂直 + 水平（并发连接面板）
    Both,
}

impl ScrollbarMode {
    /// 是否含垂直滚动条
    #[must_use]
    pub fn has_vertical(self) -> bool {
        matches!(self, ScrollbarMode::Vertical | ScrollbarMode::Both)
    }

    /// 是否含水平滚动条
    #[must_use]
    pub fn has_horizontal(self) -> bool {
        matches!(self, ScrollbarMode::Horizontal | ScrollbarMode::Both)
    }
}

/// 单轴滚动状态（单位 = 该面板的滚动单位：任务条目/连接行/内容列/网格行/文本行）
#[derive(Clone, Copy, Debug)]
pub struct ScrollState {
    /// 内容总单位数
    pub total: usize,
    /// 可视单位数（一页）
    pub page: usize,
    /// 滚动偏移（组件内部钳制到 0..=total-page，由 [`super::scrollbar::Bar`] 执行）
    pub position: usize,
}

/// 一次 [`ScrollablePanel::render_bars`] 的产物：各轴命中几何（None = 本帧
/// 未绘制该轴）。调用方原样回填 App 的对应字段（list_scrollbar /
/// conns_v_scrollbar / conns_h_scrollbar / pieces_scrollbar / help_scrollbar）。
#[derive(Clone, Copy, Debug, Default)]
pub struct PanelBars {
    /// 垂直滚动条命中几何
    pub vertical: Option<Bar>,
    /// 水平滚动条命中几何
    pub horizontal: Option<Bar>,
}

/// 面板 + 滚动条一体化组件（纯数据配置，`Copy` 语义）。
///
/// 实例映射（FR-01-109 ①）：任务列表 = Vertical、并发连接 = Both、
/// pieces 分块弹窗 = Vertical、帮助窗口 = Vertical、任务详情 = None
/// （仅边框，统一面板框架绘制路径）。
#[derive(Clone, Copy, Debug)]
pub struct ScrollablePanel {
    mode: ScrollbarMode,
    v: Option<ScrollState>,
    h: Option<ScrollState>,
    thumb: Color,
    pressed_v: Option<Arrow>,
    pressed_h: Option<Arrow>,
}

impl ScrollablePanel {
    /// 按滚动条配置创建组件（滚动状态由 `vertical`/`horizontal` 补设）
    #[must_use]
    pub fn new(mode: ScrollbarMode) -> Self {
        Self {
            mode,
            v: None,
            h: None,
            thumb: BORDER,
            pressed_v: None,
            pressed_h: None,
        }
    }

    /// 设置垂直滚动状态（total/page/position；单位 = 该面板的滚动单位）
    pub fn vertical(&mut self, total: usize, page: usize, position: usize) -> &mut Self {
        self.v = Some(ScrollState {
            total,
            page,
            position,
        });
        self
    }

    /// 设置水平滚动状态（total/page/position；单位 = 内容显示列）
    pub fn horizontal(&mut self, total: usize, page: usize, position: usize) -> &mut Self {
        self.h = Some(ScrollState {
            total,
            page,
            position,
        });
        self
    }

    /// 滑块颜色 = 所属面板边框色（聚焦绿加粗 / 未聚焦暗；FR-01-109 ⑤）
    pub fn thumb(&mut self, color: Color) -> &mut Self {
        self.thumb = color;
        self
    }

    /// 按压中的垂直箭头（App 层按压/余辉态；None = 常规）
    pub fn pressed_v(&mut self, pressed: Option<Arrow>) -> &mut Self {
        self.pressed_v = pressed;
        self
    }

    /// 按压中的水平箭头（App 层按压/余辉态；None = 常规）
    pub fn pressed_h(&mut self, pressed: Option<Arrow>) -> &mut Self {
        self.pressed_h = pressed;
        self
    }

    /// 画面板边框（含标题/样式，由调用方组装 [`Block`]），返回内容区内框。
    ///
    /// 滚动条稍后由 [`Self::render_bars`] 叠画在边框上——内容 z 序在边框
    /// 之上、滚动条之下，与组件化前的手工渲染顺序一致。
    pub fn render_block(&self, f: &mut Frame, area: Rect, block: Block<'static>) -> Rect {
        let inner = block.inner(area);
        f.render_widget(block, area);
        inner
    }

    /// 内容绘制完成后调用：按 [`ScrollbarMode`] 把滚动条叠画在面板边框
    /// 列/行上，返回命中几何（None = 该轴本帧不显示：page == 0 或
    /// total <= page，或 mode 未启用该轴）。
    pub fn render_bars(&self, f: &mut Frame, area: Rect) -> PanelBars {
        PanelBars {
            vertical: self.draw_axis(f, area, true),
            horizontal: self.draw_axis(f, area, false),
        }
    }

    /// 单轴绘制（含显示条件收口：mode 启用 + 状态已设 + page>0 + total>page）
    fn draw_axis(&self, f: &mut Frame, area: Rect, vertical: bool) -> Option<Bar> {
        let enabled = if vertical {
            self.mode.has_vertical()
        } else {
            self.mode.has_horizontal()
        };
        let state = if vertical { self.v } else { self.h };
        let s = if enabled { state? } else { return None };
        if s.page == 0 || s.total <= s.page {
            return None;
        }
        let bar = if vertical {
            Bar::vertical(area, s.total, s.page, s.position)
        } else {
            Bar::horizontal(area, s.total, s.page, s.position)
        };
        scrollbar::render(
            f,
            &bar,
            self.thumb,
            if vertical {
                self.pressed_v
            } else {
                self.pressed_h
            },
        );
        Some(bar)
    }
}

#[cfg(test)]
mod tests {
    //! 组件收口单测：显示条件集中收口（page>0 且 total>page 才返回几何）、
    //! mode 门控（未启用轴恒 None）。

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;

    #[test]
    fn render_bars_gates_by_total_page_and_mode() {
        let area = Rect::new(0, 0, 40, 20);
        let mut term = Terminal::new(TestBackend::new(40, 20)).unwrap();

        // Vertical 模式 + total > page → 垂直 Some、水平 None
        term.draw(|f| {
            let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
            sb.vertical(100, 10, 0);
            let bars = sb.render_bars(f, area);
            assert!(bars.vertical.is_some(), "超页内容返回垂直几何");
            assert!(bars.horizontal.is_none(), "Vertical 模式无水平轴");
        })
        .unwrap();

        // total <= page → 双轴 None（显示条件收口，调用方零 if/else）
        term.draw(|f| {
            let mut sb = ScrollablePanel::new(ScrollbarMode::Both);
            sb.vertical(5, 10, 0).horizontal(10, 40, 0);
            let bars = sb.render_bars(f, area);
            assert!(bars.vertical.is_none(), "未超页不绘制垂直");
            assert!(bars.horizontal.is_none(), "未超宽不绘制水平");
        })
        .unwrap();

        // page == 0（内框为零）→ None
        term.draw(|f| {
            let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
            sb.vertical(100, 0, 0);
            assert!(sb.render_bars(f, area).vertical.is_none(), "page 0 不绘制");
        })
        .unwrap();

        // 状态未设 → None（不 panic）
        term.draw(|f| {
            let sb = ScrollablePanel::new(ScrollbarMode::Both);
            let bars = sb.render_bars(f, area);
            assert!(bars.vertical.is_none() && bars.horizontal.is_none());
        })
        .unwrap();
    }
}
