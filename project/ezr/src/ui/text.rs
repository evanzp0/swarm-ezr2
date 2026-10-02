//! text — 纯文本工具（显示宽度/截断/填充/规格化/进度条，全部可单元测试）

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use super::EMPTY;

pub(super) fn w(s: &str) -> usize {
    s.chars()
        .map(|c| {
            let u = c as u32;
            if (0x2E80..=0xA4CF).contains(&u)
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

pub(super) fn truncate(s: &str, max: usize) -> String {
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

/// 按终端显示宽度右填充空格（Rust format! 的宽度按字符数计，对 CJK 不准）
pub(super) fn pad_right(s: &str, width: usize) -> String {
    let cur = w(s);
    if cur >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - cur))
    }
}

pub(super) fn fmt_size(bytes: u64) -> String {
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

pub(super) fn fmt_speed(bps: f64) -> String {
    const MB: f64 = 1_000_000.0;
    let bps = if bps == 0.0 { 0.0 } else { bps }; // 归一化负零（-0.0 == 0.0），避免「-0 B/s」显示
    if bps >= MB {
        format!("{:.1} MB/s", bps / MB)
    } else if bps >= 1000.0 {
        format!("{:.0} KB/s", bps / 1000.0)
    } else {
        format!("{:.0} B/s", bps)
    }
}

pub(super) fn fmt_eta(secs: Option<u64>) -> String {
    match secs {
        None => "--:--".to_string(),
        Some(s) => fmt_dur(s),
    }
}

pub(super) fn fmt_dur(s: u64) -> String {
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, sec)
    } else {
        format!("{:02}:{:02}", m, sec)
    }
}

/// 同单位大小对（BT 行更紧凑）：1.72/4.32 GB
pub(super) fn fmt_size_pair(a: u64, b: u64) -> String {
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

// ---------------------------------------------------------------------------
// 进度条
// ---------------------------------------------------------------------------

/// 列表整体进度条（不展示分块）
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

// ---------------------------------------------------------------------------
// 任务条目（3 行）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::GREEN;
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
        // CJK 不会被切成半格
        let t = truncate("下载任务名称很长", 6);
        assert!(w(&t) <= 6);
    }

    #[test]
    fn pad_respects_display_width() {
        assert_eq!(pad_right("ab", 5), "ab   ");
        assert_eq!(pad_right("下载", 5), "下载 ");
        assert_eq!(pad_right("abc", 2), "abc"); // 超宽不截断
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
        // 空进度仍产出整条空槽
        let spans = plain_bar(4, 0.0, GREEN);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "░".repeat(4));
    }
}
