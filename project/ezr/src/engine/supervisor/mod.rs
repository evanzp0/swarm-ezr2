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

#[derive(Clone, Debug)]
pub struct TaskSpec {
    pub id: u32,
    pub url: String,
    pub save_dir: String,
    pub name: String,
    pub concurrency: usize,
    pub block_size: u64,
    pub protocol: Protocol,
    pub expected_algo: Option<&'static str>,
    pub expected_value: Option<String>,
    pub sidecar: Option<Sidecar>,
    pub sidecar_path: Option<String>,
    pub keep_name: bool,
    pub added_at: u64,
    pub proxy_endpoint: Option<ProxyEndpoint>,
}

#[derive(Clone, Debug)]
pub struct BtSpec {
    pub id: u32,
    #[allow(dead_code)]
    pub source: String,
    pub meta: crate::bt::metainfo::TorrentMeta,
    pub save_dir: String,
    pub name: String,
    pub files: Vec<crate::model::BtFile>,
    pub trackers: Vec<String>,
    pub concurrency: usize,
    pub proxy_endpoint: Option<ProxyEndpoint>,
    pub done_pieces: Vec<u32>,
    pub seeding: bool,
    #[allow(dead_code)]
    pub max_speed: u64,
}

#[derive(Clone, Debug)]
pub struct VerifySpec {
    pub id: u32,
    pub path: String,
    pub final_path: String,
    pub sidecar_path: String,
    pub algo: &'static str,
    pub expected: String,
}

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
                    partial_blocks: state.blocks.partial_indices(),
                })
                .await;
        }
        Flow::Cancelled => {
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
                    partial_blocks,
                })
                .await;
        }
        Flow::Invalidated => {
            if let Some(p) = &spec.sidecar_path {
                let _ = Sidecar::remove(p);
            }
            let _ = evt_tx.send(Evt::Invalidated { id: spec.id }).await;
        }
    }
}

fn completed_of(sc: &Sidecar) -> u32 {
    sc.blocks
        .iter()
        .zip(0u32..)
        .filter(|(w, i)| block_range(sc.size, sc.block_size, *i).is_some_and(|(s, e)| **w >= e - s))
        .count() as u32
}

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

fn sidecar_path(save_dir: &str, name: &str) -> String {
    namegen::sidecar_path_of(save_dir, name)
}

enum Flow {
    Completed,
    Paused(PausedState),
    Cancelled,
    Failed(EngineFailure, bool, Option<(Sidecar, String)>),
    Invalidated,
}

struct PausedState {
    blocks: Blocks,
    sidecar: Sidecar,
    sidecar_path: String,
}

async fn emit(evt: &mpsc::Sender<Evt>, e: Evt) -> bool {
    evt.send(e).await.is_ok()
}

pub(crate) async fn verify(spec: VerifySpec, evt_tx: mpsc::Sender<Evt>) {
    let path = spec.path.clone();
    let algo_idx = checksum::algo_index_exact_or_default(spec.algo);
    let computed = tokio::task::spawn_blocking(move || checksum::digest_file(&path, algo_idx))
        .await
        .unwrap_or_else(|e| Err(format!("校验任务失败（{e}）")));
    match computed {
        Ok(hex) => {
            let ok = hex.eq_ignore_ascii_case(&spec.expected);
            if ok {
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

    fn fill(buf: &mut [u8], start: u64) {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = ((start as usize + i) % 251) as u8;
        }
    }

    fn mock_server(total: u64, opts: Vec<(String, String)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
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
                if opts
                    .iter()
                    .find(|(k, _)| k == "dropseq")
                    .and_then(|(_, v)| v.parse::<u32>().ok())
                    .is_some_and(|n| seq == n)
                {
                    continue;
                }
                let norange = opts.iter().any(|(k, _)| k == "norange");
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
                        std::thread::sleep(std::time::Duration::from_millis(15));
                    }
                    if (drophalf || (droponce && seq == 0)) && pos - start >= len / 2 {
                        break;
                    }
                }
            }
        });
        format!("http://{addr}/f.bin")
    }

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
            blacklist: std::sync::Mutex::new(std::collections::HashSet::new()),
        })
    }

    fn sidecar_spec(id: u32, url: &str, dir: &std::path::Path, conc: usize) -> TaskSpec {
        let mut spec = spec_for(id, url, dir.to_str().unwrap(), 4096, conc, None);
        spec.sidecar = Some(Sidecar::load(dir.join("f.bin.ezr").to_str().unwrap()).unwrap());
        spec.sidecar_path = Some(dir.join("f.bin.ezr").to_str().unwrap().to_string());
        spec
    }

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

    #[tokio::test]
    async fn reconfigure_hot_adjusts_concurrency_and_completes() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-hot");
        let total: u64 = 2 * 1024 * 1024;
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

    #[tokio::test]
    async fn progress_task_bytes_live_within_block() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-live");
        let total: u64 = 1024 * 1024;
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

    #[tokio::test]
    async fn block_plan_clamps_to_one_mb_floor() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-floor");
        let total: u64 = 2 * 1024 * 1024;
        let url = mock_server(total, vec![("slow".into(), String::new())]);
        let spec = spec_for(2, &url, dir.to_str().unwrap(), 1, 1, None);
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

    #[tokio::test]
    async fn reconfigure_scale_down_interrupts_inflight_immediately() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-scaledown");
        let total: u64 = 2 * 1024 * 1024;
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
        let total: u64 = 3 * 4096 + 1111;
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
        let path = dir.join("f.bin");
        let data = std::fs::read(&path).unwrap();
        assert_eq!(data.len() as u64, total);
        let mut expect = vec![0u8; total as usize];
        fill(&mut expect, 0);
        assert_eq!(data, expect, "文件内容应逐字节一致");
        assert!(!dir.join("f.bin.ezr").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn cancel_emits_cancelled_and_no_ghost_progress() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e-cancel");
        let total: u64 = 3 * 4096 + 1111;
        let url = mock_server(total, vec![("hold".into(), String::new())]);
        let (tx, mut rx) = mpsc::channel::<Evt>(64);
        let shared = throttle_free();
        let (cmd_tx, cmd_rx) = mpsc::channel::<TaskCmd>(4);
        let spec = spec_for(9, &url, dir.to_str().unwrap(), 4096, 4, None);
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
        let _ = probed;
        assert!(cancelled, "未收到取消确认事件（Evt::Cancelled）");
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

    #[tokio::test]
    async fn keep_name_restart_skips_dedupe() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e4");
        let total: u64 = 9000;
        let url = mock_server(total, vec![]);
        std::fs::write(dir.join("f.bin.downloading"), b"stale").unwrap();

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
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e8");
        let total: u64 = 2 * 4096;
        let url = mock_server(total, vec![]);
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
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-e2e5");
        let total: u64 = 4 * 4096;
        let url = mock_server(total, vec![]);
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

    #[tokio::test]
    async fn engine_start_replaces_live_supervisor() {
        let total: u64 = 3 * 1024 * 1024;
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
        spec.keep_name = true;
        crate::engine::start_task(&mut tasks, spec.clone(), &shared, &evt_tx);
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        crate::engine::start_task(&mut tasks, spec, &shared, &evt_tx);
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
        let done_deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut done_count = 0usize;
        while std::time::Instant::now() < done_deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), evt_rx.recv()).await {
                Ok(Some(Evt::DownloadDone { id: 7, .. })) => done_count += 1,
                Ok(Some(Evt::Failed { reason, .. })) => panic!("替换后不应失败: {reason}"),
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(_) => {}
            }
        }
        assert_eq!(
            done_count, 1,
            "替换后恰好一个 supervisor 完成（无双开双完成）"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

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
        crate::engine::start_task(&mut tasks, spec.clone(), &shared, &evt_tx);
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        crate::engine::start_task(&mut tasks, spec, &shared, &evt_tx);
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
                Err(_) => {}
            }
        }
        assert!(done, "替换后的新 supervisor 应完成下载");
        assert!(
            cancelled,
            "探测期替换必须即时取消旧 supervisor（Cancelled 事件）"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

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

    #[tokio::test]
    async fn single_stream_failure_saves_resumable_sidecar() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-fail");
        let total: u64 = 512 * 1024;
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

    #[tokio::test]
    async fn single_stream_failure_then_retry_resumes_without_restart() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-resume");
        let total: u64 = 512 * 1024;
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

    #[tokio::test]
    async fn resume_entry_without_range_preserves_progress() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-norange");
        let total: u64 = 2 * 1024 * 1024;
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

    #[tokio::test]
    async fn probe_conn_error_reports_persisted_breakpoint() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-probe-conn");
        let total: u64 = 3 * 1024 * 1024;
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

    #[tokio::test]
    async fn single_stream_resume_request_error_keeps_ledger() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-resume-err");
        let total: u64 = 512 * 1024;
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

    #[tokio::test]
    async fn full_ledger_resume_finalizes_without_retransfer() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-full");
        let total: u64 = 512 * 1024;
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

    #[tokio::test]
    async fn single_block_pause_folds_progress_into_ledger() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-ss-pause");
        let total: u64 = 1024 * 1024;
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

    #[tokio::test]
    async fn workers_refollow_redirects_from_original_url() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-v128-redir-multi");
        let total: u64 = 4 * 1024 * 1024;
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

    #[tokio::test]
    async fn single_stream_resume_refollows_from_original_url() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-v128-redir-resume");
        let total: u64 = 4096;
        let file_url = mock_server(total, vec![]);
        let hits = Arc::new(AtomicU32::new(0));
        let origin = redirect_server(file_url.clone(), hits.clone());
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
