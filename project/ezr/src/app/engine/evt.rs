use std::time::Instant;

use super::{conn_from_view, update_conn_stats};
use crate::app::App;
use crate::engine::{Cmd, Evt};
use crate::model::{namegen, FailKind, TaskState};

impl App {
    pub(super) async fn on_evt(&mut self, evt: Evt) {
        match evt {
            Evt::Probed {
                id,
                name,
                total,
                resumable,
                etag,
                last_modified,
            } => {
                let rename_to = {
                    let cur = self.tasks.iter().find(|t| t.id == id);
                    match cur {
                        Some(t0) if t0.name != name && t0.downloaded == 0 => {
                            let taken = |n: &str| {
                                self.tasks
                                    .iter()
                                    .any(|x| x.id != id && x.name == n && x.save_dir == t0.save_dir)
                            };
                            Some(namegen::dedupe(&name, taken))
                        }
                        _ => None,
                    }
                };
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                    if let Some(nn) = rename_to {
                        t.name = nn;
                    }
                    t.name_locked = true;
                    t.resume_orphan = false;
                    t.total = total;
                    t.resumable = resumable;
                    t.probed = true;
                    t.etag = etag;
                    t.last_modified = last_modified;
                    if t.state == TaskState::Queued {
                        t.state = TaskState::Downloading;
                        t.made_progress = false;
                        self.clear_conn_stats(id);
                    }
                }
            }
            Evt::Progress {
                id,
                downloaded,
                conns,
                chunk_done,
                done_blocks,
                partial_blocks,
            } => {
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
                    return;
                };
                if t.state != TaskState::Downloading {
                    return;
                }
                t.downloaded = downloaded;
                t.chunk_done = chunk_done;
                t.done_blocks = done_blocks;
                t.partial_blocks = partial_blocks;
                t.connections = conns.iter().map(conn_from_view).collect();
                if self.conn_rebase.remove(&id) {
                    for c in &conns {
                        self.conn_prev.insert((id, c.id), (c.block, c.done));
                    }
                }
                update_conn_stats(
                    &mut self.conn_prev,
                    &mut self.conn_cum,
                    &mut self.conn_windows,
                    id,
                    &conns,
                );
                if downloaded > 0 {
                    t.made_progress = true;
                }
            }
            Evt::PausedDone {
                id,
                downloaded,
                chunk_done,
                partial_blocks,
            } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                    t.downloaded = downloaded.max(t.downloaded);
                    t.speed = 0.0;
                    t.has_slot = false;
                    t.partial_blocks = partial_blocks;
                    t.apply_chunk_snapshot(downloaded, chunk_done);
                }
            }
            Evt::Failed {
                id,
                kind,
                reason,
                retry_after,
                made_progress,
                downloaded,
                chunk_done,
                partial_blocks,
            } => {
                self.handle_failure(
                    id,
                    kind,
                    reason,
                    retry_after,
                    made_progress,
                    downloaded,
                    chunk_done,
                    partial_blocks,
                );
            }
            Evt::Invalidated { id } => {
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
                    return;
                };
                let (streak, stop_wait) =
                    crate::model::retry::invalidate_streak(t.invalidation_streak);
                t.invalidation_streak = streak;
                let name = t.name.clone();
                t.downloaded = 0;
                t.chunk_done = 0;
                t.done_blocks.clear();
                t.partial_blocks.clear();
                t.connections.clear();
                t.speed = 0.0;
                self.session_seen.insert(id, 0);
                if stop_wait {
                    t.state = TaskState::Failed;
                    t.fail_kind = Some(FailKind::Fatal);
                    t.retry_in = None;
                    t.has_slot = false;
                    t.error = Some("服务器内容持续变化".to_string());
                    self.set_toast(format!(
                        "✘ 连续 3 次一致性失效，停止自动重试（按 R 手动重试）: {name}"
                    ));
                } else {
                    t.state = TaskState::Queued;
                    t.probed = false;
                    t.has_slot = false;
                    self.set_toast(format!(
                        "⚠ 服务器内容已更新，断点已作废，从头重新下载: {name}"
                    ));
                }
                self.clear_conn_stats(id);
            }
            Evt::OrphanRedirect { id } => {
                let new_name = {
                    let Some(cur) = self.tasks.iter().find(|t| t.id == id) else {
                        return;
                    };
                    let dir = cur.save_dir.clone();
                    let base = cur.name.clone();
                    let taken = |n: &str| {
                        self.tasks.iter().any(|x| x.name == n && x.save_dir == dir)
                            || namegen::exists_on_disk(&dir, n)
                    };
                    namegen::dedupe(&base, taken)
                };
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
                    return;
                };
                t.name = new_name;
                t.resume_orphan = false;
                t.downloaded = 0;
                t.chunk_done = 0;
                t.done_blocks.clear();
                t.partial_blocks.clear();
                t.connections.clear();
                t.speed = 0.0;
                t.state = TaskState::Queued;
                t.probed = false;
                t.has_slot = false;
                self.session_seen.insert(id, 0);
                self.clear_conn_stats(id);
            }
            Evt::DownloadDone { id, total } => {
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
                    return;
                };
                t.downloaded = total;
                t.total = total;
                t.speed = 0.0;
                let name = t.name.clone();
                let verify_now = t.checksum.is_some();
                if verify_now {
                    let dl = t.downloading_path();
                    let fin = t.target_path();
                    let path = if std::path::Path::new(&fin).exists() {
                        fin
                    } else {
                        dl
                    };
                    let algo = t.checksum.as_ref().map_or("SHA-256", |c| c.algo);
                    t.state = TaskState::Verifying;
                    t.has_slot = true;
                    let expected = t
                        .checksum
                        .as_ref()
                        .map_or(String::new(), |c| c.value.clone());
                    let spec = crate::engine::VerifySpec {
                        id,
                        path,
                        final_path: t.target_path(),
                        sidecar_path: t.sidecar_path(),
                        algo,
                        expected,
                    };
                    self.engine.send(Cmd::Verify { spec }).await;
                    self.set_toast(format!("✓ 下载完成，开始校验: {name}"));
                } else {
                    let dl = t.downloading_path();
                    let fin = t.target_path();
                    if std::path::Path::new(&dl).exists() {
                        let _ = tokio::fs::rename(&dl, &fin).await;
                    }
                    let _ = tokio::fs::remove_file(t.sidecar_path()).await;
                    t.state = TaskState::Completed;
                    t.has_slot = false;
                    t.verify_ok = None;
                    t.connections.clear();
                    self.clear_conn_stats(id);
                    self.set_toast(format!("✓ 下载完成: {name}（无校验）"));
                }
                self.save_registry();
            }
            Evt::VerifyDone {
                id,
                ok,
                computed,
                expected,
            } => {
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
                    return;
                };
                let algo = t.checksum.as_ref().map_or("SHA-256", |c| c.algo);
                let name = t.name.clone();
                t.speed = 0.0;
                if ok {
                    t.verify_ok = Some(true);
                    t.state = TaskState::Completed;
                    t.has_slot = false;
                    t.error = None;
                    t.connections.clear();
                    self.clear_conn_stats(id);
                    self.set_toast(format!("✓ {algo} 校验通过: {name}"));
                } else {
                    t.verify_ok = Some(false);
                    t.state = TaskState::Failed;
                    t.fail_kind = Some(FailKind::Verify);
                    t.retry_in = None;
                    t.has_slot = false;
                    t.error = Some(format!("{algo} 校验失败：内容与校验值不符"));
                    self.set_toast(format!(
                        "✘ {algo} 校验失败：不自动重试，按 R 重新校验（computed={computed} expected={expected}）: {name}"
                    ));
                }
                self.save_registry();
            }
            Evt::Cancelled { id } => {
                if let Some((dir, name)) = self.pending_deletes.remove(&id) {
                    Self::delete_task_files(&dir, &name);
                }
            }
            Evt::Toast(msg) => self.set_toast(msg),
            Evt::BtPeers {
                id,
                complete,
                incomplete,
            } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                    t.tracker_seeds = Some(complete as usize);
                    t.tracker_peers = Some(incomplete as usize);
                }
            }
            Evt::BtConns {
                id,
                conns,
                availability,
            } => {
                // 种子健康度（FR-02-37）：与并发明细同帧刷新，仅活跃态任务接收。
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                    t.availability = availability;
                    t.seeders = conns.iter().filter(|c| c.seeder).count();
                    t.peers = conns.iter().filter(|c| !c.seeder).count();
                }
                let active = self
                    .tasks
                    .iter()
                    .any(|t| t.id == id && t.state.shows_conns());
                if active {
                    let ids: std::collections::HashSet<usize> =
                        conns.iter().map(|c| c.id).collect();
                    self.bt_conn_stats
                        .retain(|(tid, cid), _| *tid != id || ids.contains(cid));
                    let now = Instant::now();
                    for c in &conns {
                        let st = self.bt_conn_stats.entry((id, c.id)).or_default();
                        // 窗口基线超过 WINDOW 后仅由 SpeedWindow::push 自行重置滑窗，
                        // 绝不能在这里清零 EMA：引擎 1s tick 与 WINDOW=1s 贴边，
                        // 偶发超时若清零 EMA 会造成速度列 '-' 一跳一跳。
                        st.down_win.push(now, c.cum_down);
                        st.up_win.push(now, c.cum_up);
                    }
                    self.bt_conns.insert(
                        id,
                        conns
                            .into_iter()
                            .map(|c| crate::app::BtConnRow {
                                id: c.id,
                                addr: c.addr,
                                protocol: c.protocol,
                                down_speed: c.down_speed,
                                cum_down: c.cum_down,
                                up_speed: c.up_speed,
                                cum_up: c.cum_up,
                                client: c.client,
                                seeder: c.seeder,
                                flags: c.flags,
                                relevance: c.relevance,
                            })
                            .collect(),
                    );
                } else {
                    self.bt_conns.remove(&id);
                    self.clear_bt_conn_stats(id);
                }
            }
            Evt::BtProgress {
                id,
                downloaded,
                chunk_done,
                done_blocks,
                partial_blocks,
                active_blocks,
                uploaded,
            } => {
                self.on_bt_progress(
                    id,
                    downloaded,
                    chunk_done,
                    done_blocks,
                    partial_blocks,
                    active_blocks,
                    uploaded,
                );
            }
            Evt::BtDone { id, total } => {
                self.on_bt_done(id, total);
            }
            Evt::BtTrackers {
                id,
                url,
                working,
                peers,
                seeds,
                next_in_secs,
            } => {
                let live = self
                    .tasks
                    .iter()
                    .any(|t| t.id == id && t.state.shows_conns());
                if live {
                    let rows = self.trackers_status.entry(id).or_default();
                    let now = Instant::now();
                    let next_at = now + std::time::Duration::from_secs(next_in_secs);
                    match rows.iter_mut().find(|r| r.url == url) {
                        Some(r) => {
                            r.working = working;
                            r.peers = peers;
                            r.seeds = seeds;
                            r.next_at = Some(next_at);
                        }
                        None => rows.push(crate::app::TrackerRow {
                            url,
                            working,
                            peers,
                            seeds,
                            next_at: Some(next_at),
                        }),
                    }
                }
            }
            Evt::BtStopped {
                id,
                downloaded,
                done_pieces,
                partial_pieces,
            } => {
                self.on_bt_stopped(id, downloaded, done_pieces, partial_pieces);
            }
            Evt::BtFailed { id, kind, reason } => {
                self.on_bt_failed(id, kind, reason);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_failure(
        &mut self,
        id: u32,
        kind: FailKind,
        reason: String,
        retry_after: Option<f64>,
        made_progress: bool,
        downloaded: u64,
        chunk_done: u32,
        partial_blocks: Vec<u32>,
    ) {
        let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
            return;
        };
        if matches!(t.state, TaskState::Paused | TaskState::Failed) {
            return;
        }
        if downloaded > 0 {
            t.downloaded = downloaded.max(t.downloaded);
        }
        t.speed = 0.0;
        t.partial_blocks = partial_blocks;
        let (auto_retry, max_retries) = (self.cfg.auto_retry, t.max_retries);
        let (new_retries, decision) = crate::model::retry::decide(
            kind,
            made_progress,
            t.retries,
            max_retries,
            retry_after,
            auto_retry,
        );
        t.retries = new_retries;
        t.made_progress = false;
        t.state = TaskState::Failed;
        t.fail_kind = Some(kind);
        t.error = Some(reason.clone());
        let name = t.name.clone();
        let retries = t.retries;
        let toast = match decision {
            crate::model::retry::RetryDecision {
                auto: true,
                delay_secs,
                ..
            } => {
                t.retry_in = Some(delay_secs);
                t.has_slot = true;
                format!(
                    "✘ 下载失败（{reason}），{delay_secs:.0}s 后自动重试（{retries}/{max_retries}）: {name}"
                )
            }
            crate::model::retry::RetryDecision { at_limit: true, .. } => {
                t.retry_in = None;
                t.has_slot = false;
                format!(
                    "✘ 已达最大重试次数（{retries}/{max_retries}），停止自动重试，按 R 手动重试: {name}"
                )
            }
            _ => {
                t.retry_in = None;
                t.has_slot = false;
                format!("✘ {reason}：不自动重试，按 R 手动重试: {name}")
            }
        };
        t.apply_chunk_snapshot(t.downloaded, chunk_done);
        self.set_toast(toast);
        self.save_registry();
    }
}

#[cfg(test)]
mod evt_arm_tests {
    use super::super::App;
    use crate::app::testutil;
    use crate::engine::Evt;
    use crate::model::{FailKind, TaskState};

    fn make_app(tag: &str) -> App {
        testutil::make_app_in("ezr-evt-arms", tag)
    }

    #[tokio::test]
    async fn bt_peers_arm_updates_seeders_and_peers() {
        let mut app = make_app("bt-peers");
        app.tasks.push(testutil::bt_task(5, TaskState::Downloading));
        app.on_evt(Evt::BtPeers {
            id: 5,
            complete: 7,
            incomplete: 3,
        })
        .await;
        let t = app.tasks.iter().find(|t| t.id == 5).unwrap();
        assert_eq!(t.tracker_seeds, Some(7), "complete → tracker_seeds（x2）");
        assert_eq!(t.tracker_peers, Some(3), "incomplete → tracker_peers（y2）");
        assert_eq!(t.seeders, 0, "tracker 应答不改连接计数（x1）");
        assert_eq!(t.peers, 0, "tracker 应答不改连接计数（y1）");
    }

    #[tokio::test]
    async fn bt_conns_arm_counts_connected_seeds_and_peers() {
        let mut app = make_app("bt-conns-counts");
        app.tasks.push(testutil::bt_task(9, TaskState::Downloading));
        app.on_evt(Evt::BtConns {
            id: 9,
            conns: vec![
                crate::engine::BtConnView {
                    id: 1,
                    addr: "127.0.0.1:1".into(),
                    protocol: "tcp".into(),
                    down_speed: 0.0,
                    cum_down: 0,
                    up_speed: 0.0,
                    cum_up: 0,
                    client: "t".into(),
                    seeder: true,
                    flags: String::new(),
                    relevance: 0.0,
                },
                crate::engine::BtConnView {
                    id: 2,
                    addr: "127.0.0.1:2".into(),
                    protocol: "tcp".into(),
                    down_speed: 0.0,
                    cum_down: 0,
                    up_speed: 0.0,
                    cum_up: 0,
                    client: "t".into(),
                    seeder: false,
                    flags: String::new(),
                    relevance: 0.0,
                },
                crate::engine::BtConnView {
                    id: 3,
                    addr: "127.0.0.1:3".into(),
                    protocol: "tcp".into(),
                    down_speed: 0.0,
                    cum_down: 0,
                    up_speed: 0.0,
                    cum_up: 0,
                    client: "t".into(),
                    seeder: false,
                    flags: String::new(),
                    relevance: 0.0,
                },
            ],
            availability: 1.5,
        })
        .await;
        let t = app.tasks.iter().find(|t| t.id == 9).unwrap();
        assert_eq!(t.seeders, 1, "连接种子计数（x1）");
        assert_eq!(
            t.peers, 2,
            "连接非种子 peers 计数（y1，排除种子，FR-02-37 v1.3/D80③）"
        );
        assert_eq!(t.availability, 1.5);
        app.on_evt(Evt::BtConns {
            id: 9,
            conns: vec![
                crate::engine::BtConnView {
                    id: 10,
                    addr: "127.0.0.1:10".into(),
                    protocol: "tcp".into(),
                    down_speed: 0.0,
                    cum_down: 0,
                    up_speed: 0.0,
                    cum_up: 0,
                    client: "t".into(),
                    seeder: true,
                    flags: String::new(),
                    relevance: 0.0,
                },
                crate::engine::BtConnView {
                    id: 11,
                    addr: "127.0.0.1:11".into(),
                    protocol: "tcp".into(),
                    down_speed: 0.0,
                    cum_down: 0,
                    up_speed: 0.0,
                    cum_up: 0,
                    client: "t".into(),
                    seeder: true,
                    flags: String::new(),
                    relevance: 0.0,
                },
            ],
            availability: 2.0,
        })
        .await;
        let t = app.tasks.iter().find(|t| t.id == 9).unwrap();
        assert_eq!(t.seeders, 2, "纯种子连接 x1 随事件刷新");
        assert_eq!(
            t.peers, 0,
            "纯种子连接 y1=0（做种态 SEEDS 有数而 PEERS 无数同构呈现）"
        );
        assert_eq!(t.availability, 2.0);
    }

    #[tokio::test]
    async fn bt_trackers_arm_pushes_then_updates_rows() {
        let mut app = make_app("bt-trackers");
        app.tasks.push(testutil::bt_task(5, TaskState::Downloading));
        app.on_evt(Evt::BtTrackers {
            id: 5,
            url: "udp://t1/ann".into(),
            working: true,
            peers: Some(4),
            seeds: Some(2),
            next_in_secs: 30,
        })
        .await;
        let rows = app.trackers_status.get(&5).unwrap();
        assert_eq!(rows.len(), 1, "新 URL 推入新行");
        assert!(rows[0].working && rows[0].peers == Some(4) && rows[0].seeds == Some(2));
        app.on_evt(Evt::BtTrackers {
            id: 5,
            url: "udp://t1/ann".into(),
            working: false,
            peers: Some(9),
            seeds: Some(1),
            next_in_secs: 60,
        })
        .await;
        let rows = app.trackers_status.get(&5).unwrap();
        assert_eq!(rows.len(), 1, "同 URL 就地更新不追加");
        assert!(!rows[0].working && rows[0].peers == Some(9));
        assert!(rows[0].next_at.is_some(), "next_at 随事件刷新");
        app.on_evt(Evt::BtTrackers {
            id: 5,
            url: "udp://t2/ann".into(),
            working: true,
            peers: None,
            seeds: None,
            next_in_secs: 5,
        })
        .await;
        assert_eq!(
            app.trackers_status.get(&5).unwrap().len(),
            2,
            "异 URL 追加第二行"
        );
    }

    #[tokio::test]
    async fn bt_trackers_arm_ignored_when_not_showing_conns() {
        let mut app = make_app("bt-trackers-idle");
        app.tasks.push(testutil::bt_task(5, TaskState::Paused));
        app.on_evt(Evt::BtTrackers {
            id: 5,
            url: "udp://t1/ann".into(),
            working: true,
            peers: Some(4),
            seeds: Some(2),
            next_in_secs: 30,
        })
        .await;
        app.on_evt(Evt::BtTrackers {
            id: 999,
            url: "udp://t2/ann".into(),
            working: true,
            peers: Some(1),
            seeds: Some(1),
            next_in_secs: 30,
        })
        .await;
        assert!(
            app.trackers_status.get(&5).is_none_or(|r| r.is_empty()),
            "非活跃态任务不收 tracker 行"
        );
    }

    #[tokio::test]
    async fn bt_stopped_arm_lands_final_sets_on_paused_task() {
        let mut app = make_app("bt-stopped");
        app.tasks.push(testutil::bt_task(5, TaskState::Paused));
        app.on_evt(Evt::BtStopped {
            id: 5,
            downloaded: 500,
            done_pieces: vec![0, 1],
            partial_pieces: vec![2],
        })
        .await;
        let t = app.tasks.iter().find(|t| t.id == 5).unwrap();
        assert_eq!(t.done_blocks, vec![0, 1], "终态 done 集合落地");
        assert_eq!(t.partial_blocks, vec![2], "终态 partial 集合落地");
        assert_eq!(t.chunk_done, 2);
        assert_eq!(t.downloaded, 500);
        assert_eq!(t.speed, 0.0, "停止后速度清零");
        assert!(!app.bt_conns.contains_key(&5), "停止后连接行清空");
    }

    #[tokio::test]
    async fn bt_stopped_arm_ignores_stale_report_for_active_state() {
        let mut app = make_app("bt-stopped-stale");
        let mut t = testutil::bt_task(5, TaskState::Downloading);
        t.done_blocks = vec![9];
        app.tasks.push(t);
        app.on_evt(Evt::BtStopped {
            id: 5,
            downloaded: 500,
            done_pieces: vec![0, 1],
            partial_pieces: vec![2],
        })
        .await;
        let t = app.tasks.iter().find(|t| t.id == 5).unwrap();
        assert_eq!(t.done_blocks, vec![9], "活跃态滞后回报不得覆盖新引擎集合");
        assert_eq!(t.state, TaskState::Downloading, "状态不得回落");
    }

    #[tokio::test]
    async fn bt_failed_arm_marks_task_failed() {
        let mut app = make_app("bt-failed");
        app.tasks.push(testutil::bt_task(5, TaskState::Downloading));
        app.on_evt(Evt::BtFailed {
            id: 5,
            kind: FailKind::Fatal,
            reason: "boom".into(),
        })
        .await;
        let t = app.tasks.iter().find(|t| t.id == 5).unwrap();
        assert_eq!(t.state, TaskState::Failed, "Fatal → Failed");
        assert_eq!(t.error.as_deref(), Some("boom"));
    }
}
