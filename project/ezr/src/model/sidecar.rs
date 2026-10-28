use serde::{Deserialize, Serialize};

use super::consistency::ServerStamp;
use super::{Checksum, Protocol};

pub const SIDECAR_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Sidecar {
    pub version: u32,
    pub size: u64,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub block_size: u64,
    pub block_count: u32,
    pub blocks: Vec<u64>,
    pub downloaded: u64,
    pub non_resumable: bool,
    pub expected: Option<ChecksumCompat>,
    pub task: SidecarTask,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ChecksumCompat {
    pub algo: String,
    pub value: String,
}

impl From<&Checksum> for ChecksumCompat {
    fn from(c: &Checksum) -> Self {
        ChecksumCompat {
            algo: c.algo.to_string(),
            value: c.value.clone(),
        }
    }
}

impl TryFrom<&ChecksumCompat> for Checksum {
    type Error = ();

    fn try_from(c: &ChecksumCompat) -> Result<Self, ()> {
        let idx = super::checksum::algo_index_by_name(&c.algo).ok_or(())?;
        Ok(Checksum {
            algo: super::checksum::CHECKSUM_ALGOS[idx].0,
            value: c.value.clone(),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SidecarTask {
    pub id: u32,
    pub added_at: u64,
    pub save_dir: String,
    pub concurrency: usize,
    pub protocol: Protocol,
}

impl Sidecar {
    #[must_use]
    pub fn build(
        stamp: &ServerStamp,
        size: u64,
        block_size: u64,
        blocks: &[u64],
        non_resumable: bool,
        expected: Option<&Checksum>,
        task: SidecarTask,
    ) -> Sidecar {
        Sidecar {
            version: SIDECAR_VERSION,
            size,
            etag: stamp.etag.clone(),
            last_modified: stamp.last_modified.clone(),
            block_size,
            block_count: blocks.len() as u32,
            blocks: blocks.to_vec(),
            downloaded: blocks
                .iter()
                .zip(0..)
                .map(|(w, i)| {
                    super::chunk::block_range(size, block_size, i as u32)
                        .map_or(0, |(s, e)| (*w).min(e - s))
                })
                .sum(),
            non_resumable,
            expected: expected.map(ChecksumCompat::from),
            task,
        }
    }

    #[must_use]
    pub fn stamp(&self) -> ServerStamp {
        ServerStamp {
            etag: self.etag.clone(),
            last_modified: self.last_modified.clone(),
            size: Some(self.size),
        }
    }

    pub fn save(&self, path: &str) -> std::io::Result<()> {
        super::save_json_atomic(self, path)
    }

    #[must_use]
    pub fn load(path: &str) -> Option<Sidecar> {
        let text = std::fs::read_to_string(path).ok()?;
        let sc: Sidecar = serde_json::from_str(&text).ok()?;
        if sc.version != SIDECAR_VERSION {
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
    use std::path::Path;

    use super::*;
    use crate::model::Protocol;

    fn tmp_dir() -> String {
        use std::sync::atomic::{AtomicU32, Ordering};
        static SEQ: AtomicU32 = AtomicU32::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!(
            "ezr-side-{}-{}-{}",
            std::process::id(),
            seq,
            unix_ms()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d.to_string_lossy().to_string()
    }

    fn unix_ms() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    }

    fn sample() -> Sidecar {
        let stamp = ServerStamp {
            etag: Some("abc123".to_string()),
            last_modified: Some("Mon, 09 Feb 2026 00:00:00 GMT".to_string()),
            size: Some(2_500_000),
        };
        Sidecar::build(
            &stamp,
            2_500_000,
            1_000_000,
            &[1_000_000, 400_000, 0],
            false,
            Some(&Checksum {
                algo: "SHA-256",
                value: "ab".repeat(32),
            }),
            SidecarTask {
                id: 7,
                added_at: 1_700_000_000,
                save_dir: "/dl".to_string(),
                concurrency: 4,
                protocol: Protocol::Http,
            },
        )
    }

    #[test]
    fn roundtrip_preserves_everything() {
        let sc = sample();
        let dir = tmp_dir();
        let p = format!("{dir}/f.bin.ezr");
        sc.save(&p).unwrap();
        let loaded = Sidecar::load(&p).unwrap();
        assert_eq!(sc, loaded);
        assert_eq!(loaded.downloaded, 1_400_000);
        assert_eq!(loaded.block_count, 3);
        assert_eq!(loaded.task.id, 7);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn atomic_write_leaves_no_tmp() {
        let sc = sample();
        let dir = tmp_dir();
        let p = format!("{dir}/f.bin.ezr");
        sc.save(&p).unwrap();
        assert!(!Path::new(&format!("{p}.tmp")).exists());
        assert!(Path::new(&p).exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn corrupt_file_loads_none() {
        let dir = tmp_dir();
        let p = format!("{dir}/broken.ezr");
        std::fs::write(&p, "{not json").unwrap();
        assert!(Sidecar::load(&p).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn wrong_version_loads_none() {
        let dir = tmp_dir();
        let p = format!("{dir}/v.ezr");
        let mut sc = sample();
        sc.version = 99;
        sc.save(&p).unwrap();
        assert!(Sidecar::load(&p).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn remove_missing_is_ok() {
        assert!(Sidecar::remove("/nonexistent/x.ezr").is_ok());
    }

    #[test]
    fn legacy_url_fields_are_ignored_and_not_written_back() {
        let dir = tmp_dir();
        let p = format!("{dir}/legacy.ezr");
        let legacy = serde_json::json!({
            "version": 1,
            "url": "http://old.example/f.bin",
            "final_url": "http://old.example/fin.bin",
            "size": 2_500_000,
            "etag": "abc123",
            "last_modified": "Mon, 09 Feb 2026 00:00:00 GMT",
            "block_size": 1_000_000,
            "block_count": 3,
            "blocks": [1_000_000, 400_000, 0],
            "downloaded": 1_400_000,
            "non_resumable": false,
            "expected": null,
            "task": {
                "id": 7,
                "added_at": 1_700_000_000,
                "save_dir": "/dl",
                "concurrency": 4,
                "protocol": "Http"
            }
        });
        std::fs::write(&p, legacy.to_string()).unwrap();
        let sc = Sidecar::load(&p).expect("旧档含 url/final_url 键仍可加载（零迁移）");
        assert_eq!(sc.size, 2_500_000, "断点数据随旧档保留");
        assert_eq!(sc.downloaded, 1_400_000);
        assert_eq!(sc.block_count, 3);
        assert_eq!(sc.blocks, vec![1_000_000, 400_000, 0]);
        let p2 = format!("{dir}/rewritten.ezr");
        sc.save(&p2).unwrap();
        let text = std::fs::read_to_string(&p2).unwrap();
        assert!(!text.contains("\"url\""), "新序列化不得含 url 键: {text}");
        assert!(
            !text.contains("final_url"),
            "新序列化不得含 final_url 键: {text}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn checksum_compat_roundtrip() {
        let orig = Checksum {
            algo: "Adler-32",
            value: "89d81b".to_string(),
        };
        let compat: ChecksumCompat = ChecksumCompat::from(&orig);
        let back = Checksum::try_from(&compat).unwrap();
        assert_eq!(back.algo, "Adler-32");
        assert_eq!(back.value, "89d81b");
        let bad = ChecksumCompat {
            algo: "CRC32".to_string(),
            value: "00".to_string(),
        };
        assert!(Checksum::try_from(&bad).is_err());
    }

    #[test]
    fn stamp_roundtrip() {
        let sc = sample();
        let s = sc.stamp();
        assert_eq!(s.size, Some(2_500_000));
        assert_eq!(s.etag.as_deref(), Some("abc123"));
        let text = serde_json::to_string(&sc).unwrap();
        assert!(
            !text.contains("final_url"),
            "新序列化不得含 final_url: {text}"
        );
    }

    #[test]
    fn remove_missing_sidecar_is_ok() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-sc-remove");
        let p = dir.join("absent.bin.ezr");
        assert!(Sidecar::remove(p.to_str().unwrap()).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
