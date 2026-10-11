# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder 终态　会话：v128-code（任务 v128-code-cpd-tile）
> 本轮（CPD +1 tile 处置）：操作者裁决「提取共享 helper」后由 coder 执行——ui 层
> `Block::default()` 惯用样板 8 处收口为单源 `bordered_block(fg)`（TDD 契约测试先行），
> 8 调用点一行委托 + 7 文件失效 import 清理；gen-drift 夹具同步（壳补真实现转发 +
> 重生成剥头副本）。基线全链绿：2527P/0F（+4 为新契约测试在 4 个挂载 crate 各执行 1 次）、
> clippy 0、fmt 0、arch 14 规则过；CPD src-only **56 → 52 tiles**（−4 全为 Block 样板族）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.1，分期模式）+ `project/mission/phase-01.md`（**v1.18**）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（v115 前历轮） | mission/phase-01.md 全量 FR/D 裁决表 + features/ 13 份 + qa/ 13 份 | 全部交付 | parser/dry-check 全过 |
| coder（v120-code-two-batch） | FR-01-102 连接速度同管线 + FR-01-103 仅添加/立即下载三钮 | 已交付 | 2438P/0F、clippy 0 |
| coder（v121-code-batch3） | FR-01-104 1MB 块下限 + FR-01-105 实时字节口径 + D30 下调即时收缩 | 已交付 | 2470P/0F、clippy 0 |
| coder（v122-code-batch4） | FR-01-06 v1.17 双对话框粘贴 + FR-01-51 v1.17 清码三分支 + mock hold 门控根治 flake | 已交付 | 2503P/0F、clippy 0 |
| specifier（v124-spec-batch5） | FR-01-51 v1.18 直判规格 + D34 v1.18 注记 + 场景 12 注/QA-IC-12 v1.18 | 已交付 | parser + dry-check 过 |
| coder（v124-code-batch5） | 修订 A 终态 sidecar 即时落盘 + 修订 B 清码 retry 文件完整性直判 + 测试 3 条 | 已交付 | 2506P/0F、clippy 0、fmt 0、arch 12 过 |
| cleaner（v125-clean） | 保持行为清理 8 处 + 四度量复核（覆盖率/CRAP/DRY/变异点 scan） | 已交付 | 2506P/0F、clippy 0、fmt 0、arch 12 过、CPD tiles 55 |
| architect（v126-arch） | 四阶段架构评审 + 页签计数访问器提取 + 属性测试 +10 + arch 规则 13/14 + 经验沉淀 2 条 | 已交付 | 2523P/0F、clippy 0、fmt 0、arch 14 过、属性 57P/0F |
| hardender（v127-hard） | 清单制差分变异 78/78 killed + CRAP/DRY 复测对账 + 软 Gherkin 受限登记 + 经验沉淀 5 条 | 已交付 | 产品树零改动（pristine cmp 0 diff）、基线全链沿用全绿 |
| coder（v128-code，本轮） | CPD +1 tile 处置：`bordered_block(fg)` 单源 helper + 8 调用点收口 + gen-drift 夹具同步 + 契约测试 1 条 | 已交付 | 2527P/0F、clippy 0、fmt 0、arch 14 过、CPD 56→52 tiles |

### 遗留受限项（当前有效）
- e2e 六套件滞后于规程（累计）：`suite_add_task.py`、`suite_ui_conns.py`、
  `suite_modify_task.py`、`suite_persistence_config.py`、`suite_tui_display.py`、
  `suite_integrity_check.py`——QA 会话须先同步全部受影响套件再跑 e2e。
  注：本轮仅重构 UI 样板（渲染产物逐单元格不变），套件断言无需因此调整。
- **软 Gherkin 验收变异受限**：`gherkin-mutator`（bb 版）工具在位、参数契约已读毕，但其
  必需的 `--runner-worker` 项目适配器唯一实现路径绑定 specifier 体系的 e2e QA runner
  （qa/runners/）——hardender 职责排除运行/维护 QA 套件，且六套件滞后未同步（判定
  不可信）。现状：13 份 feature 中仅 `01-persistence-config.feature` 有历史 stamp
  （2026-10-06 轮，28/28 killed），其余 12 份无 stamp 无判定。**移交 QA 会话**：套件
  同步后按 `packs/_common/notes/aps.md` + mutator-spec 构建适配器补齐判定。
- **变异覆盖面受限**：v127 轮差分变异清单 = 当批 delta + consistency/speed/retry 模型
  + header 邻位，合计 78 突变体全 killed。全项目 scan 共 **2159 点/54 文件**（单文件
  最大 98，均 <100 门槛），其余文件未跑变异——受会话时间预算与每命令 600s 上限约束，
  移交后续 hardender 轮按同协议（phase-1 快杀 + phase-2 全量复核）分批推进。
  **本轮触碰面（8 个 ui 文件 + hardening 套件壳）不在 v127 killed 清单内**，如需该面
  加固证据由后续 hardender 轮补跑。
- **CRAP 为受限近似口径**（Rust 无标准 CRAP 工具；llvm-cov lcov BRDA comp=分支记录数+1
  上界近似 + 源码决策点复核，近似口径偏差方向见 `packs/_common/notes/rust.md`）：
  v127 复测 72 条近似 CRAP>6 与存量精确一致（其中 4 条 NO_DA 型）；本轮仅样板收口
  （分发/解析核心未触碰），无新增高 comp 点，CRAP 读数沿用。
- **CPD 剩余 52 tiles**（PMD 7.28，`-l rust --minimum-tokens 50`，src-only 口径，较上游
  56 **−4**，−4 全为 Block 样板族：v127 登记 tile 2 个 + 同族配对 2 个；报告中
  `Block::default`/`Borders::ALL` 成员已清零，独立 grep 计数 src/ 内该样板仅剩
  `ui/mod.rs` helper 单源本体 1 处）。残余 ui 相关 tile：delete/dialog.rs:3 起的
  import 块 tile、dialog.rs 下拉选项 spans tile（293/306，v127 报告中即存在，行号漂移
  所致）——均为惯用/既有形态，沿用登记。补充口径 src+tests = **111 tiles**（上游 115，
  同步 −4：`tests/gen/detail_stripped.rs` 夹具副本随单源化同步消解）。
  `fn seed()` 测试夹具 tile（app/engine/mod.rs + app/mod.rs，7 行 53 tokens）维持
  「夹具形态登记维持」。
- **本轮沙箱工具链状态**（沿袭 v127 登记）：rustup nightly（rustc/cargo
  1.101.0-nightly，minimal + rustfmt + clippy + llvm-tools-preview），`~/.cargo` +
  `~/.rustup`，新 shell 需 `export PATH="$HOME/.cargo/bin:$PATH"` 补注入；cargo-mutants
  27.1.0、cargo-llvm-cov 0.9.1、rustfilt（均 `~/.cargo/bin`）、PMD 7.28.0
  （`.tools/pmd-bin-7.28.0`，Java 21）、Babashka 1.12.197（`.tools/bin/bb`）+ APS 仓
  （`.tools/Acceptance-Pipeline-Specification`，APS 调用须 cd 到该目录）。
- 无其他受限项。

### 实现定义值登记（当前有效）
- 本轮无新增实现定义值。沿袭有效：D28/D29（仅添加落态与三钮布局）、D30（下调即时收缩）、
  D31（1MB 下限）、D32（Enter 语义）、D33（1s 归零时延）、D34（清码 R = 不校验直接收尾，
  v1.18 实现精化见 coder v124-code-batch5 轮）。修订 B fallback（文件不完整 → 作废断点
  从头重下）为 FR-01-22 作废语义同源的实现定义值，操作者可改判（见待办）。

## 二、当前产出情况（coder v128-code-cpd-tile）

### 本会话产物清单
- `src/ui/mod.rs`：新增 `pub(super) fn bordered_block(fg: Color) -> Block<'static>`
  （圆角全边框面板单源：`Block::default().borders(ALL).border_type(Rounded)
  .border_style(fg)`）；`ui_tests` 新增渲染契约测试
  `bordered_block_renders_rounded_full_borders_with_fg`（TestBackend 断言四角 Rounded、
  四边齐、边框前景色 = 入参，BORDER/ACCENT/RED 三色巡检）。
- 8 处调用点一行收口（各自后续 `.title(...)` 链保留原样）：`header.rs` / `footer.rs` /
  `list.rs` / `conns.rs` / `detail.rs`（BORDER）、`delete.rs`（RED）、`dialog.rs` ×2
  （ACCENT）；7 个文件的 `Block/BorderType/Borders` 失效 import 清理。
- `tests/hardening_ui_backfill.rs`：`detail_fix` 壳补 `bordered_block` 真实现转发
  （与既有 text 转发同手法，非同值复刻）。
- `tests/gen/detail_stripped.rs`：按 gen-drift 守卫口径重新生成（产品源去 `//!` 头块）。
- `project/handoff.md`：本文件重写。

#### 交接四要素

**① 本地验证命令清单**（cwd：`project/ezr/`；新 shell 先
`export PATH="$HOME/.cargo/bin:$PATH"`）：
```sh
cargo test --workspace                                   # 全量 2527P/0F
cargo clippy --all-targets                               # 0 警告
cargo fmt --check                                        # 0 偏差
bash scripts/arch_check.sh                               # 14 规则全过
cargo test --bin ezr property_tests::                    # 属性独立口径 57P/0F（未触碰，沿用）
cargo test --test hardening_ui_backfill gen_fixtures_match_product_sources  # gen-drift 守卫
/home/z/swarm-ezr2/.tools/pmd-bin-7.28.0/bin/pmd cpd -l rust --minimum-tokens 50 --dir src        # DRY src-only（52 tiles）
/home/z/swarm-ezr2/.tools/pmd-bin-7.28.0/bin/pmd cpd -l rust --minimum-tokens 50 --dir src --dir tests  # src+tests（111 tiles）
```

**② 会话运行时产物位置**（`.work/tmp/`，`complete` 后随会话清除）：
- `cpd_now.log`（开工基线 56 tiles）、`cpd_after.log`（终态 52 tiles）、
  `cpd_srctests_after.log`（111 tiles）、`test_full.log`（全量单测日志）。
- CPD 验收对账方法：`rg -c "Found a .* duplication" <log>` 计 tile 数；
  `rg "Block::default|Borders::ALL" <log>` 确认样板族成员清零。

**③ feature 注释块与差分复用注意事项**：`features/01-persistence-config.feature` 头部含
`# mutation-stamp: sha256=…` 注释块（28/28 killed）——内容未变时差分复用跳过全部场景，
强制重跑须删除该注释块；其余 12 份无注释块。本轮未改任何 feature。

**④ 等价突变体清单**：不适用（本轮未跑变异；v127 轮 78/78 killed 空清单口径见其总账行）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量单测 | **2527P / 0F**（基线 2523 + 新契约测试 ×4 执行） | `cargo test --workspace` |
| clippy / fmt / arch | 0 / 0 / 14 规则过 | 见①命令清单 |
| 契约测试（TDD） | 先红 E0425 → 后绿 1P/0F | `cargo test --bin ezr bordered_block` |
| gen-drift 守卫 | 过（夹具重生成后同步） | `cargo test --test hardening_ui_backfill gen_fixtures` |
| DRY src-only | **56 → 52 tiles**，Block 样板族成员清零 | 见① pmd 命令 + ② 对账方法 |
| DRY src+tests | 115 → **111 tiles**（备查口径同步 −4） | 见① pmd 命令 |
| 样板单源 grep | src/ 内 `Block::default()` 仅 `ui/mod.rs` helper 本体 1 处 | `rg -c "Block::default\\(\\)" src/` |

### 对账口径（基线 → 终态）
- 单测 2523 → 2527：**+4** = 本轮唯一新增测试
  `ui_tests::bordered_block_renders_rounded_full_borders_with_fg` 在 4 个编译
  `src/ui/mod.rs` 的测试目标（bin ezr；经 `#[path]` 挂载的 hardening_ui_backfill、
  hardening_engine_backfill、hardening_v115）各执行 1 次（全量日志中该测试名出现
  4 次与 +4 自洽）；无删改既有测试，其余分项不变。
- CPD 56 → 52：**−4** 全部归属 Block 样板族（v127 登记的 header+list、detail+list 两个
  tile + dialog:68/202 等同族成员参与的另 2 个配对 tile）；残余 ui 相关 tile 与 BEFORE
  报告逐条对照均为既有形态（import 块、下拉 spans，行号漂移所致位置变化）。
- src+tests 115 → 111：同族 −4 夹具副本（detail_stripped.rs）随单源化消解。

### 待办与待批（未决项）
- **已闭环**：v127 登记的「CPD +1 tile 处置（移交 coder/architect 裁决）」——操作者
  2026-10-11 裁决「提取共享 helper，由 coder 处理」，本轮执行完毕（总账行 v128-code），
  该条目移出待办。
- 沿袭待批：修订 B fallback（文件不完整 → 作废断点从头重下）为实现定义值，操作者可改判
  （如改判「保留台账从缺失处补下」，代价：稀疏文件空洞风险，不建议）。
- 无新增待批。

### 移交建议
- 下一角色：**six-pack/cleaner**（链序 coder → cleaner）。建议要点：① 四度量复核以本轮
  终态为基线（CPD 52 tiles / 单测 2527P）；② delete/dialog.rs:3 import 块 tile 与
  dialog 下拉 spans tile 的「既有形态维持 vs 收口」由 cleaner 按其职责复审（本轮按登记
  维持未动）；③ 变异触碰面缺口（本轮 8 个 ui 文件）移交后续 hardender 轮。
