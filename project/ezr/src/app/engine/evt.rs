//! engine::evt — 引擎事件消费（状态机权威转换点 on_evt）与失败统一处理
//!
//! 自单文件按职责拆出（architect v116，纯移动）。各事件臂自包含、由
//! evt_tests 全量回归锁定；臂间无共享局部量，臂内借用次序注释原样保留。

use std::time::Instant;

use super::{conn_from_view, update_conn_stats};
use crate::app::App;
use crate::engine::{Cmd, Evt};
use crate::model::speed::SpeedWindow;
use crate::model::{namegen, FailKind, TaskState};

impl App {
    /// 引擎事件处理（状态机权威转换点）。pub(super)：tick 段（兄弟文件）消费；
    /// 可见性限 app::engine 子树，app 层其余文件不可达。
    pub(super) async fn on_evt(&mut self, evt: Evt) {
        match evt {
            Evt::Probed {
                id,
                name,
                final_url,
                total,
                resumable,
                etag,
                last_modified,
            } => {
                // CD 命名回写：引擎侧已在建文件前对盘上去重（derive→dedupe 先于
                // 文件创建），App 只需对其它任务同名去重。此处不得再查盘：事件
                // 异步处理时引擎可能已建 <名字>.downloading，盘查会把自己当
                // 占用者而漂移新名（m4.bin → m4.bin.1），使 sidecar 落盘名与
                // 任务名错位、下轮续传一致性检查被跳过（FR-01-22 违例）。
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
                    t.final_url = Some(final_url);
                    t.total = total;
                    t.resumable = resumable;
                    t.probed = true;
                    t.etag = etag;
                    t.last_modified = last_modified;
                    if t.state == TaskState::Queued {
                        // 进入下载 = 新的一次下载（首次启动 / 重试重新排队 / 失效重下）：
                        // 连接级累计从零重计（FR-01-99 ②）
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
            } => {
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
                    // 已删除/未知任务的幽灵进度：不建滑窗、不聚合（删除后引擎停止确认前的兜底）
                    self.windows.remove(&id);
                    self.speed_display.remove(&id);
                    return;
                };
                if let Some(w) = self.windows.get_mut(&id) {
                    w.push(Instant::now(), downloaded);
                } else {
                    let mut w = SpeedWindow::new();
                    w.push(Instant::now(), downloaded);
                    self.windows.insert(id, w);
                }
                t.downloaded = downloaded;
                t.chunk_done = chunk_done;
                t.connections = conns.iter().map(conn_from_view).collect();
                update_conn_stats(
                    &mut self.conn_prev,
                    &mut self.conn_cum,
                    &mut self.conn_speed,
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
            } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                    t.downloaded = downloaded.max(t.downloaded);
                    t.speed = 0.0;
                    t.has_slot = false;
                    // 断点视图快照（详情页分块表）；含被覆盖的 t.chunk_done 中间写
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
            } => {
                self.handle_failure(
                    id,
                    kind,
                    reason,
                    retry_after,
                    made_progress,
                    downloaded,
                    chunk_done,
                );
            }
            Evt::Invalidated { id } => {
                // 续传一致性失效（FR-01-22）：断点作废、从头重下、不计失败；
                // 连续 3 次转停等失败（防循环）
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
                    return;
                };
                // 防循环计数（FR-01-22）：连续 3 次失效转停等失败
                let (streak, stop_wait) =
                    crate::model::retry::invalidate_streak(t.invalidation_streak);
                t.invalidation_streak = streak;
                let name = t.name.clone();
                t.downloaded = 0;
                t.chunk_done = 0;
                t.connections.clear();
                t.speed = 0.0;
                self.windows.remove(&id);
                // 会话累计账本同步重置基线：失效重下的全部字节都属本次运行
                // 下载（若保留旧基线，重下进度超过旧值前会被漏计）
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
                    // 引擎任务已随 Flow::Invalidated 结束：必须释放槽位，
                    // 调度器才能重新分配并重启（FR-01-22「从头重新下载」；
                    // slots::allocate 只补 !has_slot 的等待任务，不释放则永久卡等）
                    t.has_slot = false;
                    self.set_toast(format!(
                        "⚠ 服务器内容已更新，断点已作废，从头重新下载: {name}"
                    ));
                }
                // 连接级账本同步作废（从头重下 = 新的一次下载，FR-01-99 ②）；
                // 置于任务借用结束之后（&mut self 方法借用与字段借用不交叠）
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
                // D19「完成校验用最新值」：是否校验以任务当前 checksum 为准，
                // 不用事件携带的 spec 快照标志 —— 下载中经修改对话框清空/设置
                // 校验码（Reconfigure 不携带 checksum）后，spec 标志已过期：
                // 按过期标志校验会以空期望值产生假性「校验失败」（QA-MT-05
                // 端到端指纹）或跳过新设校验（QA-MT-04）。TDD 回归测试锁定。
                // 引擎侧收尾（改名 + 删 sidecar）按 spec 快照执行：快照过期时
                // 由 App 按文件实际位置补齐/对齐，两侧操作均幂等。
                let verify_now = t.checksum.is_some();
                if verify_now {
                    let dl = t.downloading_path();
                    let fin = t.target_path();
                    // 校验目标以文件实际位置为准（spec 无校验快照时引擎已先行改名）
                    let path = if std::path::Path::new(&fin).exists() {
                        fin
                    } else {
                        dl
                    };
                    let algo = t.checksum.as_ref().map_or("SHA-256", |c| c.algo);
                    // 校验中占槽位（D12：不释放）
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
                    // 无校验值直接完成。引擎已按 spec 收尾时此处为空操作；
                    // spec 有校验快照（引擎未收尾）→ App 幂等补收尾
                    // （去 `.downloading` 扩展名 + 删 sidecar，FR-01-24/20）
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
                    t.has_slot = false; // 校验失败释放槽位（FR-01-51）
                    t.error = Some(format!("{algo} 校验失败：内容与校验值不符"));
                    self.set_toast(format!(
                        "✘ {algo} 校验失败：不自动重试，按 R 重新校验（computed={computed} expected={expected}）: {name}"
                    ));
                }
                self.save_registry();
            }
            Evt::Cancelled { id } => {
                // 引擎已确认停止：执行延迟文件删除（「删除任务和文件」），未知 id 忽略
                if let Some((dir, name)) = self.pending_deletes.remove(&id) {
                    Self::delete_task_files(&dir, &name);
                }
            }
            Evt::Toast(msg) => self.set_toast(msg),
        }
    }

    /// 失败统一处理（FR-01-41/42/43：连续性规则 + 退避 + 停等分类）
    #[allow(clippy::too_many_arguments)]
    fn handle_failure(
        &mut self,
        id: u32,
        kind: FailKind,
        reason: String,
        retry_after: Option<f64>,
        made_progress: bool,
        downloaded: u64,
        chunk_done: u32,
    ) {
        let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) else {
            return;
        };
        if downloaded > 0 {
            t.downloaded = downloaded.max(t.downloaded);
        }
        t.speed = 0.0;
        // 重试决策（FR-01-41/42/43）：连续性规则 + 退避/Retry-After + 分类，
        // 策略唯一来源在 model::retry（DRY）
        let (auto_retry, max_retries) = (self.cfg.auto_retry, t.max_retries);
        // v1.6/FR-01-92：用户已手动挂起（FailedPaused）→ 迟到失败事件仅更新
        // 错误信息，不得转回失败态或恢复自动重试（否则挂起语义被竞态覆盖）
        if t.state == TaskState::FailedPaused {
            t.error = Some(reason.clone());
            return;
        }
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
        // max_retries 已在决策前读定（处理中不变），此处仅取更新后的计数供文案
        let retries = t.retries;
        let toast = match decision {
            crate::model::retry::RetryDecision {
                auto: true,
                delay_secs,
                ..
            } => {
                t.retry_in = Some(delay_secs);
                t.has_slot = true; // 待自动重试：继续占用槽位
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
                t.has_slot = false; // 停等：释放槽位
                format!("✘ {reason}：不自动重试，按 R 手动重试: {name}")
            }
        };
        // 断点视图快照
        t.apply_chunk_snapshot(t.downloaded, chunk_done);
        // t（可变借用）在此作用域结束后自然释放
        self.windows.remove(&id);
        self.set_toast(toast);
        self.save_registry();
    }
}
