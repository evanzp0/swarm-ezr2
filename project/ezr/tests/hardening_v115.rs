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

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use app::App;
use crossterm::event::{KeyCode, KeyModifiers};
use model::config::{Config, DEFAULT_BLOCK_SIZE_HTTP};
use model::TaskState;
use ratatui::backend::TestBackend;

pub const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-01");

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn temp_reg() -> std::path::PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ezr-hard-v115-{}-{n}.json", std::process::id()))
}

fn render(app: &mut App, w: u16, h: u16) -> ratatui::buffer::Buffer {
    let mut term = ratatui::Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw(f, app)).unwrap();
    term.backend().buffer().clone()
}

fn cell(buf: &ratatui::buffer::Buffer, x: u16, y: u16) -> String {
    buf[(x, y)].symbol().to_string()
}

fn row_text(buf: &ratatui::buffer::Buffer, y: u16, w: u16) -> String {
    (0..w).map(|x| buf[(x, y)].symbol()).collect()
}

fn app_with_add_dialog() -> (App, std::path::PathBuf) {
    let reg = temp_reg();
    let mut a = App::new(Config::default(), reg.to_string_lossy().into_owned());
    a.on_key(KeyCode::Char('a'), KeyModifiers::empty());
    assert!(a.cmd_open, "'a' 应打开命令窗口（FR-02-04①）");
    a.cmd.set_text("https://example.com/f.bin");
    a.on_key(KeyCode::Enter, KeyModifiers::empty());
    assert!(!a.cmd_open, "确认后命令窗口关闭（D44 互斥）");
    assert!(a.dialog.is_some(), "http(s) 分流应预填打开添加对话框");
    a.dialog.as_mut().unwrap().url.set("");
    (a, reg)
}

#[tokio::test]
async fn field_display_exact_width_shows_full_value() {
    let (mut a, reg) = app_with_add_dialog();
    {
        let d = a.dialog.as_mut().unwrap();
        d.url.set("x".repeat(59));
        d.focus = 3;
    }
    let buf = render(&mut a, 120, 40);
    assert_eq!(cell(&buf, 35, 16), "x", "恰宽值首字符必须原样显示");
    assert_eq!(cell(&buf, 93, 16), "x", "恰宽值末字符（35+58）");
    assert_ne!(cell(&buf, 35, 16), "…", "不得进入尾部省略分支");
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn add_dialog_at_min_inner_width_renders() {
    let (mut a, reg) = app_with_add_dialog();
    let buf = render(&mut a, 26, 40);
    let joined: String = (0..40).map(|y| row_text(&buf, y, 26)).collect();
    assert!(
        joined.contains("URL"),
        "inner.width==24 边界必须渲染（守卫为 <）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn add_dialog_below_min_width_renders_nothing() {
    let (mut a, reg) = app_with_add_dialog();
    let buf = render(&mut a, 24, 40);
    let joined: String = (0..40).map(|y| row_text(&buf, y, 24)).collect();
    assert!(!joined.contains("URL"), "宽 22 < 24 必须早退（|| 守卫）");
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn add_dialog_field_rows_exact_geometry() {
    let (mut a, reg) = app_with_add_dialog();
    let buf = render(&mut a, 120, 40);
    assert!(row_text(&buf, 16, 120).contains("URL"), "URL 行 y=16");
    assert_eq!(cell(&buf, 26, 17), "保", "保存到行 y=17、label 自 x=26");
    assert_eq!(cell(&buf, 26, 18), "并", "并发行 y=18");
    assert_eq!(cell(&buf, 26, 19), "校", "校验行 y=19");
    assert!(
        row_text(&buf, 19, 120).contains("NONE"),
        "校验行显示默认 NONE（v1.32/FR-01-01：NONE 首行默认）"
    );
    assert_eq!(cell(&buf, 26, 16), "U", "URL label 首字符恰在 x=26");
    for y in 14..26 {
        assert!(
            !row_text(&buf, y, 120).contains('▏'),
            "内联字形光标已退役（行 {y}）"
        );
    }
    assert_eq!(
        a.dlg_cursor_screen,
        Some((35, 16)),
        "聚焦文本字段原生光标屏幕位（值区起点）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn add_dialog_button_and_hint_rows_exact_geometry() {
    let (mut a, reg) = app_with_add_dialog();
    let buf = render(&mut a, 120, 40);
    assert!(
        !a.dlg_btn_rects.is_empty() && a.dlg_btn_rects.iter().all(|(r, _)| r.y == 23),
        "三个按钮的命中区 y 必须恰为 23（got {:?}）",
        a.dlg_btn_rects.iter().map(|(r, _)| r.y).collect::<Vec<_>>()
    );
    assert_eq!(a.dlg_btn_rects.len(), 3, "三钮（FR-01-103）");
    assert!(row_text(&buf, 23, 120).contains("立"), "按钮行 y=23");
    assert!(!row_text(&buf, 22, 120).contains("立"), "按钮不得上移");
    assert!(
        !row_text(&buf, 27, 120).contains("立"),
        "按钮不得下移（n_rows*2 变异）"
    );
    assert!(
        !row_text(&buf, 24, 120).contains("Enter"),
        "底部提示行退役（v1.30）: {:?}",
        row_text(&buf, 24, 120)
    );
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn ck_dropdown_exact_geometry() {
    let (mut a, reg) = app_with_add_dialog();
    {
        let d = a.dialog.as_mut().unwrap();
        d.focus = 3;
        d.ck_open = true;
    }
    let buf = render(&mut a, 120, 21);
    assert_eq!(
        cell(&buf, 33, 10),
        "╭",
        "下拉框左上角必须在其 x=33、y=10（未越界不翻转）"
    );
    assert!(
        row_text(&buf, 11, 120).contains("NONE"),
        "选项 0（NONE）行 y=11"
    );
    assert!(row_text(&buf, 12, 120).contains("MD5"), "选项 1 行 y=12");
    assert!(row_text(&buf, 13, 120).contains("SHA-1"), "选项 2 行 y=13");
    assert!(
        row_text(&buf, 18, 120).contains("Adler-32"),
        "选项 7 行 y=18（8 项末行）"
    );
    assert_eq!(
        cell(&buf, 33, 19),
        "╰",
        "下拉框左下角 y=19（ph=10 收口于终端底上一行）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn proxy_dropdown_single_item_exact_geometry() {
    let (mut a, reg) = app_with_add_dialog();
    {
        let d = a.dialog.as_mut().unwrap();
        d.focus = 5;
        d.proxy_open = true;
    }
    let buf = render(&mut a, 120, 40);
    assert_eq!(cell(&buf, 33, 22), "╭", "代理下拉左上角 (33,22)");
    assert!(row_text(&buf, 23, 120).contains("直"), "唯一选项行 y=23");
    assert_eq!(cell(&buf, 33, 24), "╰", "代理下拉左下角 (33,24)（ph=3）");
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

#[test]
fn config_keeps_positive_block_size_http() {
    let cfg = Config::from_toml("block_size_http = 2097152\n");
    assert_eq!(
        cfg.block_size_http, 2_097_152,
        "显式正值 block_size_http 必须保留"
    );
    let cfg0 = Config::from_toml("block_size_http = 0\n");
    assert_eq!(
        cfg0.block_size_http, DEFAULT_BLOCK_SIZE_HTTP,
        "0 值回退默认（负对照：过滤参数确在生效）"
    );
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
                let _ = c.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                );
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

#[tokio::test]
async fn shutdown_pauses_downloading_task() {
    let addr = stub_trickle();
    let reg = temp_reg();
    let dir = std::env::temp_dir().join(format!("ezr-hard-v115-dir-{}", std::process::id()));
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
    assert_eq!(a.tasks[0].state, TaskState::Downloading, "前置：进入下载中");
    a.shutdown().await;
    let sc = dir.join("f.bin.ezr");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !sc.exists() {
        a.tick().await;
        assert!(
            std::time::Instant::now() < deadline,
            "shutdown 必须暂停下载中任务并把 sidecar 写到 {}",
            sc.display()
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(reg.exists(), "shutdown 必须保存注册表到 {}", reg.display());
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

fn stub_always_500() -> std::net::SocketAddr {
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
                let _ = c.write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                let _ = c.flush();
            });
        }
    });
    addr
}

#[tokio::test]
async fn transient_fail_at_limit_reports_max_retries() {
    let addr = stub_always_500();
    let reg = temp_reg();
    let dir = std::env::temp_dir().join(format!("ezr-hard-v115-r500-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = Config {
        max_retries: 1,
        ..Config::default()
    };
    let mut a = App::new(cfg, reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        a.tick().await;
        let toast = a.toast.clone().unwrap_or_default();
        let t = &a.tasks[0];
        if (t.state == TaskState::Failed && t.error.is_some()) || toast.contains("已达") {
            assert!(
                toast.contains("已达最大重试次数"),
                "at_limit 文案必须进 toast 且含「已达最大重试次数」（got: {toast}）"
            );
            assert!(toast.contains("手动重试"), "at_limit 文案提示手动重试");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "20s 内应到达失败终态（实际 {:?}）",
            a.tasks[0].state
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

const ETAG: &str = "W/\"ezr-hard-v115\"";

fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251 + 1) as u8).collect()
}

fn stream_slice(
    c: &mut std::net::TcpStream,
    data: &[u8],
    from: usize,
    to: usize,
    throttle: (usize, u64),
    slow: bool,
) {
    use std::io::Write;
    let (chunk, delay_ms) = if slow { (100, 50) } else { throttle };
    let mut off = from;
    while off < to {
        let end = if chunk == 0 {
            to
        } else {
            (off + chunk).min(to)
        };
        if delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }
        if c.write_all(&data[off..end]).is_err() {
            break;
        }
        let _ = c.flush();
        off = end;
    }
}

fn stub_http(
    len: usize,
    throttle: (usize, u64),
    slow_first: Option<usize>,
    deny: Option<Arc<AtomicBool>>,
) -> (std::net::SocketAddr, Arc<Vec<u8>>) {
    let data: Arc<Vec<u8>> = Arc::new(pattern(len));
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let conn_n = Arc::new(AtomicUsize::new(0));
    let data_loop = Arc::clone(&data);
    let conn_n_loop = Arc::clone(&conn_n);
    let deny_loop = deny.clone();
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let data = Arc::clone(&data_loop);
            let conn_n = Arc::clone(&conn_n_loop);
            let deny = deny_loop.clone();
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader, Write};
                let mut c = conn;
                let n = conn_n.fetch_add(1, Ordering::SeqCst);
                let r = BufReader::new(c.try_clone().unwrap());
                let mut range = String::new();
                for ok in r.lines() {
                    let l = match ok {
                        Ok(l) => l,
                        Err(_) => break,
                    };
                    let t = l.trim().to_string();
                    if t.is_empty() {
                        break;
                    }
                    if let Some(v) = t.to_ascii_lowercase().strip_prefix("range:") {
                        range = v.trim().to_string();
                    }
                }
                if range == "bytes=0-" {
                    let head = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Length: {len}\r\nAccept-Ranges: bytes\r\nETag: {ETAG}\r\nConnection: close\r\n\r\n"
                    );
                    let _ = c.write_all(head.as_bytes());
                    stream_slice(&mut c, &data, 0, len, throttle, false);
                    return;
                }
                if !range.is_empty() {
                    if deny.as_ref().is_some_and(|f| f.load(Ordering::SeqCst)) {
                        let _ = c.write_all(
                            b"HTTP/1.1 403 Forbidden\r\nContent-Length: 9\r\nConnection: close\r\n\r\nforbidden",
                        );
                        return;
                    }
                    let (start, end) = match range.trim_start_matches("bytes=").split_once('-') {
                        Some((a, b)) => (
                            a.parse::<usize>().unwrap_or(0),
                            b.parse::<usize>().map(|e| (e + 1).min(len)).unwrap_or(len),
                        ),
                        None => (0, len),
                    };
                    let head = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: {ETAG}\r\nConnection: close\r\n\r\n",
                        end.saturating_sub(start)
                    );
                    let _ = c.write_all(head.as_bytes());
                    stream_slice(
                        &mut c,
                        &data,
                        start,
                        end,
                        throttle,
                        slow_first.is_some_and(|_| n == 1),
                    );
                    return;
                }
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {len}\r\nETag: {ETAG}\r\nConnection: close\r\n\r\n"
                );
                let _ = c.write_all(head.as_bytes());
                stream_slice(&mut c, &data, 0, len, throttle, false);
            });
        }
    });
    (addr, data)
}

fn stub_ghost_trickle() -> std::net::SocketAddr {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader, Write};
                let mut c = conn;
                let r = BufReader::new(c.try_clone().unwrap());
                for line in r.lines() {
                    match line {
                        Ok(l) if l.is_empty() => break,
                        Ok(_) => {}
                        Err(_) => return,
                    }
                }
                let _ = c.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n");
                for _ in 0..3000 {
                    if c.write_all(&[9u8; 200]).is_err() {
                        return;
                    }
                    let _ = c.flush();
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            });
        }
    });
    addr
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("ezr-hard-v115-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

async fn wait_until(a: &mut App, secs: u64, mut pred: impl FnMut(&App) -> bool) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    loop {
        a.tick().await;
        if pred(a) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    }
}

async fn pause_and_wait_sidecar(a: &mut App, sc: &std::path::Path) {
    a.on_key(KeyCode::Char(' '), KeyModifiers::empty());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    while !sc.exists() {
        a.tick().await;
        assert!(
            std::time::Instant::now() < deadline,
            "暂停后 8s 内必须落 sidecar 到 {}",
            sc.display()
        );
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    }
}

#[tokio::test]
async fn dialog_clamps_out_of_range_ck_type() {
    let (mut a, reg) = app_with_add_dialog();
    {
        let d = a.dialog.as_mut().unwrap();
        d.ck_type = app::CHECKSUM_ALGOS.len();
    }
    let buf = render(&mut a, 120, 40);
    assert!(
        row_text(&buf, 19, 120).contains("Adler-32"),
        "越界 ck_type 必须钳制显示末项算法（got {:?}）",
        row_text(&buf, 19, 120).trim()
    );
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn named_dead_proxy_fails_probe() {
    let (addr, _data) = stub_http(64 * 1024, (0, 0), None, None);
    let cfg = Config::from_toml(
        "[[proxies]]\nname = \"dead\"\ntype = \"http\"\nip = \"127.0.0.1\"\nport = 9\n",
    );
    let reg = temp_reg();
    let mut a = App::new(cfg, reg.to_string_lossy().into_owned());
    a.on_key(KeyCode::Char('a'), KeyModifiers::empty());
    assert!(a.cmd_open, "'a' 应打开命令窗口（FR-02-04①）");
    a.cmd.set_text("https://placeholder/f.bin");
    a.on_key(KeyCode::Enter, KeyModifiers::empty());
    {
        let d = a.dialog.as_mut().unwrap();
        d.url.set(format!("http://{addr}/f.bin"));
        d.proxy_sel = 1;
    }
    a.on_key(KeyCode::Enter, KeyModifiers::empty());
    assert!(a.dialog.is_none(), "URL 已填 + Enter 应确认建任务");
    assert_eq!(a.tasks[0].state, TaskState::Queued, "任务已入队");
    let ok = wait_until(&mut a, 12, |a| a.tasks[0].state == TaskState::Failed).await;
    assert!(
        ok,
        "死代理必须让探测失败（实际状态 {:?}）",
        a.tasks[0].state
    );
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn speed_window_tracks_downloading_task() {
    let addr = stub_trickle();
    let reg = temp_reg();
    let mut a = App::new(Config::default(), reg.to_string_lossy().into_owned());
    a.add_cli_task(format!("http://{addr}/f.bin"), None, Some(1), None);
    assert!(
        wait_until(&mut a, 20, |a| a.tasks[0].state == TaskState::Downloading).await,
        "滴流下载应进入下载中"
    );
    tokio::time::sleep(std::time::Duration::from_millis(1700)).await;
    a.tick().await;
    assert!(
        a.tasks[0].speed > 100.0,
        "下载中任务必须有非零速度（滴流 ≈2000B/s，got {}）",
        a.tasks[0].speed
    );
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn single_conn_download_completes() {
    let len = 2 * 1024 * 1024;
    let (addr, data) = stub_http(len, (0, 0), None, None);
    let reg = temp_reg();
    let dir = temp_dir("sconn");
    let mut a = App::new(
        Config::from_toml("block_size_http = 1048576\n"),
        reg.to_string_lossy().into_owned(),
    );
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let ok = wait_until(&mut a, 30, |a| a.tasks[0].state == TaskState::Completed).await;
    assert!(ok, "单连接下载必须完成（实际状态 {:?}）", a.tasks[0].state);
    assert_eq!(a.tasks[0].downloaded, len as u64, "下载字节数必须齐全");
    let file = std::fs::read(dir.join("f.bin")).expect("完成态文件应就位");
    assert_eq!(file, *data, "文件内容必须与服务器一致");
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn range_denied_fails_task() {
    let deny = Arc::new(AtomicBool::new(true));
    let (addr, _data) = stub_http(2 * 1024 * 1024, (0, 0), None, Some(deny));
    let reg = temp_reg();
    let dir = temp_dir("r403");
    let mut a = App::new(
        Config::from_toml("block_size_http = 1048576\n"),
        reg.to_string_lossy().into_owned(),
    );
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let ok = wait_until(&mut a, 20, |a| a.tasks[0].state == TaskState::Failed).await;
    assert!(
        ok,
        "块请求 403 必须让任务失败（实际状态 {:?}）",
        a.tasks[0].state
    );
    assert_eq!(
        a.tasks[0].chunk_done, 0,
        "失败汇总不得把部分写块计入已完成块（completed_of 整块口径）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn sidecar_flush_waits_interval() {
    let addr = stub_trickle();
    let reg = temp_reg();
    let dir = temp_dir("flush");
    let mut a = App::new(
        Config::from_toml("block_size_http = 16384\n"),
        reg.to_string_lossy().into_owned(),
    );
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    assert!(
        wait_until(&mut a, 20, |a| a.tasks[0].state == TaskState::Downloading).await,
        "滴流下载应进入下载中"
    );
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    a.tick().await;
    assert!(
        !dir.join("f.bin.ezr").exists(),
        "周期未到不得写 sidecar（下载仅开始约 1.2s，期限 2s）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn pause_freezes_file_bytes_on_disk() {
    let addr = stub_trickle();
    let reg = temp_reg();
    let dir = temp_dir("pfrz");
    let mut a = App::new(
        Config::from_toml("block_size_http = 16384\n"),
        reg.to_string_lossy().into_owned(),
    );
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    assert!(
        wait_until(&mut a, 20, |a| a.tasks[0].state == TaskState::Downloading).await,
        "滴流下载应进入下载中"
    );
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    let file = dir.join("f.bin.downloading");
    pause_and_wait_sidecar(&mut a, &dir.join("f.bin.ezr")).await;
    let len0 = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    a.tick().await;
    let len1 = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
    assert!(
        len1.saturating_sub(len0) < 1000,
        "暂停后磁盘文件不得继续增长（{len0} → {len1}；stop_workers no-op 变异体 1.5s 增 ~3000B）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn conn_views_have_unique_ids() {
    let (addr, _data) = stub_http(2 * 1024 * 1024, (64 * 1024, 30), None, None);
    let reg = temp_reg();
    let dir = temp_dir("conns");
    let mut a = App::new(
        Config::from_toml("block_size_http = 1048576\n"),
        reg.to_string_lossy().into_owned(),
    );
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(2),
        None,
    );
    let mut saw_downloading = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
    loop {
        a.tick().await;
        let st = a.tasks[0].state;
        if st == TaskState::Downloading {
            saw_downloading = true;
            let ids: Vec<usize> = a.tasks[0].connections.iter().map(|c| c.id).collect();
            let mut srt = ids.clone();
            srt.sort_unstable();
            srt.dedup();
            assert_eq!(srt.len(), ids.len(), "连接视图 id 不得重复（got {ids:?}）");
        }
        if matches!(st, TaskState::Completed | TaskState::Failed)
            || std::time::Instant::now() >= deadline
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(saw_downloading, "应进入下载中并产出连接视图");
    assert_eq!(a.tasks[0].state, TaskState::Completed, "多连接下载应完成");
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn pause_mid_block_resume_content_intact() {
    let len = 2 * 1024 * 1024;
    let (addr, data) = stub_http(len, (64 * 1024, 30), None, None);
    let reg = temp_reg();
    let dir = temp_dir("resm");
    let mut a = App::new(
        Config::from_toml("block_size_http = 1048576\n"),
        reg.to_string_lossy().into_owned(),
    );
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    assert!(
        wait_until(&mut a, 30, |a| a.tasks[0].state == TaskState::Downloading).await,
        "限速滴流应进入下载中"
    );
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    a.tick().await;
    pause_and_wait_sidecar(&mut a, &dir.join("f.bin.ezr")).await;
    a.on_key(KeyCode::Char(' '), KeyModifiers::empty());
    let ok = wait_until(&mut a, 40, |a| {
        matches!(a.tasks[0].state, TaskState::Completed | TaskState::Failed)
    })
    .await;
    assert!(ok, "恢复后应到达终态（实际 {:?}）", a.tasks[0].state);
    assert_eq!(a.tasks[0].state, TaskState::Completed, "恢复后应完成");
    let file = std::fs::read(dir.join("f.bin")).expect("完成态文件应就位");
    assert_eq!(
        file, *data,
        "暂停续传后文件内容必须完整（中断块被误标完成会留下零洞）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn failed_reports_completed_blocks_of_sidecar() {
    let deny = Arc::new(AtomicBool::new(false));
    let (addr, _data) = stub_http(
        2 * 1024 * 1024,
        (64 * 1024, 30),
        None,
        Some(Arc::clone(&deny)),
    );
    let reg = temp_reg();
    let dir = temp_dir("cdof");
    let mut a = App::new(
        Config::from_toml("block_size_http = 1048576\n"),
        reg.to_string_lossy().into_owned(),
    );
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    assert!(
        wait_until(&mut a, 30, |a| a.tasks[0].state == TaskState::Downloading).await,
        "限速下载应进入下载中"
    );
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    a.tick().await;
    pause_and_wait_sidecar(&mut a, &dir.join("f.bin.ezr")).await;
    deny.store(true, Ordering::SeqCst);
    a.on_key(KeyCode::Char(' '), KeyModifiers::empty());
    let ok = wait_until(&mut a, 20, |a| a.tasks[0].state == TaskState::Failed).await;
    assert!(
        ok,
        "恢复后块请求 403 必须失败（实际状态 {:?}）",
        a.tasks[0].state
    );
    assert_eq!(
        a.tasks[0].chunk_done, 0,
        "Failed 汇总的已完成块数按整块口径：单块任务部分写 → 0（>= → < 变异体会计成 1）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn unknown_size_resume_restarts() {
    let addr = stub_ghost_trickle();
    let reg = temp_reg();
    let dir = temp_dir("ghost");
    let mut a = App::new(Config::default(), reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    assert!(
        wait_until(&mut a, 20, |a| a.tasks[0].state == TaskState::Downloading).await,
        "未知大小滴流应进入下载中"
    );
    assert!(
        wait_until(&mut a, 25, |a| a.tasks[0].downloaded >= 2048).await,
        "应在 25s 内积累 2KB"
    );
    let paused = a.tasks[0].downloaded;
    pause_and_wait_sidecar(&mut a, &dir.join("f.bin.ezr")).await;
    a.on_key(KeyCode::Char(' '), KeyModifiers::empty());
    let mut dipped = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        a.tick().await;
        if a.tasks[0].downloaded < paused {
            dipped = true;
            break;
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    }
    assert!(
        dipped,
        "未知大小任务恢复后必须从头重下（downloaded 应回落到 {paused} 之下；Some(0) 变异体会按 0 块假续传卡死不前）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

fn rewrite_block_count(raw: &str, count: u32) -> String {
    let key = "\"block_count\":";
    let start = raw.find(key).expect("sidecar 缺 block_count 字段");
    let after = start + key.len();
    let end = raw[after..].find(',').expect("block_count 后应有逗号") + after;
    let out = format!("{}{}{}", &raw[..after], count, &raw[end..]);
    let bkey = "\"blocks\":";
    let bs = out.find(bkey).expect("sidecar 缺 blocks 字段");
    let bs_after = bs + bkey.len();
    let close = out[bs_after..].find(']').expect("blocks 数组未闭合") + bs_after;
    let zeros: Vec<&str> = (0..count).map(|_| "0").collect();
    format!(
        "{}[{}]{}",
        &out[..bs_after],
        zeros.join(","),
        &out[close + 1..]
    )
}

#[tokio::test]
async fn tampered_block_count_restarts_fresh() {
    let (addr, data) = stub_http(1000, (10, 20), None, None);
    let reg = temp_reg();
    let dir = temp_dir("bcnt");
    let mut a = App::new(
        Config::from_toml("block_size_http = 100\n"),
        reg.to_string_lossy().into_owned(),
    );
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    assert!(
        wait_until(&mut a, 20, |a| a.tasks[0].state == TaskState::Downloading).await,
        "限速下载应进入下载中"
    );
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    pause_and_wait_sidecar(&mut a, &dir.join("f.bin.ezr")).await;
    let sc_path = dir.join("f.bin.ezr");
    let raw = std::fs::read_to_string(&sc_path).expect("sidecar 应可读");
    std::fs::write(&sc_path, rewrite_block_count(&raw, 3)).expect("sidecar 应可写");
    a.on_key(KeyCode::Char(' '), KeyModifiers::empty());
    let ok = wait_until(&mut a, 30, |a| {
        matches!(a.tasks[0].state, TaskState::Completed | TaskState::Failed)
    })
    .await;
    assert!(
        ok,
        "块数被破坏的断点必须到达终态（实际 {:?}）",
        a.tasks[0].state
    );
    assert_eq!(
        a.tasks[0].state,
        TaskState::Completed,
        "应作废断点从头重下并完成"
    );
    let file = std::fs::read(dir.join("f.bin")).expect("完成态文件应就位");
    assert_eq!(
        file, *data,
        "块数记账损坏必须触发 Invalidated 重下（|| 变异体会假续传 3 块只写 300 字节）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn zero_size_206_reports_not_resumable() {
    let (addr, _data) = stub_http(0, (0, 0), None, None);
    let reg = temp_reg();
    let dir = temp_dir("zero");
    let mut a = App::new(Config::default(), reg.to_string_lossy().into_owned());
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        None,
    );
    let ok = wait_until(&mut a, 20, |a| {
        matches!(a.tasks[0].state, TaskState::Completed | TaskState::Failed)
    })
    .await;
    assert!(ok, "0 字节任务应到达终态（实际 {:?}）", a.tasks[0].state);
    assert_eq!(a.tasks[0].state, TaskState::Completed, "0 字节下载应完成");
    assert!(
        !a.tasks[0].resumable,
        "total=0 的 206 探测不得标记可续传（> → >= 变异体会误标 true）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn proxy_dropdown_left_clamp_at_narrow_width() {
    let (mut a, reg) = app_with_add_dialog();
    {
        let d = a.dialog.as_mut().unwrap();
        d.focus = 5;
        d.proxy_open = true;
    }
    let buf = render(&mut a, 27, 40);
    assert_eq!(
        cell(&buf, 1, 22),
        "╭",
        "窄端代理下拉左上角必须在 (1,22)（px = area.x + 1 = 1）"
    );
    assert_eq!(
        cell(&buf, 0, 22),
        "│",
        "x=0 必须仍是对话框左边框（px 下限钳制留 1 列边距；+ → * 变异体贴边覆盖为 ╭）"
    );
    assert!(row_text(&buf, 23, 27).contains("直"), "唯一选项行仍在 y=23");
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn proxy_dropdown_no_panic_at_26_cols() {
    let (mut a, reg) = app_with_add_dialog();
    {
        let d = a.dialog.as_mut().unwrap();
        d.focus = 5;
        d.proxy_open = true;
    }
    let buf = render(&mut a, 26, 40);
    assert_eq!(cell(&buf, 1, 22), "╭", "下拉左上角仍在 (1,22)");
    assert_eq!(cell(&buf, 0, 22), "│", "x=0 仍是对话框左边框");
    assert!(row_text(&buf, 23, 26).contains("直"), "唯一选项行仍在 y=23");
    assert_eq!(cell(&buf, 25, 22), "╮", "浮层右缘钳在末列 x=25");
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn proxy_dropdown_no_panic_when_taller_than_terminal() {
    let mut cfg = Config::default();
    for i in 0..12 {
        cfg.proxies.push(model::config::ProxyConfig {
            name: format!("p{i}"),
            kind: model::config::ProxyKind::Http,
            ip: "127.0.0.1".to_string(),
            port: 8080,
            username: None,
            password: None,
        });
    }
    let reg = temp_reg();
    let mut a = App::new(cfg, reg.to_string_lossy().into_owned());
    a.on_key(KeyCode::Char('a'), KeyModifiers::empty());
    assert!(a.cmd_open, "'a' 应打开命令窗口（FR-02-04①）");
    a.cmd.set_text("https://example.com/f.bin");
    a.on_key(KeyCode::Enter, KeyModifiers::empty());
    {
        let d = a.dialog.as_mut().unwrap();
        d.focus = 5;
        d.proxy_open = true;
    }
    let buf = render(&mut a, 80, 12);
    assert_eq!(
        cell(&buf, 13, 0),
        "╭",
        "上移展开后浮层顶在 y=0（px=inner.x+9）"
    );
    assert_eq!(cell(&buf, 13, 11), "╰", "浮层下缘钳在末行 y=11");
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

#[tokio::test]
async fn verify_failed_cleared_checksum_completes_directly_without_redownload() {
    let len = 2 * 1024 * 1024;
    let (addr, data) = stub_http(len, (0, 0), None, None);
    let reg = temp_reg();
    let dir = temp_dir("cleared-ck");
    let mut a = App::new(
        Config::from_toml("block_size_http = 1048576\n"),
        reg.to_string_lossy().into_owned(),
    );
    a.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(dir.to_string_lossy().into_owned()),
        Some(1),
        Some(model::Checksum {
            algo: "SHA-256",
            value: "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff".into(),
        }),
    );
    a.selected = 0;
    let ok = wait_until(&mut a, 30, |a| a.tasks[0].state == TaskState::Failed).await;
    assert!(
        ok,
        "错误校验值必须让任务落「已失败」（实际状态 {:?}）",
        a.tasks[0].state
    );
    let sc_path = dir.join("f.bin.ezr");
    let sc = model::sidecar::Sidecar::load(&sc_path.to_string_lossy())
        .expect("完成点必须已落终态 sidecar（修订 A：旧实现快速下载从未落盘）");
    assert_eq!(
        sc.downloaded, len as u64,
        "终态 sidecar 台账必须全块完整（修订 A）"
    );
    a.tasks[0].checksum = None;
    a.on_key(KeyCode::Char('r'), KeyModifiers::empty());
    assert_eq!(
        a.tasks[0].state,
        TaskState::Completed,
        "文件完整必须不经引擎直接收尾（修订 B：旧实现落 Queued 经引擎回退重传）"
    );
    assert!(
        a.toast.as_deref().is_some_and(|t| t.contains("无校验")),
        "toast 必须含「无校验」（got {:?}）",
        a.toast
    );
    let file = std::fs::read(dir.join("f.bin")).expect("目标文件应就位");
    assert_eq!(file, *data, "收尾后目标文件内容与服务器一致");
    assert!(
        !dir.join("f.bin.downloading").exists(),
        ".downloading 已改名"
    );
    assert!(!sc_path.exists(), "sidecar 已删");
    assert!(a.tasks[0].verify_ok.is_none(), "无校验完成 verify_ok=None");
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}
