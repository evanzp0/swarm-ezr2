//! 加固测试（six-pack/hardender 步骤 7 批次 6）：app/engine.rs 56 存活体处置。
//! 事件路径经真实引擎 + 本地桩 HTTP 服务器驱动（与产品 tick_tests 同口径），
//! 纯状态路径直接构造 App 字段（pub）后 tick。
//! 等价/环境型论证见 .work/tmp/step7/EQUIVALENCE.md：
//! 188:28（展示面私有态）、205:20（时刻恰等边界）、326:70（暂停竞态权威字节）、
//! 317:31 `>=`（0 字节 Progress 不可稳定激发）、490:23×3（Progress 镜像使合并守卫不可分）、
//! 565:33（引擎侧确认事件仅真实会话可观测）、78:18 `<=`（可用空间恰等需注入）。
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

/// 桩 HTTP 服务器（i%251 内容 + 附加头）
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
                    return; // 不发完也不关：悬置连接
                }
                let _ = c.write_all(&data);
                let _ = c.flush();
            });
        }
    });
    addr
}

/// 持续滴流服务器：每 100ms 发 200 字节×100 次（单流路径 200ms 节流后必然上报 Progress）
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

// ------------------------------------------------- disk_precheck（tick 路径）---

/// 靶 68:9（→true）/70:25（delete !）/78:18（`<`→`==`/`>`）：空间不足必须判失败
#[tokio::test]
async fn disk_precheck_fails_when_insufficient() {
    let mut a = mkapp("disk1");
    let dir = std::env::temp_dir().join(format!("ezr-hard-disk1-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut t = queued(1, "disk.bin", &dir.to_string_lossy());
    t.probed = true;
    t.total = u64::MAX / 2; // need 远大于实际可用
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

/// 靶 70:22（`||→&&`）：未探测任务跳过预检
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

// ----------------------------------------------------- tick 重试与统计路径 ---

/// 靶 142:30（`<=→>`）：退避到点必须转回等待中
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

/// 靶 245:62（`==→!=`）：到点重试只清目标任务的进展标记
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

/// 靶 164:46/60/72（`&&→||`）：4b 补发过滤三条件缺一不可
#[tokio::test]
async fn retry_filter_requires_all_conditions() {
    let mut a = mkapp("retry-filter");
    a.max_slots = 0; // 冻结槽位分配，隔离 4b 过滤判定
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

/// 靶 177:24（`!=→==`）：非下载态速度立即归零
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

/// 靶 184:66（`>=→<`）：1s 节拍必须触发历史采样
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

/// 靶 212:47（`>=→<`）：5s 兜底保存不得在新建后立即触发
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

/// 靶 198:28（`+=→*=`）：零速不改变累计
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

/// 累计按真实已下载字节的运行内增量增长（FR-01-81「本次运行累计下载字节」；
/// QA 会话修订：原「速度×dt 积分」口径为 EMA 展示速度积分，短任务少计约一半，
/// 与 Gherkin 01-tui-display-12 THEN 冲突，改为 downloaded 增量账本。变异靶
/// （原 198:28 `+=→-=`、198:42 `*→+`/`*→/`）的行号待下一 hardender 会话重扫刷新。
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
        t.downloaded = 1500; // 数据面前进 1100 字节
    }
    a.tick().await;
    assert_eq!(
        a.session_bytes, 1200,
        "累计必须按真实字节增量累加（-= 变异体递减/下溢、*→+ 变异体越界）"
    );
    a.shutdown().await;
}

/// 靶 222:30 / 223:38：越界选中钳到末尾
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

/// 靶 226:28：滚动偏移不得小于选中项
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

/// 靶 230:45/230:49：滚动窗口上界推进
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

// --------------------------------------------------- 事件路径（真实引擎）---

/// 靶 265:63/267:37→false/267:45/267:70→本来/271:72/279:70/289:32：
/// Probed 基础流 —— CD 改名、probed 置位、Queued→Downloading
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

/// 靶 267:37→true / 267:53（`&&→||`）：已有下载量不得改名
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

/// 靶 271:51/271:67/271:86（taken 闭包各比较符）：任务表重名冲突 → 去重改名
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
    t2.state = TaskState::Paused; // 不参与槽位竞争
    a.tasks.push(t2);
    pump(&mut a, 20, |x| x.tasks[0].probed).await;
    assert!(
        a.tasks[0].name.starts_with("b.bin.1"),
        "任务表已占用 b.bin → 去重为 b.bin.1 系（taken 闭包比较符变异体保持 b.bin/f.bin）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// 靶 272:37（`||→&&`）：盘上重名冲突亦去重
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

/// 靶 301:67（`==→!=`）/317:31（`>`→`<`/`==`）：Progress 更新字节与进展标记
#[tokio::test]
async fn progress_updates_bytes_and_progress_flag() {
    let addr = stub_trickle(); // 持续喂数据 → 200ms 节流后必然上报 Progress
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

/// 靶 357:67（`==→!=`）：续传一致性失效必须清零并提示
#[tokio::test]
async fn invalidated_resets_and_toasts() {
    let addr = stub_with("", 2000, false); // sidecar 记录 size=1000 → 大小不一致 → 失效
    let mut a = mkapp("invalidated");
    let dir = std::env::temp_dir().join(format!("ezr-hard-inv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    Sidecar {
        version: SIDECAR_VERSION,
        url: format!("http://{addr}/f.bin"),
        final_url: None,
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

/// 靶 439:67（`==→!=`）：校验通过必须转完成态
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

/// 靶 514:13（delete auto 臂）：瞬时失败必须自动重试（占槽 + 倒计时 + 文案）
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

/// 靶 548:9（`save_registry→()`）：添加任务必须落盘注册表
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

/// 靶 36:9（resolve_checksum→None）：R 重新校验读伴随文件纠正期望值
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
    // 伴随文件给出正确校验值 → R 重新校验通过
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
