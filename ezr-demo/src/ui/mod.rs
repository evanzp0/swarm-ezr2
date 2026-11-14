use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, Panel, TaskState};

mod cmd;
mod conns;
mod detail;
mod dialogs;
mod err;
mod footer;
mod header;
mod list;
mod multiline;
mod popups;
mod tree;
use cmd::*;
use conns::*;
use detail::*;
use dialogs::*;
use footer::*;
use header::*;
use list::*;
use multiline::*;
use popups::*;
use tree::*;

pub(crate) const BORDER: Color = Color::Rgb(56, 78, 86);
pub(crate) const FG: Color = Color::Rgb(198, 208, 212);

pub(crate) const DIM: Color = Color::Rgb(126, 141, 147);
pub(crate) const DIM2: Color = Color::Rgb(88, 102, 108);
pub(crate) const SEL_BG: Color = Color::Rgb(22, 42, 48);

pub(crate) const ACCENT: Color = Color::Cyan;
pub(crate) const GREEN: Color = Color::Green;
pub(crate) const YELLOW: Color = Color::Rgb(228, 178, 62);
pub(crate) const RED: Color = Color::Rgb(226, 92, 84);
pub(crate) const MAGENTA: Color = Color::Rgb(214, 112, 214);
pub(crate) const EMPTY: Color = Color::Rgb(54, 68, 74);

const LIGHT_BLUE: Color = Color::Rgb(122, 185, 242);

pub(crate) const FOCUS_BORDER: Color = Color::Green;

pub(crate) fn overlay_open(app: &App) -> bool {
    app.dialog.is_some()
        || app.cmd_open
        || app.err_open
        || app.pieces_open
        || app.trackers_open
        || app.help_open
        || app.about_open
        || app.val_open
}

pub(crate) fn panel_border_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(FOCUS_BORDER)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(BORDER)
    }
}

pub(crate) fn panel_title_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(FOCUS_BORDER)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DIM)
    }
}

pub(crate) fn state_color(s: TaskState) -> Color {
    match s {
        TaskState::Queued => YELLOW,
        TaskState::Downloading | TaskState::Verifying | TaskState::PostProcessing => LIGHT_BLUE,

        TaskState::Paused => Color::White,
        TaskState::Completed => GREEN,
        TaskState::Failed => RED,
        TaskState::Seeding => MAGENTA,
    }
}

pub(crate) fn w(s: &str) -> usize {
    s.chars()
        .map(|c| {
            let u = c as u32;

            if (0xFE00..=0xFE0F).contains(&u)
                || (0x200B..=0x200F).contains(&u)
                || u == 0x2060
                || u == 0xFEFF
            {
                0
            } else if (0x2E80..=0xA4CF).contains(&u)
                || (0xAC00..=0xD7A3).contains(&u)
                || (0xF900..=0xFAFF).contains(&u)
                || (0xFF00..=0xFF60).contains(&u)
                || (0x3000..=0x303F).contains(&u)
                || (0xFFE0..=0xFFE6).contains(&u)
            {
                2
            } else {
                1
            }
        })
        .sum()
}

pub(crate) fn break_links(s: &str) -> String {
    const VS15: &str = "\u{FE0E}";
    s.replace("https://", &format!("https{VS15}://"))
        .replace("http://", &format!("http{VS15}://"))
        .replace("www.", &format!("www{VS15}."))
}

pub(crate) fn fg_shadow(c: Option<Color>) -> Option<Color> {
    match c {
        Some(Color::Rgb(r, g, 0)) => Some(Color::Rgb(r, g.saturating_add(1), 1)),
        Some(Color::Rgb(r, g, b)) => Some(Color::Rgb(r, g, b - 1)),
        other => other,
    }
}

pub(crate) fn link_break_spans(s: &str, style: Style) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut rest = s;
    let mut shadow = false;
    while let Some((i, c)) = rest.char_indices().find(|&(_, c)| c == '\u{FE0E}') {
        let (head, tail) = rest.split_at(i + c.len_utf8());
        let st = if shadow {
            Style {
                fg: fg_shadow(style.fg),
                ..style
            }
        } else {
            style
        };
        spans.push(Span::styled(head.to_string(), st));
        rest = tail;
        shadow = !shadow;
    }
    if !rest.is_empty() {
        let st = if shadow {
            Style {
                fg: fg_shadow(style.fg),
                ..style
            }
        } else {
            style
        };
        spans.push(Span::styled(rest.to_string(), st));
    }
    if spans.is_empty() {
        spans.push(Span::styled(String::new(), style));
    }
    spans
}

/// 任务行 1 文件名三点折叠（FR-01-118/D75）：总长 ≤ max 显示列原样；
/// 超出即保留前 max−3 列 + "..."（三个 ASCII 句点），按字符边界切分不劈宽字。
pub(crate) fn fold_name_dots(s: &str, max: usize) -> String {
    if w(s) <= max {
        return s.to_string();
    }
    let budget = max.saturating_sub(3);
    let mut out = String::new();
    let mut acc = 0;
    for ch in s.chars() {
        let cw = w(&ch.to_string());
        if acc + cw > budget {
            break;
        }
        acc += cw;
        out.push(ch);
    }
    out.push_str("...");
    out
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if w(s) <= max {
        return s.to_string();
    }
    let budget = max.saturating_sub(1);
    let mut out = String::new();
    let mut acc = 0;
    for ch in s.chars() {
        let cw = w(&ch.to_string());
        if acc + cw > budget {
            break;
        }
        acc += cw;
        out.push(ch);
    }
    out.push('…');
    out
}

pub(crate) fn pad_right(s: &str, width: usize) -> String {
    let cur = w(s);
    if cur >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - cur))
    }
}

pub(crate) fn pad_left(s: &str, width: usize) -> String {
    let cur = w(s);
    if cur >= width {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(width - cur), s)
    }
}

pub(crate) fn slice_by_width(s: &str, cut: usize) -> String {
    let mut out = String::new();
    let mut acc = 0usize;
    for ch in s.chars() {
        let cw = w(&ch.to_string());
        if acc >= cut {
            out.push(ch);
        } else if acc + cw > cut {
            out.push_str(&" ".repeat(acc + cw - cut));
        }
        acc += cw;
    }
    out
}

pub(crate) fn slice_spans(spans: &[Span<'static>], cut: usize) -> Vec<Span<'static>> {
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut acc = 0usize;
    for sp in spans {
        let tw = w(&sp.content);
        if tw == 0 {
            continue;
        }
        let end = acc + tw;
        if end <= cut {
            acc = end;
            continue;
        }
        if acc >= cut {
            out.push(sp.clone());
            acc = end;
            continue;
        }
        let vis = tw - (cut - acc);
        let mut s = slice_by_width(&sp.content, cut - acc);
        let got = w(&s);
        if got < vis {
            s.push_str(&" ".repeat(vis - got));
        }
        out.push(Span::styled(s, sp.style));
        acc = end;
    }
    out
}

pub(crate) fn fmt_size(bytes: u64) -> String {
    const MB: f64 = 1_000_000.0;
    const GB: f64 = 1_000_000_000.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= 1000.0 {
        format!("{:.0} KB", b / 1000.0)
    } else {
        format!("{} B", bytes)
    }
}

pub(crate) fn fmt_speed(bps: f64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1_048_576.0;
    const GIB: f64 = 1_073_741_824.0;
    let bps = if bps == 0.0 { 0.0 } else { bps };
    if bps >= GIB {
        format!("{:.2} GiB/s", bps / GIB)
    } else if bps >= MIB {
        format!("{:.1} MiB/s", bps / MIB)
    } else if bps >= KIB {
        format!("{:.0} KiB/s", bps / KIB)
    } else {
        format!("{:.0} B/s", bps)
    }
}

pub(crate) fn fmt_eta(secs: Option<u64>) -> String {
    match secs {
        None => "--:--".to_string(),
        Some(s) => fmt_dur(s),
    }
}

pub(crate) fn fmt_dur(s: u64) -> String {
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, sec)
    } else {
        format!("{:02}:{:02}", m, sec)
    }
}

pub(crate) fn plain_bar(width: usize, frac: f64, fill: Color) -> Vec<Span<'static>> {
    let width = width.max(1);
    let filln = ((width as f64) * frac.clamp(0.0, 1.0)).round() as usize;
    let mut spans = Vec::new();
    if filln > 0 {
        spans.push(Span::styled("█".repeat(filln), Style::default().fg(fill)));
    }
    if filln < width {
        spans.push(Span::styled(
            "░".repeat(width - filln),
            Style::default().fg(EMPTY),
        ));
    }
    spans
}

pub(crate) fn marquee_bar(width: usize, anim: u64, fill: Color) -> Vec<Span<'static>> {
    let width = width.max(1);
    let block = (width / 4).clamp(4, 8);
    let period = width + block;
    let head = ((anim as usize * 2) % period) as isize;
    let tail = head - block as isize;

    let s = tail.clamp(0, width as isize) as usize;
    let e = head.clamp(0, width as isize) as usize;
    let mut spans = Vec::new();
    if s > 0 {
        spans.push(Span::styled("░".repeat(s), Style::default().fg(EMPTY)));
    }
    if s < e {
        spans.push(Span::styled("█".repeat(e - s), Style::default().fg(fill)));
    }
    if e < width {
        spans.push(Span::styled(
            "░".repeat(width - e),
            Style::default().fg(EMPTY),
        ));
    }
    spans
}

pub(crate) fn hovered(pointer: Option<(u16, u16)>, r: Rect) -> bool {
    pointer.is_some_and(|(px, py)| {
        px >= r.x
            && px < r.x.saturating_add(r.width)
            && py >= r.y
            && py < r.y.saturating_add(r.height)
    })
}

pub(crate) fn with_underline(s: Style, on: bool) -> Style {
    if on {
        s.add_modifier(Modifier::UNDERLINED)
    } else {
        s
    }
}

pub(crate) fn draw_close_btn(
    f: &mut Frame,
    app: &App,
    popup: Rect,
    title_w: usize,
) -> Option<Rect> {
    if popup.width < 12 || popup.height < 3 {
        return None;
    }
    let bx = popup.x + popup.width - 5;
    if (bx as usize) < popup.x as usize + 1 + title_w + 2 {
        return None;
    }
    let r = Rect {
        x: bx,
        y: popup.y,
        width: 3,
        height: 1,
    };
    let hot = app.x_press || hovered(app.pointer, r);

    let bracket = panel_border_style(true);
    let x_st = if hot {
        Style::default().fg(RED).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DIM2).remove_modifier(Modifier::BOLD)
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("[", bracket),
            Span::styled("✗", x_st),
            Span::styled("]", bracket),
        ])),
        r,
    );
    Some(r)
}

/// 裁剪弹窗左右边缘外溢的宽字符尾格（防宽字符跨边框渲染错位）。
pub(crate) fn clip_wide_at_edges(f: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let buf = f.buffer_mut();
    let x0 = area.x;
    let xr = area.right() - 1;
    for y in area.y..area.bottom() {
        if x0 > 0 {
            let i = buf.index_of(x0 - 1, y);

            if w(buf.content[i].symbol()) > 1 {
                buf.content[i].set_char(' ');
            }
        }
        if xr + 1 < buf.area.width {
            let i = buf.index_of(xr + 1, y);

            if buf.content[i].symbol().is_empty() {
                buf.content[i].set_char(' ');
            }
        }
    }
}

pub fn draw(f: &mut Frame, app: &mut App) {
    app.dlg_cursor_screen = None;
    let area = f.area();
    let outer = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(8),
        Constraint::Length(4),
    ])
    .split(area);

    let wide = area.width >= 100;

    if !wide || !app.show_panes {
        app.focus = Panel::List;
    }
    if wide {
        draw_header(f, app, outer[0], true);

        if app.show_panes {
            let cols = Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
                .split(outer[1]);
            draw_list(f, app, cols[0]);
            let right =
                Layout::vertical([Constraint::Length(12), Constraint::Min(6)]).split(cols[1]);
            draw_detail(f, app, right[0]);
            draw_conns(f, app, right[1]);
        } else {
            app.conns_area = None;
            app.detail_url_rect = None;
            app.detail_ck_rect = None;
            app.detail_fail_rect = None;
            app.detail_save_rect = None;
            app.detail_val_rects = Vec::new();
            draw_list(f, app, outer[1]);
        }
    } else {
        app.conns_area = None;
        app.detail_url_rect = None;
        app.detail_ck_rect = None;
        app.detail_fail_rect = None;
        app.detail_save_rect = None;
        app.detail_val_rects = Vec::new();
        draw_header(f, app, outer[0], false);
        draw_list(f, app, outer[1]);
    }

    draw_footer(f, app, outer[2]);

    app.popup_close_rect = None;

    if app.dialog.is_some() {
        draw_dialogs(f, app, area);
    }

    if app.cmd_open {
        draw_cmd(f, app, area);
    }

    if app.err_open {
        err::draw_err(f, app, area);
    }

    if app.pieces_open {
        draw_pieces(f, app, area);
    }

    if app.trackers_open {
        draw_trackers(f, app, area);
    }

    if app.help_open {
        draw_help(f, app, area);
    }

    if app.about_open {
        draw_about(f, app, area);
    }

    if app.val_open {
        draw_val_popup(f, app, area);
    }

    if let Some((x, y)) = app.dlg_cursor_screen {
        if !app.pieces_open
            && !app.trackers_open
            && !app.help_open
            && !app.about_open
            && !app.val_open
            && !app.cmd_open
        {
            f.set_cursor_position(ratatui::layout::Position { x, y });
        }
    }

    if app.cmd_open {
        if let Some((x, y)) = app.cmd_cursor_screen {
            f.set_cursor_position(ratatui::layout::Position { x, y });
        }
    } else {
        app.cmd_cursor_screen = None;
    }
}

#[cfg(test)]
mod cursor_tests {

    use ratatui::{backend::TestBackend, Terminal};

    use super::*;
    use crate::app::DialogKind;

    #[test]
    fn dialog_native_cursor_published_and_positioned() {
        let mut app = crate::app::App::new();
        app.open_add_dialog();
        {
            let d = app.dialog.as_mut().unwrap();
            assert_eq!(d.kind, DialogKind::Add);
            d.url.set("https://ezr.dev/");
            d.focus_field(0);
            d.url.set_cursor(8);
        }
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();

        assert_eq!(
            app.dlg_cursor_screen,
            Some((43, 15)),
            "聚焦文本行光标屏幕位（值区起点 + pre 宽）"
        );
        let pos = term.get_cursor_position().unwrap();
        assert_eq!(
            (pos.x, pos.y),
            (43, 15),
            "Terminal 收口把原生光标位下发后端"
        );

        let s = term.backend().to_string();
        assert!(!s.contains('▏'), "内联字形光标退役: {s}");
        assert!(s.contains("https://ezr.dev/"), "URL 原样显示: {s}");

        let win = app.dlg_field_wins[0].as_ref().expect("文本行 win 回填");
        assert_eq!(win.value_x, 35);
        assert_eq!((win.win_lo, win.win_hi), (0, 16));
        assert_eq!(win.cols.last().copied(), Some(16), "cols 末位 = 窗口总宽");

        assert!(app.dlg_field_wins[4].is_none(), "下拉行无窗口几何");
        assert!(app.dlg_field_wins[6].is_none(), "代理行无窗口几何");

        app.dialog.as_mut().unwrap().url.set_cursor(2);
        term.draw(|f| draw(f, &mut app)).unwrap();
        assert_eq!(app.dlg_cursor_screen, Some((37, 15)), "光标列随 cursor");
    }

    #[test]
    fn dialog_field_click_positions_cursor() {
        let mut app = crate::app::App::new();
        app.open_add_dialog();
        {
            let d = app.dialog.as_mut().unwrap();
            d.url.set("https://example.com/path");
            d.focus_field(0);
        }
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();

        assert!(app.dlg_field_wins[0].is_some(), "渲染层窗口几何已回填");

        use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

        let mev = |kind: MouseEventKind, col: u16, row: u16| MouseEvent {
            kind,
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        };
        let click = |app: &mut crate::app::App, col: u16, row: u16| {
            app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), col, row));
            app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), col, row));
        };

        click(&mut app, 40, 15);
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.focus, 0, "点击聚焦 URL 行");
        assert_eq!(d.url.cursor, 5, "光标落点击列对应字符偏移（: 前）");
        let _ = KeyCode::Esc;

        click(&mut app, 90, 15);
        assert_eq!(app.dialog.as_ref().unwrap().url.cursor, 24, "越末 → 字段尾");
    }

    #[test]
    fn arrow_keys_move_cursor_in_text_fields() {
        let mut app = crate::app::App::new();
        app.open_add_dialog();
        use crossterm::event::{KeyCode, KeyModifiers};

        app.dialog.as_mut().unwrap().url.clear();

        for c in ['a', 'b', 'c'] {
            app.on_dialog_key(KeyCode::Char(c), KeyModifiers::empty());
        }
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.url.value, "abc");
        assert_eq!(d.url.cursor, 3, "键入后光标在字段尾");

        app.on_dialog_key(KeyCode::Left, KeyModifiers::empty());
        app.on_dialog_key(KeyCode::Char('X'), KeyModifiers::empty());
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.url.value, "abXc", "中位插入落在光标列");
        assert_eq!(d.url.cursor, 3);
        assert_eq!(d.focus, 0, "方向键不跳焦点");

        app.on_dialog_key(KeyCode::Right, KeyModifiers::empty());
        app.on_dialog_key(KeyCode::Right, KeyModifiers::empty());
        app.on_dialog_key(KeyCode::Down, KeyModifiers::empty());
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.url.cursor, 4, "Right/Down 到尾钳制（行尾）");

        for _ in 0..6 {
            app.on_dialog_key(KeyCode::Left, KeyModifiers::empty());
        }
        app.on_dialog_key(KeyCode::Up, KeyModifiers::empty());
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.url.cursor, 0, "Left/Up 到头钳制（行首）");

        app.on_dialog_key(KeyCode::Right, KeyModifiers::empty());
        app.on_dialog_key(KeyCode::Backspace, KeyModifiers::empty());
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.url.value, "bXc", "光标 1 处退格删前字符");
        assert_eq!(d.url.cursor, 0);

        app.on_dialog_key(KeyCode::Tab, KeyModifiers::empty());
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.focus, 1);
        assert_eq!(
            d.dir.cursor,
            d.dir.len_chars(),
            "焦点切换单源：光标迁至新字段尾"
        );
        assert_eq!(d.dir.cursor, 14);
    }

    #[test]
    fn bt_limit_fields_focus_cursor_after_last_char() {
        // 对账口径（同步自 ezr BT 限速字段聚焦光标修复轮）：BT 添加/修改对话框
        // 「并发(7 列标签)/下载限速(8 列标签)/上传限速(8 列标签)」聚焦时终端原生
        // 光标列 = 行首 + 标签宽 + 2 列提示符 + 值宽 = 最后字符后方一格。
        let mut app = crate::app::App::new();
        app.open_add_bt_dialog_with("/tmp/demo.torrent");
        {
            let d = app.dialog.as_mut().unwrap();
            d.down_limit.set("512 KiB/s");
            d.up_limit.set("2 MiB/s");
        }
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        for (focus, label_cols) in [(3usize, 7u16), (4, 8), (5, 8)] {
            app.dialog.as_mut().unwrap().focus_field(focus);
            term.draw(|f| draw(f, &mut app)).unwrap();
            let value = {
                let d = app.dialog.as_ref().unwrap();
                match focus {
                    3 => d.conns.value.clone(),
                    4 => d.down_limit.value.clone(),
                    _ => d.up_limit.value.clone(),
                }
            };
            let (rect, _) = app
                .dlg_field_rects
                .iter()
                .find(|(_, i)| *i == focus)
                .copied()
                .expect("BT 字段矩形回填");
            let (cx, cy) = app.dlg_cursor_screen.expect("BT 聚焦文本行发布终端光标");
            assert_eq!(cy, rect.y, "focus={focus}: 光标行对齐字段行");
            assert_eq!(
                cx,
                rect.x + label_cols + 2 + crate::textfield::w(&value) as u16,
                "focus={focus}: 光标列 = 行首+标签宽+2+值宽（最后字符后方一格）"
            );
        }
    }
}

#[cfg(test)]
mod conns_flags_tests {
    use ratatui::{backend::TestBackend, Terminal};

    use super::*;
    use crate::app::Panel;

    fn render_conns(app: &mut App, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        let area = Rect::new(0, 0, w, h);
        term.draw(|f| draw_conns(f, app, area)).unwrap();
        term.backend().to_string()
    }

    #[test]
    fn bt_conns_panel_renders_nine_columns_with_flags_and_relevance() {
        let mut app = App::new();
        app.focus = Panel::Conns;
        let bt = *app
            .filtered()
            .iter()
            .find(|&&i| app.tasks[i].protocol.is_bt() && app.tasks[i].state.shows_conns())
            .expect("正在下载页签含可显示并发的 BT 任务");
        app.selected = app.filtered().iter().position(|&i| i == bt).unwrap();
        assert!(!app.tasks[bt].connections.is_empty(), "BT 任务有并发明细");
        app.tasks[bt].connections[0].flags = "DUOI".to_string();
        app.tasks[bt].connections[0].speed = 1024.0;
        app.tasks[bt].connections[0].relevance = 1.0;

        let text = render_conns(&mut app, 130, 14);
        let head = text
            .lines()
            .find(|l| l.contains("地址"))
            .unwrap_or_else(|| panic!("BT 表头行缺失: {text}"));
        assert!(
            head.contains("标志"),
            "表头含标志列（FR-02-33 八列）: {head:?}"
        );
        let a = head.find("地址").unwrap();
        let fl = head.find("标志").unwrap();
        let d = head.find("下载速度").unwrap();
        assert!(a < fl && fl < d, "标志列位于地址后第二列: {head:?}");
        assert!(
            head.contains("相关度") && head.contains("协议"),
            "相关度列与末列协议在位（FR-02-36 九列）: {head:?}"
        );
        let cl = head.find("客户端").unwrap();
        let rel = head.find("相关度").unwrap();
        let p = head.find("协议").unwrap();
        assert!(cl < rel && rel < p, "相关度列位于客户端与协议之间（v1.47）: {head:?}");
        let row = text
            .lines()
            .find(|l| l.contains("TCP"))
            .unwrap_or_else(|| panic!("明细行缺失: {text}"));
        assert!(
            row.contains("DUOI"),
            "极值 4 枚连写 DUOI 左对齐渲染（数据宽 4 + 分隔 1 = 视觉列宽 5）: {row:?}"
        );
        assert!(
            row.contains("100.0%"),
            "相关度百分数一位小数（qBt relevance 口径）: {row:?}"
        );
    }

    #[test]
    fn bt_conns_panel_flags_no_flag_dash_and_shrink_base_89() {
        let mut app = App::new();
        app.focus = Panel::Conns;
        let bt = *app
            .filtered()
            .iter()
            .find(|&&i| app.tasks[i].protocol.is_bt() && app.tasks[i].state.shows_conns())
            .expect("正在下载页签含可显示并发的 BT 任务");
        app.selected = app.filtered().iter().position(|&i| i == bt).unwrap();
        app.tasks[bt].connections[0].flags = "-".to_string();
        let text = render_conns(&mut app, 130, 14);
        assert!(text.contains("标志"), "表头含标志列: {text}");
        assert!(!text.contains('…'), "宽视口无省略号截断: {text}");

        let narrow = render_conns(&mut app, 80, 14);
        let head = narrow
            .lines()
            .find(|l| l.contains("地址"))
            .unwrap_or_else(|| panic!("窄视口表头缺失: {narrow}"));
        assert!(head.contains("标志"), "窄视口标志列仍首屏可见: {head:?}");
        assert!(
            !narrow.contains('…'),
            "收缩基线 89（下限 26）后客户端单元格无省略号（v1.47 同构）: {narrow}"
        );
    }

    #[test]
    fn bt_task_line_shows_health_and_share_ratio() {
        let mut app = App::new();
        let seeding = app
            .tasks
            .iter()
            .position(|t| t.state == TaskState::Seeding)
            .expect("演示数据含做种中 BT 任务");
        {
            let t = &mut app.tasks[seeding];
            t.seeders = 0;
            t.peers = 35;
            t.availability = 2.418;
            t.uploaded = 1_712_000_000;
        }
        let lines = list::task_lines(&app.tasks[seeding], false, ' ', 160, 0, 0);
        let text = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("0(45)/35(120)/2.418"),
            "健康度三段 3 位小数 + x1(x2)/y1(y2)（FR-02-37 v1.3/D80④：做种态 SEEDS 括号外 0、PEERS = 35 非种子直连）: {text}"
        );
        assert!(
            text.contains("SR 0.40"),
            "SR 2 位小数（FR-02-38 v1.21，1712MB/4320MB）: {text}"
        );
    }

    #[test]
    fn help_window_contains_five_flag_legend_entries() {
        let lines = super::popups::help_lines();
        let text = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            lines.len(),
            83,
            "帮助内容 83 行（源码实测基线 81 = 76 + 5 标志缩写条目，v1.51 + 2 SR/LT 条目，FR-02-38 v1.21/D77）"
        );
        for needle in [
            "D=正在下载 · d=欲下被 choke",
            "U=正在上传 · u=欲传被 choke",
            "O=乐观免 choke · I=入连（对方拨入）",
            "K=对方未 choke 我方，且我方对其无需求",
            "?=我方未 choke 对方，且对方对我方无兴趣",
        ] {
            assert!(
                text.contains(needle),
                "帮助【并发连接】组标志缩写条目缺失: {needle}"
            );
        }
    }

    fn flatten(lines: &[ratatui::text::Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    fn fmt_size_pair(a: u64, b: u64) -> String {
        const KB: f64 = 1_000.0;
        const MB: f64 = 1_000_000.0;
        const GB: f64 = 1_000_000_000.0;
        let (x, y) = (a as f64, b as f64);
        let unit = if y >= GB {
            ("GB", GB)
        } else if y >= MB {
            ("MB", MB)
        } else if y >= KB {
            ("KB", KB)
        } else {
            ("B", 1.0)
        };
        let (u, div) = unit;
        if u == "GB" {
            format!("{:.2}/{:.2} {}", x / div, y / div, u)
        } else if u == "MB" {
            format!("{:.1}/{:.1} {}", x / div, y / div, u)
        } else {
            format!("{:.0}/{:.0} {}", x / div, y / div, u)
        }
    }

    #[test]
    fn bt_task_title_carries_total_size_and_detail_drops_pair() {
        let app = App::new();
        let bt = app
            .tasks
            .iter()
            .position(|t| t.protocol.is_bt() && t.state == TaskState::Downloading)
            .expect("演示数据含下载中 BT 任务");
        let t = &app.tasks[bt];
        let total_txt = fmt_size(t.total);
        let rows = flatten(&list::task_lines(t, false, ' ', 160, 0, 0));
        let l1 = &rows[0];
        let l3 = &rows[2];
        assert!(
            l1.contains(&format!("[下载中] {total_txt}")),
            "任务行状态徽标后以间隔显示 <总字节>（操作者调整轮）: {l1:?}"
        );
        assert!(
            !l3.contains("GB"),
            "明细行不再含 <已下载>/<总字节> 尺寸对: {l3:?}"
        );
        assert!(l3.contains("SR"), "SR 缩写项保留（FR-02-38 v1.21）: {l3:?}");
        assert!(l3.contains("LT"), "LT 缩写项保留（FR-02-38 v1.21）: {l3:?}");
        assert!(
            l3.contains(&format!("{}/{:.3}", sp_txt(t), t.availability)),
            "seeders/peers x1(x2)/y1(y2) + 健康度三段保留（FR-02-37 v1.3，D80④ peers=y1 直读）: {l3:?}"
        );
    }

    #[test]
    fn http_task_rows_move_total_to_title_and_drop_pair() {
        let app = App::new();
        let http = app
            .tasks
            .iter()
            .position(|t| t.state == TaskState::Downloading && !t.protocol.is_bt() && t.size_known())
            .expect("演示数据含尺寸已知的下载中 HTTP(S) 任务");
        let t = &app.tasks[http];
        let total_txt = fmt_size(t.total);
        let rows = flatten(&list::task_lines(t, false, ' ', 160, 0, 0));
        let l1 = &rows[0];
        let l3 = &rows[2];
        assert!(
            l1.contains(&format!("[下载中] {total_txt}")),
            "HTTP 任务行状态徽标后以间隔显示 <总字节>（全类型调整轮）: {l1:?}"
        );
        assert!(
            !l3.contains(&fmt_size(t.total)) && !l3.contains(&fmt_size(t.downloaded)),
            "HTTP 明细行不再含 <已下载>/<总字节> 尺寸对: {l3:?}"
        );
        assert!(
            l3.contains(" LT ") && !l3.contains("剩余"),
            "HTTP 明细行「剩余」同改 LT（FR-02-38 v1.22/D77②，v1.54 裁决扩面）: {l3:?}"
        );
    }

    #[test]
    fn unknown_size_http_task_hides_title_size_and_pair() {
        let app = App::new();
        let unk = app
            .tasks
            .iter()
            .position(|t| t.state == TaskState::Downloading && !t.protocol.is_bt() && !t.size_known())
            .expect("演示数据含尺寸未知的下载中 HTTP 任务");
        let t = &app.tasks[unk];
        let rows = flatten(&list::task_lines(t, false, ' ', 160, 0, 0));
        let l1 = &rows[0];
        let l3 = &rows[2];
        assert!(
            l1.ends_with("?%"),
            "尺寸未知任务行进度列仍显示 ?%（无伪造总字节）: {l1:?}"
        );
        assert!(
            !l3.contains("? B"),
            "尺寸未知任务明细行不再含 <已下载>/? B 对: {l3:?}"
        );
    }

    #[test]
    fn failed_queued_completed_details_drop_size_items() {
        let app = App::new();
        // 已失败：任务行携带总字节，明细只剩重试/倒计时
        let f = app
            .tasks
            .iter()
            .position(|t| t.state == TaskState::Failed)
            .expect("演示数据含已失败任务");
        let tf = &app.tasks[f];
        let rows = flatten(&list::task_lines(tf, false, ' ', 160, 0, 0));
        assert!(
            rows[0].contains(&format!("[已失败] {}", fmt_size(tf.total))),
            "已失败任务行状态徽标后显示 <总字节>: {:?}",
            rows[0]
        );
        assert!(
            !rows[2].contains("GB") && !rows[2].contains("MB"),
            "已失败明细行不再含尺寸对: {:?}",
            rows[2]
        );
        assert!(rows[2].contains("重试"), "已失败明细行保留重试信息: {:?}", rows[2]);
        // 等待中：明细只剩排队/槽位提示
        let q = app
            .tasks
            .iter()
            .position(|t| t.state == TaskState::Queued)
            .expect("演示数据含等待中任务");
        let tq = &app.tasks[q];
        let rows = flatten(&list::task_lines(tq, false, ' ', 160, 2, 0));
        assert!(
            rows[0].contains(&format!("[等待中] {}", fmt_size(tq.total))),
            "等待中任务行状态徽标后显示 <总字节>: {:?}",
            rows[0]
        );
        assert!(
            !rows[2].contains("GB") && !rows[2].contains("MB"),
            "等待中明细行不再含尺寸对: {:?}",
            rows[2]
        );
        assert!(
            rows[2].contains("排队") || rows[2].contains("槽位"),
            "等待中明细行保留排队提示: {:?}",
            rows[2]
        );
        // 已完成：明细只剩校验结论
        let c = app
            .tasks
            .iter()
            .position(|t| t.state == TaskState::Completed)
            .expect("演示数据含已完成任务");
        let tc = &app.tasks[c];
        let rows = flatten(&list::task_lines(tc, false, ' ', 160, 0, 0));
        assert!(
            rows[0].contains(&format!("[已完成] {}", fmt_size(tc.total))),
            "已完成任务行状态徽标后显示 <总字节>: {:?}",
            rows[0]
        );
        assert!(
            !rows[2].contains("GB") && !rows[2].contains("MB"),
            "已完成明细行不再含尺寸（总字节已上移任务行）: {:?}",
            rows[2]
        );
        assert!(
            rows[2].contains("校验成功") || rows[2].contains("无校验"),
            "已完成明细行保留校验结论: {:?}",
            rows[2]
        );
    }

    #[test]
    fn seeding_task_title_carries_total_and_detail_drops_uploaded_pair() {
        let app = App::new();
        let seeding = app
            .tasks
            .iter()
            .position(|t| t.state == TaskState::Seeding)
            .expect("演示数据含做种中 BT 任务");
        let t = &app.tasks[seeding];
        let total_txt = fmt_size(t.total);
        let rows = flatten(&list::task_lines(t, false, ' ', 160, 0, 0));
        let l1 = &rows[0];
        let l3 = &rows[2];
        assert!(
            l1.contains(&format!("[做种中] {total_txt}")),
            "做种中任务行状态徽标后显示 <总字节>（全类型统一口径）: {l1:?}"
        );
        assert!(
            !l3.contains("已上传") && !l3.contains(&fmt_size_pair(t.uploaded, t.total)),
            "做种明细行已上传对退役（v1.50，上传量仅经分享率体现）: {l3:?}"
        );
        assert!(l3.contains("SR"), "做种明细行保留 SR 缩写项: {l3:?}");
        assert!(
            !l3.contains("剩余做种"),
            "做种明细行剩余做种项退役（FR-02-38 v1.21，退出规则归 FR-02-40）: {l3:?}"
        );
    }

    #[test]
    fn slice_spans_window_width_invariant_mixed_cjk() {
        let spans = vec![
            Span::raw(pad_right("地址", 18)),
            Span::raw(" "),
            Span::raw(pad_left("下载速度", 12)),
            Span::raw(" "),
            Span::raw(pad_right("qBittorrent/5.1.2", 26)),
            Span::raw(" "),
            Span::raw(pad_left("相关度", 7)),
            Span::raw(" "),
            Span::raw(pad_right("TCP", 8)),
        ];
        let total: usize = spans.iter().map(|s| w(&s.content)).sum();
        for cut in 0..=total {
            let out = slice_spans(&spans, cut);
            let got: usize = out.iter().map(|s| w(&s.content)).sum();
            assert_eq!(got, total - cut, "cut={cut} 切片后可见宽度恒等");
        }
    }

    #[test]
    fn slice_spans_keeps_header_and_data_cells_aligned_at_any_offset() {
        let cw = 26usize;
        let head = vec![
            Span::raw(pad_right("地址", 18)),
            Span::raw(" "),
            Span::raw(pad_right("标志", 4)),
            Span::raw(" "),
            Span::raw(pad_left("下载速度", 12)),
            Span::raw(" "),
            Span::raw(pad_left("上传速度", 12)),
            Span::raw(" "),
            Span::raw(pad_left("累计下载", 10)),
            Span::raw(" "),
            Span::raw(pad_left("累计上传", 10)),
            Span::raw(" "),
            Span::raw(pad_right("客户端", cw)),
            Span::raw(" "),
            Span::raw(pad_left("相关度", 7)),
            Span::raw(" "),
            Span::raw(pad_right("协议", 8)),
        ];
        let row = vec![
            Span::raw(pad_right("185.*.92/4829", 18)),
            Span::raw(" "),
            Span::raw(pad_right("DU", 4)),
            Span::raw(" "),
            Span::raw(pad_left("229 KiB/s", 12)),
            Span::raw(" "),
            Span::raw(pad_left("28 KiB/s", 12)),
            Span::raw(" "),
            Span::raw(pad_left("1.4 MB", 10)),
            Span::raw(" "),
            Span::raw(pad_left("242 KB", 10)),
            Span::raw(" "),
            Span::raw(pad_right("qBittorrent/5.1.2", cw)),
            Span::raw(" "),
            Span::raw(pad_left("36.1%", 7)),
            Span::raw(" "),
            Span::raw(pad_right("TCP", 8)),
        ];
        let proto_start =
            18 + 1 + 4 + 1 + 12 + 1 + 12 + 1 + 10 + 1 + 10 + 1 + cw + 1 + 7 + 1;
        let pos_of = |spans: &[Span<'static>], token: &str| -> Option<usize> {
            let mut acc = 0usize;
            for s in spans {
                if let Some(i) = s.content.find(token) {
                    return Some(acc + w(&s.content[..i]));
                }
                acc += w(&s.content);
            }
            None
        };
        for cut in 0..proto_start + 8 {
            let h = slice_spans(&head, cut);
            let d = slice_spans(&row, cut);
            let hp = pos_of(&h, "协").map(|p| p + cut);
            let dp = pos_of(&d, "TCP").map(|p| p + cut);
            assert_eq!(
                hp, dp,
                "cut={cut} 协议列单元格起点在任意滚动偏移下表头/数据对齐"
            );
        }
    }

    #[test]
    fn conns_panel_renders_protocol_aligned_at_straddle_scroll() {
        let mut app = App::new();
        app.focus = Panel::Conns;
        let bt = *app
            .filtered()
            .iter()
            .find(|&&i| app.tasks[i].protocol.is_bt() && app.tasks[i].state.shows_conns())
            .expect("正在下载页签含可显示并发的 BT 任务");
        app.selected = app.filtered().iter().position(|&i| i == bt).unwrap();
        app.conns_hscroll = 59;
        let w = 61u16;
        let mut term = Terminal::new(TestBackend::new(w, 12)).unwrap();
        let area = Rect::new(0, 0, w, 12);
        term.draw(|f| draw_conns(f, &mut app, area)).unwrap();
        let buf = term.backend().buffer().clone();
        let cell_at = |x: u16, y: u16| buf[(x, y)].symbol().to_string();
        let mut xie = None;
        let mut tcp = None;
        for y in 0..12u16 {
            for x in 0..w {
                let s = cell_at(x, y);
                if s == "协" && xie.is_none() {
                    xie = Some((x, y));
                }
                if s == "T" && tcp.is_none() {
                    let right: String = (1..3).map(|k| cell_at(x + k, y)).collect();
                    if right == "CP" {
                        tcp = Some((x, y));
                    }
                }
            }
        }
        let (hx, hy) = xie.expect("表头协议列在位");
        let (dx, dy) = tcp.expect("数据 TCP 值在位");
        assert_eq!(
            hx, dx,
            "跨列滚动偏移（hscroll=59 切入「载」右半）下协议表头与数据同列渲染"
        );
        assert_ne!(hy, dy, "表头行与数据行分层");
    }

    #[test]
    fn tab_switch_preserves_and_restores_selection_and_scroll() {
        let mut app = App::new();
        // 页签 0：选中第 3 项（滚动置顶附近）后切走
        app.selected = 2;
        app.scroll = 0;
        let origin_id = app.tasks[app.filtered()[2]].id;
        // 切到「已完成」页签，改动选中/滚动后再切回
        app.cycle_filter(1);
        app.selected = app.filtered().len() - 1;
        app.scroll_to_selection();
        assert_ne!(app.sel_task().map(|t| t.id), Some(origin_id));
        app.cycle_filter(-1);
        assert_eq!(app.filter, 0);
        assert_eq!(
            app.sel_task().map(|t| t.id),
            Some(origin_id),
            "切回原页签选回原任务（快照按任务身份还原，FR-01-117）"
        );
        assert_eq!(app.selected, 2, "选中索引还原");
        assert_eq!(app.scroll, 0, "滚动位置还原");
    }

    #[test]
    fn tab_switch_relocates_selection_by_task_identity_when_tab_set_shifts() {
        let mut app = App::new();
        app.selected = 2;
        app.scroll = 0;
        let origin_id = app.tasks[app.filtered()[2]].id;
        app.cycle_filter(1);
        // 离开期间页签 0 集合位移：首位任务转「已完成」移出页签 0，其后任务索引整体前移
        // （纯索引还原会选中错位邻居 = 操作者报告的缺陷形态）
        app.tasks[0].state = TaskState::Completed;
        app.cycle_filter(-1);
        assert_eq!(
            app.sel_task().map(|t| t.id),
            Some(origin_id),
            "页签集合位移后仍按任务身份选回原任务"
        );
        assert_eq!(app.selected, 1, "选中索引随集合位移重新定位到原任务新位置");
    }

    #[test]
    fn tab_switch_clamps_restored_state_when_tab_set_shrinks_and_renders_all_rows() {
        let mut app = App::new();
        // 页签 1 选中尾项并让视口跟随
        app.cycle_filter(1);
        app.selected = app.filtered().len() - 1;
        app.scroll_to_selection();
        let tail_id = app.sel_task().map(|t| t.id).expect("页签 1 非空");
        app.cycle_filter(-1);
        // 离开期间页签 1 集合收缩：原尾项任务转「下载中」移出页签 1
        let ti = app
            .tasks
            .iter()
            .position(|t| t.id == tail_id)
            .expect("任务仍在 tasks 中");
        app.tasks[ti].state = TaskState::Downloading;
        app.cycle_filter(1);
        let flen = app.filtered().len();
        assert!(
            app.selected < flen,
            "还原状态越界时夹紧（selected={}/len={flen}）",
            app.selected
        );
        assert_eq!(app.selected, flen - 1, "原尾项已移出 → 兜底夹紧到新尾项");
        // 渲染列表：视口足以容纳全部任务时必须全量渲染
        // （还原 scroll 越界时 draw_list 按视口容量自愈夹紧兜底）
        let w = 110u16;
        let h = 34u16;
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        let area = Rect::new(0, 0, w, h);
        term.draw(|f| draw_list(f, &mut app, area)).unwrap();
        let buf = term.backend().buffer().clone();
        let mut rows: Vec<String> = vec![String::new(); h as usize];
        for y in 0..h {
            for x in 0..w {
                rows[y as usize].push_str(buf[(x, y)].symbol());
            }
        }
        let text = rows.join("\n");
        // ratatui Buffer 宽字符占位格为空格符号，去空格归一化后再查 CJK 徽标
        let compact = text.replace(' ', "");
        assert!(
            compact.contains("[做种中]") && compact.contains("[已完成]"),
            "切回页签后全列表渲染（做种中/已完成行同屏可见）: {text}"
        );
        assert!(
            compact.contains(&format!("{flen}/{flen}项")),
            "列表计数行显示 {flen}/{flen} 项: {text}"
        );
    }
}

#[cfg(test)]
mod fmt_speed_binary_tests {
    use super::*;

    #[test]
    fn speed_units_binary_four_tiers() {
        assert_eq!(fmt_speed(0.0), "0 B/s");
        assert_eq!(fmt_speed(1023.0), "1023 B/s");
        assert_eq!(fmt_speed(1024.0), "1 KiB/s");
        assert_eq!(fmt_speed(1500.0), "1 KiB/s");
        assert_eq!(fmt_speed(307_200.0), "300 KiB/s");
        assert_eq!(fmt_speed(1_048_576.0), "1.0 MiB/s");
        assert_eq!(fmt_speed(1_073_741_824.0), "1.00 GiB/s");
        assert!(!fmt_speed(2_000_000.0).contains("MB/s"));
        assert!(!fmt_speed(1500.0).contains("KB/s"));
    }
}

    #[test]
    fn fold_name_dots_caps_at_32_display_widths() {
        // 32 ASCII 列恰好原样（FR-01-118/D75）
        let exact = "a".repeat(32);
        assert_eq!(fold_name_dots(&exact, 32), exact);
        // 33 ASCII 列 → 前 29 列 + "..."（三个 ASCII 句点，共 32 列）
        let long = "a".repeat(33);
        let folded = fold_name_dots(&long, 32);
        assert_eq!(folded, format!("{}...", "a".repeat(29)));
        assert_eq!(w(&folded), 32);
        // 16 个汉字 = 32 显示列，恰好不折叠
        let cjk16 = "汉".repeat(16);
        assert_eq!(fold_name_dots(&cjk16, 32), cjk16);
        // 17 个汉字 = 34 列 → 折叠到 29 列内完整汉字 + "..."（不劈半）
        let cjk17 = "汉".repeat(17);
        let folded_cjk = fold_name_dots(&cjk17, 32);
        assert!(folded_cjk.ends_with("..."));
        assert!(w(&folded_cjk) <= 32, "折叠后总长恒 ≤ 32（FR-01-118 上限）");
        assert_eq!(w(&folded_cjk), 31, "CJK 粒度边界：预算 29 列容纳 14 个完整汉字（28 列）+ 3 点 = 31 ≤ 32");
        assert!(folded_cjk.starts_with(&"汉".repeat(14)));
        assert!(!folded_cjk.contains('\u{FFFD}'));
        // 预算 < 32（窄面板）继续按预算折叠
        let folded20 = fold_name_dots(&long, 20);
        assert_eq!(w(&folded20), 20);
    }

    #[test]
    fn sp_txt_omits_parens_when_tracker_unknown() {
        let app = App::new();
        let bt = app
            .tasks
            .iter()
            .position(|t| t.protocol.is_bt() && t.tracker_seeds.is_some())
            .expect("演示数据含带 tracker 值的 BT 任务");
        let t = &app.tasks[bt];
        let x2 = t.tracker_seeds.unwrap();
        let y2 = t.tracker_peers.unwrap();
        let y1 = t.peers;
        assert_eq!(
            list::sp_txt(t),
            format!("{}({})/{}({})", t.seeders, x2, y1, y2),
            "y1 = 连接的非种子 peers 数（Task.peers 与产品口径一致直读，FR-02-37 v1.3/D80④）"
        );
        let mut t2 = t.clone();
        t2.tracker_seeds = None;
        t2.tracker_peers = None;
        assert_eq!(
            list::sp_txt(&t2),
            format!("{}/{}", t.seeders, y1),
            "tracker 值未知 → 括号段省略（D78②，qBt total == -1 同款）"
        );
    }
