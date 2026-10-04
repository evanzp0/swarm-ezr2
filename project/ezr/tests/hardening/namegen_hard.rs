//! 加固测试：`namegen`（src/model/namegen.rs 变异幸存体杀灭）。
//! 靶点（放量轮 namegen.rs 幸存 9）：
//! - `52:46` `contains('/') || contains('\\')` → `&&`：单含 `/` 必须拒绝；
//! - `52:69` `contains('\\') || contains('\0')` → `&&`：单含 `\0` 必须拒绝；
//! - `71:34` `!empty && != "."` → `||`：末段 `"."` 必须被过滤；
//! - `103:29` `(c as u32) < 0x20` → `<=`：空格（0x20）必须保留；
//! - `118:18` `i + 2 < len` → `i * 2`：连续两段 %XX 只解码第一段（i=3 处 mutant 条件假）；
//! - `118:22` `i + 2 < len` → `<=`：尾部残缺 `%2` 必须保留原文（mutant 越界 panic 或误解码）；
//! - `163:5` `exists_on_disk → false`；`165:9 / 166:9` `||` → `&&`：`.downloading` /
//!   `.ezr` 形态单独存在即占用。

#![allow(missing_docs)]
#[path = "../../src/model/namegen.rs"]
mod namegen;

use namegen::{exists_on_disk, from_content_disposition, from_url_path, sanitize_name};

/// 靶 `52:46`：仅含 `/`（不含 `\\`）必须拒绝；mutant `&&` 放行。
#[test]
fn cd_filename_with_slash_rejected() {
    assert_eq!(from_content_disposition("attachment; filename=a/b"), None);
}

/// 靶 `52:69`：仅含 `\0`（不含 `/`、`\\`）必须拒绝；mutant `&&` 放行。
#[test]
fn cd_filename_with_nul_rejected() {
    assert_eq!(from_content_disposition("attachment; filename=a\u{0}b"), None);
}

/// 合法名直通（防过度拒绝的对照）。
#[test]
fn cd_filename_plain_passes() {
    assert_eq!(
        from_content_disposition("attachment; filename=\"setup.exe\""),
        Some("setup.exe".to_string())
    );
}

/// 靶 `71:34`：URL 末段恰为 `.` 必须过滤（mutant `||` 因非空而放行）。
#[test]
fn url_path_dot_segment_filtered() {
    assert_eq!(from_url_path("http://h/a/./"), None);
}

/// 合法末段解码直通（118 系列的对照）。
#[test]
fn url_path_percent_decoded() {
    assert_eq!(
        from_url_path("http://h/%41%42"),
        Some("AB".to_string()),
        "连续两段 %XX：第一段在 i=0 解码，第二段在 i=3 解码（mutant i*2 条件在 i=3 为假）"
    );
    assert_eq!(
        from_url_path("http://h/a%2"),
        Some("a%2".to_string()),
        "尾部残缺 %2 保留原文（mutant <= 变体越界）"
    );
}

/// 靶 `103:29`：空格（0x20）不是控制字符，必须保留；mutant `<=` 把空格替换为 `_`。
#[test]
fn sanitize_keeps_space() {
    assert_eq!(sanitize_name("a b"), "a b");
    assert_eq!(sanitize_name("a/b"), "a_b", "真正的分隔符仍需替换（对照）");
}

/// 靶 `163:5`：目标文件本体存在即占用。
#[test]
fn exists_on_disk_name_itself() {
    let dir = std::env::temp_dir().join(format!("ezr-hard-ng-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.bin"), b"x").unwrap();
    assert!(exists_on_disk(dir.to_str().unwrap(), "f.bin"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// 靶 `165:9`：仅 `.downloading` 形态存在即占用（mutant `&&` 需三态同存才判占用）。
#[test]
fn exists_on_disk_downloading_form() {
    let dir = std::env::temp_dir().join(format!("ezr-hard-ngd-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.bin.downloading"), b"x").unwrap();
    assert!(exists_on_disk(dir.to_str().unwrap(), "f.bin"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// 靶 `166:9`：仅 `.ezr` sidecar 存在即占用。
#[test]
fn exists_on_disk_sidecar_form() {
    let dir = std::env::temp_dir().join(format!("ezr-hard-ngs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.bin.ezr"), b"x").unwrap();
    assert!(exists_on_disk(dir.to_str().unwrap(), "f.bin"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// 对照：三者皆无 → 未占用（防恒真）。
#[test]
fn exists_on_disk_absent() {
    let dir = std::env::temp_dir().join(format!("ezr-hard-ngn-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert!(!exists_on_disk(dir.to_str().unwrap(), "absent.bin"));
    let _ = std::fs::remove_dir_all(&dir);
}
