# QA 套件 · 11-trackers-window trackers 弹窗（期号 02，v1.9 新建 / v1.10 改版同步 / v1.11 六项反馈同步 / v1.12 几何修订同步）

> 对应规格：`project/features/11-trackers-window.feature`（6 场景）。
> **v1.12 同步（bug24 归档轮，D67⑤ v1.11 修订）**：窗口 90×22 → 94×22（内框 92×20、
> 内容区宽 88；列起点不变），末列「消息」预算 8 → 12 列——QA-TW-02/03 判据修订：
> 「下轮 <s>s / 重试 <s>s」3–6 位秒数完整显示无 '…' 截断（bug24 验收判据）。
> 需求依据：`project/mission/phase-02.md` FR-02-31、D67（bug18 轮设计定义值，待批）；
> v1.10 追轮按操作者三条 UI 反馈改版：窗口 90×22、qBt 式列（#/URL/状态/Peers/Seeds/消息）、
> URL 36 列宽自动换行；v1.11 追轮按操作者六条 UI 反馈改版：明细行间 ─ 细线分隔、
> URL 折行续行与首行同色、http(s) URL 反终端自动链接（VS15 + ±1 微差色交替）、
> 内容区左右 padding 对称 2 列、底部提示行全部删除、URL 列宽 44 列。
> 行为口径：任务列表 T/t 打开当前 BT 任务的 trackers 窗口（只读、真实 tracker 池状态）；
> 原页签 t 切换废除（页签仅 ]/[）；仅 http/udp tracker（wss 不支持：引擎拒入池 + 对话框层确认拒绝）。
> 端到端 UI 层验证：按键注入 + 画面断言 + fixture tracker 日志对账（状态列 = 引擎
> tracker 池真实状态，与 fixture 侧 announce/interval/退避记录交叉核对）。
> ezr-demo 已有 PTY 级自验证 `scripts/verify_trackers_popup.py`（T1 种子任务弹窗 /
> T2 新增任务默认 tracker 入库回归 / T3 wss 拒绝；v1.11 起含 VS15 反链接/细线分隔/
> 无提示行/44 列宽折行断言）与本套件口径对应。

## 环境前置

- 伪终端驱动：按键注入、画面捕获、toast/浮层断言；独立 HOME；终端 120×44。
- 本地 BT fixture 四件套（同 09-bt-engine-qa），tracker 支持可控 interval 与失败
  （可停服模拟退避）；预置 .torrent 文件目录 /tmp/qatorrents。
- 状态列对账口径：fixture tracker 记录每轮 announce 时刻与响应 interval/peers 数，
  与窗口 Peers/消息列逐项对账（下轮 <s>s = interval 倒计时；失败重试 <s>s = 退避计数）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-TW-01 | 11-trackers-window-01 | ①未选中按 t；②选中 HTTP 任务按 t；③选中 BT 任务（下载中/做种中/暂停各一轮）按 t | ①toast `未选中任务，无法查看 trackers`；②toast `该任务没有 trackers（仅 BT 任务支持）`；③三种任务状态均正常开窗（弹窗不打开臂无窗口渲染） |
| QA-TW-02 | 11-trackers-window-02 | BT 任务（tracker 池 3 条 = config + announce-list + 手输合并去重，含 ≥1 条超 44 列宽 URL）按 t 开窗 | **94×22**（v1.12：D67⑤ 修订，90×22 退役）圆角模态居中、标题 ` Trackers `、[✗]；布局 = 任务名行/表头（#/URL/状态/Peers/Seeds/消息）/tracker 显示行；**底部无提示行**（v1.11）、**内容区左右 padding 对称各 2 列**；相邻明细行间 **─ 通栏细线分隔**（首条前/末条后不画、同条折行续行间不画）；行 = 右对齐 # + URL（**44 列宽**列内自动换行、续行仅回填 URL 列、**续行与首行同色同 FG**）+ 状态 + Peers/Seeds（右对齐；— = 无）+ 消息；**末列「消息」预算 12 列（起点 76，内容区宽 88 − 76，v1.12）**；http(s)/www 行渲染层含 VS15 反终端自动链接注入（udp 行无）；3 条 tracker 折行后不超屏无滚动条 |
| QA-TW-03 | 11-trackers-window-03 | 运行中任务（HTTP tracker 正常 / UDP 正常 / 停服退避中 / 暂停任务）分别开窗读状态/Peers/Seeds/消息列；**含 ≥3 位秒数用例（下轮倒计时 ≥100s 与重试退避 ≥100s 各一轮）** | 正常 = `工作中`（Peers/Seeds 计数与 fixture peers 响应对账、消息 = `下轮 <s>s` = interval 倒计时）；退避 = `失败`（Peers/Seeds = —、消息 = `重试 <s>s`）；暂停任务 = `待 announce`（三列均 —）；**3–6 位秒数在 12 列预算内完整显示无 '…' 截断（v1.12，bug24 验收判据：120s/3600s 级全可见）**；数据与 fixture announce 日志交叉一致 |
| QA-TW-04 | 11-trackers-window-04 | 12 条 tracker 任务（含长 URL 折行）开窗；↑↓/PgUp·PgDn/Home/End/滚轮滚动；拖拽移动；点击外部 | 右缘滚动条出现（FR-01-109：↑│║↓ 恒绿滑块；按折行后显示行计数，**分隔细线同为滚动单位**）；视口滚动首尾钳制（续行与首行同为滚动单位，折行行不被拆断）；拖拽移动钳制终端内、关闭不保留；点击外部 = 无任何操作 |
| QA-TW-05 | 11-trackers-window-05 | 开窗后依次按 t/T/Esc/q/Q（每次重开）；开窗时按 Ctrl+C；开窗时按 A/D/M | 五通道均关闭且不退出（q/Q 先关窗）；Ctrl+C 退出程序；模态期间 A/D/M 等被窗口消费（不开对话框）；开窗时底层面板降级暗框 |
| QA-TW-06 | 11-trackers-window-06 | ①主界面按 t/T 观察；②任务 tracker 含 `wss://tracker.example/ws`（config trackers 键 / announce-list / 手输三来源分别注入）后开窗；③来源含 `ftp://tracker.example/ann` 按三来源分别注入 | ①不再切换过滤页签（页签仅 ]/[ 可切；帮助【页签】组无 T 条目、【任务列表】组有 T 条目、【trackers 弹窗（T）】组含列口径与滚动条目）；②ws(s) 行同权入池照常显示（v1.13 反转：原「拒绝入池」臂退役；Peers/Seeds/消息与非 ws 行同口径）；③白名单外 scheme 三来源分流：config trackers 键来源 = 启动失败并报错（FR-02-35）；announce-list 来源 = 该行忽略 + 警告（窗口与任务 tracker 列表均不出现）；对话框手输来源 = toast `⚠ 不支持的 tracker 协议（仅 http(s)/ws(s)/udp）：…`、焦点回 trackers、不落库 |

## 通过准则

- 全部用例结果记 P（通过）/ F（失败）/ S（受限跳过）；S 必须内嵌环境判据文案。
- 页签回归：QA-TW-06① 与 01-tui-display-07（] / [ 切换）联合回归——t 退役后页签
  功能无损。
- 状态列对账容差：倒计时 <s>s 与 fixture interval 存在 ±1s 采样差属正常。
- ezr-demo 侧回归基线：`scripts/verify_trackers_popup.py` 全部通过（T1/T2/T3 共 26 项，
  v1.11 起 T1 增 VS15 反链接/细线/无提示断言、T2 增 44 列宽折行/细线 ×3/无提示/
  VS15 断言）；`scripts/verify_phase2_ui.py` B/C 段全部通过（B-TR 交互 11 项 + C-TR
  预填含无 wss 与 http 行 VS15 断言）；A 段 12 项失败为沙箱 PTY 预存（bug19 轮
  worktree 对照归因同形，非回归）。
