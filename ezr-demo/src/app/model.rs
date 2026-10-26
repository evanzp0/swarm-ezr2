use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Protocol {
    Http,
    Https,
    Bt,

    #[allow(dead_code)]
    Ftp,
}

impl Protocol {
    pub fn label(&self) -> &'static str {
        match self {
            Protocol::Http => "HTTP",
            Protocol::Https => "HTTPS",
            Protocol::Bt => "BT",
            Protocol::Ftp => "FTP",
        }
    }

    pub fn is_bt(&self) -> bool {
        matches!(self, Protocol::Bt)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskState {
    Queued,

    Downloading,

    Paused,

    Verifying,

    PostProcessing,

    Completed,

    Failed,

    Seeding,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FailKind {
    Transient,

    Fatal,

    Verify,

    Invalidated,
}

impl TaskState {
    pub fn label(&self) -> &'static str {
        match self {
            TaskState::Queued => "等待中",
            TaskState::Downloading => "下载中",
            TaskState::Paused => "已暂停",
            TaskState::Verifying => "校验中",
            TaskState::PostProcessing => "后期处理中",
            TaskState::Completed => "已完成",
            TaskState::Failed => "已失败",
            TaskState::Seeding => "做种中",
        }
    }

    pub fn is_done(&self) -> bool {
        matches!(self, TaskState::Completed | TaskState::Seeding)
    }

    pub fn shows_conns(&self) -> bool {
        matches!(self, TaskState::Downloading | TaskState::Seeding)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PieceState {
    Empty,

    Done,

    Partial,

    Active,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Tcp,
    Utp,
}

impl Transport {
    pub fn label(self) -> &'static str {
        match self {
            Transport::Tcp => "TCP",
            Transport::Utp => "uTP",
        }
    }
}

const BT_CLIENTS: [&str; 10] = ["QB", "TR", "LT", "UT", "DE", "AR", "RT", "TX", "AZ", "XL"];

fn pseudo_client(seed: u64, idx: usize) -> &'static str {
    let mix = |mut x: u64| {
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
        x ^= x >> 33;
        x
    };
    let h = mix(seed ^ mix((idx as u64 + 3).wrapping_mul(0x9E37_79B9_7F4A_7C15)));
    BT_CLIENTS[(h % BT_CLIENTS.len() as u64) as usize]
}

fn pseudo_transport(seed: u64, idx: usize) -> Transport {
    let mix = |mut x: u64| {
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
        x ^= x >> 33;
        x
    };
    let h = mix(seed ^ mix((idx as u64 + 11).wrapping_mul(0x9E37_79B9_7F4A_7C15)));
    if h % 3 == 0 {
        Transport::Utp
    } else {
        Transport::Tcp
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConnPhase {
    Connecting,

    Connected,

    Closed,
}

#[derive(Clone)]
pub struct Connection {
    pub ip: String,

    pub client: &'static str,

    pub transport: Transport,

    pub win: u64,

    pub win_up: u64,

    pub start: u64,

    pub end: u64,

    pub done: u64,

    pub speed: f64,

    pub up_speed: f64,

    pub cum_down: u64,

    pub cum_up: u64,

    pub phase: ConnPhase,

    pub phase_in: f64,
}

impl Connection {
    pub fn is_active(&self) -> bool {
        matches!(self.phase, ConnPhase::Connecting | ConnPhase::Connected)
    }
}

impl Connection {
    pub fn cap(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    pub fn ip_masked(&self) -> String {
        mask_ip(&self.ip)
    }
}

fn mask_ip(ip: &str) -> String {
    if ip.contains(':') {
        let first = ip.split(':').next().unwrap_or("");
        let last = ip.rsplit(':').next().unwrap_or("");
        format!("{}:*:{}", first, last)
    } else {
        let first = ip.split('.').next().unwrap_or("");
        let last = ip.rsplit('.').next().unwrap_or("");
        format!("{}.*.{}", first, last)
    }
}

fn pseudo_ip(seed: u64, idx: usize) -> String {
    let mix = |mut x: u64| {
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
        x ^= x >> 33;
        x
    };
    let h = mix(seed ^ mix((idx as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)));
    if h % 5 == 0 {
        let g = |k: u64| (mix(h ^ k.wrapping_mul(0x9E37_79B9_7F4A_7C15)) & 0xffff) as u16;
        (0..8)
            .map(|k| format!("{:04x}", g(k)))
            .collect::<Vec<_>>()
            .join(":")
    } else {
        let o = |k: u64, m: u64| mix(h ^ k.wrapping_mul(0x9E37_79B9_7F4A_7C15)) % m;
        format!(
            "{}.{}.{}.{}",
            1 + o(1, 223),
            o(2, 256),
            o(3, 256),
            1 + o(4, 254)
        )
    }
}

#[derive(Clone)]
pub struct Task {
    pub id: u32,
    pub name: String,
    pub protocol: Protocol,
    pub url: String,
    pub save_path: String,
    pub total: u64,
    pub downloaded: u64,

    pub speed: f64,

    pub base_speed: f64,
    pub state: TaskState,

    pub resumable: bool,

    pub concurrency: usize,

    pub connections: Vec<Connection>,

    pub chunk_done: u32,

    pub seeders: usize,

    pub peers: usize,

    pub upload_speed: f64,

    pub uploaded: u64,

    pub seed_left: f64,

    pub post_process: bool,
    pub(crate) post_started: Option<Instant>,

    pub retries: u32,

    pub max_retries: u32,

    pub fail_count: u32,

    pub made_progress: bool,

    pub retry_in: Option<f64>,

    pub fail_kind: Option<FailKind>,

    pub flaky: bool,

    pub verify_fail_once: bool,

    pub precheck_fail_once: bool,
    pub(crate) running_since: Option<Instant>,

    pub start_delay: f64,
    pub(crate) queued_since: Option<Instant>,

    pub has_slot: bool,
    pub(crate) verify_started: Option<Instant>,
    pub error: Option<String>,

    pub checksum: Option<Checksum>,

    pub verify_ok: Option<bool>,

    pub proxy: ProxyChoice,
    pub created: String,
}

impl Task {
    pub fn progress(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.downloaded as f64 / self.total as f64).clamp(0.0, 1.0)
        }
    }

    pub fn size_known(&self) -> bool {
        if self.protocol.is_bt() {
            return true;
        }
        if !matches!(self.protocol, Protocol::Http | Protocol::Https) {
            return true;
        }
        if self.resumable {
            return true;
        }
        matches!(
            self.state,
            TaskState::Verifying | TaskState::PostProcessing | TaskState::Completed
        )
    }

    pub fn eta_secs(&self) -> Option<u64> {
        if !self.size_known() {
            return None;
        }
        if self.speed <= 0.0 || self.state != TaskState::Downloading {
            return None;
        }
        let remain = self.total.saturating_sub(self.downloaded) as f64;
        Some((remain / self.speed) as u64)
    }

    pub fn chunk_info(&self) -> (u32, u32, u64) {
        let piece = chunk_size(self.protocol);
        let y = chunk_total(self.total, piece);
        (self.chunk_done.min(y), y, piece)
    }

    pub fn supports_pieces(&self) -> bool {
        if self.protocol.is_bt() {
            return true;
        }
        !matches!(self.protocol, Protocol::Ftp) && self.resumable
    }

    pub fn pieces(&self) -> Vec<PieceState> {
        let piece = chunk_size(self.protocol);
        let y = chunk_total(self.total, piece) as usize;
        if y == 0 {
            return Vec::new();
        }
        let mut v = vec![PieceState::Empty; y];
        let x = (self.chunk_done as usize).min(y);
        for s in v.iter_mut().take(x) {
            *s = PieceState::Done;
        }
        let downloading = self.state == TaskState::Downloading;
        for c in &self.connections {
            if c.cap() == 0 {
                continue;
            }
            let idx = (c.start / piece) as usize;
            if idx >= y {
                continue;
            }
            v[idx] = if c.done >= c.cap() {
                PieceState::Done
            } else if downloading {
                PieceState::Active
            } else if c.done > 0 {
                PieceState::Partial
            } else {
                PieceState::Empty
            };
        }
        v
    }
}

pub struct Rng {
    s: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { s: seed | 1 }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.s;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.s = x;
        x
    }

    pub fn f01(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / 9_007_199_254_740_992.0
    }

    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f01()
    }
}

static ID_SEQ: AtomicU32 = AtomicU32::new(0);

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

pub(super) fn alloc_task_id(tasks: &[Task]) -> u32 {
    loop {
        let stack_probe = 0u8;

        let addr = &stack_probe as *const u8 as usize as u64;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let seq = ID_SEQ.fetch_add(1, Ordering::Relaxed) as u64;
        let mut g =
            (nanos ^ ((u64::from(std::process::id())) << 32) ^ addr ^ seq.wrapping_mul(GOLDEN)) | 1;
        g = g.wrapping_add(GOLDEN);
        let mut z = g;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        let cand = ((z ^ (z >> 31)) >> 32) as u32;
        let cand = if cand == 0 { 1 } else { cand };
        if !tasks.iter().any(|t| t.id == cand) {
            return cand;
        }
    }
}

pub const CHECKSUM_ALGOS: [(&str, usize); 8] = [
    ("NONE", 0),
    ("MD5", 32),
    ("SHA-1", 40),
    ("SHA-224", 56),
    ("SHA-256", 64),
    ("SHA-384", 96),
    ("SHA-512", 128),
    ("Adler-32", 8),
];

pub const CK_NONE: usize = 0;

#[derive(Clone, Debug)]
pub struct Checksum {
    pub algo: &'static str,

    pub value: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ProxyChoice {
    Direct,

    Named(String),
}

pub(super) fn demo_proxies() -> Vec<(&'static str, &'static str)> {
    vec![("办公网代理", "http"), ("本地 SOCKS5", "socks5")]
}

pub(super) fn save_dir_of(save_path: &str) -> String {
    match save_path.rsplit_once('/') {
        Some((d, _)) => d.to_string(),
        None => String::new(),
    }
}

pub(super) fn resumed_done(id: u32, total: u64) -> u64 {
    let pct = 25 + (id as u64) % 35;
    total * pct / 100
}

pub(super) fn parse_url_task(url: &str) -> (Protocol, String) {
    if let Some(rest) = url.strip_prefix("magnet:") {
        let dn = rest
            .split('&')
            .find_map(|p| p.strip_prefix("dn="))
            .filter(|s| !s.is_empty())
            .unwrap_or("bt-task");
        return (Protocol::Bt, dn.to_string());
    }
    let proto = if url.starts_with("https://") {
        Protocol::Https
    } else {
        Protocol::Http
    };
    let path = url.split("://").nth(1).unwrap_or(url);
    let seg = path
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('/')
        .find(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!("download-{}", ts)
        });
    (proto, seg)
}

pub(super) fn pseudo_total(name: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    80_000_000 + h % 5_000_000_000
}

pub(super) fn is_archive(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".zip")
        || n.ends_with(".tar.gz")
        || n.ends_with(".tar.xz")
        || n.ends_with(".tar.zst")
        || n.ends_with(".tgz")
        || n.ends_with(".whl")
        || n.ends_with(".7z")
        || n.ends_with(".rar")
}

pub(super) fn backoff_secs(retries: u32) -> f64 {
    match retries {
        1 => 8.0,
        2 => 16.0,
        3 => 32.0,
        _ => 60.0,
    }
}

pub(super) fn fail_case(n: u32) -> (&'static str, FailKind, Option<f64>) {
    match n % 5 {
        1 => ("连接超时 (ETIMEDOUT)", FailKind::Transient, None),
        2 => ("连接被重置 (ECONNRESET)", FailKind::Transient, None),
        3 => (
            "文件大小不符（已接收 3.9 GB ≠ Content-Length 4.2 GB）",
            FailKind::Transient,
            None,
        ),
        4 => (
            "HTTP 503 Service Unavailable（Retry-After: 10s）",
            FailKind::Transient,
            Some(10.0),
        ),
        _ => (
            "续传一致性失效（ETag 已变化，服务器内容已更新）",
            FailKind::Invalidated,
            None,
        ),
    }
}

pub(super) const ADD_POOL: [&str; 6] = [
    "https://developer.download.nvidia.com/compute/cuda/12.8.0/cuda_12.8.0_linux.tar.xz",
    "https://dl.k8s.io/v1.32.2/kubernetes-server-linux-amd64.tar.gz",
    "https://huggingface.co/deepseek-ai/DeepSeek-R1-Distill-Qwen-7B/resolve/main/model-q4.gguf",
    "https://github.com/Kitware/CMake/releases/download/v3.31.6/cmake-3.31.6-linux-x86_64.tar.gz",
    "https://mirrors.edge.kernel.org/fedora/releases/42/Workstation/x86_64/iso/Fedora-Workstation-Live-x86_64-42.iso",
    "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=ubuntu-24.04.2-desktop-amd64.iso",
];

pub(super) fn default_conns(p: Protocol) -> usize {
    if p.is_bt() {
        20
    } else {
        4
    }
}

const HTTP_CHUNK_SIZE: u64 = 1024 * 1024;
const BT_CHUNK_SIZE: u64 = 256 * 1024;

pub(super) fn chunk_size(p: Protocol) -> u64 {
    if p.is_bt() {
        BT_CHUNK_SIZE
    } else {
        HTTP_CHUNK_SIZE
    }
}

pub fn chunk_size_label(p: Protocol) -> &'static str {
    if p.is_bt() {
        "256 KB"
    } else {
        "1 MB"
    }
}

pub(super) fn chunk_total(total: u64, piece: u64) -> u32 {
    if total == 0 || piece == 0 {
        0
    } else {
        total.div_ceil(piece) as u32
    }
}

const CONN_WEIGHTS: [f64; 16] = [
    0.95, 0.45, 0.10, 0.00, 0.75, 0.40, 0.85, 0.15, 0.55, 0.20, 0.55, 0.00, 0.70, 0.25, 0.80, 0.35,
];

fn spread_bytes(amount: u64, n: usize, cap: u64) -> Vec<u64> {
    let mut v = vec![0u64; n];
    if amount == 0 || n == 0 {
        return v;
    }
    let wsum: f64 = (0..n).map(|i| CONN_WEIGHTS[i % 16]).sum();
    let mut assigned = 0u64;
    for i in 0..n {
        let raw = (amount as f64 * (CONN_WEIGHTS[i % 16] / wsum)).round() as u64;
        let d = raw.min(cap);
        v[i] = d;
        assigned += d;
    }
    let mut diff = amount as i64 - assigned as i64;
    let mut k = 0;
    while diff != 0 && k < 8 * n {
        let i = k % n;
        if diff > 0 && v[i] < cap {
            let add = (diff as u64).min(cap - v[i]);
            v[i] += add;
            diff -= add as i64;
        } else if diff < 0 && v[i] > 0 {
            let sub = ((-diff) as u64).min(v[i]);
            v[i] -= sub;
            diff += sub as i64;
        }
        k += 1;
    }
    v
}

pub(super) fn make_chunk_conns(
    total: u64,
    done_total: u64,
    n: usize,
    piece: u64,
    seed: u64,
) -> (Vec<Connection>, u32) {
    let done = done_total.min(total);
    make_chunk_conns_lease(total, (done / piece) as u32, done % piece, n, piece, seed)
}

pub(super) fn make_chunk_conns_lease(
    total: u64,
    completed: u32,
    rem: u64,
    n: usize,
    piece: u64,
    seed: u64,
) -> (Vec<Connection>, u32) {
    let y = chunk_total(total, piece);
    if y == 0 {
        return (Vec::new(), 0);
    }
    let y64 = y as u64;
    let piece_end = |i: u64| if i + 1 == y64 { total } else { (i + 1) * piece };
    let x0 = (completed as u64).min(y64);
    let done = (x0 * piece + rem).min(total);
    let (x, lease_from, active, rem) = if done >= total {
        let a = (n.max(1) as u64).min(y64);
        (y64, y64 - a, a, 0u64)
    } else {
        let a = (n.max(1) as u64).min(y64 - x0);
        (x0, x0, a, rem)
    };
    let partials = spread_bytes(rem, active as usize, piece);
    let conns = (0..active as usize)
        .map(|i| {
            let idx = lease_from + i as u64;
            let start = idx * piece;
            let end = piece_end(idx);
            let partial = if done >= total {
                end - start
            } else {
                partials[i].min(end - start)
            };
            Connection {
                ip: pseudo_ip(seed, i),

                client: pseudo_client(seed, i),
                transport: pseudo_transport(seed, i),
                win: 0,
                win_up: 0,
                start,
                end,
                done: partial,
                speed: 0.0,
                up_speed: 0.0,

                cum_down: partial,
                cum_up: 0,

                phase: ConnPhase::Connected,
                phase_in: 0.0,
            }
        })
        .collect();
    (conns, x as u32)
}
