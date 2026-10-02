//! sidecar — 断点续传元数据（FR-01-20/21：`<目标文件>.ezr` 伴随同名文件）
//!
//! 内容：原始/最终 URL、文件大小、ETag、Last-Modified、块大小、总块数、
//! 各块状态（含部分下载块的块内字节）、已下载字节、不支持续传标记、校验期望值、
//! 任务元信息（ID / 添加时间 / 保存路径 / 并发数）。
//! **原子写**：先写临时文件再 rename，任意时刻断电不损坏。
//! 任务转「已完成」时删除对应 sidecar（FR-01-20）。
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
// 字节/速度/时间算术在 u64-f64 间转换是下载器领域固有；边界由调用方保证
#![allow(clippy::missing_const_for_fn)] // nursery 误报为主（含 trait impl 场景）
#![allow(clippy::doc_markdown, clippy::doc_lazy_continuation)] // 中文文档中英文术语不强制反引号
#![allow(clippy::float_cmp)] // 速度/时间为 0 的语义判断使用精确比较
#![allow(
    clippy::map_unwrap_or,
    clippy::option_if_let_else,
    clippy::unnested_or_patterns
)]
#![allow(clippy::cognitive_complexity, clippy::too_many_lines)] // 分块计算/状态机逻辑固有复杂度

use serde::{Deserialize, Serialize};

use super::consistency::ServerStamp;
use super::{Checksum, Protocol};

/// sidecar 文件格式版本
pub const SIDECAR_VERSION: u32 = 1;

/// sidecar 元数据（JSON 持久化）
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Sidecar {
    /// 格式版本
    pub version: u32,
    /// 原始 URL
    pub url: String,
    /// 最终 URL（重定向后；一致性比较用）
    pub final_url: Option<String>,
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
    /// 从运行时块表与服务端快照构建
    #[must_use]
    pub fn build(
        url: &str,
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
            url: url.to_string(),
            final_url: stamp.final_url.clone(),
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

    /// 服务端快照（一致性检查输入）
    #[must_use]
    pub fn stamp(&self) -> ServerStamp {
        ServerStamp {
            final_url: self.final_url.clone(),
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
        let d = std::env::temp_dir().join(format!("ezr-side-{}-{}", std::process::id(), unix_ms()));
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
            final_url: Some("http://x/fin.bin".to_string()),
            etag: Some("abc123".to_string()),
            last_modified: Some("Mon, 09 Feb 2026 00:00:00 GMT".to_string()),
            size: Some(2_500_000),
        };
        Sidecar::build(
            "http://x/f.bin",
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
        assert_eq!(s.final_url.as_deref(), Some("http://x/fin.bin"));
        assert_eq!(s.size, Some(2_500_000));
    }
}
