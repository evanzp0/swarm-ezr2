//! checksum — 完整性校验（FR-01-50/51/D14/D15）
//!
//! 校验算法表沿用 demo `CHECKSUM_ALGOS` 定稿（7 种，期望十六进制位数互不相同）：
//! Adler-32 = 8、MD5 = 32、SHA-1 = 40、SHA-224 = 56、SHA-256 = 64、SHA-384 = 96、
//! SHA-512 = 128。
//!
//! 校验值来源（D3，优先级从高到低）：① 对话框/CLI `-x <算法>=<校验码>` 显式提供；
//! ② 保存目录下伴随文件 `<目标文件>.<算法后缀>`（算法由后缀确定，大小写不敏感；
//! 内容为十六进制摘要或 `hex  文件名` 格式；位数不符视为无效、忽略校验；
//! 多个并存按算法表声明顺序取最先存在者）。
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_wrap)]
// 字节/速度/时间算术在 u64-f64 间转换是下载器领域固有；边界由调用方保证
#![allow(clippy::missing_const_for_fn)] // nursery 误报为主（含 trait impl 场景）
#![allow(clippy::doc_markdown, clippy::doc_lazy_continuation)] // 中文文档中英文术语不强制反引号
#![allow(clippy::float_cmp)] // 速度/时间为 0 的语义判断使用精确比较
#![allow(clippy::map_unwrap_or, clippy::option_if_let_else, clippy::unnested_or_patterns)]
#![allow(clippy::cognitive_complexity, clippy::too_many_lines)] // 分块计算/状态机逻辑固有复杂度


use std::path::Path;

use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha224, Sha256, Sha384, Sha512};

/// 校验算法表（添加对话框下拉顺序即伴随文件择序，D14）：
/// （显示名，期望十六进制长度，文件后缀）
pub const CHECKSUM_ALGOS: [(&str, usize, &str); 7] = [
    ("MD5", 32, "md5"),
    ("SHA-1", 40, "sha1"),
    ("SHA-224", 56, "sha224"),
    ("SHA-256", 64, "sha256"),
    ("SHA-384", 96, "sha384"),
    ("SHA-512", 128, "sha512"),
    ("Adler-32", 8, "adler32"),
];

/// 校验码校验错误
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChecksumError {
    /// 码值含非十六进制字符
    NotHex,
    /// 位数与所选算法不符
    BadLength { expected: usize, got: usize },
}

impl std::fmt::Display for ChecksumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChecksumError::NotHex => write!(f, "校验码只能是十六进制字符（0-9a-f）"),
            ChecksumError::BadLength { expected, got } => {
                write!(f, "校验码需为 {expected} 位十六进制（当前 {got} 位）")
            }
        }
    }
}

/// 按算法表下标校验并规范化校验码（对话框确认/CLI `-x` 用，D15）：
/// 统一小写、必须纯十六进制、位数与算法一致。
///
/// # Errors
/// 非十六进制或位数不符时返回 [`ChecksumError`]（调用方 toast/报错退出）。
pub fn validate_value(algo_idx: usize, raw: &str) -> Result<String, ChecksumError> {
    let (algo, need, _) = CHECKSUM_ALGOS[algo_idx.min(CHECKSUM_ALGOS.len() - 1)];
    let v = raw.trim().to_lowercase();
    if !v.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ChecksumError::NotHex);
    }
    let len = v.chars().count();
    if len != need {
        return Err(ChecksumError::BadLength { expected: need, got: len });
    }
    let _ = algo;
    Ok(v)
}

/// CLI `-x <算法>=<校验码>` 解析（D15）：算法名 ∈ 后缀集且大小写不敏感；
/// 缺少 `=` 前缀、算法无法识别或位数不符 → 启动即报错退出。
///
/// # Errors
/// 三类非法输入分别返回相应消息（字符串含面向用户的中文描述）。
pub fn parse_cli_x(arg: &str) -> Result<(usize, String), String> {
    let Some((algo_raw, value_raw)) = arg.split_once('=') else {
        return Err(format!("-x 参数非法：应为 <算法>=<校验码> 形式（如 sha256=…），得到「{arg}」"));
    };
    let algo_name = algo_raw.trim().to_lowercase();
    let Some(idx) = CHECKSUM_ALGOS.iter().position(|(_, _, suffix)| *suffix == algo_name) else {
        return Err(format!(
            "-x 参数非法：无法识别的校验算法「{algo_raw}」（支持：md5/sha1/sha224/sha256/sha384/sha512/adler32）"
        ));
    };
    validate_value(idx, value_raw)
        .map(|v| (idx, v))
        .map_err(|e| format!("-x 参数非法：{e}"))
}

/// 按显示名找算法下标（大小写不敏感；`Adler-32`/`adler-32` 等均匹配）
#[must_use]
pub fn algo_index_by_name(name: &str) -> Option<usize> {
    let n = name.trim().to_lowercase().replace('-', "");
    CHECKSUM_ALGOS
        .iter()
        .position(|(disp, _, _)| disp.to_lowercase().replace('-', "") == n)
}

/// 计算文件摘要并输出十六进制小写（流式，不整载内存；校验中/伴随文件预置用）
///
/// # Errors
/// 文件不可读或读取失败时返回错误消息。
pub fn digest_file(path: &str, algo_idx: usize) -> Result<String, String> {
    use std::io::Read;
    let f = std::fs::File::open(path).map_err(|e| format!("无法读取文件（{e}）"))?;
    let mut reader = std::io::BufReader::with_capacity(256 * 1024, f);
    let mut buf = vec![0u8; 256 * 1024];
    match algo_idx {
        0 => hash_stream::<Md5, _>(&mut reader, &mut buf),
        1 => hash_stream::<Sha1, _>(&mut reader, &mut buf),
        2 => hash_stream::<Sha224, _>(&mut reader, &mut buf),
        3 => hash_stream::<Sha256, _>(&mut reader, &mut buf),
        4 => hash_stream::<Sha384, _>(&mut reader, &mut buf),
        5 => hash_stream::<Sha512, _>(&mut reader, &mut buf),
        _ => {
            let mut a = adler::Adler32::new();
            loop {
                let n = reader.read(&mut buf).map_err(|e| format!("读取失败（{e}）"))?;
                if n == 0 {
                    break;
                }
                a.write_slice(&buf[..n]);
            }
            Ok(format!("{:08x}", a.checksum()))
        }
    }
}

fn hash_stream<D: Digest, R: std::io::Read>(r: &mut R, buf: &mut [u8]) -> Result<String, String> {
    let mut d = D::new();
    loop {
        let n = r.read(buf).map_err(|e| format!("读取失败（{e}）"))?;
        if n == 0 {
            break;
        }
        d.update(&buf[..n]);
    }
    Ok(d.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// 计算字节摘要（十六进制小写；运行时路径走流式 `digest_file`，
/// 本函数供测试与内存数据校验使用）
#[must_use]
#[allow(dead_code)]
pub fn digest_bytes(data: &[u8], algo_idx: usize) -> String {
    match algo_idx {
        0 => hex(&Md5::digest(data)),
        1 => hex(&Sha1::digest(data)),
        2 => hex(&Sha224::digest(data)),
        3 => hex(&Sha256::digest(data)),
        4 => hex(&Sha384::digest(data)),
        5 => hex(&Sha512::digest(data)),
        // Adler-32：按大端 8 位十六进制
        _ => format!("{:08x}", adler32_slice(data)),
    }
}

#[allow(dead_code)]
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Adler-32 直接对字节切片计算
#[allow(dead_code)]
fn adler32_slice(data: &[u8]) -> u32 {
    adler::adler32_slice(data)
}

/// 伴随文件解析结果
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidecarChecksum {
    /// 算法表下标
    pub algo_idx: usize,
    /// 十六进制摘要（小写）
    pub value: String,
}

/// 伴随文件内容行解析（D14）：`hex` 或 `hex  文件名`（GNU 工具风格，分隔为空白）。
/// 位数与算法不符 → None（视为无效、忽略校验）。
#[must_use]
pub fn parse_companion_content(algo_idx: usize, content: &str) -> Option<SidecarChecksum> {
    let (_, need, _) = CHECKSUM_ALGOS[algo_idx];
    let first = content.lines().find(|l| !l.trim().is_empty())?;
    let token = first.split_whitespace().next()?;
    let v = token.trim().to_lowercase();
    if !v.chars().all(|c| c.is_ascii_hexdigit()) || v.len() != need {
        return None;
    }
    Some(SidecarChecksum { algo_idx, value: v })
}

/// 在保存目录中查找目标文件的校验伴随文件（D14）：
/// 按算法表声明顺序取最先存在且内容有效者；文件名后缀大小写不敏感。
///
/// # Errors
/// 目录不可读不视为错误（返回 None 的伴随情形），仅文件存在但读取失败时返回错误。
pub fn find_companion(save_dir: &str, file_name: &str) -> Option<SidecarChecksum> {
    for (idx, (_, _, suffix)) in CHECKSUM_ALGOS.iter().enumerate() {
        // 大小写不敏感：先试原名后缀（快路径），再扫描目录匹配大小写变体
        let p = Path::new(save_dir).join(format!("{file_name}.{suffix}"));
        if p.is_file() {
            let content = std::fs::read_to_string(&p).ok()?;
            if let Some(sc) = parse_companion_content(idx, &content) {
                return Some(sc);
            }
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(save_dir) {
            let want = format!("{file_name}.").to_lowercase();
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_lowercase();
                if name == format!("{want}{suffix}") && e.path().is_file() {
                    let content = std::fs::read_to_string(e.path()).ok()?;
                    if let Some(sc) = parse_companion_content(idx, &content) {
                        return Some(sc);
                    }
                    break;
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn algo_table_lengths_match_d15() {
        // FR-01-50 算法表：Adler-32=8、MD5=32、SHA-1=40、SHA-224=56、SHA-256=64、SHA-384=96、SHA-512=128
        let expect = [("MD5", 32), ("SHA-1", 40), ("SHA-224", 56), ("SHA-256", 64),
            ("SHA-384", 96), ("SHA-512", 128), ("Adler-32", 8)];
        for (i, (name, len)) in expect.iter().enumerate() {
            assert_eq!(CHECKSUM_ALGOS[i].0, *name);
            assert_eq!(CHECKSUM_ALGOS[i].1, *len);
        }
    }

    #[test]
    fn validate_value_accepts_and_lowercases() {
        assert_eq!(validate_value(3, &"A".repeat(64)).unwrap(), "a".repeat(64));
        assert_eq!(validate_value(6, "024d0127").unwrap(), "024d0127");
    }

    #[test]
    fn validate_value_rejects_bad_input() {
        assert_eq!(validate_value(0, "xyz"), Err(ChecksumError::NotHex));
        assert_eq!(
            validate_value(0, "abcd"),
            Err(ChecksumError::BadLength { expected: 32, got: 4 })
        );
    }

    #[test]
    fn cli_x_requires_algo_prefix() {
        assert!(parse_cli_x("deadbeef").is_err()); // 缺 =
        assert!(parse_cli_x("crc32=deadbeef").is_err()); // 算法无法识别
        assert!(parse_cli_x("md5=deadbeef").is_err()); // 位数不符
        let (idx, v) = parse_cli_x(&format!("SHA256={}", "AbCd".repeat(16))).unwrap();
        assert_eq!(idx, 3);
        assert_eq!(v, "abcd".repeat(16));
        let (_, v) = parse_cli_x("ADLER32=024d0127").unwrap();
        assert_eq!(v, "024d0127");
    }

    #[test]
    fn algo_index_by_name_case_insensitive() {
        assert_eq!(algo_index_by_name("sha256"), Some(3));
        assert_eq!(algo_index_by_name("SHA-256"), Some(3));
        assert_eq!(algo_index_by_name("Adler-32"), Some(6));
        assert_eq!(algo_index_by_name("md5"), Some(0));
        assert_eq!(algo_index_by_name("crc32"), None);
    }

    #[test]
    fn digest_known_vectors() {
        // 空串/abc 标准向量
        assert_eq!(digest_bytes(b"", 0), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(digest_bytes(b"abc", 0), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(digest_bytes(b"abc", 1), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            digest_bytes(b"abc", 3),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(digest_bytes(b"abc", 2), "23097d223405d8228642a477bda255b32aadbce4bda0b3f7e36c9da7");
        assert_eq!(
            digest_bytes(b"abc", 4),
            "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7"
        );
        assert_eq!(
            digest_bytes(b"abc", 5),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
        // Adler-32("abc") = 0x024d0127
        assert_eq!(digest_bytes(b"abc", 6), "024d0127");
    }

    #[test]
    fn companion_content_hex_and_gnu_format() {
        let sc = parse_companion_content(3, &format!("{}  file.bin\n", "a".repeat(64))).unwrap();
        assert_eq!(sc.algo_idx, 3);
        assert_eq!(sc.value, "a".repeat(64));
        let sc = parse_companion_content(6, "024d0127").unwrap();
        assert_eq!(sc.value, "024d0127");
        // 位数不符 → 无效
        assert!(parse_companion_content(0, "abcd").is_none());
        assert!(parse_companion_content(3, &format!("{}  f\n", "a".repeat(63))).is_none());
        // 非十六进制 → 无效
        assert!(parse_companion_content(0, "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz").is_none());
        // 空内容 → 无效
        assert!(parse_companion_content(0, "\n  \n").is_none());
    }

    #[test]
    fn find_companion_prefers_table_order() {
        let dir = std::env::temp_dir().join(format!("ezr-ck-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("f.bin");
        std::fs::write(&file, b"abc").unwrap();
        // 并存 sha256 与 md5：算法表顺序 MD5 在前 → 取 md5
        std::fs::write(dir.join("f.bin.sha256"), "a".repeat(64)).unwrap();
        std::fs::write(dir.join("f.bin.md5"), "900150983cd24fb0d6963f7d28e17f72").unwrap();
        let sc = find_companion(dir.to_str().unwrap(), "f.bin").unwrap();
        assert_eq!(sc.algo_idx, 0);
        // 大写后缀文件（SHA1 大写）也要能找到
        std::fs::write(dir.join("f.bin.SHA1"), "a9993e364706816aba3e25717850c26c9cd0d89d").unwrap();
        std::fs::remove_file(dir.join("f.bin.md5")).unwrap();
        let sc = find_companion(dir.to_str().unwrap(), "f.bin").unwrap();
        assert_eq!(sc.algo_idx, 1);
        // 位数不符的伴随文件 → 忽略校验（删掉其余伴随后无有效者）
        std::fs::write(dir.join("f.bin.sha1"), "short").unwrap();
        std::fs::remove_file(dir.join("f.bin.sha256")).unwrap();
        assert!(find_companion(dir.to_str().unwrap(), "f.bin").is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn digest_file_roundtrip() {
        let dir = std::env::temp_dir().join(format!("ezr-ck2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("data.bin");
        std::fs::write(&p, b"abc").unwrap();
        let h = digest_file(p.to_str().unwrap(), 3).unwrap();
        assert_eq!(h, digest_bytes(b"abc", 3));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn digest_file_missing_is_error() {
        assert!(digest_file("/nonexistent/f.bin", 3).is_err());
    }
}
