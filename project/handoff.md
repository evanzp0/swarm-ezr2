# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect（架构评审与改进轮，本轮已完成）
> 路径：specifier → coder → cleaner → architect（2026-11-06）→ hardender（操作者指示跳过并关闭）→ QA → Done
> 　　→ 复验轮（2026-11-06）→ specifier 规格修订轮（v1.45 守卫）→ specifier 审批改判轮（v1.46）
> 　　→ coder 实现轮（v1.45+v1.46）→ coder bug26 限速修复轮 → coder bug27 修复轮 → cleaner 清理轮（2026-11-07）
> 　　→ **architect 评审与改进轮（本轮，2026-11-07）**
> 会话：ezr-architect-20261107（任务 = bug27 清理轮之后的四阶段架构评审 + 五项移交裁决 + 保持行为改进 + 属性测试 + 架构检查扩展；上游交接 = project/handoff.md（cleaner 轮））
> 环境注记：本沙箱全新重建——rustup 1.29.3 + nightly 1.101.0（minimal+rustfmt+clippy，
> rsproxy 双变量前台装齐，PATH=~/.cargo/bin；registry rsproxy sparse）；度量工具 PMD CPD
> 7.29.0 落位 `swarm/.tools/pmd`（Java 21）；cargo-llvm-cov / cargo-mutants 本轮未装
> （architect 不跑覆盖率/变异；下游按需重装，notes/environment.md 有口径）。
> **pyte/torf/libtorrent 与 harness 夹具仍未装**（QA 轮接手时按 notes/environment.md 重建）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui 交互层 + tokio 引擎）。需求权威来源 = `mission.md`（分期模式）+ `mission/` 4 份 phase 详述（phase-01 v1.47 / phase-02 v1.14）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier | features/ 全套 Gherkin 规格 + qa/ 全套 QA 套件；phase-01 v1.47 / phase-02 v1.14 | 已交付（bug27 轮含 v1.45 守卫 / v1.46 审批改判 / bug27 判重改判三轮修订） | 见 coder 各轮对账 |
| coder 历轮 | ezr 产品实现 + 12 集成测试目标 + bin 单测；bug6–bug27 修复 | 已交付 | 历轮全绿对账 |
| cleaner 历轮（含 bug27 清理轮） | 多轮保持行为清理 + 四件度量（llvm-cov/CRAP/DRY/变异点扫描）；bug27 批次三单点化 | 已交付 | bin 949P/0F、集成 6357P/0F、clippy 0、fmt 干净 |
| architect（2026-11-06） | 架构评审：规则 14 白名单、engine↔bt 枢纽、PieceMap 不变量 | 已交付 | 建议项归档 notes |
| hardender | —（操作者指示跳过并关闭，链路径改道） | 不适用 | — |
| QA（2026-11-06） | 对话框焦点几何、fixture cd=、套件过滤机制 | 已交付 | e2e 套件实跑 |
| 复验轮（2026-11-06） | KeepAlive 90s 一处产品变更；受限项 15→2 | 已交付 | 集成精确对账 |
| architect 本轮（2026-11-07） | 四阶段评审 + 五项裁决 + 测试基建收编 + 属性测试 +2 + arch_check 17 规则（详见第二节） | 已交付 | bin 951P/0F、集成 6357P/0F、clippy 0、fmt 干净、CPD 181 |

### 遗留受限项（当前有效）
- **leech 判据无 e2e 自动化**（six-pack 链唯一长期有效受限项，历轮维持）。
- **变异实跑未执行**：cleaner 只做 scan/count；hardender 路径已被操作者关闭，变异加固收益按 `--file` 分块口径悬置待裁决（本轮 architect 裁决：交互层三文件不拆，见第二节待批）。
- **e2e QA 依赖本轮沙箱未安装**：pyte/torf/libtorrent 未装、tracker/seeder 夹具未启动——QA 轮接手需先按 notes/environment.md（镜像口径 + harness README）重建，否则 e2e 套件不可跑。

## 二、当前产出情况

### 本会话产物清单（净行为变更 = 0，净产品 API 面 = 0）
- `project/ezr/src/app/dialogs.rs`：**测试基建 DRY**——8 处测试 `Dialog { … }` 字面量（179/165/142tok 克隆家族）收编 `testutil::dialog` 工厂 + 本地 `add_dlg(url, dir)` 助手（差分点参数化；逐字段等价：`set`≡`with_value`，`ck_type/ck_sel` 显式覆写 CK_NONE）；`parse_speed_limit`/`fmt_speed_limit` 可见性 `pub(super)`→`pub`（私有模块 `mod dialogs` 内，外部 API 面零增量）。
- `project/ezr/src/app/mod.rs`：判重错误窗口测试对提取 `dup_hit_app(tag, dialog_dir)` 共享 setup（差分点=对话框目录参数化，两测断言意图文案保留）；新增 `#[cfg(test)] pub(crate) use dialogs::{fmt_speed_limit, parse_speed_limit}` 测试门面（rust.md 门面 re-export 手法）。
- `project/ezr/src/property_tests.rs`：**属性测试 +2**——`prop_speed_limit_fmt_parse_round_trip`（fmt→parse 恒等，域限 2^53 f64 整数精确带 + 单位选择封闭域断言）与 `prop_speed_limit_parse_domain`（14 单位别名 × 空格 × /s 全组合二进制乘表表征 + 未收单位拒绝）。proptest 23→25。
- `project/ezr/scripts/arch_check.sh`：**16→17 规则**——规则 16 扫描域扩至 `src/bt/`（机制层纯度原漏盖 bt 层）；新增规则 17（bt 层禁 app/ui use，bt 箭头白名单 = bt→model/engine、app→bt 纯解析、engine↔bt 枢纽）；两条阳性对照实测变红后移除复绿（尾注登记）。
- `project/notes/`：architecture.md（高 comp 定性法 + 神结构不拆裁决 + bt 护栏收口）、testing.md（字面量收编等价口径 + **2^53 属性域发现** + 单测≠属性覆盖）、environment.md（镜像复测 + 大文件资产三件）各追加本轮条目，INDEX.md 同步。
- `packs/_common/notes/rust.md`：+1 条跨项目经验（u64 fmt/parse 往返属性域必须限 2^53，固定值单测探不出 f64 舍入带）。
- `project/mission.md` / `mission/`：只读未动（architect 职责边界）。

### 五项移交裁决（cleaner → architect，本轮已裁决）
| # | 对象 | 裁决 | 理由 |
|---|---|---|---|
| ① | >100 变异点三文件（app/mod 142 / dialogs 101 / bt_dialog 100）是否按模块边界拆分 | **不拆** | 点/行 ≈0.06–0.08 属交互层常态；拆分收益（hardender `--file` 分块）无消费方；App 子结构拆分触及全模块签名，属「长分派/共享状态拆分」不安全类 |
| ② | 前置高 comp 函数族（add_bt_dialog_char 13 / dlg_confirm_modify_bt 11 / modify_bt_dialog_char 10 / parse_speed_limit 9 / overlay_open 8 / merge_trackers 7 / restore 7） | **全保留** | 复核定性：焦点路由分派与多臂守卫链 = 本质路由复杂度（臂数不变即本质），各臂已委托单一职责助手；再拆即不安全类 |
| ③ | dialogs.rs 测试 Dialog 字面量克隆家族 | **收编**（已落地） | testutil::dialog 工厂已存在，字面量与工厂差集仅 2 个语义字段；8 处全部收编，PMD 200→181 |
| ④ | CLI `--dir ""` 边界差异（CLI 落 cwd vs 对话框回退 config/默认） | **不统一**（移交操作者） | 统一属行为变更（architect 硬约束禁止）；无测试钉此边界，如需统一请操作者裁决后走 specifier 修订 |
| ⑤ | 判重错误窗口测试对是否提取共享 setup | **提取**（已落地） | 差分点单一（对话框目录），参数化后两测断言意图文案原样保留，边界文档化不受损 |

### 验证证据
| 验证项 | 结果 | 命令（cd project/ezr，CARGO_INCREMENTAL=0） |
|---|---|---|
| bin 单测（含本轮属性 +2） | **951P / 0F**（基线 949 零增减） | `cargo test --bin ezr` |
| 集成 12 目标 | **6357P / 0F**（与基线逐一同数：supervisor 716 + engine 743 + appcore 747 + throttle_err 458 + v147 730 + hardening 142 + engine_backfill 849 + slots 203 + main 2 + proxy 16 + ui_backfill 879 + v115 872） | 逐目标 `cargo test --test <名>` |
| 属性测试（独立显式命令） | **76P / 0F** | `cargo test property_tests::` |
| clippy | 0 警告 / 0 错误 | `cargo clippy --bins --tests` |
| rustfmt | 全仓干净 | `cargo fmt --check` |
| 自动化架构检查 | **17 规则全绿**（规则 16 扩域 + 规则 17 新增，阳性对照 ×2 实测生效） | `bash scripts/arch_check.sh` |
| DRY（PMD CPD 7.29.0 `-l rust`，≥70tok，`.tools/pmd`） | **全仓 181 克隆**（基线 200 → −19：Dialog 字面量家族 8 处 + 判重测试对 + 重叠窗收口；剩余克隆均为历轮在案裁决保留项） | `pmd cpd --dir src --language rust --minimum-tokens 70` |

### 对账口径
- 测试数对账：本轮**零增减**集成/baseline 测试（dialogs.rs、app/mod.rs 测试仅内部形态重构，`grep -c` 复核 8 处字面量 → 0、`add_dlg(` 9 处 = 1 定义 + 8 调用）；bin 949→951 的 +2 全部为属性测试（属性对账隔离，不计基线单测对账）；7 个挂载 app 子树套件因属性挂载各 +2（716→718、743→745、747→749、458→460、730→732、879→881、872→874——复跑计数与基线逐一相符即为绿证），5 个未挂载套件（142/203/2/16/849）不变。分项复算：6357 = 716+743+747+458+730+142+849+203+2+16+879+872 ✓ 自洽（挂载套件内属性增量已含在各自计数中）。
- 变更面：仅 `src/app/dialogs.rs`（测试区 + 2 处可见性）、`src/app/mod.rs`（测试区 + 门面）、`src/property_tests.rs`（纯追加）、`scripts/arch_check.sh`——与 `src/bt/`、`src/engine/`、`src/ui/` 零接触。
- 差异澄清：集成 12 目标复跑计数与基线**完全一致**（716/743/747/458/730/142/849/203/2/16/879/872）——属性测试只挂载于 bin 套件（`src/property_tests.rs` 为 main.rs 的 cfg(test) 模块，不进 tests/ 挂载树），上句「挂载套件 +2」不成立，特此更正：12 目标计数零变化为实测量。

### 待办与待批
- **操作者裁决项 ④（沿 cleaner 登记，本轮 architect 裁决不统一）**：CLI `--dir ""` 边界差异——CLI 路径空串目录 normalize 后落 cwd，对话框路径空串走 resolve_confirm_dir 回退 config/默认。统一属行为变更，需操作者批准后走 specifier 规格修订轮。
- **操作者可否决的裁决**：①不拆 / ②保留 / ③⑤收编提取（已实施，如需回滚 `git diff` 面即上列四文件）。
- **变异加固悬置**（沿前轮）：hardender 关闭，>100 变异点三文件与高 comp 函数族的变异收益无从兑现；如重启 hardender，`cargo mutants --list --file` 分块口径沿用 cleaner 登记。
- **QA 轮接手前置**：e2e 依赖（pyte/torf/libtorrent + harness tracker/seeder 夹具）本轮沙箱未装，按 notes/environment.md 重建后再跑 QA 套件。

### 移交建议
- 下一步运行 `six-pack/QA`（six-pack 链顺序，hardender 已关闭）：消化上表待办与前置依赖重建；QA 套件设计可参照本轮五项裁决表（③⑤为测试基建变更，回归口径 = 集成 6357 不变）。
- 操作者直接复测：行为面与 bug27 轮交接**完全一致**（本轮零行为变更）——同 URL（bt/http）异目录添加 → 错误窗口 `该链接的任务已存在`；CLI 同 URL 二次 → toast `⚠ 该链接的任务已存在`；异 URL 同名 → `.1` 递增照旧。
