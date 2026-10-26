use std::collections::HashSet;
use std::time::Instant;

use crate::textfield::TextInput;

use super::demo::mk;
use super::model::ADD_POOL;
use super::*;

impl App {
    pub(super) fn add_dialog_enter(&mut self, focus: usize) {
        if focus == 3 {
            let d = self.dialog.as_mut().unwrap();
            d.ck_open = true;
            d.ck_sel = d.ck_type;
        } else if focus == 5 {
            let d = self.dialog.as_mut().unwrap();
            d.proxy_open = true;
        } else if focus == 7 {
            self.dlg_add_only();
        } else if focus == 8 {
            self.dialog = None;
        } else {
            self.dlg_confirm_add(false);
        }
    }

    fn dlg_add_only(&mut self) {
        self.dlg_confirm_add(true);
    }

    pub(super) fn modify_dialog_enter(&mut self, focus: usize) {
        if focus == 1 {
            let d = self.dialog.as_mut().unwrap();
            d.ck_open = true;
            d.ck_sel = d.ck_type;
        } else if focus == 3 {
            let d = self.dialog.as_mut().unwrap();
            d.proxy_open = true;
        } else if focus == 5 {
            self.dialog = None;
        } else {
            self.dlg_confirm_modify();
        }
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
                        if c.is_ascii_digit() && d.conns.insert(&c.to_string(), 2) > 0 {
                            d.conns_edited = true;
                        }
                    }
                }
            }
        } else if focus == 3 {
            if c == ' ' {
                let d = self.dialog.as_mut().unwrap();
                d.ck_open = true;
                d.ck_sel = d.ck_type;
            }
        } else if focus == 4 {
            if self.dialog.as_ref().unwrap().ck_type != CK_NONE && c.is_ascii_hexdigit() {
                self.dialog
                    .as_mut()
                    .unwrap()
                    .ck_value
                    .insert(&c.to_string(), 128);
            }
        } else if c == ' ' {
            if focus == 5 {
                let d = self.dialog.as_mut().unwrap();
                d.proxy_open = true;
            } else {
                self.dlg_activate_add(focus);
            }
        }
    }

    pub(super) fn modify_dialog_char(&mut self, focus: usize, c: char) {
        if focus == 0 {
            if c.is_ascii_digit()
                && self
                    .dialog
                    .as_mut()
                    .unwrap()
                    .conns
                    .insert(&c.to_string(), 2)
                    > 0
            {
                self.dialog.as_mut().unwrap().conns_edited = true;
            }
        } else if focus == 1 {
            if c == ' ' {
                let d = self.dialog.as_mut().unwrap();
                d.ck_open = true;
                d.ck_sel = d.ck_type;
            }
        } else if focus == 2 {
            if self.dialog.as_ref().unwrap().ck_type != CK_NONE && c.is_ascii_hexdigit() {
                self.dialog
                    .as_mut()
                    .unwrap()
                    .ck_value
                    .insert(&c.to_string(), 128);
            }
        } else if c == ' ' {
            if focus == 3 {
                let d = self.dialog.as_mut().unwrap();
                d.proxy_open = true;
            } else {
                self.dlg_activate_modify(focus);
            }
        }
    }

    pub(super) fn dlg_activate_add(&mut self, btn: usize) {
        match btn {
            6 => self.dlg_confirm_add(false),
            7 => self.dlg_add_only(),
            8 => self.dialog = None,
            _ => {}
        }
    }

    pub(super) fn dlg_activate_modify(&mut self, btn: usize) {
        match btn {
            4 => self.dlg_confirm_modify(),
            5 => self.dialog = None,
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
                d.focus_field(2);
            }
            return;
        } else {
            let (algo, need) = CHECKSUM_ALGOS[ck_type.min(CHECKSUM_ALGOS.len() - 1)];
            let v = ck_raw.to_lowercase();
            if !v.chars().all(|c| c.is_ascii_hexdigit()) {
                self.set_toast(format!("⚠ {algo} 校验码只能是十六进制字符（0-9a-f）"));
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = 2;
                }
                return;
            }
            let len = v.chars().count();
            if len != need {
                self.set_toast(format!(
                    "⚠ {algo} 校验码需为 {need} 位十六进制（当前 {len} 位）"
                ));
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = 2;
                }
                return;
            }
            Some(Checksum { algo, value: v })
        };

        {
            let t = &mut self.tasks[idx];
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
                d.focus_field(4);
            }
            return;
        } else {
            let (algo, need) = CHECKSUM_ALGOS[ck_type.min(CHECKSUM_ALGOS.len() - 1)];
            let v = ck_raw.to_lowercase();
            if !v.chars().all(|c| c.is_ascii_hexdigit()) {
                self.set_toast(format!("⚠ {} 校验码只能是十六进制字符（0-9a-f）", algo));
                if let Some(d) = self.dialog.as_mut() {
                    d.focus = 4;
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
                    d.focus = 4;
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

    pub fn open_add_dialog(&mut self) {
        let prefill = ADD_POOL[(self.prefill_seq as usize) % ADD_POOL.len()];

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

            ck_type: CK_NONE,
            ck_value: TextInput::new(),
            ck_open: false,
            ck_sel: CK_NONE,
            proxy_sel: self.default_proxy_sel,
            proxy_open: false,
            focus: 0,
            task_name: String::new(),
            task_id: None,
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
        let Some(t) = self.sel_task() else {
            self.set_toast("没有选中的任务");
            return;
        };
        if t.state == TaskState::Completed {
            self.set_toast("⚠ 已完成任务不可修改");
            return;
        }
        let (id, name, concurrency, checksum, proxy) = (
            t.id,
            t.name.clone(),
            t.concurrency,
            t.checksum.clone(),
            t.proxy.clone(),
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
            ck_type,
            ck_value: TextInput::with_value(checksum.map(|c| c.value).unwrap_or_default()),
            ck_open: false,
            ck_sel: ck_type,
            proxy_sel,
            proxy_open: false,
            focus: 0,
            task_name: name,
            task_id: Some(id),
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
            ck_type: 0,
            ck_value: TextInput::new(),
            ck_open: false,
            ck_sel: 0,
            proxy_sel: 0,
            proxy_open: false,
            focus: 0,
            task_name: name,
            task_id: None,
        });
    }
}
