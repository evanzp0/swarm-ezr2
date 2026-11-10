# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（修订实现轮已完成 + 2026-11-10 补缺口轮已完成）——**下游 cleaner 待操作者显式启动**
> 路径：specifier → coder（2026-11-09 上午，ws/webrtc 轮）⇒ 诊断/复验/bug28 轮（2026-11-09 下午）⇒ specifier 修订轮（2026-11-09 下午）⇒ coder 修订实现轮（2026-11-09 下午）⇒ **coder 补缺口轮（本轮，2026-11-10，操作者 chat 指令：seed_left 倒计时+归零臂+QA）** ⇒ cleaner → architect → QA（待操作者显式 begin）
> 会话：six-pack-coder-20261109-163354（任务 = 落实 specifier 修订轮交接的实现范围：①FR-02-03 v1.16 做种回退臂；②FR-02-19 v1.16 磁盘删除；③下载中补下核验；④断言翻转面。上游交接 = project/handoff.md specifier 修订轮版）
> 环境：nightly 1.102.0（rustup 经 rsproxy 官方双变量前台装齐，版本带自 1.101 首漂）+ cargo registry rsproxy sparse + bb 1.12.197 / APS 经 gh-proxy.com 前缀装于 `.tools/`（工具缓存不进归档）；RAG 索引 `.work/rag` 已随产出刷新

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，v1.46）+ `mission/` 4 份 phase 详述（phase-02 v1.16 为本轮实现依据）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder 历轮（含 ws/webrtc 轮 + bug28 轮） | ezr 产品实现 + 12 集成测试目标 + bin 单测；bug6–bug28 修复；FR-02-21/31/33/08 v1.15 + FR-02-34/35 全量实现；ezr-demo 同构同步 | 已交付 | bin 983P + 集成 12 目标 6761P 全绿（历轮口径） |
| cleaner 历轮 | 保持行为清理 + 四件度量 | 已交付 | 全绿 |
| architect（2026-11-07） | 四阶段评审 + arch_check 17 规则 + 属性测试 76P | 已交付 | bin 951P / 集成 6357P / CPD 181 |
| hardender | —（操作者 20261109 裁决定为 six-pack 可选节点，默认链不含） | 不适用 | — |
| QA + 终验（2026-11-06/07） | 14 套件 187P 实跑 + harness 自适应化 | 已交付 | 全绿 |
| 诊断轮（2026-11-09 下午） | mini_ws_tracker 修复 + Sintel 真实 swarm 129.3MB ×2 全成 + 三方冒烟 | 已交付 | ezr 源码零改动 |
| specifier（2026-11-09 修订轮） | phase-02 v1.16（FR-02-03/19 v1.16 + D73）+ mission.md v1.46 + features 2 份修订 + qa 2 份同步 | 已交付 | APS parser+dry-check 2/2 exit=0 |
| **coder（本轮 2026-11-09）** | FR-02-03 v1.16 做种回退臂 + FR-02-19 v1.16 磁盘删除 + D73 全量实现；BtTaskData.upload_paused 跨重启持久载体；引擎 Reconfigure 文件面三缺口修复（句柄关闭/done 失效/请求泵）+ PieceOrderCache avail 重建；bin 单测 +11 | 已交付 | bin 994P/0F + 集成 12 目标 6848P/0F + clippy 0 警告 + fmt 干净 + APS parser/dry-check 2/2 exit=0 |
| **coder 补缺口轮（2026-11-10）** | seed_left 倒计时递减 + 归零自动停止做种臂落地（09-bt-engine-13 归零臂/QA-BE-13 既有判据纯实现补齐）+ 空格停止臂共用落点 finish_seeding 提取 + bin 单测 +3 | 已交付 | bin 997P/0F + 集成 g_appcore 793P / g_engine 789P（各 +3 收编）+ 其余 10 集成目标 --no-run 编译通过 + clippy 0 警告 + fmt 干净 |

### 遗留受限项（当前有效）
- **leech 判据无 e2e 自动化**（six-pack 链长期受限项，历轮维持）。上传暂停（D73②）的 leecher 拉取面归入同族：本轮以 handle_peer_msg 闸门单测锁「request 不被服务、cum_up 零增长」可观察契约，leecher 端到端拉取验证待 QA/harness 轮。
- **变异实跑未执行**（hardender 可选节点；`--file` 分块口径悬置转政策性关闭）。
- **QA 规程 6 个 BT/命令窗口族无专属 runner**（harness/命令链路承载，历轮既定形态）。
- **CRAP 观察项**：`ezr_main` 与 `on_mouse_trackers` bin 套件 0 覆盖（承载面差异登记）。
- **ws(s) tracker announce 无代理面**——操作者 20261109 chat 裁决不补齐，长期已知边界关闭。
- **ezr-demo 守卫脚本先在 FAIL**（A2-A5/B0/M3/P10/R1-3 等，stash 基线对照证实与 ws/webrtc 轮无关）——沿袭移交 cleaner 裁决修否。
- **本轮实现缺口已收敛**（specifier 轮登记的四缺口 ①App 层回退事件缺失 ②取消勾选不删磁盘 ③上传暂停与 seed_left 重置未实现 ④files_handles 不关闭——全部落实并测试覆盖，本行仅为对账闭环记录）。
- **seed_left 倒计时无递减/归零自动停止实现**（✅ 已修复——2026-11-10 补缺口轮按操作者指令补齐：App tick 层 `tick_seed_countdown` 逐帧递减 + 归零臂与空格停止共用 `finish_seeding`；bin +3 全绿，详见 §三）。

### 实现定义值登记（累计，当前有效口径）
- **D44–D73 沿历轮登记不变**（全量清单见 phase-01/phase-02 D 值表；D71 ws/webrtc 八项、D72 bug28 五项、D73 文件勾选修订轮八项均已实现并验证）。
- **D73 实现载体登记（本轮 coder 落地，规格锁可观察面、载体归实现定义——如对载体有异议请在 cleaner/QA 闸门提出）**：
  - D73①（回退判据与时机）：App 层确认链路（`dlg_confirm_modify_bt`）内即时判 `selected_bytes_of_pieces(done_blocks) < new_total`（与引擎 `recompute_downloaded` 同源同值的 piece 记账）→ 状态即时翻「下载中」+ `downloaded = recalc`；不新增引擎事件、不等进度帧。
  - D73②（上传暂停）：单点闸门 = `handle_peer_msg` Request 臂 `allowed = !am_choking && !upload_paused`（闸门静默忽略：cum_up/uploaded/announce uploaded/行速度全零；choker 不动）；载体生命周期 = `BtTaskData.upload_paused`（serde default，跨暂停/恢复/应用重启持久）+ Reconfigure 回退臂置位 + 引擎双完成臂（block 校验路径与 tick 路径）与 App `on_bt_done` 双侧清除。
  - D73③（seed_left 重置）：`on_bt_done` 无条件 `seed_left = cfg.seed_duration_secs`（原 `if <= 0` 条件退役；与首次完成同源键，原剩余值作废）。
  - D73④⑤⑥⑦（删除细则/toast/暂停同口径/目录等价）：`delete_deselected_files` 单一实现路径（App 非运行态与引擎 Reconfigure 共用）——文件粒度 remove_file + 祖先链自底向上 remove_dir（非空即停）+ NotFound/IO 失败双静默不阻塞；toast 沿既有更新口径无新增臂；句柄先关后删（防 Linux unlink 后陈旧句柄写孤儿 inode）。
  - **配套引擎缺口（非 D73 条款、勾回补下链路必经，随本轮修复）**：①done/partial 跨 Reconfigure 持久——对「selected 且磁盘文件缺失」文件覆盖的 piece 失效（跨界 piece 整块重下，与「整 piece 下载」语义同构）；②请求泵纯反应式——files_changed 末尾对全部连接主动各泵一次；③`PieceOrderCache.avail` 为 Bitfield/Have 事件流累计值——Reconfigure 重建 pmap 改从既有连接 `st.has` 全量重建（原 `PieceOrderCache::new()` 清空 avail → rebuild 空 order → 泵空转）。

## 二、当前产出情况

### 本会话产物清单（coder 修订实现轮）
1. **`model/bt.rs`**：`BtTaskData.upload_paused` 字段（serde default，D73② 持久载体）+ `delete_deselected_files`（删除单一实现路径）+ 4 单测（删除三形态 + serde 往返）。
2. **`engine/supervisor/mod.rs`**：`BtSpec.upload_paused` 字段（引擎启动注入）。
3. **`bt/engine.rs`**：Reconfigure 文件面重写（句柄关闭 → 磁盘删除 → pmap 重建 → avail 重建 → done/partial 失效 → 回退臂置 upload_paused → 请求泵）+ 双完成臂清 upload_paused + Request 闸门；`handle_peer_msg` 增参。
4. **`app/bt_dialog.rs`**：`dlg_confirm_modify_bt` 回退翻转/重算/暂停任务删除臂 + `make_bt_spec` 载体注入；4 场景测试（10/11/12/spec 载体）。
5. **`app/engine/bt.rs`**：`on_bt_done` 无条件 seed_left 重置 + 清 upload_paused + toast 照常；1 测试。
6. **`tests/gen/detail_stripped.rs`**：BtTaskData 字面量字段同步（挂载套件编译面）。
7. **`notes/`**：architecture/testing/environment 三文件追记 + INDEX.md 索引同步。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| bin 全部单元测试 | 994P/0F（含本轮新增 11） | `cargo test --bin ezr`（CARGO_*_DEBUG=0 + CARGO_INCREMENTAL=0 三件套，nightly） |
| 集成 12 目标 | 6848P/0F | `cargo test --tests --no-run` 后逐目标 `cargo test --test <t>`（hardening 247/engine_backfill 892/ui_backfill 922/v115 915/g_appcore 790/g_engine 786/g_main 2/g_proxy 16/g_slots 254/g_supervisor 759/g_throttle_err 492/g_v147 773） |
| clippy | 0 警告 | `cargo clippy --bins --tests` |
| fmt | 干净 | `cargo fmt --check` |
| 引擎勾回补下集成 | 全绿 2.19s | `cargo test --bin ezr -- reconfigure_deselect_deletes`（真 seeder 全链：首下载→取消勾选删除+目录清理→勾回从头补下字节一致） |
| APS parser（08-modify-bt） | exit=0，12 场景 | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-parser <abs>/project/features/08-modify-bt.feature <abs>/.work/tmp/08.json)` |
| APS dry-check（08） | exit=0 | 同目录 `bb gherkin-ir-dry-checker` |
| APS parser/dry-check（09-bt-engine） | exit=0，20 场景 | 同上换文件名 |

### 对账口径
- 单测 bin：983 → 994（+11：delete_deselected ×3 + upload_paused serde ×1 + request_silenced_while_upload_paused ×1 + reconfigure_deselect_deletes 集成 ×1 + confirm_modify_seeding/paused/spec ×4 + bt_done_resets ×1）。
- 集成 12 目标：6761 → 6848（+87 与 src bin 测试挂载自动收编口径一致，无套件减员）。
- features/qa/mission：本轮零改动（specifier 修订轮产物原样，coder 只落实实现）。

### 待办与待批
- 无待批项（D73 操作者裁决已于 specifier 轮闭环；本轮实现载体登记见上，供下游否决）。
- **移交 cleaner**：新代码度量四件与保持行为清理；ezr-demo 守卫先在 FAIL 修否（沿袭）；`seed_left` 递减/归零缺口已由 2026-11-10 补缺口轮修复（§三），原「修复或登记受限」裁决项关闭。
- **移交 QA（下下游）**：QA-MB-10/11/12 与 QA-BE-08 可实跑（删除/回退/上传暂停可观察面均已落地）；上传暂停 leecher 拉取面可借 harness mini_ws_tracker/lt leecher 观察。

### 移交建议
- 下一步运行 **six-pack/cleaner**（以本文件为上游交接内容）；之后沿 six-pack 链 architect → QA。
- 操作者可直接 `bin/swarm begin six-pack/cleaner`（以本文件为上游交接内容）。

## 三、补缺口轮（2026-11-10，操作者 chat 指令：seed_left 倒计时+归零臂+QA）

### 背景
- 操作者核证 2026-11-09 遗留受限项「seed_left 倒计时无递减/归零自动停止实现」后指令补齐（含对应 QA）。规格面零改动：09-bt-engine-13 归零臂与 QA-BE-13 既有判据（toast `✓ 做种结束: <name>`、转「已完成」、上传连接清空、tracker 收 stopped）原样，纯实现缺口补齐。
- 关系澄清登记：`seed_duration_secs`（配置总时长，已实现）与 `seed_left`（任务剩余量）的衔接原只有完成时一次性赋值与 UI 显示两环——「倒计时」半句（递减与归零）全仓无实现，本轮补的正是这两环。

### 产物清单
1. **`app/engine/tick.rs`**：`tick_seed_countdown(dt)`（与 tick_retry_countdown 同款口径：逐帧 dt 累计、仅 Seeding 态递减、归零清底转臂）+ tick() 主循环接线（tick_retry_countdown 之后）；`seed_countdown_tests` 3 测（递减仅做种态 / 归零臂全可观察面 + StopSeed 命令 + 不重复触发 / 主 tick 接线回归锁）。
2. **`app/tasks.rs`**：做种停止共用落点 `finish_seeding(idx, toast)`（pub(super)）提取——空格提前停止臂（原内联实现退役）与归零臂共用：转「已完成」+ upload_speed 归零 + 释放槽位 + seed_left 清底 + spawn 发 Cmd::StopSeed（tracker stopped/上传连接清空由引擎既有停止臂承载）+ 即时 save_registry；入口差异仅 toast 文案。

### 验证证据（全部本轮实跑）
| 验证项 | 结果 | 命令 |
|---|---|---|
| bin 全部单元测试 | 997P/0F（+3） | `cargo test --bin ezr`（CARGO_INCREMENTAL=0 + CARGO_PROFILE_*_DEBUG=0，nightly 1.102.0） |
| 集成 g_appcore | 793P/0F（790+3 收编） | `cargo test --test hardening_g_appcore` |
| 集成 g_engine | 789P/0F（786+3 收编） | `cargo test --test hardening_g_engine` |
| 其余 10 集成目标 | 全部编译通过 | `cargo test --tests --no-run` |
| clippy | 0 警告 | `cargo clippy --bins --tests` |
| fmt | 干净 | `cargo fmt --check` |

### 对账口径
- bin：994 → 997（+3：seed_countdown_decrements_only_while_seeding / seed_countdown_zero_stops_seeding_to_completed / full_tick_drives_seed_countdown）。
- features/qa/mission：零改动（QA-BE-13 既有规程已完整覆盖归零臂判据，纯实现缺口）。
- notes：architecture/testing/environment 三文件追记 + INDEX.md 同步。

### 待办与移交
- 无待批项（操作者指令闭环；归零臂可观察面与 QA-BE-13 逐条对齐）。
- **移交 cleaner**：新代码度量四件与保持行为清理（沿袭）；ezr-demo 守卫 FAIL 修否（沿袭）。
- **移交 architect（下下游，跨 cleaner 轮待兑现，2026-11-10 操作者 chat 指令登记）**：提请把「规格逐 When/Then 全量对账」固化为 arch_check 第 18 条（现 17 条 → 18 条）。动机 = seed_left 纯实现缺口教训：QA-BE-13 判据在、四个局部实现点（配置键/赋值/显示/持久化）grep 全命中产生「功能已存在」假象，但「spec 场景 → 判据 → 代码落点」链条无人机械核对，行为链断在递减+归零臂——对账主体不能是被对账链条上的当事人。固化口径建议：①对账主体归 architect（轮末独立守门，非实现者自查、非 QA 元层，对账发现缺口 → 登记 handoff → 移交 coder 轮）；②落法沿 arch_check 既有 grep 级方案传统（scripts/arch_check.sh，归属 six-pack/architect 交付，历轮 17 条均由 architect 亲手增补）；③新增规则沿「阳性对照实测本轮会话」纪律生效（POSITIVE-CONTROL 尾注传统，engineering.md：零发现结论以阳性对照生效为前提）；④成本口径 = 每轮约一次 spec 逐行扫描，归 architect 轮承担。同构先例：规则 14 白名单扩展「cleaner v141 裁决移交自 coder v140」——登记移交、规则所有者裁决后亲手修订。本条为提请非指令，architect 有裁量权（不采纳须登记理由）。
- 环境备注：沙箱 2026-11-10 重置，rustup 经 rsproxy 前台重装（nightly 1.102.0 同版本带；rsproxy UPDATE_ROOT 布局漂移绕行见 environment.md），产物归档已刷新。
