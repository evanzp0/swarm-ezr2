use std::time::{Duration, Instant};

use crossterm::event::MouseEvent;

use crate::scrollbar;

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SbKey {
    List,

    ConnsV,

    ConnsH,

    Pieces,

    Help,

    Val,
}

impl SbKey {
    pub fn step(self) -> i32 {
        match self {
            SbKey::ConnsH => 2,
            _ => 1,
        }
    }

    fn bar(&self, app: &App) -> Option<scrollbar::Bar> {
        match self {
            SbKey::List => app.list_sb,
            SbKey::ConnsV => app.conns_v_sb,
            SbKey::ConnsH => app.conns_h_sb,
            SbKey::Pieces => app.pieces_sb,
            SbKey::Help => app.help_sb,
            SbKey::Val => app.val_sb,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SbHold {
    pub key: SbKey,

    pub dir: i32,

    pub page: bool,
    pub since: Instant,
    pub last: Instant,
}

impl SbHold {
    const FIRST_DELAY: Duration = Duration::from_millis(400);

    const REPEAT: Duration = Duration::from_millis(120);

    fn due(&self) -> Instant {
        if self.since == self.last {
            self.since + Self::FIRST_DELAY
        } else {
            self.last + Self::REPEAT
        }
    }
}
impl App {
    pub(super) fn sb_max(&self, key: SbKey) -> usize {
        match key {
            SbKey::List => self
                .filtered()
                .len()
                .saturating_sub(self.visible_rows.max(1)),

            SbKey::ConnsV => self
                .conn_count()
                .saturating_sub(self.visible_conns_rows.max(1)),

            SbKey::ConnsH => self.conns_max_hscroll,
            SbKey::Pieces => self.pieces_max_scroll,
            SbKey::Help => self.help_max_scroll,
            SbKey::Val => self.val_max_scroll,
        }
    }

    pub(super) fn sb_scroll(&mut self, key: SbKey, delta: i32) {
        let max = self.sb_max(key);
        match key {
            SbKey::List => {
                self.scroll = clamp_offset(self.scroll as i64 + delta as i64, max);
            }
            SbKey::ConnsV => {
                self.conns_scroll = clamp_offset(self.conns_scroll as i64 + delta as i64, max);
            }
            SbKey::ConnsH => {
                self.conns_hscroll = clamp_offset(self.conns_hscroll as i64 + delta as i64, max);
            }
            SbKey::Pieces => {
                self.pieces_scroll = clamp_offset(self.pieces_scroll as i64 + delta as i64, max);
            }
            SbKey::Help => {
                self.help_scroll = clamp_offset(self.help_scroll as i64 + delta as i64, max);
            }
            SbKey::Val => {
                self.val_scroll = clamp_offset(self.val_scroll as i64 + delta as i64, max);
            }
        }
    }

    pub(super) fn sb_set(&mut self, key: SbKey, pos: usize) {
        let max = self.sb_max(key);
        let pos = pos.min(max);
        match key {
            SbKey::List => self.scroll = pos,
            SbKey::ConnsV => self.conns_scroll = pos,
            SbKey::ConnsH => self.conns_hscroll = pos,
            SbKey::Pieces => self.pieces_scroll = pos,
            SbKey::Help => self.help_scroll = pos,
            SbKey::Val => self.val_scroll = pos,
        }
    }

    pub(super) fn sb_page(&self, key: SbKey) -> i32 {
        key.bar(self).map(|b| b.page.max(1) as i32).unwrap_or(1)
    }

    pub(super) fn sb_press(&mut self, key: SbKey, bar: &scrollbar::Bar, hit: scrollbar::Hit) {
        match hit {
            scrollbar::Hit::Thumb(grab) => {
                self.sb_drag = Some((key, grab));
            }
            scrollbar::Hit::Head => {
                let step = key.step();
                self.sb_scroll(key, -step);
                self.sb_pressed = Some((key, scrollbar::Arrow::Head));
                self.sb_flash = Some((
                    key,
                    scrollbar::Arrow::Head,
                    Instant::now() + Duration::from_millis(400),
                ));
                self.sb_hold = Some(SbHold {
                    key,
                    dir: -1,
                    page: false,
                    since: Instant::now(),
                    last: Instant::now(),
                });
            }
            scrollbar::Hit::Tail => {
                let step = key.step();
                self.sb_scroll(key, step);
                self.sb_pressed = Some((key, scrollbar::Arrow::Tail));
                self.sb_flash = Some((
                    key,
                    scrollbar::Arrow::Tail,
                    Instant::now() + Duration::from_millis(400),
                ));
                self.sb_hold = Some(SbHold {
                    key,
                    dir: 1,
                    page: false,
                    since: Instant::now(),
                    last: Instant::now(),
                });
            }
            scrollbar::Hit::Track(t) => {
                let dir = if t < bar.thumb_pos() { -1 } else { 1 };
                let page = self.sb_page(key);
                self.sb_scroll(key, dir * page);
                self.sb_hold = Some(SbHold {
                    key,
                    dir,
                    page: true,
                    since: Instant::now(),
                    last: Instant::now(),
                });
            }
        }
    }

    pub(super) fn sb_drag_move(&mut self, m: MouseEvent) {
        let Some((key, grab)) = self.sb_drag else {
            return;
        };
        let Some(bar) = key.bar(self) else { return };
        let t = bar.track_index_at(m.column, m.row);
        let pos = bar.pos_from_thumb(t.saturating_sub(grab));
        self.sb_set(key, pos);
    }

    pub(super) fn sb_release(&mut self) {
        self.sb_drag = None;
        self.sb_hold = None;
        self.sb_pressed = None;
    }

    pub(super) fn sb_arrow_follow(&mut self, m: MouseEvent) {
        if self.sb_drag.is_some() {
            self.sb_pressed = None;
            return;
        }
        let Some(h) = self.sb_hold else {
            return;
        };
        if h.page {
            return;
        }
        let arrow = if h.dir < 0 {
            scrollbar::Arrow::Head
        } else {
            scrollbar::Arrow::Tail
        };
        let expect = match arrow {
            scrollbar::Arrow::Head => scrollbar::Hit::Head,
            scrollbar::Arrow::Tail => scrollbar::Hit::Tail,
        };
        let on = h.key.bar(self).and_then(|b| b.hit(m.column, m.row)) == Some(expect);
        self.sb_pressed = if on { Some((h.key, arrow)) } else { None };
    }

    pub fn sb_arrow_pressed(&self, key: SbKey) -> Option<scrollbar::Arrow> {
        if let Some((k, a)) = self.sb_pressed {
            if k == key {
                return Some(a);
            }
        }
        if let Some((k, a, until)) = self.sb_flash {
            if k == key && Instant::now() < until {
                return Some(a);
            }
        }
        None
    }

    pub(super) fn sb_hold_tick(&mut self, now: Instant) {
        let Some(h) = self.sb_hold else { return };
        if now < h.due() {
            return;
        }
        let step = if h.page {
            self.sb_page(h.key)
        } else {
            h.key.step()
        };
        self.sb_scroll(h.key, h.dir * step);
        if let Some(h) = &mut self.sb_hold {
            h.last = now;
        }
    }
}

pub(crate) fn clamp_offset(v: i64, max: usize) -> usize {
    v.clamp(0, max as i64) as usize
}
