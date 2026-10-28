#![allow(missing_docs)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]
#![allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::too_many_arguments
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
#![allow(dead_code)]
#![allow(clippy::duplicate_mod)]
#![allow(unused_imports)]

#[path = "../src/app/mod.rs"]
mod app;
#[path = "../src/bt/mod.rs"]
mod bt;
#[path = "../src/engine/mod.rs"]
mod engine;
#[path = "common/fixture.rs"]
mod fixture;
#[path = "../src/model/mod.rs"]
mod model;
#[path = "../src/multiline.rs"]
mod multiline;
#[path = "../src/scrollbar.rs"]
mod scrollbar;
#[path = "../src/textfield.rs"]
mod textfield;

use app::{Dialog, DialogKind};
use crossterm::event::{KeyCode, KeyModifiers};
use model::config::Config;
use model::sidecar::{Sidecar, SidecarTask, SIDECAR_VERSION};
use model::{unix_now, Protocol, Task, TaskState};

type App = app::App;

fn mkapp(tag: &str) -> App {
    let reg = std::env::temp_dir().join(format!("ezr-hard-app-{}-{tag}.json", std::process::id()));
    App::new(Config::default(), reg.to_string_lossy().into_owned())
}

fn push_tasks(a: &mut App, n: u32) {
    for i in 1..=n {
        let t = Task::new_queued(
            i,
            format!("t{i}.bin"),
            Protocol::Http,
            format!("http://example.com/t{i}.bin"),
            "/tmp".into(),
            1 << 20,
            4,
            3,
            None,
            crate::model::config::ProxyChoice::Direct,
            unix_now(),
        );
        a.tasks.push(t);
    }
}

fn tap(a: &mut App, code: KeyCode) {
    a.on_key(code, KeyModifiers::empty());
}

async fn shutdown(mut a: App) {
    a.shutdown().await;
}

#[tokio::test]
async fn keys_navigation_moves_selection() {
    let mut a = mkapp("nav");
    push_tasks(&mut a, 3);
    tap(&mut a, KeyCode::Down);
    assert_eq!(
        a.selected, 1,
        "Down 应下移（delete-arm/no-op/== 变异体不动）"
    );
    tap(&mut a, KeyCode::PageUp);
    assert_eq!(
        a.selected, 0,
        "PageUp = -4：1-4 钳到 0（delete `-` 变异体 +4 → 2）"
    );
    tap(&mut a, KeyCode::PageDown);
    assert_eq!(a.selected, 2, "PageDown = +4：0+4 钳到 2");
    tap(&mut a, KeyCode::Home);
    assert_eq!(a.selected, 0, "Home 归零");
    tap(&mut a, KeyCode::End);
    assert_eq!(a.selected, 2, "End 到末尾");
    shutdown(a).await;
}

#[tokio::test]
async fn keys_end_on_empty_list_is_noop() {
    let mut a = mkapp("end-empty");
    tap(&mut a, KeyCode::End);
    assert_eq!(
        a.selected, 0,
        "空列表 End 必须空转（==/>= 变异体在此下溢 panic）"
    );
    shutdown(a).await;
}

#[tokio::test]
async fn keys_space_toggles_pause_on_queued() {
    let mut a = mkapp("space");
    push_tasks(&mut a, 1);
    let before = a.tasks[0].state;
    tap(&mut a, KeyCode::Char(' '));
    assert_ne!(
        a.tasks[0].state, before,
        "空格必须切换暂停态（delete-arm 变异体不动）"
    );
    assert!(
        a.toast.as_deref().is_some_and(|t| t.contains("暂停")),
        "toast={:?}",
        a.toast
    );
    shutdown(a).await;
}

#[tokio::test]
async fn keys_r_on_queued_toasts_retry_hint() {
    let mut a = mkapp("retry-hint");
    push_tasks(&mut a, 1);
    tap(&mut a, KeyCode::Char('r'));
    assert!(
        a.toast.as_deref().is_some_and(|t| t.contains("重试")),
        "R 必须提示重试语义，toast={:?}",
        a.toast
    );
    shutdown(a).await;
}

#[tokio::test]
async fn keys_d_opens_delete_dialog() {
    let mut a = mkapp("dlg-del");
    push_tasks(&mut a, 1);
    tap(&mut a, KeyCode::Char('d'));
    assert!(a.dialog.is_some(), "D 必须打开删除对话框");
    assert_eq!(a.dialog.as_ref().map(|d| d.kind), Some(DialogKind::Delete));
    shutdown(a).await;
}

#[tokio::test]
async fn keys_c_clears_completed() {
    let mut a = mkapp("clear");
    push_tasks(&mut a, 2);
    a.tasks[0].state = TaskState::Completed;
    tap(&mut a, KeyCode::Char('c'));
    assert_eq!(a.tasks.len(), 1, "C 必须清掉已完成任务");
    assert_ne!(a.tasks[0].state, TaskState::Completed);
    shutdown(a).await;
}

#[tokio::test]
async fn keys_u_moves_task_up() {
    let mut a = mkapp("move-up");
    push_tasks(&mut a, 3);
    a.selected = 1;
    tap(&mut a, KeyCode::Char('u'));
    assert_eq!(
        a.tasks[0].id, 2,
        "U 应把 1 号位任务与 0 号位换位（delete-arm 与 `-` 删除变异体均不成立）"
    );
    shutdown(a).await;
}

#[tokio::test]
async fn restore_keeps_tasks_and_ids() {
    let reg = std::env::temp_dir().join(format!("ezr-hard-restore-{}.json", std::process::id()));
    let dir = std::env::temp_dir().join(format!("ezr-hard-restore-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut a1 = App::new(Config::default(), reg.to_string_lossy().into_owned());
    a1.add_cli_task(
        "http://example.com/restore-a.bin".into(),
        Some(dir.to_string_lossy().into_owned()),
        None,
        None,
    );
    a1.add_cli_task(
        "http://example.com/restore-b.bin".into(),
        Some(dir.to_string_lossy().into_owned()),
        None,
        None,
    );
    let ids_before: Vec<u32> = a1.tasks.iter().map(|t| t.id).collect();
    assert_eq!(ids_before.len(), 2);
    a1.shutdown().await;
    let a2 = App::new(Config::default(), reg.to_string_lossy().into_owned());
    assert_eq!(a2.tasks.len(), 2, "重启后任务必须恢复（恒空变异体不成立）");
    let ids_after: Vec<u32> = a2.tasks.iter().map(|t| t.id).collect();
    assert_eq!(
        ids_after, ids_before,
        "任务 ID 必须跨会话保留（恒重派变异体不成立）"
    );
    shutdown(a2).await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

fn fixture_sidecar(_url: &str, save_dir: &str, non_resumable: bool) -> Sidecar {
    Sidecar {
        version: SIDECAR_VERSION,
        size: 1000,
        etag: None,
        last_modified: None,
        block_size: 512,
        block_count: 2,
        blocks: vec![512, 488],
        downloaded: 100,
        non_resumable,
        expected: None,
        task: SidecarTask {
            id: 1,
            added_at: 0,
            save_dir: save_dir.into(),
            concurrency: 4,
            protocol: Protocol::Http,
        },
    }
}

#[tokio::test]
async fn restore_merges_nonresumable_sidecar() {
    let reg = std::env::temp_dir().join(format!("ezr-hard-scr-{}.json", std::process::id()));
    let dir = std::env::temp_dir().join(format!("ezr-hard-scr-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut a1 = App::new(Config::default(), reg.to_string_lossy().into_owned());
    a1.add_cli_task(
        "http://example.com/scr.bin".into(),
        Some(dir.to_string_lossy().into_owned()),
        None,
        None,
    );
    a1.shutdown().await;
    let name = a1.tasks[0].name.clone();
    let sc_path = dir.join(format!("{name}.ezr"));
    fixture_sidecar("http://example.com/scr.bin", &dir.to_string_lossy(), true)
        .save(&sc_path.to_string_lossy())
        .unwrap();
    let a2 = App::new(Config::default(), reg.to_string_lossy().into_owned());
    assert!(
        !a2.tasks[0].resumable,
        "non_resumable=true 必须令 resumable=false（delete ! 变异体给出 true）"
    );
    assert_eq!(a2.tasks[0].downloaded, 100, "断点字节必须合并");
    shutdown(a2).await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn spinner_cycles_braille_table() {
    let mut a = mkapp("spinner");
    a.frame = 0;
    assert_eq!(a.spinner(), '⠋');
    a.frame = 10;
    assert_eq!(
        a.spinner(),
        '⠋',
        "frame 10 % 10 = 0（`%→/` 变异体给出表[1]）"
    );
    a.frame = 3;
    assert_eq!(a.spinner(), '⠸');
    shutdown(a).await;
}

#[tokio::test]
async fn used_slots_counts_active_tasks() {
    let mut a = mkapp("slots");
    push_tasks(&mut a, 3);
    a.tasks[0].state = TaskState::Downloading;
    a.tasks[1].has_slot = true;
    a.tasks[2].state = TaskState::Completed;
    assert_eq!(
        a.used_slots(),
        2,
        "1 下载中 + 1 排队占槽 = 2（恒 0/恒 1 变异体不成立）"
    );
    shutdown(a).await;
}

#[tokio::test]
async fn queue_pos_orders_waiting_tasks() {
    let mut a = mkapp("qpos");
    push_tasks(&mut a, 3);
    a.tasks[0].state = TaskState::Downloading;
    assert_eq!(
        a.queue_pos_of(2),
        Some(1),
        "首个等待任务位次 1（None/Some(0) 变异体不成立）"
    );
    assert_eq!(
        a.queue_pos_of(3),
        Some(2),
        "第二位 2（Some(1) 变异体不成立）"
    );
    assert_eq!(a.queue_pos_of(1), None, "下载中不排队");
    shutdown(a).await;
}

#[tokio::test]
async fn filtered_tab1_shows_completed_only() {
    let mut a = mkapp("filter");
    push_tasks(&mut a, 2);
    a.tasks[1].state = TaskState::Completed;
    a.filter = 1;
    assert_eq!(
        a.filtered(),
        vec![1],
        "页签 1 只含已完成（delete-arm 变异体回退到非完成）"
    );
    a.filter = 0;
    assert_eq!(a.filtered(), vec![0]);
    shutdown(a).await;
}

#[tokio::test]
async fn sel_idx_follows_selection() {
    let a = mkapp("sel-empty");
    assert_eq!(a.sel_idx(), None, "空表必须 None（Some(0) 变异体不成立）");
    shutdown(a).await;
    let mut b = mkapp("sel-follow");
    push_tasks(&mut b, 3);
    b.selected = 2;
    assert_eq!(b.sel_idx(), Some(2));
    shutdown(b).await;
}

#[tokio::test]
async fn set_toast_survives_immediate_tick() {
    let mut a = mkapp("toast");
    a.set_toast("加固测试");
    a.tick().await;
    assert!(a.toast.is_some(), "刚设置的 toast 不应在下一个 tick 过期");
    shutdown(a).await;
}

#[tokio::test]
async fn push_hist_scales_bytes_to_kb() {
    let mut a = mkapp("hist");
    a.push_hist(2048.0, 1024.0);
    assert_eq!(
        *a.speed_hist.last().unwrap(),
        2,
        "2048 B/s = 2 KB/s（% 变异体 0）"
    );
    assert_eq!(
        *a.up_hist.last().unwrap(),
        1,
        "1024 B/s = 1 KB/s（* 变异体饱和）"
    );
    shutdown(a).await;
}

#[tokio::test]
async fn clear_completed_toasts_exact_count() {
    let mut a = mkapp("clear-count");
    push_tasks(&mut a, 3);
    a.tasks[0].state = TaskState::Completed;
    a.tasks[2].state = TaskState::Completed;
    a.clear_completed();
    assert!(
        a.toast.as_deref().is_some_and(|t| t.contains("已清理 2")),
        "清理 2 个，toast={:?}（`+`→5、`/`→3 变异体不成立）",
        a.toast
    );
    assert_eq!(a.tasks.len(), 1);
    shutdown(a).await;
}

#[tokio::test]
async fn clear_completed_none_toasts_hint() {
    let mut a = mkapp("clear-none");
    push_tasks(&mut a, 1);
    a.clear_completed();
    assert!(
        a.toast.as_deref().is_some_and(|t| t.contains("没有可清理")),
        "toast={:?}",
        a.toast
    );
    shutdown(a).await;
    let mut b = mkapp("clear-lt");
    push_tasks(&mut b, 2);
    b.tasks[0].state = TaskState::Completed;
    b.clear_completed();
    assert!(
        b.toast.as_deref().is_some_and(|t| t.contains("已清理 1")),
        "`>→<` 变异体在此给出「没有可清理」，toast={:?}",
        b.toast
    );
    shutdown(b).await;
}

#[tokio::test]
async fn toggle_pause_on_failed_suspends_then_resume_requeues() {
    let mut a = mkapp("pause-failed");
    push_tasks(&mut a, 1);
    a.tasks[0].state = TaskState::Failed;
    a.tasks[0].retry_in = Some(8.0);
    a.tasks[0].retries = 3;
    a.tasks[0].error = Some("连接被重置".to_string());
    a.selected = 0;
    tap(&mut a, KeyCode::Char(' '));
    assert_eq!(
        a.tasks[0].state,
        TaskState::Paused,
        "Failed + 空格 → 已暂停（v1.21 合并口径，FR-01-92）"
    );
    assert!(a.tasks[0].retry_in.is_none(), "自动重试倒计时清除");
    assert!(!a.tasks[0].has_slot, "挂起不占槽位");
    assert!(
        a.tasks[0].error.is_none(),
        "挂起即重置错误信息（v1.23：error=null）"
    );
    assert_eq!(
        a.tasks[0].retries, 1,
        "挂起即重置重试计数（v1.23：retries=1）"
    );
    tap(&mut a, KeyCode::Char(' '));
    assert_eq!(
        a.tasks[0].state,
        TaskState::Queued,
        "恢复 + 空格 → 重新排队（delete-arm 变异体落到兜底提示）"
    );
    shutdown(a).await;
}

#[tokio::test]
async fn toggle_pause_on_completed_hints() {
    let mut a = mkapp("pause-done");
    push_tasks(&mut a, 1);
    a.tasks[0].state = TaskState::Completed;
    a.filter = 1;
    a.selected = 0;
    tap(&mut a, KeyCode::Char(' '));
    assert!(
        a.toast.as_deref().is_some_and(|t| t.contains("已完成")),
        "toast={:?}",
        a.toast
    );
    shutdown(a).await;
}

#[tokio::test]
async fn toggle_pause_resume_missing_named_proxy_warns() {
    let mut a = mkapp("pause-gone-proxy");
    push_tasks(&mut a, 1);
    a.tasks[0].state = TaskState::Paused;
    a.tasks[0].proxy = crate::model::config::ProxyChoice::Named("gone-proxy".into());
    a.selected = 0;
    tap(&mut a, KeyCode::Char(' '));
    assert_eq!(
        a.tasks[0].state,
        TaskState::Queued,
        "恢复 = 重新排队（v1.21 统一路径）"
    );
    let toast = a.toast.as_deref().unwrap_or_default();
    assert!(
        toast.contains("已进入等待队列"),
        "恢复提示（普通暂停走续传双臂 + 排队口径）: {toast}"
    );
    assert!(
        !toast.contains("不存在"),
        "按键时刻不触发代理解析（提醒移至获槽启动时）: {toast}"
    );
    shutdown(a).await;
}

fn add_dialog(focus: usize, ck_open: bool) -> Dialog {
    Dialog {
        kind: DialogKind::Add,
        url: textfield::TextInput::new(),
        dir: textfield::TextInput::new(),
        conns: textfield::TextInput::new(),
        conns_edited: false,
        ck_type: 3,
        ck_value: textfield::TextInput::new(),
        ck_open,
        ck_sel: 3,
        proxy_sel: 0,
        proxy_open: false,
        focus,
        task_name: String::new(),
        task_id: None,
        torrent: textfield::TextInput::new(),
        bt_tree: Vec::new(),
        bt_cursor: 0,
        bt_scroll: 0,
        bt_follow: true,
        trackers: multiline::MultiLineBuf::new(),
    }
}

#[tokio::test]
async fn paste_routes_into_add_dialog_url() {
    let mut a = mkapp("paste-url");
    a.dialog = Some(add_dialog(0, false));
    a.on_paste("http://example.com/p.bin");
    assert_eq!(
        a.dialog.as_ref().unwrap().url.value,
        "http://example.com/p.bin"
    );
    shutdown(a).await;
}

#[tokio::test]
async fn paste_ignored_when_dropdown_open() {
    let mut a = mkapp("paste-open");
    a.dialog = Some(add_dialog(0, true));
    a.on_paste("http://example.com/p.bin");
    assert!(
        a.dialog.as_ref().unwrap().url.is_empty(),
        "下拉展开时粘贴不得写入（delete ! 变异体会写入）"
    );
    shutdown(a).await;
}

#[tokio::test]
async fn paste_routes_into_dir_field() {
    let mut a = mkapp("paste-dir");
    a.dialog = Some(add_dialog(1, false));
    a.on_paste("/tmp/ezr-dl");
    assert_eq!(a.dialog.as_ref().unwrap().dir.value, "/tmp/ezr-dl");
    assert!(a.dialog.as_ref().unwrap().url.is_empty());
    shutdown(a).await;
}

#[tokio::test]
async fn add_cli_task_resumes_with_matching_sidecar() {
    let dir = std::env::temp_dir().join(format!("ezr-hard-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let url = "http://example.com/resume.bin";
    fixture_sidecar(url, &dir.to_string_lossy(), false)
        .save(&dir.join("resume.bin.ezr").to_string_lossy())
        .unwrap();
    let mut a = mkapp("cli-resume");
    a.add_cli_task(
        url.into(),
        Some(dir.to_string_lossy().into_owned()),
        None,
        None,
    );
    assert!(
        a.toast.as_deref().is_some_and(|t| t.contains("断点")),
        "同 URL sidecar 必须判为接续，toast={:?}（==→!= 变异体判非接续）",
        a.toast
    );
    assert_eq!(a.tasks[0].name, "resume.bin", "接续沿用原名");
    shutdown(a).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn retry_verify_failed_cleared_checksum_requeues_without_verify() {
    let dir = crate::model::testenv::uniq_tmp_dir("ezr-retry-cleared-ck");
    let mut a = mkapp("retry-cleared-ck");
    push_tasks(&mut a, 1);
    a.tasks[0].state = TaskState::Failed;
    a.tasks[0].fail_kind = Some(model::FailKind::Verify);
    a.tasks[0].checksum = None;
    a.tasks[0].save_dir = dir.to_string_lossy().into_owned();
    a.selected = 0;
    a.retry();
    assert_eq!(
        a.tasks[0].state,
        TaskState::Queued,
        "清码 + 无伴随 → 正常重新排队（不进「校验中」，FR-01-51 v1.17③）"
    );
    assert!(
        a.tasks[0].checksum.is_none(),
        "不复活期望值（无伴随可解析）"
    );
    assert!(a.tasks[0].fail_kind.is_none(), "失败类别随重排清除");
    assert!(!a.tasks[0].has_slot, "等待槽位，不占槽");
    shutdown(a).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn retry_verify_failed_cleared_checksum_completes_directly_when_file_intact() {
    let dir = crate::model::testenv::uniq_tmp_dir("ezr-retry-direct-hit");
    let mut a = mkapp("retry-direct-hit");
    push_tasks(&mut a, 1);
    a.tasks[0].state = TaskState::Failed;
    a.tasks[0].fail_kind = Some(model::FailKind::Verify);
    a.tasks[0].checksum = None;
    a.tasks[0].total = 1024;
    a.tasks[0].downloaded = 1024;
    a.tasks[0].save_dir = dir.to_string_lossy().into_owned();
    std::fs::write(dir.join("t1.bin.downloading"), vec![0u8; 1024]).unwrap();
    std::fs::write(dir.join("t1.bin.ezr"), b"sidecar").unwrap();
    a.selected = 0;
    a.retry();
    assert_eq!(
        a.tasks[0].state,
        TaskState::Completed,
        "文件完整必须不经引擎直接收尾（修订 B 直判臂）"
    );
    assert!(a.tasks[0].verify_ok.is_none(), "无校验完成 verify_ok=None");
    assert_eq!(a.tasks[0].speed, 0.0, "完成态速度归零");
    assert!(a.tasks[0].connections.is_empty(), "完成态清连接视图");
    assert!(
        dir.join("t1.bin").exists(),
        "目标文件在（`.downloading` 已改名）"
    );
    assert!(
        !dir.join("t1.bin.downloading").exists(),
        ".downloading 已改名消失"
    );
    assert!(!dir.join("t1.bin.ezr").exists(), "sidecar 已删");
    assert!(
        a.toast.as_deref().is_some_and(|t| t.contains("无校验")),
        "toast 必须含「无校验」（got {:?}）",
        a.toast
    );
    shutdown(a).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn retry_verify_failed_cleared_checksum_discards_checkpoint_when_file_short() {
    let dir = crate::model::testenv::uniq_tmp_dir("ezr-retry-discards-ck");
    let mut a = mkapp("retry-discards-ck");
    push_tasks(&mut a, 1);
    a.tasks[0].state = TaskState::Failed;
    a.tasks[0].fail_kind = Some(model::FailKind::Verify);
    a.tasks[0].checksum = None;
    a.tasks[0].total = 1024;
    a.tasks[0].downloaded = 1024;
    a.tasks[0].chunk_done = 1;
    a.tasks[0].save_dir = dir.to_string_lossy().into_owned();
    std::fs::write(dir.join("t1.bin.downloading"), vec![0u8; 100]).unwrap();
    std::fs::write(dir.join("t1.bin.ezr"), b"sidecar").unwrap();
    a.selected = 0;
    a.retry();
    assert_eq!(
        a.tasks[0].state,
        TaskState::Queued,
        "大小不符 → 作废断点从头重下（保持等待中）"
    );
    assert_eq!(a.tasks[0].downloaded, 0, "进度视图清零");
    assert_eq!(a.tasks[0].chunk_done, 0, "块视图清零");
    assert!(!a.tasks[0].has_slot, "等待槽位，不占槽");
    assert!(!dir.join("t1.bin.ezr").exists(), "sidecar 已作废");
    assert!(
        a.toast.as_deref().is_some_and(|t| t.contains("断点已作废")),
        "toast 必须含「断点已作废」（got {:?}）",
        a.toast
    );
    shutdown(a).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn retry_verify_failed_with_companion_reverifies_with_companion_value() {
    let dir = crate::model::testenv::uniq_tmp_dir("ezr-retry-companion");
    std::fs::write(
        dir.join("t1.bin.sha256"),
        "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
    )
    .unwrap();
    let mut a = mkapp("retry-companion");
    push_tasks(&mut a, 1);
    a.tasks[0].state = TaskState::Failed;
    a.tasks[0].fail_kind = Some(model::FailKind::Verify);
    a.tasks[0].save_dir = dir.to_string_lossy().into_owned();
    a.selected = 0;
    a.retry();
    assert_eq!(
        a.tasks[0].state,
        TaskState::Verifying,
        "伴随在 → 重校验（D10① 语义不回归）"
    );
    let ck = a.tasks[0].checksum.as_ref().expect("伴随现值已回填");
    assert_eq!(
        ck.value, "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
        "期望值 = 伴随现值（伴随在时无视任务显式旧值，D10 重查语义）"
    );
    assert_eq!(ck.algo, "SHA-256", "算法按伴随后缀解析");
    assert!(a.tasks[0].has_slot, "校验中占槽位（D12）");
    shutdown(a).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn retry_verify_failed_with_explicit_checksum_reverifies() {
    let dir = crate::model::testenv::uniq_tmp_dir("ezr-retry-explicit-ck");
    let mut a = mkapp("retry-explicit-ck");
    push_tasks(&mut a, 1);
    a.tasks[0].state = TaskState::Failed;
    a.tasks[0].fail_kind = Some(model::FailKind::Verify);
    a.tasks[0].save_dir = dir.to_string_lossy().into_owned();
    a.tasks[0].checksum = Some(model::Checksum {
        algo: "SHA-256",
        value: "aa".repeat(32),
    });
    a.selected = 0;
    a.retry();
    assert_eq!(
        a.tasks[0].state,
        TaskState::Verifying,
        "显式值在 → 重校验（D10② 不回归）"
    );
    let ck = a.tasks[0].checksum.as_ref().expect("显式值保留");
    assert_eq!(
        ck.value,
        "aa".repeat(32),
        "期望值 = 任务显式值（D3 显式优先）"
    );
    shutdown(a).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn direct_finalize_total_zero_empty_file_not_intact() {
    let dir = crate::model::testenv::uniq_tmp_dir("ezr-hard-df-total0");
    let mut a = mkapp("df-total0");
    push_tasks(&mut a, 1);
    a.tasks[0].state = TaskState::Failed;
    a.tasks[0].fail_kind = Some(model::FailKind::Verify);
    a.tasks[0].save_dir = dir.to_string_lossy().into_owned();
    a.tasks[0].name = "df0.bin".into();
    a.tasks[0].total = 0;
    a.tasks[0].downloaded = 0;
    a.tasks[0].checksum = None;
    let dl_path = format!("{}/df0.bin.downloading", dir.to_string_lossy());
    std::fs::write(&dl_path, b"").expect("写入 len==0 的 .downloading 文件");
    a.selected = 0;
    a.retry();
    assert_eq!(
        a.tasks[0].state,
        TaskState::Queued,
        "total=0 时空文件不构成直判依据：保持等待中（作废断点从头重下）"
    );
    let fin = format!("{}/df0.bin", dir.to_string_lossy());
    assert!(
        !std::path::Path::new(&fin).exists(),
        "未发生「文件已完整」改名收尾"
    );
    shutdown(a).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn clear_completed_spares_live_task_conn_ledger() {
    let root = crate::model::testenv::uniq_tmp_dir("ezr-hard-cc-root");
    let save = crate::model::testenv::uniq_tmp_dir("ezr-hard-cc-save");
    std::fs::write(root.join("slow.bin"), vec![0u8; 200_000]).expect("写 fixture 文件");
    let (_fx, port) = fixture::spawn_serve(&root);

    let mut a = mkapp("clear-completed-ledger");
    let mut done = crate::model::sample_task();
    done.id = 1;
    done.name = "done.bin".into();
    done.state = TaskState::Completed;
    let slow = Task::new_queued(
        2,
        "slow.bin".to_string(),
        Protocol::Http,
        format!("http://127.0.0.1:{port}/slow.bin?speed=20000"),
        save.to_string_lossy().into_owned(),
        1 << 16,
        1,
        3,
        None,
        crate::model::config::ProxyChoice::Direct,
        unix_now(),
    );
    a.tasks.push(done);
    a.tasks.push(slow);
    a.selected = 0;

    let mut populated = false;
    for _ in 0..100 {
        a.tick().await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if a.conn_cum_of(2, 1) > 0 {
            populated = true;
            break;
        }
        assert_ne!(
            a.tasks[1].state,
            TaskState::Failed,
            "下载不应失败（fixture 可用）"
        );
    }
    assert!(populated, "连接级累计未填充（fixture/引擎异常）");
    assert_eq!(
        a.tasks[1].state,
        TaskState::Downloading,
        "慢速门控下仍在下载"
    );

    a.clear_completed();
    assert_eq!(a.tasks.len(), 1, "仅已完成任务被移出列表");
    assert_eq!(a.tasks[0].id, 2, "未完成任务保留");
    assert!(
        a.conn_cum_of(2, 1) > 0,
        "清理不得波及未完成任务的连接级累计账本"
    );
    shutdown(a).await;
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&save);
}
