use std::time::Instant;

use super::{conn_from_view, update_conn_stats};
use crate::app::App;
use crate::engine::{Cmd, Evt};
use crate::model::speed::SpeedWindow;
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
                    self.windows.remove(&id);
                    self.speed_display.remove(&id);
                    return;
                };
                if t.state != TaskState::Downloading {
                    return;
                }
                if let Some(w) = self.windows.get_mut(&id) {
                    w.push(Instant::now(), downloaded);
                } else {
                    let mut w = SpeedWindow::new();
                    w.push(Instant::now(), downloaded);
                    self.windows.insert(id, w);
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
                self.windows.remove(&id);
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
                self.windows.remove(&id);
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
            Evt::DownloadDone { id, total } => {
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
                    return;
                };
                t.downloaded = total;
                t.total = total;
                t.speed = 0.0;
                self.windows.remove(&id);
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
                    t.seeders = complete as usize;
                    t.peers = incomplete as usize;
                }
            }
            Evt::BtConns { id, conns } => {
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
                                down_speed: c.down_speed,
                                cum_down: c.cum_down,
                                up_speed: c.up_speed,
                                cum_up: c.cum_up,
                                client: c.client,
                                seeder: c.seeder,
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
                upload_speed,
            } => {
                self.on_bt_progress(
                    id,
                    downloaded,
                    chunk_done,
                    done_blocks,
                    partial_blocks,
                    active_blocks,
                    uploaded,
                    upload_speed,
                );
            }
            Evt::BtDone { id, total } => {
                self.on_bt_done(id, total);
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
        self.windows.remove(&id);
        self.set_toast(toast);
        self.save_registry();
    }
}
