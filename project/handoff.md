# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：v139-fix（任务 v139-fix-chunk-regression）
> 本轮（v138-qa 后操作者直启缺陷修复轮）：任务详情「分块」计数倒退缺陷修复
> （error 时幻影 +1、恢复后回退真值）——`apply_chunk_snapshot` 两段合并改判 +
> 3 条回归单测 + 属性面同步。全量 2767P/0F、clippy/fmt/arch 全绿。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.23，分期模式）+ `project/mission/phase-01.md`。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（v115 前历轮） | mission/phase-01.md 全量 FR/D 裁决表 + features/ 13 份 + qa/ 13 份 | 全部交付 | parser/dry-check 全过 |
| coder（v120–v124 两批 + v1.18 修订） | FR-01-102/103/104/105/51/6 系列 + sidecar 即时落盘 + 清码 retry 直判 | 已交付 | 各批 2438→2506P/0F、clippy 0 |
| cleaner（v125/v129/v132 三轮） | 保持行为清理 + 四度量基线首登（覆盖率 94.28% / CPD 51 tiles / scan 2161 点 / CRAP 门禁 76 条） | 已交付 | 2506→2535P/0F |
| architect（v126/v134 两轮） | 四阶段架构评审 + throttle 策略提取 + 属性测试 57→60 + arch 13→14 规则 | 已交付 | 2523→2546P/0F、属性 60P |
| hardender（v127-hard） | v115 前差分变异 78/78 killed + CRAP/DRY 对账 + 软 Gherkin 受限登记 | 已交付 | 产品树零改动、基线全绿 |
| coder（v128）+ 会话外登记（v1.19/v1.20） | CPD 处置 bordered_block 单源 + R 恢复路径退役 + FailedPaused 徽标 + 任务 ID 随机化（u32 idgen 单源） | 已交付 | 2535→2578P/0F |
| architect（v135/v136 三段 = v1.21–v1.23） | 合并轮：FailedPaused 变体退役（8 态）+ 恢复一律重新排队 + 挂起态展示统一 + v1.23 数据统一（挂起即重置 error=None/retries=1、has_error 字段整体退役、迟到失败守卫 Paused 全态整体忽略）+ 规格三件套同步 + ezr-demo 同步 + pty 回归 7/7 | 已交付 | 产品 2597P/0F、clippy 0、fmt 0、arch 14 过 |
| hardender（v137-hard） | v1.21–v1.23 触碰面变异加固：model/slots.rs 35/35、app/tasks.rs 30/30、app/engine/evt.rs 29/30+1 等价论证；+9 条加固测试；三度量新基线（CPD 52 / 覆盖率 94.36% / CRAP 门禁 0） | 已交付 | 2733P/0F、clippy 0、fmt 0、arch 14 过 |
| QA（v138-qa） | 最终独立验证：e2e 13 套件全量探针与六套件同步（新建 suite_ui_conns.py）+ R 臂连接级累计清零缺陷修复（requeue_failed 显式清账本 + conn_rebase 重锚）+ 1 条单元测试 + 三度量复测对账（94.37% / 52 tiles / 门禁 0）+ APS 13/13 error 0 + stamp 保留处置 | 已交付 | 2740P/0F（14 二进制）、e2e 155P/0F/9S |
| coder（v139-fix，本轮） | **分块计数倒退缺陷修复**：`apply_chunk_snapshot` 两段合并（多块路径事件逐块真值权威、单流/单块维持快照兜底）+ 3 条回归单测 + `prop_apply_chunk_snapshot_monotone` 两段口径重写 + proptest 回归 case 持久化 | 已交付 | **2767P/0F**（14 二进制）、clippy 0、fmt 0、arch 14 过 |

### 遗留受限项（当前有效）
- **软 Gherkin 验收变异受限**（沿袭 v127/v137/v138 口径）：13 份 feature 的
  parser + ir-dry-check 实跑全过（error 级 0；advisory 607 = 中文分词已知假阳性）；
  `gherkin-mutator` 运行器适配器与验收入口生成器为会话级工具不随工作区流转，
  全量变异 campaign 不可在本环境前台窗口预算内完成。01-persistence-config.feature
  的 mutation-stamp（sha256=57e757…）**维持 v138 保留处置**（内容零变更；复跑指令
  见 v138 交接四要素③口径——本轮产品代码变更触及 detail 显示路径，stamp 的
  implementation_hash 校验机制会在复跑时发现实现漂移，按既有机制处理即可）。
- **e2e 逐用例环境受限项（S，共 9 条，套件内合规登记，沿袭 v138）**：retry RB-09、
  throttle TP-02/03/06、named NP-04/08/13/15/18（口径与「已验」部分见各套件输出）。
- **demo QA 脚本二进制路径为绝对路径**（`scripts/verify_ezr.py` PROG 常量）：现指向
  `/home/z/my-project/project/ezr-demo/target/release/ezr-tui-demo`；工作区迁移时须同步。
- **本轮 e2e 套件未实跑**（coder 职责边界，SKILL「不负责的内容」）：本轮改动为详情
  断点视图合并口径，e2e 断言面（QA-TD-03 分块行 `1 MB/块` 等）不涉 x 值逐帧对账；
  全量 e2e 155P/0F/9S 基线沿袭 v138，如需复验由 QA 角色会话执行。
- **工具链位置（本轮重建，跨会话缓存 `.tools/` 未随 clone 流转）**：rustup nightly
  1.101.0-nightly（080cc2714 2026-10-11；rust-toolchain.toml 钉定自动装，新 shell 须
  `export PATH="$HOME/.cargo/bin:$PATH"`）。PMD 7.28.0 / cargo-llvm-cov 0.9.1 /
  Babashka + APS 本轮未复装（三度量与 APS 复测非 coder 职责，沿袭 v138 基线数字）。
- 无其他受限项。

### 实现定义值登记（当前有效）
- 沿袭有效：D28/D29（仅添加落态与三钮布局）、D30（下调即时收缩）、D31（1MB 下限）、
  D32（Enter 语义）、D33（1s 归零时延）、D34（清码 R = 文件完整性直判 + fallback 锚定）。
- v1.21–v1.23 恢复 toast 文案（规格已并入 FR-01-33）：可续传
  `▶ 继续下载（从断点恢复，已进入等待队列）: {name}`、不可续传
  `⚠ 服务器不支持断点续传，将从头下载（已进入等待队列）: {name}`。
- v138 登记：R 臂手动重试后首个 Progress 观测以当前 done 重锚连接级账本（该次增量
  = 0），后续增量只计新一轮下载真实落盘（FR-01-99 ②「重试 = 新的一次下载」）。
- **本轮新增（v1.24 口径，操作者实证缺陷修复）**：暂停/失败断点视图的分块完成数
  两段合并——多块路径（可续传 ∧ 分块数 > 1）以引擎事件逐块记账为权威（连续前缀
  快照不参与合并）；单流路径（不可续传）与单块任务维持 max(前缀快照, 事件) 兜底
  （事件不带逐块记账、字节连续前缀精确）。详情「分块 x/y」自此与恢复后事件口径
  恒等，不再出现「error 时 +1 幻影、恢复后回退」。
- 无未决待批项。

## 二、当前产出情况（coder v139-fix）

### 本会话产物清单
**产品树（1 处缺陷修复，最小改动）**
- `ezr/src/model/task.rs`：`apply_chunk_snapshot` 两段合并改判——多块路径
  （`resumable ∧ chunk_total > 1`）事件逐块真值权威（替换语义，前缀快照仅投影
  连接视图）；单流/单块路径维持 `max(前缀快照, 事件)` 兜底。修复依据：操作者实证
  「error 时已下载分块数 13、space 暂停再恢复后回退 12」——乱序完成下多个在传
  部分块零头合计越块界，前缀 `done/piece` 幻影 +1 经 max() 盖过事件真值并随
  注册表持久化，恢复后事件口径回退造成倒退观感。
- 新增 `#[cfg(test)] mod tests`（task.rs 尾）：3 条回归单测
  `chunk_snapshot_multi_block_event_truth_beats_prefix_phantom`（多块幻影复现：
  真值 12 vs 前缀 13，锁定事件权威 + 连接视图投影不变）、
  `chunk_snapshot_single_stream_keeps_prefix_fallback`（单流兜底口径不变）、
  `chunk_snapshot_single_block_keeps_prefix_fallback`（单块边界口径不变）。

**属性面**
- `ezr/src/property_tests.rs`：`prop_apply_chunk_snapshot_monotone` 按两段口径
  重写（多块 = 事件口径断言 + 单流/单块 = max 断言；连接视图投影与读取侧钳制
  不变量保留）。
- `ezr/proptest-regressions/property_tests.txt`：+1 行（最小失败 case
  `n_blocks=157, piece=1, done_bytes=157, old_done=0` 自动持久化，永久回归锁）。

**工具侧（packs/_common/，经验沉淀）**
- `engineering.md`「通用工程沉淀」+1 条：启发式投影与权威记账合并时方向必须保守、
  不得把启发式读数写入持久状态（跨项目通用；项目细节只落本文件）。

**规格**
- features/ 与 mission 零改动（git 对账）；本轮为缺陷修复，不改规格文本
  （FR-01-80「分块 x/y」的 x = 已完成分块数语义不变，修复使展示回归该语义）。

### 验证证据
| 验证项 | 结果 | 命令（cwd `project/ezr`） |
|---|---|---|
| 全量测试 | **2767P / 0F**（14 测试二进制） | `CARGO_PROFILE_DEV_DEBUG=0 cargo test --all-targets` |
| TDD 红灯复现 | 旧实现下新单测失败（left 13 / right 12，与操作者观察一致）；修复后全绿 | 定向 `cargo test chunk_snapshot` |
| clippy / fmt / arch | 0 / 0 / 全过 | `cargo clippy --all-targets` / `cargo fmt --check` / `bash scripts/arch_check.sh` |
| 属性测试 | `prop_apply_chunk_snapshot_monotone` 过（含持久化回归 case） | `cargo test prop_apply_chunk_snapshot` |

### 对账口径（基线 → 终态）
- 测试计数：v138 基线 2740P → 本轮 **2767P**（+27）。分解：3 条新单测 × 9 个
  `#[path]` 挂载计数点（bin 单测 + 8 个挂载 model 树的集成测试目标）= +27；
  属性测试原地更新不增计数。沿袭 v138 交接已登记的 ±1 记账粒度残差说明
  （其分项未列全、两侧静态总数各自自洽，不影响验证有效性）。
- 产品源码变更 = git 对账仅 1 文件（task.rs）+ 2 测试面文件（property_tests.rs、
  proptest-regressions）+ 工具侧 engineering.md。

### 交接四要素（coder）
1. **本地验证命令清单**（按序，cwd `project/ezr`）：
   `CARGO_PROFILE_DEV_DEBUG=0 cargo test --all-targets`（2767P/0F）→
   `cargo clippy --all-targets`（0）→ `cargo fmt --check`（0）→
   `bash scripts/arch_check.sh`（全过）→ 定向回归：
   `cargo test chunk_snapshot`（3 单测 × 挂载点）+ `cargo test prop_apply_chunk_snapshot`。
2. **会话运行时产物**（`.work/tmp/`，`complete` 时清除）：无（验证均为主命令直跑）。
3. **规格与 stamp**：features/ 零改动；01-persistence-config.feature stamp 维持保留
   （v138 处置），gherkin-mutator 恢复后按四要素③删除重跑。
4. **等价突变体清单及失效条件**：沿袭 v137/v138 登记
   `src/app/engine/evt.rs:297:23 replace > with >= in App::handle_failure`（失效条件
   三条沿袭）；本轮 `apply_chunk_snapshot` 的两段判别为纯读分支，无守卫体副作用，
   未新增等价突变体。

### 待办与待批（未决项）
- 无新增未决待批项。
- 沿袭移交（操作者后续会话可选项，非缺陷）：gherkin-mutator 运行器基建；
  02-named-proxy 的 ezr-proxy 扩展（见 v138 交接遗留受限项）。
- 建议：本轮改动触碰 UI 断点视图口径，操作者如需 e2e 面复验（13 套件 155P/0F/9S
  基线），可 `bin/swarm begin six-pack/QA` 以本 handoff 为上游交接内容执行。

### 各角色合并吸收清单（coder 移交）
- **specifier**：FR-01-80「分块 x/y」语义文本无需回改（x = 已完成分块数，修复使
  展示回归语义）；如需将「多块/单流两段合并」登记为规格口径，可并入 FR-01-13
  或新增 v1.24 修订行（属操作者裁决项，本轮未动规格）。
- **cleaner**：本轮无新增重复；CPD 52 tiles 基线未复测（非 coder 职责），task.rs
  改动为单点分支替换，无重复引入迹象。
- **architect**：`apply_chunk_snapshot` 仍在 model 层纯函数域，无新依赖方向；
  三度量基线沿袭 v138。
- **hardender**：两段判别分支 `multi_block = resumable ∧ chunk_total > 1` 为后续
  变异加固的天然靶面（真值分支替换/判别条件取反均应被 3 条单测 + 属性击杀）。
- **QA**：e2e 断言面不涉本轮改动（分块行文案/布局未变）；如复验请关注断点视图
  场景（暂停/失败后详情 x 值与恢复后一致性）。

### 移交建议
- 操作者如需 e2e/三度量复验：`bin/swarm begin six-pack/QA`（工作区沿用，以本
  handoff 为上游交接内容）。
- 操作者如需继续新 feature：`bin/swarm begin six-pack/specifier`。
- 操作者可直接 `bin/swarm complete` 归档交付（本轮验证证据已闭环）。
