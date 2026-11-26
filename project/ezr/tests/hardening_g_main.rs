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

use std::process::{Command, Stdio};

fn isolated_home(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("ezr-hard-main-{}-{tag}", std::process::id()));
    let _ = std::fs::create_dir_all(&d);
    d
}

fn ezr(tag: &str) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ezr"));
    c.env("EZR_HOME", isolated_home(tag))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    c
}

#[test]
fn help_prints_usage_and_exits_zero() {
    let out = ezr("help").arg("--help").output().expect("spawn ezr");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "help 应 0 退出，got {:?}", out.status);
    assert!(
        stdout.contains("用法: ezr"),
        "stdout 应含用法文本: {stdout}"
    );
    assert!(
        stdout.contains("EZR Downloader"),
        "stdout 应含产品名: {stdout}"
    );
}

#[test]
fn tui_without_terminal_fails() {
    let mut child = ezr("tui").spawn().expect("spawn ezr");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    let status = loop {
        if let Some(st) = child.try_wait().expect("try_wait") {
            break Some(st);
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let status = status.expect("无终端环境 TUI 应快速失败退出，而非长时间运行");
    assert!(
        !status.success(),
        "无终端下 TUI 初始化必须失败（恒 Ok 变异体退出 0）"
    );
}
