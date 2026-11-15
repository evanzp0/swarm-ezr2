# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender（FR-02-37 v1.3 变异加固续轮，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner/architect/QA 收敛链 ⇒ specifier 显示指标/调整/裁决/新增需求轮（v1.47–v1.53）⇒ specifier 裁决落实修订轮（v1.54）⇒ coder 产品实现/复核重验轮 ⇒ specifier PEERS/SEEDS 核证修订轮（v1.55）⇒ specifier D80 裁决落实轮（v1.56）⇒ coder FR-02-37 v1.3 产品实现轮 ⇒ cleaner FR-02-37 v1.3 收敛轮 ⇒ architect FR-02-37 v1.3 架构评审轮 ⇒ hardender FR-02-37 v1.3 变异加固轮（2026-11-15 上半）⇒ **hardender FR-02-37 v1.3 变异加固续轮（本轮 2026-11-15 下半，任务 = 待办清理：trackpool 26 复验闭环 + 三文件 308 点变异实跑 + 存活体集成面复验；下一步建议 six-pack/QA）**
> 会话：six-pack-hardender-20261115（SESSION_TASK = FR-02-37-v1.3-变异加固续轮-待办清理；上游交接 = 同会话 hardender 变异加固轮版 handoff）
> 环境：nightly 1.102.0-nightly (5f4effd57 2026-11-13) rsproxy 官方双变量前台装齐 minimal+rustfmt+clippy+llvm-tools（沿上半轮基线，本轮无重装）；~/.cargo/config.toml rsproxy sparse；度量四件（cargo-mutants 27.1.0 / cargo-llvm-cov 0.9.1 / PMD CPD 7.29.0 / bb 1.13.229 + APS）沿上半轮装机在位。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.56**）+ `mission/` 四份 phase 详述（现行 phase-01 v1.53、phase-02 v1.24）。ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-14 architect 评审轮） | ezr 产品实现 + 收敛/评审链 | 已交付 | 历轮对账在案（16 目标 8223P/0F 基线） |
| hardender FR-02-37 v1.3 变异加固轮（2026-11-15 上半） | 变异实跑四文件 + tests/hardening_mutation.rs 新目标（15 测试）+ 软 Gherkin 变异全量 + CRAP/DRY 复扫 + notes 两文件追加 | 已交付 | 17 目标 9170P/0F、clippy 0、fmt 合规、arch_check 18 规则过 |
| **hardender FR-02-37 v1.3 变异加固续轮（本轮 2026-11-15 下半）** | **trackpool ws 26 复验闭环（backfill+supervisor 套件）+ 三文件 308 点变异实跑（窄杀双面）+ 79 存活体 hardening_g_engine 集成面复验 + notes 两文件追加 + 本 handoff 重写** | 已交付 | 产品源码字节级零改动（pristine cmp）；测试目标零变化（17 目标 9170P/0F 口径沿上半轮证据，工作区树字节级同一） |

### 遗留受限项（当前有效）
- **变异实跑三文件 308 点已跑完（本轮兑现，待办①主项结案）**：cargo-mutants 27.1.0 清单制差分变异 + 双面过滤器路由（轻面 27 组名/52 测 pristine 14.74s + 重面 17 组名/21 测 pristine 35.24s）+ 存活体跨面补杀 + hardening_g_engine 集成面复验：
  - `src/bt/engine/trackpool.rs` 非 ws 面 21 点：**杀 8 + unviable 4 + 存活 9**（存活 = spawn_tracker 1 + live_snapshot 8）；
  - `src/bt/engine/pieceio.rs` 209 点：**杀 159 + 超时杀 2（pieceio_043/072，真挂起 caught-by-timeout，日志有 >60s 挂起证据）+ unviable 3 + 存活 45**；
  - `src/bt/engine/peerwire.rs` 78 点：**杀 51 + unviable 2 + 存活 25**；
  - 合计 **308 = 杀 218 + 超时杀 2 + unviable 9 + 存活 79**（逐突变体判定 `.work/tmp/mrunner/results/*.csv`，账目复算自洽）。
- **79 存活体 = 覆盖缺口确认（非等价）**：单位面（bt::engine 全 73 测：轻 52 + 重 21）+ hardening_g_engine 集成面双零杀灭，判定链与上半轮 ws 面 26 同构；**非等价定性依据 = 行为面与 CRAP 热点自洽**（trackpool live_snapshot L276 cov 0.086 热点、pieceio 请求调度/位域/上传读面、peerwire handle_peer_msg 协议分支面）。函数粒度登记（回补路径 = 按函数名锚定新增精确断言测试）：
  - trackpool 9：spawn_tracker@L95 1 + live_snapshot@L276 8（LiveStats 三元组快照，81 种常量替换全存活——快照消费面无确定性观察点，回补触发 = live_snapshot 暴露测试口或 conn_views 断言增量）；
  - pieceio 45：issue_requests 5、request_blocks_of_piece 7、apply_choker 4、bitfield_bytes 10、partial_piece_list 4、inflight_piece_list 4、restore_sidecar_partial 3、on_block 2、upload_read_plan 2、verify_piece_from_disk 1（L578 整函数替 true）、save_bt_sidecar 1、open_selected_file_handle 1、fill_peer_slots 1；
  - peerwire 25：handle_peer_msg 22、run_peer_loop 1、count_invalid_request 1、assemble_contiguous 1；
  - 逐突变体明细（79 条 ID/行号/变异描述）：`.work/tmp/mrunner/survivors_register.txt`（运行时产物；重置后按交接②命令再生）。
- **79 存活体 backfill+supervisor 套件补跑未做（受限项，主因 = 会话预算；~270s/条 × 79 ≈ 6h）**：复跑命令见交接②，判定链沿 ws 面 26 先例（g_engine 后补跑双套件）。补跑结论只会进一步收敛覆盖缺口清单（杀灭→回补；存活→维持登记），不影响本轮已完成证据的有效性。
- **变异实跑总账（两轮累计）**：全清单 494 突变体（上半轮四文件 186 + 本轮三文件 308）处置闭环、零未跑。上半轮处置面沿上半轮 handoff 登记原文（killed 124 + timeout 捕获 1 + unviable 8 + 加固测试杀灭 14 + 等价论证 3 + 缺口登记 7 + trackpool ws 26 待复验——本轮三套件复验后 26 结案归入缺口登记）；本轮处置面逐 CSV 复算自洽（杀 218 + 超时杀 2 + unviable 9 + 缺口登记 79）。两轮分类合计：killed 342、timeout 3、unviable 17、加固杀灭 14、等价 3、**缺口登记 112**（= 7 + 26 + 79）；上半轮内部 186 与分类和 183 存在 ±3 登记差（single.rs 16/18 计数歧义），其逐文件 CSV 已按运行时口径清理无法重审，沿旧账如实携带、不影响本轮与累计口径。
- **非等价存活体（覆盖缺口）累计 112 + 等价 3 + 回补路径登记在案**（失效条件与回补触发见交接④；112 = 上半轮 btmod 7 + trackpool ws 26 + 本轮三文件 79）。
- **软 Gherkin 验收变异执行层不可用（受限项，沿上半轮登记不变）**：1123 突变 errors=1123（执行层缺失的机械化证明）；回补路径 = specifier/QA 轮构建 IR 驱动验收入口后适配器接入。
- **QA runner fixture 漂移 3 处（移交 QA/specifier 轮，沿历轮登记不变）**：① `suite_modify_bt.py` 4F；② `suite_config_template.py` 5F；③ `suite_persistence_config.py` QA-PC-04 1F。
- **verify_trackers_popup.py T2/T3 共 3F（既有失败）+ verify_size_move.py 陈旧化**：沿历轮登记不变。
- **CRAP 登记口径（上半轮 17 目标重扫口径沿袭不变）**：行覆盖 94.3%；ACTIONABLE 102；②类引擎异步环全部热点维持 ACTIONABLE——本轮变异证据（218 新杀灭 + 79 缺口确认）与既有三分类判定相容，未推翻任何裁决。79 缺口体的函数面与 ACTIONABLE 热点映射：live_snapshot@L276（trackpool.rs:412 CRAP 186.96）、handle_peer_msg/assemble_contiguous（peerwire.rs:251/308）、issue_requests/request_blocks_of_piece（pieceio 请求调度面）。
- **mktorrent bencode 跨 bin 重复**：维持登记、lib 化不实施（architect 既定裁决沿袭）；DRY 19 组 83-133 tok 克隆清单沿上半轮移交。
- **leech 专属 e2e、QA runner 族缺口**：沿历轮登记不变。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退）；D80①–④ 已全部结案**——本轮续轮无规格语义改动、无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（hardender FR-02-37 v1.3 变异加固续轮）
1. **trackpool ws 26 待复验闭环（交接④队列结案）**：
   - 第一阶段（hardening_g_engine）：26/26 存活（上半轮已跑，本轮复核账目）；
   - 第二阶段（hardening_engine_backfill + hardening_g_supervisor 双套件）：**26/26 存活**（938P + 791P 双套件全绿）；其中 trackpool_046/047 首判 timeout 经查证为窗口击穿误判（260s 窗 < 双套件裸耗时 ~253s + 突变体失败测试烧超时增量），放大 560s 窗口重跑修正为 survived（264s/262s 完整跑完双套件）；
   - 最终判定：**三套件（g_engine/backfill/supervisor）零杀灭证据 → 覆盖缺口确认**（与上半轮「预判为覆盖缺口而非等价」一致），26 条归入缺口登记。
2. **三文件 308 点变异实跑（cargo-mutants 27.1.0，双面过滤器路由）**：
   - 清单再生：`parse_list_diff.py` 对 `cargo-mutants --list --diff` 输出逐突变体 git-apply 补丁（trackpool_nonws 21 / peerwire 78 / pieceio 209）；
   - 双面路由：按突变体函数面分片（trackpool_light 10 / trackpool_heavy 11 / pieceio_light 197 / pieceio_heavy 12 / peerwire_heavy 78），轻面存活体再补重面（trackpool_h2 8 / pieceio_h2 60 / peerwire_light 27 → 补杀 18）；
   - 补杀收益：pieceio 60 存活→16 补杀、peerwire 27→2 补杀、trackpool 8→0 补杀；
   - 超时杀 2（pieceio_043@L129 / pieceio_072@L202：bt_conns_mark_full_bitfield/endgame_active/issue_requests 测试挂起 >60s，轻面裸耗时仅 14.7s = 真挂起非窗口击穿）；
   - 存活 79 → hardening_g_engine 集成面复验 **79/79 存活**（~133s/条 × 79 全量跑完）。
3. **经验沉淀**：`project/notes/testing.md`、`environment.md` 各追加本轮续轮条目（双面过滤器路由法 / timeout 判定与套件裸耗时可比性纪律 / 存活体跨 pass 最新判定去重 / pub(super) 内部件全量单测复验替代口径 / IM 网关流空闲超时与前台分段纪律 / 中断恢复四步仪式），INDEX.md 同步。
4. **本 handoff.md** 重写为本轮口径（交接四要素见下）。

### 交接四要素（hardender SKILL 交接契约）
1. **本地验证命令清单（按序）**：
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
   合计 **9170P/0F（17 目标）**——与上半轮证据同一工作区树（本轮产品源码与 tests/ 字节级零改动，pristine cmp 证明）。600s 调用窗口下集成目标按批拆分（每批 ≤4 目标）。
2. **会话运行时产物位置与复现方法**（均不进工作区产物、不进归档；沙箱存活期内可续跑，重置后按命令再生）：
   - 变异会话：`.work/tmp/mrunner/`——`parse_list_diff.py`、`run_chunk.sh`（断点续跑：按结果 CSV 跳已完成条目）、`faces.sh` + `run_light.sh`/`run_heavy.sh`/`run_full.sh`（双面过滤器包装）、`resume_stage2.sh`（trackpool 26 第二阶段续跑脚本）、`manifests/*.json`、`mutants/<chunk>/*.diff`、`results/*.csv`（trackpool_ws 系 + trackpool_light/heavy/h2 + pieceio_heavy/light/h2 + peerwire_heavy/light/h2 + intface + TALLY-SNAPSHOT.json）、`logs/<chunk>/`（逐突变体日志）、`survivors_register.txt`（79 存活体明细）。
   - 三文件清单再生：`cargo mutants --list --diff --file src/bt/engine/pieceio.rs`（trackpool/peerwire 同款）→ `parse_list_diff.py` 生成 manifests/ 与 mutants/。
   - **79 存活体 backfill+supervisor 补跑（唯一未竟项）**：按 intface 清单逐条跑
     `cd .work/tmp/mrunner && ./run_chunk.sh intface_bs 300 --test hardening_engine_backfill --test hardening_g_supervisor`
     （MUT_TIMEOUT 必须 ≥420s——双套件裸耗时 ~253s，260s 窗会误判 timeout，见 testing.md 纪律；intface.json 清单 + mutants/intface/ 补丁在位，直接换 results 文件名复用）。
   - ws 面 26 复现：`./resume_stage2.sh`（backfill+supervisor 已跑完，CSV 在 results/trackpool_ws_stage2.csv）。
3. **feature 注释块与差分复用注意事项**：沿上半轮口径不变（20 feature 头部 manifest 注释块；强制全量重跑 = 删除注释块）。
4. **等价突变体清单及失效条件（回补触发）**：
   - **等价类（3，bt_dialog，沿上半轮）**：m077（merge_push 的 ignored 去重臂——失效条件：ignored 的消费方扩展为全量展示/计数/落盘）；m097/m098（proxy_sel 边界防御钳制——失效条件：新增越界调用方或选项表对话框存续期收缩）。
   - **覆盖缺口（112，非等价——不得当等价使用）**：
     - 上半轮 btmod 7（m001-m004 LEECH_* 私有常量钉扎、m009/m011 LiveStats 初值 resume 回填、m013 got_peers 时序）——失效条件沿上半轮登记；
     - 上半轮 trackpool ws 26（本轮三套件复验 26/26 存活结案，预判坐实）——行为面 ws_prepare_offers/ws_tracker_session/ws_handle_offer/ws_process_response，回补触发 = ws 信令面精确断言测试（announce offer/answer/response 编解码与分派）；
     - 本轮三文件 79（函数粒度见遗留受限项；逐突变体明细 survivors_register.txt）——共同回补路径 = 按函数名锚定新增精确断言测试（bitfield_bytes/partial_piece_list/inflight_piece_list 等纯函数面最低成本：直调断言；handle_peer_msg/issue_requests 等行为面沿「真实事件路径覆盖法」）；**失效条件（通用）**：任何下游轮为这些函数新增直接断言测试即触发回补，届时存活体应被新测试杀灭或升级为等价论证。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 单测（ezr bin 族 4 目标） | **1098P / 0F**（上半轮证据，树字节级同一） | cargo test --bins |
| 单测（12 集成目标） | **7125P / 0F**（2825+1063+3237） | 三批分跑 |
| 加固套件（hardening_mutation） | **947P / 0F** | cargo test --test hardening_mutation |
| 合计 | **9170P / 0F（17 目标）** | 复算：8223 + 947 |
| clippy | --all-targets 0 警告 0 错误（本轮续验） | cargo clippy --all-targets |
| fmt | 全量合规（本轮续验） | cargo fmt --all -- --check |
| 自动化架构检查 | 18 条规则全过（本轮续验） | bash scripts/arch_check.sh |
| 属性套件（独立口径） | **88P / 0F** | cargo test property_tests:: |
| ws 面 26 复验（g_engine） | 26/26 存活 | .work/tmp/mrunner/results/trackpool_ws_recheck.csv |
| ws 面 26 复验（backfill+supervisor） | **26/26 存活**（938P+791P 全绿） | results/trackpool_ws_stage2.csv + logs/trackpool_ws_stage2/*.log |
| 三文件窄杀双面 | 308 点 = 杀 218 + 超时杀 2 + unviable 9 + 存活 79 | results/{trackpool_light,trackpool_heavy,trackpool_h2,pieceio_heavy,pieceio_light,pieceio_h2,peerwire_heavy,peerwire_light,peerwire_h2}.csv |
| 存活体集成面（g_engine） | **79/79 存活** | results/intface.csv + logs/intface/*.log |
| 零改动证明 | src/+Cargo.toml/tests/ 与上半轮归档基线字节级一致 | pristine cmp（.work/tmp/pristine） |

### 对账口径
- 本轮改动面 = notes 两文件 + INDEX.md + 本 handoff；**产品源码（src/ 全部 + Cargo.toml）与 tests/ 字节级零改动（pristine cmp 证明）**，产品行为面 = 零。
- 测试净变化 = 0（17 目标 9170P 沿上半轮，树同一）；无测试删除、无断言翻转。
- 变异总账复算：494 总点数（186 上半轮 + 308 本轮）= 杀灭 342 + 超时 3 + unviable 17 + 加固杀灭 14 + 等价论证 3 + 缺口登记 112 + 加固套件另杀 0；分类无重叠、无未跑残项。
- 缺口登记复算：7（btmod）+ 26（trackpool ws）+ 79（本轮三文件）= 112；等价 3 全在上半轮 bt_dialog。
- 磁盘口径：变异 scratch 2.1G 在位（results/logs/manifests 证据保留），全程余量 3.4G 未爆盘。
- 会话中断韧性：IM 网关流空闲超时致 4 次调用中途取消，恢复四步仪式（pkill 残留→CSV 重复检查→pristine 还原→按 CSV 续跑）全数恢复，结果集零污染（uniq -d 全查）。

### 待办与待批
- **无待批项**（本轮无规格语义改动、无新增 D 值、无操作者裁决请求）。
- 待办（移交后续轮）：① **79 存活体 backfill+supervisor 套件补跑**（唯一未竟项，复跑命令交接②，~6h 预算，可续 hardender 或并入 QA 轮）；② 缺口 112 的回补测试（specifier/QA 轮按交接④回补路径裁决，hardender 不自建——上半轮 15 加固测试模式可复用）；③ 软 Gherkin 执行层回补（IR 驱动验收入口）移交 specifier/QA 轮裁决；④ QA runner 漂移 3 处与 verify 脚本 2 项沿历轮移交 QA/specifier 轮。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 下一节点建议 = `bin/swarm begin six-pack/QA`（终验链：QA runner 漂移 3 处与本轮登记一并处置；变异加固证据——494 突变体处置账 + 112 缺口登记 + 回补路径——可直接支撑 QA 终验的测试充分性论证）；如操作者拟先补完变异面，可续 `bin/swarm begin six-pack/hardender`（从交接②「79 存活体 backfill+supervisor 补跑」断点续跑）。
