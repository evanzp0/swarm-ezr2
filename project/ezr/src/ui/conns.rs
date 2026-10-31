use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::scrollable_panel::{ScrollablePanel, ScrollbarMode};
use super::text::{fmt_size, fmt_speed, pad_left, pad_right, truncate};
use super::{
    panel_border_style, panel_title_style, BORDER, DIM, DIM2, FG, FOCUS_BORDER, LIGHT_BLUE,
    MAGENTA, PINK, SEL_BG,
};
use crate::app::{App, Panel, SbKey};

const BT_COLW: [usize; 7] = [18, 8, 8, 8, 8, 34, 4];
const BT_CONTENT_W: usize = 94;
const HTTP_COLW: [usize; 3] = [4, 8, 8];
const HTTP_CONTENT_W: usize = 22;

fn active_title(active: usize, max_conc: usize) -> String {
    format!(" {active}/{max_conc} 项 ")
}

fn mask_ip(ip: &str) -> String {
    let ip = ip.trim_start_matches('[').trim_end_matches(']');
    if ip.contains(':') {
        let first = ip.split(':').next().unwrap_or("");
        let last = ip.rsplit(':').next().unwrap_or("");
        format!("{first}:*:{last}")
    } else {
        let first = ip.split('.').next().unwrap_or("");
        let last = ip.rsplit('.').next().unwrap_or("");
        format!("{first}.*.{last}")
    }
}

fn ip_display(addr: &str) -> String {
    let cell = if !addr.contains(':') {
        return pad_right(&truncate(addr, BT_COLW[0]), BT_COLW[0]);
    } else if let Some(i) = addr.find('[') {
        let inner = &addr[i + 1..];
        let inner = inner.split(']').next().unwrap_or(inner);
        let port = addr
            .split(']')
            .nth(1)
            .and_then(|s| s.strip_prefix(':'))
            .unwrap_or("");
        if port.is_empty() {
            mask_ip(inner)
        } else {
            format!("{}/{}", mask_ip(inner), port)
        }
    } else if addr.matches(':').count() == 1 {
        let (ip, port) = addr.split_once(':').unwrap_or((addr, ""));
        if port.is_empty() {
            mask_ip(ip)
        } else {
            format!("{}/{}", mask_ip(ip), port)
        }
    } else {
        let (ip, port) = addr.rsplit_once(':').unwrap_or((addr, ""));
        if port.is_empty() {
            mask_ip(ip)
        } else {
            format!("{}/{}", mask_ip(ip), port)
        }
    };
    pad_right(&truncate(&cell, BT_COLW[0]), BT_COLW[0])
}

type BtRowCells = (String, f64, f64, u64, u64, String, bool);

enum Rows {
    Bt(Vec<BtRowCells>),
    Http(Vec<(usize, f64, u64, bool)>),
}

impl Rows {
    fn len(&self) -> usize {
        match self {
            Rows::Bt(v) => v.len(),
            Rows::Http(v) => v.len(),
        }
    }
}

pub(super) fn draw_conns(f: &mut Frame, app: &mut App, area: Rect) {
    app.conns_v_scrollbar = None;
    app.conns_h_scrollbar = None;
    app.conns_max_hscroll = 0;

    let (is_bt, rows, max_conc) = match app.sel_task() {
        Some(t) => {
            let shows = t.state.shows_conns();
            if t.protocol.is_bt() {
                let v: Vec<BtRowCells> = if shows {
                    app.bt_conns
                        .get(&t.id)
                        .map(|cs| {
                            cs.iter()
                                .map(|c| {
                                    let (down, up) = app.bt_conn_speeds_of(t.id, c.id);
                                    (
                                        ip_display(&c.addr),
                                        down,
                                        up,
                                        c.cum_down,
                                        c.cum_up,
                                        c.client.clone(),
                                        c.seeder,
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                (true, Rows::Bt(v), t.concurrency)
            } else {
                let v: Vec<(usize, f64, u64, bool)> = if shows {
                    app.active_conn_ids()
                        .iter()
                        .filter_map(|cid| {
                            let t = app.sel_task()?;
                            t.connections.iter().find(|c| c.id == *cid)?;
                            let speed = app.conn_speed_of(t.id, *cid);
                            let cum = app.conn_cum_of(t.id, *cid);
                            Some((*cid, speed, cum, cum > 0))
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                (false, Rows::Http(v), t.concurrency)
            }
        }
        None => (false, Rows::Http(Vec::new()), 0),
    };

    let focused = app.focus == Panel::Conns && !app.overlay_open();
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(focused))
        .title(Span::styled(" 并发连接 ", panel_title_style(focused)))
        .title(
            Line::from(Span::styled(
                active_title(rows.len(), max_conc),
                Style::default().fg(DIM),
            ))
            .alignment(Alignment::Right),
        );
    let mut sb = ScrollablePanel::new(ScrollbarMode::Both);
    sb.thumb(if focused { FOCUS_BORDER } else { BORDER })
        .pressed_v(app.sb_arrow_pressed(SbKey::ConnsV))
        .pressed_h(app.sb_arrow_pressed(SbKey::ConnsH));
    let inner = sb.render_block(f, area, block);
    app.conns_area = Some(inner);

    if app.sel_task().is_none() {
        app.visible_conns_rows = 0;
        app.conns_area = None;
        f.render_widget(
            Paragraph::new(Span::styled("（未选中任务）", Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    let n_conns = rows.len();
    if n_conns == 0 {
        app.visible_conns_rows = 0;
        app.conns_area = None;
        f.render_widget(
            Paragraph::new(Span::styled("（无并发连接）", Style::default().fg(DIM)))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    let head_cell = |s: &str, wd: usize| Span::styled(pad_right(s, wd), Style::default().fg(DIM));
    let head = if is_bt {
        vec![
            head_cell("地址", BT_COLW[0]),
            Span::raw(" "),
            head_cell("下载速度", BT_COLW[1]),
            Span::raw(" "),
            head_cell("上传速度", BT_COLW[2]),
            Span::raw(" "),
            head_cell("累计下载", BT_COLW[3]),
            Span::raw(" "),
            head_cell("累计上传", BT_COLW[4]),
            Span::raw(" "),
            head_cell("客户端", BT_COLW[5]),
            Span::raw(" "),
            head_cell("协议", BT_COLW[6]),
        ]
    } else {
        vec![
            head_cell("序号", HTTP_COLW[0]),
            Span::raw(" "),
            head_cell("下载速度", HTTP_COLW[1]),
            Span::raw(" "),
            head_cell("累计下载", HTTP_COLW[2]),
        ]
    };
    let mut lines = vec![Line::from(head)];

    const PAD_X: u16 = 2;
    let view = Rect {
        x: inner.x.saturating_add(PAD_X),
        y: inner.y,
        width: inner.width.saturating_sub(PAD_X * 2),
        height: inner.height,
    };

    let rows_h = (view.height as usize).saturating_sub(1);
    let data_rows = rows_h.max(1);
    let start = app.conns_scroll.min(n_conns.saturating_sub(data_rows));
    let end = (start + data_rows).min(n_conns);
    let show_sel = focused;

    for i in start..end {
        let sel = show_sel && i == app.conns_sel;
        let bg = if sel { SEL_BG } else { Color::Reset };
        let cell = |s: String, wd: usize, col: Color| {
            Span::styled(pad_left(&s, wd), Style::default().fg(col).bg(bg))
        };
        let cell_left = |s: String, wd: usize, col: Color| {
            Span::styled(
                pad_right(&truncate(&s, wd), wd),
                Style::default().fg(col).bg(bg),
            )
        };
        let sep = Span::styled(" ", Style::default().bg(bg));
        let sep = move || sep.clone();
        let spans = match &rows {
            Rows::Bt(v) => {
                let (ip, down, up, cum_down, cum_up, client, seeder) = &v[i];
                let dl_on = *down > 0.0;
                let up_on = *up > 0.0;
                vec![
                    Span::styled(
                        ip.clone(),
                        Style::default()
                            .fg(if sel {
                                Color::White
                            } else if *seeder {
                                PINK
                            } else {
                                FG
                            })
                            .bg(bg),
                    ),
                    sep(),
                    cell(
                        if dl_on {
                            fmt_speed(*down)
                        } else {
                            "-".to_string()
                        },
                        BT_COLW[1],
                        if dl_on { LIGHT_BLUE } else { DIM2 },
                    ),
                    sep(),
                    cell(
                        if up_on {
                            fmt_speed(*up)
                        } else {
                            "-".to_string()
                        },
                        BT_COLW[2],
                        if up_on { MAGENTA } else { DIM2 },
                    ),
                    sep(),
                    cell(fmt_size(*cum_down), BT_COLW[3], FG),
                    sep(),
                    cell(fmt_size(*cum_up), BT_COLW[4], FG),
                    sep(),
                    cell_left(client.clone(), BT_COLW[5], DIM),
                    sep(),
                    cell("TCP".to_string(), BT_COLW[6], DIM),
                ]
            }
            Rows::Http(v) => {
                let (cid, speed, cum, transferring) = &v[i];
                vec![
                    cell(format!("{cid}"), 4, if sel { Color::White } else { FG }),
                    sep(),
                    cell(
                        if *transferring {
                            fmt_speed(*speed)
                        } else {
                            "-".to_string()
                        },
                        8,
                        if *transferring { LIGHT_BLUE } else { DIM2 },
                    ),
                    sep(),
                    cell(fmt_size(*cum), 8, FG),
                ]
            }
        };
        lines.push(Line::from(spans));
    }

    app.visible_conns_rows = data_rows;
    let content_w = if is_bt { BT_CONTENT_W } else { HTTP_CONTENT_W };
    app.conns_max_hscroll = content_w
        .saturating_sub(view.width as usize)
        .saturating_add(1);
    let hscroll = app.conns_hscroll.min(app.conns_max_hscroll) as u16;
    f.render_widget(Paragraph::new(lines).scroll((0, hscroll)), view);

    sb.vertical(n_conns, data_rows, app.conns_scroll)
        .horizontal(content_w, view.width as usize, app.conns_hscroll);
    let bars = sb.render_bars(f, area);
    app.conns_v_scrollbar = bars.vertical;
    app.conns_h_scrollbar = bars.horizontal;
}

#[cfg(test)]
mod tests {

    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    use super::super::testfx;
    use super::{BORDER, BT_COLW, BT_CONTENT_W, FOCUS_BORDER, HTTP_COLW, HTTP_CONTENT_W, SEL_BG};
    use crate::app::{App, BtConnRow, Panel};
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

    #[tokio::test]
    async fn hint_line_retired_entirely() {
        let mut app = testfx::make_app_in("ezr-conns", "hint-retired");
        app.tasks.push(task_with_conns(Protocol::Http));
        app.selected = 0;
        let text = conns_render(&mut app, 60, 14).backend().to_string();
        assert!(!text.contains("Ctrl+↑↓"), "HTTP 提示行退役: {text}");
        assert!(!text.contains("Ctrl+B"), "Ctrl+B 提示退役: {text}");
        assert!(!text.contains("（仅 BT）"), "「（仅 BT）」标注退役: {text}");
        let mut app2 = testfx::make_app_in("ezr-conns", "hint-retired-bt");
        app2.tasks.push(task_with_conns(Protocol::Bt));
        app2.selected = 0;
        let text2 = conns_render(&mut app2, 60, 14).backend().to_string();
        assert!(!text2.contains("Ctrl+"), "BT 提示行退役: {text2}");
    }

    #[tokio::test]
    async fn selected_row_highlight_only_when_focused() {
        let mut app = testfx::make_app_in("ezr-conns", "selvis");
        app.tasks.push(task_with_conns(Protocol::Http));
        app.selected = 0;
        app.conns_sel = 1;
        app.focus = Panel::Conns;
        let term = conns_render(&mut app, 60, 14);
        let buf = term.backend().buffer();
        let sel_cell = buf.cell((3, 3)).unwrap();
        assert_eq!(sel_cell.bg, SEL_BG, "聚焦时选中行深青背景");
        app.focus = Panel::List;
        let term = conns_render(&mut app, 60, 14);
        let buf = term.backend().buffer();
        let cell = buf.cell((3, 3)).unwrap();
        assert_ne!(cell.bg, SEL_BG, "非聚焦时选中行不高亮");
    }

    #[tokio::test]
    async fn scrollbar_thumb_color_follows_focus() {
        let mut app = testfx::make_app_in("ezr-conns", "sbar");
        let mut t = task_with_conns(Protocol::Http);
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
        app.focus = Panel::List;
        let term = conns_render(&mut app, 40, 7);
        let buf = term.backend().buffer();
        let dim_thumb = (1..6).any(|y| {
            buf.cell((col, y))
                .map(|c| c.symbol() == "║" && c.fg == BORDER)
                .unwrap_or(false)
        });
        assert!(dim_thumb, "未聚焦时滑块暗色");
        let has_arrow = (1..6).any(|y| {
            buf.cell((col, y))
                .map(|c| c.symbol() == "↑" || c.symbol() == "↓")
                .unwrap_or(false)
        });
        assert!(has_arrow, "上下箭头紧贴角（垂直不留白）");
        let mut app2 = testfx::make_app_in("ezr-conns", "nosbar");
        app2.tasks.push(task_with_conns(Protocol::Http));
        app2.selected = 0;
        let term = conns_render(&mut app2, 40, 12);
        let buf = term.backend().buffer();
        let no_thumb =
            (1..11).all(|y| buf.cell((39, y)).map(|c| c.symbol() != "║").unwrap_or(true));
        assert!(no_thumb, "不超一屏无滚动条");
    }

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

    #[test]
    fn active_title_padded_both_sides() {
        assert_eq!(super::active_title(0, 4), " 0/4 项 ");
        assert_eq!(super::active_title(7, 64), " 7/64 项 ");
    }

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
        assert!(text.contains("3/4 项"), "活跃口径徽标: {text}");
    }

    #[tokio::test]
    async fn bt_seven_columns_render_engine_rows() {
        fn bt_row(addr: &str, down: f64, cum: u64) -> BtConnRow {
            BtConnRow {
                id: 0,
                addr: addr.to_string(),
                down_speed: down,
                cum_down: cum,
                up_speed: 0.0,
                cum_up: 0,
                client: "QB".to_string(),
                seeder: false,
            }
        }
        let mut app = testfx::make_app_in("ezr-conns", "bt7");
        let mut t = task_with_conns(Protocol::Bt);
        t.connections.clear();
        app.tasks.push(t);
        app.selected = 0;
        app.bt_conns.insert(
            1,
            vec![
                bt_row("1.2.3.4:51400", 0.0, 0),
                bt_row("5.6.7.8:51401", 1024.0, 4096),
            ],
        );
        let text = conns_render(&mut app, 60, 14).backend().to_string();
        assert!(text.contains("地址"), "表头 地址: {text}");
        assert!(
            text.contains("下载速度") && text.contains("累计下载"),
            "表头列: {text}"
        );
        assert!(
            text.contains("上传速度") && text.contains("累计上传"),
            "表头列: {text}"
        );
        assert!(
            text.contains("1.*.4/51400"),
            "IPv4 遮掩/端口（斜杠）: {text}"
        );
        assert!(
            text.contains("5.*.8/51401"),
            "第二行 IPv4 遮掩/端口（斜杠）: {text}"
        );
        assert!(
            !text.contains("1.*.4:51400"),
            "旧冒号分隔地址形态退役: {text}"
        );
        assert!(!text.contains("1.2.3.4"), "完整 IP 不得出现: {text}");
        assert!(text.contains("2/4 项"), "徽标 x = bt_conns 行数: {text}");
        app.conns_hscroll = app.conns_max_hscroll;
        let text2 = conns_render(&mut app, 60, 14).backend().to_string();
        assert!(text2.contains("TCP"), "协议列恒 TCP: {text2}");
        assert!(text2.contains("4 KB"), "累计下载: {text2}");
    }

    #[tokio::test]
    async fn bt_seeder_ip_renders_pink() {
        fn bt_row(id: usize, addr: &str, seeder: bool) -> BtConnRow {
            BtConnRow {
                id,
                addr: addr.to_string(),
                down_speed: 0.0,
                cum_down: 0,
                up_speed: 0.0,
                cum_up: 0,
                client: "QB".to_string(),
                seeder,
            }
        }
        let mut app = testfx::make_app_in("ezr-conns", "btseed");
        let mut t = task_with_conns(Protocol::Bt);
        t.connections.clear();
        app.tasks.push(t);
        app.selected = 0;
        app.bt_conns.insert(
            1,
            vec![
                bt_row(0, "1.2.3.4:51400", true),
                bt_row(1, "5.6.7.8:51401", false),
            ],
        );
        let term = conns_render(&mut app, 60, 14);
        let buf = term.backend().buffer();
        let fg_of = |col: u16, row: u16| buf[(col, row)].fg;
        assert_eq!(
            fg_of(3, 2),
            super::super::PINK,
            "seeder IP 用粉色（首数据行 y=2, IP 列 x=3）"
        );
        assert_eq!(fg_of(3, 3), super::super::FG, "非 seeder IP 保持常规前景色");
    }

    #[test]
    fn mask_ip_demo_rules() {
        assert_eq!(super::mask_ip("1.2.3.4"), "1.*.4", "IPv4 首尾组");
        assert_eq!(super::mask_ip("2001:db8::1"), "2001:*:1", "IPv6 首尾组");
        assert_eq!(super::mask_ip("[2001:db8::1]"), "2001:*:1", "IPv6 括号剥离");
        assert_eq!(super::mask_ip("9.9.9.9"), "9.*.9", "单字符组不膨胀");
    }

    #[test]
    fn ip_display_masks_and_caps_width() {
        assert_eq!(super::ip_display("1.2.3.4:51400"), "1.*.4/51400       ");
        assert_eq!(
            super::ip_display("[2001:db8::1]:51400"),
            "2001:*:1/51400    "
        );
        assert_eq!(super::ip_display("2001:db8::1:51400"), "2001:*:1/51400    ");
        assert_eq!(
            super::ip_display("255.255.255.255:65535"),
            "255.*.255/65535   ",
            "IPv4 极值 15 列，右填充至 18"
        );
        assert_eq!(super::ip_display("noport"), "noport            ");
        assert_eq!(
            super::ip_display("ffff::ffff:65535"),
            "ffff:*:ffff/65535 ",
            "IPv6 极值 17 列 = 列宽 18 − 1 空白"
        );
    }

    #[test]
    fn bt_column_widths_match_spec() {
        assert_eq!(BT_COLW[0], 18, "地址列宽 18（ffff:*:ffff/65535 + 1 空白）");
        assert_eq!(
            BT_COLW[5], 34,
            "客户端列宽 34（Propagate Data Client/35.35.35.35 = 33 + 1 空白，v1.36）"
        );
        assert_eq!(BT_CONTENT_W, 94, "BT 内容宽 = 七列 + 6 分隔（v1.36）");
    }

    #[tokio::test]
    async fn bt_header_left_aligned_and_ordered() {
        fn bt_row(addr: &str, down: f64, up: f64) -> BtConnRow {
            BtConnRow {
                id: 0,
                addr: addr.to_string(),
                down_speed: down,
                cum_down: 0,
                up_speed: up,
                cum_up: 0,
                client: "qBittorrent/2.5.0.0".to_string(),
                seeder: false,
            }
        }
        let mut app = testfx::make_app_in("ezr-conns", "bthead");
        let mut t = task_with_conns(Protocol::Bt);
        t.connections.clear();
        app.tasks.push(t);
        app.selected = 0;
        app.bt_conns
            .insert(1, vec![bt_row("1.2.3.4:51400", 1024.0, 512.0)]);
        let text = conns_render(&mut app, 120, 14).backend().to_string();
        let head_line = text
            .lines()
            .find(|l| l.contains("下载速度") && l.contains("客户端"))
            .unwrap_or_else(|| panic!("BT 表头行缺失: {text}"));
        let up_pos = head_line.find("上传速度").unwrap();
        let cum_pos = head_line.find("累计下载").unwrap();
        let down_pos = head_line.find("下载速度").unwrap();
        assert!(
            down_pos < up_pos && up_pos < cum_pos,
            "列序：累计下载在上传速度后: {head_line}"
        );
        let addr_pos = head_line.find("地址").unwrap();
        assert!(
            head_line[..addr_pos]
                .chars()
                .all(|c| c == '"' || c == '│' || c == ' '),
            "地址表头靠左（边框与 padding 之外无填充空白）: {head_line:?}"
        );
        assert!(
            head_line.contains("地址               下载速度"),
            "地址列左对齐、右侧空格填充至 18 列（4 列字宽 + 14 填充 + 1 分隔）: {head_line:?}"
        );
        assert!(
            head_line.contains("客户端                             协议"),
            "客户端列左对齐、右侧空格填充至 34 列（6 列字宽 + 28 填充 + 1 分隔，v1.36）: {head_line:?}"
        );
        let data_line = text
            .lines()
            .find(|l| l.contains("1.*.4/51400"))
            .unwrap_or_else(|| panic!("明细行缺失: {text}"));
        assert!(
            data_line.contains("1.*.4/51400               -"),
            "地址数据左对齐（右填充至 18）+ 速度数据右对齐至 8 列: {data_line:?}"
        );
        assert!(
            data_line.contains("qBittorrent/2.5.0.0                 TCP"),
            "客户端数据左对齐至 34 列（右填充 15 + 分隔 1 + 协议 pad_left 1，v1.36）: {data_line:?}"
        );
    }

    #[tokio::test]
    async fn bt_client_cell_left_aligned_and_truncated() {
        fn bt_row(addr: &str, client: &str) -> BtConnRow {
            BtConnRow {
                id: 0,
                addr: addr.to_string(),
                down_speed: 0.0,
                cum_down: 0,
                up_speed: 0.0,
                cum_up: 0,
                client: client.to_string(),
                seeder: false,
            }
        }
        let mut app = testfx::make_app_in("ezr-conns", "btclient");
        let mut t = task_with_conns(Protocol::Bt);
        t.connections.clear();
        app.tasks.push(t);
        app.selected = 0;
        app.bt_conns.insert(
            1,
            vec![
                bt_row("1.2.3.4:51400", "BitComet 1.64"),
                bt_row(
                    "5.6.7.8:51401",
                    "Propagate Data Client/35.35.35.359999999999999",
                ),
            ],
        );
        let term = conns_render(&mut app, 120, 14);
        let text = term.backend().to_string();
        let line = text
            .lines()
            .find(|l| l.contains("BitComet 1.64"))
            .unwrap_or_else(|| panic!("v 自报名明细行缺失: {text}"));
        assert!(
            line.contains("0 B BitComet 1.64"),
            "客户端数据左对齐（紧跟前列单元格，无右对齐填充空白）: {line:?}"
        );
        assert!(
            text.contains("Propagate Data Client/35.35.35.35…"),
            "超宽客户端截断（省略号收尾，v 自报名可超极值）: {text}"
        );
    }

    #[tokio::test]
    async fn http_header_left_aligned() {
        let mut app = testfx::make_app_in("ezr-conns", "httphead");
        app.tasks.push(task_with_conns(Protocol::Http));
        app.selected = 0;
        let text = conns_render(&mut app, 60, 14).backend().to_string();
        let head_line = text
            .lines()
            .find(|l| l.contains("序号") && l.contains("下载速度"))
            .unwrap_or_else(|| panic!("HTTP 表头行缺失: {text}"));
        let pos = head_line.find("序号").unwrap();
        assert!(
            head_line[..pos]
                .chars()
                .all(|c| c == '"' || c == '│' || c == ' '),
            "序号表头靠左（边框与 padding 之外无填充空白）: {head_line:?}"
        );
        assert!(
            head_line.contains("序号 下载速度"),
            "表头单元格左对齐（单空格分隔、右侧填充无额外空白可见）: {head_line:?}"
        );
    }
}
