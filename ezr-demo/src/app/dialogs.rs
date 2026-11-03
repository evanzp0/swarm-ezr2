use std::collections::HashSet;
use std::time::Instant;

use crate::multiline::MultiLineBuf;
use crate::textfield::TextInput;
use crate::ui::{fmt_size, truncate};

use super::demo::mk;
use super::model::ADD_POOL;
use super::*;

/// BT 任务默认 trackers（新增未填时采用；修改面板预填同源；4 行，5 可视行内全可见；行超宽可水平滚动）
fn demo_bt_trackers() -> Vec<String> {
    vec![
        "udp://tracker.ezr.dev:6969/announce".to_string(),
        "udp://tracker.opentorrent.example:1337/announce".to_string(),
        "https://tracker.ezr.dev/announce".to_string(),
        "http://tracker.ezr.dev:8080/announce".to_string(),
    ]
}

fn tracker_reject(lines: &[String]) -> Option<String> {
    lines
        .iter()
        .find(|l| {
            let s = l.to_lowercase();
            !(s.starts_with("http://") || s.starts_with("https://") || s.starts_with("udp://"))
        })
        .cloned()
}

pub(super) fn parse_speed_limit(raw: &str) -> Option<u64> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let split = s
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.'))
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    if num.is_empty() || num.matches('.').count() > 1 {
        return None;
    }
    let n: f64 = num.parse().ok()?;
    if !n.is_finite() || n < 0.0 {
        return None;
    }
    let unit = unit.trim().trim_end_matches("/s").trim().to_lowercase();
    let mult = super::dialog::speed_unit_mult(&unit)?;
    let v = n * mult;
    if v >= u64::MAX as f64 {
        return Some(u64::MAX);
    }
    Some(v.round() as u64)
}

pub(super) fn format_speed_limit(v: u64) -> String {
    if v == 0 {
        return "0".to_string();
    }
    if v % 1_073_741_824 == 0 {
        return format!("{} GiB/s", v / 1_073_741_824);
    }
    if v % 1_048_576 == 0 {
        return format!("{} MiB/s", v / 1_048_576);
    }
    if v % 1024 == 0 {
        return format!("{} KiB/s", v / 1024);
    }
    format!("{} B/s", v)
}

impl App {
    pub(super) fn add_dialog_enter(&mut self, focus: usize) {
        if focus == 4 {
            let d = self.dialog.as_mut().unwrap();
            d.ck_open = true;
            d.ck_sel = d.ck_type;
        } else if focus == 6 {
            let d = self.dialog.as_mut().unwrap();
            d.proxy_open = true;
        } else if focus == 8 {
            self.dlg_add_only();
        } else if focus == 9 {
            self.dialog = None;
        } else {
            self.dlg_confirm_add(false);
        }
    }

    fn dlg_add_only(&mut self) {
        self.dlg_confirm_add(true);
    }

    pub(super) fn modify_dialog_enter(&mut self, focus: usize) {
        if focus == 2 {
            let d = self.dialog.as_mut().unwrap();
            d.ck_open = true;
            d.ck_sel = d.ck_type;
        } else if focus == 4 {
            let d = self.dialog.as_mut().unwrap();
            d.proxy_open = true;
        } else if focus == 6 {
            self.dialog = None;
        } else {
            self.dlg_confirm_modify();
        }
    }

    fn limit_char_slot(kind: DialogKind, focus: usize, c: char) -> Option<usize> {
        let valid =
            !c.is_control() && (c.is_ascii_alphanumeric() || c == ' ' || c == '/' || c == '.');
        let slot = match (kind, focus) {
            (DialogKind::Add, 3) => 0,
            (DialogKind::Modify, 1) => 0,
            (DialogKind::AddBt, 4) => 0,
            (DialogKind::ModifyBt, 2) => 0,
            (DialogKind::AddBt, 5) => 1,
            (DialogKind::ModifyBt, 3) => 1,
            _ => return None,
        };
        valid.then_some(slot)
    }

    pub(super) fn add_dialog_char(&mut self, focus: usize, c: char) {
        if focus < 3 {
            if !c.is_control() {
                let d = self.dialog.as_mut().unwrap();
                match focus {
                    0 => {
                        if d.url.insert(&c.to_string(), 300) > 0 {
                            if !d.conns_edited {
                                let (p, _) = parse_url_task(d.url.value.trim());
                                d.conns.set(default_conns(p).to_string());
                            }
                        }
                    }
                    1 => {
                        d.dir.insert(&c.to_string(), 300);
                    }
                    _ => {
                        super::dialog::insert_conns_digit(d, c);
                    }
                }
            }
        } else if let Some(slot) = Self::limit_char_slot(DialogKind::Add, focus, c) {
            let d = self.dialog.as_mut().unwrap();
            if slot == 0 {
                super::dialog::insert_limit_char(&mut d.down_limit, c);
            } else {
                super::dialog::insert_limit_char(&mut d.up_limit, c);
            }
        } else if focus == 4 {
            if c == ' ' {
                let d = self.dialog.as_mut().unwrap();
                d.ck_open = true;
                d.ck_sel = d.ck_type;
            }
        } else if focus == 5 {
            if self.dialog.as_ref().unwrap().ck_type != CK_NONE && c.is_ascii_hexdigit() {
                self.dialog
                    .as_mut()
                    .unwrap()
                    .ck_value
                    .insert(&c.to_string(), 128);
            }
        } else if c == ' ' {
            if focus == 6 {
                let d = self.dialog.as_mut().unwrap();
                d.proxy_open = true;
            } else {
                self.dlg_activate_add(focus);
            }
        }
    }

    pub(super) fn modify_dialog_char(&mut self, focus: usize, c: char) {
        if focus == 0 {
            let d = self.dialog.as_mut().unwrap();
            super::dialog::insert_conns_digit(d, c);
        } else if let Some(slot) = Self::limit_char_slot(DialogKind::Modify, focus, c) {
            let d = self.dialog.as_mut().unwrap();
            if slot == 0 {
                super::dialog::insert_limit_char(&mut d.down_limit, c);
            } else {
                super::dialog::insert_limit_char(&mut d.up_limit, c);
            }
        } else if focus == 2 {
            if c == ' ' {
                let d = self.dialog.as_mut().unwrap();
                d.ck_open = true;
                d.ck_sel = d.ck_type;
            }
        } else if focus == 3 {
            if self.dialog.as_ref().unwrap().ck_type != CK_NONE && c.is_ascii_hexdigit() {
                self.dialog
                    .as_mut()
                    .unwrap()
                    .ck_value
                    .insert(&c.to_string(), 128);
            }
        } else if c == ' ' {
            if focus == 4 {
                let d = self.dialog.as_mut().unwrap();
                d.proxy_open = true;
            } else {
                self.dlg_activate_modify(focus);
            }
        }
    }

    pub(super) fn dlg_activate_add(&mut self, btn: usize) {
        match btn {
            7 => self.dlg_confirm_add(false),
            8 => self.dlg_add_only(),
            9 => self.dialog = None,
            _ => {}
        }
    }

    pub(super) fn dlg_activate_modify(&mut self, btn: usize) {
        match btn {
            5 => self.dlg_confirm_modify(),
            6 => self.dialog = None,
            _ => {}
        }
    }

    pub(super) fn dlg_confirm_modify(&mut self) {
        let Some(d) = self.dialog.as_ref() else {
            return;
        };
        let Some(id) = d.task_id else {
            return;
        };
        let Some(idx) = self.tasks.iter().position(|t| t.id == id) else {
            self.dialog = None;
            return;
        };
        let conns_raw = d.conns.value.trim().to_string();
        let down_raw = d.down_limit.value.trim().to_string();
        let ck_raw = d.ck_value.value.trim().to_string();
        let ck_type = d.ck_type;
        let proxy_sel = d.proxy_sel.min(self.proxy_options.len().saturating_sub(1));
        let proxy = self.proxy_options[proxy_sel].clone();

        let concurrency = if conns_raw.is_empty() {
            self.tasks[idx].concurrency
        } else {
            conns_raw.parse::<usize>().unwrap_or(0).clamp(1, 64)
        };

        let checksum = if ck_type == CK_NONE {
            None
        } else if ck_raw.is_empty() {
            let (algo, need) = CHECKSUM_ALGOS[ck_type.min(CHECKSUM_ALGOS.len() - 1)];
            self.set_toast(format!(
                "⚠ {} 校验码不可为空（需 {} 位十六进制）",
                algo, need
            ));
            if let Some(d) = self.dialog.as_mut() {
                d.focus_field(3);
            }
            return;
        } else {
            let (algo, need) = CHECKSUM_ALGOS[ck_type.min(CHECKSUM_ALGOS.len() - 1)];
            let v = ck_raw.to_lowercase();
            if !v.chars().all(|c| c.is_ascii_hexdigit()) {
                self.set_toast(format!("⚠ {algo} 校验码只能是十六进制字符（0-9a-f）"));
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = 3;
                }
                return;
            }
            let len = v.chars().count();
            if len != need {
                self.set_toast(format!(
                    "⚠ {algo} 校验码需为 {need} 位十六进制（当前 {len} 位）"
                ));
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = 3;
                }
                return;
            }
            Some(Checksum { algo, value: v })
        };

        {
            let t = &mut self.tasks[idx];
            let down = if down_raw.is_empty() {
                TASK_LIMIT_DEFAULTS.0
            } else {
                parse_speed_limit(&down_raw).unwrap_or(t.task_max_down)
            };
            t.task_max_down = down;
            let changed = t.concurrency != concurrency;
            t.concurrency = concurrency;
            t.checksum = checksum;
            t.proxy = proxy;
            if changed {
                let old_speeds: Vec<f64> = t.connections.iter().map(|c| c.speed).collect();
                let piece = chunk_size(t.protocol);

                let mut done_blocks = t.chunk_done as u64;
                let mut rem: u64 = 0;
                for c in t.connections.iter() {
                    if c.cap() == 0 {
                        continue;
                    }
                    if c.done >= c.cap() {
                        done_blocks += 1;
                    } else {
                        rem += c.done;
                    }
                }
                let y = chunk_total(t.total, piece) as u64;
                let done_blocks = done_blocks.min(y);
                let (mut conns_new, x) = make_chunk_conns_lease(
                    t.total,
                    done_blocks as u32,
                    rem,
                    concurrency,
                    piece,
                    t.id as u64,
                );
                for (i, c) in conns_new.iter_mut().enumerate() {
                    if let Some(s) = old_speeds.get(i) {
                        c.speed = *s;
                    }
                }
                t.connections = conns_new;
                t.chunk_done = x;

                let rem_new: u64 = t
                    .connections
                    .iter()
                    .filter(|c| c.cap() > 0 && c.done < c.cap())
                    .map(|c| c.done)
                    .sum();
                t.downloaded = (x as u64 * piece + rem_new).min(t.total);
            }
        }
        self.dialog = None;
        self.set_toast("✓ 任务参数已更新，立即生效");
    }

    pub(super) fn dlg_confirm_add(&mut self, paused: bool) {
        let Some(d) = self.dialog.as_ref() else {
            return;
        };
        let url = d.url.value.trim().to_string();
        if url.is_empty() {
            self.set_toast("⚠ 请输入下载 URL");
            return;
        }
        let dir_raw = d.dir.value.trim().to_string();
        let conns_raw = d.conns.value.trim().to_string();
        let down_raw = d.down_limit.value.trim().to_string();
        let ck_raw = d.ck_value.value.trim().to_string();
        let ck_type = d.ck_type;

        let proxy_sel = d.proxy_sel.min(self.proxy_options.len() - 1);

        let checksum = if ck_type == CK_NONE {
            None
        } else if ck_raw.is_empty() {
            let (algo, need) = CHECKSUM_ALGOS[ck_type.min(CHECKSUM_ALGOS.len() - 1)];
            self.set_toast(format!(
                "⚠ {} 校验码不可为空（需 {} 位十六进制）",
                algo, need
            ));
            if let Some(d) = self.dialog.as_mut() {
                d.focus_field(5);
            }
            return;
        } else {
            let (algo, need) = CHECKSUM_ALGOS[ck_type.min(CHECKSUM_ALGOS.len() - 1)];
            let v = ck_raw.to_lowercase();
            if !v.chars().all(|c| c.is_ascii_hexdigit()) {
                self.set_toast(format!("⚠ {} 校验码只能是十六进制字符（0-9a-f）", algo));
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = 5;
                }
                return;
            }
            let len = v.chars().count();
            if len != need {
                self.set_toast(format!(
                    "⚠ {} 校验码需为 {} 位十六进制（当前 {} 位）",
                    algo, need, len
                ));
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = 5;
                }
                return;
            }
            Some(Checksum { algo, value: v })
        };

        let dir = if dir_raw.is_empty() {
            "/srv/downloads".to_string()
        } else {
            dir_raw.trim_end_matches('/').to_string()
        };

        if self
            .tasks
            .iter()
            .any(|t| t.url == url && save_dir_of(&t.save_path) == dir)
        {
            self.set_toast("⚠ 任务已存在");
            return;
        }
        let down_prefill = d.down_prefill;
        self.sync_sim_sidecars();
        let (proto, base_name) = parse_url_task(&url);

        let resumed = self
            .sim_sidecars
            .contains(&(dir.clone(), base_name.clone()))
            || self.tasks.iter().any(|t| {
                t.url == url
                    && save_dir_of(&t.save_path) == dir
                    && t.name == base_name
                    && !matches!(t.state, TaskState::Completed | TaskState::Seeding)
            });
        let name = if resumed {
            base_name.clone()
        } else {
            let mut seq = 1u32;
            let mut n = base_name.clone();
            while self
                .tasks
                .iter()
                .any(|t| t.name == n && save_dir_of(&t.save_path) == dir)
            {
                n = format!("{}.{}", base_name, seq);
                seq += 1;
            }
            n
        };

        let conns = conns_raw
            .parse::<usize>()
            .map(|v| v.clamp(1, 64))
            .unwrap_or_else(|_| default_conns(proto));

        let id = alloc_task_id(&self.tasks);
        self.prefill_seq = self.prefill_seq.wrapping_add(1);
        let proxy = self.proxy_options[proxy_sel].clone();
        let total = pseudo_total(&name);
        let save_path = format!("{}/{}", dir, name);

        let done = if resumed { resumed_done(id, total) } else { 0 };
        let base = 5_000_000.0 + (id as f64 * 1.37e6) % 7_000_000.0;
        let mut t = mk(
            id,
            &name,
            proto,
            &url,
            &save_path,
            total,
            if paused {
                TaskState::Paused
            } else {
                TaskState::Queued
            },
            done,
            conns,
            base,
            "刚刚",
        );
        t.post_process = is_archive(&name);
        if paused {
        } else {
            t.queued_since = Some(Instant::now());
            t.start_delay = 2.0;
        }
        t.checksum = checksum;
        t.proxy = proxy;
        t.task_max_down = parse_speed_limit(&down_raw).unwrap_or(down_prefill);
        self.tasks.push(t);
        self.dialog = None;

        self.filter = 0;
        let flen = self.filtered().len();
        if flen > 0 {
            self.selected = flen - 1;
        }

        if paused {
            if resumed {
                self.set_toast(format!(
                    "⏸ 已添加任务 #{}: {}（发现有效断点，已暂停，按空格继续）",
                    id, name
                ));
            } else {
                self.set_toast(format!(
                    "⏸ 已添加任务 #{}: {}（已暂停，按空格开始下载）",
                    id, name
                ));
            }
        } else if resumed {
            self.set_toast(format!(
                "✓ 已添加任务 #{}: {}（发现有效断点，将自动接续）",
                id, name
            ));
        } else {
            self.set_toast(format!("✓ 已添加任务 #{}: {}", id, name));
            if self.used_slots() >= MAX_DOWNLOAD_SLOTS {
                self.set_toast(format!(
                    "✓ 已添加任务 #{}: {}（槽位已满 {}/{}，进入等待队列）",
                    id,
                    name,
                    self.used_slots(),
                    MAX_DOWNLOAD_SLOTS
                ));
            }
        }
    }

    pub(super) fn sync_sim_sidecars(&mut self) {
        let live: HashSet<(String, String)> = self
            .tasks
            .iter()
            .filter(|t| matches!(t.state, TaskState::Completed | TaskState::Seeding))
            .map(|t| (save_dir_of(&t.save_path), t.name.clone()))
            .collect();
        self.sim_sidecars.retain(|k| !live.contains(k));
    }

    pub(super) fn dlg_activate_delete(&mut self, btn: usize) {
        let Some(idx) = self.sel_idx() else {
            self.dialog = None;
            return;
        };
        let (name, dir, has_sidecar) = {
            let t = &self.tasks[idx];
            (
                t.name.clone(),
                save_dir_of(&t.save_path),
                !matches!(t.state, TaskState::Completed | TaskState::Seeding),
            )
        };
        match btn {
            0 => {
                if has_sidecar {
                    self.sim_sidecars.insert((dir, name.clone()));
                }
                self.tasks.remove(idx);
                self.dialog = None;
                self.set_toast(format!("🗑 已删除任务（保留文件）: {}", name));
            }
            1 => {
                self.sim_sidecars.remove(&(dir, name.clone()));
                self.tasks.remove(idx);
                self.dialog = None;
                self.set_toast(format!("🗑 已删除任务及本地文件: {}", name));
            }
            _ => {
                self.dialog = None;
            }
        }
        let flen = self.filtered().len();
        if self.selected >= flen {
            self.selected = flen.saturating_sub(1);
        }
    }

    /// 打开「添加任务：HTTP(S)」（ADD_POOL 轮换预填；A 键已改为命令窗口入口，
    /// 本入口保留给回归测试与未来直加通道使用）
    #[allow(dead_code)]
    pub fn open_add_dialog(&mut self) {
        let prefill = ADD_POOL[(self.prefill_seq as usize) % ADD_POOL.len()];
        self.open_add_dialog_with(prefill);
    }

    /// 打开「添加任务：HTTP(S)」并预填指定 URL（命令窗口分流入口）
    pub fn open_add_dialog_with(&mut self, prefill: &str) {
        let (proto, _) = parse_url_task(prefill);

        self.dialog_pos = None;
        self.dialog_drag = None;
        self.dialog_drag_moved = false;
        self.dialog = Some(Dialog {
            kind: DialogKind::Add,

            url: TextInput::with_value(prefill),
            dir: TextInput::with_value("/srv/downloads"),
            conns: TextInput::with_value(default_conns(proto).to_string()),
            conns_edited: false,

            down_limit: TextInput::with_value(format_speed_limit(TASK_LIMIT_DEFAULTS.0)),
            up_limit: TextInput::with_value(format_speed_limit(TASK_LIMIT_DEFAULTS.1)),
            down_prefill: TASK_LIMIT_DEFAULTS.0,
            up_prefill: TASK_LIMIT_DEFAULTS.1,

            ck_type: CK_NONE,
            ck_value: TextInput::new(),
            ck_open: false,
            ck_sel: CK_NONE,
            proxy_sel: self.default_proxy_sel,
            proxy_open: false,
            focus: 0,
            task_name: String::new(),
            task_id: None,
            torrent: TextInput::new(),
            bt_tree: Vec::new(),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: MultiLineBuf::new(),
        });
    }

    /// 打开「添加任务：BT」并预填种子来源（本地 .torrent 路径 / 磁力链接；命令窗口分流入口）
    pub fn open_add_bt_dialog_with(&mut self, source: &str) {
        let name = bt_task_name(source);
        let total = pseudo_total(&name);
        self.dialog_pos = None;
        self.dialog_drag = None;
        self.dialog_drag_moved = false;
        self.dialog = Some(Dialog {
            kind: DialogKind::AddBt,
            url: TextInput::new(),
            dir: TextInput::with_value("/srv/downloads"),
            conns: TextInput::with_value(default_conns(Protocol::Bt).to_string()),
            conns_edited: false,
            down_limit: TextInput::with_value(format_speed_limit(TASK_LIMIT_DEFAULTS.0)),
            up_limit: TextInput::with_value(format_speed_limit(TASK_LIMIT_DEFAULTS.1)),
            down_prefill: TASK_LIMIT_DEFAULTS.0,
            up_prefill: TASK_LIMIT_DEFAULTS.1,
            ck_type: CK_NONE,
            ck_value: TextInput::new(),
            ck_open: false,
            ck_sel: CK_NONE,
            proxy_sel: self.default_proxy_sel,
            proxy_open: false,
            focus: 0,
            task_name: String::new(),
            task_id: None,
            torrent: TextInput::with_value(source),
            bt_tree: bt_build_tree(&pseudo_bt_files(&name, total)),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: MultiLineBuf::new(),
        });
    }

    /// 打开「修改任务：BT」（选中任务为 BT 时由 M 分流进入）
    pub fn open_modify_bt_dialog(&mut self) {
        let Some(t) = self.sel_task() else {
            self.set_toast("没有选中的任务");
            return;
        };
        if t.state == TaskState::Completed {
            self.set_toast("⚠ 已完成任务不可修改");
            return;
        }
        let (id, name, concurrency, proxy, max_down, max_up) = (
            t.id,
            t.name.clone(),
            t.concurrency,
            t.proxy.clone(),
            t.task_max_down,
            t.task_max_up,
        );
        let files = if t.bt_files.is_empty() {
            pseudo_bt_files(&name, t.total)
        } else {
            t.bt_files.clone()
        };
        let trackers_src = if t.trackers.is_empty() {
            demo_bt_trackers()
        } else {
            t.trackers.clone()
        };

        let proxy_sel = match self.proxy_options.iter().position(|o| o == &proxy) {
            Some(i) => i,
            None => {
                self.proxy_labels.push(match &proxy {
                    ProxyChoice::Named(n) => n.clone(),
                    ProxyChoice::Direct => "直连".to_string(),
                });
                self.proxy_options.push(proxy.clone());
                self.proxy_options.len() - 1
            }
        };

        self.dialog_pos = None;
        self.dialog_drag = None;
        self.dialog_drag_moved = false;
        self.dialog = Some(Dialog {
            kind: DialogKind::ModifyBt,
            url: TextInput::new(),
            dir: TextInput::new(),
            conns: TextInput::with_value(concurrency.to_string()),
            conns_edited: false,
            down_limit: TextInput::with_value(format_speed_limit(max_down)),
            up_limit: TextInput::with_value(format_speed_limit(max_up)),
            down_prefill: max_down,
            up_prefill: max_up,
            ck_type: CK_NONE,
            ck_value: TextInput::new(),
            ck_open: false,
            ck_sel: CK_NONE,
            proxy_sel,
            proxy_open: false,
            focus: Dialog::bt_list_focus(DialogKind::ModifyBt),
            task_name: name,
            task_id: Some(id),
            torrent: TextInput::new(),
            bt_tree: bt_build_tree(&files),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: {
                let mut tr = MultiLineBuf::new();
                tr.set_text(&trackers_src.join("\n"));
                tr
            },
        });
    }

    pub fn open_about(&mut self) {
        self.about_open = true;
    }

    pub fn close_about(&mut self) {
        self.about_open = false;
        self.about_area = None;
        self.about_drag = None;
    }

    pub fn open_modify_dialog(&mut self) {
        if self
            .sel_task()
            .is_some_and(|t| t.protocol.is_bt() && t.state != TaskState::Completed)
        {
            self.open_modify_bt_dialog();
            return;
        }
        let Some(t) = self.sel_task() else {
            self.set_toast("没有选中的任务");
            return;
        };
        if t.state == TaskState::Completed {
            self.set_toast("⚠ 已完成任务不可修改");
            return;
        }
        let (id, name, concurrency, checksum, proxy, max_down, max_up) = (
            t.id,
            t.name.clone(),
            t.concurrency,
            t.checksum.clone(),
            t.proxy.clone(),
            t.task_max_down,
            t.task_max_up,
        );
        let ck_type = CHECKSUM_ALGOS
            .iter()
            .position(|(n, _)| checksum.as_ref().is_some_and(|c| c.algo == *n))
            .unwrap_or(CK_NONE);

        let proxy_sel = match self.proxy_options.iter().position(|o| o == &proxy) {
            Some(i) => i,
            None => {
                self.proxy_labels.push(match &proxy {
                    ProxyChoice::Named(n) => n.clone(),
                    ProxyChoice::Direct => "直连".to_string(),
                });
                self.proxy_options.push(proxy.clone());
                self.proxy_options.len() - 1
            }
        };

        self.dialog_pos = None;
        self.dialog_drag = None;
        self.dialog_drag_moved = false;
        self.dialog = Some(Dialog {
            kind: DialogKind::Modify,
            url: TextInput::new(),
            dir: TextInput::new(),
            conns: TextInput::with_value(concurrency.to_string()),
            conns_edited: false,
            down_limit: TextInput::with_value(format_speed_limit(max_down)),
            up_limit: TextInput::with_value(format_speed_limit(max_up)),
            down_prefill: max_down,
            up_prefill: max_up,
            ck_type,
            ck_value: TextInput::with_value(checksum.map(|c| c.value).unwrap_or_default()),
            ck_open: false,
            ck_sel: ck_type,
            proxy_sel,
            proxy_open: false,
            focus: 0,
            task_name: name,
            task_id: Some(id),
            torrent: TextInput::new(),
            bt_tree: Vec::new(),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: MultiLineBuf::new(),
        });
    }

    pub fn open_delete_dialog(&mut self) {
        let name = match self.sel_task() {
            Some(t) => t.name.clone(),
            None => {
                self.set_toast("没有选中的任务");
                return;
            }
        };

        self.dialog_pos = None;
        self.dialog_drag = None;
        self.dialog_drag_moved = false;
        self.dialog = Some(Dialog {
            kind: DialogKind::Delete,
            url: TextInput::new(),
            dir: TextInput::new(),
            conns: TextInput::new(),
            conns_edited: false,
            down_limit: TextInput::new(),
            up_limit: TextInput::new(),
            down_prefill: 0,
            up_prefill: 0,
            ck_type: 0,
            ck_value: TextInput::new(),
            ck_open: false,
            ck_sel: 0,
            proxy_sel: 0,
            proxy_open: false,
            focus: 0,
            task_name: name,
            task_id: None,
            torrent: TextInput::new(),
            bt_tree: Vec::new(),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: MultiLineBuf::new(),
        });
    }
}

impl App {
    // ---------- BT 对话框（添加 / 修改） ----------

    /// （仅 AddBt）trackers 焦点键入：字符插入（控制字符忽略）
    fn bt_trackers_char(&mut self, c: char) {
        if !c.is_control() {
            if let Some(d) = self.dialog.as_mut() {
                d.trackers.insert_char(c);
            }
        }
    }

    pub(super) fn add_bt_dialog_enter(&mut self, focus: usize) {
        // 焦点表：0 种子 / 1 保存到 / 2 文件列表 / 3 并发 / 4 代理 / 5 trackers
        // / 6 立即下载 / 7 仅添加 / 8 取消
        if focus == 6 {
            let d = self.dialog.as_mut().unwrap();
            d.proxy_open = true;
        } else if focus == 8 {
            self.dlg_confirm_add_bt(false);
        } else if focus == 9 {
            self.dlg_confirm_add_bt(true);
        } else if focus == 10 {
            self.dialog = None;
        } else {
            self.dlg_confirm_add_bt(false);
        }
    }

    pub(super) fn modify_bt_dialog_enter(&mut self, focus: usize) {
        // 焦点表：0 文件列表 / 1 并发 / 2 代理 / 3 trackers / 4 确定 / 5 取消
        if focus == 4 {
            let d = self.dialog.as_mut().unwrap();
            d.proxy_open = true;
        } else if focus == 7 {
            self.dialog = None;
        } else {
            self.dlg_confirm_modify_bt();
        }
    }

    pub(super) fn add_bt_dialog_char(&mut self, focus: usize, c: char) {
        if focus < 2 {
            if !c.is_control() {
                let d = self.dialog.as_mut().unwrap();
                match focus {
                    0 => {
                        d.torrent.insert(&c.to_string(), 300);
                    }
                    1 => {
                        d.dir.insert(&c.to_string(), 300);
                    }
                    _ => {}
                }
            }
        } else if focus == 2 {
            self.bt_list_char(c);
        } else if focus == 3 {
            let d = self.dialog.as_mut().unwrap();
            super::dialog::insert_conns_digit(d, c);
        } else if let Some(slot) = Self::limit_char_slot(DialogKind::AddBt, focus, c) {
            let d = self.dialog.as_mut().unwrap();
            if slot == 0 {
                super::dialog::insert_limit_char(&mut d.down_limit, c);
            } else {
                super::dialog::insert_limit_char(&mut d.up_limit, c);
            }
        } else if focus == 7 {
            self.bt_trackers_char(c);
        } else if c == ' ' {
            if focus == 6 {
                let d = self.dialog.as_mut().unwrap();
                d.proxy_open = true;
            } else {
                self.dlg_activate_add_bt(focus);
            }
        }
    }

    pub(super) fn modify_bt_dialog_char(&mut self, focus: usize, c: char) {
        if focus == 0 {
            self.bt_list_char(c);
        } else if focus == 1 {
            let d = self.dialog.as_mut().unwrap();
            super::dialog::insert_conns_digit(d, c);
        } else if let Some(slot) = Self::limit_char_slot(DialogKind::ModifyBt, focus, c) {
            let d = self.dialog.as_mut().unwrap();
            if slot == 0 {
                super::dialog::insert_limit_char(&mut d.down_limit, c);
            } else {
                super::dialog::insert_limit_char(&mut d.up_limit, c);
            }
        } else if focus == 5 {
            self.bt_trackers_char(c);
        } else if c == ' ' {
            if focus == 4 {
                let d = self.dialog.as_mut().unwrap();
                d.proxy_open = true;
            } else {
                self.dlg_activate_modify_bt(focus);
            }
        }
    }

    fn bt_list_char(&mut self, c: char) {
        let Some(d) = self.dialog.as_mut() else {
            return;
        };
        match c {
            ' ' => d.bt_toggle_cursor(),
            'a' | 'A' => d.bt_select_all(true),
            'n' | 'N' => d.bt_select_all(false),
            'i' | 'I' => d.bt_invert(),
            _ => {}
        }
    }

    pub(super) fn dlg_activate_add_bt(&mut self, btn: usize) {
        match btn {
            8 => self.dlg_confirm_add_bt(false),
            9 => self.dlg_confirm_add_bt(true),
            10 => self.dialog = None,
            _ => {}
        }
    }

    pub(super) fn dlg_activate_modify_bt(&mut self, btn: usize) {
        match btn {
            6 => self.dlg_confirm_modify_bt(),
            7 => self.dialog = None,
            _ => {}
        }
    }

    fn reject_bad_trackers(&mut self, trackers: &[String], focus: usize) {
        let Some(bad) = tracker_reject(trackers) else {
            return;
        };
        self.set_toast(format!(
            "⚠ 不支持的 tracker 协议（仅 http/https/udp）：{}",
            truncate(&bad, 44)
        ));
        if let Some(d) = self.dialog.as_mut() {
            d.focus_field(focus);
        }
    }

    pub(super) fn dlg_confirm_add_bt(&mut self, paused: bool) {
        let Some(d) = self.dialog.as_ref() else {
            return;
        };
        let source = d.torrent.value.trim().to_string();
        if source.is_empty() {
            self.set_toast("⚠ 请输入种子文件路径或磁力链接");
            if let Some(d) = self.dialog.as_mut() {
                d.focus_field(0);
            }
            return;
        }
        let (sel_cnt, total_files) = d.bt_counts();
        if sel_cnt == 0 {
            self.set_toast("⚠ 请至少选择一个要下载的文件（空格勾选）");
            if let Some(d) = self.dialog.as_mut() {
                d.focus = Dialog::bt_list_focus(DialogKind::AddBt);
            }
            return;
        }
        let files = d.bt_flatten();
        let dir_raw = d.dir.value.trim().to_string();
        let conns_raw = d.conns.value.trim().to_string();
        let down_raw = d.down_limit.value.trim().to_string();
        let up_raw = d.up_limit.value.trim().to_string();
        let trackers = d.trackers.trim_lines();
        if tracker_reject(&trackers).is_some() {
            self.reject_bad_trackers(&trackers, 7);
            return;
        }
        let proxy_sel = d.proxy_sel.min(self.proxy_options.len() - 1);

        let dir = if dir_raw.is_empty() {
            "/srv/downloads".to_string()
        } else {
            dir_raw.trim_end_matches('/').to_string()
        };

        let name0 = bt_task_name(&source);
        let url = source.clone();

        if self
            .tasks
            .iter()
            .any(|t| t.url == url && save_dir_of(&t.save_path) == dir)
        {
            self.set_toast("⚠ 任务已存在");
            return;
        }
        let down_prefill = d.down_prefill;
        let up_prefill = d.up_prefill;
        self.sync_sim_sidecars();

        let resumed = self.sim_sidecars.contains(&(dir.clone(), name0.clone()))
            || self.tasks.iter().any(|t| {
                t.url == url
                    && save_dir_of(&t.save_path) == dir
                    && t.name == name0
                    && !matches!(t.state, TaskState::Completed | TaskState::Seeding)
            });
        let name = if resumed {
            name0.clone()
        } else {
            let mut seq = 1u32;
            let mut n = name0.clone();
            while self
                .tasks
                .iter()
                .any(|t| t.name == n && save_dir_of(&t.save_path) == dir)
            {
                n = format!("{}.{}", name0, seq);
                seq += 1;
            }
            n
        };

        let conns = conns_raw
            .parse::<usize>()
            .map(|v| v.clamp(1, 64))
            .unwrap_or_else(|_| default_conns(Protocol::Bt));

        let id = alloc_task_id(&self.tasks);
        let proxy = self.proxy_options[proxy_sel].clone();
        let total = bt_selected_total(&files).max(1);
        let save_path = format!("{}/{}", dir, name);

        let done = if resumed { resumed_done(id, total) } else { 0 };
        let base = 5_000_000.0 + (id as f64 * 1.37e6) % 7_000_000.0;
        let mut t = mk(
            id,
            &name,
            Protocol::Bt,
            &url,
            &save_path,
            total,
            if paused {
                TaskState::Paused
            } else {
                TaskState::Queued
            },
            done,
            conns,
            base,
            "刚刚",
        );
        t.checksum = None;
        t.proxy = proxy;
        t.bt_files = files;
        t.task_max_down = parse_speed_limit(&down_raw).unwrap_or(down_prefill);
        t.task_max_up = parse_speed_limit(&up_raw).unwrap_or(up_prefill);
        t.trackers = if trackers.is_empty() {
            demo_bt_trackers()
        } else {
            trackers
        };
        // BT 演示口径：伪生成做种人数/ peers，避免详情全为 0
        let h = (id as u64) % 97;
        t.seeders = (3 + h % 40) as usize;
        t.peers = (t.seeders * 2 + (h % 17) as usize).max(t.seeders);
        if paused {
        } else {
            t.queued_since = Some(Instant::now());
            t.start_delay = 2.0;
        }
        self.tasks.push(t);
        self.dialog = None;

        self.filter = 0;
        let flen = self.filtered().len();
        if flen > 0 {
            self.selected = flen - 1;
        }

        if paused {
            self.set_toast(format!(
                "⏸ 已添加任务 #{}: {}（BT，{}/{} 文件，已暂停，按空格开始）",
                id, name, sel_cnt, total_files
            ));
        } else {
            self.set_toast(format!(
                "✓ 已添加任务 #{}: {}（BT，已选 {}/{} 文件）",
                id, name, sel_cnt, total_files
            ));
        }
    }

    pub(super) fn dlg_confirm_modify_bt(&mut self) {
        let Some(d) = self.dialog.as_ref() else {
            return;
        };
        let Some(id) = d.task_id else {
            return;
        };
        let Some(idx) = self.tasks.iter().position(|t| t.id == id) else {
            self.dialog = None;
            return;
        };
        let (sel_cnt, _total_files) = d.bt_counts();
        if sel_cnt == 0 {
            self.set_toast("⚠ 请至少保留一个要下载的文件（空格勾选）");
            if let Some(d) = self.dialog.as_mut() {
                d.focus = Dialog::bt_list_focus(DialogKind::ModifyBt);
            }
            return;
        }
        let files = d.bt_flatten();
        let conns_raw = d.conns.value.trim().to_string();
        let down_raw = d.down_limit.value.trim().to_string();
        let up_raw = d.up_limit.value.trim().to_string();
        let trackers = d.trackers.trim_lines();
        if tracker_reject(&trackers).is_some() {
            self.reject_bad_trackers(&trackers, 5);
            return;
        }
        let proxy_sel = d.proxy_sel.min(self.proxy_options.len().saturating_sub(1));
        let proxy = self.proxy_options[proxy_sel].clone();

        let concurrency = if conns_raw.is_empty() {
            self.tasks[idx].concurrency
        } else {
            conns_raw.parse::<usize>().unwrap_or(0).clamp(1, 64)
        };

        let old_total = self.tasks[idx].total;
        let new_total = bt_selected_total(&files).max(1);
        let changed = new_total != old_total || concurrency != self.tasks[idx].concurrency;

        {
            let t = &mut self.tasks[idx];
            let down = if down_raw.is_empty() {
                TASK_LIMIT_DEFAULTS.0
            } else {
                parse_speed_limit(&down_raw).unwrap_or(t.task_max_down)
            };
            let up = if up_raw.is_empty() {
                TASK_LIMIT_DEFAULTS.1
            } else {
                parse_speed_limit(&up_raw).unwrap_or(t.task_max_up)
            };
            t.task_max_down = down;
            t.task_max_up = up;
            t.trackers = trackers;
            t.bt_files = files;
            t.concurrency = concurrency;
            t.proxy = proxy;
            if changed {
                let old_speeds: Vec<f64> = t.connections.iter().map(|c| c.speed).collect();
                t.total = new_total;
                t.downloaded = t.downloaded.min(new_total);
                let piece = chunk_size(t.protocol);

                let mut done_blocks = t.chunk_done as u64;
                let mut rem: u64 = 0;
                for c in t.connections.iter() {
                    if c.cap() == 0 {
                        continue;
                    }
                    if c.done >= c.cap() {
                        done_blocks += 1;
                    } else {
                        rem += c.done;
                    }
                }
                let y = chunk_total(t.total, piece) as u64;
                let done_blocks = done_blocks.min(y);
                let (mut conns_new, x) = make_chunk_conns_lease(
                    t.total,
                    done_blocks as u32,
                    rem,
                    concurrency,
                    piece,
                    t.id as u64,
                );
                for (i, c) in conns_new.iter_mut().enumerate() {
                    if let Some(s) = old_speeds.get(i) {
                        c.speed = *s;
                    }
                }
                t.connections = conns_new;
                t.chunk_done = x;

                let rem_new: u64 = t
                    .connections
                    .iter()
                    .filter(|c| c.cap() > 0 && c.done < c.cap())
                    .map(|c| c.done)
                    .sum();
                t.downloaded = (x as u64 * piece + rem_new).min(t.total);
            }
        }
        self.dialog = None;
        self.set_toast(format!(
            "✓ BT 任务已更新：{}/{} 文件 · 共 {}（立即生效）",
            sel_cnt,
            self.tasks[idx].bt_files.len(),
            fmt_size(self.tasks[idx].total)
        ));
    }
}

#[cfg(test)]
mod limit_backfill_tests {
    use super::*;

    #[test]
    fn parse_speed_limit_bare_number_is_b_per_s() {
        assert_eq!(
            parse_speed_limit("512"),
            Some(512),
            "纯数字按 B/s 解析（bug23 口径）"
        );
        assert_eq!(parse_speed_limit("512 B/s"), Some(512));
        assert_eq!(parse_speed_limit("512 KiB/s"), Some(524_288));
        assert_eq!(parse_speed_limit("1 MiB/s"), Some(1_048_576));
    }

    #[test]
    fn format_speed_limit_round_trips_through_parse() {
        for v in [0u64, 1, 512, 5120, 1536, 500_000, 1_048_576, 1_073_741_824] {
            assert_eq!(
                parse_speed_limit(&format_speed_limit(v)),
                Some(v),
                "fmt → parse 恒等：{v}"
            );
        }
    }

    #[test]
    fn modify_confirm_empty_down_limit_backfills_task_default() {
        let mut app = App::new();
        app.tasks[0].task_max_down = 5120;
        app.open_modify_dialog();
        assert_eq!(
            app.dialog.as_ref().unwrap().down_limit.value,
            format_speed_limit(5120),
            "修改窗口预填规范化单位显示（5120 = 5 KiB/s）"
        );
        app.dialog.as_mut().unwrap().down_limit.clear();
        app.dlg_confirm_modify();
        assert_eq!(
            app.tasks[0].task_max_down, TASK_LIMIT_DEFAULTS.0,
            "清空确认 = 回填任务级默认限速，而非保留旧值 5120"
        );
    }

    #[test]
    fn modify_bt_confirm_empty_limits_backfill_task_defaults() {
        let mut app = App::new();
        let bt0 = app
            .tasks
            .iter()
            .position(|t| t.protocol.is_bt())
            .expect("demo 任务含 BT");
        app.selected = bt0;
        app.open_modify_bt_dialog();
        let tid = app
            .dialog
            .as_ref()
            .unwrap()
            .task_id
            .expect("BT 修改对话框携带任务 id");
        let idx = app
            .tasks
            .iter()
            .position(|t| t.id == tid)
            .expect("task_id 可回查任务");
        app.tasks[idx].task_max_down = 5120;
        app.tasks[idx].task_max_up = 8192;
        {
            let d = app.dialog.as_mut().unwrap();
            d.down_limit.clear();
            d.up_limit.clear();
        }
        app.dlg_confirm_modify_bt();
        assert!(app.dialog.is_none(), "确认后关闭");
        assert_eq!(
            app.tasks[idx].task_max_down, TASK_LIMIT_DEFAULTS.0,
            "BT 修改清空确认 = 回填任务级下载限速默认"
        );
        assert_eq!(
            app.tasks[idx].task_max_up, TASK_LIMIT_DEFAULTS.1,
            "BT 修改清空确认 = 回填任务级上传限速默认"
        );
    }
}
