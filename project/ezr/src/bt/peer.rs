use std::collections::BTreeMap;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::bencode::{self, BValue};
use super::{BtError, REQUEST_BLOCK_LEN};
#[cfg(test)]
use crate::model::config::DEFAULT_HANDSHAKE_TIMEOUT_SECS;

pub const MAX_MSG_LEN: u32 = REQUEST_BLOCK_LEN * 4;

const PSTR: &[u8; 19] = b"BitTorrent protocol";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandshakeOk {
    pub reserved: [u8; 8],
    pub peer_id: [u8; 20],
}

#[must_use]
pub const fn reserved_bits(ext_metadata: bool) -> [u8; 8] {
    let mut r = [0u8; 8];
    if ext_metadata {
        r[5] = 0x10;
    }
    r
}

pub async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(
    io: &mut S,
    infohash: &[u8; 20],
    peer_id: &[u8; 20],
    ext_metadata: bool,
    timeout_secs: u64,
) -> Result<HandshakeOk, BtError> {
    let mut out = Vec::with_capacity(68);
    out.push(19);
    out.extend_from_slice(PSTR);
    out.extend_from_slice(&reserved_bits(ext_metadata));
    out.extend_from_slice(infohash);
    out.extend_from_slice(peer_id);

    let exchange = async {
        io.write_all(&out).await?;
        io.flush().await?;
        let mut len_byte = [0u8; 1];
        io.read_exact(&mut len_byte).await?;
        if len_byte[0] != 19 {
            return Err(BtError::Peer(format!(
                "握手协议名长度非法：{}",
                len_byte[0]
            )));
        }
        let mut name = [0u8; 19];
        io.read_exact(&mut name).await?;
        if &name != PSTR {
            return Err(BtError::Peer("握手协议名不匹配".into()));
        }
        let mut reserved = [0u8; 8];
        io.read_exact(&mut reserved).await?;
        let mut their_infohash = [0u8; 20];
        io.read_exact(&mut their_infohash).await?;
        if &their_infohash != infohash {
            return Err(BtError::Peer("握手 infohash 不匹配".into()));
        }
        let mut their_peer_id = [0u8; 20];
        io.read_exact(&mut their_peer_id).await?;
        Ok(HandshakeOk {
            reserved,
            peer_id: their_peer_id,
        })
    };
    match tokio::time::timeout(Duration::from_secs(timeout_secs), exchange).await {
        Ok(r) => r,
        Err(_) => Err(BtError::Peer(format!("握手超时（{timeout_secs}s）"))),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeerMsg {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
    Bitfield(Vec<u8>),
    Request {
        index: u32,
        begin: u32,
        length: u32,
    },
    Block {
        index: u32,
        begin: u32,
        data: Vec<u8>,
    },
    Cancel {
        index: u32,
        begin: u32,
        length: u32,
    },
    Port(u16),
    Extended {
        ext_id: u8,
        payload: Vec<u8>,
    },
    Unknown(u8, Vec<u8>),
}

const fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

fn triplet(index: u32, begin: u32, length: u32) -> [u8; 12] {
    let mut b = [0u8; 12];
    b[0..4].copy_from_slice(&index.to_be_bytes());
    b[4..8].copy_from_slice(&begin.to_be_bytes());
    b[8..12].copy_from_slice(&length.to_be_bytes());
    b
}

fn frame(id: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + payload.len());
    out.extend_from_slice(&((payload.len() + 1) as u32).to_be_bytes());
    out.push(id);
    out.extend_from_slice(payload);
    out
}

#[must_use]
pub fn encode_msg(m: &PeerMsg) -> Vec<u8> {
    match m {
        PeerMsg::KeepAlive => vec![0, 0, 0, 0],
        PeerMsg::Choke => frame(0, &[]),
        PeerMsg::Unchoke => frame(1, &[]),
        PeerMsg::Interested => frame(2, &[]),
        PeerMsg::NotInterested => frame(3, &[]),
        PeerMsg::Have(index) => frame(4, &index.to_be_bytes()),
        PeerMsg::Bitfield(bits) => frame(5, bits),
        PeerMsg::Request {
            index,
            begin,
            length,
        } => frame(6, &triplet(*index, *begin, *length)),
        PeerMsg::Cancel {
            index,
            begin,
            length,
        } => frame(8, &triplet(*index, *begin, *length)),
        PeerMsg::Block { index, begin, data } => {
            let mut payload = Vec::with_capacity(8 + data.len());
            payload.extend_from_slice(&index.to_be_bytes());
            payload.extend_from_slice(&begin.to_be_bytes());
            payload.extend_from_slice(data);
            frame(7, &payload)
        }
        PeerMsg::Port(port) => frame(9, &port.to_be_bytes()),
        PeerMsg::Extended { ext_id, payload } => {
            let mut buf = Vec::with_capacity(payload.len() + 1);
            buf.push(*ext_id);
            buf.extend_from_slice(payload);
            frame(20, &buf)
        }
        PeerMsg::Unknown(id, payload) => frame(*id, payload),
    }
}

pub fn decode_msg(buf: &[u8]) -> Result<PeerMsg, BtError> {
    if buf.len() < 4 {
        return Err(BtError::Peer("消息帧缺失长度前缀".into()));
    }
    let len = be32(&buf[0..4]);
    if len == 0 {
        if buf.len() != 4 {
            return Err(BtError::Peer("keep-alive 帧带多余字节".into()));
        }
        return Ok(PeerMsg::KeepAlive);
    }
    if len > MAX_MSG_LEN {
        return Err(BtError::Peer(format!("消息超长：{len} > {MAX_MSG_LEN}")));
    }
    let body = buf.len() - 4;
    if body != len as usize {
        return Err(BtError::Peer(format!(
            "帧长度不符：声明 {len}，实际 {body}"
        )));
    }
    let id = buf[4];
    let payload = &buf[5..];
    match id {
        0..=3 => {
            if !payload.is_empty() {
                return Err(BtError::Peer(format!("消息 id {id} 不应携带载荷")));
            }
            Ok(match id {
                0 => PeerMsg::Choke,
                1 => PeerMsg::Unchoke,
                2 => PeerMsg::Interested,
                _ => PeerMsg::NotInterested,
            })
        }
        4 => {
            if payload.len() != 4 {
                return Err(BtError::Peer("have 载荷应 4 字节".into()));
            }
            Ok(PeerMsg::Have(be32(payload)))
        }
        5 => Ok(PeerMsg::Bitfield(payload.to_vec())),
        6 | 8 => {
            if payload.len() != 12 {
                return Err(BtError::Peer(format!("消息 id {id} 载荷应 12 字节")));
            }
            let msg = if id == 6 {
                PeerMsg::Request {
                    index: be32(&payload[0..4]),
                    begin: be32(&payload[4..8]),
                    length: be32(&payload[8..12]),
                }
            } else {
                PeerMsg::Cancel {
                    index: be32(&payload[0..4]),
                    begin: be32(&payload[4..8]),
                    length: be32(&payload[8..12]),
                }
            };
            Ok(msg)
        }
        7 => {
            if payload.len() < 8 {
                return Err(BtError::Peer("piece 消息载荷截断".into()));
            }
            Ok(PeerMsg::Block {
                index: be32(&payload[0..4]),
                begin: be32(&payload[4..8]),
                data: payload[8..].to_vec(),
            })
        }
        9 => {
            if payload.len() != 2 {
                return Err(BtError::Peer("port 载荷应 2 字节".into()));
            }
            Ok(PeerMsg::Port(u16::from_be_bytes([payload[0], payload[1]])))
        }
        20 => {
            let ext_id = *payload
                .first()
                .ok_or_else(|| BtError::Peer("扩展消息缺 ext_id".into()))?;
            Ok(PeerMsg::Extended {
                ext_id,
                payload: payload[1..].to_vec(),
            })
        }
        other => Ok(PeerMsg::Unknown(other, payload.to_vec())),
    }
}

pub async fn read_msg<S: AsyncRead + Unpin>(io: &mut S) -> Result<PeerMsg, BtError> {
    let mut prefix = [0u8; 4];
    io.read_exact(&mut prefix).await?;
    let len = be32(&prefix);
    if len == 0 {
        return Ok(PeerMsg::KeepAlive);
    }
    if len > MAX_MSG_LEN {
        return Err(BtError::Peer(format!(
            "消息超长：{len} > {MAX_MSG_LEN}（拒绝分配）"
        )));
    }
    let mut frame_buf = vec![0u8; 4 + len as usize];
    frame_buf[..4].copy_from_slice(&prefix);
    io.read_exact(&mut frame_buf[4..]).await?;
    decode_msg(&frame_buf)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UtMetadata {
    Request(u32),
    Data {
        piece: u32,
        total_size: u32,
        data: Vec<u8>,
    },
    Reject(u32),
}

fn ut_dict(msg_type: i64, piece: u32, total_size: Option<u32>) -> BValue {
    let mut m = BTreeMap::new();
    m.insert(b"msg_type".to_vec(), BValue::Int(msg_type));
    m.insert(b"piece".to_vec(), BValue::Int(i64::from(piece)));
    if let Some(ts) = total_size {
        m.insert(b"total_size".to_vec(), BValue::Int(i64::from(ts)));
    }
    BValue::Dict(m)
}

#[must_use]
pub fn encode_ut_metadata(m: &UtMetadata) -> Vec<u8> {
    match m {
        UtMetadata::Request(piece) => bencode::encode(&ut_dict(0, *piece, None)),
        UtMetadata::Data {
            piece,
            total_size,
            data,
        } => {
            let mut out = bencode::encode(&ut_dict(1, *piece, Some(*total_size)));
            out.extend_from_slice(data);
            out
        }
        UtMetadata::Reject(piece) => bencode::encode(&ut_dict(2, *piece, None)),
    }
}

#[must_use]
pub fn parse_ut_metadata(payload: &[u8]) -> Option<UtMetadata> {
    let (v, consumed) = bencode::decode(payload).ok()?;
    let msg_type = v.get_int("msg_type")?;
    let piece = u32::try_from(v.get_int("piece")?).ok()?;
    let rest = &payload[consumed..];
    match msg_type {
        0 => Some(UtMetadata::Request(piece)),
        2 => Some(UtMetadata::Reject(piece)),
        1 => {
            let total_size = u32::try_from(v.get_int("total_size")?).ok()?;
            Some(UtMetadata::Data {
                piece,
                total_size,
                data: rest.to_vec(),
            })
        }
        _ => None,
    }
}

#[must_use]
pub fn ext_handshake_payload(ut_metadata_id: u8, metadata_size: u32) -> Vec<u8> {
    let mut inner = BTreeMap::new();
    inner.insert(
        b"ut_metadata".to_vec(),
        BValue::Int(i64::from(ut_metadata_id)),
    );
    let mut outer = BTreeMap::new();
    outer.insert(b"m".to_vec(), BValue::Dict(inner));
    outer.insert(
        b"metadata_size".to_vec(),
        BValue::Int(i64::from(metadata_size)),
    );
    bencode::encode(&BValue::Dict(outer))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtHandshake {
    pub ut_metadata: Option<u8>,
    pub metadata_size: Option<u32>,
    pub v: Option<String>,
}

#[must_use]
pub fn parse_ext_handshake(payload: &[u8]) -> Option<ExtHandshake> {
    let v = bencode::decode_all(payload).ok()?;
    let ut = v
        .get("m")
        .and_then(|m| m.get_int("ut_metadata"))
        .and_then(|i| u8::try_from(i).ok());
    let size = v
        .get_int("metadata_size")
        .and_then(|i| u32::try_from(i).ok());
    let client = v.get_str("v").map(str::to_string);
    Some(ExtHandshake {
        ut_metadata: ut,
        metadata_size: size,
        v: client,
    })
}

#[cfg(test)]
mod tests {
    use tokio::io::duplex;

    use super::*;

    const DUR5: Duration = Duration::from_secs(5);

    #[test]
    fn reserved_bits_ext_metadata() {
        assert_eq!(reserved_bits(true), [0, 0, 0, 0, 0, 0x10, 0, 0]);
        assert_eq!(reserved_bits(false), [0u8; 8]);
    }

    #[test]
    fn max_msg_len_is_64k_guard() {
        assert_eq!(MAX_MSG_LEN, 64 * 1024);
        assert_eq!(MAX_MSG_LEN, REQUEST_BLOCK_LEN * 4);
    }

    #[test]
    fn msg_codec_round_trip_all_variants() {
        let cases = vec![
            PeerMsg::KeepAlive,
            PeerMsg::Choke,
            PeerMsg::Unchoke,
            PeerMsg::Interested,
            PeerMsg::NotInterested,
            PeerMsg::Have(7),
            PeerMsg::Bitfield(vec![0b1010_0001, 0xFF]),
            PeerMsg::Request {
                index: 1,
                begin: 2,
                length: REQUEST_BLOCK_LEN,
            },
            PeerMsg::Block {
                index: 3,
                begin: 4,
                data: vec![0xAB; 33],
            },
            PeerMsg::Cancel {
                index: 5,
                begin: 6,
                length: 7,
            },
            PeerMsg::Port(6881),
            PeerMsg::Extended {
                ext_id: 1,
                payload: b"d1:md11:ut_metadatai1ee".to_vec(),
            },
            PeerMsg::Unknown(99, b"xyz".to_vec()),
        ];
        for m in &cases {
            assert_eq!(
                decode_msg(&encode_msg(m)).unwrap(),
                *m,
                "round-trip 失败：{m:?}"
            );
        }
        assert_eq!(encode_msg(&PeerMsg::KeepAlive), vec![0, 0, 0, 0]);
    }

    #[test]
    fn decode_msg_rejects_bad_frames() {
        assert!(decode_msg(&[]).is_err());
        assert!(decode_msg(&[0, 0, 1]).is_err());
        let enc = encode_msg(&PeerMsg::Have(1));
        assert!(decode_msg(&enc[..enc.len() - 1]).is_err(), "截断帧应拒绝");
        assert!(decode_msg(&[0, 0, 0, 5, 4, 1, 2]).is_err(), "声明 5 实际 3");
        let mut extra = enc.clone();
        extra.push(0xEE);
        assert!(decode_msg(&extra).is_err(), "帧尾多余字节应拒绝");
        assert!(decode_msg(&[0, 0, 0, 0, 9]).is_err());
        assert!(decode_msg(&[0, 0, 0, 2, 0, 9]).is_err());
        assert!(decode_msg(&[0, 0, 0, 4, 4, 1]).is_err());
        assert!(decode_msg(&[0x00, 0x02, 0x00, 0x00]).is_err());
    }

    #[tokio::test]
    async fn read_msg_rejects_oversized_prefix() {
        let (mut a, mut b) = duplex(64);
        a.write_all(&[0x00, 0x02, 0x00, 0x00]).await.unwrap();
        drop(a);
        let err = read_msg(&mut b).await.unwrap_err();
        assert!(err.to_string().contains("超长"), "应报超长而非 EOF：{err}");
    }

    #[tokio::test]
    async fn read_msg_assembles_partial_writes() {
        let (mut a, mut b) = duplex(64);
        let enc = encode_msg(&PeerMsg::Block {
            index: 1,
            begin: 2,
            data: vec![0xAB; 10],
        });
        let writer = tokio::spawn(async move {
            a.write_all(&enc[..5]).await.unwrap();
            a.write_all(&enc[5..]).await.unwrap();
        });
        let m = read_msg(&mut b).await.unwrap();
        writer.await.unwrap();
        assert_eq!(
            m,
            PeerMsg::Block {
                index: 1,
                begin: 2,
                data: vec![0xAB; 10]
            },
            "分片写入应拼装为完整消息"
        );
    }

    #[tokio::test]
    async fn read_msg_streams_sequential_messages() {
        let (mut a, mut b) = duplex(256);
        let mut buf = encode_msg(&PeerMsg::Have(1));
        buf.extend_from_slice(&encode_msg(&PeerMsg::KeepAlive));
        buf.extend_from_slice(&encode_msg(&PeerMsg::Request {
            index: 9,
            begin: 8,
            length: 7,
        }));
        a.write_all(&buf).await.unwrap();
        drop(a);
        assert_eq!(read_msg(&mut b).await.unwrap(), PeerMsg::Have(1));
        assert_eq!(read_msg(&mut b).await.unwrap(), PeerMsg::KeepAlive);
        assert_eq!(
            read_msg(&mut b).await.unwrap(),
            PeerMsg::Request {
                index: 9,
                begin: 8,
                length: 7
            }
        );
        assert!(read_msg(&mut b).await.is_err());
    }

    async fn mock_peer_handshake<S: AsyncRead + AsyncWrite + Unpin>(
        io: &mut S,
        client_reserved: [u8; 8],
        reply_infohash: [u8; 20],
        reply_peer_id: [u8; 20],
    ) -> std::io::Result<()> {
        let mut head = [0u8; 28];
        io.read_exact(&mut head).await?;
        assert_eq!(head[0], 19, "pstr 长度");
        assert_eq!(&head[1..20], &PSTR[..], "协议名");
        assert_eq!(&head[20..28], &client_reserved, "客户端 reserved 扩展位");
        let mut client_ih = [0u8; 20];
        io.read_exact(&mut client_ih).await?;
        let mut out = Vec::with_capacity(68);
        out.push(19);
        out.extend_from_slice(&PSTR[..]);
        out.extend_from_slice(&client_reserved);
        out.extend_from_slice(&reply_infohash);
        out.extend_from_slice(&reply_peer_id);
        io.write_all(&out).await?;
        io.flush().await
    }

    #[tokio::test]
    async fn handshake_echoes_peer_and_reserved() {
        let (mut a, mut b) = duplex(128);
        let ih = [3u8; 20];
        let server_pid = [9u8; 20];
        let handle = tokio::spawn(async move {
            handshake(
                &mut a,
                &ih,
                &[4u8; 20],
                true,
                DEFAULT_HANDSHAKE_TIMEOUT_SECS,
            )
            .await
        });
        mock_peer_handshake(&mut b, reserved_bits(true), ih, server_pid)
            .await
            .unwrap();
        let ok = tokio::time::timeout(DUR5, handle)
            .await
            .expect("握手应在超时保护内完成")
            .unwrap()
            .unwrap();
        assert_eq!(ok.reserved, reserved_bits(true), "对端 reserved 原样回传");
        assert_eq!(ok.peer_id, server_pid, "对端 peer id 原样回传");
    }

    #[tokio::test]
    async fn handshake_rejects_infohash_mismatch() {
        let (mut a, mut b) = duplex(128);
        let handle = tokio::spawn(async move {
            handshake(
                &mut a,
                &[5u8; 20],
                &[6u8; 20],
                true,
                DEFAULT_HANDSHAKE_TIMEOUT_SECS,
            )
            .await
        });
        mock_peer_handshake(&mut b, reserved_bits(true), [7u8; 20], [8u8; 20])
            .await
            .unwrap();
        let res = tokio::time::timeout(DUR5, handle).await.unwrap().unwrap();
        let err = res.expect_err("infohash 不匹配应失败");
        assert!(
            err.to_string().contains("infohash"),
            "错误应指明 infohash：{err}"
        );
    }

    #[tokio::test]
    async fn handshake_rejects_bad_protocol_name() {
        let (mut a, mut b) = duplex(128);
        let t = tokio::spawn(async move {
            handshake(
                &mut a,
                &[1u8; 20],
                &[2u8; 20],
                false,
                DEFAULT_HANDSHAKE_TIMEOUT_SECS,
            )
            .await
        });
        let mut head = Vec::new();
        head.push(19);
        head.extend_from_slice(b"BitTorrent protoc0l");
        head.extend_from_slice(&[0u8; 8]);
        head.extend_from_slice(&[1u8; 20]);
        head.extend_from_slice(&[2u8; 20]);
        b.write_all(&head).await.unwrap();
        b.flush().await.unwrap();
        let err = tokio::time::timeout(DUR5, t)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert!(
            err.to_string().contains("协议名"),
            "应报协议名不匹配：{err}"
        );
        drop(b);
        let (mut a2, mut b2) = duplex(128);
        let t2 = tokio::spawn(async move {
            handshake(
                &mut a2,
                &[1u8; 20],
                &[2u8; 20],
                false,
                DEFAULT_HANDSHAKE_TIMEOUT_SECS,
            )
            .await
        });
        b2.write_all(&[5]).await.unwrap();
        let err2 = tokio::time::timeout(DUR5, t2)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert!(
            err2.to_string().contains("长度"),
            "应报协议名长度非法：{err2}"
        );
        drop(b2);
    }

    #[tokio::test(start_paused = true)]
    async fn handshake_times_out_when_peer_silent() {
        let (mut a, b) = duplex(128);
        let res = handshake(
            &mut a,
            &[1u8; 20],
            &[2u8; 20],
            false,
            DEFAULT_HANDSHAKE_TIMEOUT_SECS,
        )
        .await;
        let err = res.expect_err("静默对端应握手超时");
        assert!(err.to_string().contains("超时"), "应报握手超时：{err}");
        drop(b);
    }

    #[tokio::test]
    async fn handshake_fails_on_immediate_eof() {
        let (mut a, b) = duplex(128);
        drop(b);
        assert!(handshake(
            &mut a,
            &[1u8; 20],
            &[2u8; 20],
            false,
            DEFAULT_HANDSHAKE_TIMEOUT_SECS
        )
        .await
        .is_err());
    }

    #[test]
    fn ut_metadata_request_exact_bytes() {
        assert_eq!(
            encode_ut_metadata(&UtMetadata::Request(5)),
            b"d8:msg_typei0e5:piecei5ee".as_slice()
        );
        assert_eq!(
            encode_ut_metadata(&UtMetadata::Reject(2)),
            b"d8:msg_typei2e5:piecei2ee".as_slice()
        );
        assert_eq!(
            encode_ut_metadata(&UtMetadata::Data {
                piece: 1,
                total_size: 10,
                data: b"ZZ".to_vec(),
            }),
            b"d8:msg_typei1e5:piecei1e10:total_sizei10eeZZ".as_slice()
        );
    }

    #[test]
    fn ut_metadata_codec_round_trip() {
        let cases = vec![
            UtMetadata::Request(3),
            UtMetadata::Reject(7),
            UtMetadata::Data {
                piece: 0,
                total_size: 21,
                data: b"d4:infod6:lengthi1eee".to_vec(),
            },
            UtMetadata::Data {
                piece: 2,
                total_size: 16384,
                data: Vec::new(),
            },
        ];
        for m in &cases {
            let enc = encode_ut_metadata(m);
            assert_eq!(
                parse_ut_metadata(&enc).as_ref(),
                Some(m),
                "round-trip 失败：{m:?}"
            );
        }
    }

    #[test]
    fn ut_metadata_parse_rejects_garbage_and_unknown() {
        assert!(parse_ut_metadata(b"").is_none());
        assert!(parse_ut_metadata(b"garbage").is_none());
        let mut m = BTreeMap::new();
        m.insert(b"msg_type".to_vec(), BValue::Int(7));
        m.insert(b"piece".to_vec(), BValue::Int(0));
        let enc = bencode::encode(&BValue::Dict(m));
        assert!(parse_ut_metadata(&enc).is_none(), "未知 msg_type 应 None");
        let mut m2 = BTreeMap::new();
        m2.insert(b"msg_type".to_vec(), BValue::Int(0));
        let enc2 = bencode::encode(&BValue::Dict(m2));
        assert!(parse_ut_metadata(&enc2).is_none());
        let mut m3 = BTreeMap::new();
        m3.insert(b"msg_type".to_vec(), BValue::Int(1));
        m3.insert(b"piece".to_vec(), BValue::Int(0));
        let enc3 = bencode::encode(&BValue::Dict(m3));
        assert!(parse_ut_metadata(&enc3).is_none());
    }

    #[test]
    fn ext_handshake_round_trip() {
        let payload = ext_handshake_payload(2, 54321);
        assert_eq!(
            payload,
            b"d1:md11:ut_metadatai2ee13:metadata_sizei54321ee".as_slice(),
            "BEP 10 最小子集线格式金样"
        );
        assert_eq!(
            parse_ext_handshake(&payload),
            Some(ExtHandshake {
                ut_metadata: Some(2),
                metadata_size: Some(54321),
                v: None,
            })
        );
        assert_eq!(
            parse_ext_handshake(&ext_handshake_payload(1, 484)),
            Some(ExtHandshake {
                ut_metadata: Some(1),
                metadata_size: Some(484),
                v: None,
            })
        );
    }

    #[test]
    fn ext_handshake_v_capture() {
        let mut m = BTreeMap::new();
        m.insert(b"v".to_vec(), BValue::Bytes(b"BitComet 1.64".to_vec()));
        let payload = bencode::encode(&BValue::Dict(m));
        let eh = parse_ext_handshake(&payload).expect("仅 v 的扩展握手可解析");
        assert_eq!(eh.v.as_deref(), Some("BitComet 1.64"));
        assert_eq!(eh.ut_metadata, None);
        assert_eq!(eh.metadata_size, None);
    }

    #[test]
    fn ext_handshake_parse_tolerates_extra_keys_and_rejects_missing() {
        let mut m = BTreeMap::new();
        let mut md = BTreeMap::new();
        md.insert(b"ut_metadata".to_vec(), BValue::Int(3));
        md.insert(b"ut_pex".to_vec(), BValue::Int(4));
        m.insert(b"m".to_vec(), BValue::Dict(md));
        m.insert(b"metadata_size".to_vec(), BValue::Int(484));
        m.insert(b"p".to_vec(), BValue::Int(6881));
        let payload = bencode::encode(&BValue::Dict(m));
        let eh = parse_ext_handshake(&payload).expect("额外键容忍");
        assert_eq!(eh.ut_metadata, Some(3));
        assert_eq!(eh.metadata_size, Some(484));
        let mut m2 = BTreeMap::new();
        m2.insert(b"metadata_size".to_vec(), BValue::Int(484));
        let p2 = bencode::encode(&BValue::Dict(m2));
        let eh2 = parse_ext_handshake(&p2).expect("缺 m 容忍（v 优先口径）");
        assert_eq!(eh2.ut_metadata, None);
        assert_eq!(eh2.metadata_size, Some(484));
        let mut m3 = BTreeMap::new();
        let mut md3 = BTreeMap::new();
        md3.insert(b"ut_metadata".to_vec(), BValue::Int(1));
        m3.insert(b"m".to_vec(), BValue::Dict(md3));
        let p3 = bencode::encode(&BValue::Dict(m3));
        let eh3 = parse_ext_handshake(&p3).expect("缺 metadata_size 容忍");
        assert_eq!(eh3.ut_metadata, Some(1));
        assert_eq!(parse_ext_handshake(b"junk"), None);
    }
}
