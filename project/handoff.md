# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（本轮已完成）——**下游 coder 待操作者显式启动**
> 路径：specifier → coder（2026-11-09 上午，ws/webrtc 轮）⇒ 诊断/复验/bug28 轮（2026-11-09 下午）⇒ **specifier 修订轮（本轮，2026-11-09 下午）** ⇒ cleaner → architect → QA（待操作者显式 begin）
> 会话：ezr-specifier-filesel-20261109（任务 = 操作者三需求：①已完成的 BT 任务（做种中）勾选此前未下载的文件/目录 → 重算完成度（<100%）→ 状态改回「下载中」继续下载；②下载中或已完成的 BT 任务取消文件/目录勾选 → 确认后删除该文件/目录；③下载中的 BT 任务勾选此前未下载的文件/目录 → 确认后被下载。上游交接 = project/handoff.md bug28 轮版）
> 环境：bb 1.12.197 + APS 仓库经 gh-proxy.com 前缀装于 `.tools/`（工具缓存不进归档）；RAG 索引 `.work/rag` 已随产出刷新

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，v1.46）+ `mission/` 4 份 phase 详述（phase-02 v1.16 为本轮修订依据）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder 历轮（含 2026-11-09 ws/webrtc 轮 + bug28 轮） | ezr 产品实现 + 12 集成测试目标 + bin 单测；bug6–bug28 修复；FR-02-21/31/33/08 v1.15 + FR-02-34/35 全量实现；ezr-demo 同构同步 | 已交付 | bin 983P + 集成 12 目标 6761P 全绿；clippy 0 警告；fmt 干净 |
| cleaner 历轮 | 保持行为清理 + 四件度量 | 已交付 | 全绿 |
| architect（2026-11-07） | 四阶段评审 + arch_check 17 规则 + 属性测试 76P | 已交付 | bin 951P / 集成 6357P / CPD 181 |
| hardender | —（操作者 20261109 裁决定为 six-pack 可选节点，默认链不含） | 不适用 | — |
| QA + 终验（2026-11-06/07） | 14 套件 187P 实跑 + harness 自适应化 | 已交付 | 全绿 |
| 诊断轮（2026-11-09 下午） | mini_ws_tracker 修复（响应补 action/info_hash + offer/answer 顶层即时推送）；Sintel 真实 swarm 129.3MB ×2 全成；三方冒烟（webtorrent-cli）纯 WebRTC 6MB/38s 全过 | 已交付 | ezr 源码零改动 |
| **specifier（本轮 2026-11-09）** | phase-02 v1.16（FR-02-03/19 v1.16 修订 + D73）+ mission.md v1.46 + features 2 份修订 + qa 2 份同步 | 已交付 | APS parser+dry-check 2/2 exit=0 |

### 遗留受限项（当前有效）
- **leech 判据无 e2e 自动化**（six-pack 链长期受限项，历轮维持）。
- **变异实跑未执行**（hardender 可选节点；`--file` 分块口径悬置转政策性关闭）。
- **QA 规程 6 个 BT/命令窗口族无专属 runner**（harness/命令链路承载，历轮既定形态）。
- **CRAP 观察项**：`ezr_main` 与 `on_mouse_trackers` bin 套件 0 覆盖（承载面差异登记）。
- **ws(s) tracker announce 无代理面**——操作者 20261109 chat 裁决不补齐，长期已知边界关闭。
- **ezr-demo 守卫脚本先在 FAIL**（A2-A5/B0/M3/P10/R1-3 等，stash 基线对照证实与 ws/webrtc 轮无关）——沿袭移交 cleaner 裁决修否。
- **本轮规格修订的实现缺口（待 coder 轮落实，现状探针实证）**：①App 层任务状态无「做种中→下载中」回退事件（引擎 Reconfigure 臂 `seeding=false` 已在，缺 App 侧状态翻转）；②取消勾选不删除磁盘数据（Reconfigure 仅重建 pmap）；③上传暂停与 seed_left 重置未实现；④`files_handles` 对新取消选中的文件句柄不关闭。

### 实现定义值登记（累计，当前有效口径）
- **D44–D72 沿历轮登记不变**（全量清单见 phase-01/phase-02 D 值表；D71 ws/webrtc 八项、D72 bug28 五项均已实现并验证）。
- **新增 D73（本轮文件勾选修订轮，操作者 chat 选择题答案 1A 2B 3B 4A 5A 即裁决，随合成落账）**：①「状态改回下载中」判据 = 确认重算后存在未完成缺口即由「做种中」即时翻转「下载中」（App 层状态即时改，不等下一进度帧；实现载体归 coder）；②上传暂停语义 = 自确认回退起至再次全部完成止该任务零上传（上传速度 0/uploaded/并发行累计/announce uploaded 均不增长、leecher 拉不到数据；实现载体 choke/断开归实现定义；期间暂停/恢复不改变该语义；**仅作用于做种回退下载态，正常下载任务互惠上传 D58 不受影响**）；③seed_left 重置 = 再次转做种时重置为 config `seed_duration_secs` 当前值，原剩余值作废；④删除 IO 失败不阻塞确认流程、不改变勾选生效、剩余目标照常删除（与静默跳过同族，失败不 toast 不阻断）；⑤toast 沿既有 `✓ BT 任务已更新…（立即生效）`；再次完成 toast `✓ 下载完成: <name>` 照常出现；⑥已暂停任务同口径（1A+需求②对话框统一语义派生，操作者 2026-11-09 chat「D73⑥ 批准」裁定生效）：确认时立即删除、勾选新文件不唤醒任务（恢复后补下）；⑦目录勾选取消整删 = 逐文件删除后变空目录自底向上清理的复合效果（同一实现路径）；⑧Inversion 受限退化沿袭（chat 选择题面板不可用，1A 2B 3B 4A 5A 一轮收齐，无遗留歧义未触发阶段 2）。

## 二、当前产出情况

### 本会话产物清单（规格修订轮）
1. **`mission.md` v1.46**：大纲注记（三需求 + 答案记录 + D73 指针）。
2. **`mission/phase-02.md` v1.16**：v1.16 修订注记；**FR-02-03 v1.16**（做种中任务文件勾选修订臂：total/完成度重算 → 状态即时回「下载中」→ 上传暂停至再次全部完成 → 回做种 seed_left 重置 + 再次完成 toast）；**FR-02-19 v1.16**（取消选中 = 落盘数据删除钦定落实 + 细则：文件粒度删除+变空目录自底向上整树清理/不存在静默跳过/IO 失败不阻塞/勾回从头补下——原「数据保留断点续传」语义退役；已暂停任务同口径）；**新增 D73**；验收期望 3 更新。
3. **`features/08-modify-bt.feature` v1.10**：场景 03 反转修订（取消勾选删除 + 勾回从头补下）；**场景 10 新增**（做种中勾新文件回「下载中」，需求①全链判据）、**场景 11 新增**（做种中取消勾选已完成文件：删除+保持做种中，需求②做种形态）、**场景 12 新增**（已暂停任务同口径，D73⑥）。
4. **`features/09-bt-engine.feature` v1.43**：场景 08 修订（引擎侧：取消选中落盘删除 + 变空目录清理 + 勾回从头补下 + 做种回退臂上传暂停/seed_left 重置；需求③补选补下既有语义维持）。
5. **`qa/08-modify-bt-qa.md`**：v1.8 同步——QA-MB-03 修订（删除语义）、**QA-MB-10/11/12 新增**（对位场景 10/11/12）。
6. **`qa/09-bt-engine-qa.md`**：v1.43 注记 + QA-BE-08 修订（删除 + 做种回退臂判据；UI 层严格检查归口 QA-MB-03/10/11/12）。
7. **`notes/environment.md`**：追记 gh-proxy 前缀对 bb release 资产可用复测（合并入当日既有条目，去重）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| APS parser（08-modify-bt） | exit=0，12 场景 | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-parser project/features/08-modify-bt.feature .work/tmp/08-modify-bt.json)` |
| APS dry-check（08-modify-bt） | exit=0（near-duplicate findings 89 信息级，与基线同形态） | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-ir-dry-checker .work/tmp/08-modify-bt.json .work/tmp/08-modify-bt.dry.json)` |
| APS parser（09-bt-engine） | exit=0，20 场景 | 同上换文件名 |
| APS dry-check（09-bt-engine） | exit=0（72 信息级） | 同上 |
| RAG 索引 | 180 文件 / 1486 分块已刷新 | `bin/rag/rag index project --types rs,md,feature` |

### 对账口径
- features：08-modify-bt 场景 9→12（+10/11/12，既有 01–09 编号不变）、09-bt-engine 场景 20 不变（08 修订）。
- QA：QA-MB 9→12 用例、QA-BE-08 单行修订（用例数不变）。
- mission：mission.md v1.45→v1.46、phase-02 v1.15→v1.16；D 值表 +D73。

### 待办与待批
- **D73 随合成落账生效**（操作者 chat 答案即裁决）；其中 **D73⑥（已暂停任务同口径）为派生裁决，已获操作者显式批准（2026-11-09 chat「D73⑥ 批准」），回退窗口关闭**——即使有异议也不再影响场景 12/QA-MB-12。操作者并确认本轮收尾摘要准确（「A. 准确」），D73⑧（受限退化沿袭）随摘要确认无异议，一并生效。
- **移交 coder（本轮实现范围）**：①FR-02-03 v1.16 做种回退臂——App 层「做种中→下载中」状态回退事件（确认重算存在缺口即翻转）、上传暂停（可观察面见 D73②）、回做种 seed_left 重置（D73③）；②FR-02-19 v1.16 磁盘删除——文件粒度删除 + 变空目录自底向上整树清理 + 不存在静默跳过 + IO 失败不阻塞 + `files_handles` 取消选中句柄关闭；③下载中补下沿既有 Reconfigure 实现核验（pmap 重建/开句柄/fill_peer_slots/reannounce_notify 已在）；④测试翻转面：08-modify-bt-03 旧「断点保留」语义的既有断言（`bt_dialog.rs` 等处）随语义反转同步翻转。
- **沿袭移交 cleaner**：新代码度量四件与保持行为清理；ezr-demo 守卫先在 FAIL 修否。

### 移交建议
- 下一步运行 **six-pack/coder**（以本文件为上游交接内容）；之后沿 six-pack 链 cleaner → architect → QA。
- 操作者审批已闭环：D73⑥ 显式批准、D73⑧ 随摘要确认生效，无待批项——可直接 `bin/swarm begin six-pack/coder`（以本文件为上游交接内容）。
