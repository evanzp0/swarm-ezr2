# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender（**mutator 等价突变过滤器扩展轮**——操作者显式授权 hardender 越过「上游脚本字节冻结」边界修改 APS mutator，彻底解决历轮遗留项）
> 路径：…hardender 修订轮 ⇒ QA 轮 ⇒ specifier 裁决落实轮 ⇒ **hardender 过滤器扩展轮（本轮，操作者 chat 指令）**
> 会话：six-pack-hardender-20261118（SESSION_TASK = mutator 等价突变过滤器扩展；上游交接 = 上轮 handoff + 操作者 chat 指令）
> 环境：rustup stable 1.100.0（rsproxy 镜像前台装机）、bb 1.13.229（.tools/bin/bb，ghfast.top 断点续传）、
> APS 仓 .tools/Acceptance-Pipeline-Specification（ghfast.top clone + 本轮扩展补丁）；ezr 产品源码零改动（沿 QA 轮 1090P/0F 基线）。
> 受限项：APS 执行层 runtime（.work/tmp/aps/ 四件套 + 59 handler + column_schema.json）本轮未重建——**verdict 级全量重跑待 runtime 重建**（机制与数据验证已闭环，见验证证据；软 Gherkin 历轮总账 1124 = 1107 killed + 17 等价类 + 0 errors 沿 r3 口径有效）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.57**）。ezr 不做旧版本兼容。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同，见 11-trackers-window v1.14 注记）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| QA 轮 | 五规程缺口可执行化 + harness 退出礼仪修复 + 产品一处 bug 修复 | 已交付 | 06 = 8P/0F；07 = 11P/1S；11 = 6P/0F；09 = 17P/7S；10 = 1P/7S；产品单测 1090P/0F、fmt/clippy clean |
| specifier 裁决落实轮 | 规格分歧两处裁决落地（QA-AB-11④ 文案 + FR-02-21 v1.25 补 udp 端口完备） | 已交付 | rg 残留清零；三层口径对齐；产品零改动 |
| **hardender 过滤器扩展轮（本轮）** | **mutator 项目等价突变过滤器扩展（授权越界）：工具机制 + 17 格数据登记 + 补丁落盘** | 已交付 | bb test 11 测/51 assertions 全绿（上游 8 测不动）；探针四对照 + 复用对照全过；真实数据 6 feature 副本 **17/17 精确命中、零过度匹配**；产品源码零改动（git status 仅 2 新增文件） |

### 待办承接（历轮待办 → 本轮处置）
- **mutator 等价突变过滤器扩展（上游字节冻结，操作者裁决面）**：✅ **本轮彻底解决**——操作者显式授权越界，APS bb 版 mutator 已支持 `--equivalence-filter`（mutator-spec §Equivalent Mutation Filters 口径），17 格登记为 `project/qa/equivalence-filter.json` 过滤器条目，机制四对照 + 真实数据命中验证闭环。遗留待办**结清**。
- **规格分歧登记两处**：✅ 上轮（specifier）已裁决落地结案。
- **历轮受限项（缺口账目 ID / trackpool.rs:644 等价论证 / 2 条 unviable / RB-09+UC-10 / mktorrent bencode）**：沿历轮登记不变。

### 遗留受限项（当前有效）
- **APS 执行层 runtime 未重建（本轮新增登记）**：`.work/tmp/aps/` 会话运行时产物不随会话流转，runtime.py/generator.py/adapter.py/drive.py/handlers/column_schema.json 需重建后才能跑 verdict 级软 Gherkin 全量。**失效条件/回补触发 = 操作者开 runtime 重建轮**（重建口径见 notes/testing.md 修订轮条目 + r3 handoff 四要素第 2 条）。重建后 `--level full --equivalence-filter` 直跑即得 verdict 级账目（预期 = 1107 killed + 17 filtered_equivalents + 0 survived + 0 errors）。
- **Go 版 mutator 未扩展**：`--equivalence-filter` 仅 bb 版实现；工程规则「APS 优先 Babashka 版」成立。回退 Go 备用路径时过滤不生效、等价体如实回现为幸存（安全失败模式）。
- **缺口账目 ID 不可精确复现 / trackpool.rs:644 等价论证 / 2 条 unviable / RB-09+UC-10 / mktorrent bencode**：沿历轮登记不变。
- **07-add-bt.feature 场景哈希滞后**：沿上轮登记（下次 gherkin 变异运行自刷新）。

## 二、当前产出情况

### 本会话产物清单（hardender 过滤器扩展轮）
1. **mutator 等价突变过滤器扩展**（`.tools/Acceptance-Pipeline-Specification` bb 版，**操作者授权越界修改**）：
   - CLI 新增 `--equivalence-filter <file>`（缺省 = 上游行为字节级不变）；
   - 过滤发生在执行前（spec §Equivalent Mutation Filters 口径）：命中突变体不执行、不进 total/结果列表，进单独报告区（文本 `filtered_equivalents=<n>` + `filtered equivalents (not executed)` 区；JSON `summary.FilteredEquivalents` + `filtered` 数组 + `filter-hash`）；exit 码只看幸存/错误，等价格不触发 exit 1；
   - 匹配键 = feature stem + 突变路径 `$.scenarios[i].examples[j].<col>` + original + mutated **四元全匹配**（任一分量漂移即失配 → 突变体如实回现为幸存，fail-loud）；
   - manifest/stamp 过滤器感知：manifest 顶层 `equivalence_filter_hash` + 场景 result.FilteredEquivalents；场景 mutation_count = 执行数；复用条件含过滤器哈希相等（过滤器变更/移除 → 场景如实重执行）；stamp 哈希掺入过滤哈希（跨过滤器整文件快捷跳过失效）；
   - **缺陷修复（校准轮发现）**：bb `System/exit` 丢弃未 flush 的 stdout——上游 CLI 在幸存者>0 时报告静默丢失（潜伏缺陷）；CLI 统一 `exit!` 助手 flush 两路再退出。
2. **`project/qa/equivalence-filter.json`**：17 格逐格登记（6+2+4+3+1+1，模式/论证/失效条件承 r3 handoff 等价类清单——input-mirror ×4、classification-mirror ×2、capacity-mirror ×2、echo-mirror ×4、sampling-mirror ×3、endpoint-mirror ×1、count-mirror ×1），**逐格权威登记自此以本文件为准**。
3. **`project/qa/aps-mutator-equivalence-extension.patch`**：扩展全量 diff（516 行：CLI 24 + mutation.clj 193 + 测试 147）。**下游复现 = clone APS（ghfast.top/gh-proxy 前缀）→ `git apply <patch>` → `bb test` 全绿 → 调用带 `--equivalence-filter project/qa/equivalence-filter.json`**。
4. **经验沉淀**：`project/notes/environment.md` 3 条（bb System/exit 吞 stdout 判据与修复、镜像 magic bytes 先验纪律、APS bb/Go 能力分叉）+ `project/notes/testing.md` 3 条（过滤器口径、manifest/stamp 过滤哈希复用语义、机制验证四对照与副本纪律）+ INDEX 同步。
5. **本 handoff.md** 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 工具侧基线 | 扩展前 8 tests/30 assertions 绿 → 扩展后 **11 tests/51 assertions 绿**（上游测试零改动全过 = 无过滤旗标行为不变） | `(cd .tools/Acceptance-Pipeline-Specification && PATH=$PWD/../../.tools/bin:$PATH bb test)` |
| 机制四对照（known-answer 校准） | ①基线无过滤器 total=4/survived=4/exit 1（上游行为复现）；②阳性命中 1 格 → total=3/filtered_equivalents=1/单独报告区；③阴性 original 错配 → 零过滤（total=4）；④killed+全过滤 → total=0/filtered=4/exit 0 | 探针与 stub 见 `.work/tmp/probe/`（会话级；复现 = patch 应用后按 handoff 四要素 2 重建） |
| 同过滤器复用对照 | 二次运行 skipped_scenarios=1/total=0/filtered_equivalents=4/exit 0（stamp+manifest 过滤哈希感知生效）；bb 单测另证：改 justification（哈希变）→ 如实重执行；无过滤器 → 全量重测 | 同上 |
| **真实数据命中**（6 feature 副本 + stub） | **17/17 精确命中、零过度匹配**：retry-backoff 6（retry_after ×4 + status ×2）、slots-queue 2、modify-task 4、ui-conns 3、named-proxy 1、cmd-window 1；无过滤器对照 total=16 = 全量（机制缺省惰性） | `bb gherkin-mutator --feature <副本> --work-dir .work/tmp/realval/work/<stem> --runner-worker "python3 .work/tmp/probe/stub_worker.py" --level soft --workers 1 --equivalence-filter project/qa/equivalence-filter.json` |
| JSON 报告形态 | summary.FilteredEquivalents + filtered 数组（ID/Path/Class/Justification/Invalidation）+ filter-hash；results 无过滤污染 | `--json` 落盘样例 `.work/tmp/realval/json-probe.json`（会话级） |
| 产品源码零改动 | `git status --short` 仅 2 个新增文件（patch + filter json）；features/ezr/qa 文档零修改；产品基线沿 QA 轮 1090P/0F（零代码改动不重跑） | `git status --short` |
| RAG 索引 | 200 文件 / 1899 分块 / 索引健康（产出写入后已刷新） | `bin/rag/rag status` |

### 对账口径
- **17 格三源一致**：r3 handoff 等价类清单（git 历史 640afa2）≡ `project/qa/equivalence-filter.json` 逐格登记 ≡ 当前 features discover 派生（`.work/tmp/muts-dump.ndjson` 232 突变体中 17/17 唯一命中，δ 确定性复现登记值 10→14/60→68/90→99/120→112/403→399/404→401/0→2 等）。
- **软 Gherkin 历轮总账口径不变**：1124 = 1107 killed + 17 等价类 + 0 errors（r3 口径沿有效）；本轮后该账的「17 等价类」在工具层升格为 filtered_equivalents 分桶，verdict 级确认待 runtime 重建轮 `--level full --equivalence-filter` 直跑（预期零跳过全执行）。
- **副本纪律**：机制验证/stub 运行全部指向 `.work/tmp/` 副本，真实 features 零污染（metadata 自管理写回仅落在副本）。

### 待办与待批
- **本会话无待批**——遗留待办（mutator 过滤器扩展）已结清；verdict 级重跑属受限项回补（操作者显式开轮），非待批。
- **制度性修订建议（不自行改动，提请操作者裁决）**：`packs/_common/engineering.md`「上游钉死路径的兼容桥接」条款与本轮授权越界的关系——建议后续增补例外条款：「APS mutator 项目等价突变过滤器扩展（bb 版）经操作者授权已落地，扩展以 patch 形态随工作区流转（project/qa/aps-mutator-equivalence-extension.patch），重建工具链时先 clone 后 apply」；同类授权越界轮可直接引用本条。
- **（沿历轮，非裁决项）QA runner 族五规程缺口**：07/09/10/11 handler 可执行化为独立小轮，由操作者显式决定。

### 交接四要素（hardender 要素）
1. **本地验证命令清单（按序）**：
   - 工具侧：`(cd .tools/Acceptance-Pipeline-Specification && PATH=…/.tools/bin:$PATH bb test)` 全绿 →
   - 软 Gherkin verdict 级（runtime 重建后）：`python3 .work/tmp/aps/drive.py --feature <stem>`（基线全绿先行）→ `bb gherkin-mutator --feature project/features/<stem>.feature --work-dir .work/tmp/aps/mut/<stem> --generated-dir .work/tmp/aps/mut/<stem>/generated --runner-worker "python3 .work/tmp/aps/adapter.py" --level full --workers 1 --equivalence-filter project/qa/equivalence-filter.json --json` →
   - 产品面（沿 QA 轮口径，产品零改动轮可继承）：`cargo fmt --check` / `cargo clippy --bin ezr --all-targets` / `cargo test --bin ezr` / CRAP 直采管线 / PMD cpd（命令细节见 notes/environment.md）。
2. **会话运行时产物位置与复现**：探针与 stub `.work/tmp/probe/`（probe.feature/stub_worker.py/kill_worker.py/filter-pos-neg-all.json）、真实数据验证 `.work/tmp/realval/`（6 副本 + 报告）、突变派生清单 `.work/tmp/muts-dump.ndjson`、17 格路径推导脚本同目录 dump-muts.bb——全部会话级 complete 即清；复现 = ①clone APS → `git apply project/qa/aps-mutator-equivalence-extension.patch` → bb test；②`bb <repo>/…/dump-muts.bb project/features/<6 feature>` 派生路径核对 filter json；③按上表命令对副本跑四对照。
3. **feature 注释块与差分复用注意事项**：mutator manifest 注释块自管理写回（宪法允许）；**本轮后 manifest 新增顶层 `equivalence_filter_hash` 与场景 result.FilteredEquivalents、场景 mutation_count = 执行数**——旧 manifest（无过滤哈希）在新工具下复用语义兼容（零幸存条目仍可复用）；**强制全量重跑 = `--level full`**（忽略 stamp/manifest，过滤仍生效）；**过滤器文件变更（含 justification 措辞）→ 哈希变 → 相关场景如实重执行**；真实 features 本轮零写回（验证全在副本）。
4. **等价突变体清单及失效条件**：软 Gherkin 17 格 → **权威登记 = `project/qa/equivalence-filter.json`**（逐格模式/论证/失效条件，四元匹配键绑定 original+mutated；失效 = feature 编辑致路径/值漂移、产品引入校验边界、规格新增区分性断言、mutator δ 逻辑变更——任一触发即过滤失配、突变体如实回现为幸存，回补 = 重派生路径更新条目）；沿历轮不变：trackpool.rs:644 `delete !`（失效 = webrtc 栈可用测试环境）、2 条工具判定 unviable、trackpool.rs:412 CRAP/ACT 名义值竞速登记。

### 移交建议
- **six-pack 流转路径保持完结态**：hardender 为 post-Done 专项小轮（操作者显式 begin），无自动流转。
- 如需续做：①APS runtime 重建轮（verdict 级全量确认，预期 1107 killed + 17 filtered_equivalents + 0 errors）；②五规程 handler 可执行化（07/09/10/11）——均由操作者显式决定。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。

By hardender.
