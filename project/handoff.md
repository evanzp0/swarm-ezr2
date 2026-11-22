# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender（变异加固轮——architect SKILL 标准移交路径）
> 路径：…竞态销项轮（上上上轮）⇒ 清理轮 ⇒ 架构评审轮 ⇒ **变异加固轮（本轮，操作者 chat 指令「用 six-pack/hardender，顺便结案 schema 口径演进——不否决」）**
> 会话：six-pack-hardender-20261122（上游交接 = 上轮 handoff（架构评审轮）+ 操作者 chat 指令；ezr 不做旧版本兼容）
> 环境：rustup stable 1.100.0 + nightly 1.102.0-nightly（bdf712ee8 2026-11-21，nightly 移动目标以测试全绿为准；rust-toolchain.toml 钉 nightly 全项目统一）、cargo registry rsproxy sparse、llvm-cov 0.9.2 + cargo-mutants 27.1.0 + PMD 7.29.0（.tools/pmd-bin-7.29.0）+ bb 1.12.200（.tools/bin/bb）+ APS bb 工具（equivalence-filter patch 已应用）全部就位；CARGO_INCREMENTAL=0 + CARGO_PROFILE_DEV_DEBUG=0 + CARGO_PROFILE_TEST_DEBUG=0 三件套全程前台分块。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.57**）。ezr 不做旧版本兼容。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同，见 11-trackers-window v1.14 注记）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| QA runtime 重建轮 | APS 四件套重建 + verdict 级全量 + 07-add-bt 哈希销项 | 已交付 | verdict Σ = 1107 killed + 17 filtered + 0S/0E |
| 遗留受限项销项轮 | ③644 改判杀灭 + ①缺口账目 ID 权威清单 + ④LO_ONLY 敞口消灭 + ②Go 版 mutator filter 移植；**open_int 三分退役二分交付（本轮经操作者裁定结案）** | 已交付 | verdict Σ = 1107 killed + 0 survived + 17 filtered |
| 竞态销项轮 | block_worker 打开失败有界重试窗 + 两行为锁定单测 | 已交付 | bins 1147P/0F + 集成 8440P/0F 全绿 |
| 清理轮 | 竞态销项改动面度量收口 + 3 个纯追加 worker 级测试 | 已交付 | bins 1150P/0F + 集成 8467P/0F；CRAP/DRY/变异点三度量全过 |
| 架构评审轮 | 四阶段架构评审 + model↔bt 模块环销项 + arch_check 规则 1 扩域 + PeerTransport 属性测试补缺 | 已交付 | bins 1152P/0F + 集成 8479P/0F + 属性 90P/0F；arch_check 18 规则全过 |
| **变异加固轮（本轮）** | architect 批次变异加固（82 变异点全判定）+ 加固测试 3 条 + schema 口径演进结案（操作者不否决）+ APS patch 复用回归修复 + 最终验证序列四步全过 | 已交付 | 82 = 75 killed + 6 unviable + 1 等价；soft 全复用守恒 1124；CRAP 94.6% 恒等；DRY 双扫描恒等；src 零改动 cmp 证明 |

### 遗留受限项（当前有效）
- **workers.rs 227-228（磁盘写入失败 fatal）与 239-240（resp.chunk 流错误 classify）为集成级错误臂**：bin 口径纯追加测试不可确定性触发（需 fs/流层注入基建），由集成链覆盖；bin 口径不追测，非缺陷。
- **workers.rs 257-258 死防御臂保留**（`if failure.is_some() { return; }` 于 `if aborted` 返回之后）：控制流证明不可达，属防御性不变式护栏，按历轮裁决登记保留；若未来新增 failure 赋值臂未同迭代置 aborted 则该臂转活，须重新对账。
- bin 口径 cov=0 五处登记（本轮 CRAP 复验全部在列）：property_tests.rs（失败闭包 by design）、main.rs:303（入口壳 env 兜底）、bt/webrtc.rs（webrtc 覆盖非确定性受限项）、mouse.rs:516（交互层）、metainfo.rs:52（批次外积压）。
- 2 条 unviable 终态（mutation 账目人工实证定案）+ trackpool.rs CRAP 竞速对账口径（已接受，非可执行任务；现存 FN 行号 413，历轮登记 412 为行漂移）沿历轮登记不变。
- **bt/mod.rs 8 子模块 pub mod 面维持现状（本轮 hardender 裁决收口）**：bin-only crate 无 crate 外泄漏面；消费点 = engine↔bt 协调枢纽（裁决在案）与属性测试；本轮批次变异（82 点含词表全逻辑）未暴露任何需经收窄才能杀灭的位点——收窄需引入 re-export 间接层，收益不抵风险，**观察项关闭**。
- **DRY 几何再登记残留**：CPD 组数 221 vs cleaner 轮基线 219 的 +2 为 architect 纯移动轮几何再登记残留（双扫描证本轮与 HEAD 逐字节恒等，无克隆组涉批次文件），登记为观察项非缺陷。

## 二、当前产出情况

### 本会话产物清单（变异加固轮）
1. **变异加固账目（batch = architect 轮新增产品逻辑 model/bt.rs + model/persist.rs，82 变异点）**：killed 75 + unviable 6（Default 替换型——BtPartialBlock/BtSidecar/TrackerUrl/PeerTransport 四类型均无 Default 派生，E0277 活体探针 + derive 逐类型核验实证）+ 幸存 1（bt.rs:79:19 `lo < hi`→`lo <= hi`，边界空转型无条件等价，书面论证见交接四要素④）。非等价存活率 = 0。
2. **加固测试 `tests/hardening/bt_vocab_hard.rs`（新增，3 条）**：杀灭两个覆盖缺口幸存体（bt.rs:44:45 `==`→`!=`、persist.rs:4:19 NotFound 守卫→true），一测一靶；hardening 259→262。
3. **schema 口径演进结案（操作者 20261122 裁定不否决，否决路径关闭）**：open_int 三分退役二分定案——FILTERED 17 + closed 1107、153 格逐格裁决、账目守恒 1124=1107+17；本轮以修复后 mutator 软差分复用全程复现守恒（193 场景全复用、executed=0、Σ=1124 恒等）。
4. **APS mutator patch 复用回归修复**：patch 内复用判据 remove→filter（本场景滤选后计数）；bb 测试 11 tests/51 assertions 全绿；patch 交付件重生成（`qa/aps-mutator-equivalence-extension.patch`，下一沙箱 clone 后 apply 即为修复版）。
5. **feature stamp/manifest 刷新**（20 份 feature，工具自管理注释更新：tested_at 刷新、stamp 重算，账目数字不变）。
6. **最终验证序列四步**：语言变异（账目见上）→ 软 Gherkin 变异（--level soft 全复用守恒 1124）→ CRAP（llvm-cov bins + 段级过滤 + qa/runners/crap_calc.py：行覆盖 94.6% 与历轮恒等、mouse:178=77.09 锚恒等、ACTIONABLE 101 全为历轮既定账、本轮新增 0）→ DRY（PMD CPD min-tokens 70 双扫描 worktree vs git HEAD 逐字节恒等 221 组）。
7. **经验沉淀**：notes/testing.md 本轮 6 条（schema 结案/patch 回归/变异账目/等价失效条件/DRY 残留/CRAP 复验）；notes/environment.md 2 条（rustup 镜像当期实测/磁盘口径）；packs/_common/notes/mutation-hardening.md 新增第 7 节（分块前台续跑协议）；packs/_common/notes/aps.md 追加复用判据校准与 patch 重生成纪律。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 基线复验（开工门） | bins 1152P/0F + 集成 8479P/0F + 属性 90P/0F + fmt clean + clippy 零警告 + arch_check 18 规则全过，与上轮 handoff 逐项恒等 | `cargo test --bins` / 逐 `cargo test --test <目标>` / `cargo test --bin ezr property_tests::` / `cargo fmt --check && cargo clippy --all-targets` / `bash scripts/arch_check.sh` |
| 语言变异 | 82 = 75 killed + 6 unviable + 1 等价（清单制差分，`--file src/model/bt.rs --file src/model/persist.rs`，nightly 钉定口径） | `cargo mutants --in-place -d <worker副本> --file src/model/bt.rs --file src/model/persist.rs --baseline run --no-shuffle --build-timeout 900 -t 300 --cargo-arg=--bin --cargo-arg=ezr --cargo-arg=--test --cargo-arg=hardening --cargo-test-arg=-- --cargo-test-arg=model:: --cargo-test-arg=bt::tracker:: --cargo-test-arg=bt::webrtc:: --cargo-test-arg=bt_vocab_hard::`（复现方法见交接四要素②） |
| 加固套件 | 262P/0F（259 + 3 新增） | `cargo test --test hardening` |
| 软 Gherkin 变异 | 20 feature 全 exit=0；193 场景全复用、executed=0、Σ = 1107 复用 killed + 17 filtered = 1124 守恒 | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-mutator --runner-worker <协议worker> --feature <绝对路径> --work-dir <会话目录> --level soft --equivalence-filter /home/z/swarm-ezr2/project/qa/equivalence-filter.json --json)` 逐 feature |
| CRAP | 行覆盖 94.6%（39828/42084，历轮恒等）；ACTIONABLE_COUNT=101 全为历轮既定账，新增 0 | `cargo llvm-cov --lcov --bin ezr --output-path <lcov> -- --skip property_tests` → `awk '/^SF:/{keep=index($0,"/project/ezr/src/")>0} keep'` → `python3 qa/runners/crap_calc.py <lcov> /home/z/swarm-ezr2/project/ezr` |
| DRY | 221 组，worktree vs git archive HEAD 双扫描逐字节恒等（本轮零漂移） | `pmd cpd -l rust --minimum-tokens 70 --dir <src> --format text`（两棵树同参数） |
| fmt / clippy（含新增测试） | clean / 全目标零警告 | `cargo fmt --check && cargo clippy --all-targets` |
| 产品源码零改动 | pristine 逐文件 cmp：src/ 逐字节恒等；新增仅 tests/hardening/bt_vocab_hard.rs + tests/hardening.rs mod 行 | `diff -rq <pristine>/ezr/src project/ezr/src` |

### 对账口径
- **产品源码零改动**：词表迁移后 src/ 逐字节恒等（pristine cmp），本轮唯一产品树新增 = 加固测试（允许面）；feature stamp/manifest 更新为 mutator 自管理注释（允许面）。
- **集成计数 +3 归因**：hardening 259→262 = bt_vocab_hard.rs 三测；其余 12 集成目标与 bins 全量与基线恒等（src 零改动的编译级实证）。
- **变异账目守恒**：82（--list 权威口径 bt.rs 76 + persist.rs 6）= 75 + 6 + 1 逐分项可复算；unviable 6 = Default 替换型全集（四类型 derive 缺失逐一核验 + 代表位点 E0277 活体探针）。
- **schema 守恒复现**：soft 差分 Σ(skipped_mut=1107 + filtered=17) = 1124 与 QA 轮 verdict 账目逐数恒等；FILTERED 17 分项 = retry-backoff 6 / modify-task 4 / ui-conns 3 / slots-queue 2 / named-proxy 1 / cmd-window 1。
- **工具口径**：沿用上轮交接的工具与版本（llvm-cov 0.9.2 / cargo-mutants 27.1.0 / PMD 7.29.0）；nightly 由 2026-11-20（c7d5ba59d）前移至 2026-11-21（bdf712ee8，移动目标），基线全套在该 nightly 下复验绿。

### 交接四要素
1. **本地验证命令清单（按序）**：`cargo test --bins`（1152P）→ 逐集成目标（8479+3=8482P，hardening 262）→ `cargo test --bin ezr property_tests::`（90P）→ `cargo fmt --check` + `cargo clippy --all-targets` → `bash scripts/arch_check.sh`；变异复现按上表命令（worker 副本 + touch 恢复协议，见 notes/mutation-hardening.md 第 7 节）；软 Gherkin/CRAP/DRY 按上表。
2. **会话运行时产物与复现**：`.work/`（session/plan/tmp/logs、mutants-out-r1 账目、aps-soft 报告、pristine/ 备份）不进工作区产物；复现 = 按 notes/environment.md 开工顺序重建工具链（rsproxy 镜像 + nightly 钉定 + llvm-tools-preview + bb/PMD/.tools）后按上表命令重跑；pristine 备份即本轮零改动证明的对照基座。
3. **feature 注释块与差分复用**：20 份 feature 的 stamp/manifest 为 mutator 自管理注释（本轮软跑已刷新）；差分复用按 scenario_hash + mutation_count（滤选后）+ filter-hash 判定，soft 级不校验 implementation_hash、hard/full 级校验；强制重跑单 feature = 删除该 feature 头部 `# mutation-stamp` 至 `# acceptance-mutation-manifest-end` 注释块。
4. **等价突变体清单及失效条件**：唯一等价体 = src/model/bt.rs:79:19 `lo < hi`→`lo <= hi`（BtTaskData::span_sum_over_selected_files 边界空转：lo==hi 时 `sum += hi - lo` 恒加 0，任何语料无可观察面；**失效条件**：该累加引入副作用、或改用饱和/panic 算术使 lo==hi 分支可观察，或 hi==lo 边界成为规格区分点——任一成立须回补边界测试）。

### 待办与待批
- **本会话无待批无待办**——操作者指令的变异加固轮与 schema 口径结案均已落地（不否决即定案，否决路径关闭）。

### 移交建议
- 下一步建议运行 `six-pack/QA` 技能（hardender SKILL 标准移交路径；产品源码零改动 + 四步最终验证序列全绿，QA 可直接复用 1124 守恒账目）。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。

By hardender.
