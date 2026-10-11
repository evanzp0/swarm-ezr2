# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner 终态　会话：v132-clean-v119（任务 v132-clean-v119）
> 本轮（cleaner 清理轮）：承接 v130-code-v119 + v131-spec-v119 交接，对 v1.19 切片 coder
> 产出做保持行为复核与四度量复核。**零改动轮**：触碰面（ui/mod.rs）复核无清理点，
> 代码/feature/QA/mission 零改动，v130 终态验证口径全部精确复现；另完成新沙箱全套
> 工具链重装（Babashka/APS 除外，本轮无需）与覆盖率基线首次登记（94.28%）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.1，分期模式）+ `project/mission/phase-01.md`
  （v1.19，头部修订注记与六处内嵌口径已对账一致）。

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
| coder（v130-code-v119） | v1.19 切片正式承接核对（零偏差）+ 挂起 toast 文案断言补强 + APS 验收流水线补跑 + handoff 重写并账 | 已交付 | 2535P/0F、clippy 0、fmt 0、parser IR 15/15×2、dry-check error 级 0 |
| specifier（v131-spec-v119） | phase-01.md 头部 v1.19 修订注记补写（R 恢复路径退役 + FailedPaused 徽标白 + 规格/QA 引用）——v130 交接遗留待办闭环 | 已交付 | 注记与六处内嵌口径逐项对账一致；纯文档补账 +7 行 |
| cleaner（v132-clean-v119，本轮） | 零改动清理轮：触碰面复核（无清理点）+ 新沙箱工具链重装 + 四度量复核 + 覆盖率基线首次登记 | 已交付 | **2535P/0F**、clippy 0、fmt 0、arch 14 过、覆盖率 94.28%、CPD 51 tiles、scan 2161 点/54 文件、CRAP 门禁 76 条（触碰面零新增） |

### 遗留受限项（当前有效）
- **本沙箱工具链已重装就位（v132 轮完成，替代 v131「全套待装」登记）**：Rust nightly
  1.101.0（2026-10-10；rustup minimal + rustfmt + clippy + llvm-tools-preview，新 shell 须
  `export PATH="$HOME/.cargo/bin:$PATH"`）、ld.lld 桥接符号链接（rust-lld →
  `~/.local/bin/ld.lld`，PATH 须含 `~/.local/bin`）、cargo-llvm-cov 0.9.1、
  cargo-mutants 27.1.0、rustfilt 0.2.1（均 `~/.cargo/bin/` 预编译二进制/小构建）、
  PMD 7.28.0（`.tools/pmd-bin-7.28.0`）。**未装**：Babashka、APS 仓库克隆——本轮未改
  feature 无欠账；**移交 QA 会话**按 `packs/_common/notes/aps.md` 重装后再跑 feature
  dry-check 与 e2e。磁盘余约 5G：llvm-cov `--branch` 插桩全量重编译 ≈6G，重跑覆盖率前
  先清主 `target/debug`（本轮实测插桩树 + 主树峰值未超预算）。
- e2e 六套件滞后于规程（累计）：`suite_add_task.py`、`suite_ui_conns.py`、
  `suite_modify_task.py`、`suite_persistence_config.py`、`suite_tui_display.py`、
  `suite_integrity_check.py`——QA 会话须先同步全部受影响套件再跑 e2e。
  注：v1.19 改变可观测行为（挂起 toast 文案、FailedPaused 徽标白、R 收窄），
  QA-TD-05/QA-SQ-15 已同步，六套件代码仍滞后，套件同步前 e2e 不可信。
- **软 Gherkin 验收变异受限**（沿袭 v127 登记）：13 份 feature 中仅
  `01-persistence-config.feature` 有历史 stamp（28/28 killed）——内容未变时差分复用
  跳过全部场景，强制重跑须删除该注释块；其余 12 份无注释块。`gherkin-mutator` 的
  `--runner-worker` 适配器唯一路径绑定 e2e QA runner（六套件滞后未同步，判定不可信）。
  **移交 QA 会话**：套件同步后按 `packs/_common/notes/aps.md` + mutator-spec 补齐判定。
- **变异覆盖面受限**（沿袭）：v127 轮差分变异 78/78 killed；全项目 scan **2161 点/54 文件**
  （v132 本轮复测精确一致；单文件最大 98——`ui/chart.rs`，均 <100 门槛）。v128 触碰面
  8 个 ui 文件、v129 触碰面 4 文件、v130/v132 触碰面（ui/mod.rs）均不在 killed 清单内
  ——如需该面加固证据由后续 hardender 轮补跑（`--file` 按文件分块协议）。
- **CRAP 为受限近似口径**（Rust 无标准 CRAP 工具；llvm-cov lcov BRDA comp=分支记录数+1
  上界近似 + 源码决策点复核，偏差方向见 `packs/_common/notes/rust.md`）：**v132 轮在新
  沙箱复测门禁命中 76 条**（产品侧 67 + fixture/proxy 9），对 v129 基线 90 条的偏差已按
  环境边界归因（详见本轮交接对账口径）：fixture 子进程 LLVM_PROFILE_FILE 继承使
  NO_DA 口径漂移（v129 环境三类函数无 DA 数据不上榜，本环境全部具数据）、tests/ 上下文
  缺失使测试基建口径条目不复出现、产品侧 -3（v1.19 收窄减支 + nightly BRDA 粒度差）。
  **后续轮次对账以本轮 76 条 + 归因口径为新基线**，跨环境再对账先探 fixture 源文件是否
  出现在 lcov 且 DA>0（`packs/_common/notes/rust.md` 第三环境边界条款）。
- **CPD 剩余 51 tiles**（PMD 7.28，`-l rust --minimum-tokens 50`，src-only；v132 本轮
  复测精确一致）：形态沿用 v129 登记（delete/dialog.rs import 块 tile、`fn seed()` 夹具
  tile、supervisor 测试区 mock 样板、分发臂/UI 绘制既有形态）。
- 无其他受限项。

### 实现定义值登记（当前有效）
- v132 无新增实现定义值。沿袭有效：D28/D29（仅添加落态与三钮布局）、D30（下调即时收缩）、
  D31（1MB 下限）、D32（Enter 语义）、D33（1s 归零时延）、D34（清码 R = 不校验直接收尾；
  v1.18 实现精化 = 文件完整性直判，见 D34 v1.18 注记）。修订 B fallback（文件不完整 →
  作废断点从头重下）为 FR-01-22 作废语义同源的实现定义值，操作者可改判（见待办）。

## 二、当前产出情况（cleaner v132-clean-v119）

### 本会话产物清单
- **代码与规格零改动**（零改动清理轮）：触碰面（v130 的 ui/mod.rs：STATE_COLORS 9 态
  映射表 + 穷尽守卫测试 + 挂起 toast 断言补强 + `render_frame_string` 单源 helper；产品
  侧 retry() 收窄与 TaskState doc）逐项复核——改动均已带依据注释、命名清晰、无死代码、
  无重复、测试可读性良好；v129/v130 两轮清理后代码面已收敛，无新清理点。
- `project/handoff.md`：本文件重写——总账并入 cleaner v132 行、工具链受限项刷新为
  「已重装就位（Babashka/APS 除外）」、CRAP 基线上翻为 v132 沙箱口径 76 条、覆盖率基线
  首次登记。
- 工具链安装（会话产物，工具侧）：见遗留受限项第一条（版本与位置清单）。

### 验证证据
| 验证项 | 结果 | 命令（可复现，cwd `project/ezr/`） |
|---|---|---|
| 单元测试 | **2535P / 0F**（13 测试目标合计，与 v130 终态精确一致） | `cargo test --workspace` |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| rustfmt | 0 diff | `cargo fmt --check` |
| 架构边界 | 14/14 规则通过 | `bash scripts/arch_check.sh` |
| 覆盖率（行） | **94.28%**（12552/13314，57 文件；插桩运行 2535P/0F 与基线一致） | `CARGO_PROFILE_DEV_DEBUG=0 cargo llvm-cov --workspace --branch --lcov --output-path <lcov>`（首登记基线，含 fixture/proxy bin） |
| CRAP 门禁（受限近似） | **76 条**（产品 67 + fixture/proxy 9；NO_DA=0；cfg(test) 模块排除 450 函数；触碰面 ui/mod.rs 仅既有 draw 分发核心 1 条、零新增） | 解析器重建口径见 `packs/_common/notes/rust.md`（脚本随会话清除；对账分析见本轮对账口径节） |
| DRY（CPD） | **51 tiles**（与 v129 基线精确一致） | `<swarm>/.tools/pmd-bin-7.28.0/bin/pmd cpd -l rust --minimum-tokens 50 --dir src`（exit 4 = 发现重复，正常语义） |
| 变异点扫描 | **2161 点 / 54 文件**（与基线精确一致；单文件最大 98 < 100 门槛；触碰面 ui/mod.rs 5 点） | `cargo mutants --list --line-col=true` |
| 工具版本 | nightly 1.101.0 / llvm-cov 0.9.1 / mutants 27.1.0 / rustfilt 0.2.1 / PMD 7.28.0 | 各工具 `--version` |

### 对账口径（基线 → 终态）
- 代码与规格基线 2535P/0F / clippy 0 / fmt 0 / arch 14：**不变**（零改动轮，全部精确复现）。
- CPD 51 = 51、变异点 scan 2161/54 = 2161/54：**精确一致**（上游基线口径沿用：同工具
  同开关同目录范围）。
- CRAP 门禁 76 vs v129 登记 90：**偏差已归因，非回归**——①本环境 fixture/代理子进程继承
  `LLVM_PROFILE_FILE`，profdata 并入其覆盖，`ezr-fixture` 三函数（respond.rs `ead`、
  fixture/proxy `main`×2）由 v129 环境的 NO_DA 型（不上榜）变为具数据 cov=0 满值上榜
  （fixture/proxy 6→9 的全部增量）；②v129「测试基建 13」条目依赖 tests/ 上下文，本环境
  cargo-llvm-cov lcov 不含挂载测试 crate 上下文（工具既有边界），条目整体不再出现；
  ③产品侧 70→67：v1.19 收窄减支（retry/FailedPaused 相关 comp 下降）+ nightly BRDA
  粒度差（±few 容差内）。三条均属 `packs/_common/notes/rust.md` 既有环境边界条款，
  无一指向产品缺陷；触碰面零新增为本轮核心结论。
- 覆盖率 94.28%：**首登记基线**（上游历轮未登记行覆盖数字；口径 = llvm-cov 0.9.1
  `--workspace --branch --lcov`，行口径 LF/LH 汇总，含 fixture/proxy bin）。剩余缺口
  为入口适配壳（run/ezr_main/run_tui——TUI/信号环境不适单测，保持小型外壳纪律）与
  分发核心注册形态；按 SKILL 杠杆顺序（cov=0 纯函数 → 高 comp 拆分）无可纯追加收敛
  目标，零改动轮成立。

### 待办与待批（未决项）
- 沿袭待批：修订 B fallback（文件不完整 → 作废断点从头重下）为实现定义值，操作者可改判
  （如改判「保留台账从缺失处补下」，代价：稀疏文件空洞风险，不建议）。
- 六 e2e 套件同步移交 QA 会话（沿袭受限项；套件未同步前 e2e 不可信）。
- Babashka + APS 仓库克隆移交 QA 会话按需重装（本轮未改 feature，无 dry-check 欠账）。
- 无新增待办。

### 移交建议
- 下一角色：**six-pack/architect**（v130 交接既定链序 cleaner → architect，本轮链序不变）。
  建议要点：①四度量基线以 v132 终态为准（2535P/0F、CPD 51、scan 2161/54、CRAP 76 条
  新沙箱口径、覆盖率首登记 94.28%）；②architect 轮属性测试扩展沿 v126 口径
  （`src/property_tests.rs` 独立运行 `cargo test property_tests::`，全量对账不含属性）；
  ③CRAP 高 comp 残余（supervisor block_worker/download、dialogs dlg_finish_add、
  evt on_evt、mouse on_mouse_dialog 等分发/编排核心）沿 v129 登记口径（comp=分支数
  注册而非硬拆），模块级拆分属 architect 职权边界——如需拆分先收模块边界再交
  hardender 以变异兑现收益（`packs/_common/notes/rust.md` 变异点与拆分条款）；
  ④本沙箱工具链已就位（Babashka/APS 除外），构建/测试/度量可直接开工。
- 若操作者继续补规格侧账：无（规格侧当前无未决项）。
