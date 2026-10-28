mod app;
mod multiline;
mod scrollable_panel;
mod scrollbar;

mod textfield;
mod ui;

use std::io;

use crossterm::{
    event::{
        poll, read, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste,
        EnableMouseCapture, Event, EventStream, KeyboardEnhancementFlags, KeyEventKind,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
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

const MOUSE_SETTLE: std::time::Duration = std::time::Duration::from_millis(50);

const MOUSE_SETTLE_RETRY: std::time::Duration = std::time::Duration::from_millis(20);

fn restore_into<W: std::io::Write>(w: &mut W) -> std::io::Result<()> {
    let kbd_off = execute!(w, PopKeyboardEnhancementFlags);
    let mouse_off = execute!(w, DisableMouseCapture);
    settle_and_drain();
    let _ = disable_raw_mode();
    let rest = execute!(w, DisableBracketedPaste, LeaveAlternateScreen);
    kbd_off.and(mouse_off).and(rest)
}

fn settle_and_drain() {
    std::thread::sleep(MOUSE_SETTLE);
    if drain_input_events() {
        std::thread::sleep(MOUSE_SETTLE_RETRY);
        drain_input_events();
    }
}

fn drain_input_events() -> bool {
    let mut consumed = false;

    for _ in 0..10_000 {
        match poll(std::time::Duration::from_millis(5)) {
            Ok(true) => {
                consumed = true;
                if read().is_err() {
                    break;
                }
            }
            _ => break,
        }
    }
    consumed
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

    crossterm::style::force_color_output(true);

    enable_raw_mode()?;
    let mut stdout = io::stdout();

    execute!(
        stdout,
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableMouseCapture,
        crossterm::cursor::EnableBlinking,
        // disambiguate escape codes：支持 kitty 键盘协议的终端上 Shift+Enter
        // 可区分上报（命令窗口换行）；不支持的终端降级为 Ctrl+J 换行
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        )
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

                        if key.kind == KeyEventKind::Press {
                            app.on_key(key.code, key.modifiers);
                        }
                    }
                    Some(Ok(Event::Mouse(m))) => {
                        app.on_mouse(m);
                    }
                    Some(Ok(Event::Paste(text))) => {

                        app.on_paste(&text);
                    }
                    Some(Ok(_)) => {}

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

    struct TimedWriter {
        buf: Vec<u8>,
        start: std::time::Instant,
        marks: Vec<(std::time::Duration, String)>,
    }

    impl std::io::Write for TimedWriter {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.marks.push((
                self.start.elapsed(),
                String::from_utf8_lossy(b).into_owned(),
            ));
            self.buf.extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn restore_into_writes_mouse_off_before_settle_and_rest_after() {
        let mut w = TimedWriter {
            buf: Vec::new(),
            start: std::time::Instant::now(),
            marks: Vec::new(),
        };
        restore_into(&mut w).expect("复原序列写入 TimedWriter 不应报错");
        let ts_of = |needle: &str, label: &str| {
            w.marks
                .iter()
                .find(|(_, s)| s.contains(needle))
                .map(|(t, _)| *t)
                .unwrap_or_else(|| panic!("{label} 序列在位"))
        };
        let mouse_ts = ts_of("?1006l", "SGR 鼠标关闭");
        let paste_ts = ts_of("?2004l", "paste 关闭");
        let alt_ts = ts_of("?1049l", "备用屏离开");
        let tolerance = std::time::Duration::from_millis(45);
        assert!(
            mouse_ts < tolerance,
            "鼠标关闭先于 settle 窗口写出（实际 {mouse_ts:?}）"
        );
        assert!(
            paste_ts >= tolerance && alt_ts >= tolerance,
            "paste/备用屏关闭晚于 settle 窗口（实际 paste {paste_ts:?} / alt {alt_ts:?}）"
        );
        assert!(paste_ts <= alt_ts, "paste 关闭先于（或同批）备用屏离开");
    }

    #[test]
    fn drain_input_events_returns_false_promptly_without_events() {
        let start = std::time::Instant::now();
        let consumed = drain_input_events();
        assert!(!consumed, "无事件源/空队列不消费任何事件");
        assert!(
            start.elapsed() < std::time::Duration::from_secs(2),
            "drain 非阻塞（实际 {:?}）",
            start.elapsed()
        );
    }

    #[test]
    fn settle_and_drain_spends_primary_window_only_without_events() {
        let start = std::time::Instant::now();
        settle_and_drain();
        let e = start.elapsed();
        assert!(e >= MOUSE_SETTLE, "至少等待主 settle 窗口（实际 {e:?}）");
        assert!(
            e < MOUSE_SETTLE + MOUSE_SETTLE_RETRY + std::time::Duration::from_secs(2),
            "无事件时不追加二轮窗口（实际 {e:?}）"
        );
    }
}
