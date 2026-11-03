# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（本轮已完成）→ 建议下一节点 six-pack/specifier
> （bug23/bug24 新判据落规格三层）或 six-pack/cleaner
> 会话：ezr-coder-bug24（2026-11-03，bug24 轮 coder：trackers 弹窗末列「消息」倒计时
> 3 位秒数被 '…' 截断——窗口 90→94、末列预算 8→12 列（容 6 位秒数）；TDD 抗漂移
> 断言（数字段位数 + 's' 收尾，规避测试与渲染两次 Instant::now() 秒级漂移）；
> ezr-demo 代码 + README 变更记录 36 + UI 规范 §6.12 同步）
> 会话：ezr-coder-bug23（2026-11-03，bug23 轮 coder：①config.toml 限速单位口径改二进制
> B/s·KiB/s·MiB/s（旧 KB/MB 十进制兼容）；②BT/HTTP 添加/修改对话框限速输入——清空确认
> 回填 config 任务级默认、纯数字恒按 B/s（512 → 512 B/s）、预填/回显规范化单位显示；
> ③BT 暂停→恢复触发 BitComet 1.64 seeder 反吸血排查与根治（partial 块 sidecar 持久化 +
> endgame 跳过已收块）；④ezr-demo 代码 + 文档同步）
> 上游交接：project/handoff.md（bug23 轮 coder 交接；bug24 任务来源 = 操作者 chat
> 直启 bug 报告；bug23 任务来源 = 操作者 chat 直启 bug 报告三项 + 附加指令）

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式 v1.39）+ `project/mission/phase-01…04.md`。
- phase-01（HTTP/HTTPS 内核与 TUI）收官；phase-02（BT 内核/做种/管理）已实现并经
  互操作夹具多轮收口。历轮 bug11–22 全部落地。
- **本轮（coder，bug23）**：限速单位三层口径归一（config 键解析 + 对话框字段 + 显示）、
  对话框限速输入确认语义重定义（清空回填 config 默认 / 纯数字 B/s / 规范化显示）、
  BT 暂停→恢复重复下载特征根治（反吸血）、ezr-demo 同步。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| **coder（本轮，bug24）** | `project/ezr/`：ui/popups.rs TRACKERS_W 90→94 + 末列预算 8→12 + 倒计时抗漂移测试 1 例；`ezr-demo/`：popups.rs 同步 + README 变更记录 36 + UI 规范 4 处 | **已交付** | **bin 904P/0F + 集成 12 目标 6044P/0F + demo 35P/0F + clippy 0 警告 + fmt 干净（demo 存量漂移 5 文件沿袭）** |
| specifier（phase-01/02 及历轮 bug11–22） | features/ 20 份 + qa/ 对应套件 | 已收官/已交付 | 见历轮登记 |
| coder（phase-02 及历次收口轮，含 bug18/21/22） | `project/ezr/` 全量 | 已交付 | 见历轮登记 |
| **coder（本轮，bug23）** | `project/ezr/`：config.rs 单位表/模板四键/CLI 帮助；dialogs.rs fmt_speed_limit + 预填×2 + 清空回填；bt_dialog.rs 预填×2 + 清空回填；cmd.rs 预填；bt.rs sidecar partial 字段 + b64 编解码；engine.rs save_full/restore_sidecar_partial/组装写盘因子化/Pause 全量保存/endgame 跳过已收块；新增测试 12 例；`ezr-demo/`：dialogs.rs 清空回填 + limit_backfill_tests 4 例 + README 记录 35 + 规范 §6.1/§6.2 | **已交付** | **bin 903P/0F（基线 891 + 净增 12）+ 集成 12 目标 6041P/0F + demo 35P/0F + clippy 0 警告 + fmt 干净** |

### 遗留受限项（当前有效，沿袭 + 本轮登记）
- 沿袭（bug14–22 轮登记，全部不变）：cargo-mutants 缺位；死 tracker 约 4 分钟静默窗；
  TUI 恐慌自愈边界（bug6）；BT partial 进度不持久化——**本轮销项**（见下）；QA-SQ-13
  竞态沿袭待办；QA-RS-06 沿袭；supervisor block_worker fatal 竞态观察项；`App.windows`
  失去读者（cleaner 候选裁决）；leech 判据无 e2e 自动化（阈值常量固定）；bad_piece
  归属口径 = 交付完成块 peer 计 1 次（D67 允许）；Inversion 访谈受限项（chat 选择题
  面板不可用）。
- **本轮销项**：①操作者 bug24 报告销项（trackers 末列 3 位倒计时 '…' 截断，窗口与
  末列同步放宽后完整显示）；②操作者 bug23 复核反馈销项（config 模板注释去换算/兼容
  说明括注，只列取值形态）；③操作者 bug23 三项报告全部修复销项；④「BT partial 进度
  不持久化」沿袭受限项随 bug23 反吸血修复销项（sidecar 现携带 partial 块，恢复不重下；
  启动重校验窗内二次暂停仍丢上一轮 partial，属登记的可接受边界，见 notes）。
- **本轮登记（flake 观察项）**：bin 1 例、v147 `engine_conns_quota_exact_and_unique_ids`
  套件负载下 2 连败（真 TCP 桩 + 25s 截止的负载敏感 e2e；单跑 0.31s 即绿），均按
  notes/testing.md bug17 轮口径复跑全绿判环境性时序非回归——沙箱高负载时段该用例
  稳定性下降，沿袭观察（不为本轮改动所致：纯 UI 常量 + 测试，无引擎接触面）。
- **本轮新登记（specifier 轮/后续）**：①「限速单位三层口径」通用判据（config 键
  二进制 KiB/MiB/GiB 新规范 + 旧 KB/MB 十进制兼容解析、对话框/CLI 同源 parse_speed、
  裸数字恒 B/s）待 specifier 落 features/QA 三层（沿 bug21/22 先例：coder 先修、
  specifier 归档）；②「修改窗口清空限速确认 = 回填 config 任务级默认」判据待归档
  （01-add-task/03-modify-task/07-add-bt/08-modify-bt 四 feature 的确认场景增补）；
  ③「预填/回显规范化单位显示 + fmt→parse 恒等」判据待归档；④BT sidecar partial
  持久化行为（恢复不重下、坏块自愈）待归档（09-bt-engine/01-resume-sidecar 相关场景）。
- 工具链：rustup nightly 1.101.0（rsproxy 双变量直链装齐）+ cargo registry rsproxy；
  bb/APS 沿袭（.tools/bin、.tools/Acceptance-Pipeline-Specification，本轮未用）；
  环境不持久时按 notes/environment.md 重建。

### 实现定义值登记（累计口径，本轮修订处加粗）
| 值 | 取值 | 位置 |
|---|---|---|
| leech 阈值 | up ≥ 1 MiB 且 down < 64 KiB 且存活 ≥ 120s | bt/engine.rs LEECH_* 常量 |
| bad_piece 阈值 | 同连接累计 3 次 | BAD_PIECE_BAN_THRESHOLD |
| protocol 阈值 | 未知 id/帧错首次即封；非法区间 request 重复 3 次 | INVALID_REQUEST_BAN_THRESHOLD |
| 限速输入上限 | 32 列（FR-01-06 形态对齐） | routing.rs CAP_LIMIT |
| 限速非法 toast | `⚠ 限速值非法（整数 B/s 或单位字符串，如 512 KiB/s）`（HTTP/BT、下载/上传四处统一） | dialogs.rs / bt_dialog.rs |
| 限速字段占位 | 无占位（空态纯空白） | ui/dialog.rs / ui/bt_dialog.rs |
| **限速单位口径（config 文件键）** | **二进制：1 KiB = 1024 B/s、1 MiB = 1024²、1 GiB = 1024³（本轮新规范）；旧写法 KB/MB/GB 按十进制（10^n）兼容解析不回改；裸整数 = B/s** | config.rs SPEED_UNITS / parse_speed |
| 限速单位口径（对话框） | 二进制：1 KiB = 1024 B/s、1 MiB = 1024²、1 GiB = 1024³ | routing.rs speed_unit_mult / dialogs.rs parse_speed_limit |
| **限速输入规范化** | 输入/粘贴即规范化显示（KiB/s、MiB/s、GiB/s、B/s；前导零剥离；canonical 完成态吸收单位后缀续入；退格不回写）；**裸数字不缀单位（输入态保持原样，确认按 B/s 解析）** | routing.rs canonical_speed_form / insert_limit |
| **限速预填/回显显示** | **fmt_speed_limit 整除链（GiB→MiB→KiB→B，非整倍数显裸 B/s，0 显「0」）；fmt→parse 恒等；5 预填点 = Add/Modify HTTP + AddBt/ModifyBt + cmd open_add_dialog_with** | dialogs.rs fmt_speed_limit |
| **修改窗口清空限速确认** | **回填 config.toml 任务级默认（task_max_download_speed / task_max_upload_speed），HTTP 修改无上传字段；添加窗口空输入回退同值（down_prefill）** | dialogs.rs dlg_confirm_modify / bt_dialog.rs dlg_confirm_modify_bt |
| **BT sidecar partial 持久化** | **BtSidecar 增可选 partial_pieces（serde default + skip_if_empty，版本恒 1 不 bump）；块数据 b64（decode 失败逐块丢弃）；Pause/StopSeed 全量保存；启动恢复验证链 = 覆盖内 ∧ 非 done ∧ 16 KiB 对齐 ∧ 单调不重叠 ∧ 界内；完整恢复 piece 组装校验通过直接落盘转 done；校验失败/写盘失败丢弃重下（自愈）** | model/bt.rs BtPartialPiece/BtPartialBlock + bt/engine.rs restore_sidecar_partial / save_bt_sidecar_full |
| **endgame 已收块豁免** | **endgame 臂跳过 partial 已收块（endgame = 找回丢失请求，已收块重发 = 重复下载特征）** | engine.rs request_blocks_of_piece endgame 分支 |
| **trackers 弹窗几何** | **94×22（bug24：90→94）**；内框 92×20 + 右缘滚动条列；内容区左右 padding 对称各 2 列；末列「消息」起点 76、预算 = content_w 88 − 76 = **12 列**，「下轮 / 重试 NNNNs」3–6 位秒数完整显示 | ui/popups.rs TRACKERS_W / TRACKERS_COL_MSG |
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
- **bug24 轮（trackers 倒计时列截断）**：根因 = TRACKERS_W 90 → content_w 84 →
  末列预算 = 84 − 76 = 8 列 < 「下轮 120s」9 列 → truncate 加 '…'；修复 =
  TRACKERS_W 90→94（content_w 88、末列预算 12 列，容 6 位秒数）；新测试
  `trackers_popup_countdown_fits_multi_digit_seconds`（120s/3600s 双用例，抗秒级
  漂移断言：'下轮 '/'重试 ' 后数字段 ≥3 位 + 's' 收尾，不钉具体数值——首版钉
  `下轮 120s` 被测试进程启动耗时 ≥1s 的秒级漂移打破，教训已记 notes）；既有几何
  测试 90×22 → 94×22 对齐。ezr-demo：TRACKERS_W 同步 94；README 变更记录 36；
  UI 规范 4 处（头部追轮块 / §6.12 标题 / 尺寸 bullet / 几何汇总行）。
- **修复①（config 单位口径）**：`model/config.rs` SPEED_UNITS 扩为 7 单元
  （gib/mib/kib 二进制 1024^n + gb/mb/kb 十进制 10^n 兼容 + b；**后缀最长优先排列
  修掉旧表「512 MiB/s 被 b 后缀截断解析为 0」的真缺陷**）；配置模板四键注释
  改「"2 MiB/s" / "500 KiB/s" / 整数 B/s」——操作者复核反馈：模板注释只列取值
  形态，不写换算/兼容性说明文字（已去掉「二进制口径 1 MiB = …；旧写法 … 兼容解析」
  括注）；CLI --max-speed 帮助文案同步 MiB 示例；既有
  十进制断言全部保留为兼容锚。
- **修复②（清空回填）**：`dlg_confirm_modify`（HTTP）与 `dlg_confirm_modify_bt`
  空输入臂由「保留任务当前值」改为「回填 self.cfg.task_max_download_speed /
  task_max_upload_speed」；添加窗口空输入本就回退 down_prefill（= config 默认）
  不变——两窗口「空输入」语义统一为跟随配置默认。
- **修复③（纯数字 B/s + 规范化显示）**：新 `fmt_speed_limit`（GiB/MiB/KiB/B
  整除链）应用于全部 5 个限速预填点（Add/Modify HTTP、AddBt/ModifyBt、cmd
  open_add_dialog_with），修改窗口对 512 B/s 任务显示 `512 B/s`（操作者验收判据，
  有专项测试）；裸数字解析恒 B/s（parse_speed/parse_speed_limit 均然，测试钉死）；
  fmt→parse 恒等（round-trip 测试覆盖 0..u64::MAX 样本）。
- **修复④（BT 反吸血）**：排查结论 = Pause 终结引擎丢 partial（已收未验块）→
  Resume 向同一 peer（BitComet）重请求完全相同 (index, begin, length) 区间，
  构成对端可观测的「同区间重复下载」——反吸血规则触发特征。根治三件：
  ①`model/bt.rs` BtSidecar 增可选 partial_pieces（版本恒 1、serde default，
  前后向兼容）+ BtPartialPiece/BtPartialBlock（b64 String，decode 失败逐块丢弃
  不拖垮整档）；②`engine.rs` save_bt_sidecar_full（Pause/StopSeed 全量保存）+
  restore_sidecar_partial（启动恢复：对齐/界内/单调不重叠/非 done/覆盖内验证链；
  完整恢复 piece 组装校验通过直接落盘转 done；失败丢弃自愈）——「BT partial 进度
  不持久化」沿袭受限项一并销项；③endgame 分支跳过 partial 已收块（endgame =
  找回丢失请求；不加豁免时恢复后的 endgame 窗口照样重发恢复块，实测复现后定位）。
  另因子化 assemble_contiguous / write_piece_to_files 供到达路径与恢复路径复用
  （行为不变）。
- **新增测试（12 例，防回归）**：config speed_parsing_binary_units + 属性测试
  KiB/MiB/GiB 扩展；dialogs modify_confirm_empty_limit_backfills_config_default、
  fmt_speed_limit_canonical_units、fmt_speed_limit_round_trips_through_parse、
  modify_window_shows_bare_number_as_b_per_s（512 → 显示 `512 B/s` 验收）；bt_dialog
  confirm_modify_bt_empty_limits_backfill_config_defaults；model/bt
  partial_blocks_roundtrip_base64_and_ordering、legacy_sidecar_without_partial…；
  engine save_bt_sidecar_full_persists_done_and_partial_blocks、
  restore_sidecar_partial_writes_verified_and_filters_invalid、
  restore_sidecar_partial_keeps_incomplete_for_resume、
  pause_persists_partial_blocks_and_resume_skips_restored_requests
  （假 peer 请求日志：phase1 只服务 piece0 块0 → Pause → sidecar 含该块字节保真；
  phase2 全服务 peer 恢复 → 完成且 **(0,0) 零重请求** + 缺失块正常请求）。
- **ezr-demo 同步（操作者指令：修改同步给 ezr-demo 文档并改写 demo 代码）**：
  ①`src/app/dialogs.rs` dlg_confirm_modify / dlg_confirm_modify_bt 空输入臂 →
  TASK_LIMIT_DEFAULTS（demo 的 config 等价物）——demo 修改窗口与产品同款缺陷，
  本轮修复；②limit_backfill_tests 4 例守卫（裸数字 B/s / fmt↔parse 恒等 / HTTP
  修改清空回填 + 预填规范化断言 / BT 修改双字段清空回填）；③README 变更记录 35；
  ④规范文档 §6.1 mock 注释 + 新增「限速确认语义（bug23）」bullet + §6.2 限速确认
  语义 bullet。demo 的 parse/format 素来与目标口径一致（纯数字 B/s、规范化显示），
  只补守卫防漂移。BT 引擎反吸血为产品侧修复，demo 为模拟数据无 BT 引擎，文档注明
  不涉及。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| bin 全量单测 | **904P / 0F**（bug23 终态 903 + 净增 1；首轮 1 例时序 flake 复跑即绿） | `cargo test --bin ezr` |
| 集成 12 目标 | **6044P / 0F**（含 ui 模块的 engine_backfill/ui_backfill/v115 三套件各 +1 收编新测试；v147 首轮 1 例负载时序失败复跑即绿 = 既有登记口径） | `cargo test --test <目标>`（hardening 140 / engine_backfill 807 / g_appcore 707 / g_engine 702 / g_main 2 / g_proxy 16 / g_slots 200 / g_supervisor 676 / g_throttle_err 437 / g_v147 690 / ui_backfill 837 / v115 830） |
| ezr-demo 全量 | **35P / 0F** | `cargo test`（ezr-demo/） |
| clippy | 0 警告（--bin 与 --tests） | `cargo clippy --bin ezr --tests` |
| fmt | ezr 干净；demo 存量漂移 5 文件沿袭未触碰 | `cargo fmt --check` |

### 对账口径
- 基线 = bug23 轮登记 bin 903P/0F + 集成 6041P/0F + demo 35P/0F = 6979P/0F；本轮
  终态 bin 904 + 集成 6044 + demo 35 = **6983P/0F**（净增 +4 = 新测试 1 例在 bin
  与收编 ui 模块的 3 个集成套件各计 1 次；对账以「全绿 + 逐套件基线吻合」为准）。
- 本轮无规格/QA 套件改动（coder 直启轮）；bug23 交互语义变更（清空回填/单位口径）
  与 bug24 几何变更均待 specifier 轮落 spec 三层后，QA runner 同步评估（沿袭待办）。
- 操作者验收判据对照：①bug24：trackers 窗口末列 3 位倒计时完整显示（无 '…'）✓
  （专项测试，抗漂移断言）；②bug23：config 模板出现 KiB/s·MiB/s 示例 ✓；③修改窗口
  清空限速确认后值 = config.toml 对应键 ✓（测试）；④输入 512 确认后修改窗口显示
  `512 B/s` ✓（专项测试 modify_window_shows_bare_number_as_b_per_s）；⑤暂停→恢复
  不再向同一 peer 重发已收块 ✓（wire 级请求日志测试）。

### 待办与待批
- **待批（操作者）**：无新增待批项。
- **待办（specifier 轮）**：bug23 新登记①–④判据 + bug24「trackers 弹窗几何/末列
  预算」判据落 spec 三层（features 场景增补 + QA 用例）。
- **待办（QA 轮）**：QA runner 执行层对本轮交互语义（限速确认/单位口径）改动的
  同步评估（沿袭开放）。
- **待办（cleaner 轮，沿袭）**：`App.windows` 失去读者裁决；handle_peer_msg 参数
  收拢；bt/engine.rs 拆分评估；`up_prefill` 在 HTTP 路径已无读者（字段面 BT 专用化
  归并评估）；`canonical_speed_form` 与 `parse_speed_limit` 的解析前缀重复归并评估
  （本轮新增 fmt_speed_limit 与 parse_speed_limit 的整除链亦同族，可一并评估）。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/specifier`（bug23 四项判据归档）或
  `bin/swarm begin six-pack/cleaner`（沿袭清理）：以本文件为上游交接。
- 后续链：specifier → coder / cleaner → architect → hardender → QA（宪法第一章：
  不自动流转）。
- 操作者授权的收尾口径：`bin/swarm complete` 后立即将产物 tar.gz 复制到
  `/home/z/my-project/`（沿 bug18/20/21/22 轮授权口径执行）。
- 工具链交接：rustup nightly 1.101.0（PATH = ~/.cargo/bin）+ cargo registry rsproxy；
  环境不跨会话持久时按 notes/environment.md 重建（本轮实测 rsproxy 直链 + 双变量
  一次装齐）。
