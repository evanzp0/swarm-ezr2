# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（缺陷修复·明细表重复表头与闪烁）　会话：coder-20261002c

## 一、产物历史完成情况

### 项目与需求

- EZR Downloader（ezr download）：TUI 高性能多协议下载器，正式版一期。
- 需求权威来源：`project/mission.md`（v1.1 大纲）+ `project/mission/phase-01.md`（v1.2 详述）。

### 产物总账

| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier | `features/` 9 份 Gherkin（115 场景）+ `qa/` 9 份规程 | 已交付 | 三层一致 |
| coder | `project/ezr/` 下载内核+TUI+CLI+fixture | 已交付（P0+P1 全量） | 100 单测、clippy 0 警告 |
| cleaner/architect/hardender（首轮） | 清理/架构/加固 | 已交付 | 归档在案 |
| QA | `project/qa/runners/` 9 套可执行套件（114 用例）+ 缺陷修复 12 处 | 已交付 | 110 P + 4 S / 0 F |
| cleaner（二次） | QA 修复后回归清理 | 已交付 | 112 P / 0 F、覆盖率 49.0% |
| cleaner（三次） | 遗留 4 项攻坚（rustfmt/CRAP/变异点/流式化） | 已交付 | 202 测全绿、覆盖率 83.4% |
| cleaner（四次） | rustfmt 切 nightly + unstable 选项全量生效 + rcgen 死依赖清除 | 已交付 | 202 测全绿、clippy 0、fmt 清零 |
| specifier（一轮修订） | FR-01-17 修订：任务级/全局速度展示面平滑（三层一致同步） | 已交付 | 归档 `swarm-20261002-075852` |
| coder（一轮修订） | FR-01-17 修订实现（SmoothedSpeed + tick 第 5 步重写） | 已交付 | 205 测全绿；归档 `swarm-20261002-080622` |
| specifier（二轮修订） | FR-01-81 范围修订：连接级速度展示与任务级同拍平滑（三层一致同步） | 已交付 | 三层一致自查全绿；归档 `swarm-20261002-085533` |
| coder（二轮修订） | FR-01-81 修订实现 + 场景 16 行为 | 已交付 | 208 测全绿；归档 `swarm-20261002-091047` |
| coder（缺陷修复，本会话） | 明细表重复表头 + 高频闪烁两缺陷修复（TDD +2 测） | 已交付 | 210 测全绿（+2 纯追加）、clippy 0、fmt 清零 |

## 二、当前产出情况（coder → 移交）

### 缺陷报告（操作者 chat 输入，任务 bugfix-conn-detail-table）

任务详情「并发分块明细」表：①表头重复渲染 2 行；②明细行整体高频闪烁（"一闪一闪"）。

### 根因与修法

1. **表头重复（陈年缺陷，最早归档即存在）**：`ui/detail.rs` `draw_detail` 在分节标题下推入
   一次表头，而 `conn_table()` 内部自带同一表头——每次渲染出现两行。
   **修法**：移除 `draw_detail` 侧表头推入，表头唯一来源回归 `conn_table`（其 doc 注释
   「表头 + 至多 rows_avail 行」即契约）；`rows_avail` 预算不变（+1 恰为 conn_table 表头行），
   面板行密度与修前一致，明细表可视行数 +1。
2. **明细表闪烁（FR-01-81 二轮修订引入的交互缺陷）**：`Evt::Progress`（~10Hz）每次整表重建
   `t.connections`，重建视图速度默认 0（`ConnView::to_connection` 无速度字段）；而连接级
   平滑展示值仅由 tick 第 5 步按 1Hz 节拍写出 → 节拍间隔内展示值被重建清零，明细表速度列与
   状态列在「数值/传输中」与「—/挂起」间高频交替。任务级不受影响（Progress 不碰 `t.speed`），
   故当时未被发现。**修法**：Progress 重建连接视图后，从 `conn_speed_display` 回填各连接
   平滑展示值（新连接无条目保持 0，下拍起写）——`c.speed` 作为展示面唯一数据源的
   FR-01-81 口径不变，状态列同源判定不受影响。

### 本会话改动清单

- `project/ezr/src/ui/detail.rs`：`draw_detail` 移除重复表头推入（-12 行），注释写明唯一来源。
- `project/ezr/src/app/engine.rs`：Progress 处理加展示值回填（+9 行），注释写明闪烁机制。
- TDD 纯追加 2 测：`tick_tests::progress_rebuild_keeps_smoothed_connection_speeds`
  （红：重建后 0.0 ≠ EMA 12000.0）；`ui_tests::draw_conn_table_header_renders_once`
  （红：表头锚点 ×2 ≠ ×1）。既有 208 测断言零改动。

### 验证证据

| 验证项 | 结果 | 命令（project/ezr 下可复现） |
|---|---|---|
| 单元测试 | 210 P / 0 F（169+30+11；+2 纯追加） | `cargo test` |
| clippy 全目标（nightly） | 0 警告 | `cargo clippy --all-targets` |
| rustfmt（nightly） | 0 差异 | `cargo fmt --check` |
| 红绿证据 | 两测先红（数值精确复现两症状）后绿 | `cargo test --bin ezr -- progress_rebuild draw_conn_table` |

### 对账口径

- 本轮为缺陷修复（展示不符既有规格 FR-01-81），非行为变更，未动 `features/` 与
  `mission/phase-01.md`；specifier 修订口径（同拍平滑、零值速断、状态列同源、唯一数据源）
  全部保持。
- 计划估计「+2 = 210 P」与实测一致；测试分布实测 169+30+11（基线同分布 +2 于 lib）。
- 行为切片锁定：重建回填 1 测（数据面）、表头唯一 1 测（渲染面）；无 UI 视觉回归
  （行密度不变）。

### 待办与待批（未决项，承前不变）

1. QA-TD-15 / QA-TD-16 端到端 runner 实现（QA 节点；fixture 慢速门控沿用既有约定
   `?speed=B/s&gate=N`；场景 16 需并发 4 + 终端高度足以渲染明细表）。
2. QA 9 套 runner 随两轮平滑修订与本轮缺陷修复的整体回归（QA 会话统一安排）。
3. CRAP >6 残余登记口径（architect 裁决）。

### 移交建议

- 下一步建议 **six-pack/qa**：runner 实现（QA-TD-15/16）与整体回归一并处理——本轮两缺陷
  恰属 TUI 显示域，正可作为 runner 的回归靶标。
- 亦可先起 **six-pack/cleaner** 复核新增代码指标（改动极小，预计无清理面）。
- 收尾：`./bin/swarm complete`（构建缓存 target/ 由 complete 自动清理）。
