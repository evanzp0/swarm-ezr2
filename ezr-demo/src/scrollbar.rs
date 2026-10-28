use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    Frame,
};

use crate::ui::{DIM, DIM2};

const EDGE_PAD: u16 = 1;

const ARROW_PAD: u16 = 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arrow {
    Head,

    Tail,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hit {
    Head,

    Tail,

    Track(usize),

    Thumb(usize),
}

#[derive(Clone, Copy, Debug)]
pub struct Bar {
    pub panel: Rect,
    pub vertical: bool,

    pub total: usize,

    pub page: usize,

    pub pos: usize,
}

impl Bar {
    pub fn vertical(panel: Rect, total: usize, page: usize, pos: usize) -> Self {
        Self {
            panel,
            vertical: true,
            total,
            page,
            pos,
        }
    }

    pub fn horizontal(panel: Rect, total: usize, page: usize, pos: usize) -> Self {
        Self {
            panel,
            vertical: false,
            total,
            page,
            pos,
        }
    }

    pub fn max_pos(&self) -> usize {
        self.total.saturating_sub(self.page)
    }

    fn edge_pad(&self) -> u16 {
        if self.vertical {
            0
        } else {
            EDGE_PAD
        }
    }

    fn arrow_pad(&self) -> u16 {
        if self.vertical {
            0
        } else {
            ARROW_PAD
        }
    }

    fn arrow_span(&self) -> (u16, u16) {
        let pad = self.edge_pad();
        if self.vertical {
            (
                self.panel.y + 1 + pad,
                self.panel.y + self.panel.height.saturating_sub(2 + pad),
            )
        } else {
            (
                self.panel.x + 1 + pad,
                self.panel.x + self.panel.width.saturating_sub(2 + pad),
            )
        }
    }

    fn track_origin(&self) -> u16 {
        self.arrow_span().0 + 1 + self.arrow_pad()
    }

    pub fn track_len(&self) -> usize {
        let (a, b) = self.arrow_span();

        ((b.saturating_sub(a) + 1) as usize).saturating_sub(2 + 2 * self.arrow_pad() as usize)
    }

    pub fn thumb_len(&self) -> usize {
        let track = self.track_len();
        if track == 0 {
            return 0;
        }
        let raw = (((self.page as f64) / (self.total as f64)) * track as f64).round() as usize;
        let mut len = raw.clamp(1, track);
        if self.max_pos() > 0 && track >= 2 {
            len = len.min(track - 1);
        }
        len
    }

    pub fn travel(&self) -> usize {
        self.track_len().saturating_sub(self.thumb_len())
    }

    pub fn thumb_pos(&self) -> usize {
        let max = self.max_pos();
        let travel = self.travel();
        if max == 0 || travel == 0 {
            return 0;
        }
        if self.pos >= max {
            return travel;
        }
        ((self.pos as f64 / max as f64) * travel as f64).round() as usize
    }

    pub fn pos_from_thumb(&self, thumb_at: usize) -> usize {
        let max = self.max_pos();
        let travel = self.travel();
        if max == 0 || travel == 0 {
            return 0;
        }
        let at = thumb_at.clamp(0, travel);
        ((at as f64 / travel as f64) * max as f64).round() as usize
    }

    pub fn hit(&self, col: u16, row: u16) -> Option<Hit> {
        if self.total <= self.page || self.page == 0 || self.track_len() < 2 {
            return None;
        }
        let (a, b) = self.arrow_span();
        let along = if self.vertical {
            if col != self.panel.x + self.panel.width.saturating_sub(1) {
                return None;
            }
            if row < a || row > b {
                return None;
            }
            row
        } else {
            if row != self.panel.y + self.panel.height.saturating_sub(1) {
                return None;
            }
            if col < a || col > b {
                return None;
            }
            col
        };
        if along == a {
            return Some(Hit::Head);
        }
        if along == b {
            return Some(Hit::Tail);
        }

        let org = self.track_origin();
        if along < org || (along - org) as usize >= self.track_len() {
            return None;
        }
        let t = (along - org) as usize;
        let tp = self.thumb_pos();
        let tl = self.thumb_len();
        if t >= tp && t < tp + tl {
            Some(Hit::Thumb(t - tp))
        } else {
            Some(Hit::Track(t))
        }
    }

    pub fn track_index_at(&self, col: u16, row: u16) -> usize {
        let along = if self.vertical { row } else { col };
        let len = self.track_len();
        (along.saturating_sub(self.track_origin()) as usize).min(len)
    }
}

pub fn render(f: &mut Frame, bar: &Bar, thumb_color: Color, pressed: Option<Arrow>) {
    if bar.page == 0 || bar.total <= bar.page {
        return;
    }

    let min_len = if bar.vertical {
        2 + 2 + 2
    } else {
        2 + 2 * EDGE_PAD + 2 + 2 * ARROW_PAD + 2
    };
    let span_ok = if bar.vertical {
        bar.panel.height >= min_len && bar.panel.width > 0
    } else {
        bar.panel.width >= min_len && bar.panel.height > 0
    };
    if !span_ok {
        return;
    }
    let (a, b) = bar.arrow_span();
    let arrow_style = Style::default().fg(DIM);
    let track_style = Style::default().fg(DIM2);
    let thumb_style = Style::default()
        .fg(thumb_color)
        .add_modifier(Modifier::BOLD);

    let pressed_style = thumb_style;
    let blank_style = Style::default();

    let (head, tail, track_glyph, thumb_glyph) = if bar.vertical {
        ("↑", "↓", "│", "║")
    } else {
        ("←", "→", "─", "═")
    };
    let tp = bar.thumb_pos();
    let tl = bar.thumb_len();
    let fixed = if bar.vertical {
        bar.panel.x + bar.panel.width.saturating_sub(1)
    } else {
        bar.panel.y + bar.panel.height.saturating_sub(1)
    };

    let ep = bar.edge_pad();
    let lo = a.saturating_sub(ep);
    let hi = b.saturating_add(ep);
    let org = bar.track_origin();
    let track_end = org + bar.track_len() as u16;
    for p in lo..=hi {
        let (sym, style) = if p < a || p > b {
            (" ", blank_style)
        } else if p == a {
            (
                head,
                if pressed == Some(Arrow::Head) {
                    pressed_style
                } else {
                    arrow_style
                },
            )
        } else if p == b {
            (
                tail,
                if pressed == Some(Arrow::Tail) {
                    pressed_style
                } else {
                    arrow_style
                },
            )
        } else if p < org || p >= track_end {
            (" ", blank_style)
        } else {
            let t = (p - org) as usize;
            if t >= tp && t < tp + tl {
                (thumb_glyph, thumb_style)
            } else {
                (track_glyph, track_style)
            }
        };
        let (col, row) = if bar.vertical { (fixed, p) } else { (p, fixed) };
        if let Some(cell) = f.buffer_mut().cell_mut((col, row)) {
            cell.set_symbol(sym);
            cell.set_style(style);
        }
    }
}
