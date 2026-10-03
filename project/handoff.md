# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect（架构评审 + 属性测试批次）　会话：architect-20261003

## 一、产物历史完成情况

### 项目与需求

- EZR Downloader（ezr download）：TUI 高性能多协议下载器，正式版一期。
- 需求权威来源：`project/mission.md`（v1.1 大纲）+ `project/mission/phase-01.md`（v1.2 详述）。

### 产物总账

| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier / coder / cleaner 组 / QA（首轮至四轮） | 详见历史归档 `swarm-20261002-062546` ~ `091047` | 已交付 | 208 测全绿基线 |
| coder（缺陷修复） | 明细表重复表头 + 闪烁两缺陷修复 | 已交付 | 210 测全绿 |
| specifier（三轮修订） | FR-01-81 修订二：并发分块明细表整体移除（三层同步） | 已交付 | 归档 specifier-20261002c |
| coder（三轮修订实现） | 明细表 UI 段落 + 连接级展示机制整体移除 | 已交付 | 206 测全绿、clippy 0、fmt 清零 |
| cleaner（清理批次） | 保持行为清理 3 处 + 覆盖率/CRAP/DRY/变异点四项度量与补测 15 条 | 已交付 | 221 测全绿、clippy 0、fmt 0 |
| **architect（本轮）** | 四阶段架构评审 + 4 项保持行为改造 + 2 项裁决 + proptest 属性测试 12 条 + arch_check.sh 自动化边界检查 | 已交付 | 233 测全绿（221 单测 + 12 属性）、clippy 0、fmt 0、arch_check EXIT=0 |

### 遗留受限项（当前有效）

- CPD 余 3 块 `#![allow]` lint 配置头跨文件同形：Rust 逐文件豁免惯用法，登记接受，无需再报。
- CRAP 为受限近似口径（cleaner 会话，llvm-cov 行覆盖近似）；hardender 期以变异结果为准。
- cargo-mutants / cargo-llvm-cov / PMD 本轮沙箱未安装（architect 无需求）；hardender/QA 期按 notes/rust.md 重装。
- QA-TD-15 端到端 runner 实现与 9 套整体回归仍欠（QA 节点，承前）。

## 二、当前产出情况（architect → 移交）

### 四阶段架构评审结论（结论 + 违规清单 + 改进动作）

1. **UI/核心分离——合格**。ui 层零 IO、零 engine 依赖（渲染纯函数经 TestBackend 测试锁定）；model 层纯逻辑零外部依赖。登记接受 2 项：① App 持 ratatui `Rect` 回填字段（`dlg_btn_rects` 等，鼠标命中测试每帧回填；交互层权威状态 + 鼠标测试锁定，抽象化改造行为风险高于测试性收益）；② main.rs 保留 CLI 解析（`parse_cli`/`print_help`，入口 shell 职责，不触 App 内部）。
2. **依赖规则——净 DAG 无环，违规 1 项已修**。分层 `model(∅) ← engine ← app ← ui ← main`，方向全部指向内层。违规：`engine/mod.rs` 的 `ConnView::to_connection()` 低层构造高层类型（notes/rust.md §8 违规形态）→ 改造③。
3. **信息隐藏与封装——违规 1 项 + 双源风险 1 项已修**。`EngineHandle.cmd_tx`/`evt_rx` 私有封装良好；`Cmd`/`Evt` 窄接口齐备。违规：`Evt::Probed.cd_name` 死字段跨边界（App 消费侧显式 `cd_name: _` 忽略，引擎内部消费的是 `ProbeHead` 非事件字段）→ 改造②。双源：`CHECKSUM_ALGOS` app/model 各一份 → 改造④。登记接受：model 层 serde derives（sidecar/registry 持久化格式本身是产品契约，拆 DTO 序列化兼容风险 > 收益）；App pub 回填字段（见上）。
4. **局部代码质量——合格**。clippy 0（含 nightly 漂移 1 条 `bool_assert_comparison` 基线恢复）、fmt 0；命名与模块文档一致；comp 残余三函数裁决见下。

### 架构改造清单（保持行为不变，221 单测基线全绿核验）

1. **`add_cli_task` 迁入应用层**：main.rs 的 `impl App` 块 → 新建 `src/app/cli.rs`（含 `cli_add_tests` 迁移）；main.rs 仅余 CLI 解析/事件循环/终端壳。notes/rust.md「入口不扩展应用主类型的 impl」条款归位，`grep -nE '^impl App \{' src/main.rs` 零命中。
2. **`Evt::Probed.cd_name` 死字段移除**（3 处：`engine/mod.rs` 变体定义、`supervisor.rs` 构造点、`app/engine.rs` 消费模式）。字段无任何消费者，删除零行为面（SKILL 安全类「移除无消费者的死字段」）。
3. **ConnView 适配移消费侧**：删除低层 `ConnView::to_connection()` 方法（engine 层回归纯数据视图）；消费侧新增 `app/engine.rs` 私有 `conn_from_view()`，进度消费点 `.map(conn_from_view)`；映射锁定测试 `to_connection_maps_progress_fields` 随适配面迁入 app 层改名 `conn_view_maps_progress_fields`（断言逐字段等价）。SKILL 安全类「适配逻辑等价地移到消费侧」。
4. **`CHECKSUM_ALGOS` 双源收口**：删除 app 层 2 元组同名表，改为 `pub use crate::model::checksum::CHECKSUM_ALGOS`（3 元组单源：显示名/期望位数/伴随后缀）；`ui/dialog.rs` 两处解构补 `_`。消除对话框下拉与 CLI `-x`/伴随文件解析两套口径的漂移风险（同名同序同值，行为逐字节等价）。

### 裁决（cleaner 遗留两项，结论收口）

- **裁决一（CRAP>6 残余登记口径）**：确立「**登记不硬凑**」为口径——cov=1.0 的 comp 残余（on_mouse comp=12、dlg_confirm_add comp=13、spread_bytes comp=9、on_evt/on_key 分派器）CRAP=comp（(1-cov)³ 项为 0），表驱动化属指标化妆而非清晰度收益。登记要素 = 函数名 + comp 值 + 臂结构理由；后续会话无需重议；hardender 变异若暴露其中具体函数的缺陷，再按变异结果针对性处理。
- **裁决二（三函数不表驱动化，保留）**：
  - `on_mouse`（app/mouse.rs）：交互分派器，臂间异构状态突变（对话框三分表命中 / 列表三臂），已有 `hit` 闭包提取；表驱动需 (kind, handler) 闭包表，间接层不减真实复杂度。
  - `dlg_confirm_add`（app/dialogs.rs）：校验链各臂副作用互异（不同 toast / focus 重置）。与 `add_cli_task` 的共性合并**被否决**：两条添加流在重复拒绝（对话框拒 / CLI 不拒）、并发语义（对话框非法回退默认 / CLI 启动报错）、校验值来源（伴随文件解析仅对话框）、接续判定臂（对话框含任务表臂）四处行为承载点不同——共享核需旗标参数膨胀，属 SKILL「不安全」类。
  - `spread_bytes`（model/chunk/lease.rs）：迭代差额修正算法（守恒 + 钳制不变量），契约清晰的纯函数；本轮属性测试已锁定其守恒与钳制面。

### 属性测试（独立交付物，对账隔离）

- 框架：`proptest 1.11`（dev-dependency，不进发布物）；独立模块 `src/property_tests.rs`（main.rs `#[cfg(test)]` 挂载）；独立运行命令：`cargo test property_tests::`。
- **12 条属性全绿**，覆盖 SKILL 要求的不变量/守恒性/幂等性/排序/钳制类别：
  1. `prop_chunk_total_is_ceil_div` 块数 = ⌈total/piece⌉（含 u32::MAX 封顶口径）
  2. `prop_block_ranges_tile_total` 块区间无缝铺满 + 末块吸收余数 + 越界 None
  3. `prop_fmt_block_size_stable` 块大小展示三段式口径稳定
  4. `prop_spread_bytes_conserves` 散布总和守恒 `min(amount, n*cap)` + 逐份钳制
  5. `prop_lease_snapshot_invariants` 快照区间有序不重叠 + done 不越块界 + 完成块数双口径 + 完成态写满
  6. `prop_retry_decide_rules` 重试连续性 + at_limit ⟺ 达上限 + Retry-After 优先
  7. `prop_backoff_monotone_capped` 退避序列非降 + 60s 封顶
  8. `prop_sanitize_idempotent` sanitize 幂等 + 路径分隔符/控制字符清零
  9. `prop_join_path_trailing_slash_idempotent` 尾分隔符幂等
  10. `prop_dedupe_returns_free_name` 去重结果不被占用集命中 + 空闲原样返回
  11. `prop_ema_bounded` EMA 值域 [0, max]（凸组合）
  12. `prop_ema_converges_constant` 恒定输入距离单调收敛 + zero 归零
- 覆盖评估：model 层纯逻辑面已铺；IO/异步耦合面（sidecar 文件往返、config 磁盘加载、supervisor async 引擎）不入属性面，留既有单测与 e2e。
- **对账隔离**：单测基线 221（180+30+11）逐项不变；全量 `cargo test` = **233 = 221 + 12**；属性测试失败不与单测基线对账（随机输入概率性，复核时先重跑确认可复现失败再归因）。

### 自动化架构检查

- `project/ezr/scripts/arch_check.sh`（grep 级零依赖，依据 notes/rust.md §8 方案），6 条规则：①model 零外层依赖 ②engine 不依赖 app/ui ③ui 不直接依赖 engine ④main.rs 无 impl App ⑤engine 不构造 Connection（低层构造高层类型）⑥CHECKSUM_ALGOS 单源。
- 基线全绿 EXIT=0；阳性对照已做（临时注入违规行：规则⑤/④/③逐一变红命中 file:line，移除后恢复全绿），零发现结论以对照生效为前提。
- 下游集成：可作为验证链 `&&` 环节（如 hardender/QA 期前置）。

### 验证证据

| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 单元测试（基线） | 221P / 0F | `cargo test`（property_tests 过滤后口径 221） |
| 属性测试（独立面） | 12P / 0F | `cargo test property_tests::` |
| 全量测试 | 233P / 0F | `cargo test` |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| 格式化 | 通过 | `cargo fmt --check` |
| 架构边界 | 6/6 规则通过 | `bash scripts/arch_check.sh` |

### 对账口径

- 单测基线：206 + 15 = 221（cleaner 口径）**承前不变**——本轮迁移类改动（add_cli_task+测试 → app/cli.rs、映射锁定测试随适配面改名迁层）净增减为 0，可由 `cargo test | grep '^test result'` 三段分项复算。
- 属性测试：+12 为独立交付物，不入单测对账公式（对账隔离纪律）。

### 观察项（下游关注，非缺陷）

- CLI 添加路径（`app/cli.rs add_cli_task`）不调用 `resolve_checksum`（伴随文件解析），对话框/重试路径调用。对照规格：01-add-task-12 CLI 仅显式 `-x`；01-integrity-check 伴随文件场景限定对话框/校验流——未见规格违规。若操作者期望 CLI 添加也自动接伴随校验值，属需求变更，交 specifier 裁量。

### 待办与待批（未决项）

1. QA-TD-15 端到端 runner 实现 + 9 套 runner 整体回归（QA 节点；含 FR-01-81 修订二失效用例清理，承前）。
2. hardender 期工具重建：cargo-mutants（本轮沙箱未装）；如需复核覆盖率则 cargo-llvm-cov 同理（notes/rust.md「本体丢失」条款）。

### 工具与环境（本沙箱重建记录）

- rustup minimal + nightly 1.101.0（rustfmt/clippy/llvm-tools-preview），`~/.cargo/bin`；lld 桥接 `~/.local/bin/ld.lld → ~/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld`（PATH 需含两目录）。
- proptest 1.11（crates.io，dev-dependency，随 Cargo.lock）。
- 经强制重做验证（全量真编译 + 233 测），非增量假绿。

### 移交建议

- 下一步建议 **six-pack/hardender**：变异测试暴露高复杂度函数后针对性加固；`scripts/arch_check.sh` 可纳入其验证链，comp 残余登记口径已收口无需重议。
- 或按操作者指示先跑 **six-pack/qa**（QA-TD-15 runner 与整体回归仍欠）。
- 收尾：`./bin/swarm complete`（target/ 由 complete 自动清理）。
