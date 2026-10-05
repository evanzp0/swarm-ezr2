# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：coder-20261005-r17（D17 操作者裁决落账，零代码改动）

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（ezr download）：TUI 高性能多协议下载器，phase-01（HTTP/HTTPS 真实下载
  内核与 TUI 正式版）已交付并通过 QA 终局独立验证；QA 后三轮增量：coder-20261005 裁决
  落地轮（CRAP 裁决登记 / on_evt arm 级单测 / 单实例锁回退分支收口）、specifier-20261005
  （v1.4 配置模板需求建档 FR-01-85/D17）、coder-20261005-v14（FR-01-85 实现，TDD）、
  coder-20261005-r17（本轮：D17 操作者裁决落账——维持 A「存在不覆写 + 失败静默」，
  零代码改动，待办清零）。
  需求权威来源 `project/mission.md`（分期模式，v1.4），phase-01 详述见
  `project/mission/phase-01.md`（v1.4），feature 规格在 `project/features/`，QA 规程与
  可执行套件在 `project/qa/`。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier | mission.md + mission/phase-01.md + features/ 9 份规格 + qa/ 9 套件 | 已交付 | 上游既定口径 |
| coder（phase-01） | ezr 下载内核 + TUI + ezr-fixture/ezr-proxy + 属性测试 | 已交付 | 上游既定口径（258 测基线） |
| cleaner / architect（历轮） | 工具链钉 nightly、模块边界收敛、两轮架构复核 | 已交付 | arch_check 8/8、clippy 0、fmt 0（其会话口径） |
| hardender（20261004） | 变异加固：7 个 hardening_g_* 套件 ~110 条新测试 | 已交付 | 加固后全绿；equivalence 台账见其归档 |
| coder（20261004） | 空闲期注册表零写放大修复 + 3 条聚焦单测 | 已交付 | 955P/0F；clippy 0；arch_check 8/8 |
| cleaner（20261005） | tests/hardening* 环境漂移收敛 + 度量流水线全量复跑 | 已交付 | fmt 0；clippy 0；955P/0F 零漂移；arch_check 8/8 |
| architect（20261005） | 架构评审四阶段 + lint 姿态单源化 + engine 窄接口收口 + arch_check 10 规则 | 已交付 | 961P/0F；clippy 0；fmt 0；arch_check 10/10；CPD 0 |
| QA（20261005） | QA 终局独立验证：9 套 PTY e2e + 3 处一致性修复 + 3 处产品 bug 修复 + README 收尾 | 已交付（终局） | 966P/0F；e2e 111P/3S（S 预登记）；行覆盖 90.5% |
| coder（20261005 裁决落地轮） | 三项操作者裁决落地：①CRAP 维持现状登记 ②on_evt arm 级单测 24 条 ③锁回退分支同走 try_lock_path 收口 | 已交付 | 1063P/0F；clippy 0；fmt 0；arch_check 10/10 |
| specifier（20261005 v1.4） | 配置模板需求建档：mission/phase-01 v1.4（FR-01-85+D17）+ features/01-config-template.feature（5 场景）+ qa/01-config-template-qa.md（5 用例）+ persistence-config 同步修订 | 已交付 | APS parser 通过；dry-check 4 findings 有意保留 |
| coder（20261005-v14） | FR-01-85 实现（TDD）：`Config::default_template()`（关联函数；内容从 `Config::default()` 单一事实来源拼装，每键用途+取值范围注释，10 键默认值行）+ `ensure_default_config(path)`（create_new 语义不覆写含竞态；父目录自动创建；错误上传播）+ main.rs 接线（锁后/加载前，`let _ =` 静默）+ 7 条契约单测 + README 配置节说明 | 已交付 | 1105P/0F（10 二进制）；clippy 0；fmt 0；arch_check 10/10（详见第二节） |
| **coder（20261005-r17）** | D17 裁决落账：操作者裁决维持 A（①存在即不覆写 ②生成失败静默不阻塞）；phase-01 D17 行改判「操作者 20261005 裁决维持」，handoff 待办节该项清零；零代码改动 | **已交付** | 上轮基线 1105P/0F 不变（FR-01-85 契约测试复跑通过） |

### 遗留受限项（当前有效）
- **CRAP 为受限近似口径**（继承上游；操作者已裁决高 comp 点维持现状）：无标准 CRAP 工具，
  以 llvm-cov lcov 分支计数代入估算；多轮读数不可直接互减。本轮未复算（coder 职责边界
  不运行度量工具；本轮产品代码为新增纯函数 + 单点接线，无既有分支改写）。
- **e2e 套件预登记 S 项（3 用例）**：retry-backoff RB-09（64MB 受限目录）；throttle-proxy
  TP-02/TP-03/TP-06（测量方法学受限，限速语义由 TP-01 权威覆盖）。
- **沙箱无 TTY**：TUI 进程级端到端不可自动化；pty E2E 由 QA 以 pyte 伪终端口径完成。
- **gherkin-mutator 不存在于 npm registry**（继承上游）：Gherkin 差分变异受限；本轮
  feature 无步骤处理器绑定（与既有 9 份 feature 口径一致）。
- **QA-CT-01..05 为规程就绪、套件脚本未落**：新需求的 QA 套件规格已由 specifier 产出，
  可执行 python 套件（qa/runners/suite_config_template.py）按 six-pack 编排属 QA 角色会话
  产物，本轮（coder）不越权补写；如需自动化复跑请启动 QA 会话。
- **本沙箱工具链为重装**（20261005）：nightly 1.101.0（2026-10-04）+ rustfmt/clippy；
  bb 1.13.225（/usr/local/bin）+ APS 工具仓 `.tools/Acceptance-Pipeline-Specification`；
  PATH 需 `export PATH="$HOME/.cargo/bin:$PATH"`。

### 实现定义值登记
- 无新增契约值。模板示例行形态（`# download_dir = ""`、`# max_speed = 0`、
  `# block_size_http = 1048576`、`# backoff_initial = 8.0` 等）按 feature Examples 表
  契约锚点实现，为规格既定口径而非新实现定义值；默认值取自 `Config::default()`
  单一事实来源，不存在平行字面量。

## 二、当前产出情况（coder-20261005-v14）

### 本会话产物清单
- **`src/model/config.rs`**：① `Config::default_template()`（关联函数）——全注释默认值
  模板：文件头说明 + 每键三行注释段（用途 / 取值范围 / `# 键 = 默认值` 示例行），覆盖
  FR-01-71 全部 10 键；数值全部经 `format!` 取自 `Config::default()`（block_size_http=
  1048576、download_slots=5、max_speed=0、max_retries=5、auto_retry=true、
  backoff_initial=8.0、backoff_cap=60.0、default_concurrency=4；download_dir/proxy 空串
  形态），无第二份字面量。② `ensure_default_config(path)`——`create_new` 语义：已存在
  （含损坏文件与并发竞态）一律 `Ok(())` 不覆写（D17①）；缺失时创建父目录（空父目录名
  过滤）并写入模板；创建/写失败错误向上传播（D17②，调用点静默）。③ 新增 7 条契约单测
  （全注释 / 10 键默认值行逐条 / 每键用途+取值范围段落结构（10 段计数）/ 模板解析=
  `Config::default()` 往返契约 / 缺失创建含父目录+幂等 / 已存在字节不变 / 只读目录
  报错且不留半截文件（`#[cfg(unix)]`））。
- **`src/main.rs`**：`ezr_main` 在单实例锁之后、配置加载之前调用
  `ensure_default_config`，`let _ =` 静默跳过失败（D17②注释依据在位）。
- **`README.md`**：配置节补「首次启动自动生成全注释模板、已存在不覆盖」说明；测试计数
  966 → 1105。
- **工具侧沉淀（宪法第三章）**：`packs/_common/engineering.md` +1 条（「生成默认文件」类
  功能三条纪律：内容单一事实来源拼装 / 往返契约测试 / create-new 不覆写语义）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量测试 | 1105P / 0F（10 个测试二进制） | `cargo test`（project/ezr 下） |
| 新增模板契约测试 | 7P / 0F（产品 bin 内） | `cargo test --bin ezr config::` |
| TDD 红灯实证 | 实现前 E0599/E0425 编译失败（7 处） | 实现顺序记录于 `.work/plan.md` 第 2 步 |
| fmt 全树 | 0 偏差 | `cargo fmt --check` |
| clippy 全目标 | 0 警告 | `cargo clippy --all-targets` |
| 架构边界 | 10/10 通过 | `bash scripts/arch_check.sh` |
| 模板行为抽查（进程级） | 干净 `EZR_HOME` 启动 → `<home>/config.toml` 生成且全注释（10 键 + 用途 + 取值范围，非注释行过滤为零有效行）；预置 `download_slots = 2` 再启动 → 内容不覆写 | `EZR_HOME=<tmp> ./target/debug/ezr </dev/null`（TUI 无 TTY 退出属预期，生成点在此之前）；注：`--help`/`--version` 短路径不触发生成（帮助不落盘） |

### 对账口径
- 测试计数：1063 → 1105（+42 = 7 条新单测 × 6 个编译上下文：产品 bin 1 + 挂载
  model/config.rs 的加固 crate 5（hardening / hardening_g_appcore / hardening_g_engine /
  hardening_g_supervisor / hardening_g_proxy），`#[path]` 挂载机制固有倍增，见
  rust.md 20261005 沉淀条）。分项复算：main 252→259、hardening 79→86、appcore
  192→199、engine 191→198、supervisor 169→176、proxy 125→132，其余二进制零变化；
  既有测试零删除、零修改。
- 产品代码变化面：`src/model/config.rs`（新增两个 pub 函数 + 7 测试）、`src/main.rs`
  （+4 行接线 + use 扩一项）、`README.md`（文档）。无既有行为改动：模板写入点在配置
  加载之前、锁之后，全部注释 ⇒ `Config::load` 解析结果与此前缺失口径逐字节一致
  （`template_parses_to_defaults` 契约为证）。
- FR-01-85 验收对照：feature 5 场景中 01（生成+全注释+10 键契约+默认行为）/02（不覆写）/
  03（损坏不重写不阻塞）/04（失败静默）已由单测锁定可观察语义；05（EZR_HOME 协同）由
  `config_path()` 既有语义承载（ezr_home_env 重定位测试既有在位）+ 进程级抽查证据。
  PTY e2e 层复验（QA-CT-01..05 套件脚本化）移交 QA 角色会话。

### 待办与待批（未决项）
- 无阻塞项。**D17 两条边界已由操作者裁决维持**（A：①存在即不覆写 ②生成失败静默不阻塞，
  20261005，coder-20261005-r17 落账）——实现与测试无需变更；模板示例行形态按 feature
  契约锚点口径维持（键集/默认值/用途与取值范围三点不可缺，空格顺序自由）。
- **QA-CT-01..05 套件脚本化**：建议后续 QA 会话把规程落成 `qa/runners/suite_config_template.py`
  并入 9 套 e2e 回归（现 10 份规程、9 份脚本）。

### 移交建议
- D17 裁决已落账（本轮零代码改动，FR-01-85 实现基线 1105P/0F、clippy 0、fmt 0、
  arch_check 10/10 不变），建议操作者运行 `bin/swarm complete` 归档本轮产物。
- 如需继续：① QA 会话补 e2e 套件脚本（移交建议 ①）；② cleaner/architect 例行轮收口
  新增代码的度量口径；③ phase-02（BT/磁力链）开工时 `bin/swarm begin six-pack/specifier`
  补写 phase-02 需求详述。
