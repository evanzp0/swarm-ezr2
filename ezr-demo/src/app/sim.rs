use std::time::{Duration, Instant};

use super::*;

const SPEED_TICK: Duration = Duration::from_secs(1);

const SPEED_EMA_ALPHA: f64 = 0.2;
impl App {
    pub fn tick(&mut self) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_tick)
            .as_secs_f64()
            .clamp(0.001, 0.5);
        self.last_tick = now;
        self.frame = self.frame.wrapping_add(1);

        self.enforce_slots();

        self.bw_t += dt;
        let bw = 1.0
            + 0.11 * (std::f64::consts::TAU * self.bw_t / 6.5).sin()
            + 0.05 * (std::f64::consts::TAU * self.bw_t / 2.1 + 1.7).sin()
            + 0.03 * (std::f64::consts::TAU * self.bw_t / 1.1 + 4.0).sin();

        let rng = &mut self.rng;
        let mut notices: Vec<String> = Vec::new();

        let mut frame_dl_bytes: u64 = 0u64;

        for t in self.tasks.iter_mut() {
            match t.state {
                TaskState::Downloading => {
                    let stalled = t.flaky && t.fail_count > 1;

                    let task_inst = if stalled || t.connections.is_empty() {
                        0.0
                    } else {
                        t.base_speed * bw * rng.range(0.92, 1.08)
                    };
                    let n = t.connections.len().max(1) as f64;
                    for c in t.connections.iter_mut() {
                        if c.phase == ConnPhase::Connecting {
                            c.phase_in -= dt;
                            if c.phase_in <= 0.0 {
                                c.phase = ConnPhase::Connected;
                                c.phase_in = 0.0;
                            }
                            continue;
                        }
                        if c.cap() > 0 && c.done < c.cap() {
                            let rate = (task_inst / n) * rng.range(0.88, 1.12);
                            let nd = ((c.done as f64 + rate * dt).min(c.cap() as f64) as u64)
                                .saturating_sub(c.done);
                            c.win += nd;
                            c.cum_down = c.cum_down.saturating_add(nd);
                            c.done += nd;
                            frame_dl_bytes += nd;
                        }
                    }

                    let piece = chunk_size(t.protocol);
                    let y = chunk_total(t.total, piece) as u64;
                    let k = t
                        .connections
                        .iter()
                        .filter(|c| c.cap() > 0 && c.done >= c.cap())
                        .count() as u32;
                    if k > 0 {
                        let l = t.connections.iter().filter(|c| c.cap() > 0).count() as u32;
                        let mut next = (t.chunk_done + l) as u64;
                        for c in t.connections.iter_mut() {
                            if c.cap() > 0 && c.done >= c.cap() {
                                if next < y {
                                    c.start = next * piece;
                                    c.end = if next + 1 == y {
                                        t.total
                                    } else {
                                        c.start + piece
                                    };
                                    c.done = 0;

                                    next += 1;
                                } else {
                                    c.start = 0;
                                    c.end = 0;
                                    c.done = 0;

                                    c.phase = ConnPhase::Closed;
                                    c.phase_in = 0.0;
                                }
                            }
                        }
                        t.chunk_done += k;
                    }

                    let inflight: u64 = t
                        .connections
                        .iter()
                        .filter(|c| c.cap() > 0 && c.done < c.cap())
                        .map(|c| c.done)
                        .sum();
                    let mut agg = t.chunk_done as u64 * piece + inflight;
                    if t.chunk_done as u64 >= y {
                        agg = t.total;
                    }
                    t.downloaded = agg.min(t.total);

                    if !stalled {
                        t.made_progress = true;
                    }

                    if t.protocol.is_bt() {
                        let up_inst = if stalled || t.connections.is_empty() {
                            0.0
                        } else {
                            420_000.0 * rng.range(0.5, 1.7)
                        };
                        let un = t.connections.len().max(1) as f64;
                        for c in t.connections.iter_mut() {
                            if c.cap() > 0 && c.done < c.cap() {
                                let rate = (up_inst / un) * rng.range(0.88, 1.12);
                                let nb = (rate * dt) as u64;
                                c.win_up += nb;
                                c.cum_up = c.cum_up.saturating_add(nb);
                            }
                        }
                        t.uploaded = t.uploaded.saturating_add((up_inst * dt) as u64);
                    }

                    if t.flaky {
                        let fail_after = if t.fail_count > 1 { 2.0 } else { 6.0 };
                        let mut re_fail = false;
                        if let Some(rs) = t.running_since {
                            if now.duration_since(rs) > Duration::from_secs_f64(fail_after) {
                                re_fail = true;
                            }
                        }
                        if re_fail {
                            t.state = TaskState::Failed;
                            t.running_since = None;
                            t.speed = 0.0;
                            t.upload_speed = 0.0;
                            t.fail_count += 1;
                            let (msg, kind, retry_after) = fail_case(t.fail_count);
                            t.error = Some(msg.to_string());
                            t.fail_kind = Some(kind);

                            let reset = t.made_progress;
                            t.retries = if reset { 1 } else { t.retries + 1 };
                            t.made_progress = false;
                            let rule = if reset {
                                format!(
                                    "非连续失败，重试次数重置为 {}/{}",
                                    t.retries, t.max_retries
                                )
                            } else {
                                format!("连续失败，重试次数累加至 {}/{}", t.retries, t.max_retries)
                            };

                            let secs = retry_after.unwrap_or_else(|| backoff_secs(t.retries));
                            let auto_kind =
                                matches!(kind, FailKind::Transient | FailKind::Invalidated);
                            if auto_kind && t.retries < t.max_retries {
                                t.retry_in = Some(secs);
                                t.has_slot = true;
                                if kind == FailKind::Invalidated {
                                    let total = t.total;
                                    let n = t.connections.len().max(1);
                                    let piece = chunk_size(t.protocol);
                                    t.downloaded = 0;
                                    t.chunk_done = 0;
                                    t.connections =
                                        make_chunk_conns(total, 0, n, piece, t.id as u64).0;
                                    notices.push(format!(
                                        "⚠ 服务器内容已更新，断点已作废，{}s 后从头重新下载: {}",
                                        secs as u64, t.name
                                    ));
                                } else {
                                    notices.push(format!(
                                        "✘ 下载失败（{}），{}s 后自动重试: {}",
                                        rule, secs as u64, t.name
                                    ));
                                }
                            } else if auto_kind {
                                t.retry_in = None;
                                t.has_slot = false;
                                notices.push(format!(
                                    "✘ 已达最大重试次数（{}/{}），停止自动重试，按 R 手动重试: {}",
                                    t.retries, t.max_retries, t.name
                                ));
                            } else {
                                t.retry_in = None;
                                t.has_slot = false;
                                notices.push(format!(
                                    "✘ 下载失败（{}）：不自动重试，按 R 手动重试: {}",
                                    msg, t.name
                                ));
                            }
                            continue;
                        }
                    }
                    if t.downloaded >= t.total {
                        t.speed = 0.0;
                        t.upload_speed = 0.0;
                        t.running_since = None;
                        t.has_slot = false;

                        let algo = t.checksum.as_ref().map(|c| c.algo);
                        if let Some(algo) = algo {
                            t.state = TaskState::Verifying;
                            t.verify_started = Some(now);
                            notices.push(format!("✓ 下载完成，开始{}校验: {}", algo, t.name));
                        } else if t.post_process {
                            t.state = TaskState::PostProcessing;
                            t.post_started = Some(now);
                            notices.push(format!("✓ 下载完成，开始后期处理: {}", t.name));
                        } else {
                            t.state = TaskState::Completed;

                            t.connections.clear();
                            notices.push(format!("✓ 下载完成: {}", t.name));
                        }
                    }
                }
                TaskState::Verifying => {
                    if let Some(s) = t.verify_started {
                        if now.duration_since(s) > Duration::from_millis(2600) {
                            let algo = t.checksum.as_ref().map(|c| c.algo).unwrap_or("SHA-256");
                            t.verify_started = None;
                            t.speed = 0.0;
                            t.upload_speed = 0.0;

                            if t.verify_fail_once {
                                t.verify_fail_once = false;
                                t.verify_ok = Some(false);
                                t.state = TaskState::Failed;
                                t.fail_kind = Some(FailKind::Verify);
                                t.retry_in = None;
                                t.has_slot = false;
                                t.error = Some(format!("{} 校验失败：内容与校验值不符", algo));
                                notices.push(format!(
                                    "✘ {} 校验失败：不自动重试，按 R 重新校验: {}",
                                    algo, t.name
                                ));
                            } else {
                                t.verify_ok = Some(true);
                                if t.post_process {
                                    t.state = TaskState::PostProcessing;
                                    t.post_started = Some(now);
                                    notices.push(format!(
                                        "✓ {} 校验通过，开始后期处理: {}",
                                        algo, t.name
                                    ));
                                } else {
                                    t.state = TaskState::Completed;

                                    t.connections.clear();
                                    notices.push(format!("✓ {} 校验通过: {}", algo, t.name));
                                }
                            }
                        }
                    }
                }
                TaskState::PostProcessing => {
                    if let Some(s) = t.post_started {
                        if now.duration_since(s) > Duration::from_millis(3400) {
                            t.state = TaskState::Completed;
                            t.post_started = None;
                            t.speed = 0.0;
                            t.upload_speed = 0.0;

                            t.connections.clear();
                            notices.push(format!("✓ 后期处理完成: {}", t.name));
                        }
                    }
                }
                TaskState::Seeding => {
                    let up_inst = 1_900_000.0 * rng.range(0.6, 1.5);
                    let un = t.connections.iter().filter(|c| c.cap() > 0).count().max(1) as f64;
                    for c in t.connections.iter_mut() {
                        if c.cap() > 0 {
                            let rate = (up_inst / un) * rng.range(0.88, 1.12);
                            let nb = (rate * dt) as u64;
                            c.win_up += nb;
                            c.cum_up = c.cum_up.saturating_add(nb);
                        }
                    }
                    t.uploaded = t.uploaded.saturating_add((up_inst * dt) as u64);
                    t.seed_left = (t.seed_left - dt).max(0.0);
                    if t.seed_left <= 0.0 {
                        t.state = TaskState::Completed;
                        t.upload_speed = 0.0;

                        t.connections.clear();
                        notices.push(format!("✓ 做种结束: {}", t.name));
                    }
                }
                TaskState::Queued => {
                    if !t.has_slot {
                        continue;
                    }
                    if let Some(q) = t.queued_since {
                        if now.duration_since(q) > Duration::from_secs_f64(t.start_delay.max(0.5)) {
                            if t.precheck_fail_once {
                                t.precheck_fail_once = false;
                                t.state = TaskState::Failed;
                                t.fail_kind = Some(FailKind::Fatal);
                                t.retry_in = None;
                                t.has_slot = false;
                                t.queued_since = None;
                                t.error = Some(
                                    "磁盘空间不足（保存目录剩余空间小于待下载量）".to_string(),
                                );
                                notices.push(format!(
                                    "✘ 磁盘空间不足：开始前预检失败，不自动重试，按 R 手动重试: {}",
                                    t.name
                                ));
                                continue;
                            }

                            if !t.resumable {
                                let total = t.total;
                                let n = t.connections.len().max(1);
                                let piece = chunk_size(t.protocol);
                                t.downloaded = 0;
                                t.chunk_done = 0;
                                t.connections = make_chunk_conns(total, 0, n, piece, t.id as u64).0;
                            }
                            t.state = TaskState::Downloading;
                            t.queued_since = None;
                            t.running_since = Some(now);
                            t.made_progress = false;

                            for c in t.connections.iter_mut() {
                                c.cum_down = 0;
                                c.cum_up = 0;
                                c.up_speed = 0.0;
                            }

                            for (i, c) in t.connections.iter_mut().enumerate() {
                                c.phase = ConnPhase::Connecting;
                                c.phase_in = (0.2 + 0.1 * i as f64).min(1.5);
                            }
                            let name = t.name.clone();
                            if t.fail_count > 0 {
                                notices.push(format!(
                                    "↻ 自动重试开始（{}/{}）: {}",
                                    t.retries, t.max_retries, name
                                ));
                            } else {
                                notices.push(format!("▶ 槽位空闲，开始下载: {}", name));
                            }
                        }
                    }
                }
                TaskState::Failed => {
                    if let Some(left) = t.retry_in.as_mut() {
                        *left -= dt;
                        if *left <= 0.0 {
                            t.retry_in = None;

                            t.state = TaskState::Queued;
                            t.error = None;
                            t.queued_since = Some(now);
                            t.start_delay = 3.0;
                            notices.push(format!(
                                "↻ 自动重试（{}/{}）→ 等待中: {}",
                                t.retries, t.max_retries, t.name
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        let speed_due = now.duration_since(self.last_speed_tick) >= SPEED_TICK;
        if speed_due {
            self.last_speed_tick = now;
        }
        for t in self.tasks.iter_mut() {
            match t.state {
                TaskState::Downloading => {
                    if !speed_due {
                        continue;
                    }
                    if t.connections.is_empty() {
                        t.speed = 0.0;
                        continue;
                    }

                    let win: u64 = t.connections.iter().map(|c| c.win).sum();
                    let sample = win as f64 / SPEED_TICK.as_secs_f64();
                    t.speed += (sample - t.speed) * SPEED_EMA_ALPHA;
                    for c in t.connections.iter_mut() {
                        if c.cap() == 0 || c.done >= c.cap() {
                            c.speed = 0.0;
                        } else {
                            let s = c.win as f64 / SPEED_TICK.as_secs_f64();
                            c.speed += (s - c.speed) * SPEED_EMA_ALPHA;
                        }
                    }

                    if t.protocol.is_bt() {
                        let win_up: u64 = t.connections.iter().map(|c| c.win_up).sum();
                        let up = win_up as f64 / SPEED_TICK.as_secs_f64();
                        t.upload_speed += (up - t.upload_speed) * SPEED_EMA_ALPHA;
                        for c in t.connections.iter_mut() {
                            if c.cap() == 0 || c.done >= c.cap() {
                                c.up_speed = 0.0;
                            } else {
                                let s = c.win_up as f64 / SPEED_TICK.as_secs_f64();
                                c.up_speed += (s - c.up_speed) * SPEED_EMA_ALPHA;
                            }
                        }
                    }

                    for c in t.connections.iter_mut() {
                        c.win = 0;
                        c.win_up = 0;
                    }
                }
                TaskState::Seeding => {
                    if !speed_due {
                        continue;
                    }
                    let win_up: u64 = t.connections.iter().map(|c| c.win_up).sum();
                    let up = win_up as f64 / SPEED_TICK.as_secs_f64();
                    t.upload_speed += (up - t.upload_speed) * SPEED_EMA_ALPHA;
                    for c in t.connections.iter_mut() {
                        c.speed = 0.0;
                        if c.cap() == 0 {
                            c.up_speed = 0.0;
                        } else {
                            let s = c.win_up as f64 / SPEED_TICK.as_secs_f64();
                            c.up_speed += (s - c.up_speed) * SPEED_EMA_ALPHA;
                        }
                        c.win = 0;
                        c.win_up = 0;
                    }
                }
                _ => {
                    t.speed = 0.0;
                    t.upload_speed = 0.0;
                    for c in t.connections.iter_mut() {
                        c.speed = 0.0;
                        c.up_speed = 0.0;
                    }
                }
            }
        }

        self.session_bytes = self.session_bytes.saturating_add(frame_dl_bytes);
        if speed_due {
            let global_dl: f64 = self
                .tasks
                .iter()
                .filter(|t| t.state == TaskState::Downloading)
                .map(|t| t.speed)
                .sum();
            let global_ul: f64 = self
                .tasks
                .iter()
                .filter(|t| t.upload_speed > 0.0)
                .map(|t| t.upload_speed)
                .sum();
            self.speed_hist.push((global_dl / 1024.0) as u64);
            if self.speed_hist.len() > 180 {
                self.speed_hist.remove(0);
            }
            self.up_hist.push((global_ul / 1024.0) as u64);
            if self.up_hist.len() > 180 {
                self.up_hist.remove(0);
            }
        }

        if let Some(last) = notices.pop() {
            self.set_toast(last);
        }
        if let Some(u) = self.toast_until {
            if now > u {
                self.toast = None;
                self.toast_until = None;
            }
        }

        let flen = self.filtered().len();
        if flen == 0 {
            self.selected = 0;
            self.scroll = 0;
        } else {
            if self.selected >= flen {
                self.selected = flen - 1;
            }

            if self.last_selected != self.selected {
                self.scroll_to_selection();
            }
            let max_scroll = flen.saturating_sub(self.visible_rows.max(1));
            self.scroll = self.scroll.min(max_scroll);
        }
        self.last_selected = self.selected;

        if self.conns_scroll_for != self.selected {
            self.conns_scroll_for = self.selected;
            self.conns_scroll = 0;
            self.conns_sel = 0;
            self.conns_hscroll = 0;
        }

        let conns_visible = self
            .sel_task()
            .map(|t| t.state.shows_conns())
            .unwrap_or(false);
        let clen = if conns_visible {
            self.active_conn_map().len()
        } else {
            0
        };
        let cvis = self.visible_conns_rows.max(1);
        let cmax = clen.saturating_sub(cvis);
        self.conns_scroll = self.conns_scroll.min(cmax);

        self.conns_hscroll = self.conns_hscroll.min(self.conns_max_hscroll);
        if self.conns_sel >= clen.max(1) {
            self.conns_sel = clen.saturating_sub(1);
        }

        self.sb_hold_tick(now);
    }
}
