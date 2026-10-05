//! 加固测试（six-pack/hardender 步骤 7 批次 4）：app 层（mod/keys/tasks/paste/cli）存活体杀灭。
//! 布局：#[path] 挂载 app/mod.rs（其 mod 声明自动带 keys/tasks/cli/paste/dialogs/…
//! 子模块与各自带的产品单测）；App 字段为 pub，断言直接读状态。
//! 等价/环境型论证（不写测试）见 .work/tmp/step7/EQUIVALENCE.md：
//! sentinel.rs 9 点（进程级 TTY 语义）与 main.rs 58:5/68:5/339:5/347:16、overlay_sig 331:22。
// lint 姿态与产品 bin（src/main.rs crate 级 allow）对齐：#[path] 收编的产品源
// 在本测试 crate 内沿用产品面的豁免口径（产品面 clippy 0 的同一合同）。
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
// 测试挂载机制固有豁免：产品源经 #[path] 部分收编进独立测试 crate，部分产物项
// 在本 crate 上下文无消费者或被重复挂载（产品全量编译下均非死代码）。
#![allow(dead_code)]
#![allow(clippy::duplicate_mod)]
#![allow(unused_imports)]

#[path = "../src/app/mod.rs"]
mod app;
#[path = "../src/engine/mod.rs"]
mod engine;
#[path = "../src/model/mod.rs"]
mod model;

use app::{Dialog, DialogKind};
use crossterm::event::{KeyCode, KeyModifiers};
use model::config::Config;
use model::sidecar::{Sidecar, SidecarTask, SIDECAR_VERSION};
use model::{unix_now, Protocol, Task, TaskState};

type App = app::App;

/// 构建空 App（临时注册表；引擎随 App 启动，测试收尾 shutdown）
fn mkapp(tag: &str) -> App {
    let reg = std::env::temp_dir().join(format!("ezr-hard-app-{}-{tag}.json", std::process::id()));
    App::new(Config::default(), reg.to_string_lossy().into_owned())
}

/// 直接入列 n 个排队任务（id = 1..=n，name t{n}.bin）
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

// ---------------------------------------------------------------- keys.rs ---

/// 靶 keys 22:13/24:13/25:13/25:46/26:13/27:13/48:9/49:17：方向键导航全家桶
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

/// 靶 keys 29:25（`>→==`/`>→>=`）：空表按 End 不得把 selected 下溢成 -1
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

/// 靶 keys 35:13：空格在排队任务上触发暂停/继续
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

/// 靶 keys 36:13：R 在非失败任务上给出提示文案
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

/// 靶 keys 38:13：D 打开删除对话框
#[tokio::test]
async fn keys_d_opens_delete_dialog() {
    let mut a = mkapp("dlg-del");
    push_tasks(&mut a, 1);
    tap(&mut a, KeyCode::Char('d'));
    assert!(a.dialog.is_some(), "D 必须打开删除对话框");
    assert_eq!(a.dialog.as_ref().map(|d| d.kind), Some(DialogKind::Delete));
    shutdown(a).await;
}

/// 靶 keys 39:13：C 清理已完成任务
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

/// 靶 keys 42:13/41:71：U 上移任务（换位语义）
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

// ------------------------------------------------------------- app/mod.rs ---

/// 靶 mod 210:9：restore 必须从注册表恢复任务与 next_id
#[tokio::test]
async fn restore_keeps_tasks_and_next_id() {
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
    a1.shutdown().await;
    let a2 = App::new(Config::default(), reg.to_string_lossy().into_owned());
    assert_eq!(a2.tasks.len(), 2, "重启后任务必须恢复（恒空变异体不成立）");
    assert_eq!(a2.next_id, 3, "next_id 必须延续（恒 1 变异体不成立）");
    shutdown(a2).await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

/// 测试夹具：手写 Sidecar（字段全 pub；version 用产品常量）
fn fixture_sidecar(url: &str, save_dir: &str, non_resumable: bool) -> Sidecar {
    Sidecar {
        version: SIDECAR_VERSION,
        url: url.into(),
        final_url: None,
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

/// 靶 mod 219:31（delete `!`）：sidecar non_resumable=true → 恢复后不可续传
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

/// 靶 mod 228:9/228:39：spinner 按 frame 取 braille 表（% 语义）
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

/// 靶 mod 234:9：used_slots 统计占槽任务（Downloading + Queued&has_slot）
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

/// 靶 mod 240:9：queue_pos 1 基位次；未排队返回 None
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

/// 靶 mod 250:17：页签 1 = 已完成视图
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

/// 靶 mod 260:9：sel_idx 空表 None、跟随选中
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

/// 靶 mod 271:48：set_toast 的 toast_until = now + 3s（`+→-` 变异体立即过期）
#[tokio::test]
async fn set_toast_survives_immediate_tick() {
    let mut a = mkapp("toast");
    a.set_toast("加固测试");
    a.tick().await;
    assert!(a.toast.is_some(), "刚设置的 toast 不应在下一个 tick 过期");
    shutdown(a).await;
}

/// 靶 mod 280:38（`/→%`/`/→*`）：push_hist 按字节→KB 换算
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

// ------------------------------------------------------------- tasks.rs ---

/// 靶 tasks 166:24（`-→+`/`-→/`）：清理数量 toast 精确
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

/// 靶 tasks 167:14（`>→==`/`>→>=`/`>→<`）：无可清理时给提示而非「已清理 0 个」
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

/// 靶 tasks 82:13（v1.6/FR-01-92 行为变更登记）：Failed 任务按空格 = 暂停
/// 挂起（转「已暂停（失败）」、清倒计时）；再按空格 = 恢复重新排队（原
/// 「重新排队」口径由恢复动作承接，变异体仍落到兜底提示臂）
#[tokio::test]
async fn toggle_pause_on_failed_suspends_then_resume_requeues() {
    let mut a = mkapp("pause-failed");
    push_tasks(&mut a, 1);
    a.tasks[0].state = TaskState::Failed;
    a.tasks[0].retry_in = Some(8.0);
    a.selected = 0;
    tap(&mut a, KeyCode::Char(' '));
    assert_eq!(
        a.tasks[0].state,
        TaskState::FailedPaused,
        "Failed + 空格 → 已暂停（失败）（FR-01-92）"
    );
    assert!(a.tasks[0].retry_in.is_none(), "自动重试倒计时清除");
    assert!(!a.tasks[0].has_slot, "挂起不占槽位");
    // 再按空格 = 恢复 → 重新排队（原 requeue 口径）
    tap(&mut a, KeyCode::Char(' '));
    assert_eq!(
        a.tasks[0].state,
        TaskState::Queued,
        "恢复 + 空格 → 重新排队（delete-arm 变异体落到兜底提示）"
    );
    shutdown(a).await;
}

/// 靶 tasks 85:13：Completed 任务按空格给提示
#[tokio::test]
async fn toggle_pause_on_completed_hints() {
    let mut a = mkapp("pause-done");
    push_tasks(&mut a, 1);
    a.tasks[0].state = TaskState::Completed;
    a.filter = 1; // 已完成页签下任务才可见（sel_idx 走 filtered 视图）
    a.selected = 0;
    tap(&mut a, KeyCode::Char(' '));
    assert!(
        a.toast.as_deref().is_some_and(|t| t.contains("已完成")),
        "toast={:?}",
        a.toast
    );
    shutdown(a).await;
}

// ------------------------------------------------------------- paste.rs ---

fn add_dialog(focus: usize, ck_open: bool) -> Dialog {
    Dialog {
        kind: DialogKind::Add,
        url: String::new(),
        dir: String::new(),
        conns: String::new(),
        conns_edited: false,
        ck_type: 3,
        ck_value: String::new(),
        ck_open,
        ck_sel: 3,
        proxy_sel: 0,
        proxy_open: false,
        focus,
        task_name: String::new(),
        task_id: None,
    }
}

/// 靶 paste 9:9：on_paste 把文本路由进 Add 对话框 URL 字段
#[tokio::test]
async fn paste_routes_into_add_dialog_url() {
    let mut a = mkapp("paste-url");
    a.dialog = Some(add_dialog(0, false));
    a.on_paste("http://example.com/p.bin");
    assert_eq!(a.dialog.as_ref().unwrap().url, "http://example.com/p.bin");
    shutdown(a).await;
}

/// 靶 paste 12:12（delete `!`）：下拉框展开时粘贴必须被忽略
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

/// 靶 paste 46:9：focus=1 粘贴进目录字段（delete-arm 变异体落到 `_ => false`）
#[tokio::test]
async fn paste_routes_into_dir_field() {
    let mut a = mkapp("paste-dir");
    a.dialog = Some(add_dialog(1, false));
    a.on_paste("/tmp/ezr-dl");
    assert_eq!(a.dialog.as_ref().unwrap().dir, "/tmp/ezr-dl");
    assert!(a.dialog.as_ref().unwrap().url.is_empty());
    shutdown(a).await;
}

// -------------------------------------------------------------- cli.rs ---

/// 靶 cli 35:36（`==→!=`）：同 URL 有效 sidecar → 断点接续 toast + 沿用原名
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
