# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner（二次清理）　会话：cleaner-20261002（QA 修复后回归清理）

## 一、产物历史完成情况

### 项目与需求

- EZR Downloader（ezr download）：TUI 高性能多协议下载器，正式版一期。
- 需求权威来源：`project/mission.md`（v1.1 大纲）+ `project/mission/phase-01.md`（v1.2 详述）。

### 产物总账

| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier | `features/` 9 份 Gherkin（114 场景）+ `qa/` 9 份规程 | 已交付 | 三层一致 |
| coder | `project/ezr/` 下载内核+TUI+CLI+fixture | 已交付（P0+P1 全量） | 100 单测、clippy 0 警告 |
| cleaner/architect/hardender（首轮） | 清理/架构/加固 | 已交付 | 归档在案（CRAP 阈值 30 口径 46/200 超、UI 交互层豁免） |
| QA | `project/qa/runners/` 9 套可执行套件（114 用例）+ 产品缺陷修复 10 处 + QA 基建修复 2 处 | 已交付 | 最终 110 P + 4 S / 0 F；单测 100 P 基线 |
| cleaner（本会话，二次清理） | QA 修复后全量回归清理：行为不变重构 11 文件 + 覆盖率/CRAP/DRY/变异点四类度量复跑 | 已交付 | 单测 112 P / 0 F、clippy 0 警告、CPD 产品代码 0 重复块（详见下） |

### 遗留受限项（当前有效）

- **CRAP ≤6 未全域达成**：60/203 函数超 6（阈值 30 口径下较首轮 46/200 改善）。剩余 >30
  集中于 TUI 交互层（on_dialog_key/draw_*/tick 等）与 QA 基建二进制（ezr-fixture/ezr-proxy）：
  该层可测性依赖伪终端 e2e（QA 套件域，cleaner 按 SKILL 不运行）；压到 comp≤6 属架构级
  拆分。沿用首轮豁免口径，交 architect/操作者裁决。
- **rustfmt 基线非整洁**：`cargo fmt --check` 全仓 170 处差异（首轮以来即如此，非本会话引入）；
  本会话仅修复真实缩进破损 4 处，未整树重排（避免污染 QA 评审基线）。是否整树格式化待
  操作者/architect 决策。
- **变异点 >100 的文件待拆分裁决**：见当前节变异点扫描。
- **QA 基建注记（phase-02 建议）**：ezr-fixture 对每请求整文件读入内存（64 并发 × big-100m.bin
  ≈ 6.4GB OOM 实证）；改为按 Range 流式读盘可根除。

### 实现定义值登记

- 沿用前轮全部登记（toast 文案、证书错误文案、v0.1.0-01、多伴随校验择序 = 算法表声明序
  （MD5 在前）、default_concurrency 非法值钳制 1–64）。本会话未新增契约值。

## 二、当前产出情况

### 本会话产物清单（全部行为不变，`git diff` 可复核）

- `src/model/mod.rs`：新增 `Task::new_queued`（添加路径初值单源）与 `Task::apply_chunk_snapshot`
  （断点视图快照单源）；新增 `save_json_atomic`（JSON 原子写单源）；删除 tests 内与模块级
  重复的 `sample_task`（40 行 ×2 归一）；新增构造器单测。
- `src/app.rs`：dlg_confirm_add 目录尾斜杠归一（单点化，删 dir_check/dir_trim 三重 trim）；
  两处快照逻辑改用 `apply_chunk_snapshot`；toggle_pause 永假 or-模式守卫简化；handle_failure
  绑定遮蔽清理；缩进破损 3 处修复；新增对话框确认流单测（锚定目录归一行为）。
- `src/main.rs`：CLI 添加手写去重循环改用 `namegen::dedupe`（与对话框/引擎同口径）；Task
  字面量改用 `Task::new_queued`；新增 CLI 添加去重单测。
- `src/engine/mod.rs`：删除 `Evt::Failed.blocks` 死字段（app 层 `let _ = blocks;` 显式丢弃，
  过期 allow 同步移除）。
- `src/engine/supervisor.rs`：worker 停止序列 ×4 提取 `stop_workers`；删除 build_sidecar 死参数
  `_name`（5 调用点同步）；`y` 改名 `total_blocks`/`expected_blocks`；e2e 测试夹具提取
  `launch_spec`/`sidecar_spec`（cmd_tx 作为 keep-alive 句柄返回调用方）。
- `src/ui.rs`：按钮行渲染 ×2 提取 `draw_button_row`；新增纯文本工具测试模块（w/truncate/
  pad/fmt_size/fmt_speed/fmt_dur/fmt_size_pair/plain_bar 共 8 测）。
- `src/model/namegen.rs`：percent_decode 重复谓词与边界冗余归一（行为等价）；新增残缺转义
  尾巴边界测试。
- `src/model/{sidecar,registry,chunk,slots}.rs`：save 委托 `save_json_atomic`；`let _ = s;`
  死语句删除；slots 测试夹具改用共享 `sample_task`。

### 验证证据

| 验证项 | 结果 | 命令（project/ezr 下可复现） |
|---|---|---|
| 单元测试 | 112 P / 0 F | `cargo test` |
| clippy 全目标 | 0 警告 | `cargo clippy --all-targets` |
| 覆盖率（llvm-cov 0.9.1） | 总行覆盖 43.9%→49.0%；app.rs 8.8%→27.6%、main.rs 0%→26.6%、ui.rs 13.5%（纯函数已测，交互层 e2e 覆盖） | `cargo llvm-cov --lcov --output-path <lcov>` |
| CRAP（cargo-crap 0.6.1，LCOV 输入，阈值 6） | 60/203 超；阈值 30 口径较首轮 46/200 改善；dlg_confirm_add 306→25.1、add_cli_task 6.0 | `cargo crap --lcov <lcov> --path src --threshold 6` |
| DRY（PMD 7.28.0 CPD，70-token，`-l rust`） | 产品逻辑代码 0 重复块；余 2 类豁免（#![allow] 强制模板头 ×15 文件、ui.rs BT/HTTP 预留并行版式） | `pmd cpd -d src -l rust --minimum-tokens 70` |
| 变异点扫描（cargo-mutants 27.1.0 --list，全仓 1545 点） | app.rs 376 / ui.rs 311 / chunk.rs 143 / model/mod.rs 138 / supervisor.rs 55 / namegen.rs 65 / main.rs 43 / ezr-fixture.rs 126（未在本会话更改）/ 其余 ≤50 | `cargo mutants --list` |

### 对账口径（与 QA 会话总账）

- 单测 100 P → 112 P（+12：ui 8、model 1、namegen 1、流级 2；全部纯追加，既有 100 测未改动）。
- QA 的 10 处产品缺陷修复逻辑全部保留（本会话仅重构单源化，未触碰判定分支与文案）；
  clippy 0 警告与"QA 修复后 100 P"基线在本会话开头复现成立。

### 待办与待批

1. **变异点 >100 文件的拆分裁决（移交 architect）**：app.rs 376 / ui.rs 311 / chunk.rs 143 /
   model/mod.rs 138 / ezr-fixture.rs 126。app/ui 的对话框、粘贴、渲染、事件四类职责可拆但
   属模块边界决策，且验证依赖 QA e2e 套件——建议 architect 在 hardender 全量变异测试前
   定模块边界（hardender 可用 `--file` 按文件分块）。
2. **CRAP 阈值口径确认**：SKILL 目标 ≤6 与交互层现实（comp 20+ 的状态机函数、e2e 覆盖）
   差距需操作者/architect 表态：接受首轮豁免口径延续，或专项拆分。
3. rustfmt 整树格式化与否（遗留受限项）。
4. ezr-fixture Range 流式化（phase-02 基建优化，非阻塞）。

### 移交建议

- six-pack 流转路径：specifier → coder → **cleaner（本轮，已收尾）** → **architect（建议下一棒）**
  → hardender → QA。
- architect 评审时请携带本会话变异点计数与 CRAP 分布（见验证证据表），就模块边界与
  app.rs/ui.rs 拆分一并裁决；hardender 变异测试请在裁决后进行。
- 收尾：`./bin/swarm complete`（构建缓存 target/ 由 complete 自动清理）。
