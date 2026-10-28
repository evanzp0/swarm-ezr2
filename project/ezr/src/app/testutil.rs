use super::{App, Dialog, DialogKind};
use crate::textfield::TextInput;

pub(crate) fn make_app_in(prefix: &str, tag: &str) -> App {
    let dir = crate::model::testenv::uniq_tmp_dir(&format!("{prefix}-{tag}"));
    let reg = dir.join("registry.json").to_string_lossy().into_owned();
    App::new(crate::model::config::Config::default(), reg)
}

pub(crate) fn dl_task(id: u32, name: &str) -> crate::model::Task {
    let mut t = crate::model::sample_task();
    t.id = id;
    t.name = name.to_string();
    t.state = crate::model::TaskState::Downloading;
    t.probed = true;
    t.total = 5_000_000;
    t
}

pub(crate) fn dl_task_mb(id: u32, name: &str) -> crate::model::Task {
    let mut t = dl_task(id, name);
    t.block_size = 1_000_000;
    t
}

pub(crate) fn seed_queued(app: &mut App, n: usize) {
    for _ in 0..n {
        let mut t = dl_task(1, "t.bin");
        t.state = crate::model::TaskState::Queued;
        app.tasks.push(t);
    }
}

pub(crate) fn dialog(kind: DialogKind, focus: usize) -> Dialog {
    Dialog {
        kind,
        url: TextInput::new(),
        dir: TextInput::new(),
        conns: TextInput::new(),
        conns_edited: false,
        ck_type: 4,
        ck_value: TextInput::new(),
        ck_open: false,
        ck_sel: 4,
        proxy_sel: 0,
        proxy_open: false,
        focus,
        task_name: String::new(),
        task_id: None,
        torrent: TextInput::new(),
        bt_tree: Vec::new(),
        bt_cursor: 0,
        bt_scroll: 0,
        bt_follow: true,
        trackers: crate::multiline::MultiLineBuf::new(),
    }
}

pub(crate) fn preset_resume_sidecar(dir: &std::path::Path) {
    let sc = crate::model::sidecar::Sidecar::build(
        &crate::model::consistency::ServerStamp {
            etag: None,
            last_modified: None,
            size: Some(1000),
        },
        1000,
        1024 * 1024,
        &[0],
        false,
        None,
        crate::model::sidecar::SidecarTask {
            id: 99,
            added_at: 0,
            save_dir: dir.to_string_lossy().into_owned(),
            concurrency: 4,
            protocol: crate::model::Protocol::Http,
        },
    );
    sc.save(&dir.join("r.bin.ezr").to_string_lossy()).ok();
}

pub(crate) fn task_with_conns(
    id: u32,
    name: &str,
    state: crate::model::TaskState,
    n: usize,
) -> crate::model::Task {
    let mut t = crate::model::sample_task();
    t.id = id;
    t.name = name.to_string();
    t.state = state;
    t.connections = (1..=n)
        .map(|i| crate::model::Connection {
            id: i,
            start: (i as u64 - 1) * 1000,
            end: i as u64 * 1000,
            done: 100,
        })
        .collect();
    t
}
