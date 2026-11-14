# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（产品复核重验轮，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner/architect/QA 收敛链 ⇒ specifier 显示指标/调整/裁决/新增需求轮（v1.47–v1.53）⇒ specifier 裁决落实修订轮（v1.54）⇒ coder 产品实现轮（2026-11-13）⇒ **coder 复核重验轮（本轮 2026-11-14，产品端零新增需求；下一步建议 six-pack/cleaner 收敛轮）**
> 会话：six-pack-coder-20261114-052721（任务 = 上游交接「产品实现轮」复核重验：环境重建 + 全量单测/clippy/fmt 复核 + 实测对账；上游交接 = project/handoff.md 产品实现轮版）
> 环境：nightly 1.102.0-nightly @ ~/.cargo/bin（rustup 经 TUNA 镜像前台装齐，rust-toolchain.toml 钉 nightly）+ cargo registry rsproxy sparse（~/.cargo/config.toml）+ bb 1.13.229（.tools/bb/bb）+ APS 仓（.tools/Acceptance-Pipeline-Specification，codeload main 装入）+ python3.12 e2e 依赖已装（torf/libtorrent/pyte/wcwidth）；llvm-cov/cargo-mutants/PMD 未装机（本轮无度量需求，沿 architect 收敛轮先例）。本轮补记：全量构建瘦身口径 = `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`（target 1.4G；default 口径增量缓存 + debug=2 冲至 6.6G 爆盘不可行），且每 Bash 调用即新 shell、瘦身 env 须随每条命令显式重 export（漏带 = 指纹翻转全量重编爆盘）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.54**）+ `mission/` 两份 phase 详述（**phase-01 v1.53**、**phase-02 v1.22**）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-13 QA 观察项清理轮） | ezr 产品实现 + 15 套件 e2e + 收敛/终验链 | 已交付 | 历轮对账在案（单测 8060P/0F、e2e 191P/0F 基线，历环境口径） |
| specifier 新增需求轮（2026-11-13，v1.53） | mission v1.53 + phase 详述 + features 六份修订 + QA 六份同步 + ezr-demo 同构落地 | 已交付 | APS 双检 6/6；demo 单测 58P/0F、PTY 29 项全过 |
| specifier 裁决落实修订轮（2026-11-13，v1.54） | mission v1.54 + phase-01 v1.53/phase-02 v1.22 + features 三份修订 + QA 三份同步 + ezr-demo HTTP 臂 LT 同构同步 | 已交付 | APS 双检 3/3；demo 单测 58P/0F、PTY 28 项全过、clippy 零警告 |
| coder 产品实现轮（2026-11-13） | 产品端五项全量落地：FR-01-118（D75 行 1 折叠）/FR-02-37 v1.2（D78 四值形态）/FR-02-38 v1.21+v1.22（D77 SR/LT + 做种行退役 + HTTP LT）/FR-02-39（D76 顺序下载：对话框+引擎+持久化）/FR-02-40（D79 做种退出规则 + default_seed_duration_m + 模板 v1.18）+ help SR/LT 条目（82 行）+ 模型层 tracker/连接值分离 + registry seed_elapsed 持久化；产品源码零注释纪律保持 | 已交付 | 单测 8100P/0F、clippy --all-targets 0 警告、fmt 过（三项口径本轮全部实测复核成立；分项目标计数 6 处归属错位经本轮实测修正，总账不变） |
| **coder 复核重验轮（本轮 2026-11-14）** | **产品端零代码新增（src/ 零改动）；唯一产品仓改动 = tests/gen 剥头副本 4 份按契约机械重生成（修复上轮 detail_stripped 尾逗号漂移）+ notes 三文件沉淀 + 本 handoff 重写；13 目标单测 / clippy / fmt 三项全量复测** | 已交付 | 单测 **8100P/0F**（13 目标逐目标实测，与总账精确一致）、clippy --all-targets **0 警告 0 错误**、fmt --check **全量合规**、剥头副本契约测试 PASS |

### 遗留受限项（当前有效）
- **QA runner fixture 漂移 3 处（移交 QA/specifier 轮，coder 不维护 QA 套件——SKILL 边界）**：① `suite_modify_bt.py` 4F——docstring 内嵌 v1.20 前焦点链（树到「立即下载」6×Tab），v1.21 焦点环扩位后需 7×Tab，4 用例全走不到确认臂；② `suite_config_template.py` 5F——硬编码钦定原文 190 行基线，v1.17/v1.18 后为 207（split 口径）/206（lines 口径）；③ `suite_persistence_config.py` QA-PC-04 1F——用例写已废弃键 `seed_duration_secs = 0` 期待拒绝启动，v1.21 该键按未知键口径静默废弃（FR-02-35 ⑤/D79，产品行为符合规格；`default_seed_duration_m = 0` 拒绝启动已由单测锁定）。runner 同步欠账三层形态沿 testing.md 既登记例。
- leech 专属 e2e、变异实跑（hardender 未跑）、BT engine/add-bt/anti-leech/trackers-window/cmd-window 族无专属 runner（沿历轮）；`suite_bt_engine` 等六族文件在本仓不存在（非本轮新增缺口）。
- PMD/llvm-cov/cargo-mutants 未装机（无度量需求）。
- 本沙箱 rustup dist 面仅 TUNA 可用、rsproxy sparse registry 可达（dist 与 registry 可用性解耦）——镜像选型实测见 notes/environment.md 历轮追加；10G 盘全量构建必须走瘦身口径（本轮实测见 environment.md 本轮追加）。

### 实现定义值登记（累计，当前有效口径）
- D44–D74 沿历轮登记不变；D75–D79 已于 v1.54 落定（否决窗口关闭不可回退），上轮全部实现、本轮复核在位（关键符号与行为锚点抽查五项全过），实现取值与裁决逐条一致：
  - **D75** 折叠：上限 32 显示列、`...` 三 ASCII 句点、字符边界不劈半、仅行 1、预算 = min(动态预算, 32)（`fold_name_dots`，budget = max−3）。
  - **D76** 顺序切换：确认即生效不重置进度、`PieceOrderCache.sequential` 标志切换（索引升序 ⇄ rarest-first）、自首个缺口推进（partial-first 保留）、其余机制不变。
  - **D77** SR 做种/下载两臂同构；LT = BT 下载臂 + HTTP 任务行两态（`↓ <speed> LT <eta>` / `↓ - LT -`）；帮助 82 行。
  - **D78** x1(x2)/y1(y2)：x2/y2 = tracker 最新成功应答 complete/incomplete（`Option`，未知省括号段显示 `x1/y1`）、`-/-/-` 三段不变、做种臂同构；y1 = 连接 peers 总数（含种子，BtConns 快照计数）。
  - **D79** 三键 AND；share_ratio_gt 正浮点保留 1 位小数（>1 位小数拒绝启动）、seeders_count_gt/seed_duration_m 正整数（min1）、0/负拒绝启动；exit rule 任一键 → default_seed_duration_m 失效（seed_left = ∞）；子表缺省 → default_seed_duration_m 生效（30 分钟）；seed_duration_m 按 x1 实际连接种子/累计做种时长（registry 持久化续走）；share_ratio_gt 判定用 `Task::share_ratio_value`（qBt realRatio 同源，∞ 满足任意正阈值）。

## 二、当前产出情况

### 本会话产物清单（coder 复核重验轮）
1. **剥头副本契约修复（唯一产品仓改动面）**：`tests/gen/{chart,clipboard,detail,btn}_stripped.rs` 4 份按契约测试同构变换（跳过起始 `//!` 行）由源码机械重生成——根因 = 上轮对 fixture 侧手改 `sequential: false` 少尾逗号（src 侧经 cargo fmt 有逗号；cargo fmt 不覆盖 include! 装载文件，双侧手改必漂）；`gen_fixtures_match_product_sources` 由 FAILED → PASS，`src/` 与测试零改动（cargo fmt --check exit 0 佐证）。
2. **全量单测复跑**：13 目标逐目标落盘 `.work/tmp/t_*.log`，分项实测对账表见验证证据（上轮 6 处目标级归属错位修正）。
3. **clippy + fmt 复核**：`cargo clippy --all-targets` 0 警告 0 错误；`cargo fmt --check` 全量合规（exit 0）。
4. **notes 沉淀**：`project/notes/` testing/environment 两文件本轮追加 + INDEX 同步；跨项目通用条目（rustfmt/cargo fmt 不覆盖 include! 文件）入 `packs/_common/notes/rust.md` §7。
5. **本 handoff.md** 重写为复核重验轮口径。

### 验证证据（命令可复现）
| 验证项 | 结果 | 命令 |
|---|---|---|
| 单元/集成测试 | **8100P/0F**（13 目标实测：bin 1040、g_appcore 814、g_slots 254、g_engine 810、g_proxy 16、g_main 2、g_supervisor 783、hardening 247、v115 951、engine_backfill 928、g_throttle_err 500、g_v147 797、ui_backfill 958——分项为上轮归属错位修正后口径，总和 8100 与总账精确一致，历轮守卫零丢失） | `cd project/ezr && CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo test --bin ezr` / `cargo test --test <目标>`（逐目标日志 .work/tmp/t*.log） |
| 剥头副本契约 | **PASS**（4 副本与源逐行一致；修复前 detail_stripped 尾逗号漂移 1 断言红） | 同上（gen_fixtures_match_product_sources，挂载于 hardening_ui_backfill） |
| clippy | **0 警告 0 错误**（all-targets，nightly 1.102.0 同版本复核） | `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo clippy --all-targets` |
| fmt | 过（exit 0） | `cargo fmt --check` |
| e2e 套件 | 未复跑（coder SKILL 边界：不运行 QA 套件；沿上轮登记口径 181P/5F，5F = 套件 fixture 漂移交 QA 轮） | （移交 QA 轮复跑） |

### 对账口径
- 本轮改动面 = tests/gen 4 副本重生成 + notes 三文件（testing/environment/INDEX）+ packs/_common/notes/rust.md 一条目 + handoff；features/qa/mission 零改动（coder 只读纪律）；产品 `src/` 零改动（fmt --check 与单测双佐证）；ezr-demo 零改动。
- 单测对账：8100 = 1040+814+254+810+16+2+783+247+951+928+500+797+958（逐目标日志可复算；与上轮同总数——分项 6 处归属错位即上轮分项表与实际目标-数字映射不符，总数与 0F 口径两轮一致）。
- 上轮声称口径收敛情况：①「fmt 过」src 侧成立（--check exit 0）；②「ui_backfill 958P」上轮不成立（fixture 漂移致 957P/1F）、本轮修复后成立；③「8100P/0F 总账」两轮实测一致成立。教训入 notes：剥头副本双侧手改必漂，一律由源码机械重生成。
- 验证环境口径：瘦身 env（INC=0 + DEBUG=0）不影响测试语义与计数（上轮历环境口径同总数 8100 互证）；target/ 全量重建自零（complete 清理后），非增量残留口径。

### 待办与待批
- **runner 漂移 3 处移交 QA 轮**（suite_modify_bt 焦点链 7×Tab + suite_config_template 207 行基线 + suite_persistence_config QA-PC-04 废弃键翻转）——建议随下一 cleaner/QA 轮按 testing.md「runner 同步欠账三层形态」修法重写，本轮不越权改动。
- 变异实跑维持移交 hardender；leech e2e 缺口沿历轮。
- 本轮无阻塞项、无新增待批项（上轮 D75–D79 实现口径经复核无新歧义）。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 下一节点建议 = `bin/swarm begin six-pack/cleaner`（收敛轮：覆盖率/CRAP/DRY 对上轮新增面复测、改动面纪律复查）；或按操作者裁量直接 QA 验证轮（QA runner 修复后可全绿复跑）。
