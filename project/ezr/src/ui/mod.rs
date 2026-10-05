//! ui.rs — EZR Downloader TUI Demo 的界面渲染
//!
//! 布局：头部(全局统计) / 页签(正在下载|已完成) / 主区(任务列表 + 详情与速度图) / 页脚(快捷键与提示)
//! 每个任务条目 3 行：标题行(名称+协议/状态徽标+百分比)、整体进度条(不分块)、统计行。
//! 协议徽标 [HTTP/HTTPS/BT] 统一黄色；详情页分块以文字显示 x/y（已完成/总块数）与块大小 N/块。
//! 分块策略：块大小按协议写死（HTTP 1 MB / BT 256 KB），块数与并发数解耦。
//! 对话框：添加任务(URL+目录+并发+校验算法下拉+校验码) / 删除任务(仅任务|任务和文件|取消)。
//! 模块划分：`text`（纯文本工具）/ `task_lines`（列表行构造）/
//! `list`（任务列表区）/ `header`（头部·页签·页脚）/ `detail`（详情与速度图）/
//! `dialog`（对话框与浮层）/ 总入口 [`draw`]。

use ratatui::layout::{Constraint, Layout};
use ratatui::style::Color;
use ratatui::Frame;

use crate::app::App;
use crate::model::TaskState;

mod btn;
mod delete;
mod detail;
mod dialog;
mod header;
mod list;
mod task_lines;
mod text;

// 测试可达性门面：property_tests 属性测试需触达 text 纯函数（显示宽度不变量/
// 截断·填充幂等/时长往返/格式化稳定性），经 cfg(test) 门控 re-export
// （notes/rust.md「门面 re-export」手法）；产品构建（cfg(test)=off）不产生
// 该路径，外部 API 面零增量。
use detail::{draw_chart, draw_detail};
use dialog::draw_dialogs;
use header::{draw_footer, draw_header, draw_tabs};
use list::draw_list;
#[cfg(test)]
pub(crate) use text::{
    fmt_dur, fmt_eta, fmt_size, fmt_size_pair, fmt_speed, pad_right, truncate, w,
};

// ---------------------------------------------------------------------------
// 调色板（语义状态色；下载中=淡蓝，暂停=白，等待=黄，失败=红，完成=绿，做种=粉）
// ---------------------------------------------------------------------------

pub(super) const BORDER: Color = Color::Rgb(56, 78, 86);
pub(super) const FG: Color = Color::Rgb(198, 208, 212);
pub(super) const DIM: Color = Color::Rgb(126, 141, 147);
pub(super) const DIM2: Color = Color::Rgb(88, 102, 108);
pub(super) const SEL_BG: Color = Color::Rgb(22, 42, 48);
pub(super) const ACCENT: Color = Color::Cyan;
pub(super) const GREEN: Color = Color::Green;
pub(super) const YELLOW: Color = Color::Rgb(228, 178, 62);
pub(super) const RED: Color = Color::Rgb(226, 92, 84);
pub(super) const MAGENTA: Color = Color::Rgb(214, 112, 214);
pub(super) const EMPTY: Color = Color::Rgb(54, 68, 74);
/// 下载中/校验中/后期处理（淡蓝）
pub(super) const LIGHT_BLUE: Color = Color::Rgb(122, 185, 242);

/// 状态 → 语义色映射表（覆盖全部 6 态 + 2 预留态）
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

// ---------------------------------------------------------------------------
// 文本工具（CJK 宽度感知）
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 总入口
// ---------------------------------------------------------------------------

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let outer = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(4),
    ])
    .split(area);

    draw_header(f, app, outer[0]);
    draw_tabs(f, app, outer[1]);

    let wide = outer[2].width >= 100;
    if wide && app.show_chart {
        let cols = Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
            .split(outer[2]);
        draw_list(f, app, cols[0]);
        let right =
            Layout::vertical([Constraint::Percentage(58), Constraint::Min(4)]).split(cols[1]);
        draw_detail(f, app, right[0]);
        draw_chart(f, app, right[1]);
    } else {
        draw_list(f, app, outer[2]);
    }

    draw_footer(f, app, outer[3]);

    // 对话框最后绘制（覆盖层）
    if app.dialog.is_some() {
        draw_dialogs(f, app, area);
    }
}

#[cfg(test)]
mod ui_tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;
    use crate::app::CHECKSUM_ALGOS;
    use crate::model::config::Config;
    use crate::model::{Checksum, FailKind, Protocol, Task};

    fn make_app(tag: &str) -> App {
        let dir = std::env::temp_dir().join(format!("ezr-ui-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        App::new(Config::default(), reg)
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

    /// FR-01-81 修订二：并发分块明细表整体移除——下载中含活跃连接时，
    /// 详情面板不得再出现「并发分块明细」段落；任务级「分块」字段行保留。
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
        app.show_chart = true;
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(!s.contains("并发分块明细"), "明细表段落已移除: {s}");
        assert!(s.contains("分块"), "任务级分块字段行保留");
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
        assert!(s.contains("添加下载任务"), "对话框标题");
        assert!(s.contains("SHA-256"), "算法行显示当前算法");
        assert!(s.contains("确认"), "确认按钮");
        assert!(s.contains("取消"), "取消按钮");
        // 展开下拉浮层
        if let Some(d) = app.dialog.as_mut() {
            d.ck_open = true;
        }
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        // 注：TestBackend Display 会把宽字符半格单元以 Hidden 注记隐藏，
        // 断言锚定浮层条目的稳定可见文本（选中指示 + 算法名 + 位数）
        assert!(s.contains("▸ SHA-256"), "下拉选中项: {s}");
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
        app.show_chart = true;
        let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("n1.bin"), "窄布局列表");
        app.shutdown().await;
    }

    /// 详情校验行：超长校验码只显示前 10 位加省略号（窄面板适配）；
    /// verify_status 四态文案（已通过/未通过/校验中/待校验）。
    #[tokio::test]
    async fn draw_detail_checksum_prefix_and_verify_states() {
        let mut app = make_app("ck-row");
        let mut t = task(1, "ck.bin", TaskState::Verifying);
        let value = "0123456789abcdef".repeat(4); // 64 位十六进制
        t.checksum = Some(crate::model::Checksum {
            algo: "SHA-256",
            value: value.clone(),
        });
        app.tasks.push(t);
        app.show_chart = true; // 详情面板仅在宽布局 + 图表开启分支渲染
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();

        // 待校验（None + 非校验中态）
        app.tasks[0].verify_ok = None;
        app.tasks[0].state = TaskState::Paused;
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("0123456789"), "前 10 位: {s}");
        assert!(s.contains("…"), "省略号截断: {s}");
        assert!(!s.contains(&value), "完整校验码不出现: {s}");
        assert!(s.contains("（待校验）"), "待校验文案: {s}");

        // 校验中（None + Verifying）
        app.tasks[0].state = TaskState::Verifying;
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("（校验中）"), "校验中文案: {s}");

        // 已通过
        app.tasks[0].verify_ok = Some(true);
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("（已通过）"), "已通过文案: {s}");

        // 未通过
        app.tasks[0].verify_ok = Some(false);
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(s.contains("（未通过）"), "未通过文案: {s}");
        app.shutdown().await;
    }

    /// 列表内移动（U/J）：过滤视图相邻交换 + 选中跟随 + 顶界 toast（FR-01-32）
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

    /// Space/R/C 任务操作状态机（FR-01-33/34/D10）：
    /// 暂停继续双臂、槽位满进等待队列、失败重试计数重置、校验失败重校验、清理已完成
    #[tokio::test]
    async fn toggle_pause_retry_and_clear_state_flows() {
        let mut app = make_app("ops");
        app.max_slots = 1;

        // 下载中 → 暂停：状态翻转、速度归零、槽位释放
        app.tasks.push(task(1, "p1.bin", TaskState::Downloading));
        app.tasks[0].speed = 123.0;
        app.tasks[0].has_slot = true;
        app.selected = 0;
        app.toggle_pause();
        assert_eq!(app.tasks[0].state, TaskState::Paused);
        assert_eq!(app.tasks[0].speed, 0.0);
        assert!(!app.tasks[0].has_slot, "暂停释放槽位");

        // 已暂停 → 继续：槽位满（另一任务占 1/1）→ 进等待队列
        app.tasks.push(task(2, "p2.bin", TaskState::Downloading));
        app.tasks[1].has_slot = true;
        app.toggle_pause(); // 选中仍是 0（p1 已暂停）
        assert_eq!(
            app.tasks[0].state,
            TaskState::Queued,
            "无空闲槽位进等待队列"
        );
        assert!(!app.tasks[0].has_slot);

        // 等待中 → 暂停：退出等待队列
        app.toggle_pause();
        assert_eq!(app.tasks[0].state, TaskState::Paused);

        // 失败 → R 手动重试：计数重置 1、回等待队列、错误清空
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

        // 校验失败 → R = 重新校验：直接转校验中并占槽位（D10 无块重传）
        app.tasks[0].state = TaskState::Failed;
        app.tasks[0].fail_kind = Some(FailKind::Verify);
        app.retry();
        assert_eq!(app.tasks[0].state, TaskState::Verifying);
        assert!(app.tasks[0].has_slot, "重校验占槽位");

        // C 清理已完成：只移除完成态，其余保留
        app.tasks.push(task(3, "p3.bin", TaskState::Completed));
        app.clear_completed();
        assert_eq!(app.tasks.len(), 2, "仅清理完成任务");
        assert!(!app.tasks.iter().any(|t| t.name == "p3.bin"));

        // 非操作态兜底臂：校验中 Space 提示不可操作
        app.selected = 0;
        app.toggle_pause();
        assert_eq!(
            app.tasks[0].state,
            TaskState::Verifying,
            "校验中不受 Space 影响"
        );
        app.shutdown().await;
    }

    /// v1.6/FR-01-92/D25：失败任务 Space 暂停（挂起自动重试）↔ 恢复重新排队；
    /// R 对「已暂停（失败）」同样有效（= 恢复）
    #[tokio::test]
    async fn failed_pause_resume_flow() {
        let mut app = make_app("fpause");
        app.max_slots = 1;

        // 失败（倒计时进行中）→ Space = 暂停：转 FailedPaused、清倒计时、释放槽位
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
            TaskState::FailedPaused,
            "失败按空格 = 暂停"
        );
        assert!(app.tasks[0].retry_in.is_none(), "倒计时清除，不再自动重试");
        assert!(!app.tasks[0].has_slot, "不占下载槽位");
        assert_eq!(
            app.tasks[0].error.as_deref(),
            Some("连接被重置"),
            "错误信息保留可见"
        );
        assert!(app.toast.as_deref().is_some_and(|m| m.contains("已暂停")));

        // tick 不推进挂起任务（FailedPaused 不参与 retry_in 到点重排）
        assert!(
            !app.tasks.iter().any(|t| t.state == TaskState::Queued),
            "挂起任务不重新排队"
        );

        // 已暂停（失败）→ Space = 恢复：重新排队（计数重置、错误清空）
        app.toggle_pause();
        assert_eq!(app.tasks[0].state, TaskState::Queued, "恢复 = 重新排队");
        assert_eq!(app.tasks[0].retries, 1, "重新排队计数重置");
        assert!(app.tasks[0].error.is_none());

        // R 对 FailedPaused 同样有效
        app.tasks[0].state = TaskState::FailedPaused;
        app.tasks[0].retries = 4;
        app.tasks[0].error = Some("超时".to_string());
        app.retry();
        assert_eq!(app.tasks[0].state, TaskState::Queued, "R 恢复挂起任务");
        assert_eq!(app.tasks[0].retries, 1);
        app.shutdown().await;
    }

    /// 全局键盘路由（FR-01-80）：退出/页签/选择/跳转/图表开关/添加入口，
    /// 对话框打开时整键交给 on_dialog_key（Esc 关闭）
    #[tokio::test]
    async fn global_key_router_arms() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = make_app("keys");
        app.tasks.push(task(1, "k1.bin", TaskState::Downloading));
        app.tasks.push(task(2, "k2.bin", TaskState::Downloading));
        app.tasks[1].state = TaskState::Completed;

        // Ctrl+C 退出
        app.on_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(app.quit, "Ctrl+C 退出");
        app.quit = false;

        // q / Esc 退出
        app.on_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(app.quit);
        app.quit = false;
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.quit);
        app.quit = false;

        // Tab/BackTab 页签循环
        app.on_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.filter, 1, "Tab 到已完成页签");
        app.on_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.filter, 0, "两页签循环回首");
        app.on_key(KeyCode::BackTab, KeyModifiers::NONE);
        assert_eq!(app.filter, 1, "BackTab 环绕到末尾");
        app.on_key(KeyCode::BackTab, KeyModifiers::NONE);
        assert_eq!(app.filter, 0);

        // 选择移动：Down/Up/PageUp/PageEnd/Home/End 钳制
        // filter=0 视图含 k1（k2 已完成被过滤）
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

        // g 图表开关
        let chart = app.show_chart;
        app.on_key(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(app.show_chart, !chart, "g 切换图表面板");

        // a 打开添加对话框；对话框打开后 Esc 关闭（on_dialog_key 路由）
        app.on_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(
            matches!(
                app.dialog,
                Some(crate::app::Dialog {
                    kind: crate::app::DialogKind::Add,
                    ..
                })
            ),
            "a 打开添加对话框"
        );
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.dialog.is_none(), "对话框内 Esc 关闭");
        app.shutdown().await;
    }

    /// 鼠标：列表点击选中、滚轮滚动钳制、对话框字段/下拉/按钮命中
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

        // 点击第 2 项（row 6+4）→ 选中
        app.on_mouse(mk(MouseEventKind::Down(MouseButton::Left), 10, 10));
        assert_eq!(app.selected, 1, "点击选中第二项");
        // 点击列表外不改变选中
        app.on_mouse(mk(MouseEventKind::Down(MouseButton::Left), 10, 2));
        assert_eq!(app.selected, 1);
        // 滚轮：下滚钳制到 max，上滚回收
        app.on_mouse(mk(MouseEventKind::ScrollDown, 0, 0));
        assert_eq!(app.scroll, 0, "0 项可视差时钳制");
        app.on_mouse(mk(MouseEventKind::ScrollUp, 0, 0));
        assert_eq!(app.scroll, 0);

        // 对话框：字段点击聚焦 + 算法下拉点选 + 按钮激活
        app.dialog = Some(crate::app::Dialog {
            kind: crate::app::DialogKind::Add,
            url: String::new(),
            dir: String::new(),
            conns: String::new(),
            conns_edited: false,
            ck_type: 0,
            ck_value: String::new(),
            ck_open: true,
            ck_sel: 0,
            proxy_sel: 0,
            proxy_open: false,
            focus: 0,
            task_name: String::new(),
            task_id: None,
        });
        app.dlg_ck_rects = vec![(Rect::new(2, 3, 20, 1), 4)];
        app.on_mouse(mk(MouseEventKind::Down(MouseButton::Left), 5, 3));
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.ck_type, 4, "下拉点选算法");
        assert_eq!(d.ck_sel, 4);
        assert!(!d.ck_open, "点选后收起下拉");
        app.shutdown().await;
    }
}
