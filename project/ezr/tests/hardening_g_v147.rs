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

#[path = "../src/bt/mod.rs"]
mod bt;
#[path = "../src/engine/mod.rs"]
mod engine;
#[path = "../src/model/mod.rs"]
mod model;

use crossterm::event::{KeyCode, KeyModifiers};
use model::config::Config;
use model::TaskState;

#[path = "../src/app/mod.rs"]
mod app;
#[path = "../src/multiline.rs"]
mod multiline;
#[path = "../src/scrollbar.rs"]
mod scrollbar;
#[path = "../src/textfield.rs"]
mod textfield;

use app::App;

fn workspace(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "ezr-hard-v147-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let reg = std::env::temp_dir().join(format!("ezr-hard-v147-{tag}-{}.json", std::process::id()));
    (dir, reg)
}

fn cleanup(dir: &std::path::Path, reg: &std::path::Path) {
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_file(reg);
}

fn stub_status(code: u16, reason: &str) -> std::net::SocketAddr {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let head =
        format!("HTTP/1.1 {code} {reason}\r\nContent-Length: 5\r\nConnection: close\r\n\r\noops\n");
    std::thread::spawn(move || {
        for mut c in listener.incoming().flatten() {
            {
                let mut reader = BufReader::new(&c);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                }
            }
            let _ = c.write_all(head.as_bytes());
            let _ = c.flush();
        }
    });
    addr
}

fn stub_200_burst_cut(bytes_before_cut: usize) -> std::net::SocketAddr {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for mut c in listener.incoming().flatten() {
            {
                let mut reader = BufReader::new(&c);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                }
            }
            let _ = c.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\nConnection: close\r\n\r\n",
            );
            let _ = c.flush();
            let chunk = [7u8; 8192];
            let mut sent = 0usize;
            while sent < bytes_before_cut {
                if c.write_all(&chunk).is_err() {
                    break;
                }
                sent += chunk.len();
            }
            drop(c);
        }
    });
    addr
}

fn stub_206_chunked_cut() -> std::net::SocketAddr {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let head = "HTTP/1.1 206 Partial Content\r\nContent-Length: 3000000\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n";
    std::thread::spawn(move || {
        for mut c in listener.incoming().flatten() {
            {
                let mut reader = BufReader::new(&c);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                }
            }
            let _ = c.write_all(head.as_bytes());
            let _ = c.flush();
        }
    });
    addr
}

fn stub_206_then_403() -> std::net::SocketAddr {
    use std::io::{BufRead, BufReader, Write};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let n = std::sync::Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for mut c in listener.incoming().flatten() {
            {
                let mut reader = BufReader::new(&c);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                }
            }
            if n.fetch_add(1, Ordering::Relaxed) == 0 {
                let _ = c.write_all(
                    b"HTTP/1.1 206 Partial Content\r\nContent-Length: 3000000\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                );
            } else {
                let _ = c.write_all(
                    b"HTTP/1.1 403 Forbidden\r\nContent-Length: 5\r\nConnection: close\r\n\r\noops\n",
                );
            }
            let _ = c.flush();
        }
    });
    addr
}

fn stub_206_cut() -> std::net::SocketAddr {
    stub_status(206, "Partial Content")
}

fn stub_206_trickle() -> std::net::SocketAddr {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for c in listener.incoming().flatten() {
            let mut c = c;
            let _ = c.write_all(
                b"HTTP/1.1 206 Partial Content\r\nContent-Length: 3000000\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
            );
            let _ = c.flush();
            for _ in 0..50 {
                if c.write_all(&[9u8; 200]).is_err() {
                    break;
                }
                let _ = c.flush();
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            drop(c);
        }
    });
    addr
}

fn stub_200_norange_slow() -> std::net::SocketAddr {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for c in listener.incoming().flatten() {
            let mut c = c;
            let _ = c.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\nConnection: close\r\n\r\n",
            );
            let _ = c.flush();
            for _ in 0..80 {
                if c.write_all(&[3u8; 256]).is_err() {
                    break;
                }
                let _ = c.flush();
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            drop(c);
        }
    });
    addr
}

async fn wait_state(a: &mut App, idx: usize, target: TaskState, secs: u64) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        a.tick().await;
        if a.tasks[idx].state == target {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    }
    a.tasks[idx].state == target
}

#[tokio::test]
async fn worker_block_403_fails_fatal_not_transient() {
    let addr = stub_206_then_403();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let (dir, reg) = workspace("h403");
    let cfg = Config {
        auto_retry: false,
        ..Config::default()
    };
    let mut a = App::new(cfg, reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let hit = wait_state(&mut a, 0, TaskState::Failed, 20).await;
    assert!(
        hit,
        "块请求 403 必须失败；实际 {:?} err={:?} dl={}",
        a.tasks[0].state, a.tasks[0].error, a.tasks[0].downloaded
    );
    assert_eq!(
        a.tasks[0].fail_kind,
        Some(model::FailKind::Fatal),
        "4xx 块请求必须按语义性错误（Fatal）分类；37:18 突变体（guard→true）把 \
         403 当成功 → body EOF → 网络分类 → Transient；实际 kind={:?} err={:?}",
        a.tasks[0].fail_kind,
        a.tasks[0].error
    );
    a.shutdown().await;
    cleanup(&dir, &reg);
}

#[tokio::test]
async fn zero_written_chunk_failure_counts_continuous_retry() {
    let addr = stub_206_chunked_cut();
    let (dir, reg) = workspace("r0");
    let cfg = Config {
        auto_retry: false,
        ..Config::default()
    };
    let mut a = App::new(cfg, reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let hit = wait_state(&mut a, 0, TaskState::Failed, 20).await;
    assert!(hit, "立即断连必须失败；实际 {:?}", a.tasks[0].state);
    assert_eq!(
        a.tasks[0].retries, 1,
        "首次失败 retries=0+1（made_progress=false）"
    );
    a.on_key(KeyCode::Char('r'), KeyModifiers::empty());
    let hit2 = wait_state(&mut a, 0, TaskState::Failed, 20).await;
    assert!(hit2, "重试后必须再次失败");
    assert_eq!(
        a.tasks[0].retries, 2,
        "0 进度失败必须按连续失败累计（made_progress=false → retries=1+1=2）；\
         73:47 突变体（> → ==）把 0 进度误判有进展 → 重置为 1"
    );
    a.shutdown().await;
    cleanup(&dir, &reg);
}

#[tokio::test]
async fn single_stream_200_failure_writes_no_ledger() {
    let addr = stub_200_burst_cut(400 * 1024);
    let (dir, reg) = workspace("lg");
    let mut a = App::new(Config::default(), reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let hit = wait_state(&mut a, 0, TaskState::Failed, 25).await;
    assert!(hit, "burst 后断连必须失败；实际 {:?}", a.tasks[0].state);
    let sc = dir.join("f.bin.ezr");
    assert!(
        !sc.exists(),
        "200 单流（multi_ok=false）失败不得写断点台账；88:18 突变体（||→&&）会写 Some"
    );
    a.shutdown().await;
    cleanup(&dir, &reg);
}

#[tokio::test]
async fn single_stream_downgrade_emits_toast() {
    let addr = stub_200_norange_slow();
    let (dir, reg) = workspace("tst");
    let mut a = App::new(Config::default(), reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut found = false;
    while std::time::Instant::now() < deadline {
        a.tick().await;
        if let Some(t) = &a.toast {
            if t.contains("单线程") {
                found = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    }
    assert!(
        found,
        "200 降级单流必须发降级 Toast；81:8 突变体（delete !）只在 multi_ok 时发"
    );
    a.shutdown().await;
    cleanup(&dir, &reg);
}

#[tokio::test]
async fn periodic_flush_writes_sidecar_mid_download() {
    let addr = stub_206_trickle();
    let (dir, reg) = workspace("fl");
    let mut a = App::new(Config::default(), reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let dl = wait_state(&mut a, 0, TaskState::Downloading, 20).await;
    assert!(dl, "滴流分块应进入下载中");
    tokio::time::sleep(std::time::Duration::from_millis(4600)).await;
    a.tick().await;
    let sc = dir.join("f.bin.ezr");
    let content = std::fs::read_to_string(&sc).unwrap_or_default();
    assert!(
        sc.exists() && content.contains("\"downloaded\""),
        "分块下载中途必须周期落盘 sidecar（54:5 突变体 flush_sidecar→() 不落盘）；\
         sc={} content_len={}",
        sc.display(),
        content.len()
    );
    a.on_key(KeyCode::Char(' '), KeyModifiers::empty());
    a.tick().await;
    a.shutdown().await;
    cleanup(&dir, &reg);
}

fn stub_nocl_trickle() -> std::net::SocketAddr {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for mut c in listener.incoming().flatten() {
            {
                let mut reader = BufReader::new(&c);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                }
            }
            let _ = c.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n");
            let _ = c.flush();
            for _ in 0..60 {
                if c.write_all(&[5u8; 128]).is_err() {
                    break;
                }
                let _ = c.flush();
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    });
    addr
}

fn stub_range_recorder(log: std::sync::Arc<std::sync::Mutex<Vec<String>>>) -> std::net::SocketAddr {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for mut c in listener.incoming().flatten() {
            let mut head = String::new();
            {
                let mut reader = BufReader::new(&c);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                    head.push_str(&line);
                }
            }
            log.lock().unwrap().push(head);
            let _ = c.write_all(
                b"HTTP/1.1 206 Partial Content\r\nContent-Length: 6291456\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
            );
            let _ = c.flush();
            for _ in 0..40 {
                if c.write_all(&[6u8; 512]).is_err() {
                    break;
                }
                let _ = c.flush();
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    });
    addr
}

fn stub_two_phase_cl() -> std::net::SocketAddr {
    use std::io::{BufRead, BufReader, Write};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let n = std::sync::Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for mut c in listener.incoming().flatten() {
            {
                let mut reader = BufReader::new(&c);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                }
            }
            let cl = if n.fetch_add(1, Ordering::Relaxed) < 2 {
                2621440u64
            } else {
                3040870u64
            };
            let _ = c.write_all(
                format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {cl}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            );
            let _ = c.flush();
            for _ in 0..30 {
                if c.write_all(&[7u8; 4096]).is_err() {
                    break;
                }
                let _ = c.flush();
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
        }
    });
    addr
}

fn stub_206_cl0() -> std::net::SocketAddr {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for mut c in listener.incoming().flatten() {
            {
                let mut reader = BufReader::new(&c);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                }
            }
            let _ = c.write_all(
                b"HTTP/1.1 206 Partial Content\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
            );
            let _ = c.flush();
            for _ in 0..80 {
                if c.write_all(&[1u8; 64]).is_err() {
                    break;
                }
                let _ = c.flush();
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    });
    addr
}

async fn wait_until(a: &mut App, secs: u64, mut pred: impl FnMut(&App) -> bool) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        a.tick().await;
        if pred(a) {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    }
    pred(a)
}

#[tokio::test]
async fn unknown_size_resume_invalidates_not_transient_fail() {
    let addr = stub_nocl_trickle();
    let (dir, reg) = workspace("u0");
    let sc = model::sidecar::Sidecar {
        version: 1,
        size: 0,
        etag: None,
        last_modified: None,
        block_size: 1024 * 1024,
        block_count: 0,
        blocks: vec![],
        downloaded: 0,
        non_resumable: false,
        expected: None,
        task: model::sidecar::SidecarTask {
            id: 999,
            added_at: 0,
            save_dir: dir.to_string_lossy().into_owned(),
            concurrency: 1,
            protocol: model::Protocol::Http,
        },
    };
    sc.save(&dir.join("f.bin.ezr").to_string_lossy()).unwrap();
    let cfg = Config {
        auto_retry: false,
        ..Config::default()
    };
    let mut a = App::new(cfg, reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let _ = wait_until(&mut a, 12, |a| a.tasks[0].invalidation_streak >= 1).await;
    assert!(
        a.tasks[0].invalidation_streak >= 1,
        "未知大小台账 + 无 CL 探测必须走一致性失效重下（Invalidated）；\
         117:44 突变体（> → >=）把 size=0 台账误判「已知大小」→ 提前暂态失败；\
         实际 streak={} state={:?} err={:?}",
        a.tasks[0].invalidation_streak,
        a.tasks[0].state,
        a.tasks[0].error
    );
    a.shutdown().await;
    cleanup(&dir, &reg);
}

#[tokio::test]
async fn matching_sidecar_resumes_with_nonzero_range() {
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let addr = stub_range_recorder(log.clone());
    let (dir, reg) = workspace("sc");
    let cfg = Config {
        auto_retry: false,
        ..Config::default()
    };
    let mut a = App::new(cfg, reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let hit = wait_state(&mut a, 0, TaskState::Failed, 20).await;
    assert!(hit, "滴流断连必须失败 state={:?}", a.tasks[0].state);
    assert!(a.tasks[0].downloaded > 0, "失败前应有进度");
    assert!(
        dir.join("f.bin.ezr").exists(),
        "分块失败（multi_ok、有进度）必须落失败台账 failed_flow→Some"
    );
    a.on_key(KeyCode::Char('r'), KeyModifiers::empty());
    let _ = wait_until(&mut a, 8, |a| {
        a.tasks[0].state == TaskState::Downloading || a.tasks[0].state == TaskState::Failed
    })
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    a.tick().await;
    let heads_snapshot: Vec<String> = log.lock().unwrap().clone();
    let has_resume_range = heads_snapshot.iter().any(|h| {
        for line in h.lines() {
            let line = line.trim();
            let lower = line.to_ascii_lowercase();
            if let Some(r) = lower.strip_prefix("range:") {
                let r = r.trim();
                if let Some(rest) = r.strip_prefix("bytes=") {
                    if let Some(start) = rest.split('-').next() {
                        if let Ok(n) = start.parse::<u64>() {
                            if n > 0 {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    });
    assert!(
        has_resume_range,
        "同名同 id sidecar 必须断点接续（恢复请求带非零 Range 起点）；\
         108:78 突变体（== → !=）翻转过滤器 → 弃账重下；请求头日志 {:?}",
        heads_snapshot
            .iter()
            .map(|h| h.replace("\r\n", " | "))
            .collect::<Vec<_>>()
    );
    a.shutdown().await;
    cleanup(&dir, &reg);
}

#[tokio::test]
async fn inconsistent_block_count_invalidates_even_when_size_matches() {
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let addr = stub_range_recorder(log.clone());
    let (dir, reg) = workspace("bc");
    let sc = model::sidecar::Sidecar {
        version: 1,
        size: 3_000_000,
        etag: None,
        last_modified: None,
        block_size: 1024 * 1024,
        block_count: 2,
        blocks: vec![20_480, 0],
        downloaded: 20_480,
        non_resumable: false,
        expected: None,
        task: model::sidecar::SidecarTask {
            id: 999,
            added_at: 0,
            save_dir: dir.to_string_lossy().into_owned(),
            concurrency: 1,
            protocol: model::Protocol::Http,
        },
    };
    sc.save(&dir.join("f.bin.ezr").to_string_lossy()).unwrap();
    let cfg = Config {
        auto_retry: false,
        ..Config::default()
    };
    let mut a = App::new(cfg, reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let _ = wait_until(&mut a, 12, |a| a.tasks[0].invalidation_streak >= 1).await;
    assert!(
        a.tasks[0].invalidation_streak >= 1,
        "块数不自洽（size 一致）必须失效重下；129:59 突变体（&&→||）被 \
         size 恒等单条件放行 → 误接续；实际 streak={} state={:?} err={:?}",
        a.tasks[0].invalidation_streak,
        a.tasks[0].state,
        a.tasks[0].error
    );
    a.shutdown().await;
    cleanup(&dir, &reg);
}

#[tokio::test]
async fn zero_length_206_still_downgrades_with_toast() {
    let addr = stub_206_cl0();
    let (dir, reg) = workspace("z0");
    let mut a = App::new(Config::default(), reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let found = wait_until(&mut a, 30, |a| {
        a.toast.as_ref().is_some_and(|t| t.contains("单线程"))
    })
    .await;
    assert!(
        found,
        "206+CL:0（total=0）必须降级单流并发提示；151:53 突变体（> → >=）把 \
         total=0 误判 multi_ok → 降级提示消失"
    );
    a.shutdown().await;
    cleanup(&dir, &reg);
}

fn stub_probe_then_403_then_trickle() -> std::net::SocketAddr {
    use std::io::{BufRead, BufReader, Write};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let n = std::sync::Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for mut c in listener.incoming().flatten() {
            {
                let mut reader = BufReader::new(&c);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                }
            }
            let k = n.fetch_add(1, Ordering::Relaxed);
            if k == 0 {
                let _ = c.write_all(
                    b"HTTP/1.1 206 Partial Content\r\nContent-Length: 3000000\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                );
            } else if k == 1 {
                let _ = c.write_all(
                    b"HTTP/1.1 403 Forbidden\r\nContent-Length: 5\r\nConnection: close\r\n\r\noops\n",
                );
            } else {
                let _ = c.write_all(
                    b"HTTP/1.1 206 Partial Content\r\nContent-Length: 3000000\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                );
                let _ = c.flush();
                loop {
                    if c.write_all(&[8u8; 4096]).is_err() {
                        break;
                    }
                    let _ = c.flush();
                    std::thread::sleep(std::time::Duration::from_millis(40));
                }
            }
            let _ = c.flush();
        }
    });
    addr
}

fn stub_206_burst_2m() -> std::net::SocketAddr {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for mut c in listener.incoming().flatten() {
            let mut head = String::new();
            {
                let mut reader = BufReader::new(&c);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                    head.push_str(&line);
                }
            }
            let mut start = 0u64;
            let mut end = 2_097_151u64;
            for line in head.lines() {
                let lower = line.to_ascii_lowercase();
                if let Some(rest) = lower.strip_prefix("range:") {
                    let rest = rest.trim().strip_prefix("bytes=").unwrap_or(rest.trim());
                    let mut it = rest.split('-');
                    if let Some(s) = it.next().and_then(|v| v.parse::<u64>().ok()) {
                        start = s;
                    }
                    if let Some(e) = it.next().and_then(|v| v.parse::<u64>().ok()) {
                        end = e;
                    }
                }
            }
            let len = end - start + 1;
            let _ = c.write_all(
                format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {len}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            );
            let _ = c.flush();
            let mut sent = 0u64;
            let chunk = [0xA5u8; 65536];
            while sent < len {
                let n = chunk.len().min((len - sent) as usize);
                if c.write_all(&chunk[..n]).is_err() {
                    break;
                }
                sent += n as u64;
            }
        }
    });
    addr
}

#[tokio::test]
async fn failure_freezes_all_worker_progress() {
    let addr = stub_probe_then_403_then_trickle();
    let (dir, reg) = workspace("stp");
    let cfg = Config {
        auto_retry: false,
        ..Config::default()
    };
    let mut a = App::new(cfg, reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(2),
        None,
    );
    let hit = wait_state(&mut a, 0, TaskState::Failed, 20).await;
    assert!(hit, "403 块失败必须导致任务失败");
    let frozen = a.tasks[0].downloaded;
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    a.tick().await;
    assert_eq!(
        a.tasks[0].downloaded, frozen,
        "失败收口必须冻结全部 worker 进度（stop_workers no-op 突变体下 \
         滴流 worker 继续收数据）"
    );
    a.shutdown().await;
    cleanup(&dir, &reg);
}

#[tokio::test]
async fn conns_match_quota_with_unique_ids() {
    let addr = stub_206_trickle();
    let (dir, reg) = workspace("cn");
    let mut a = App::new(Config::default(), reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(3),
        None,
    );
    let dl = wait_until(&mut a, 20, |a| {
        a.tasks[0].state == TaskState::Downloading && a.tasks[0].connections.len() == 3
    })
    .await;
    assert!(
        dl,
        "并发 3 必须恰好 3 条连接行（51:25 突变体多填 1 → 4 行）；实际 {} 行 state={:?}",
        a.tasks[0].connections.len(),
        a.tasks[0].state
    );
    let mut ids: Vec<usize> = a.tasks[0].connections.iter().map(|c| c.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(
        ids.len(),
        3,
        "连接序号必须互异（57:14 突变体 next 恒等 → 重复 id）；实际 {:?}",
        a.tasks[0]
            .connections
            .iter()
            .map(|c| c.id)
            .collect::<Vec<_>>()
    );
    a.on_key(
        crossterm::event::KeyCode::Char(' '),
        crossterm::event::KeyModifiers::empty(),
    );
    a.tick().await;
    a.shutdown().await;
    cleanup(&dir, &reg);
}

#[tokio::test]
async fn two_block_burst_download_completes() {
    let addr = stub_206_burst_2m();
    let (dir, reg) = workspace("fin");
    let cfg = Config {
        auto_retry: false,
        ..Config::default()
    };
    let mut a = App::new(cfg, reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let done = wait_until(&mut a, 15, |a| a.tasks[0].state == TaskState::Completed).await;
    assert!(
        done,
        "2 块 burst 必须正常完成；152:24（领块翻转）会卡死在重写已完成块，\
         168:25（越界扫描）会以幽灵块请求失败；实际 {:?} err={:?} dl={}",
        a.tasks[0].state, a.tasks[0].error, a.tasks[0].downloaded
    );
    a.shutdown().await;
    cleanup(&dir, &reg);
}

#[tokio::test]
async fn sidecar_not_written_within_first_500ms_of_download() {
    let addr = stub_206_trickle();
    let (dir, reg) = workspace("fdl");
    let mut a = App::new(Config::default(), reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let dl = wait_until(&mut a, 20, |a| {
        a.tasks[0].state == TaskState::Downloading && a.tasks[0].downloaded > 0
    })
    .await;
    assert!(dl, "滴流分块应进入下载");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    a.tick().await;
    let sc = dir.join("f.bin.ezr");
    assert!(
        !sc.exists(),
        "下载启动 500ms 内不得出现周期落盘的 sidecar（277:58 突变体把 \
         flush deadline 推到过去 → 首 tick 即写盘）"
    );
    a.on_key(
        crossterm::event::KeyCode::Char(' '),
        crossterm::event::KeyModifiers::empty(),
    );
    a.tick().await;
    a.shutdown().await;
    cleanup(&dir, &reg);
}

#[tokio::test]
async fn engine_progress_freezes_after_failure_event() {
    let addr = stub_probe_then_403_then_trickle();
    let (dir, reg) = workspace("e1");
    let cfg = Config {
        auto_retry: false,
        ..Config::default()
    };
    let (etx, mut erx) = tokio::sync::mpsc::channel(1024);
    let handle = engine::EngineHandle::start(&cfg, etx);
    let spec = engine::TaskSpec {
        id: 1,
        url: format!("http://{addr}/f.bin"),
        save_dir: dir.to_string_lossy().into_owned(),
        name: "f.bin".to_string(),
        concurrency: 2,
        block_size: 1024 * 1024,
        protocol: model::Protocol::Http,
        expected_algo: None,
        expected_value: None,
        sidecar: None,
        sidecar_path: None,
        keep_name: false,
        added_at: 0,
        proxy_endpoint: None,
    };
    handle.send(engine::Cmd::Start { spec }).await;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut failed_dl: Option<u64> = None;
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(500), erx.recv()).await {
            Ok(Some(engine::Evt::Failed { downloaded, .. })) => {
                failed_dl = Some(downloaded);
                break;
            }
            Ok(Some(_)) => {}
            _ => {}
        }
    }
    let at_fail = failed_dl.expect("必须收到 Failed 事件");
    let dl_file = dir.join("f.bin.downloading");
    let size_at_fail = std::fs::metadata(&dl_file).map(|m| m.len()).unwrap_or(0);
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    let size_after = std::fs::metadata(&dl_file).map(|m| m.len()).unwrap_or(0);
    assert_eq!(
        size_at_fail, size_after,
        "失败后盘上下载文件必须冻结（27:5 突变体 stop_workers→() 让 \
         滴流 worker 继续写盘）；失败时 {at_fail}/{size_at_fail} 之后 \
         文件涨到 {size_after}"
    );
    handle.send(engine::Cmd::Shutdown).await;
    cleanup(&dir, &reg);
}

#[tokio::test]
async fn engine_conns_quota_exact_and_unique_ids() {
    let addr = stub_206_trickle();
    let (dir, reg) = workspace("e2");
    let cfg = Config::default();
    let (etx, mut erx) = tokio::sync::mpsc::channel(1024);
    let handle = engine::EngineHandle::start(&cfg, etx);
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let spec = engine::TaskSpec {
        id: 1,
        url: format!("http://{addr}/f.bin"),
        save_dir: dir.to_string_lossy().into_owned(),
        name: "f.bin".to_string(),
        concurrency: 3,
        block_size: 1024 * 1024,
        protocol: model::Protocol::Http,
        expected_algo: None,
        expected_value: None,
        sidecar: None,
        sidecar_path: None,
        keep_name: false,
        added_at: 0,
        proxy_endpoint: None,
    };
    handle.send(engine::Cmd::Start { spec }).await;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    let mut max_len = 0usize;
    let mut seen_ids: Vec<usize> = Vec::new();
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(400), erx.recv()).await {
            Ok(Some(engine::Evt::Progress { conns, .. })) => {
                max_len = max_len.max(conns.len());
                if conns.len() >= 3 {
                    seen_ids = conns.iter().map(|c| c.id).collect();
                    break;
                }
            }
            Ok(Some(_ev)) => {}
            _ => {}
        }
    }
    assert_eq!(
        max_len, 3,
        "并发 3 的连接视图必须恰好 3 行（51:25 突变体 fill `<=` 多填 1 → 4 行）"
    );
    let mut ids = seen_ids.clone();
    ids.sort();
    ids.dedup();
    assert_eq!(
        ids.len(),
        3,
        "连接序号必须互异（57:14 突变体 next 恒等 → 重复 id）；实际 {seen_ids:?}"
    );
    handle.send(engine::Cmd::Shutdown).await;
    cleanup(&dir, &reg);
}
