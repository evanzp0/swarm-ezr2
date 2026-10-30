const EZ_PREFIX: &[u8; 8] = b"-EZ2000-";

#[must_use]
pub fn make_peer_id() -> [u8; 20] {
    let mut id = [0u8; 20];
    id[..8].copy_from_slice(EZ_PREFIX);
    const ALPHA: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let addr = &id as *const _ as u64;
    let mut state = nanos as u64 ^ addr.rotate_left(17) ^ 0x9E37_79B9_7F4A_7C15;
    for slot in id.iter_mut().skip(8) {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let idx = ((state >> 33) as usize) % ALPHA.len();
        *slot = ALPHA[idx];
    }
    id
}

const CLIENT_NAMES: &[(&str, &str)] = &[
    ("7T", "aTorrent"),
    ("AB", "AnyEvent BitTorrent"),
    ("AG", "Ares"),
    ("AR", "ArcticTorrent"),
    ("AX", "BitTorrent X"),
    ("AZ", "Azureus"),
    ("BB", "BitBuddy"),
    ("BC", "BitComet"),
    ("BE", "BareTorrent"),
    ("BF", "Bitflu"),
    ("BG", "BTG"),
    ("BR", "BitRocket"),
    ("BS", "BTSlave"),
    ("BT", "BBTor"),
    ("BX", "~Bittorrent X"),
    ("CD", "Enhanced CTorrent"),
    ("CT", "CTorrent"),
    ("DE", "Deluge"),
    ("DP", "Propagate Data Client"),
    ("EB", "EBit"),
    ("ES", "electric sheep"),
    ("EZ", "EZR Downloader"),
    ("FT", "Free Torrent"),
    ("FX", "Freebox BitTorrent"),
    ("HL", "Halite"),
    ("HN", "Hydranode"),
    ("KT", "KTorrent"),
    ("LH", "LH-ABC"),
    ("LP", "Lphant"),
    ("LT", "libtorrent"),
    ("MO", "MonoTorrent"),
    ("MP", "MooPolice"),
    ("MR", "Miro"),
    ("MT", "MoonlightTorrent"),
    ("NB", "Net::BitTorrent"),
    ("NX", "Net Transport"),
    ("OS", "OneSwarm"),
    ("OT", "OmegaTorrent"),
    ("PD", "Pando"),
    ("QD", "QQDownload"),
    ("QT", "Qt 4 Torrent example"),
    ("RS", "Rufus"),
    ("RT", "Retriever"),
    ("SD", "Thunder"),
    ("SP", "BitSpirit"),
    ("ST", "SymTorrent"),
    ("SZ", "Shareaza"),
    ("TB", "Torch"),
    ("TN", "TorrentDotORG"),
    ("TR", "Transmission"),
    ("TS", "Torrentstorm"),
    ("TT", "TuoTu"),
    ("UL", "uLeecher"),
    ("UM", "uTorrent Mac"),
    ("UT", "uTorrent"),
    ("VG", "Vagaa"),
    ("WD", "WebTorrent"),
    ("WY", "Wyzo"),
    ("XL", "Xunlei"),
    ("XT", "Xan Torrent"),
    ("XX", "Xtorrent"),
    ("ZT", "Zip Torrent"),
    ("qB", "qBittorrent"),
];

fn client_full_name(code: &str) -> Option<&'static str> {
    CLIENT_NAMES
        .iter()
        .find(|(c, _)| c.eq_ignore_ascii_case(code))
        .map(|(_, n)| *n)
}

fn version_component(c: u8) -> String {
    if c.is_ascii_digit() {
        char::from(c).to_string()
    } else if c.is_ascii_alphabetic() {
        match char::from(c).to_ascii_lowercase().to_digit(36) {
            Some(v) => v.to_string(),
            None => "0".to_string(),
        }
    } else {
        "0".to_string()
    }
}

#[must_use]
pub fn client_display(peer_id: &[u8; 20]) -> String {
    if peer_id.first() != Some(&b'-') || peer_id[7] != b'-' {
        return "Unknown".to_string();
    }
    let mid = &peer_id[1..7];
    if !mid.iter().take(2).all(u8::is_ascii_alphabetic) {
        return "Unknown".to_string();
    }
    let code = std::str::from_utf8(&mid[..2]).unwrap_or("??");
    let Some(name) = client_full_name(code) else {
        return "Unknown".to_string();
    };
    let version: Vec<String> = peer_id[3..7].iter().map(|&c| version_component(c)).collect();
    format!("{name}/{}", version.join("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_id_shape() {
        let id = make_peer_id();
        assert_eq!(id.len(), 20);
        assert_eq!(&id[..8], EZ_PREFIX);
        for b in &id[8..] {
            assert!(b.is_ascii_alphanumeric(), "随机段 {b} 非字母数字");
        }
    }

    #[test]
    fn peer_ids_differ_across_calls() {
        let a = make_peer_id();
        let b = make_peer_id();
        assert_ne!(a, b, "两次生成应不同（随机段）");
    }

    fn mk(code: &str, ver: &str) -> [u8; 20] {
        let mut id = [0u8; 20];
        let p = format!("-{code}{ver}-");
        id[..p.len()].copy_from_slice(p.as_bytes());
        id
    }

    #[test]
    fn client_display_full_name_and_version() {
        assert_eq!(client_display(&mk("QB", "2500")), "qBittorrent/2.5.0.0");
        assert_eq!(client_display(&mk("TR", "4030")), "Transmission/4.0.3.0");
        assert_eq!(client_display(&mk("LT", "2050")), "libtorrent/2.0.5.0");
        assert_eq!(client_display(&mk("AR", "1001")), "ArcticTorrent/1.0.0.1");
        assert_eq!(client_display(&mk("EZ", "2000")), "EZR Downloader/2.0.0.0");
        assert_eq!(client_display(&mk("qb", "2500")), "qBittorrent/2.5.0.0", "两位码大小写不敏感");
    }

    #[test]
    fn client_display_unknown_shapes() {
        assert_eq!(client_display(&mk("QX", "7777")), "Unknown", "映射表外的码 → Unknown");
        let mut id = [0u8; 20];
        id[..6].copy_from_slice(b"M7-100");
        assert_eq!(client_display(&id), "Unknown", "非 Azureus 形态 → Unknown");
        let mut id2 = [0u8; 20];
        id2[..7].copy_from_slice(b"-QB2500");
        assert_eq!(client_display(&id2), "Unknown", "缺尾部 '-' → Unknown");
        let mut id3 = [0u8; 20];
        id3[..8].copy_from_slice(b"-Q#2500-");
        assert_eq!(client_display(&id3), "Unknown", "码含非字母 → Unknown");
        let short = [b'-'; 20];
        assert_eq!(client_display(&short), "Unknown");
    }

    #[test]
    fn client_display_alpha_version_base36() {
        assert_eq!(client_display(&mk("AB", "1Z2Y")), "AnyEvent BitTorrent/1.35.2.34");
    }

    #[test]
    fn client_display_max_width_fits_column() {
        let s = client_display(&mk("AR", "ZZZZ"));
        assert_eq!(s, "ArcticTorrent/35.35.35.35");
        assert_eq!(s.chars().count(), 25, "极值样例 ArcticTorrent/100.100.100 同宽 25 列");
        assert!(s.chars().count() <= 25, "极值宽度 ≤ 25（列宽 26 含 1 空白）");
    }
}
