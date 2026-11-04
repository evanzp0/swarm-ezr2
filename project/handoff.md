# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（本轮已完成）→ 建议下一节点 six-pack/cleaner
> （沿袭清理）或 six-pack/architect
> 会话：ezr-coder-config-retire（2026-11-04，coder 轮：config 键旧写法 KB/MB/GB 十进制
> 废弃实现对齐——SPEED_UNITS 7→4 + 兼容测试翻转 + CLI 报错路径核对；操作者 chat 追加
> 指令「修改涉及 UI/交互的内容同步 ezr-demo 文档并改写 ezr-demo 代码」——config 键
> 同口径核查 + 规范文档分层口径注记 + demo 守卫测试）
> 上游交接：project/handoff.md（bug23/24 轮 specifier 归档交接）

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式 v1.40）+ `project/mission/phase-01…04.md`。
- phase-01（HTTP/HTTPS 内核与 TUI）收官；phase-02（BT 内核/做种/管理）已实现并经
  互操作夹具多轮收口。历轮 bug11–24 全部落地。
- **本轮（coder，config 键废弃）**：FR-02-28/29 v1.11 规格先行偏差销项——SPEED_UNITS
  移除 gb/mb/kb 三单元、兼容测试全量翻转（含 proptest）、CLI --max-speed 旧写法报错
  路径核对（单测 + 实机）；ezr-demo config 键解析同口径核查通过 + UI/交互修改同步
  （规范文档 + README + 守卫测试）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| **coder（本轮，config 键废弃）** | `project/ezr/`：config.rs SPEED_UNITS 7→4 + 测试翻转 3 处 + 新增废弃断言 2 例；main.rs CLI 测试翻转 + 旧写法报错断言；property_tests.rs 两 proptest 翻转。`ezr-demo/`：dialogs.rs 守卫测试 1 例；docs 规范文档头注 + §6.1 分层 bullet；README 变更记录 37 | 已交付 | bins 952P/0F（ezr 906 + fixture 30 + mktorrent 1 + proxy 15）+ 集成 5232P/0F（11 套件分项见验证证据）+ demo 36P/0F + clippy 双 0 警告 + fmt 双干净 + CLI 实机报错验证 |
| specifier（bug23/24 归档轮） | 需求层 mission.md v1.40 + phase-02 v1.11；规格层 features 八文件（throttle-proxy v1.11 等）；QA 层八套件同步 | 已交付 | gherkin-parser 8/8 + dry-checker 8/8（建议级 findings）+ 判据①–⑤交叉核对全过 |
| coder（bug24） | ui/popups.rs TRACKERS_W 90→94 + 末列预算 12 + 倒计时抗漂移测试；ezr-demo 同步 | 已交付 | bin 904P/0F + demo 35P/0F + clippy/fmt 干净 |
| coder（bug23） | config.rs 单位表/模板四键/CLI 帮助；dialogs.rs fmt_speed_limit + 预填 + 清空回填；bt_dialog.rs；cmd.rs；bt.rs sidecar partial；engine.rs 反吸血；ezr-demo 同步 | 已交付 | bin 903P/0F + demo 35P/0F + clippy/fmt 干净 |
| specifier（phase-01/02 及历轮 bug11–22） | features/ 20 份 + qa/ 对应套件 | 已收官/已交付 | 见历轮登记 |

### 遗留受限项（当前有效，沿袭 + 本轮修订）
- **本轮销项**：上轮新登记的「config 键旧写法废弃规格先行偏差」——实现已对齐
  （SPEED_UNITS 4 单元 gib/mib/kib/b；旧写法值 = 非法回退 0；CLI 旧写法参数报错退出
  exit 2，文案 `ezr: --max-speed 速度非法（{v}）：示例 2 MiB/s / 500 KiB/s`）。
- 沿袭（bug14–24 轮登记，除注明外全部不变）：cargo-mutants 缺位；死 tracker 约 4 分钟
  静默窗；TUI 恐慌自愈边界（bug6）；QA-SQ-13 竞态沿袭待办；QA-RS-06 沿袭；supervisor
  block_worker fatal 竞态观察项；`App.windows` 失去读者（cleaner 候选裁决）；leech 判据
  无 e2e 自动化（阈值常量固定）；bad_piece 归属口径 = 交付完成块 peer 计 1 次（D67
  允许）；启动重校验窗内二次暂停丢上一轮 partial（D68⑥ 规格允许边界，非受限项）；
  v147 `engine_conns_quota_exact_and_unique_ids` 负载敏感 flake（连续两轮四次 full-run
  FAIL / 单跑五连绿 0.3s 级，环境性非回归，notes/testing.md 实录）。
- 工具链：bb v1.12.209（`.tools/bin/bb`）+ APS 仓库（`.tools/Acceptance-Pipeline-
  Specification`）**本会话未安装**（本轮纯实现轮，无规格解析需求；需要时按
  notes/environment.md 重建，gh-proxy 前缀）；rustup nightly 1.101.0（USTC 13.6MB/s
  前台装齐，PATH = ~/.cargo/bin）+ cargo registry USTC sparse；`.tools/` 不跨会话保留。

### 实现定义值登记（累计口径，本轮修订处加粗）
| 值 | 取值 | 位置 |
|---|---|---|
| leech 阈值 | up ≥ 1 MiB 且 down < 64 KiB 且存活 ≥ 120s | bt/engine.rs LEECH_* 常量 |
| bad_piece 阈值 | 同连接累计 3 次 | BAD_PIECE_BAN_THRESHOLD |
| protocol 阈值 | 未知 id/帧错首次即封；非法区间 request 重复 3 次 | INVALID_REQUEST_BAN_THRESHOLD |
| 限速输入上限 | 32 列（FR-01-06 形态对齐） | routing.rs CAP_LIMIT |
| 限速非法 toast | `⚠ 限速值非法（整数 B/s 或单位字符串，如 512 KiB/s）`（HTTP/BT、下载/上传四处统一） | dialogs.rs / bt_dialog.rs |
| 限速字段占位 | 无占位（空态纯空白） | ui/dialog.rs / ui/bt_dialog.rs |
| **限速单位口径（config 文件键，已实现）** | **新写法 KiB/MiB/GiB 二进制（1024^n，大小写不敏感，可带 /s 后缀）+ 裸整数与 B/s 后缀 = B/s；旧写法 KB/MB/GB 十进制废弃不再识别（含旧写法的值 = 非法 → 回退默认 0，FR-01-71）——SPEED_UNITS 4 单元 gib/mib/kib/b，本轮已对齐** | config.rs SPEED_UNITS / parse_speed |
| **CLI --max-speed 旧写法（已实现）** | **同源 parse_speed：旧写法参数 = 启动报错退出（exit 2）** | main.rs opt_speed |
| 限速单位口径（对话框） | 二进制：1 KiB = 1024 B/s、1 MiB = 1024²、1 GiB = 1024³；输入速记 k/kb/kib 等同映射二进制（kb → 1024，与 config 键层废弃口径不同面属有意设计） | routing.rs speed_unit_mult / dialogs.rs parse_speed_limit |
| 限速输入规范化 | 输入/粘贴即规范化显示（KiB/s、MiB/s、GiB/s、B/s；前导零剥离；canonical 完成态吸收单位后缀续入；退格不回写）；裸数字不缀单位（输入态保持原样，确认按 B/s 解析） | routing.rs canonical_speed_form / insert_limit |
| 限速预填/回显显示 | fmt_speed_limit 整除链（GiB→MiB→KiB→B，非 1024 整倍数显裸 `N B/s`，0 显「0」）；fmt→parse 恒等；5 预填点 = Add/Modify HTTP + AddBt/ModifyBt + cmd 分流添加对话框 | dialogs.rs fmt_speed_limit |
| 修改窗口清空限速确认（D67②） | 回填 config.toml 任务级默认（task_max_download_speed / task_max_upload_speed），HTTP 修改仅下载字段、BT 框双字段各自回填；添加窗口空输入回退同值（down_prefill） | dialogs.rs dlg_confirm_modify / bt_dialog.rs dlg_confirm_modify_bt |
| BT sidecar partial 持久化（D68） | BtSidecar 可选 partial_pieces（serde default + skip_if_empty，版本恒 1 不 bump）；块数据 b64（decode 失败逐块丢弃）；Pause/StopSeed 全量保存；启动恢复验证链 = 覆盖内 ∧ 非 done ∧ 16 KiB 对齐 ∧ 单调不重叠 ∧ 界内；完整恢复 piece 组装校验通过直接落盘转 done；校验失败/写盘失败丢弃重下（自愈）；允许边界（D68⑥）= 启动重校验窗内二次暂停丢上一轮 partial | model/bt.rs + bt/engine.rs |
| endgame 已收块豁免（D68⑤） | endgame 臂跳过 partial 已收块 | engine.rs request_blocks_of_piece endgame 分支 |
| trackers 弹窗几何（D67⑤） | 94×22；内框 92×20 + 右缘滚动条列；内容区左右 padding 对称各 2 列；末列「消息」起点 76、预算 12 列，「下轮 / 重试 NNNNs」3–6 位秒数完整显示 | ui/popups.rs TRACKERS_W / TRACKERS_COL_MSG |
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
- **产品（project/ezr/）**：
  - `src/model/config.rs`：SPEED_UNITS 移除 gb/mb/kb 三单元（7→4，gib/mib/kib/b）；
    `speed_parsing` 移除旧写法正断言；新增 `speed_parsing_decimal_units_retired`
    （2 MB/s、500 KB/s、1.5 MB、2 gb → 0）；`speed_parsing_binary_units` 末断言翻转
    （2 MB/s → 0）；`speed_limit_keys_accept_string_and_integer` 改二进制新写法
    （2 MiB/s → 2_097_152、500 KiB/s → 512_000）；新增
    `speed_limit_keys_decimal_old_style_fall_back_zero`（2 MB/s、500 KB/s、2 GB 键值 → 0）。
  - `src/main.rs`：`parse_cli_all_flags` 改 `--max-speed 2 MiB/s` → 2_097_152；
    `parse_cli_max_speed_zero_and_invalid` 增旧写法报错断言（2 MB/s、500 KB/s → is_err）。
  - `src/property_tests.rs`：`prop_parse_speed_suffix_and_units` 单位枚举表去 KB/MB、
    KB/MB 断言改归零；`prop_parse_speed_normalization_and_zero` 改 MiB/kib 口径 +
    kb 无空格后缀归零断言 + 单调 MiB > KiB > b。
- **ezr-demo/**：
  - `src/app/dialogs.rs`：limit_backfill_tests 新增
    `dialog_input_shorthand_decimal_spelling_maps_to_binary`（速记面 512 kb/s → 524_288、
    2 MB/s → 2_097_152、1.5 gb → 1_610_612_736——钉住对话框层与 config 键层的口径分层）。
  - `docs/ezr-demo UI界面及交互设计规范.md`：头部新增「config 键限速单位废弃轮」注记；
    §6.1 新增「限速单位口径分层」bullet（config 键层二进制新写法/旧写法废弃 vs 对话框层
    速记二进制补全，两层有意不同；demo 无 config 解析面、TASK_LIMIT_DEFAULTS 为演示
    等价物；守卫测试名登记）。
  - `README.md`：变更记录新增第 37 条（产品侧废弃实现概要 + demo 核查结论 + 分层口径 +
    守卫测试 + 文档同步点）。
- **project/notes/**：testing.md（v147 flake 判法稳定直引 + 废弃类口径变更测试翻转面
  三件套）；environment.md（cargo build 同受小盘约束 / begin 清空 .work/tmp/ /
  2026-11-04 镜像复测 USTC 唯一可用）；INDEX.md 两行同步。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 产品 bins | 952P/0F（ezr 906 + fixture 30 + mktorrent 1 + proxy 15） | `cargo test --bins`（project/ezr，CARGO_INCREMENTAL=0） |
| 产品集成 11 套件 | 5232P/0F：hardening 142 + hardening_engine_backfill 809 + hardening_g_appcore 709 + hardening_g_engine 704 + hardening_g_main 2 + hardening_g_proxy 16 + hardening_g_slots 202 + hardening_g_supervisor 678 + hardening_g_throttle_err 439 + hardening_g_v147 691 + hardening_ui_backfill 839（v147 另有 quota 用例 full-run 偶发、单跑绿 ×3，环境性，见遗留受限项） | `cargo test --test <名>`（逐套件） |
| ezr-demo | 36P/0F | `cargo test`（ezr-demo，CARGO_INCREMENTAL=0） |
| clippy | 产品 + demo 双 0 警告 | `cargo clippy --bins --tests` / `cargo clippy --all-targets` |
| fmt | 产品 + demo 双干净 | `cargo fmt --check` |
| CLI 旧写法实机 | `--max-speed "2 MB/s"` / `"500 KB/s"` → stderr `ezr: --max-speed 速度非法（…）：示例 2 MiB/s / 500 KiB/s`、exit 2 | `./target/debug/ezr --max-speed "2 MB/s" http://example.com/f.bin` |

### 对账口径
- 上轮基线 bug24 登记「bin 904P + 集成 6044P + demo 35P」；本轮实测 bins 952（+48 =
  ezr +2 新增废弃断言测试、extra bins 46 与上轮同口径）+ 集成 5232 + demo 36（+1 守卫）。
  **集成实测 5232 与上轮登记 6044 存在 -812 差异**：本轮 11 套件逐一实跑、分项可复算
  （见验证证据），未发现缺套件或漏跑；按 engineering.md「以实测为准并记录偏差」登记，
  上轮口径疑为不同计数方式，下游如需对账以本轮分项表为基线。
- 规格与实现对齐：FR-02-28/29 v1.11 判据逐条落地（config 键二进制新写法生效行 +
  旧写法废弃回退行含 `500KB/s` 无空格形态；CLI 旧写法报错退出；场景 09 预填链
  fmt(parse_speed(值)) 对旧写法值显示 "0"）。
- ezr-demo 同口径核查结论：demo 全源码无 toml/config 解析面（rg 零命中），
  `TASK_LIMIT_DEFAULTS` 数值常量即产品 config 任务级默认的演示等价物，解析代码无需
  改动；对话框层 `speed_unit_mult` 与产品 routing.rs 同表同构（速记 kb/mb/gb → 二进制），
  守卫测试钉住防与 config 废弃口径混淆。

### 待办与待批
- **待办（QA 轮，沿袭扩大）**：QA runner 执行层同步评估（TP-04/05 废弃行、TP-09/
  01-20/AB-10 fmt 预填、MT-10/MB-08 清空回填与回显、BE-17 新用例、TW-02/03 几何、
  CT-01 40 键锚点）——本轮新增：config 键/CLI 旧写法废弃的执行层语义翻转
  （旧写法写入 → 回退 0 / CLI 报错退出）在 runner 侧如有旧写法 fixture 需同翻。
- **待办（cleaner 轮，沿袭）**：`App.windows` 失去读者裁决；handle_peer_msg 参数收拢；
  bt/engine.rs 拆分评估；`up_prefill` 归并评估；`canonical_speed_form` 与
  `parse_speed_limit` 解析前缀重复归并评估（含 fmt_speed_limit 整除链同族）。
- 无阻塞；无需操作者新批准项（本轮纯规格既定偏差对齐 + 操作者 chat 指令执行）。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`（沿袭清理）或 `bin/swarm begin
  six-pack/architect`：以本文件为上游交接。
- 后续链：specifier → coder / cleaner → architect → hardender → QA（宪法第一章：
  不自动流转）。
- 操作者授权的收尾口径：`bin/swarm complete` 后立即将产物 tar.gz 复制到
  `/home/z/my-project/`（沿 bug18/20/21/22/23/24 轮授权口径执行）。
- 工具链交接：rustup nightly 1.101.0（USTC 镜像前台装齐，PATH = ~/.cargo/bin）+
  cargo registry USTC sparse；bb + APS 本会话未装（需要时按 notes/environment.md
  重建）。
