use sha1::{Digest, Sha1};

use super::bencode::{self, BValue};
use super::BtError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorrentFile {
    pub path: String,
    pub length: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorrentMeta {
    pub info_bytes: Vec<u8>,
    pub infohash: [u8; 20],
    pub name: String,
    pub piece_length: u32,
    pub pieces: Vec<[u8; 20]>,
    pub files: Vec<TorrentFile>,
    pub announce: Option<String>,
    pub announce_list: Vec<String>,
}

impl TorrentMeta {
    pub fn parse(data: &[u8]) -> Result<TorrentMeta, BtError> {
        let (root, info_span) = bencode::decode_with_info_span(data)?;
        let info = root
            .get("info")
            .ok_or_else(|| BtError::Meta("缺 info 字典".into()))?;

        let info_bytes = match &info_span {
            Some(span) => data[span.clone()].to_vec(),
            None => return Err(BtError::Meta("info 区间定位失败".into())),
        };
        let recheck = bencode::decode_all(&info_bytes)?;
        if &recheck != info {
            return Err(BtError::Meta("info 精确字节切片错位".into()));
        }

        let mut hasher = Sha1::new();
        hasher.update(&info_bytes);
        let infohash: [u8; 20] = hasher.finalize().into();

        let name = info
            .get_str("name")
            .ok_or_else(|| BtError::Meta("info 缺 name".into()))?
            .to_string();

        let piece_length = info
            .get_int("piece length")
            .filter(|v| *v > 0)
            .ok_or_else(|| BtError::Meta("info 缺合法 piece length".into()))?;
        let piece_length =
            u32::try_from(piece_length).map_err(|_| BtError::Meta("piece length 越界".into()))?;

        let pieces_raw = info
            .get("pieces")
            .and_then(BValue::as_bytes)
            .ok_or_else(|| BtError::Meta("info 缺 pieces".into()))?;
        if pieces_raw.is_empty() || pieces_raw.len() % 20 != 0 {
            return Err(BtError::Meta("pieces 长度非 20 的倍数".into()));
        }
        let pieces: Vec<[u8; 20]> = pieces_raw
            .as_chunks::<20>()
            .0
            .iter()
            .map(|c| {
                let mut h = [0u8; 20];
                h.copy_from_slice(c);
                h
            })
            .collect();

        let files = if let Some(len) = info.get_int("length") {
            if len < 0 {
                return Err(BtError::Meta("单文件 length 为负".into()));
            }
            vec![TorrentFile {
                path: name.clone(),
                length: len as u64,
            }]
        } else {
            let fl = info
                .get("files")
                .and_then(BValue::as_list)
                .ok_or_else(|| BtError::Meta("info 缺 length/files".into()))?;
            let mut out = Vec::with_capacity(fl.len());
            for f in fl {
                let length = f
                    .get_int("length")
                    .filter(|v| *v >= 0)
                    .ok_or_else(|| BtError::Meta("files 条目缺 length".into()))?;
                let segs = f
                    .get("path")
                    .and_then(BValue::as_list)
                    .ok_or_else(|| BtError::Meta("files 条目缺 path".into()))?;
                let mut parts = vec![name.clone()];
                for s in segs {
                    parts.push(
                        s.as_str()
                            .ok_or_else(|| BtError::Meta("path 段非字节串".into()))?
                            .to_string(),
                    );
                }
                out.push(TorrentFile {
                    path: parts.join("/"),
                    length: length as u64,
                });
            }
            if out.is_empty() {
                return Err(BtError::Meta("files 为空".into()));
            }
            out
        };

        let announce = root.get_str("announce").map(str::to_string);
        let mut announce_list = Vec::new();
        if let Some(tiers) = root.get("announce-list").and_then(BValue::as_list) {
            for tier in tiers {
                if let Some(urls) = tier.as_list() {
                    for u in urls {
                        if let Some(s) = u.as_str() {
                            announce_list.push(s.to_string());
                        }
                    }
                }
            }
        }

        Ok(TorrentMeta {
            info_bytes,
            infohash,
            name,
            piece_length,
            pieces,
            files,
            announce,
            announce_list,
        })
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn total_len(&self) -> u64 {
        self.files.iter().map(|f| f.length).sum()
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn num_pieces(&self) -> u32 {
        let total = self.total_len();
        let pl = u64::from(self.piece_length);
        total.div_ceil(pl).max(1) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mini_single_torrent() -> Vec<u8> {
        let data = b"0123456789";
        let mut pieces = Vec::new();
        for chunk in data.chunks(4) {
            let mut h = Sha1::new();
            h.update(chunk);
            pieces.extend_from_slice(&h.finalize());
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"d8:announce4:urls");
        out.extend_from_slice(
            format!(
                "4:infod6:lengthi10e4:name4:aaab12:piece lengthi4e6:pieces{}:",
                pieces.len()
            )
            .as_bytes(),
        );
        out.extend_from_slice(&pieces);
        out.push(b'e');
        out.push(b'e');
        out
    }

    #[test]
    fn parse_single_file() {
        let bytes = mini_single_torrent();
        let m = TorrentMeta::parse(&bytes).unwrap();
        assert_eq!(m.name, "aaab");
        assert_eq!(m.piece_length, 4);
        assert_eq!(m.files.len(), 1);
        assert_eq!(m.files[0].path, "aaab");
        assert_eq!(m.files[0].length, 10);
        assert_eq!(m.num_pieces(), 3);
        assert_eq!(m.announce.as_deref(), Some("urls"));
        assert_eq!(m.pieces.len(), 3);
    }

    #[test]
    fn parse_multi_file() {
        let pieces = [0u8; 20];
        let mut buf = Vec::new();
        buf.extend_from_slice(b"d4:infod5:filesl");
        buf.extend_from_slice(b"d6:lengthi5e4:pathl3:sub5:a.txtee");
        buf.extend_from_slice(b"d6:lengthi7e4:pathl5:b.binee");
        buf.extend_from_slice(b"e4:name3:tor12:piece lengthi16384e6:pieces20:");
        buf.extend_from_slice(&pieces);
        buf.push(b'e');
        buf.push(b'e');
        let m = TorrentMeta::parse(&buf).unwrap();
        assert_eq!(m.name, "tor");
        assert_eq!(m.files.len(), 2);
        assert_eq!(m.files[0].path, "tor/sub/a.txt");
        assert_eq!(m.files[0].length, 5);
        assert_eq!(m.files[1].path, "tor/b.bin");
        assert_eq!(m.total_len(), 12);
        assert_eq!(m.num_pieces(), 1);
    }

    #[test]
    fn announce_list_flattened() {
        let mut buf = mini_single_torrent();
        buf.splice(17..17, b"13:announce-listll3:tr1el3:tr2ee".iter().copied());
        let m = TorrentMeta::parse(&buf).unwrap();
        assert_eq!(m.announce_list, vec!["tr1", "tr2"]);
    }

    #[test]
    fn invalid_inputs_rejected() {
        assert!(TorrentMeta::parse(b"garbage").is_err());
        assert!(TorrentMeta::parse(b"de").is_err());
        assert!(
            TorrentMeta::parse(b"d4:infod6:lengthi1e4:name1:a12:piece lengthi16384ee").is_err()
        );
        let mut h = Sha1::new();
        h.update(b"x");
        let pieces = h.finalize();
        let mut buf = Vec::new();
        buf.extend_from_slice(b"d4:infod6:lengthi1e12:piece lengthi16384e6:pieces20:");
        buf.extend_from_slice(&pieces);
        buf.extend_from_slice(b"ee");
        assert!(TorrentMeta::parse(&buf).is_err());
        let mut buf2 = Vec::new();
        buf2.extend_from_slice(
            b"d4:infod6:lengthi1e4:name1:a12:piece lengthi16384e6:pieces3:abcee",
        );
        assert!(TorrentMeta::parse(&buf2).is_err());
    }

    #[test]
    fn info_bytes_round_trip_to_infohash() {
        let bytes = mini_single_torrent();
        let m = TorrentMeta::parse(&bytes).unwrap();
        assert!(bencode::decode_all(&m.info_bytes).is_ok());
        let mut h = Sha1::new();
        h.update(&m.info_bytes);
        assert_eq!(h.finalize().as_slice(), &m.infohash);
    }
}
