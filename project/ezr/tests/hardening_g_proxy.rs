//! 加固测试（six-pack/hardender 步骤 7 批次 3）：src/bin/ezr-proxy.rs 存活体处置。
//!
//! 根因登记：放量轮 41 个存活属 **no_tests 批量类**——变异 runner 测试口径为
//! `--bin ezr --test hardening`，未包含 ezr-proxy bin 自身的 `#[cfg(test)]`
//! 单测（该套件已覆盖纯函数解析与 socket 网络层）。处置方式：以 `include!`
//! 把 bin 源挂载进本测试目标，其自带单测即进入 runner 口径（§2「整模块级
//! no_tests 批量类」处置落地后该类应整体消失）。
//!
//! 自带单测未覆盖的补充靶点：
//! - main 286:5（`replace main with ()`）：serve 模式必须真实拉起监听进程；
//! - main 288:14（`!=→==`）：分支反转 → serve 入参走 usage+exit(2)。
//! 等价/超时型（论证见 .work/tmp/step7/EQUIVALENCE.md）：
//! - dispatch_conn 194:45 `||→&&` 等价（首读错误 ⇒ 行必空）；
//! - parse_proxy_args 265:11 `+=→*=` 超时型（循环不推进，任何直测挂起）。
#![allow(missing_docs)]

#[path = "../src/bin/ezr-proxy.rs"]
mod sut;

/// 靶 main `286:5` / `288:14`：`ezr-proxy serve --port P` 必须拉起存活进程并在
/// P 上监听（变异体分别使进程空转退出 / 走 usage+exit(2) 分支）。
#[test]
fn proxy_serve_mode_starts_listener_process() {
    let exe = env!("CARGO_BIN_EXE_ezr-proxy");
    // 取一个空闲端口（先 bind(0) 探测后释放）
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
    assert!(alive, "serve 模式 600ms 后进程必须仍存活（空转/usage 退出即变异）");
    let conn = std::net::TcpStream::connect(("127.0.0.1", port));
    assert!(conn.is_ok(), "监听端口必须可连接");
    let _ = child.kill();
    let _ = child.wait();
}
