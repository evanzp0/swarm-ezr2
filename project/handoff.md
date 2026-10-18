# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier　会话：ezr-demo-ui-refactor-spec-sync
> 上游交接：coder v131-fix（交接内容 = 前轮产物总账）+ 操作者 chat 指令。
> 当轮任务：ezr-demo 的 UI 与交互重构后，按权威文档《ezr-demo UI界面及交互设计规范.md》
> 与 ezr-demo 源码修订相关规格文档（mission / phase-01 / features / qa）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.30，分期模式）+ `project/mission/phase-01.md`（v1.30）。
- 本轮任务 = 操作者指令「ezr-demo UI 和交互重构，按《ezr-demo UI界面及交互设计规范.md》
  与 ezr-demo 代码修改相关文档」（非新规格切片；specifier 为首角色，操作者 chat 指令即
  mission 修订授权）。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.30 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | 逐轮人工规范化核对（APS 不可用轮沿袭，见受限项） |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F；CPD 17→15 |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 hardening_g_v147 | 已交付 | 3363P/0F、clippy 0、fmt 0、arch 15 |
| QA（ezr-qa-final） | QA 规程可执行化 13 端到端套件（qa/runners/）+ 缺陷修复 + 双度量复测 | 已交付 | e2e 165P/0F/0S（13 套件）、3363P/0F、覆盖 94.37%、CRAP≥30=0、CPD 89/15 |
| specifier（v129-spec） | mission v1.29（FR-01-106 + FR-01-96 修订 + D35）+ features/05-ui-pieces.feature + features/04-ui-conns 场景 08 + qa 同步 | 已交付 | APS 不可用（受限项），人工规范化核对通过 |
| coder（v129-impl / v130-fix / v131-fix） | FR-01-106 实现 + 两轮缺陷修复（多并发乱序白屏、失败/暂停部分块白屏） | 已交付 | bin 485P/0F + 集成 3727P/0F、clippy 0、fmt 0（v131-fix 口径） |
| **specifier（v130-spec，本轮）** | **mission v1.30（UI 对齐 ezr-demo 重构版 + FR-01-107/108/109 新增 + FR-01-80/96/97/98/103/106/25 修订 + D36/D37）+ features 5 份修订（TD 18–21 新增、UP-08 退役）+ qa 5 份同步（QA-TD-18..21/QA-01-19 新增、QA-UP-08 退役）+ 顺手校准 v1.14/v1.20 两处漏同步** | 已交付 | 反向自检 grep 通过（退役标识零漏改）；APS 不可用沿袭人工规范化核对 |

### 遗留受限项（当前有效）
- **APS 工具（gherkin-parser / gherkin-ir-dry-checker / gherkin-mutator）不可用**（沿袭开放）：
  环境为 fresh clone（.tools/ 空），Babashka 静态二进制经 GitHub release 下载停滞（网络受限）。
  影响面：本轮修订的 5 份 feature 未过 parser/dry-checker 机检（人工规范化核对替代）；
  累计欠账：05-ui-pieces、04-ui-conns（场景 08）+ 本轮全部修订面。
- **Rust 工具链沙箱不持久（沿袭登记）**：本轮未触碰构建，无影响；新会话先按
  `packs/_common/notes/rust.md` §1 探针归因（本体丢失才重装）。
- **观察受限突变体（沿袭 hardender 登记，仍开放）**：workers.rs:27:5/51:25/57:14/168:25、
  download.rs:277:52 共 5 条 + 等价 download.rs:85:27——run 级回补基建已铺。
- **ezr-proxy.rs 测试基建（沿袭裁决）**：127 变异点不拆、CRAP/CPD 读数不追责。
- **磁盘/沙箱**：沿袭口径（throttle_proxy 慢速门控用例前台分批 `QA_TP_ONLY=1,3`）。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D35、v138/v140 口径（见归档 handoff 登记清单）。
- **本轮新增（v1.30，均 ezr-demo 重构版源码逐字核证，非臆造）**：
  - 页脚文案 = `H 帮助  ·  Q 退出`（键名青色加粗、说明暗灰）+ toast `◆ 消息`（黄色）；
    toast 3 秒自动消失（启动欢迎语 4 秒）、一次只显示最新一条。
  - 帮助窗口 70×24，七组标题逐字 =【全局】【页签】【任务列表】【任务详情】【并发连接】
    【弹窗（添加/修改/删除/pieces/帮助）】【pieces 分块弹窗】；末行提示
    ` ↑↓/滚轮 滚动本页 · Esc/H 关闭`；帮助窗口位置跨次记忆、pieces 关闭即清、对话框每次居中。
  - 滚动条字形 ▲│█▼；显示条件 = 总数 > 可视 且 面板高 ≥ 5 且 可绘行 ≥ 3；滑块长度下取整
    （设计规范文档称 ⌈⌉，代码为 floor——以代码为准）；滚轮 ±2（帮助 ±1）只移视口不动选中。
  - 对话框尺寸：添加 74×11、修改 74×9、删除 66×7、pieces 64×19（v1.29 的 64×21 退役）、
    校验下拉宽 24、代理下拉宽 26。
  - 键位：明细选择 `↑↓`（并发面板聚焦时）、断开 `B`（不限焦点）、页签 `]`/`[`/`T`
    （含 `}`/`{`）、Esc 主界面无操作；q/Q 先关窗仅适用 pieces/帮助（对话框中 q 为字符输入）。
  - toast 逐字：`⚠ 已完成任务不可修改`（M 拒绝；设计规范 §7.4 的 `已完成任务不支持修改`
    与代码不符，以代码为准）、`仅 BT 任务支持断开并发连接（B）`、
    `当前状态无并发明细（仅下载中/做种中可选）`、`当前状态无可断开的并发连接（仅下载中/做种中）`、
    `已复制 url`/`已复制 校验码`、pieces 四拒绝臂文案不变。
  - 删除框提示行 ` 1/2/3 快捷选择 · Enter 确认 · Esc 取消`（全浮层唯一保留）；Space = 取消。
  - 添加对话框：URL 输入自动回填默认并发（未手动改过并发时；HTTP 4 / BT 20 预留）、
    校验码占位 `<n> 位十六进制（可留空）`、超宽显示 `…+尾部`。

## 二、当前产出情况（specifier v130-spec）

### 本会话产物清单
- `project/mission.md`：v1.30 注记（UI 与交互对齐 ezr-demo 重构版，①–⑪ 十一项差异点全录）。
- `project/mission/phase-01.md`：v1.30 修订注记 + §3.9 条款修订——FR-01-80（页脚收敛/快捷键
  全局/鼠标/toast/窄屏）、FR-01-96（提示行退役 + 滚动条）、FR-01-97（↑↓/B 改版）、FR-01-98
  （页脚 G 项退役 + 焦点回列表）、FR-01-103（提示行退役）、FR-01-106（64×19/图例行/滚动态）、
  FR-01-25（删除框数字直选/Space/唯一提示行）、FR-01-01（74×11/无提示行/自动回填并发/
  超宽尾部）、FR-01-17（Sparkline→流量图术语校准，v1.13 漏同步）；新增 FR-01-107（帮助窗口
  与页脚收敛）/FR-01-108（面板焦点模型）/FR-01-109（通用列表滚动条）；D36/D37 新增。
- `project/features/01-tui-display.feature`：场景 01/03/06/07/08/09/11 修订（03 顺手校准
  v1.14 类型行漏同步）+ 新增 18（面板焦点模型）/19（帮助窗口）/20（通用滚动条）/21
  （删除框数字直选）。
- `project/features/01-add-task.feature`：场景 01/18 修订 + 新增 19（URL 自动回填默认并发
  与 Esc 取消）。
- `project/features/03-modify-task.feature`：场景 07 toast 逐字校准 `⚠ 已完成任务不可修改`
  + v1.30 头部注记。
- `project/features/04-ui-conns.feature`：场景 03（页签键与高亮跟随焦点）/08（无提示行 +
  滚动条）/11（↑↓ 明细选择）/12（B 断开）/14（G 页脚项退役）/15（页脚不精简 + 焦点钳制）修订。
- `project/features/05-ui-pieces.feature`：场景 01（64×19 + 图例行）/02（图例恒显）/04
  （滚动条字形）/05（回归键 ↑↓/B）/07（位置不保留注记）修订；场景 08（页脚 P 分块）
  **退役留空不重排**（撤销痕写进清单注释）。
- `project/features/01-download-engine.feature`：一处术语校准（Sparkline 面板 → 头部内嵌
  单色面积流量图，v1.13 漏同步）。
- `project/qa/01-tui-display-qa.md`：QA-TD-01/03/06/07/08/09/11 同步 + 新增 QA-TD-18..21。
- `project/qa/01-add-task-qa.md`：QA-01-01/18 同步 + 新增 QA-01-19。
- `project/qa/03-modify-task-qa.md`：QA-MT-07 toast 逐字校准。
- `project/qa/04-ui-conns-qa.md`：QA-UC-03/08/11/12/14/15 同步 + **QA-UC-07 漏同步校准**
  （详情定高 11→12 行，v1.20 起即错）。
- `project/qa/05-ui-pieces-qa.md`：QA-UP-01/02/04/05/07 同步 + QA-UP-08 退役（断言反转
  并入 QA-TD-01）+ 一屏块数 496→476（64×19 口径）。
- 工具侧经验沉淀：`packs/_common/engineering.md`（退役标识反向自检纪律）+
  `packs/six-pack/specifier/SKILL.md`（参照实现驱动的修订核证：逐字断言以参照源码为准）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 设计规范 vs 源码核证 | 16 项契约点逐条核证，13 项吻合、3 项以代码为准（B 不限焦点 / q 不关对话框 / 滚动条 thumb floor）+ 2 处文档措辞偏差登记 | 人工通读 `ezr-demo/src/{app,ui,main}.rs`（6.3k 行）+ rg 逐字锚点 |
| 退役标识反向自检 | 0 漏改 | `rg "Ctrl\+↑↓|Ctrl\+B|64×21|提示行含|页脚快捷键|逐级精简|Sparkline" project/features project/qa project/mission.md project/mission/`——命中仅限历史注记与 qa/runners（QA 职责欠账）+ 错字探针 `rg "雳"` 零命中 |
| 规格一致性 | mission ↔ features ↔ qa 三层口径一致（v1.30 项逐一对应；场景与用例号一一映射） | 人工逐文件核对 |
| 场景序号纪律 | 稳定序号不重排：TD 缺 04/16、UP 缺 08（退役痕入注释）；新增取下一空号 | 人工核对场景清单注释 |

### 对账口径（如存在基线）
- 上游 v129-spec 规格基线：features 5 份（TD 15 场景 / ADD 18 / MT 9 / UC 18 / UP 8）→
  本轮 TD 19（+18/19/20/21）、ADD 19（+19）、MT 9（0）、UC 18（0，6 场景修订）、
  UP 7（08 退役）；QA 用例数随场景同幅增减。mission v1.29 → v1.30（新增 FR-01-107..109、
  D36/D37，7 条 FR 修订）。源码基线对账 = ezr-demo 源码逐字核证（见上表第一行）。

### 待办与待批（未决项）
- **D35 待操作者复核**（沿袭，仍开放）：未探测任务按 `P` 拒绝臂文案与门槛序（本轮未触碰，
  拒绝臂四臂与 D35 文案保持 v1.29 口径；demo 源码无未探测臂——demo 无此态，不改判）。
- **APS 机检欠账**（沿袭 + 本轮扩大）：本轮修订的 5 份 feature 待网络可用时补跑
  parser/dry-checker。
- **qa/runners/ 可执行化同步欠账**（沿袭移交，本轮追加同步面）：13 个可执行套件与本轮
  v1.30 规格增量的同步（suite_ui_conns/suite_tui_display 等仍断言 `Ctrl+↑↓`/`Ctrl+B`/
  旧页脚提示行/64×21）——属 six-pack/QA 职责范围，下一轮 QA 会话承接。
- **实现待办（coder 轮承接）**：v1.30 全部 UI 改版（页脚收敛、帮助窗口、焦点模型、滚动条、
  键位改版、64×19、删除框数字直选、toast 节奏、自动回填并发）需在正式版 ezr
  （`project/ezr/src/ui/`、`app/`）实现——现行正式版源码仍为重构前 UI。
- **设计规范文档两处与代码不符已按代码落规格**（供操作者知悉，非待批）：M 拒绝 toast
  文案、滚动条 thumb 舍入方向；如操作者希望以文档为准可改判（改动面 = FR-01-25/
  MT-07 文案 + FR-01-109 几何式）。

### 移交建议
- 建议下一步运行 `six-pack/coder`（以本 handoff 为上游交接内容）：按 v1.30 规格实现
  ezr-demo 重构版 UI 对齐（实现面集中在 `ezr/src/ui/` 七文件 + `app/keys.rs`/`mouse.rs`/
  `dialogs.rs` + 页脚/帮助/焦点/滚动条新组件）。实现完成后再经 cleaner → architect →
  hardender → QA 链路；QA 轮顺带承接 qa/runners 可执行化同步（含本轮 v1.30 增量）。
  移交仅为建议，不构成自动流转（宪法第一章）。

By specifier.
