//! main.rs — EZR Downloader TUI Demo 入口
//!
//! 终端初始化（raw mode + alternate screen + 鼠标捕获）、tokio 事件循环
//! （键盘/鼠标事件流 + 100ms 模拟 tick）、退出时恢复终端。

mod app;
mod scrollbar;
mod ui;

use std::io;

use crossterm::{
    event::{
        poll, read, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste,
        EnableMouseCapture, Event, EventStream, KeyEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::App;

fn restore_terminal() {
    let _ = restore_into(&mut io::stdout());
}

/// 终端复原的可测内核（同步产品 v137 修复）：接受任意 Write，测试以
/// `Vec<u8>` 断言关闭命令的字节顺序。
/// 顺序（鼠标组最先）：`DisableMouseCapture → DisableBracketedPaste →
/// LeaveAlternateScreen`——鼠标上报停止得越早，disable 传播延迟窗口内
/// 新产生的鼠标事件字节越少（退出竞态最小化；各关闭命令相互独立，无栈
/// 顺序约束，crossterm 0.28 逐字序列不变）。
fn restore_into<W: std::io::Write>(w: &mut W) -> std::io::Result<()> {
    drain_input_events();
    let _ = disable_raw_mode();
    execute!(
        w,
        DisableMouseCapture,
        DisableBracketedPaste,
        LeaveAlternateScreen
    )
}

/// 退出前消费输入队列中已到达的事件字节（q 退出后鼠标序列泄漏到 shell
/// 的修复，与产品 v137 同步）：按 q 退出瞬间终端可能已发出鼠标事件字节
/// （SGR any-event 模式下 motion 事件高频，如 `ESC[<35;56;26M`），restore
/// 只阻止后续上报，不消费已排队字节——进程退出后它们被继承 tty 的 shell
/// （zle）当作输入回显成乱码。在 raw mode 仍开启时以短超时 poll 清空队列。
///
/// poll 超时取 5ms 而非 0，两个理由：
/// ① crossterm 的 Ok(false) 是个混合语义——带超时的 poll 先
/// `try_lock_internal_event_reader_for(timeout)`，锁竞争立即失败同样返回
/// Ok(false)，并非查过队列（EventStream 的常驻后台线程在
/// `poll_internal(None, …)` 中持锁阻塞等事件，Drop 只置 shutdown 标志
/// 不 join，drain 首轮可能撞上锁交接尾巴）；给 5ms 预算即先等到锁、
/// 再按队列真实状态判定。
/// ② pty/终端的字节从设备到 line discipline 输入队列有微秒级异步刷入
/// 延迟，0ms 轮询只看一次就退，5ms 给在途字节一个着陆窗口
/// （非规范模式字节入队即可读；无 tty 环境 poll 报错即视为无事件）。
fn drain_input_events() {
    // 上限护栏：异常积压也不阻塞退出（10_000 远超正常积压）
    for _ in 0..10_000 {
        match poll(std::time::Duration::from_millis(5)) {
            Ok(true) => {
                if read().is_err() {
                    break;
                }
            }
            _ => break, // Ok(false) 队列空 / Err 无 tty（测试与 CI 环境）
        }
    }
}

fn setup_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    setup_panic_hook();

    // 本 Demo 的界面语义完全依赖配色（状态色/协议徽标/分块图示），
    // 因此无视 NO_COLOR 环境变量强制输出颜色（CI/沙箱环境常设 NO_COLOR=1，
    // 而 crossterm 0.28 会遵循 no-color.org 约定关闭全部颜色输出）。
    crossterm::style::force_color_output(true);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let result = run(&mut terminal, &mut app).await;

    restore_terminal();
    result
}

async fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> std::io::Result<()> {
    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(100));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // 浮层翻转（打开/关闭对话框、展开/收起下拉）不再 terminal.clear() 全屏
    // 清屏重绘——清屏会在浮层出现/消失的瞬间清空整个终端，造成整屏闪烁
    // （打开新窗口闪烁修复）。
    //
    // 历史背景：底层列表文本含 CJK 宽字符，浮层边框恰好落在某宽字符右半格
    // 上时，增量 diff 会跳过被覆盖的尾格，宽字残留在浮层边框上且被后续帧
    // 固化——旧版用 clear() 全屏重绘掩盖该残影。现残影已由渲染层
    // `clip_wide_at_edges`（ui.rs，每处浮层 Clear 后调用）在缓冲区层根治：
    // 左缘宽字符主格截断、右缘孤立半格补空格，前后帧模型自洽，diff 输出的
    // 连续两格覆写与终端宽字符语义一致，开/关两个方向均无残影，全屏清屏
    // 随之不再需要（scripts/check_overlay_redraw.py 已改断言「翻转零清屏」）。
    loop {
        // 浮层翻转（打开/关闭对话框、展开/收起下拉）：diff 直接绘制，无清屏
        terminal.draw(|f| ui::draw(f, app))?;

        tokio::select! {
            _ = ticker.tick() => {
                app.tick();
                if app.quit {
                    break;
                }
            }
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        // 过滤掉按键释放事件（部分终端两者都会上报）
                        if key.kind == KeyEventKind::Press {
                            app.on_key(key.code, key.modifiers);
                        }
                    }
                    Some(Ok(Event::Mouse(m))) => {
                        app.on_mouse(m);
                    }
                    Some(Ok(Event::Paste(text))) => {
                        // bracketed paste（FR-01-06 同步）：对话框文本字段整体接收
                        app.on_paste(&text);
                    }
                    Some(Ok(_)) => {}
                    // 事件流结束（stdin 关闭）或错误：退出
                    _ => {
                        app.quit = true;
                    }
                }
                if app.quit {
                    break;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 同步产品 v137 回归（退出后鼠标序列泄漏到 shell）：复原序列必须
    /// **最先**关闭鼠标捕获。按 q 退出瞬间鼠标 motion 事件字节（SGR
    /// `ESC[<b;x;yM`）可能已排队在 tty 缓冲；旧顺序（Paste → Leave →
    /// Mouse）中鼠标关闭最后发出——disable 传播延迟窗口内终端还在上报
    /// 新事件，与已排队字节一起在进程退出后被 shell 读走，zle 回显成
    /// `^[[<35;56;26M` 类乱码。锁定新顺序：鼠标组关闭（?1006l … ?1000l）
    /// 先于备用屏离开（?1049l）与 paste 关闭（?2004l）出现；三组命令
    /// 逐字在位（crossterm 0.28）。
    #[test]
    fn restore_sequence_disables_mouse_before_leave_and_paste() {
        let mut buf: Vec<u8> = Vec::new();
        restore_into(&mut buf).expect("复原序列写入 Vec<u8> 不应报错");
        let s = String::from_utf8_lossy(&buf);
        let mouse_sgr_off = s.find("\u{1b}[?1006l").expect("SGR 鼠标关闭在位");
        let mouse_basic_off = s.find("\u{1b}[?1000l").expect("基础鼠标捕获关闭在位");
        let paste_off = s.find("\u{1b}[?2004l").expect("bracketed paste 关闭在位");
        let leave_alt = s.find("\u{1b}[?1049l").expect("备用屏离开在位");
        assert!(
            mouse_sgr_off < paste_off && mouse_sgr_off < leave_alt,
            "鼠标 SGR 关闭最先发出"
        );
        assert!(
            mouse_basic_off < paste_off && mouse_basic_off < leave_alt,
            "基础鼠标捕获关闭先于 paste/备用屏"
        );
        assert!(!s.contains("?1000h"), "复原序列只含关闭（l）不含启用（h）");
    }

    /// 配套：drain 对空队列/无事件源环境（测试与 CI 无 tty）非阻塞快速
    /// 返回——不挂死、不 panic；这是退出路径上的消费循环，任何阻塞都会
    /// 拖慢退出。
    #[test]
    fn drain_input_events_returns_promptly_without_events() {
        let start = std::time::Instant::now();
        drain_input_events();
        assert!(
            start.elapsed() < std::time::Duration::from_secs(2),
            "drain 非阻塞（实际 {:?}）",
            start.elapsed()
        );
    }
}
