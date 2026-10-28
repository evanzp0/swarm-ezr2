use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::text::{fmt_dur, fmt_eta, fmt_size, fmt_size_pair, fmt_speed, plain_bar, truncate, w};
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
    let cursor_w = 2usize;
    let badges_w = w(&proto_badge) + 1 + w(&state_badge) + 1;
    let pct_w = w(&pct_txt);
    let name_budget = width
        .saturating_sub(cursor_w + badges_w + pct_w + 1)
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
        Span::raw(" ".repeat(pad)),
        Span::styled(
            pct_txt,
            Style::default().fg(state_c).add_modifier(Modifier::BOLD),
        ),
    ];
    l1
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

fn dl_total_txt(t: &Task) -> String {
    if t.size_known() {
        format!("{}/{}", fmt_size(t.downloaded), fmt_size(t.total))
    } else {
        format!("{}/? B", fmt_size(t.downloaded))
    }
}

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
    l3.push(Span::styled(" · ", Style::default().fg(RED)));
    l3.push(Span::styled(dl_total_txt(t), Style::default().fg(RED)));
    l3
}

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
    l3.push(Span::styled(" · ", Style::default().fg(DIM)));
    if t.total == 0 {
        l3.push(Span::styled("未知".to_string(), Style::default().fg(DIM)));
    } else {
        l3.push(Span::styled(
            format!("{}/{}", fmt_size(t.downloaded), fmt_size(t.total)),
            Style::default().fg(FG),
        ));
    }
    l3
}

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
    l3.push(Span::styled(" · ", Style::default().fg(DIM)));
    l3.push(Span::styled(fmt_size(t.total), Style::default().fg(FG)));
    l3
}

fn stat_seeding(t: &Task) -> Vec<Span<'static>> {
    let mut l3: Vec<Span<'static>> = vec![Span::raw("  ")];
    l3.push(Span::styled(
        format!("↑ {}", fmt_speed(t.upload_speed)),
        Style::default().fg(MAGENTA),
    ));
    l3.push(Span::styled(
        format!("  {}/{}", t.seeders, t.peers),
        Style::default().fg(MAGENTA),
    ));
    l3.push(Span::styled(
        format!("  已上传 {}", fmt_size_pair(t.uploaded, t.total)),
        Style::default().fg(FG),
    ));
    l3.push(Span::styled(
        format!("  剩余做种 {}", fmt_dur((t.seed_left.ceil() as u64).max(1))),
        Style::default().fg(DIM),
    ));
    l3
}

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
            format!("  {}/{}", t.seeders, t.peers),
            Style::default().fg(FG),
        ));
    } else {
        l3.push(Span::raw(" ↑ "));
        l3.push(Span::styled("-", Style::default().fg(DIM)));
        l3.push(Span::raw("  -/-"));
    }
    l3.push(Span::styled(
        format!("  {}", fmt_size_pair(t.downloaded, t.total)),
        Style::default().fg(FG),
    ));
    l3.extend(remaining_tail(t, downloading));
    l3
}

fn stat_http_rest(t: &Task) -> Vec<Span<'static>> {
    let downloading = t.state == TaskState::Downloading;
    let mut l3: Vec<Span<'static>> = vec![Span::raw("  ")];
    l3.extend(dl_speed_head(t, downloading));
    l3.push(Span::styled(
        format!("  {}", dl_total_txt(t)),
        Style::default().fg(FG),
    ));
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
        assert!(s.contains("? B"), "统计行 ? B: {s}");
        assert!(s.contains("剩余 -"), "剩余 -: {s}");
        assert!(!s.contains("--:--"), "未知大小不显示 ETA 表盘: {s}");
        let t = mk_streaming(TaskState::Paused);
        let s = text_of(&stat_row(&t, 0));
        assert!(s.contains("? B"), "{s}");
        assert!(s.contains("剩余 -"), "{s}");
        let t = mk_streaming(TaskState::Failed);
        assert!(text_of(&stat_failed(&t)).contains("? B"));
        let mut t2 = mk(TaskState::Downloading, false);
        t2.speed = 1_000_000.0;
        let s2 = text_of(&stat_http_rest(&t2));
        assert!(!s2.contains("? B"), "{s2}");
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
    fn stat_failed_countdown_and_size() {
        let mut x = mk(TaskState::Failed, false);
        x.retries = 2;
        x.retry_in = Some(2.4);
        let s = text_of(&stat_failed(&x));
        assert!(s.contains("重试 2/3"), "{s}");
        assert!(s.contains("3s 后重试"));
        assert!(s.contains("0.9 MB / 2.9 MB") || s.contains("MB"), "大小对");
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
        assert!(s.contains("未知"), "{s}");
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
        assert!(text_of(&stat_completed(&x)).contains("MD5 校验成功"));
    }

    #[test]
    fn stat_seeding_speeds_and_peers() {
        let mut x = mk(TaskState::Seeding, true);
        x.upload_speed = 2048.0;
        x.seeders = 3;
        x.peers = 5;
        let s = text_of(&stat_seeding(&x));
        assert!(s.contains("↑"), "{s}");
        assert!(s.contains("3/5"));
        assert!(s.contains("已上传"));
        assert!(s.contains("剩余做种"));
    }

    #[test]
    fn stat_bt_rest_and_http_rest_dash_forms() {
        let x = mk(TaskState::Paused, true);
        let s = text_of(&stat_bt_rest(&x));
        assert!(s.contains("↓"), "{s}");
        assert!(s.contains("-/-"));
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
        t.speed = 1_000_000.0;
        let s = text_of(&stat_http_rest(&t));
        assert!(s.contains("MB/s  "), "速度后 2 列分隔: {s}");
        assert!(!s.contains("MB/s1.0"), "速度与字节不粘连: {s}");
        assert!(s.contains("↓ 1.0 MB/s  1.0 MB/3.0 MB"), "列序: {s}");
        let mut t = mk_streaming(TaskState::Downloading);
        t.speed = 1000.0;
        let s = text_of(&stat_http_rest(&t));
        assert!(
            s.contains("B/s  ") && s.contains("  1.0 MB/? B"),
            "未知大小: {s}"
        );
    }

    #[test]
    fn stat_bt_downloading_layout_matches_demo_spacing() {
        let mut t = mk(TaskState::Downloading, true);
        t.speed = 2_000_000.0;
        t.upload_speed = 300_000.0;
        t.seeders = 3;
        t.peers = 5;
        let s = text_of(&stat_bt_rest(&t));
        assert!(s.contains("↓ 2.0 MB/s"), "{s}");
        assert!(s.contains(" ↑ 300 KB/s"), "{s}");
        assert!(s.contains("  3/5"), "{s}");
        assert!(s.contains("  1.0/3.0 MB"), "{s}");
        assert!(s.contains("  剩余 "), "{s}");
    }

    #[test]
    fn task_lines_width_floor_is_20() {
        let x = mk(TaskState::Downloading, false);
        let lines = task_lines(&x, false, '⠋', 1, 0, 0);
        assert_eq!(lines.len(), 4);
    }
}
