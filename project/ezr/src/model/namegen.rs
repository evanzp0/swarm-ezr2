use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DOWNLOADING_EXT: &str = ".downloading";

pub const SIDECAR_EXT: &str = ".ezr";

#[must_use]
pub fn sidecar_path_of(dir: &str, name: &str) -> String {
    format!("{}{SIDECAR_EXT}", join_path(dir, name))
}

#[must_use]
pub fn join_path(dir: &str, name: &str) -> String {
    let dir = dir.trim_end_matches(['/', '\\']);
    if dir.is_empty() {
        return name.to_string();
    }
    format!("{dir}/{name}")
}

#[must_use]
pub fn from_content_disposition(cd: &str) -> Option<String> {
    let lower = cd.to_lowercase();
    let pos = lower.find("filename=")?;
    let rest = &cd[pos + "filename=".len()..];
    let token = rest.split(';').next()?.trim();
    let name = token
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(token)
        .trim()
        .to_string();
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains('\0') {
        return None;
    }
    Some(name)
}

#[must_use]
pub fn from_url_path(url: &str) -> Option<String> {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let path = after_scheme.split_once('/').map_or("", |(_, p)| p);
    let seg = path
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('/')
        .find(|s| !s.is_empty())
        .map(percent_decode);
    seg.filter(|s| !s.is_empty() && s != ".")
}

#[must_use]
pub fn derive_name(cd_filename: Option<&str>, final_url: Option<&str>, url: &str) -> String {
    if let Some(n) = cd_filename.map(str::trim).filter(|s| !s.is_empty()) {
        return sanitize_name(n);
    }
    if let Some(u) = final_url {
        if let Some(n) = from_url_path(u) {
            return sanitize_name(&n);
        }
    }
    if let Some(n) = from_url_path(url) {
        return sanitize_name(&n);
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("download-{ts}")
}

#[must_use]
pub fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '\0' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect()
}

fn decode_triplet(b: &[u8], i: usize) -> Option<u8> {
    if b[i] != b'%' || i + 2 >= b.len() {
        return None;
    }
    let hi = (b[i + 1] as char).to_digit(16)?;
    let lo = (b[i + 2] as char).to_digit(16)?;
    Some(((hi * 16) + lo) as u8)
}

#[must_use]
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if let Some(byte) = decode_triplet(b, i) {
            out.push(byte);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

#[must_use]
pub fn dedupe<F>(base: &str, exists: F) -> String
where
    F: Fn(&str) -> bool,
{
    if !exists(base) {
        return base.to_string();
    }
    for seq in 1..10_000u32 {
        let candidate = format!("{base}.{seq}");
        if !exists(&candidate) {
            return candidate;
        }
    }
    format!(
        "{base}.{ts}",
        ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    )
}

#[must_use]
pub fn exists_on_disk(save_dir: &str, name: &str) -> bool {
    let dir = Path::new(save_dir);
    dir.join(name).exists()
        || dir.join(format!("{name}{DOWNLOADING_EXT}")).exists()
        || dir.join(format!("{name}{SIDECAR_EXT}")).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cd_filename_extraction() {
        assert_eq!(
            from_content_disposition("attachment; filename=data.tar.gz"),
            Some("data.tar.gz".to_string())
        );
        assert_eq!(
            from_content_disposition("attachment; filename=\"my file.bin\"; size=1"),
            Some("my file.bin".to_string())
        );
        assert_eq!(from_content_disposition("inline"), None);
        assert_eq!(from_content_disposition("attachment; filename="), None);
    }

    #[test]
    fn url_last_segment() {
        assert_eq!(
            from_url_path("http://x/a/b/file.zip"),
            Some("file.zip".to_string())
        );
        assert_eq!(
            from_url_path("http://x/a/b/f.zip?q=1"),
            Some("f.zip".to_string())
        );
        assert_eq!(from_url_path("http://x/"), None);
        assert_eq!(
            from_url_path("http://x/a%20b.bin"),
            Some("a b.bin".to_string())
        );
    }

    #[test]
    fn url_partial_escape_tail_kept_verbatim() {
        assert_eq!(
            from_url_path("http://x/a%2.bin"),
            Some("a%2.bin".to_string())
        );
        assert_eq!(from_url_path("http://x/a%.bin"), Some("a%.bin".to_string()));
        assert_eq!(
            from_url_path("http://x/50%25.bin"),
            Some("50%.bin".to_string())
        );
    }

    #[test]
    fn derive_priority_cd_then_final_then_url() {
        assert_eq!(
            derive_name(
                Some("cd-name.bin"),
                Some("http://f/fin.bin"),
                "http://o/orig.bin"
            ),
            "cd-name.bin"
        );
        assert_eq!(
            derive_name(None, Some("http://f/fin.bin"), "http://o/orig.bin"),
            "fin.bin"
        );
        assert_eq!(derive_name(None, None, "http://o/orig.bin"), "orig.bin");
        let name = derive_name(None, None, "http://o/");
        assert!(name.starts_with("download-"));
    }

    #[test]
    fn sanitize_blocks_traversal() {
        assert_eq!(sanitize_name("a/b/c"), "a_b_c");
        assert_eq!(sanitize_name("..\\evil"), ".._evil");
    }

    #[test]
    fn dedupe_appends_seq() {
        let taken = ["a.bin", "a.bin.1"];
        let f = |n: &str| taken.contains(&n);
        assert_eq!(dedupe("a.bin", f), "a.bin.2");
        assert_eq!(dedupe("b.bin", f), "b.bin");
    }

    #[test]
    fn join_path_trims_trailing_slash() {
        assert_eq!(join_path("/dl/", "f"), "/dl/f");
        assert_eq!(join_path("/dl", "f"), "/dl/f");
        assert_eq!(join_path("", "f"), "f");
    }

    #[test]
    fn derive_name_falls_back_to_original_url_when_landing_has_no_path() {
        assert_eq!(
            derive_name(None, Some("http://f/"), "http://o/orig.bin"),
            "orig.bin"
        );
    }

    #[test]
    fn dedupe_falls_back_to_timestamp_when_all_taken() {
        let name = dedupe("f.bin", |_| true);
        assert!(name.starts_with("f.bin."), "got {name}");
        assert_ne!(name, "f.bin");
    }
}
