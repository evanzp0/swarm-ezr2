# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier　会话：v128-spec-url-authority（任务 air2-url-authority-design）
> 本轮（操作者指令：URL 权威模型改版，基本参考 AIR2 (aria2) 的原始 URL 与跳转 URL 设计）：
> 全程以「原始输入 URL」为任务级唯一权威，跳转后 URL 降格为每条连接内部的「当前值」；
> sidecar 不再存储任何 URL；并发 URL 来源 = 各连接独立从原始 URL 跟跳转；失败重试回滚
> 原始 URL 重新跟跳转。规格修订 v1.28 落账（mission + phase-01 + 3 份 feature + 3 份 QA 套件），
> gherkin-parser / ir-dry-checker 实跑通过。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.28，分期模式）+ `project/mission/phase-01.md`。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（v115 前历轮） | mission/phase-01.md 全量 FR/D 裁决表 + features/ 13 份 + qa/ 13 份 | 全部交付 | parser/dry-check 全过 |
| coder（v120–v124 两批 + v1.18 修订） | FR-01-102/103/104/105/51/6 系列 + sidecar 即时落盘 + 清码 retry 直判 | 已交付 | 各批 2438→2506P/0F、clippy 0 |
| cleaner（v125/v129/v132 三轮） | 保持行为清理 + 四度量基线首登 | 已交付 | 2506→2535P/0F |
| architect（v126/v134 两轮） | 四阶段架构评审 + throttle 策略提取 + 属性测试 60 + arch 14 规则 | 已交付 | 2523→2546P/0F |
| hardender（v127-hard / v137-hard） | 差分变异加固 + 三度量基线（CPD 52 / 94.36% / 门禁 0） | 已交付 | 基线全绿 |
| coder（v128）+ 会话外登记（v1.19/v1.20） | CPD 处置 + R 恢复路径退役 + 任务 ID 随机化 | 已交付 | 2535→2578P/0F |
| architect（v135/v136 三段 = v1.21–v1.23） | FailedPaused 退役（8 态）+ 恢复一律重新排队 + 规格三件套同步 | 已交付 | 产品 2597P/0F |
| QA（v138-qa / v140-qa / v141-qa） | e2e 同步 + 桩扩展 + TP 收口（S→P ×3） | 已交付 | e2e 164 条 164P/0F/0S；产品 2775P/0F |
| coder（v139-fix） | 分块计数倒退缺陷修复（apply_chunk_snapshot 两段合并） | 已交付 | 2767P/0F |
| coder（v142-fix） | **幽灵 supervisor 集群根因修复**：探测期命令消费 + 补发去重 + 失败去重守卫 + 幽灵进度门，4 处缺陷联动 + 4 个回归测试 | 已交付 | 2805P/0F、clippy 0、fmt 0、arch 14 过 |
| coder（v143-fix） | **单流降级路径断点保全**：失败/暂停臂折叠进度落账 + 恢复 Range 续传（单块+206）+ 无 Range 有进度暂态保留 + 缺 CL 探测不作废，4 处修复 + 6 个回归测试 | 已交付 | 2853P/0F、clippy 0、fmt 0、arch 14 过 |
| coder（v144-fix） | **探测/请求臂失败弃账修复**：探测网络错误臂、探测 HTTP 状态错误臂、预分配致命错 ×2 臂、单流续传请求错误 ×2 臂 + 单流 open 致命错臂——7 处失败回报全部携带台账持久化状态 + 3 个回归测试 | 已交付 | **产品 2877P/0F、clippy 0、fmt 0、arch 14 过** |
| **specifier（v128-spec，本轮）** | **v1.28 URL 权威模型改版规格落账** + v1.24–v1.27 版本注记补记（上游交接显式请求） | 已交付 | **parser 3/3 实跑过、dry-check 0 增量错误级** |

### 遗留受限项（当前有效）
- **软 Gherkin 验收变异受限（沿袭 v127/v137/v138/v140/v141 口径）**：13+1 份 feature 的
  parser + ir-dry-checker 本轮实跑全过（bb 1.12.197 + APS 仓库本轮已安装至 `.tools/`，随
  会话不入档）；`gherkin-mutator` 全量变异 campaign 仍不可在本环境前台窗口预算内完成。
  01-persistence-config.feature 的 mutation-stamp（sha256=57e757…）维持保留处置（内容零变更）。
- **三度量未在本轮复跑（沿袭 QA 轮口径）**：覆盖率/CPD/CRAP 基线沿袭 v138
  （94.37% / 52 tiles / 门禁 0）；本轮为纯规格修订、产品源码零触碰，量级未触发重测门槛。
- **工具链位置（沿袭 v143 登记）**：rustup nightly 1.101.0-nightly（rust-toolchain.toml
  钉定自动装，新 shell 须 `export PATH="$HOME/.cargo/bin:$PATH"`；rustup/crates 均走
  rsproxy.cn 镜像，见 packs/_common/notes/rust.md 沉淀）。本轮新增：bb + APS 位于
  `.tools/`（gitignore 排除、不入档），续跑会话需重建或复用本机缓存。
- 无其他受限项。

### 实现定义值登记（当前有效）
- 沿袭有效：D28/D29/D30/D31/D32/D33/D34、v1.21–v1.27 各条口径（v1.24–v1.27 已于本轮以
  台账口径补记入 mission/phase-01 版本注记）、v138 R 臂重锚口径、v140 桩口径与 TD-17
  采样口径。
- **本轮新增（v1.28，操作者指令直判；已入 mission/phase-01 注记）**：
  - **URL 权威模型（AIR2 口径）**：原始输入 URL = 任务级唯一 URL 权威；跳转落点 URL 仅为
    每条连接/请求内部的「当前值」——不持久化（任务模型/注册表/sidecar 三层均不再持有
    final_url；旧档多余键反序列化忽略，零迁移）、不跨连接共享（原「探测捕获落点共享
    worker 池」口径退役）、不作一致性比较对象（FR-01-22 收敛为 ETag/Last-Modified/大小
    三要素）；每条请求链重定向上限 10 次不变（超限仍「重定向次数超限」停等）。
  - **FR-01-26 匹配路径基**：保存目录 + 推导同名 sidecar 即接续（无同 URL 校验门）；异 URL
    同推导名的误接续由 FR-01-22 一致性兜底（探测后三要素不符 → 作废从头下 + toast），
    规格 01-resume-sidecar-14。
  - **D27 收敛**：详情「URL」展示与点击复制 = 原始 URL 单值（「复制所见值」锚定不变）。
- 无未决待批项（本轮三条后果性推论①②③已随 v1.28 注记落账，操作者如对任一推论改判，
  改动面见「各角色合并吸收清单」）。

## 二、当前产出情况（specifier v128-spec）

### 本会话产物清单（规格 8 文件，产品源码零触碰）
**`project/mission.md`**
- 版本注记 v1.24–v1.27 补记（一行式，台账口径）+ v1.28 URL 权威模型改版注记（六点口径 +
  规格/QA 同步清单）。

**`project/mission/phase-01.md`**
- 头部注记：v1.24–v1.27 补记块 + v1.28 修订块。
- FR-01-02：文件名推导第二优先级述语改「探测请求跳转落点 URL 路径末段（该次连接的当前值，
  不持久化）」，可观察行为不变。
- FR-01-14 **重写**（五款：①原始 URL 唯一权威；②各连接独立跟跳转、上限 10 次/请求链、
  落点不缓存不复用——原「探测捕获落点共享 worker 池」口径退役；③失败重试回滚原始 URL
  重新跟跳转；④任务级 final_url 状态与注册表持久化字段退役（旧档忽略零迁移）；⑤「最终
  URL 参与续传一致性比较」退役）。
- FR-01-20 修订：sidecar 内容去「原始/最终 URL」两键——**sidecar 不存储任何 URL**（AIR2
  口径：控制文件根本不存 URL；持久化 URL 仅任务注册表中的原始 URL，FR-01-70 承载）；旧
  sidecar url/final_url 键按多余字段忽略（零迁移，断点仍可续传）。
- FR-01-22 修订：续传一致性比较收敛为 **ETag / Last-Modified / 文件大小** 三要素（URL 腿
  退役）；其余口径不变（任一变化或不再返回这些头 → 失效；3 次防循环；不计失败）。
- FR-01-26 修订：断点接续匹配改**路径基**（保存目录 + 推导同名 sidecar 即接续，无「同
  URL」校验门；AIR2 口径：控制文件按路径关联）；误接续由 FR-01-22 兜底（场景 14）；任务
  列表内同 URL 同目录非完成任务的同名接续判定不变。
- D27 修订：详情「URL」展示与复制收敛为原始 URL 单值（最终 URL 优先臂退役）；「校验」热区
  复制完整校验码值不变。

**`project/features/01-download-engine.feature`**
- 场景 08 重写：「重定向跟随完成下载（原始 URL 唯一权威，各连接独立跟跳转）」——Then 断言
  详情 URL 显示**原始 URL**（原「详情 URL 显示最终 URL」退役）+ fixture 记录的每个分块请求
  均以原始 URL 为起点独立跟跳转（非共享探测落点）；示例表列名 final_url → landing_url。
- 头部注记 v1.24→v1.28 口径说明。场景清单注释同步。

**`project/features/01-resume-sidecar.feature`**
- 场景 01：sidecar 内容清单去 URL + 新增「sidecar 不含任何 URL 字段」断言。
- 场景 07：一致性失效示例行删「最终 URL」行（四要素行 → 三要素行）。
- **场景 14 新增**：「重加同路径异 URL 任务由一致性检查兜底」——路径基接续沿用原名与
  sidecar → 探测后三要素不符 → 「服务器内容已更新，已从头重新下载」toast、sidecar 作废
  从头重下、不计失败。场景清单注释同步（13→14 场景）。

**`project/features/01-add-task.feature`**
- 场景 08/09 Given 述语对齐（「最终 URL 路径末段」→「探测请求跳转落点 URL 路径末段」），
  示例值零变更。

**QA 套件（3 份）**
- `qa/01-download-engine-qa.md` QA-DE-08：通过判据改「详情 URL 为原始 URL + 请求日志显示
  每个分块请求均从原始 URL 独立发起并跟跳转」。
- `qa/01-resume-sidecar-qa.md`：规格指针 13→14 场景；QA-RS-01 判据改「不含任何 URL 字段」；
  QA-RS-07 变量行删「最终 URL」；新增 QA-RS-14；环境前置补 fixture 能力「异路径同推导名
  文件（files/alias.bin 与 other/alias.bin 内容不同）」。
- `qa/01-add-task-qa.md` QA-01-08：述语对齐。

**零改动核对（本轮确认无需触碰）**：`features/01-tui-display.feature`（03 仅列字段名
「URL」，无落点语义）、`features/04-ui-conns.feature` + `qa/04-ui-conns-qa.md`（QA-UC-18
「剪贴板=详情展示 URL」语义锚定不变——展示值定义经 D27 v1.28 收敛为原始 URL，非重定向
示例值不变）、`qa/runners/`（QA 角色可执行层，见移交）。

### 验证证据
| 验证项 | 结果 | 命令 |
|---|---|---|
| gherkin-parser | 3/3 修改 feature 实跑通过（IR 生成成功） | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-parser <feature> .work/tmp/<stem>.json)` |
| gherkin-ir-dry-checker | 3/3 通过——**0 增量错误级 findings** | `bb gherkin-ir-dry-checker <ir> <out>` |
| dry-check 增量对账（aps.md 口径） | 01-download-engine 104→113、01-resume-sidecar 64→79、01-add-task 118→117（advisory 全量；增量 = 场景 08/14 新步骤与既有相似臂的 near-duplicate/synonym，逐条人工复核为**有意语义差异**——不同守卫臂/不同文案，按 aps.md「语义性差异即通过」判定） | git HEAD 版本预跑基线 vs 现版对比 |
| 结构自查 | 3 文件 Feature/Scenario/Examples 计数正确、示例表列数一致 | python 结构扫描 |
| 产品测试 | 未触碰（纯规格轮，沿袭 v144-fix 基线 2877P/0F） | —（下游 coder 轮按其四要素复跑） |

### 对账口径（基线 → 终态）
- 规格修订 8 文件（git status 口径）：mission.md +16 行、phase-01.md +29/−6 行、
  01-download-engine.feature +14/−5 行、01-resume-sidecar.feature +20/−3 行（含新场景 14）、
  01-add-task.feature +2/−2 行、qa 三份 +4/−3 行。features 场景总数 13 份 → 14 份文件内
  场景计数：download-engine 16（不变）、resume-sidecar 13→14（+1 新增）、add-task 18（不变）。
- 产品源码 / e2e runner / 配置模板：零 diff。

### 各角色合并吸收清单（specifier 移交）
- **coder**：v1.28 实现触碰面（参考，实现细节归 coder）——①`supervisor.rs`：删除
  `worker_url` 共享（451/815 行 `resp.url()` 捕获），worker/单流续传请求改从 `spec.url`
  （原始 URL）发起（reqwest client 既有 `Policy::limited(10)` 天然按请求链跟跳转，无需
  新机制）；②任务模型 `Task.final_url` 退役（task.rs 40 行）+ `evt.rs` Probed 事件
  final_url 字段退役 + `engine/mod.rs` `keep_name: t.final_url.is_some()` 需换「首次探测
  已定名」标记（行为不变：任务名首次探测后定稿）；③`sidecar.rs`：`url`/`final_url` 字段
  退役（serde 反序列化对旧档多余键默认忽略，版本号可维持 1——沿袭 has_error 退役先例）；
  ④`consistency.rs`：`ServerStamp.final_url` 退役、`same_url` 腿删除、MissingStamp 条件
  改 `stored.size.is_none()`；⑤`registry.rs`：任务快照 final_url 键退役（旧档忽略零迁移）；
  ⑥`ui/detail.rs` 219 行 + `app/mouse.rs` 104 行：展示/复制改 `t.url`；⑦`dialogs.rs`/
  `cli.rs` FR-01-26：删 `sc.url == url` 校验，路径基匹配。回归建议：worker 独立跟跳转
  （fixture 记录 origin 命中数 = 请求数）、旧 sidecar（含 url 键）续传、异 URL 同名接续
  兜底（场景 14）。
- **cleaner / architect / hardender**：v1.28 落地后按各自门槛对触碰面差分复测；一致性
  三要素化后 `consistency.rs` 为主要缩面（URL 腿删除）。
- **QA**：`qa/runners/suite_resume_sidecar.py` QA-RS-07 的「最终 URL 变更」变体
  （go4.bin 经 302 → m4.bin，`mut.redirect_to` 热变更）在 v1.28 下 URL 腿不再参与判定——
  该变体若变更后 ETag/LM/大小均未变将不再触发失效，需按三要素口径改造变体（如配合
  swapsize 保证至少一要素变化）或改判为「URL 变更但三要素不变 → 不失效」正向用例
  （AIR2 语义下的预期行为，建议 QA 轮裁决）；QA-RS-14 需新 runner 场景（fixture 需
  files/alias.bin 与 other/alias.bin 异内容文件对）；suite_download_engine.py 重定向用例
  需补「各分块请求 origin 命中」断言。
- **操作者**：v1.28 三条后果性推论（一致性三要素化 / 路径基匹配 + 兜底 / 详情与复制收敛
  原始 URL）已随注记落账；如对任一改判，改动面限定于对应 FR 行 + 场景 07/14 + D27。

### 待办与待批（未决项）
- 无阻塞项。
- 制度性修订建议（按宪法第三章不自行改动，提请操作者裁量）：建议 `packs/six-pack/
  specifier/SKILL.md`「定稿文档视同已收齐答案」条款增补一句——合成时须把定稿指令的
  **强制后果推论**（指令未言明、但由既有条款被迫联动的口径，如本次「一致性去 URL 腿」）
  逐条显式登记（推论依据 + 行为差 + 改判改动面），随合成一并呈报，不作为独立待批阻塞项。
- 沿袭移交（非缺陷）：gherkin-mutator 运行器基建（见遗留受限项）。

### 移交建议
- 下一步建议 `bin/swarm begin six-pack/coder`（v1.28 实现轮——触碰面清单见上；TDD 先红
  后绿，regression 锚点 = 01-download-engine-08 / 01-resume-sidecar-01/07/14 /
  01-add-task-08/09）。
- 实现合入后如需收口，可续 `six-pack/QA`（e2e runner 按 QA 移交项改造 + 复跑）。
