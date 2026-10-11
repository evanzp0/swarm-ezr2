# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder 终态　会话：v130-code-v119（任务 v130-code-v119）
> 本轮（coder 正式承接 v1.19 行为切片 + 验收流水线补跑）：以上游 cleaner v129-clean 交接与
> 两节 v1.19 会话外登记为基线，核对切片实现零偏差，补跑 APS 验收流水线（parser + dry-check +
> 基线增量对账），TDD 缺口复查补强挂起 toast 文案断言 1 处。
> 终态全链绿：**2535P/0F**（与交接基线持平，本轮纯断言追加不加测试）、clippy 0、fmt 0、
> APS dry-check error 级 0（advisory 基线 delta=0）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.1，分期模式）+ `project/mission/phase-01.md`
  （**v1.18**；FR-01-92/D25 已内嵌 v1.19 修订注记，头部版本号上翻遗留 specifier，见待办）。

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
| coder（v130-code-v119，本轮） | v1.19 切片正式承接核对（零偏差）+ 挂起 toast 文案断言补强 + APS 验收流水线补跑 + handoff 重写并账 | 已交付 | **2535P/0F**、clippy 0、fmt 0、parser IR 15/15×2、dry-check error 级 0 |

### 遗留受限项（当前有效）
- e2e 六套件滞后于规程（累计）：`suite_add_task.py`、`suite_ui_conns.py`、
  `suite_modify_task.py`、`suite_persistence_config.py`、`suite_tui_display.py`、
  `suite_integrity_check.py`——QA 会话须先同步全部受影响套件再跑 e2e。
  注：v1.19 改动改变可观测行为（挂起 toast 文案、FailedPaused 徽标白、R 收窄），
  **QA-TD-05/QA-SQ-15 已同步，六套件代码仍滞后**，套件同步前 e2e 不可信。
- **软 Gherkin 验收变异受限**（沿袭 v127 登记，本轮环境注记刷新）：bb 1.12.209 与
  APS 仓库本轮已装回（`.tools/Acceptance-Pipeline-Specification`），`gherkin-parser`
  /`gherkin-ir-dry-checker` 可用；但 `gherkin-mutator` 的 `--runner-worker` 适配器
  唯一路径绑定 e2e QA runner（六套件滞后未同步，判定不可信）。13 份 feature 中仅
  `01-persistence-config.feature` 有历史 stamp（28/28 killed）。**移交 QA 会话**：
  套件同步后按 `packs/_common/notes/aps.md` + mutator-spec 补齐判定。
- **变异覆盖面受限**（沿袭）：v127 轮差分变异 78/78 killed；全项目 scan
  **2161 点/54 文件**（单文件最大 98——`ui/chart.rs`，均 <100 门槛）。v128 触碰面 8 个
  ui 文件、v129 触碰面 4 文件与 v130 触碰面 1 文件（ui/mod.rs）均不在 killed 清单内
  ——如需该面加固证据由后续 hardender 轮补跑（`--file` 按文件分块协议）。
- **CRAP 为受限近似口径**（Rust 无标准 CRAP 工具；llvm-cov lcov BRDA comp=分支记录数+1
  上界近似 + 源码决策点复核，偏差方向见 `packs/_common/notes/rust.md`）：v129 轮解析器
  重建（crap_v5.py）复测门禁命中 **90 条**（产品侧 70 + 测试基建 13 + fixture/proxy 6 +
  路径伪影 1）。v130 触碰面无新增（断言补强不引入新决策点）。
- **CPD 剩余 51 tiles**（PMD 7.28，`-l rust --minimum-tokens 50`，src-only；src+tests
  口径 **110**）：形态沿用 v129 登记（delete/dialog.rs import 块 tile、`fn seed()` 夹具
  tile、supervisor 测试区 mock 样板、分发臂/UI 绘制既有形态）。v130 触碰面无变化。
- **本轮沙箱工具链状态**：rustup nightly（rustc/cargo **1.101.0-nightly (6f5da406b
  2026-10-10)**，minimal + rustfmt + clippy + llvm-tools-preview），`~/.cargo`，新 shell 需
  `export PATH="$HOME/.cargo/bin:$PATH"`；cargo-mutants 27.1.0、cargo-llvm-cov 0.9.1、
  rustfilt（均 `~/.cargo/bin`）、PMD 7.28.0（`.tools/pmd-bin-7.28.0`）；**本轮补装**
  Babashka **1.12.209**（`~/.local/bin/bb`，静态二进制）+ APS 仓库克隆
  （`.tools/Acceptance-Pipeline-Specification`）。磁盘预算注记：llvm-cov `--branch`
  插桩全量重编译后 target 树 ≈6G，沙箱 10G 盘需先清主 `target/debug` 再跑。
- 无其他受限项。

### 实现定义值登记（当前有效）
- v130 无新增实现定义值。沿袭有效：D28/D29（仅添加落态与三钮布局）、D30（下调即时收缩）、
  D31（1MB 下限）、D32（Enter 语义）、D33（1s 归零时延）、D34（清码 R = 不校验直接收尾，
  v1.18 实现精化见 coder v124-code-batch5 轮）。修订 B fallback（文件不完整 → 作废断点
  从头重下）为 FR-01-22 作废语义同源的实现定义值，操作者可改判（见待办）。

## 二、当前产出情况（coder v130-code-v119）

### 本会话产物清单
- `src/ui/mod.rs`：`failed_pause_resume_flow` 挂起段 toast 断言补强（+9 行，纯断言追加
  不加测试）——在原「已暂停」包含断言外，新增「按空格恢复」包含断言 + 「或 R」排除断言
  （v1.19 收窄口径的引导文案锚定：回退旧文案「按空格或 R 恢复」从此有测试可杀）。
- `project/handoff.md`：本文件重写——两节 v1.19 会话外登记并入产物总账（coder v130 行 +
  会话外登记两行），徽标色裁决闭环落账，受限项/待办刷新为当前有效口径。
- `packs/_common/engineering.md`：经验沉淀 1 条（「显示层伪影 ≠ 文件损坏」——工具结果显示
  层吞字/注入伪影的交叉验证纪律：grep 计数 + od 字节核对 + 编译器为准，禁止凭显示层证据
  改写文件）。
- `.tools/`（工具缓存，非 project/ 产物）：Babashka 1.12.209 + APS 仓库补装
  （验收流水线工具就位，见遗留受限项刷新）。

#### 交接四要素

**① 本地验证命令清单**（cwd：`project/ezr/`；新 shell 先
`export PATH="$HOME/.cargo/bin:$PATH"`；APS 工具按 engineering.md 须入
`.tools/Acceptance-Pipeline-Specification` 目录调用，输出写 `.work/tmp/`）：
```sh
cargo test --workspace                                   # 全量 2535P/0F
cargo clippy --all-targets                               # 0 警告
cargo fmt --check                                        # 0 偏差
bash scripts/arch_check.sh                               # 14 规则（v130 实跑过）
cargo test --bin ezr failed_pause_resume_flow            # 本切片主测试单独口径
cargo test --test hardening_ui_backfill gen_fixtures_match_product_sources  # gen-drift 守卫
(cd ../../.tools/Acceptance-Pipeline-Specification && bb gherkin-parser ../../project/features/01-slots-queue.feature ../../.work/tmp/slots_queue.json)
(cd ../../.tools/Acceptance-Pipeline-Specification && bb gherkin-ir-dry-checker ../../.work/tmp/slots_queue.json ../../.work/tmp/slots_queue.dry.json)
```

**② 会话运行时产物位置**（`.work/tmp/`，`complete` 后随会话清除）：
- `fmt_v130.log`、`clippy_v130.log`、`test_v130.log`、`test_v130b.log`（补强后复核轮）、
  `clippy_v130b.log`；APS：`slots_queue.json/.dry.json`、`tui_display.json/.dry.json`、
  `base/`（HEAD 基线 feature + IR + dry，增量对账用）。

**③ feature 注释块与差分复用注意事项**：`features/01-persistence-config.feature` 头部含
`# mutation-stamp: sha256=…` 注释块（28/28 killed）——内容未变时差分复用跳过全部场景，
强制重跑须删除该注释块；其余 12 份无注释块。本轮未改任何 feature（v1.19 改动已随会话外
登记入树）。

**④ 等价突变体清单**：不适用（本轮未跑变异测试；v127 轮 78/78 killed 空清单口径见其总账行）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量单测 | **2535P / 0F**（与交接基线持平） | `cargo test --workspace` |
| fmt / clippy / arch | 0 偏差 / 0 警告 / 14 规则过（均含补强后复核轮） | `cargo fmt --check`、`cargo clippy --all-targets`、`bash scripts/arch_check.sh` |
| APS parser | 两份 v1.19 feature IR 解析成功，场景数 15/15；场景 15 步骤如实含收窄形态 `When 按空格` | `bb gherkin-parser`（见①） |
| APS dry-check | **error 级 0**（advisory 6+15 全 medium） | `bb gherkin-ir-dry-checker`（见①） |
| dry-check 基线对账 | findings 6→6、15→15，**delta=0**；唯一文本变化为同一 possible-synonym advisory 内嵌场景标题串更新（v1.19 注记），语义零漂移 | git show HEAD 版本 feature 重跑对比 |
| 切片断言审查 | R 合法路径三臂在位（Fatal 重排/Verify 重校验/Verify 无码直排）；穷尽守卫 + DIM 兜底断言在位；R 误恢复四重断言在位 | `src/ui/mod.rs`、`src/app/tasks.rs` |
| 补强测试执行 | `failed_pause_resume_flow` 4 挂载上下文各执行 1 次全过 | 全量日志 grep 计数 |

### 对账口径（基线 → 终态）
- 单测 2535 → 2535：**+0**——本轮为既有测试内断言追加（`failed_pause_resume_flow`
  挂起 toast 文案锚定），非新增测试；2531 → 2535 的 +4（穷尽守卫测试 × 4 挂载上下文）
  属上一轮会话外登记，已在总账行确认。全量日志中补强测试名出现 4 次与挂载口径自洽。
- dry-check findings：slots_queue 6→6、tui_display 15→15，delta=0（基线 = git HEAD
  未含 v1.19 改动版本；增量提示逐条审为标题串更新，非语义漂移）。
- 本轮触碰面 1 文件（`src/ui/mod.rs`）；CPD/变异点/CRAP 口径不变（断言不引入重复块与
  新决策点，未复跑三度量——cleaner 四度量复核按链序承接）。

### 待办与待批（未决项）
- **phase-01.md 头部版本号未上翻**（现 v1.18；v1.19 修订注记已内嵌 FR-01-92/D25 行）——
  mission 文件 specifier 专属维护（coder 只读），移交操作者决定是否开 specifier 轮补账。
- 沿袭待批：修订 B fallback（文件不完整 → 作废断点从头重下）为实现定义值，操作者可改判
  （如改判「保留台账从缺失处补下」，代价：稀疏文件空洞风险，不建议）。
- 六 e2e 套件同步移交 QA 会话（沿袭受限项，v1.19 后滞后面扩大：挂起 toast 文案、徽标白、
  R 收窄三处行为已变，套件未同步前 e2e 不可信）。
- 已闭环（本轮确认落账）：FailedPaused 徽标色待批——裁决选项①（显式映射白）已随 v1.19
  续登记落地，穷尽守卫测试堵再犯根因；本会话核对映射表 9 态与守卫测试在位，闭环成立。

### 移交建议
- 下一角色：**six-pack/cleaner**（链序 coder → cleaner）。建议要点：
  ① 四度量复核以 v130 终态为基线（2535P/0F），触碰面 1 文件（ui/mod.rs），预计无新
  tile/无新 CRAP 命中；② 本轮断言补强与 v129 的 Modify 渲染补测同为「纯追加」路线，
  cleaner 复核时可直接沿用其口径对账；③ APS 工具已在本沙箱就位，cleaner 如需对 feature
  做清理期 dry-check 可直接用（无需重装）。
- 若操作者先补规格账：`six-pack/specifier` 上翻 phase-01 头部版本号（v1.19，注记已内嵌），
  属小轮次、无代码影响。
