# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-p2-coder-bug10（2026-10-30，
> 「多 peer 下载降速设计问题 + 并发 panel IP 列显示」修复轮）
> 上游交接：project/handoff.md（bug9 前轮交接：大种子热路径/速度同源/参数对齐，
> 6206P/0F）+ 操作者 chat 当轮指令（任务依据＝操作者当次明确指令）：
> ①ezr bt 局域网下载单 seeder 峰值 ~2.6MB/s，再添加一个 peer 反而降速，检查设计
> 问题（参考 qBittorrent 下载参数与源码）；②并发 panel 'ip' 列名改 '地址'，明细
> 内容从 'ip 缩写' 改 'ip 缩写:port'；③修复后 harness 完整测试、bin/swarm complete、
> 产物复制 /home/z/my-project/；④（环境项）找最快镜像前台装 rustup。
> 上轮 rustup 沿袭说明：环境不跨会话持久，本轮沙箱工具链缺位，已按④以 TUNA 镜像
> 前台重装 nightly 1.101.0（选型实测见 notes/environment.md，cargo registry 仍配
> rsproxy sparse）。

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
| coder（phase-02 及历次收口轮） | `project/ezr/` 全量 | 已交付 | 前轮 5866P/0F；clippy 0；互操作 e2e PASS |
| coder（bug5…bug8 各修复轮） | 见前轮登记 | 已交付 | 6030/788/796/804 各轮 0F |
| coder（bug9：大种子热路径/速度同源/参数对齐轮） | 产品改动 ×7 文件 + 测试净 +8 | 已交付 | 6206P/0F；clippy 0；256MB 基准 123.8s |
| coder（本轮 bug10：上传离环/空闲置换/入连同 IP/地址列轮） | 产品改动 ×2 文件（bt/engine.rs + ui/conns.rs）+ harness 新增 bench_multipeer.py + README 增补 + 测试净 +4 | 已交付 | bin ezr **816P/0F** + hardening **5411P/0F** + 其余 bin **46P/0F**；clippy 0 警告；fmt 干净；harness 4MB PASS + 256MB 基准 **64.2s**（上轮 123.8s）+ 16KiB piece **69.6s**（上轮 129.8s）；多 peer 对照 5 场景全绿 |

### 遗留受限项（当前有效）
- **BT 下载限速未接**（沿袭：BtSpec.max_speed 仅对 HTTP 生效）；DHT/BEP 10 完整扩展/
  uTP 属 phase-03/04。
- **cargo-mutants 缺位**（加固轮职责）；demo 回归长脚本沙箱挂起（沿袭）；doc 测试不适用
  （bin-only）。
- **环境性抖动族**（既有登记项）：本轮全量未复现；处置依据 notes/testing.md 双测试二进
  制并发假 FAIL 判据。
- **死 tracker 的用户可见延迟**（沿袭待批项）：tracker 不可达时任务停留「下载中 0.0%」
  约 4 分钟才报「全部 tracker 失败」。
- **TUI 恐慌自愈的边界**（bug6 登记）：循环粒度自愈，零闪烁需恢复感知终端句柄。
- **BT partial 进度不持久化**（沿袭登记口径）：引擎磁盘 SHA-1 校验自愈（libtorrent
  同口径）。
- 源码注释零新增口径延续（既有注释保留）；挂载套件剥头副本随产品源同步——本轮改动
  文件 bt/engine.rs 有剥头对应项吗：无（engine.rs 未在剥头清单），conns.rs 同。
- **规格偏离登记延续**：FR-02-17「单 peer 在飞窗口 5 块」放宽为 `PEER_PIPELINE = 64`；
  FR-02-02「trackers = 空」改为预填。**本轮新增偏离（操作者指令级）**：
  `features/04-ui-conns.feature` L128 钉死「IP(11)/内容宽 59」按操作者指令改为
  「**地址(15)/内容宽 63**」，行内容「ip 缩写」→「ip 缩写:port」——feature 文件未改
  （specifier 职责），QA 复核以本登记为准。
- **设计决定登记（沿袭 + 本轮新增）**：
  - registry.json = 任务档案、.ezr = 数据断点；sidecar 版本恒 1、同形 v2 放行。
  - BT 分块三集合；BT 任务 connections 恒空、pieces() 走 BT 分支。
  - 单连接-per-peer（同 IP 去重 + 握手 peer_id 去重）；任务速度同源（Σ per-conn EMA）；
    选片缓存 PieceOrderCache；事件节流 250ms。
  - **上传服务离环（本轮定案）**：Request 服务＝预计算读盘计划（`upload_read_plan`）
    + `tokio::spawn` 任务（`read_upload_blocks` + 发送）；上传读一律独立 `File::open`
    （try_clone 共享 open file description 偏移，与引擎写句柄并发互踩）；进程级
    `UPLOAD_READ_PERMITS` Semaphore(128) 限并发读盘；发送阶段不持读盘资源，慢对端由
    out 通道背压限流。
  - **空闲置换 + 入连同 IP（本轮定案）**：`!seeding` 且连接 cum_down 60s
    （`IDLE_EVICTION_SECS`，libtorrent inactivity_timeout 同口径）无新增 → 引擎侧
    断开（conns.remove 先行 → Closed 路径不计 fails）→ 立即重拨同 IP 下一候选；
    入连 accept 走 `inbound_admitted`（同 IP 已有连接拒绝 + 容量上限）。

### 实现定义值登记
- 沿袭 phase-01/02 全部口径（D1–D59）+ bug5…bug9 轮补充（RECHOKE_INTERVAL_SECS=10、
  OPTIMISTIC_INTERVAL_SECS=30、UPLOAD_SLOTS=4、PEER_RETRY_BASE/CAP、REQUESTS_PER_PIECE=4、
  PEER_PIPELINE=64、TRACKER_NUMWANT=200、PROGRESS_EMIT_MIN_INTERVAL=250ms、
  CONNECT_TIMEOUT_SECS=3 等）。
- 本轮新增：`IDLE_EVICTION_SECS` = **60**、`UPLOAD_READ_CONCURRENCY` = **128**
  （static Semaphore）；**cum_up 计账口径变更**：serve 决策时计入 length（读盘失败
  才可能多计，量级可忽略；旧口径为读盘成功后计 buf.len()）。

## 二、当前产出情况

### 本会话产物清单
- **bug②（多 peer 降速）——两段式根因，均修复**：
  - **根因 A：上传服务内联在引擎循环**。受控实验（harness 新增
    `bench_multipeer.py`，seeder 限速 2.5MB/s 复现「单 seeder 2.6MB/s」口径）排除法
    定位：2 seeder 从头（4.68MB/s ✓）、late seeder 不同 IP（✓ 爬升 4.1–6.3MB/s）、
    16KiB piece（✓）均正常；**1 seeder + 1 并行 leecher 复现 0.77MB/s（公平份额
    1.25MB/s）**。中途屏幕快照 + leecher 实时状态证明：leecher 停止索要后 ezr 立即
    恢复满速 → 上传服务（读盘+发送）被 leecher Request 洪流（在飞可达 500）串行
    阻塞整个引擎。修复＝上传离环三件套（见设计决定登记）。
  - **根因 B：同 IP 候选去重无一选定终身盲区**。同机三方场景首连随机落在空 leecher
    时 seeder 被同 IP 去重**永久**挡住（ezr 曲线 124s 零进度、并发 panel 只挂空
    peer），直到 leecher 从 seeder 下完转做种才解锁。修复＝空闲 60s 置换 + 入连
    同 IP 拒绝（见设计决定登记）。
  - **qBittorrent/libtorrent 参数与源码对照结论**：下载侧瓶颈不在 ezr 的下载参数
    （PEER_PIPELINE=64/numwant=200 与 libtorrent 动态深窗/numwant 同维度，实验证明
    多 seeder 扩速正常）；差的是**服务侧结构**——libtorrent 的 aio 线程池（上传读盘
    不占网络循环）与 peer 置换机制（inactivity_timeout），本轮按此补齐。libtorrent
    的 max_out_request_queue(500)/choking_algorithm 属服务端参数，对 ezr 下载侧
    无对应缺口。
- **bug①（并发 panel 列显示）**：`ui/conns.rs` 表头 'IP'→'地址'；行内容 'ip 缩写'
  →'ip 缩写:port'（IPv4/IPv6 括号/裸 v6 三形态均带端口，无端口地址维持原样）；
  `BT_COLW[0]` 11→**15**（容纳 `255.*.255:65535`）、`BT_CONTENT_W` 59→**63**；
  detail.rs 无 per-conn IP 展示（无需同改）；qa/runners/suite_ui_conns.py 未断言
  该列（无需改）。
- **测试（净 +4）**：`request_serve_does_not_block_engine_loop_when_out_channel_full`
  （out 通道满 + 接收端存活不排水，handle_peer_msg 2s 内必须返回——旧内联实现红灯）；
  `request_serve_keeps_engine_handle_cursor_untouched`（serve 前后引擎句柄
  stream_position 不变——try_clone 共享偏移隐患回归，旧实现红灯 0→4096）；
  `idle_conn_without_progress_evicted_after_threshold`（零进度 60s 到置换点/有进账
  不置换/历史进账无新增到置换点）；`inbound_admitted_blocks_same_ip_and_cap`。
  既有 `request_handler_serves_only_when_not_choking` 适配 spawn 语义（recv 带超时）
  + 新增 cum_up 断言；`read_piece_range_multi_file...` 迁移为
  `upload_read_plan_multi_file_reads_only_requested_slice`（read_piece_range 删除，
  分段数学收口进 upload_read_plan）。
- **harness**：新增 `bench_multipeer.py`（N seeder + M 并行 leecher 对照基准：
  seeder 限速/late seeder 绑独立 IP/sidecar 采样速度曲线/中途屏幕快照），README 增
  用法与结果记录；`project/notes/` 三分类条目 + INDEX 同步。

### 验证证据
| 验证项 | 结果 | 命令（可复现，均在 project/ezr 下） |
|---|---|---|
| bin ezr | **816P / 0F**（基线 812 净 +4） | `CARGO_PROFILE_TEST_DEBUG=0 cargo test --bin ezr --quiet` |
| hardening 挂载套件 ×12 | **5411P / 0F**（133+627+719+601+615+2+16+749+632+186+389+742） | 逐二进制 `--quiet` 取 test result 行（二进制位于 target/debug/build/ezr/*/out/） |
| 其余 bin 目标 | 46P / 0F（ezr-fixture 30 + ezr-mktorrent 1 + ezr-proxy 15） | `cargo test --bin ezr-fixture --bin ezr-mktorrent --bin ezr-proxy --quiet` |
| clippy | 0 警告 | `cargo clippy --bin ezr --all-targets` |
| fmt | 干净 | `cargo fmt --check`（nightly 钉版） |
| harness 互操作 e2e | **PASS** | `project/harness`：`python3 swarm_e2e.py --ezr-bin ../ezr/target/release/ezr --payload-mb 4 --timeout 150` |
| harness 256MB 基准 | **PASS，64.2s（4.0MB/s）**——上轮 123.8s，近 2 倍 | 同上 `--payload-mb 256 --timeout 420`（标准编排带并行 leecher，上传离环修复直接受益） |
| harness 16KiB-piece 压测 | **PASS，69.6s**——上轮 129.8s | 同上 `--payload-mb 256 --piece-kib 16 --timeout 420` |
| 多 peer 对照 ①1 seeder | 2.39MB/s（修复前 2.42，无退化） | `python3 bench_multipeer.py --ezr-bin <release/ezr> --seeders 1 --payload-mb 128 --seeder-upload-kbps 2560 --workdir …` |
| 多 peer 对照 ②2 seeders | 4.68MB/s（正确扩速） | 同上 `--seeders 2` |
| 多 peer 对照 ③late seeder（127.0.0.2） | re-announce 后爬升 4.1–6.3MB/s | 同上 `--seeders 1 --late-seeders 1 --late-seeder-delay 15` |
| 多 peer 对照 ④1 seeder + 1 leecher | **0.77→1.16MB/s**（115.6s，60s 置换后满速；修复前 175.1s 且 124s 零进度） | 同上 `--seeders 1 --leechers 1` |
| 多 peer 对照 ⑤16KiB piece ×2 seeders | 4.63MB/s（正确扩速） | 同上 `--seeders 2 --piece-kib 16` |

### 对账口径
- 上轮 bin ezr 812 → 本轮 **816**（净 +4：新增 ×4，迁移改名 ×1 不计，既有断言增强
  ×1 不计）；hardening 上轮口径未单列分块计数，本轮 12 套件实跑 5411；其余 bin
  上轮 15 → 本轮 46（ezr-fixture 30 计入口径差异，实跑数字为准）。
- 全量套件在 `CARGO_PROFILE_TEST_DEBUG=0` 下编译运行（小盘 12×~200MB 必爆盘，
  9.9G 盘 os error 28 同族；断言/运行时行为不变，仅丢回溯符号，登记
  notes/testing.md）。
- release 构建 1m08s（增量）；256MB/16KiB 基准与 bug9 数字同 harness 同参数可比；
  多 peer 对照的 seeder 限速口径（2.5MB/s/个）用于复现操作者「单 seeder 2.6MB/s」
  的真实局域网带宽环境。
- 沙箱为全新环境：rustup 由 TUNA 镜像前台装 nightly 1.101.0（操作者指令④），
  cargo registry rsproxy sparse；镜像选型实测登记 notes/environment.md。

### 待办与待批
- **待批（沿袭）**：监听端口随机是否需固定范围/config 键；FR-02-17/FR-02-02 规格同步
  （本轮新增 04-ui-conns 列名/列宽偏离，见规格偏离登记）；死 tracker 约 4 分钟静默窗
  可视状态；做种任务重启不重挂引擎协程；多协议喂窗公共化重构；BT partial 不持久化。
- **待批（本轮）**：①`IDLE_EVICTION_SECS=60` 取值（libtorrent inactivity_timeout
  同维度默认；更短恢复更快但可能误伤排队等 unchoke 的慢 seeder，如需 30s 请裁定）；
  ②cum_up「serve 决策时计入」的口径（读盘失败才可能多计，如需精确请裁定回传机制）；
  ③并发 panel 「地址」列宽 15（`255.*.255:65535` 恰满，更长 IPv6 遮掩截断显示）。
- 遗留观察项沿袭：黑名单仅按 IP、无 UI 展示；BT 多文件「删除并清理文件」对目录无效；
  endgame 重复请求无去重上限；choker 速率窗口 10s 判别粒度；**本轮新增**：on_block
  的 SHA-1 校验 + 落盘仍内联在引擎循环（回环实测非瓶颈，49MB/s 全绿；真实慢盘场景
  与 libtorrent aio 口径的差距留给 hardender/架构评估，未夹带）。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。建议顺序：
  ①`bt/engine.rs` ~4.1k 行（本轮 +~120 行），上传服务（upload_read_plan/
  read_upload_blocks/UPLOAD_READ_PERMITS）与空闲置换（refresh_idle/idle_evict_due/
  inbound_admitted）可独立成模块评估；②`Evt::BtProgress` 载荷结构体化（沿袭）；
  ③同一 spec 构造在测试里重复 4 次（request_serve 三兄弟 + 既有 serve-gate），可提
  测试夹具。
- 给操作者的手测口径：①真实局域网：ezr 从单 seeder 下载应稳定在 seeder 服务能力
  量级；**在同一台机器再加一个客户端（同 IP）不再拖死下载**——最坏 60s 置换窗后
  恢复；②不同主机再加 seeder/peer，总速度应上升；③下载中观察并发 panel：列名
  「地址」、行显示 `ip 缩写:port`；④若有慢盘/大量做种请求场景，观察 ezr 自身下载
  是否仍被上传拖累（本轮修复目标，异常请回报）。
- 后续链：cleaner → architect → hardender → QA（宪法第一章：不自动流转，操作者
  显式续跑）。
