use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use super::btn::{clip_wide_at_edges, dialog_rect, draw_close_btn};
use super::scrollable_panel::{ScrollablePanel, ScrollbarMode};
use super::text::truncate;
use super::{
    panel_border_style, panel_title_style, EMPTY, FG, FOCUS_BORDER, GREEN, LIGHT_BLUE, YELLOW,
};
use crate::app::{App, SbKey};
use crate::model::chunk::fmt_block_size;
use crate::model::PieceState;

const PIECES_W: u16 = 68;
const PIECES_H: u16 = 20;
const PAD_X: u16 = 2;

fn piece_span(st: PieceState, n: usize) -> Span<'static> {
    let (ch, color) = match st {
        PieceState::Done => ('🟩', GREEN),
        PieceState::Active => ('🟦', LIGHT_BLUE),
        PieceState::Partial => ('🟨', YELLOW),
        PieceState::Empty => ('⬜', EMPTY),
    };
    Span::styled(ch.to_string().repeat(n), Style::default().fg(color))
}

pub(super) fn draw_pieces(f: &mut Frame, app: &mut App, area: Rect) {
    let (name, proto_label, bsize, conc, x, y, states) = {
        let snap = app.sel_task().map(|t| {
            let (x, y, plen) = t.chunk_info();
            (
                t.name.clone(),
                t.protocol.label().to_string(),
                fmt_block_size(plen),
                t.concurrency,
                x,
                y,
                t.pieces(),
            )
        });
        let Some(snap) = snap else {
            app.close_pieces();
            return;
        };
        snap
    };
    let y = y as usize;

    let rect = match app.pieces_pos {
        Some((px, py)) => {
            let dw = PIECES_W.min(area.width);
            let dh = PIECES_H.min(area.height);
            Rect {
                x: px.min(area.width.saturating_sub(dw)),
                y: py.min(area.height.saturating_sub(dh)),
                width: dw,
                height: dh,
            }
        }
        None => {
            let r = dialog_rect(area, PIECES_W, PIECES_H);
            app.pieces_pos = Some((r.x, r.y));
            r
        }
    };
    f.render_widget(Clear, rect);
    clip_wide_at_edges(f, rect);
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(Span::styled(" 分块状态 ", panel_title_style(true)));
    let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
    sb.thumb(FOCUS_BORDER)
        .pressed_v(app.sb_arrow_pressed(SbKey::Pieces));
    let inner = sb.render_block(f, rect, block);
    app.pieces_area = Some(rect);
    app.popup_close_rect = draw_close_btn(f, app, rect, super::text::w(" 分块状态 "));

    let cols = ((inner.width.saturating_sub(PAD_X * 2) as usize) / 2).max(1);
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);
    let grid = rows[4];
    let grid_h = grid.height as usize;
    let view = Rect {
        x: inner.x.saturating_add(PAD_X),
        width: inner.width.saturating_sub(PAD_X * 2),
        ..inner
    };

    let total_rows = y.div_ceil(cols);
    if app.pieces_follow {
        app.pieces_follow = false;
        let frontier = x as usize / cols;
        app.pieces_scroll = frontier.saturating_sub(grid_h / 2);
    }
    app.pieces_scroll = app.pieces_scroll.min(total_rows.saturating_sub(grid_h));
    let start_row = app.pieces_scroll;

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            truncate(&name, view.width as usize),
            Style::default().fg(FG).add_modifier(Modifier::BOLD),
        ))),
        Rect {
            y: rows[1].y,
            ..view
        },
    );

    let meta = Span::styled(
        format!(
            "{} · 块大小 {} · {}/{} 块 · 并发 {}",
            proto_label, bsize, x, y, conc
        ),
        Style::default().fg(super::DIM),
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![meta])),
        Rect {
            y: rows[2].y,
            ..view
        },
    );

    let mut lines: Vec<Line> = Vec::with_capacity(grid_h);
    for r in 0..grid_h {
        let s = (start_row + r) * cols;
        if s >= y {
            break;
        }
        let e = (s + cols).min(y);
        let mut spans: Vec<Span> = Vec::new();
        let mut run = states[s];
        let mut run_at = s;
        for (off, st) in states[s..e].iter().enumerate() {
            let i = s + off;
            if *st != run {
                spans.push(piece_span(run, i - run_at));
                run = *st;
                run_at = i;
            }
        }
        spans.push(piece_span(run, e - run_at));
        lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(lines), Rect { y: grid.y, ..view });

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("🟩", Style::default().fg(GREEN)),
            Span::styled(" 已下载   ", Style::default().fg(FG)),
            Span::styled("🟦", Style::default().fg(LIGHT_BLUE)),
            Span::styled(" 正在下载   ", Style::default().fg(FG)),
            Span::styled("🟨", Style::default().fg(YELLOW)),
            Span::styled(" 部分下载   ", Style::default().fg(FG)),
            Span::styled("⬜", Style::default().fg(EMPTY)),
            Span::styled(" 未下载", Style::default().fg(FG)),
        ])),
        Rect {
            y: rows[6].y,
            ..view
        },
    );

    sb.vertical(total_rows, grid_h, start_row);
    app.pieces_scrollbar = sb.render_bars(f, rect).vertical;
    app.pieces_max_scroll = total_rows.saturating_sub(grid_h);
}

#[cfg(test)]
mod tests {

    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    use crate::app::App;
    use crate::model::{Connection, Task, TaskState};

    fn make_app(tag: &str) -> App {
        crate::ui::testfx::make_app_in("ezr-upieces", tag)
    }

    fn dl_task(id: u32, name: &str, total: u64) -> Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t.state = TaskState::Downloading;
        t.probed = true;
        t.total = total;
        t.block_size = 1_048_576;
        t.concurrency = 4;
        t
    }

    fn render_rows(app: &mut App, w: u16, h: u16) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| crate::ui::draw(f, app)).unwrap();
        term.backend()
            .to_string()
            .split('\n')
            .map(String::from)
            .collect()
    }

    fn open_popup(app: &mut App) {
        app.open_pieces();
        assert!(app.pieces_open);
    }

    #[tokio::test]
    async fn popup_geometry_and_four_sections() {
        let mut app = make_app("geom");
        app.tasks.push(dl_task(1, "piece-dl8.bin", 5_242_880));
        app.selected = 0;
        open_popup(&mut app);
        let rows = render_rows(&mut app, 120, 34);
        assert!(rows[7].contains('╭'), "圆角顶边框：{}", rows[7]);
        assert!(rows[7].contains("分块状态"), "标题：{}", rows[7]);
        assert!(rows[26].contains('╰'), "圆角底边框：{}", rows[26]);
        assert!(rows[9].contains("piece-dl8.bin"), "任务名：{}", rows[9]);
        assert!(
            rows[10].contains("HTTP · 块大小 1 MB · 0/5 块 · 并发 4"),
            "元信息行：{}",
            rows[10]
        );
        assert!(rows[12].contains('⬜'), "网格行含未下载块：{}", rows[12]);
        assert!(
            rows[24].contains("已下载") && rows[24].contains("正在下载"),
            "图例行：{}",
            rows[24]
        );
        assert!(
            rows[24].contains("部分下载") && rows[24].contains("未下载"),
            "图例四色齐全：{}",
            rows[24]
        );
        assert!(
            !rows.iter().any(|r| r.contains("PgUp/PgDn 滚动")),
            "快捷键提示行已退役"
        );
        let mut term = Terminal::new(TestBackend::new(120, 34)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        let buf = term.backend().buffer();
        assert_eq!(buf.cell((26, 7)).unwrap().symbol(), "╭", "居中 x=26");
        assert_eq!(buf.cell((93, 7)).unwrap().symbol(), "╮", "右缘 x=93");
        assert_eq!(buf.cell((26, 26)).unwrap().symbol(), "╰", "底边框 y=26");
        assert_eq!(buf.cell((93, 26)).unwrap().symbol(), "╯", "右下角圆角");
        for &y in &[8u16, 11, 23, 25] {
            assert!(
                (27..93).all(|x| buf.cell((x, y)).is_none_or(|c| c.symbol() == " ")),
                "段行 y={y} 应为弹窗内空行（padding/呼吸行）"
            );
        }
        assert!(
            rows[31].contains("退出"),
            "页脚 chips 行不被破坏: {}",
            rows[31]
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn popup_color_blocks_four_states() {
        let mut app = make_app("color");
        let mut t = dl_task(1, "piece-pause.bin", 5_242_880);
        t.state = TaskState::Paused;
        t.chunk_done = 3;
        app.tasks.push(t);
        app.selected = 0;
        open_popup(&mut app);
        let rows = render_rows(&mut app, 120, 34);
        assert!(
            rows[12].contains("🟩🟩🟩⬜⬜"),
            "网格 = 前 3 🟩 + 后 2 ⬜（同状态连续）：{}",
            rows[12]
        );
        assert!(!rows[12].contains('🟦'), "非下载中无 🟦");

        let mut app = make_app("color2");
        let mut t = dl_task(1, "piece-act.bin", 5_242_880);
        t.chunk_done = 3;
        t.connections = vec![
            Connection {
                id: 1,
                start: 3_145_728,
                end: 4_194_304,
                done: 300_000,
            },
            Connection {
                id: 2,
                start: 4_194_304,
                end: 5_242_880,
                done: 0,
            },
        ];
        app.tasks.push(t);
        app.selected = 0;
        open_popup(&mut app);
        let rows = render_rows(&mut app, 120, 34);
        assert!(
            rows[12].contains("🟩🟩🟩🟦🟦"),
            "下载中：前 3 🟩 + 2 在传 🟦（分配即算）：{}",
            rows[12]
        );

        app.tasks[0].state = TaskState::Paused;
        let rows = render_rows(&mut app, 120, 34);
        assert!(
            rows[12].contains("🟩🟩🟩🟨⬜"),
            "暂停后：零头冻结块转 🟨、无零头转 ⬜：{}",
            rows[12]
        );
    }

    #[tokio::test]
    async fn popup_opens_centered_on_frontier_and_stays() {
        let mut app = make_app("frontier");
        app.tasks.push(dl_task(1, "piece-big.bin", 629_145_600));
        app.tasks[0].chunk_done = 300;
        app.selected = 0;
        open_popup(&mut app);
        assert!(app.pieces_follow);
        let _rows = render_rows(&mut app, 120, 34);
        assert_eq!(app.pieces_scroll, 4, "前沿行居中定位");
        assert!(!app.pieces_follow, "follow 首帧消费一次");
        app.tasks[0].chunk_done = 500;
        let _rows = render_rows(&mut app, 120, 34);
        assert_eq!(app.pieces_scroll, 4, "打开后不自动跟随下载前沿");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn popup_clamps_in_small_terminal() {
        let mut app = make_app("clamp");
        app.tasks.push(dl_task(1, "piece-dl8.bin", 5_242_880));
        app.selected = 0;
        open_popup(&mut app);
        let rows = render_rows(&mut app, 56, 16);
        assert_eq!(
            app.pieces_area,
            Some(Rect {
                x: 0,
                y: 0,
                width: 56,
                height: 16
            }),
            "弹窗钳制为终端尺寸且居中（0,0）"
        );
        assert!(rows[0].contains('╭'), "顶边框贴边");
        assert!(rows[0].contains("分块状态"));
        assert!(
            rows[3].contains("HTTP · 块大小 1 MB · 0/5 块 · 并发 4"),
            "元信息完整（v1.35 八段：顶 padding + 上呼吸行后网格随内框收缩）"
        );
        assert!(rows[5].contains('⬜'), "网格存在（v1.35 八段：内框收缩后仍 Min 1 行，屏行 5 = 顶 padding+名称+元信息+上呼吸行后）");
        assert!(
            rows[13].contains("已下载") && rows[13].contains("未下载"),
            "四色图例行"
        );
        assert!(rows[15].contains('╰'), "底边框贴边");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn popup_shows_scrollbar_when_overflow() {
        let mut app = make_app("sbar");
        app.tasks.push(dl_task(1, "piece-big.bin", 629_145_600));
        app.selected = 0;
        open_popup(&mut app);
        let mut term = Terminal::new(TestBackend::new(120, 34)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        let buf = term.backend().buffer();
        let thumb = (8..26).any(|y| {
            buf.cell((93, y))
                .map(|c| c.symbol() == "║" && c.fg == super::FOCUS_BORDER)
                .unwrap_or(false)
        });
        assert!(thumb, "右缘滚动条滑块出现（聚焦绿）");
        assert!(app.pieces_scrollbar.is_some(), "pieces 滚动条几何回填");
        let mut app2 = make_app("nosbar");
        app2.tasks.push(dl_task(1, "small.bin", 5_242_880));
        app2.selected = 0;
        app2.open_pieces();
        let rows = render_rows(&mut app2, 120, 34);
        let no_thumb = (7..26).all(|y| !rows[y].contains('║'));
        assert!(no_thumb, "不超一屏无滚动条");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn popup_end_key_clamps_to_last_page() {
        let mut app = make_app("end");
        app.tasks.push(dl_task(1, "piece-big.bin", 629_145_600));
        app.selected = 0;
        open_popup(&mut app);
        let _rows = render_rows(&mut app, 120, 34);
        assert!(!app.pieces_follow, "首帧已消费 follow");
        app.pieces_scroll = usize::MAX;
        let _rows = render_rows(&mut app, 120, 34);
        assert_eq!(app.pieces_scroll, 9, "End 钳制到末页首行");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn bt_pieces_meta_line_shows_piece_length() {
        let mut app = make_app("btmeta");
        let mut t = dl_task(1, "bt-pieces.bin", 3_000_000);
        t.protocol = crate::model::Protocol::Bt;
        t.block_size = 1_048_576;
        t.bt = Some(crate::model::bt::BtTaskData {
            source: "/tmp/a.torrent".to_string(),
            trackers: vec![],
            piece_length: 262_144,
            files: vec![crate::model::bt::BtFile {
                path: "a.bin".to_string(),
                size: 3_000_000,
                selected: true,
            }],
            done_pieces: vec![],
        });
        app.tasks.push(t);
        app.selected = 0;
        open_popup(&mut app);
        let rows = render_rows(&mut app, 120, 34);
        assert!(
            rows[10].contains("BT · 块大小 256 KB · 0/12 块 · 并发 4"),
            "BT 元信息行: {}",
            rows[10]
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn popup_closes_when_selected_task_gone() {
        let mut app = make_app("gone");
        app.open_pieces();
        app.pieces_open = true;
        let _rows = render_rows(&mut app, 120, 34);
        assert!(!app.pieces_open, "无选中任务渲染时自动关闭");
        app.shutdown().await;
    }
}
