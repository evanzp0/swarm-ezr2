# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：v144-fix（任务 fix-probe-failure-breakpoint-view）
> 本轮（操作者三次报告缺陷修复轮）：修复操作者实证的「磁盘 .ezr 台账 2 块全 + 1 部分
> 块（downloaded=5884570）而任务详情『分块』显示 0/11」。**单一根因：探测/请求臂失败
> 弃账上报 0/0**——带台账任务的探测阶段网络错误（连接超时/拒连，重试每次尝试先探测）
> 与 HTTP 状态错误臂、单流续传 Range 请求错误臂、预分配致命错臂均以 `sidecar=None`
> 收尾，失败事件 (downloaded=0, chunk_done=0)；App 事件权威口径用 0 覆盖分块视图
> （字节因 >0 守卫保留 → 「已下载 5.6 MB / 分块 0/11」自相矛盾），并随注册表持久化。
> v142/v143 修复保持有效，产品测试 2877P/0F 全绿。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.23，分期模式）+ `project/mission/phase-01.md`。

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
| **coder（v144-fix，本轮）** | **探测/请求臂失败弃账修复**：探测网络错误臂、探测 HTTP 状态错误臂、预分配致命错 ×2 臂、单流续传请求错误 ×2 臂 + 单流 open 致命错臂——7 处失败回报全部携带台账持久化状态 + 3 个回归测试 | 已交付 | **产品 2877P/0F、clippy 0、fmt 0、arch 14 过** |

### 遗留受限项（当前有效）
- **软 Gherkin 验收变异受限（沿袭 v127/v137/v138/v140/v141 口径）**：13 份 feature 的
  parser + ir-dry-check 实跑全过；`gherkin-mutator` 运行器适配器与验收入口生成
  器为会话级工具不随工作区流转，全量变异 campaign 不可在本环境前台窗口预算内
  完成。01-persistence-config.feature 的 mutation-stamp（sha256=57e757…）维持
  保留处置（内容零变更；复跑指令见 v138 交接四要素③口径）。
- **三度量未在本轮复跑（沿袭 QA 轮口径）**：覆盖率/CPD/CRAP 基线沿袭 v138
  （94.37% / 52 tiles / 门禁 0）；本轮产品变更 1 文件（见下），量级未触发
  重测门槛，hardender 轮可按需复测。
- **工具链位置（沿袭 v143 登记）**：rustup nightly 1.101.0-nightly（rust-toolchain.toml
  钉定自动装，新 shell 须 `export PATH="$HOME/.cargo/bin:$PATH"`；rustup/crates 均走
  rsproxy.cn 镜像，见 packs/_common/notes/rust.md 沉淀）。
- 无其他受限项。

### 实现定义值登记（当前有效）
- 沿袭有效：D28/D29/D30/D31/D32/D33/D34、v1.21–v1.23 恢复 toast 文案、v138 R 臂
  重锚口径、v1.24 分块断点视图两段合并、v140 桩口径与 TD-17 采样口径、v1.25 四条
  口径（见 v142 交接）、v1.26 四条口径（缺 CL 守卫/恢复臂 A/B/三臂折叠，见 v143 交接）。
- **本轮新增（v1.27，操作者三次 bug 报告直判；mission 待 specifier 同步）**：
  - **失败回报携带持久化事实（核验与上报分离）**：凡磁盘断点台账在握的失败点
    （探测请求网络错误/HTTP 状态错误、单流续传 Range 请求网络错误/状态错误、
    预分配与 open 致命错），失败事件必须携带台账的 downloaded/chunk_done（多块 =
    原账原样；单流 = `failed_ledger` 折叠块内进度），不得弃账上报 0/0；未核验 ≠
    断点消失，一致性裁决（续传/作废）留给下一次成功探测。FR-01-22 作废语义零变更
    （确变仍作废；v115 unknown_size_resume_restarts 锁定口径不变——size=0 未知大小
    台账维持作废）；FR-01-41 重试决策零变更（made_progress 口径不变）。
- 无未决待批项。

## 二、当前产出情况（coder v144-fix）

### 本会话产物清单（产品树 1 文件 + 测试 3 个）
**`project/ezr/src/engine/supervisor.rs`**（唯一产品源触碰面）
- `download` 探测循环网络错误臂（根因主臂——重试每次尝试先探测，连接超时/拒连
  全部命中此处）：`Flow::Failed(..., None)` → `spec.sidecar.clone().zip(
  spec.sidecar_path.clone())`——spec 携带台账（App 每次 Start 从磁盘装载，单源）
  原样随失败事件回报并重存（内容零变更）。
- `download` 探测 HTTP 状态错误臂（503/404 等非 2xx）：同口径回报台账。
- `download` 预分配致命错 ×2 臂（set_len 失败/目标文件创建失败）：以一致性过滤后的
  `sc` 原样回报（致命错不抹断点视图）。
- `single_stream` 续传请求臂 ×2（Range 续传请求本身网络错误/状态错误）：以
  `failed_ledger(spec, stamp, &blocks, multi_ok, resumed, sc_path)` 折叠块内进度回报
  （resume 判据保证 multi_ok ∧ resumed>0 → 必落账）；单流 open 致命错臂同口径。
- mock_server 回归门控新增：`dropseq=N`（第 N 个连接不回任何响应直接断开——构造
  「探测成功、续传 Range 请求即断」的请求臂失败形态）。
- 回归测试 ×3（TDD 先红后绿，红灯 `left: Some((0, 0))` 与操作者报告逐字对应）：
  `probe_conn_error_reports_persisted_breakpoint`（拒连探测 + 3 块台账 2 全 1 部分 →
  事件必须 (2297152, 2)）；`probe_http_error_reports_persisted_breakpoint`（503 探测
  同口径）；`single_stream_resume_request_error_keeps_ledger`（单块续传请求断连 →
  事件必须携带台账字节 200000）。三测均另断言 .ezr 台账保留。

**工程沉淀（packs/_common/）**
- `engineering.md` 新增：「失败回报必须携带持久化事实——事件权威口径下上报 0/0
  等于替用户清零」通用纪律（核验与上报分离 + `Flow::Failed(..., None)` 逐臂遍历法
  + 验证锚点）。
- `notes/rust.md` 新增：按连接序号门控 mock（dropseq=N）造「请求臂失败」形态。

**规格**
- features/ 与 mission 零改动（git 对账）；本轮为缺陷修复。FR-01-22/41/13 语义零变更；
  v142 幽灵 supervisor 修复、v143 单流断点保全全部保持有效（11 个既有回归全绿）。

### 验证证据
| 验证项 | 结果 | 命令 |
|---|---|---|
| 产品测试全量 | **2877P / 0F**（v143 基线 2853P + 本轮 3 个新回归，零失败） | `cargo test` |
| 新增回归（TDD 先红后绿） | 3/3 绿（修复前 3/3 红，`left: Some((0, 0))` 复现报告形态） | `cargo test -- probe_conn_error probe_http_error single_stream_resume_request` |
| 既有锁定回归保持 | v142 四测 + v143 六测 + unknown_size_resume_restarts / tampered_block_count_restarts_fresh / probe_missing_headers_invalidates 等规约锁定用例全绿 | 全量套件 |
| clippy | **0 警告** | `cargo clippy --all-targets` |
| fmt | 0 diff | `cargo fmt --check` |
| arch | 14 规则全过 | `bash scripts/arch_check.sh` |

### 对账口径（基线 → 终态）
- 产品测试：v143 基线 2853P/0F → 本轮 **2877P/0F**（+3 本轮新回归；其余为各测试目标
  计数的自然汇总口径差，逐目标全绿零失败）。
- 产品源码变更 = **1 文件**（engine/supervisor.rs），全部限「失败回报弃账」缺陷面
  （7 臂 + 1 门控）；传输/落账/收尾路径、App 层、UI 层零触碰。
- e2e 套件零触碰（qa/runners/ 零 diff）；features/ 零改动。

### 交接四要素（coder）
1. **本地验证命令清单**（按序）：
   环境：`export PATH="$HOME/.cargo/bin:$PATH"`（rustup nightly 自动装；离线/慢网
   先配 rsproxy 镜像，见 notes/rust.md）→ cwd `project/ezr`：
   `cargo test`（2877P/0F）→ `cargo clippy --all-targets`（0）→ `cargo fmt --check`（0）→
   `bash scripts/arch_check.sh`（14 过）。
2. **会话运行时产物**（`.work/tmp/`，`complete` 时清除）：无失败现场残留（本轮全绿）。
3. **规格与 stamp**：features/ 零改动；01-persistence-config.feature stamp 维持
   保留（v138 处置），gherkin-mutator 恢复后按四要素③删除重跑。
4. **等价突变体清单及失效条件**：沿袭 v137–v143 登记；本轮新增分支（7 个失败臂的
   台账携带）均有行为锁（3 个新回归逐臂覆盖主形态），未发现新等价突变体。

### 待办与待批（未决项）
- 无新增未决待批项。
- 沿袭移交（非缺陷）：gherkin-mutator 运行器基建（见 v138 交接遗留受限项）。
- 建议（specifier 同步项，非阻塞）：v1.27 口径已以代码注释 + 本交接承载；
  mission.md/phase-01.md 的版本注记由 specifier 下一轮按其建档职责补记
  （coder 依宪法不触碰 mission）。

### 各角色合并吸收清单（coder 移交）
- **specifier**：v1.27 口径见「实现定义值登记」；如需升格为 FR/D 裁决项（「失败事件
  必须携带持久化断点状态」），按建档流程补写。
- **cleaner**：探测两臂的 `spec.sidecar.clone().zip(spec.sidecar_path.clone())` 与
  预分配两臂的 `sc.clone().map(...)` 形态相近（CPD 复测可裁决是否提公共 helper）；
  mock_server 门控已达 7 个（status/dropseq/norange/noclen/drophalf/droponce/slow/hold），
  可评估按门控家族收敛。
- **architect**：本轮把「失败回报的口径」收敛为不变式——凡磁盘台账在握的失败臂必须
  回报台账；`run()` Failed 臂的 `Some→save + 汇总事件` 单点收尾已天然承载该不变式，
  后续新增失败臂只需遵守「在握必报」纪律（engineering.md 已沉淀）。
- **hardender**：3 个新回归为行为锁（探测拒连/503/续传请求断连）；变异靶面新增
  7 个失败臂的第三参数（None ↔ 台账），v1.27 触碰面可按需差分加固。
- **QA**：e2e 套件零触碰沿袭有效；01-resume-sidecar.feature 场景族对「失败后详情
  分块/字节视图一致」的断言本轮起在多块路径同样成立。

### 移交建议
- 操作者可直接 `bin/swarm complete` 归档交付（本轮验证证据已闭环）。
- 如需继续收口（可选）：`bin/swarm begin six-pack/cleaner`（失败臂 helper 形态对账）
  或 `bin/swarm begin six-pack/hardender`（v1.27 触碰面差分加固）。
