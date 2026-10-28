use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, BorderType, Borders};
use ratatui::Frame;

use crate::app::App;
use crate::model::TaskState;

mod about;
mod bt_dialog;
mod btn;
mod chart;
mod cmd;
mod conns;
mod delete;
mod detail;
mod dialog;
mod err;
mod footer;
mod header;
mod help;
mod list;
mod multiline;
mod pieces;
mod scrollable_panel;
mod task_lines;
pub(crate) mod text;
mod tree;
mod val_popup;

use about::draw_about;
use conns::draw_conns;
use detail::draw_detail;
use dialog::draw_dialogs;
use footer::draw_footer;
use header::draw_header;
use help::draw_help;
use list::draw_list;
use pieces::draw_pieces;
#[cfg(test)]
pub(crate) use text::{
    fmt_dur, fmt_eta, fmt_size, fmt_size_pair, fmt_speed, marquee_bar, pad_right, truncate, w,
    wrap_text,
};
use val_popup::draw_val_popup;

use crate::scrollbar::{DIM, DIM2};

pub(super) const BORDER: Color = Color::Rgb(56, 78, 86);
pub(super) const FG: Color = Color::Rgb(198, 208, 212);
pub(super) const SEL_BG: Color = Color::Rgb(22, 42, 48);
pub(super) const ACCENT: Color = Color::Cyan;
pub(super) const GREEN: Color = Color::Green;
pub(super) const YELLOW: Color = Color::Rgb(228, 178, 62);
pub(super) const RED: Color = Color::Rgb(226, 92, 84);
pub(super) const MAGENTA: Color = Color::Rgb(214, 112, 214);
pub(super) const EMPTY: Color = Color::Rgb(54, 68, 74);
pub(super) const LIGHT_BLUE: Color = Color::Rgb(122, 185, 242);

pub(super) const FOCUS_BORDER: Color = Color::Green;

pub(super) fn overlay_open(app: &crate::app::App) -> bool {
    app.overlay_open()
}

pub(super) fn panel_border_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(FOCUS_BORDER)
            .add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        Style::default().fg(BORDER)
    }
}

pub(super) fn panel_title_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(FOCUS_BORDER)
            .add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        Style::default().fg(DIM)
    }
}

const STATE_COLORS: [(TaskState, Color); 8] = [
    (TaskState::Queued, YELLOW),
    (TaskState::Downloading, LIGHT_BLUE),
    (TaskState::Verifying, LIGHT_BLUE),
    (TaskState::PostProcessing, LIGHT_BLUE),
    (TaskState::Paused, Color::White),
    (TaskState::Completed, GREEN),
    (TaskState::Failed, RED),
    (TaskState::Seeding, MAGENTA),
];

pub(super) fn state_color(s: TaskState) -> Color {
    STATE_COLORS
        .iter()
        .find(|(st, _)| *st == s)
        .map_or(DIM, |(_, c)| *c)
}

pub(super) fn bordered_block(fg: Color) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(fg))
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    app.dlg_cursor_screen = None;
    app.list_scrollbar = None;
    app.conns_v_scrollbar = None;
    app.conns_h_scrollbar = None;
    app.pieces_scrollbar = None;
    app.help_scrollbar = None;
    app.cmd_sb = None;
    app.cmd_btn_rect = None;
    app.cmd_area = None;
    app.err_btn_rect = None;
    app.err_area = None;
    app.dlg_bt_sb = None;
    app.dlg_trackers_sb = None;
    app.dlg_trackers_h_sb = None;
    app.dlg_btfile_rects.clear();
    app.dlg_btbtn_rects.clear();
    app.cmd_cursor_screen = None;
    let outer = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(8),
        Constraint::Length(4),
    ])
    .split(area);

    let wide = area.width >= 100;
    if !wide || !app.show_panes {
        app.focus = crate::app::Panel::List;
    }
    if wide {
        draw_header(f, app, outer[0], true);

        if app.show_panes {
            let cols = Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
                .split(outer[1]);
            draw_list(f, app, cols[0]);
            let right =
                Layout::vertical([Constraint::Length(12), Constraint::Min(6)]).split(cols[1]);
            draw_detail(f, app, right[0]);
            draw_conns(f, app, right[1]);
        } else {
            app.conns_area = None;
            app.visible_conns_rows = 0;
            app.detail_url_rect = None;
            app.detail_ck_rect = None;
            app.detail_fail_rect = None;
            app.detail_save_rect = None;
            app.detail_val_rects = Vec::new();
            app.detail_area = None;
            draw_list(f, app, outer[1]);
        }
    } else {
        app.conns_area = None;
        app.visible_conns_rows = 0;
        app.detail_url_rect = None;
        app.detail_ck_rect = None;
        app.detail_fail_rect = None;
        app.detail_save_rect = None;
        app.detail_val_rects = Vec::new();
        app.detail_area = None;
        draw_header(f, app, outer[0], false);
        draw_list(f, app, outer[1]);
    }

    draw_footer(f, app, outer[2]);

    app.popup_close_rect = None;
    if app.dialog.is_some() {
        draw_dialogs(f, app, area);
    }

    if app.cmd_open {
        cmd::draw_cmd(f, app, area);
    }
    if app.err_open {
        err::draw_err(f, app, area);
    }

    if app.pieces_open {
        draw_pieces(f, app, area);
    }

    if app.help_open {
        draw_help(f, app, area);
    }

    if app.about_open {
        draw_about(f, app, area);
    }

    if app.val_open {
        draw_val_popup(f, app, area);
    }

    if app.dialog.is_some()
        && !app.pieces_open
        && !app.help_open
        && !app.about_open
        && !app.val_open
        && !app.cmd_open
        && !app.err_open
    {
        if let Some((x, y)) = app.dlg_cursor_screen {
            f.set_cursor_position(ratatui::layout::Position { x, y });
        }
    }

    if app.cmd_open {
        if let Some((x, y)) = app.cmd_cursor_screen {
            f.set_cursor_position(ratatui::layout::Position { x, y });
        }
    } else {
        app.cmd_cursor_screen = None;
    }
}

#[cfg(test)]
mod ui_tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;
    use crate::app::CHECKSUM_ALGOS;
    use crate::model::{Checksum, FailKind, Protocol, Task};

    fn make_app(tag: &str) -> App {
        super::testfx::make_app_in("ezr-ui", tag)
    }

    fn task(id: u32, name: &str, state: TaskState) -> Task {
        let mut t = Task::new_queued(
            id,
            name.to_string(),
            Protocol::Http,
            format!("http://example.com/{name}"),
            "/tmp".to_string(),
            1024 * 1024,
            4,
            3,
            None,
            crate::model::config::ProxyChoice::Direct,
            0,
        );
        t.state = state;
        t.total = 3000;
        t.downloaded = 1500;
        t.probed = true;
        t
    }

    #[test]
    fn state_color_covers_all_states() {
        for (st, c) in STATE_COLORS {
            assert_eq!(state_color(st), c, "{st:?}");
        }
    }

    #[test]
    fn state_color_covers_every_variant_without_fallback() {
        for s in [
            TaskState::Queued,
            TaskState::Downloading,
            TaskState::Verifying,
            TaskState::PostProcessing,
            TaskState::Paused,
            TaskState::Completed,
            TaskState::Failed,
            TaskState::Seeding,
        ] {
            match s {
                TaskState::Queued
                | TaskState::Downloading
                | TaskState::Verifying
                | TaskState::PostProcessing
                | TaskState::Paused
                | TaskState::Completed
                | TaskState::Failed
                | TaskState::Seeding => {}
            }
            assert_ne!(
                state_color(s),
                DIM,
                "{s:?} 落 DIM 兜底（未入 STATE_COLORS 映射表）"
            );
        }
    }

    #[test]
    fn bordered_block_renders_rounded_full_borders_with_fg() {
        for c in [BORDER, ACCENT, RED] {
            let mut term = Terminal::new(TestBackend::new(5, 3)).unwrap();
            term.draw(|f| f.render_widget(bordered_block(c), f.area()))
                .unwrap();
            let buf = term.backend().buffer();
            for (x, y, sym) in [
                (0, 0, "╭"),
                (4, 0, "╮"),
                (0, 2, "╰"),
                (4, 2, "╯"),
                (2, 0, "─"),
                (2, 2, "─"),
                (0, 1, "│"),
                (4, 1, "│"),
            ] {
                let cell = buf.cell((x, y)).expect("网格坐标在界内");
                assert_eq!(cell.symbol(), sym, "({x},{y}) 符号");
                assert_eq!(cell.fg, c, "({x},{y}) 边框色");
            }
            let inner = buf.cell((2, 1)).expect("网格坐标在界内");
            assert_eq!(inner.symbol(), " ", "内部应保持空白");
        }
    }

    #[tokio::test]
    async fn draw_renders_tasks_in_every_state() {
        let mut app = make_app("all");
        for (i, st) in [
            TaskState::Queued,
            TaskState::Downloading,
            TaskState::Paused,
            TaskState::Verifying,
            TaskState::PostProcessing,
            TaskState::Completed,
            TaskState::Failed,
            TaskState::Seeding,
        ]
        .into_iter()
        .enumerate()
        {
            let mut t = task(i as u32 + 1, &format!("f{}.bin", i + 1), st);
            if st == TaskState::Failed {
                t.error = Some("连接失败（拒绝）".to_string());
                t.retries = 1;
                t.retry_in = Some(3.0);
            }
            if st == TaskState::Completed {
                t.verify_ok = Some(true);
                t.checksum = Some(Checksum {
                    algo: CHECKSUM_ALGOS[0].0,
                    value: "0123abcd0123abcd0123abcd0123abcd".to_string(),
                });
            }
            app.tasks.push(t);
        }
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("任务详情"), "详情面板");
        assert!(s.contains("f1.bin"), "列表条目");
        assert!(s.contains("f7.bin"), "失败条目");
        assert!(s.contains("下载中"), "进行中徽标");
        assert!(
            s.contains("不自动重试") || s.contains("后重试"),
            "失败统计行"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn draw_detail_omits_conn_table() {
        let mut app = make_app("no-conn-table");
        let mut t = task(1, "detail.bin", TaskState::Downloading);
        t.connections = (1..=4usize)
            .map(|i| crate::model::Connection {
                id: i,
                start: (i as u64 - 1) * 750,
                end: i as u64 * 750,
                done: 100 * i as u64,
            })
            .collect();
        app.tasks.push(t);
        app.show_panes = true;
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(!s.contains("并发分块明细"), "明细表段落已移除: {s}");
        assert!(s.contains("分块"), "任务级分块字段行保留");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn draw_detail_omits_status_and_speed_rows() {
        let mut app = make_app("no-status-speed");
        let mut t = task(1, "detail.bin", TaskState::Downloading);
        t.speed = 123456.0;
        app.tasks.push(t);
        let area = ratatui::layout::Rect::new(0, 0, 110, 24);
        let mut term = Terminal::new(TestBackend::new(110, 24)).unwrap();
        term.draw(|f| draw_detail(f, &mut app, area)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("任务详情"), "详情标题在位: {s}");
        assert!(!s.contains("状态"), "状态行应已移除: {s}");
        assert!(
            !s.contains("速度"),
            "速度行应已移除（speed≠0 也不展示）: {s}"
        );
        assert!(s.contains("分块"), "分块行保留: {s}");
        assert!(s.contains("大小"), "大小行保留: {s}");
        assert!(s.contains("路径"), "路径行保留: {s}");
        assert!(!s.contains("保存 "), "「保存」字段名已精简: {s}");
        assert!(s.contains(" ID "), "「ID」字段名精简在位: {s}");
        assert!(s.contains(" 创建 "), "「创建」字段名精简在位: {s}");
        assert!(!s.contains("任务 ID"), "「任务 ID」已精简: {s}");
        assert!(!s.contains("创建时间"), "「创建时间」已精简: {s}");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn draw_detail_size_row_omits_remaining_suffix() {
        let mut app = make_app("no-remaining");
        let t = task(1, "detail.bin", TaskState::Downloading);
        assert!(t.downloaded < t.total, "夹具须未完成（否则负断言无意义）");
        app.tasks.push(t);
        let area = ratatui::layout::Rect::new(0, 0, 110, 24);
        let mut term = Terminal::new(TestBackend::new(110, 24)).unwrap();
        term.draw(|f| draw_detail(f, &mut app, area)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("大小"), "大小行在位: {s}");
        assert!(!s.contains("剩余"), "大小行不应含「（剩余 …）」后缀: {s}");
        assert!(s.contains("分块"), "分块行保留: {s}");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn draw_empty_app_shows_placeholder() {
        let mut app = make_app("empty");
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("（未选中任务）"), "空详情占位");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn draw_add_dialog_with_dropdown() {
        let mut app = make_app("dlg");
        app.open_add_dialog();
        let mut term = Terminal::new(TestBackend::new(110, 34)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(
            s.contains("添加任务：HTTP(S)"),
            "对话框标题（v1.32 更名）: {s}"
        );
        assert!(s.contains("NONE"), "算法行显示默认 NONE（v1.32）");
        assert!(
            !s.contains("https://example.com/file.zip"),
            "URL 占位提示已删除（v1.32/P3）: {s}"
        );
        assert!(s.contains("立即下载"), "立即下载按钮");
        assert!(s.contains("仅添加"), "仅添加按钮");
        assert!(s.contains("取消"), "取消按钮");
        if let Some(d) = app.dialog.as_mut() {
            d.ck_open = true;
        }
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("▸ NONE"), "下拉首行选中 NONE: {s}");
        assert!(s.contains("MD5"), "下拉算法项");
        assert!(s.contains("Adler-32"), "下拉算法项2");
        assert!(s.contains("128 位"), "下拉位数列");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn draw_delete_dialog() {
        let mut app = make_app("del");
        app.open_add_dialog();
        let t = task(9, "todelete.bin", TaskState::Paused);
        app.tasks.push(t);
        app.open_delete_dialog();
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("删除任务"), "删除对话框");
        assert!(s.contains("仅删除任务"), "按钮1");
        assert!(s.contains("删除任务和文件"), "按钮2");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn draw_narrow_hides_split_layout() {
        let mut app = make_app("narrow");
        app.tasks.push(task(1, "n1.bin", TaskState::Downloading));
        app.show_panes = true;
        let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("n1.bin"), "窄布局列表");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn draw_detail_checksum_prefix_and_verify_states() {
        let mut app = make_app("ck-row");
        let mut t = task(1, "ck.bin", TaskState::Verifying);
        let value = "0123456789abcdef".repeat(4);
        t.checksum = Some(crate::model::Checksum {
            algo: "SHA-256",
            value: value.clone(),
        });
        app.tasks.push(t);
        app.show_panes = true;
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();

        app.tasks[0].verify_ok = None;
        app.tasks[0].state = TaskState::Paused;
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("0123456789"), "前 10 位: {s}");
        assert!(s.contains("…"), "省略号截断: {s}");
        assert!(!s.contains(&value), "完整校验码不出现: {s}");
        assert!(s.contains("（待校验）"), "待校验文案: {s}");

        app.tasks[0].state = TaskState::Verifying;
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("（校验中）"), "校验中文案: {s}");

        app.tasks[0].verify_ok = Some(true);
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("（已通过）"), "已通过文案: {s}");

        app.tasks[0].verify_ok = Some(false);
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("（未通过）"), "未通过文案: {s}");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn move_task_swaps_adjacent_and_clamps() {
        let mut app = make_app("move");
        app.tasks.push(task(1, "a.bin", TaskState::Downloading));
        app.tasks.push(task(2, "b.bin", TaskState::Downloading));
        app.tasks.push(task(3, "c.bin", TaskState::Downloading));
        app.selected = 1;
        app.move_task(-1);
        assert_eq!(app.tasks[0].name, "b.bin", "上移交换");
        assert_eq!(app.tasks[1].name, "a.bin");
        assert_eq!(app.selected, 0, "选中跟随移动项");
        app.move_task(-1);
        assert_eq!(app.tasks[0].name, "b.bin", "顶界不动");
        assert_eq!(app.tasks[2].name, "c.bin", "未移动项不受影响");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn toggle_pause_retry_and_clear_state_flows() {
        let mut app = make_app("ops");
        app.max_slots = 1;

        app.tasks.push(task(1, "p1.bin", TaskState::Downloading));
        app.tasks[0].speed = 123.0;
        app.tasks[0].has_slot = true;
        app.selected = 0;
        app.toggle_pause();
        assert_eq!(app.tasks[0].state, TaskState::Paused);
        assert_eq!(app.tasks[0].speed, 0.0);
        assert!(!app.tasks[0].has_slot, "暂停释放槽位");

        app.tasks.push(task(2, "p2.bin", TaskState::Downloading));
        app.tasks[1].has_slot = true;
        app.toggle_pause();
        assert_eq!(
            app.tasks[0].state,
            TaskState::Queued,
            "继续 = 重新排队（统一路径，不检测空闲槽位）"
        );
        assert!(!app.tasks[0].has_slot);

        app.toggle_pause();
        assert_eq!(app.tasks[0].state, TaskState::Paused);

        app.tasks[0].state = TaskState::Failed;
        app.tasks[0].fail_kind = Some(FailKind::Fatal);
        app.tasks[0].retries = 3;
        app.tasks[0].error = Some("磁盘错误".to_string());
        app.selected = 0;
        app.retry();
        assert_eq!(app.tasks[0].state, TaskState::Queued);
        assert_eq!(app.tasks[0].retries, 1, "手动重试计数重置");
        assert!(app.tasks[0].error.is_none());
        assert_eq!(app.tasks[0].fail_kind, None);

        app.tasks[0].state = TaskState::Failed;
        app.tasks[0].fail_kind = Some(FailKind::Verify);
        app.tasks[0].checksum = Some(crate::model::Checksum {
            algo: "SHA-256",
            value: "ab".repeat(32),
        });
        app.retry();
        assert_eq!(app.tasks[0].state, TaskState::Verifying);
        assert!(app.tasks[0].has_slot, "重校验占槽位");

        app.tasks[0].state = TaskState::Failed;
        app.tasks[0].fail_kind = Some(FailKind::Verify);
        app.tasks[0].checksum = None;
        app.retry();
        assert_eq!(app.tasks[0].state, TaskState::Queued);
        assert!(!app.tasks[0].has_slot, "等待排队不占槽");
        app.tasks[0].state = TaskState::Verifying;
        app.tasks[0].has_slot = true;

        app.tasks.push(task(3, "p3.bin", TaskState::Completed));
        app.clear_completed();
        assert_eq!(app.tasks.len(), 2, "仅清理完成任务");
        assert!(!app.tasks.iter().any(|t| t.name == "p3.bin"));

        app.selected = 0;
        app.toggle_pause();
        assert_eq!(
            app.tasks[0].state,
            TaskState::Verifying,
            "校验中不受 Space 影响"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn failed_pause_resume_flow() {
        let mut app = make_app("fpause");
        app.max_slots = 1;

        app.tasks.push(task(1, "f1.bin", TaskState::Failed));
        app.tasks[0].retries = 2;
        app.tasks[0].retry_in = Some(5.0);
        app.tasks[0].has_slot = true;
        app.tasks[0].fail_kind = None;
        app.tasks[0].error = Some("连接被重置".to_string());
        app.selected = 0;
        app.toggle_pause();
        assert_eq!(
            app.tasks[0].state,
            TaskState::Paused,
            "失败按空格 = 暂停（v1.21 合并口径）"
        );
        assert_eq!(
            app.tasks[0].state.label(),
            "已暂停",
            "徽标文案与普通暂停一致（v1.22 展示不细分）"
        );
        assert!(app.tasks[0].retry_in.is_none(), "倒计时清除，不再自动重试");
        assert!(!app.tasks[0].has_slot, "不占下载槽位");
        assert!(
            app.tasks[0].error.is_none(),
            "错误信息挂起即重置（v1.23：error=null）"
        );
        assert_eq!(
            app.tasks[0].retries, 1,
            "重试计数挂起即重置（v1.23：retries=1）"
        );
        assert!(app.toast.as_deref().is_some_and(|m| m.contains("已暂停")));
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("按空格恢复") && !m.contains("或 R")),
            "挂起引导文案应只指空格（v1.19）: {:?}",
            app.toast
        );

        assert!(
            !app.tasks.iter().any(|t| t.state == TaskState::Queued),
            "挂起任务不重新排队"
        );

        app.toggle_pause();
        assert_eq!(app.tasks[0].state, TaskState::Queued, "恢复 = 重新排队");
        assert_eq!(app.tasks[0].retries, 1, "重新排队计数重置");
        assert!(app.tasks[0].error.is_none());

        app.tasks[0].state = TaskState::Paused;
        app.tasks[0].retries = 4;
        app.tasks[0].error = Some("超时".to_string());
        app.retry();
        assert_eq!(app.tasks[0].state, TaskState::Paused, "R 不恢复挂起任务");
        assert_eq!(app.tasks[0].retries, 4, "重试计数不被重置");
        assert_eq!(app.tasks[0].error.as_deref(), Some("超时"), "错误信息保留");
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("仅「已失败」的任务可以重试")),
            "toast={:?}",
            app.toast
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn paused_resume_always_requeues_even_with_free_slot() {
        let mut app = make_app("requeue");
        app.max_slots = 5;
        app.tasks.push(task(1, "r1.bin", TaskState::Paused));
        app.tasks[0].resumable = true;
        app.selected = 0;
        app.toggle_pause();
        assert_eq!(
            app.tasks[0].state,
            TaskState::Queued,
            "有空闲槽也一律重新排队（直开路径废除）"
        );
        assert!(!app.tasks[0].has_slot, "重新排队不占槽，待调度分配");
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("已进入等待队列")),
            "toast={:?}",
            app.toast
        );
        app.tasks[0].state = TaskState::Paused;
        app.tasks[0].resumable = false;
        app.toggle_pause();
        assert_eq!(app.tasks[0].state, TaskState::Queued);
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("将从头下载") && m.contains("已进入等待队列")),
            "toast={:?}",
            app.toast
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn global_key_router_arms() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = make_app("keys");
        app.tasks.push(task(1, "k1.bin", TaskState::Downloading));
        app.tasks.push(task(2, "k2.bin", TaskState::Downloading));
        app.tasks[1].state = TaskState::Completed;

        app.on_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(app.quit, "Ctrl+C 退出");
        app.quit = false;

        app.on_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(app.quit);
        app.quit = false;
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.quit, "Esc 主界面无操作（v1.30）");
        assert!(!app.help_open && !app.pieces_open && app.dialog.is_none());

        app.on_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.focus, crate::app::Panel::Detail, "Tab → 详情");
        assert_eq!(app.filter, 0, "Tab 不切页签（v1.30 改版）");
        app.on_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.focus, crate::app::Panel::Conns, "Tab → 并发");
        app.on_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.focus, crate::app::Panel::List, "Tab 环回列表");
        app.on_key(KeyCode::BackTab, KeyModifiers::NONE);
        assert_eq!(app.focus, crate::app::Panel::Conns, "BackTab 反向");
        app.on_key(KeyCode::BackTab, KeyModifiers::NONE);
        assert_eq!(app.focus, crate::app::Panel::Detail, "BackTab → 详情");
        app.on_key(KeyCode::BackTab, KeyModifiers::NONE);
        assert_eq!(app.focus, crate::app::Panel::List, "BackTab 环回列表");

        app.on_key(KeyCode::Char(']'), KeyModifiers::NONE);
        assert_eq!(app.filter, 1, "] 到已完成页签");
        app.on_key(KeyCode::Char('['), KeyModifiers::NONE);
        assert_eq!(app.filter, 0, "[ 回首");
        app.on_key(KeyCode::Char('T'), KeyModifiers::NONE);
        assert_eq!(app.filter, 1, "T 循环");
        app.on_key(KeyCode::Char('T'), KeyModifiers::NONE);
        assert_eq!(app.filter, 0);

        app.on_key(KeyCode::Char('h'), KeyModifiers::NONE);
        assert!(app.help_open, "H 打开帮助");
        app.on_key(KeyCode::Char('h'), KeyModifiers::NONE);
        assert!(!app.help_open, "H 再按关闭");

        app.on_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.selected, 0, "单项视图不越界");
        app.on_key(KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(app.selected, 0);
        app.on_key(KeyCode::End, KeyModifiers::NONE);
        assert_eq!(app.selected, 0, "End 到末项");
        app.on_key(KeyCode::Home, KeyModifiers::NONE);
        assert_eq!(app.selected, 0);
        app.on_key(KeyCode::PageUp, KeyModifiers::NONE);
        assert_eq!(app.selected, 0);

        let panes = app.show_panes;
        app.on_key(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(app.show_panes, !panes, "g 切换右栏两面板（FR-01-98）");

        app.on_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(app.cmd_open, "a 打开命令窗口");
        assert!(app.dialog.is_none(), "不再直开添加对话框");
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "对话框内 Esc 关闭");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn mouse_click_scroll_and_dialog_hits() {
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        use ratatui::layout::Rect;

        let mk = |kind: MouseEventKind, col: u16, row: u16| MouseEvent {
            kind,
            column: col,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };

        let mut app = make_app("mouse");
        app.tasks.push(task(1, "m1.bin", TaskState::Downloading));
        app.tasks.push(task(2, "m2.bin", TaskState::Downloading));
        app.tasks.push(task(3, "m3.bin", TaskState::Downloading));
        app.list_area = Some(Rect::new(0, 6, 60, 12));
        app.visible_rows = 3;

        app.on_mouse(mk(MouseEventKind::Down(MouseButton::Left), 10, 10));
        assert_eq!(app.selected, 1, "点击选中第二项");
        app.on_mouse(mk(MouseEventKind::Down(MouseButton::Left), 10, 2));
        assert_eq!(app.selected, 1);
        app.on_mouse(mk(MouseEventKind::ScrollDown, 0, 0));
        assert_eq!(app.scroll, 0, "0 项可视差时钳制");
        app.on_mouse(mk(MouseEventKind::ScrollUp, 0, 0));
        assert_eq!(app.scroll, 0);

        app.dialog = Some(crate::app::Dialog {
            kind: crate::app::DialogKind::Add,
            url: crate::textfield::TextInput::new(),
            dir: crate::textfield::TextInput::new(),
            conns: crate::textfield::TextInput::new(),
            conns_edited: false,
            ck_type: 0,
            ck_value: crate::textfield::TextInput::new(),
            ck_open: true,
            ck_sel: 0,
            proxy_sel: 0,
            proxy_open: false,
            focus: 0,
            task_name: String::new(),
            task_id: None,
            torrent: crate::textfield::TextInput::new(),
            bt_tree: Vec::new(),
            bt_cursor: 0,
            bt_scroll: 0,
            bt_follow: true,
            trackers: crate::multiline::MultiLineBuf::new(),
        });
        app.dlg_ck_rects = vec![(Rect::new(2, 3, 20, 1), 4)];
        app.on_mouse(mk(MouseEventKind::Down(MouseButton::Left), 5, 3));
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.ck_type, 4, "下拉点选算法");
        assert_eq!(d.ck_sel, 4);
        assert!(!d.ck_open, "点选后收起下拉");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn resume_with_free_slot_also_requeues_and_completed_fallback() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ui-resume2");
        let mut app = make_app("resume2");
        app.max_slots = 5;
        let mut t = task(1, "r1.bin", TaskState::Paused);
        t.resumable = true;
        t.save_dir = dir.to_string_lossy().into_owned();
        app.tasks.push(t);
        app.selected = 0;

        app.toggle_pause();
        assert_eq!(
            app.tasks[0].state,
            TaskState::Queued,
            "有空闲槽也一律重新排队（v1.21 直开路径废除）"
        );
        assert!(!app.tasks[0].has_slot, "重新排队不占槽，待调度分配");
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("从断点恢复") && m.contains("已进入等待队列")),
            "可续传文案: {:?}",
            app.toast
        );

        app.tasks[0].state = TaskState::Paused;
        app.tasks[0].resumable = false;
        app.toggle_pause();
        assert_eq!(app.tasks[0].state, TaskState::Queued);
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("将从头下载")),
            "不可续传文案: {:?}",
            app.toast
        );

        app.tasks[0].state = TaskState::Completed;
        app.filter = 1;
        app.selected = 0;
        app.toggle_pause();
        assert!(
            app.toast.as_deref().is_some_and(|m| m.contains("已完成")),
            "完成态提示: {:?}",
            app.toast
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn draw_modify_dialog_renders_fields_buttons_and_hint() {
        let mut app = make_app("mdlg");
        let mut t = task(7, "mod.bin", TaskState::Paused);
        t.checksum = Some(crate::model::Checksum {
            algo: "MD5",
            value: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
        });
        app.tasks.push(t);
        app.selected = 0;
        app.open_modify_dialog();
        let s = super::testfx::render_frame_string(&mut app, 110, 34);
        assert!(s.contains("修改任务"), "对话框标题: {s}");
        assert!(s.contains("校验码"), "校验码字段行: {s}");
        assert!(s.contains("MD5"), "校验行显示任务算法: {s}");
        assert!(s.contains("直连"), "代理行显示当前选择: {s}");
        assert!(s.contains("[ 确定 ]"), "确定按钮: {s}");
        assert!(s.contains("[ 取消 ]"), "取消按钮: {s}");
        assert!(
            !s.contains("确定后立即生效"),
            "Modify 提示行退役（v1.30）: {s}"
        );
        assert!(
            !s.contains("添加任务：HTTP(S)"),
            "不得渲染 Add 标题（v1.32 更名后新标题）: {s}"
        );
        app.shutdown().await;
    }
}

#[cfg(test)]
mod ui_v113_tests {

    use crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use ratatui::Terminal;

    use super::*;
    use crate::model::{Connection, Task};

    fn make_app(tag: &str) -> App {
        super::testfx::make_app_in("ezr-uiv13", tag)
    }

    fn task(id: u32, name: &str, state: TaskState) -> Task {
        super::testfx::task_in(id, name, state)
    }

    fn conns(ids: &[usize]) -> Vec<Connection> {
        ids.iter()
            .map(|&i| Connection {
                id: i,
                start: (i as u64 - 1) * 1000,
                end: i as u64 * 1000,
                done: 100,
            })
            .collect()
    }

    fn grid(term: &Terminal<TestBackend>) -> Vec<Vec<(char, Color)>> {
        let buf = term.backend().buffer();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| {
                        let c = buf.cell((x, y)).expect("网格坐标在界内");
                        (c.symbol().chars().next().unwrap_or(' '), c.fg)
                    })
                    .collect()
            })
            .collect()
    }

    fn row_strings(term: &Terminal<TestBackend>) -> Vec<String> {
        super::testfx::row_strings(term)
    }

    fn render(app: &mut App, w: u16, h: u16) -> Terminal<TestBackend> {
        super::testfx::render_full(app, w, h)
    }

    fn hist() -> Vec<u64> {
        (0..60).map(|i| 30 + (i % 7) * 10).collect()
    }

    #[tokio::test]
    async fn header_band_final_layout() {
        let mut app = make_app("hdr");
        app.tasks.push(task(1, "h1.bin", TaskState::Downloading));
        let term = render(&mut app, 120, 40);
        let s = term.backend().to_string();
        assert!(!s.contains("任务队列"), "任务队列面板已撤销: {s}");
        assert!(!s.contains("全局速度"), "旧 Sparkline 面板已移除: {s}");
        let rows: Vec<String> = row_strings(&term)
            .iter()
            .map(|r| r.replace(' ', ""))
            .collect();
        assert!(rows[0].contains("EZRDownloader"), "标题行");
        assert!(
            !rows[0].contains("峰值"),
            "峰值写在面板内容而非标题行边框（REQ-6.1）"
        );
        let r1 = &rows[1];
        let (a, b, c, d) = (
            r1.find("并发"),
            r1.find("会话已下载"),
            r1.find("全局"),
            r1.find("峰值"),
        );
        assert!(
            matches!((a, b, c, d), (Some(a), Some(b), Some(c), Some(d)) if a < b && b < c && c < d),
            "字段单行顺序 并发→会话已下载→全局→峰值: {r1}"
        );
        let r3 = &rows[3];
        let (t1, t2, t3) = (
            r3.find("正在下载("),
            r3.find("已完成("),
            r3.find("下载槽位"),
        );
        assert!(
            matches!((t1, t2, t3), (Some(a), Some(b), Some(c)) if a < b && b < c),
            "页签行：正在下载 │ 已完成 │ 下载槽位（内联左对齐）: {r3}"
        );
        assert!(
            r3.contains("已完成(0)│下载槽位1/5"),
            "槽位紧跟「已完成」页签并以 │ 分隔（内联左对齐，REQ-8.1）: {r3}"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn chart_area_single_color_partial_blocks_no_braille() {
        let mut app = make_app("chart");
        app.tasks.push(task(1, "w1.bin", TaskState::Downloading));
        app.speed_hist = hist();
        let term = render(&mut app, 120, 40);
        let g = grid(&term);
        let mut partials = 0;
        let mut foreign_color = false;
        for row in &g[1..4] {
            for &(ch, fg) in &row[96..119] {
                if ch != ' ' {
                    partials += 1;
                    if fg != LIGHT_BLUE {
                        foreign_color = true;
                    }
                }
            }
        }
        assert!(partials > 0, "图区每列均有面积块字符（无断列）");
        assert!(!foreign_color, "全图仅一种颜色（下载淡蓝，REQ-8.5）");
        let top_line: String = (96..119).map(|x| g[1][x].0).collect();
        assert!(
            top_line.chars().any(|c| ('▁'..='▇').contains(&c)),
            "顶线含部分块字符（1/8 格精度）: {top_line}"
        );
        let mut right_edge_ink = false;
        for (y, row) in g.iter().enumerate().take(4).skip(1) {
            assert_eq!(row[95].0, '│', "│ 分隔符贯通全高 y={y}");
            if row[117].0 != ' ' || row[118].0 != ' ' {
                right_edge_ink = true;
            }
        }
        assert!(right_edge_ink, "图区右缘两列内有墨（无右 padding，v1.32）");
        for row in &g {
            for (ch, _) in row {
                let u = *ch as u32;
                assert!(!(0x2800..=0x28FF).contains(&u), "无盲文点阵: {ch}");
            }
        }
        app.shutdown().await;
    }

    #[tokio::test]
    async fn conns_panel_http_structure_and_state_gate() {
        let mut app = make_app("conns");
        let mut t = task(1, "c1.bin", TaskState::Downloading);
        t.connections = conns(&[1, 2, 3]);
        app.tasks.push(t);
        let term = render(&mut app, 120, 40);
        let rows = row_strings(&term);
        let s = term.backend().to_string();
        assert!(s.contains("并发连接"), "面板标题");
        assert!(s.contains("3/4 项"), "活跃口径徽标 x/y 项: {s}");
        assert!(s.contains("序号"), "HTTP 表头 序号 列");
        assert!(s.contains("下载速度") && s.contains("累计下载"), "数据列");
        assert!(!s.contains("Ctrl+↑↓"), "提示行退役（v1.30）");
        assert!(!s.contains("Ctrl+B"), "Ctrl+B 提示退役（v1.30）");
        assert!(!s.contains("（仅 BT）"), "「（仅 BT）」标注退役");
        assert!(
            rows.iter().any(|r| r.contains(" 1 ") && r.contains('-')),
            "数据行按序号展示、正在连接速度显示 -"
        );
        let row17: String = rows[17].replace(' ', "");
        assert!(
            row17.contains("并发连接"),
            "明细面板紧随详情 12 行: {}",
            rows[17]
        );
        app.tasks[0].state = TaskState::Paused;
        let term = render(&mut app, 120, 40);
        let s = term.backend().to_string();
        assert!(s.contains("（无并发连接）"), "非下载态空面板: {s}");
        assert!(s.contains("0/4 项"), "非明细状态 x=0");
        assert!(app.conns_area.is_none(), "空明细面板不接收滚轮（穿透）");
        app.shutdown().await;
    }

    fn count_partials(
        g: &[Vec<(char, Color)>],
        ys: std::ops::Range<usize>,
        xs: std::ops::Range<usize>,
    ) -> usize {
        let mut n = 0;
        for y in ys {
            for x in xs.clone() {
                let (ch, _) = g[y][x];
                if (char::from_u32(0x2581).unwrap()..=char::from_u32(0x2588).unwrap()).contains(&ch)
                {
                    n += 1;
                }
            }
        }
        n
    }

    #[tokio::test]
    async fn g_compact_keeps_chart_and_narrow_hides_it() {
        let mut app = make_app("gkey");
        app.tasks.push(task(1, "g1.bin", TaskState::Downloading));
        app.speed_hist = hist();
        app.on_key(KeyCode::Char('g'), KeyModifiers::NONE);
        assert!(!app.show_panes, "G 切换右栏两面板");
        let term = render(&mut app, 120, 40);
        let s = term.backend().to_string();
        assert!(!s.contains("任务详情"), "紧凑布局收起详情");
        assert!(!s.contains("并发连接"), "紧凑布局收起明细");
        let g = grid(&term);
        let partials = count_partials(&g, 1..4, 1..119);
        assert!(partials > 0, "紧凑布局下头部流量图仍显示（FR-01-98）");
        let mut app = make_app("narrow");
        app.tasks.push(task(1, "n1.bin", TaskState::Downloading));
        app.speed_hist = hist();
        let term = render(&mut app, 80, 34);
        let g = grid(&term);
        let partials = count_partials(&g, 1..4, 1..79);
        assert_eq!(partials, 0, "窄终端流量图不渲染");
        let s = term.backend().to_string();
        assert!(!s.contains("并发连接"), "窄终端明细不渲染");
        assert!(s.contains("n1.bin"), "列表满宽");
        assert!(!s.contains("任务详情"), "窄终端右栏不渲染");
        app.shutdown().await;
    }
}

#[cfg(test)]
mod ui_v114_tests {

    use ratatui::backend::TestBackend;
    use ratatui::style::{Color, Modifier};
    use ratatui::Terminal;

    use super::*;
    use crate::model::{Checksum, Task};

    fn make_app(tag: &str) -> App {
        super::testfx::make_app_in("ezr-uiv14", tag)
    }

    fn task(id: u32, name: &str, state: TaskState) -> Task {
        super::testfx::task_in(id, name, state)
    }

    fn render_detail(app: &mut App, w: u16, h: u16) -> Terminal<TestBackend> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw_detail(f, app, ratatui::layout::Rect::new(0, 0, w, h)))
            .unwrap();
        term
    }

    fn render_full(app: &mut App, w: u16, h: u16) -> Terminal<TestBackend> {
        super::testfx::render_full(app, w, h)
    }

    fn row_strings(term: &Terminal<TestBackend>) -> Vec<String> {
        super::testfx::row_strings(term)
    }

    fn style_grid(term: &Terminal<TestBackend>) -> Vec<Vec<(Color, Modifier)>> {
        let buf = term.backend().buffer();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| {
                        let c = buf.cell((x, y)).expect("网格坐标在界内");
                        (c.fg, c.modifier)
                    })
                    .collect()
            })
            .collect()
    }

    #[tokio::test]
    async fn type_row_omits_concurrency() {
        let mut app = make_app("t14-type");
        let mut t = task(1, "type.bin", TaskState::Downloading);
        t.concurrency = 4;
        t.connections = (1..=4usize)
            .map(|i| crate::model::Connection {
                id: i,
                start: (i as u64 - 1) * 750,
                end: i as u64 * 750,
                done: 100,
            })
            .collect();
        app.tasks.push(t);
        let term = render_detail(&mut app, 110, 24);
        let type_row = row_strings(&term)[4].replace(' ', "");
        assert!(type_row.contains("HTTP"), "协议名在位: {type_row}");
        assert!(
            type_row.contains("支持断点续传"),
            "续传说明在位: {type_row}"
        );
        assert!(
            !type_row.contains("并发"),
            "类型行无「并发」字样: {type_row}"
        );
        assert!(!type_row.contains("·4"), "类型行无并发数值段: {type_row}");
        let all = term.backend().to_string();
        assert!(all.contains("HTTP · 支持断点续传"), "类型行定稿形态: {all}");
        assert!(!all.contains("不支持断点续传"), "resumable=true 无否定口径");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn link_labels_styled_and_hot_rects_backfilled() {
        let mut app = make_app("t14-link");
        let mut t = task(1, "link.bin", TaskState::Downloading);
        t.checksum = Some(Checksum {
            algo: "SHA-256",
            value: "0123abcd0123abcd0123abcd0123abcd".to_string(),
        });
        app.tasks.push(t);
        let term = render_detail(&mut app, 110, 24);
        let g = style_grid(&term);

        let underlined = |x: usize, y: usize| -> (Color, bool) {
            let (fg, m) = g[y][x];
            (fg, m.contains(Modifier::UNDERLINED))
        };

        for x in [3, 5] {
            let (fg, ul) = underlined(x, 5);
            assert_eq!(fg, LIGHT_BLUE, "校验字段名淡蓝 x={x}");
            assert!(ul, "校验字段名带下划线 x={x}");
        }
        for x in [7, 8, 9] {
            let (_, ul) = underlined(x, 5);
            assert!(!ul, "校验填充空格无下划线 x={x}");
        }
        for x in [3, 4, 5] {
            let (fg, ul) = underlined(x, 8);
            assert_eq!(fg, LIGHT_BLUE, "URL 字段名淡蓝 x={x}");
            assert!(ul, "URL 字段名带下划线 x={x}");
        }
        for x in [6, 8, 10] {
            let (_, ul) = underlined(x, 8);
            assert!(!ul, "URL 填充空格无下划线 x={x}");
        }
        let (fg, ul) = underlined(3, 4);
        assert_eq!(fg, DIM, "类型标签仍暗灰");
        assert!(!ul, "类型标签无下划线");

        assert_eq!(
            app.detail_url_rect,
            Some(ratatui::layout::Rect::new(3, 8, 9, 1)),
            "URL 热区"
        );
        assert_eq!(
            app.detail_ck_rect,
            Some(ratatui::layout::Rect::new(3, 5, 9, 1)),
            "校验热区"
        );
        assert!(app.detail_fail_rect.is_none(), "下载态无失败行热区");
        assert_eq!(
            app.detail_save_rect,
            Some(ratatui::layout::Rect::new(3, 7, 9, 1)),
            "路径热区"
        );
        assert_eq!(app.detail_val_rects.len(), 3, "下载态值热区三项");
        assert!(
            app.detail_val_rects.iter().all(|(r, _)| r.x >= 12),
            "值热区 x = 内框 + 11"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn hot_rects_cleared_for_no_checksum_and_no_selection() {
        let mut app = make_app("t14-nock");
        app.tasks.push(task(1, "nock.bin", TaskState::Downloading));
        let term = render_detail(&mut app, 110, 24);
        assert!(term.backend().to_string().contains("URL"), "URL 行在位");
        assert!(app.detail_url_rect.is_some(), "URL 热区在位");
        assert!(app.detail_ck_rect.is_none(), "无校验行 → 校验热区 None");

        let mut app = make_app("t14-nosel");
        let term = render_detail(&mut app, 110, 24);
        assert!(
            term.backend().to_string().contains("未选中任务"),
            "空态占位"
        );
        assert!(app.detail_url_rect.is_none(), "无选中 → URL 热区清空");
        assert!(app.detail_ck_rect.is_none(), "无选中 → 校验热区清空");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn hot_rects_cleared_when_layout_hides_detail() {
        let mut app = make_app("t14-g");
        app.tasks.push(task(1, "g.bin", TaskState::Downloading));
        app.show_panes = false;
        render_full(&mut app, 120, 40);
        assert!(app.detail_url_rect.is_none(), "G 收起 → URL 热区清空");
        assert!(app.detail_ck_rect.is_none(), "G 收起 → 校验热区清空");

        let mut app = make_app("t14-narrow");
        app.tasks.push(task(1, "n.bin", TaskState::Downloading));
        render_full(&mut app, 80, 34);
        assert!(app.detail_url_rect.is_none(), "窄终端 → URL 热区清空");
        assert!(app.detail_ck_rect.is_none(), "窄终端 → 校验热区清空");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn detail_shows_failure_reason_only_for_failed_state() {
        let mut app = make_app("detail-fail-reason");
        let mut t = task(1, "failed.bin", TaskState::Failed);
        t.error = Some("连接被重置".to_string());
        app.tasks.push(t);
        let term = render_detail(&mut app, 110, 24);
        let s = term.backend().to_string();
        assert!(s.contains("失败"), "已失败态展示失败行: {s}");
        assert!(
            !s.contains("失败原因"),
            "字段名已精简（toast 文案除外）: {s}"
        );
        assert!(s.contains("连接被重置"), "失败原因文本: {s}");
        app.shutdown().await;

        let mut app2 = make_app("detail-paused-no-reason");
        app2.tasks.push(task(2, "paused.bin", TaskState::Paused));
        let term2 = render_detail(&mut app2, 110, 24);
        let s2 = term2.backend().to_string();
        assert!(!s2.contains("失败"), "非失败态不展示失败行: {s2}");
        app2.shutdown().await;
    }
}

#[cfg(test)]
pub(crate) mod testfx {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use crate::app::App;
    use crate::model::config::Config;
    use crate::model::{Task, TaskState};

    pub(crate) fn make_app_in(prefix: &str, tag: &str) -> App {
        let dir = crate::model::testenv::uniq_tmp_dir(&format!("{prefix}-{tag}"));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        App::new(Config::default(), reg)
    }

    pub(crate) fn task_in(id: u32, name: &str, state: TaskState) -> Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t.state = state;
        t.total = 3000;
        t.downloaded = 1500;
        t.probed = true;
        t
    }

    pub(crate) fn render_full(app: &mut App, w: u16, h: u16) -> Terminal<TestBackend> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| super::draw(f, app)).unwrap();
        term
    }

    pub(crate) fn render_frame_string(app: &mut App, w: u16, h: u16) -> String {
        render_full(app, w, h).backend().to_string()
    }

    pub(crate) fn row_strings(term: &Terminal<TestBackend>) -> Vec<String> {
        let buf = term.backend().buffer();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| {
                        buf.cell((x, y))
                            .expect("网格坐标在界内")
                            .symbol()
                            .to_string()
                    })
                    .collect::<String>()
            })
            .collect()
    }
}
