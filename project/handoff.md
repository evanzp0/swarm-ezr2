# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（FR-01-81 修订二实现·并发分块明细表移除）　会话：coder-20261002d

## 一、产物历史完成情况

### 项目与需求

- EZR Downloader（ezr download）：TUI 高性能多协议下载器，正式版一期。
- 需求权威来源：`project/mission.md`（v1.1 大纲）+ `project/mission/phase-01.md`（v1.2 详述）。

### 产物总账

| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier / coder / cleaner 组 / QA（首轮至四轮） | 详见历史归档 `swarm-20261002-062546` ~ `091047` | 已交付 | 208 测全绿基线 |
| coder（缺陷修复） | 明细表重复表头 + 闪烁两缺陷修复 | 已交付 | 210 测全绿 |
| specifier（三轮修订） | FR-01-81 修订二：并发分块明细表整体移除（三层同步，决策点 1-5 登记） | 已交付 | 归档 specifier-20261002c |
| coder（三轮修订实现，本会话） | 明细表 UI 段落 + 连接级展示机制整体移除，thread_count 数据面口径 | 已交付 | 206 测全绿、clippy 0、fmt 清零 |

## 二、当前产出情况（coder → 移交）

### 实现清单（对应 FR-01-81 修订二决策点 1-5）

- **`ui/detail.rs`**：删「并发分块明细」渲染段（分节标题/表头/行）与 `conn_status` /
  `chunk_col_text` / `conn_row` / `conn_table` 四函数及死导入（pad_left/DIM2）；详情面板止于
  任务级字段行（决策点 3：「分块 x/y · N/块」行保留）。
- **`app/mod.rs` + `app/engine.rs`**：删 `conn_windows` / `conn_speed_display` 字段与初始化、
  tick 第 5 步连接采样与归零连接段、Progress 每连接滑窗 push 与重建回填段、7 处生命周期
  retain（engine ×5、dialogs ×1、tasks ×1）（决策点 1）。任务级平滑（speed_display/windows/
  last_speed_tick/零值速断）零改动。
- **`model/task.rs`**：删 `Connection.speed` 字段与 `frac()` 方法；`thread_count()` 下载中分支
  改数据面口径（`cap()>0 && done<cap()`，决策点 2；头部"并发线程"语义不变）。
- **`engine/mod.rs` / `chunk/lease.rs` / `ui/text.rs`**：`to_connection`、`lease_snapshot` 去速度
  字段；`pad_left` 工具及专属断言随最后消费者一并移除。

### 测试对账（实测）

| 验证项 | 结果 | 命令（project/ezr 下可复现） |
|---|---|---|
| 单元测试 | **206 P / 0 F**（165+30+11） | `cargo test` |
| clippy 全目标（nightly） | 0 警告（死代码兜底网清零） | `cargo clippy --all-targets` |
| rustfmt（nightly） | 0 差异 | `cargo fmt --check` |
| 对账公式 | 210 − 6（4 连接级 tick 测 + 1 UI 明细表测 + 1 frac 测）+ 2（`draw_detail_omits_conn_table` 负向渲染测、`thread_counts_active_conns_by_data_plane` 口径测）= 206 ✓ | — |
| 数据面无回归 | stub 全流程下载测仍绿（删除中误吞的 `t.downloaded` 写回已即时发现修复） | `cargo test --bin ezr tick_completes` |

### 对账口径

- 与 specifier 交接决策点 1-5 逐项对应落地；任务级 FR-01-17 平滑行为与既有测断言零改动。
- 预期对账数字（specifier 估计"约 204"）与实测 206 的偏差原因：specifier 估计未计入新增的
  2 个守卫测试；以实测为准。
- 决策点 5（runner 同步）未实现，属 QA 节点边界。

### 待办与待批（未决项，承前不变）

1. **QA-TD-15 端到端 runner 实现 + 9 套 runner 整体回归**（QA 节点；含本次修订失效用例
   清理——suite_display 中明细表断言用例删除/改写）。
2. CRAP >6 残余登记口径（architect 裁决）。

### 移交建议

- 下一步建议 **six-pack/qa**：runner 同步（删明细表用例）+ QA-TD-15 实现 + 整体回归一并处理。
- 收尾：`./bin/swarm complete`（target/ 由 complete 自动清理）。
