//! pieces — 分块状态弹窗（`P` 快捷键，FR-01-106）应用层：弹窗状态、打开
//! 功能门槛（四拒绝臂，顺序固定）、关闭与网格滚动。
//!
//! 口径（FR-01-106，对齐 ezr-demo 源码 open_pieces/close_pieces）：
//! - 功能门槛 = 仅支持分块下载的任务：未选中任务 → 任务未探测（D35）→
//!   不支持分块下载（HTTP 不可续传）→ 空内容（total = 0），逐臂 toast 且
//!   弹窗不打开（toast 文案照录 demo 定稿 §6.5 与 D35）；
//! - 打开即置 follow 定位标记（首帧由渲染层按实际列数定位到下载前沿），
//!   滚动清零、拖拽位置清空（重新打开恒居中）；
//! - 关闭清全部弹窗态（区域/位置/拖拽），`q`/`Q` 同为关闭而非退出；
//! - 网格滚动以行为单位，上限由渲染层按实际总行数钳制（`End` 置
//!   `usize::MAX` 由渲染层收敛到末页）。

use super::App;

impl App {
    /// 打开选中任务的 pieces 分块弹窗（`P`，FR-01-106 ①功能门槛）。
    ///
    /// 拒绝臂顺序固定（逐臂 toast 且弹窗不打开，文案照录 demo 定稿 §6.5
    /// 与 D35）：未选中任务 → 任务未探测（D35）→ 不支持分块下载（HTTP
    /// 不可续传）→ 空内容（total = 0）。
    ///
    /// 未探测臂（D35，v133 收窄）：立臂理由是「探测前大小未知」——故该臂
    /// 只在大小未知态（`!probed && total == 0`）命中。恢复（继续）后的重探
    /// 窗口（获槽清 probed、Probed 事件未至；操作者 v133 实证：此时按 P
    /// 被误拒）任务大小已知，不属「未知态」，落入正常三臂可开窗。臂序与
    /// 文案不变（D35 操作者裁决：保留现方案）。
    pub fn open_pieces(&mut self) {
        // 先取门槛快照再分派（借用收窄：sel_task 的 &self 借用与 set_toast
        // 的 &mut self 不重叠，notes/rust.md「路径敏感借用」纪律）
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
        self.pieces_follow = true; // 首帧由渲染层按实际列数定位到下载前沿
        self.pieces_scroll = 0;
        self.pieces_pos = None; // 打开时居中（渲染层首帧回填）
        self.pieces_drag = None;
    }

    /// 关闭 pieces 分块弹窗（`Esc`/`P`/`q`/`Q`、点外点击；位置记忆不保留）
    pub fn close_pieces(&mut self) {
        self.pieces_open = false;
        self.pieces_area = None;
        self.pieces_pos = None;
        self.pieces_drag = None;
    }

    /// pieces 网格滚动（行单位；上限由渲染层按实际总行数钳制）
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
    //! FR-01-106 ①功能门槛四拒绝臂（文案照录 + D35 门槛序）与打开/关闭/
    //! 滚动状态机单测。

    use super::*;

    /// 夹具单源委托（app 树 App 夹具族收口 testutil，architect v153）
    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-pieces", tag)
    }

    /// 可打开弹窗的基准任务：已探测、可续传、total 5 MB（5 块 ×1 MB）——
    /// 分块数学族口径单源 testutil（architect v153）
    fn openable(id: u32, name: &str) -> crate::model::Task {
        crate::app::testutil::dl_task_mb(id, name)
    }

    /// D35 门槛序：未探测臂先于「不支持分块」「空内容」两臂——三条件同时
    /// 命中时 toast 必须是未探测文案
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

    /// v133 收窄回归（操作者实证：恢复后重探窗口按 P 被误拒）：probed 在
    /// 获槽时清 false、Probed 事件未至，但任务大小已知（total > 0）——
    /// 不属 D35「探测前大小未知态」，未探测臂不得命中，可续传时直接开窗。
    #[tokio::test]
    async fn open_pieces_reprobe_window_with_known_size_opens() {
        let mut app = make_app("gate-reprobe");
        let mut t = openable(1, "rp.bin");
        t.probed = false; // 获槽重探窗口（tick_grant_and_start 清标记）
                          // total/resumable 保持持久化真值（5 MB、可续传）
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

    /// v133 收窄补臂：重探窗口 + 不可续传 → 未探测臂不命中，落「不支持
    /// 分块」臂（文案照录，序不变）
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

    /// 拒绝臂 1：未选中任务（空列表）
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

    /// 拒绝臂 3：HTTP 不可续传（已探测、total 有值）
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

    /// 拒绝臂 4：空内容（探测完成 total = 0）
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

    /// 门槛序：不支持分块臂先于空内容臂（两条件同时命中 → 续传拒绝文案）
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

    /// 打开成功：置位 + follow 定位标记 + 滚动清零 + 位置/拖拽清空（重开恒居中）
    #[tokio::test]
    async fn open_pieces_success_initializes_state() {
        let mut app = make_app("open");
        app.tasks.push(openable(1, "ok.bin"));
        app.selected = 0;
        // 预置脏状态：上次会话遗留的滚动/位置/拖拽
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

    /// 关闭：清全部弹窗态（区域/位置/拖拽），再次打开重新居中
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
        // 再次打开 → 重新居中口径（pos 为 None）
        app.open_pieces();
        assert!(app.pieces_open);
        assert_eq!(app.pieces_pos, None, "拖拽位置不保留（FR-01-106 ⑦）");
        app.shutdown().await;
    }

    /// 滚动：正向累加、负向扣减、上滚钳制到 0（下限钳制）
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
