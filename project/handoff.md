# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect（FR-02-37 v1.3 架构评审轮，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner/architect/QA 收敛链 ⇒ specifier 显示指标/调整/裁决/新增需求轮（v1.47–v1.53）⇒ specifier 裁决落实修订轮（v1.54）⇒ coder 产品实现/复核重验轮 ⇒ specifier PEERS/SEEDS 核证修订轮（v1.55）⇒ specifier D80 裁决落实轮（v1.56）⇒ coder FR-02-37 v1.3 产品实现轮 ⇒ cleaner FR-02-37 v1.3 收敛轮 ⇒ **architect FR-02-37 v1.3 架构评审轮（本轮 2026-11-14，任务 = 例行架构评审；下一步建议 six-pack/hardender 或 six-pack/QA）**
> 会话：six-pack-architect-20261114（任务 = FR-02-37-v1.3-架构评审轮；上游交接 = project/handoff.md cleaner 收敛轮版）
> 环境：nightly 1.102.0-nightly (8eb778d04) 本轮经 TUNA 前台装齐 minimal+rustfmt+clippy（官方脚本 + DIST_SERVER/UPDATE_ROOT 双变量；与上游基线同 commit，lint 基线无漂移）；~/.cargo/config.toml rsproxy sparse 新配；磁盘全程峰值余量 3.4G（INC=0 + DEV_DEBUG=0 + TEST_DEBUG=0 口径全量跑通）。度量四件（cargo-mutants/cargo-llvm-cov/PMD/APS-bb）未装机——本轮评审零违规、无度量与规格任务，工具面收窄沿 architect 轮先例（2026-11-12 判例），后续 hardender/QA 涉度量轮按 notes/environment.md 重建。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.56**）+ `mission/` 四份 phase 详述（现行 phase-01 v1.53、phase-02 v1.24）。ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-13 QA 收敛轮） | ezr 产品实现 + 收敛/终验链 | 已交付 | 历轮对账在案 |
| specifier v1.53–v1.56 五轮 | mission 五版 + phase 详述 + features 修订 + QA 同步 + ezr-demo 同构落地 | 已交付 | APS 双检历轮在案；demo 单测 58P/0F、PTY 全过 |
| coder 复核重验轮（2026-11-14） | 零代码新增；tests/gen 剥头副本机械重生成 + notes + handoff 重写 | 已交付 | 单测 8100P/0F、clippy 0、fmt 过 |
| specifier v1.55/v1.56 两轮 | FR-02-37 v1.3 规格核证与 D80 结案 + ezr-demo 字段语义对齐 | 已交付 | APS 双检通过；demo 全绿 |
| coder FR-02-37 v1.3 产品实现轮（2026-11-14） | evt.rs 入口减落地 + 单测同步 + clippy 机械修复 | 已交付 | 单测 8146P/0F（16 目标）、clippy 0 警告、fmt 合规 |
| cleaner FR-02-37 v1.3 收敛轮（2026-11-14） | CRAP 收敛批（scrollbar hit/render 拆分 + 三文件补测 10 条 + 死臂删除 3 处）+ DRY 单点化两处 + 变异点扫描登记 + notes 三文件 + handoff 重写 | 已交付 | 全量回归 8221P/0F（16 目标）、clippy 0、fmt 合规 |
| **architect FR-02-37 v1.3 架构评审轮（本轮 2026-11-14）** | **四阶段评审（零违规）+ 移交项裁决两项（维持登记）+ 属性测试 +2 + notes 三文件 + 本 handoff 重写** | 已交付 | 全量 **8223P/0F**（16 目标 = 基线 8221 + 2 属性，复算自洽）、clippy --all-targets 0 警告、fmt 合规、arch_check 18 规则全过 |

### 遗留受限项（当前有效）
- **APS/bb 未装机（沿历轮）**：本轮规格与 QA 零改动、无 Gherkin 度量任务，APS 双检不适用；后续涉规格/度量轮按 engineering.md 重建。
- **度量四件未装机（本轮沿用 architect 工具面收窄先例）**：cargo-mutants/cargo-llvm-cov/PMD 未重建，CRAP/DRY/变异扫描本轮未复测——本轮评审零违规无度量任务；hardender/QA 涉度量轮需先重建（装机位置与镜像口径沿 notes/environment.md）。
- **QA runner fixture 漂移 3 处（移交 QA/specifier 轮，cleaner/architect 不维护 QA 套件——SKILL 边界）**：① `suite_modify_bt.py` 4F；② `suite_config_template.py` 5F；③ `suite_persistence_config.py` QA-PC-04 1F。沿历轮登记不变。
- **verify_trackers_popup.py T2/T3 共 3F（既有失败）**：移交后续轮修复，沿历轮登记。
- **verify_size_move.py 陈旧化**：v1.48 时代脚本，移交后续轮决定修复或退役，沿历轮登记（architect 本轮维持，非架构产物）。
- **CRAP 登记口径（cleaner 全量扫描基线，16 目标 lcov 并集 89.6%；architect 本轮确认维持三分类）**：排除 property_tests 跳跑伪影与 SF 别名重复后，产品代码 CRAP>6 且 cov<1.0 共 158 点。①TUI 分发/守卫链（keys.rs:7 comp=32、mouse.rs:178/275/395/454/532、cmd.rs:255、ui/mod.rs:113、multiline.rs:24、textfield.rs:225 cov=0.925 残余 steps>8 病态臂、textfield.rs:334 cov=0.868）——判例「路由/守卫链=本质复杂度不拆」；②引擎异步环（bt/engine/mod.rs:289/593、peerwire.rs:251/308、supervisor/single.rs:48、pieceio.rs:435、trackpool ws_* 族）——不安全类移交 hardender；③环境边界（main.rs:253/369、ezr-proxy SUT、sentinel.rs:51、webrtc DataChannel 臂）——e2e/手动验证域。config.rs:548 comp=48/cov=1.0 维持登记不硬拆。
- **mktorrent bencode 跨 bin 重复（architect 本轮裁决：维持登记，lib 化不实施）**：`src/bin/ezr-mktorrent.rs` 自带 encode/encode_into 副本（232-token 顶级克隆对）。取证：`src/bt/bencode.rs` 依赖 `super::BtError` 门面类型非自包含，`#[path]` 硬包含不可行；bin-only 结构下消除重复只有 lib 化一途（构建图级变更）——mktorrent 为构建工具 bin（非变异热点），收益不抵风险，移交 hardender 按需裁决。
- demo clippy 口径：--all-targets 零警告（沿历轮）。
- leech 专属 e2e、变异实跑（hardender 未跑）、BT engine/add-bt/anti-leech/trackers-window/cmd-window 族无专属 runner（沿历轮）。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退）；D80①–④ 已全部结案（2026-11-14 操作者闸门裁定）**——本轮 architect 为评审轮，无新增 D 值、无规格语义改动。

## 二、当前产出情况

### 本会话产物清单（architect FR-02-37 v1.3 架构评审轮）
1. **四阶段架构评审（结论 + 违规清单 + 改进动作）**：
   - UI/核心分离：ui 层零 IO 框架导入（规则 12）、app 层零 ui 依赖（规则 13）、sentinel 仅入口可见（规则 7）均实测成立；根级交互原语（scrollbar/multiline/textfield）为 app/ui 双消费共享层，crate 根放置正确（移入 ui/ 即 app→ui 违规），零违规零动作。
   - 依赖规则：arch_check 18 条规则复跑全过（model 零外层依赖、engine/bt 机制层方向纯净、engine↔bt 协调枢纽判例维持）；规则集本轮零新增形态，维持不扩。
   - 信息隐藏：engine/supervisor 窄接口（私有 mod + 门面 re-export + pub(super)）、model task/timefmt 私有 + re-export、textfield/multiline 公共面最小且消费者在案（track_index_at 消费点 app/scrollbar.rs:233），零违规零动作。
   - 局部代码质量：cleaner 改动面（scrollbar hit/render 拆分、textfield 死臂删除、multiline/textfield 补测、两处测夹具单点化）逐项复核结构清晰、辅助私有、无跨层泄漏，零违规零动作。
2. **移交项裁决两项（均维持登记）**：CRAP 158 点三分类确认维持（判例一致）；mktorrent bencode 副本裁决维持（取证 bencode.rs 非自包含，见受限项）。
3. **属性测试 +2（`src/property_tests.rs`，独立交付物）**：
   - `prop_wrap_segments_partition_and_bounds`：段有序不重叠/段长 ≥1/段界不越域/段宽 ≤width（单字超宽豁免）+ 双侧空格校验（跳过域与尾域必为断点空格）——锁定 cleaner 死臂删除与断点空格丢弃契约。
   - `prop_scrollbar_hit_domain_classification`：可滚动域全分类（Head/Tail 仅箭头位、Thumb/Track 下标界）+ 计数恒等式 seen_thumb==th、seen_track==tl−th + 三向越界静默——锁定 cleaner hit 拆分行为。
   - `proptest-regressions/property_tests.txt` 新增永久回归案例（shrink 最小输入）。
4. **经验沉淀**：`project/notes/` 三文件追加（architecture：根级交互原语放置裁决/mktorrent 取证法；testing：断言过界实例/hit 全域分类模板/clippy 机械修形态；environment：rustup 镜像复测/600s 拆批口径/磁盘三 DEBUG 口径）+ INDEX.md 同步；无跨项目通用条目（归属分流自检）。
5. **本 handoff.md** 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 单测（ezr bin 族 4 目标） | **1098P / 0F**（ezr 1052 = 基线 1050 + 2 属性、fixture 30、mktorrent 1、proxy 15） | cargo test --bins |
| 单测（12 hardening 集成目标） | **7125P / 0F**（2159+4005+961 分批实测，属性过滤在各集成目标 0 命中 = 挂载面零增量） | cargo test --test <目标> 逐目标实测 |
| 合计 | **8223P / 0F（16 目标）** | 分项复算自洽 |
| clippy | --all-targets 0 警告 0 错误 | cargo clippy --all-targets |
| fmt | 全量合规 | cargo fmt --all -- --check |
| 自动化架构检查 | 18 条规则全过 | bash scripts/arch_check.sh |
| 属性套件（独立口径） | **88P / 0F**（86→88，+2 本轮新增） | cargo test property_tests:: |

### 对账口径
- 本轮改动面 = `src/property_tests.rs`（+2 属性）+ `proptest-regressions/property_tests.txt`（+1 回归案例）+ notes 三文件 + INDEX.md + 本 handoff；**产品行为面 = 零**（评审轮零违规零改造，产品代码零改动）。
- 测试净变化 = 属性 +2（独立交付物，属性套 86→88）；基线单测零变化、无测试删除、无断言翻转、无构建配置改动。
- 总账复算：cleaner 基线 8221 + 2 属性 = 8223，与 16 目标分项（1098 + 2159 + 4005 + 961）精确自洽。
- 属性对账隔离：属性测试独立命令独立报告（cargo test property_tests:: = 88P），不计入基线单测对账公式；bin 族总数含属性（ezr 1052 = 1050 + 2），与历轮「bin 对账属性分层口径」一致。
- 磁盘口径：INC=0 + DEV_DEBUG=0 + TEST_DEBUG=0 每命令重 export；12 集成目标分两批（6+6）前台实跑，第二批第 6 目标超时中断后单目标补跑（二进制复用，计数全额计入）。

### 待办与待批
- **无待批项**（本轮评审轮无规格语义改动、无新增 D 值、无操作者裁决请求）。
- 受限项清单维持移交后续轮（见第一节）：变异实跑移交 hardender；QA runner 漂移 3 处与 verify 脚本 2 项移交 QA/specifier 轮；leech e2e 缺口沿历轮。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 下一节点建议 = `bin/swarm begin six-pack/hardender`（变异实跑兑现 cleaner/architect 登记面：CRAP ②类引擎异步环 + bt_dialog 119 点 --file 分块消费；度量四件需先重建）或 `bin/swarm begin six-pack/QA`（终验链；QA runner 漂移 3 处与本轮登记一并处置）；如操作者拟终结收敛链，亦可直接 complete 收档。
