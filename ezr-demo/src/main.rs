//! main.rs — EZR Downloader TUI Demo 入口
//!
//! 终端初始化（raw mode + alternate screen + 鼠标捕获）、tokio 事件循环
//! （键盘/鼠标事件流 + 100ms 模拟 tick）、退出时恢复终端。

mod app;
mod ui;

use std::io;

use crossterm::{
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, EventStream, KeyEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::App;

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        DisableBracketedPaste,
        LeaveAlternateScreen,
        DisableMouseCapture
    );
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
