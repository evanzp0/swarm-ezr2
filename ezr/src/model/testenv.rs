//! testenv — 测试专用临时目录唯一化（仅 `cfg(test)` 编译，产品构建零产物）。
//!
//! 唯一事实来源：src 内全部 cfg(test) 的临时目录都经本模块构造。命名 =
//! `<前缀>-<pid>-<进程内自增序号>-<毫秒时间戳>`：序号使同进程内每次调用
//! 唯一（同毫秒并发调用不撞库），毫秒分量使跨会话/跨目标 pid 复用不再
//! 有可复现的撞库——固定 `ezr-<前缀>-<pid>` 式目录曾让后到测试进程经
//! `App::new` 恢复前到进程残留的 registry（跨目标污染，coder v116 修复，
//! 方案同 sidecar.rs tests 的 tmp_dir 序号法）。model 层为最内层、被全部
//! 测试 crate 收编，故收口于此；对 app/ui/engine 是合规的向下依赖。

/// 构造进程内唯一临时目录并创建（失败即 panic：测试前置条件不满足应立即暴露）
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
