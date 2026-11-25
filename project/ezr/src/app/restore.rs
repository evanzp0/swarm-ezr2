use super::App;
use crate::model::registry::Registry;
use crate::model::sidecar::Sidecar;
use crate::model::Task;

impl App {
    pub(super) fn restore(path: &str) -> Vec<Task> {
        let Some(reg) = Registry::load(path) else {
            return Vec::new();
        };
        let mut tasks = reg.restore_tasks();
        for t in &mut tasks {
            let sc = Sidecar::load(&t.sidecar_path());
            if let Some(sc) = sc {
                t.downloaded = t.downloaded.max(sc.downloaded);
                t.resumable = !sc.non_resumable;
                // HTTP 分块还原：sidecar.blocks 记录逐块字节进度，
                // 满块 → 🟩 Done，零头块 → 🟨 Partial（否则重启后全白）。
                if t.done_blocks.is_empty() && !sc.blocks.is_empty() {
                    let mut done: Vec<u32> = Vec::new();
                    let mut part: Vec<u32> = Vec::new();
                    for (i, &w) in sc.blocks.iter().enumerate() {
                        let i = i as u32;
                        let full = crate::model::chunk::block_range(sc.size, sc.block_size, i)
                            .map_or(0, |(s, e)| e - s);
                        if full > 0 && w >= full {
                            done.push(i);
                        } else if w > 0 {
                            part.push(i);
                        }
                    }
                    t.chunk_done = t.chunk_done.max(done.len() as u32);
                    t.done_blocks = done;
                    t.partial_blocks = part;
                }
            }
            if t.protocol.is_bt() {
                if let Some(bt) = t.bt.clone() {
                    let bt_path = crate::model::namegen::sidecar_path_of(&t.save_dir, &t.name);
                    if let Some(pieces) = crate::model::BtSidecar::load_done(&bt_path) {
                        let pieces: Vec<u32> = pieces
                            .into_iter()
                            .filter(|p| {
                                u64::from(*p) * u64::from(bt.piece_length.max(1))
                                    < bt.files.iter().map(|f| f.size).sum::<u64>()
                            })
                            .collect();
                        t.downloaded = t.downloaded.max(bt.selected_bytes_of_pieces(&pieces));
                        t.chunk_done = pieces.len() as u32;
                        t.done_blocks = pieces;
                    }
                }
            }
        }
        tasks
    }
}

#[cfg(test)]
mod hardening_restore {
    use super::App;
    use crate::model::registry::Registry;
    use crate::model::sidecar::Sidecar;
    use crate::model::TaskState;

    fn http_task(id: u32, name: &str, save_dir: &str, done_blocks: Vec<u32>) -> crate::model::Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.protocol = crate::model::Protocol::Http;
        t.name = name.to_string();
        t.save_dir = save_dir.to_string();
        t.state = TaskState::Paused;
        t.total = 2_000;
        t.block_size = 1_000;
        t.done_blocks = done_blocks;
        t
    }

    fn write_registry(dir: &std::path::Path, tasks: &[crate::model::Task]) -> std::path::PathBuf {
        let reg = Registry::from_tasks(tasks);
        let reg_path = dir.join("registry.json");
        reg.save(reg_path.to_str().unwrap()).unwrap();
        reg_path
    }

    fn http_sidecar(
        dir: &str,
        id: u32,
        name: &str,
        size: u64,
        block: u64,
        blocks: &[u64],
        non_resumable: bool,
    ) {
        let task = crate::model::sidecar::SidecarTask {
            id,
            added_at: 0,
            save_dir: dir.to_string(),
            concurrency: 4,
            protocol: crate::model::Protocol::Http,
        };
        let stamp = crate::model::consistency::ServerStamp::default();
        let sc = Sidecar::build(&stamp, size, block, blocks, non_resumable, None, task);
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t.save_dir = dir.to_string();
        sc.save(&t.sidecar_path()).unwrap();
    }

    #[test]
    fn resumable_follows_sidecar_non_resumable_flag() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-rh-resumable");
        let sd = dir.to_string_lossy().into_owned();
        let t = http_task(11, "r.bin", &sd, vec![]);
        let reg_path = write_registry(&dir, &[t]);
        http_sidecar(&sd, 11, "r.bin", 2_000, 1_000, &[1_000, 400], true);
        let tasks = App::restore(reg_path.to_str().unwrap());
        assert!(
            !tasks[0].resumable,
            "sidecar non_resumable=true → 恢复后不可续传"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn restore_classifies_partial_and_ignores_zero_blocks_beyond_size() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-rh-zb");
        let sd = dir.to_string_lossy().into_owned();
        let t = http_task(13, "z.bin", &sd, vec![]);
        let reg_path = write_registry(&dir, &[t]);
        http_sidecar(
            &sd,
            13,
            "z.bin",
            2_000,
            1_000,
            &[1_000, 1_000, 400, 0, 0, 0, 0],
            false,
        );
        let tasks = App::restore(reg_path.to_str().unwrap());
        assert_eq!(tasks[0].done_blocks, vec![0, 1], "仅满块入 done");
        assert_eq!(
            tasks[0].partial_blocks,
            vec![2],
            "零头块入 partial；越界零块既非 done 也非 partial"
        );
        assert!(
            !tasks[0].done_blocks.contains(&5),
            "越界零块不得标记为 done"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn restore_bt_filters_piece_offset_exactly_at_total() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-rh-btb");
        let sd = dir.to_string_lossy().into_owned();
        let mut t = crate::model::sample_task();
        t.id = 14;
        t.protocol = crate::model::Protocol::Bt;
        t.name = "b.bin".to_string();
        t.url = "/tmp/b.bin.torrent".to_string();
        t.save_dir = sd.clone();
        t.state = TaskState::Queued;
        t.total = 200;
        t.block_size = 100;
        t.bt = Some(crate::model::BtTaskData {
            source: "/tmp/b.bin.torrent".to_string(),
            trackers: vec!["udp://t/ann".to_string()],
            piece_length: 100,
            files: vec![crate::model::BtFile {
                path: "b.bin".to_string(),
                size: 200,
                selected: true,
            }],
            upload_paused: false,
            sequential: false,
        });
        let reg_path = write_registry(&dir, &[t]);
        let _ = crate::model::BtSidecar::build([0, 1, 2])
            .save(&crate::model::namegen::sidecar_path_of(&sd, "b.bin"));
        let tasks = App::restore(reg_path.to_str().unwrap());
        assert_eq!(
            tasks[0].done_blocks,
            vec![0, 1],
            "piece 2 起始偏移恰等于 total（200）→ 越界陈旧片被过滤"
        );
        assert_eq!(tasks[0].downloaded, 200);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
