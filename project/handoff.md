# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner（三次清理·遗留问题攻坚）　会话：cleaner-20261003

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
| cleaner（二次） | QA 修复后回归清理：11 文件行为不变重构 + 单源化 | 已交付 | 112 P / 0 F、覆盖率 49.0% |
| cleaner（三次，本会话） | 遗留 4 项攻坚：rustfmt 整树、CRAP 收敛、>100 变异点文件全拆分、fixture 流式化 | 已交付 | 202 测全绿、覆盖率 83.4%、见下表 |

### 遗留受限项（当前有效）

- **CRAP >6 残余 46/268 函数**：其中 comp≤6 的均已 100% 覆盖（CRAP=comp）；残余 comp>6
  的集中在异步/事件编排入口（`on_key`/`on_mouse`/`on_evt`/`tick`/`on_dialog_key`/
  `toggle_pause`/`main`，comp 12–25）——分派臂数即 comp，纯逻辑已尽数抽为可测纯函数
  并锁定；压到 comp≤6 属进一步碎片化，**建议 architect 裁决是否接受登记口径**。
  >30 的函数由二次清理时 24 个降至 8 个。
- **QA 套件（9 套 runner）本轮未运行**（cleaner 职责边界，SKILL 禁止）：fixture 与 proxy
  的行为保持由字节级单测锁定（fixture 30 测/proxy 11 测覆盖 200/206/416/404/ghost/
  streamy/swapsize/disconnect/etag/redirect/隧道/转发），**建议 QA 节点回归**。
- **rustfmt unstable 选项**（imports_granularity 等）在 stable 通道不生效：配置模板原样
  保留（notes/rust.md 口径），未切 nightly。
- **下游角色开工先读** `packs/_common/notes/rust.md`（PATH 注入、工具位置）与
  `packs/_common/engineering.md` 新增「清理会话沉淀」节。

### 实现定义值登记

- 沿用前轮全部登记（toast 文案、证书错误文案、v0.1.0-01、多伴随校验择序 = 算法表声明序、
  default_concurrency 钳制 1–64、ETag 规范化 `/?&=` → `_`、Last-Modified 固定串、
  fixture 字节模式 `i%251`）。本会话未新增契约值。

## 二、当前产出情况

### 本会话产物清单（全部行为不变，`git diff` 可复核）

- **rustfmt 整树**：`cargo fmt` 全仓一次过，`fmt --check` 清零（171→0，19 文件重排）。
- **模块拆分（全部 ≤100 变异点，最高 91；上游最高 376）**：
  - `src/app.rs`(1765 行/376 点) → `app/`{mod,engine,keys,dialog_keys,dialogs,paste,mouse,tasks}
  - `src/ui.rs`(1602 行/311 点) → `ui/`{mod,text,task_lines,list,header,detail,dialog,btn,delete}
  - `src/model/chunk.rs` → `chunk/`{mod,plan,blocks,lease}；`src/model/mod.rs` → 抽出 `model/task.rs`+`model/timefmt.rs`
  - `src/bin/ezr-fixture.rs` → 目录式 bin `ezr-fixture/`{main,query,request,files,range,respond,handle}（Cargo.toml bin 路径同步更新）
- **CRAP 收敛（>30 函数 24→8）**：checksum `digest_file` 函数表化；error `status_text` 表驱动 +
  `classify_reqwest` 抽 `FailureFeatures` 决策核；engine `main_loop` 拆 5 单命令处理器 +
  `handle_cmd`；main `parse_cli` 拆 opt_conns/opt_speed/pos_url + `try_lock_path`；
  ui `state_color` 表驱动、`task_lines` 拆 10 个行构造器、`draw_detail` 拆 9 个纯生成器、
  dialog 抽 `field_display`/`dropdown_rect`。
- **ezr-fixture Range 流式化**：body 输出从每请求整文件读入改为按需 seek + 64 KiB 分块流式
  读盘（每请求内存恒定，根除 64 并发 × big-100m.bin ≈ 6.4GB OOM）；`?swapsize=N` 例外保留
  物化（QA 约定仅小文件使用）；越界 Range、反向 Range 的 panic 口径原样保留。
- **测试 +90（112→202）**：ui 渲染 6（TestBackend 全状态/对话框/窄布局）、状态行 8、
  对话框键 3、tick 集成 2（bogus-URL 失败路径 + 桩服务器完成路径，真实驱动
  tick/on_evt/handle_failure/make_spec）、fixture 字节级 30、proxy 环回 11。
- **经验沉淀 5 条** → `packs/_common/notes/rust.md` + `packs/_common/engineering.md`
  （TestBackend 显示口径、ANSI 吞字、双向拷贝测试关写侧、拆分两步法、llvm-cov 首跑交互提示）。

### 验证证据

| 验证项 | 结果 | 命令（project/ezr 下可复现） |
|---|---|---|
| 单元测试 | 202 P / 0 F（161+30+11） | `cargo test` |
| clippy 全目标 | 0 警告 | `cargo clippy --all-targets` |
| rustfmt | 0 差异（上游 171） | `cargo fmt --check` |
| 覆盖率（llvm-cov 0.9.1） | 总行 83.4%（49.0%→）；渲染层/交互层经 TestBackend 与桩服务器路径覆盖 | `cargo llvm-cov --lcov --output-path <lcov>` |
| CRAP（cargo-crap 0.6.1，阈值 6） | 46/268 超（60/204→）；>30 由 24→8，残余为事件编排入口（登记待裁决） | `cargo crap --lcov <lcov> --path src --threshold 6` |
| DRY（PMD 7.28.0 CPD，70-token） | 产品逻辑代码 0 重复块；余 3 处 `#![allow]` 模板头（豁免类） | `pmd cpd -d src -l rust --minimum-tokens 70` |
| 变异点（cargo-mutants 27.1.0 --list） | 全文件 ≤100（最高 app/engine.rs 91；上游最高 app.rs 376） | `cargo mutants --list` |

### 对账口径（与二次清理总账）

- 单测 112 P → 202 P（+90：ui 渲染与状态行 14、对话框键 3、tick 集成 2、fixture 30、
  proxy 11、error 8、checksum 2、engine 命令 6、cli 解析/锁/overlay 12、misc 2）；既有
  112 测未改动（fmt 重排除外）。
- QA 会话与二次清理的全部产品修复逻辑保留（仅行为不变重构与文件重组）。

### 待办与待批

1. **CRAP >6 残余登记口径**：46/268 中 comp>6 的全部为异步/事件编排入口（见遗留受限项）；
   请操作者/architect 表态「接受登记」或「专项拆分」（拆分边际收益递减，覆盖率已 83.4%）。
2. **QA 套件回归**：fixture/proxy 重构后建议运行 9 套 runner 确认端到端（单测已字节级锁定）。
3. ezr-fixture `?swapsize` 大文件物化（>64MB 级）如需支持 → 转流式拼接（02 期基建）。

### 移交建议

- six-pack 流转路径：specifier → coder → cleaner×3 → **architect（建议下一棒）** →
  hardender → QA。
- architect 评审请携带：模块树现状（app/9 文件、ui/11 文件、model/13 文件）、变异点分布
  （`cargo mutants --list`）、CRAP 残余清单（`.work` 已清，重跑命令见上表）；hardender
  可用 `--file` 按文件分块。
- 收尾：`./bin/swarm complete`（构建缓存 target/ 由 complete 自动清理）。
