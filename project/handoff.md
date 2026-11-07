# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner（bug27 URL 唯一性轮之后的清理轮，本轮已完成）
> 路径：specifier → coder → cleaner → architect → hardender（操作者指示跳过并关闭）→ QA → Done
> 　　→ 复验轮（2026-11-06）→ specifier 规格修订轮（v1.45 守卫）→ specifier 审批改判轮（v1.46）
> 　　→ coder 实现轮（v1.45+v1.46）→ coder bug26 限速修复轮 → coder bug27 修复轮（2026-11-07）
> 　　→ **cleaner 清理轮（本轮，2026-11-07）**
> 会话：ezr-cleaner-bug27-cleanup（2026-11-07，任务 = 对 coder bug27 轮产出做保持行为清理 + 覆盖率/CRAP/DRY/变异点扫描四件度量；上游交接 = project/handoff.md（coder bug27 轮））
> 环境注记：工具链沿历轮版本带（rustup nightly 1.101.0，PATH=~/.cargo/bin；cargo registry rsproxy sparse；
> rustup dist rsproxy 官方双变量，本轮前台装齐）。度量工具本轮新装：cargo-llvm-cov 0.9.1 / cargo-mutants 27.1.0
> （~/.local/bin 预编译）、PMD CPD 7.29.0（~/.tools-pmd，DRY 用）。**pyte/torf/libtorrent 与 harness 夹具本轮沙箱未装**（cleaner 不跑 e2e；QA 轮接手时按 notes/environment.md 重装）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui 交互层 + tokio 引擎）。需求权威来源 = `mission.md`（分期模式）+ `mission/` 4 份 phase 详述（phase-01 v1.47 / phase-02 v1.14，bug27 轮裁决存档于头注）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier | features/ 全套 Gherkin 规格 + qa/ 全套 QA 套件；phase-01 v1.47 / phase-02 v1.14 | 已交付（bug27 轮含 v1.45 守卫 / v1.46 审批改判 / bug27 判重改判三轮修订） | 见 coder 各轮对账 |
| coder 历轮 | ezr 产品实现 + 12 集成测试目标 + bin 单测；bug6–bug27 修复 | 已交付（最后节点 = bug27 任务 URL 唯一性：三入口判重同口径、CLI 新增 URL 判重） | bug27 交接基线：bin 945P/0F、集成 6329P/0F、clippy 0 警告、fmt 干净 |
| cleaner 历轮 | 多轮保持行为清理（工具链 nightly 钉版、resolve_confirm_dir 单点化等） | 已交付 | 各轮全绿对账 |
| architect（2026-11-06） | 架构评审：规则 14 白名单、engine↔bt 枢纽、PieceMap 不变量 | 已交付 | 建议项归档 notes |
| hardender | —（操作者指示跳过并关闭，链路径改道） | 不适用 | — |
| QA（2026-11-06） | 对话框焦点几何、fixture cd=、套件过滤机制 | 已交付 | e2e 套件实跑 |
| 复验轮（2026-11-06） | KeepAlive 90s 一处产品变更；受限项 15→2 | 已交付 | 集成 6201P/0F 精确对账 |
| cleaner 本轮 | bug27 批次保持行为清理 + 四件度量（详见第二节） | 已交付 | bin 949P/0F、集成 6357P/0F、clippy 0、fmt 干净 |

### 遗留受限项（当前有效）
- **leech 判据无 e2e 自动化**（six-pack 链唯一长期有效受限项，历轮维持）。
- **变异实跑未执行**：cleaner 只做 scan/count（纪律边界：不跑变异测试）；hardender 路径已被操作者关闭，变异加固收益按 `--file` 分块口径登记待裁决。
- **e2e QA 依赖本轮沙箱未安装**：pyte/torf/libtorrent 未装、tracker/seeder 夹具未启动——QA 轮接手需先按 notes/environment.md（阿里云镜像 + harness README）重建，否则 e2e 套件不可跑。

## 二、当前产出情况

### 本会话产物清单（净行为变更 = 0）
- `project/ezr/src/app/dialogs.rs`：**三单点化 + 拆分**——`url_task_exists`（bug27 判重谓词三入口收敛，dialogs.rs 唯一实现面）、`derive_add_task_name`（occupied→sidecar→dedupe 15 行名推导，dialogs/cli 双胞胎收敛）、`register_new_task`（push+关窗+filter 归零+选中末行注册尾部）+ `announce_add_task`（paused×resumed 四臂 toast 矩阵）；自由函数 `validate_add_url`（空/协议守卫）、`conns_from_input`（并发钳制）；`dlg_finish_add` 拆分后 comp 12→5；测试 `preset_orphan_sidecar` 局部复制退役（委托 testutil）。
- `project/ezr/src/app/bt_dialog.rs`：判重谓词接入 `url_task_exists`；注册尾部复用 `register_new_task`（第三处手写复制消除）；新增 4 测（解析失败臂 / 非法协议 tracker 臂 / 暂停添加臂+无对话框空操作 / 空选守卫臂）。
- `project/ezr/src/app/cli.rs`：谓词与名推导接入单点助手；`SIDECAR_EXT` 失效导入删除。
- `project/ezr/src/app/testutil.rs`：`preset_sidecar_named(dir, name)` 提取（`preset_resume_sidecar` 委托之）。
- `project/notes/`：environment.md（镜像复测 + 度量三件套装机）、testing.md（llvm-cov 幻影行判据 + CRAP 受限近似口径 + 4 测追加对账）、architecture.md（确认链三单点化 + BT 序号循环不并网裁决）各追加本轮条目，INDEX.md 同步。
- `packs/_common/notes/rust.md`：+3 条跨项目经验（PMD CPD `-l rust` 为 Rust 可用 DRY 扫描器且 jscpd npm 版 rust tokenizer 缺失的阳性对照判法；llvm-cov if-let 收尾 `}` 幻影未覆盖行；rustup-init 落盘名陷阱）。
- `project/mission.md` / `mission/`：只读未动（cleaner 职责边界）。

### 验证证据
| 验证项 | 结果 | 命令（cd project/ezr，CARGO_INCREMENTAL=0） |
|---|---|---|
| bin 单测（含本轮净增 4 测） | **949P / 0F** | `cargo test --bin ezr` |
| 集成 12 目标（本轮净增 28） | **6357P / 0F** | supervisor 716 + engine 743 + appcore 747 + throttle_err 458 + v147 730 + hardening 142 + engine_backfill 849 + slots 203 + main 2 + proxy 16 + ui_backfill 879 + v115 872，逐目标单跑 |
| clippy | 0 警告 / 0 错误 | `cargo clippy --bins --tests` |
| rustfmt | 全仓干净 | `cargo fmt --check` |
| 覆盖率（llvm-cov 0.9.1，bin 套件口径） | bug27 触及区域 + 新助手 100%；批次文件行覆盖 cli 100% / dialogs ~97% / mod ~95% / bt_dialog ~95% | `cargo llvm-cov --bin ezr --lcov` |
| CRAP（受限近似：行覆盖+语法决策点代公式） | 批次函数全部 comp≤6：dlg_finish_add 12→5、dlg_confirm_add_bt cov 0.87→0.95（CRAP≈6.005，残余=3 行幻影花括号）、add_cli_task 4、新助手 1–5 | notes/testing.md 口径 |
| DRY（PMD CPD 7.29.0 `-l rust`，≥70tok） | 全仓 200 克隆（本轮 -1：fixture 双胞胎消除；目标三克隆全收口）；bug27 测试对等 5 项裁决保留（理由登记 notes） | `pmd cpd --dir src --language rust --minimum-tokens 70` |
| 变异点扫描（cargo-mutants 27.1.0 `--list`，未跑变异） | dialogs 101 / bt_dialog 100 / cli 2 / mod 142 | `cargo mutants --list --file src/app/dialogs.rs …` |

### 对账口径
- 基线（bug27 轮交接）：bin 945 + 集成 6329。本轮纯追加 4 测，挂载收编：bin +4 → 949；7 个挂载 app 子树的套件各 +4（845→849、743→747、739→743、712→716、726→730、875→879、868→872）；5 个未挂载套件计数不变（458/142/203/2/16）。分项复算：6329 + 28 = 6357 ✓ 自洽。
- 变更面与 bt::engine 零接触；ui_backfill 全量中 `graceful_close_sends_fin` 单例 FAIL 经三证（单测复跑绿 + 全套件单跑 879P/0F 绿 + 仅此一例）判时序 flake，非回归（第 4 例沿例）。

### 待办与待批
- **architect 轮裁决对象（登记移交，cleaner 不越权）**：① 变异点 >100 的三个交互层文件（dialogs 101 / bt_dialog 100 / mod 142）是否按模块边界拆分——点数与行数近似线性（~0.06–0.08 点/行）属交互层常态，拆分收益在 hardender `--file` 分块兑现，但 hardender 路径已关闭，请操作者裁决；② 前置高 comp 函数（add_bt_dialog_char 13 / dlg_confirm_modify_bt 11 / modify_bt_dialog_char 10 / parse_speed_limit 9 / overlay_open 8 / merge_trackers 7 / restore 7）均非 bug27 批次，未清理；③ dialogs.rs 测试的 Dialog 字面量构造克隆家族（179/165/142tok）——收敛需 Dialog 构造器/(builder)，属类型面设计，architect 裁决。
- **CLI `--dir ""` 边界差异登记**：CLI 路径空串目录 normalize 后落 cwd，对话框路径空串走 resolve_confirm_dir 回退 config/默认——两者语义不同，本轮按保持行为纪律未统一（无测试钉此边界）；如需统一属行为变更，移交裁决。
- **判重错误窗口测试对（mod.rs 同目录/异目录两测）裁决保留**：2 次出现、断言意图（同目录命中 vs 异目录也命中）不同，提取共享 setup 会模糊边界文档化；如 architect 认为应提取，属测试基建微调。

### 移交建议
- 下一步运行 `six-pack/architect`（six-pack 链顺序）：消化上表三项裁决对象 + 常规架构评审；hardender 已关闭，architect 之后按链进 QA（QA 轮接手前先重建 e2e 依赖，见遗留受限项）。
- 操作者直接复测（release 二进制需 `cargo build --release`——本轮仅 debug 测试面）：同 URL（bt/http）异目录添加 → 错误窗口 `该链接的任务已存在`；CLI 同 URL 二次 → toast `⚠ 该链接的任务已存在`；异 URL 同名 → `.1` 递增照旧——行为与 bug27 轮交接完全一致（本轮零行为变更）。
