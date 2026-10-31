use std::collections::HashMap;
use std::time::Instant;

use super::App;
use crate::engine::Cmd;
#[cfg(test)]
use crate::engine::Evt;
use crate::model::registry::Registry;
use crate::model::sidecar::Sidecar;
use crate::model::speed::{SpeedWindow, WINDOW};
use crate::model::{checksum, Checksum, FailKind, Task, TaskState};

mod bt;
mod evt;
mod tick;

fn conn_from_view(c: &crate::engine::ConnView) -> crate::model::Connection {
    crate::model::Connection {
        id: c.id,
        start: c.start,
        end: c.end,
        done: c.done,
    }
}

type ConnKey = (u32, usize);
type ConnObs = (u32, u64);

fn update_conn_stats(
    conn_prev: &mut HashMap<ConnKey, ConnObs>,
    conn_cum: &mut HashMap<ConnKey, u64>,
    conn_windows: &mut HashMap<ConnKey, SpeedWindow>,
    task_id: u32,
    conns: &[crate::engine::ConnView],
) {
    let now = Instant::now();
    for c in conns {
        let key = (task_id, c.id);
        let delta = match conn_prev.get(&key) {
            Some(&(pb, pd)) if pb == c.block => c.done.saturating_sub(pd),
            _ => c.done,
        };
        conn_prev.insert(key, (c.block, c.done));
        let cap = c.end.saturating_sub(c.start);
        let active = crate::model::conn_transferring(cap, c.done);
        if delta > 0 {
            *conn_cum.entry(key).or_insert(0) += delta;
        }
        if active {
            let w = conn_windows.entry(key).or_default();
            if w.last_push()
                .is_some_and(|t| now.duration_since(t) > WINDOW)
            {
                *w = SpeedWindow::new();
            }
            w.push(now, conn_cum.get(&key).copied().unwrap_or(0));
        }
    }
}

impl App {
    pub(super) fn resolve_checksum(
        save_dir: &str,
        name: &str,
        explicit: Option<Checksum>,
    ) -> Option<Checksum> {
        if explicit.is_some() {
            return explicit;
        }
        checksum::find_companion(save_dir, name).map(|sc| Checksum {
            algo: checksum::CHECKSUM_ALGOS[sc.algo_idx].0,
            value: sc.value,
        })
    }

    pub(super) fn make_spec(
        &self,
        t: &Task,
        proxy_endpoint: Option<crate::model::ProxyEndpoint>,
    ) -> crate::engine::TaskSpec {
        let sidecar = Sidecar::load(&t.sidecar_path());
        let sc_path = (sidecar.is_some()).then(|| t.sidecar_path());
        crate::engine::TaskSpec {
            id: t.id,
            url: t.url.clone(),
            save_dir: t.save_dir.clone(),
            name: t.name.clone(),
            concurrency: t.concurrency,
            block_size: t.block_size,
            protocol: t.protocol,
            expected_algo: t.checksum.as_ref().map(|c| c.algo),
            expected_value: t.checksum.as_ref().map(|c| c.value.clone()),
            sidecar,
            sidecar_path: sc_path,
            keep_name: t.name_locked,
            added_at: t.added_at,
            proxy_endpoint,
            progress_tick_ms: self.cfg.progress_tick_ms,
            sidecar_flush_secs: self.cfg.sidecar_flush_secs,
        }
    }

    pub(super) fn resolve_endpoint(
        &mut self,
        choice: &crate::model::ProxyChoice,
    ) -> Option<crate::model::ProxyEndpoint> {
        let (endpoint, missing) = self.cfg.resolve_proxy(choice);
        if missing {
            if let crate::model::ProxyChoice::Named(name) = choice {
                self.set_toast(format!("⚠ 引用的代理「{name}」不存在，已按直连"));
            }
        }
        endpoint
    }

    pub(super) fn disk_precheck(&mut self, idx: usize) -> bool {
        let t = &self.tasks[idx];
        let need = t.total.saturating_sub(t.downloaded);
        if need == 0 || !t.probed {
            return true;
        }
        let dir = std::path::Path::new(&t.save_dir);
        let avail = std::fs::create_dir_all(dir)
            .ok()
            .and_then(|()| fs2::available_space(dir).ok())
            .unwrap_or(u64::MAX);
        if avail < need {
            let t = &mut self.tasks[idx];
            t.state = TaskState::Failed;
            t.fail_kind = Some(FailKind::Fatal);
            t.retry_in = None;
            t.has_slot = false;
            t.error = Some("磁盘空间不足（保存目录剩余空间小于待下载量）".to_string());
            let name = t.name.clone();
            self.set_toast(format!(
                "✘ 磁盘空间不足：开始前预检失败，不自动重试，按 R 手动重试: {name}"
            ));
            return false;
        }
        true
    }

    pub(crate) fn save_registry(&mut self) {
        self.last_save = Instant::now();
        if let Some(dir) = std::path::Path::new(&self.registry_path).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let reg = Registry::from_tasks(&self.tasks);
        let _ = reg.save(&self.registry_path);
    }

    pub async fn shutdown(&mut self) {
        let downloading: Vec<u32> = self
            .tasks
            .iter()
            .filter(|t| t.state == TaskState::Downloading)
            .map(|t| t.id)
            .collect();
        for id in downloading {
            self.engine.send(Cmd::Pause { id }).await;
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        for (_, (dir, name)) in self.pending_deletes.drain() {
            Self::delete_task_files(&dir, &name);
        }
        self.save_registry();
        self.engine.send(Cmd::Shutdown).await;
    }
}

#[cfg(test)]
mod tick_tests {
    use super::*;
    use crate::model::config::Config;

    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-tick", tag)
    }

    #[tokio::test]
    async fn resolve_endpoint_missing_named_proxy_warns() {
        let mut a = make_app("np-gone");
        let ep = a.resolve_endpoint(&crate::model::ProxyChoice::Named("gone-proxy".into()));
        assert!(ep.is_none(), "失效引用按直连（None = 无自定义端点）");
        assert!(
            a.toast
                .as_deref()
                .is_some_and(|m| m.contains("不存在") && m.contains("已按直连")),
            "缺失引用提醒可见: {:?}",
            a.toast
        );
        a.shutdown().await;
    }

    #[tokio::test]
    async fn make_app_same_tag_never_restores_previous_registry() {
        let mut a = make_app("iso");
        let mut t = crate::model::sample_task();
        t.id = 1;
        t.state = TaskState::Completed;
        a.tasks.push(t);
        a.save_registry();
        a.shutdown().await;
        let dir_a = std::path::Path::new(&a.registry_path)
            .parent()
            .unwrap()
            .to_path_buf();
        let mut b = make_app("iso");
        assert!(
            b.tasks.is_empty(),
            "同 tag 两次构造不得恢复前次 registry（临时目录进程内唯一化）"
        );
        b.shutdown().await;
        let dir_b = std::path::Path::new(&b.registry_path)
            .parent()
            .unwrap()
            .to_path_buf();
        assert_ne!(dir_a, dir_b, "两次构造的临时目录必须不同");
        std::fs::remove_dir_all(&dir_a).ok();
        std::fs::remove_dir_all(&dir_b).ok();
    }

    async fn pump_until(app: &mut App, deadline: std::time::Instant, done: impl Fn(&App) -> bool) {
        while std::time::Instant::now() < deadline {
            app.tick().await;
            if done(app) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    #[tokio::test]
    async fn tick_drives_failed_state_from_bogus_url() {
        let mut app = make_app("fail");
        let url = "http://127.0.0.1:1/none.bin".to_string();
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-tick-dl");
        app.add_cli_task(url, Some(dir.to_string_lossy().into_owned()), Some(2), None);
        assert_eq!(app.tasks.len(), 1);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        pump_until(&mut app, deadline, |a| {
            a.tasks[0].state == TaskState::Failed
        })
        .await;
        assert_eq!(app.tasks[0].state, TaskState::Failed, "连接拒绝 → Failed");
        assert!(app.tasks[0].error.is_some());
        app.shutdown().await;
    }

    fn stub_server(len: u64) -> std::net::SocketAddr {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let data: Vec<u8> = (0..len as u32).map(|i| (i % 251) as u8).collect();
        std::thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                let data = data.clone();
                std::thread::spawn(move || {
                    use std::io::{BufRead, BufReader, Write};
                    let mut c = conn;
                    let mut r = BufReader::new(c.try_clone().unwrap());
                    let mut line = String::new();
                    while r.read_line(&mut line).unwrap_or(0) > 0 {
                        if line == "\r\n" {
                            break;
                        }
                        line.clear();
                    }
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        data.len()
                    );
                    let _ = c.write_all(head.as_bytes());
                    let _ = c.write_all(&data);
                    let _ = c.flush();
                });
            }
        });
        addr
    }

    async fn pump_to_completed(app: &mut App, deadline: std::time::Instant) {
        pump_until(app, deadline, |a| a.tasks[0].state == TaskState::Completed).await;
        assert_eq!(app.tasks[0].state, TaskState::Completed, "stub 下载应完成");
    }

    #[tokio::test]
    async fn tick_completes_download_from_stub_server() {
        let addr = stub_server(128_000);
        let mut app = make_app("done");
        let save = crate::model::testenv::uniq_tmp_dir("ezr-tick-save");
        app.add_cli_task(
            format!("http://{addr}/stub.bin"),
            Some(save.to_string_lossy().into_owned()),
            Some(2),
            None,
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        pump_to_completed(&mut app, deadline).await;
        let out = save.join("stub.bin");
        assert_eq!(std::fs::metadata(&out).unwrap().len(), 128_000);
        app.shutdown().await;
        std::fs::remove_dir_all(&save).ok();
    }

    fn stub_206_range_trickle(total: u64) -> std::net::SocketAddr {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for c in listener.incoming().flatten() {
                let mut c = c;
                let mut r = BufReader::new(c.try_clone().unwrap());
                let mut line = String::new();
                let mut range_start: u64 = 0;
                while r.read_line(&mut line).unwrap_or(0) > 0 {
                    if let Some(rest) = line.strip_prefix("Range: bytes=") {
                        range_start = rest
                            .split('-')
                            .next()
                            .and_then(|v| v.trim().parse().ok())
                            .unwrap_or(0);
                    }
                    if line == "\r\n" {
                        break;
                    }
                    line.clear();
                }
                let remain = total.saturating_sub(range_start);
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {remain}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n"
                );
                let _ = c.write_all(head.as_bytes());
                let _ = c.flush();
                let mut sent: u64 = 0;
                while sent < remain {
                    let n = 4096usize.min((remain - sent) as usize);
                    if c.write_all(&vec![7u8; n]).is_err() {
                        break;
                    }
                    let _ = c.flush();
                    sent += n as u64;
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                drop(c);
            }
        });
        addr
    }

    #[tokio::test]
    async fn paused_task_with_partial_blocks_still_opens_pieces() {
        let addr = stub_206_range_trickle(2_000_000);
        let mut app = make_app("pause-pieces");
        let save = crate::model::testenv::uniq_tmp_dir("ezr-tick-save-pp");
        app.add_cli_task(
            format!("http://{addr}/stub-pp.bin"),
            Some(save.to_string_lossy().into_owned()),
            Some(4),
            None,
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        pump_until(&mut app, deadline, |a| {
            a.tasks[0].state == TaskState::Downloading
        })
        .await;
        assert_eq!(
            app.tasks[0].state,
            TaskState::Downloading,
            "滴流桩应处于下载中"
        );
        app.toggle_pause();
        pump_until(&mut app, deadline, |a| {
            a.tasks[0].state == TaskState::Paused && !a.tasks[0].partial_blocks.is_empty()
        })
        .await;
        assert_eq!(app.tasks[0].state, TaskState::Paused);
        assert!(app.tasks[0].probed, "暂停不得清探测标记");
        assert!(app.tasks[0].resumable, "暂停不得改判续传能力");
        assert!(app.tasks[0].total > 0, "暂停不得清内容大小");
        app.open_pieces();
        assert!(
            app.pieces_open,
            "暂停后按 P 必须打开分块弹窗（toast={:?}）",
            app.toast
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&save).ok();
    }

    #[tokio::test]
    async fn reconfigure_shrink_prunes_stale_conn_rows() {
        let addr = stub_206_range_trickle(6_000_000);
        let mut app = make_app("shrink-conns");
        let save = crate::model::testenv::uniq_tmp_dir("ezr-tick-save-sc");
        app.add_cli_task(
            format!("http://{addr}/stub-sc.bin"),
            Some(save.to_string_lossy().into_owned()),
            Some(4),
            None,
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        pump_until(&mut app, deadline, |a| {
            a.tasks[0].state == TaskState::Downloading
        })
        .await;
        assert_eq!(app.tasks[0].state, TaskState::Downloading);
        pump_until(&mut app, deadline, |a| a.tasks[0].connections.len() >= 4).await;
        assert!(
            app.tasks[0].connections.iter().any(|c| c.id >= 3),
            "缩容前应有槽位 3/4 的连接行"
        );
        app.engine
            .send(crate::engine::Cmd::Reconfigure {
                id: app.tasks[0].id,
                concurrency: 2,
                endpoint: None,
            })
            .await;
        for _ in 0..20 {
            app.tick().await;
            tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        }
        assert!(
            app.tasks[0].connections.iter().all(|c| c.id <= 2),
            "缩容后只允许槽位 1/2 的连接行，实际 = {:?}",
            app.tasks[0]
                .connections
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>()
        );
        assert!(
            app.tasks[0].state == TaskState::Downloading,
            "缩容不打断下载"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&save).ok();
    }

    #[tokio::test]
    async fn retry_expiry_resends_start_and_counts_round() {
        let mut app = make_app("retry4a");
        app.add_cli_task(
            "http://127.0.0.1:1/none.bin".to_string(),
            None,
            Some(1),
            None,
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        pump_until(&mut app, deadline, |a| {
            a.tasks[0].state == TaskState::Failed
        })
        .await;
        let retries_first = app.tasks[0].retries;
        app.tasks[0].retry_in = Some(0.05);
        app.tasks[0].made_progress = true;
        pump_until(&mut app, deadline, |a| a.tasks[0].retries > retries_first).await;
        assert!(
            app.tasks[0].retries > retries_first,
            "退避到点后自动重试新一轮（Start 补发）"
        );
        assert_eq!(app.tasks[0].state, TaskState::Failed, "重试后再次失败");
        assert!(!app.tasks[0].made_progress, "新一轮尝试进展标记已清");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn requeued_probed_task_gets_restart_from_slot() {
        let addr = stub_server(64_000);
        let mut app = make_app("retry4b");
        let save = crate::model::testenv::uniq_tmp_dir("ezr-tick-save4b");
        app.add_cli_task(
            format!("http://{addr}/stub4b.bin"),
            Some(save.to_string_lossy().into_owned()),
            Some(2),
            None,
        );
        app.tasks[0].state = TaskState::Queued;
        app.tasks[0].has_slot = true;
        app.tasks[0].probed = true;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        pump_to_completed(&mut app, deadline).await;
        app.shutdown().await;
        std::fs::remove_dir_all(&save).ok();
    }

    #[tokio::test]
    async fn retry_resend_clears_probed_flag() {
        let mut app = make_app("retry-probed");
        app.add_cli_task(
            "http://127.0.0.1:1/none.bin".to_string(),
            None,
            Some(1),
            None,
        );
        app.tasks[0].state = TaskState::Failed;
        app.tasks[0].retry_in = Some(0.05);
        app.tasks[0].probed = true;
        app.tasks[0].has_slot = true;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        pump_until(&mut app, deadline, |a| {
            a.tasks[0].state == TaskState::Failed
                && a.tasks[0].retry_in.is_some()
                && a.tasks[0].retries >= 1
        })
        .await;
        assert!(
            !app.tasks[0].probed,
            "补发 Start 后 probed 必须清 false（探测在途窗口 4b 不得重发）"
        );
        assert_eq!(
            app.tasks[0].retries, 1,
            "一轮重试恰好计一次失败（无幽灵重复计数）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn duplicate_failed_event_on_failed_task_is_ignored() {
        let mut app = make_app("dup-failed");
        app.add_cli_task(
            "http://127.0.0.1:1/none.bin".to_string(),
            None,
            Some(1),
            None,
        );
        {
            let t = &mut app.tasks[0];
            t.state = TaskState::Failed;
            t.fail_kind = Some(crate::model::FailKind::Transient);
            t.retries = 3;
            t.retry_in = Some(8.0);
            t.error = Some("连接超时".to_string());
            t.downloaded = 5000;
            t.chunk_done = 4;
        }
        let id = app.tasks[0].id;
        app.on_evt(Evt::Failed {
            id,
            kind: crate::model::FailKind::Transient,
            reason: "幽灵迟到失败".to_string(),
            retry_after: None,
            made_progress: false,
            downloaded: 999_999,
            chunk_done: 0,
            partial_blocks: Vec::new(),
        })
        .await;
        let t = &app.tasks[0];
        assert_eq!(t.retries, 3, "重复失败不得推进计数");
        assert_eq!(t.retry_in, Some(8.0), "重复失败不得重挂倒计时");
        assert_eq!(t.downloaded, 5000, "重复失败不得改写已下载字节");
        assert_eq!(t.chunk_done, 4, "重复失败不得覆盖断点视图");
        assert_eq!(
            t.error.as_deref(),
            Some("连接超时"),
            "错误信息保持首次失败原值"
        );
        assert_eq!(t.state, TaskState::Failed, "状态保持已失败");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn push_hist_caps_length_and_toast_expires() {
        let mut app = make_app("hist-toast");
        app.push_hist(1024.0, 0.0);
        assert_eq!(app.speed_hist.last(), Some(&1), "KB/s 采样换算");
        for _ in 0..300 {
            app.push_hist(0.0, 0.0);
        }
        assert_eq!(app.speed_hist.len(), 180, "历史窗口上限 180 点");
        assert_eq!(app.up_hist.len(), 180);
        app.set_toast("测试提示");
        assert!(app.toast.is_some());
        app.toast_until = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        app.tick().await;
        assert!(app.toast.is_none(), "过期 toast 清除");
        app.shutdown().await;
    }

    #[test]
    fn conn_view_maps_progress_fields() {
        let cv = crate::engine::ConnView {
            id: 3,
            block: 1,
            start: 100,
            end: 1100,
            done: 400,
        };
        let c = conn_from_view(&cv);
        assert_eq!(c.id, 3);
        assert_eq!(c.start, 100);
        assert_eq!(c.end, 1100);
        assert_eq!(c.done, 400);
    }

    fn make_app_with_reg(tag: &str) -> (App, std::path::PathBuf) {
        let dir = crate::model::testenv::uniq_tmp_dir(&format!("ezr-tick-{tag}"));
        let reg = dir.join("registry.json").to_string_lossy().into_owned();
        (App::new(Config::default(), reg), dir.join("registry.json"))
    }

    #[tokio::test]
    async fn idle_tick_skips_registry_rewrite_until_state_drifts() {
        let (mut app, reg_path) = make_app_with_reg("idlesave");
        let mut t = crate::model::sample_task();
        t.id = 1;
        t.state = TaskState::Completed;
        t.total = 200;
        t.downloaded = 100;
        app.tasks.push(t);
        app.last_save = std::time::Instant::now() - std::time::Duration::from_secs(10);
        app.tick().await;
        let m1 = std::fs::metadata(&reg_path).unwrap().modified().unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        app.last_save = std::time::Instant::now() - std::time::Duration::from_secs(10);
        app.tick().await;
        let m2 = std::fs::metadata(&reg_path).unwrap().modified().unwrap();
        assert_eq!(m1, m2, "空闲期周期保存不得重写内容未变的注册表（零写放大）");
        app.tasks[0].downloaded = 150;
        app.last_save = std::time::Instant::now() - std::time::Duration::from_secs(10);
        app.tick().await;
        let reg = Registry::load(reg_path.to_str().unwrap()).expect("注册表应可加载");
        assert_eq!(reg.tasks[0].downloaded, 150, "内容漂移后周期保存必须落盘");
        app.shutdown().await;
        std::fs::remove_dir_all(reg_path.parent().unwrap()).ok();
    }
}

#[cfg(test)]
mod evt_tests {

    use super::*;
    use crate::engine::ConnView;
    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-evt", tag)
    }

    fn seed_task(id: u32, name: &str) -> Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t
    }

    fn conn_view(id: usize, start: u64, end: u64, done: u64) -> ConnView {
        ConnView {
            id,
            block: 0,
            start,
            end,
            done,
        }
    }

    #[tokio::test]
    async fn probed_unknown_id_is_noop() {
        let mut app = make_app("probe-ghost");
        app.on_evt(Evt::Probed {
            id: 9,
            name: "x.bin".to_string(),
            total: 10,
            resumable: true,
            etag: None,
            last_modified: None,
        })
        .await;
        assert!(app.tasks.is_empty(), "未知 id：无任务被创建或修改");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn probed_renames_with_dedupe_when_name_taken_in_same_dir() {
        let mut app = make_app("probe-dedupe");
        let mut t1 = seed_task(1, "a.bin");
        t1.save_dir = "/dl".to_string();
        let mut t2 = seed_task(2, "b.bin");
        t2.save_dir = "/dl".to_string();
        app.tasks.push(t1);
        app.tasks.push(t2);
        app.on_evt(Evt::Probed {
            id: 1,
            name: "b.bin".to_string(),
            total: 10,
            resumable: true,
            etag: None,
            last_modified: None,
        })
        .await;
        assert_eq!(app.tasks[0].name, "b.bin.1", "同名任务占用 → 追加 .1 去重");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn probed_renames_plainly_when_name_only_taken_in_other_dir() {
        let mut app = make_app("probe-otherdir");
        let mut t1 = seed_task(1, "a.bin");
        t1.save_dir = "/dl".to_string();
        let mut t2 = seed_task(2, "b.bin");
        t2.save_dir = "/other".to_string();
        app.tasks.push(t1);
        app.tasks.push(t2);
        app.on_evt(Evt::Probed {
            id: 1,
            name: "b.bin".to_string(),
            total: 10,
            resumable: true,
            etag: None,
            last_modified: None,
        })
        .await;
        assert_eq!(app.tasks[0].name, "b.bin", "不同保存目录的同名不算占用");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn probed_skips_rename_when_task_already_has_progress() {
        let mut app = make_app("probe-resumed");
        let mut t1 = seed_task(1, "a.bin");
        t1.save_dir = "/dl".to_string();
        t1.downloaded = 100;
        let mut t2 = seed_task(2, "b.bin");
        t2.save_dir = "/dl".to_string();
        app.tasks.push(t1);
        app.tasks.push(t2);
        app.on_evt(Evt::Probed {
            id: 1,
            name: "b.bin".to_string(),
            total: 10,
            resumable: true,
            etag: None,
            last_modified: None,
        })
        .await;
        assert_eq!(
            app.tasks[0].name, "a.bin",
            "已下载 > 0 时不得改名（FR-01-22 断点一致性）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn probed_fills_fields_and_starts_queued_task() {
        let mut app = make_app("probe-fields");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Queued;
        t.has_slot = true;
        app.tasks.push(t);
        app.on_evt(Evt::Probed {
            id: 1,
            name: "f.bin".to_string(),
            total: 1234,
            resumable: false,
            etag: Some("\"e1\"".to_string()),
            last_modified: Some("yesterday".to_string()),
        })
        .await;
        let t = &app.tasks[0];
        assert!(t.probed, "探测标记置位");
        assert!(t.name_locked, "首次探测置位「已定名」标记（v1.28）");
        assert_eq!(t.total, 1234);
        assert!(!t.resumable);
        assert_eq!(t.etag.as_deref(), Some("\"e1\""));
        assert_eq!(t.last_modified.as_deref(), Some("yesterday"));
        assert_eq!(
            t.state,
            TaskState::Downloading,
            "等待中 + 探测完成 → 下载中"
        );
        assert!(!t.made_progress, "新一轮尝试进展标记从零起算");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn probed_keeps_nonqueued_state_but_marks_probed() {
        let mut app = make_app("probe-paused");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Paused;
        app.tasks.push(t);
        app.on_evt(Evt::Probed {
            id: 1,
            name: "f.bin".to_string(),
            total: 5,
            resumable: true,
            etag: None,
            last_modified: None,
        })
        .await;
        let t = &app.tasks[0];
        assert!(t.probed);
        assert_eq!(t.state, TaskState::Paused, "非等待中状态不被探测改写");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn progress_ghost_cleans_speed_state_without_touching_tasks() {
        let mut app = make_app("progress-ghost");
        app.windows.insert(9, SpeedWindow::new());
        app.on_evt(Evt::Progress {
            id: 9,
            downloaded: 100,
            conns: vec![conn_view(1, 0, 100, 100)],
            chunk_done: 1,
            done_blocks: vec![0],
            partial_blocks: Vec::new(),
        })
        .await;
        assert!(
            !app.windows.contains_key(&9),
            "已删除/未知任务的幽灵进度：速度滑窗一并清除"
        );
        assert!(app.tasks.is_empty());
        app.shutdown().await;
    }

    #[tokio::test]
    async fn progress_seeds_window_and_updates_task_view() {
        let mut app = make_app("progress-seed");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        app.tasks.push(t);
        app.on_evt(Evt::Progress {
            id: 1,
            downloaded: 500,
            conns: vec![conn_view(2, 0, 1000, 500)],
            chunk_done: 3,
            done_blocks: vec![0, 1, 2],
            partial_blocks: Vec::new(),
        })
        .await;
        assert!(app.windows.contains_key(&1), "首次进度为任务建速度滑窗");
        let t = &app.tasks[0];
        assert_eq!(t.downloaded, 500);
        assert_eq!(t.chunk_done, 3);
        assert_eq!(t.done_blocks, vec![0, 1, 2], "逐块完成集合随 Progress 重铺");
        assert_eq!(t.connections.len(), 1, "连接视图按 ConnView 直传适配");
        assert_eq!(t.connections[0].id, 2);
        assert_eq!(t.connections[0].done, 500);
        assert!(t.made_progress, "downloaded > 0 → 连续性进展标记");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn progress_zero_bytes_does_not_mark_progress() {
        let mut app = make_app("progress-zero");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        app.tasks.push(t);
        app.on_evt(Evt::Progress {
            id: 1,
            downloaded: 0,
            conns: vec![],
            chunk_done: 0,
            done_blocks: vec![],
            partial_blocks: Vec::new(),
        })
        .await;
        assert!(!app.tasks[0].made_progress, "零字节进度不算进展");
        assert!(app.windows.contains_key(&1), "窗口仍然建立（观测后续速率）");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn progress_updates_done_blocks_and_invalidated_clears() {
        let mut app = make_app("progress-done-blocks");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        app.tasks.push(t);
        app.on_evt(Evt::Progress {
            id: 1,
            downloaded: 500,
            conns: vec![],
            chunk_done: 2,
            done_blocks: vec![2, 5],
            partial_blocks: Vec::new(),
        })
        .await;
        assert_eq!(
            app.tasks[0].done_blocks,
            vec![2, 5],
            "Progress 携带逐块完成集合（乱序位置）"
        );
        app.on_evt(Evt::Invalidated { id: 1 }).await;
        assert!(app.tasks[0].done_blocks.is_empty(), "失效重下清空逐块集合");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn partial_blocks_relaid_by_progress_paused_done_and_failed() {
        let mut app = make_app("partial-blocks-relay");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        app.tasks.push(t);
        app.on_evt(Evt::Progress {
            id: 1,
            downloaded: 500,
            conns: vec![],
            chunk_done: 1,
            done_blocks: vec![0],
            partial_blocks: vec![1, 3],
        })
        .await;
        assert_eq!(
            app.tasks[0].partial_blocks,
            vec![1, 3],
            "Progress 携带逐块部分集合"
        );
        app.on_evt(Evt::PausedDone {
            id: 1,
            downloaded: 500,
            chunk_done: 1,
            partial_blocks: vec![1, 3, 4],
        })
        .await;
        assert_eq!(
            app.tasks[0].partial_blocks,
            vec![1, 3, 4],
            "PausedDone 终态快照重铺"
        );
        app.tasks[0].state = TaskState::Downloading;
        app.on_evt(Evt::Failed {
            id: 1,
            kind: FailKind::Transient,
            reason: "连接被重置".to_string(),
            retry_after: None,
            made_progress: false,
            downloaded: 500,
            chunk_done: 1,
            partial_blocks: vec![1, 4],
        })
        .await;
        assert_eq!(
            app.tasks[0].partial_blocks,
            vec![1, 4],
            "Failed 台账终态快照重铺"
        );
        app.on_evt(Evt::Invalidated { id: 1 }).await;
        assert!(
            app.tasks[0].partial_blocks.is_empty(),
            "失效重下清空部分集合"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn paused_done_snapshots_breakpoint_and_clears_window() {
        let mut app = make_app("paused-snap");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.total = 1000;
        t.downloaded = 30;
        t.has_slot = true;
        t.speed = 999.0;
        app.tasks.push(t);
        app.windows.insert(1, SpeedWindow::new());
        app.on_evt(Evt::PausedDone {
            id: 1,
            downloaded: 50,
            chunk_done: 2,
            partial_blocks: Vec::new(),
        })
        .await;
        let t = &app.tasks[0];
        assert_eq!(t.downloaded, 50, "事件口径较大值生效（30 → 50）");
        assert_eq!(t.speed, 0.0);
        assert!(!t.has_slot, "暂停完成释放槽位");
        assert_eq!(
            t.chunk_done, 2,
            "断点视图快照含事件块数（snap=0 → max 取 2）"
        );
        assert_eq!(t.connections.len(), 1, "快照推导连接视图（详情页分块表）");
        assert!(!app.windows.contains_key(&1), "速度窗口随暂停清除");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn paused_done_keeps_larger_local_downloaded() {
        let mut app = make_app("paused-max");
        let mut t = seed_task(1, "f.bin");
        t.downloaded = 80;
        app.tasks.push(t);
        app.on_evt(Evt::PausedDone {
            id: 1,
            downloaded: 50,
            chunk_done: 0,
            partial_blocks: Vec::new(),
        })
        .await;
        assert_eq!(
            app.tasks[0].downloaded, 80,
            "本地已下载较大时不得回退（事件滞后兜底）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn failed_transient_schedules_auto_retry_and_keeps_slot() {
        let mut app = make_app("failed-transient");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.downloaded = 100;
        t.has_slot = true;
        app.tasks.push(t);
        app.windows.insert(1, SpeedWindow::new());
        app.on_evt(Evt::Failed {
            id: 1,
            kind: FailKind::Transient,
            reason: "连接被重置".to_string(),
            retry_after: None,
            made_progress: false,
            downloaded: 400,
            chunk_done: 1,
            partial_blocks: Vec::new(),
        })
        .await;
        let t = &app.tasks[0];
        assert_eq!(t.state, TaskState::Failed);
        assert_eq!(t.retries, 1, "无进展失败 → 计数累加（FR-01-41）");
        assert_eq!(t.retry_in, Some(8.0), "首轮退避 8s（FR-01-42）");
        assert!(t.has_slot, "待自动重试继续占槽位");
        assert_eq!(t.downloaded, 400, "失败时点进度取较大值落账");
        assert_eq!(t.error.as_deref(), Some("连接被重置"));
        assert_eq!(t.speed, 0.0);
        assert!(!app.windows.contains_key(&1), "失败清除速度窗口");
        let toast = app.toast.clone().unwrap_or_default();
        assert!(
            toast.contains("8s 后自动重试") && toast.contains("1/5"),
            "toast={toast}"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn failed_fatal_releases_slot_and_stops_auto_retry() {
        let mut app = make_app("failed-fatal");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.has_slot = true;
        app.tasks.push(t);
        app.on_evt(Evt::Failed {
            id: 1,
            kind: FailKind::Fatal,
            reason: "HTTP 403".to_string(),
            retry_after: None,
            made_progress: true,
            downloaded: 0,
            chunk_done: 0,
            partial_blocks: Vec::new(),
        })
        .await;
        let t = &app.tasks[0];
        assert_eq!(t.state, TaskState::Failed);
        assert_eq!(t.retries, 1, "有进展 → 计数重置为 1");
        assert!(t.retry_in.is_none(), "停等类别不设自动重试倒计时");
        assert!(!t.has_slot, "停等释放槽位");
        let toast = app.toast.clone().unwrap_or_default();
        assert!(
            toast.contains("不自动重试") && toast.contains("按 R 手动重试"),
            "toast={toast}"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn failed_unknown_id_is_noop() {
        let mut app = make_app("failed-ghost");
        app.on_evt(Evt::Failed {
            id: 42,
            kind: FailKind::Transient,
            reason: "ghost".to_string(),
            retry_after: None,
            made_progress: false,
            downloaded: 0,
            chunk_done: 0,
            partial_blocks: Vec::new(),
        })
        .await;
        assert!(app.tasks.is_empty());
        app.shutdown().await;
    }

    #[tokio::test]
    async fn invalidated_unknown_id_is_noop() {
        let mut app = make_app("inval-ghost");
        app.on_evt(Evt::Invalidated { id: 7 }).await;
        assert!(app.tasks.is_empty(), "未知 id 直接忽略");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn invalidated_first_streak_requeues_and_resets_session_baseline() {
        let mut app = make_app("inval-requeue");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.downloaded = 500;
        t.chunk_done = 1;
        t.has_slot = true;
        t.probed = true;
        t.speed = 42.0;
        t.connections = vec![crate::model::Connection {
            id: 1,
            start: 0,
            end: 100,
            done: 50,
        }];
        app.tasks.push(t);
        app.windows.insert(1, SpeedWindow::new());
        app.session_seen.insert(1, 500);
        app.on_evt(Evt::Invalidated { id: 1 }).await;
        let t = &app.tasks[0];
        assert_eq!(t.invalidation_streak, 1, "连击计数推进（0 → 1）");
        assert_eq!(t.state, TaskState::Queued, "从头重下 → 等待中重排");
        assert!(!t.probed, "重排后需重新探测");
        assert!(
            !t.has_slot,
            "引擎任务已结束，槽位释放供调度器重分（FR-01-22）"
        );
        assert_eq!(t.downloaded, 0);
        assert_eq!(t.chunk_done, 0);
        assert!(t.connections.is_empty());
        assert_eq!(t.speed, 0.0);
        assert!(!app.windows.contains_key(&1));
        assert_eq!(
            app.session_seen.get(&1),
            Some(&0),
            "会话累计账本基线同步归零（重下字节全属本次运行）"
        );
        let toast = app.toast.clone().unwrap_or_default();
        assert!(toast.contains("从头重新下载"), "toast={toast}");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn invalidated_third_streak_fails_and_stops_auto() {
        let mut app = make_app("inval-stop");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.invalidation_streak = 2;
        t.has_slot = true;
        t.retry_in = Some(8.0);
        app.tasks.push(t);
        app.on_evt(Evt::Invalidated { id: 1 }).await;
        let t = &app.tasks[0];
        assert_eq!(t.invalidation_streak, 3);
        assert_eq!(
            t.state,
            TaskState::Failed,
            "连续 3 次失效 → 停等失败（防循环）"
        );
        assert_eq!(t.fail_kind, Some(FailKind::Fatal));
        assert!(t.retry_in.is_none());
        assert!(!t.has_slot);
        assert_eq!(t.error.as_deref(), Some("服务器内容持续变化"));
        let toast = app.toast.clone().unwrap_or_default();
        assert!(toast.contains("连续 3 次一致性失效"), "toast={toast}");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn download_done_without_checksum_completes_and_clears() {
        let mut app = make_app("done-nocheck");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.has_slot = true;
        t.connections = vec![crate::model::Connection {
            id: 1,
            start: 0,
            end: 100,
            done: 100,
        }];
        app.tasks.push(t);
        app.windows.insert(1, SpeedWindow::new());
        app.on_evt(Evt::DownloadDone { id: 1, total: 1000 }).await;
        let t = &app.tasks[0];
        assert_eq!(t.state, TaskState::Completed, "无校验值直接完成");
        assert_eq!(t.downloaded, 1000);
        assert_eq!(t.total, 1000);
        assert!(!t.has_slot);
        assert!(t.verify_ok.is_none(), "未校验 → 结果 None");
        assert!(t.connections.is_empty(), "连接视图随完成清空");
        assert!(!app.windows.contains_key(&1));
        let toast = app.toast.clone().unwrap_or_default();
        assert!(toast.contains("无校验"), "toast={toast}");
        assert!(
            std::path::Path::new(&app.registry_path).exists(),
            "完成态即时落盘注册表"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn download_done_with_checksum_moves_to_verifying_and_keeps_slot() {
        let mut app = make_app("done-check");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.has_slot = true;
        t.checksum = Some(Checksum {
            algo: "MD5",
            value: "aa".to_string(),
        });
        app.tasks.push(t);
        app.on_evt(Evt::DownloadDone { id: 1, total: 64 }).await;
        let t = &app.tasks[0];
        assert_eq!(
            t.state,
            TaskState::Verifying,
            "有校验值 → 校验中（D12 占槽位）"
        );
        assert!(t.has_slot, "校验中不释放槽位（D12）");
        assert_eq!(t.speed, 0.0);
        let toast = app.toast.clone().unwrap_or_default();
        assert!(toast.contains("开始校验"), "toast={toast}");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn download_done_unknown_id_is_noop() {
        let mut app = make_app("done-ghost");
        app.on_evt(Evt::DownloadDone { id: 11, total: 1 }).await;
        assert!(app.tasks.is_empty());
        app.shutdown().await;
    }

    #[tokio::test]
    async fn download_done_checksum_cleared_midflight_skips_verify() {
        let mut app = make_app("done-ck-cleared");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.has_slot = true;
        t.checksum = Some(Checksum {
            algo: "SHA-256",
            value: "aa".to_string(),
        });
        app.tasks.push(t);
        app.tasks[0].checksum = None;
        app.on_evt(Evt::DownloadDone { id: 1, total: 1000 }).await;
        let t = &app.tasks[0];
        assert_eq!(
            t.state,
            TaskState::Completed,
            "最新值无校验 → 直接完成，不得进入校验"
        );
        assert!(t.verify_ok.is_none(), "清除后完成不应产生校验结果");
        let toast = app.toast.clone().unwrap_or_default();
        assert!(toast.contains("无校验"), "toast={toast}");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn download_done_checksum_set_midflight_verifies() {
        let mut app = make_app("done-ck-set");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Downloading;
        t.has_slot = true;
        app.tasks.push(t);
        app.tasks[0].checksum = Some(Checksum {
            algo: "MD5",
            value: "aa".to_string(),
        });
        app.on_evt(Evt::DownloadDone { id: 1, total: 64 }).await;
        let t = &app.tasks[0];
        assert_eq!(
            t.state,
            TaskState::Verifying,
            "最新值有校验 → 应进入校验（D19 完成校验用最新值）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn verify_done_ok_completes_and_clears_error() {
        let mut app = make_app("verify-ok");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Verifying;
        t.has_slot = true;
        t.error = Some("历史错误残留".to_string());
        t.checksum = Some(Checksum {
            algo: "MD5",
            value: "aa".to_string(),
        });
        t.connections = vec![crate::model::Connection {
            id: 1,
            start: 0,
            end: 10,
            done: 10,
        }];
        app.tasks.push(t);
        app.on_evt(Evt::VerifyDone {
            id: 1,
            ok: true,
            computed: "aa".to_string(),
            expected: "aa".to_string(),
        })
        .await;
        let t = &app.tasks[0];
        assert_eq!(t.state, TaskState::Completed);
        assert_eq!(t.verify_ok, Some(true));
        assert!(t.error.is_none(), "校验通过清除错误行");
        assert!(!t.has_slot);
        assert!(t.connections.is_empty());
        let toast = app.toast.clone().unwrap_or_default();
        assert!(toast.contains("MD5 校验通过"), "toast={toast}");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn verify_done_mismatch_fails_without_auto_retry() {
        let mut app = make_app("verify-bad");
        let mut t = seed_task(1, "f.bin");
        t.state = TaskState::Verifying;
        t.has_slot = true;
        t.checksum = Some(Checksum {
            algo: "MD5",
            value: "aa".to_string(),
        });
        app.tasks.push(t);
        app.on_evt(Evt::VerifyDone {
            id: 1,
            ok: false,
            computed: "bb".to_string(),
            expected: "aa".to_string(),
        })
        .await;
        let t = &app.tasks[0];
        assert_eq!(t.state, TaskState::Failed);
        assert_eq!(t.fail_kind, Some(FailKind::Verify));
        assert_eq!(t.verify_ok, Some(false));
        assert!(t.retry_in.is_none(), "校验失败不自动重试（FR-01-51）");
        assert!(!t.has_slot, "校验失败释放槽位");
        assert_eq!(t.error.as_deref(), Some("MD5 校验失败：内容与校验值不符"));
        let toast = app.toast.clone().unwrap_or_default();
        assert!(
            toast.contains("computed=bb") && toast.contains("expected=aa"),
            "toast 带实测/期望摘要：{toast}"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn cancelled_executes_pending_delete_then_unknown_id_ignored() {
        let mut app = make_app("cancelled-del");
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-evt-files");
        let base = dir.to_string_lossy().trim_end_matches('/').to_string();
        for f in ["f.bin", "f.bin.downloading", "f.bin.ezr"] {
            std::fs::write(format!("{base}/{f}"), b"x").unwrap();
        }
        app.pending_deletes
            .insert(5, (base.clone(), "f.bin".to_string()));
        app.on_evt(Evt::Cancelled { id: 99 }).await;
        assert!(
            std::path::Path::new(&format!("{base}/f.bin")).exists(),
            "未知 id 的停止确认不触发删除"
        );
        assert!(app.pending_deletes.contains_key(&5));
        app.on_evt(Evt::Cancelled { id: 5 }).await;
        for f in ["f.bin", "f.bin.downloading", "f.bin.ezr"] {
            assert!(
                !std::path::Path::new(&format!("{base}/{f}")).exists(),
                "{f} 应被删除"
            );
        }
        assert!(app.pending_deletes.is_empty(), "已执行的登记应清除");
        std::fs::remove_dir_all(&dir).ok();
        app.shutdown().await;
    }

    #[tokio::test]
    async fn toast_sets_message_with_fresh_expiry() {
        let mut app = make_app("toast");
        app.on_evt(Evt::Toast("引擎提示".to_string())).await;
        assert_eq!(app.toast.as_deref(), Some("引擎提示"));
        assert!(app.toast_until.is_some(), "toast 附带过期时刻");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn disk_precheck_skips_passes_and_fails_overneed() {
        let dir = crate::model::testenv::uniq_tmp_dir("ezr-evt-disk");
        let mut app = make_app("disk");

        let mut t = seed_task(1, "d1.bin");
        t.probed = false;
        t.total = u64::MAX;
        t.save_dir = dir.to_string_lossy().into_owned();
        app.tasks.push(t);
        app.selected = 0;
        assert!(app.disk_precheck(0), "未探测时跳过预检");
        assert_eq!(app.tasks[0].state, crate::model::TaskState::Queued);

        app.tasks[0].probed = true;
        app.tasks[0].total = 0;
        assert!(app.disk_precheck(0), "无待下载量时通过");
        assert_eq!(app.tasks[0].state, crate::model::TaskState::Queued);

        app.tasks[0].total = 1024;
        assert!(app.disk_precheck(0), "小需求通过预检");

        app.tasks[0].total = u64::MAX;
        assert!(!app.disk_precheck(0), "超量需求预检失败");
        assert_eq!(app.tasks[0].state, crate::model::TaskState::Failed);
        assert_eq!(
            app.tasks[0].fail_kind,
            Some(crate::model::FailKind::Fatal),
            "预检失败为 Fatal 不自动重试"
        );
        assert!(app.tasks[0].retry_in.is_none());
        assert!(!app.tasks[0].has_slot);
        assert!(
            app.tasks[0]
                .error
                .as_deref()
                .is_some_and(|m| m.contains("磁盘空间不足")),
            "错误信息: {:?}",
            app.tasks[0].error
        );
        assert!(
            app.toast
                .as_deref()
                .is_some_and(|m| m.contains("磁盘空间不足")),
            "toast 提示"
        );
        app.shutdown().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn progress_on_non_downloading_task_is_ignored() {
        let mut app = make_app("progress-paused");
        app.tasks.push(seed_task(1, "f.bin"));
        app.tasks[0].state = TaskState::Paused;
        app.on_evt(Evt::Progress {
            id: 1,
            downloaded: 500,
            conns: vec![],
            chunk_done: 1,
            done_blocks: vec![0],
            partial_blocks: Vec::new(),
        })
        .await;
        assert_eq!(app.tasks[0].downloaded, 0, "非下载中：进度不被采纳");
        assert_eq!(app.tasks[0].state, TaskState::Paused);
        assert!(!app.windows.contains_key(&1), "不产生幽灵速度窗");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn failure_at_retry_limit_stops_auto_retry() {
        let mut app = make_app("retry-limit");
        app.tasks.push(seed_task(1, "f.bin"));
        app.tasks[0].state = TaskState::Downloading;
        app.tasks[0].retries = app.tasks[0].max_retries - 1;
        app.on_evt(Evt::Failed {
            id: 1,
            kind: crate::model::FailKind::Transient,
            reason: "连接被重置".to_string(),
            retry_after: None,
            made_progress: false,
            downloaded: 100,
            chunk_done: 1,
            partial_blocks: Vec::new(),
        })
        .await;
        assert_eq!(app.tasks[0].state, TaskState::Failed);
        assert_eq!(app.tasks[0].retries, app.tasks[0].max_retries);
        assert!(
            app.tasks[0].retry_in.is_none(),
            "达上限：不再挂自动重试倒计时"
        );
        assert!(!app.tasks[0].has_slot, "达上限：释放槽位");
        let toast = app.toast.as_deref().unwrap_or_default();
        assert!(
            toast.contains("已达最大重试次数") && toast.contains("按 R 手动重试"),
            "toast: {toast}"
        );
        app.shutdown().await;
    }
}

#[cfg(test)]
mod conn_stats_tests {

    use super::*;
    use crate::engine::ConnView;
    fn make_app(tag: &str) -> App {
        crate::app::testutil::make_app_in("ezr-cc", tag)
    }

    fn seed(id: u32, name: &str, state: TaskState) -> Task {
        let mut t = crate::model::sample_task();
        t.id = id;
        t.name = name.to_string();
        t.state = state;
        t
    }

    fn cv(id: usize, block: u32, start: u64, end: u64, done: u64) -> ConnView {
        ConnView {
            id,
            block,
            start,
            end,
            done,
        }
    }

    async fn progress(app: &mut App, id: u32, conns: Vec<ConnView>) {
        let downloaded = conns.iter().map(|c| c.done).sum();
        app.on_evt(Evt::Progress {
            id,
            downloaded,
            conns,
            chunk_done: 0,
            done_blocks: Vec::new(),
            partial_blocks: Vec::new(),
        })
        .await;
    }

    #[tokio::test]
    async fn conn_speed_and_cum_accumulate_within_block() {
        let mut app = make_app("acc");
        app.tasks.push(seed(1, "a.bin", TaskState::Downloading));
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 100)]).await;
        assert_eq!(app.conn_cum_of(1, 1), 100, "首次观测：新块内已写计入累计");
        std::thread::sleep(std::time::Duration::from_millis(20));
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 400)]).await;
        assert_eq!(app.conn_cum_of(1, 1), 400, "同块增量 = done 差值");
        app.sample_conn_speed_display();
        assert!(
            app.conn_speed_of(1, 1) > 0.0,
            "同块有增量 → 窗口速率经 EMA 非零"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn conn_block_change_counts_new_block_done() {
        let mut app = make_app("reblock");
        app.tasks.push(seed(1, "b.bin", TaskState::Downloading));
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 1000)]).await;
        assert_eq!(app.conn_cum_of(1, 1), 1000);
        progress(&mut app, 1, vec![cv(1, 1, 1000, 2000, 200)]).await;
        assert_eq!(app.conn_cum_of(1, 1), 1200, "换块增量 = 新块内已写");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn idle_conn_speed_snaps_to_zero() {
        let mut app = make_app("idle");
        app.tasks.push(seed(1, "c.bin", TaskState::Downloading));
        progress(&mut app, 1, vec![cv(2, 0, 0, 0, 0), cv(3, 0, 0, 500, 500)]).await;
        assert_eq!(app.conn_speed_of(1, 2), 0.0, "空连接待命 → 0");
        assert_eq!(app.conn_speed_of(1, 3), 0.0, "块满待命 → 0");
        std::thread::sleep(std::time::Duration::from_millis(20));
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 100)]).await;
        std::thread::sleep(std::time::Duration::from_millis(20));
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 300)]).await;
        app.sample_conn_speed_display();
        assert!(app.conn_speed_of(1, 1) > 0.0, "在传连接速度立起");
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 1000)]).await;
        app.sample_conn_speed_display();
        assert_eq!(app.conn_speed_of(1, 1), 0.0, "在传 → 待命：精确归零无拖尾");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn streaming_conn_speed_rises_in_display_sampling() {
        let mut app = make_app("stream");
        app.tasks.push(seed(1, "s.bin", TaskState::Downloading));
        progress(&mut app, 1, vec![cv(1, 0, 0, 0, 100)]).await;
        std::thread::sleep(std::time::Duration::from_millis(20));
        progress(&mut app, 1, vec![cv(1, 0, 0, 0, 400)]).await;
        assert_eq!(app.conn_cum_of(1, 1), 400, "流式累计随 done 增量累加");
        app.sample_conn_speed_display();
        assert!(
            app.conn_speed_of(1, 1) > 0.0,
            "流式单连接展示速度非零（旧实现恒 0）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn conn_speed_zeroed_when_task_not_downloading() {
        let mut app = make_app("nondl");
        app.tasks.push(seed(1, "x.bin", TaskState::Downloading));
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 100)]).await;
        std::thread::sleep(std::time::Duration::from_millis(20));
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 400)]).await;
        app.sample_conn_speed_display();
        assert!(app.conn_speed_of(1, 1) > 0.0, "前置：速度已立起");
        app.tasks[0].state = TaskState::Paused;
        app.sample_conn_speed_display();
        assert_eq!(
            app.conn_speed_of(1, 1),
            0.0,
            "任务非下载态 → 连接速度零值速断"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn conn_speed_sampling_gated_to_one_second() {
        let mut app = make_app("gate");
        app.tasks.push(seed(1, "g.bin", TaskState::Downloading));
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 100)]).await;
        std::thread::sleep(std::time::Duration::from_millis(30));
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 900)]).await;
        app.sample_conn_speed_display();
        let base = app.conn_speed_of(1, 1);
        std::thread::sleep(std::time::Duration::from_millis(30));
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 1700)]).await;
        app.tick().await;
        assert_eq!(
            app.conn_speed_of(1, 1),
            base,
            "1s 节拍内 tick 不重采样（每秒最多变化一次）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn http_task_speed_equals_sum_of_conn_rows() {
        let mut app = make_app("sum");
        app.tasks.push(seed(1, "sum.bin", TaskState::Downloading));
        progress(
            &mut app,
            1,
            vec![cv(1, 0, 0, 1000, 100), cv(2, 0, 0, 1000, 300)],
        )
        .await;
        std::thread::sleep(std::time::Duration::from_millis(20));
        progress(
            &mut app,
            1,
            vec![cv(1, 0, 0, 1000, 400), cv(2, 0, 0, 1000, 900)],
        )
        .await;
        app.sample_conn_speed_display();
        let r1 = app.conn_speed_of(1, 1);
        let r2 = app.conn_speed_of(1, 2);
        assert!(r1 > 0.0 && r2 > 0.0, "前置：两行速度立起");
        let t = app.tasks.iter().find(|t| t.id == 1).unwrap();
        assert_eq!(
            t.speed,
            r1 + r2,
            "任务行速度 = Σ 并发行速度（FR-01-102 v1.39/D63③，同一采样源）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn http_single_conn_task_speed_equals_row() {
        let mut app = make_app("single");
        app.tasks.push(seed(1, "one.bin", TaskState::Downloading));
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 100)]).await;
        std::thread::sleep(std::time::Duration::from_millis(20));
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 500)]).await;
        app.sample_conn_speed_display();
        let r1 = app.conn_speed_of(1, 1);
        assert!(r1 > 0.0);
        let t = app.tasks.iter().find(|t| t.id == 1).unwrap();
        assert_eq!(t.speed, r1, "单连接任务速度 == 该行速度（构造上相等）");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn http_task_speed_zero_when_no_active_rows() {
        let mut app = make_app("norate");
        app.tasks.push(seed(1, "zero.bin", TaskState::Downloading));
        progress(&mut app, 1, vec![cv(2, 0, 0, 0, 0)]).await;
        app.sample_conn_speed_display();
        let t = app.tasks.iter().find(|t| t.id == 1).unwrap();
        assert_eq!(
            t.speed, 0.0,
            "无活跃并发行 → 任务速度 0（与行合计一致，不再来自任务级窗口）"
        );
        app.shutdown().await;
    }

    #[tokio::test]
    async fn conn_cum_resets_on_new_download_run() {
        let mut app = make_app("newrun");
        app.tasks.push(seed(1, "n.bin", TaskState::Downloading));
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 500)]).await;
        assert_eq!(app.conn_cum_of(1, 1), 500);
        app.tasks[0].state = TaskState::Queued;
        app.on_evt(Evt::Probed {
            id: 1,
            name: "n.bin".to_string(),
            total: 4000,
            resumable: true,
            etag: None,
            last_modified: None,
        })
        .await;
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 120)]).await;
        assert_eq!(app.conn_cum_of(1, 1), 120, "新一次下载从零重计");
        app.shutdown().await;
    }

    #[tokio::test]
    async fn conn_cum_reset_on_requeue_resume_probed() {
        let mut app = make_app("keep");
        app.tasks.push(seed(1, "k.bin", TaskState::Downloading));
        progress(&mut app, 1, vec![cv(1, 0, 0, 1000, 500)]).await;
        assert_eq!(app.conn_cum_of(1, 1), 500);
        app.tasks[0].state = TaskState::Paused;
        app.tasks[0].resumable = true;
        app.selected = 0;
        app.toggle_pause();
        assert_eq!(app.tasks[0].state, TaskState::Queued, "恢复 = 重新排队");
        app.on_evt(Evt::Probed {
            id: 1,
            name: "k.bin".to_string(),
            total: 4000,
            resumable: true,
            etag: None,
            last_modified: None,
        })
        .await;
        assert_eq!(app.tasks[0].state, TaskState::Downloading, "Probed 转下载");
        progress(&mut app, 1, vec![cv(1, 1, 1000, 2000, 250)]).await;
        assert_eq!(app.conn_cum_of(1, 1), 250, "新一次下载从零重计");
        app.shutdown().await;
    }
}
