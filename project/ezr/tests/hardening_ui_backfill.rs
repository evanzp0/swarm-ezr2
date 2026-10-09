//! 加固测试（six-pack/hardender v118-ui-backfill 轮）：UI 层 + 交互分发层
//! 变异存活体杀灭套件（第一目标：无状态/渲染/纯函数簇）。
//!
//! 布局：#[path] 挂载 app/engine/model/ui 全树（同 hardening_g_appcore 口径）+
//! `tests/gen/*_stripped.rs` 剥头副本壳（私有纯函数/私有模块项同模块可达；
//! gen-drift 守卫逐字比对产品源码防漂移）。入口命令：`cargo test --test hardening_ui_backfill`。
//!
//! 靶点对账（v118 补跑轮 missed 清单，本文件负责 46 点）：
//! - app/mod.rs getters 12：global_dl 4 / global_ul 5 / conns_display_total 2 /（396:40 `>`→`>=`
//!   为求和不变量等价，登记不杀）
//! - dialogs.rs 6：dlg_confirm_add 5 + open_modify_dialog 1（经 on_key 公开路径驱动）
//! - keys.rs 2：'j' 臂删除 + move_conn_sel 滚动跟随（45:76 BackTab `±1 mod 2` 等价，登记不杀）
//! - mouse.rs 5：对话框右缘 / 列表行界 / ti<len / ScrollDown 臂 / pointer_in 右缘
//! - clipboard.rs 1：b64 `>`→`>=`（len%3==1 向量；`|`→`^` ×2 位域不相交等价 + copy_osc52
//!   环境类，登记不杀）
//! - ui/chart.rs 11：chart_columns 金样 8 + draw_chart 守卫/偏移 3
//! - ui/detail.rs 3：queued_value / chunk_row 渲染 + hot_rect 直测
//! - ui/delete.rs 3：高度守卫 + 提示行/按钮行金样
//! - ui/conns.rs 1：数据行存在性（92:22；101:35 需速度>0 → 引擎 e2e 目标）
//! - ui/btn.rs 1：按钮行居中金样
//!
//! 进程级/环境类论证见 handoff；引擎账本簇见 hardening_engine_backfill.rs。

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

use app::{Dialog, DialogKind};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use model::config::Config;
use model::{Connection, Protocol, Task, TaskState};
use ratatui::layout::Rect;
use ui::testfx::{make_app_in, render_full, row_strings, task_in};

// --------------------------------------------------------------- 剥头副本壳 ---

/// chart.rs 剥头副本壳（tests/gen/chart_stripped.rs = 产品源去 //! 头块）；
/// 双层结构模拟 ui::chart 层级使 super:: 解析成立；无 impl 块 → 无 E0592 碰撞。
mod chart_fix {
    #[allow(clippy::items_after_test_module)]
    pub mod chart {
        include!("gen/chart_stripped.rs");

        /// 私有纯函数的同模块转发
        pub fn cols(hist: &[u64], w: usize, h: usize) -> Vec<usize> {
            chart_columns(hist, w, h)
        }

        /// pub(super) 绘制函数的同模块转发
        pub fn draw_iso(f: &mut ratatui::Frame, a: &crate::app::App, area: Rect) {
            draw_chart(f, a, area)
        }
    }
    // super::LIGHT_BLUE 解析点（ui/mod.rs 定稿同值）
    pub const LIGHT_BLUE: ratatui::style::Color = ratatui::style::Color::Rgb(122, 185, 242);
}

/// clipboard.rs 剥头副本壳：同形假 App 宿主（从不实例化），真 b64/osc52 逐字编译
mod clip_fix {
    pub mod clipboard {
        include!("gen/clipboard_stripped.rs");
    }
    // super::App 解析点：与 clipboard.rs impl 触点同形的最小宿主
    #[allow(dead_code)]
    pub struct App {
        pub last_copied: Option<String>,
        pub clipboard: Option<arboard::Clipboard>,
    }
}

/// btn.rs 剥头副本壳（clip_right_edge 直测）；super 依赖同 detail_fix 口径
mod btn_fix {
    pub mod text {
        pub(crate) use crate::ui::w;
    }
    use ratatui::style::Color;
    pub const ACCENT: Color = Color::Cyan;
    pub const FG: Color = Color::Rgb(198, 208, 212);
    pub mod btn {
        include!("gen/btn_stripped.rs");
        pub fn clip_right(buf: &mut ratatui::buffer::Buffer, xr: u16, y: u16) {
            clip_right_edge(buf, xr, y)
        }
    }
}

/// detail.rs 剥头副本壳（hot_rect 直测）；text 转发 ui 层真实现，调色板为壳内
/// 同值复刻（仅样式，断言不依赖色值）
mod detail_fix {
    pub mod text {
        pub(crate) use crate::ui::{fmt_size, pad_right, truncate, w};
    }
    use ratatui::style::Color;
    pub const ACCENT: Color = Color::Cyan;
    pub const BORDER: Color = Color::Rgb(56, 78, 86);
    pub const DIM: Color = Color::Rgb(126, 141, 147);
    pub const FG: Color = Color::Rgb(198, 208, 212);
    pub const GREEN: Color = Color::Green;
    pub const LIGHT_BLUE: Color = Color::Rgb(122, 185, 242);
    pub const RED: Color = Color::Rgb(226, 92, 84);
    pub const YELLOW: Color = Color::Rgb(228, 178, 62);
    pub fn state_color(s: crate::model::TaskState) -> Color {
        use crate::model::TaskState::*;
        match s {
            Queued => YELLOW,
            Downloading | Verifying | PostProcessing => LIGHT_BLUE,
            Paused => Color::White,
            Completed => GREEN,
            Failed => RED,
            Seeding => Color::Rgb(214, 112, 214),
            _ => DIM,
        }
    }
    pub mod detail {
        include!("gen/detail_stripped.rs");
        pub fn hot(inner: Rect, line_idx: usize) -> Option<Rect> {
            hot_rect(inner, line_idx)
        }
    }
}

/// gen-drift 守卫：剥头副本必须与产品源码逐字一致（仅缺文件头 //! 块）；
/// 产品源变更 → 本测试红 → 副本必须重生成（杜绝静默漂移）
fn assert_fixture_matches(product: &str, fixture: &str) {
    let src = std::fs::read_to_string(product).unwrap_or_else(|e| panic!("read {product}: {e}"));
    let fix = std::fs::read_to_string(fixture).unwrap_or_else(|e| panic!("read {fixture}: {e}"));
    let head = src
        .lines()
        .take_while(|l| l.starts_with("//!") || l.trim().is_empty() && !src.is_empty())
        .count();
    // 头块 = 连续的 //! 行（从首行起）；其后可能有空行
    let mut n = 0;
    for l in src.lines() {
        if l.starts_with("//!") {
            n += 1;
        } else {
            break;
        }
    }
    let expect: String = src.lines().skip(n).collect::<Vec<_>>().join("\n");
    let actual: String = fix.lines().collect::<Vec<_>>().join("\n");
    assert_eq!(
        expect, actual,
        "剥头副本漂移：{fixture} 与 {product} 不一致（重新生成）"
    );
    let _ = head;
}

#[test]
fn gen_fixtures_match_product_sources() {
    assert_fixture_matches("src/ui/chart.rs", "tests/gen/chart_stripped.rs");
    assert_fixture_matches("src/app/clipboard.rs", "tests/gen/clipboard_stripped.rs");
    assert_fixture_matches("src/ui/detail.rs", "tests/gen/detail_stripped.rs");
    assert_fixture_matches("src/ui/btn.rs", "tests/gen/btn_stripped.rs");
}

// ---------------------------------------------------------------- 夹具工具 ---

fn sample_task(id: u32, name: &str) -> Task {
    let mut t = model::sample_task();
    t.id = id;
    t.name = name.to_string();
    t
}

fn queued_task(id: u32, name: &str, url: &str, dir: &str) -> Task {
    Task::new_queued(
        id,
        name.to_string(),
        Protocol::Http,
        url.to_string(),
        dir.to_string(),
        1 << 20,
        4,
        3,
        None,
        model::config::ProxyChoice::Direct,
        model::unix_now(),
    )
}

fn mev(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn tap(a: &mut app::App, code: KeyCode) {
    a.on_key(code, KeyModifiers::empty());
}

/// 经公开按键路径添加任务：A 打开对话框 → 填 URL → Enter 确认
fn add_via_dialog(a: &mut app::App, url: &str, dir: &str) {
    tap(a, KeyCode::Char('a'));
    let d = a.dialog.as_mut().expect("A 打开添加对话框");
    d.url = url.to_string();
    d.dir = dir.to_string();
    tap(a, KeyCode::Enter);
}

// ------------------------------------------------- app/mod.rs getters 簇 12 ---

/// 靶 383:9 ×3 / 385:33：global_dl_speed 仅合计 Downloading 任务的 speed
#[tokio::test]
async fn global_dl_speed_sums_downloading_only() {
    let mut a = make_app_in("ezr-h118", "gdl");
    let mut t1 = queued_task(1, "a.bin", "http://example.com/a.bin", "/tmp");
    t1.state = TaskState::Downloading;
    t1.speed = 10.0;
    let mut t2 = queued_task(2, "b.bin", "http://example.com/b.bin", "/tmp");
    t2.state = TaskState::Downloading;
    t2.speed = 20.0;
    let mut t3 = queued_task(3, "c.bin", "http://example.com/c.bin", "/tmp");
    t3.state = TaskState::Queued;
    t3.speed = 999.0;
    a.tasks.push(t1);
    a.tasks.push(t2);
    a.tasks.push(t3);
    assert_eq!(a.global_dl_speed(), 30.0, "仅合计下载中任务速度");
    a.shutdown().await;
}

/// 靶 394:9 ×3 / 396:40 `>`→`==` 与 `>`→`<`（`>=` 为求和不变量等价，登记）
#[tokio::test]
async fn global_ul_speed_sums_positive_uploads() {
    let mut a = make_app_in("ezr-h118", "gul");
    let mut t1 = queued_task(1, "a.bin", "http://example.com/a.bin", "/tmp");
    t1.upload_speed = 5.0;
    let mut t2 = queued_task(2, "b.bin", "http://example.com/b.bin", "/tmp");
    t2.upload_speed = 0.0;
    let mut t3 = queued_task(3, "c.bin", "http://example.com/c.bin", "/tmp");
    t3.upload_speed = 7.0;
    a.tasks.push(t1);
    a.tasks.push(t2);
    a.tasks.push(t3);
    assert_eq!(a.global_ul_speed(), 12.0, "仅合计 upload_speed > 0 的任务");
    a.shutdown().await;
}

/// 靶 405:9 ×2：conns_display_total 仅合计 shows_conns 任务的连接数
#[tokio::test]
async fn conns_display_total_counts_visible_conn_states() {
    let mut a = make_app_in("ezr-h118", "cdt");
    let mut t1 = queued_task(1, "a.bin", "http://example.com/a.bin", "/tmp");
    t1.state = TaskState::Downloading;
    t1.connections = vec![
        Connection {
            id: 1,
            start: 0,
            end: 100,
            done: 0,
        },
        Connection {
            id: 2,
            start: 100,
            end: 200,
            done: 0,
        },
        Connection {
            id: 3,
            start: 200,
            end: 300,
            done: 0,
        },
    ];
    let mut t2 = queued_task(2, "b.bin", "http://example.com/b.bin", "/tmp");
    t2.state = TaskState::Downloading;
    t2.connections = vec![Connection {
        id: 1,
        start: 0,
        end: 100,
        done: 0,
    }];
    let mut t3 = queued_task(3, "c.bin", "http://example.com/c.bin", "/tmp");
    t3.state = TaskState::Queued;
    t3.connections = vec![
        Connection {
            id: 1,
            start: 0,
            end: 100,
            done: 0,
        },
        Connection {
            id: 2,
            start: 100,
            end: 200,
            done: 0,
        },
        Connection {
            id: 3,
            start: 200,
            end: 300,
            done: 0,
        },
        Connection {
            id: 4,
            start: 300,
            end: 400,
            done: 0,
        },
        Connection {
            id: 5,
            start: 400,
            end: 500,
            done: 0,
        },
    ];
    a.tasks.push(t1);
    a.tasks.push(t2);
    a.tasks.push(t3);
    assert_eq!(a.conns_display_total(), 4, "仅合计并发明细态任务的连接数");
    a.shutdown().await;
}

// ---------------------------------------------------- dialogs.rs 簇 6（公开键路径） ---

/// 靶 179:21（resumed 表内臂 `&&`→`||`）：变异解析为 `(url&&dir) || (name&&state)`
/// ——同名异 URL 存量任务在 real 下不得触发接续（url 臂假），mutant 经 name 臂
/// 误判「接续」。
#[tokio::test]
async fn confirm_add_resume_task_arm_requires_full_conjunction() {
    let mut a = make_app_in("ezr-h118", "dup179");
    let dir_s = model::testenv::uniq_tmp_dir("ezr-h118-dup179")
        .to_string_lossy()
        .to_string();
    // 存量任务：与派生名同名（same.bin）、异 URL（盘上无 sidecar）
    let mut t = queued_task(1, "same.bin", "http://a.com/same.bin", &dir_s);
    t.state = TaskState::Queued;
    a.tasks.push(t);
    add_via_dialog(&mut a, "http://b.com/same.bin", &dir_s);
    let toast = a.toast.clone().unwrap_or_default();
    assert!(
        !toast.contains("接续"),
        "同名异 URL 不得触发接续口径（mutant 179:21 经 name 臂误接续）：{toast}"
    );
    assert_eq!(a.tasks.len(), 2, "非重复应正常创建");
    a.shutdown().await;
    std::fs::remove_dir_all(&dir_s).ok();
}

/// 靶 189:21（dedupe 闭包 `||`→`&&`）：重名检测 = 表内同名同目录 或 盘上存在
/// （任一即改名）。异 URL 同名同目录：表内臂真、盘上臂假 → real 改名
/// （mutant 合取后不改名 → 与既有任务同名冲突）。
#[tokio::test]
async fn confirm_add_dedupe_renames_on_task_table_match() {
    let mut a = make_app_in("ezr-h118", "dedupe189");
    let dir = model::testenv::uniq_tmp_dir("ezr-h118-dedupe189");
    let dir_s = dir.to_string_lossy().to_string();
    // 存量任务：名为 dup.bin（异 URL、同目录）；盘上无同名文件
    a.tasks
        .push(queued_task(1, "dup.bin", "http://a.com/dup.bin", &dir_s));
    // 新 URL 派生同名 dup.bin → 须自动改名
    add_via_dialog(&mut a, "http://b.com/dup.bin", &dir_s);
    assert_eq!(a.tasks.len(), 2, "异 URL 不构成重复，应创建");
    assert_ne!(
        a.tasks[1].name, "dup.bin",
        "表内同名必须改名（mutant 189:21 && 后不改名）"
    );
    a.shutdown().await;
    std::fs::remove_dir_all(&dir).ok();
}

/// 靶 198:22（+=→-=）：next_id 单调递增
#[tokio::test]
async fn confirm_add_increments_next_id() {
    let mut a = make_app_in("ezr-h118", "nid");
    let before = a.next_id;
    add_via_dialog(&mut a, "http://example.com/fresh.bin", "/tmp");
    assert_eq!(a.tasks[0].id, before, "新任务取得当前 id");
    assert_eq!(a.next_id, before + 1, "next_id 递增 1");
    a.shutdown().await;
}

/// 靶 220:34（-→/）：添加后选中最后一个（flen-1），不越界
#[tokio::test]
async fn confirm_add_selects_last_row() {
    let mut a = make_app_in("ezr-h118", "sel");
    add_via_dialog(&mut a, "http://example.com/only.bin", "/tmp");
    assert_eq!(a.filtered().len(), 1);
    assert_eq!(a.selected, 0, "单任务列表选中第 0 行（flen-1）");
    a.shutdown().await;
}

/// 靶 142:81（-→+）：proxy_sel 越界必须钳到 len-1（mutant 越界索引 panic）
#[tokio::test]
async fn confirm_add_clamps_out_of_range_proxy_sel() {
    let mut a = make_app_in("ezr-h118", "clamp");
    tap(&mut a, KeyCode::Char('a'));
    let len = a.proxy_options.len();
    {
        let d = a.dialog.as_mut().expect("add dialog");
        d.url = "http://example.com/px.bin".to_string();
        d.proxy_sel = len; // 越界（合法下标 0..len-1）
    }
    tap(&mut a, KeyCode::Enter);
    assert_eq!(a.tasks.len(), 1, "越界 proxy_sel 应被钳回并正常创建");
    a.shutdown().await;
}

/// 靶 353:42（-→+）：存量任务的表外代理应追加兜底项并选中它（len-1）
#[tokio::test]
async fn open_modify_dialog_appends_unknown_named_proxy() {
    let mut a = make_app_in("ezr-h118", "modpx");
    let mut t = queued_task(1, "px.bin", "http://example.com/px.bin", "/tmp");
    t.proxy = model::config::ProxyChoice::Named("ghost-proxy".to_string());
    a.tasks.push(t);
    a.selected = 0;
    tap(&mut a, KeyCode::Char('m'));
    let d = a.dialog.as_ref().expect("m 打开修改对话框");
    assert_eq!(
        d.proxy_sel,
        a.proxy_options.len() - 1,
        "表外命名代理追加为兜底项并选中末位"
    );
    a.shutdown().await;
}

// ------------------------------------------------------------- keys.rs 簇 2 ---

/// 靶 55:13（删除 match 臂）：'j' 触发任务下移调序
#[tokio::test]
async fn key_j_moves_task_down() {
    let mut a = make_app_in("ezr-h118", "jkey");
    a.tasks
        .push(queued_task(1, "a.bin", "http://example.com/a.bin", "/tmp"));
    a.tasks
        .push(queued_task(2, "b.bin", "http://example.com/b.bin", "/tmp"));
    a.selected = 0;
    tap(&mut a, KeyCode::Char('j'));
    assert_eq!(
        a.tasks[0].name, "b.bin",
        "'j' 应将选中任务下移（原第 2 项到首位）"
    );
    a.shutdown().await;
}

/// 靶 85:41（-→/）：选中越出可视窗口时滚动跟随 new+1-vis
#[tokio::test]
async fn move_conn_sel_follows_scroll_window() {
    let mut a = make_app_in("ezr-h118", "mcs");
    let mut t = queued_task(1, "mcs.bin", "http://example.com/mcs.bin", "/tmp");
    t.state = TaskState::Downloading;
    t.connections = (1..=6)
        .map(|i| Connection {
            id: i,
            start: 0,
            end: 100,
            done: 0,
        })
        .collect();
    a.tasks.push(t);
    a.selected = 0;
    a.visible_conns_rows = 2;
    a.conns_sel = 0;
    a.conns_scroll = 0;
    a.move_conn_sel(5); // new=5 ≥ scroll+vis=2 → scroll = 5+1-2 = 4
    assert_eq!(a.conns_sel, 5);
    assert_eq!(
        a.conns_scroll, 4,
        "滚动跟随 = new+1-vis（mutant new+1/vis=5）"
    );
    a.shutdown().await;
}

// ------------------------------------------------------------ mouse.rs 簇 5 ---

/// 靶 31:41（<→<=）：命中矩形右开边界——右缘列不算命中
#[tokio::test]
async fn dialog_hit_rect_right_edge_exclusive() {
    let mut a = make_app_in("ezr-h118", "medge");
    a.dialog = Some(app::testutil::dialog(DialogKind::Add, 0));
    a.dlg_field_rects = vec![(
        Rect {
            x: 2,
            y: 2,
            width: 30,
            height: 1,
        },
        3,
    )];
    // 列 32 == x+width（右开边界外一格）
    a.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 32, 2));
    let d = a.dialog.as_ref().unwrap();
    assert!(!d.ck_open, "右缘列不命中算法下拉");
    assert_ne!(d.focus, 3, "右缘列不聚焦");
    a.shutdown().await;
}

/// 靶 123:54（<→<=）：列表命中行界右开——y == a.y+height 不选中
#[tokio::test]
async fn list_click_bottom_edge_exclusive() {
    let mut a = make_app_in("ezr-h118", "medgey");
    a.list_area = Some(Rect {
        x: 0,
        y: 1,
        width: 40,
        height: 9,
    });
    for i in 1..=3 {
        a.tasks.push(queued_task(
            i,
            &format!("t{i}.bin"),
            &format!("http://example.com/t{i}.bin"),
            "/tmp",
        ));
    }
    a.selected = 0;
    // 行 10 == a.y + height（区域外一格）
    a.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 10));
    assert_eq!(a.selected, 0, "列表底界外一格不选中");
    a.shutdown().await;
}

/// 靶 127:31（<→<=）：ti 越过列表长度不选中（空行点击无害）
#[tokio::test]
async fn list_click_past_items_no_select() {
    let mut a = make_app_in("ezr-h118", "mti");
    a.list_area = Some(Rect {
        x: 0,
        y: 1,
        width: 40,
        height: 9,
    });
    a.tasks
        .push(queued_task(1, "a.bin", "http://example.com/a.bin", "/tmp"));
    a.tasks
        .push(queued_task(2, "b.bin", "http://example.com/b.bin", "/tmp"));
    a.selected = 0;
    // 行 9 → idx 2 → ti == len(2)：越界一格
    a.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 9));
    assert_eq!(a.selected, 0, "ti == len 不选中（防越界）");
    a.shutdown().await;
}

/// 靶 148:13（删除 ScrollDown 臂）：主列表滚轮下滚推进 scroll
#[tokio::test]
async fn wheel_down_scrolls_task_list() {
    let mut a = make_app_in("ezr-h118", "wheel");
    for i in 1..=10 {
        a.tasks.push(queued_task(
            i,
            &format!("t{i}.bin"),
            &format!("http://example.com/t{i}.bin"),
            "/tmp",
        ));
    }
    a.visible_rows = 6;
    assert!(
        a.conns_area.is_none(),
        "前置：无并发明细区（滚轮作用于任务列表）"
    );
    a.on_mouse(mev(MouseEventKind::ScrollDown, 5, 5));
    assert_eq!(a.scroll, 2, "下滚 +2（mutant 删除臂后恒 0）");
    a.shutdown().await;
}

/// 靶 161:23（<→<=）：热区右开边界——右缘列点击不触发复制
#[tokio::test]
async fn detail_hot_rect_right_edge_exclusive() {
    let mut a = make_app_in("ezr-h118", "pedge");
    let mut t = queued_task(1, "copy.bin", "http://example.com/copy.bin", "/tmp");
    t.final_url = Some("http://final.example/copy.bin".to_string());
    a.tasks.push(t);
    a.detail_url_rect = Some(Rect {
        x: 1,
        y: 7,
        width: 9,
        height: 1,
    });
    // 列 10 == 1+9（右开边界外一格）
    a.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 10, 7));
    assert!(a.last_copied.is_none(), "热区右缘列不触发复制");
    a.shutdown().await;
}

// --------------------------------------------------------- clipboard.rs 1 ---

/// 靶 53:33（>→>=）：b64 len%3==1 尾块必须 '=' 填充（RFC 4648）
#[test]
fn osc52_b64_single_byte_tail_pads() {
    assert_eq!(
        clip_fix::clipboard::osc52_sequence("a"),
        "\x1b]52;c;YQ==\x07",
        "1 字节载荷：两处 '=' 填充（mutant 输出 YQAA）"
    );
}

// ---------------------------------------------------------- ui/chart.rs 11 ---

/// 靶 chart_columns 8 点（30:23/36:27/39:15/48:20/56:29/56:52/58:65/58:70）：
/// 私有纯函数金样（捕获自干净实现，gen-drift 守卫保证同源）
#[test]
#[allow(clippy::type_complexity)]
fn chart_columns_golden_vectors() {
    // (hist, w, h) → 期望列高（捕获于 v118 干净实现）
    let cases: Vec<(Vec<u64>, usize, usize, Vec<usize>)> = vec![
        (
            (0..40).map(|i| (i * 137) % 997).collect(),
            20,
            3,
            vec![
                10, 17, 20, 19, 12, 17, 18, 14, 13, 17, 17, 9, 17, 21, 21, 12, 16, 17, 14, 5,
            ],
        ),
        (
            vec![0, 0, 0, 500, 500, 500, 0, 250],
            8,
            2,
            vec![4, 7, 11, 14, 14, 11, 8, 6],
        ),
        (vec![5, 1, 6, 3, 9, 2], 4, 1, vec![4, 5, 5, 5]),
        (
            (0..90).map(|i| (i * 7 % 23) * 11).collect(),
            30,
            4,
            vec![
                23, 21, 21, 28, 24, 23, 19, 16, 18, 25, 26, 22, 20, 21, 28, 28, 24, 19, 19, 18, 22,
                26, 22, 21, 20, 24, 27, 21, 17, 13,
            ],
        ),
    ];
    for (hist, w, h, expect) in cases {
        assert_eq!(
            chart_fix::chart::cols(&hist, w, h),
            expect,
            "chart_columns 金样漂移（w={w} h={h}）"
        );
    }
}

/// 靶 draw_chart 71:42（||→&&）/ 71:66（<→==）/ 99:27（+→*）：
/// 隔离渲染金样——len==2 输入 + y 偏移区域 + w=1 守卫
#[tokio::test]
async fn draw_chart_isolated_goldens() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut a = make_app_in("ezr-h118", "chart");
    a.speed_hist = vec![10, 900]; // len==2（71:66 的判别窗口）

    // 区域 y 偏移 2：mutant 99:27（y=area.y*row）会把三行都画到 y=0
    let mut term = Terminal::new(TestBackend::new(30, 8)).unwrap();
    term.draw(|f| chart_fix::chart::draw_iso(f, &a, Rect::new(0, 2, 20, 3)))
        .unwrap();
    let rows = row_strings(&term);
    assert_eq!(
        rows[0], "                              ",
        "y=0 行必须为空（mutant 会误绘）"
    );
    assert!(
        rows[2].contains('▁') || rows[2].contains('▂') || rows[2].contains('▄'),
        "y=2 行应有图形（mutant 71:66 在 len==2 提前返回）: {:?}",
        rows[2]
    );
    assert!(rows[4].contains('█'), "y=4 行应有满块: {:?}", rows[4]);

    // w=1 守卫：real 直接返回（mutant ||→&& 后照绘或 panic）
    let mut term2 = Terminal::new(TestBackend::new(30, 8)).unwrap();
    term2
        .draw(|f| chart_fix::chart::draw_iso(f, &a, Rect::new(0, 0, 1, 3)))
        .unwrap();
    for (i, r) in row_strings(&term2).iter().enumerate() {
        assert_eq!(r.trim(), "", "w=1 守卫下整帧无图形（row {i}）");
    }
    a.shutdown().await;
}

// --------------------------------------------------------- ui/detail.rs 3 ---

/// 靶 39:5（queued_value → "xyzzy"）：排队任务详情展示排队位次文案
#[tokio::test]
async fn detail_queued_value_verbatim() {
    let mut a = make_app_in("ezr-h118", "qv");
    let mut t = task_in(1, "qv.bin", TaskState::Queued);
    t.url = "http://example.com/qv.bin".into();
    t.save_dir = "/tmp".into();
    a.tasks.push(t);
    a.selected = 0;
    let term = render_full(&mut a, 100, 20);
    let rows = row_strings(&term);
    let hit = rows.iter().find(|r| r.contains("排 队 第"));
    assert!(hit.is_some(), "排队位次文案必须出现（mutant 渲染 xyzzy）");
    assert!(!rows.iter().any(|r| r.contains("xyzzy")), "禁止 xyzzy 占位");
    a.shutdown().await;
}

/// 靶 69:16（==→!=）：有分块任务显示 x/y · N/块，而非 '-' 占位
#[tokio::test]
async fn detail_chunk_row_shows_blocks() {
    let mut a = make_app_in("ezr-h118", "cr");
    let mut t = task_in(1, "cr.bin", TaskState::Queued);
    t.url = "http://example.com/cr.bin".into();
    t.save_dir = "/tmp".into();
    a.tasks.push(t);
    a.selected = 0;
    let term = render_full(&mut a, 100, 25);
    let rows = row_strings(&term);
    let hit = rows.iter().find(|r| r.contains("分 块"));
    assert!(hit.is_some(), "分块行出现");
    assert!(
        hit.unwrap().contains("/块"),
        "分块行显示 N/块（mutant 误入 '-' 臂）: {hit:?}"
    );
    a.shutdown().await;
}

/// 靶 238:8（<→<=）：行号 == 内框高度（一格外）不设热区
#[test]
fn hot_rect_boundary_line_exclusive() {
    let inner = Rect {
        x: 3,
        y: 5,
        width: 20,
        height: 6,
    };
    assert_eq!(
        detail_fix::detail::hot(inner, 5),
        Some(Rect {
            x: 4,
            y: 10,
            width: 9,
            height: 1
        }),
        "行号 5 < 高度 6 → 设热区（正控）"
    );
    assert!(
        detail_fix::detail::hot(inner, 6).is_none(),
        "行号 == 高度 → 不设（mutant <= 误设）"
    );
    assert!(
        detail_fix::detail::hot(inner, 9).is_none(),
        "行号超出更多 → 不设"
    );
    let tiny = Rect {
        x: 0,
        y: 0,
        width: 12,
        height: 1,
    };
    assert!(
        detail_fix::detail::hot(tiny, 1).is_none(),
        "h=1 时行号 1 越界"
    );
}

// --------------------------------------------------------- ui/delete.rs 3 ---

/// 靶 53:24 / 77:24（+→-）：提示行与按钮行 x 定位金样（80×24 整帧）
#[tokio::test]
async fn delete_dialog_position_golden() {
    let mut a = make_app_in("ezr-h118", "del");
    let mut t = sample_task(1, "删除目标文件.bin");
    t.url = "http://example.com/del.bin".into();
    a.tasks.push(t);
    a.selected = 0;
    a.open_delete_dialog();
    if let Some(d) = a.dialog.as_mut() {
        d.task_name = "删除目标文件.bin".into();
    }
    let rows = row_strings(&render_full(&mut a, 80, 24));
    assert_eq!(
        rows[10],
        "│      │  删 除 任 务  「 删 除 目 标 文 件 .bin」  ？                               │      │",
        "标题行 x=inner.x+1 定位金样（靶 53:24，mutant x=inner.x-1 左移压边）"
    );
    assert_eq!(
        rows[12],
        "│      │        [ 仅 删 除 任 务  ]    [ 删 除 任 务 和 文 件  ]    [ 取 消  ]        │      │",
        "按钮行居中定位金样（mutant 整行左移 2 列）"
    );
    assert_eq!(
        rows[13],
        "│      │  1/2/3 快 捷 选 择  · Enter 确 认  · Esc 取 消                         │      │",
        "提示行（inner.y+4）x=inner.x+1 定位金样（靶 77:24，mutant x=inner.x-1 左移压边；\
         77:24 只控制提示行，标题行 rows[10] 对其不敏感——须逐字断言本行）"
    );
    a.shutdown().await;
}

/// 靶 28:25（||→&&）：内框高度 < 5 时提示不绘（窄高守卫分支）
#[tokio::test]
async fn delete_dialog_height_guard_returns_early() {
    let mut a = make_app_in("ezr-h118", "delh");
    let mut t = sample_task(1, "guard.bin");
    t.url = "http://example.com/guard.bin".into();
    a.tasks.push(t);
    a.selected = 0;
    a.open_delete_dialog();
    if let Some(d) = a.dialog.as_mut() {
        d.task_name = "guard.bin".into();
    }
    let rows = row_strings(&render_full(&mut a, 80, 6));
    assert!(
        !rows.iter().any(|r| r.contains("仅 删 除 任 务")),
        "矮终端 h=6（内框 h=4 <5）不绘提示与按钮（mutant && 短路后照绘）"
    );
    a.shutdown().await;
}

/// 靶 100:5（clip_right_edge → ()）：浮层右缘外一列的孤立半格必须补空格。
/// 场景复刻：宽字符主格被浮层改写后续格残留空符号 → 调用后补成 ' '；
/// 右缘外越界守卫（xr+1 >= width）保持无操作。
#[test]
fn clip_right_edge_fills_orphan_half_cell() {
    use ratatui::buffer::Buffer;
    let mut buf = Buffer::empty(Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 1,
    });
    // 宽字符主格（占 2 列）：浮层改写主格后续格残留空符号
    let i2 = buf.index_of(2, 0);
    let i3 = buf.index_of(3, 0);
    buf.content[i2].set_symbol("中");
    buf.content[i3].set_symbol(""); // 孤立半格
    btn_fix::btn::clip_right(&mut buf, 2, 0); // xr=2 → 检查 xr+1=3
    assert_eq!(
        buf.content[buf.index_of(3, 0)].symbol(),
        " ",
        "孤立半格必须补空格（mutant 整体替换为 () 后保持空）"
    );
    // 非空续格不改动（幂等面）
    let mut buf2 = Buffer::empty(Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 1,
    });
    let j3 = buf2.index_of(3, 0);
    buf2.content[j3].set_symbol("x");
    btn_fix::btn::clip_right(&mut buf2, 2, 0);
    assert_eq!(buf2.content[j3].symbol(), "x", "非空符号不补写");
    // 越界守卫：xr+1 == width → 无操作不 panic
    let mut buf3 = Buffer::empty(Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 1,
    });
    btn_fix::btn::clip_right(&mut buf3, 9, 0);
}

// ------------------------------------------------- ui/conns.rs 1 + ui/btn.rs 1 ---

/// 靶 92:22（+→*）：数据行窗口 end = start + data_rows → 连接行必须绘出
#[tokio::test]
async fn conns_panel_draws_data_rows() {
    let mut a = make_app_in("ezr-h118", "conns");
    let mut t = task_in(1, "conns_cap.bin", TaskState::Downloading);
    t.url = "http://example.com/conns.bin".into();
    t.save_dir = "/tmp".into();
    t.connections = vec![
        Connection {
            id: 1,
            start: 0,
            end: 1500,
            done: 600,
        },
        Connection {
            id: 2,
            start: 1500,
            end: 3000,
            done: 0,
        },
    ];
    a.tasks.push(t);
    a.selected = 0;
    let rows = row_strings(&render_full(&mut a, 100, 24));
    assert!(
        rows[16].contains("   1"),
        "连接 1 数据行存在（mutant end=start*rows 后无数据行）: {:?}",
        rows[16]
    );
    assert!(
        rows[17].contains("   2"),
        "连接 2 数据行存在: {:?}",
        rows[17]
    );
    a.shutdown().await;
}

/// 靶 71:86（/→%）：按钮行水平居中金样（Add 对话框 80×24）
#[tokio::test]
async fn add_dialog_button_row_centering_golden() {
    let mut a = make_app_in("ezr-h118", "btn");
    a.open_add_dialog();
    let rows = row_strings(&render_full(&mut a, 80, 24));
    let hit = rows.iter().find(|r| r.contains("[ 确 认"));
    assert!(hit.is_some(), "确认按钮行出现");
    assert_eq!(
        hit.unwrap(),
        "│  │                          [ 确 认  ]    [ 取 消  ]                          │  │",
        "按钮行居中金样（mutant % 使按钮组整体左移）"
    );
    a.shutdown().await;
}
