use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::net::UdpSocket;

use super::bencode::{self, BValue};
use super::BtError;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TrackerUrl {
    Http(String),
    Udp(String),
}

impl TrackerUrl {
    #[must_use]
    pub fn parse(s: &str) -> Option<TrackerUrl> {
        let lower = s.to_ascii_lowercase();
        let rest = lower
            .strip_prefix("https://")
            .or_else(|| lower.strip_prefix("http://"))
            .or_else(|| lower.strip_prefix("udp://"))?;
        if rest.is_empty() {
            return None;
        }
        if lower.starts_with("udp://") {
            udp_host_port(s)?;
            Some(TrackerUrl::Udp(s.to_string()))
        } else {
            Some(TrackerUrl::Http(s.to_string()))
        }
    }
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
const UDP_BASE_TIMEOUT_SECS: u64 = 15;
const UDP_RETRIES: u32 = 3;
const UDP_RECV_BUF: usize = 4096;

pub async fn announce_udp(
    sock: UdpSocket,
    host: &str,
    port: u16,
    p: &AnnounceParams,
) -> Result<TrackerResponse, BtError> {
    let addr = resolve_udp_target(host, port).await?;
    let connect_txid = rand_u32();
    let req = build_connect_request(connect_txid);
    let conn_id = udp_roundtrip(&sock, addr, &req, connect_txid, parse_connect_response).await?;
    let announce_txid = rand_u32();
    let req = build_announce_request(conn_id, announce_txid, p);
    udp_roundtrip(&sock, addr, &req, announce_txid, parse_announce_response).await
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
) -> Result<T, BtError> {
    let mut wait_secs = UDP_BASE_TIMEOUT_SECS;
    let mut buf = vec![0u8; UDP_RECV_BUF];
    let mut last_err: Option<BtError> = None;
    for attempt in 0..=UDP_RETRIES {
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
        let handle = tokio::spawn(async move { announce_udp(sock, "127.0.0.1", tport, &p).await });
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
        let res = announce_udp(sock, "127.0.0.1", port, &p).await;
        let err = res.expect_err("静默 tracker 应在重试耗尽后判失败");
        assert!(err.to_string().contains("无响应"), "应报无响应：{err}");
    }
}
