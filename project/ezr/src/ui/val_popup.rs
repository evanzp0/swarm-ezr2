use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use super::btn::{clip_wide_at_edges, dialog_rect, draw_close_btn};
use super::scrollable_panel::{ScrollablePanel, ScrollbarMode};
use super::text::{break_links_deep, link_break_spans, w, wrap_text};
use super::{panel_border_style, panel_title_style, FG, FOCUS_BORDER};
use crate::app::{App, SbKey};

const VAL_W: u16 = 48;
const VAL_H: u16 = 12;

const VAL_PAD_X: u16 = 2;
const VAL_PAD_Y: u16 = 1;

pub(super) fn draw_val_popup(f: &mut Frame, app: &mut App, area: Rect) {
    let rect = match app.val_pos {
        Some((px, py)) => {
            let dw = VAL_W.min(area.width);
            let dh = VAL_H.min(area.height);
            Rect {
                x: px.min(area.width.saturating_sub(dw)),
                y: py.min(area.height.saturating_sub(dh)),
                width: dw,
                height: dh,
            }
        }
        None => {
            let r = dialog_rect(area, VAL_W, VAL_H);
            app.val_pos = Some((r.x, r.y));
            r
        }
    };
    f.render_widget(Clear, rect);
    clip_wide_at_edges(f, rect);
    let title = format!(" {} ", app.val_field.title());
    let title_w = w(&title);
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(panel_border_style(true))
        .title(ratatui::text::Span::styled(title, panel_title_style(true)));
    let mut sb = ScrollablePanel::new(ScrollbarMode::Vertical);
    sb.thumb(FOCUS_BORDER)
        .pressed_v(app.sb_arrow_pressed(SbKey::Val));
    let inner = sb.render_block(f, rect, block);
    app.val_area = Some(rect);
    app.popup_close_rect = draw_close_btn(f, app, rect, title_w);

    let pad_x = VAL_PAD_X.min(inner.width / 2);
    let pad_y = VAL_PAD_Y.min(inner.height / 2);
    let content = Rect {
        x: inner.x + pad_x,
        y: inner.y + pad_y,
        width: inner.width.saturating_sub(pad_x * 2),
        height: inner.height.saturating_sub(pad_y * 2),
    };
    let view_h = content.height as usize;
    let lines: Vec<Line> = wrap_text(&app.val_text, content.width.max(1) as usize)
        .into_iter()
        .map(|s| {
            Line::from(link_break_spans(
                &break_links_deep(&s),
                Style::default().fg(FG),
            ))
        })
        .collect();
    let total = lines.len();
    let max_scroll = total.saturating_sub(view_h);
    app.val_scroll = app.val_scroll.min(max_scroll);
    let start = app.val_scroll;
    let shown: Vec<Line> = lines.into_iter().skip(start).take(view_h).collect();
    f.render_widget(Paragraph::new(shown), content);

    sb.vertical(total, view_h, start);
    app.val_scrollbar = sb.render_bars(f, rect).vertical;
    app.val_max_scroll = total.saturating_sub(view_h.max(1));
}

#[cfg(test)]
mod tests {

    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::Modifier;
    use ratatui::Terminal;

    use super::super::btn::dialog_rect;
    use super::super::testfx;
    use super::*;
    use crate::app::DetailField;
    use crate::model::TaskState;

    fn app_with_url_popup(url: &str) -> App {
        let mut app = testfx::make_app_in("ezr-valpopup", "deep-break");
        let mut t = crate::model::sample_task();
        t.url = url.to_string();
        t.state = TaskState::Paused;
        app.tasks.push(t);
        app.selected = 0;
        app.open_val_popup(DetailField::Url);
        app
    }

    #[tokio::test]
    async fn popup_content_deep_broken_and_value_unaffected() {
        let url = "https://example.com/media-release.zip";
        let mut app = app_with_url_popup(url);
        assert_eq!(app.val_text, url, "复制语义走原值（快照不注入）");
        let term = testfx::render_full(&mut app, 100, 24);
        let buf = term.backend().buffer();
        let rect = dialog_rect(Rect::new(0, 0, 100, 24), VAL_W, VAL_H);
        let content = Rect {
            x: rect.x + 3,
            y: rect.y + 2,
            width: VAL_W - 6,
            height: VAL_H - 4,
        };
        let mut visible_chars = 0usize;
        let mut broken_chars = 0usize;
        for y in content.y..content.y + content.height {
            for x in content.x..content.x + content.width {
                let sym = buf.cell((x, y)).unwrap().symbol();
                if sym == " " {
                    continue;
                }
                visible_chars += 1;
                if sym.contains('\u{FE0E}') {
                    broken_chars += 1;
                }
            }
        }
        assert_eq!(
            broken_chars, visible_chars,
            "内容区每个可见字符格都携带 VS15（{visible_chars} 格）"
        );
        assert!(visible_chars >= url.chars().count(), "URL 完整可见");
    }

    #[tokio::test]
    async fn popup_has_no_underline_even_on_hover() {
        let mut app = app_with_url_popup("https://example.com/media-release.zip");
        let term = testfx::render_full(&mut app, 100, 24);
        let rect = dialog_rect(Rect::new(0, 0, 100, 24), VAL_W, VAL_H);
        let no_underline = |term: &Terminal<TestBackend>| {
            let buf = term.backend().buffer();
            for y in rect.y..rect.y + rect.height {
                for x in rect.x..rect.x + rect.width {
                    let m = buf.cell((x, y)).unwrap().modifier;
                    assert!(
                        !m.contains(Modifier::UNDERLINED),
                        "弹窗区域 ({x},{y}) 出现下划线修饰"
                    );
                }
            }
        };
        no_underline(&term);
        app.pointer = Some((rect.x + 20, rect.y + 3));
        let term = testfx::render_full(&mut app, 100, 24);
        no_underline(&term);
    }
}
