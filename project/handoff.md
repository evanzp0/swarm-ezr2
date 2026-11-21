# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（**遗留受限项销项轮**——操作者 chat 裁决「一轮串行」：③trackpool.rs:644 等价论证重验 + ①缺口账目 ID 再生 + ④LO_ONLY 格产品锚回补 + ②Go 版 mutator --equivalence-filter 移植）
> 路径：…QA runtime 重建轮（上轮）⇒ **遗留受限项销项轮（本轮，操作者 chat 裁决）**
> 会话：six-pack-QA-20261121-disposal（SESSION_TASK = 四项遗留受限项前台串行销项；上游交接 = 上轮 handoff + 操作者 chat 裁决）
> 环境：rustup nightly 1.102.0（TUNA 0.05s 最快 + default profile）、cargo-mutants 27.1.0、bb 1.13.229 + APS（bb 补丁干净应用 + bb test 11 测/51 assertions 0F/0E）、Go 1.23.9（aliyun）、pyte/pexpect 补装；ezr/ezr-fixture/ezr-mktorrent/ezr-proxy debug 四件在位；APS 执行层四件套本轮重建在位（.work/tmp/aps/，会话运行时产物）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.57**）。ezr 不做旧版本兼容。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同，见 11-trackers-window v1.14 注记）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| QA runtime 重建轮（上轮） | APS 四件套重建 + verdict 级全量 + 07-add-bt 哈希销项 | 已交付 | verdict Σ = 1107 killed + 17 filtered + 0S/0E；20/20 基线 0F/0E |
| **遗留受限项销项轮（本轮）** | ③644 改判杀灭 + ①缺口账目 ID 权威清单 + ④LO_ONLY 敞口消灭（schema 口径演进）+ ②Go 版 mutator filter 移植 | 已交付 | **verdict Σ = 1107 killed + 0 survived + 0 errors + 17 filtered（历轮口径精确复现）**；bin 基线 1091P/0F + 12 集成 + 3 bin unittests 全绿（总 9567P/0F）；fmt/clippy clean；go test 全绿 + 四对照 + 真实数据 17/17 |

### 待办承接（历轮待办 → 本轮处置）
- **trackpool.rs:644 等价论证（失效条件 = webrtc 栈可用测试环境）**：✅ **本轮销项**——new_peer_connection 沙箱可用性实证（RTCPeerConnection 本地创建成功），杀灭测试 gap644 落 mod.rs（trackpool.rs 零改动 → --list 不漂移），cargo-mutants 单突变体实测 caught；账目改判 140 覆盖函数突变体 = **138 killed + 0 等价 + 2 unviable 终态**（详见本轮产物 1）。
- **缺口账目 ID 再生（cargo-mutants 27.1 + 339 scope）**：✅ **本轮销项**——--list 三文件实测对账恒等（209/78/52）+ 权威 ID 清单落档 `project/qa/mutation-gap-registry.json`（339 entry + 644 改判 + 2 unviable 注记），历轮数字登记升级为逐 ID 清单（详见本轮产物 2）。
- **LO_ONLY 格"未来界内突变如实幸存"敞口**：✅ **本轮销项（口径演进）**——第一轮无 handler verdict 实跑收集 153 界内幸存格，逐格裁决全部符合 closed 缺省语义，schema 收敛为 FILTERED 17 + closed 1107；LO_ONLY 18 格 closed 化后敞口永久消灭且 verdict 账目守恒精确复现（详见本轮产物 3）。
- **Go 版 mutator 未扩展（--equivalence-filter 仅 bb 版）**：✅ **本轮销项**——Go 版移植完成（equivalence.go + 三处接线），go test 全绿 + 合成探针四对照 + 真实数据 20 feature 副本 17/17 精确命中零过度匹配；补丁落盘 `project/qa/aps-go-equivalence-extension.patch`（详见本轮产物 4）。
- **2 条 unviable 终态 / trackpool.rs:412 CRAP 竞速登记**：沿历轮登记不变（unviable = 已人工实证定案账目；CRAP 竞速对账口径已接受，非可执行任务）。**历轮遗留受限项至此全部处置完毕（销项或定案），账面无未决受限项**。

## 二、当前产出情况

### 本会话产物清单（遗留受限项销项轮）
1. **③644 等价体改判杀灭**：`gap644_ws_answer_gate_webrtc_off_answers_arm_inert` 落 `src/bt/engine/mod.rs` cfg(test)（GAP644_REMOTE_ID 常量 + PendingOffer 显式 use）——构造 = webrtc:false ctx + 本地 accept_async ws 会话 + pending_offers pre-insert 真实 pc 的 PendingOffer + 假 sdp answer；突变体（delete ! 后 answers 臂执行）经 accept_answer 失败臂 mark_peer_fail 使 peer 进失败冷却 → 断言 killed；原始下守卫 return 断言过。cargo-mutants `--baseline skip -F --re 'trackpool.rs:644:8' -- gap644_` 实测 caught。产品源码增量仅 mod.rs 测试模块（trackpool.rs/黑名单账目三文件 --list 零漂移）。
2. **①缺口账目 ID 权威清单**：`project/qa/mutation-gap-registry.json`（version 1；339 mutants 逐 ID + counts + adjudged 注记；tool=cargo-mutants 27.1.0，generated_at 2026-11-21）——339 = pieceio 209/peerwire 78/trackpool 52 与历轮逐数恒等（零漂移）；未覆盖函数 199 突变体沿历轮杀灭状态登记；覆盖函数 140 = 138 killed（含 644 改判）+ 2 unviable 终态。
3. **④LO_ONLY 敞口消灭 + schema 口径演进**：APS 四件套重建（.work/tmp/aps/：runtime/generator/adapter/drive/build_schema + column_schema.json 1124 格）→ 第一轮无 handler verdict（954K+153S+17F=1124）→ 153 幸存格逐格裁决（上下文导出）全 closed → **schema version 3：FILTERED 17（equivalence-filter.json 镜像，open_int lo0/hi base+15）+ closed 1107（单元素集）**→ verdict 全量 Σ = **1107 killed + 0 survived + 0 errors + 17 filtered_equivalents**（与历轮预期口径精确一致）。LO_ONLY 18 格（handoff 列级简写的 IR 行展开：01-add-task s0.concurrency、01-slots-queue s9.slots、01-throttle-proxy s9.g_down/t_down×3 行、03-modify-task s1.after×2/s9.after×3、04-ui-conns s15.conns、05-ui-pieces s0/s4.conns、06-cmd-window s1.extra_lines row1/row2）当前突变（全负 δ + 限速 dither）全 killed + 未来界内突变由 closed 先拒——敞口永久消灭。**口径演进要点**：历轮 ANCHORED 锚面（33 格清单不可恢复）以 closed 先拒等效杀灭（账目守恒），产品面锚验证由 qa/runners suite_*.py 独立承载；mutator δ 确定性（fnv64a 定向）使 LO_ONLY 敞口封闭无需等未来轮。
4. **②Go 版 mutator --equivalence-filter 移植**：`.tools/Acceptance-Pipeline-Specification` 内 Go 侧六文件改动（equivalence.go 新增：LoadEquivalenceFilter version 1 校验 + 四元全匹配 + EquivalentView + stampHash；mutator.go：Summary.FilteredEquivalents + Report.filtered/filter-hash + 执行前排除 + acceptedSkips/manifestEntryReusable 过滤器感知 + 文本报告 filtered 区；manifest.go：equivalence_filter_hash + MutationCount 扣减 + FeatureStampValid 掺 filter-hash；main.go：--equivalence-filter 旗标）；**bb 与 Go Discover/MutateValue 逐值恒等互证**（16/16 对照 diffs=0）；go build/vet clean + go test 三包全绿（含 4 新 filter 测试）；**合成探针四对照**：A 基线无过滤 total=16 不变 / B 阳性 total=14+filtered_equivalents=2 / C 阴性 original 错配零过滤 / D killed+全过滤 exit 0；**真实数据 20 feature 副本 filtered=17 精确命中零过度匹配**；Go-only 补丁落盘 `project/qa/aps-go-equivalence-extension.patch`（2312 行，HEAD 树干净可应用，与 bb 补丁互不重叠）。
5. **产品基线复核**：bin 单测 **1091P/0F**（1090 历轮基线 + gap644 精确对账）；12 集成目标 + 3 bin unittests 全绿（总计 9567P/0F）；fmt clean + clippy clean（gap644 初版 MutexGuard-过-await lint 按历轮作用域块修法收口）；产品源码改动仅 mod.rs 测试模块追加（trackpool.rs 等三文件逐字节零改动——--list 339 恒等为证）。
6. **经验沉淀**：`project/notes/testing.md` 8 条（schema 三分收敛二分定案、δ 确定性与敞口时点语义、四件套重建接口契约清单、bb-Go discover 互证、四元匹配 stem 同名纪律、runner-worker 协议契约、`[m` 序列三通道改写幻象 chr(91) 双证判别、644 杀灭实证细节、-F --re 双旗标、缺口账目清单落档）+ `project/notes/environment.md` 4 条（销项轮全新沙箱装机实录、9.9G 盘贴边清理口径、逐二进制路径提取 regex、gh-proxy 降速断点续传）+ INDEX 同步 + 本 handoff.md 重写。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| bb 版 mutator 扩展装机 | 11 测/51 assertions 0F/0E | `cd .tools/Acceptance-Pipeline-Specification && PATH=.tools/bin:$PATH bb test`（bb 补丁 git apply 先行） |
| 644 突变体杀灭 | caught by gap644 | scratch 副本删 tests/：`cargo mutants --baseline skip -F --re 'trackpool.rs:644:8' -- gap644_`（log/src__bt__engine__trackpool.rs_line_644_col_8.log = kill 证据） |
| 产品基线 | bin 1091P/0F + 集成全绿 | `cargo test --no-run`（DEBUG=0 三件套）→ 逐二进制 timeout 前台跑 |
| 缺口账目对账 | 339 恒等（209/78/52） | `cargo mutants --list --file src/bt/engine/pieceio.rs --file src/bt/engine/peerwire.rs --file src/bt/engine/trackpool.rs` |
| verdict 级全量 | Σ 1107 killed + 0S/0E + 17 filtered | `python3 .work/tmp/aps/drive.py`（报告 .work/tmp/aps/reports/ + verdict-summary.json） |
| Go 版 filter | go test 全绿 + 四对照 + 17/17 | `go test ./...`；探针脚本见本轮 notes/testing.md 四对照口径 |
| 补丁可复现性 | bb/go 补丁 HEAD 树干净应用 | `git apply --check project/qa/aps-mutator-equivalence-extension.patch`（原始 APS clone 上）+ go 补丁同 |
| fmt/clippy | clean | `cargo fmt --check && cargo clippy --all-targets` |

### 对账口径
- **verdict 账目守恒**：历轮 1107+17 与本轮 1107+17 **逐数精确一致**——closed 化不改变 total（job 照发、词汇面先拒计入 killed），filter 17 未扩容（153 裁决格全走 closed 非 filter）。
- **schema 口径演进（版本 3）**：open_int 三分（FILTERED/ANCHORED/LO_ONLY）退役为二分（FILTERED 17 + closed 1107）；演进依据 = 153 界内幸存格逐格裁决 + 账目守恒 + LO_ONLY 敞口永久消灭；**此演进属操作者裁决面的口径变更——本轮以实跑数据落定，若操作者否决（要求恢复 ANCHORED + handler 形态），按 notes/testing.md 锚回补轮先例为 153 格补 handler（工程量预估 ~19 feature bespoke PTY handler）**。
- **工作区零残留**：.work/tmp/aps/（四件套 + ir + gen + aps-run + reports）与 .work/tmp/qa-run/（harness 隔离环境）——complete 自动清理；scratch 突变验证目录 .work/tmp/mut-scratch644 同随 .work 清空；产品树改动 = mod.rs 测试模块 + features stamp/manifest 注释块（工具自管理，宪法允许）+ qa/ 两个新登记文件 + notes/handoff。
- **07-add-bt 哈希销项维持**：verdict 运行重写 20 feature stamp/manifest（tested_at 2026-11-21 + equivalence_filter_hash 落账）——哈希自刷新机制再次实证，历轮销项维持有效。

### 待办与待批
- **本会话无待批无待办**——操作者裁决的四项销项全部落地结清。
- **登记供知情**：schema 口径演进（见对账口径第二节）属历轮 ANCHORED 形态的退役——操作者可否决，否决路径已留档。

### 交接四要素（QA 要素）
1. **本地验证命令清单（按序）**：bb test（mutator 扩展装机）→ cargo-mutants --list 三文件对账（339）→ gap644 单突变体 caught 复验（scratch 副本）→ 产品基线逐二进制 → `python3 .work/tmp/aps/drive.py`（verdict 全量）→ verdict-summary.json 汇总对账 → go test + 四对照（Go 版 filter）。历轮全套验证命令沿上轮 handoff 四要素 1 不变。
2. **会话运行时产物位置与复现**：`.work/tmp/aps/`（四件套 + schema + reports）——complete 即清；复现 = 按本轮 notes/testing.md「APS 执行层四件套重建的接口契约清单」逐条重建（slug 全路径算法/base 推导三级/summary 大驼峰键为关键陷阱）。
3. **feature 注释块与差分复用注意事项**：20 feature 的 stamp/manifest 已重写为本轮 verdict 口径（tested_at 2026-11-21 + equivalence_filter_hash）；下一轮 soft 级复用条件 = feature 内容 / background / implementation_hash / 过滤哈希不变；feature 编辑 → 场景哈希漂移 → 如实重执行（fail-loud）；副本纪律：机制验证运行绝不指向真实 features，且**副本必须保持同名**（四元匹配 feature 字段 = stem）。
4. **等价突变体清单及失效条件**：17 格权威登记 `project/qa/equivalence-filter.json` 不变；历轮 trackpool.rs:644 等价体已改判杀灭（失效条件兑现，账目移除）；2 条 unviable 终态 + trackpool.rs:412 CRAP 竞速登记沿历轮；**LO_ONLY 登记已清空**（closed 化，无失效条件——若产品演进引入对这些格的区分性行为，重新裁决为 open_int + handler 按锚回补轮先例）。

### 移交建议
- **six-pack 流转路径保持完结态**：QA 为 post-Done 专项小轮（操作者显式 begin），无自动流转。**历轮遗留受限项已全部处置完毕（3 销项 + 1 定案维持），账面清空**。
- Go 版 mutator 补丁（aps-go-equivalence-extension.patch）与 bb 版补丁（aps-mutator-equivalence-extension.patch）独立可应用——未来沙箱重置后按 notes/environment.md 装机清单重建 APS 双版能力对齐。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。

By QA.
