//! task — 任务实体与单连接状态（模型层核心类型）

use super::config::ProxyChoice;
use super::namegen::{sidecar_path_of, DOWNLOADING_EXT};
use super::timefmt::fmt_created;
use super::{chunk, namegen, Checksum, FailKind, Protocol, TaskState};

#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
    /// 连接序号（1 基；连接池槽位号，稳定不随断连重编号——v1.34/FR-01-96 ③
    /// HTTP 序号 = 连接在任务连接池中的槽位号）
    pub id: usize,
    /// 分块起始字节（含）
    pub start: u64,
    /// 分块结束字节（不含）
    pub end: u64,
    /// 已下载字节
    pub done: u64,
}

/// 并发连接阶段（v1.34/FR-01-96 ⑦ 活跃口径，对齐 ezr-demo 重构版 ConnPhase）：
/// 正在连接（Connecting，握手/建连准备中，尚未传数据）/ 已连接（Connected，
/// 握手完成、持有块在传）/ 已断开（Closed，块队列耗尽后待命或自然退出，
/// 非活跃）。活跃 = 正在连接 或 已连接——并发连接面板明细只显示活跃并发，
/// 右上角「x/y 项」的 x、头部「并发」字段（FR-01-81 v1.34）与明细行数同源。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnPhase {
    /// 正在连接（握手中，尚未传数据）
    Connecting,
    /// 已连接（握手完成，持有块在传）
    Connected,
    /// 已断开/待命（块队列耗尽后自然退出；面板明细不显示）
    Closed,
}

impl ConnPhase {
    /// 是否活跃并发（正在连接 或 已连接）——活跃判定单源（FR-01-96/81 v1.34：
    /// 并发面板明细行、右上角徽标 x 与头部「并发」字段严格同源）
    #[must_use]
    pub fn is_active(self) -> bool {
        matches!(self, ConnPhase::Connecting | ConnPhase::Connected)
    }
}

impl Connection {
    /// 该连接当前持有块的总字节
    #[must_use]
    pub fn cap(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// 连接阶段派生（v1.34 活跃口径的数据面观测映射，纯函数单测面）。
    ///
    /// 引擎 Progress 视图只含持块/待命连接（连接序号 = 连接池槽位号，稳定），
    /// 「正在连接 ⇄ 已连接」的握手边界引擎不单独上报——由本轮下载累计字节
    /// （`cum_down`，App 消费侧 FR-01-99 ② 账本）观测派生：
    /// - 持块为空（cap = 0，待命条目，块队列耗尽后存续至任务收尾）→ Closed；
    /// - 持块但本轮尚无落盘字节（cum = 0）→ Connecting（建连/握手中，
    ///   含续传恢复时已领续传块但请求尚在途的连接——新请求即重新建连）；
    /// - 本轮已有落盘字节（cum > 0）→ Connected（此后保持：换块不清累计，
    ///   FR-01-99 ②；传输停顿不降级——仍持块即仍在连接）。
    #[must_use]
    pub fn conn_phase(&self, cum_down: u64) -> ConnPhase {
        if self.cap() == 0 {
            ConnPhase::Closed
        } else if cum_down > 0 {
            ConnPhase::Connected
        } else {
            ConnPhase::Connecting
        }
    }
}

/// 分块（piece）四态——pieces 弹窗（`P` 快捷键，FR-01-106）展示口径，
/// 色块字符与状态一一对应：Empty ⬜ 未下载 · Done 🟩 已下载 ·
/// Partial 🟨 部分下载 · Active 🟦 正在下载。Active 仅当任务处于「下载中」
/// 且该块已分配并发连接时出现（「分配了并发下载」即算，块内零头多少不影响）；
/// 同一块「部分下载」且「正在下载」时只显示 Active（🟦 优先于 🟨）——
/// 任务非「下载中」时任何块都不会出现 🟦。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PieceState {
    /// 未下载（⬜）
    Empty,
    /// 已下载（🟩）
    Done,
    /// 部分下载（🟨）：块内有已落盘零头，但未分配并发下载
    /// （下载中缩容回收的无租约块，或暂停/等待/失败等状态下沉时冻结的
    /// 持块连接）
    Partial,
    /// 正在下载（🟦）：已分配并发连接（仅「下载中」任务）
    Active,
}

/// 下载任务（UI 权威状态；真实进度由引擎事件驱动更新）
#[derive(Clone, Debug)]
pub struct Task {
    /// 任务 ID（会话内自增，注册表持久化）
    pub id: u32,
    /// 目标文件名（最终名；下载中盘上文件追加 `.downloading`，FR-01-24）
    pub name: String,
    /// 协议
    pub protocol: Protocol,
    /// 原始 URL（v1.28：任务级唯一 URL 权威，FR-01-14；探测/数据请求均从
    /// 本 URL 独立发起并跟跳转，跳转落点不持久化）
    pub url: String,
    /// 任务名已随首次探测定稿（v1.28「首次探测已定名」标记；原
    /// `final_url.is_some()` 口径的替身——首次 Probed 置位后不再重置，
    /// 之后的一切重启沿用既有名，防止去重漂移，FR-01-22）
    pub name_locked: bool,
    /// 保存目录（不含文件名）
    pub save_dir: String,
    /// 文件总大小（字节；探测前为 0，等待行显示「未知」）
    pub total: u64,
    /// 已下载字节
    pub downloaded: u64,
    /// 当前速度 B/s（1s 滑窗，FR-01-17）
    pub speed: f64,
    /// 状态
    pub state: TaskState,
    /// 服务器是否支持断点续传（不支持时单并发整体下载，FR-01-12）
    pub resumable: bool,
    /// 是否已完成探测（等待中未探测时大小显示「未知」）
    pub probed: bool,
    /// 并发分块连接（下载中 = 活跃分块；其余状态 = 空或最后快照）
    pub connections: Vec<Connection>,
    /// AIR2：已完成分块数 x
    pub chunk_done: u32,
    /// 已完成块号集合（升序；乱序完成安全，pieces 弹窗 🟩 权威口径）。
    /// 由 Progress 事件逐 tick 全量重铺（引擎 Blocks 逐块真值），仅会话内
    /// 存在，不持久化——重启后空集回退 chunk_done 计数前缀口径。
    pub done_blocks: Vec<u32>,
    /// 部分下载块号集合（升序；块内已有字节未达块长，pieces 弹窗 🟨 权威
    /// 口径）。由 Progress 逐 tick 全量重铺、PausedDone/Failed 携带终态快照
    /// 重铺（引擎 Blocks 逐块真值），仅会话内存在，不持久化——重启后空集
    /// 回退投影租约零头口径（v129 行为）。
    pub partial_blocks: Vec<u32>,
    /// 块大小（字节；HTTP 默认 1 MB 可配置，添加时按当时配置定死，D13）
    pub block_size: u64,
    /// 并发数（1–64）
    pub concurrency: usize,
    /// 任务级代理选择（v1.5/FR-01-86，D18 三态；旧注册表缺省 = Global）
    pub proxy: ProxyChoice,
    /// 已自动重试次数（连续失败时累加；有进展重置为 1；R 手动重试重置为 1）
    pub retries: u32,
    /// 自动重试上限（配置 `max_retries`，默认 5）
    pub max_retries: u32,
    /// 本次尝试（自上次失败/开始以来）是否下载到过数据（重试计数连续性判定）
    pub made_progress: bool,
    /// 距下次自动重试的倒计时（秒）；None = 不再自动重试
    pub retry_in: Option<f64>,
    /// 最近一次失败的类别（决定「不自动重试/已达上限」行文案与 R 重试语义）
    pub fail_kind: Option<FailKind>,
    /// 最近一次失败原因（「已失败」态详情「失败原因」行；挂起态不再持有——
    /// v1.23 起失败挂起时即重置，与普通暂停数据一致）
    pub error: Option<String>,
    /// 连续「续传一致性失效」次数（FR-01-22 防循环：连续 3 次转已失败停等）
    pub invalidation_streak: u32,
    /// 完整性校验期望（None = 无校验值，完成后不校验）
    pub checksum: Option<Checksum>,
    /// 校验结果；None = 未校验（无校验码或尚未完成）
    pub verify_ok: Option<bool>,
    /// 服务器 ETag（探测后填；一致性检查用）
    pub etag: Option<String>,
    /// 服务器 Last-Modified（探测后填；一致性检查用）
    pub last_modified: Option<String>,
    /// 是否持有下载槽位
    pub has_slot: bool,
    /// BT 上传速度 B/s（01 恒 0；FR-01-81 头部 ↑ 字段保留，BT 二期启用）
    #[allow(dead_code)]
    pub upload_speed: f64,
    /// BT 累计上传字节（02 预留）
    #[allow(dead_code)]
    pub uploaded: u64,
    /// BT 可连接做种数（02 预留，01 详情不出现）
    #[allow(dead_code)]
    pub seeders: usize,
    /// BT 邻居节点数（02 预留）
    #[allow(dead_code)]
    pub peers: usize,
    /// BT 剩余做种时间秒（02 预留）
    #[allow(dead_code)]
    pub seed_left: f64,
    /// 添加时间（显示与 sidecar/注册表记录）
    pub created: String,
    /// 添加时刻的 Unix 秒（注册表用）
    pub added_at: u64,
}

impl Task {
    /// 新建排队中的下载任务（添加对话框与 CLI 添加共用初值口径，FR-01-01/03/04）：
    /// 进度/连接/重试等运行时字段全部取初值，`created` 由 `added_at` 换算。
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new_queued(
        id: u32,
        name: String,
        protocol: Protocol,
        url: String,
        save_dir: String,
        block_size: u64,
        concurrency: usize,
        max_retries: u32,
        checksum: Option<Checksum>,
        proxy: ProxyChoice,
        added_at: u64,
    ) -> Task {
        Task {
            id,
            name,
            protocol,
            url,
            name_locked: false,
            save_dir,
            total: 0,
            downloaded: 0,
            speed: 0.0,
            state: TaskState::Queued,
            resumable: true,
            probed: false,
            connections: vec![],
            chunk_done: 0,
            done_blocks: Vec::new(),
            partial_blocks: Vec::new(),
            block_size,
            concurrency,
            proxy,
            retries: 0,
            max_retries,
            made_progress: false,
            retry_in: None,
            fail_kind: None,
            error: None,
            invalidation_streak: 0,
            checksum,
            verify_ok: None,
            etag: None,
            last_modified: None,
            has_slot: false,
            upload_speed: 0.0,
            uploaded: 0,
            seeders: 0,
            peers: 0,
            seed_left: 0.0,
            created: fmt_created(added_at),
            added_at,
        }
    }

    /// 进度比例 0.0–1.0
    #[must_use]
    pub fn progress(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.downloaded as f64 / self.total as f64).clamp(0.0, 1.0)
        }
    }

    /// 文件总大小是否已知（v1.34/FR-01-116 展示口径，对齐 ezr-demo 重构版
    /// `Task::size_known` bug 修订）：
    /// - BT：元信息自带大小，恒已知（phase-02）；
    /// - 非 HTTP(S) 协议：按已知处理；
    /// - HTTP(S) 支持断点续传（响应携带 Content-Length）：已知；
    /// - HTTP(S) 不支持续传 = 流式下载（服务器不回 Content-Length）：
    ///   下载期间总大小未知——直到流结束进入校验/后期处理/完成态
    ///   （DownloadDone 携带实际字节数回填 total）才得知实际大小。
    #[must_use]
    pub fn size_known(&self) -> bool {
        if self.protocol.is_bt() {
            return true;
        }
        if !matches!(self.protocol, Protocol::Http | Protocol::Https) {
            return true;
        }
        if self.resumable {
            return true;
        }
        matches!(
            self.state,
            TaskState::Verifying | TaskState::PostProcessing | TaskState::Completed
        )
    }

    /// 剩余时间（秒；速度为 0 或非下载态返回 None）。
    /// v1.34/FR-01-116：总大小未知（不支持续传的 HTTP(S) 流式下载）时
    /// 剩余时间无从估计，恒返回 None（展示层显示 `-`）。
    #[must_use]
    pub fn eta_secs(&self) -> Option<u64> {
        if !self.size_known() {
            return None;
        }
        if self.speed <= 0.0 || self.state != TaskState::Downloading {
            return None;
        }
        let remain = self.total.saturating_sub(self.downloaded) as f64;
        Some((remain / self.speed) as u64)
    }

    /// 并发线程数（展示用）：下载中 = 活跃连接数（数据面口径，FR-01-81 修订二：
    /// 持有未完成块的连接），其余状态 = 有效连接数（排除已待命的空连接）
    /// FR-01-100（详情类型行去并发数）后产品面暂无调用点；数据面语义测试保留，
    /// phase-02 BT（做种/上传速度展示）预期复用
    #[allow(dead_code)]
    #[must_use]
    pub fn thread_count(&self) -> usize {
        if self.state == TaskState::Downloading {
            self.connections
                .iter()
                .filter(|c| c.cap() > 0 && c.done < c.cap())
                .count()
        } else {
            self.connections.iter().filter(|c| c.cap() > 0).count()
        }
    }

    /// 分块信息 (x=已完成块数, y=总块数, n=块大小字节)
    #[must_use]
    pub fn chunk_info(&self) -> (u32, u64, u64) {
        let y = chunk::chunk_total(self.total, self.block_size);
        (self.chunk_done.min(y), u64::from(y), self.block_size)
    }

    /// 是否支持分块下载（pieces 弹窗 `P` 快捷键的功能门槛，FR-01-106）：
    /// BT 恒分块（协议原生模型，phase-02 预留）；HTTP(S) 需服务器支持断点
    /// 续传（Range）——不支持续传时只能单流整段下载、无分块概念。
    #[must_use]
    pub fn supports_pieces(&self) -> bool {
        if self.protocol.is_bt() {
            return true;
        }
        self.resumable
    }

    /// 全量分块状态（pieces 弹窗展示口径，块序 = 字节序，FR-01-106 ③）。
    ///
    /// 口径（与 [`PieceState`] 文档一致）：
    /// - 已完成块 🟩：优先按逐块完成集合 `done_blocks`（Progress 事件逐 tick
    ///   全量重铺的引擎 Blocks 逐块真值）——多并发乱序完成时计数前缀错位
    ///   （真完成块不在前缀、连接已移走后无租约可依，操作者实证：已完成块
    ///   白屏）；集合为空（重启后未收到 Progress）时回退计数前缀
    ///   `块号 < chunk_done`；
    /// - 块被并发连接持有（分块队列按块边界整块领块，连接 start 恒对齐块
    ///   边界）：任务「下载中」→ Active（分配即算，块内零头多少不影响——
    ///   「部分下载」且「正在下载」时只显示 Active）；任务非「下载中」
    ///   （暂停/等待/失败等状态下沉冻结的持块连接）→ 有零头 Partial、
    ///   零头为 0 → Empty；
    /// - 连接块内写满（done >= cap）→ Done（乱序完成块不必依赖前缀）；
    /// - 部分下载块 🟨（v131；**v136 扩面：集合权威不再限定任务状态**）：
    ///   逐块部分集合 `partial_blocks` 非空（Progress 逐 tick / PausedDone・
    ///   Failed 终态快照重铺的引擎 Blocks 逐块真值）→ 集合权威标 🟨——
    ///   收尾投影租约按前缀同参投影只覆盖尾部 ≤ 并发数个块，投影外的真
    ///   部分块无租约错标 ⬜（操作者实证：失败/暂停后部分块白屏）、投影内
    ///   假零头虚标 🟨，集合非空时租约零头退出 🟨 判定（双向消除）；v136
    ///   操作者实证「M 缩容 4→2 继续下载后无租约部分块白屏」：下载中同样
    ///   存在「有落盘零头但无租约」的块（缩容回收的租约、quota 中止块），
    ///   集合权威扩到下载中——有租约的部分块仍 🟦（连接循环先判 Active，
    ///   集合循环只把 ⬜ 提升为 🟨，「部分下载且正在下载只显示 Active」
    ///   不变）、已完成块不被降级；集合为空时回退投影租约零头口径
    ///   （v129 行为，重启后未收到逐块事件的存量面）；
    /// - 已完成块（🟩）不被部分/空租约降级（暂停快照为前缀同参投影，可能
    ///   与逐块真值错位；下载中领块跳过已完成块，部分租约不落已完成块）；
    /// - 其余 → Empty（未下载）。
    ///
    /// 空/极小文件：y = 0（无分块）返回空表。
    #[must_use]
    pub fn pieces(&self) -> Vec<PieceState> {
        let y = usize::try_from(chunk::chunk_total(self.total, self.block_size)).unwrap_or(0);
        if y == 0 {
            return Vec::new();
        }
        let mut v = vec![PieceState::Empty; y];
        if self.done_blocks.is_empty() {
            // 回退：无逐块集合（重启后未收到 Progress / 旧事件流）时维持
            // 计数前缀口径（v129 原行为）
            let x = (self.chunk_done as usize).min(y);
            for s in v.iter_mut().take(x) {
                *s = PieceState::Done;
            }
        } else {
            // 逐块真值：乱序完成安全，已完成块精确标 🟩
            for &i in &self.done_blocks {
                if let Some(idx) = usize::try_from(i).ok().filter(|&idx| idx < y) {
                    v[idx] = PieceState::Done;
                }
            }
        }
        let downloading = self.state == TaskState::Downloading;
        // v131：引擎逐块部分真值在位 → 部分块集合权威（与 🟩 集合口径对称）：
        // 收尾投影租约的块内零头是按权重散布的展示值，不反映真实分布——
        // 集合非空时租约零头退出 🟨 判定。v136（操作者实证「M 缩容 4→2 继续
        // 下载后无租约部分块白屏」）：集合权威不再限定「非下载中」——下载中
        // 同样存在有落盘零头但无租约的块（缩容回收的租约、quota 中止块），
        // 排除下载中会让它们错标 ⬜；下游已保证优先级不破：活跃租约块由
        // 连接循环先判 Active（🟦 优先于 🟨 不变）、集合循环只把 ⬜ 提升为
        // 🟨 且不覆盖 🟩。
        let partial_authoritative = !self.partial_blocks.is_empty();
        for c in &self.connections {
            if c.cap() == 0 {
                continue; // 待命连接（start=end）不持块
            }
            let idx = (c.start / self.block_size) as usize;
            if idx >= y {
                continue; // 防御：越界租约不计
            }
            let s = if c.done >= c.cap() {
                PieceState::Done
            } else if downloading {
                PieceState::Active
            } else if partial_authoritative {
                PieceState::Empty // 集合权威：租约零头不参与 🟨 判定
            } else if c.done > 0 {
                PieceState::Partial
            } else {
                PieceState::Empty
            };
            // 已完成块不被降级：🟩 保持，除非租约同样判满（Done）
            if v[idx] != PieceState::Done || s == PieceState::Done {
                v[idx] = s;
            }
        }
        if partial_authoritative {
            // 集合标 🟨：只把 ⬜ 提升为 🟨（不覆盖 🟩；与租约臂的防降级
            // 对称，投影租约已摆错的块由集合真值改正）
            for &i in &self.partial_blocks {
                if let Some(idx) = usize::try_from(i).ok().filter(|&idx| idx < y) {
                    if v[idx] == PieceState::Empty {
                        v[idx] = PieceState::Partial;
                    }
                }
            }
        }
        v
    }

    /// 目标文件完整路径（保存目录 + 文件名）
    #[must_use]
    pub fn target_path(&self) -> String {
        namegen::join_path(&self.save_dir, &self.name)
    }

    /// 下载中的盘上文件路径（追加 `.downloading` 扩展名，FR-01-24；扩展名
    /// 单源于 [`namegen::DOWNLOADING_EXT`]，architect v116）
    #[must_use]
    pub fn downloading_path(&self) -> String {
        format!("{}{DOWNLOADING_EXT}", self.target_path())
    }

    /// sidecar 元数据文件路径（`<目标文件>.ezr`，D1；委托
    /// [`namegen::sidecar_path_of`] 单源——与 (目录, 文件名) 散点同口径）
    #[must_use]
    pub fn sidecar_path(&self) -> String {
        sidecar_path_of(&self.save_dir, &self.name)
    }

    /// 暂停/失败收尾的断点视图快照（详情页分块表）：按 total/downloaded/
    /// concurrency/block_size 推导连接视图。分块口径两段合并（v139 修订，
    /// 操作者实证：error 时显示 13、恢复后回退 12 的倒退缺陷）：多块路径
    /// （可续传 ∧ 分块数 > 1）事件逐块记账权威——连续前缀 x = done/piece
    /// 在乱序完成 + 多个部分块零头合计越块界时高估完成数（幻影 +1），
    /// 不得盖过事件口径；单流路径（不可续传/单块）Progress 事件不带逐块
    /// 记账（chunk_done = 0），字节连续、前缀快照精确，维持两者较大值兜底。
    /// `downloaded` 由调用方显式传入（暂停/失败事件对「已下载」口径不同）。
    pub fn apply_chunk_snapshot(&mut self, downloaded: u64, chunk_done: u32) {
        let (conns, snap) =
            chunk::lease_snapshot(self.total, downloaded, self.concurrency, self.block_size);
        self.connections = conns;
        let multi_block = self.resumable && chunk::chunk_total(self.total, self.block_size) > 1;
        self.chunk_done = if multi_block {
            chunk_done
        } else {
            snap.max(chunk_done)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// v129/FR-01-106：分块（piece）四态——pieces 弹窗展示口径。
    /// Empty ⬜ 未下载 · Done 🟩 已下载 · Partial 🟨 部分下载 · Active 🟦 正在下载。
    #[test]
    fn piece_state_covers_four_states() {
        // 四态互异且可判等（渲染层 run 合并与单测断言依赖 PartialEq）
        let states = [
            PieceState::Empty,
            PieceState::Done,
            PieceState::Partial,
            PieceState::Active,
        ];
        for (i, a) in states.iter().enumerate() {
            for (j, b) in states.iter().enumerate() {
                assert_eq!(a == b, i == j, "四态互异：{a:?} vs {b:?}");
            }
        }
    }

    /// v129/FR-01-106 功能门槛：BT 恒支持分块（协议原生模型，phase-02 预留）；
    /// HTTP(S) 需服务器支持断点续传——不支持续传只能单流整段下载、无分块概念。
    #[test]
    fn supports_pieces_bt_always_http_needs_resume() {
        let mut t = crate::model::sample_task();
        t.protocol = Protocol::Bt;
        t.resumable = false;
        assert!(t.supports_pieces(), "BT 恒支持（resumable 与门槛无关）");
        t.protocol = Protocol::Http;
        assert!(!t.supports_pieces(), "HTTP 不可续传 → 不支持分块");
        t.resumable = true;
        assert!(t.supports_pieces(), "HTTP 可续传 → 支持");
        t.protocol = Protocol::Https;
        assert!(t.supports_pieces(), "HTTPS 可续传 → 支持");
    }

    /// v129/FR-01-106：空内容（total = 0，无分块）→ 空 Vec（弹窗拒绝臂在 app 层，
    /// 模型层直接给空表）
    #[test]
    fn pieces_empty_file_returns_empty_vec() {
        let mut t = crate::model::sample_task();
        t.total = 0;
        t.probed = true;
        t.state = TaskState::Downloading;
        assert!(t.pieces().is_empty(), "total=0 无分块");
    }

    /// v129/FR-01-106 ③：块号 < chunk_done → Done（前缀口径）；其余 Empty；
    /// 非「下载中」任务任何块都不出现 Active。
    #[test]
    fn pieces_done_prefix_rest_empty_when_not_downloading() {
        let mut t = crate::model::sample_task();
        t.total = 6_000_000;
        t.block_size = 1_000_000;
        t.chunk_done = 2;
        t.state = TaskState::Paused;
        t.probed = true;
        let v = t.pieces();
        assert_eq!(v.len(), 6);
        assert_eq!(v[0], PieceState::Done);
        assert_eq!(v[1], PieceState::Done);
        assert!(v[2..].iter().all(|s| *s == PieceState::Empty));
        assert!(!v.contains(&PieceState::Active), "非下载中不出现 🟦");
    }

    /// v129/FR-01-106 ③：下载中任务被并发连接持有的块 → Active（分配即算，
    /// 块内零头多少不影响；done=0 也算）。连接租约恒为单块（引擎 ConnView
    /// 携带 block 号，start 恒对齐块边界）。
    #[test]
    fn pieces_active_when_downloading_assigned_even_zero_progress() {
        let mut t = crate::model::sample_task();
        t.total = 4_000_000;
        t.block_size = 1_000_000;
        t.chunk_done = 1;
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![
            Connection {
                id: 1,
                start: 1_000_000, // 块 1
                end: 2_000_000,
                done: 0, // 零头 0：分配即算
            },
            Connection {
                id: 2,
                start: 3_000_000, // 块 3
                end: 4_000_000,
                done: 0,
            },
        ];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Done, "块 0 已完成");
        assert_eq!(v[1], PieceState::Active, "块 1 被持有（done=0 也算）");
        assert_eq!(v[2], PieceState::Empty, "块 2 无连接持有");
        assert_eq!(v[3], PieceState::Active, "块 3 被持有");
    }

    /// v129/FR-01-106 ③：同一块既有已落盘零头又在传 → 只显示 Active
    ///（🟦 优先于 🟨）；任务非「下载中」时同块有零头 → Partial（状态下沉
    /// 冻结的持块连接）、无零头 → Empty。
    #[test]
    fn pieces_active_wins_over_partial_and_freezes_on_pause() {
        let mut t = crate::model::sample_task();
        t.total = 3_000_000;
        t.block_size = 1_000_000;
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![Connection {
            id: 1,
            start: 1_000_000,
            end: 2_000_000,
            done: 300_000, // 块 1 有零头且在传
        }];
        let v = t.pieces();
        assert_eq!(v[1], PieceState::Active, "🟦 优先于 🟨（下载中）");
        // 状态下沉（空格暂停）：同一连接视图冻结 → 有零头 Partial
        t.state = TaskState::Paused;
        let v = t.pieces();
        assert_eq!(v[1], PieceState::Partial, "非下载中：有零头 → 🟨");
        // 无零头的冻结持块 → Empty
        t.connections[0].done = 0;
        let v = t.pieces();
        assert_eq!(v[1], PieceState::Empty, "非下载中：零头 0 → ⬜");
    }

    /// v129/FR-01-106 ③：连接块内写满（done >= cap）→ Done（乱序完成块不必
    /// 依赖 chunk_done 前缀）；前缀与乱序完成并存时互不覆盖。
    #[test]
    fn pieces_completed_lease_shows_done() {
        let mut t = crate::model::sample_task();
        t.total = 4_000_000;
        t.block_size = 1_000_000;
        t.chunk_done = 1;
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![Connection {
            id: 2,
            start: 2_000_000,
            end: 3_000_000,
            done: 1_000_000, // 块 2 已写满（乱序完成）
        }];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Done, "前缀块 0");
        assert_eq!(v[1], PieceState::Empty, "块 1 无连接持有（跳过）");
        assert_eq!(v[2], PieceState::Done, "块 2 乱序写满 → 🟩");
        assert_eq!(v[3], PieceState::Empty);
    }

    /// 操作者 bug 报告回归（多并发 >= 4 乱序完成）：块 2、5 已完成且下载
    /// 线程已移走（不在 conns），chunk_done 计数 = 2 但前缀位置 0/1 并未
    /// 完成——pieces 必须按 Progress 携带的逐块完成集合（done_blocks）
    /// 标 🟩；计数前缀口径在乱序完成下错位（真完成块白屏、前缀位置虚绿）。
    #[test]
    fn pieces_out_of_order_done_set_shows_green() {
        let mut t = crate::model::sample_task();
        t.total = 6_000_000;
        t.block_size = 1_000_000;
        t.chunk_done = 2;
        t.done_blocks = vec![2, 5];
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![
            Connection {
                id: 1,
                start: 0,
                end: 1_000_000,
                done: 400_000, // 块 0 在传
            },
            Connection {
                id: 2,
                start: 1_000_000,
                end: 2_000_000,
                done: 0, // 块 1 在传（分配即算）
            },
            Connection {
                id: 3,
                start: 3_000_000,
                end: 4_000_000,
                done: 700_000, // 块 3 在传
            },
            Connection {
                id: 4,
                start: 4_000_000,
                end: 5_000_000,
                done: 100_000, // 块 4 在传
            },
        ];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Active);
        assert_eq!(v[1], PieceState::Active);
        assert_eq!(v[2], PieceState::Done, "乱序完成块 2：连接已移走仍须 🟩");
        assert_eq!(v[3], PieceState::Active);
        assert_eq!(v[4], PieceState::Active);
        assert_eq!(v[5], PieceState::Done, "乱序完成块 5：连接已移走仍须 🟩");
    }

    /// 已完成块不被冻结租约降级：暂停/失败快照（前缀同参投影）可能与逐块
    /// 真值错位——投影把部分零头租约摆在真实已完成块上时，🟩 不得被降级
    /// 为 🟨/⬜（下载中满租约与集合口径恒一致：领块跳过已完成块，部分
    /// 租约不会落在真实已完成块上）。
    #[test]
    fn pieces_done_set_not_downgraded_by_frozen_lease() {
        let mut t = crate::model::sample_task();
        t.total = 3_000_000;
        t.block_size = 1_000_000;
        t.chunk_done = 1;
        t.done_blocks = vec![0, 2];
        t.state = TaskState::Paused;
        t.probed = true;
        t.connections = vec![Connection {
            id: 1,
            start: 2_000_000, // 投影租约错位落在真实已完成块 2 上
            end: 3_000_000,
            done: 300_000,
        }];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Done);
        assert_eq!(v[2], PieceState::Done, "已完成块不被冻结租约降级为 🟨");
    }

    /// 操作者 bug 报告回归（v131，多并发失败/暂停）：部分下载块（引擎
    /// Blocks.written 已有字节）在收尾投影租约之外 → 旧实现白屏（⬜）。
    /// pieces 🟨 必须按事件携带的逐块部分集合（partial_blocks）标黄；投影
    /// 租约的假零头（集合外块）不再虚标 🟨——集合非空时部分块口径以集合
    /// 为权威（与 🟩 集合口径对称）。
    #[test]
    fn pieces_failed_partial_set_shows_yellow() {
        let mut t = crate::model::sample_task();
        t.total = 5_000_000;
        t.block_size = 1_000_000;
        t.done_blocks = vec![0];
        t.partial_blocks = vec![1, 3];
        t.state = TaskState::Failed;
        t.probed = true;
        // 失败收尾投影租约（lease_snapshot 前缀同参投影的形态）：只覆盖
        // 尾部一个块（min(并发, 未完成块数)），真实部分块 1、3 在投影外
        t.connections = vec![Connection {
            id: 1,
            start: 2_000_000, // 投影把零头摆在块 2 上（块 2 真值无字节）
            end: 3_000_000,
            done: 300_000,
        }];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Done, "块 0 已完成");
        assert_eq!(v[1], PieceState::Partial, "投影外的真部分块 1 → 🟨");
        assert_eq!(v[2], PieceState::Empty, "投影租约假零头不虚标（集合权威）");
        assert_eq!(v[3], PieceState::Partial, "投影外的真部分块 3 → 🟨");
        assert_eq!(v[4], PieceState::Empty, "块 4 无字节");
    }

    /// 暂停态同口径：真部分块上有零头为 0 的冻结租约（Empty 臂）时，
    /// 集合真值不得被降级为 ⬜。
    #[test]
    fn pieces_paused_partial_set_not_downgraded_by_idle_lease() {
        let mut t = crate::model::sample_task();
        t.total = 3_000_000;
        t.block_size = 1_000_000;
        t.done_blocks = vec![0, 2];
        t.partial_blocks = vec![1];
        t.state = TaskState::Paused;
        t.probed = true;
        t.connections = vec![Connection {
            id: 1,
            start: 1_000_000, // 投影租约落在部分块 1 上，零头 0
            end: 2_000_000,
            done: 0,
        }];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Done);
        assert_eq!(v[1], PieceState::Partial, "零头租约不把集合真值降级为 ⬜");
        assert_eq!(v[2], PieceState::Done);
    }

    /// 下载中口径不变：有租约的部分块仍 🟦（「部分下载且正在下载只显示
    /// Active」，FR-01-106 定稿口径）——集合权威扩面（v136）只补无租约
    /// 部分块的 🟨，不改有租约块的 🟦。
    #[test]
    fn pieces_downloading_partial_set_stays_active() {
        let mut t = crate::model::sample_task();
        t.total = 3_000_000;
        t.block_size = 1_000_000;
        t.done_blocks = vec![0];
        t.partial_blocks = vec![1];
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![Connection {
            id: 1,
            start: 1_000_000,
            end: 2_000_000,
            done: 400_000,
        }];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Done);
        assert_eq!(v[1], PieceState::Active, "下载中 🟦 口径不变");
        assert_eq!(v[2], PieceState::Empty);
    }

    /// 操作者 bug 报告回归（v136，下载中缩容白屏）：4 并发 4 个部分块时
    /// 暂停（partial_blocks = [0,1,2,3]，按 P 恒 4 个 🟨），M 缩容为 2 并发
    /// 继续下载——引擎仅 2 个活跃租约（块 0、1 被领走继续传），块 2、3
    /// 有落盘零头但无租约。旧实现集合权威限定 `!downloading` → 无租约
    /// 部分块在下载中无判定路径 → ⬜ 白屏。修复后口径：集合权威不限任务
    /// 状态——有租约的部分块仍 🟦（🟦 优先于 🟨），无租约部分块恢复 🟨，
    /// 无字节块仍 ⬜。
    #[test]
    fn pieces_downloading_unleased_partial_blocks_show_yellow() {
        let mut t = crate::model::sample_task();
        t.total = 6_000_000;
        t.block_size = 1_000_000;
        t.partial_blocks = vec![0, 1, 2, 3];
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![
            Connection {
                id: 1,
                start: 0, // 缩容后 worker 领走块 0 继续传
                end: 1_000_000,
                done: 400_000,
            },
            Connection {
                id: 2,
                start: 1_000_000, // worker 领走块 1（零头 0 也算）
                end: 2_000_000,
                done: 0,
            },
        ];
        let v = t.pieces();
        assert_eq!(v.len(), 6);
        assert_eq!(v[0], PieceState::Active, "有租约的部分块 0 仍 🟦");
        assert_eq!(v[1], PieceState::Active, "有租约的块 1 🟦（分配即算）");
        assert_eq!(v[2], PieceState::Partial, "缩容回收的无租约部分块 2 → 🟨");
        assert_eq!(v[3], PieceState::Partial, "缩容回收的无租约部分块 3 → 🟨");
        assert_eq!(v[4], PieceState::Empty, "块 4 无字节");
        assert_eq!(v[5], PieceState::Empty, "块 5 无字节");
    }

    /// 同口径防降级护栏：下载中无租约部分块的集合提升只作用于 ⬜——
    /// 集合真值为已完成的块（🟩）不得被 partial_blocks 误降级为 🟨。
    #[test]
    fn pieces_downloading_unleased_partial_not_downgrading_done() {
        let mut t = crate::model::sample_task();
        t.total = 5_000_000;
        t.block_size = 1_000_000;
        t.done_blocks = vec![2];
        t.partial_blocks = vec![1, 2, 3];
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![Connection {
            id: 1,
            start: 0, // 仅块 0 在传；块 1、2、3 无租约
            end: 1_000_000,
            done: 100_000,
        }];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Active);
        assert_eq!(v[1], PieceState::Partial, "无租约部分块 1 → 🟨");
        assert_eq!(v[2], PieceState::Done, "🟩 不被集合降级");
        assert_eq!(v[3], PieceState::Partial, "无租约部分块 3 → 🟨");
        assert_eq!(v[4], PieceState::Empty);
    }

    /// 空集回退（与 🟩 前缀回退对称）：重启后未收到逐块事件（partial_blocks
    /// 空）时维持 v129 投影租约零头口径（有零头 → 🟨）。
    #[test]
    fn pieces_paused_empty_partial_set_falls_back_to_lease() {
        let mut t = crate::model::sample_task();
        t.total = 3_000_000;
        t.block_size = 1_000_000;
        t.done_blocks = Vec::new();
        t.partial_blocks = Vec::new();
        t.chunk_done = 1;
        t.state = TaskState::Paused;
        t.probed = true;
        t.connections = vec![Connection {
            id: 1,
            start: 2_000_000,
            end: 3_000_000,
            done: 300_000,
        }];
        let v = t.pieces();
        assert_eq!(v[0], PieceState::Done, "空集回退计数前缀");
        assert_eq!(v[2], PieceState::Partial, "回退租约零头口径（v129 行为）");
    }

    /// v129/FR-01-106 ③：待命连接（cap = 0）不持块，不计入四态；
    /// 越界租约（start 超出总块数）防御性忽略。
    #[test]
    fn pieces_idle_and_out_of_range_leases_ignored() {
        let mut t = crate::model::sample_task();
        t.total = 2_000_000;
        t.block_size = 1_000_000;
        t.state = TaskState::Downloading;
        t.probed = true;
        t.connections = vec![
            Connection {
                id: 1,
                start: 0,
                end: 0, // 待命连接
                done: 0,
            },
            Connection {
                id: 2,
                start: 9_000_000, // 越界租约
                end: 10_000_000,
                done: 100,
            },
        ];
        let v = t.pieces();
        assert!(v.iter().all(|s| *s == PieceState::Empty), "两条连接均不计");
    }

    /// v129/FR-01-106 ③：已完成任务全量 🟩（chunk_done = y 钳制口径与
    /// chunk_info 一致，详情「分块」行同源）。
    #[test]
    fn pieces_completed_task_all_done() {
        let mut t = crate::model::sample_task();
        t.total = 2_500_000;
        t.block_size = 1_000_000;
        t.chunk_done = 3;
        t.state = TaskState::Completed;
        t.probed = true;
        let v = t.pieces();
        assert_eq!(v.len(), 3);
        assert!(v.iter().all(|s| *s == PieceState::Done));
        // chunk_done 超出总块数时按 y 钳制（防御：不 panic 不越界）
        t.chunk_done = 99;
        assert_eq!(t.pieces().len(), 3);
    }

    /// v139 回归（操作者实证 bug）：多块乱序完成 + 多个部分块零头合计越块界
    /// 时，暂停/失败断点视图的连续前缀快照（x = done/piece）把未完成块计成
    /// 完成（幻影 +1），经 max() 盖过事件逐块真值写入 task.chunk_done 并随
    /// 注册表持久化；恢复后引擎事件回到逐块真值——详情「分块」计数倒退
    /// 观感。多块路径（可续传 ∧ 分块数 > 1）事件口径权威：快照只投影连接
    /// 视图，不得抬高完成数。
    #[test]
    fn chunk_snapshot_multi_block_event_truth_beats_prefix_phantom() {
        let mut t = crate::model::sample_task();
        t.total = 15_000_000;
        t.block_size = 1_000_000;
        t.resumable = true;
        // 事件逐块真值：块 0-10 满 + 块 12 乱序完成（计 12），块 11/13 在传
        // 部分块（0.6MB + 0.5MB）；合计 13.1MB → 前缀快照 floor(13.1) = 13
        // > 真值 12（幻影 +1）
        let downloaded = 11_000_000 + 600_000 + 1_000_000 + 500_000;
        t.apply_chunk_snapshot(downloaded, 12);
        assert_eq!(
            t.chunk_done, 12,
            "多块路径事件逐块真值权威，前缀幻影不得盖过"
        );
        let (x, y, piece) = t.chunk_info();
        assert_eq!((x, y, piece), (12, 15, 1_000_000));
        // 连接视图仍为前缀同参投影（展示面不变）
        let (conns, _) = chunk::lease_snapshot(t.total, downloaded, t.concurrency, t.block_size);
        assert_eq!(t.connections, conns);
    }

    /// 单流路径（不可续传）事件不带逐块记账（chunk_done = 0），字节连续、
    /// 前缀快照精确：维持 max(快照, 事件) 兜底（v139 修订不改变单流口径）。
    #[test]
    fn chunk_snapshot_single_stream_keeps_prefix_fallback() {
        let mut t = crate::model::sample_task();
        t.total = 15_000_000;
        t.block_size = 1_000_000;
        t.resumable = false;
        t.apply_chunk_snapshot(13_100_000, 0);
        assert_eq!(t.chunk_done, 13, "单流路径前缀快照精确，维持较大值兜底");
    }

    /// 单块任务（分块数 = 1）即便可续传也走快照口径（块内连续、前缀精确）。
    #[test]
    fn chunk_snapshot_single_block_keeps_prefix_fallback() {
        let mut t = crate::model::sample_task();
        t.total = 1_000_000;
        t.block_size = 1_000_000;
        t.resumable = true;
        t.apply_chunk_snapshot(600_000, 0);
        assert_eq!(t.chunk_done, 0);
        t.apply_chunk_snapshot(1_000_000, 0);
        assert_eq!(t.chunk_done, 1);
    }

    // -----------------------------------------------------------------------
    // v1.34/FR-01-96 ⑦ ConnPhase 活跃口径（对齐 ezr-demo 重构版）
    // -----------------------------------------------------------------------

    fn conn(id: usize, start: u64, end: u64, done: u64) -> Connection {
        Connection {
            id,
            start,
            end,
            done,
        }
    }

    /// 待命连接（cap = 0，块队列耗尽后存续至任务收尾）→ Closed，非活跃
    #[test]
    fn conn_phase_standby_is_closed() {
        let c = conn(1, 0, 0, 0);
        assert_eq!(c.conn_phase(0), ConnPhase::Closed);
        assert!(!c.conn_phase(0).is_active());
        // 防御：cap=0 但 done 异常非零（历史残留）仍 Closed（cap 权威）
        assert_eq!(c.conn_phase(100), ConnPhase::Closed);
    }

    /// 持块但本轮尚无落盘字节 → Connecting（建连/握手中；含续传恢复
    /// 已领块但请求在途——cum 按本轮清零重计）
    #[test]
    fn conn_phase_holding_block_without_bytes_is_connecting() {
        // 新建连接：领块后请求在途
        let c = conn(2, 0, 1_000_000, 0);
        assert_eq!(c.conn_phase(0), ConnPhase::Connecting);
        // 续传恢复：块内已有断点字节（done > 0）但本轮累计为 0 → 仍在建连
        let r = conn(3, 0, 1_000_000, 400_000);
        assert_eq!(r.conn_phase(0), ConnPhase::Connecting);
        // 活跃判定：Connecting 属活跃（明细显示、速度列 -）
        assert!(r.conn_phase(0).is_active());
    }

    /// 本轮已有落盘字节 → Connected；换块不清累计（FR-01-99 ②）持续 Connected；
    /// 传输停顿不降级（仍持块即仍在连接）
    #[test]
    fn conn_phase_with_bytes_is_connected_and_stable() {
        let c = conn(4, 0, 1_000_000, 100);
        assert_eq!(c.conn_phase(1), ConnPhase::Connected);
        assert!(c.conn_phase(1).is_active());
        // 停顿（窗口老化、速度归零）不降级：仍持块 + cum > 0 → Connected
        assert_eq!(c.conn_phase(1_000_000), ConnPhase::Connected);
    }

    /// is_active 单源三分支穷尽（活跃 = Connecting ∪ Connected，Closed 非活跃）
    #[test]
    fn conn_phase_is_active_exhaustive() {
        assert!(ConnPhase::Connecting.is_active());
        assert!(ConnPhase::Connected.is_active());
        assert!(!ConnPhase::Closed.is_active());
    }

    // -----------------------------------------------------------------------
    // v1.34/FR-01-116 总大小未知展示口径（Task::size_known / eta_secs）
    // -----------------------------------------------------------------------

    /// 不支持续传的 HTTP(S) 流式下载：下载期间大小未知，进入校验/后期
    /// 处理/完成态（流结束）才得知实际大小
    #[test]
    fn size_known_streaming_http_unknown_until_stream_ends() {
        let mut t = crate::model::sample_task();
        t.protocol = Protocol::Http;
        t.resumable = false;
        t.state = TaskState::Downloading;
        assert!(!t.size_known(), "流式下载期间大小未知");
        t.state = TaskState::Queued;
        assert!(!t.size_known(), "排队（重探窗口）同样未知");
        t.state = TaskState::Paused;
        assert!(!t.size_known(), "暂停未知");
        t.state = TaskState::Failed;
        assert!(!t.size_known(), "失败未知");
        // 流结束：校验/后期处理/完成态已知
        for st in [
            TaskState::Verifying,
            TaskState::PostProcessing,
            TaskState::Completed,
        ] {
            t.state = st;
            assert!(t.size_known(), "流结束态 {st:?} 大小已知");
        }
    }

    /// 支持续传的 HTTP(S) 与 BT 恒已知；ETA 未知大小时恒 None
    #[test]
    fn size_known_resumable_and_bt_always_known_and_eta_gated() {
        let mut t = crate::model::sample_task();
        t.protocol = Protocol::Http;
        t.resumable = true;
        t.state = TaskState::Downloading;
        assert!(t.size_known());
        t.protocol = Protocol::Bt;
        t.resumable = false;
        assert!(t.size_known(), "BT 恒已知");
        // ETA 门槛：未知大小 → None（即便速度 > 0 且下载中）
        t.protocol = Protocol::Http;
        t.resumable = true;
        t.state = TaskState::Downloading;
        t.total = 3_000_000;
        t.downloaded = 1_000_000;
        t.speed = 1_000_000.0;
        assert!(t.eta_secs().is_some(), "已知大小时 ETA 正常");
        t.resumable = false;
        assert!(t.eta_secs().is_none(), "未知大小时 ETA 无从估计");
    }
}
