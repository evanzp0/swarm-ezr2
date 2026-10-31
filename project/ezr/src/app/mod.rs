use std::collections::HashMap;
use std::time::Instant;

use ratatui::layout::Rect;

use crate::engine::{EngineHandle, Evt};
use crate::model::config::Config;
use crate::model::registry::Registry;
use crate::model::sidecar::Sidecar;
use crate::model::speed::{SmoothedSpeed, SpeedWindow};
use crate::model::{slots, Connection, Task};
use crate::textfield::TextInput;

mod bt_dialog;
mod bt_tree;
mod cli;
mod clipboard;
mod cmd;
mod dialog_keys;
mod dialogs;
mod engine;
mod keys;
mod mouse;
mod paste;
mod pieces;
mod scrollbar;
mod tasks;

pub use scrollbar::{SbHold, SbKey};

use crate::scrollbar::{Arrow as SbArrow, Bar as SbBar};
#[cfg(test)]
pub(crate) mod testutil;

#[cfg(test)]
pub(crate) use dialog_keys::{
    dropdown_open_key, proxy_dropdown_open_key, push_hex_capped, text_backspace, text_char,
    TextField,
};

pub const ITEM_HEIGHT: u16 = 4;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Panel {
    #[default]
    List,
    Detail,
    Conns,
}

impl Panel {
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Panel::List => Panel::Detail,
            Panel::Detail => Panel::Conns,
            Panel::Conns => Panel::List,
        }
    }

    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Panel::List => Panel::Conns,
            Panel::Detail => Panel::List,
            Panel::Conns => Panel::Detail,
        }
    }
}

pub const FILTERS: [&str; 2] = ["正在下载", "已完成"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DetailField {
    Checksum,
    Url,
    Failed,
    SavePath,
}

impl DetailField {
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            DetailField::Checksum => "校验",
            DetailField::Url => "URL",
            DetailField::Failed => "失败",
            DetailField::SavePath => "路径",
        }
    }

    #[must_use]
    pub fn copy_label(self) -> &'static str {
        match self {
            DetailField::Checksum => "校验码",
            DetailField::Url => "url",
            DetailField::Failed => "失败原因",
            DetailField::SavePath => "保存路径",
        }
    }
}

const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub use crate::model::checksum::CHECKSUM_ALGOS;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DialogKind {
    Add,
    Delete,
    Modify,
    AddBt,
    ModifyBt,
}

pub use crate::textfield::FieldWin;

pub struct Dialog {
    pub kind: DialogKind,
    pub url: TextInput,
    pub dir: TextInput,
    pub conns: TextInput,
    pub conns_edited: bool,
    pub ck_type: usize,
    pub ck_value: TextInput,
    pub ck_open: bool,
    pub ck_sel: usize,
    pub proxy_sel: usize,
    pub proxy_open: bool,
    pub focus: usize,
    pub task_name: String,
    pub task_id: Option<u32>,
    pub torrent: TextInput,
    pub bt_tree: Vec<crate::app::bt_tree::BtNode>,
    pub bt_cursor: usize,
    pub bt_scroll: usize,
    pub bt_follow: bool,
    pub trackers: crate::multiline::MultiLineBuf,
}

pub struct App {
    pub tasks: Vec<Task>,
    pub selected: usize,
    pub scroll: usize,
    pub filter: usize,
    pub focus: Panel,
    pub speed_hist: Vec<u64>,
    pub up_hist: Vec<u64>,
    pub session_bytes: u64,
    session_seen: HashMap<u32, u64>,
    conn_prev: HashMap<(u32, usize), (u32, u64)>,
    conn_cum: HashMap<(u32, usize), u64>,
    conn_speed: HashMap<(u32, usize), SmoothedSpeed>,
    conn_windows: HashMap<(u32, usize), SpeedWindow>,
    conn_rebase: std::collections::HashSet<u32>,
    pub frame: u64,
    pub quit: bool,
    pub toast: Option<String>,
    toast_until: Option<Instant>,
    last_selected: usize,
    pub help_open: bool,
    pub help_scroll: usize,
    pub help_pos: Option<(u16, u16)>,
    pub help_drag: Option<(u16, u16)>,
    pub help_area: Option<Rect>,
    pub list_scrollbar: Option<SbBar>,
    pub conns_v_scrollbar: Option<SbBar>,
    pub conns_h_scrollbar: Option<SbBar>,
    pub pieces_scrollbar: Option<SbBar>,
    pub help_scrollbar: Option<SbBar>,
    pub val_scrollbar: Option<SbBar>,
    pub pieces_max_scroll: usize,
    pub help_max_scroll: usize,
    pub sb_drag: Option<(SbKey, usize)>,
    pub sb_hold: Option<SbHold>,
    pub sb_pressed: Option<(SbKey, SbArrow)>,
    pub sb_flash: Option<(SbKey, SbArrow, Instant)>,
    pub about_open: bool,
    pub about_pos: Option<(u16, u16)>,
    pub about_drag: Option<(u16, u16)>,
    pub about_area: Option<Rect>,
    pub tab_rects: Vec<(Rect, usize)>,
    pub footer_cmd_rects: Vec<(Rect, usize)>,
    pub footer_press: Option<usize>,
    pub footer_flash: Option<(usize, Instant)>,
    pub max_slots: usize,
    engine: EngineHandle,
    evt_rx: tokio::sync::mpsc::Receiver<Evt>,
    windows: HashMap<u32, SpeedWindow>,
    last_speed_tick: Instant,
    pub proxy_options: Vec<crate::model::ProxyChoice>,
    pub proxy_labels: Vec<String>,
    pub default_proxy_sel: usize,
    pub cfg: Config,
    registry_path: String,
    last_save: Instant,
    last_tick: Instant,
    pub visible_rows: usize,
    pub list_area: Option<Rect>,
    pub show_panes: bool,
    pub conns_scroll: usize,
    pub conns_sel: usize,
    pub conns_area: Option<Rect>,
    pub detail_url_rect: Option<Rect>,
    pub detail_ck_rect: Option<Rect>,
    pub detail_fail_rect: Option<Rect>,
    pub detail_save_rect: Option<Rect>,
    pub detail_val_rects: Vec<(Rect, DetailField)>,
    pub detail_area: Option<Rect>,
    pub pointer: Option<(u16, u16)>,
    pub popup_close_rect: Option<Rect>,
    pub x_press: bool,
    pub val_open: bool,
    pub val_field: DetailField,
    pub val_text: String,
    pub val_scroll: usize,
    pub val_max_scroll: usize,
    pub val_pos: Option<(u16, u16)>,
    pub val_drag: Option<(u16, u16)>,
    pub val_area: Option<Rect>,
    clipboard: Option<arboard::Clipboard>,
    pub(crate) last_copied: Option<String>,
    pub visible_conns_rows: usize,
    conns_scroll_for: usize,
    pub conns_hscroll: usize,
    pub conns_max_hscroll: usize,
    pub dialog: Option<Dialog>,
    pub dlg_btn_rects: Vec<(Rect, usize)>,
    pub dlg_field_rects: Vec<(Rect, usize)>,
    pub dlg_field_wins: Vec<Option<FieldWin>>,
    pub dlg_cursor_screen: Option<(u16, u16)>,
    pub dlg_ck_rects: Vec<(Rect, usize)>,
    pub dlg_proxy_rects: Vec<(Rect, usize)>,
    pub dlg_area: Option<Rect>,
    pub dlg_pos: Option<(u16, u16)>,
    pub dlg_drag: Option<(u16, u16)>,
    pub pieces_open: bool,
    pub pieces_scroll: usize,
    pub pieces_follow: bool,
    pub pieces_area: Option<Rect>,
    pub pieces_pos: Option<(u16, u16)>,
    pub pieces_drag: Option<(u16, u16)>,
    pending_deletes: HashMap<u32, (String, String)>,
    pub dlg_btfile_rects: Vec<(Rect, usize)>,
    pub dlg_btbtn_rects: Vec<(Rect, usize)>,
    pub dlg_bt_sb: Option<SbBar>,
    pub dlg_bt_area: Option<Rect>,
    pub dlg_bt_visible: usize,
    pub dlg_trackers_sb: Option<SbBar>,
    pub dlg_trackers_h_sb: Option<SbBar>,
    pub dlg_trackers_area: Option<Rect>,
    pub cmd_open: bool,
    pub cmd: crate::multiline::MultiLineBuf,
    pub cmd_focus: usize,
    pub cmd_pos: Option<(u16, u16)>,
    pub cmd_drag: Option<(u16, u16)>,
    pub cmd_drag_moved: bool,
    pub cmd_area: Option<Rect>,
    pub cmd_text_area: Option<Rect>,
    pub cmd_sb: Option<SbBar>,
    pub cmd_btn_rect: Option<Rect>,
    pub cmd_cursor_screen: Option<(u16, u16)>,
    pub err_open: bool,
    pub err_msg: String,
    pub err_pos: Option<(u16, u16)>,
    pub err_drag: Option<(u16, u16)>,
    pub err_area: Option<Rect>,
    pub err_btn_rect: Option<Rect>,
    pub bt_conns: HashMap<u32, Vec<BtConnRow>>,
    bt_conn_stats: HashMap<(u32, usize), BtConnSpeedStat>,
    /// BT 引擎「正在停止」登记：暂停是乐观置状态（引擎 select 循环异步消费
    /// Pause），恢复若早于引擎真正退出会拉起第二个引擎——两个引擎同写文件与
    /// sidecar，进度互相回退。登记在暂停时写入、BtStopped/BtFailed 时清除；
    /// 槽位调度见正在停止的任务先不启动（带兜底超时，见 tick_housekeeping）。
    pub(crate) bt_stopping: HashMap<u32, Instant>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BtConnRow {
    pub id: usize,
    pub addr: String,
    pub down_speed: f64,
    pub cum_down: u64,
    pub up_speed: f64,
    pub cum_up: u64,
    pub client: String,
    pub seeder: bool,
}

#[derive(Default)]
pub(crate) struct BtConnSpeedStat {
    pub down_win: SpeedWindow,
    pub up_win: SpeedWindow,
    pub down_ema: SmoothedSpeed,
    pub up_ema: SmoothedSpeed,
}

impl App {
    #[must_use]
    pub fn new(cfg: Config, registry_path: String) -> App {
        let (evt_tx, evt_rx) = tokio::sync::mpsc::channel::<Evt>(256);
        let engine = EngineHandle::start(&cfg, evt_tx);
        let mut proxy_options = vec![crate::model::ProxyChoice::Direct];
        let mut proxy_labels = vec!["直连".to_string()];
        for p in &cfg.proxies {
            proxy_options.push(crate::model::ProxyChoice::Named(p.name.clone()));
            proxy_labels.push(format!("{}（{}）", p.name, p.kind.label()));
        }
        let default_proxy_sel = 0;
        let tasks = Self::restore(&registry_path);
        let mut app = App {
            tasks,
            selected: 0,
            scroll: 0,
            filter: 0,
            focus: Panel::List,
            last_selected: 0,
            speed_hist: vec![0; 90],
            up_hist: vec![0; 90],
            session_bytes: 0,
            session_seen: HashMap::new(),
            conn_prev: HashMap::new(),
            conn_cum: HashMap::new(),
            conn_speed: HashMap::new(),
            conn_windows: HashMap::new(),
            conn_rebase: std::collections::HashSet::new(),
            frame: 0,
            quit: false,
            toast: Some("EZR Downloader 就绪".to_string()),
            toast_until: Some(Instant::now() + std::time::Duration::from_secs(4)),
            max_slots: cfg.download_slots,
            engine,
            evt_rx,
            windows: HashMap::new(),
            cfg,
            registry_path,
            last_save: Instant::now(),
            last_tick: Instant::now(),
            last_speed_tick: Instant::now(),
            visible_rows: 6,
            list_area: None,
            show_panes: true,
            conns_scroll: 0,
            conns_sel: 0,
            conns_area: None,
            detail_url_rect: None,
            detail_ck_rect: None,
            detail_fail_rect: None,
            detail_save_rect: None,
            detail_val_rects: Vec::new(),
            detail_area: None,
            pointer: None,
            popup_close_rect: None,
            x_press: false,
            val_open: false,
            val_field: DetailField::Url,
            val_text: String::new(),
            val_scroll: 0,
            val_max_scroll: 0,
            val_pos: None,
            val_drag: None,
            val_area: None,
            clipboard: None,
            last_copied: None,
            visible_conns_rows: 8,
            conns_scroll_for: 0,
            conns_hscroll: 0,
            conns_max_hscroll: 0,
            list_scrollbar: None,
            conns_v_scrollbar: None,
            conns_h_scrollbar: None,
            pieces_scrollbar: None,
            help_scrollbar: None,
            val_scrollbar: None,
            pieces_max_scroll: 0,
            help_max_scroll: 0,
            sb_drag: None,
            sb_hold: None,
            sb_pressed: None,
            sb_flash: None,
            about_open: false,
            about_pos: None,
            about_drag: None,
            about_area: None,
            dialog: None,
            dlg_btn_rects: Vec::new(),
            dlg_field_rects: Vec::new(),
            dlg_field_wins: Vec::new(),
            dlg_cursor_screen: None,
            dlg_ck_rects: Vec::new(),
            dlg_proxy_rects: Vec::new(),
            dlg_area: None,
            dlg_pos: None,
            dlg_drag: None,
            pieces_open: false,
            pieces_scroll: 0,
            pieces_follow: false,
            pieces_area: None,
            pieces_pos: None,
            pieces_drag: None,
            help_open: false,
            help_scroll: 0,
            help_pos: None,
            help_drag: None,
            help_area: None,
            tab_rects: Vec::new(),
            footer_cmd_rects: Vec::new(),
            footer_press: None,
            footer_flash: None,
            proxy_options,
            proxy_labels,
            default_proxy_sel,
            pending_deletes: HashMap::new(),
            dlg_btfile_rects: Vec::new(),
            dlg_btbtn_rects: Vec::new(),
            dlg_bt_sb: None,
            dlg_bt_area: None,
            dlg_bt_visible: 0,
            dlg_trackers_sb: None,
            dlg_trackers_h_sb: None,
            dlg_trackers_area: None,
            cmd_open: false,
            cmd: crate::multiline::MultiLineBuf::new(),
            cmd_focus: 0,
            cmd_pos: None,
            cmd_drag: None,
            cmd_drag_moved: false,
            cmd_area: None,
            cmd_text_area: None,
            cmd_sb: None,
            cmd_btn_rect: None,
            cmd_cursor_screen: None,
            err_open: false,
            err_msg: String::new(),
            err_pos: None,
            err_drag: None,
            err_area: None,
            err_btn_rect: None,
            bt_conns: HashMap::new(),
            bt_conn_stats: HashMap::new(),
            bt_stopping: HashMap::new(),
        };
        if !app.cfg.warnings.is_empty() {
            let msg = app.cfg.warnings.join("；");
            app.set_toast(format!("⚠ {msg}"));
        }
        app
    }

    fn restore(path: &str) -> Vec<Task> {
        let Some(reg) = Registry::load(path) else {
            return Vec::new();
        };
        let mut tasks = reg.restore_tasks();
        for t in &mut tasks {
            let sc = Sidecar::load(&t.sidecar_path());
            if let Some(sc) = sc {
                t.downloaded = t.downloaded.max(sc.downloaded);
                t.resumable = !sc.non_resumable;
                // HTTP 分块还原：sidecar.blocks 记录逐块字节进度，
                // 满块 → 🟩 Done，零头块 → 🟨 Partial（否则重启后全白）。
                if t.done_blocks.is_empty() && !sc.blocks.is_empty() {
                    let mut done: Vec<u32> = Vec::new();
                    let mut part: Vec<u32> = Vec::new();
                    for (i, &w) in sc.blocks.iter().enumerate() {
                        let i = i as u32;
                        let full = crate::model::chunk::block_range(sc.size, sc.block_size, i)
                            .map_or(0, |(s, e)| e - s);
                        if full > 0 && w >= full {
                            done.push(i);
                        } else if w > 0 {
                            part.push(i);
                        }
                    }
                    t.chunk_done = t.chunk_done.max(done.len() as u32);
                    t.done_blocks = done;
                    t.partial_blocks = part;
                }
            }
            if t.protocol.is_bt() {
                if let Some(bt) = t.bt.clone() {
                    let bt_path = crate::model::namegen::sidecar_path_of(&t.save_dir, &t.name);
                    if let Some(pieces) = crate::model::BtSidecar::load_done(&bt_path) {
                        let pieces: Vec<u32> = pieces
                            .into_iter()
                            .filter(|p| {
                                u64::from(*p) * u64::from(bt.piece_length.max(1))
                                    < bt.files.iter().map(|f| f.size).sum::<u64>()
                            })
                            .collect();
                        t.downloaded = t.downloaded.max(bt.selected_bytes_of_pieces(&pieces));
                        t.chunk_done = pieces.len() as u32;
                        t.done_blocks = pieces;
                    }
                }
            }
        }
        tasks
    }

    #[must_use]
    pub fn alloc_task_id(&self) -> u32 {
        crate::model::idgen::alloc_task_id(|cand| self.tasks.iter().any(|t| t.id == cand))
    }

    #[must_use]
    pub fn spinner(&self) -> char {
        SPINNER[(self.frame as usize) % SPINNER.len()]
    }

    #[must_use]
    pub fn used_slots(&self) -> usize {
        slots::used(&self.tasks)
    }

    #[must_use]
    pub fn queue_pos_of(&self, task_id: u32) -> Option<usize> {
        slots::queue_pos(&self.tasks, task_id)
    }

    #[must_use]
    pub fn overlay_open(&self) -> bool {
        self.dialog.is_some()
            || self.pieces_open
            || self.help_open
            || self.about_open
            || self.val_open
            || self.cmd_open
            || self.err_open
    }

    pub fn open_about(&mut self) {
        self.about_open = true;
    }

    pub fn close_about(&mut self) {
        self.about_open = false;
        self.about_area = None;
        self.about_drag = None;
    }

    pub fn open_val_popup(&mut self, field: DetailField) {
        let Some(t) = self.sel_task() else {
            return;
        };
        let text = match field {
            DetailField::Checksum => {
                let Some(ck) = t.checksum.as_ref() else {
                    return;
                };
                ck.value.clone()
            }
            DetailField::Url => t.url.clone(),
            DetailField::Failed => t.error.clone().unwrap_or_else(|| "未知错误".to_string()),
            DetailField::SavePath => t.target_path(),
        };
        self.val_open = true;
        self.val_field = field;
        self.val_text = text;
        self.val_scroll = 0;
        self.val_pos = None;
        self.val_drag = None;
        self.val_area = None;
        self.val_scrollbar = None;
        self.focus = Panel::Detail;
    }

    pub fn close_val_popup(&mut self) {
        self.val_open = false;
        self.val_area = None;
        self.val_pos = None;
        self.val_drag = None;
        self.val_scrollbar = None;
    }

    #[must_use]
    pub fn footer_chip_hot(&self, i: usize) -> bool {
        if self.footer_press == Some(i) {
            return true;
        }
        matches!(
            self.footer_flash,
            Some((fi, until)) if fi == i && Instant::now() < until
        )
    }

    pub(crate) fn run_footer_cmd(&mut self, i: usize) {
        match i {
            0 => self.open_cmd_window(),
            1 => self.open_delete_dialog(),
            2 => self.open_modify_dialog(),
            3 => self.quit = true,
            4 => {
                self.help_open = true;
                self.help_scroll = 0;
            }
            _ => {}
        }
    }

    #[must_use]
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

    #[must_use]
    pub fn sel_idx(&self) -> Option<usize> {
        self.filtered().get(self.selected).copied()
    }

    #[must_use]
    pub fn sel_task(&self) -> Option<&Task> {
        self.sel_idx().and_then(|i| self.tasks.get(i))
    }

    pub(crate) fn set_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some(msg.into());
        self.toast_until = Some(Instant::now() + std::time::Duration::from_secs(3));
    }

    pub(super) fn push_hist(&mut self, global_dl: f64, global_ul: f64) {
        self.speed_hist.push((global_dl / 1024.0) as u64);
        if self.speed_hist.len() > 180 {
            self.speed_hist.remove(0);
        }
        self.up_hist.push((global_ul / 1024.0) as u64);
        if self.up_hist.len() > 180 {
            self.up_hist.remove(0);
        }
    }

    pub(crate) fn conn_speed_of(&self, task_id: u32, conn_id: usize) -> f64 {
        self.conn_speed
            .get(&(task_id, conn_id))
            .map_or(0.0, SmoothedSpeed::value)
    }

    pub(crate) fn bt_conn_speeds_of(&self, task_id: u32, conn_id: usize) -> (f64, f64) {
        self.bt_conn_stats
            .get(&(task_id, conn_id))
            .map_or((0.0, 0.0), |s| (s.down_ema.value(), s.up_ema.value()))
    }

    pub(crate) fn conn_cum_of(&self, task_id: u32, conn_id: usize) -> u64 {
        self.conn_cum.get(&(task_id, conn_id)).copied().unwrap_or(0)
    }

    #[must_use]
    pub fn global_dl_speed(&self) -> f64 {
        self.tasks
            .iter()
            .filter(|t| t.state == crate::model::TaskState::Downloading)
            .map(|t| t.speed)
            .sum()
    }

    #[must_use]
    pub fn global_ul_speed(&self) -> f64 {
        self.tasks
            .iter()
            .filter(|t| t.upload_speed > 0.0)
            .map(|t| t.upload_speed)
            .sum()
    }

    #[must_use]
    pub fn conns_display_total(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.state.shows_conns())
            .map(|t| self.conns_actual_of(t))
            .sum()
    }

    /// 单任务的「实际并发连接数」（并发面板行数同口径）：
    /// BT = 引擎回报的已建立 peer 连接（握手完成；connections 恒空），
    /// HTTP = 当前活跃的分块连接。
    #[must_use]
    pub fn conns_actual_of(&self, t: &Task) -> usize {
        if !t.state.shows_conns() {
            return 0;
        }
        if t.protocol.is_bt() {
            // BT 的真实并发 = 引擎回报的 peer 连接，
            // 不用 HTTP lease 快照（BT 任务 connections 恒空）。
            self.bt_conns.get(&t.id).map_or(0, Vec::len)
        } else {
            t.connections
                .iter()
                .filter(|c| self.conn_is_active(t.id, c))
                .count()
        }
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn active_conn_count(&self, t: &Task) -> usize {
        if !t.state.shows_conns() {
            return 0;
        }
        t.connections
            .iter()
            .filter(|c| self.conn_is_active(t.id, c))
            .count()
    }

    pub(crate) fn conn_is_active(&self, task_id: u32, c: &Connection) -> bool {
        let cum = self.conn_cum.get(&(task_id, c.id)).copied().unwrap_or(0);
        c.conn_phase(cum).is_active()
    }

    pub(crate) fn active_conn_ids(&self) -> Vec<usize> {
        self.sel_task()
            .filter(|t| t.state.shows_conns())
            .map(|t| {
                let mut ids: Vec<usize> = t
                    .connections
                    .iter()
                    .filter(|c| self.conn_is_active(t.id, c))
                    .map(|c| c.id)
                    .collect();
                ids.sort_unstable();
                ids
            })
            .unwrap_or_default()
    }

    #[must_use]
    pub fn tab_counts(&self) -> [usize; 2] {
        let doing = self.tasks.iter().filter(|t| !t.state.is_done()).count();
        [doing, self.tasks.len() - doing]
    }

    #[must_use]
    pub fn peak_dl_speed(&self) -> f64 {
        self.speed_hist.iter().copied().max().unwrap_or(0) as f64 * 1024.0
    }

    #[must_use]
    pub fn peak_ul_speed(&self) -> f64 {
        self.up_hist.iter().copied().max().unwrap_or(0) as f64 * 1024.0
    }

    pub(super) fn clear_conn_stats(&mut self, task_id: u32) {
        self.conn_prev.retain(|(tid, _), _| *tid != task_id);
        self.conn_cum.retain(|(tid, _), _| *tid != task_id);
        self.conn_speed.retain(|(tid, _), _| *tid != task_id);
        self.conn_windows.retain(|(tid, _), _| *tid != task_id);
    }

    pub(super) fn clear_bt_conn_stats(&mut self, task_id: u32) {
        self.bt_conn_stats.retain(|(tid, _), _| *tid != task_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskState;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-app", tag)
    }

    fn bt_snapshot_task(id: u32, name: &str, save_dir: &str) -> Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.protocol = crate::model::Protocol::Bt;
        t.name = name.to_string();
        t.url = format!("/tmp/{name}.torrent");
        t.save_dir = save_dir.to_string();
        t.state = TaskState::Queued;
        t.total = 200;
        t.block_size = 100;
        t.bt = Some(crate::model::BtTaskData {
            source: format!("/tmp/{name}.torrent"),
            trackers: vec!["udp://t/ann".to_string()],
            piece_length: 100,
            files: vec![crate::model::BtFile {
                path: name.to_string(),
                size: 200,
                selected: true,
            }],
        });
        t
    }

    #[test]
    fn restore_maps_http_sidecar_blocks_to_piece_states() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-restore-http");
        let sd = dir.to_string_lossy().into_owned();
        let mut t = crate::model::sample_task();
        t.id = 11;
        t.protocol = crate::model::Protocol::Http;
        t.name = "http.bin".to_string();
        t.save_dir = sd.clone();
        t.state = TaskState::Paused;
        t.total = 10_000;
        t.block_size = 1_000;
        t.downloaded = 3_400;
        let reg = Registry::from_tasks(&[t]);
        let reg_path = dir.join("registry.json");
        reg.save(reg_path.to_str().unwrap()).unwrap();
        let task = crate::model::sidecar::SidecarTask {
            id: 11,
            added_at: 0,
            save_dir: sd.clone(),
            concurrency: 4,
            protocol: crate::model::Protocol::Http,
        };
        let stamp = crate::model::consistency::ServerStamp::default();
        let sc = crate::model::sidecar::Sidecar::build(
            &stamp,
            10_000,
            1_000,
            &[1_000, 1_000, 1_000, 400, 0, 0, 0, 0, 0, 0],
            false,
            None,
            task,
        );
        let sidecar_path = {
            let mut t = crate::model::sample_task();
            t.id = 11;
            t.name = "http.bin".to_string();
            t.save_dir = sd.clone();
            t.sidecar_path()
        };
        sc.save(&sidecar_path).unwrap();
        let tasks = App::restore(reg_path.to_str().unwrap());
        assert_eq!(tasks.len(), 1);
        let t = &tasks[0];
        assert_eq!(
            t.done_blocks,
            vec![0, 1, 2],
            "满块 → 🟩 Done（重启后 HTTP 分块不再全白）"
        );
        assert_eq!(t.partial_blocks, vec![3], "零头块 → 🟨 Partial");
        assert_eq!(t.chunk_done, 3);
        assert_eq!(t.downloaded, 3_400);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn restore_populates_bt_piece_state_from_sidecar() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-restore-bt");
        let sd = dir.to_string_lossy().into_owned();
        let mut t = bt_snapshot_task(7, "tor.bin", &sd);
        t.downloaded = 50;
        let reg = Registry::from_tasks(&[t]);
        let reg_path = dir.join("registry.json");
        reg.save(reg_path.to_str().unwrap()).unwrap();
        crate::model::BtSidecar::build([1, 0])
            .save(&crate::model::namegen::sidecar_path_of(&sd, "tor.bin"))
            .unwrap();
        let tasks = App::restore(reg_path.to_str().unwrap());
        assert_eq!(tasks.len(), 1);
        let t = &tasks[0];
        assert_eq!(
            t.done_blocks,
            vec![0, 1],
            "重启后 pieces 窗口数据直接来自 .ezr 断点（含乱序存储的 piece）"
        );
        assert_eq!(t.chunk_done, 2);
        assert_eq!(
            t.downloaded, 200,
            "downloaded = sidecar piece 覆盖的选中字节（2×100），不小于快照值"
        );
        assert!(t.done_blocks.iter().all(|&p| p < 2), "无越界 piece 混入");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn restore_bt_without_sidecar_keeps_snapshot_values() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-restore-bt-nosc");
        let sd = dir.to_string_lossy().into_owned();
        let mut t = bt_snapshot_task(8, "nsc.bin", &sd);
        t.downloaded = 40;
        let reg = Registry::from_tasks(&[t]);
        let reg_path = dir.join("registry.json");
        reg.save(reg_path.to_str().unwrap()).unwrap();
        let tasks = App::restore(reg_path.to_str().unwrap());
        assert!(tasks[0].done_blocks.is_empty());
        assert_eq!(tasks[0].chunk_done, 0);
        assert_eq!(tasks[0].downloaded, 40);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn restore_bt_filters_stale_pieces_beyond_torrent() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-restore-bt-stale");
        let sd = dir.to_string_lossy().into_owned();
        let t = bt_snapshot_task(9, "stale.bin", &sd);
        let reg = Registry::from_tasks(&[t]);
        let reg_path = dir.join("registry.json");
        reg.save(reg_path.to_str().unwrap()).unwrap();
        let _ = crate::model::BtSidecar::build([0, 5, 9])
            .save(&crate::model::namegen::sidecar_path_of(&sd, "stale.bin"));
        let tasks = App::restore(reg_path.to_str().unwrap());
        assert_eq!(
            tasks[0].done_blocks,
            vec![0],
            "piece 5/9 超出 2-piece torrent（200B/100B）被过滤，不计进度"
        );
        assert_eq!(tasks[0].chunk_done, 1);
        assert_eq!(tasks[0].downloaded, 100);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn task(id: u32, name: &str, state: TaskState) -> Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t.state = state;
        t
    }

    #[tokio::test]
    async fn tab_counts_split_by_done_state() {
        let mut app = make_app("tab");
        assert_eq!(app.tab_counts(), [0, 0], "空表双零");
        app.tasks.push(task(1, "a.bin", TaskState::Downloading));
        app.tasks.push(task(2, "b.bin", TaskState::Queued));
        app.tasks.push(task(3, "c.bin", TaskState::Paused));
        app.tasks.push(task(4, "d.bin", TaskState::Completed));
        app.tasks.push(task(5, "e.bin", TaskState::Seeding));
        let counts = app.tab_counts();
        assert_eq!(
            counts[0], 3,
            "未完成态 = 下载中/等待/暂停（含挂起，v1.23 数据同普通暂停）"
        );
        assert_eq!(counts[1], 2, "is_done = 已完成/做种");
        assert_eq!(counts[0] + counts[1], app.tasks.len(), "两页签不重不漏");
        app.tasks[2].state = TaskState::Completed;
        assert_eq!(app.tab_counts(), [2, 3]);
        app.shutdown().await;
    }

    #[tokio::test]
    async fn peak_speeds_max_of_history_in_bytes() {
        let mut app = make_app("peak");
        assert_eq!(app.peak_dl_speed(), 0.0, "零历史峰值 0");
        assert_eq!(app.peak_ul_speed(), 0.0, "上传历史恒 0（01 期无 BT）");
        app.speed_hist = vec![0, 12, 340, 5];
        app.up_hist = vec![0, 0, 0];
        assert_eq!(
            app.peak_dl_speed(),
            340.0 * 1024.0,
            "最大采样 ×1024 还原 B/s"
        );
        assert_eq!(app.peak_ul_speed(), 0.0);
        app.speed_hist = vec![1, 2];
        assert_eq!(app.peak_dl_speed(), 2.0 * 1024.0);
        app.shutdown().await;
    }
}

#[cfg(test)]
mod interaction_tests {

    use crossterm::event::{KeyCode, KeyModifiers};

    use super::*;
    use crate::app::testutil::dl_task;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-sb", tag)
    }

    #[tokio::test]
    async fn conns_horizontal_keys_clamp_to_zero_for_http() {
        let mut app = make_app("hkey");
        app.tasks.push(dl_task(1, "h.bin"));
        app.selected = 0;
        app.focus = Panel::Conns;
        app.conns_max_hscroll = 0;
        app.on_key(KeyCode::Right, KeyModifiers::NONE);
        app.on_key(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(app.conns_hscroll, 0, "←/→ 无横向内容不响应（钳制归零）");
        app.on_key(KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(app.conns_hscroll, 0);
        app.focus = Panel::List;
        app.conns_max_hscroll = 10;
        app.on_key(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(app.conns_hscroll, 0, "列表聚焦时 ←→ 不滚并发明细");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn about_modal_lifecycle_and_priority() {
        let mut app = make_app("about");
        app.on_key(KeyCode::Char('o'), KeyModifiers::NONE);
        assert!(app.about_open);
        assert!(app.overlay_open(), "浮层集合含关于窗口（FR-01-108 注记）");
        app.about_pos = Some((7, 9));
        app.close_about();
        assert!(!app.about_open && !app.overlay_open());
        app.open_about();
        assert_eq!(app.about_pos, Some((7, 9)), "位置跨次记忆");
        app.on_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "About 模态消费 A");
        app.on_key(KeyCode::Char('h'), KeyModifiers::NONE);
        assert!(!app.help_open, "About 模态消费 H");
        app.on_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.focus, Panel::List, "About 模态消费 Tab");
        app.on_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(!app.about_open && !app.quit, "q 先关窗不退出");
        app.help_open = true;
        app.on_key(KeyCode::Char('O'), KeyModifiers::NONE);
        assert!(app.about_open && app.help_open, "帮助内打开 About");
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.about_open && app.help_open, "关闭后回帮助");
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.help_open);
        app.shutdown().await;
    }
}
