# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（本轮已完成）→ 建议下一节点 six-pack/cleaner
> 会话：ezr-coder-qa-runner-sync（2026-11-04，coder 轮：QA runner 执行层同步——handoff
> 沿袭待办承接，操作者钦定 bin/swarm begin six-pack/coder：QA-TD-19 帮助断言 v1.44 口径
> 同步（十二组 80 行 + 标志缩写 5 条目锚点）+ QA-TD-07 T 键 FR-02-31 门控语义同步 +
> QA-UC-08/QA-BE-18 既定形态注记登记）
> 上游交接：project/handoff.md（标志列实现轮 + D69 批准登记轮交接）

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式 v1.41）+ `project/mission/phase-01…04.md`。
- phase-01（HTTP/HTTPS 内核与 TUI）收官；phase-02（BT 内核/做种/管理）已实现并经
  互操作夹具多轮收口。历轮 bug11–24 全部落地；本轮为「并发 panel 标志列」轮（FR-02-33）
  实现落地。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| **coder（本轮，QA runner 执行层同步）** | `project/qa/runners/`：suite_tui_display.py QA-TD-19 同步 v1.44 口径（docstring + 组列表 8→12 组逐字取 help.rs sec() 清单 + 滚动收集 7→10×PgDn 适配 80 行 + 【并发连接】标志缩写 5 条目逐字锚点）；QA-TD-07 T/t 断言同步 FR-02-31 门控语义（toast 二选一 + 页签不变 + 不开窗）；suite_ui_conns.py 头部 v1.41 同步注（BT 八列判据单测锁登记 + QA-BE-18 执行方式核对）+ notes/testing.md 经验条目 | 已交付 | TD-19 单用例 P（12.8s）+ TD-07 单用例 P；全套 TD P=19+1（TD-07 同步后）/F=1、UC P=15/F=3——四个 F（TD-23 下拉点按时序、UC-07/17/18 校验码填入链）经 HEAD worktree 对照全部同形 = 环境预存非回归（本轮零产品代码改动） |
| coder（上轮，标志列实现） | `project/ezr/`：engine 侧 `ConnStat.inbound` + `conn_flags` 判定表 + `conn_views` 注入 + listener accept 入连标记 + 判定表单元测试；`BtConnView`/`BtConnRow` 增 `flags` + evt.rs 透传 + 全部字面量构造点补字段；`ui/conns.rs` 八列形态（BT_COLW [18,4,8,8,8,8,34,4]/内容宽 99/收缩基线 65/标志左对齐 DIM/`-` 空值）+ 测试面翻转；`ui/help.rs` 【并发连接】组 5 条目（75→80）+ 断言翻转。`ezr-demo/`：`Connection.flags` + FLAG_POOL 伪随机判定池 + `ui/conns.rs` 八列 + 帮助组 5 条目 + docs 规范（头部注记/§5.4/§6.6）+ README 记录 38 + 守卫测试 6 个 | 已交付 | **bins 956P/0F + 集成 6091P/0F（12 组全绿，含 v147 负载敏感组 695P）= 7047P/0F + clippy 0 警告 + fmt 干净**；ezr-demo 42P/0F + fmt 干净（clippy 9 警告均未触碰文件既有基线） |
| specifier（上轮，标志列规格） | 需求层 mission.md v1.41 + phase-02 v1.12（FR-02-33/D69）；规格层 04-ui-conns v1.41（场景 08 八列）、09-bt-engine v1.40（场景 18 新增）、01-tui-display v1.44（场景 19 帮助）；QA 层 QA-UC-08/QA-BE-18/QA-TD-19 同步 | 已交付（D69 已批：操作者 20261104 chat 批准七项全项） | gherkin-parser 3/3（IR 18/18/25 场景）+ dry-checker 3/3（建议级 findings，涉新场景命中均中文分词假阳性、人工复核不改文本） |
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
  v147 `engine_conns_quota_exact_and_unique_ids` 负载敏感 flake（环境性非回归，
  notes/testing.md 实录；**本轮 full-run 695P 全绿**，沿「单跑绿 + 改动无接触面再下
  非回归结论」判法继续观察）。
- **本轮新增**：无受限项——FR-02-33/D69 全项落地，实现与规格判定表逐项一致。
- P/S/X/H/E/e/L 六旗标不出现属规格钦定边界（对应能力 phase-02 未实现），非受限项
  （09-bt-engine-18 末条）。
- ezr-demo clippy 9 警告为未触碰文件既有基线（cmd/dialogs/mouse/multiline/ui/dialogs/
  ui/tree），本轮改动零新增，归 cleaner 轮候选清理面。
- 工具链：bb v1.13.229（`.tools/bin/bb`）+ APS 仓库（`.tools/Acceptance-Pipeline-
  Specification`）上轮已装齐跑通（gh-proxy 前缀；下会话 `.tools/` 不跨会话保留，
  需要时按 notes/environment.md 重建）；rustup nightly 1.101.0（前台装齐，PATH =
  ~/.cargo/bin）+ cargo registry sparse——本轮未动，沿用。

### 实现定义值登记（累计口径，本轮落地处加粗）
| 值 | 取值 | 位置 |
|---|---|---|
| 标志列参考源（D69①，已批） | qBittorrent Peers（用户）面板 Flags 列（中文列名「标志」）；Trackers 面板无 Flags 列（官方 FAQ 核证） | phase-02 FR-02-33 |
| 标志集（D69②，已批） | qBt 官方 FAQ 图例 ezr 可产生子集 {D,d,U,u,O,I,K,?}（P/S/X/H/E/e/L 不出现）；判定表见 09-bt-engine-18 | bt/engine.rs `conn_flags`（**本轮落地**） |
| 标志列结构（D69③④⑤，已批） | 地址(18)/**标志(5 = 数据 4 + 分隔 1)**/下载速度(8)/上传速度(8)/累计下载(8)/累计上传(8)/客户端(34)/协议(4)，内容宽 99；多标志 qBt 图例序无空格连写；无标志 `-`；数据左对齐、DIM 色 | ui/conns.rs BT_COLW/BT_CONTENT_W（**本轮落地**） |
| 客户端收缩基线（D69⑥，已批） | max(26, 视口宽 − 65)（基线 60→65、下限 26 不变；视口 ≥ 91 整表完整可见、≥ 99 维持 34） | ui/conns.rs BT_CLIENT_SHRINK_BASE（**本轮落地**） |
| 标志判定引擎状态源（**本轮落地**） | D/d = !peer_choking/peer_choking ∧ 该连接有未完成需求（covered∩未 done∩peer has）；U/u = peer_interested ∧ !am_choking/am_choking；O = apply_choker optimistic_slot 命中；I = listener accept 入连标记（ConnStat.inbound）；K = !peer_choking ∧ 无未完成需求；? = !am_choking ∧ !peer_interested | bt/engine.rs conn_flags + ConnStat.inbound + engine/mod.rs accept |
| demo 标志演示池（**本轮落地**） | FLAG_POOL 12 形态（`-`/D/d/U/u/DU/DUO/UI/I/K/?/DUOI）互斥规则合法、seed+槽位确定性、极值 DUOI 在池 | ezr-demo model.rs FLAG_POOL/pseudo_flags |
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
- **产品端 `project/ezr/`（FR-02-33 实现）**：
  - `src/bt/engine.rs`：`ConnStat` 增 `inbound`；`conn_flags` 判定表函数（D/d/U/u/O/I/K/?
    逐项 + qBt 图例序连写 + 空值 `-`）；`conn_views` 注入 flags（done/optimistic_slot
    入参，7 调用点同步）；listener accept 路径 `inc.inbound = true`；测试模块新增
    判定表逐项/连写互斥/极值用例。
  - `src/engine/mod.rs`：`BtConnView` 增 `flags: String`。
  - `src/app/mod.rs`：`BtConnRow` 增 `flags: String`；`src/app/engine/evt.rs` BtConns
    事件透传；`src/app/engine/bt.rs`、`src/app/keys.rs`、`src/app/mouse.rs`、
    `src/ui/pieces.rs` 既有字面量构造点全部补字段。
  - `src/ui/conns.rs`：BT_COLW 7→8 列 [18, **4**, 8, 8, 8, 8, 34, 4]（标志数据 4 +
    分隔 1 = 视觉 5）；BT_CONTENT_W 94→99；BT_CLIENT_SHRINK_BASE 60→65；表头插
    「标志」（左对齐 DIM）+ 数据行插标志单元格（`cell_left` DIM、`-` 空值）；索引
    平移（客户端 5→6、协议 6→7）；测试翻转（bt_seven→bt_eight_columns、视口
    65/97/105 断言、收缩/表头断言新口径）+ 新增 `bt_table_complete_at_viewport_91`。
  - `src/ui/help.rs`：【并发连接】组新增标志缩写 5 条目（D/d、U/u、O/I、K、?，文案与
    09-bt-engine-18 判定语义逐字一致）；help_lines_len 断言 75→80、End 钳制 55→60。
- **demo 端 `ezr-demo/`（同构同步）**：
  - `src/app/model.rs`：`Connection` 增 `flags: String`；`FLAG_POOL` 12 形态 +
    `pseudo_flags`（seed+槽位确定性，互斥规则合法，极值 DUOI）；构造单点
    `make_chunk_conns_lease` 注入；守卫测试 3 个（互斥/极值/确定性/构造注入）。
  - `src/ui/conns.rs`：BT 八列 [18,4,8,8,8,8,34,4] + 收缩 `colw[6] = vw − 65`（下限
    26）+ 表头/数据行标志单元格（左对齐 DIM）。
  - `src/ui/popups.rs`：帮助【并发连接】组 5 条目（与产品端逐字一致）。
  - `src/ui/mod.rs`：守卫测试 3 个（八列表头序/DUOI 渲染/窄视口无 `…`/帮助条目 +
    81 行实测钉值）。
  - `docs/ezr-demo UI界面及交互设计规范.md`：头部修订注记 + §5.4（列序表八列 99、
    标志 bullet 全语义、横向滚动口径、收缩基线 65）+ §6.6（内容 81 行源码实测 +
    【并发连接】组 5 条目；历史登记 68 行为滞后值一并修正）。
  - `README.md`：变更记录 38。
- **`project/notes/`**：testing.md 增 1 条（demo 文档计数登记以源码实测为准 +
  filtered() 视图渲染测试面变体）；INDEX.md 同条目尾部并入。
- `project/handoff.md`：本文件（整理重写）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 产品端 bins 单元测试 | **956P/0F**（910 main + 30/1/15 附属 bin；上轮 952 → +4 引擎判定表新测试） | `cd project/ezr && CARGO_INCREMENTAL=0 cargo test --bins` |
| 产品端集成测试（12 组） | **6091P/0F**（hardening 142、engine_backfill 813、g_appcore 712、g_engine 707、g_main 2、g_proxy 16、g_slots 202、g_supervisor 681、g_throttle_err 442、g_v147 695、ui_backfill 843、v115 836） | `CARGO_INCREMENTAL=0 cargo test --tests`（分目标跑同绿） |
| clippy（产品端） | 0 警告（--all-targets） | `CARGO_INCREMENTAL=0 cargo clippy --all-targets` |
| fmt（产品端） | 干净（首查 1 处 bt/engine.rs diff 已 fmt 修复，bins 复跑无损复验） | `cargo fmt --check` |
| ezr-demo 测试 | **42P/0F**（含本轮 6 个新守卫测试） | `cd ezr-demo && CARGO_INCREMENTAL=0 cargo test` |
| ezr-demo fmt / clippy | fmt 干净；clippy 9 警告均未触碰文件既有基线（cmd/dialogs/mouse/multiline/ui/dialogs/ui/tree） | `cargo fmt --check` / `cargo clippy --all-targets` |
| 判据锚点核对 | 标志缩写 5 条目文案：产品端 help.rs = ezr-demo popups.rs = 01-tui-display-19 逐字一致；列宽数字 99/65/91 三处（规格/QA/实现）一致 | rg 锚点检索 |

### 对账口径
- 规格基线 = 上轮 specifier 交接（features/qa 26+ 份全绿基线）；本轮**未触碰任何
  features/qa/mission 文件**（coder 职责边界），实现全部落在 `project/ezr/src/` 与
  `ezr-demo/`；rg 版本注记检索可复核。
- 上次中断会话的未提交工作区改动经逐文件核对与本轮任务口径一致（步骤 1–6 产物），
  补验后并入本轮交付，无重复实现、无口径分叉。
- 帮助内容行数：产品端 80 行（v1.44 口径）；ezr-demo 81 行（源码实测基线 76 + 5，
  与产品端行数本就不同源——demo 组态历史增量独立演进，本轮已按实测登记）。

### 待办与待批
- **已销账（本轮，coder runner 同步）**：「QA runner 执行层同步（上轮清单 +
  QA-UC-08/QA-BE-18/QA-TD-19 对应 runner 断言面）」——QA-TD-19 已同步 v1.44 口径
  （runner 维持 HTTP 驱动的 QA-UC-08 BT 判据按既定形态注记登记，QA-BE-18 fixture 侧
  判定表测试核对在位）；沿袭的 runner 欠账清单清零。
- **待批（specifier 轮，新登记）**：QA-TD-07 规程行滞后——`01-tui-display-qa.md` QA-TD-07
  判据「]/[/T 切/循环页签」未随 phase-02 v1.9/FR-02-31 修订（T/t 现为 trackers 弹窗门控键、
  页签循环仅 ]/[ 承担、帮助【页签】组 T 条目已删 01-tui-display v1.42）；runner 已按已批准
  需求语义同步，规程行修订待 specifier 轮落账。
- **环境预存 FAIL 登记（非回归，沿 testing.md 归因口径）**：QA-TD-23（下拉展开期点按
  不应拖拽窗口）与 QA-UC-07/17/18（添加对话框校验码填入链在本沙箱 pty 时序下不生效
  →详情无校验行）——四例 HEAD worktree 对照全部同形失败（本轮零产品代码改动），既有
  QA 执行环境曾全绿过；不放宽断言，待 QA 轮环境适配/复验。
- **待办（cleaner 轮候选，沿袭）**：`App.windows` 失去读者裁决；ezr-demo 既有 9 处
  clippy 警告清理（未触碰文件基线，runner 轮零新增）。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接（FR-02-33 实现轮
  收尾：重复/死代码清理、`App.windows` 裁决、demo clippy 基线清理候选）。
- 可选 specifier 小轮：QA-TD-07 规程行修订（T 循环页签 → trackers 门控，见待批节）。
- 后续链：cleaner → architect → hardender → QA（宪法第一章：不自动流转）。
- 操作者授权的收尾口径：`bin/swarm complete` 后立即将产物 tar.gz 复制到
  `/home/z/my-project/`（沿 bug18/20/21/22/23/24 轮授权口径执行）。
- 工具链交接：bb v1.13.229 + APS 本会话沿用上轮安装（`.tools/` 不跨会话保留；需要时
  按 notes/environment.md 重建）；rustup nightly 1.101.0（PATH = ~/.cargo/bin）+
  cargo registry sparse 沿用。
