use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::Frame;

pub(crate) const DIM: Color = Color::Rgb(126, 141, 147);
pub(crate) const DIM2: Color = Color::Rgb(88, 102, 108);

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
    #[must_use]
    pub fn vertical(panel: Rect, total: usize, page: usize, pos: usize) -> Self {
        Self {
            panel,
            vertical: true,
            total,
            page,
            pos,
        }
    }

    #[must_use]
    pub fn horizontal(panel: Rect, total: usize, page: usize, pos: usize) -> Self {
        Self {
            panel,
            vertical: false,
            total,
            page,
            pos,
        }
    }

    #[must_use]
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

    #[must_use]
    pub fn track_len(&self) -> usize {
        let (a, b) = self.arrow_span();
        ((b.saturating_sub(a) + 1) as usize).saturating_sub(2 + 2 * self.arrow_pad() as usize)
    }

    #[must_use]
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

    #[must_use]
    pub fn travel(&self) -> usize {
        self.track_len().saturating_sub(self.thumb_len())
    }

    #[must_use]
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

    #[must_use]
    pub fn pos_from_thumb(&self, thumb_at: usize) -> usize {
        let max = self.max_pos();
        let travel = self.travel();
        if max == 0 || travel == 0 {
            return 0;
        }
        let at = thumb_at.clamp(0, travel);
        ((at as f64 / travel as f64) * max as f64).round() as usize
    }

    fn along_vertical(&self, col: u16, row: u16, a: u16, b: u16) -> Option<u16> {
        if col != self.panel.x + self.panel.width.saturating_sub(1) {
            return None;
        }
        if row < a || row > b {
            return None;
        }
        Some(row)
    }

    fn along_horizontal(&self, col: u16, row: u16, a: u16, b: u16) -> Option<u16> {
        if row != self.panel.y + self.panel.height.saturating_sub(1) {
            return None;
        }
        if col < a || col > b {
            return None;
        }
        Some(col)
    }

    fn edge_hit(along: u16, a: u16, b: u16) -> Option<Hit> {
        if along == a {
            return Some(Hit::Head);
        }
        if along == b {
            return Some(Hit::Tail);
        }
        None
    }

    fn track_hit(&self, along: u16) -> Option<Hit> {
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

    #[must_use]
    pub fn hit(&self, col: u16, row: u16) -> Option<Hit> {
        if self.total <= self.page || self.page == 0 || self.track_len() < 2 {
            return None;
        }
        let (a, b) = self.arrow_span();
        let along = if self.vertical {
            self.along_vertical(col, row, a, b)
        } else {
            self.along_horizontal(col, row, a, b)
        }?;
        Self::edge_hit(along, a, b).or_else(|| self.track_hit(along))
    }

    #[must_use]
    pub fn track_index_at(&self, col: u16, row: u16) -> usize {
        let along = if self.vertical { row } else { col };
        let len = self.track_len();
        (along.saturating_sub(self.track_origin()) as usize).min(len)
    }
}

fn glyph_set(vertical: bool) -> (&'static str, &'static str, &'static str, &'static str) {
    if vertical {
        ("↑", "↓", "│", "║")
    } else {
        ("←", "→", "─", "═")
    }
}

fn min_span_ok(bar: &Bar) -> bool {
    let (long_ok, cross_ok) = if bar.vertical {
        (bar.panel.height >= 2 + 2 + 2, bar.panel.width > 0)
    } else {
        (
            bar.panel.width >= 2 + 2 * EDGE_PAD + 2 + 2 * ARROW_PAD + 2,
            bar.panel.height > 0,
        )
    };
    long_ok && cross_ok
}

fn pressed_arrow_style(
    pressed: Option<Arrow>,
    want: Arrow,
    normal: Style,
    pressed_s: Style,
) -> Style {
    if pressed == Some(want) {
        pressed_s
    } else {
        normal
    }
}

#[allow(clippy::too_many_arguments)]
fn edge_glyph(
    p: u16,
    a: u16,
    b: u16,
    pressed: Option<Arrow>,
    head: &'static str,
    tail: &'static str,
    arrow_style: Style,
    pressed_s: Style,
    blank_style: Style,
) -> Option<(&'static str, Style)> {
    if p < a || p > b {
        return Some((" ", blank_style));
    }
    if p == a {
        return Some((
            head,
            pressed_arrow_style(pressed, Arrow::Head, arrow_style, pressed_s),
        ));
    }
    if p == b {
        return Some((
            tail,
            pressed_arrow_style(pressed, Arrow::Tail, arrow_style, pressed_s),
        ));
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn track_glyph(
    p: u16,
    org: u16,
    track_end: u16,
    tp: usize,
    tl: usize,
    track_glyph_c: &'static str,
    thumb_glyph_c: &'static str,
    track_style: Style,
    thumb_style: Style,
    blank_style: Style,
) -> (&'static str, Style) {
    if p < org || p >= track_end {
        return (" ", blank_style);
    }
    let t = (p - org) as usize;
    if t >= tp && t < tp + tl {
        (thumb_glyph_c, thumb_style)
    } else {
        (track_glyph_c, track_style)
    }
}

fn paint(f: &mut Frame, bar: &Bar, p: u16, fixed: u16, sym: &str, style: Style) {
    let (col, row) = if bar.vertical { (fixed, p) } else { (p, fixed) };
    if let Some(cell) = f.buffer_mut().cell_mut((col, row)) {
        cell.set_symbol(sym);
        cell.set_style(style);
    }
}

pub fn render(f: &mut Frame, bar: &Bar, thumb_color: Color, pressed: Option<Arrow>) {
    if bar.page == 0 || bar.total <= bar.page {
        return;
    }
    if !min_span_ok(bar) {
        return;
    }
    let (a, b) = bar.arrow_span();
    let arrow_style = Style::default().fg(DIM);
    let track_style = Style::default().fg(DIM2);
    let thumb_style = Style::default()
        .fg(thumb_color)
        .add_modifier(Modifier::BOLD);
    let pressed_s = thumb_style;
    let blank_style = Style::default();
    let (head, tail, track_glyph_c, thumb_glyph_c) = glyph_set(bar.vertical);
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
        let (sym, style) = edge_glyph(
            p,
            a,
            b,
            pressed,
            head,
            tail,
            arrow_style,
            pressed_s,
            blank_style,
        )
        .unwrap_or_else(|| {
            track_glyph(
                p,
                org,
                track_end,
                tp,
                tl,
                track_glyph_c,
                thumb_glyph_c,
                track_style,
                thumb_style,
                blank_style,
            )
        });
        paint(f, bar, p, fixed, sym, style);
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn vbar(h: u16, total: usize, page: usize, pos: usize) -> Bar {
        Bar::vertical(Rect::new(0, 0, 40, h), total, page, pos)
    }

    #[test]
    fn vertical_geometry_track_arrows_no_padding() {
        let bar = vbar(8, 100, 10, 0);
        assert_eq!(bar.track_len(), 4, "轨道 = 可绘格 − 2 箭头（垂直无留白）");
        assert_eq!(bar.thumb_len(), 1, "滑块占页比例取整 + 行程保证");
        assert_eq!(bar.travel(), 3);
        assert_eq!(bar.thumb_pos(), 0, "首屏贴头吸附");
        assert_eq!(bar.thumb_pos(), 0);
        let end = vbar(8, 100, 10, 90);
        assert_eq!(end.thumb_pos(), 3, "末页贴尾吸附");
        let mid = vbar(8, 100, 10, 45);
        assert_eq!(mid.thumb_pos(), 2, "中间按比例 round");
    }

    #[test]
    fn thumb_drag_roundtrip_is_exact_inverse() {
        let bar = vbar(20, 500, 10, 0);
        let travel = bar.travel();
        for at in 0..=travel {
            let pos = bar.pos_from_thumb(at);
            let back = Bar { pos, ..bar }.thumb_pos();
            assert_eq!(back, at, "拖拽反解与正向严格互逆（at={at}）");
        }
    }

    #[test]
    fn travel_guarantee_thumb_never_fills_track() {
        let bar = vbar(10, 11, 10, 0);
        assert!(bar.max_pos() > 0);
        assert_eq!(bar.thumb_len(), bar.track_len() - 1, "行程保证 ≥ 1");
        assert_eq!(bar.travel(), 1);
    }

    #[test]
    fn display_conditions() {
        let none = vbar(20, 10, 10, 0);
        assert_eq!(none.hit(39, 5), None);
        let tiny = vbar(5, 100, 2, 0);
        assert_eq!(tiny.track_len(), 1, "高 5 = 2 角 + 2 箭头 + 1 轨道");
        assert_eq!(tiny.hit(39, 2), None, "轨道 < 2 不可命中（退化态）");
    }

    #[test]
    fn hit_regions_and_blank_cells_unclickable() {
        let bar = vbar(10, 100, 10, 0);
        assert_eq!(bar.hit(39, 1), Some(Hit::Head), "上箭头行命中 Head");
        assert_eq!(bar.hit(39, 8), Some(Hit::Tail), "下箭头行命中 Tail");
        assert_eq!(bar.hit(39, 0), None, "上角行不可命中");
        assert_eq!(bar.hit(38, 5), None, "非右边框列不可命中");
        assert!(matches!(bar.hit(39, 2), Some(Hit::Thumb(_))));
        assert!(matches!(bar.hit(39, 5), Some(Hit::Track(_))));
    }

    #[test]
    fn horizontal_has_edge_padding() {
        let bar = Bar::horizontal(Rect::new(0, 0, 20, 10), 200, 20, 0);
        assert_eq!(bar.track_len(), 12, "水平轨道 = 区间 − 2 箭头 − 2 内留白");
        assert_eq!(bar.hit(0, 9), None, "外侧留白（角旁）不可点");
        assert_eq!(bar.hit(1, 9), None, "外侧留白格不可点");
        assert_eq!(bar.hit(2, 9), Some(Hit::Head), "左箭头");
        assert_eq!(bar.hit(3, 9), None, "内侧留白格不可点");
        assert_eq!(bar.hit(17, 9), Some(Hit::Tail), "右箭头");
        assert_eq!(bar.hit(18, 9), None, "外侧留白格不可点");
        assert!(matches!(bar.hit(4, 9), Some(Hit::Thumb(_))));
    }

    #[test]
    fn degenerate_geometry_guards_return_zero() {
        assert_eq!(vbar(2, 100, 10, 0).thumb_len(), 0, "轨道 0 → 滑块 0");
        assert_eq!(vbar(8, 10, 10, 0).thumb_pos(), 0, "max_pos 0 → 0");
        assert_eq!(vbar(3, 100, 2, 0).thumb_pos(), 0, "travel 0 → 0");
        assert_eq!(vbar(8, 10, 10, 0).pos_from_thumb(3), 0, "max_pos 0 → 0");
        assert_eq!(vbar(3, 100, 2, 0).pos_from_thumb(3), 0, "travel 0 → 0");
    }

    #[test]
    fn horizontal_hit_wrong_row_is_none() {
        let bar = Bar::horizontal(Rect::new(0, 0, 20, 10), 200, 20, 0);
        assert_eq!(bar.hit(4, 3), None, "水平条非底行不可命中");
    }

    fn draw(bar: &Bar, pressed: Option<Arrow>) -> ratatui::buffer::Buffer {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let mut term = Terminal::new(TestBackend::new(40, 12)).unwrap();
        term.draw(|f| render(f, bar, Color::Blue, pressed)).unwrap();
        term.backend().buffer().clone()
    }

    #[test]
    fn render_noop_when_not_scrollable() {
        let buf = draw(&vbar(8, 10, 10, 0), None);
        for y in 0..12 {
            assert_eq!(buf[(39, y)].symbol(), " ", "不可滚动时渲染零改动");
        }
    }

    #[test]
    fn render_pressed_arrow_takes_thumb_style() {
        let bar = vbar(8, 100, 10, 0);
        let none = draw(&bar, None);
        let head = draw(&bar, Some(Arrow::Head));
        let tail = draw(&bar, Some(Arrow::Tail));
        assert!(
            !none[(39, 1)].style().add_modifier.contains(Modifier::BOLD),
            "未按下箭头无 BOLD"
        );
        assert!(
            head[(39, 1)].style().add_modifier.contains(Modifier::BOLD),
            "按下上箭头 → BOLD"
        );
        assert!(
            tail[(39, 6)].style().add_modifier.contains(Modifier::BOLD),
            "按下下箭头 → BOLD"
        );
        assert!(
            !head[(39, 6)].style().add_modifier.contains(Modifier::BOLD),
            "未按下臂仍无 BOLD"
        );
        assert_eq!(head[(39, 1)].symbol(), "↑");
        assert_eq!(tail[(39, 6)].symbol(), "↓");
    }

    #[test]
    fn render_out_of_bounds_panel_is_silent_noop() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let bar = Bar::vertical(Rect::new(0, 0, 40, 8), 100, 10, 0);
        let mut term = Terminal::new(TestBackend::new(10, 5)).unwrap();
        term.draw(|f| render(f, &bar, Color::Blue, None)).unwrap();
    }
}
