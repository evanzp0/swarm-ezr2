//! consistency — 续传一致性检查（FR-01-22/D11）
//!
//! 续传前比较**最终 URL**（FR-01-14）/ ETag / Last-Modified / 文件大小，
//! 任一变化或服务器不再返回这些头 → 判定「续传一致性失效」：
//! sidecar 作废、从头重新下载（不计失败、不占重试计数）；
//! 同一任务连续 3 次失效 → 转为「已失败（服务器内容持续变化）」停等。

/// sidecar 记录的服务端标识（探测快照）
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerStamp {
    /// 最终 URL（重定向后）
    pub final_url: Option<String>,
    /// ETag
    pub etag: Option<String>,
    /// Last-Modified
    pub last_modified: Option<String>,
    /// 文件大小
    pub size: Option<u64>,
}

/// 一致性判定结果
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consistency {
    /// 一致，可续传
    Valid,
    /// 失效：服务器内容已更新（sidecar 作废、从头下载、不计失败）
    Invalidated,
    /// sidecar 缺少服务端标识（记录不全），按失效处理
    MissingStamp,
}

/// 续传一致性检查：sidecar 快照 vs 本次探测快照。
/// 任一字段变化或本次探测缺失（服务器不再返回这些头）→ 失效（D11）。
#[must_use]
pub fn check(stored: &ServerStamp, probed: &ServerStamp) -> Consistency {
    // sidecar 记录不全：无法证明是同一内容，按失效处理（保守正确）
    if stored.final_url.is_none() || stored.size.is_none() {
        return Consistency::MissingStamp;
    }
    let same_url = stored.final_url == probed.final_url;
    let same_size = stored.size == probed.size;
    // ETag/Last-Modified：sidecar 有记录而探测缺失，或两侧都有但不一致 → 失效
    let etag_ok = match (stored.etag.as_ref(), probed.etag.as_ref()) {
        (Some(a), Some(b)) => a == b,
        (Some(_), None) => false,
        (None, _) => true, // sidecar 未记录时以 URL/大小为准
    };
    let lm_ok = match (stored.last_modified.as_ref(), probed.last_modified.as_ref()) {
        (Some(a), Some(b)) => a == b,
        (Some(_), None) => false,
        (None, _) => true,
    };
    if same_url && same_size && etag_ok && lm_ok {
        Consistency::Valid
    } else {
        Consistency::Invalidated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(url: &str, etag: &str, size: u64) -> ServerStamp {
        ServerStamp {
            final_url: Some(url.to_string()),
            etag: Some(etag.to_string()),
            last_modified: Some("Wed, 01 Jan 2026 00:00:00 GMT".to_string()),
            size: Some(size),
        }
    }

    #[test]
    fn identical_stamp_is_valid() {
        let s = stamp("http://x/f", "e1", 100);
        assert_eq!(check(&s, &s), Consistency::Valid);
    }

    #[test]
    fn etag_change_invalidates() {
        let a = stamp("http://x/f", "e1", 100);
        let b = stamp("http://x/f", "e2", 100);
        assert_eq!(check(&a, &b), Consistency::Invalidated);
    }

    #[test]
    fn size_change_invalidates() {
        let a = stamp("http://x/f", "e1", 100);
        let b = stamp("http://x/f", "e1", 200);
        assert_eq!(check(&a, &b), Consistency::Invalidated);
    }

    #[test]
    fn final_url_change_invalidates() {
        // 重定向目标变化（FR-01-14 以最终 URL 参与）
        let a = stamp("http://x/a", "e1", 100);
        let b = stamp("http://x/b", "e1", 100);
        assert_eq!(check(&a, &b), Consistency::Invalidated);
    }

    #[test]
    fn probe_missing_headers_invalidates() {
        // 服务器不再返回 ETag/Last-Modified → 失效（FR-01-22）
        let a = stamp("http://x/f", "e1", 100);
        let mut b = stamp("http://x/f", "e1", 100);
        b.etag = None;
        assert_eq!(check(&a, &b), Consistency::Invalidated);
        let mut c = stamp("http://x/f", "e1", 100);
        c.last_modified = None;
        assert_eq!(check(&a, &c), Consistency::Invalidated);
    }

    #[test]
    fn stored_without_etag_falls_back_to_url_size() {
        let a = ServerStamp {
            final_url: Some("http://x/f".to_string()),
            etag: None,
            last_modified: None,
            size: Some(100),
        };
        let b = stamp("http://x/f", "whatever", 100);
        assert_eq!(check(&a, &b), Consistency::Valid);
    }

    #[test]
    fn stored_missing_stamp_is_conservatively_invalid() {
        let empty = ServerStamp::default();
        let b = stamp("http://x/f", "e", 100);
        assert_eq!(check(&empty, &b), Consistency::MissingStamp);
    }
}
