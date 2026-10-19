# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier　会话：ezr-v134-spec-m2（2026-10-19）
> 上游交接：coder v133-opfix（交接内容 = 前轮产物总账 + D35/D38 补录移交）+ 操作者 chat 指令
> （「ezr-demo 的 ui 和交互进行了重构和 bug 修复，根据『ezr-demo UI界面及交互设计规范.md』和
> ezr-demo 代码，修改相关的文档」）。
> 当轮任务：规格同步轮 v1.31（UI 与交互对齐 ezr-demo m2 修订版 + 上游移交 D35/D38 补录落账）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.31，分期模式）+ `project/mission/phase-01.md`（v1.31）。
- 本轮任务 = 操作者指令轮（v134）：规格对齐 ezr-demo m2（11 项）+ 上游移交 D35 结案/收窄、
  D38 补录；无产品代码改动（纯规格轮）。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.30 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | v133 起 features 已过 APS 机检（0 错误级） |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F；CPD 17→15 |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 hardening_g_v147 | 已交付 | 3363P/0F、clippy 0、fmt 0 |
| QA（ezr-qa-final） | QA 规程可执行化 13 端到端套件（qa/runners/）+ 缺陷修复 | 已交付 | e2e 165P/0F/0S、3363P/0F、覆盖 94.37% |
| specifier（v129/v130/v133 前 spec 轮） | FR-01-106/107/108/109 + D35/D36/D37 规格 | 已交付 | 反向自检 grep 通过 |
| coder（v129–v131） | FR-01-106 实现 + 两轮缺陷修复 | 已交付 | bin 485P/0F + 集成 3727P/0F（v131-fix 口径） |
| coder（v132-ui） | v1.30 全部 UI 改版实现 | 已交付 | bin 497P/0F + 集成 2993P/0F、clippy 0、fmt 0、arch 规则级守恒 |
| coder（v133-opfix） | 三缺陷修复（对话框拖拽 D38、缩容连接回收、未探测臂收窄）+ D35 结案登记 + APS 欠账清偿（14 features 机检 0 错误级） | 已交付 | 3843P/0F、clippy 0、fmt 0、arch 15 OK 守恒（规则 14 基线同败，见受限项） |
| **specifier（v134-spec-m2，本轮）** | **mission.md v1.31 大纲注记 + phase-01.md v1.31（FR-01-95/96/103/106/107/109 修订、FR-01-110 新增、D35 结案/收窄注记、D36 v1.31 注记、D38/FR-01-110 登记）+ features 3 份修订（01-tui-display 01/09/11/19 修订 + 22/23 新增；04-ui-conns 05/10/13/14/15 修订；05-ui-pieces 01(⑥前)/06/07 修订）+ QA 3 份同步（QA-TD-01/09/11/19 修订 + QA-TD-22/23 新增、QA-UC-05/10/13/14/15、QA-UP-06/07）** | **已交付** | **APS 机检 3/3 feature 0 错误级（advisory：01-tui-display 124 / 04-ui-conns 58 / 05-ui-pieces 22，含 1 条既有合法 duplicate-in-scenario）；退役标识反向自检通过（1 处漏改当场修复）** |

### 遗留受限项（当前有效）
- **arch_check 规则 14（ui 回填白名单）沿袭未过**：基线 6 行 + v133 新增 2 行（dlg_pos 首帧
  居中回填）= 8 行同形态（视图态回填而非策略状态改写），规则级守恒不变（15 项 OK、仍仅
  规则 14 一条失败）。**移交建议（不变 + 扩面）**：cleaner/architect 轮将白名单正则扩为
  含 `scroll|pos|follow`。注意：v1.31 页脚双行与浮层零清屏落地后 ui/main 层将新增回填
  字段（footer_cmd_rects 等）与移除 overlay_sig，coder 轮落地后 arch 扫描面会变化。
- **v147 时序 flake（沿袭）**：`sidecar_not_written_within_first_500ms` / `hardening_g_v147`
  偶发（隔离 ×3 + 重跑绿）；建议 QA 轮放宽 `wait_state` 预算或加重试。
- **Rust 工具链沙箱不持久（沿袭登记）**：新会话先按 `packs/_common/notes/rust.md` §1 探针归因。
- **观察受限突变体（沿袭 hardender 登记，仍开放）**：workers.rs 5 条 + 等价 download.rs:85:27。
- **ezr-proxy.rs 测试基建（沿袭裁决）**：127 变异点不拆、CRAP/CPD 读数不追责。
- **qa/runners/ 可执行化同步欠账（沿袭 + 本轮再扩大）**：13 个可执行套件与 v1.30 增量的
  同步欠账沿用；本轮 v1.31 新增行为面（页脚双行 chips 点击、并发空态二分未选中臂、
  滚动到底即停、对话框拖拽场景 23、Ctrl+C 直通、帮助 36 行）未进 runner——属 six-pack/QA
  职责，QA 轮一并承接。
- **APS 工具链环境重建（本轮登记）**：`.tools/` 随会话不持久，本轮重建 bb v1.13.226 +
  APS main tarball（GitHub 直连超时，经 gh-proxy.com 镜像直链取得，配方沉淀
  `packs/_common/engineering.md`「release 资产与 codeload 全域慢/超时走镜像代理直链」）；
  下轮需机检时先确认 `.tools/bb` 与 `.tools/Acceptance-Pipeline-Specification/` 在位，缺则按
  engineering.md 条目重建。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D37、v138/v140 口径（见归档 handoff 登记清单）。
- **D35 结案（操作者裁决 2026-10-18：保留现在方案，本轮落账进 mission）**：未探测任务按
  `P` 拒绝臂的文案与门槛序维持 v1.29 定型，复核待办关闭。
- **D35 实现层收窄（v133 实现，本轮 v1.31 规格落账）**：未探测臂命中条件由 `!probed` 收窄为
  `!probed && total == 0`（依据 = D35 立臂理由原文「探测前大小未知态」；恢复重探窗口
  total 已知不属未知态，落入正常三臂可开窗）。FR-01-106① 已同步。
- **D38 登记（v133 操作者指令，本轮 v1.31 规格落账）**：全部弹窗（添加/修改/删除对话框）
  支持左键拖拽移动（FR-01-110）。**遗留待裁决差异**：产品口径 = 非交互区起拖 + 字段/按钮
  Down 即点击；demo m2 口径 = 窗口内任意位置起拖 + Up 未移动 = 点击（可观察差异：字段行/
  按钮上按住拖动，demo 窗口随动、产品不随动）。FR-01-110① 已双向登记，是否对齐 demo
  留操作者裁决（见待办与待批）。

## 二、当前产出情况（specifier v134-spec-m2）

### 本会话产物清单
- `project/mission.md`：头部新增 v1.31 大纲注记（11 项同步范围 + demo-only 差异登记 + 规格指针）。
- `project/mission/phase-01.md`（v1.31）：
  - 头部 v1.31 修订注记（对齐 ezr-demo m2 逐项源码核证 + 上游移交补录）；
  - FR-01-107 ①页脚双行改版（行 1 五 chips 可点击：按压反色高亮 + 400ms 余辉、原地松开执行
    同键盘口径、拖出取消、命中矩形每帧重建、P7 整枚不渲染、模态不可达；行 2 toast 独占行；
    总高 4 行、左右各 1 列内边距）+ ②【全局】组「页脚命令」条目、内容 35→36 行；
  - FR-01-80 页脚描述改双行 + Ctrl+C 全局例外注记 + toast 语义前缀注记（✂/⚠/⏸/▶/✓）；
  - FR-01-96 并发空态二分（未选中 →「（未选中任务）」/无明细 →「（无并发连接）」）+ 滚动
    到底即停双重防线注记；FR-01-109 滚轮向下事件内立即钳制注记；FR-01-95 流量图暖机注记；
    FR-01-103④ 按钮行右缘钳制注记；
  - FR-01-106 ①未探测臂收窄（`probed = false 且 total = 0`）+ ⑧浮层渲染口径改增量 diff
    零清屏（FNV-1a + terminal.clear() 退役，clip_wide_at_edges 根治）；
  - 新增 FR-01-110 对话框拖拽（产品契约口径 + demo m2 差异登记）与 D38 行；
  - D35 结案 + 收窄注记；D36 v1.31 修订注记。
- `project/features/01-tui-display.feature`：场景清单头 + 头部 v1.31 同步注；场景 01/09/11/19
  修订（页脚双行断言、toast 独占行、窄屏双行降级、帮助【全局】组页脚命令条目）；新增场景
  22（页脚命令 chips 点击执行与点击效果）、23（对话框拖拽移动与 Ctrl+C 全局直通）。
- `project/features/04-ui-conns.feature`：场景清单头 + 头部 v1.31 同步注；场景 05 暖机判据、
  10 空态二分（取消选中臂）、13 到底即停双重防线、14/15 页脚双行断言。
- `project/features/05-ui-pieces.feature`：场景清单头；场景 06 未探测臂前置条件细化
  （`!probed && total == 0`）+ 重探窗口注记（端到端不驱动、产品单测承载）；场景 01 与 07
  零清屏渲染口径。
- `project/qa/01-tui-display-qa.md`：v1.31 同步注；QA-TD-01/09/11/19 修订 + QA-TD-22/23 新增。
- `project/qa/04-ui-conns-qa.md`：v1.31 同步注；QA-UC-05/10/13/14/15 同步。
- `project/qa/05-ui-pieces-qa.md`：v1.31 同步注；QA-UP-06/07 同步。
- `.tools/`：bb v1.13.226（linux-amd64-static，gh-proxy 镜像直链）+ Acceptance-Pipeline-
  Specification 仓（main tarball）——APS 机检环境就位（会话缓存，不随工作区流转）。
- 工具侧经验沉淀：`packs/_common/engineering.md` 新增「release 资产与 codeload 全域慢/超时
  走镜像代理直链」条目（镜像代理 URL 拼法 + 多镜像短超时试拉 + 落盘 file 校验）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| APS 机检（本轮 3 份修改 feature） | **3/3 解析通过、错误级 0**；advisory findings：01-tui-display 124（near-duplicate 35 / possible-synonym 89）、04-ui-conns 58（21/37）、05-ui-pieces 22（duplicate-in-scenario 1 / 8 / 13）——全部为既有 advisory 形态；05-ui-pieces 的 1 条 duplicate-in-scenario = 场景 05「弹窗关闭且应用未退出」逐臂同断言（v133 轮已人工复核为合法重复），非本轮引入 | `(cd .tools/Acceptance-Pipeline-Specification && ../bb run gherkin-parser <feature> <out>.json && ../bb run gherkin-ir-dry-checker <ir> <out>.dry.json)`；产物在 `.work/tmp/aps/`（会话级，复跑即得） |
| 参照实现源码核证 | 本轮 11 项 v1.31 断言逐项核证于 ezr-demo 源码：draw_footer（chips/分隔符/PAD=1/footer_cmd_rects）、footer_press/footer_flash（Duration::from_millis(400)）、run_footer_cmd（与键盘同口径）、keys Ctrl+C 直通臂（先于模态分派）、mouse.rs ScrollDown `(scroll+2).min(count−vis)`、main.rs 无 terminal.clear() + clip_wide_at_edges（7 处调用点）、help_lines 页脚命令 kd 条目、toast ✂/⚠/⏸/▶/✓ 文案、conns 空态两分（ui.rs 1233/1244）、ADD_POOL/prefill_seq（demo-only）、暖机注释 | 本会话 grep + 精读 `ezr-demo/src/{ui,app,main}.rs`（HEAD = b9062cf） |
| 退役标识反向自检 | 通过：`H 帮助 · Q 退出`（余量全在历史注记/D36 行/FR-01-106⑨ 历史段）、`全量重绘`（余量全在退役描述）、`64×21`/`Ctrl+B`/`Ctrl+↑↓`（余量全在改键历史注记）、`（无并发连接）`（余量为正确两臂断言）；发现 1 处漏改（05-ui-pieces:34 场景 01「全量重绘」活断言）当场修复并复跑机检通过 | `grep -rn "<退役标识>" project/features/ project/qa/ project/mission/` 人工判读语境 |
| 产品实现面对账（为 coder 移交） | 已实现无需动：Ctrl+C 全局例外（keys.rs:19 先于模态分派）、滚动事件内钳制（mouse.rs ScrollDown）、并发空态二分（ui/conns.rs 64/75）、对话框拖拽 D38（mouse.rs/dialogs.rs）、按钮右缘钳制（ui/btn.rs）、未探测臂收窄（pieces.rs）、toast 前缀；**待实现（v1.31 规格面）**：页脚双行 chips（ui/footer.rs 单行 + mod.rs Length(3)）、帮助 36 行 + 页脚命令条目（ui/help.rs 现 35 行）、浮层翻转零清屏（main.rs overlay_sig + terminal.clear() 待退役）、并发明细渲染层最后满窗钳制（ui/conns.rs:115 现钳 n−1） | 本会话 grep 产品源码核对 |

### 对账口径（如存在基线）
- 上游 v133-opfix 规格基线 = mission v1.30 + features 14 份（v133 机检 0 错误级）。本轮纯规格
  轮：features 场景总数 14 份不变、场景数 +2（01-tui-display 22/23），场景序号无重排（退役
  留空惯例沿袭）；qa/ 14 份不变、QA 用例 +2（QA-TD-22/23）。产品代码 0 改动，测试计数口径
  不变（上游 3843P/0F 仍为当前有效口径）。
- 05-ui-pieces 机检 findings 8→15（possible-synonym）系场景 06/07 注记扩写引入的同义提示，
  全部 advisory 级，人工判读无误报风险（错误级仍 0）。

### 待办与待批（未决项）
- **待操作者裁决：对话框拖拽起拖面/点击时机**（FR-01-110①/D38 已双向登记）——产品 v133
  口径（非交互区起拖 + 字段/按钮 Down 即点击）vs demo m2 口径（窗口内任意位置起拖 +
  Up 未移动 = 点击）。可观察差异仅在「字段行/按钮上按住并拖动」手势：demo 窗口随动、
  产品按点击消费。裁决 A = 维持产品口径（规格已按此落账，零改动）；裁决 B = 对齐 demo
  （coder 轮调整 mouse.rs 起拖面与点击时机 + 测试）。默认不阻塞下游。
- **v1.31 规格面实现欠账（移交 coder）**：①页脚双行 chips（含 footer_cmd_rects/press/flash
  与 Constraint 3→4）；②帮助【全局】组页脚命令条目 + 36 行；③浮层翻转零清屏（退役
  overlay_sig + terminal.clear()，引入 clip_wide_at_edges 或等价清扫——注意与 arch 规则 14
  回填白名单的联动）；④并发明细渲染层最后满窗起点钳制（`start = min(len − rows)`）。
  逐字断言依据 = ezr-demo m2 源码 + 本轮 FR-01-107①/106⑧/96/109。
- **arch_check 规则 14 白名单扩展建议**（沿袭移交）：cleaner/architect 裁决；v1.31 落地后
  回填字段面会变化，建议 coder 落地后复核扫描面。
- **v147 时序 flake**（沿袭）：如复现，建议 QA 轮放宽 `wait_state` 预算或加重试。
- **qa/runners/ 同步欠账**（沿袭 + 本轮再扩大）：QA 轮承接（含 v1.30/v1.31 增量行为面）。
- **制度修订建议（报操作者，不自行改动 SKILL.md）**：specifier SKILL.md「参照实现驱动的
  修订核证」建议补一句——参照实现文档/源码与操作者已裁决登记的产品口径冲突时，以操作者
  裁决为规格锚、demo 差异登记进修订注记留裁决（本轮 D38 实际执行口径，跨项目可复用）。

### 移交建议
- 建议下一步运行 `six-pack/coder`（以本 handoff 为上游交接内容）：承接「v1.31 规格面实现
  欠账」四项（页脚双行、帮助 36 行、浮层零清屏、并发明细最后满窗钳制），逐字契约以
  ezr-demo m2 源码与 phase-01 v1.31 为权威；对话框拖拽起拖面差异如操作者裁决改判 B，一并
  承接。
- 其后按链路 specifier（如裁决改判需同步规格）→ cleaner → architect → hardender → QA；
  QA 轮顺带承接 qa/runners 可执行化同步（含 v1.30/v1.31 增量）。移交仅为建议，不构成自动
  流转（宪法第一章）。

By specifier.
