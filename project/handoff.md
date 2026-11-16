# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（max_speed 退役键拒绝行同步轮——QA 终验轮登记待批① 裁决落实）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner/architect/QA 收敛链 ⇒ specifier 显示指标/调整/裁决/新增需求轮（v1.47–v1.53）⇒ specifier 裁决落实修订轮（v1.54）⇒ coder 产品实现/复核重验轮 ⇒ specifier PEERS/SEEDS 核证修订轮（v1.55）⇒ specifier D80 裁决落实轮（v1.56）⇒ coder FR-02-37 v1.3 产品实现轮 ⇒ cleaner 收敛轮 ⇒ architect 架构评审轮 ⇒ hardender 变异加固三轮 ⇒ QA FR-02-37 v1.3 终验轮（2026-11-16，工作流走完 Done）⇒ specifier FR-02-40② 退役键口径同步轮（2026-11-16）⇒ **specifier max_speed 退役键拒绝行同步轮（本轮 20261116，独立小轮：待批① 结案 + 同形全查）**
> 会话：six-pack-specifier-20261116（SESSION_TASK = 同步max_speed退役键拒绝行；上游交接 = FR-02-40② 同步轮 handoff，其待批① 为本轮任务依据，操作者指令「批准①」即批准）
> Inversion 受限退化：操作者指令「批准①」即待批① 批准；处置口径无残余歧义未触发阶段 2——QA 终验轮已按三方一致口径裁定 runner 行集锚（沿 v1.45–v1.54 八轮与 FR-02-40② 轮先例）。
> 环境：纯规格面修订零产品改动，未跑 cargo/APS（.tools 未重建，结构校验替代）；rustup 已在位（~/.cargo/bin），本轮无 cargo 需求。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.56**）+ `mission/` 四份 phase 详述（现行 phase-01 v1.53、phase-02 v1.24）。ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA/hardender 历轮（至 2026-11-16 hardender 三轮） | ezr 产品实现 + 收敛/评审/加固链 + 变异实跑 494 突变体全处置 | 已交付 | 17 目标 9170P/0F、clippy 0、fmt 合规、arch_check 18 规则过 |
| QA FR-02-37 v1.3 终验轮（2026-11-16） | 产品缺陷修复 2 处 + QA runner 同步修复 3 套件 + 全量独立验证 + notes/handoff | 已交付 | 基线 17 目标 9170P/0F + prop 88P + 15 套件 191P/0F 全绿 + CRAP/DRY 实跑登记 |
| specifier FR-02-40② 退役键口径同步轮（2026-11-16） | 3 feature 判据面修订（01-PC v1.24 / 08-MB v1.52 / 09-BE v1.54）+ 3 份 QA 规程同步 + notes/INDEX + handoff | 已交付 | 结构校验全过；零产品改动，基线沿 QA 终验轮口径不变 |
| **specifier max_speed 退役键拒绝行同步轮（本轮）** | 01-PC feature v1.25（场景 04 拒绝行删除 + 场景 03/12 标签清理）+ 01-PC QA 规程 v1.25（QA-PC-04 结案态 + QA-PC-13 改标）+ notes/INDEX + 本 handoff | 已交付 | 结构校验全过（见验证证据）；零产品改动，基线沿 QA 终验轮口径不变 |

### 遗留受限项（当前有效）
- **变异实跑总账（三轮，全收敛）**：全清单 494 突变体处置闭环、零未跑；登记函数 235 = 杀 149 + unviable 6 + 超时杀 1 + 存活 79；非登记函数 84 沿杀灭账免跑；trackpool ws 面 31 = 20 定理证明 + 11 免跑。逐突变体判定权威 = `adaptive_resolved.csv` 最新行（沙箱重置后按交接②再生）。
- **79 存活体 = 覆盖缺口确认（非等价），双套件零杀灭证据已补齐**：`results/bs_runs.csv` 23 批次 exit=0、恰覆盖 79 id；回补路径沿 hardender 交接④。**缺口 112 回补测试归属（QA 裁决）**：移交操作者裁决是否追加专项轮（待批③）。
- **软 Gherkin 验收变异执行层不可用（受限项，沿历轮登记不变）**：回补属专项基建轮（待批④）。本轮延续：.tools/APS/bb 未重建，规格层 parser/dry-check 未跑，以结构校验 + 行集对账替代（零产品改动面，风险=极低）。
- **QA-PC 规程/runner 组成差异（沿 FR-02-40② 轮登记不变，待批②）**：qa/01-persistence-config-qa.md 有 QA-PC-12 规程行而 runner 无 _case_12；QA-PC-10/11（场景 10 EZR_HOME 重定位 / 场景 11 kill -9）规程行与 runner 用例双缺——是否补规程与可执行化待操作者裁决（属 QA 可执行化职责面）。
- **QA runner 族缺口（沿历轮登记不变）**：06-cmd-window、07-add-bt、09-bt-engine、10-bt-anti-leech、11-trackers-window 五规程暂无可执行套件；现行 15 套件沿 QA 终验轮全绿。
- **ezr-demo 侧验证脚本（工作区外，移交建议沿 QA 登记）**：verify_trackers_popup.py 3F + verify_size_move.py 陈旧化——demo 专项轮归操作者指派。
- **CRAP 登记（沿 QA 终验轮口径）**：行覆盖 94.4%、ACTIONABLE_COUNT=99，注册热点逐一对账在位。
- **DRY 登记（沿 QA 终验轮口径）**：阈值 200 结构性克隆 6 组（architect 裁决「维持登记、lib 化不实施」沿袭）；阈值 70 全量 198 组 = 产品侧 23 / 测试侧 175。
- **QA-PC-06 环境性 flake（沿 QA 登记）**：负载敏感时序 flake 非回归，复现按三证归因。
- **mktorrent bencode 跨 bin 重复**：维持登记、lib 化不实施。
- **既存 [/] 改写字节状态（沿 bug16/bug18 登记）**：规格面文件存在历史会话遗留的 ttp] 形态字节（01-PC QA 规程 13 处、feature 6 处、runner 2 处）与完整字节并存；本轮编辑全程避开敏感序列并经字节计数终审零附带损伤，既存状态未改动、是否统一修复属独立卫生轮。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退），D80①–④ 已全部结案**——本轮为规格判据面结案，无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（specifier max_speed 退役键拒绝行同步轮）
1. **features/01-persistence-config.feature v1.25（QA 待批① 落实）**：场景 04 Examples 拒绝行 `common.max_speed = xyz` 删除（33→32 行；max_speed = FR-01-60 v1.42 退役键，按未知键静默忽略——场景 12 同口径、单测 legacy_max_speed_key_retired_as_unknown 承载，退役键不入拒绝集）；**替代键 global_max_download_speed 拒绝行不新增**（拒绝集以 runner QA-PC-04 行集 32 行为锚——QA 终验轮已按三方一致口径裁定，改判 = 新增覆盖超出待批① 范围）；头部 v1.25 修订注 + 场景 04 v1.25 注记 + 清单行 v1.25 撤销痕。**同形全查另改判 2 处**（沿退役键同步轮 testing.md:808 先例）：场景 03/12 默认行为描述「max_speed 不限」→「不限速」（与 QA-PC-03/12 对应表述同形；行为语义零变更，仅去退役键名标签），清单行 03/12 撤销痕。
2. **qa/01-persistence-config-qa.md v1.25**：头部对应规格行改 v1.25 口径 + 新增 v1.25 结案注；QA-PC-04 操作列去 `[common] max_speed=xyz` 枚举、判据列偏差登记改**结案态**（登记类注记裁决后回填结案态，沿锚点对齐轮裁决轮先例）；**QA-PC-13 判据限速标签改标**「max_speed = 速度封顶」→「global_max_download_speed = 速度封顶」（FR-02-28 权威键名；与 runner _case_13 实写 `global_max_download_speed = "2 MiB/s"` 及断言对账；行为语义零变更）。
3. **runner（qa/runners/）零改动**：行集本即终态锚（32 行，QA 终验轮已同步）；其 QA-PC-04「已知偏差（待操作者裁决）」注记与 _case_13 docstring「待 specifier 同步」表述已转过时，**结案回填沿 20261112 锚点对齐轮先例归 QA 面**，登记待办（见下）。
4. **经验沉淀**：project/notes/testing.md 新增「退役键拒绝行结案轮」条目（处置选型锚 = runner 行集、登记注记结案回填面归属、键名标签/退役语义分层改判、改写敏感序列避开与字节零损伤终审），INDEX.md 同步。
5. **本 handoff.md** 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令/证据（可复现） |
|---|---|---|
| feature Examples ↔ runner rows 行集对账 | **32 = 32，键集全等零分歧**（三处表值形态差 = Gherkin 表格值无引号/带中文注记 vs Python 元组带引号，既有形态非本轮引入） | `python3 .work/tmp/reconcile.py`（正则解析场景 04 Examples 与 runner `rows = (...)` 逐项比对） |
| max_speed 同形全查复扫（规格面） | 活跃判据/标签零残留；余 3 行均合法——01-config-template v1.11 历史注记段 1 行、场景 12 旧格式正探针输入 `max_speed = "1 MB/s"` 1 行、01-PC QA 规程 v1.17 历史待办注记 1 行 | `rg max_speed project/features/*.feature project/qa/*.md` + 排除词清单 + 逐行判读 |
| Gherkin 结构完整性 | 三文件场景数 14/13/24 不变（判据面修订，稳定序号全部不变） | python `Scenario:` 计数 |
| spec 面字节终审 | feature `ttp]`=6/`[http]`=6、QA 规程 `ttp]`=13/`[http]`=8——**编辑前后完全一致**，零附带损伤；新增文本全程避开 [/] 改写敏感序列 | python bytes 计数前后比对 |
| 产品基线 | 零产品改动，沿 QA 终验轮口径不变（17 目标 9170P/0F、15 套件 191P/0F） | 本轮未触碰 src/、tests/、qa/runners/；复验命令沿 QA 终验轮 handoff 验证证据节 |
| APS parser/dry-check | 未跑（受限：.tools/bb 本沙箱未重建）；结构校验替代，受限项登记 | — |

### 对账口径
- 本轮改动面 = 1 feature + 1 QA 规程 + notes 两文件 + INDEX.md + 本 handoff；**src/、tests/、qa/runners/ 零改动**。
- **处置选型披露**：待批① 批准文本含「改判/删除」两案，本轮采**删除**——依据 = QA 终验轮已同步 runner 行集（32 行，不含退役键亦不含替代键拒绝行）为既裁锚点；改判（把该行改写为替代键拒绝行）= 新增覆盖，超出待批① 批准范围。如操作者希望补 `global_max_download_speed` 非法值拒绝覆盖，需另批小轮（当前该覆盖不存在的状态 = QA 终验轮既裁口径）。
- 同步覆盖对账：待批① 登记 1 处（场景 04 行）落实；同形全查新增 3 处落实（场景 03/12 标签、QA-PC-13 标签）；越面发现 2 项登记不越权（README 产品面、runner 注记 QA 面，见待办）。

### 待办与待批
- **待批②（沿 FR-02-40② 轮登记不变）**：QA-PC-10/11 规程行与 runner 用例双缺、QA-PC-12 规程行无 runner 对应——是否补规程/可执行化（QA 面小轮）。
- **待批③（沿 QA 登记）**：缺口 112 回补测试专项轮。
- **待批④（沿 QA 登记）**：软 Gherkin 执行层回补基建专项轮。
- **待办（QA 面，新）**：runner QA-PC-04「已知偏差（待操作者裁决）」注记与 _case_13 docstring「待 specifier 同步」表述的结案回填（本轮已裁决并落实，注记转过时；纯注记无行为面，沿 20261112 锚点对齐轮先例归 runner 面）。
- **待办（产品面，新）**：ezr/README.md 配置示例仍写 `max_speed = "2 MB/s"`——退役键名 + 十进制旧写法对新键已非法（FR-02-28 v1.11 二进制口径）双料过时；移交 coder/cleaner 面修订。
- **观察（QA 规程面，新）**：QA-PC-13 操作列组成算术（40 键 = [common] 3 键 + proxy 双条目 + HTTP 表 11 键 + [bt] 20 键）沿 v1.18 时代口径，未随 v1.20 键集改组（+4 限速键 −max_speed +listen_port）更新；runner _case_13 实写键集为现行权威——是否重述组成待操作者/QA 轮裁决（本轮不越权）。
- 待办：ezr-demo 侧 verify 脚本 3F/陈旧化（demo 专项轮，工作区外）。

### 移交建议
- 本轮为待批① 结案独立小轮，已收口：规格面 max_speed 退役键活跃残留清零、feature↔runner 行集对账 32=32 归零，**无新增行为规格、无实现面待办，无需流转 coder**；six-pack 工作流保持 QA 终验轮后的已完结态。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 如需续做：待批②～④ 与上述待办均为独立小轮，由操作者显式决定；无流水线自动流转。
