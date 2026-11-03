# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（本轮已完成）→ 建议下一节点 six-pack/specifier（本轮交互判据归档）或 six-pack/cleaner
> 会话：ezr-coder-bug22（2026-11-03，bug22 轮 coder：修复 BT 添加/修改对话框
> 「下载限速/上传限速」聚焦光标左偏缺陷——终端光标压在最后字符上、无法退格删最后
> 字符；根因 = bt_text_row 光标列/点击 value_x 硬编码「标签 7 列」而限速两行标签宽
> 8 列；修为动态 w(label).max(7) 并补防回归测试；ezr-demo 对账无同款缺陷、产品源码
> 零改动，补守卫测试 + 文档同步）
> 上游交接：project/handoff.md（bug21 修订轮 coder 交接；本轮任务来源 = 操作者 chat
> 直启 bug 报告：BT 限速输入框聚焦光标应停最后字符后方 = ezr 全输入框通用输入模式）

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式 v1.39）+ `project/mission/phase-01…04.md`。
- phase-01（HTTP/HTTPS 内核与 TUI）收官；phase-02（BT 内核/做种/管理）已实现并经
  互操作夹具多轮收口。历轮 bug11–18 全部落地；bug21 轮四项 UI/交互修复已实现；
  bug21 修订轮（specifier）已把操作者两批裁决（限速二进制批准 + HTTP 上传限速退役
  FR-02-29 v1.10/D67②）落规格三层（mission/mission注记 + features 6 份 + QA 6 份）。
- **本轮（coder，bug22）**：BT 对话框限速两行聚焦光标左偏修复——`ui/bt_dialog.rs
  bt_text_row` 光标 x 与点击 value_x 由硬编码 `+9` 改 `w(label).max(7)` 动态化
  （与 HTTP 对话框 ui/dialog.rs 及 ezr-demo 同位点公式归一）；逻辑光标（focus_field
  → cursor_end）本就正确，缺陷纯在渲染/点击映射层。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01/02 及历轮 bug11–18） | features/ 20 份 + qa/ 对应套件 | 已收官/已交付 | 见历轮登记 |
| coder（phase-02 及历次收口轮，含 bug18/bug21） | `project/ezr/` 全量 | 已交付 | 见历轮登记 |
| specifier（bug21 修订轮） | mission 注记 + features 6 份修订 + QA 6 份同步 | 已交付 | APS dry-check 全过（见上轮登记） |
| **coder（bug21 修订轮）** | `project/ezr/`：bt_dialog.rs toast 统一 + 防回归单测新增 + engine.rs 三处测试观察环竞态修复 | 已交付 | bin 890P/0F + 集成 5943P/0F + clippy 0 警告 + fmt 干净（合计 6833P/0F） |
| **coder（本轮，bug22）** | `project/ezr/`：bt_text_row 标签宽动态化（光标 x + value_x 两处）+ 防回归渲染/点击测试；`ezr-demo/`：对账零产品改动 + 守卫测试 + README 记录 34 + 规范 §6.1 光标规则 bullet | **已交付** | **bin 891P/0F + 集成 12 目标 5946P/0F + demo 31P/0F + clippy 0 警告 + fmt 干净（ezr 全量合计 6837P/0F）** |

### 遗留受限项（当前有效，沿袭 + 本轮登记）
- 沿袭（bug14–21 轮登记，全部不变）：cargo-mutants 缺位；死 tracker 约 4 分钟静默窗；
  TUI 恐慌自愈边界（bug6）；BT partial 进度不持久化；QA-SQ-13 竞态沿袭待办；QA-RS-06
  沿袭；supervisor block_worker fatal 竞态观察项；`App.windows` 失去读者（cleaner 候选
  裁决）；leech 判据无 e2e 自动化（阈值常量固定）；bad_piece 归属口径 = 交付完成块
  peer 计 1 次（D67 允许）；Inversion 访谈受限项（chat 选择题面板不可用）。
- **本轮销项**：操作者 bug22 报告（BT 限速两行聚焦光标压最后字符）**本轮修复销项**
  （bug21 修订轮交接无遗留待修项，上轮登记项均已闭环）。
- **本轮新登记（specifier 轮/后续）**：①「对话框单行字段聚焦光标位」通用判据
  （聚焦即光标迁最后字符后方，终端光标列 = 行首 + w(label).max(7) + 2 + 值宽；点击
  value_x 同式）——操作者本轮 chat 明言「ezr 全输入框通用输入模式」，spec 三层
  （features 01-add-task/03-modify-task/07-add-bt/08-modify-bt 场景增补 + QA 用例）
  待 specifier 轮归档（沿 bug21 先例：coder 先修、specifier 归档）；②ezr-demo 侧
  已同口径落规范（§6.1 聚焦光标位 bullet）+ 守卫测试（cursor_tests）+ README 记录 34。
- 工具链：rustup nightly 1.101.0 + rustfmt + clippy + bb v1.13.229 + APS 均就位
  （PATH 需含 ~/.cargo/bin 与 .tools/bin；环境不持久时按 notes/environment.md 重建；
  本轮沙箱 PATH 曾被重置，export 补数即复）。

### 实现定义值登记（累计口径，本轮修订处加粗）
| 值 | 取值 | 位置 |
|---|---|---|
| leech 阈值 | up ≥ 1 MiB 且 down < 64 KiB 且存活 ≥ 120s | bt/engine.rs LEECH_* 常量 |
| bad_piece 阈值 | 同连接累计 3 次 | BAD_PIECE_BAN_THRESHOLD |
| protocol 阈值 | 未知 id/帧错首次即封；非法区间 request 重复 3 次 | INVALID_REQUEST_BAN_THRESHOLD |
| 限速输入上限 | 32 列（FR-01-06 形态对齐） | routing.rs CAP_LIMIT |
| 限速非法 toast | `⚠ 限速值非法（整数 B/s 或单位字符串，如 512 KiB/s）`（**HTTP/BT、下载/上传四处全部统一——本轮收口 bt_dialog.rs 上传臂**） | dialogs.rs / bt_dialog.rs |
| 限速字段占位 | 无占位（空态纯空白） | ui/dialog.rs / ui/bt_dialog.rs |
| 限速单位口径（对话框） | 二进制：1 KiB = 1024 B/s、1 MiB = 1024²、1 GiB = 1024³（config 文件键解析口径不变十进制） | routing.rs speed_unit_mult / dialogs.rs parse_speed_limit |
| 限速输入规范化 | 输入/粘贴即规范化显示（KiB/s、MiB/s、GiB/s、B/s；前导零剥离；canonical 完成态吸收单位后缀续入；退格不回写） | routing.rs canonical_speed_form / insert_limit |
| 并发满容量语义 | 光标在字段末尾且已满 2 位时键入/粘贴数字 = 清空重输；中位不动不插 | routing.rs insert_conns_digit |
| 多行合并行光标 | 接缝 = 合并前上一行长度处 | multiline.rs backspace |
| **对话框单行字段聚焦光标位** | **聚焦即光标迁最后字符后方（cursor = len_chars，退格删最后字符）；终端光标列 = 行首 + w(label).max(7) + 2 + 值宽，点击 value_x 同式（本轮修复 bt_text_row 硬编码 +9 对 8 列宽标签行左偏一列的缺陷，与 HTTP 对话框/ezr-demo 公式归一）** | ui/bt_dialog.rs bt_text_row / ui/dialog.rs / ezr-demo ui/dialogs.rs |
| 监听占用警告 | `BT 监听端口 {port} 被占用，已回退随机端口` | engine/mod.rs bind_bt_listener |
| 黑名单损坏警告 | `黑名单文件无法解析（{path}），已按空名单启动` | model/blacklist.rs |
| banned_at 格式 | unix 秒（u64） | model/blacklist.rs BlacklistEntry |
| trackers 空态文案 | `（无 trackers）` | ui/popups.rs |
| HTTP 对话框几何/焦点 | 添加 74×13（字段 7 行焦点 0–6、按钮 7/8/9）；修改 74×11（字段 5 行焦点 0–4、按钮 5/6）；「上传限速」字段退役（HTTP 无上传语义；BT 框保留） | ui/dialog.rs + dialog_keys + mouse.rs |
| **封禁 toast 与黑名单写入顺序** | **blacklist_add_with 先于 Evt::Toast 发送（实现事实；测试观察环须双条件等齐）** | bt/engine.rs ban 路径 |

## 二、当前产出情况

### 本会话产物清单
- **修复（bug22）**：`ui/bt_dialog.rs::bt_text_row` 终端光标列与点击 value_x 由
  硬编码 `rect.x + 9 + …`（= 标签 7 列 + "> " 2 列）改为 `rect.x + label_cols + 2 + …`，
  `label_cols = w(label).max(7)` 动态计算——「下载限速/上传限速」标签 4 个 CJK =
  8 显示列（pad_right(7) 超宽不截断），旧实现限速两行光标/点击基准左偏 1 列 →
  聚焦光标压在最后字符上、退格看似删不到最后字符；种子/保存到/并发（标签 ≤7 列）
  恰好不触发。与 HTTP 对话框（ui/dialog.rs `w(row.label).max(7)`）及 ezr-demo 同位点
  公式归一；逻辑光标（focus_field → cursor_end）本就正确，缺陷纯在渲染/点击映射层。
- **新增测试（防回归）**：`ui/bt_dialog.rs` tests 新增
  `bt_limit_focus_cursor_lands_after_last_char`——AddBt 并发(3)/下载限速(4)/上传限速(5)
  + ModifyBt 并发(1)/下载限速(2)/上传限速(3) 六行聚焦断言（自校准矩形：
  `dlg_cursor_screen == rect.x + label_cols + 2 + w(value)` + 逻辑 cursor = len_chars）；
  点击映射判据（value_x = 行首+10、首字符格/末字符格/越尾/右侧空白四点）；
  末尾光标退格删最后字符行为断言（旧硬编码实现下必红）。
- **ezr-demo 对账（操作者指令：若不同则同步）**：逐位点核对 demo `bt_text_row` 与
  HTTP 文本行——**素来动态计算 label_cols，无同款缺陷，产品源码零改动**；同步落
  三件：①守卫测试 `bt_limit_fields_focus_cursor_after_last_char`（src/ui/mod.rs
  cursor_tests，AddBt 三行同口径断言，31P/0F）；②规范文档 §6.1 新增「聚焦光标位
  （全应用单行字段通用输入模式）」bullet（规则 + 对账结论；§6.9 经「与 §6.1
  同口径」覆盖）；③README 变更记录 34（对账结论 + 守卫 + 文档同步；无需重建
  bin/ 预编译二进制——产品源码未动）。demo 存量 fmt 漂移 5 文件
  （scroll/multiline/popups，rustfmt 版本差遗留）未触碰，本轮改文件 fmt 干净。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| bin 全量单测 | **891P / 0F**（基线 890 + 净增 1） | `cargo test --bin ezr`（一次全绿） |
| 集成 12 目标 | **5946P / 0F** | `cargo test --test <目标>`（hardening 139 / engine_backfill 794 / g_appcore 695 / g_engine 690 / g_main 2 / g_proxy 16 / g_slots 197 / g_supervisor 664 / g_throttle_err 430 / g_v147 678 / ui_backfill 824 / v115 817；v147 首轮 1 例时序 flake，复跑即绿 = 既有登记口径） |
| ezr 全量合计 | **6837P / 0F** | 上述之和 |
| ezr-demo 全量 | **31P / 0F**（含新守卫测试） | `cargo test`（ezr-demo/） |
| clippy | 0 警告（--bin 与 --tests） | `cargo clippy --bin ezr --tests` |
| fmt | ezr 干净；demo 本轮改文件干净 | `cargo fmt --check` |

### 对账口径
- 基线 = bug21 修订轮登记 bin 890P/0F + 集成 5943P/0F = 6833P/0F；本轮终态 6837P/0F
  （bin +1 新渲染测试；集成 +3 = 新测试属 `ui::bt_dialog` 模块，收编面 =
  engine_backfill/ui_backfill/v115 三套件各 +1——**收编面按测试所在模块分流**，
  与上轮 `app::bt_dialog` 收编 7 套件的清单不同，+1+3 = +4 同源吻合）。
- v147 首轮 1F = 时序 flake（复跑 678P/0F；总数 678 与上轮持平 = v147 未收编本轮
  ui 模块测试，失败与本轮改动无关）。
- 集成存量断言扫描：`dlg_cursor_screen`/`value_x` 在 tests/ 仅两处命中
  （hardening_v115.rs:147 = HTTP 框 URL 行动态公式断言、hardening_ui_backfill.rs:868
  = 详情面板 URL 行布局常量），均与本轮 bt_dialog 位点无关，实测全绿佐证。
- 操作者验收判据：BT 添加/修改对话框 Tab/点击进入「下载限速/上传限速」→ 终端光标
  停最后字符后方，Backspace 删最后字符；与「并发」框行为一致。

### 待办与待批
- **待批（操作者）**：无新增待批项（本轮为缺陷修复 + demo 对账同步）。
- **待办（specifier 轮）**：「对话框单行字段聚焦光标位」通用判据落 spec 三层
  （features 01-add-task/03-modify-task/07-add-bt/08-modify-bt 场景增补 + QA 用例；
  本轮新登记①）。
- **待办（QA 轮）**：QA runner 执行层对修订规格/QA 套件改动的同步评估（沿袭开放）。
- **待办（cleaner 轮，沿袭）**：`App.windows` 失去读者裁决；handle_peer_msg 参数收拢；
  bt/engine.rs 拆分评估；`up_prefill` 在 HTTP 路径已无读者（字段面 BT 专用化归并评估）；
  `canonical_speed_form` 与 `parse_speed_limit` 的解析前缀重复归并评估。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/specifier`（本轮判据归档）或
  `bin/swarm begin six-pack/cleaner`（沿上轮建议）：以本文件为上游交接。
- 后续链：specifier → coder / cleaner → architect → hardender → QA（宪法第一章：
  不自动流转）。
- 操作者授权的收尾口径：`bin/swarm complete` 后立即将产物 tar.gz 复制到
  `/home/z/my-project/`（沿 bug18/bug20/bug21 轮授权口径执行）。
- 工具链交接：rustup nightly 1.101.0（PATH = ~/.cargo/bin）+ cargo registry rsproxy；
  bb + APS 已装（.tools/bin、.tools/Acceptance-Pipeline-Specification）——
  环境不跨会话持久时按 notes/environment.md 重建。
