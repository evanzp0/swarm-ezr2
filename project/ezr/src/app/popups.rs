use super::App;

impl App {
    pub(crate) fn trackers_scroll_by(&mut self, delta: i32) {
        if delta < 0 {
            self.trackers_scroll = self
                .trackers_scroll
                .saturating_sub(delta.unsigned_abs() as usize);
        } else {
            self.trackers_scroll = self.trackers_scroll.saturating_add(delta as usize);
        }
    }

    pub fn open_trackers_popup(&mut self) {
        let Some(t) = self.sel_task() else {
            self.set_toast("未选中任务，无法查看 trackers");
            return;
        };
        if !t.protocol.is_bt() {
            self.set_toast("该任务没有 trackers（仅 BT 任务支持）");
            return;
        }
        self.trackers_open = true;
        self.trackers_pos = None;
        self.trackers_drag = None;
        self.trackers_area = None;
        self.trackers_scroll = 0;
    }

    pub fn close_trackers_popup(&mut self) {
        self.trackers_open = false;
        self.trackers_area = None;
        self.trackers_pos = None;
        self.trackers_drag = None;
    }
}
