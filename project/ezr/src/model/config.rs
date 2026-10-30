use std::path::{Path, PathBuf};

use serde::Deserialize;

pub const DEFAULT_BLOCK_SIZE_HTTP: u64 = super::chunk::HTTP_CHUNK_SIZE;
pub const DEFAULT_DOWNLOAD_SLOTS: usize = 5;
pub const DEFAULT_HTTP_CONCURRENCY: usize = 4;
pub const DEFAULT_MAX_RETRIES: u32 = 5;
pub const DEFAULT_PROGRESS_TICK_MS: u64 = 100;
pub const DEFAULT_SIDECAR_FLUSH_SECS: u64 = 2;
pub const DEFAULT_THROTTLE_BURST_SECS: f64 = 0.5;
pub const DEFAULT_THROTTLE_SLEEP_MS: u64 = 20;
pub const DEFAULT_DNS_RACE_GRACE_MS: u64 = 1000;
pub const DEFAULT_BT_CONCURRENCY: usize = 20;
pub const DEFAULT_PEER_PIPELINE: usize = 64;
pub const DEFAULT_UPLOAD_SLOTS: usize = 4;
pub const DEFAULT_UPLOAD_READ_CONCURRENCY: usize = 128;
pub const DEFAULT_IDLE_EVICTION_SECS: u64 = 60;
pub const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 3;
pub const DEFAULT_HANDSHAKE_TIMEOUT_SECS: u64 = 10;
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 20;
pub const DEFAULT_REQUESTS_PER_PIECE: usize = 4;
pub const DEFAULT_RECHOKE_INTERVAL_SECS: u64 = 10;
pub const DEFAULT_OPTIMISTIC_INTERVAL_SECS: u64 = 30;
pub const DEFAULT_PROGRESS_EMIT_MIN_INTERVAL_MS: u64 = 250;
pub const DEFAULT_TRACKER_NUMWANT: u32 = 200;
pub const DEFAULT_PEER_RETRY_BASE_SECS: u64 = 5;
pub const DEFAULT_PEER_RETRY_CAP_SECS: u64 = 120;
pub const DEFAULT_TRACKER_BACKOFF_START_SECS: u64 = 8;
pub const DEFAULT_TRACKER_BACKOFF_CAP_SECS: u64 = 60;
pub const DEFAULT_UDP_BASE_TIMEOUT_SECS: u64 = 15;
pub const DEFAULT_UDP_RETRIES: u32 = 3;
pub const DEFAULT_SEED_DURATION_SECS: f64 = 1800.0;

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
    pub download_slots: usize,
    pub max_speed: u64,
    pub proxies: Vec<ProxyConfig>,
    pub block_size_http: u64,
    pub http_concurrency: usize,
    pub max_retries: u32,
    pub auto_retry: bool,
    pub backoff_initial: f64,
    pub backoff_cap: f64,
    pub progress_tick_ms: u64,
    pub sidecar_flush_secs: u64,
    pub throttle_burst_secs: f64,
    pub throttle_sleep_ms: u64,
    pub dns_race_grace_ms: u64,
    pub http_trackers: Vec<String>,
    pub udp_trackers: Vec<String>,
    pub bt_concurrency: usize,
    pub peer_pipeline: usize,
    pub upload_slots: usize,
    pub upload_read_concurrency: usize,
    pub idle_eviction_secs: u64,
    pub connect_timeout_secs: u64,
    pub handshake_timeout_secs: u64,
    pub request_timeout_secs: u64,
    pub requests_per_piece: usize,
    pub rechoke_interval_secs: u64,
    pub optimistic_interval_secs: u64,
    pub progress_emit_min_interval_ms: u64,
    pub tracker_numwant: u32,
    pub peer_retry_base_secs: u64,
    pub peer_retry_cap_secs: u64,
    pub tracker_backoff_start_secs: u64,
    pub tracker_backoff_cap_secs: u64,
    pub udp_base_timeout_secs: u64,
    pub udp_retries: u32,
    pub seed_duration_secs: f64,
    pub warnings: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            download_dir: None,
            download_slots: DEFAULT_DOWNLOAD_SLOTS,
            max_speed: 0,
            proxies: Vec::new(),
            block_size_http: DEFAULT_BLOCK_SIZE_HTTP,
            http_concurrency: DEFAULT_HTTP_CONCURRENCY,
            max_retries: DEFAULT_MAX_RETRIES,
            auto_retry: true,
            backoff_initial: 8.0,
            backoff_cap: 60.0,
            progress_tick_ms: DEFAULT_PROGRESS_TICK_MS,
            sidecar_flush_secs: DEFAULT_SIDECAR_FLUSH_SECS,
            throttle_burst_secs: DEFAULT_THROTTLE_BURST_SECS,
            throttle_sleep_ms: DEFAULT_THROTTLE_SLEEP_MS,
            dns_race_grace_ms: DEFAULT_DNS_RACE_GRACE_MS,
            http_trackers: Vec::new(),
            udp_trackers: Vec::new(),
            bt_concurrency: DEFAULT_BT_CONCURRENCY,
            peer_pipeline: DEFAULT_PEER_PIPELINE,
            upload_slots: DEFAULT_UPLOAD_SLOTS,
            upload_read_concurrency: DEFAULT_UPLOAD_READ_CONCURRENCY,
            idle_eviction_secs: DEFAULT_IDLE_EVICTION_SECS,
            connect_timeout_secs: DEFAULT_CONNECT_TIMEOUT_SECS,
            handshake_timeout_secs: DEFAULT_HANDSHAKE_TIMEOUT_SECS,
            request_timeout_secs: DEFAULT_REQUEST_TIMEOUT_SECS,
            requests_per_piece: DEFAULT_REQUESTS_PER_PIECE,
            rechoke_interval_secs: DEFAULT_RECHOKE_INTERVAL_SECS,
            optimistic_interval_secs: DEFAULT_OPTIMISTIC_INTERVAL_SECS,
            progress_emit_min_interval_ms: DEFAULT_PROGRESS_EMIT_MIN_INTERVAL_MS,
            tracker_numwant: DEFAULT_TRACKER_NUMWANT,
            peer_retry_base_secs: DEFAULT_PEER_RETRY_BASE_SECS,
            peer_retry_cap_secs: DEFAULT_PEER_RETRY_CAP_SECS,
            tracker_backoff_start_secs: DEFAULT_TRACKER_BACKOFF_START_SECS,
            tracker_backoff_cap_secs: DEFAULT_TRACKER_BACKOFF_CAP_SECS,
            udp_base_timeout_secs: DEFAULT_UDP_BASE_TIMEOUT_SECS,
            udp_retries: DEFAULT_UDP_RETRIES,
            seed_duration_secs: DEFAULT_SEED_DURATION_SECS,
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
struct CommonRaw {
    download_dir: Option<String>,
    download_slots: Option<i64>,
    max_speed: Option<toml::Value>,
    proxy: Option<Vec<ProxyRaw>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct HttpRaw {
    block_size_http: Option<i64>,
    http_concurrency: Option<i64>,
    max_retries: Option<i64>,
    auto_retry: Option<bool>,
    backoff_initial: Option<f64>,
    backoff_cap: Option<f64>,
    progress_tick_ms: Option<i64>,
    sidecar_flush_secs: Option<i64>,
    throttle_burst_secs: Option<f64>,
    throttle_sleep_ms: Option<i64>,
    dns_race_grace_ms: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct BtRaw {
    http_trackers: Option<Vec<String>>,
    udp_trackers: Option<Vec<String>>,
    bt_concurrency: Option<i64>,
    peer_pipeline: Option<i64>,
    upload_slots: Option<i64>,
    upload_read_concurrency: Option<i64>,
    idle_eviction_secs: Option<i64>,
    connect_timeout_secs: Option<i64>,
    handshake_timeout_secs: Option<i64>,
    request_timeout_secs: Option<i64>,
    requests_per_piece: Option<i64>,
    rechoke_interval_secs: Option<i64>,
    optimistic_interval_secs: Option<i64>,
    progress_emit_min_interval_ms: Option<i64>,
    tracker_numwant: Option<i64>,
    peer_retry_base_secs: Option<i64>,
    peer_retry_cap_secs: Option<i64>,
    tracker_backoff_start_secs: Option<i64>,
    tracker_backoff_cap_secs: Option<i64>,
    udp_base_timeout_secs: Option<i64>,
    udp_retries: Option<i64>,
    seed_duration_secs: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct ConfigRaw {
    common: Option<CommonRaw>,
    http: Option<HttpRaw>,
    bt: Option<BtRaw>,
}

fn min1_u64(v: Option<i64>, d: u64) -> u64 {
    v.and_then(|v| u64::try_from(v).ok())
        .filter(|v| *v >= 1)
        .unwrap_or(d)
}

fn min1_usize(v: Option<i64>, d: usize) -> usize {
    v.filter(|v| *v >= 1)
        .and_then(|v| usize::try_from(v).ok())
        .unwrap_or(d)
}

fn min1_u32(v: Option<i64>, d: u32) -> u32 {
    v.filter(|v| *v >= 1)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(d)
}

fn clamp_u(v: Option<i64>, d: usize) -> usize {
    v.filter(|v| *v >= 1)
        .map(|v| v.clamp(1, 64) as usize)
        .unwrap_or(d)
}

fn pos_f64(v: Option<f64>, d: f64) -> f64 {
    v.filter(|v| *v > 0.0 && v.is_finite()).unwrap_or(d)
}

fn max_speed_of(v: Option<toml::Value>) -> u64 {
    match v {
        Some(toml::Value::String(s)) => parse_speed(&s),
        Some(toml::Value::Integer(n)) => parse_speed(&n.to_string()),
        _ => 0,
    }
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
        let common_proxy = raw.common.as_ref().and_then(|c| c.proxy.clone()).unwrap_or_default();
        for p in common_proxy {
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
        let common = raw.common.unwrap_or_default();
        let http = raw.http.unwrap_or_default();
        let bt = raw.bt.unwrap_or_default();
        Config {
            download_dir: common.download_dir.filter(|s| !s.trim().is_empty()),
            download_slots: min1_usize(common.download_slots, d.download_slots),
            max_speed: max_speed_of(common.max_speed),
            proxies,
            block_size_http: http
                .block_size_http
                .and_then(|v| u64::try_from(v).ok())
                .filter(|v| *v > 0)
                .map(|v| v.max(super::chunk::MIN_HTTP_BLOCK_SIZE))
                .unwrap_or(d.block_size_http),
            http_concurrency: clamp_u(http.http_concurrency, d.http_concurrency),
            max_retries: min1_u32(http.max_retries, d.max_retries),
            auto_retry: http.auto_retry.unwrap_or(d.auto_retry),
            backoff_initial: pos_f64(http.backoff_initial, d.backoff_initial),
            backoff_cap: pos_f64(http.backoff_cap, d.backoff_cap),
            progress_tick_ms: min1_u64(http.progress_tick_ms, d.progress_tick_ms),
            sidecar_flush_secs: min1_u64(http.sidecar_flush_secs, d.sidecar_flush_secs),
            throttle_burst_secs: pos_f64(http.throttle_burst_secs, d.throttle_burst_secs),
            throttle_sleep_ms: min1_u64(http.throttle_sleep_ms, d.throttle_sleep_ms),
            dns_race_grace_ms: min1_u64(http.dns_race_grace_ms, d.dns_race_grace_ms),
            http_trackers: sanitize_trackers(
                bt.http_trackers.unwrap_or_default(),
                &["http://", "https://"],
                "http_trackers",
                &mut warnings,
            ),
            udp_trackers: sanitize_trackers(
                bt.udp_trackers.unwrap_or_default(),
                &["udp://"],
                "udp_trackers",
                &mut warnings,
            ),
            bt_concurrency: clamp_u(bt.bt_concurrency, d.bt_concurrency),
            peer_pipeline: min1_usize(bt.peer_pipeline, d.peer_pipeline),
            upload_slots: min1_usize(bt.upload_slots, d.upload_slots),
            upload_read_concurrency: min1_usize(bt.upload_read_concurrency, d.upload_read_concurrency),
            idle_eviction_secs: min1_u64(bt.idle_eviction_secs, d.idle_eviction_secs),
            connect_timeout_secs: min1_u64(bt.connect_timeout_secs, d.connect_timeout_secs),
            handshake_timeout_secs: min1_u64(bt.handshake_timeout_secs, d.handshake_timeout_secs),
            request_timeout_secs: min1_u64(bt.request_timeout_secs, d.request_timeout_secs),
            requests_per_piece: min1_usize(bt.requests_per_piece, d.requests_per_piece),
            rechoke_interval_secs: min1_u64(bt.rechoke_interval_secs, d.rechoke_interval_secs),
            optimistic_interval_secs: min1_u64(bt.optimistic_interval_secs, d.optimistic_interval_secs),
            progress_emit_min_interval_ms: min1_u64(
                bt.progress_emit_min_interval_ms,
                d.progress_emit_min_interval_ms,
            ),
            tracker_numwant: min1_u32(bt.tracker_numwant, d.tracker_numwant),
            peer_retry_base_secs: min1_u64(bt.peer_retry_base_secs, d.peer_retry_base_secs),
            peer_retry_cap_secs: min1_u64(bt.peer_retry_cap_secs, d.peer_retry_cap_secs),
            tracker_backoff_start_secs: min1_u64(
                bt.tracker_backoff_start_secs,
                d.tracker_backoff_start_secs,
            ),
            tracker_backoff_cap_secs: min1_u64(bt.tracker_backoff_cap_secs, d.tracker_backoff_cap_secs),
            udp_base_timeout_secs: min1_u64(bt.udp_base_timeout_secs, d.udp_base_timeout_secs),
            udp_retries: min1_u32(bt.udp_retries, d.udp_retries),
            seed_duration_secs: pos_f64(bt.seed_duration_secs, d.seed_duration_secs),
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
pub fn expand_tilde_with(path: &str, home: Option<&Path>) -> PathBuf {
    let p = path.trim();
    if p == "~" {
        return home
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(p));
    }
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(h) = home {
            return h.join(rest);
        }
    }
    PathBuf::from(p)
}

#[must_use]
pub fn expand_tilde(path: &str) -> PathBuf {
    expand_tilde_with(path, home_dir().as_deref())
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

# ===== [common] 公共配置（两引擎共用） =====

# download_dir：默认保存目录（添加对话框目录留空时使用）
# 取值范围：任意目录路径；留空或缺失 = 用户主目录下的下载目录
# download_dir = ""

# download_slots：全局下载槽位数（同时下载的任务数上限）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# download_slots = 5

# max_speed：全局下载限速（0 = 不限）
# 取值范围：≥0；支持 "2 MB/s" / "500 KB/s" / 整数 B/s（十进制口径 1 MB = 1000000 B/s）
# max_speed = 0

# proxy：命名代理列表（可配置多个）
# [[common.proxy]]
# name = "办公网代理"
# type = "http"
# ip = "proxy.corp.example"
# port = 8080
# username = "alice"
# password = "secret"

# [[common.proxy]]
# name = "本地 SOCKS5"
# type = "socks5"
# ip = "127.0.0.1"
# port = 1080
# username = "ezr"
# password = "secret"

# ===== [http] HTTP 引擎配置 =====

# block_size_http：HTTP 分块块大小（字节）
# 取值范围：正整数（≥1）；0 或非法值回退默认（1 MB）
# block_size_http = 1048576

# http_concurrency：默认并发数
# 取值范围：1–64 的整数；越界钳制到边界
# http_concurrency = 4

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

# progress_tick_ms：HTTP 下载进度事件节拍（毫秒）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# progress_tick_ms = 100

# sidecar_flush_secs：断点 sidecar 落盘间隔（秒）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# sidecar_flush_secs = 2

# throttle_burst_secs：限速令牌桶突发窗（秒）
# 取值范围：>0 的有限数；非法值回退默认
# throttle_burst_secs = 0.5

# throttle_sleep_ms：限速循环步进睡眠（毫秒）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# throttle_sleep_ms = 20

# dns_race_grace_ms：DNS 双腿竞速宽限（毫秒，双栈并行解析）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# dns_race_grace_ms = 1000

# ===== [bt] BT 引擎配置 =====

# http_trackers：HTTP(S) tracker 列表（BT 任务默认 tracker 池来源之一）
# 取值范围：字符串数组，每项为 http:// 或 https:// 开头的 tracker announce URL；非法 URL 行启动时忽略并警告
# http_trackers = []

# udp_trackers：UDP tracker 列表（BT 任务默认 tracker 池来源之一）
# 取值范围：字符串数组，每项为 udp:// 开头的 tracker announce URL（含端口）；非法 URL 行启动时忽略并警告
# udp_trackers = []

# bt_concurrency：BT 任务默认并发数（新任务预填）
# 取值范围：1–64 的整数；越界钳制到边界
# bt_concurrency = 20

# peer_pipeline：单 peer 在飞请求块数上限（×16 KiB = 在飞窗口）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# peer_pipeline = 64

# upload_slots：同时获得上传槽位的 peer 数（choker 周期分配）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# upload_slots = 4

# upload_read_concurrency：上传读盘并发上限（进程级）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# upload_read_concurrency = 128

# idle_eviction_secs：空闲连接置换阈值（秒，无新增进账超时即断开重拨）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# idle_eviction_secs = 60

# connect_timeout_secs：出连 TCP 拨号超时（秒）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# connect_timeout_secs = 3

# handshake_timeout_secs：peer 握手超时（秒）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# handshake_timeout_secs = 10

# request_timeout_secs：在飞块请求超时（秒）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# request_timeout_secs = 20

# requests_per_piece：单 piece 请求块粒度（piece 细分为 n 块请求）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# requests_per_piece = 4

# rechoke_interval_secs：上传 choker 重排周期（秒）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# rechoke_interval_secs = 10

# optimistic_interval_secs：乐观 unchoke 轮换周期（秒）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# optimistic_interval_secs = 30

# progress_emit_min_interval_ms：进度事件最小发送间隔（毫秒，节流）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# progress_emit_min_interval_ms = 250

# tracker_numwant：announce 请求的 peer 数上限
# 取值范围：正整数（≥1）；0 或非法值回退默认
# tracker_numwant = 200

# peer_retry_base_secs：peer 连接失败重试退避起始（秒，倍增）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# peer_retry_base_secs = 5

# peer_retry_cap_secs：peer 连接失败重试退避封顶（秒）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# peer_retry_cap_secs = 120

# tracker_backoff_start_secs：tracker 失败退避起始（秒，倍增）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# tracker_backoff_start_secs = 8

# tracker_backoff_cap_secs：tracker 失败退避封顶（秒）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# tracker_backoff_cap_secs = 60

# udp_base_timeout_secs：UDP tracker 单轮超时起始（秒，倍增重试）
# 取值范围：正整数（≥1）；0 或非法值回退默认
# udp_base_timeout_secs = 15

# udp_retries：UDP tracker 超时重试次数
# 取值范围：正整数（≥1）；0 或非法值回退默认
# udp_retries = 3

# seed_duration_secs：做种时长默认值（秒，任务做种倒计时起点）
# 取值范围：>0 的数；0 或非法值回退默认
# seed_duration_secs = 1800
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
        assert_eq!(c.progress_tick_ms, 100);
        assert_eq!(c.bt_concurrency, 20);
        assert_eq!(c.idle_eviction_secs, 60);
        assert_eq!(c.seed_duration_secs, 1800.0);
    }

    #[test]
    fn partial_keys_fallback_defaults() {
        let c = Config::from_toml("[common]\ndownload_slots = 3\n");
        assert_eq!(c.download_slots, 3);
        assert_eq!(c.block_size_http, DEFAULT_BLOCK_SIZE_HTTP);
    }

    #[test]
    fn invalid_values_fall_back() {
        let c = Config::from_toml(
            "[http]\nblock_size_http = 0\nmax_retries = 0\nhttp_concurrency = 999\n[common]\ndownload_slots = 0\n",
        );
        assert_eq!(c.block_size_http, DEFAULT_BLOCK_SIZE_HTTP);
        assert_eq!(c.download_slots, 5);
        assert_eq!(c.max_retries, 5);
        assert_eq!(c.http_concurrency, 64);
    }

    #[test]
    fn block_size_http_positive_below_floor_clamped() {
        let mk = |v: &str| Config::from_toml(&format!("[http]\nblock_size_http = {v}\n"));
        assert_eq!(mk("512").block_size_http, 1_048_576);
        assert_eq!(mk("1048575").block_size_http, 1_048_576, "下限之下 1 字节也钳制");
        assert_eq!(mk("1048576").block_size_http, 1_048_576, "恰在下限 = 原值");
        assert_eq!(mk("2097152").block_size_http, 2_097_152, "大于下限原值生效");
    }

    #[test]
    fn legacy_flat_keys_ignored_silently() {
        let c = Config::from_toml(
            "download_slots = 2\nblock_size_http = 4194304\nhttp_concurrency = 8\nmax_speed = \"1 MB/s\"\ndefault_concurrency = 9\nproxy = \"http://127.0.0.1:8118\"\n",
        );
        assert_eq!(c, Config::default(), "旧格式顶层键全部按未知键忽略，全部默认");
        assert!(c.warnings.is_empty(), "零警告（FR-01-71 未知键口径）");
    }

    #[test]
    fn legacy_proxies_table_ignored() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"a\"\ntype = \"http\"\nip = \"127.0.0.1\"\nport = 8080\n",
        );
        assert!(c.proxies.is_empty(), "旧 [[proxies]] 表不读");
        assert!(c.warnings.is_empty(), "旧表静默忽略零警告");
    }

    #[test]
    fn legacy_and_new_format_coexist() {
        let c = Config::from_toml(
            "download_slots = 2\n[common]\ndownload_slots = 3\n",
        );
        assert_eq!(c.download_slots, 3, "新表键生效且不受旧顶层键干扰");
        assert!(c.warnings.is_empty());
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
    fn max_speed_accepts_string_and_integer() {
        let c = Config::from_toml("[common]\nmax_speed = \"2 MB/s\"\n");
        assert_eq!(c.max_speed, 2_000_000);
        let c2 = Config::from_toml("[common]\nmax_speed = 1048576\n");
        assert_eq!(c2.max_speed, 1_048_576, "整数 B/s 形态直接生效");
        let c3 = Config::from_toml("[common]\nmax_speed = \"xyz\"\n");
        assert_eq!(c3.max_speed, 0, "不可解析限速弃用");
    }

    #[test]
    fn empty_dir_and_invalid_numbers_fall_back() {
        let c = Config::from_toml(
            "[common]\ndownload_dir = \"   \"\n[http]\nbackoff_initial = -1.0\nbackoff_cap = 0.0\n",
        );
        assert!(c.download_dir.is_none(), "空白目录弃用");
        assert_eq!(c.backoff_initial, Config::default().backoff_initial);
        assert_eq!(c.backoff_cap, Config::default().backoff_cap);
    }

    #[test]
    fn http_tunables_fallback_and_effective() {
        let d = Config::default();
        let bad = Config::from_toml(
            "[http]\nprogress_tick_ms = 0\nsidecar_flush_secs = -1\nthrottle_burst_secs = 0.0\nthrottle_sleep_ms = 0\ndns_race_grace_ms = 0\n",
        );
        assert_eq!(bad.progress_tick_ms, d.progress_tick_ms);
        assert_eq!(bad.sidecar_flush_secs, d.sidecar_flush_secs);
        assert_eq!(bad.throttle_burst_secs, d.throttle_burst_secs);
        assert_eq!(bad.throttle_sleep_ms, d.throttle_sleep_ms);
        assert_eq!(bad.dns_race_grace_ms, d.dns_race_grace_ms);
        let ok = Config::from_toml(
            "[http]\nprogress_tick_ms = 250\nsidecar_flush_secs = 5\nthrottle_burst_secs = 1.5\nthrottle_sleep_ms = 40\ndns_race_grace_ms = 2000\n",
        );
        assert_eq!(ok.progress_tick_ms, 250);
        assert_eq!(ok.sidecar_flush_secs, 5);
        assert_eq!(ok.throttle_burst_secs, 1.5);
        assert_eq!(ok.throttle_sleep_ms, 40);
        assert_eq!(ok.dns_race_grace_ms, 2000);
    }

    #[test]
    fn bt_tunables_fallback_and_effective() {
        let d = Config::default();
        let bad = Config::from_toml(
            "[bt]\nbt_concurrency = 0\npeer_pipeline = 0\nupload_slots = -1\nupload_read_concurrency = 0\nidle_eviction_secs = 0\nconnect_timeout_secs = 0\nhandshake_timeout_secs = 0\nrequest_timeout_secs = 0\nrequests_per_piece = 0\nrechoke_interval_secs = 0\noptimistic_interval_secs = 0\nprogress_emit_min_interval_ms = 0\ntracker_numwant = 0\npeer_retry_base_secs = 0\npeer_retry_cap_secs = 0\ntracker_backoff_start_secs = 0\ntracker_backoff_cap_secs = 0\nudp_base_timeout_secs = 0\nudp_retries = 0\nseed_duration_secs = 0.0\n",
        );
        assert_eq!(bad.bt_concurrency, d.bt_concurrency);
        assert_eq!(bad.peer_pipeline, d.peer_pipeline);
        assert_eq!(bad.upload_slots, d.upload_slots);
        assert_eq!(bad.upload_read_concurrency, d.upload_read_concurrency);
        assert_eq!(bad.idle_eviction_secs, d.idle_eviction_secs);
        assert_eq!(bad.connect_timeout_secs, d.connect_timeout_secs);
        assert_eq!(bad.handshake_timeout_secs, d.handshake_timeout_secs);
        assert_eq!(bad.request_timeout_secs, d.request_timeout_secs);
        assert_eq!(bad.requests_per_piece, d.requests_per_piece);
        assert_eq!(bad.rechoke_interval_secs, d.rechoke_interval_secs);
        assert_eq!(bad.optimistic_interval_secs, d.optimistic_interval_secs);
        assert_eq!(
            bad.progress_emit_min_interval_ms,
            d.progress_emit_min_interval_ms
        );
        assert_eq!(bad.tracker_numwant, d.tracker_numwant);
        assert_eq!(bad.peer_retry_base_secs, d.peer_retry_base_secs);
        assert_eq!(bad.peer_retry_cap_secs, d.peer_retry_cap_secs);
        assert_eq!(bad.tracker_backoff_start_secs, d.tracker_backoff_start_secs);
        assert_eq!(bad.tracker_backoff_cap_secs, d.tracker_backoff_cap_secs);
        assert_eq!(bad.udp_base_timeout_secs, d.udp_base_timeout_secs);
        assert_eq!(bad.udp_retries, d.udp_retries);
        assert_eq!(bad.seed_duration_secs, d.seed_duration_secs);
        let ok = Config::from_toml(
            "[bt]\nbt_concurrency = 8\npeer_pipeline = 32\nupload_slots = 2\nupload_read_concurrency = 64\nidle_eviction_secs = 30\nseed_duration_secs = 60\n",
        );
        assert_eq!(ok.bt_concurrency, 8);
        assert_eq!(ok.peer_pipeline, 32);
        assert_eq!(ok.upload_slots, 2);
        assert_eq!(ok.upload_read_concurrency, 64);
        assert_eq!(ok.idle_eviction_secs, 30);
        assert_eq!(ok.seed_duration_secs, 60.0);
        let clamped = Config::from_toml("[bt]\nbt_concurrency = 99\n");
        assert_eq!(clamped.bt_concurrency, 64, "越界钳制 1–64");
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
    fn expand_tilde_with_maps_home_prefix() {
        let home = Path::new("/home/u");
        assert_eq!(expand_tilde_with("~", Some(home)), PathBuf::from("/home/u"));
        assert_eq!(
            expand_tilde_with("~/dl/a.torrent", Some(home)),
            PathBuf::from("/home/u/dl/a.torrent")
        );
        assert_eq!(
            expand_tilde_with("  ~/x.torrent  ", Some(home)),
            PathBuf::from("/home/u/x.torrent"),
            "首尾空白先修剪再展开"
        );
    }

    #[test]
    fn expand_tilde_with_falls_back_to_literal() {
        assert_eq!(
            expand_tilde_with("~/a.torrent", None),
            PathBuf::from("~/a.torrent"),
            "无主目录 → 原样字面路径"
        );
        assert_eq!(
            expand_tilde_with("~x/a", Some(Path::new("/h"))),
            PathBuf::from("~x/a"),
            "~user 形式不展开，按字面路径"
        );
        assert_eq!(
            expand_tilde_with("/abs/a.torrent", Some(Path::new("/h"))),
            PathBuf::from("/abs/a.torrent")
        );
        assert_eq!(
            expand_tilde_with("./rel/a.torrent", Some(Path::new("/h"))),
            PathBuf::from("./rel/a.torrent")
        );
    }

    #[test]
    fn expand_tilde_passthrough_absolute() {
        assert_eq!(
            expand_tilde("/abs/a.torrent"),
            PathBuf::from("/abs/a.torrent"),
            "绝对路径不受进程环境影响"
        );
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

    #[test]
    fn template_line_count_is_168() {
        assert_eq!(
            Config::default_template().lines().count(),
            168,
            "钦定模板 v1.11 = 168 行"
        );
        assert!(Config::default_template().ends_with('\n'));
        assert!(!Config::default_template().ends_with("\n\n"), "EOF 单换行收尾");
    }

    #[test]
    fn template_three_table_headers() {
        let t = Config::default_template();
        assert!(t.contains("# ===== [common] 公共配置（两引擎共用） ====="));
        assert!(t.contains("# ===== [http] HTTP 引擎配置 ====="));
        assert!(t.contains("# ===== [bt] BT 引擎配置 ====="));
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
    fn template_36_keys_three_line_form() {
        let t = Config::default_template();
        let keys = [
            "download_dir", "download_slots", "max_speed",
            "block_size_http", "http_concurrency", "max_retries", "auto_retry",
            "backoff_initial", "backoff_cap", "progress_tick_ms", "sidecar_flush_secs",
            "throttle_burst_secs", "throttle_sleep_ms", "dns_race_grace_ms",
            "http_trackers", "udp_trackers", "bt_concurrency", "peer_pipeline",
            "upload_slots", "upload_read_concurrency", "idle_eviction_secs",
            "connect_timeout_secs", "handshake_timeout_secs", "request_timeout_secs",
            "requests_per_piece", "rechoke_interval_secs", "optimistic_interval_secs",
            "progress_emit_min_interval_ms", "tracker_numwant", "peer_retry_base_secs",
            "peer_retry_cap_secs", "tracker_backoff_start_secs", "tracker_backoff_cap_secs",
            "udp_base_timeout_secs", "udp_retries", "seed_duration_secs",
        ];
        assert_eq!(keys.len(), 36);
        for k in keys {
            assert!(t.contains(&format!("\n# {k} = ")), "模板缺键 {k} 的示例行");
            assert!(t.matches(&format!("\n# {k} = ")).count() == 1, "键 {k} 示例行不唯一");
        }
        for k in keys {
            if k == "download_dir" {
                continue;
            }
            let key_line = t
                .lines()
                .find(|l| l.starts_with(&format!("# {k} = ")))
                .unwrap();
            let name_line_idx = t.lines().position(|l| l == key_line).unwrap();
            let lines: Vec<&str> = t.lines().collect();
            let purpose = lines[name_line_idx - 2];
            assert_eq!(
                purpose.split('：').next().unwrap().trim_start_matches("# "),
                k,
                "键 {k} 的用途行首必须是键名三行式"
            );
        }
        assert_eq!(t.matches("取值范围").count(), 36, "36 键各一段取值范围标注");
    }

    #[test]
    fn template_defaults_match_contract() {
        let t = Config::default_template();
        for expected in [
            "# download_dir = \"\"",
            "# download_slots = 5",
            "# max_speed = 0",
            "# block_size_http = 1048576",
            "# http_concurrency = 4",
            "# max_retries = 5",
            "# auto_retry = true",
            "# backoff_initial = 8.0",
            "# backoff_cap = 60.0",
            "# progress_tick_ms = 100",
            "# sidecar_flush_secs = 2",
            "# throttle_burst_secs = 0.5",
            "# throttle_sleep_ms = 20",
            "# dns_race_grace_ms = 1000",
            "# http_trackers = []",
            "# udp_trackers = []",
            "# bt_concurrency = 20",
            "# peer_pipeline = 64",
            "# upload_slots = 4",
            "# upload_read_concurrency = 128",
            "# idle_eviction_secs = 60",
            "# connect_timeout_secs = 3",
            "# handshake_timeout_secs = 10",
            "# request_timeout_secs = 20",
            "# requests_per_piece = 4",
            "# rechoke_interval_secs = 10",
            "# optimistic_interval_secs = 30",
            "# progress_emit_min_interval_ms = 250",
            "# tracker_numwant = 200",
            "# peer_retry_base_secs = 5",
            "# peer_retry_cap_secs = 120",
            "# tracker_backoff_start_secs = 8",
            "# tracker_backoff_cap_secs = 60",
            "# udp_base_timeout_secs = 15",
            "# udp_retries = 3",
            "# seed_duration_secs = 1800",
        ] {
            assert!(t.contains(expected), "模板缺默认值行：{expected}");
        }
        assert!(
            !t.contains("# proxies"),
            "proxies 键已改名 proxy 并迁入 [common]，模板不得输出旧键名"
        );
    }

    #[test]
    fn template_proxy_dual_examples_in_common() {
        let t = Config::default_template();
        assert!(
            t.contains("# proxy：命名代理列表（可配置多个）"),
            "proxy 段一句头注释在位"
        );
        assert_eq!(t.matches("# [[common.proxy]]").count(), 2, "双示例块");
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
        assert!(!t.contains("# url ="), "url 键退役，模板不得输出 url 行");
        assert!(!t.contains("凭证"), "凭证规则注释行退出模板");
        assert!(!t.contains("唯一事实来源"), "type 语义注释退出模板");
    }

    #[test]
    fn template_example_values_match_code_defaults() {
        let c = Config::default();
        let t = Config::default_template();
        let val_of = |key: &str| -> String {
            t.lines()
                .find(|l| l.starts_with(&format!("# {key} = ")))
                .unwrap_or_else(|| panic!("模板缺 {key}"))
                .split_once(" = ")
                .unwrap()
                .1
                .to_string()
        };
        assert_eq!(val_of("download_slots"), c.download_slots.to_string());
        assert_eq!(val_of("block_size_http"), c.block_size_http.to_string());
        assert_eq!(val_of("http_concurrency"), c.http_concurrency.to_string());
        assert_eq!(val_of("max_retries"), c.max_retries.to_string());
        assert_eq!(val_of("auto_retry"), c.auto_retry.to_string());
        assert_eq!(val_of("progress_tick_ms"), c.progress_tick_ms.to_string());
        assert_eq!(val_of("sidecar_flush_secs"), c.sidecar_flush_secs.to_string());
        assert_eq!(val_of("throttle_burst_secs"), "0.5");
        assert_eq!(val_of("throttle_sleep_ms"), c.throttle_sleep_ms.to_string());
        assert_eq!(val_of("dns_race_grace_ms"), c.dns_race_grace_ms.to_string());
        assert_eq!(val_of("bt_concurrency"), c.bt_concurrency.to_string());
        assert_eq!(val_of("peer_pipeline"), c.peer_pipeline.to_string());
        assert_eq!(val_of("upload_slots"), c.upload_slots.to_string());
        assert_eq!(
            val_of("upload_read_concurrency"),
            c.upload_read_concurrency.to_string()
        );
        assert_eq!(val_of("idle_eviction_secs"), c.idle_eviction_secs.to_string());
        assert_eq!(val_of("connect_timeout_secs"), c.connect_timeout_secs.to_string());
        assert_eq!(
            val_of("handshake_timeout_secs"),
            c.handshake_timeout_secs.to_string()
        );
        assert_eq!(val_of("request_timeout_secs"), c.request_timeout_secs.to_string());
        assert_eq!(val_of("requests_per_piece"), c.requests_per_piece.to_string());
        assert_eq!(val_of("rechoke_interval_secs"), c.rechoke_interval_secs.to_string());
        assert_eq!(
            val_of("optimistic_interval_secs"),
            c.optimistic_interval_secs.to_string()
        );
        assert_eq!(
            val_of("progress_emit_min_interval_ms"),
            c.progress_emit_min_interval_ms.to_string()
        );
        assert_eq!(val_of("tracker_numwant"), c.tracker_numwant.to_string());
        assert_eq!(val_of("peer_retry_base_secs"), c.peer_retry_base_secs.to_string());
        assert_eq!(val_of("peer_retry_cap_secs"), c.peer_retry_cap_secs.to_string());
        assert_eq!(
            val_of("tracker_backoff_start_secs"),
            c.tracker_backoff_start_secs.to_string()
        );
        assert_eq!(
            val_of("tracker_backoff_cap_secs"),
            c.tracker_backoff_cap_secs.to_string()
        );
        assert_eq!(val_of("udp_base_timeout_secs"), c.udp_base_timeout_secs.to_string());
        assert_eq!(val_of("udp_retries"), c.udp_retries.to_string());
        assert_eq!(val_of("seed_duration_secs"), "1800");
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
            "[[common.proxy]]\nname = \"a\"\ntype = \"http\"\nip = \"127.0.0.1\"\nport = 8080\nusername = \"u\"\npassword = \"p\"\n[[common.proxy]]\nname = \"b\"\ntype = \"socks5\"\nip = \"127.0.0.1\"\nport = 1080\nusername = \"u2\"\npassword = \"p2\"\n",
        );
        assert_eq!(c.proxies.len(), 2);
        assert_eq!(c.proxies[0].name, "a");
        assert_eq!(c.proxies[0].kind, ProxyKind::Http);
        assert_eq!(c.proxies[0].port, 8080, "port 以整数落位（u16）");
        assert_eq!(c.proxies[1].kind, ProxyKind::Socks5);
        assert!(c.warnings.is_empty());
    }

    #[test]
    fn proxy_type_three_values_explicit() {
        let c = Config::from_toml(
            "[[common.proxy]]\nname = \"h\"\ntype = \"http\"\nip = \"x\"\nport = 1\n[[common.proxy]]\nname = \"hs\"\ntype = \"HTTPS\"\nip = \"x\"\nport = 2\n[[common.proxy]]\nname = \"s5\"\ntype = \"socks5\"\nip = \"x\"\nport = 3\nusername = \"u\"\npassword = \"p\"\n",
        );
        assert_eq!(c.proxies.len(), 3);
        assert_eq!(c.proxies[0].kind, ProxyKind::Http);
        assert_eq!(c.proxies[1].kind, ProxyKind::Https, "type 大小写不敏感三值");
        assert_eq!(c.proxies[2].kind, ProxyKind::Socks5);
        assert!(c.warnings.is_empty());
    }

    #[test]
    fn proxy_type_missing_or_unknown_invalidated() {
        let c = Config::from_toml(
            "[[common.proxy]]\nname = \"no-type\"\nip = \"127.0.0.1\"\nport = 18766\n[[common.proxy]]\nname = \"badtype\"\ntype = \"ftp\"\nip = \"x\"\nport = 5\n[[common.proxy]]\nname = \"valid\"\ntype = \"http\"\nip = \"x\"\nport = 9\n",
        );
        assert_eq!(c.proxies.len(), 1, "type 缺失/未知即作废，仅剩合法条目");
        assert_eq!(c.proxies[0].name, "valid");
        assert_eq!(c.warnings.len(), 2);
        assert!(c.warnings[0].contains("type 缺失"));
        assert!(c.warnings[1].contains("type 未知"));
        assert!(c.warnings.iter().all(|w| w.contains("http / https / socks5")));
    }

    #[test]
    fn proxy_ip_port_validation_kept() {
        let c = Config::from_toml(
            "[[common.proxy]]\nname = \"bad-port\"\ntype = \"http\"\nip = \"127.0.0.1\"\nport = 65536\n[[common.proxy]]\nname = \"bad-ip\"\ntype = \"http\"\nport = 1\n[[common.proxy]]\nname = \"valid\"\ntype = \"http\"\nip = \"x\"\nport = 9\n",
        );
        assert_eq!(c.proxies.len(), 1);
        assert_eq!(c.proxies[0].name, "valid");
        assert_eq!(c.warnings.len(), 2);
        assert!(c.warnings[0].contains("1..=65535"));
        assert!(c.warnings[1].contains("ip 缺失或空白"));
    }

    #[test]
    fn socks5_and_http_credential_rules_kept() {
        let c = Config::from_toml(
            "[[common.proxy]]\nname = \"s5-no-user\"\ntype = \"socks5\"\nip = \"127.0.0.1\"\nport = 18768\npassword = \"p1\"\n[[common.proxy]]\nname = \"half-http\"\ntype = \"http\"\nip = \"x\"\nport = 9\nusername = \"u1\"\n[[common.proxy]]\nname = \"anon-http\"\ntype = \"http\"\nip = \"x\"\nport = 10\n",
        );
        assert_eq!(c.proxies.len(), 1);
        assert_eq!(c.proxies[0].name, "anon-http", "匿名 http 合法，socks5 缺凭证与 http 半凭证作废");
        assert_eq!(c.warnings.len(), 2);
        assert!(c.warnings.iter().all(|w| !w.contains("p1") && !w.contains("u1")));
    }

    #[test]
    fn duplicate_names_keep_first() {
        let c = Config::from_toml(
            "[[common.proxy]]\nname = \"dup\"\ntype = \"http\"\nip = \"x\"\nport = 2\n[[common.proxy]]\nname = \"dup\"\ntype = \"http\"\nip = \"x\"\nport = 3\n",
        );
        assert_eq!(c.proxies.len(), 1);
        assert_eq!(c.proxies[0].port, 2, "重名条目保留首个");
    }

    #[test]
    fn resolve_proxy_two_states() {
        let c = Config::from_toml(
            "[[common.proxy]]\nname = \"a\"\ntype = \"http\"\nip = \"a\"\nport = 2\nusername = \"u\"\npassword = \"pw\"\n",
        );
        assert_eq!(c.resolve_proxy(&ProxyChoice::Direct), (None, false));
        let (ep, missing) = c.resolve_proxy(&ProxyChoice::Named("a".into()));
        let ep = ep.unwrap();
        assert_eq!(
            (ep.url.as_str(), ep.username.as_deref()),
            ("http://a:2", Some("u"))
        );
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
        std::fs::write(&p, "[common]\ndownload_slots = 2\n").unwrap();
        ensure_default_config(&p).unwrap();
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            "[common]\ndownload_slots = 2\n",
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
