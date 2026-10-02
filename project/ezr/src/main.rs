//! main.rs — EZR Downloader 入口（01：HTTP/HTTPS 真实下载内核与 TUI 正式版）
//!
//! 职责：CLI 参数解析（FR-01-01）、单实例文件锁（FR-01-72）、配置加载、
//! 终端初始化（raw mode + alternate screen + bracketed paste FR-01-06 + 鼠标捕获）、
//! tokio 事件循环（键盘/鼠标/粘贴事件流 + 100ms tick）、优雅退出（恢复终端 + 保存状态，FR-01-73）。
//!
//! 覆盖层（对话框/下拉浮层）出现或消失的那一帧必须全量重绘（CJK 宽字符
//! 半格覆盖问题，沿用 demo 定稿注释与方案）。
#![allow(missing_docs)] // 交互层：demo 定稿基线复用，接口文档见 model/engine 层
#![allow(clippy::multiple_crate_versions)] // 依赖树固有重复（rcgen/reqwest 链条），无法单侧消除
#![allow(clippy::pedantic)] // 交互层字节/速度展示算术与 demo 基线风格豁免
#![allow(clippy::nursery)] // 同上
#![allow(clippy::cognitive_complexity, clippy::too_many_lines, clippy::too_many_arguments)]
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_wrap)]


mod app;
mod engine;
mod model;
mod ui;

use std::io;
use std::path::PathBuf;

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
use crate::model::config::{config_path, state_dir, Config};
use crate::model::{checksum, Checksum, Protocol, Task, unix_now};

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

/// 手写 CLI 解析（参数集小，不引入 clap）
fn parse_cli(args: &[String]) -> Result<Cli, String> {
    let mut cli = Cli { urls: Vec::new(), dir: None, conns: None, x: None, max_speed: None, help: false, version: false };
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "-h" | "--help" => cli.help = true,
            "-V" | "--version" => cli.version = true,
            "-d" | "--dir" => {
                i += 1;
                cli.dir = Some(args.get(i).ok_or("-d 需要目录参数")?.clone());
            }
            "-c" | "--concurrency" => {
                i += 1;
                let v = args.get(i).ok_or("-c 需要并发数参数")?;
                let parsed = v
                    .parse::<usize>()
                    .map_err(|_| format!("-c 并发数非法（{v}）：应为 1–64 的整数"))?;
                // 范围校验（Gherkin 01-add-task-14：0/65 等 1–64 之外一律
                // 启动报错退出，与 abc 同语义，不静默钳制）
                if !(1..=64).contains(&parsed) {
                    return Err(format!("-c 并发数非法（{v}）：应为 1–64 的整数"));
                }
                cli.conns = Some(parsed);
            }
            "-x" | "--checksum" => {
                i += 1;
                let v = args.get(i).ok_or("-x 需要 <算法>=<校验码> 参数")?;
                cli.x = Some(checksum::parse_cli_x(v)?);
            }
            "--max-speed" => {
                i += 1;
                let v = args.get(i).ok_or("--max-speed 需要速度参数（如 2 MB/s）")?;
                let s = crate::model::config::parse_speed(v);
                if s == 0 && !v.trim().starts_with('0') {
                    return Err(format!("--max-speed 速度非法（{v}）：示例 2 MB/s / 500 KB/s"));
                }
                cli.max_speed = Some(s);
            }
            other => {
                if other.starts_with('-') {
                    return Err(format!("未知参数（{other}），--help 查看用法"));
                }
                // URL 参数校验（Gherkin 01-add-task-15：多 URL 部分非法
                // 全部拒绝——任一 URL 非 http(s) 即启动报错退出）
                if !(other.starts_with("http://") || other.starts_with("https://")) {
                    return Err(format!("URL 非法（仅支持 http/https）：{other}"));
                }
                cli.urls.push(other.to_string());
            }
        }
        i += 1;
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

/// 单实例文件锁（FR-01-72：`~/.ezr/state/ezr.lock`，flock 独占）。
/// 锁文件句柄保持打开直至进程退出；`Ok(None)` = 已有实例在运行。
fn acquire_instance_lock() -> std::io::Result<Option<std::fs::File>> {
    let Some(dir) = state_dir() else {
        return Ok(Some(std::fs::File::create(std::env::temp_dir().join("ezr.lock"))?));
    };
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("ezr.lock");
    let f = std::fs::OpenOptions::new().create(true).write(true).truncate(false).open(path)?;
    use fs2::FileExt;
    match f.try_lock_exclusive() {
        Ok(()) => Ok(Some(f)),
        Err(_) => Ok(None),
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
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

    // 本 Demo 的界面语义完全依赖配色（状态色/协议徽标/分块图示），
    // 无视 NO_COLOR 环境变量强制输出颜色（沿用 demo 定稿决定）
    crossterm::style::force_color_output(true);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, &mut app).await;

    restore_terminal();
    // 优雅退出收尾（FR-01-73）：停传保留断点 → 保存注册表
    app.shutdown().await;
    result
}

async fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> std::io::Result<()> {
    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(100));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // 覆盖层翻转检测（沿用 demo：对话框/下拉浮层出现或消失那一帧全量重绘）
    fn overlay_sig(app: &App) -> u8 {
        match app.dialog.as_ref() {
            None => 0,
            Some(d) => 1 | ((d.ck_open as u8) << 1),
        }
    }
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

impl App {
    /// CLI 启动参数直接添加任务（FR-01-01；与对话框共用校验与命名规则）
    pub fn add_cli_task(
        &mut self,
        url: String,
        dir: Option<String>,
        conns: Option<usize>,
        checksum: Option<Checksum>,
    ) {
        // URL 校验（FR-01-05）
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            self.set_toast(format!("⚠ 仅支持 http:// 或 https:// 链接（{url}）"));
            return;
        }
        let dir = dir
            .or_else(|| self.cfg.download_dir.clone())
            .unwrap_or_else(crate::model::config::default_download_dir)
            .trim_end_matches('/')
            .to_string();
        let base_name = crate::model::namegen::derive_name(None, None, &url);
        // 断点自动接续（FR-01-26）：同 URL 同路径的既有 sidecar → 沿用原名；
        // 否则重名检测（任务表 + 盘上）自动追加序号（与对话框添加同用
        // namegen::dedupe，口径一致）
        let resumed = crate::model::sidecar::Sidecar::load(&format!("{dir}/{base_name}.ezr"))
            .is_some_and(|s| s.url == url);
        let name = if resumed {
            base_name.clone()
        } else {
            let tasks = &self.tasks;
            crate::model::namegen::dedupe(&base_name, |n| {
                tasks.iter().any(|t| t.name == n && t.save_dir == dir)
                    || crate::model::namegen::exists_on_disk(&dir, n)
            })
        };
        let protocol = if url.starts_with("https://") { Protocol::Https } else { Protocol::Http };
        let ts = unix_now();
        let id = self.next_id;
        let t = Task::new_queued(
            id,
            name.clone(),
            protocol,
            url,
            dir,
            self.cfg.block_size_http,
            conns.unwrap_or(self.cfg.default_concurrency).clamp(1, 64),
            self.cfg.max_retries,
            checksum,
            ts,
        );
        self.next_id += 1;
        self.tasks.push(t);
        if resumed {
            self.set_toast(format!("✓ 已添加任务 #{id}: {name}（发现有效断点，将自动接续）"));
        } else {
            self.set_toast(format!("✓ 已添加任务 #{id}: {name}"));
        }
        self.save_registry();
    }
}

#[cfg(test)]
mod cli_add_tests {
    use super::*;
    use crate::model::config::Config;

    #[tokio::test]
    async fn cli_add_dedupes_duplicate_names() {
        let dir =
            std::env::temp_dir().join(format!("ezr-cli-add-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut app = App::new(Config::default(), reg);
        let url = "http://example.com/f.bin".to_string();
        let save = dir.to_string_lossy().to_string();
        app.add_cli_task(url.clone(), Some(save.clone()), None, None);
        app.add_cli_task(url, Some(save), None, None);
        assert_eq!(app.tasks.len(), 2);
        assert_eq!(app.tasks[0].name, "f.bin");
        // 同目录同名：第二个任务经 namegen::dedupe（任务表+盘上口径）追加序号
        assert_eq!(app.tasks[1].name, "f.bin.1");
        assert_eq!(app.next_id, 3);
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
