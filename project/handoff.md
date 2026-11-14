# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（v1.56 D80 裁决落实与 demo 口径对齐轮，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner/architect/QA 收敛链 ⇒ specifier 显示指标/调整/裁决/新增需求轮（v1.47–v1.53）⇒ specifier 裁决落实修订轮（v1.54）⇒ coder 产品实现/复核重验轮 ⇒ specifier PEERS/SEEDS 核证修订轮（v1.55）⇒ **specifier D80 裁决落实与 demo 口径对齐轮（本轮 2026-11-14，需求 = 操作者 D80 闸门裁定 chat 两项；下一步建议 six-pack/coder 产品实现轮）**
> 会话：six-pack-specifier-20261114（任务 = D80①②③ 批准结案登记 + D80④ 修订（EZR-DEMO 中 SEEDS、PEERS 字段定义与 EZR 保持一致）+ 做种臂样例真实做种形态 0(45)/35(120)；上游交接 = project/handoff.md 核证修订轮版 + 操作者本轮裁定文本）
> 环境：stable 1.100.0 + nightly 1.102.0-nightly 双工具链 + APS 仓/pyte 沿上轮在位；本轮无新环境重建项（每 Bash 调用重 export PATH=~/.cargo/bin 纪律沿用）。qBt 源快照已随上轮 complete 清理（结论固化 D80① 与需求文档）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.56**）+ `mission/` 两份 phase 详述（**phase-01 v1.53**、**phase-02 v1.24**）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-13 QA 观察项清理轮） | ezr 产品实现 + 15 套件 e2e + 收敛/终验链 | 已交付 | 历轮对账在案（单测 8060P/0F、e2e 191P/0F 基线，历环境口径） |
| specifier 新增需求轮（2026-11-13，v1.53） | mission v1.53 + phase 详述 + features 六份修订 + QA 六份同步 + ezr-demo 同构落地 | 已交付 | APS 双检 6/6；demo 单测 58P/0F、PTY 29 项全过 |
| specifier 裁决落实修订轮（2026-11-13，v1.54） | mission v1.54 + phase-01 v1.53/phase-02 v1.22 + features 三份修订 + QA 三份同步 + ezr-demo HTTP 臂 LT 同构 | 已交付 | APS 双检 3/3；demo 单测 58P/0F、PTY 28 项全过、clippy 零警告 |
| coder 产品实现轮（2026-11-13） | 产品端五项全量落地（FR-01-118/FR-02-37 v1.2/FR-02-38 v1.21+22/FR-02-39/FR-02-40 + help SR/LT + 模型层 tracker/连接值分离） | 已交付 | 单测 8100P/0F、clippy --all-targets 0 警告、fmt 过 |
| coder 复核重验轮（2026-11-14） | 产品端零代码新增；tests/gen 剥头副本 4 份机械重生成 + notes 沉淀 + handoff 重写 | 已交付 | 单测 8100P/0F（13 目标逐目标实测）、clippy 0 警告、fmt 全量合规 |
| specifier PEERS/SEEDS 核证修订轮（2026-11-14，v1.55） | mission v1.55 + phase-02 v1.23（FR-02-37 v1.3、D80①–④）+ features/09-bt-engine v1.53（场景 10/21）+ QA-BE-10/21 v1.53 + ezr-demo y1 排除种子同构落地 + notes + handoff 重写 | 已交付 | APS 双检通过；demo 单测 58P/0F（nightly）、clippy --all-targets 0 警告、PTY verify_v153 28 项全过（当轮口径 C1 = 12(45)/23(120)/2.418，已被本轮 D80④ 修订取代） |
| **specifier D80 裁决落实与 demo 口径对齐轮（本轮 2026-11-14，v1.56）** | **mission v1.56 + phase-02 v1.24（D80①②③ 批准结案、④ 修订结案）+ ezr-demo 字段语义对齐（seeders=x1/peers=y1 直存直读，饱和减法退役）+ 做种臂样例真实做种形态 0(45)/35(120)/2.418 + Trackers 弹窗假数据锚点 + 守卫测试 + verify_v153 判据 + docs 设计规范 v1.56 补记 + notes 两文件 + 本 handoff 重写** | 已交付 | demo 单测 **58P/0F**（nightly）、clippy **--all-targets 0 警告**（nightly）、PTY verify_v153 **28 项全过**（C1 = **0(45)/35(120)/2.418**、B1/B2 = 8(14)/15(31)/1.675 实屏验证） |

### 遗留受限项（当前有效）
- **QA runner fixture 漂移 3 处（移交 QA/specifier 轮，coder 不维护 QA 套件——SKILL 边界）**：① `suite_modify_bt.py` 4F；② `suite_config_template.py` 5F；③ `suite_persistence_config.py` QA-PC-04 1F。沿历轮登记。
- **demo verify_trackers_popup.py T2/T3 共 3F（既有失败，v1.55 轮 stash 基线归因与本轮 diff 无关）**：「T2 弹窗打开」「T3 修改对话框打开」——疑 v1.51 焦点环扩位/沙箱 PTY 时序漂移；移交后续轮修复。本轮判据面 verify_v153.py 28 项全过不受影响。
- **本轮新增登记：verify_size_move.py 陈旧化**——v1.48 时代脚本，其断言自 v1.51 起随显示格式演进失效（旧形态串 `12/35/2.418`/`8/23/1.675`/「分享」/「剩余」均已退役），历轮验证证据未引用（非活跃判据集）；移交后续轮决定修复或退役，本轮不动。
- demo clippy 口径：后续轮一律 --all-targets 零警告口径声明（v1.55 轮已归零并登记）。
- leech 专属 e2e、变异实跑（hardender 未跑）、BT engine/add-bt/anti-leech/trackers-window/cmd-window 族无专属 runner（沿历轮）；`suite_bt_engine` 等六族文件在本仓不存在（非本轮新增缺口）。
- PMD/llvm-cov/cargo-mutants 未装机（无度量需求）。

### 实现定义值登记（累计，当前有效口径）
- **D44–D79 沿历轮登记不变（均不可回退）；D80 本轮闸门结案（操作者 20261114 chat 裁定：「d80的 1-3 都批准，4 的话 EZR-DEMO 中的定义 SEEDS、PEERS 应该和 EZR 保持一致」+ 做种臂样例真实做种形态「要」）**：
  - **D80①（qBt 源核证结论）批准结案（不可回退）**：qBt master 任务列表 SEEDS/PEERS 列取值 = `amountString(seedsCount(), totalSeedsCount())` / `amountString(leechsCount(), totalLeechersCount())`（transferlistmodel.cpp L386-388，amountString = `"%1 (%2)"`）；`seedsCount = num_seeds`（连接种子）、`leechsCount = num_peers − num_seeds`（连接 peers 排除种子）、`totalSeedsCount = num_complete`（tracker 报告，fallback list_seeds）、`totalLeechersCount = num_incomplete`（tracker 报告，fallback list_peers − list_seeds）（torrentimpl.cpp L1488-1517）——操作者引文与源完全一致。
  - **D80②（EZR 对判）批准结案（不可回退）**：x1 = BtConns 快照 seeder==true 计数（连接种子数）已与 qBt seedsCount 同源维持；y1 = conns.len()（含种子，D78③）不同源修订；v1.2 条款括注 qBt 锚点 `peersCount/totalPeersCount` 系误引，更正为 `leechsCount/totalLeechersCount`。
  - **D80③（y1 新口径）批准结案（不可回退）**：y1 = 实际连接的**非种子 peers 数**（leecher，排除种子）= BtConns 快照 seeder==false 连接计数（等价连接 peers 总数 − x1）；x2/y2 括号段、tracker 未知括号段省略（D78②）、非活跃 `-/-/-`、做种臂同构形态均不变。
  - **D80④（demo 承接口径）修订结案（操作者钦定新版，不可回退）**：**EZR-DEMO 中 SEEDS、PEERS 的字段定义与 EZR（FR-02-37 v1.3）保持一致**——Task.seeders = 实际连接种子数（x1）、Task.peers = 实际连接**非种子 peers 数**（y1），显示层直读无换算；v1.55「Task.peers = 连接 peers 总数（含种子）+ 显示层饱和减法」口径退役；产品端实现层裁量不变（入口减/显示减观测一致，**建议入口减与 demo 字段语义对齐**）。

## 二、当前产出情况

### 本会话产物清单（specifier D80 裁决落实与 demo 口径对齐轮）
1. **需求文档修订**：`mission.md` v1.56 注记 + 标题版本；`mission/phase-02.md` v1.24 头注（D80 闸门结案 + ④ 修订结案；**FR-02-37 v1.3 条款/features/QA 零改动**——产品语义不变，APS 双检不适用）。
2. **ezr-demo 字段语义对齐（D80④ 修订落地）**：`app/model.rs` 四字段 doc 注释锁定语义（seeders=x1/tracker_seeds=x2/tracker_peers=y2/peers=y1）；`ui/list.rs` sp_txt 直读（`saturating_sub` 减法退役）+ 文档注释更新；`ui/mod.rs` 三守卫测试同步（做种行断言 0(45)/35(120)/2.418 + sp_txt 直读判据 + 标签版本 v1.3）；`app/dialogs.rs` 添加 BT 对话框假数据口径（peers 直存 y1）；`app/demo.rs` 样例（v[2] 做种臂 seeders 12→0；v[13] 下载臂 peers 23→15 语义直存，观测形态不变）。
3. **Trackers 弹窗假数据锚点**：`ui/popups.rs` draw_trackers 快照扩展（tracker_seeds/tracker_peers），per-tracker「工作中」行种子/Peer 假值改锚 tracker 报告值（fallback 连接值）——做种态 x1=0 不再退化弹窗数据。
4. **做种臂样例真实做种形态（操作者钦定）**：显示 `12(45)/23(120)/2.418` → **`0(45)/35(120)/2.418`**（呈现「做种态 SEEDS=0 而 PEERS 有数」引文口径）；下载臂样例观测形态 `8(14)/15(31)/1.675` 不变。
5. **验证证据**：demo 单测 58P/0F（nightly）、clippy --all-targets 0 警告（nightly）、PTY verify_v153.py 28 项全过（C1 = 0(45)/35(120)/2.418、B1/B2 = 8(14)/15(31)/1.675 实屏）。
6. **docs 设计规范** v1.56 同步补记（四项：字段对齐/样例形态/弹窗锚点/判据同步）+ §5.2 做种臂示例更新（含行 3 形态注记）。
7. **经验沉淀**：`project/notes/` testing.md 本轮三条（字段语义即显示语义判据/共享显示面联动排查/样例数据语义化）+ INDEX.md 同步；无跨项目通用条目。
8. **本 handoff.md** 重写为本轮口径。

### 对账口径
- 本轮改动面 = mission.md + mission/phase-02.md + ezr-demo 六文件（model.rs/list.rs/mod.rs/popups.rs/dialogs.rs/demo.rs）+ verify_v153.py + docs 设计规范 + notes 两文件 + handoff；产品端 `project/ezr/src/` **零改动**（coder 只读纪律，产品实现归下一 coder 轮）。
- 观测行为对账：下载臂样例显示值不变（8(14)/15(31)——peers 字段 23→15 为语义直存）；做种臂样例显示值按操作者钦定变更（12(45)/23(120) → 0(45)/35(120)）；Trackers 弹窗做种任务 per-tracker 假值从连接锚改 tracker 报告锚（~41-45 种子，原连接锚将为 0/1）；其余显示面零变化。
- 规格与 QA 零修订（FR-02-37 v1.3 条款不变）→ APS 双检不适用，无 IR/dry 产物。

### 待办与待批
- **D80①–④ 全部结案（本轮闸门关闭），无待批项。**
- **产品端 ezr 实现移交下一 coder 轮**（FR-02-37 v1.3 + D80④ 对齐建议）：y1 = BtConns 快照 seeder==false 计数——**建议入口减**（evt.rs `Evt::BtConns` 注入时 `t.peers = seeder==false 计数`，与 demo 字段语义完全对齐），显示减亦可（观测行为一致，coder 裁量）；单测同步点 = task_lines.rs sp_txt 判据 + evt.rs BtConns 注入口径；ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35，本轮不涉持久化格式）。
- verify_size_move.py 陈旧化处置（修复或退役）+ QA runner 漂移 3 处 + verify_trackers_popup T2/T3 3F 移交后续轮（见受限项）。
- 变异实跑维持移交 hardender；leech e2e 缺口沿历轮。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 下一节点建议 = `bin/swarm begin six-pack/coder`（产品实现轮：FR-02-37 v1.3 y1 排除种子落地 + evt.rs peers 口径入口减对齐 + 单测同步）。
