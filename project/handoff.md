# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-v134-uifix2-impl（2026-10-24）
> 上游交接：project/handoff.md（coder v134-uifix-impl 交接：三类 UI bug 首轮修复交付）
> 当轮任务：操作者直判三类 UI bug 二次修复——①不支持断点续传的下载中任务并发
> panel 速度恒 0；②弹窗输入框方向键误作 Tab（应在框内移动输入光标）；③任务
> 详情 www. 起始域名悬停仍现终端默认下划线（双线）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.34，分期模式）+ `project/mission/phase-01.md`（v1.34）。
- 本轮后源码状态：v1.34 全条款落地 + 两轮 UI bug 修复（首轮 4336P/0F → 本轮
  **4367P/0F**，15 目标）；规格面不变（mission v1.34 / features 14 份 / qa 规程
  14 份——三类二次修复均属既有条款实现缺陷回收，不新增/不改写条款）。

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
| **coder（v134-uifix2-impl，本轮）** | **三类二次修复：①连接在传判定单源 `conn_transferring`（model）——数据面 update_conn_stats 与展示面 sample_conn_speed_display 两门收口，流式单连接（cap=0、done>0）展示速度正常立起（原展示采样门漏流式臂恒 0）；gen 副本同步 + 流式 e2e 补速度断言；②弹窗文本字段光标编辑（Dialog.cursor 字符偏移 + focus_field 焦点切换单源 + routing 光标算子单源 + 方向键分流：文本字段内 Left/Right ±1、Up 行首、Down 行尾不跳焦点，非文本焦点保持导航；Tab/BackTab 恒跳焦；text_char/text_backspace/push_hex_capped/paste 光标处生效；modify_dialog_char 内联 append 收口 text_char；渲染 ▏落光标列 + 超宽光标锚定取窗 + … 指示）；③详情 URL 行 break_links → break_links_deep 逐字符深度断链（www. 形态识别归一化后仍命中的终端补强）；break_links 退役删除** | **已交付** | **4367P/0F（15 目标，+31 vs 4336 基线）、clippy 0、fmt clean、gen 守卫绿（update_conn_stats + detail 两副本）** |

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
- 本轮无新增（三类修复均在既有产品契约值域内：在传判定 = 既有数据面判别式
  单源化；光标 = 字符偏移 0..=字段长、Up/Down 映射单行字段行首/行尾；断链 =
  零宽 VS15 U+FE0E 强度升级，显示宽零影响）。

## 二、当前产出情况（coder v134-uifix2-impl）

### 本会话产物清单
- `src/model/task.rs`（bug①）：新增 `conn_transferring(cap, done)` 在传判定门
  单源（pub(crate)，Connection/ConnView 两同构形态共用谓词；判别式 = 持块未写满
  或流式 done>0）。测试 +1（单源四臂穷尽）。
- `src/model/mod.rs`（bug①）：`conn_transferring` re-export（跨层共享谓词路径）。
- `src/app/engine/mod.rs` + `tests/gen/update_conn_stats_stripped.rs`（bug①）：
  数据面在传门改单源调用；剥头副本逐字同步（gen 守卫绿）。
- `src/app/engine/tick.rs`（bug①）：展示采样 `sample_conn_speed_display` 在传门
  改单源调用（原内联 `cap > 0 && done < cap` 漏流式臂——速度恒 0 根因）；采样
  doc 零值语义更新。测试 +1（流式单连接展示速度立起）。
- `tests/hardening_engine_backfill.rs`（bug①）：流式 e2e 补展示速度回归断言
  （1s 采样节拍后 conn_speed_of > 0）。
- `src/ui/detail.rs`（bug③）：URL 行断链升级 `break_links_deep`（逐字符 VS15）；
  模块 doc ⑤ 同步。
- `src/ui/text.rs`（bug③）：`break_links`（scheme 锚点版）随最后消费者切换退役
  删除；`break_links_deep` doc 更新（消费者：详情 URL 行 + 弹窗内容）；link_break_spans
  分段测试改手工 VS15 样例、deep 测试补 www 断言。
- `tests/gen/detail_stripped.rs` + `tests/hardening_ui_backfill.rs`（bug③）：
  detail 副本重生成；壳内 text 转发改 break_links_deep；新增详情 URL 行缓冲区级
  深度断链回归测试（www 起始域名，行文本无裸片段 + 值区每格携带 VS15）。
- `src/app/mod.rs`（bug②）：`Dialog.cursor` 字段（焦点文本字段字符偏移）+
  cfg(test) 门面 re-export 补 text_field_at/TextField。
- `src/app/dialog_keys/routing.rs`（bug②）：字段映射单源 `text_field_at`/`TextField`；
  光标算子（cursor_insert/backspace/left/right，char 边界安全）；`Dialog::focus_field`
  焦点切换单源（光标随迁字段尾）+ `cursor_field_end`；text_char/text_backspace/
  push_hex_capped 改光标语义（消费矩阵不变）。
- `src/app/dialog_keys/mod.rs`（bug②）：on_dialog_key 方向键分流（文本字段内
  光标移动不跳焦点 / 非文本焦点保持导航）；Tab/BackTab 走 focus_field；下拉
  Tab/BackTab 跳焦同步；modify_dialog_char 并发臂收口 text_char（原内联 append
  分叉）。测试改造 4 项 + 新增方向键光标专项 e2e（含 Modify 预填字段中位插入）。
- `src/app/dialogs.rs`（bug②）：三构造器 cursor 初始化（Modify 预填并发尾）；
  checksum_from_input 回焦走 focus_field；测试夹具补 cursor。
- `src/app/mouse.rs`（bug②）：字段/按钮点击聚焦走 focus_field。
- `src/app/paste.rs`（bug②）：dlg_apply_paste 光标处插入（text_field_at 单源
  路由；push_capped 退役）；测试 +1（中位插入与光标随迁）。
- `src/app/testutil.rs` / `src/ui/mod.rs` / `tests/hardening_g_appcore.rs`（bug②）：
  Dialog 夹具补 cursor 字段。
- `src/ui/dialog.rs`（bug②）：聚焦文本行光标 ▏ 渲染在光标列（原恒缀尾）；
  `cursor_field_display` 光标锚定取窗（超宽 + … 指示 + 指示占列不动点重取，
  CJK 原子；预算 −1 容光标字形）；测试 +1（三态窗口）。
- `src/property_tests.rs`（bug②）：text_char/text_backspace 属性改光标口径
  （夹具置光标到靶字段尾；新增光标 ≤ 字段长不变量与退格随迁断言）。
- `tests/hardening_v115.rs`（bug②）：field_display 恰宽边界靶改经未聚焦行触发
  （聚焦行显示面改光标窗预算 −1 语义变化）。
- 经验沉淀：`packs/_common/notes/rust.md` 第 9 节新增「append/pop 式文本输入升级
  光标编辑的迁移清单」条目（char 边界/双向钳制/焦点单源/多路由同源/渲染预算/
  测试迁移七点，宪法第三章；成对判定门收单源属 engineering.md 既有「重复实现
  收口单源」条款覆盖，未重复沉淀）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量测试 | **4367P / 0F**（15 目标；基线 4336 → +31，两轮复跑） | `cargo test -- --test-threads=2`（project/ezr，exit 0 ×2） |
| clippy | 0 警告（全目标） | `cargo clippy --all-targets` |
| fmt | clean（nightly） | `cargo fmt --check` |
| gen-drift 守卫 | update_conn_stats + detail 副本逐字一致 | `cargo test --test hardening_engine_backfill gen_ucs` + `cargo test --test hardening_ui_backfill gen_fixtures` |
| 流式速度 e2e | 明细行 1 + 徽标 1/1 项 + **conn_speed_of > 0** | `cargo test --test hardening_engine_backfill streaming_single_thread` |
| 弹窗光标 e2e | 方向键不跳焦、中位插入/退格随光标、钳制正确 | `cargo test --bin ezr arrow_keys_move_cursor` |
| 详情断链 e2e | www URL 行文本无裸片段、值区每格 VS15 | `cargo test --test hardening_ui_backfill detail_url_value_deep_broken` |
| 属性套件 | 3P/0F（光标口径） | `cargo test --bin ezr prop_` |

### 对账口径
- 测试增量对账：4336 → 4367（+31 = 本轮新增 9 测试 ×（产品 bin 1 + 挂载该模块的
  加固 crate 数）机制展开 − detail 内联测试迁移到 hardening_ui_backfill ±0；
  分项 582+30+15+128+494+422+417+2+16+171+391+254+405+523+517 复算自洽）。
- 三类修复均无规格条款改动：①=FR-01-102 连接级速度窗口既有数据面判别式的
  展示面缺陷回收（单源收口）；②=FR-01-01/87 对话框文本字段输入交互的可用性
  缺陷回收（方向键/Tab 语义分流 + 光标编辑，未改字段容量/过滤/消费矩阵契约）；
  ③=FR-01-113 ④断链机制的强度升级（机制不变、覆盖面加深，scheme 锚点版退役）。
- 沿袭口径：qa/runners e2e 167P/0F 为 v148 基线态（未随本轮运行，移交 QA 同步）。

### 待办与待批（未决项）
- **D40 微决策 / PC-09-场景09 待操作者裁决（沿袭）**：状态不变。
- **APS 机检欠账 8 份（沿袭）**：GitHub 可达后补跑。
- **qa/runners e2e 套件同步（移交 QA）**：随 v1.34 改版场景同步可执行套件后运行。
- **本轮无阻塞**：三类 bug 根因均定位到实现层（展示面门漏臂 / 方向键同臂跳焦
  + 无光标模型 / 弱断链对 www 形态无效），无规格歧义。

### 移交建议
- 建议下一步运行 `six-pack/QA`：同步 qa/runners/ 可执行套件至 v1.34 口径时顺带
  覆盖两轮修复的回归面（统计行列序、流式单连接明细与**面板速度列**、弹窗内容
  断链、详情 URL 断链、弹窗光标编辑语义）；随后 hardender 可对
  `conn_transferring`/`cursor_insert`/`cursor_field_display`/`break_links_deep`
  做变异加固（本轮新增单源点均为纯函数，靶点几何清晰）。
