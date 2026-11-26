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

pub const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-01");

#[path = "../src/app/mod.rs"]
mod app;
#[path = "../src/bt/mod.rs"]
mod bt;
#[path = "../src/engine/mod.rs"]
mod engine;
#[path = "../src/model/mod.rs"]
mod model;
#[path = "../src/multiline.rs"]
mod multiline;
#[path = "../src/scrollbar.rs"]
mod scrollbar;
#[path = "../src/textfield.rs"]
mod textfield;

#[path = "../src/ui/mod.rs"]
mod ui;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::Duration;

use app::App;
use model::config::Config;
use model::{Connection, Protocol, Task, TaskState};

fn fill(buf: &mut [u8], start: u64) {
    for (i, b) in buf.iter_mut().enumerate() {
        *b = ((start as usize + i) % 251) as u8;
    }
}

fn mock_server(total: u64, slow: bool) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut w = conn.try_clone().unwrap();
                let mut r = BufReader::new(conn);
                let mut line = String::new();
                if r.read_line(&mut line).is_err() || line.is_empty() {
                    return;
                }
                let mut range_header = None;
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).is_err() || h.trim().is_empty() {
                        break;
                    }
                    if let Some((k, v)) = h.split_once(':') {
                        if k.trim().eq_ignore_ascii_case("range") {
                            range_header = Some(v.trim().to_string());
                        }
                    }
                }
                let (start, end, status) = match range_header
                    .as_deref()
                    .and_then(|v| v.strip_prefix("bytes="))
                {
                    Some(rv) => {
                        let (a, b) = rv.split_once('-').unwrap();
                        let s = a.parse::<u64>().unwrap_or(0);
                        let e = if b.is_empty() {
                            total - 1
                        } else {
                            b.parse::<u64>().unwrap_or(total - 1).min(total - 1)
                        };
                        (s, e, 206)
                    }
                    _ => (0, total - 1, 200),
                };
                let len = end - start + 1;
                let _ = write!(
                w,
                "HTTP/1.1 {status} OK\r\nAccept-Ranges: bytes\r\nETag: \"mock-{total}\"\r\nLast-Modified: Mon, 09 Feb 2026 08:00:00 GMT\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
            );
                let mut buf = vec![0u8; 8192];
                let mut pos = start;
                while pos <= end {
                    let n = (end - pos + 1).min(8192) as usize;
                    fill(&mut buf[..n], pos);
                    if w.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    if slow {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    pos += n as u64;
                }
            });
        }
    });
    format!("http://{addr}/f.bin")
}

fn queued_task(id: u32, url: &str, dir: &str, conns: usize) -> Task {
    Task::new_queued(
        id,
        "f.bin".to_string(),
        Protocol::Http,
        url.to_string(),
        dir.to_string(),
        512 * 1024,
        conns,
        3,
        None,
        model::config::ProxyChoice::Direct,
        model::unix_now(),
    )
}

const TOTAL: u64 = 2 * 1024 * 1024;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn engine_conn_ledger_kills() {
    let url = mock_server(TOTAL, true);
    let dir = model::testenv::uniq_tmp_dir("ezr-h118-eng");
    let reg = dir.join("registry.json").to_string_lossy().into_owned();
    let mut a = App::new(Config::default(), reg);
    a.tasks
        .push(queued_task(1, &url, &dir.to_string_lossy(), 1));

    let mut guard = 0;
    loop {
        a.tick().await;
        if a.conn_cum_of(1, 1) > 0 && a.conn_speed_of(1, 1) > 0.0 {
            break;
        }
        if guard % 5 == 0 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        guard += 1;
        assert!(guard < 60_000, "下载未在预算内产生连接展示速度");
    }

    assert!(
        a.conn_speed_of(1, 1) > 0.0,
        "速度 EMA 已立起（窗口速率经每秒采样）"
    );
    let mid_rows = ui::testfx::row_strings(&ui::testfx::render_full(&mut a, 100, 24));
    let speed_visible = mid_rows[16..=18].iter().any(|r| r.contains("B/s"));
    assert!(
        speed_visible,
        "下载中并发明细数据行速度列应显数值（mutant 恒 '-'）：{:?}",
        &mid_rows[15..=21]
    );
    let mut probe = queued_task(1, "probe", "probe", 1);
    probe.state = TaskState::Downloading;
    probe.connections = vec![
        Connection {
            id: 1,
            start: 0,
            end: 100,
            done: 0,
        },
        Connection {
            id: 3,
            start: 0,
            end: 0,
            done: 0,
        },
        Connection {
            id: 2,
            start: 0,
            end: 100,
            done: 0,
        },
    ];
    assert_eq!(
        a.active_conn_count(&probe),
        2,
        "持块即活跃（Connected+Connecting）+ cap=0 不计（mutant count-all/filter 丢失 → 3/1）"
    );
    let mut probe_b = queued_task(1, "probeb", "probeb", 1);
    probe_b.state = TaskState::Downloading;
    probe_b.connections = vec![Connection {
        id: 1,
        start: 0,
        end: 0,
        done: 0,
    }];
    assert_eq!(
        a.active_conn_count(&probe_b),
        0,
        "cap=0 连接不计（cap 权威；mutant 忽 cap 判 cum → 1）"
    );
    let mut idle = queued_task(2, "idle", "idle", 1);
    idle.state = TaskState::Queued;
    assert_eq!(
        a.active_conn_count(&idle),
        0,
        "非明细态恒 0（靶 416:12 delete !）"
    );

    let cum1_before = a.conn_cum_of(1, 1);
    assert!(cum1_before > 0);
    a.clear_conn_stats(999);
    assert_eq!(
        a.conn_cum_of(1, 1),
        cum1_before,
        "清除他者 id 应保留累计账本（cum-retain mutant 清空）"
    );
    assert!(
        a.conn_speed_of(1, 1) > 0.0,
        "清除他者 id 应保留速度账本（speed-retain mutant 429:51 清空）"
    );

    let done = queued_task(2, "done", "done", 2);
    a.tasks.push(done);
    a.tasks[1].state = TaskState::Completed;
    a.clear_completed();
    assert_eq!(a.tasks.len(), 1, "已完成任务被清理");
    assert_eq!(
        a.conn_cum_of(1, 1),
        cum1_before,
        "下载中任务账本必须保留（mutant 收集臂反转后误清）"
    );

    a.shutdown().await;
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn gen_ucs_fixture_matches_product_source() {
    let src = std::fs::read_to_string("src/app/engine/mod.rs").expect("read product source");
    let fix =
        std::fs::read_to_string("tests/gen/update_conn_stats_stripped.rs").expect("read fixture");
    let extract = |text: &str| -> String {
        let mut out: Vec<&str> = Vec::new();
        let mut on = false;
        for line in text.lines() {
            if !on && line.starts_with("fn update_conn_stats(") {
                on = true;
            }
            if on {
                out.push(line);
                if line == "}" {
                    break;
                }
            }
        }
        assert!(on, "锚点 fn update_conn_stats 未找到");
        out.join("\n")
    };
    assert_eq!(
        extract(&src),
        extract(&fix),
        "剥头副本漂移：update_conn_stats_stripped.rs 与 src/app/engine/mod.rs 的 \
         update_conn_stats 不一致（重新生成副本）"
    );
}

mod ucs_fix {
    use std::collections::HashMap;
    use std::time::Instant;

    pub(crate) use crate::engine::ConnView;
    pub(crate) use crate::model::speed::{SpeedWindow, WINDOW};
    include!("gen/update_conn_stats_stripped.rs");

    #[allow(clippy::too_many_arguments)]
    pub fn call(
        prev: &mut HashMap<ConnKey, ConnObs>,
        cum: &mut HashMap<ConnKey, u64>,
        windows: &mut HashMap<ConnKey, SpeedWindow>,
        task_id: u32,
        conns: &[ConnView],
    ) {
        update_conn_stats(prev, cum, windows, task_id, conns)
    }
}

#[test]
fn update_conn_stats_pure_targets() {
    use std::collections::HashMap;

    use ucs_fix::{ConnView, SpeedWindow};
    let view = |block: u32, done: u64| ConnView {
        id: 1,
        block,
        start: 0,
        end: 4_000_000,
        done,
    };

    let mut prev = HashMap::new();
    let mut cum = HashMap::new();
    let mut windows: HashMap<(u32, usize), SpeedWindow> = HashMap::new();
    ucs_fix::call(&mut prev, &mut cum, &mut windows, 7, &[view(0, 100)]);
    std::thread::sleep(Duration::from_millis(100));
    ucs_fix::call(&mut prev, &mut cum, &mut windows, 7, &[view(0, 400)]);
    assert_eq!(cum.get(&(7, 1)), Some(&400u64), "累计量随增量累加");
    let r = windows.get_mut(&(7, 1)).unwrap().rate();
    assert!(
        (2_500.0..3_500.0).contains(&r),
        "窗口速率 = Δcum/Δt ≈ 3KB/s（push-读数突变 → 2KB/s）：{r}"
    );

    std::thread::sleep(Duration::from_millis(30));
    ucs_fix::call(
        &mut prev,
        &mut cum,
        &mut windows,
        7,
        &[ConnView {
            id: 1,
            block: 0,
            start: 0,
            end: 1_000,
            done: 1_000,
        }],
    );
    let r_idle = windows.get_mut(&(7, 1)).unwrap().rate();
    assert!(
        (2_500.0..3_500.0).contains(&r_idle),
        "待命连接窗口冻结（误推 → 速率稀释）：{r_idle}"
    );

    std::thread::sleep(Duration::from_millis(1_100));
    ucs_fix::call(&mut prev, &mut cum, &mut windows, 7, &[view(1, 200)]);
    let r_stale = windows.get_mut(&(7, 1)).unwrap().rate();
    assert_eq!(
        r_stale, 0.0,
        "老化清窗后单样本速率为 0（守卫删除 → 摊薄残留）: {r_stale}"
    );

    let mut prev_s = HashMap::new();
    let mut cum_s = HashMap::new();
    let mut win_s: HashMap<(u32, usize), SpeedWindow> = HashMap::new();
    let stream_view = |done: u64| ConnView {
        id: 1,
        block: 0,
        start: 0,
        end: 0,
        done,
    };
    ucs_fix::call(&mut prev_s, &mut cum_s, &mut win_s, 9, &[stream_view(100)]);
    std::thread::sleep(Duration::from_millis(100));
    ucs_fix::call(&mut prev_s, &mut cum_s, &mut win_s, 9, &[stream_view(400)]);
    assert_eq!(cum_s.get(&(9, 1)), Some(&400u64), "流式累计随增量累加");
    let r_stream = win_s.get_mut(&(9, 1)).unwrap().rate();
    assert!(
        (2_500.0..3_500.0).contains(&r_stream),
        "流式连接窗口速率 ≈ 3KB/s（原门恒 false → 无样本）: {r_stream}"
    );
    ucs_fix::call(
        &mut prev_s,
        &mut cum_s,
        &mut win_s,
        10,
        &[ConnView {
            id: 1,
            block: 0,
            start: 0,
            end: 0,
            done: 0,
        }],
    );
    assert!(
        !win_s.contains_key(&(10, 1)),
        "待命连接（done=0）不推进窗口"
    );
}

fn mock_server_streaming(total: u64, slow: bool) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut w = conn.try_clone().unwrap();
                let mut r = BufReader::new(conn);
                let mut line = String::new();
                if r.read_line(&mut line).is_err() || line.is_empty() {
                    return;
                }
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).is_err() || h.trim().is_empty() {
                        break;
                    }
                }
                let _ = write!(w, "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n");
                let mut buf = vec![0u8; 8192];
                let mut pos = 0;
                while pos < total {
                    let n = (total - pos).min(8192) as usize;
                    fill(&mut buf[..n], pos);
                    if w.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    if slow {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    pos += n as u64;
                }
            });
        }
    });
    format!("http://{addr}/f.bin")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streaming_single_thread_shows_in_conns_panel() {
    let url = mock_server_streaming(4 * 1024 * 1024, true);
    let dir = model::testenv::uniq_tmp_dir("ezr-v134-stream");
    let reg = dir.join("registry.json").to_string_lossy().into_owned();
    let mut a = App::new(Config::default(), reg);
    a.tasks
        .push(queued_task(1, &url, &dir.to_string_lossy(), 4));

    let mut guard = 0;
    loop {
        a.tick().await;
        if a.conn_cum_of(1, 1) > 0 {
            break;
        }
        if guard % 5 == 0 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        guard += 1;
        assert!(guard < 60_000, "流式下载未在预算内产生连接账本");
    }

    assert_eq!(a.active_conn_count(&a.tasks[0]), 1, "活跃并发 = 1");
    assert_eq!(a.conns_display_total(), 1, "头部并发计数 = 1");

    tokio::time::sleep(Duration::from_millis(1_100)).await;
    a.tick().await;
    assert!(
        a.conn_speed_of(1, 1) > 0.0,
        "流式单连接面板速度非零（旧实现恒 0）: {}",
        a.conn_speed_of(1, 1)
    );

    let rows = ui::testfx::row_strings(&ui::testfx::render_full(&mut a, 100, 24));
    assert!(
        rows.iter().any(|r| r.contains("1/1 项")),
        "徽标 1/1 项（原 0/4 项 = 恒空面板）: {rows:?}"
    );
    assert!(
        rows.iter()
            .any(|r| r.contains("   1 ") && !r.contains("序号")),
        "流式并发明细行 1 行（序号 1）: {rows:?}"
    );
}
