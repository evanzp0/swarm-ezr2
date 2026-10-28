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

use app::App;
use crossterm::event::{KeyCode, KeyModifiers};
use model::config::Config;
use model::sidecar::{Sidecar, SidecarTask, SIDECAR_VERSION};
use model::{unix_now, Checksum, FailKind, Protocol, Task, TaskState};

fn mkapp(tag: &str) -> App {
    let reg = std::env::temp_dir().join(format!("ezr-hard-eng-{}-{tag}.json", std::process::id()));
    App::new(Config::default(), reg.to_string_lossy().into_owned())
}

fn queued(id: u32, name: &str, dir: &str) -> Task {
    Task::new_queued(
        id,
        name.into(),
        Protocol::Http,
        format!("http://example.com/{name}"),
        dir.into(),
        1 << 20,
        2,
        3,
        None,
        crate::model::config::ProxyChoice::Direct,
        unix_now(),
    )
}

async fn pump(app: &mut App, secs: u64, done: impl Fn(&App) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        app.tick().await;
        if done(app) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

fn stub_with(extra_head: &str, len: u64, trickle: bool) -> std::net::SocketAddr {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let data: Vec<u8> = (0..len as u32).map(|i| (i % 251) as u8).collect();
    let head_extra = extra_head.to_string();
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let data = data.clone();
            let head_extra = head_extra.clone();
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader, Write};
                let mut c = conn;
                let mut r = BufReader::new(c.try_clone().unwrap());
                let mut line = String::new();
                while r.read_line(&mut line).unwrap_or(0) > 0 {
                    if line == "\r\n" {
                        break;
                    }
                    line.clear();
                }
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n{head_extra}Connection: close\r\n\r\n",
                    data.len()
                );
                let _ = c.write_all(head.as_bytes());
                if trickle {
                    let _ = c.write_all(&data[..data.len().min(3000)]);
                    let _ = c.flush();
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    return;
                }
                let _ = c.write_all(&data);
                let _ = c.flush();
            });
        }
    });
    addr
}

fn stub_trickle() -> std::net::SocketAddr {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader, Write};
                let mut c = conn;
                let mut r = BufReader::new(c.try_clone().unwrap());
                let mut line = String::new();
                while r.read_line(&mut line).unwrap_or(0) > 0 {
                    if line == "\r\n" {
                        break;
                    }
                    line.clear();
                }
                let _ = c.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n");
                let _ = c.flush();
                for _ in 0..100 {
                    let _ = c.write_all(&[9u8; 200]);
                    let _ = c.flush();
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            });
        }
    });
    addr
}
fn data_pattern(len: u64) -> Vec<u8> {
    (0..len as u32).map(|i| (i % 251) as u8).collect()
}

#[tokio::test]
async fn disk_precheck_fails_when_insufficient() {
    let mut a = mkapp("disk1");
    let dir = std::env::temp_dir().join(format!("ezr-hard-disk1-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut t = queued(1, "disk.bin", &dir.to_string_lossy());
    t.probed = true;
    t.total = u64::MAX / 2;
    a.tasks.push(t);
    a.tick().await;
    assert_eq!(
        a.tasks[0].state,
        TaskState::Failed,
        "空间不足必须 Failed（恒 true / skip / `>`、`==` 变异体放行）"
    );
    assert!(a.tasks[0]
        .error
        .as_deref()
        .is_some_and(|e| e.contains("磁盘空间不足")));
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn disk_precheck_skips_unprobed() {
    let mut a = mkapp("disk2");
    let mut t = queued(1, "disk2.bin", "/tmp");
    t.probed = false;
    t.total = u64::MAX / 2;
    a.tasks.push(t);
    a.tick().await;
    assert_ne!(
        a.tasks[0].error.as_deref(),
        Some("磁盘空间不足（保存目录剩余空间小于待下载量）"),
        "未探测任务不得触发磁盘预检失败（&& 变异体在 need!=0 && !probed 时误判）"
    );
    a.shutdown().await;
}

#[tokio::test]
async fn retry_countdown_expires_to_queued() {
    let mut a = mkapp("countdown");
    let mut t = queued(1, "cd.bin", "/tmp");
    t.state = TaskState::Failed;
    t.fail_kind = Some(FailKind::Transient);
    t.retry_in = Some(0.05);
    t.has_slot = true;
    a.tasks.push(t);
    pump(&mut a, 5, |x| x.tasks[0].state == TaskState::Queued).await;
    assert_eq!(
        a.tasks[0].state,
        TaskState::Queued,
        "到点必须转 Queued（`>` 变异体永不到期）"
    );
    a.shutdown().await;
}

#[tokio::test]
async fn retry_start_clears_only_expired_target() {
    let mut a = mkapp("retry-target");
    let dir = "/tmp";
    let mut t1 = queued(1, "a.bin", dir);
    t1.state = TaskState::Failed;
    t1.fail_kind = Some(FailKind::Transient);
    t1.retry_in = Some(0.05);
    t1.has_slot = true;
    t1.made_progress = true;
    let mut t2 = queued(2, "b.bin", dir);
    t2.state = TaskState::Paused;
    t2.made_progress = true;
    a.tasks.push(t1);
    a.tasks.push(t2);
    pump(&mut a, 5, |x| !x.tasks[0].made_progress).await;
    assert!(
        !a.tasks[0].made_progress,
        "目标任务标记必须被清（`==→!=` 变异体清错对象）"
    );
    assert!(a.tasks[1].made_progress, "非目标任务不得被动");
    a.shutdown().await;
}

#[tokio::test]
async fn retry_filter_requires_all_conditions() {
    let mut a = mkapp("retry-filter");
    a.max_slots = 0;
    let dir = "/tmp";
    let mut t1 = queued(1, "f1.bin", dir);
    t1.state = TaskState::Paused;
    t1.has_slot = true;
    t1.probed = true;
    let mut t2 = queued(2, "f2.bin", dir);
    t2.probed = true;
    let mut t3 = queued(3, "f3.bin", dir);
    t3.has_slot = true;
    for t in [&mut t1, &mut t2, &mut t3] {
        t.made_progress = true;
    }
    a.tasks.push(t1);
    a.tasks.push(t2);
    a.tasks.push(t3);
    a.tick().await;
    for (i, why) in [(0usize, "state"), (1usize, "has_slot"), (2usize, "probed")] {
        assert!(
            a.tasks[i].made_progress,
            "任务{}（不满足 {why}）不得被补发重试（|| 变异体误纳入）",
            i + 1
        );
    }
    a.shutdown().await;
}

#[tokio::test]
async fn non_downloading_speed_zeroed() {
    let mut a = mkapp("speed-zero");
    let mut t = queued(1, "s.bin", "/tmp");
    t.state = TaskState::Paused;
    t.speed = 5.0;
    a.tasks.push(t);
    a.tick().await;
    assert_eq!(
        a.tasks[0].speed, 0.0,
        "非下载态速度必须归零（== 变异体只清下载态）"
    );
    a.shutdown().await;
}

#[tokio::test]
async fn speed_hist_samples_after_interval() {
    let mut a = mkapp("hist-tick");
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    a.tick().await;
    assert_eq!(
        a.speed_hist.len(),
        91,
        "初始 90 点 + 1 次采样（`<` 变异体永不采样）"
    );
    a.shutdown().await;
}

#[tokio::test]
async fn registry_periodic_save_not_immediate() {
    let mut a = mkapp("save-tick");
    let path = std::env::temp_dir().join(format!(
        "ezr-hard-eng-{}-save-tick.json",
        std::process::id()
    ));
    a.tick().await;
    assert!(
        !path.exists(),
        "5s 兜底未到不得写注册表（`<` 变异体首帧即写）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn session_bytes_zero_speed_noop() {
    let mut a = mkapp("sb-zero");
    let mut t = queued(1, "z.bin", "/tmp");
    t.state = TaskState::Downloading;
    t.speed = 0.0;
    a.tasks.push(t);
    a.session_bytes = 100;
    a.tick().await;
    assert_eq!(a.session_bytes, 100, "零速不得改变累计（*= 变异体清零）");
    a.shutdown().await;
}

#[tokio::test]
async fn session_bytes_accumulates() {
    let mut a = mkapp("sb-acc");
    let mut t = queued(1, "a2.bin", "/tmp");
    t.state = TaskState::Downloading;
    t.downloaded = 400;
    a.tasks.push(t);
    a.session_bytes = 100;
    a.tick().await;
    assert_eq!(
        a.session_bytes, 100,
        "首次观察仅建基线，不得计入既有（sidecar 恢复）进度（+=→*= 变异体放大）"
    );
    if let Some(t) = a.tasks.get_mut(0) {
        t.downloaded = 1500;
    }
    a.tick().await;
    assert_eq!(
        a.session_bytes, 1200,
        "累计必须按真实字节增量累加（-= 变异体递减/下溢、*→+ 变异体越界）"
    );
    a.shutdown().await;
}

#[tokio::test]
async fn tick_clamps_selection() {
    let mut a = mkapp("sel-clamp");
    for i in 1..=2 {
        a.tasks.push(queued(i, &format!("c{i}.bin"), "/tmp"));
    }
    a.selected = 5;
    a.tick().await;
    assert_eq!(
        a.selected, 1,
        "selected 必须钳到 flen-1（`<` 变异体不钳、`+`/`/` 变异体越界）"
    );
    a.shutdown().await;
}

#[tokio::test]
async fn tick_clamps_scroll_lower_bound() {
    let mut a = mkapp("scroll-lo");
    for i in 1..=20 {
        a.tasks.push(queued(i, &format!("s{i}.bin"), "/tmp"));
    }
    a.visible_rows = 6;
    a.selected = 1;
    a.scroll = 3;
    a.tick().await;
    assert_eq!(
        a.scroll, 1,
        "scroll 须收敛到 selected（`<`/`==` 变异体不动）"
    );
    a.shutdown().await;
}

#[tokio::test]
async fn tick_clamps_scroll_upper_bound() {
    let mut a = mkapp("scroll-hi");
    for i in 1..=20 {
        a.tasks.push(queued(i, &format!("u{i}.bin"), "/tmp"));
    }
    a.visible_rows = 6;
    a.selected = 8;
    a.scroll = 2;
    a.tick().await;
    assert_eq!(
        a.scroll, 3,
        "selected 8 出窗（≥ 2+6）→ scroll = 8+1-6 = 3（`+`/`-`/`*` 变异体不成立）"
    );
    a.shutdown().await;
}

#[tokio::test]
async fn probed_renames_and_transitions() {
    let addr = stub_with(
        "Content-Disposition: attachment; filename=\"b.bin\"\r\n",
        8_000_000,
        true,
    );
    let mut a = mkapp("probed-basic");
    let dir = std::env::temp_dir().join(format!("ezr-hard-pb-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(2),
        None,
    );
    pump(&mut a, 20, |x| x.tasks[0].probed).await;
    assert!(a.tasks[0].probed, "必须完成探测");
    assert!(
        a.tasks[0].name.starts_with("b.bin"),
        "CD 名必须回写（find/守卫/`&&→||`/apply 变异体保持 f.bin）: {}",
        a.tasks[0].name
    );
    assert_eq!(
        a.tasks[0].state,
        TaskState::Downloading,
        "探测后必须转下载中（`==→!=` 变异体不转）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn probed_no_rename_when_downloaded() {
    let addr = stub_with(
        "Content-Disposition: attachment; filename=\"b.bin\"\r\n",
        64_000,
        false,
    );
    let mut a = mkapp("probed-keep");
    let dir = std::env::temp_dir().join(format!("ezr-hard-pk-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(2),
        None,
    );
    a.tasks[0].downloaded = 100;
    pump(&mut a, 20, |x| x.tasks[0].probed).await;
    assert!(a.tasks[0].probed);
    assert_eq!(
        a.tasks[0].name, "f.bin",
        "downloaded>0 时必须保留原名（guard→true / || 变异体会改名）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn probed_rename_dedupes_task_conflict() {
    let addr = stub_with(
        "Content-Disposition: attachment; filename=\"b.bin\"\r\n",
        64_000,
        false,
    );
    let mut a = mkapp("probed-conflict");
    let dir = std::env::temp_dir().join(format!("ezr-hard-pc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(2),
        None,
    );
    let mut t2 = queued(2, "b.bin", &dir.to_string_lossy());
    t2.state = TaskState::Paused;
    a.tasks.push(t2);
    pump(&mut a, 20, |x| x.tasks[0].probed).await;
    assert!(
        a.tasks[0].name.starts_with("b.bin.1"),
        "任务表已占用 b.bin → 去重为 b.bin.1 系（taken 闭包比较符变异体保持 b.bin/f.bin）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn probed_rename_dedupes_disk_conflict() {
    let addr = stub_with(
        "Content-Disposition: attachment; filename=\"b.bin\"\r\n",
        64_000,
        false,
    );
    let mut a = mkapp("probed-disk");
    let dir = std::env::temp_dir().join(format!("ezr-hard-pd-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("b.bin"), b"occupied").unwrap();
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(2),
        None,
    );
    pump(&mut a, 20, |x| x.tasks[0].probed).await;
    assert!(
        a.tasks[0].name.starts_with("b.bin.1"),
        "盘上占用必须去重（`||→&&` 变异体漏检盘上冲突）: {}",
        a.tasks[0].name
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn progress_updates_bytes_and_progress_flag() {
    let addr = stub_trickle();
    let mut a = mkapp("progress");
    let dir = std::env::temp_dir().join(format!("ezr-hard-pg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    pump(&mut a, 20, |x| {
        x.tasks[0].downloaded > 0 && x.tasks[0].made_progress
    })
    .await;
    assert!(
        a.tasks[0].downloaded > 0,
        "Progress 必须更新已下载字节（find `==→!=` 变异体不更新）"
    );
    assert!(
        a.tasks[0].made_progress,
        "downloaded>0 必须置进展标记（`<`/`==` 变异体不置）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn invalidated_resets_and_toasts() {
    let addr = stub_with("", 2000, false);
    let mut a = mkapp("invalidated");
    let dir = std::env::temp_dir().join(format!("ezr-hard-inv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    Sidecar {
        version: SIDECAR_VERSION,
        size: 1000,
        etag: None,
        last_modified: None,
        block_size: 512,
        block_count: 2,
        blocks: vec![512, 488],
        downloaded: 100,
        non_resumable: false,
        expected: None,
        task: SidecarTask {
            id: 1,
            added_at: 0,
            save_dir: dir.to_string_lossy().into_owned(),
            concurrency: 2,
            protocol: Protocol::Http,
        },
    }
    .save(&dir.join("f.bin.ezr").to_string_lossy())
    .unwrap();
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(2),
        None,
    );
    a.tasks[0].downloaded = 500;
    pump(&mut a, 20, |x| {
        x.toast
            .as_deref()
            .is_some_and(|t| t.contains("作废") || t.contains("从头"))
    })
    .await;
    assert!(
        a.toast
            .as_deref()
            .is_some_and(|t| t.contains("作废") || t.contains("从头")),
        "一致性失效必须提示（find `==→!=` 变异体静默跳过）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn verify_done_completes_task() {
    let addr = stub_with("", 128_000, false);
    let data = data_pattern(128_000);
    let idx = model::checksum::algo_index_by_name("SHA-256").unwrap();
    let good = model::checksum::digest_bytes(&data, idx);
    let mut a = mkapp("verify-ok");
    let dir = std::env::temp_dir().join(format!("ezr-hard-vo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(2),
        Some(Checksum {
            algo: "SHA-256",
            value: good,
        }),
    );
    pump(&mut a, 30, |x| x.tasks[0].state == TaskState::Completed).await;
    assert_eq!(
        a.tasks[0].state,
        TaskState::Completed,
        "校验通过必须完成（find `==→!=` 变异体停在校验中）"
    );
    assert_eq!(a.tasks[0].verify_ok, Some(true));
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn transient_failure_auto_retries() {
    let mut a = mkapp("auto-retry");
    a.add_cli_task("http://127.0.0.1:1/none.bin".into(), None, Some(1), None);
    pump(&mut a, 15, |x| x.tasks[0].state == TaskState::Failed).await;
    assert_eq!(a.tasks[0].state, TaskState::Failed);
    assert!(
        a.tasks[0].retry_in.is_some(),
        "瞬时失败必须有自动重试倒计时（delete-arm 变异体落入停等臂）"
    );
    assert!(a.tasks[0].has_slot, "待自动重试必须继续占槽");
    assert!(
        a.toast.as_deref().is_some_and(|t| t.contains("自动重试")),
        "toast={:?}",
        a.toast
    );
    a.shutdown().await;
}

#[tokio::test]
async fn add_task_persists_registry() {
    let mut a = mkapp("persist");
    let dir = std::env::temp_dir().join(format!("ezr-hard-ps-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    a.add_cli_task(
        "http://example.com/p.bin".into(),
        Some(dir.to_string_lossy().into_owned()),
        None,
        None,
    );
    let reg =
        std::env::temp_dir().join(format!("ezr-hard-eng-{}-persist.json", std::process::id()));
    assert!(reg.exists(), "添加任务后注册表必须落盘（no-op 变异体不写）");
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn reverify_reads_companion_checksum() {
    let addr = stub_with("", 128_000, false);
    let data = data_pattern(128_000);
    let idx = model::checksum::algo_index_by_name("SHA-256").unwrap();
    let good = model::checksum::digest_bytes(&data, idx);
    let mut a = mkapp("reverify");
    let dir = std::env::temp_dir().join(format!("ezr-hard-rv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(2),
        Some(Checksum {
            algo: "SHA-256",
            value: "0".repeat(64),
        }),
    );
    pump(&mut a, 30, |x| {
        x.tasks[0].state == TaskState::Failed && x.tasks[0].fail_kind == Some(FailKind::Verify)
    })
    .await;
    assert_eq!(
        a.tasks[0].fail_kind,
        Some(FailKind::Verify),
        "错误期望值必须以校验失败收场"
    );
    std::fs::write(dir.join("f.bin.sha256"), &good).unwrap();
    a.on_key(KeyCode::Char('r'), KeyModifiers::empty());
    pump(&mut a, 30, |x| x.tasks[0].state == TaskState::Completed).await;
    assert_eq!(
        a.tasks[0].state,
        TaskState::Completed,
        "R 读伴随纠正期望值后必须校验通过（resolve_checksum→None 变异体永远失败）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn failure_at_limit_toast_reports_exhausted_retries() {
    let root = crate::model::testenv::uniq_tmp_dir("ezr-hard-al-root");
    let save = crate::model::testenv::uniq_tmp_dir("ezr-hard-al-save");
    std::fs::write(root.join("f503.bin"), vec![0u8; 1024]).expect("写 fixture 文件");
    let (_fx, port) = fixture::spawn_serve(&root);

    let mut a = mkapp("at-limit-toast");
    let t = Task::new_queued(
        1,
        "f503.bin".into(),
        Protocol::Http,
        format!("http://127.0.0.1:{port}/f503.bin?status=503"),
        save.to_string_lossy().into_owned(),
        1 << 16,
        1,
        2,
        None,
        crate::model::config::ProxyChoice::Direct,
        unix_now(),
    );
    a.tasks.push(t);
    a.selected = 0;

    let toast1 = std::cell::RefCell::new(String::new());
    pump(&mut a, 10, |x| {
        if x.tasks[0].state == TaskState::Failed && x.tasks[0].retry_in.is_some() {
            *toast1.borrow_mut() = x.toast.clone().unwrap_or_default();
            return true;
        }
        false
    })
    .await;
    assert_eq!(
        a.tasks[0].state,
        TaskState::Failed,
        "503 → Transient：首次失败进入自动重试倒计时"
    );
    assert!(
        toast1.borrow().contains("自动重试"),
        "首次失败 toast = 自动重试口径: [{}]",
        toast1.borrow()
    );

    let toast2 = std::cell::RefCell::new(String::new());
    pump(&mut a, 25, |x| {
        if x.tasks[0].state == TaskState::Failed && x.tasks[0].retry_in.is_none() {
            *toast2.borrow_mut() = x.toast.clone().unwrap_or_default();
            return true;
        }
        false
    })
    .await;
    assert!(
        toast2.borrow().contains("已达最大重试次数") && toast2.borrow().contains("停止自动重试"),
        "达上限 toast = at_limit 臂口径（n/m + 停止自动重试）: [{}]",
        toast2.borrow()
    );
    assert!(
        toast2.borrow().contains("按 R 手动重试"),
        "达上限 toast 指引手动重试入口: [{}]",
        toast2.borrow()
    );
    assert!(
        a.tasks[0].retry_in.is_none() && !a.tasks[0].has_slot,
        "达上限：停等释放槽位"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&save);
}

#[tokio::test]
async fn block_worker_completes_multi_block_download() {
    let root = crate::model::testenv::uniq_tmp_dir("ezr-hard-bw-root");
    let save = crate::model::testenv::uniq_tmp_dir("ezr-hard-bw-save");
    std::fs::write(root.join("f.bin"), vec![7u8; 3_000_000]).expect("写 fixture 文件");
    let (_fx, port) = fixture::spawn_serve(&root);
    let mut a = mkapp("block-complete");
    a.cfg.block_size_http = 1 << 20;
    a.add_cli_task(
        format!("http://127.0.0.1:{port}/f.bin"),
        Some(save.to_string_lossy().into_owned()),
        Some(4),
        None,
    );
    pump(&mut a, 30, |x| x.tasks[0].state == TaskState::Completed).await;
    assert_eq!(
        a.tasks[0].state,
        TaskState::Completed,
        "多块下载必须整块完成（mark_done + 领块循环推进）"
    );
    assert_eq!(a.tasks[0].downloaded, 3_000_000, "多块路径字节守恒");
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&save);
}

#[tokio::test]
async fn block_worker_pause_mid_download_preserves_partial_block() {
    let root = crate::model::testenv::uniq_tmp_dir("ezr-hard-bwp-root");
    let save = crate::model::testenv::uniq_tmp_dir("ezr-hard-bwp-save");
    std::fs::write(root.join("slow.bin"), vec![0u8; 3_000_000]).expect("写 fixture 文件");
    let (_fx, port) = fixture::spawn_serve(&root);
    let mut a = mkapp("block-pause-mid");
    a.cfg.block_size_http = 1 << 20;
    a.add_cli_task(
        format!("http://127.0.0.1:{port}/slow.bin?speed=20000"),
        Some(save.to_string_lossy().into_owned()),
        Some(4),
        None,
    );
    pump(&mut a, 15, |x| {
        x.tasks[0].state == TaskState::Downloading && x.tasks[0].downloaded > 0
    })
    .await;
    assert_eq!(a.tasks[0].state, TaskState::Downloading, "慢速门控下在传");
    let before = a.tasks[0].downloaded;
    a.selected = 0;
    a.toggle_pause();
    pump(&mut a, 15, |x| x.tasks[0].state == TaskState::Paused).await;
    assert_eq!(a.tasks[0].state, TaskState::Paused, "暂停收尾");
    assert!(
        a.tasks[0].downloaded >= before,
        "块内已写字节保留（不回退）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&save);
}

#[tokio::test]
async fn block_worker_mid_transfer_disconnect_fails_transient() {
    let root = crate::model::testenv::uniq_tmp_dir("ezr-hard-bwd-root");
    let save = crate::model::testenv::uniq_tmp_dir("ezr-hard-bwd-save");
    std::fs::write(root.join("disc.bin"), vec![0u8; 3_000_000]).expect("写 fixture 文件");
    let (_fx, port) = fixture::spawn_serve(&root);
    let mut a = mkapp("block-disconnect");
    a.cfg.block_size_http = 1 << 20;
    a.add_cli_task(
        format!("http://127.0.0.1:{port}/disc.bin?disconnect=5000"),
        Some(save.to_string_lossy().into_owned()),
        Some(4),
        None,
    );
    pump(&mut a, 20, |x| x.tasks[0].state == TaskState::Failed).await;
    assert_eq!(
        a.tasks[0].state,
        TaskState::Failed,
        "中途断连必须转失败（chunk Err → classify）"
    );
    assert!(
        a.tasks[0].fail_kind == Some(model::FailKind::Transient),
        "断连分类为瞬时（可自动重试）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&save);
}
