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
