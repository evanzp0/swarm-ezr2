#![allow(missing_docs)]
#[path = "../../src/model/chunk/plan.rs"]
mod chunk;

#[path = "../../src/model/config.rs"]
mod config;

use std::path::PathBuf;

use config::{home_dir, Config};

#[test]
fn load_reads_existing_config_file() {
    let path = std::env::temp_dir().join(format!("ezr-hardening-cfg-{}.toml", std::process::id()));
    std::fs::write(&path, "backoff_initial = 0.75\nbackoff_cap = 30.0\n").expect("write fixture");
    let loaded = Config::load(&path);
    let _ = std::fs::remove_file(&path);
    let d = Config::default();
    assert!(
        (loaded.backoff_initial - 0.75).abs() < 1e-9,
        "backoff_initial={}（恒默认变体=8.0）",
        loaded.backoff_initial
    );
    assert!(
        (loaded.backoff_cap - 30.0).abs() < 1e-9,
        "backoff_cap={}（恒默认变体=60.0）",
        loaded.backoff_cap
    );
    assert_ne!(loaded, d, "合法非默认配置被 load 读成了默认");
}

#[test]
fn valid_positive_backoff_values_pass_through() {
    let c = Config::from_toml("backoff_initial = 0.75\nbackoff_cap = 30.0\n");
    assert!(
        (c.backoff_initial - 0.75).abs() < 1e-9,
        "backoff_initial={}（== 0.0 / < 0.0 变体回退 8.0）",
        c.backoff_initial
    );
    assert!(
        (c.backoff_cap - 30.0).abs() < 1e-9,
        "backoff_cap={}（< 0.0 变体回退 60.0）",
        c.backoff_cap
    );
}

#[test]
fn zero_backoff_falls_back_to_default() {
    let c = Config::from_toml("backoff_initial = 0.0\nbackoff_cap = 0.0\n");
    let d = Config::default();
    assert!(
        (c.backoff_initial - d.backoff_initial).abs() < 1e-9 && c.backoff_initial > 0.0,
        "backoff_initial={}（>= 变体放行 0.0）",
        c.backoff_initial
    );
    assert!(
        (c.backoff_cap - d.backoff_cap).abs() < 1e-9 && c.backoff_cap > 0.0,
        "backoff_cap={}（>= 变体放行 0.0）",
        c.backoff_cap
    );
}

#[test]
fn home_dir_env_phases() {
    std::env::set_var("HOME", "/ezr-hardening-fake-home");
    std::env::remove_var("USERPROFILE");
    assert_eq!(
        home_dir(),
        Some(PathBuf::from("/ezr-hardening-fake-home")),
        "恒 Some(\"\") 变体不读 HOME"
    );
    std::env::remove_var("HOME");
    std::env::remove_var("USERPROFILE");
    assert_eq!(home_dir(), None, "恒 Some(\"\") 变体在全缺时非 None");
}
