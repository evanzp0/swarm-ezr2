//! task_lines — 列表条目三行构造（纯函数：名称行/进度条行/统计行）
//!
//! 拆分口径：[`title_row`]（光标+名称+徽标+百分比）/ [`progress_row`]（整体进度条）
//! / [`stat_row`]（按状态分派的统计行，各状态一个纯函数生成器，逐一单测锁定文案）。

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::model::{FailKind, Task, TaskState};

use super::state_color;
use super::text::{fmt_dur, fmt_eta, fmt_size, fmt_size_pair, fmt_speed, plain_bar, truncate, w};
use super::{ACCENT, DIM, FG, GREEN, LIGHT_BLUE, MAGENTA, RED, YELLOW};

pub(super) fn task_lines(
    t: &Task,
    sel: bool,
    spinner: char,
    width: usize,
    queue_pos: usize,
) -> Vec<Line<'static>> {
    let width = width.max(20);
    let state_c = state_color(t.state);
    vec![
        Line::from(title_row(t, sel, width)),
        Line::from(progress_row(t, width, state_c, spinner)),
        Line::from(stat_row(t, queue_pos)),
        Line::from(vec![Span::raw("")]),
    ]
}

/// 行 1：光标 + 名称 + 徽标 + 右对齐百分比
fn title_row(t: &Task, sel: bool, width: usize) -> Vec<Span<'static>> {
    let state_c = state_color(t.state);
    // ---- 行 1：光标 + 名称 + 徽标 + 右对齐百分比 ----
    // 等待中任务显示真实进度（可能持有断点进度）
    let pct_txt = format!("{:>5.1}%", t.progress() * 100.0);
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
        Span::styled(
            proto_badge,
            // 协议徽标统一黄色
            Style::default().fg(YELLOW),
        ),
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

/// 行 2：整体进度条（不分块）+ 校验/后期处理活动提示
fn progress_row(t: &Task, width: usize, state_c: Color, spinner: char) -> Vec<Span<'static>> {
    let bar_w = width.saturating_sub(4);
    let mut l2 = vec![Span::raw("  ")];
    l2.extend(plain_bar(bar_w, t.progress(), state_c));
    // 行内动态提示（非状态文案，仅校验/后期处理的活动提示）
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

/// 下载速度列：下载中 = 实速（ACCENT），其余 = 灰色箭头 + 占位 -
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

/// 其余状态（非下载中）的速度列头：↓ 箭头 + 灰色占位 -；返回该片段
fn rest_dash_speed() -> Vec<Span<'static>> {
    vec![Span::raw("↓ "), Span::styled("-", Style::default().fg(DIM))]
}

/// 剩余时间片段：下载中显示 ETA，其余显示灰色 -
fn remaining_tail(t: &Task, downloading: bool) -> Vec<Span<'static>> {
    if downloading {
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

/// 行 3：统计信息（不显示状态文案；缺失数值用 -）——按状态分派
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

/// 已失败：已重试次数/上限 · 重试倒计时 · 已下载/需要下载
fn stat_failed(t: &Task) -> Vec<Span<'static>> {
    let mut l3: Vec<Span<'static>> = vec![Span::raw("  ")];
    // 已重试次数/上限 · 重试倒计时 · 已下载/需要下载
    let retry = format!("重试 {}/{}", t.retries, t.max_retries);
    let countdown = match t.retry_in {
        Some(s) => format!("{}s 后重试", s.ceil() as u64),
        // 无倒计时：区分「已达上限」与「不自动重试」（语义性 4xx /
        // 磁盘空间不足 / 校验失败直接停等，FR-M1-43/44/51）。
        // 瞬态失败未达上限却停等 = auto_retry=false 配置（Gherkin
        // 01-retry-backoff-10：显示「不自动重试」而非「已达上限」）
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
    l3.push(Span::styled(
        format!("{}/{}", fmt_size(t.downloaded), fmt_size(t.total)),
        Style::default().fg(RED),
    ));
    l3
}

/// 等待中：槽位排队信息 + 大小（未探测显示未知）
fn stat_queued(t: &Task, queue_pos: usize) -> Vec<Span<'static>> {
    let mut l3: Vec<Span<'static>> = vec![Span::raw("  ")];
    // 槽位排队信息：位次按列表顺序从上往下（已获槽位的重试任务显示即将开始）
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
        // 等待任务不预取（规格 01-download-engine-15）：探测前大小显示「未知」
        l3.push(Span::styled("未知".to_string(), Style::default().fg(DIM)));
    } else {
        l3.push(Span::styled(
            format!("{}/{}", fmt_size(t.downloaded), fmt_size(t.total)),
            Style::default().fg(FG),
        ));
    }
    l3
}

/// 已完成：校验情况（按算法显示）+ 文件大小
fn stat_completed(t: &Task) -> Vec<Span<'static>> {
    let mut l3: Vec<Span<'static>> = vec![Span::raw("  ")];
    // 校验情况（按算法显示）+ 文件大小
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

/// 做种中：↑ 速度 · 种子/节点 · 已上传/需要下载 · 剩余做种时间
fn stat_seeding(t: &Task) -> Vec<Span<'static>> {
    let mut l3: Vec<Span<'static>> = vec![Span::raw("  ")];
    // ↑ 速度 · 种子/节点 · 已上传/需要下载 · 剩余做种时间
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
        format!("  剩余做种 {}", fmt_dur(t.seed_left.ceil() as u64)),
        Style::default().fg(DIM),
    ));
    l3
}

/// BT 其余状态：同下载中版式，缺失用 -
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
        // 非下载中（暂停/排队等）：速度缺失用 -，但 ↓/↑ 箭头保留
        l3.extend(rest_dash_speed());
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

/// HTTP 其余状态：同下载中版式，缺失用 -
fn stat_http_rest(t: &Task) -> Vec<Span<'static>> {
    let downloading = t.state == TaskState::Downloading;
    let mut l3: Vec<Span<'static>> = vec![Span::raw("  ")];
    l3.extend(dl_speed_head(t, downloading));
    l3.push(Span::styled(
        format!("  {}/{}", fmt_size(t.downloaded), fmt_size(t.total)),
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
        // 未选中：空光标
        let row = title_row(&t, false, 60);
        assert!(text_of(&row).starts_with("  "));
    }

    #[test]
    fn progress_row_activity_hints() {
        let t = mk(TaskState::Verifying, false);
        let row = progress_row(&t, 60, state_color(t.state), '⠋');
        let s = text_of(&row);
        assert!(s.contains("分块校验"), "校验提示: {s}");
        let t = mk(TaskState::PostProcessing, false);
        let row = progress_row(&t, 60, state_color(t.state), '⠋');
        assert!(text_of(&row).contains("后期处理中"));
        // 下载中无活动提示
        let t = mk(TaskState::Downloading, false);
        let row = progress_row(&t, 60, state_color(t.state), '⠋');
        assert!(!text_of(&row).contains("校验"));
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
        // 无倒计时 + 瞬态未达上限 → 不自动重试（auto_retry=false 口径）
        let mut x = mk(TaskState::Failed, false);
        x.retry_in = None;
        assert!(text_of(&stat_failed(&x)).contains("不自动重试"));
        // 语义性失败（Fatal）→ 不自动重试
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
        // 未探测（total=0）→ 未知
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
        // 暂停中的 BT：↓/↑ 与 - 占位并存
        let x = mk(TaskState::Paused, true);
        let s = text_of(&stat_bt_rest(&x));
        assert!(s.contains("↓"), "{s}");
        assert!(s.contains("-/-"));
        // 暂停中的 HTTP：↓ - 剩余 -
        let x = mk(TaskState::Paused, false);
        let s = text_of(&stat_http_rest(&x));
        assert!(s.contains("↓"), "{s}");
        assert!(s.contains("剩余"));
    }

    #[test]
    fn task_lines_width_floor_is_20() {
        let x = mk(TaskState::Downloading, false);
        let lines = task_lines(&x, false, '⠋', 1, 0);
        assert_eq!(lines.len(), 4);
        // 行内不 panic 即可（宽度下限收敛）
    }
}
