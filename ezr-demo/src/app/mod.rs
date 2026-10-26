use std::collections::HashSet;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ratatui::layout::Rect;

use crate::scrollbar;
use crate::textfield::FieldWin;

pub const ITEM_HEIGHT: u16 = 4;

mod clipboard;
mod demo;
mod dialog;
mod dialogs;
mod keys;
mod model;
mod mouse;
mod panel;
mod popups;
mod scroll;
mod sim;
mod tasks;

use demo::demo_tasks;
pub use dialog::*;
pub use model::*;
pub use panel::*;
pub use scroll::*;

pub const FILTERS: [&str; 2] = ["正在下载", "已完成"];

pub const MAX_DOWNLOAD_SLOTS: usize = 5;

const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub struct App {
    pub tasks: Vec<Task>,

    pub selected: usize,

    last_selected: usize,

    pub scroll: usize,

    pub filter: usize,

    pub speed_hist: Vec<u64>,

    pub bw_t: f64,

    pub up_hist: Vec<u64>,

    pub session_bytes: u64,
    pub rng: Rng,
    pub frame: u64,
    pub quit: bool,
    pub toast: Option<String>,
    toast_until: Option<Instant>,

    pub prefill_seq: u32,
    last_tick: Instant,

    last_speed_tick: Instant,

    pub visible_rows: usize,

    pub list_area: Option<Rect>,

    pub focus: Panel,

    pub help_open: bool,

    pub about_open: bool,

    pub about_pos: Option<(u16, u16)>,

    pub about_drag: Option<(u16, u16)>,

    pub about_area: Option<Rect>,

    pub help_scroll: usize,

    pub help_pos: Option<(u16, u16)>,

    pub help_drag: Option<(u16, u16)>,

    pub help_area: Option<Rect>,

    pub tab_rects: Vec<(Rect, usize)>,

    pub detail_area: Option<Rect>,

    pub conns_scroll: usize,

    pub conns_sel: usize,

    pub conns_area: Option<Rect>,

    pub detail_url_rect: Option<Rect>,

    pub detail_ck_rect: Option<Rect>,

    pub detail_fail_rect: Option<Rect>,

    pub detail_save_rect: Option<Rect>,

    pub detail_val_rects: Vec<(Rect, DetailField)>,

    pub pointer: Option<(u16, u16)>,

    pub visible_conns_rows: usize,

    conns_scroll_for: usize,

    pub conns_hscroll: usize,

    pub conns_max_hscroll: usize,

    pub list_sb: Option<scrollbar::Bar>,

    pub conns_v_sb: Option<scrollbar::Bar>,

    pub conns_h_sb: Option<scrollbar::Bar>,

    pub pieces_sb: Option<scrollbar::Bar>,

    pub help_sb: Option<scrollbar::Bar>,

    pub pieces_max_scroll: usize,

    pub help_max_scroll: usize,

    pub sb_drag: Option<(SbKey, usize)>,

    pub sb_hold: Option<SbHold>,

    pub sb_pressed: Option<(SbKey, scrollbar::Arrow)>,

    pub sb_flash: Option<(SbKey, scrollbar::Arrow, Instant)>,

    pub show_panes: bool,

    pub pieces_open: bool,

    pub pieces_scroll: usize,

    pub pieces_follow: bool,

    pub pieces_area: Option<Rect>,

    pub pieces_pos: Option<(u16, u16)>,

    pub pieces_drag: Option<(u16, u16)>,

    pub val_open: bool,

    pub val_field: DetailField,

    pub val_title: String,

    pub val_text: String,

    pub val_scroll: usize,

    pub val_max_scroll: usize,

    pub val_area: Option<Rect>,

    pub val_pos: Option<(u16, u16)>,

    pub val_drag: Option<(u16, u16)>,

    pub val_sb: Option<scrollbar::Bar>,

    pub popup_close_rect: Option<Rect>,

    pub x_press: bool,

    pub dialog: Option<Dialog>,

    pub dialog_pos: Option<(u16, u16)>,

    pub dialog_drag: Option<(u16, u16)>,

    pub dialog_drag_moved: bool,

    pub dialog_area: Option<Rect>,

    pub dlg_btn_rects: Vec<(Rect, usize)>,

    pub dlg_field_rects: Vec<(Rect, usize)>,

    pub dlg_field_wins: Vec<Option<FieldWin>>,

    pub dlg_cursor_screen: Option<(u16, u16)>,

    pub dlg_ck_rects: Vec<(Rect, usize)>,

    pub dlg_proxy_rects: Vec<(Rect, usize)>,

    pub footer_cmd_rects: Vec<(Rect, usize)>,

    pub footer_press: Option<usize>,

    pub footer_flash: Option<(usize, Instant)>,

    pub proxy_options: Vec<ProxyChoice>,

    pub proxy_labels: Vec<String>,

    pub default_proxy_sel: usize,

    clipboard: Option<arboard::Clipboard>,

    pub sim_sidecars: HashSet<(String, String)>,
}

impl App {
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E3779B97F4A7C15);

        let mut proxy_options = vec![ProxyChoice::Direct];
        let mut proxy_labels = vec!["直连".to_string()];
        for (name, kind) in demo_proxies() {
            proxy_options.push(ProxyChoice::Named(name.to_string()));
            proxy_labels.push(format!("{}（{}）", name, kind));
        }
        App {
            tasks: demo_tasks(),
            selected: 0,
            last_selected: 0,
            scroll: 0,
            filter: 0,

            speed_hist: Vec::new(),
            bw_t: 0.0,
            up_hist: Vec::new(),
            session_bytes: 0,
            rng: Rng::new(seed),
            frame: 0,
            quit: false,
            help_open: false,
            about_open: false,
            about_pos: None,
            about_drag: None,
            about_area: None,
            help_scroll: 0,
            help_pos: None,
            help_drag: None,
            help_area: None,
            tab_rects: Vec::new(),
            detail_area: None,
            toast: Some("EZR Downloader Demo 就绪 · 数据为模拟数据".to_string()),
            toast_until: Some(Instant::now() + Duration::from_secs(4)),
            prefill_seq: 100,
            sim_sidecars: HashSet::new(),
            last_tick: Instant::now(),
            last_speed_tick: Instant::now(),
            visible_rows: 6,
            list_area: None,
            focus: Panel::List,
            conns_scroll: 0,
            conns_sel: 0,
            conns_area: None,
            detail_url_rect: None,
            detail_ck_rect: None,
            detail_fail_rect: None,
            detail_save_rect: None,
            detail_val_rects: Vec::new(),
            pointer: None,
            visible_conns_rows: 8,
            conns_scroll_for: 0,
            conns_hscroll: 0,
            conns_max_hscroll: 0,
            list_sb: None,
            conns_v_sb: None,
            conns_h_sb: None,
            pieces_sb: None,
            help_sb: None,
            pieces_max_scroll: 0,
            help_max_scroll: 0,
            sb_drag: None,
            sb_hold: None,
            sb_pressed: None,
            sb_flash: None,
            show_panes: true,
            pieces_open: false,
            pieces_scroll: 0,
            pieces_follow: false,
            pieces_area: None,
            pieces_pos: None,
            pieces_drag: None,
            val_open: false,
            popup_close_rect: None,
            x_press: false,
            val_field: DetailField::Url,
            val_title: String::new(),
            val_text: String::new(),
            val_scroll: 0,
            val_max_scroll: 0,
            val_area: None,
            val_pos: None,
            val_drag: None,
            val_sb: None,
            dialog: None,
            dialog_pos: None,
            dialog_drag: None,
            dialog_drag_moved: false,
            dialog_area: None,
            dlg_btn_rects: Vec::new(),
            dlg_field_rects: Vec::new(),
            dlg_field_wins: Vec::new(),
            dlg_cursor_screen: None,
            dlg_ck_rects: Vec::new(),
            dlg_proxy_rects: Vec::new(),
            footer_cmd_rects: Vec::new(),
            footer_press: None,
            footer_flash: None,
            proxy_options,
            proxy_labels,
            default_proxy_sel: 0,
            clipboard: None,
        }
    }

    pub fn spinner(&self) -> char {
        SPINNER[(self.frame as usize) % SPINNER.len()]
    }

    pub fn used_slots(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| match t.state {
                TaskState::Downloading => true,

                TaskState::Failed => t.retry_in.is_some(),
                TaskState::Queued => t.has_slot,
                _ => false,
            })
            .count()
    }

    pub fn queue_pos_of(&self, task_id: u32) -> Option<usize> {
        let mut pos = 0;
        for t in &self.tasks {
            if t.state == TaskState::Queued && !t.has_slot {
                pos += 1;
                if t.id == task_id {
                    return Some(pos);
                }
            }
        }
        None
    }

    fn enforce_slots(&mut self) {
        for t in self.tasks.iter_mut() {
            match t.state {
                TaskState::Downloading => t.has_slot = true,
                TaskState::Failed => t.has_slot = t.retry_in.is_some(),
                TaskState::Queued => {}
                _ => t.has_slot = false,
            }
        }
        let mut used = self.used_slots();
        if used >= MAX_DOWNLOAD_SLOTS {
            return;
        }
        for t in self.tasks.iter_mut() {
            if used >= MAX_DOWNLOAD_SLOTS {
                break;
            }
            if t.state == TaskState::Queued && !t.has_slot {
                t.has_slot = true;
                used += 1;
            }
        }
    }

    pub fn filtered(&self) -> Vec<usize> {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| match self.filter {
                1 => t.state.is_done(),
                _ => !t.state.is_done(),
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn sel_idx(&self) -> Option<usize> {
        self.filtered().get(self.selected).copied()
    }

    pub fn sel_task(&self) -> Option<&Task> {
        self.sel_idx().and_then(|i| self.tasks.get(i))
    }

    fn set_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some(msg.into());
        self.toast_until = Some(Instant::now() + Duration::from_secs(3));
    }

    fn move_sel(&mut self, delta: i32) {
        let flen = self.filtered().len();
        if flen == 0 {
            return;
        }
        let cur = self.selected as i32;
        self.selected = cur.saturating_add(delta).clamp(0, flen as i32 - 1) as usize;

        self.scroll_to_selection();
    }

    fn cycle_filter(&mut self, delta: i32) {
        let n = FILTERS.len() as i32;
        self.filter = ((self.filter as i32 + delta).rem_euclid(n)) as usize;
        self.scroll_to_selection();
    }

    fn scroll_to_selection(&mut self) {
        let vis = self.visible_rows.max(1);
        if self.scroll > self.selected {
            self.scroll = self.selected;
        }
        if self.selected >= self.scroll + vis {
            self.scroll = self.selected + 1 - vis;
        }
    }

    pub fn move_task(&mut self, delta: i32) {
        let fl = self.filtered();
        if fl.is_empty() {
            return;
        }
        let new = self.selected as i32 + delta;
        if new < 0 {
            self.set_toast("已在顶部");
            return;
        }
        if new >= fl.len() as i32 {
            self.set_toast("已在底部");
            return;
        }
        let new = new as usize;
        let (a, b) = (fl[self.selected], fl[new]);
        if a != b {
            self.tasks.swap(a, b);
        }
        self.selected = new;
        let name = self.tasks[fl[new]].name.clone();
        self.set_toast(format!(
            "{} 任务: {}",
            if delta < 0 {
                "↑ 已上移"
            } else {
                "↓ 已下移"
            },
            name
        ));
    }
}
impl App {
    pub fn active_conn_map(&self) -> Vec<usize> {
        self.sel_task()
            .filter(|t| t.state.shows_conns())
            .map(|t| {
                t.connections
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.is_active())
                    .map(|(i, _)| i)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn conn_count(&self) -> usize {
        self.active_conn_map().len()
    }
}
