//! sentinel — 终端哨兵（FR-01-84，v1.3/D16）
//!
//! 背景：TUI 依赖 crossterm raw mode（`cfmakeraw` 会关闭 ISIG）+ 鼠标捕获 +
//! 备用屏幕。`kill -9` 在进程内无法捕获，异常死亡后终端残留三重损伤：
//! ① ISIG 关 → CTRL+C 变成普通输入字节，无法产生 SIGINT（「无法中断」）；
//! ② 终端模拟器仍处于鼠标上报模式，持续发送 SGR 鼠标事件 `ESC[<b;x;yM`
//!    （如 `32;64;10M`：32=移动标志+按住、64=列、10=行），被恢复的 shell
//!    回显成 `32;64;10M35;…` 转义字符残影；③ 备用屏幕未离开。
//!
//! 方案：TUI 启动前用同一二进制武装一个**存活哨兵子进程**（内部环境变量
//! [`ENV_SENTINEL`] 短路进 [`run_child`]，绝不触碰任务数据与单实例锁）：
//! 1. 子进程先捕获 cooked termios（`stty -g` 可移植编码），经 ready 管道
//!    握手后父进程才 `enable_raw_mode`（消除竞态）；
//! 2. 子进程阻塞等待 wake 管道：读到数据 = 父进程正常收尾（静默退出）；
//!    EOF = 父进程已死（kill -9 / 崩溃 / 任意异常退出均触发）→ 用保存的
//!    termios 还原并写 crossterm 复原序列后退出；
//! 3. 父进程正常收尾时先复原终端再 `release`（哨兵收到字节即不重复还原）。
//!
//! 零 unsafe（`unsafe_code = deny`）：termios 还原经 coreutils `stty -g`
//! 往返（Linux/macOS 皆内置）；管道用 `std::io::pipe`（1.87+），
//! `PipeReader/PipeWriter → Stdio` 转换为安全 API，无需裸 fd。
//! 父子 fd 隔离依赖 Rust `Command` 的 CLOEXEC 保证：哨兵不继承单实例锁
//! fd（kill -9 后重启不会被「已在运行」卡住，AC-2）。
#![allow(clippy::cognitive_complexity, clippy::too_many_lines)] // 握手/还原小状态机固有分支

use std::io::{self, Read, Write};
use std::process::Child;
#[cfg(unix)]
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

/// 哨兵子进程短路标记（内部协议，非用户面）：`Command.env` 只注入哨兵子进程，
/// 不污染用户环境；值为 1，只判存在性。
#[cfg(unix)]
pub const ENV_SENTINEL: &str = "EZR_INTERNAL_SENTINEL";

/// ready 握手字节（子进程 termios 捕获完成后发出）
const READY_BYTE: &[u8; 1] = b"R";

/// ready 握手超时：哨兵子进程（fork + /dev/tty + `stty -g`）正常毫秒级完成；
/// 超时视为武装失败，TUI 照常启动（仅失去保护，不阻塞主流程）。
const READY_TIMEOUT: Duration = Duration::from_secs(2);

/// 终端复原序列：逐字对应 crossterm 0.28.1 父进程正常收尾写入的三组关闭命令
/// （DisableBracketedPaste → LeaveAlternateScreen → DisableMouseCapture，
/// 鼠标组按其源码 `?1006l ?1015l ?1003l ?1002l ?1000l` 反序）；
/// 备用屏幕 `?1049l` 同时恢复进入前光标位置与可见性（DECSC 语义）。
#[cfg(unix)]
const RESTORE_SEQUENCE: &str = concat!(
    "\u{1b}[?2004l",
    "\u{1b}[?1049l",
    "\u{1b}[?1006l",
    "\u{1b}[?1015l",
    "\u{1b}[?1003l",
    "\u{1b}[?1002l",
    "\u{1b}[?1000l",
);

/// 哨兵子进程结局
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(unix)]
enum SentinelOutcome {
    /// 父进程正常收尾（收到 wake 字节）→ 不触碰终端
    CleanExit,
    /// 父进程死亡（EOF/读错误）→ 已尽力还原终端
    ParentDeath {
        /// termios 是否经 `stty` 还原成功
        termios_restored: bool,
        /// 复原序列是否成功写入 /dev/tty
        seq_written: bool,
    },
}

/// 武装终端哨兵（FR-01-84）：TUI 启动前调用。
/// 返回 `None` = 非 unix 或武装失败（TUI 照常运行，仅失去异常死亡保护；
/// 静默不打印——stderr 将进入 TUI 画面）。
#[must_use]
#[cfg(unix)]
pub fn arm() -> Option<TerminalSentinel> {
    // 武装失败仅失去保护，不阻塞 TUI（静默：stderr 将进入 TUI 画面）
    arm_inner().ok()
}

/// 非 unix 平台：无哨兵（Windows 控制台模式恢复机制不同，01 期交付平台
/// Linux x86_64，Windows 仅保证可编译，NFR-4）。
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
        .stdin(wake_rx) // wake 通道：父进程死亡 → EOF
        .stdout(ready_tx) // ready 通道：子进程 termios 捕获完成 → 握手字节
        .stderr(Stdio::inherit())
        // 独立进程组（关键）：会话首（父进程）死亡时内核向**前台进程组**广播
        // SIGHUP——哨兵若留在父进程组会被同波及死、还原代码无机会执行（本轮
        // 实测根因）；独立成组后 SIGHUP 只落在已死的前台组，哨兵凭管道 EOF
        // （而非信号）感知死亡，这正是设计语义
        .process_group(0)
        .spawn()?;
    let ok = wait_ready(ready_rx);
    trace("parent: handshake", &format!("{ok} pid={}", child.id()));
    if !ok {
        // 握手失败（子进程启动失败/超时）：放弃保护；wake_tx 随之 drop →
        // 子进程见 EOF 走还原分支（此时终端未被改动，写入序列无副作用）
        let _ = child.kill();
        let _ = child.wait();
        return Err(io::Error::other("sentinel ready handshake failed"));
    }
    Ok(TerminalSentinel {
        wake_tx: Some(wake_tx),
        child: Some(child),
    })
}

/// 等待 ready 握手（带超时；std 线程无 join 超时，故经 channel 转交结果，
/// 超时后残留的等待线程随子进程写入/死亡自然收束）。
#[cfg(unix)]
fn wait_ready(rx: std::io::PipeReader) -> bool {
    let (tx, rx_chan) = mpsc::channel();
    std::thread::spawn(move || {
        let mut rx = rx;
        let mut b = [0u8; 1];
        let ok = loop {
            match rx.read(&mut b) {
                Ok(0) => break false, // 子进程死亡
                Ok(_) => break true,  // 握手字节
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break false,
            }
        };
        let _ = tx.send(ok);
    });
    rx_chan.recv_timeout(READY_TIMEOUT).unwrap_or(false)
}

/// 终端哨兵句柄：持有 wake 管道写端与子进程句柄。
/// `release`/`Drop` 都会通知哨兵静默退出并回收（幂等）；因此任何退出路径
/// （含 panic unwind）都不会让哨兵误判父进程死亡。
#[cfg(unix)]
pub struct TerminalSentinel {
    wake_tx: Option<std::io::PipeWriter>,
    child: Option<Child>,
}

#[cfg(unix)]
impl TerminalSentinel {
    /// 正常收尾：通知哨兵静默退出并回收。须在终端复原**之后**调用
    /// （哨兵收到字节即不再触碰终端）。
    pub fn release(mut self) {
        self.wake_and_reap();
    }

    fn wake_and_reap(&mut self) {
        if let Some(mut tx) = self.wake_tx.take() {
            let _ = tx.write_all(READY_BYTE);
            // drop(tx) 关闭写端；哨兵若已收字节则早已退出，不受影响
        }
        if let Some(c) = self.child.as_mut() {
            let _ = c.wait(); // 回收，避免僵尸
        }
    }
}

#[cfg(unix)]
impl Drop for TerminalSentinel {
    fn drop(&mut self) {
        self.wake_and_reap();
    }
}

/// 非 unix 平台：无哨兵句柄（no-op）。
#[cfg(not(unix))]
pub struct TerminalSentinel;

#[cfg(not(unix))]
impl TerminalSentinel {
    /// 正常收尾（非 unix no-op）
    pub fn release(self) {}
}

/// 哨兵子进程入口（不返回）：main 中检测到 [`ENV_SENTINEL`] 时短路调用。
/// `std::process::exit(0)` 两种结局同码——哨兵的产物是终端状态，非自身状态。
#[cfg(unix)]
pub fn run_child() -> ! {
    // /dev/tty 必须**读写**打开（File::open 只读，write 会 EBADF；而 stty 的
    // ioctl 不受影响——这正是首轮排查被 “termios 已还原” 掩盖的原因）。
    // 与 crossterm 的终端访问口径一致（进程可能 stdio 已重定向）。
    let tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .ok();
    // 先于父进程 raw mode 捕获 cooked 状态（父进程在 ready 握手后才动终端）
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
    let _ = outcome;
    std::process::exit(0);
}

/// 会话诊断追踪（临时）：设 `EZR_SENTINEL_TRACE=1` 时逐事件追加
/// /tmp/ezr-sentinel-trace.log；未设时零输出（不进 TUI 画面）。
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

/// 非 unix 平台：哨兵模式不应被触发（`arm` 恒 None），触发则直接退出。
#[cfg(not(unix))]
pub fn run_child() -> ! {
    std::process::exit(0)
}

/// 哨兵核心决策（与真实 IO 解耦以便单测）：先 ready 握手，再等 wake——
/// 字节 = 正常收尾；EOF/读错误 = 父进程死亡，调用还原闭包。
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

/// 等 wake 管道：`Ok(true)` = 收到字节（正常收尾）；`Ok(false)` = EOF（父死亡）。
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

/// 还原终端：先 `stty <saved>` 还原 termios（TCSA 等价语义由 stty 决定），
/// 再写 crossterm 复原序列（鼠标捕获/备用屏幕/bracketed paste）。
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

/// `stty -g` 捕获当前 termios（可移植编码，跨平台往返由同一 stty 保证）
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

/// `stty <saved>` 还原 termios
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

    // 注：arm()/run_child() 涉及真实子进程与 /dev/tty，不入进程内单测
    // （current_exe 在 cargo test 下是测试器自身），由 pty 端到端验证兜底。

    #[test]
    fn restore_sequence_mirrors_crossterm_0_28_disable_set() {
        // 顺序镜像 main.rs restore_terminal()：DisableBracketedPaste →
        // LeaveAlternateScreen → DisableMouseCapture（鼠标组按 0.28.1 源码反序）
        assert_eq!(
            RESTORE_SEQUENCE,
            "\u{1b}[?2004l\u{1b}[?1049l\u{1b}[?1006l\u{1b}[?1015l\u{1b}[?1003l\u{1b}[?1002l\u{1b}[?1000l"
        );
        assert!(
            !RESTORE_SEQUENCE.contains("?1000h"),
            "只含关闭（l）不含开启（h）"
        );
    }

    #[test]
    fn wait_wake_byte_means_clean_exit_and_eof_means_parent_death() {
        // std::io::pipe() 返回（读端， 写端）
        let (mut rx, mut tx) = pipe().unwrap();
        tx.write_all(b"R").unwrap();
        drop(tx); // 先写后关：读端先得字节，EOF 不覆盖
        assert!(wait_wake(&mut rx).unwrap(), "收到字节 = 正常收尾");

        let (mut rx2, _tx2) = pipe().unwrap();
        drop(_tx2); // 写端关闭 → 读端 EOF
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
        // `_` 直接绑定：PipeWriter 即弃即 drop → 读端立刻 EOF
        //（写成 `_tx` 前缀绑定会存活到作用域结束，EOF 永不到来 → 挂起）
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
}
