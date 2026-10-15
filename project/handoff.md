# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect　会话：v145-arch（任务 ezr-v145-arch）
> 本轮（上游交接：cleaner v128-clean 的 v1.28 保持行为清理）：v1.28 清理后架构评审
> 与属性测试覆盖——四阶段评审（UI/核心分离、依赖规则、信息隐藏、局部代码质量 +
> 渲染聚合逐点检查）零违规；**supervisor.rs 114 变异点目录化拆分**（上游登记的
> architect 裁决项闭环，六文件全低于 100 阈值、点数总和守恒 = 纯移动）；header
> 会话峰值聚合提取为 App 访问器；属性测试 60→62；arch 规则 14→15。
> **产品 3049P/0F、clippy 0、fmt 0、arch 15 规则全过**。

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
| cleaner（v128-clean） | supervisor 失败分类/完成收尾两处单源化 + 14 个纯追加测试 | 已交付 | 产品 3040P/0F、clippy 0、fmt 0、arch 14 过；CPD 产品区 17→15 |
| **architect（v145-arch，本轮）** | **supervisor 目录化拆分（114 点 → 6 文件）+ header 峰值访问器 + 属性测试 62 + arch 规则 15** | 已交付 | **产品 3049P/0F、clippy 0、fmt 0、arch 15 过；supervisor 六文件 33/28/21/16/11/5 全低于 100** |

### 遗留受限项（当前有效）
- **环境重建（本轮实况）**：全新沙箱——nightly 1.101.0-nightly（2026-10-13）按
  rust-toolchain.toml 钉定重装（rustup minimal + clippy/rustfmt/llvm-tools；
  rsproxy.cn 镜像）；lld 桥接 `~/.local/bin/ld.lld` → 工具链 rust-lld（PATH 需含
  `~/.cargo/bin` 与 `~/.local/bin`）；crates 镜像 `~/.cargo/config.toml`
  rsproxy-sparse。度量工具：cargo-mutants 27.1.0（`~/.cargo/bin`，本轮装妥并跑
  `--list` 扫描）。
- **本轮未装度量工具（影响面登记）**：cargo-llvm-cov 未装（覆盖率未复测——上游
  94.06% 口径沿袭有效；本轮产品行变化小：+2 访问器/−内联聚合，增量可由新测试
  覆盖行推定）；PMD 未装（CPD 未复跑——上游 89 tiles/15 产品区基线沿袭有效，
  本轮 supervisor 拆分不改重复形态：拆分文件内部无跨文件拷贝新增，产品区 tile
  数预期不变或微降，由下一轮 cleaner/hardender 按同口径复核）；bb + APS 未装
  （本轮未运行 Gherkin 验收——architect 职责边界）。01-persistence-config.feature
  的 mutation-stamp（sha256=57e757…）维持保留处置。
- **ezr-proxy.rs 127 变异点（本轮裁决：维持不拆）**：独立代理外壳 bin（测试基建，
  15 个单测），非 ezr 产品触碰面；拆分无 hardender 变异收益（变异 campaign 聚焦
  产品 crate），登记维持上游「非触碰」判定。
- **磁盘预算**：10G 级沙箱（本轮实测余 2.7G）；llvm-cov 插桩全量构建 ≈3.5G——
  覆盖率复测前先清理 `project/ezr/target` 或评估预算。
- 无其他受限项。

### 实现定义值登记（当前有效）
- 沿袭有效：D28/D29/D30/D31/D32/D33/D34、v1.21–v1.27 各条口径、v138 R 臂重锚口径、
  v140 桩口径与 TD-17 采样口径、v1.28 三条后果性推论（一致性三要素化 / 路径基匹配 +
  兜底 / 详情与复制收敛原始 URL）、v1.28 实现定义值（name_locked 替身语义 /
  derive_name 三参签名 / sidecar 版本号维持 1）。
- **本轮新增：无**（纯重构 + 测试追加，产品契约零变更；supervisor 拆分行为等价由
  全量测试 + 变异点守恒双重锁定；header 峰值访问器为同口径搬运——展示值逐字节
  等价由既有 UI 测试锁定）。
- 无未决待批项。

## 二、当前产出情况（architect v145-arch）

### 本会话产物清单（产品源重组 6 新 1 删 + 测试 3 文件 + 脚本 1 + 工具侧沉淀 1）
- `src/engine/supervisor/`（**产品区唯一重组**：supervisor.rs 2872 行 → 目录模块，
  纯移动零行为变化；测试模块原地保留于 mod.rs）：
  - `mod.rs`：域类型（TaskSpec/VerifySpec/Flow/PausedState）、入口 run/verify、
    emit/sidecar_path/completed_of 小 helper + 测试模块（原 27 测试零改动）；
  - `http.rs`：req_with_identity / req_checked（请求臂失败分类收口）/
    ProbeHead/probe_head（探测头解析）；
  - `finalize.rs`：finalize_completed（完成收尾单源）/ build_sidecar / flush_sidecar /
    failed_flow / failed_ledger / block_span（sidecar 记账面）；
  - `workers.rs`：WorkerDeps / block_worker / stop_workers / prune_finished /
    fill_workers（worker 池面）；
  - `download.rs`：download() 多块监督主流程 + PROGRESS_TICK/SIDECAR_FLUSH 常量；
  - `single.rs`：single_stream() 单流降级路径。
- `src/ui/header.rs` + `src/app/mod.rs`：会话峰值聚合提取（architect v116/v126 同法
  延伸）——`App::peak_dl_speed()/peak_ul_speed()`（速度历史 max 采样 KB/s→B/s 还原
  收口应用层）+ 无头单测 `peak_speeds_max_of_history_in_bytes`；header 渲染改消费
  访问器，展示值等价。
- `src/property_tests.rs`：属性测试 60→62（文末独立 proptest! 块 + 头注登记）——
  `prop_derive_name_fallback_chain`（derive_name 三级优先 + 时间戳兜底契约：非空 /
  输出已卫生化 / 优先级链逐级回退）、`prop_alloc_task_id_fresh_nonzero_unique`
  （v1.20 随机 ID 分配契约：非 0 / 不与占用集命中 / 连续分配互不相同）。
- `scripts/arch_check.sh`：新增规则 15——supervisor 实现子模块保持私有（pub(super)
  暴露纪律；`pub mod` 出现即窄接口破坏）；阳性对照实测命中（临时 `pub mod finalize;`
  → FAIL → 移除复归全过）。
- `packs/_common/notes/rust.md`（工具侧经验沉淀）：「拆大文件」条增补两件低成本佐证
  （mutants --list 点数守恒 = 纯移动强信号；测试补导入清单以编译器 unused 警告为准，
  grep 预判被注释/全路径引用高估）；「单文件升级为目录模块」三机械点扩为四
  （④ 路径深度不对称：mod.rs 保持 `super::X`、孙层一律 `super::super::X`，按编译
  错误清单收敛，勿批量直觉替换）。

### 验证证据
| 验证项 | 结果 | 命令 |
|---|---|---|
| 全量测试（单元+集成+属性） | **3049P / 0F**（+9 = 峰值访问器测试 ×7 编译语境 + 属性测试 ×2 bin 语境） | `cargo test`（ezr crate 根） |
| 属性测试（独立口径） | 62P / 0F（60 沿袭 + 2 新增） | `cargo test --bin ezr -- property_tests::` |
| clippy | 0 警告（nightly 1.101.0-nightly，--all-targets 产品+测试双面） | `cargo clippy --all-targets` |
| 格式 | 干净 | `cargo fmt --check` |
| 架构边界 | 15/15 规则过（+规则 15，阳性对照实测） | `bash scripts/arch_check.sh` |
| 变异点扫描 | 全仓 2312 点；**supervisor 拆分后六文件 33/28/21/16/11/5（总和 114 = 拆分前单文件数，守恒）**；ezr-proxy 127（维持不拆） | `cargo mutants --list --line-col=true` |
| 新测试定向 | 3/3 绿（bin 语境单跑） | `cargo test --bin ezr -- peak_speeds prop_derive prop_alloc` |

### 对账口径（基线 → 终态）
- 测试计数：3040P → **3049P（+9）**，逐 target 实测复算自洽：本轮新增测试 3 个——
  `peak_speeds_max_of_history_in_bytes`（app/mod.rs 测试区）随 app 树挂载语境 ×7 =
  产品 bin + 6 个挂载 app 的 hardening crate（实测 +1：engine_backfill 353→354、
  g_appcore 330→331、g_engine 325→326、g_supervisor 299→300、ui_backfill 382→383、
  v115 377→378）；2 个属性测试（property_tests.rs 仅产品 bin 语境，bin 单测 435→438）。
  合计 7 + 2 = 9。不挂载 app 的语境计数不变（g_throttle_err 206、g_main 2、
  g_proxy 16、g_slots 144、hardening.rs 126、fixture 30、proxy 15）。
- `git diff --cached --stat`：supervisor.rs 删除（2872 行）→ supervisor/ 六文件
  （mod.rs 保留测试区，五子文件 1132 行产品区）+ app/mod.rs +37（2 访问器 + 1 测试）
  + header.rs 13 行改写 + property_tests.rs +71 + arch_check.sh +15。
- 变异点：全仓 2302（上游 scan）→ 2312（+10 = 峰值访问器产品行产点，本轮唯一
  产品逻辑面增量）；supervisor 114 守恒拆分（33/28/21/16/11/5），hardender 可
  `cargo mutants --file` 按文件分块跑差分变异。
- 规格 / QA / mission 文件：零 diff（本轮纯架构轮）。

### 待办与待批（未决项）
- 无阻塞项、无待批项。
- 沿袭移交（非缺陷）：覆盖率/CRAP 复测（下游轮按上游口径补跑，本轮未装
  cargo-llvm-cov）、CPD 复跑（PMD 未装，下游 cleaner/hardender 口径见上游移交）、
  gherkin-mutator 运行器基建。

### 移交建议
- 下一步建议 `bin/swarm begin six-pack/hardender`（supervisor 模块边界已收口，
  六文件全部低于 100 阈值——`--file` 分块差分变异的前提已就绪；三度量基线沿袭
  上游口径 + 本轮 mutants scan 已实测）；或按需直启 `six-pack/QA`（QA 轮需按
  v1.28 移交项改造 e2e runner：QA-RS-07 变体三要素化改造、QA-RS-14 新 runner
  场景、suite_download_engine.py 补「各分块请求 origin 命中」断言）。
