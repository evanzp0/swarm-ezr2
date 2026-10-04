//! 加固测试（six-pack/hardender 步骤 7 批次 5）：main.rs 进程级靶点。
//! 靶点：print_help 162:5（no-op）、main 211:5（Ok 恒返）、ezr_main 221:5（Ok 恒返）、
//! run_tui 299:5（Ok 恒返）。
//! 机制：进程级观测——`--help` 路径的真实行为是 stdout 帮助文本 + 0 退出；
//! 无参数路径在本环境（无控制终端）下 enable_raw_mode 失败 → 主进程非零退出
//! （恒 Ok 变异体退出 0）。终端态类靶点（restore_terminal/setup_panic_hook/run
//! 事件循环）仅在真实 TTY 可观测 → 环境条件型论证，见 EQUIVALENCE.md。
#![allow(missing_docs)]

use std::process::{Command, Stdio};

/// 独立 EZR_HOME：不读用户配置、锁文件隔离
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

/// 靶 print_help 162:5 / main 211:5 / ezr_main 221:5：`--help` 必须打印用法且 0 退出
#[test]
fn help_prints_usage_and_exits_zero() {
    let out = ezr("help").arg("--help").output().expect("spawn ezr");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "help 应 0 退出，got {:?}", out.status);
    assert!(stdout.contains("用法: ezr"), "stdout 应含用法文本: {stdout}");
    assert!(stdout.contains("EZR Downloader"), "stdout 应含产品名: {stdout}");
}

/// 靶 run_tui 299:5：无终端环境启动 TUI 必须失败（非零退出）；
/// 恒 Ok 变异体会走 shutdown 并 0 退出。加超时兜底防挂死。
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
