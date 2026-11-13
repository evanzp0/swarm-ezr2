# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（遗留项裁决轮，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner 收敛轮 ⇒ architect 收敛轮 ⇒ QA 终验轮 ⇒ specifier 显示指标/调整轮 v1.48–v1.51 ⇒ coder 产品实现轮（FR-01-117）⇒ coder 补偿轮（v1.47–v1.50 六项欠账）⇒ cleaner 清理收敛轮 ⇒ architect 架构收敛轮 ⇒ QA 终验轮（2026-11-12）⇒ specifier 锚点对齐轮（2026-11-12）⇒ **specifier 遗留项裁决轮（本轮 2026-11-12，操作者裁决落地）**
> 会话：six-pack-specifier-20261112-r2（任务 = 锚点对齐轮移交的两项待批项经操作者 chat 裁决落地：①05-ui-pieces 扩展修订**保留**结案 ②mission 观察项**出修订注记**；上游交接 = project/handoff.md 锚点对齐轮版 + 操作者 chat 裁决）
> 环境：nightly 1.102.0（9c5648833 2026-11-10，与交接基线同 commit）@ ~/.cargo/bin（TUNA 镜像前台装齐，锚点对齐轮实测 26.9MB/s 最快可用）+ cargo registry rsproxy sparse（~/.cargo/config.toml）；bb 1.13.229（.tools/bin/bb，不在 PATH 需全路径调用）+ APS 仓 .tools/Acceptance-Pipeline-Specification；度量四件本轮未用（llvm-cov/PMD 未装机，specifier 职责面未用）；harness 依赖未动（libtorrent/torf/pyte 沿 pip 阿里云镜像口径）

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.52**）+ `mission/` 两份 phase 详述（**phase-01 v1.51**、**phase-02 v1.20**）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-10 QA 终验轮） | ezr 产品实现 + 15 套件 e2e + 引擎拆分 + 终验 | 已交付 | 历轮对账在案 |
| specifier 显示指标/调整轮 v1.48–v1.51（2026-11-10/11） | phase-01 v1.50 / phase-02 v1.20 + mission.md v1.47–v1.51 + features/qa 配套 + ezr-demo 同构修改 | 已交付 | 历轮在案 |
| coder 产品实现轮（2026-11-11，FR-01-117） | project/ezr 源码 6 文件 + 守卫三守卫 | 已交付 | cargo test 在案 |
| coder 补偿轮（2026-11-11，v1.47–v1.50 六项欠账） | project/ezr 源码 9 文件 + 守卫 10 条 | 已交付 | 在案 |
| cleaner 清理收敛轮（2026-11-12） | lint 基线恢复 + conn_flags 表驱动 + CRAP 收敛 + 守卫 8 条 + notes 三分类 | 已交付 | 在案 |
| architect 架构收敛轮（2026-11-12） | 四阶段评审 + VS15 工具族单源化 + arch_check 18 条 + 属性 +4 | 已交付 | fmt clean；clippy 0W；8060P/0F；arch_check 18 全过；属性 86P |
| QA 终验轮（2026-11-12） | 全量独立复验 + e2e 15 套 191P + APS 20 feature 双检 + CRAP/DRY 度量 + 修复 4 项 + notes/INDEX | 已交付 | 与交接基线逐项持平（8060P/0F、191P/0F） |
| specifier 锚点对齐轮（2026-11-12） | v1.49 观察锚漂移 4 处待批项修订 + 同类漂移扩展 1 处（供否决）+ mission 观察项登记 + notes/handoff | 已交付 | APS 双检 2 份修订 feature 全过（parser + dry-check EXIT=0）；产品源码零改动，基线 8060P/0F 沿用 |
| **specifier 遗留项裁决轮（本轮 2026-11-12）** | **待批项①②经操作者裁决落地：05-ui-pieces 扩展修订保留结案（头部注记回填）+ mission 出 FR-01-116① 修订注记（mission.md v1.52 / phase-01 v1.51 / 行内注记三层落点）+ notes 闭环回填** | 已交付 | **APS 双检 05-ui-pieces 全过（parser 7 场景 + dry-check 均 EXIT=0）；产品源码零改动，基线 8060P/0F 沿用** |

### 遗留受限项（当前有效，沿历轮口径）
- leech 专属 e2e、变异实跑（hardender 未跑，`--file` 分块消费面就绪）、6 个 BT/命令窗口族无专属 runner、ws(s) announce 无代理面。
- ezr-demo verify_ezr.py 预存 FAIL 15 项维持（脚本锚点漂移，非产品回归）；ezr-demo 零改动（锚点对齐轮起维持）。
- CRAP comp 残余登记沿 cleaner 复核口径维持（run/on_evt/draw_conns/pieces/restore/overlay_open/on_cmd_key/fake_udp_tracker 族，QA 终验轮全目标复跑抽查无新增可修点）。
- 变异点 >100 四文件登记维持（app/mod.rs / bt/engine/conn.rs / ui/text.rs / model/task.rs）。
- CPD min200 登记维持：bt/engine/mod.rs 测试克隆 ×7 + ezr-mktorrent ↔ bencode 设计性重复 ×1（产品家族 0）。

### 实现定义值登记（累计，当前有效口径）
- D44–D74 沿历轮登记不变，锚点对齐轮与裁决轮均无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（specifier 遗留项裁决轮）
1. **待批项①结案（05-ui-pieces 扩展修订保留）**：`features/05-ui-pieces.feature` 头部锚点注记「登记供否决」回填「操作者 20261112 裁决保留，结案」；场景 06 行文零改动（保留即无回退，D35 谓词与 toast 文案维持对齐后口径）。
2. **待批项②结案（mission 出修订注记，三层落点）**：
   - `mission.md` v1.52：标题版本递进 + 头部裁决注记（①保留结案 ②出注记，详见 phase-01 v1.51）；
   - `mission/phase-01.md` v1.51：头部 v1.51 注记 + **FR-01-116① 行内注记**——v1.34「统计行 `<已下载>/<总大小>` 显示 `<已下载>/? B`」随 v1.49 明细尺寸上移退役（列表统计行与任务行行 1 均不显示尺寸项、无伪造总字节），`<已下载> / ? B` 观察载体收窄为②详情「大小」行；①其余臂（`?%` 百分比/走马灯/空轨道/剩余 `-`/速度）与②③④不变；
   - 规格与 QA 套件零改动：现行口径已由 01-tui-display v1.49（场景 02/28）/ QA-TD-28 v1.49 / phase-02 v1.19 承载，本注记仅 mission 侧措辞对齐（宪法第一章「用户明确要求修订」例外，操作者 chat 指令「修改 mission」为授权依据）。
3. **notes 闭环回填**：`notes/testing.md`「mission 只读约束下的口径张力处理」条目补裁决闭环形态（显式指令即例外成立 + 登记类注记裁决后必须回填结案态）+ `notes/INDEX.md` 对应行同步。
4. **APS 双检**：05-ui-pieces.feature parser（7 场景）+ gherkin-ir-dry-checker 均 EXIT=0（头部注记回填后复检）。
5. **本 handoff.md** 重写为当前有效口径。

### 验证证据（命令可复现）
| 验证项 | 结果 | 命令 |
|---|---|---|
| APS 解析 | 7 场景通过，EXIT=0 | `(cd .tools/Acceptance-Pipeline-Specification && .tools/bin/bb gherkin-parser <abs>/project/features/05-ui-pieces.feature <abs>/.work/tmp/05-ui-pieces.json)` |
| APS dry-check | EXIT=0 | `.tools/bin/bb gherkin-ir-dry-checker <abs>/.work/tmp/05-ui-pieces.json <abs>/.work/tmp/05-ui-pieces.dry.json` |
| 修订文本在位 | 三层落点 grep 逐字命中 | `grep -c "v1.51 注记：统计行臂随 v1.49" project/mission/phase-01.md`（=1）；`grep -c "v1.52" project/mission.md`（头部注记 + 标题 ≥2）；`grep -n "裁决保留，结案" project/features/05-ui-pieces.feature` |

### 对账口径
- 本轮改动面 = **纯 mission 文档 + feature 头部注记**：mission.md、mission/phase-01.md、features/05-ui-pieces.feature（仅头部注记 2 行，场景正文零改动）+ notes/testing.md、notes/INDEX.md + handoff；**产品源码（project/ezr/src）零改动、qa/（含 runners）零改动、ezr-demo 零改动**。
- 测试对账：产品基线 8060P/0F 与 e2e 191P/0F（QA 终验轮）**不重跑沿用**——本轮零源码零 runner 改动，feature 场景正文零改动（头部注记为说明性注释，parser/dry-check 复检通过），基线有效。
- 措辞对齐核验：FR-01-116① 行内注记新口径与 QA-TD-28 v1.49 判据（「统计行与任务行行 1 均不显示尺寸项……详情大小行 `<已下载> / ? B`（保留不变）」）及 phase-02 v1.19 注记（「含未知尺寸 <已下载>/? B 形态均上移退役」）三方一致。

### 待办与待批
- **【观察·移交 QA/coder】三份 runner docstring 尾注「待 specifier 修订后对齐」已过时**（锚点对齐轮登记，本轮未处理）：断言本体与修订后规格已一致，仅叙述性尾注陈旧；清理属 qa/runners 改动面（specifier 不改 runner），建议下轮 QA 或 coder 会话顺手收敛，不阻塞任何验证。
- 变异实跑维持移交 hardender（受限项，操作者显式决定是否续跑）；其余遗留受限项沿历轮口径维持（见第一节）。
- **本轮无待批项**（锚点对齐轮移交的两项待批均已结案），无其他阻塞项。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 本轮仍为文档对齐轮：six-pack 角色链保持 QA 终验轮后的「工作流完结」终态；若操作者需 ③收敛 runner 尾注（QA/coder）④补跑 hardender——均以 `bin/swarm begin <角色>` 显式启动对应角色。
