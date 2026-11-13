# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（观察项清理轮，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner 收敛轮 ⇒ architect 收敛轮 ⇒ QA 终验轮 ⇒ specifier 显示指标/调整轮 v1.48–v1.51 ⇒ coder 产品实现轮（FR-01-117）⇒ coder 补偿轮（v1.47–v1.50 六项欠账）⇒ cleaner 清理收敛轮 ⇒ architect 架构收敛轮 ⇒ QA 终验轮（2026-11-12）⇒ specifier 锚点对齐轮（2026-11-12）⇒ specifier 遗留项裁决轮（2026-11-12）⇒ QA 收敛轮（2026-11-13）⇒ specifier 规程措辞对齐轮（2026-11-13）⇒ **QA 观察项清理轮（本轮 2026-11-13，QA 收敛轮/specifier 对齐轮移交观察项落地：suite_tui_display.py QA-TD-07 过时移交指针清理 + 度量工具面重建复验）**
> 会话：six-pack-QA-20261113（任务 = QA 收敛轮移交观察项「QA-TD-07 T/t 门控注释尾过时移交指针」顺手闭合；上游交接 = project/handoff.md specifier 规程措辞对齐轮版）
> 环境：nightly（rustc 1.102.0-nightly 8eb778d04 2026-11-12）@ ~/.cargo/bin（rsproxy 镜像前台装齐，本轮实测最快 dist 206 @ 16.3MB/s、清单 date 2026-11-13 当日新鲜）+ cargo registry rsproxy sparse（~/.cargo/config.toml）；pip 依赖 pyte/wcwidth（阿里云镜像装齐）；度量二件 cargo-llvm-cov 0.9.1（gh-proxy 预编译）+ PMD 7.29.0（swarm/.tools/pmd-bin-7.29.0）装齐并完成复验；bb/APS 仓未装机（本轮改动面无 feature/规程修订，APS 双检无对象，沿 architect 收敛轮「工具面收窄」先例；下轮涉 feature/规程面时开工先按「工具启动」条款重建）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.52**）+ `mission/` 两份 phase 详述（**phase-01 v1.51**、**phase-02 v1.20**）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-12 QA 终验轮） | ezr 产品实现 + 15 套件 e2e + 引擎拆分 + 终验 | 已交付 | 历轮对账在案 |
| specifier 显示指标/调整轮 v1.48–v1.51（2026-11-10/11） | phase-01 v1.50 / phase-02 v1.20 + mission.md v1.47–v1.51 + features/qa 配套 + ezr-demo 同构修改 | 已交付 | 历轮在案 |
| coder 产品实现轮（2026-11-11，FR-01-117） | project/ezr 源码 6 文件 + 守卫三守卫 | 已交付 | cargo test 在案 |
| coder 补偿轮（2026-11-11，v1.47–v1.50 六项欠账） | project/ezr 源码 9 文件 + 守卫 10 条 | 已交付 | 在案 |
| cleaner 清理收敛轮（2026-11-12） | lint 基线恢复 + conn_flags 表驱动 + CRAP 收敛 + 守卫 8 条 + notes 三分类 | 已交付 | 在案 |
| architect 架构收敛轮（2026-11-12） | 四阶段评审 + VS15 工具族单源化 + arch_check 18 条 + 属性 +4 | 已交付 | fmt clean；clippy 0W；8060P/0F；arch_check 18 全过；属性 86P |
| QA 终验轮（2026-11-12） | 全量独立复验 + e2e 15 套 191P + APS 20 feature 双检 + CRAP/DRY 度量 + 修复 4 项 + notes/INDEX | 已交付 | 与交接基线逐项持平（8060P/0F、191P/0F） |
| specifier 锚点对齐轮（2026-11-12） | v1.49 观察锚漂移 4 处待批项修订 + 同类漂移扩展 1 处（供否决）+ mission 观察项登记 + notes/handoff | 已交付 | APS 双检 2 份修订 feature 全过；产品源码零改动 |
| specifier 遗留项裁决轮（2026-11-12） | 待批项①②经操作者裁决落地：05-ui-pieces 扩展修订保留结案 + mission 出 FR-01-116① 修订注记（mission.md v1.52 / phase-01 v1.51 / 行内注记）+ notes 闭环回填 | 已交付 | APS 双检 05-ui-pieces 全过；产品源码零改动 |
| QA 收敛轮（2026-11-13） | 裁决轮移交待办③收敛落地（三份 runner 过时尾注清理）+ 全量独立复验收官 + 工具链与度量四件全新沙箱重建 | 已交付 | 单测 8060P/0F、e2e 15 套 191P/0F 与基线逐项持平；CRAP/CPD 登记口径精确对账 |
| specifier 规程措辞对齐轮（2026-11-13） | QA 收敛轮移交观察项落地：qa/01-tui-display-qa.md QA-TD-07 规程行「T 切/循环页签」滞后措辞对齐 feature v1.42/FR-02-31（头部补对齐注记） | 已交付 | suite_tui_display.py 21P/0F/0S；features/、产品源码、qa/runners、ezr-demo 零改动 |
| **QA 观察项清理轮（本轮 2026-11-13）** | **qa/runners/suite_tui_display.py QA-TD-07 注释尾过时移交指针清理（断言零改动）+ 同形全查 + 度量二件（cargo-llvm-cov/PMD）重建与口径复验** | 已交付 | **suite_tui_display 全套 21P/0F/0S（四段分段）；bin ezr 单测 1028P/0F；CRAP 94.2%（103/36）与 CPD 八家族精确对账；arch_check 全过；产品源码零改动** |

### 遗留受限项（当前有效，沿历轮口径）
- leech 专属 e2e、变异实跑（hardender 未跑，`--file` 分块消费面就绪）、6 个 BT/命令窗口族无专属 runner、ws(s) announce 无代理面。
- ezr-demo verify_ezr.py 预存 FAIL 15 项维持（脚本锚点漂移，非产品回归）；ezr-demo 零改动（锚点对齐轮起维持）。
- CRAP comp 残余登记沿 cleaner 复核口径维持（run/on_evt/draw_conns/pieces/restore/overlay_open/on_cmd_key/fake_udp_tracker 族）——本轮登记口径（bin ezr 单目标 1028 单测无 skip）复跑：行覆盖 94.2%（37251/39559，分子 ±2 行为 proptest 随机探索噪声、分母清单恒等）、ACTIONABLE=103/REG=36 与登记基线精确一致；产品源码零改动 + 同管线产出恒等，无新增可修点。
- 变异点 >100 四文件登记维持（app/mod.rs / bt/engine/conn.rs / ui/text.rs / model/task.rs）。
- CPD min200 登记维持：bt/engine/mod.rs 测试克隆 ×7 + ezr-mktorrent ↔ bencode 设计性重复 ×1（产品家族 0）——本轮复跑 8 克隆家族精确对账。
- bb/APS 仓未装机（本轮改动面无 feature/规程修订，APS 双检无对象；涉 feature/规程面时开工先按「工具启动」条款重建）。

### 实现定义值登记（累计，当前有效口径）
- D44–D74 沿历轮登记不变，本轮无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（QA 观察项清理轮）
1. **qa/runners/suite_tui_display.py（本轮唯一改动文件）**：
   - QA-TD-07 T/t 门控注释尾「规程行『T 切/循环页签』滞后项已移交操作者/specifier 待办」过时移交指针删除——该规程行已由 specifier 规程措辞对齐轮对齐结案，指针所指待办不复存在；断言本体零改动（git diff 1 文件 1+/2- 全注释行）。
2. **同形全查登记（不动改动面）**：qa/runners 全量检索「已移交/待办/待操作者」共 3 处——目标 1 处清理；suite_resume_sidecar.py:374（FR-01-22 规格缺口历史缘由注释，修订已按 v1.46 改判落地、非待办态）与 suite_persistence_config.py:206（PC-04 已知偏差登记、待操作者裁决协议载体）均非过时态，维持不动。
3. **notes 经验回填**：`notes/testing.md` 追加本轮 CRAP 登记口径澄清（bin ezr 单目标 = 登记口径；--skip 非覆盖恒等修正 QA 收敛轮条目；QA_ONLY 完整用例号陷阱）+ 过时移交指针清理面条款；`notes/environment.md` 追加本轮镜像复测与度量二件套装机实录；`notes/INDEX.md` 无新增文件不改。
4. **本 handoff.md** 重写为当前有效口径。

### 验证证据（命令可复现）
| 验证项 | 结果 | 命令 |
|---|---|---|
| e2e 改动面关联套件 | 21P / 0F / 0S（QA-TD-07 在内，四段 QA_ONLY 分段各 EXIT=0） | `QA_ONLY=QA-TD-01,… python3 suite_tui_display.py`（qa/runners，分四段点名） |
| runner 基线 diff | 仅注释行（1+/2-） | `git diff project/qa/runners/` |
| py_compile | EXIT=0 | `python3 -m py_compile qa/runners/suite_tui_display.py` |
| bin ezr 全量单测 | 1028P / 0F（覆盖率运行兼单测实跑） | `cargo llvm-cov --bin ezr --lcov`（无 skip） |
| CRAP 口径复验 | 94.2%（37251/39559）、ACTIONABLE=103/REG=36 与登记基线精确一致 | `python3 crap_calc.py <bin-ezr.lcov> project/ezr` |
| CPD 口径复验 | 8 家族精确对账（mod.rs×7 + mktorrent↔bencode×1，产品 0），EXIT=4 | `pmd cpd -d src --language rust --minimum-tokens 200` |
| arch_check | 全部边界规则通过，EXIT=0 | `bash scripts/arch_check.sh`（project/ezr） |

### 对账口径
- 本轮改动面 = **qa/runners/suite_tui_display.py 单文件注释行清理**（QA-TD-07 注释尾过时移交指针删除）；**产品源码（project/ezr/src）零改动、features/ 零改动、qa/ 其余规程文件零改动、qa/runners 其余文件零改动、ezr-demo 零改动**。
- 单测基线对账：产品源码零改动且本轮覆盖率管线实跑 bin ezr 全量 1028P/0F，与 QA 收敛轮分层口径（1028=942+86）自洽。
- CRAP/CPD 对账：登记口径复跑数字与登记基线逐项一致（见验证证据表），无新增可修点、无新增克隆家族。

### 待办与待批
- **本轮无待办、无待批、无阻塞项**（QA 收敛轮/specifier 对齐轮移交的观察项已全部收敛结案）。
- 变异实跑维持移交 hardender（受限项，操作者显式决定是否续跑）；其余遗留受限项沿历轮口径维持（见第一节）。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 本轮为移交观察项收口单点轮：six-pack 角色链保持 QA 终验轮后的「工作流完结」终态（specifier → coder → cleaner → architect → (hardender 可选) → QA → Done）；如操作者需补跑 hardender 或启动下一新需求，均以 `bin/swarm begin <角色>` 显式启动对应角色。
