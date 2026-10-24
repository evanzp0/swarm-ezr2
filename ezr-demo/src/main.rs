//! main.rs — EZR Downloader TUI Demo 入口
//!
//! 终端初始化（raw mode + alternate screen + 鼠标捕获）、tokio 事件循环
//! （键盘/鼠标事件流 + 100ms 模拟 tick）、退出时恢复终端。

mod app;
mod scrollable_panel;
mod scrollbar;
// 单行文本输入框组件（与产品 ezr/src/textfield.rs 同源收口：值+光标状态机 /
// 显示取窗 / 原生光标定位 / 点击列映射 / 宽度工具 w+truncate 单源）
mod textfield;
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

/// 鼠标关闭后的 settle 主窗口（同步产品 v150）：`DisableMouseCapture` 字节
/// flush 后，终端模拟器**异步**处理关闭序列——处理完成前 1003 any-event
/// 模式仍在上报，motion 事件字节持续生成。固定窗口等待（本地终端处理
/// 延迟毫秒级，50ms 远超之），把「关闭序列已发出但尚未生效」的生成窗口
/// 整个纳入等待，窗口内到达的字节随后由 drain 一并消费——进程退出时
/// tty 输入队列必空，shell 不再读到任何鼠标残序（`^[[<35;58;18M` 类
/// 乱码的根治，v137 的「drain 最先」顺序对该窗口无能为力）。
const MOUSE_SETTLE: std::time::Duration = std::time::Duration::from_millis(50);

/// 二轮 settle 短窗口（同步产品 v150）：首轮 drain 消费到事件（说明积压
/// 较大或关闭仍在传播）时追加等待并再消费一轮；首轮即空则单窗口收工
/// （常见退出路径总耗时 ≈50ms，不可感知）。
const MOUSE_SETTLE_RETRY: std::time::Duration = std::time::Duration::from_millis(20);

/// 终端复原的可测内核（同步产品 v150 重排：**鼠标关闭最先发出 → settle
/// → drain → raw 关 → 其余关闭**）：接受任意 Write，测试以 `Vec<u8>` 断言
/// 关闭命令的字节顺序、以 TimedWriter（tests 模块）断言相对 settle 窗口的
/// 时序。v137 的「drain 最先」顺序留有生成窗口：drain 之后、终端真正处理
/// `?1003l` 之前，鼠标上报仍在进行，新到字节排队在 tty 输入队列，进程
/// 退出后被 shell 回显成 `^[[<35;58;18M` 类乱码。v150 把鼠标关闭提到最前，
/// 用 settle 窗口覆盖 disable 传播延迟，窗口内字节随后全部消费——退出
/// 瞬间 tty 队列必空。各关闭命令相互独立，无栈顺序约束，crossterm 0.28
/// 逐字序列不变。错误容忍：两段 execute! 都执行（前段失败不跳过后续
/// 关闭命令），按序上报首个 Err。
fn restore_into<W: std::io::Write>(w: &mut W) -> std::io::Result<()> {
    let mouse_off = execute!(w, DisableMouseCapture);
    settle_and_drain();
    let _ = disable_raw_mode();
    let rest = execute!(w, DisableBracketedPaste, LeaveAlternateScreen);
    mouse_off.and(rest)
}

/// 退出收尾的「等待 + 消费」（同步产品 v150，取代 v137 的裸 drain）：
/// ① settle——等待终端真正处理鼠标关闭序列（[`MOUSE_SETTLE`]）；
/// ② drain——消费窗口内到达的事件字节；首轮消费到事件（积压较大或关闭
/// 仍在传播）则追加一轮短窗口再消费（[`MOUSE_SETTLE_RETRY`]）。
/// raw mode 由调用方在本函数**之后**关闭：canonical 模式下无换行的鼠标
/// 序列不可读（内核 line discipline 攒行等 EOL），drain 会失效——
/// 「raw 开着 drain、drain 完才关 raw」是本函数与调用方的顺序契约。
fn settle_and_drain() {
    std::thread::sleep(MOUSE_SETTLE);
    if drain_input_events() {
        std::thread::sleep(MOUSE_SETTLE_RETRY);
        drain_input_events();
    }
}

/// 消费输入队列中已到达的事件字节（同步产品 v150 契约升级：返回是否
/// 消费到事件）：q 退出后鼠标序列泄漏到 shell 的修复，与产品同步：
/// SGR any-event 模式下 motion 事件高频（如 `ESC[<35;56;26M`），restore
/// 只阻止后续上报，不消费已排队/在途字节——进程退出后它们被继承 tty 的
/// shell（zle）当作输入回显成乱码。在 raw mode 仍开启时以短超时 poll
/// 清空队列（非规范模式字节入队即可读；无 tty 环境 poll 报错即视为无事件）。
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
fn drain_input_events() -> bool {
    let mut consumed = false;
    // 上限护栏：异常积压也不阻塞退出（10_000 远超正常积压；有数据时
    // poll 立即返回 true，循环耗时受单次 read 主导而非 5ms 超时）
    for _ in 0..10_000 {
        match poll(std::time::Duration::from_millis(5)) {
            Ok(true) => {
                consumed = true;
                if read().is_err() {
                    break;
                }
            }
            _ => break, // Ok(false) 队列空 / Err 无 tty（测试与 CI 环境）
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

    // 本 Demo 的界面语义完全依赖配色（状态色/协议徽标/分块图示），
    // 因此无视 NO_COLOR 环境变量强制输出颜色（CI/沙箱环境常设 NO_COLOR=1，
    // 而 crossterm 0.28 会遵循 no-color.org 约定关闭全部颜色输出）。
    crossterm::style::force_color_output(true);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    // 输入框光标改用终端原生光标后，显式请求闪烁模式（对齐产品 v1.34：
    // Alacritty 等默认稳态终端不请求不闪）；restore 不动——离开备用屏即
    // 复位终端默认光标态
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableMouseCapture,
        crossterm::cursor::EnableBlinking
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

    /// 同步产品 v137→v150 回归（退出后鼠标序列泄漏到 shell）：复原序列
    /// 必须**最先**关闭鼠标捕获。按 q 退出瞬间鼠标 motion 事件字节（SGR
    /// `ESC[<b;x;yM`）可能已排队在 tty 缓冲；旧顺序（Paste → Leave →
    /// Mouse）中鼠标关闭最后发出——disable 传播延迟窗口内终端还在上报
    /// 新事件，与已排队字节一起在进程退出后被 shell 读走，zle 回显成
    /// `^[[<35;56;26M` 类乱码。锁定顺序：鼠标组关闭（?1006l … ?1000l）
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

    /// 记录每次 write 时刻的探针 Writer（同步产品 v150 时序锁定）：让
    /// restore_into 的字节流自带时间戳，验证「鼠标关闭在 settle 窗口前
    /// 写出、paste/备用屏关闭在窗口后写出」的相对时序。
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

    /// 同步产品 v150 回归（退出竞态根治的时序契约）：`restore_into` 必须
    /// ① 鼠标关闭序列在 settle 窗口开启**之前**写出；② paste/备用屏
    /// 关闭在 settle 窗口**之后**写出（settle 先等终端处理 `?1003l`，
    /// 窗口内新到的鼠标字节才有机会被 drain 消费；若回退为「先 drain
    /// 后关鼠标」，生成窗口重开，shell 再次读到鼠标残序）。睡眠保证
    /// ≥50ms，断言取 45ms 容差。
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

    /// 配套（同步产品 v150 契约升级）：drain 对空队列/无事件源环境（测试
    /// 与 CI 无 tty）非阻塞快速返回且**未消费**（返回 false）——不挂死、
    /// 不 panic；这是退出路径上的消费循环，任何阻塞都会拖慢退出。
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

    /// 同步产品 v150 配套：settle_and_drain 在无事件环境至少等待主 settle
    /// 窗口（覆盖 disable 传播延迟的契约本体），且首轮无消费不追加二轮
    /// （总耗时 ≈ MOUSE_SETTLE，退出路径不加无谓延迟）。
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
