# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（peer_connection_protocol 裁决复验轮——认可保持 ["tcp"] 的 QA 回归确认）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner/architect/QA 收敛链 ⇒ specifier 显示指标/调整/裁决/新增需求轮（v1.47–v1.53）⇒ specifier 裁决落实修订轮（v1.54）⇒ coder 产品实现/复核重验轮 ⇒ specifier PEERS/SEEDS 核证修订轮（v1.55）⇒ specifier D80 裁决落实轮（v1.56）⇒ coder FR-02-37 v1.3 产品实现轮 ⇒ cleaner 收敛轮 ⇒ architect 架构评审轮 ⇒ hardender 变异加固三轮 ⇒ QA FR-02-37 v1.3 终验轮（2026-11-16，工作流走完 Done）⇒ specifier 退役键口径/拒绝行/组成算术三轮 ⇒ QA _case_13 键集补齐与注记结案回填轮 ⇒ specifier QA-PC-10/11 规程行补齐轮 ⇒ QA 01-PC 套件可执行化扩面轮（20261116b）⇒ specifier peer_connection_protocol 裁决落实轮（20261116c，v1.28）⇒ **QA peer_connection_protocol 裁决复验轮（本轮 20261116d）**
> 会话：six-pack-QA-20261116d（SESSION_TASK = peer-conn-proto-adjudication-qa-verify；上游交接 = specifier 裁决落实轮 handoff + 操作者 chat 裁决「认可保持 ["tcp"]」）
> 环境：nightly 1.102.0-nightly（5f4effd57，TUNA dist）+ ezr 四 bin debug 构建（cargo build 2m29s，rsproxy.cn crate 源）+ suite 依赖（pyte 0.8.2/wcwidth 0.6.0/libtorrent 2.1.1，aliyun PyPI wheel）；零产品源码改动面。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.56**）+ `mission/` 四份 phase 详述（现行 phase-01 v1.53、phase-02 v1.24）。ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA/hardender 历轮（至 2026-11-16 hardender 三轮） | ezr 产品实现 + 收敛/评审/加固链 + 变异实跑 494 突变体全处置 | 已交付 | 17 目标 9170P/0F、clippy 0、fmt 合规、arch_check 18 规则过 |
| QA FR-02-37 v1.3 终验轮（2026-11-16） | 产品缺陷修复 2 处 + QA runner 同步修复 3 套件 + 全量独立验证 + notes/handoff | 已交付 | 基线 17 目标 9170P/0F + prop 88P + 15 套件 191P/0F 全绿 + CRAP/DRY 实跑登记 |
| specifier 退役键口径/拒绝行/组成算术三轮（2026-11-16） | 01-PC 规程 v1.24→v1.26 + feature v1.24→v1.26 + notes/INDEX + handoff | 已交付 | 结构校验全过；零产品改动 |
| QA _case_13 键集补齐与注记结案回填轮（2026-11-16） | suite_persistence_config.py v1.26 同步 + 01-PC 规程/feature 组成算术按模板权威拨正 | 已交付 | 套件全量重跑 P=11 F=0 S=0（83s）；零产品源码改动 |
| specifier QA-PC-10/11 规程行补齐轮（2026-11-16） | 01-PC QA 规程 v1.27（用例表补 QA-PC-10/11 两规程行，14 用例 ↔ 14 场景全对齐）+ notes/INDEX + handoff | 已交付 | 结构校验全过；零产品改动 |
| QA 01-PC 套件可执行化扩面轮（2026-11-16） | suite_persistence_config.py v1.27（_case_10/11/12 新增 + _case_13 补 BT 下载流程，14 用例全量注册）+ notes 三文件 + handoff | 已交付 | 套件全量重跑 P=14 F=0 S=0（105.6s）；零产品源码改动；peer_connection_protocol 改判披露待操作者追认 |
| specifier peer_connection_protocol 裁决落实轮（2026-11-16） | qa/01-persistence-config-qa.md v1.28（头部裁决注记 + QA-PC-13 判据列取值钉 ["tcp"]）+ handoff | 已交付 | 结构校验过；零产品/feature/runner 改动；观察项收口 |
| **QA peer_connection_protocol 裁决复验轮（本轮）** | suite_persistence_config.py 全量复跑 + notes/environment.md 镜像经验补录 + 本 handoff | 已交付 | **套件全量复跑 P=14 F=0 S=0（106.1s）**；零产品源码改动；裁决认可保持 ["tcp"] 回归确认 |

### 遗留受限项（当前有效）
- **peer_connection_protocol 改判（已结案）**：QA v1.27 在 runner _case_13 改 ["webrtc"] → ["tcp"]，经 specifier v1.28 spec 面钉值落录 + 操作者裁决「认可保持 ["tcp"]」+ 本轮 QA 全量复跑 P=14 F=0 S=0 回归确认，全链收口。
- **变异实跑总账（三轮，全收敛）**：全清单 494 突变体处置闭环、零未跑；登记函数 235 = 杀 149 + unviable 6 + 超时杀 1 + 存活 79；非登记函数 84 沿杀灭账免跑；trackpool ws 面 31 = 20 定理证明 + 11 免跑。
- **79 存活体 = 覆盖缺口确认（非等价）**：缺口 112 回补测试归属（QA 裁决）移交操作者裁决（待批③）。
- **软 Gherkin 验收变异执行层不可用（受限项）**：回补属专项基建轮（待批④）。
- **QA runner 族缺口**：06-cmd-window、07-add-bt、09-bt-engine、10-bt-anti-leech、11-trackers-window 五规程暂无可执行套件。
- **ezr-demo 侧验证脚本（工作区外）**：verify_trackers_popup.py 3F + verify_size_move.py 陈旧化——demo 专项轮归操作者指派。
- **CRAP/DRY 登记（沿 QA 终验轮口径不变）**：本轮零产品源码改动面，登记值不变。
- **QA-PC-06 环境性 flake（沿 QA 登记）**：负载敏感时序 flake 非回归。
- **mktorrent bencode 跨 bin 重复**：维持登记、lib 化不实施。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退），D80①–④ 已全部结案**——本轮为裁决复验，无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（QA peer_connection_protocol 裁决复验轮）
1. **suite_persistence_config.py 全量复跑**（裁决落录的回归确认，零 runner 改动）：复跑 PC-01..14 全量 → P=14 F=0 S=0（累计 106.1s）；其中 _case_13（peer_connection_protocol = ["tcp"]，含 BT 下载流程）P（13.2s）。spec v1.28 钉值下套件全绿，裁决认可保持 ["tcp"] 经实证无回归。
2. **project/notes/environment.md 追加**（经验沉淀）：cargo crate 文件来源镜像选型（rsproxy.cn index+crate 同源 vs TUNA 只镜像 index 指回慢 CDN）+ pip libtorrent wheel 来源（TUNA 403/官方超时 → aliyun）+ 全新沙箱复建 01-PC 套件复跑前置四件实测。
3. **本 handoff.md** 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令/证据（可复现） |
|---|---|---|
| spec ↔ runner ↔ feature 三方对齐 | 一致：QA 规程 v1.28 钉 peer_connection_protocol = ["tcp"] ↔ runner _case_13 实写 `peer_connection_protocol = ["tcp"]`（suite_persistence_config.py L764）↔ feature 场景 13 通用「合法非默认值」口径 | 逐文件核对 |
| **套件全量复跑** | **P=14 F=0 S=0（累计 106.1s）**——PC-01..14 全绿，_case_13（["tcp"] 臂，含 BT 下载）P（13.2s） | `cd project/qa/runners && python3 suite_persistence_config.py` |
| 单案试跑（环境自检） | PC-06 P（8.8s） | `QA_ONLY=QA-PC-06 python3 suite_persistence_config.py` |
| 产品基线 | 零产品源码改动（src/、tests/、feature、mission、runner 均未触碰）；仅 notes/environment.md + 本 handoff | — |

### 对账口径
- 本轮改动面 = notes/environment.md（经验追加）+ 本 handoff；**规程、feature、mission、src/、tests/、runner 零改动**。
- **裁决认可保持 ["tcp"] 回归确认**：specifier v1.28 spec 面钉值 + 操作者裁决 + 本轮 QA 全量复跑 P=14 F=0 S=0，三证闭环；零 runner 改动故结果同 v1.27（裁决落录不改变可执行行为，复跑为回归确认）。
- **需求口径不变**：mission v1.56 FR-02-34 peer_connection_protocol 默认 ["tcp","webrtc"]、无跨协议回退；非默认单值臂 ["tcp"] 属合法取值子集。

### 待办与待批
- **待批③（沿 QA 登记）**：缺口 112 回补测试专项轮。
- **待批④（沿 QA 登记）**：软 Gherkin 执行层回补基建专项轮。
- **QA runner 族缺口（沿历轮登记）**：06-cmd-window、07-add-bt、09-bt-engine、10-bt-anti-leech、11-trackers-window 五规程暂无可执行套件。
- **待办（产品面，沿前轮登记不变）**：ezr/README.md 配置示例仍写 `max_speed = "2 MB/s"`——移交 coder/cleaner 面修订。
- 待办：ezr-demo 侧 verify 脚本 3F/陈旧化（demo 专项轮，工作区外）。
- ~~peer_connection_protocol 改判可回退~~ → **已全链收口**：specifier v1.28 钉值 + 操作者裁决认可 + QA 复跑实证。

### 移交建议
- **six-pack 流转路径自 QA 终验轮起保持完结态**；本轮为 post-Done 独立维护小轮（specifier 裁决落实 + QA 复验），无自动流转。
- 裁决已全链收口：spec v1.28 钉值 ↔ runner _case_13 实写 ↔ feature 通用口径，三方一致；套件全量 P=14 F=0 S=0 实证无回归。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 如需续做：上方待批③④与五规程可执行化均为独立小轮，由操作者显式决定。
