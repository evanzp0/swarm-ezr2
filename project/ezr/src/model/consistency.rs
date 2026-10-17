//! consistency — 续传一致性检查（FR-01-22/D11；v1.28 URL 权威模型改版）
//!
//! 续传前比较 **ETag / Last-Modified / 文件大小** 三要素（v1.28：原「最终 URL」
//! 腿随 FR-01-14/20 去 URL 退役——sidecar 不存储任何 URL，无比较对象），
//! 任一变化或服务器不再返回这些头 → 判定「续传一致性失效」：
//! sidecar 作废、从头重新下载（不计失败、不占重试计数）；
//! 同一任务连续 3 次失效 → 转为「已失败（服务器内容持续变化）」停等。

/// sidecar 记录的服务端标识（探测快照）
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerStamp {
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

/// 头部一致性（ETag / Last-Modified 共用口径）：sidecar 有记录而探测缺失，
/// 或两侧都有但不一致 → 失效（false）；sidecar 未记录时以大小为准（true）。
fn header_unchanged(stored: Option<&String>, probed: Option<&String>) -> bool {
    match (stored, probed) {
        (Some(a), Some(b)) => a == b,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

/// 续传一致性检查：sidecar 快照 vs 本次探测快照（三要素，FR-01-22 v1.28）。
/// 任一字段变化或本次探测缺失（服务器不再返回这些头）→ 失效（D11）。
#[must_use]
pub fn check(stored: &ServerStamp, probed: &ServerStamp) -> Consistency {
    // sidecar 记录不全：无法证明是同一内容，按失效处理（保守正确）
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
        // 三要素第三腿：Last-Modified 变化同样失效（v1.28 口径下三要素齐全）
        let mut a = stamp("e1", 100);
        let mut b = stamp("e1", 100);
        a.last_modified = Some("Tue, 01 Jan 2019 00:00:00 GMT".to_string());
        b.last_modified = Some("Wed, 01 Jan 2026 00:00:00 GMT".to_string());
        assert_eq!(check(&a, &b), Consistency::Invalidated);
    }

    #[test]
    fn probe_missing_headers_invalidates() {
        // 服务器不再返回 ETag/Last-Modified → 失效（FR-01-22）
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
        // sidecar 未记录头部 → 以大小为准（头部宽容口径不变）
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
        // v1.28：MissingStamp 判据收敛为「缺大小」（URL 不再是记录要素）
        let empty = ServerStamp::default();
        let b = stamp("e", 100);
        assert_eq!(check(&empty, &b), Consistency::MissingStamp);
        let no_size = ServerStamp {
            etag: Some("e".to_string()),
            last_modified: Some("Wed, 01 Jan 2026 00:00:00 GMT".to_string()),
            size: None,
        };
        assert_eq!(check(&no_size, &b), Consistency::MissingStamp);
        // 仅有大小（头部双缺）= 记录完整（以大小为准宽容）→ 可判 Valid
        let size_only = ServerStamp {
            etag: None,
            last_modified: None,
            size: Some(100),
        };
        assert_eq!(check(&size_only, &size_only), Consistency::Valid);
    }
}
