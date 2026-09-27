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

    // 覆盖层（对话框/下拉浮层）出现或消失的那一帧必须全量重绘。
    //
    // 原因：底层列表文本含 CJK 宽字符（一格占两列）。当浮层边框恰好落在
    // 某个宽字符的右半格上时（如 112 列终端下添加对话框左边框 x=19 正好
    // 是列表行「排队第 1 位 · 等待空闲下载槽位」中“待”字的右半格），增量
    // diff 只会输出被改变的单个单元格（MoveTo + 空格/边框字符），部分终端
    // 无法正确处理“覆盖宽字符半格”的写入，宽字会残留在浮层边框之上
    // （表现为“等待空闲下…”盖住对话框左侧框线）。更糟糕的是后续帧 diff
    // 认为该区域无变化、不再重绘，残影被固化。
    // clear() 先清空终端网格并重置内部前后缓冲，下一帧全量输出：从左到右
    // 连续打印，宽字符打印后游标自然推进两格，与网格位置严格同步，
    // 彻底避免半格覆盖问题。
    fn overlay_sig(app: &App) -> u8 {
        match app.dialog.as_ref() {
            None => 0,
            Some(d) => 1 | ((d.ck_open as u8) << 1),
        }
    }
    let mut last_overlay = overlay_sig(app);

    loop {
        // 浮层翻转（打开/关闭对话框、展开/收起下拉）：先清屏强制全量重绘
        let sig = overlay_sig(app);
        if sig != last_overlay {
            terminal.clear()?;
            last_overlay = sig;
        }

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
