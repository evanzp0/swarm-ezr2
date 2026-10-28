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

#[must_use]
pub fn client_abbrev(peer_id: &[u8; 20]) -> String {
    if peer_id.first() != Some(&b'-') || peer_id.len() < 8 || peer_id[7] != b'-' {
        return "--".to_string();
    }
    let mid = &peer_id[1..7];
    if !mid.iter().take(2).all(u8::is_ascii_alphabetic) {
        return "--".to_string();
    }
    let ok = mid[2..]
        .iter()
        .all(|b| b.is_ascii_alphanumeric() && b.is_ascii_alphabetic() || b.is_ascii_digit());
    if !ok {
        return "--".to_string();
    }
    String::from_utf8_lossy(&mid[..2]).to_ascii_uppercase()
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

    #[test]
    fn client_abbrev_known_prefixes() {
        let mk = |code: &str, ver: &str| {
            let mut id = [0u8; 20];
            let p = format!("-{code}{ver}-");
            id[..p.len()].copy_from_slice(p.as_bytes());
            id
        };
        assert_eq!(client_abbrev(&mk("QB", "2500")), "QB");
        assert_eq!(client_abbrev(&mk("TR", "4030")), "TR");
        assert_eq!(client_abbrev(&mk("LT", "1050")), "LT");
        assert_eq!(client_abbrev(&mk("EZ", "2000")), "EZ");
        assert_eq!(client_abbrev(&mk("qb", "2500")), "QB");
    }

    #[test]
    fn client_abbrev_unknown_shapes() {
        let mut id = [0u8; 20];
        id[..6].copy_from_slice(b"M7-100");
        assert_eq!(client_abbrev(&id), "--");
        let mut id2 = [0u8; 20];
        id2[..7].copy_from_slice(b"-QB2500");
        assert_eq!(client_abbrev(&id2), "--");
        let mut id3 = [0u8; 20];
        id3[..8].copy_from_slice(b"-Q#2500-");
        assert_eq!(client_abbrev(&id3), "--");
        let short = [b'-'; 20];
        assert_eq!(client_abbrev(&short), "--");
    }
}
