# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect　会话：v136-uniform-paused-ui（任务 v136-uniform-paused-ui）
> 本轮（v1.23 数据统一轮，同会话 v136 续段）：承接 v1.22，操作者 20261012 bug 报告直判——
> 失败挂起在**数据层也与普通「已暂停」完全一致**：Failed 按空格挂起时即重置 error=None、
> retries=1（注册表快照同口径 `"error": null`/`"retries": 1`，新序列化不再写出 has_error 键）；
> `has_error` 字段**整体退役**（迟到失败守卫改 Paused 全态整体忽略、恢复 toast「重新排队」臂
> 退役回归续传双臂；v1.21 产物快照多余字段反序列化忽略零迁移，兼容测试钉住）。产品
> **2597P/0F**、clippy 0、fmt clean、arch 14 过；ezr-demo 同步（verify 134/0 + 弹窗 3/3 +
> 覆盖层 10/10 + pty 回归 7/7 复跑过）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.23，分期模式）+ `project/mission/phase-01.md`
  （v1.23，挂起态展示与数据均与普通已暂停统一）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（v115 前历轮） | mission/phase-01.md 全量 FR/D 裁决表 + features/ 13 份 + qa/ 13 份 | 全部交付 | parser/dry-check 全过 |
| coder（v120–v122 三批） | FR-01-102/103/104/105 + FR-01-06/51 v1.17 双对话框粘贴与清码 | 已交付 | 各批 2438→2503P/0F、clippy 0 |
| specifier（v124-spec-batch5） | FR-01-51 v1.18 直判规格 + D34 注记 + QA-IC-12 v1.18 | 已交付 | parser + dry-check 过 |
| coder（v124-code-batch5） | 修订 A 终态 sidecar 即时落盘 + 修订 B 清码 retry 直判 | 已交付 | 2506P/0F、clippy 0、fmt 0、arch 12 过 |
| cleaner（v125-clean） | 保持行为清理 8 处 + 四度量复核 | 已交付 | 2506P/0F、CPD tiles 55 |
| architect（v126-arch） | 四阶段架构评审 + 页签计数访问器 + 属性测试 +10 + arch 13/14 规则 | 已交付 | 2523P/0F、arch 14 过、属性 57P/0F |
| hardender（v127-hard） | 差分变异 78/78 killed + CRAP/DRY 对账 + 软 Gherkin 受限登记 | 已交付 | 产品树零改动、基线全绿 |
| coder（v128-code-cpd-tile） | CPD 处置：`bordered_block(fg)` 单源 + 8 调用点收口 + gen-drift 夹具同步 | 已交付 | 2527P/0F、CPD 56→52 tiles |
| cleaner（v129-clean） | 保持行为清理 5 处 + Modify 对话框渲染补测 1 条 + 四度量复核 | 已交付 | 2531P/0F、CPD 51 tiles、scan 2161 点 |
| 会话外登记（操作者直判 v1.19，v130 会话并入正式账） | FR-01-92 R 恢复路径退役：retry() 收窄仅 Failed、挂起 toast「按空格恢复」、R 段测试反转四重断言、FailedPaused 枚举 doc 注记；规格三件套同步（phase-01 注记、01-slots-queue 场景 15、QA-SQ-15） | 已交付 | 2531P/0F（断言反转非增删） |
| 会话外登记（操作者直判 v1.19 续，v130 会话并入正式账） | FailedPaused 徽标色裁决①落账：STATE_COLORS 8→9 态（FailedPaused=White 相邻 Paused）+ 穷尽守卫测试 + 规格/QA 同步（01-tui-display 场景 05、QA-TD-05 七态）；ezr-demo 同步（TaskState/挂起臂/恢复臂/文档 v1.7） | 已交付 | 2535P/0F（+4=守卫测试×4 挂载）、demo clippy 过（8 存量警告） |
| coder（v130-code-v119） | v1.19 切片正式承接核对（零偏差）+ 挂起 toast 文案断言补强 + APS 验收流水线补跑 + handoff 重写并账 | 已交付 | 2535P/0F、clippy 0、fmt 0、parser IR 15/15×2、dry-check error 级 0 |
| specifier（v131-spec-v119） | phase-01.md 头部 v1.19 修订注记补写（R 恢复路径退役 + FailedPaused 徽标白 + 规格/QA 引用）——v130 交接遗留待办闭环 | 已交付 | 注记与六处内嵌口径逐项对账一致；纯文档补账 +7 行 |
| cleaner（v132-clean-v119） | 零改动清理轮：触碰面复核（无清理点）+ 新沙箱工具链重装 + 四度量复核 + 覆盖率基线首次登记 | 已交付 | **2535P/0F**、clippy 0、fmt 0、arch 14 过、覆盖率 94.28%、CPD 51 tiles、scan 2161 点/54 文件、CRAP 门禁 76 条（触碰面零新增） |
| specifier（v133-spec-anchor-fallback） | 修订 B fallback 锚定并入正式账：phase-01.md D34 v1.18 注记补「操作者 20261011 直判认可维持」锚定行——v132 交接遗留待办闭环 | 已交付 | 纯注记补账：git diff 仅 phase-01.md 1 行（D34），代码/feature/QA/度量零改动 |
| architect（v134-arch-clean） | 四阶段架构评审（零结构性违规）+ throttle 令牌账本纯策略提取（bucket_step 策略/IO 分离，机械等价）+ 属性测试 57→60 + 新沙箱工具链重装（nightly + PMD）+ CPD 复测 51 同形 | 已交付 | **2546P/0F**（属性 60P 独立计数）、clippy 0、fmt 0、arch 14 过、CPD 51 tiles 与 v132 基线逐 tile 同形 |
| 会话外登记（操作者直判 v1.20，20261011 指令） | 任务 ID 设计改版：u32 随机数分配（`model::idgen` 单源：SplitMix64 查重分配、0 保留哨兵），`next_id` 全链路退役（App 字段/注册表持久化/测试靶替换），注册表 v1 格式零迁移（旧 next_id 字段反序列化忽略）；详情面板「任务 ID」「创建时间」拆分两行、详情定高 11→12 行（FR-01-80/FR-01-94⑥ 修订 + 两 feature 场景同步）；ezr-demo 同步（随机 ID + 两行详情 + 需求文档 + 预填轮换口径保持 + release 二进制刷新）；归档前复核补遗：`tests/gen/detail_stripped.rs` 因 rustfmt 收尾晚于夹具重生成而漂移，已按守卫口径重生成（经验沉淀 → notes/rust.md「剥头夹具再生成排在 fmt 之后」条目） | 已交付 | 产品 **2578P/0F** 实测（13 测试二进制）、clippy 0、fmt clean；demo cargo check 过 + smoke_v_sync 全过 + verify_ezr 回归（场景 C Fedora 轮换口径保持） |
| architect（v135-merge-failedpaused，本轮） | **v1.21 合并轮（操作者 20261012 四条直判）**：①TaskState 删 FailedPaused 变体（9→8 态），Task 新增 `has_error: bool`（快照持久化 `#[serde(default)]`，旧档缺字段 = false；含旧 `"FailedPaused"` 值的注册表**不兼容**→load None 按全新档处理，拒绝测试钉住）；失败挂起 = Paused+has_error=true（error/fail_kind/retries 保留可见）；②空格恢复统一「一律重新排队」：`resume_paused` 新路径（requeue_reset 公共重置：error/fail_kind/retry_in/has_error 清、retries=1、失效连击清零；校验失败挂起走重校验分支；toast 三臂 = 重新排队/断点恢复+已入队/从头下载+已入队），「有空闲槽直接开跑 Downloading+Start」路径删除，「无空闲下载槽位（n/n）」toast 退役；③UI 按 has_error 判定：STATE_COLORS 9→8（Paused=White 单臂）、列表统计行守卫臂 `Paused if has_error`、详情失败原因行、徽标 `Task::state_label()`（已暂停（失败）文案保留）；④规格三件套同步（phase-01 头部 v1.21 注记 + FR-01-33/92/D25 修订、mission.md v1.21 行、01-slots-queue 场景 09/10/14/15 + QA-SQ-09/10/14/15）；⑤ezr-demo 同步（TaskState/has_error/state_label/resume 臂/requeue 重构/文档 v1.8/verify_ezr 场景 H 新口径/release 二进制刷新）；⑥测试修补 9 处（registry 往返+拒绝、slots 挂起不占槽、ui 挂起恢复流+一律重排钉住、engine 连接级清零改判+resolve 提醒守卫、hardening/tab/property/gen 夹具） | 已交付 | 产品 **2597P/0F**（13 二进制；对账：2578 + 8×引擎挂载 resolve 守卫 + 3×UI 挂载重排钉住 + 8×模型挂载旧档拒绝 = 2597 ✓）、clippy 0、fmt 0、arch_check 全过；demo cargo check 0 警告 + release 构建 + smoke_v_sync 0 失败 + verify_ezr **134/0** + 弹窗 3/3 + 覆盖层 10/10 |
| architect（v136-uniform-paused-ui，本轮） | **v1.22 展示统一轮（操作者 20261012 bug 报告直判）**：①ui/task_lines.rs 徽标改 `t.state.label()` 单源 + stat_row 删 `Paused if has_error` 守卫臂 + 删 `stat_failed_paused` 函数；②ui/detail.rs 失败原因行条件收窄为仅 `Failed`；③model/task.rs 删 `Task::state_label()`（统一后纯委托死抽象）+ has_error 字段 doc 改纯内部语义口径；④model/mod.rs 头注 v1.22 注记、slots.rs 头注、app/tasks.rs 注释口径、ui/mod.rs STATE_COLORS doc；⑤测试重写：`stat_failed_paused_renders_like_plain_paused`（挂起态统计行与普通暂停逐字一致断言）+ `failed_pause_resume_flow` 徽标断言改「已暂停」；⑥gen 夹具 fmt 后重生成；⑦规格三件套同步（phase-01 头部 v1.22 注记 + FR-01-92 展示语义改写 + D25 v1.22 注记、mission.md v1.22 行、01-slots-queue 场景 14/15 + 01-tui-display 场景 05 表行、QA-SQ-14 + QA-TD-05）；⑧ezr-demo 同步（app.rs 删 state_label/注释口径、ui.rs 徽标/统计行/详情三处退役、文档 v1.9 REQ-13.1、release 二进制刷新）+ 新增 pty 回归脚本（操作者 bug 场景端到端 7/7） | 已交付 | 产品 **2597P/0F**（13 二进制；计数与 v135 持平：测试重写 1:1 无增删）、clippy 0、fmt 0、arch_check 全过；demo cargo check 0 警告 + release 构建 + smoke_v_sync 0 失败 + verify_ezr **134/0** + 弹窗 3/3 + 覆盖层 10/10 + **pty 展示统一回归 7/7**（win11 失败→空格挂起→徽标/统计行/详情三断言→空格恢复回等待中） |
| architect（v136-uniform-paused-ui 续段，本轮） | **v1.23 数据统一轮（操作者 20261012 bug 报告直判：registry.json 中 error 未置 null / has_error 未置 false / retries 未置 1）**：①app/tasks.rs Failed 臂挂起即重置 error=None、retries=1（原保留可见口径废止）；②model/task.rs 删 `Task::has_error` 字段 + 初值；③model/registry.rs 删 `TaskSnapshot::has_error`（serde default 随之退役）+ From 双向映射删行 + 头注 v1.23 兼容性文档 + 测试重写 `legacy_has_error_field_is_ignored`（新序列化无 has_error 键断言 + v1.21 产物多余字段注入加载钉住）+ `legacy_failed_paused_variant_is_rejected` 构造修正；④app/engine/evt.rs 迟到失败守卫 `Paused && has_error` → `Paused` 全态**整体忽略**（不写回 error——写回会污染已重置挂起数据）；⑤app/tasks.rs resume_paused 删 had_error 臂（toast 回归续传双臂）+ requeue_reset 删 has_error 行 + 注释/doc 全线 v1.23 口径；⑥model/slots.rs 测试 `paused_with_error_holds_no_slot` → `paused_holds_no_slot`（全 Paused 统一不变式）+ 头注；⑦ui/mod.rs `failed_pause_resume_flow` 重写（挂起即重置 error=null/retries=1 断言 = 操作者 bug 钉住；R 段构造修正）+ STATE_COLORS doc；⑧ui/task_lines.rs 徽标/统计行注释 v1.23 + 测试构造修正（error/retries 构造留作旧档遗留防御）；⑨app/mod.rs tab_counts 测试构造修正；⑩tests/hardening_g_appcore.rs 靶测试改数据重置断言；⑪gen 夹具 fmt 后重生成；⑫规格三件套（phase-01 头部 v1.23 注记 + FR-01-33/92 改写 + D25 v1.23 注记、mission.md v1.23 行、01-slots-queue 场景 14/15、01-tui-display 场景 05 表行、QA-SQ-14/15、QA-TD-05）；⑬ezr-demo 同步（app.rs 字段/两臂/重置/注释、ui.rs 注释、文档 v1.10 REQ-14.1 + toast 汇总挂起臂退役 + 版本历史、release 二进制刷新） | 已交付 | 产品 **2597P/0F**（13 二进制；与 v1.22 持平：测试重写 1:1）、clippy 0、fmt 0、arch_check 全过；demo cargo check 0 警告 + fmt clean + release 刷新 + smoke 0 失败 + verify_ezr **134/0** + 弹窗 3/3 + 覆盖层 10/10 + **pty 展示统一回归 7/7 复跑过**（v1.22 断言与 v1.23 完全兼容） |
### 遗留受限项（当前有效）
- **新沙箱工具链部分重装（v135 轮再装）**：Rust nightly 1.101.0-nightly（080cc2714
  2026-10-11；rustup minimal + rustfmt + clippy；新 shell 须 `export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"`）。
  **未装**：cargo-llvm-cov / cargo-mutants / rustfilt（磁盘余 ~6.5G，llvm-cov 插桩全量
  重编译不可承载）、PMD 7.28.0（`.tools/` 随沙箱重置丢失，Java 环境未验）——**CPD 本轮
  未复测**（触碰面 = model/app/ui 大面积合并重构，51 tiles 基线预计形态变化，移交
  hardender/QA 工具就位后 `pmd cpd -l rust --minimum-tokens 50 --dir src` 复测并登记新基线）。
- **覆盖率 / CRAP / scan 沿 v132 登记口径（多轮未复跑）**：94.28% / 76 条 / 2161 点/54 文件。
  v134 触碰面 throttle.rs + **v135/v136 触碰面 model/app/ui 合并重构（大面积）** 均不在度量内，
  工具就位后须整体复测并替换基线。
- e2e 六套件滞后于规程（累计，v135 扩大）：`suite_add_task.py`、`suite_ui_conns.py`、
  `suite_modify_task.py`、`suite_persistence_config.py`、`suite_tui_display.py`、
  `suite_integrity_check.py`——**v1.21–v1.23 改变可观测行为（挂起态入 Paused、恢复一律重新排队、满槽 toast 退役、
  连接级累计清零、v1.23 挂起数据重置与恢复 toast 双臂）**，`suite_slots_queue.py` /
  `suite_tui_display.py` 断言面进一步滞后，套件同步前 e2e 不可信（移交 QA 会话先同步）。
- **软 Gherkin 验收变异受限**（沿袭 v127 登记）：13 份 feature 中仅
  `01-persistence-config.feature` 有历史 stamp（28/28 killed）；本轮修订
  `01-slots-queue.feature` 场景 09/10/14/15，dry-check 未跑（Babashka/APS 未装）——
  **移交 QA 会话**按 `packs/_common/notes/aps.md` 重装后跑 feature dry-check 与 e2e。
- **变异覆盖面受限**（沿袭）：v135 触碰面（model/task、mod、registry、slots、app/tasks、
  evt、ui 三文件）不在 killed 清单内——如需加固证据由后续 hardender 轮补跑
  （`--file` 按文件分块协议，建议先 `model/slots.rs`+`app/tasks.rs`）。
- **demo QA 脚本二进制路径为绝对路径**（`scripts/verify_ezr.py` PROG 常量）：本轮已指向
  `/home/z/my-project/project/ezr-demo/target/release/ezr-tui-demo`；工作区迁移/克隆路径
  变化时须同步修改（项目级注意事项，非通用）。
- 高 comp 残余（supervisor block_worker/download、dialogs dlg_finish_add、evt on_evt、
  mouse on_mouse_dialog 等分发/编排核心）沿 v129 登记口径；supervisor 模块拆分议题留
  hardender 变异收益兑现时再议。
- 无其他受限项。

### 实现定义值登记（当前有效）
- v135 无新增实现定义值（操作者四条直判即为口径，无自由裁量遗留）。沿袭有效：D28/D29
  （仅添加落态与三钮布局）、D30（下调即时收缩）、D31（1MB 下限）、D32（Enter 语义）、
  D33（1s 归零时延）、D34（清码 R = 不校验直接收尾；v1.18 实现精化 = 文件完整性直判；
  fallback 锚定行已并入）。
- v1.21 恢复 toast 文案（规格已并入 FR-01-33；**v1.23 挂起专属「↻ 重新排队（等待中）」臂
  退役**，挂起恢复与普通暂停同走续传双臂）：可续传
  `▶ 继续下载（从断点恢复，已进入等待队列）: {name}`、不可续传
  `⚠ 服务器不支持断点续传，将从头下载（已进入等待队列）: {name}`。
- 无未决待批项。

## 二、当前产出情况（architect v136-uniform-paused-ui 续段 = v1.23 数据统一轮）

### 本会话产物清单（v1.23 续段；v1.22 展示统一段见产物总账 v136 行）
**产品树（project/ezr）**
- `src/app/tasks.rs`：**Failed 臂挂起即重置** error=None、retries=1（原 has_error=true 置位
  删除；注释 v1.23 口径；fail_kind 保留供校验失败型恢复走重校验分支 D10）；`resume_paused`
  删 had_error 变量与「↻ 重新排队（等待中）」臂（toast 回归续传双臂）；`requeue_reset` 删
  has_error 行；`retry()`/Paused 臂注释口径同步。
- `src/app/engine/evt.rs`：迟到失败守卫 `Paused && has_error`（仅更新 error）→ `Paused`
  全态**整体忽略**（不转态、不写回 error——写回会污染已重置挂起数据，违背 error=null 口径）。
- `src/model/task.rs`：删 `Task::has_error` 字段 + `new_queued` 初值；error 字段 doc 注明
  挂起态不再持有。
- `src/model/mod.rs`：头注 v1.23 注记；`TaskState::Paused` doc；`sample_task` 初值删行。
- `src/model/registry.rs`：删 `TaskSnapshot::has_error`（`#[serde(default)]` 随之退役）+
  From 双向映射删行；头注 v1.23 兼容性文档（新序列化不写该键；v1.21 产物多余字段忽略零迁移）；
  测试：`paused_with_error_roundtrip_preserved` → `legacy_has_error_field_is_ignored`
  （**新序列化无 has_error 键断言** + v1.21 产物注入 `"has_error":true` 加载钉住 + Paused
  按普通暂停处理断言）；`legacy_failed_paused_variant_is_rejected` 构造修正。
- `src/model/slots.rs`：头注/used 注释 v1.23 口径；测试 `paused_with_error_holds_no_slot`
  → `paused_holds_no_slot`（全 Paused 统一不占槽不变式）。
- `src/ui/mod.rs`：STATE_COLORS doc v1.22/v1.23；`failed_pause_resume_flow` 重写——挂起断言
  error=None/retries=1（**操作者 bug 钉住**）替代原 has_error 置位断言，R 段构造删 has_error，
  恢复段删 has_error 清除断言（error/retries 已随挂起重置）。
- `src/ui/task_lines.rs`：徽标/统计行注释 v1.22/v1.23 口径；测试构造删 has_error（error/retries
  构造保留 = 旧档遗留防御断言）。
- `src/ui/detail.rs`：失败原因行注释 v1.23（挂起态不再展示也不再持有）。
- `src/app/mod.rs`：tab_counts 测试删 has_error 构造 + 断言消息更新。
- `tests/hardening_g_appcore.rs`：靶 tasks 82:13 测试改「挂起即重置」断言
  （error=None/retries=1）+ doc 登记。
- `tests/gen/detail_stripped.rs`：fmt 后按守卫口径重生成（detail.rs 注释变更触发漂移红）。

**规格（project/）**
- `mission.md`：v1.23 版本行。
- `mission/phase-01.md`：头部 v1.23 修订注记；FR-01-33 措辞（挂起时即重置）；FR-01-92 改写
  （数据统一 + has_error 退役 + Paused 全态守卫）；D25 v1.23 注记。
- `features/01-slots-queue.feature`：场景 14/15 修订（数据统一 + 恢复路径无差别）。
- `features/01-tui-display.feature`：场景 05 表行注记 v1.22/v1.23。
- `qa/01-slots-queue-qa.md`：QA-SQ-14/15 同步；`qa/01-tui-display-qa.md`：QA-TD-05 注记。

**ezr-demo/**
- `src/app.rs`：删 `has_error` 字段/初值/requeue 行；Paused 臂删 had_error 三臂（回归双臂）；
  Failed 臂 error=None、retries=1（has_error 置位删除）；retry()/头注注释 v1.23 口径。
- `src/ui.rs`：state_color 注记 v1.22/v1.23。
- `docs/UI交互需求文档-定稿.md`：v1.10（生产同步轮六）：REQ-14.1（数据层统一 + has_error
  退役）+ toast 汇总删挂起专属臂 + 版本历史 v1.10 行 + 头部版本号/日期。
- `bin/ezr-tui-demo`：release 二进制刷新（cargo build --release）。

**工具侧（packs/）**
- 无新增（v1.22 段 pty 陷阱沉淀沿用）。

### 验证证据
| 验证项 | 结果 | 命令（可复现，cwd `project/ezr` / `project/ezr-demo`） |
|---|---|---|
| 全量测试 | **2597P / 0F**（13 测试二进制，与 v135/v1.22 持平） | `CARGO_PROFILE_DEV_DEBUG=0 cargo test --all-targets` |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| rustfmt | 0 diff | `cargo fmt --check` |
| 架构检查 | 14/14 规则过 | `bash scripts/arch_check.sh` |
| gen-drift 守卫 | 过（夹具 fmt 后重生成） | 含于全量测试 |
| demo 编译 | 0 警告 | `cd ezr-demo && cargo check` |
| demo smoke | 0 失败 | `python3 scripts/smoke_v_sync.py` |
| demo 回归 | **134/0** | `python3 scripts/verify_ezr.py` |
| demo 弹窗/覆盖层 | 3/3 + 10/10 | `check_popup_integrity.py` / `check_overlay_redraw.py` |
| pty 展示统一回归 | **7/7**（v1.23 复跑过） | `python3 /home/z/.work-tmp/oneoff_v136_paused_ui.py ./target/release/ezr-tui-demo`（一次性脚本，已留档 /home/z/.work-tmp/） |

### 对账口径（基线 → 终态）
- 测试计数：上游 2597P → 本轮 2597P（+0）。v1.23 段测试重写 1:1（registry
  `paused_with_error_roundtrip_preserved` → `legacy_has_error_field_is_ignored`、slots
  `paused_with_error_holds_no_slot` → `paused_holds_no_slot`，其余断言改写不增删）。
- clippy 0 / fmt 0 / arch 14：与 v132–v136 基线持平。
- CPD / 覆盖率 / CRAP / scan：未复测（工具未装，受限项沿袭；v1.23 段触碰面 =
  model 四文件 + app/tasks + app/engine/evt + ui/mod/task_lines/detail + app/mod +
  hardening_g + gen 夹具）。

### 待办与待批（未决项）
- 无新增未决待批项。
- 沿袭移交：六 e2e 套件同步（v1.21/v1.22 行为变更继续扩大 suite_slots_queue /
  suite_tui_display 断言面）+ Babashka/APS/PMD/覆盖率工具重装移交 QA/hardender 会话；
  CPD/覆盖率/scan 基线在工具就位后整体复测替换。

### 移交建议
- 下一角色：**six-pack/hardender**（链序 cleaner → architect → hardender，宪法第一章
  不自动流转，由操作者显式 `bin/swarm begin six-pack/hardender`）。建议要点：
  ①工具链补装 PMD（验 Java）+ cargo-mutants/rustfilt/cargo-llvm-cov（磁盘先清主
  `target/debug`）；②CPD 复测登记新基线（本轮触碰面 = ui 三文件 + model 三文件 +
  app/tasks）；③变异补跑建议先 `model/slots.rs`、`app/tasks.rs`，再 `app/engine/evt.rs`
  （v1.23 守卫改 Paused 全态 + 挂起数据重置后分支变化）；④CRAP/scan 对账沿 v132 口径整体替换。
- 操作者要求新增 feature 时：直接 `bin/swarm begin six-pack/specifier`（首角色按
  宪法第一章由操作者显式指定）。
