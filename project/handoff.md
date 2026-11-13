# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（收敛轮，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner 收敛轮 ⇒ architect 收敛轮 ⇒ QA 终验轮 ⇒ specifier 显示指标/调整轮 v1.48–v1.51 ⇒ coder 产品实现轮（FR-01-117）⇒ coder 补偿轮（v1.47–v1.50 六项欠账）⇒ cleaner 清理收敛轮 ⇒ architect 架构收敛轮 ⇒ QA 终验轮（2026-11-12）⇒ specifier 锚点对齐轮（2026-11-12）⇒ specifier 遗留项裁决轮（2026-11-12）⇒ **QA 收敛轮（本轮 2026-11-13，runner 尾注收敛 + 全量独立复验）**
> 会话：six-pack-qa-20261113（任务 = 裁决轮移交待办③收敛：三份 runner 过时尾注清理 + QA 全量独立复验收官；上游交接 = project/handoff.md 裁决轮版）
> 环境：nightly 1.102.0（9c5648833 2026-11-10，与交接基线同 commit）@ ~/.cargo/bin（TUNA 镜像前台装齐，本轮实测 36.6MB/s 最快可用）+ cargo registry rsproxy sparse（~/.cargo/config.toml）；度量与 APS 四件本轮全部就位：cargo-llvm-cov 0.9.1（~/.cargo/bin）+ llvm-tools-preview、PMD 7.29.0（.tools/pmd-bin-7.29.0）、bb 1.13.229（.tools/bin/bb）+ APS 仓（.tools/Acceptance-Pipeline-Specification）；harness pip 依赖 pyte/wcwidth/libtorrent 2.1.1.0（阿里云镜像装齐）

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
| specifier 锚点对齐轮（2026-11-12） | v1.49 观察锚漂移 4 处待批项修订 + 同类漂移扩展 1 处（供否决）+ mission 观察项登记 + notes/handoff | 已交付 | APS 双检 2 份修订 feature 全过（parser + dry-check EXIT=0）；产品源码零改动，基线 8060P/0F 沿用 |
| specifier 遗留项裁决轮（2026-11-12） | 待批项①②经操作者裁决落地：05-ui-pieces 扩展修订保留结案（头部注记回填）+ mission 出 FR-01-116① 修订注记（mission.md v1.52 / phase-01 v1.51 / 行内注记三层落点）+ notes 闭环回填 | 已交付 | APS 双检 05-ui-pieces 全过（parser 7 场景 + dry-check 均 EXIT=0）；产品源码零改动，基线 8060P/0F 沿用 |
| **QA 收敛轮（本轮 2026-11-13）** | **裁决轮移交待办③收敛落地：三份 runner 过时尾注清理（断言本体零改动）+ 全量独立复验收官（单测 8060P/0F、e2e 191P/0F、CRAP/DRY/arch_check/APS 全对账）+ 工具链与度量四件全新沙箱重建** | 已交付 | **单测 8060P/0F、e2e 15 套 191P/0F 与基线逐项持平；CRAP/CPD 登记口径精确对账无新增** |

### 遗留受限项（当前有效，沿历轮口径）
- leech 专属 e2e、变异实跑（hardender 未跑，`--file` 分块消费面就绪）、6 个 BT/命令窗口族无专属 runner、ws(s) announce 无代理面。
- ezr-demo verify_ezr.py 预存 FAIL 15 项维持（脚本锚点漂移，非产品回归）；ezr-demo 零改动（锚点对齐轮起维持）。
- CRAP comp 残余登记沿 cleaner 复核口径维持（run/on_evt/draw_conns/pieces/restore/overlay_open/on_cmd_key/fake_udp_tracker 族）——本轮 bin 套件口径复跑：行覆盖 94.2%（37249/39559）、ACTIONABLE=103/REG=36，登记族全部在位；产品源码零改动 + 同 nightly commit 下同管线产出恒等，无新增可修点。
- 变异点 >100 四文件登记维持（app/mod.rs / bt/engine/conn.rs / ui/text.rs / model/task.rs）。
- CPD min200 登记维持：bt/engine/mod.rs 测试克隆 ×7 + ezr-mktorrent ↔ bencode 设计性重复 ×1（产品家族 0）——本轮复跑 8 克隆族精确对账。

### 实现定义值登记（累计，当前有效口径）
- D44–D74 沿历轮登记不变，QA 收敛轮无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（QA 收敛轮）
1. **裁决轮移交待办③收敛落地（三份 runner 过时尾注清理，断言本体零改动）**：
   - `qa/runners/suite_tui_display.py`：QA-TD-17 docstring 尾「待 specifier 修订后对齐，已记 handoff 待批。」删除（偏差注记本体保留——头部「会话已下载」锚为场景 12 规格口径，逐处核实）；同文件 T/t 门控注释的过时 handoff 指针改「断言按 feature v1.42/FR-02-31 口径」。
   - `qa/runners/suite_download_engine.py`：QA-DE-15 docstring 尾「待 specifier 修订场景 15 THEN-1 与规程行后对齐（已记 handoff 待批）。」删除（feature 场景 15 已随 v1.49 锚点对齐修订为「等待行不显示尺寸…以 fixture 请求日志为准」，与现行断言一致）。
   - `qa/runners/suite_modify_bt.py`：QA-MB-03 断言消息尾「待 specifier 修订规程锚后对齐，已记 handoff 待批」删除并改为对齐口径引证（08-modify-bt-qa.md v1.49 锚点对齐注记已显式声明与 runner 现行断言一致）；断言条件零改动。
2. **全量独立复验（QA 终验基线全面对账，见验证证据表）**：fmt clean、clippy 0W、单测 8060P/0F、e2e 15 套 191P/0F、CRAP/CPD 登记口径对账、arch_check 18 全过、APS 双检抽查 3 feature 全过。
3. **全新沙箱环境与度量工具重建**：nightly 1.102.0（9c5648833，与基线同 commit）+ rsproxy + pip 依赖装齐；cargo-llvm-cov 0.9.1 / PMD 7.29.0 / bb 1.13.229 / APS 仓就位（装位见头部环境行）。
4. **notes 经验回填**：`notes/environment.md`、`notes/testing.md` 各追加本轮 2 条（镜像复测与防爆盘口径 / QA_ONLY 分段对账与 CRAP 复跑简化），`notes/INDEX.md` 同步。
5. **本 handoff.md** 重写为当前有效口径。

### 验证证据（命令可复现）
| 验证项 | 结果 | 命令 |
|---|---|---|
| rustfmt | clean | `cargo fmt --check`（project/ezr） |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| 单元测试 | 8060P / 0F（16 目标逐项持平基线） | `cargo test`（bin ezr 1028 + bins 46 + 集成 12 目标 6986；doc-tests 无 lib 目标 = 0，与基线同口径） |
| e2e 套件 | 15 套全绿 191P / 0F | `python3 suite_*.py`（qa/runners；throttle_proxy 用 `QA_ONLY=QA-TP-05,…` 分段拼接对账） |
| CRAP | 行覆盖 94.2%；ACTIONABLE=103/REG=36，登记族在位无新增 | `cargo llvm-cov --lcov --bin ezr` + `crap_calc.py <lcov> <src_root>` |
| DRY（CPD min200） | 8 克隆族 = 登记口径（bt/engine/mod.rs ×7 + bencode↔mktorrent ×1，产品 0） | `pmd cpd --dir src --language rust --minimum-tokens 200` |
| arch_check | 18 条全过 EXIT=0 | `bash scripts/arch_check.sh` |
| APS 双检（抽查） | 3 feature parser + dry-check 均 EXIT=0 | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-parser <abs>/.work/tmp/… )` |

### 对账口径
- 本轮改动面 = **qa/runners 3 文件的叙述性尾注/断言消息**（suite_tui_display.py、suite_download_engine.py、suite_modify_bt.py）+ notes 三文件 + handoff；**产品源码（project/ezr/src）零改动、qa/ 规程文件零改动、features/ 零改动、ezr-demo 零改动**。
- 测试对账：基线 = QA 终验轮（2026-11-12）8060P/0F + 191P/0F；本轮全量复验**逐项持平**（单测 16 目标分项：1028/30/1/15/247/916/805/801/2/16/254/774/498/788/946/939；e2e 分项：AT 21/CT 5/DE 16/IC 12/MB 4/MT 10/NP 18/PC 11/RB 9/RS 14/SQ 15/TP 10/TD 21/UC 18/UP 7）。runner 改动仅叙述性文本，py_compile 通过，e2e 改动套件 3/3 实跑全绿。
- CRAP/DRY：产品源码零改动 + 同 nightly commit（9c5648833）→ 同管线产出恒等；登记族与克隆族逐项在位，无新增（A/B 恒等判法，notes/testing.md 口径）。

### 待办与待批
- **【观察·移交 specifier】QA-TD-07 规程行「T 切/循环页签」措辞滞后**（本轮自 runner 侧注释指针发现并转登记）：qa/01-tui-display-qa.md 规程行仍含 v1.30 期「]/[/T 切/循环页签」措辞，与 feature v1.42/FR-02-31（T = trackers 弹窗、页签仅 ]/[）不一致；runner 断言已按 feature 权威口径且实跑全绿，仅规程文件叙述性措辞待 specifier 下轮顺手对齐，不阻塞任何验证。
- 变异实跑维持移交 hardender（受限项，操作者显式决定是否续跑）；其余遗留受限项沿历轮口径维持（见第一节）。
- **本轮无待批项、无阻塞项**（裁决轮移交的 runner 尾注待办已收敛结案）。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 本轮为收敛 + 全量复验轮：six-pack 角色链保持 QA 终验轮后的「工作流完结」终态（specifier → coder → cleaner → architect → (hardender 可选) → QA → Done 已走完）；若操作者需 ⑤规程行措辞对齐（specifier）⑥补跑 hardender——均以 `bin/swarm begin <角色>` 显式启动对应角色。
