use super::App;

impl App {
    pub fn open_pieces(&mut self) {
        let gate = self
            .sel_task()
            .map(|t| (t.probed, t.supports_pieces(), t.total));
        let Some((probed, supports, total)) = gate else {
            self.set_toast("未选中任务，无法查看分块");
            return;
        };
        if !probed && total == 0 {
            self.set_toast("任务尚未探测，暂无分块信息");
            return;
        }
        if !supports {
            self.set_toast("该任务不支持分块下载（服务器不支持断点续传），无 pieces 信息");
            return;
        }
        if total == 0 {
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

    pub(crate) fn pieces_scroll_by(&mut self, delta: i32) {
        if delta < 0 {
            self.pieces_scroll = self
                .pieces_scroll
                .saturating_sub(delta.unsigned_abs() as usize);
        } else {
            self.pieces_scroll = self.pieces_scroll.saturating_add(delta as usize);
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-pieces", tag)
    }

    fn openable(id: u32, name: &str) -> crate::model::Task {
        crate::app::testutil::dl_task_mb(id, name)
    }

    #[tokio::test]
    async fn open_pieces_unprobed_gate_precedes_others() {
        let mut app = make_app("gate1");
        let mut t = openable(1, "u.bin");
        t.probed = false;
        t.resumable = false;
        t.total = 0;
        app.tasks.push(t);
        app.selected = 0;
        app.open_pieces();
        assert!(!app.pieces_open, "弹窗不打开");
        assert_eq!(
            app.toast.as_deref(),
            Some("任务尚未探测，暂无分块信息"),
            "未探测臂（D35）先于不支持分块/空内容两臂"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn open_pieces_reprobe_window_with_known_size_opens() {
        let mut app = make_app("gate-reprobe");
        let mut t = openable(1, "rp.bin");
        t.probed = false;
        app.tasks.push(t);
        app.selected = 0;
        app.open_pieces();
        assert!(app.pieces_open, "大小已知的重探窗口不得误拒");
        assert_ne!(
            app.toast.as_deref(),
            Some("任务尚未探测，暂无分块信息"),
            "未探测臂不得命中（toast 停留启动欢迎语）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn open_pieces_reprobe_window_non_resumable_hits_support_arm() {
        let mut app = make_app("gate-reprobe-nr");
        let mut t = openable(1, "rp-nr.bin");
        t.probed = false;
        t.resumable = false;
        app.tasks.push(t);
        app.selected = 0;
        app.open_pieces();
        assert!(!app.pieces_open);
        assert_eq!(
            app.toast.as_deref(),
            Some("该任务不支持分块下载（服务器不支持断点续传），无 pieces 信息"),
            "重探窗口大小已知 → 正常三臂口径"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn open_pieces_no_selection_toast() {
        let mut app = make_app("gate2");
        app.open_pieces();
        assert!(!app.pieces_open);
        assert_eq!(
            app.toast.as_deref(),
            Some("未选中任务，无法查看分块"),
            "文案照录 FR-01-106 ①"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn open_pieces_non_resumable_toast() {
        let mut app = make_app("gate3");
        let mut t = openable(1, "n.bin");
        t.resumable = false;
        app.tasks.push(t);
        app.selected = 0;
        app.open_pieces();
        assert!(!app.pieces_open);
        assert_eq!(
            app.toast.as_deref(),
            Some("该任务不支持分块下载（服务器不支持断点续传），无 pieces 信息"),
            "文案照录 FR-01-106 ①"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn open_pieces_empty_content_toast() {
        let mut app = make_app("gate4");
        let mut t = openable(1, "e.bin");
        t.total = 0;
        app.tasks.push(t);
        app.selected = 0;
        app.open_pieces();
        assert!(!app.pieces_open);
        assert_eq!(
            app.toast.as_deref(),
            Some("任务内容为空，无分块信息"),
            "文案照录 FR-01-106 ①"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn open_pieces_resume_gate_precedes_empty() {
        let mut app = make_app("gate5");
        let mut t = openable(1, "n2.bin");
        t.resumable = false;
        t.total = 0;
        app.tasks.push(t);
        app.selected = 0;
        app.open_pieces();
        assert_eq!(
            app.toast.as_deref(),
            Some("该任务不支持分块下载（服务器不支持断点续传），无 pieces 信息"),
            "不支持分块先于空内容"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn open_pieces_success_initializes_state() {
        let mut app = make_app("open");
        app.tasks.push(openable(1, "ok.bin"));
        app.selected = 0;
        app.pieces_scroll = 7;
        app.pieces_pos = Some((3, 4));
        app.pieces_drag = Some((1, 2));
        app.open_pieces();
        assert!(app.pieces_open);
        assert!(app.pieces_follow, "首帧由渲染层定位下载前沿");
        assert_eq!(app.pieces_scroll, 0, "打开滚动清零");
        assert_eq!(app.pieces_pos, None, "位置清空 → 渲染层居中回填");
        assert_eq!(app.pieces_drag, None);
        app.shutdown().await;
    }

    #[tokio::test]
    async fn open_pieces_bt_always_supported() {
        let mut app = make_app("gate-bt");
        let mut t = openable(1, "bt.bin");
        t.protocol = crate::model::Protocol::Bt;
        t.resumable = false;
        t.probed = false;
        app.tasks.push(t);
        app.selected = 0;
        app.open_pieces();
        assert!(app.pieces_open, "BT 任务恒支持分块：P 可开弹窗");
        assert_ne!(
            app.toast.as_deref(),
            Some("该任务不支持分块下载（服务器不支持断点续传），无 pieces 信息"),
            "BT 不落「不支持分块」拒绝臂"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn close_pieces_resets_all_state() {
        let mut app = make_app("close");
        app.tasks.push(openable(1, "ok.bin"));
        app.selected = 0;
        app.open_pieces();
        app.pieces_area = Some(ratatui::layout::Rect {
            x: 10,
            y: 5,
            width: 64,
            height: 21,
        });
        app.pieces_pos = Some((10, 5));
        app.pieces_drag = Some((2, 1));
        app.close_pieces();
        assert!(!app.pieces_open);
        assert_eq!(app.pieces_area, None);
        assert_eq!(app.pieces_pos, None);
        assert_eq!(app.pieces_drag, None);
        app.open_pieces();
        assert!(app.pieces_open);
        assert_eq!(app.pieces_pos, None, "拖拽位置不保留（FR-01-106 ⑦）");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn pieces_scroll_by_saturates_at_zero() {
        let mut app = make_app("scroll");
        app.pieces_scroll_by(8);
        assert_eq!(app.pieces_scroll, 8);
        app.pieces_scroll_by(3);
        assert_eq!(app.pieces_scroll, 11);
        app.pieces_scroll_by(-3);
        assert_eq!(app.pieces_scroll, 8);
        app.pieces_scroll_by(-100);
        assert_eq!(app.pieces_scroll, 0, "上滚钳制到 0");
        app.shutdown().await;
    }
}
