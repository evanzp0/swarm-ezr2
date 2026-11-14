# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（FR-02-37 v1.3 产品实现轮，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner/architect/QA 收敛链 ⇒ specifier 显示指标/调整/裁决/新增需求轮（v1.47–v1.53）⇒ specifier 裁决落实修订轮（v1.54）⇒ coder 产品实现/复核重验轮 ⇒ specifier PEERS/SEEDS 核证修订轮（v1.55）⇒ specifier D80 裁决落实轮（v1.56）⇒ **coder FR-02-37 v1.3 产品实现轮（本轮 2026-11-14，需求 = v1.55/v1.56 交接待办；下一步建议 six-pack/cleaner）**
> 会话：six-pack-coder-20261114（任务 = FR-02-37 v1.3 y1 排除种子落地 + evt.rs peers 口径入口减对齐 + 单测同步；上游交接 = project/handoff.md 核证修订轮版）
> 环境：stable 1.100.0 + nightly 1.102.0-nightly 双工具链本轮经 rsproxy 直链前台重装（rustup-init.sh 脚本路径 404，直链二进制 + DIST_SERVER/UPDATE_ROOT 双变量）；~/.cargo/config.toml rsproxy sparse 本轮新配（crates.io 索引）；磁盘 9.9G 受限，测试分批实跑（批间清 build out/incremental）。APS/bb/PMD 未装机——本轮规格与 QA 零改动（APS 双检不适用）、无度量任务，后续轮按需重建（受限项登记）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.56**）+ `mission/` 两份 phase 详述（**phase-01 v1.53**、**phase-02 v1.24**）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-13 QA 观察项清理轮） | ezr 产品实现 + 15 套件 e2e + 收敛/终验链 | 已交付 | 历轮对账在案 |
| specifier 新增需求轮（2026-11-13，v1.53） | mission v1.53 + phase 详述 + features 六份修订 + QA 六份同步 + ezr-demo 同构落地 | 已交付 | APS 双检 6/6；demo 单测 58P/0F、PTY 29 项全过 |
| specifier 裁决落实修订轮（2026-11-13，v1.54） | mission v1.54 + phase-01 v1.53/phase-02 v1.22 + features 三份修订 + QA 三份同步 + ezr-demo HTTP 臂 LT 同构 | 已交付 | APS 双检 3/3；demo 单测 58P/0F、PTY 28 项全过、clippy 零警告 |
| coder 产品实现轮（2026-11-13） | 产品端五项全量落地（FR-01-118/FR-02-37 v1.2/FR-02-38 v1.21+22/FR-02-39/FR-02-40 + help SR/LT + 模型层 tracker/连接值分离） | 已交付 | 单测 8100P/0F、clippy --all-targets 0 警告、fmt 过 |
| coder 复核重验轮（2026-11-14） | 产品端零代码新增；tests/gen 剥头副本 4 份机械重生成 + notes 沉淀 + handoff 重写 | 已交付 | 单测 8100P/0F（13 目标逐目标实测）、clippy 0 警告、fmt 全量合规 |
| specifier PEERS/SEEDS 核证修订轮（2026-11-14，v1.55） | mission v1.55 + phase-02 v1.23（FR-02-37 v1.3、D80①–④）+ features/09-bt-engine v1.53（场景 10/21）+ QA-BE-10/21 v1.53 + ezr-demo y1 排除种子同构落地 + notes + handoff 重写 | 已交付 | APS 双检通过；demo 单测 58P/0F（nightly）、clippy --all-targets 0 警告、PTY verify_v153 28 项全过 |
| specifier D80 裁决落实轮（2026-11-14，v1.56） | mission v1.56 + phase-02 v1.24（D80①②③ 批准结案、④ 修订结案）+ ezr-demo 字段语义对齐（seeders=x1/peers=y1 直存直读）+ 做种臂样例真实做种形态 0(45)/35(120)/2.418 + Trackers 弹窗假数据锚点 + 守卫测试 + verify_v153 判据 + docs 设计规范 v1.56 补记 + notes 两文件 + handoff 重写 | 已交付 | demo 单测 58P/0F（nightly）、clippy --all-targets 0 警告（nightly）、PTY verify_v153 28 项全过（C1 = 0(45)/35(120)/2.418、B1/B2 = 8(14)/15(31)/1.675 实屏验证） |
| **coder FR-02-37 v1.3 产品实现轮（本轮 2026-11-14）** | **产品端入口减落地（evt.rs BtConns 臂 y1 = seeder==false 计数直存）+ 单测同步（evt.rs 判据翻转 + 纯种子 y1=0 断言；task_lines.rs sp_txt 判据注释 v1.3 口径）+ clippy 机械修复两处（nightly lint 漂移）+ notes 三文件追加 + 本 handoff 重写** | 已交付 | 单测 **8146P/0F**（16 目标逐目标实测；13 目标 coder 基线口径 **8100P** 与历轮基线总数精确吻合）、clippy **--all-targets 0 警告 0 错误**、fmt **--check 全量合规** |

### 遗留受限项（当前有效）
- **全新沙箱工具缓存空仓（本轮登记）**：`.tools/` APS/bb 未装机——本轮规格与 QA 零改动（FR-02-37 v1.3 条款不变），APS 双检不适用；后续涉及规格修订/度量任务的轮次按 engineering.md 重建（装机路径与口径见 notes/environment.md 历轮记录）。
- **QA runner fixture 漂移 3 处（移交 QA/specifier 轮，coder 不维护 QA 套件——SKILL 边界）**：① `suite_modify_bt.py` 4F；② `suite_config_template.py` 5F；③ `suite_persistence_config.py` QA-PC-04 1F。沿历轮登记。
- **verify_trackers_popup.py T2/T3 共 3F（既有失败，v1.55 轮 stash 基线归因与本轮 diff 无关）**：「T2 弹窗打开」「T3 修改对话框打开」——移交后续轮修复。
- **verify_size_move.py 陈旧化**——v1.48 时代脚本，断言自 v1.51 起随显示格式演进失效；移交后续轮决定修复或退役。
- demo clippy 口径：后续轮一律 --all-targets 零警告口径声明。
- leech 专属 e2e、变异实跑（hardender 未跑）、BT engine/add-bt/anti-leech/trackers-window/cmd-window 族无专属 runner（沿历轮）；`suite_bt_engine` 等六族文件在本仓不存在（非新增缺口）。
- PMD/llvm-cov/cargo-mutants 未装机（无度量需求）。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退）；D80①–④ 已全部结案（2026-11-14 操作者闸门裁定）**：
  - **D80①（qBt 源核证结论）批准结案（不可回退）**：qBt master 任务列表 SEEDS/PEERS 列取值 = `amountString(seedsCount(), totalSeedsCount())` / `amountString(leechsCount(), totalLeechersCount())`（transferlistmodel.cpp L386-388，amountString = `"%1 (%2)"`）；`seedsCount = num_seeds`（连接种子）、`leechsCount = num_peers − num_seeds`、`totalSeedsCount = num_complete`、`totalLeechersCount = num_incomplete`（torrentimpl.cpp L1488-1517）。
  - **D80②（EZR 对判）批准结案（不可回退）**：x1 = BtConns 快照 seeder==true 计数与 qBt seedsCount 同源维持；v1.2 条款括注 qBt 锚点 `peersCount/totalPeersCount` 系误引，更正为 `leechsCount/totalLeechersCount`。
  - **D80③（y1 新口径）批准结案（不可回退）**：y1 = 实际连接的**非种子 peers 数**（leecher，排除种子）= BtConns 快照 seeder==false 连接计数；x2/y2 括号段、tracker 未知括号段省略（D78②）、非活跃 `-/-/-`、做种臂同构形态均不变。**本轮产品端落地（入口减）。**
  - **D80④（demo 承接口径）修订结案（操作者钦定新版，不可回退）**：EZR-DEMO 中 SEEDS、PEERS 的字段定义与 EZR（FR-02-37 v1.3）保持一致——Task.seeders = x1、Task.peers = y1，显示层直读无换算；v1.55「含种子 + 饱和减法」口径退役。**产品实现层入口减与 demo 字段语义已对齐（本轮）。**

## 二、当前产出情况

### 本会话产物清单（coder FR-02-37 v1.3 产品实现轮）
1. **产品端入口减落地**：`project/ezr/src/app/engine/evt.rs` Evt::BtConns 臂一行改动——`t.peers = conns.iter().filter(|c| !c.seeder).count()`（原 `conns.len()` 含种子口径退役）；与 demo Task.peers 字段语义完全对齐（D80④）。显示层 `ui/task_lines.rs` sp_txt 直读**零改动**。
2. **单测同步（交接钦定两同步点）**：
   - `evt.rs` `bt_conns_arm_counts_connected_seeds_and_peers`：判据翻转 y1 = 2（1 种子 + 2 leecher 混合快照，旧实现 3 实测变红）+ 新增纯种子连接二次注入断言（x1=2、y1=0——做种态 SEEDS 有数而 PEERS 无数同构呈现）；
   - `task_lines.rs` `sp_txt_bracket_form_uses_tracker_values` 判据注释同步 v1.3/D80③ 口径（y1 = 非种子 peers 数由注入层直存、显示直读）。
3. **clippy 机械修复**：`src/app/bt_dialog.rs` 两处测试断言（`.contains(&"…".to_string())` → `iter().any(|i| i == "…")`）——nightly 2026-11-14 构建 `unnecessary_to_owned` lint 覆盖面漂移暴露的既有代码告警，非本轮改动引入；不修复则 --all-targets 0 警告门槛失守。
4. **经验沉淀**：`project/notes/` 三文件追加（architecture：BtConns 臂唯一注入点与消费面普查判据；testing：16 目标口径对账 / nightly lint 漂移实录 / ENOSPC build out 清理法；environment：沙箱镜像双配置 / nightly 残骸愈法 / 镜像复测）+ INDEX.md 同步；无跨项目通用条目。
5. **本 handoff.md** 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 单测（ezr bin） | 1040P / 0F | cargo test --bins（含 fixture 30P/proxy 15P/mktorrent 1P） |
| 单测（12 hardening 集成目标） | 7060P / 0F | cargo test --test <目标> 逐目标实测（hardening 247 / engine_backfill 928 / g_appcore 814 / g_engine 810 / g_main 2 / g_proxy 16 / g_slots 254 / g_supervisor 783 / g_throttle_err 500 / g_v147 797 / ui_backfill 958 / v115 951） |
| 合计 | 8146P / 0F（16 目标）；13 目标基线口径 8100P 与历轮基线精确吻合 | 分项复算自洽 |
| clippy | --all-targets 0 警告 0 错误 | cargo clippy --all-targets |
| fmt | 全量合规 | cargo fmt --all -- --check |
| TDD 红→绿 | 旧实现 y1=3 实测变红 → 入口减后 7/7 evt 臂全绿 | cargo test --bin ezr evt_arm_tests:: |

### 对账口径
- 本轮改动面 = `evt.rs`（注入臂一行 + 单测扩展）+ `task_lines.rs`（注释一行）+ `bt_dialog.rs`（两处测试断言机械改写）+ notes 三文件 + handoff；产品行为面 = BT 任务 y1 取值语义（BtConns 注入时排除种子）。
- 观测行为对账：下载中/做种中 BT 任务的 y1（括号外 PEERS）从「连接 peers 总数（含种子）」变为「非种子连接数」；x1/x2/y2、括号省略、`-/-/-`、做种臂同构、HTTP 面零变化；`t.peers` 无其他计算消费（Trackers 弹窗产品端直用 tracker 报告行、无连接值 fallback；tick.rs 退出规则仅夹具用值）——入口减零波及。
- 兼容口径：ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）；本轮为取值逻辑修订，不涉持久化格式。
- 磁盘受限说明：单测 16 目标分批前台实跑（批间清理 `target/debug/build/*/*/out` 与 `debug/incremental`），批间清理仅移除已完成目标的可重建产物，不影响结果口径；debug 产物（target/）由 `bin/swarm complete` 打包前自动清理。

### 待办与待批
- **无待批项**（D80 全结案；本轮交接钦定实现完成）。
- verify_size_move.py 陈旧化处置（修复或退役）+ QA runner 漂移 3 处 + verify_trackers_popup T2/T3 3F 移交后续轮（见受限项）。
- 变异实跑维持移交 hardender；leech e2e 缺口沿历轮。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 下一节点建议 = `bin/swarm begin six-pack/cleaner`（收敛轮：本轮改动面小且 clippy/fmt 已零基线，cleaner 可做例行收敛或操作者直接裁定终结本轮流程）。
