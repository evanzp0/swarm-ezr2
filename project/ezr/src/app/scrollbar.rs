//! scrollbar — 通用滚动条交互状态机（app 侧）。
//!
//! 自 `app/mod.rs` 收口（architect v151 模块边界裁决：App 核心——状态构造/
//! 持久化/UI 聚合访问器——与滚动条交互状态机是两个内聚单元，后者被
//! `keys`/`mouse`/`engine::tick` 三个输入通路消费，独立成模块后按文件
//! 变异加固与回归定位都以此为界）。类型经 `app` 根门面 re-export
//! （`crate::app::SbKey` 路径不变，ui 层消费零改动）。
//!
//! 语义基线（v1.32/FR-01-109 ⑥，对齐 ezr-demo m3 app.rs sb_* 方法族）：
//! 滑块拖拽反解、箭头步进 + 按压高亮 + 0.4s 起振/0.12s 周期连发、
//! 轨道整页翻页（朝点击方向）、松开收尾、按下滚动条同时聚焦所属面板。

use std::time::{Duration, Instant};

use super::{App, Panel};
use crate::scrollbar::{Arrow as SbArrow, Bar as SbBar, Hit as SbHit};

/// 滚动条实例标识（每实例一个；[`SbKey::bar`] 从 App 回填几何取本帧 [`SbBar`]）
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SbKey {
    /// 任务列表（垂直）
    List,
    /// 并发连接面板（垂直）
    ConnsV,
    /// 并发连接面板（水平，m3 新增）
    ConnsH,
    /// pieces 分块弹窗（垂直）
    Pieces,
    /// 帮助窗口（垂直）
    Help,
    /// 字段值弹窗（垂直，v1.34/FR-01-113）
    Val,
}

impl SbKey {
    /// 箭头单击/按住连发的步长（内容单位：任务条目/连接行/内容列/网格行/文本行；
    /// 水平以 2 列为单位——单列步进肉眼难辨，FR-01-109 ⑥）
    #[must_use]
    pub fn step(self) -> i32 {
        match self {
            SbKey::ConnsH => 2,
            _ => 1,
        }
    }

    /// 本实例当前帧的滚动条几何（ui 层每帧回填；None = 本帧未绘制）
    pub(crate) fn bar(self, app: &App) -> Option<SbBar> {
        match self {
            SbKey::List => app.list_scrollbar,
            SbKey::ConnsV => app.conns_v_scrollbar,
            SbKey::ConnsH => app.conns_h_scrollbar,
            SbKey::Pieces => app.pieces_scrollbar,
            SbKey::Help => app.help_scrollbar,
            SbKey::Val => app.val_scrollbar,
        }
    }
}

/// 滚动条箭头/轨道按住连发状态（Down 记录、Up 取消、tick 驱动重复）
#[derive(Clone, Copy, Debug)]
pub struct SbHold {
    /// 哪个滚动条实例
    pub key: SbKey,
    /// 方向：-1 = 朝头箭头（↑/←），+1 = 朝尾箭头（↓/→）
    pub dir: i32,
    /// true = 轨道翻页（按 page 步长），false = 箭头步进（按 step 步长）
    pub page: bool,
    /// 按下时刻（起振延迟基准）
    pub since: Instant,
    /// 上次重复时刻
    pub last: Instant,
}

impl SbHold {
    /// 首次重复的延迟（按住起振时间，FR-01-109 ⑥：0.4s）
    const FIRST_DELAY: Duration = Duration::from_millis(400);
    /// 后续重复周期（0.12s）
    const REPEAT: Duration = Duration::from_millis(120);

    /// 下次重复到期时刻（首次按起振延迟，之后按周期）
    fn due(&self) -> Instant {
        if self.since == self.last {
            self.since + Self::FIRST_DELAY
        } else {
            self.last + Self::REPEAT
        }
    }
}

impl App {
    // -------------------------------------------------------------------
    // 通用滚动条交互（v1.32/FR-01-109 ⑥：滑块拖拽/箭头步进连发/轨道翻页/
    // 按压高亮余辉；对齐 ezr-demo m3 app.rs sb_* 方法族）
    // -------------------------------------------------------------------

    /// 滚动条滚动上限（按实例取各自的 total − page；ConnsH = 横向内容差）
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
        }
    }

    /// 滚动偏移写入（按实例写回各自的滚动字段；事件内立即钳制 0..=max_pos，
    /// 末页即停——FR-01-109 ⑥）
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
        }
    }

    /// 滚动偏移绝对写入（滑块拖拽反解结果；钳制到 0..=max_pos）
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
        }
    }

    /// 轨道翻页页长（本实例一屏可视单位数；几何缺失时退化为 1）
    fn sb_page(&self, key: SbKey) -> i32 {
        key.bar(self).map_or(1, |b| b.page.max(1) as i32)
    }

    /// 滚动条 Down 事件分派（FR-01-109 ⑥）：滑块 = 开始拖拽；箭头 = 立即
    /// 步进一次 + 按压高亮 + 400ms 余辉 + 记连发；轨道 = 朝点击方向整页滚动
    /// + 记连发。同时聚焦所属面板（主界面实例）。
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
                // 朝点击方向整页滚动：点击滑块上方/左侧 = 向头翻页，反之为向尾
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

    /// 滑块拖拽随动（Drag 事件）：指针在轨道内的下标 − 抓取偏移 = 滑块新位，
    /// 反解回滚动偏移（与正向显示取整严格互逆，FR-01-109 ④）
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

    /// 松开左键：结束拖拽/连发/按压高亮（任何位置松开都调用）
    pub(super) fn sb_release(&mut self) {
        self.sb_drag = None;
        self.sb_hold = None;
        self.sb_pressed = None;
    }

    /// tick 驱动的按住连发（FR-01-109 ⑥：0.4s 起振、0.12s 周期，松开即停）
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

    /// 渲染查询：箭头按压高亮态（按压中优先；否则 400ms 余辉内可见——
    /// 快速单击也有可见点击效果，FR-01-109 ⑥）
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

    /// 按住箭头期间的拖出/拖回高亮跟随：指针在按住的箭头上 = 恢复高亮，
    /// 拖出 = 取消高亮（连发不中断，FR-01-109 ⑥）
    pub(crate) fn sb_arrow_follow(&mut self, col: u16, row: u16) {
        let Some(h) = self.sb_hold else {
            return;
        };
        if h.page {
            return; // 轨道翻页无箭头高亮
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

    /// 主界面滚动条 Down 命中分派（FR-01-109 ⑥：按下滚动条同时聚焦所属面板）。
    /// 依实例序检查命中（列表 → 并发垂直 → 并发水平）；命中即消费。
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
    //! v1.32/FR-01-109 通用滚动条交互状态机单测：箭头步进+按压高亮+连发、
    //! 轨道翻页方向、滑块拖拽反解、松开收尾（随状态机自 app/mod.rs 迁移，
    //! 语义不变；←→ 横向钳制与 About 模态等应用层交互测试留在 app/mod.rs）。

    use ratatui::layout::Rect;

    use super::*;
    use crate::model::config::Config;
    use crate::model::TaskState;

    fn make_app(tag: &str) -> App {
        let dir = crate::model::testenv::uniq_tmp_dir(&format!("ezr-sb-{tag}"));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        App::new(Config::default(), reg)
    }

    fn dl_task(id: u32, name: &str) -> crate::model::Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t.state = TaskState::Downloading;
        t.probed = true;
        t.total = 5_000_000;
        t
    }

    /// 40×20 面板的垂直滚动条几何（total=30、page=5 → max 25，与下方
    /// 测试播种的 30 个任务一致——回填几何与真实滚动上限同源，模拟 ui 层
    /// 每帧回填的真实契约）
    fn vbar() -> SbBar {
        SbBar::vertical(Rect::new(0, 0, 40, 20), 30, 5, 0)
    }

    /// 以当前 scroll 重建回填几何（模拟下一帧 ui 层回填：pos 字段随帧刷新）
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
        app.list_scrollbar = Some(vbar());
        app.visible_rows = 5;
        for _ in 0..30 {
            let mut t = dl_task(1, "t.bin");
            t.state = TaskState::Queued;
            app.tasks.push(t);
        }
        app.focus = Panel::Detail;
        // 尾箭头命中（几何：高 20 → 轨道 [2,17]；Tail 在行 18）
        let bar = app.list_scrollbar.unwrap();
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
        // 主界面按下滚动条同时聚焦所属面板（sb_press_at 入口语义）
        app.focus = Panel::Detail;
        assert!(app.sb_press_at(39, 18), "尾箭头再次命中");
        assert_eq!(app.focus, Panel::List, "按下滚动条聚焦所属面板");
        // 渲染查询：按压态可见
        assert_eq!(app.sb_arrow_pressed(SbKey::List), Some(SbArrow::Tail));
        // 头箭头：scroll 回 0 后再按被钳 0
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
        app.list_scrollbar = Some(vbar());
        app.visible_rows = 5;
        for _ in 0..30 {
            let mut t = dl_task(1, "t.bin");
            t.state = TaskState::Queued;
            app.tasks.push(t);
        }
        let bar = app.list_scrollbar.unwrap();
        // 首屏（pos 0）：滑块在轨道头部 → 轨道下半段点击 = 朝尾翻页
        let hit = bar.hit(39, 10);
        assert!(matches!(hit, Some(SbHit::Track(_))));
        app.sb_press(SbKey::List, &bar, hit.unwrap());
        assert_eq!(app.scroll, 5, "整页翻页（page = 可视 5）");
        assert!(app.sb_hold.as_ref().is_some_and(|h| h.page), "轨道连发按页");
        // 滑块拖到末页后（下一帧回填 pos）：轨道上半段点击 = 朝头翻页
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
        app.list_scrollbar = Some(vbar());
        app.visible_rows = 5;
        for _ in 0..30 {
            let mut t = dl_task(1, "t.bin");
            t.state = TaskState::Queued;
            app.tasks.push(t);
        }
        let bar = app.list_scrollbar.unwrap();
        // 滑块命中（首屏滑块在轨道首格：行 2）→ 抓取偏移 0
        let hit = bar.hit(39, 2);
        assert!(matches!(hit, Some(SbHit::Thumb(0))));
        app.sb_press(SbKey::List, &bar, hit.unwrap());
        assert_eq!(app.sb_drag, Some((SbKey::List, 0)));
        // 拖到轨道中部（行 10 → 轨道下标 8）→ 反解滚动偏移
        app.sb_drag_move(39, 10);
        let expected = bar.pos_from_thumb(8);
        assert_eq!(app.scroll, expected, "拖拽反解与显示取整互逆");
        // 松开 = 结束拖拽/连发/按压
        app.sb_release();
        assert!(app.sb_drag.is_none() && app.sb_hold.is_none() && app.sb_pressed.is_none());
        app.shutdown().await;
    }

    #[tokio::test]
    async fn hold_autorepeat_first_delay_then_period() {
        let mut app = make_app("hold");
        app.list_scrollbar = Some(vbar());
        app.visible_rows = 5;
        for _ in 0..30 {
            let mut t = dl_task(1, "t.bin");
            t.state = TaskState::Queued;
            app.tasks.push(t);
        }
        let bar = app.list_scrollbar.unwrap();
        app.sb_press(SbKey::List, &bar, SbHit::Tail);
        assert_eq!(app.scroll, 1, "按下立即步进一次");
        // 起振前（<400ms）tick 不重复
        app.sb_hold_tick(Instant::now());
        assert_eq!(app.scroll, 1);
        // 起振（>400ms）→ 重复一次
        let later = Instant::now() + std::time::Duration::from_millis(450);
        app.sb_hold_tick(later);
        assert_eq!(app.scroll, 2, "0.4s 起振后重复");
        // 周期（0.12s 内）不重复
        app.sb_hold_tick(later + std::time::Duration::from_millis(60));
        assert_eq!(app.scroll, 2);
        // 周期到期 → 再重复
        app.sb_hold_tick(later + std::time::Duration::from_millis(130));
        assert_eq!(app.scroll, 3, "0.12s 周期重复");
        app.shutdown().await;
    }
}
