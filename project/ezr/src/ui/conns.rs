//! conns — 并发连接面板（FR-01-96，对齐 ezr-demo 定稿 5.5；v1.30 修订）
//!
//! 位置 = 右列、任务详情正下方，占右列余下全部高度。可见性状态门槛（REQ-3.1）：
//! 仅「下载中」（与 BT 预留「做种中」）显示逐连接明细，其余状态空面板居中提示
//! 「（无并发连接）」+ 活跃 0（渲染层门槛：状态下沉时连接数据冻结隐藏，恢复下载
//! 原样续用）。列定义（01 期 HTTP 三列，表头随协议自适应）：
//! 序号（4 列，1 基 = 连接 id，按序号升序）/ 下载速度（8 列右对齐，待命显示 `-`）/
//! 累计下载（8 列右对齐，自本次开始下载起算，FR-01-99）。BT 五列（对端 IP 掩码
//! 11 列/下载速度/累计下载/上传速度/累计上传）为 phase-02 预留形态（01 期无 BT
//! 任务，不触发）。标题右侧显示「活跃 x」（速度 > 0 的连接数）。
//! v1.30 修订：**面板无底部提示行**（P3 原则/FR-01-107：交互说明收敛 H 帮助窗口，
//! 原「提示行随任务协议自适应」口径整体退役）；选中行高亮仅本面板聚焦时显示
//! （FR-01-108 可见性策略）；右缘滚动条按 FR-01-109 通用口径绘制（滑块色随焦点）。
//! 连接数超出可视高度时滚轮 / ↑↓（面板聚焦时）翻看。明细为空或未选中任务时
//! 面板不接收滚轮（conns_area 置 None，滚轮穿透滚动任务列表）。

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::text::{fmt_size, fmt_speed, pad_left};
use super::{
    lazy_scrollbar, panel_border_style, panel_title_style, BORDER, DIM, DIM2, FG, FOCUS_BORDER,
    LIGHT_BLUE, SEL_BG,
};
use crate::app::{App, Panel};

pub(super) fn draw_conns(f: &mut Frame, app: &mut App, area: Rect) {
    let (active, empty) = match app.sel_task() {
        Some(t) => {
            // 状态门槛（REQ-3.1）：仅「下载中/做种中」显示明细，其余按空面板处理
            let shows = t.state.shows_conns();
            let n = if shows { t.connections.len() } else { 0 };
            // 「活跃 x」计数单源 App::active_conn_count（architect v116 自渲染路径提取）
            (app.active_conn_count(t), n == 0)
        }
        None => (0, true),
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
                format!("活跃 {} ", active),
                Style::default().fg(DIM),
            ))
            .alignment(Alignment::Right),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);
    app.conns_area = Some(inner);

    let Some(t) = app.sel_task() else {
        app.visible_conns_rows = 0;
        // 未选中任务：面板不接收滚轮（指针悬停时滚轮滚任务列表）
        app.conns_area = None;
        f.render_widget(
            Paragraph::new(Span::styled("（未选中任务）", Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    };
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

    // 表头（HTTP 三列：序号 4 / 数据列各 8，单空格分隔；列对齐 CJK 宽度感知）
    let dim = |s: String, wd: usize| Span::styled(pad_left(&s, wd), Style::default().fg(DIM));
    let head = vec![
        dim("序号".to_string(), 4),
        Span::raw(" "),
        dim("下载速度".to_string(), 8),
        Span::raw(" "),
        dim("累计下载".to_string(), 8),
    ];
    let mut lines = vec![Line::from(head)];

    // 数据行按序号（连接 id，1 基）升序展示（FR-01-96）。行数据先预计算为
    // 拥有值（借用收窄：t 借用 app，下方 visible_conns_rows 可变赋值不得与
    // 之重叠——notes/rust.md「路径敏感借用」纪律）
    let n_conns = t.connections.len();
    let mut rows_data: Vec<(usize, f64, u64, bool)> = t
        .connections
        .iter()
        .map(|c| {
            let speed = app.conn_speed_of(t.id, c.id);
            (
                c.id,
                speed,
                app.conn_cum_of(t.id, c.id),
                c.cap() > 0 && speed > 0.0,
            )
        })
        .collect();
    rows_data.sort_by_key(|r| r.0);

    // v1.30：无底部提示行 → 数据行 = 内框高 - 表头 1 行
    let rows = (inner.height as usize).saturating_sub(1);
    let data_rows = rows.max(1);
    let start = app.conns_scroll.min(n_conns.saturating_sub(1));
    let end = (start + data_rows).min(n_conns);
    // 选中行高亮仅本面板聚焦时显示（v1.30/FR-01-108 可见性策略：
    // 非聚焦面板不显示内部选中态，保证视觉焦点唯一）
    let show_sel = focused;

    for (vi, (cid, speed, cum, active_conn)) in rows_data[start..end].iter().copied().enumerate() {
        let i = start + vi;
        let sel = show_sel && i == app.conns_sel;
        // 选中行：深青背景高亮（同任务列表选中色）+ 首列白字（FR-01-97）
        let bg = if sel { SEL_BG } else { Color::Reset };
        let cell = |s: String, wd: usize, col: Color| {
            Span::styled(pad_left(&s, wd), Style::default().fg(col).bg(bg))
        };
        let spans = vec![
            cell(format!("{cid}"), 4, if sel { Color::White } else { FG }),
            Span::styled(" ", Style::default().bg(bg)),
            // 下载=淡蓝（同头部 ↓ 配色），待命显示 -
            cell(
                if active_conn {
                    fmt_speed(speed)
                } else {
                    "-".to_string()
                },
                8,
                if active_conn { LIGHT_BLUE } else { DIM2 },
            ),
            Span::styled(" ", Style::default().bg(bg)),
            cell(fmt_size(cum), 8, FG),
        ];
        lines.push(Line::from(spans));
    }

    // v1.30/FR-01-107：底部快捷键提示行退役（原 `Ctrl+↑↓ 选择 · Ctrl+B 断开`
    // 协议自适应文案随页脚收敛整体退役，交互说明收敛帮助窗口【并发连接】组）

    app.visible_conns_rows = data_rows;
    f.render_widget(Paragraph::new(lines), inner);

    // 右缘滚动条（v1.30/FR-01-109：连接数超一屏时；滑块色随面板焦点）
    if n_conns > data_rows {
        let thumb = if focused { FOCUS_BORDER } else { BORDER };
        lazy_scrollbar(f, area, n_conns, app.conns_scroll, data_rows, thumb);
    }
}

#[cfg(test)]
mod tests {
    //! v1.30 提示行退役 + 焦点选中可见性单测：面板无任何快捷键提示行；
    //! 选中行高亮仅并发面板聚焦时显示；右缘滚动条聚焦绿/未聚焦暗。

    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    use super::super::testfx;
    use super::{BORDER, FOCUS_BORDER, SEL_BG};
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
        // 表头 y=1（inner.y），数据首行 y=2；选中行 = conns_sel=1 → y=3
        let sel_cell = buf.cell((2, 3)).unwrap();
        assert_eq!(sel_cell.bg, SEL_BG, "聚焦时选中行深青背景");
        // 列表聚焦：同位置无高亮（内部选中态不显示）
        app.focus = Panel::List;
        let term = conns_render(&mut app, 60, 14);
        let buf = term.backend().buffer();
        let cell = buf.cell((2, 3)).unwrap();
        assert_ne!(cell.bg, SEL_BG, "非聚焦时选中行不高亮");
    }

    /// v1.30/FR-01-109：右缘滚动条——聚焦绿 / 未聚焦暗色；连接数不超一屏不绘制
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
        // 聚焦：滑块聚焦绿
        app.focus = Panel::Conns;
        let term = conns_render(&mut app, 40, 7);
        let buf = term.backend().buffer();
        let col = 40 - 1;
        let green_thumb = (1..6).any(|y| {
            buf.cell((col, y))
                .map(|c| c.symbol() == "█" && c.fg == FOCUS_BORDER)
                .unwrap_or(false)
        });
        assert!(green_thumb, "聚焦时滑块聚焦绿");
        // 未聚焦：滑块暗色
        app.focus = Panel::List;
        let term = conns_render(&mut app, 40, 7);
        let buf = term.backend().buffer();
        let dim_thumb = (1..6).any(|y| {
            buf.cell((col, y))
                .map(|c| c.symbol() == "█" && c.fg == BORDER)
                .unwrap_or(false)
        });
        assert!(dim_thumb, "未聚焦时滑块暗色");
        // 不超一屏：无滑块
        let mut app2 = testfx::make_app_in("ezr-conns", "nosbar");
        app2.tasks.push(task_with_conns(Protocol::Http));
        app2.selected = 0;
        let term = conns_render(&mut app2, 40, 12);
        let buf = term.backend().buffer();
        let no_thumb =
            (1..11).all(|y| buf.cell((39, y)).map(|c| c.symbol() != "█").unwrap_or(true));
        assert!(no_thumb, "不超一屏无滚动条");
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
        assert!(text.contains("活跃 0"), "待命活跃 0: {text}");
    }
}
