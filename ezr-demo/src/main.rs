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
    fn overlay_sig(app: &App) -> u64 {
        // FNV-1a 混合（替代旧位域打包：浮层增至三个后 64 位不再够用）。
        // 任一浮层开关 / 拖拽态 / 位置变化 → sig 变化 → terminal.clear()
        // 全量重绘，根除增量 diff 覆盖宽字符半格造成的残影
        const FNV_OFF: u64 = 0xcbf2_9ce4_8422_2325;
        let mut h = FNV_OFF;
        fn mix(h: &mut u64, v: u64) {
            for b in 0..8 {
                *h = (*h ^ ((v >> (b * 8)) & 0xFF)).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        let (dlg, ck, proxy) = match app.dialog.as_ref() {
            None => (0u64, 0u64, 0u64),
            Some(d) => (1, d.ck_open as u64, d.proxy_open as u64),
        };
        mix(&mut h, dlg | (ck << 1) | (proxy << 2));
        mix(
            &mut h,
            (app.pieces_open as u64)
                | ((app.dialog_drag.is_some() as u64) << 1)
                | ((app.help_open as u64) << 2)
                | ((app.help_drag.is_some() as u64) << 3),
        );
        let (px, py) = app.pieces_pos.unwrap_or((0, 0));
        mix(&mut h, ((px as u64) & 0xFFFF) | (((py as u64) & 0xFFFF) << 16));
        let (dx, dy) = app.dialog_pos.unwrap_or((0, 0));
        mix(&mut h, ((dx as u64) & 0xFFFF) | (((dy as u64) & 0xFFFF) << 16));
        let (hx, hy) = app.help_pos.unwrap_or((0, 0));
        mix(&mut h, ((hx as u64) & 0xFFFF) | (((hy as u64) & 0xFFFF) << 16));
        h
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
