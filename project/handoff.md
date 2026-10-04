# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（空闲期注册表零写放大修复）　会话：coder-20261004

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
| hardender（20261004） | 变异加固：7 个 hardening_g_* 套件 ~110 条新测试；等价/超时/环境型台账；CRAP/DRY 复核 | 已交付 | 加固后全绿；equivalence 台账与存活处置见其归档 |
| **coder（本轮）** | 空闲期注册表零写放大修复：model 层 `save_json_atomic` 内容幂等短路 + 3 条聚焦单测（TDD）+ 注释/文档修订 | 已交付 | 全量 955P/0F；产品面 clippy 0；触碰文件 fmt 0；arch_check 8/8（详见第二节） |

### 遗留受限项（当前有效）
- **本沙箱 nightly 工具链漂移**（rustc 1.101.0-nightly 2026-10-03，rustup 全新安装于
  `~/.cargo/bin`）：`cargo fmt --check` 在 `tests/hardening*` 14 个文件报预存格式偏差、
  `cargo clippy --all-targets` 对加固套件报预存警告（产品面 `--lib --bins` 为 0）。
  与本轮改动无关——逐套件警告数与基线对账一致。消除途径：cleaner 以沙箱同版 nightly
  收敛，或操作者钉回 hardender 会话同期 nightly 日期。
- **沙箱无 TTY**：TUI 进程级端到端（真实按键/渲染）不可自动化；pty E2E 归 six-pack/QA
  （hardender 交接既有建议，本轮未改变该边界）。
- 继承上游：CRAP 为受限近似口径；单实例锁无 home 回退分支的 flock 缺失（待操作者裁决）；
  gherkin-mutator 不存在于 npm registry（Gherkin 差分变异受限，feature 无步骤处理器绑定）。

### 实现定义值登记
- 无新增（本轮未引入契约值）。既有登记继承上游归档。

## 二、当前产出情况

### 本会话产物清单
- `ezr/src/model/mod.rs`：`save_json_atomic` 增加内容幂等短路——序列化结果与盘上现有文件
  字节一致时跳过写盘（读失败视为不一致照常原子落盘）。注册表与 sidecar 共用该原语，
  两类持久化一并获得幂等性。
- `ezr/src/model/registry.rs`：`Registry::save` 文档注释同步；新增 2 条单测
  （`save_skips_rewrite_when_content_unchanged` 内容一致不重写[mtime 口径]、
  `save_writes_when_content_changes` 内容漂移必须落盘[防过度跳过]）。
- `ezr/src/app/engine.rs`：tick 第 7 步注释修订（写明空闲期短路语义）；新增 1 条单测
  `idle_tick_skips_registry_rewrite_until_state_drifts`（空闲 tick 越过 5s 窗口不重写注册表 +
  状态漂移后下一周期窗口必须落盘）与夹具 `make_app_with_reg`。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量测试 | 955P / 0F（10 个测试二进制，EXIT=0） | `cd project/ezr && cargo test` |
| 新增单测 | 3 条，TDD 红→绿 | `cargo test --bin ezr -- save_skips_rewrite_when_content_unchanged save_writes_when_content_changes idle_tick_skips_registry_rewrite_until_state_drifts` |
| clippy 产品面 | 0 警告 | `cargo clippy --lib --bins` |
| clippy 全目标 | 各加固套件警告数与基线逐项一致（无新增） | `cargo clippy --all-targets` |
| fmt（本轮触碰文件） | 0 偏差 | `rustfmt --check --edition 2021 src/model/mod.rs src/model/registry.rs src/app/engine.rs` |
| 架构边界 | 8/8 通过 | `bash scripts/arch_check.sh` |

### 对账口径
- 基线（本沙箱复测，改动前）941P/0F → 修复后 955P/0F：+14 = 产品 bin +3（本轮新增单测）
  + 加固套件对产品源 include 复制 +11（registry 2 测 × appcore/engine/supervisor/throttle_err
  4 套件 + engine 1 测 × appcore/engine/supervisor 3 套件）。加固套件以 `tests/../src` 路径
  收编产品源，产品源 `cfg(test)` 测试在各套件二进制重复实例化属既有机制，非异常。
- 行为语义不变：5s 周期兜底 + 关键状态转换即时保存全部保留；唯一变化是静止期（序列化
  内容与盘上字节一致）不再产生磁盘写（无 temp+rename、无 mtime 抖动）。

### 待办与待批（未决项）
- `tests/hardening*` 的 fmt 偏差与 clippy 警告属环境漂移预存项（见遗留受限项），是否由
  cleaner 收敛请操作者裁决。
- 本轮 bug 判定与处置已落实：空闲期每 5s 重写字节相同 `~/.ezr/state/registry.json` 判定为
  bug（无谓写放大），以持久化原语内容幂等修复；无需操作者再批准。

### 移交建议
- 建议操作者 `bin/swarm complete` 归档本轮产物；如需续跑：`six-pack/cleaner`（可顺带以
  沙箱同版 nightly 收敛 tests/hardening* 漂移偏差）或 `six-pack/QA`（pty E2E 回补环境
  条件型论证面，继承 hardender 交接建议）。
