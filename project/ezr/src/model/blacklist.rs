use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlacklistEntry {
    pub ip: String,
    pub reason: String,
    pub banned_at: u64,
}

pub fn load(path: &Path) -> (Vec<BlacklistEntry>, Option<String>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (Vec::new(), None);
    };
    if text.trim().is_empty() {
        return (Vec::new(), None);
    }
    let parsed: Result<Vec<BlacklistEntry>, _> = serde_yaml::from_str(&text);
    match parsed {
        Ok(entries) => {
            let valid: Vec<BlacklistEntry> = entries
                .into_iter()
                .filter(|e| e.ip.parse::<std::net::IpAddr>().is_ok())
                .collect();
            (valid, None)
        }
        Err(_) => (
            Vec::new(),
            Some(format!(
                "黑名单文件无法解析（{}），已按空名单启动",
                path.display()
            )),
        ),
    }
}

pub fn record(path: &Path, ip: std::net::IpAddr, reason: &str) {
    let (mut entries, _) = load(path);
    if entries
        .iter()
        .any(|e| e.ip.parse::<std::net::IpAddr>().ok() == Some(ip))
    {
        return;
    }
    entries.push(BlacklistEntry {
        ip: ip.to_string(),
        reason: reason.to_string(),
        banned_at: unix_now(),
    });
    let body = match serde_yaml::to_string(&entries) {
        Ok(s) => s,
        Err(_) => return,
    };
    let tmp = path.with_extension("yaml.tmp");
    if std::fs::write(&tmp, body).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ezr-blacklist-test-{}-{tag}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir.join("blacklist.yaml")
    }

    #[test]
    fn missing_file_loads_empty_without_warning() {
        let p = tmp_path("missing");
        let _ = std::fs::remove_file(&p);
        let (entries, warn) = load(&p);
        assert!(entries.is_empty());
        assert!(warn.is_none());
    }

    #[test]
    fn round_trip_record_and_load() {
        let p = tmp_path("roundtrip");
        let _ = std::fs::remove_file(&p);
        let ip: std::net::IpAddr = "192.0.2.7".parse().unwrap();
        record(&p, ip, "leech");
        record(&p, ip, "leech");
        let ip2: std::net::IpAddr = "192.0.2.8".parse().unwrap();
        record(&p, ip2, "protocol");
        let (entries, warn) = load(&p);
        assert!(warn.is_none());
        assert_eq!(entries.len(), 2, "同 IP 重复封禁不重复记条目");
        assert_eq!(entries[0].ip, "192.0.2.7");
        assert_eq!(entries[0].reason, "leech");
        assert_eq!(entries[1].reason, "protocol");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn corrupt_file_loads_empty_with_warning() {
        let p = tmp_path("corrupt");
        std::fs::write(&p, "[not yaml ===").unwrap();
        let (entries, warn) = load(&p);
        assert!(entries.is_empty());
        assert!(warn.is_some(), "损坏文件应给出警告文本");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn entries_with_invalid_ip_are_skipped() {
        let p = tmp_path("badip");
        std::fs::write(&p, "- ip: not-an-ip\n  reason: leech\n  banned_at: 1\n").unwrap();
        let (entries, warn) = load(&p);
        assert!(entries.is_empty(), "非法 IP 条目不进名单");
        assert!(warn.is_none(), "单条非法 IP 不算文件损坏");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn delete_entry_and_reload_unbans() {
        let p = tmp_path("unban");
        let _ = std::fs::remove_file(&p);
        record(&p, "192.0.2.9".parse().unwrap(), "manual");
        let body = std::fs::read_to_string(&p).unwrap();
        let kept: Vec<&str> = body.lines().filter(|l| !l.contains("192.0.2.9")).collect();
        let has_entry = kept.iter().any(|l| l.trim_start().starts_with('-'));
        std::fs::write(
            &p,
            if has_entry {
                kept.join("\n")
            } else {
                String::new()
            },
        )
        .unwrap();
        let (entries, warn) = load(&p);
        assert!(warn.is_none(), "删空后的文件按空名单解析: {warn:?}");
        assert!(entries.is_empty(), "手工删除条目后重启 = 解封");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }
}
