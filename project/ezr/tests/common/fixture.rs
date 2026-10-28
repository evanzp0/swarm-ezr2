#![allow(dead_code)]
#![allow(missing_docs)]

pub struct FxGuard(pub std::process::Child);

impl Drop for FxGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

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
