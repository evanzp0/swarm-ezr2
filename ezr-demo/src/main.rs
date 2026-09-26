//! main.rs — EZR Downloader TUI Demo 入口
//!
//! 终端初始化（raw mode + alternate screen + 鼠标捕获）、tokio 事件循环
//! （键盘/鼠标事件流 + 100ms 模拟 tick）、退出时恢复终端。

mod app;
mod ui;

use std::io;

use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyEventKind,
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

    loop {
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
