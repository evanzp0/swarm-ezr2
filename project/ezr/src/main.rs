//! main.rs — EZR Downloader 入口（01：HTTP/HTTPS 真实下载内核与 TUI 正式版）
//!
//! 职责：CLI 参数解析（FR-01-01）、单实例文件锁（FR-01-72）、配置加载、
//! 终端初始化（raw mode + alternate screen + bracketed paste FR-01-06 + 鼠标捕获）、
//! tokio 事件循环（键盘/鼠标/粘贴事件流 + 100ms tick）、优雅退出（恢复终端 + 保存状态，FR-01-73）、
//! 终端哨兵武装与退场（FR-01-84：异常死亡后终端自恢复，机制见 sentinel.rs）。
//!
//! 覆盖层（对话框/下拉浮层）出现或消失的那一帧必须全量重绘（CJK 宽字符
//! 半格覆盖问题，沿用 demo 定稿注释与方案）。
#![allow(missing_docs)] // 交互层：demo 定稿基线复用，接口文档见 model/engine 层
#![allow(clippy::multiple_crate_versions)] // 依赖树固有重复（reqwest/rustls 链条），无法单侧消除
#![allow(clippy::pedantic)] // 交互层字节/速度展示算术与 demo 基线风格豁免
#![allow(clippy::nursery)] // 同上
#![allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::too_many_arguments
)]
// 字节/速度/时间算术在 u64-f64 间转换是下载器领域固有；边界由调用方保证
// （architect 第三轮：模块级姿态块 16 份拷贝收敛至此——lint 姿态单源 crate 根）
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

mod app;
mod engine;
mod model;
mod sentinel;
mod ui;

#[cfg(test)]
mod property_tests;

use std::io;
use std::path::PathBuf;

use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event,
    EventStream, KeyEventKind,
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

/// 版本号随期号递进（FR-01-83）
pub const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-01");

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

/// CLI 参数形态
#[derive(Default)]
struct Cli {
    /// 位置参数 URL 列表（FR-01-01：启动参数添加任务）
    urls: Vec<String>,
    /// `-d` 保存目录
    dir: Option<String>,
    /// `-c` 并发数
    conns: Option<usize>,
    /// `-x <算法>=<校验码>`（算法下标，十六进制小写值）
    x: Option<(usize, String)>,
    /// `--max-speed`（B/s）
    max_speed: Option<u64>,
    /// `--help`
    help: bool,
    /// `--version`
    version: bool,
}

/// `-c` 并发数解析：缺失/非数字/超出 1–64 均报错（不静默钳制）
fn opt_conns(v: Option<&String>) -> Result<usize, String> {
    let v = v.ok_or("-c 需要并发数参数")?;
    let parsed = v
        .parse::<usize>()
        .map_err(|_| format!("-c 并发数非法（{v}）：应为 1–64 的整数"))?;
    // 范围校验（Gherkin 01-add-task-14：0/65 等 1–64 之外一律
    // 启动报错退出，与 abc 同语义，不静默钳制）
    if !(1..=64).contains(&parsed) {
        return Err(format!("-c 并发数非法（{v}）：应为 1–64 的整数"));
    }
    Ok(parsed)
}

/// `--max-speed` 速度解析：`parse_speed` 得 0 且原词非 0 开头 → 非法
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

/// 位置参数校验：未知旗标与非 http(s) URL 均启动报错（Gherkin 01-add-task-15：
/// 多 URL 部分非法全部拒绝）
fn pos_url(s: &str) -> Result<String, String> {
    if s.starts_with('-') {
        return Err(format!("未知参数（{s}），--help 查看用法"));
    }
    if !(s.starts_with("http://") || s.starts_with("https://")) {
        return Err(format!("URL 非法（仅支持 http/https）：{s}"));
    }
    Ok(s.to_string())
}

/// 手写 CLI 解析（参数集小，不引入 clap）；单选项解析见 [`opt_conns`]/[`opt_speed`]/[`pos_url`]
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
         快捷键（TUI 内）: A 添加 / Space 暂停·继续 / R 重试 / D 删除 / U·J 调序 /\n\
         C 清已完成 / G 图表 / Tab 切页签 / Q·Esc 退出"
    );
}

/// 对指定锁文件尝试 flock 独占锁定（打开失败向上传播；已锁 → Ok(None)）
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

/// 单实例锁落点目录：state 目录可用则用之；不可得（无主目录环境）回退系统
/// 临时目录。抽为纯函数使回退决策可单测（no-HOME 场景不入 env 类测试，避免
/// 与并行读 HOME 的测试竞态，口径同 model/config 测试注记）。
fn lock_dir(state: Option<PathBuf>) -> PathBuf {
    state.unwrap_or_else(std::env::temp_dir)
}

/// 单实例文件锁（FR-01-72：`<.ezr>/state/ezr.lock`，flock 独占；`.ezr` 根目录
/// 经 `EZR_HOME` 重定位，v1.3/D16——不同 EZR_HOME 的实例互不冲突）。
/// state 目录不可得（无主目录环境）或不可写（权限受限等退化环境）时回退
/// `<系统临时目录>/ezr.lock`：回退分支与正常分支同走 [`try_lock_path`] 的
/// 「打开 + flock」（原回退分支仅 `File::create` 无锁，二次启动防重在无主目录
/// 环境失效——操作者裁决已修；不可写目录回退为 QA 终局轮 01-config-template-04
/// 缺陷修复，TDD 回归测试锁定）。
/// 锁文件句柄保持打开直至进程退出；`Ok(None)` = 已有实例在运行。
fn acquire_instance_lock() -> std::io::Result<Option<std::fs::File>> {
    acquire_lock_at(&[lock_dir(state_dir()), std::env::temp_dir()])
}

/// 按候选落点序列获取单实例锁（FR-01-72 v1.4 回退口径扩展，01-config-template-04
/// 「配置所在目录不可写时启动不报错不崩溃」）：逐落点「create_dir_all + 打开 +
/// flock」，首选落点不可得或不可写（含权限受限）时回退下一落点（末位恒为
/// `<系统临时目录>`）；全部失败才 Err（main 据此报错退出）。
/// 回退分支与正常分支共用同一「打开 + flock」收口（engineering.md 回退分支条款）。
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
    // 哨兵子进程模式（FR-01-84 内部协议）：短路与 TUI 主流程完全隔离——
    // 不装 panic hook、不碰单实例锁/配置/注册表，只守护终端状态
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

    // 单实例保护（FR-01-72/AC-11）：二次启动提示并退出（非零退出码）
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

    // 配置模板自动生成（FR-01-85，v1.4）：配置文件不存在时按默认值生成全注释
    // 模板（每键注明用途与取值范围）；已存在（含损坏）不覆写，生成失败静默
    // 跳过（D17）——便利性增强不阻塞启动，下方配置加载按 FR-01-71 缺失/回退口径。
    if let Some(p) = config_path() {
        let _ = ensure_default_config(&p);
    }

    // 配置加载 + `--max-speed` 参数优先（FR-01-60）
    let mut cfg = config_path().map(|p| Config::load(&p)).unwrap_or_default();
    if let Some(ms) = cli.max_speed {
        cfg.max_speed = ms;
    }

    // 注册表路径
    let registry_path: String = state_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("registry.json")
        .to_string_lossy()
        .to_string();

    let mut app = App::new(cfg, registry_path);

    // CLI 启动参数添加任务（FR-01-01/D6）
    for url in &cli.urls {
        let checksum = cli.x.as_ref().map(|(idx, v)| Checksum {
            algo: checksum::CHECKSUM_ALGOS[*idx].0,
            value: v.clone(),
        });
        app.add_cli_task(url.clone(), cli.dir.clone(), cli.conns, checksum);
    }

    // 终端哨兵（FR-01-84）：TUI 前武装——父进程异常死亡（含 kill -9）时由哨兵
    // 还原 termios 并写终端复原序列；武装失败（非 unix/无 stty 等）仅失去保护，
    // 不阻塞 TUI。哨兵子进程经 ready 握手后才允许 enable_raw_mode（防竞态）。
    let sentinel = sentinel::arm();

    let result = run_tui(&mut app).await;

    // 哨兵退场：终端已在 run_tui 内全路径复原 → 通知静默退出（收到字节才退，
    // EOF 才还原）；此后异常死亡也仅是冗余还原，无副作用。
    // Option<..> 保有内部 Drop：panic unwind 经 Drop 同样通知，哨兵不会误判。
    if let Some(s) = sentinel {
        s.release();
    }
    // 优雅退出收尾（FR-01-73）：停传保留断点 → 保存注册表
    app.shutdown().await;
    result
}

/// TUI 会话：终端初始化（raw mode + 备用屏幕 + 鼠标 + bracketed paste）、
/// 事件循环、全路径终端复原（正常/错误/panic hook 之外的错误分支也复原）。
async fn run_tui(app: &mut App) -> std::io::Result<()> {
    // 本 Demo 的界面语义完全依赖配色（状态色/协议徽标/分块图示），
    // 无视 NO_COLOR 环境变量强制输出颜色（沿用 demo 定稿决定）
    crossterm::style::force_color_output(true);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(e) = execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    ) {
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

/// 覆盖层翻转检测（沿用 demo：对话框/下拉浮层出现或消失那一帧全量重绘）
fn overlay_sig(app: &App) -> u8 {
    match app.dialog.as_ref() {
        None => 0,
        Some(d) => 1 | ((d.ck_open as u8) << 1),
    }
}

async fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> std::io::Result<()> {
    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(100));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut last_overlay = overlay_sig(app);

    loop {
        let sig = overlay_sig(app);
        if sig != last_overlay {
            terminal.clear()?;
            last_overlay = sig;
        }

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
                        // 过滤按键释放事件（部分终端两者都上报）
                        if key.kind == KeyEventKind::Press {
                            app.on_key(key.code, key.modifiers);
                        }
                    }
                    Some(Ok(Event::Mouse(m))) => {
                        app.on_mouse(m);
                    }
                    Some(Ok(Event::Paste(text))) => {
                        // bracketed paste（FR-01-06）：对话框文本字段整体接收
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
        assert!(parse_cli(&args(&["-c"])).is_err()); // 缺值
    }

    #[test]
    fn parse_cli_max_speed_zero_and_invalid() {
        assert_eq!(
            parse_cli(&args(&["--max-speed", "0"])).unwrap().max_speed,
            Some(0)
        );
        assert!(parse_cli(&args(&["--max-speed", "nonsense"])).is_err());
        assert!(parse_cli(&args(&["--max-speed"])).is_err()); // 缺值
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
        assert!(parse_cli(&args(&["-x"])).is_err()); // 缺值
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
        assert!(pos_url("-d").is_err()); // 旗标形状先于 URL 校验
        assert!(opt_conns(None).is_err());
        assert!(opt_speed(None).is_err());
    }

    #[test]
    fn instance_lock_excludes_second_holder_then_releases() {
        let first = acquire_instance_lock().unwrap();
        assert!(first.is_some());
        let second = acquire_instance_lock().unwrap();
        assert!(second.is_none(), "同进程第二个打开描述符应锁冲突");
        drop(first); // 释放 → 可再次获取
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

    /// 回退落点决策（纯函数）：state 目录缺失 → 系统临时目录（回退分支与正常
    /// 分支同走 try_lock_path「打开 + flock」，本测试锁定回退决策点本身；
    /// no-HOME 场景不入 env 类测试——与并行读 HOME 的测试竞态，见 config 测试注记）
    #[test]
    fn lock_dir_falls_back_to_temp_without_state() {
        assert_eq!(lock_dir(None), std::env::temp_dir());
        let custom = std::path::PathBuf::from("/tmp/ezr-state-lockdir-probe");
        assert_eq!(lock_dir(Some(custom.clone())), custom);
    }

    /// 不可写 state 目录的锁回退（01-config-template-04：配置所在目录不可写
    /// 时启动不报错不崩溃）：首选落点锁定失败 → 回退落点应成功上锁。
    /// 失败先行（TDD）：旧实现仅回退 no-HOME 形态，不可写目录直接 Err 退出。
    #[test]
    fn instance_lock_falls_back_when_state_dir_unwritable() {
        let base = crate::model::testenv::uniq_tmp_dir("ezr-lock-ro");
        let ro = base.join("ro-state");
        std::fs::create_dir_all(&ro).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
        let alt = base.join("alt-state");
        let res = acquire_lock_at(&[ro.clone(), alt.clone()]);
        // 清理权限后再删除（删只读目录内容会失败）
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
        let locked = res.expect("首选落点不可写应回退第二落点而非报错退出");
        assert!(locked.is_some(), "回退落点应成功上锁");
        drop(locked);
        std::fs::remove_dir_all(&base).ok();
    }

    #[tokio::test]
    async fn overlay_sig_tracks_dialog_and_dropdown() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ov");
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut app = App::new(Config::default(), reg);
        assert_eq!(overlay_sig(&app), 0);
        app.open_add_dialog();
        assert_eq!(overlay_sig(&app), 1);
        if let Some(d) = app.dialog.as_mut() {
            d.ck_open = true;
        }
        assert_eq!(overlay_sig(&app), 3);
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
