# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-v134-uifix3-nativecursor-impl（2026-10-24）
> 上游交接：project/handoff.md（coder v134-uifix2-impl 交接：三类 UI bug 二次修复交付）
> 当轮任务：操作者直判 TUI 输入框体验升级——①评估成熟 TUI 输入框 crate 接入；
> ②左右移动光标时后续字符平移；③鼠标点击不在点击处落光标；④光标不闪烁。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.34，分期模式）+ `project/mission/phase-01.md`（v1.34）。
- 本轮后源码状态：v1.34 全条款落地 + 三轮 UI bug 修复（首轮 4336P/0F → 二轮
  4367P/0F → 本轮 **4388P/0F**，15 目标）；规格面不变（mission v1.34 / features
  14 份 / qa 规程 14 份——本轮三项修复均属既有条款实现缺陷回收与展示机制升级，
  不新增/不改写条款）。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.34 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | 历轮口径见各批注记；v132 轮 3 份 + v149 轮 5 份 feature APS 机检沿袭未跑 |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F（v128 口径） |
| architect（v145-arch） | supervisor 六文件拆分 + 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 | 已交付 | 3363P/0F、clippy 0、fmt 0 |
| QA（ezr-qa-final / v148-qa） | 13+1 套件 e2e 可执行化（qa/runners/ 14 份）+ 欠账消化 | 已交付 | e2e 167P/0F、插桩全量 4200P/0F、行覆盖 94.9%、CPD 125 tile |
| specifier（v129–v140/v149） | FR-01-106…112 + m2/m3 对齐 + v1.34 规格轮（v149-demo-sync） | 已交付 | v149 轮全部逐字断言经 ezr-demo 源码核证；D41–D43 登记 |
| coder（v129–v140 系列） | FR-01-106 实现 + v1.30/v1.31 UI 改版 + m3 对齐 + FR-01-112 内置 DNS | 已交付 | 4162P/0F（v140 口径） |
| cleaner（v141）/ architect（v146） | dns 清理 + 四阶段评审 | 已交付 | 4200P/0F、arch 15/15 |
| coder（v134-demo-sync-impl） | v1.34 全条款实现（FR-01-113…116 + 修订十二条） | 已交付 | 4300P/0F、clippy 0、fmt clean |
| coder（v134-uifix-impl） | 三类 UI bug 首轮修复（统计行排版 / 流式单连接并发明细 / 弹窗深度断链） | 已交付 | 4336P/0F、clippy 0、fmt clean |
| coder（v134-uifix2-impl） | 三类二次修复：①连接在传判定单源 `conn_transferring`（数据面/展示面两门收口，流式单连接速度立起）；②弹窗文本字段光标编辑（Dialog.cursor 字符偏移 + focus_field 单源 + 方向键分流）；③详情 URL 行 break_links_deep 深度断链 | 已交付 | 4367P/0F、clippy 0、fmt clean、gen 守卫绿 |
| **coder（v134-uifix3-nativecursor-impl，本轮）** | **输入框光标体验升级（原生终端光标替代内联字形）：①渲染层 field_display/cursor_field_display 两同构实现收口 field_window 单源（聚焦=光标锚定取窗、未聚焦=尾窗、CJK 原子、发布累积列半开边界 cols）；②内联「▏」字形退役（字符位移根因），聚焦文本行发布 `dlg_cursor_screen` 屏幕位，ui::draw 收口按 z 序 `Frame::set_cursor_position` 画终端原生光标；③鼠标层纯函数 `click_cursor_offset`（消费渲染层窗口几何，宽字符左右半格分置、越界钳制）+ on_mouse_dialog 文本字段点击 = focus_field + 光标落点击处；④启动序列抽 enter_tui_sequence 可测内核 + `EnableBlinking` 显式请求闪烁** | **已交付** | **4388P/0F（15 目标，+21 vs 4367 基线，两轮复跑）、clippy 0、fmt clean、gen 守卫绿** |

### 遗留受限项（当前有效）
- **APS 机检沿袭未执行**：v132 修订 3 份 + v149 修订 5 份 feature 未过
  `gherkin-ir-dry-checker`；本沙箱 GitHub 不可达，待环境可用时一并补跑。
- **qa/runners/ e2e 套件未随 v1.34 同步（移交 QA）**：可执行套件仍为 v1.30–v1.32 口径，
  对 v1.34 改版场景（尺寸/文案/几何/活跃口径）会 F，属预期——待 QA 轮同步。
- **真实 TCP stub e2e 时序脆弱（v147 沿袭）**：默认 `--test-threads` 下 hardening_g_v147
  内 engine 级滴流下载测试偶发假失败（CPU 竞争超时）；`--test-threads=2` 全绿即
  环境竞争非回归。收官验证用 `cargo test -- --test-threads=2`（本轮两轮复跑通过）。
- **dns.rs 覆盖率残余 3 行 / 观察受限突变体 / ezr-proxy 测试基建豁免（沿袭登记）**：状态不变。
- **CRAP>30 9 处处置（沿袭，移交 hardender）**：状态不变。
- **D40 / PC-09-场景09 待操作者裁决（沿袭）**：状态不变。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D43。
- 本轮无新增（原生光标为显示机制升级：光标屏幕位 = 值区起点 + w(pre) 属既有
  「焦点文本行显示光标」契约的承载方式变更，字段容量/过滤/焦点矩阵契约不变；
  点击半开区间映射 = 终端鼠标以格为单位的标准惯例语义）。

## 二、当前产出情况（coder v134-uifix3-nativecursor-impl）

### crate 评估结论（对应操作者问题①）
- 成熟 TUI 输入框 crate 存在（**tui-input**（ratatui 社区生态，Input 状态机 +
  handle_event）、**tui-textarea**（多行编辑）），**但本轮三项缺陷均不在状态机层**：
  字符位移 = 内联字形占列的渲染层后果；点击不定位 = 鼠标层无列→偏移映射；
  不闪烁 = 终端层无原生光标/闪烁请求。tui-input 官方示例的渲染同样要手动
  `Frame::set_cursor_position`——接入 crate 不修任何一项，反要迁移容量上限/
  十六进制过滤/NONE 锁定/焦点迁尾等已单源且被全量测试锁定的字段语义
  （4367P 基线全数重写）。故本轮直接修根因（原生光标 + 点击映射），
  未引入新依赖；评估结论与依据随本 handoff 留档供操作者复核。

### 本会话产物清单
- `src/app/mod.rs`：新增 `FieldWin`（渲染层→鼠标层的窗口几何单源载荷：
  value_x/left_cut/cols 累积列半开边界/win_lo/win_hi）+ App 字段
  `dlg_field_wins: Vec<Option<FieldWin>>`（与 dlg_field_rects 平行、非文本行
  None）与 `dlg_cursor_screen: Option<(u16,u16)>`。
- `src/ui/dialog.rs`：`field_display` + `cursor_field_display` 两同构实现收口
  `field_window` 单源（聚焦锚=光标切分 pre/post、未聚焦锚=字段尾尾窗——原
  field_display 按字符数计的 CJK 溢出口径一并修正为宽度原子；超宽不动点取窗
  逻辑保持）；内联「▏」字形退役（pre/post 直接拼接，聚焦预算 −1 归还全额）；
  渲染循环回填 FieldWin（value_x = rect.x + 9）与聚焦行 `dlg_cursor_screen`
  （= value_x + w(pre)）；测试改造 2 项（field_window 三态/cols 自洽/CJK 原子/
  未聚焦尾窗）+ 新增渲染集成 e2e（字形不在位、光标屏位、TestBackend
  `get_cursor_position` 读回、窗口几何回填、光标列随 cursor）。
- `src/ui/mod.rs`：draw() 开头每帧清空 `dlg_cursor_screen`（防陈旧位跨帧残留）；
  收口处在「对话框为顶层浮层（pieces/help/about/val 均未打开）」时经
  `Frame::set_cursor_position` 画原生光标（z 序门控——光标屏位不随图层遮挡消失）。
- `src/app/mouse.rs`：纯函数 `click_cursor_offset`（rel = 点击列 − value_x −
  左缘指示列；cols 半开区间定位：宽字符左半格=其前、右半格=其后；rel ≤ 0 钳
  win_lo、越末钳 win_hi、空字段恒 0）；on_mouse_dialog 文本字段点击 = focus_field
  （迁尾单源不变）+ 窗口几何在位时光标落点击处（覆盖迁尾值）；dlg_field_wins
  宽松读取（`get(i).and_then(flatten)`，None 回退旧语义）；测试 +2
  （映射纯函数穷举 + 点击定位 e2e 三场景）。
- `src/main.rs`：启动序列抽 `enter_tui_sequence<W: Write>` 可测内核并追加
  `crossterm::cursor::EnableBlinking`（原生光标闪烁显式请求；Alacritty 等默认
  稳态终端不请求不闪）；restore 序列不动（离开备用屏即复位终端默认光标态）；
  模块 doc 同步；测试 +1（`Vec<u8>` 断言 `ESC[?12h` 在位且顺序 屏→鼠标→paste→闪烁）。
- `tests/hardening_v115.rs`：字段行几何测试 ▏ 断言迁移为原生光标口径
  （整屏无字形 + `dlg_cursor_screen == Some((35,16))` 精确锚点）；恰宽边界
  靶点 doc 更新（field_window 单源口径）。
- 经验沉淀（宪法第三章）：`packs/_common/notes/rust.md` 第 9 节新增 4 条——
  原生光标根治字形顽疾（换 crate 不解决）/ 点击映射以渲染层几何为单源 /
  光标屏位 z 序门控 / 平行表 + 宽松读取的几何发布形态。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量测试 | **4388P / 0F**（15 目标；基线 4367 → +21，两轮复跑） | `cargo test -- --test-threads=2`（project/ezr，exit 0 ×2） |
| clippy | 0 警告（全目标） | `cargo clippy --all-targets` |
| fmt | clean（nightly） | `cargo fmt --check` |
| gen-drift 守卫 | update_conn_stats + detail 副本逐字一致 | `cargo test --test hardening_engine_backfill gen_ucs` + `cargo test --test hardening_ui_backfill gen_fixtures` |
| 原生光标渲染 e2e | 整屏无 ▏、光标屏位 (43,16)、TestBackend 读回一致、光标列随 cursor | `cargo test --bin ezr dialog_native_cursor` |
| 点击定位 e2e | 聚焦+光标落点击列、越末钳字段尾、未聚焦行点击映射覆盖迁尾 | `cargo test --bin ezr dialog_field_click` |
| 映射纯函数 | 窄字符/宽字半格/左缘指示/越界/空字段 全分支 | `cargo test --bin ezr click_cursor_offset` |
| 取窗单源 | 三态窗口 + cols 自洽 + CJK 原子 + 未聚焦尾窗 | `cargo test --bin ezr field_window` |
| 启动序列 | `ESC[?12h` 在位且顺序正确 | `cargo test --bin ezr enter_tui_sequence` |
| 属性套件 | 3P/0F（光标口径不变） | `cargo test --bin ezr prop_` |

### 对账口径
- 测试增量对账：4367 → 4388（**+21 = 本轮新增 6 测试 × 挂载上下文展开**：
  dialog.rs 渲染集成 e2e +1 × 4 上下文（bin + hardening_v115/ui_backfill/
  engine_backfill 挂载 ui 树）= +4；mouse.rs 点击两测 +2 × 8 上下文（bin + 7 个
  挂载 app 树的 hardening_* crate）= +16；main.rs 启动序列 +1 × 1（bin 独有）=
  +1；helper_tests 两测更名等量替换 ±0。分项 586+30+15+128+497+424+419+2+16+
  171+393+254+407+526+520 复算自洽 = 4388）。
- 三项修复均无规格条款改动：②③④ = FR-01-01/87 对话框文本字段输入交互的
  可用性缺陷回收与承载方式升级（光标模型/字段容量/消费矩阵契约不变，显示面
  从字形承载改为终端原生光标承载）；① = 评估结论（零代码、零依赖变更）。
- 沿袭口径：qa/runners e2e 167P/0F 为 v148 基线态（未随本轮运行，移交 QA 同步）。

### 待办与待批（未决项）
- **D40 微决策 / PC-09-场景09 待操作者裁决（沿袭）**：状态不变。
- **APS 机检欠账 8 份（沿袭）**：GitHub 可达后补跑。
- **qa/runners e2e 套件同步（移交 QA）**：随 v1.34 改版场景同步可执行套件后运行。
- **本轮无阻塞**：三项缺陷根因均定位在渲染/鼠标/终端层（字形占列位移 / 无列→
  偏移映射 / 无原生光标与闪烁请求），无规格歧义；crate 评估结论如上，供操作者
  复核「接 crate vs 修根因」取舍。
- **原生光标闪烁的终端依赖（须知，非阻塞）**：闪烁由终端承载（本轮已显式
  EnableBlinking）；个别禁用光标闪烁的终端配置（如某些 tmux/终端偏好设置）
  下光标稳态显示——属终端侧配置，应用侧无法强制。

### 移交建议
- 建议下一步运行 `six-pack/QA`：同步 qa/runners/ 可执行套件至 v1.34 口径时顺带
  覆盖本轮交互面（弹窗聚焦文本行原生光标位、字段点击定位、方向键光标移动、
  超宽字段锚定窗口）。
- 随后 `six-pack/hardender` 可对 `field_window`/`click_cursor_offset`/
  `cursor_insert`/`break_links_deep` 做变异加固（两个新单源点均为纯函数，
  靶点几何清晰；`dlg_cursor_screen` 发布与 z 序门控属渲染装配，建议以
  TestBackend 断言守卫）。
- `six-pack/cleaner` 如先行，可复核 `field_window` 未聚焦臂与原 field_display
  的等价性登记（ASCII 同形、CJK 为有意行为修正——原实现按字符数取尾会溢出
  显示预算）。
