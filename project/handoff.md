# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-p2-coder（2026-10-28）
> 上游交接：project/handoff.md（ezr-p2-coder 前轮交接）
> 当轮任务：操作者 bug 报告——命令窗口输入 .torrent 文件路径后报「网络错误
> （builder error: relative URL without a base）」；BT 启动路径收口修复轮。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式）+ `project/mission/phase-01…04.md`；各期现行口径以
  对应分期文件为准。
- **phase-01（HTTP/HTTPS 下载内核与 TUI 正式版）已收官**：`project/ezr/` 全量源码、
  `project/features/` 与 `project/qa/` 期号 01 存量。
- **phase-02（BT 下载内核、做种上传与 BT 任务管理）已实现**：需求详述
  `mission/phase-02.md`（FR-02-01…23 + D44–D59）全部落地；规格 `features/06…09`
  四份（34 场景）对应实现与单测覆盖；QA 套件 `qa/06…09-*-qa.md` 待 QA 轮执行。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01） | features/ 期号 01 十份 + qa/ 期号 01 十份 | 已收官 | 收官基线 4564P/0F |
| specifier（phase-02） | features/06…09 四份 + qa/06…09 四份 | 全部交付 | 06/07/08 各场景 ↔ qa 逐例对应 |
| coder（phase-02） | `project/ezr/` phase-02 全量实现（BT 内核七模块、命令窗三分流、BT 对话框、BT 展示、ezr-mktorrent） | 已交付 | 交付时 cargo test 16 套件 5596P/0F；clippy 0 警告；fmt clean |
| coder（注释删除轮） | src/tests 123 个 .rs 全量剥离注释（行为零变更） | 已交付 | 三线复验全绿，5596P/0F |
| coder（tilde 修复轮） | 三处 .torrent 读取点接入 `~/` 展开（config::expand_tilde + 5 测试） | 已交付 | 5645P/0F；clippy 0 警告；fmt clean |
| coder（BT 启动收口轮，本轮） | tick 层协议分流 + `start_bt_from_slot` 单点启动 + `dlg_confirm_add_bt` 移除立即 StartBt + bt_route_tests 5 例 | 已交付 | 5685P/0F（16 套件）；clippy 0 警告；fmt clean |

### 遗留受限项（当前有效）
- **BT 下载限速未接**：BtSpec.max_speed 字段保留、本期引擎未接 throttle（phase-02
  规格未钦定 BT 限速口径；`max_speed` 仅对 HTTP 生效）。
- **peer 调度为贪心序**（无 rarity 统计）、choke 状态机最小集（interested/unchoke
  维护 + 合法 request 响应）——满足 D52 口径；DHT/BEP 10 完整扩展/uTP 属 phase-03/04。
- **cargo-mutants 缺位**（加固轮职责）；cargo-llvm-cov 未在本会话使用。
- **demo 回归长脚本沙箱挂起**（沿袭）：按场景子集分半跑。
- **doc 测试不适用**：包为 bin-only（无 lib target），`cargo test --doc` 报
  no library targets——与 phase-01 口径一致。
- 挂载测试 crate（hardening_*）经 `#[path]` 收编产品源：phase-02 新增 `mod bt` /
  `mod multiline` / `mod textfield` 挂载已补齐；`tests/gen/*_stripped.rs` 剥头副本
  （btn/chart/clipboard/detail/update_conn_stats 五份）与产品源逐字一致（gen-drift
  守卫通过，本轮改动文件无剥头副本，无需同步）。
- **源码注释已全量删除**（操作者指令，注释删除轮）：src/ 与 tests/ 无任何 // 与 /* */
  注释（含 /// doc 注释）；D 值/FR 对应关系以本文件「实现定义值登记」与
  mission/phase-02.md 为权威。恢复路径：带注释备份
  /home/z/my-project/ezr-src-with-comments-backup-20261028.tar.gz 或注释删除前归档
  swarm-20261028-035851（均不在本包内）。

### 实现定义值登记
- 累计维持 phase-01 收官口径（D1–D42）；phase-02 新增 D44–D59 已在
  `mission/phase-02.md` §3 定稿登记，实现无偏离。补充两项实现口径：
  - 引擎监听端口 = 127.0.0.1:0 随机绑定（announce port 携带；phase-02 规格未钦定
    固定端口与配置键——做种入向通道按随机端口实现）。
  - tracker 首轮 announce 延迟 2s（连接初期快速发现；协议无此要求，实现口径）。
- 本轮新增实现口径：BT 任务获槽即转 Downloading（BT 无探测流程，D44 分流②口径的
  自然延伸）；种子读取/解析失败为 Fatal 失败（不自动重试，错误文案
  「无法读取种子文件：<来源路径>」）。

## 二、当前产出情况

### 本会话产物清单
- **`src/app/engine/tick.rs`**：`tick_grant_and_start` 与 `send_retry_start` 按协议
  分流；新增 `start_bt_from_slot` 助手——BT 任务获槽/重试时读种子（经 expand_tilde）
  + 解析 + `make_bt_spec` + `Cmd::StartBt` + Queued→Downloading；读取/解析失败 →
  Failed + Fatal + 释放槽位 + 错误「无法读取种子文件：<路径>」+ toast。修复点：
  此前两处不分协议一律发 HTTP `Cmd::Start`，HTTP 引擎把本地 .torrent 路径当 URL
  （reqwest「builder error: relative URL without a base」→ 分类为「网络错误」）。
- **`src/app/bt_dialog.rs`**：`dlg_confirm_add_bt` 移除非暂停确认后的立即
  `Cmd::StartBt`（消除与槽位路径的双重启动）；添加统一入队等待槽位单点启动。
- **`src/property_tests.rs`**：`prop_throttle_bucket_step_invariants` 预算守恒断言由
  f64 精确相等改为相对容差 1e-9（proptest 随机种子命中极端量级暴露 1-ULP 舍入差；
  与本修复零交集，见 notes/testing.md）。
- **`src/engine/mod.rs`**：`EngineHandle::from_sender`（cfg(test) 测试构造器，供 spy
  断言引擎命令）。
- **测试**：`src/app/engine/tick.rs` 新增 `bt_route_tests` 5 例——BT 获槽发 StartBt
  且转 Downloading、HTTP 获槽保持 Start（对照）、种子缺失 Fatal 且零引擎命令、BT
  自动重试路由 StartBt、确认添加入队等槽后由槽位路径单点启动。
- **`project/notes/`**：新建项目经验库（architecture/testing/environment 三主题 +
  INDEX.md）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量测试（16 套件） | 5685P / 0F | `cargo test -- --test-threads=2`（project/ezr 下；长命令分块跑） |
| 其中：bin ezr 单测 | 747P / 0F | `cargo test --bin ezr -- --test-threads=2` |
| 其中：12 个 hardening 挂载套件 | 4892P / 0F | `cargo test --tests -- --test-threads=2`（hardening 131 + engine_backfill 650 + g_appcore 567 + g_engine 562 + g_main 2 + g_proxy 16 + g_slots 176 + g_supervisor 536 + g_throttle_err 349 + g_v147 550 + ui_backfill 680 + v115 673） |
| 其中：其余 bin 目标 | 46P / 0F | `cargo test --bin ezr-fixture --bin ezr-mktorrent --bin ezr-proxy`（30+1+15） |
| 其中：BT 路由聚焦 | 5P / 0F | `cargo test --bin ezr bt_route_tests` |
| clippy | 0 警告 | `cargo clippy --bins --tests` |
| fmt | clean | `cargo fmt --all --check` |
| 守恒属性测试复跑 | 3/3 绿 | `cargo test --bin ezr prop_throttle_bucket_step_invariants`（连跑 3 次） |
| 引擎下载 e2e | UDP tracker 发现 → 握手 → 4 piece 下载 → SHA-1 全过 → 产物逐字节一致 | `cargo test --bin ezr engine_downloads_via_udp_tracker_and_seeder`（含于 747） |
| 做种 e2e | leecher 拉块字节一致 + BEP 9 元数据 SHA-1==infohash + 上传统计真实增长 | `cargo test --bin ezr engine_seeds_to_leecher_with_metadata_exchange`（含于 747） |

### 对账口径
- 5645 → 5685（+40）：本轮新增 5 例（bt_route_tests）；bin ezr +5（742→747）；7 个
  `#[path]` 挂载套件随产品源收编各 +5（engine_backfill 645→650、g_appcore 562→567、
  g_engine 557→562、g_supervisor 531→536、g_v147 545→550、ui_backfill 675→680、
  v115 668→673），5 + 7×5 = 40 对账自洽；g_slots/g_main/g_proxy/g_throttle_err/
  hardening/其余 bin 目标不挂 tick.rs，计数不变。
- 全量并行复跑未复现 hardening_v147 500ms 时序抖动（前轮登记项，本轮 550P/0F 一次
  通过）；proptest 种子敏感性处置见 `notes/testing.md`。

### 待办与待批
- 无待批事项（本轮为 bug 修复轮，无规格偏离；无新增实现定义值争议）。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。建议顺序：
  ① `src/bt/engine.rs` 主循环复杂度审查（协程 select 面较宽，存在可提纯函数）；
  ② `src/app/engine/tick.rs` 获槽/重试两条启动路径与 `start_bt_from_slot` 的重复面
  （HTTP 分支的 spec 构造三行在两处重复，可提助手）；③ app/ui 层 demo 移植代码与
  phase-01 存量的重复面；④ 覆盖率基线（cargo-llvm-cov 0.9.1 已装）。
- 后续链：cleaner → architect → hardender → QA（宪法第一章：不自动流转，操作者
  显式续跑）。
