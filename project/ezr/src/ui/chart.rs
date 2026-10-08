//! chart — 单色面积流量图（FR-01-95，定稿口径）
//!
//! 自 header.rs 按渲染区块拆出（architect v116：头部仪表面板 / 流量图 / 页脚
//! 三区块各自成模块；同时把数值核提取为纯函数 [`chart_columns`]，曲线数学
//! 可无头单测与属性测试）。逐列自底向上填充，顶线以 ▁▂▃▄▅▆▇█ 八级部分块
//! 呈现 1/8 格亚字符精度的平滑曲线；全图统一一种颜色（下载淡蓝
//! RGB(122,185,242)，REQ-8.5）；序列经双向 EMA 平滑（α=0.75）+ 窗口 min–max
//! 自适应缩放（上下 ≈15% 呼吸边距，下限取窗口峰值 5%），每列取双子样本较大值
//! （保尖峰）——面积首尾相接、无断列、无间隔、无盲文点阵。

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::LIGHT_BLUE;
use crate::app::App;

/// 图形列高计算（[`draw_chart`] 的纯数值核，architect v116 提取为可无头
/// 单测/属性测试的纯函数）：取最近 2×(列数+1) 个样本（不足则全量）做双向
/// EMA 平滑（α=0.75）→ 窗口 min–max 自适应缩放（上下 ≈15% 呼吸边距、
/// 下限取窗口峰值 5% 与 1.0 的较大者）→ 逐列取双子样本较大值（保尖峰）→
/// 亚像素高度 ∈ [0, levels]（levels = h×8）。`hist` 为 KB/s 采样
/// （与 `App::speed_hist` 同源）。
#[must_use]
fn chart_columns(hist: &[u64], w: usize, h: usize) -> Vec<usize> {
    let levels = h * 8;
    // 取最近 2×(列数+1) 个样本（每列两个子采样），双向 EMA 平滑
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

    // 窗口 min–max 自适应缩放（上下呼吸边距）→ 波形纵贯图区；
    // 边距下限取窗口峰值 5%，保证稳态抖动在图上仍有可见起伏
    let hi = sm.iter().copied().fold(f64::MIN, f64::max);
    let lo = sm.iter().copied().fold(f64::MAX, f64::min);
    let pad = ((hi - lo) * 0.15).max(hi * 0.05).max(1.0);
    let y_min = (lo - pad).max(0.0);
    let span = (hi + pad - y_min).max(1.0);
    let last = sm.len() - 1;

    // 逐列取两个子采样的较大值（保尖峰）→ 归一化高度 t ∈ [0,1] → 亚像素高度
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

/// 单色面积流量图渲染：几何钳制守卫（宽 <2 / 高 <1 / 样本 <2 不绘）+
/// [`chart_columns`] 列高 + 逐行填充（REQ-8.5：全图仅一种颜色，统一下载淡蓝；
/// rem = 列亚像素高度 − 本行亚像素带下界 → ≤0 空格 / ≥8 全块 █ / 其余部分块）
pub(super) fn draw_chart(f: &mut Frame, app: &App, area: Rect) {
    if area.width < 2 || area.height < 1 || app.speed_hist.len() < 2 {
        return;
    }
    let w = area.width as usize;
    let h = area.height as usize;
    // 亚字符精度：每字符行 8 级部分块（▁▂▃▄▅▆▇█），纵向总分辨率 = h×8
    const PARTIAL: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let px = chart_columns(&app.speed_hist, w, h);
    for row in 0..h {
        let band0 = (h - 1 - row) * 8; // 本行覆盖的亚像素带 [band0, band0+8)
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

    /// 列高值域不变量：任意 (样本, w, h) 组合下每列 ∈ [0, levels]；
    /// h≥1 时 levels ≥ 8（含边界 w=2/h=1 与全零样本）
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

    /// 恒定样本 → 全列满格（呼吸边距下恒值窗口仍有可见起伏：span ≥ 1 且
    /// 恒值位于窗口内 → 每列取到正值高度）
    #[test]
    fn chart_columns_constant_input_fills() {
        let px = chart_columns(&vec![10u64; 40], 20, 3);
        assert!(px.iter().all(|&p| p > 0), "恒定非零输入应填满各列: {px:?}");
    }

    /// 渲染冒烟：默认速度历史（90 点全零）下 draw_chart 全流程不 panic
    /// （守卫分支 chart_columns 最小输入由值域测试覆盖）
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
