# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA　会话：QA-20261005（终局角色，工作流完结）

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（ezr download）：TUI 高性能多协议下载器，phase-01（HTTP/HTTPS 真实下载
  内核与 TUI 正式版）已交付并通过 QA 终局独立验证。需求权威来源 `project/mission.md`
  （分期模式），phase-01 详述见 `project/mission/phase-01.md`，feature 规格在
  `project/features/`，QA 规程与可执行套件在 `project/qa/`。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier | mission.md + mission/phase-01.md + features/ 9 份规格 + qa/ 9 套件 | 已交付 | 上游既定口径 |
| coder（phase-01） | ezr 下载内核 + TUI + ezr-fixture/ezr-proxy + 属性测试 | 已交付 | 上游既定口径（258 测基线） |
| cleaner / architect（历轮） | 工具链钉 nightly、模块边界收敛、两轮架构复核 | 已交付 | arch_check 8/8、clippy 0、fmt 0（其会话口径） |
| hardender（20261004） | 变异加固：7 个 hardening_g_* 套件 ~110 条新测试；等价/超时/环境型台账 | 已交付 | 加固后全绿；equivalence 台账见其归档 |
| coder（20261004） | 空闲期注册表零写放大修复：model 层 `save_json_atomic` 内容幂等短路 + 3 条聚焦单测 | 已交付 | 全量 955P/0F；产品面 clippy 0；arch_check 8/8 |
| cleaner（20261005） | tests/hardening* 环境漂移收敛：14 文件 fmt 归零 + 7 套件 clippy 警告归零；度量流水线全量复跑 | 已交付 | 全树 fmt 0；clippy 0；955P/0F 零漂移；arch_check 8/8；src/ 零改动 |
| architect（20261005） | 架构评审四阶段 + lint 姿态块单源化（16 文件净 −280 行）+ engine 窄接口收口 + 属性测试 24→30 + arch_check 增至 10 规则（含阳性对照） | 已交付 | 961P/0F；clippy 0；fmt 0；arch_check 10/10；CPD 100-token 0；行覆盖 90.4% |
| **QA（20261005）** | QA 最终独立验证：9 套 PTY e2e 全部执行通过 + 一致性审查 3 处漂移修复 + 产品 bug 修复 3 处（失效重启任务名钉住 / 会话累计账本 / CD 命名回写）+ 新增 5 测试 + DRY/CRAP 复算与回归修复 + README 验证方式收尾 | **已交付（终局）** | 966P/0F；9 套 e2e 114 用例 = 111P/3S（S 均预登记环境/方法学受限）；CPD 100-token 0；行覆盖 90.5%；arch_check 10/10；clippy 0；fmt 0（详见第二节） |

### 遗留受限项（当前有效）
- **CRAP 为受限近似口径**（继承上游）：无标准 CRAP 工具，以 llvm-cov lcov 分支计数代入
  公式估算（comp=函数区间 BRDA 条目数+1，`testss_` 排除测试模块，闭包归并取极值）。
  QA 轮复算：产品函数 comp>6 或 CRAP>6 共 91 个（architect 轮 79、cleaner 轮 88；差异
  来自闭包归并区间归属实现 + 各轮间产品代码分支数变化，同为受限近似，多轮读数不可直接
  互减），集中于 app(33)/ui(15)/engine(14) 交互分发核心；不拆裁决继承 architect 轮（见待批节）。
- **e2e 套件预登记 S 项（3 用例）**：retry-backoff RB-09（64MB 受限目录：沙箱无
  mount/配额权限，磁盘预检路径已由代码审查+单测覆盖，环境满足时以受限目录实测）；
  throttle-proxy TP-02/TP-03/TP-06（loopback 短传输绝对速率受启动瞬态支配，测量方法学
  受限；限速生效语义由 TP-01 100MB 总量口径权威覆盖）。
- **沙箱无 TTY**：TUI 进程级端到端（真实按键/渲染）不可自动化；pty E2E 已由 QA 以
  pyte 伪终端口径完成（9 套全绿）。
- **gherkin-mutator 不存在于 npm registry**（继承上游）：Gherkin 差分变异受限，feature 无
  步骤处理器绑定。
- **工具链 PATH 注入不跨调用持久**：`cargo` 等需 `export PATH="$HOME/.cargo/bin:$PATH"`
  （lld 桥接另需 `~/.local/bin`；工具链本体在位）。
- **GitHub release 资产直链在本沙箱 404**（cargo-mutants 仓库）：经 crates.io
  `cargo install cargo-mutants --version 27.1.0` 安装（编译安装，非预编译版）。

### 实现定义值登记
- 无新增（QA 轮未引入契约值；keep_name 属修复口径回归既有规格 FR-01-22/02/26）。
  既有登记继承上游归档。

## 二、当前产出情况（QA-20261005）

### 本会话产物清单
- **QA 套件一致性审查（3 处漂移修复）**：① suite_add_task 死条件装饰器残留
  （"QA-10" if False）清理；② suite_tui_display TD-03 误断言已撤销的「并发分块明细」
  反转为 assert_not_in（FR-01-81 修订二口径）；③ TD-04 测已撤销的连接明细表 → 删除，
  补齐规程已有而脚本缺失的 TD-15（1s 节拍/EMA 归零/爬升）。
- **产品 bug 修复 3 处（e2e 暴露，最小修复）**：① 失效重启任务名钉住——`TaskSpec.keep_name`
  （engine/supervisor.rs + app/engine.rs：final_url 已定后一切重启沿用既有名，残留
  `.downloading` 不得触发去重改名导致 sidecar 落盘名与任务名错位、FR-01-22 一致性检查
  被跳过）；② 会话累计下载字节改增量账本——`session_seen` HashMap 按任务真实下载字节
  累计（FR-01-81：EMA 爬升期少计随任务结束固化，3MB 短任务实测少计约一半），失效重下
  基线同步归零；③ CD 命名回写去除盘查（引擎侧已先于建文件去重，事件异步处理时盘查会
  把自己的 `.downloading` 当占用者而漂移新名）。
- **新增测试 5 个**：supervisor keep_name 重启去重语义（keep_name_restart_skips_dedupe，
  含 true/false 双路径）+ tests/hardening_g_engine.rs 扩展。
- **e2e 套件缺陷修复 3 处**（失败归因后最小修复）：① RB-07 两示例行独立 HOME
  （Scenario Outline 独立场景纪律）+ R 重试锚定重试计数 1/5→2/5 持久证据（探针秒败型
  失败瞬时态不可观察）；② TD-02/TD-15 门控慢速下载经配置缩小 HTTP 块至 128KB +
  TD-15 限速参数改 30000（fixture ?speed 为每请求口径，3 连接聚合下 3MB 需 ~34s，
  保证采样/暂停/恢复全程任务活跃）；③ TD-15 Sparkline 点数口径修正（限定「↓ 全局速度」
  面板内按列去重，排除进度条 █ 字形与纵向柱高幅度污染）。
- **度量流水线复算与回归修复**：PMD CPD 首跑检出 1 处 101-token 重复（新增 keep_name
  测试两段事件泵）→ 提取 `wait_download_done` 辅助函数消除 → 复跑 0 重复；阳性对照
  60-token 检出 14 处（检测能力确认）。cargo-llvm-cov 0.9.1 重装（上游同版）后 CRAP/覆盖率
  复算（口径见遗留受限项）。
- **README 验证方式收尾**：cargo test 计数 258→966、arch_check 6→10 规则、自动化验证
  补 PTY e2e 套件入口（9 套 114 用例，与 mission §5.3「端到端 QA 套件可本地重复执行」衔接）；
  release 三二进制构建（ezr 4.5MB / ezr-fixture / ezr-proxy，补齐 §5.4 交付物）。
- **工具侧沉淀（宪法第三章）**：`packs/six-pack/QA/SKILL.md` 端到端验证纪律扩展 2 条
  （探针秒败型瞬时中间态锚持久证据；Scenario Outline 独立现场的隔离粒度）+ 新增 2 条
  （门控慢速场景观察窗下限与聚合速率口径；画面字符统计限区域按列去重）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量测试 | 966P / 0F（10 个测试二进制） | `cargo test`（project/ezr 下） |
| fmt 全树 | 0 偏差 | `cargo fmt --check` |
| clippy 全目标 | 0 警告 | `cargo clippy --all-targets` |
| 架构边界 | 10/10 通过 | `bash scripts/arch_check.sh` |
| e2e add-task | P=17 F=0 | `python3 suite_add_task.py`（project/qa/runners 下，下同） |
| e2e download-engine | P=16 F=0 | `python3 suite_download_engine.py` |
| e2e integrity-check | P=11 F=0 | `python3 suite_integrity_check.py` |
| e2e persistence-config | P=9 F=0 | `python3 suite_persistence_config.py` |
| e2e resume-sidecar | P=13 F=0（连续两轮确认） | `python3 suite_resume_sidecar.py` |
| e2e retry-backoff | P=9 F=1→修复→0；S=1（RB-09 预登记受限） | `python3 suite_retry_backoff.py` |
| e2e slots-queue | P=13 F=0 | `python3 suite_slots_queue.py` |
| e2e throttle-proxy | P=8 F=0；S=3（TP-02/03/06 预登记受限） | `python3 suite_throttle_proxy.py` |
| e2e tui-display | P=12 F=2→修复→14 F=0 | `python3 suite_tui_display.py` |
| DRY（CPD） | 100-token 重复 0 处（阳性对照 60-token 14 处确认检测生效） | `pmd cpd --minimum-tokens 100 -l rust src`（PMD 7.28.0，.tools/ 下） |
| 覆盖率 | 合并行覆盖 90.5%（8449 DA 行/7644 命中） | `cargo llvm-cov --branch --lcov --output-path <path>`（0.9.1） |
| CRAP 受限近似 | comp>6 或 CRAP>6 共 91 个（口径与归因见遗留受限项） | lcov 分支计数代入（脚本 `.work/tmp/crap_calc.py`，会话级） |
| release 构建 | 三二进制在位（ezr 4.5MB） | `cargo build --release` |

### 对账口径
- 测试计数：961 → 966（+5P/−0F），+5 全部为 QA 轮产品修复新增测试（keep_name 语义
  1 + hardening_g_engine 扩展 4）；既有测试零删除。
- 覆盖率：90.4% → 90.5%（DA 行 8399→8449、命中 7596→7644，净 +50 覆盖行来自新增测试
  与修复代码路径），持平口径。
- CRAP：79 → 91：① QA 轮产品修复新增分支（session_seen 账本、keep_name、CD 命名回写
  修正）②闭包归并区间归属实现差异（architect handoff 已登记该口径噪声，多轮读数不可
  直接互减）。上游点名示例 comp 值精确对齐（mouse on_mouse 29 / dialog_keys add_dialog
  19 / draw_add_dialog 37），区间口径一致。
- 产品行为改动均为 e2e 暴露缺陷的最小修复，与已接受规格一致（FR-01-22/02/26/81、
  Gherkin 01-add-task-10）；每处修复附对应单测/e2e 证据，全量 966P/0F + 覆盖率持平为证。

### 待办与待批（未决项）
- **CRAP>6 高 comp 点裁决**：继承 architect 轮裁决**不拆**（79→91 个集中于交互分发核心
  dialog_keys/mouse/dialog/main 事件循环与 engine on_evt/tick 闭包，属「依赖共享可变
  状态与对象生命周期的长分派拆分」不安全类；变异点扫描无单文件超 100 点，无强拆义务）；
  QA 轮 e2e 全绿未发现该口径的可观察缺陷，裁决维持，待操作者确认或否决。
- app/engine.rs `on_evt` 部分 arm/closure 单测覆盖 0%（CRAP 12-56 段）：继承 architect
  轮登记；QA 轮以 PTY e2e 从用户可达入口覆盖了其可观察行为（on_evt 驱动的全部界面状态
  转换均有套件断言），内部 arm 级单测缺口仍待 coder/hardender 类角色按需补齐。
- 继承上游待批：单实例锁无 home 回退分支的 flock 缺失，待操作者裁决。

### 移交建议
- QA 为 six-pack 最后一名角色，验证全部通过，**工作流完结**：建议操作者运行
  `bin/swarm complete` 归档本轮产物。
- 如需继续项目：phase-02（BT/磁力链）开工时由操作者显式启动新会话
  （`bin/swarm begin six-pack/specifier`，依宪法第一章补写 phase-02 需求详述）。
