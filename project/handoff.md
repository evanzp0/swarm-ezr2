# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect　会话：ezr-v153-arch（2026-10-25）
> 上游交接：project/handoff.md（cleaner v152-clean 交接：v152 清理批次 + 四项度量基线
> 重建，建议下一步 architect）
> 当轮任务：v152-clean 交接批次的架构轮——上游登记的 CPD tile 漂移（scrollbar 拆分
> 测试脚手架 + property 策略脚手架）模块边界裁决与收口、>100 变异点 7 文件分块口径
> 裁决、四阶段架构评审、属性测试支持。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.35，分期模式）+ `project/mission/phase-01.md`（v1.35）。
- 本轮后源码状态：v1.34 全条款落地 + 历轮 UI/加固/覆盖轮 + 输入框组件化 +
  退出路径鼠标残序根治（v150）+ 清理批次（v150-clean）+ 架构轮（v151-arch）+
  v1.35 规格修订与实现 + field_window 振荡缺陷修复（coder）+ 清理轮（v152-clean）+
  **本轮架构轮（v153-arch）**——全量 **4564P/0F**（`--test-threads=2`，15 测试目标），
  产品代码零改动（逐文件 hunk 边界核对：全部变更落在 `#[cfg(test)]` 区与测试夹具模块）。

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
| coder（v150-quitmouse-impl） | 退出路径鼠标残序根治 + ezr-demo 全量同步 + TimedWriter 时序锁定测试 | 已交付 | ezr：4486P/0F、clippy 0、fmt clean；ezr-demo：20P/0F、回归 8 套件全绿 |
| cleaner（v150-clean） | mouse.rs 弹窗命中臂 +4 测、CRAP 门禁 6→2、rect_hit 单源化、度量基线重建 | 已交付 | ezr：4518P/0F、clippy 0、fmt clean；覆盖率 95.1% 行 |
| architect（v151-arch） | 模块边界收口（scrollbar.rs）+ on_mouse_main 拆分 + 属性 71 + arch 16 规则 | 已交付 | ezr：4532P/0F（15 目标）、clippy 0、fmt clean、arch 16/16、属性 71P/0F |
| specifier（v135-pieces-sync） | v1.35 规格修订：pieces 弹窗布局对齐（68×20/八段/31 块/341） | 已交付 | 沿袭测试基线 4532P/0F 不变 |
| coder（v135-impl-fieldwin） | ①v1.35 pieces 布局实现；②field_window 振荡缺陷修复 + 属性论域扩回全字母表 | 已交付 | ezr：4548P/0F（15 目标）、clippy 0、fmt clean、arch 16/16 |
| cleaner（v152-clean） | 孤儿 fixture 删除裁决 + task_with_conns 夹具共享单源 + mouse.rs +2 测（CRAP 收敛杠杆）+ field_window 职责评审裁决 + 四项度量基线重建 | 已交付 | ezr：4564P/0F（15 目标）、clippy 0、fmt clean、arch 16/16；覆盖率 95.2% 行；CRAP>30 门禁 1 条（豁免类）；CPD 137 tiles；变异点 3247 |
| **architect（v153-arch，本轮）** | **①四阶段架构评审（UI/核心分离、依赖规则、信息隐藏、局部质量——结论：分层基线无漂移，16 条护栏全部在位且必要）；②app 树 App 夹具族收口 testutil 单源（make_app_in/dl_task/dl_task_mb/seed_queued 四 helper + 10 文件测试区一行委托，对齐 ui::testfx 既有先例）；③属性策略构造三单源 helper（arb_task_states/slot_task_vec/field_window_of，生成分布不变）；④>100 变异点 7 文件分块裁决（全部整文件 shard，依据逐文件登记，见当前节）** | **已交付** | **ezr：4564P/0F（15 目标，--test-threads=2，rc=0）、clippy 0（-D warnings）、fmt clean、arch 16/16；CPD 137→125 tiles（PMD 7.28.0，登记漂移族全部消解）；属性 71P/0F（数量守恒）；变异点 3247 守恒（产品区零改动）** |

### 遗留受限项（当前有效）
- **APS 机检沿袭未执行**：v132 修订 3 份 + v149 修订 5 份 feature 未过
  `gherkin-ir-dry-checker`；待环境可用时一并补跑。
- **qa/runners/ e2e 套件欠账（移交 QA）**：`suite_ui_pieces.py` 仍为 v1.30 口径
  （断言 64×19），05 规程已修订至 v1.35（68×20/31 块/341）——待 QA 轮随其余
  套件（v1.30–v1.32 口径对 v1.34/35 场景会 F，属预期）一并同步。
- **demo 回归长脚本沙箱卡顿（沿袭）**：verify_ezr / test_focus_style 连续全量跑在
  沙箱内中后段随机挂起，按场景子集分半跑全绿（v135/v150 两轮复证）；本轮未改动
  ezr-demo，未重跑 demo 回归（无触碰面）。
- **真实 TCP stub e2e 时序脆弱（v147 沿袭，本轮复证）**：本轮首轮全量中
  `single_stream_downgrade_emits_toast`（hardening_g_v147）在 `--test-threads=2`
  下假失败 1 次，单跑 0.39s 通过、复跑全绿——环境竞争非回归；全绿口径以
  `--test-threads=2` 复跑为准。
- **dns.rs 覆盖率残余 3 行 / 观察受限突变体（沿袭登记）**：状态不变。
- **ezr-proxy 测试基建豁免（沿袭登记）**：ezr-proxy.rs 覆盖率口径随全量 lcov
  （handle_socks5 CRAP 80.2 为 v150 登记值）。
- **CRAP>30 残余口径（移交 hardender，当前 1 条）**：`handle_socks5`
  （ezr-proxy.rs:342，随 ezr-proxy 豁免）。
- **textfield.rs 4 未覆盖行（登记不硬凑）**：340-341 + 359/368（结构性不可达
  防御分支，v152 登记口径）；状态不变，留 hardender/操作者裁决。
- **D40 / PC-09-场景09 待操作者裁决（沿袭）**：状态不变。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D43、MOUSE_SETTLE=50ms / MOUSE_SETTLE_RETRY=20ms、
  drain poll 超时 5ms。本轮零新增（架构轮不涉产品契约值，产品代码零改动）。

## 二、当前产出情况（architect v153-arch）

### 本会话产物清单
- **`ezr/src/app/testutil.rs`（App 夹具族单源，+4 helper）**：`make_app_in(prefix, tag)`
  （唯一临时目录 + 默认配置构造 App，与 `ui::testfx::make_app_in` 同型）、
  `dl_task(id, name)`（下载中基准任务：state=Downloading/已探测/total 5MB）、
  `dl_task_mb(id, name)`（同上 + block_size=1MB 十进制，分块数学族口径）、
  `seed_queued(app, n)`（n 个排队任务播种）。
- **app 树 10 文件测试区夹具单源委托**（删除 14 处 make_app 同构克隆、5 处
  dl_task 克隆、4 处播种循环）：`app/mod.rs`（两模块）、`app/keys.rs`（四模块，
  pkey 的 dl_task 委托 dl_task_mb）、`app/mouse.rs`（两模块）、`app/scrollbar.rs`
  （make_app 委托 + 四测共享 `seed_scrollbar` 脚手架）、`app/paste.rs`、
  `app/pieces.rs`（openable 一行委托 dl_task_mb）、`app/dialog_keys/mod.rs`、
  `app/engine/mod.rs`（三模块；`make_app_with_reg` 为独立夹具保持原样）、
  `ui/pieces.rs`（委托本层 `ui::testfx::make_app_in`）。临时目录命名逐字不变
  （`<prefix>-<tag>-…`）；产品区零改动。
- **`ezr/src/property_tests.rs`（策略构造单源化，+3 helper）**：
  `arb_task_states(n)`（六态 TaskState 向量策略，slots/registry 两属性共享）、
  `slot_task_vec(n, states, flags, retry_in)`（slot 两属性共享播种）、
  `field_window_of(s, cursor, budget, focused)`（textfield 取窗四属性共享构造）。
  生成分布与断言零改动，属性数 71 守恒。
- **`packs/_common/engineering.md`（经验沉淀 ×2）**：①「发行资产形态会随版本漂移」
  条追加——大资产（百 MB 级）GitHub 直链中段超时换 `gh-proxy.com/` 前缀代理重试；
  ②新增「多步批量编辑失败后先重读落盘状态」条——批量编辑工具非原子，失败后先
  核对哪些子编辑已生效再补做（重复定义与行号漂移两类实证形态）。
- **环境（沙箱重建，随交接）**：rustup 1.29.2 + nightly 1.101.0-nightly
  （2026-10-24，rustfmt/clippy 组件齐）经 rsproxy.cn 镜像安装
  （`RUSTUP_DIST_SERVER=https://rsproxy.cn` + `RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup`，
  操作者钦定）；`~/.cargo/config.toml` 已配 rsproxy-sparse crates 镜像 +
  `RUSTUP_TOOLCHAIN=nightly-x86_64-unknown-linux-gnu`（绕 rustup 网络解析，
  notes/rust.md 既有条目）。度量工具 `.tools/`：PMD 7.28.0（GitHub release
  pmd-dist bin zip，经 gh-proxy 直链；Java 21 系统自带）；cargo-mutants /
  cargo-llvm-cov **本轮未安装**（沙箱重建后缺位，见待办）。新 shell 需
  `export PATH="$HOME/.cargo/bin:$PATH"`。

### 验证证据

| 验证项 | 结果 | 命令/方法（可复现） |
|---|---|---|
| 全量测试 | **4564P / 0F**（15 目标，rc=0） | `cargo test --tests -- --test-threads=2`（project/ezr） |
| clippy 零警告 | 0（-D warnings 门禁） | `cargo clippy --all-targets --all-features -- -D warnings` |
| fmt | clean | `cargo fmt -- --check` |
| 架构护栏 | 16/16 规则通过 | `bash scripts/arch_check.sh`（project/ezr） |
| DRY/CPD | **125 tiles**（基线 137 → −12；v152-clean 登记的 scrollbar 脚手架 5 tile + property 策略 3 tile 同族全部消解，含部分配对） | `.tools/pmd-bin-7.28.0/bin/pmd cpd -l rust --minimum-tokens 50 --dir src`（exit 4 = 有重复属正常语义） |
| 属性测试 | **71P / 0F**（数量守恒） | `cargo test property_tests:: -- --test-threads=2`（独立命令独立计数） |
| 产品区零改动核对 | 11 文件全部 hunk 落在 `#[cfg(test)]` 区之后 | `git diff -U0` 逐文件 hunk 行号 × cfg(test) 边界比对 |
| 变异点 | 3247 守恒（沿 v152 登记值，产品代码零改动故计数不漂移） | v152-clean `cargo mutants --list` 基线沿用 |

### 对账口径（与 cleaner v152-clean 基线）
- 测试计数：4564 → 4564 = **0 增减**（纯测试区重构：夹具单源化不增删测试，
  逐目标 `test result` 复算自洽：bin 614 + ezr-fixture 30 + ezr-proxy 15 +
  hardening 系 12 文件 = 4564）。
- 源码触碰面：产品区零改动；具体文件 `src/app/testutil.rs`（+4 helper）、
  `src/app/{mod,keys,mouse,scrollbar,paste,pieces,engine/mod,dialog_keys/mod}.rs`
  （测试区）、`src/ui/pieces.rs`（测试区）、`src/property_tests.rs`、
  `packs/_common/engineering.md`（经验沉淀）。
- 度量口径：CPD 工具版本与开关沿 v152 口径（PMD 7.28.0，min-tokens 50，src）；
  137→125 的 −12 全部归属登记漂移族（scrollbar 脚手架 + property 策略脚手架
  及其部分配对），无新增 tile（每步改造后复跑核对）。

### 待办与待批（未决项）
- **沿袭项全部不变**（见第一节遗留受限项）；本轮无新增阻塞、无新增待批。
- **cargo-mutants / cargo-llvm-cov 未安装（沙箱重建欠账，移交 hardender）**：
  `.tools/` 现仅 PMD；hardender 开工需按 notes/cargo-mutants.md 与 rust.md
  安装（cargo-mutants 27.1.0 版本口径取发布 tag 探针勿用 -V；cargo-llvm-cov
  0.9.1 经 gh-proxy 直链），变异点计数以 v152 登记 3247 为先验、装后
  `--list` 复核对账。

### 移交建议
- 建议下一步运行 `six-pack/hardender`（角色链顺序）。本轮交付的裁决口径：
  ①**>100 变异点 7 文件全部整文件 shard**，不再机械拆分——scrollbar.rs（190）
  v151 已完成 engine/interaction 边界拆分、余下为纯几何引擎（零 IO、已最大化
  可测，再拆破坏内聚）；mouse.rs（151）/keys.rs（137）为单职责输入分派状态机
  （拆 impl 块只增 pub(super) 暴露面、不降耦合）；textfield.rs（140）v152-clean
  已裁决 field_window 不上提（维护该裁决）；app/mod.rs（108）为组装根 facade
  （v151 已收口 scrollbar 状态机）；ui/text.rs（101）超门禁仅 1 点；
  ezr-proxy.rs（127）独立 bin + CRAP 豁免类。②建议分片顺序（无 IO 纯逻辑先行）：
  scrollbar.rs → textfield.rs → keys.rs → mouse.rs → app/mod.rs → ui/text.rs →
  ezr-proxy.rs。③CRAP>30 门禁残余 1 条（handle_socks5，豁免类）+ textfield.rs
  振荡回归 2 测 + 本轮 main_down_conns 2 测可作加固基线的一部分。
- 变异点批次文件口径即上表 7 文件计数（scrollbar.rs 190 / mouse.rs 151 /
  textfield.rs 140 / keys.rs 137 / ezr-proxy.rs 127 / app/mod.rs 108 /
  ui/text.rs 101，全量 3247）。
