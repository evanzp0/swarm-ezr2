# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-p2-coder-bug9（2026-10-29，
> 「BT 局域网下载速度 <100KB/s / 只从一个 peer 下载 / 并发 panel 与任务列表速度不一致」
> 三 bug 修复轮）
> 上游交接：project/handoff.md（ezr-p2-coder-bug8 前轮交接：暂停继续竞态门闸 / 同 IP
> 两行复发 / 并发口径，804P/0F）
> 当轮任务：操作者三项指令——①ezr bt 局域网下载速度 100KB/s 不到（其他 BT 客户端
> 2MB/s，参考 qBittorrent 下载参数与源码）；②只从一个 peer 连接下载；③并发 panel
> 的该连接下载速度与任务列表下载速度不一致，上传速度两者也不一致。
> 修复后用 project/harness 完整测试，bin/swarm complete 打包，产物复制
> /home/z/my-project/。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式）+ `project/mission/phase-01…04.md`。
- **phase-01（HTTP/HTTPS 内核与 TUI）已收官**；**phase-02（BT 内核/做种/管理）已实现**
  并经互操作夹具多轮收口，详见既有登记。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01/02） | features/ 01 十份 + 06…09 四份；qa/ 对应十四份 | 已收官/已交付 | 见前轮登记 |
| coder（phase-02 及历次收口轮） | `project/ezr/` 全量 | 已交付 | 前轮 5866P/0F；clippy 0 警告；互操作 e2e 三连 PASS |
| coder（bug5：去重/还原/peer 策略/策略移植/tracker 轮） | 产品改动 ×9 文件 + 回归测试 ×18 | 已交付 | 6030P/0F；clippy 0 警告；fmt 干净 |
| coder（bug6：版本回退/total order panic/花屏轮） | 产品改动 ×3 文件；测试净 +1 | 已交付 | 788P/0F；clippy 0 警告；fmt 干净 |
| coder（bug7：速度闪烁/同 IP 两行/分块还原/连片轮） | 产品改动 ×8 文件 + 新增回归测试 ×8 | 已交付 | 全量 796P/0F；clippy 0；fmt 干净；harness e2e 两连 PASS |
| coder（bug8：速度/恢复竞态/同 IP 两行复发/并发口径轮） | 产品改动 ×8 文件 + 新增回归测试 ×8 | 已交付 | 全量 804P/0F；clippy 0；fmt 干净；harness 两连 PASS + 256MB 基准 ezr≈libtorrent |
| coder（本轮 bug9：大种子热路径/速度同源/参数对齐轮） | 产品改动 ×7 文件（bt/engine.rs + bt/mod.rs + engine/mod.rs + app/engine/{bt,evt,tick}.rs + harness/swarm_e2e.py）+ 测试改净 +8 | 已交付 | 全量 6206P/0F（bin ezr 812 + hardening 12 bin 5379 + 其余 bin 15）；clippy 0 警告；fmt 干净；harness 4MB PASS + 256MB 基准 123.8s + 16KiB-piece 新旧对照（旧 162.3s / 新 129.8s） |

### 遗留受限项（当前有效）
- **BT 下载限速未接**（沿袭：BtSpec.max_speed 仅对 HTTP 生效）；DHT/BEP 10 完整扩展/
  uTP 属 phase-03/04。
- **cargo-mutants 缺位**（加固轮职责）；demo 回归长脚本沙箱挂起（沿袭）；doc 测试不适用
  （bin-only）。
- **环境性抖动族**（既有登记项）：本轮全量未复现；处置依据 notes/testing.md 双测试二进
  制并发假 FAIL 判据，继续以「单跑复跑 + 整批复跑」口径处置。
- **死 tracker 的用户可见延迟**（沿袭待批项）：tracker 不可达时任务停留「下载中 0.0%」
  约 4 分钟才报「全部 tracker 失败」；是否加可视状态由操作者裁定。
- **TUI 恐慌自愈的边界**（bug6 登记）：被捕获恐慌后的重进 TUI 为循环粒度（≤100ms）；
  零闪烁需恢复感知终端句柄（涉及 TUI 架构，未夹带）。
- **BT partial 进度不持久化**（沿袭登记口径）：引擎 partial（已收块未验完）只存内存，
  暂停/重启后该部分退回未下载、由引擎磁盘 SHA-1 校验自愈重下——libtorrent 同口径
  （piece 粒度断点），如需块粒度断点请操作者裁定。
- 源码注释全量删除口径延续；挂载套件剥头副本随产品源同步（本轮改动文件无剥头对应项）。
- **规格偏离登记延续**：FR-02-17「单 peer 在飞窗口 5 块」放宽为 `PEER_PIPELINE = 64`
  （本轮 32→64，libtorrent request_queue_time=3s 动态深窗精神）；FR-02-02
  「trackers = 空（显示占位提示）」改为预填 config + .torrent trackers。feature
  文件未改（属 specifier 职责），QA 复核时以登记为准。
- **设计决定登记（沿袭 + 本轮新增）**：
  - **registry.json = 任务档案、.ezr = 数据断点**（bug5 定案）；sidecar 版本恒 1、同形
    v2 遗留文件放行（bug6/bug8 定案，旧全量档嵌套形状仍拒绝）。
  - **BT 分块三集合**（bug8 定案）：`done_blocks`/`partial_blocks`/`active_blocks`；
    BT 任务 `connections` 恒空、`pieces()` 走 BT 分支；`BtProgress` 携带三集合 +
    uploaded（**本轮删除 upload_speed 字段**）。
  - **单连接-per-peer**（bug8 定案，对齐 libtorrent）：`fill_peer_slots` 按 IP 去重；
    握手 peer_id 去重；ConnStat 记 peer_id。
  - **任务速度同源（本轮定案）**：BT 任务 `t.speed`/`t.upload_speed` = Σ per-conn EMA
    （`sample_bt_conn_speed_display` 汇总写入），与并发 panel 行完全同源；单连接时
    任务速度 == 该行速度（构造上相等）。引擎逐块碎片窗口求和与逐块 upload_speed=0
    旧口径废除；BT 任务不喂进度窗（windows 只属 HTTP）。
  - **选片缓存（本轮定案）**：`PieceOrderCache` 增量维护 peer 报有数（Bitfield/Have/
    Closed/Disconnect 维护），按（稀有度、每重建轮 jitter 种子）缓存排序，dirty 失效；
    逐块 refill 走 `refill_visit_order`（partial 按已收块数降序前置 + 缓存序）。
  - **事件节流（本轮定案）**：BtProgress/BtConns 逐块发送 250ms 节流
    （`PROGRESS_EMIT_MIN_INTERVAL`），tick 1s 强制发；进度/速度显示秒级粒度足够。

### 实现定义值登记
- 沿袭 phase-01/02 全部口径（D1–D59）+ bug5/bug6 轮补充（`RECHOKE_INTERVAL_SECS` = 10、
  `OPTIMISTIC_INTERVAL_SECS` = 30、`UPLOAD_SLOTS` = 4、`PEER_RETRY_BASE_SECS` (5) ×
  2^fails 封顶 `PEER_RETRY_CAP_SECS` (120)、`REQUESTS_PER_PIECE` = 4、per-tracker
  completed Notify 立即发送、BT_SIDECAR_VERSION = 1 等）。
- 本轮新增/变更：`PEER_PIPELINE` 32→**64**、tracker `TRACKER_NUMWANT` 50→**200**
  （libtorrent num_want 默认）、`PROGRESS_EMIT_MIN_INTERVAL` = **250ms**；
  `CONNECT_TIMEOUT_SECS` = 3（bug8 沿袭）。

## 二、当前产出情况

### 本会话产物清单
- **Bug①（局域网速度 <100KB/s）**：主根因 = 引擎**逐块 O(P)~O(P log P) 热路径**——
  `availability_map` 每块全量重建（O(Σ|has|)）、`issue_requests` 每块重建 wanted_all +
  全排序（O(P log P)）、逐块 `conn_views`（seeder 检查 O(P)/conn）、逐块 BtProgress/
  BtConns 事件克隆 done/partial/active 全集（O(P)×3）并由 app 全量替换。真实大种子
  （数万 pieces × 16KiB 块的高块率）下引擎任务 CPU 饱和 → socket 读取被饿 → TCP 窗口
  塌缩。loopback 基准用 256KiB piece（千级 pieces）测不出。修复三件套：①
  `PieceOrderCache` 增量 availability + 缓存选片序（dirty 失效：Bitfield/Have/连接
  关闭）；②逐块 refill 走 `refill_visit_order`（partial 前置 + 缓存序，O(小)）；
  ③逐块事件 250ms 节流。配套参数对齐 libtorrent（官方 reference-Settings 核对）：
  `PEER_PIPELINE` 32→64（request_queue_time=3s 动态深窗精神）、`numwant` 50→200。
  **Bug②（只从一个 peer 下载）**：连接管理/默认并发（20）/tracker 解析无「只连一个」
  的硬 bug；同 IP/同 peer_id 单连接为 libtorrent 同款正确口径。速度层根因同 Bug①——
  引擎饱和时全部连接的 refill 一并被拖慢（其余 peer 消息滞留队列），修复后多 peer
  窗口同享深管线；numwant 200 扩大候选来源。
- **Bug③（panel 行速度 vs 任务列表速度不一致，上传尤甚）**：三处口径分裂——①逐块
  BtProgress 硬编码 `upload_speed: 0.0`（下载中任务上传速度被反复清 0）；②tick 的
  任务上传速度 = 引擎 `conn_views` 碎片窗口和（窗口被每块到达重置，数值无意义）；
  ③任务下载速度 = verified-piece 粒度进度窗（低速时锯齿）。修复：`Evt::BtProgress`
  删 upload_speed 字段；BT 任务不喂进度窗；`sample_bt_conn_speed_display` 采样时把
  Σ per-conn EMA 写入任务 `t.speed`/`t.upload_speed`——任务速度与 panel 行**同源**，
  单连接构造上相等，多连接等于各行之和。
- **测试（净 +8）**：引擎 ×6（PieceOrderCache dirty/稀有度/全排列 total order、
  refill_visit_order partial 前置、progress_emit_due 节流闸、常量钉）；app ×4 新
  （BT 不喂进度窗、单连接任务速度==行速度、多连接任务速度==Σ行速度、做种上传速度
  同源）+ 旧断言改写 ×2（order_wanted 测试迁移到新结构、on_bt_progress 去参）。
  e2e 断言适配节流语义（等完成后的 tick 事件）。
- **harness**：`swarm_e2e.py` 加 `--piece-kib`（默认 256 不变）——速度回归压测小
  piece 场景的入口。
- **`project/notes/`**：architecture.md 改写三段管线条目（任务速度同源化）+ 新增逐块
  热路径三件套条目；testing.md 新增两条（节流后 e2e 断言口径、新旧二进制对照法）；
  environment.md 新增一条（--piece-kib 压测口径）；INDEX.md 同步。

### 验证证据
| 验证项 | 结果 | 命令（可复现，均在 project/ezr 下） |
|---|---|---|
| bin ezr | **812P / 0F**（含 property_tests） | `cargo test --bin ezr --quiet` |
| hardening 挂载套件 ×12 bin | **5379P / 0F** | `cargo test --tests --quiet`（分块：hardening 133 + g_engine 715 + engine_backfill 623 + g_supervisor 2 + g_v147 16 + g_main 597 + g_proxy 611 + ui_backfill 628 + g_appcore 186 + g_slots 385 + g_throttle_err 745 + v115 738） |
| 其余 bin 目标 | 15P / 0F | `cargo test --bin ezr-fixture --bin ezr-mktorrent --bin ezr-proxy --quiet` |
| clippy | 0 警告 | `cargo clippy --bin ezr --all-targets` |
| fmt | 干净 | `cargo fmt`（rust-toolchain.toml 钉 nightly） |
| harness 互操作 e2e | **PASS** | `project/harness`：`python3 swarm_e2e.py --ezr-bin ../ezr/target/release/ezr --payload-mb 4 --timeout 150`——libtorrent seeder+leecher 真实 swarm，ezr 2.2s 完成转做种，三方 SHA-256 一致 |
| harness 256MB 基准 | **PASS，ezr≈libtorrent** | 同上 `--payload-mb 256 --timeout 420`：ezr 123.8s（2.07MB/s），上轮 126.8s |
| harness 16KiB-piece 压力对照 | **新 129.8s vs 旧 162.3s** | 同上 `--payload-mb 256 --piece-kib 16`（16384 pieces）：旧实现 -31%（热路径开销），新实现贴 seeder 上限无塌缩 |

### 对账口径
- 上轮基线 804（bin ezr）+ 15（其余 bin）+ hardening 套件（上轮未单列，本轮实跑
  5379）。本轮 bin ezr 804→812（净 +8：引擎 ×5 新增 + app ×4 新增 − 旧
  order_wanted_large_input ×1，picker ×2 迁移改写实测并入）。
- 16KiB-piece 对照的「旧二进制」由 HEAD 源码独立 target-dir 构建得出（同 harness
  同参数），方法登记 notes/testing.md。
- release 构建耗时 3m01s；环境为新沙箱（rustup 由 rsproxy 本体 + TUNA dist 前台装
  nightly 1.101.0，镜像选型登记 notes/environment.md）。

### 待办与待批
- **待批（沿袭）**：监听端口随机是否需固定范围/config 键；FR-02-17/FR-02-02 规格同步；
  死 tracker 约 4 分钟静默窗可视状态；做种任务重启不重挂引擎协程（FR-02-22⑥）；
  多协议喂窗公共化重构；BT partial 不持久化（引擎校验自愈，libtorrent 同口径）。
- **待批（本轮）**：①逐块事件 250ms 节流——进度条/速度列刷新粒度从「每块」变为
  250ms/tick 1s（qBittorrent 同为秒级），如需更细粒度请裁定；②「任务速度 = Σ 行
  速度」口径下，BT 任务速度不再精确等于 verified 字节增速（piece 校验滞后的在途
  字节也计入），与 qBittorrent 显示口径一致，如需「落盘字节增速」口径请裁定；
  ③PEER_PIPELINE=64 与 numwant=200 取值（libtorrent 同维度默认为动态 3s×速率
  cap500 / 200）。
- 遗留观察项沿袭：黑名单仅按 IP、无 UI 展示；BT 多文件「删除并清理文件」对目录无效；
  endgame 重复请求无去重上限；choker 速率窗口 10s 判别粒度。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。建议顺序：
  ①`bt/engine.rs` ~3.8k 行（本轮 +~250 行），PieceOrderCache/refill 路径可独立成
  模块评估；②`refill_visit_order` 与 `order_wanted` 语义迁移后的死代码复查；③
  `Evt::BtProgress` 载荷结构体化（沿袭上轮建议）。
- 给操作者的手测口径：①真实局域网大种子（首选 piece 小/体积大）用 ezr 下载，速度
  应与 qBittorrent 同量级（本轮 loopback 16k pieces 压测已贴 seeder 上限）；②下载中
  观察并发 panel 与任务列表——单连接时两者下载/上传速度一致，多连接时任务速度 =
  各行之和；③上传速度不再闪烁归零；④progress 刷新为 250ms 粒度属预期。
- 后续链：cleaner → architect → hardender → QA（宪法第一章：不自动流转，操作者显式续跑）。
