# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（v1.51 调整轮已完成，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 修订实现轮 ⇒ coder 补缺口轮 ⇒ cleaner 收敛轮 ⇒ architect 收敛轮 ⇒ QA 终验轮 ⇒ specifier 显示指标轮（2026-11-10）⇒ specifier 调整轮 v1.48/v1.49/v1.50（2026-11-11）⇒ **specifier 调整轮（本轮 v1.51，2026-11-11）** ⇒（操作者显式指令：本轮由 specifier 会话直接落地 ezr-demo 修改）
> 会话：six-pack-specifier-20261111（任务 = tab-switch-state-preserve；上游交接 = project/handoff.md v1.50 版 + 操作者 chat 指令「下载任务列表：从一个 tab 切换到另一个 tab，然后又切换回原来的 tab，原下载任务列表的滚动条不在原先位置，并且切回后的选中的任务也不是原来选中的任务了。应该是 tab 切换时，需要保留原有列表控件的状态（包括滚动条位置以及选中的明细等），切回原列表时，根据之前保留的状态尽可能进行还原」）
> 环境：stable 1.99.1（rustup 1.29.x 前台装齐）+ cargo registry rsproxy sparse + bb 1.13.229 / APS（.tools，工具缓存不进归档）

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.51**）+ `mission/` 两份 phase 详述（**phase-01 v1.50** 为当前页签交互与 FR-01-117 依据、**phase-02 v1.20** 为当前 BT 列表行布局与显示指标依据）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-10 QA 终验轮） | ezr 产品实现 + 15 套件 e2e + 引擎拆分 + 终验 | 已交付 | QA 终验全绿（上轮 handoff 口径在案） |
| specifier 显示指标轮（2026-11-10） | phase-02 v1.17（FR-02-33 v1.17 修订 + FR-02-36/37/38 新增 + D74）+ mission.md v1.47 + features 2 份 + qa 2 份 + **ezr-demo 同构修改** | 已交付 | APS parser+dry-check 2/2 exit=0；ezr-demo cargo test 45P/0F；PTY 实证三项显示形态 |
| specifier 调整轮 v1.48（2026-11-11） | phase-02 v1.18（FR-02-38 v1.18 布局修订 + FR-02-33/36/37 渲染口径注记）+ mission.md v1.48 + features 2 份 + qa 2 份 + **ezr-demo 修改（BT 尺寸对上移/协议列切片渲染/守卫 6 项）** | 已交付 | APS dry-check 2/2 exit=0；cargo test 51P/0F；PTY 实证 90–190 列全宽协议列对齐 |
| specifier 调整轮 v1.49（2026-11-11） | phase-02 v1.19（FR-02-38 v1.19 全类型扩面）+ mission.md v1.49 + features 2 份 + qa 3 份 + **ezr-demo 修改（全类型尺寸上移/切页签自愈缺陷修复/守卫 9 项/verify_size_move.py）** | 已交付 | APS dry-check exit=0；cargo test 54P/0F；PTY 实证 39 项全过 |
| specifier 调整轮 v1.50（2026-11-11） | phase-02 v1.20（FR-02-38 v1.20 做种已上传对退役 + FR-02-09 ② 退役注 + 验收清单 11）+ mission.md v1.50 + features/09-bt-engine v1.50（场景 10/13/21）+ qa/09-bt-engine-qa.md v1.50 + **ezr-demo 修改（list.rs 做种臂去已上传对/守卫反转/verify 脚本改版/README 40/设计规范 §5.2）** | 已交付 | APS parser+dry-check 2/2 exit=0；cargo test 54P/0F；PTY 实证 40 项全过 |
| **specifier 调整轮（本轮 2026-11-11，v1.51）** | **phase-01 v1.50（FR-01-117 新增：页签切换列表状态保留与还原）** + mission.md v1.51 + features/01-tui-display v1.50（场景 07/08 修订）+ qa/01-tui-display-qa.md v1.50（QA-TD-07/08）+ **ezr-demo 修改（TabListState/set_filter 三路径收口/守卫三守卫改版/verify 脚本场景 E/README 41/设计规范 §4.1.6+§5.2）** | 已交付 | cargo build 零警告、cargo test 56P/0F；PTY 实证 53 项全过 |

### 遗留受限项（当前有效）
- 历轮受限项沿 QA 终验轮口径不变（leech 专属 e2e、变异实跑、6 个 BT/命令窗口族无专属 runner、CRAP 观察项、ws(s) announce 无代理面、bt_dialog 变异点 119、CPD 残余克隆家族）。
- **ezr-demo verify_ezr.py 预存 FAIL 15 项维持（脚本锚点漂移，非产品回归）**：表头锚/conn_token 正则/横滚算术/t 键语义四族全部先于 v1.48 存在；本轮改动面（页签切换状态还原）不在其锚点面，实测 56 项单元守卫全绿为证。
- dialogs.rs 3 条预存 clippy 风格警告 + ui/mod.rs 5 条预存 unused-mut 警告（均先于本轮存在，与本轮无关；cargo build 零警告维持）。

### 实现定义值登记（累计，当前有效口径）
- **D44–D74 沿历轮登记不变**。
- v1.49 轮登记的两个可回退口径中，「做种中已上传对保留」已经 v1.50 轮退役（另一口径「尺寸未知任务行 1 不显示任何尺寸」维持）。
- v1.51 轮无新增 D 值；两个推导裁决可回退：①**滚动位置原样还原、不做视口吸附**（另一候选 = 还原后将视口吸附回选中行，未采纳——操作者明确要求「滚动条在原先位置」，且滚轮「只移视口不动选中」的浏览态属用户意志）；②**页签快照含选中任务身份（id）而非仅索引**（另一候选 = 纯索引快照，未采纳——集合位移时纯索引会选中错位邻居，正是操作者报告的缺陷形态）。

## 二、当前产出情况

### 本会话产物清单（specifier 调整轮 v1.51——页签切换列表状态保留与还原）
1. **需求文档**：`project/mission.md` 标题升 v1.51 + v1.51 头注记；`project/mission/phase-01.md` **v1.50 修订注**（追加式）+ **FR-01-117 立项**（页签切换列表状态保留与还原：切走快照「选中任务身份 + 选中索引 + 滚动位置」、切回按任务身份重定位 + 滚动原样还原、空页签复位 0/0、互不串扰、夹紧兜底；v1.49 切页签夹紧/视口跟随切换语义退役）+ **FR-01-109 ⑧ 退役注记**（「页签切换视口跟随选中项」移出滚动跟随触发源）。
2. **规格**：`project/features/01-tui-display.feature` v1.50（头部 v1.50 修订注 + 场景清单 07/08 注 + 场景 07 `]`/`[` 快照/还原断言改版 + 场景 08 页签点击同口径注）。09-bt-engine/04-ui-conns 等其余规格核对无页签状态语义表述，零改动。
3. **QA 规程**：`project/qa/01-tui-display-qa.md` v1.50 注记 + 对应规格行版本对齐 + QA-TD-07 判据修订（页签往返状态还原对账：计数行/▌ 行号/首可见任务行 + 不串扰 + 位移/收缩兜底）+ QA-TD-08 判据修订（页签点击与键盘同口径快照还原）。09-bt-engine-qa.md 零改动。
4. **ezr-demo 修改（操作者显式指令）**：`src/app/mod.rs`（新增 `TabListState` 快照结构 + `tab_states` 每页签状态 + `set_filter` 统一切页签入口：快照→切换→还原（按任务 id 重定位选中）→夹紧→同步 last_selected 抑制 tick 视口跟随；`cycle_filter` 退化为 `set_filter(next)`）、`src/app/mouse.rs`（页签点击接 set_filter，原直改 filter + scroll_to_selection 退役）、`src/app/dialogs.rs`（添加任务完成跳回页签 0 两处接 set_filter）、`src/ui/mod.rs`（旧 `cycle_filter_clamps_stale_selection…` 自愈守卫重写为快照语义三守卫：`tab_switch_preserves_and_restores_selection_and_scroll` / `tab_switch_relocates_selection_by_task_identity_when_tab_set_shifts` / `tab_switch_clamps_restored_state_when_tab_set_shrinks_and_renders_all_rows`）、`scripts/verify_size_move.py`（键序重排 Home/End 基准态 + 新增场景 E 13 项：切走期间页签独立状态证明 + 还原 A/复验还原 A/还原 B 帧对断言，40→53 项）、`README.md`（变更记录新增 41 条）、`docs/ezr-demo UI界面及交互设计规范.md`（头部 v1.51 注 + §4.1.6 滚动跟随触发源改版（页签切换拆出为快照还原源）+ §5.2 新增「页签切换状态保留与还原」段）。
5. **经验沉淀**：`project/notes/`（architecture v1.51 节：共享 selected/scroll 漂移根因与任务身份快照/set_filter 后抑制 tick 视口跟随/三路径收口单入口/还原与夹紧语义分层；testing v1.51 节：PTY 帧对断言法/pyte 显示行 CJK 字间空格锚定计数徽标/还原实证确定性键序与差异制造臂；INDEX.md 追加节）。
6. **本 handoff.md** 更新为当前有效口径。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| ezr-demo 构建 | release 零错误零警告（预存 dialogs.rs 3 条 clippy 风格警告与本轮无关） | `cd ezr-demo && cargo build --release` |
| ezr-demo 单元测试 | **56P/0F**（含改版快照语义三守卫 tab_switch_* 三项） | `cd ezr-demo && cargo test` |
| PTY 实证 | **53 项全过**：13 尺寸不变量 + 关键状态行 7 项 + 旧尺寸对缺席 8 项 + 已上传对缺席 2 项 + 保留项 7 项 + **场景 E 页签还原 13 项**（切走期间页签独立状态证明 3 项 + 还原 A/复验还原 A/还原 B 帧对断言 12 项：计数行 k/y 项、▌ 光标行号、首可见任务行逐项对账） | `cd ezr-demo && python3 scripts/verify_size_move.py` |
| APS 复验 | parser + dry-checker 双 exit=0（01-tui-display.feature/01-tui-display-qa.md 本轮修订后复验） | `.tools/bin/bb gherkin-parser / gherkin-ir-dry-checker`（.tools/Acceptance-Pipeline-Specification 目录内调用） |
| 做种行/尺寸项（v1.49/v1.50 口径沿用） | list.rs 本轮零改动，守卫 + PTY 场景 A–D 结论沿用 | `cargo test seeding_task_title` |

### 对账口径
- 产品端 ezr（project/ezr）**零改动**——本轮只有需求文档、规格、QA 规程与 ezr-demo 改动；ezr 实现 FR-01-117 页签状态还原归下一 coder 轮（**实现要点：每页签独立快照（选中任务 id + 索引 + 滚动）；键盘/鼠标/添加任务跳回三路径收口单入口；还原后抑制视口跟随（同步选中代偿标记）；集合收缩夹紧兜底沿用；水平滚动渲染切片（v1.48）、任务行尺寸项 size_known 门控（v1.49）、做种臂无已上传对（v1.50）均为验收判据——notes/architecture.md v1.49–v1.51 节**）。
- ezr-demo 改动面 = 源码 4 文件 + 文档 2 文件 + 脚本 1 个；守卫测试 56 项全绿；做种行/尺寸项口径零改动沿用。
- 产物归档 = `bin/swarm complete`（含 CONSTITUTION.md、packs、bin、project）；**ezr-demo 不在归档集内**——操作者要求产物复制 my-project 时额外打包 ezr-demo 源码归档（不含 target/）与 release 二进制一并交付。

### 待办与待批
- **v1.51 调整轮为操作者 chat 直接裁决**（无 Inversion 题目、无新增 D 值）；本轮登记的两个推导裁决可回退：①滚动位置原样还原不做视口吸附（另一候选 = 还原后视口吸附回选中行，未采纳）；②快照含选中任务身份 id（另一候选 = 纯索引快照，未采纳）。若操作者另有指示按批注回退。
- v1.49 轮「切页签夹紧残留 selection + 视口跟随选中项」切换语义经本轮退役（夹紧保留为还原兜底，历史注记保留）。
- 下一角色建议：`six-pack/coder`（ezr 产品端实现 FR-01-117 + 01-tui-display v1.50，规格与 QA 已就绪；**页签快照按任务身份重定位 + 三路径单入口 + 还原后抑制视口跟随 + 集合收缩夹紧兜底为验收判据**）。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留，下一轮可直接续跑）。
- 续跑下一轮：`bin/swarm begin six-pack/coder`（以本文件为上游交接内容）。
