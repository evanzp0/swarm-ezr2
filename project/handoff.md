# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect　会话：v134-arch-clean（任务 v134-arch-clean）
> 本轮（architect 架构评审 + 策略提取 + 属性测试扩展轮）：承接 v133 交接，四阶段架构评审
> 零结构性违规（14 条 arch 规则全过 + 补充扫描零命中）；唯一实质改进 = throttle 令牌账本
> 纯策略提取（bucket_step，策略/IO 外壳分离，机械等价）；属性测试 57→60（新沙箱工具链
> 重装后基线复跑全绿）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.20，分期模式）+ `project/mission/phase-01.md`
  （v1.20，任务 ID 随机化 + 详情拆行）。

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
| cleaner（v132-clean-v119） | 零改动清理轮：触碰面复核（无清理点）+ 新沙箱工具链重装 + 四度量复核 + 覆盖率基线首次登记 | 已交付 | **2535P/0F**、clippy 0、fmt 0、arch 14 过、覆盖率 94.28%、CPD 51 tiles、scan 2161 点/54 文件、CRAP 门禁 76 条（触碰面零新增） |
| specifier（v133-spec-anchor-fallback） | 修订 B fallback 锚定并入正式账：phase-01.md D34 v1.18 注记补「操作者 20261011 直判认可维持」锚定行——v132 交接会话外登记项闭环 | 已交付 | 纯注记补账：git diff 仅 phase-01.md 1 行（D34），代码/feature/QA/度量零改动 |
| architect（v134-arch-clean，本轮） | 四阶段架构评审（零结构性违规）+ throttle 令牌账本纯策略提取（bucket_step 策略/IO 分离，机械等价）+ 属性测试 57→60 + 新沙箱工具链重装（nightly + PMD）+ CPD 复测 51 同形 | 已交付 | **2546P/0F**（属性 60P 独立计数）、clippy 0、fmt 0、arch 14 过、CPD 51 tiles 与 v132 基线逐 tile 同形 |
| 会话外登记（操作者直判 v1.20，20261011 指令） | 任务 ID 设计改版：u32 随机数分配（`model::idgen` 单源：SplitMix64 查重分配、0 保留哨兵），`next_id` 全链路退役（App 字段/注册表持久化/测试靶替换），注册表 v1 格式零迁移（旧 next_id 字段反序列化忽略）；详情面板「任务 ID」「创建时间」拆分两行、详情定高 11→12 行（FR-01-80/FR-01-94⑥ 修订 + 两 feature 场景同步）；ezr-demo 同步（随机 ID + 两行详情 + 需求文档 + 预填轮换口径保持 + release 二进制刷新）；归档前复核补遗：`tests/gen/detail_stripped.rs` 因 rustfmt 收尾晚于夹具重生成而漂移，已按守卫口径重生成（经验沉淀 → notes/rust.md「剥头夹具再生成排在 fmt 之后」条目） | 已交付 | 产品 **2578P/0F** 实测（13 测试二进制 `cargo test --all-targets`，账面历史口径 2546 为 v134 计数）、clippy 0、fmt clean；demo cargo check 过 + smoke_v_sync 全过 + verify_ezr 回归（场景 C Fedora 轮换口径保持） |

### 遗留受限项（当前有效）
- **新沙箱工具链部分重装（v134 轮）**：Rust nightly 1.101.0（2026-10-10；rustup minimal +
  rustfmt + clippy + llvm-tools-preview，新 shell 须 `export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"`）、
  ld.lld 桥接符号链接（`~/.local/bin/ld.lld` → 工具链 rust-lld，PATH 须含 `~/.local/bin`）、
  PMD 7.28.0（`.tools/pmd-bin-7.28.0`）已就位。**未装**：cargo-llvm-cov / cargo-mutants /
  rustfilt（沙箱磁盘余 1.8G，`llvm-cov --branch` 插桩全量重编译 ≈6G 不可承载；cargo-mutants
  安装亦受磁盘约束未做）、Babashka、APS 仓库克隆——本轮未改 feature，无 dry-check 欠账；
  **移交 QA 会话**按 `packs/_common/notes/aps.md` 重装后再跑 feature dry-check 与 e2e。
- **覆盖率 / CRAP / scan 沿 v132 登记口径（本轮未复跑）**：94.28% / 76 条 / 2161 点/54 文件。
  本轮触碰面 = `engine/throttle.rs`（策略提取，产品区净 +1 函数 bucket_step、acquire 收敛）
  与测试文件——scan 预计小幅新增（bucket_step 新函数变异点），精确计数移交 hardender 轮
  `cargo mutants --list` 复测；覆盖率影响仅限 throttle 域（纯函数新增测试面，方向向好）。
  工具重装就位后如需对账先清主 `target/debug`（磁盘预算见 v132 条）。
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
- **变异覆盖面受限**（沿袭）：v127 轮差分变异 78/78 killed；v128 触碰面 8 个 ui 文件、
  v129 触碰面 4 文件、v130/v132 触碰面（ui/mod.rs）、**v134 触碰面（engine/throttle.rs +
  属性测试）**均不在 killed 清单内——如需该面加固证据由后续 hardender 轮补跑
  （`--file` 按文件分块协议）。
- **CRAP 为受限近似口径**（Rust 无标准 CRAP 工具；llvm-cov lcov BRDA comp=分支记录数+1
  上界近似 + 源码决策点复核，偏差方向见 `packs/_common/notes/rust.md`）：v132 轮新沙箱
  复测门禁命中 76 条，**后续轮次对账以 v132 的 76 条 + 归因口径为基线**；本轮未复跑
  （工具未装），触碰面 throttle.rs 原 CRAP 残余（时序测试域）不因策略提取改变。
- **CPD 51 tiles**（PMD 7.28，`-l rust --minimum-tokens 50`，src-only）：本轮新沙箱复装
  PMD 后复测**逐 tile 同形**（51 条发现清单与 v132 基线 diff 为空）。形态沿用 v129/v132
  登记（delete/dialog.rs import 块 tile、`fn seed()` 夹具 tile、supervisor 测试区 mock
  样板、分发臂/UI 绘制既有形态）。
- 高 comp 残余（supervisor block_worker/download、dialogs dlg_finish_add、evt on_evt、
  mouse on_mouse_dialog 等分发/编排核心）沿 v129 登记口径（comp=分支数注册而非硬拆）；
  v134 评审复核 supervisor 模块拆分议题：职责凝聚（单任务生命周期四子域同一状态机），
  拆分收益（变异分块）低于机械移动风险（840 行测试区同步漂移），维持现状——模块级
  拆分留待 hardender 以变异收益兑现时再议。
- 无其他受限项。

### 实现定义值登记（当前有效）
- v134 无新增实现定义值。沿袭有效：D28/D29（仅添加落态与三钮布局）、D30（下调即时收缩）、
  D31（1MB 下限）、D32（Enter 语义）、D33（1s 归零时延）、D34（清码 R = 不校验直接收尾；
  v1.18 实现精化 = 文件完整性直判）。修订 B fallback（文件缺失/大小不符 → 作废断点从头重下，
  FR-01-22 作废语义同源）＝**操作者钦定**（20261011 直判认可维持；锚定行已由
  v133-spec-anchor-fallback 并入 phase-01.md D34 v1.18 注记）。
- 无未决待批项。

## 二、当前产出情况（architect v134-arch-clean）

### 本会话产物清单
- `project/ezr/src/engine/throttle.rs`：令牌账本纯策略提取——`acquire` 内联数学逐行等价
  提取为 `pub fn bucket_step(rate, tokens, elapsed, remaining) -> (tokens_after, taken, wait_ms)`
  （补币钳制 → 取走 min → deficit 等待换算），`acquire` 收敛为「读时钟 + 睡眠」IO 外壳；
  `BURST_SECS`/`SLEEP_MS` 升 pub（私有模块内）供测试单源消费；新增无头单测
  `bucket_step_headless_decisions`（补币钳制/取走/等待下限确定性锚点）。行为不变佐证：
  既有 3 个时序测试（unlimited_is_instant / limited_throttles_bulk /
  small_acquire_within_burst_is_fast）零改动全绿。
- `project/ezr/src/engine/mod.rs`：`#[cfg(test)] pub use throttle::{bucket_step, BURST_SECS,
  SLEEP_MS}` 门面 re-export（notes/rust.md 手法；常量随函数外借防测试侧双源；产品构建
  不产生该路径，窄接口不受影响）。
- `project/ezr/src/property_tests.rs`：属性测试 57→60——①`prop_throttle_bucket_step_invariants`
  （取走特征化 / 预算守恒 / 桶余钳制 / 等待收束四不变量；生成域 rate>0 注记）；
  ②`prop_apply_chunk_snapshot_monotone`（连接视图同参投影 / chunk_done 替换语义
  max(快照,事件) / 读侧钳制两段口径——属性实测纠正初版误设的「写侧钳制」假设）；
  ③`prop_thread_count_data_plane`（数据面口径泛化既有两状态单测，oracle 口径复述不借
  cap() 实现）。
- `project/ezr/scripts/arch_check.sh`：零改动（14 规则复核全过；本轮补充扫描——app→engine
  内部 reach-in、ui→app 子模块 reach-in、engine→app 反向、model 跨层私有路径——均零命中；
  无新违规形态即无新规则可固化，阳性对照纪律维持既有登记）。
- `packs/_common/notes/rust.md`：门面 re-export 条目扩展「自 async IO 外壳提取纯策略时的
  三件套」（函数 pub + cfg(test) 门面 re-export + 策略常量随函数外借防测试侧双源）。
- `packs/_common/engineering.md`：「设计与可测试性」新增「属性测试的 oracle 先核实现的真实
  契约层，再落断言」（写侧/读侧两段口径，实测断点快照案例）。
- `project/handoff.md`：本文件重写（历史总账并入 architect v134 行；当前节重写）。

### 验证证据
| 验证项 | 结果 | 命令（可复现，cwd `project/ezr`） |
|---|---|---|
| 全量测试 | 2546P / 0F（13 测试二进制） | `cargo test`（CARGO_PROFILE_DEV_DEBUG=0） |
| 属性测试（独立计数） | 60P / 0F（57+3） | `cargo test property_tests::` |
| clippy | 0 警告（模板 lints 全开） | `cargo clippy --all-targets` |
| rustfmt | 0 diff | `cargo fmt --check` |
| 架构检查 | 14/14 规则过 | `bash scripts/arch_check.sh` |
| CPD | 51 tiles，与 v132 基线逐 tile 同形 | `.tools/pmd-bin-7.28.0/bin/pmd cpd -l rust --minimum-tokens 50 --dir src` |
| throttle 行为不变 | 4/4（3 时序 + 1 无头） | `cargo test throttle` |

### 对账口径（基线 → 终态）
- 测试计数：上游 2535P（含属性 57）→ 本轮 2546P。复算：非属性基线 2535 − 57 = 2478；
  +8 = `bucket_step_headless_decisions` 在 8 个编译上下文各注册一次（主 bin ezr test +
  7 个挂载 engine/mod.rs 的加固测试 crate，`#[path]` 挂载机制合法副本，notes/rust.md
  「挂载树内 cfg(test) 测试重复注册」条款）；2478 + 8 = 2486；属性 57 → 60（+3，独立
  计数不对账）；2486 + 60 = 2546 ✓ 自洽。
- clippy 0 / fmt 0 / arch 14：与 v132/v133 基线持平（本轮修复自身引入的 1 条
  unnecessary_cast 后归零）。
- CPD 51：逐 tile 同形（发现清单 diff 为空），基线不因触碰面漂移。
- 覆盖率 94.28% / CRAP 76 / scan 2161/54：沿 v132 登记（本轮未复跑，工具未装；受限项
  已登记触碰面与预期方向）。

### 待办与待批（未决项）
- 无新增未决待批项。
- 沿袭移交：六 e2e 套件同步 + Babashka/APS 重装移交 QA 会话；覆盖率/CRAP/scan 精确
  复测移交 hardender 或 QA（工具重装后）；supervisor 模块拆分议题留 hardender 变异
  收益兑现时再议。

### 移交建议
- 下一角色：**six-pack/hardender**（链序 cleaner → architect → hardender，宪法第一章
  不自动流转，由操作者显式 `bin/swarm begin six-pack/hardender`）。建议要点：
  ①工具链先补装 cargo-mutants / rustfilt / cargo-llvm-cov（磁盘不足时先清主
  `target/debug`，v132 预算口径）；②`cargo mutants --list` 复测 scan 计数（本轮触碰面
  engine/throttle.rs 净 +1 函数 bucket_step，预计 +4~6 点）；③触碰面变异补跑按
  `--file` 分块协议先 `engine/throttle.rs`（bucket_step 纯函数，变异可 killed 面大），
  再议 supervisor 分发核心（v129 登记口径维持）；④CRAP/DRY 对账沿 v132 口径。
- 操作者要求新增 feature 时：直接 `bin/swarm begin six-pack/specifier`（首角色按
  chat 输入访谈建档，宪法第一章）。
