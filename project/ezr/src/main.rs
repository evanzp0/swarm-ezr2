//! main.rs — EZR Downloader 入口（01：HTTP/HTTPS 真实下载内核与 TUI 正式版）
//!
//! 职责：CLI 参数解析（FR-01-01）、单实例文件锁（FR-01-72）、配置加载、
//! 终端初始化（raw mode + alternate screen + bracketed paste FR-01-06 +
//! 鼠标捕获 + 光标闪烁请求（v1.34 三批：输入框原生光标））、
//! tokio 事件循环（键盘/鼠标/粘贴事件流 + 100ms tick）、优雅退出（恢复终端 + 保存状态，FR-01-73）、
//! 终端哨兵武装与退场（FR-01-84：异常死亡后终端自恢复，机制见 sentinel.rs）。
//!
//! 覆盖层（对话框/下拉浮层/pieces 弹窗/帮助窗口）翻转零清屏（v1.31/FR-01-106⑧）：
//! 开/关/拖拽帧由增量 diff 直接绘制，不清屏；CJK 宽字符半格覆盖残影由
//! clip_wide_at_edges（ui/btn.rs，每处浮层 Clear 后调用）在缓冲区层根治。
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
// 通用滚动条引擎（v1.32/FR-01-109，对齐 ezr-demo m3 根级 scrollbar.rs +
// ui/scrollable_panel.rs 组合：几何/命中/反解/渲染单一实现，app 与 ui 共同消费）
mod scrollbar;
mod sentinel;
// 单行文本输入框组件（值+光标状态机 / 显示取窗 / 原生光标定位 / 点击列
// 映射 / 宽度工具 w+truncate 单源——对话框字段行与任何未来输入框共用，
// app 与 ui 共同消费；与 scrollbar 同级同风格的可复用组件模块）
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

/// 版本号随期号递进（FR-01-83）
pub const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-01");

fn restore_terminal() {
    let _ = restore_into(&mut io::stdout());
}

/// TUI 进入序列的可测内核（v1.34 三批 bug 修订：追加 `EnableBlinking`——
/// 输入框光标改用终端原生光标后，显式请求闪烁模式，光标可见性不再依赖
/// 终端默认值；不支持的终端忽略之，还原端不需要对称关闭——离开备用屏
/// 即恢复终端默认光标状态）。顺序即调用序：备用屏 → 鼠标捕获 →
/// bracketed paste → 闪烁请求（相互独立，无栈顺序约束）。
fn enter_tui_sequence<W: std::io::Write>(w: &mut W) -> std::io::Result<()> {
    execute!(
        w,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste,
        crossterm::cursor::EnableBlinking
    )
}

/// 鼠标关闭后的 settle 主窗口（v150）：`DisableMouseCapture` 字节 flush 后，
/// 终端模拟器**异步**处理关闭序列——处理完成前 1003 any-event 模式仍在
/// 上报，motion 事件字节持续生成。固定窗口等待（本地终端处理延迟毫秒级，
/// 50ms 远超之），把「关闭序列已发出但尚未生效」的生成窗口整个纳入等待，
/// 窗口内到达的字节随后由 drain 一并消费——进程退出时 tty 输入队列必空，
/// shell 不再读到任何鼠标残序（`^[[<35;58;18M` 类乱码的根治，v137 的
/// 「drain 最先」顺序对该窗口无能为力）。
const MOUSE_SETTLE: std::time::Duration = std::time::Duration::from_millis(50);

/// 二轮 settle 短窗口（v150）：首轮 drain 消费到事件（说明积压较大或关闭
/// 仍在传播）时追加等待并再消费一轮；首轮即空则单窗口收工（常见退出
/// 路径总耗时 ≈50ms，不可感知）。
const MOUSE_SETTLE_RETRY: std::time::Duration = std::time::Duration::from_millis(20);

/// 终端复原的可测内核（v150 重排：**鼠标关闭最先发出 → settle → drain →
/// raw 关 → 其余关闭**）：接受任意 Write，测试以 `Vec<u8>` 断言关闭命令的
/// 字节顺序、以 TimedWriter（tests 模块）断言相对 settle 窗口的时序。
/// v137 的「drain 最先」顺序留有生成窗口：drain 之后、终端真正处理
/// `?1003l` 之前，鼠标上报仍在进行，新到字节排队在 tty 输入队列，进程
/// 退出后被 shell 回显成 `^[[<35;58;18M` 类乱码（操作者实测仍复现）。
/// v150 把鼠标关闭提到最前，用 settle 窗口覆盖 disable 传播延迟，窗口内
/// 字节随后全部消费——退出瞬间 tty 队列必空。各关闭命令相互独立，无栈
/// 顺序约束，crossterm 0.28.1 逐字序列不变；哨兵 `RESTORE_SEQUENCE`
/// （sentinel.rs）「鼠标组最先」的同形镜像不变量随之保持。
/// 错误容忍：两段 execute! 都执行（前段失败不跳过后续关闭命令），按序
/// 上报首个 Err（复原路径尽最大努力，失败也走完全部关闭）。
fn restore_into<W: std::io::Write>(w: &mut W) -> std::io::Result<()> {
    let mouse_off = execute!(w, DisableMouseCapture);
    settle_and_drain();
    let _ = disable_raw_mode();
    let rest = execute!(w, DisableBracketedPaste, LeaveAlternateScreen);
    mouse_off.and(rest)
}

/// 退出收尾的「等待 + 消费」（v150，取代 v137 的裸 drain）：
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

/// 消费输入队列中已到达的事件字节（v150 契约升级：返回是否消费到事件）：
/// SGR any-event 模式下 motion 事件高频（如 `ESC[<35;56;26M`），restore
/// 只阻止后续上报，不消费已排队/在途字节——进程退出后它们被继承 tty 的
/// shell（zle）当作输入回显成乱码。在 raw mode 仍开启时以短超时 poll
/// 清空队列（非规范模式字节入队即可读；无 tty 环境 poll 报错即视为无事件）。
///
/// poll 超时取 5ms 而非 0（v150 自 ezr-demo 回填——v137 demo 同步时发现、
/// 只改了 demo 的混合语义问题）：
/// ① crossterm 0.28 带超时 poll 的 `Ok(false)` 语义混杂——先
/// `try_lock_internal_event_reader_for(timeout)`，锁竞争立即失败同样返回
/// Ok(false)，并非查过队列（EventStream 的常驻后台线程在
/// `poll_internal(None, …)` 中持锁阻塞等事件，Drop 只置 shutdown 标志
/// 不 join，restore 时 drain 首轮可能撞上锁交接尾巴）；给 5ms 预算即先
/// 等到锁、再按队列真实状态判定。v137 产品侧的 poll(0) 因此经常空转
/// （已排队字节原样留给 shell）——本轮操作者复现的主因之一。
/// ② pty/终端的字节从设备到 line discipline 输入队列有微秒级异步刷入
/// 延迟，0ms 轮询只看一次就退，5ms 给在途字节一个着陆窗口。
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
    if !crate::model::is_http_url(s) {
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
         快捷键（TUI 内）: Tab 切面板 / ]·[·T 切页签 / A 添加 / Space 暂停·继续 /\n\
         R 重试 / D 删除 / U·J 调序 / C 清已完成 / G 收起右栏 / P 分块 /\n\
         B 断开（仅 BT）/ H 帮助 / Q 退出（Esc 仅关闭浮层）"
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

    // 浮层翻转（打开/关闭对话框/下拉、pieces/帮助窗口出现或消失、拖拽
    // 逐帧移动）不再 terminal.clear() 全屏清屏重绘——清屏会在浮层出现/
    // 消失/移动的瞬间清空整个终端，造成整屏闪烁（弹窗显示与移动主界面
    // 闪烁修复，v1.31/FR-01-106⑧ 浮层翻转零清屏）。
    //
    // 历史背景：底层列表文本含 CJK 宽字符，浮层边框恰好落在某宽字符右半格
    // 上时，增量 diff 会跳过被覆盖的尾格，宽字残留在浮层边框上且被后续帧
    // 固化——旧版用 overlay_sig 签名比对 + clear() 全屏重绘掩盖该残影。
    // 现残影已由渲染层 clip_wide_at_edges（ui/btn.rs，每处浮层 Clear 后
    // 调用）在缓冲区层根治：左缘宽字符主格截断、右缘孤立半格补空格，
    // 前后帧模型自洽，diff 输出的连续两格覆写与终端宽字符语义一致，
    // 开/关/拖拽三个方向均无残影，全屏清屏随之退役。
    loop {
        // 浮层翻转（打开/关闭/拖拽）：diff 直接绘制，无清屏
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

    /// v1.34 三批回归（输入框光标闪烁）：进入序列必须显式请求终端闪烁
    /// 模式（`ESC[?12h`，crossterm 0.28.1 EnableBlinking）——输入框光标改
    /// 用终端原生光标后，可见性/闪烁由终端承载，Alacritty 等默认稳态光标
    /// 的终端需显式请求。锁定顺序即调用序：备用屏 → 鼠标捕获 →
    /// bracketed paste → 闪烁请求。
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

    /// v137 回归（退出后鼠标序列泄漏到 shell）→ v150 延续：复原序列必须
    /// **最先**关闭鼠标捕获。按 q 退出瞬间鼠标 motion 事件字节（SGR
    /// `ESC[<b;x;yM`）可能已排队在 tty 缓冲；旧顺序（Paste → Leave → Mouse）
    /// 中鼠标关闭最后发出——disable 传播延迟窗口内终端还在上报新事件，
    /// 与已排队字节一起在进程退出后被 shell 读走，zle 回显成
    /// `^[[<35;56;26M` 类乱码。锁定顺序：鼠标组关闭（?1006l … ?1000l）
    /// 先于备用屏离开（?1049l）与 paste 关闭（?2004l）出现；三组命令
    /// 逐字在位（crossterm 0.28.1）。
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

    /// 记录每次 write 时刻的探针 Writer（v150 时序锁定）：让 restore_into
    /// 的字节流自带时间戳，验证「鼠标关闭在 settle 窗口前写出、paste/
    /// 备用屏关闭在窗口后写出」的相对时序。
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

    /// v150 回归（退出竞态根治的时序契约）：`restore_into` 必须
    /// ① 鼠标关闭序列在 settle 窗口开启**之前**写出（否则窗口白等——
    /// disable 序列本身还在路上，窗口覆盖不到传播延迟）；
    /// ② paste/备用屏关闭在 settle 窗口**之后**写出（settle 先等终端
    /// 处理 `?1003l`，窗口内新到的鼠标字节才有机会被 drain 消费；
    /// 若回退为「先 drain 后关鼠标」，生成窗口重开，shell 再次读到
    /// 鼠标残序）。睡眠保证 ≥50ms，断言取 45ms 容差。
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

    /// v137 回归配套 → v150 契约升级：drain 对空队列/无事件源环境（测试
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

    /// v150 配套：settle_and_drain 在无事件环境至少等待主 settle 窗口
    /// （覆盖 disable 传播延迟的契约本体），且首轮无消费不追加二轮
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

    /// 浮层翻转零清屏的缓冲区模型守卫（v1.31/FR-01-106⑧）：同一 Terminal
    /// 上连续三帧（无浮层 → 对话框打开 → 对话框关闭），CJK 任务名文本
    /// 铺满对话框边界区，开/关两向翻转均由增量 diff 直接绘制——关帧
    /// 缓冲区与首帧完全一致（无残影、无半格伪影、无清屏痕迹）。
    /// 残影根因（diff 跳过被覆盖宽字符尾格）由 clip_wide_at_edges
    /// 缓冲区级清扫根治；本测试以「同一 TestBackend 双帧复绘」复现
    /// 真实终端的增量 diff 语境。
    #[tokio::test]
    async fn overlay_flip_redraws_via_diff_without_clear() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ovflip");
        let reg = dir.join("registry.json").to_string_lossy().to_string();
        let mut app = App::new(Config::default(), reg);
        // CJK 任务名铺在对话框居中区域（74×11 @ 120×34 → x=23..96, y=11..21），
        // 底层文本含宽字符（残影高危形态）
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

        // 帧 2：对话框打开（增量 diff 直接绘制，不清屏——清屏会整屏闪烁）
        app.open_add_dialog();
        term.draw(|f| ui::draw(f, &mut app)).unwrap();
        let s_open = term.backend().to_string();
        assert!(
            s_open.contains("添加任务：HTTP(S)"),
            "帧 2 对话框已绘制（v1.32 标题）"
        );
        // 残影根治锚（clip_wide_at_edges）：对话框底层文本被浮层边界切断处，
        // 边界列内不残留「宽字 + 窄字」同格重叠——对话框左缘列起的行内，
        // 缓冲区在浮层渲染后的宽字符主格不跨界（凡主格在浮层左缘外一格的
        // 宽字已被截断为空格， orphan 半格已补空格）。此处以「浮层区域每行
        // 起止列符号为边框/空白/浮层内容」的保守断言验证缓冲区自洽。
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

        // 帧 3：对话框关闭（关向翻转，同 Terminal 复绘 → 恢复原画面）。
        // 注意 TestBackend 的模拟屏对宽字符有固有精度边界：diff 下发更新时
        // 当前帧宽字符的续格被跳过（真实终端由宽字符双格覆盖语义兜底），
        // 故帧 2 曾被浮层边框占据的续格在模拟屏上会残留旧符号——断言按
        // 「首帧宽字符的续格豁免」口径逐格比对（其余任何残影即红）。
        app.dialog = None;
        term.draw(|f| ui::draw(f, &mut app)).unwrap();
        let after = screen(&term);
        for (y, row) in before.iter().enumerate() {
            for (x, sym_before) in row.iter().enumerate() {
                // 宽字主格的续格（x+1）为模拟屏豁免位：首帧该位由宽字覆盖
                //（真实终端语义），diff 模拟中该位从不下发更新
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
