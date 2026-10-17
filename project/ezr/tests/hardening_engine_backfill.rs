//! 加固测试（six-pack/hardender v118-ui-backfill 轮）：引擎账本簇存活体杀灭
//! （第二目标：需要真实连接级账本的靶点）。
//!
//! 布局：#[path] 挂载 app/engine/model 全树（无 ui，缩编译面）。连接级账本
//! （conn_prev/conn_cum/conn_speed）为 App 私有字段，唯一灌入路径 = 真实引擎
//! ChunkProgress 事件流：本目标以进程内 mock HTTP 服务器 + `App::tick` 自动
//! 发车（slots::allocate 空闲分配）驱动真实下载，经 pub(crate) getter
//! （conn_speed_of/conn_cum_of）与 pub 字段断言。入口：
//! `cargo test --test hardening_engine_backfill`。
//!
//! 靶点对账（v118 补跑轮 missed 清单，本文件负责 11 点；v1.20/FR-01-102 重构
//! 后 update_conn_stats 的 EMA 直推位点消失，靶点几何由 update_conn_stats_pure_targets
//! 与 gen-drift 守卫按新数据面口径承接）：
//! - app/engine/mod.rs update_conn_stats：累计累加 / 窗口推进读数 / 老化清窗 /
//!   待命不推窗（pure_targets 直测副本；gen-drift 守卫测产品源一致）
//! - app/mod.rs active_conn_count 6：416:9 / 416:12 / 421:33 ×3 / 421:71
//!   （cap×speed 2×2 合成矩阵 + 无账本第三连接，real=2 对全部 mutant 唯一）
//! - app/mod.rs clear_conn_stats 1：429:51（!=→==）清除他者 id 不得误伤在册账本
//! - app/tasks.rs clear_completed 1：200:33（==→!=）只清已完成任务的账本
//! - ui/conns.rs 1：101:35（>→<）活跃连接速度列须显数值（并发明细数据行渲染，
//!   需速度>0 的真实下载帧，本目标独有）

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
#![allow(dead_code)]
#![allow(clippy::duplicate_mod)]
#![allow(unused_imports)]

// ui 挂载需要 crate::VERSION（header.rs 版本角标；与 src/main.rs 同宏同值）
pub const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-01");

#[path = "../src/app/mod.rs"]
mod app;
#[path = "../src/engine/mod.rs"]
mod engine;
#[path = "../src/model/mod.rs"]
mod model;
#[path = "../src/ui/mod.rs"]
mod ui;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::Duration;

use app::App;
use model::config::Config;
use model::{Connection, Protocol, Task, TaskState};

/// 确定性内容（同 supervisor 测试口径）
fn fill(buf: &mut [u8], start: u64) {
    for (i, b) in buf.iter_mut().enumerate() {
        *b = ((start as usize + i) % 251) as u8;
    }
}

/// mock HTTP 服务器（同 supervisor::tests 模式 + 慢速写节流）：支持 Range；
/// 每连接独立线程（串行 accept 会让第二连接干等整个第一连接的生命周期），
/// 写盘间歇 sleep 拉长下载窗口（进度事件 200ms 节流 → 需要足够长的下载期）。
fn mock_server(total: u64, slow: bool) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut w = conn.try_clone().unwrap();
                let mut r = BufReader::new(conn);
                let mut line = String::new();
                if r.read_line(&mut line).is_err() || line.is_empty() {
                    return; // 每连接独立闭包：连接级失败直接结束该连接
                }
                let mut range_header = None;
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).is_err() || h.trim().is_empty() {
                        break;
                    }
                    if let Some((k, v)) = h.split_once(':') {
                        if k.trim().eq_ignore_ascii_case("range") {
                            range_header = Some(v.trim().to_string());
                        }
                    }
                }
                let (start, end, status) = match range_header
                    .as_deref()
                    .and_then(|v| v.strip_prefix("bytes="))
                {
                    Some(rv) => {
                        let (a, b) = rv.split_once('-').unwrap();
                        let s = a.parse::<u64>().unwrap_or(0);
                        let e = if b.is_empty() {
                            total - 1
                        } else {
                            b.parse::<u64>().unwrap_or(total - 1).min(total - 1)
                        };
                        (s, e, 206)
                    }
                    _ => (0, total - 1, 200),
                };
                let len = end - start + 1;
                let _ = write!(
                w,
                "HTTP/1.1 {status} OK\r\nAccept-Ranges: bytes\r\nETag: \"mock-{total}\"\r\nLast-Modified: Mon, 09 Feb 2026 08:00:00 GMT\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
            );
                let mut buf = vec![0u8; 8192];
                let mut pos = start;
                while pos <= end {
                    let n = (end - pos + 1).min(8192) as usize;
                    fill(&mut buf[..n], pos);
                    if w.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    if slow {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    pos += n as u64;
                }
            });
        }
    });
    format!("http://{addr}/f.bin")
}

fn queued_task(id: u32, url: &str, dir: &str, conns: usize) -> Task {
    // 块大小 = 整文件：块完成周期 > 引擎 200ms 发包节流 → 同块连续多帧
    // （pb == c.block）→ dt > 0 → 速度 EMA 立起（小块会每帧换块，dt 恒 0）
    Task::new_queued(
        id,
        "f.bin".to_string(),
        Protocol::Http,
        url.to_string(),
        dir.to_string(),
        512 * 1024,
        conns,
        3,
        None,
        model::config::ProxyChoice::Direct,
        model::unix_now(),
    )
}

const TOTAL: u64 = 2 * 1024 * 1024; // 2 MB：块 512KB（块周期 320ms > 200ms 节流 → 同块帧 push 速度）

/// 引擎账本簇主靶（单一下载会话串起全部 11 点）。
/// multi_thread 运行时：current_thread 下 tick 的 await 全部同步完成（mpsc 有
/// 容量 / try_recv 非阻塞）不发生 yield，引擎任务会被饿死（实测死锁 Queued）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn engine_conn_ledger_kills() {
    let url = mock_server(TOTAL, true);
    let dir = model::testenv::uniq_tmp_dir("ezr-h118-eng");
    let reg = dir.join("registry.json").to_string_lossy().into_owned();
    let mut a = App::new(Config::default(), reg);
    a.tasks
        .push(queued_task(1, &url, &dir.to_string_lossy(), 1));

    // ① 等账本灌入（FR-01-102：展示速度 = tick ⑤ 每秒采样窗口速率 → EMA；
    //    需跨过 1s 采样节拍——窗口速率非零后，首个 speed_due 采样立起展示值）
    let mut guard = 0;
    loop {
        a.tick().await;
        if a.conn_cum_of(1, 1) > 0 && a.conn_speed_of(1, 1) > 0.0 {
            break;
        }
        if guard % 5 == 0 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        guard += 1;
        assert!(guard < 60_000, "下载未在预算内产生连接展示速度");
    }

    // ② 靶 update_conn_stats（FR-01-102 重构后：窗口读数 = 连接级累计）+ tick 采样：
    //    速度 EMA 已立起（窗口速率 > 0 经采样）。同帧兼靶 ui/conns.rs 101:35（>→<）：
    //    速度>0 时数据行显数值（mutant 恒 '-'）。
    assert!(
        a.conn_speed_of(1, 1) > 0.0,
        "速度 EMA 已立起（窗口速率经每秒采样）"
    );
    let mid_rows = ui::testfx::row_strings(&ui::testfx::render_full(&mut a, 100, 24));
    assert!(
        mid_rows[16].contains("B/s"),
        "下载中并发明细数据行速度列应显数值（mutant 恒 '-'）：{:?}",
        mid_rows[16]
    );
    // ③ 靶 active_conn_count 6 点：cap×speed 合成矩阵（单速度账本连接 1；
    //    real=1 对全部 mutant {0,0,0,3} 唯一）
    let mut probe = queued_task(1, "probe", "probe", 1);
    probe.state = TaskState::Downloading;
    probe.connections = vec![
        Connection {
            id: 1,
            start: 0,
            end: 100,
            done: 0,
        }, // cap>0, sp>0 → 计
        Connection {
            id: 2,
            start: 0,
            end: 0,
            done: 0,
        }, // cap=0, sp=0 → 不计
        Connection {
            id: 2,
            start: 0,
            end: 100,
            done: 0,
        }, // cap>0, sp=0 → 不计
    ];
    assert_eq!(
        a.active_conn_count(&probe),
        1,
        "仅 cap>0 且 speed>0 计入（mutant: 0/0/0/3 皆异）"
    );
    // 探针 B：cap=0 却有速度的连接 → real=0，杀 421:33（>→>=，mutant 误计 1）
    let mut probe_b = queued_task(1, "probeb", "probeb", 1);
    probe_b.state = TaskState::Downloading;
    probe_b.connections = vec![
        Connection {
            id: 1,
            start: 0,
            end: 0,
            done: 0,
        }, // cap=0, sp>0 → 不计（real=0）
        Connection {
            id: 2,
            start: 0,
            end: 100,
            done: 0,
        }, // cap>0, sp=0 → 不计
    ];
    assert_eq!(
        a.active_conn_count(&probe_b),
        0,
        "cap=0 连接不计（探针 B：mutant >= 误计 cap=0 正速度连接 → 1）"
    );
    let mut idle = queued_task(2, "idle", "idle", 1);
    idle.state = TaskState::Queued;
    assert_eq!(
        a.active_conn_count(&idle),
        0,
        "非明细态恒 0（靶 416:12 delete !）"
    );

    // ④ 靶 clear_conn_stats 429:51（!=→==）：清除无账本 id 不得误伤在册账本
    let cum1_before = a.conn_cum_of(1, 1);
    assert!(cum1_before > 0);
    a.clear_conn_stats(999);
    assert_eq!(
        a.conn_cum_of(1, 1),
        cum1_before,
        "清除他者 id 应保留累计账本（cum-retain mutant 清空）"
    );
    assert!(
        a.conn_speed_of(1, 1) > 0.0,
        "清除他者 id 应保留速度账本（speed-retain mutant 429:51 清空）"
    );

    // ⑤ 靶 clear_completed 200:33（==→!=）：只清已完成任务的账本
    let done = queued_task(2, "done", "done", 2);
    a.tasks.push(done);
    a.tasks[1].state = TaskState::Completed;
    a.clear_completed();
    assert_eq!(a.tasks.len(), 1, "已完成任务被清理");
    assert_eq!(
        a.conn_cum_of(1, 1),
        cum1_before,
        "下载中任务账本必须保留（mutant 收集臂反转后误清）"
    );

    a.shutdown().await;
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------ gen-drift 守卫 ------------

/// gen-drift 守卫：update_conn_stats 剥头副本必须与产品源码逐字一致。
/// 副本为函数提取式（独立 type 别名 + use，不能整文件剥头比对）——从两文件
/// 各自提取 `fn update_conn_stats(` 起至首个顶格 `}` 止的片段逐字比对。
/// 杀灭链：pure_targets 测副本（不变真品）、本守卫测产品（变异即红 → 场景
/// 判 caught），二者合力覆盖 73:30/73:40/79:33——缺守卫则产品变异无感知。
#[test]
fn gen_ucs_fixture_matches_product_source() {
    let src = std::fs::read_to_string("src/app/engine/mod.rs").expect("read product source");
    let fix =
        std::fs::read_to_string("tests/gen/update_conn_stats_stripped.rs").expect("read fixture");
    let extract = |text: &str| -> String {
        let mut out: Vec<&str> = Vec::new();
        let mut on = false;
        for line in text.lines() {
            if !on && line.starts_with("fn update_conn_stats(") {
                on = true;
            }
            if on {
                out.push(line);
                if line == "}" {
                    break;
                }
            }
        }
        assert!(on, "锚点 fn update_conn_stats 未找到");
        out.join("\n")
    };
    assert_eq!(
        extract(&src),
        extract(&fix),
        "剥头副本漂移：update_conn_stats_stripped.rs 与 src/app/engine/mod.rs 的 \
         update_conn_stats 不一致（重新生成副本）"
    );
}

// ------------------------------------------------ update_conn_stats 夹具直测 ---

/// update_conn_stats 剥头副本壳（type 别名 + 纯函数逐字；无 impl 块 → 无碰撞）
mod ucs_fix {
    use std::collections::HashMap;
    use std::time::Instant;

    pub(crate) use crate::engine::ConnView;
    pub(crate) use crate::model::speed::{SpeedWindow, WINDOW};
    include!("gen/update_conn_stats_stripped.rs");

    /// 私有纯函数的同模块转发
    #[allow(clippy::too_many_arguments)]
    pub fn call(
        prev: &mut HashMap<ConnKey, ConnObs>,
        cum: &mut HashMap<ConnKey, u64>,
        windows: &mut HashMap<ConnKey, SpeedWindow>,
        task_id: u32,
        conns: &[ConnView],
    ) {
        update_conn_stats(prev, cum, windows, task_id, conns)
    }
}

/// 靶（v1.20 重构后位点，FR-01-102 数据面）：①累计随增量累加（+= / delta 突变）；
/// ②在传连接窗口推进且读数 = 连接级累计（push-读数突变 → 窗口速率量级错）；
/// ③端点老化清窗（守卫删除 → 停传时长摊进速率，rate 残留非 0）；
/// ④待命连接不推进窗口（展示面零值速断由 tick 采样承担，数据面不产生新样本）。
/// 旧靶 73:30/73:40/79:33（EMA 直推位点）随 FR-01-102 重构消失；零值速断靶点
/// 移至 App 展示采样（idle_conn_speed_snaps_to_zero，engine/mod.rs 内联测试）。
#[test]
fn update_conn_stats_pure_targets() {
    use std::collections::HashMap;

    use ucs_fix::{ConnView, SpeedWindow};
    let view = |block: u32, done: u64| ConnView {
        id: 1,
        block,
        start: 0,
        end: 4_000_000,
        done,
    };

    // ①② 同块两次观测（100ms，rate() 的 dt 下限 0.05s 之上）：cum 累加至 400；
    //    窗口速率 = 300B/100ms = 3KB/s（mutant push(delta)：样本 (100,300) → 2KB/s）
    let mut prev = HashMap::new();
    let mut cum = HashMap::new();
    let mut windows: HashMap<(u32, usize), SpeedWindow> = HashMap::new();
    ucs_fix::call(&mut prev, &mut cum, &mut windows, 7, &[view(0, 100)]);
    std::thread::sleep(Duration::from_millis(100));
    ucs_fix::call(&mut prev, &mut cum, &mut windows, 7, &[view(0, 400)]);
    assert_eq!(cum.get(&(7, 1)), Some(&400u64), "累计量随增量累加");
    let r = windows.get_mut(&(7, 1)).unwrap().rate();
    assert!(
        (2_500.0..3_500.0).contains(&r),
        "窗口速率 = Δcum/Δt ≈ 3KB/s（push-读数突变 → 2KB/s）：{r}"
    );

    // ④ 待命连接（done == cap）：窗口不再推进——rate 冻结为末次值
    //    （mutant 误推：新样本进入窗口 → rate 被稀释偏离 3KB/s）
    std::thread::sleep(Duration::from_millis(30));
    ucs_fix::call(
        &mut prev,
        &mut cum,
        &mut windows,
        7,
        &[ConnView {
            id: 1,
            block: 0,
            start: 0,
            end: 1_000,
            done: 1_000,
        }],
    );
    let r_idle = windows.get_mut(&(7, 1)).unwrap().rate();
    assert!(
        (2_500.0..3_500.0).contains(&r_idle),
        "待命连接窗口冻结（误推 → 速率稀释）：{r_idle}"
    );

    // ③ 端点老化清窗：停传 > WINDOW 后再推 → 清窗重开，单样本 rate == 0
    //    （守卫删除突变：旧端点参与差值 → rate 为停传摊薄的小值 ≠ 0）
    std::thread::sleep(Duration::from_millis(1_100));
    ucs_fix::call(&mut prev, &mut cum, &mut windows, 7, &[view(1, 200)]);
    let r_stale = windows.get_mut(&(7, 1)).unwrap().rate();
    assert_eq!(
        r_stale, 0.0,
        "老化清窗后单样本速率为 0（守卫删除 → 摊薄残留）: {r_stale}"
    );
}
