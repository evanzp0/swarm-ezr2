# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect（收敛轮已完成）——**下游 QA 待操作者显式启动**
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 修订实现轮 ⇒ coder 补缺口轮（2026-11-10）⇒ cleaner 收敛轮（2026-11-10）⇒ **architect 收敛轮（本轮，2026-11-10）** ⇒ QA（待操作者显式 begin）
> 会话：six-pack-architect-20261110（任务 = cleaner 交接的架构评审与属性测试；上游交接 = project/handoff.md cleaner 收敛轮版）
> 环境：nightly 1.102.0（rustup 经 rsproxy 直链前台装齐，dist 清单 date 2026-11-10）+ cargo registry rsproxy sparse + bb 1.13.229 / APS / PMD 7.29.0 / llvm-cov 0.9.1 / mutants 27.1.0 装于 `.tools/` 与 `~/.local/bin`（工具缓存不进归档）；RAG 索引 `.work/rag` 已随产出刷新（200 文件，索引健康）

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，v1.46）+ `mission/` 4 份 phase 详述（phase-02 v1.16 为当前实现依据）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder 历轮（ws/webrtc + bug28 + 修订 + 补缺口轮） | ezr 产品实现 + 12 集成测试目标 + bin 单测；bug6–bug28 修复；FR-02 全量实现 + D71/D72/D73 落地；ezr-demo 同构同步 | 已交付 | bin 997P + 集成 12 目标 6869P 全绿（cleaner 轮口径） |
| cleaner 历轮 | 保持行为清理 + 度量四件（覆盖率/CRAP/DRY/变异点扫描） | 已交付 | 全绿；CPD 198（70 阈值） |
| architect（2026-11-07） | 四阶段评审 + arch_check 17 规则 + 属性测试 76P | 已交付 | bin 951P / 集成 6357P / CPD 181 |
| hardender | —（操作者 20261109 裁决定为可选节点，默认链不含） | 不适用 | — |
| QA + 终验（2026-11-06/07） | 14 套件 187P 实跑 + harness 自适应化 | 已交付 | 全绿 |
| 诊断轮（2026-11-09 下午） | mini_ws_tracker 修复 + Sintel 真实 swarm 129.3MB ×2 全成 + 三方冒烟 | 已交付 | ezr 源码零改动 |
| specifier 修订轮（2026-11-09） | phase-02 v1.16（FR-02-03/19 v1.16 + D73）+ mission.md v1.46 + features 2 份 + qa 2 份同步 | 已交付 | APS parser+dry-check 2/2 exit=0 |
| cleaner 收敛轮（2026-11-10） | CRAP 收敛拆分 ×2 + DRY 单点化 ×3 + demo 守卫修否裁决=否 | 已交付 | bin 997P/0F + 集成 6869P/0F + clippy 0 警告 + fmt 干净 + APS 20/20 exit=0 + CPD 198 |
| **architect 收敛轮（本轮 2026-11-10）** | engine.rs 8477 行目录化拆分（5 文件零下游改动）+ 属性测试 +6（76→82）+ arch_check 第 18 条裁决=否 | 已交付 | bin 1003P/0F + 集成 6869P/0F + clippy 0 警告 + fmt 干净 + arch_check 17 规则通过 + 变异点 5994 精确再分布 |

### 遗留受限项（当前有效）
- **leech 判据无 e2e 自动化**（six-pack 链长期受限项，历轮维持）。上传暂停（D73②）的 leecher 拉取面归入同族：闸门单测锁「request 不被服务、cum_up 零增长」可观察契约，leecher 端到端拉取验证待 QA/harness 轮。
- **变异实跑未执行**（hardender 可选节点；`--file` 分块口径悬置转政策性关闭）。全仓 5994 点在案；engine 拆分后 `--file` 分块消费面就绪（五文件 20/106/78/209/52，全部 <250/文件）。
- **QA 规程 6 个 BT/命令窗口族无专属 runner**（harness/命令链路承载，历轮既定形态）。
- **CRAP 观察项**：`ezr_main` 与 `on_mouse_trackers` bin 套件 0 覆盖（承载面差异登记，历轮维持）。
- **ws(s) tracker announce 无代理面**——操作者 20261109 chat 裁决不补齐，长期已知边界关闭。
- **ezr-demo 守卫 FAIL（cleaner 轮裁决=否，architect 轮维持不立项重定基线）**：影响面 = `ezr-demo/scripts/` 10 个 PTY 守卫（根因两类：A 族 = 守卫清场退格连发被沙箱 PTY 投递上限吞没，环境预存；主守卫/弹窗族 = 场景时间线与锚点基线漂移）。行为口径权威裁决归操作者；产品面验证（bin/集成/APS）不受影响。如需立项守卫重定基线轮请显式指令，建议由 specifier 先行钉各守卫行为口径版本。
- **CRAP 残余高 comp 登记（拆不动的本质复杂度，architect 定性法）**：`on_bt_done` comp=7（事件臂守卫链）、`handle_peer_msg`/Reconfigure 臂（引擎分发核心）、`dlg_confirm_modify_bt`/`make_bt_spec`（对话框路由）——本轮拆分不触及，口径维持。
- **CPD 残余克隆家族**：198 群组（70 阈值）/9 群组（200 阈值），主体 = engine 测试字面量家族（309 处，随拆分分布于 engine/*.rs，未消）+ supervisor 测试夹具（47 处）。
- **bt_dialog.rs 变异点 119（>100 登记维持）**：可见性耦合 + 阈值边缘不强拆（rust.md 口径）；engine.rs 465 点同族登记已随本轮拆分结案。
- **arch_check 第 18 条（规格逐 When/Then 对账固化）裁决=否**（本轮按职权结案，判据见 notes/architecture.md；登记备查）。

### 实现定义值登记（累计，当前有效口径）
- **D44–D73 沿历轮登记不变**（全量清单见 phase-01/phase-02 D 值表；D71 ws/webrtc 八项、D72 bug28 五项、D73 文件勾选修订轮八项均已实现并验证）。
- **D73 实现载体登记（coder 轮落地，cleaner 轮确认、本轮零改动原样维持；规格锁可观察面、载体归实现定义——如对载体有异议请在 QA 闸门提出）**：
  - D73①（回退判据与时机）：App 层确认链路内即时判 `selected_bytes_of_pieces(done_blocks) < new_total` → 状态即时翻「下载中」+ `downloaded = recalc`；不新增引擎事件、不等进度帧。
  - D73②（上传暂停）：单点闸门 = `handle_peer_msg` Request 臂 `allowed = !am_choking && !upload_paused`（闸门静默忽略）；载体 = `BtTaskData.upload_paused`（serde default，跨重启持久）+ Reconfigure 回退臂置位 + 引擎双完成臂与 App `on_bt_done` 双侧清除。
  - D73③（seed_left 重置）：`on_bt_done` 无条件 `seed_left = cfg.seed_duration_secs`。
  - D73④⑤⑥⑦（删除细则/toast/暂停同口径/目录等价）：`delete_deselected_files` 单一实现路径（文件粒度 remove_file + 祖先链自底向上 remove_dir 非空即停 + NotFound/IO 失败双静默不阻塞；句柄先关后删）。
  - 配套引擎缺口四件（done/partial 跨 Reconfigure 持久、请求泵主动泵动、PieceOrderCache.avail 重建、句柄关闭）维持 coder 轮落地原样。

## 二、当前产出情况

### 本会话产物清单（architect 收敛轮）
1. **`src/bt/engine.rs` → `src/bt/engine/` 目录化拆分**：mod 6260（编排核心 + 测试主体）/pieceio 754/trackpool 738/peerwire 468/conn 358；私有子模块 + `use *` 收编 = 公共 API 与下游 use 面零改动（行为保持）；变异点 465 精确再分布 20/106/78/209/52。
2. **`src/property_tests.rs`**：+6 属性（76→82）——tracker URL scheme 分类、hex_lower 往返、bin_string latin-1 双射、id20 双形态、peer_id 会话稳定、滚动条 Bar 几何单调（bt 层纯函数覆盖缺口评估后选定）。
3. **`notes/`**：architecture/testing/environment 三文件追记 architect 轮条目 + INDEX.md 索引同步。
4. **arch_check 第 18 条裁决=否**：arch_check.sh 保持 17 规则零改动。
5. **`project/handoff.md`**：重写为当前有效口径（本文件）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| bin 全部单元测试 | 1003P/0F（= 921 非属性 + 82 属性；非属性面与基线精确一致） | `cargo test --bin ezr`（CARGO_INCREMENTAL=0 + CARGO_PROFILE_*_DEBUG=0 三件套，nightly 1.102.0） |
| 属性测试（独立报告，对账隔离） | 82P/0F（+6 新增全绿） | `cargo test property_tests::` |
| 集成 12 目标 | 6869P/0F（时序校验有效：engine/ mtime 05:48 早于验证 05:55–06:25；被改 property_tests.rs 不在集成编译域） | 逐目标 `cargo test --test <t>`（分项 247+925+793+918+895+789+762+2+16+254+492+776 = 6869） |
| clippy | 0 警告 | `cargo clippy --bins --tests` |
| fmt | 干净 | `cargo fmt --check` |
| arch_check | 17 规则全过 | `bash scripts/arch_check.sh` |
| 变异点扫描（cargo-mutants 27.1） | 全仓 5994 不变；engine 465 → mod 20/conn 106/peerwire 78/pieceio 209/trackpool 52 精确再分布；bt_dialog 119 维持 | `cargo mutants --list` |

### 对账口径
- bin：997 → 1003（+6 全归属新增属性测试；非属性单测面 921 零变动——属性测试独立报告不计入单测基线对账）。
- 集成 12 目标：6869 → 6869（精确一致，拆分零下游改动的直接证据）。
- 变异点：5994 → 5994（engine.rs 465 → engine/ 五文件 465，搬运不增减）。
- mission/features/qa：零改动（architect 只动实现结构与测试基建）。

### 待办与待批
- 无待批项（arch_check 第 18 条按职权裁决=否；ezr-demo 守卫重定基线维持 cleaner 裁决不立项——如操作者希望立项请显式指令）。
- **移交 QA（下下游）**：QA-MB-10/11/12 与 QA-BE-08 可实跑（历轮口径不变）；上传暂停 leecher 拉取面可借 harness mini_ws_tracker/lt leecher 观察。
- **移交 hardender（若启动）**：engine 拆分后 `--file` 分块消费面就绪（五文件 20–209 点/文件）；bt_dialog.rs 119 点单文件块维持。

### 移交建议
- 下一步运行 **six-pack/QA**（以本文件为上游交接内容）。
- 操作者可直接 `bin/swarm begin six-pack/QA`（以本文件为上游交接内容）。
