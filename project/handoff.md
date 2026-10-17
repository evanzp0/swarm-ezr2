# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier　会话：v129-spec（任务 ezr-v129-pieces-pkey）
> 上游交接：QA ezr-qa-final（six-pack 全链路已走完一轮：specifier → coder → cleaner →
> architect → hardender → QA；3363P/0F、e2e 165P/0F/0S、clippy 0、arch 15、覆盖 94.37%）。
> 本轮（操作者指令两批新增需求：P 键 pieces 分块弹窗 + 并发面板提示行随协议自适应）：
> mission 升版 v1.29（FR-01-106 新增、FR-01-96 修订、D35 新增），规格 `05-ui-pieces.feature`
> （8 场景新增）与 `04-ui-conns.feature`（场景 08 修订），QA 套件 `05-ui-pieces-qa.md`
> （QA-UP-01..08 新增）与 `qa/04-ui-conns-qa.md`（QA-UC-08 同步）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.29，分期模式）+ `project/mission/phase-01.md`（v1.29）。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.28 全量规格 + features/ 13 份 + qa/ 13 份 | 全部交付 | 逐轮 parser/dry-check 通过（见归档台账） |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F；CPD 17→15 |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 hardening_g_v147 | 已交付 | 3363P/0F、clippy 0、fmt 0、arch 15 |
| QA（ezr-qa-final） | QA 规程可执行化 13 端到端套件（qa/runners/）+ 缺陷修复（接续钉名）+ 双度量复测 | 已交付 | e2e 165P/0F/0S（13 套件）、3363P/0F、覆盖 94.37%、CRAP≥30=0、CPD 89/15 |
| **specifier（v129-spec，本轮）** | **mission v1.29（FR-01-106 + FR-01-96 修订 + D35）+ features/05-ui-pieces.feature（8 场景）+ features/04-ui-conns.feature（场景 08 修订）+ qa/05-ui-pieces-qa.md（QA-UP-01..08）+ qa/04-ui-conns-qa.md（QA-UC-08 同步）** | 已交付 | **APS 不可用（本轮受限项），人工规范化核对通过；规格就绪待批** |

### 遗留受限项（当前有效）
- **APS 工具（gherkin-parser / gherkin-ir-dry-checker / gherkin-mutator）本轮不可用**（沿袭开放）：
  环境为 fresh clone（.tools/ 空），Babashka 静态二进制经 GitHub release 下载反复停滞
  （24MB 仅取 1.5MB，网络受限），无法按 engineering.md 调用约定安装。影响面：本轮新增/
  修订 feature 未过 parser/dry-checker 机检，以人工核对替代（场景稳定序号连续、参数与
  Examples 列逐一对应、关键字/表格结构对齐既有已过机检的 13 份 feature 同款）；软 Gherkin
  变异继续开放。建议下游会话在网络可用时补装机检复核本轮两份 feature。
- **环境重建（跨会话不持久，沿袭）**：nightly 1.101.0-nightly（rustup minimal + clippy/rustfmt/
  llvm-tools）；lld 桥接 `~/.local/bin/ld.lld`；cargo-llvm-cov 0.9.1、PMD 7.19.0（.tools/）、
  java 21、Python e2e 依赖 pyte/wcwidth——均需在新会话重建（本轮未涉及 Rust 构建与测试）。
- **观察受限突变体（沿袭 hardender 登记，仍开放）**：workers.rs:27:5/51:25/57:14/168:25、
  download.rs:277:52 共 5 条 + 等价 download.rs:85:27——run 级回补基建已铺。
- **ezr-proxy.rs 测试基建（沿袭裁决）**：127 变异点不拆、CRAP/CPD 读数不追责。
- **磁盘/沙箱**：沿袭口径（throttle_proxy 慢速门控用例前台分批 `QA_TP_ONLY=1,3`）。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D34、v138/v140 口径（见归档 handoff 登记清单）。
- **本轮新增：D35**（FR-01-106 未探测臂）——未探测任务（probed = false）按 `P` → toast
  `任务尚未探测，暂无分块信息`、弹窗不打开，门槛序位于「不支持分块」「空内容」两臂之前；
  理由与改判面见 phase-01 §6 D35 行。**待操作者复核（见待办与待批）**。
- 本轮其余全部为 demo 定稿照录（FR-01-106 各臂文案/尺寸/交互逐字对齐 ezr-demo 定稿 §6.4
  与 demo 源码），非实现定义值。

## 二、当前产出情况（specifier v129-spec）

### 本会话产物清单
- `project/mission.md`（v1.29）：头部同步注记——两批需求摘要与产物指针。
- `project/mission/phase-01.md`（v1.29）：①头部 v1.29 修订注记；②**FR-01-106 新增**（P 键
  pieces 分块弹窗：功能门槛四拒绝臂（未选中/未探测 D35/不支持续传/空内容，文案逐字照录
  demo 定稿 §6.5）、窗口形态 64×21 居中可拖拽、四态色块 🟩🟦🟨⬜ 与 🟦 优先、前沿定位、
  滚动/模态/关闭（P/Esc/q/Q 先关窗不退出、Ctrl 分支 demo 同构）、鼠标滚轮/拖拽/点击外部、
  浮层全量重绘、页脚 `P 分块` 新增；BT 恒支持臂 = phase-02 预留）；③**FR-01-96 v1.29 修订**
  （面板底部提示行随任务协议自适应：BT = `Ctrl+↑↓ 选择 · Ctrl+B 断开`、非 BT =
  `Ctrl+↑↓ 选择`，「（仅 BT）」标注退役；01 期实际显示恒为 `Ctrl+↑↓ 选择`）；④**D35 新增**。
- `project/features/05-ui-pieces.feature`（新增，8 场景 01–08）：窗口形态/四态色块（非下载中）/
  🟦 在传与优先/前沿定位与滚动/模态与关闭/功能门槛拒绝臂/鼠标交互/页脚提示。
- `project/features/04-ui-conns.feature`：场景 08 提示行断言改随协议自适应 + 头部 v1.29 修订
  注记 + 场景清单注释同步（其余 17 场景与稳定序号零变动）。
- `project/qa/05-ui-pieces-qa.md`（新增）：QA-UP-01..08，fixture 文件清单 8 份（含 520MB 稀疏
  piece-big.bin 与 0 字节/无 Range/未探测三拒绝臂种子），BT 臂不覆盖声明（phase-02 增补）。
- `project/qa/04-ui-conns-qa.md`：头部 v1.29 同步注记 + QA-UC-08 判据同步（提示行 HTTP =
  `Ctrl+↑↓ 选择`，全行无「Ctrl+B」段与「（仅 BT）」标注）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 规格三方对账（mission ↔ feature ↔ QA） | 一致（FR-01-106 九款、FR-01-96 提示行、D35 均有三方载体；toast 文案与 ezr-demo 定稿 §6.5/源码逐字一致） | 人工核对（对照 ezr-demo/src/app.rs open_pieces/draw_pieces/disconnect_conn、ui.rs draw_pieces/draw_conns、docs/UI交互需求文档-定稿.md §6.1/6.3/6.4/6.5） |
| Gherkin 机检（parser/dry-checker） | **未执行——APS 不可用（受限项）**；人工核对：场景名稳定序号 01–08 连续、Examples 参数与步骤占位逐一对应、关键字/表格结构与既有 13 份已机检 feature 同款 | 机检待补：`(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-parser <feature> <out>.json && bb gherkin-ir-dry-checker <ir> <out>.dry.json)` |
| 场景稳定序号守恒 | 04-ui-conns 18 场景序号零重排（仅场景 08 内容修订）；05-ui-pieces 新占用 01–08，无冲突 | 人工核对两份 feature 场景清单注释 |

### 对账口径（如存在基线）
- 相对上游 QA ezr-qa-final 基线：mission v1.28 → v1.29（+FR-01-106、FR-01-96 修订、D35）；
  features 13 → 14 份（+05-ui-pieces，8 场景）；04-ui-conns 18 → 18 场景（内容修订 1）；
  qa 套件规格 13 → 14 份（+05-ui-pieces，QA-UP-01..08）；qa/04-ui-conns-qa.md 用例数 18 → 18
  （QA-UC-08 判据修订）。规格层增量，不涉及源码与测试基线数字变动。

### 待办与待批（未决项）
- **D35 待操作者复核**（本轮唯一非 demo 照录决策）：未探测任务按 `P` 的拒绝臂文案
  `任务尚未探测，暂无分块信息` 与门槛序（未探测臂先于「不支持分块」「空内容」两臂）。
  如改判（例如改沿用 demo「任务内容为空，无分块信息」），改动面 = FR-01-106①、
  05-ui-pieces 场景 06 一处断言、QA-UP-06 一处判据。裁决后按 handoff-rule 6b 同轮落账
  （D35 行状态列 + 本节闭环注记）。
- APS 机检欠账：两份 feature（05-ui-pieces 新增、04-ui-conns 场景 08 修订）待网络可用时
  补跑 parser/dry-checker（受限项已登记）。
- 沿袭移交（非本轮缺口）：① 5+1 观察受限突变体回补；② 软 Gherkin 变异（APS 安装 + runner
  适配器）；③ qa/runners/ 13 个可执行套件与本轮规格增量（QA-UC-08 判据、05-ui-pieces 全部）
  的可执行化同步——属 six-pack/QA 职责范围，下一轮 QA 会话承接。

### 移交建议
- 建议下一步运行 `six-pack/coder`（以本 handoff 与 mission v1.29 为上游交接内容）：实现
  FR-01-106（P 键 pieces 分块弹窗 + 页脚提示）与 FR-01-96 v1.29（提示行随协议自适应），
  单测覆盖功能门槛四臂与色块口径；完成后再经 cleaner → architect → hardender → QA 链路
  （QA 轮承接 qa/runners 可执行化同步）。移交仅为建议，不构成自动流转（宪法第一章）。
