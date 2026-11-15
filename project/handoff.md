# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender（FR-02-37 v1.3 变异加固轮，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner/architect/QA 收敛链 ⇒ specifier 显示指标/调整/裁决/新增需求轮（v1.47–v1.53）⇒ specifier 裁决落实修订轮（v1.54）⇒ coder 产品实现/复核重验轮 ⇒ specifier PEERS/SEEDS 核证修订轮（v1.55）⇒ specifier D80 裁决落实轮（v1.56）⇒ coder FR-02-37 v1.3 产品实现轮 ⇒ cleaner FR-02-37 v1.3 收敛轮 ⇒ architect FR-02-37 v1.3 架构评审轮 ⇒ **hardender FR-02-37 v1.3 变异加固轮（本轮 2026-11-15，任务 = 变异实跑兑现 cleaner/architect 登记面 + 最终验证序列；下一步建议 six-pack/QA）**
> 会话：six-pack-hardender-20261115（任务 = FR-02-37-v1.3-变异加固轮；上游交接 = project/handoff.md architect 评审轮版）
> 环境：nightly 1.102.0-nightly (5f4effd57 2026-11-13) rsproxy 官方双变量前台装齐 minimal+rustfmt+clippy+llvm-tools（与上游基线同版本带 1.102.0，commit 漂移一晚 8eb778d04→5f4effd57——日期钉版清单全镜像 404 含官方，无法按 commit 精确复装；沿 2026-11-09 文件勾选轮版本带漂移判例，clippy/fmt 全绿判基线无漂移，如实登记）；~/.cargo/config.toml rsproxy sparse 新配。**度量四件本轮全部装机**（沿 architect 轮受限项收窄判例的欠账兑现）：cargo-mutants 27.1.0 + cargo-llvm-cov 0.9.1（gh-proxy 预编译 → ~/.local/bin）、PMD CPD 7.29.0（.tools/pmd，java 21）、bb 1.13.229 + APS 仓库克隆（.tools/Acceptance-Pipeline-Specification，parser/mutator 契约探针过）；装机后最小探针独立验证（llvm-cov 曾因安装链中断静默缺失，cargo 子命令报 no such command 才暴露）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.56**）+ `mission/` 四份 phase 详述（现行 phase-01 v1.53、phase-02 v1.24）。ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-14 architect 评审轮） | ezr 产品实现 + 收敛/评审链 | 已交付 | 历轮对账在案（16 目标 8223P/0F 基线） |
| **hardender FR-02-37 v1.3 变异加固轮（本轮 2026-11-15）** | **变异实跑四文件 + tests/hardening_mutation.rs 新目标（15 测试）+ 软 Gherkin 变异全量（适配器）+ CRAP/DRY 复扫 + notes 两文件追加 + 本 handoff 重写** | 已交付 | 17 目标 **9170P/0F**（= 基线 8223 + 新目标 947，复算自洽）、clippy 0、fmt 合规、arch_check 18 规则过；产品源码字节级零改动（pristine cmp） |

### 遗留受限项（当前有效）
- **变异实跑部分完成（受限项，主因 = 单 crate 两核沙箱吞吐 ~50-110s/突变体 + 会话预算）**：
  - ✅ 已闭环：`src/engine/supervisor/single.rs` 16/16、`src/app/bt_dialog.rs` 119/119（--file 登记面兑现）、`src/bt/engine/mod.rs` 20/20；
  - ◐ 首轮完成待复验：`src/bt/engine/trackpool.rs` ws_* 登记面 31/31 首轮（3 杀 + 2 unviable + 26 待复验，复验命令见交接②）；
  - ✗ 未跑：trackpool 非 ws 21 点、`peerwire.rs` 78 点、`pieceio.rs` 209 点（复跑命令见交接②，preflight：pristine 备份已在 .work/tmp/pristine）。
- **非等价存活体（覆盖缺口，共 7 个已处置 + 26 个待复验）**——失效条件与回补触发见交接④：
  - btmod 7 个：m001-m004（LEECH_* 私有常量钉扎不可达）、m009/m011（LiveStats 初值 resume 回填无确定性观察点）、m013（got_peers 时序语义）；
  - trackpool ws 面 26 个（首轮窄过滤存活，未经集成套件复验，杀灭/等价判定悬置）。
- **软 Gherkin 验收变异执行层不可用（受限项）**：gherkin-mutator 已装机且 20 feature / 1123 突变全量执行（--level soft），但项目无 IR 驱动验收测试入口（验收入口生成器/验收运行时/项目步骤处理器从未构建；QA PTY 套件属 specifier 端到端契约，hardender SKILL 明令不实现/不运行/不维护）——适配器按协议如实报 errors=1123，即执行层缺失的机械化证明；2026-10-06 旧 manifest（persistence-config）证明历史上曾有验收入口，后随 QA 套件路线退役。回补路径：specifier/QA 轮构建 IR 驱动验收入口后，适配器 `execute_mutated_ir()` 接入真实入口即可复用。
- **QA runner fixture 漂移 3 处（移交 QA/specifier 轮，沿历轮登记不变）**：① `suite_modify_bt.py` 4F；② `suite_config_template.py` 5F；③ `suite_persistence_config.py` QA-PC-04 1F。
- **verify_trackers_popup.py T2/T3 共 3F（既有失败）+ verify_size_move.py 陈旧化**：沿历轮登记不变。
- **CRAP 登记口径更新（hardender 17 目标重扫）**：行覆盖 94.3%（39859/42279）；ACTIONABLE（CRAP>6 且 cov<1.0）**158 → 102**——降幅来自并集扩容（新增 hardening_mutation 目标的重复单测 + 15 加固测试贡献增量覆盖），非产品改动（产品源码零改动）。**②类引擎异步环全部热点维持 ACTIONABLE 且获得变异证据支撑**（mod.rs:289 CRAP 70.07、peerwire.rs:251/308、single.rs:48、pieceio.rs:435、trackpool.rs:412 CRAP 186.96 cov=0.086——与该函数 26 个变异存活缺口自洽）：cleaner/architect 三分类（①路由守卫链不拆 ②移交 hardender ③e2e 域）判定成立，变异实跑未推翻任何既有裁决。
- **mktorrent bencode 跨 bin 重复**：hardender 按 architect 裁决**维持登记、lib 化不实施**（构建图级变更 + 产品源码零改动达标线）；DRY 重扫产品侧另有 19 组 83-133 tok 克隆（登记为下游 specifier/architect 清单，见交接②分类文件）。
- **leech 专属 e2e、QA runner 族缺口**：沿历轮登记不变。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退）；D80①–④ 已全部结案**——本轮 hardender 无规格语义改动、无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（hardender FR-02-37 v1.3 变异加固轮）
1. **变异实跑（cargo-mutants 27.1.0，清单制差分变异）**：
   - single.rs 16 突变体：14 killed + 3 unviable（IpAddr: Default 真实编译错）+ 1 存活（m015 `delete !` toast 分支翻转）→ m015 由加固测试杀灭（复验全量单测 1052P 仍绿确认真覆盖缺口，非等价）。
   - bt_dialog.rs 119 突变体（--file 分块消费兑现）：窄过滤 98 killed + 1 timeout 捕获（m099 `&&`→`||` 挂起 = caught-by-timeout）+ 1 unviable + 19 存活；存活体全量单测复验再杀 3（m008/m015/m102），加固测试杀 13（m004/m005/m006/m011/m012/m016/m032/m057/m061/m064/m086/m087/m103），等价/受限论证 3（m077 ignored 去重无观察消费方；m097/m098 proxy_sel 边界防御钳制在当前调用图不可达）。**非等价存活率 0（bt_dialog 面）**。
   - bt/engine/mod.rs 20 突变体：9 killed + 3 unviable（IpAddr Default）+ 1 加固杀灭（m019 BT 启动 Io 失败 → Evt::BtFailed 确定性杀灭）+ 7 覆盖缺口登记（上表）。
   - trackpool.rs ws_* 面 31 突变体首轮：3 killed + 2 unviable + 26 待复验。
   - 总杀灭/处理论账：四文件 186 突变体 = killed 124 + timeout 捕获 1 + unviable 8 + 加固测试杀灭 14 + 等价论证 3 + 覆盖缺口登记 7 + 待复验 26 + 未跑 3 文件 308 点（复算自洽）。
2. **tests/hardening_mutation.rs（新增加固测试目标，独立成套，一测一靶）**：15 测试覆盖 14 个存活体（m005/m006 与 m086/m087 各合用一条精确规则集断言）+ 1 个既有缺口（m015 single_stream toast）；**全部经 known-answer 杀灭验证**（scratch 应用突变体 → 对应测试必红/必挂，13 killed + 1 timeout 捕获）；测试走真实事件路径（on_key 键盘 / TestBackend ui::draw + on_mouse 鼠标热区），产品源码零触碰。
3. **软 Gherkin 验收变异（最终验证序列第二步，受限兑现）**：runner 适配器 `.work/tmp/aps_adapter/runner_worker.py`（NDJSON 持久 worker，per-job 隔离，mutator-spec 协议完备，执行语义偏差已在适配器头注文档化）+ `gherkin-mutator --level soft` × 20 feature = **1123 突变全量执行，errors=1123**（执行层缺失的机械化证明，见受限项）。
4. **CRAP 复扫（最终验证序列第三步）**：cargo-llvm-cov 0.9.1 十七目标插桩合并 lcov（`--no-report` 分块 + `--no-run --lcov` 报告），crap_calc.py（cleaner 受限近似口径）产出 `.work/tmp/crap-report.txt`：94.3% 行覆盖 / 102 ACTIONABLE（对账见受限项）。
5. **DRY 复扫（最终验证序列第四步）**：PMD CPD 7.29.0 `-l rust --minimum-tokens 70`（EXIT=4 属正常），198 克隆组 = 产品侧 20 + 测试侧 178（划界按 #[cfg(test)] mod 起始行）；mktorrent 登记克隆在案 = 阳性对照；分类明细 `.work/tmp/cpd-classified.json`。
6. **经验沉淀**：`project/notes/testing.md`、`environment.md` 各追加本轮条目（cargo-mutants 27.1 持久 scratch 三段式口径 / pub(super) 臂真实事件路径覆盖法 / 存活体三分诚实分类 / 日期钉版清单 404 与版本带判例 / PMD 克隆划界正解 / llvm-cov 分块报告口径 / 磁盘账），INDEX.md 同步。
7. **本 handoff.md** 重写为本轮口径（交接四要素见下）。

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
   合计 **9170P/0F（17 目标）**。600s 调用窗口下集成目标按批拆分（每批 ≤4 目标）。
2. **会话运行时产物位置与复现方法**（均不进工作区产物、不进归档）：
   - 变异会话：`.work/tmp/mrunner/`——`parse_list_diff.py`（cargo-mutants `--list --diff` → 逐突变体 git-apply 补丁）、`run_chunk.sh`（持久 scratch 单 worker 应用-测试-还原-分类）、`recheck_survivors.sh`（存活体宽套件复验）、`verify_kills.sh`（加固测试杀灭验证）、`manifests/`（清单）、`mutants/<chunk>/*.diff`、`results/*.csv`（逐突变体判定）、`logs/`（逐突变体测试日志）。
   - 复现变异实跑示例：`cd .work/tmp/mrunner && ./run_chunk.sh <chunk> "<libtest过滤器>" 460`（scratch 缺失时脚本自动重建；复跑 peerwire：先用 parse_list_diff.py 生成清单再跑）。
   - trackpool 26 待复验复现：`./recheck_survivors.sh trackpool_ws --test hardening_g_engine`（后再跑 backfill/supervisor 套件）。
   - Gherkin 段：`.work/tmp/aps_adapter/runner_worker.py`（适配器）+ `.work/tmp/aps-mut/<feature>-report.json`（20 份 mutator JSON 报告）——复现：进 `.tools/Acceptance-Pipeline-Specification` 后 `bb gherkin-mutator --runner-worker "python3 …/runner_worker.py" --feature <abs> --work-dir <abs> --level soft --json`。
   - CRAP/DRY：`.work/tmp/ezr-17targets.lcov` + `crap-report.txt`；`cpd-report.txt` + `cpd-classified.json`；pristine 字节级备份 `.work/tmp/pristine/`（零改动证明基线）。
3. **feature 注释块与差分复用注意事项**：20 个 feature 头部已被本轮写入 `# acceptance-mutation-manifest-begin/end` 工具自管理注释块（含 tested_at 时间戳与 implementation_hash=unknown）。`--level soft` 差分复用会合并历史条目——`01-persistence-config.feature` 的 2026-10-06 旧 manifest 因 soft 级复用被部分保留（2 场景条目沿用）。内容未变的 feature 差分复跑会跳过已登记场景；**强制全量重跑 = 删除该注释块**（含 01-persistence-config 的旧条目一并清除后重跑）。
4. **等价突变体清单及失效条件（回补触发）**：
   - **等价类（3，bt_dialog）**：m077（merge_push 的 ignored 去重臂——ignored 仅被 `.first()` 消费，重复项无任何可观察差异；**失效条件：ignored 的消费方扩展为全量展示/计数/落盘**）；m097/m098（`proxy_sel.min(len-1)` 边界防御钳制——对话框内 proxy_sel 恒为有效索引、选项表在对话框存续期不收缩，钳制臂不可达；**失效条件：新增调用方传入越界 proxy_sel，或选项表在对话框打开期间可收缩**）。
   - **覆盖缺口（7，bt/engine/mod.rs，非等价——不得当等价使用）**：m001-m004（LEECH_UP_MIN/DOWN_MAX 私有常量算子替换；**回补触发：常量提升可见性，或引入时域可控的 leech 驱逐行为测试**——现行为边界需 ≥120s 连接存活时域）；m009/m011（LiveStats uploaded/downloaded/left 初值 resume 回填；**回补触发：LiveStats 暴露测试口或 resume 进度确定性断言**）；m013（`got_peers = !trackers.is_empty()` 首轮 rechoke 门控时序；**回补触发：announce/rechoke 时序行为测试**）。
   - **待复验队列（26，trackpool ws 面）**：判定悬置，复验命令见交接②；其行为面（ws_prepare_offers/ws_tracker_session/ws_handle_offer/ws_process_response）与 trackpool.rs:412 cov=0.086 的 CRAP 热点自洽，**预判为覆盖缺口而非等价**。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 单测（ezr bin 族 4 目标） | **1098P / 0F**（与基线一致） | cargo test --bins |
| 单测（12 集成目标） | **7125P / 0F**（2825+1063+3237，与基线一致） | 三批分跑 |
| 加固套件（新目标 hardening_mutation） | **947P / 0F**（含 15 加固测试 + 挂载单测） | cargo test --test hardening_mutation |
| 合计 | **9170P / 0F（17 目标）** | 复算：8223 + 947 |
| clippy | --all-targets 0 警告 0 错误 | cargo clippy --all-targets |
| fmt | 全量合规 | cargo fmt --all -- --check |
| 自动化架构检查 | 18 条规则全过 | bash scripts/arch_check.sh |
| 属性套件（独立口径） | **88P / 0F** | cargo test property_tests:: |
| 变异实跑 | 186 突变体处置闭环（见清单）+ 308 点未跑登记 | .work/tmp/mrunner/results/*.csv |
| 加固测试杀灭验证 | 14/14 通过（13 killed + 1 timeout 捕获） | .work/tmp/mrunner/results/verify_kills.csv |
| 零改动证明 | src/+Cargo.toml 字节级一致；tests/ 仅新增加固文件；features/ 仅工具自管理注释 | pristine cmp（.work/tmp/pristine） |

### 对账口径
- 本轮改动面 = `tests/hardening_mutation.rs`（新增，15 加固测试）+ 20 个 feature 头部工具自管理 manifest 注释块 + notes 两文件 + INDEX.md + 本 handoff；**产品源码（src/ 全部 + Cargo.toml）字节级零改动（pristine cmp 证明）**，产品行为面 = 零。
- 测试净变化 = 新增集成目标 1 个（947 = 挂载产品单测 932 + 15 加固测试）；基线 16 目标 8223P 零变化；无测试删除、无断言翻转。
- 总账复算：8223（基线 16 目标）+ 947（新目标）= 9170；bins 1098 + 集成 7125 + 加固 947 = 9170 精确自洽。
- CRAP 158→102 对账：新目标贡献并集扩容（挂载单测 + 加固测试的新覆盖路径），无产品改动；②类热点全部维持 ACTIONABLE（数值与未变异状态一致）。
- 磁盘口径：INC=0 + DEV_DEBUG=0 + TEST_DEBUG=0 每命令重 export；变异段 scratch（2G）与 llvm-cov 插桩工件分段清理，全程峰值余量 3.4G 未爆盘。

### 待办与待批
- **无待批项**（本轮无规格语义改动、无新增 D 值、无操作者裁决请求；mktorrent 维持登记系 architect 既定裁决的直接沿用）。
- 待办（移交后续轮）：① trackpool 26 待复验 + 3 文件 308 点未跑（复现命令交接②，操作者可续轮 hardender 或并入 QA 轮预算）；② 软 Gherkin 执行层回补（IR 驱动验收入口）移交 specifier/QA 轮裁决；③ QA runner 漂移 3 处与 verify 脚本 2 项沿历轮移交 QA/specifier 轮。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 下一节点建议 = `bin/swarm begin six-pack/QA`（终验链：QA runner 漂移 3 处与本轮登记一并处置；变异加固证据可直接支撑 QA 终验的测试充分性论证）；如操作者拟先补完变异面，可续 `bin/swarm begin six-pack/hardender`（从交接②断点续跑 peerwire/pieceio/trackpool 复验）。
