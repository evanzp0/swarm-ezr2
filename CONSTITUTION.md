# swarm 宪法（CONSTITUTION）

本宪法对所有技能（工作流/角色）生效，优先级高于各 `SKILL.md` 与 `packs/_common/` 条款；与之冲突时以本宪法为准。
本宪法是流水线的唯一总文档：总纲、计划制度、工作区纪律与交接、操作手册与会话规则均在此维护（原 README 与 `packs/_common/workspace.md` 已并入）。
全局规则各有唯一来源：制度性内容以本宪法为唯一来源，工程与工具规则以 `packs/_common/engineering.md`、语言/工具/框架专项注意事项以 `packs/_common/notes/` 为唯一来源——其他文件只做引用与角色特有补充，不得复述（见第五章「DRY 约束」）。
主控脚本 `bin/swarm` 在 `begin` 时把上游产物复制进 `project/` 并提示角色阅读本宪法；`complete` 时打包出箱并检查执行情况。行为明细见第四章「标准工作流程」。

## 第一章 总纲与目录分工

- **流水线总纲**：swarm 是单角色顺序流水线——上游工作流产物经 `inbox/` 进入，由指定角色技能在 `project/` 中加工，完成后打包到 `outbox/` 交付操作者。技能包在 `packs/`（总目录 `packs/INDEX.md`）。
- **会话节律**：每个技能的一次会话遵循固定节律——接收任务（操作者指令与 `project/mission.md`）→ 制定计划（`.work/plan.md`，第二章）→ 执行并持续更新计划 → 产出（写入 `project/`）。
- **目录分工**（各目录只承担一种职责）：
  - `project/`——**唯一工作区**：上游产物副本（`begin` 时自动复制）、`mission.md`、`handoff.md` 与角色产出文件；
  - `.work/`——**当前工作流运行时状态**：`session`（主控脚本内部状态，勿手工编辑）、`plan.md`（工作计划，第二章）、`tmp/`（临时文件）；
  - `inbox/`——上游产物暂存区，由主控脚本管理：`unpack` 解压至此（按产物名建子目录），`begin` 时复制进 `project/`；
  - `outbox/`——完成的产物包：`<时间戳>-<工作流>-<角色>.zip`；
  - `.tools/`——工具缓存：跨会话复用，安装/构建工具时创建。
  访问与写入纪律见第三章。
- **项目使命（project/mission.md）**：
  - 各工作流的**第一个角色**（adversaries/coder、two-pack/coder、four-pack/specifier、six-pack/specifier、squad/squad-leader）负责接收用户的**原始项目需求**并记录为 `project/mission.md`：会话开始时若该文件尚不存在，依据用户原始需求如实建档（项目目标、范围、约束、验收期望等；保持原始意图，不增删曲解）；已存在时保持不变，除非用户明确要求修订。
  - 流程中的**其他角色一律只读**，**不得修改**其内容；缺失时向操作者报告，不得臆造项目信息。
  - `project/mission.md` 是关于项目需求的唯一来源文件，任何流程都**不能自行创建任务简报文件（比如: brief.md）或其他项目描述文件（比如: requirements.md）**；所有角色通过 `project/mission.md` 与 `project/` 内上游产物了解项目信息。
  - `project/mission.md` 属于项目产物，随产物包流转给下游。 
- **不自动流转（会话终结规则）**：`complete` 打包出箱即本轮任务终点，产物包交付操作者；流水线与主控脚本**不会自动流转**到工作流的下一个角色。是否续跑由操作者显式决定；仅当操作者明确要求续跑时才手动链式交接（命令见第四章「标准工作流程」）。各 `SKILL.md` 的"移交建议"仅为给操作者的建议，不构成自动续跑。
- **上下文与技能定位**：上下文从一手资料获取——`project/mission.md` 与 `project/`（上游产物）、技能文件与 `_common/` 条款；不设外部检索库。技能定位先查 `packs/INDEX.md` 再读对应 `SKILL.md`；工程与工具规则见 `packs/_common/engineering.md`，语言/工具/框架专项注意事项见 `packs/_common/notes/`（索引在 `packs/INDEX.md`），交接文件结构与书写规则见 `packs/_common/handoff-rule.md`。

## 第二章 计划制度（plan.md）

- **开工前必须先计划**：任何技能在开始实际工作前，必须依据会话任务与 `project/`（含 `project/mission.md`）的工作内容，先生成 `.work/plan.md`，列出完成当前任务需要进行的操作。
- plan.md 使用复选框步骤清单，格式如下：

  ```markdown
  # 工作计划

  - 任务：<沿用会话的任务名>
  - 技能：<工作流>/<角色>

  1. [ ] <步骤：要执行的操作、涉及文件、预期产出>
  2. [ ] <步骤…>
  ```

- **执行中勾选核销**：某步骤开始时可标注进行中；**每完成一步，立即把该步的 `[ ]` 改为 `[x]` 核销掉，并在该行末尾附简短结果**（如 `→ 产出 docs/x.md`）。plan.md 必须随时反映真实进度：做完一步勾一步，禁止事后批量补勾或凭记忆回填——会话无论因何中断，续做完全依赖 plan.md 的勾选状态。
- **中断恢复**：无论何种原因中断（会话退出、上下文丢失、超时等），恢复工作的第一步是读取 `.work/plan.md`，核对已完成（已勾选）与未完成（未勾选）步骤，从第一个未完成步骤继续；不要重做已完成步骤，也不要凭记忆臆造进度。若 plan.md 本身丢失，先依据 `project/` 中已存在的产物评估实际进度、重建剩余步骤清单后再续做。
- plan.md 属于**会话级运行状态**：随会话存续于 `.work/`，不进 `project/`、不打包进 outbox zip；`bin/swarm complete` 或 `reset` 会随会话清除它。
- **命名区分**：`.work/plan.md` 专指本宪法要求的**工作计划**；squad 工作流 analyst 的实现计划产品为 `project/implementation-plan.md`，两者不得混淆或相互覆盖。

## 第三章 工作区纪律与交接说明

### 工作区纪律

- 只在 `project/`（工作内容）与 `.work/`（运行时状态）内工作，只访问 `project/` 下的文件、不准访问 `inbox/` 下的文件（`inbox/` 由主控脚本管理）。不要写入 `bin/`、`outbox/` 或 swarm 目录之外的任何位置（例外：`.tools/` 可在安装/构建工具时写入；`packs/_common/` 仅限按「踩坑与经验沉淀」条款写入经验条目，`packs/` 其余内容只读）。
- 上游产物副本已在 `project/` 中，按角色职责在其中直接加工；不要回 `inbox/` 取用、比对或复原文件。
- 临时文件统一放 `.work/tmp/`，不要使用 `/tmp`；解析、dry-check、交接草稿等输出一律写入 `.work/tmp/`。不要把会话状态（plan.md）写进 `project/`，也不要把产物写进 `.work/`。
- 不要阅读或依赖 `.work/session`——那是主控脚本的内部机制。
- 不做角色范围之外的工作。发现超出职责的问题时，在答复操作者时说明（阻塞与风险/移交建议），不要自行越权修复。
- **踩坑与经验沉淀（交接前强制步骤）**：每次交接前，把本轮工作中沉淀出的**可复用经验**自行写入工具侧 `packs/_common/`，按归属分流：
  - **通用经验**（不针对特定语言、框架和工具的纪律/手法/反模式）→ 写入 `packs/_common/engineering.md`，合并进最合适的既有章节（无合适章节时新增小节）；
  - **专项经验**（与特定语言、框架或工具相关）→ 写入 `packs/_common/notes/` 对应分类文件（索引见 `packs/INDEX.md`）；没有合适分类文件时**新建**（命名 `<主题>.md`，并在 `packs/INDEX.md` 补一行索引）。
  条目写法：只写可复用的结论与纪律——现象/处置/根因三段式或直接可执行的一条规则，不写会话叙事；写入前先检索既有内容，**去重合并**（DRY 约束，第五章），修改既有条目时保持其原文结构。确属制度性修订（宪法/`SKILL.md` 条款）的，不自行改动，在答复操作者时提出修订建议。经验沉淀在 `packs/` 统一维护，**不写入 `handoff.md`、不随产物包流转**（交接文件只含交接内容，见下）；踩坑成本只应付一次，让后续会话与下游角色免于重踩。

### 交接说明（project/handoff.md）

- 角色之间不直接交接：产物由操作者运行 `bin/swarm complete` 打包进 `outbox/`，随产物 zip 流转给下游角色（会话终结规则见第一章）；需要下游角色与操作者了解的**交接内容**写入 `project/handoff.md`——结构、书写规则与例子见 `packs/_common/handoff-rule.md`：
  - **产物历史完成情况**：流程开工以来各节点产物的累计状态总账（什么已完成、验证状态、
    遗留受限项、实现定义值登记），每次会话整理重写为当前有效口径；
  - **当前产出情况**：本会话产出的产物清单、验证证据、对账口径、待办与待批、移交建议，
    每次会话重写（上一会话的"当前"已并入历史总账）。
- **交接文件不含开发、验收等技术经验与注意事项**——那些属于项目经验，按「踩坑与经验
  沉淀」条款在交接前直接写入工具侧 `packs/_common/`（engineering.md 或 notes/ 分类文件），
  不随产物包流转。
- 需要用户批准或澄清的内容：在答复操作者时列出（含产物路径），不要臆测补齐；异步会话下
  一并写入 `project/handoff.md` 的待办与待批节。
- 工作被阻塞（blocked）时：在答复操作者时报告，写明已尝试的路径与缺失的输入，让操作者
  可以补救；重要事实可一并记入 `project/handoff.md`。
- 交接说明保持简洁、明确、结构化（按 handoff-template 的两节结构）；**不要**包含验证
  过程日志或本角色的流程叙述。
- 角色工作结束后告知操作者运行 `bin/swarm complete`；不要自行删除或移动 `project/` 内容
  （`mission.md` 按第一章的建档/只读规则处理）。

### 通用失败条件

- 输入缺失、相互矛盾，或对角色而言不自洽完备时：报告 blocked 并详细说明（缺什么、已尝试什么），不要臆测补齐，也不要默默在错误的位置工作或凭空编造输入。
- 必需的验证失败且你是该产物责任人：继续修复，直到通过或遇到真正阻塞。
- 必需的验证失败且你是评审者：按角色职责报告未通过，不直接修复（评审类角色的修复建议写入建议文件）。

## 第四章 流水线操作

### 目录结构

```text
swarm/
  bin/swarm          主控脚本（本流水线的唯一入口）
  inbox/             上游产物暂存区（第一章）
  project/           唯一工作区：上游产物副本、mission.md、handoff.md 与角色产出（第一章）
  outbox/            完成的产物包：<时间戳>-<工作流>-<角色>.zip
  .work/             当前工作流运行时状态：session、plan.md（第二章）、tmp/
  .tools/            工具缓存：跨会话复用，安装工具时创建
  packs/             角色技能包（总目录 packs/INDEX.md，供 LLM 按需读取）
    _common/         共享条款：engineering.md（工程规则）、handoff-rule.md（交接书写规则）、
                     notes/（语言/工具/用途专项注意事项）
    adversaries/     coder、reviewer
    two-pack/        coder、cleaner
    four-pack/       specifier、coder、refactorer、architect
    six-pack/        specifier、coder、cleaner、architect、hardender、QA
    squad/           squad-leader、troubleshooter + 11 个角色模板 + reference/
  CONSTITUTION.md    宪法（本文件）
```

### 命令速查

```sh
bin/swarm list                          # 列出全部 27 个技能（工作流/角色）
bin/swarm chains                        # 查看各工作流的角色流转路径
bin/swarm unpack <产物.zip|目录>         # 先清空 inbox/ 与 outbox/，再解压/复制到 inbox/<名字>/
bin/swarm begin <工作流>/<角色>          # 开始会话，自动把上游产物复制进 project/（可加 --input <名字> --task <名称>）
bin/swarm status                        # 查看会话、plan.md 进度（x/y 步完成）、inbox/outbox
bin/swarm complete [--force] [--keep]   # 打包 project/ → outbox/（打包前自动清理构建缓存），归档已消费的 inbox 输入
bin/swarm reset                         # 放弃当前会话（清空 project/ 与会话级状态）
bin/swarm pack                          # 打包工具本体（bin/、packs/、CONSTITUTION.md）为 .tar.gz 置于当前目录
bin/swarm help                          # 用法
```

### 标准工作流程

1. **上传/解压**：`bin/swarm unpack 上游.zip`——视为收到新上游：先清空 `inbox/` 与 `outbox/` 的全部内容（保留 .gitkeep 骨架，含 `inbox/.consumed/` 归档与旧产物包），再把上游产物解压到 `inbox/<名字>/`；新上游到达即一轮新流水线开始。接着简单的输出所有工作流的流程和可选的 `bin/swarm` 指令即可。
2. **选择技能**：按用户提示确定处理该产物的工作流角色——查 `packs/INDEX.md` 或 `bin/swarm list`。
3. **开始会话**：`bin/swarm begin <工作流>/<角色>`——把 `inbox/<名字>/` 下的文件与子目录复制到 `project/` 下，并输出输入清单与 `project/mission.md` 状态提示（首角色建档/其他角色只读，见第一章）。
4. **计划先行**：角色按第二章生成 `.work/plan.md`，执行中逐项勾选核销、中断凭它续做（详见第二章）。
5. **角色工作**：角色智能体通读 `packs/<工作流>/<角色>/SKILL.md` 与本宪法并严格遵循（上下文来源见第一章，访问边界与临时文件纪律见第三章）。
6. **打包出箱**：`bin/swarm complete`——交接前角色应已按第三章完成经验沉淀（写入 `packs/_common/`）；打包前自动清理 `project/` 内编译等产生的临时文件（`target/`、`node_modules/`、`__pycache__/` 等无可争议的构建缓存；`build/`、`dist/` 等歧义名不在自动清理范围，由角色按 `packs/_common/engineering.md`「交付前清理构建缓存」负责），然后打包 `project/` 全部文件生成 `outbox/<时间戳>-<工作流>-<角色>.zip`，归档已消费的 inbox 输入（`inbox/.consumed/`，下次 `unpack` 收到新上游时随收件箱一并清空），清除会话级状态（plan/tmp）并清空 `project/` 备用；plan.md 缺失时给出警告（违反第二章）。
7. **会话终结**：出箱即本轮任务终点，不自动流转（第一章）；仅操作者显式要求续跑时手动链式：`bin/swarm unpack outbox/<最新zip>.zip` → `bin/swarm begin <下一角色>` → …（`unpack` 会先暂存源包再清空 `outbox/`，链式取包安全）。

### 各工作流的角色链（bin/swarm chains）

> 角色链仅描述各工作流（packs）的**可用编排顺序**，供操作者规划多角色会话；`complete` 后不自动流转（第一章）。各链的**第一个角色**负责依用户原始项目需求建档 `project/mission.md`（第一章）。

各工作流的编排详见 `packs/INDEX.md`

## 第五章 会话规则与生效

### 会话规则

- 同一时刻只有一个会话：`.work/session` 存在时 `begin` 会拒绝，请先 `complete` 或 `reset`。
- `bin/swarm complete` 要求 `project/` 中有实际产物；确要跳过校验加 `--force`，打包后想保留工作区加 `--keep`。
- 调用脚本/工具的通用陷阱（解释器契约、失败先归因、路径与工作目录确定性）统一遵循 `packs/_common/engineering.md`（「工具启动」与「验证」），不在此复述。

### DRY 约束与生效

- 未尽事宜按 `packs/_common/engineering.md`（工程与工具规则）与 `packs/_common/notes/`（语言/工具/框架专项注意事项）执行。
- **DRY 约束**：本宪法（制度性内容）与 `packs/_common/engineering.md`（工程与工具规则）是各 `SKILL.md` 与 `_common/` 条款的唯一规则来源——计划制度、上下文来源、工作区纪律、交接说明、mission.md 规则、不自动流转，以及工具启动、验收流水线、验证与护栏等已在此两处说明的内容，各 `SKILL.md` 与 `_common/` 条款**不得重复**，只做引用与角色特有补充（如本角色使用哪些工具、以何种顺序、达到何种门槛）；发现重复时以来源原文为准，并应在会话之外裁剪重复。各 `SKILL.md` 的工作流程/交接段如与本宪法冲突，以本宪法原文为准。
