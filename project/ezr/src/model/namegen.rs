//! namegen — 目标文件名推导与落盘命名（FR-01-02 / FR-01-03 / FR-01-24 / FR-01-26）
//!
//! 推导优先级：`Content-Disposition: attachment` 的 `filename=` 到 `;` 间的
//! 字符串 → 重定向后最终 URL 路径末段 → 原始 URL 路径末段 → `download-<时间戳>`。
//! 目标文件已存在时自动追加序号 `.1` / `.2` …（不覆盖既有文件；断点自动接续
//! FR-01-26 除外）。

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// 下载中盘上文件扩展名（FR-01-24 命名约定单一来源）：模型方法（Task）、
/// 引擎（supervisor 探测盘上残留）、对话框/CLI（断点接续探测、删除清理）
/// 共用本常量，防手拼漂移（architect v116 收口；join 语义按消费点自留，
/// 见 `sidecar_path_of` 注记）。
pub const DOWNLOADING_EXT: &str = ".downloading";

/// sidecar 元数据文件扩展名（D1 命名约定单一来源，同 [`DOWNLOADING_EXT`] 口径）
pub const SIDECAR_EXT: &str = ".ezr";

/// sidecar 元数据文件路径（(目录, 文件名) 形态散点与 [`crate::model::Task::sidecar_path`]
/// 共用；join 语义 = [`join_path`]，与模型方法同口径。注意：对话框/CLI 的断点
/// 探测点沿用各自既有 `format!("{dir}/{name}")` 拼接（目录尾 '\\' 不归一的
/// 历史口径），仅扩展名收口本常量——join 语义统一属行为变化，不在本轮裁决内）
#[must_use]
pub fn sidecar_path_of(dir: &str, name: &str) -> String {
    format!("{}{SIDECAR_EXT}", join_path(dir, name))
}

/// 路径拼接（处理目录尾分隔符）
#[must_use]
pub fn join_path(dir: &str, name: &str) -> String {
    let dir = dir.trim_end_matches(['/', '\\']);
    if dir.is_empty() {
        return name.to_string();
    }
    format!("{dir}/{name}")
}

/// 从 `Content-Disposition` 提取文件名（`filename=` 到 `;` 间的字符串，FR-01-02）。
/// 兼容 `filename="quoted name"` 与裸 token；失败返回 None。
#[must_use]
pub fn from_content_disposition(cd: &str) -> Option<String> {
    let lower = cd.to_lowercase();
    let pos = lower.find("filename=")?;
    let rest = &cd[pos + "filename=".len()..];
    let token = rest.split(';').next()?.trim();
    let name = token
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(token)
        .trim()
        .to_string();
    // 反斜杠转义与空名防御
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains('\0') {
        return None;
    }
    Some(name)
}

/// 从 URL 路径提取末段（剥离 scheme/host 与 query/fragment；全空路径返回 None）
#[must_use]
pub fn from_url_path(url: &str) -> Option<String> {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    // 剥离 host 部分：第一个 '/' 之后才是路径
    let path = after_scheme.split_once('/').map_or("", |(_, p)| p);
    let seg = path
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('/')
        .find(|s| !s.is_empty())
        .map(percent_decode);
    seg.filter(|s| !s.is_empty() && s != ".")
}

/// 目标文件名推导（三级优先 + 时间戳兜底，FR-01-02）：
/// `cd_filename`（探测响应的 Content-Disposition 文件名，可 None）、
/// `final_url`（重定向后最终 URL，可 None）、`url`（原始 URL）。
#[must_use]
pub fn derive_name(cd_filename: Option<&str>, final_url: Option<&str>, url: &str) -> String {
    if let Some(n) = cd_filename.map(str::trim).filter(|s| !s.is_empty()) {
        return sanitize_name(n);
    }
    if let Some(u) = final_url {
        if let Some(n) = from_url_path(u) {
            return sanitize_name(&n);
        }
    }
    if let Some(n) = from_url_path(url) {
        return sanitize_name(&n);
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("download-{ts}")
}

/// 文件名卫生处理：替换路径分隔符与控制字符，避免越目录写入
#[must_use]
pub fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '\0' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect()
}

/// 解码自下标 `i` 起的完整 `%XX` 三元组；不是 `%`、不足三位（含尾部 `%2`）
/// 或两位非十六进制 → None（调用方保留原文逐字节）。
fn decode_triplet(b: &[u8], i: usize) -> Option<u8> {
    if b[i] != b'%' || i + 2 >= b.len() {
        return None;
    }
    // is_ascii_hexdigit 已前置保证 to_digit 恒成功（原 unwrap_or(0) 等价消除）
    let hi = (b[i + 1] as char).to_digit(16)?;
    let lo = (b[i + 2] as char).to_digit(16)?;
    Some(((hi * 16) + lo) as u8)
}

/// 百分号解码（URL 路径末段显示用；不严格校验，失败保留原文）
#[must_use]
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        // 完整 %XX（两位十六进制）才解码；否则原文逐字节
        if let Some(byte) = decode_triplet(b, i) {
            out.push(byte);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// 目标文件名去重：已存在时自动追加 `.1` / `.2` …（FR-01-02）。
/// `exists` 判定由调用方注入（任务表查重 / 盘上查重均可）；
/// `resume_name`（FR-01-26 断点自动接续）由调用方先行短路，不进本函数。
#[must_use]
pub fn dedupe<F>(base: &str, exists: F) -> String
where
    F: Fn(&str) -> bool,
{
    if !exists(base) {
        return base.to_string();
    }
    for seq in 1..10_000u32 {
        let candidate = format!("{base}.{seq}");
        if !exists(&candidate) {
            return candidate;
        }
    }
    format!(
        "{base}.{ts}",
        ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    )
}

/// 盘上查重（目标文件或其 `.downloading` 形态或 sidecar 任一存在即算占用）
#[must_use]
pub fn exists_on_disk(save_dir: &str, name: &str) -> bool {
    let dir = Path::new(save_dir);
    dir.join(name).exists()
        || dir.join(format!("{name}{DOWNLOADING_EXT}")).exists()
        || dir.join(format!("{name}{SIDECAR_EXT}")).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cd_filename_extraction() {
        assert_eq!(
            from_content_disposition("attachment; filename=data.tar.gz"),
            Some("data.tar.gz".to_string())
        );
        assert_eq!(
            from_content_disposition("attachment; filename=\"my file.bin\"; size=1"),
            Some("my file.bin".to_string())
        );
        assert_eq!(from_content_disposition("inline"), None);
        assert_eq!(from_content_disposition("attachment; filename="), None);
    }

    #[test]
    fn url_last_segment() {
        assert_eq!(
            from_url_path("http://x/a/b/file.zip"),
            Some("file.zip".to_string())
        );
        assert_eq!(
            from_url_path("http://x/a/b/f.zip?q=1"),
            Some("f.zip".to_string())
        );
        assert_eq!(from_url_path("http://x/"), None);
        assert_eq!(
            from_url_path("http://x/a%20b.bin"),
            Some("a b.bin".to_string())
        );
    }

    #[test]
    fn url_partial_escape_tail_kept_verbatim() {
        // 尾部残缺 %2（不足 %XX 三位）不误读，保留原文（percent_decode 边界）
        assert_eq!(
            from_url_path("http://x/a%2.bin"),
            Some("a%2.bin".to_string())
        );
        assert_eq!(from_url_path("http://x/a%.bin"), Some("a%.bin".to_string()));
        assert_eq!(
            from_url_path("http://x/50%25.bin"),
            Some("50%.bin".to_string())
        );
    }

    #[test]
    fn derive_priority_cd_then_final_then_url() {
        assert_eq!(
            derive_name(
                Some("cd-name.bin"),
                Some("http://f/fin.bin"),
                "http://o/orig.bin"
            ),
            "cd-name.bin"
        );
        assert_eq!(
            derive_name(None, Some("http://f/fin.bin"), "http://o/orig.bin"),
            "fin.bin"
        );
        assert_eq!(derive_name(None, None, "http://o/orig.bin"), "orig.bin");
        // 全部无法推导 → download-<时间戳>
        let name = derive_name(None, None, "http://o/");
        assert!(name.starts_with("download-"));
    }

    #[test]
    fn sanitize_blocks_traversal() {
        assert_eq!(sanitize_name("a/b/c"), "a_b_c");
        assert_eq!(sanitize_name("..\\evil"), ".._evil");
    }

    #[test]
    fn dedupe_appends_seq() {
        let taken = ["a.bin", "a.bin.1"];
        let f = |n: &str| taken.contains(&n);
        assert_eq!(dedupe("a.bin", f), "a.bin.2");
        assert_eq!(dedupe("b.bin", f), "b.bin");
    }

    #[test]
    fn join_path_trims_trailing_slash() {
        assert_eq!(join_path("/dl/", "f"), "/dl/f");
        assert_eq!(join_path("/dl", "f"), "/dl/f");
        assert_eq!(join_path("", "f"), "f");
    }
}
