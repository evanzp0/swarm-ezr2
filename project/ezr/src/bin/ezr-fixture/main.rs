#![allow(missing_docs)]
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

mod files;
mod handle;
mod query;
mod range;
mod request;
mod respond;

#[cfg(test)]
#[path = "../../model/testenv.rs"]
mod testenv;

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;

use crate::files::gen_files;
use crate::handle::handle;
use crate::request::AccessLog;

struct FixtureArgs {
    cmd: String,
    root: String,
    port: u16,
    log_path: Option<String>,
}

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

fn serve_loop(listener: TcpListener, root: Arc<PathBuf>, log: Arc<AccessLog>) {
    for conn in listener.incoming() {
        match conn {
            Ok(s) => {
                let root = Arc::clone(&root);
                let log = Arc::clone(&log);
                std::thread::spawn(move || handle(s, root, log));
            }
            Err(e) => {
                eprintln!("ezr-fixture: accept 错误（{e}）");
            }
        }
    }
}

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
        let a = parse_fixture_args(&args(&["ezr-fixture", "serve", "--root"]));
        assert_eq!(a.root, ".");
    }

    #[test]
    fn run_cmd_gen_creates_fileset_and_is_idempotent() {
        let dir = crate::testenv::uniq_tmp_dir("ezr-fx-gen");
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
        let dir = crate::testenv::uniq_tmp_dir("ezr-fx-serve");
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
