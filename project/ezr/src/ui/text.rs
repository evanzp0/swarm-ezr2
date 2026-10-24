//! text — 纯文本工具（显示宽度/截断/填充/规格化/进度条，全部可单元测试）

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use super::EMPTY;

/// CJK 全角区间表（显示宽度按 2 计的码点闭区间，FR-01 定稿宽度口径）
const WIDE_RANGES: [(u32, u32); 6] = [
    (0x2E80, 0xA4CF), // CJK 部首扩展 … 谚文音节前段
    (0xAC00, 0xD7A3), // Hangul 音节
    (0xF900, 0xFAFF), // CJK 兼容表意文字
    (0xFF00, 0xFF60), // 全角形式
    (0x3000, 0x303F), // CJK 符号与标点
    (0xFFE0, 0xFFE6), // 全角补足
];

/// 码点显示宽度是否为 2（查表，短路任一区间命中）
fn wide(c: u32) -> bool {
    WIDE_RANGES.iter().any(|&(lo, hi)| lo <= c && c <= hi)
}

/// 码点是否零宽（v1.34/FR-01-113 bug 修订对齐 ezr-demo 重构版：URL 反链接
/// 注入 VS15 后，宽度计算不得把它当占位字符，否则热区/wrap/truncate 全部
/// 偏移）——变体选择符 U+FE00..=FE0F（含本应用使用的 VS15）、零宽空格/
/// 连接符 U+200B..=200F、词连接符 U+2060、BOM U+FEFF——终端渲染均为零宽
///（VS15 兼属组合符号 Mn，各终端零宽口径最稳）
fn zero_width(c: u32) -> bool {
    (0xFE00..=0xFE0F).contains(&c) || (0x200B..=0x200F).contains(&c) || c == 0x2060 || c == 0xFEFF
}

pub fn w(s: &str) -> usize {
    s.chars()
        .map(|c| {
            let u = c as u32;
            if zero_width(u) {
                0
            } else if wide(u) {
                2
            } else {
                1
            }
        })
        .sum()
}

pub fn truncate(s: &str, max: usize) -> String {
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
pub fn pad_right(s: &str, width: usize) -> String {
    let cur = w(s);
    if cur >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - cur))
    }
}

/// 按终端显示宽度左填充空格（数值列右对齐用，CJK 宽度感知）
pub fn pad_left(s: &str, width: usize) -> String {
    let cur = w(s);
    if cur >= width {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(width - cur), s)
    }
}

pub fn fmt_size(bytes: u64) -> String {
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

pub fn fmt_speed(bps: f64) -> String {
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

/// 同单位大小对（BT 行更紧凑）：1.72/4.32 GB
pub fn fmt_size_pair(a: u64, b: u64) -> String {
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
// 断链显示（v1.34/FR-01-113 ④/FR-01-80 ⑤，对齐 ezr-demo 重构版 break_links；
// v1.34 二批 bug 修订：断链机制升级为逐字符深度断链 [`break_links_deep`]，
// 原 scheme 锚点版 [`break_links`] 对 www. 形态 URL 识别无能为力、已随
// 最后一个消费者（详情 URL 行）切换而退役删除）
// ---------------------------------------------------------------------------

/// 反链接显示变换（深度断链）：在**每个字符之后**注入零宽 VS15
///（U+FE0E——零宽组合符号 Mn、Default_Ignorable，各主流终端均零宽渲染且
/// 不产生可见字形）——文本里不再存在任何 ≥2 字符的连续裸串（scheme、域名、
/// 路径、www 前缀、任意片段均不可整体匹配）：终端自身的 URL 自动识别即便
/// 把行内孤立组合符号透明化处理（归一化后正则匹配），也无从拼出连续匹配
/// 串——显示文本因此不再被终端叠加默认链接下划线（根治「详情 URL 值悬停
/// 出现双下划线（终端识别 + ezr 自身 SGR 下划线叠加）」「字段值弹窗内容带
/// 终端默认下划线」；ezr 自身的悬停下划线仍由 SGR 控制不受影响）。仅影响
/// 显示层：复制语义走 App 状态原值（字段名点击 / Y 键），不经注入。VS15 与
/// 宿主字符构成同一 grapheme 簇（零宽），[`w`]/[`truncate`]/[`wrap_text`]
/// 及 ratatui 渲染管线均按零宽透传。
///
/// 消费者：详情面板 URL 行（v1.34 二批 bug 修订自 scheme 锚点版升级）与
/// 字段值弹窗内容行。
///
/// **注意**：断链文本必须经 [`link_break_spans`] 渲染（而非直接作为单
/// Span）——原因见该函数文档。
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

/// 影子前景色：与原色仅最低位灰度 ±1（肉眼不可见），唯一作用是让
/// crossterm 在两段文本之间输出样式切换转义序列（见 [`link_break_spans`]）。
fn fg_shadow(c: Option<Color>) -> Option<Color> {
    match c {
        Some(Color::Rgb(r, g, 0)) => Some(Color::Rgb(r, g.saturating_add(1), 1)),
        Some(Color::Rgb(r, g, b)) => Some(Color::Rgb(r, g, b - 1)),
        other => other,
    }
}

/// 断链文本 span 化（pyte 等简易终端模拟器兼容）：
///
/// VS15 是「零宽但组合类为 0」的字符——pyte 的 `Screen::draw` 对这类字符
/// 直接 `break`，**丢弃同一文本 run 中其后的全部字符**（真终端无此问题，
/// 会继续渲染）。而 ratatui 同一样式相邻单元格连续输出、中间无转义序列，
/// VS15 天然落在 run 中间，pyte 解码画面会丢失 URL 尾部。
///
/// 处理：以每个 VS15 为界把文本切段，奇数段换用「影子前景色」
///（[`fg_shadow`]，与原色肉眼不可分）——crossterm 由此在每段之间输出
/// SGR 切换序列，**每个 VS15 都落在各自文本 run 的末尾**，pyte 在 run 末尾
/// break 零损失；真终端按单元格渲染，网格文本不变、视觉不变、断链效果
/// 不变。段样式交替保证相邻段必不相同（连续多个 VS15 也逐段隔断）。
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

// ---------------------------------------------------------------------------
// 字段值弹窗文本工具（v1.34/FR-01-113）
// ---------------------------------------------------------------------------

/// CJK 感知 wrap text：按显示宽折行（超宽即断行，不做词边界回退——URL/校验码
/// 等无空格长串与中文文案统一按列宽折行），供字段值弹窗完整展示字段值；
/// 每行显示宽 ≤ width（width ≤ 1 时按 1 处理，极端窄窗退化为逐字符折行）
pub(super) fn wrap_text(s: &str, width: usize) -> Vec<String> {
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

// ---------------------------------------------------------------------------
// 走马灯进度条（v1.34/FR-01-116，对齐 ezr-demo 重构版 marquee_bar）
// ---------------------------------------------------------------------------

/// 走马灯进度条（总大小未知的下载中任务）：不支持续传的 HTTP(S) 流式下载
/// 拿不到 Content-Length，百分比/填充进度无从计算——进度条以循环滑动的
/// 色块（marquee）表达「传输进行中、进度不可知」：滑块自左缘滑入、匀速
/// 右移、右缘滑出，空一帧后重新滑入循环。anim = App::frame（100ms/tick 的
/// 10fps 节拍，与 spinner 同源）；每帧 2 列 → 全周期（宽+滑块长）≈ 3s。
pub(super) fn marquee_bar(width: usize, anim: u64, fill: Color) -> Vec<Span<'static>> {
    let width = width.max(1);
    let block = (width / 4).clamp(4, 8);
    let period = width + block; // 滑块完全离场后一个空帧，再从左缘重新滑入
    let head = ((anim as usize * 2) % period) as isize; // 滑块头部（排他）
    let tail = head - block as isize; // 滑块尾部（含）
                                      // 线性裁剪到轨道（不绕环）：滑块 = [tail, head) ∩ [0, width)
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

    // -------------------------------------------------------------------
    // v1.34/FR-01-113 ④ 断链显示（break_links_deep / link_break_spans）
    // -------------------------------------------------------------------

    /// 构造含 VS15 的通用样例（link_break_spans 分段测试用；覆盖多字符段）
    fn vs15_sample() -> String {
        "ab\u{FE0E}cd".to_string()
    }

    #[test]
    fn link_break_spans_segments_at_vs15_boundaries() {
        let s = vs15_sample();
        let spans = link_break_spans(&s, Style::default().fg(FG));
        // 两段（VS15 前/后，多字符段），相邻段样式必不相同（影子色交替）
        assert_eq!(spans.len(), 2, "按 VS15 切段: {spans:?}");
        assert_ne!(spans[0].style.fg, spans[1].style.fg, "相邻段影子色交替");
        assert_eq!(
            spans[0].content, "ab\u{FE0E}",
            "VS15 落在段末（pyte 零丢失）"
        );
        assert_eq!(spans[1].content, "cd");
        // 拼回内容 = 原文（分段零损失）
        let joined: String = spans.iter().map(|x| x.content.as_ref()).collect();
        assert_eq!(joined, s);
        // 无 VS15 的文本单段原样
        let spans = link_break_spans("plain", Style::default().fg(FG));
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "plain");
    }

    // -------------------------------------------------------------------
    // v1.34 bug 修订：深度断链（break_links_deep）——详情 URL 行 + 字段值
    // 弹窗内容共用（www 形态识别归一化后仍命中的终端补强）
    // -------------------------------------------------------------------

    #[test]
    fn break_links_deep_injects_vs15_after_every_char() {
        let raw = "https://www.example.com/f.bin";
        let deep = break_links_deep(raw);
        // 每个字符后都有 VS15：不存在任何 ≥2 字符的连续裸串
        assert!(!deep.contains("https"), "scheme 断: {deep:?}");
        assert!(!deep.contains("://"), "scheme 分隔断: {deep:?}");
        assert!(
            !deep.contains("www"),
            "www 前缀断（二批 bug 靶点）: {deep:?}"
        );
        assert!(!deep.contains("example"), "域名断: {deep:?}");
        assert!(!deep.contains(".bin"), "路径断: {deep:?}");
        // 逐字符剥离 VS15 还原原文（显示内容零改变）
        let restored: String = deep.chars().filter(|&c| c != '\u{FE0E}').collect();
        assert_eq!(restored, raw);
        // 显示宽度不变（VS15 零宽）
        assert_eq!(w(&deep), w(raw), "VS15 零宽：显示宽不变");
        // wrap 折行结果不受注入影响（零宽透传）
        assert_eq!(wrap_text(&deep, 40), vec![deep.clone()]);
        // 空串透传
        assert_eq!(break_links_deep(""), "");
        // CJK 同样逐字断（弹窗可能展示中文错误文案等）
        let zh = break_links_deep("失败原因");
        assert_eq!(w(&zh), 8);
        assert_eq!(zh.chars().filter(|&c| c == '\u{FE0E}').count(), 4);
    }

    /// 深度断链经 link_break_spans 渲染：逐字符分段、相邻段影子色交替、
    /// 拼回 = 断链原文（pyte 兼容形态：VS15 恒在各段末尾）
    #[test]
    fn deep_break_renders_via_link_break_spans() {
        let deep = break_links_deep("http://a.b");
        let spans = link_break_spans(&deep, Style::default().fg(FG));
        // 每字符一段（10 段），段内容 = 单字符 + VS15
        assert_eq!(spans.len(), 10, "逐字符分段: {spans:?}");
        for (i, sp) in spans.iter().enumerate() {
            assert!(
                sp.content.ends_with('\u{FE0E}') && sp.content.chars().count() == 2,
                "段 {i} = 单字符+VS15: {sp:?}"
            );
        }
        // 相邻段样式必不相同（影子色交替——crossterm 逐段输出 SGR 切换）
        for w in spans.windows(2) {
            assert_ne!(w[0].style.fg, w[1].style.fg);
        }
        let joined: String = spans.iter().map(|x| x.content.as_ref()).collect();
        assert_eq!(joined, deep, "分段零损失");
    }

    // -------------------------------------------------------------------
    // v1.34/FR-01-113 wrap_text（CJK 感知折行）
    // -------------------------------------------------------------------

    #[test]
    fn wrap_text_folds_by_display_width() {
        // ASCII 逐列折行
        assert_eq!(wrap_text("abcdef", 4), vec!["abcd", "ef"]);
        // CJK 宽度感知：宽 5 = 2 汉字 + 1 列（第 3 个汉字放不下换行）
        assert_eq!(wrap_text("中文中文", 4), vec!["中文", "中文"]);
        assert_eq!(wrap_text("中中中", 5), vec!["中中", "中"]);
        // 窄到 1 退化为逐字符折行（首字符宽超过 width 时先产出一空行——
        // demo 同形态，仅 VAL_PAD 钳制兜底可达，正常内容区不会出现）
        assert_eq!(wrap_text("中a", 1), vec!["", "中", "a"]);
        // 空串产出一空行
        assert_eq!(wrap_text("", 4), vec![""]);
    }

    // -------------------------------------------------------------------
    // v1.34/FR-01-116 走马灯进度条（marquee_bar）
    // -------------------------------------------------------------------

    #[test]
    fn marquee_bar_block_length_clamped() {
        // 块长 = clamp(宽/4, 4..8)：宽 40 → 8（anim=0 滑块尚未入场 head=0；
        // anim=4 → head=8，滑块完全入场 = 块长 8）
        let spans = marquee_bar(40, 4, GREEN);
        let filled: usize = spans
            .iter()
            .filter(|s| s.content.contains('█'))
            .map(|s| s.content.chars().count())
            .sum();
        assert_eq!(filled, 8, "块长 = min(宽/4, 8)");
        // 宽 12 → 4（下限 4；anim=2 → head=4 完全入场）
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
        // 全周期（宽 + 块长）内任意帧总宽恒定 = width
        for anim in 0..40u64 {
            let spans = marquee_bar(width, anim, GREEN);
            let total: usize = spans
                .iter()
                .flat_map(|s| s.content.chars())
                .filter(|c| *c == '█' || *c == '░')
                .count();
            assert_eq!(total, width, "anim={anim} 总宽恒定");
        }
        // 周期性：anim 与 anim+period 帧画面一致（空一帧重新滑入）
        let block = (width / 4).clamp(4, 8);
        let period = (width + block) as u64;
        let a = marquee_bar(width, 3, GREEN);
        let b = marquee_bar(width, 3 + period, GREEN);
        let at: String = a.iter().map(|s| s.content.as_ref()).collect();
        let bt: String = b.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(at, bt, "走马灯按周期循环");
    }
}
