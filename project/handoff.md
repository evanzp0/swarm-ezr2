# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（**QA runtime 重建轮**——操作者 chat 显式指令：①开一次 QA runtime 重建轮，按 notes/testing.md 修订轮口径重建四件套 + verdict 级全量；②解决 07-add-bt.feature 场景哈希滞后）
> 路径：…hardender PMD 销项轻量轮 ⇒ **QA runtime 重建轮（本轮，操作者 chat 指令）**
> 会话：six-pack-QA-20261119-runtime（SESSION_TASK = APS 执行层四件套重建（runtime/generator/adapter/drive + column_schema + 产品面 handler）+ 20 feature verdict 级全量 + 07-add-bt 哈希自刷新核验；上游交接 = 上轮 handoff + 操作者 chat 指令）
> 环境：rustup nightly 1.102.0（rust-toolchain.toml 钉版，TUNA 重装）、bb v1.13.229 + APS（gh-proxy 装机 + project/qa/aps-mutator-equivalence-extension.patch 干净应用，bb test 11 测/51 assertions 0F/0E）、ezr/ezr-fixture/ezr-mktorrent debug 三件构建在位；PMD 7.29.0 与 cargo-mutants 不在位（沿登记）；**APS 执行层四件套本轮重建在位**（.work/tmp/aps/，会话运行时产物）。
> 受限项：**历轮两项核心受限项本轮销项**——APS runtime 未重建（✅ 四件套重建 + verdict 级账目闭合）与 07-add-bt 场景哈希滞后（✅ verdict 运行自刷新 + 独立复算 9/9 一致）；Go 版 mutator 未扩展、缺口账目 ID 再生条件、trackpool.rs:644 等价论证、2 条 unviable 终态沿登记。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.57**）。ezr 不做旧版本兼容。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同，见 11-trackers-window v1.14 注记）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| QA 轮 | 五规程缺口可执行化 + harness 退出礼仪修复 + 产品一处 bug 修复 | 已交付 | 06 = 8P/0F；07 = 11P/1S；11 = 6P/0F；09 = 17P/7S；10 = 1P/7S；产品单测 1090P/0F、fmt/clippy clean |
| specifier 裁决落实轮 | 规格分歧两处裁决落地 | 已交付 | rg 残留清零；三层口径对齐；产品零改动 |
| hardender 过滤器扩展轮 | mutator `--equivalence-filter`（授权越界）+ 17 格登记 + 补丁落盘 | 已交付 | bb test 11 测/51 assertions 全绿；探针四对照 + 复用对照全过；17/17 精确命中；产品源码零改动 |
| hardender 合规修订轮 | mktorrent bencode 方案 A（自包含化 + #[path] 单源）+ engineering.md 授权越界例外条款 | 已交付 | 验证门全绿（基线 1090P/0F 精确复现、arch_check 18 规则、CRAP ACT=101 恒等） |
| hardender PMD 销项轻量轮 | PMD 7.29.0 装机 + cpd 双扫描销项对账 + spawn_outgoing unviable 人工补证 | 已交付 | cpd 220→219 差集全额归因；unviable E0599 人工实证终态；产品源码零改动 |
| **QA runtime 重建轮（本轮）** | **APS 四件套重建 + column_schema 1124 格 + 19 产品面 handler + verdict 级全量 + 07-add-bt 哈希销项** | 已交付 | **verdict Σ = 1107 killed + 17 filtered_equivalents + 0 survived + 0 errors（与预期口径精确一致）**；20/20 基线 0F/0E；07 manifest 9/9 场景哈希独立复算一致；产品源码零改动 |

### 待办承接（历轮待办 → 本轮处置）
- **APS 执行层 runtime 未重建**：✅ **本轮结清**——四件套 + column_schema.json（1124 格）+ 19 handler 重建，verdict 级全量闭合（详见本轮产物 2）。
- **07-add-bt.feature 场景哈希滞后**：✅ **本轮销项**——verdict 级运行（--level full 忽略 stamp/manifest 全执行）重写 manifest（tested_at 2026-11-19T00:54:15Z），hash-json 同算法独立复算 9/9 场景哈希与当前 feature 内容一致（详见本轮产物 3）。
- **mutator 等价突变过滤器扩展 / 规格分歧裁决 / mktorrent bencode / PMD cpd**：✅ 历轮结清不变。
- **RB-09 + UC-10（QA 规程 S 口径用例）**：本轮 runtime 落点更新——两处场景格全为 closed，突变在词汇面先拒、无 mutation job 到达 handler，SkipExecution 保留为 handler 侧语义（本轮未触发）；verdict 0 errors 与 r3 账目相容。规程层面 S 口径登记不变。
- **Go 版 mutator 未扩展 / 缺口账目 ID 再生（cargo-mutants 27.1 + 339 scope）/ trackpool.rs:644 等价论证 / 2 条 unviable 终态 / trackpool.rs:412 CRAP 竞速登记**：沿历轮登记不变（本轮仅文档 + .work 运行时产物，产品源码与变异 scope 零触碰）。

## 二、当前产出情况

### 本会话产物清单（QA runtime 重建轮）
1. **APS 执行层四件套重建**（`.work/tmp/aps/`，会话运行时产物不入归档）：
   - `runtime.py`：IR 展开（Background 前置 + Outline 按行展开 + 无 Examples 单执行）+ **格级 schema 校验（词汇面第一道：closed 域外先拒 / open_int lo0/hi base+15；双层统一在调度器公共上游落地——handler 分派前强制 check_cell，handler 分支不得绕过）** + 占位符解析（示例列→行值，非示例自由参数保留字面）+ handler 调度；
   - `generator.py`：acceptance-entrypoint-generator（`<json-ir> <generated-dir>` 两位置参数；entry.py 确定性产出 + metadata/<slug>.json，implementation_hash 仅覆盖 generated_files）；
   - `adapter.py`：NDJSON 持久 worker（job{id,feature_json,generated_dir,work_dir} → resp{id,outcome,output,error,duration}；base/变异 IR 深比较逐格差分定位 → APS_AFFECTED 单行执行，协议偏差沿登记，全量覆盖由基线承担）；
   - `drive.py`：20 feature 全量驱动（parser → generator → **基线先行（不绿不开工）** → mutator `--level full --equivalence-filter`）；
   - `column_schema.json`（build_schema.py 生成）：**1124 格全登记**——closed 1064（缺省 = 格原值单元素集，场景数据格）+ open_int 60（FILTERED 17 镜像格 + ANCHORED handler 锚格 + LO_ONLY 纯输入域格）；
   - `handlers/`（19 个 bespoke PTY 场景 handler，复用 qa/runners/harness.py，逐场景对齐 suite_*.py 蓝图）：徽标收敛锚（fixture 块数截断）/ 步骤字面锚（排队第 1/2 位）/ 固定夹具锚（9 文件种子统计行）/ 产品常量锚（并发回退 4、重试 1/5 分母）/ 速度观察锚（修改限速前后 plateau 对照）/ 期望报错锚（CLI 非法值 PTY 内报错退出）。
2. **verdict 级全量**（--level full --equivalence-filter，20 feature 顺序前台）：**Σ total=1107（全部执行）killed=1107 survived=0 errors=0 filtered_equivalents=17**——与 handoff 预期口径（1107 + 17 + 0 + 0）精确一致；逐 feature 报告落 `.work/tmp/aps/reports/<stem>.json`，汇总 `.work/tmp/aps/verdict-summary.json`。对账链：Δ复刻盘点（Python 复刻 fnv64a/mutate-value/discover，先与 mutator 全量对账 1107+17=1124 逐数恒等）→ 基线全绿（20/20）→ verdict 闭合。
3. **07-add-bt 场景哈希销项**：verdict 运行重写 stamp/manifest；**hash-json 同算法独立复算（sha256 of cheshire compact JSON）9/9 场景哈希与当前 feature 内容一致**；带过滤 feature 的 stamp 与 equivalence_filter_hash 落账核验通过（retry-backoff 6F / modify-task 4F / cmd-window 1F，filter_hash = sha256(filter.json) 一致）。
4. **产品基线复核**：20/20 feature 基线全绿（词汇面 + 19 handler 产品面），产品源码零改动（git 状态 = features manifest/stamp 写回 + notes/handoff 文档）。
5. **经验沉淀**：`project/notes/testing.md` 11 条（双层统一落调度器公共上游、格级 closed 缺省语义与 open_int 三分、锚六类实例化、非 PTY CLI 直跑 ENXIO 假报错、fixture ?speed 每连接门控聚合口径、校验中窗口时序归约、plateau 采样、BT 树 root/root/ 双层展开、ACCENT 段文本匹配、SkipExecution 落点变化、Δ复刻对账与 hash-json 独立复算）+ `project/notes/environment.md` 5 条（bb 子命令 APS 目录内调用、bb/APS 装机复证、无 TTY ENXIO、ss 监听端口观察、磁盘账）+ INDEX 同步 + 本 handoff.md 重写。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| mutator 扩展装机 | 11 测/51 assertions 0F/0E | `cd .tools/Acceptance-Pipeline-Specification && PATH=.tools/bin:$PATH bb test`（补丁 git apply 先行） |
| Δ复刻对账 | 1107 + 17 = 1124 逐数恒等 | `python3 .work/tmp/aps/mutate_table.py` |
| 基线（每 feature） | 20/20 全绿 0F/0E | `APS_FEATURE_PATH=<feature> APS_BASELINE=1 python3 .work/tmp/aps/runtime.py .work/tmp/aps/ir/<stem>.json` |
| verdict 级全量 | Σ 1107 killed + 17 filtered + 0S/0E | `python3 .work/tmp/aps/drive.py`（或逐 stem）；报告 `.work/tmp/aps/reports/` |
| 07 哈希核验 | 9/9 场景哈希一致 | hash-json 复算脚本（本轮 notes/testing.md 记载算法），对照 manifest |
| 产品源码零改动 | git 状态 = features 注释块写回 + 文档 | `git status --short project/` |

### 对账口径
- **verdict 账目语义**：`--level full` 忽略 stamp/manifest 全执行；等价过滤器 17 格执行前过滤（不进 total，单独 filtered_equivalents 汇总）；全部 20 feature mutator exit 0。**预期口径精确命中 = 1107 + 17 + 0 + 0**。
- **schema 分类口径**：closed 缺省 = 格原值单元素集（场景数据格：夹具名/枚举/期望显示/动作计数/非法输入向量/fixture 参数——「该格规格值唯一确定」）；open_int = 产品输入格（lo0/hi base+15），三分登记 = FILTERED 17（镜像，执行前过滤）+ ANCHORED（handler 锚杀灭，界内突变放行产品面）+ LO_ONLY（本轮突变全负 δ，lo=0 杀灭；**缺 handler 时未来界内突变如实幸存**）。与修订轮口径（格级键 / closed 值集封闭含产品面格 / open_int lo0 hi base+15 / extra_lines 0 格 lo=0）逐条对应。
- **与 r3 59 handler 的口径关系**：本轮 19 handler 为精简重建——r3 词汇面的示例值语义校验已由格级 closed schema 承载（域外先拒），handler 仅保留「界内突变需产品锚杀灭」的场景与基线产品面验证；verdict 账目与 r3 总账精确一致（1124 = 1107 + 17），执行层架构与登记口径一致（双层、差分定位、副本纪律、全量覆盖由基线承担）。
- **RB-09/UC-10**：无 mutation job 到达（相关格 closed 词汇面先拒），SkipExecution 保留为 handler 侧语义，verdict 0 errors 相容。
- **CRAP/DRY 段**：按操作者指令本轮验证序列 = APS runtime 重建 + verdict 级全量（CRAP 工具与 PMD/cargo-mutants 不在位沿登记，不在本轮范围）。
- **工作区零残留**：handler 隔离环境 `.work/tmp/aps-run/`、盘点副本 `.work/tmp/discover/`、四件套与 IR/work/gen/reports `.work/tmp/aps/`——complete 自动清理 `.work`；产品树改动仅 features stamp/manifest 注释块（工具自管理，宪法允许）+ notes/handoff。

### 待办与待批
- **本会话无待批无待办**——操作者两项指令（runtime 重建轮 + 07 哈希销项）均落地结清。

### 交接四要素（QA 要素）
1. **本地验证命令清单（按序）**：bb test（mutator 扩展装机）→ Δ复刻盘点对账 → 逐 feature 基线 → `python3 .work/tmp/aps/drive.py`（verdict 全量）→ verdict-summary.json 汇总对账 → 07 哈希复算。历轮全套验证命令沿上轮 handoff 四要素 1 不变——产品源码零改动，基线 1090P/0F、CRAP ACT=101、arch_check 18 规则全绿均沿历轮有效。
2. **会话运行时产物位置与复现**：`.work/tmp/aps/`（四件套 + schema + reports）与 `.work/tmp/aps-run/`、`.work/tmp/discover/`——complete 即清；复现 = 按本轮 notes/testing.md / environment.md 条目重建（bb/APS 装机 → 补丁 → 二进制三件 → 四件套随 .work 清空需按登记口径重建）。
3. **feature 注释块与差分复用注意事项**：20 feature 的 stamp/manifest 已重写为本轮 verdict 口径（tested_at 2026-11-19，带过滤 feature 含 equivalence_filter_hash）；下一轮 soft 级复用条件 = feature 内容 / background / implementation_hash / 过滤哈希不变；feature 编辑 → 场景哈希漂移 → 如实重执行（fail-loud）；副本纪律：机制验证运行绝不指向真实 features。
4. **等价突变体清单及失效条件**：沿历轮不变——17 格权威登记 `project/qa/equivalence-filter.json`；trackpool.rs:644 `delete !`（失效 = webrtc 栈可用测试环境）；2 条 unviable（spawn_outgoing 删臂人工实证 E0599 终态；count_invalid_request 编译错类）；trackpool.rs:412 CRAP/ACT 名义值竞速登记。本轮新增登记：schema open_int **LO_ONLY 格**（01-add-task s0.concurrency、01-slots-queue s9.slots、01-throttle-proxy s9.g_down/t_down、03-modify-task s1/s9.after、04-ui-conns s15.conns、05-ui-pieces s0/s4.conns、06-cmd-window s1.extra_lines 非过滤行）——失效条件 = 产品引入对这些格的区分性锚（届时应转 ANCHORED + handler）或登记等价类（转 FILTERED）。

### 移交建议
- **six-pack 流转路径保持完结态**：QA 为 post-Done 专项小轮（操作者显式 begin），无自动流转。
- 遗留受限项可解路径沿历轮移交建议不变：①Go 版 mutator 移植（QA 或授权越界）；②缺口账目 ID 再生（hardender，需装 cargo-mutants 27.1）；③644 等价论证重验（webrtc 环境）——均由操作者显式决定。
- **APS runtime 重建口径已完整落档**（notes/testing.md 本轮条目 + 四件套源码随会话归档面 = .work 不入归档，重建按登记口径执行）；下轮如需复用执行层，建议在沙箱重置后按登记口径重建（本轮全程命令与判定锚均已留档）。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。

By QA.
