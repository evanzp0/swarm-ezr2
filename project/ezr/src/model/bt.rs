use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BtFile {
    pub path: String,
    pub size: u64,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BtTaskData {
    pub source: String,
    pub trackers: Vec<String>,
    pub piece_length: u32,
    pub files: Vec<BtFile>,
}

impl BtTaskData {
    #[must_use]
    pub fn selected_bytes_of_pieces(&self, pieces: &[u32]) -> u64 {
        let pl = u64::from(self.piece_length).max(1);
        let mut offsets = Vec::with_capacity(self.files.len());
        let mut acc = 0u64;
        for f in &self.files {
            offsets.push(acc);
            acc += f.size;
        }
        let total = acc;
        let mut sum = 0u64;
        for &p in pieces {
            let start = u64::from(p) * pl;
            if start >= total {
                continue;
            }
            let end = (start + pl).min(total);
            for (i, f) in self.files.iter().enumerate() {
                if !f.selected || f.size == 0 {
                    continue;
                }
                let f_start = offsets[i];
                let f_end = f_start + f.size;
                let lo = start.max(f_start);
                let hi = end.min(f_end);
                if lo < hi {
                    sum += hi - lo;
                }
            }
        }
        sum
    }
}

pub const BT_SIDECAR_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BtSidecar {
    pub version: u32,
    pub done_pieces: Vec<u32>,
}

impl BtSidecar {
    #[must_use]
    pub fn build(pieces: impl IntoIterator<Item = u32>) -> BtSidecar {
        let mut done: Vec<u32> = pieces.into_iter().collect();
        done.sort_unstable();
        done.dedup();
        BtSidecar {
            version: BT_SIDECAR_VERSION,
            done_pieces: done,
        }
    }

    pub fn save(&self, path: &str) -> std::io::Result<()> {
        super::save_json_atomic(self, path)
    }

    #[must_use]
    pub fn load_done(path: &str) -> Option<Vec<u32>> {
        let text = std::fs::read_to_string(path).ok()?;
        let sc: BtSidecar = serde_json::from_str(&text).ok()?;
        // 版本 1 为当前唯一版本；2 为内部开发期短暂出现过的同形版本号
        // （bug6 轮回退，遗留 v2 文件与 v1 字节同形），同形放行以还原
        // 遗留任务进度；其余版本/形状一律拒绝（v1 全量档兼容已删除）。
        if sc.version != BT_SIDECAR_VERSION && sc.version != 2 {
            return None;
        }
        Some(sc.done_pieces)
    }

    pub fn remove(path: &str) -> std::io::Result<()> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> String {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let d = std::env::temp_dir().join(format!("ezr-btsc-{tag}-{}-{ms}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d.to_string_lossy().to_string()
    }

    fn sample() -> BtSidecar {
        BtSidecar::build([5, 0, 2, 2])
    }

    #[test]
    fn roundtrip_preserves_done_pieces() {
        let dir = tmp_dir("rt");
        let p = format!("{dir}/tor.ezr");
        let sc = sample();
        sc.save(&p).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        let loaded: BtSidecar = serde_json::from_str(&text).unwrap();
        assert_eq!(loaded, sc, "存取后字段全等");
        assert_eq!(loaded.version, BT_SIDECAR_VERSION);
        assert_eq!(loaded.done_pieces, vec![0, 2, 5], "构建即排序去重");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_done_reads_v1_and_rejects_unknown() {
        let dir = tmp_dir("ver");
        let p = format!("{dir}/a.ezr");
        sample().save(&p).unwrap();
        assert_eq!(BtSidecar::load_done(&p), Some(vec![0, 2, 5]));
        // 遗留 v2（bug6 轮回退前的同形版本号）同形放行，进度可还原。
        std::fs::write(&p, "{\"version\":2,\"done_pieces\":[0,2,5]}").unwrap();
        assert_eq!(
            BtSidecar::load_done(&p),
            Some(vec![0, 2, 5]),
            "遗留 v2 同形文件放行"
        );
        std::fs::write(&p, "{\"version\":99,\"done_pieces\":[1]}").unwrap();
        assert!(BtSidecar::load_done(&p).is_none(), "未知版本拒绝");
        // 旧 v1 全量档（嵌套 data）形状不兼容，拒绝（bug6 定案不保留兼容）。
        let legacy = serde_json::json!({
            "version": 1,
            "id": 9,
            "name": "tor",
            "save_dir": "/dl",
            "concurrency": 4,
            "data": {"done_pieces": [0, 2, 5]}
        });
        std::fs::write(&p, legacy.to_string()).unwrap();
        assert!(BtSidecar::load_done(&p).is_none(), "旧全量档嵌套形状拒绝");
        std::fs::write(&p, "not json").unwrap();
        assert!(BtSidecar::load_done(&p).is_none(), "坏 JSON 拒绝");
        assert!(
            BtSidecar::load_done(&format!("{dir}/absent.ezr")).is_none(),
            "缺文件 None"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn remove_is_idempotent() {
        let dir = tmp_dir("rm");
        let p = format!("{dir}/gone.ezr");
        sample().save(&p).unwrap();
        BtSidecar::remove(&p).unwrap();
        assert!(!std::path::Path::new(&p).exists());
        BtSidecar::remove(&p).unwrap();
        BtSidecar::remove("/nonexistent/x.ezr").unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }

    fn data(files: Vec<BtFile>, piece_length: u32) -> BtTaskData {
        BtTaskData {
            source: "/tmp/tor.torrent".into(),
            trackers: vec!["udp://t/ann".into()],
            piece_length,
            files,
        }
    }

    #[test]
    fn selected_bytes_counts_only_selected_overlap() {
        let d = data(
            vec![
                BtFile {
                    path: "a".into(),
                    size: 100,
                    selected: true,
                },
                BtFile {
                    path: "b".into(),
                    size: 100,
                    selected: false,
                },
                BtFile {
                    path: "c".into(),
                    size: 100,
                    selected: true,
                },
            ],
            100,
        );
        assert_eq!(d.selected_bytes_of_pieces(&[0]), 100);
        assert_eq!(
            d.selected_bytes_of_pieces(&[1]),
            0,
            "未选文件所在 piece 计 0"
        );
        assert_eq!(d.selected_bytes_of_pieces(&[2]), 100);
        assert_eq!(d.selected_bytes_of_pieces(&[0, 1, 2]), 200);
        assert_eq!(d.selected_bytes_of_pieces(&[9]), 0, "越界 piece 不计");
    }

    #[test]
    fn selected_bytes_partial_piece_overlap() {
        let d = data(
            vec![BtFile {
                path: "a".into(),
                size: 250,
                selected: true,
            }],
            100,
        );
        assert_eq!(d.selected_bytes_of_pieces(&[2]), 50, "末块吸收余数");
        assert_eq!(d.selected_bytes_of_pieces(&[0, 1, 2]), 250);
    }

    #[test]
    fn bt_task_data_has_no_progress_field() {
        let d = data(
            vec![BtFile {
                path: "a".into(),
                size: 1,
                selected: true,
            }],
            16,
        );
        let raw = serde_json::to_string(&d).unwrap();
        assert!(
            !raw.contains("done_pieces"),
            "任务定义数据不得携带进度字段（进度唯一归属 .ezr）: {raw}"
        );
    }
}
