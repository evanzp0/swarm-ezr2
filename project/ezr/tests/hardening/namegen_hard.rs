#![allow(missing_docs)]
#[path = "../../src/model/namegen.rs"]
mod namegen;

use namegen::{exists_on_disk, from_content_disposition, from_url_path, sanitize_name};

#[test]
fn cd_filename_with_slash_rejected() {
    assert_eq!(from_content_disposition("attachment; filename=a/b"), None);
}

#[test]
fn cd_filename_with_nul_rejected() {
    assert_eq!(
        from_content_disposition("attachment; filename=a\u{0}b"),
        None
    );
}

#[test]
fn cd_filename_plain_passes() {
    assert_eq!(
        from_content_disposition("attachment; filename=\"setup.exe\""),
        Some("setup.exe".to_string())
    );
}

#[test]
fn url_path_dot_segment_filtered() {
    assert_eq!(from_url_path("http://h/a/./"), None);
}

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

#[test]
fn sanitize_keeps_space() {
    assert_eq!(sanitize_name("a b"), "a b");
    assert_eq!(sanitize_name("a/b"), "a_b", "真正的分隔符仍需替换（对照）");
}

#[test]
fn exists_on_disk_name_itself() {
    let dir = std::env::temp_dir().join(format!("ezr-hard-ng-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.bin"), b"x").unwrap();
    assert!(exists_on_disk(dir.to_str().unwrap(), "f.bin"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exists_on_disk_downloading_form() {
    let dir = std::env::temp_dir().join(format!("ezr-hard-ngd-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.bin.downloading"), b"x").unwrap();
    assert!(exists_on_disk(dir.to_str().unwrap(), "f.bin"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exists_on_disk_sidecar_form() {
    let dir = std::env::temp_dir().join(format!("ezr-hard-ngs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.bin.ezr"), b"x").unwrap();
    assert!(exists_on_disk(dir.to_str().unwrap(), "f.bin"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exists_on_disk_absent() {
    let dir = std::env::temp_dir().join(format!("ezr-hard-ngn-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert!(!exists_on_disk(dir.to_str().unwrap(), "absent.bin"));
    let _ = std::fs::remove_dir_all(&dir);
}
