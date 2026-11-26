#![allow(missing_docs)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]
#![allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::too_many_arguments
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
#![allow(dead_code)]
#![allow(clippy::duplicate_mod)]
#![allow(unused_imports)]

#[path = "../src/bin/ezr-proxy.rs"]
mod sut;

#[test]
fn proxy_serve_mode_starts_listener_process() {
    let exe = env!("CARGO_BIN_EXE_ezr-proxy");
    let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("probe bind");
    let port = probe.local_addr().expect("port").port();
    drop(probe);

    let mut child = std::process::Command::new(exe)
        .args([
            "serve",
            "--port",
            &port.to_string(),
            "--name",
            "hardening-t",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn ezr-proxy");

    std::thread::sleep(std::time::Duration::from_millis(600));
    let alive = child.try_wait().expect("try_wait").is_none();
    assert!(
        alive,
        "serve 模式 600ms 后进程必须仍存活（空转/usage 退出即变异）"
    );
    let conn = std::net::TcpStream::connect(("127.0.0.1", port));
    assert!(conn.is_ok(), "监听端口必须可连接");
    let _ = child.kill();
    let _ = child.wait();
}
