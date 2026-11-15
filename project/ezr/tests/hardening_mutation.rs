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

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use app::{App, Dialog, DialogKind};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use model::{unix_now, Protocol, ProxyChoice, Task, TaskState};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn uniq_dir(prefix: &str) -> PathBuf {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let d = std::env::temp_dir().join(format!("{prefix}-{}-{seq}-{ms}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn mk_app(tag: &str) -> App {
    let reg = uniq_dir(&format!("ezr-mut-{tag}")).join("registry.json");
    App::new(Config::default(), reg.to_string_lossy().into_owned())
}

use model::config::Config;

fn write_torrent(dir: &std::path::Path, file_name: &str, announce: &str, name: &str) -> String {
    let pieces = [0u8; 20];
    let mut buf = Vec::new();
    buf.extend_from_slice(b"d8:announce");
    buf.extend_from_slice(format!("{}:{}", announce.len(), announce).as_bytes());
    buf.extend_from_slice(b"4:infod5:filesl");
    buf.extend_from_slice(b"d6:lengthi5e4:pathl3:sub5:a.txtee");
    buf.extend_from_slice(b"d6:lengthi7e4:pathl5:b.binee");
    buf.extend_from_slice(b"e4:name");
    buf.extend_from_slice(format!("{}:{}", name.len(), name).as_bytes());
    buf.extend_from_slice(b"12:piece lengthi4e6:pieces20:");
    buf.extend_from_slice(&pieces);
    buf.push(b'e');
    buf.push(b'e');
    let path = dir.join(file_name);
    std::fs::write(&path, &buf).unwrap();
    path.to_string_lossy().to_string()
}

fn parse_at(path: &str) -> bt::metainfo::TorrentMeta {
    let bytes = std::fs::read(path).unwrap();
    bt::metainfo::TorrentMeta::parse(&bytes).unwrap()
}

fn mk_task(id: u32, name: &str, url: &str, dir: &str, proxy: ProxyChoice) -> Task {
    Task::new_queued(
        id,
        name.into(),
        model::Protocol::Bt,
        url.into(),
        dir.into(),
        1024,
        2,
        3,
        None,
        proxy,
        unix_now(),
    )
}

#[tokio::test]
async fn mut_m057_bt_list_char_n_deselects_all() {
    let dir = uniq_dir("ezr-mut-m057");
    let path = write_torrent(&dir, "a.torrent", "udp://seed/ann", "tor");
    let meta = parse_at(&path);
    let mut app = mk_app("m057");
    app.open_add_bt_dialog_with(&path, &meta);
    let list_focus = app::Dialog::bt_list_focus(DialogKind::AddBt);
    if let Some(d) = app.dialog.as_mut() {
        d.focus = list_focus;
    }
    app.on_key(KeyCode::Char('a'), KeyModifiers::NONE);
    let d = app.dialog.as_ref().expect("对话框在位");
    assert!(d.bt_tree.iter().any(|n| n.selected), "'a' 后应有节点被全选");
    app.on_key(KeyCode::Char('n'), KeyModifiers::NONE);
    let d = app.dialog.as_ref().expect("对话框在位");
    assert!(
        d.bt_tree.iter().all(|n| !n.selected),
        "'n' 必须清除全部选择"
    );
    app.shutdown().await;
}

#[tokio::test]
async fn mut_m019_bt_boot_io_fail_reports_failed() {
    let dir = uniq_dir("ezr-mut-m019");
    let path = write_torrent(&dir, "a.torrent", "udp://seed/ann", "tor");
    let meta = parse_at(&path);
    let mut app = mk_app("m019");
    app.open_add_bt_dialog_with(&path, &meta);
    if let Some(d) = app.dialog.as_mut() {
        d.dir.value = "/dev/null".into();
    }
    app.dlg_confirm_add_bt(false);
    assert_eq!(app.tasks.len(), 1, "任务应创建");
    pump(&mut app, 15, |a| {
        a.tasks
            .first()
            .is_some_and(|t| t.state == TaskState::Failed)
    })
    .await;
    assert_eq!(
        app.tasks[0].state,
        TaskState::Failed,
        "BT 引擎启动期 Io 失败必须经 report_fail 送达失败终态"
    );
    app.shutdown().await;
}

#[tokio::test]
async fn mut_m004_modify_proxy_prefill_selects_matching_option() {
    let mut app = mk_app("m004");
    app.proxy_options = vec![
        ProxyChoice::Direct,
        ProxyChoice::Named("np1".into()),
        ProxyChoice::Named("np2".into()),
    ];
    app.proxy_labels = vec!["直连".into(), "np1".into(), "np2".into()];
    app.tasks.push(mk_task(
        1,
        "t1",
        "src-a",
        "/tmp/ezr-mut-a",
        ProxyChoice::Named("np1".into()),
    ));
    app.selected = 0;
    app.open_modify_bt_dialog();
    let d = app.dialog.as_ref().expect("修改对话框应打开");
    assert_eq!(d.proxy_sel, 1, "代理预选必须命中任务代理所在的选项下标");
    app.shutdown().await;
    std::fs::remove_dir_all(uniq_dir("ezr-mut-m004")).ok();
}

#[tokio::test]
async fn mut_m005_m006_modify_proxy_unknown_appended_at_last_index() {
    let mut app = mk_app("m005");
    app.proxy_options = vec![ProxyChoice::Direct];
    app.proxy_labels = vec!["直连".into()];
    app.tasks.push(mk_task(
        1,
        "t1",
        "src-a",
        "/tmp/ezr-mut-a",
        ProxyChoice::Named("npX".into()),
    ));
    app.selected = 0;
    app.open_modify_bt_dialog();
    let n = app.proxy_options.len();
    let last = app.proxy_options.last().cloned().expect("选项非空");
    let d = app.dialog.as_ref().expect("修改对话框应打开");
    assert_eq!(
        last,
        ProxyChoice::Named("npX".into()),
        "未知代理应追加到选项表"
    );
    assert_eq!(n, 2, "追加后恰为 2 项");
    assert_eq!(d.proxy_sel, n - 1, "追加项必须被选中（末位下标）");
    app.shutdown().await;
}

#[tokio::test]
async fn mut_m011_make_bt_spec_tcp_flag_from_config() {
    let dir = uniq_dir("ezr-mut-m011");
    let path = write_torrent(&dir, "a.torrent", "udp://seed/ann", "tor");
    let meta = parse_at(&path);
    let mut app = mk_app("m011");
    app.cfg.peer_connection_protocol = vec!["tcp".into()];
    let t = mk_task(1, "t1", &path, dir.to_str().unwrap(), ProxyChoice::Direct);
    let spec = app.make_bt_spec(&t, &meta);
    assert!(
        spec.tunables.transports.tcp,
        "tcp 配置必须映射 PeerTransports.tcp"
    );
    assert!(!spec.tunables.transports.webrtc, "tcp 配置不得误开 webrtc");
    app.shutdown().await;
}

#[tokio::test]
async fn mut_m012_make_bt_spec_webrtc_flag_from_config() {
    let dir = uniq_dir("ezr-mut-m012");
    let path = write_torrent(&dir, "a.torrent", "udp://seed/ann", "tor");
    let meta = parse_at(&path);
    let mut app = mk_app("m012");
    app.cfg.peer_connection_protocol = vec!["webrtc".into()];
    let t = mk_task(1, "t1", &path, dir.to_str().unwrap(), ProxyChoice::Direct);
    let spec = app.make_bt_spec(&t, &meta);
    assert!(
        spec.tunables.transports.webrtc,
        "webrtc 配置必须映射 PeerTransports.webrtc"
    );
    assert!(!spec.tunables.transports.tcp, "webrtc 配置不得误开 tcp");
    app.shutdown().await;
}

#[tokio::test]
async fn mut_m016_trackers_newline_success_keeps_toast_none() {
    let dir = uniq_dir("ezr-mut-m016");
    let path = write_torrent(&dir, "a.torrent", "udp://seed/ann", "tor");
    let meta = parse_at(&path);
    let mut app = mk_app("m016");
    app.open_add_bt_dialog_with(&path, &meta);
    assert!(app.dialog.is_some());
    let before = app.dialog.as_ref().unwrap().trackers.lines.len();
    app.bt_trackers_newline();
    let d = app.dialog.as_ref().unwrap();
    assert_eq!(d.trackers.lines.len(), before + 1, "成功换行必须新增一行");
    assert!(
        !app.toast
            .as_deref()
            .is_some_and(|t| t.contains("trackers 最多")),
        "成功换行不得触发行数上限 toast"
    );
    app.shutdown().await;
}

#[tokio::test]
async fn mut_m032_add_dialog_char_focus1_inserts_into_dir() {
    let dir = uniq_dir("ezr-mut-m032");
    let path = write_torrent(&dir, "a.torrent", "udp://seed/ann", "tor");
    let meta = parse_at(&path);
    let mut app = mk_app("m032");
    app.open_add_bt_dialog_with(&path, &meta);
    if let Some(d) = app.dialog.as_mut() {
        d.focus = 1;
    }
    app.on_key(KeyCode::Char('x'), KeyModifiers::NONE);
    let d = app.dialog.as_ref().expect("对话框在位");
    assert!(d.dir.value.contains('x'), "focus=1 的字符必须落入目录字段");
    app.shutdown().await;
}

fn draw_and_click(app: &mut App, want_btn: usize) {
    let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
    term.draw(|f| ui::draw(f, app)).unwrap();
    let rects = app
        .dlg_btn_rects
        .iter()
        .find(|(_, idx)| *idx == want_btn)
        .or_else(|| app.dlg_btbtn_rects.iter().find(|(_, idx)| *idx == want_btn))
        .map(|(r, _)| *r)
        .expect("目标按钮热区必须在渲染后登记");
    let col = rects.x + rects.width / 2;
    let row = rects.y + rects.height / 2;
    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: KeyModifiers::NONE,
    });
}

#[tokio::test]
async fn mut_m061_activate_add_btn10_confirms_paused_task() {
    let dir = uniq_dir("ezr-mut-m061");
    let path = write_torrent(&dir, "a.torrent", "udp://seed/ann", "tor");
    let meta = parse_at(&path);
    let mut app = mk_app("m061");
    app.open_add_bt_dialog_with(&path, &meta);
    draw_and_click(&mut app, 10);
    assert_eq!(app.tasks.len(), 1, "按钮 10 必须确认添加");
    assert_eq!(app.tasks[0].state, TaskState::Paused, "按钮 10 = 暂停添加");
    app.shutdown().await;
}

#[tokio::test]
async fn mut_m064_activate_modify_btn7_applies_conns() {
    let dir = uniq_dir("ezr-mut-m064");
    let path = write_torrent(&dir, "a.torrent", "udp://seed/ann", "tor");
    let meta = parse_at(&path);
    let mut app = mk_app("m064");
    app.open_add_bt_dialog_with(&path, &meta);
    app.dlg_confirm_add_bt(false);
    assert_eq!(app.tasks.len(), 1);
    app.selected = 0;
    app.open_modify_bt_dialog();
    if let Some(d) = app.dialog.as_mut() {
        d.conns.set("9");
    }
    draw_and_click(&mut app, 7);
    assert_eq!(
        app.tasks[0].concurrency, 9,
        "按钮 7 必须确认修改（并发数落盘）"
    );
    app.shutdown().await;
}

#[tokio::test]
async fn mut_m086_m087_confirm_merges_ws_and_wss_schemes() {
    let dir = uniq_dir("ezr-mut-m086");
    let path = write_torrent(&dir, "a.torrent", "http://h/ann", "tor");
    let meta = parse_at(&path);
    let mut app = mk_app("m086");
    app.cfg.trackers = vec![
        "udp://u/ann".into(),
        "ws://w/ann".into(),
        "wss://s/ann".into(),
    ];
    app.open_add_bt_dialog_with(&path, &meta);
    if let Some(d) = app.dialog.as_mut() {
        d.trackers.set_text("");
    }
    app.dlg_confirm_add_bt(false);
    assert_eq!(app.tasks.len(), 1, "任务应创建");
    let trackers = app.tasks[0].bt.as_ref().expect("bt 数据").trackers.clone();
    assert_eq!(
        trackers,
        vec![
            "udp://u/ann".to_string(),
            "ws://w/ann".to_string(),
            "wss://s/ann".to_string(),
            "http://h/ann".to_string(),
        ],
        "ws/wss 属白名单 scheme，必须进 merged 而非 ignored"
    );
    app.shutdown().await;
}

#[tokio::test]
async fn mut_m103_confirm_name_dedup_resolves_second_conflict() {
    let dir = uniq_dir("ezr-mut-m103");
    let path = write_torrent(&dir, "c.torrent", "udp://z/ann", "tor");
    let meta = parse_at(&path);
    let save = uniq_dir("ezr-mut-m103-save");
    let save_s = save.to_string_lossy().to_string();
    let mut app = mk_app("m103");
    app.tasks
        .push(mk_task(1, "tor", "src-a", &save_s, ProxyChoice::Direct));
    app.tasks
        .push(mk_task(2, "tor.1", "src-b", &save_s, ProxyChoice::Direct));
    app.open_add_bt_dialog_with(&path, &meta);
    if let Some(d) = app.dialog.as_mut() {
        d.dir.value = save_s.clone();
    }
    app.dlg_confirm_add_bt(false);
    assert_eq!(app.tasks.len(), 3, "第三任务应创建");
    assert_eq!(
        app.tasks[2].name, "tor.2",
        "双重名冲突必须推进到第二个空闲序号"
    );
    app.shutdown().await;
}

fn no_range_stub_addr(total: u64) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let mut w = match conn.try_clone() {
                Ok(w) => w,
                Err(_) => continue,
            };
            let mut r = BufReader::new(conn);
            let mut line = String::new();
            if r.read_line(&mut line).is_err() || line.is_empty() {
                continue;
            }
            loop {
                let mut h = String::new();
                match r.read_line(&mut h) {
                    Ok(0) | Err(_) => break,
                    Ok(_) if h.trim().is_empty() => break,
                    Ok(_) => {}
                }
            }
            let body: Vec<u8> = (0..total).map(|i| (i as usize % 251) as u8).collect();
            let head = format!(
                "HTTP/1.1 200 OK\r\nAccept-Ranges: bytes\r\nETag: \"stub-{total}\"\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n"
            );
            if w.write_all(head.as_bytes()).is_err() {
                continue;
            }
            for chunk in body.chunks(8192) {
                if w.write_all(chunk).is_err() {
                    break;
                }
                let _ = w.flush();
                std::thread::sleep(std::time::Duration::from_millis(60));
            }
        }
    });
    addr
}

async fn pump(app: &mut App, secs: u64, done: impl Fn(&App) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        app.tick().await;
        if done(app) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn mut_single_m015_norange_path_emits_single_stream_toast() {
    let save = uniq_dir("ezr-mut-m015-save");

    let addr = no_range_stub_addr(300_000);
    let mut app = mk_app("m015");
    app.add_cli_task(
        format!("http://{addr}/f.bin"),
        Some(save.to_string_lossy().into_owned()),
        None,
        None,
    );
    let toast_seen = Arc::new(AtomicBool::new(false));
    let flag = toast_seen.clone();
    pump(&mut app, 25, |a| {
        if a.toast
            .as_deref()
            .is_some_and(|t| t.contains("不支持断点续传"))
        {
            flag.store(true, Ordering::Relaxed);
        }
        a.tasks
            .first()
            .is_some_and(|t| t.state == TaskState::Completed)
    })
    .await;
    assert!(
        toast_seen.load(Ordering::Relaxed),
        "norange 服务器必须触发「单线程下载」toast"
    );
    assert_eq!(
        app.tasks[0].state,
        TaskState::Completed,
        "单流回退应完成下载"
    );
    app.shutdown().await;
}
