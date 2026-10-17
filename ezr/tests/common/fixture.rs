//! 共享 fixture：ezr-fixture 服务进程的拉起与生命周期守卫。
//! 加固测试共用（hardening_g_appcore / hardening_g_engine 的真实下载场景）；
//! 进程守卫保证断言失败路径上也不泄漏服务进程。

#![allow(dead_code)]
#![allow(missing_docs)]

/// ezr-fixture 子进程守卫：drop 时 kill + wait（断言失败/panic 路径不泄漏）。
pub struct FxGuard(pub std::process::Child);

impl Drop for FxGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// 在空闲端口拉起 `ezr-fixture serve --root <root>`（root 须已含待服务文件）。
/// 返回（进程守卫, 端口）；就绪等待用有界重试短连接探测（阻塞式，调用方
/// 应在创建被测 App 之前调用，避免阻塞 tokio 运行时中的驱动任务）。
pub fn spawn_serve(root: &std::path::Path) -> (FxGuard, u16) {
    let port = {
        let l = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("占位端口");
        l.local_addr().expect("端口").port()
    };
    let child = std::process::Command::new(env!("CARGO_BIN_EXE_ezr-fixture"))
        .args([
            "serve",
            "--root",
            root.to_string_lossy().as_ref(),
            "--port",
            &port.to_string(),
        ])
        .spawn()
        .expect("拉起 ezr-fixture");
    let guard = FxGuard(child);
    for _ in 0..50 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return (guard, port);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("ezr-fixture 未就绪");
}
