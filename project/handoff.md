# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（FR-01-81 修订实现·连接级速度展示同拍平滑）　会话：coder-20261002b

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
| coder（二轮修订，本会话） | FR-01-81 修订实现 + 场景 16 行为 | 已交付 | 208 测全绿（+3 纯追加）、clippy 0、fmt 清零 |

## 二、当前产出情况（coder → 移交）

### 本会话产物清单（对应 FR-01-81 修订与场景 01-tui-display-16）

- **`app/mod.rs` 新增 `conn_speed_display: HashMap<(u32, usize), SmoothedSpeed>`**：连接级
  展示面平滑值容器（FR-01-17 修订/FR-01-81：与任务级同拍同口径）；生命周期与
  `conn_windows` 一致；构造器同步初始化。
- **`app/engine.rs` Progress 事件调整**：每连接数据面仅保留 1s 滑窗累计字节 push（速率
  基准不变）；**移除 `tc.speed` 裸写**——展示值统一由 tick 第 5 步按节拍写出。
- **`app/engine.rs` tick 第 5 步扩展**：
  - 零值速断扩展到连接级：任务非下载态时连接行 `c.speed` 立即归零、其全部连接展示值
    `zero()`（EMA 无拖尾）；暂停态明细表由快照重建、速度天然为 0；
  - 下载态同一 `last_speed_tick` 节拍内：任务级采样后逐连接采样 `conn_windows` rate →
    各自 EMA push → `c.speed`——明细表速度列与任务速度**同拍**（每秒最多变化一次）；
  - 状态列「传输中/挂起」与速度数字**同源**（`conn_status` 读 `c.speed` 即平滑值，
    `ui/detail.rs` 无需改动）。
- **7 个生命周期清理点镜像 retain**（与 conn_windows 逐点对照）：engine.rs Progress 幽灵 /
  PausedDone / 续传一致性失效 / Completed / handle_failure 尾，dialogs.rs 删除任务，
  tasks.rs Start 重启。
- **TDD 3 个新单测**（tick_tests，纯追加）：同拍平滑接线（连接值随节拍写出、首拍 α 份额
  非裸传、1s 内不变）；非下载态连接行立即归零；归零后恢复从 0 平滑爬升（防冻结旧值）。

### 验证证据

| 验证项 | 结果 | 命令（project/ezr 下可复现） |
|---|---|---|
| 单元测试 | 208 P / 0 F（167+30+11；+3 纯追加） | `cargo test` |
| clippy 全目标（nightly） | 0 警告 | `cargo clippy --all-targets` |
| rustfmt（nightly） | 0 差异 | `cargo fmt --check` |
| 行为切片 | 连接级接线 3 测锁定；7 个生命周期点人工对照 conn_windows 逐一核验 | `cargo test --bin ezr tick_` |
| 数据面无回归 | stub 全流程下载完成测仍绿（Progress 移除裸写后传输路径不变） | `cargo test --bin ezr tick_completes` |

### 对账口径（与 specifier 交接）

- 验收口径 4 项中 1-3 已达成（单测/clippy/fmt）；第 4 项 QA-TD-16 端到端 runner 属 QA
  节点职责，未实现（coder 边界）。
- 既有 205 测断言零改动；本修订仅影响展示面，数据面（每连接 1s 滑窗/节流/限速）路径
  未触碰； specifier 关键决策点 ①状态列同源（取平滑值）②同拍共用节拍锚点 ③场景 15
  fixture 参数修正，均按规格落地（操作者可否决，否决后按规格回滚口径）。

### 待办与待批（未决项）

1. QA-TD-15 / QA-TD-16 端到端 runner 实现（QA 节点；fixture 慢速门控沿用既有约定
   `?speed=B/s&gate=N`；场景 16 需并发 4 + 终端高度足以渲染明细表）。
2. QA 9 套 runner 随两轮平滑修订的整体回归（QA 会话统一安排）。
3. CRAP >6 残余登记口径（architect 裁决）。

### 移交建议

- 下一步建议 **six-pack/cleaner**：本轮新增代码的回归清理与指标复核（变异点/CRAP/覆盖率
  在新代码上的分布；conn_speed_display 已按可测模块放置，预计无拆分需求）。
- 亦可由操作者裁决先起 **six-pack/qa**（runner 实现与两轮修订的整体回归一并处理）。
- 收尾：`./bin/swarm complete`（构建缓存 target/ 由 complete 自动清理）。
