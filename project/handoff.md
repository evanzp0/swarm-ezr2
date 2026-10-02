# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（FR-01-17 修订实现·速度展示平滑）　会话：coder-20261002

## 一、产物历史完成情况

### 项目与需求

- EZR Downloader（ezr download）：TUI 高性能多协议下载器，正式版一期。
- 需求权威来源：`project/mission.md`（v1.1 大纲）+ `project/mission/phase-01.md`（v1.2 详述）。

### 产物总账

| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier | `features/` 9 份 Gherkin（114 场景）+ `qa/` 9 份规程 | 已交付 | 三层一致 |
| coder | `project/ezr/` 下载内核+TUI+CLI+fixture | 已交付（P0+P1 全量） | 100 单测、clippy 0 警告 |
| cleaner/architect/hardender（首轮） | 清理/架构/加固 | 已交付 | 归档在案 |
| QA | `project/qa/runners/` 9 套可执行套件（114 用例）+ 缺陷修复 12 处 | 已交付 | 110 P + 4 S / 0 F |
| cleaner（二次） | QA 修复后回归清理 | 已交付 | 112 P / 0 F、覆盖率 49.0% |
| cleaner（三次） | 遗留 4 项攻坚（rustfmt/CRAP/变异点/流式化） | 已交付 | 202 测全绿、覆盖率 83.4% |
| cleaner（四次） | rustfmt 切 nightly + unstable 选项全量生效 + rcgen 死依赖清除 | 已交付 | 202 测全绿、clippy 0、fmt 清零 |
| specifier | FR-01-17 修订：速度展示面平滑（三层一致同步） | 已交付 | 归档 `swarm-20261002-075852-six-pack-specifier.tar.gz` |
| coder（本会话） | FR-01-17 修订实现 + 场景 15 行为 | 已交付 | 205 测全绿（+3 纯追加）、clippy 0、fmt 清零 |

## 二、当前产出情况（coder → 移交）

### 本会话产物清单（对应 FR-01-17 修订与场景 01-tui-display-15）

- **`model/speed.rs` 新增 `SmoothedSpeed`**：展示面 EMA 平滑器（α=1/5，等效约 5 秒窗口，
  libtorrent second_tick 同款口径）；`push(sample)` 收敛 1/5、`zero()` 立即归零、
  `value()` 取展示值。数据面 `SpeedWindow` 原样保留。
- **TDD 3 个新单测**（纯追加）：首采样为 α 份额（防恒等直传）、恒定输入 40 步收敛 ±2%
  （防固定偏置）、zero 后精确归零（防自然衰减拖尾）。
- **`app/mod.rs` 状态接线**：`speed_display: HashMap<u32, SmoothedSpeed>`（生命周期与
  windows 一致，幽灵进度清理点同步 remove）+ `last_speed_tick` 节拍锚点。
- **`app/engine.rs` tick 第 5 步重写**：
  - 非下载态（排队/暂停/校验/失败/完成）每 tick **立即归零**（零值速断，不走 EMA 衰减），
    头部全局速度自动同步扣除（全局 = Σ 任务展示值）；
  - 下载态按 `SPEED_TICK = 1s` 门控采样窗口 rate → EMA → `t.speed`——数值每秒最多变化
    一次，UI 重绘（100ms）与数值更新解耦；
  - `push_hist` 移入 1s 门控——Sparkline 每秒 1 点、180 点 = 180 秒（修复原 100ms 压点
    导致的 18 秒窗口缺陷）；
  - `session_bytes` 会话累计改按展示值积分（分段常数，口径平滑一致）。
- **范围边界**：连接级速度（FR-01-81）与 ETA 派生逻辑未动（ETA 现随展示值每秒刷新）。

### 验证证据

| 验证项 | 结果 | 命令（project/ezr 下可复现） |
|---|---|---|
| 单元测试 | 205 P / 0 F（164+30+11；+3 纯追加） | `cargo test` |
| clippy 全目标（nightly） | 0 警告 | `cargo clippy --all-targets` |
| rustfmt（nightly） | 0 差异 | `cargo fmt --check` |
| 行为切片 | EMA 数学 3 测锁定；接线点人工核验（tick 第 5 步/幽灵清理/构造器） | `cargo test --bin ezr speed::` |

### 对账口径（与 specifier 交接）

- 验收口径 4 项中 1-3 已达成（单测/clippy/fmt）；第 4 项 QA-TD-15 端到端 runner 属 QA
  节点职责，未实现（coder 边界）。
- 既有 202 测断言零改动；平滑仅影响展示面，数据面（窗口/节流/限速）路径未触碰。

### 待办与待批

1. CRAP >6 残余登记口径（前会话遗留，architect 裁决）。
2. QA-TD-15 的端到端 runner 实现（QA 节点；fixture 慢速门控文件沿用既有约定）。
3. 连接级速度是否同步平滑——操作者如需请另行指示（FR-01-81 范围）。

### 移交建议

- 下一步建议 **six-pack/cleaner**（本轮新增代码的回归清理与指标复核：变异点/CRAP/
  覆盖率在新代码上的分布；SmoothedSpeed 已按可测模块放置，预计无拆分需求）。
- 亦可由操作者裁决直接进入 architect（CRAP 残余裁决一并处理）。
- 收尾：`./bin/swarm complete`（构建缓存 target/ 由 complete 自动清理）。
