# 交接（handoff）

> 流程：six-pack　当前节点：**six-pack/specifier（phase-05 HTTP 引擎提速 P0–P2 规格轮）——已完成，规格就绪待批**
> 路径：**six-pack/specifier（本轮）** → coder → cleaner → architect → (hardender) → QA
> 会话：six-pack-specifier-turbo（上游交接 = 上轮 handoff + 操作者 chat 指令）
> 环境：stable 1.100.0（rustup 默认链，TUNA 整链路前台装齐 10.4MB/s + cargo registry TUNA sparse）+ bb 1.13.230（gh-proxy，.tools/bb/）+ APS clone（.tools/Acceptance-Pipeline-Specification/，gherkin-parser/ir-dry-checker 可用）；ezr 产品树本轮**零改动**（未编译未跑测试——specifier 验证边界：规格层校验只跑 APS）
> 受限项：①chat 选择题面板不可用（IM 通道）沿袭 v1.45–v1.62 十三轮先例——Inversion 阶段 1 = 操作者 chat 指令文本 + `project/ezr-http-dw-tunning.md` 全文即答案，残余微口径 7 项按源裁决登记 D83–D89（可回退）；②IM 通道 Bash 输出层吞 `[h` 序列（显示伪象）沿 bug18 族，判别口径 python read_text + 负向前瞻（本轮已清点并修复真缺陷存量，见一节）；③与 MDM 的性能基准（同机对比/netem 高 RTT）沙箱无 MDM 与 tc netem 环境，登记为操作者线下验收口径，不进自动化 QA 判据。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.62**）；phase-01/02 为现行详述（03/04 计划登记稿；**phase-05 本轮新建**，FR-05-01–13 + D83–D89）。ezr 不做旧版本兼容沿袭（操作者钦定沿 v1.45/FR-02-35）。
- ezr-demo：仓库内独立子项目；本轮零接触（phase-05 未涉 demo 镜像需求——提速改造属产品引擎层）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固 | features 01–12 + qa 规程 + runners + APS 四件套 | 已交付（沿上轮账） | 历轮全绿（上轮账口径：bin 1168P/0F + 集成 9016P/0F + demo 61P/0F + CRAP 100 + CPD 222） |
| **phase-05 规格轮（20261125，本轮）** | **mission.md v1.62 + mission/phase-05.md（FR-05-01–13/D83–D89）+ features/13-http-turbo.feature（17 场景）+ qa/13-http-turbo-qa.md（17 用例）+ 六份既有规格与六份既有 QA 同步修订 + notes 三文件沉淀** | 已交付 | APS parser+dry-check 7/7 feature 全过（exit 0；dry findings 建议性口径与未改动基线对照一致） |

### 遗留受限项（当前有效）
- **全仓 >100 变异点存量 / CRAP 100 / CPD 222**：历轮登记沿袭（上轮账口径），本轮零触及（未跑变异/度量、零产品改动面）；如需收敛由后续 architect 轮裁决。
- **qa/09-bt-engine-qa.md:70 字面 `ttp]` 真缺陷 1 处**（应为 `[http]`，历史会话遗留写入侧损坏）：登记待修，归下一 QA 轮顺带修复（本轮不动非本轮范围文件）。
- **规格待批**（见二节待办与待批）：phase-05 规格 = 本轮合成待操作者确认口径。

### 实现定义值登记
- **本轮新增（phase-05/D83–D89，随 mission/phase-05.md §7）**：D83 动态模式分块行 `<已下载>/<总字节> · 动态分段`；D84 scheduler 键 {dynamic, blocks} 大小写不敏感/默认 dynamic/非法拒绝启动/B 类快照；D85 min_split_kib 默认 256 仅动态模式/自适应降半/align 4096；D86 原地重试首立即 + 1/2/4/8s 退避上限 5 次（累计 6 次连接尝试升级任务级）；D87 probe 裸 GET + absorb/续传开区间/Range 失效臂保留；D88 稀疏失败静默回退 set_len；D89 sidecar 双格式（scheduler 字段 + (start,cursor) 段列表 / blocks 原格式，格式即引擎）。
- 默认值变更登记：DEFAULT_HTTP_CONCURRENCY 4→8、HTTP 并范围 1–64→1–128（CLI/config/引擎 clamp/对话框）、DEFAULT_THROTTLE_BURST_SECS 0.5→1.0、MIN_HTTP_BLOCK_SIZE 1MB→256KiB（DEFAULT_BLOCK_SIZE_HTTP 维持 1 MB——操作者裁决「默认 4 MB 去除」）。
- 历轮登记（D81①–⑤/D82①–③ v1.57 落定版、运行端点判定口径、ApplyGlobals 命令面、CLI 覆盖登记、进度条行尾恒空）全部沿袭有效。

## 二、当前产出情况

### 本会话产物清单（phase-05 规格轮）
1. **`project/mission.md` v1.62**：修订注 + §4 核心参数同步（默认并发 8/1–128、block_size_http 仅 blocks 模式）+ 分期索引 phase-05 行。
2. **`project/mission/phase-05.md`（新建）**：P0–P2 需求详述——FR-05-01 写合并（256 KiB WBuf）/02 throttle 比例等待/03 probe 复用 + 块下限 256 KiB/04 失败粒度原地重试/05 默认参数/06 planner 动态分段（assign/claim/commit/release/absorb + 不变量）/07 启动收尾（裸 GET + 聚合游标完成判定）/08 sidecar 双格式与动态续传/09 writer 写流水线（≤4 MiB 合并定位写 + 256 MiB 跑道）/10 稀疏预分配/11 条件 sync_data/12 config 面（scheduler/min_split_kib + FR-02-35 严格校验 + 模板 42 键）/13 UI 口径（D83 + 并发面板不变）；验收标准（正确性回归/planner proptest/端到端 QA/**MDM 对比基准 = 操作者线下口径**）；§7 裁决登记 D83–D89。**P3 增益层明确排除**。
3. **`project/features/13-http-turbo.feature`（新建，17 场景）**：默认动态引擎（裸 GET/开区间/动态分段行）/自适应切分/抢占/原地吸收/耗尽升级退避/限速贴附 ±10%/默认并发 8/范围 1–128（CLI+config）/大文件端到端/稀疏磁盘占用/kill -9 一致性/blocks 回退/新键校验/B 类快照/动态续传/无 Range 降级。
4. **`project/qa/13-http-turbo-qa.md`（新建，QA-HT-01–15 + 08b/13b）**：环境前置（fixture 新能力：Range 形态日志/延迟应答/断连注入/忽略 Range/1 GiB 流式/du 采样）；通过准则（请求日志唯一事实源/±10% 沿既有/原地重试 6 次口径/sidecar ≤ 磁盘实/稀疏 S 兜底/四组关联回归/性能基准线下口径）。**runner 可执行化归 QA 轮**。
5. **六份既有规格修订**（最小同步 + v1.51 修订注）：01-download-engine（04/05/06/07 钉 blocks、16 AC-1 改判动态默认）、01-resume-sidecar（01/03/04/05 mode-aware）、01-persistence-config（05 钉 blocks + 256 KiB 行回归）、01-config-template（42 键 + 内嵌钦定文本 v1.51 版 180 行 + 表行同步）、01-retry-backoff（02 原地重试耗尽口径 + 修订注）、01-add-task（06 默认 8、14 边界 65→129）。
6. **六份既有 QA 同步**：qa/01-{download-engine,resume-sidecar,persistence-config,config-template,retry-backoff,add-task}-qa.md（判据/前置/修订注逐条对应）。
7. **notes 沉淀**：environment.md（rustup stable 装机与 dist 路径判别 + `[h` 吞噬伪象扩族与真缺陷存量清点）、testing.md（dry-check findings 建议性口径与基线对照判读）、architecture.md（引擎默认切换的 mode-aware 钉定模式）+ INDEX.md 索引。
8. **附带修复**：历史会话遗留字面 `ttp]` 真缺陷 7 处归一（features 2 + qa 5）；qa/09-bt-engine-qa.md 余 1 处登记待办。

### 验证证据
| 验证项 | 结果 | 命令（cwd 见行注） |
|---|---|---|
| gherkin-parser + ir-dry-checker（7 个涉改 feature） | 7/7 exit 0 | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-parser <feature> <out>.ir.json && bb gherkin-ir-dry-checker <ir> <out>.dry.json)`（IR/dry 落 .work/tmp/，bb=.tools/bb/bb） |
| dry findings 基线对照 | 涉改 7 文件 12–204 条建议性 findings，与未改动基线（09-bt-engine 117/12-reload-config 206/01-tui-display 215）同族同量级；唯一 duplicate-in-scenario/high = 01-add-task-05 既有有意重复（非本轮） | 同上 |
| 字面 ttp] 真缺陷清点 | 本轮范围文件 0 残留（python 负向前瞻扫描） | `python3` 扫描脚本（见 notes/environment.md 本轮条目） |
| 产品构建/测试 | **未跑**（specifier 验证边界：规格层校验只跑 APS；产品端归 coder 轮） | — |

### 对账口径
- 场景总数：13-http-turbo 17（15 主 + 08b/13b）；QA-HT 17；既有六规格修订面 = 场景级最小同步（场景总数不变，无删除重排——01-add-task-06/14 与 01-resume-sidecar-01/03/04/05 为原场景口径修订，01-download-engine-04/05/06/07/16 同）。
- config 模板键数：40 → 42（[common] 6 + [http] 13 + [bt] 23）；内嵌钦定文本 174 → 180 行。
- 产品代码零改动：`git diff` 面 = mission/、features/ 7 文件、qa/ 7 文件、notes/ 4 文件、handoff.md。

### 待办与待批（未决项）
- **规格合成确认（待操作者）**：phase-05 规格（mission/phase-05.md + features/13-http-turbo.feature + qa/13-http-turbo-qa.md + 六份同步修订）是否准确捕获意图？受限退化选择题（chat 输入答案即可）：**1A 准确，无需修改 / 1B 基本准确，需局部修改 / 1C 有偏差，需重新讨论**。残余微口径 D83–D89（mission/phase-05.md §7）可整体否决/修订——随 1B/1C 答案一并给出编号（如 `1B D85 改为…`）。操作者 20261125 指令预设「完成后 complete」，本轮按预设归档；确认/修订意见可随后续 chat 指令落下一 specifier 轮（修订口径沿修订纪律）。
- **qa/09-bt-engine-qa.md:70 `ttp]`→`[http]`**：1 处历史真缺陷待修，建议归下一 QA 轮顺带（与本轮范围无关）。
- **runner 可执行化**：suite_http_turbo.py（QA-HT 全用例）+ 六份既有 runner 的受影响断言同步——归 QA 轮。
- **产品端实现**：FR-05-01–13 全量归 six-pack/coder 轮（设计权威 ezr-http-dw-tunning.md + phase-05.md）。

### 移交建议
- 下一步 `bin/swarm begin six-pack/coder`：按 phase-05.md FR-05-01–13 实现（建议切片：P0 补丁层 → P1 planner/启动收尾 → P2 writer/稀疏/sync；scheduler 开关与 blocks 回退保绿贯穿）。
- 本轮规格含跨热加载稳定断言（13b/快照）与 fixture 新能力清单（qa/13-http-turbo-qa.md 环境前置）——coder 实现与 QA runner 落地时先读这两处。
- 沿袭受限项（>100 变异点存量 / CRAP 100 / CPD 222）如需收敛，由后续 architect 轮裁决后按既有口径拆分。

By specifier.
