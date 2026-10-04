//! 加固测试：`Config`（src/model/config.rs 变异幸存体杀灭）。
//! 靶点（校准轮 12 幸存中的 5 个）：
//! - `94:9 Config::load` → 恒返回默认——真实文件读取路径无观测；
//! - `120:32 backoff_initial` 过滤 `> 0.0` → `== 0.0` / `>= 0.0`；
//! - `124:32 backoff_cap` 过滤 `> 0.0` → `< 0.0`；
//! - `164:5 home_dir` → 恒返回 `Some(PathBuf::new())`——env 读取路径无观测。
//!
//! 布局：config.rs 引用 `super::chunk::HTTP_CHUNK_SIZE`，以 plan.rs（自包含）
//! 顶层的 `chunk` 模块名挂载满足路径解析；serde/toml 走包依赖（测试目标可见）。
//! env 相位（HOME/USERPROFILE）合并为单测试顺序相位（engineering.md：同一 env 键
//! 只归一个测试触碰）。

#![allow(missing_docs)]
#[path = "../../src/model/chunk/plan.rs"]
mod chunk;

#[path = "../../src/model/config.rs"]
mod config;

use config::{home_dir, Config};
use std::path::PathBuf;

/// 靶 `94:9`：`load` 必须真实读取文件内容（mutant 恒默认 → backoff_initial=8.0）。
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

/// 靶 `120:32 / 124:32 > → ==` 与 `> → <`：合法正值必须直通（mutant 过滤成默认）。
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

/// 靶 `120:32 > → >=`：0.0 非法（回退默认 8.0/60.0），`>= 0.0` 变体会放行 0.0。
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

/// 靶 `164:5`：home_dir 必须读 env（mutant 恒 Some("")）。
/// 相位 1：HOME 设置 → 读 HOME；相位 2：HOME/USERPROFILE 全缺 → None。
#[test]
fn home_dir_env_phases() {
    // 相位 1：HOME 设置（USERPROFILE 移除避免 Windows 键干扰）
    std::env::set_var("HOME", "/ezr-hardening-fake-home");
    std::env::remove_var("USERPROFILE");
    assert_eq!(
        home_dir(),
        Some(PathBuf::from("/ezr-hardening-fake-home")),
        "恒 Some(\"\") 变体不读 HOME"
    );
    // 相位 2：全部缺失 → None（mutant 仍 Some("")）
    std::env::remove_var("HOME");
    std::env::remove_var("USERPROFILE");
    assert_eq!(home_dir(), None, "恒 Some(\"\") 变体在全缺时非 None");
}
