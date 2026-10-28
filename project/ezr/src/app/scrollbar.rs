use std::time::{Duration, Instant};

use super::{App, Panel};
use crate::scrollbar::{Arrow as SbArrow, Bar as SbBar, Hit as SbHit};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SbKey {
    List,
    ConnsV,
    ConnsH,
    Pieces,
    Help,
    Val,
    CmdText,
    BtFiles,
    BtTrackers,
    BtTrackersH,
}

impl SbKey {
    #[must_use]
    pub fn step(self) -> i32 {
        match self {
            SbKey::ConnsH => 2,
            _ => 1,
        }
    }

    pub(crate) fn bar(self, app: &App) -> Option<SbBar> {
        match self {
            SbKey::List => app.list_scrollbar,
            SbKey::ConnsV => app.conns_v_scrollbar,
            SbKey::ConnsH => app.conns_h_scrollbar,
            SbKey::Pieces => app.pieces_scrollbar,
            SbKey::Help => app.help_scrollbar,
            SbKey::Val => app.val_scrollbar,
            SbKey::CmdText => app.cmd_sb,
            SbKey::BtFiles => app.dlg_bt_sb,
            SbKey::BtTrackers => app.dlg_trackers_sb,
            SbKey::BtTrackersH => app.dlg_trackers_h_sb,
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
    fn sb_max(&self, key: SbKey) -> usize {
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
            SbKey::CmdText => self.cmd.max_scroll,
            SbKey::BtFiles => self.dialog.as_ref().map_or(0, |d| {
                d.bt_row_count().saturating_sub(self.dlg_bt_visible.max(1))
            }),
            SbKey::BtTrackers => self.dialog.as_ref().map_or(0, |d| d.trackers.max_scroll),
            SbKey::BtTrackersH => self.dialog.as_ref().map_or(0, |d| d.trackers.max_scroll_x),
        }
    }

    pub(super) fn sb_scroll(&mut self, key: SbKey, delta: i32) {
        let max = self.sb_max(key);
        let clamp = |v: i64| v.clamp(0, max as i64) as usize;
        match key {
            SbKey::List => self.scroll = clamp(self.scroll as i64 + delta as i64),
            SbKey::ConnsV => self.conns_scroll = clamp(self.conns_scroll as i64 + delta as i64),
            SbKey::ConnsH => self.conns_hscroll = clamp(self.conns_hscroll as i64 + delta as i64),
            SbKey::Pieces => self.pieces_scroll = clamp(self.pieces_scroll as i64 + delta as i64),
            SbKey::Help => self.help_scroll = clamp(self.help_scroll as i64 + delta as i64),
            SbKey::Val => self.val_scroll = clamp(self.val_scroll as i64 + delta as i64),
            SbKey::CmdText => {
                if delta < 0 {
                    self.cmd.scroll_up(delta.unsigned_abs() as usize);
                } else {
                    self.cmd.scroll_down(delta as usize);
                }
            }
            SbKey::BtFiles => {
                if let Some(d) = self.dialog.as_mut() {
                    d.bt_scroll = clamp(d.bt_scroll as i64 + delta as i64);
                }
            }
            SbKey::BtTrackers => {
                if let Some(d) = self.dialog.as_mut() {
                    if delta < 0 {
                        d.trackers.scroll_up(delta.unsigned_abs() as usize);
                    } else {
                        d.trackers.scroll_down(delta as usize);
                    }
                }
            }
            SbKey::BtTrackersH => {
                if let Some(d) = self.dialog.as_mut() {
                    if delta < 0 {
                        d.trackers.scroll_x_by(delta.unsigned_abs() as usize);
                    } else {
                        d.trackers.scroll_x_by(delta as usize);
                    }
                }
            }
        }
    }

    pub(super) fn sb_set(&mut self, key: SbKey, pos: usize) {
        let max = self.sb_max(key);
        let v = pos.min(max);
        match key {
            SbKey::List => self.scroll = v,
            SbKey::ConnsV => self.conns_scroll = v,
            SbKey::ConnsH => self.conns_hscroll = v,
            SbKey::Pieces => self.pieces_scroll = v,
            SbKey::Help => self.help_scroll = v,
            SbKey::Val => self.val_scroll = v,
            SbKey::CmdText => self.cmd.scroll_to(v),
            SbKey::BtFiles => {
                if let Some(d) = self.dialog.as_mut() {
                    d.bt_scroll = v;
                }
            }
            SbKey::BtTrackers => {
                if let Some(d) = self.dialog.as_mut() {
                    d.trackers.scroll_x_to(0);
                    d.trackers.scroll_to(v);
                }
            }
            SbKey::BtTrackersH => {
                if let Some(d) = self.dialog.as_mut() {
                    d.trackers.scroll_x_to(v);
                }
            }
        }
    }

    fn sb_page(&self, key: SbKey) -> i32 {
        key.bar(self).map_or(1, |b| b.page.max(1) as i32)
    }

    pub(super) fn sb_press(&mut self, key: SbKey, bar: &SbBar, hit: SbHit) {
        match hit {
            SbHit::Thumb(grab) => {
                self.sb_drag = Some((key, grab));
            }
            SbHit::Head => {
                let step = key.step();
                self.sb_scroll(key, -step);
                self.sb_pressed = Some((key, SbArrow::Head));
                self.sb_flash = Some((
                    key,
                    SbArrow::Head,
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
            SbHit::Tail => {
                let step = key.step();
                self.sb_scroll(key, step);
                self.sb_pressed = Some((key, SbArrow::Tail));
                self.sb_flash = Some((
                    key,
                    SbArrow::Tail,
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
            SbHit::Track(t) => {
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

    pub(super) fn sb_drag_move(&mut self, col: u16, row: u16) {
        let Some((key, grab)) = self.sb_drag else {
            return;
        };
        let Some(bar) = key.bar(self) else {
            return;
        };
        let t = bar.track_index_at(col, row);
        let pos = bar.pos_from_thumb(t.saturating_sub(grab));
        self.sb_set(key, pos);
    }

    pub(super) fn sb_release(&mut self) {
        self.sb_drag = None;
        self.sb_hold = None;
        self.sb_pressed = None;
    }

    pub(crate) fn sb_hold_tick(&mut self, now: Instant) {
        let Some(h) = self.sb_hold else {
            return;
        };
        if now < h.due() {
            return;
        }
        let delta = if h.page {
            self.sb_page(h.key)
        } else {
            h.key.step()
        };
        self.sb_scroll(h.key, h.dir * delta);
        if let Some(hh) = self.sb_hold.as_mut() {
            hh.last = now;
        }
    }

    #[must_use]
    pub fn sb_arrow_pressed(&self, key: SbKey) -> Option<SbArrow> {
        if let Some((k, a)) = self.sb_pressed {
            if k == key {
                return Some(a);
            }
        }
        match self.sb_flash {
            Some((k, a, until)) if k == key && Instant::now() < until => Some(a),
            _ => None,
        }
    }

    pub(crate) fn sb_arrow_follow(&mut self, col: u16, row: u16) {
        let Some(h) = self.sb_hold else {
            return;
        };
        if h.page {
            return;
        }
        let key = h.key;
        let arrow = if h.dir < 0 {
            SbArrow::Head
        } else {
            SbArrow::Tail
        };
        let on_arrow = key
            .bar(self)
            .and_then(|b| b.hit(col, row))
            .is_some_and(|hit| match hit {
                SbHit::Head => arrow == SbArrow::Head,
                SbHit::Tail => arrow == SbArrow::Tail,
                _ => false,
            });
        if on_arrow {
            self.sb_pressed = Some((key, arrow));
        } else if self.sb_pressed == Some((key, arrow)) {
            self.sb_pressed = None;
        }
    }

    pub(crate) fn sb_press_at(&mut self, col: u16, row: u16) -> bool {
        for (key, focus) in [
            (SbKey::List, Panel::List),
            (SbKey::ConnsV, Panel::Conns),
            (SbKey::ConnsH, Panel::Conns),
        ] {
            if let Some(bar) = key.bar(self) {
                if let Some(hit) = bar.hit(col, row) {
                    self.focus = focus;
                    self.sb_press(key, &bar, hit);
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod sb_interaction_tests {

    use ratatui::layout::Rect;

    use super::*;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-sb", tag)
    }

    fn vbar() -> SbBar {
        SbBar::vertical(Rect::new(0, 0, 40, 20), 30, 5, 0)
    }

    fn seed_scrollbar(app: &mut App) -> SbBar {
        app.list_scrollbar = Some(vbar());
        app.visible_rows = 5;
        crate::app::testutil::seed_queued(app, 30);
        app.list_scrollbar.unwrap()
    }

    fn refill(app: &mut App) -> SbBar {
        let bar = SbBar {
            pos: app.scroll,
            ..app.list_scrollbar.unwrap()
        };
        app.list_scrollbar = Some(bar);
        bar
    }

    #[tokio::test]
    async fn arrow_press_steps_and_focuses_panel() {
        let mut app = make_app("arrow");
        let bar = seed_scrollbar(&mut app);
        app.focus = Panel::Detail;
        let hit = bar.hit(39, 18);
        assert!(matches!(hit, Some(SbHit::Tail)));
        app.sb_press(SbKey::List, &bar, hit.unwrap());
        assert_eq!(app.scroll, 1, "尾箭头步进 1");
        assert!(app.sb_hold.is_some(), "记录连发");
        assert_eq!(
            app.sb_pressed,
            Some((SbKey::List, SbArrow::Tail)),
            "按压高亮"
        );
        assert!(app.sb_flash.is_some(), "400ms 余辉");
        app.focus = Panel::Detail;
        assert!(app.sb_press_at(39, 18), "尾箭头再次命中");
        assert_eq!(app.focus, Panel::List, "按下滚动条聚焦所属面板");
        assert_eq!(app.sb_arrow_pressed(SbKey::List), Some(SbArrow::Tail));
        app.scroll = 0;
        let hit = bar.hit(39, 1);
        assert!(matches!(hit, Some(SbHit::Head)));
        app.sb_press(SbKey::List, &bar, hit.unwrap());
        assert_eq!(app.scroll, 0, "事件内立即钳制（末页/首页即停）");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn track_press_pages_toward_click_side() {
        let mut app = make_app("track");
        let bar = seed_scrollbar(&mut app);
        let hit = bar.hit(39, 10);
        assert!(matches!(hit, Some(SbHit::Track(_))));
        app.sb_press(SbKey::List, &bar, hit.unwrap());
        assert_eq!(app.scroll, 5, "整页翻页（page = 可视 5）");
        assert!(app.sb_hold.as_ref().is_some_and(|h| h.page), "轨道连发按页");
        app.sb_set(SbKey::List, 25);
        let bar = refill(&mut app);
        let hit = bar.hit(39, 3);
        if let Some(SbHit::Track(t)) = hit {
            assert!(t < bar.thumb_pos(), "点击点在滑块上方");
            app.sb_press(SbKey::List, &bar, hit.unwrap());
            assert_eq!(app.scroll, 20, "朝头整页翻页（25 − page 5）");
        } else {
            panic!("轨道上半段应命中 Track");
        }
        app.shutdown().await;
    }

    #[tokio::test]
    async fn thumb_drag_inverse_and_release() {
        let mut app = make_app("thumb");
        let bar = seed_scrollbar(&mut app);
        let hit = bar.hit(39, 2);
        assert!(matches!(hit, Some(SbHit::Thumb(0))));
        app.sb_press(SbKey::List, &bar, hit.unwrap());
        assert_eq!(app.sb_drag, Some((SbKey::List, 0)));
        app.sb_drag_move(39, 10);
        let expected = bar.pos_from_thumb(8);
        assert_eq!(app.scroll, expected, "拖拽反解与显示取整互逆");
        app.sb_release();
        assert!(app.sb_drag.is_none() && app.sb_hold.is_none() && app.sb_pressed.is_none());
        app.shutdown().await;
    }

    #[tokio::test]
    async fn hold_autorepeat_first_delay_then_period() {
        let mut app = make_app("hold");
        let bar = seed_scrollbar(&mut app);
        app.sb_press(SbKey::List, &bar, SbHit::Tail);
        assert_eq!(app.scroll, 1, "按下立即步进一次");
        app.sb_hold_tick(Instant::now());
        assert_eq!(app.scroll, 1);
        let later = Instant::now() + std::time::Duration::from_millis(450);
        app.sb_hold_tick(later);
        assert_eq!(app.scroll, 2, "0.4s 起振后重复");
        app.sb_hold_tick(later + std::time::Duration::from_millis(60));
        assert_eq!(app.scroll, 2);
        app.sb_hold_tick(later + std::time::Duration::from_millis(130));
        assert_eq!(app.scroll, 3, "0.12s 周期重复");
        app.shutdown().await;
    }
}
