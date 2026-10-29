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
    pub done_pieces: Vec<u32>,
}

pub const BT_SIDECAR_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BtSidecar {
    pub version: u32,
    pub id: u32,
    pub name: String,
    pub save_dir: String,
    pub concurrency: usize,
    pub data: BtTaskData,
}

impl BtSidecar {
    #[must_use]
    pub fn build(
        id: u32,
        name: &str,
        save_dir: &str,
        concurrency: usize,
        data: BtTaskData,
    ) -> BtSidecar {
        BtSidecar {
            version: BT_SIDECAR_VERSION,
            id,
            name: name.to_string(),
            save_dir: save_dir.to_string(),
            concurrency,
            data,
        }
    }

    pub fn save(&self, path: &str) -> std::io::Result<()> {
        super::save_json_atomic(self, path)
    }

    #[must_use]
    pub fn load(path: &str) -> Option<BtSidecar> {
        let text = std::fs::read_to_string(path).ok()?;
        let sc: BtSidecar = serde_json::from_str(&text).ok()?;
        if sc.version != BT_SIDECAR_VERSION {
            return None;
        }
        Some(sc)
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

    fn sample(id: u32) -> BtSidecar {
        BtSidecar::build(
            id,
            "tor",
            "/dl",
            4,
            BtTaskData {
                source: "/tmp/tor.torrent".into(),
                trackers: vec!["udp://t/ann".into()],
                piece_length: 16_384,
                files: vec![BtFile {
                    path: "tor.bin".into(),
                    size: 32_768,
                    selected: true,
                }],
                done_pieces: vec![0, 2, 5],
            },
        )
    }

    #[test]
    fn roundtrip_preserves_everything() {
        let dir = tmp_dir("rt");
        let p = format!("{dir}/tor.ezr");
        let sc = sample(9);
        sc.save(&p).unwrap();
        let loaded = BtSidecar::load(&p).unwrap();
        assert_eq!(loaded, sc, "存取后字段全等");
        assert_eq!(loaded.version, BT_SIDECAR_VERSION);
        assert_eq!(loaded.data.done_pieces, vec![0, 2, 5]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_rejects_unknown_version_and_broken_json() {
        let dir = tmp_dir("ver");
        let p = format!("{dir}/bad.ezr");
        std::fs::write(&p, "{\"version\":99}").unwrap();
        assert!(BtSidecar::load(&p).is_none(), "未知版本拒绝");
        std::fs::write(&p, "not json").unwrap();
        assert!(BtSidecar::load(&p).is_none(), "坏 JSON 拒绝");
        assert!(
            BtSidecar::load(&format!("{dir}/absent.ezr")).is_none(),
            "缺文件 None"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn remove_is_idempotent() {
        let dir = tmp_dir("rm");
        let p = format!("{dir}/gone.ezr");
        sample(1).save(&p).unwrap();
        BtSidecar::remove(&p).unwrap();
        assert!(!std::path::Path::new(&p).exists());
        BtSidecar::remove(&p).unwrap();
        BtSidecar::remove("/nonexistent/x.ezr").unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }
}
