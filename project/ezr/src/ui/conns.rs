//! conns — 并发连接面板（FR-01-96，对齐 ezr-demo 定稿 5.5；v1.30 修订；
//! v1.32 对齐 ezr-demo m3；v1.34 活跃口径重构）
//!
//! 位置 = 右列、任务详情正下方，占右列余下全部高度。可见性状态门槛（REQ-3.1）：
//! 仅「下载中」（与 BT 预留「做种中」）显示逐连接明细，其余状态空面板居中提示
//! 「（无并发连接）」+ 徽标 x = 0（渲染层门槛：状态下沉时连接数据冻结隐藏，恢复下载
//! 原样续用）。**v1.34/FR-01-96 ⑦：明细只显示活跃并发**（正在连接 = 握手中、
//! 已连接 = 持块在传；待命/已断开不显示；ConnPhase::is_active 活跃判定单源）。
//! 列定义（01 期 HTTP 三列，表头随协议自适应）：
//! 序号（4 列，1 基 = 连接池槽位号，稳定不随断连重编号，明细可能不连续）/
//! 下载速度（8 列右对齐，正在连接显示 `-`）/ 累计下载（8 列右对齐，自本次开始
//! 下载起算，FR-01-99）。**BT 七列为 phase-02 预留形态**：对端 IP 掩码 11 列 /
//! 下载速度 8 / 累计下载 8 / 上传速度 8 / 累计上传 8 / 客户端 6 / 协议 4——
//! 内容宽 59 列超内框 → 底部横向滚动条（唯一 Both 实例）+ ←/→ 键横向滚动；
//! v1.34：明细数据区左右各留 2 列 padding（S2）→ BT 视口宽相应 48→44 列。
//! 右上角徽标 = ` x/y 项 `（v1.34：原「 活跃 x 」退役；x = 活跃明细行数、
//! y = 任务设置的最大并发数 1–64，修改对话框热调；空态 ` 0/y 项 `）。
//! 横向滚动上限放宽 1 列（内容宽 − 视口宽 + 1，末列宽字完整可见）。
//! v1.30 修订：**面板无底部提示行**（P3 原则/FR-01-107）；选中行高亮仅本面板
//! 聚焦时显示（FR-01-108）；滚动条按 FR-01-109 v1.32 通用组件绘制
//! （ScrollablePanel Both 实例，滑块色随焦点）。
//! 连接数超出可视高度时滚轮 / ↑↓（面板聚焦时）翻看。明细为空或未选中任务时
//! 面板不接收滚轮（conns_area 置 None，滚轮穿透滚动任务列表）。

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::scrollable_panel::{ScrollablePanel, ScrollbarMode};
use super::text::{fmt_size, fmt_speed, pad_left};
use super::{
    panel_border_style, panel_title_style, BORDER, DIM, DIM2, FG, FOCUS_BORDER, LIGHT_BLUE, SEL_BG,
};
use crate::app::{App, Panel, SbKey};

/// BT 七列列宽（phase-02 预留形态，v1.32/FR-01-96 修订对齐 ezr-demo m3：
/// 对端 IP 11 / 下载速度 8 / 累计下载 8 / 上传速度 8 / 累计上传 8 /
/// 客户端 6 / 协议 4，列间单空格分隔；01 期无 BT 任务不触发，
/// 列宽常量与内容宽口径先行预留，数据源 phase-02 定夺）
#[allow(dead_code)]
const BT_COLW: [usize; 7] = [11, 8, 8, 8, 8, 6, 4];
/// BT 七列内容总宽（59 列 = 53 数据列 + 6 个单空格分隔；与 [`BT_COLW`]
/// 单源对应性由测试模块 bt_content_w_matches_colw 守卫；超内框 → 水平滚动条
/// + ←/→）
#[allow(dead_code)]
const BT_CONTENT_W: usize = 59;
/// HTTP 三列列宽（序号 4 / 下载速度 8 / 累计下载 8，单空格分隔）
const HTTP_COLW: [usize; 3] = [4, 8, 8];
/// HTTP 三列内容总宽（22 列 = 20 数据列 + 2 个单空格分隔；与 [`HTTP_COLW`]
/// 单源对应性由测试守卫；不超宽 → 无水平滚动条）
const HTTP_CONTENT_W: usize = 22;

/// 右上角徽标（v1.34/FR-01-96 ⑦：` 活跃 x ` → ` x/y 项 `——S1 前后各 1 空格：
/// x = 活跃明细行数（活跃并发数，与明细行严格同源）、y = 任务设置的最大并发数
/// 1–64（添加对话框设定、修改对话框热调；y 不是当前连接总数也不是分块总数）；
/// 空态 ` 0/y 项 `。单源供渲染与形态守卫测试）
fn active_title(active: usize, max_conc: usize) -> String {
    format!(" {active}/{max_conc} 项 ")
}

pub(super) fn draw_conns(f: &mut Frame, app: &mut App, area: Rect) {
    let (active, max_conc, empty) = match app.sel_task() {
        Some(t) => {
            // 状态门槛（REQ-3.1）：仅「下载中/做种中」显示明细，其余按空面板处理
            let shows = t.state.shows_conns();
            // v1.34：x/y 徽标与明细行严格同源（活跃判定 ConnPhase 单源）
            let x = app.active_conn_count(t);
            let shows = if shows { x > 0 } else { false };
            (x, t.concurrency, !shows)
        }
        None => (0, 0, true),
    };
    // 聚焦边框（v1.30/FR-01-108）：并发面板聚焦且无浮层 → 绿色加粗，否则暗色
    let focused = app.focus == Panel::Conns && !app.overlay_open();
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(focused))
        .title(Span::styled(" 并发连接 ", panel_title_style(focused)))
        .title(
            Line::from(Span::styled(
                active_title(active, max_conc),
                Style::default().fg(DIM),
            ))
            .alignment(Alignment::Right),
        );
    // v1.32/FR-01-109：ScrollablePanel 一体化组件（并发面板 = 唯一 Both 实例：
    // 垂直连接行 + 水平内容列）。滑块色随焦点；箭头按压高亮随 App 态。
    let mut sb = ScrollablePanel::new(ScrollbarMode::Both);
    sb.thumb(if focused { FOCUS_BORDER } else { BORDER })
        .pressed_v(app.sb_arrow_pressed(SbKey::ConnsV))
        .pressed_h(app.sb_arrow_pressed(SbKey::ConnsH));
    let inner = sb.render_block(f, area, block);
    app.conns_area = Some(inner);

    // v1.31/FR-01-96 ⑦ 空态二分：未选中任务 → 居中「（未选中任务）」；
    // 选中但无活跃明细（状态门槛或全部待命/断开）→ 居中「（无并发连接）」
    if app.sel_task().is_none() {
        app.visible_conns_rows = 0;
        // 未选中任务：面板不接收滚轮（指针悬停时滚轮滚任务列表）
        app.conns_area = None;
        f.render_widget(
            Paragraph::new(Span::styled("（未选中任务）", Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    // 明细数据（活跃口径）：选中任务的活跃连接行（已按 id 升序 = 连接池槽位
    // 号序；先取拥有值再回填 app，借用收窄——notes/rust.md「路径敏感借用」纪律）
    let rows_data: Vec<(usize, f64, u64, bool)> = app
        .active_conn_ids()
        .iter()
        .filter_map(|cid| {
            let t = app.sel_task()?;
            t.connections.iter().find(|c| c.id == *cid)?;
            let speed = app.conn_speed_of(t.id, *cid);
            // 正在连接（握手/建连中）速度列显示 `-`（FR-01-116 ③：
            // 「正在连接」阶段速度列 `-` 语义保留）
            let cum = app.conn_cum_of(t.id, *cid);
            Some((*cid, speed, cum, cum > 0))
        })
        .collect();
    if empty {
        app.visible_conns_rows = 0;
        // 明细为空：面板不接收滚轮（指针悬停时滚轮滚任务列表）
        app.conns_area = None;
        f.render_widget(
            Paragraph::new(Span::styled("（无并发连接）", Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    // 表头（HTTP 三列：序号 4 / 数据列各 8，单空格分隔；列对齐 CJK 宽度感知）。
    // BT 七列表头（IP/下载速度/累计下载/上传速度/累计上传/客户端/协议）为
    // phase-02 预留（BT_COLW 常量同源），01 期无 BT 任务不触发
    let dim = |s: String, wd: usize| Span::styled(pad_left(&s, wd), Style::default().fg(DIM));
    // 列宽单源 HTTP_COLW（与 content_w 同源，防漂移）
    let head = vec![
        dim("序号".to_string(), HTTP_COLW[0]),
        Span::raw(" "),
        dim("下载速度".to_string(), HTTP_COLW[1]),
        Span::raw(" "),
        dim("累计下载".to_string(), HTTP_COLW[2]),
    ];
    let mut lines = vec![Line::from(head)];

    // v1.34/FR-01-115 S2：明细数据区左右各收 2 列（表头与数据行在面板内框
    // 基础上左右各收 2 列；滚动/命中/横向滚动条几何随数据区平移，点击与
    // 键盘语义不变；BT 视口宽相应 48→44 列，phase-02 承接）
    const PAD_X: u16 = 2;
    let view = Rect {
        x: inner.x.saturating_add(PAD_X),
        y: inner.y,
        width: inner.width.saturating_sub(PAD_X * 2),
        height: inner.height,
    };

    // v1.30：无底部提示行 → 数据行 = 内框高 - 表头 1 行
    let rows = (view.height as usize).saturating_sub(1);
    let data_rows = rows.max(1);
    let n_conns = rows_data.len();
    // v1.31/FR-01-96/109 注记（滚到底闪烁修复第二道防线）：起点钳到
    // 「最后一个满窗」起点——连接数收缩的过渡帧不产生底部空白行
    //（顺带消除 len < rows 时的减法恐慌；第一道防线 = 滚轮向下事件内
    // 按可视行数立即钳制，mouse.rs ScrollDown 臂）
    let start = app.conns_scroll.min(n_conns.saturating_sub(data_rows));
    let end = (start + data_rows).min(n_conns);
    // 选中行高亮仅本面板聚焦时显示（v1.30/FR-01-108 可见性策略：
    // 非聚焦面板不显示内部选中态，保证视觉焦点唯一）
    let show_sel = focused;

    for (vi, (cid, speed, cum, transferring)) in rows_data[start..end].iter().copied().enumerate() {
        let i = start + vi;
        let sel = show_sel && i == app.conns_sel;
        // 选中行：深青背景高亮（同任务列表选中色）+ 首列白字（FR-01-97）
        let bg = if sel { SEL_BG } else { Color::Reset };
        let cell = |s: String, wd: usize, col: Color| {
            Span::styled(pad_left(&s, wd), Style::default().fg(col).bg(bg))
        };
        let spans = vec![
            // v1.34/FR-01-96 ③：序号 = 连接池槽位号（1 基、稳定不随断连重编号，
            // 明细可能不连续）；Active/Connecting 均属活跃（活跃过滤已上收）
            cell(format!("{cid}"), 4, if sel { Color::White } else { FG }),
            Span::styled(" ", Style::default().bg(bg)),
            // 下载=淡蓝（同头部 ↓ 配色），正在连接（尚无落盘字节）显示 -
            cell(
                if transferring {
                    fmt_speed(speed)
                } else {
                    "-".to_string()
                },
                8,
                if transferring { LIGHT_BLUE } else { DIM2 },
            ),
            Span::styled(" ", Style::default().bg(bg)),
            cell(fmt_size(cum), 8, FG),
        ];
        lines.push(Line::from(spans));
    }

    // v1.30/FR-01-107：底部快捷键提示行退役（原 `Ctrl+↑↓ 选择 · Ctrl+B 断开`
    // 协议自适应文案随页脚收敛整体退役，交互说明收敛帮助窗口【并发连接】组）

    app.visible_conns_rows = data_rows;
    // v1.32/FR-01-96/109：横向滚动（唯一 Both 实例）——内容宽 = HTTP 22 列
    // （BT 预留 59 列，phase-02 承接）；v1.34：上限 = 内容宽 − 视口宽 + 1
    //（放宽 1 列，末列宽字完整可见），偏移钳制后整表（表头 + 数据）随视口
    // 平移；HTTP 22 列常态不超宽 → 上限 0、←/→ 无横向内容不响应
    let content_w = HTTP_CONTENT_W;
    app.conns_max_hscroll = content_w
        .saturating_sub(view.width as usize)
        .saturating_add(1);
    let hscroll = app.conns_hscroll.min(app.conns_max_hscroll) as u16;
    f.render_widget(Paragraph::new(lines).scroll((0, hscroll)), view);

    // 滚动条叠画（v1.32/FR-01-109：ScrollablePanel 组件收口——垂直连接行 +
    // 水平内容列；显示条件由组件内判定）
    sb.vertical(n_conns, data_rows, app.conns_scroll)
        .horizontal(content_w, view.width as usize, app.conns_hscroll);
    let bars = sb.render_bars(f, area);
    app.conns_v_scrollbar = bars.vertical;
    app.conns_h_scrollbar = bars.horizontal;
}

#[cfg(test)]
mod tests {
    //! v1.30 提示行退役 + 焦点选中可见性单测：面板无任何快捷键提示行；
    //! 选中行高亮仅并发面板聚焦时显示；右缘滚动条聚焦绿/未聚焦暗。

    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    use super::super::testfx;
    use super::{BORDER, BT_COLW, BT_CONTENT_W, FOCUS_BORDER, HTTP_COLW, HTTP_CONTENT_W, SEL_BG};
    use crate::app::{App, Panel};
    use crate::model::{Connection, Protocol, Task, TaskState};

    fn conns_render(app: &mut App, w: u16, h: u16) -> Terminal<TestBackend> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| {
            super::draw_conns(
                f,
                app,
                Rect {
                    x: 0,
                    y: 0,
                    width: w,
                    height: h,
                },
            )
        })
        .unwrap();
        term
    }

    fn task_with_conns(protocol: Protocol) -> Task {
        let mut t = crate::model::sample_task();
        t.protocol = protocol;
        t.state = TaskState::Downloading;
        t.probed = true;
        t.total = 5_000_000;
        t.connections = (1..=3)
            .map(|i| Connection {
                id: i,
                start: (i as u64 - 1) * 1_000_000,
                end: i as u64 * 1_000_000,
                done: 100,
            })
            .collect();
        t
    }

    /// v1.30/FR-01-107：提示行整体退役（原 HTTP `Ctrl+↑↓ 选择` /
    /// BT `Ctrl+↑↓ 选择 · Ctrl+B 断开` 均不再渲染）
    #[tokio::test]
    async fn hint_line_retired_entirely() {
        let mut app = testfx::make_app_in("ezr-conns", "hint-retired");
        app.tasks.push(task_with_conns(Protocol::Http));
        app.selected = 0;
        let text = conns_render(&mut app, 60, 14).backend().to_string();
        assert!(!text.contains("Ctrl+↑↓"), "HTTP 提示行退役: {text}");
        assert!(!text.contains("Ctrl+B"), "Ctrl+B 提示退役: {text}");
        assert!(!text.contains("（仅 BT）"), "「（仅 BT）」标注退役: {text}");
        // BT 任务同样无提示行
        let mut app2 = testfx::make_app_in("ezr-conns", "hint-retired-bt");
        app2.tasks.push(task_with_conns(Protocol::Bt));
        app2.selected = 0;
        let text2 = conns_render(&mut app2, 60, 14).backend().to_string();
        assert!(!text2.contains("Ctrl+"), "BT 提示行退役: {text2}");
    }

    /// v1.30/FR-01-108：选中行高亮仅并发面板聚焦时显示；列表聚焦时同位置无高亮
    #[tokio::test]
    async fn selected_row_highlight_only_when_focused() {
        let mut app = testfx::make_app_in("ezr-conns", "selvis");
        app.tasks.push(task_with_conns(Protocol::Http));
        app.selected = 0;
        app.conns_sel = 1;
        // 并发面板聚焦：选中行高亮深青背景
        app.focus = Panel::Conns;
        let term = conns_render(&mut app, 60, 14);
        let buf = term.backend().buffer();
        // 表头 y=1（inner.y），数据首行 y=2；选中行 = conns_sel=1 → y=3；
        // v1.34 S2：明细数据区左右各 2 列 → 数据首列 x = inner.x+2 = 3
        let sel_cell = buf.cell((3, 3)).unwrap();
        assert_eq!(sel_cell.bg, SEL_BG, "聚焦时选中行深青背景");
        // 列表聚焦：同位置无高亮（内部选中态不显示）
        app.focus = Panel::List;
        let term = conns_render(&mut app, 60, 14);
        let buf = term.backend().buffer();
        let cell = buf.cell((3, 3)).unwrap();
        assert_ne!(cell.bg, SEL_BG, "非聚焦时选中行不高亮");
    }

    /// v1.32/FR-01-109：右缘滚动条——↑│║│↓ 字形（双线滑块在单线轨道上滑动）；
    /// 聚焦绿加粗 / 未聚焦暗色；连接数不超一屏不绘制
    #[tokio::test]
    async fn scrollbar_thumb_color_follows_focus() {
        let mut app = testfx::make_app_in("ezr-conns", "sbar");
        let mut t = task_with_conns(Protocol::Http);
        // 12 条连接、可视 4 行（高 7：边框2+表头1+数据4）→ 超一屏
        t.connections = (1..=12)
            .map(|i| Connection {
                id: i,
                start: (i as u64 - 1) * 1000,
                end: i as u64 * 1000,
                done: 100,
            })
            .collect();
        app.tasks.push(t);
        app.selected = 0;
        // 聚焦：滑块 ║ 聚焦绿（首屏贴顶吸附：轨道首格为滑块）
        app.focus = Panel::Conns;
        let term = conns_render(&mut app, 40, 7);
        let buf = term.backend().buffer();
        let col = 40 - 1;
        let green_thumb = (1..6).any(|y| {
            buf.cell((col, y))
                .map(|c| c.symbol() == "║" && c.fg == FOCUS_BORDER)
                .unwrap_or(false)
        });
        assert!(green_thumb, "聚焦时滑块 ║ 聚焦绿");
        // 未聚焦：滑块暗色
        app.focus = Panel::List;
        let term = conns_render(&mut app, 40, 7);
        let buf = term.backend().buffer();
        let dim_thumb = (1..6).any(|y| {
            buf.cell((col, y))
                .map(|c| c.symbol() == "║" && c.fg == BORDER)
                .unwrap_or(false)
        });
        assert!(dim_thumb, "未聚焦时滑块暗色");
        // 垂直边框列连续：上角之下紧贴 ↑ 箭头、无中断（v1.32 不留白）
        let has_arrow = (1..6).any(|y| {
            buf.cell((col, y))
                .map(|c| c.symbol() == "↑" || c.symbol() == "↓")
                .unwrap_or(false)
        });
        assert!(has_arrow, "上下箭头紧贴角（垂直不留白）");
        // 不超一屏：无滑块
        let mut app2 = testfx::make_app_in("ezr-conns", "nosbar");
        app2.tasks.push(task_with_conns(Protocol::Http));
        app2.selected = 0;
        let term = conns_render(&mut app2, 40, 12);
        let buf = term.backend().buffer();
        let no_thumb =
            (1..11).all(|y| buf.cell((39, y)).map(|c| c.symbol() != "║").unwrap_or(true));
        assert!(no_thumb, "不超一屏无滚动条");
    }

    /// 内容宽常量与列宽表单源对应性守卫（BT 预留 59 / HTTP 22；防两处漂移）
    #[test]
    fn content_w_matches_colw() {
        assert_eq!(
            BT_CONTENT_W,
            BT_COLW.iter().sum::<usize>() + BT_COLW.len() - 1
        );
        assert_eq!(
            HTTP_CONTENT_W,
            HTTP_COLW.iter().sum::<usize>() + HTTP_COLW.len() - 1
        );
    }

    /// v1.34：右上角徽标 ` x/y 项 ` 前后各 1 空格（S1；x = 活跃明细行数、
    /// y = 任务设置的最大并发数）；单源 [`active_title`]，防字面量漂移回归
    #[test]
    fn active_title_padded_both_sides() {
        assert_eq!(super::active_title(0, 4), " 0/4 项 ");
        assert_eq!(super::active_title(7, 64), " 7/64 项 ");
    }

    /// 表头仍在位（序号/下载速度/累计下载）
    #[tokio::test]
    async fn header_columns_present() {
        let mut app = testfx::make_app_in("ezr-conns", "head");
        app.tasks.push(task_with_conns(Protocol::Http));
        app.selected = 0;
        let text = conns_render(&mut app, 60, 14).backend().to_string();
        assert!(text.contains("序号"), "表头 序号: {text}");
        assert!(
            text.contains("下载速度") && text.contains("累计下载"),
            "数据列: {text}"
        );
        // v1.34：3 条持块连接（本轮无字节）= 正在连接（活跃）→ x/y 项徽标
        assert!(text.contains("3/4 项"), "活跃口径徽标: {text}");
    }
}
