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
        || app.pieces_open
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
    const MB: f64 = 1_000_000.0;
    if bps >= MB {
        format!("{:.1} MB/s", bps / MB)
    } else if bps >= 1000.0 {
        format!("{:.0} KB/s", bps / 1000.0)
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

pub(crate) fn fmt_size_pair(a: u64, b: u64) -> String {
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

    if app.pieces_open {
        draw_pieces(f, app, area);
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
        if !app.pieces_open && !app.help_open && !app.about_open && !app.val_open && !app.cmd_open {
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
            Some((43, 16)),
            "聚焦文本行光标屏幕位（值区起点 + pre 宽）"
        );
        let pos = term.get_cursor_position().unwrap();
        assert_eq!(
            (pos.x, pos.y),
            (43, 16),
            "Terminal 收口把原生光标位下发后端"
        );

        let s = term.backend().to_string();
        assert!(!s.contains('▏'), "内联字形光标退役: {s}");
        assert!(s.contains("https://ezr.dev/"), "URL 原样显示: {s}");

        let win = app.dlg_field_wins[0].as_ref().expect("文本行 win 回填");
        assert_eq!(win.value_x, 35);
        assert_eq!((win.win_lo, win.win_hi), (0, 16));
        assert_eq!(win.cols.last().copied(), Some(16), "cols 末位 = 窗口总宽");

        assert!(app.dlg_field_wins[3].is_none(), "下拉行无窗口几何");

        app.dialog.as_mut().unwrap().url.set_cursor(2);
        term.draw(|f| draw(f, &mut app)).unwrap();
        assert_eq!(app.dlg_cursor_screen, Some((37, 16)), "光标列随 cursor");
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

        click(&mut app, 40, 16);
        let d = app.dialog.as_ref().unwrap();
        assert_eq!(d.focus, 0, "点击聚焦 URL 行");
        assert_eq!(d.url.cursor, 5, "光标落点击列对应字符偏移（: 前）");
        let _ = KeyCode::Esc;

        click(&mut app, 90, 16);
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
}
