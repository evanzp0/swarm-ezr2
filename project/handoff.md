# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（01-PC 套件可执行化扩面轮——待批② runner 面 + QA-PC-13 补 BT 下载流程，均经操作者裁决执行）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner/architect/QA 收敛链 ⇒ specifier 显示指标/调整/裁决/新增需求轮（v1.47–v1.53）⇒ specifier 裁决落实修订轮（v1.54）⇒ coder 产品实现/复核重验轮 ⇒ specifier PEERS/SEEDS 核证修订轮（v1.55）⇒ specifier D80 裁决落实轮（v1.56）⇒ coder FR-02-37 v1.3 产品实现轮 ⇒ cleaner 收敛轮 ⇒ architect 架构评审轮 ⇒ hardender 变异加固三轮 ⇒ QA FR-02-37 v1.3 终验轮（2026-11-16，工作流走完 Done）⇒ specifier 退役键口径/拒绝行/组成算术三轮 ⇒ QA _case_13 键集补齐与注记结案回填轮 ⇒ specifier QA-PC-10/11 规程行补齐轮 ⇒ **QA 01-PC 套件可执行化扩面轮（本轮 20261116，post-Done 维护小轮）**
> 会话：six-pack-QA-20261116b（SESSION_TASK = 待批② runner 面（_case_10/11/12 可执行化）+ 观察项收口（_case_13 补 BT 下载流程，复用 suite_modify_bt tracker driver 基建）；上游交接 = specifier QA-PC-10/11 规程行补齐轮 handoff「已批待执行」节 + 操作者 chat 裁决）
> Inversion 受限退化：任务为已批待办执行，无未决需求问题未触发访谈。
> 环境：nightly 1.102.0（TUNA dist，清单日期先验——aliyun 快照停 2026-05 编译 E0658 弃选）、ezr 四 bin debug 构建、套件依赖（pyte/wcwidth/libtorrent 2.1.1 wheel）装齐；fmt/clippy 未跑（零产品源码改动面）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.56**）+ `mission/` 四份 phase 详述（现行 phase-01 v1.53、phase-02 v1.24）。ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA/hardender 历轮（至 2026-11-16 hardender 三轮） | ezr 产品实现 + 收敛/评审/加固链 + 变异实跑 494 突变体全处置 | 已交付 | 17 目标 9170P/0F、clippy 0、fmt 合规、arch_check 18 规则过 |
| QA FR-02-37 v1.3 终验轮（2026-11-16） | 产品缺陷修复 2 处 + QA runner 同步修复 3 套件 + 全量独立验证 + notes/handoff | 已交付 | 基线 17 目标 9170P/0F + prop 88P + 15 套件 191P/0F 全绿 + CRAP/DRY 实跑登记 |
| specifier 退役键口径/拒绝行/组成算术三轮（2026-11-16） | 01-PC 规程 v1.24→v1.26 + feature v1.24→v1.26 + notes/INDEX + handoff | 已交付 | 结构校验全过；零产品改动 |
| QA _case_13 键集补齐与注记结案回填轮（2026-11-16） | suite_persistence_config.py v1.26 同步 + 01-PC 规程/feature 组成算术按模板权威拨正 | 已交付 | 套件全量重跑 P=11 F=0 S=0（83s）；零产品源码改动 |
| specifier QA-PC-10/11 规程行补齐轮（2026-11-16） | 01-PC QA 规程 v1.27（用例表补 QA-PC-10/11 两规程行，14 用例 ↔ 14 场景全对齐）+ notes/INDEX + handoff | 已交付 | 结构校验全过；零产品改动（归档 swarm-20261116-132619-six-pack-specifier.tar.gz） |
| **QA 01-PC 套件可执行化扩面轮（本轮）** | suite_persistence_config.py v1.27（_case_10/11/12 新增 + _case_13 补 BT 下载流程，14 用例全量注册）+ notes 三文件 + 本 handoff | 已交付 | **套件全量重跑 P=14 F=0 S=0（105.6s）**；零产品源码改动 |

### 遗留受限项（当前有效）
- **变异实跑总账（三轮，全收敛）**：全清单 494 突变体处置闭环、零未跑；登记函数 235 = 杀 149 + unviable 6 + 超时杀 1 + 存活 79；非登记函数 84 沿杀灭账免跑；trackpool ws 面 31 = 20 定理证明 + 11 免跑。逐突变体判定权威 = `adaptive_resolved.csv` 最新行（沙箱重置后按交接②再生）。
- **79 存活体 = 覆盖缺口确认（非等价），双套件零杀灭证据已补齐**：`results/bs_runs.csv` 23 批次 exit=0、恰覆盖 79 id；回补路径沿 hardender 交接④。**缺口 112 回补测试归属（QA 裁决）**：移交操作者裁决是否追加专项轮（待批③）。
- **软 Gherkin 验收变异执行层不可用（受限项，沿历轮登记不变）**：回补属专项基建轮（待批④）。
- **QA runner 族缺口（沿历轮登记收窄后现状）**：06-cmd-window、07-add-bt、09-bt-engine、10-bt-anti-leech、11-trackers-window 五规程暂无可执行套件（沿登记不变）；01-PC 套件本轮扩面后 **14 用例全量在册**（待批② 收口）。
- **ezr-demo 侧验证脚本（工作区外）**：verify_trackers_popup.py 3F + verify_size_move.py 陈旧化——demo 专项轮归操作者指派。
- **CRAP/DRY 登记（沿 QA 终验轮口径不变）**：本轮零产品源码改动面（改动面 = qa/runners Python 侧），登记值不变（CRAP 行覆盖 94.4%/ACTIONABLE_COUNT=99；DRY 阈值 200 六组、阈值 70 全量 198 组 = 产品侧 23/测试侧 175）。
- **QA-PC-06 环境性 flake（沿 QA 登记）**：负载敏感时序 flake 非回归，复现按三证归因。
- **mktorrent bencode 跨 bin 重复**：维持登记、lib 化不实施。
- **既存 [/] 改写字节状态（沿 bug16/bug18 登记）**：规格面文件存在历史会话遗留的损伤形态字节（01-PC QA 规程 13 处、feature 6 处、runner 2 处）与完整字节并存；本轮 runner 扩面编辑全程字节计数终审零附带损伤（损伤形态 ttp]=2 不变、完整形态 [http]=2 不变；[common] 6→7、[[proxies]] 0→4、[bt] 计数相应新增——均为 _case_12 旧格式样例/注记与 _case_13 cfg 的有意新增内容，无意外漂移）；**显示通道对 [/] 序列有改写**——审计只认 bytes 计数，不认工具输出显示文本。
- **本轮构建工具链（受限项解除）**：nightly 1.102.0（TUNA dist）装齐、ezr 四 bin debug 构建在位、suite 基线全绿——上上轮 stable 构建受限披露解除；本轮零产品源码改动面，fmt/clippy 未跑（沿先例）。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退），D80①–④ 已全部结案**——本轮为 runner 可执行化扩面，无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（QA 01-PC 套件可执行化扩面轮）
1. **qa/runners/suite_persistence_config.py v1.27（runner 面，待批② 裁决 + 观察项收口）**：
   - **_case_10 新增（EZR_HOME 重定位）**：主臂 = EZR_HOME 指向空目录启动 → 模板生成 $EZR_HOME/config.toml + 注册表/锁写入 $EZR_HOME/state/ + 默认 ~/.ezr 未创建 + 改值生效环（download_slots=2 → 队列栏 0/2）承载「配置读取自 $EZR_HOME」行为证据；空串臂 = EZR_HOME="" 回退 ~/.ezr；隔离臂 = A 实例运行中 B 实例（不同 EZR_HOME）可启动、各自持锁（单实例保护按 EZR_HOME 隔离）。
   - **_case_11 新增（kill -9 异常终止终端自恢复）**：SIGKILL 主进程 → 哨兵 RESTORE_SEQUENCE 七关闭臂（?1006l/?1015l/?1003l/?1002l/?1000l/?2004l/?1049l）全量出现在 kill 后新增原始尾流段 + 开启序列零再现（无残影）+ cooked 回显探针（termios 恢复后按键回显）+ 主进程非零终止 + /proc/exe 精确断言主进程与哨兵均退出无残留 + 重启正常；「Ctrl+C 产生 SIGINT」由哨兵 stty 全量恢复的 termios 承载（runner 面收缩注记见套件 docstring）。
   - **_case_12 新增（旧格式顶层键与旧 [[proxies]] 表按未知键忽略，UI 层观察面）**：旧格式预置（顶层平铺 download_slots=2/block_size_http=4194304/http_concurrency=8/max_speed="1 MB/s" + 顶层 [[proxies]] 合法条目）→ 启动零警告零迁移 + 全部配置项按默认值生效（槽位 0/5、块 1 MB/块、并发 4 头部数据面）+ 代理下拉无旧条目（直连项在、QA12-OLD 缺席）+ 同文件追加 [common] download_slots=3 后新键生效（0/3）；解析行为与 hardening_v115.rs 集成锚双层承载不重复（docstring 口径）。
   - **_case_13 补 BT 下载流程（观察项收口）**：复用 suite_modify_bt tracker driver 基建（_Tracker/LtPeer/_make_payload/_make_torrent/_tab_to_button/_read_driver_announces 跨套件 import，suite_modify_bt 零改动）——真实 tracker URL 进 cfg trackers 首位（预填断言不变 + 种子 announce 合并断言新增），BT 对话框确认下载（_tab_to_button 聚焦立即下载）→ 从 libtorrent seeder 完成四文件字节级校验（90s 窗）→ 自动做种态断言（D45）→ tracker 日志 announce port = listen_port 断言（规程判据「fixture announce port 可观察」字面承载）；**改判披露**：peer_connection_protocol 非默认单值臂由 ["webrtc"] 改 ["tcp"]——合法单值（白名单内）且非默认（≠ 模板双值），WebRTC 单值臂下 ezr 无跨协议回退无法与 libtorrent seeder（TCP only）建立连接，BT 下载流程不可达成；判据「[bt] 其余键 = 下载做种全程无错误」在 TCP 臂下达成。
   - **run() 扩面与 docstring 更新**：实注册 11 → 14 用例（PC-01..14 全量）；头部注记 v1.27 同步。
2. **经验沉淀**：notes/testing.md 新增「01-PC 可执行化扩面轮」五条目（协议臂相容性/恢复序列断言窗口/秒传终态断言/跨套件 import 选型/complete 清 target 连锁）；notes/environment.md 追加 aliyun dist 滞后后果实证条目；INDEX.md 同步。
3. **本 handoff.md** 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令/证据（可复现） |
|---|---|---|
| 单案试跑 | PC-10 P（6.2s）/ PC-11 P（1.3s）/ PC-12 P（5.6s）/ PC-13 P（13.2s，含 BT 下载流程） | `cd project/qa/runners && QA_ONLY=QA-PC-1x python3 suite_persistence_config.py` |
| **套件全量重跑** | **P=14 F=0 S=0（累计 105.6s）**——PC-01..14 全绿，扩面不影响既有案 | `cd project/qa/runners && python3 suite_persistence_config.py` |
| BT 下载链路 | 四文件字节级完整 + 做种态 + tracker 日志 announce port = listen_port（含 seeder/ezr 双端 announce） | tracker JSONL 日志逐行 port 字段过滤断言 |
| runner 字节终审 | 损伤形态 ttp]=2 不变（零新增）、完整形态 [http]=2 不变；[common] 6→7、[[proxies]] 0→4、[bt] 新增——均为本轮有意新增内容（_case_12 样例与注记、_case_13 cfg），零意外漂移 | python bytes 计数前后比对（不认显示文本） |
| 产品基线 | 零产品源码改动（src/、tests/ 未触碰）；runner 扩面 + notes 三文件 | — |
| CRAP/DRY | 沿 QA 终验轮登记口径不变（零产品改动面，不重复实跑） | — |

### 对账口径
- 本轮改动面 = 1 runner（qa/runners/suite_persistence_config.py）+ notes 三文件（testing.md/environment.md/INDEX.md）+ 本 handoff；**规程、feature、src/、tests/ 零改动**（规程 v1.27 由上一轮 specifier 交付，本轮直接消费）。
- **改判披露（runner 面，沿 20261112 锚点对齐轮先例）**：peer_connection_protocol 臂取值改判为 BT 下载流程可达成的最小改动（["webrtc"] → ["tcp"]，单值合法且非默认口径不变）；如操作者认为不当，可回退该值并登记「WebRTC 臂下 BT 下载流程受限」（runner 其余断言与全绿证据不受影响）。
- **越面复用披露（suite_modify_bt 基建 import）**：DRY 正统 = 上提共享基建入 harness.py（须改 suite_modify_bt + 重跑其套件验证）；本轮按最小改动面选直接 import（suite_modify_bt 零改动零重验），上提归后续 cleaner/QA 收敛轮（notes/testing.md 登记）。
- _case_11 断言面收缩注记：「Ctrl+C 可正常产生 SIGINT 中断」由哨兵 stty 全量恢复的 termios（ISIG 在内）承载——kill 后无前台进程组可观察 SIGINT 投递，runner 以恢复序列 + 回显探针承载（docstring 已注记）。

### 待办与待批
- **待批③（沿 QA 登记）**：缺口 112 回补测试专项轮。
- **待批④（沿 QA 登记）**：软 Gherkin 执行层回补基建专项轮。
- **QA runner 族缺口（沿历轮登记）**：06-cmd-window、07-add-bt、09-bt-engine、10-bt-anti-leech、11-trackers-window 五规程暂无可执行套件。
- **观察（本轮新登记，peer_connection_protocol 改判可回退）**：见对账口径，待操作者追认。
- **待办（产品面，沿前轮登记不变）**：ezr/README.md 配置示例仍写 `max_speed = "2 MB/s"`——移交 coder/cleaner 面修订。
- 待办：ezr-demo 侧 verify 脚本 3F/陈旧化（demo 专项轮，工作区外）。

### 移交建议
- 待批② 与观察项均已收口：规程 14 用例 ↔ feature 14 场景 ↔ runner 14 用例三方全对齐，套件全量 P=14 F=0 S=0 实证；**QA-PC-13 三方一致在 BT 下载流程面补全**（listen_port 判据由 fixture announce port 字面承载）。
- **six-pack 流转路径自 QA 终验轮起保持完结态**；本轮为 post-Done 独立维护小轮，无自动流转。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 如需续做：上方待批③④与五规程可执行化均为独立小轮，由操作者显式决定。
