# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner（FR-01-81 修订二交付后的保持行为清理批次）　会话：cleaner-20261003

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
| coder（三轮修订实现） | 明细表 UI 段落 + 连接级展示机制整体移除，thread_count 数据面口径 | 已交付 | 206 测全绿、clippy 0、fmt 清零 |
| cleaner（本轮，清理批次） | 保持行为清理 3 处 + 覆盖率/CRAP/DRY/变异点四项度量与补测 | 已交付 | 221 测全绿、clippy 0、fmt 0、CPD 余 3 块（登记）、变异点无文件 >100 |

## 二、当前产出情况（cleaner → 移交）

### 清理清单（保持行为不变）

- **`app/engine.rs`**：`tick()` 4a/4b 两段重复的「补发 Start + 清 made_progress」提取为
  `send_retry_start(&mut self, id)` 私有辅助（决策点 4a 到点重试与 4b 重排补发共用，
  未知 id 静默跳过，行为不变）。
- **`ui/detail.rs`**：三处重复的「pad_right(9) + DIM 前景」标签 Span 构造收口为
  `dim_label(s)` 模块级辅助（speed_row / draw_detail / chunk_row 共 14 调用点）；
  过期注释修正——分块行注释「块大小按协议写死」→「按协议默认值，可经配置
  block_size_http 调整」（mission v1.1 §4/D13 依据，注释级修正、零行为面）。
- **测试 fixture DRY**（cleaner 职责内的测试基建清理）：`tick_tests` 内联桩服务器提取为
  共享 `stub_server(len)`（既有 tick_completes 同步改用，响应字节级不变）；
  stub 完成断言提取为 `pump_to_completed`（3 调用点）；`dialog_key_tests` 新增 `del_dlg`
  fixture。PMD CPD 首轮 5 块 → 修掉 2 块（本轮自产），余 3 块见「受限项与登记」。

### 覆盖率与补测（纯追加，206 → 221）

- 覆盖率（cargo llvm-cov 0.9.1）：行覆盖 **84.7% → 88.8%**；零覆盖函数 122 → 84
  （余量集中于 main.rs 入口壳、fixture/proxy 辅助 bin、async 引擎内部——均为环境耦合边界）。
- 纯追加 15 个测试（不改产品行为面、不改构建配置），对账公式：**206 + 15 = 221 ✓**（180+30+11）：
  1. tick 4a 重试到点补发（retry_expiry_resends_start_and_counts_round）
  2. tick 4b 重排任务补发 Start 全流程（requeued_probed_task_gets_restart_from_slot）
  3. push_hist 窗口上限 + toast 过期（push_hist_caps_length_and_toast_expires）
  4. 任务操作状态机：暂停/继续双臂、槽位满排队、手动重试重置、重校验占槽、清理已完成
     （toggle_pause_retry_and_clear_state_flows）
  5. 全局键盘路由臂（global_key_router_arms）
  6. 鼠标：点击选中/滚轮钳制/下拉点选（mouse_click_scroll_and_dialog_hits）
  7. 确认添加校验链：空 URL/非法协议/校验码位数/重复任务/取消按钮
     （confirm_add_validates_then_creates）
  8. 断点自动接续 FR-01-26（confirm_add_resumes_from_existing_sidecar）
  9. 对话框键盘导航 + Delete 三选激活（dialog_key_navigates_and_activates_delete）
  10. ConnView→Connection 映射锁定（to_connection_maps_progress_fields，coder 修订二适配面）
  11. 校验行四态 + 超长校验码截断（draw_detail_checksum_prefix_and_verify_states）
  12. 列表内移动（move_task_swaps_adjacent_and_clamps）
  13. completed_of 完成块计数（completed_of_counts_full_blocks_only）
  14. network/fatal 构造器（network_and_fatal_constructors）
  15. config 空白/非法值回退（empty_dir_and_invalid_numbers_fall_back）

### CRAP / DRY / 变异点扫描结果

- **CRAP（受限近似口径，非标准工具输出）**：Rust 无公认 CRAP 工具，按 engineering.md
  「受限口径的近似度量」以 源码圈复杂度（决策点计数）× llvm-cov 行覆盖率代入
  comp²(1-cov)³+comp 估算（分析器留存 `.work/tmp/crap_approx.py`，会话级不进归档）。
  收敛后残余三类登记：① cov=1.0 的 comp 残余（on_mouse comp=12、dlg_confirm_add comp=13、
  spread_bytes comp=9、on_evt/on_key 分派器）——分派臂数固有，按前轮 cleaner 口径登记不硬凑；
  ② supervisor download 闭包（cov≈0.44）——async 引擎内部，e2e/hardender 边界；
  ③ 上轮已登记的「CRAP>6 残余登记口径（architect 裁决）」待办承前有效。
- **DRY（PMD 7.16.0 CPD，70-token，-l rust）**：产品代码真实重复 0 块；余 3 块均为
  模块级 `#![allow(...)]` lint 配置头跨文件同形（Rust 逐文件 lint 豁免惯用法，各文件
  豁免项随模块职责不同，去重需 include! 非惯用手法）——登记接受。
- **变异点扫描（cargo-mutants 27.1.0 --list，未跑变异）**：总 1401 点 / 35 文件；
  最高 `model/chunk/lease.rs` 88，**全库无文件 >100，无需保持行为拆分**；本轮变更面
  各文件均 ≤84（app/engine.rs 84 最高）。清单由工具自管理，未手工编辑。

### 工具与环境（新沙箱完整重建，交接下游）

- 工具链：rustup minimal + nightly（1.101.0-nightly，rustfmt/clippy/llvm-tools-preview），
  `~/.cargo/bin` + `~/.local/bin`（PATH 需含两者）；lld 桥接：`~/.local/bin/ld.lld →
  ~/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld`。
- 度量工具：cargo-llvm-cov 0.9.1、cargo-mutants 27.1.0（release 二进制，`~/.cargo/bin`）；
  PMD 7.16.0（`swarm/.tools/pmd-bin-7.16.0/`，经 bin/pmd 启动，`cpd --language rust`）。
- 环境重建经强制重做验证（全量真编译 + 221 测），非增量假绿。

### 待办与待批（未决项，承前不变）

1. **QA-TD-15 端到端 runner 实现 + 9 套 runner 整体回归**（QA 节点；含 FR-01-81 修订二
   失效用例清理——suite_display 中明细表断言用例删除/改写）。
2. CRAP >6 残余登记口径（architect 裁决）；本轮新增：cov=1.0 的 comp 残余三函数
   （on_mouse / dlg_confirm_add / spread_bytes）是否表驱动化由 architect 裁决。

### 受限项与登记

- 新沙箱无既有工具链/缓存：按 notes/rust.md「本体丢失」条款完整重装（见上节）。
- CRAP 为受限近似口径（Rust 无标准工具），数字不可与标准工具输出互比；下游如需精确
  口径由 hardender 以变异结果为准。
- CPD 余 3 块 `#![allow]` 头为接受项，后续会话无需再报。

### 移交建议

- 下一步建议 **six-pack/architect**：模块边界复核（本轮仅做局部清理，未动依赖方向）+
  comp 残余函数裁决 + 遗留架构清单推进。
- 或按操作者指示先跑 **six-pack/qa**（QA-TD-15 runner 与整体回归仍欠）。
- 收尾：`./bin/swarm complete`（target/ 由 complete 自动清理；本轮会话内文件均已在
  git 索引中刷新）。
