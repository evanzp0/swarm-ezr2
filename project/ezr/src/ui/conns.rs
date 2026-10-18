//! conns — 并发连接面板（FR-01-96，对齐 ezr-demo 定稿 5.5）
//!
//! 位置 = 右列、任务详情正下方，占右列余下全部高度。可见性状态门槛（REQ-3.1）：
//! 仅「下载中」（与 BT 预留「做种中」）显示逐连接明细，其余状态空面板居中提示
//! 「（无并发连接）」+ 活跃 0（渲染层门槛：状态下沉时连接数据冻结隐藏，恢复下载
//! 原样续用）。列定义（01 期 HTTP 三列，表头随协议自适应）：
//! 序号（4 列，1 基 = 连接 id，按序号升序）/ 下载速度（8 列右对齐，待命显示 `-`）/
//! 累计下载（8 列右对齐，自本次开始下载起算，FR-01-99）。BT 五列（对端 IP 掩码
//! 11 列/下载速度/累计下载/上传速度/累计上传）为 phase-02 预留形态（01 期无 BT
//! 任务，不触发）。标题右侧显示「活跃 x」（速度 > 0 的连接数）；面板底部提示行
//! 恒定显示快捷键提示；连接数超出可视高度时滚轮 / Ctrl+↑↓ 翻看。明细为空或
//! 未选中任务时面板不接收滚轮（conns_area 置 None，滚轮穿透滚动任务列表）。

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::text::{fmt_size, fmt_speed, pad_left};
use super::{bordered_block, ACCENT, BORDER, DIM, DIM2, FG, LIGHT_BLUE, SEL_BG};
use crate::app::App;

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
    let block = bordered_block(BORDER)
        .title(Span::styled(" 并发连接 ", Style::default().fg(ACCENT)))
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

    // 数据行按序号（连接 id，1 基）升序展示（FR-01-96）
    let mut order: Vec<&crate::model::Connection> = t.connections.iter().collect();
    order.sort_by_key(|c| c.id);

    let rows = (inner.height as usize).saturating_sub(2); // 表头 1 行 + 提示行 1 行
    let data_rows = rows.max(1);
    let start = app.conns_scroll.min(order.len() - 1);
    let end = (start + data_rows).min(order.len());

    for (i, c) in order[start..end].iter().enumerate() {
        let i = start + i;
        let sel = i == app.conns_sel;
        // 选中行：深青背景高亮（同任务列表选中色）+ 首列白字（FR-01-97）
        let bg = if sel { SEL_BG } else { Color::Reset };
        let speed = app.conn_speed_of(t.id, c.id);
        let cum = app.conn_cum_of(t.id, c.id);
        let active_conn = c.cap() > 0 && speed > 0.0;
        let cell = |s: String, wd: usize, col: Color| {
            Span::styled(pad_left(&s, wd), Style::default().fg(col).bg(bg))
        };
        let spans = vec![
            cell(format!("{}", c.id), 4, if sel { Color::White } else { FG }),
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

    // 提示行（恒定显示快捷键提示；文案照录 demo 定稿原文，D26。
    // v1.29/FR-01-96 修订：提示行随任务协议自适应——「Ctrl+B 断开」段仅 BT
    // 任务显示（断开能力本就仅 BT，FR-01-97），原恒显「（仅 BT）」标注退役；
    // 01 期全部任务为 HTTP(S) → 实际显示恒为 `Ctrl+↑↓ 选择`）
    let hint = if t.protocol.is_bt() {
        "Ctrl+↑↓ 选择 · Ctrl+B 断开"
    } else {
        "Ctrl+↑↓ 选择"
    };
    f.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(DIM2))),
        Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        },
    );

    app.visible_conns_rows = data_rows;
    f.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    //! FR-01-96 v1.29 提示行随任务协议自适应单测：非 BT = `Ctrl+↑↓ 选择`
    //!（无 Ctrl+B 段、无「（仅 BT）」标注）、BT = `Ctrl+↑↓ 选择 · Ctrl+B 断开`。

    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    use super::super::testfx;
    use crate::app::App;
    use crate::model::{Connection, Protocol, Task, TaskState};

    fn conns_text(app: &mut App) -> String {
        let mut term = Terminal::new(TestBackend::new(60, 14)).unwrap();
        term.draw(|f| {
            super::draw_conns(
                f,
                app,
                Rect {
                    x: 0,
                    y: 0,
                    width: 60,
                    height: 14,
                },
            )
        })
        .unwrap();
        term.backend().to_string()
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

    #[tokio::test]
    async fn http_hint_has_no_disconnect_segment() {
        let mut app = testfx::make_app_in("ezr-conns", "hint-http");
        app.tasks.push(task_with_conns(Protocol::Http));
        app.selected = 0;
        let text = conns_text(&mut app);
        assert!(text.contains("Ctrl+↑↓ 选择"), "HTTP 提示行：{text}");
        assert!(!text.contains("Ctrl+B"), "HTTP 无断开段：{text}");
        assert!(!text.contains("（仅 BT）"), "「（仅 BT）」标注退役：{text}");
    }

    #[tokio::test]
    async fn bt_hint_includes_disconnect_segment() {
        let mut app = testfx::make_app_in("ezr-conns", "hint-bt");
        app.tasks.push(task_with_conns(Protocol::Bt));
        app.selected = 0;
        let text = conns_text(&mut app);
        assert!(
            text.contains("Ctrl+↑↓ 选择 · Ctrl+B 断开"),
            "BT 提示行：{text}"
        );
        assert!(!text.contains("（仅 BT）"), "「（仅 BT）」标注退役：{text}");
    }

    #[tokio::test]
    async fn https_hint_same_as_http() {
        let mut app = testfx::make_app_in("ezr-conns", "hint-https");
        app.tasks.push(task_with_conns(Protocol::Https));
        app.selected = 0;
        let text = conns_text(&mut app);
        assert!(text.contains("Ctrl+↑↓ 选择"));
        assert!(!text.contains("Ctrl+B"));
    }
}
