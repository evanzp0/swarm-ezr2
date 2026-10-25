# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA　会话：ezr-qa-final2（2026-10-25）
> 上游交接：project/handoff.md（architect v153-arch 交接：四阶段架构评审 + App 夹具族/
> 属性策略单源收口 + >100 变异点 7 文件分块裁决，基线 4564P/0F、CPD 125 tiles）
> 当轮任务：v153-arch 交接批次的 QA 终验轮——全量基线复验、e2e 套件全量首轮腐化探针
> 与 v1.34/35 口径同步、CRAP/DRY 工具门禁、APS 沿袭欠账清偿、six-pack 流转完结。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.35，分期模式）+ `project/mission/phase-01.md`（v1.35）。
- 本轮后源码状态：v1.34 全条款落地 + 历轮 UI/加固/覆盖轮 + 输入框组件化 +
  退出路径鼠标残序根治（v150）+ 清理批次（v150-clean）+ 架构轮（v151-arch）+
  v1.35 规格修订与实现 + field_window 振荡缺陷修复 + 清理轮（v152-clean）+
  架构轮（v153-arch）+ **本轮 QA 终验轮（v154-qa）——产品区零改动（含测试区）；
  e2e 可执行套件 14 份全量同步 v1.34/35 已接受规格口径，终态 179P/0F/0S**。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.35 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | **APS 机检欠账已清偿（本轮）**：14/14 feature parser ok + gherkin-ir-dry-checker 全过 |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F |
| architect（v145-arch） | supervisor 六文件拆分 + 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 | 已交付 | 3363P/0F |
| QA（ezr-qa-final / v148-qa 等） | 13+1 套件 e2e 可执行化 + llvm-cov 覆盖轮 | 已交付 | e2e 167P/0F（v148 基线态，已随版本演进为 179 例口径） |
| coder（v129–v140 系列） | FR-01-106 实现 + v1.30/v1.31 UI 改版 + FR-01-112 内置 DNS | 已交付 | 4162P/0F |
| cleaner（v141）/ architect（v146） | dns 清理 + 四阶段评审 | 已交付 | 4200P/0F、arch 15/15 |
| coder（v134-demo-sync-impl） | v1.34 全条款实现（FR-01-113…116 + 修订十二条） | 已交付 | 4300P/0F |
| coder（v134-uifix 系列 ×3） | 三类 UI bug 修复 + 输入框原生光标 | 已交付 | 4388P/0F、clippy 0、fmt clean |
| coder（v135-fieldcomp-impl） | ①输入框组件化（src/textfield.rs）；②ezr-demo 同步 | 已交付 | ezr：4484P/0F；ezr-demo：18P/0F + 回归 8 套件全绿 |
| coder（v150-quitmouse-impl） | 退出路径鼠标残序根治 + ezr-demo 全量同步 + TimedWriter 时序锁定测试 | 已交付 | ezr：4486P/0F、clippy 0、fmt clean；ezr-demo：20P/0F、回归 8 套件全绿 |
| cleaner（v150-clean） | mouse.rs 弹窗命中臂 +4 测、CRAP 门禁 6→2、rect_hit 单源化、度量基线重建 | 已交付 | ezr：4518P/0F、clippy 0、fmt clean；覆盖率 95.1% 行 |
| architect（v151-arch） | 模块边界收口（scrollbar.rs）+ on_mouse_main 拆分 + 属性 71 + arch 16 规则 | 已交付 | ezr：4532P/0F（15 目标）、clippy 0、fmt clean、arch 16/16、属性 71P/0F |
| specifier（v135-pieces-sync） | v1.35 规格修订：pieces 弹窗布局对齐（68×20/八段/31 块/341） | 已交付 | 沿袭测试基线 4532P/0F 不变 |
| coder（v135-impl-fieldwin） | ①v1.35 pieces 布局实现；②field_window 振荡缺陷修复 + 属性论域扩回全字母表 | 已交付 | ezr：4548P/0F（15 目标）、clippy 0、fmt clean、arch 16/16 |
| cleaner（v152-clean） | 孤儿 fixture 删除裁决 + task_with_conns 夹具共享单源 + mouse.rs +2 测（CRAP 收敛杠杆）+ field_window 职责评审裁决 + 四项度量基线重建 | 已交付 | ezr：4564P/0F（15 目标）、clippy 0、fmt clean、arch 16/16；覆盖率 95.2% 行；CRAP>30 门禁 1 条（豁免类）；CPD 137 tiles；变异点 3247 |
| architect（v153-arch） | ①四阶段架构评审（结论：分层基线无漂移，16 条护栏全部在位且必要）；②app 树 App 夹具族收口 testutil 单源；③属性策略构造三单源 helper；④>100 变异点 7 文件分块裁决（全部整文件 shard） | 已交付 | ezr：4564P/0F（15 目标，--test-threads=2，rc=0）、clippy 0（-D warnings）、fmt clean、arch 16/16；CPD 137→125 tiles；属性 71P/0F；变异点 3247 守恒（产品区零改动） |
| **QA（v154-qa，本轮）** | **①15 目标全量基线复验 + 门禁组（clippy/fmt/arch/属性）；②e2e 14 套件全量首轮腐化探针（179 例 166P/13F）→ 13 例失败逐例归因（全部套件陈旧、零产品回归）→ 同轮同步脚本 + 规程（UC-18 措辞）→ 终态 179P/0F/0S；③CRAP 轮（llvm-cov 15 目标 lcov 合并，覆盖率 95.2%、CRAP>30 非豁免域 0 条）；④DRY 轮（PMD CPD 125 tiles 与 v153 基线逐位一致）；⑤APS 沿袭欠账清偿（14/14 dry-check 过）；⑥经验沉淀 ×4（engineering.md ×2 + notes/rust.md ×2）** | **已交付（six-pack 流转完结）** | **产品区零改动；详见第二节验证证据** |

### 遗留受限项（当前有效）
- **demo 回归长脚本沙箱卡顿（沿袭）**：verify_ezr / test_focus_style 连续全量跑在
  沙箱内中后段随机挂起，按场景子集分半跑全绿（v135/v150 两轮复证）；本轮未改动
  ezr-demo，未重跑 demo 回归（无触碰面）。
- **真实 TCP stub e2e 时序脆弱（v147 沿袭）**：全绿口径以 `--test-threads=2` 复跑为准；
  本轮全量两遍（单测 15 目标 + e2e）均无假失败复发。
- **dns.rs 覆盖率残余（读数更新）**：本轮并集口径实测未覆盖 5 行
  [169, 243, 249, 478, 547]——其中 L478 为 cfg(test) 负向守卫闭包
  （`panic!("hosts 命中不应触内置解析")`，通过路径结构性不执行），余为观察受限
  错误/异步分支；v152 登记 3 行为其测量口径读数。本轮零产品改动，差异为口径
  与插桩上下文，非回归。
- **ezr-proxy 测试基建豁免（沿袭登记）**：ezr-proxy.rs 豁免（独立 bin + 测试基建）。
  `handle_socks5` CRAP 读数：v150 登记 80.2 为 bin-only 覆盖上下文口径；本轮 15 目标
  并集口径实测 CRAP 1.1（comp=1、cov 55.7%）——口径差异登记，豁免类维持。
- **CRAP>30 门禁（本轮读数）**：非豁免域 **0 条**（豁免域亦无 >30 读数）。
  近似口径：CRAP = comp²(1−cov)³ + comp，comp = 1 + 函数行跨内 llvm-cov BRDA 条数
  （与 v150/v152 的复杂度来源不同，门禁结论一致方向）。
- **textfield.rs 4 未覆盖行（登记不硬凑）**：340-341 + 359/368——本轮逐行吻合
  （[340, 341, 359, 368]），结构性不可达防御分支，登记维持。
- **cargo-mutants / cargo-llvm-cov 安装态（更新）**：cargo-llvm-cov 0.9.1 本轮已装
  （`~/.cargo/bin`）；cargo-mutants 仍缺位（QA 轮按纪律未运行变异测试，hardender
  职责），后续 hardender 开工需按 notes/cargo-mutants.md 安装。
- **D40 / PC-09-场景09 待操作者裁决（沿袭）**：状态不变。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D43、MOUSE_SETTLE=50ms / MOUSE_SETTLE_RETRY=20ms、
  drain poll 超时 5ms。本轮零新增（QA 轮产品代码零改动，不涉产品契约值）。

## 二、当前产出情况（QA ezr-qa-final2）

### 本会话产物清单
- **`qa/runners/harness.py`（+`conns_badge` 共享助手）**：并发连接面板标题徽标
  `(x, y)` 区域锚定读取（v1.34/FR-01-96 ` x/y 项 `；右栏 buffer 列空间，防「下载任务」
  面板同形态徽标撞车）——区域锚点类助手收口共享 harness，两套件（ui_conns、
  modify_task）共用一处修正。
- **`qa/runners/suite_ui_pieces.py`（v1.32→v1.35 同步，移交欠账清偿）**：UP-01 弹窗
  64×19→68×20（`r-l==67`/`bottom-ty==19`）、居中期望 (120−68)//2=26；UP-04 注记
  一屏 341 块/网格 11 行；UP-07 重开居中 68 列口径；头部同步基线补 v1.35。
- **`qa/runners/suite_ui_conns.py`**：UC-07 详情字段名精简映射（任务 ID→ID、
  创建时间→创建、保存→路径，D42）；UC-08 活跃计数锚点 `活跃 x`→`x/y 项` 徽标；
  UC-10 五态空明细锚点同步（全局正则收窄面板标题行）；UC-17 DIM 断言循环更新
  v1.34 字段名全集（旧「任务 ID/保存」因跳过守卫静默脱断——反向自检捕获）；
  UC-18 无热区点击锚点「URL 上方一格（历史校验区）」→「大小字段行」（v1.34
  复制热区扩展至路径后，URL 上一行为路径热区）。
- **`qa/runners/suite_modify_task.py`**：`_active_conns` 观测口径更新（徽标 x =
  活跃明细行数，v1.34 FR-01-81 is_active 单源；旧「速度 > 0」双口径退役）；
  MT-09 下拉展开导航 `up`→`tab×5` 环绕（v1.34 二批修订：文本字段内方向键 =
  光标移动不跳焦点，产品源码 + 单测佐证）。
- **`qa/runners/suite_tui_display.py`**：TD-03 字段名精简映射；TD-19 帮助末行 note
  与 TD-21 删除框提示行改反相断言（v1.34/FR-01-107、FR-01-25 P10 定版退役，
  全屏检索为零）。
- **`qa/runners/suite_download_engine.py`**：DE-03 进度判定改磁盘落盘口径
  （v1.34/FR-01-116 ①：不支持续传流式下载列表行恒 `?%` 无字节文本，显示读数
  正则永不可命中；单流写 `<name>.downloading` 分部文件完成才改名——分部 +
  最终名合计判定 ≥30%）。
- **`qa/runners/suite_throttle_proxy.py`**：TP-07 页脚断言「Space 暂停/继续」→
  页脚命令 chips（v1.13/FR-01-107 页脚收敛，「暂停」属 footer.rs 退役词表）。
- **`qa/04-ui-conns-qa.md`（规程同步）**：UC-18 行措辞同步（非热区锚点漂移注记，
  与脚本同轮一致）。
- **`packs/_common/engineering.md`（经验沉淀 ×2）**：①定位式断言循环的容错跳过
  守卫会把锚点漂移转化为静默覆盖缺失（反向自检要覆盖「跳过守卫 + 旧标识」组合）；
  ②分部写盘系统的进度断言锚分部文件不锚最终名（temp+rename 下最终名 0→全量
  跳变）。
- **`packs/_common/notes/rust.md`（经验沉淀 ×2）**：①rustup `component add` 报
  `detected conflict` = 部分安装残留，component 层无法自愈，须 toolchain 卸载重装；
  ②cargo-llvm-cov 独立 target-dir 双倍磁盘预算 + 分目标 lcov 分片与并集合并口径。
- **环境（沙箱重建，随交接）**：rustup 1.29.2 + nightly 1.101.0-nightly
  （12433c307 2026-10-24，rustfmt/clippy/llvm-tools 齐）经 rsproxy.cn 镜像安装
  （`RUSTUP_DIST_SERVER` / `RUSTUP_UPDATE_ROOT` 操作者钦定；残缺工具链冲突按
  notes/rust.md 新条目处置）；`~/.cargo/config.toml` rsproxy-sparse crates 镜像 +
  `RUSTUP_TOOLCHAIN=nightly-x86_64-unknown-linux-gnu`。新 shell 需
  `export PATH="$HOME/.cargo/bin:$PATH"`。度量工具 `.tools/`：PMD 7.28.0
  （132947834B 逐字节合 engineering.md 登记值，`-l rust` 阳性对照过）、
  cargo-llvm-cov 0.9.1、Babashka 1.13.227（`~/.local/bin/bb`）+ APS 工具仓
  `.tools/Acceptance-Pipeline-Specification`（codeload main 分支 tarball）。
  cargo-mutants 仍缺位。磁盘预算注意：llvm-cov 插桩轮需 2× 构建树
  （本轮曾写满 9.9G rootfs，删 target/debug 后 4.9G 可用）。

### 验证证据

| 验证项 | 结果 | 命令/方法（可复现） |
|---|---|---|
| 全量测试 | **4564P / 0F**（15 目标，逐目标复算自洽：bin 614 + fixture 30 + proxy 15 + hardening 系 12 文件） | `cargo test --tests -- --test-threads=2`（project/ezr；本轮逐目标分片执行，各目标 rc=0） |
| clippy 零警告 | 0（-D warnings 门禁，nightly 无新 lint） | `cargo clippy --all-targets --all-features -- -D warnings` |
| fmt | clean | `cargo fmt -- --check` |
| 架构护栏 | 16/16 规则通过 | `bash scripts/arch_check.sh`（project/ezr） |
| 属性测试 | **71P / 0F**（数量守恒） | `cargo test property_tests:: -- --test-threads=2`（独立命令独立计数） |
| e2e 套件（腐化探针→同步后） | 首轮 **179 例 166P/13F** → 逐例归因全部套件陈旧（零产品回归）→ 同步后 **179P / 0F / 0S** | `python3 suite_<名>.py`（project/qa/runners；throttle_proxy 用 `QA_TP_ONLY` 分片合账） |
| 行覆盖率 | **95.2%**（19260/20221，15 目标 lcov 并集） | `cargo llvm-cov --lcov --output-path <t>.info --test <t>` ×15 + 合并（`.work/tmp/merge_crap.py`，会话级） |
| CRAP 门禁 | 非豁免域 **0 条** >30（豁免域 handle_socks5 并集口径 1.1） | 同上合并口径 comp=1+BRDA |
| DRY/CPD | **125 tiles**（与 v153-arch 基线逐位一致，零漂移） | `.tools/pmd-bin-7.28.0/bin/pmd cpd -l rust --minimum-tokens 50 --dir src`（exit 4 = 有重复属正常语义） |
| APS 机检 | **14/14 feature parser ok + dry-checker 全过（rc=0）**，v132/v149 沿袭欠账清偿 | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-parser … && bb gherkin-ir-dry-checker …)` |
| 产物一致性 | qa/ 14 规程 ↔ runners/ 14 脚本头注直引 14/14 OK | 逐对 head-grep 核对 |
| 产品区零改动 | 本轮 git 触碰面仅 qa/ 规程 ×1 + runners/ 脚本 ×7 + harness ×1 + packs/ 经验 ×2 | `git status` / 逐文件核对 |

### 对账口径（与 architect v153-arch 基线）
- 测试计数：4564 → 4564 = **0 增减**（QA 轮产品区与测试区零改动）；e2e 用例数
  179 → 179 = 0 增减（仅断言锚点与判定口径同步，不增删用例）。
- 源码触碰面：产品区零改动；具体文件 `qa/runners/{harness,suite_ui_pieces,
  suite_tui_display,suite_ui_conns,suite_modify_task,suite_download_engine,
  suite_throttle_proxy}.py`、`qa/04-ui-conns-qa.md`、
  `packs/_common/{engineering.md,notes/rust.md}`（经验沉淀）。
- 度量口径：覆盖率/CRAP 为 15 目标 lcov 并集口径（v152 的 95.2% 基线可直比，
  总 % 一致）；CPD 工具版本与开关沿 v152/v153 口径（PMD 7.28.0，min-tokens 50，
  src）125 tiles 直比一致；CRAP 复杂度来源与 v150 口径不同（BRDA vs 其来源），
  门禁结论（非豁免域 0 条）与豁免登记（handle_socks5）均如实登记差异。

### 待办与待批（未决项）
- **沿袭项全部不变**（见第一节遗留受限项）；本轮无新增阻塞、无新增待批。
- D40 / PC-09-场景09 待操作者裁决（沿袭，状态不变）。
- cargo-mutants 缺位（沿袭；本轮 QA 职责边界内无需变异测试，如需加固轮由
  操作者显式启动 hardender）。

### 移交建议
- **six-pack 流转路径（specifier → coder → cleaner → architect → hardender →
  QA → Done）已走完，工作流完结**：v1.35 全量条款落地、4564P/0F、clippy 0、
  fmt clean、arch 16/16、e2e 179P/0F、覆盖率 95.2%、CRAP 门禁清零、CPD 125
  tiles、APS 机检 14/14——phase-01 收官态。
- 需要 specifier、coder、cleaner、architect、hardender 合并吸收的决策（QA 末角色
  汇总）：①e2e 断言锚点一律以 `harness.conns_badge` 等共享区域助手承载，新增
  面板级断言先查 harness 既有助手（DRY 收口）；②断言循环禁止对「必存元素」用
  跳过守卫（engineering.md 新条）；③进度类断言锚分部文件（engineering.md 新条）；
  ④文本字段内方向键导航锚点不得再入套件（v1.34 二批修订定版，MT-09 同步注）；
  ⑤后续规格演进轮的退役文案清单（`活跃 x`、`任务 ID`、`保存`、页脚 chips 前文案、
  帮助末行 note、删除框提示行、pieces 64×19）即退役标识反向自检的固定检索词集。
- 后续如需新需求：由操作者发起新工作流会话（如 six-pack/specifier 建 phase-02
  分期详述）；`project/` 工作区跨会话保留可直接续用。
