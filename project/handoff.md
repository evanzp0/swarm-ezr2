# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA　会话：ezr-v148-qa（2026-10-22）
> 上游交接：project/handoff.md（architect v146-arch 交接：4200P/0F + 两项移交闭环）
> + 操作者 chat 指令（运行 six-pack/QA，最终独立验证轮）。
> 当轮任务：e2e 腐化探针全收绿 + qa/runners 欠账消化 + 05-ui-pieces 可执行化 +
> CRAP/DRY 门禁对账 + 覆盖率读数复核；产品代码零改动（QA 轮边界）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.33，分期模式）+ `project/mission/phase-01.md`（v1.33）。
- 源码状态 v148（规格 v1.33 / features 14 份 / qa 规程 14 份；全 15 目标 **4200P/0F**，
  对账见下）。本轮为最终独立验证轮：**产品源码零改动**，全部产出在 qa/（套件与规程
  同步）与工具侧 packs/_common/（经验沉淀）。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.33 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | v134 起 3 份 UI feature 过 APS 机检（0 错误级）；v132 轮修订未过机检（见受限项） |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F（v128 口径，CPD 读数不可比见 v141 登记） |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 hardening_g_v147 | 已交付 | 3363P/0F、clippy 0、fmt 0 |
| QA（ezr-qa-final） | QA 规程可执行化 13 端到端套件（qa/runners/）+ 缺陷修复 | 已交付 | e2e 165P/0F/0S（v1.29 口径快照）、3363P/0F、覆盖 94.37% |
| specifier（v129–v134/v132/v140） | FR-01-106…110 + v1.30/v1.31/v1.32 对齐规格 + v1.33 FR-01-112/D40 | 已交付 | v140 反向自检过（纯新增） |
| coder（v129–v137） | FR-01-106 实现 + 缺陷修复 + v1.30/v1.31 UI 改版 + q 退出鼠标泄漏修复 | 已交付 | 3889P/0F、clippy 0、fmt clean（v137 口径） |
| coder（v138-m3-impl） | m3 对齐全量实现（scrollbar 引擎 + scrollable_panel + About + 头部/对话框/帮助/模态口径） | 已交付 | 3963P/0F、clippy 0、fmt clean |
| coder（v139-m3-padding-fix） | 活跃计数标题 padding 对称；ezr-demo 同步 + demo 文档 §5.4 | 已交付 | 4009P/0F、clippy 0、fmt clean |
| coder（v140-builtin-dns-impl） | FR-01-112：src/engine/dns.rs 内置异步 DNS 解析器 + base_client_builder 挂接 + config hosts_file_path 缝 | 已交付 | 4162P/0F（全 15 目标）、clippy 0、fmt clean |
| cleaner（v141-clean-dns） | 保持行为清理：dns.rs E1–E3 去重 + load_from 测试缝 + 补测 4 例；arch 规则 14 白名单收口 | 已交付 | 4198P/0F、clippy 0、fmt clean、arch 15/15、dns.rs 覆盖 97.8%、CPD 125 tile |
| architect（v146-arch） | 四阶段评审（零违规）+ 两项移交裁决收口 + dns 属性面 +2 + arch 阳性对照复测 | 已交付 | 4200P/0F、clippy 0、fmt clean、arch 15/15、CPD 125 tile（零新增） |
| **QA（v148-qa，本轮）** | **14 套件 e2e 全收绿（167P/0F）+ 欠账全量消化 + 05-ui-pieces 可执行化（+1 套件 7 用例）+ CRAP/CPD 全项目首扫基线 + 覆盖率读数复核** | **已交付** | **e2e 167P/0F、插桩全量 4186P/0F（+14 doc = 4200 闭合）、行覆盖 94.9%、分支命中 94.1%、CPD 125 tile（与 v146 精确一致、零新增）、CRAP>30 共 9 处登记移交、clippy 0、fmt clean、arch 15/15（动工前基线复核）** |

### 遗留受限项（当前有效）
- **APS 机检沿袭未执行**（v132 修订的 3 份 UI feature 未过 `gherkin-ir-dry-checker`；
  本沙箱 GitHub 不可达未补跑，状态不变）。
- **v147 时序 flake（沿袭）**：本轮插桩全量 + e2e 全程未见 flake（hardening_g_v147
  分块内 943 用例含其全量，0F）；`wait_state` 预算放宽建议维持给 hardender。
- **Rust 工具链沙箱不持久（本轮再证）**：全新沙箱完整重装 nightly
  1.101.0-nightly（rsproxy rustup-init + USTC dist + rsproxy-sparse + rust-lld 桥接 +
  pyte pip）。冷启动配方见 rust.md §1。**本轮新增：GitHub 不可达**（pmd bin zip /
  APS gherkin-ir-dry-checker 的获取路径受影响；替代路径见 rust.md PMD 条目）。
- **dns.rs 覆盖率残余 3 行（沿袭 v141 登记）**：167、241/247（需真实 DNS 应答）。
  hardender 变异轮若在这三行产生存活突变体，按 v141 登记处置。
- **观察受限突变体（沿袭 hardender 登记，仍开放）**：workers.rs 5 条 + 等价
  download.rs:85:27。
- **ezr-proxy.rs 测试基建（沿袭裁决）**：127 变异点不拆、CRAP/CPD 读数不追责。
- **CRAP 全项目首扫基线（本轮新登记，移交 hardender 处置）**：产品区 1193 逻辑函数
  （BRDA comp=BRDA+1 受限近似，闭包归组取极值，测试区行界排除 1499 函数），
  **CRAP>30 共 9 处**：mouse.rs:325/188/74/41、scrollbar.rs:297/236、keys.rs:18、
  supervisor/single.rs:63（UI 分派主链/守卫链，源码决策点复核为真复杂 + 单测残余
  覆盖，e2e 套件实际覆盖鼠标/键盘/滚动条全路径）、ezr-fixture respond.rs:156
  （QA 工具，e2e 覆盖、单测盲区）；豁免区（ezr-proxy）2 处（沿袭裁决不追责）。
  另 comp>6 离群点 91 处（BRDA 上界口径，含 `&&`/`||` 短路链双计虚高，见
  rust.md 对应条目）。QA 轮不擅改产品，处置建议：hardender 轮按「补测提覆盖优先、
  拆函数其次、守卫链以源码决策点数复核后豁免登记」排序。
- **退出竞态修复的可观测边界（沿袭 v137 登记）**：极端时序下毫秒级鼠标事件残留属
  业界共有限界。
- **沙箱后台进程回收（本轮再证）**：detached 后台进程随工具调用结束被连带清场——
  长验证一律前台分段，不用后台进程（engineering.md 已载）。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D40。
- **本轮零新增实现定义值**（QA 验证轮，产品零改动）。

## 二、当前产出情况（QA v148-qa）

### 本会话产物清单
- `project/qa/runners/suite_ui_pieces.py`（**新增，05 规程可执行化**，7 用例）：
  P 键分块弹窗端到端套件——64×19 弹窗几何/四段内容/56×16 钳制（UP-01）、暂停/完成
  网格四色分布 + 图例恒显（UP-02）、下载中 🟦/🟩 分布 + 暂停清零（UP-03）、2000 块
  通用滚动条 + 键盘滚动全键位（UP-04）、功能键无动作 + 关闭通道（UP-05）、四拒绝
  臂 toast 逐字（UP-06）、滚轮/拖拽/重开居中/点击外部无操作（UP-07）。修复过程
  五点：图例行底部扫描区间、24 MiB=24 块口径（24000000 仅 23 块）、20% 同时完成点
  门槛（4 并发 ~16.7% 四块同刻）、2 GiB 稀疏真实文件替代 swapsize 注入（后者物化
  进内存 GB 级即爆）、UP-06 各臂独立 HOME（会话恢复互染）与 ④臂 416 不可达登记。
- `project/qa/runners/suite_tui_display.py` 等 8 套件 + `harness.py`：v1.30–v1.32
  口径同步（版本锚/对话框标题/页签键 ]/[/T、TD-07 重写、PC-09 Esc 口径、滚动条
  字形、帮助行数、点外关闭类——v146 登记欠账全部消化）。
- `project/qa/05-ui-pieces-qa.md`：规程同轮同步——环境判据修订（piece-dl8.bin
  24 MiB=24 块口径、piece-big.bin 2 GiB 稀疏=2000 块≈65 行网格、原 520 块仅 2 行
  滚程不足以驱动 PgDn/End/Home 且 14×34 算术与实测 31 块/行不符）+ UP-06 ④ 判据
  修订（0 字节资源探测恒 416 → D35 臂先中，第 4 臂端到端不可达由产品单测承载，
  套件 S 登记口径落套件头注）。
- `project/qa/01-*-qa.md / 02/03/04-*-qa.md`：随套件同步的规程期望行（同轮一致，
  不擅改产品；含 PC-09/feature 场景 09 旧口径待裁决注记）。
- `.work/tmp/crap_scan_v148.py` + `lcov_v148.info` + `cpd_v148.txt`：度量产物
  （会话级，口径见脚本头注）。
- 工具侧经验沉淀（宪法第三章，7 条）：engineering.md 4 条（fixture 注入参数
  物化进内存→大文件稀疏真实文件路径；多臂用例共享 HOME 会话恢复互染；wait_for
  锚点与页签头常驻文本撞车；并发均分带宽同时完成点门槛 + RFC 7233 0 字节 416
  环境判据校验）；notes/rust.md 2 条扩展（llvm-cov 分块 --no-report 跨构建指纹
  一致性 + 报告对象列表污染按 mtime 清理；PMD 7 GitHub 不可达时 Maven Central
  pmd-cli+pmd-rust 组装路径）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 基线复核（动工前） | 4200P/0F + clippy 0 + fmt clean + arch 15/15（与 v146 精确一致） | `cargo test --all-features` 等（本轮前段已执行并记录于 plan） |
| e2e 端到端（14 套件） | **167P / 0F**：retry_backoff 9、config_template 5、throttle_proxy 5、persistence_config 9、add_task 19、download_engine 16、integrity_check 12、modify_task 9、named_proxy 18、resume_sidecar 14、slots_queue 15、tui_display 20、ui_conns 18、ui_pieces 7 | `python3 suite_<name>.py`（qa/runners/，逐套件） |
| 插桩全量测试（覆盖率口径） | 分块累计 **4186P/0F**（555 bin + 45 fixture/proxy + 943/803/1840 集成三批；+14 doctest = 4200 闭合） | `cargo llvm-cov --all-features --branch --lcov --no-report`（按 15 目标分块）+ `report` |
| 覆盖率读数 | 行 **94.9%**（17359/18295）、分支命中 **94.1%**（2092/2224）（src 含测试区口径） | 解析 `.work/tmp/lcov_v148.info` |
| CRAP 受限近似（首扫基线） | 产品区 1193 逻辑函数；**CRAP>30 共 9 处**（8 产品 + 1 fixture）+ 豁免区 2 处；阳性对照 Top5 经源码决策点复核 | `python3 .work/tmp/crap_scan_v148.py`（口径见头注） |
| DRY（CPD） | **125 tile，与 v146 精确一致；本轮新增区零 tile**（产品零改动自动成立）；阳性对照（同构文件对）报重复正常 | `pmd cpd -l rust --minimum-tokens 50 --dir src`（pmd 7.28.0，Maven Central 组装） |
| UP 套件修复后复跑 | 7/7 P（修复前 2P/5F，五点根因全修复） | `python3 suite_ui_pieces.py` |

### 对账口径
- 测试基线：上轮 handoff 记 4200P/0F（v146）。本轮产品零改动 → 基线复核 4200P/0F
  成立；插桩分块 4186P + doctest 14 = 4200，口径闭合（插桩分块不跑 doctest 段）。
- e2e 对账：本轮 167P（14 套件，逐套件数字如上）；前轮 QA 交接 165P/0F/0S（13 套件，
  v1.29 口径快照）——两轮之间规程历经 v1.30/31/32 改版（页脚收敛退役 UP-08、
  TD-07/PC-09 重写、UP 套件可执行化 +7 等），套件面随规程同步演化，两数字非同口径
  可比基线；**167P 为当前有效总账**。
- CPD 对账：125 tile（v146）→ 125 tile（本轮，pmd 7.28.0 同口径复跑），产品零改动
  一致性成立；本轮新增区零 tile。
- 覆盖率对账：前轮 QA 记 94.37%（v1.29 口径、13 套件时代的 bin harness 口径）；
  本轮 94.9%/分支 94.1%（全 15 目标插桩口径），口径变化（分块全量 vs 单 bin）
  已在登记中注明，不直接对比。

### 待办与待批（未决项）
- **D40 微决策待操作者裁决（沿袭）**：宽限期 300ms / 加 `dns` 配置键等改判。
- **PC-09/feature 场景 09 旧口径待裁决（本轮登记）**：PC-09 Esc 行为（v1.32 口径
  无操作+Ctrl+C）与 feature 场景 09 旧文字残留并存，规程行已同步、场景文字待
  specifier 轮统一（不擅改 features，本轮只登记）。
- **CRAP>30 9 处处置**：移交 hardender 轮（登记见受限项；QA 轮不修产品）。
- 沿袭待办：APS 机检补跑（本沙箱 GitHub 不可达）、v147 flake 预算放宽。

### 移交建议
- 本轮最终独立验证已闭环：14 套件 e2e 全绿 + 欠账全量消化 + CRAP/CPD 基线落账 +
  覆盖率读数复核，**01 期可交付操作者审阅**（`bin/swarm complete` 产物归档）。
- 若继续 01 期收尾：残余三件移交项（CRAP 9 处、PC-09 场景文字统一、D40 裁决）
  建议下一轮 hardender（CRAP 处置）+ specifier（场景文字）小会话处理。
- 02 期（BT）启动时按 mission/phase-02 详述建档，UP 套件预留 BT 用例增补位
  （05 规程已注）。
- 移交仅为建议，不构成自动流转（宪法第一章）。

By QA.
