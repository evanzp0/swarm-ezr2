//! 加固测试（six-pack/hardender v115 轮）：supervisor / dialog / config / engine
//! 存活突变体处置。一测一靶：每条测试对应一个存活位点或一类几何/行为契约，
//! 断言精确值（坐标 / 边界 / 文案），不使用子串宽松匹配以外的模糊判据。
//!
//! 靶点索引（cargo-mutants v115 轮 MISSED 清单，位点 = 文件:行:列）：
//! - ui/dialog.rs 19:22      field_display 恰宽边界            → field_display_exact_width_shows_full_value
//! - ui/dialog.rs 187:20 ×2  内框最小宽边界（<24）             → add_dialog_at_min_inner_width_renders
//! - ui/dialog.rs 187:25     最小宽/高守卫 ||→&&               → add_dialog_below_min_width_renders_nothing
//! - ui/dialog.rs 259:29/33  字段行 row_y 算术 ×4              → add_dialog_field_rows_exact_geometry
//! - ui/dialog.rs 258:29     焦点判定 ==→!=                    → add_dialog_field_rows_exact_geometry
//! - ui/dialog.rs 314:24     字段行 rect.x 算术 ×2             → add_dialog_field_rows_exact_geometry
//! - ui/dialog.rs 330:17/26  按钮行 y 算术 ×3                  → add_dialog_button_and_hint_rows_exact_geometry
//! - ui/dialog.rs 351:24     提示行 x 算术 ×2                  → add_dialog_button_and_hint_rows_exact_geometry
//! - ui/dialog.rs 352:33     提示行 y 算术 ×2                  → add_dialog_button_and_hint_rows_exact_geometry
//! - ui/dialog.rs 36:16      下拉越界翻转边界（py+ph==底）      → ck_dropdown_exact_geometry
//! - ui/dialog.rs 41:21      下拉 x 定位算术                   → ck_dropdown_exact_geometry
//! - ui/dialog.rs 123:21     下拉选项行 y 算术                 → ck_dropdown_exact_geometry
//! - ui/dialog.rs 363:37     算法下拉高度 ph=len+2             → ck_dropdown_exact_geometry
//! - ui/dialog.rs 382:37     代理下拉高度 ph=len+2（单项）      → proxy_dropdown_single_item_exact_geometry
//! - model/config.rs 312:32  block_size_http 正值过滤          → config_keeps_positive_block_size_http
//! - app/engine.rs 622:9     shutdown 整体 no-op               → shutdown_pauses_downloading_task
//! - app/engine.rs 625:33    shutdown 过滤 ==→!=               → shutdown_pauses_downloading_task
//! - app/engine.rs 585:13    at_limit 分支删除                 → transient_fail_at_limit_reports_max_retries
//!
//! 等价/受限登记不在本文件断言（见 project/handoff.md 等价突变体清单）。
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
#[path = "../src/ui/mod.rs"]
mod ui;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use app::App;
use crossterm::event::{KeyCode, KeyModifiers};
use model::config::{Config, DEFAULT_BLOCK_SIZE_HTTP};
use model::TaskState;
use ratatui::backend::TestBackend;

/// ui/header.rs 引用 `crate::VERSION`（产品内定义于 src/main.rs）：
/// 挂载测试 crate 根部提供同口径常量（逐字复用产品表达式）。
pub const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-01");

static SEQ: AtomicUsize = AtomicUsize::new(0);

/// 唯一临时注册表路径（pid + 进程内自增序号，防并发/跨会话复用）
fn temp_reg() -> std::path::PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ezr-hard-v115-{}-{n}.json", std::process::id()))
}

/// 渲染 App 全 UI 到指定尺寸的 TestBackend 并返回缓冲区快照
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

/// 打开添加对话框（默认焦点 URL 行），返回 (App, 注册表路径)
fn app_with_add_dialog() -> (App, std::path::PathBuf) {
    let reg = temp_reg();
    let mut a = App::new(Config::default(), reg.to_string_lossy().into_owned());
    a.on_key(KeyCode::Char('a'), KeyModifiers::empty());
    assert!(a.dialog.is_some(), "'a' 应打开添加对话框");
    (a, reg)
}

// ---------------------------------------------------------------------------
// ui/dialog.rs —— field_display 恰宽边界（19:22）
// ---------------------------------------------------------------------------

/// 120×40 终端：Add 对话框 dlg=(23,14,74,12)，inner=(24,15,72,10)，
/// avail = 72-12 = 60。URL 值显示宽度恰为 60 时必须走"原样显示"分支
/// （`w(val) > avail` 为假）；`>=` 变异体会改走尾部省略分支（首字符变 …）。
#[tokio::test]
async fn field_display_exact_width_shows_full_value() {
    let (mut a, reg) = app_with_add_dialog();
    {
        let d = a.dialog.as_mut().unwrap();
        d.url = "x".repeat(60); // 恰等于 avail
    }
    let buf = render(&mut a, 120, 40);
    // 字段行 rect.x = inner.x+1 = 25；label 7 列 + "> " 2 列 → 值自 x=34 起
    assert_eq!(cell(&buf, 34, 16), "x", "恰宽值首字符必须原样显示");
    assert_eq!(cell(&buf, 93, 16), "x", "恰宽值末字符（34+59）");
    assert_ne!(cell(&buf, 34, 16), "…", "不得进入尾部省略分支");
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

// ---------------------------------------------------------------------------
// ui/dialog.rs —— 内框最小尺寸守卫（187:20 ×2、187:25）
// ---------------------------------------------------------------------------

/// 终端宽 26 → dlg 宽被钳到 26，inner.width 恰为 24（边界 `24 < 24` 为假）
/// → 必须正常渲染字段行；`<=`/`==` 变异体会整体早退不渲染。
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

/// 终端宽 24 → inner.width=22 < 24 且 inner.height=10 ≥ min_h(10)：
/// `||` 守卫左臂独真即早退 → 全屏不得出现任何字段行；`&&` 变异体会继续渲染。
#[tokio::test]
async fn add_dialog_below_min_width_renders_nothing() {
    let (mut a, reg) = app_with_add_dialog();
    let buf = render(&mut a, 24, 40);
    let joined: String = (0..40).map(|y| row_text(&buf, y, 24)).collect();
    assert!(!joined.contains("URL"), "宽 22 < 24 必须早退（|| 守卫）");
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

// ---------------------------------------------------------------------------
// ui/dialog.rs —— 字段行几何与焦点（259:29/33 ×4、314:24 ×2、258:29）
// ---------------------------------------------------------------------------

/// 120×40：inner=(24,15,72,10)。六行的 y 恰为 inner.y+1+i：
/// URL=16、保存到=17、并发=18、校验=19、校验码=20、代理=21；
/// 行 rect.x 恰为 inner.x+1=25；焦点在 URL（i=0）→ 仅该行有光标 ▏。
#[tokio::test]
async fn add_dialog_field_rows_exact_geometry() {
    let (mut a, reg) = app_with_add_dialog();
    let buf = render(&mut a, 120, 40);
    assert!(row_text(&buf, 16, 120).contains("URL"), "URL 行 y=16");
    // CJK 渲染为带空格形态（「保 存 到」），逐格断言 label 首字
    assert_eq!(cell(&buf, 25, 17), "保", "保存到行 y=17、label 自 x=25");
    assert_eq!(cell(&buf, 25, 18), "并", "并发行 y=18");
    assert_eq!(cell(&buf, 25, 19), "校", "校验行 y=19");
    assert!(
        row_text(&buf, 19, 120).contains("SHA-256"),
        "校验行显示当前算法"
    );
    // 行首列：label 从 x=25 起（mutant x=inner.x*1=24 / inner.x-1=23 皆错位）
    assert_eq!(cell(&buf, 25, 16), "U", "URL label 首字符恰在 x=25");
    // 焦点光标：URL 行有 ▏，并发行没有
    assert!(row_text(&buf, 16, 120).contains('▏'), "焦点行（URL）有光标");
    assert!(
        !row_text(&buf, 18, 120).contains('▏'),
        "非焦点行（并发）无光标"
    );
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

// ---------------------------------------------------------------------------
// ui/dialog.rs —— 按钮行与提示行几何（330:17/26 ×3、351:24 ×2、352:33 ×2）
// ---------------------------------------------------------------------------

/// 按钮行 y = inner.y + n_rows + 2 = 15+6+2 = 23；提示行 y = inner.y + n_rows + 3 = 24，
/// x = inner.x + 1 = 25（提示文本以空格开头、'E' 在 x=26）。
#[tokio::test]
async fn add_dialog_button_and_hint_rows_exact_geometry() {
    let (mut a, reg) = app_with_add_dialog();
    let buf = render(&mut a, 120, 40);
    assert!(
        !a.dlg_btn_rects.is_empty() && a.dlg_btn_rects.iter().all(|(r, _)| r.y == 23),
        "两个按钮的命中区 y 必须恰为 23（got {:?}）",
        a.dlg_btn_rects.iter().map(|(r, _)| r.y).collect::<Vec<_>>()
    );
    assert!(row_text(&buf, 23, 120).contains("确"), "按钮行 y=23");
    assert!(!row_text(&buf, 22, 120).contains("确"), "按钮不得上移");
    assert!(
        !row_text(&buf, 27, 120).contains("确"),
        "按钮不得下移（n_rows*2 变异）"
    );
    assert_eq!(cell(&buf, 25, 24), " ", "提示行起始于 x=25（首字符为空格）");
    assert_eq!(cell(&buf, 26, 24), "E", "提示文本 Enter 的 E 在 x=26");
    assert!(row_text(&buf, 24, 120).contains("Enter"), "提示行 y=24");
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

// ---------------------------------------------------------------------------
// ui/dialog.rs —— 校验算法下拉几何（36:16、41:21、123:21、363:37）
// ---------------------------------------------------------------------------

/// 120×18 终端：dlg.y=3、inner.y=4、校验行 y=8、下拉 py=9、ph=7+2=9、
/// px=inner.x+8=32（未越界不触发钳制）。py+ph=18 恰等于终端底 →
/// 越界翻转守卫（`py+ph > bottom` 为假）必须保持行下方展开。
/// 选项行 y = pinner.y + i = 10+i（MD5@10、SHA-1@11、Adler-32@16），
/// 底边框角 ╰ 在 (32,17)。
#[tokio::test]
async fn ck_dropdown_exact_geometry() {
    let (mut a, reg) = app_with_add_dialog();
    {
        let d = a.dialog.as_mut().unwrap();
        d.focus = 3; // 校验行
        d.ck_open = true;
    }
    let buf = render(&mut a, 120, 18);
    assert_eq!(
        cell(&buf, 32, 9),
        "╭",
        "下拉框左上角必须在其 x=32、y=9（未越界不翻转）"
    );
    assert!(row_text(&buf, 10, 120).contains("MD5"), "选项 0 行 y=10");
    assert!(row_text(&buf, 11, 120).contains("SHA-1"), "选项 1 行 y=11");
    assert!(
        row_text(&buf, 16, 120).contains("Adler-32"),
        "选项 6 行 y=16"
    );
    assert_eq!(
        cell(&buf, 32, 17),
        "╰",
        "下拉框左下角 y=17（ph=9 收口于终端底）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

// ---------------------------------------------------------------------------
// ui/dialog.rs —— 代理下拉单项高度（382:37 ×2）
// ---------------------------------------------------------------------------

/// 默认仅「直连」一项：ph = 1+2 = 3，下拉 y=22..24，选项行 y=23，
/// 底角 ╰ 在 (32,24)。ph 变异（*2=2 → 内框 0 行；-2 → u16 下溢 panic）
/// 都会破坏该契约。
#[tokio::test]
async fn proxy_dropdown_single_item_exact_geometry() {
    let (mut a, reg) = app_with_add_dialog();
    {
        let d = a.dialog.as_mut().unwrap();
        d.focus = 5; // 代理行
        d.proxy_open = true;
    }
    let buf = render(&mut a, 120, 40);
    assert_eq!(cell(&buf, 32, 22), "╭", "代理下拉左上角 (32,22)");
    assert!(row_text(&buf, 23, 120).contains("直"), "唯一选项行 y=23");
    assert_eq!(cell(&buf, 32, 24), "╰", "代理下拉左下角 (32,24)（ph=3）");
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

// ---------------------------------------------------------------------------
// model/config.rs —— block_size_http 正值过滤（312:32）
// ---------------------------------------------------------------------------

/// 配置显式正值必须生效；0 必须回退默认（负对照）。
/// `*v > 0` 变异为 `<` 后正值会被静默丢弃回退默认。
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

// ---------------------------------------------------------------------------
// app/engine.rs —— shutdown（622:9、625:33）
// ---------------------------------------------------------------------------

/// 滴流服务器：长体响应，客户端持续收数据
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

/// 优雅退出必须暂停下载中任务（过滤 `state == Downloading`）并保存注册表：
/// 622 整体 no-op 变异 / 625 `==`→`!=` 变异都会让下载中任务保持下载态。
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
    // 收尾窗口内引擎处理 Pause → sidecar 落盘（非下载中任务不会被暂停：
    // 625 `==`→`!=` 变异会跳过下载中任务、622 整体 no-op 则无人被暂停）
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

// ---------------------------------------------------------------------------
// app/engine.rs —— handle_failure at_limit 分支（585:13）
// ---------------------------------------------------------------------------

/// 恒 500 服务器（Transient 失败）
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

/// max_retries=1 时首次瞬时失败即达上限：错误文案必须是
/// 「已达最大重试次数…按 R 手动重试」（at_limit 专属分支），
/// 而非通用停等文案。分支被删则落入 `_` 臂、文案不同 → 测试红。
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

// ===========================================================================
// v115 补充处置（会话恢复轮）：supervisor / engine / dialog 剩余存活位点。
// 靶点索引（接头部清单）：
// - ui/dialog.rs 206:85     越界 ck_type 钳制（- → + / - → /）  → dialog_clamps_out_of_range_ck_type
// - app/engine.rs 84:9      任务级命名代理解析 no-op            → named_dead_proxy_fails_probe
// - app/engine.rs 217:28    速度窗口采样过滤翻转                → speed_window_tracks_downloading_task
// - engine/supervisor.rs 822:16 ×2  配额边界（== / >=）         → single_conn_download_completes
// - engine/supervisor.rs 831:21     领块扫描越界（<=）          → single_conn_download_completes
// - engine/supervisor.rs 875:22     块响应非 2xx 守卫           → range_denied_fails_task
// - engine/supervisor.rs 449:58     sidecar 首个落盘期限        → sidecar_flush_waits_interval
// - engine/supervisor.rs 248:5      stop_workers 整体 no-op     → pause_freezes_file_bytes_on_disk
// - engine/supervisor.rs 860:44     待命并入 retain 翻转        → conn_views_have_unique_ids
// - engine/supervisor.rs 932:20     中断块不得标记完成          → pause_mid_block_resume_content_intact
// - engine/supervisor.rs 154:91     completed_of 整块口径       → failed_reports_completed_blocks_of_sidecar
// - engine/supervisor.rs 289:27     未知大小 stamp 包装         → unknown_size_resume_restarts
// ===========================================================================

const ETAG: &str = "W/\"ezr-hard-v115\"";

/// 模式化内容：byte[i] = i % 251 + 1（无 0 字节，便于识别「洞」）
fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251 + 1) as u8).collect()
}

/// 按分片流出 `[from, to)` 字节；slow=true 时固定 100B/50ms（供暂停时机）
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

/// 模式化静态 HTTP 服务器（加固测试专用）：
/// - 无 Range → 200 + 全量 CL + ETag；`bytes=0-`（探测）→ 206 + 全量 CL + ETag；
/// - 其余 Range 请求 → 206 自 start 起流出；`deny` 置位时一律 403；
/// - `throttle` = (每 chunk 字节, 间隔 ms)，(0, _) 不限速；
/// - `slow_first` = Some(n) 时：首个下载连接（第 2 个连接，探测是第 1 个）
///   按 100B/50ms 滴流（供「暂停时机」确定性）。
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
                    let start: usize = range
                        .trim_start_matches("bytes=")
                        .split('-')
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    let head = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: {ETAG}\r\nConnection: close\r\n\r\n",
                        len.saturating_sub(start)
                    );
                    let _ = c.write_all(head.as_bytes());
                    // 慢速语义落在首个 worker 连接（块 0 慢流，供暂停时机确定性）
                    stream_slice(
                        &mut c,
                        &data,
                        start,
                        len,
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

/// 无 Content-Length 的滴流服务器（未知大小；200B/100ms，每连接供 300s）
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

/// 唯一临时目录（pid + 标签）
fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("ezr-hard-v115-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// 逐 tick 轮询至谓词成立或超时（返回是否成立）
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

/// 暂停（Space）并等待 sidecar 落盘
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

// ---------------------------------------------------------------------------
// ui/dialog.rs 206:85 —— 越界 ck_type 必须被 min 钳制（- → + / - → / 变异体
// 会以越界下标取 CHECKSUM_ALGOS 直接 panic）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dialog_clamps_out_of_range_ck_type() {
    let (mut a, reg) = app_with_add_dialog();
    {
        let d = a.dialog.as_mut().unwrap();
        d.ck_type = app::CHECKSUM_ALGOS.len(); // 越界下标（合法域 0..len-1）
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

// ---------------------------------------------------------------------------
// app/engine.rs 84:9 —— 任务级命名代理必须真实生效：死代理指向的探测必须失败
//（resolve_endpoint→None 变异体会无视命名代理、直连成功）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn named_dead_proxy_fails_probe() {
    let (addr, _data) = stub_http(64 * 1024, (0, 0), None, None);
    let cfg = Config::from_toml(
        "[[proxies]]\nname = \"dead\"\ntype = \"http\"\nip = \"127.0.0.1\"\nport = 9\n",
    );
    let reg = temp_reg();
    let mut a = App::new(cfg, reg.to_string_lossy().into_owned());
    a.on_key(KeyCode::Char('a'), KeyModifiers::empty());
    {
        let d = a.dialog.as_mut().unwrap();
        d.url = format!("http://{addr}/f.bin");
        d.proxy_sel = 1; // 跳过「直连」（0），选第一个命名代理 dead
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

// ---------------------------------------------------------------------------
// app/engine.rs 217:28 —— 速度窗口只采样下载中任务（!= → == 变异体会跳过
// 下载中任务、其速度恒 0）
// ---------------------------------------------------------------------------

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
    tokio::time::sleep(std::time::Duration::from_millis(1700)).await; // ≥ SPEED_TICK(1s)
    a.tick().await;
    assert!(
        a.tasks[0].speed > 100.0,
        "下载中任务必须有非零速度（滴流 ≈2000B/s，got {}）",
        a.tasks[0].speed
    );
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

// ---------------------------------------------------------------------------
// engine/supervisor.rs 822:16 ×2 / 831:21 —— 单连接（wid==quota）下载必须完成：
// 配额 ==/>= 变异体让唯一 worker 立即退出；领块扫描 <= 变异体越界访问
// ---------------------------------------------------------------------------

#[tokio::test]
async fn single_conn_download_completes() {
    let len = 96 * 1024;
    let (addr, data) = stub_http(len, (0, 0), None, None);
    let reg = temp_reg();
    let dir = temp_dir("sconn");
    // 16KiB 块强制多块路径（默认 1MiB 会因 total_blocks<=1 走单流、不触及 block_worker）
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
    let ok = wait_until(&mut a, 30, |a| a.tasks[0].state == TaskState::Completed).await;
    assert!(ok, "单连接下载必须完成（实际状态 {:?}）", a.tasks[0].state);
    assert_eq!(a.tasks[0].downloaded, len as u64, "下载字节数必须齐全");
    let file = std::fs::read(dir.join("f.bin")).expect("完成态文件应就位");
    assert_eq!(file, *data, "文件内容必须与服务器一致");
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

// ---------------------------------------------------------------------------
// engine/supervisor.rs 875:22 —— 块响应非 2xx 守卫：403 必须按失败处理
//（守卫→true 变异体会把 403 响应当成功流写盘并「完成」）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn range_denied_fails_task() {
    let deny = Arc::new(AtomicBool::new(true));
    let (addr, _data) = stub_http(64 * 1024, (0, 0), None, Some(deny));
    let reg = temp_reg();
    let dir = temp_dir("r403");
    // 16KiB 块强制多块路径（默认 1MiB 会因 total_blocks<=1 走单流、不触及 block_worker）
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

// ---------------------------------------------------------------------------
// engine/supervisor.rs 449:58 —— sidecar 周期落盘不从第 0 秒开始
//（+ → - 变异体把首个 flush 期限推到过去、立即落盘）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sidecar_flush_waits_interval() {
    let addr = stub_trickle();
    let reg = temp_reg();
    let dir = temp_dir("flush");
    // 16KiB 块强制多块路径（默认 1MiB 会因 total_blocks<=1 走单流、不触及 block_worker）
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
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await; // < SIDECAR_FLUSH(2s)
    a.tick().await;
    assert!(
        !dir.join("f.bin.ezr").exists(),
        "周期未到不得写 sidecar（下载仅开始约 1.2s，期限 2s）"
    );
    a.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&reg);
}

// ---------------------------------------------------------------------------
// engine/supervisor.rs 248:5 —— stop_workers 整体 no-op：暂停后磁盘字节流必须
// 冻结（no-op 变异体的 worker 失去停止信号、脱离句柄表后仍每 100ms 写 200B）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn pause_freezes_file_bytes_on_disk() {
    let addr = stub_trickle();
    let reg = temp_reg();
    let dir = temp_dir("pfrz");
    // 16KiB 块强制多块路径（默认 1MiB 会因 total_blocks<=1 走单流、不触及 block_worker）
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
    tokio::time::sleep(std::time::Duration::from_millis(400)).await; // 磁盘上先有些字节
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

// ---------------------------------------------------------------------------
// engine/supervisor.rs 860:44 —— 待命并入 retain：连接视图不得出现重复 id
//（!= → == 变异体保留旧条目 + 追加待命条目）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn conn_views_have_unique_ids() {
    let (addr, _data) = stub_http(64 * 1024, (2048, 40), None, None); // ≈51KB/s/连接
    let reg = temp_reg();
    let dir = temp_dir("conns");
    // 16KiB 块强制多块路径（默认 1MiB 会因 total_blocks<=1 走单流、不触及 block_worker）
    let mut a = App::new(
        Config::from_toml("block_size_http = 16384\n"),
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

// ---------------------------------------------------------------------------
// engine/supervisor.rs 932:20 —— 暂停中断的块不得标记完成：续传后内容必须完整
//（|| → && 变异体在 abort 后仍 mark_done，续传跳过该块留下「洞」）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn pause_mid_block_resume_content_intact() {
    let len = 64 * 1024; // 16KiB 块 → 4 块；暂停点落在块 0 中部
    let (addr, data) = stub_http(len, (0, 0), Some(6 * 1024), None);
    let reg = temp_reg();
    let dir = temp_dir("resm");
    // 16KiB 块强制多块路径（默认 1MiB 会因 total_blocks<=1 走单流、不触及 block_worker）
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
        wait_until(&mut a, 30, |a| a.tasks[0].state == TaskState::Downloading).await,
        "慢速滴流应进入下载中"
    );
    // App 可见 downloaded 是块级粒度（块内进度流结束才回写），故用固定延时
    // 落在块 0 中部：慢速首连 100B/50ms ≈ 2KB/s，1.5s ≈ 3KB ≪ 16KiB 块
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    a.tick().await;
    pause_and_wait_sidecar(&mut a, &dir.join("f.bin.ezr")).await;
    a.on_key(KeyCode::Char(' '), KeyModifiers::empty()); // 恢复（toggle_pause）
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

// ---------------------------------------------------------------------------
// engine/supervisor.rs 154:91 —— completed_of 整块口径：失败汇总只计整块
//（>= → < 变异体把部分写块计成已完成）
// ---------------------------------------------------------------------------

#[tokio::test]
async fn failed_reports_completed_blocks_of_sidecar() {
    let deny = Arc::new(AtomicBool::new(false));
    // 5KB/s 限速：暂停时机落在块 0 中部（16KiB 块），且不会在暂停前跑完
    let (addr, _data) = stub_http(64 * 1024, (256, 50), None, Some(Arc::clone(&deny)));
    let reg = temp_reg();
    let dir = temp_dir("cdof");
    // 16KiB 块强制多块路径（默认 1MiB 会因 total_blocks<=1 走单流、不触及 block_worker）
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
        wait_until(&mut a, 30, |a| a.tasks[0].state == TaskState::Downloading).await,
        "限速下载应进入下载中"
    );
    // App 可见 downloaded 是块级粒度，用固定延时落在块 0 中部：
    // 5KB/s 限速下 1s ≈ 4-5KB ≪ 16KiB 块
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    a.tick().await;
    pause_and_wait_sidecar(&mut a, &dir.join("f.bin.ezr")).await;
    deny.store(true, Ordering::SeqCst); // 恢复后的块请求一律 403
    a.on_key(KeyCode::Char(' '), KeyModifiers::empty()); // 恢复
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

// ---------------------------------------------------------------------------
// engine/supervisor.rs 289:27 —— 未知大小任务的断点戳记：size 缺省必须保持
// None（> → >= 变异体写成 Some(0)，续传一致性走 Valid 分支按 0 块假续传卡死；
// 原行为 MissingStamp → Invalidated 从头重下 → downloaded 必然回落）
// ---------------------------------------------------------------------------

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
    pause_and_wait_sidecar(&mut a, &dir.join("f.bin.ezr")).await; // 先暂停（Sidecar 必落）
    a.on_key(KeyCode::Char(' '), KeyModifiers::empty()); // 恢复
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

// ===========================================================================
// v115 收尾处置（会话恢复第二轮）：剩余 3 个真实存活位点杀灭。
// 另有 2 个等价登记（engine/supervisor.rs 309:27 与 309:78，sidecar filter
// 谓词）：上游守卫（292 行 sidecar.is_some() → name = spec.name）+ sidecar
// 与任务绑定不变式（task.id 随任务持久化恢复）使命题恒真，论证见
// project/handoff.md ④。
// ===========================================================================

/// 字符串级重写 sidecar 的 block_count 与 blocks 数组（测试专用 helper）。
/// dev-dependencies 无 serde_json，且 sidecar 为 serde 紧凑序列化（字段唯一、
/// 无嵌套歧义），直接定位 `"block_count":` 与 `"blocks":` 改写即可。
/// blocks 同步改为 count 项全 0：隔离靶点，只让 315 行块数自洽守卫可见。
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

/// engine/supervisor.rs 315:59 —— 续传块数自洽守卫（`&&` → `||`）。
/// consistency::check 的 same_size 已保证 Valid 分支内 size == head.total 恒真，
/// 故 `sidecar.size == head.total` 在该分支是冗余臂，守卫实义只在块数自洽：
/// 手改 sidecar 使 chunk_total(1000,100)=10 ≠ block_count=3 ——
/// 原实现 → Invalidated → 从头重下 → 文件完整 1000 字节；
/// 变异体（恒真）→ 假续传 Blocks{written:[0,0,0]} → 只领 3 块写 300 字节。
#[tokio::test]
async fn tampered_block_count_restarts_fresh() {
    // (10B, 20ms) = 500B/s：1000B 约 2s，暂停窗口落在块 0-2（远未完成）
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
    // 手改 sidecar：块数记账与 size/block_size 不自洽（10 块文件记成 3 块）
    let sc_path = dir.join("f.bin.ezr");
    let raw = std::fs::read_to_string(&sc_path).expect("sidecar 应可读");
    std::fs::write(&sc_path, rewrite_block_count(&raw, 3)).expect("sidecar 应可写");
    a.on_key(KeyCode::Char(' '), KeyModifiers::empty()); // 恢复
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

/// engine/supervisor.rs 334:53 —— multi_ok 的大小守卫（`>` → `>=`）。
/// stub_http(0)：探测 bytes=0- 得 206 + Content-Length: 0 → head.total=0：
/// 原实现 multi_ok = true && 0>0 = false → resumable 必须为 false；
/// 变异体（u64 上 0>=0 恒真）→ multi_ok = true → 误标可续传。
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

// ---------------------------------------------------------------------------
// ui/dialog.rs 41:21 —— px 下限钳制（area.x+1 → area.x*1）。
// 唯一可达且不越界的触发点是 27 列终端 + 代理下拉（pw=26）：dw 钳到 27 →
// inner=(1,15,25,10)；右钳臂 = 27-(26+1) = 0 < inner.x+8=9 → px 触下限。
// 原实现 px = max(0, 0+1) = 1：左边界 ╭ 恰在 x=1、x=0 必须空白；
// `+ → *` 变异体（area.x*1=0）把左边界推到 x=0。
// 备注：26 列触发下限分支时旧实现 prect 右缘 27>26 越界 panic（窄端渲染
// 缺陷，hardender 登记移交、操作者裁决由 coder v116 直接修复——浮层宽/高
// 钳制在 area 内，回归测试见下方 26 列与高于终端两例）；
// 校验下拉（pw=24）触发需 width<26 → inner<24 被守卫早退，永不可达。
// ---------------------------------------------------------------------------

#[tokio::test]
async fn proxy_dropdown_left_clamp_at_narrow_width() {
    let (mut a, reg) = app_with_add_dialog();
    {
        let d = a.dialog.as_mut().unwrap();
        d.focus = 5; // 代理行
        d.proxy_open = true;
    }
    let buf = render(&mut a, 27, 40);
    assert_eq!(
        cell(&buf, 1, 22),
        "╭",
        "窄端代理下拉左上角必须在 (1,22)（px = area.x + 1 = 1）"
    );
    // 27 列时对话框 dw 钳满全宽，x=0 是对话框自身左边框 │；
    // `+ → *` 变异体 px=0 会用下拉左上角 ╭ 覆盖该格
    assert_eq!(
        cell(&buf, 0, 22),
        "│",
        "x=0 必须仍是对话框左边框（px 下限钳制留 1 列边距；+ → * 变异体贴边覆盖为 ╭）"
    );
    assert!(row_text(&buf, 23, 27).contains("直"), "唯一选项行仍在 y=23");
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

/// 缺陷回归（coder v116，操作者裁决直接修复）：26 列终端 + 代理下拉。
/// 旧几何 px 触下限钳（area.x+1）后浮层宽不钳，右缘 1+26=27>26 →
/// ratatui Clear 渲染越界 panic；修复后浮层宽钳为 area.width-1=25，
/// 渲染不 panic，左上角/对话框左边框与 27 列情形一致，右缘落在末列。
#[tokio::test]
async fn proxy_dropdown_no_panic_at_26_cols() {
    let (mut a, reg) = app_with_add_dialog();
    {
        let d = a.dialog.as_mut().unwrap();
        d.focus = 5; // 代理行
        d.proxy_open = true;
    }
    // 修复前：render 内部 Clear 越界 panic（prect 右缘 27 > 缓冲 26）
    let buf = render(&mut a, 26, 40);
    assert_eq!(cell(&buf, 1, 22), "╭", "下拉左上角仍在 (1,22)");
    assert_eq!(cell(&buf, 0, 22), "│", "x=0 仍是对话框左边框");
    assert!(row_text(&buf, 23, 26).contains("直"), "唯一选项行仍在 y=23");
    assert_eq!(cell(&buf, 25, 22), "╮", "浮层右缘钳在末列 x=25");
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}

/// 缺陷回归（coder v116，同缺陷纵向分支）：下拉浮层高于终端（12 个命名
/// 代理 → ph=15 > 12 行终端）时，旧几何上移展开后下缘 0+15=15>12 越界
/// panic；修复后高钳为终端高，渲染不 panic。
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
    {
        let d = a.dialog.as_mut().unwrap();
        d.focus = 5; // 代理行（Add 第 6 行）
        d.proxy_open = true;
    }
    // 修复前：Clear 越界 panic（prect 下缘 15 > 终端 12）
    let buf = render(&mut a, 80, 12);
    assert_eq!(
        cell(&buf, 12, 0),
        "╭",
        "上移展开后浮层顶在 y=0（px=inner.x+8）"
    );
    assert_eq!(cell(&buf, 12, 11), "╰", "浮层下缘钳在末行 y=11");
    a.shutdown().await;
    let _ = std::fs::remove_file(&reg);
}
