# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-v134-demo-sync-impl（2026-10-23）
> 上游交接：project/handoff.md（specifier v149-demo-sync 交接：mission v1.34 规格轮交付）
> 当轮任务：实现 v1.34 规格条款到 project/ezr（FR-01-113…116 全量 + 既有条款 padding/
> 活跃口径/文案修订），产品源码与测试同步落地。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.34，分期模式）+ `project/mission/phase-01.md`（v1.34）。
- 本轮后源码状态：v1.34 全条款落地（15 目标 4300P/0F）；规格面不变
  （mission v1.34 / features 14 份 / qa 规程 14 份）。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.34 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | 历轮口径见各批注记；v132 轮 3 份 + v149 轮 5 份 feature APS 机检沿袭未跑 |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F（v128 口径） |
| architect（v145-arch） | supervisor 六文件拆分 + 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 | 已交付 | 3363P/0F、clippy 0、fmt 0 |
| QA（ezr-qa-final / v148-qa） | 13+1 套件 e2e 可执行化（qa/runners/ 14 份）+ 欠账消化 | 已交付 | e2e 167P/0F、插桩全量 4200P/0F、行覆盖 94.9%、CPD 125 tile |
| specifier（v129–v140/v149） | FR-01-106…112 + m2/m3 对齐 + **v1.34 规格轮（v149-demo-sync）** | 已交付 | v149 轮全部逐字断言经 ezr-demo 源码核证；D41–D43 登记 |
| coder（v129–v140 系列） | FR-01-106 实现 + v1.30/v1.31 UI 改版 + m3 对齐 + FR-01-112 内置 DNS | 已交付 | 4162P/0F（v140 口径） |
| cleaner（v141）/ architect（v146） | dns 清理 + 四阶段评审 | 已交付 | 4200P/0F、arch 15/15 |
| **coder（v134-demo-sync-impl，本轮）** | **v1.34 全条款实现：FR-01-113 字段值弹窗（ui/val_popup.rs 新增 48×12/Y 复制/悬停下划线/断链）、FR-01-114 [✗] 关闭按钮（btn.rs draw_close_btn + 七弹窗接入 + 鼠标臂）、FR-01-115 padding 体系全落点（header/footer/list/detail/conns/dialog×3/delete/pieces/help）、FR-01-116 大小未知口径（size_known/?%/走马灯/? B/ETA -/分块 -）、FR-01-96/81 活跃口径改版（ConnPhase 单源 + x/y 徽标 + 明细活跃过滤）、FR-01-107 帮助九组 50 行、FR-01-80/101 字段名精简/热区/断链、FR-01-25/87/103/106 尺寸改版；测试同步：help 三帧取样、engine_backfill ConnPhase 探针、ui_backfill 壳转发 + delete 金样、v115 几何金样 ×5、gen 剥头副本重生成 ×2、dialog avail −13 溢出修正** | **已交付** | **4300P/0F（15 目标，+100 vs v148 基线）、clippy 0、fmt clean** |

### 遗留受限项（当前有效）
- **APS 机检沿袭未执行**：v132 修订 3 份 + v149 修订 5 份 feature 未过
  `gherkin-ir-dry-checker`；本沙箱 GitHub 不可达，待环境可用时一并补跑。
- **qa/runners/ e2e 套件未随 v1.34 同步（移交 QA）**：可执行套件仍为 v1.30–v1.32 口径，
  对 v1.34 改版场景（尺寸/文案/几何/活跃口径）会 F，属预期——待 QA 轮同步。
- **真实 TCP stub e2e 时序脆弱（v147 沿袭，本轮实证补充）**：默认 `--test-threads` 下
  hardening_g_v147 内 engine 级滴流下载测试偶发假失败（CPU 竞争超时，每次挂的用例不同）；
  单过滤重跑秒过、`--test-threads=2` 全绿即环境竞争非回归。收官验证已改用
  `cargo test -- --test-threads=2`（4300P/0F）。
- **dns.rs 覆盖率残余 3 行 / 观察受限突变体 / ezr-proxy 测试基建豁免（沿袭登记）**：状态不变。
- **CRAP>30 9 处处置（沿袭，移交 hardender）**：状态不变。
- **D40 / PC-09-场景09 待操作者裁决（沿袭）**：状态不变。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D40、D41–D43（v149 轮登记）。
- 本轮无新增（全部产品契约值以 v1.34 规格 + 既有 D 表为准，实现逐字对齐 demo 源码）。

## 二、当前产出情况（coder v134-demo-sync-impl）

### 本会话产物清单
- `project/ezr/src/ui/val_popup.rs`（新增）：FR-01-113 字段值弹窗——48×12 模态、快照
  完整值 wrap、仅垂直滚动条、Y 复制 + toast、拖拽移动关闭即清、Esc/q/Q/[✗] 关闭、
  L6 最上层、break_links 断链显示。
- `src/ui/btn.rs`：draw_close_btn（FR-01-114，D41 配色拆分：中括号同边框、✗ 悬停/按压红粗）。
- `src/ui/{about,conns,delete,dialog,footer,header,help,list,pieces,task_lines,text,mod}.rs`：
  [✗] 七弹窗接入、FR-01-115 S1/S2/S3 padding 全落点、FR-01-116 大小未知渲染
  （?%/走马灯 marquee_bar/? B/ETA -/分块 -）、帮助九组 50 行（S3 缩进 + 可视 20）、
  x/y 徽标与活跃明细、字段名精简 + 悬停下划线 + 值热区回填。
- `src/model/{task,mod}.rs`：ConnPhase 三态派生（cap 权威/cum 观测映射）、
  Task::size_known/eta_secs、supports_pieces 分块门槛。
- `src/app/{mod,keys,mouse}.rs`：val_open 状态与键/鼠标臂、DetailField 值热区、
  popup_close_rect、active_conn_count/ids 活跃口径收口、头部并发活跃计数。
- `src/ui/text.rs`：break_links VS15 断链 + link_break_spans 分段渲染 + 零宽宽度口径
  （pub(crate) 供加固壳转发）。
- 测试同步：
  - `src/ui/help.rs` 帮助测试改三帧取样（首 0..19 / 中 10..29 / 末钳 30..49——50 行内容
    末页视窗不再覆盖中段组）；
  - `tests/hardening_engine_backfill.rs` 活跃口径探针重写（ConnPhase：持块即活跃 +
    cap 权威 + 非明细态恒 0）；
  - `tests/hardening_ui_backfill.rs` gen 壳补转发（DIM2/panel_border_style/RED/
    break_links 族）、hot 按产品新口径重写、delete 金样按 x+2/提示行退役更新；
  - `tests/gen/{detail,btn}_stripped.rs` 随产品源 + fmt 重生成；
  - `tests/hardening_v115.rs` 五个几何金样按 74×12/x+2/px+9 更新。
- `src/ui/dialog.rs`：字段行值宽预算 avail = inner.width−13（宽−4 改版漏改点修正，
  防 Paragraph 右缘截尾 1 列）。
- 工具侧经验沉淀（宪法第三章）：`packs/_common/notes/rust.md` 新增 4 条
  （ld SIGBUS≈磁盘满归因与 target/incremental 清理次序、cargo fmt 后重生成剥头副本的
  次序纪律、剥头副本壳符号转发与 pub(crate) 放宽最小化、真实 TCP stub e2e 的
  test-threads 限流鉴别法）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量测试 | **4300P / 0F**（15 目标；基线 4200 → +100） | `cargo test -- --test-threads=2`（project/ezr） |
| clippy | 0 警告（全目标） | `cargo clippy --all-targets` |
| fmt | clean | `cargo +nightly fmt --check` |
| 帮助内容行数 | 50（D43 锚定） | `cargo test help`（help_lines_len 断言） |
| gen-drift 守卫 | 4 副本逐字一致 | `cargo test --test hardening_ui_backfill gen_fixtures` |

### 对账口径
- v1.34 条款覆盖对账：FR-01-113/114/115/116（新增四条）全实现；FR-01-80/81/25/87/
  94/96/101/103/106/107/108/109（修订十二条）全落点——逐条对应产物清单与渲染测试。
- 测试增量对账：4200 → 4300（+100 = v1.34 新增渲染/交互/口径单测 + 三帧取样重写后的
  help 组），各二进制分项：571+30+15+128+483+417+412+2+16+169+386+252+512+507+400。
- 沿袭口径：qa/runners e2e 167P/0F 为 v148 基线态（未随本轮运行，移交 QA 同步）。

### 待办与待批（未决项）
- **D40 微决策 / PC-09-场景09 待操作者裁决（沿袭）**：状态不变。
- **APS 机检欠账 8 份（沿袭）**：GitHub 可达后补跑。
- **qa/runners e2e 套件同步（移交 QA）**：随 v1.34 改版场景同步可执行套件后运行。
- **本轮无阻塞**：规格与 demo 源码两来源齐备，实现无歧义积压。

### 移交建议
- 建议下一步运行 `six-pack/QA`：同步 qa/runners/ 可执行套件至 v1.34 口径（尺寸/文案/
  几何/活跃徽标/字段值弹窗/[✗] 通道）并做独立验证；随后 hardender 可对新增 UI 面
  （val_popup/draw_close_btn/marquee_bar）做变异加固。
