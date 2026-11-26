use std::collections::HashMap;
use std::time::Instant;

use ratatui::layout::Rect;

use crate::engine::{EngineHandle, Evt};
use crate::model::config::Config;
#[cfg(test)]
use crate::model::registry::Registry;
use crate::model::speed::{SmoothedSpeed, SpeedWindow};
use crate::model::{slots, Task};
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
mod popups;
mod reload;
mod restore;
mod scrollbar;
mod stats;
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
#[cfg(test)]
pub(crate) use dialogs::{fmt_speed_limit, parse_speed_limit};

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

#[derive(Clone, Copy)]
pub struct TabListState {
    pub selected_task: Option<u32>,
    pub selected: usize,
    pub scroll: usize,
}

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
    pub down_limit: TextInput,
    pub up_limit: TextInput,
    pub down_prefill: u64,
    pub up_prefill: u64,
    pub ck_type: usize,
    pub ck_value: TextInput,
    pub ck_open: bool,
    pub ck_sel: usize,
    pub proxy_sel: usize,
    pub proxy_open: bool,
    pub sequential: bool,
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
    pub tab_states: Vec<TabListState>,
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
    pub trackers_open: bool,
    pub trackers_scroll: usize,
    pub trackers_max_scroll: usize,
    pub trackers_sb: Option<SbBar>,
    pub trackers_area: Option<Rect>,
    pub trackers_pos: Option<(u16, u16)>,
    pub trackers_drag: Option<(u16, u16)>,
    pub trackers_status: HashMap<u32, Vec<TrackerRow>>,
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
    /// 启动期 CLI `--max-speed` 覆盖值（FR-01-119⑥）：每次热加载后重应用，
    /// CLI > config 文件优先级贯穿运行期。
    pub(crate) cli_max_speed_override: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BtConnRow {
    pub id: usize,
    pub addr: String,
    pub protocol: String,
    pub down_speed: f64,
    pub cum_down: u64,
    pub up_speed: f64,
    pub cum_up: u64,
    pub client: String,
    pub seeder: bool,
    pub flags: String,
    /// 相关度（FR-02-36：qBt Peers Relevance 同源，1 位小数百分数显示）。
    pub relevance: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrackerRow {
    pub url: String,
    pub working: bool,
    pub peers: Option<u32>,
    pub seeds: Option<u32>,
    pub next_at: Option<Instant>,
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
            tab_states: vec![
                TabListState {
                    selected_task: None,
                    selected: 0,
                    scroll: 0,
                };
                FILTERS.len()
            ],
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
            trackers_open: false,
            trackers_scroll: 0,
            trackers_max_scroll: 0,
            trackers_sb: None,
            trackers_area: None,
            trackers_pos: None,
            trackers_drag: None,
            trackers_status: HashMap::new(),
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
            cli_max_speed_override: None,
        };
        if !app.cfg.warnings.is_empty() {
            let msg = app.cfg.warnings.join("；");
            app.set_toast(format!("⚠ {msg}"));
        }
        app
    }

    #[must_use]
    pub fn alloc_task_id(&self) -> u32 {
        crate::model::idgen::alloc_task_id(|cand| self.tasks.iter().any(|t| t.id == cand))
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
            || self.trackers_open
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

    pub(crate) fn set_filter(&mut self, idx: usize) {
        if idx >= FILTERS.len() || idx == self.filter {
            return;
        }
        self.tab_states[self.filter] = TabListState {
            selected_task: self.sel_task().map(|t| t.id),
            selected: self.selected,
            scroll: self.scroll,
        };
        self.filter = idx;
        let saved = self.tab_states[idx];
        self.selected = saved.selected;
        self.scroll = saved.scroll;
        let idxs = self.filtered();
        if idxs.is_empty() {
            self.selected = 0;
            self.scroll = 0;
            self.last_selected = 0;
            return;
        }
        if let Some(tid) = saved.selected_task {
            if let Some(pos) = idxs.iter().position(|&ti| self.tasks[ti].id == tid) {
                self.selected = pos;
            }
        }
        self.selected = self.selected.min(idxs.len() - 1);
        self.last_selected = self.selected;
    }

    pub(crate) fn set_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some(msg.into());
        self.toast_until = Some(Instant::now() + std::time::Duration::from_secs(3));
    }

    /// 登记启动期 CLI `--max-speed` 覆盖（main.rs 启动链调用一次），
    /// 使每次热加载后覆盖被重应用（FR-01-119⑥）。
    pub fn set_cli_max_speed_override(&mut self, speed_bps: u64) {
        self.cli_max_speed_override = Some(speed_bps);
        self.cfg.global_max_download_speed = speed_bps;
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
            upload_paused: false,
            sequential: false,
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

    // ---- 判重错误窗口（bug25 轮 cleaner 补测，纯追加）----

    /// 判重命中场景的共享 setup：既有任务 URL 与对话框 URL 同为 a.bin，
    /// 既有任务目录固定，对话框目录为参数（同目录/异目录两臂的差分点）。
    fn dup_hit_app(tag: &str, dialog_dir: &str) -> App {
        let mut app = make_app(tag);
        let mut t = crate::app::testutil::dl_task(1, "a.bin");
        t.url = "http://example.com/a.bin".to_string();
        t.save_dir = crate::model::config::normalize_save_dir("ezr-dup-err-dir");
        app.tasks.push(t);
        let mut d = crate::app::testutil::dialog(crate::app::DialogKind::Add, 0);
        d.url.set("http://example.com/a.bin");
        d.dir.set(dialog_dir);
        d.ck_type = crate::model::checksum::CK_NONE;
        app.dialog = Some(d);
        app
    }

    #[tokio::test]
    async fn finish_add_duplicate_opens_error_window() {
        let mut app = dup_hit_app("dup-err", "ezr-dup-err-dir");
        app.dlg_confirm_add();
        assert!(
            app.err_open && app.err_msg == "该链接的任务已存在",
            "HTTP 判重命中 → 错误窗口（D56 第三臂），err={:?}",
            app.err_msg
        );
        assert_eq!(app.tasks.len(), 1, "判重命中不新增任务");
        assert!(app.dialog.is_some(), "对话框保持打开（规格口径）");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn finish_add_different_dir_is_duplicate() {
        let mut app = dup_hit_app("dup-dir", "ezr-dup-err-dir-other");
        app.dlg_confirm_add();
        assert!(
            app.err_open && app.err_msg == "该链接的任务已存在",
            "同 URL 异目录也判重（bug27：任务按 URL 唯一，与保存目录无关），err={:?}",
            app.err_msg
        );
        assert_eq!(app.tasks.len(), 1, "判重命中不新增任务");
        assert!(app.dialog.is_some(), "对话框保持打开（规格口径）");
        app.shutdown().await;
    }

    #[test]
    fn detail_field_labels_cover_all_variants() {
        use DetailField::{Checksum, Failed, SavePath, Url};
        assert_eq!(Checksum.title(), "校验");
        assert_eq!(Url.title(), "URL");
        assert_eq!(Failed.title(), "失败");
        assert_eq!(SavePath.title(), "路径");
        assert_eq!(Checksum.copy_label(), "校验码");
        assert_eq!(Url.copy_label(), "url");
        assert_eq!(Failed.copy_label(), "失败原因");
        assert_eq!(SavePath.copy_label(), "保存路径");
    }

    #[tokio::test]
    async fn open_val_popup_field_arms_and_guards() {
        let mut app = make_app("valarms");
        app.open_val_popup(DetailField::Url);
        assert!(!app.val_open, "无任务打开值弹窗是 no-op");

        let mut t = crate::app::testutil::dl_task(21, "v.bin");
        t.error = Some("磁盘已满".to_string());
        app.tasks.push(t);
        app.selected = 0;

        app.open_val_popup(DetailField::Failed);
        assert!(app.val_open, "失败原因臂打开弹窗");
        assert_eq!(app.val_text, "磁盘已满", "Failed 臂取任务错误");
        assert_eq!(app.focus, Panel::Detail, "打开后焦点进明细面板");
        app.close_val_popup();

        let save_path = app.tasks[0].target_path();
        app.open_val_popup(DetailField::SavePath);
        assert!(app.val_open, "保存路径臂打开弹窗");
        assert_eq!(app.val_text, save_path, "SavePath 臂取目标路径");
        app.close_val_popup();

        app.open_val_popup(DetailField::Checksum);
        assert!(!app.val_open, "无校验和任务 Checksum 臂守卫返回");

        let mut ck = crate::app::testutil::dl_task(22, "c.bin");
        ck.checksum = Some(crate::model::Checksum {
            algo: crate::model::checksum::CHECKSUM_ALGOS[0].0,
            value: "abcd".to_string(),
        });
        app.tasks.push(ck);
        app.selected = 1;
        app.open_val_popup(DetailField::Checksum);
        assert!(app.val_open, "有校验和 Checksum 臂打开弹窗");
        assert_eq!(app.val_text, "abcd", "Checksum 臂取校验值");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn run_footer_cmd_delete_modify_quit_help_arms() {
        let mut app = make_app("footcmd");
        app.tasks.push(crate::app::testutil::dl_task(31, "f.bin"));
        app.selected = 0;

        app.run_footer_cmd(1);
        assert!(
            matches!(
                app.dialog.as_ref().map(|d| d.kind),
                Some(DialogKind::Delete)
            ),
            "页脚 1 打开删除对话框"
        );
        app.dialog = None;

        app.run_footer_cmd(2);
        assert!(
            matches!(
                app.dialog.as_ref().map(|d| d.kind),
                Some(DialogKind::Modify)
            ),
            "页脚 2 打开修改对话框"
        );
        app.dialog = None;

        app.run_footer_cmd(3);
        assert!(app.quit, "页脚 3 退出");

        app.run_footer_cmd(4);
        assert!(
            app.help_open && app.help_scroll == 0,
            "页脚 4 打开帮助并复位滚动"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn global_ul_speed_sums_only_uploading_tasks() {
        let mut app = make_app("ulspeed");
        let mut a = crate::app::testutil::dl_task(41, "a.bin");
        a.upload_speed = 100.0;
        let mut b = crate::app::testutil::dl_task(42, "b.bin");
        b.upload_speed = 50.5;
        let c = crate::app::testutil::dl_task(43, "c.bin");
        app.tasks.extend([a, b, c]);
        assert!(
            (app.global_ul_speed() - 150.5).abs() < 1e-9,
            "全局上传速度只累加上传中任务"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn app_new_surfaces_config_warnings_as_startup_toast() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-warn-toast");
        let cfg = crate::model::config::Config {
            warnings: vec!["未知键 foo 已忽略".to_string()],
            ..crate::model::config::Config::default()
        };
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        let mut app = App::new(cfg, reg);
        let toast = app.toast.clone().expect("启动期配置警告应以 toast 呈现");
        assert!(
            toast.contains("⚠") && toast.contains("未知键 foo 已忽略"),
            "toast 汇总配置警告: {toast}"
        );
        app.shutdown().await;
    }
}

#[cfg(test)]
mod hardening_app_core {
    use super::*;
    use crate::model::TaskState;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-ah", tag)
    }

    fn mk(id: u32, name: &str, state: TaskState) -> crate::model::Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t.state = state;
        t
    }

    #[tokio::test]
    async fn used_slots_counts_downloading_failed_retry_and_slotted_queue() {
        let mut app = make_app("ahus");
        let mut failed = mk(3, "f.bin", TaskState::Failed);
        failed.retry_in = Some(8.0);
        let mut slotted = mk(4, "q.bin", TaskState::Queued);
        slotted.has_slot = true;
        app.tasks = vec![
            mk(1, "d.bin", TaskState::Downloading),
            mk(2, "w.bin", TaskState::Queued),
            failed,
            slotted,
        ];
        assert_eq!(
            app.used_slots(),
            3,
            "占用 = 下载中(1) + 失败待自动重试(1) + 持槽排队(1)；无槽排队不计"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn queue_pos_counts_undecorated_waiting_tasks() {
        let mut app = make_app("ahqp");
        let mut slotted = mk(4, "q0.bin", TaskState::Queued);
        slotted.has_slot = true;
        app.tasks = vec![
            mk(1, "d.bin", TaskState::Downloading),
            mk(2, "q1.bin", TaskState::Queued),
            mk(3, "q2.bin", TaskState::Queued),
            slotted,
        ];
        assert_eq!(app.queue_pos_of(2), Some(1), "第一顺位等待");
        assert_eq!(app.queue_pos_of(3), Some(2), "第二顺位等待");
        assert_eq!(app.queue_pos_of(1), None, "下载中任务无排队位");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn overlay_open_true_for_each_single_overlay() {
        let mut app = make_app("ahov");
        assert!(!app.overlay_open());
        app.pieces_open = true;
        assert!(app.overlay_open(), "仅 pieces 开 → 谓词为真");
        app.pieces_open = false;
        app.trackers_open = true;
        assert!(app.overlay_open(), "仅 trackers 开 → 谓词为真");
        app.trackers_open = false;
        app.about_open = true;
        assert!(app.overlay_open(), "仅 about 开 → 谓词为真");
        app.about_open = false;
        app.cmd_open = true;
        assert!(app.overlay_open(), "仅 cmd 开 → 谓词为真");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn set_filter_ignores_out_of_range_index() {
        let mut app = make_app("ahsf");
        app.tasks = vec![mk(1, "d.bin", TaskState::Downloading)];
        app.set_filter(2);
        assert_eq!(app.filter, 0, "越界索引必须被卫语句拒绝");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn set_toast_window_lies_in_future() {
        let mut app = make_app("ahst");
        app.set_toast("x");
        let until = app.toast_until.expect("toast 窗口必须存在");
        assert!(until > Instant::now(), "toast 存活窗必须落在未来（now+3s）");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn cli_max_speed_override_registers_and_applies() {
        let mut app = make_app("ahcli");
        app.set_cli_max_speed_override(12345);
        assert_eq!(app.cli_max_speed_override, Some(12345));
        assert_eq!(app.cfg.global_max_download_speed, 12345);
        app.shutdown().await;
    }

    #[tokio::test]
    async fn push_hist_appends_scaled_samples_and_caps_at_180() {
        let mut app = make_app("ahph");
        app.speed_hist = vec![0; 179];
        app.up_hist = vec![0; 179];
        app.push_hist(1024.0, 2048.0);
        assert_eq!(app.speed_hist.len(), 180, "首推不触顶");
        assert_eq!(app.speed_hist.last(), Some(&1), "1024/1024 = 1 KB/s");
        assert_eq!(app.up_hist.len(), 180, "首推不触顶（上行）");
        assert_eq!(app.up_hist.last(), Some(&2), "2048/1024 = 2 KB/s");
        app.push_hist(0.0, 0.0);
        assert_eq!(app.speed_hist.len(), 180, "触顶后淘汰最旧样本");
        assert_eq!(app.up_hist.len(), 180, "触顶后淘汰最旧样本（上行）");
        app.shutdown().await;
    }
}
