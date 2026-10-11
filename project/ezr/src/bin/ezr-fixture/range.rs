//! range — HTTP Range 头解析（纯函数）

/// `bytes=a-b` / `bytes=a-` 解析（开放末端 → u64::MAX；非法 → None）
pub fn parse_range(v: &str) -> Option<(u64, u64)> {
    let v = v.strip_prefix("bytes=")?;
    let (a, b) = v.split_once('-')?;
    let start = a.trim().parse::<u64>().ok()?;
    let end = if b.trim().is_empty() {
        u64::MAX
    } else {
        b.trim().parse::<u64>().ok()?
    };
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_range_closed_open_and_garbage() {
        assert_eq!(parse_range("bytes=0-99"), Some((0, 99)));
        assert_eq!(parse_range("bytes=100-"), Some((100, u64::MAX)));
        assert_eq!(parse_range("bytes= 5 - 10 "), Some((5, 10)));
        assert_eq!(parse_range("bytes=abc-10"), None);
        assert_eq!(parse_range("bytes=10"), None);
        assert_eq!(parse_range("items=0-9"), None);
        assert_eq!(parse_range(""), None);
    }
}
