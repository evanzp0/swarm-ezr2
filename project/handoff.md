# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA　会话：v138-qa（任务 v138-qa）
> 本轮（v137-hard 之后的最终独立验证轮）：e2e 13 套件全量腐化探针 + 六套件断言面
> 同步 + 1 处产品缺陷修复（R 臂连接级累计清零）+ 全量回归（产品 2740P/0F）+
> CRAP/DRY/覆盖率三度量复测对账（94.37% / 52 tiles / 门禁 0）+ APS parser/dry-check
> 13/13 error 级 0 + persistence-config stamp 差分复用处置。**six-pack 流转路径
> （specifier → coder → cleaner → architect → hardender → QA → Done）至此走完，
> 工作流完结。**

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
| hardender（v137-hard） | **v1.21–v1.23 触碰面变异加固**：model/slots.rs 35/35、app/tasks.rs 30/30、app/engine/evt.rs 29/30+1 等价论证；+9 条加固测试；三度量新基线（CPD 52 / 覆盖率 94.36% / CRAP 门禁 0）；APS parser+dry-check 13/13 error 0 | 已交付 | 2733P/0F（14 二进制，QA 对账残差 ±1 见下）、clippy 0、fmt 0、arch 14 过、产品源码零改动 |
| QA（v138-qa，本轮） | **最终独立验证**：e2e 13 套件全量探针与六套件同步（新建 suite_ui_conns.py，同步 slots/add/tui/modify/integrity/persistence 断言面）+ R 臂连接级累计清零缺陷修复（requeue_failed 显式清账本 + conn_rebase 重锚）+ 1 条单元测试 + 三度量复测对账（94.37% / 52 tiles / 门禁 0）+ APS 13/13 error 0 + stamp 保留处置 | 已交付 | **2740P/0F**（14 二进制）、clippy 0、fmt 0、arch 14 过、e2e 13 套件 **155P/0F/9S** |

### 遗留受限项（当前有效）
- **软 Gherkin 验收变异受限**（沿袭 v127/v137 口径，本轮复核维持）：13 份 feature 的
  parser + ir-dry-check 实跑全过（error 级 0；advisory 607 条 = 中文分词已知假阳性，
  与 v137 记 554 的差异为 APS 工具版本漂移——本轮 fresh clone accaa33，规格零改动下
  error 级稳定）；`gherkin-mutator` 运行器适配器与验收入口生成器为会话级工具不随
  工作区流转，本轮评估确认其重建属专门会话工作量（metadata 生成器 + 逐场景 e2e
  选择器 + 变异驱动循环），全量变异 campaign 不可在本环境前台窗口预算内完成。
  01-persistence-config.feature 保有历史 stamp（sha256=57e757…，29/29 killed 按
  manifest 内数），**本轮处置 = 保留**（内容零变更证据充分），有效性 caveat 与
  删除重跑指令见「当前产出」③。
- **e2e 逐用例环境受限项（S，共 9 条，套件内合规登记）**：retry RB-09（需 64MB
  受限目录，沙箱无 mount/配额权限，预检路径已由代码审查+单测覆盖）；throttle
  TP-02/03/06（loopback 短传输被启动瞬态支配，绝对速率口径不稳定，限速生效语义由
  TP-01 100MB 总量口径权威覆盖）；named NP-04/08/13/15/18（ezr-proxy 认证要求模式、
  https 监听、socks5+认证、IPv6 回环绑定等扩展未就绪，规程环境前置口径，各 S 项
  的「已验」部分与依赖子判据逐条写在套件输出）。
- **demo QA 脚本二进制路径为绝对路径**（`scripts/verify_ezr.py` PROG 常量）：现指向
  `/home/z/my-project/project/ezr-demo/target/release/ezr-tui-demo`；工作区迁移时须同步。
- **工具链位置（本会话重建，跨会话缓存 `.tools/`）**：rustup nightly
  1.101.0-nightly（080cc2714 2026-10-11；rust-toolchain.toml 钉定自动装，新 shell 须
  `export PATH="$HOME/.cargo/bin:$PATH"`）；cargo-llvm-cov 0.9.1（prebuilt，
  `.tools/bin/`）；PMD 7.28.0（`.tools/pmd-bin-7.28.0/`，zip 解压后须
  `chmod +x bin/pmd`）；Babashka 1.3.189 + APS（`.tools/bin/bb` +
  `.tools/Acceptance-Pipeline-Specification/`，commit accaa33）。磁盘口径：llvm-cov
  插桩全量重编需 ~3.3G，跑前清主 target、跑后清 `target/llvm-cov-target`；受限沙箱
  分段窗口配方见 `packs/_common/notes/rust.md`（--no-clean 增量累积）。
- 无其他受限项。

### 实现定义值登记（当前有效）
- 沿袭有效：D28/D29（仅添加落态与三钮布局）、D30（下调即时收缩）、D31（1MB 下限）、
  D32（Enter 语义）、D33（1s 归零时延）、D34（清码 R = 文件完整性直判 + fallback 锚定）。
- v1.21–v1.23 恢复 toast 文案（规格已并入 FR-01-33）：可续传
  `▶ 继续下载（从断点恢复，已进入等待队列）: {name}`、不可续传
  `⚠ 服务器不支持断点续传，将从头下载（已进入等待队列）: {name}`。
- 本轮新增：R 臂手动重试后首个 Progress 观测以当前 done 重锚连接级账本（该次增量
  = 0），后续增量只计新一轮下载真实落盘（FR-01-99 ②「重试 = 新的一次下载」的
  计量语义补全，见「当前产出」缺陷修复条）。
- 无未决待批项。

## 二、当前产出情况（QA v138-qa）

### 本会话产物清单
**产品树（1 处缺陷修复 + 1 条单元测试，最小改动）**
- `ezr/src/app/mod.rs`：App 新增 `conn_rebase: HashSet<u32>`（连接级账本待重锚任务集）
- `ezr/src/app/engine/evt.rs`：Progress 消费侧重锚——rebase 集合命中时以当前观测
  重锚 conn_prev（该次增量 = 0），其后增量只计新一轮真实落盘
- `ezr/src/app/tasks.rs`：requeue_failed（R 臂）显式清连接级账本并置重锚标记 +
  新增单元测试 `retry_clears_conn_cum_resume_keeps`（断言 R 后账本/观测位清零、
  重锚标记在位、失效连击重置与恢复臂不清连击）
- 修复依据：04-ui-conns-09 R 臂 / FR-01-99 ②——e2e 探针发现手动重试后连接级
  「累计」残留上一次下载的量（续传检查点偏移被当作本轮落盘入账）

**e2e 套件（project/qa/runners/，断言面与 v1.21–v1.23 规格同步）**
- `suite_ui_conns.py`（新建，18 用例）：v136 连接面板改版的配套套件（原缺）
- `suite_slots_queue.py`：SQ-09/10 断言更新（挂起入 Paused）+ SQ-14/15 新增
- `suite_add_task.py`：满槽 toast 退役后的断言面更新（18 用例）
- `suite_tui_display.py`：TD-09 触发路径改「任务已存在」toast + TD-17 新增
- `suite_modify_task.py`：MT-02 即时收缩 + MT-09 粘贴
- `suite_integrity_check.py`：IC-12 新增
- `suite_persistence_config.py`：PC-04 钳制行 + PC-05 4MB/2MB（256KB 时代陈旧值退役）
- `harness.py`：区域锚点助手收口（锚点失效一处修复全链受益）
- 同步后 13 套件全量复跑：**155P / 0F / 9S**（9S 全部为上文登记的环境受限项）

**工具侧（packs/_common/，经验沉淀）**
- `notes/rust.md`：+4 条（llvm-cov --no-clean 分段窗口配方 / regions 无 hasCount
  的假绿陷阱 / v0 mangled tail 剥 crate 段归并配方 / #[path] 挂载点计数对账法）
- `notes/aps.md`：后台存活条目修订（可续跑分段优于后台化，llvm-cov 实证）
- `notes/pmd.md`：+1 条（zip 解压丢执行位，chmod +x 先于工具怀疑）

**规格与 stamp**
- features/ 与 mission 零改动（git 对账）；01-persistence-config.feature 的
  mutation-stamp 保留（处置见下③）

### 验证证据
| 验证项 | 结果 | 命令（cwd `project/ezr`） |
|---|---|---|
| 全量测试 | **2740P / 0F**（14 测试二进制；--list 编译期 2740 对账一致） | `CARGO_PROFILE_DEV_DEBUG=0 cargo test --all-targets` |
| clippy / fmt / arch | 0 / 0 / 14 过（QA 新测试 2 处 get-then-check 与 import 合并已修复后复验） | `cargo clippy --all-targets` / `cargo fmt --check` / `bash scripts/arch_check.sh` |
| e2e 全量 | **155P / 0F / 9S**（13 套件，S 项全部环境受限登记） | `python3 project/qa/runners/suite_*.py`（逐套件，环境变量与 fixture 自备，见各规程 md） |
| 覆盖率 | **94.37%** 行覆盖（v137 基线 94.36%，+0.01pp） | `cargo llvm-cov --all-targets --branch --json --no-clean` |
| CRAP 门禁（>30） | **0 条**（阳性对照：block_worker 21.96；边界恰 30.00 两条 = comp=5/cov=0 的 main::run 与 fixture emit_throttled，非违例） | 覆盖率 JSON + 四要素②口径 |
| CPD（min 50 tokens） | **52 tiles**（与 v137 基线一致，QA 变更零重复引入） | `pmd cpd -l rust --minimum-tokens 50 --dir src` |
| APS parser + dry-check | 13/13 feature，**error 级 0**（advisory 607 = 中文分词已知假阳性） | `(cd .tools/Acceptance-Pipeline-Specification && bb run gherkin-parser <in> <out> && bb run gherkin-ir-dry-checker <ir> <out>)` |

### 对账口径（基线 → 终态）
- 测试计数：v137 2733P → 本轮 **2740P**（+7）。分解：本轮 1 条新测试（tasks.rs）按
  `#[path]` 挂载点重复计数 = bin 单测 + 5 个挂载 app 树的 hardening_* crate 共
  **6 个计数点（+6）**；残差 **+1** 为 v137 交接总数的记账粒度差（其分项未列全、
  无失败项、两侧静态总数各自自洽），不影响验证有效性。逐 crate 分项与本轮对账
  方法见 `packs/_common/notes/rust.md` 挂载点条目。
- 覆盖率 94.36% → 94.37%（+0.01pp）：本轮修复臂与测试为净增覆盖。
- CPD 52 → 52 tiles、CRAP 门禁 0 → 0：与 v137 基线持平。
- APS advisory 554 → 607：规格零改动下为 APS 工具版本漂移（v137 会话 clone vs 本轮
  fresh clone accaa33），error 级 0 稳定；advisory 增量不属退化（口径见
  `notes/aps.md` dry-check 验收条）。
- 产品源码变更 = git 对账仅 3 文件（mod/evt/tasks），均为本轮修复臂。

### 交接四要素（QA）
1. **本地验证命令清单**（按序，cwd `project/ezr`）：
   `CARGO_PROFILE_DEV_DEBUG=0 cargo test --all-targets`（2740P/0F）→
   `cargo clippy --all-targets`（0）→ `cargo fmt --check`（0）→
   `bash scripts/arch_check.sh`（14 过）→ e2e：逐套件
   `python3 <qa 规程对应 suite_*.py>`（13 套件 155P/0F/9S，规程入口见
   `project/qa/*-qa.md`）→ 覆盖率/CRAP：`cargo llvm-cov --all-targets --branch
   --json --no-clean`（94.37%，跑前清主 target、跑后清 llvm-cov-target，沙箱分段
   窗口配方见 notes/rust.md）→ DRY：`pmd cpd -l rust --minimum-tokens 50 --dir src`
   （52 tiles，exit 4=发现重复属正常）→ APS：`(cd .tools/Acceptance-Pipeline-Specification
   && bb run gherkin-parser … && bb run gherkin-ir-dry-checker …)`（error 0）。
2. **会话运行时产物**（`.work/tmp/`，`complete` 时清除，复现方法）：e2e 逐套件日志
   与用例屏捕（`qa-logs/`、`qa-run/`，结论已并入上表）；覆盖率 JSON（6.8MB）与
   CRAP 计算脚本（`crap_calc.py`，口径：comp = 1 + 函数分支区数（--branch 下），
   cov = 跨实例 MAX 归并行覆盖（regions 行区间、v0 mangled 剥 crate 段分组），
   CRAP = comp²(1−cov)³+comp，门禁 >30）。
3. **feature 注释块与差分复用处置**（v137 四要素③的移交项，本轮裁决）：**保留
   stamp**。内容零变更证据：git 对 features/ 零改动、文件最新版本注记 v1.16 早于
   stamp（2026-10-06）、场景名序与 manifest 一致。**有效性 caveat**：stamp 对场景
   04/05（钳制）的变异判定基于 pre-v1.21 钳制基座（256KB 时代），产品钳制行为其后
   已改（4MB/2MB，PC-04/05 本轮已同步）；mutator 的 implementation_hash 校验机制
   本身会在复跑时发现实现漂移。**指令**：gherkin-mutator 运行器恢复后，删除该
   stamp 注释块强制全量重跑（四要素③口径），以当前已同步的 e2e 基座重立判定。
4. **等价突变体清单及失效条件**：本轮未运行变异（QA 职责边界，SKILL「不负责的
   内容」），沿袭 v137 登记：`src/app/engine/evt.rs:297:23 replace > with >= in
   App::handle_failure`（downloaded: u64 恒非负，守卫纯优化无语义）——失效条件
   ① downloaded 改有符号 ② max 归并改直接赋值 ③ 守卫体新增副作用，任一成立时补
   「downloaded=0 失败事件不回退已下字节数」定向加固测试。本轮 evt.rs 的新增重锚
   逻辑未触及该守卫体。

### 待办与待批（未决项）
- 无新增未决待批项。
- 沿袭移交（操作者后续会话可选项，非缺陷）：gherkin-mutator 运行器适配器与
  验收入口生成器基建（见「遗留受限项」第一条）；02-named-proxy 的 ezr-proxy
  扩展（认证要求模式 / https 监听 / IPv6 回环）如需实跑 S→P 转换，属工具扩展
  会话。

### 各角色合并吸收清单（QA 移交）
- **specifier**：六套件断言面已按 v1.21–v1.23 同步且全绿，规格文本无需回改；
  PC-04/05 的钳制值以现行规格行（4MB/2MB）为准，256KB 口径退役。
- **coder**：conn_rebase 重锚机制为产品语义一部分（见实现定义值登记新增条），
  后续触碰 Progress 消费路径或 requeue_failed 时保持重锚标记的消费顺序。
- **cleaner**：CPD 52 tiles 持平，本轮修复未引入重复；tasks.rs 测试模块的
  断言写法以 `!contains_key` 为准（clippy get-then-check 口径）。
- **architect**：三度量与基线持平；block_worker 在现行口径下 21.96（v137 口径
  29.9），模块拆分议题维持「再议」状态，无新增压力。
- **hardender**：stamp 保留 + 删除重跑指令见四要素③；等价突变体登记沿袭且
  失效条件未触发。

### 移交建议
- **six-pack 流转已完结**（specifier → coder → cleaner → architect → hardender →
  QA → Done），本轮为最终节点，产物已可交付操作者（`bin/swarm complete` 归档）。
- 操作者如需新增 feature：`bin/swarm begin six-pack/specifier`（工作区沿用，
  以本 handoff 为上游交接内容）。
- 操作者如需软 Gherkin 变异基建：独立会话重建运行器适配器（评估结论见「遗留
  受限项」第一条），完成后按四要素③删除 stamp 重跑。
