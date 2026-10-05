# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner　会话：cleaner-20261005

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
| **cleaner（20261005）** | tests/hardening* 环境漂移收敛：14 文件 fmt 归零 + 7 套件 clippy 警告归零；度量流水线全量复跑（coverage/CRAP/DRY/变异点扫描） | 已交付 | 全树 fmt 0；clippy --all-targets 0；产品面 --bins 0；955P/0F 零漂移；arch_check 8/8；src/ 零改动（详见第二节） |

### 遗留受限项（当前有效）
- **CRAP 为受限近似口径**（继承上游）：无标准 CRAP 工具，以 llvm-cov lcov 分支计数代入
  公式估算（本轮口径：comp=函数区间 BRDA 条目数+1，`testss_` 排除测试模块，闭包归并取极值）。
  本轮读数：产品函数 621 个中 88 个 CRAP>6，集中于交互分发核心（dialog_keys/mouse/dialog/
  main 事件循环，分发核心 comp≈分支数属登记口径而非硬凑项）。
- **沙箱无 TTY**：TUI 进程级端到端（真实按键/渲染）不可自动化；pty E2E 归 six-pack/QA。
- **gherkin-mutator 不存在于 npm registry**（继承上游）：Gherkin 差分变异受限，feature 无
  步骤处理器绑定。
- **单实例锁无 home 回退分支的 flock 缺失**（继承上游）：待操作者裁决。
- **工具链 PATH 注入不跨调用持久**：`cargo` 等需 `export PATH="$HOME/.cargo/bin:$PATH"`
  后使用（工具链本体在位，见第二节环境记录）。

### 实现定义值登记
- 无新增（本轮未引入契约值）。既有登记继承上游归档。

## 二、当前产出情况

### 本会话产物清单
- **tests/hardening.rs + 6 个 hardening_g_*.rs**：crate 根部新增 lint 姿态块——镜像产品
  main.rs 的 crate 级 allow（pedantic/nursery/cast 系/complexity 系）+ 挂载机制固有豁免
  （dead_code/duplicate_mod/unused_imports），均附依据注释。消除「产品源经 #[path] 收编
  后在测试编译下重燃产品面已豁免警告」的机制性偏差。
- **tests/hardening* 14 文件**：nightly rustfmt（项目 rustfmt.toml 口径）全量收敛，
  `cargo fmt --check` 全树归零。
- **tests/hardening_g_engine.rs**：删除死代码辅助 `stub_die`（编译器警告为证、零引用）；
  `format!` 无参调用改字面量；`assert_eq!(x, true)` 改 `assert!(x)`。
- **tests/hardening_g_appcore.rs**：移除未用 `mut`。
- **6 处 doc list 缩进修正**（blocks_hard/lease_spread/timefmt_hard/speed_evict/
  hardening_g_proxy/hardening_g_throttle_err 头注释）：列表与后续段落间补空 `//!` 行。
- **工具侧沉淀**（宪法第三章）：`packs/_common/notes/rust.md` 新增 3 条（日期版工具链无法
  别名桥接保留通道名；#[path] 收编测试 crate 的 lint 姿态镜像三分法；cargo-mutants 27.x
  与 PMD 7.28 调用形态变化）。
- **环境与工具在位**（本沙箱重建，供下游复现命令）：rustup nightly
  `1.101.0-nightly (db8f076d2 2026-10-03)` 安装于 `~/.cargo/bin`（与 coder 会话同版号）；
  cargo-llvm-cov 0.9.1（cargo install）；PMD 7.28.0 与 cargo-mutants 27.1.0（预编译发行版）
  安装于 `.tools/`（工具缓存，跨会话复用）。

### 验证证据
| 验证项 | 结果 | 命令（可复现，均在 project/ezr 下） |
|---|---|---|
| 全量测试 | 955P / 0F（10 个测试二进制，EXIT=0） | `cargo test` |
| fmt 全树 | 0 偏差 | `cargo fmt --check` |
| clippy 全目标 | 0 警告 | `cargo clippy --all-targets` |
| clippy 产品面 | 0 警告 | `cargo clippy --bins` |
| 架构边界 | 8/8 通过 | `bash scripts/arch_check.sh` |
| 覆盖率 | 全目标合并行覆盖 90.4%（8399 DA 行/7596 命中） | `cargo llvm-cov --branch --lcov --output-path <path>`（0.9.1） |
| CRAP（受限近似） | 产品函数 621 个，CRAP>6 共 88 个（交互分发核心为主） | 由 llvm-cov lcov 行按口径计算：comp=函数区间 BRDA 条目数+1、`testss_` 排除、闭包归并取极值（口径详见遗留受限项与 notes/rust.md，归档不含 .work/ 故脚本不随包） |
| DRY（CPD） | 100-token 重复 2 处：均为模块级 allow 头惯用块（supervisor/config/registry/sidecar），预存 | `pmd cpd --minimum-tokens 100 -l rust src`（PMD 7.28.0） |
| 变异点扫描 | 全产品 1658 点；单文件最大 88（lease.rs）；0 文件超 100 → 无需拆分 | `cargo mutants --list --line-col=true`（27.1.0） |

### 对账口径
- 基线（改动前本沙箱复测）与收敛后均为 955P/0F：测试计数零漂移。+0/-0 的构成：
  fmt 全量重排、lint 机械修正与死测试辅助删除均不触碰测试定义。
- fmt：收敛前 `--check` 71 处 diff 全落 tests/hardening* 14 文件（与 coder 交接逐项一致）
  → 收敛后 0。
- clippy --all-targets：收敛前 255 行警告全部落在 7 个加固套件（含跨二进制重复计数；
  throttle_err 121/appcore 116/engine 114/supervisor 110/hardening 77/main 9/proxy 6）
  → 收敛后 0；产品面 `--bins` 前后均 0。
- 归因分解：机制固有告警（pedantic/nursery 在收编产品源上重燃 + dead_code/
  duplicate_mod/unused_imports 挂载机制类）→ 测试 crate 根部 allow 姿态镜像消除；
  测试文件自身真实问题 15 项（死代码 1、unused_mut 1、useless_format 1、bool_assert 1、
  doc 缩进 11 类位）→ 机械修正消除。
- **行为不变硬证据：`git diff --stat -- src/` 为空（产品源零改动），15 个改动文件全部在
  tests/ 下**。

### 待办与待批（未决项）
- CRAP>6 的 88 个函数为受限近似读数（comp=BRDA+1 口径，偏差方向继承 notes/rust.md 条目）；
  按 SKILL 杠杆顺序的表驱动拆分属度量驱动工作，本轮受「产品源零改动」约束未触碰，
  留 architect/cleaner 后续会话按模块边界裁决。
- 模块 allow 头重复（CPD 2 处）的提取方式（宏/共享 include）属结构性决策，移交 architect
  裁决；本轮维持现状（产品源零改动）。
- 继承上游待批：单实例锁无 home 回退分支的 flock 缺失，待操作者裁决。

### 移交建议
- 建议操作者 `bin/swarm complete` 归档本轮产物；如需续跑：`six-pack/architect`
  （cleaner 的标准下一跳：allow 头重复提取与 CRAP 高 comp 点拆分的模块边界裁决）或
  `six-pack/QA`（pty E2E 回补环境条件型论证面，继承 hardender 交接建议）。
