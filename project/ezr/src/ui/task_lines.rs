use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::text::{fmt_dur, fmt_eta, fmt_size, fmt_speed, plain_bar, truncate, w};
use super::{state_color, ACCENT, DIM, FG, GREEN, LIGHT_BLUE, MAGENTA, RED, YELLOW};
use crate::model::{FailKind, Task, TaskState};

pub(super) fn task_lines(
    t: &Task,
    sel: bool,
    spinner: char,
    width: usize,
    queue_pos: usize,
    anim: u64,
) -> Vec<Line<'static>> {
    let width = width.max(20);
    let state_c = state_color(t.state);
    vec![
        Line::from(title_row(t, sel, width)),
        Line::from(progress_row(t, width, state_c, spinner, anim)),
        Line::from(stat_row(t, queue_pos)),
        Line::from(vec![Span::raw("")]),
    ]
}

fn title_row(t: &Task, sel: bool, width: usize) -> Vec<Span<'static>> {
    let state_c = state_color(t.state);
    let known = t.size_known();
    let pct_txt = if known {
        format!("{:>5.1}%", t.progress() * 100.0)
    } else {
        "    ?%".to_string()
    };
    let proto_badge = format!("[{}]", t.protocol.label());
    let state_badge = format!("[{}]", t.state.label());
    // 任务行标题尺寸项（FR-02-38 v1.18 立项 / v1.19 全类型扩面）：
    // 尺寸已知即在「[状态]」徽标后单空格显示 <总字节>，明细统计行
    // 尺寸项整项退役；尺寸未知不显示，无伪造总字节。
    let title_size = title_size_txt(t);
    let cursor_w = 2usize;
    let badges_w = w(&proto_badge) + 1 + w(&state_badge) + w(&title_size);
    let pct_w = w(&pct_txt);
    // 预留 2 列：1 列名称与徽标间隔 + 1 列徽标/尺寸与右端百分比最小间隔。
    let name_budget = width
        .saturating_sub(cursor_w + badges_w + pct_w + 2)
        .clamp(8, 64);
    let name_txt = truncate(&t.name, name_budget);
    let used = cursor_w + w(&name_txt) + 1 + badges_w;
    let pad = width.saturating_sub(used + pct_w);

    let l1 = vec![
        Span::styled(
            if sel { "▌ " } else { "  " }.to_string(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            name_txt,
            if sel {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG)
            },
        ),
        Span::raw(" "),
        Span::styled(proto_badge, Style::default().fg(YELLOW)),
        Span::raw(" "),
        Span::styled(state_badge, Style::default().fg(state_c)),
        Span::styled(title_size, Style::default().fg(FG)),
        Span::raw(" ".repeat(pad)),
        Span::styled(
            pct_txt,
            Style::default().fg(state_c).add_modifier(Modifier::BOLD),
        ),
    ];
    l1
}

/// 任务行标题尺寸项：所有协议/所有状态下，尺寸已知即在状态徽标后显示
/// <总字节>（FR-02-38 v1.19 全类型/全状态；FR-01-116 size_known 语义）。
/// 尺寸未知时不显示，避免伪造的 0 B。
pub(super) fn title_size_txt(t: &Task) -> String {
    if t.size_known() {
        format!(" {}", fmt_size(t.total))
    } else {
        String::new()
    }
}

/// 分享率文本（FR-02-38，qBt realRatio 同源，D74⑦）：累计上传 ÷ 累计下载；
/// 下载量 < total_done 1% 时按 total_done 兑底（防 seeder 丢统计特例）；
/// 分母 0 且上传 > 0 → ∞（qBt C_INFINITY 同款）；两者均 0 → 0.00。
fn share_ratio_text(t: &Task) -> String {
    let up = t.uploaded as f64;
    let total = t.total as f64;
    let dl = if total > 0.0 && (t.downloaded as f64) < total * 0.01 {
        total
    } else {
        t.downloaded as f64
    };
    if dl == 0.0 {
        if up == 0.0 {
            "0.00".to_string()
        } else {
            "∞".to_string()
        }
    } else {
        format!("{:.2}", up / dl)
    }
}

/// 「 分享 <分享率>」项（D74④）：单空格衔接宿主项（<seeders>/<peers>/<健康度>），
/// 异于行内其他数据项双空格；「分享」标签 DIM，数字 MAGENTA（粉红，操作者钦定）。
fn share_spans(t: &Task) -> Vec<Span<'static>> {
    vec![
        Span::styled(" 分享".to_string(), Style::default().fg(DIM)),
        Span::styled(
            format!(" {}", share_ratio_text(t)),
            Style::default().fg(MAGENTA),
        ),
    ]
}

fn progress_row(
    t: &Task,
    width: usize,
    state_c: Color,
    spinner: char,
    anim: u64,
) -> Vec<Span<'static>> {
    let bar_w = width.saturating_sub(4);
    let mut l2 = vec![Span::raw("  ")];
    if !t.size_known() && t.state == TaskState::Downloading {
        l2.extend(super::text::marquee_bar(bar_w, anim, state_c));
    } else if t.size_known() {
        l2.extend(plain_bar(bar_w, t.progress(), state_c));
    } else {
        l2.extend(plain_bar(bar_w, 0.0, state_c));
    }
    match t.state {
        TaskState::Verifying => {
            l2.push(Span::raw("  "));
            let algo = t.checksum.as_ref().map(|c| c.algo).unwrap_or("SHA-256");
            l2.push(Span::styled(
                format!("{} 分块校验 {}", algo, spinner),
                Style::default().fg(LIGHT_BLUE),
            ));
        }
        TaskState::PostProcessing => {
            l2.push(Span::raw("  "));
            l2.push(Span::styled(
                format!("后期处理中 {}", spinner),
                Style::default().fg(LIGHT_BLUE),
            ));
        }
        _ => {}
    }
    l2
}

fn dl_speed_head(t: &Task, downloading: bool) -> Vec<Span<'static>> {
    if downloading {
        vec![Span::styled(
            format!("↓ {}", fmt_speed(t.speed)),
            Style::default().fg(ACCENT),
        )]
    } else {
        rest_dash_speed()
    }
}

fn rest_dash_speed() -> Vec<Span<'static>> {
    vec![Span::raw("↓ "), Span::styled("-", Style::default().fg(DIM))]
}

fn remaining_tail(t: &Task, downloading: bool) -> Vec<Span<'static>> {
    if downloading && t.size_known() {
        vec![Span::styled(
            format!("  剩余 {}", fmt_eta(t.eta_secs())),
            Style::default().fg(DIM),
        )]
    } else {
        vec![
            Span::raw("  剩余 "),
            Span::styled("-", Style::default().fg(DIM)),
        ]
    }
}

fn stat_row(t: &Task, queue_pos: usize) -> Vec<Span<'static>> {
    match t.state {
        TaskState::Failed => stat_failed(t),
        TaskState::Queued => stat_queued(t, queue_pos),
        TaskState::Completed => stat_completed(t),
        TaskState::Seeding => stat_seeding(t),
        _ => {
            if t.protocol.is_bt() {
                stat_bt_rest(t)
            } else {
                stat_http_rest(t)
            }
        }
    }
}

/// 失败统计行：重试计数 · 倒计时（尾随尺寸对已上移退役，FR-02-38 v1.19）。
fn stat_failed(t: &Task) -> Vec<Span<'static>> {
    let mut l3: Vec<Span<'static>> = vec![Span::raw("  ")];
    let retry = format!("重试 {}/{}", t.retries, t.max_retries);
    let countdown = match t.retry_in {
        Some(s) => format!("{}s 后重试", s.ceil() as u64),
        None => match t.fail_kind {
            Some(FailKind::Fatal) | Some(FailKind::Verify) => "不自动重试".to_string(),
            _ if t.retries >= t.max_retries => "已达上限".to_string(),
            _ => "不自动重试".to_string(),
        },
    };
    l3.push(Span::styled(retry, Style::default().fg(RED)));
    l3.push(Span::styled(" · ", Style::default().fg(RED)));
    l3.push(Span::styled(countdown, Style::default().fg(RED)));
    l3
}

/// 等待统计行：槽位/排队话术（尾随尺寸对已上移退役，FR-02-38 v1.19）。
fn stat_queued(t: &Task, queue_pos: usize) -> Vec<Span<'static>> {
    let mut l3: Vec<Span<'static>> = vec![Span::raw("  ")];
    if t.has_slot {
        l3.push(Span::styled(
            "已获得下载槽位 · 即将开始".to_string(),
            Style::default().fg(YELLOW),
        ));
    } else if queue_pos > 0 {
        l3.push(Span::styled(
            format!("排队第 {} 位 · 等待空闲下载槽位", queue_pos),
            Style::default().fg(YELLOW),
        ));
    }
    l3
}

/// 已完成统计行：校验结论（尾随 ` · total` 已上移退役，FR-02-38 v1.19）。
fn stat_completed(t: &Task) -> Vec<Span<'static>> {
    let mut l3: Vec<Span<'static>> = vec![Span::raw("  ")];
    match (&t.checksum, t.verify_ok) {
        (Some(ck), Some(true)) => l3.push(Span::styled(
            format!("{} 校验成功", ck.algo),
            Style::default().fg(GREEN),
        )),
        (Some(ck), Some(false)) => l3.push(Span::styled(
            format!("{} 校验失败", ck.algo),
            Style::default().fg(RED),
        )),
        _ => l3.push(Span::styled("无校验".to_string(), Style::default().fg(DIM))),
    }
    l3
}

/// 做种统计行（FR-02-09 ② / FR-02-38 v1.20）：
/// `↑ <上传速度>  <seeders>/<peers>/<健康度> 分享 <分享率>  剩余做种 <剩余时长>`。
/// v1.50：`已上传 <已上传>/<总大小>` 项整项退役，上传量仅经分享率体现。
fn stat_seeding(t: &Task) -> Vec<Span<'static>> {
    let mut l3: Vec<Span<'static>> = vec![Span::raw("  ")];
    l3.push(Span::styled(
        format!("↑ {}", fmt_speed(t.upload_speed)),
        Style::default().fg(MAGENTA),
    ));
    l3.push(Span::styled(
        format!("  {}/{}/{:.3}", t.seeders, t.peers, t.availability),
        Style::default().fg(MAGENTA),
    ));
    l3.extend(share_spans(t));
    l3.push(Span::styled(
        format!("  剩余做种 {}", fmt_dur((t.seed_left.ceil() as u64).max(1))),
        Style::default().fg(DIM),
    ));
    l3
}

/// BT 其余状态统计行（下载中/校验中/后期处理/已暂停；FR-02-37/38）：
/// 下载臂 `↓ <速度> ↑ <上传>  <s>/<p>/<健康度> 分享 <r>  剩余 <eta>`；
/// 非下载臂速度/健康度逐位 dash（`-/-/-`），分享率照常显示（D74⑤ 其余两臂）。
fn stat_bt_rest(t: &Task) -> Vec<Span<'static>> {
    let downloading = t.state == TaskState::Downloading;
    let mut l3: Vec<Span<'static>> = vec![Span::raw("  ")];
    l3.extend(dl_speed_head(t, downloading));
    if downloading {
        l3.push(Span::styled(
            format!(" ↑ {}", fmt_speed(t.upload_speed)),
            Style::default().fg(MAGENTA),
        ));
        l3.push(Span::styled(
            format!("  {}/{}/{:.3}", t.seeders, t.peers, t.availability),
            Style::default().fg(FG),
        ));
    } else {
        l3.push(Span::raw(" ↑ "));
        l3.push(Span::styled("-", Style::default().fg(DIM)));
        l3.push(Span::raw("  -/-/-"));
    }
    l3.extend(share_spans(t));
    l3.extend(remaining_tail(t, downloading));
    l3
}

/// HTTP(S) 统计行（尺寸对 `x/y` 与未知尺寸 `? B` 形态已上移退役，v1.49）。
fn stat_http_rest(t: &Task) -> Vec<Span<'static>> {
    let downloading = t.state == TaskState::Downloading;
    let mut l3: Vec<Span<'static>> = vec![Span::raw("  ")];
    l3.extend(dl_speed_head(t, downloading));
    l3.extend(remaining_tail(t, downloading));
    l3
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Checksum, Protocol};

    fn mk(state: TaskState, bt: bool) -> Task {
        let mut t = Task::new_queued(
            1,
            "sample.bin".to_string(),
            if bt { Protocol::Bt } else { Protocol::Http },
            "http://example.com/sample.bin".to_string(),
            "/tmp".to_string(),
            1024 * 1024,
            4,
            3,
            None,
            crate::model::config::ProxyChoice::Direct,
            0,
        );
        t.state = state;
        t.total = 3_000_000;
        t.downloaded = 1_000_000;
        t.probed = true;
        t
    }

    fn text_of(spans: &[Span<'static>]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn title_row_shows_cursor_badges_and_percent() {
        let t = mk(TaskState::Downloading, false);
        let row = title_row(&t, true, 60);
        let s = text_of(&row);
        assert!(s.contains("▌"), "选中光标");
        assert!(s.contains("sample.bin"));
        assert!(s.contains("[HTTP]"));
        assert!(s.contains("33.3%"), "百分比: {s}");
        let row = title_row(&t, false, 60);
        assert!(text_of(&row).starts_with("  "));
    }

    #[test]
    fn title_size_after_state_badge_all_types_all_states() {
        // FR-02-38 v1.19：全类型/全状态，尺寸已知任务行「[状态]」徽标后
        // 单空格显示 <总字节>；未知尺寸不显示。
        for bt in [true, false] {
            for st in [
                TaskState::Downloading,
                TaskState::Seeding,
                TaskState::Completed,
                TaskState::Failed,
                TaskState::Queued,
                TaskState::Paused,
                TaskState::Verifying,
            ] {
                let t = mk(st, bt);
                let s = text_of(&title_row(&t, false, 80));
                let want = format!("[{}] 3.0 MB", st.label());
                assert!(s.contains(&want), "{st:?} bt={bt} 徽标后总字节: {s}");
            }
        }
        let t = mk_streaming(TaskState::Downloading);
        let s = text_of(&title_row(&t, false, 80));
        assert!(s.contains("?%"), "未知尺寸 ?% 保留: {s}");
        assert!(!s.contains("3.0 MB"), "未知尺寸不显示总字节: {s}");
    }

    #[test]
    fn progress_row_activity_hints() {
        let t = mk(TaskState::Verifying, false);
        let row = progress_row(&t, 60, state_color(t.state), '⠋', 0);
        let s = text_of(&row);
        assert!(s.contains("分块校验"), "校验提示: {s}");
        let t = mk(TaskState::PostProcessing, false);
        let row = progress_row(&t, 60, state_color(t.state), '⠋', 0);
        assert!(text_of(&row).contains("后期处理中"));
        let t = mk(TaskState::Downloading, false);
        let row = progress_row(&t, 60, state_color(t.state), '⠋', 0);
        assert!(!text_of(&row).contains("校验"));
    }

    fn mk_streaming(state: TaskState) -> Task {
        let mut t = mk(state, false);
        t.resumable = false;
        t
    }

    #[test]
    fn unknown_size_shows_question_percent() {
        let t = mk_streaming(TaskState::Downloading);
        let s = text_of(&title_row(&t, false, 60));
        assert!(s.contains("?%"), "未知大小 ?%: {s}");
        assert!(!s.contains("33.3%"), "不显示数值百分比: {s}");
        assert_eq!(w("    ?%"), w(&format!("{:>5.1}%", 33.3)));
        let t2 = mk(TaskState::Downloading, false);
        assert!(text_of(&title_row(&t2, false, 60)).contains("33.3%"));
    }

    #[test]
    fn unknown_size_marquee_when_downloading_else_empty_track() {
        let t = mk_streaming(TaskState::Downloading);
        let a0 = text_of(&progress_row(&t, 60, state_color(t.state), '⠋', 4));
        let a5 = text_of(&progress_row(&t, 60, state_color(t.state), '⠋', 9));
        assert!(a0.contains('█'), "下载中走马灯色块: {a0}");
        assert_ne!(a0, a5, "滑块随帧移动");
        for st in [TaskState::Paused, TaskState::Queued, TaskState::Failed] {
            let t = mk_streaming(st);
            let s = text_of(&progress_row(&t, 60, state_color(t.state), '⠋', 3));
            assert!(!s.contains('█'), "{st:?} 空轨道无色块: {s}");
            assert!(s.contains('░'), "{st:?} 空轨道 ░: {s}");
        }
        let t2 = mk(TaskState::Downloading, false);
        assert!(text_of(&progress_row(&t2, 60, state_color(t.state), '⠋', 0)).contains('█'));
    }

    #[test]
    fn unknown_size_stat_rows_and_eta_dash() {
        let mut t = mk_streaming(TaskState::Downloading);
        t.speed = 1000.0;
        let s = text_of(&stat_http_rest(&t));
        assert!(!s.contains("? B"), "列表行 ? B 形态上移退役（v1.49）: {s}");
        assert!(s.contains("剩余 -"), "剩余 -: {s}");
        assert!(!s.contains("--:--"), "未知大小不显示 ETA 表盘: {s}");
        let t = mk_streaming(TaskState::Paused);
        let s = text_of(&stat_row(&t, 0));
        assert!(!s.contains("? B"), "暂停行无伪造尺寸: {s}");
        assert!(s.contains("剩余 -"), "{s}");
        let t = mk_streaming(TaskState::Failed);
        assert!(!text_of(&stat_failed(&t)).contains("? B"));
        let mut t2 = mk(TaskState::Downloading, false);
        t2.speed = 1_000_000.0;
        let s2 = text_of(&stat_http_rest(&t2));
        assert!(!s2.contains("MB"), "尺寸对已上移退役: {s2}");
        assert!(
            s2.contains("剩余 ") && !s2.contains("剩余 -"),
            "ETA 正常: {s2}"
        );
    }

    #[test]
    fn task_lines_anim_threading() {
        let t = mk_streaming(TaskState::Downloading);
        let l = task_lines(&t, false, '⠋', 60, 0, 4);
        let joined: String = l[1].spans.iter().map(|x| x.content.as_ref()).collect();
        assert!(joined.contains('█'));
    }

    #[test]
    fn stat_failed_paused_renders_like_plain_paused() {
        let mut suspended = mk(TaskState::Paused, false);
        suspended.retries = 2;
        suspended.error = Some("连接超时".to_string());
        let plain = mk(TaskState::Paused, false);
        assert_eq!(
            text_of(&stat_row(&suspended, 0)),
            text_of(&stat_row(&plain, 0)),
            "失败挂起与普通暂停统计行一致"
        );
        let s = text_of(&stat_row(&suspended, 0));
        assert!(!s.contains("重试"), "不显示重试计数: {s}");
        assert!(!s.contains("不再自动重试"), "不显示挂起话术: {s}");
        assert!(!s.contains("已暂停（失败）"), "不显示细分状态文案: {s}");
    }

    #[test]
    fn stat_failed_countdown_without_size() {
        let mut x = mk(TaskState::Failed, false);
        x.retries = 2;
        x.retry_in = Some(2.4);
        let s = text_of(&stat_failed(&x));
        assert!(s.contains("重试 2/3"), "{s}");
        assert!(s.contains("3s 后重试"));
        assert!(!s.contains("MB"), "尾随尺寸对已上移退役（v1.49）: {s}");
        assert_eq!(s.matches(" · ").count(), 1, "仅重试·倒计时一处间隔: {s}");
        let mut x = mk(TaskState::Failed, false);
        x.retry_in = None;
        assert!(text_of(&stat_failed(&x)).contains("不自动重试"));
        let mut x = mk(TaskState::Failed, false);
        x.fail_kind = Some(FailKind::Fatal);
        assert!(text_of(&stat_failed(&x)).contains("不自动重试"));
    }

    #[test]
    fn stat_queued_slot_and_position() {
        let mut x = mk(TaskState::Queued, false);
        x.has_slot = false;
        let s = text_of(&stat_queued(&x, 2));
        assert!(s.contains("排队第 2 位"), "{s}");
        x.has_slot = true;
        let s = text_of(&stat_queued(&x, 0));
        assert!(s.contains("已获得下载槽位"), "{s}");
        let mut x = mk(TaskState::Queued, false);
        x.total = 0;
        let s = text_of(&stat_queued(&x, 0));
        assert!(
            !s.contains("未知"),
            "尾随尺寸/未知项已上移退役（v1.49）: {s}"
        );
        let x = mk(TaskState::Queued, false);
        let s = text_of(&stat_queued(&x, 0));
        assert!(!s.contains("MB"), "等待行无尺寸: {s}");
    }

    #[test]
    fn stat_completed_verify_outcomes() {
        let mut x = mk(TaskState::Completed, false);
        let s = text_of(&stat_completed(&x));
        assert!(s.contains("无校验"), "{s}");
        x.checksum = Some(Checksum {
            algo: "MD5",
            value: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
        });
        x.verify_ok = Some(false);
        assert!(text_of(&stat_completed(&x)).contains("MD5 校验失败"));
        x.verify_ok = Some(true);
        let s = text_of(&stat_completed(&x));
        assert!(s.contains("MD5 校验成功"));
        assert!(
            !s.contains(" · "),
            "尾随 ` · total` 已上移退役（v1.49）: {s}"
        );
    }

    #[test]
    fn stat_seeding_speeds_peers_health_share() {
        // FR-02-37/38 + v1.50：三段健康度 + 分享率，已上传对整项退役。
        let mut x = mk(TaskState::Seeding, true);
        x.upload_speed = 2048.0;
        x.seeders = 3;
        x.peers = 5;
        x.availability = 1.675;
        x.uploaded = 300_000;
        x.downloaded = 3_000_000;
        let s = text_of(&stat_seeding(&x));
        assert!(s.contains("↑"), "{s}");
        assert!(s.contains("3/5/1.675"), "做种臂同形态三段: {s}");
        assert!(
            s.contains(" 分享 0.10"),
            "分享率（单空格衔接，2 位小数）: {s}"
        );
        assert!(
            !s.contains("已上传") && !s.contains("300 KB"),
            "已上传对整项退役（v1.50，上传量仅经分享率体现）: {s}"
        );
        assert!(s.contains("剩余做种"));
    }

    #[test]
    fn share_ratio_qbt_real_ratio_rules() {
        // D74⑦：1% 兑底、∞、0.00 三边界。
        let mut t = mk(TaskState::Seeding, true);
        t.total = 0;
        t.downloaded = 0;
        t.uploaded = 0;
        assert_eq!(share_ratio_text(&t), "0.00", "两者均 0 → 0.00");
        // 分母 0（total 未知且无下载）且上传 > 0 → ∞。
        t.uploaded = 4096;
        assert_eq!(share_ratio_text(&t), "∞");
        t.total = 3_000_000;
        t.downloaded = 1_000; // < 1% total_done → 兑底按 total
        t.uploaded = 150_000;
        assert_eq!(share_ratio_text(&t), "0.05", "1% 兑底: 150000/3000000");
        t.downloaded = 1_500_000;
        t.uploaded = 1_125_000;
        assert_eq!(share_ratio_text(&t), "0.75", "正常口径");
        // 下载量小但 total 已知：兜底后分母非 0，不显 ∞（qBt realRatio 同源）。
        t.total = 3_000_000;
        t.downloaded = 0;
        t.uploaded = 4096;
        assert_eq!(
            share_ratio_text(&t),
            "0.00",
            "兑底分母 total ≠ 0 → 数值口径"
        );
    }

    #[test]
    fn share_spans_label_dim_number_magenta() {
        let mut t = mk(TaskState::Seeding, true);
        t.downloaded = 3_000_000;
        t.uploaded = 300_000;
        let sp = share_spans(&t);
        assert_eq!(sp[0].content, " 分享", "单空格衔接（D74④）");
        assert_eq!(sp[0].style.fg, Some(DIM), "分享标签 DIM");
        assert_eq!(sp[1].content, " 0.10");
        assert_eq!(
            sp[1].style.fg,
            Some(MAGENTA),
            "数字粉红 MAGENTA（操作者钦定）"
        );
    }

    #[test]
    fn stat_bt_rest_and_http_rest_dash_forms() {
        let x = mk(TaskState::Paused, true);
        let s = text_of(&stat_bt_rest(&x));
        assert!(s.contains("↓"), "{s}");
        assert!(
            s.contains("-/-/-"),
            "非活跃 BT 逐位 dash 扩位（FR-02-37）: {s}"
        );
        assert!(s.contains("-/-/- 分享"), "dash 臂后单空格衔接分享率: {s}");
        assert!(s.contains(" 分享 0.00"), "其余两臂分享率照常（D74⑤）: {s}");
        assert!(!s.contains("↓ -↓ -"), "速度占位不双写: {s}");
        assert_eq!(s.matches("↓ ").count(), 1, "单个 ↓ 箭头: {s}");
        let x = mk(TaskState::Paused, false);
        let s = text_of(&stat_http_rest(&x));
        assert!(s.contains("↓"), "{s}");
        assert!(s.contains("剩余"));
    }

    #[test]
    fn stat_http_downloading_layout_matches_demo_spacing() {
        let mut t = mk(TaskState::Downloading, false);
        t.speed = 1_048_576.0;
        let s = text_of(&stat_http_rest(&t));
        assert!(s.contains("MiB/s  "), "速度后 2 列分隔（接剩余项）: {s}");
        assert!(!s.contains("MiB/s1.0"), "速度与后项不粘连: {s}");
        assert!(
            s.contains("↓ 1.0 MiB/s  剩余"),
            "尺寸对退役后速度直接接剩余: {s}"
        );
        let mut t = mk_streaming(TaskState::Downloading);
        t.speed = 500.0;
        let s = text_of(&stat_http_rest(&t));
        assert!(
            s.contains("B/s  ") && s.contains("剩余 -"),
            "未知大小：无 ? B、剩余 dash: {s}"
        );
    }

    #[test]
    fn stat_bt_downloading_layout_matches_demo_spacing() {
        // FR-02-37/38：下载臂 `↓ <速度> ↑ <上传>  <s>/<p>/<健康度> 分享 <r>  剩余 <eta>`。
        let mut t = mk(TaskState::Downloading, true);
        t.speed = 2_097_152.0;
        t.upload_speed = 307_200.0;
        t.seeders = 3;
        t.peers = 5;
        t.availability = 1.675;
        t.downloaded = 1_000_000;
        t.uploaded = 70_000;
        let s = text_of(&stat_bt_rest(&t));
        assert!(s.contains("↓ 2.0 MiB/s"), "{s}");
        assert!(s.contains(" ↑ 300 KiB/s"), "{s}");
        assert!(s.contains("  3/5/1.675"), "三段健康度（3 位小数）: {s}");
        assert!(s.contains(" 分享 0.07"), "分享率（D74 示例字面口径）: {s}");
        assert!(
            !s.contains("1.0/3.0 MB"),
            "明细统计行尺寸对已上移退役（v1.18）: {s}"
        );
        assert!(s.contains("  剩余 "), "{s}");
        // 健康度跟随所在项原色：下载臂 FG / 做种臂 MAGENTA（D74③）。
        let sp = stat_bt_rest(&t);
        let seg = sp.iter().find(|x| x.content.contains("3/5/1.675")).unwrap();
        assert_eq!(seg.style.fg, Some(FG), "下载臂三段 FG");
        t.state = TaskState::Seeding;
        let sp2 = stat_seeding(&t);
        let seg2 = sp2
            .iter()
            .find(|x| x.content.contains("3/5/1.675"))
            .unwrap();
        assert_eq!(seg2.style.fg, Some(MAGENTA), "做种臂三段 MAGENTA");
    }

    #[test]
    fn task_lines_width_floor_is_20() {
        let x = mk(TaskState::Downloading, false);
        let lines = task_lines(&x, false, '⠋', 1, 0, 0);
        assert_eq!(lines.len(), 4);
    }
}
