//! 加固测试（six-pack/hardender 步骤 7 批次 2）：engine/throttle + engine/error + engine/mod。
//! 靶点（mutants 行:列 口径）：
//! - throttle 68:63 `*→+`/`*→/`：突发容量 = rate×0.5，空闲蓄满后封顶；
//! - throttle 71:29 `-=→/=`、74:30 `<=→>`、78:30 `/→%|*`、78:37 `*→+|/`：
//!   耗尽边界——剩余归零必须立即返回；
//! - error 159:42 `||→&&`：body 读取失败须分类「传输中断」；
//! - error 189:5 恒串变异：连接失败 reason 必须内嵌真实错误链文本；
//! - error 195:18 `>→<`/`>→==`：链文本 >160 必须截断；
//! - engine/mod 313:5 `spawn_verify→()`：Cmd::Verify 必须产出 VerifyDone。
//!
//! 等价体（不写测试，论证见 .work/tmp/step7/EQUIVALENCE.md）：
//! throttle 70:25、81:21(`>=`)、81:21(`==`)、error 195:18(`>=`)；
//! 超时型：throttle 81:21(`<`)（不 yield 变异体在单线程执行器下饿死计时器）。
//!
//! 布局：ezr 为 bin-only crate，#[path] 挂载 engine/mod.rs 与 model/mod.rs
//! （内部相对 mod 声明自动解析到真实源树；产品自身 #[cfg(test)] 单测随挂载一并运行）。
// lint 姿态与产品 bin（src/main.rs crate 级 allow）对齐：#[path] 收编的产品源
// 在本测试 crate 内沿用产品面的豁免口径（产品面 clippy 0 的同一合同）。
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
// 测试挂载机制固有豁免：产品源经 #[path] 部分收编进独立测试 crate，部分产物项
// 在本 crate 上下文无消费者或被重复挂载（产品全量编译下均非死代码）。
#![allow(dead_code)]
#![allow(clippy::duplicate_mod)]
#![allow(unused_imports)]

#[path = "../src/engine/mod.rs"]
mod engine;
#[path = "../src/model/mod.rs"]
mod model;

use std::io::{Read, Write};
use std::time::Duration;

use engine::throttle::Throttle;

/// 靶 throttle `68:63`：突发容量 = rate × BURST_SECS(0.5)。
/// 空闲 600ms 蓄满后真实桶容量 5MB；`*→+`（cap=rate+0.5）与 `*→/`（cap=rate/0.5）
/// 变异体把桶放大到 ≥6MB → acquire(6MB) 立即完成；真实实现须再等 ~100ms。
#[tokio::test]
async fn throttle_burst_cap_is_rate_times_half_second() {
    let t = Throttle::new(10_000_000);
    tokio::time::sleep(Duration::from_millis(600)).await;
    let r = tokio::time::timeout(Duration::from_millis(50), t.acquire(6_000_000)).await;
    assert!(
        r.is_err(),
        "6MB > 5MB 桶容量应等待；立即完成说明 cap 被放大"
    );
}

/// 靶 throttle `71:29 / 74:30 / 78:30 / 78:37`：耗尽边界。
/// 预取 10_000（真实剩 40_000；`-=/` 变异体把 50_000/10_000 除成 5），
/// 再取 40_000：真实实现一次取尽、剩余 0 立即返回；`<=→>` 与四个算术变异体
/// 都会在剩余 0 时进入等待分支（20ms 死循环或 200ms 长眠），80ms 内不可能完成。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn throttle_drain_exits_immediately_at_zero_remaining() {
    let t = Throttle::new(100_000);
    let r = tokio::time::timeout(Duration::from_secs(2), t.acquire(10_000)).await;
    assert!(r.is_ok(), "预取 10KB 必须立即完成");
    let r = tokio::time::timeout(Duration::from_millis(80), t.acquire(40_000)).await;
    assert!(
        r.is_ok(),
        "40KB ≤ 剩余 40KB 必须立即完成（变异体在剩余 0 处死循环）"
    );
}

/// 靶 error `159:42`：`is_body() || is_decode()` —— body 读取失败（is_body=true、
/// is_decode=false）必须分类为「传输中断」；`&&` 变异体会落入兜底「网络错误」。
#[tokio::test]
async fn body_read_failure_classifies_as_transfer_interrupted() {
    let lst = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = lst.local_addr().expect("addr");
    let srv = std::thread::spawn(move || {
        let (mut s, _) = lst.accept().expect("accept");
        // 先读完请求再响应（hyper 在发送请求途中收到数据会报 UnexpectedMessage）
        let mut buf = [0u8; 4096];
        let _ = s.read(&mut buf);
        // 声明 1000 字节只发 16 字节，保活后关闭 → body 读取失败
        // （is_body=true、is_decode=false —— 只被 `||` 命中，`&&` 变异体漏判）
        s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\nshort-but-closed")
            .ok();
        s.flush().ok();
        std::thread::sleep(Duration::from_millis(300));
        drop(s);
    });
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    let err = client
        .get(format!("http://{addr}/x"))
        .send()
        .await
        .expect("响应应成功")
        .bytes()
        .await
        .expect_err("body 长度不足必须读取失败");
    srv.join().expect("server thread");
    let f = engine::error::classify_reqwest(&err);
    assert!(
        f.reason.starts_with("传输中断"),
        "body 错误须分类传输中断，got: {}",
        f.reason
    );
}

/// 靶 error `189:5`（恒串 "xyzzy"/""）：连接失败 reason 必须内嵌真实错误链文本。
#[tokio::test]
async fn connect_failure_reason_embeds_real_chain_text() {
    let client = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(3))
        .build()
        .expect("client");
    let err = client
        .get("http://127.0.0.1:1/ezr-hardening")
        .send()
        .await
        .expect_err("端口 1 必拒连");
    let f = engine::error::classify_reqwest(&err);
    assert!(f.reason.starts_with("连接失败（"), "reason={}", f.reason);
    assert!(
        f.reason.contains("refused") || f.reason.contains("os error 111"),
        "须内嵌真实链文本: {}",
        f.reason
    );
}

/// 靶 error `195:18`（`>→<`/`>→==`）：链文本 >160 必须截断——尾部标记不得漏出。
#[tokio::test]
async fn chain_text_truncated_at_160() {
    let tail = "ZZTAILMARK9";
    let path = format!("{}{tail}", "a".repeat(180));
    let client = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(3))
        .build()
        .expect("client");
    let err = client
        .get(format!("http://127.0.0.1:1/{path}"))
        .send()
        .await
        .expect_err("端口 1 必拒连");
    let f = engine::error::classify_reqwest(&err);
    assert!(f.reason.contains("连接失败（"), "reason={}", f.reason);
    assert!(
        !f.reason.contains(tail),
        "链文本须在 160 截断: {}",
        f.reason
    );
}

/// 靶 engine/mod `313:5`（`spawn_verify→()`）：Cmd::Verify 必须孵化校验任务并
/// 产出 `Evt::VerifyDone`；通过时完成改名 + 删 sidecar 收尾。
#[tokio::test]
async fn verify_command_spawns_verify_task() {
    let dir = std::env::temp_dir().join(format!("ezr-hard-verify-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let content = b"ezr hardening verify payload";
    let dl = dir.join("v.downloading");
    let fin = dir.join("v.bin");
    let side = dir.join("v.downloading.sidecar");
    std::fs::write(&dl, content).expect("write dl");
    std::fs::write(&side, "{}").expect("write sidecar");
    let idx = model::checksum::algo_index_by_name("SHA-256").expect("SHA-256 in CHECKSUM_ALGOS");
    let expected = model::checksum::digest_bytes(content, idx);

    let (evt_tx, mut evt_rx) = tokio::sync::mpsc::channel(16);
    let h = engine::EngineHandle::start(&model::config::Config::default(), evt_tx);
    h.send(engine::Cmd::Verify {
        spec: engine::supervisor::VerifySpec {
            id: 7,
            path: dl.to_string_lossy().into_owned(),
            final_path: fin.to_string_lossy().into_owned(),
            sidecar_path: side.to_string_lossy().into_owned(),
            algo: "SHA-256",
            expected,
        },
    })
    .await;

    let evt = tokio::time::timeout(Duration::from_secs(5), evt_rx.recv())
        .await
        .expect("5s 内必须收到校验事件")
        .expect("通道未关闭");
    match evt {
        engine::Evt::VerifyDone {
            id, ok, computed, ..
        } => {
            assert_eq!(id, 7);
            assert!(ok, "摘要应匹配 computed={computed}");
        }
        other => panic!("首个事件应为 VerifyDone，got {other:?}"),
    }
    assert!(fin.exists(), "校验通过后必须改名到最终路径");
    assert!(!dl.exists(), "校验通过后 .downloading 必须消失");
    let _ = std::fs::remove_file(&side);
    let _ = std::fs::remove_dir_all(&dir);
}
