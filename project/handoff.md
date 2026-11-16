# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（_case_13 键集补齐与注记结案回填轮——操作者批准 B 案 runner 半落实 + 模板权威对账拨正）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner/architect/QA 收敛链 ⇒ specifier 显示指标/调整/裁决/新增需求轮（v1.47–v1.53）⇒ specifier 裁决落实修订轮（v1.54）⇒ coder 产品实现/复核重验轮 ⇒ specifier PEERS/SEEDS 核证修订轮（v1.55）⇒ specifier D80 裁决落实轮（v1.56）⇒ coder FR-02-37 v1.3 产品实现轮 ⇒ cleaner 收敛轮 ⇒ architect 架构评审轮 ⇒ hardender 变异加固三轮 ⇒ QA FR-02-37 v1.3 终验轮（2026-11-16，工作流走完 Done）⇒ specifier FR-02-40② 退役键口径同步轮 ⇒ specifier max_speed 退役键拒绝行同步轮 ⇒ specifier QA-PC-13 组成算术重述轮 ⇒ **QA _case_13 键集补齐与注记结案回填轮（本轮 20261116，post-Done 维护小轮）**
> 会话：six-pack-QA-20261116（SESSION_TASK = 补齐runner _case_13实写键集至现行全量口径并结案回填过时注记；上游交接 = specifier 组成算术重述轮 handoff，其「待办（QA 面，已批待执行）」首条为任务依据；操作者指令「批准，按 b 处理」= 批准 B 案全案，runner 半由本会话落实）
> Inversion 受限退化：任务为已批待办执行，无未决需求问题未触发访谈。
> 环境：**stable 工具链构建（受限披露）**——rust-toolchain.toml 钉 nightly 而本沙箱仅装 stable（nightly 未装；pin 语义 = fmt/clippy unstable 选项一致性，非编译必需）；本轮以 `RUSTUP_TOOLCHAIN=stable` 构建执行二进制（源码零改动、无 nightly feature，套件 11/11 全绿为运行等价性实证）；fmt/clippy 未跑（零产品源码改动面）。cargo 初判 target 消失系误诊——实为 crates.io 索引/依赖下载卡死未进编译；切 rsproxy sparse（environment.md 既证口径）后 2m14s 完成构建。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.56**）+ `mission/` 四份 phase 详述（现行 phase-01 v1.53、phase-02 v1.24）。ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA/hardender 历轮（至 2026-11-16 hardender 三轮） | ezr 产品实现 + 收敛/评审/加固链 + 变异实跑 494 突变体全处置 | 已交付 | 17 目标 9170P/0F、clippy 0、fmt 合规、arch_check 18 规则过 |
| QA FR-02-37 v1.3 终验轮（2026-11-16） | 产品缺陷修复 2 处 + QA runner 同步修复 3 套件 + 全量独立验证 + notes/handoff | 已交付 | 基线 17 目标 9170P/0F + prop 88P + 15 套件 191P/0F 全绿 + CRAP/DRY 实跑登记 |
| specifier FR-02-40② 退役键口径同步轮（2026-11-16） | 3 feature 判据面修订 + 3 份 QA 规程同步 + notes/INDEX + handoff | 已交付 | 结构校验全过；零产品改动 |
| specifier max_speed 退役键拒绝行同步轮（2026-11-16） | 01-PC feature v1.25 + 01-PC QA 规程 v1.25 + notes/INDEX + handoff | 已交付 | 结构校验全过；零产品改动 |
| specifier QA-PC-13 组成算术重述轮（2026-11-16） | 01-PC QA 规程 v1.26 + 01-PC feature v1.26 + notes/INDEX + handoff | 已交付 | 结构校验全过；零产品改动（归档 swarm-20261116-115255-six-pack-specifier.tar.gz） |
| **QA _case_13 键集补齐与注记结案回填轮（本轮）** | suite_persistence_config.py v1.26 同步（_case_13 cfg 补齐模板权威 40 键 + 断言补列 + 注记结案回填）+ 01-PC 规程/feature 组成算术按模板权威拨正 + notes/INDEX + 本 handoff | 已交付 | **套件全量重跑 P=11 F=0 S=0（83s）**；零产品源码改动 |

### 遗留受限项（当前有效）
- **变异实跑总账（三轮，全收敛）**：全清单 494 突变体处置闭环、零未跑；登记函数 235 = 杀 149 + unviable 6 + 超时杀 1 + 存活 79；非登记函数 84 沿杀灭账免跑；trackpool ws 面 31 = 20 定理证明 + 11 免跑。逐突变体判定权威 = `adaptive_resolved.csv` 最新行（沙箱重置后按交接②再生）。
- **79 存活体 = 覆盖缺口确认（非等价），双套件零杀灭证据已补齐**：`results/bs_runs.csv` 23 批次 exit=0、恰覆盖 79 id；回补路径沿 hardender 交接④。**缺口 112 回补测试归属（QA 裁决）**：移交操作者裁决是否追加专项轮（待批③）。
- **软 Gherkin 验收变异执行层不可用（受限项，沿历轮登记不变）**：回补属专项基建轮（待批④）。
- **本轮构建工具链受限（新登记）**：nightly 未装（rustup 沙箱仅 stable），本轮以 stable 构建执行二进制跑套件——源码零改动 + 11/11 全绿为运行等价性证据；fmt/clippy/arch_check 未跑（零产品源码改动面，风险=极低）；nightly 装齐走 rsproxy/TUNA dist（environment.md 口径），如后续轮需 fmt/clippy/变异再装。
- **QA-PC 规程/runner 组成差异（沿 FR-02-40② 轮登记不变，待批②）**：qa/01-persistence-config-qa.md 有 QA-PC-12 规程行而 runner 无 _case_12；QA-PC-10/11 规程行与 runner 用例双缺——是否补规程与可执行化待操作者裁决（属 QA 可执行化职责面）。套件头部计数注记已按实注册 11 用例回填（本轮）。
- **QA runner 族缺口（沿历轮登记不变）**：06-cmd-window、07-add-bt、09-bt-engine、10-bt-anti-leech、11-trackers-window 五规程暂无可执行套件；现行 15 套件沿 QA 终验轮全绿。
- **ezr-demo 侧验证脚本（工作区外）**：verify_trackers_popup.py 3F + verify_size_move.py 陈旧化——demo 专项轮归操作者指派。
- **CRAP/DRY 登记（沿 QA 终验轮口径不变）**：本轮零产品源码改动面，登记值不变（CRAP 行覆盖 94.4%/ACTIONABLE_COUNT=99；DRY 阈值 200 六组、阈值 70 全量 198 组 = 产品侧 23/测试侧 175）。
- **QA-PC-06 环境性 flake（沿 QA 登记）**：负载敏感时序 flake 非回归，复现按三证归因。
- **mktorrent bencode 跨 bin 重复**：维持登记、lib 化不实施。
- **既存 [/] 改写字节状态（沿 bug16/bug18 登记）**：规格面文件存在历史会话遗留的 ttp] 形态字节（01-PC QA 规程 13 处、feature 6 处、runner 2 处）与完整字节并存；本轮两轮编辑全程避开敏感序列并经字节计数终审零附带损伤（规程 ttp]=13/[http]=8 不变、runner ttp]=2/[http]=2 不变、feature ttp]]=[http]=7/7（+1 为本轮有意新增注记行的完整形态，损伤形态 0 不变））；**显示通道对 [/] 序列有改写**——审计只认 bytes 计数，不认工具输出显示文本（bug18 判别法本轮第 3 次实证，差点误诊自伤）。

### 实现定义值登记（累计，当前有效口径）
- **D44–D80 沿历轮登记不变（均不可回退），D80①–④ 已全部结案**——本轮为 runner 键集同步 + 文字面对账，无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（QA _case_13 键集补齐与注记结案回填轮）
1. **qa/runners/suite_persistence_config.py v1.26 同步（runner 面，操作者已批 B 案）**：_case_13 cfg 键集补齐至模板权威 40 键——[common] 补 global_max_upload_speed = "1 MiB/s" / task_max_download_speed = "512 KiB/s" / task_max_upload_speed = "256 KiB/s"；[bt] 补 listen_port（新增 `_free_listen_port()` 空闲端口探针 helper + socket import，防占用回退警告破坏零警告断言）、退役键 `seed_duration_secs = 1500.0` 残留改 `default_seed_duration_m = 25`；**追加发现并修正**：peer_connection_protocol 原写 `["tcp", "webrtc"]` = 模板默认值（违反「非默认」口径）改合法单值 `["webrtc"]`（单测 peer_connection_protocol_validation 锚证单值合法/大小写规范化/重复项拒绝）。断言补列：HTTP 对话框下载限速预填 + BT 对话框下载/上传限速预填（`下载限速\s*>\s*512 KiB/s` 正则，QA-01-20 同款；BT 添加对话框预填源 = cfg.task_max_download_speed/task_max_upload_speed，app/bt_dialog.rs L34-40 核证）。注记结案回填四处：套件头「12 用例」计数注记改实注册 11 用例口径、_case_13 docstring v1.26 同步注、QA-PC-04 docstring「待 specifier 同步」结案态、QA-PC-04「已知偏差（待操作者裁决）」结案态（max_speed 偏差已于 v1.25 结案）。
2. **01-PC 规程/feature 组成算术按模板权威拨正（越面披露，见对账口径）**：规程 v1.26 头注 + QA-PC-13 操作列、feature v1.26 头注 + 场景 13 Given——组成算术改「40 键 = [common] 6 键 + HTTP 表 11 键 + [bt] 23 键，proxy 双条目为可选附加面不计入」（[bt] 23 = trackers/peer_connection_protocol/bt_concurrency 3 核心键 + 20 调优键；模板权威 = 43 活跃行 = 40 键值 + 3 表声明，suite_config_template 逐键断言锚）；spec 面自 v1.20 沿用的「+ proxy 双条目 + [bt] 21」分解两项偏差恰好抵消合 40（proxy 误计入 +2、[bt] 少计 2），本轮拨正。行为语义零变更。
3. **经验沉淀**：project/notes/testing.md「QA-PC-13 组成算术重述轮」条目扩充（锚选型补 feature 算术可漂移/模板权威为终点 + 新增全量冒烟 cfg 逐键审读纪律两条），INDEX.md 同步。
4. **本 handoff.md** 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令/证据（可复现） |
|---|---|---|
| runner cfg 逐键审计 | 与模板 40 键清单**全等**（common 6 / http 11 / bt 23，proxy 双条目附加；无缺键、无多键、无退役键） | python 解析 cfg 字符串逐表比对 suite_config_template 键清单 |
| 非默认值审计 | 40 键取值全部非代码默认（peer_connection_protocol 单值臂修正后达标） | 对照 suite_config_template 默认值断言表逐键核对 |
| 单案试跑 | QA_ONLY=QA-PC-13 → **P（3.9s）**：零警告 + 新键集解析 + 预填断言全过 | `cd project/qa/runners && QA_ONLY=QA-PC-13 python3 suite_persistence_config.py` |
| **套件全量重跑** | **P=11 F=0 S=0（累计 83s）**——PC-01..09/13/14 全绿，补齐案不影响既有案 | `cd project/qa/runners && python3 suite_persistence_config.py` |
| spec 面字节终审 | 规程 ttp]=13/[http]=8 不变、runner ttp]=2/[http]=2 不变、feature ttp]]=[http]=7/7（+1 = 有意新增注记行完整形态，损伤形态 0→0）——**零附带损伤** | python bytes 计数前后比对（不认显示文本——显示通道对 [/] 序列有改写） |
| 产品基线 | 零产品源码改动（src/、tests/ 未触碰）；runner/规程/feature 文字与键集面同步 | — |
| CRAP/DRY | 沿 QA 终验轮登记口径不变（零产品改动面，不重复实跑） | — |

### 对账口径
- 本轮改动面 = 1 runner（qa/runners/suite_persistence_config.py）+ 1 QA 规程 + 1 feature + notes 两文件 + INDEX.md + 本 handoff；**src/、tests/ 零改动**。
- **越面披露（QA 面 → specifier 文字面，卫生面顺手修）**：本轮逐键审计发现 spec 面组成算术分解与模板权威不符（feature v1.20 起既有错误，上一轮 specifier 重述时沿用了该分解未察觉）——依 B 案「三方一致」目标与 testing.md:808 先例（文档计数校正属卫生面可顺手修），在规程/feature 内拨正分解表述并双头注记登记 QA 拨正；**行为语义零变更，仅文字对账**。如操作者认为越面不当，可回退该两处文字（runner 与套件绿证据不受影响）。
- **显示通道误诊实录**：轮内曾据工具输出显示文本误判 feature 出现新损伤 ttp]（实为显示通道对 [/] 序列的改写）——以 bytes 计数终审纠正（bug18 判别法第 3 次实证）；审计纪律：**只认 bytes 计数与 hex，不认显示文本**。
- 规程 QA-PC-13 判据「listen_port 生效断言由 suite_modify_bt announce 观察承载 / default_seed_duration_m 生效断言由引擎级承载」沿通过准则调优键承载面写法；套件内以合法解析零警告承载。

### 待办与待批
- **观察（QA 规程面，沿上一轮登记不变）**：QA-PC-13 操作列「完成一次 BT 下载」与 runner 实跑流程差（runner 实跑 = HTTP 下载完成 + BT 对话框预填观察，无 BT 下载；feature 判据 listen_port「fixture announce port 可观察」在 _case_13 现行流程不可观察）——是否补 BT 下载流程（可复用 suite_modify_bt tracker driver 基建）待操作者裁决（QA 可执行化扩面）。
- **待批②（沿 FR-02-40② 轮登记不变）**：QA-PC-10/11 规程行与 runner 用例双缺、QA-PC-12 规程行无 runner 对应——是否补规程/可执行化（QA 面小轮）。
- **待批③（沿 QA 登记）**：缺口 112 回补测试专项轮。
- **待批④（沿 QA 登记）**：软 Gherkin 执行层回补基建专项轮。
- **待办（产品面，沿前轮登记不变）**：ezr/README.md 配置示例仍写 `max_speed = "2 MB/s"`——移交 coder/cleaner 面修订。
- 待办：ezr-demo 侧 verify 脚本 3F/陈旧化（demo 专项轮，工作区外）。

### 移交建议
- B 案两半均已落实收口：规程/feature 文字面（specifier 轮）+ runner 键集/断言/注记（本轮）→ **QA-PC-13 三方一致达成**（规程 ↔ feature ↔ runner 键集 40 全等 + 套件 11/11 全绿实证）；操作列「完成一次 BT 下载」流程差留观察待裁决，不影响现行绿态。
- **six-pack 流转路径（specifier → coder → cleaner → architect → hardender → QA → Done）自 QA 终验轮起已走完，工作流保持完结态**；本轮与前三轮均为 post-Done 独立维护小轮，无自动流转。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 如需续做：上方观察与待批②③④均为独立小轮，由操作者显式决定。
