# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA　会话：ezr-qa-final（任务 ezr-qa-final）
> 上游交接：hardender v147-hard（163 点差分变异 campaign：杀灭 11 + 等价 1 + 受限登记
> 5+1；加固套件 hardening_g_v147 12 测试；产品源码零改动；3363P/0F、clippy 0、fmt 0、
> arch 15、属性 62P）。
> 本轮（最终独立验证 + QA 规程可执行化 + UI 层端到端 + 双度量复测）：把 specifier 全部
> QA 规程落成 **13 个可执行端到端套件**（project/qa/runners/，伪终端按键/鼠标注入 +
> fixture 驱动，只经用户可达入口、零内部 API），腐化探针 **13/13 全绿 165P/0F/0S**；
> 修复 harness 缺口 1（ca.pem 派生件不入库致 fresh 环境 https 代理用例全灭）、套件陈旧
> 同步 1（resume_sidecar 对齐 mission v1.28）、**产品缺陷 1**（接续任务添加入口接续即钉
> name_locked，堵住失效重下去重漂移 `<name>.1` 违反规格「沿用既有名」）；终局验证序列
> 全绿（3363P/0F、属性 62P、clippy 0、fmt clean、arch 15/15）；上游移交项 ③④ 双度量
> 复测完成（覆盖率 94.37% / CRAP≥30 零发现；CPD 89 tiles / 产品区 15，双口径与上游
> 基线一致）。
> **six-pack 流转路径（specifier → coder → cleaner → architect → hardender → QA）已
> 走完，工作流完结。**

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.28，分期模式）+ `project/mission/phase-01.md`。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（v128-spec） | v1.28 URL 权威模型改版规格落账 | 已交付 | parser 3/3、dry-check 0 增量 |
| coder（v128-impl） | v1.28 URL 权威模型全量实现 | 已交付 | 2925P/0F |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F；CPD 17→15 |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 峰值访问器 + 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 hardening_g_v147（12 测试/314 断言组）+ 杀灭 11 + 等价 1 + 受限登记 5+1 | 已交付 | 3363P/0F、clippy 0、fmt 0、arch 15 过；产品源码零改动 |
| **QA（ezr-qa-final，本轮）** | **QA 规程可执行化 13 端到端套件 + harness 基建（EzrApp/Fixture/Suite）+ 套件同轮同步 v1.28 + 产品缺陷修复（接续钉名）+ 双度量复测** | 已交付 | **e2e 165P/0F/0S（13 套件）、3363P/0F、属性 62P、clippy 0、fmt clean、arch 15/15、覆盖 94.37%、CRAP≥30=0、CPD 89 tiles/产品区 15** |

### 遗留受限项（当前有效）
- **环境重建（跨会话不持久，沿袭 + 本轮复验）**：nightly 1.101.0-nightly
  （rustup minimal + clippy/rustfmt/llvm-tools）；lld 桥接 `~/.local/bin/ld.lld`；
  cargo-llvm-cov 0.9.1（`~/.local/bin/`）、PMD 7.19.0（`.tools/pmd-bin-7.19.0/`）、
  java 21、Python e2e 依赖 pyte 0.8.2/wcwidth 0.6.0——本轮全部实证在位并投入使用。
- **软 Gherkin 变异（沿袭，仍开放）**：APS 工具（gherkin-parser/gherkin-mutator）未装；
  01-persistence-config.feature 的 mutation-stamp（sha256=57e757…）维持保留处置。
- **观察受限突变体（沿袭 hardender 登记，仍开放）**：workers.rs:27:5/51:25/57:14/
  168:25、download.rs:277:52 共 5 条 + 等价 download.rs:85:27 的失效条件——run 级
  回补基建已铺，任何观察面扩展落地后应优先补杀（全文 `.work/tmp/EQUIV-v147.md`，
  会话运行时产物，复现方法见上游 hardender 交接第 2 要素口径）。
- **ezr-proxy.rs 测试基建（沿袭裁决）**：127 变异点不拆、CRAP/CPD 读数不追责
  （本轮 Top CRAP 23.0 即在该文件，属测试基建触碰面）。
- **磁盘**：本轮实测余 5.2G（llvm-cov 插桩轮 cargo clean 后重建，主 target 为
  CARGO_PROFILE_DEV_DEBUG=0 增量口径）。
- **沙箱运行时口径（复现提示）**：throttle_proxy 套件 TP-01/02/03 为慢速门控用例
  （106/211/206s），前台单命令时限内用套件内建 `QA_TP_ONLY=1,3` 分批（v141 设计），
  结果与全量单跑同口径。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D28–D34、v138/v140 口径（见上游 handoff 登记清单）。
- **本轮新增：无**（修复③ name_locked 是既有规格「沿用既有名」口径的落地，非新契约值；
  覆盖率 94.37% 与 CPD 89/15 为度量读数，非契约值）。
- 无未决待批项。

## 二、当前产出情况（QA ezr-qa-final）

### 本会话产物清单
- **`project/qa/runners/`（ specifier 13 份 QA 规程的可执行化落产物，本轮核心交付）**：
  `harness.py`（EzrApp 伪终端驱动：按键/鼠标/SGR 注入、画面/逐格配色断言、toast/
  列表/详情断言；Fixture serve 子进程 + JSONL 请求日志解析；Case/Suite P/F/S 记账、
  用例级 HOME/保存目录隔离；pyte display 防御渲染）+ 13 套件：
  `suite_config_template` (5)、`suite_add_task` (18)、`suite_tui_display` (15)、
  `suite_slots_queue` (15)、`suite_persistence_config` (9)、`suite_named_proxy` (18)、
  `suite_resume_sidecar` (14)、`suite_retry_backoff` (9)、`suite_integrity_check` (12)、
  `suite_modify_task` (9)、`suite_download_engine` (16)、`suite_throttle_proxy` (7)、
  `suite_ui_conns` (18)。
- **harness 修复①**：`_ensure_proxy_ca_pem`——ca.pem 是 server.der 派生件不入库，
  fresh 环境缺失致 named_proxy https 代理用例全灭；harness 内按需派生。
- **套件同轮同步②（腐化探针归因：套件陈旧，非产品缺陷）**：`suite_resume_sidecar`
  对齐 mission v1.28——RS-01 去 URL 字段断言、RS-07 去 URL 腿、新增 RS-14；
  规程文件与脚本同轮一致（`01-resume-sidecar-qa.md`）。
- **产品缺陷修复③（腐化探针归因：产品缺陷，按已接受规格最小修复）**：接续任务
  （按路径 sidecar 接续）在添加入口（dialogs.rs/cli.rs）接续即钉 `name_locked`——
  修复前首探失效重下去重漂移 `<name>.1`，违反规格「沿用既有名」；单测 10P +
  e2e resume_sidecar 14P 复验。
- **fmt 归一**：修复③新增断言 dialogs.rs:561 超宽行，`cargo fmt` 归一 + dialogs
  单测 7P 复验。
- **packs/_common/notes/rust.md（工具侧沉淀，DRY 修订既有条目）**：测试模块 lcov
  mangled 双形态（`testss_` ∪ `5tests\d`）排除口径。
- 会话运行时产物（**不进归档**）：`.work/tmp/qa-lcov.info`（llvm-cov lcov，--branch
  在位）、`crap_calc.py` + `qa-crap-top.txt`（CRAP 解析与全量清单）、
  `qa-cpd-src-report.txt` + `cpd_split.py`（CPD 报告与产品/测试区分解）、
  `qa-final-cargo-test.log`（全量测试日志）、`run_filtered.py`、`re-run_*.log`
  （套件复跑日志）、上游 hardender 运行时产物（mutants-out/pristine/EQUIV-v147.md 等）。

### 验证证据
| 验证项 | 结果 | 命令（project/ezr 下，`PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH" CARGO_PROFILE_DEV_DEBUG=0`） |
|---|---|---|
| 腐化探针（QA 终局第一动作） | **13/13 套件全绿 165P/0F/0S** | `cd project/qa/runners && python3 suite_<name>.py`（throttle_proxy 慢速用例可 `QA_TP_ONLY=1,3` 分批） |
| 全量测试 | **3363P / 0F**（15 target，与上游基线精确一致） | `cargo test` |
| 属性测试（独立口径） | 62P / 0F | `cargo test --bin ezr -- property_tests::` |
| clippy | 0 警告（--all-targets） | `cargo clippy --all-targets` |
| 格式 | clean | `cargo fmt --check` |
| 架构边界 | 15/15 规则过 | `bash scripts/arch_check.sh` |
| 覆盖率（上游移交项③） | **产品区行覆盖 94.37%**（14191/15037；上游 94.06% +0.31pp，漂移与修复③小改一致；口径=cargo test 行覆盖 sum(LH)/sum(LF)，不含 TUI 进程外 e2e 与 doc tests） | `cargo llvm-cov --lcov --branch --output-path .work/tmp/qa-lcov.info` |
| CRAP（上游移交项③） | **CRAP≥30 零发现**（产品区 1000 逻辑函数，Top 23.0 在 ezr-proxy.rs 测试基建）；阳性对照 spread_bytes fnda=4056 过 | `python3 .work/tmp/crap_calc.py .work/tmp/qa-lcov.info`（comp=BRDA 近似，rust.md 双偏差方向登记在案） |
| CPD/DRY（上游移交项④） | **89 tiles（src-only 口径，与上游零漂移）/ 纯产品区 15 = 上游 ✓**；涉 testutil 两 tile 均为 Dialog 构造惯用样板/夹具形态（登记移交，非真重复回归） | `pmd cpd -l rust --minimum-tokens 50 --dir src`（exit 4=发现重复，正常语义）+ `.work/tmp/cpd_split.py`（产品区/测试区分解） |

### 待办与待批（未决项）
- 无阻塞项、无待批项。
- **沿袭移交（非本轮产物缺口，均已登记在 §一受限项）**：① 5+1 观察受限突变体回补
  （run 级基建已铺）；② 软 Gherkin 变异（APS 安装 + runner 适配器）。

### 移交建议
- QA 是 six-pack 最后一名角色，本轮验证全部通过：**six-pack 流转路径（specifier →
  coder → cleaner → architect → hardender → QA）已走完，工作流完结**。建议操作者运行
  `bin/swarm complete` 归档终结本轮。
- 若启动新分期：从 mission 分期规划开始（`bin/swarm begin six-pack/specifier`），
  下游以本 handoff §一总账与 `project/mission.md` 为上游交接内容。
