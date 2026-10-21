# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier　会话：ezr-v132-spec-m3-align（2026-10-21）
> 上游交接：coder v137-mouse-leak-fix（交接内容 = 前轮产物总账 + 移交建议 cleaner）
> + 操作者 chat 指令（「ezr-demo ui 和交互进行了重大重构，根据 ezr-demo 文档和它的代码，
> 对 ezr 项目进行更新」——权威 = `ezr-demo/docs/ezr-demo UI界面及交互设计规范.md`
> 适用版本 v0.1.0-m3 与 ezr-demo 源码）。
> 当轮任务：v132 规格轮——UI 与交互对齐 ezr-demo m3 修订版（通用滚动条组件化、并发面板
> 客户端/协议列、头部去版本号、对话框更名、校验 NONE、O 关于窗口、模态统一口径等）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.32，分期模式）+ `project/mission/phase-01.md`（v1.32）。
- 本轮任务 = 操作者 m3 对齐指令轮（v1.32）：仅规格面（mission/phase-01/features/qa），
  无代码改动；ezr 源码仍处 v137 状态（3889P/0F），待 coder 轮实现。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.32 全量规格 + features/ 15 份 + qa/ 15 份 | 全部交付 | v134 起 3 份 UI feature 过 APS 机检（0 错误级）；本轮 APS 机检因沙箱无 bb/网络未执行（见受限项） |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F；CPD 17→15 |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 hardening_g_v147 | 已交付 | 3363P/0F、clippy 0、fmt 0 |
| QA（ezr-qa-final） | QA 规程可执行化 13 端到端套件（qa/runners/）+ 缺陷修复 | 已交付 | e2e 165P/0F/0S、3363P/0F、覆盖 94.37% |
| specifier（v129/v130/v133/v134） | FR-01-106/107/108/109/110 + D35/D36/D38 规格 + v1.31 对齐 | 已交付 | 反向自检通过 |
| coder（v129–v137） | FR-01-106 实现 + 缺陷修复 + v1.30/v1.31 UI 改版 + pieces 🟨 修复 + q 退出鼠标泄漏修复 | 已交付 | 3889P/0F、clippy 0、fmt clean（v137 口径） |
| **specifier（v132-spec-m3-align，本轮）** | **v1.32 规格：P9 通用滚动条组件（FR-01-109 重写）+ FR-01-111 关于窗口 + FR-01-94/95/96/106/107/108/110/80/01/50/87/83 修订 + D39；features 5 份修订（14 退役/24 新增）+ qa 5 份同步** | **已交付** | **反向自检通过（退役标识残留仅历史注记与 runners 欠账）；Gherkin 表结构检查 5/5 通过** |

### 遗留受限项（当前有效）
- **APS 机检本轮未执行**：`.tools/` 不随工作区流转且沙箱无 Babashka、无外网（仓库拉取超时）；
  影响面 = 本轮修订的 3 份 UI feature 未过 `gherkin-ir-dry-checker`。**移交建议**：coder/QA 轮
  在工具链可用环境中补跑（进入 APS 仓库目录调用，输出写 `.work/tmp/`）。
- **qa/runners/ 可执行化同步欠账（沿袭 + v1.32 增量再扩大）**：旧断言残留——`suite_tui_display.py`
  QA-TD-14 版本号断言（场景已退役）、`suite_ui_conns.py` 头部版本号断言、四处 `添加下载任务`
  对话框标题断言、`suite_modify_task.py` 「默认 SHA-256」注释头、滚动条字形/帮助七组类断言；
  影响面 = runners 与 v1.32 规格不符，coder 实现后必然红。**移交建议**：QA 轮一并承接
  （与既有 v1.30/v1.31 增量欠账同账）。
- **arch_check 规则 14（ui 回填白名单）沿袭未过**：8 行同集合（dlg_pos×2/help_pos/
  help_scroll/pieces_pos/pieces_follow/pieces_scroll×2），规则级守恒不变。本轮零触碰代码。
  **移交建议（不变）**：cleaner/architect 轮将白名单正则扩为含 `scroll|pos|follow` 并裁决存量 8 行。
- **v147 时序 flake（沿袭）**：`sidecar_not_written_within_first_500ms` / `hardening_g_v147`
  偶发（隔离 ×3 + 重跑绿）。建议 QA 轮放宽 `wait_state` 预算或加重试。
- **Rust 工具链沙箱不持久（沿袭登记）**：重装清单须含 cargo 镜像配置（已沉淀 rust.md）。
- **观察受限突变体（沿袭 hardender 登记，仍开放）**：workers.rs 5 条 + 等价 download.rs:85:27。
- **ezr-proxy.rs 测试基建（沿袭裁决）**：127 变异点不拆、CRAP/CPD 读数不追责。
- **gen 剥头副本守卫覆盖面（沿袭核对）**：五件守卫面（btn/chart/clipboard/detail/
  update_conn_stats）不含本轮触碰文件（规格文档轮零代码），无需再生成。
- **退出竞态修复的可观测边界（沿袭 v137 登记）**：极端时序下毫秒级鼠标事件残留属业界共有限界。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D38、v138/v140 口径、D35 结案收窄、D38 登记差异
  （维持产品口径，待操作者裁决）。
- **D39（本轮新增）**：关于窗口版本文案照录 demo 钦定 `EZR Download v1.0.0`（不随期号变体；
  期号/产品版本号差异化策略留操作者裁决，改动面 = FR-01-111 与 01-tui-display-24 各一处）。
- **本轮口径注记（非新增 D 值）**：点击浮层外部统一无操作、校验 NONE 默认/必填、对话框更名、
  头部去版本号、图区 0 列空白、滚动条组件契约（几何公式/连发 0.4s+0.12s/余辉 400ms/
  步长 1（水平 2））均为 demo m3 权威口径的直接照录，非角色自定值。
- **demo-only 差异登记（沿袭 + 本轮沿用）**：添加对话框预填演示数据（ADD_POOL 轮换）为 demo
  演示辅助，正式版维持空字段；并发面板「客户端/协议」两列的演示数据生成方式（确定性缩写池 +
  1/3 uTP 概率）为 demo 演示辅助，正式版数据源 phase-02 定夺。

## 二、当前产出情况（specifier v132-spec-m3-align）

### 本会话产物清单
- `project/mission.md`：新增 v1.32 注记（m3 对齐需求十项全录，含逐项 demo 源码核证锚点）。
- `project/mission/phase-01.md`：新增 v1.32 修订注记；FR-01-109 全面重写（P9 通用滚动条组件 =
  滚动条引擎 + ScrollablePanel 一体化封装：字形 `═║`/单线轨道/箭头留白分轴向规则/显示条件
  组件内收口/几何公式与行程保证/拖拽-连发-按压高亮-轨道翻页契约/键盘 ←→ 横向）；新增
  FR-01-111（O 快捷键与关于窗口 40×7，z-order 最上层、位置跨次记忆）；FR-01-94①⑤（头部去
  版本号 + 左对齐、图区 0 列空白）、FR-01-95（clamp −93）、FR-01-96（BT 七列预留 + 客户端/协议
  两列 + 横向滚动 + ←/→ ±2 列）、FR-01-01（NONE 默认/必填/更名/提示删除）、FR-01-87（NONE
  预填与清除路径、更名）、FR-01-107（八组 44 行 + O 条目 + 点外无操作）、FR-01-108（overlay
  集合含关于）、FR-01-110②（统一口径注记）、FR-01-80（O 键/←→/鼠标口径）、FR-01-50（NONE
  下拉项注记）修订；FR-01-83 退役登记；新增 D39。
- `project/features/01-tui-display.feature`：场景 14 退役（版本号随期号递进，留空不重排）、
  场景 24 新增（关于窗口）、场景 07/19/20/22/23 修订（O 键、帮助八组 44 行 + 点外无操作 +
  O 叠帮助、通用滚动条组件全交互契约、chips 点外无操作、对话框点外统一口径注记）。
- `project/features/04-ui-conns.feature`：场景 01/04/08 修订（标题无版本 + 左对齐、图区 0 列
  空白 + clamp −93、滚动条组件字形 + BT 七列预留 + HTTP ←/→ 不响应）。
- `project/features/05-ui-pieces.feature`：场景 04/07 修订（通用滚动条 + 连发/高亮/轨道翻页/
  滑块拖拽滚网格、点外无操作、滚动条命中优先于窗口拖拽）。
- `project/features/01-add-task.feature`：场景 01/02/03/05/19 修订（更名、八项含 NONE 默认、
  NONE 禁输入臂、必填拒绝臂、占位提示删除断言）。
- `project/features/03-modify-task.feature`：场景 01/05 修订（更名 + NONE 预填臂、NONE = 清除
  校验重写 + 必填拒绝臂）。
- `project/qa/01-tui-display-qa.md`：头部计数与 v1.32 同步注；QA-TD-07/19/20/22 修订、
  QA-TD-14 退役登记、QA-TD-24 新增。
- `project/qa/04-ui-conns-qa.md`：QA-UC-01/04/08 判据修订 + v1.32 同步注。
- `project/qa/05-ui-pieces-qa.md`：QA-UP-04/07 判据修订 + v1.32 同步注。
- `project/qa/01-add-task-qa.md`：QA-01-01/02/03/05 判据修订。
- `project/qa/03-modify-task-qa.md`：QA-MT-01/05 判据修订。
- `packs/six-pack/specifier/SKILL.md`：经验沉淀 1 条（组件化重构对齐轮的三层核证，跨项目纪律）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 参照实现逐字核证 | m3 全部逐字断言（文案/尺寸/字形/常量/公式/时序）经 ezr-demo 源码核证：scrollbar.rs（EDGE_PAD/ARROW_PAD/min_len/thumb 公式/render）、scrollable_panel.rs（四态/收口）、app.rs（FIRST_DELAY 400ms/REPEAT 120ms/SbKey::step ConnsH=2/CHECKSUM_ALGOS 8 项含 NONE/必填 toast/about/hover）、ui.rs（列宽 59/帮助 44 行 8 组/About 40×7/头部标题/图区 chart_w clamp −93） | 逐文件通读（本轮会话记录） |
| 退役标识反向自检 | 通过——`▲│█▼`/「点击外部关闭」/旧标题/「默认 SHA-256」/`v0.1.0-01` 全仓检索：features/qa 活跃断言零残留（残留仅为「原 X 退役」历史注记、空态引导文案「按 A 添加下载任务」（demo m3 仍保留，非退役对象）与 qa/runners 欠账（已登记）） | `rg -n '▲│█▼|点击外部关闭\|添加下载任务\|默认 SHA-256\|v0\.1\.0-01' project/features project/qa` |
| Gherkin 结构检查 | 5 份修订 feature 表行结构 5/5 通过 | python 逐行表行完整性检查（本轮） |
| APS 机检 | **未执行**（受限项） | 需 bb + APS 仓库（`.tools/` 不流转、无网络） |

### 对账口径（如存在基线）
- 本轮为纯规格轮，零代码改动：ezr 源码测试基线维持 v137 口径 3889P/0F 不变（未重跑——
  代码与工具链均未触碰；Rust 工具链沙箱不持久，重跑需先按 rust.md 配方重建）。
- 规格面对账：features 有效场景数 01-tui-display 23→22（14 退役、24 新增）；其余 4 份场景数
  不变（口径修订）；qa 用例 QA-TD +1（24）、QA-TD-14 退役留空。

### 待办与待批（未决项）
- **待操作者裁决（沿袭开放）：对话框拖拽起拖面/点击时机**（FR-01-110①/D38）——本轮未触及，
  维持产品口径零改动。
- **待操作者知悉（本轮改判项，均照录 demo m3 权威口径，如需推翻请指示）**：
  ①点击浮层外部统一 = 无任何操作（v1.29/v1.30 的 pieces/帮助「点击外部关闭」退役）；
  ②校验语义改判——NONE 默认 + 非 NONE 算法校验码必填（原「任一算法留空 = 不校验」退役，
  「无校验」路径改由 NONE 承载；CLI `-x` 与伴随文件来源不受影响）；
  ③头部标题栏版本号移除（原「版本号随期号递进」退役，版本展示面迁移关于窗口，文案
  `EZR Download v1.0.0` 不随期号变体 = D39，如需期号版本可改判）。
- **qa/runners/ 同步欠账**（沿袭 + v1.32 增量，清单见遗留受限项）：QA 轮承接。
- **APS 机检补跑**（本轮受限项）：工具链可用环境补过 3 份 UI feature。
- **arch 规则 14 白名单扩展建议**（沿袭）：cleaner/architect 裁决。
- **v147 时序 flake**（沿袭）：如复现，QA 轮放宽预算或加重试。

### 移交建议
- 建议下一步运行 `six-pack/coder`（以本 handoff 为上游交接内容）：v1.32 规格已就绪待批，
  按 coder 职责实现 m3 对齐（实现面建议顺序：①ScrollablePanel 组件 + 滚动条引擎（FR-01-109
  重写，全实例替换旧 lazy_scrollbar）；②头部标题/图区（FR-01-94①⑤/95）；③对话框更名 +
  校验 NONE/必填/提示删除（FR-01-01/87/50）；④FR-01-111 关于窗口 + O 键；⑤帮助八组 44 行；
  ⑥模态统一口径（点击外部无操作）；并发面板 BT 七列仅更新预留常量/注释（01 期不触发））。
  实现后 QA 轮顺带承接 qa/runners 同步欠账。
- 其后按链路 cleaner → architect → hardender → QA。移交仅为建议，不构成自动流转（宪法第一章）。

By specifier.
