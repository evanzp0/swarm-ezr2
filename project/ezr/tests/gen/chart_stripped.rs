use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::LIGHT_BLUE;
use crate::app::App;

#[must_use]
fn chart_columns(hist: &[u64], w: usize, h: usize) -> Vec<usize> {
    let levels = h * 8;
    let n = (((w + 1) * 2).min(hist.len())).max(2);
    let start = hist.len() - n;
    let raw: Vec<f64> = hist[start..].iter().map(|&v| v as f64).collect();
    const ALPHA: f64 = 0.75;
    let mut sm = raw.clone();
    for i in 1..sm.len() {
        sm[i] = sm[i - 1] + (raw[i] - sm[i - 1]) * ALPHA;
    }
    for i in (0..sm.len() - 1).rev() {
        sm[i] += (sm[i + 1] - sm[i]) * ALPHA;
    }

    let hi = sm.iter().copied().fold(f64::MIN, f64::max);
    let lo = sm.iter().copied().fold(f64::MAX, f64::min);
    let pad = ((hi - lo) * 0.15).max(hi * 0.05).max(1.0);
    let y_min = (lo - pad).max(0.0);
    let span = (hi + pad - y_min).max(1.0);
    let last = sm.len() - 1;

    (0..w)
        .map(|c| {
            let mut m = 0.0f64;
            for k in 0..2 {
                let s = ((c * 2 + k) as f64 + 0.5) * last as f64 / (w * 2) as f64;
                let i = (s as usize).min(last);
                let v = sm[i] + (sm[(i + 1).min(last)] - sm[i]) * (s - i as f64);
                let t = ((v - y_min) / span).clamp(0.0, 1.0);
                m = m.max(t);
            }
            ((m * levels as f64).round() as usize).min(levels)
        })
        .collect()
}

pub(super) fn draw_chart(f: &mut Frame, app: &App, area: Rect) {
    if area.width < 2 || area.height < 1 || app.speed_hist.len() < 2 {
        return;
    }
    let w = area.width as usize;
    let h = area.height as usize;
    const PARTIAL: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let px = chart_columns(&app.speed_hist, w, h);
    for row in 0..h {
        let band0 = (h - 1 - row) * 8;
        let line: String = px
            .iter()
            .map(|&p| {
                let rem = p as isize - band0 as isize;
                if rem <= 0 {
                    ' '
                } else {
                    PARTIAL[(rem.min(8) - 1) as usize]
                }
            })
            .collect();
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                line,
                Style::default().fg(LIGHT_BLUE),
            ))),
            Rect {
                x: area.x,
                y: area.y + row as u16,
                width: area.width,
                height: 1,
            },
        );
    }
}

#[cfg(test)]
mod chart_tests {
    use super::*;

    #[test]
    fn chart_columns_within_bounds() {
        for hist in [
            vec![0u64; 4],
            vec![0, 1],
            (0..90).map(|i| (i * 7 % 23) as u64).collect::<Vec<_>>(),
            vec![u64::MAX / 1024, 0, u64::MAX / 1024],
        ] {
            for w in 2..40usize {
                for h in 1..6usize {
                    let levels = h * 8;
                    for &p in &chart_columns(&hist, w, h) {
                        assert!(p <= levels, "列高 {p} 超出 levels {levels}");
                    }
                }
            }
        }
    }

    #[test]
    fn chart_columns_constant_input_fills() {
        let px = chart_columns(&vec![10u64; 40], 20, 3);
        assert!(px.iter().all(|&p| p > 0), "恒定非零输入应填满各列: {px:?}");
    }

    #[tokio::test]
    async fn draw_chart_smoke_no_panic() {
        let app = crate::ui::testfx::make_app_in("ezr-chart", "guard");
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(40, 6)).unwrap();
        term.draw(|f| {
            super::draw_chart(f, &app, ratatui::layout::Rect::new(0, 0, 20, 3));
        })
        .unwrap();
    }
}
