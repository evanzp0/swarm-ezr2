//! pieces — 分块状态弹窗（`P` 快捷键，FR-01-106）渲染层：68×20 圆角浮层
//!（小终端钳制）、八段内容（顶 padding/任务名/元信息/上网格呼吸行/
//! 分块网格/下网格呼吸行/四色图例行/底 padding）、下载前沿定位、右缘
//! 滚动条（v1.32 通用滚动条组件 ScrollablePanel Vertical 实例）、CJK 宽
//! 字符边界切割（与对话框同口径）。
//!
//! 口径（FR-01-106 ②④⑤⑧，v1.35 对齐 ezr-demo README 变更记录 21+22
//! draw_pieces 源码终态；v1.32 滚动条对齐 ezr-demo m3；v1.34 padding
//! 统一 FR-01-115）：
//! - 固定 68×20（含圆角边框；内框 66×18：顶 padding 1 + 名称/元信息/
//!   网格上呼吸行 1 + 网格 11 行 + 网格下呼吸行 1 + 图例 1 + 底 padding 1；
//!   名称/元信息/图例/网格左右各 2 列（S2）→ 视口 62 列 → 每行 31 块 →
//!   一屏 341 块；v1.34 口径（64×21/六段/29 块/406 块）由本修订取代——
//!   两步演进 64×21→67×23 呼吸行 + 加宽 3 列、67×23→68×20 高度收缩
//!   3 行 + 网格右边扩 1 列）；打开时终端内居中，拖拽位置
//!   优先（钳制在终端内），关闭不保留；
//! - 右上角 [✗]（v1.34/FR-01-114，原地松开 = 关闭弹窗）；
//! - 模态弹窗恒定绿色加粗边框/标题（与主面板聚焦色统一，v1.30/FR-01-108）；
//! - 网格每块占宽 2 列、同状态连续块合并为单 span；块序 = 字节序；
//! - 打开首帧定位下载前沿（第一块未完成块所在行滚动至网格垂直居中），
//!   此后滚动完全由用户控制、不自动跟随；
//! - 滚动上限按实际总行数钳制（`End` 置 `usize::MAX` 在此收敛到末页）；
//! - 块数超一屏时右缘（右边框列）通用滚动条（↑│║│↓，滑块恒聚焦绿加粗；
//!   v1.32/FR-01-109：拖滑块滚网格不移窗——m3 修复，ScrollablePanel 组件收口）；
//! - 浮层翻转零清屏（v1.31/FR-01-106⑧）：开/关/拖拽帧由增量 diff 直接绘制、
//!   不清屏，无整屏闪烁；残影由 clip_wide_at_edges 缓冲区级清扫根治。

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

/// pieces 弹窗固定尺寸（宽×高，含边框；v1.35/FR-01-106：64×21 → 68×20——
/// 呼吸行 + 加宽 + 高度收缩，内框 66×18）
const PIECES_W: u16 = 68;
/// pieces 弹窗固定高度（含边框）
const PIECES_H: u16 = 20;
/// 内容区左右内边距（v1.34/FR-01-115 S2：名称/元信息/图例/网格左右各 2 列）
const PAD_X: u16 = 2;

/// piece 状态色块 span：同状态连续 n 块合并为一个 span（色块字符 emoji 自带
/// 语义色，fg 兜底供无彩色 emoji 字体的终端降级显示）
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
    // 先取快照（owned）再回填 app（滚动定位/热区），避免借用冲突
    let (name, proto_label, bsize, conc, x, y, states) = {
        let snap = app.sel_task().map(|t| {
            let (x, y, _piece) = t.chunk_info();
            (
                t.name.clone(),
                t.protocol.label().to_string(),
                fmt_block_size(t.block_size),
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

    // 弹窗尺寸 68×20（内框 66×18：顶 padding 1 + 名称/元信息/上呼吸行各
    // 1 行 + 网格 11 行 + 下呼吸行/图例/底 padding 各 1 行；名称/元信息/
    // 图例/网格左右各 2 列（S2）→ 视口 62 列整除 → 每行 31 块 → 一屏
    // 341 块，v1.35/FR-01-106）；位置优先取记忆的拖拽位置
    //（钳制在终端内），打开首帧居中并记忆；小终端钳制尺寸，网格行数随内框收缩
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
    // 浮层边界 CJK 宽字符切割处理（残影根治点，与对话框同口径）
    clip_wide_at_edges(f, rect);
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        // 模态弹窗恒定高亮边框（与主面板聚焦色统一的绿+粗；
        // 与底层面板降级暗边框形成对比，v1.30/FR-01-108）
        .border_style(panel_border_style(true))
        .title(Span::styled(" 分块状态 ", panel_title_style(true)));
    // v1.32/FR-01-109：ScrollablePanel 一体化组件（pieces = Vertical 实例，
    // 滑块恒聚焦绿）；滑块拖拽滚网格、箭头/轨道交互随 App 态
    let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
    sb.thumb(FOCUS_BORDER)
        .pressed_v(app.sb_arrow_pressed(SbKey::Pieces));
    let inner = sb.render_block(f, rect, block);
    app.pieces_area = Some(rect);
    // 右上角 [✗]（v1.34/FR-01-114：全部弹窗统一；原地松开 = 关闭分块弹窗）
    app.popup_close_rect = draw_close_btn(f, app, rect, super::text::w(" 分块状态 "));

    let cols = ((inner.width.saturating_sub(PAD_X * 2) as usize) / 2).max(1); // 每块占 2 列（内容区宽 = 内框 − 4；68×20 下 62 列 → 31 块，整除无余列）
    let rows = Layout::vertical([
        Constraint::Length(1), // 顶部 padding（v1.34/FR-01-115 S2：内容区上 1 行）
        Constraint::Length(1), // 任务名
        Constraint::Length(1), // 元信息
        Constraint::Length(1), // 网格上方空行（v1.35/FR-01-106：分块图上空 1 行呼吸行）
        Constraint::Min(1),    // 分块网格
        Constraint::Length(1), // 网格下方空行（v1.35/FR-01-106：分块图下空 1 行呼吸行）
        Constraint::Length(1), // 四色图例行（v1.30：替代快捷键提示行，P3）
        Constraint::Length(1), // 底部 padding（v1.34/FR-01-115 S2：内容区下 1 行）
    ])
    .split(inner);
    let grid = rows[4];
    let grid_h = grid.height as usize;
    // 内容区（S2 左右各 2 列）：名称/元信息/图例/网格渲染矩形
    let view = Rect {
        x: inner.x.saturating_add(PAD_X),
        width: inner.width.saturating_sub(PAD_X * 2),
        ..inner
    };

    let total_rows = y.div_ceil(cols);
    // 打开首帧：以第一块未完成块所在行为中心定位（下载前沿可见：
    // 上方 🟩、前沿 🟦/🟨、下方 ⬜，一眼读出整体进度形态）
    if app.pieces_follow {
        app.pieces_follow = false;
        let frontier = x as usize / cols;
        app.pieces_scroll = frontier.saturating_sub(grid_h / 2);
    }
    app.pieces_scroll = app.pieces_scroll.min(total_rows.saturating_sub(grid_h));
    let start_row = app.pieces_scroll;

    // 行 1（rows[1]）：任务名（超宽截尾，白色加粗；S2 左右各 2 列）——行号
    // 注：rows[0]=顶 padding、rows[3]=上呼吸行（v1.35 八段）
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

    // 行 2（rows[2]）：元信息 = 协议 · 块大小 · x/y 块（x = 已完成块数，与详情面板
    // 「分块」行同口径同值）· 并发上限
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

    // 行 3：分块网格（仅构建可视行；同状态连续块合并为单 span）
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

    // 行 6（rows[6]）：四色图例（v1.30/FR-01-106：底部状态说明——色块字符
    // 与网格同源，字色兜底同 piece_span；v1.34 S2：左右各 2 列，无前导空格；
    // v1.35：rows[5] = 下呼吸行，图例行随八段布局后移至 rows[6]）
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

    // 右缘滚动条（v1.32/FR-01-109：ScrollablePanel 组件收口——块数超一屏时
    // 画在弹窗右边框列；显示条件由组件内判定；滑块恒聚焦绿）
    sb.vertical(total_rows, grid_h, start_row);
    app.pieces_scrollbar = sb.render_bars(f, rect).vertical;
    app.pieces_max_scroll = total_rows.saturating_sub(grid_h);
}

#[cfg(test)]
mod tests {
    //! pieces 弹窗渲染单测（TestBackend 伪终端金样）：窗口几何与四段内容、
    //! 四态色块、前沿定位与不跟随、小终端钳制、右缘滚动条、任务消失自动关窗。

    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    use crate::app::App;
    use crate::model::{Connection, Task, TaskState};

    fn make_app(tag: &str) -> App {
        let dir = crate::model::testenv::uniq_tmp_dir(&format!("ezr-upieces-{tag}"));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        App::new(crate::model::config::Config::default(), reg)
    }

    fn dl_task(id: u32, name: &str, total: u64) -> Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t.state = TaskState::Downloading;
        t.probed = true;
        t.total = total;
        t.block_size = 1_048_576; // 1 MiB（fmt_block_size 1024 基 → "1 MB"）
        t.concurrency = 4;
        t
    }

    /// 整帧渲染并返回缓冲区行文本（backend().to_string 的终端语义：宽字符
    /// 占两格但只印一次，续格不产生多余空格——与既有 conns 金样断言同口径）
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

    /// ①②窗口形态：120×34 下 68×20 居中（x=26,y=7）、圆角边框、标题
    /// 「分块状态」、八段齐全（顶 padding/任务名/元信息/上网格呼吸行/
    /// 分块网格/下网格呼吸行/四色图例行/底 padding，v1.35）
    #[tokio::test]
    async fn popup_geometry_and_four_sections() {
        let mut app = make_app("geom");
        app.tasks.push(dl_task(1, "piece-dl8.bin", 5_242_880));
        app.selected = 0;
        open_popup(&mut app);
        let rows = render_rows(&mut app, 120, 34);
        // 边框：x=26,y=7 起 68×20（v1.35/FR-01-106：行 7 顶边框 + 标题、行 26 底边框）
        assert!(rows[7].contains('╭'), "圆角顶边框：{}", rows[7]);
        assert!(rows[7].contains("分块状态"), "标题：{}", rows[7]);
        assert!(rows[26].contains('╰'), "圆角底边框：{}", rows[26]);
        // 八段（v1.35）：顶 padding（行 8）/ 任务名（行 9）/ 元信息（行 10）/
        // 网格上方呼吸行（行 11）/ 网格（行 12 起 11 行）/ 网格下方呼吸行
        // （行 23）/ 四色图例行（行 24）/ 底 padding（行 25）
        assert!(rows[9].contains("piece-dl8.bin"), "任务名：{}", rows[9]);
        assert!(
            rows[10].contains("HTTP · 块大小 1 MB · 0/5 块 · 并发 4"),
            "元信息行：{}",
            rows[10]
        );
        assert!(rows[12].contains('⬜'), "网格行含未下载块：{}", rows[12]);
        // v1.30：四色图例行（替代快捷键提示行）
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
        // 原快捷键提示行退役
        assert!(
            !rows.iter().any(|r| r.contains("PgUp/PgDn 滚动")),
            "快捷键提示行已退役"
        );
        // 居中：弹窗左缘 26 列、右缘 93 列（顶/底边框逐格验证，QA-UP-01 逐格口径）
        let mut term = Terminal::new(TestBackend::new(120, 34)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        let buf = term.backend().buffer();
        assert_eq!(buf.cell((26, 7)).unwrap().symbol(), "╭", "居中 x=26");
        assert_eq!(buf.cell((93, 7)).unwrap().symbol(), "╮", "右缘 x=93");
        assert_eq!(buf.cell((26, 26)).unwrap().symbol(), "╰", "底边框 y=26");
        assert_eq!(buf.cell((93, 26)).unwrap().symbol(), "╯", "右下角圆角");
        // 八段逐段核验：顶/底 padding 与网格上下呼吸行在弹窗内为整行空行
        //（内框列 27..=92 逐格检查，v1.35 八段布局）
        for &y in &[8u16, 11, 23, 25] {
            assert!(
                (27..93).all(|x| buf.cell((x, y)).is_none_or(|c| c.symbol() == " ")),
                "段行 y={y} 应为弹窗内空行（padding/呼吸行）"
            );
        }
        // 弹窗外主界面仍完整渲染（页脚仍在：v1.31 双行页脚 4 行 =
        // y30 边框 + y31 chips + y32 toast + y33 边框）
        assert!(
            rows[31].contains("退出"),
            "页脚 chips 行不被破坏: {}",
            rows[31]
        );
        app.shutdown().await;
    }

    /// ③四态色块：已完成前缀 🟩 + 未下载 ⬜（非下载中无 🟦）；在传块 🟦
    ///（下载中）；暂停后有零头冻结块 🟨
    #[tokio::test]
    async fn popup_color_blocks_four_states() {
        // 已暂停任务：chunk_done=3 → 前 3 块 🟩、其余 ⬜、无 🟦
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

        // 下载中任务：块 3/4 被连接持有（块 3 有零头）→ 🟦（🟦 优先于 🟨）
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

        // 同任务空格暂停：在传连接冻结 → 块 3 有零头转 🟨、块 4 转 ⬜
        app.tasks[0].state = TaskState::Paused;
        let rows = render_rows(&mut app, 120, 34);
        assert!(
            rows[12].contains("🟩🟩🟩🟨⬜"),
            "暂停后：零头冻结块转 🟨、无零头转 ⬜：{}",
            rows[12]
        );
    }

    /// ④打开定位下载前沿：首帧以第一块未完成块所在行为中心（滚动 =
    /// frontier - grid_h/2，按总行数钳制），此后不自动跟随（follow 消费一次）
    #[tokio::test]
    async fn popup_opens_centered_on_frontier_and_stays() {
        let mut app = make_app("frontier");
        // 600 块 → 20 网格行（31 块/行，v1.35）；chunk_done=300 → 前沿行 9
        app.tasks.push(dl_task(1, "piece-big.bin", 629_145_600));
        app.tasks[0].chunk_done = 300;
        app.selected = 0;
        open_popup(&mut app);
        assert!(app.pieces_follow);
        let _rows = render_rows(&mut app, 120, 34);
        // frontier = 300/31 = 9；grid_h = 11（v1.35：内框 18 行 − 八段定高 7）；
        // scroll = 9 − 11/2 = 4
        assert_eq!(app.pieces_scroll, 4, "前沿行居中定位");
        assert!(!app.pieces_follow, "follow 首帧消费一次");
        // 下载推进（chunk_done 增长）后再渲染：滚动不变（不自动跟随）
        app.tasks[0].chunk_done = 500;
        let _rows = render_rows(&mut app, 120, 34);
        assert_eq!(app.pieces_scroll, 4, "打开后不自动跟随下载前沿");
        app.shutdown().await;
    }

    /// ②小终端钳制：56×16 下弹窗钳制 56×16、八段完整、网格行随内框收缩
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

    /// ②块数超一屏：右缘（右边框列）出现滚动条滑块（v1.32：通用滚动条字形
    /// ↑│║│↓，双线滑块恒聚焦绿加粗）
    #[tokio::test]
    async fn popup_shows_scrollbar_when_overflow() {
        let mut app = make_app("sbar");
        app.tasks.push(dl_task(1, "piece-big.bin", 629_145_600));
        app.selected = 0;
        open_popup(&mut app);
        let mut term = Terminal::new(TestBackend::new(120, 34)).unwrap();
        term.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
        let buf = term.backend().buffer();
        // 弹窗 x=26、宽 68 → 右边框列 x=93；总 20 行 > 网格 11 行 → 有滑块
        //（v1.32：滑块 ║ 恒聚焦绿）
        let thumb = (8..26).any(|y| {
            buf.cell((93, y))
                .map(|c| c.symbol() == "║" && c.fg == super::FOCUS_BORDER)
                .unwrap_or(false)
        });
        assert!(thumb, "右缘滚动条滑块出现（聚焦绿）");
        // 命中几何回填（滑块拖拽/箭头/轨道交互依据）
        assert!(app.pieces_scrollbar.is_some(), "pieces 滚动条几何回填");
        // 不超一屏（5 块）→ 无滚动条
        let mut app2 = make_app("nosbar");
        app2.tasks.push(dl_task(1, "small.bin", 5_242_880));
        app2.selected = 0;
        app2.open_pieces();
        let rows = render_rows(&mut app2, 120, 34);
        let no_thumb = (7..26).all(|y| !rows[y].contains('║'));
        assert!(no_thumb, "不超一屏无滚动条");
        app.shutdown().await;
    }

    /// ⑤滚动钳制：`End` 置 usize::MAX 后渲染收敛到末页（总行数 - 网格行数）
    #[tokio::test]
    async fn popup_end_key_clamps_to_last_page() {
        let mut app = make_app("end");
        app.tasks.push(dl_task(1, "piece-big.bin", 629_145_600));
        app.selected = 0;
        open_popup(&mut app);
        // 首帧渲染消费 follow（真实流程序曲：先开窗渲染、后按 End）
        let _rows = render_rows(&mut app, 120, 34);
        assert!(!app.pieces_follow, "首帧已消费 follow");
        app.pieces_scroll = usize::MAX; // 模拟 End 键
        let _rows = render_rows(&mut app, 120, 34);
        // 总行数 = ceil(600 块 / 31 块每行) = 20；末页首行 = 20 - 11 = 9（v1.35 网格 11 行）
        assert_eq!(app.pieces_scroll, 9, "End 钳制到末页首行");
        app.shutdown().await;
    }

    /// 弹窗打开期间选中任务消失（任务被清）→ 渲染层自动关窗（demo 同构）
    #[tokio::test]
    async fn popup_closes_when_selected_task_gone() {
        let mut app = make_app("gone");
        app.open_pieces(); // 无任务直接开（越过门槛：模拟任务后被清）
        app.pieces_open = true;
        let _rows = render_rows(&mut app, 120, 34);
        assert!(!app.pieces_open, "无选中任务渲染时自动关闭");
        app.shutdown().await;
    }
}
