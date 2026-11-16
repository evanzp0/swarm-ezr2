# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（demo 结算轮——PTY Alt+Enter 受限项定性收口）
> 路径：…coder demo 三轮（20261116g，L623 UI 变量根因修复）⇒ **coder demo 结算轮（本轮 20261116h，操作者实测确认 Alt+Enter 真终端可用 → PTY 受限项移出账面）**
> 会话：six-pack-coder-20261116h（SESSION_TASK = settle-alt-enter-restriction；上游交接 = 上轮 handoff + 操作者 chat 实测反馈「Alt+Enter 换行键可用，这一条可以结算掉了」）
> 环境：无构建（纯文档定性更新）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.56**）。ezr 不做旧版本兼容。
- ezr-demo：仓库内独立子项目（`ezr-demo/`，工作区外），Rust + ratatui 终端 TUI 演示应用，41 轮变更记录；自带 PTY 级 verify 脚本。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| coder demo 三轮（2026-11-16g） | UI 层 L623 trackers_focus 变量根因修复（五点对齐）+ 鼠标点击输入探针双对话框实证 | 已交付 | verify 两连绿 + 鼠标点击 trackers 输入 ModifyBt/AddBt 双对话框实证通过 |
| **coder demo 结算轮（本轮）** | PTY Alt+Enter 受限项定性收口（操作者真终端实测可用 → 移出遗留受限项）+ notes 补记 + 本 handoff | 已交付 | 无代码改动；定性更新 |

### 遗留受限项（当前有效）
- **~~PTY 下 Alt+Enter 换行键不可达~~（本轮结算移出）**：操作者真终端实测 ezr-demo 添加/修改 BT 对话框 Alt+Enter 换行可用——证实「PTY 下不被 crossterm 解析」是 PTY 测试环境限制（escape 解析时序与真终端不同），非 demo/产品缺陷。verify T3 保持 Tab×6 键盘导航 + 清空 + 单行 wss 测法（PTY 环境限制仍在），但不再作为待修受限项登记。
- **peer_connection_protocol 改判（已结案）**、**变异实跑总账（79 存活体=覆盖缺口，待批③）**、**软 Gherkin 执行层（待批④）**、**QA runner 族五规程缺口**、**CRAP/DRY 登记（零产品改动）**、**mktorrent bencode 跨 bin 重复**——均沿历轮登记不变。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退），D80①–④ 已全部结案**——本轮无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（coder demo 结算轮）
1. **project/notes/testing.md** 追加：PTY Alt+Enter 不可达定性收口补记（操作者真终端实测确认可用 → PTY 环境限制定性收口；区分「测试环境限制」与「产品缺陷」判据 = PTY 不可达 + 真终端可用 = 环境限制，PTY 不可达 + 真终端也不可达 = 产品缺陷）。
2. **project/notes/INDEX.md** 同步。
3. **本 handoff.md** 重写为本轮口径。

> 注：本轮无代码改动。verify_trackers_popup.py T3 测法（Tab×6 键盘导航 + 清空 + 单行 wss）保持不变——PTY 环境下 Alt+Enter 仍不可达，T3 用键盘导航测法是稳定选择；真终端下 Alt+Enter 可用已由操作者实证，产品/demo 无需改。

### 验证证据
| 验证项 | 结果 | 命令/证据（可复现） |
|---|---|---|
| Alt+Enter 真终端可用 | 操作者实测通过（ezr-demo 添加/修改 BT 对话框 Alt+Enter 换行正常）| 操作者真终端手测 |
| verify 脚本基线 | 沿上轮两连绿口径不变（本轮无代码改动）| `cd ezr-demo && python3 scripts/verify_trackers_popup.py && python3 scripts/verify_size_move.py` |
| ezr 产品基线 | 零产品源码改动；零 demo 源码改动（仅 notes + handoff）| — |

### 对账口径
- 本轮改动面 = project/notes/{testing,INDEX}.md + 本 handoff；**ezr 产品与 ezr-demo 源码零改动**。
- 定性收口：PTY Alt+Enter 不可达从「遗留受限项」移出，归类为「测试环境限制」（verify 变通测法绕开，产品不动），不构成待修项。

### 待办与待批
- **待批③（沿 QA 登记）**：缺口 112 回补测试专项轮。
- **待批④（沿 QA 登记）**：软 Gherkin 执行层回补基建专项轮。
- **QA runner 族缺口（沿历轮登记）**：五规程暂无可执行套件。

### 移交建议
- **six-pack 流转路径自 QA 终验轮起保持完结态**；本轮为 post-Done 独立维护小轮（受限项定性收口），无自动流转。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 如需续做：待批③④、五规程可执行化均为独立小轮，由操作者显式决定。
