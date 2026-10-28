use std::time::{Duration, Instant};

use crate::app::App;
use crate::engine::Cmd;
use crate::model::speed::SpeedWindow;
use crate::model::{slots, TaskState};

const SPEED_TICK: Duration = Duration::from_secs(1);

impl App {
    pub async fn tick(&mut self) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_tick)
            .as_secs_f64()
            .clamp(0.001, 0.5);
        self.last_tick = now;
        self.frame = self.frame.wrapping_add(1);

        self.tick_consume_events().await;
        self.tick_grant_and_start().await;
        self.tick_retry_countdown(dt).await;
        self.tick_speed_and_session(now);
        self.tick_housekeeping(now);
        self.tick_clamp_selection(now);
    }

    async fn tick_consume_events(&mut self) {
        loop {
            match self.evt_rx.try_recv() {
                Ok(evt) => self.on_evt(evt).await,
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
            }
        }
    }

    async fn tick_grant_and_start(&mut self) {
        let granted = {
            let tasks = &mut self.tasks;
            slots::allocate(tasks, self.max_slots)
        };
        for id in granted {
            let Some(idx) = self.tasks.iter().position(|t| t.id == id) else {
                continue;
            };
            if !self.disk_precheck(idx) {
                continue;
            }
            self.tasks[idx].probed = false;
            let choice = self.tasks[idx].proxy.clone();
            let endpoint = self.resolve_endpoint(&choice);
            let spec = self.make_spec(&self.tasks[idx], endpoint);
            self.engine.send(Cmd::Start { spec }).await;
        }
    }

    async fn tick_retry_countdown(&mut self, dt: f64) {
        let mut expired: Vec<u32> = Vec::new();
        for t in &mut self.tasks {
            if t.state == TaskState::Failed {
                if let Some(left) = t.retry_in.as_mut() {
                    *left -= dt;
                    if *left <= 0.0 {
                        t.retry_in = None;
                        t.state = TaskState::Queued;
                        t.error = None;
                        expired.push(t.id);
                    }
                }
            }
        }
        for id in &expired {
            self.send_retry_start(*id).await;
        }
        let retry_starts: Vec<u32> = self
            .tasks
            .iter()
            .filter(|t| {
                t.state == TaskState::Queued && t.has_slot && t.probed && !expired.contains(&t.id)
            })
            .map(|t| t.id)
            .collect();
        for id in retry_starts {
            self.send_retry_start(id).await;
        }
    }

    fn tick_speed_and_session(&mut self, now: Instant) {
        for t in self.tasks.iter_mut() {
            if t.state != TaskState::Downloading {
                if let Some(d) = self.speed_display.get_mut(&t.id) {
                    d.zero();
                }
                t.speed = 0.0;
            }
        }
        let speed_due = now.duration_since(self.last_speed_tick) >= SPEED_TICK;
        if speed_due {
            self.last_speed_tick = now;
            for t in self.tasks.iter_mut() {
                if t.state != TaskState::Downloading {
                    continue;
                }
                let sample = self.windows.get_mut(&t.id).map(|w| w.rate()).unwrap_or(0.0);
                let d = self.speed_display.entry(t.id).or_default();
                d.push(sample);
                t.speed = d.value();
            }
            self.sample_conn_speed_display();
        }
        let global_dl = self.global_dl_speed();
        let mut gained: u64 = 0;
        for t in self.tasks.iter() {
            match self.session_seen.get(&t.id).copied() {
                None => {
                    self.session_seen.insert(t.id, t.downloaded);
                }
                Some(prev) if t.downloaded > prev => {
                    gained += t.downloaded - prev;
                    self.session_seen.insert(t.id, t.downloaded);
                }
                Some(_) => {}
            }
        }
        self.session_bytes += gained;
        if speed_due {
            self.push_hist(global_dl, 0.0);
        }
    }

    pub(super) fn sample_conn_speed_display(&mut self) {
        for t in &self.tasks {
            if t.state != TaskState::Downloading {
                for ((tid, _), d) in self.conn_speed.iter_mut() {
                    if *tid == t.id {
                        d.zero();
                    }
                }
                continue;
            }
            for c in &t.connections {
                let key = (t.id, c.id);
                let active = crate::model::conn_transferring(c.cap(), c.done);
                let d = self.conn_speed.entry(key).or_default();
                if !active {
                    d.zero();
                    continue;
                }
                let sample = self
                    .conn_windows
                    .get_mut(&key)
                    .map(SpeedWindow::rate)
                    .unwrap_or(0.0);
                d.push(sample);
            }
        }
    }

    fn tick_housekeeping(&mut self, now: Instant) {
        if let Some(u) = self.toast_until {
            if now > u {
                self.toast = None;
                self.toast_until = None;
            }
        }
        if self.last_save.elapsed().as_secs() >= 5 {
            self.save_registry();
        }
    }

    fn tick_clamp_selection(&mut self, now: Instant) {
        let flen = self.filtered().len();
        if flen == 0 {
            self.selected = 0;
            self.scroll = 0;
        } else {
            if self.selected >= flen {
                self.selected = flen - 1;
            }
            let selection_changed = self.last_selected != self.selected;
            let vis = self.visible_rows.max(1);
            if selection_changed {
                if self.scroll > self.selected {
                    self.scroll = self.selected;
                }
                if self.selected >= self.scroll + vis {
                    self.scroll = self.selected + 1 - vis;
                }
            }
            let max_scroll = flen.saturating_sub(vis);
            self.scroll = self.scroll.min(max_scroll);
        }
        self.last_selected = self.selected;
        if self.conns_scroll_for != self.selected {
            self.conns_scroll_for = self.selected;
            self.conns_scroll = 0;
            self.conns_sel = 0;
            self.conns_hscroll = 0;
        }
        let clen = self
            .sel_task()
            .filter(|t| t.state.shows_conns())
            .map_or(0, |t| t.connections.len());
        if self.conns_sel >= clen.max(1) {
            self.conns_sel = clen.saturating_sub(1);
        }
        let cvis = self.visible_conns_rows.max(1);
        self.conns_scroll = self.conns_scroll.min(clen.saturating_sub(cvis));
        self.conns_hscroll = self.conns_hscroll.min(self.conns_max_hscroll);
        self.sb_hold_tick(now);
    }

    async fn send_retry_start(&mut self, id: u32) {
        let Some(idx) = self.tasks.iter().position(|t| t.id == id) else {
            return;
        };
        let choice = self.tasks[idx].proxy.clone();
        let endpoint = self.resolve_endpoint(&choice);
        let spec = self.make_spec(&self.tasks[idx], endpoint);
        self.engine.send(Cmd::Start { spec }).await;
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.made_progress = false;
            t.probed = false;
        }
    }
}
