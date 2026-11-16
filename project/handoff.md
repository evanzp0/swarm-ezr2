# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender（FR-02-37 v1.3 变异加固续轮二，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner/architect/QA 收敛链 ⇒ specifier 显示指标/调整/裁决/新增需求轮（v1.47–v1.53）⇒ specifier 裁决落实修订轮（v1.54）⇒ coder 产品实现/复核重验轮 ⇒ specifier PEERS/SEEDS 核证修订轮（v1.55）⇒ specifier D80 裁决落实轮（v1.56）⇒ coder FR-02-37 v1.3 产品实现轮 ⇒ cleaner FR-02-37 v1.3 收敛轮 ⇒ architect FR-02-37 v1.3 架构评审轮 ⇒ hardender FR-02-37 v1.3 变异加固轮（2026-11-15 上半）⇒ hardender 变异加固续轮（2026-11-15 下半）⇒ **hardender FR-02-37 v1.3 变异加固续轮二（本轮 2026-11-16，任务 = 交接②断点续跑：79 存活体 backfill+supervisor 双套件补跑；变异面全收敛，下一步建议 six-pack/QA）**
> 会话：six-pack-hardender-20261115（SESSION_TASK = FR-02-37-v1.3-变异加固续轮二-待办清理；上游交接 = 续轮版 handoff；会话中段沙箱重置丢失 `.work/session`，按手工六字段恢复续跑、`complete` 无损）
> 环境：nightly 1.102.0-nightly (5f4effd57 2026-11-13) rsproxy 官方双变量（沙箱重置后重建，与 handoff 基线 commit 精确一致，RUSTUP_TOOLCHAIN 注入钉扎）；度量四件在位（cargo-mutants 27.1.0 / cargo-llvm-cov 0.9.1 / PMD CPD 7.29.0 / bb 1.13.229 + APS）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.56**）+ `mission/` 四份 phase 详述（现行 phase-01 v1.53、phase-02 v1.24）。ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-14 architect 评审轮） | ezr 产品实现 + 收敛/评审链 | 已交付 | 历轮对账在案（16 目标 8223P/0F 基线） |
| hardender FR-02-37 v1.3 变异加固轮（2026-11-15 上半） | 变异实跑四文件 + tests/hardening_mutation.rs 新目标（15 测试）+ 软 Gherkin 变异全量 + CRAP/DRY 复扫 + notes 两文件追加 | 已交付 | 17 目标 9170P/0F、clippy 0、fmt 合规、arch_check 18 规则过 |
| hardender FR-02-37 v1.3 变异加固续轮（2026-11-15 下半） | trackpool ws 26 复验闭环 + 三文件 308 点变异实跑（窄杀双面）+ 79 存活体 hardening_g_engine 集成面复验 + notes 两文件追加 | 已交付 | 产品源码字节级零改动（pristine cmp）；17 目标 9170P/0F 口径沿上半轮 |
| **hardender FR-02-37 v1.3 变异加固续轮二（本轮 2026-11-16）** | **79 存活体 backfill+supervisor 双套件补跑全量完成（交接②唯一未竟项结案）+ 三文件 339 全池自适应混合流结案（零漂移）+ pristine 零改动证明 + notes 两文件追加 + 本 handoff 重写** | 已交付 | 产品源码与 tests/ 字节级零改动（3 树 × 137 文件 cmp 全一致）；测试目标零变化（17 目标 9170P/0F 口径不变，树字节级同一） |

### 遗留受限项（当前有效）
- **变异实跑总账（两轮 + 续轮二，全收敛）**：全清单 494 突变体（上半轮四文件 186 + 三文件 308）处置闭环、零未跑；续轮二把三文件全池 339（= 308 + trackpool ws 面 31）以自适应混合流重推导/补证结案——**登记函数 235 全部实跑处置：杀 149 + unviable 6（真编译失败）+ 超时杀 1（pieceio_072，caught-by-timeout）+ 存活 79；非登记函数 84 沿上轮杀灭账免跑；trackpool ws 面 31 = 20 定理证明（本轮 20/20 全 span 未覆盖复验通过）+ 11 非登记函数免跑**。逐突变体判定权威 = `adaptive_resolved.csv` 最新行（406 行/339 id），证据底座 = unit 212 日志 + bs 24 日志（运行时产物，重置后按交接②再生）。
- **79 存活体 = 覆盖缺口确认（非等价），backfill+supervisor 双套件零杀灭证据已补齐（续轮二兑现）**：`results/bs_runs.csv` 23 批次全部 exit=0、恰覆盖 79 id（批内 logs/bs_batch_*.log 在案）；单位面（bt::engine 73 测）重推导 56 条存活 + 23 条沿上轮身份直接 BS 实跑结案。判定链与上轮 ws 面 26 同构；非等价定性依据 = 行为面与 CRAP 热点自洽（trackpool live_snapshot L276 cov 0.086 热点、pieceio 请求调度/位域/上传读面、peerwire handle_peer_msg 协议分支面）。函数粒度登记（回补路径 = 按函数名锚定新增精确断言测试）：
  - trackpool 9：spawn_tracker@L95 1 + live_snapshot@L276 8（LiveStats 三元组快照，81 种常量替换全存活——快照消费面无确定性观察点，回补触发 = live_snapshot 暴露测试口或 conn_views 断言增量）；
  - pieceio 45：issue_requests 5、request_blocks_of_piece 7、apply_choker 4、bitfield_bytes 10、partial_piece_list 4、inflight_piece_list 4、restore_sidecar_partial 3、on_block 2、upload_read_plan 2、verify_piece_from_disk 1（L578 整函数替 true）、save_bt_sidecar 1、open_selected_file_handle 1、fill_peer_slots 1；
  - peerwire 25：handle_peer_msg 22、run_peer_loop 1、count_invalid_request 1、assemble_contiguous 1；
  - 本轮函数级对账与上轮登记**逐一精确一致、全程零漂移**（漂移守卫 `adaptive.py check` 每轮次校验）；逐突变体身份以本轮 CSV 为权威（上轮逐条 CSV 随沙箱重置丢失，无法逐条比对——沿 plan 既登记口径）。
- **软 Gherkin 验收变异执行层不可用（受限项，沿历轮登记不变）**：1123 突变 errors=1123（执行层缺失的机械化证明）；回补路径 = specifier/QA 轮构建 IR 驱动验收入口后适配器接入。
- **QA runner fixture 漂移 3 处（移交 QA/specifier 轮，沿历轮登记不变）**：① `suite_modify_bt.py` 4F；② `suite_config_template.py` 5F；③ `suite_persistence_config.py` QA-PC-04 1F。
- **verify_trackers_popup.py T2/T3 共 3F（既有失败）+ verify_size_move.py 陈旧化**：沿历轮登记不变。
- **CRAP 登记口径（17 目标重扫口径沿袭不变）**：行覆盖 94.3%；ACTIONABLE 102；②类引擎异步环全部热点维持 ACTIONABLE——续轮二变异证据（双套件零杀灭 79/79）与既有三分类判定相容，未推翻任何裁决。79 缺口体的函数面与 ACTIONABLE 热点映射：live_snapshot@L276（trackpool.rs:412 CRAP 186.96）、handle_peer_msg/assemble_contiguous（peerwire.rs:251/308）、issue_requests/request_blocks_of_piece（pieceio 请求调度面）。
- **mktorrent bencode 跨 bin 重复**：维持登记、lib 化不实施（architect 既定裁决沿袭）；DRY 19 组 83-133 tok 克隆清单沿移交。
- **leech 专属 e2e、QA runner 族缺口**：沿历轮登记不变。
- **上半轮 186 与分类和 ±3 登记差（single.rs 16/18 计数歧义）**：逐文件 CSV 已按运行时口径清理无法重审，沿旧账如实携带、不影响累计口径。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退）；D80①–④ 已全部结案**——本轮续轮二无规格语义改动、无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（hardender FR-02-37 v1.3 变异加固续轮二）
1. **79 存活体 backfill+supervisor 双套件补跑（交接②唯一未竟项，结案）**：`cargo test --test hardening_engine_backfill --test hardening_g_supervisor` 组批实跑（bs_run.sh，批内冲突检查 ±4 行 span），23 批次全部 exit=0、恰覆盖 79 id——**79/79 双套件零杀灭，全部维持覆盖缺口登记**；批均值 ~264s，账目与日志（logs/bs_batch_*.log 24 份）在案。
2. **三文件 339 全池自适应混合流结案**：pool 状态机（POOL→单测面重推导→DEAD/BSQ→BS 批次→DONE）+ 漂移守卫逐轮校验，最终 POOL/BSQ 清零、79/79 识别与登记一致零漂移；单测面重推导 212 突变体（unit_runs.csv 227 行 + 212 日志）。
3. **pristine 逐文件 cmp 零改动证明**：真实树 project/ezr（worker 零接触）+ scratch + scratch2 双工作副本，各 137 文件（src/ + Cargo.toml + tests/）cmp 全一致。
4. **经验沉淀**：`project/notes/testing.md`、`environment.md` 各追加续轮二条目（runner 简化 verdict 假分类与账目改判 / 直接 BS 结案账目口径 / 调用间后台进程回收第 2 例 / .work/session 丢失禁 begin 重建的手工恢复 / timeout 贴边判据细化），INDEX.md 同步。
5. **本 handoff.md** 重写为本轮口径（交接四要素见下）。

### 交接四要素（hardender SKILL 交接契约）
1. **本地验证命令清单（按序）**（产品树与 tests/ 零改动，口径沿上半轮证据，同一工作区树）：
   ```sh
   cd project/ezr && export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 PATH=$HOME/.cargo/bin:$PATH
   cargo fmt --all -- --check
   cargo clippy --all-targets          # 0 警告
   cargo test --bins                   # 1098P/0F
   cargo test --test hardening --test hardening_engine_backfill --test hardening_g_appcore --test hardening_g_engine      # 2825P
   cargo test --test hardening_g_main --test hardening_g_proxy --test hardening_g_slots --test hardening_g_supervisor    # 1063P
   cargo test --test hardening_g_throttle_err --test hardening_g_v147 --test hardening_ui_backfill --test hardening_v115  # 3237P
   cargo test --test hardening_mutation   # 947P/0F（加固套件入口，含 15 加固测试）
   cargo test property_tests::            # 88P/0F（独立口径）
   bash scripts/arch_check.sh             # 18 规则
   ```
   合计 **9170P/0F（17 目标）**。600s 调用窗口下集成目标按批拆分（每批 ≤4 目标）。
2. **会话运行时产物位置与复现方法**（均不进工作区产物、不进归档；沙箱存活期内可续查，重置后按命令再生）：
   - 自适应混合流会话：`.work/tmp/mrunner/`——`pool.json`（339 全池状态机）+ `results/adaptive_resolved.csv`（判定权威，406 行）+ `results/unit_runs.csv`（227 行）+ `results/bs_runs.csv`（23 批次）+ `logs/unit_*.log`（212）+ `logs/bs_*.log`（24）+ `mutants/<stem>/*.diff` + `adaptive.py`（next/record/fail_split/check 四命令）+ `loop_drive.sh`（预算控窗驱动）+ `bs_run.sh`（BS 批次）+ `theorem.py`（REG 登记 79 + 覆盖率定理）。
   - 全池再生：`cargo mutants --list --diff --file src/bt/engine/<stem>.rs`（三文件）→ `parse_list_diff.py` 生成 mutants/ 清单与补丁。
   - BS 批次复跑：`cd .work/tmp/mrunner && ./bs_run.sh batch <N>`（batches.json 分批）或 `./bs_run.sh single <id>`；MUT 窗口纪律（timeout 480 ≥ 双套件裸 ~253s × 1.5）沿 testing.md。
3. **feature 注释块与差分复用注意事项**：沿历轮口径不变（20 feature 头部 manifest 注释块；强制全量重跑 = 删除注释块）。
4. **等价突变体清单及失效条件（回补触发）**：
   - **等价类（3，bt_dialog，沿历轮）**：m077（merge_push 的 ignored 去重臂——失效条件：ignored 的消费方扩展为全量展示/计数/落盘）；m097/m098（proxy_sel 边界防御钳制——失效条件：新增越界调用方或选项表对话框存续期收缩）。
   - **覆盖缺口（112，非等价——不得当等价使用；79 的双套件零杀灭证据已补齐）**：
     - 上半轮 btmod 7（m001-m004 LEECH_* 私有常量钉扎、m009/m011 LiveStats 初值 resume 回填、m013 got_peers 时序）——失效条件沿上半轮登记；
     - 上半轮 trackpool ws 26——行为面 ws_prepare_offers/ws_tracker_session/ws_handle_offer/ws_process_response，本轮 ws 面 20 条定理复验（20/20 全 span 未覆盖）佐证覆盖缺口定性，回补触发 = ws 信令面精确断言测试（announce offer/answer/response 编解码与分派）；
     - 三文件 79（函数粒度见遗留受限项；逐突变体判定 = adaptive_resolved.csv）——共同回补路径 = 按函数名锚定新增精确断言测试（bitfield_bytes/partial_piece_list/inflight_piece_list 等纯函数面最低成本：直调断言；handle_peer_msg/issue_requests 等行为面沿「真实事件路径覆盖法」）；**失效条件（通用）**：任何下游轮为这些函数新增直接断言测试即触发回补，届时存活体应被新测试杀灭或升级为等价论证。

### 验证证据
| 验证项 | 结果 | 命令/证据（可复现） |
|---|---|---|
| 79 存活体双套件补跑 | **23 批次 exit=0、79/79 零杀灭** | results/bs_runs.csv + logs/bs_batch_*.log |
| 339 全池结案 | POOL/BSQ 清零，判定权威 resolved 最新行 | python3 .work/tmp/mrunner/adaptive.py check（对账 79/79 无漂移） |
| 单测面重推导 | 212 突变体：杀 149 + unviable 6 + 超时杀 1 + 存活 56 | results/unit_runs.csv + logs/unit_*.log |
| 漂移守卫 | 全程零漂移（识别存活 79/79 = REG） | adaptive.py check 输出（各轮次在 meta/loop 日志） |
| ws 面 20 定理复验 | 20/20 全 span 未覆盖（BS 面 lcov） | theorem.py + bs_face.lcov 复核脚本 |
| 零改动证明 | **真实树 + 双 scratch 各 137 文件 cmp 全一致** | pristine cmp（.work/tmp/pristine） |
| 17 目标基线 | 9170P/0F（口径沿上半轮，树字节级同一） | 交接①命令清单 |

### 对账口径
- 本轮改动面 = notes 两文件 + INDEX.md + 本 handoff；**产品源码（src/ 全部 + Cargo.toml）与 tests/ 字节级零改动**，产品行为面 = 零；测试净变化 = 0（17 目标 9170P 口径不变）。
- **339 全池复算**：登记函数 235（= 杀 149 + unviable 6 + 超时杀 1 + 存活 79）+ 非登记函数 84（免跑沿上轮杀灭账）+ ws 面 31（20 定理证明 + 11 免跑）= 339 ✓；79 存活 = 56（单测面重推导）+ 23（直接 BS 实跑）= 79 ✓；BS 批次 23 个 exit=0 恰覆盖 79 id ✓；identified-alive 79/79 = REG 79 ✓。
- 与上轮口径的衔接：上轮 308（杀 218 + 超时杀 2 + unviable 9 + 存活 79）与本轮 339 重推导在函数级登记上精确一致；逐突变体身份以本轮 CSV 为权威（上轮 CSV 随沙箱重置丢失，沿既登记口径如实携带）。pieceio_072 两轮同为超时杀（caught-by-timeout，日志挂起签名一致）；pieceio_043 本轮免跑（非登记函数，沿上轮超时杀账）。
- 会话中断韧性：沙箱重置丢 `.work/session`（手工六字段恢复）+ 调用间后台进程回收（前台分段 15 段续跑）均无损，结果集零污染（pool 状态机 + resolved 最新行口径）。

### 待办与待批
- **无待批项**（本轮无规格语义改动、无新增 D 值、无操作者裁决请求）。
- 待办（移交后续轮）：① ~~79 存活体 backfill+supervisor 补跑~~ **已结案（本轮兑现）**；② 缺口 112 的回补测试（specifier/QA 轮按交接④回补路径裁决，hardender 不自建——上半轮 15 加固测试模式可复用）；③ 软 Gherkin 执行层回补（IR 驱动验收入口）移交 specifier/QA 轮裁决；④ QA runner 漂移 3 处与 verify 脚本 2 项沿历轮移交 QA/specifier 轮。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- **变异加固面已全收敛**（494 突变体处置闭环 + 339 全池续验零漂移 + 79 双套件证据补齐），下一节点建议 = `bin/swarm begin six-pack/QA`（终验链：QA runner 漂移 3 处与本轮登记一并处置；变异加固证据可直接支撑 QA 终验的测试充分性论证）。
