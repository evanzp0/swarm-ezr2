use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_gathering_state::RTCIceGatheringState;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

use super::tracker::{hex_lower, WsAnswer, WsSignal};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PeerTransport {
    Tcp,
    WebRtc,
}

impl PeerTransport {
    #[must_use]
    pub fn parse(s: &str) -> Option<PeerTransport> {
        let lower = s.trim().to_ascii_lowercase();
        match lower.as_str() {
            "tcp" => Some(PeerTransport::Tcp),
            "webrtc" => Some(PeerTransport::WebRtc),
            _ => None,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            PeerTransport::Tcp => "TCP",
            PeerTransport::WebRtc => "WebRTC",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerTransports {
    pub tcp: bool,
    pub webrtc: bool,
}

impl Default for PeerTransports {
    fn default() -> Self {
        Self {
            tcp: true,
            webrtc: true,
        }
    }
}

impl PeerTransports {
    #[must_use]
    pub fn allows(self, t: PeerTransport) -> bool {
        match t {
            PeerTransport::Tcp => self.tcp,
            PeerTransport::WebRtc => self.webrtc,
        }
    }
}

const OUTBOUND_QUEUE_CAP: usize = 64;
const OFFER_TIMEOUT_SECS: u64 = 45;
pub const OFFERS_PER_ANNOUNCE: usize = 4;
pub const PENDING_OFFER_CAP: usize = 8;
const ICE_GATHER_TIMEOUT_MS: u64 = 3000;

/// ICE 服务器：沙箱环境实测 `stun.l.google.com` 不可达、cloudflare 可通，
/// 故 cloudflare 在前；google 作为公网环境下的备选（webrtc-rs 会逐个尝试）。
const STUN_SERVERS: &[&str] = &[
    "stun:stun.cloudflare.com:3478",
    "stun:stun.l.google.com:19302",
];

/// 发起侧远端 peer_id 回填槽：盲 offer 发出时尚不知对端身份，
/// answer 推送到达后由会话层回填，DataChannel 打开时据此生成 RtcReady。
pub type RemoteSlot = Arc<std::sync::Mutex<Option<[u8; 20]>>>;

pub struct RtcReady {
    pub stream: DataChannelStream,
    pub initiator: bool,
    pub remote_peer_id: [u8; 20],
}

#[derive(Clone, Debug, Default)]
pub struct RtcCandidate {
    pub fails: u32,
    pub last_try: Option<Instant>,
}

impl RtcCandidate {
    #[must_use]
    pub fn cooldown_ok(&self, base: u64, cap: u64) -> bool {
        let Some(t) = self.last_try else {
            return true;
        };
        let wait = backoff_secs(self.fails, base, cap);
        t.elapsed() >= Duration::from_secs(wait)
    }
}

const fn backoff_secs(fails: u32, base: u64, cap: u64) -> u64 {
    if base == 0 {
        return cap;
    }
    let mut d = base;
    let mut i = 1;
    while i < fails && d < cap {
        d = d.saturating_mul(2);
        i += 1;
    }
    if d > cap {
        cap
    } else {
        d
    }
}

pub struct PendingOffer {
    pub to_peer_id: [u8; 20],
    pub pc: Arc<RTCPeerConnection>,
    pub answered: bool,
    pub created: Instant,
    pub remote_slot: RemoteSlot,
}

#[derive(Default)]
pub struct SignalState {
    pub swarm_peers: HashSet<[u8; 20]>,
    pub candidates: HashMap<[u8; 20], RtcCandidate>,
    pub pending_offers: HashMap<String, PendingOffer>,
    pub active_peers: HashSet<[u8; 20]>,
    pub pcs: Vec<Arc<RTCPeerConnection>>,
}

impl SignalState {
    #[must_use]
    pub fn is_busy_with(&self, peer_id: &[u8; 20]) -> bool {
        self.active_peers.contains(peer_id)
            || self
                .pending_offers
                .values()
                .any(|po| po.to_peer_id.as_slice() == peer_id.as_slice())
    }

    pub fn mark_peer_fail(&mut self, peer_id: &[u8; 20]) {
        let c = self.candidates.entry(*peer_id).or_default();
        c.fails = c.fails.saturating_add(1);
        c.last_try = Some(Instant::now());
    }

    pub fn mark_peer_alive(&mut self, peer_id: &[u8; 20]) {
        self.active_peers.insert(*peer_id);
        self.candidates.remove(peer_id);
    }

    pub fn mark_peer_closed(&mut self, peer_id: &[u8; 20]) {
        self.active_peers.remove(peer_id);
        self.mark_peer_fail(peer_id);
    }

    /// 本端主动收口（缩容 shed / 空闲置换 / 自身拒绝）：撤出活跃集并清除
    /// 失败冷却，使下一轮 announce 的 plan_offer_targets 可立即重新选中
    /// 该 peer（区别于异常断开的 mark_peer_fail 惩罚性退避；bug28③）。
    pub fn mark_peer_idle(&mut self, peer_id: &[u8; 20]) {
        self.active_peers.remove(peer_id);
        self.candidates.remove(peer_id);
    }

    #[must_use]
    pub fn plan_offer_targets(
        &self,
        our_peer_id: &[u8; 20],
        max: usize,
        retry_base: u64,
        retry_cap: u64,
    ) -> Vec<[u8; 20]> {
        let mut targets: Vec<[u8; 20]> = self
            .swarm_peers
            .iter()
            .filter(|p| p.as_slice() > our_peer_id.as_slice())
            .filter(|p| !self.is_busy_with(p))
            .filter(|p| {
                self.candidates
                    .get(*p)
                    .is_none_or(|c| c.cooldown_ok(retry_base, retry_cap))
            })
            .copied()
            .collect();
        targets.sort();
        targets.truncate(max);
        targets
    }

    pub fn prune_expired(&mut self) -> Vec<[u8; 20]> {
        let ids: Vec<String> = self
            .pending_offers
            .iter()
            .filter(|(_, po)| {
                !po.answered && po.created.elapsed() >= Duration::from_secs(OFFER_TIMEOUT_SECS)
            })
            .map(|(k, _)| k.clone())
            .collect();
        let mut expired: Vec<[u8; 20]> = Vec::new();
        for id in ids {
            if let Some(po) = self.pending_offers.remove(&id) {
                tokio::spawn(async move {
                    let _ = po.pc.close().await;
                });
                expired.push(po.to_peer_id);
            }
        }
        for p in &expired {
            self.mark_peer_fail(p);
        }
        expired
    }
}

struct Inbox {
    chunks: VecDeque<(Vec<u8>, usize)>,
    closed: bool,
    readers: Vec<Waker>,
    overflow: Vec<Waker>,
}

impl Inbox {
    fn wake_readers(&mut self) {
        for w in self.readers.drain(..) {
            w.wake();
        }
    }

    fn wake_writers(&mut self) {
        for w in self.overflow.drain(..) {
            w.wake();
        }
    }
}

pub struct DataChannelStream {
    dc: Arc<RTCDataChannel>,
    inbox: Arc<Mutex<Inbox>>,
    tx: mpsc::Sender<Vec<u8>>,
    closed: Arc<AtomicBool>,
}

impl DataChannelStream {
    #[must_use]
    pub fn new(dc: Arc<RTCDataChannel>) -> DataChannelStream {
        let inbox = Arc::new(Mutex::new(Inbox {
            chunks: VecDeque::new(),
            closed: false,
            readers: Vec::new(),
            overflow: Vec::new(),
        }));
        let closed = Arc::new(AtomicBool::new(false));
        {
            let inbox = Arc::clone(&inbox);
            dc.on_message(Box::new(move |m: DataChannelMessage| {
                let inbox = Arc::clone(&inbox);
                Box::pin(async move {
                    if m.is_string {
                        return;
                    }
                    let mut g = inbox.lock().expect("dc inbox 锁");
                    g.chunks.push_back((m.data.to_vec(), 0));
                    g.wake_readers();
                })
            }));
        }
        {
            let inbox = Arc::clone(&inbox);
            let closed = Arc::clone(&closed);
            dc.on_close(Box::new(move || {
                let inbox = Arc::clone(&inbox);
                let closed = Arc::clone(&closed);
                Box::pin(async move {
                    closed.store(true, Ordering::SeqCst);
                    let mut g = inbox.lock().expect("dc inbox 锁");
                    g.closed = true;
                    g.wake_readers();
                    g.wake_writers();
                })
            }));
        }
        {
            let inbox = Arc::clone(&inbox);
            let closed = Arc::clone(&closed);
            dc.on_error(Box::new(move |_| {
                let inbox = Arc::clone(&inbox);
                let closed = Arc::clone(&closed);
                Box::pin(async move {
                    closed.store(true, Ordering::SeqCst);
                    let mut g = inbox.lock().expect("dc inbox 锁");
                    g.closed = true;
                    g.wake_readers();
                    g.wake_writers();
                })
            }));
        }
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(OUTBOUND_QUEUE_CAP);
        {
            let dc = Arc::clone(&dc);
            let inbox = Arc::clone(&inbox);
            let closed = Arc::clone(&closed);
            tokio::spawn(async move {
                while let Some(frame) = rx.recv().await {
                    let sent = dc.send(&frame.into()).await.is_ok();
                    if !sent {
                        closed.store(true, Ordering::SeqCst);
                    }
                    let mut g = inbox.lock().expect("dc inbox 锁");
                    if sent {
                        g.wake_writers();
                    } else {
                        g.closed = true;
                        g.wake_readers();
                        g.wake_writers();
                    }
                    drop(g);
                    if !sent {
                        break;
                    }
                    if closed.load(Ordering::SeqCst) {
                        break;
                    }
                }
            });
        }
        DataChannelStream {
            dc,
            inbox,
            tx,
            closed,
        }
    }
}

impl AsyncRead for DataChannelStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut g = self.inbox.lock().expect("dc inbox 锁");
        if let Some((chunk, off)) = g.chunks.front_mut() {
            let remaining = &chunk[*off..];
            let n = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..n]);
            *off += n;
            if *off >= chunk.len() {
                g.chunks.pop_front();
            }
            return Poll::Ready(Ok(()));
        }
        if g.closed {
            return Poll::Ready(Ok(()));
        }
        g.readers.push(cx.waker().clone());
        Poll::Pending
    }
}

impl AsyncWrite for DataChannelStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.closed.load(Ordering::SeqCst) {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "datachannel closed",
            )));
        }
        match self.tx.try_reserve() {
            Ok(permit) => {
                permit.send(buf.to_vec());
                Poll::Ready(Ok(buf.len()))
            }
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                let mut g = self.inbox.lock().expect("dc inbox 锁");
                g.overflow.push(cx.waker().clone());
                Poll::Pending
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => Poll::Ready(Err(
                io::Error::new(io::ErrorKind::BrokenPipe, "datachannel sender dropped"),
            )),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.closed.store(true, Ordering::SeqCst);
        {
            let mut g = self.inbox.lock().expect("dc inbox 锁");
            g.closed = true;
            g.wake_readers();
            g.wake_writers();
        }
        let dc = Arc::clone(&self.dc);
        tokio::spawn(async move {
            let _ = dc.close().await;
        });
        Poll::Ready(Ok(()))
    }
}

pub async fn new_peer_connection() -> Result<Arc<RTCPeerConnection>, super::BtError> {
    let api = APIBuilder::new().build();
    let config = RTCConfiguration {
        ice_servers: STUN_SERVERS
            .iter()
            .map(|url| RTCIceServer {
                urls: vec![(*url).to_string()],
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let pc = api
        .new_peer_connection(config)
        .await
        .map_err(|e| super::BtError::Peer(format!("WebRTC PeerConnection 创建失败：{e}")))?;
    Ok(Arc::new(pc))
}

async fn wait_gather_complete(pc: &RTCPeerConnection) {
    let deadline = Instant::now() + Duration::from_millis(ICE_GATHER_TIMEOUT_MS);
    while Instant::now() < deadline && pc.ice_gathering_state() != RTCIceGatheringState::Complete {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn local_sdp_of(pc: &RTCPeerConnection) -> Result<String, super::BtError> {
    let d = pc.local_description().await;
    d.map_or_else(
        || Err(super::BtError::Peer("WebRTC local description 缺失".into())),
        |d| Ok(d.sdp),
    )
}

pub struct OfferBundle {
    pub offer_id: String,
    pub sdp: String,
    pub pc: Arc<RTCPeerConnection>,
    pub remote_slot: RemoteSlot,
}

pub async fn create_data_channel_offer(
    pc: &Arc<RTCPeerConnection>,
    our_peer_id: &[u8; 20],
    to_peer_id: Option<[u8; 20]>,
    rtc_tx: mpsc::Sender<RtcReady>,
) -> Result<OfferBundle, super::BtError> {
    let dc = pc
        .create_data_channel("data_channel", None)
        .await
        .map_err(|e| super::BtError::Peer(format!("WebRTC DataChannel 创建失败：{e}")))?;
    let stream = DataChannelStream::new(Arc::clone(&dc));
    let remote_slot: RemoteSlot = Arc::new(std::sync::Mutex::new(to_peer_id));
    let tx_open = rtc_tx.clone();
    let slot_open = Arc::clone(&remote_slot);
    dc.on_open(Box::new(move || {
        let tx_open = tx_open.clone();
        let stream = stream;
        let slot_open = Arc::clone(&slot_open);
        Box::pin(async move {
            let remote_peer_id = slot_open
                .lock()
                .map(|g| g.unwrap_or([0u8; 20]))
                .unwrap_or([0u8; 20]);
            let _ = tx_open
                .send(RtcReady {
                    stream,
                    initiator: true,
                    remote_peer_id,
                })
                .await;
        })
    }));
    let offer = pc
        .create_offer(None)
        .await
        .map_err(|e| super::BtError::Peer(format!("WebRTC offer 创建失败：{e}")))?;
    pc.set_local_description(offer)
        .await
        .map_err(|e| super::BtError::Peer(format!("WebRTC set_local_description 失败：{e}")))?;
    wait_gather_complete(pc).await;
    let sdp = local_sdp_of(pc).await?;
    Ok(OfferBundle {
        offer_id: next_offer_id(our_peer_id),
        sdp,
        pc: Arc::clone(pc),
        remote_slot,
    })
}

pub async fn create_data_channel_answer(
    signal: &WsSignal,
    rtc_tx: mpsc::Sender<RtcReady>,
) -> Result<(Arc<RTCPeerConnection>, WsAnswer), super::BtError> {
    let pc = new_peer_connection().await?;
    let tx_ready = rtc_tx.clone();
    let remote = signal.peer_id;
    let opened = Arc::new(AtomicBool::new(false));
    {
        let opened = Arc::clone(&opened);
        pc.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
            let tx_ready = tx_ready.clone();
            let opened = Arc::clone(&opened);
            Box::pin(async move {
                if opened.swap(true, Ordering::SeqCst) {
                    return;
                }
                let stream = DataChannelStream::new(dc);
                let _ = tx_ready
                    .send(RtcReady {
                        stream,
                        initiator: false,
                        remote_peer_id: remote,
                    })
                    .await;
            })
        }));
    }
    let remote_offer = RTCSessionDescription::offer(signal.sdp.clone())
        .map_err(|e| super::BtError::Peer(format!("WebRTC offer SDP 解析失败：{e}")))?;
    pc.set_remote_description(remote_offer)
        .await
        .map_err(|e| super::BtError::Peer(format!("WebRTC set_remote_description 失败：{e}")))?;
    let answer = pc
        .create_answer(None)
        .await
        .map_err(|e| super::BtError::Peer(format!("WebRTC answer 创建失败：{e}")))?;
    pc.set_local_description(answer)
        .await
        .map_err(|e| super::BtError::Peer(format!("WebRTC set_local_description 失败：{e}")))?;
    wait_gather_complete(&pc).await;
    let sdp = local_sdp_of(&pc).await?;
    Ok((
        pc,
        WsAnswer {
            offer_id: signal.offer_id.clone(),
            sdp,
            to_peer_id: signal.peer_id,
        },
    ))
}

pub async fn accept_answer(
    pc: &Arc<RTCPeerConnection>,
    signal: &WsSignal,
) -> Result<(), super::BtError> {
    let remote_answer = RTCSessionDescription::answer(signal.sdp.clone())
        .map_err(|e| super::BtError::Peer(format!("WebRTC answer SDP 解析失败：{e}")))?;
    pc.set_remote_description(remote_answer)
        .await
        .map_err(|e| super::BtError::Peer(format!("WebRTC set_remote_description 失败：{e}")))?;
    Ok(())
}

fn next_offer_id(our_peer_id: &[u8; 20]) -> String {
    static SEQ: AtomicU64 = AtomicU64::new(1);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{}-{n:04x}", hex_lower(&our_peer_id[..4]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_parse_and_labels() {
        assert_eq!(PeerTransport::parse("tcp"), Some(PeerTransport::Tcp));
        assert_eq!(
            PeerTransport::parse(" WebRTC "),
            Some(PeerTransport::WebRtc)
        );
        assert_eq!(PeerTransport::parse("TCP"), Some(PeerTransport::Tcp));
        assert_eq!(PeerTransport::parse("quic"), None);
        assert_eq!(PeerTransport::parse(""), None);
        assert_eq!(PeerTransport::Tcp.label(), "TCP");
        assert_eq!(PeerTransport::WebRtc.label(), "WebRTC");
    }

    #[test]
    fn transports_default_and_gate() {
        let t = PeerTransports::default();
        assert!(t.allows(PeerTransport::Tcp));
        assert!(t.allows(PeerTransport::WebRtc));
        let only_tcp = PeerTransports {
            tcp: true,
            webrtc: false,
        };
        assert!(only_tcp.allows(PeerTransport::Tcp));
        assert!(!only_tcp.allows(PeerTransport::WebRtc));
    }

    #[test]
    fn signal_state_role_split_by_peer_id() {
        let mut st = SignalState::default();
        let ours = [1u8; 20];
        let bigger = [2u8; 20];
        let smaller = [0u8; 20];
        st.swarm_peers.insert(bigger);
        st.swarm_peers.insert(smaller);
        assert_eq!(st.plan_offer_targets(&ours, 4, 5, 60), vec![bigger]);
        let mirrored = st.plan_offer_targets(&bigger, 4, 5, 60);
        assert!(mirrored.is_empty(), "较大 peer_id 一侧不发 offer");
    }

    #[test]
    fn signal_state_busy_marks_and_cooldown() {
        let mut st = SignalState::default();
        let ours = [1u8; 20];
        let p = [2u8; 20];
        st.swarm_peers.insert(p);
        st.mark_peer_alive(&p);
        assert!(st.is_busy_with(&p));
        assert!(st.plan_offer_targets(&ours, 4, 5, 60).is_empty());
        st.mark_peer_closed(&p);
        assert!(!st.is_busy_with(&p));
        assert!(!st.candidates[&p].cooldown_ok(5, 60), "刚失败应有冷却");
        st.candidates.get_mut(&p).unwrap().last_try = None;
        assert!(st.candidates[&p].cooldown_ok(5, 60));
    }

    #[test]
    fn signal_state_idle_close_allows_immediate_reoffer() {
        // bug28③：本端主动收口（shed/置换）不得把健康 peer 记为失败——
        // mark_peer_idle 清冷却，下一轮 plan_offer_targets 立即可重新选中。
        let mut st = SignalState::default();
        let ours = [1u8; 20];
        let p = [2u8; 20];
        st.swarm_peers.insert(p);
        st.mark_peer_alive(&p);
        st.mark_peer_idle(&p);
        assert!(!st.is_busy_with(&p), "活跃集应撤出");
        assert!(!st.candidates.contains_key(&p), "主动收口应清失败冷却");
        assert_eq!(
            st.plan_offer_targets(&ours, 4, 5, 60),
            vec![p],
            "收口后无需退避，立即重新 offer"
        );
    }

    #[test]
    fn backoff_progression() {
        assert_eq!(backoff_secs(0, 5, 60), 5);
        assert_eq!(backoff_secs(1, 5, 60), 5);
        assert_eq!(backoff_secs(2, 5, 60), 10);
        assert_eq!(backoff_secs(3, 5, 60), 20);
        assert_eq!(backoff_secs(10, 5, 60), 60);
        assert_eq!(backoff_secs(2, 0, 60), 60);
    }

    #[test]
    fn offer_id_unique_and_prefixed() {
        let pid = [9u8; 20];
        let a = next_offer_id(&pid);
        let b = next_offer_id(&pid);
        assert_ne!(a, b);
        assert!(a.starts_with("09090909-"));
    }
}
