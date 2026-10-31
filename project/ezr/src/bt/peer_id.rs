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

const NAME_MAP: &[(&str, &str)] = &[
    ("7T", "aTorrent for android"),
    ("A", "ABC"),
    ("AB", "AnyEvent BitTorrent"),
    ("AG", "Ares"),
    ("AR", "Arctic Torrent"),
    ("AT", "Artemis"),
    ("AV", "Avicora"),
    ("AX", "BitPump"),
    ("AZ", "Azureus"),
    ("A~", "Ares"),
    ("BB", "BitBuddy"),
    ("BC", "BitComet"),
    ("BE", "baretorrent"),
    ("BF", "Bitflu"),
    ("BG", "BTG"),
    ("BI", "BiglyBT"),
    ("BL", "BitBlinder"),
    ("BP", "BitTorrent Pro"),
    ("BR", "BitRocket"),
    ("BS", "BTSlave"),
    ("BT", "BitTorrent"),
    ("BU", "BigUp"),
    ("BW", "BitWombat"),
    ("BX", "BittorrentX"),
    ("CD", "Enhanced CTorrent"),
    ("CT", "CTorrent"),
    ("DE", "Deluge"),
    ("DP", "Propagate Data Client"),
    ("EB", "EBit"),
    ("ES", "electric sheep"),
    ("FC", "FileCroc"),
    ("FT", "FoxTorrent"),
    ("FW", "FrostWire"),
    ("FX", "Freebox BitTorrent"),
    ("GS", "GSTorrent"),
    ("HK", "Hekate"),
    ("HL", "Halite"),
    ("HN", "Hydranode"),
    ("IL", "iLivid"),
    ("KC", "Koinonein"),
    ("KG", "KGet"),
    ("KT", "KTorrent"),
    ("LC", "LeechCraft"),
    ("LH", "LH-ABC"),
    ("LK", "Linkage"),
    ("LP", "lphant"),
    ("LR", "LibreTorrent"),
    ("LT", "libtorrent"),
    ("LW", "Limewire"),
    ("M", "Mainline"),
    ("ML", "MLDonkey"),
    ("MO", "Mono Torrent"),
    ("MP", "MooPolice"),
    ("MR", "Miro"),
    ("MT", "Moonlight Torrent"),
    ("NX", "Net Transport"),
    ("O", "Osprey Permaseed"),
    ("OS", "OneSwarm"),
    ("OT", "OmegaTorrent"),
    ("PD", "Pando"),
    ("Q", "BTQueue"),
    ("QD", "QQDownload"),
    ("QT", "Qt 4"),
    ("R", "Tribler"),
    ("RT", "Retriever"),
    ("RZ", "RezTorrent"),
    ("S", "Shadow"),
    ("SB", "Swiftbit"),
    ("SD", "Xunlei"),
    ("SK", "spark"),
    ("SN", "ShareNet"),
    ("SS", "SwarmScope"),
    ("ST", "SymTorrent"),
    ("SZ", "Shareaza"),
    ("S~", "Shareaza (beta)"),
    ("T", "BitTornado"),
    ("TB", "Torch"),
    ("TL", "Tribler"),
    ("TN", "Torrent.NET"),
    ("TR", "Transmission"),
    ("TS", "TorrentStorm"),
    ("TT", "TuoTu"),
    ("U", "UPnP"),
    ("UL", "uLeecher"),
    ("UM", "uTorrent Mac"),
    ("UT", "uTorrent"),
    ("VG", "Vagaa"),
    ("WD", "WebTorrent Desktop"),
    ("WT", "BitLet"),
    ("WW", "WebTorrent"),
    ("WY", "FireTorrent"),
    ("XF", "Xfplay"),
    ("XL", "Xunlei"),
    ("XS", "XSwifter"),
    ("XT", "XanTorrent"),
    ("XX", "Xtorrent"),
    ("ZO", "Zona"),
    ("ZT", "ZipTorrent"),
    ("lt", "rTorrent"),
    ("pX", "pHoeniX"),
    ("qB", "qBittorrent"),
    ("st", "SharkTorrent"),
];

const GENERIC_MAPPINGS: &[(usize, &str, &str)] = &[
    (0, "Deadman Walking-", "Deadman"),
    (5, "Azureus", "Azureus 2.0.3.2"),
    (0, "DansClient", "XanTorrent"),
    (4, "btfans", "SimpleBT"),
    (0, "PRC.P---", "Bittorrent Plus! II"),
    (0, "P87.P---", "Bittorrent Plus!"),
    (0, "S587Plus", "Bittorrent Plus!"),
    (0, "martini", "Martini Man"),
    (0, "Plus---", "Bittorrent Plus"),
    (0, "turbobt", "TurboBT"),
    (0, "a00---0", "Swarmy"),
    (0, "a02---0", "Swarmy"),
    (0, "T00---0", "Teeweety"),
    (0, "BTDWV-", "Deadman Walking"),
    (2, "BS", "BitSpirit"),
    (0, "Pando-", "Pando"),
    (0, "LIME", "LimeWire"),
    (0, "btuga", "BTugaXP"),
    (0, "oernu", "BTugaXP"),
    (0, "Mbrst", "Burst!"),
    (0, "PEERAPP", "PeerApp"),
    (0, "Plus", "Plus!"),
    (0, "-Qt-", "Qt"),
    (0, "exbc", "BitComet"),
    (0, "DNA", "BitTorrent DNA"),
    (0, "-G3", "G3 Torrent"),
    (0, "-FG", "FlashGet"),
    (0, "-ML", "MLdonkey"),
    (0, "-MG", "Media Get"),
    (0, "XBT", "XBT"),
    (0, "OP", "Opera"),
    (2, "RS", "Rufus"),
    (0, "AZ2500BT", "BitTyrant"),
    (0, "btpd/", "BitTorrent Protocol Daemon"),
    (0, "TIX", "Tixati"),
    (0, "QVOD", "Qvod"),
];

struct Fingerprint {
    name: String,
    major: i64,
    minor: i64,
    revision: i64,
    tag: i64,
}

fn decode_digit(c: u8) -> i64 {
    if c.is_ascii_digit() {
        i64::from(c - b'0')
    } else {
        i64::from(c) - i64::from(b'A') + 10
    }
}

fn is_print(c: u8) -> bool {
    (32..=126).contains(&c)
}

fn find_string(p: &[u8], search: &[u8]) -> bool {
    p.len() >= search.len() && &p[..search.len()] == search
}

fn parse_az_style(p: &[u8; 20]) -> Option<Fingerprint> {
    if p[0] != b'-'
        || !is_print(p[1])
        || p[2] < b'0'
        || p[3] < b'0'
        || p[4] < b'0'
        || p[5] < b'0'
        || p[6] < b'0'
        || p[7] != b'-'
    {
        return None;
    }
    Some(Fingerprint {
        name: String::from_utf8_lossy(&p[1..3]).into_owned(),
        major: decode_digit(p[3]),
        minor: decode_digit(p[4]),
        revision: decode_digit(p[5]),
        tag: decode_digit(p[6]),
    })
}

fn parse_shadow_style(p: &[u8; 20]) -> Option<Fingerprint> {
    if !p[0].is_ascii_alphanumeric() {
        return None;
    }
    let (major, minor, revision);
    if p[4] == b'-' && p[5] == b'-' {
        if p[1] < b'0' || p[2] < b'0' || p[3] < b'0' {
            return None;
        }
        major = decode_digit(p[1]);
        minor = decode_digit(p[2]);
        revision = decode_digit(p[3]);
    } else {
        if p[8] != 0 || p[1] > 127 || p[2] > 127 || p[3] > 127 {
            return None;
        }
        major = i64::from(p[1]);
        minor = i64::from(p[2]);
        revision = i64::from(p[3]);
    }
    Some(Fingerprint {
        name: (p[0] as char).to_string(),
        major,
        minor,
        revision,
        tag: 0,
    })
}

fn parse_transmission_legacy_style(p: &[u8; 20]) -> Option<Fingerprint> {
    if p[0] != b'-' || p[1] != b'T' || p[2] != b'R' || p[7] != b'-' {
        return None;
    }
    if !(b'0'..=b'3').contains(&p[3]) {
        return None;
    }
    if !p[4].is_ascii_digit() || !p[5].is_ascii_digit() {
        return None;
    }
    if !p[6].is_ascii_digit() && !p[6].is_ascii_uppercase() {
        return None;
    }
    Some(Fingerprint {
        name: "TR".to_string(),
        major: decode_digit(p[3]),
        minor: decode_digit(p[4]) * 10 + decode_digit(p[5]),
        revision: 0,
        tag: decode_digit(p[6]),
    })
}

fn parse_mainline_style(p: &[u8; 20]) -> Option<Fingerprint> {
    if !is_print(p[0]) {
        return None;
    }
    let mut i = 1usize;
    let major = scan_i64(p, &mut i)?;
    expect_byte(p, &mut i, b'-')?;
    let minor = scan_i64(p, &mut i)?;
    expect_byte(p, &mut i, b'-')?;
    let revision = scan_i64(p, &mut i)?;
    expect_byte(p, &mut i, b'-')?;
    expect_byte(p, &mut i, b'-')?;
    Some(Fingerprint {
        name: (p[0] as char).to_string(),
        major,
        minor,
        revision,
        tag: 0,
    })
}

fn scan_i64(p: &[u8; 20], i: &mut usize) -> Option<i64> {
    while *i < p.len() && (p[*i] as char).is_ascii_whitespace() {
        *i += 1;
    }
    let mut neg = false;
    if *i < p.len() && (p[*i] == b'+' || p[*i] == b'-') {
        neg = p[*i] == b'-';
        *i += 1;
    }
    let mut val: i64 = 0;
    let mut n = 0usize;
    while *i < p.len() && n < 3 && p[*i].is_ascii_digit() {
        val = val * 10 + i64::from(p[*i] - b'0');
        *i += 1;
        n += 1;
    }
    if n == 0 {
        return None;
    }
    Some(if neg { -val } else { val })
}

fn expect_byte(p: &[u8; 20], i: &mut usize, b: u8) -> Option<()> {
    if *i < p.len() && p[*i] == b {
        *i += 1;
        Some(())
    } else {
        None
    }
}

fn lookup(f: &Fingerprint) -> (String, String) {
    let name = NAME_MAP
        .iter()
        .find(|(id, _)| *id == f.name)
        .map(|(_, n)| *n)
        .unwrap_or(&f.name)
        .to_string();
    let mut version = format!("{}.{}.{}", f.major, f.minor, f.revision);
    if f.tag != 0 {
        version.push_str(&format!(".{}", f.tag));
    }
    (name, version)
}

enum Identified {
    Name(String),
    Versioned { name: String, version: String },
}

fn identify_client(p: &[u8; 20]) -> Identified {
    if p.iter().all(|&b| b == 0) {
        return Identified::Name("Unknown".to_string());
    }

    for &(off, id, name) in GENERIC_MAPPINGS {
        if off + id.len() <= p.len() && find_string(&p[off..], id.as_bytes()) {
            return Identified::Name(name.to_string());
        }
    }

    if find_string(p, b"-BOW") && p[7] == b'-' {
        return Identified::Name(format!(
            "Bits on Wheels {}",
            String::from_utf8_lossy(&p[4..7])
        ));
    }

    if find_string(p, b"eX") {
        let user = String::from_utf8_lossy(&p[2..14]);
        return Identified::Name(format!("eXeem ('{user}')"));
    }

    let is_equ_zero = p[..12].iter().all(|&b| b == 0);
    if is_equ_zero && p[12] == 0x97 {
        return Identified::Name("Experimental 3.2.1b2".to_string());
    }
    if is_equ_zero && p[12] == 0 {
        return Identified::Name("Experimental 3.1".to_string());
    }

    if let Some(f) = parse_transmission_legacy_style(p) {
        let (name, version) = lookup(&f);
        return Identified::Versioned { name, version };
    }
    if let Some(f) = parse_az_style(p) {
        let (name, version) = lookup(&f);
        return Identified::Versioned { name, version };
    }
    if let Some(f) = parse_shadow_style(p) {
        let (name, version) = lookup(&f);
        return Identified::Versioned { name, version };
    }
    if let Some(f) = parse_mainline_style(p) {
        let (name, version) = lookup(&f);
        return Identified::Versioned { name, version };
    }

    if is_equ_zero {
        return Identified::Name("Generic".to_string());
    }

    Identified::Name("Unknown".to_string())
}

fn sanitize_self_report(s: &str) -> String {
    let printable: String = s.chars().filter(|c| !c.is_control()).collect();
    printable.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[must_use]
pub fn client_display(v: Option<&str>, peer_id: &[u8; 20]) -> String {
    if let Some(report) = v.map(sanitize_self_report).filter(|s| !s.is_empty()) {
        return report;
    }
    match identify_client(peer_id) {
        Identified::Name(s) => s,
        Identified::Versioned { name, version } => format!("{name}/{version}"),
    }
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
    fn client_display_self_report_wins() {
        let id = mk("BC", "0164");
        assert_eq!(
            client_display(Some("BitComet 1.64"), &id),
            "BitComet 1.64",
            "验收例：BitComet 1.64（扩展握手 v 自报名，qBittorrent 同值）"
        );
        assert_eq!(
            client_display(Some("  qBittorrent  5.1.2 "), &id),
            "qBittorrent 5.1.2",
            "v 自报名空白归并"
        );
        let dirty = format!("BitComet{} 1.64", char::from(0x01));
        assert_eq!(
            client_display(Some(&dirty), &id),
            "BitComet 1.64",
            "不可打印字符剔除后空白归并"
        );
        assert_eq!(
            client_display(Some("   "), &id),
            "BitComet/0.1.6.4",
            "空 v 回退 peer id 解码"
        );
        assert_eq!(client_display(None, &id), "BitComet/0.1.6.4");
    }

    #[test]
    fn client_display_azureus_style() {
        assert_eq!(
            client_display(None, &mk("qB", "2500")),
            "qBittorrent/2.5.0",
            "tag=0 省略第 4 段"
        );
        assert_eq!(
            client_display(None, &mk("qB", "2531")),
            "qBittorrent/2.5.3.1"
        );
        assert_eq!(
            client_display(None, &mk("TR", "4030")),
            "Transmission/4.0.3"
        );
        assert_eq!(client_display(None, &mk("LT", "2110")), "libtorrent/2.1.1");
        assert_eq!(
            client_display(None, &mk("AR", "1001")),
            "Arctic Torrent/1.0.0.1",
            "全名映射表 102 项（Arctic Torrent 带空格）"
        );
        assert_eq!(
            client_display(None, &mk("BC", "0164")),
            "BitComet/0.1.6.4",
            "peer id 解码路径四段"
        );
        assert_eq!(
            client_display(None, &mk("QB", "2500")),
            "QB/2.5.0",
            "码匹配区分大小写（lt 口径）：QB 不命中映射表 → 码本体"
        );
        assert_eq!(
            client_display(None, &mk("QX", "7777")),
            "QX/7.7.7.7",
            "映射表外码 → 码本体 + 版本（不再是 Unknown）"
        );
    }

    #[test]
    fn client_display_transmission_legacy() {
        assert_eq!(
            client_display(None, &mk("TR", "3010")),
            "Transmission/3.1.0",
            "legacy x∈0–3：minor 两位合一，rev 恒 0"
        );
        assert_eq!(
            client_display(None, &mk("TR", "304A")),
            "Transmission/3.4.0.10",
            "legacy rev 恒 0 且 lookup 恒印三段：3.4.0 + tag .10（lt 原语义）"
        );
    }

    #[test]
    fn client_display_shadow_mainline() {
        let mut s = [0u8; 20];
        s[..6].copy_from_slice(b"S567--");
        assert_eq!(
            client_display(None, &s),
            "Shadow/5.6.7",
            "shadow -- 结尾臂逐位 decode"
        );
        let mut m = [0u8; 20];
        m[..16].copy_from_slice(b"M5-6-7--abcdefgh");
        assert_eq!(
            client_display(None, &m),
            "Mainline/5.6.7",
            "mainline sscanf 形态（随机尾避开 shadow 原始字节臂，lt 同语义）"
        );
    }

    #[test]
    fn client_display_generic_no_version() {
        let mut g = [0u8; 20];
        g[..7].copy_from_slice(b"exbc\0\x01L");
        assert_eq!(
            client_display(None, &g),
            "BitComet",
            "generic_mappings 仅名无版本"
        );
        let mut bs = [0u8; 20];
        bs[..4].copy_from_slice(b"XXBS");
        assert_eq!(client_display(None, &bs), "BitSpirit", "offset=2 前缀匹配");
    }

    #[test]
    fn client_display_unknown_shapes() {
        assert_eq!(
            client_display(None, &[0u8; 20]),
            "Unknown",
            "全零 → Unknown"
        );
        let mut id = [0u8; 20];
        id[..14].copy_from_slice(b"M7-100zzzzzzzz");
        assert_eq!(
            client_display(None, &id),
            "Unknown",
            "残缺形态全不匹配（非零尾避开 shadow 原始字节臂）→ Unknown"
        );
        let mut x = [0u8; 20];
        x[..12].copy_from_slice(b"xxxxxxxxxxxx");
        assert_eq!(
            client_display(None, &x),
            "Unknown",
            "无 style 可匹配 → Unknown"
        );
        let short = [b'-'; 20];
        assert_eq!(client_display(None, &short), "Unknown");
    }

    #[test]
    fn client_display_special_forms() {
        let mut bow = [0u8; 20];
        bow[..8].copy_from_slice(b"-BOWABC-");
        assert_eq!(client_display(None, &bow), "Bits on Wheels ABC");
        let mut az = [0u8; 20];
        az[..18].copy_from_slice(b"xxxxxAzureusxxxxxx");
        assert_eq!(
            client_display(None, &az),
            "Azureus 2.0.3.2",
            "offset=5 generic 特判"
        );
    }

    #[test]
    fn client_display_max_width_fits_column() {
        let s = client_display(None, &mk("DP", "ZZZZ"));
        assert_eq!(s, "Propagate Data Client/35.35.35.35");
        assert_eq!(
            s.chars().count(),
            33,
            "极值样例 = 全名最长 21 + /版本极值 11 + 1 分隔"
        );
        assert!(
            s.chars().count() <= 33,
            "极值宽度 ≤ 33（列宽 34 含 1 空白）"
        );
    }
}
