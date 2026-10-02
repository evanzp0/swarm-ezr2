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
#![allow(missing_docs)] // 交互层：demo 定稿基线复用，接口文档见 model/engine 层
#![allow(clippy::pedantic)] // 交互层字节/速度展示算术与 demo 基线风格豁免
#![allow(clippy::nursery)] // 同上
#![allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::too_many_arguments
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

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

use detail::{draw_chart, draw_detail};
use dialog::draw_dialogs;
use header::{draw_footer, draw_header, draw_tabs};
use list::draw_list;

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
    use crate::model::{Checksum, Protocol, Task};

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
}
