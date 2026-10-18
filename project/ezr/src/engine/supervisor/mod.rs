//! supervisor — 单任务下载监督者（探测 → 分块下载 → 完成/失败收尾 → 校验）
//!
//! 生命周期：`Cmd::Start` 携带 [`TaskSpec`] 触发 spawn。流程：
//! 1. 探测（`GET` + `Range: bytes=0-`）拿 Content-Length / ETag /
//!    Last-Modified / Content-Disposition（FR-01-10）；
//! 2. 续传一致性检查（spec 带 sidecar 时）：失效 → 作废断点回报
//!    [`Evt::Invalidated`]（不计失败、不占重试计数，FR-01-22/D11）；
//!    一致 → 恢复块表续传；
//! 3. 分块下载（支持 Range 且多块）：worker 池动态领块、乱序完成、
//!    实际并发 = min(并发数, 未完成块数)（FR-01-11）；单块或不支持 Range →
//!    单流降级（FR-01-12）；
//! 4. 完成：全块 + 大小校验（FR-01-52）→ [`Evt::DownloadDone`]；有校验值时
//!    App 转「校验中」并下发 `Cmd::Verify`，通过后 finalize
//!    （去 `.downloading` 扩展名 + 删 sidecar，FR-01-24/20）；
//! 5. 暂停/失败：停 worker → 块表快照写 sidecar（原子写）→ 回报事件。
//!
//! 停止信号用 `tokio::sync::watch`；worker 在流读取 select 中响应，无忙轮询。
//! sidecar 周期落盘（2s）+ 暂停/失败即时落盘（NFR-2：kill -9 不损坏、最多丢
//! 一个窗口的进度）。
//!
//! 模块布局（architect v145：原单文件 114 变异点 > 100 阈值，按职责目录化——
//! 与 app/engine v116 拆分同法，各文件变异点均低于阈值，hardender 可 `--file`
//! 分块跑变异；纯移动零行为变化，#[path] 挂载测试 crate 经 engine/mod.rs 的
//! `mod supervisor;` 解析等价，见 notes/rust.md「单文件升级为目录模块」）：
//! - 本文件（mod.rs）：域类型（TaskSpec/VerifySpec/Flow/PausedState）、入口
//!   run/verify、事件发送与 sidecar 路径/台账小 helper + 测试模块（原地保留）；
//! - `http`：请求构建（identity 头）与请求臂失败分类收口 req_checked、
//!   探测响应头解析；
//! - `finalize`：完成收尾单源与 sidecar 记账（build/flush/failed_flow/
//!   failed_ledger/block_span）；
//! - `workers`：worker 池（WorkerDeps/block_worker/stop/prune/fill）；
//! - `download`：多块监督主流程（探测 select、命令臂、进度 tick、完成判定）；
//! - `single`：单流降级路径。

// 测试模块（本文件 cfg(test) mod 经 use super::* 消费）专用导入：产品构建
// 不产生该路径（notes/rust.md「目录化三机械点」③）。Ordering/AtomicU32 为
// mock 服务器连接序号与 redirect 计数用；其余为测试构造 TaskSpec/Sidecar
// /断言口径所用（Duration/Path 等测试经 std:: 全路径引用，无需导入）。
#[cfg(test)]
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;

use super::error::EngineFailure;
use super::{Evt, TaskCmd};
use crate::model::chunk::{block_range, Blocks};
use crate::model::config::ProxyEndpoint;
#[cfg(test)]
use crate::model::consistency::ServerStamp;
use crate::model::sidecar::Sidecar;
#[cfg(test)]
use crate::model::sidecar::SidecarTask;
#[cfg(test)]
use crate::model::FailKind;
use crate::model::{checksum, namegen, Protocol};

mod download;
mod finalize;
mod http;
mod single;
mod workers;

use download::download;

/// 任务运行上下文（App 在 `Cmd::Start` 时构建携带）
#[derive(Clone, Debug)]
pub struct TaskSpec {
    /// 任务 ID
    pub id: u32,
    /// 原始 URL
    pub url: String,
    /// 保存目录
    pub save_dir: String,
    /// 文件名（添加时由 URL 推导；探测后 CD 优先改名）
    pub name: String,
    /// 并发数
    pub concurrency: usize,
    /// 块大小
    pub block_size: u64,
    /// 协议
    pub protocol: Protocol,
    /// 校验期望算法显示名（如有）
    pub expected_algo: Option<&'static str>,
    /// 校验期望值（十六进制小写，如有）
    pub expected_value: Option<String>,
    /// sidecar 内容（App 预读；None = 无断点）
    pub sidecar: Option<Sidecar>,
    /// sidecar 盘上路径（作废断点时删除用；sidecar 存在时必填）
    pub sidecar_path: Option<String>,
    /// 沿用既有任务名（探测后的重启：失效重下/续传）；false = 首次启动，
    /// 按规则推导并盘上去重（FR-01-02）。失效重启若重新去重，会因残留的
    /// `.downloading` 文件漂移到新名（m.bin → m.bin.1…），导致 sidecar 落盘
    /// 名与任务名错位、下轮续传一致性检查被跳过（FR-01-22 违例），故必须钉住。
    pub keep_name: bool,
    /// 任务添加时间（Unix 秒）
    pub added_at: u64,
    /// 任务级代理端点（v1.5/FR-01-86/D18；None = 直连。App 已解析三态选择；
    /// url 合法性由引擎在 start_task 时校验，非法回退直连 + toast）
    pub proxy_endpoint: Option<ProxyEndpoint>,
}

/// 校验请求上下文（App 在 `Cmd::Verify` 时构建携带）
#[derive(Clone, Debug)]
pub struct VerifySpec {
    /// 任务 ID
    pub id: u32,
    /// 待校验文件（`.downloading` 形态）
    pub path: String,
    /// 最终目标路径（通过后 rename 到此）
    pub final_path: String,
    /// sidecar 路径（通过后删除）
    pub sidecar_path: String,
    /// 算法显示名（`CHECKSUM_ALGOS`）
    pub algo: &'static str,
    /// 期望摘要（十六进制小写）
    pub expected: String,
}

/// supervisor 入口：跑下载流程并按终态收尾（落盘 sidecar + 回报事件）
pub(crate) async fn run(
    spec: TaskSpec,
    shared: Arc<super::EngineShared>,
    mut cmd_rx: mpsc::Receiver<TaskCmd>,
    evt_tx: mpsc::Sender<Evt>,
) {
    match download(&spec, &shared, &mut cmd_rx, &evt_tx).await {
        Flow::Completed => {}
        Flow::Paused(state) => {
            let _ = Sidecar::save(&state.sidecar, &state.sidecar_path);
            let _ = evt_tx
                .send(Evt::PausedDone {
                    id: spec.id,
                    downloaded: state.blocks.downloaded(),
                    chunk_done: state.blocks.completed(),
                    // 终态逐块真值（stop_workers 后快照）：pieces 🟨 收尾口径
                    partial_blocks: state.blocks.partial_indices(),
                })
                .await;
        }
        Flow::Cancelled => {
            // 回报停止确认：App 据此执行延迟文件删除（删除任务和文件）
            let _ = evt_tx.send(Evt::Cancelled { id: spec.id }).await;
        }
        Flow::Failed(f, made_progress, sidecar) => {
            if let Some((sc, sc_path)) = &sidecar {
                let _ = Sidecar::save(sc, sc_path);
            }
            let (downloaded, chunk_done, partial_blocks) = match &sidecar {
                Some((sc, _)) => (sc.downloaded, completed_of(sc), partial_of(sc)),
                None => (0, 0, Vec::new()),
            };
            let _ = evt_tx
                .send(Evt::Failed {
                    id: spec.id,
                    kind: f.kind,
                    reason: f.reason,
                    retry_after: f.retry_after,
                    made_progress,
                    downloaded,
                    chunk_done,
                    // 失败台账逐块真值：pieces 🟨 收尾口径（无台账失败臂为空集）
                    partial_blocks,
                })
                .await;
        }
        Flow::Invalidated => {
            // 作废断点：删 sidecar、回报失效（不计失败，FR-01-22）
            if let Some(p) = &spec.sidecar_path {
                let _ = Sidecar::remove(p);
            }
            let _ = evt_tx.send(Evt::Invalidated { id: spec.id }).await;
        }
    }
}

/// sidecar 中已完成块数（Failed 事件汇总用）
fn completed_of(sc: &Sidecar) -> u32 {
    sc.blocks
        .iter()
        .zip(0u32..)
        .filter(|(w, i)| block_range(sc.size, sc.block_size, *i).is_some_and(|(s, e)| **w >= e - s))
        .count() as u32
}

/// sidecar 中部分下载块号集合（升序；Failed 事件 pieces 🟨 汇总用，v131）。
/// 与 [`completed_of`] 同口径单次扫描（0 < written < 块长）。
fn partial_of(sc: &Sidecar) -> Vec<u32> {
    sc.blocks
        .iter()
        .zip(0u32..)
        .filter(|(w, i)| {
            block_range(sc.size, sc.block_size, *i).is_some_and(|(s, e)| **w > 0 && **w < e - s)
        })
        .map(|(_, i)| i)
        .collect()
}

/// sidecar 路径推导（委托 model 单源 namegen::sidecar_path_of，architect v116：
/// 原 `.ezr` 后缀手拼与模型 Task 方法重复）
fn sidecar_path(save_dir: &str, name: &str) -> String {
    namegen::sidecar_path_of(save_dir, name)
}

/// 下载流程结果
enum Flow {
    /// 完成（DownloadDone 已报）
    Completed,
    /// 暂停（sidecar 已构建待落盘）
    Paused(PausedState),
    /// 取消（删除任务，不落盘）
    Cancelled,
    /// 失败（失败信息 + 是否有进展 + 可选 sidecar 待落盘）
    Failed(EngineFailure, bool, Option<(Sidecar, String)>),
    /// 一致性失效（FR-01-22）
    Invalidated,
}

/// 暂停态
struct PausedState {
    blocks: Blocks,
    sidecar: Sidecar,
    sidecar_path: String,
}

/// 事件发送辅助（App 退出后静默）
async fn emit(evt: &mpsc::Sender<Evt>, e: Evt) -> bool {
    evt.send(e).await.is_ok()
}

/// 校验（App 转校验中后下发；流式哈希不整载内存，FR-01-51）
pub(crate) async fn verify(spec: VerifySpec, evt_tx: mpsc::Sender<Evt>) {
    let path = spec.path.clone();
    // 规范名回查单源 checksum::algo_index_exact_or_default（architect v116：
    // 原内联 exact-match + unwrap_or(3) 与修改对话框预填重复）
    let algo_idx = checksum::algo_index_exact_or_default(spec.algo);
    let computed = tokio::task::spawn_blocking(move || checksum::digest_file(&path, algo_idx))
        .await
        .unwrap_or_else(|e| Err(format!("校验任务失败（{e}）")));
    match computed {
        Ok(hex) => {
            let ok = hex.eq_ignore_ascii_case(&spec.expected);
            if ok {
                // 收尾：去 `.downloading` 扩展名 + 删 sidecar（FR-01-24/20）
                let _ = tokio::fs::rename(&spec.path, &spec.final_path).await;
                let _ = Sidecar::remove(&spec.sidecar_path);
            }
            let _ = evt_tx
                .send(Evt::VerifyDone {
                    id: spec.id,
                    ok,
                    computed: hex,
                    expected: spec.expected,
                })
                .await;
        }
        Err(e) => {
            // 文件不可读（被外部删除等）：按校验失败停等
            let _ = evt_tx
                .send(Evt::VerifyDone {
                    id: spec.id,
                    ok: false,
                    computed: e,
                    expected: spec.expected,
                })
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    use super::*;

    /// 确定性内容（与 fixture 同模式）
    fn fill(buf: &mut [u8], start: u64) {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = ((start as usize + i) % 251) as u8;
        }
    }

    /// mock HTTP 服务器：支持 Range/状态码注入/慢速；返回 (地址, 关闭标志)。
    /// 线程 detach（engineering.md：无退出条件的测试线程一律 detach）。
    fn mock_server(total: u64, opts: Vec<(String, String)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        // 连接序号计数（droponce 门控：仅首个连接半途断连，后续正常——
        // 同一 URL 内构造「首轮断连、重试恢复」的用户故障形态）
        let conn_seq = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        std::thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                let seq = conn_seq.fetch_add(1, Ordering::Relaxed);
                let mut w = conn.try_clone().unwrap();
                let mut r = BufReader::new(conn);
                let mut line = String::new();
                if r.read_line(&mut line).is_err() || line.is_empty() {
                    continue;
                }
                let mut range_header = None;
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).is_err() || h.trim().is_empty() {
                        break;
                    }
                    if let Some((k, v)) = h.split_once(':') {
                        if k.trim().eq_ignore_ascii_case("range") {
                            range_header = Some(v.trim().to_string());
                        }
                    }
                }
                let want_status = opts
                    .iter()
                    .find(|(k, _)| k == "status")
                    .and_then(|(_, v)| v.parse::<u16>().ok());
                if let Some(code) = want_status {
                    let _ = write!(w, "HTTP/1.1 {code} Injected\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    continue;
                }
                // v1.27 回归门控：dropseq = 第 N 个连接不回任何响应直接断开
                // （构造「探测成功、续传 Range 请求即断」的请求臂失败形态）
                if opts
                    .iter()
                    .find(|(k, _)| k == "dropseq")
                    .and_then(|(_, v)| v.parse::<u32>().ok())
                    .is_some_and(|n| seq == n)
                {
                    continue;
                }
                let norange = opts.iter().any(|(k, _)| k == "norange");
                // v1.26 回归门控：noclen = 响应缺 Content-Length（超时型/降级
                // 服务器的典型形态）；drophalf = 传输半途断连（连接超时故障形态）
                let noclen = opts.iter().any(|(k, _)| k == "noclen");
                let drophalf = opts.iter().any(|(k, _)| k == "drophalf");
                let droponce = opts.iter().any(|(k, _)| k == "droponce");
                let (start, end, status) = match range_header
                    .as_deref()
                    .and_then(|v| v.strip_prefix("bytes="))
                {
                    Some(rv) if !norange => {
                        let (a, b) = rv.split_once('-').unwrap();
                        let s = a.parse::<u64>().unwrap_or(0);
                        let e = if b.is_empty() {
                            total - 1
                        } else {
                            b.parse::<u64>().unwrap_or(total - 1).min(total - 1)
                        };
                        (s, e, 206)
                    }
                    _ => (0, total - 1, 200),
                };
                let len = end - start + 1;
                let cl = if noclen {
                    String::new()
                } else {
                    format!("Content-Length: {len}\r\n")
                };
                let _ = write!(
                    w,
                    "HTTP/1.1 {status} OK\r\nAccept-Ranges: bytes\r\nETag: \"mock-{total}\"\r\nLast-Modified: Mon, 09 Feb 2026 08:00:00 GMT\r\n{cl}Connection: close\r\n\r\n"
                );
                let slow = opts.iter().any(|(k, _)| k == "slow");
                // 挂起门控（v1.17 计划外对齐，同轮扩面）：头部照发、正文永不应答
                // （阻塞读等客户端断开），对该连接的**一切请求**生效——探测
                // （开区间 `bytes=0-`）只需头部（Probed 事件照常），分块 worker
                // （有界区间）与单流（复用探测响应直接流 body、无独立 Range
                // 请求）全部阻塞在首字节读上，取消竞态测试中取消必然先于任何
                // 数据/进度/完成事件被处理。
                // 背景两段：①微文件瞬时完成与取消处理存在调度赛跑，workspace
                // 并行负载下偶发 DownloadDone 先于 Evt::Cancelled（原测试无门控
                // 靠时序幸运，硬定时非确定性）；②首轮门控只拦有界区间，漏了
                // v1.16/FR-01-104 1MB 下限把微文件块计划收敛成单块后的单流
                // 路径（total_blocks <= 1 走 single_stream 流探测响应 body）——
                // 门控对它失明，赛跑复现（fail-fast 全量实测 1/4 挂载点引爆
                // 「取消后不应完成下载」）。
                let hold = opts.iter().any(|(k, _)| k == "hold");
                if hold {
                    let mut sink = [0u8; 1];
                    let _ = std::io::Read::read(&mut r, &mut sink);
                    continue;
                }
                let mut buf = vec![0u8; 8192];
                let mut pos = start;
                while pos <= end {
                    let n = (end - pos + 1).min(8192) as usize;
                    fill(&mut buf[..n], pos);
                    if w.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    pos += n as u64;
                    if slow {
                        // 慢速门控：8 KB/15ms ≈ 0.53 MB/s，1 MB 块约 1.9 s——
                        // 构造跨多个进度帧的块内观察窗（FR-01-105/FR-01-87 测试用）
                        std::thread::sleep(std::time::Duration::from_millis(15));
                    }
                    if (drophalf || (droponce && seq == 0)) && pos - start >= len / 2 {
                        // 半途断连：读写句柄随循环作用域关闭（abrupt close，
                        // 客户端读到截断体 → 传输中断错误）；droponce 仅首个连接
                        break;
                    }
                }
            }
        });
        format!("http://{addr}/f.bin")
    }

    /// 重定向服务器（v1.28 回归夹具）：对**每一个**请求回 302 → `target`，
    /// 命中计数累入 `hits`。v1.28 口径下探测与全部数据请求各自从原始 URL
    /// （本服务器）独立跟跳转——worker 每个分块请求都应产生一次命中；
    /// 旧口径（探测捕获落点共享 worker 池）下仅探测命中 1 次。
    fn redirect_server(target: String, hits: std::sync::Arc<AtomicU32>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                let mut w = conn.try_clone().unwrap();
                let mut r = BufReader::new(conn);
                let mut line = String::new();
                if r.read_line(&mut line).is_err() || line.is_empty() {
                    continue;
                }
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).is_err() || h.trim().is_empty() {
                        break;
                    }
                }
                hits.fetch_add(1, Ordering::Relaxed);
                let _ = write!(
                    w,
                    "HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
            }
        });
        format!("http://{addr}/f.bin")
    }

    /// 消费事件至 DownloadDone（Failure/Invalidated 即 panic；redirect 用例共用事件泵）
    async fn pump_until_done(rx: &mut mpsc::Receiver<Evt>, deadline: std::time::Instant) {
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::DownloadDone { .. })) => return,
                Ok(Some(Evt::Invalidated { .. })) => panic!("重定向链路不应失效"),
                Ok(Some(Evt::Failed { reason, .. })) => panic!("重定向链路不应失败: {reason}"),
                Err(_) => panic!("超时"),
                _ => {}
            }
        }
        panic!("超时");
    }

    fn spec_for(
        id: u32,
        url: &str,
        dir: &str,
        block: u64,
        conc: usize,
        expected: Option<(&'static str, String)>,
    ) -> TaskSpec {
        TaskSpec {
            id,
            url: url.to_string(),
            save_dir: dir.to_string(),
            name: "f.bin".to_string(),
            concurrency: conc,
            block_size: block,
            protocol: Protocol::Http,
            expected_algo: expected.as_ref().map(|(a, _)| *a),
            expected_value: expected.as_ref().map(|(_, v)| v.clone()),
            sidecar: None,
            sidecar_path: None,
            keep_name: false,
            added_at: 0,
            proxy_endpoint: None,
        }
    }

    fn throttle_free() -> Arc<super::super::EngineShared> {
        Arc::new(super::super::EngineShared {
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            clients: std::sync::Mutex::new(std::collections::HashMap::new()),
            throttle: Arc::new(super::super::throttle::Throttle::new(0)),
        })
    }

    /// 构建 sidecar 接续 spec（断点测试夹具共用）
    fn sidecar_spec(id: u32, url: &str, dir: &std::path::Path, conc: usize) -> TaskSpec {
        let mut spec = spec_for(id, url, dir.to_str().unwrap(), 4096, conc, None);
        spec.sidecar = Some(Sidecar::load(dir.join("f.bin.ezr").to_str().unwrap()).unwrap());
        spec.sidecar_path = Some(dir.join("f.bin.ezr").to_str().unwrap().to_string());
        spec
    }

    /// 启动一次 run()：返回 (命令句柄 keep-alive，事件接收端，10s 超时截止)。
    /// cmd_tx 必须由调用方持有到断言结束：所有 sender drop 会让 supervisor 的
    /// cmd_rx.recv() 立即得到 None → Flow::Cancelled，下载永远完不成。
    #[allow(clippy::type_complexity)]
    fn launch_spec(
        spec: TaskSpec,
    ) -> (
        mpsc::Sender<TaskCmd>,
        mpsc::Receiver<Evt>,
        std::time::Instant,
    ) {
        let (tx, rx) = mpsc::channel::<Evt>(64);
        let (cmd_tx, cmd_rx) = mpsc::channel::<TaskCmd>(4);
        tokio::spawn(run(spec, throttle_free(), cmd_rx, tx));
        (
            cmd_tx,
            rx,
            std::time::Instant::now() + std::time::Duration::from_secs(10),
        )
    }

    /// 泵事件至 DownloadDone（返回 true）或 deadline（返回 false）；
    /// Failed 即 panic（两个 keep_name 重启用例共用的事件泵）。
    async fn wait_download_done(
        rx: &mut mpsc::Receiver<Evt>,
        deadline: std::time::Instant,
    ) -> bool {
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::DownloadDone { .. })) => return true,
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                _ => {}
            }
        }
        false
    }

    /// v1.5/FR-01-87（D19）：Reconfigure 并发热调——1→4 上调后新 worker 加入
    /// （活跃连接数峰值 ≥2），任务完成且无失败（代理热调同通道，None=直连）。
    /// v1.16/FR-01-104 对齐：块计划入口下限 1 MB 后，测试规格改用 1 MB 块
    /// （2 块文件 + 慢速门控，保证多帧观测窗与上调后新 worker 领块）
    #[tokio::test]
    async fn reconfigure_hot_adjusts_concurrency_and_completes() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-hot");
        let total: u64 = 2 * 1024 * 1024; // 2 块 × 1 MB（下限领域；慢速维持观测窗）
        let url = mock_server(total, vec![("slow".into(), String::new())]);
        let spec = spec_for(
            1,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            1,
            None,
        );
        let (cmd_tx, mut rx, deadline) = launch_spec(spec);
        let mut hot = false;
        let mut max_conns = 0usize;
        let mut done = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Probed { .. })) => {
                    // 探测完成即上调并发：1 → 4（直连）
                    cmd_tx
                        .send(TaskCmd::Reconfigure {
                            concurrency: 4,
                            endpoint: None,
                        })
                        .await
                        .unwrap();
                    hot = true;
                }
                Ok(Some(Evt::Progress { conns, .. })) => {
                    max_conns = max_conns.max(conns.len());
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(hot, "Reconfigure 应在探测完成后发出");
        assert!(done, "热调后任务应完成");
        assert!(max_conns >= 2, "上调后活跃连接数应上升（峰值 {max_conns}）");
        let f = dir.join("f.bin");
        let meta = std::fs::metadata(&f).expect("目标文件在位");
        assert_eq!(meta.len(), total);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// v1.16/FR-01-105：任务级 Progress 读数实时化——块内（chunk_done 未增长）
    /// downloaded 随字节落盘即时推进且严格单调（旧实现 = blocks.downloaded()
    /// 块边界粒度，块内恒为基线值 → 本测试红）
    #[tokio::test]
    async fn progress_task_bytes_live_within_block() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-live");
        let total: u64 = 1024 * 1024; // 1 块（1 MB），慢速门控跨多个进度帧
        let url = mock_server(total, vec![("slow".into(), String::new())]);
        let spec = spec_for(
            1,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            1,
            None,
        );
        let (_cmd_tx, mut rx, deadline) = launch_spec(spec);
        let mut in_block_samples: Vec<u64> = Vec::new();
        let mut done = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Progress {
                    downloaded,
                    chunk_done,
                    ..
                })) => {
                    if chunk_done == 0 && downloaded > 0 {
                        in_block_samples.push(downloaded);
                    }
                }
                Ok(Some(Evt::DownloadDone { total: t, .. })) => {
                    assert_eq!(t, total);
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(done, "任务应完成");
        assert!(
            in_block_samples.len() >= 3,
            "块内应出现多个非零实时读数（got {}）",
            in_block_samples.len()
        );
        assert!(
            in_block_samples.windows(2).all(|w| w[0] < w[1]),
            "块内读数应严格递增: {in_block_samples:?}"
        );
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        assert_eq!(data.len() as u64, total);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// v1.16/FR-01-104（D31）：块计划入口生效值下限 1 MB——spec.block_size = 1
    /// 的新一次下载按 1 MB 块计划（连接区间 = 1 MB；旧实现 1 字节块 → 本测试红）；
    /// 任务完成且文件正确（钳制不破坏下载链路）
    #[tokio::test]
    async fn block_plan_clamps_to_one_mb_floor() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-floor");
        let total: u64 = 2 * 1024 * 1024; // 钳制后 = 2 块 × 1 MB
        let url = mock_server(total, vec![("slow".into(), String::new())]);
        let spec = spec_for(2, &url, dir.to_str().unwrap(), 1, 1, None); // 遗留 1 字节块值
        let (_cmd_tx, mut rx, deadline) = launch_spec(spec);
        let mut first_range: Option<(u64, u64)> = None;
        let mut done = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Progress { conns, .. })) => {
                    if first_range.is_none() {
                        if let Some(c) = conns.first() {
                            first_range = Some((c.start, c.end));
                        }
                    }
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(done, "钳制后任务应完成");
        let (s, e) = first_range.expect("应观测到连接块区间");
        assert_eq!(
            e - s,
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            "块计划应按 1 MB 下限钳制（got {s}..{e}）"
        );
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect, "钳制路径文件内容应逐字节一致");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// v1.16/FR-01-87 修订臂（D30）：并发下调即时收缩——2→1 确定后约 1 秒内
    /// 多余连接的在传块被中止（不再等待块边界，旧实现块级过渡延迟 → 本测试红）、
    /// 连接行消失；被中断块块内进度保留（最终文件逐字节正确、无回退）
    #[tokio::test]
    async fn reconfigure_scale_down_interrupts_inflight_immediately() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-scaledown");
        let total: u64 = 2 * 1024 * 1024; // 2 块 × 1 MB（慢速 1.9 s/块）
        let url = mock_server(total, vec![("slow".into(), String::new())]);
        let spec = spec_for(
            3,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            2,
            None,
        );
        let (cmd_tx, mut rx, deadline) = launch_spec(spec);
        let mut downgraded_at: Option<std::time::Instant> = None;
        let mut max_conns_after = 0usize;
        let mut last_downloaded: u64 = 0;
        let mut done = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Probed { .. })) => {
                    // 探测完成即下调并发：2 → 1（两 worker 均已在途）
                    cmd_tx
                        .send(TaskCmd::Reconfigure {
                            concurrency: 1,
                            endpoint: None,
                        })
                        .await
                        .unwrap();
                    downgraded_at = Some(std::time::Instant::now());
                }
                Ok(Some(Evt::Progress {
                    downloaded, conns, ..
                })) => {
                    assert!(
                        downloaded >= last_downloaded,
                        "已下载字节单调不减: {last_downloaded} -> {downloaded}"
                    );
                    last_downloaded = downloaded;
                    if let Some(t0) = downgraded_at {
                        if t0.elapsed() >= std::time::Duration::from_millis(600) {
                            max_conns_after = max_conns_after.max(conns.len());
                        }
                    }
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(downgraded_at.is_some(), "下调命令应已发出");
        assert!(done, "下调后任务应完成");
        assert!(
            max_conns_after <= 1,
            "下调 600ms 后活跃连接数应 ≤ 1（got {max_conns_after}）——旧实现块级过渡延迟会超窗"
        );
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect, "被中断块的块内进度应保留（文件逐字节一致）");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn end_to_end_resumable_download_completes() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e");
        let total: u64 = 3 * 4096 + 1111; // 4 块（块 4KB），末块吸收余数
        let url = mock_server(total, vec![]);
        let (_cmd_tx, mut rx, deadline) =
            launch_spec(spec_for(1, &url, dir.to_str().unwrap(), 4096, 4, None));
        let mut done = false;
        let mut probed = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::DownloadDone { total: t, .. })) => {
                    assert_eq!(t, total);
                    done = true;
                    break;
                }
                Ok(Some(Evt::Probed {
                    total: t,
                    resumable: true,
                    ..
                })) => {
                    assert_eq!(t, total);
                    probed = true;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                Ok(Some(_)) => {}
                _ => {}
            }
        }
        assert!(probed, "未收到探测事件");
        assert!(done, "未收到完成事件");
        // 文件字节一致（.downloading 改名后为 f.bin；无校验引擎已 finalize）
        let path = dir.join("f.bin");
        let data = std::fs::read(&path).unwrap();
        assert_eq!(data.len() as u64, total);
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect, "文件内容应逐字节一致");
        // sidecar 已删（完成收尾）
        assert!(!dir.join("f.bin.ezr").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn cancel_emits_cancelled_and_no_ghost_progress() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e-cancel");
        let total: u64 = 3 * 4096 + 1111;
        // v1.17 计划外对齐：hold 门控（有界区间请求正文永不应答）——微文件瞬时
        // 完成与取消处理存在调度赛跑，workspace 并行负载下偶发 DownloadDone
        // 先于 Evt::Cancelled（非确定性失败两次复现于基线复跑）；门控后取消
        // 必然先于任何数据/完成事件被处理，测试转确定性
        let url = mock_server(total, vec![("hold".into(), String::new())]);
        let (tx, mut rx) = mpsc::channel::<Evt>(64);
        let shared = throttle_free();
        let (cmd_tx, cmd_rx) = mpsc::channel::<TaskCmd>(4);
        let spec = spec_for(9, &url, dir.to_str().unwrap(), 4096, 4, None);
        // 取消命令先于 run 入队：v1.25 探测 select 消费命令后，取消在探测
        // 在途即命中（即时停机）或在探测完成后的首个监督轮询命中——两者
        // 顺序均合法（loopback 头部到达与取消命令的就绪竞速为 50/50），
        // 本测试不锁定 Probed 是否先到，只锁定取消语义：Cancelled 必到、
        // 无幽灵进度、无完成、无产物
        cmd_tx.send(TaskCmd::Cancel).await.unwrap();
        tokio::spawn(run(spec, shared, cmd_rx, tx));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut probed = false;
        let mut cancelled = false;
        let mut ghost = false;
        let mut done = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Probed {
                    resumable: true, ..
                })) => probed = true,
                Ok(Some(Evt::Cancelled { .. })) => {
                    cancelled = true;
                    break;
                }
                Ok(Some(Evt::Progress { .. })) => ghost = true,
                Ok(Some(Evt::DownloadDone { .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                Ok(Some(_)) => {}
                _ => {}
            }
        }
        assert!(!done, "取消后不应完成下载");
        assert!(!ghost, "取消后不应再有进度事件");
        let _ = probed; // 探测事件与取消事件的先后不锁定（见上）
        assert!(cancelled, "未收到取消确认事件（Evt::Cancelled）");
        // Flow::Cancelled 路径不 finalize、不写 sidecar：最终文件与 .ezr 都不应出现
        assert!(!dir.join("f.bin").exists(), "取消不应产出最终文件");
        assert!(!dir.join("f.bin.ezr").exists(), "取消路径不应写 sidecar");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn norange_server_falls_back_single_stream() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e2");
        let total: u64 = 9000;
        let url = mock_server(total, vec![("norange".to_string(), "1".to_string())]);
        let (_cmd_tx, mut rx, deadline) =
            launch_spec(spec_for(2, &url, dir.to_str().unwrap(), 4096, 4, None));
        let mut done = false;
        let mut non_resumable = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::DownloadDone { total: t, .. })) => {
                    assert_eq!(t, total);
                    done = true;
                    break;
                }
                Ok(Some(Evt::Probed { resumable, .. })) => {
                    if !resumable {
                        non_resumable = true;
                    }
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                _ => {}
            }
        }
        assert!(non_resumable, "应探测为不支持续传");
        assert!(done, "单流应完成");
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        assert_eq!(data.len() as u64, total);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 失效重启（keep_name=true）必须沿用既有任务名：残留 `.downloading` 文件
    /// 不得触发去重改名（否则 sidecar 落盘名与任务名错位，FR-01-22 一致性
    /// 检查在下轮续传被跳过）；keep_name=false（首次启动）仍按规则去重。
    #[tokio::test]
    async fn keep_name_restart_skips_dedupe() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e4");
        let total: u64 = 9000;
        let url = mock_server(total, vec![]);
        // 残留的部分下载文件（失效重启现场）：f.bin.downloading + 旧 sidecar 名
        std::fs::write(dir.join("f.bin.downloading"), b"stale").unwrap();

        // keep_name=true：沿用 f.bin（写入 f.bin.downloading，不漂移新名）
        let mut spec = spec_for(4, &url, dir.to_str().unwrap(), 4096, 2, None);
        spec.keep_name = true;
        let (cmd_tx, mut rx, deadline) = launch_spec(spec);
        let done = wait_download_done(&mut rx, deadline).await;
        assert!(done, "keep_name 重启应完成");
        assert!(
            dir.join("f.bin").exists(),
            "keep_name 重启应写回原名目标文件"
        );
        assert!(
            !dir.join("f.bin.1").exists() && !dir.join("f.bin.1.downloading").exists(),
            "keep_name 重启不得去重改名"
        );
        drop(cmd_tx);
        std::fs::remove_dir_all(&dir).ok();

        // keep_name=false（首次启动）：残留文件存在时按规则去重
        let dir2 = crate::model::testenv::uniq_tmp_dir("ezr-e2e7");
        std::fs::write(dir2.join("f.bin.downloading"), b"stale").unwrap();
        let (_cmd_tx, mut rx, deadline) =
            launch_spec(spec_for(5, &url, dir2.to_str().unwrap(), 4096, 2, None));
        let done = wait_download_done(&mut rx, deadline).await;
        assert!(done, "首次启动应完成");
        assert!(
            dir2.join("f.bin.1").exists(),
            "首次启动遇残留文件应去重到 f.bin.1"
        );
        std::fs::remove_dir_all(&dir2).ok();
    }

    #[tokio::test]
    async fn status_503_fails_transient() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e3");
        let url = mock_server(1000, vec![("status".to_string(), "503".to_string())]);
        let (_cmd_tx, mut rx, deadline) =
            launch_spec(spec_for(3, &url, dir.to_str().unwrap(), 4096, 2, None));
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Failed { kind, reason, .. })) => {
                    assert_eq!(kind, FailKind::Transient, "503 应为可自动重试类: {reason}");
                    return;
                }
                Ok(Some(Evt::Toast(_))) => {}
                _ => {}
            }
        }
        panic!("503 应产生失败事件");
    }

    #[tokio::test]
    async fn verify_computes_matching_digest() {
        // 校验通过路径：expected = 真实摘要（模式填充内容）
        // 目录名唯一化（原与 keep_name_restart_skips_dedupe 同用 ezr-e2e4-{pid}，
        // 并行测试下先完成者的 remove_dir_all 会删除另一测试的工作目录）
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e8");
        let total: u64 = 2 * 4096;
        let url = mock_server(total, vec![]);
        // 先下载（无校验值）生成文件
        let (_cmd_tx, mut rx, deadline) =
            launch_spec(spec_for(4, &url, dir.to_str().unwrap(), 4096, 2, None));
        loop {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::DownloadDone { .. })) => break,
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                Err(_) => panic!("超时"),
                _ => {}
            }
            if std::time::Instant::now() > deadline {
                panic!("超时");
            }
        }
        // 模拟校验中状态：文件在下载完成后被 rename；重命名为 .downloading 以走 VerifySpec
        let final_path = dir.join("f.bin");
        let dl = dir.join("f.bin.downloading");
        std::fs::rename(&final_path, &dl).unwrap();
        let digest = checksum::digest_file(dl.to_str().unwrap(), 3).unwrap();
        let vspec = VerifySpec {
            id: 4,
            path: dl.to_str().unwrap().to_string(),
            final_path: final_path.to_str().unwrap().to_string(),
            sidecar_path: dir.join("f.bin.ezr").to_str().unwrap().to_string(),
            algo: "SHA-256",
            expected: digest.clone(),
        };
        let (tx2, mut rx2) = mpsc::channel::<Evt>(8);
        verify(vspec, tx2).await;
        match rx2.recv().await {
            Some(Evt::VerifyDone {
                ok: true, computed, ..
            }) => assert_eq!(computed, digest),
            other => panic!("校验应通过: {other:?}"),
        }
        assert!(final_path.exists(), "通过后应 rename 到最终名");
        assert!(!dl.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn sidecar_resume_continues_from_breakpoint() {
        // 断点接续：预置 sidecar（前 1.5 块已写）→ 探测一致 → 续传 → 完成
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e5");
        let total: u64 = 4 * 4096; // 4 块
        let url = mock_server(total, vec![]);
        // 盘上预写 .downloading（真实内容前 6000 字节）+ sidecar（块 0 完成、块 1 部分写 1904）
        let mut content = vec![0u8; total as usize];
        fill(&mut content, 0);
        std::fs::write(dir.join("f.bin.downloading"), &content).unwrap();
        let stamp = ServerStamp {
            etag: Some(format!("\"mock-{total}\"")),
            last_modified: Some("Mon, 09 Feb 2026 08:00:00 GMT".to_string()),
            size: Some(total),
        };
        let sc = Sidecar::build(
            &stamp,
            total,
            4096,
            &[4096, 1904, 0, 0],
            false,
            None,
            SidecarTask {
                id: 5,
                added_at: 0,
                save_dir: dir.to_str().unwrap().to_string(),
                concurrency: 2,
                protocol: Protocol::Http,
            },
        );
        sc.save(dir.join("f.bin.ezr").to_str().unwrap()).unwrap();
        let (_cmd_tx, mut rx, deadline) = launch_spec(sidecar_spec(5, &url, &dir, 2));
        let mut resumed_progress = None;
        loop {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Progress { downloaded, .. })) => {
                    resumed_progress.get_or_insert(downloaded);
                }
                Ok(Some(Evt::DownloadDone { total: t, .. })) => {
                    assert_eq!(t, total);
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("一致快照不应失效"),
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                Err(_) => panic!("超时"),
                _ => {}
            }
            if std::time::Instant::now() > deadline {
                panic!("超时");
            }
        }
        // 续传起点应 ≥ sidecar 已写字节（6000），证明从断点续传（未从头）
        assert!(
            resumed_progress.unwrap_or(0) >= 6000,
            "应从断点续传: {resumed_progress:?}"
        );
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn full_sidecar_no_checksum_finalizes_without_transfer() {
        // FR-01-51 v1.17③/D34 引擎链路：sidecar 全块完成 + 无期望值 → Start 后
        // 不发起任何传输，直接收尾「已完成」（rename + 删 sidecar）。用盘上
        // 特征字节证明无重传：预写内容与 mock 供数模式（fill）不同，若引擎误
        // 重传，内容会被覆盖为 fill 模式、断言失败。
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e-full-sc");
        let total: u64 = 4 * 4096;
        let url = mock_server(total, vec![]);
        std::fs::write(dir.join("f.bin.downloading"), vec![0xAB; total as usize]).unwrap();
        let stamp = ServerStamp {
            etag: Some(format!("\"mock-{total}\"")),
            last_modified: Some("Mon, 09 Feb 2026 08:00:00 GMT".to_string()),
            size: Some(total),
        };
        let sc = Sidecar::build(
            &stamp,
            total,
            4096,
            &[4096, 4096, 4096, 4096],
            false,
            None,
            SidecarTask {
                id: 6,
                added_at: 0,
                save_dir: dir.to_str().unwrap().to_string(),
                concurrency: 2,
                protocol: Protocol::Http,
            },
        );
        sc.save(dir.join("f.bin.ezr").to_str().unwrap()).unwrap();
        let (_cmd_tx, mut rx, deadline) = launch_spec(sidecar_spec(6, &url, &dir, 2));
        loop {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::DownloadDone { total: t, .. })) => {
                    assert_eq!(t, total);
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("一致快照不应失效"),
                Ok(Some(Evt::Failed { reason, .. })) => panic!("下载失败: {reason}"),
                Err(_) => panic!("超时"),
                _ => {}
            }
            if std::time::Instant::now() > deadline {
                panic!("超时");
            }
        }
        // 收尾形态：最终文件存在且内容为预写特征字节（无重传证据）、.downloading
        // 与 sidecar 均已清除
        let data = std::fs::read(dir.join("f.bin")).unwrap();
        assert_eq!(
            data,
            vec![0xAB; total as usize],
            "预写内容必须原样保留（误重传会被 fill 模式覆盖）"
        );
        assert!(!dir.join("f.bin.downloading").exists());
        assert!(!dir.join("f.bin.ezr").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn changed_etag_invalidates_sidecar() {
        // 一致性失效：sidecar ETag 与探测不符 → Invalidated
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e6");
        let total: u64 = 2 * 4096;
        let url = mock_server(total, vec![]);
        let stamp = ServerStamp {
            etag: Some("\"stale\"".to_string()),
            last_modified: None,
            size: Some(total),
        };
        let sc = Sidecar::build(
            &stamp,
            total,
            4096,
            &[4096, 0],
            false,
            None,
            SidecarTask {
                id: 6,
                added_at: 0,
                save_dir: dir.to_str().unwrap().to_string(),
                concurrency: 1,
                protocol: Protocol::Http,
            },
        );
        sc.save(dir.join("f.bin.ezr").to_str().unwrap()).unwrap();
        let (_cmd_tx, mut rx, deadline) = launch_spec(sidecar_spec(6, &url, &dir, 1));
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Invalidated { .. })) => return,
                Ok(Some(Evt::Failed { reason, .. })) => panic!("不应失败: {reason}"),
                _ => {}
            }
        }
        panic!("应回报一致性失效");
    }

    /// v1.25 缺陷回归（引擎单写者不变式；操作者实证：重试 10/5 / 进度清零 /
    /// 已失败红条增长）：同任务 id 重复 Start 时，引擎必须先取消存活旧
    /// supervisor 再孵化新 supervisor。历史缺陷：旧句柄被静默覆盖、双
    /// supervisor 并发写同一 `.downloading` 文件与 sidecar——重复 Failed
    /// 计数冲破重试上限、进度回退、已失败态进度条被幽灵事件推高。
    /// 观察点：第二个 Start 后旧 supervisor 被取消（收到 Evt::Cancelled），
    /// 替换后的新 supervisor 独自跑完下载（恰好一个 DownloadDone）。
    #[tokio::test]
    async fn engine_start_replaces_live_supervisor() {
        let total: u64 = 3 * 1024 * 1024; // 3 块 × 1 MB；slow 门控拉长在传窗口
        let url = mock_server(total, vec![("slow".into(), String::new())]);
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-replace");
        let shared = throttle_free();
        let mut tasks: std::collections::HashMap<u32, mpsc::Sender<TaskCmd>> =
            std::collections::HashMap::new();
        let (evt_tx, mut evt_rx) = mpsc::channel::<Evt>(256);
        let mut spec = spec_for(
            7,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            1,
            None,
        );
        // 钉住任务名（比照真实重试路径 make_spec 的 keep_name 口径）
        spec.keep_name = true;
        crate::engine::start_task(&mut tasks, spec.clone(), &shared, &evt_tx);
        // 首个 supervisor 越过探测进入在传窗口（slow ≈ 1.9s/块）
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        crate::engine::start_task(&mut tasks, spec, &shared, &evt_tx);
        // 旧 supervisor 必须被取消（Cancelled 事件）；deadline 内未到即失败
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut cancelled = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), evt_rx.recv()).await {
                Ok(Some(Evt::Cancelled { id: 7 })) => {
                    cancelled = true;
                    break;
                }
                Ok(Some(_)) => {}
                _ => break,
            }
        }
        assert!(
            cancelled,
            "同任务 id 重复 Start：存活旧 supervisor 必须被替换取消"
        );
        // 替换后的新 supervisor 独自跑完下载：恰好一个 DownloadDone
        let done_deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut done_count = 0usize;
        while std::time::Instant::now() < done_deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), evt_rx.recv()).await {
                Ok(Some(Evt::DownloadDone { id: 7, .. })) => done_count += 1,
                Ok(Some(Evt::Failed { reason, .. })) => panic!("替换后不应失败: {reason}"),
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(_) => {} // 静默窗：继续等待
            }
        }
        assert_eq!(
            done_count, 1,
            "替换后恰好一个 supervisor 完成（无双开双完成）"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 探测期专用服务器：首个连接延迟应答 503（慢探测失败注入），后续连接
    /// 按 Range 正常 206 服务。线程 detach（engineering.md 口径）。
    fn slow_first_fail_server(total: u64, delay_ms: u64) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            let mut first = true;
            for conn in listener.incoming().flatten() {
                let mut w = conn.try_clone().unwrap();
                let mut r = BufReader::new(conn);
                let mut line = String::new();
                if r.read_line(&mut line).is_err() || line.is_empty() {
                    continue;
                }
                let mut range_header = None;
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).is_err() || h.trim().is_empty() {
                        break;
                    }
                    if let Some((k, v)) = h.split_once(':') {
                        if k.trim().eq_ignore_ascii_case("range") {
                            range_header = Some(v.trim().to_string());
                        }
                    }
                }
                if first {
                    first = false;
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    let _ = write!(
                        w,
                        "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    continue;
                }
                let (start, end) = match range_header
                    .as_deref()
                    .and_then(|v| v.strip_prefix("bytes="))
                {
                    Some(rv) => {
                        let (a, b) = rv.split_once('-').unwrap();
                        let s = a.parse::<u64>().unwrap_or(0);
                        let e = if b.is_empty() {
                            total - 1
                        } else {
                            b.parse::<u64>().unwrap_or(total - 1).min(total - 1)
                        };
                        (s, e)
                    }
                    _ => (0, total - 1),
                };
                let len = end - start + 1;
                let _ = write!(
                    w,
                    "HTTP/1.1 206 Partial Content\r\nAccept-Ranges: bytes\r\nETag: \"mock-{total}\"\r\nLast-Modified: Mon, 09 Feb 2026 08:00:00 GMT\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
                );
                let mut buf = vec![0u8; 8192];
                let mut pos = start;
                while pos <= end {
                    let n = (end - pos + 1).min(8192) as usize;
                    fill(&mut buf[..n], pos);
                    if w.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    pos += n as u64;
                }
            }
        });
        format!("http://{addr}/f.bin")
    }

    /// v1.25 缺陷回归（探测期替换；操作者实证「重试 10/5」的引擎侧根因）：
    /// 历史 supervisor 探测阶段不消费任务命令——重复 Start 替换时，慢探测的
    /// 存活旧 supervisor 不受句柄替换影响，探测失败后补发**迟到 Failed**
    ///（App 侧重试计数被幽灵事件重复推进，冲破 5 次上限观测 10/5）；探测
    /// 成功则孵化幽灵 worker 与新 supervisor 并发写盘（进度回退/已失败态
    /// 红条增长）。修复后探测请求与命令同 select：替换即停机（Cancelled
    /// 事件），不再有迟到 Failed，新 supervisor 独自完成。
    #[tokio::test]
    async fn probe_phase_replace_never_yields_late_failed() {
        let total: u64 = 256 * 1024;
        let url = slow_first_fail_server(total, 2_000);
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-probe-replace");
        let shared = throttle_free();
        let mut tasks: std::collections::HashMap<u32, mpsc::Sender<TaskCmd>> =
            std::collections::HashMap::new();
        let (evt_tx, mut evt_rx) = mpsc::channel::<Evt>(256);
        let mut spec = spec_for(
            9,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            1,
            None,
        );
        spec.keep_name = true;
        // Start#1：旧 supervisor 进入 2s 慢探测（连接 1 排队 503 注入）
        crate::engine::start_task(&mut tasks, spec.clone(), &shared, &evt_tx);
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        // Start#2：替换——旧 supervisor 必须立即停机（探测 select 消费命令）
        crate::engine::start_task(&mut tasks, spec, &shared, &evt_tx);
        // 泵事件至新 supervisor 完成（连接 2 起正常服务）；迟到 Failed 即违例
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let (mut cancelled, mut done) = (false, false);
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), evt_rx.recv()).await {
                Ok(Some(Evt::Cancelled { id: 9 })) => cancelled = true,
                Ok(Some(Evt::DownloadDone { id: 9, .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { id: 9, reason, .. })) => {
                    panic!("替换后的存活旧 supervisor 不得补发迟到 Failed: {reason}")
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(_) => {} // 静默窗（慢探测注入期）：继续等待
            }
        }
        assert!(done, "替换后的新 supervisor 应完成下载");
        assert!(
            cancelled,
            "探测期替换必须即时取消旧 supervisor（Cancelled 事件）"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// sidecar 已完成块数：部分写与零块不计入（Failed 事件汇总口径）
    #[test]
    fn completed_of_counts_full_blocks_only() {
        let stamp = ServerStamp {
            etag: None,
            last_modified: None,
            size: Some(4 * 4096),
        };
        let sc = Sidecar::build(
            &stamp,
            4 * 4096,
            4096,
            &[4096, 4096, 1904, 0],
            false,
            None,
            SidecarTask {
                id: 1,
                added_at: 0,
                save_dir: crate::model::testenv::uniq_tmp_dir("ezr-sup-sd")
                    .to_string_lossy()
                    .into_owned(),
                concurrency: 2,
                protocol: Protocol::Http,
            },
        );
        assert_eq!(completed_of(&sc), 2, "部分写块与零块不计入完成");
    }

    // -----------------------------------------------------------------------
    // v1.26（操作者 20261014 二次报告）：单流路径三缺陷 —— 「几次连接超时后
    // 分块/字节/进度全部清零重新下载 + .ezr 控制文件中途被删」。三缺陷：
    //   ① 单流失败臂不落 sidecar（206 可续传流型的块内进度全弃，重试从零）；
    //   ② 单流入口无视恢复进度一律截断重下（单块计划 + 206 本可 Range 续传）；
    //   ③ 带断点重试的探测缺 Content-Length 被误判 Invalidated → 删 .ezr。
    // -----------------------------------------------------------------------

    /// 预置 sidecar 落盘（stamp 与 mock 服务器响应头严格一致；v1.28：sidecar
    /// 不存储 URL，stamp 三要素）
    fn seed_sidecar(dir: &std::path::Path, _url: &str, total: u64, piece: u64, written: &[u64]) {
        let stamp = ServerStamp {
            etag: Some(format!("\"mock-{total}\"")),
            last_modified: Some("Mon, 09 Feb 2026 08:00:00 GMT".to_string()),
            size: Some(total),
        };
        let sc = Sidecar::build(
            &stamp,
            total,
            piece,
            written,
            false,
            None,
            SidecarTask {
                id: 1,
                added_at: 0,
                save_dir: dir.to_string_lossy().into_owned(),
                concurrency: 4,
                protocol: Protocol::Http,
            },
        );
        sc.save(dir.join("f.bin.ezr").to_str().unwrap()).unwrap();
    }

    /// ① 单流（单块计划 + 206 可续传）半途断连：失败必须落可续传台账
    /// （旧实现失败臂 sidecar=None，块内进度全弃 → 重试从零）
    #[tokio::test]
    async fn single_stream_failure_saves_resumable_sidecar() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-fail");
        let total: u64 = 512 * 1024; // ≤ 1MB 下限 → 单块计划 → 单流路径
        let url = mock_server(total, vec![("drophalf".into(), String::new())]);
        let spec = spec_for(
            1,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            4,
            None,
        );
        let (_cmd_tx, mut rx, deadline) = launch_spec(spec);
        let mut failed: Option<u64> = None;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Failed { downloaded, .. })) => {
                    failed = Some(downloaded);
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("断连应按失败而非作废收尾"),
                _ => {}
            }
        }
        let downloaded = failed.expect("半途断连应回报 Failed");
        assert!(
            downloaded > 0 && downloaded < total,
            "失败事件应携带真实进度（{downloaded}/{total}）"
        );
        let sc = Sidecar::load(dir.join("f.bin.ezr").to_str().unwrap())
            .expect("断点台账应落盘（旧实现失败臂 sidecar=None 全弃）");
        assert_eq!(sc.downloaded, downloaded, "台账字节 = 失败事件字节");
        assert!(!sc.non_resumable, "206 流型失败台账可续传");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ② 断点续传全程：断连失败 → 台账 → 恢复 Range 续传不回退直至完成
    /// （同一 URL 内构造「首轮断连、重试恢复」；旧实现恢复即截断重下）
    #[tokio::test]
    async fn single_stream_failure_then_retry_resumes_without_restart() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-resume");
        let total: u64 = 512 * 1024;
        // droponce：首个连接（探测+复用流）半途断连，后续连接正常
        let url = mock_server(total, vec![("droponce".into(), String::new())]);
        let spec1 = spec_for(
            1,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            4,
            None,
        );
        let (_tx, mut rx, deadline) = launch_spec(spec1);
        let mut resumed_at = 0u64;
        while std::time::Instant::now() < deadline {
            if let Ok(Some(Evt::Failed { downloaded, .. })) =
                tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await
            {
                resumed_at = downloaded;
                break;
            }
        }
        assert!(
            resumed_at > 0 && resumed_at < total,
            "第一轮应半途失败（{resumed_at}/{total}）"
        );
        // 第二轮：同一 URL 重试 + 断点接续
        let spec2 = sidecar_spec(1, &url, &dir, 4);
        let (_tx2, mut rx2, deadline2) = launch_spec(spec2);
        let mut done = false;
        let mut first_progress = true;
        while std::time::Instant::now() < deadline2 {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx2.recv()).await {
                Ok(Some(Evt::Progress { downloaded, .. })) => {
                    if first_progress {
                        assert!(
                            downloaded >= resumed_at,
                            "续传首个进度不得回退（{downloaded} < {resumed_at}；旧实现截断重下）"
                        );
                        first_progress = false;
                    }
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("一致台账不应作废"),
                Ok(Some(Evt::Failed { reason, .. })) => panic!("续传不应失败: {reason}"),
                _ => {}
            }
        }
        assert!(done, "断点续传应完成");
        let data = std::fs::read(dir.join("f.bin")).expect("完成文件在位");
        assert_eq!(data.len() as u64, total);
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect, "续传后文件字节完整（不重不漏）");
        assert!(!dir.join("f.bin.ezr").exists(), "完成后台账删除");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ②-补：服务器本次未支持 Range 且台账有进度 → 暂态失败保留断点
    /// （不得静默截断重下；.downloading 与 .ezr 均保持原样）
    #[tokio::test]
    async fn resume_entry_without_range_preserves_progress() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-norange");
        let total: u64 = 2 * 1024 * 1024; // 2 块 × 1MB
        let url = mock_server(total, vec![("norange".into(), String::new())]);
        seed_sidecar(&dir, &url, total, 1024 * 1024, &[1024 * 1024, 200_000]);
        std::fs::write(dir.join("f.bin.downloading"), vec![0u8; total as usize]).unwrap();
        let spec = sidecar_spec(1, &url, &dir, 4);
        let (_tx, mut rx, deadline) = launch_spec(spec);
        let mut reason: Option<String> = None;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Failed { reason: r, .. })) => {
                    reason = Some(r);
                    break;
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    panic!("有断点时 200 探测不得静默截断重下（旧实现行为）");
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("服务端戳一致不得作废"),
                _ => {}
            }
        }
        let reason = reason.expect("应暂态失败保留断点");
        assert!(
            reason.contains("Range"),
            "失败原因应说明 Range 未支持: {reason}"
        );
        assert!(dir.join("f.bin.ezr").exists(), "台账保留");
        assert_eq!(
            std::fs::metadata(dir.join("f.bin.downloading"))
                .unwrap()
                .len(),
            total,
            "半成品文件不得被截断"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ③ 带断点重试的探测缺 Content-Length → 暂态失败保留断点
    /// （不得判 Invalidated 删 .ezr——操作者实证「.ezr 中途被删」根因）
    #[tokio::test]
    async fn resume_probe_without_content_length_keeps_sidecar() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-noclen");
        let total: u64 = 1024 * 1024;
        let url = mock_server(total, vec![("noclen".into(), String::new())]);
        seed_sidecar(&dir, &url, total, 1024 * 1024, &[400_000]);
        let spec = sidecar_spec(1, &url, &dir, 4);
        let (_tx, mut rx, deadline) = launch_spec(spec);
        let mut failed = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Failed { .. })) => {
                    failed = true;
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => {
                    panic!("缺 Content-Length 的降级探测不得作废断点（旧实现删 .ezr）");
                }
                _ => {}
            }
        }
        assert!(failed, "缺 Content-Length 探测应按暂态失败保留断点");
        assert!(dir.join("f.bin.ezr").exists(), "台账保留");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ④ v1.27 回归（操作者实证：详情「分块 0/11」而磁盘 .ezr 台账 2 块全 +
    /// 1 部分块）：带断点台账的任务在**探测阶段**网络错误（连接超时/拒连——
    /// 重试的每次尝试都先探测）时，失败事件不得把台账弃成 None 上报 0/0——
    /// 多块路径事件口径权威，App apply_chunk_snapshot 会用 0 覆盖分块视图
    /// （字节因 >0 守卫保留，详情页「已下载 2.2 MB / 分块 0/3」自相矛盾，
    /// 注册表随之持久化错误快照）。探测失败 = 未核验 ≠ 断点消失：上报台账
    /// 的持久化状态（不确变、不删账）。
    #[tokio::test]
    async fn probe_conn_error_reports_persisted_breakpoint() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-probe-conn");
        let total: u64 = 3 * 1024 * 1024; // 3 块 × 1MB
                                          // 拒连端口：bind 后立即 drop（探测请求必网络错误，无响应可用）
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        drop(l);
        let url = format!("http://{addr}/f.bin");
        seed_sidecar(
            &dir,
            &url,
            total,
            1024 * 1024,
            &[1_048_576, 1_048_576, 200_000],
        );
        let spec = sidecar_spec(1, &url, &dir, 4);
        let (_tx, mut rx, deadline) = launch_spec(spec);
        let mut got: Option<(u64, u32)> = None;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Failed {
                    downloaded,
                    chunk_done,
                    ..
                })) => {
                    got = Some((downloaded, chunk_done));
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("无响应探测不得作废断点"),
                _ => {}
            }
        }
        assert_eq!(
            got,
            Some((1_048_576 + 1_048_576 + 200_000, 2)),
            "失败事件应携带台账持久化状态（旧实现上报 0/0 → 详情分块清零）"
        );
        assert!(dir.join("f.bin.ezr").exists(), "台账保留");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ⑤ 同 ④：探测期 HTTP 状态错误（503）同口径——失败事件携带台账状态。
    #[tokio::test]
    async fn probe_http_error_reports_persisted_breakpoint() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-probe-http");
        let total: u64 = 3 * 1024 * 1024;
        let url = mock_server(total, vec![("status".into(), "503".into())]);
        seed_sidecar(
            &dir,
            &url,
            total,
            1024 * 1024,
            &[1_048_576, 1_048_576, 200_000],
        );
        let spec = sidecar_spec(1, &url, &dir, 4);
        let (_tx, mut rx, deadline) = launch_spec(spec);
        let mut got: Option<(u64, u32)> = None;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Failed {
                    downloaded,
                    chunk_done,
                    ..
                })) => {
                    got = Some((downloaded, chunk_done));
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("状态错误探测不得作废断点"),
                _ => {}
            }
        }
        assert_eq!(
            got,
            Some((1_048_576 + 1_048_576 + 200_000, 2)),
            "失败事件应携带台账持久化状态（旧实现上报 0/0）"
        );
        assert!(dir.join("f.bin.ezr").exists(), "台账保留");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ⑥ 单流续传请求臂（Range 续传请求本身网络错误）同口径：单块台账
    /// 块内部分进度（无完整块 → chunk_done=0 属实），失败事件必须携带台账
    /// 字节（旧实现 None → 「已下载」清零观感，字节与分块双失真）。
    #[tokio::test]
    async fn single_stream_resume_request_error_keeps_ledger() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-resume-err");
        let total: u64 = 512 * 1024; // 单块计划 → 单流路径
                                     // dropseq=1：连接 0 = 探测（206 全量头），连接 1 = Range 续传请求 → 直接断开
        let url = mock_server(total, vec![("dropseq".into(), "1".into())]);
        seed_sidecar(&dir, &url, total, 1024 * 1024, &[200_000]);
        let spec = sidecar_spec(1, &url, &dir, 4);
        let (_tx, mut rx, deadline) = launch_spec(spec);
        let mut got: Option<u64> = None;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Failed { downloaded, .. })) => {
                    got = Some(downloaded);
                    break;
                }
                Ok(Some(Evt::Invalidated { .. })) => panic!("一致台账不得作废"),
                _ => {}
            }
        }
        assert_eq!(
            got,
            Some(200_000),
            "续传请求失败应携带台账字节（旧实现上报 0 → 已下载清零观感）"
        );
        assert!(dir.join("f.bin.ezr").exists(), "台账保留");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ②-补2：全块完成台账（校验等待窗中断）恢复 → 直判收尾不重传
    /// （旧实现截断后阻塞在 hold 体上；台账与磁盘脱节才作废重下）
    #[tokio::test]
    async fn full_ledger_resume_finalizes_without_retransfer() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-full");
        let total: u64 = 512 * 1024;
        // hold 门控：有头无体——全块台账直判收尾不需要响应体
        let url = mock_server(total, vec![("hold".into(), String::new())]);
        seed_sidecar(&dir, &url, total, 1024 * 1024, &[total]);
        std::fs::write(dir.join("f.bin.downloading"), vec![7u8; total as usize]).unwrap();
        let spec = sidecar_spec(1, &url, &dir, 4);
        let (_tx, mut rx, deadline) = launch_spec(spec);
        let mut done = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::DownloadDone { total: t, .. })) => {
                    assert_eq!(t, total);
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("全块台账应直判收尾: {reason}"),
                _ => {}
            }
        }
        assert!(
            done,
            "全块完成台账恢复应直接收尾（旧实现截断后阻塞在 hold 体上）"
        );
        assert!(!dir.join("f.bin.ezr").exists(), "收尾后台账删除");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ①-补：206 单流暂停臂折叠块内进度（旧实现恒落全零台账 + 不可续传
    /// 标记，恢复必从头重下）；恢复 Range 接续至完成
    #[tokio::test]
    async fn single_block_pause_folds_progress_into_ledger() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-pause");
        let total: u64 = 1024 * 1024; // 1 块（慢速门控留出暂停窗）
        let url = mock_server(total, vec![("slow".into(), String::new())]);
        let spec = spec_for(
            1,
            &url,
            dir.to_str().unwrap(),
            crate::model::chunk::MIN_HTTP_BLOCK_SIZE,
            1,
            None,
        );
        let (cmd_tx, mut rx, deadline) = launch_spec(spec);
        let mut paused_at = 0u64;
        let mut pause_sent = false;
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(Evt::Progress { downloaded, .. }))
                    if !pause_sent && downloaded >= 64 * 1024 =>
                {
                    let _ = cmd_tx.send(TaskCmd::Pause).await;
                    pause_sent = true;
                }
                Ok(Some(Evt::PausedDone { downloaded, .. })) => {
                    paused_at = downloaded;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("慢速流不应失败: {reason}"),
                _ => {}
            }
        }
        assert!(
            paused_at > 0 && paused_at < total,
            "暂停应折叠真实进度（{paused_at}/{total}）"
        );
        let sc = Sidecar::load(dir.join("f.bin.ezr").to_str().unwrap()).expect("暂停台账落盘");
        assert!(!sc.non_resumable, "206 单流暂停台账可续传（旧实现恒 true）");
        assert_eq!(sc.downloaded, paused_at, "台账字节 = PausedDone 字节");
        // 恢复：同一 URL Range 接续至完成
        let spec2 = sidecar_spec(1, &url, &dir, 1);
        let (_tx2, mut rx2, deadline2) = launch_spec(spec2);
        let mut done = false;
        let mut first = true;
        while std::time::Instant::now() < deadline2 {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx2.recv()).await {
                Ok(Some(Evt::Progress { downloaded, .. })) => {
                    if first {
                        assert!(downloaded >= paused_at, "恢复进度不得回退");
                        first = false;
                    }
                }
                Ok(Some(Evt::DownloadDone { .. })) => {
                    done = true;
                    break;
                }
                Ok(Some(Evt::Failed { reason, .. })) => panic!("恢复不应失败: {reason}"),
                _ => {}
            }
        }
        assert!(done, "暂停恢复应完成");
        let data = std::fs::read(dir.join("f.bin")).expect("完成文件在位");
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect, "恢复后文件字节完整");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// v1.28/FR-01-14 ②回归（规格 01-download-engine-08）：分块 worker 的每个
    /// 数据请求均从**原始 URL** 独立发起并跟跳转——不得复用探测捕获的落点
    /// （旧口径共享 worker_url：重定向服务器仅探测命中 1 次）。4 块 × 1 MB
    /// （FR-01-104 块计划下限钳制后的真实多块形态）× 2 worker：重定向命中 =
    /// 1 次探测 + 4 次分块请求 = 5；最终内容与直连一致。
    #[tokio::test]
    async fn workers_refollow_redirects_from_original_url() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-v128-redir-multi");
        let total: u64 = 4 * 1024 * 1024; // 4 块 × 1 MB（块计划下限钳制后）
        let file_url = mock_server(total, vec![]);
        let hits = Arc::new(AtomicU32::new(0));
        let origin = redirect_server(file_url.clone(), hits.clone());
        let spec = spec_for(9, &origin, dir.to_str().unwrap(), 4096, 2, None);
        let (_cmd_tx, mut rx, deadline) = launch_spec(spec);
        pump_until_done(&mut rx, deadline).await;
        let data = std::fs::read(dir.join("f.bin")).expect("完成文件在位");
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect, "经重定向链路下载内容完整");
        assert_eq!(
            hits.load(Ordering::Relaxed),
            5,
            "重定向命中 = 探测 1 + 分块请求 4（每个数据请求都从原始 URL 独立跟跳转，v1.28）"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// v1.28/FR-01-14 ③回归（规格 01-resume-sidecar 场景 01/07 续传链路）：
    /// 单流续传请求（Range 断点接续）同样从原始 URL 独立跟跳转——旧口径
    /// 复用探测落点（重定向服务器仅 1 次命中）。单块 206 可续传：命中 =
    /// 探测 1 + 续传 Range 请求 1 = 2，文件字节完整。
    #[tokio::test]
    async fn single_stream_resume_refollows_from_original_url() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-v128-redir-resume");
        let total: u64 = 4096; // 单块（single_stream + 206 续传臂）
        let file_url = mock_server(total, vec![]);
        let hits = Arc::new(AtomicU32::new(0));
        let origin = redirect_server(file_url.clone(), hits.clone());
        // 盘上预写 .downloading（前 1904 字节真实内容）+ 一致 sidecar
        let mut content = vec![0u8; total as usize];
        fill(&mut content, 0);
        std::fs::write(dir.join("f.bin.downloading"), &content).unwrap();
        let stamp = ServerStamp {
            etag: Some(format!("\"mock-{total}\"")),
            last_modified: Some("Mon, 09 Feb 2026 08:00:00 GMT".to_string()),
            size: Some(total),
        };
        let sc = Sidecar::build(
            &stamp,
            total,
            4096,
            &[1904],
            false,
            None,
            SidecarTask {
                id: 10,
                added_at: 0,
                save_dir: dir.to_str().unwrap().to_string(),
                concurrency: 1,
                protocol: Protocol::Http,
            },
        );
        sc.save(dir.join("f.bin.ezr").to_str().unwrap()).unwrap();
        let spec = sidecar_spec(10, &origin, &dir, 1);
        let (_cmd_tx, mut rx, deadline) = launch_spec(spec);
        pump_until_done(&mut rx, deadline).await;
        let data = std::fs::read(dir.join("f.bin")).expect("完成文件在位");
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect, "续传后文件字节完整");
        assert_eq!(
            hits.load(Ordering::Relaxed),
            2,
            "重定向命中 = 探测 1 + 续传请求 1（续传同样从原始 URL 独立跟跳转，v1.28）"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
