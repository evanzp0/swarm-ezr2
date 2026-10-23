# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier　会话：ezr-v149-demo-sync（2026-10-23）
> 上游交接：project/handoff.md（QA v148-qa 交接：最终独立验证闭环、01 期可交付）
> + 操作者 chat 指令（six-pack/specifier：ezr-demo 进行了重构，根据 ezr-demo 的文档和代码，
> 修改 ezr 项目的相关文档）。
> 当轮任务：UI 与交互对齐 ezr-demo 重构版（demo 提交 ef951d2「task detail refactor」+
> 7b0095a「refactor for ui and unknown len」，README 变更记录 9–19）——纯规格轮，
> 产品源码零改动，全部产出在 mission/features/qa 文档与工具侧 packs/（经验沉淀）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.34，分期模式）+ `project/mission/phase-01.md`（v1.34）。
- 源码状态 v148（本轮规格 v1.34 不改代码；全 15 目标 4200P/0F 沿袭 v148-QA 总账）。
  本轮后规格面：mission v1.34 / features 14 份（01-tui-display 有效场景 25）/ qa 规程 14 份。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.34 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | 历轮口径见各批注记；v132 轮 3 份 UI feature APS 机检沿袭未跑（GitHub 不可达） |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F（v128 口径） |
| architect（v145-arch） | supervisor 六文件拆分 + 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 | 已交付 | 3363P/0F、clippy 0、fmt 0 |
| QA（ezr-qa-final / v148-qa） | 13+1 套件 e2e 可执行化（qa/runners/ 14 份）+ 欠账消化 | 已交付 | e2e 167P/0F、插桩全量 4186P+14 doc = 4200P/0F、行覆盖 94.9%、CPD 125 tile |
| specifier（v129–v134/v140/v1.30–v1.33） | FR-01-106…112 + m2/m3 对齐规格 | 已交付 | v140 反向自检过（纯新增） |
| coder（v129–v140 系列） | FR-01-106 实现 + v1.30/v1.31 UI 改版 + m3 对齐 + FR-01-112 内置 DNS | 已交付 | 4162P/0F（v140 口径）、clippy 0、fmt clean |
| cleaner（v141）/ architect（v146） | dns 清理 + 四阶段评审 | 已交付 | 4200P/0F、arch 15/15、CPD 125 tile |
| **specifier（v149-demo-sync，本轮）** | **mission v1.34 + phase-01 v1.34（FR-01-80/81/25/87/94/96/101/103/106/107/108/109 修订 + 新增 FR-01-113…116 + D41…D43）+ features 5 份修订（01-tui-display 新增场景 25–28）+ qa 规程 6 份同步 + 反向自检两处漏改修复** | **已交付** | **逐字断言全部经 ezr-demo 源码核证（app.rs/ui.rs：DetailField/copy_label/draw_val_popup/marquee_bar/draw_close_btn/draw_conns/help_lines/draw_task_dialog 等）；退役标识反向自检通过（活断言零残留）；D43 帮助行数文档冲突以源码落账** |

### 遗留受限项（当前有效）
- **APS 机检沿袭未执行**：v132 修订的 3 份 UI feature 未过 `gherkin-ir-dry-checker`；
  本沙箱 GitHub 不可达未补跑；**本轮修订的 5 份 feature 同样未过机检**（同因），待环境可用时
  与沿袭项一并补跑。
- **本轮规格与既有实现存在预期差（正常）**：v1.34 规格超前于源码 v148（coder 未实现本轮条款），
  e2e 套件（qa/runners/）仍为 v1.30–v1.32 口径、未随本轮修订——运行套件对新增场景会 F，
  属预期，待 coder 实现轮后由 QA 轮同步可执行套件。
- **v147 时序 flake（沿袭）**：本轮全程未运行测试（纯文档轮），状态不变。
- **Rust 工具链沙箱不持久 + GitHub 不可达（沿袭）**：不影响本轮（无构建需求）。
- **dns.rs 覆盖率残余 3 行 / 观察受限突变体 / ezr-proxy 测试基建豁免（沿袭 hardender/QA 登记）**：
  状态不变。
- **CRAP>30 9 处处置（沿袭，移交 hardender）**：状态不变。
- **PC-09/feature 场景 09 旧口径待裁决（沿袭）**：v1.31 登记的 persistence 场景 09 文字统一
  仍待 specifier 小会话处理（本轮范围外，未动）。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D40。
- **本轮新增**：
  - **D41**：[✗] 配色拆分——中括号同边框色（绿+粗、无高亮），高亮的只有 ✗（常态 DIM2、
    悬停/按压 = 仅前景红+粗，无背景/反色）；demo 首版两版配色均废弃。
  - **D42**：复制 toast 描述被复制的值——字段名点击与字段值弹窗 Y 复制共用 `copy_label` 单源
    （`已复制 校验码`/`已复制 url`/`已复制 失败原因`/`已复制 保存路径`），与字段名显示文本
    （v1.34 精简后「失败」「路径」）刻意分离。
  - **D43**：帮助窗口内容行数 = **50 行**（源码 `help_lines()` 逐项清点：9 组标题 + 41 条目）；
    demo 自述设计文档记 51 行为文档误差，以源码为准。

## 二、当前产出情况（specifier v149-demo-sync）

### 本会话产物清单
- `project/mission.md`：追加 v1.34 注记（九条要点全录，与 phase-01 v1.34 同步）。
- `project/mission/phase-01.md`：
  - 头部修订注记新增 v1.34 段（demo 提交号、README 记录号、九条要点、规格/QA 落点清单）；
  - FR 修订 12 条：FR-01-80（字段名精简/行首 2 列/大小 ? B/分块 -/URL 断链注记）、FR-01-81
    （头部「并发」活跃口径）、FR-01-25（删除框提示行退役 + 66×7 padding）、FR-01-87（74×10）、
    FR-01-94（字段/页签行左 2 列）、FR-01-96（活跃口径明细/x·y 徽标/2 列 padding/序号=槽位号/
    B 映射真实连接/横滚上限 +1）、FR-01-101（复制扩展四项 + 热区 x+2/x+11 + D42 toast）、
    FR-01-103（74×12 + [✗]）、FR-01-106（64×21、29 块/行 406 一屏、[✗] 通道）、FR-01-107
    （帮助九组 50 行、组标题 2 列/条目 4 列、可视 20 行、note 行退役、页脚 2 列）、
    FR-01-108（overlay 集合含字段值弹窗）、FR-01-109（实例映射 + 字段值弹窗 Vertical）；
  - FR 新增 4 条：FR-01-113 字段值弹窗（48×12 快照完整值/wrap/仅垂直滚动条/Y 复制/纯静态文本/
    断链显示/拖拽关闭即清/L6 最上层）、FR-01-114 [✗] 关闭按钮（P11；几何/配色/交互/回填）、
    FR-01-115 通用 padding 体系（P12；S1/S2/S3 + 特例表 + 设计清单）、FR-01-116 总大小未知
    展示口径（?%/走马灯/? B/ETA -/分块 -）；
  - D 表新增 D41/D42/D43。
- `project/features/01-tui-display.feature`：场景 03/07/19/21/22/24 修订；**新增场景 25–28**
  （字段名精简与复制四项 / 字段值弹窗 / [✗] 几何配色交互 / 总大小未知口径）；场景 04/14/16
  沿袭退役留空。
- `project/features/04-ui-conns.feature`：场景 06（并发活跃口径）/08（x·y 徽标、活跃明细、
  2 列 padding、槽位号、横滚上限 +1、「待命 -」臂退役）/10（0/y 空态徽标）/17（字段名列表
  更新 + 扩展注记）修订。
- `project/features/01-add-task.feature`：场景 01/02/18 修订（74×12、下拉选项左 2 列前缀、[✗]）。
- `project/features/03-modify-task.feature`：场景 01 修订（74×10、[✗]）。
- `project/features/05-ui-pieces.feature`：场景 01/04/05/07 修订（64×21 六段布局、29 块/行
  406 一屏、[✗] 关闭通道）。
- `project/features/01-slots-queue.feature`：场景 14 失败行述语随字段名精简同步（「失败」行）。
- `project/qa/01-tui-display-qa.md`：QA-TD-03/07/19/21/22/24 判据同步；**新增 QA-TD-25/26/27/28**；
  有效场景计数勘正（25 个 = 场景块实数；前版表头 22 系误计 04/16）。
- `project/qa/04-ui-conns-qa.md`：QA-UC-06/08/10/17 判据同步。
- `project/qa/01-add-task-qa.md`：QA-01-01/02/18 判据同步。
- `project/qa/03-modify-task-qa.md`：QA-MT-01 判据同步。
- `project/qa/05-ui-pieces-qa.md`：QA-UP-01/04/05/07 判据同步。
- `project/qa/01-slots-queue-qa.md`：QA-SQ-14 失败行述语同步。
- 工具侧经验沉淀（宪法第三章，1 条扩展）：`packs/six-pack/specifier/SKILL.md`
  「参照实现驱动的修订核证」补两点延伸——①计数断言同属逐字类（行数/条目数/块数须源码
  逐项清点，文档合计数字易滞后）；②变更记录链可能含「撤销再恢复」来回（按源码终态落账，
  不按记录序线性累积）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 参照实现源码核证 | 本轮全部逐字断言（文案/尺寸/键位/几何/颜色/计数）逐条在 ezr-demo/src 核证 | `grep -n` 于 `ezr-demo/src/{app,ui}.rs`：字段名（ui.rs:1217–1345）、copy_label（app.rs:178–196）、48×12（ui.rs:2803）、[✗]（ui.rs:1055–1070）、74×12/74×10/66×7（ui.rs:1997/2305）、64×21（ui.rs:2399）、帮助 50 行（`help_lines` vec 清点 9 sec + 41 kd）、marquee（ui.rs:409–440）、x/y 项（ui.rs:1525）、横滚 +1（ui.rs:1712）、页脚 PAD=2（ui.rs:1749）、值热区 x+11（ui.rs:1163） |
| 退役标识反向自检 | 活断言零残留（两处漏改已修复：04-ui-conns-17 字段名列表、01-slots-queue-14 失败行述语）；残留命中均为历史注记或退役说明 | 全仓 `grep -rn` 旧字段名（任务 ID/创建时间/保存）/旧尺寸（74×11、74×9、66×6、64×19）/旧徽标（活跃 x）/旧提示行/44 行/[ ✗ ] 于 project/features、project/qa |
| 规格一致性 | mission v1.34 ↔ phase-01 v1.34 ↔ features ↔ qa 四层引用闭合（FR 号、场景号、QA 号一一对应） | 逐文件交叉核对（FR-01-113→TD-25/26、FR-01-114→TD-27、FR-01-116→TD-28 等） |
| 产品源码零改动 | 本轮未触碰 `project/ezr/`（QA 轮 v148 基线 4200P/0F 不变） | `git status`（project/ezr 无变更） |

### 对账口径
- demo 重构范围对账：README 变更记录 9–19 共 11 条 → 本轮规格吸收 9 条行为需求
  （记录 15 为 demo 内部编译修复与回归校准、记录 16 被 18 部分撤销——页脚 chips 恢复，
  均按源码终态落账，不单独立 FR）；记录 16 的 P10 提示行退役、17 的 padding 体系以终态
  吸收（FR-01-25/107/115）。
- 场景/用例增量对账：01-tui-display 有效场景 21 → 25（+25/26/27/28，序号留空不重排沿袭）；
  QA-TD 21 → 25（+4）；其余套件场景数不变、判据修订。
- 测试基线：产品零改动 → 4200P/0F 沿袭有效（本轮未运行测试，纯文档轮）。

### 待办与待批（未决项）
- **D40 微决策待操作者裁决（沿袭）**：宽限期 300ms / 加 `dns` 配置键等改判。
- **PC-09/feature 场景 09 旧口径待裁决（沿袭）**：场景文字统一待 specifier 小会话。
- **APS 机检欠账扩大（沿袭 + 本轮新增 5 份）**：GitHub 恢复可达后补跑
  `gherkin-ir-dry-checker`（v132 轮 3 份 + 本轮 5 份）。
- **本轮无阻塞**：定稿文档视同已收齐答案（SKILL「定稿文档视同已收齐答案」条款——
  ezr-demo README 记录 9–19 + 设计规范 + 源码构成逐项可观察行为与验收口径的定稿级需求，
  两阶段访谈以免答合成执行；真实歧义点仅帮助行数一处，已按源码核证落 D43 并在
  反向自检中复核）。

### 移交建议
- 本轮为规格轮，**建议下一步运行 `six-pack/coder`** 实现 v1.34 条款（实现优先级建议：
  FR-01-113/114（弹窗族）→ FR-01-115（padding 横切）→ FR-01-96/81（活跃口径）→
  FR-01-116（大小未知展示）→ FR-01-80/101/107 细节收口；实现后由 QA 轮同步
  qa/runners/ 可执行套件）。
- 若需独立验证规格一致性，可先跑 `bin/swarm list` / 阅读本 handoff 验证证据列的 grep 入口。
- 移交仅为建议，不构成自动流转（宪法第一章）。

By specifier.
