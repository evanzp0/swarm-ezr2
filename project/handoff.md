# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（规程措辞对齐轮，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner 收敛轮 ⇒ architect 收敛轮 ⇒ QA 终验轮 ⇒ specifier 显示指标/调整轮 v1.48–v1.51 ⇒ coder 产品实现轮（FR-01-117）⇒ coder 补偿轮（v1.47–v1.50 六项欠账）⇒ cleaner 清理收敛轮 ⇒ architect 架构收敛轮 ⇒ QA 终验轮（2026-11-12）⇒ specifier 锚点对齐轮（2026-11-12）⇒ specifier 遗留项裁决轮（2026-11-12）⇒ QA 收敛轮（2026-11-13）⇒ **specifier 规程措辞对齐轮（本轮 2026-11-13，QA 收敛轮移交观察项落地：QA-TD-07 规程行对齐 feature v1.42/FR-02-31）**
> 会话：six-pack-specifier-20261113（任务 = QA 收敛轮移交观察项「QA-TD-07 规程行『T 切/循环页签』措辞滞后」对齐结案；上游交接 = project/handoff.md QA 收敛轮版）
> 环境：nightly（rustc 1.102.0-nightly 8eb778d04 2026-11-12）@ ~/.cargo/bin（rsproxy 镜像前台装齐，本轮实测 ~16.4MB/s 最快、dist 清单 date 2026-11-13 有效）+ cargo registry rsproxy sparse（~/.cargo/config.toml）；pip 依赖 pyte/wcwidth（阿里云镜像装齐）。度量与 APS 四件（cargo-llvm-cov / PMD / bb / APS 仓）本轮未装机——对齐轮验证面无需（沿 architect 收敛轮「工具面收窄」先例），hardender/QA 轮开工先按「工具启动」条款重建。

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
| QA 收敛轮（2026-11-13） | 裁决轮移交待办③收敛落地：三份 runner 过时尾注清理（断言本体零改动）+ 全量独立复验收官（单测 8060P/0F、e2e 191P/0F、CRAP/DRY/arch_check/APS 全对账）+ 工具链与度量四件全新沙箱重建 | 已交付 | 单测 8060P/0F、e2e 15 套 191P/0F 与基线逐项持平；CRAP/CPD 登记口径精确对账无新增 |
| **specifier 规程措辞对齐轮（本轮 2026-11-13）** | **QA 收敛轮移交观察项落地：qa/01-tui-display-qa.md QA-TD-07 规程行「T 切/循环页签」滞后措辞对齐 feature v1.42/FR-02-31（头部补对齐注记；叙述性零判据语义改动）** | 已交付 | **suite_tui_display.py 全套 21P/0F/0S（QA-TD-07 在内，runner 断言零改动）；features/、产品源码、qa/runners、ezr-demo 零改动，单测基线 8060P/0F 沿用** |

### 遗留受限项（当前有效，沿历轮口径）
- leech 专属 e2e、变异实跑（hardender 未跑，`--file` 分块消费面就绪）、6 个 BT/命令窗口族无专属 runner、ws(s) announce 无代理面。
- ezr-demo verify_ezr.py 预存 FAIL 15 项维持（脚本锚点漂移，非产品回归）；ezr-demo 零改动（锚点对齐轮起维持）。
- CRAP comp 残余登记沿 cleaner 复核口径维持（run/on_evt/draw_conns/pieces/restore/overlay_open/on_cmd_key/fake_udp_tracker 族）——QA 收敛轮 bin 套件口径复跑：行覆盖 94.2%（37249/39559）、ACTIONABLE=103/REG=36，登记族全部在位；产品源码零改动 + 同管线产出恒等，无新增可修点。
- 变异点 >100 四文件登记维持（app/mod.rs / bt/engine/conn.rs / ui/text.rs / model/task.rs）。
- CPD min200 登记维持：bt/engine/mod.rs 测试克隆 ×7 + ezr-mktorrent ↔ bencode 设计性重复 ×1（产品家族 0）——QA 收敛轮复跑 8 克隆族精确对账。

### 实现定义值登记（累计，当前有效口径）
- D44–D74 沿历轮登记不变，本轮无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（specifier 规程措辞对齐轮）
1. **qa/01-tui-display-qa.md（本轮唯一改动文件）**：
   - QA-TD-07 通过判据行 v1.30 期「]/[/T 切/循环页签（v1.30：Tab 不再切页签）」滞后措辞修订为「]/[ 切页签（v1.30：Tab 不再切页签；v1.42/FR-02-31：原 T 页签循环切换退役，页签切换仅 ]/[ 承担）；T/t 打开选中 BT 任务的 trackers 弹窗（v1.42/FR-02-31：未选中/HTTP 任务 toast 门控拒绝且不开窗、页签不变，窗口细则见 11-trackers-window-01/02）」——与 feature 场景 07（v1.42/FR-02-31）权威口径及 runner 现行断言一致。
   - 头部新增「规程措辞对齐（specifier 对齐轮 20261113，QA 收敛轮移交观察项）」注记（沿 v1.49 锚点对齐注记先例；语义零变更仅叙述对齐，不递增 feature 版本号，注记锚定权威语义版本 v1.42/FR-02-31）。
2. **notes 经验回填**：`notes/environment.md` 追加本轮镜像复测 1 条（rsproxy 本轮最快且清单有效新鲜）；`notes/testing.md` 锚点对齐轮条目按 DRY 去重合并补「移交指针注释同形态」实证；`notes/INDEX.md` 无新增文件不改。
3. **本 handoff.md** 重写为当前有效口径。

### 验证证据（命令可复现）
| 验证项 | 结果 | 命令 |
|---|---|---|
| e2e 改动面关联套件 | 21P / 0F / 0S（QA-TD-07 在内全绿，EXIT=0） | `python3 suite_tui_display.py`（qa/runners） |
| runner 基线零改动核验 | suite_tui_display.py 与 QA 收敛轮交付基线 diff 0 行 | `git diff project/qa/runners/` |
| 旧措辞清零核验 | 规程行与 features/ 检索「T 切/循环页签」为零（仅头部对齐注记引用旧措辞作历史锚） | grep `T 切/循环页签` project/qa/ project/features/ |
| py_compile | 语法通过 | `python3 -m py_compile qa/runners/suite_tui_display.py` |

### 对账口径
- 本轮改动面 = **qa/01-tui-display-qa.md 单文件叙述性修订**（头部注记 + QA-TD-07 判据行措辞）；**产品源码（project/ezr/src）零改动、features/ 零改动、qa/runners 零改动（与 QA 收敛轮交付基线逐字节一致）、ezr-demo 零改动、qa/ 其余规程文件零改动**。
- 判据语义对账：QA-TD-07 行修订仅为叙述对齐——runner 断言（] 切「已完成」/[ 回「正在下载」、T/t toast 门控拒绝不开窗且页签不变、页签往返快照还原）改动前后逐条不变，实跑 21P/0F 即语义一致性证据；产品源码零改动且单测与本轮改动面无交集 → 单测基线 8060P/0F 沿用（QA 终验轮 / QA 收敛轮口径）。

### 待办与待批
- **【观察·移交 QA 侧顺手清理，不阻塞】**：suite_tui_display.py QA-TD-07 T/t 门控注释尾「规程行『T 切/循环页签』滞后项已移交操作者/specifier 待办」随本轮规程行对齐结案转为过时叙述——清理属 qa/runners 改动面（沿 notes/testing.md「specifier 不改 runner」先例），移交 QA/six-pack 下轮顺手闭合，不阻塞任何验证。
- 变异实跑维持移交 hardender（受限项，操作者显式决定是否续跑）；其余遗留受限项沿历轮口径维持（见第一节）。
- **本轮无待批项、无阻塞项**（QA 收敛轮移交的规程行措辞观察项已收敛结案）。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 本轮为叙述性对齐单点轮：six-pack 角色链保持 QA 终验轮后的「工作流完结」终态（specifier → coder → cleaner → architect → (hardender 可选) → QA → Done）；QA 收敛轮移交建议中的 ⑤规程行措辞对齐已由本轮结案，若操作者需 ⑥补跑 hardender 或启动下一新需求——均以 `bin/swarm begin <角色>` 显式启动对应角色。
