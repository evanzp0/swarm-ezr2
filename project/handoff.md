# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（本轮已完成）→ 建议下一节点 six-pack/coder
> 会话：ezr-specifier-bug18（2026-11-02，「五项新功能规格轮」：全局上传/下载限速、
> 任务级上传/下载限速、自动反吸血 blacklist.yaml、't' trackers 弹窗、[bt] listen_port）
> 续会话：ezr-specifier-bug19 追轮（2026-11-02，操作者三条 UI 反馈修复 + trackers 弹窗
> 改版：①修改 BT 面板出现 wss://（明确不支持）；②新增 BT 任务 T 弹窗空、面板却有值；
> ③弹窗列参考 qBittorrent、窗口更大、URL 支持 word wrap——修复 + 改版 + 全量验证 + 归档）
> 续会话2：ezr-specifier-bug20 追轮（2026-11-02，操作者六条 UI 反馈：①明细行间细线分隔；
> ②URL 换行续行同色；③http URL 禁用终端 hover 下划线；④右侧 padding 缩至与左侧相同；
> ⑤底部提示全部删除；⑥BT 添加/修改窗口 trackers 面板同口径禁 hover 下划线——
> 修复 + 规格 v1.11 + 全量验证 + 归档）
> 上游交接：project/handoff.md（bug17 coder 轮交接：bin 864P/0F 基线 + FR-02-27 全量交付，
> 任务依据 = 已批规格 + 操作者 bug18 五项功能指令，含「complete 后归档复制 /home/z/my-project/」授权）

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式 v1.38）+ `project/mission/phase-01…04.md`。
- phase-01（HTTP/HTTPS 内核与 TUI）已收官；phase-02（BT 内核/做种/管理）已实现并经
  互操作夹具多轮收口。历轮 bug11–17 全部落地（config 三大表/客户端识别/归一/padding/
  BT 热调/连接生命周期礼仪）。本轮为操作者 bug18 指令级需求修订规格轮（五项新功能）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01/02 及历轮 bug11–17） | features/ 20 份 + qa/ 对应套件 | 已收官/已交付 | 见历轮登记 |
| coder（phase-02 及历次收口轮，含 bug15/16/17 落地） | `project/ezr/` 全量 | 已交付 | bin 864P/0F（859 基线 +5）+ 集成 12 目标 5742P/0F + clippy 零警告 + fmt 干净（bug17 轮登记） |
| **specifier（本轮 bug18 规格轮）** | mission/phase-02 v1.9 + phase-01 v1.42 注记 + mission.md v1.38 + features 修订/新建 ×11 + qa 修订/新建 ×7 + notes ×3 + ezr-demo 同步（代码 ×12 + 文档 ×2） | **已交付** | **APS parser+dry-check 11/11 EXIT=0（findings 为 report-only 咨询、与历轮同形态）；ezr-demo cargo check 通过** |
| **specifier（bug19 追轮）** | 11-trackers-window v1.10 重写 + 07/08 feature +场景11/09 + 01-tui-display v1.43（帮助 12 组 75 行对齐）+ phase-02 v1.9.1（FR-02-31/D67⑤）+ QA ×4 + notes ×3 + ezr-demo 修复与改版（代码 ×7 + 脚本 ×2 + 文档 ×2） | **已交付** | **cargo check 通过；PTY 端到端：verify_trackers_popup.py 21/21 全过、verify_phase2_ui.py B/C 段全绿（12 项失败 = HEAD worktree 对照实证为沙箱环境预存、非本轮回归）；APS dry-check ×4 EXIT=0** |
| **specifier（bug20 追轮）** | phase-02 v1.9.2（FR-02-31 v1.9.2 注记）+ 11-trackers-window v1.11 + 07/08 feature 反链接条目 + QA-TW 同步 + notes ×2 + ezr-demo 代码 ×4（ui/popups.rs 列几何与细线与反链接、ui/multiline.rs no_link 参数、ui/dialogs.rs + ui/cmd.rs 接线）+ 脚本 ×2（verify_trackers_popup 26 项新断言、verify_phase2_ui C-TR VS15 断言）+ 文档 ×2（UI 规范 §6.9/§6.12/头注/尺寸速查、README 条目 32） | **已交付** | **cargo check + build 通过；verify_trackers_popup.py 26/26 全过（新增 VS15 反链接/细线 ×3/无提示行/44 列宽折行断言）；verify_phase2_ui.py B 段 21 项 + C 段 11 项全绿（A 段 12 项失败与 bug19 轮归因完全同形 = 沙箱 PTY \x7f 预存，非本轮回归）；画面转储目检：左右 padding 对称、细线在位、VS15 可见** |

### 遗留受限项（当前有效，沿袭 + 本轮登记）
- 沿袭（bug14–17 轮登记，全部不变）：cargo-mutants 缺位；死 tracker 约 4 分钟静默窗；
  TUI 恐慌自愈边界（bug6）；BT partial 进度不持久化；QA-SQ-13 竞态沿袭待办；QA-RS-06
  沿袭；supervisor block_worker fatal 竞态观察项；`App.windows` 失去读者（cleaner 候选
  裁决）；ezr-demo verify_ezr.py 场景 N 四项预存假 FAIL（待 demo 脚本维护轮）；
  ezr-demo 11 条预存 nightly lint（未触碰文件）；QA 套件可执行化（QA-MB-05/07/QA-BE-15
  的 runner 化与实跑）属 six-pack/QA 职责。
- **沿袭解除一项**：「BT 下载限速未接」（bug14 起登记）——本轮 FR-02-29 已规格化
  BT 引擎任务级限速真实接入（BtSpec.max_speed dead_code 退役），coder 落地后解除。
- **Inversion 访谈受限项（沿袭）**：chat 选择题面板不可用，操作者 bug18 指令文本即
  阶段 1 答案（沿 bug13–17 轮先例，登记 D67⑧）。
- 本轮新受限项：无阻塞；五项功能全部完成规格化，无待澄清需求。

### 设计决定登记（沿袭 + 本轮）
- 沿袭 D1–D66 全部裁决（D66 = bug17 轮已批项，见 mission/phase-02.md）。
- **本轮 D67（bug18 轮设计定义值，specifier 裁决，待操作者批）**：①全局限速键名
  global_max_download_speed / global_max_upload_speed（原 max_speed 键退役按未知键忽略、
  --max-speed CLI 保留）；②min(全局, 任务级) 合成、0 = 不限；对话框限速字段格式与回退口径；
  ③反吸血三判据与阈值（leech：up≥1MiB 且 down<64KiB 且存活≥120s 仅下载侧、做种豁免 /
  bad_piece：3 次 / protocol：破坏性消息首封、非法区间 request 3 次）；④blacklist.yaml
  = ~/.ezr/blacklist.yaml（条目 ip/reason/banned_at；损坏=空名单+警告非致命；无自动过期，
  手工删除条目重启=解封）；⑤trackers 窗口 90×22（bug19 追轮改版，原 74×16）只读、qBt 六列（#/URL/状态/Peers/Seeds/消息）、URL 36 列宽自动换行、三态状态列；⑥listen_port 占用回退
  随机+警告、进程级共享单端口；⑦反吸血 toast 前缀 ⛔。

## 二、当前产出情况

### 本会话产物清单
- **需求层**：`mission/phase-02.md` v1.9（新增 FR-02-28 全局上传/下载限速 / FR-02-29
  任务级上传/下载限速 / FR-02-30 BT 自动反吸血与黑名单 / FR-02-31 't' trackers 弹窗 /
  FR-02-32 [bt] listen_port + D67 + 验收期望第 7 条）；`mission/phase-01.md` v1.42 注记
  （FR-01-60 改名 / FR-01-01、FR-01-87 对话框 +2 字段 / FR-01-80、FR-01-107 T 键改判）；
  `mission.md` v1.38（分期索引 phase-02 主题句扩展）。
- **规格层（features ×11）**：`01-throttle-proxy.feature` v1.9 全量重写（14 场景：
  全局两键改名+BT 并入、上传限速、任务级四联动、min 合成、持久化、旧键退役）；
  `10-bt-anti-leech.feature` 新建（8 场景：三判据/做种豁免/blacklist.yaml 加载与解封/
  三过滤面/手动共存）；`11-trackers-window.feature` 新建（6 场景：门控/74×16 形态/
  三态状态列/滚动鼠标/关闭模态/页签 t 退役+wss 不支持）；`09-bt-engine.feature` v1.38
  （场景 16 listen_port 新增 + 头注）；`01-add-task.feature` v1.39（74×14 七字段 + 场景 20）；
  `03-modify-task.feature` v1.36（74×12 六字段 + 场景 10）；`07-add-bt.feature` v1.38
  （78×28 + 场景 10）；`08-modify-bt.feature` v1.5（78×26 + 场景 08 + 场景 07 组合
  纳入限速）；`01-tui-display.feature` v1.42（T 键改判 + 帮助条目替换 50 行不变）；
  `01-config-template.feature` v1.14（36→40 键，钦定原文内嵌更新，实测 191 行/48 空行）；
  `01-persistence-config.feature` v1.20（40 键冒烟/活跃模板口径）。
- **QA 层（×7）**：`01-throttle-proxy-qa.md` v1.9 全量重写（QA-TP-01..07 修订 +
  08..14 新增）；`10-bt-anti-leech-qa.md` 新建（QA-AL-01..08）；`11-trackers-window-qa.md`
  新建（QA-TW-01..06）；`09-bt-engine-qa.md`（+QA-BE-16）；`01-add-task-qa.md`（+QA-01-20）；
  `03-modify-task-qa.md`（+QA-MT-10）；`07-add-bt-qa.md`（+QA-AB-10）；`08-modify-bt-qa.md`
  （+QA-MB-08）。
- **经验沉淀**：`notes/architecture.md`（限速现状证据链/反吸血设计口径/trackers 数据源/
  listen_port 落点）+ `notes/environment.md`（[/] 显示改写 vs 磁盘损坏判别法/gh-proxy
  复证）+ INDEX 同步；`packs/_common/notes/aps.md` 已有 dry-checker 条目覆盖本轮发现（DRY 不重复）。
- **ezr-demo 同步**：代码 ×12 文件（`app/dialog.rs` Dialog+down/up_limit+prefill 字段
  与全部分发表；`app/dialogs.rs` 四框预填/确认链/parse_speed_limit/demo_bt_trackers；
  `app/keys.rs` t 臂→open_trackers_popup + 浮层内关闭臂；`app/popups.rs` 门控三臂；
  `app/mod.rs` trackers_open；`app/model.rs` Task.task_max_down/up；`app/mouse.rs`
  热区联动；`app/demo.rs`、`ui/dialogs.rs` 四框几何 74×14/74×12/78×28/78×26 + 渲染行；
  `ui/popups.rs` draw_trackers 74×16；`ui/mod.rs` 浮层集合/渲染调度）——焦点表
  Add/Modify/AddBt/ModifyBt = 11/8/11/8；文档 ×2（README 变更记录 26/27/28 条 +
  设计规范 §6.12 新增、§4.2.6/§4.3/§6.1/§6.2/§6.9/§6.10/帮助分组修订）。
  全局键/blacklist/listen_port 为 ezr 产品端行为，demo 不模拟（README 条目 28 声明）。

### bug19 追轮产物（续会话追加）
- **修复层（ezr-demo 代码 ×7）**：`app/dialogs.rs`（demo_bt_trackers 去 wss 改 4 条
  http/https/udp + tracker_reject scheme 白名单 + reject_bad_trackers 确认层拒绝
  （add/modify 双路，NLL 安全模式：纯判定在 if 条件、副作用在早退分支）+ 新增 BT
  trackers 留空确认时默认 4 条入库）；`ui/popups.rs`（draw_trackers 全量重写：
  90×22、qBt 六列、wrap_by_width URL 折行、push_col 绝对列偏移排版、SbKey::Trackers
  滚动条、底部提示行；帮助 trackers 组 2 条目同步）；`app/mod.rs`（trackers_scroll/
  trackers_max_scroll/trackers_sb 三字段）；`app/scroll.rs`（SbKey::Trackers 五处）；
  `app/keys.rs`（trackers 弹窗 ↑↓/PgUp·PgDn/Home·End 滚动键）；`app/mouse.rs`
  （滚轮 ±2 + 滚动条按压/拖拽 + sb_arrow_follow）。
- **验证脚本 ×2**：`scripts/verify_trackers_popup.py` 新建（T1 种子任务弹窗几何/六列/
  数据 / T2 新任务默认 tracker 入库回归 + URL 折行 + 待 announce / T3 wss 拒绝——
  21 项全过）；`scripts/verify_phase2_ui.py`（几何 +2 平移、点击聚焦、合法 URL 直确认
  三处修复后 B/C 段全绿）。
- **规格层**：`features/11-trackers-window.feature` v1.10 重写（90×22/六列/换行/
  折行滚动/对话框层 wss 拒绝/演示口径默认 trackers 注记）；`features/07-add-bt.feature`
  +场景 11（scheme 校验与默认 trackers）；`features/08-modify-bt.feature` +场景 09
  （拒绝不落库）；`features/01-tui-display.feature` v1.43（帮助全量对齐源码 12 组 75
  行——修复 v1.35 起三组实装增量未回写的漂移）；`mission/phase-02.md` v1.9.1
  （头注 + FR-02-31 窗口形态/对话框层拒绝 + D67⑤）；QA：QA-TW v1.10 重写 +
  QA-AB-11/QA-MB-09 追加 + QA-TD-19 对齐。
- **文档层（ezr-demo ×2）**：README 变更记录条目 29 + 脚本清单；
  设计规范头注/尺寸清单/§6.12 整节改写。
- **经验沉淀**：notes/architecture.md（同源同库模式/wss 双过滤面/弹窗改版落点）+
  environment.md（PTY \x7f ≈2/批投递上限定性 + release 路径假全 FAIL 陷阱）+
  testing.md（worktree HEAD 对照归因法/几何断言平移/点击聚焦）+ INDEX 同步。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| APS gherkin-parser ×11 | 全部 EXIT=0 | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-parser <feature> <out>)` |
| APS gherkin-ir-dry-checker ×11 | 全部 EXIT=0（report-only 咨询 findings：09=55/历轮 52、08=35/历轮 28 同形态；unique steps：09 95/95、03 50/50、07 67/67、08 56/56、10 39/39 全唯一） | `bb gherkin-ir-dry-checker <ir> <dry>`（输出在 `.work/tmp/*.dry.json`） |
| ezr-demo cargo check | 通过（0.86s，修复子代理遗留 10 个编译错误：filter &char 闭包×2、E0502 prefill 借用提前提取×2、popups u16 算术） | `cd ezr-demo && cargo check` |
| 磁盘字节终审 | [http]/[bt]/[[common.proxy]] 字面量全部完好（bug16 轮 [/] 改写陷阱全程规避 + L195 历史磁盘损坏修复） | `python3 bytes 计数（见 notes/environment.md 判别法）` |
| APS parser+dry-check ×4（bug19 追轮修订件） | 全部 EXIT=0（report-only findings：01-tui-display=227、07=86、08=45、11=3——重复字面量/近义步骤咨询，与历轮同形态） | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-parser <feature> <ir> && bb gherkin-ir-dry-checker <ir> <dry>)`（输出 `.work/tmp/dry/`） |
| ezr-demo PTY 端到端（bug19 追轮） | verify_trackers_popup.py **21/21 全过**（T1 几何 90×22/六列/数据、T2 新任务 4 条默认 tracker + 47 字符 URL 折行 + 待 announce、T3 wss 拒绝 toast + 不落库）；verify_phase2_ui.py B/C 段全绿（B-TR 交互 11 项 + C-TR 预填含无 wss 断言） | `python3 scripts/verify_trackers_popup.py ./target/debug/ezr-tui-demo` |
| 回归归因对照（bug19 追轮） | A 段 12 项失败在 HEAD worktree 对照构建下**完全同形**——定性沙箱环境预存（PTY \x7f 连发 ≈2/批投递上限），非本轮回归 | `git worktree add /tmp/ezr-head HEAD && (cd /tmp/ezr-head/ezr-demo && cargo build) && python3 scripts/verify_phase2_ui.py /tmp/ezr-head/ezr-demo/target/debug/ezr-tui-demo` |
| ezr-demo cargo check（bug19 追轮后复检） | 通过 | `cd ezr-demo && cargo check` |

### 对账口径
- 新增 FR ×5（FR-02-28/29/30/31/32）、D ×1（D67）、feature 场景净增 13（14 场景文件
  ×1 全量重写、新建 ×2 共 14 场景、增量场景 20/10/10/08/16 ×5）；QA 用例净增 12
  （QA-TP-08..14 ×7 + QA-AL ×8 + QA-TW ×6 + QA-BE-16/QA-01-20/QA-MT-10/QA-AB-10/
  QA-MB-08 ×5——其中 TP-08..14 为 7 条新增、TP-01..07 修订不计数）。
- 五项功能与操作者指令逐项对应：①全局限速 = FR-02-28（含 config 键、跨协议合计、
  上传方向新增）；②任务级限速 = FR-02-29（config 两键钦定名、添加框预填可改、修改框
  确认立即生效——FR-02-26 实例）；③自动反吸血 = FR-02-30（qBt 无自动封禁的核证结论
  + BitComet PSR 参考 + 自设计三判据 + blacklist.yaml 独立持久化）；④t trackers 窗口 =
  FR-02-31（页签 t 退役、仅 http/udp、wss 不支持）；⑤listen_port = FR-02-32（未配置随机）。
- ezr-demo：UI/交互面全量同步（对齐 FR-02-29/31 的 demo 可模拟面）；行为面（引擎限速/
  黑名单/端口）为产品端专属，demo 按 README 条目 28 声明不模拟——与 bug16/17 轮
  「demo 模拟引擎本就按确认即重建口径」的同步边界判定同构。

### 待办与待批
- **待批（操作者）**：D67 全部设计定义值（全局键名与旧键退役、min 合成、反吸血三判据
  阈值与做种豁免、blacklist.yaml 形态与非致命口径、trackers 窗口几何与列口径与拒绝文案
  ——⑤已按 bug19 追轮更新为 90×22/qBt 六列/URL 换行、listen_port 回退与共享单端口、
  ⛔ 前缀）；规格轮产物措辞（feature/QA 修订注记）。
- **待办（coder 轮）**：五项功能实现——BT 引擎任务级限速真实接入（Reconfigure 载体
  扩展）、全局限速协议无关层上传方向扩展、反吸血三判据引擎接线 + blacklist.yaml
  serde_yaml 持久化、trackers 弹窗真实数据源（引擎 tracker 池状态 emit）、listen_port
  进程级共享监听、config 解析 5 新键 + 模板 40 键、任务注册表新字段；ezr 端四对话框
  +2 字段（焦点表/键路由/鼠标三张表成对接入——notes 纪律）；帮助窗口文案同步。
- **待办（QA 轮沿袭）**：QA 套件 runner 化与实跑（含本轮 QA-AL/QA-TW 新套件）。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/coder`：以本文件为上游交接。实现顺序建议：
  ①config 5 新键 + 模板 40 键 + 任务注册表字段（FR-02-29/32 基础面）→ ②全局限速
  协议无关层（EngineShared 扩上传方向 + BT 引擎接入，FR-02-28）→ ③任务级限速
  四对话框 + 引擎热调（FR-02-29，Reconfigure 载体扩展）→ ④listen_port 共享监听
  （FR-02-32）→ ⑤反吸血三判据 + blacklist.yaml（FR-02-30，serde_yaml 新依赖）→
  ⑥t 键 trackers 弹窗 + 页签退役（FR-02-31，UI 面；窗口形态按 v1.10 = 90×22/qBt
  六列/URL 36 列宽换行/折行显示行滚动，ezr-demo `ui/popups.rs draw_trackers` 为
  同构参照实现）。全量验证基线 = bin 864P/0F
  （本轮新增场景测试后计数沿登记惯例由 coder 申报）。
- 给操作者的手测口径（实现后）：①config 设 global_max_download_speed = "1 MB/s" 后
  HTTP+BT 混合下载合计应封顶；②添加/修改任务窗口改限速确认后速度立即收敛；③对
  fixture 式吸血 peer（只收不给）120s 后观察 ⛔ toast + blacklist.yaml 落盘 + 三面拒绝；
  ④BT 任务按 t 弹出 trackers 窗口、]/[ 仍可切页签；⑤[bt] listen_port = 51413 后
  fixture tracker 记录 announce port = 51413。
- 后续链：coder → cleaner → architect → hardender → QA（宪法第一章：不自动流转）。
- 操作者授权的收尾口径：`bin/swarm complete` 后**立即**将产物 tar.gz 复制到
  `/home/z/my-project/`。bug18 主轮归档 `~/swarm-20261102-044505-six-pack-specifier.tar.gz`
  已按此执行；bug19 追轮归档按同口径复制。

## bug20 追轮交付（specifier，2026-11-02）

### 范围（操作者六条 UI 反馈，全部仅展示层，数据面/确认链不变）
1. trackers 弹窗相邻明细行之间 `─` 通栏细线分隔（首条前/末条后不画、同条折行续行间不画，细线计入滚动总行数）；
2. URL 折行续行与首行同色同 FG（不再降暗）；
3. http(s)/www URL 渲染层反终端自动链接——VS15（U+FE0E 零宽）+ 相邻 span ±1 微差色交替（复用既有 `break_links`/`link_break_spans`，与详情页 URL 字段同构）；udp 行不注入；
4. 内容区左右 padding 对称各 2 列——URL 列 36→44 承接增量，状态/Peers/Seeds/消息 列右移 49/62/69/76，消息列预算 8 列与右 padding 齐平；
5. 底部提示行全部删除（vis = 内框高 − 3，内容区延伸至底边框）；
6. 添加/修改 BT 对话框 trackers 面板同口径反终端自动链接——`draw_multi_line_input` 新增 `no_link` 参数（渲染层注入发生在 disp_window 之后，不改缓冲与光标/点击映射；命令窗口 false）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| ezr-demo cargo check / build | 通过 | `cd ezr-demo && cargo check && cargo build` |
| verify_trackers_popup.py | **26/26 全过**（T1 增 VS15/细线/无提示断言；T2 增 4 条默认 tracker 全可见/44 列宽折行「annou|nce」/细线 ×3/无提示/VS15 断言） | `python3 scripts/verify_trackers_popup.py ./target/debug/ezr-tui-demo` |
| verify_phase2_ui.py | B 段 21 项 + C 段 11 项全绿（C-TR 新增 http 行 VS15 / udp 行无断言）；A 段 12 项失败与 bug19 轮 HEAD worktree 对照归因完全同形（沙箱 PTY \x7f ≈2/批预存，非本轮回归——本轮 A 段路径 wrap/no_link=false 输出与改前逐字节一致） | `python3 scripts/verify_phase2_ui.py ./target/debug/ezr-tui-demo` |
| 画面转储目检 | 弹窗 90×22：左右 padding 对称 2 列、细线在位、消息列与右 padding 齐平、无底部提示、http 行含 VS15 | `.work/tmp/dump_trackers.py`（会话临时件） |
| 磁盘字节终审 | 本轮 11 个编辑文件 utf-8 完好；`[common]`/`[bt]` 字面量、`─`、列常量字节级在位 | `python3 bytes 校验（见 notes/environment.md 判别法）` |
| APS parser+dry-check ×3（bug20 追轮修订件） | 全部 EXIT=0（report-only findings：11-trackers-window=4（v1.11 增量 And 条目 +1 咨询）、07=86、08=45——与历轮同形态） | `(cd .tools/Acceptance-Pipeline-Specification && ../../.tools/bin/bb gherkin-parser <feature> <ir> && ../../.tools/bin/bb gherkin-ir-dry-checker <ir> <dry>)`（输出 `.work/tmp/dry/`） |

### 规格同步面
phase-02 v1.9.2 头注 + FR-02-31 行内改版（v1.9.2 标注）；11-trackers-window v1.11（v1.11 注记 + 场景 02 四条 And + 场景 04 细线滚动单位）；07-add-bt / 08-modify-bt trackers 面板反链接 And 条目；qa/11 v1.11（头注 + QA-TW-02 判据改版 + QA-TW-04 细线滚动单位 + 通过准则基线 26 项）；ezr-demo UI 规范（头注 bug20 行 + §6.12 五处 + §6.9 面板条目 + §4 尺寸速查）+ README（条目 32 + 脚本 26 项）；notes/architecture.md bug20 节 + INDEX 同步。

### 待办与移交建议（不变项沿用 bug19 轮）
- D67 待批项不变；coder 轮实现顺序建议不变（六项 UI 反馈全部为展示层，ezr 端实现 trackers 弹窗时按 11-trackers-window v1.11 口径对齐——细线/对称 padding/无提示行/44 列/续行同色/VS15 反链接六项一并纳入）。
- ezr-demo 回归基线更新：verify_trackers_popup.py **26 项**、verify_phase2_ui.py B/C 段全绿 + A 段 12 项沙箱预存失败（归因沿 bug19 轮）。
- 操作者授权收尾口径不变：`bin/swarm complete` 后立即将产物 tar.gz 复制到 `/home/z/my-project/`。
