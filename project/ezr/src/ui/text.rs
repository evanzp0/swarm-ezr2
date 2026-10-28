use ratatui::style::{Color, Style};
use ratatui::text::Span;

use super::EMPTY;
pub use crate::textfield::{truncate, w};

pub fn pad_right(s: &str, width: usize) -> String {
    let cur = w(s);
    if cur >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - cur))
    }
}

pub fn pad_left(s: &str, width: usize) -> String {
    let cur = w(s);
    if cur >= width {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(width - cur), s)
    }
}

pub use crate::model::sizefmt::{fmt_size, fmt_size_pair};

pub fn fmt_speed(bps: f64) -> String {
    const MB: f64 = 1_000_000.0;
    let bps = if bps == 0.0 { 0.0 } else { bps };
    if bps >= MB {
        format!("{:.1} MB/s", bps / MB)
    } else if bps >= 1000.0 {
        format!("{:.0} KB/s", bps / 1000.0)
    } else {
        format!("{:.0} B/s", bps)
    }
}

pub fn fmt_eta(secs: Option<u64>) -> String {
    match secs {
        None => "--:--".to_string(),
        Some(s) => fmt_dur(s),
    }
}

pub fn fmt_dur(s: u64) -> String {
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, sec)
    } else {
        format!("{:02}:{:02}", m, sec)
    }
}

pub(super) fn plain_bar(width: usize, frac: f64, fill: Color) -> Vec<Span<'static>> {
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

pub(crate) fn break_links_deep(s: &str) -> String {
    const VS15: &str = "\u{FE0E}";
    let mut out = String::with_capacity(s.len() + s.chars().count() * 3);
    for ch in s.chars() {
        out.push(ch);
        if ch != '\u{FE0E}' {
            out.push_str(VS15);
        }
    }
    out
}

fn fg_shadow(c: Option<Color>) -> Option<Color> {
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

pub(crate) fn wrap_text(s: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0usize;
    for ch in s.chars() {
        let cw = w(&ch.to_string());
        if cur_w + cw > width {
            out.push(std::mem::take(&mut cur));
            cur_w = 0;
        }
        cur.push(ch);
        cur_w += cw;
    }
    out.push(cur);
    out
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

#[cfg(test)]
mod tests {
    use super::super::{FG, GREEN};
    use super::*;

    #[test]
    fn width_counts_cjk_as_two() {
        assert_eq!(w("abc"), 3);
        assert_eq!(w("下载"), 4);
        assert_eq!(w("a 下载 b"), 8);
        assert_eq!(w(""), 0);
    }

    #[test]
    fn truncate_adds_ellipsis_and_respects_width() {
        assert_eq!(truncate("short", 10), "short");
        let t = truncate("very-long-name.bin", 8);
        assert_eq!(w(&t), 8);
        assert!(t.ends_with('…'));
        let t = truncate("下载任务名称很长", 6);
        assert!(w(&t) <= 6);
    }

    #[test]
    fn pad_respects_display_width() {
        assert_eq!(pad_right("ab", 5), "ab   ");
        assert_eq!(pad_right("下载", 5), "下载 ");
        assert_eq!(pad_right("abc", 2), "abc");
    }

    #[test]
    fn fmt_size_thresholds() {
        assert_eq!(fmt_size(999), "999 B");
        assert_eq!(fmt_size(1000), "1 KB");
        assert_eq!(fmt_size(1_500_000), "1.5 MB");
        assert_eq!(fmt_size(2_500_000_000), "2.50 GB");
    }

    #[test]
    fn fmt_speed_normalizes_negative_zero() {
        assert_eq!(fmt_speed(-0.0), "0 B/s");
        assert_eq!(fmt_speed(0.0), "0 B/s");
        assert_eq!(fmt_speed(1500.0), "2 KB/s");
        assert_eq!(fmt_speed(2_000_000.0), "2.0 MB/s");
    }

    #[test]
    fn fmt_dur_and_eta() {
        assert_eq!(fmt_dur(65), "01:05");
        assert_eq!(fmt_dur(3_725), "1:02:05");
        assert_eq!(fmt_eta(None), "--:--");
        assert_eq!(fmt_eta(Some(65)), "01:05");
    }

    #[test]
    fn fmt_size_pair_same_unit() {
        assert_eq!(fmt_size_pair(1_720_000_000, 4_320_000_000), "1.72/4.32 GB");
        assert_eq!(fmt_size_pair(500, 900), "500/900 B");
    }

    #[test]
    fn plain_bar_fills_and_pads() {
        let spans = plain_bar(10, 0.5, GREEN);
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 10);
        assert_eq!(spans[0].content, "█".repeat(5));
        let spans = plain_bar(4, 0.0, GREEN);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "░".repeat(4));
    }

    fn vs15_sample() -> String {
        "ab\u{FE0E}cd".to_string()
    }

    #[test]
    fn link_break_spans_segments_at_vs15_boundaries() {
        let s = vs15_sample();
        let spans = link_break_spans(&s, Style::default().fg(FG));
        assert_eq!(spans.len(), 2, "按 VS15 切段: {spans:?}");
        assert_ne!(spans[0].style.fg, spans[1].style.fg, "相邻段影子色交替");
        assert_eq!(
            spans[0].content, "ab\u{FE0E}",
            "VS15 落在段末（pyte 零丢失）"
        );
        assert_eq!(spans[1].content, "cd");
        let joined: String = spans.iter().map(|x| x.content.as_ref()).collect();
        assert_eq!(joined, s);
        let spans = link_break_spans("plain", Style::default().fg(FG));
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "plain");
    }

    #[test]
    fn break_links_deep_injects_vs15_after_every_char() {
        let raw = "https://www.example.com/f.bin";
        let deep = break_links_deep(raw);
        assert!(!deep.contains("https"), "scheme 断: {deep:?}");
        assert!(!deep.contains("://"), "scheme 分隔断: {deep:?}");
        assert!(
            !deep.contains("www"),
            "www 前缀断（二批 bug 靶点）: {deep:?}"
        );
        assert!(!deep.contains("example"), "域名断: {deep:?}");
        assert!(!deep.contains(".bin"), "路径断: {deep:?}");
        let restored: String = deep.chars().filter(|&c| c != '\u{FE0E}').collect();
        assert_eq!(restored, raw);
        assert_eq!(w(&deep), w(raw), "VS15 零宽：显示宽不变");
        assert_eq!(wrap_text(&deep, 40), vec![deep.clone()]);
        assert_eq!(break_links_deep(""), "");
        let zh = break_links_deep("失败原因");
        assert_eq!(w(&zh), 8);
        assert_eq!(zh.chars().filter(|&c| c == '\u{FE0E}').count(), 4);
    }

    #[test]
    fn deep_break_renders_via_link_break_spans() {
        let deep = break_links_deep("http://a.b");
        let spans = link_break_spans(&deep, Style::default().fg(FG));
        assert_eq!(spans.len(), 10, "逐字符分段: {spans:?}");
        for (i, sp) in spans.iter().enumerate() {
            assert!(
                sp.content.ends_with('\u{FE0E}') && sp.content.chars().count() == 2,
                "段 {i} = 单字符+VS15: {sp:?}"
            );
        }
        for w in spans.windows(2) {
            assert_ne!(w[0].style.fg, w[1].style.fg);
        }
        let joined: String = spans.iter().map(|x| x.content.as_ref()).collect();
        assert_eq!(joined, deep, "分段零损失");
    }

    #[test]
    fn wrap_text_folds_by_display_width() {
        assert_eq!(wrap_text("abcdef", 4), vec!["abcd", "ef"]);
        assert_eq!(wrap_text("中文中文", 4), vec!["中文", "中文"]);
        assert_eq!(wrap_text("中中中", 5), vec!["中中", "中"]);
        assert_eq!(wrap_text("中a", 1), vec!["", "中", "a"]);
        assert_eq!(wrap_text("", 4), vec![""]);
    }

    #[test]
    fn marquee_bar_block_length_clamped() {
        let spans = marquee_bar(40, 4, GREEN);
        let filled: usize = spans
            .iter()
            .filter(|s| s.content.contains('█'))
            .map(|s| s.content.chars().count())
            .sum();
        assert_eq!(filled, 8, "块长 = min(宽/4, 8)");
        let spans = marquee_bar(12, 2, GREEN);
        let filled: usize = spans
            .iter()
            .filter(|s| s.content.contains('█'))
            .map(|s| s.content.chars().count())
            .sum();
        assert_eq!(filled, 4, "块长下限 4");
    }

    #[test]
    fn marquee_bar_cycles_and_total_width_constant() {
        let width = 30;
        for anim in 0..40u64 {
            let spans = marquee_bar(width, anim, GREEN);
            let total: usize = spans
                .iter()
                .flat_map(|s| s.content.chars())
                .filter(|c| *c == '█' || *c == '░')
                .count();
            assert_eq!(total, width, "anim={anim} 总宽恒定");
        }
        let block = (width / 4).clamp(4, 8);
        let period = (width + block) as u64;
        let a = marquee_bar(width, 3, GREEN);
        let b = marquee_bar(width, 3 + period, GREEN);
        let at: String = a.iter().map(|s| s.content.as_ref()).collect();
        let bt: String = b.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(at, bt, "走马灯按周期循环");
    }
}
