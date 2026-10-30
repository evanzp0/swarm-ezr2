# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-coder-bug11（2026-10-30，
> 「config 三大标准表重构 + bt 并发 panel 列修正 + tracker url 即时生效」落地轮）
> 上游交接：project/handoff.md（bug11 specifier 轮交接：规格 ×9 修订 + qa ×6 修订，
> 规格确认门已获操作者 chat 批复「A」放行）+ 操作者当轮指令（任务依据＝已批规格）。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式）+ `project/mission/phase-01…04.md`。
- **phase-01（HTTP/HTTPS 内核与 TUI）已收官**；**phase-02（BT 内核/做种/管理）已实现**
  并经互操作夹具多轮收口。bug11 轮为操作者指令级需求修订（三大表重构 / panel 列修正 /
  tracker 即时生效），specifier 轮规格已批，本轮 coder 全量落地。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01/02 及历轮） | features/ 18 份 + qa/ 对应套件 | 已收官/已交付 | 见前轮登记 |
| coder（phase-02 及历次收口轮） | `project/ezr/` 全量 | 已交付 | bug10 轮 816P/0F；互操作 e2e PASS |
| **coder（本轮 bug11 落地）** | 产品 ×11 文件 + 测试适配 + runners 迁移 + ezr-demo 同步 | **已交付** | **cargo test 全目标 4162P/0F（13 个测试二进制）+ clippy --all-targets 零警告；harness swarm_e2e PASS（4MB 互操作，ezr↔libtorrent 双向做种对账 sha256 一致）+ bench_multipeer PASS；QA 冒烟：config-template 5/5、persistence-config 9/9、ui-conns 18/18、slots-queue 14/15（SQ-13 固有竞态，见受限项）；ezr-demo cargo test 30P/0F** |

### 遗留受限项（当前有效）
- **QA-SQ-13 判据固有竞态**（本轮登记）：R 重排队后「等待中」窗口可能短于 0.1s 轮询
  粒度（授槽 tick 对齐时整链 <100ms），套件断言瞬时中间态偶发假红；产品侧行为经单测
  与手工复验正确（toast「↻ 手动重试（等待中）」+ 重排队语义在）。判据口径建议 QA 轮
  改落 toast/终态等长驻可观察物（notes/testing.md 已登记处置法）。
- **BT 下载限速未接**（沿袭）；DHT/BEP 10 完整扩展/uTP 属 phase-03/04。
- **cargo-mutants 缺位**（加固轮职责）；死 tracker 约 4 分钟静默窗（沿袭待批项）；
  TUI 恐慌自愈边界（bug6）；BT partial 进度不持久化（沿袭）。
- **QA runners 尚有 9 套件未在本轮冒烟**（add-task/modify-task/named-proxy/retry-
  backoff/resume-sidecar/tui-display/download-engine/integrity-check/ui-pieces）——
  迁移已全量完成且编译通过、入口统一走命令窗口，但完整跑绿属 QA 轮职责。
- 环境性抖动族处置依据 notes/testing.md；APS dry-checker 缺位（沿袭受限项）。

### 设计决定登记（沿袭 + 本轮实现口径）
- 沿袭 bug11 轮 D60 全部裁决（不兼容旧格式/全套常量外置/平铺表内/表头左对齐/列宽
  极值口径/idle_eviction_secs=60/cum_up 现行口径）。
- **本轮实现定义值（coder 细化，规格框架内的实现选择）**：
  - `BtTunables`（17 键）随 `BtSpec` 进引擎；进程级量（dns_race_grace_ms→BuiltinResolver
    grace、upload_read_concurrency→EngineShared 信号量、throttle_burst_secs/sleep_ms→
    Throttle::with_params）挂 EngineShared，`EngineHandle::start(cfg)` 注入。模块级
    常量（bt/mod.rs 五常量、engine.rs 14 常量、download.rs 两常量、tracker.rs 两常量、
    dns.rs GRACE 生产用法）全部退役，默认值单点收敛 `config::DEFAULT_*`。
  - 并发键回退语义：0/负值 → 回退默认（QA-PC-04 bt.bt_concurrency=0→20 口径），
    正值越界 → 钳制 1–64（`clamp_u` 先 filter ≥1 再 clamp）。
  - 客户端列解析（peer_id.rs `client_display`）：Azureus 形态两位码 → 62 项全名映射
    （libtorrent identify_client 口径子集，AR→ArcticTorrent 与操作者极值样例同形），
    版本 = id[3..7] 逐位 digit 原值/字母 base-36，四段点分（极值 11 列，列宽 26 吻合）；
    非 Azureus 形态/映射外码 → `Unknown`。
  - tracker 热更新：`BtTaskCmd::UpdateTrackers` → 引擎 abort 旧池（AbortHandle）→
    `start_tracker_pool` 重建（初建/重建同路）→ fail_count 归零；app 侧仅
    `trackers_changed && !pool_updated && state∈{Downloading,Seeding}` 发热更新
    （纯 tracker 修改不重挂引擎；total/并发变化走既有重建路径自带新池，不双发）。
  - 旧格式清理：顶层旧键与旧 [[proxies]] 随 ConfigRaw 三表结构天然忽略（零警告），
    `max_speed` 兼容字符串/整数两形态（toml::Value 归一）。

### 实现定义值登记（累计，覆盖为最新全量）
- 沿袭 phase-01/02 全部口径（D1–D59）+ bug5…bug10 轮补充 + bug11 轮 D60 与 FR-02-24/25。
- 本轮新增键位缺省（与钦定模板 168 行逐键一致）：[common] download_slots=5；
  [http] block_size_http=1MB、http_concurrency=4、max_retries=5、auto_retry=true、
  backoff_initial=8.0、backoff_cap=60.0、progress_tick_ms=100、sidecar_flush_secs=2、
  throttle_burst_secs=0.5、throttle_sleep_ms=20、dns_race_grace_ms=1000；[bt] 22 键
  见模板（bt_concurrency=20、peer_pipeline=64、upload_slots=4、
  upload_read_concurrency=128、idle_eviction_secs=60、connect/handshake/request
  timeout=3/10/20s、requests_per_piece=4、rechoke/optimistic=10/30s、
  progress_emit_min_interval_ms=250、tracker_numwant=200、peer_retry_base/cap=5/120s、
  tracker_backoff_start/cap=8/60s、udp_base_timeout_secs=15、udp_retries=3、
  seed_duration_secs=1800）。统一回退：0/负/非法 → 代码常量默认；并发键越界钳 1–64。

## 二、当前产出情况

### 本会话产物清单
- **产品（project/ezr/src/）×11**：
  - `model/config.rs`：三表化全量重写——Config 36 键平铺结构 + ConfigRaw
    [common]/[http]/[bt] 三子表反序列化（旧格式顶层键/[[proxies]] 天然未知键忽略）+
    钦定模板 v1.11（168 行逐字节）+ 37 项单测（三表解析/回退钳制/模板契约/旧格式）。
  - `engine/mod.rs`：EngineShared 增 grace/upload_read_permits；base_client_builder
    (grace)、build_endpoint_client(grace)、Throttle::with_params、Cmd::BtUpdateTrackers
    路由。
  - `engine/throttle.rs`：bucket_step/Throttle 参数化（burst_secs/sleep_ms 注入）。
  - `engine/dns.rs`：BuiltinResolver grace 字段（try_system_with_grace）。
  - `engine/supervisor/mod.rs`：TaskSpec +progress_tick_ms/sidecar_flush_secs；BtSpec
    +tunables: BtTunables（17 键，Default=config DEFAULT_*）。
  - `engine/supervisor/download.rs`：PROGRESS_TICK/SIDECAR_FLUSH 改 spec 注入。
  - `bt/mod.rs`：五常量退役、BtTaskCmd::UpdateTrackers 新增。
  - `bt/peer_id.rs`：client_display（62 项全名映射 + base-36 版本解码 + Unknown）。
  - `bt/peer.rs`：handshake 超时参数化；`bt/tracker.rs`：announce_udp 超时/重试参数化。
  - `bt/engine.rs`：tunables 贯通（17 键全部替换模块常量）+ spawn_tracker 返回
    AbortHandle + start_tracker_pool + UpdateTrackers 池重建 + UPLOAD_READ_PERMITS
    改 EngineShared + client 字段全名化；新增 tracker 池热更新 e2e 测试（双 tracker
    日志对账：B 立即 announce 同 infohash、A 停止、任务不重挂）。
  - `app/bt_dialog.rs`：make_bt_spec 注入 tunables；dlg_confirm_modify_bt tracker
    变更检测与 Cmd::BtUpdateTrackers 分流（+spy 测试：不重挂、无 CancelBt/StartBt）；
    `app/cmd.rs`/`app/engine/bt.rs`：BT_DEFAULT_CONCURRENCY/SEED_DURATION_SECS 退役
    改 cfg；`app/engine/mod.rs`：make_spec 注入 HTTP 两键。
  - `ui/conns.rs`：BT_COLW [18,8,8,8,8,26,4]/内容宽 86、列序「累计下载」移「上传速度」
    后、表头全部左对齐（head_cell=pad_right）、地址数据 ip缩写/port 斜杠、明细行对齐
    维持现状（数据右对齐、地址列左对齐）；渲染级断言（表头左对齐几何/列序/极值宽度）。
- **QA runners（project/qa/runners/）×14**：58 处 write_config 三表化（标量键=表头+
  键值行、多键分表、proxies_toml→[[common.proxy]]、harness/swarm_e2e/bench_multipeer
  夹具 config 同步）；suite_config_template 三处断言同步（锚 v1.11/54→168 行/36 键
  抽验+三表分隔头）；**add-task 入口统一命令窗口**（harness 新增 open_add_dialog_via_cmd
  helper + add_task_via_dialog 与 5 套件 23 处直开点迁移 + DUMMY_URL 常量 + chip
  「A 命令」/空态文案锚点同步——phase-02 命令窗口落地的 runner 欠账一并清偿）。
- **ezr-demo（仓库根 ezr-demo/）**：Connection +port/addr_display（ip缩写/port）、
  BT_CLIENTS 全名/版本池（10 客户端）、并发面板七列改版（列宽 18/26、列序、表头
  左对齐、地址/客户端新格式）；权威文档 §5.4 列结构/规则/横向滚动宽度与头部修订注记
  同步 v1.35 口径。cargo test 30P/0F。
- **notes**：architecture.md +2（调优注入模式/tracker 热更新）、testing.md +2（交互
  变更同步 runner 帮助层/转场窗口按键竞态）、environment.md +1（rustup 镜像补记/USTC
  一站式/小盘增量缓存）、INDEX 同步。

### 验证证据
| 验证项 | 结果 | 说明 |
|---|---|---|
| cargo test（bin ezr） | 819P/0F | 含 config 37、conns 19、peer_id 8、bt_dialog 15、tracker 热更新 e2e |
| cargo test（全部集成目标 ×12） | 3343P/0F | hardening/hardening_g_*/gen 全绿，合计 4162P/0F |
| cargo clippy --all-targets | 零警告 | bin + tests + 集成目标全量 |
| harness swarm_e2e | PASS | 4MB payload：ezr 与 libtorrent leecher 双向对账 sha256 一致、HTTP+UDP tracker、ezr 转做种 |
| harness bench_multipeer | PASS | 8MB 单 seeder 2.1s，sha256 对账一致（sidecar 采样） |
| QA 冒烟（4 套件） | 46P/1F | config-template 5/5（模板逐字节）、persistence-config 9/9（点路径回退/钳制）、ui-conns 18/18（panel 改版端到端）、slots-queue 14/15（SQ-13 固有竞态，登记） |
| ezr-demo | 30P/0F + 编译零警告 | 面板改版/模型同步后全绿 |

### 对账口径
- 本轮为落地轮：产品代码净改动 ×11 文件（+runner ×14、demo ×3 + 文档 ×1）；测试
  计数从 bug10 的 816P/0F 增至 819P/0F（config 重写净 -11 旧 +48 新，conns/peer_id/
  bt_dialog/热更新 e2e 净增），集成目标合计 3343P。
- 钦定模板 168 行经 QA-CT-01 与生成文件 cmp 逐字节一致（权威机制沿 v1.10）。

### 待办与待批
- **待办（QA 轮）**：①其余 9 套件完整跑绿（迁移已就绪，注意 notes/testing.md 新两条
  口径）；②SQ-13 判据口径 refinement（建议改落 toast/终态断言）；③APS dry-checker
  补装补跑（沿袭）。
- **待批（沿袭）**：监听端口随机范围/死 tracker 静默窗可视状态/做种任务重启不重挂/
  多协议喂窗公共化/BT partial 不持久化。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。重点核对：
  ①config.rs 三表化后的重复（测试与生产间模板常量双份——钦定文本双份系契约锚点，
  属既有口径非重复）；②bt/engine.rs 大文件在 tunables 注入后的 helper 参数链可读性；
  ③QA runners 帮助层新 helper 的命名与摆放。
- 给操作者的手测口径（沿 specifier 轮清单，coder 已实现）：①首次启动生成三表模板；
  ②旧格式配置静默按默认生效；③BT 并发面板：表头左对齐、地址 `1.*.4/51400`、客户端
  `qBittorrent/2.5.0.0`、累计下载列在上传速度后；④M 修改 trackers 确认后任务不断、
  新 tracker 立即生效（引擎级 e2e 已验证）；⑤[bt] bt_concurrency=8 后新 BT 任务预填 8；
  ⑥[http]/[bt] 调优键非法值回退默认。
- 后续链：coder → cleaner → architect → hardender → QA（宪法第一章：不自动流转，
  操作者显式续跑）。
