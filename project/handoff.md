# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect　会话：architect-20261005

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（ezr download）：TUI 高性能多协议下载器，phase-01（HTTP/HTTPS 真实下载
  内核与 TUI 正式版）已交付。需求权威来源 `project/mission.md`（分期模式），phase-01 详述
  见 `project/mission/phase-01.md`，feature 规格在 `project/features/`，QA 套件在 `project/qa/`。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier | mission.md + mission/phase-01.md + features/ 9 份规格 + qa/ 9 套件 | 已交付 | 上游既定口径 |
| coder（phase-01） | ezr 下载内核 + TUI + ezr-fixture/ezr-proxy + 属性测试 | 已交付 | 上游既定口径（258 测基线） |
| cleaner / architect（历轮） | 工具链钉 nightly、模块边界收敛、两轮架构复核 | 已交付 | arch_check 8/8、clippy 0、fmt 0（其会话口径） |
| hardender（20261004） | 变异加固：7 个 hardening_g_* 套件 ~110 条新测试；等价/超时/环境型台账 | 已交付 | 加固后全绿；equivalence 台账见其归档 |
| coder（20261004） | 空闲期注册表零写放大修复：model 层 `save_json_atomic` 内容幂等短路 + 3 条聚焦单测 | 已交付 | 全量 955P/0F；产品面 clippy 0；arch_check 8/8 |
| cleaner（20261005） | tests/hardening* 环境漂移收敛：14 文件 fmt 归零 + 7 套件 clippy 警告归零；度量流水线全量复跑 | 已交付 | 全树 fmt 0；clippy 0；955P/0F 零漂移；arch_check 8/8；src/ 零改动 |
| **architect（20261005）** | 架构评审四阶段 + lint 姿态块单源化（16 文件 280 行冗余删除）+ engine 窄接口收口（实现子模块私有化 + 门面 re-export）+ 属性测试 24→30 + arch_check 增至 10 规则（含阳性对照） | 已交付 | 961P/0F；clippy --all-targets 0 / --bins 0；fmt 0；arch_check 10/10；CPD 100-token 0 重复；覆盖率 90.4% 持平；变异点 1658/max 88 持平（详见第二节） |

### 遗留受限项（当前有效）
- **CRAP 为受限近似口径**（继承上游）：无标准 CRAP 工具，以 llvm-cov lcov 分支计数代入
  公式估算（comp=函数区间 BRDA 条目数+1，`testss_` 排除测试模块，闭包归并取极值）。
  本轮复测：产品函数 comp>6 或 CRAP>6 共 79 个（cleaner 上轮读数 88；差异来自闭包归并
  口径的区间归属实现不同，同为受限近似，两轮读数不可直接互减），集中于 app(30)/engine(12)/
  ui(12) 交互分发核心。
- **沙箱无 TTY**：TUI 进程级端到端（真实按键/渲染）不可自动化；pty E2E 归 six-pack/QA。
- **gherkin-mutator 不存在于 npm registry**（继承上游）：Gherkin 差分变异受限，feature 无
  步骤处理器绑定。
- **单实例锁无 home 回退分支的 flock 缺失**（继承上游）：待操作者裁决。
- **工具链 PATH 注入不跨调用持久**：`cargo` 等需 `export PATH="$HOME/.cargo/bin:$PATH"`
  （lld 桥接另需 `~/.local/bin`；工具链本体在位，见第二节环境记录）。
- **GitHub release 资产直链在本沙箱 404**（cargo-mutants 仓库）：经 crates.io
  `cargo install cargo-mutants --version 27.1.0` 安装（编译安装，非预编译版）。

### 实现定义值登记
- 无新增（本轮未引入契约值）。既有登记继承上游归档。

## 二、当前产出情况

### 本会话产物清单
- **lint 姿态块单源化（cleaner 移交项 A 裁决）**：CPD 报告"2 处"实为 **16 个文件携带
  字节级相同的 16 行模块级 `#![allow]` 姿态块**（supervisor/config/registry/sidecar/
  consistency/checksum/namegen/retry/speed/model mod/chunk blocks/plan/slots/engine mod/
  error/throttle）；main.rs crate 级 posture 已全量覆盖（含 pedantic/nursery 传递覆盖），
  clippy 实证后全部删除（净 −280 行）；「字节/速度/时间算术…u64-f64 转换豁免」依据注释
  收敛至 `src/main.rs` cast-family allow 处单源。宏/include! 提取路径经语言探针证实不可行
  （内部属性禁止经宏展开拼接，经验已沉淀 notes/rust.md）。
- **engine 窄接口收口（评审阶段 3 产出）**：`engine::{error,supervisor,throttle}` 由
  `pub mod` 收为私有 `mod`；`TaskSpec`/`VerifySpec` 经门面 `pub use supervisor::{...}`
  暴露（app 侧 4 处构造点改走 `crate::engine::{TaskSpec,VerifySpec}`）；`EngineShared.throttle`
  字段私有化（仅 engine 树内消费）。app/ui 从此无法 reach 引擎实现子模块（类型系统强制）。
  挂载测试可达性经 `#[cfg(test)] pub use` 门控门面（classify_reqwest/Throttle；allow
  依据注释注明三分法 ①）。
- **属性测试 24 → 30**：新增 `src/property_tests.rs` ui/text 纯函数 6 属性——显示宽度界
  n≤w(s)≤2n、truncate 宽度界+幂等、pad_right 精确宽度+幂等、fmt_dur parse-back 往返+
  fmt_eta 同口径、fmt_size/fmt_size_pair 后缀封闭域+单位绑定总量、fmt_speed 后缀域+负零
  归一；策略含 CJK/全角混合文本。可达性：`ui/mod.rs` `#[cfg(test)] pub(crate) use` 门面 +
  `text.rs` 8 个纯函数 `pub(super)`→`pub`（私有模块内，产品 API 面零增量）。
- **arch_check 8 → 10 规则**（`scripts/arch_check.sh`）：规则 9【窄接口】engine 实现子模块
  保持私有；规则 10【单源】lint 姿态块不得出现 main.rs 之外的第二拷贝。两规则均做阳性对照
  （临时注入违规→命中非零退出→移除），对照记录见脚本尾注。
- **工具侧沉淀（宪法第三章）**：`packs/_common/notes/rust.md` 新增 3 条 + 扩展 1 条（lint
  姿态块去重正确路径；#[path] 挂载×私有化联动与 E0603/E0364；纯文本工具属性模板；PMD CPD
  报告清单≠全集 + 块注释全角词法错误）；`packs/_common/engineering.md` 新增 1 条（分组/配对
  类报告的发现清单≠全量清单）。
- **环境与工具在位**（本沙箱重建）：rustup nightly `1.101.0-nightly (db8f076d2 2026-10-03)`
  于 `~/.cargo/bin`（与上游同版号）+ llvm-tools-preview；lld 桥接符号链接
  `~/.local/bin/ld.lld → ~/.rustup/toolchains/nightly-*/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld`；
  cargo-llvm-cov 0.9.1（cargo install）；PMD 7.28.0 于 `.tools/pmd-bin-7.28.0`；
  cargo-mutants 27.1.0（crates.io cargo install，GitHub 资产直链 404）。

### 验证证据
| 验证项 | 结果 | 命令（可复现，均在 project/ezr 下） |
|---|---|---|
| 全量测试 | 961P / 0F（+6 属性，10 个测试二进制，EXIT=0） | `cargo test` |
| 属性测试（独立命令） | 30P / 0F | `cargo test property_tests::` |
| fmt 全树 | 0 偏差 | `cargo fmt --check` |
| clippy 全目标 | 0 警告 | `cargo clippy --all-targets` |
| clippy 产品面 | 0 警告 | `cargo clippy --bins` |
| 架构边界 | 10/10 通过 | `bash scripts/arch_check.sh` |
| DRY（CPD） | 100-token 重复 0 处（基线 2） | `pmd cpd --minimum-tokens 100 -l rust src`（PMD 7.28.0，在 .tools/pmd-bin-7.28.0 下调用） |
| 覆盖率 | 合并行覆盖 90.4%（8399 DA 行/7596 命中，与基线持平） | `cargo llvm-cov --branch --lcov --output-path <path>`（0.9.1） |
| 变异点扫描 | 全产品 1658 点；单文件最大 88（lease.rs）；0 文件超 100（与基线持平） | `cargo mutants --list --line-col=true`（27.1.0） |
| 规则 9 阳性对照 | 注入 `pub mod supervisor;` → FAIL 并非零退出 | 已移除，见脚本尾注 |
| 规则 10 阳性对照 | 注入姿态注释于 model/slots.rs → FAIL 并非零退出 | 已移除，见脚本尾注 |

### 对账口径
- 测试计数：955 → 961（+6P/−0F），+6 全部为本会话新增 ui/text 属性
  （prop_text_width_within_bounds / prop_truncate_bounded_and_idempotent /
  prop_pad_right_exact_and_idempotent / prop_fmt_dur_round_trip /
  prop_fmt_size_suffix_domain / prop_fmt_speed_suffix_domain）；既有测试零改动、零删除。
- 源码改动面：25 文件 +208/−310（净 −102 行）。构成：姿态块删除 −280（16 文件）+ app/ui
  姿态块 −24（2 文件，同批裁决）；新增 = 属性测试 ~120 行 + 门面 re-export/文档 ~25 行 +
  arch_check 规则 ~20 行；机械改动 = text.rs 可见性 8 处、app 构造点路径 4 处、
  tests/hardening_g_throttle_err.rs 门面路径 5 处。**产品行为零改动**：全量测试 0F +
  覆盖率/变异点计数与基线持平为证（lint 属性与可见性/路径改动不触碰运行时分支）。
- CRAP 读数 88 → 79：两轮闭包归并实现口径不同（区间归属差异），非同一口径下的改善，
  登记为读数口径差异（见遗留受限项）。
- 环境重建（新沙箱）后的基线复核与上游口径全部一致（955P/0F、clippy 0、fmt 0、
  arch_check 8/8、90.4%、1658/88），无 nightly 工具链漂移。

### 待办与待批（未决项）
- CRAP>6 高 comp 点裁决（移交项 B）：**本轮裁决为不拆**——79 个超限函数集中于交互分发
  核心（dialog_keys/mouse/dialog/main 事件循环、engine on_evt/tick 闭包），按 SKILL 改造
  裁决边界属「依赖共享可变状态与对象生命周期的长分派拆分」不安全类（&mut App 跨模块状态）；
  变异点扫描 0 文件超 100 点，无强拆义务；高 comp 高覆盖函数（ui/dialog.rs draw_add_dialog
  comp 37 cov 0.99、model/consistency.rs check comp 13 cov 1.00 等）CRAP 纯由分支计数驱动，
  表驱动拆分徒增间接层。Top 清单（CRAP 前列：dialog_keys add_dialog comp 19 cov 0.29、
  mouse on_mouse 闭包 comp 29 cov 0.49、supervisor download comp 13-26）已留存
  `.work/tmp/crap_top.txt`（会话级，不入归档；hardender 可按同口径重算），建议 hardender
  变异暴露具体存活体后针对性处理。
- app/engine.rs `on_evt` 部分 arm/closure 覆盖 0%（CRAP 12-56 段）：单测覆盖缺口属
  cleaner/coder 职责，本轮不越权补测，登记移交。
- 继承上游待批：单实例锁无 home 回退分支的 flock 缺失，待操作者裁决。

### 移交建议
- 建议操作者 `bin/swarm complete` 归档本轮产物；如需续跑：`six-pack/hardender`（architect
  的标准下一跳——变异加固兑现本轮登记的高 comp 点拆分裁决 + on_evt 覆盖缺口）或
  `six-pack/QA`（pty E2E 回补，继承上游建议）。
