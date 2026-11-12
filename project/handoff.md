# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（终验轮已完成，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner 收敛轮 ⇒ architect 收敛轮 ⇒ QA 终验轮 ⇒ specifier 显示指标/调整轮 v1.48–v1.51 ⇒ coder 产品实现轮（FR-01-117）⇒ coder 补偿轮（v1.47–v1.50 六项欠账）⇒ cleaner 清理收敛轮 ⇒ architect 架构收敛轮 ⇒ **QA 终验轮（本轮 2026-11-12）** ⇒ Done（QA 是最后一名角色，six-pack 流转路径走完）
> 会话：six-pack-QA-20261112（任务 = QA 终验独立验证；上游交接 = project/handoff.md architect 收敛轮版）
> 环境：nightly 1.102.0（9c5648833 2026-11-10，与交接基线同 commit）@ ~/.cargo/bin（TUNA 镜像前台装齐）+ cargo registry rsproxy sparse；度量四件本轮重建：llvm-cov 0.9.1 / PMD CPD 7.29.0（.tools/pmd）/ bb 1.13.229（.tools/bin/bb）/ APS（.tools/Acceptance-Pipeline-Specification）；harness 依赖 libtorrent 2.1.1.0 + torf + pyte（pip 阿里云镜像）

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.51**）+ `mission/` 两份 phase 详述（**phase-01 v1.50**、**phase-02 v1.20**）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-10 QA 终验轮） | ezr 产品实现 + 15 套件 e2e + 引擎拆分 + 终验 | 已交付 | 历轮对账在案 |
| specifier 显示指标/调整轮 v1.48–v1.51（2026-11-10/11） | phase-01 v1.50 / phase-02 v1.20 + mission.md v1.47–v1.51 + features/qa 配套 + ezr-demo 同构修改 | 已交付 | 历轮在案 |
| coder 产品实现轮（2026-11-11，FR-01-117） | project/ezr 源码 6 文件 + 守卫三守卫 | 已交付 | cargo test 在案 |
| coder 补偿轮（2026-11-11，v1.47–v1.50 六项欠账） | project/ezr 源码 9 文件 + 守卫 10 条 | 已交付 | 在案 |
| cleaner 清理收敛轮（2026-11-12） | lint 基线恢复 + conn_flags 表驱动 + CRAP 收敛 + 守卫 8 条 + notes 三分类 | 已交付 | 在案 |
| architect 架构收敛轮（2026-11-12） | 四阶段评审 + VS15 工具族单源化 + arch_check 18 条 + 属性 +4 + notes | 已交付 | **fmt clean；clippy 0W；release 0W；8060P/0F；arch_check 18 全过；属性 86P** |
| **QA 终验轮（本轮 2026-11-12）** | **全量独立复验 + e2e 15 套 191P + APS 20 feature + CRAP/DRY 度量 + 修复 4 项（1 属性拆臂 + 3 套件锚）+ notes/INDEX** | 已交付 | **见第二节验证证据表（与交接基线逐项持平）** |

### 遗留受限项（当前有效，沿历轮口径）
- leech 专属 e2e、变异实跑（hardender 未跑，`--file` 分块消费面就绪）、6 个 BT/命令窗口族无专属 runner、ws(s) announce 无代理面。
- ezr-demo verify_ezr.py 预存 FAIL 15 项维持（脚本锚点漂移，非产品回归）；本轮 ezr-demo 零改动。
- CRAP comp 残余登记沿 cleaner 复核口径维持（run/on_evt/draw_conns/pieces/restore/overlay_open/on_cmd_key/fake_udp_tracker 族）——本轮全目标复跑抽查最大项（ws_prepare_offers / on_mouse_trackers / SOCKS5 握手）均落 e2e 专属路径/交互层/夹具既定类，无新增可修点。
- 变异点 >100 四文件登记维持（app/mod.rs / bt/engine/conn.rs / ui/text.rs / model/task.rs）。
- CPD min200 登记维持：bt/engine/mod.rs 测试克隆 ×7 + ezr-mktorrent ↔ bencode 设计性重复 ×1（本轮实扫 8/8 与登记一致，产品家族 0）。

### 实现定义值登记（累计，当前有效口径）
- D44–D74 沿历轮登记不变，本轮无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（QA 终验轮）
1. **静态基线复验**：cargo fmt --check clean / clippy --all-targets 0W / release 0W（5m13s = 交接基线）/ arch_check.sh 18 条全过。
2. **全量测试复验**：bin ezr 1028P（=942 单测 + 86 属性）/ ezr-proxy 15P / ezr-fixture 30P / ezr-mktorrent 1P / 集成 12 目标 6986P——**全量 8060P/0F 与交接基线逐项持平**；属性独立命令 86P/0F。
3. **e2e QA 套件全量**：15 套 **191P/0F**（add_task 21 / config_template 5 / download_engine 16 / integrity_check 12 / modify_bt 4 / modify_task 10 / named_proxy 18 / persistence_config 11 / resume_sidecar 14 / retry_backoff 9 / slots_queue 15 / throttle_proxy 10〔QA_TP_ONLY 四块拼接〕/ tui_display 21 / ui_conns 18 / ui_pieces 7）。
4. **修复 4 项（均最小改动）**：
   - ①`prop_scrollbar_bar_geometry_monotone` 过宽臂拆分（src/property_tests.rs，llvm-cov 轮 proptest 随机探索中签的最小失败输入；裁决 = 属性错非产品错：`page==0 && total>page` 时 max=total>0 几何合法，render/hit 的 page==0 守卫与 TD-20 显示条件一致；拆臂后属性 86P/0F 复绿，回归文件落盘输入成常驻守卫；fmt/clippy/bin 1028P 复验通过）。
   - ②③④e2e 观察锚偏差修复 ×3（suite_download_engine.py DE-15 / suite_modify_bt.py MB-03 / suite_tui_display.py TD-17）——v1.49 明细行尺寸对/「未知」项退役后规程锚未同步；按实现口径断言 + docstring 偏差注记，产品零改动，**待批项见下**。
5. **APS 规格流水线**：20 份 feature 全量 gherkin-parser + ir-dry-checker 双检通过（bb 任务在 APS 仓目录内调用，40 份 JSON 零失败）。
6. **CRAP 度量（受限近似，全目标口径）**：llvm-cov 13 份逐目标 lcov 并集合并（插桩运行全绿）；行覆盖 **92.3%**（40553 DA 行口径）；CRAP>6 集中于 e2e 专属路径/交互层/夹具类，无新增可修点；计算脚本落 `qa/runners/crap_calc.py`（口径头注内嵌，随产物流转）。
7. **DRY 度量（PMD CPD 7.29，-l rust，阳性对照先行）**：min200 **8 家族与交接登记 8/8 精确对账**（产品家族 0 = architect 轮单源化保持）；min70 198 家族为本轮新基线读数（上游未留存 min70 计数，读数落 testing.md）。
8. **notes 追加**：testing.md 4 条 + environment.md 3 条 + INDEX.md 同步（QA 终验轮节）。
9. **本 handoff.md** 重写为当前有效口径。

### 验证证据（命令可复现）
| 验证项 | 结果 | 命令 |
|---|---|---|
| 格式化 | clean | `cargo fmt --check`（project/ezr） |
| Lint | 0 警告（属性拆臂后复验） | `cargo clippy --all-targets` |
| release 构建 | 0 错误 0 警告，5m13s（=基线） | `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo build --release` |
| 全量测试 | **8060P/0F 逐项持平基线** | `cargo test --bin ezr` + `--bin ezr-proxy/ezr-fixture/ezr-mktorrent` + `--test <12 目标>` |
| 属性独立命令 | 86P/0F | `cargo test --bin ezr property_tests::` |
| e2e 套件 | **15 套 191P/0F** | `python3 qa/runners/suite_*.py`（throttle_proxy 沿 QA_TP_ONLY 分块） |
| 架构检查 | 18 条全过 | `bash scripts/arch_check.sh` |
| APS | 20 feature 双检零失败 | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-parser <f> <out>.json && bb gherkin-ir-dry-checker <ir> <out>.dry.json)` |
| CRAP | 行覆盖 92.3%；登记类抽查同构 | `cargo llvm-cov --lcov --bin/--test <t>` ×13 + `python3 qa/runners/crap_calc.py <13 份 lcov>` |
| DRY | min200 8/8 对账；min70=198 | `.tools/pmd/bin/pmd cpd --dir src --language rust --minimum-tokens {200,70}` |

### 对账口径
- 本轮改动面 = **product src 仅 property_tests.rs 1 处拆臂**（cfg(test) 测试代码）+ qa/runners 3 套件断言修复 + qa/runners/crap_calc.py 新增 + notes 2 文件 + handoff；features/、mission/、规程 md **零改动**（drift 只登记不自改）。ezr-demo 零改动。
- 测试对账：全量 8060 = 1028+15+30+1+6986 分项自洽；bin 1028 = 942 单测 + 86 属性（属性修复后计数不变）；e2e 191 = 15 套分项和；集成结果沿用有效（property_tests.rs 不在集成编译域，已核验 tests/ 零引用 + 无 lib.rs）。
- **CRAP 口径差异声明**：本轮 comp 计数器为新建脚本（qa/runners/crap_calc.py，if/loop/while/for/match/&&/||/? 正则计数 + 闭包并组取极值），与 cleaner 轮脚本的 comp 绝对值**不可直接对表**（如 run 本轮 comp=38 vs 登记 110——计数粒度差异，非代码变化）；产品源码本轮零改动，登记结论沿用 cleaner 原文。行覆盖 92.3% vs cleaner 95.5% 的差异主要来自本轮全目标口径含 ezr-proxy/ezr-fixture/ezr-mktorrent 的 main 路径（e2e 经子进程覆盖、插桩不可见）。

### 待办与待批
- **【待批·移交 specifier】v1.49 观察锚漂移 4 处文档修订建议**（QA 已按实现口径断言并注记，验证不受阻）：①features/01-download-engine.feature 场景 15 THEN-1「等待行大小显示『未知』」→ 改「等待行不显示尺寸（尺寸未知不显示、无预取，以 fixture 请求日志为准）」；②qa/01-download-engine-qa.md QA-DE-15 行同口径；③qa/08-modify-bt-qa.md QA-MB-03 行「1.0/1.6 MB · 66.7%」→「任务行[状态]徽标后总字节 + 进度百分数」；④qa/01-tui-display-qa.md QA-TD-17 行「已下载字节」观察载体 →「头部会话已下载」。建议下一 specifier 轮一并处理。
- 变异实跑维持移交 hardender（受限项，操作者显式决定是否续跑）。
- 本轮无其他操作者待批事项。
- **产物交付（操作者 chat 指令）**：`bin/swarm complete` 归档包 + ezr release 二进制（ezr/ezr-proxy，最终源码构建）→ 复制 /home/z/my-project/download/（操作者执行）。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留），并按上文把产物复制到 /home/z/my-project/download/。
- **six-pack 流转路径（specifier → coder → cleaner → architect → hardender(可选，未跑) → QA → Done）已走完，工作流完结**；是否补跑 hardender 或开新修订轮由操作者显式决定（`bin/swarm begin <角色>`）。
