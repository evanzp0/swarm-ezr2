//! sidecar — 断点续传元数据（FR-01-20/21：`<目标文件>.ezr` 伴随同名文件）
//!
//! 内容（v1.28 URL 权威模型改版）：文件大小、ETag、Last-Modified、块大小、总块数、
//! 各块状态（含部分下载块的块内字节）、已下载字节、不支持续传标记、校验期望值、
//! 任务元信息（ID / 添加时间 / 保存路径 / 并发数）。
//! **sidecar 不存储任何 URL**（AIR2 口径：控制文件根本不存 URL；持久化 URL 仅
//! 任务注册表中的原始 URL，FR-01-70 承载；任务与 sidecar 的关联靠文件路径伴随
//! 关系）。旧档（v1.27 及更早产物）的 `url`/`final_url` 键按多余字段反序列化忽略
//! （零迁移，断点仍可续传——一致性改按 FR-01-22 v1.28 三要素判定）。
//! **原子写**：先写临时文件再 rename，任意时刻断电不损坏。
//! 任务转「已完成」时删除对应 sidecar（FR-01-20）。

use serde::{Deserialize, Serialize};

use super::consistency::ServerStamp;
use super::{Checksum, Protocol};

/// sidecar 文件格式版本
pub const SIDECAR_VERSION: u32 = 1;

/// sidecar 元数据（JSON 持久化；v1.28 起不含任何 URL 字段）
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Sidecar {
    /// 格式版本
    pub version: u32,
    /// 文件总大小
    pub size: u64,
    /// ETag
    pub etag: Option<String>,
    /// Last-Modified
    pub last_modified: Option<String>,
    /// 块大小
    pub block_size: u64,
    /// 总块数
    pub block_count: u32,
    /// 各块已写字节（索引 = 块号；值 == 块长表示完成）
    pub blocks: Vec<u64>,
    /// 已下载字节合计
    pub downloaded: u64,
    /// 不支持断点续传标记（单并发整体下载）
    pub non_resumable: bool,
    /// 校验期望值（如有）
    pub expected: Option<ChecksumCompat>,
    /// 任务元信息
    pub task: SidecarTask,
}

/// 校验期望值的序列化形态（显示名 → 存储名，避免 `&'static str` 直接序列化）
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ChecksumCompat {
    /// 算法显示名（CHECKSUM_ALGOS 中名称）
    pub algo: String,
    /// 十六进制校验码（小写）
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

/// 任务元信息（sidecar 内嵌）
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SidecarTask {
    /// 任务 ID
    pub id: u32,
    /// 添加时间（Unix 秒）
    pub added_at: u64,
    /// 保存路径（目录）
    pub save_dir: String,
    /// 并发数
    pub concurrency: usize,
    /// 协议（显示徽标用）
    pub protocol: Protocol,
}

impl Sidecar {
    /// 从运行时块表与服务端快照构建（v1.28：不含任何 URL——任务与 sidecar 的
    /// 关联靠文件路径伴随关系）
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

    /// 服务端快照（一致性检查输入；v1.28 三要素，无 URL）
    #[must_use]
    pub fn stamp(&self) -> ServerStamp {
        ServerStamp {
            etag: self.etag.clone(),
            last_modified: self.last_modified.clone(),
            size: Some(self.size),
        }
    }

    /// 原子写：先写 `<path>.tmp` 再 rename（任意时刻断电不损坏，FR-01-20）
    ///
    /// # Errors
    /// 临时文件写入或 rename 失败时返回 IO 错误。
    pub fn save(&self, path: &str) -> std::io::Result<()> {
        super::save_json_atomic(self, path)
    }

    /// 读取并解析（损坏/版本不符返回 None：按无断点处理，从头下载）
    #[must_use]
    pub fn load(path: &str) -> Option<Sidecar> {
        let text = std::fs::read_to_string(path).ok()?;
        let sc: Sidecar = serde_json::from_str(&text).ok()?;
        if sc.version != SIDECAR_VERSION {
            return None;
        }
        Some(sc)
    }

    /// 删除 sidecar（任务完成/作废时）；文件不存在不算错误
    ///
    /// # Errors
    /// 删除失败（权限等）时返回 IO 错误。
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
        // 进程内自增序号参与命名：同 pid 同毫秒的并发 tmp_dir() 调用
        // 曾撞出同一目录（先完成者 remove_dir_all 令后者 save 报 NotFound）；
        // 序号使每次调用唯一，跨会话 pid 复用 + 毫秒撞库同样消除
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

    /// v1.28/FR-01-20 回归（规格 01-resume-sidecar-01）：sidecar 不再存储任何
    /// URL——旧档（v1.27 及更早产物）的 `url`/`final_url` 键按多余字段反序列化
    /// 忽略（零迁移，断点仍可续传），新序列化不再写回这两个键。
    #[test]
    fn legacy_url_fields_are_ignored_and_not_written_back() {
        let dir = tmp_dir();
        let p = format!("{dir}/legacy.ezr");
        // v1.27 产物形态：含 url + final_url 两键的 sidecar JSON（原样构造，
        // 不经现行结构体——两代字段集差异正是本测试的锚点）
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
        // 重新落盘：新序列化不得写回任何 URL 键（FR-01-20 v1.28「不存储任何 URL」）
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
        // 未知算法名 → 恢复失败（按无校验处理由调用方决定）
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
        // v1.28：快照不含任何 URL 字段（结构层面无 final_url）
        let text = serde_json::to_string(&sc).unwrap();
        assert!(
            !text.contains("final_url"),
            "新序列化不得含 final_url: {text}"
        );
    }
}
