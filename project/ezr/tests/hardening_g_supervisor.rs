//! 加固测试（six-pack/hardender 步骤 7 批次 7）：engine/supervisor.rs 部分存活体处置。
//! 靶点：sidecar_path 167:5（恒串变异 ×2）——暂停落盘必须写到标准
//! `{save_dir}/{name}.ezr`；stop_workers 255:5（no-op）——暂停后传输必须停止。
//! 其余 download/block_worker 存活体按会话偏差登记（见 .work/tmp/step7/EQUIVALENCE.md
//! 与 project/handoff.md 的未处置清单及补测指引）。
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

#[path = "../src/engine/mod.rs"]
mod engine;
#[path = "../src/model/mod.rs"]
mod model;

use crossterm::event::{KeyCode, KeyModifiers};
use model::config::Config;
use model::TaskState;

#[path = "../src/app/mod.rs"]
mod app;

use app::App;

/// 持续滴流服务器：每 100ms 发 200 字节 × 100 次
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

/// 靶 supervisor 167:5 / 255:5：暂停必须停传并把 sidecar 落盘到标准路径
#[tokio::test]
async fn pause_stops_transfer_and_writes_sidecar() {
    let addr = stub_trickle();
    let reg = std::env::temp_dir().join(format!("ezr-hard-sup-{}.json", std::process::id()));
    let dir = std::env::temp_dir().join(format!("ezr-hard-sup-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut a = App::new(Config::default(), reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while std::time::Instant::now() < deadline {
        a.tick().await;
        if a.tasks[0].state == TaskState::Downloading {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(
        a.tasks[0].state,
        TaskState::Downloading,
        "滴流下载应进入下载中"
    );
    a.on_key(KeyCode::Char(' '), KeyModifiers::empty()); // Space → 暂停
    let sc = dir.join("f.bin.ezr");
    while std::time::Instant::now() < deadline {
        a.tick().await;
        if sc.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(!a.tasks[0].has_slot || true, "占位"); // has_slot 在本地即时置位，不作判据
    assert!(
        sc.exists(),
        "暂停必须把 sidecar 写到标准路径 {}（恒串变异体写去 xyzzy/空路径）",
        sc.display()
    );
    let frozen = a.tasks[0].downloaded;
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    a.tick().await;
    assert_eq!(
        a.tasks[0].downloaded, frozen,
        "暂停后下载不得继续增长（stop_workers no-op 变异体仍在收数据）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}
