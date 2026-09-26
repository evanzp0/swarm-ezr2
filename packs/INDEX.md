# 技能总目录（工作流 → 角色 → 技能路径）

> 供 LLM 按需读取：先在本索引定位技能路径与摘要，再读取对应 `SKILL.md` 全文。
> 开工前必读根目录 `CONSTITUTION.md`（总纲、计划制度 plan.md、工作区纪律与交接、项目使命 `project/mission.md`）。

## 工作流总览

| 工作流 | 角色数 | 角色链 |
|---|---|---|
| adversaries | 2 | coder ⇄ reviewer（对抗式评审循环，reviewer 满意后结束） |
| two-pack | 2 | coder → cleaner |
| four-pack | 4 | specifier → coder → refactorer → architect |
| six-pack | 6 | specifier → coder → cleaner → architect → hardender → QA |
| squad | 13 | squad-leader 编排：analyst → gherkin-writer → qa-procedure-writer → implementer → cleaner → code-reviewer → hardener → qa → architect → senior-implementer；常驻：troubleshooter（排障）；支撑：system-analyst（产品框架） |

> 角色链仅为**可用编排顺序**：`complete` 归档即会话终结，不自动流转到下一角色；
> 续跑由操作者显式触发（宪法第一章「不自动流转」）；下游角色以 `project/handoff.md` 为上游交接内容
> （宪法第一章「上游交接双来源」，不存在时以操作者 chat 输入为上游交接内容）。

## adversaries

| 角色 | 技能路径 | 摘要 |
|---|---|---|
| coder | `packs/adversaries/coder/SKILL.md` | 用 TDD 实现功能并根据 reviewer 建议迭代修改 |
| reviewer | `packs/adversaries/reviewer/SKILL.md` | 对抗式严格评审 coder 实现，产出建议文件或批准文件 |

## two-pack

| 角色 | 技能路径 | 摘要 |
|---|---|---|
| cleaner | `packs/two-pack/cleaner/SKILL.md` | 快速后端工作流：清理、CRAP/DRY 审查、架构审查、封装修复与变异加固 |
| coder | `packs/two-pack/coder/SKILL.md` | 快速后端工作流：以 TDD 和单元测试实现所请求的行为 |

## four-pack

| 角色 | 技能路径 | 摘要 |
|---|---|---|
| architect | `packs/four-pack/architect/SKILL.md` | 紧凑规格工作流：架构评审、依赖方向、变异加固、DRY、软 Gherkin 变异与完成通知 |
| coder | `packs/four-pack/coder/SKILL.md` | 紧凑规格工作流：以 TDD、单元测试和生成的验收测试实现已批准的行为切片 |
| refactorer | `packs/four-pack/refactorer/SKILL.md` | 紧凑规格工作流：保持行为不变的清理、覆盖率提升、CRAP/DRY、变异点扫描与属性测试 |
| specifier | `packs/four-pack/specifier/SKILL.md` | 紧凑规格工作流：Inversion 选择题访谈（使用 AskUserQuestion 方式）收齐意图后，将用户意图转化为精确的 Gherkin 验收规格 |

## six-pack

| 角色 | 技能路径 | 摘要 |
|---|---|---|
| QA | `packs/six-pack/QA/SKILL.md` | 完整工作流：最终独立验证、QA 规程可执行化、UI 层端到端验证与完成通知 |
| architect | `packs/six-pack/architect/SKILL.md` | 完整工作流：架构评审、模块边界、依赖方向与属性测试覆盖率 |
| cleaner | `packs/six-pack/cleaner/SKILL.md` | 完整工作流：保持行为不变的清理、覆盖率提升、CRAP/DRY 审查与变异点扫描 |
| coder | `packs/six-pack/coder/SKILL.md` | 完整工作流：以 TDD、单元测试和生成的验收测试实现已批准的行为切片 |
| hardender | `packs/six-pack/hardender/SKILL.md` | 完整工作流：变异加固、语言变异、CRAP/DRY 验证与软 Gherkin 变异 |
| specifier | `packs/six-pack/specifier/SKILL.md` | 完整工作流：Inversion 选择题访谈（使用 AskUserQuestion 方式）收齐意图后，产出已认可的 Gherkin 规格与端到端 QA 套件规格 |

## squad

| 角色 | 技能路径 | 摘要 |
|---|---|---|
| analyst | `packs/squad/analyst/SKILL.md` | 临时分析师：为单个故事编写实现计划（唯一产物 implementation-plan.md） |
| architect | `packs/squad/architect/SKILL.md` | 临时架构师：架构评审并提建议，保持模块图与依赖规则最新 |
| cleaner | `packs/squad/cleaner/SKILL.md` | 临时清理者：保持行为不变的清理与属性测试支持 |
| code-reviewer | `packs/squad/code-reviewer/SKILL.md` | 临时代码评审者：只写建议，交给加固者落实 |
| gherkin-writer | `packs/squad/gherkin-writer/SKILL.md` | 临时 Gherkin 编写者：把已批准的故事转化为精确的 Gherkin feature 文件 |
| hardener | `packs/squad/hardener/SKILL.md` | 临时加固者：落实 reviewer 建议后按 six-pack 方式加固，达标或交回阻塞项 |
| implementer | `packs/squad/implementer/SKILL.md` | 临时实现者：精确实现故事，单元优先，按需构建 APS 验收流水线 |
| qa | `packs/squad/qa/SKILL.md` | 临时 QA：对故事包或批次做最终独立验证并产出可执行 QA 脚本 |
| qa-procedure-writer | `packs/squad/qa-procedure-writer/SKILL.md` | 临时 QA 规程编写者：产出 QA 规程与 implementer notes（两个文件一次交接） |
| senior-implementer | `packs/squad/senior-implementer/SKILL.md` | 临时资深实现者：落实选定的架构建议，在改进结构的同时保持行为 |
| squad-leader | `packs/squad/squad-leader/SKILL.md` | 小队负责人：流水线残余编排与用户沟通，绝不撰写产品产物 |
| system-analyst | `packs/squad/system-analyst/SKILL.md` | 临时系统分析师：产出产品框架可执行程序、frame.md 与 qa/product.md |
| troubleshooter | `packs/squad/troubleshooter/SKILL.md` | 排障者：swarm 流水线的操作者与修复者，被呼叫前保持空闲 |

## 共享条款与引用资料

| 文件 | 作用 |
|---|---|
| `packs/_common/engineering.md` | 工程规则（工具启动、语言默认项、验证、测试执行纪律、护栏） |
| `packs/_common/handoff-rule.md` | 交接文件 `project/handoff.md` 的结构、书写规则与例子（历史总账 + 当前产出两节；整理重写不追加；不含技术经验）；所有角色写/重写交接时套用 |
| `packs/_common/notes/rust.md` | Rust 专项注意事项（含 Rust 的角色开工前先读）：工具链/PATH、lld 桥接、强制配置模板（Cargo.toml lints、clippy.toml、rustfmt.toml）、编译 lint、编码实践、跨平台 IPC 事实 |
| `packs/_common/notes/aps.md` | APS 验收流水线与 Babashka 专项注意事项：dry-checker 中文假阳性、IR 展开行口径、跑块目录重建 |
| `packs/_common/notes/mutation-hardening.md` | 变异加固策略（语言无关）：会话隔离（含 worker 副本隔离与真实树零接触）、pristine 备份、基线同语境、清单制差分变异、分轮收敛、存活突变体与等价论证高频模式库（随复盘开放增长）、加固测试写法、交接四要素；six-pack/hardender 与 squad/hardener 引用 |
| `packs/squad/reference/clean-architecture.md` | squad 各角色：模块边界与依赖方向 |
| `packs/squad/reference/tool-table.edn` | squad 各角色：语言工具表（变异/CRAP/DRY/APS） |

角色上下文从一手资料获取：上游交接来源（`project/handoff.md` 存在 → 以该文件为上游交接内容；不存在 → 以操作者 chat 输入为上游交接内容，宪法第一章「上游交接双来源」）、`project/mission.md`（项目原始需求档案，各工作流第一个角色按「需求存放两模式」建档、其他角色只读；分期模式含 `project/mission/` 分期详述，宪法第一章）、`project/`（既有产物与角色产出跨会话保留在工作区，角色只访问 `project/` 下的文件）与本索引所列技能文件；交接说明写入 `project/handoff.md`（下游角色以其为上游交接内容；结构与书写规则见 `packs/_common/handoff-rule.md`；技术经验由角色在交接前直接写入 `packs/_common/` 工具侧——通用纪律归 `engineering.md`、语言/工具/框架专项归 `notes/` 分类文件，不写入交接文件）。流水线用法、目录结构、命令速查与工作区纪律：见根目录 `CONSTITUTION.md`。

## 编程语言配置约束
- rust
    - 如果项目使用的是 rust 语言，那么配置文件就必须使用 `packs/_common/notes/rust.md` 中的配置模板（含 `Cargo.toml` 的 lints 配置、`clippy.toml`、`rustfmt.toml`），工作流的开发和测试中都要使用。
    - Rust 专项问题与解法（工具链/PATH、lld 桥接、编译 lint、平台 IPC 等）同见 `packs/_common/notes/rust.md`，含 Rust 的角色开工前先读。


