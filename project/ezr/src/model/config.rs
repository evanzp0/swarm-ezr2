//! config — 配置文件（FR-01-71：`~/.ezr/config.toml`，缺省回退、非法值回退；
//! v1.3/D16：`.ezr` 根目录可经环境变量 `EZR_HOME` 重定位）
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

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// HTTP 默认块大小（1 MB，D13；单一来源在 `chunk::HTTP_CHUNK_SIZE`）
pub const DEFAULT_BLOCK_SIZE_HTTP: u64 = super::chunk::HTTP_CHUNK_SIZE;
/// 下载槽位默认值（D13）
pub const DEFAULT_DOWNLOAD_SLOTS: usize = 5;
/// 默认并发（HTTP 4，FR-01-04）
pub const DEFAULT_CONCURRENCY: usize = 4;
/// 自动重试上限默认值（FR-01-41）
pub const DEFAULT_MAX_RETRIES: u32 = 5;

/// 配置（全部键可缺失 → 默认值；非法值 → 回退默认，FR-01-71）
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// 默认下载目录（对话框留空时使用；缺省 = 用户主目录下载目录，FR-01-03）
    pub download_dir: Option<String>,
    /// HTTP 块大小（字节，默认 1 MB；非法 0 → 默认）
    pub block_size_http: u64,
    /// 全局下载槽位（默认 5；0 → 默认）
    pub download_slots: usize,
    /// 全局限速 B/s（0 = 不限）
    pub max_speed: u64,
    /// 自动重试上限（默认 5）
    pub max_retries: u32,
    /// 是否自动重试（默认 true）
    pub auto_retry: bool,
    /// 退避初始秒（默认 8）
    pub backoff_initial: f64,
    /// 退避封顶秒（默认 60）
    pub backoff_cap: f64,
    /// HTTP(S) 代理（仅配置文件；None = 不用代理，FR-01-61）
    pub proxy: Option<String>,
    /// 默认并发数（对话框留空时；1–64 钳制，默认 4）
    pub default_concurrency: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            download_dir: None,
            block_size_http: DEFAULT_BLOCK_SIZE_HTTP,
            download_slots: DEFAULT_DOWNLOAD_SLOTS,
            max_speed: 0,
            max_retries: DEFAULT_MAX_RETRIES,
            auto_retry: true,
            backoff_initial: 8.0,
            backoff_cap: 60.0,
            proxy: None,
            default_concurrency: DEFAULT_CONCURRENCY,
        }
    }
}

/// `config.toml` 的反序列化形态（全部键可选）
#[derive(Clone, Debug, Default, Deserialize)]
struct ConfigRaw {
    download_dir: Option<String>,
    block_size_http: Option<u64>,
    download_slots: Option<usize>,
    max_speed: Option<String>,
    max_retries: Option<u32>,
    auto_retry: Option<bool>,
    backoff_initial: Option<f64>,
    backoff_cap: Option<f64>,
    proxy: Option<String>,
    default_concurrency: Option<usize>,
}

impl Config {
    /// 加载配置：文件可缺失（全默认）；解析失败 = 全部非法 → 默认；
    /// 单键非法值回退默认（见 [`Config::from_toml`]）。
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => Config::from_toml(&text),
            Err(_) => Config::default(),
        }
    }

    /// 从 TOML 文本构建（单键非法回退默认）
    #[must_use]
    pub fn from_toml(text: &str) -> Self {
        let raw: ConfigRaw = toml::from_str(text).unwrap_or_default();
        let d = Config::default();
        Config {
            download_dir: raw.download_dir.filter(|s| !s.trim().is_empty()),
            block_size_http: raw
                .block_size_http
                .filter(|v| *v > 0)
                .unwrap_or(d.block_size_http),
            download_slots: raw
                .download_slots
                .filter(|v| *v > 0)
                .unwrap_or(d.download_slots),
            max_speed: raw.max_speed.as_deref().map_or(0, parse_speed),
            max_retries: raw.max_retries.filter(|v| *v >= 1).unwrap_or(d.max_retries),
            auto_retry: raw.auto_retry.unwrap_or(d.auto_retry),
            backoff_initial: raw
                .backoff_initial
                .filter(|v| *v > 0.0 && v.is_finite())
                .unwrap_or(d.backoff_initial),
            backoff_cap: raw
                .backoff_cap
                .filter(|v| *v > 0.0 && v.is_finite())
                .unwrap_or(d.backoff_cap),
            proxy: raw.proxy.filter(|s| !s.trim().is_empty()),
            default_concurrency: raw
                .default_concurrency
                .map(|v| v.clamp(1, 64))
                .unwrap_or(d.default_concurrency),
        }
    }
}

/// 限速单位后缀表（十进制口径：1 MB = 1_000_000 B/s，与 UI 显示一致）。
/// 匹配顺序即原 else-if 链顺序：mb → kb → b。
const SPEED_UNITS: [(&str, f64); 3] = [("mb", 1_000_000.0), ("kb", 1_000.0), ("b", 1.0)];

/// 限速字符串解析：`"2 MB/s"` / `"500 KB/s"` / `"1048576"`（B/s）。
/// 无法解析 → 0（不限）。十进制口径（1 MB = 1_000_000 B/s，与 UI 显示一致）。
#[must_use]
pub fn parse_speed(s: &str) -> u64 {
    let t = s.trim().to_lowercase();
    let t = t.strip_suffix("/s").unwrap_or(&t).trim();
    if let Ok(v) = t.parse::<u64>() {
        return v;
    }
    let (num_part, mult) = SPEED_UNITS
        .iter()
        .find_map(|(suffix, m)| t.strip_suffix(suffix).map(|n| (n.trim(), *m)))
        .unwrap_or((t, 1.0));
    num_part
        .trim()
        .parse::<f64>()
        .map(|v| (v * mult) as u64)
        .unwrap_or(0)
}

/// 用户主目录（跨平台；无主目录环境返回 None）
#[must_use]
pub fn home_dir() -> Option<PathBuf> {
    // 不引入 dirs 依赖：按平台惯例读 HOME/USERPROFILE（notes/rust.md 的 dirs
    // 建议针对 IPC 路径选型；此处仅两个环境变量键，直读即可）
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// `.ezr` 根目录（v1.3/D16，FR-01-70/71 修订）：环境变量 `EZR_HOME` 非空 → `$EZR_HOME`；
/// 未设置或空串 → 用户主目录下 `.ezr`（缺省口径不变）。单实例锁随 `state/` 迁移，
/// 不同 `EZR_HOME` 的实例互不冲突（隔离沙箱语义）。
#[must_use]
pub fn ezr_dir() -> Option<PathBuf> {
    if let Some(v) = std::env::var_os("EZR_HOME") {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    home_dir().map(|h| h.join(".ezr"))
}

/// 配置文件路径：`<.ezr>/config.toml`（FR-01-71；`.ezr` 根目录见 [`ezr_dir`]）
#[must_use]
pub fn config_path() -> Option<PathBuf> {
    ezr_dir().map(|d| d.join("config.toml"))
}

/// 任务注册表目录：`<.ezr>/state/`（FR-01-70；`.ezr` 根目录见 [`ezr_dir`]）
#[must_use]
pub fn state_dir() -> Option<PathBuf> {
    ezr_dir().map(|d| d.join("state"))
}

/// 用户主目录下的下载目录（FR-01-03：Linux/macOS `~/Downloads`；Windows
/// `%USERPROFILE%\Downloads`——三者路径形态一致）
#[must_use]
pub fn default_download_dir() -> String {
    home_dir()
        .map(|h| h.join("Downloads").to_string_lossy().to_string())
        .unwrap_or_else(|| "Downloads".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_defaults() {
        let c = Config::load(Path::new("/nonexistent/ezr/config.toml"));
        assert_eq!(c, Config::default());
        assert_eq!(c.block_size_http, 1_048_576);
        assert_eq!(c.download_slots, 5);
        assert_eq!(c.max_retries, 5);
        assert!(c.auto_retry);
    }

    #[test]
    fn partial_keys_fallback_defaults() {
        let c = Config::from_toml("download_slots = 3\n");
        assert_eq!(c.download_slots, 3);
        assert_eq!(c.block_size_http, DEFAULT_BLOCK_SIZE_HTTP);
    }

    #[test]
    fn invalid_values_fall_back() {
        let c = Config::from_toml(
            "block_size_http = 0\ndownload_slots = 0\nmax_retries = 0\ndefault_concurrency = 999\n",
        );
        assert_eq!(c.block_size_http, DEFAULT_BLOCK_SIZE_HTTP);
        assert_eq!(c.download_slots, 5);
        assert_eq!(c.max_retries, 5);
        assert_eq!(c.default_concurrency, 64); // 钳制而非回退
    }

    #[test]
    fn garbage_file_falls_back_all() {
        let c = Config::from_toml("not [valid toml ===");
        assert_eq!(c, Config::default());
    }

    #[test]
    fn speed_parsing() {
        assert_eq!(parse_speed("2 MB/s"), 2_000_000);
        assert_eq!(parse_speed("500 KB/s"), 500_000);
        assert_eq!(parse_speed("1048576"), 1_048_576);
        assert_eq!(parse_speed("1.5 MB"), 1_500_000);
        assert_eq!(parse_speed("garbage"), 0);
        assert_eq!(parse_speed("0"), 0);
    }

    #[test]
    fn proxy_key_roundtrip() {
        let c = Config::from_toml("proxy = \"http://127.0.0.1:8118\"\n");
        assert_eq!(c.proxy.as_deref(), Some("http://127.0.0.1:8118"));
        let c = Config::from_toml("proxy = \"\"\n");
        assert!(c.proxy.is_none());
    }

    /// 空白目录与非法数值逐键回退（filter 分支：空白/非正/非有限一律弃用）
    #[test]
    fn empty_dir_and_invalid_numbers_fall_back() {
        let c = Config::from_toml(
            "download_dir = \"   \"\nmax_speed = \"garbage\"\nbackoff_initial = -1.0\nbackoff_cap = 0.0\nproxy = \"  \"\n",
        );
        assert!(c.download_dir.is_none(), "空白目录弃用");
        assert_eq!(c.max_speed, 0, "不可解析限速弃用");
        assert_eq!(c.backoff_initial, Config::default().backoff_initial);
        assert_eq!(c.backoff_cap, Config::default().backoff_cap);
        assert!(c.proxy.is_none(), "空白代理弃用");
    }

    #[test]
    fn default_download_dir_is_home_downloads() {
        let dir = default_download_dir();
        assert!(dir.ends_with("Downloads"), "dir={dir}");
    }

    // ===== EZR_HOME 重定位（v1.3/D16，FR-01-70/71 修订）=====
    // 注：env 读写仅限本组测试触碰 EZR_HOME（其余测试不读该键，无并行竞态）；
    //     no-HOME 场景不入单测（临时改 HOME 会与并行读 HOME 的测试竞态），
    //     由 pty 端到端验证兜底。

    #[test]
    fn ezr_home_env_relocates_config_and_state() {
        let dir = std::env::temp_dir().join(format!("ezr-home-env-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("EZR_HOME", &dir);
        assert_eq!(ezr_dir(), Some(dir.clone()), "EZR_HOME 非空 → 直接采用");
        assert_eq!(config_path(), Some(dir.join("config.toml")));
        assert_eq!(state_dir(), Some(dir.join("state")));
        std::env::remove_var("EZR_HOME");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ezr_home_empty_or_unset_falls_back_to_dot_ezr() {
        let expect_dir = home_dir().map(|h| h.join(".ezr"));
        let expect_cfg = expect_dir.clone().map(|d| d.join("config.toml"));
        let expect_state = expect_dir.clone().map(|d| d.join("state"));
        std::env::set_var("EZR_HOME", "");
        assert_eq!(ezr_dir(), expect_dir, "空串视同未设置 → ~/.ezr");
        assert_eq!(config_path(), expect_cfg, "config_path 随 ezr_dir 重定位");
        assert_eq!(state_dir(), expect_state, "state_dir 随 ezr_dir 重定位");
        std::env::remove_var("EZR_HOME");
        assert_eq!(
            ezr_dir(),
            expect_dir,
            "未设置 → ~/.ezr（FR-01-70/71 缺省口径）"
        );
    }
}
