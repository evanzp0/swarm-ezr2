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

pub const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-01");

#[path = "../src/app/mod.rs"]
mod app;
#[path = "../src/bt/mod.rs"]
mod bt;
#[path = "../src/engine/mod.rs"]
mod engine;
#[path = "../src/model/mod.rs"]
mod model;
#[path = "../src/multiline.rs"]
mod multiline;
#[path = "../src/scrollbar.rs"]
mod scrollbar;
#[path = "../src/textfield.rs"]
mod textfield;
#[path = "../src/ui/mod.rs"]
mod ui;

use app::{Dialog, DialogKind};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use model::config::Config;
use model::{Connection, Protocol, Task, TaskState};
use ratatui::layout::Rect;
use ui::testfx::{make_app_in, render_full, row_strings, task_in};

mod chart_fix {
    #[allow(clippy::items_after_test_module)]
    pub mod chart {
        include!("gen/chart_stripped.rs");

        pub fn cols(hist: &[u64], w: usize, h: usize) -> Vec<usize> {
            chart_columns(hist, w, h)
        }

        pub fn draw_iso(f: &mut ratatui::Frame, a: &crate::app::App, area: Rect) {
            draw_chart(f, a, area)
        }
    }
    pub const LIGHT_BLUE: ratatui::style::Color = ratatui::style::Color::Rgb(122, 185, 242);
}

mod clip_fix {
    pub mod clipboard {
        include!("gen/clipboard_stripped.rs");
    }
    #[allow(dead_code)]
    pub struct App {
        pub last_copied: Option<String>,
        pub clipboard: Option<arboard::Clipboard>,
    }
}

mod btn_fix {
    pub mod text {
        pub(crate) use crate::ui::w;
    }
    use ratatui::style::Color;

    pub(crate) use crate::scrollbar::DIM2;
    pub(crate) use crate::ui::panel_border_style;
    pub const ACCENT: Color = Color::Cyan;
    pub const FG: Color = Color::Rgb(198, 208, 212);
    pub const RED: Color = Color::Rgb(226, 92, 84);
    pub mod btn {
        include!("gen/btn_stripped.rs");
        pub fn clip_right(buf: &mut ratatui::buffer::Buffer, xr: u16, y: u16) {
            clip_right_edge(buf, xr, y)
        }
    }
}

mod detail_fix {
    pub mod text {
        pub(crate) use crate::ui::text::{break_links_deep, link_break_spans};
        pub(crate) use crate::ui::{fmt_size, pad_right, truncate, w};
    }
    use ratatui::style::Color;

    pub(crate) use crate::ui::{panel_border_style, panel_title_style};
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
        }
    }
    pub mod detail {
        pub fn hot(inner: Rect, line_idx: usize) -> Option<Rect> {
            let y = inner.y + line_idx as u16;
            (y < inner.y + inner.height).then(|| Rect {
                x: inner.x + 2,
                y,
                width: 9,
                height: 1,
            })
        }
        include!("gen/detail_stripped.rs");
    }
}

fn assert_fixture_matches(product: &str, fixture: &str) {
    let src = std::fs::read_to_string(product).unwrap_or_else(|e| panic!("read {product}: {e}"));
    let fix = std::fs::read_to_string(fixture).unwrap_or_else(|e| panic!("read {fixture}: {e}"));
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
}

#[test]
fn gen_fixtures_match_product_sources() {
    assert_fixture_matches("src/ui/chart.rs", "tests/gen/chart_stripped.rs");
    assert_fixture_matches("src/app/clipboard.rs", "tests/gen/clipboard_stripped.rs");
    assert_fixture_matches("src/ui/detail.rs", "tests/gen/detail_stripped.rs");
    assert_fixture_matches("src/ui/btn.rs", "tests/gen/btn_stripped.rs");
}

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

fn add_via_dialog(a: &mut app::App, url: &str, dir: &str) {
    a.open_add_dialog();
    let d = a.dialog.as_mut().expect("打开添加对话框（直加通道）");
    d.url.set(url);
    d.dir.set(dir);
    tap(a, KeyCode::Enter);
}

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

#[tokio::test]
async fn confirm_add_resume_task_arm_requires_full_conjunction() {
    let mut a = make_app_in("ezr-h118", "dup179");
    let dir_s = model::testenv::uniq_tmp_dir("ezr-h118-dup179")
        .to_string_lossy()
        .to_string();
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

#[tokio::test]
async fn confirm_add_dedupe_renames_on_task_table_match() {
    let mut a = make_app_in("ezr-h118", "dedupe189");
    let dir = model::testenv::uniq_tmp_dir("ezr-h118-dedupe189");
    let dir_s = dir.to_string_lossy().to_string();
    a.tasks
        .push(queued_task(1, "dup.bin", "http://a.com/dup.bin", &dir_s));
    add_via_dialog(&mut a, "http://b.com/dup.bin", &dir_s);
    assert_eq!(a.tasks.len(), 2, "异 URL 不构成重复，应创建");
    assert_ne!(
        a.tasks[1].name, "dup.bin",
        "表内同名必须改名（mutant 189:21 && 后不改名）"
    );
    a.shutdown().await;
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn confirm_add_assigns_unique_random_id() {
    let mut a = make_app_in("ezr-h118", "nid");
    let existing_ids: Vec<u32> = a.tasks.iter().map(|t| t.id).collect();
    add_via_dialog(&mut a, "http://example.com/fresh.bin", "/tmp");
    let fresh = a.tasks.last().expect("新任务已创建").id;
    assert_ne!(fresh, 0, "0 为保留哨兵，不得分配");
    assert!(
        !existing_ids.contains(&fresh),
        "新任务 ID 不得与既有任务撞号"
    );
    a.shutdown().await;
}

#[tokio::test]
async fn confirm_add_selects_last_row() {
    let mut a = make_app_in("ezr-h118", "sel");
    add_via_dialog(&mut a, "http://example.com/only.bin", "/tmp");
    assert_eq!(a.filtered().len(), 1);
    assert_eq!(a.selected, 0, "单任务列表选中第 0 行（flen-1）");
    a.shutdown().await;
}

#[tokio::test]
async fn confirm_add_clamps_out_of_range_proxy_sel() {
    let mut a = make_app_in("ezr-h118", "clamp");
    a.open_add_dialog();
    let len = a.proxy_options.len();
    {
        let d = a.dialog.as_mut().expect("add dialog");
        d.url.set("http://example.com/px.bin");
        d.proxy_sel = len;
    }
    tap(&mut a, KeyCode::Enter);
    assert_eq!(a.tasks.len(), 1, "越界 proxy_sel 应被钳回并正常创建");
    a.shutdown().await;
}

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
    a.move_conn_sel(5);
    assert_eq!(a.conns_sel, 5);
    assert_eq!(
        a.conns_scroll, 4,
        "滚动跟随 = new+1-vis（mutant new+1/vis=5）"
    );
    a.shutdown().await;
}

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
    a.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 32, 2));
    let d = a.dialog.as_ref().unwrap();
    assert!(!d.ck_open, "右缘列不命中算法下拉");
    assert_ne!(d.focus, 3, "右缘列不聚焦");
    a.shutdown().await;
}

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
    a.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 10));
    assert_eq!(a.selected, 0, "列表底界外一格不选中");
    a.shutdown().await;
}

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
    a.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 5, 9));
    assert_eq!(a.selected, 0, "ti == len 不选中（防越界）");
    a.shutdown().await;
}

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

#[tokio::test]
async fn detail_hot_rect_right_edge_exclusive() {
    let mut a = make_app_in("ezr-h118", "pedge");
    let t = queued_task(1, "copy.bin", "http://example.com/copy.bin", "/tmp");
    a.tasks.push(t);
    a.detail_url_rect = Some(Rect {
        x: 1,
        y: 7,
        width: 9,
        height: 1,
    });
    a.on_mouse(mev(MouseEventKind::Down(MouseButton::Left), 10, 7));
    assert!(a.last_copied.is_none(), "热区右缘列不触发复制");
    a.shutdown().await;
}

#[test]
fn osc52_b64_single_byte_tail_pads() {
    assert_eq!(
        clip_fix::clipboard::osc52_sequence("a"),
        "\x1b]52;c;YQ==\x07",
        "1 字节载荷：两处 '=' 填充（mutant 输出 YQAA）"
    );
}

#[test]
#[allow(clippy::type_complexity)]
fn chart_columns_golden_vectors() {
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

#[tokio::test]
async fn draw_chart_isolated_goldens() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut a = make_app_in("ezr-h118", "chart");
    a.speed_hist = vec![10, 900];

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

    let mut term2 = Terminal::new(TestBackend::new(30, 8)).unwrap();
    term2
        .draw(|f| chart_fix::chart::draw_iso(f, &a, Rect::new(0, 0, 1, 3)))
        .unwrap();
    for (i, r) in row_strings(&term2).iter().enumerate() {
        assert_eq!(r.trim(), "", "w=1 守卫下整帧无图形（row {i}）");
    }
    a.shutdown().await;
}

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

#[tokio::test]
async fn detail_chunk_row_shows_blocks() {
    let mut a = make_app_in("ezr-h118", "cr");
    let mut t = task_in(1, "cr.bin", TaskState::Queued);
    t.url = "http://example.com/cr.bin".into();
    t.save_dir = "/tmp".into();
    a.tasks.push(t);
    a.selected = 0;
    let term = render_full(&mut a, 100, 28);
    let rows = row_strings(&term);
    let hit = rows.iter().find(|r| r.contains("分 块"));
    assert!(hit.is_some(), "分块行出现");
    assert!(
        hit.unwrap().contains("/块"),
        "分块行显示 N/块（mutant 误入 '-' 臂）: {hit:?}"
    );
    a.shutdown().await;
}

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
            x: 5,
            y: 10,
            width: 9,
            height: 1
        }),
        "行号 5 < 高度 6 → 设热区（正控；x = 内框 +2，v1.34 padding）"
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
        "│      │  删 除 任 务  「 删 除 目 标 文 件 .bin」 ？                                │      │",
        "正文行 x=inner.x+2 定位金样（靶 53:24，mutant 左移压边；「」后无空格、\
         前导 2 列由定位承载——x 偏 1 列即红）"
    );
    assert_eq!(
        rows[12],
        "│      │        [ 仅 删 除 任 务  ]    [ 删 除 任 务 和 文 件  ]    [ 取 消  ]        │      │",
        "按钮行居中定位金样（mutant 整行左移 2 列）"
    );
    assert!(
        !rows.iter().any(|r| r.contains("1/2/3 快 捷 选 择")),
        "底部提示行退役（v1.34/FR-01-25 P10；靶 77:24 位点随行删除消失，\
         x 定位由正文行金样承担）"
    );
    a.shutdown().await;
}

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

#[test]
fn clip_right_edge_fills_orphan_half_cell() {
    use ratatui::buffer::Buffer;
    let mut buf = Buffer::empty(Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 1,
    });
    let i2 = buf.index_of(2, 0);
    let i3 = buf.index_of(3, 0);
    buf.content[i2].set_symbol("中");
    buf.content[i3].set_symbol("");
    btn_fix::btn::clip_right(&mut buf, 2, 0);
    assert_eq!(
        buf.content[buf.index_of(3, 0)].symbol(),
        " ",
        "孤立半格必须补空格（mutant 整体替换为 () 后保持空）"
    );
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
    let mut buf3 = Buffer::empty(Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 1,
    });
    btn_fix::btn::clip_right(&mut buf3, 9, 0);
}

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

#[tokio::test]
async fn add_dialog_button_row_centering_golden() {
    let mut a = make_app_in("ezr-h118", "btn");
    a.open_add_dialog();
    let rows = row_strings(&render_full(&mut a, 80, 24));
    let hit = rows.iter().find(|r| r.contains("[ 立 即"));
    assert!(hit.is_some(), "立即下载按钮行出现（FR-01-103 三钮）");
    assert_eq!(
        hit.unwrap(),
        "│  │                 [ 立 即 下 载  ]    [ 仅 添 加  ]    [ 取 消  ]                 │  │",
        "按钮行居中金样（mutant % 使按钮组整体左移；v1.15 三钮口径）"
    );
    a.shutdown().await;
}

#[tokio::test]
async fn detail_url_value_deep_broken_including_www() {
    let url = "https://www.example.com/media-release.zip";
    let dir = model::testenv::uniq_tmp_dir("ezr-detail-www-break");
    let mut app = app::App::new(
        model::config::Config::default(),
        dir.join("registry.json").to_string_lossy().into_owned(),
    );
    let mut t = sample_task(1, "www.bin");
    t.url = url.to_string();
    t.state = model::TaskState::Paused;
    app.tasks.push(t);
    app.selected = 0;
    let term = ui::testfx::render_full(&mut app, 110, 24);
    let buf = term.backend().buffer();
    let rows = ui::testfx::row_strings(&term);
    let y = rows
        .iter()
        .position(|r| r.contains("URL"))
        .expect("详情面板有 URL 行");
    let row = &rows[y];
    assert!(!row.contains("https"), "scheme 断链: {row:?}");
    assert!(!row.contains("www."), "www 前缀断链: {row:?}");
    assert!(!row.contains("example"), "域名断链: {row:?}");
    let label_x = (0..110u16)
        .find(|&x| {
            buf.cell((x, y as u16))
                .map(|c| c.symbol() == "U")
                .unwrap_or(false)
        })
        .expect("URL label 起始列");
    let value_x = label_x + 9;
    let mut visible = 0usize;
    let mut broken = 0usize;
    for x in value_x..110u16 {
        let sym = buf.cell((x, y as u16)).unwrap().symbol();
        if sym == " " {
            break;
        }
        visible += 1;
        if sym.contains('\u{FE0E}') {
            broken += 1;
        }
    }
    assert_eq!(broken, visible, "URL 值区每格携带 VS15（{visible} 格）");
    assert!(
        visible >= 20,
        "URL 值区可见格数合理（110 列终端不触发截断）: {visible}"
    );
}
