# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（本轮已完成）→ 建议下一节点 six-pack/cleaner
> 会话：ezr-coder-bug15（2026-10-31，「D64 客户端列收缩下限 26 落地 + ezr-demo 同步 +
> 全量验证」实现轮）
> 上游交接：project/handoff.md（bug15 specifier 轮交接：规格 v1.40/D64，任务依据＝已批
> 规格 + 操作者 bug15 指令，含 ezr-demo 同步授权）。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式）+ `project/mission/phase-01…04.md`。
- phase-01（HTTP/HTTPS 内核与 TUI）已收官；phase-02（BT 内核/做种/管理）已实现并经
  互操作夹具多轮收口。bug15 轮为操作者指令级需求修订落地轮（D64：客户端列收缩下限
  12→26），本轮 coder 全量交付（实现 + 测试 + ezr-demo 同步 + 全量验证）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01/02 及历轮） | features/ 18 份 + qa/ 对应套件 | 已收官/已交付 | 见历轮登记 |
| coder（phase-02 及历次收口轮，含 bug14 落地） | `project/ezr/` 全量 | 已交付 | 见 bug14 轮登记（bin 844P 基线） |
| specifier（bug15 规格轮） | mission ×2 修订 + feature v1.40 + qa 同步 | 已交付 | APS dry-check 通过（findings 99→98） |
| **coder（本轮 bug15 落地）** | 产品 `ui/conns.rs` ×1 + 单测 ×2 + ezr-demo 同步 ×3 文件 | **已交付** | **cargo test bin 852P/0F + 集成 13 目标 5904P/0F + clippy --all-targets 零警告 + fmt 干净；ezr-demo 30P/0F + fmt 干净** |

### 遗留受限项（当前有效，沿袭 + 本轮登记）
- 沿袭（bug14 轮登记，全部不变）：BT 下载限速未接；DHT/BEP 10 完整扩展/uTP 属 phase-03/04；
  cargo-mutants 缺位；死 tracker 约 4 分钟静默窗；TUI 恐慌自愈边界（bug6）；BT partial 进度
  不持久化；QA-SQ-13 竞态沿袭待办；QA-RS-06 沿袭；supervisor block_worker fatal 竞态观察项；
  `App.windows` 失去读者（cleaner 候选裁决）。
- **Inversion 访谈受限项（沿袭）**：chat 选择题面板不可用，操作者 bug15 指令文本即阶段 1
  答案（沿 bug13/14 先例）。
- **ezr-demo verify_ezr.py 场景 N 四项预存假 FAIL（本轮 A/B 判定登记，待 demo 脚本维护轮）**：
  N5/N5b/N10/N11——表头查找断言依赖 44 列视口下「累计下载」单元格完整可见，bug11 列宽
  改版（内容 59→86/94）后该单元格被视口右缘裁切成「累计下」→ 查找 not found 连锁失败；
  A/B 对照（max(12) 旧口径同败 4 项）已证与本轮 D64 改动无关；demo 代码本身正常（pty 探针
  实证 sintel BT 面板 12/12 行 + BT 表头 + 底部水平滚动条）。
- **规格/QA 文档页脚 chips 措辞滞后（预存，待 specifier 轮收编）**：01-tui-display.feature
  与 01-tui-display-qa.md 写 `A 添加 · D 删除 · M 修改 · Q 退出 · H 帮助`，ezr 实现与
  ezr-demo 实现一致渲染 `A 命令 · …`（phase-02 添加任务唯一入口改命令窗口时 runner 与
  实现已同步、两份文档未跟上）；runner 实测断言与实现一致（suite_tui_display 21/21 绿），
  非两端漂移。

### 设计决定登记（沿袭 + 本轮实现口径）
- 沿袭 D1–D64 全部裁决（D64 = bug15 轮已批项，见 mission/phase-02.md）。
- **本轮实现定义值（coder 细化，规格框架内）**：
  - 收缩下限 26 = `BT_CLIENT_MIN` 单常量点（`project/ezr/src/ui/conns.rs`）；收缩公式
    `max(26, 视口宽 − 60)`、基数 60 与吸附窗口 (c−4, c) 不变；内容宽动态 = 94 − 34 +
    client_w；视口 < 86 时内容宽 86 > 视口，超出部分走既有水平滚动条（hmax = 内容宽 −
    视口宽 + 1 口径不变），滚到最右末列「协议」完整可见（D63④ 吸附臂回归锁定）。
  - 规格措辞澄清 ×1（实现期发现，specifier 产物同步修订）：v1.40 判据句「滚到最右后
    各列表头与数据完整不截断」修正为「逐列横向滚动可完整查看各列表头与数据（单元格级
    `…` 截断不再出现，视口外列经滚动到达）」——滚到最右时左侧列本就在视口外，经滚动
    查看即"完整"；四份规格文件（feature/qa/phase-01/phase-02）已同步。**待操作者知悉**。

### 实现定义值登记（累计，覆盖为最新全量）
- 沿袭 phase-01/02 全部口径（D1–D63）+ bug11 键位缺省表 + bug12 客户端列样例集 + bug13
  保存目录归一五组验收例 + bug14（速度舍入容差/收缩基数 60/吸附窗口）+ 本轮：收缩下限
  26（= 极值样例 ArcticTorrent/100.100.100 25 列 + 1 空白）。

## 二、当前产出情况

### 本会话产物清单
- **产品（project/ezr/src/）×1**：
  - `ui/conns.rs`：`BT_CLIENT_MIN` 12→26（D64 唯一实现点）；单测 ×2 新增
    （`bt_client_floor_26_extreme_sample_complete_at_viewport_80`：视口 80 下极值样例完整、
    无 `…`、默认视图前六列表头完整、滚到最右「协议」表头与数据完整；
    `bt_client_floor_26_applies_below_old_min_regime`：视口 72（旧口径 client=12 必截断区）
    无 `…` 且滚动后完整）。
- **ezr-demo 同步 ×3 文件**：
  - `src/ui/conns.rs`：收缩式 `max(12)` → `max(26)`（同构点）；
  - `README.md`：变更记录第 24 条（bug15 同步，D64 全量口径）；
  - `docs/ezr-demo UI界面及交互设计规范.md`：「本轮需求修订」新增 bug15 同步块。
- **notes**：architecture.md ×2（D64 实现口径/demo 同构同步点与同步面边界）；testing.md ×2
  （demo pty 脚本列宽假设陷阱与 A/B 判定/pyte 依赖与单场景跑法）；INDEX 同步。
- **规格措辞澄清 ×4 文件**（见设计决定登记）：feature/qa/phase-01/phase-02 各一处措辞修正。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| cargo test --bin ezr | 852P/0F（含本轮 +2） | `cargo test --bin ezr` |
| cargo test 集成 13 目标 | 5904P/0F | hardening 137 / engine_backfill 755 / g_appcore 659 / g_engine 654 / g_main 2 / g_proxy 16 / g_slots 190 / g_supervisor 628 / g_throttle_err 402 / v115 778 / ui_backfill 785 / g_v147 642 + bin 夹具 46（30+1+15）；首轮 --tests 570s 超时按增量续跑补齐（engineering.md 增量纪律） |
| cargo clippy --all-targets | 零警告 | nightly 1.101.0-nightly（rsproxy 装机） |
| cargo fmt --check | 干净 | ezr + ezr-demo |
| ezr-demo cargo test | 30P/0F | 同步后回归；触面 conns.rs clippy 零命中（11 条预存警告均在未触碰文件，bug14 已登记沿袭） |
| demo pty 探针 | D64 生效实证 | 120×44 sintel BT 面板 12/12 行、客户端列 26、底部水平滚动条出现 |

### 对账口径
- 产品净改动 ×1 文件 +1 常量值 −0（单常量点）；单测净增 2；测试计数 bin 850→852
  （bug14 账面 844 与本轮起点 850 的 +6 差为 HEAD 提交 3a6619a 的后置增量，实测为准）。
- 集成计数与 bug14 账面偏差（v115 776→778、ui_backfill 783→785、engine_backfill
  753→755，各 +2）同源为 HEAD 后置增量；全部 0F。
- ezr-demo 净改动 ×3 文件；行为面仅 conns.rs 收缩下限一处。
- 规格 v1.40 判据句与 QA-UC-08 增判据随 D64 落地（specifier 轮产物 + 本轮措辞澄清）。

### 待办与待批
- **待办（demo 脚本维护，建议归属 QA/下一 demo 同步轮）**：verify_ezr.py 场景 N 四项
  假 FAIL 修复（查找锚改列起点稳定列或滚动步数对齐 hmax=43；见 notes/testing.md 条目）。
- **待办（specifier 轮候选）**：01-tui-display.feature/QA 文档 chips 措辞收编
  （`A 添加` → `A 命令`，与实现/runner/demo 三方一致；需操作者确认属文档收编而非行为改判）。
- **待办（沿袭，cleaner 轮）**：`windows` 退役裁决等 bug14 四项不变；ezr-demo 11 条预存
  nightly lint（未触碰文件）。
- **待批**：D64 实现定义值（下限 26 单常量点）；本轮规格措辞澄清（可回退，单句改版）。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。重点核对：
  ①`windows` 退役裁决；②conns.rs client_w 与 BT_COLW[5] 双源收敛（本轮下限 26 后该
  双源结构不变，收敛收益不变）；③ezr-demo 11 条预存 lint（未触碰文件，nightly 漂移）。
- 给操作者的手测口径：①窄终端（<86 列）BT 下载 → 并发面板客户端值完整无 `…`，底部
  水平滚动条出现，←/→ 滚到最右「协议」表头与数据完整；②86–93 列整表无横向滚动完整
  可见；③≥94 列维持 34 列钦定形态（回归不变）。
- 后续链：cleaner → architect → hardender → QA（宪法第一章：不自动流转）。
