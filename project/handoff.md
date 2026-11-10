# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner（收敛轮已完成）——**下游 architect 待操作者显式启动**
> 路径：specifier → coder（2026-11-09 上午，ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 修订实现轮 ⇒ coder 补缺口轮（2026-11-10，seed_left 倒计时+归零臂）⇒ **cleaner 收敛轮（本轮，2026-11-10）** ⇒ architect → QA（待操作者显式 begin）
> 会话：six-pack-cleaner-20261110（任务 = coder 交接的两项：①新代码度量四件与保持行为清理；②ezr-demo 守卫 FAIL 修否裁决。上游交接 = project/handoff.md coder 补缺口轮版）
> 环境：nightly 1.102.0（rustup 经 rsproxy 直链前台装齐，18.8MB/s，清单 date 2026-11-10）+ cargo registry rsproxy sparse + bb 1.13.229 / APS / PMD 7.29.0 / llvm-cov 0.9.1 / mutants 27.1.0 装于 `.tools/` 与 `~/.local/bin`（工具缓存不进归档）；RAG 索引 `.work/rag` 已随产出刷新（164 文件）

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，v1.46）+ `mission/` 4 份 phase 详述（phase-02 v1.16 为当前实现依据）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder 历轮（含 ws/webrtc 轮 + bug28 轮） | ezr 产品实现 + 12 集成测试目标 + bin 单测；bug6–bug28 修复；FR-02-21/31/33/08 v1.15 + FR-02-34/35 全量实现；ezr-demo 同构同步 | 已交付 | bin 983P + 集成 12 目标 6761P 全绿（历轮口径） |
| cleaner 历轮 | 保持行为清理 + 四件度量 | 已交付 | 全绿 |
| architect（2026-11-07） | 四阶段评审 + arch_check 17 规则 + 属性测试 76P | 已交付 | bin 951P / 集成 6357P / CPD 181 |
| hardender | —（操作者 20261109 裁决定为 six-pack 可选节点，默认链不含） | 不适用 | — |
| QA + 终验（2026-11-06/07） | 14 套件 187P 实跑 + harness 自适应化 | 已交付 | 全绿 |
| 诊断轮（2026-11-09 下午） | mini_ws_tracker 修复 + Sintel 真实 swarm 129.3MB ×2 全成 + 三方冒烟 | 已交付 | ezr 源码零改动 |
| specifier（2026-11-09 修订轮） | phase-02 v1.16（FR-02-03/19 v1.16 + D73）+ mission.md v1.46 + features 2 份修订 + qa 2 份同步 | 已交付 | APS parser+dry-check 2/2 exit=0 |
| coder（2026-11-09 修订实现轮） | FR-02-03 v1.16 做种回退臂 + FR-02-19 v1.16 磁盘删除 + D73 全量实现；BtTaskData.upload_paused 跨重启持久载体；引擎 Reconfigure 文件面三缺口修复 + PieceOrderCache avail 重建；bin 单测 +11 | 已交付 | bin 994P/0F + 集成 12 目标 6848P/0F + clippy 0 警告 + fmt 干净 + APS 2/2 exit=0 |
| coder 补缺口轮（2026-11-10） | seed_left 倒计时递减 + 归零自动停止做种臂 + 空格停止臂共用落点 finish_seeding 提取 + bin 单测 +3 | 已交付 | bin 997P/0F + 集成 g_appcore 793P / g_engine 789P（各 +3 收编）+ clippy/fmt 干净 |
| **cleaner 收敛轮（本轮 2026-11-10）** | 度量四件（覆盖率/CRAP/DRY/变异点扫描）+ 保持行为清理 5 处单点化（详见 §二）+ ezr-demo 守卫修否裁决 = 否 | 已交付 | bin 997P/0F + 集成 12 目标 6869P/0F + clippy 0 警告 + fmt 干净 + APS 20/20 exit=0 + CPD 198（70 阈值） |

### 遗留受限项（当前有效）
- **leech 判据无 e2e 自动化**（six-pack 链长期受限项，历轮维持）。上传暂停（D73②）的 leecher 拉取面归入同族：闸门单测锁「request 不被服务、cum_up 零增长」可观察契约，leecher 端到端拉取验证待 QA/harness 轮。
- **变异实跑未执行**（hardender 可选节点；`--file` 分块口径悬置转政策性关闭）。本轮全仓变异点扫描 5994 点在案（详见 §二对账），hardender 全量变异前可按文件分块消费。
- **QA 规程 6 个 BT/命令窗口族无专属 runner**（harness/命令链路承载，历轮既定形态）。
- **CRAP 观察项**：`ezr_main` 与 `on_mouse_trackers` bin 套件 0 覆盖（承载面差异登记，历轮维持）。
- **ws(s) tracker announce 无代理面**——操作者 20261109 chat 裁决不补齐，长期已知边界关闭。
- **ezr-demo 守卫脚本 FAIL（本轮裁决 = 否，不修）**：影响面 = `ezr-demo/scripts/` 10 个 PTY 守卫（主守卫 75 FAIL、phase2 9 FAIL、popup integrity 0/5、trackers 3 FAIL、overlay 3、smoke 14、focus 1、scrollbar 4；close_btn_color 与 footer_pad_scroll 全绿）。证据链：demo 树 git-clean 于 HEAD（本轮零改动）+ 从源码全新构建复现。根因两类：①A 族（multiline）= 守卫清场退格连发被沙箱 PTY 投递上限吞没（环境预存，bug19 家族；缓解方案未回移植 demo 守卫），判据 = FAIL 消息携带残留字符；②主守卫/弹窗族 = 场景时间线与锚点钉历史轮次同步口径的基线漂移。修复需对 demo 守卫重定基线（行为口径权威裁决归操作者），超出 cleaner 本地可验证规模；产品面验证（bin/集成/APS）不受影响。移交 architect/操作者裁决是否立项守卫重定基线轮。
- **变更文件变异点 >100 两件**（登记移交，rust.md 口径不强拆）：`bt/engine.rs` 465 点（历轮沿袭）、`app/bt_dialog.rs` 119 点（本轮新登记）。hardender `--file` 分块兑现拆分收益，architect 先收模块边界。
- **CRAP 残余高 comp 登记（拆不动的本质复杂度，architect 定性法）**：`on_bt_done` comp=7（事件臂守卫链）、`handle_peer_msg`/Reconfigure 臂（引擎分发核心）、`dlg_confirm_modify_bt`/`make_bt_spec`（对话框路由）。其余本轮新代码 comp 全部 ≤6。
- **CPD 残余克隆家族**：198 群组（70 阈值）/9 群组（200 阈值），主体 = bt/engine.rs 测试字面量家族（309 处，历轮登记维持）+ supervisor 测试夹具（47 处）；新代码面本轮已清零（3 处消除）。
- **本轮实现缺口对账闭环**（specifier 轮登记的四缺口 + seed_left 补缺口，均已在 coder 轮落实并测试覆盖，仅保留对账记录）。

### 实现定义值登记（累计，当前有效口径）
- **D44–D73 沿历轮登记不变**（全量清单见 phase-01/phase-02 D 值表；D71 ws/webrtc 八项、D72 bug28 五项、D73 文件勾选修订轮八项均已实现并验证）。
- **D73 实现载体登记（coder 轮落地，本轮 cleaner 零改动、原样维持，规格锁可观察面、载体归实现定义——如对载体有异议请在 architect/QA 闸门提出）**：
  - D73①（回退判据与时机）：App 层确认链路内即时判 `selected_bytes_of_pieces(done_blocks) < new_total` → 状态即时翻「下载中」+ `downloaded = recalc`；不新增引擎事件、不等进度帧。
  - D73②（上传暂停）：单点闸门 = `handle_peer_msg` Request 臂 `allowed = !am_choking && !upload_paused`（闸门静默忽略）；载体 = `BtTaskData.upload_paused`（serde default，跨重启持久）+ Reconfigure 回退臂置位 + 引擎双完成臂与 App `on_bt_done` 双侧清除。
  - D73③（seed_left 重置）：`on_bt_done` 无条件 `seed_left = cfg.seed_duration_secs`。
  - D73④⑤⑥⑦（删除细则/toast/暂停同口径/目录等价）：`delete_deselected_files` 单一实现路径（本轮内部拆分 `push_ancestor_dirs` 提纯，行为与调用面不变）——文件粒度 remove_file + 祖先链自底向上 remove_dir（非空即停）+ NotFound/IO 失败双静默不阻塞；句柄先关后删。
  - 配套引擎缺口四件（done/partial 跨 Reconfigure 持久、请求泵主动泵动、PieceOrderCache.avail 重建、句柄关闭）维持 coder 轮落地原样。

## 二、当前产出情况

### 本会话产物清单（cleaner 收敛轮）
1. **`src/model/bt.rs`**：CRAP 收敛拆分 ×2（行为保持，公共 API 不变）——`delete_deselected_files` 提取 `push_ancestor_dirs`（祖先目录收集，comp 8→6）；`BtTaskData::selected_bytes_of_pieces` 提取关联函数 `span_sum_over_selected_files`（单 piece 交叠求和，comp 8→4/5）。
2. **`src/model/mod.rs` + `src/model/sidecar.rs`**：DRY 单点化——HTTP/BT 两侧 sidecar `remove` 同体 NotFound 容忍删除收编 `model::remove_missing_ok`，两侧方法改单行委托。
3. **`src/app/tasks.rs`**：DRY 单点化——暂停（Queued）/失败（Failed）两臂的「bt_stopping 登记 + spawn 引擎取消命令」重复块收编 `App::request_engine_cancel`（is_bt 参数化，入口差异留在调用侧）。
4. **`src/app/engine/tick.rs` + `src/app/testutil.rs`**：DRY 单点化——`bt_route_tests`/`seed_countdown_tests` 两兄弟测试模块的 `recv_cmd`/`assert_no_cmd` 重复测试基建收编 `crate::app::testutil`（pub(crate)，两模块改 use 引用）。
5. **`notes/`**：architecture/testing/environment 三文件追记 cleaner 收敛轮条目 + INDEX.md 索引同步。
6. **ezr-demo 守卫修否裁决登记**（见遗留受限项；本会话零 demo 代码改动）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| bin 全部单元测试 | 997P/0F（与基线精确一致） | `cargo test --bin ezr`（CARGO_INCREMENTAL=0 + CARGO_PROFILE_*_DEBUG=0 三件套，nightly 1.102.0） |
| 集成 12 目标（清理后复验） | 6869P/0F | 逐目标 `cargo test --test <t>`（hardening 247/engine_backfill 895/ui_backfill 925/v115 918/g_appcore 793/g_engine 789/g_main 2/g_proxy 16/g_slots 254/g_supervisor 762/g_throttle_err 492/g_v147 776） |
| clippy | 0 警告 | `cargo clippy --bins --tests` |
| fmt | 干净 | `cargo fmt --check` |
| 引擎勾回补下集成 | 全绿 2.19s | `cargo test --bin ezr -- reconfigure_deselect_deletes`（真 seeder 全链） |
| APS 验收流水线 | 20/20 feature parser + dry-check 全 exit=0 | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-parser <abs-feature> <out>.json && bb gherkin-ir-dry-checker <ir> <out>.dry.json)` |
| 覆盖率（llvm-cov 0.9.1） | 本轮新代码函数级 100%（handle_peer_msg 1249 命中/dlg_confirm_modify_bt 12 闭环/finish_seeding 双入口/tick_seed_countdown 56/on_bt_done/delete_deselected_files/selected_bytes_of_pieces 双侧） | `cargo llvm-cov --bin ezr --lcov --output-path .work/tmp/lcov.info` |
| CRAP 受限近似 | 新代码 comp 全部 ≤6（拆分后）；残余高 comp 登记（守卫链/分发核心） | 决策点+1 代 comp、cov=1 时 CRAP=comp（口径见 notes/testing.md 历轮条目） |
| DRY（PMD CPD 7.29） | 201→198 群组（70 阈值，本轮消除 3 处）；200 阈值 9 群组（历轮形态维持）；阳性对照命中预验 | `.tools/pmd/bin/pmd cpd -d src -l rust --minimum-tokens 70 -f csv` |
| 变异点扫描（cargo-mutants 27.1） | 全仓 5994 点；变更文件 bt/engine.rs 465、bt_dialog.rs 119（>100 登记移交） | `cargo mutants --list` |

### 对账口径
- 单测 bin：997 → 997（本轮零新增/零删除测试；清理为结构单点化，测试面不变）。
- 集成 12 目标：6869 → 6869（与 coder 补缺口轮口径精确一致；6848 + 3×7 挂载收编已在历轮对账）。
- CPD：201 → 198（-3：tick.rs 测试基建对、remove 语义重复、spawn-cancel 块；其余家族零变动）。
- mission/features/qa：本轮零改动（cleaner 只动实现与测试基建，规格与规程原样）。

### 待办与待批
- 无待批项（ezr-demo 守卫修否裁决按 cleaner 职权完成 = 否，证据链与影响面已登记遗留受限项；如操作者希望立项「demo 守卫重定基线轮」，请显式指令，建议由 specifier 先行确认各守卫应钉的行为口径版本）。
- **移交 architect**：①变异点 >100 两文件（bt/engine.rs 465、bt_dialog.rs 119）与 CRAP 残余高 comp 登记清单——hardender 全量变异前先收模块边界（历轮口径）；②coder 轮提请的「规格逐 When/Then 全量对账固化为 arch_check 第 18 条」提请仍待裁决（跨 cleaner 轮移交，原提请见 coder 轮 handoff §三，本轮原样转呈）。
- **移交 QA（下下游）**：QA-MB-10/11/12 与 QA-BE-08 可实跑（历轮口径不变）；上传暂停 leecher 拉取面可借 harness mini_ws_tracker/lt leecher 观察。

### 移交建议
- 下一步运行 **six-pack/architect**（以本文件为上游交接内容）；之后沿 six-pack 链 QA。
- 操作者可直接 `bin/swarm begin six-pack/architect`（以本文件为上游交接内容）。
