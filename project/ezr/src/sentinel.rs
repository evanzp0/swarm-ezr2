#![allow(clippy::cognitive_complexity, clippy::too_many_lines)]

use std::io::{self, Read, Write};
use std::process::Child;
#[cfg(unix)]
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

#[cfg(unix)]
pub const ENV_SENTINEL: &str = "EZR_INTERNAL_SENTINEL";

const READY_BYTE: &[u8; 1] = b"R";

const READY_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(unix)]
const RESTORE_SEQUENCE: &str = concat!(
    "\u{1b}[?1006l",
    "\u{1b}[?1015l",
    "\u{1b}[?1003l",
    "\u{1b}[?1002l",
    "\u{1b}[?1000l",
    "\u{1b}[?2004l",
    "\u{1b}[?1049l",
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(unix)]
enum SentinelOutcome {
    CleanExit,
    ParentDeath {
        termios_restored: bool,
        seq_written: bool,
    },
}

#[must_use]
#[cfg(unix)]
pub fn arm() -> Option<TerminalSentinel> {
    arm_inner().ok()
}

#[must_use]
#[cfg(not(unix))]
pub fn arm() -> Option<TerminalSentinel> {
    None
}

#[cfg(unix)]
fn arm_inner() -> io::Result<TerminalSentinel> {
    use std::io::pipe;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;

    let exe = std::env::current_exe()?;
    let (wake_rx, wake_tx) = pipe()?;
    let (ready_rx, ready_tx) = pipe()?;
    let mut child = Command::new(&exe)
        .env(ENV_SENTINEL, "1")
        .stdin(wake_rx)
        .stdout(ready_tx)
        .stderr(Stdio::inherit())
        .process_group(0)
        .spawn()?;
    let ok = wait_ready(ready_rx);
    trace("parent: handshake", &format!("{ok} pid={}", child.id()));
    if !ok {
        let _ = child.kill();
        let _ = child.wait();
        return Err(io::Error::other("sentinel ready handshake failed"));
    }
    Ok(TerminalSentinel {
        wake_tx: Some(wake_tx),
        child: Some(child),
    })
}

#[cfg(unix)]
fn wait_ready(rx: std::io::PipeReader) -> bool {
    let (tx, rx_chan) = mpsc::channel();
    std::thread::spawn(move || {
        let mut rx = rx;
        let mut b = [0u8; 1];
        let ok = loop {
            match rx.read(&mut b) {
                Ok(0) => break false,
                Ok(_) => break true,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break false,
            }
        };
        let _ = tx.send(ok);
    });
    rx_chan.recv_timeout(READY_TIMEOUT).unwrap_or(false)
}

#[cfg(unix)]
pub struct TerminalSentinel {
    wake_tx: Option<std::io::PipeWriter>,
    child: Option<Child>,
}

#[cfg(unix)]
impl TerminalSentinel {
    pub fn release(mut self) {
        self.wake_and_reap();
    }

    fn wake_and_reap(&mut self) {
        if let Some(mut tx) = self.wake_tx.take() {
            let _ = tx.write_all(READY_BYTE);
        }
        if let Some(c) = self.child.as_mut() {
            let _ = c.wait();
        }
    }
}

#[cfg(unix)]
impl Drop for TerminalSentinel {
    fn drop(&mut self) {
        self.wake_and_reap();
    }
}

#[cfg(not(unix))]
pub struct TerminalSentinel;

#[cfg(not(unix))]
impl TerminalSentinel {
    pub fn release(self) {}
}

#[cfg(unix)]
pub fn run_child() -> ! {
    let tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .ok();
    let saved = tty.as_ref().and_then(stty_save);
    trace("child: stty_save", saved.as_deref().unwrap_or("<none>"));
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();
    let outcome = sentinel_core(&mut stdin, &mut stdout, &mut || {
        let r = restore_tty(tty.as_ref(), saved.as_deref());
        trace("child: restore_tty", &format!("{r:?}"));
        r
    });
    trace("child: outcome", &format!("{outcome:?}"));
    std::process::exit(0);
}

#[cfg(unix)]
fn trace(tag: &str, detail: &str) {
    use std::fmt::Write as _;
    if std::env::var_os("EZR_SENTINEL_TRACE").is_none() {
        return;
    }
    let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/ezr-sentinel-trace.log")
    else {
        return;
    };
    let mut line = String::new();
    let _ = writeln!(
        &mut line,
        "[{:?} pid={}] {tag}: {detail}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        std::process::id()
    );
    let _ = f.write_all(line.as_bytes());
}

#[cfg(not(unix))]
pub fn run_child() -> ! {
    std::process::exit(0)
}

#[cfg(unix)]
fn sentinel_core<R: Read, W: Write>(
    wake: &mut R,
    ready: &mut W,
    restore: &mut dyn FnMut() -> (bool, bool),
) -> SentinelOutcome {
    let _ = ready.write_all(READY_BYTE);
    let _ = ready.flush();
    match wait_wake(wake) {
        Ok(true) => SentinelOutcome::CleanExit,
        Ok(false) | Err(_) => {
            let (termios_restored, seq_written) = restore();
            SentinelOutcome::ParentDeath {
                termios_restored,
                seq_written,
            }
        }
    }
}

#[cfg(unix)]
fn wait_wake<R: Read>(r: &mut R) -> io::Result<bool> {
    let mut b = [0u8; 1];
    loop {
        match r.read(&mut b) {
            Ok(0) => return Ok(false),
            Ok(_) => return Ok(true),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
}

#[cfg(unix)]
fn restore_tty(tty: Option<&std::fs::File>, saved: Option<&str>) -> (bool, bool) {
    let Some(mut t) = tty else {
        return (false, false);
    };
    let termios_restored = saved.is_some_and(|s| stty_restore(t, s));
    let seq_written = t
        .write_all(RESTORE_SEQUENCE.as_bytes())
        .and_then(|()| t.flush())
        .is_ok();
    (termios_restored, seq_written)
}

#[cfg(unix)]
fn stty_save(tty: &std::fs::File) -> Option<String> {
    let stdin = tty.try_clone().ok()?;
    let out = Command::new("stty").arg("-g").stdin(stdin).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[cfg(unix)]
fn stty_restore(tty: &std::fs::File, saved: &str) -> bool {
    let Ok(stdin) = tty.try_clone() else {
        return false;
    };
    Command::new("stty")
        .arg(saved)
        .stdin(stdin)
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(all(test, unix))]
mod tests {
    use std::io::pipe;

    use super::*;

    #[test]
    fn restore_sequence_mirrors_crossterm_0_28_disable_set() {
        assert_eq!(
            RESTORE_SEQUENCE,
            "\u{1b}[?1006l\u{1b}[?1015l\u{1b}[?1003l\u{1b}[?1002l\u{1b}[?1000l\u{1b}[?2004l\u{1b}[?1049l"
        );
        assert!(
            !RESTORE_SEQUENCE.contains("?1000h"),
            "只含关闭（l）不含开启（h）"
        );
    }

    #[test]
    fn wait_wake_byte_means_clean_exit_and_eof_means_parent_death() {
        let (mut rx, mut tx) = pipe().unwrap();
        tx.write_all(b"R").unwrap();
        drop(tx);
        assert!(wait_wake(&mut rx).unwrap(), "收到字节 = 正常收尾");

        let (mut rx2, _tx2) = pipe().unwrap();
        drop(_tx2);
        assert!(!wait_wake(&mut rx2).unwrap(), "EOF = 父进程死亡");
    }

    #[test]
    fn sentinel_core_clean_exit_skips_restore() {
        let (mut rx, mut tx) = pipe().unwrap();
        tx.write_all(b"R").unwrap();
        drop(tx);
        let mut restored = 0;
        let mut out = Vec::new();
        let outcome = sentinel_core(&mut rx, &mut out, &mut || {
            restored += 1;
            (true, true)
        });
        assert_eq!(outcome, SentinelOutcome::CleanExit);
        assert_eq!(restored, 0, "正常收尾不触发还原");
        assert_eq!(out, READY_BYTE, "ready 握手字节先于等待发出");
    }

    #[test]
    fn sentinel_core_parent_death_invokes_restore() {
        let (mut rx, _) = pipe().unwrap();
        let mut out = Vec::new();
        let mut calls = 0;
        let outcome = sentinel_core(&mut rx, &mut out, &mut || {
            calls += 1;
            (true, true)
        });
        assert_eq!(
            outcome,
            SentinelOutcome::ParentDeath {
                termios_restored: true,
                seq_written: true
            }
        );
        assert_eq!(calls, 1, "死亡分支还原恰好一次");
    }

    #[test]
    fn sentinel_core_read_error_treated_as_parent_death() {
        struct Explode;
        impl Read for Explode {
            fn read(&mut self, _b: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("boom"))
            }
        }
        let mut out = Vec::new();
        let outcome = sentinel_core(&mut Explode, &mut out, &mut || (false, false));
        assert_eq!(
            outcome,
            SentinelOutcome::ParentDeath {
                termios_restored: false,
                seq_written: false
            }
        );
    }

    #[test]
    fn wait_ready_handshake_byte_then_eof() {
        let (rx, mut tx) = pipe().unwrap();
        tx.write_all(READY_BYTE).unwrap();
        drop(tx);
        assert!(wait_ready(rx), "握手字节 → 武装成功");

        let (rx2, tx2) = pipe().unwrap();
        drop(tx2);
        assert!(!wait_ready(rx2), "写端关闭（EOF）→ 武装失败");
    }

    #[test]
    fn restore_tty_none_tty_is_noop() {
        assert_eq!(restore_tty(None, None), (false, false));
        assert_eq!(restore_tty(None, Some("4:600:5:bf")), (false, false));
    }

    #[test]
    fn restore_tty_non_tty_file_writes_sequence_but_no_termios() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-sentinel-restore");
        let f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(dir.join("tty-probe"))
            .unwrap();
        assert_eq!(
            restore_tty(Some(&f), None),
            (false, true),
            "未保存 termios → 只写序列"
        );
        assert_eq!(
            restore_tty(Some(&f), Some("not-a-real-stty-code")),
            (false, true),
            "stty 失败不阻断序列写入"
        );
        drop(f);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn stty_save_and_restore_fail_without_tty() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-sentinel-stty");
        let f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .read(true)
            .open(dir.join("tty-probe"))
            .unwrap();
        assert_eq!(stty_save(&f), None, "非 tty 的 stty -g 必失败");
        assert!(!stty_restore(&f, "4:600:5:bf"), "非 tty 的 stty 还原必失败");
        drop(f);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sentinel_handle_release_and_drop_send_wake_byte() {
        let (mut rx, tx) = pipe().unwrap();
        let handle = TerminalSentinel {
            wake_tx: Some(tx),
            child: None,
        };
        handle.release();
        let mut b = [0u8; 1];
        rx.read_exact(&mut b).unwrap();
        assert_eq!(&b, READY_BYTE, "release 通知哨兵正常收尾");

        let (mut rx2, tx2) = pipe().unwrap();
        let handle2 = TerminalSentinel {
            wake_tx: Some(tx2),
            child: None,
        };
        drop(handle2);
        let mut b2 = [0u8; 1];
        rx2.read_exact(&mut b2).unwrap();
        assert_eq!(&b2, READY_BYTE, "Drop 兜底同样通知");
    }

    #[test]
    fn trace_env_gated_full_and_silent_paths() {
        let log = std::path::Path::new("/tmp/ezr-sentinel-trace.log");
        std::env::set_var("EZR_SENTINEL_TRACE", "1");
        trace("test: full", "written");
        assert!(log.exists(), "env 设置时写出追踪日志");
        let size_after_first = std::fs::metadata(log).map(|m| m.len()).unwrap_or(0);
        trace("test: second", "appended");
        let size_after_second = std::fs::metadata(log).map(|m| m.len()).unwrap_or(0);
        assert!(
            size_after_second > size_after_first,
            "第二次调用追加而非截断（append 语义）"
        );

        std::env::remove_var("EZR_SENTINEL_TRACE");
        let size_before_silent = std::fs::metadata(log).map(|m| m.len()).unwrap_or(0);
        trace("test: silent", "no side effect expected");
        let size_after_silent = std::fs::metadata(log).map(|m| m.len()).unwrap_or(0);
        assert_eq!(
            size_before_silent, size_after_silent,
            "未设 env 时零输出零副作用"
        );
        std::fs::remove_file(log).ok();
    }
}
