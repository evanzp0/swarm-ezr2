# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender 终态　会话：v127-hard（任务 v127-hard）
> 本轮（变异加固批次）：对 architect v126-arch 交付做清单制差分变异（78 唯一突变体全
> killed：0 存活 / 0 等价 / 0 unviable / 0 timeout）→ 软 Gherkin 变异（受限登记，移交 QA）
> → CRAP 复测（72 条与存量精确一致）→ DRY 复测（src-only 56 tiles，+1 样板 tile 登记移交）。
> **产品树字节级零改动**（pristine 逐文件 cmp 证明）；单测基线沿用：2523P/0F、clippy 0、
> fmt 0、arch 14 规则过、属性 57P/0F。

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
| hardender（v127-hard，本轮） | 清单制差分变异 78/78 killed + CRAP/DRY 复测对账 + 软 Gherkin 受限登记 + 经验沉淀 5 条（见二） | 已交付 | 产品树零改动（pristine cmp 0 diff）、基线全链沿用全绿 |

### 遗留受限项（当前有效）
- e2e 六套件滞后于规程（累计）：`suite_add_task.py`、`suite_ui_conns.py`、
  `suite_modify_task.py`、`suite_persistence_config.py`、`suite_tui_display.py`、
  `suite_integrity_check.py`——QA 会话须先同步全部受影响套件再跑 e2e（见二）。
- **软 Gherkin 验收变异受限（本轮新登记）**：`gherkin-mutator`（bb 版）工具在位、参数契约
  已读毕，但其必需的 `--runner-worker` 项目适配器唯一实现路径绑定 specifier 体系的 e2e QA
  runner（qa/runners/）——hardender 职责排除运行/维护 QA 套件，且六套件滞后未同步（判定
  不可信）。现状：13 份 feature 中仅 `01-persistence-config.feature` 有历史 stamp（2026-10-06
  轮，28/28 killed），其余 12 份无 stamp 无判定。**移交 QA 会话**：套件同步后按
  `packs/_common/notes/aps.md` + mutator-spec 构建适配器补齐判定（复现要点见二·四要素②）。
- **变异覆盖面受限（本轮登记）**：本轮差分变异清单 = 本批 delta（`src/app/mod.rs:436-439`
  tab_counts 访问器 + `src/ui/header.rs:120-129` 消费面）+ `src/model/consistency.rs` +
  `src/model/speed.rs` + `src/model/retry.rs` + `src/ui/header.rs:193/194` 邻位，合计 78
  突变体全 killed。全项目 scan 共 **2159 点/54 文件**（单文件最大 98，均 <100 门槛，沿用
  cleaner v125-clean 登记口径），其余文件未在本轮跑变异——受会话时间预算与每命令 600s
  上限约束，移交后续 hardender 轮按同协议（phase-1 快杀 + phase-2 全量复核）分批推进；
  scan 清单复现见二·四要素②。
- **CRAP 为受限近似口径**（Rust 无标准 CRAP 工具；llvm-cov lcov BRDA comp=分支记录数+1
  上界近似 + 源码决策点复核，近似口径偏差方向见 `packs/_common/notes/rust.md`）：
  本轮复测 **72 条近似 CRAP>6** 与 cleaner v125-clean 存量 72 条**精确一致**（其中 4 条
  NO_DA 型 = fixture bin/入口函数无测试执行数据），全部为既有分发核心/UI 绘制/解析函数
  （on_evt、draw、block_worker、from_toml 等，comp≈分支数属「拆不动残余高 comp 点」登记
  口径）；本轮产品面零改动，无新增高 comp 点。
- **CPD 剩余 56 tiles**（PMD 7.28，`-l rust --minimum-tokens 50`，src-only 口径，较上游
  55 **+1**）：+1 为 7 行 53 tokens `Block::default()` 惯用样板 4 文件 tile
  （app/engine/mod.rs:1252 / app/mod.rs:461 / ui/header.rs:32 / ui/list.rs:36，其中 2 文件
  位于 architect v126 触碰面，归因行号漂移跨阈值——形态为惯用样板非真重复回归，非本轮
  引入，**处置移交 coder/architect**）。补充口径 src+tests = 115 tiles（含 tests/ 夹具与
  supervisor 测试区 mock 样板，登记备查）。
- **本轮沙箱为全新环境**：工具链已重装——rustup nightly（rustc/cargo 1.101.0-nightly
  6f5da406b 2026-10-10，minimal + rustfmt + clippy + llvm-tools-preview），`~/.cargo` +
  `~/.rustup`，新 shell 需 `export PATH="$HOME/.cargo/bin:$PATH"` 补注入；另装
  cargo-mutants 27.1.0、cargo-llvm-cov 0.9.1、rustfilt（均 `~/.cargo/bin`）、PMD 7.28.0
  （`.tools/pmd-bin-7.28.0`，Java 21）、Babashka 1.12.197（`.tools/bin/bb`）+ APS 仓
  （`.tools/Acceptance-Pipeline-Specification`，APS 调用须 cd 到该目录）。
- 无其他受限项。

### 实现定义值登记（当前有效）
- 本轮无新增实现定义值。沿袭有效：D28/D29（仅添加落态与三钮布局）、D30（下调即时收缩）、
  D31（1MB 下限）、D32（Enter 语义）、D33（1s 归零时延）、D34（清码 R = 不校验直接收尾，
  v1.18 实现精化见 coder v124-code-batch5 轮）。修订 B fallback（文件不完整 → 作废断点
  从头重下）为 FR-01-22 作废语义同源的实现定义值，操作者可改判（见待办）。

## 二、当前产出情况（hardender v127-hard）

### 本会话产物清单
- **产品源码零改动**：pristine 逐文件 cmp 零差异；本轮加固测试新增亦为零——78 个突变体
  全部被既有测试（bin 自带单测 + tests/hardening* 挂载加固套件 + 属性测试之外的常规套件）
  杀灭，无存活体需要新增加固测试。
- `packs/_common/notes/cargo-mutants.md`：经验沉淀 3 组——「被中断轮次的 caught 不会自动
  并入 resume 账本（续跑前手动合并）」、「test-command 参数边界（`--` 两侧语义，假杀灭的
  识别与外科处置）」、「双口径加速（phase-1 快杀 + phase-2 全量复核）」。
- `packs/_common/notes/rust.md`：经验沉淀 2 条——「CRAP 近似解析器重建对账的两处排除边界
  （NO_DA 型 / `*_tests` 模块名形态）」、「机械等价改造可让惯用样板跨过 CPD 阈值形成新
  tile」。

#### 交接四要素

**① 本地验证命令清单（按序，含加固套件入口）**（cwd：`project/ezr/`）：
```sh
cargo test --workspace                                   # 全量 2523P/0F（含全部加固套件）
cargo clippy --all-targets                               # 0 警告
cargo fmt --check                                        # 0 偏差
bash scripts/arch_check.sh                               # 14 规则全过
cargo test --bin ezr property_tests::                    # 属性独立口径 57P/0F
cargo test --test hardening --test hardening_g_main --test hardening_g_appcore \
  --test hardening_g_engine --test hardening_g_supervisor --test hardening_g_proxy \
  --test hardening_g_throttle_err --test hardening_v115 --test hardening_ui_backfill \
  --test hardening_engine_backfill                       # 加固套件独立入口（已含于 --workspace）
cargo llvm-cov --lcov --branch --output-path <out>/cov.lcov -- --skip property_tests::  # CRAP 输入
pmd cpd -l rust --minimum-tokens 50 --dir <ezr>/src      # DRY（src-only 口径 56 tiles）
```

**② 会话运行时产物位置与复现方法**（全部在 `.work/tmp/`，`complete` 后随会话清除，复现
命令如下；变异 scratch 副本由工具置于系统临时目录随轮次自清理）：
- 变异点 scan：`cargo mutants --list --line-col=true > mutants_scan.txt`（2159 行）。
- 变异跑批（phase-1 快杀口径）：`cargo mutants -j 2 -t 150 --build-timeout 300 --baseline
  skip --iterate -F '<清单正则>' -o <work>/mutrun_p1 -- --bin ezr -- --skip property_tests::`
  ；phase-2/3 幸存者复核：同形但去 `-- --bin…` 段（默认全量命令）、`-t 300`。
  本轮账本：`.work/tmp/mutrun{,_p1,_p2,_p3}/mutants.out/previously_caught.txt`。
- CRAP：lcov 见 `.work/tmp/cov.lcov`；解析脚本 `.work/tmp/crap_calc.py`（用法
  `python3 crap_calc.py cov.lcov > crap_report.txt`，依赖 rustfilt demangle）。
- DRY：`.work/tmp/cpd_src.log`（src-only）与 `cpd_srctests.log`（src+tests）。
- pristine 备份：`.work/tmp/pristine/ezr/`（逐文件 cmp 基线）。

**③ feature 注释块与差分复用注意事项**：`features/01-persistence-config.feature` 头部含
`# mutation-stamp: sha256=…` + `# acceptance-mutation-manifest-begin/end` 注释块（2026-10-06
轮产物，28/28 killed）——内容未变时差分复用会跳过全部场景，**强制重跑须删除该注释块**；
其余 12 份 feature 无注释块。任何会话重写 feature 内容后 stamp 失效属预期行为。

**④ 等价突变体清单及失效条件**：**空清单**——本轮 78/78 killed，0 存活，无需等价论证。
回补触发：后续轮次若在相同清单面出现 missed，先按 `mutation-hardening.md` 三分法归因
（工具缺陷 / 覆盖缺口 / 等价类）再处置；本轮 killed 判定对「同文件新增消费方/新挂载测试
crate」持续有效，对「删除对应杀灭测试」立即失效。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 基线全量单测（开工时） | 2523P / 0F | `cargo test --workspace` |
| 基线 clippy / fmt / arch / 属性 | 0 / 0 / 14 规则过 / 57P-0F | 见①命令清单 |
| 差分变异（清单制） | **78/78 killed**，0 missed / 0 unviable / 0 timeout | 见②复现命令 |
| pristine 逐文件 cmp | 零差异（产品树字节级零改动） | `diff -rq .work/tmp/pristine/ezr project/ezr --exclude target` |
| CRAP（llvm-cov --branch 受限近似） | 377 具名产品函数，>6 共 **72 条**（与存量精确一致，含 4 NO_DA） | 见② crap_calc.py |
| DRY（PMD CPD src-only） | **56 tiles**（+1 惯用样板 tile 登记移交） | 见① pmd 命令 |
| 终局护栏 | arch 14 规则过、fmt 0（收尾复跑） | `bash scripts/arch_check.sh && cargo fmt --check` |

### 对账口径（基线 → 终态）
- 变异判定 78 个唯一突变体，分项可复算：本批 delta 10（`app/mod.rs:436-439` 5 + `ui/header.rs`
  120-129 行段 5）+ consistency 10 + speed 30 + retry 22 + header 193/194 邻位 6 = **78**。
  phase-1（bin 快杀口径）杀 55（45 speed/retry + 10 delta）+ consistency 10（首轮清单）；
  phase-1 幸存 7（speed evict/rate 比较算子）经 phase-2 全量套件复核全部被挂载加固测试
  杀灭（6 于 mutrun_p2、1 于 mutrun_p3，日志均含真实 `test result: FAILED`）；另 6 个
  header 193/194 邻位突变体（正则命中扩面）亦全量口径杀灭。
- 判定可信度：两度遭遇 test-command 参数边界假杀灭（`-o`、`--skip` 落错 `--` 侧），均按
  作废重建外科处置（ledger 剔除 + 正确命令重跑 + 日志证据复核）；终局账本内全部 killed
  判定均有对应日志真实测试失败证据。经验已沉淀 `notes/cargo-mutants.md`。
- 基线沿用依据：产品树字节级零改动（pristine cmp），开工基线（2523P/0F 全链绿）即终态
  基线；收尾复跑 arch/fmt 护栏确认。

### QA 会话须知（e2e 前必读）
- 套件同步清单（累计，不变）：六套件（见一·遗留受限项）先同步再跑 e2e。
- 本轮不改变任何可观测行为（产品零改动），套件断言无需调整。
- 软 Gherkin 补判定路径：gherkin-mutator（bb 任务，cd `.tools/Acceptance-Pipeline-Specification`
  调用）+ 自建 runner-worker 适配器（per-job 隔离、协议见 mutator-spec；放会话 .work/tmp/）；
  强制重跑已 stamp 的 feature 先删注释块（四要素③）。

### 待办与待批（未决项）
- **CPD +1 tile 处置**（`Block::default()` 样板 4 文件 tile）：移交 coder/architect 裁决
  （登记为既有形态维持，或提取共享 helper——后者属产品改动，非本轮职责）。
- 无新增裁决待批。沿袭待批：修订 B fallback（文件不完整 → 作废断点从头重下）为
  实现定义值，操作者可改判（如改判「保留台账从缺失处补下」，代价：稀疏文件空洞风险，
  不建议）。

### 移交建议
- 下一角色：**six-pack/QA**（SKILL 链序 architect → hardender → QA）。本批变异加固收口：
  零存活、零等价、零 unviable；CRAP/DRY 对账完成；产品树零改动。QA 会话要点：①先同步
  六套件再跑 e2e；②属性测试独立口径 `cargo test --bin ezr property_tests::`（57P/0F）；
  ③软 Gherkin 判定可按 QA 会话须知补齐；④变异覆盖面其余 2081 点的后续加固轮次按
  四要素②协议分批推进。
