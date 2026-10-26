use ratatui::{layout::Rect, style::Color, widgets::Block, Frame};

use crate::scrollbar::{self, Arrow, Bar};
use crate::ui::BORDER;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
pub enum ScrollbarMode {
    None,

    Vertical,

    Horizontal,

    Both,
}

impl ScrollbarMode {
    pub fn has_vertical(self) -> bool {
        matches!(self, ScrollbarMode::Vertical | ScrollbarMode::Both)
    }

    pub fn has_horizontal(self) -> bool {
        matches!(self, ScrollbarMode::Horizontal | ScrollbarMode::Both)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ScrollState {
    pub total: usize,

    pub page: usize,

    pub position: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PanelBars {
    pub vertical: Option<Bar>,

    pub horizontal: Option<Bar>,
}

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

    pub fn vertical(&mut self, total: usize, page: usize, position: usize) -> &mut Self {
        self.v = Some(ScrollState {
            total,
            page,
            position,
        });
        self
    }

    pub fn horizontal(&mut self, total: usize, page: usize, position: usize) -> &mut Self {
        self.h = Some(ScrollState {
            total,
            page,
            position,
        });
        self
    }

    pub fn thumb(&mut self, color: Color) -> &mut Self {
        self.thumb = color;
        self
    }

    pub fn pressed_v(&mut self, pressed: Option<Arrow>) -> &mut Self {
        self.pressed_v = pressed;
        self
    }

    pub fn pressed_h(&mut self, pressed: Option<Arrow>) -> &mut Self {
        self.pressed_h = pressed;
        self
    }

    pub fn render_block(&self, f: &mut Frame, area: Rect, block: Block) -> Rect {
        let inner = block.inner(area);
        f.render_widget(block, area);
        inner
    }

    pub fn render_bars(&self, f: &mut Frame, area: Rect) -> PanelBars {
        PanelBars {
            vertical: self.draw_axis(f, area, true),
            horizontal: self.draw_axis(f, area, false),
        }
    }

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
