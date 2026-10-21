//! help — 帮助窗口（H 快捷键，v1.30/FR-01-107）渲染层
//!
//! 对齐 ezr-demo m3 draw_help/help_lines：70×24 模态浮层（小终端钳制）、
//! 绿色加粗圆角边框、**八组 44 行**分类交互说明（v1.32/FR-01-107② 修订：
//! 新增【滚动条（通用组件）】组、【全局】组 O 条目、【并发连接】组 ←→ 与
//! 鼠标滚轮两条目、【弹窗】组标题与拖拽条目改版——页脚收敛 P3 原则后
//! 交互说明的唯一载体）、内容超窗时 ↑↓/PgUp·PgDn/滚轮滚动 + 右缘通用滚动条
//! （ScrollablePanel Vertical 实例，滑块恒聚焦绿）、鼠标拖拽移动（位置跨次
//! 打开记忆）、末行滚动提示。

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::btn::{clip_wide_at_edges, dialog_rect};
use super::scrollable_panel::{ScrollablePanel, ScrollbarMode};
use super::text::pad_right;
use super::{panel_border_style, panel_title_style, ACCENT, DIM, FG, FOCUS_BORDER};
use crate::app::{App, SbKey};

/// 帮助窗口固定尺寸（宽×高，含边框）
const HELP_W: u16 = 70;
const HELP_H: u16 = 24;

/// 分类帮助内容（FR-01-107 ②：八组 44 行分类交互说明，逐字对齐 ezr-demo m3
/// help_lines——v1.32 修订：新增【滚动条（通用组件）】组（4 条目）、【全局】组
/// `O` 条目、【并发连接】组 ←→ 与鼠标滚轮两条目、【弹窗】组标题含关于、
/// 拖拽条目改「空白处…（滑块/边框除外）」；BT 相关条目为 phase-02 预留文案，
/// 照录 demo 原文不删改，D26）。每帧重建（行数小、无状态）；键名列用 CJK
/// 感知宽度对齐（16 显示宽）。
fn help_lines() -> Vec<Line<'static>> {
    /// 分组标题（绿色加粗）
    fn sec(t: &str) -> Line<'static> {
        Line::from(Span::styled(
            format!(" 【{t}】"),
            Style::default()
                .fg(FOCUS_BORDER)
                .add_modifier(Modifier::BOLD),
        ))
    }
    /// 键位 + 说明（键列 16 显示宽，CJK 感知对齐；键名青色加粗、说明普通前景）
    fn kd(k: &str, d: &str) -> Line<'static> {
        Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled(
                pad_right(k, 16),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(d.to_string(), Style::default().fg(FG)),
        ])
    }
    /// 补充说明（暗色）
    fn note(s: &str) -> Line<'static> {
        Line::from(Span::styled(s.to_string(), Style::default().fg(DIM)))
    }
    vec![
        sec("全局"),
        kd("Tab / BackTab", "切换面板（任务列表 → 详情 → 并发连接）"),
        kd("鼠标左键", "点击面板聚焦；点击任务/连接选中明细"),
        kd("页脚命令", "A/D/M/Q/H 图例可点击：按压高亮，松开执行"),
        kd("H", "打开 / 关闭本帮助窗口"),
        kd("O", "打开 / 关闭关于窗口（About）"),
        kd("G", "收起/展开 任务详情 + 并发连接面板"),
        kd("Esc", "关闭弹窗"),
        kd("Q", "退出程序（主界面；弹窗内先关窗）"),
        sec("页签"),
        kd("] / [", "右/左切换页签（正在下载 ⇄ 已完成）"),
        kd("鼠标点击页签", "切换到该页签"),
        kd("T", "循环切换页签（与 ] 等效）"),
        sec("任务列表"),
        kd("↑ / ↓", "选择上/下一个任务"),
        kd("PgUp / PgDn", "翻页选择（Home / End 跳首/尾）"),
        kd("鼠标滚轮", "滚动列表（视口自由浏览全表）"),
        kd("A / M / D", "添加 / 修改 / 删除任务"),
        kd("Space / R", "暂停·继续 / 重试"),
        kd("U / J", "上移 / 下移任务"),
        kd("C", "清除已完成任务"),
        kd("P", "查看分块状态（仅分块下载任务）"),
        sec("任务详情"),
        kd("鼠标点击", "URL / 校验 字段名复制对应值"),
        sec("并发连接"),
        kd("↑ / ↓", "选择连接明细（视口跟随滚动）"),
        kd("← / →", "横向滚动明细列（客户端/协议等超宽列）"),
        kd("鼠标滚轮", "悬停并发面板 = 滚动连接明细（±2/格）"),
        kd("B", "阻塞并移除选中连接（仅 BT 任务）"),
        sec("滚动条（通用组件）"),
        kd(
            "拖拽滑块 ═║",
            "按住双线滑块拖动 = 比例滚动内容（浮层内同样滚内容）",
        ),
        kd("点击 ↑↓←→", "步进滚动 1 格；按住不放自动连发"),
        kd("点击轨道", "朝点击方向整页滚动（可按住连发）"),
        kd("方向键", "面板聚焦时 ↑↓ 选择/纵向滚动、←→ 横向滚动（并发）"),
        sec("弹窗（添加/修改/删除/pieces/帮助/关于）"),
        kd("Tab / ↑↓", "切换字段/焦点（下拉框内选择选项）"),
        kd("Enter", "确认"),
        kd("Esc", "取消/关闭"),
        kd("鼠标拖拽", "按住弹窗内空白处拖动移动窗口（滑块/边框除外）"),
        sec("pieces 分块弹窗"),
        kd("↑↓ / PgUp·PgDn", "滚动分块网格（Home / End 跳首/尾）"),
        kd("Esc / P", "关闭弹窗"),
        note(""),
        note(" ↑↓/滚轮 滚动本页 · Esc/H 关闭"),
    ]
}

/// 帮助窗口（H）：模态浮层——打开时底层面板降级暗框；绿色高亮边框（与
/// 主面板聚焦色统一）；分类交互说明；内容超窗时 ↑↓/PgUp/PgDn/滚轮滚动 +
/// 右缘通用滚动条（v1.32/FR-01-109 ScrollablePanel Vertical 实例）；支持
/// 鼠标拖拽移动（位置跨次记忆）、点击外部 = 无任何操作（模态统一口径）。
pub(super) fn draw_help(f: &mut Frame, app: &mut App, area: Rect) {
    // 可拖拽定位：记忆位置优先（钳制在终端内），首帧居中并回填；
    // 帮助窗口位置跨次打开记忆（关闭不清除，与 pieces 关闭即清不同）
    let rect = match app.help_pos {
        Some((px, py)) => {
            let dw = HELP_W.min(area.width);
            let dh = HELP_H.min(area.height);
            Rect {
                x: px.min(area.width.saturating_sub(dw)),
                y: py.min(area.height.saturating_sub(dh)),
                width: dw,
                height: dh,
            }
        }
        None => {
            let r = dialog_rect(area, HELP_W, HELP_H);
            app.help_pos = Some((r.x, r.y));
            r
        }
    };
    f.render_widget(Clear, rect);
    // 浮层边界 CJK 宽字符切割处理（残影根治点，与对话框同口径）
    clip_wide_at_edges(f, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" 帮助 ", panel_title_style(true)));
    // v1.32/FR-01-109：ScrollablePanel 一体化组件（帮助 = Vertical 实例，
    // 滑块恒聚焦绿）；箭头按压高亮随 App 态
    let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
    sb.thumb(FOCUS_BORDER)
        .pressed_v(app.sb_arrow_pressed(SbKey::Help));
    let inner = sb.render_block(f, rect, block);
    app.help_area = Some(rect);

    let lines = help_lines();
    let view_h = inner.height as usize;
    let max_scroll = lines.len().saturating_sub(view_h);
    app.help_scroll = app.help_scroll.min(max_scroll);
    app.help_max_scroll = max_scroll;
    let start = app.help_scroll;
    let shown: Vec<Line> = lines.iter().skip(start).take(view_h).cloned().collect();
    f.render_widget(Paragraph::new(shown), inner);

    // 右缘滚动条（v1.32/FR-01-109：ScrollablePanel 组件收口——内容超窗时
    // 画在窗口右边框列；显示条件由组件内判定；滑块恒聚焦绿）
    sb.vertical(lines.len(), view_h, start);
    app.help_scrollbar = sb.render_bars(f, rect).vertical;
}

/// 帮助内容总行数（渲染层钳制与测试锚定用；单源 [`help_lines`]）
#[cfg(test)]
pub(super) fn help_lines_len() -> usize {
    help_lines().len()
}

#[cfg(test)]
mod tests {
    //! v1.30/FR-01-107 帮助窗口渲染单测：70×24 几何、组标题逐字、
    //! 末行提示、滚动钳制、滚动条、位置跨次记忆。v1.32：八组 44 行
    //! （新增【滚动条（通用组件）】组与【全局】 O 条目等）。

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::super::testfx;
    use crate::app::App;

    fn render_help(app: &mut App, w: u16, h: u16) -> Terminal<TestBackend> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        app.help_open = true;
        term.draw(|f| crate::ui::draw(f, app)).unwrap();
        term
    }

    #[tokio::test]
    async fn help_window_geometry_and_sections() {
        let mut app = testfx::make_app_in("ezr-help", "geom");
        let term = render_help(&mut app, 120, 40);
        let s = term.backend().to_string();
        // 70×24 居中：120 宽 → x=25、40 高 → y=8
        assert!(s.contains("帮助"), "标题: {s}");
        // 首帧可见的前三组标题逐字（FR-01-107 ② v1.32：内容 44 行 > 内框
        // 22 行，首帧恰好显示到【任务列表】组末行——后四组断言在滚动后帧）
        for sec in ["【全局】", "【页签】", "【任务列表】"] {
            assert!(s.contains(sec), "组标题 {sec}: {s}");
        }
        // 关键条目锚点（逐字，v1.32 修订）
        assert!(
            s.contains("切换面板（任务列表 → 详情 → 并发连接）"),
            "全局 Tab 条目: {s}"
        );
        assert!(
            s.contains("打开 / 关闭关于窗口（About）"),
            "全局 O 条目（v1.32 新增）: {s}"
        );
        assert!(
            s.contains("右/左切换页签（正在下载 ⇄ 已完成）"),
            "页签条目: {s}"
        );
        assert!(s.contains("查看分块状态（仅分块下载任务）"), "P 条目: {s}");
        // 滚动到末页：后四组标题 + 并发连接/滚动条组条目 + 末行滚动提示逐字
        app.help_scroll = usize::MAX;
        let mut term2 = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term2.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        let s2 = term2.backend().to_string();
        for sec in [
            "【任务详情】",
            "【并发连接】",
            "【滚动条（通用组件）】",
            "【弹窗（添加/修改/删除/pieces/帮助/关于）】",
            "【pieces 分块弹窗】",
        ] {
            assert!(s2.contains(sec), "滚动后组标题 {sec}: {s2}");
        }
        assert!(
            s2.contains("选择连接明细（视口跟随滚动）"),
            "并发连接 ↑↓ 条目 v1.32 文案: {s2}"
        );
        assert!(
            s2.contains("横向滚动明细列（客户端/协议等超宽列）"),
            "并发连接 ←→ 条目（v1.32 新增）: {s2}"
        );
        assert!(
            s2.contains("悬停并发面板 = 滚动连接明细（±2/格）"),
            "并发连接滚轮条目（v1.32 新增）: {s2}"
        );
        assert!(
            s2.contains("按住双线滑块拖动 = 比例滚动内容"),
            "滚动条组条目（v1.32 新增组；尾括注超内框宽被截，断言可见前段）: {s2}"
        );
        assert!(
            s2.contains("阻塞并移除选中连接（仅 BT 任务）"),
            "B 条目照录（D26 预留）: {s2}"
        );
        assert!(
            s2.contains("↑↓/滚轮 滚动本页 · Esc/H 关闭"),
            "末行滚动提示逐字: {s2}"
        );
        // 窗口区域回填（拖拽/滚轮/命中测试依据）
        assert_eq!(
            app.help_area,
            Some(ratatui::layout::Rect::new(25, 8, 70, 24)),
            "区域回填"
        );
        // 位置首帧回填（跨次记忆基础）
        assert_eq!(app.help_pos, Some((25, 8)), "首帧居中并记忆");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn help_scroll_clamps_to_last_page_and_scrollbar() {
        let mut app = testfx::make_app_in("ezr-help", "scroll");
        let total = super::help_lines_len();
        assert_eq!(total, 44, "v1.32 内容 44 行（八组）");
        app.help_scroll = usize::MAX; // 模拟 End
        let term = render_help(&mut app, 120, 40);
        // 钳制到末页：scroll = total - 22
        assert_eq!(
            app.help_scroll,
            total - 22,
            "End 渲染层钳制到末页（内容 {total} 行 - 内框 22 行）"
        );
        // 右缘滚动条滑块（↑│║│↓，v1.32：双线滑块恒聚焦绿）
        let buf = term.backend().buffer();
        let col = 25 + 70 - 1; // 右边框列
        let thumb = (9..31).any(|y| {
            buf.cell((col, y))
                .map(|c| c.symbol() == "║")
                .unwrap_or(false)
        });
        assert!(thumb, "右缘滚动条滑块出现（内容超窗）");
        // 命中几何回填（鼠标拖拽/箭头/轨道交互依据）
        assert!(app.help_scrollbar.is_some(), "帮助滚动条几何回填");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn help_position_memory_across_reopens() {
        let mut app = testfx::make_app_in("ezr-help", "mem");
        // 首次打开 → 首帧居中回填
        app.help_open = true;
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        let first = app.help_pos;
        assert!(first.is_some(), "首帧回填位置");
        // 关闭（位置保留）
        app.help_open = false;
        // 模拟拖拽到新位置
        app.help_pos = Some((3, 5));
        // 再次打开 → 使用记忆位置（3,5），不重新居中
        app.help_open = true;
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        assert_eq!(app.help_pos, Some((3, 5)), "位置跨次记忆（v1.30）");
        assert_eq!(
            app.help_area,
            Some(ratatui::layout::Rect::new(3, 5, 70, 24))
        );
        app.shutdown().await;
    }

    /// 小终端钳制：窗口缩小到终端尺寸仍可渲染（内框不足时仅边框不 panic）
    #[tokio::test]
    async fn help_clamps_in_small_terminal() {
        let mut app = testfx::make_app_in("ezr-help", "clamp");
        let term = render_help(&mut app, 50, 18);
        let s = term.backend().to_string();
        assert!(s.contains("帮助"), "钳制后仍渲染标题: {s}");
        assert_eq!(
            app.help_area,
            Some(ratatui::layout::Rect::new(0, 0, 50, 18))
        );
        app.shutdown().await;
    }
}
