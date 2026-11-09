use std::collections::HashSet;
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
pub const DEFAULT_PEER_CONNECTION_PROTOCOL: [&str; 2] = ["tcp", "webrtc"];

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
    pub global_max_download_speed: u64,
    pub global_max_upload_speed: u64,
    pub task_max_download_speed: u64,
    pub task_max_upload_speed: u64,
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
    pub trackers: Vec<String>,
    pub peer_connection_protocol: Vec<String>,
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
    pub bt_listen_port: u16,
    pub warnings: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            download_dir: None,
            download_slots: DEFAULT_DOWNLOAD_SLOTS,
            global_max_download_speed: 0,
            global_max_upload_speed: 0,
            task_max_download_speed: 0,
            task_max_upload_speed: 0,
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
            trackers: Vec::new(),
            peer_connection_protocol: DEFAULT_PEER_CONNECTION_PROTOCOL
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
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
            bt_listen_port: 0,
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
    download_slots: Option<toml::Value>,
    global_max_download_speed: Option<toml::Value>,
    global_max_upload_speed: Option<toml::Value>,
    task_max_download_speed: Option<toml::Value>,
    task_max_upload_speed: Option<toml::Value>,
    proxy: Option<Vec<ProxyRaw>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct HttpRaw {
    block_size_http: Option<toml::Value>,
    http_concurrency: Option<toml::Value>,
    max_retries: Option<toml::Value>,
    auto_retry: Option<toml::Value>,
    backoff_initial: Option<toml::Value>,
    backoff_cap: Option<toml::Value>,
    progress_tick_ms: Option<toml::Value>,
    sidecar_flush_secs: Option<toml::Value>,
    throttle_burst_secs: Option<toml::Value>,
    throttle_sleep_ms: Option<toml::Value>,
    dns_race_grace_ms: Option<toml::Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct BtRaw {
    trackers: Option<toml::Value>,
    peer_connection_protocol: Option<toml::Value>,
    bt_concurrency: Option<toml::Value>,
    peer_pipeline: Option<toml::Value>,
    upload_slots: Option<toml::Value>,
    upload_read_concurrency: Option<toml::Value>,
    idle_eviction_secs: Option<toml::Value>,
    connect_timeout_secs: Option<toml::Value>,
    handshake_timeout_secs: Option<toml::Value>,
    request_timeout_secs: Option<toml::Value>,
    requests_per_piece: Option<toml::Value>,
    rechoke_interval_secs: Option<toml::Value>,
    optimistic_interval_secs: Option<toml::Value>,
    progress_emit_min_interval_ms: Option<toml::Value>,
    tracker_numwant: Option<toml::Value>,
    peer_retry_base_secs: Option<toml::Value>,
    peer_retry_cap_secs: Option<toml::Value>,
    tracker_backoff_start_secs: Option<toml::Value>,
    tracker_backoff_cap_secs: Option<toml::Value>,
    udp_base_timeout_secs: Option<toml::Value>,
    udp_retries: Option<toml::Value>,
    seed_duration_secs: Option<toml::Value>,
    listen_port: Option<toml::Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct ConfigRaw {
    common: Option<CommonRaw>,
    http: Option<HttpRaw>,
    bt: Option<BtRaw>,
}

fn int_of(raw: Option<&toml::Value>) -> Result<Option<i64>, String> {
    match raw {
        None => Ok(None),
        Some(toml::Value::Integer(i)) => Ok(Some(*i)),
        Some(v) => Err(format!("应为整数，实际 {v}")),
    }
}

fn min1_val(raw: Option<&toml::Value>, key: &str, default: u64) -> Result<u64, String> {
    let v = int_of(raw).map_err(|e| format!("键 {key}：{e}（拒绝启动）"))?;
    match v {
        None => Ok(default),
        Some(i) if i >= 1 => Ok(i as u64),
        Some(i) => Err(format!("键 {key}：{i}（应为 ≥1 的整数，拒绝启动）")),
    }
}

fn min1_usize_val(raw: Option<&toml::Value>, key: &str, default: usize) -> Result<usize, String> {
    min1_val(raw, key, default as u64).map(|v| v as usize)
}

fn min1_u32_val(raw: Option<&toml::Value>, key: &str, default: u32) -> Result<u32, String> {
    let v = int_of(raw).map_err(|e| format!("键 {key}：{e}（拒绝启动）"))?;
    match v {
        None => Ok(default),
        Some(i) if i >= 1 && i <= i64::from(u32::MAX) => Ok(i as u32),
        Some(i) => Err(format!(
            "键 {key}：{i}（应为 ≥1 且 ≤ {} 的整数，拒绝启动）",
            u32::MAX
        )),
    }
}

fn range1_64_val(raw: Option<&toml::Value>, key: &str, default: usize) -> Result<usize, String> {
    let v = int_of(raw).map_err(|e| format!("键 {key}：{e}（拒绝启动）"))?;
    match v {
        None => Ok(default),
        Some(i) if (1..=64).contains(&i) => Ok(i as usize),
        Some(i) => Err(format!("键 {key}：{i}（应为 1–64 的整数，拒绝启动）")),
    }
}

fn pos_f64_val(raw: Option<&toml::Value>, key: &str, default: f64) -> Result<f64, String> {
    match raw {
        None => Ok(default),
        Some(toml::Value::Float(f)) if *f > 0.0 && f.is_finite() => Ok(*f),
        Some(toml::Value::Integer(i)) if *i > 0 => Ok(*i as f64),
        Some(v) => Err(format!("键 {key}：{v}（应为 >0 的有限数，拒绝启动）")),
    }
}

fn speed_val(raw: Option<&toml::Value>, key: &str) -> Result<u64, String> {
    match raw {
        None => Ok(0),
        Some(toml::Value::Integer(i)) if *i >= 0 => Ok(*i as u64),
        Some(toml::Value::Integer(i)) => {
            Err(format!("键 {key}：{i}（应为 ≥0 的限速值，拒绝启动）"))
        }
        Some(toml::Value::String(s)) => parse_speed_strict(s).ok_or_else(|| {
            format!(
                "键 {key}：{s:?}（无法识别的限速写法，应为 KiB/MiB/GiB 二进制或整数 B/s，拒绝启动）"
            )
        }),
        Some(v) => Err(format!(
            "键 {key}：{v}（无法识别的限速写法，应为 KiB/MiB/GiB 二进制或整数 B/s，拒绝启动）"
        )),
    }
}

fn bool_val(raw: Option<&toml::Value>, key: &str, default: bool) -> Result<bool, String> {
    match raw {
        None => Ok(default),
        Some(toml::Value::Boolean(b)) => Ok(*b),
        Some(v) => Err(format!("键 {key}：{v}（应为 true / false，拒绝启动）")),
    }
}

fn listen_port_val(raw: Option<&toml::Value>) -> u16 {
    match raw {
        Some(toml::Value::Integer(n)) => u16::try_from(*n).ok().unwrap_or(0),
        _ => 0,
    }
}

fn str_list_val(raw: Option<&toml::Value>, key: &str) -> Result<Option<Vec<String>>, String> {
    match raw {
        None => Ok(None),
        Some(toml::Value::Array(a)) => {
            let mut out = Vec::with_capacity(a.len());
            for (i, v) in a.iter().enumerate() {
                match v.as_str() {
                    Some(s) => out.push(s.to_string()),
                    None => {
                        return Err(format!(
                            "键 {key}：第 {} 项 {v}（应为字符串，拒绝启动）",
                            i + 1
                        ))
                    }
                }
            }
            Ok(Some(out))
        }
        Some(v) => Err(format!("键 {key}：{v}（应为字符串数组，拒绝启动）")),
    }
}

fn normalize_tracker_for_dedup(s: &str) -> String {
    match s.find("://") {
        Some(i) => s[..i].to_ascii_lowercase() + &s[i..],
        None => s.to_string(),
    }
}

fn trackers_val(raw: Option<&toml::Value>, key: &str) -> Result<Vec<String>, String> {
    let Some(entries) = str_list_val(raw, key)? else {
        return Ok(Vec::new());
    };
    let mut out: Vec<String> = Vec::with_capacity(entries.len());
    let mut seen: HashSet<String> = HashSet::new();
    for e in entries {
        let t = e.trim();
        if crate::bt::tracker::TrackerUrl::parse(t).is_none() {
            return Err(format!(
                "键 {key}：{t:?}（未知 tracker scheme，应为 http(s)://、ws(s):// 或 udp://，拒绝启动）"
            ));
        }
        let norm = normalize_tracker_for_dedup(t);
        if !seen.insert(norm) {
            return Err(format!("键 {key}：{t:?}（重复项，拒绝启动）"));
        }
        out.push(t.to_string());
    }
    Ok(out)
}

fn peer_connection_protocol_val(
    raw: Option<&toml::Value>,
    key: &str,
) -> Result<Vec<String>, String> {
    let Some(entries) = str_list_val(raw, key)? else {
        return Ok(DEFAULT_PEER_CONNECTION_PROTOCOL
            .iter()
            .map(|s| (*s).to_string())
            .collect());
    };
    let mut out: Vec<String> = Vec::with_capacity(entries.len());
    let mut seen: HashSet<String> = HashSet::new();
    for e in &entries {
        let t = e.trim();
        let Some(norm) = crate::bt::webrtc::PeerTransport::parse(t) else {
            return Err(format!(
                "键 {key}：{t:?}（未知值，应为 tcp / webrtc，拒绝启动）"
            ));
        };
        let norm = match norm {
            crate::bt::webrtc::PeerTransport::Tcp => "tcp".to_string(),
            crate::bt::webrtc::PeerTransport::WebRtc => "webrtc".to_string(),
        };
        if !seen.insert(norm.clone()) {
            return Err(format!("键 {key}：{t:?}（重复项，拒绝启动）"));
        }
        out.push(norm);
    }
    if out.is_empty() {
        return Err(format!(
            "键 {key}：[]（应为去重后 1–2 项的 tcp / webrtc，拒绝启动）"
        ));
    }
    Ok(out)
}

impl Config {
    pub fn try_load(path: &Path) -> Result<Self, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => return Ok(Config::default()),
        };
        match Self::try_from_toml(&text) {
            Ok(c) => Ok(c),
            Err(e) if e.starts_with("解析失败") => {
                Err(format!("配置文件解析失败：{}\n{e}", path.display()))
            }
            Err(e) => Err(format!("配置文件 {} 非法：{e}", path.display())),
        }
    }

    #[cfg(test)]
    pub fn from_toml(text: &str) -> Self {
        Self::try_from_toml(text).unwrap_or_default()
    }

    pub fn try_from_toml(text: &str) -> Result<Self, String> {
        let raw: ConfigRaw = toml::from_str(text).map_err(|e| format!("解析失败：{e}"))?;
        let d = Config::default();
        let mut warnings = Vec::new();
        let mut proxies = Vec::new();
        let common_proxy = raw
            .common
            .as_ref()
            .and_then(|c| c.proxy.clone())
            .unwrap_or_default();
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
        let download_slots = min1_usize_val(
            common.download_slots.as_ref(),
            "common.download_slots",
            d.download_slots,
        )?;
        let global_max_download_speed = speed_val(
            common.global_max_download_speed.as_ref(),
            "common.global_max_download_speed",
        )?;
        let global_max_upload_speed = speed_val(
            common.global_max_upload_speed.as_ref(),
            "common.global_max_upload_speed",
        )?;
        let task_max_download_speed = speed_val(
            common.task_max_download_speed.as_ref(),
            "common.task_max_download_speed",
        )?;
        let task_max_upload_speed = speed_val(
            common.task_max_upload_speed.as_ref(),
            "common.task_max_upload_speed",
        )?;
        let block_size_http = min1_val(
            http.block_size_http.as_ref(),
            "http.block_size_http",
            d.block_size_http,
        )?
        .max(super::chunk::MIN_HTTP_BLOCK_SIZE);
        let http_concurrency = range1_64_val(
            http.http_concurrency.as_ref(),
            "http.http_concurrency",
            d.http_concurrency,
        )?;
        let max_retries =
            min1_u32_val(http.max_retries.as_ref(), "http.max_retries", d.max_retries)?;
        let auto_retry = bool_val(http.auto_retry.as_ref(), "http.auto_retry", d.auto_retry)?;
        let backoff_initial = pos_f64_val(
            http.backoff_initial.as_ref(),
            "http.backoff_initial",
            d.backoff_initial,
        )?;
        let backoff_cap =
            pos_f64_val(http.backoff_cap.as_ref(), "http.backoff_cap", d.backoff_cap)?;
        let progress_tick_ms = min1_val(
            http.progress_tick_ms.as_ref(),
            "http.progress_tick_ms",
            d.progress_tick_ms,
        )?;
        let sidecar_flush_secs = min1_val(
            http.sidecar_flush_secs.as_ref(),
            "http.sidecar_flush_secs",
            d.sidecar_flush_secs,
        )?;
        let throttle_burst_secs = pos_f64_val(
            http.throttle_burst_secs.as_ref(),
            "http.throttle_burst_secs",
            d.throttle_burst_secs,
        )?;
        let throttle_sleep_ms = min1_val(
            http.throttle_sleep_ms.as_ref(),
            "http.throttle_sleep_ms",
            d.throttle_sleep_ms,
        )?;
        let dns_race_grace_ms = min1_val(
            http.dns_race_grace_ms.as_ref(),
            "http.dns_race_grace_ms",
            d.dns_race_grace_ms,
        )?;
        let trackers = trackers_val(bt.trackers.as_ref(), "bt.trackers")?;
        let peer_connection_protocol = peer_connection_protocol_val(
            bt.peer_connection_protocol.as_ref(),
            "bt.peer_connection_protocol",
        )?;
        let bt_concurrency = range1_64_val(
            bt.bt_concurrency.as_ref(),
            "bt.bt_concurrency",
            d.bt_concurrency,
        )?;
        let peer_pipeline = min1_usize_val(
            bt.peer_pipeline.as_ref(),
            "bt.peer_pipeline",
            d.peer_pipeline,
        )?;
        let upload_slots =
            min1_usize_val(bt.upload_slots.as_ref(), "bt.upload_slots", d.upload_slots)?;
        let upload_read_concurrency = min1_usize_val(
            bt.upload_read_concurrency.as_ref(),
            "bt.upload_read_concurrency",
            d.upload_read_concurrency,
        )?;
        let idle_eviction_secs = min1_val(
            bt.idle_eviction_secs.as_ref(),
            "bt.idle_eviction_secs",
            d.idle_eviction_secs,
        )?;
        let connect_timeout_secs = min1_val(
            bt.connect_timeout_secs.as_ref(),
            "bt.connect_timeout_secs",
            d.connect_timeout_secs,
        )?;
        let handshake_timeout_secs = min1_val(
            bt.handshake_timeout_secs.as_ref(),
            "bt.handshake_timeout_secs",
            d.handshake_timeout_secs,
        )?;
        let request_timeout_secs = min1_val(
            bt.request_timeout_secs.as_ref(),
            "bt.request_timeout_secs",
            d.request_timeout_secs,
        )?;
        let requests_per_piece = min1_usize_val(
            bt.requests_per_piece.as_ref(),
            "bt.requests_per_piece",
            d.requests_per_piece,
        )?;
        let rechoke_interval_secs = min1_val(
            bt.rechoke_interval_secs.as_ref(),
            "bt.rechoke_interval_secs",
            d.rechoke_interval_secs,
        )?;
        let optimistic_interval_secs = min1_val(
            bt.optimistic_interval_secs.as_ref(),
            "bt.optimistic_interval_secs",
            d.optimistic_interval_secs,
        )?;
        let progress_emit_min_interval_ms = min1_val(
            bt.progress_emit_min_interval_ms.as_ref(),
            "bt.progress_emit_min_interval_ms",
            d.progress_emit_min_interval_ms,
        )?;
        let tracker_numwant = min1_u32_val(
            bt.tracker_numwant.as_ref(),
            "bt.tracker_numwant",
            d.tracker_numwant,
        )?;
        let peer_retry_base_secs = min1_val(
            bt.peer_retry_base_secs.as_ref(),
            "bt.peer_retry_base_secs",
            d.peer_retry_base_secs,
        )?;
        let peer_retry_cap_secs = min1_val(
            bt.peer_retry_cap_secs.as_ref(),
            "bt.peer_retry_cap_secs",
            d.peer_retry_cap_secs,
        )?;
        let tracker_backoff_start_secs = min1_val(
            bt.tracker_backoff_start_secs.as_ref(),
            "bt.tracker_backoff_start_secs",
            d.tracker_backoff_start_secs,
        )?;
        let tracker_backoff_cap_secs = min1_val(
            bt.tracker_backoff_cap_secs.as_ref(),
            "bt.tracker_backoff_cap_secs",
            d.tracker_backoff_cap_secs,
        )?;
        let udp_base_timeout_secs = min1_val(
            bt.udp_base_timeout_secs.as_ref(),
            "bt.udp_base_timeout_secs",
            d.udp_base_timeout_secs,
        )?;
        let udp_retries = min1_u32_val(bt.udp_retries.as_ref(), "bt.udp_retries", d.udp_retries)?;
        let seed_duration_secs = pos_f64_val(
            bt.seed_duration_secs.as_ref(),
            "bt.seed_duration_secs",
            d.seed_duration_secs,
        )?;
        let bt_listen_port = listen_port_val(bt.listen_port.as_ref());
        Ok(Config {
            download_dir: common.download_dir.filter(|s| !s.trim().is_empty()),
            download_slots,
            global_max_download_speed,
            global_max_upload_speed,
            task_max_download_speed,
            task_max_upload_speed,
            proxies,
            block_size_http,
            http_concurrency,
            max_retries,
            auto_retry,
            backoff_initial,
            backoff_cap,
            progress_tick_ms,
            sidecar_flush_secs,
            throttle_burst_secs,
            throttle_sleep_ms,
            dns_race_grace_ms,
            trackers,
            peer_connection_protocol,
            bt_concurrency,
            peer_pipeline,
            upload_slots,
            upload_read_concurrency,
            idle_eviction_secs,
            connect_timeout_secs,
            handshake_timeout_secs,
            request_timeout_secs,
            requests_per_piece,
            rechoke_interval_secs,
            optimistic_interval_secs,
            progress_emit_min_interval_ms,
            tracker_numwant,
            peer_retry_base_secs,
            peer_retry_cap_secs,
            tracker_backoff_start_secs,
            tracker_backoff_cap_secs,
            udp_base_timeout_secs,
            udp_retries,
            seed_duration_secs,
            bt_listen_port,
            warnings,
        })
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

const SPEED_UNITS: [(&str, f64); 4] = [
    ("gib", 1_073_741_824.0),
    ("mib", 1_048_576.0),
    ("kib", 1_024.0),
    ("b", 1.0),
];

#[must_use]
pub fn parse_speed_strict(s: &str) -> Option<u64> {
    let t = s.trim().to_lowercase();
    let t = t.strip_suffix("/s").unwrap_or(&t).trim();
    if let Ok(v) = t.parse::<u64>() {
        return Some(v);
    }
    let (num_part, mult) = SPEED_UNITS
        .iter()
        .find_map(|(suffix, m)| t.strip_suffix(suffix).map(|n| (n.trim(), *m)))?;
    if num_part.is_empty() {
        return None;
    }
    let v = num_part.parse::<f64>().ok()?;
    if v < 0.0 || !v.is_finite() {
        return None;
    }
    Some((v * mult) as u64)
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
pub fn normalize_save_dir_with(path: &str, home: Option<&Path>, cwd: Option<&Path>) -> String {
    let p = path.trim();
    let p = if p.len() > 1 {
        p.trim_end_matches('/')
    } else {
        p
    };
    let expanded = expand_tilde_with(p, home);
    let expanded = if expanded.is_absolute() {
        expanded
    } else {
        match cwd {
            Some(c) => c.join(&expanded),
            None => expanded,
        }
    };
    expanded.to_string_lossy().to_string()
}

#[must_use]
pub fn normalize_save_dir(path: &str) -> String {
    normalize_save_dir_with(
        path,
        home_dir().as_deref(),
        std::env::current_dir().ok().as_deref(),
    )
}

#[must_use]
pub fn default_save_dir_prefill(cfg: &Config) -> String {
    cfg.download_dir
        .clone()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(default_download_dir)
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

# ===== 公共配置 =====

[common]

# 默认保存目录（添加对话框目录留空时使用）
# 取值范围：任意目录路径；留空或缺失 = 用户主目录下的下载目录
download_dir = ""

# 全局下载槽位数（同时下载的任务数上限）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
download_slots = 5

# 全局下载限速（全部任务合计；0 = 不限）
# 取值范围：≥0；支持 "2 MiB/s" / "500 KiB/s" / 整数 B/s
global_max_download_speed = 0

# 全局上传限速（BT 上传合计；0 = 不限）
# 取值范围：≥0；支持 "2 MiB/s" / "500 KiB/s" / 整数 B/s
global_max_upload_speed = 0

# 新任务默认任务级下载限速（0 = 不限）
# 取值范围：≥0；支持 "2 MiB/s" / "500 KiB/s" / 整数 B/s
task_max_download_speed = 0

# 新任务默认任务级上传限速（0 = 不限）
# 取值范围：≥0；支持 "2 MiB/s" / "500 KiB/s" / 整数 B/s
task_max_upload_speed = 0

# 命名代理列表（可配置多个）
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

# ===== HTTP 引擎配置 =====

[http]

# HTTP 分块块大小（字节）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动（1 MB）
block_size_http = 1048576

# 默认并发数
# 取值范围：1–64 的整数；0 或越界拒绝启动
http_concurrency = 4

# 自动重试上限次数（达上限转停等，可按 R 手动重试）
# 取值范围：≥1 的整数；0 或非法值拒绝启动
max_retries = 5

# 失败后是否自动重试
# 取值范围：true / false
auto_retry = true

# 自动重试退避初始秒（指数退避序列起点）
# 取值范围：>0 的有限数；非法值拒绝启动
backoff_initial = 8.0

# 自动重试退避封顶秒
# 取值范围：>0 的有限数；非法值拒绝启动
backoff_cap = 60.0

# HTTP 下载进度事件节拍（毫秒）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
progress_tick_ms = 100

# 断点 sidecar 落盘间隔（秒）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
sidecar_flush_secs = 2

# 限速令牌桶突发窗（秒）
# 取值范围：>0 的有限数；非法值拒绝启动
throttle_burst_secs = 0.5

# 限速循环步进睡眠（毫秒）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
throttle_sleep_ms = 20

# DNS 双腿竞速宽限（毫秒，双栈并行解析）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
dns_race_grace_ms = 1000

# ===== BT 引擎配置 =====

[bt]

# tracker 列表（BT 任务默认 tracker 池来源之一，三来源合并入池）
# 取值范围：字符串数组，每项为 http(s)://、ws(s):// 或 udp:// 开头的 tracker announce URL（scheme 大小写不敏感）；未知 scheme 或重复项拒绝启动
trackers = []

# BT peer 连接与数据传输的传输协议全局开关（无序数组；无跨协议回退）
# 取值范围：无序字符串数组，元素限 tcp / webrtc（大小写不敏感、去重）；未知值或重复项拒绝启动
peer_connection_protocol = ["tcp", "webrtc"]

# BT 任务默认并发数（新任务预填）
# 取值范围：1–64 的整数；0 或越界拒绝启动
bt_concurrency = 20

# 单 peer 在飞请求块数上限（×16 KiB = 在飞窗口）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
peer_pipeline = 64

# 同时获得上传槽位的 peer 数（choker 周期分配）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
upload_slots = 4

# 上传读盘并发上限（进程级）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
upload_read_concurrency = 128

# 空闲连接置换阈值（秒，无新增进账超时即断开重拨）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
idle_eviction_secs = 60

# 出连 TCP 拨号超时（秒）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
connect_timeout_secs = 3

# peer 握手超时（秒）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
handshake_timeout_secs = 10

# 在飞块请求超时（秒）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
request_timeout_secs = 20

# 单 piece 请求块粒度（piece 细分为 n 块请求）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
requests_per_piece = 4

# 上传 choker 重排周期（秒）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
rechoke_interval_secs = 10

# 乐观 unchoke 轮换周期（秒）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
optimistic_interval_secs = 30

# 进度事件最小发送间隔（毫秒，节流）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
progress_emit_min_interval_ms = 250

# announce 请求的 peer 数上限
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
tracker_numwant = 200

# peer 连接失败重试退避起始（秒，倍增）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
peer_retry_base_secs = 5

# peer 连接失败重试退避封顶（秒）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
peer_retry_cap_secs = 120

# tracker 失败退避起始（秒，倍增）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
tracker_backoff_start_secs = 8

# tracker 失败退避封顶（秒）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
tracker_backoff_cap_secs = 60

# UDP tracker 单轮超时起始（秒，倍增重试）
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
udp_base_timeout_secs = 15

# UDP tracker 超时重试次数
# 取值范围：正整数（≥1）；0 或非法值拒绝启动
udp_retries = 3

# 做种时长默认值（秒，任务做种倒计时起点）
# 取值范围：>0 的数；0 或非法值拒绝启动
seed_duration_secs = 1800

# BT 监听端口（0 = 随机绑定）
# 取值范围：0 = 随机绑定；1–65535 = 指定端口（占用时回退随机 + 启动警告）；负值或非法值回退随机
listen_port = 0
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
        let c = Config::try_load(Path::new("/nonexistent/ezr/config.toml")).unwrap();
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
    fn invalid_values_fail_fast_with_key_path() {
        let cases = [
            ("[http]\nblock_size_http = 0\n", "http.block_size_http"),
            ("[http]\nmax_retries = 0\n", "http.max_retries"),
            ("[http]\nhttp_concurrency = 999\n", "http.http_concurrency"),
            ("[common]\ndownload_slots = 0\n", "common.download_slots"),
            ("[bt]\nbt_concurrency = 0\n", "bt.bt_concurrency"),
        ];
        for (text, key) in cases {
            let err = Config::try_from_toml(text).expect_err(&format!("{key} 应拒绝启动"));
            assert!(err.contains(key), "错误含键点路径 {key}：{err}");
            assert!(err.contains("拒绝启动"), "错误含拒绝启动语义：{err}");
        }
    }

    #[test]
    fn block_size_http_positive_below_floor_clamped() {
        let mk = |v: &str| Config::from_toml(&format!("[http]\nblock_size_http = {v}\n"));
        assert_eq!(mk("512").block_size_http, 1_048_576);
        assert_eq!(
            mk("1048575").block_size_http,
            1_048_576,
            "下限之下 1 字节也钳制"
        );
        assert_eq!(mk("1048576").block_size_http, 1_048_576, "恰在下限 = 原值");
        assert_eq!(mk("2097152").block_size_http, 2_097_152, "大于下限原值生效");
        let err0 = Config::try_from_toml("[http]\nblock_size_http = 0\n").expect_err("0 拒绝启动");
        assert!(err0.contains("http.block_size_http"));
        let errneg =
            Config::try_from_toml("[http]\nblock_size_http = -4\n").expect_err("负值拒绝启动");
        assert!(errneg.contains("http.block_size_http"));
        let errstr = Config::try_from_toml("[http]\nblock_size_http = \"256 KB\"\n")
            .expect_err("非法类型拒绝启动");
        assert!(errstr.contains("http.block_size_http"), "{errstr}");
    }

    #[test]
    fn legacy_flat_keys_ignored_silently() {
        let c = Config::from_toml(
            "download_slots = 2\nblock_size_http = 4194304\nhttp_concurrency = 8\nmax_speed = \"1 MB/s\"\ndefault_concurrency = 9\nproxy = \"http://127.0.0.1:8118\"\n",
        );
        assert_eq!(
            c,
            Config::default(),
            "旧格式顶层键全部按未知键忽略，全部默认"
        );
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
        let c = Config::from_toml("download_slots = 2\n[common]\ndownload_slots = 3\n");
        assert_eq!(c.download_slots, 3, "新表键生效且不受旧顶层键干扰");
        assert!(c.warnings.is_empty());
    }

    #[test]
    fn garbage_file_falls_back_all() {
        let c = Config::from_toml("not [valid toml ===");
        assert_eq!(c, Config::default());
    }

    #[test]
    fn garbage_parse_is_err_in_strict_path() {
        let err = Config::try_from_toml("not [valid toml ===")
            .expect_err("v1.13/D63①：非法 TOML 必须报错（解析失败致命）");
        assert!(!err.is_empty(), "错误信息含解析原因: {err}");
    }

    #[test]
    fn try_load_reports_path_and_reason() {
        let dir = std::env::temp_dir().join(format!("ezr-cfg-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.toml");
        std::fs::write(&p, "not [valid toml ===").unwrap();
        let err = Config::try_load(&p).expect_err("损坏配置必须报错");
        assert!(
            err.contains("配置文件解析失败"),
            "报错含解析失败语义: {err}"
        );
        assert!(
            err.contains(p.to_string_lossy().as_ref()),
            "报错含配置文件路径: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn template_v16_parses_to_defaults() {
        let c = Config::try_from_toml(&Config::default_template())
            .expect("v1.13 活跃模板必须可解析（整数/浮点键均合法）");
        assert_eq!(
            c,
            Config::default(),
            "示例值 = 代码默认 ⇒ 解析结果与文件缺失一致"
        );
        assert!(c.warnings.is_empty(), "活跃模板零警告");
        assert!(c.proxies.is_empty(), "proxy 示例块保持注释，未激活");
        assert_eq!(c.peer_connection_protocol, vec!["tcp", "webrtc"]);
        assert!(c.trackers.is_empty());
    }

    #[test]
    fn template_v16_range_comments_flipped_and_tracker_rows() {
        let t = Config::default_template();
        assert!(t.contains("# tracker 列表（BT 任务默认 tracker 池来源之一，三来源合并入池）"));
        assert!(t.contains("# 取值范围：字符串数组，每项为 http(s)://、ws(s):// 或 udp:// 开头的 tracker announce URL（scheme 大小写不敏感）；未知 scheme 或重复项拒绝启动"));
        assert!(t.contains("# BT peer 连接与数据传输的传输协议全局开关（无序数组；无跨协议回退）"));
        assert!(t.contains("# 取值范围：无序字符串数组，元素限 tcp / webrtc（大小写不敏感、去重）；未知值或重复项拒绝启动"));
        assert_eq!(t.matches("peer_connection_protocol").count(), 1);
        assert!(!t.contains("http_trackers"));
        assert!(!t.contains("udp_trackers"));
        assert!(
            !t.contains("回退默认"),
            "FR-02-35：取值范围注释全量翻转，无回退字样"
        );
        assert_eq!(
            t.matches("拒绝启动").count(),
            33,
            "翻转后的拒绝启动注释计数"
        );
        assert!(
            t.contains("占用时回退随机 + 启动警告"),
            "listen_port 保留回退语义"
        );
    }

    #[test]
    fn float_keys_accept_integer_form_per_key() {
        let c = Config::from_toml(
            "[http]\nthrottle_burst_secs = 1\nbackoff_initial = 2\n[bt]\nseed_duration_secs = 1800\n",
        );
        assert_eq!(c.throttle_burst_secs, 1.0, "整数写法逐键生效");
        assert_eq!(c.seed_duration_secs, 1800.0);
        assert_eq!(c.backoff_initial, 2.0);
        let err = Config::try_from_toml("[http]\nthrottle_burst_secs = 0\n")
            .expect_err("FR-02-35：0 不再回退默认，拒绝启动");
        assert!(err.contains("http.throttle_burst_secs"), "{err}");
    }

    #[test]
    fn speed_parsing() {
        assert_eq!(parse_speed("1048576"), 1_048_576);
        assert_eq!(parse_speed("garbage"), 0);
        assert_eq!(parse_speed("0"), 0);
    }

    #[test]
    fn speed_parsing_decimal_units_retired() {
        assert_eq!(
            parse_speed("2 MB/s"),
            0,
            "旧写法 KB/MB/GB 十进制废弃不再识别"
        );
        assert_eq!(
            parse_speed("500 KB/s"),
            0,
            "旧写法废弃 → 非法回退 0（FR-02-28 v1.11）"
        );
        assert_eq!(parse_speed("1.5 MB"), 0, "『/s』后缀可省同废弃");
        assert_eq!(parse_speed("2 gb"), 0);
    }

    #[test]
    fn speed_parsing_binary_units() {
        assert_eq!(
            parse_speed("2 MiB/s"),
            2_097_152,
            "二进制口径 1 MiB = 1048576 B/s"
        );
        assert_eq!(parse_speed("500 KiB/s"), 512_000);
        assert_eq!(parse_speed("1 GiB/s"), 1_073_741_824);
        assert_eq!(parse_speed("1.5 MiB"), 1_572_864, "『/s』后缀可省");
        assert_eq!(parse_speed("2 mib/s"), 2_097_152, "单位大小写不敏感");
        assert_eq!(parse_speed("512 b/s"), 512);
        assert_eq!(
            parse_speed("2 MB/s"),
            0,
            "旧写法 MB/s 十进制废弃不再识别（FR-02-28 v1.11）"
        );
    }

    #[test]
    fn speed_limit_keys_accept_string_and_integer() {
        let c = Config::from_toml("[common]\nglobal_max_download_speed = \"2 MiB/s\"\n");
        assert_eq!(c.global_max_download_speed, 2_097_152, "新写法二进制生效");
        let c2 = Config::from_toml("[common]\nglobal_max_upload_speed = 1048576\n");
        assert_eq!(
            c2.global_max_upload_speed, 1_048_576,
            "整数 B/s 形态直接生效"
        );
        let c3 = Config::from_toml("[common]\ntask_max_download_speed = \"500 KiB/s\"\n");
        assert_eq!(c3.task_max_download_speed, 512_000);
        let err = Config::try_from_toml("[common]\ntask_max_upload_speed = \"xyz\"\n")
            .expect_err("FR-02-35：不可解析限速拒绝启动");
        assert!(err.contains("common.task_max_upload_speed"), "{err}");
    }

    #[test]
    fn speed_limit_keys_decimal_old_style_fail_fast() {
        for (v, note) in [
            ("2 MB/s", "旧写法 MB/s"),
            ("500 KB/s", "旧写法 KB/s"),
            ("2 GB", "旧写法 GB"),
        ] {
            let err =
                Config::try_from_toml(&format!("[common]\nglobal_max_download_speed = \"{v}\"\n"))
                    .expect_err(&format!("{note}：FR-02-35 拒绝启动"));
            assert!(err.contains("common.global_max_download_speed"), "{err}");
            assert!(err.contains("拒绝启动"), "{err}");
        }
    }

    #[test]
    fn legacy_max_speed_key_retired_as_unknown() {
        let c = Config::from_toml("[common]\nmax_speed = \"1 MB/s\"\n");
        assert_eq!(
            c.global_max_download_speed, 0,
            "旧键按未知键静默忽略（01-throttle-proxy-14）"
        );
        assert!(c.warnings.is_empty(), "零警告口径");
    }

    #[test]
    fn listen_port_valid_range_and_fallback() {
        let c = Config::from_toml("[bt]\nlisten_port = 51413\n");
        assert_eq!(c.bt_listen_port, 51413);
        let c0 = Config::from_toml("[bt]\nlisten_port = 0\n");
        assert_eq!(c0.bt_listen_port, 0, "0 = 随机绑定");
        let cn = Config::from_toml("[bt]\nlisten_port = -5\n");
        assert_eq!(cn.bt_listen_port, 0, "负值回退随机");
        let cb = Config::from_toml("[bt]\nlisten_port = 70000\n");
        assert_eq!(cb.bt_listen_port, 0, "越界回退随机");
        let cs = Config::from_toml("[bt]\nlisten_port = \"abc\"\n");
        assert_eq!(cs.bt_listen_port, 0, "非法类型回退随机");
    }

    #[test]
    fn listen_port_negative_and_out_of_range_still_fallback_not_fail_fast() {
        let cn = Config::try_from_toml("[bt]\nlisten_port = -5\n")
            .expect("listen_port 不在 FR-02-35 翻转面");
        assert_eq!(cn.bt_listen_port, 0);
    }

    #[test]
    fn empty_dir_and_invalid_numbers_fail_fast() {
        let c = Config::from_toml("[common]\ndownload_dir = \"   \"\n");
        assert!(c.download_dir.is_none(), "空白目录弃用");
        let e1 =
            Config::try_from_toml("[http]\nbackoff_initial = -1.0\n").expect_err("负退避拒绝启动");
        assert!(e1.contains("http.backoff_initial"), "{e1}");
        let e2 = Config::try_from_toml("[http]\nbackoff_cap = 0.0\n").expect_err("0 封顶拒绝启动");
        assert!(e2.contains("http.backoff_cap"), "{e2}");
    }

    #[test]
    fn http_tunables_effective_and_invalid_fail_fast() {
        let ok = Config::from_toml(
            "[http]\nprogress_tick_ms = 250\nsidecar_flush_secs = 5\nthrottle_burst_secs = 1.5\nthrottle_sleep_ms = 40\ndns_race_grace_ms = 2000\n",
        );
        assert_eq!(ok.progress_tick_ms, 250);
        assert_eq!(ok.sidecar_flush_secs, 5);
        assert_eq!(ok.throttle_burst_secs, 1.5);
        assert_eq!(ok.throttle_sleep_ms, 40);
        assert_eq!(ok.dns_race_grace_ms, 2000);
        for (text, key) in [
            ("[http]\nprogress_tick_ms = 0\n", "http.progress_tick_ms"),
            (
                "[http]\nsidecar_flush_secs = -1\n",
                "http.sidecar_flush_secs",
            ),
            (
                "[http]\nthrottle_burst_secs = 0.0\n",
                "http.throttle_burst_secs",
            ),
            ("[http]\nthrottle_sleep_ms = 0\n", "http.throttle_sleep_ms"),
            ("[http]\ndns_race_grace_ms = 0\n", "http.dns_race_grace_ms"),
        ] {
            let err = Config::try_from_toml(text).expect_err(&format!("{key} 拒绝启动"));
            assert!(err.contains(key), "{err}");
        }
    }
    #[test]
    fn bt_tunables_effective_and_invalid_fail_fast() {
        let ok = Config::from_toml(
            "[bt]\nbt_concurrency = 8\npeer_pipeline = 32\nupload_slots = 2\nupload_read_concurrency = 64\nidle_eviction_secs = 30\nseed_duration_secs = 60\n",
        );
        assert_eq!(ok.bt_concurrency, 8);
        assert_eq!(ok.peer_pipeline, 32);
        assert_eq!(ok.upload_slots, 2);
        assert_eq!(ok.upload_read_concurrency, 64);
        assert_eq!(ok.idle_eviction_secs, 30);
        assert_eq!(ok.seed_duration_secs, 60.0);
        for (text, key) in [
            ("[bt]\nbt_concurrency = 0\n", "bt.bt_concurrency"),
            ("[bt]\nbt_concurrency = 99\n", "bt.bt_concurrency"),
            ("[bt]\npeer_pipeline = 0\n", "bt.peer_pipeline"),
            ("[bt]\nupload_slots = -1\n", "bt.upload_slots"),
            (
                "[bt]\nupload_read_concurrency = 0\n",
                "bt.upload_read_concurrency",
            ),
            ("[bt]\nidle_eviction_secs = 0\n", "bt.idle_eviction_secs"),
            (
                "[bt]\nconnect_timeout_secs = 0\n",
                "bt.connect_timeout_secs",
            ),
            (
                "[bt]\nhandshake_timeout_secs = 0\n",
                "bt.handshake_timeout_secs",
            ),
            (
                "[bt]\nrequest_timeout_secs = 0\n",
                "bt.request_timeout_secs",
            ),
            ("[bt]\nrequests_per_piece = 0\n", "bt.requests_per_piece"),
            (
                "[bt]\nrechoke_interval_secs = 0\n",
                "bt.rechoke_interval_secs",
            ),
            (
                "[bt]\noptimistic_interval_secs = 0\n",
                "bt.optimistic_interval_secs",
            ),
            (
                "[bt]\nprogress_emit_min_interval_ms = 0\n",
                "bt.progress_emit_min_interval_ms",
            ),
            ("[bt]\ntracker_numwant = 0\n", "bt.tracker_numwant"),
            (
                "[bt]\npeer_retry_base_secs = 0\n",
                "bt.peer_retry_base_secs",
            ),
            ("[bt]\npeer_retry_cap_secs = 0\n", "bt.peer_retry_cap_secs"),
            (
                "[bt]\ntracker_backoff_start_secs = 0\n",
                "bt.tracker_backoff_start_secs",
            ),
            (
                "[bt]\ntracker_backoff_cap_secs = 0\n",
                "bt.tracker_backoff_cap_secs",
            ),
            (
                "[bt]\nudp_base_timeout_secs = 0\n",
                "bt.udp_base_timeout_secs",
            ),
            ("[bt]\nudp_retries = 0\n", "bt.udp_retries"),
            ("[bt]\nseed_duration_secs = 0.0\n", "bt.seed_duration_secs"),
        ] {
            let err = Config::try_from_toml(text).expect_err(&format!("{key} 拒绝启动"));
            assert!(err.contains(key), "{err}");
            assert!(err.contains("拒绝启动"), "{err}");
        }
    }

    #[test]
    fn trackers_key_accepts_mixed_families_case_insensitive() {
        let c = Config::from_toml(
            "[bt]\ntrackers = [\"https://t1/ann\", \"ws://t2:8080/announce\", \"UDP://t3:6969/announce\", \"WS://t4/ws\"]\n",
        );
        assert_eq!(
            c.trackers,
            vec![
                "https://t1/ann",
                "ws://t2:8080/announce",
                "UDP://t3:6969/announce",
                "WS://t4/ws"
            ]
        );
        let c2 = Config::from_toml("[bt]\ntrackers = []\n");
        assert!(c2.trackers.is_empty());
        let c3 = Config::from_toml("[bt]\nlisten_port = 0\n");
        assert!(c3.trackers.is_empty(), "缺键 = 空");
    }

    #[test]
    fn trackers_key_fail_fast_unknown_scheme_and_duplicates() {
        let e1 = Config::try_from_toml("[bt]\ntrackers = [\"ftp://x/ann\"]\n")
            .expect_err("白名单外 scheme 拒绝启动");
        assert!(e1.contains("bt.trackers"), "{e1}");
        assert!(e1.contains("未知 tracker scheme"), "{e1}");
        let e2 = Config::try_from_toml("[bt]\ntrackers = [\"ws://t/ann\", \"ws://t/ann\"]\n")
            .expect_err("重复项拒绝启动");
        assert!(e2.contains("重复项"), "{e2}");
        let e3 = Config::try_from_toml("[bt]\ntrackers = [\"WS://t/ann\", \"ws://t/ann\"]\n")
            .expect_err("scheme 大小写不敏感判重");
        assert!(e3.contains("重复项"), "{e3}");
        let e4 = Config::try_from_toml("[bt]\ntrackers = [\"udp://noport\"]\n")
            .expect_err("udp 缺端口拒绝启动");
        assert!(e4.contains("bt.trackers"), "{e4}");
        let e5 =
            Config::try_from_toml("[bt]\ntrackers = \"not-a-list\"\n").expect_err("非数组拒绝启动");
        assert!(e5.contains("bt.trackers"), "{e5}");
    }

    #[test]
    fn peer_connection_protocol_validation() {
        let d = Config::from_toml("[bt]\nlisten_port = 0\n");
        assert_eq!(
            d.peer_connection_protocol,
            vec!["tcp", "webrtc"],
            "缺键 = 默认双栈"
        );
        let c = Config::from_toml("[bt]\npeer_connection_protocol = [\"WEBRTC\", \"Tcp\"]\n");
        assert_eq!(
            c.peer_connection_protocol,
            vec!["webrtc", "tcp"],
            "大小写不敏感归一小写"
        );
        let only = Config::from_toml("[bt]\npeer_connection_protocol = [\"webrtc\"]\n");
        assert_eq!(only.peer_connection_protocol, vec!["webrtc"]);
        let e1 = Config::try_from_toml("[bt]\npeer_connection_protocol = [\"quic\"]\n")
            .expect_err("未知值拒绝启动");
        assert!(e1.contains("bt.peer_connection_protocol"), "{e1}");
        assert!(e1.contains("未知值"), "{e1}");
        let e2 = Config::try_from_toml("[bt]\npeer_connection_protocol = [\"tcp\", \"tcp\"]\n")
            .expect_err("重复项拒绝启动");
        assert!(e2.contains("重复项"), "{e2}");
        let e3 = Config::try_from_toml("[bt]\npeer_connection_protocol = [\"TCP\", \"tcp\"]\n")
            .expect_err("大小写重复拒绝启动");
        assert!(e3.contains("重复项"), "{e3}");
        let e4 = Config::try_from_toml("[bt]\npeer_connection_protocol = []\n")
            .expect_err("空数组拒绝启动");
        assert!(e4.contains("bt.peer_connection_protocol"), "{e4}");
    }

    #[test]
    fn try_load_semantic_error_includes_path_key_and_reason() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-cfg-semantic");
        let p = dir.join("config.toml");
        std::fs::write(&p, "[bt]\npeer_connection_protocol = [\"tcp\", \"tcp\"]\n").unwrap();
        let err = Config::try_load(&p).expect_err("语义非法必须报错");
        assert!(err.contains(p.to_string_lossy().as_ref()), "含路径：{err}");
        assert!(
            err.contains("bt.peer_connection_protocol"),
            "含键点路径：{err}"
        );
        assert!(err.contains("重复项"), "含原因：{err}");
        assert!(err.contains("拒绝启动"), "含拒绝启动语义：{err}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fr_02_35_persistence_examples_all_fail_fast() {
        let rows = [
            (
                "[http]\nblock_size_http = \"256 KB\"\n",
                "http.block_size_http",
            ),
            ("[common]\ndownload_slots = 0\n", "common.download_slots"),
            ("[http]\nmax_retries = -1\n", "http.max_retries"),
            ("[http]\nhttp_concurrency = 99\n", "http.http_concurrency"),
            (
                "[common]\nglobal_max_download_speed = \"xyz\"\n",
                "common.global_max_download_speed",
            ),
            ("[http]\nprogress_tick_ms = 0\n", "http.progress_tick_ms"),
            (
                "[http]\nsidecar_flush_secs = 0\n",
                "http.sidecar_flush_secs",
            ),
            (
                "[http]\nthrottle_burst_secs = 0\n",
                "http.throttle_burst_secs",
            ),
            ("[http]\nthrottle_sleep_ms = 0\n", "http.throttle_sleep_ms"),
            ("[http]\ndns_race_grace_ms = 0\n", "http.dns_race_grace_ms"),
            ("[bt]\nbt_concurrency = 0\n", "bt.bt_concurrency"),
            ("[bt]\nbt_concurrency = 99\n", "bt.bt_concurrency"),
            ("[bt]\npeer_pipeline = 0\n", "bt.peer_pipeline"),
            ("[bt]\nupload_slots = 0\n", "bt.upload_slots"),
            (
                "[bt]\nupload_read_concurrency = 0\n",
                "bt.upload_read_concurrency",
            ),
            ("[bt]\nidle_eviction_secs = -1\n", "bt.idle_eviction_secs"),
            (
                "[bt]\nconnect_timeout_secs = 0\n",
                "bt.connect_timeout_secs",
            ),
            (
                "[bt]\nhandshake_timeout_secs = 0\n",
                "bt.handshake_timeout_secs",
            ),
            (
                "[bt]\nrequest_timeout_secs = 0\n",
                "bt.request_timeout_secs",
            ),
            ("[bt]\nrequests_per_piece = 0\n", "bt.requests_per_piece"),
            (
                "[bt]\nrechoke_interval_secs = 0\n",
                "bt.rechoke_interval_secs",
            ),
            (
                "[bt]\noptimistic_interval_secs = 0\n",
                "bt.optimistic_interval_secs",
            ),
            (
                "[bt]\nprogress_emit_min_interval_ms = 0\n",
                "bt.progress_emit_min_interval_ms",
            ),
            ("[bt]\ntracker_numwant = 0\n", "bt.tracker_numwant"),
            (
                "[bt]\npeer_retry_base_secs = 0\n",
                "bt.peer_retry_base_secs",
            ),
            ("[bt]\npeer_retry_cap_secs = 0\n", "bt.peer_retry_cap_secs"),
            (
                "[bt]\ntracker_backoff_start_secs = 0\n",
                "bt.tracker_backoff_start_secs",
            ),
            (
                "[bt]\ntracker_backoff_cap_secs = 0\n",
                "bt.tracker_backoff_cap_secs",
            ),
            (
                "[bt]\nudp_base_timeout_secs = 0\n",
                "bt.udp_base_timeout_secs",
            ),
            ("[bt]\nudp_retries = 0\n", "bt.udp_retries"),
            ("[bt]\nseed_duration_secs = 0\n", "bt.seed_duration_secs"),
            ("[bt]\ntrackers = [\"ftp://x/ann\"]\n", "bt.trackers"),
            (
                "[bt]\npeer_connection_protocol = [\"tcp\", \"tcp\"]\n",
                "bt.peer_connection_protocol",
            ),
        ];
        for (text, key) in rows {
            let err = Config::try_from_toml(text)
                .expect_err(&format!("FR-02-35：{key} 语义非法拒绝启动"));
            assert!(err.contains(key), "键点路径 {key} 未出现在：{err}");
        }
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
    fn normalize_save_dir_expands_tilde_and_relative() {
        let home = Path::new("/home/u");
        let cwd = Path::new("/work/dir");
        assert_eq!(
            normalize_save_dir_with("~/Documents", Some(home), Some(cwd)),
            "/home/u/Documents",
            "~ 前缀展开为主目录"
        );
        assert_eq!(
            normalize_save_dir_with("~", Some(home), Some(cwd)),
            "/home/u",
            "裸 ~ 展开为主目录"
        );
        assert_eq!(
            normalize_save_dir_with("qadl-rel/sub", Some(home), Some(cwd)),
            "/work/dir/qadl-rel/sub",
            "相对路径按 cwd 转绝对"
        );
        assert_eq!(
            normalize_save_dir_with("/tmp/qadl-new", Some(home), Some(cwd)),
            "/tmp/qadl-new",
            "绝对路径原样保留"
        );
    }

    #[test]
    fn normalize_save_dir_trims_trailing_slash_and_space() {
        let home = Path::new("/home/u");
        let cwd = Path::new("/work");
        assert_eq!(
            normalize_save_dir_with("/tmp/qadl/", Some(home), Some(cwd)),
            "/tmp/qadl",
            "末尾 / 归一去除"
        );
        assert_eq!(
            normalize_save_dir_with("  /tmp/qadl  ", Some(home), Some(cwd)),
            "/tmp/qadl",
            "首尾空白修剪"
        );
        assert_eq!(
            normalize_save_dir_with("/", Some(home), Some(cwd)),
            "/",
            "根目录保持 / 不清空"
        );
        assert_eq!(
            normalize_save_dir_with("~/", Some(home), Some(cwd)),
            "/home/u",
            "~/ 尾斜杠展开主目录"
        );
    }

    #[test]
    fn normalize_save_dir_without_home_keeps_literal_tilde() {
        let cwd = Path::new("/work");
        assert_eq!(
            normalize_save_dir_with("~/Documents", None, Some(cwd)),
            "/work/~/Documents",
            "无主目录时 ~/x 保持字面（相对形态）再按 cwd 转绝对"
        );
    }

    #[test]
    fn normalize_save_dir_without_cwd_keeps_relative() {
        let home = Path::new("/home/u");
        assert_eq!(
            normalize_save_dir_with("rel/sub", Some(home), None),
            "rel/sub",
            "cwd 不可得时相对路径保持原样"
        );
    }

    #[test]
    fn default_save_dir_prefill_follows_config_chain() {
        let mut cfg = Config::default();
        assert_eq!(
            default_save_dir_prefill(&cfg),
            default_download_dir(),
            "未配置 → 代码默认下载目录"
        );
        cfg.download_dir = Some("/tmp/qadl-home".to_string());
        assert_eq!(
            default_save_dir_prefill(&cfg),
            "/tmp/qadl-home",
            "配置 download_dir → 预填配置值"
        );
        cfg.download_dir = Some("   ".to_string());
        assert_eq!(
            default_save_dir_prefill(&cfg),
            default_download_dir(),
            "空白配置视同未设（from_toml 已过滤，双保险）"
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
    fn template_line_count_is_174() {
        assert_eq!(
            Config::default_template().lines().count(),
            190,
            "钦定模板 v1.14 = 190 行（split 计数 191，含 48 空行）"
        );
        assert!(Config::default_template().ends_with('\n'));
        assert!(
            !Config::default_template().ends_with("\n\n"),
            "EOF 单换行收尾"
        );
    }

    #[test]
    fn template_three_table_headers_two_line_form() {
        let t = Config::default_template();
        for (sep, decl) in [
            ("# ===== 公共配置 =====", "[common]"),
            ("# ===== HTTP 引擎配置 =====", "[http]"),
            ("# ===== BT 引擎配置 =====", "[bt]"),
        ] {
            let pos = t.find(sep).unwrap_or_else(|| panic!("缺分隔头 {sep}"));
            assert!(!t.contains("# ===== ["), "旧形态分隔头（含表名）已退役");
            let after: Vec<&str> = t[pos..].lines().collect();
            assert_eq!(after[1], "", "分隔头后应空行");
            assert_eq!(after[2], decl, "表声明行应紧随空行");
            assert_eq!(after[3], "", "表声明行后应空行");
        }
    }

    #[test]
    fn template_v13_active_structure() {
        let t = Config::default_template();
        assert!(t.contains("\n[common]\n"), "v1.13 表声明行活跃");
        assert!(t.contains("\n[http]\n"));
        assert!(t.contains("\n[bt]\n"));
        assert!(t.contains("\ndownload_slots = 5\n"), "键值行活跃");
        assert!(t.contains("\nbt_concurrency = 20\n"));
        assert!(
            !t.contains("\n[[common.proxy]]\n"),
            "proxy 双示例块保持注释（D63②：占位主机/凭证不激活）"
        );
        assert!(t.contains("# 取值范围"), "用途/取值范围注释保持");
        assert_eq!(t.matches("取值范围").count(), 40, "40 键各一段取值范围标注");
        let active: Vec<&str> = t
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
            .collect();
        assert_eq!(
            active.len(),
            43,
            "活跃行 = 40 键值行 + 3 表声明行，实际 {active:?}"
        );
    }

    #[test]
    fn template_36_keys_three_line_form() {
        let t = Config::default_template();
        let keys = [
            "download_dir",
            "download_slots",
            "global_max_download_speed",
            "global_max_upload_speed",
            "task_max_download_speed",
            "task_max_upload_speed",
            "block_size_http",
            "http_concurrency",
            "max_retries",
            "auto_retry",
            "backoff_initial",
            "backoff_cap",
            "progress_tick_ms",
            "sidecar_flush_secs",
            "throttle_burst_secs",
            "throttle_sleep_ms",
            "dns_race_grace_ms",
            "trackers",
            "peer_connection_protocol",
            "bt_concurrency",
            "peer_pipeline",
            "upload_slots",
            "upload_read_concurrency",
            "idle_eviction_secs",
            "connect_timeout_secs",
            "handshake_timeout_secs",
            "request_timeout_secs",
            "requests_per_piece",
            "rechoke_interval_secs",
            "optimistic_interval_secs",
            "progress_emit_min_interval_ms",
            "tracker_numwant",
            "peer_retry_base_secs",
            "peer_retry_cap_secs",
            "tracker_backoff_start_secs",
            "tracker_backoff_cap_secs",
            "udp_base_timeout_secs",
            "udp_retries",
            "seed_duration_secs",
            "listen_port",
        ];
        assert_eq!(keys.len(), 40);
        for k in keys {
            assert!(
                t.contains(&format!("\n{k} = ")),
                "模板缺键 {k} 的活跃键值行"
            );
            assert!(
                t.matches(&format!("\n{k} = ")).count() == 1,
                "键 {k} 示例行不唯一"
            );
        }
        for k in keys {
            if k == "download_dir" {
                continue;
            }
            let key_line = t
                .lines()
                .find(|l| l.starts_with(&format!("{k} = ")))
                .unwrap();
            let name_line_idx = t.lines().position(|l| l == key_line).unwrap();
            let lines: Vec<&str> = t.lines().collect();
            let purpose = lines[name_line_idx - 2];
            assert!(
                !purpose.starts_with(&format!("# {k}：")) && !purpose.contains("："),
                "键 {k} 的用途行不得再带键名前缀（v1.12）"
            );
            assert!(!purpose.starts_with("# 取值范围"), "键 {k} 用途行占位");
            let range = lines[name_line_idx - 1];
            assert!(range.starts_with("# 取值范围"), "键 {k} 取值范围行占位");
        }
        assert_eq!(t.matches("取值范围").count(), 40, "40 键各一段取值范围标注");
    }

    #[test]
    fn template_defaults_match_contract() {
        let t = Config::default_template();
        for expected in [
            "download_dir = \"\"",
            "download_slots = 5",
            "global_max_download_speed = 0",
            "global_max_upload_speed = 0",
            "task_max_download_speed = 0",
            "task_max_upload_speed = 0",
            "block_size_http = 1048576",
            "http_concurrency = 4",
            "max_retries = 5",
            "auto_retry = true",
            "backoff_initial = 8.0",
            "backoff_cap = 60.0",
            "progress_tick_ms = 100",
            "sidecar_flush_secs = 2",
            "throttle_burst_secs = 0.5",
            "throttle_sleep_ms = 20",
            "dns_race_grace_ms = 1000",
            "trackers = []",
            "peer_connection_protocol = [\"tcp\", \"webrtc\"]",
            "bt_concurrency = 20",
            "peer_pipeline = 64",
            "upload_slots = 4",
            "upload_read_concurrency = 128",
            "idle_eviction_secs = 60",
            "connect_timeout_secs = 3",
            "handshake_timeout_secs = 10",
            "request_timeout_secs = 20",
            "requests_per_piece = 4",
            "rechoke_interval_secs = 10",
            "optimistic_interval_secs = 30",
            "progress_emit_min_interval_ms = 250",
            "tracker_numwant = 200",
            "peer_retry_base_secs = 5",
            "peer_retry_cap_secs = 120",
            "tracker_backoff_start_secs = 8",
            "tracker_backoff_cap_secs = 60",
            "udp_base_timeout_secs = 15",
            "udp_retries = 3",
            "seed_duration_secs = 1800",
            "listen_port = 0",
        ] {
            assert!(t.contains(expected), "模板缺默认值行：{expected}");
        }
        assert!(
            !t.contains("proxies"),
            "proxies 键已改名 proxy 并迁入 [common]，模板不得输出旧键名"
        );
        assert!(
            !t.contains("\nmax_speed = "),
            "旧键 max_speed 退役，模板不得输出（01-throttle-proxy-14）"
        );
    }

    #[test]
    fn template_proxy_dual_examples_in_common() {
        let t = Config::default_template();
        assert!(
            t.contains("# 命名代理列表（可配置多个）"),
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
                .find(|l| l.starts_with(&format!("{key} = ")))
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
        assert_eq!(
            val_of("sidecar_flush_secs"),
            c.sidecar_flush_secs.to_string()
        );
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
        assert_eq!(
            val_of("idle_eviction_secs"),
            c.idle_eviction_secs.to_string()
        );
        assert_eq!(
            val_of("connect_timeout_secs"),
            c.connect_timeout_secs.to_string()
        );
        assert_eq!(
            val_of("handshake_timeout_secs"),
            c.handshake_timeout_secs.to_string()
        );
        assert_eq!(
            val_of("request_timeout_secs"),
            c.request_timeout_secs.to_string()
        );
        assert_eq!(
            val_of("requests_per_piece"),
            c.requests_per_piece.to_string()
        );
        assert_eq!(
            val_of("rechoke_interval_secs"),
            c.rechoke_interval_secs.to_string()
        );
        assert_eq!(
            val_of("optimistic_interval_secs"),
            c.optimistic_interval_secs.to_string()
        );
        assert_eq!(
            val_of("progress_emit_min_interval_ms"),
            c.progress_emit_min_interval_ms.to_string()
        );
        assert_eq!(val_of("tracker_numwant"), c.tracker_numwant.to_string());
        assert_eq!(
            val_of("peer_retry_base_secs"),
            c.peer_retry_base_secs.to_string()
        );
        assert_eq!(
            val_of("peer_retry_cap_secs"),
            c.peer_retry_cap_secs.to_string()
        );
        assert_eq!(
            val_of("tracker_backoff_start_secs"),
            c.tracker_backoff_start_secs.to_string()
        );
        assert_eq!(
            val_of("tracker_backoff_cap_secs"),
            c.tracker_backoff_cap_secs.to_string()
        );
        assert_eq!(
            val_of("udp_base_timeout_secs"),
            c.udp_base_timeout_secs.to_string()
        );
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
        assert!(c
            .warnings
            .iter()
            .all(|w| w.contains("http / https / socks5")));
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
        assert_eq!(
            c.proxies[0].name, "anon-http",
            "匿名 http 合法，socks5 缺凭证与 http 半凭证作废"
        );
        assert_eq!(c.warnings.len(), 2);
        assert!(c
            .warnings
            .iter()
            .all(|w| !w.contains("p1") && !w.contains("u1")));
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
