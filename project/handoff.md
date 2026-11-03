# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（本轮已完成）→ 建议下一节点 six-pack/coder
> （config 键旧写法废弃实现对齐）或 six-pack/cleaner
> 会话：ezr-specifier-bug23-24（2026-11-03，specifier 归档轮：bug23 四项判据 + bug24 几何判据
> 落规格三层；Inversion 阶段 1 = chat 选择题面板恢复可用六题作答（五项全做 / 三项反转批准 /
> 边界规格注记 / 无下一 feature）+ 操作者 chat 修订指令「config 键旧写法 KB/MB/GB 十进制
> 不再支持，直接废弃」（取代面板兼容答案，废弃值按非法回退默认 0））
> 上游交接：project/handoff.md（bug24 轮 coder 交接）

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式 v1.40）+ `project/mission/phase-01…04.md`。
- phase-01（HTTP/HTTPS 内核与 TUI）收官；phase-02（BT 内核/做种/管理）已实现并经
  互操作夹具多轮收口。历轮 bug11–24 全部落地。
- **本轮（specifier，bug23/24 归档）**：bug23 四项判据（限速单位三层口径 / 修改框清空回填 /
  预填回显 fmt 规范化 / BT sidecar partial 持久化）+ bug24 几何判据（trackers 94×22）
  落需求/规格/QA 三层；config 键旧写法废弃为规格先行修订（实现对齐属 coder 待办）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| **specifier（本轮，bug23/24 归档）** | 需求层：mission.md v1.40 + phase-02.md v1.11（头注 + FR-02-22/28/29/31 注记 + D67 修订 + D68 新增）；规格层：features 八文件（01-config-template v1.15、01-throttle-proxy v1.11、01-add-task v1.41、03-modify-task v1.38、07-add-bt v1.40、08-modify-bt v1.7、09-bt-engine v1.39 场景 17 新增、11-trackers-window v1.12）；QA 层：qa 八套件同步（QA-TP/QA-01/QA-MT/QA-AB/QA-MB/QA-CT/QA-TW 修订 + QA-BE-17 新增） | **已交付** | **gherkin-parser 8/8 + gherkin-ir-dry-checker 8/8（findings = 建议级 near-duplicate/possible-synonym，与存量同型无结构错误）+ 判据①–⑤覆盖交叉核对全过** |
| coder（bug24） | `project/ezr/`：ui/popups.rs TRACKERS_W 90→94 + 末列预算 12 + 倒计时抗漂移测试；`ezr-demo/` 同步 | 已交付 | bin 904P/0F + 集成 6044P/0F + demo 35P/0F + clippy 0 警告 + fmt 干净 |
| coder（bug23） | config.rs 单位表/模板四键/CLI 帮助；dialogs.rs fmt_speed_limit + 预填×2 + 清空回填；bt_dialog.rs；cmd.rs；bt.rs sidecar partial；engine.rs 反吸血三件；新增测试 12 例；ezr-demo 同步 | 已交付 | bin 903P/0F + 集成 6041P/0F + demo 35P/0F + clippy 0 警告 + fmt 干净 |
| specifier（phase-01/02 及历轮 bug11–22） | features/ 20 份 + qa/ 对应套件 | 已收官/已交付 | 见历轮登记 |

### 遗留受限项（当前有效，沿袭 + 本轮修订）
- 沿袭（bug14–23 轮登记，除注明外全部不变）：cargo-mutants 缺位；死 tracker 约 4 分钟静默窗；
  TUI 恐慌自愈边界（bug6）；QA-SQ-13 竞态沿袭待办；QA-RS-06 沿袭；supervisor block_worker
  fatal 竞态观察项；`App.windows` 失去读者（cleaner 候选裁决）；leech 判据无 e2e 自动化
  （阈值常量固定）；bad_piece 归属口径 = 交付完成块 peer 计 1 次（D67 允许）；启动重校验
  窗内二次暂停丢上一轮 partial——**随本轮 D68⑥ 升格为操作者批准的规格允许边界（非受限项）**。
- **本轮销项**：①「Inversion 访谈受限项（chat 选择题面板不可用）」销项——面板恢复可用，
  六题作答成功；②上轮 handoff 待办（bug23 四项判据 + bug24 判据落规格三层）销项。
- **本轮新登记（coder 轮待办，规格先行偏差）**：config 键旧写法 KB/MB/GB 十进制废弃——
  实现当前仍兼容旧写法（bug23 轮实现 SPEED_UNITS 7 单元含 gb/mb/kb + 兼容测试），与本轮
  规格（FR-02-28/29 v1.11：旧写法不再识别 → 非法回退 0；CLI --max-speed 旧写法参数报错退出）
  存在偏差；coder 轮 = SPEED_UNITS 移除三十进制单元 + 兼容测试翻转 + CLI 报错文案核对 +
  ezr-demo config 键解析同口径核查。
- 工具链：bb v1.12.209（`.tools/bin/bb`，gh-proxy 下载，资产实名 static.tar.gz）+ APS 仓库
  （`.tools/Acceptance-Pipeline-Specification`，gh-proxy clone）；rustup nightly 1.101.0 +
  cargo registry rsproxy；`.tools/` 不跨会话保留，缺时按 notes/environment.md 重建。

### 实现定义值登记（累计口径，本轮修订处加粗）
| 值 | 取值 | 位置 |
|---|---|---|
| leech 阈值 | up ≥ 1 MiB 且 down < 64 KiB 且存活 ≥ 120s | bt/engine.rs LEECH_* 常量 |
| bad_piece 阈值 | 同连接累计 3 次 | BAD_PIECE_BAN_THRESHOLD |
| protocol 阈值 | 未知 id/帧错首次即封；非法区间 request 重复 3 次 | INVALID_REQUEST_BAN_THRESHOLD |
| 限速输入上限 | 32 列（FR-01-06 形态对齐） | routing.rs CAP_LIMIT |
| 限速非法 toast | `⚠ 限速值非法（整数 B/s 或单位字符串，如 512 KiB/s）`（HTTP/BT、下载/上传四处统一） | dialogs.rs / bt_dialog.rs |
| 限速字段占位 | 无占位（空态纯空白） | ui/dialog.rs / ui/bt_dialog.rs |
| **限速单位口径（config 文件键，FR-02-28/29 v1.11 规格）** | **新写法 KiB/MiB/GiB 二进制（1024^n，大小写不敏感，可带 /s 后缀）+ 裸整数与 B/s 后缀 = B/s；旧写法 KB/MB/GB 十进制废弃不再识别（含旧写法的值 = 非法 → 回退默认 0，FR-01-71；实现待 coder 轮对齐）** | config.rs SPEED_UNITS（待删 3 单元）/ parse_speed |
| 限速单位口径（对话框） | 二进制：1 KiB = 1024 B/s、1 MiB = 1024²、1 GiB = 1024³ | routing.rs speed_unit_mult / dialogs.rs parse_speed_limit |
| 限速输入规范化 | 输入/粘贴即规范化显示（KiB/s、MiB/s、GiB/s、B/s；前导零剥离；canonical 完成态吸收单位后缀续入；退格不回写）；裸数字不缀单位（输入态保持原样，确认按 B/s 解析） | routing.rs canonical_speed_form / insert_limit |
| **限速预填/回显显示（FR-02-29 v1.11 规格）** | **fmt_speed_limit 整除链（GiB→MiB→KiB→B，非 1024 整倍数显裸 `N B/s`，0 显「0」）；fmt→parse 恒等；5 预填点 = Add/Modify HTTP + AddBt/ModifyBt + cmd 分流添加对话框** | dialogs.rs fmt_speed_limit |
| **修改窗口清空限速确认（D67② v1.11 修订）** | **回填 config.toml 任务级默认（task_max_download_speed / task_max_upload_speed），HTTP 修改仅下载字段、BT 框双字段各自回填（原「留空 = 保持当前值」退役）；添加窗口空输入回退同值（down_prefill）** | dialogs.rs dlg_confirm_modify / bt_dialog.rs dlg_confirm_modify_bt |
| **BT sidecar partial 持久化（D68）** | **BtSidecar 可选 partial_pieces（serde default + skip_if_empty，版本恒 1 不 bump）；块数据 b64（decode 失败逐块丢弃）；Pause/StopSeed 全量保存；启动恢复验证链 = 覆盖内 ∧ 非 done ∧ 16 KiB 对齐 ∧ 单调不重叠 ∧ 界内；完整恢复 piece 组装校验通过直接落盘转 done；校验失败/写盘失败丢弃重下（自愈）；允许边界（D68⑥）= 启动重校验窗内二次暂停丢上一轮 partial** | model/bt.rs + bt/engine.rs |
| **endgame 已收块豁免（D68⑤）** | endgame 臂跳过 partial 已收块 | engine.rs request_blocks_of_piece endgame 分支 |
| **trackers 弹窗几何（D67⑤ v1.11 修订）** | **94×22（bug24：90→94）**；内框 92×20 + 右缘滚动条列；内容区左右 padding 对称各 2 列；末列「消息」起点 76、预算 = content_w 88 − 76 = **12 列**，「下轮 / 重试 NNNNs」3–6 位秒数完整显示 | ui/popups.rs TRACKERS_W / TRACKERS_COL_MSG |
| 并发满容量语义 | 光标在字段末尾且已满 2 位时键入/粘贴数字 = 清空重输；中位不动不插 | routing.rs insert_conns_digit |
| 多行合并行光标 | 接缝 = 合并前上一行长度处 | multiline.rs backspace |
| 对话框单行字段聚焦光标位 | 聚焦即光标迁最后字符后方；终端光标列 = 行首 + w(label).max(7) + 2 + 值宽，点击 value_x 同式（bug22） | ui/bt_dialog.rs / ui/dialog.rs / ezr-demo ui/dialogs.rs |
| 监听占用警告 | `BT 监听端口 {port} 被占用，已回退随机端口` | engine/mod.rs bind_bt_listener |
| 黑名单损坏警告 | `黑名单文件无法解析（{path}），已按空名单启动` | model/blacklist.rs |
| banned_at 格式 | unix 秒（u64） | model/blacklist.rs BlacklistEntry |
| trackers 空态文案 | `（无 trackers）` | ui/popups.rs |
| HTTP 对话框几何/焦点 | 添加 74×13（字段 7 行焦点 0–6、按钮 7/8/9）；修改 74×11（字段 5 行焦点 0–4、按钮 5/6）；「上传限速」字段退役（HTTP 无上传语义；BT 框保留） | ui/dialog.rs + dialog_keys + mouse.rs |
| 封禁 toast 与黑名单写入顺序 | blacklist_add_with 先于 Evt::Toast 发送（实现事实） | bt/engine.rs ban 路径 |

## 二、当前产出情况

### 本会话产物清单
- **需求层**：`project/mission.md` v1.40 大纲注记（bug23/24 修订批准一行，指向 phase-02 v1.11）；
  `project/mission/phase-02.md` v1.11 头注（面板恢复可用 + chat 修订指令记录）+
  FR-02-28 v1.11 注记（config 键二进制新规范 + 旧写法废弃）+ FR-02-29 v1.11 注记（清空回填 +
  fmt 预填/回显 + 裸数字 B/s）+ FR-02-22 v1.11 注记（partial 持久化）+ FR-02-31 v1.11 注记
  （94×22）+ D67②⑤ v1.11 修订 + **D68 新增**（partial 持久化定义六项 + 允许边界 D68⑥）。
- **规格层（八 feature，稳定序号零重排）**：
  - `01-config-template.feature` v1.15：四限速键取值范围注释改 `≥0；支持 "2 MiB/s" / "500 KiB/s" / 整数 B/s`（只列取值形态、无换算/兼容说明——操作者复核反馈钦定）；Examples 四行 range 列同步；钦定原文四块同步（191 行行集不变）。
  - `01-throttle-proxy.feature` v1.11：头注 v1.11；场景 01/02/03/06/08/10 示例值二进制化（1 MB/s → 1 MiB/s 等）；场景 04 增 `2 MB/s` 废弃回退行；场景 05 改版「新写法二进制生效 + 旧写法十进制废弃」（8 行 Examples：2 MiB/s / 500 KiB/s / 2 GiB / 2000000 裸 / 512 B/s 生效，2 MB/s / 500KB/s 废弃回退 0）；场景 09 预填列改 fmt 形态 + 裸数字 512 行 + config 旧写法行（6 行 Examples）。
  - `01-add-task.feature` v1.41：头注 v1.41；场景 20 预填改 fmt 形态 + 512 裸数字行 + `2 MB/s` 废弃回退行（6 行 Examples）。
  - `03-modify-task.feature` v1.38：头注 v1.38；场景 10 增清空确认回填臂（cfg = 2 MiB/s / 2000000 两形态）+ fmt 回显判据（512 → `512 B/s` 操作者验收判据）+ cfg 列入表。
  - `07-add-bt.feature` v1.40：头注 v1.40；场景 10 双字段 fmt 预填 + 裸数字 512 行 + 旧写法行（4 行 Examples，up_raw 列新增）。
  - `08-modify-bt.feature` v1.7：头注 v1.7；场景 02 预填判据改 fmt 口径；场景 08 增双字段清空各自回填（2 MiB/s / 1 MiB/s）+ fmt 回显（512 / 0）。
  - `09-bt-engine.feature` v1.39：头注 v1.39；**新增场景 17「partial 进度持久化与恢复不重下」**（恢复零重请求 / 字节保真 / 完整 piece 直接落盘 / 篡改自愈 / D68⑥ 允许边界）。
  - `11-trackers-window.feature` v1.12：头注 v1.12；场景清单与场景 02 改 94×22 + 末列预算 12 列（起点 76、内容区宽 88）；场景 03 增 3–6 位秒数完整显示判据。
- **QA 层（八套件）**：qa/01-throttle-proxy（TP-01/02/03/06/08/10 二进制化、TP-04/05 废弃回退行、TP-09 fmt 预填 + 512 行）、qa/01-add-task（01-20 修订）、qa/03-modify-task（MT-10 清空回填 + fmt 回显）、qa/07-add-bt（AB-01 fmt 预填 + AB-10 修订）、qa/08-modify-bt（MB-02 fmt + MB-08 清空回填；**顺带修复 MB-08/09 行脱表排版，编号不动**）、qa/09-bt-engine（**BE-17 新增**三臂）、qa/11-trackers-window（TW-02/03 94×22 + 倒计时完整显示）、qa/01-config-template（CT-01 键集陈旧口径 36→40 修正（v1.14 期漏改）+ v1.15 同步注）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| gherkin-parser | 8/8 通过（八改动 feature） | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-parser <feature> .work/tmp/<stem>.json)` |
| gherkin-ir-dry-checker | 8/8 通过；findings 全为建议级 near-duplicate/possible-synonym（与存量套件同型，无结构错误；剪裁属后续轮裁量） | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-ir-dry-checker <ir> .work/tmp/<stem>.dry.json)` |
| 判据①–⑤覆盖交叉核对 | 全覆盖：①config 键口径 = FR-02-28 v1.11 + 模板 v1.15 + throttle-03/04/05；②清空回填 = FR-02-29 v1.11/D67② + modify-10 + modify-bt-08；③fmt 预填/回显 = throttle-09 + add-20 + add-bt-10 + modify-bt-02/08 + modify-10（512 → `512 B/s` 四处钉死）；④partial 持久化 = FR-02-22 v1.11/D68 + 09-bt-engine-17 + QA-BE-17；⑤94×22 = D67⑤ + trackers-02/03 + QA-TW-02/03 | rg 核对（`_parsed` 残留 0、`废弃`×8、`回填`×10、`94×22`×3） |

### 对账口径
- 本轮纯规格层修订，无代码/测试改动：实现基线维持 bug24 轮登记 6983P/0F 不变。
- 规格与实现已知偏差唯一一处 = config 键旧写法废弃（规格先行、coder 轮对齐，见遗留受限项）；
  其余判据实现均已落地并有专项测试。
- 修订纪律执行：无删除类修订（场景 07-add-bt-07 既有留空先例未触碰）；稳定序号零重排；
  QA 对账数字均标注以实测为准。

### 待办与待批
- **待批（操作者）**：规格就绪待批——本轮八 feature + 需求层修订（清单见上）；如对任一裁决
  有异议可按轮回退（面板答案 + chat 修订指令均已在 phase-02 v1.11 头注留痕）。
- **待办（coder 轮）**：config.rs SPEED_UNITS 移除 gb/mb/kb 三单元（7 → 4）；兼容测试翻转
  （`2 MB/s`/`500 KB/s` → 0 回退断言替换兼容断言）；CLI --max-speed 旧写法报错路径核对
  （opt_speed 已按 0 判非法，验证即可）；ezr-demo config 键解析同口径核查。
- **待办（QA 轮，沿袭扩大）**：QA runner 执行层同步评估本轮判据改动面（TP-04/05 废弃行、
  TP-09/01-20/AB-10 fmt 预填、MT-10/MB-08 清空回填与回显、BE-17 新用例、TW-02/03 几何、
  CT-01 40 键锚点）。
- **待办（cleaner 轮，沿袭）**：`App.windows` 失去读者裁决；handle_peer_msg 参数收拢；
  bt/engine.rs 拆分评估；`up_prefill` 归并评估；`canonical_speed_form` 与 `parse_speed_limit`
  解析前缀重复归并评估（含 fmt_speed_limit 整除链同族）。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/coder`（config 键旧写法废弃实现对齐）或
  `bin/swarm begin six-pack/cleaner`（沿袭清理）：以本文件为上游交接。
- 后续链：specifier → coder / cleaner → architect → hardender → QA（宪法第一章：
  不自动流转）。
- 操作者授权的收尾口径：`bin/swarm complete` 后立即将产物 tar.gz 复制到
  `/home/z/my-project/`（沿 bug18/20/21/22/23/24 轮授权口径执行）。
- 工具链交接：bb v1.12.209 + APS 已装 `.tools/`（不跨会话保留，重建口径见
  notes/environment.md）；rustup nightly 1.101.0（PATH = ~/.cargo/bin）+ rsproxy。
