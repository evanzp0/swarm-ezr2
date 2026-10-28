#![allow(missing_docs)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]
#![allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::too_many_arguments
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

mod app;
mod bt;
mod engine;
mod model;
mod multiline;
mod scrollbar;
mod sentinel;
mod textfield;
mod ui;

#[cfg(test)]
mod property_tests;

use std::io;
use std::path::PathBuf;

use crossterm::event::{
    poll, read, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste,
    EnableMouseCapture, Event, EventStream, KeyEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::App;
use crate::model::config::{config_path, ensure_default_config, state_dir, Config};
use crate::model::{checksum, Checksum};

pub const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-01");

fn restore_terminal() {
    let _ = restore_into(&mut io::stdout());
}

fn enter_tui_sequence<W: std::io::Write>(w: &mut W) -> std::io::Result<()> {
    execute!(
        w,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste,
        crossterm::cursor::EnableBlinking
    )
}

const MOUSE_SETTLE: std::time::Duration = std::time::Duration::from_millis(50);

const MOUSE_SETTLE_RETRY: std::time::Duration = std::time::Duration::from_millis(20);

fn restore_into<W: std::io::Write>(w: &mut W) -> std::io::Result<()> {
    let mouse_off = execute!(w, DisableMouseCapture);
    settle_and_drain();
    let _ = disable_raw_mode();
    let rest = execute!(w, DisableBracketedPaste, LeaveAlternateScreen);
    mouse_off.and(rest)
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

#[derive(Default)]
struct Cli {
    urls: Vec<String>,
    dir: Option<String>,
    conns: Option<usize>,
    x: Option<(usize, String)>,
    max_speed: Option<u64>,
    help: bool,
    version: bool,
}

fn opt_conns(v: Option<&String>) -> Result<usize, String> {
    let v = v.ok_or("-c 需要并发数参数")?;
    let parsed = v
        .parse::<usize>()
        .map_err(|_| format!("-c 并发数非法（{v}）：应为 1–64 的整数"))?;
    if !(1..=64).contains(&parsed) {
        return Err(format!("-c 并发数非法（{v}）：应为 1–64 的整数"));
    }
    Ok(parsed)
}

fn opt_speed(v: Option<&String>) -> Result<u64, String> {
    let v = v.ok_or("--max-speed 需要速度参数（如 2 MB/s）")?;
    let s = crate::model::config::parse_speed(v);
    if s == 0 && !v.trim().starts_with('0') {
        return Err(format!(
            "--max-speed 速度非法（{v}）：示例 2 MB/s / 500 KB/s"
        ));
    }
    Ok(s)
}

fn pos_url(s: &str) -> Result<String, String> {
    if s.starts_with('-') {
        return Err(format!("未知参数（{s}），--help 查看用法"));
    }
    if !crate::model::is_http_url(s) {
        return Err(format!("URL 非法（仅支持 http/https）：{s}"));
    }
    Ok(s.to_string())
}

fn parse_cli(args: &[String]) -> Result<Cli, String> {
    let mut cli = Cli::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => cli.help = true,
            "-V" | "--version" => cli.version = true,
            "-d" | "--dir" => {
                cli.dir = Some(it.next().ok_or("-d 需要目录参数")?.clone());
            }
            "-c" | "--concurrency" => {
                cli.conns = Some(opt_conns(it.next())?);
            }
            "-x" | "--checksum" => {
                let v = it.next().ok_or("-x 需要 <算法>=<校验码> 参数")?;
                cli.x = Some(checksum::parse_cli_x(v)?);
            }
            "--max-speed" => {
                cli.max_speed = Some(opt_speed(it.next())?);
            }
            other => {
                cli.urls.push(pos_url(other)?);
            }
        }
    }
    Ok(cli)
}

fn print_help() {
    println!(
        "EZR Downloader v{VERSION} — 终端高性能多协议下载器\n\
         \n\
         用法: ezr [URL...] [选项]\n\
         \n\
         选项:\n\
           -d, --dir <目录>          保存目录（默认取配置 download_dir，缺省 ~/Downloads）\n\
           -c, --concurrency <n>     并发数 1–64（默认 4）\n\
           -x, --checksum <算法>=<校验码>\n\
                                     完整性校验（算法: md5/sha1/sha224/sha256/sha384/sha512/adler32）\n\
           --max-speed <速度>        全局限速（如 2 MB/s；0 = 不限）\n\
           -h, --help                显示本帮助\n\
           -V, --version             显示版本\n\
         \n\
         快捷键（TUI 内）: Tab 切面板 / ]·[·T 切页签 / A 添加 / Space 暂停·继续 /\n\
         R 重试 / D 删除 / U·J 调序 / C 清已完成 / G 收起右栏 / P 分块 /\n\
         B 断开（仅 BT）/ H 帮助 / Q 退出（Esc 仅关闭浮层）"
    );
}

fn try_lock_path(path: &std::path::Path) -> std::io::Result<Option<std::fs::File>> {
    let f = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)?;
    use fs2::FileExt;
    match f.try_lock_exclusive() {
        Ok(()) => Ok(Some(f)),
        Err(_) => Ok(None),
    }
}

fn lock_dir(state: Option<PathBuf>) -> PathBuf {
    state.unwrap_or_else(std::env::temp_dir)
}

fn acquire_instance_lock() -> std::io::Result<Option<std::fs::File>> {
    acquire_lock_at(&[lock_dir(state_dir()), std::env::temp_dir()])
}

fn acquire_lock_at(dirs: &[std::path::PathBuf]) -> std::io::Result<Option<std::fs::File>> {
    let mut last_err: Option<std::io::Error> = None;
    for dir in dirs {
        match std::fs::create_dir_all(dir).and_then(|()| try_lock_path(&dir.join("ezr.lock"))) {
            Ok(res) => return Ok(res),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::other("无可用的锁落点")))
}

fn main() -> std::io::Result<()> {
    if std::env::var_os(sentinel::ENV_SENTINEL).is_some() {
        sentinel::run_child();
    }
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(ezr_main())
}

async fn ezr_main() -> std::io::Result<()> {
    setup_panic_hook();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match parse_cli(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ezr: {e}");
            std::process::exit(2);
        }
    };
    if cli.help {
        print_help();
        return Ok(());
    }
    if cli.version {
        println!("ezr v{VERSION}");
        return Ok(());
    }

    let _lock = match acquire_instance_lock() {
        Ok(Some(f)) => f,
        Ok(None) => {
            eprintln!("ezr 已在运行");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("ezr: 无法创建实例锁（{e}）");
            std::process::exit(1);
        }
    };

    if let Some(p) = config_path() {
        let _ = ensure_default_config(&p);
    }

    let mut cfg = config_path().map(|p| Config::load(&p)).unwrap_or_default();
    if let Some(ms) = cli.max_speed {
        cfg.max_speed = ms;
    }

    let registry_path: String = state_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("registry.json")
        .to_string_lossy()
        .to_string();

    let mut app = App::new(cfg, registry_path);

    for url in &cli.urls {
        let checksum = cli.x.as_ref().map(|(idx, v)| Checksum {
            algo: checksum::CHECKSUM_ALGOS[*idx].0,
            value: v.clone(),
        });
        app.add_cli_task(url.clone(), cli.dir.clone(), cli.conns, checksum);
    }

    let sentinel = sentinel::arm();

    let result = run_tui(&mut app).await;

    if let Some(s) = sentinel {
        s.release();
    }
    app.shutdown().await;
    result
}

async fn run_tui(app: &mut App) -> std::io::Result<()> {
    crossterm::style::force_color_output(true);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(e) = enter_tui_sequence(&mut stdout) {
        restore_terminal();
        return Err(e);
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(e) => {
            restore_terminal();
            return Err(e);
        }
    };

    let result = run(&mut terminal, app).await;

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
                app.tick().await;
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
mod cli_parse_tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_cli_empty_defaults() {
        let c = parse_cli(&args(&[])).unwrap();
        assert!(c.urls.is_empty() && c.dir.is_none() && c.conns.is_none());
        assert!(c.x.is_none() && c.max_speed.is_none());
        assert!(!c.help && !c.version);
    }

    #[test]
    fn parse_cli_urls_collected_in_order() {
        let c = parse_cli(&args(&["http://a.com/1.bin", "https://b.com/2.bin"])).unwrap();
        assert_eq!(
            c.urls,
            vec![
                "http://a.com/1.bin".to_string(),
                "https://b.com/2.bin".to_string()
            ]
        );
    }

    #[test]
    fn parse_cli_all_flags() {
        let c = parse_cli(&args(&[
            "-d",
            "/tmp/dl",
            "-c",
            "8",
            "--max-speed",
            "2 MB/s",
            "http://h/f.bin",
            "-h",
            "-V",
        ]))
        .unwrap();
        assert_eq!(c.dir.as_deref(), Some("/tmp/dl"));
        assert_eq!(c.conns, Some(8));
        assert_eq!(c.max_speed, Some(2_000_000));
        assert_eq!(c.urls, vec!["http://h/f.bin".to_string()]);
        assert!(c.help && c.version);
    }

    #[test]
    fn parse_cli_long_forms_equivalent() {
        let a = parse_cli(&args(&[
            "--dir",
            "x",
            "--concurrency",
            "3",
            "--help",
            "--version",
        ]))
        .unwrap();
        assert_eq!(a.dir.as_deref(), Some("x"));
        assert_eq!(a.conns, Some(3));
        assert!(a.help && a.version);
    }

    #[test]
    fn parse_cli_conns_bounds_and_errors() {
        assert_eq!(parse_cli(&args(&["-c", "1"])).unwrap().conns, Some(1));
        assert_eq!(parse_cli(&args(&["-c", "64"])).unwrap().conns, Some(64));
        for bad in ["0", "65", "abc"] {
            assert!(parse_cli(&args(&["-c", bad])).is_err(), "conns {bad}");
        }
        assert!(parse_cli(&args(&["-c"])).is_err());
    }

    #[test]
    fn parse_cli_max_speed_zero_and_invalid() {
        assert_eq!(
            parse_cli(&args(&["--max-speed", "0"])).unwrap().max_speed,
            Some(0)
        );
        assert!(parse_cli(&args(&["--max-speed", "nonsense"])).is_err());
        assert!(parse_cli(&args(&["--max-speed"])).is_err());
    }

    #[test]
    fn parse_cli_checksum_flag() {
        let c = parse_cli(&args(&[
            "-x",
            "sha256=0123abcd0123abcd0123abcd0123abcd0123abcd0123abcd0123abcd0123abcd",
        ]))
        .unwrap();
        let (idx, v) = c.x.expect("x parsed");
        assert_eq!(checksum::CHECKSUM_ALGOS[idx].0, "SHA-256");
        assert_eq!(
            v,
            "0123abcd0123abcd0123abcd0123abcd0123abcd0123abcd0123abcd0123abcd"
        );
        assert!(parse_cli(&args(&["-x", "bad-format"])).is_err());
        assert!(parse_cli(&args(&["-x"])).is_err());
    }

    #[test]
    fn parse_cli_rejects_unknown_flag_and_bad_url() {
        assert!(parse_cli(&args(&["-z"])).is_err());
        assert!(parse_cli(&args(&["--bogus"])).is_err());
        assert!(parse_cli(&args(&["ftp://h/f.bin"])).is_err());
        assert!(parse_cli(&args(&["plain-name.bin"])).is_err());
    }

    #[test]
    fn pos_url_and_opt_helpers_direct() {
        assert!(pos_url("-d").is_err());
        assert!(opt_conns(None).is_err());
        assert!(opt_speed(None).is_err());
    }

    #[test]
    fn enter_tui_sequence_requests_blinking_after_screen_setup() {
        let mut buf: Vec<u8> = Vec::new();
        enter_tui_sequence(&mut buf).expect("进入序列写入 Vec<u8> 不应报错");
        let s = String::from_utf8_lossy(&buf);
        let enter_alt = s.find("\u{1b}[?1049h").expect("备用屏进入在位");
        let mouse_on = s.find("\u{1b}[?1000h").expect("鼠标捕获启用在位");
        let paste_on = s.find("\u{1b}[?2004h").expect("bracketed paste 启用在位");
        let blink_on = s.find("\u{1b}[?12h").expect("光标闪烁请求在位");
        assert!(
            enter_alt < mouse_on && mouse_on < paste_on && paste_on < blink_on,
            "进入序列顺序（屏→鼠标→paste→闪烁）: {s:?}"
        );
    }

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

    #[test]
    fn instance_lock_excludes_second_holder_then_releases() {
        let first = acquire_instance_lock().unwrap();
        assert!(first.is_some());
        let second = acquire_instance_lock().unwrap();
        assert!(second.is_none(), "同进程第二个打开描述符应锁冲突");
        drop(first);
        let third = acquire_instance_lock().unwrap();
        assert!(third.is_some());
    }

    #[test]
    fn try_lock_path_exclusive_on_fresh_file() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-lock");
        let p = dir.join("probe.lock");
        let a = try_lock_path(&p).unwrap();
        assert!(a.is_some());
        let b = try_lock_path(&p).unwrap();
        assert!(b.is_none());
        drop(a);
        let c = try_lock_path(&p).unwrap();
        assert!(c.is_some());
        drop(c);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn lock_dir_falls_back_to_temp_without_state() {
        assert_eq!(lock_dir(None), std::env::temp_dir());
        let custom = std::path::PathBuf::from("/tmp/ezr-state-lockdir-probe");
        assert_eq!(lock_dir(Some(custom.clone())), custom);
    }

    #[test]
    fn instance_lock_falls_back_when_state_dir_unwritable() {
        let base = crate::model::testenv::uniq_tmp_dir("ezr-lock-ro");
        let ro = base.join("ro-state");
        std::fs::create_dir_all(&ro).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
        let alt = base.join("alt-state");
        let res = acquire_lock_at(&[ro.clone(), alt.clone()]);
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
        let locked = res.expect("首选落点不可写应回退第二落点而非报错退出");
        assert!(locked.is_some(), "回退落点应成功上锁");
        drop(locked);
        std::fs::remove_dir_all(&base).ok();
    }

    #[tokio::test]
    async fn overlay_flip_redraws_via_diff_without_clear() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ovflip");
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut app = App::new(Config::default(), reg);
        for i in 0..3 {
            let mut t = crate::model::sample_task();
            t.id = 100 + i;
            t.name = format!("视频文件第{i}段超长名称测试宽字符边界.bin");
            t.state = crate::model::TaskState::Downloading;
            t.probed = true;
            t.total = 4_000_000;
            app.tasks.push(t);
        }
        app.selected = 0;
        app.toast = None;

        let screen = |t: &Terminal<TestBackend>| -> Vec<Vec<String>> {
            (0..34u16)
                .map(|y| {
                    (0..120u16)
                        .map(|x| {
                            t.backend()
                                .buffer()
                                .cell((x, y))
                                .expect("界内")
                                .symbol()
                                .to_string()
                        })
                        .collect()
                })
                .collect()
        };

        let mut term = Terminal::new(TestBackend::new(120, 34)).unwrap();
        term.draw(|f| ui::draw(f, &mut app)).unwrap();
        let before = screen(&term);

        app.open_add_dialog();
        term.draw(|f| ui::draw(f, &mut app)).unwrap();
        let s_open = term.backend().to_string();
        assert!(
            s_open.contains("添加任务：HTTP(S)"),
            "帧 2 对话框已绘制（v1.32 标题）"
        );
        let dlg = app.dlg_area.expect("帧 2 已回填对话框区域");
        for y in dlg.top()..dlg.bottom() {
            let x = dlg.x.saturating_sub(1);
            let left_sym = term
                .backend()
                .buffer()
                .cell((x, y))
                .expect("界内")
                .symbol()
                .to_string();
            let width = crate::ui::w(&left_sym);
            assert!(
                width <= 1,
                "浮层左缘外一格不得是会侵入浮层的宽字主格 @({x},{y}): {left_sym}"
            );
        }

        app.dialog = None;
        term.draw(|f| ui::draw(f, &mut app)).unwrap();
        let after = screen(&term);
        for (y, row) in before.iter().enumerate() {
            for (x, sym_before) in row.iter().enumerate() {
                if x > 0 && crate::ui::w(&row[x - 1]) > 1 {
                    continue;
                }
                let sym = &after[y][x];
                assert_eq!(sym, sym_before, "关帧恢复首帧（无残影） @({x},{y})");
            }
        }
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
