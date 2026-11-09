use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::{SinkExt, StreamExt};
use tokio::net::UdpSocket;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

use super::bencode::{self, BValue};
use super::BtError;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TrackerUrl {
    Http(String),
    Ws(String),
    Udp(String),
}

impl TrackerUrl {
    #[must_use]
    pub fn parse(s: &str) -> Option<TrackerUrl> {
        let lower = s.to_ascii_lowercase();
        let rest = lower
            .strip_prefix("https://")
            .or_else(|| lower.strip_prefix("http://"))
            .or_else(|| lower.strip_prefix("wss://"))
            .or_else(|| lower.strip_prefix("ws://"))
            .or_else(|| lower.strip_prefix("udp://"))?;
        if rest.is_empty() {
            return None;
        }
        if lower.starts_with("udp://") {
            udp_host_port(s)?;
            Some(TrackerUrl::Udp(s.to_string()))
        } else if lower.starts_with("ws://") || lower.starts_with("wss://") {
            Some(TrackerUrl::Ws(s.to_string()))
        } else {
            Some(TrackerUrl::Http(s.to_string()))
        }
    }
}

#[must_use]
pub fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[usize::from(b >> 4)] as char);
        out.push(HEX[usize::from(b & 0x0F)] as char);
    }
    out
}

/// 字节序列 → latin-1 二进制字符串：每字节映射为同码点 char。这是真实
/// WebTorrent ws tracker 协议的 id 编码（JSON 序列化时控制字符转义、
/// ≥0x80 以 UTF-8 多字节落地；服务端 JSON.parse 后按 bin2hex 还原字节）。
#[must_use]
pub fn bin_string(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// 20 字节 id 解析，兼容两种线上形态：20 字符二进制串（真实 WebTorrent）与
/// 40 字符小写 hex（mini tracker/harness 契约）。
#[must_use]
pub fn id20(s: &str) -> Option<[u8; 20]> {
    let chars: Vec<char> = s.chars().collect();
    match chars.len() {
        20 => {
            let mut out = [0u8; 20];
            for (i, c) in chars.iter().enumerate() {
                let v = u32::from(*c);
                if v > u32::from(u8::MAX) {
                    return None;
                }
                out[i] = v as u8;
            }
            Some(out)
        }
        40 => hex20(s),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WsOffer {
    pub offer_id: String,
    pub sdp: String,
    pub to_peer_id: [u8; 20],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WsAnswer {
    pub offer_id: String,
    pub sdp: String,
    pub to_peer_id: [u8; 20],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WsSignal {
    pub offer_id: String,
    pub sdp: String,
    pub peer_id: [u8; 20],
}

#[derive(Clone, Debug, Default)]
pub struct WsAnnounceResponse {
    pub interval: u32,
    pub min_interval: Option<u32>,
    pub complete: u32,
    pub incomplete: u32,
    pub peers: Vec<PeerAddr>,
    pub peer_ids: Vec<[u8; 20]>,
    pub offers_to_answer: Vec<WsSignal>,
    pub answers: Vec<WsSignal>,
}

#[must_use]
pub fn udp_host_port(url: &str) -> Option<(String, u16)> {
    const SCHEME: &str = "udp://";
    if !url
        .get(..SCHEME.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(SCHEME))
    {
        return None;
    }
    let authority = url[SCHEME.len()..].split(['/', '?']).next()?;
    if authority.is_empty() {
        return None;
    }
    let (host, port_str) = if let Some(bracketed) = authority.strip_prefix('[') {
        let close = bracketed.find(']')?;
        let after = &bracketed[close + 1..];
        (&bracketed[..close], after.strip_prefix(':')?)
    } else {
        let colon = authority.rfind(':')?;
        (&authority[..colon], &authority[colon + 1..])
    };
    if host.is_empty() {
        return None;
    }
    let port: u16 = port_str.parse().ok()?;
    if port == 0 {
        return None;
    }
    Some((host.to_string(), port))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackerEvent {
    Started,
    Completed,
    Stopped,
}

#[derive(Clone, Copy, Debug)]
pub struct AnnounceParams {
    pub infohash: [u8; 20],
    pub peer_id: [u8; 20],
    pub port: u16,
    pub uploaded: u64,
    pub downloaded: u64,
    pub left: u64,
    pub event: Option<TrackerEvent>,
    pub numwant: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerAddr {
    pub ip: IpAddr,
    pub port: u16,
}

#[derive(Clone, Debug)]
pub struct TrackerResponse {
    pub interval: u32,
    pub min_interval: Option<u32>,
    pub complete: u32,
    pub incomplete: u32,
    pub peers: Vec<PeerAddr>,
}

fn percent_encode_opt(raw: &[u8], force: bool) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(raw.len() * 3);
    for &b in raw {
        if !force
            && matches!(
                b,
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~'
            )
        {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(HEX[usize::from(b >> 4)] as char);
            out.push(HEX[usize::from(b & 0x0F)] as char);
        }
    }
    out
}

const fn event_param(e: Option<TrackerEvent>) -> Option<&'static str> {
    match e {
        None => None,
        Some(TrackerEvent::Started) => Some("started"),
        Some(TrackerEvent::Completed) => Some("completed"),
        Some(TrackerEvent::Stopped) => Some("stopped"),
    }
}

fn announce_query(p: &AnnounceParams) -> String {
    let mut q = String::with_capacity(192);
    q.push_str("info_hash=");
    q.push_str(&percent_encode_opt(&p.infohash, true));
    q.push_str("&peer_id=");
    q.push_str(&percent_encode_opt(&p.peer_id, true));
    q.push_str("&port=");
    q.push_str(&p.port.to_string());
    q.push_str("&uploaded=");
    q.push_str(&p.uploaded.to_string());
    q.push_str("&downloaded=");
    q.push_str(&p.downloaded.to_string());
    q.push_str("&left=");
    q.push_str(&p.left.to_string());
    q.push_str("&compact=1");
    if let Some(ev) = event_param(p.event) {
        q.push_str("&event=");
        q.push_str(ev);
    }
    q.push_str("&numwant=");
    q.push_str(&p.numwant.to_string());
    q
}

pub async fn announce_http(
    url: &str,
    p: &AnnounceParams,
    client: &reqwest::Client,
) -> Result<TrackerResponse, BtError> {
    let sep = if url.contains('?') { '&' } else { '?' };
    let full = format!("{url}{sep}{}", announce_query(p));
    let resp = client
        .get(&full)
        .send()
        .await
        .map_err(|e| BtError::Tracker(format!("HTTP announce 请求失败：{e}")))?;
    let status = resp.status();
    let body = resp
        .bytes()
        .await
        .map_err(|e| BtError::Tracker(format!("HTTP announce 响应读取失败：{e}")))?;
    if !status.is_success() {
        return Err(BtError::Tracker(format!(
            "HTTP announce 状态码异常：{status}"
        )));
    }
    parse_http_response(&body)
}

fn ws_json_field_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn ws_json_field_u64(v: &serde_json::Value, key: &str) -> Option<u64> {
    v.get(key).and_then(serde_json::Value::as_u64)
}

fn hex20(s: &str) -> Option<[u8; 20]> {
    let b = s.as_bytes();
    if b.len() != 40 {
        return None;
    }
    let mut out = [0u8; 20];
    for (i, slot) in out.iter_mut().enumerate() {
        let hi = (b[i * 2] as char).to_digit(16)?;
        let lo = (b[i * 2 + 1] as char).to_digit(16)?;
        *slot = ((hi << 4) | lo) as u8;
    }
    Some(out)
}

fn parse_ws_signal(v: &serde_json::Value) -> Option<WsSignal> {
    let offer_id = ws_json_field_str(v, "offer_id")?;
    let sdp = v
        .get("offer")
        .and_then(|o| ws_json_field_str(o, "sdp"))
        .or_else(|| v.get("answer").and_then(|a| ws_json_field_str(a, "sdp")))
        .or_else(|| ws_json_field_str(v, "sdp"))?;
    let peer_id = id20(&ws_json_field_str(v, "peer_id")?)?;
    Some(WsSignal {
        offer_id,
        sdp,
        peer_id,
    })
}

fn parse_ws_announce_value(root: &serde_json::Value) -> Result<WsAnnounceResponse, BtError> {
    if let Some(reason) = root
        .get("failure reason")
        .and_then(serde_json::Value::as_str)
    {
        return Err(BtError::Tracker(format!("tracker 拒绝：{reason}")));
    }
    let interval = ws_json_field_u64(root, "interval")
        .filter(|v| *v > 0)
        .map_or(60, |v| v.min(u64::from(u32::MAX)) as u32);
    let complete = ws_json_field_u64(root, "complete")
        .unwrap_or(0)
        .min(u64::from(u32::MAX)) as u32;
    let incomplete = ws_json_field_u64(root, "incomplete")
        .unwrap_or(0)
        .min(u64::from(u32::MAX)) as u32;
    let mut resp = WsAnnounceResponse {
        interval,
        min_interval: ws_json_field_u64(root, "min interval")
            .map(|v| v.min(u64::from(u32::MAX)) as u32),
        complete,
        incomplete,
        ..Default::default()
    };
    if let Some(list) = root.get("peers").and_then(serde_json::Value::as_array) {
        for p in list {
            // peer_id 不再依赖 ip 存在：真实 ws swarm 的 peers 只有 peer_id，
            // 连接完全靠 offer 中继。
            if let Some(pid) = ws_json_field_str(p, "peer_id").and_then(|s| id20(&s)) {
                resp.peer_ids.push(pid);
            }
            let ip = ws_json_field_str(p, "ip");
            let port = ws_json_field_u64(p, "port");
            if let (Some(ip), Some(port)) = (ip, port) {
                let Ok(port) = u16::try_from(port) else {
                    continue;
                };
                if let Ok(ip) = ip.parse::<IpAddr>() {
                    resp.peers.push(PeerAddr { ip, port });
                }
            }
        }
    }
    if let Some(list) = root
        .get("offers_to_answer")
        .and_then(serde_json::Value::as_array)
    {
        for p in list {
            if let Some(sig) = parse_ws_signal(p) {
                resp.offers_to_answer.push(sig);
            }
        }
    }
    if let Some(list) = root.get("answers").and_then(serde_json::Value::as_array) {
        for p in list {
            if let Some(sig) = parse_ws_signal(p) {
                resp.answers.push(sig);
            }
        }
    }
    // 真实 tracker 的顶层单条推送：offer/answer 中继为独立消息，
    // {action, info_hash, offer|answer:{type,sdp}, offer_id, peer_id(二进制串)}。
    let has_offer = root.get("offer").is_some_and(serde_json::Value::is_object);
    let has_answer = root.get("answer").is_some_and(serde_json::Value::is_object);
    if has_offer || has_answer {
        if let Some(sig) = parse_ws_signal(root) {
            if has_offer {
                resp.offers_to_answer.push(sig);
            } else {
                resp.answers.push(sig);
            }
        }
    }
    Ok(resp)
}

/// str 入口（测试/调试用；运行时路径经 classify_ws_message → parse_ws_announce_value）。
#[allow(dead_code)]
pub fn parse_ws_announce_response(text: &str) -> Result<WsAnnounceResponse, BtError> {
    let root: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| BtError::Tracker(format!("ws announce 响应非 JSON：{e}")))?;
    parse_ws_announce_value(&root)
}

async fn ws_connect(
    url: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    BtError,
> {
    let request = url
        .into_client_request()
        .map_err(|e| BtError::Tracker(format!("ws tracker 请求构造失败：{e}")))?;
    if url.to_ascii_lowercase().starts_with("wss://") {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let cfg = Arc::new(
            rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(WsAcceptAnyVerifier { provider }))
                .with_no_client_auth(),
        );
        let (ws, _resp) = tokio_tungstenite::connect_async_tls_with_config(
            request,
            None,
            false,
            Some(tokio_tungstenite::Connector::Rustls(cfg)),
        )
        .await
        .map_err(|e| BtError::Tracker(format!("ws tracker 连接失败：{e}")))?;
        return Ok(ws);
    }
    let (ws, _resp) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| BtError::Tracker(format!("ws tracker 连接失败：{e}")))?;
    Ok(ws)
}

#[derive(Debug)]
struct WsAcceptAnyVerifier {
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl rustls::client::danger::ServerCertVerifier for WsAcceptAnyVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// 组装 ws announce 请求（真实 WebTorrent 线上协议）：
/// - info_hash/peer_id 为 20 字节二进制串（latin-1 字符，非 hex）；
/// - offers 元素为 `{offer_id, offer:{type,sdp}}` 对象，定向场景附 `to_peer_id`
///   （真实服务器忽略该字段，offer 由 tracker 抽样转发；harness 靠它做定向中继）；
/// - answer 为顶层 `answer:{type,sdp}` + 顶层 `offer_id` + 顶层 `to_peer_id`
///   （v11 bittorrent-tracker 服务端解析口径）。
#[must_use]
pub fn build_announce_json(
    p: &AnnounceParams,
    offers: &[WsOffer],
    answer: Option<&WsAnswer>,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "action": "announce",
        "info_hash": bin_string(&p.infohash),
        "peer_id": bin_string(&p.peer_id),
        "port": p.port,
        "uploaded": p.uploaded,
        "downloaded": p.downloaded,
        "left": p.left,
        "numwant": p.numwant,
    });
    if let Some(ev) = event_param(p.event) {
        body["event"] = serde_json::Value::String(ev.to_string());
    }
    if !offers.is_empty() {
        let list: Vec<serde_json::Value> = offers
            .iter()
            .map(|o| {
                let mut e = serde_json::json!({
                    "offer_id": o.offer_id,
                    "offer": {"type": "offer", "sdp": o.sdp},
                });
                if o.to_peer_id != [0u8; 20] {
                    e["to_peer_id"] = serde_json::Value::String(bin_string(&o.to_peer_id));
                }
                e
            })
            .collect();
        body["offers"] = serde_json::Value::Array(list);
    }
    if let Some(a) = answer {
        body["answer"] = serde_json::json!({"type": "answer", "sdp": a.sdp});
        body["offer_id"] = serde_json::Value::String(a.offer_id.clone());
        body["to_peer_id"] = serde_json::Value::String(bin_string(&a.to_peer_id));
    }
    body
}

/// answer 独立 announce（真实协议：answer 必须顶层携带 to_peer_id/offer_id，
/// 服务器对 answer 类 announce 不回响应——故不能等待响应）。
#[must_use]
pub fn build_answer_announce_json(
    infohash: &[u8; 20],
    peer_id: &[u8; 20],
    a: &WsAnswer,
) -> serde_json::Value {
    serde_json::json!({
        "action": "announce",
        "info_hash": bin_string(infohash),
        "peer_id": bin_string(peer_id),
        "to_peer_id": bin_string(&a.to_peer_id),
        "answer": {"type": "answer", "sdp": a.sdp},
        "offer_id": a.offer_id,
    })
}

/// ws 入站消息分类：真实 tracker 的 announce 响应（含 interval/complete）与
/// offer/answer 顶层推送走不同分支；harness 捎带数组由 Response 解析承载。
pub enum WsInbound {
    Response(WsAnnounceResponse),
    Failure(String),
    Offer(WsSignal),
    Answer(WsSignal),
    Other,
}

#[must_use]
pub fn classify_ws_message(v: &serde_json::Value) -> WsInbound {
    if let Some(reason) = v.get("failure reason").and_then(serde_json::Value::as_str) {
        return WsInbound::Failure(reason.to_string());
    }
    let has_offer = v.get("offer").is_some_and(serde_json::Value::is_object);
    let has_answer = v.get("answer").is_some_and(serde_json::Value::is_object);
    if has_offer || has_answer {
        if let Some(sig) = parse_ws_signal(v) {
            return if has_offer {
                WsInbound::Offer(sig)
            } else {
                WsInbound::Answer(sig)
            };
        }
    }
    if v.get("interval").is_some() || v.get("complete").is_some() || v.get("incomplete").is_some() {
        if let Ok(r) = parse_ws_announce_value(v) {
            return WsInbound::Response(r);
        }
    }
    WsInbound::Other
}

type WsIo =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// 与 ws tracker 的持久会话（真实 WebTorrent 语义）：连接保持打开，
/// offer/answer 由服务器随时推送，announce 沿同一 socket 周期重发。
pub struct WsSession {
    sink: futures::stream::SplitSink<WsIo, Message>,
    stream: futures::stream::SplitStream<WsIo>,
}

impl WsSession {
    pub async fn connect(url: &str) -> Result<WsSession, BtError> {
        let ws = ws_connect(url).await?;
        let (sink, stream) = ws.split();
        Ok(WsSession { sink, stream })
    }

    pub async fn send_json(&mut self, v: &serde_json::Value) -> Result<(), BtError> {
        let text = serde_json::to_string(v)
            .map_err(|e| BtError::Tracker(format!("ws announce 序列化失败：{e}")))?;
        self.sink
            .send(Message::Text(text))
            .await
            .map_err(|e| BtError::Tracker(format!("ws announce 发送失败：{e}")))
    }

    /// 收一条消息；`wait` 内无消息返回 `Ok(None)`；连接断开返回 Err。
    pub async fn recv_json(
        &mut self,
        wait: Duration,
    ) -> Result<Option<serde_json::Value>, BtError> {
        let deadline = Instant::now() + wait;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            match tokio::time::timeout(remaining, self.stream.next()).await {
                Ok(Some(Ok(Message::Text(t)))) => {
                    if let Ok(v) = serde_json::from_str(&t) {
                        return Ok(Some(v));
                    }
                }
                Ok(Some(Ok(Message::Binary(b)))) => {
                    if let Ok(s) = String::from_utf8(b) {
                        if let Ok(v) = serde_json::from_str(&s) {
                            return Ok(Some(v));
                        }
                    }
                }
                Ok(Some(Ok(_))) => {}
                Ok(Some(Err(e))) => {
                    return Err(BtError::Tracker(format!("ws tracker 连接读取失败：{e}")));
                }
                Ok(None) => return Err(BtError::Tracker("ws tracker 连接已关闭".into())),
                Err(_) => return Ok(None),
            }
        }
    }

    pub async fn close(&mut self) {
        let _ = self.sink.close().await;
    }
}

pub const WS_CONNECT_TIMEOUT_SECS: u64 = 10;
pub const WS_RESPONSE_TIMEOUT_SECS: u64 = 15;

/// 一次性 announce（连接-发送-收一条响应-关闭）：用于 stopped 等无信令诉求的
/// 场景；常规数据面走 `WsSession` 持久会话。响应前的他人推送在此丢弃。
pub async fn announce_ws(
    url: &str,
    p: &AnnounceParams,
    offers: &[WsOffer],
    answer: Option<&WsAnswer>,
) -> Result<WsAnnounceResponse, BtError> {
    let mut sess = tokio::time::timeout(
        Duration::from_secs(WS_CONNECT_TIMEOUT_SECS),
        WsSession::connect(url),
    )
    .await
    .map_err(|_| BtError::Tracker("ws tracker 连接超时".to_string()))??;
    let body = build_announce_json(p, offers, answer);
    sess.send_json(&body).await?;
    let deadline = Instant::now() + Duration::from_secs(WS_RESPONSE_TIMEOUT_SECS);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(BtError::Tracker("ws announce 无响应".into()));
        }
        match sess.recv_json(remaining).await {
            Ok(Some(v)) => match classify_ws_message(&v) {
                WsInbound::Failure(fr) => {
                    sess.close().await;
                    return Err(BtError::Tracker(format!("tracker 拒绝：{fr}")));
                }
                WsInbound::Response(r) => {
                    sess.close().await;
                    return Ok(r);
                }
                _ => continue,
            },
            Ok(None) => {
                sess.close().await;
                return Err(BtError::Tracker("ws announce 无响应".into()));
            }
            Err(e) => return Err(e),
        }
    }
}

fn parse_http_response(body: &[u8]) -> Result<TrackerResponse, BtError> {
    let root = bencode::decode_all(body)?;
    if let Some(reason) = root.get_str("failure reason") {
        return Err(BtError::Tracker(reason.to_string()));
    }
    let interval = root
        .get_int("interval")
        .filter(|v| *v > 0)
        .map_or(60, |v| v as u32);
    let min_interval = root
        .get_int("min interval")
        .filter(|v| *v > 0)
        .map(|v| v as u32);
    let complete = root.get_int("complete").unwrap_or(0).max(0) as u32;
    let incomplete = root.get_int("incomplete").unwrap_or(0).max(0) as u32;
    let mut peers = match root.get("peers") {
        Some(BValue::Bytes(b)) => parse_compact_peers4(b),
        Some(BValue::List(l)) => parse_peer_dict_list(l),
        _ => Vec::new(),
    };
    if let Some(BValue::Bytes(b)) = root.get("peers6") {
        peers.extend(parse_compact_peers6(b));
    }
    Ok(TrackerResponse {
        interval,
        min_interval,
        complete,
        incomplete,
        peers,
    })
}

fn parse_peer_dict_list(list: &[BValue]) -> Vec<PeerAddr> {
    list.iter()
        .filter_map(|v| {
            let ip: IpAddr = v.get_str("ip")?.parse().ok()?;
            let port = u16::try_from(v.get_int("port")?).ok()?;
            Some(PeerAddr { ip, port })
        })
        .collect()
}

#[must_use]
pub fn parse_compact_peers4(buf: &[u8]) -> Vec<PeerAddr> {
    buf.as_chunks::<6>()
        .0
        .iter()
        .map(|c| PeerAddr {
            ip: IpAddr::V4(Ipv4Addr::new(c[0], c[1], c[2], c[3])),
            port: be16(&c[4..6]),
        })
        .collect()
}

#[must_use]
pub fn parse_compact_peers6(buf: &[u8]) -> Vec<PeerAddr> {
    buf.as_chunks::<18>()
        .0
        .iter()
        .map(|c| PeerAddr {
            ip: IpAddr::V6(Ipv6Addr::new(
                be16(&c[0..2]),
                be16(&c[2..4]),
                be16(&c[4..6]),
                be16(&c[6..8]),
                be16(&c[8..10]),
                be16(&c[10..12]),
                be16(&c[12..14]),
                be16(&c[14..16]),
            )),
            port: be16(&c[16..18]),
        })
        .collect()
}

const fn be16(b: &[u8]) -> u16 {
    u16::from_be_bytes([b[0], b[1]])
}

const fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

const UDP_MAGIC: u64 = 0x417_2710_1980;
const UDP_RECV_BUF: usize = 4096;

pub async fn announce_udp(
    sock: UdpSocket,
    host: &str,
    port: u16,
    p: &AnnounceParams,
    base_timeout_secs: u64,
    retries: u32,
) -> Result<TrackerResponse, BtError> {
    let addr = resolve_udp_target(host, port).await?;
    let connect_txid = rand_u32();
    let req = build_connect_request(connect_txid);
    let conn_id = udp_roundtrip(
        &sock,
        addr,
        &req,
        connect_txid,
        parse_connect_response,
        base_timeout_secs,
        retries,
    )
    .await?;
    let announce_txid = rand_u32();
    let req = build_announce_request(conn_id, announce_txid, p);
    udp_roundtrip(
        &sock,
        addr,
        &req,
        announce_txid,
        parse_announce_response,
        base_timeout_secs,
        retries,
    )
    .await
}

async fn resolve_udp_target(host: &str, port: u16) -> Result<SocketAddr, BtError> {
    tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| BtError::Tracker(format!("tracker 地址解析失败：{e}")))?
        .next()
        .ok_or_else(|| BtError::Tracker("tracker 地址解析无结果".into()))
}

fn build_connect_request(txid: u32) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[0..8].copy_from_slice(&UDP_MAGIC.to_be_bytes());
    b[12..16].copy_from_slice(&txid.to_be_bytes());
    b
}

fn build_announce_request(conn_id: u64, txid: u32, p: &AnnounceParams) -> [u8; 98] {
    let mut b = [0u8; 98];
    b[0..8].copy_from_slice(&conn_id.to_be_bytes());
    b[8..12].copy_from_slice(&1u32.to_be_bytes());
    b[12..16].copy_from_slice(&txid.to_be_bytes());
    b[16..36].copy_from_slice(&p.infohash);
    b[36..56].copy_from_slice(&p.peer_id);
    b[56..64].copy_from_slice(&p.downloaded.to_be_bytes());
    b[64..72].copy_from_slice(&p.left.to_be_bytes());
    b[72..80].copy_from_slice(&p.uploaded.to_be_bytes());
    b[80..84].copy_from_slice(&udp_event(p.event).to_be_bytes());
    b[92..96].copy_from_slice(&num_want(p.numwant).to_be_bytes());
    b[96..98].copy_from_slice(&p.port.to_be_bytes());
    b
}

const fn udp_event(e: Option<TrackerEvent>) -> u32 {
    match e {
        None => 0,
        Some(TrackerEvent::Started) => 2,
        Some(TrackerEvent::Completed) => 1,
        Some(TrackerEvent::Stopped) => 3,
    }
}

const fn num_want(numwant: u32) -> u32 {
    if numwant == 0 {
        u32::MAX
    } else {
        numwant
    }
}

fn parse_connect_response(payload: &[u8], txid: u32) -> Result<u64, BtError> {
    validate_udp_header(payload, 0, txid)?;
    if payload.len() < 16 {
        return Err(BtError::Tracker("connect 响应截断".into()));
    }
    let mut b = [0u8; 8];
    b.copy_from_slice(&payload[8..16]);
    Ok(u64::from_be_bytes(b))
}

fn parse_announce_response(payload: &[u8], txid: u32) -> Result<TrackerResponse, BtError> {
    validate_udp_header(payload, 1, txid)?;
    if payload.len() < 20 {
        return Err(BtError::Tracker("announce 响应截断".into()));
    }
    Ok(TrackerResponse {
        interval: be32(&payload[8..12]),
        min_interval: None,
        complete: be32(&payload[16..20]),
        incomplete: be32(&payload[12..16]),
        peers: parse_compact_peers4(&payload[20..]),
    })
}

fn validate_udp_header(payload: &[u8], expect_action: u32, txid: u32) -> Result<(), BtError> {
    if payload.len() < 8 {
        return Err(BtError::Tracker("UDP 响应截断".into()));
    }
    let action = be32(&payload[0..4]);
    if be32(&payload[4..8]) != txid {
        return Err(BtError::Tracker("UDP 响应 transaction_id 不匹配".into()));
    }
    if action == 3 {
        let msg = String::from_utf8_lossy(&payload[8..]);
        return Err(BtError::Tracker(format!("tracker 错误：{msg}")));
    }
    if action != expect_action {
        return Err(BtError::Tracker(format!(
            "UDP 响应 action 不符：{action}（期望 {expect_action}）"
        )));
    }
    Ok(())
}

type UdpParser<T> = fn(&[u8], u32) -> Result<T, BtError>;

async fn udp_roundtrip<T>(
    sock: &UdpSocket,
    addr: SocketAddr,
    req: &[u8],
    txid: u32,
    parse: UdpParser<T>,
    base_timeout_secs: u64,
    retries: u32,
) -> Result<T, BtError> {
    let mut wait_secs = base_timeout_secs;
    let mut buf = vec![0u8; UDP_RECV_BUF];
    let mut last_err: Option<BtError> = None;
    for attempt in 0..=retries {
        if attempt > 0 {
            wait_secs = wait_secs.saturating_mul(2);
        }
        sock.send_to(req, addr)
            .await
            .map_err(|e| BtError::Tracker(format!("UDP announce 发送失败：{e}")))?;
        let recv =
            tokio::time::timeout(Duration::from_secs(wait_secs), sock.recv_from(&mut buf)).await;
        match recv {
            Err(_) => {}
            Ok(Err(e)) => return Err(BtError::Tracker(format!("UDP announce 接收失败：{e}"))),
            Ok(Ok((n, _))) => match parse(&buf[..n], txid) {
                Ok(v) => return Ok(v),
                Err(e) => {
                    #[cfg(test)]
                    eprintln!("PROBE-UDP parse err n={n}: {e}");
                    last_err = Some(e);
                }
            },
        }
    }
    Err(last_err.unwrap_or_else(|| BtError::Tracker("UDP tracker 无响应（重试耗尽）".into())))
}

fn rand_u32() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut state = (u64::from(nanos) ^ (u64::from(n) << 32)).wrapping_add(0x9E37_79B9_7F4A_7C15);
    state ^= state >> 33;
    state = state.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    state ^= state >> 33;
    state as u32
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;
    use crate::model::config::{DEFAULT_UDP_BASE_TIMEOUT_SECS, DEFAULT_UDP_RETRIES};

    fn dict(pairs: &[(&str, BValue)]) -> BValue {
        let m: BTreeMap<Vec<u8>, BValue> = pairs
            .iter()
            .map(|(k, v)| (k.as_bytes().to_vec(), v.clone()))
            .collect();
        BValue::Dict(m)
    }

    #[test]
    fn percent_encode_unreserved_and_binary() {
        let raw = [
            0x00, 0x7F, 0x80, 0xFF, b'a', b'Z', b'0', b'-', b'_', b'.', b'~',
        ];
        assert_eq!(percent_encode_opt(&raw, false), "%00%7F%80%FFaZ0-_.~");
        assert_eq!(
            percent_encode_opt(&raw, true),
            "%00%7F%80%FF%61%5A%30%2D%5F%2E%7E"
        );
    }

    #[test]
    fn announce_query_layout_matches_fr02_14() {
        let ih = [
            0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
        ];
        let pid = *b"-EZ2000-abcDEF012345";
        let p = AnnounceParams {
            infohash: ih,
            peer_id: pid,
            port: 6881,
            uploaded: 10,
            downloaded: 20,
            left: 30,
            event: Some(TrackerEvent::Started),
            numwant: 50,
        };
        assert_eq!(
            announce_query(&p),
            "info_hash=%12%34%56%78%9A%BC%DE%F0%01%02%03%04%05%06%07%08%09%0A%0B%0C\
             &peer_id=%2D%45%5A%32%30%30%30%2D%61%62%63%44%45%46%30%31%32%33%34%35\
             &port=6881&uploaded=10&downloaded=20&left=30\
             &compact=1&event=started&numwant=50"
        );
        let p2 = AnnounceParams { event: None, ..p };
        assert!(!announce_query(&p2).contains("event="));
    }

    #[test]
    fn tracker_url_parse_schemes() {
        assert_eq!(
            TrackerUrl::parse("http://t.example/announce"),
            Some(TrackerUrl::Http("http://t.example/announce".into()))
        );
        assert_eq!(
            TrackerUrl::parse("https://t.example/a?x=1"),
            Some(TrackerUrl::Http("https://t.example/a?x=1".into()))
        );
        assert_eq!(
            TrackerUrl::parse("udp://t.example:6969/announce"),
            Some(TrackerUrl::Udp("udp://t.example:6969/announce".into()))
        );
        assert!(matches!(
            TrackerUrl::parse("UDP://h:1"),
            Some(TrackerUrl::Udp(_))
        ));
        assert!(matches!(
            TrackerUrl::parse("HTTP://x"),
            Some(TrackerUrl::Http(_))
        ));
        assert_eq!(TrackerUrl::parse("ftp://x"), None);
        assert_eq!(TrackerUrl::parse("httpx://x"), None);
        assert_eq!(TrackerUrl::parse(""), None);
        assert_eq!(TrackerUrl::parse("http://"), None);
        assert_eq!(TrackerUrl::parse("udp://"), None);
        assert_eq!(TrackerUrl::parse("udp://noport.example/announce"), None);
        assert_eq!(TrackerUrl::parse("udp://h:99999"), None);
    }

    #[test]
    fn tracker_url_parse_ws_family() {
        assert_eq!(
            TrackerUrl::parse("ws://127.0.0.1:17905/announce"),
            Some(TrackerUrl::Ws("ws://127.0.0.1:17905/announce".into()))
        );
        assert_eq!(
            TrackerUrl::parse("wss://tracker.fixture.local/ws"),
            Some(TrackerUrl::Ws("wss://tracker.fixture.local/ws".into()))
        );
        assert!(matches!(
            TrackerUrl::parse("WS://h/ann"),
            Some(TrackerUrl::Ws(_))
        ));
        assert!(matches!(
            TrackerUrl::parse("WSS://h:443/ws"),
            Some(TrackerUrl::Ws(_))
        ));
        assert_eq!(TrackerUrl::parse("ws://"), None);
        assert_eq!(TrackerUrl::parse("wss://"), None);
        assert_eq!(
            TrackerUrl::parse("ws://x?q=1"),
            Some(TrackerUrl::Ws("ws://x?q=1".into()))
        );
    }

    #[test]
    fn ws_announce_response_json_contract() {
        let text = serde_json::json!({
            "interval": 30,
            "complete": 5,
            "incomplete": 2,
            "peers": [
                {"ip": "9.9.9.9", "port": 7000, "peer_id": hex_lower(&[7u8; 20])},
                {"ip": "not-an-ip", "port": 1, "peer_id": "zz"},
                {"port": 8},
                {"ip": "2001:db8::1", "port": 6883}
            ],
            "offers_to_answer": [
                {"offer_id": "abc", "sdp": "v=0...", "peer_id": hex_lower(&[1u8; 20])},
                {"offer_id": "bad", "sdp": "v=0..."}
            ]
        })
        .to_string();
        let r = parse_ws_announce_response(&text).unwrap();
        assert_eq!(r.interval, 30);
        assert_eq!(r.complete, 5);
        assert_eq!(r.incomplete, 2);
        assert_eq!(r.peers.len(), 2);
        assert_eq!(r.peers[0].port, 7000);
        assert_eq!(r.peers[1].ip, "2001:db8::1".parse::<IpAddr>().unwrap());
        assert_eq!(r.peer_ids, vec![[7u8; 20]]);
        assert_eq!(r.offers_to_answer.len(), 1);
        assert_eq!(r.offers_to_answer[0].offer_id, "abc");
        assert_eq!(r.offers_to_answer[0].peer_id, [1u8; 20]);
        assert!(r.answers.is_empty());
    }

    #[test]
    fn ws_announce_response_defaults_and_rejects_non_json() {
        let r = parse_ws_announce_response("{}").unwrap();
        assert_eq!(r.interval, 60);
        assert_eq!(r.complete, 0);
        assert!(r.peers.is_empty());
        assert!(parse_ws_announce_response("not json").is_err());
    }

    #[test]
    fn ws_failure_reason_is_error() {
        let text = serde_json::json!({
            "action": "announce",
            "failure reason": "info_hash field is missing or wrong",
        })
        .to_string();
        let err = parse_ws_announce_response(&text).unwrap_err();
        assert!(err.to_string().contains("info_hash"));
    }

    #[test]
    fn ws_push_single_offer_and_answer_classify() {
        // 真实 tracker 推送：顶层 offer 对象 + 二进制 id
        let offer_msg = serde_json::json!({
            "action": "announce",
            "info_hash": bin_string(&[3u8; 20]),
            "offer": {"type": "offer", "sdp": "v=0 push"},
            "offer_id": bin_string(&[4u8; 20]),
            "peer_id": bin_string(&[5u8; 20]),
        });
        match classify_ws_message(&offer_msg) {
            WsInbound::Offer(sig) => {
                assert_eq!(sig.sdp, "v=0 push");
                assert_eq!(sig.peer_id, [5u8; 20]);
                assert_eq!(sig.offer_id, bin_string(&[4u8; 20]));
            }
            _ => panic!("应分类为 Offer"),
        }
        // answer 推送（hex id 兼容 harness 契约）
        let answer_msg = serde_json::json!({
            "action": "announce",
            "answer": {"type": "answer", "sdp": "v=0 ans"},
            "offer_id": "abcd-0001",
            "peer_id": hex_lower(&[6u8; 20]),
        });
        match classify_ws_message(&answer_msg) {
            WsInbound::Answer(sig) => {
                assert_eq!(sig.sdp, "v=0 ans");
                assert_eq!(sig.peer_id, [6u8; 20]);
            }
            _ => panic!("应分类为 Answer"),
        }
    }

    #[test]
    fn ws_announce_response_peer_ids_without_ip() {
        // 真实 ws swarm 的 peers 只有 peer_id（无 ip/port）→ 应进入 peer_ids
        let text = serde_json::json!({
            "interval": 111,
            "complete": 38,
            "incomplete": 7,
            "peers": [
                {"peer_id": bin_string(&[1u8; 20])},
                {"peer_id": hex_lower(&[2u8; 20])},
                {"ip": "9.9.9.9", "port": 7000, "peer_id": hex_lower(&[3u8; 20])},
            ],
        })
        .to_string();
        let r = parse_ws_announce_response(&text).unwrap();
        assert_eq!(r.interval, 111);
        assert_eq!(r.complete, 38);
        assert_eq!(r.incomplete, 7);
        assert_eq!(r.peer_ids.len(), 3);
        assert_eq!(r.peer_ids[0], [1u8; 20]);
        assert_eq!(r.peer_ids[1], [2u8; 20]);
        assert_eq!(r.peers.len(), 1);
        assert_eq!(r.peers[0].port, 7000);
    }

    #[test]
    fn id20_accepts_binary_and_hex() {
        assert_eq!(id20(&bin_string(&[0xABu8; 20])), Some([0xABu8; 20]));
        assert_eq!(id20(&hex_lower(&[0xABu8; 20])), Some([0xABu8; 20]));
        assert_eq!(id20("short"), None);
        assert_eq!(id20(""), None);
    }

    #[test]
    fn bin_string_latin1_round_trip() {
        let raw: Vec<u8> = (0u8..=255).collect();
        let s = bin_string(&raw);
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars.len(), 256);
        for (i, c) in chars.iter().enumerate() {
            assert_eq!(u32::from(*c) as u8, raw[i]);
        }
    }

    #[test]
    fn build_announce_json_real_wire_shape() {
        let p = AnnounceParams {
            infohash: [7u8; 20],
            peer_id: [8u8; 20],
            port: 6881,
            uploaded: 0,
            downloaded: 0,
            left: 30,
            event: Some(TrackerEvent::Started),
            numwant: 50,
        };
        let offers = vec![
            WsOffer {
                offer_id: "blind-1".into(),
                sdp: "v=0 blind".into(),
                to_peer_id: [0u8; 20],
            },
            WsOffer {
                offer_id: "aimed-1".into(),
                sdp: "v=0 aimed".into(),
                to_peer_id: [9u8; 20],
            },
        ];
        let v = build_announce_json(&p, &offers, None);
        assert_eq!(v["info_hash"], bin_string(&[7u8; 20]));
        assert_eq!(v["peer_id"], bin_string(&[8u8; 20]));
        let list = v["offers"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert!(
            list[0].get("to_peer_id").is_none(),
            "盲 offer 不带 to_peer_id"
        );
        assert_eq!(list[0]["offer"]["sdp"], "v=0 blind");
        assert_eq!(list[1]["to_peer_id"], bin_string(&[9u8; 20]));
        // answer 顶层化
        let ans = WsAnswer {
            offer_id: "abcd-0001".into(),
            sdp: "v=0 ans".into(),
            to_peer_id: [9u8; 20],
        };
        let v2 = build_answer_announce_json(&[7u8; 20], &[8u8; 20], &ans);
        assert_eq!(v2["answer"]["type"], "answer");
        assert_eq!(v2["to_peer_id"], bin_string(&[9u8; 20]));
        assert_eq!(v2["offer_id"], "abcd-0001");
        assert!(v2.get("offers").is_none());
    }

    #[tokio::test]
    async fn ws_announce_round_trip_with_local_server() {
        use futures::{SinkExt, StreamExt};
        use tokio_tungstenite::accept_async;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(sock).await.unwrap();
            let msg = ws.next().await.unwrap().unwrap();
            let Message::Text(t) = msg else {
                panic!("应为文本帧");
            };
            let req: serde_json::Value = serde_json::from_str(&t).unwrap();
            assert_eq!(req["action"], "announce");
            assert_eq!(req["info_hash"], bin_string(&[7u8; 20]));
            assert_eq!(req["peer_id"], bin_string(&[8u8; 20]));
            assert_eq!(req["port"], 6881);
            assert_eq!(req["uploaded"], 10);
            assert_eq!(req["downloaded"], 20);
            assert_eq!(req["left"], 30);
            assert_eq!(req["event"], "started");
            assert_eq!(req["numwant"], 50);
            let offers = req["offers"].as_array().unwrap();
            assert_eq!(offers.len(), 1);
            assert_eq!(offers[0]["offer_id"], "offer-1");
            assert_eq!(offers[0]["offer"]["type"], "offer");
            assert_eq!(offers[0]["offer"]["sdp"], "v=0 offer");
            assert_eq!(offers[0]["to_peer_id"], bin_string(&[9u8; 20]));
            let resp = serde_json::json!({
                "interval": 25,
                "complete": 3,
                "incomplete": 1,
                "peers": [{"ip": "9.9.9.9", "port": 7000, "peer_id": hex_lower(&[9u8; 20])}],
                "offers_to_answer": [
                    {"offer_id": "in-offer", "sdp": "v=0 offer2", "peer_id": hex_lower(&[10u8; 20])}
                ],
                "answers": [
                    {"offer_id": "offer-1", "sdp": "v=0 answer", "peer_id": hex_lower(&[9u8; 20])}
                ]
            });
            ws.send(Message::Text(resp.to_string())).await.unwrap();
            let _ = ws.close(None).await;
        });
        let p = AnnounceParams {
            infohash: [7u8; 20],
            peer_id: [8u8; 20],
            port: 6881,
            uploaded: 10,
            downloaded: 20,
            left: 30,
            event: Some(TrackerEvent::Started),
            numwant: 50,
        };
        let offers = vec![WsOffer {
            offer_id: "offer-1".into(),
            sdp: "v=0 offer".into(),
            to_peer_id: [9u8; 20],
        }];
        let r = announce_ws(&format!("ws://{addr}/announce"), &p, &offers, None)
            .await
            .unwrap();
        assert_eq!(r.interval, 25);
        assert_eq!(r.complete, 3);
        assert_eq!(r.incomplete, 1);
        assert_eq!(r.peers.len(), 1);
        assert_eq!(r.peers[0].ip, IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9)));
        assert_eq!(r.peer_ids, vec![[9u8; 20]]);
        assert_eq!(r.offers_to_answer.len(), 1);
        assert_eq!(r.offers_to_answer[0].sdp, "v=0 offer2");
        assert_eq!(r.answers.len(), 1);
        assert_eq!(r.answers[0].offer_id, "offer-1");
        server.await.unwrap();
    }

    #[test]
    fn hex_lower_round_trip() {
        let raw: Vec<u8> = (0u8..=255).collect();
        let hx = hex_lower(&raw);
        assert_eq!(hx.len(), 512);
        for (i, chunk) in hx.as_bytes().chunks(2).enumerate() {
            let hi = (chunk[0] as char).to_digit(16).unwrap();
            let lo = (chunk[1] as char).to_digit(16).unwrap();
            assert_eq!(((hi << 4) | lo) as u8, raw[i]);
        }
    }

    #[test]
    fn udp_host_port_extraction() {
        assert_eq!(
            udp_host_port("udp://tracker.example.com:6969/announce"),
            Some(("tracker.example.com".to_string(), 6969))
        );
        assert_eq!(
            udp_host_port("udp://127.0.0.1:8080"),
            Some(("127.0.0.1".to_string(), 8080))
        );
        assert_eq!(
            udp_host_port("udp://[2001:db8::1]:6969/a"),
            Some(("2001:db8::1".to_string(), 6969))
        );
        assert_eq!(udp_host_port("udp://h:0"), None);
        assert_eq!(udp_host_port("udp://h:/a"), None);
        assert_eq!(udp_host_port("http://h:1"), None);
    }

    #[test]
    fn compact_peers4_layout() {
        let buf = [1, 2, 3, 4, 0x1A, 0xE1, 5, 6, 7, 8, 0x00, 0x50];
        assert_eq!(
            parse_compact_peers4(&buf),
            vec![
                PeerAddr {
                    ip: IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
                    port: 6881
                },
                PeerAddr {
                    ip: IpAddr::V4(Ipv4Addr::new(5, 6, 7, 8)),
                    port: 80
                },
            ]
        );
        assert!(parse_compact_peers4(&[]).is_empty());
        let mut tail = buf.to_vec();
        tail.push(0xEE);
        assert_eq!(parse_compact_peers4(&tail).len(), 2);
    }

    #[test]
    fn compact_peers6_layout() {
        let mut buf = vec![0x20u8, 0x01, 0x0D, 0xB8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        buf.extend_from_slice(&[0x1A, 0xE1]);
        assert_eq!(
            parse_compact_peers6(&buf),
            vec![PeerAddr {
                ip: IpAddr::V6("2001:db8::1".parse().unwrap()),
                port: 6881
            }]
        );
        assert!(parse_compact_peers6(&[]).is_empty());
        buf.push(0xEE);
        assert_eq!(parse_compact_peers6(&buf).len(), 1);
    }

    #[test]
    fn http_response_full_parse() {
        let peers4 = [1u8, 2, 3, 4, 0x1A, 0xE1, 5, 6, 7, 8, 0x1A, 0xE2];
        let mut peers6 = vec![0x20u8, 0x01, 0x0D, 0xB8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        peers6.extend_from_slice(&[0x1A, 0xE3]);
        let body = bencode::encode(&dict(&[
            ("complete", BValue::Int(3)),
            ("incomplete", BValue::Int(4)),
            ("interval", BValue::Int(120)),
            ("min interval", BValue::Int(30)),
            ("peers", BValue::Bytes(peers4.to_vec())),
            ("peers6", BValue::Bytes(peers6)),
        ]));
        let r = parse_http_response(&body).unwrap();
        assert_eq!(r.interval, 120);
        assert_eq!(r.min_interval, Some(30));
        assert_eq!(r.complete, 3);
        assert_eq!(r.incomplete, 4);
        assert_eq!(r.peers.len(), 3);
        assert_eq!(r.peers[0].ip, IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)));
        assert_eq!(r.peers[0].port, 6881);
        assert_eq!(r.peers[2].ip, IpAddr::V6("2001:db8::1".parse().unwrap()));
        assert_eq!(r.peers[2].port, 6883);
    }

    #[test]
    fn http_response_defaults_and_dict_model_peers() {
        let body = bencode::encode(&dict(&[(
            "peers",
            BValue::List(vec![dict(&[
                ("ip", BValue::Bytes(b"9.9.9.9".to_vec())),
                ("port", BValue::Int(7000)),
            ])]),
        )]));
        let r = parse_http_response(&body).unwrap();
        assert_eq!(r.interval, 60);
        assert_eq!(r.min_interval, None);
        assert_eq!(r.complete, 0);
        assert_eq!(r.incomplete, 0);
        assert_eq!(
            r.peers,
            vec![PeerAddr {
                ip: "9.9.9.9".parse().unwrap(),
                port: 7000
            }]
        );
    }

    #[test]
    fn http_response_failure_reason_is_tracker_error() {
        let body = bencode::encode(&dict(&[(
            "failure reason",
            BValue::Bytes(b"torrent not registered".to_vec()),
        )]));
        let err = parse_http_response(&body).unwrap_err();
        assert!(
            matches!(err, BtError::Tracker(ref m) if m.contains("torrent not registered")),
            "failure reason 应转为 tracker 错误：{err:?}"
        );
    }

    #[test]
    fn http_response_invalid_bencode_rejected() {
        assert!(parse_http_response(b"not-bencode").is_err());
    }

    #[test]
    fn udp_header_validation() {
        let txid = 0xDEAD_BEEFu32;
        let mut ok = Vec::new();
        ok.extend_from_slice(&0u32.to_be_bytes());
        ok.extend_from_slice(&txid.to_be_bytes());
        ok.extend_from_slice(&0x1122_3344_5566_7788u64.to_be_bytes());
        assert_eq!(
            parse_connect_response(&ok, txid).unwrap(),
            0x1122_3344_5566_7788
        );
        let mut bad_tx = ok.clone();
        bad_tx[4] = 0x01;
        assert!(parse_connect_response(&bad_tx, txid).is_err());
        let mut err = Vec::new();
        err.extend_from_slice(&3u32.to_be_bytes());
        err.extend_from_slice(&txid.to_be_bytes());
        err.extend_from_slice(b"no such torrent");
        let e = parse_connect_response(&err, txid).unwrap_err();
        assert!(
            e.to_string().contains("no such torrent"),
            "错误消息应透出：{e}"
        );
        assert!(parse_announce_response(&ok, txid).is_err());
    }

    #[tokio::test]
    async fn http_announce_round_trip_with_mock_server() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 2048];
            let mut head = Vec::new();
            loop {
                let n = sock.read(&mut buf).await.unwrap();
                assert!(n > 0, "对端提前关闭");
                head.extend_from_slice(&buf[..n]);
                if head.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let head_str = String::from_utf8_lossy(&head);
            assert!(head_str.starts_with("GET /announce?"), "请求行：{head_str}");
            assert!(
                head_str.contains(
                    "info_hash=%00%7F%80%FF%61%2E%2D%5F%7E%01%02%03%04%05%06%07%08%09%0A%0B"
                ),
                "info_hash 应逐字节编码（FR-02-14）：{head_str}"
            );
            assert!(
                head_str.contains(
                    "peer_id=%2D%45%5A%32%30%30%30%2D%61%62%63%64%65%66%67%68%69%6A%30%31"
                ),
                "peer_id 应逐字节编码：{head_str}"
            );
            assert!(head_str.contains("port=6881"));
            assert!(head_str.contains("uploaded=10&downloaded=20&left=30"));
            assert!(head_str.contains("compact=1"));
            assert!(head_str.contains("event=started"));
            assert!(head_str.contains("numwant=50"));
            let body = bencode::encode(&dict(&[
                ("complete", BValue::Int(2)),
                ("incomplete", BValue::Int(1)),
                ("interval", BValue::Int(90)),
                ("peers", BValue::Bytes(vec![9, 9, 9, 9, 0x1A, 0xE1])),
            ]));
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
            sock.write_all(&body).await.unwrap();
        });
        let client = reqwest::Client::new();
        let p = AnnounceParams {
            infohash: [
                0x00, 0x7F, 0x80, 0xFF, b'a', b'.', b'-', b'_', b'~', 1, 2, 3, 4, 5, 6, 7, 8, 9,
                10, 11,
            ],
            peer_id: *b"-EZ2000-abcdefghij01",
            port: 6881,
            uploaded: 10,
            downloaded: 20,
            left: 30,
            event: Some(TrackerEvent::Started),
            numwant: 50,
        };
        let r = announce_http(&format!("http://{addr}/announce"), &p, &client)
            .await
            .unwrap();
        assert_eq!(r.interval, 90);
        assert_eq!(r.min_interval, None);
        assert_eq!(r.complete, 2);
        assert_eq!(r.incomplete, 1);
        assert_eq!(
            r.peers,
            vec![PeerAddr {
                ip: IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9)),
                port: 6881
            }]
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn udp_announce_full_handshake_with_mock_tracker() {
        let tracker = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let tport = tracker.local_addr().unwrap().port();
        let p = AnnounceParams {
            infohash: [7u8; 20],
            peer_id: [8u8; 20],
            port: 6881,
            uploaded: 1,
            downloaded: 2,
            left: 3,
            event: Some(TrackerEvent::Started),
            numwant: 50,
        };
        let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let handle = tokio::spawn(async move {
            announce_udp(
                sock,
                "127.0.0.1",
                tport,
                &p,
                DEFAULT_UDP_BASE_TIMEOUT_SECS,
                DEFAULT_UDP_RETRIES,
            )
            .await
        });
        let mut buf = [0u8; 512];
        let (n, from) = tracker.recv_from(&mut buf).await.unwrap();
        assert_eq!(n, 16);
        assert_eq!(
            u64::from_be_bytes(buf[0..8].try_into().unwrap()),
            0x417_2710_1980,
            "BEP 15 魔数"
        );
        assert_eq!(be32(&buf[8..12]), 0);
        let txid = be32(&buf[12..16]);
        let mut resp = Vec::new();
        resp.extend_from_slice(&0u32.to_be_bytes());
        resp.extend_from_slice(&txid.to_be_bytes());
        resp.extend_from_slice(&0x0123_4567_89AB_CDEFu64.to_be_bytes());
        tracker.send_to(&resp, &from).await.unwrap();
        let (n, from) = tracker.recv_from(&mut buf).await.unwrap();
        assert_eq!(n, 98);
        assert_eq!(
            u64::from_be_bytes(buf[0..8].try_into().unwrap()),
            0x0123_4567_89AB_CDEF,
            "connection_id 应回传"
        );
        assert_eq!(be32(&buf[8..12]), 1);
        assert_eq!(&buf[16..36], &[7u8; 20], "info_hash");
        assert_eq!(&buf[36..56], &[8u8; 20], "peer_id");
        assert_eq!(
            u64::from_be_bytes(buf[56..64].try_into().unwrap()),
            2,
            "downloaded"
        );
        assert_eq!(
            u64::from_be_bytes(buf[64..72].try_into().unwrap()),
            3,
            "left"
        );
        assert_eq!(
            u64::from_be_bytes(buf[72..80].try_into().unwrap()),
            1,
            "uploaded"
        );
        assert_eq!(be32(&buf[80..84]), 2, "event=started → 2");
        assert_eq!(be32(&buf[92..96]), 50, "num_want");
        assert_eq!(be16(&buf[96..98]), 6881, "port");
        let atxid = be32(&buf[12..16]);
        let mut resp = vec![0u8; 26];
        resp[0..4].copy_from_slice(&1u32.to_be_bytes());
        resp[4..8].copy_from_slice(&atxid.to_be_bytes());
        resp[8..12].copy_from_slice(&30u32.to_be_bytes());
        resp[12..16].copy_from_slice(&2u32.to_be_bytes());
        resp[16..20].copy_from_slice(&5u32.to_be_bytes());
        resp[20..26].copy_from_slice(&[9, 9, 9, 9, 0x1A, 0xE1]);
        tracker.send_to(&resp, &from).await.unwrap();
        let out = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("announce 应在超时保护内完成")
            .unwrap()
            .unwrap();
        assert_eq!(out.interval, 30);
        assert_eq!(out.incomplete, 2, "leechers → incomplete");
        assert_eq!(out.complete, 5, "seeders → complete");
        assert_eq!(
            out.peers,
            vec![PeerAddr {
                ip: IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9)),
                port: 6881
            }]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn udp_announce_gives_up_after_retries_when_silent() {
        let silent = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let port = silent.local_addr().unwrap().port();
        let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let p = AnnounceParams {
            infohash: [1u8; 20],
            peer_id: [2u8; 20],
            port: 6881,
            uploaded: 0,
            downloaded: 0,
            left: 100,
            event: None,
            numwant: 50,
        };
        let res = announce_udp(
            sock,
            "127.0.0.1",
            port,
            &p,
            DEFAULT_UDP_BASE_TIMEOUT_SECS,
            DEFAULT_UDP_RETRIES,
        )
        .await;
        let err = res.expect_err("静默 tracker 应在重试耗尽后判失败");
        assert!(err.to_string().contains("无响应"), "应报无响应：{err}");
    }
}
