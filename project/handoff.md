# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（FR-02-37 v1.3 QA 终验轮，**工作流完结**）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner/architect/QA 收敛链 ⇒ specifier 显示指标/调整/裁决/新增需求轮（v1.47–v1.53）⇒ specifier 裁决落实修订轮（v1.54）⇒ coder 产品实现/复核重验轮 ⇒ specifier PEERS/SEEDS 核证修订轮（v1.55）⇒ specifier D80 裁决落实轮（v1.56）⇒ coder FR-02-37 v1.3 产品实现轮 ⇒ cleaner 收敛轮 ⇒ architect 架构评审轮 ⇒ hardender 变异加固三轮（2026-11-15/16）⇒ **QA FR-02-37 v1.3 终验轮（本轮 2026-11-16，任务 = 最终独立验证 + 交接移交项处置；six-pack 流转路径 specifier → coder → cleaner → architect → (hardender) → QA → Done 已走完）**
> 会话：six-pack-QA-20261116（SESSION_TASK = FR-02-37-v1.3-QA终验轮；上游交接 = hardender 续轮二版 handoff）
> 环境：rustc 1.102.0-nightly (5f4effd57 2026-11-13)（安装名 nightly-2026-11-14，RUSTUP_TOOLCHAIN 注入钉扎，rsproxy 直链前台装齐）；度量四件在位（cargo-llvm-cov 0.9.1 / cargo-mutants 27.1.0 / PMD CPD 7.29.0 / bb 1.13.229 + APS，全经 gh-proxy/直链重建）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.56**）+ `mission/` 四份 phase 详述（现行 phase-01 v1.53、phase-02 v1.24）。ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-14 architect 评审轮） | ezr 产品实现 + 收敛/评审链 | 已交付 | 历轮对账在案（16 目标 8223P/0F 基线） |
| hardender FR-02-37 v1.3 变异加固三轮（2026-11-15/16） | 变异实跑 494 突变体全处置 + 加固/复验套件 + 79 存活体双套件零杀灭证据 + notes/handoff | 已交付 | 17 目标 9170P/0F、clippy 0、fmt 合规、arch_check 18 规则过；产品源码字节级零改动证明在案 |
| **QA FR-02-37 v1.3 终验轮（本轮 2026-11-16）** | 产品缺陷修复 2 处（v1.53 焦点环 Enter 分发映射）+ QA runner 同步修复 3 套件 + 全量独立验证（基线重立 + 15 套件全绿 + CRAP/DRY 实跑）+ notes 两文件追加 + 本 handoff 重写 | 已交付 | 基线 17 目标 9170P/0F（修复后重立，口径不变）+ prop 88P + fmt 0 + clippy 0 + arch 18 规则 + **15 套件 191P/0F 全绿** |

### 遗留受限项（当前有效）
- **变异实跑总账（三轮，全收敛）**：全清单 494 突变体处置闭环、零未跑；登记函数 235 = 杀 149 + unviable 6 + 超时杀 1 + 存活 79；非登记函数 84 沿杀灭账免跑；trackpool ws 面 31 = 20 定理证明 + 11 免跑。逐突变体判定权威 = `adaptive_resolved.csv` 最新行（沙箱重置后按交接②再生）。
- **79 存活体 = 覆盖缺口确认（非等价），双套件零杀灭证据已补齐（hardender 续轮二）**：`results/bs_runs.csv` 23 批次 exit=0、恰覆盖 79 id；函数粒度登记（trackpool 9 / pieceio 45 / peerwire 25）与回补路径沿 hardender 交接④不变。**缺口 112 回补测试归属（本轮 QA 裁决）**：不入 QA 终验轮（修复最小化原则；缺口已定性、回补路径明确登记、变异证据链完整）——移交操作者裁决是否追加专项轮（可复用上半轮 15 加固测试模式，按函数名锚定精确断言测试）。
- **软 Gherkin 验收变异执行层不可用（受限项，沿历轮登记不变）**：1123 突变 errors=1123；**本轮 QA 裁决**：回补（IR 驱动验收入口后适配器接入）属专项基建轮，不入终验轮，同移操作者裁决。
- **specifier 同步欠账 2 处（本轮发现，登记待批）**：① `features/01-persistence-config.feature` 场景 04 Examples 残留 `bt.seed_duration_secs | 0` 拒绝行——与 FR-02-40②（seed_duration_secs 按未知键口径静默废弃，不报错不生效）互斥；QA 按三方一致口径（FR-02-40② 规则文本 + 实现 + 单测）断言：拒绝行改判 `bt.default_seed_duration_m = 0`，并新增退役键静默忽略正探针。② `qa/08-modify-bt-qa.md` QA-MB-10 行仍表述「做种剩余时长重置为 config seed_duration_secs 当前值」——同涉 FR-02-40② 退役键与 FR-02-38 v1.21 剩余做种展示退役（09-bt-engine-qa.md QA-BE-13/14 已同步可作口径样板）。两处均属 specifier 职责面，QA 不越权改，待操作者批准后由 specifier 轮同步。
- **QA runner 族缺口（沿历轮登记不变）**：06-cmd-window、07-add-bt、09-bt-engine、10-bt-anti-leech、11-trackers-window 五规程暂无可执行套件（leech 专属 e2e 缺口同此账）；现行 15 套件本轮全绿。
- **ezr-demo 侧验证脚本（工作区外，移交建议）**：`ezr-demo/scripts/verify_trackers_popup.py` T2/T3 共 3F（specifier 历轮 stash 基线归因的 demo 侧既有失败）+ `verify_size_move.py` 陈旧化——位于 project/ 工作区之外，不在 QA 验证批次内，QA 不越权修复；建议操作者如需收敛则指派 demo 专项轮。
- **CRAP 登记口径（bin ezr 单目标全量、无 skip）**：行覆盖 38124/40406 = **94.4%**（上轮 94.3%）；**ACTIONABLE_COUNT=99**（上轮 102，改善向）；注册热点逐一对账在位（trackpool.rs:412 live_snapshot CRAP=186.96 精确一致、engine 异步环/peerwire/pieceio 面全在），无新增热点。登记差异归属：本轮路由单测同步使 bt_dialog 分发函数接近全覆盖（2 函数离开 ACTIONABLE）。
- **DRY 登记（PMD CPD 7.29.0 -l rust，EXIT=4 正常语义）**：阈值 200 结构性克隆 6 组（engine/mod.rs 测试区 5 组 + **mktorrent bencode 跨 bin 1 组**——architect 既定裁决「维持登记、lib 化不实施」沿袭）；阈值 70 全量 198 组 = **产品侧 23 / 测试侧 175**；无新增大型产品克隆，无 QA 侧修复项。
- **QA-PC-06 环境性 flake（登记）**：全量首跑 1F、点名复跑 ×2 全绿、产品树同轮字节级零改动面——负载敏感时序 flake 非回归；后续轮如复现按三证归因口径处置。
- **mktorrent bencode 跨 bin 重复**：维持登记、lib 化不实施（architect 既定裁决沿袭）。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退）；D80①–④ 已全部结案**——本轮 QA 终验无规格语义改动、无新增 D 值（产品缺陷修复属实现面，不改契约）。

## 二、当前产出情况

### 本会话产物清单（QA FR-02-37 v1.3 终验轮）
1. **产品缺陷修复（本轮唯一产品改动，最小面）**：`src/app/bt_dialog.rs` `add_bt_dialog_enter` / `modify_bt_dialog_enter` 焦点映射重对齐——v1.53 焦点环插入「按顺序下载」复选项（AddBt 11→12、ModifyBt 8→9）后渲染 indices 与 nfocus 已更新而 Enter 分发映射停留旧 indices，用户可观察后果 = 立即下载 Enter 落暂停添加、确定 Enter 关闭不保存、取消 Enter 反而确认。同步更新两个路由单测（`add_bt_dialog_enter_routes_focus` / `modify_bt_dialog_enter_routes_focus`）至新焦点链。PTY 实证：修复前 _add_bt_task 落暂停态（toast ⏸ 已暂停）、修复后立即下载正确建任务；修复后 15 套件全绿 + 基线重立。
2. **QA runner 同步修复（3 套件）**：① `suite_config_template.py` 重锚（190→206 行，default_seed_duration_m 键行、注释态 seed_exit_rule 键保持断言）→ 5P/0F；② `suite_persistence_config.py` QA-PC-04 拒绝行改判 + 退役键静默忽略正探针 → 全套 11P/0F；③ `suite_modify_bt.py` 剩余做种展示退役翻转（assert_in→assert_not_in ×2、倒计时数值断言退役）+ `_driver_config` 退役键移除 → 4P/0F。
3. **全量独立验证**：基线 17 目标 **9170P/0F**（bins 1098 + hardening 三批 2825/1063/3237 + hardening_mutation 947；修复后重立，与交接①口径精确一致）+ property_tests 88P + fmt 0 + clippy 0（all-targets）+ arch_check 18 规则全过。**15 套件 191P/0F 全绿**（PTY/UI-only，串行前台；throttle_proxy 按 QA_ONLY 四段）。
4. **度量工具实跑**：CRAP（llvm-cov 0.9.1 → lcov → crap_calc.py）与 DRY（PMD CPD 双阈值）结果登记入第一节（均无 QA 侧修复项）。
5. **经验沉淀**：`project/notes/testing.md`、`environment.md` 各追加本轮条目（焦点环五表同步缺陷模式 / cargo test 不刷新 bin / crap_calc argv 契约 / PMD 多测试区划界 / 退役键退役显示 runner 翻转三件套 / TP 分段实录 / 工具传输 EOF 风暴 / nightly 钉扎名 +1 天 / 度量四件重建链 / 覆盖率隔离盘账），INDEX.md 同步。
6. **本 handoff.md** 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令/证据（可复现） |
|---|---|---|
| 基线 17 目标 | **9170P/0F（修复后重立，口径不变）** | `cd project/ezr && export PATH=$HOME/.cargo/bin:$PATH RUSTUP_TOOLCHAIN=nightly-2026-11-14 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0` + 交接①命令清单（fmt/clippy/--bins/三批 hardening/mutation） |
| 属性测试 | 88P/0F | `cargo test property_tests::` |
| 架构检查 | 18 规则全过 | `bash scripts/arch_check.sh` |
| QA 套件全量 | **15 套件 191P/0F** | `cd project/qa/runners && python3 suite_<name>.py`（throttle_proxy 用 QA_ONLY 分段；日志 .work/tmp/qa-logs/） |
| CRAP | 94.4% 覆盖 / ACTIONABLE 99（热点对账在位） | `cargo llvm-cov --bin ezr --lcov --output-path <lcov>`（CARGO_TARGET_DIR 隔离）→ `crap_calc.py <lcov> <src_root>` |
| DRY | 结构克隆 6 组 / 产品侧 23 组（阈值 70） | `pmd cpd --dir src --language rust --minimum-tokens {200,70}`（EXIT=4 正常） |
| APS 规格层抽查 | gherkin-parser + ir-dry-checker 跑通 | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-parser <feature> <out>.json && bb gherkin-ir-dry-checker ...)` |

### 对账口径
- 本轮改动面 = `src/app/bt_dialog.rs`（2 分发函数 + 2 路由单测）+ qa/runners 3 套件 + notes 两文件 + INDEX.md + 本 handoff；**其余产品源码与 tests/ 零改动**。
- 基线重立对账：修复后 17 目标 9170P/0F 与 hardender 交接①逐批精确一致（1098/2825/1063/3237/947），fmt 0、clippy 0、arch 18 过——修复零回归。
- 套件全量对账：191P = 21+5+16+12+11+14+9+15+10+21+18+10+18+7+4（15 套件分项之和 ✓）。
- CRAP 对账：94.4%/99 vs 登记口径 94.3%/102——行覆盖 +0.1pp、ACTIONABLE −3（归属 = 路由单测同步覆盖 2 分发函数全臂），注册热点清单逐项在位、零漂移。

### 待办与待批
- **待批①（specifier 同步欠账）**：feature 场景 04 `bt.seed_duration_secs | 0` 残留行 + qa/08-modify-bt-qa.md QA-MB-10 旧键表述——请操作者批准后由 specifier 轮按 FR-02-40②/FR-02-38 v1.21 同步（QA 已按现行规格断言，不阻塞）。
- **待批②（专项轮裁决）**：缺口 112 回补测试是否追加专项轮（回补路径沿 hardender 交接④：按函数名锚定精确断言测试，纯函数面直调断言最低成本）。
- **待批③（专项轮裁决）**：软 Gherkin 执行层回补（IR 驱动验收入口 + 适配器接入）是否追加专项轮。
- 待办：ezr-demo 侧 verify 脚本 3F/陈旧化如需收敛，请操作者指派 demo 专项轮（工作区外，本流程不可达）。

### 移交建议
- **six-pack 工作流已完结**（specifier → coder → cleaner → architect → (hardender) → QA → Done 全链走完，QA 为最后一名角色）。请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 如需续做：待批①～③ 均为独立小轮（specifier 同步轮 / 回补专项轮 / Gherkin 基建专项轮），由操作者显式决定是否开工；无流水线自动流转。
