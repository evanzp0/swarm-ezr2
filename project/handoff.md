# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender　会话：v137-hard（任务 v137-hard）
> 本轮（v1.23 后变异加固轮）：对 v1.21–v1.23 触碰面三文件做清单制差分变异（95 突变体
> 94 杀 + 1 书面等价），补 9 条加固测试杀灭 7 个存活体；新沙箱工具链全量重建
> （cargo-mutants / PMD / cargo-llvm-cov / Babashka+APS）；CPD/覆盖率/CRAP 三度量
> 复测并登记新基线；软 Gherkin 变异按 APS 受限口径登记（parser+dry-check 全过）。
> 产品 **2733P/0F**（14 测试二进制）、clippy 0、fmt 0、arch 14 过、产品源码零改动
> （pristine 逐文件 cmp）。

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
| architect（v135/v136 三段 = v1.21–v1.23） | 合并轮：FailedPaused 变体退役（8 态）+ 恢复一律重新排队 + 挂起态展示统一 + v1.23 数据统一（挂起即重置 error=None/retries=1、has_error 字段整体退役、迟到失败守卫 Paused 全态整体忽略）+ 规格三件套同步 + ezr-demo 同步 + pty 回归 7/7 | 已交付 | 产品 2597P/0F、clippy 0、fmt 0、arch 14 过；demo verify_ezr 134/0 + 弹窗 3/3 + 覆盖层 10/10 + pty 7/7 |
| hardender（v137-hard，本轮） | **v1.21–v1.23 触碰面变异加固**：model/slots.rs 35/35、app/tasks.rs 30/30、app/engine/evt.rs 29/30+1 等价论证；新增 tests/hardening_g_slots.rs（3 测试）+ tests/common/fixture.rs（共享 fixture）+ hardening_g_appcore +2 + hardening_g_engine +4；CPD 52 tiles / 覆盖率 94.36% / CRAP 门禁 0 条三基线复测登记；APS parser+dry-check 13/13 error 0 | 已交付 | **2733P/0F**（14 二进制）、clippy 0、fmt 0、arch 14 过、**产品源码零改动** |

### 遗留受限项（当前有效）
- **软 Gherkin 验收变异受限**（沿袭 v127 口径，本轮部分收敛）：13 份 feature 的
  parser + ir-dry-check 已全过（error 级 0）；`gherkin-mutator` 运行仍受限——运行器
  适配器与验收入口生成器为会话级工具不随工作区流转，且 e2e 六套件滞后（见下），
  变异判定基座不可信。01-persistence-config.feature 保有历史 stamp
  （sha256=57e757…，28/28 killed），差分复用跳过条件见「当前产出」③。
- **e2e 六套件滞后于规程**（沿袭，v135 扩大）：`suite_add_task.py`、`suite_ui_conns.py`、
  `suite_modify_task.py`、`suite_persistence_config.py`、`suite_tui_display.py`、
  `suite_integrity_check.py`——v1.21–v1.23 改变可观测行为（挂起入 Paused、恢复一律
  重新排队、满槽 toast 退役、连接级累计清零、挂起数据重置），套件同步前 e2e 不可信
  （移交 QA 会话先同步）。
- **demo QA 脚本二进制路径为绝对路径**（`scripts/verify_ezr.py` PROG 常量）：现指向
  `/home/z/my-project/project/ezr-demo/target/release/ezr-tui-demo`；工作区迁移时须同步。
- **supervisor 高 comp/覆盖残余**（沿袭 v129 口径收窄）：block_worker
  （supervisor.rs:873-1064，comp=19）error/abort 边仍有 24 行未覆盖（文件打开失败、
  在传写盘失败、配额下调在传中止 D30 臂、连接失败分类臂、再领队列臂等）；CRAP 两口径
  已压线达标（29.9/25.8 < 30），模块拆分议题留 coder/specifier 路径再议。
- **工具链位置（本会话重建，跨会话缓存 `.tools/`）**：rustup nightly
  1.101.0-nightly（29c5daa1a 2026-10-07；rust-toolchain.toml 钉定自动装，新 shell 须
  `export PATH="$HOME/.cargo/bin:$PATH"`）；cargo-mutants 27.1.0 / cargo-llvm-cov 0.9.1
  （prebuilt，`.tools/bin/`）；PMD 7.28.0（`.tools/pmd-bin-7.28.0/`）；Babashka
  1.13.226 + APS（`.tools/bin/bb` + `.tools/Acceptance-Pipeline-Specification/`）。
  磁盘口径：llvm-cov 插桩全量重编需 ~3G，跑前清主 target、跑后清
  `target/llvm-cov-target`。
- 无其他受限项。

### 实现定义值登记（当前有效）
- 沿袭有效：D28/D29（仅添加落态与三钮布局）、D30（下调即时收缩）、D31（1MB 下限）、
  D32（Enter 语义）、D33（1s 归零时延）、D34（清码 R = 文件完整性直判 + fallback 锚定）。
- v1.21–v1.23 恢复 toast 文案（规格已并入 FR-01-33）：可续传
  `▶ 继续下载（从断点恢复，已进入等待队列）: {name}`、不可续传
  `⚠ 服务器不支持断点续传，将从头下载（已进入等待队列）: {name}`。
- 无未决待批项。

## 二、当前产出情况（hardender v137-hard）

### 本会话产物清单
**产品树新增（全部为加固测试，src/ 零改动）**
- `ezr/tests/hardening_g_slots.rs`（新增）：3 测试杀 model/slots.rs 5 存活体
  （enforce_invariants Downloading|Verifying 恒持槽臂 + queue_positions 状态/槽位过滤
  与位次累加 4 点）；`#[path]` 挂载 model 全树。
- `ezr/tests/common/fixture.rs`（新增）：ezr-fixture 服务进程拉起 + FxGuard 生命周期
  守卫（appcore/g_engine 共享，防 CPD 重复 tile）。
- `ezr/tests/hardening_g_appcore.rs`（+2，纯新增）：`direct_finalize_total_zero_empty_
  file_not_intact`（靶 223:19：total=0 空文件不构成「文件已完整」直判依据）+
  `clear_completed_spares_live_task_conn_ledger`（靶 272:33：经真实下载填充连接级累计
  后断言清理不波及未完成任务账本）。
- `ezr/tests/hardening_g_engine.rs`（+4，纯新增）：`failure_at_limit_toast_reports_
  exhausted_retries`（靶 340:13：max_retries=2 真实走满自动重试后达上限 toast 口径）+
  block_worker 三测试（多块完成 mark_done、暂停在传中止保留部分块、中途断连瞬时失败
  分类——将 block_worker 覆盖 84→98/122 行，CRAP 37.8→29.9 压线达标）。

**工具侧（packs/）**
- `notes/rust.md`：+1 条（llvm-cov 跨实例 MAX 归并 + 协程 segments 低估 + --branch）。
- `notes/cargo-mutants.md`：+2 条（--in-place 与 -j 互斥；stdout 块缓冲、以台账为准）。
- `notes/mutation-hardening.md`：+§8（前台调用窗口受限沙箱的 in-place+pristine 双端
  cmp 纪律，baseline 兼作清场金探针）。
- `notes/pmd.md`（新增）+ `INDEX.md` 补索引：CPD 退出码 4=发现重复、XML 命名空间、
  `-l rust` 显式作用域。

**交接四要素（hardender）**
1. **本地验证命令清单**（按序，cwd `project/ezr`）：
   `CARGO_PROFILE_DEV_DEBUG=0 cargo test --all-targets`（2733P/0F，14 二进制）→
   `cargo clippy --all-targets`（0）→ `cargo fmt --check`（0）→
   `bash scripts/arch_check.sh`（14 过）→ 加固套件入口（独立可跑）：
   `cargo test --test hardening`（124P）/ `--test hardening_g_slots`（130P）/
   `--test hardening_g_appcore`（294P）/ `--test hardening_g_engine`（289P）/
   其余 hardening_* 与 v115 套件；属性测试独立命令 `cargo test property_tests::`
   （不并入变异/CRAP/覆盖率被测命令）；覆盖率/CRAP：`cargo llvm-cov --all-targets
   --branch --json`（94.36%）；DRY：`pmd cpd -l rust --minimum-tokens 50 --dir src`
   （52 tiles）。
2. **会话运行时产物**（`.work/tmp/`，`complete` 时清除，复现方法）：
   变异会话 `mutants-out-{slots,tasks,evt}/mutants.out/`（台账+逐突变体日志）经
   `cargo mutants --in-place --iterate --no-shuffle -t 240 --minimum-test-timeout 60
   --build-timeout 540 -o <out> -f <file> -- --bin ezr --test hardening_g_slots
   [--test hardening_g_appcore [--test hardening_g_engine]] -- --skip property_tests::`
   复现；覆盖率 `cargo llvm-cov --all-targets --branch --json`（跑前清主 target，
   跑后清 target/llvm-cov-target）；CRAP 计算脚本口径：comp = 1 + 函数分支区数
   （--branch 下），cov = 跨实例 MAX 归并行覆盖，CRAP = comp²(1−cov)³+comp，门禁 >30；
   pristine 基线 `.work/tmp/pristine/`（字节级源备份）。
3. **feature 注释块与差分复用**：本轮未修改任何 feature；`01-persistence-config.
   feature` 头部 mutation-stamp（sha256=57e757…）与 manifest 注释块由工具自管理——
   内容未变时差分复用跳过全部场景，强制重跑需删除该注释块（移交 QA 会话处置）。
4. **等价突变体清单及失效条件**（本轮共 1 条，非等价存活率 0）：
   - `src/app/engine/evt.rs:297:23 replace > with >= in App::handle_failure`
     （downloaded > 0 守卫）：`downloaded: u64` 恒非负，守卫体
     `t.downloaded = downloaded.max(t.downloaded)` 在 downloaded=0 时恒等幂
     （0 为 u64 最小元），守卫纯优化无语义。**失效条件**：① downloaded 改有符号类型；
     ② max 归并改直接赋值/其他合并语义（「事件值覆盖」式改写）；③ 该守卫体新增副作用。
     回补触发：任一失效条件成立时，补「downloaded=0 失败事件不回退已下字节数」定向
     加固测试（当前行为上不可区分，无测试可杀）。

### 验证证据
| 验证项 | 结果 | 命令（cwd `project/ezr`） |
|---|---|---|
| 全量测试 | **2733P / 0F**（14 测试二进制） | `CARGO_PROFILE_DEV_DEBUG=0 cargo test --all-targets` |
| clippy / fmt / arch | 0 / 0 / 14 过 | `cargo clippy --all-targets` / `cargo fmt --check` / `bash scripts/arch_check.sh` |
| 变异（差分，清单制） | slots 35/35、tasks 30/30、evt 29/30+1 等价 | `cargo mutants --in-place --iterate …`（见四要素②） |
| 杀灭链实证 | 抽查日志含真实 `test result: FAILED` | `mutants.out/log/*.log` |
| 覆盖率 | 94.36% 行覆盖（v132 基线 94.28%） | `cargo llvm-cov --all-targets --branch --json` |
| CRAP 门禁（>30） | **0 条**（block_worker 37.8→29.9） | 覆盖率 JSON + 交接四要素②口径 |
| CPD（min 50 tokens） | **52 tiles**（v132 基线 51；触碰面 0 涉入） | `pmd cpd -l rust --minimum-tokens 50 --dir src` |
| APS parser + dry-check | 13/13 feature，**error 级 0**（advisory 554 条 = 中文分词已知假阳性） | `bb gherkin-parser` + `bb gherkin-ir-dry-checker`（APS 目录内调用） |
| 产品源码零改动 | src/ 字节级一致；tests/ 纯新增（2 文件 diff 0 删改行） | `diff -r .work/tmp/pristine/src project/ezr/src` |

### 对账口径（基线 → 终态）
- 测试计数：上游 2597P（13 二进制）→ 本轮 2733P（14 二进制）。**+9 条唯一新测试**
  （slots 3 + appcore 2 + g_engine 4）+ **127 条为新增 hardening_g_slots 测试 crate
  重挂载 model 树内嵌单测的重复计数**（#[path] 挂载机制固有，与既有 hardening_g_*
  同形态），分项复算：2597 + 9 + 127 = 2733 ✓。
- clippy 0 / fmt 0 / arch 14：与 v132–v136 基线持平。
- 覆盖率 94.28% → 94.36%（+0.08pp，9 条加固测试）；CRAP 门禁 76 条（v132 口径）→
  0 条：本轮按 llvm-cov 0.9.1 --branch 重立口径（comp=1+分支区数、cov=跨实例 MAX
  归并行覆盖），v132 会话级脚本已不存、口径不可复现——新基线以本轮口径为准；两口径
  （行覆盖/区域覆盖取大）收敛一致。
- CPD 51 → 52 tiles：+1 为 v135/v136 model/app/ui 大面积重构的预计形态变化（v136
  交接已预告），触碰面三文件（slots/tasks/evt）0 涉入；逐 tile 明细见
  `.work/tmp/cpd-src.xml`（会话级，复现命令见上）。
- 变异对账：95 突变体 = 94 killed + 1 等价登记；unviable/timeout 0；产品树零残留。

### 待办与待批（未决项）
- 无新增未决待批项。
- 沿袭移交：e2e 六套件同步 + gherkin-mutator 运行器适配器/验收入口生成器基建
  （移交 QA 会话；APS 工具已就位）。

### 移交建议
- 下一角色：**six-pack/QA**（链序 hardender → QA，宪法第一章不自动流转，由操作者
  显式 `bin/swarm begin six-pack/QA`）。建议要点：①e2e 六套件按 v1.21–v1.23 行为
  变更同步断言面（挂起入 Paused、恢复一律重新排队、满槽 toast 退役、连接级累计清零、
  挂起数据重置）；②APS 已安装（`.tools/`），建 gherkin-mutator 运行器适配器与
  验收入口生成器，跑软 Gherkin 变异与 feature dry-check；③01-persistence-config
  stamp 差分复用按四要素③处置。
- 操作者要求新增 feature 时：直接 `bin/swarm begin six-pack/specifier`。
