use super::*;

impl App {
    pub fn open_pieces(&mut self) {
        let Some(t) = self.sel_task() else {
            self.set_toast("未选中任务，无法查看分块");
            return;
        };
        if !t.supports_pieces() {
            self.set_toast("该任务不支持分块下载（服务器不支持断点续传），无 pieces 信息");
            return;
        }
        if t.total == 0 {
            self.set_toast("任务内容为空，无分块信息");
            return;
        }
        self.pieces_open = true;
        self.pieces_follow = true;
        self.pieces_scroll = 0;
        self.pieces_pos = None;
        self.pieces_drag = None;
    }

    pub fn close_pieces(&mut self) {
        self.pieces_open = false;
        self.pieces_area = None;
        self.pieces_pos = None;
        self.pieces_drag = None;
    }

    pub(super) fn pieces_scroll_by(&mut self, delta: i32) {
        if delta < 0 {
            self.pieces_scroll = self
                .pieces_scroll
                .saturating_sub(delta.unsigned_abs() as usize);
        } else {
            self.pieces_scroll = self.pieces_scroll.saturating_add(delta as usize);
        }
    }

    pub fn open_val_popup(&mut self, field: DetailField) {
        let Some(t) = self.sel_task() else {
            return;
        };
        let text = match field {
            DetailField::Checksum => {
                let Some(ck) = &t.checksum else { return };
                ck.value.clone()
            }
            DetailField::Url => t.url.clone(),
            DetailField::Failed => t.error.clone().unwrap_or_else(|| "未知错误".to_string()),
            DetailField::SavePath => t.save_path.clone(),
        };
        self.val_open = true;
        self.val_field = field;
        self.val_title = field.title().to_string();
        self.val_text = text;
        self.val_scroll = 0;
        self.val_pos = None;
        self.val_drag = None;
    }

    pub fn close_val_popup(&mut self) {
        self.val_open = false;
        self.val_area = None;
        self.val_pos = None;
        self.val_drag = None;
        self.val_sb = None;
    }

    pub(super) fn val_scroll_by(&mut self, delta: i32) {
        if delta < 0 {
            self.val_scroll = self
                .val_scroll
                .saturating_sub(delta.unsigned_abs() as usize);
        } else {
            self.val_scroll = self.val_scroll.saturating_add(delta as usize);
        }
    }
}
