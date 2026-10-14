# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：v128-url-authority-impl（任务 v128-url-authority-impl）
> 本轮（上游交接：specifier v128-spec 的 v1.28 URL 权威模型改版规格）：v1.28 全量实现落地——
> 原始 URL = 任务级唯一权威，跳转落点降格为每请求链内部当前值（不持久化、不共享、不作状态）。
> 七处触碰面 + `name_locked` 替身标记 + 6 个新回归测试；TDD 先红后绿；
> **产品 2925P/0F、clippy 0、fmt 0、arch 14 规则全过**。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.28，分期模式）+ `project/mission/phase-01.md`。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（v115 前历轮） | mission/phase-01.md 全量 FR/D 裁决表 + features/ 13 份 + qa/ 13 份 | 全部交付 | parser/dry-check 全过 |
| coder（v120–v124 两批 + v1.18 修订） | FR-01-102/103/104/105/51/6 系列 + sidecar 即时落盘 + 清码 retry 直判 | 已交付 | 各批 2438→2506P/0F、clippy 0 |
| cleaner（v125/v129/v132 三轮） | 保持行为清理 + 四度量基线首登 | 已交付 | 2506→2535P/0F |
| architect（v126/v134 两轮） | 四阶段架构评审 + throttle 策略提取 + 属性测试 60 + arch 14 规则 | 已交付 | 2523→2546P/0F |
| hardender（v127-hard / v137-hard） | 差分变异加固 + 三度量基线（CPD 52 / 94.36% / 门禁 0） | 已交付 | 基线全绿 |
| coder（v128）+ 会话外登记（v1.19/v1.20） | CPD 处置 + R 恢复路径退役 + 任务 ID 随机化 | 已交付 | 2535→2578P/0F |
| architect（v135/v136 三段 = v1.21–v1.23） | FailedPaused 退役（8 态）+ 恢复一律重新排队 + 规格三件套同步 | 已交付 | 产品 2597P/0F |
| QA（v138-qa / v140-qa / v141-qa） | e2e 同步 + 桩扩展 + TP 收口（S→P ×3） | 已交付 | e2e 164 条 164P/0F/0S；产品 2775P/0F |
| coder（v139-fix / v142-fix / v143-fix / v144-fix） | 分块计数倒退 + 幽灵 supervisor + 单流断点保全 + 失败弃账 四轮缺陷修复 | 已交付 | 产品 2877P/0F、clippy 0、fmt 0、arch 14 过 |
| specifier（v128-spec） | v1.28 URL 权威模型改版规格落账 + v1.24–v1.27 版本注记补记 | 已交付 | parser 3/3 实跑过、dry-check 0 增量错误级 |
| **coder（v128-impl，本轮）** | **v1.28 URL 权威模型全量实现**（七处触碰面 + name_locked 替身 + 6 个新回归测试） | 已交付 | **产品 2925P/0F、clippy 0、fmt 0、arch 14 过** |

### 遗留受限项（当前有效）
- **软 Gherkin 验收变异受限（沿袭 v127/v137/v138/v140/v141 口径）**：本轮为纯产品实现轮，
  feature 文件零触碰（v1.28 规格已由 specifier 轮落账并 parser/dry-check 验证）；
  `gherkin-mutator` 全量变异 campaign 仍不可在本环境前台窗口预算内完成。
  01-persistence-config.feature 的 mutation-stamp（sha256=57e757…）维持保留处置。
- **三度量未在本轮复跑（沿袭 QA 轮口径）**：覆盖率/CPD/CRAP 基线沿袭 v138
  （94.37% / 52 tiles / 门禁 0）；本轮触碰面集中在 supervisor/consistency/sidecar，
  `consistency.rs` 为主要缩面（URL 腿删除），量级变化建议 cleaner/architect 轮差分复测。
- **APS 工具链（本轮实况）**：bb + APS 未安装（本轮未运行 Gherkin 验收——coder 职责边界
  为单元/回归层；验收 runner 实跑归 QA 轮）。rustup nightly 1.101.0-nightly 已按
  rust-toolchain.toml 钉定装入（rsproxy.cn 镜像；`export PATH="$HOME/.cargo/bin:$PATH"`
  补注入；crates 镜像 `~/.cargo/config.toml` rsproxy-sparse，均 gitignore 排除不入档）。
- 无其他受限项。

### 实现定义值登记（当前有效）
- 沿袭有效：D28/D29/D30/D31/D32/D33/D34、v1.21–v1.27 各条口径、v138 R 臂重锚口径、
  v140 桩口径与 TD-17 采样口径、v1.28 三条后果性推论（一致性三要素化 / 路径基匹配 +
  兜底 / 详情与复制收敛原始 URL）。
- **本轮新增（v1.28 实现定义值）**：
  - **`name_locked`（首次探测已定名标记）**：`Task`/`TaskSnapshot` 新增布尔字段——首个
    `Evt::Probed` 置位、**不随失效重下（Invalidated）重置**（行为等价原
    `final_url.is_some()` 的 keep_name 口径）；注册表持久化配 `#[serde(default)]`，
    v1.27 旧档缺省 false（首个重启的探测按 FR-01-02 v1.28 述语重新推导命名，sidecar
    在位任务由引擎 sidecar 短路保持原名——零迁移，行为影响面：仅「旧档 + 无 sidecar +
    CD 改名」组合下的首次重启名可能回落 URL 推导名）。
  - **`derive_name` 保留三参签名**：第二参述语改「探测请求跳转落点 URL（该次连接的
    当前值，不持久化）」——FR-01-02 v1.28 可观察行为不变。
  - **sidecar 版本号维持 1**：URL 键退役按多余字段忽略（沿袭 has_error 退役先例，
    字段集随版本演进不漂移）。
- 无未决待批项。

## 二、当前产出情况（coder v128-impl）

### 本会话产物清单（产品源码 + 测试 20 文件，规格/QA 零触碰）
**模型层**
- `src/model/consistency.rs`：`ServerStamp.final_url` 退役；`check()` 收敛三要素
  （ETag/Last-Modified/大小），`same_url` 腿删除；MissingStamp 条件改
  `stored.size.is_none()`；测试重写（+last_modified 三腿用例）。
- `src/model/sidecar.rs`：`url`/`final_url` 字段退役；`build()` 去 url 参；`stamp()` 无 URL；
  模块文档 v1.28 口径；**新增 `legacy_url_fields_are_ignored_and_not_written_back`**
  （旧档含 url/final_url 键照常加载 + 新序列化不写回）。
- `src/model/task.rs`：`Task.final_url` 退役 → `name_locked` 替身。
- `src/model/registry.rs`：`TaskSnapshot.final_url` 退役 → `name_locked`（serde default）；
  **新增 `legacy_final_url_field_is_ignored_and_not_written`**。
- `src/model/namegen.rs`：文档述语 v1.28（derive_name 签名与行为不变）。
- `src/model/mod.rs`：sample_task 夹具同步。

**引擎层**
- `src/engine/supervisor.rs`：**worker_url 共享退役**——多块 worker（WorkerDeps.url =
  spec.url 原始 URL）与单流续传请求均从原始 URL 独立发起跟跳转（reqwest
  `Policy::limited(10)` 按请求链）；`Evt::Probed` 去 final_url；`probed_stamp` 无 URL；
  **新增 `redirect_server` 夹具 + 2 个重定向回归**（多块 5 次命中 / 单流续传 2 次命中）；
  测试夹具 ServerStamp/Sidecar::build 全量同步新形态。
- `src/engine/mod.rs`：`Evt::Probed` 字段退役。

**应用层**
- `src/app/engine/evt.rs`：Probed 臂 `t.final_url = …` → `t.name_locked = true`。
- `src/app/engine/mod.rs`：`keep_name: t.name_locked`；evt_tests 全量同步 + name_locked 断言。
- `src/app/dialogs.rs` + `src/app/cli.rs`：FR-01-26 断点接续改**路径基**
  （`Sidecar::load(..).is_some()`，删同 URL 校验门）；**新增路径基接续回归**
  （异 URL 同推导名 v1.27 形态 sidecar → 沿用原名）。
- `src/app/mouse.rs`：URL 点击复制收敛原始 URL 单值（D27 v1.28）；**新增
  `detail_url_click_copies_original_url_single_source`** + 既有复制用例同步。
- `src/app/testutil.rs`：preset_resume_sidecar 夹具同步。

**UI 层**
- `src/ui/detail.rs`：详情 URL 行 = `&t.url`（原始 URL 单值，FR-01-80/D27 v1.28）；
  `tests/gen/detail_stripped.rs` 剥头副本同步再生（gen-drift 守卫过）。

**属性测试**
- `src/property_tests.rs`：`prop_consistency_decision_rules`（三要素口径）+
  `prop_sidecar_json_round_trip`（无 URL 往返 + 序列化无 URL 键断言）重写。

**集成测试**
- `tests/hardening_g_appcore.rs` / `tests/hardening_g_engine.rs` /
  `tests/hardening_ui_backfill.rs`：Sidecar/Task 字面量与 final_url 残留同步清理。

### 验证证据
| 验证项 | 结果 | 命令 |
|---|---|---|
| 全量测试（单元+集成+属性） | **2925P / 0F**（14 目标；基线 2877P + 6 新回归） | `cargo test`（ezr crate 根） |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| 格式 | 干净（已应用 fmt） | `cargo fmt --check` |
| 架构边界 | 14/14 规则过 | `bash scripts/arch_check.sh` |
| TDD 红证据 | 4 行为用例实测红后转绿（dialogs 路径基 / sidecar 旧键 / registry final_url / mouse 复制原值） | 红：`cargo test --bin ezr -- legacy_url_fields legacy_final_url_field confirm_add_resumes_by_path detail_url_click_copies_original` |
| gen-drift 守卫 | 剥头副本与产品源逐字一致 | `cargo test --test hardening_ui_backfill gen_fixtures` |

### 对账口径（基线 → 终态）
- 测试计数：2877P → 2925P（+48 = 6 新用例 × 各挂载测试 crate 复记；新用例明细：
  supervisor 重定向 ×2、sidecar 旧档兼容 ×1、registry 旧档兼容 ×1、dialogs 路径基 ×1、
  mouse 复制单值 ×1；consistency 用例一退一进净零）。`git diff --stat`：20 文件
  +447/−153。
- 规格 / QA / mission 文件：零 diff（本轮纯实现轮）。

### 待办与待批（未决项）
- 无阻塞项、无待批项。
- 沿袭移交（非缺陷）：gherkin-mutator 运行器基建（见遗留受限项）。

### 移交建议
- 下一步建议 `bin/swarm begin six-pack/cleaner`（v1.28 落地后按其门槛对触碰面做
  保持行为清理与 CRAP/DRY 差分——`consistency.rs` URL 腿删除后为主要缩面）；
  或按需直启 `six-pack/architect` / `six-pack/hardender` / `six-pack/QA`
  （QA 轮需按 v1.28 移交项改造 e2e runner：QA-RS-07 变体三要素化改造、QA-RS-14
  新 runner 场景、suite_download_engine.py 补「各分块请求 origin 命中」断言）。
