# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner　会话：ezr-v152-clean（2026-10-25）
> 上游交接：project/handoff.md（coder v135-impl-fieldwin 交接：v1.35 pieces 布局实现 +
> field_window 振荡缺陷修复，建议下一步 cleaner）
> 当轮任务：v135-impl-fieldwin 交接批次的保持行为清理 + 覆盖率/CRAP/DRY/变异点
> 四项度量基线重建（v151-arch 未重建覆盖率度量管线的欠账一并补齐）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.35，分期模式）+ `project/mission/phase-01.md`（v1.35）。
- 本轮后源码状态：v1.34 全条款落地 + 历轮 UI/加固/覆盖轮 + 输入框组件化 +
  退出路径鼠标残序根治（v150）+ 清理批次（v150-clean）+ 架构轮（v151-arch）+
  v1.35 规格修订（specifier）+ v1.35 实现 + field_window 振荡缺陷修复（coder）+
  **本轮清理轮（cleaner）**——全量 **4564P/0F**（`--test-threads=2`，15 测试目标）。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.35 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | v132 轮 3 份 + v149 轮 5 份 feature APS 机检沿袭未跑 |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F |
| architect（v145-arch） | supervisor 六文件拆分 + 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 | 已交付 | 3363P/0F |
| QA（ezr-qa-final / v148-qa 等） | 13+1 套件 e2e 可执行化 + llvm-cov 覆盖轮 | 已交付 | e2e 167P/0F（v148 基线态） |
| coder（v129–v140 系列） | FR-01-106 实现 + v1.30/v1.31 UI 改版 + FR-01-112 内置 DNS | 已交付 | 4162P/0F |
| cleaner（v141）/ architect（v146） | dns 清理 + 四阶段评审 | 已交付 | 4200P/0F、arch 15/15 |
| coder（v134-demo-sync-impl） | v1.34 全条款实现（FR-01-113…116 + 修订十二条） | 已交付 | 4300P/0F |
| coder（v134-uifix 系列 ×3） | 三类 UI bug 修复 + 输入框原生光标 | 已交付 | 4388P/0F、clippy 0、fmt clean |
| coder（v135-fieldcomp-impl） | ①输入框组件化（src/textfield.rs）；②ezr-demo 同步 | 已交付 | ezr：4484P/0F；ezr-demo：18P/0F + 回归 8 套件全绿 |
| coder（v136/v137 中间轮） | 退出路径首轮防泄漏 | 已交付 | 操作者实测仍复现（v150 根因所据） |
| coder（v150-quitmouse-impl） | 退出路径鼠标残序根治 + ezr-demo 全量同步 + TimedWriter 时序锁定测试 | 已交付 | ezr：4486P/0F、clippy 0、fmt clean；ezr-demo：20P/0F、回归 8 套件全绿 |
| cleaner（v150-clean） | mouse.rs 弹窗命中臂 +4 测、CRAP 门禁 6→2、rect_hit 单源化、度量基线重建 | 已交付 | ezr：4518P/0F、clippy 0、fmt clean；覆盖率 95.1% 行 |
| architect（v151-arch） | 模块边界收口（scrollbar.rs）+ on_mouse_main 拆分 + 属性 71 + arch 16 规则 | 已交付 | ezr：4532P/0F（15 目标）、clippy 0、fmt clean、arch 16/16、属性 71P/0F；变异点 3232 |
| specifier（v135-pieces-sync） | v1.35 规格修订：pieces 弹窗布局对齐 ezr-demo README 记录 21+22（68×20/八段/31 块/341，逐字断言经 demo 源码核证） | 已交付 | 文档与源码一致无差异登记；沿袭测试基线 4532P/0F 不变 |
| coder（v135-impl-fieldwin） | ①v1.35 pieces 布局实现（68×20 八段、31 块/行、341/屏）；②field_window 振荡缺陷修复 + 属性论域扩回全字母表 | 已交付 | ezr：4548P/0F（15 目标）、clippy 0、fmt clean、arch 16/16；属性 3 测全字母表 5 万例复跑全绿 |
| **cleaner（v152-clean，本轮）** | **①孤儿 fixture `project/ezr/.tmp` 删除裁决（422B 误入库测试残留，src/tests 零消费点 rg 全量核查）；②`task_with_conns` 测试夹具从 keys.rs 上提 app/testutil.rs 共享单源（keys 10 调用点改走共享）；③mouse.rs 纯追加 2 测覆盖 main_down_conns 全臂（CRAP 门禁收敛杠杆）；④field_window `window_for`/`fill_cols` 职责划分评审裁决（不上提，登记依据）；⑤覆盖率/CRAP/DRY/变异点四项度量基线重建（CRAP 解析器按 notes 口径重建 + v150 口径逆向还原验证）** | **已交付** | **ezr：4564P/0F（15 目标，--test-threads=2）、clippy 0（-D warnings）、fmt clean、arch 16/16；覆盖率 95.2% 行；CRAP>30 门禁 1 条（仅 handle_socks5 豁免类，80.2 与 v150 登记值精确一致）；CPD src 口径 137 tiles（开工中性）；变异点 3247（+15 全落 textfield.rs，逐文件对账自洽）** |

### 遗留受限项（当前有效）
- **APS 机检沿袭未执行**：v132 修订 3 份 + v149 修订 5 份 feature 未过
  `gherkin-ir-dry-checker`；待环境可用时一并补跑。
- **qa/runners/ e2e 套件欠账（移交 QA）**：`suite_ui_pieces.py` 仍为 v1.30 口径
  （断言 64×19），05 规程已修订至 v1.35（68×20/31 块/341）——待 QA 轮随其余
  套件（v1.30–v1.32 口径对 v1.34/35 场景会 F，属预期）一并同步。
- **demo 回归长脚本沙箱卡顿（沿袭）**：verify_ezr / test_focus_style 连续全量跑在
  沙箱内中后段随机挂起，按场景子集分半跑全绿（v135/v150 两轮复证）；本轮未改动
  ezr-demo，未重跑 demo 回归（无触碰面）。
- **真实 TCP stub e2e 时序脆弱（v147 沿袭）**：`--test-threads=2` 全绿即环境竞争非回归。
- **dns.rs 覆盖率残余 3 行 / 观察受限突变体（沿袭登记）**：状态不变。
- **ezr-proxy 测试基建豁免（沿袭登记）**：ezr-proxy.rs 覆盖率口径随全量 lcov
  （handle_socks5 CRAP 80.2 本轮复测值与 v150 登记值精确一致），随此豁免登记。
- **CRAP>30 残余口径（移交 hardender，当前 1 条）**：`handle_socks5`
  （ezr-proxy.rs:342，随 ezr-proxy 豁免）。`on_mouse_main` 拆分复测遗留项**已销账**：
  拆分后各臂全部 ≤30（v151-arch 未重建度量管线的欠账本轮补齐），最后一处越门禁的
  `main_down_conns`（comp 9、cov 23%、CRAP 46.0）经本轮 +2 测收敛至 ≈9。
- **textfield.rs 4 未覆盖行（登记不硬凑）**：340-341（超宽取窗硬上限兜底臂——
  终止性论证保证不可达，防御未来演化破坏论证前提的保留分支）+ 359/368（未聚焦
  路径 if 链穷举臂——未聚焦溢出取窗恒 left_cut/无 right_cut，结构性不可达）。
  纯追加测试无法覆盖，删除削弱防御，登记口径留 hardender/操作者裁决。
- **上游 CPD tile 漂移登记（移交 architect，非本轮回归）**：开工态较 v150-clean
  +3 tiles（loose 口径 6 新/-3 消），形态归因：4 tile = v151-arch scrollbar 模块
  拆分的测试脚手架同构（scrollbar.rs 4 处自配对 + 与 mod/keys/mouse/pieces 跨文件
  配对）、1 tile = v135-impl property_tests.rs 两属性策略脚手架自配对——全部为
  惯用样板/夹具形态，按 notes 处置登记移交，拆样板依赖 architect 模块边界裁决。
- **>100 变异点文件 7 处（移交 hardender/architect，沿袭口径）**：scrollbar.rs 190 /
  mouse.rs 151 / textfield.rs 140（+15 本轮重构净增，其余文件 +0）/ keys.rs 137 /
  ezr-proxy.rs 127 / app/mod.rs 108 / ui/text.rs 101——按 notes 规则不硬拆，
  `--file` 分块加固前由 architect 先收模块边界。
- **D40 / PC-09-场景09 待操作者裁决（沿袭）**：状态不变。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D43、MOUSE_SETTLE=50ms / MOUSE_SETTLE_RETRY=20ms、
  drain poll 超时 5ms。本轮零新增（清理轮不涉产品契约值）。

## 二、当前产出情况（cleaner v152-clean）

### 本会话产物清单
- **`ezr/src/app/mouse.rs`（纯追加测试）**：+2 测覆盖 `main_down_conns` 全臂——
  面板内聚焦 + 表头行不选行 + 数据行选中（`ci = conns_scroll + idx`）；
  面板缺省/矩形外不聚焦 + `idx ≥ visible_conns_rows` / `ci ≥ 连接数` 双钳制 +
  回落界内正常选中。附共享夹具 `seed_conns_panel`（消两测脚手架自配对 CPD tile）。
- **`ezr/src/app/testutil.rs`（夹具共享单源）**：`task_with_conns` 从 keys.rs
  conn_key_tests 上提至共享测试夹具模块（pub(crate)），keys.rs 10 处调用点改走
  共享（消未来第二拷贝的 DRY 收口）。
- **`ezr/src/app/keys.rs`（测试区）**：本地 `task_with_conns` 定义删除，改
  `use crate::app::testutil::task_with_conns`；随定义退役的 `Connection` 导入一并清理。
- **`project/ezr/.tmp`（删除裁决）**：hardender 轮误入库的 sidecar 快照 JSON
  （git 索引内跟踪、422B mock 元数据、src/tests 零消费点 rg 全量核查、非
  gitignore 排除形态）——删除；归档内容随之收敛（v150-clean 移交裁决项闭环）。
- **`packs/_common/notes/rust.md`（经验沉淀 ×2）**：①「comp 口径漂移」条——
  CRAP 受限近似解析器的 comp 换算式必须随门禁基线登记（两种口径对同一 lcov 的
  >30 清单可差 10 倍；反推手法 = 用历史登记 CRAP 读数代入公式反解 comp 并精确
  复现）；②「FN 记录收集解析器边界」条追加第 ④ 边界——嵌套命名 fn 的 mangled
  名是父名路径扩展，须按前缀归并进父函数跨距（父 comp 塌缩/嵌套 comp 虚高的
  实证形态）。
- **`packs/_common/notes/cargo-mutants.md`（经验沉淀 ×1）**：版本探针勿用
  `-V`/`--version`（27.1.0 在项目内会直接开扫而非打印版本），版本口径取发布 tag。
- **环境（沙箱重建，随交接）**：rustup 1.29.2 + nightly 1.101.0-nightly
  （2026-10-24，rustfmt/clippy/llvm-tools 组件齐）经 rsproxy.cn 镜像安装
  （`RUSTUP_DIST_SERVER`/`RUSTUP_UPDATE_ROOT`）；`~/.cargo/config.toml` 已配
  rsproxy-sparse crates 镜像。度量工具 `.tools/`：cargo-mutants 27.1.0、
  cargo-llvm-cov 0.9.1（GitHub release 预编译二进制，经 gh-proxy 直链）、
  PMD 7.28.0（pmd-dist bin zip，Java 21）。新 shell 需
  `export PATH="$HOME/.cargo/bin:$PATH"` 补注入；rustup 直调命令在受限沙箱会挂
  网络解析，以 `export RUSTUP_TOOLCHAIN=nightly-x86_64-unknown-linux-gnu` 绕过
  （notes/rust.md 既有条目）。

### 验证证据

| 验证项 | 结果 | 命令/方法（可复现） |
|---|---|---|
| 全量测试 | **4564P / 0F**（15 目标） | `cargo test -- --test-threads=2`（project/ezr） |
| 增量对账 | 4548 → 4564 = **+16 = 新增 2 测 × 8 个挂载 app 树的测试目标**（bin 1 + hardening_g_* 7，逐目标 `test result` 复算自洽） | `grep "^test result:" <log>` 逐目标比对 |
| clippy 零警告 | 0（-D warnings 门禁） | `cargo clippy --all-targets --all-features -- -D warnings` |
| fmt | clean | `cargo fmt -- --check` |
| 架构护栏 | 16/16 规则通过 | `bash scripts/arch_check.sh`（project/ezr） |
| 覆盖率 | **95.2% 行**（20183/21205，73 文件；开工 95.1%）——textfield 99.1%（4 未覆盖行登记）、pieces 100%、mouse 90.8%→94.5% | `cargo llvm-cov --no-clean --lcov --branch --output-path <lcov>`（两段前台分跑，度量后删 llvm-cov-target 还盘） |
| CRAP 门禁 | **>30 共 1 条**（开工口径复现 2 条：handle_socks5 + main_down_conns；本轮收敛至 1 条） | 解析器按 notes 口径重建 + v150 换算式逆向还原（comp = 1 + BRDA 记录数/2，handle_socks5 复算 80.2 与登记值精确一致）；`python3 .work/tmp/crap_gate.py <lcov>` |
| DRY | **src 口径 137 tiles**（v150 基线 134 → 开工 137 上游漂移 → 终 137 本轮中性：+1 自配对即时消解） | `.tools/pmd/bin/pmd cpd -l rust --minimum-tokens 50 --dir src`（阳性对照先行：构造重复对 106tok 识别通过）；tile 级 diff 归因经 v150/开工双 worktree 对照 |
| 变异点扫描 | **全量 3247 点**（v151-arch 3232，+15 全落 textfield.rs 125→140，其余文件 +0） | `cargo mutants --list --line-col=true`（27.1.0） |
| 新增测试 | 2P / 0F（conns_click_focuses_and_selects_data_row / conns_click_clamps_selection_and_misses_outside） | `cargo test --bin ezr conns_click` |

### 对账口径（与 coder v135-impl-fieldwin 基线）
- 测试计数：4548 → 4564 = +2 新测 × 8 挂载目标（无删除）。
- 源码触碰面：产品区零改动（纯测试追加 + 测试夹具上提 + 孤儿 fixture 删除）；
  具体文件 `src/app/mouse.rs`（测试区）、`src/app/testutil.rs`、`src/app/keys.rs`
  （测试区）、`.tmp`（删）；textfield.rs / ui/pieces.rs / property_tests.rs 零改动
  （coder 产物原样保留，评审裁决为不上提/不提取）。
- 度量口径：覆盖率/CRAP/DRY/变异点四管线全部重建并落账，换算式与工具版本
  随表可复现（CRAP comp = 1 + BRDA 记录数/2，经 80.2 精确复现验证为 v150 口径）。

### 待办与待批（未决项）
- **沿袭项全部不变**（见第一节遗留受限项）；本轮无新增阻塞、无新增待批。
- `.tmp` 删除裁决与 field_window 评审裁决已在本轮落账闭环（前者直接执行、
  后者登记不上提依据），无遗留待批。

### 移交建议
- 建议下一步运行 `six-pack/architect`（角色链顺序）：本轮登记的 6 个上游 CPD
  tile 漂移（scrollbar 拆分脚手架 + property 策略脚手架）与 >100 变异点 7 文件的
  模块边界裁决都在 architect 职责面；textfield.rs 140 点的 `--file` 分块口径
  也待其收口。
- 或按 v151-arch 原计划跳 `six-pack/hardender`：CRAP 门禁已收敛至豁免类 1 条、
  覆盖率/变异点基线已重建，变异点批次文件口径（本轮第一节登记的 7 文件清单）
  可直接作为 `--file` 分块依据；textfield.rs 振荡回归 2 测 + 本轮 main_down_conns
  2 测可作为加固基线的一部分。
