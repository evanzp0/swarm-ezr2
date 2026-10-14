# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner　会话：v128-clean（任务 ezr-v128-clean）
> 本轮（上游交接：coder v128-impl 的 v1.28 URL 权威模型全量实现）：v1.28 落地后保持行为清理
> + 四度量差分复测——supervisor 两处产品区重复单源化（`req_checked` / `finalize_completed`）、
> 14 个纯追加测试补齐触碰面覆盖缺口；
> **产品 3040P/0F、clippy 0、fmt 0、arch 14 规则全过；CPD 产品区 tile 17→15**。

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
| coder（v128-impl） | v1.28 URL 权威模型全量实现（七处触碰面 + name_locked 替身 + 6 个新回归测试） | 已交付 | 产品 2925P/0F、clippy 0、fmt 0、arch 14 过 |
| **cleaner（v128-clean，本轮）** | **保持行为清理：supervisor 失败分类/完成收尾两处单源化 + 14 个纯追加测试** | 已交付 | **产品 3040P/0F、clippy 0、fmt 0、arch 14 过；CPD 产品区 17→15** |

### 遗留受限项（当前有效）
- **环境重建（本轮实况）**：本轮为全新沙箱（上轮工具链不入档，本体丢失）——nightly
  1.101.0-nightly（2026-10-13）按 rust-toolchain.toml 钉定重装（rustup minimal +
  clippy/rustfmt/llvm-tools；rsproxy.cn 镜像）；lld 桥接 `~/.local/bin/ld.lld` → 工具链
  rust-lld（PATH 需含 `~/.cargo/bin` 与 `~/.local/bin`）；crates 镜像 `~/.cargo/config.toml`
  rsproxy-sparse。度量工具：cargo-llvm-cov 0.9.1、cargo-mutants 27.1.0（`~/.cargo/bin`）、
  PMD 7.28.0（`swarm/.tools/pmd-bin-7.28.0`，ghfast.top 镜像下载；GitHub 直链 403）。
- **三度量口径与基线对账（本轮实测）**：
  - 覆盖率：llvm-cov `--branch --lcov` 一次性产出；src 全域（含 `src/bin/*` 外壳）
    **94.06%**（13390/14236 行），剔除 bin 外壳口径 ≈94.78%。与上游 94.36/94.37%
    不可直接对比（口径/树态/nightly 版本差异）；本轮编辑后未复测（磁盘预算受限，
    见下），增量可由 14 个新测试的覆盖行推定。触碰面实测：consistency/task/
    testenv/testutil 100%、sidecar 99.5%、registry 96.2%、namegen 95.8%、evt 95.1%、
    dialogs 94.3%、mouse 95.9%、detail 91.5%、cli 90.9%、supervisor 88.3%（深水区为
    错误/重试/回滚路径）。
  - CRAP：受限近似（lcov BRDA 口径 comp=BRDA 记录数+1；排除 closure/测试模块/
    tests 目录；NO_DA=0）原始门禁 **82 条**。触碰面逐函数源码核验：多条件短路链
    （consistency::check 源码 comp≈4 等）属 rust.md 已知假阳性；supervisor
    spawn（BRDA comp=199）/download/single_stream、dialogs/mouse/detail 分发核心
    为 select!/match 臂固有分支数——**登记口径移交**，不据此硬拆（SKILL 杠杆顺序
    末条）；「门禁 0」沿袭解读 = 登记清单外零漂移。
  - DRY：PMD 7.28 `-l rust --minimum-tokens 50 --dir src` 全量 **89 tiles**
    （17→**15 产品区** + 74 测试区）。上游基线 52 不可复现（内嵌测试脚手架随
    v1.24–v1.28 各轮增长；阈值扫描 70→31/100→6 亦无匹配）——**产品区 tile 数
    为跨轮对账稳定口径**（17→15 本轮净 −2）。残余产品区 tile 全部为登记形态：
    dialogs↔testutil（夹具↔产品构造器同构）、property_tests 自重复×2（整文件
    测试代码）、其余均非本轮触碰面（clipboard/proxy/fixture/main/tick/task_lines）。
- **变异点 >100 文件（移交 architect）**：cargo-mutants 27.1 `--list --line-col=true`
  scan 口径全仓 2302 点；**supervisor.rs 114（触碰面）**、ezr-proxy.rs 127（非触碰，
  代理外壳）。按 rust.md：模块边界拆分依赖 architect 裁决、验证依赖 e2e（cleaner
  不运行）——登记移交，hardender 全量变异前由 architect 先收模块边界。
- **APS 工具链**：bb + APS 未安装（本轮未运行 Gherkin 验收——cleaner 职责边界）；
  gherkin-mutator 全量 campaign 沿袭受限。01-persistence-config.feature 的
  mutation-stamp（sha256=57e757…）维持保留处置。
- **磁盘预算（本轮实况）**：10G 级沙箱，llvm-cov 插桩全量构建 ≈3.5G；覆盖率复测
  受磁盘预算限制未跑（度量解析产物 lcov.out 已落盘 `.work/tmp/` 可复用）。
- 无其他受限项。

### 实现定义值登记（当前有效）
- 沿袭有效：D28/D29/D30/D31/D32/D33/D34、v1.21–v1.27 各条口径、v138 R 臂重锚口径、
  v140 桩口径与 TD-17 采样口径、v1.28 三条后果性推论（一致性三要素化 / 路径基匹配 +
  兜底 / 详情与复制收敛原始 URL）、v1.28 实现定义值（name_locked 替身语义 /
  derive_name 三参签名 / sidecar 版本号维持 1）。
- **本轮新增：无**（纯清理 + 测试追加，产品契约零变更；两处单源化行为等价由全量
  测试锁定）。
- 无未决待批项。

## 二、当前产出情况（cleaner v128-clean）

### 本会话产物清单（产品源码 1 文件修改 + 测试 6 文件纯追加；规格/QA/mission 零触碰）
- `src/engine/supervisor.rs`（**产品区唯一修改文件**，行为保持）：
  - 提取 `req_checked(client, url, range)`：请求臂失败分类收口（2xx → Response；
    非 2xx → 读尽响应体后 `EngineFailure::http`；传输错误 → `classify_reqwest`），
    多块 worker 与单流续传两调用点委托（消解 CPD 82tok 产品区 tile；v1.27 失败
    回报口径由调用方各自接续，行为等价）；
  - 提取 `finalize_completed(spec, stamp, blocks, …)`：完成收尾单源（无校验 → 改名
    清台账直判完成；有校验 → 落全块完成终态台账）+ DownloadDone，多块聚合完成臂
    与单流全块台账直判臂委托（消解 CPD 53tok 产品区 tile）。
- 测试纯追加（只追加于既有测试模块末尾，不改产品行为、不改构建配置）14 个：
  - `src/model/namegen.rs` ×2：落点 URL 无路径回退原始 URL 推导、dedupe 时间戳兜底；
  - `src/model/registry.rs` ×6：ProxyChoiceCompat::default、旧 "global" 映射直连、
    未知形态回退 direct、Registry::default、load 版本不符 → None、restore 重置
    未定校验结果；
  - `src/model/sidecar.rs` ×1：remove 缺失 sidecar 幂等 Ok；
  - `src/app/cli.rs` ×2：非法 URL toast 拒绝、路径基 sidecar 接续沿用原名；
  - `src/app/engine/mod.rs`（evt_tests）×2：非下载中 Progress 早退（不产生幽灵速度
    窗）、重试达上限停等（释放槽位 + 引导 R）；
  - `src/ui/mod.rs`（ui_v114_tests）×1：失败原因行仅「已失败」态展示（挂起态不展示）。

### 验证证据
| 验证项 | 结果 | 命令 |
|---|---|---|
| 全量测试（单元+集成+属性） | **3040P / 0F**（14 目标；基线 2925P + 115 增量） | `cargo test`（ezr crate 根） |
| clippy | 0 警告（nightly 1.101.0-nightly） | `cargo clippy --all-targets` |
| 格式 | 干净（已应用 fmt） | `cargo fmt --check` |
| 架构边界 | 14/14 规则过 | `bash scripts/arch_check.sh` |
| 新测试定向 | 14/14 绿（bin 语境单跑） | `cargo test --bin ezr -- <新测试名…>` |
| CPD 复跑 | 89 tiles（15 产品区 + 74 测试区），触碰面产品区 tile −2 | `pmd cpd -l rust --minimum-tokens 50 --dir src` |
| 变异点扫描 | supervisor 114 / ezr-proxy 127 / 全仓 2302（scan，未跑变异） | `cargo mutants --list --line-col=true` |
| 覆盖率/CRAP | 94.06% src 全域 / 门禁原始 82 条（口径见遗留受限项） | `cargo llvm-cov --branch --lcov` + `.work/tmp/crap_analyze.py` |

### 对账口径（基线 → 终态）
- 测试计数：2925P → **3040P（+115）**，逐 target 实测闭环：14 个新测试 × 各自编译
  语境数（namegen ×2 → 10 语境〔bin + 8 个挂载 model 树 crate + hardening 经
  namegen_hard 壳层再挂载〕= 20；registry ×6 → 9 语境 = 54；sidecar ×1 → 9 = 9；
  cli ×2 → 7 = 14；evt ×2 → 7 = 14；detail ×1 → 4 = 4；合计 115）。
- `git diff --stat`：产品 1 文件 + 测试 6 文件（supervisor 净 −34 产品行：两臂去重
  大于 helper 本体；新增测试 +230 行）。
- CPD：产品区 17 → 15 tiles（−2 = supervisor 两处单源化）；测试区 74 不变。
- 规格 / QA / mission 文件：零 diff（本轮纯清理轮）。

### 待办与待批（未决项）
- 无阻塞项、无待批项。
- 沿袭移交（非缺陷）：gherkin-mutator 运行器基建、覆盖率/CRAP 复测（architect/QA
  轮按本会话口径跑）、supervisor/ezr-proxy 变异点拆分裁决（architect）。

### 移交建议
- 下一步建议 `bin/swarm begin six-pack/architect`（v1.28 清理后按惯例收架构边界与
  属性测试覆盖；并裁决 supervisor 114 变异点的模块边界拆分）；或按需直启
  `six-pack/hardender`（三度量已差分复测，可直接差分变异）/ `six-pack/QA`
  （QA 轮需按 v1.28 移交项改造 e2e runner：QA-RS-07 变体三要素化改造、QA-RS-14
  新 runner 场景、suite_download_engine.py 补「各分块请求 origin 命中」断言）。
