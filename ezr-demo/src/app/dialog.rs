use crossterm::event::KeyCode;

use crate::multiline::MultiLineBuf;
use crate::textfield::TextInput;

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DialogKind {
    Add,

    Delete,

    Modify,

    /// 添加任务：BT（选择本地 torrent 文件 / 磁力链接 + 勾选要下载的内部文件）
    AddBt,

    /// 修改任务：BT（重新勾选内部文件 + 并发/代理热调）
    ModifyBt,
}

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

    /// （仅 AddBt）种子来源：本地 .torrent 路径或磁力链接
    pub torrent: TextInput,

    /// （仅 AddBt/ModifyBt）BT 内部文件树（目录三态 + 展开收拢，可勾选）
    pub bt_tree: Vec<BtNode>,

    /// （仅 AddBt/ModifyBt）文件树光标（可见行 0 基）
    pub bt_cursor: usize,

    /// （仅 AddBt/ModifyBt）文件树视口起始（可见行）
    pub bt_scroll: usize,

    /// （仅 AddBt/ModifyBt）视口跟随开关：光标移动/树结构变化时置 true
    /// （渲染层消费一次后复位），滚动条/滚轮滚动时置 false——否则每帧强制
    /// 回贴光标会让滚动条"失效"（尤其光标停在最后一行时完全无法上滚）
    pub bt_follow: bool,

    /// （仅 AddBt/ModifyBt）trackers 多行输入（可复用 MultiLineBuf 编辑模型：
    /// 与命令窗口文本框同构；Alt+Enter / kitty Shift+Enter 换行，Enter = 确认）
    pub trackers: MultiLineBuf,
}

impl Dialog {
    pub fn text_field_at(&self, focus: usize) -> Option<TextField> {
        match (self.kind, focus) {
            (DialogKind::Add, 0) => Some(TextField::Url),
            (DialogKind::Add, 1) => Some(TextField::Dir),
            (DialogKind::Add, 2) | (DialogKind::Modify, 0) => Some(TextField::Conns),
            (DialogKind::Add, 4) | (DialogKind::Modify, 2) => Some(TextField::CkValue),
            (DialogKind::AddBt, 0) => Some(TextField::Torrent),
            (DialogKind::AddBt, 1) => Some(TextField::Dir),
            (DialogKind::AddBt, 3) | (DialogKind::ModifyBt, 1) => Some(TextField::Conns),
            _ => None,
        }
    }

    pub fn field_mut(&mut self, f: TextField) -> &mut TextInput {
        match f {
            TextField::Url => &mut self.url,
            TextField::Dir => &mut self.dir,
            TextField::Conns => &mut self.conns,
            TextField::CkValue => &mut self.ck_value,
            TextField::Torrent => &mut self.torrent,
        }
    }

    /// （仅 AddBt/ModifyBt）trackers 多行输入在对话框内的 focus 编号
    pub fn trackers_focus(&self) -> Option<usize> {
        match self.kind {
            DialogKind::AddBt => Some(5),
            DialogKind::ModifyBt => Some(3),
            _ => None,
        }
    }

    pub fn focus_field(&mut self, f: usize) {
        self.focus = f;
        if self.trackers_focus() == Some(f) {
            // 多行输入：光标迁至当前行尾 + 拉回视口跟随
            self.trackers.line_mut().cursor_end();
            self.trackers.follow = true;
        }
        if let Some(t) = self.text_field_at(f) {
            self.field_mut(t).cursor_end();
        }
    }

    /// 当前可见行（收拢目录跳过子树）：(索引路径, 节点引用)
    pub fn bt_rows(&self) -> Vec<(Vec<usize>, &BtNode)> {
        let mut out = Vec::new();
        let mut prefix = Vec::new();
        bt_visible_rows(&self.bt_tree, &mut prefix, &mut out);
        out
    }

    /// 当前展开状态下的可见行数
    pub fn bt_row_count(&self) -> usize {
        bt_row_count(&self.bt_tree)
    }

    /// （仅 AddBt/ModifyBt）文件列表光标移动 + 视口钳制
    pub(super) fn bt_list_move(&mut self, delta: i32) {
        self.bt_follow = true;
        let n = self.bt_row_count() as i32;
        if n <= 0 {
            self.bt_cursor = 0;
            return;
        }
        self.bt_cursor = (self.bt_cursor as i32 + delta).clamp(0, n - 1) as usize;
    }

    /// （仅 AddBt/ModifyBt）确保光标行在视口内（渲染前调用）
    pub(crate) fn bt_scroll_follow(&mut self, visible: usize) {
        let vis = visible.max(1);
        let n = self.bt_row_count();
        if self.bt_cursor < self.bt_scroll {
            self.bt_scroll = self.bt_cursor;
        }
        if self.bt_cursor >= self.bt_scroll + vis {
            self.bt_scroll = self.bt_cursor + 1 - vis;
        }
        let max = n.saturating_sub(vis);
        self.bt_scroll = self.bt_scroll.min(max);
    }

    fn bt_path_at_cursor(&self) -> Option<Vec<usize>> {
        self.bt_rows().get(self.bt_cursor).map(|(p, _)| p.clone())
    }

    /// 空格/点击复选框：切换光标行勾选（文件翻转；目录 全选⇄全不选，部分 → 全选）
    pub(super) fn bt_toggle_cursor(&mut self) {
        if let Some(path) = self.bt_path_at_cursor() {
            bt_toggle_node(&mut self.bt_tree, &path);
            bt_sync_dir_states(&mut self.bt_tree);
        }
    }

    /// 点击目录行（非复选框）：切换展开/收拢；收拢后钳制光标
    pub(super) fn bt_toggle_expand_at_cursor(&mut self) {
        let Some(path) = self.bt_path_at_cursor() else {
            return;
        };
        let expanded = self
            .bt_rows()
            .get(self.bt_cursor)
            .map(|(_, n)| n.expanded)
            .unwrap_or(false);
        bt_set_expanded(&mut self.bt_tree, &path, !expanded);
        self.bt_cursor = self.bt_cursor.min(self.bt_row_count().saturating_sub(1));
        // 树结构变化：重新拉回视口跟随，保证光标行可见
        self.bt_follow = true;
    }

    /// ←/→：收拢 / 展开光标目录（文件行无操作）
    pub(super) fn bt_expand_cursor(&mut self, expand: bool) {
        if let Some(path) = self.bt_path_at_cursor() {
            bt_set_expanded(&mut self.bt_tree, &path, expand);
            self.bt_cursor = self.bt_cursor.min(self.bt_row_count().saturating_sub(1));
            // 树结构变化：重新拉回视口跟随
            self.bt_follow = true;
        }
    }

    pub(super) fn bt_select_all(&mut self, on: bool) {
        bt_set_all(&mut self.bt_tree, on);
        bt_sync_dir_states(&mut self.bt_tree);
    }

    pub(super) fn bt_invert(&mut self) {
        bt_invert_all(&mut self.bt_tree);
        bt_sync_dir_states(&mut self.bt_tree);
    }

    /// （仅 AddBt/ModifyBt）当前（已选文件数, 文件总数）——目录不计数
    pub fn bt_counts(&self) -> (usize, usize) {
        bt_count_files(&self.bt_tree)
    }

    /// （仅 AddBt/ModifyBt）已选文件字节合计（收拢目录下的已选文件同样计入）
    pub fn bt_selected_bytes(&self) -> u64 {
        bt_selected_bytes(&self.bt_tree)
    }

    /// 确认时摊平为扁平文件列表（保持插入顺序）
    pub(super) fn bt_flatten(&self) -> Vec<BtFile> {
        let mut out = Vec::new();
        bt_tree_files(&self.bt_tree, &mut out);
        out
    }

    /// （仅 AddBt/ModifyBt）文件列表焦点在 BT 对话框内的 focus 编号
    pub(super) fn bt_list_focus(kind: DialogKind) -> usize {
        match kind {
            DialogKind::AddBt => 2,
            DialogKind::ModifyBt => 0,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextField {
    Url,

    Dir,

    Conns,

    CkValue,

    Torrent,
}

pub(super) fn proxy_dropdown_open_key(d: &mut Dialog, n: usize, code: KeyCode) {
    let n = n.max(1);
    match code {
        KeyCode::Up => {
            d.proxy_sel = (d.proxy_sel + n - 1) % n;
        }
        KeyCode::Down => {
            d.proxy_sel = (d.proxy_sel + 1) % n;
        }
        KeyCode::Home => d.proxy_sel = 0,
        KeyCode::End => d.proxy_sel = n - 1,
        KeyCode::Enter | KeyCode::Esc => d.proxy_open = false,
        KeyCode::Tab => {
            d.proxy_open = false;
            d.focus_field(if d.kind == DialogKind::Add { 6 } else { 4 });
        }
        KeyCode::BackTab => {
            d.proxy_open = false;
            d.focus_field(if d.kind == DialogKind::Add { 4 } else { 2 });
        }
        _ => d.proxy_open = false,
    }
}

pub(super) fn ck_dropdown_open_key(d: &mut Dialog, code: KeyCode) {
    match code {
        KeyCode::Up => {
            d.ck_sel = (d.ck_sel + CHECKSUM_ALGOS.len() - 1) % CHECKSUM_ALGOS.len();
            d.ck_type = d.ck_sel;
            if d.ck_type == CK_NONE {
                d.ck_value.clear();
            }
        }
        KeyCode::Down => {
            d.ck_sel = (d.ck_sel + 1) % CHECKSUM_ALGOS.len();
            d.ck_type = d.ck_sel;
            if d.ck_type == CK_NONE {
                d.ck_value.clear();
            }
        }
        KeyCode::Home => {
            d.ck_sel = 0;
            d.ck_type = 0;
            d.ck_value.clear();
        }
        KeyCode::End => {
            d.ck_sel = CHECKSUM_ALGOS.len() - 1;
            d.ck_type = d.ck_sel;
        }
        KeyCode::Enter | KeyCode::Esc => d.ck_open = false,
        KeyCode::Tab | KeyCode::BackTab => {
            d.ck_open = false;
            d.focus_field(if code == KeyCode::Tab { 4 } else { 2 });
        }
        _ => d.ck_open = false,
    }
}

pub(super) fn text_backspace(kind: DialogKind, d: &mut Dialog, focus: usize) -> bool {
    match (kind, focus) {
        (DialogKind::Add, 0) => {
            d.url.backspace();
            true
        }
        (DialogKind::Add, 1) => {
            d.dir.backspace();
            true
        }
        (DialogKind::Add, 2)
        | (DialogKind::Modify, 0)
        | (DialogKind::AddBt, 3)
        | (DialogKind::ModifyBt, 1) => {
            d.conns.backspace();
            d.conns_edited = true;
            true
        }
        (DialogKind::Add, 4) | (DialogKind::Modify, 2) => {
            d.ck_value.backspace();
            true
        }
        (DialogKind::AddBt, 0) => {
            d.torrent.backspace();
            true
        }
        (DialogKind::AddBt, 1) => {
            d.dir.backspace();
            true
        }
        // trackers 多行输入：退格与上一行合并由 MultiLineBuf 处理
        (DialogKind::AddBt, 5) | (DialogKind::ModifyBt, 3) => {
            d.trackers.backspace();
            true
        }
        _ => false,
    }
}

pub(super) fn paste_sanitize(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

pub(super) fn dlg_apply_paste(kind: DialogKind, d: &mut Dialog, focus: usize, text: &str) -> bool {
    let clean = paste_sanitize(text);
    match (kind, focus) {
        (DialogKind::Add, 0) => d.url.insert(&clean, 300) > 0,
        (DialogKind::Add, 1) => d.dir.insert(&clean, 300) > 0,
        (DialogKind::Add, 2)
        | (DialogKind::Modify, 0)
        | (DialogKind::AddBt, 3)
        | (DialogKind::ModifyBt, 1) => {
            let digits: String = clean.chars().filter(|c| c.is_ascii_digit()).collect();
            let inserted = d.conns.insert(&digits, 2) > 0;
            d.conns_edited |= inserted;
            inserted
        }
        (DialogKind::Add, 4) | (DialogKind::Modify, 2) => {
            if d.ck_type == CK_NONE {
                return false;
            }
            let hex: String = clean.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            d.ck_value.insert(&hex, 128) > 0
        }
        (DialogKind::AddBt, 0) => d.torrent.insert(&clean, 300) > 0,
        (DialogKind::AddBt, 1) => d.dir.insert(&clean, 300) > 0,
        // trackers 多行粘贴不在此处理（需保留换行分段）：on_paste 直通
        // MultiLineBuf::insert_str（粘贴 sanitize 在行内过滤控制字符）
        _ => false,
    }
}
