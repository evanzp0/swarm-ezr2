//! config — 配置文件（FR-01-71：`~/.ezr/config.toml`，缺省回退、非法值回退；
//! v1.3/D16：`.ezr` 根目录可经环境变量 `EZR_HOME` 重定位）

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// HTTP 默认块大小（1 MB，D13；单一来源在 `chunk::HTTP_CHUNK_SIZE`）
pub const DEFAULT_BLOCK_SIZE_HTTP: u64 = super::chunk::HTTP_CHUNK_SIZE;
/// 下载槽位默认值（D13）
pub const DEFAULT_DOWNLOAD_SLOTS: usize = 5;
/// 默认并发（HTTP 4，FR-01-04；v1.5/FR-01-88 自 DEFAULT_CONCURRENCY 改名）
pub const DEFAULT_HTTP_CONCURRENCY: usize = 4;
/// 自动重试上限默认值（FR-01-41）
pub const DEFAULT_MAX_RETRIES: u32 = 5;

/// 代理类型（v1.6/FR-01-89 建；v1.7 两值；v1.9 恢复三值 http / https / socks5）。
/// **type 为唯一事实来源**（v1.9/FR-01-89：url 键退役后无 scheme 可言，内部代理
/// url 由 type+ip+port 构造）；代理协议与下载目标协议正交：三类型均同时服务
/// http 与 https 下载目标（http/https 型经 CONNECT 隧道——https 型为代理自身
/// 走 TLS，basic auth 在 TLS 会话内生效；socks5 型经 SOCKS 隧道）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProxyKind {
    /// HTTP 代理（`http://{ip}:{port}`，含 https 下载目标经 CONNECT 隧道）
    Http,
    /// HTTPS 代理（v1.9 独立类型：代理自身走 TLS，`https://{ip}:{port}`）
    Https,
    /// SOCKS5 代理
    Socks5,
}

impl ProxyKind {
    /// 从 type 键字面量解析（v1.9：必填三值，大小写不敏感；未知 → None，
    /// 由校验链作废该条目）
    #[must_use]
    pub fn from_label(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "http" => Some(Self::Http),
            "https" => Some(Self::Https),
            "socks5" => Some(Self::Socks5),
            _ => None,
        }
    }

    /// 类型显示名（下拉类型标注与警告文案用；与内部 url scheme 同源，
    /// 单一事实在 [`ProxyKind::scheme`]）
    #[must_use]
    pub fn label(self) -> &'static str {
        self.scheme()
    }

    /// 内部代理 url 的 scheme（v1.9/FR-01-89：`endpoint_url` 构造用）
    #[must_use]
    pub fn scheme(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
            Self::Socks5 => "socks5",
        }
    }
}

/// 命名代理配置条目（v1.5/FR-01-86，D18；v1.9/FR-01-86/89 字段重构）：
/// `[[proxies]]` 数组项。name 非空且唯一；type 必填三值（唯一事实来源）；
/// ip 非空（IP 字面量或主机名）；port 整数 1..=65535；username/password 可选
/// （凭证规则按类型分级，FR-01-93；不写入界面可见文案与日志）。url 键已退役
/// （按未知键忽略，零警告零迁移）；内部代理 url 经 [`ProxyConfig::endpoint_url`]
/// 由 type+ip+port 构造（单一构造点）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyConfig {
    /// 代理名（添加/修改对话框按名选用；非空且唯一）
    pub name: String,
    /// 代理类型（v1.9/FR-01-89：必填三值，唯一事实来源）
    pub kind: ProxyKind,
    /// 代理地址：IP 字面量或主机名（非空；IPv6 字面量含 `:`，构造 url 时加方括号）
    pub ip: String,
    /// 代理端口（整数 1..=65535）
    pub port: u16,
    /// 代理用户名（可选）
    pub username: Option<String>,
    /// 代理密码（可选；不落日志不显界面）
    pub password: Option<String>,
}

impl ProxyConfig {
    /// 内部代理 url（v1.9/FR-01-89：由 type+ip+port 构造，引擎建 client 用；
    /// 无凭证形态——凭证分字段存放，FR-01-93 ④，url 可安全进 toast/界面展示）。
    /// ip 为 IPv6 字面量（含 `:`）时按方括号形态构造（如 `http://[::1]:8080`），
    /// 三类型统一收口在单一构造点（packs/_common/notes/rust.md 口径）。
    #[must_use]
    pub fn endpoint_url(&self) -> String {
        if self.ip.contains(':') {
            format!("{}://[{}]:{}", self.kind.scheme(), self.ip, self.port)
        } else {
            format!("{}://{}:{}", self.kind.scheme(), self.ip, self.port)
        }
    }
}

/// 任务级代理选择（v1.5/FR-01-86；v1.6/FR-01-90 两态收窄——旧全局 `proxy` 键退役，
/// D18 改判）。旧注册表/旧行为缺省 = [`ProxyChoice::Direct`]（旧 "global" 值加载时
/// 映射直连，零迁移）。
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ProxyChoice {
    /// 直连（不使用任何代理；缺省）
    #[default]
    Direct,
    /// 命名引用（`[[proxies]].name`；配置中不存在该名时按直连 + toast 提醒）
    Named(String),
}

/// 解析后的代理端点（引擎建 client 用；纯数据，不含 reqwest 类型——
/// App 层经 [`Config::resolve_proxy`] 得到，引擎层据此构建/查池客户端）
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyEndpoint {
    /// 代理地址（**无凭证形态**，FR-01-93 ④：凭证分字段存放，url 可安全
    /// 进 toast/界面展示；认证 url 由引擎构建 client 时内部构造）
    pub url: String,
    /// 代理类型（v1.8/FR-01-93 ③：socks5 型凭证经引擎内部认证 url 走
    /// RFC 1929 user/pass 握手；http 型成对凭证走 HTTP basic auth）
    pub kind: ProxyKind,
    /// 代理用户名（可选）
    pub username: Option<String>,
    /// 代理密码（可选）
    pub password: Option<String>,
}

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
    /// 命名代理列表（v1.5/FR-01-86；非法条目跳过并记入 [`Config::warnings`]）
    pub proxies: Vec<ProxyConfig>,
    /// 默认并发数（对话框留空时；1–64 钳制，默认 4；v1.5/FR-01-88 改名）
    pub http_concurrency: usize,
    /// 配置加载警告（v1.5/FR-01-86：非法代理条目等；启动时入 toast，不入界面其余处）
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
            warnings: Vec::new(),
        }
    }
}

/// `[[proxies]]` 条目的反序列化形态（六键均可缺；`type` 经 rename 承接）。
/// `port` 用 `toml::Value` 容错承接：TOML 整数之外的形态（如字符串 "eighty"）
/// 不毒化整个文档解析，交由校验链按「非整数」作废该条目（FR-01-86 v1.9 非法清单）。
/// url 键已退役（v1.9）：不在结构内声明，serde 按未知键忽略（零警告零迁移）。
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
    proxies: Option<Vec<ProxyRaw>>,
    http_concurrency: Option<usize>,
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

    /// 从 TOML 文本构建（单键非法回退默认；非法代理条目跳过并记警告）
    #[must_use]
    pub fn from_toml(text: &str) -> Self {
        let raw: ConfigRaw = toml::from_str(text).unwrap_or_default();
        let d = Config::default();
        let mut warnings = Vec::new();
        // 命名代理条目（FR-01-86/D18）：空名/重名一律跳过 + 警告一次（容错口径
        // 同 FR-01-71：便利性配置不阻塞启动）；v1.9 校验链顺序：空名 → 重名 →
        // type（缺失/未知作废）→ ip（缺失/空白作废）→ port（缺失/非整数/越界作废）
        // → 凭证规则（FR-01-93）；url 键退役不参与校验（按未知键忽略）。
        // 密码不进警告文案
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
            // type 校验（v1.9/FR-01-89）：必填三值 http/https/socks5、唯一事实来源——
            // 缺失或未知 → 条目作废 + 警告（v1.7「推断/按 url 处理」随 url 键退役废止）
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
            // ip 校验（v1.9/FR-01-86）：必填非空（IP 字面量或主机名；允许主机名，
            // 不做 IP 语法强校验），缺失或 trim 后空白 → 作废 + 警告
            let ip = p.ip.unwrap_or_default().trim().to_string();
            if ip.is_empty() {
                warnings.push(format!(
                    "已忽略无效代理条目「{name}」：ip 缺失或空白（应为 IP 字面量或主机名）"
                ));
                continue;
            }
            // port 校验（v1.9/FR-01-86）：合法域 1..=65535——键缺失、非整数形态
            // （toml::Value 容错承接，如 "eighty"）或越界（0/65536/负数）→ 作废 + 警告
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
            // 凭证规则（v1.8/FR-01-93；v1.9 https 与 http 同级）：socks5 型凭证必填——
            // 缺任一 → 条目作废 + 警告（SOCKS5 认证为 RFC 1929 user/pass 握手，不支持
            // 匿名 socks5）；http 与 https 型凭证可选——都缺省 = 匿名代理（无警告），
            // 只填其一 → 作废 + 警告（成对原则）。警告文案沿用「已忽略无效代理
            // 条目」家族，凭证值不回显（FR-01-93 ④）
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
            warnings,
        }
    }

    /// 任务级代理选择 → 具体端点（v1.5/FR-01-86；v1.6/D18 两态）。Direct → None；
    /// Named → 按 name 查 [`Config::proxies`]（不存在 = None + `true` 缺失标记，
    /// 调用方 toast 一次）。返回 `(端点, 引用是否失效)`；端点 url 由
    /// [`ProxyConfig::endpoint_url`] 构造（v1.9：type+ip+port），合法性在引擎建
    /// client 时判定（非法 → 引擎 toast + 直连回退）。
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

/// 操作者钦定模板文本（v1.10/FR-01-85：`default_template()` 输出的**逐字节唯一
/// 权威**）。权威原文内嵌于 `project/features/01-config-template.feature` 场景 01
/// （54 行 = 标题行 + 9 键×3 行三行式 + proxies 一句头注释 + 双示例块各 7 行 +
/// 11 空行，EOF 以一个换行符收尾），此处为同一文本的整段字面量——本常量与测试
/// 基线 `CHARTER_TEMPLATE_TEXT` 均由提取脚本从 feature 内嵌原文生成；任何内容
/// 变更必须先改 feature 内嵌原文，再经脚本同步两处字面量（契约测试
/// `template_matches_charter_text` 以独立基线把关）。
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
    /// 配置模板（FR-01-85，v1.4；v1.10 修订——操作者钦定文本唯一权威）：输出与
    /// feature 内嵌钦定原文**逐字节一致**（含空行、行序、EOF 单换行收尾；
    /// v1.6「空格/顺序可调」口径废止）。9 键保留「用途 + 取值范围 + 示例行」
    /// 三行式；proxies 段豁免解释性注释（一句头注释 + 双示例块；FR-01-93⑤
    /// 凭证注释行条款废止，凭证运行期语义不变）。钦定示例值与 [`Config::default`]
    /// / `DEFAULT_*` 比对 9/9 一致（specifier v1.10 登记）；模板整体被解析时得到
    /// 全部默认值——保持注释状态 = 文件缺失行为（契约测试
    /// `template_parses_to_defaults`）。
    #[must_use]
    pub fn default_template() -> String {
        AUTHORIZED_TEMPLATE.to_string()
    }
}

/// 配置模板自动生成（FR-01-85/D17）：`path` 不存在时创建父目录并写入全注释
/// 默认模板；已存在（含损坏文件）一律不覆写（D17①，`create_new` 语义：
/// 并发竞态下同样宁可不写）；创建/写失败错误向上传播（调用点按 D17② 静默
/// 跳过，不阻塞启动，配置按 FR-01-71 缺失口径加载）。
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
        assert_eq!(c.http_concurrency, 64); // 钳制而非回退
    }

    /// v1.16/FR-01-104（D31）：block_size_http 正值 < 1 MB 一律钳制为 1 MB 生效
    /// （下限钳制，非回退默认）；≥ 1 MB 原值生效；0 仍回退默认
    #[test]
    fn block_size_http_positive_below_floor_clamped() {
        // 小于下限的正值 → 钳到 1 MB（非回退路径：回退也恰好是 1 MB，用可区分
        // 的默认值场景断言钳制来源——0 → 默认；512 → 钳制，两者同值但语义分支
        // 不同，另用 ≥1MB 原值生效锁住“非无条件覆盖”）
        let c = Config::from_toml("block_size_http = 512\n");
        assert_eq!(c.block_size_http, 1_048_576);
        let c = Config::from_toml("block_size_http = 1048575\n");
        assert_eq!(c.block_size_http, 1_048_576, "下限之下 1 字节也钳制");
        let c = Config::from_toml("block_size_http = 1048576\n");
        assert_eq!(c.block_size_http, 1_048_576, "恰在下限 = 原值");
        let c = Config::from_toml("block_size_http = 2097152\n");
        assert_eq!(c.block_size_http, 2_097_152, "大于下限原值生效");
    }

    /// v1.5/FR-01-88：旧键名 default_concurrency 不设别名——按未知键忽略
    /// （FR-01-71 口径），并发取默认值
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

    /// v1.6/FR-01-90：旧全局 proxy 键退役——按未知键忽略（FR-01-71 口径），
    /// 不产生警告、不影响代理列表
    #[test]
    fn legacy_proxy_key_ignored() {
        let c = Config::from_toml("proxy = \"http://127.0.0.1:8118\"\n");
        assert_eq!(c, Config::default(), "旧 proxy 键按未知键忽略");
        assert!(c.warnings.is_empty());
    }

    /// 空白目录与非法数值逐键回退（filter 分支：空白/非正/非有限一律弃用）
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

    // ===== EZR_HOME 重定位（v1.3/D16，FR-01-70/71 修订）=====
    // 注：env 读写仅限本组测试触碰 EZR_HOME（其余测试不读该键，无并行竞态）；
    //     no-HOME 场景不入单测（临时改 HOME 会与并行读 HOME 的测试竞态），
    //     由 pty 端到端验证兜底。

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

    // ===== 配置模板自动生成（FR-01-85，v1.4；v1.10 钦定文本逐字节锁定；D17 两条边界）=====

    /// 钦定原文基线（v1.10/FR-01-85）：由提取脚本从
    /// `project/features/01-config-template.feature` 场景 01 内嵌原文生成（54 行，
    /// 含空行与行序，EOF 以一个换行符收尾），作为整文本比对的**独立基线**——改动
    /// 实现 [`AUTHORIZED_TEMPLATE`] 而不同步此基线即红。重生成路径：feature 内嵌
    /// 原文 → 提取脚本 → 两处字面量同步更新。
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

    /// v1.10 核心契约：`default_template()` 输出与钦定原文**整文本逐字节一致**
    /// （含空行、行序与 EOF 收尾字节；engineering.md「生成默认文件」第 ④ 条——
    /// 整文本比对取代逐行 contains，锁住多余行/缺失行/空行增删/行尾空白）。
    /// 失败信息打印逐行差异定位。
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

    /// 行级差异定位（仅 [`template_matches_charter_text`] 失败信息用）：逐行对出
    /// 全部内容差异；行内容一致时提示差异在行尾/EOF 收尾字节
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

    /// 模板内每条非空行均为注释（全注释 ⇒ 解析结果与文件缺失一致）
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

    /// 9 键逐一以 `# 键 = 默认值` 示例行在位（值与 FR-01-71/D13 默认口径一致；
    /// v1.6：proxy 键退役不再输出。v1.10：示例值随钦定文本逐字节锁定，本测试
    /// 降为行级诊断辅助——权威契约是 `template_matches_charter_text` 整文本比对）
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

    /// v1.10/FR-01-85/FR-01-93⑤：proxies 段**最小形态**——一句头注释 + 双
    /// `# [[proxies]]` 示例块（各含 name/type/ip/port/username/password 6 键行，
    /// 无 url 行）；解释性注释（凭证规则/三类型语义/IPv6/type 唯一事实来源/
    /// 示例块凭证语义注记）全部退出模板（语义由运行期校验警告与 README/规格
    /// 承载）。行序与逐字节内容由 `template_matches_charter_text` 权威锁定，
    /// 本测试为形态诊断辅助。
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
            // v1.9 起字段形态 ip/port/type；v1.10：不再附凭证语义注记
            "# username = \"ezr\"",
        ] {
            assert!(t.contains(expected), "模板代理示例缺行：{expected}");
        }
        assert!(
            !t.contains("# url ="),
            "url 键退役，模板不得输出 url 行（FR-01-86 v1.9）"
        );
        // FR-01-93⑤ 废止承载：解释性注释退出模板（v1.10）
        assert!(!t.contains("凭证"), "凭证规则注释行退出模板（FR-01-93⑤）");
        assert!(!t.contains("唯一事实来源"), "type 语义注释退出模板");
        assert!(!t.contains("代理自身走 TLS"), "https 语义注释退出模板");
        assert!(!t.contains("方括号"), "IPv6 注记退出模板");
        assert!(!t.contains("型示例"), "示例块凭证语义注记退出模板");
    }

    /// 每键段落含：用途说明 + 「取值范围」标注（FR-01-85 用途/取值范围双注释
    /// 契约对 9 键成立；v1.10：9 键各为三行式段落（用途行 + 取值范围行 + 示例行），
    /// proxies 段豁免解释性注释——无「取值范围」标注）
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
            9,
            "9 键各一段取值范围标注；proxies 段豁免（v1.10）"
        );
    }

    /// 核心行为契约：模板整体被解析时得到全部默认值（模板本身不改变行为，
    /// FR-01-85「与文件缺失完全一致」；v1.5：proxies 注释块解析为空列表）
    #[test]
    fn template_parses_to_defaults() {
        assert_eq!(
            Config::from_toml(&Config::default_template()),
            Config::default()
        );
    }

    // ===== 命名代理配置（v1.5/FR-01-86，D18；v1.9/FR-01-86/89 字段重构 ip/port/type）=====

    /// 合法条目全量解析（v1.9 字段形态 ip/port/type）：名称/类型/地址端口/
    /// 认证三元组逐字段落位（密码不进警告）
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

    /// v1.9/FR-01-89：type 必填三值 http/https/socks5（大小写不敏感）→ 三变体；
    /// https 恢复独立类型（代理自身走 TLS），v1.7 收编口径废止
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

    /// v1.9/FR-01-89（场景 09）：type 缺失 → 条目作废 + 警告一次
    /// （url 键退役无从推断，v1.7 推断口径废止；文案对齐「已忽略无效代理条目」家族）
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

    /// v1.9/FR-01-89（场景 10）：type 未知值（三值之外）→ 条目作废 + 警告
    /// （v1.7「保留按 url 处理」随 url 键退役废止）
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

    /// v1.9/FR-01-86：url 键退役——按未知键忽略（FR-01-71 口径，零警告、零迁移）；
    /// 其余字段合法的条目照常加载，不做 url→ip/port 回读推断
    #[test]
    fn legacy_url_key_ignored() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"old\"\ntype = \"http\"\nurl = \"http://old.example:8080\"\nip = \"127.0.0.1\"\nport = 8080\n",
        );
        assert_eq!(c.proxies.len(), 1, "url 键忽略，条目按 ip/port/type 加载");
        assert_eq!(c.proxies[0].endpoint_url(), "http://127.0.0.1:8080");
        assert!(c.warnings.is_empty(), "url 键退役零警告");
    }

    /// v1.9/FR-01-86（场景 16 三相位）：ip 键缺失 / 空字符串 / 仅空白 →
    /// 条目作废 + 警告一次（ip 必填非空；合法条目不受影响）
    #[test]
    fn proxy_ip_missing_or_blank_invalidated() {
        // 相位 1：ip 键缺失
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
        // 相位 2：空字符串；相位 3：仅空白字符
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

    /// v1.9/FR-01-86（场景 17 五相位）：port 键缺失 / 非整数（"eighty" 字符串）/
    /// 0 / 65536 / -1 → 条目作废 + 警告一次（合法域 1..=65535；字符串形态由
    /// toml::Value 容错承接，不毒化整个文档解析）
    #[test]
    fn proxy_port_missing_or_out_of_range_invalidated() {
        for port_raw in [
            "",                  // 键缺失
            "port = \"eighty\"", // 非整数形态
            "port = 0",          // 越界（下界外）
            "port = 65536",      // 越界（上界外）
            "port = -1",         // 负数
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

    /// v1.9/FR-01-89（场景 08 config 层）：内部代理 url 由 type+ip+port 经
    /// `endpoint_url` 单一构造点生成，三类型三 scheme（无凭证形态）
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

    /// v1.9/FR-01-89（场景 18 config 层）：ip 为 IPv6 字面量（含 `:`）时内部 url
    /// 加方括号构造（对配置与界面透明；主机名/IPv4 形态不加方括号）
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

    // ===== 代理凭证规则（v1.8/FR-01-93；v1.9 https 与 http 同级）=====

    /// FR-01-93 ①（场景 11）：socks5 型缺 username → 条目作废 + 警告一次
    /// （文案含 socks5 / username 与 password / 不支持匿名语义，不回显凭证值）
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

    /// FR-01-93 ①（场景 12）：socks5 型缺 password → 条目作废 + 警告一次（同口径）
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

    /// FR-01-93 ①：socks5 凭证 trim 后均非空——空白值视同缺失，作废口径同缺任一
    #[test]
    fn socks5_blank_credentials_invalidated() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"s5-blank\"\ntype = \"socks5\"\nip = \"127.0.0.1\"\nport = 18768\nusername = \"u1\"\npassword = \"   \"\n",
        );
        assert!(c.proxies.is_empty(), "空白 password 归 None → 视同缺失作废");
        assert_eq!(c.warnings.len(), 1);
        assert!(c.warnings[0].contains("socks5"));
    }

    /// FR-01-93 ②（场景 13 http 相位）：http 型无凭证 → 匿名代理正常加载，无警告
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

    /// FR-01-93 ②（场景 14 http 相位，两行 Examples 合一测试双相位）：http 型
    /// 只填其一 → 条目作废 + 警告（成对配置文案，不回显凭证值；https 相位见
    /// `https_credentials_three_phases`）
    #[test]
    fn http_half_credentials_invalidated() {
        // 相位 1：只填 username 未填 password
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
        // 相位 2：只填 password 未填 username
        let c2 = Config::from_toml(
            "[[proxies]]\nname = \"half-http\"\ntype = \"http\"\nip = \"127.0.0.1\"\nport = 18766\npassword = \"p1-s3cret\"\n",
        );
        assert!(c2.proxies.is_empty(), "只填 password 同样作废");
        assert_eq!(c2.warnings.len(), 1);
        assert!(c2.warnings[0].contains("成对"));
        assert!(!c2.warnings[0].contains("p1-s3cret"), "警告不回显凭证值");
    }

    /// FR-01-93 ② v1.9 增补（场景 13/14/15 https 相位合一）：https 型凭证与
    /// http 型完全同级——都缺省 = 匿名（无警告）、成对 = 保留（TLS 会话内
    /// basic auth）、只填其一 = 作废 + 警告（成对文案，不回显凭证值）
    #[test]
    fn https_credentials_three_phases() {
        // 相位 1：匿名（都缺省）
        let c = Config::from_toml(
            "[[proxies]]\nname = \"anon-tls\"\ntype = \"https\"\nip = \"127.0.0.1\"\nport = 18767\n",
        );
        assert_eq!(c.proxies.len(), 1, "https 匿名代理合法加载");
        assert_eq!(c.proxies[0].kind, ProxyKind::Https);
        assert_eq!(c.proxies[0].username, None);
        assert_eq!(c.proxies[0].password, None);
        assert!(c.warnings.is_empty(), "匿名代理不产生凭证相关警告");
        // 相位 2：成对凭证 → 保留
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
        // 相位 3：只填 username 未填 password → 作废 + 成对警告
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

    /// FR-01-93（场景 15 socks5 相位 config 层）：socks5 带凭证 → 条目保留且
    /// 类型/地址端口/凭证字段在位
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

    /// 非法条目（空名/重名/空 ip）跳过 + 警告；合法条目不受影响；密码不进警告
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

    /// 两态选择解析（resolve_proxy，v1.6/D18 两态）：Direct/Named 与失效引用；
    /// 端点 url 经 `endpoint_url` 构造（v1.9：type+ip+port，无凭证形态）
    #[test]
    fn resolve_proxy_two_states() {
        let c = Config::from_toml(
            "[[proxies]]\nname = \"a\"\ntype = \"http\"\nip = \"a\"\nport = 2\nusername = \"u\"\npassword = \"pw\"\n",
        );
        // Direct → None
        assert_eq!(c.resolve_proxy(&ProxyChoice::Direct), (None, false));
        // Named → 命名条目（内部 url 由 type+ip+port 构造；含认证字段）
        let (ep, missing) = c.resolve_proxy(&ProxyChoice::Named("a".into()));
        let ep = ep.unwrap();
        assert_eq!(
            (ep.url.as_str(), ep.username.as_deref()),
            ("http://a:2", Some("u"))
        );
        assert_eq!(ep.kind, ProxyKind::Http);
        assert!(!missing);
        // Named 失效引用 → None + missing 标记
        assert_eq!(
            c.resolve_proxy(&ProxyChoice::Named("gone".into())),
            (None, true)
        );
    }

    /// 不存在 → 建父目录 + 写模板；幂等（再次调用内容不变）
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

    /// 已存在（含生效值）→ 不覆写（D17①：操作者手工配置优先于模板）
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

    /// 写失败（目录只读）→ 错误向上传播，不写半截文件（调用点按 D17② 静默跳过）
    #[cfg(unix)]
    #[test]
    fn ensure_propagates_write_error() {
        use std::os::unix::fs::PermissionsExt;
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-tpl-ro");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        let p = dir.join("config.toml");
        let r = ensure_default_config(&p);
        // 先恢复权限再断言，防断言失败时残留只读目录
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(r.is_err(), "只读目录写失败应报错");
        assert!(!p.exists(), "失败路径不得留下半截文件");
        std::fs::remove_dir_all(&dir).ok();
    }
}
