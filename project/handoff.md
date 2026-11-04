# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（本轮已完成）→ 建议下一节点 six-pack/coder
> 会话：ezr-specifier-flags-column（2026-11-05，specifier 轮：并发 panel BT 行「标志」列
> FR-02-33/D69——qBt Peers Flags 子集 D/d/U/u/O/I/K/?、BT 八列形态 99 宽、收缩基线 65、
> 帮助【并发连接】组标志缩写说明 5 条目（帮助 75→80 行））
> 上游交接：project/handoff.md（config 废弃轮 coder 交接）

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式 v1.41）+ `project/mission/phase-01…04.md`。
- phase-01（HTTP/HTTPS 内核与 TUI）收官；phase-02（BT 内核/做种/管理）已实现并经
  互操作夹具多轮收口。历轮 bug11–24 全部落地；本轮为「并发 panel 标志列」轮（FR-02-33）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| **specifier（本轮，标志列）** | 需求层 mission.md v1.41 + phase-02 v1.12（FR-02-33/D69）；规格层 04-ui-conns v1.41（场景 08 八列）、09-bt-engine v1.40（场景 18 新增）、01-tui-display v1.44（场景 19 帮助）；QA 层 QA-UC-08/QA-BE-18/QA-TD-19 同步 | 已交付（规格待批：D69） | gherkin-parser 3/3（IR 18/18/25 场景）+ dry-checker 3/3（建议级 findings，涉新场景命中均中文分词假阳性、人工复核不改文本） |
| coder（config 废弃轮） | `project/ezr/`：config.rs SPEED_UNITS 7→4 + 测试翻转 + 废弃断言；main.rs CLI 报错；property_tests 翻转。`ezr-demo/`：守卫测试 + docs + README 37 | 已交付 | bins 952P/0F + 集成 5232P/0F + demo 36P/0F + clippy 双 0 警告 + fmt 双干净 + CLI 实机报错验证 |
| specifier（bug23/24 归档轮） | 需求层 mission.md v1.40 + phase-02 v1.11；规格层 features 八文件；QA 层八套件同步 | 已交付 | gherkin-parser 8/8 + dry-checker 8/8（建议级 findings）+ 判据①–⑤交叉核对全过 |
| coder（bug23/bug24） | popups.rs TRACKERS_W 94、config 键限速/预填/清空回填、bt_dialog、sidecar partial、反吸血、listen_port；ezr-demo 同步 | 已交付 | bin 903→904P/0F + demo 35P/0F + clippy/fmt 干净 |
| specifier（phase-01/02 及历轮 bug11–22） | features/ 20 份 + qa/ 对应套件 | 已收官/已交付 | 见历轮登记 |

### 遗留受限项（当前有效，沿袭 + 本轮修订）
- 沿袭（bug14–24 轮登记，除注明外全部不变）：cargo-mutants 缺位；死 tracker 约 4 分钟
  静默窗；TUI 恐慌自愈边界（bug6）；QA-SQ-13 竞态沿袭待办；QA-RS-06 沿袭；supervisor
  block_worker fatal 竞态观察项；`App.windows` 失去读者（cleaner 候选裁决）；leech 判据
  无 e2e 自动化（阈值常量固定）；bad_piece 归属口径 = 交付完成块 peer 计 1 次（D67
  允许）；启动重校验窗内二次暂停丢上一轮 partial（D68⑥ 规格允许边界，非受限项）；
  v147 `engine_conns_quota_exact_and_unique_ids` 负载敏感 flake（连续两轮四次 full-run
  FAIL / 单跑五连绿 0.3s 级，环境性非回归，notes/testing.md 实录）。
- **本轮新增**：无受限项——本轮纯规格轮。P/S/X/H/E/e/L 六旗标不出现属规格钦定边界
  （对应能力 phase-02 未实现），非受限项（09-bt-engine-18 末条）。
- 工具链：bb v1.13.229（`.tools/bin/bb`）+ APS 仓库（`.tools/Acceptance-Pipeline-
  Specification`）**本轮已装齐并跑通**（gh-proxy 前缀；下会话 `.tools/` 不跨会话保留，
  需要时按 notes/environment.md 重建）；rustup nightly 1.101.0（USTC 前台装齐，PATH =
  ~/.cargo/bin）+ cargo registry USTC sparse——本轮未动，沿用。

### 实现定义值登记（累计口径，本轮新增处加粗）
| 值 | 取值 | 位置 |
|---|---|---|
| **标志列参考源（D69①，待批）** | **qBittorrent Peers（用户）面板 Flags 列（中文列名「标志」）；Trackers 面板无 Flags 列（官方 FAQ 核证）** | phase-02 FR-02-33 |
| **标志集（D69②，待批）** | **qBt 官方 FAQ 图例 ezr 可产生子集 {D,d,U,u,O,I,K,?}（P/S/X/H/E/e/L 不出现）；判定表见 09-bt-engine-18** | bt/engine.rs ConnStat + choker 乐观槽 + 入连方向（coder 承接） |
| **标志列结构（D69③④⑤，待批）** | **地址(18)/标志(5)/下载速度(8)/上传速度(8)/累计下载(8)/累计上传(8)/客户端(34)/协议(4)，内容宽 99；多标志 qBt 图例序无空格连写；无标志 `-`；数据左对齐、DIM 色** | ui/conns.rs BT_COLW（coder 承接） |
| **客户端收缩基线（D69⑥，待批）** | **max(26, 视口宽 − 65)（基线 60→65、下限 26 不变；视口 ≥ 91 整表完整可见、≥ 99 维持 34）** | ui/conns.rs BT_CLIENT_SHRINK_BASE（coder 承接） |
| leech 阈值 | up ≥ 1 MiB 且 down < 64 KiB 且存活 ≥ 120s | bt/engine.rs LEECH_* 常量 |
| bad_piece 阈值 | 同连接累计 3 次 | BAD_PIECE_BAN_THRESHOLD |
| protocol 阈值 | 未知 id/帧错首次即封；非法区间 request 重复 3 次 | INVALID_REQUEST_BAN_THRESHOLD |
| 限速输入上限 | 32 列（FR-01-06 形态对齐） | routing.rs CAP_LIMIT |
| 限速非法 toast | `⚠ 限速值非法（整数 B/s 或单位字符串，如 512 KiB/s）`（HTTP/BT、下载/上传四处统一） | dialogs.rs / bt_dialog.rs |
| 限速字段占位 | 无占位（空态纯空白） | ui/dialog.rs / ui/bt_dialog.rs |
| 限速单位口径（config 文件键） | 新写法 KiB/MiB/GiB 二进制（1024^n，大小写不敏感，可带 /s 后缀）+ 裸整数与 B/s 后缀 = B/s；旧写法 KB/MB/GB 十进制废弃不再识别（非法回退 0，FR-01-71）——SPEED_UNITS 4 单元 gib/mib/kib/b | config.rs SPEED_UNITS / parse_speed |
| CLI --max-speed 旧写法 | 同源 parse_speed：旧写法参数 = 启动报错退出（exit 2） | main.rs opt_speed |
| 限速单位口径（对话框） | 二进制：1 KiB = 1024 B/s、1 MiB = 1024²、1 GiB = 1024³；输入速记 k/kb/kib 等同映射二进制（与 config 键层废弃口径不同面属有意设计） | routing.rs speed_unit_mult / dialogs.rs parse_speed_limit |
| 限速输入规范化 | 输入/粘贴即规范化显示（KiB/s、MiB/s、GiB/s、B/s；前导零剥离；canonical 完成态吸收单位后缀续入；退格不回写）；裸数字不缀单位 | routing.rs canonical_speed_form / insert_limit |
| 限速预填/回显显示 | fmt_speed_limit 整除链（GiB→MiB→KiB→B，非 1024 整倍数显裸 `N B/s`，0 显「0」）；fmt→parse 恒等；5 预填点 | dialogs.rs fmt_speed_limit |
| 修改窗口清空限速确认（D67②） | 回填 config.toml 任务级默认（task_max_download_speed / task_max_upload_speed）；添加窗口空输入回退同值 | dialogs.rs / bt_dialog.rs dlg_confirm_modify* |
| BT sidecar partial 持久化（D68） | BtSidecar 可选 partial_pieces（serde default + skip_if_empty，版本恒 1）；恢复验证链五条件；允许边界（D68⑥）= 启动重校验窗内二次暂停丢上一轮 partial | model/bt.rs + bt/engine.rs |
| endgame 已收块豁免（D68⑤） | endgame 臂跳过 partial 已收块 | engine.rs request_blocks_of_piece |
| trackers 弹窗几何（D67⑤） | 94×22；内框 92×20 + 右缘滚动条列；内容区左右 padding 对称各 2 列；末列「消息」起点 76、预算 12 列 | ui/popups.rs TRACKERS_W / TRACKERS_COL_MSG |
| 并发满容量语义 | 光标在字段末尾且已满 2 位时键入/粘贴数字 = 清空重输；中位不动不插 | routing.rs insert_conns_digit |
| 多行合并行光标 | 接缝 = 合并前上一行长度处 | multiline.rs backspace |
| 对话框单行字段聚焦光标位 | 聚焦即光标迁最后字符后方；终端光标列 = 行首 + w(label).max(7) + 2 + 值宽 | ui/bt_dialog.rs / ui/dialog.rs / ezr-demo ui/dialogs.rs |
| 监听占用警告 | `BT 监听端口 {port} 被占用，已回退随机端口` | engine/mod.rs bind_bt_listener |
| 黑名单损坏警告 | `黑名单文件无法解析（{path}），已按空名单启动` | model/blacklist.rs |
| banned_at 格式 | unix 秒（u64） | model/blacklist.rs BlacklistEntry |
| trackers 空态文案 | `（无 trackers）` | ui/popups.rs |
| HTTP 对话框几何/焦点 | 添加 74×13（字段 7 行焦点 0–6、按钮 7/8/9）；修改 74×11（字段 5 行焦点 0–4、按钮 5/6）；「上传限速」字段退役 | ui/dialog.rs + dialog_keys + mouse.rs |
| 封禁 toast 与黑名单写入顺序 | blacklist_add_with 先于 Evt::Toast 发送 | bt/engine.rs ban 路径 |

## 二、当前产出情况

### 本会话产物清单
- **需求层**：`project/mission.md` v1.41 注记；`project/mission/phase-02.md` v1.12
  （头注 + **FR-02-33** 并发 panel BT「标志」列 + **D69** 设计定义值待批）。
- **规格层（Gherkin）**：
  - `features/04-ui-conns.feature` v1.41：场景 08 BT 七列改八列（地址(18)/**标志(5)**/…/协议(4)，
    内容宽 99；标志连写/`-`/左对齐/DIM；收缩基线 60→65、视口下限 86→91；末列吸附零改动注记）。
  - `features/09-bt-engine.feature` v1.40：**场景 18 新增**（标志判定表：D/d/U/u/O/I/K/? ×
    引擎状态源；连写顺序；实时翻转；HTTP 无标志列；P/S/X/H/E/e/L 不出现边界）。
  - `features/01-tui-display.feature` v1.44：场景 19 帮助【并发连接】组新增**标志缩写说明
    5 条目**（标志 D/d、U/u、O/I、K、?），内容总行数 75→80。
- **QA 层**：`qa/04-ui-conns-qa.md` v1.41（QA-UC-08 八列判据 + 收缩基线判据）；
  `qa/09-bt-engine-qa.md` v1.40（**QA-BE-18 新增**：fixture 驱动判定表逐项 + wire 翻转）；
  `qa/01-tui-display-qa.md` v1.44（QA-TD-19 帮助 80 行 + 5 条目逐字判据）。
- **project/notes/**：architecture.md（Flags 取证锚点/旗标裁剪法/判定表映射/极值互斥推演/
  列插入位置/收缩基线平移纪律/帮助图例宽度上限）、environment.md（bb+APS 本轮复测装齐）、
  INDEX.md 两行同步。
- `project/handoff.md`：本文件（整理重写）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| gherkin-parser | 3/3 通过（IR 场景 18/18/25，场景 18 名称与判定表在案） | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-parser <feature> .work/tmp/<stem>.json)` |
| gherkin-ir-dry-checker | 3/3 通过（findings 97/64/227 全 advisory；人工复核涉新场景命中 = 中文分词假阳性——notes/aps.md 在案口径，不改规格文本） | `bb gherkin-ir-dry-checker <ir> <out>` |
| 判据①–③交叉核对 | FR-02-33 ↔ 04-ui-conns-08 ↔ QA-UC-08 列宽/基线/视口下限三处数字一致（99/65/91）；09-bt-engine-18 ↔ 01-tui-display-19 ↔ QA-TD-19 标志缩写文案逐字一致 | 人工核对（rg 锚点检索） |

### 对账口径
- 规格基线 = 上轮 coder 交接（features/qa 26+ 份全绿基线）；本轮增量 = 3 份 feature +
  3 份 QA 修订、2 份 mission 文件，其余 23 份 feature / 15 份 QA 未触碰（rg 版本注记检索核对）。
- 场景序号稳定：04-ui-conns-08、01-tui-display-19 原序号修订不重排；09-bt-engine-18 新增尾接。

### 待办与待批
- **待批（操作者）**：D69 七项裁决——①参考源 = qBt Peers（用户）面板 Flags 列
  （Trackers 面板无 Flags 列经官方 FAQ 核证）；②标志集 = {D,d,U,u,O,I,K,?}（P/S/X/H/E/e/L
  对应能力未实现不出现）；③列位置 = 地址后第二列（首屏可见，非 qBt GUI 的列尾）；④列宽 5 =
  极值 4 枚连写 + 1 空白、无空格连写；⑤无标志 `-`、左对齐、DIM 色；⑥收缩基线 60→65
  （视口下限 86→91）；⑦Inversion 受限退化沿 bug13–18 轮先例。如对任一裁决有异议可回退
  （改动均为规格注记级）。
- **待办（coder 轮）**：①FR-02-33 实现（ui/conns.rs 八列 + BtConnView 扩展标志字段 +
  engine conn_views 判定注入 + ConnStat 入连方向/乐观槽标志贯通）；②帮助窗口 5 条目
  （ui/help.rs 75→80 行 + help_lines_len 断言同步）；③ezr-demo 并发 panel BT 行同构同步 +
  帮助组同步 + docs《ezr-demo UI界面及交互设计规范》§6 同步 + 守卫测试；④既有测试面
  翻转评估（BT_COLW/内容宽 94→99 相关断言、收缩基线断言）。
- **待办（QA 轮，沿袭）**：QA runner 执行层同步（上轮清单 + 本轮 QA-UC-08/QA-BE-18/QA-TD-19）。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/coder`：以本文件为上游交接（FR-02-33 实现 +
  D69 落地 + ezr-demo 同构同步）。
- 后续链：coder → cleaner → architect → hardender → QA（宪法第一章：不自动流转）。
- 操作者授权的收尾口径：`bin/swarm complete` 后立即将产物 tar.gz 复制到
  `/home/z/my-project/`（沿 bug18/20/21/22/23/24 轮授权口径执行）。
- 工具链交接：bb v1.13.229 + APS 本会话已装（`.tools/`，不跨会话保留；需要时按
  notes/environment.md 重建）；rustup nightly 1.101.0（PATH = ~/.cargo/bin）+ cargo
  registry USTC sparse 沿用。
