//! ezr-fixture — 本地 fixture 服务器（QA 基建 + 交付物，AC-10）
//!
//! 能力清单（对应 qa/ 各套件环境前置节）：
//! - Range 支持/关闭（`?norange=1`）、隐藏 Content-Length（`streamy.bin` chunked）
//! - 状态码注入（`?status=503`）+ Retry-After（`?retry_after=10`）
//! - 传输中途断连（`?disconnect=N`：传 N 字节后断）、慢速门控（`?speed=B/s`）
//! - 重定向链（`?redirect=N`：再跳 N 次）
//! - 一致性头控制：`?etag=X` 覆盖 / `?noheaders=1` 不返回 ETag/Last-Modified
//! - Content-Disposition（`?cd=name`）
//! - 同 URL 换内容（`?swapsize=N`）
//! - 请求头记录（`--log <jsonl>`：method/path/range/accept-encoding）
//! - 文件魔法名：`ghost-404.bin` → 404；`streamy.bin` → chunked 无 Content-Length
//!
//! 用法：
//! ```text
//! ezr-fixture gen --root <dir>            # 生成标准文件集（含 100MB 稀疏）
//! ezr-fixture serve --root <dir> --port 8765 [--log access.jsonl]
//! ```
//!
//! 文件内容确定性：字节模式 `i % 251`，供校验值预计算。
//! body 输出为流式（按需 seek + 64 KiB 分块读盘），每请求内存占用恒定；
//! 仅 `?swapsize=N` 请求例外物化到内存（QA 约定仅小文件使用该注入）。
#![allow(missing_docs)] // 交互层：demo 定稿基线复用，接口文档见 model/engine 层
#![allow(clippy::pedantic)] // 交互层字节/速度展示算术与 demo 基线风格豁免
#![allow(clippy::nursery)] // 同上
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

mod files;
mod handle;
mod query;
mod range;
mod request;
mod respond;

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;

use crate::files::gen_files;
use crate::handle::handle;
use crate::request::AccessLog;

/// CLI 参数（gen/serve 共用；默认 root `.`，port 8765）
struct FixtureArgs {
    cmd: String,
    root: String,
    port: u16,
    log_path: Option<String>,
}

/// 解析 `gen|serve --root <dir> --port <n> [--log <路径>]`（未知参数忽略）
fn parse_fixture_args(args: &[String]) -> FixtureArgs {
    let mut out = FixtureArgs {
        cmd: String::new(),
        root: String::from("."),
        port: 8765,
        log_path: None,
    };
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "gen" | "serve" => out.cmd = args[i].clone(),
            "--root" => {
                i += 1;
                out.root = args.get(i).cloned().unwrap_or_else(|| out.root.clone());
            }
            "--port" => {
                i += 1;
                out.port = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(out.port);
            }
            "--log" => {
                i += 1;
                out.log_path = args.get(i).cloned();
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// accept 循环：逐连接孵化 detach 线程（瞬时 accept 错误记录后继续，
/// engineering.md 网络 fixture 条款）
fn serve_loop(listener: TcpListener, root: Arc<PathBuf>, log: Arc<AccessLog>) {
    for conn in listener.incoming() {
        match conn {
            Ok(s) => {
                let root = Arc::clone(&root);
                let log = Arc::clone(&log);
                // fixture 线程：detach（测试进程退出自然回收，engineering.md 纪律）
                std::thread::spawn(move || handle(s, root, log));
            }
            Err(e) => {
                eprintln!("ezr-fixture: accept 错误（{e}）");
            }
        }
    }
}

/// 按子命令执行（gen 生成文件集 / serve 起服务）；返回进程退出码
fn run_cmd(a: FixtureArgs) -> i32 {
    let root_path: PathBuf = a.root.into();
    match a.cmd.as_str() {
        "gen" => match gen_files(&root_path) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("ezr-fixture: 生成失败（{e}）");
                1
            }
        },
        "serve" => {
            let listener = TcpListener::bind(("127.0.0.1", a.port)).expect("绑定失败");
            println!(
                "ezr-fixture listening on http://127.0.0.1:{} root={}",
                a.port,
                root_path.display()
            );
            let log = Arc::new(AccessLog::open(a.log_path.as_deref()));
            serve_loop(listener, Arc::new(root_path), log);
            0
        }
        _ => {
            eprintln!(
                "用法:\n  ezr-fixture gen --root <dir>\n  ezr-fixture serve --root <dir> --port 8765 [--log access.jsonl]"
            );
            2
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let a = parse_fixture_args(&args);
    let code = run_cmd(a);
    if code != 0 {
        std::process::exit(code);
    }
}

#[cfg(test)]
mod main_tests {
    use std::io::{Read as _, Write as _};

    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_args_defaults() {
        let a = parse_fixture_args(&[]);
        assert_eq!(a.cmd, "");
        assert_eq!(a.root, ".");
        assert_eq!(a.port, 8765);
        assert!(a.log_path.is_none());
    }

    #[test]
    fn parse_args_full_form() {
        let a = parse_fixture_args(&args(&[
            "ezr-fixture",
            "serve",
            "--root",
            "/tmp/r",
            "--port",
            "9001",
            "--log",
            "x.jsonl",
        ]));
        assert_eq!(a.cmd, "serve");
        assert_eq!(a.root, "/tmp/r");
        assert_eq!(a.port, 9001);
        assert_eq!(a.log_path.as_deref(), Some("x.jsonl"));
        // 非法 port 沿用默认；--root 一律消费下一参数为值（原实现口径）；未知参数忽略
        let a = parse_fixture_args(&args(&[
            "ezr-fixture",
            "serve",
            "--port",
            "zz",
            "--root",
            "--bogus",
        ]));
        assert_eq!(a.port, 8765);
        assert_eq!(a.root, "--bogus");
        assert_eq!(a.cmd, "serve");
        // --root 无后续参数 → 保持默认
        let a = parse_fixture_args(&args(&["ezr-fixture", "serve", "--root"]));
        assert_eq!(a.root, ".");
    }

    #[test]
    fn run_cmd_gen_creates_fileset_and_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("ezr-fx-gen-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let code = run_cmd(FixtureArgs {
            cmd: "gen".to_string(),
            root: dir.to_string_lossy().into_owned(),
            port: 0,
            log_path: None,
        });
        assert_eq!(code, 0);
        for name in [
            "five-m.bin",
            "small.bin",
            "streamy.bin",
            "ghost-404.bin",
            "big-100m.bin",
        ] {
            assert!(dir.join(name).exists(), "{name}");
        }
        assert_eq!(dir.join("small.bin").metadata().unwrap().len(), 1024);
        assert_eq!(
            dir.join("big-100m.bin").metadata().unwrap().len(),
            100 * 1024 * 1024
        );
        // 幂等：再次 gen 不重建既有文件（跳过存在项）
        let code = run_cmd(FixtureArgs {
            cmd: "gen".to_string(),
            root: dir.to_string_lossy().into_owned(),
            port: 0,
            log_path: None,
        });
        assert_eq!(code, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn run_cmd_unknown_usage_exit_code() {
        let code = run_cmd(FixtureArgs {
            cmd: String::new(),
            root: ".".to_string(),
            port: 0,
            log_path: None,
        });
        assert_eq!(code, 2);
    }

    #[test]
    fn serve_loop_serves_generated_root() {
        let dir = std::env::temp_dir().join(format!("ezr-fx-serve-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("small.bin"), b"0123456789").unwrap();
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let log = Arc::new(AccessLog::open(None));
        std::thread::spawn(move || serve_loop(listener, Arc::new(dir), log));
        let mut c = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        c.set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        c.write_all(b"GET /small.bin HTTP/1.1\r\nHost: t\r\n\r\n")
            .unwrap();
        let mut resp = Vec::new();
        let _ = c.read_to_end(&mut resp);
        let resp = String::from_utf8_lossy(&resp);
        assert!(resp.contains("200 OK"));
        assert!(resp.ends_with("0123456789"));
    }
}
