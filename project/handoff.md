# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner（四次清理·工具链切换 nightly）　会话：cleaner-20261002b

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
| cleaner（三次） | 遗留 4 项攻坚：rustfmt 整树、CRAP 收敛、>100 变异点文件全拆分、fixture 流式化 | 已交付 | 202 测全绿、覆盖率 83.4% |
| cleaner（四次，本会话） | rustfmt 切 nightly：`rust-toolchain.toml` 钉 nightly、unstable 选项全量生效整树重排、死依赖清除 | 已交付 | 202 测全绿、clippy 0、fmt --check 清零、见下表 |

### 遗留受限项（当前有效）

- **CRAP >6 残余 46/268 函数**：其中 comp≤6 的均已 100% 覆盖（CRAP=comp）；残余 comp>6
  的集中在异步/事件编排入口（`on_key`/`on_mouse`/`on_evt`/`tick`/`on_dialog_key`/
  `toggle_pause`/`main`，comp 12–25）——分派臂数即 comp，纯逻辑已尽数抽为可测纯函数
  并锁定；压到 comp≤6 属进一步碎片化，**建议 architect 裁决是否接受登记口径**。
  >30 的函数由二次清理时 24 个降至 8 个。
- **QA 套件（9 套 runner）本轮未运行**（cleaner 职责边界，SKILL 禁止）：fixture 与 proxy
  的行为保持由字节级单测锁定（fixture 30 测/proxy 11 测覆盖 200/206/416/404/ghost/
  streamy/swapsize/disconnect/etag/redirect/隧道/转发），**建议 QA 节点回归**。
- **下游角色开工先读** `packs/_common/notes/rust.md`（PATH 注入、工具位置、nightly 钉）
  与 `packs/_common/engineering.md`（清理会话沉淀 + tarball 覆盖解压纪律）。

### 实现定义值登记

- 沿用前轮全部登记（toast 文案、证书错误文案、v0.1.0-01、多伴随校验择序 = 算法表声明序、
  default_concurrency 钳制 1–64、ETag 规范化 `/?&=` → `_`、Last-Modified 固定串、
  fixture 字节模式 `i%251`）。本会话未新增契约值。

## 二、当前产出情况

### 本会话产物清单（全部行为不变，`git diff` 可复核）

- **`project/ezr/rust-toolchain.toml` 新增**：`channel = "nightly"` +
  `components = ["rustfmt", "clippy"]`——全项目 cargo 命令（build/test/fmt/clippy）
  统一走 nightly，`imports_granularity`/`group_imports` 等 unstable 选项全量生效，
  下游角色无需手工 `+nightly`（工具链本体现装：rustc 1.101.0-nightly / rustfmt
  1.11.0-nightly / clippy 0.1.101，2026-10-01 nightly）。
- **nightly rustfmt 整树重排**：23 文件（+50/-65，净 -15 行），import 按
  StdExternalCrate 分组归位、按 Module 粒度合并；`cargo fmt --check` 清零，
  stable 时代 "can't set … unstable" 警告不复现。
- **死依赖清除**：nightly 默认 lint `cargo::unused_dependencies` 揪出 `rcgen`
  （全仓零代码引用，仅注释提及）→ 从 `Cargo.toml` 移除，`Cargo.lock` 同步净化；
  `main.rs` 顶部 `multiple_crate_versions` 豁免注释的依赖链口径同步修正
  （rcgen/reqwest → reqwest/rustls）。
- **经验沉淀 2 处**：`packs/_common/notes/rust.md`（rustfmt unstable 选项必须配
  nightly 工具链钉——旧口径「不为此切 nightly」作废）；
  `packs/_common/engineering.md`（交付 tarball 覆盖解压到既有树会留过期文件——
  tar 无删除语义，解压前 diff 文件清单，E0761 模块歧义预防）。

### 验证证据

| 验证项 | 结果 | 命令（project/ezr 下可复现） |
|---|---|---|
| 单元测试 | 202 P / 0 F（161+30+11），nightly rustc 下全绿 | `cargo test` |
| clippy 全目标（nightly 0.1.101） | 0 警告（含 manifest 级） | `cargo clippy --all-targets` |
| rustfmt（nightly 1.11.0） | 0 差异；unstable 选项真实生效 | `cargo fmt --check` |
| 构建全目标 | 通过（dev profile 1m25s 全量重编） | `cargo build --all-targets` |
| 死依赖 | `rcgen` 已从 Cargo.toml/Cargo.lock 移除 | `grep -c rcgen Cargo.lock` → 0 |
| 变异点分布（cargo-mutants 27.1.0 --list） | 1648 总点；最高 app/engine.rs 91；>100 文件数 0（与三次清理一致，格式化零漂移） | `cargo mutants --list` |

### 对账口径（与三次清理总账）

- 202 测既有用例未改动任何断言（本会话仅 import 重排与依赖清单净化）。
- 三次清理的全部产物逻辑与行为保持不变；本会话新增仅 `rust-toolchain.toml`。

### 待办与待批

1. **CRAP >6 残余登记口径**：46/268 中 comp>6 的全部为异步/事件编排入口；
   请操作者/architect 表态「接受登记」或「专项拆分」（拆分边际收益递减，覆盖率已 83.4%）。
2. **QA 套件回归**：fixture/proxy 重构后建议运行 9 套 runner 确认端到端（单测已字节级锁定）。
3. ezr-fixture `?swapsize` 大文件物化（>64MB 级）如需支持 → 转流式拼接（02 期基建）。

### 移交建议

- six-pack 流转路径：specifier → coder → cleaner×4 → **architect（建议下一棒）** →
  hardender → QA。
- architect 评审请携带：模块树现状（app/9 文件、ui/11 文件、model/13 文件）、变异点分布
  （`cargo mutants --list`）、CRAP 残余清单（重跑命令见验证证据表）；hardender
  可用 `--file` 按文件分块。
- 注意：项目已钉 nightly（`rust-toolchain.toml`），任何机器首次构建会由 rustup
  自动拉取 nightly 工具链（minimal + rustfmt + clippy）。
- 收尾：`./bin/swarm complete`（构建缓存 target/ 由 complete 自动清理）。
