# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner（FR-02-37 v1.3 收敛轮，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner/architect/QA 收敛链 ⇒ specifier 显示指标/调整/裁决/新增需求轮（v1.47–v1.53）⇒ specifier 裁决落实修订轮（v1.54）⇒ coder 产品实现/复核重验轮 ⇒ specifier PEERS/SEEDS 核证修订轮（v1.55）⇒ specifier D80 裁决落实轮（v1.56）⇒ coder FR-02-37 v1.3 产品实现轮 ⇒ **cleaner FR-02-37 v1.3 收敛轮（本轮 2026-11-14，任务 = 例行收敛；下一步建议 six-pack/architect）**
> 会话：six-pack-cleaner-20261114（任务 = FR-02-37-v1.3-收敛轮；上游交接 = project/handoff.md coder 产品实现轮版）
> 环境：nightly 1.102.0-nightly (8eb778d04) 本轮经 TUNA 前台装齐 minimal+rustfmt+clippy+llvm-tools-preview（rustup-init 直链 + DIST_SERVER/UPDATE_ROOT 双层路径双变量）；~/.cargo/config.toml rsproxy sparse 新配；磁盘 9.9G（INC=0 + DEBUG=0 口径全量跑通，峰值余量 1.9G）；度量三件套装机 .tools/bin（cargo-mutants 27.1.0、cargo-llvm-cov 0.9.1 经 gh-proxy 预编译直链）+ .tools/pmd（PMD 7.29.0，gh-proxy 断点续传 135MB）。APS/bb 未装机——本轮规格与 QA 零改动（APS 双检不适用）、无 Gherkin 度量任务（受限项登记）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.56**）+ `mission/` 四份 phase 详述（现行 phase-01 v1.53、phase-02 v1.24）。ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35，本轮清理同样不引入兼容层）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-13 QA 收敛轮） | ezr 产品实现 + 收敛/终验链 | 已交付 | 历轮对账在案 |
| specifier v1.53–v1.56 五轮 | mission 五版 + phase 详述 + features 修订 + QA 同步 + ezr-demo 同构落地 | 已交付 | APS 双检历轮在案；demo 单测 58P/0F、PTY 全过 |
| coder 复核重验轮（2026-11-14） | 零代码新增；tests/gen 剥头副本机械重生成 + notes + handoff 重写 | 已交付 | 单测 8100P/0F、clippy 0、fmt 过 |
| specifier v1.55/v1.56 两轮 | FR-02-37 v1.3 规格核证与 D80 结案 + ezr-demo 字段语义对齐 | 已交付 | APS 双检通过；demo 全绿 |
| coder FR-02-37 v1.3 产品实现轮（2026-11-14） | evt.rs 入口减落地 + 单测同步 + clippy 机械修复 | 已交付 | 单测 8146P/0F（16 目标）、clippy 0 警告、fmt 合规 |
| **cleaner FR-02-37 v1.3 收敛轮（本轮 2026-11-14）** | **CRAP 收敛批（scrollbar hit/render 拆分 + 三文件补测 10 条 + field_window/wrap_segments 死臂删除）+ DRY 单点化两处（supervisor/engine 测夹具）+ 变异点扫描登记 + notes 三文件 + 本 handoff 重写** | 已交付 | 全量回归 **8221P/0F**（16 目标，= 基线 8146 + 10 新测 + 65 挂载副本，分项对账自洽）、clippy --all-targets 0 警告 0 错误、fmt --check 全量合规 |

### 遗留受限项（当前有效）
- **APS/bb 未装机（沿历轮）**：本轮规格与 QA 零改动、无 Gherkin 度量任务，APS 双检不适用；后续涉规格/度量轮按 engineering.md 重建。
- **QA runner fixture 漂移 3 处（移交 QA/specifier 轮，cleaner 不维护 QA 套件——SKILL 边界）**：① `suite_modify_bt.py` 4F；② `suite_config_template.py` 5F；③ `suite_persistence_config.py` QA-PC-04 1F。沿历轮登记不变。
- **verify_trackers_popup.py T2/T3 共 3F（既有失败）**：移交后续轮修复，沿历轮登记。
- **verify_size_move.py 陈旧化**：v1.48 时代脚本，移交后续轮决定修复或退役，沿历轮登记。
- **CRAP 登记口径（本轮全量扫描基线，16 目标 lcov 并集 89.6%）**：排除 property_tests 跳跑伪影与 SF 别名重复后，产品代码 CRAP>6 且 cov<1.0 共 158 点。结构性登记分类：①TUI 分发/守卫链（keys.rs:7 comp=32、mouse.rs:178/275/395/454/532、cmd.rs:255 on_cmd_key、ui/mod.rs:113 draw、multiline.rs:24 渲染内核 comp=22/cov=1.0、textfield.rs:225 field_window comp=17/cov=0.925 残余 steps>8 病态臂、textfield.rs:334 wrap_segments comp=12/cov=0.868）——architect 轮判例「路由/守卫链=本质复杂度不拆」；②引擎异步环（bt/engine/mod.rs:289/593、peerwire.rs:251/308、supervisor/single.rs:48、pieceio.rs:435、trackpool ws_* 族）——异步核心 + 拆分属不安全类移交 hardender；③环境边界（main.rs:253/369 CLI 入口、ezr-proxy SUT、sentinel.rs:51 进程守卫、webrtc DataChannel 臂）——e2e/手动验证域。config.rs:548 配置解析核心 comp=48/cov=1.0（CRAP=comp 下限 48）随 strict 校验条款本质复杂，维持登记不硬拆。
- **死臂登记结案（本轮已删 3 处，证据在案）**：field_window 未聚焦 pre/post 中间臂（budget.max(1) 钳制 × anchor=len 不变式证明，见 notes/architecture.md）、wrap_segments 空段防御块（非空输入必有 push 的计数论证）。field_window 残余 steps>8 回退臂（L255 区段）为病态震荡兜底，登记不硬凑（构造 9 轮震荡需人工病态宽度序列，收益不抵风险）。
- **mktorrent bencode 跨 bin 重复（登记不并）**：`src/bin/ezr-mktorrent.rs` 自带 encode/encode_into 副本（CPD 232-token 顶级克隆对）——bin-only crate 结构下跨 bin 共享需 lib 化重构，属构建图级变更移交 architect/hardender 裁决；mktorrent 为构建工具 bin，副本短期维持。
- demo clippy 口径：--all-targets 零警告（沿历轮）。
- leech 专属 e2e、变异实跑（hardender 未跑）、BT engine/add-bt/anti-leech/trackers-window/cmd-window 族无专属 runner（沿历轮）。
- PMD/jscpd 口径：CPD 双阈值 100tok=74 组 / 70tok=204 组（本轮实测基线）；PMD 7.29.0 落位 swarm/.tools/pmd（工具缓存不进归档）。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退）；D80①–④ 已全部结案（2026-11-14 操作者闸门裁定）**——本轮 cleaner 为行为保持收敛，无新增 D 值、无规格语义改动。

## 二、当前产出情况

### 本会话产物清单（cleaner FR-02-37 v1.3 收敛轮）
1. **CRAP 收敛批（保持行为）**：
   - `src/scrollbar.rs`：`Bar::hit` 拆分（along_vertical/along_horizontal/edge_hit/track_hit 四辅助，comp 17→6）+ `render` 拆分（glyph_set/min_span_ok/pressed_arrow_style/edge_glyph/track_glyph/paint 六辅助，comp 23→6）；新增 5 测（退化几何守卫/水平错行/render 不可滚零改动/render 按下箭头样式/越界面板静默）。**验证：bin 套件口径两函数从 CRAP>6 清单消失（CRAP=6）。**
   - `src/ui/multiline.rs`：新增 2 测（wrap 跟随臂消费/空缓冲占位符 DIM2 渲染）——draw_multi_line_input cov 0.947→**1.000**（CRAP=comp=22 渲染内核登记）。
   - `src/textfield.rs`：新增 3 测（零预算未聚焦早返回/断点+空格跳过/断点后超宽单字）+ 删除 3 处可证死臂（field_window 未聚焦中间臂 ×2、wrap_segments 空段防御块）——field_window cov 0.526→**0.925**、wrap_segments cov→0.868（残余 steps>8 病态兜底臂登记）。
2. **DRY 单点化两处**：
   - `src/engine/supervisor/mod.rs`：throttle_free/throttle_global_dl 双构造体收编 `test_shared_with_dl_rate(rate)`（差异面=dl 速率参数）。
   - `src/bt/engine/mod.rs`：fake_udp_tracker/fake_udp_tracker_report_engine_port 公共 serve 循环提取 `fake_udp_tracker_serve(sock, seeder_port, Option<port_tx>)`，两夹具退化为三行委托；CPD 36-line/316-token 顶级克隆对消除。
   - mktorrent bencode 重复登记不并（见受限项）。
3. **变异点扫描（--list 不实跑，沿 SKILL scan/count 口径）**：本轮 coder 改动文件计数 = evt.rs 48 / task_lines.rs 75 / bt_dialog.rs 119（与 architect 轮登记 119 精确同值，文件结构未漂移）；bt_dialog 119>100 维持 architect 轮「不强拆，hardender --file 分块消费」登记。
4. **覆盖率基建（会话级，随 .work 清理）**：llvm-cov 直采管线建立（LLVM_PROFILE_FILE 逐二进制 → llvm-profdata merge → llvm-cov export --format=lcov → crap_calc.py 并集），16 目标 lcov 并集 89.6% 基线报告； cargo-llvm-cov 自动合并不可信三坑与 profraw 组合矩阵污染实录入 notes。
5. **经验沉淀**：`project/notes/` 三文件追加（architecture：拆分模板/死臂证明/i==start 构造/夹具单点化；testing：直采管线/污染实录/死臂验证闭环/挂载不均匀对账；environment：覆盖率构建隔离/PMD 大资产/mutants 成本口径）+ INDEX.md 同步；无跨项目通用条目（constitution 归属分流）。
6. **本 handoff.md** 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 单测（ezr bin 族 4 目标） | **1096P / 0F**（ezr 1050 = 基线 1040+10 新测、fixture 30、mktorrent 1、proxy 15） | cargo test --bins |
| 单测（12 hardening 集成目标） | **7125P / 0F**（挂载增量 +10/+8/+8/+8/+0/+0/+0/+8/+3/+8/+10/+10，总账 8146+10+65=8221 复算自洽） | cargo test --test <目标> 逐目标实测 |
| 合计 | **8221P / 0F（16 目标）** | 分项复算自洽 |
| clippy | --all-targets 0 警告 0 错误 | cargo clippy --all-targets |
| fmt | 全量合规 | cargo fmt --all -- --check |
| 覆盖率（16 目标 lcov 并集） | 89.6%（79623/88853 行） | crap_calc.py 直采管线（notes/testing.md 管线记录） |
| CRAP 收敛 | scrollbar hit 17.6→6、render 23→6（清单除名）；multiline draw cov→1.000；textfield field_window 0.526→0.925、wrap_segments→0.868 | crap_calc.py bin 口径复扫 |
| DRY | CPD 100tok 74 组 / 70tok 204 组（基线实测）；顶级克隆对（engine 夹具 316tok、supervisor 夹具 179tok）已消除 | pmd cpd -d src --language rust --minimum-tokens 100 |
| 变异点扫描 | evt.rs 48 / task_lines.rs 75 / bt_dialog.rs 119（不实跑） | cargo mutants --list --file <三文件> |

### 对账口径
- 本轮改动面 = `src/scrollbar.rs`（hit/render 拆分 + 5 测）+ `src/ui/multiline.rs`（2 测）+ `src/textfield.rs`（3 测 + 死臂删除）+ `src/engine/supervisor/mod.rs`（测夹具单点化）+ `src/bt/engine/mod.rs`（测夹具单点化）+ notes 三文件 + INDEX.md + 本 handoff；产品行为面 = 零（保持行为收敛，仅死臂删除为可证不可达代码）。
- 测试净变化 = src 新增 10 条（scrollbar 5 / multiline 2 / textfield 3），挂载自动收编 65 副本入 12 集成套件；无测试删除、无断言翻转、无构建配置改动。
- 死臂删除证据：field_window 未聚焦臂——`budget.max(1)` 入口钳制（L175）× 未聚焦 anchor=chars.len()（L202-206）⇒ window_for hi≡len（right_cut 恒假）且 lo=0 ⟺ total≤b≤budget<total 矛盾（left_cut 恒真）；wrap_segments 空段块——非空输入必有 push（零溢出走收尾 push L377-379，有溢出走环内 push），L384 判空恒假。行为不变性由既有 + 新增单测双重锁定。
- 兼容口径：ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）；本轮为结构收敛，不涉持久化格式。
- 磁盘受限说明：单测 16 目标分批前台实跑；覆盖率构建隔离 CARGO_TARGET_DIR=.work/tmp/cover-target（line-tables-only 口径 2.2G）+ profraw 直采，度量毕即删；target/debug 构建缓存由 `bin/swarm complete` 打包前自动清理。

### 待办与待批
- **无待批项**（本轮无规格语义改动、无新增 D 值）。
- CRAP 登记清单（158 点三分类）与 mktorrent bencode 副本归属，移交 architect 轮确认或维持登记（见受限项）。
- verify_size_move.py 陈旧化处置 + QA runner 漂移 3 处 + verify_trackers_popup T2/T3 3F 维持移交后续轮（见受限项）。
- 变异实跑维持移交 hardender；leech e2e 缺口沿历轮。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 下一节点建议 = `bin/swarm begin six-pack/architect`（架构评审轮：本轮收敛面集中 scrollbar/textfield/engine 测夹具，architect 可做依赖方向与模块边界复核；如操作者拟终结收敛链，亦可直接 QA 终验或 complete 收档）。
