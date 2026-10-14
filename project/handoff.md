# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA　会话：v141-qa（任务 v141-qa-tp-methodology）
> 本轮（操作者直启 TP 收口轮）：TP-02/03/06 按 TP-01 同款 100MB 大文件总量口径
> 实跑化（S→P），**e2e 13 套件 164 条首次全绿 164P/0F/0S（零 S）**；产品树零改动。
> 附带修复打包卫生问题：环境重置曾把工作区文件模式整体刷成 755 并经 `git add -A`
> 进入 v140 归档（119 个源文件 644→755 污染），本轮已按 HEAD 模式表归一化
> （索引相对 HEAD 零模式变更），归一化手法固化进 packs/_common/engineering.md。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.23，分期模式）+ `project/mission/phase-01.md`。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（v115 前历轮） | mission/phase-01.md 全量 FR/D 裁决表 + features/ 13 份 + qa/ 13 份 | 全部交付 | parser/dry-check 全过 |
| coder（v120–v124 两批 + v1.18 修订） | FR-01-102/103/104/105/51/6 系列 + sidecar 即时落盘 + 清码 retry 直判 | 已交付 | 各批 2438→2506P/0F、clippy 0 |
| cleaner（v125/v129/v132 三轮） | 保持行为清理 + 四度量基线首登（覆盖率 94.28% / CPD 51 tiles / scan 2161 点 / CRAP 门禁 76 条） | 已交付 | 2506→2535P/0F |
| architect（v126/v134 两轮） | 四阶段架构评审 + throttle 策略提取 + 属性测试 57→60 + arch 13→14 规则 | 已交付 | 2523→2546P/0F、属性 60P |
| hardender（v127-hard） | v115 前差分变异 78/78 killed + CRAP/DRY 对账 + 软 Gherkin 受限登记 | 已交付 | 产品树零改动、基线全绿 |
| coder（v128）+ 会话外登记（v1.19/v1.20） | CPD 处置 bordered_block 单源 + R 恢复路径退役 + FailedPaused 徽标 + 任务 ID 随机化（u32 idgen 单源） | 已交付 | 2535→2578P/0F |
| architect（v135/v136 三段 = v1.21–v1.23） | 合并轮：FailedPaused 变体退役（8 态）+ 恢复一律重新排队 + 挂起态展示统一 + v1.23 数据统一 + 规格三件套同步 + ezr-demo 同步 + pty 回归 7/7 | 已交付 | 产品 2597P/0F、clippy 0、fmt 0、arch 14 过 |
| hardender（v137-hard） | v1.21–v1.23 触碰面变异加固 + 9 条加固测试 + 三度量新基线（CPD 52 / 94.36% / 门禁 0） | 已交付 | 2733P/0F、clippy 0、fmt 0、arch 14 过 |
| QA（v138-qa） | e2e 六套件同步（新建 suite_ui_conns.py）+ R 臂累计清零缺陷修复（requeue_failed 显式清账本 + conn_rebase 重锚）+ 三度量复测对账 + APS 13/13 + stamp 保留处置 | 已交付 | 2740P/0F、e2e 登记 155P/0F/9S（v140 实证该登记含 TD-17 一条隐性 F 残差） |
| coder（v139-fix） | 分块计数倒退缺陷修复：`apply_chunk_snapshot` 两段合并（多块路径事件逐块真值权威）+ 3 回归单测 + 属性面重写 | 已交付 | 2767P/0F、clippy/fmt/arch 全绿 |
| QA（v140-qa） | 桩扩展轮：ezr-proxy tokio 异步重写 + 三形态监听（https TLS / socks5 RFC 1929 / http 认证 407）+ IPv6 绑定 + 4 新增单测；NP-04/08/13/15/18 S→实跑；RB-09 删除；TD-17 断言机制修复 | 已交付 | 2775P/0F、e2e 164 条 161P/0F/3S（归档含模式位污染，见下） |
| **QA（v141-qa，本轮）** | **TP 收口轮**：TP-02/03/06 按 TP-01 同款 100MB 大文件总量口径实跑化（`_detail_dl` 字节级读数助手 + QA_TP_ONLY 分批过滤器）；规程 v1.7 修订；打包模式位归一化（119 文件 644 恢复 + engineering.md 固化手法） | 已交付 | **e2e 13 套件 164 条 = 164P / 0F / 0S（零 S 首次全绿）**；产品树零改动，产品测试沿袭 v140 基线 2775P/0F |

### 遗留受限项（当前有效）
- **e2e 逐用例环境受限项（S）：无**。v140 的 3 条（TP-02/03/06）已于本轮全部
  实跑化转 P，e2e 首次 0F/0S 全绿；RB-09 已于 v140 按操作者裁决删除（编号留空）。
- **软 Gherkin 验收变异受限（沿袭 v127/v137/v138/v140 口径）**：13 份 feature 的
  parser + ir-dry-check 实跑全过；`gherkin-mutator` 运行器适配器与验收入口生成
  器为会话级工具不随工作区流转，全量变异 campaign 不可在本环境前台窗口预算内
  完成。01-persistence-config.feature 的 mutation-stamp（sha256=57e757…）维持
  保留处置（内容零变更；复跑指令见 v138 交接四要素③口径）。
- **三度量未在本轮复跑（QA 职责沿袭口径）**：本轮产品源码零变更（二进制为
  同源重建）；PMD/cargo-llvm-cov/Babashka+APS 未复装（环境重置后）。覆盖率/
  CPD/CRAP 基线沿袭 v138（94.37% / 52 tiles / 门禁 0）。
- **工具链位置（本轮重建，跨会话缓存 `.tools/` 未随 clone 流转）**：rustup
  nightly 1.101.0-nightly（e9bbadf68 2026-10-12；rust-toolchain.toml 钉定自动装，
  新 shell 须 `export PATH="$HOME/.cargo/bin:$PATH"`；本轮 `cargo build` debug
  三件套 1m35s）。PMD / cargo-llvm-cov / Babashka+APS 未复装。
- 无其他受限项。

### 实现定义值登记（当前有效）
- 沿袭有效：D28/D29/D30/D31/D32/D33/D34、v1.21–v1.23 恢复 toast 文案、v138 R 臂
  重锚口径、v1.24 分块断点视图两段合并（v139 交接）、v140 桩口径与 TD-17 采样
  口径（均见 v140 交接，零变更）。
- **本轮新增（v141 套件量法口径，QA 基建非产品语义）**：
  - **TP 三条实跑量法（TP-01 权威口径的对齐扩展）**：100MB 大文件使启动瞬态
    （桶突发 0.5s + socket 预读）摊薄至 <3%，跳过前 10MB 瞬态窗后按
    「（总量−窗量）/（完成时刻−窗时刻）」两端精确计，容差 ±10%；断言读数用
    详情「大小」（字节级实时口径，FR-01-105），显示速度按块粒度跳变不作依据。
  - TP-02：2 任务并发下载 big-100m，`?cd=tp02{a,b}.bin` 区分命名（FR-01-02）+
    独立子目录（规避同 URL 重复守卫 QA-01-16）；组合稳态窗 = 每任务 10MB；
    导航用 `home`/`down` 键 + 选中行 ▌ 标记逐轮读双任务详情。
  - TP-03：配置 1 MB/s + CLI `--max-speed 512 KB/s`（FR-01-60 参数优先）；
    判别力 = 512K+10% 上界 563K 与 1M−10% 下界 900K 不相交。
  - TP-06：`big-100m.bin?norange=1`（fixture 忽略 Range 回 200 全量 → 引擎单流
    路径）；证据面 = 详情「不支持断点续传」+ 除 bytes=0- 探测外无分块 Range
    请求（FR-01-10，DE-02 同款判据）。
  - **QA_TP_ONLY=1,3 环境变量**：套件内用例过滤器（沙箱前台单命令 10min 上限，
    本套件全量 >13min 时分批前台跑；缺省全量行为不变）。
- 无未决待批项。

## 二、当前产出情况（QA v141-qa）

### 本会话产物清单
**QA 套件（qa/runners/suite_throttle_proxy.py）**
- `_detail_dl` 上提为模块级读数助手（详情「大小 X / Y」→ 字节，十进制口径；
  `_steady_rate` 内嵌实现收编，TP-01 行为零变化实测 106.4s 与 v140 一致）。
- TP-02 删除 SkipCase 改实跑：双任务并发 + 组合稳态窗 + 合计 ±10% 断言
  （211.6s 实测）。
- TP-03 删除 SkipCase 改实跑：CLI 参数优先判别（512K 档带宽断言，206.4s 实测）。
- TP-06 删除 SkipCase 改实跑：单流路径限速 + UI 证据 + fixture 访问日志
  Range 形态证据（106.4s 实测，与 TP-01 同周期）。
- run() 增 QA_TP_ONLY 过滤器（分批前台复跑对策，缺省全量）。

**规程**
- `qa/01-throttle-proxy-qa.md`：v1.7 修订行 + 环境前置补 big-100m.bin 与
  QA_TP_ONLY 口径 + 速度断言口径升为权威量法段 + TP-02/03/06 判据行改写 +
  通过准则「本套件无常设 S 登记项」。

**工程沉淀（packs/_common/engineering.md）**
- 新增「工作区文件模式位卫生」条目：沙箱环境重置会把跟踪文件可执行位整体
  刷成 755，`git add -A` 照单全收经按索引打包的归档扩散（v140 归档 119 文件
  644→755 污染实证）；打包前按 `git ls-tree -r HEAD -z` 模式表逐文件归一化 +
  `git diff --cached --summary` 零 mode change + 归档成员模式抽验三重验证。

**打包卫生（本轮执行记录）**
- 索引模式归一化：122×100644 + 71×100755（71 为 HEAD 既有 755 集，含 bin/swarm、
  ezr-demo 预编译二进制与上游脚本），相对 HEAD 零纯模式变更。

**规格**
- features/ 与 mission 零改动（git 对账）。本轮为 QA 套件量法收口，
  产品行为零变更（产品树零改动，二进制同源重建）。

### 验证证据
| 验证项 | 结果 | 命令 |
|---|---|---|
| throttle 套件（分批） | **7P/0F/0S**：TP-01 106.4s / TP-02 211.6s / TP-03 206.4s / TP-04 4.9s / TP-05 43.3s / TP-06 106.4s / TP-07 2.3s（四批全部一次过） | `QA_TP_ONLY=<批> python3 suite_throttle_proxy.py`（runners 目录） |
| e2e 全量（13 套件） | **164 条 = 164P / 0F / 0S**：add 18 / ct 5 / de 16 / ic 12 / mt 9 / np 18 / pc 9 / rb 9 / rs 13 / sq 15 / tp 7 / td 15 / uc 18 | 逐套件 `python3 suite_*.py`（runners 目录，本轮全部实跑） |
| 产品测试 | 沿袭 v140 基线 **2775P/0F**（本轮零 rust 变更，二进制同源重建） | `cargo build`（debug 三件套 1m35s）后无需重跑 |
| 模式位对账 | 索引相对 HEAD **零 mode change**（122×644 + 71×755） | `git diff --cached --summary \| grep -c "mode change"` → 0 |
| 归档模式抽验 | tar 成员 755 仅限 HEAD 既有集 | `tar -tvf <归档> \| grep 100755` 对账 |

### 对账口径（基线 → 终态）
- **e2e 对账**：v140 基线 164 条 161P/0F/3S → 本轮 **164 条 164P/0F/0S**
  （+3P = TP-02/03/06 S→P；套件用例总数不变 164）。
- 测试计数沿袭 v140 的 2775P/0F（产品树零改动；属性测试零改动）。
- 产品源码变更 = 0（git 对账产品树无 diff；上轮 ezr-proxy 桩本轮零触碰）。

### 交接四要素（QA）
1. **本地验证命令清单**（按序）：
   环境：`export PATH="$HOME/.cargo/bin:$PATH"`（rustup nightly 自动装）→
   cwd `project/ezr`：`cargo build`（e2e 依赖 target/debug 三件套）。
   cwd `project/qa/runners`：13 套件逐个 `python3 suite_*.py`；其中
   `suite_throttle_proxy.py` 用 `QA_TP_ONLY` 分批（建议 1,4,7 / 5,6 / 2 / 3
   四批）或直接全量（>13min，注意前台窗口上限）→ 汇总 164P/0F/0S。
   产品树如需复验：`CARGO_PROFILE_DEV_DEBUG=0 cargo test --all-targets`
   （沿袭 2775P/0F）→ clippy/fmt/arch。
2. **会话运行时产物**（`.work/tmp/`，`complete` 时清除）：无失败现场残留
   （本轮零 F）。
3. **规格与 stamp**：features/ 零改动；01-persistence-config.feature stamp 维持
   保留（v138 处置），gherkin-mutator 恢复后按四要素③删除重跑。
4. **等价突变体清单及失效条件**：沿袭 v137/v138/v139/v140 登记
   `src/app/engine/evt.rs:297:23 replace > with >= in App::handle_failure`（失效
   条件三条沿袭）；本轮零产品变更，未新增等价突变体。

### 待办与待批（未决项）
- 无新增未决待批项。
- 沿袭移交（操作者后续会话可选项，非缺陷）：gherkin-mutator 运行器基建（见
  v138 交接遗留受限项）。**TP-02/03/06 已于本轮清账，不再是遗留项。**
- 建议：本 handoff 为上游交接内容，操作者可直接 `bin/swarm complete` 归档交付
  （本轮验证证据已闭环；打包前模式位三重验证已固化于 engineering.md）。

### 各角色合并吸收清单（QA 移交）
- **specifier**：规格零回改。TP 三条量法为 QA 基建口径（规程环境前置已改写），
  产品 FR 面无需新增。
- **coder**：产品树零触碰。后续若改限速器（Throttle）语义，注意 e2e 三条
  100MB 量法用例的耗时预算（单套件 >13min，分批跑法见 QA_TP_ONLY）。
- **cleaner**：本轮无新增重复；`_detail_dl` 为上提收编（消一处内嵌重复），
  CPD 基线未复测（沿袭）。
- **architect**：无产品结构变更；套件层新增模块级助手一个，arch_check 不涉。
- **hardender**：零产品变更，变异靶面无新增；三条 100MB 量法用例为行为
  回归锁（限速器语义变化的天然验收面）。

### 移交建议
- 操作者可直接 `bin/swarm complete` 归档交付（本轮验证证据已闭环）。
- 操作者如需继续新 feature：`bin/swarm begin six-pack/specifier`。
- 交付目录注意：`swarm-20261013-145434-six-pack-QA.tar.gz`（v140）含模式位
  污染（119 文件误 755），本 v141 归档为修复后的正确基线，建议以 v141 替代
  v140 归档对外分发。
