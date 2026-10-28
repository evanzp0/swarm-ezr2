pub(crate) fn uniq_tmp_dir(prefix: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let d = std::env::temp_dir().join(format!("{prefix}-{}-{seq}-{ms}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}
