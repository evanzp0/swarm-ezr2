use std::path::{Path, PathBuf};

use serde::Deserialize;

pub const DEFAULT_BLOCK_SIZE_HTTP: u64 = super::chunk::HTTP_CHUNK_SIZE;
pub const DEFAULT_DOWNLOAD_SLOTS: usize = 5;
pub const DEFAULT_HTTP_CONCURRENCY: usize = 4;
pub const DEFAULT_MAX_RETRIES: u32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProxyKind {
    Http,
    Https,
    Socks5,
}

impl ProxyKind {
    #[must_use]
    pub fn from_label(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "http" => Some(Self::Http),
            "https" => Some(Self::Https),
            "socks5" => Some(Self::Socks5),
            _ => None,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        self.scheme()
    }

    #[must_use]
    pub fn scheme(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
            Self::Socks5 => "socks5",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyConfig {
    pub name: String,
    pub kind: ProxyKind,
    pub ip: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl ProxyConfig {
    #[must_use]
    pub fn endpoint_url(&self) -> String {
        if self.ip.contains(':') {
            format!("{}://[{}]:{}", self.kind.scheme(), self.ip, self.port)
        } else {
            format!("{}://{}:{}", self.kind.scheme(), self.ip, self.port)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ProxyChoice {
    #[default]
    Direct,
    Named(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyEndpoint {
    pub url: String,
    pub kind: ProxyKind,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub download_dir: Option<String>,
    pub block_size_http: u64,
    pub download_slots: usize,
    pub max_speed: u64,
    pub max_retries: u32,
    pub auto_retry: bool,
    pub backoff_initial: f64,
    pub backoff_cap: f64,
    pub proxies: Vec<ProxyConfig>,
    pub http_concurrency: usize,
    pub http_trackers: Vec<String>,
    pub udp_trackers: Vec<String>,
    pub warnings: Vec<String>,
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
            proxies: Vec::new(),
            http_concurrency: DEFAULT_HTTP_CONCURRENCY,
            http_trackers: Vec::new(),
            udp_trackers: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
struct ProxyRaw {
    name: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    ip: Option<String>,
    port: Option<toml::Value>,
    username: Option<String>,
    password: Option<String>,
}

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
    proxies: Option<Vec<ProxyRaw>>,
    http_concurrency: Option<usize>,
    http_trackers: Option<Vec<String>>,
    udp_trackers: Option<Vec<String>>,
}

impl Config {
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => Config::from_toml(&text),
            Err(_) => Config::default(),
        }
    }

    #[must_use]
    pub fn from_toml(text: &str) -> Self {
        let raw: ConfigRaw = toml::from_str(text).unwrap_or_default();
        let d = Config::default();
        let mut warnings = Vec::new();
        let mut proxies = Vec::new();
        for p in raw.proxies.into_iter().flatten() {
            let name = p.name.unwrap_or_default().trim().to_string();
            if name.is_empty() {
                warnings.push("已忽略无效代理条目：name 为空".to_string());
                continue;
            }
            if proxies.iter().any(|x: &ProxyConfig| x.name == name) {
                warnings.push(format!("已忽略重复代理条目：{name}（重名）"));
                continue;
            }
            let type_raw = p.kind.as_deref().map(str::trim).filter(|s| !s.is_empty());
            let Some(kind) = type_raw.and_then(ProxyKind::from_label) else {
                let why = if type_raw.is_some() {
                    "未知"
                } else {
                    "缺失"
                };
                warnings.push(format!(
                    "已忽略无效代理条目「{name}」：type {why}（应为 http / https / socks5 三值之一）"
                ));
                continue;
            };
            let ip = p.ip.unwrap_or_default().trim().to_string();
            if ip.is_empty() {
                warnings.push(format!(
                    "已忽略无效代理条目「{name}」：ip 缺失或空白（应为 IP 字面量或主机名）"
                ));
                continue;
            }
            let port = p.port.and_then(|v| match v {
                toml::Value::Integer(n) => u16::try_from(n).ok().filter(|u| *u >= 1),
                _ => None,
            });
            let Some(port) = port else {
                warnings.push(format!(
                    "已忽略无效代理条目「{name}」：port 须为 1..=65535 内的整数（缺失、非整数或越界均作废）"
                ));
                continue;
            };
            let username = p
                .username
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let password = p
                .password
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            match kind {
                ProxyKind::Socks5 if username.is_none() || password.is_none() => {
                    warnings.push(format!(
                        "已忽略无效代理条目「{name}」：socks5 型代理 username 与 password 必填（不支持匿名 socks5），缺任一即作废"
                    ));
                    continue;
                }
                ProxyKind::Http | ProxyKind::Https if username.is_some() != password.is_some() => {
                    warnings.push(format!(
                        "已忽略无效代理条目「{name}」：{} 型代理 username 与 password 需成对配置（都缺省为匿名代理），只填其一即作废",
                        kind.label()
                    ));
                    continue;
                }
                _ => {}
            }
            proxies.push(ProxyConfig {
                name,
                kind,
                ip,
                port,
                username,
                password,
            });
        }
        Config {
            download_dir: raw.download_dir.filter(|s| !s.trim().is_empty()),
            block_size_http: raw
                .block_size_http
                .filter(|v| *v > 0)
                .map(|v| v.max(super::chunk::MIN_HTTP_BLOCK_SIZE))
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
            proxies,
            http_concurrency: raw
                .http_concurrency
                .map(|v| v.clamp(1, 64))
                .unwrap_or(d.http_concurrency),
            http_trackers: sanitize_trackers(
                raw.http_trackers.unwrap_or_default(),
                &["http://", "https://"],
                "http_trackers",
                &mut warnings,
            ),
            udp_trackers: sanitize_trackers(
                raw.udp_trackers.unwrap_or_default(),
                &["udp://"],
                "udp_trackers",
                &mut warnings,
            ),
            warnings,
        }
    }

    #[must_use]
    pub fn resolve_proxy(&self, choice: &ProxyChoice) -> (Option<ProxyEndpoint>, bool) {
        match choice {
            ProxyChoice::Direct => (None, false),
            ProxyChoice::Named(name) => match self.proxies.iter().find(|p| &p.name == name) {
                Some(p) => (
                    Some(ProxyEndpoint {
                        url: p.endpoint_url(),
                        kind: p.kind,
                        username: p.username.clone(),
                        password: p.password.clone(),
                    }),
                    false,
                ),
                None => (None, true),
            },
        }
    }
}

const SPEED_UNITS: [(&str, f64); 3] = [("mb", 1_000_000.0), ("kb", 1_000.0), ("b", 1.0)];

fn sanitize_trackers(
    entries: Vec<String>,
    schemes: &[&str],
    key: &str,
    warnings: &mut Vec<String>,
) -> Vec<String> {
    let mut out = Vec::new();
    for e in entries {
        let t = e.trim();
        if t.is_empty() {
            continue;
        }
        if schemes.iter().any(|p| t.starts_with(p)) {
            out.push(t.to_string());
        } else {
            warnings.push(format!("已忽略无效 tracker 条目（{key}）：{t}"));
        }
    }
    out
}

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

#[must_use]
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

#[must_use]
pub fn ezr_dir() -> Option<PathBuf> {
    if let Some(v) = std::env::var_os("EZR_HOME") {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    home_dir().map(|h| h.join(".ezr"))
}

#[must_use]
pub fn config_path() -> Option<PathBuf> {
    ezr_dir().map(|d| d.join("config.toml"))
}

#[must_use]
pub fn state_dir() -> Option<PathBuf> {
    ezr_dir().map(|d| d.join("state"))
}

#[must_use]
pub fn hosts_file_path() -> PathBuf {
    #[cfg(windows)]
    let path = {
        std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join(r"System32\drivers\etc\hosts")
    };
    #[cfg(not(windows))]
    let path = PathBuf::from("/etc/hosts");
    path
}

#[must_use]
pub fn default_download_dir() -> String {
    home_dir()
        .map(|h| h.join("Downloads").to_string_lossy().to_string())
        .unwrap_or_else(|| "Downloads".to_string())
}

const AUTHORIZED_TEMPLATE: &str = r#"# EZR Downloader 配置模板

# download_dir：默认保存目录（添加对话框目录留空时使用）
# 取值范围：任意目录路径；留空或缺失 = 用户主目录下的下载目录
# download_dir = ""

# block_size_http：HTTP 分块块大小（字节）
# 取值范围：正整数（≥1）；0 或非法值回退默认（1 MB）
# block_size_http = 1048576

# download_slots：全局下载槽位数（同时下载的任务数上限）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# download_slots = 5

# max_speed：全局下载限速（0 = 不限）
# 取值范围：≥0；支持 "2 MB/s" / "500 KB/s" / 整数 B/s（十进制口径 1 MB = 1000000 B/s）
# max_speed = 0

# max_retries：自动重试上限次数（达上限转停等，可按 R 手动重试）
# 取值范围：≥1 的整数；0 或非法值回退默认
# max_retries = 5

# auto_retry：失败后是否自动重试
# 取值范围：true / false
# auto_retry = true

# backoff_initial：自动重试退避初始秒（指数退避序列起点）
# 取值范围：>0 的有限数；非法值回退默认
# backoff_initial = 8.0

# backoff_cap：自动重试退避封顶秒
# 取值范围：>0 的有限数；非法值回退默认
# backoff_cap = 60.0

# http_concurrency：默认并发数
# 取值范围：1–64 的整数；越界钳制到边界
# http_concurrency = 4

# http_trackers：HTTP(S) tracker 列表（BT 任务默认 tracker 池来源之一，phase-02）
# 取值范围：字符串数组，每项为 http:// 或 https:// 开头的 tracker announce URL；非法 URL 行启动时忽略并警告
# http_trackers = []

# udp_trackers：UDP tracker 列表（BT 任务默认 tracker 池来源之一，phase-02）
# 取值范围：字符串数组，每项为 udp:// 开头的 tracker announce URL（含端口）；非法 URL 行启动时忽略并警告
# udp_trackers = []

# proxies：命名代理列表（可配置多个）
# [[proxies]]
# name = "办公网代理"
# type = "http"
# ip = "proxy.corp.example"
# port = 8080
# username = "alice"
# password = "secret"

# [[proxies]]
# name = "本地 SOCKS5"
# type = "socks5"
# ip = "127.0.0.1"
# port = 1080
# username = "ezr"
# password = "secret"
"#;

impl Config {
    #[must_use]
    pub fn default_template() -> String {
        AUTHORIZED_TEMPLATE.to_string()
    }
}

pub fn ensure_default_config(path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut f) => std::io::Write::write_all(&mut f, Config::default_template().as_bytes()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
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
            "block_size_http = 0\ndownload_slots = 0\nmax_retries = 0\nhttp_concurrency = 999\n",
        );
        assert_eq!(c.block_size_http, DEFAULT_BLOCK_SIZE_HTTP);
        assert_eq!(c.download_slots, 5);
        assert_eq!(c.max_retries, 5);
        assert_eq!(c.http_concurrency, 64);
    }

    #[test]
    fn block_size_http_positive_below_floor_clamped() {
        let c = Config::from_toml("block_size_http = 512\n");
        assert_eq!(c.block_size_http, 1_048_576);
        let c = Config::from_toml("block_size_http = 1048575\n");
        assert_eq!(c.block_size_http, 1_048_576, "下限之下 1 字节也钳制");
        let c = Config::from_toml("block_size_http = 1048576\n");
        assert_eq!(c.block_size_http, 1_048_576, "恰在下限 = 原值");
        let c = Config::from_toml("block_size_http = 2097152\n");
        assert_eq!(c.block_size_http, 2_097_152, "大于下限原值生效");
    }

    #[test]
    fn legacy_concurrency_key_ignored() {
        let c = Config::from_toml("default_concurrency = 9\n");
        assert_eq!(c.http_concurrency, DEFAULT_HTTP_CONCURRENCY);
        assert!(c.warnings.is_empty(), "未知键静默忽略");
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
    fn legacy_proxy_key_ignored() {
        let c = Config::from_toml("proxy = \"http://127.0.0.1:8118\"\n");
        assert_eq!(c, Config::default(), "旧 proxy 键按未知键忽略");
        assert!(c.warnings.is_empty());
    }

    #[test]
    fn empty_dir_and_invalid_numbers_fall_back() {
        let c = Config::from_toml(
            "download_dir = \"   \"\nmax_speed = \"garbage\"\nbackoff_initial = -1.0\nbackoff_cap = 0.0\n",
        );
        assert!(c.download_dir.is_none(), "空白目录弃用");
        assert_eq!(c.max_speed, 0, "不可解析限速弃用");
        assert_eq!(c.backoff_initial, Config::default().backoff_initial);
        assert_eq!(c.backoff_cap, Config::default().backoff_cap);
    }

    #[test]
    fn default_download_dir_is_home_downloads() {
        let dir = default_download_dir();
        assert!(dir.ends_with("Downloads"), "dir={dir}");
    }

    #[test]
    fn ezr_home_env_relocates_config_and_state() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-home-env");
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

    const CHARTER_TEMPLATE_TEXT: &str = r#"# EZR Downloader 配置模板

# download_dir：默认保存目录（添加对话框目录留空时使用）
# 取值范围：任意目录路径；留空或缺失 = 用户主目录下的下载目录
# download_dir = ""

# block_size_http：HTTP 分块块大小（字节）
# 取值范围：正整数（≥1）；0 或非法值回退默认（1 MB）
# block_size_http = 1048576

# download_slots：全局下载槽位数（同时下载的任务数上限）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# download_slots = 5

# max_speed：全局下载限速（0 = 不限）
# 取值范围：≥0；支持 "2 MB/s" / "500 KB/s" / 整数 B/s（十进制口径 1 MB = 1000000 B/s）
# max_speed = 0

# max_retries：自动重试上限次数（达上限转停等，可按 R 手动重试）
# 取值范围：≥1 的整数；0 或非法值回退默认
# max_retries = 5

# auto_retry：失败后是否自动重试
# 取值范围：true / false
# auto_retry = true

# backoff_initial：自动重试退避初始秒（指数退避序列起点）
# 取值范围：>0 的有限数；非法值回退默认
# backoff_initial = 8.0

# backoff_cap：自动重试退避封顶秒
# 取值范围：>0 的有限数；非法值回退默认
# backoff_cap = 60.0

# http_concurrency：默认并发数
# 取值范围：1–64 的整数；越界钳制到边界
# http_concurrency = 4

# http_trackers：HTTP(S) tracker 列表（BT 任务默认 tracker 池来源之一，phase-02）
# 取值范围：字符串数组，每项为 http:// 或 https:// 开头的 tracker announce URL；非法 URL 行启动时忽略并警告
# http_trackers = []

# udp_trackers：UDP tracker 列表（BT 任务默认 tracker 池来源之一，phase-02）
# 取值范围：字符串数组，每项为 udp:// 开头的 tracker announce URL（含端口）；非法 URL 行启动时忽略并警告
# udp_trackers = []

# proxies：命名代理列表（可配置多个）
# [[proxies]]
# name = "办公网代理"
# type = "http"
# ip = "proxy.corp.example"
# port = 8080
# username = "alice"
# password = "secret"

# [[proxies]]
# name = "本地 SOCKS5"
# type = "socks5"
# ip = "127.0.0.1"
# port = 1080
# username = "ezr"
# password = "secret"
"#;

    #[test]
    fn template_matches_charter_text() {
        let t = Config::default_template();
        assert_eq!(
            t,
            CHARTER_TEMPLATE_TEXT,
            "default_template() 输出必须与钦定原文逐字节一致（v1.10/FR-01-85）\n{}",
            first_diff(CHARTER_TEMPLATE_TEXT, &t)
        );
    }

    fn first_diff(expected: &str, actual: &str) -> String {
        let (e, a): (Vec<&str>, Vec<&str>) = (expected.lines().collect(), actual.lines().collect());
        let mut out = String::new();
        for i in 0..e.len().max(a.len()) {
            let el = e.get(i).copied().unwrap_or("<EOF>");
            let al = a.get(i).copied().unwrap_or("<EOF>");
            if el != al {
                let n = i + 1;
                out.push_str(&format!("  行 {n}：钦定={el:?} 实际={al:?}\n"));
            }
        }
        if out.is_empty() {
            out.push_str("  行内容一致——差异在行尾/EOF 收尾字节（核对末尾换行符数量）");
        }
        out
    }

    #[test]
    fn template_lines_all_commented() {
        for line in Config::default_template().lines() {
            let t = line.trim_start();
            assert!(
                t.is_empty() || t.starts_with('#'),
                "模板出现非注释行：{line}"
            );
        }
    }

    #[test]
    fn template_defaults_match_contract() {
        let t = Config::default_template();
        for expected in [
            "# download_dir = \"\"",
            "# block_size_http = 1048576",
            "# download_slots = 5",
            "# max_speed = 0",
            "# max_retries = 5",
            "# auto_retry = true",
            "# backoff_initial = 8.0",
            "# backoff_cap = 60.0",
            "# http_concurrency = 4",
        ] {
            assert!(t.contains(expected), "模板缺默认值行：{expected}");
        }
        assert!(
            !t.contains("\n# proxy = "),
            "旧 proxy 键已退役，模板不得输出（FR-01-90）"
        );
    }

    #[test]
    fn template_proxy_dual_examples_with_type() {
        let t = Config::default_template();
        assert!(
            t.contains("# proxies：命名代理列表（可配置多个）"),
            "proxies 段一句头注释在位"
        );
        assert_eq!(t.matches("# [[proxies]]").count(), 2, "双示例块");
        for expected in [
            "# name = \"办公网代理\"",
            "# type = \"http\"",
            "# ip = \"proxy.corp.example\"",
            "# port = 8080",
            "# username = \"alice\"",
            "# password = \"secret\"",
            "# name = \"本地 SOCKS5\"",
            "# type = \"socks5\"",
            "# ip = \"127.0.0.1\"",
            "# port = 1080",
            "# username = \"ezr\"",
        ] {
            assert!(t.contains(expected), "模板代理示例缺行：{expected}");
        }
        assert!(
            !t.contains("# url ="),
            "url 键退役，模板不得输出 url 行（FR-01-86 v1.9）"
        );
        assert!(!t.contains("凭证"), "凭证规则注释行退出模板（FR-01-93⑤）");
        assert!(!t.contains("唯一事实来源"), "type 语义注释退出模板");
        assert!(!t.contains("代理自身走 TLS"), "https 语义注释退出模板");
        assert!(!t.contains("方括号"), "IPv6 注记退出模板");
        assert!(!t.contains("型示例"), "示例块凭证语义注记退出模板");
    }

    #[test]
    fn template_documents_purpose_and_range_for_every_key() {
        let t = Config::default_template();
        let blocks: Vec<&str> = t.split("\n\n").collect();
        for (key, purpose) in [
            ("download_dir", "保存目录"),
            ("block_size_http", "块大小"),
            ("download_slots", "槽位"),
            ("max_speed", "限速"),
            ("max_retries", "重试上限"),
            ("auto_retry", "自动重试"),
            ("backoff_initial", "退避"),
            ("backoff_cap", "退避"),
            ("http_concurrency", "并发"),
            ("http_trackers", "tracker"),
            ("udp_trackers", "tracker"),
        ] {
            let block = blocks
                .iter()
                .find(|b| b.contains(&format!("# {key} = ")))
                .unwrap_or_else(|| panic!("模板缺键 {key} 的默认值段落"));
            assert!(
                block.contains(purpose),
                "键 {key} 的段落缺用途说明（应含「{purpose}」）"
            );
            assert!(block.contains("取值范围"), "键 {key} 的段落缺取值范围标注");
        }
        assert_eq!(
            t.matches("取值范围").count(),
            11,
            "11 键各一段取值范围标注（phase-02/FR-02-21 增 tracker 两键）；proxies 段豁免"
        );
    }

    #[test]
    fn template_parses_to_defaults() {
        assert_eq!(
            Config::from_toml(&Config::default_template()),
            Config::default()
        );
    }

    #[test]
    fn proxies_parse_with_auth() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"a\"\ntype = \"http\"\nip = \"127.0.0.1\"\nport = 8080\nusername = \"u\"\npassword = \"p\"\n[[proxies]]\nname = \"b\"\ntype = \"socks5\"\nip = \"127.0.0.1\"\nport = 1080\nusername = \"u2\"\npassword = \"p2\"\n",
        );
        assert_eq!(c.proxies.len(), 2);
        assert_eq!(c.proxies[0].name, "a");
        assert_eq!(c.proxies[0].kind, ProxyKind::Http);
        assert_eq!(c.proxies[0].ip, "127.0.0.1");
        assert_eq!(c.proxies[0].port, 8080, "port 以整数落位（u16）");
        assert_eq!(c.proxies[0].username.as_deref(), Some("u"));
        assert_eq!(c.proxies[0].password.as_deref(), Some("p"));
        assert_eq!(c.proxies[1].kind, ProxyKind::Socks5);
        assert_eq!(c.proxies[1].port, 1080);
        assert!(c.warnings.is_empty());
    }

    #[test]
    fn proxy_type_three_values_explicit() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"h\"\ntype = \"http\"\nip = \"x\"\nport = 1\n[[proxies]]\nname = \"hs\"\ntype = \"HTTPS\"\nip = \"x\"\nport = 2\n[[proxies]]\nname = \"s5\"\ntype = \"socks5\"\nip = \"x\"\nport = 3\nusername = \"u\"\npassword = \"p\"\n",
        );
        assert_eq!(c.proxies.len(), 3);
        assert_eq!(c.proxies[0].kind, ProxyKind::Http);
        assert_eq!(
            c.proxies[1].kind,
            ProxyKind::Https,
            "type=\"https\"（大小写不敏感）为独立类型（v1.9 三值）"
        );
        assert_eq!(c.proxies[2].kind, ProxyKind::Socks5);
        assert!(c.warnings.is_empty(), "合法三值 type 不产生警告");
    }

    #[test]
    fn proxy_type_missing_invalidated() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"no-type\"\nip = \"127.0.0.1\"\nport = 18766\n[[proxies]]\nname = \"valid\"\ntype = \"http\"\nip = \"x\"\nport = 9\n",
        );
        assert_eq!(c.proxies.len(), 1, "type 缺失即作废，仅剩合法条目");
        assert_eq!(c.proxies[0].name, "valid");
        assert_eq!(c.warnings.len(), 1, "作废警告恰一次");
        assert!(
            c.warnings[0].contains("no-type"),
            "警告点名条目：{}",
            c.warnings[0]
        );
        assert!(c.warnings[0].contains("type 缺失"), "{}", c.warnings[0]);
        assert!(
            c.warnings[0].contains("http / https / socks5"),
            "警告体现 type 三值：{}",
            c.warnings[0]
        );
    }

    #[test]
    fn proxy_type_unknown_invalidated() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"badtype\"\ntype = \"ftp\"\nip = \"127.0.0.1\"\nport = 18766\n",
        );
        assert!(c.proxies.is_empty(), "type 未知即作废（v1.9 不再保留）");
        assert_eq!(c.warnings.len(), 1);
        assert!(c.warnings[0].contains("type 未知"), "{}", c.warnings[0]);
        assert!(
            c.warnings[0].contains("http / https / socks5"),
            "警告体现 type 三值：{}",
            c.warnings[0]
        );
        assert!(!c.warnings[0].contains("password"), "警告不回显凭证值");
    }

    #[test]
    fn legacy_url_key_ignored() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"old\"\ntype = \"http\"\nurl = \"http://old.example:8080\"\nip = \"127.0.0.1\"\nport = 8080\n",
        );
        assert_eq!(c.proxies.len(), 1, "url 键忽略，条目按 ip/port/type 加载");
        assert_eq!(c.proxies[0].endpoint_url(), "http://127.0.0.1:8080");
        assert!(c.warnings.is_empty(), "url 键退役零警告");
    }

    #[test]
    fn proxy_ip_missing_or_blank_invalidated() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"bad-ip\"\ntype = \"http\"\nport = 18766\n[[proxies]]\nname = \"valid\"\ntype = \"http\"\nip = \"x\"\nport = 9\n",
        );
        assert_eq!(c.proxies.len(), 1, "ip 缺失即作废，仅剩合法条目");
        assert_eq!(c.proxies[0].name, "valid");
        assert_eq!(c.warnings.len(), 1, "作废警告恰一次");
        assert!(
            c.warnings[0].contains("bad-ip"),
            "警告点名条目：{}",
            c.warnings[0]
        );
        assert!(c.warnings[0].contains("ip"), "{}", c.warnings[0]);
        for ipval in ["\"\"", "\"   \""] {
            let c2 = Config::from_toml(&format!(
                "[[proxies]]\nname = \"bad-ip\"\ntype = \"http\"\nip = {ipval}\nport = 18766\n"
            ));
            assert!(
                c2.proxies.is_empty(),
                "ip={ipval} 应作废（trim 后非空要求）"
            );
            assert_eq!(c2.warnings.len(), 1);
            assert!(
                c2.warnings[0].contains("ip 缺失或空白"),
                "{}",
                c2.warnings[0]
            );
        }
    }

    #[test]
    fn proxy_port_missing_or_out_of_range_invalidated() {
        for port_raw in [
            "",
            "port = \"eighty\"",
            "port = 0",
            "port = 65536",
            "port = -1",
        ] {
            let text = format!(
                "[[proxies]]\nname = \"bad-port\"\ntype = \"http\"\nip = \"127.0.0.1\"\n{port_raw}\n[[proxies]]\nname = \"valid\"\ntype = \"http\"\nip = \"x\"\nport = 9\n"
            );
            let c = Config::from_toml(&text);
            assert_eq!(
                c.proxies.len(),
                1,
                "port 原始值「{port_raw}」应作废该条目，仅剩合法条目"
            );
            assert_eq!(c.proxies[0].name, "valid");
            assert_eq!(c.warnings.len(), 1, "port=「{port_raw}」警告恰一次");
            assert!(
                c.warnings[0].contains("1..=65535"),
                "警告体现 port 合法域：{}",
                c.warnings[0]
            );
        }
    }

    #[test]
    fn endpoint_url_three_schemes() {
        let mk = |kind: ProxyKind| ProxyConfig {
            name: "p".into(),
            kind,
            ip: "127.0.0.1".into(),
            port: 8080,
            username: None,
            password: None,
        };
        assert_eq!(mk(ProxyKind::Http).endpoint_url(), "http://127.0.0.1:8080");
        assert_eq!(
            mk(ProxyKind::Https).endpoint_url(),
            "https://127.0.0.1:8080"
        );
        assert_eq!(
            mk(ProxyKind::Socks5).endpoint_url(),
            "socks5://127.0.0.1:8080"
        );
    }

    #[test]
    fn endpoint_url_ipv6_bracketed() {
        let mk = |kind: ProxyKind, ip: &str| ProxyConfig {
            name: "s6".into(),
            kind,
            ip: ip.into(),
            port: 18766,
            username: None,
            password: None,
        };
        assert_eq!(
            mk(ProxyKind::Http, "::1").endpoint_url(),
            "http://[::1]:18766",
            "IPv6 字面量 ip 加方括号"
        );
        assert_eq!(
            mk(ProxyKind::Socks5, "2001:db8::1").endpoint_url(),
            "socks5://[2001:db8::1]:18766"
        );
        assert_eq!(
            mk(ProxyKind::Http, "proxy.corp.example").endpoint_url(),
            "http://proxy.corp.example:18766",
            "主机名不加方括号"
        );
    }

    #[test]
    fn socks5_missing_username_invalidated() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"s5-no-user\"\ntype = \"socks5\"\nip = \"127.0.0.1\"\nport = 18768\npassword = \"p1-s3cret\"\n[[proxies]]\nname = \"valid\"\ntype = \"http\"\nip = \"x\"\nport = 9\n",
        );
        assert_eq!(
            c.proxies.len(),
            1,
            "socks5 缺 username 即作废，仅剩合法条目"
        );
        assert_eq!(c.proxies[0].name, "valid");
        assert_eq!(c.warnings.len(), 1, "作废警告恰一次");
        let w = &c.warnings[0];
        assert!(w.contains("socks5"), "警告含 socks5 提示：{w}");
        assert!(
            w.contains("username 与 password"),
            "警告含凭证字段语义：{w}"
        );
        assert!(
            w.contains("不支持匿名"),
            "警告含不支持匿名 socks5 语义：{w}"
        );
        assert!(!w.contains("p1-s3cret"), "警告不回显凭证值：{w}");
    }

    #[test]
    fn socks5_missing_password_invalidated() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"s5-no-pass\"\ntype = \"socks5\"\nip = \"127.0.0.1\"\nport = 18768\nusername = \"u1-s3cret\"\n[[proxies]]\nname = \"valid\"\ntype = \"http\"\nip = \"x\"\nport = 9\n",
        );
        assert_eq!(
            c.proxies.len(),
            1,
            "socks5 缺 password 即作废，仅剩合法条目"
        );
        assert_eq!(c.proxies[0].name, "valid");
        assert_eq!(c.warnings.len(), 1, "作废警告恰一次");
        let w = &c.warnings[0];
        assert!(w.contains("socks5"), "警告含 socks5 提示：{w}");
        assert!(
            w.contains("username 与 password"),
            "警告含凭证字段语义：{w}"
        );
        assert!(
            w.contains("不支持匿名"),
            "警告含不支持匿名 socks5 语义：{w}"
        );
        assert!(!w.contains("u1-s3cret"), "警告不回显凭证值：{w}");
    }

    #[test]
    fn socks5_blank_credentials_invalidated() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"s5-blank\"\ntype = \"socks5\"\nip = \"127.0.0.1\"\nport = 18768\nusername = \"u1\"\npassword = \"   \"\n",
        );
        assert!(c.proxies.is_empty(), "空白 password 归 None → 视同缺失作废");
        assert_eq!(c.warnings.len(), 1);
        assert!(c.warnings[0].contains("socks5"));
    }

    #[test]
    fn http_no_credentials_anonymous_kept() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"anon-http\"\ntype = \"http\"\nip = \"127.0.0.1\"\nport = 18766\n",
        );
        assert_eq!(
            c.proxies.len(),
            1,
            "匿名代理为 http 型合法形态，条目正常加载"
        );
        assert_eq!(c.proxies[0].name, "anon-http");
        assert_eq!(c.proxies[0].username, None);
        assert_eq!(c.proxies[0].password, None);
        assert!(c.warnings.is_empty(), "匿名代理不产生凭证相关警告");
    }

    #[test]
    fn http_half_credentials_invalidated() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"half-http\"\ntype = \"http\"\nip = \"127.0.0.1\"\nport = 18766\nusername = \"u1-s3cret\"\n[[proxies]]\nname = \"valid\"\ntype = \"http\"\nip = \"x\"\nport = 9\n",
        );
        assert_eq!(c.proxies.len(), 1, "只填 username 即作废，仅剩合法条目");
        assert_eq!(c.proxies[0].name, "valid");
        assert_eq!(c.warnings.len(), 1, "作废警告恰一次");
        assert!(
            c.warnings[0].contains("成对"),
            "警告含 username 与 password 需成对配置文案：{}",
            c.warnings[0]
        );
        assert!(!c.warnings[0].contains("u1-s3cret"), "警告不回显凭证值");
        let c2 = Config::from_toml(
            "[[proxies]]\nname = \"half-http\"\ntype = \"http\"\nip = \"127.0.0.1\"\nport = 18766\npassword = \"p1-s3cret\"\n",
        );
        assert!(c2.proxies.is_empty(), "只填 password 同样作废");
        assert_eq!(c2.warnings.len(), 1);
        assert!(c2.warnings[0].contains("成对"));
        assert!(!c2.warnings[0].contains("p1-s3cret"), "警告不回显凭证值");
    }

    #[test]
    fn https_credentials_three_phases() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"anon-tls\"\ntype = \"https\"\nip = \"127.0.0.1\"\nport = 18767\n",
        );
        assert_eq!(c.proxies.len(), 1, "https 匿名代理合法加载");
        assert_eq!(c.proxies[0].kind, ProxyKind::Https);
        assert_eq!(c.proxies[0].username, None);
        assert_eq!(c.proxies[0].password, None);
        assert!(c.warnings.is_empty(), "匿名代理不产生凭证相关警告");
        let c2 = Config::from_toml(
            "[[proxies]]\nname = \"auth-tls\"\ntype = \"https\"\nip = \"127.0.0.1\"\nport = 18767\nusername = \"u1\"\npassword = \"p1\"\n",
        );
        assert_eq!(
            c2.proxies.len(),
            1,
            "https 成对凭证保留（TLS 会话内 basic auth）"
        );
        assert_eq!(c2.proxies[0].kind, ProxyKind::Https);
        assert_eq!(c2.proxies[0].username.as_deref(), Some("u1"));
        assert_eq!(c2.proxies[0].password.as_deref(), Some("p1"));
        assert!(c2.warnings.is_empty(), "成对凭证无警告");
        let c3 = Config::from_toml(
            "[[proxies]]\nname = \"half-tls\"\ntype = \"https\"\nip = \"127.0.0.1\"\nport = 18767\nusername = \"u1-s3cret\"\n",
        );
        assert!(
            c3.proxies.is_empty(),
            "https 半填即作废（与 http 同级口径）"
        );
        assert_eq!(c3.warnings.len(), 1);
        assert!(
            c3.warnings[0].contains("成对"),
            "警告含成对配置文案：{}",
            c3.warnings[0]
        );
        assert!(
            c3.warnings[0].contains("https"),
            "警告体现 https 类型：{}",
            c3.warnings[0]
        );
        assert!(!c3.warnings[0].contains("u1-s3cret"), "警告不回显凭证值");
    }

    #[test]
    fn socks5_with_credentials_kept() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"auth-s5\"\ntype = \"socks5\"\nip = \"127.0.0.1\"\nport = 18768\nusername = \"u1\"\npassword = \"p1\"\n",
        );
        assert_eq!(c.proxies.len(), 1, "socks5 凭证齐备 → 条目保留");
        let p = &c.proxies[0];
        assert_eq!(p.kind, ProxyKind::Socks5);
        assert_eq!(p.port, 18768);
        assert_eq!(p.username.as_deref(), Some("u1"), "凭证字段在位");
        assert_eq!(p.password.as_deref(), Some("p1"), "凭证字段在位");
        assert!(c.warnings.is_empty(), "合法条目无警告");
    }

    #[test]
    fn invalid_proxy_entries_skipped_with_warning() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"\"\ntype = \"http\"\nip = \"x\"\nport = 1\n[[proxies]]\nname = \"dup\"\ntype = \"http\"\nip = \"x\"\nport = 2\n[[proxies]]\nname = \"dup\"\ntype = \"http\"\nip = \"x\"\nport = 3\n[[proxies]]\nname = \"ok\"\ntype = \"http\"\nip = \"  \"\nport = 4\n[[proxies]]\nname = \"ok\"\ntype = \"http\"\nip = \"x\"\nport = 9\n",
        );
        assert_eq!(c.proxies.len(), 2, "首个 dup 合法在位 + ok");
        assert_eq!(c.proxies[0].name, "dup", "重名条目保留首个");
        assert_eq!(c.proxies[1].name, "ok");
        assert_eq!(c.warnings.len(), 3, "空名 1 + 重名 1 + 空 ip 1");
        assert!(c.warnings.iter().all(|w| !w.contains("password")));
    }

    #[test]
    fn resolve_proxy_two_states() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"a\"\ntype = \"http\"\nip = \"a\"\nport = 2\nusername = \"u\"\npassword = \"pw\"\n",
        );
        assert_eq!(c.resolve_proxy(&ProxyChoice::Direct), (None, false));
        let (ep, missing) = c.resolve_proxy(&ProxyChoice::Named("a".into()));
        let ep = ep.unwrap();
        assert_eq!(
            (ep.url.as_str(), ep.username.as_deref()),
            ("http://a:2", Some("u"))
        );
        assert_eq!(ep.kind, ProxyKind::Http);
        assert!(!missing);
        assert_eq!(
            c.resolve_proxy(&ProxyChoice::Named("gone".into())),
            (None, true)
        );
    }

    #[test]
    fn ensure_creates_template_when_missing() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-tpl-create");
        let p = dir.join("nested/config.toml");
        std::fs::remove_dir_all(&dir).ok();
        ensure_default_config(&p).unwrap();
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            Config::default_template(),
            "缺失时写入模板原文"
        );
        ensure_default_config(&p).unwrap();
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            Config::default_template(),
            "已存在后再次调用内容不变（幂等）"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ensure_keeps_existing_content() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-tpl-keep");
        let p = dir.join("config.toml");
        std::fs::write(&p, "download_slots = 2\n").unwrap();
        ensure_default_config(&p).unwrap();
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            "download_slots = 2\n",
            "已存在文件字节级不变"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn ensure_propagates_write_error() {
        use std::os::unix::fs::PermissionsExt;
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-tpl-ro");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        let p = dir.join("config.toml");
        let r = ensure_default_config(&p);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(r.is_err(), "只读目录写失败应报错");
        assert!(!p.exists(), "失败路径不得留下半截文件");
        std::fs::remove_dir_all(&dir).ok();
    }
}
