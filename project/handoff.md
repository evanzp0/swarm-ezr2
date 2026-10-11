# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier 终态　会话：v131-spec-v119（任务 v131-spec-v119）
> 本轮（specifier 补账小轮）：以上游 coder v130-code-v119 交接为基线，补写
> phase-01.md 头部 **v1.19 修订注记**（R 恢复路径退役 + FailedPaused 徽标白），
> 头部版本口径 v1.18 → **v1.19** 上翻落账。纯 mission 文件补账：feature/QA/源码零改动，
> v130 终态验证口径（**2535P/0F**、clippy 0、fmt 0、APS dry-check error 级 0）不变。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.1，分期模式）+ `project/mission/phase-01.md`
  （**v1.19**——FR-01-92/D25 内嵌注记与头部 v1.19 注记已对账一致）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（v115 前历轮） | mission/phase-01.md 全量 FR/D 裁决表 + features/ 13 份 + qa/ 13 份 | 全部交付 | parser/dry-check 全过 |
| coder（v120–v122 三批） | FR-01-102/103/104/105 + FR-01-06/51 v1.17 双对话框粘贴与清码 | 已交付 | 各批 2438→2503P/0F、clippy 0 |
| specifier（v124-spec-batch5） | FR-01-51 v1.18 直判规格 + D34 注记 + QA-IC-12 v1.18 | 已交付 | parser + dry-check 过 |
| coder（v124-code-batch5） | 修订 A 终态 sidecar 即时落盘 + 修订 B 清码 retry 直判 | 已交付 | 2506P/0F、clippy 0、fmt 0、arch 12 过 |
| cleaner（v125-clean） | 保持行为清理 8 处 + 四度量复核 | 已交付 | 2506P/0F、CPD tiles 55 |
| architect（v126-arch） | 四阶段架构评审 + 页签计数访问器 + 属性测试 +10 + arch 13/14 规则 | 已交付 | 2523P/0F、arch 14 过、属性 57P/0F |
| hardender（v127-hard） | 差分变异 78/78 killed + CRAP/DRY 对账 + 软 Gherkin 受限登记 | 已交付 | 产品树零改动、基线全绿 |
| coder（v128-code-cpd-tile） | CPD 处置：`bordered_block(fg)` 单源 + 8 调用点收口 + gen-drift 夹具同步 | 已交付 | 2527P/0F、CPD 56→52 tiles |
| cleaner（v129-clean） | 保持行为清理 5 处 + Modify 对话框渲染补测 1 条 + 四度量复核 | 已交付 | 2531P/0F、CPD 51 tiles、scan 2161 点 |
| 会话外登记（操作者直判 v1.19，v130 会话并入正式账） | FR-01-92 R 恢复路径退役：retry() 收窄仅 Failed、挂起 toast「按空格恢复」、R 段测试反转四重断言、FailedPaused 枚举 doc 注记；规格三件套同步（phase-01 注记、01-slots-queue 场景 15、QA-SQ-15） | 已交付 | 2531P/0F（断言反转非增删） |
| 会话外登记（操作者直判 v1.19 续，v130 会话并入正式账） | FailedPaused 徽标色裁决①落账：STATE_COLORS 8→9 态（FailedPaused=White 相邻 Paused）+ 穷尽守卫测试 + 规格/QA 同步（01-tui-display 场景 05、QA-TD-05 七态）；ezr-demo 同步（TaskState/挂起臂/恢复臂/文档 v1.7） | 已交付 | 2535P/0F（+4=守卫测试×4 挂载）、demo clippy 过（8 存量警告） |
| coder（v130-code-v119） | v1.19 切片正式承接核对（零偏差）+ 挂起 toast 文案断言补强 + APS 验收流水线补跑 + handoff 重写并账 | 已交付 | **2535P/0F**、clippy 0、fmt 0、parser IR 15/15×2、dry-check error 级 0 |
| specifier（v131-spec-v119，本轮） | phase-01.md 头部 v1.19 修订注记补写（R 恢复路径退役 + FailedPaused 徽标白 + 规格/QA 引用）——v130 交接遗留待办闭环 | 已交付 | 注记与六处内嵌口径（FR-01-92/D25、场景 15/QA-SQ-15、场景 05/QA-TD-05）逐项对账一致；纯文档补账 +7 行，代码/feature/QA 零改动 |

### 遗留受限项（当前有效）
- **当前沙箱为全新环境，全套工具链待装**（前沙箱工具状态不随 git 归档流转：`.tools/` 在
  gitignore 排除列，`~/.cargo`、`~/.local/bin/bb`、rustup 均属机器本地）：Rust nightly 工具链
  （rustup minimal + rustfmt + clippy + llvm-tools-preview，新 shell 须
  `export PATH="$HOME/.cargo/bin:$PATH"`）、cargo-mutants 27.1.0、cargo-llvm-cov 0.9.1、
  rustfilt、PMD 7.28.0（`.tools/pmd-bin-7.28.0`）、Babashka 1.12.209（`~/.local/bin/bb`）、
  APS 仓库克隆（`.tools/Acceptance-Pipeline-Specification`）。**下轮任何构建/测试/变异/APS
  工作前先按 `packs/_common/notes/rust.md` 与 `packs/_common/notes/aps.md` 重装**。磁盘
  9.9G（余约 8G）：llvm-cov `--branch` 插桩全量重编译 target ≈6G，跑前先清主 `target/debug`。
- e2e 六套件滞后于规程（累计）：`suite_add_task.py`、`suite_ui_conns.py`、
  `suite_modify_task.py`、`suite_persistence_config.py`、`suite_tui_display.py`、
  `suite_integrity_check.py`——QA 会话须先同步全部受影响套件再跑 e2e。
  注：v1.19 改变可观测行为（挂起 toast 文案、FailedPaused 徽标白、R 收窄），
  **QA-TD-05/QA-SQ-15 已同步，六套件代码仍滞后**，套件同步前 e2e 不可信。
- **软 Gherkin 验收变异受限**（沿袭 v127 登记）：13 份 feature 中仅
  `01-persistence-config.feature` 有历史 stamp（`# mutation-stamp: sha256=…` 注释块，
  28/28 killed）——内容未变时差分复用跳过全部场景，强制重跑须删除该注释块；其余 12 份无
  注释块。`gherkin-mutator` 的 `--runner-worker` 适配器唯一路径绑定 e2e QA runner（六套件
  滞后未同步，判定不可信）。**移交 QA 会话**：套件同步后按 `packs/_common/notes/aps.md` +
  mutator-spec 补齐判定（APS 工具先按工具链条重装）。
- **变异覆盖面受限**（沿袭）：v127 轮差分变异 78/78 killed；全项目 scan
  **2161 点/54 文件**（单文件最大 98——`ui/chart.rs`，均 <100 门槛）。v128 触碰面 8 个
  ui 文件、v129 触碰面 4 文件、v130 触碰面 1 文件（ui/mod.rs）均不在 killed 清单内
  ——如需该面加固证据由后续 hardender 轮补跑（`--file` 按文件分块协议）。
- **CRAP 为受限近似口径**（Rust 无标准 CRAP 工具；llvm-cov lcov BRDA comp=分支记录数+1
  上界近似 + 源码决策点复核，偏差方向见 `packs/_common/notes/rust.md`）：v129 轮解析器
  重建（crap_v5.py）复测门禁命中 **90 条**（产品侧 70 + 测试基建 13 + fixture/proxy 6 +
  路径伪影 1）。v130 触碰面无新增；v131 纯文档轮无触碰。
- **CPD 剩余 51 tiles**（PMD 7.28，`-l rust --minimum-tokens 50`，src-only；src+tests
  口径 **110**）：形态沿用 v129 登记（delete/dialog.rs import 块 tile、`fn seed()` 夹具
  tile、supervisor 测试区 mock 样板、分发臂/UI 绘制既有形态）。v130/v131 触碰面无变化。
- 无其他受限项。

### 实现定义值登记（当前有效）
- v131 无新增实现定义值。沿袭有效：D28/D29（仅添加落态与三钮布局）、D30（下调即时收缩）、
  D31（1MB 下限）、D32（Enter 语义）、D33（1s 归零时延）、D34（清码 R = 不校验直接收尾；
  v1.18 实现精化 = 文件完整性直判，见 D34 v1.18 注记）。修订 B fallback（文件不完整 →
  作废断点从头重下）为 FR-01-22 作废语义同源的实现定义值，操作者可改判（见待办）。

## 二、当前产出情况（specifier v131-spec-v119）

### 本会话产物清单
- `project/mission/phase-01.md`：头部补写 **v1.19 修订注记**（+7 行，插入于 v1.15 块与
  v1.18 块之间，沿历轮「最新在前」插入惯例）——①R 恢复路径退役（FR-01-92 修订、D25 注记；
  `01-slots-queue` 场景 15 / QA-SQ-15）；②FailedPaused 状态徽标白落账（状态配色 8→9 态、
  穷尽守卫测试；`01-tui-display` 场景 05 / QA-TD-05）。v130 交接「头部版本号未上翻」
  待办就此闭环。标题行 `(v1.1)` 沿历轮惯例冻结不动（版本口径以头部修订注记最新条为准）。
- `project/handoff.md`：本文件重写——总账并入 specifier v131 行、工具链受限项刷新为
  当前沙箱实况（全新环境全套待装）、待办待批重整（头部版本号项移出）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 头部注记在位与插入序 | v1.19 注记位于 v1.15 与 v1.18 之间；头部修订序列 …v1.15 → v1.19 → v1.18 → v1.17 → v1.16… | `rg -n "v1\\.19 修订" project/mission/phase-01.md` |
| 注记对账（六处内嵌口径） | 逐项一致：FR-01-92 行（R 恢复路径退役）、D25 行（R 收窄注记）、`features/01-slots-queue.feature` 场景 15、`qa/01-slots-queue-qa.md` QA-SQ-15、`features/01-tui-display.feature` 场景 05（已暂停（失败）=白）、`qa/01-tui-display-qa.md` QA-TD-05（七态白入表） | `rg -n "v1\\.19" project/mission/phase-01.md project/features/01-slots-queue.feature project/qa/01-slots-queue-qa.md project/features/01-tui-display.feature project/qa/01-tui-display-qa.md` |
| 改动面 | 仅 `project/mission/phase-01.md` +7 行；feature/QA/源码/ezr-demo 零改动 | `git diff --stat` |
| 代码基线 | 不变（v130 终态口径沿用，本轮未触碰代码，无需重跑） | v130 交接「验证证据」节 |

### 对账口径（基线 → 终态）
- 代码与规格基线 2535P/0F / clippy 0 / fmt 0 / dry-check error 级 0：**不变**（本轮纯
  mission 文档补账，无代码与 feature/QA 改动）。
- phase-01.md 头部版本口径：v1.18 → **v1.19**（唯一变更点，+7 行注记）。

### 待办与待批（未决项）
- 已闭环（本轮落账）：phase-01.md 头部版本号上翻——v1.19 注记补齐，v130 交接该项待办
  关闭；FailedPaused 徽标色待批此前已随 v1.19 续登记闭环（v130 会话确认落账）。
- 沿袭待批：修订 B fallback（文件不完整 → 作废断点从头重下）为实现定义值，操作者可改判
  （如改判「保留台账从缺失处补下」，代价：稀疏文件空洞风险，不建议）。
- 六 e2e 套件同步移交 QA 会话（沿袭受限项；套件未同步前 e2e 不可信）。
- 新沙箱工具链重装为下轮构建/测试/变异/APS 工作的前置任务（见遗留受限项第一条）。

### 移交建议
- 下一角色：**six-pack/cleaner**（v130 交接既定链序建议 coder → cleaner；本轮 specifier
  补账不改链序）。建议要点（沿 v130 并刷新）：①四度量复核以 v130 终态为基线（2535P/0F），
  触碰面 1 文件（ui/mod.rs），预计无新 tile/无新 CRAP 命中——**但本沙箱全套工具链需先重装**
  （Rust nightly、cargo-llvm-cov、PMD、cargo-mutants 等，见遗留受限项第一条）；②本轮断言
  补强与 v129 Modify 渲染补测同为「纯追加」路线，复核口径可直接沿用；③本地验证命令清单
  沿用 v130 归档口径（cwd `project/ezr/`：`cargo test --workspace` / `cargo clippy
  --all-targets` / `cargo fmt --check` / `bash scripts/arch_check.sh`）；④APS 工具重装就位
  前，feature 相关 dry-check 不可跑（本轮未改 feature，无遗留欠账）。
- 若操作者继续补规格侧账：无（规格侧当前无未决项）。
