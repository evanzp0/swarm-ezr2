//! ui.rs — EZR Downloader 的界面渲染
//!
//! 布局（定稿口径，FR-01-94/95/96/98，对齐 ezr-demo 八轮定稿）：
//! 顶部带 5 行 = 头部仪表面板通栏全宽（标题边框行 + 字段/分割线/页签 3 内容行 +
//! 底边框；字段单行：并发、会话已下载、全局 ↓↑、峰值 ↓↑；页签行 = 原「任务队列」
//! 面板并入：正在下载 (n) │ 已完成 (n) │ 下载槽位 x/y 内联左对齐）；内容区右侧 =
//! │ 竖线分隔符（贯通全高）+ 左右各 1 列空白 + 内嵌无边框单色面积流量图
//! （占内容区全高 3 行、宽 20%，仅绘下行流量，八级部分块 ▁▂▃▄▅▆▇█ 亚字符
//! 精度，全图仅一种颜色 = 下载淡蓝）。主区：左列 58% = 任务列表（占满全高）；
//! 右列 42% = 任务详情（10 行内容 + 边框 = 定高 12 行；v1.20：任务 ID 与
//! 创建时间拆分两行）及并发连接面板
//! （占右列余下全部高度；仅「下载中」显示明细，HTTP 三列：序号/下载速度/
//! 累计下载）。页脚 = 快捷键与提示（G 提示「面板」）。窄终端 / 紧凑布局
//! （G 键，FR-01-98）降级：头部与列表满宽、右栏两面板不渲染（流量图仅在
//! 窄终端时隐藏；紧凑布局下头部流量图常显）。
//! 每个任务条目 3 行：标题行(名称+协议/状态徽标+百分比)、整体进度条(不分块)、统计行。
//! 协议徽标 [HTTP/HTTPS] 统一黄色；详情分块以文字显示 x/y（已完成/总块数）与块大小 N/块。
//! 对话框：添加任务(URL+目录+并发+校验算法下拉+校验码+代理下拉) / 修改任务 /
//! 删除任务(仅任务|任务和文件|取消)。

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, BorderType, Borders};
use ratatui::Frame;

use crate::app::App;
use crate::model::TaskState;

mod btn;
mod chart;
mod conns;
mod delete;
mod detail;
mod dialog;
mod footer;
mod header;
mod help;
mod list;
mod pieces;
mod task_lines;
mod text;

// 测试可达性门面：property_tests 属性测试需触达 text 纯函数（显示宽度不变量/
// 截断·填充幂等/时长往返/格式化稳定性），经 cfg(test) 门控 re-export
// （notes/rust.md「门面 re-export」手法）；产品构建（cfg(test)=off）不产生
// 该路径，外部 API 面零增量。
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
    fmt_dur, fmt_eta, fmt_size, fmt_size_pair, fmt_speed, pad_right, truncate, w,
};

// ---------------------------------------------------------------------------
// 调色板（语义状态色；下载中=淡蓝，暂停=白，等待=黄，失败=红，完成=绿，做种=粉，
// 挂起（失败）=白——v1.19/FR-01-92 裁决，与暂停同色承载静置语义）
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

/// 聚焦面板边框色（lazygit 主题默认 activeBorderColor = green + bold，
/// v1.30/FR-01-108）
pub(super) const FOCUS_BORDER: Color = Color::Green;

/// 是否存在浮层（对话框/pieces 弹窗/帮助窗口）：浮层打开时底层面板全部
/// 降级为暗边框，焦点面板不再高亮（视觉焦点让位给模态浮层，FR-01-107/108）
pub(super) fn overlay_open(app: &crate::app::App) -> bool {
    app.overlay_open()
}

/// 面板边框样式：聚焦 = 绿色加粗（lazygit activeBorderColor 口径），
/// 未聚焦/浮层打开 = 暗色（inactiveBorderColor 口径，v1.30/FR-01-108）
pub(super) fn panel_border_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(FOCUS_BORDER)
            .add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        Style::default().fg(BORDER)
    }
}

/// 面板标题样式：聚焦 = 绿色加粗，未聚焦/浮层打开 = 暗灰（v1.30/FR-01-108）
pub(super) fn panel_title_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(FOCUS_BORDER)
            .add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        Style::default().fg(DIM)
    }
}

/// lazygit 式滚动条（右侧边框列：▲/▼ 上下箭头 + │ 轨道 + █ 滑块，
/// v1.30/FR-01-109）。
///
/// * `panel`：面板完整区域（含边框）——滚动条画在右边框列、上下角之间的行；
/// * `total`/`page`：内容总项数 / 可视项数（total <= page 时不绘制）；
/// * `position`：滚动偏移（内部钳制到 0..=total-page）；
/// * `thumb`：滑块颜色（跟随便属面板边框色：聚焦面板/弹窗均为高亮绿）。
///
/// 显示条件：total > page 且面板高 ≥ 5 且可绘行数 ≥ 3。
/// 滑块位置口径同 lazygit gocui scrollbar.go：顶/底端吸附，中间按比例取整、
/// 末页前不吸底（ceil 保证下滑即移动，-1 预留行程）；滑块长度下取整
/// （规格 FR-01-109：以代码为准 = floor）。
pub(super) fn lazy_scrollbar(
    f: &mut Frame,
    panel: Rect,
    total: usize,
    position: usize,
    page: usize,
    thumb: Color,
) {
    if page == 0 || total <= page || panel.height < 5 || panel.width == 0 {
        return;
    }
    let col = panel.x + panel.width - 1; // 右边框列
    let top = panel.y + 1; // 上角之下
    let bottom = panel.y + panel.height - 1; // 下角（不含）
    let h = (bottom.saturating_sub(top)) as usize; // 可绘行数（▲+轨道+▼）
    if h < 3 {
        return;
    }
    let track = h - 2; // 轨道行数（扣除上下箭头）
    let thumb_h = ((((page as f64) / (total as f64)) * track as f64) as usize).clamp(1, track);
    let max_pos = total - page;
    let position = position.min(max_pos);
    let thumb_y = if position >= max_pos {
        track - thumb_h
    } else {
        (((position as f64 / max_pos as f64) * (track - thumb_h - 1) as f64).ceil()) as usize
    }
    .min(track.saturating_sub(thumb_h));
    // 字形全部取格内居中（▲ │ █ ▼ 同列垂直对齐）：滑块用全块 █ 而非
    // 左对齐的半块 ▐——半块贴左缘会与居中的轨道/箭头错位（对齐修复）
    let arrow_style = Style::default().fg(DIM);
    let track_style = Style::default().fg(DIM2);
    let thumb_style = Style::default()
        .fg(thumb)
        .add_modifier(ratatui::style::Modifier::BOLD);
    for (i, y) in (top..bottom).enumerate() {
        let (sym, style) = if i == 0 {
            ("▲", arrow_style)
        } else if i == h - 1 {
            ("▼", arrow_style)
        } else {
            let t = i - 1; // 轨道行下标 0..track
            if t >= thumb_y && t < thumb_y + thumb_h {
                ("█", thumb_style)
            } else {
                ("│", track_style)
            }
        };
        if let Some(cell) = f.buffer_mut().cell_mut((col, y)) {
            cell.set_symbol(sym);
            cell.set_style(style);
        }
    }
}

/// 状态 → 语义色映射表（8 态全显式映射。v1.19/FR-01-92 徽标色裁决：失败挂起
/// 映射白——与 Paused 同色承载「操作者主动静置」语义；v1.21 合并口径：失败
/// 挂起不再独立状态，数据与展示均同普通已暂停（v1.22/v1.23）；徽标文案
/// 亦不细分（state.label() 单源），挂起态展示与普通已暂停完全一致。DIM 兜底
/// 保留仅防御未来新增态漏配，由 state_color_covers_every_variant_without_fallback
/// 穷尽守卫测试拦截）
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

/// 圆角全边框面板（各面板共用样板，CPD tile 处置：coder cpd-tile-shared-helper 轮）：
/// 边框前景色取入参；标题由调用方按需续链 `.title(...)`。
pub(super) fn bordered_block(fg: Color) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(fg))
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
        Constraint::Length(5), // 顶部带：头部仪表面板通栏全宽 5 行（FR-01-94）
        Constraint::Min(8),    // 主区：任务列表 | 任务详情 + 并发连接面板
        Constraint::Length(3), // 页脚：单行 H 帮助 · Q 退出 + toast（v1.30/FR-01-107）
    ])
    .split(area);

    let wide = area.width >= 100;
    // 焦点钳制（v1.30/FR-01-108 ④）：详情/并发面板未渲染（紧凑布局 G /
    // 窄终端 <100 列）时焦点只允许落在任务列表（与 G 键处理同口径，双保险）
    if !wide || !app.show_panes {
        app.focus = crate::app::Panel::List;
    }
    if wide {
        // 头部仪表面板：通栏全宽；字段行 + 分割线 + 页签行，右侧内嵌无边框
        // 单色面积流量图（占内容区全高、20% 宽）。FR-01-98：流量图常显，
        // 不再随 G 键隐藏
        draw_header(f, app, outer[0], true);

        if app.show_panes {
            // 主区：左列 58% = 任务列表（自头部正下方起、占满左列全高）；
            // 右列 42% = 任务详情（定高 12 行，v1.20）+ 并发连接面板（占余下全部高度）
            let cols = Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
                .split(outer[1]);
            draw_list(f, app, cols[0]);
            let right =
                Layout::vertical([Constraint::Length(12), Constraint::Min(6)]).split(cols[1]);
            draw_detail(f, app, right[0]);
            draw_conns(f, app, right[1]);
        } else {
            // G 键紧凑布局（FR-01-98）：流量图保留在头部，仅收起任务详情 /
            // 并发连接面板，任务列表占满整行
            app.conns_area = None;
            app.visible_conns_rows = 0;
            // FR-01-101：详情字段名热区随面板收起清空（点击无动作）；
            // 详情面板区域同步清空（点击聚焦失效，v1.30/FR-01-108）
            app.detail_url_rect = None;
            app.detail_ck_rect = None;
            app.detail_area = None;
            draw_list(f, app, outer[1]);
        }
    } else {
        // 窄终端（< 100 列）：头部面板无图（分割线与页签行贯通全宽），任务列表占满整行
        app.conns_area = None;
        app.visible_conns_rows = 0;
        // FR-01-101：详情不渲染 → 字段名热区清空；详情面板区域同步清空
        app.detail_url_rect = None;
        app.detail_ck_rect = None;
        app.detail_area = None;
        draw_header(f, app, outer[0], false);
        draw_list(f, app, outer[1]);
    }

    draw_footer(f, app, outer[2]);

    // 对话框最后绘制（覆盖层）
    if app.dialog.is_some() {
        draw_dialogs(f, app, area);
    }

    // pieces 分块弹窗（覆盖层；与对话框互斥——按键层已保证，FR-01-106 ②；
    // 打开/关闭/拖拽帧全量重绘由 main.rs overlay_sig 保证）
    if app.pieces_open {
        draw_pieces(f, app, area);
    }

    // 帮助窗口（最上层覆盖，v1.30/FR-01-107；模态：打开时底层面板暗框）
    if app.help_open {
        draw_help(f, app, area);
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

    /// 8 态全态入表守卫（match 空臂 = 编译期穷尽锚：TaskState 新增变体时本测试
    /// 编译失败强制回看；DIM 兜底断言防漏配再犯——v1.6/FR-01-92 失败挂起
    /// 曾漏配落 DIM 的缺口教训；v1.21 合并后 FailedPaused 变体退役）
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
            // 四角 = Rounded 圆角、四边齐全、前景色 = 入参色
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
            // 内部未被绘制
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
        app.show_panes = true;
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = term.backend().to_string();
        assert!(!s.contains("并发分块明细"), "明细表段落已移除: {s}");
        assert!(s.contains("分块"), "任务级分块字段行保留");
        app.shutdown().await;
    }

    /// v1.11/FR-01-80 修订（操作者第八批指令）：详情面板移除「状态」「速度」字段行。
    /// 单独渲染 draw_detail 隔离断言——整帧含图表标题「全局速度」（含「速度」子串）
    /// 与列表速度值，整帧负向断言不适用；隔离面板内两标签必须完全不出现。
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
        assert!(s.contains("保存"), "保存行保留: {s}");
        app.shutdown().await;
    }

    /// v1.12/FR-01-80 修订（操作者第九批指令）：详情「大小」行移除「（剩余 …）」后缀，
    /// 收敛为「已下载/总大小」。隔离渲染 draw_detail：downloaded<total 时旧实现必渲染
    /// 「（剩余 …）」，负向断言先红；进度展示由列表行承载（FR-01-17 口径不变）。
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
        assert!(s.contains("添加下载任务"), "对话框标题");
        assert!(s.contains("SHA-256"), "算法行显示当前算法");
        // v1.15/FR-01-103 三钮：立即下载 / 仅添加 / 取消
        assert!(s.contains("立即下载"), "立即下载按钮");
        assert!(s.contains("仅添加"), "仅添加按钮");
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
        app.show_panes = true;
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
        app.show_panes = true; // 详情面板仅在宽布局 + 图表开启分支渲染
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

        // 已暂停 → 继续：一律重新排队（v1.21/FR-01-33 修订；此处槽位满 1/1）
        app.tasks.push(task(2, "p2.bin", TaskState::Downloading));
        app.tasks[1].has_slot = true;
        app.toggle_pause(); // 选中仍是 0（p1 已暂停）
        assert_eq!(
            app.tasks[0].state,
            TaskState::Queued,
            "继续 = 重新排队（统一路径，不检测空闲槽位）"
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

        // 校验失败 → R（FR-01-51 v1.17 三分支）：显式值在（无伴随）→ 重新校验：
        // 直接转校验中并占槽位（D10 无块重传）
        app.tasks[0].state = TaskState::Failed;
        app.tasks[0].fail_kind = Some(FailKind::Verify);
        app.tasks[0].checksum = Some(crate::model::Checksum {
            algo: "SHA-256",
            value: "ab".repeat(32),
        });
        app.retry();
        assert_eq!(app.tasks[0].state, TaskState::Verifying);
        assert!(app.tasks[0].has_slot, "重校验占槽位");

        // 校验失败 + 校验码已清空（无伴随）→ R = 不校验（FR-01-51 v1.17③/D34）：
        // 正常重新排队落等待中（旧实现发空期望 Verify、空串比对恒败，R 后仍
        // 显示校验失败——操作者 20261009 第十四批指令缺陷 B）
        app.tasks[0].state = TaskState::Failed;
        app.tasks[0].fail_kind = Some(FailKind::Verify);
        app.tasks[0].checksum = None;
        app.retry();
        assert_eq!(app.tasks[0].state, TaskState::Queued);
        assert!(!app.tasks[0].has_slot, "等待排队不占槽");
        // 恢复校验中态，供下方「校验中不受 Space 影响」兜底臂断言
        app.tasks[0].state = TaskState::Verifying;
        app.tasks[0].has_slot = true;

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

    /// v1.6/FR-01-92/D25 + v1.21 合并口径：失败任务 Space 暂停（挂起自动重试）
    /// ↔ 恢复重新排队；v1.23（操作者 20261012 bug 报告直判）：挂起时即重置
    /// error=None、retries=1——数据层与普通暂停完全一致（注册表快照同口径）；
    /// R 对挂起态同样无效
    #[tokio::test]
    async fn failed_pause_resume_flow() {
        let mut app = make_app("fpause");
        app.max_slots = 1;

        // 失败（倒计时进行中）→ Space = 暂停：转 Paused、清倒计时、释放槽位，
        // 并重置 error/retries（v1.23：数据层与普通暂停完全一致）
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
        // v1.19/FR-01-92 收窄口径的引导文案断言：挂起引导只指空格，
        // 不得回退「按空格或 R 恢复」旧文案（R 不适用本态）
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("按空格恢复") && !m.contains("或 R")),
            "挂起引导文案应只指空格（v1.19）: {:?}",
            app.toast
        );

        // tick 不推进挂起任务（Paused 不参与 retry_in 到点重排；v1.23 起迟到
        // 失败事件亦被 Paused 全态守卫忽略，不写回 error）
        assert!(
            !app.tasks.iter().any(|t| t.state == TaskState::Queued),
            "挂起任务不重新排队"
        );

        // 已暂停（失败挂起）→ Space = 恢复：重新排队（error/计数已随挂起重置，
        // 恢复路径与普通暂停无差别）
        app.toggle_pause();
        assert_eq!(app.tasks[0].state, TaskState::Queued, "恢复 = 重新排队");
        assert_eq!(app.tasks[0].retries, 1, "重新排队计数重置");
        assert!(app.tasks[0].error.is_none());

        // R 对挂起态（Paused）无效（v1.19/FR-01-92 收窄：空格为唯一
        // 恢复键）——状态保持、字段不动、仅给兜底提示
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

    /// v1.21/FR-01-33 修订（操作者 20261012 指令）：已暂停 → 继续 = 一律重新
    /// 排队——「有空闲槽直接开跑（Downloading + Start）」路径废除，有空闲槽
    /// 也进等待队列；断点数据不丢（获槽后从断点续传）
    #[tokio::test]
    async fn paused_resume_always_requeues_even_with_free_slot() {
        let mut app = make_app("requeue");
        app.max_slots = 5; // 全部空闲：旧行为会直接 Downloading + Start
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
        // 不支持续传的任务同路径，toast 指明从头下载
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

        // q 退出；Esc 主界面无操作（v1.30/FR-01-80：Esc 仅用于关闭浮层）
        app.on_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(app.quit);
        app.quit = false;
        app.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.quit, "Esc 主界面无操作（v1.30）");
        assert!(!app.help_open && !app.pieces_open && app.dialog.is_none());

        // Tab/BackTab 面板焦点循环（v1.30/FR-01-108；页签切换由 ]/[/T 承接）
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

        // 页签键：] / [ / T（v1.30/FR-01-80）
        app.on_key(KeyCode::Char(']'), KeyModifiers::NONE);
        assert_eq!(app.filter, 1, "] 到已完成页签");
        app.on_key(KeyCode::Char('['), KeyModifiers::NONE);
        assert_eq!(app.filter, 0, "[ 回首");
        app.on_key(KeyCode::Char('T'), KeyModifiers::NONE);
        assert_eq!(app.filter, 1, "T 循环");
        app.on_key(KeyCode::Char('T'), KeyModifiers::NONE);
        assert_eq!(app.filter, 0);

        // H 帮助窗口开/关（v1.30/FR-01-107）
        app.on_key(KeyCode::Char('h'), KeyModifiers::NONE);
        assert!(app.help_open, "H 打开帮助");
        app.on_key(KeyCode::Char('h'), KeyModifiers::NONE);
        assert!(!app.help_open, "H 再按关闭");

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
        let panes = app.show_panes;
        app.on_key(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(app.show_panes, !panes, "g 切换右栏两面板（FR-01-98）");

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

    /// 已暂停 → 继续：一律重新排队（v1.21/FR-01-33 修订，直开路径废除；
    /// 续传双文案臂 + Completed 兜底臂；不可续传任务 toast 指明从头下载）
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

        // 可续传：重新排队 + 断点文案（不再直接 Downloading）
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

        // 不可续传：从头下载文案（同样重新排队）
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

        // Completed 兜底臂：提示无需操作（完成任务在「已完成」页签可见）
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

    /// Modify 对话框渲染（FR-01-87；cleaner v129 补测：此前仅 Add/Delete 有渲染
    /// 金样，Modify 渲染路径（is_add=false 分支族）无任何测试触达）。固化现状：
    /// 标题/字段行/任务算法与代理选择/确定·取消钮/Modify 提示行（与 Add 相异文案）。
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
        // v1.30/FR-01-107：底部操作提示行退役（P3 收敛，交互说明收敛帮助窗口）
        assert!(
            !s.contains("确定后立即生效"),
            "Modify 提示行退役（v1.30）: {s}"
        );
        assert!(!s.contains("添加下载任务"), "不得渲染 Add 标题: {s}");
        app.shutdown().await;
    }
}

#[cfg(test)]
mod ui_v113_tests {
    //! v1.13 定稿布局渲染测试（FR-01-94/95/96/98）：通栏 5 行头部带、单色面积
    //! 流量图、并发连接面板、G 键紧凑布局与窄终端降级。

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

    /// 逐单元格 (符号, 前景色) 网格
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
        // 布局推算（120 宽）：头部内框 x=1..119（宽 118）→ 图宽 23、字段区 92、
        // │ 分隔符 x=93、图区 x=95..118
        let mut partials = 0;
        let mut foreign_color = false;
        for row in &g[1..4] {
            for &(ch, fg) in &row[95..118] {
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
        // 顶线含部分块字符（亚字符精度，N2l 口径）
        let top_line: String = (95..118).map(|x| g[1][x].0).collect();
        assert!(
            top_line.chars().any(|c| ('▁'..='▇').contains(&c)),
            "顶线含部分块字符（1/8 格精度）: {top_line}"
        );
        // 分隔符贯通内容区全高 + 左右各 1 列空白
        for (y, row) in g.iter().enumerate().take(4).skip(1) {
            assert_eq!(row[93].0, '│', "│ 分隔符贯通全高 y={y}");
            assert_eq!(row[94].0, ' ', "分隔符右侧（图区侧）1 列空白");
        }
        // 全帧无盲文点阵字符
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
        assert!(s.contains("活跃 0"), "待命连接速度 0 → 活跃 0");
        assert!(s.contains("序号"), "HTTP 表头 序号 列");
        assert!(s.contains("下载速度") && s.contains("累计下载"), "数据列");
        // v1.30/FR-01-107：提示行整体退役（P3 收敛，交互说明收敛帮助窗口）
        assert!(!s.contains("Ctrl+↑↓"), "提示行退役（v1.30）");
        assert!(!s.contains("Ctrl+B"), "Ctrl+B 提示退役（v1.30）");
        assert!(!s.contains("（仅 BT）"), "「（仅 BT）」标注退役");
        assert!(
            rows.iter()
                .any(|r| r.trim().contains(" 1 ") && r.contains('-')),
            "数据行按序号展示、待命速度显示 -"
        );
        // 详情定高 12 行（v1.20）：右列详情 y=5..17，明细面板标题在 y=17
        let row17: String = rows[17].replace(' ', "");
        assert!(
            row17.contains("并发连接"),
            "明细面板紧随详情 12 行: {}",
            rows[17]
        );
        // 状态门槛：已暂停 → 空面板 + 活跃 0 + 面板不接收滚轮
        app.tasks[0].state = TaskState::Paused;
        let term = render(&mut app, 120, 40);
        let s = term.backend().to_string();
        assert!(s.contains("（无并发连接）"), "非下载态空面板: {s}");
        assert!(s.contains("活跃 0"));
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
        // 窄终端：头部无图、列表满宽、明细不渲染
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
    //! v1.14 详情面板测试（FR-01-100/101，对齐 ezr-demo 源码修订-3/5/6/7）：
    //! 类型行去并发数；「校验」「URL」字段名淡蓝下划线链接样式；热区每帧回填与清空。

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

    /// 整帧渲染（draw 全流程，走 G/窄终端分支）
    fn render_full(app: &mut App, w: u16, h: u16) -> Terminal<TestBackend> {
        super::testfx::render_full(app, w, h)
    }

    fn row_strings(term: &Terminal<TestBackend>) -> Vec<String> {
        super::testfx::row_strings(term)
    }

    /// (前景色, 修饰) 网格（下划线断言用）
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

    /// FR-01-100（demo 修订-3）：类型行不再展示并发数——下载中任务（含活跃连接）
    /// 类型行 = `协议名 · 支持断点续传`，全面板无「并发」字样与并发数值。
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
        // 手工逐格拼接对 CJK 含续格占位（buffer_view 跳过）→ 去空格后断言
        // inner(1,1) 起：0 名 / 1 任务ID / 2 创建时间 / 3 类型（v1.20 两行拆分）
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
        // to_string() 跳过续格（CJK 连续）：resumable=true 显示支持口径
        let all = term.backend().to_string();
        assert!(all.contains("HTTP · 支持断点续传"), "类型行定稿形态: {all}");
        assert!(!all.contains("不支持断点续传"), "resumable=true 无否定口径");
        app.shutdown().await;
    }

    /// FR-01-101（demo 修订-5/6）：「校验」「URL」字段名 = 淡蓝 RGB(122,185,242) +
    /// 下划线（仅覆盖文字部分）；其余字段名暗灰无下划线。热区随行回填（x=inner.x+1、
    /// 宽 9；无校验行 → ck 热区 None）。
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

        // 行结构（inner y=1 起）：1 名 / 2 任务ID / 3 创建时间 / 4 类型 / 5 校验 /
        // 6 大小 / 7 保存 / 8 URL / 9 分块（v1.20：任务 ID/创建时间拆两行）
        let underlined = |x: usize, y: usize| -> (Color, bool) {
            let (fg, m) = g[y][x];
            (fg, m.contains(Modifier::UNDERLINED))
        };

        // 「校验」字段名（CJK 2 字各占 2 格，首格 x=2/4）：淡蓝 + 下划线
        for x in [2, 4] {
            let (fg, ul) = underlined(x, 5);
            assert_eq!(fg, LIGHT_BLUE, "校验字段名淡蓝 x={x}");
            assert!(ul, "校验字段名带下划线 x={x}");
        }
        // 填充空格（x≥6）不带下划线（修订-6：下划线仅覆盖文字部分）
        for x in [6, 7, 8] {
            let (_, ul) = underlined(x, 5);
            assert!(!ul, "校验填充空格无下划线 x={x}");
        }
        // 「URL」字段名（x 2..5）：淡蓝 + 下划线；填充空格无下划线
        for x in [2, 3, 4] {
            let (fg, ul) = underlined(x, 8);
            assert_eq!(fg, LIGHT_BLUE, "URL 字段名淡蓝 x={x}");
            assert!(ul, "URL 字段名带下划线 x={x}");
        }
        for x in [5, 7, 9] {
            let (_, ul) = underlined(x, 8);
            assert!(!ul, "URL 填充空格无下划线 x={x}");
        }
        // 其余字段名仍暗灰无下划线（类型行「类」字）
        let (fg, ul) = underlined(2, 4);
        assert_eq!(fg, DIM, "类型标签仍暗灰");
        assert!(!ul, "类型标签无下划线");

        // 热区回填：x = inner.x + 1 = 2、宽 9、高 1；y = inner.y + 行号
        assert_eq!(
            app.detail_url_rect,
            Some(ratatui::layout::Rect::new(2, 8, 9, 1)),
            "URL 热区"
        );
        assert_eq!(
            app.detail_ck_rect,
            Some(ratatui::layout::Rect::new(2, 5, 9, 1)),
            "校验热区"
        );
        app.shutdown().await;
    }

    /// FR-01-101：无校验码任务无「校验」热区（URL 热区仍在）；无选中任务热区双清空。
    #[tokio::test]
    async fn hot_rects_cleared_for_no_checksum_and_no_selection() {
        // 无校验码任务
        let mut app = make_app("t14-nock");
        app.tasks.push(task(1, "nock.bin", TaskState::Downloading));
        let term = render_detail(&mut app, 110, 24);
        assert!(term.backend().to_string().contains("URL"), "URL 行在位");
        assert!(app.detail_url_rect.is_some(), "URL 热区在位");
        assert!(app.detail_ck_rect.is_none(), "无校验行 → 校验热区 None");

        // 无选中任务：热区双清空
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

    /// FR-01-101：G 收起右栏两面板（show_panes=false）与窄终端（<100 列）不渲染详情
    /// → 整帧渲染后热区双清空（点击无动作）。
    #[tokio::test]
    async fn hot_rects_cleared_when_layout_hides_detail() {
        // G 紧凑布局
        let mut app = make_app("t14-g");
        app.tasks.push(task(1, "g.bin", TaskState::Downloading));
        app.show_panes = false;
        render_full(&mut app, 120, 40);
        assert!(app.detail_url_rect.is_none(), "G 收起 → URL 热区清空");
        assert!(app.detail_ck_rect.is_none(), "G 收起 → 校验热区清空");

        // 窄终端
        let mut app = make_app("t14-narrow");
        app.tasks.push(task(1, "n.bin", TaskState::Downloading));
        render_full(&mut app, 80, 34);
        assert!(app.detail_url_rect.is_none(), "窄终端 → URL 热区清空");
        assert!(app.detail_ck_rect.is_none(), "窄终端 → 校验热区清空");
        app.shutdown().await;
    }

    /// v1.6/FR-01-92（v1.22/v1.23 直判维持）：失败原因行仅「已失败」态展示——
    /// 挂起态与普通已暂停完全一致（不再持有也不再展示错误信息）
    #[tokio::test]
    async fn detail_shows_failure_reason_only_for_failed_state() {
        // 已失败：失败原因行 + 错误文本
        let mut app = make_app("detail-fail-reason");
        let mut t = task(1, "failed.bin", TaskState::Failed);
        t.error = Some("连接被重置".to_string());
        app.tasks.push(t);
        let term = render_detail(&mut app, 110, 24);
        let s = term.backend().to_string();
        assert!(s.contains("失败原因"), "已失败态展示失败原因行: {s}");
        assert!(s.contains("连接被重置"), "失败原因文本: {s}");
        app.shutdown().await;

        // 挂起（普通已暂停，error=None）：不展示失败原因行
        let mut app2 = make_app("detail-paused-no-reason");
        app2.tasks.push(task(2, "paused.bin", TaskState::Paused));
        let term2 = render_detail(&mut app2, 110, 24);
        let s2 = term2.backend().to_string();
        assert!(!s2.contains("失败原因"), "非失败态不展示失败原因行: {s2}");
        app2.shutdown().await;
    }
}

/// 测试基建单源（DRY：三处 cfg(test) 模块的 make_app/task/渲染/行文本助手收口于此）。
/// 仅测试编译；挂载树语义与同文件既有 cfg(test) 模块一致（crate:: 路径点由
/// 各挂载壳提供，与本文件既有测试模块相同约束）。
#[cfg(test)]
pub(crate) mod testfx {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use crate::app::App;
    use crate::model::config::Config;
    use crate::model::{Task, TaskState};

    /// 唯一临时目录 + 默认配置构造 App（prefix 区分调用方测试组）
    pub(crate) fn make_app_in(prefix: &str, tag: &str) -> App {
        let dir = crate::model::testenv::uniq_tmp_dir(&format!("{prefix}-{tag}"));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        App::new(Config::default(), reg)
    }

    /// sample_task 基底 + 展示字段覆盖（total/downloaded/probed 定值）
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

    /// 整帧渲染（draw 全流程，走 G/窄终端分支）
    pub(crate) fn render_full(app: &mut App, w: u16, h: u16) -> Terminal<TestBackend> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| super::draw(f, app)).unwrap();
        term
    }

    /// 整帧渲染并返回缓冲区全文（对话框渲染金样脚手架单源，cleaner v129：
    /// TestBackend 构造 + draw + to_string 三步在 ui_tests 多对话框金样测试重复，
    /// 提取到 testfx 消解渲染脚手架 tile）。
    pub(crate) fn render_frame_string(app: &mut App, w: u16, h: u16) -> String {
        render_full(app, w, h).backend().to_string()
    }

    /// 逐行拼接缓冲区符号文本（CJK 续格自动并入，TestBackend::to_string 语义）
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
