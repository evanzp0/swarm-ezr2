use base64::Engine as _;
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
    #[serde(default)]
    pub upload_paused: bool,
    #[serde(default)]
    pub sequential: bool,
}

fn push_ancestor_dirs(
    abs: &std::path::Path,
    base: &std::path::Path,
    dirs: &mut Vec<std::path::PathBuf>,
) {
    let mut parent = abs.parent();
    while let Some(p) = parent {
        let pc: std::path::PathBuf = p.components().collect();
        if pc == base {
            break;
        }
        dirs.push(pc);
        parent = p.parent();
    }
}

pub fn delete_deselected_files(save_dir: &str, old: &[BtFile], new: &[BtFile]) -> usize {
    let base: std::path::PathBuf = std::path::Path::new(save_dir).components().collect();
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    let mut deleted = 0usize;
    for f in new.iter().filter(|f| !f.selected) {
        let was = old.iter().any(|o| o.path == f.path && o.selected);
        if !was {
            continue;
        }
        let abs = base.join(&f.path);
        if std::fs::remove_file(&abs).is_ok() {
            deleted += 1;
            push_ancestor_dirs(&abs, base.as_path(), &mut dirs);
        }
    }
    dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    dirs.dedup();
    for d in dirs {
        let _ = std::fs::remove_dir(&d);
    }
    deleted
}

impl BtTaskData {
    #[must_use]
    fn span_sum_over_selected_files(
        files: &[BtFile],
        offsets: &[u64],
        start: u64,
        end: u64,
    ) -> u64 {
        let mut sum = 0u64;
        for (i, f) in files.iter().enumerate() {
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
        sum
    }

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
            sum += Self::span_sum_over_selected_files(&self.files, &offsets, start, end);
        }
        sum
    }
}

pub const BT_SIDECAR_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BtSidecar {
    pub version: u32,
    pub done_pieces: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partial_pieces: Vec<BtPartialPiece>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BtPartialPiece {
    pub piece: u32,
    pub blocks: Vec<BtPartialBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BtPartialBlock {
    pub begin: u32,
    #[serde(default)]
    data_b64: String,
}

impl BtPartialBlock {
    #[must_use]
    pub fn encode_block(begin: u32, data: &[u8]) -> BtPartialBlock {
        BtPartialBlock {
            begin,
            data_b64: base64::engine::general_purpose::STANDARD.encode(data),
        }
    }

    #[must_use]
    pub fn decode_data(&self) -> Option<Vec<u8>> {
        base64::engine::general_purpose::STANDARD
            .decode(self.data_b64.as_bytes())
            .ok()
    }
}

impl BtSidecar {
    #[must_use]
    pub fn build(pieces: impl IntoIterator<Item = u32>) -> BtSidecar {
        BtSidecar::build_full(pieces, Vec::new())
    }

    #[must_use]
    pub fn build_full(
        pieces: impl IntoIterator<Item = u32>,
        partial: Vec<BtPartialPiece>,
    ) -> BtSidecar {
        let mut done: Vec<u32> = pieces.into_iter().collect();
        done.sort_unstable();
        done.dedup();
        let mut partial = partial;
        partial.sort_by_key(|p| p.piece);
        for p in &mut partial {
            p.blocks.sort_by_key(|b| b.begin);
            p.blocks.dedup_by_key(|b| b.begin);
        }
        partial.retain(|p| !p.blocks.is_empty());
        BtSidecar {
            version: BT_SIDECAR_VERSION,
            done_pieces: done,
            partial_pieces: partial,
        }
    }

    pub fn save(&self, path: &str) -> std::io::Result<()> {
        super::save_json_atomic(self, path)
    }

    #[must_use]
    pub fn load_full(path: &str) -> Option<BtSidecar> {
        let text = std::fs::read_to_string(path).ok()?;
        let sc: BtSidecar = serde_json::from_str(&text).ok()?;
        // 版本 1 为当前唯一版本；2 为内部开发期短暂出现过的同形版本号
        // （bug6 轮回退，遗留 v2 文件与 v1 字节同形），同形放行以还原
        // 遗留任务进度；其余版本/形状一律拒绝（v1 全量档兼容已删除）。
        if sc.version != BT_SIDECAR_VERSION && sc.version != 2 {
            return None;
        }
        Some(sc)
    }

    #[must_use]
    pub fn load_done(path: &str) -> Option<Vec<u32>> {
        Self::load_full(path).map(|sc| sc.done_pieces)
    }

    pub fn remove(path: &str) -> std::io::Result<()> {
        super::remove_missing_ok(path)
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

    fn partial_block(begin: u32, byte: u8, len: usize) -> BtPartialBlock {
        BtPartialBlock::encode_block(begin, &vec![byte; len])
    }

    #[test]
    fn partial_blocks_roundtrip_base64_and_ordering() {
        let dir = tmp_dir("partial");
        let p = format!("{dir}/tor.ezr");
        let sc = BtSidecar::build_full(
            [1],
            vec![
                BtPartialPiece {
                    piece: 7,
                    blocks: vec![partial_block(16384, 0xBB, 16), partial_block(0, 0xAA, 16)],
                },
                BtPartialPiece {
                    piece: 3,
                    blocks: vec![partial_block(0, 0xCC, 8)],
                },
            ],
        );
        sc.save(&p).unwrap();
        let loaded = BtSidecar::load_full(&p).unwrap();
        assert_eq!(loaded.done_pieces, vec![1]);
        assert_eq!(
            loaded
                .partial_pieces
                .iter()
                .map(|x| x.piece)
                .collect::<Vec<_>>(),
            vec![3, 7],
            "partial 按 piece 序归一"
        );
        let p7 = &loaded.partial_pieces[1];
        assert_eq!(
            p7.blocks.iter().map(|b| b.begin).collect::<Vec<_>>(),
            vec![0, 16384],
            "块按 begin 升序归一"
        );
        assert_eq!(p7.blocks[0].decode_data().unwrap(), vec![0xAA; 16]);
        assert_eq!(p7.blocks[1].decode_data().unwrap(), vec![0xBB; 16]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn legacy_sidecar_without_partial_loads_empty_and_bad_b64_decodes_none() {
        let dir = tmp_dir("legacy");
        let p = format!("{dir}/old.ezr");
        std::fs::write(&p, "{\"version\":1,\"done_pieces\":[0,2]}").unwrap();
        let sc = BtSidecar::load_full(&p).unwrap();
        assert!(sc.partial_pieces.is_empty(), "旧格式 partial 缺省空");
        assert_eq!(sc.done_pieces, vec![0, 2]);

        let p2 = format!("{dir}/bad.ezr");
        std::fs::write(
            &p2,
            "{\"version\":1,\"done_pieces\":[],\"partial_pieces\":[{\"piece\":0,\"blocks\":[{\"begin\":0,\"data_b64\":\"!!!not-b64!!!\"}]}]}",
        )
        .unwrap();
        let sc2 = BtSidecar::load_full(&p2).unwrap();
        assert!(
            sc2.partial_pieces[0].blocks[0].decode_data().is_none(),
            "坏 base64 由消费侧逐块丢弃，不拖垮整档"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    fn data(files: Vec<BtFile>, piece_length: u32) -> BtTaskData {
        BtTaskData {
            source: "/tmp/tor.torrent".into(),
            trackers: vec!["udp://t/ann".into()],
            piece_length,
            files,
            upload_paused: false,
            sequential: false,
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

    fn bf(path: &str, size: u64, selected: bool) -> BtFile {
        BtFile {
            path: path.into(),
            size,
            selected,
        }
    }

    #[test]
    fn delete_deselected_removes_files_and_empty_dirs_bottom_up() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bt-deldirs");
        std::fs::create_dir_all(dir.join("tor/sub/deep")).unwrap();
        std::fs::create_dir_all(dir.join("tor/keep")).unwrap();
        std::fs::write(dir.join("tor/sub/a.txt"), b"aaaa").unwrap();
        std::fs::write(dir.join("tor/sub/deep/b.bin"), b"bbbbbb").unwrap();
        std::fs::write(dir.join("tor/keep/c.txt"), b"cc").unwrap();
        let old = vec![
            bf("tor/sub/a.txt", 4, true),
            bf("tor/sub/deep/b.bin", 6, true),
            bf("tor/keep/c.txt", 2, true),
        ];
        let new = vec![
            bf("tor/sub/a.txt", 4, true),
            bf("tor/sub/deep/b.bin", 6, false),
            bf("tor/keep/c.txt", 2, true),
        ];
        let n = super::delete_deselected_files(dir.to_str().unwrap(), &old, &new);
        assert_eq!(n, 1, "只删取消选中的一个文件");
        assert!(!dir.join("tor/sub/deep/b.bin").exists(), "落盘数据已删除");
        assert!(
            !dir.join("tor/sub/deep").exists(),
            "变空目录自底向上整树清理（深）"
        );
        assert!(
            dir.join("tor/sub").exists(),
            "仍有选中文件 a.txt 的目录保留"
        );
        assert!(dir.join("tor/sub/a.txt").exists(), "保留文件不动");
        assert!(dir.join("tor").exists(), "非空祖先目录保留");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_deselected_missing_skipped_and_io_failure_nonblocking() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bt-delskip");
        std::fs::create_dir_all(dir.join("tor")).unwrap();
        std::fs::write(dir.join("tor/plain"), b"x").unwrap();
        std::fs::write(dir.join("tor/keep.txt"), b"k").unwrap();
        let old = vec![
            bf("tor/never-downloaded.bin", 4, true),
            bf("tor/plain", 1, true),
            bf("tor/plain/enotdir-child", 1, true),
            bf("tor/keep.txt", 1, true),
        ];
        let new = vec![
            bf("tor/never-downloaded.bin", 4, false),
            bf("tor/plain", 1, false),
            bf("tor/plain/enotdir-child", 1, false),
            bf("tor/keep.txt", 1, true),
        ];
        let n = super::delete_deselected_files(dir.to_str().unwrap(), &old, &new);
        assert_eq!(n, 1, "目标不存在静默跳过（5A），可删的照常删除");
        assert!(!dir.join("tor/plain").exists(), "普通文件删除生效");
        assert!(dir.join("tor/keep.txt").exists(), "保留文件不动");
        assert!(dir.join("tor").exists(), "IO 失败不阻塞不扩散");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_deselected_only_touches_previously_selected() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-bt-delwas");
        std::fs::create_dir_all(dir.join("tor")).unwrap();
        std::fs::write(dir.join("tor/x.bin"), b"x").unwrap();
        let old = vec![bf("tor/x.bin", 1, false)];
        let new = vec![bf("tor/x.bin", 1, false)];
        let n = super::delete_deselected_files(dir.to_str().unwrap(), &old, &new);
        assert_eq!(n, 0, "此前未选中的文件无落盘数据，不算取消选中");
        assert!(dir.join("tor/x.bin").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn upload_paused_field_serde_default_roundtrip() {
        let legacy = serde_json::from_str::<BtTaskData>(
            r#"{"source":"s","trackers":[],"piece_length":16,"files":[]}"#,
        )
        .expect("旧档无 upload_paused 键按 false 加载");
        assert!(!legacy.upload_paused);
        let d = BtTaskData {
            upload_paused: true,
            ..BtTaskData::default()
        };
        let raw = serde_json::to_string(&d).unwrap();
        let back: BtTaskData = serde_json::from_str(&raw).unwrap();
        assert!(back.upload_paused, "标志随序列化往返保留");
    }
}
