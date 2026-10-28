use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use super::text::w;
use super::{ACCENT, BORDER, DIM2, FG, YELLOW};
use crate::app::App;

pub(super) fn draw_footer(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(BORDER));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }
    const PAD: u16 = 2;
    let row_x = inner.x + PAD;
    let row_w = inner.width.saturating_sub(PAD * 2);
    let row1 = Rect {
        x: row_x,
        y: inner.y,
        width: row_w,
        height: 1,
    };
    let row2 = Rect {
        x: row_x,
        y: inner.y + 1,
        width: row_w,
        height: 1,
    };

    app.footer_cmd_rects.clear();
    let cmds = [
        ("A", "命令"),
        ("D", "删除"),
        ("M", "修改"),
        ("Q", "退出"),
        ("H", "帮助"),
    ];
    let mut spans: Vec<Span> = Vec::new();
    let mut x = row1.x;
    let right = row1.x.saturating_add(row1.width);
    for (i, (key, label)) in cmds.iter().enumerate() {
        let chip_w = 2 + w(label) as u16;
        let sep_w = 3u16;
        let placed = spans.is_empty();
        let need = if placed { chip_w } else { sep_w + chip_w };
        if x.saturating_add(need) > right {
            break;
        }
        if !placed {
            spans.push(Span::styled(" · ", Style::default().fg(DIM2)));
            x += sep_w;
        }
        let hot = app.footer_chip_hot(i);
        let (key_style, label_style) = if hot {
            (
                Style::default()
                    .fg(Color::Black)
                    .bg(ACCENT)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::Black).bg(ACCENT),
            )
        } else {
            (
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                Style::default().fg(FG),
            )
        };
        let key_span = Span::styled(format!("{key} "), key_style);
        let label_span = Span::styled((*label).to_string(), label_style);
        app.footer_cmd_rects.push((
            Rect {
                x,
                y: row1.y,
                width: chip_w,
                height: 1,
            },
            i,
        ));
        x += chip_w;
        spans.push(key_span);
        spans.push(label_span);
    }
    f.render_widget(Paragraph::new(Line::from(spans)), row1);

    if let Some(msg) = &app.toast {
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                format!("◆ {msg}"),
                Style::default().fg(YELLOW),
            )])),
            row2,
        );
    }
}

#[cfg(test)]
mod tests {

    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    use super::super::testfx;
    use crate::app::App;

    fn mev(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }

    fn render_footer_rows(app: &mut App, w: u16, h: u16) -> Vec<String> {
        let term = testfx::render_full(app, w, h);
        let rows = testfx::row_strings(&term);
        rows[(h - 4) as usize..].to_vec()
    }

    #[tokio::test]
    async fn footer_double_row_chips_and_toast() {
        let mut app = testfx::make_app_in("ezr-footer", "double");
        app.set_toast("测试消息");
        let rows = render_footer_rows(&mut app, 120, 34);
        assert_eq!(rows.len(), 4, "页脚总高 4 行（边框 2 + 内容 2）");
        let r1 = rows[1].replace(' ', "");
        for chip in ["A命令", "D删除", "M修改", "Q退出", "H帮助"] {
            assert!(r1.contains(chip), "chip「{chip}」: {}", rows[1]);
        }
        assert!(r1.contains("·"), "chips 间 · 分隔: {}", rows[1]);
        let r2 = rows[2].replace(' ', "");
        assert!(r2.contains("◆测试消息"), "toast 独占行: {}", rows[2]);
        assert!(!r2.contains("命令"), "toast 行无 chips: {}", rows[2]);
        let joined = rows.join("\n").replace(' ', "");
        for retired in ["分块", "面板", "暂停", "重试", "上移", "鼠标", "页签"] {
            assert!(
                !joined.contains(retired),
                "历史项「{retired}」应退役: {joined}"
            );
        }
        app.shutdown().await;
    }

    #[tokio::test]
    async fn footer_content_has_two_col_padding() {
        let mut app = testfx::make_app_in("ezr-footer", "pad");
        app.set_toast("内边距");
        let term = testfx::render_full(&mut app, 120, 34);
        let buf = term.backend().buffer();
        let h = 34;
        let top = h - 4;
        let first = (1..10)
            .find(|&x| {
                buf.cell((x, top + 1))
                    .map(|c| c.symbol() != " " && !c.symbol().is_empty())
                    .unwrap_or(false)
            })
            .expect("行 1 有内容");
        assert_eq!(
            first, 3,
            "chips 首列 = 边框内 2 列内边距（PAD=2，v1.34 S2）"
        );
        let first2 = (1..10)
            .find(|&x| {
                buf.cell((x, top + 2))
                    .map(|c| c.symbol() != " " && !c.symbol().is_empty())
                    .unwrap_or(false)
            })
            .expect("行 2 有内容");
        assert_eq!(first2, 3, "toast 首列与 chips 对齐（PAD=2，v1.34 S2）");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn footer_narrow_terminal_drops_oversized_chip() {
        let mut app = testfx::make_app_in("ezr-footer", "p7");
        app.set_toast("窄屏消息");
        let rows = render_footer_rows(&mut app, 40, 24);
        let r1 = rows[1].replace(' ', "");
        for chip in ["A命令", "D删除", "M修改", "Q退出"] {
            assert!(r1.contains(chip), "窄屏可放 chip「{chip}」: {}", rows[1]);
        }
        assert!(
            !r1.contains("帮助"),
            "放不下的 chip 整枚不渲染（P7）: {}",
            rows[1]
        );
        assert!(
            rows[2].replace(' ', "").contains("◆窄屏消息"),
            "toast 行保留: {}",
            rows[2]
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn footer_chip_click_runs_keyboard_equivalent() {
        let mut app = testfx::make_app_in("ezr-footer", "click");
        app.set_toast("点击");
        let term = testfx::render_full(&mut app, 120, 34);
        let rects = app.footer_cmd_rects.clone();
        assert_eq!(rects.len(), 5, "五枚 chip 命中矩形");
        for w in rects.windows(2) {
            assert!(
                w[0].0.x + w[0].0.width < w[1].0.x,
                "矩形不含分隔符（chip 间有间隔列）"
            );
        }
        assert!(rects.iter().all(|(r, _)| r.height == 1), "命中区高 1 行");
        let (h_rect, h_i) = *rects.iter().find(|(_, i)| *i == 4).expect("H chip");
        assert_eq!(h_i, 4);
        app.on_mouse(mev(
            MouseEventKind::Down(MouseButton::Left),
            h_rect.x,
            h_rect.y,
        ));
        assert_eq!(app.footer_press, Some(4), "Down 记录按压");
        assert!(app.footer_chip_hot(4), "按压中 chip 高亮");
        assert!(app.footer_flash.is_some(), "余辉计时已启动");
        app.on_mouse(mev(
            MouseEventKind::Up(MouseButton::Left),
            h_rect.x,
            h_rect.y,
        ));
        assert!(app.help_open, "原地松开执行 H 命令（打开帮助窗口）");
        assert_eq!(app.help_scroll, 0, "帮助打开滚动复位（与键盘同口径）");
        assert_eq!(app.footer_press, None, "松开后按压态清除");
        assert!(!app.quit, "H 不退出");
        drop(term);
        app.shutdown().await;
    }

    #[tokio::test]
    async fn footer_chip_drag_out_cancels() {
        let mut app = testfx::make_app_in("ezr-footer", "dragout");
        let term = testfx::render_full(&mut app, 120, 34);
        let rects = app.footer_cmd_rects.clone();
        let (q_rect, _) = *rects.iter().find(|(_, i)| *i == 3).expect("Q chip");
        app.on_mouse(mev(
            MouseEventKind::Down(MouseButton::Left),
            q_rect.x,
            q_rect.y,
        ));
        assert_eq!(app.footer_press, Some(3));
        app.on_mouse(mev(MouseEventKind::Up(MouseButton::Left), 10, 10));
        assert!(!app.quit, "拖出松开 = 取消（不执行 Q）");
        assert_eq!(app.footer_press, None, "取消后按压态清除");
        drop(term);
        app.shutdown().await;
    }

    #[tokio::test]
    async fn footer_chip_q_quits_a_opens_cmd_window() {
        let mut app = testfx::make_app_in("ezr-footer", "qa");
        let term = testfx::render_full(&mut app, 120, 34);
        let rects = app.footer_cmd_rects.clone();
        let (a_rect, _) = *rects.iter().find(|(_, i)| *i == 0).expect("A chip");
        app.on_mouse(mev(
            MouseEventKind::Down(MouseButton::Left),
            a_rect.x,
            a_rect.y,
        ));
        app.on_mouse(mev(
            MouseEventKind::Up(MouseButton::Left),
            a_rect.x,
            a_rect.y,
        ));
        assert!(app.cmd_open, "A chip 打开命令窗口（FR-02-04②）");
        assert!(app.dialog.is_none(), "A chip 不再直开添加对话框");
        app.close_cmd_window();
        let (q_rect, _) = *rects.iter().find(|(_, i)| *i == 3).expect("Q chip");
        app.on_mouse(mev(
            MouseEventKind::Down(MouseButton::Left),
            q_rect.x,
            q_rect.y,
        ));
        app.on_mouse(mev(
            MouseEventKind::Up(MouseButton::Left),
            q_rect.x,
            q_rect.y,
        ));
        assert!(app.quit, "Q chip 退出（与键盘同口径）");
        drop(term);
        app.shutdown().await;
    }

    #[tokio::test]
    async fn footer_press_cleared_when_overlay_takes_over() {
        let mut app = testfx::make_app_in("ezr-footer", "modal");
        app.footer_press = Some(2);
        app.help_open = true;
        app.help_area = Some(ratatui::layout::Rect::new(20, 5, 70, 24));
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 30, 10));
        assert_eq!(app.footer_press, None, "帮助接管清除按压态");
        app.footer_press = Some(1);
        app.open_pieces();
        app.pieces_area = Some(ratatui::layout::Rect::new(28, 7, 64, 19));
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 40, 12));
        assert_eq!(app.footer_press, None, "pieces 接管清除按压态");
        app.close_pieces();
        app.footer_press = Some(0);
        app.open_add_dialog();
        app.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 60, 17));
        assert_eq!(app.footer_press, None, "对话框接管清除按压态");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn footer_flash_lingers_within_400ms() {
        let mut app = testfx::make_app_in("ezr-footer", "flash");
        app.footer_press = None;
        app.footer_flash = Some((
            1,
            std::time::Instant::now() + std::time::Duration::from_millis(400),
        ));
        assert!(app.footer_chip_hot(1), "余辉窗内 chip 仍高亮");
        assert!(!app.footer_chip_hot(2), "余辉只对被点 chip 生效");
        app.footer_flash = Some((
            1,
            std::time::Instant::now() - std::time::Duration::from_millis(1),
        ));
        assert!(!app.footer_chip_hot(1), "余辉过期恢复常色");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn footer_without_toast_has_no_bullet() {
        let mut app = testfx::make_app_in("ezr-footer", "notoast");
        app.toast = None;
        let rows = render_footer_rows(&mut app, 120, 34);
        let r1 = rows[1].replace(' ', "");
        assert!(r1.contains("帮助") && r1.contains("退出"));
        assert!(!rows[2].contains('◆'), "无 toast 无 ◆: {}", rows[2]);
        app.shutdown().await;
    }
}
