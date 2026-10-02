//! query — fixture URL 查询参数解析（纯函数）

/// 简单查询参数解析（k=v&k=v；值不解码——fixture 路径约定为 ASCII）
pub fn parse_query(url: &str) -> (String, Vec<(String, String)>) {
    let (path, q) = match url.split_once('?') {
        Some((p, q)) => (p.to_string(), q),
        None => return (url.to_string(), Vec::new()),
    };
    let mut kv = Vec::new();
    for pair in q.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            kv.push((k.to_string(), v.to_string()));
        } else if !pair.is_empty() {
            kv.push((pair.to_string(), String::new()));
        }
    }
    (path, kv)
}

/// 取查询参数值（首次出现者优先）
pub fn qget<'a>(kv: &'a [(String, String)], k: &str) -> Option<&'a str> {
    kv.iter().find(|(key, _)| key == k).map(|(_, v)| v.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_query_without_query_returns_path_only() {
        assert_eq!(
            parse_query("/small.bin"),
            ("/small.bin".to_string(), vec![])
        );
    }

    #[test]
    fn parse_query_kv_pairs_and_bare_keys() {
        let (path, kv) = parse_query("/f.bin?status=503&retry_after=10&flag&empty=");
        assert_eq!(path, "/f.bin");
        assert_eq!(
            kv,
            vec![
                ("status".to_string(), "503".to_string()),
                ("retry_after".to_string(), "10".to_string()),
                ("flag".to_string(), String::new()),
                ("empty".to_string(), String::new()),
            ]
        );
    }

    #[test]
    fn qget_first_occurrence_and_missing() {
        let kv = vec![
            ("a".to_string(), "1".to_string()),
            ("a".to_string(), "2".to_string()),
        ];
        assert_eq!(qget(&kv, "a"), Some("1"));
        assert_eq!(qget(&kv, "b"), None);
    }
}
