# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（FR-02-40② 退役键口径同步轮——QA 终验轮待批① 落实）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner/architect/QA 收敛链 ⇒ specifier 显示指标/调整/裁决/新增需求轮（v1.47–v1.53）⇒ specifier 裁决落实修订轮（v1.54）⇒ coder 产品实现/复核重验轮 ⇒ specifier PEERS/SEEDS 核证修订轮（v1.55）⇒ specifier D80 裁决落实轮（v1.56）⇒ coder FR-02-37 v1.3 产品实现轮 ⇒ cleaner 收敛轮 ⇒ architect 架构评审轮 ⇒ hardender 变异加固三轮 ⇒ QA FR-02-37 v1.3 终验轮（2026-11-16，工作流走完 Done）⇒ **specifier FR-02-40② 退役键口径同步轮（本轮 2026-11-16，独立小轮：待批① 两处同步 + 同口径全查）**
> 会话：six-pack-specifier-20261116（SESSION_TASK = 同步FR-02-40②退役键口径；上游交接 = QA 终验轮 handoff，其「待批①」为本轮任务依据，操作者指令文本即批准）
> Inversion 受限退化：chat 选择题面板不可用，操作者指令「同步FR-02-40②退役键口径」即待批① 批准与阶段 1 答案（Inversion 阶段 1 澄清对象 = 待批① 两处改判口径，QA 已按三方一致口径先行断言，无残余歧义未触发阶段 2；沿 v1.45–v1.54 八轮先例）。
> 环境：本沙箱为全新克隆——度量四件与 APS/bb 未重建（.tools 缺失）；本轮为纯规格面修订零产品改动，未跑 cargo/APS，规格层以结构校验替代（见验证证据）；rustup 镜像复测 rsproxy 吞吐 15.3MB/s（notes/environment.md）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.56**）+ `mission/` 四份 phase 详述（现行 phase-01 v1.53、phase-02 v1.24）。ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA/hardender 历轮（至 2026-11-16 hardender 三轮） | ezr 产品实现 + 收敛/评审/加固链 + 变异实跑 494 突变体全处置 | 已交付 | 17 目标 9170P/0F、clippy 0、fmt 合规、arch_check 18 规则过 |
| QA FR-02-37 v1.3 终验轮（2026-11-16） | 产品缺陷修复 2 处 + QA runner 同步修复 3 套件 + 全量独立验证 + notes/handoff | 已交付 | 基线 17 目标 9170P/0F + prop 88P + 15 套件 191P/0F 全绿 + CRAP/DRY 实跑登记 |
| **specifier FR-02-40② 退役键口径同步轮（本轮）** | 3 feature 判据面修订（01-PC v1.24 / 08-MB v1.52 / 09-BE v1.54）+ 3 份 QA 规程同步 + notes/INDEX + 本 handoff | 已交付 | 结构校验全过（见验证证据）；零产品改动，基线沿 QA 终验轮口径不变 |

### 遗留受限项（当前有效）
- **变异实跑总账（三轮，全收敛）**：全清单 494 突变体处置闭环、零未跑；登记函数 235 = 杀 149 + unviable 6 + 超时杀 1 + 存活 79；非登记函数 84 沿杀灭账免跑；trackpool ws 面 31 = 20 定理证明 + 11 免跑。逐突变体判定权威 = `adaptive_resolved.csv` 最新行（沙箱重置后按交接②再生）。
- **79 存活体 = 覆盖缺口确认（非等价），双套件零杀灭证据已补齐**：`results/bs_runs.csv` 23 批次 exit=0、恰覆盖 79 id；回补路径沿 hardender 交接④。**缺口 112 回补测试归属（QA 裁决）**：移交操作者裁决是否追加专项轮（待批③）。
- **软 Gherkin 验收变异执行层不可用（受限项，沿历轮登记不变）**：回补属专项基建轮（待批④）。**本轮补充**：全新沙箱 .tools/APS/bb 未重建，本轮规格层 parser/dry-check 未跑，以结构校验 + 行集对账替代（零产品改动面，风险=极低）。
- **`common.max_speed = xyz` 拒绝行互斥偏差（本轮沿 QA 终验轮 runner 注记升级登记，待批①新）**：max_speed 为 FR-01-60 v1.42 退役键（按未知键静默忽略），场景 04 Examples 该行与场景 12/单测 legacy_max_speed_key_retired_as_unknown 互斥——runner 已跳过该行并注记；本轮已将偏差补入 QA-PC-04 v1.24 注记，**改判/删除该行涉 FR-01-60 面、不在 FR-02-40② 批准范围，待操作者裁决**。
- **QA-PC 规程/runner 组成差异（本轮新发现登记，待批②新）**：qa/01-persistence-config-qa.md 有 QA-PC-12 规程行而 runner 无 _case_12；QA-PC-10/11（场景 10 EZR_HOME 重定位 / 场景 11 kill -9）规程行与 runner 用例双缺——是否补规程与可执行化待操作者裁决（属 QA 可执行化职责面，本轮不越权补）。
- **QA runner 族缺口（沿历轮登记不变）**：06-cmd-window、07-add-bt、09-bt-engine、10-bt-anti-leech、11-trackers-window 五规程暂无可执行套件；现行 15 套件沿 QA 终验轮全绿。
- **ezr-demo 侧验证脚本（工作区外，移交建议沿 QA 登记）**：verify_trackers_popup.py 3F + verify_size_move.py 陈旧化——demo 专项轮归操作者指派。
- **CRAP 登记（沿 QA 终验轮口径）**：行覆盖 94.4%、ACTIONABLE_COUNT=99，注册热点逐一对账在位。
- **DRY 登记（沿 QA 终验轮口径）**：阈值 200 结构性克隆 6 组（architect 裁决「维持登记、lib 化不实施」沿袭）；阈值 70 全量 198 组 = 产品侧 23 / 测试侧 175。
- **QA-PC-06 环境性 flake（沿 QA 登记）**：负载敏感时序 flake 非回归，复现按三证归因。
- **mktorrent bencode 跨 bin 重复**：维持登记、lib 化不实施。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退），D80①–④ 已全部结案**——本轮为规格判据面同步，无新增 D 值（D73③ 改判口径沿 FR-02-40④/D79⑦ 既有登记）。

## 二、当前产出情况

### 本会话产物清单（specifier FR-02-40② 同步轮）
1. **features/01-persistence-config.feature v1.24（QA 待批①-①）**：场景 04 Examples 拒绝行 `bt.seed_duration_secs | 0` 改判 `bt.default_seed_duration_m | 0`（新键标量非法，FR-02-35 ③；退役键不入拒绝集）；场景 12 增退役键 seed_duration_secs 静默忽略正探针条目（启动成功，与 runner QA-PC-04 v1.24 正探针对账）；头部 v1.24 注记 + 场景 04/12 清单注记；卫生面：清单补登漏登的场景 14 行。
2. **features/08-modify-bt.feature v1.52（同口径全查）**：场景 10 Given/回退步骤「做种倒计时可见/停止显示」改缺席判据（「剩余做种」展示已整项退役，FR-02-38 v1.21）；「做种剩余时长重置为 config seed_duration_secs 当前值（D73③）」改判——机制面不可经 UI 观察（引擎级单测承载）、时长臂来源改 default_seed_duration_m / [bt.seed_exit_rule]、exit rule 生效臂 = 做种时长累计清零（FR-02-40④/D79⑦）；头部 v1.52 注记 + 清单注记。
3. **features/09-bt-engine.feature v1.54（同口径全查）**：场景 08 回做种步骤 seed_left 重置同口径改判（与 08-MB v1.52 同款）；头部 v1.54 注记 + 清单注记。
4. **qa/08-modify-bt-qa.md（QA 待批①-②）**：QA-MB-10 判据两段同步（缺席判据 + D73③ 改判，runner assert_not_in/机制面单测承载口径对账）+ 对应规格行 v1.52 枚举 + 头部 v1.52 同步注。
5. **qa/09-bt-engine-qa.md（同口径全查）**：QA-BE-08 判据同步（同款改判）+ 对应规格行 v1.54 枚举 + 头部 v1.54 同步注。
6. **qa/01-persistence-config-qa.md**：QA-PC-04 判据 v1.24 注（改判 + 正探针 + max_speed 偏差沿 QA 登记）+ QA-PC-12 退役键判据补 seed_duration_secs + 对应规格行场景计数 13→14 校正 + 头部 v1.24 同步注。
7. **经验沉淀**：project/notes/testing.md（退役键同步欠账同形全查法）、environment.md（rustup 镜像复测）各追加条目，INDEX.md 同步。
8. **本 handoff.md** 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令/证据（可复现） |
|---|---|---|
| 退役键活跃判据残留 | 零残留（余项均为退役语义/历史注记/正探针描述） | `grep -n seed_duration_secs project/features/*.feature project/qa/*.md` + 排除词清单核对 |
| Examples ↔ runner 行集对账 | feature 场景 04 拒绝行 33 行 vs runner rows 32 行；唯一分歧 = common.max_speed（QA runner 注记跳过，已登记待裁决）；`bt.default_seed_duration_m` 在列、`seed_duration_secs` 不在列 | python 正则解析 Examples 表与 `rows = (...)` 元组逐项比对 |
| Gherkin 结构完整性 | 三文件场景数 14/13/24 不变（判据面修订，稳定序号全部不变）、步骤关键词/顶格行零异常 | grep Scenario 计数 + 逐行关键词校验 |
| 产品基线 | 零产品改动，沿 QA 终验轮口径不变（17 目标 9170P/0F、15 套件 191P/0F） | 本轮未触碰 src/、tests/、runners/；复验命令沿 QA 终验轮 handoff 验证证据节 |
| APS parser/dry-check | 未跑（受限：.tools/bb 本沙箱未重建）；结构校验替代，登记受限项 | — |

### 对账口径
- 本轮改动面 = 3 feature + 3 份 QA 规程 + notes 两文件 + INDEX.md + 本 handoff；**src/、tests/、qa/runners/ 零改动**（runner 先行同步沿 QA 终验轮）。
- 同步覆盖对账：QA 待批① 登记 2 处（feature 场景 04 行 + QA-MB-10）全部落实；同口径全查新增 3 处（08-MB 场景 10、09-BE 场景 08、QA-BE-08）落实；历轮卫生面 2 处（01-PC 清单漏场景 14、QA 文档场景计数 13→14）顺手修正并披露。

### 待办与待批
- **待批①（本轮升级登记）**：场景 04 `common.max_speed = xyz` 拒绝行改判/删除（max_speed = FR-01-60 v1.42 退役键；runner 已跳过该行）。批准后可开 specifier 小轮同步。
- **待批②（本轮新发现登记）**：QA-PC-10/11 规程行与 runner 用例双缺、QA-PC-12 规程行无 runner 对应——是否补规程/可执行化（QA 面小轮）。
- **待批③（沿 QA 登记）**：缺口 112 回补测试专项轮。
- **待批④（沿 QA 登记）**：软 Gherkin 执行层回补基建专项轮。
- 待办：ezr-demo 侧 verify 脚本 3F/陈旧化（demo 专项轮，工作区外）。

### 移交建议
- 本轮为独立规格同步小轮，已收口：**无新增行为规格、无实现面待办，无需流转 coder**；six-pack 工作流保持 QA 终验轮后的已完结态。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 如需续做：待批①～④ 均为独立小轮，由操作者显式决定；无流水线自动流转。
