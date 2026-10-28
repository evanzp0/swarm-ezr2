#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerStamp {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub size: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consistency {
    Valid,
    Invalidated,
    MissingStamp,
}

fn header_unchanged(stored: Option<&String>, probed: Option<&String>) -> bool {
    match (stored, probed) {
        (Some(a), Some(b)) => a == b,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

#[must_use]
pub fn check(stored: &ServerStamp, probed: &ServerStamp) -> Consistency {
    if stored.size.is_none() {
        return Consistency::MissingStamp;
    }
    let same_size = stored.size == probed.size;
    let etag_ok = header_unchanged(stored.etag.as_ref(), probed.etag.as_ref());
    let lm_ok = header_unchanged(stored.last_modified.as_ref(), probed.last_modified.as_ref());
    if same_size && etag_ok && lm_ok {
        Consistency::Valid
    } else {
        Consistency::Invalidated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(etag: &str, size: u64) -> ServerStamp {
        ServerStamp {
            etag: Some(etag.to_string()),
            last_modified: Some("Wed, 01 Jan 2026 00:00:00 GMT".to_string()),
            size: Some(size),
        }
    }

    #[test]
    fn identical_stamp_is_valid() {
        let s = stamp("e1", 100);
        assert_eq!(check(&s, &s), Consistency::Valid);
    }

    #[test]
    fn etag_change_invalidates() {
        let a = stamp("e1", 100);
        let b = stamp("e2", 100);
        assert_eq!(check(&a, &b), Consistency::Invalidated);
    }

    #[test]
    fn size_change_invalidates() {
        let a = stamp("e1", 100);
        let b = stamp("e1", 200);
        assert_eq!(check(&a, &b), Consistency::Invalidated);
    }

    #[test]
    fn last_modified_change_invalidates() {
        let mut a = stamp("e1", 100);
        let mut b = stamp("e1", 100);
        a.last_modified = Some("Tue, 01 Jan 2019 00:00:00 GMT".to_string());
        b.last_modified = Some("Wed, 01 Jan 2026 00:00:00 GMT".to_string());
        assert_eq!(check(&a, &b), Consistency::Invalidated);
    }

    #[test]
    fn probe_missing_headers_invalidates() {
        let a = stamp("e1", 100);
        let mut b = stamp("e1", 100);
        b.etag = None;
        assert_eq!(check(&a, &b), Consistency::Invalidated);
        let mut c = stamp("e1", 100);
        c.last_modified = None;
        assert_eq!(check(&a, &c), Consistency::Invalidated);
    }

    #[test]
    fn stored_without_headers_falls_back_to_size() {
        let a = ServerStamp {
            etag: None,
            last_modified: None,
            size: Some(100),
        };
        let b = stamp("whatever", 100);
        assert_eq!(check(&a, &b), Consistency::Valid);
    }

    #[test]
    fn stored_missing_stamp_is_conservatively_invalid() {
        let empty = ServerStamp::default();
        let b = stamp("e", 100);
        assert_eq!(check(&empty, &b), Consistency::MissingStamp);
        let no_size = ServerStamp {
            etag: Some("e".to_string()),
            last_modified: Some("Wed, 01 Jan 2026 00:00:00 GMT".to_string()),
            size: None,
        };
        assert_eq!(check(&no_size, &b), Consistency::MissingStamp);
        let size_only = ServerStamp {
            etag: None,
            last_modified: None,
            size: Some(100),
        };
        assert_eq!(check(&size_only, &size_only), Consistency::Valid);
    }
}
