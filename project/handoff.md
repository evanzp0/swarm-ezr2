# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（本轮已完成）→ 建议下一节点 six-pack/cleaner
> 会话：ezr-coder-bug18（2026-11-02，五项新功能实现轮：FR-02-28 全局上传/下载限速、
> FR-02-29 任务级上传/下载限速、FR-02-30 BT 自动反吸血 + blacklist.yaml、
> FR-02-31 't' trackers 弹窗、FR-02-32 [bt] listen_port；UI/交互以 ezr-demo 为同构参照）
> 上游交接：project/handoff.md（bug20 specifier 轮交接：规格 v1.9/v1.9.1/v1.9.2 全量
> 修订 + D67 设计定义值 + ezr-demo 参照同步完成，含「complete 后归档复制
> /home/z/my-project/」授权）

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式 v1.38）+ `project/mission/phase-01…04.md`。
- phase-01（HTTP/HTTPS 内核与 TUI）已收官；phase-02（BT 内核/做种/管理）已实现并经
  互操作夹具多轮收口。历轮 bug11–17 全部落地；bug18 规格轮（specifier，三期追轮
  v1.9/v1.9.1/v1.9.2）完成五项新功能规格化；本轮 coder 实现 bug18 五项功能全量落地。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01/02 及历轮 bug11–17） | features/ 20 份 + qa/ 对应套件 | 已收官/已交付 | 见历轮登记 |
| coder（phase-02 及历次收口轮，含 bug15/16/17 落地） | `project/ezr/` 全量 | 已交付 | 见历轮登记（bug17 轮：bin 864P/0F + 集成 5742P/0F） |
| specifier（bug18 三期追轮） | mission/phase-02 v1.9…v1.9.2 + features ×11 修订/新建 + qa ×7 + ezr-demo 同步（代码/脚本/文档） | 已交付 | APS dry-check 全 EXIT=0；ezr-demo cargo check + PTY 26 项全过 |
| **coder（本轮 bug18 实现轮）** | `project/ezr/` 全量五项功能（清单见下节） | **已交付** | **全量 6791P/0F（bin 885P/0F + 集成 12 目标 5906P/0F）+ clippy 0 警告 + fmt 干净** |

### 遗留受限项（当前有效，沿袭 + 本轮登记）
- 沿袭（bug14–17 轮登记，全部不变）：cargo-mutants 缺位；死 tracker 约 4 分钟静默窗；
  TUI 恐慌自愈边界（bug6）；BT partial 进度不持久化；QA-SQ-13 竞态沿袭待办；QA-RS-06
  沿袭；supervisor block_worker fatal 竞态观察项；`App.windows` 失去读者（cleaner 候选
  裁决）；ezr-demo verify_ezr.py 场景 N 四项预存假 FAIL；ezr-demo 11 条预存 nightly
  lint（未触碰文件）。
- **沿袭解除两项**：①「BT 下载限速未接」（bug14 起登记）——本轮 BtSpec.max_speed
  dead_code 退役、任务级双字段真实接入引擎（下载 pump 侧 + 上传 spawn 侧）；②「BT 引擎
  无并发热调命令」之外新增热调载体——限速经 Reconfigure 载体热调生效（不重启引擎）。
- **本轮新受限项**：①leech 判据（120s 存活窗 + 1MiB 上传阈值）无 e2e 自动化测试——
  阈值常量固定（LEECH_UP_MIN/LEECH_DOWN_MAX/LEECH_MIN_ALIVE_SECS），e2e 需真实传输
  ≥1MiB + 120s 等待，测试成本不可接受；判据逻辑与另两判据共享 ban_and_drop 原语，
  bad_piece/protocol 两判据已有 e2e 覆盖；②QA 套件 runner 化与实跑（含 QA-AL/QA-TW
  新套件）属 six-pack/QA 职责——本轮交互变更（t/T 键、四对话框 +2 字段、trackers
  弹窗）对既有 QA runner 的影响未评估未同步（沿 coder 职责边界，QA 轮处理）；
  ③bad_piece 归属口径 = 交付完成块的 peer 计 1 次（piece 可能多 peer 拼块，
  实现定义近似，D67 允许）。
- Inversion 访谈受限项（沿袭）：chat 选择题面板不可用，操作者指令文本即阶段 1 答案。

### 实现定义值登记（本轮新增，累计口径见 mission/phase-02 D67）
| 值 | 取值 | 位置 |
|---|---|---|
| leech 阈值 | up ≥ 1 MiB 且 down < 64 KiB 且存活 ≥ 120s | bt/engine.rs LEECH_* 常量 |
| bad_piece 阈值 | 同连接累计 3 次 | BAD_PIECE_BAN_THRESHOLD |
| protocol 阈值 | 未知 id/帧错首次即封；非法区间 request 重复 3 次 | INVALID_REQUEST_BAN_THRESHOLD |
| 限速输入上限 | 32 列（FR-01-06 形态对齐） | dialog_keys CAP_LIMIT |
| 限速非法 toast | `⚠ 限速值非法（整数 B/s 或十进制单位字符串，如 2 MB/s）` | dialogs.rs / bt_dialog.rs |
| 限速字段占位 | `0` | ui/dialog.rs / ui/bt_dialog.rs |
| 监听占用警告 | `BT 监听端口 {port} 被占用，已回退随机端口` | engine/mod.rs bind_bt_listener |
| 黑名单损坏警告 | `黑名单文件无法解析（{path}），已按空名单启动` | model/blacklist.rs |
| banned_at 格式 | unix 秒（u64） | model/blacklist.rs BlacklistEntry |
| trackers 空态文案 | `（无 trackers）` | ui/popups.rs |

## 二、当前产出情况

### 本会话产物清单（全部在 `project/ezr/`）
- **FR-02-28 全局限速**：`model/config.rs`（max_speed 键退役 → global_max_download_speed/
  global_max_upload_speed，旧键按未知键静默忽略；模板 v1.14 钦定原文逐字节内嵌 191 行
  48 空行 40 键 + 模板测试组全量对齐）；`engine/throttle.rs`（Throttle::set_rate 热调）；
  `engine/mod.rs`（EngineShared 双桶 throttle_dl/throttle_ul + 访问器 + 启动构造）；
  `supervisor/workers.rs`、`supervisor/single.rs`（HTTP 每 chunk 全局桶 acquire）；
  `main.rs`（CLI --max-speed 语义 = 全局下载限速，参数优先不变）。
- **FR-02-29 任务级限速**：`model/task.rs` + `model/registry.rs`（Task/TaskSnapshot
  max_download_speed/max_upload_speed，serde default 存量兼容）；`supervisor/mod.rs`
  （TaskSpec.task_max_download_speed + BtSpec 双字段替代 dead max_speed）；TaskCmd/
  BtTaskCmd/Cmd 三层 Reconfigure 载体扩展 + download.rs watch 通道热调 + bt/engine
  set_rate；`app/dialogs.rs`、`app/bt_dialog.rs`（四框 +2 字段：添加框预填 config 默认、
  留空/非法回退；修改框预填当前值、留空保持、非法 toast + 聚焦不关窗；parse_speed_limit
  解析整数 B/s / 十进制单位）；`app/dialog_keys/{mod,routing}.rs`、`app/paste.rs`、
  `app/mouse.rs`（11/8/11/8 焦点矩阵七处登记点全量同步 + 限速粘贴白名单 32 列）；
  `ui/dialog.rs`（74×14/74×12 + 动态 label 对齐）；`ui/bt_dialog.rs`（78×28/78×26）。
- **FR-02-30 反吸血**：`model/blacklist.rs` 新建（BlacklistEntry ip/reason/banned_at，
  serde_yaml 持久化，损坏=空名单+警告；record 原子写、同 IP 去重；Cargo.toml + serde_yaml
  0.9）；`bt/engine.rs`（ConnStat +connected_at/bad_pieces/invalid_requests；leech 判据
  1s tick 臂 + 做种豁免；bad_piece 计数封禁；Unknown 消息 id 首封 + PeerIn::ProtocolViolation
  帧错分类 + 非法区间 request 3 次封禁；ban_and_drop 统一原语：黑名单 + yaml 落盘 + ⛔
  toast + 优雅关闭 + 候选摘除；三面过滤沿用 is_blacklisted）；`engine/mod.rs`
  （blacklist_add_with + blacklist_file 门 + 启动加载 seed + 损坏警告）。
- **FR-02-31 trackers 弹窗**：`bt/engine.rs`（TrackerReport 带 url/interval_secs/
  retry_secs；spawn_tracker 按池内 URL 回报）；`engine/mod.rs`（Evt::BtTrackers）；
  `app/engine/evt.rs`（BtTrackers 臂 → TrackerRow 存储）；`app/mod.rs`（TrackerRow +
  trackers_* 八字段状态组 + overlay_open 扩展）；`app/popups.rs` 新建（门控三臂 toast
  文案按规格 + 开关/滚动）；`ui/popups.rs` 新建（draw_trackers 90×22 qBt 六列、URL 44 列
  宽折行、细线分隔计入滚动、对称 padding 2、无底部提示行、VS15+微差色反自动链接、
  三态真实数据源、滚动条 SbKey::Trackers、拖拽/[✗]）；`app/scrollbar.rs`
  （SbKey::Trackers 五件套）；`app/keys.rs`（弹窗键臂 + t/T 臂改弹窗入口，页签切换
  收敛 ]/[）；`app/mouse.rs`（滚轮 ±2/滚动条/拖拽/[✗]）；`ui/multiline.rs`
  （draw_multi_line_input + no_link 参数，渲染层注入不改缓冲，命令窗口 false/BT
  trackers 面板 true）；`app/bt_dialog.rs`（trackers 确认链 scheme 白名单
  http/https/udp 拒绝：toast + 焦点回 trackers + 不落库）；`ui/help.rs`
  （12 组 75 行：页签 T 删、任务列表 T 增、trackers 弹窗组新增按 feature 置尾）。
- **FR-02-32 listen_port**：`model/config.rs`（[bt] listen_port，0/负/非法 → 随机）；
  `engine/mod.rs`（bind_bt_listener 同步 bind + 占用回退随机 + 启动警告；EngineShared
  std listener + OnceLock<Arc<tokio TcpListener>> 单次 from_std）；`bt/engine.rs`
  （删除每任务本地 bind，改 shared.bt_listener()；announce port = 实际监听端口全任务
  共享）。
- **测试新增 +26**：config（speed_limit_keys/legacy_max_speed 退役/listen_port 四分支/
  模板 40 键）；blacklist 5 项（缺文件/round-trip/损坏/非法 IP 跳过/手工解封）；
  throttle set_rate 2 项；parse_speed_limit 2 项；bind_bt_listener 3 项；trackers 弹窗
  渲染 2 项（几何六列折行 + 三态真实数据源/细线计数）；反吸血 e2e 3 项（未知消息 id
  首封/非法 request 三振/坏 piece 三振）+ tracker 状态事件 1 项。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| bin 全量单测 | **885P / 0F**（基线 864 + 21 新增） | `cargo test --bin ezr` |
| 集成 12 目标 | **5906P / 0F**（基线 5742 + 164，挂载套件自动收编本轮 src 新测试） | `cargo test --test hardening…`（12 目标分块跑） |
| 全量合计 | **6791P / 0F**（基线 6606 + 185） | 上述之和 |
| clippy | 0 警告（--bin 与 --tests 均 0） | `cargo clippy --bin ezr --tests` |
| fmt | 干净 | `cargo fmt --check` |
| 时序 flake 复核 | hardening_v115 / hardening_g_v147 各 1 次单点 FAIL，原样复跑全绿（811P/673P）——环境性时序，非回归 | 复跑同命令 |

### 对账口径
- 基线 = bug17 轮登记 bin 864P/0F + 集成 5742P/0F = 6606P/0F（本会话开工首日复现一致，
  hardening_g_engine 首轮 1 次时序 flake 复跑全绿）。本轮终态 6791P/0F = +185
  （bin +21 直增；集成 +164 为挂载套件对 src 新测试的自动收编重复计数，实际新增测试
  同源）。
- 五项功能与 FR 逐项对应：FR-02-28（config 两键改名退役 + 双桶 + HTTP/BT 接入 + CLI
  保留）、FR-02-29（任务字段 + 四对话框 + Reconfigure 热调 + min 合成）、FR-02-30
  （三判据 + yaml + 三面过滤 + B 键共存）、FR-02-31（门控/90×22/qBt 六列/折行/细线/
  padding/无提示/VS15/t 退役/wss 双过滤面/帮助同步）、FR-02-32（listen_port 四分支 +
  共享单端口 + 占用回退警告）。
- bug20 六条 UI 反馈全部纳入 ezr 端弹窗实现（细线/续行同色/VS15/对称 padding/无提示
  行/44 列）+ 对话框 no_link。

### 待办与待批
- **待批（操作者）**：D67 设计定义值（沿袭 specifier 轮，全部已按 D67 实现，若对任一
  裁决有异议可回退）；本轮实现定义值登记表（上表 10 项）。
- **待办（cleaner 轮）**：`App.windows` 失去读者裁决；handle_peer_msg 参数进一步收拢
  （现有 #[allow(clippy::too_many_arguments)]）；bt/engine.rs 体量（5300+ 行）拆分评估；
  ezr-demo `add_bt_dialog_enter`/`modify_bt_dialog_enter` 陈旧注释清理（demo 侧）。
- **待办（QA 轮）**：QA 套件 runner 化与实跑（QA-TP 修订版/QA-AL/QA-TW 新套件 + 既有
  套件对本轮交互变更的同步评估：t/T 键改判、四对话框 74×14/78×28 几何、trackers 弹窗）。
- **待办（hardender 轮沿袭）**：cargo-mutants 缺位待环境补齐。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。coder 侧已知清理候选：
  `App.windows` 死字段裁决、handle_peer_msg 参数结构化、bt/engine.rs 拆分评估（变异点
  密度见 notes/rust.md 每文件计数口径）、`speed_limit_of`/`listen_port_of` 与既有 helper
  的归并评估。
- 后续链：cleaner → architect → hardender → QA（宪法第一章：不自动流转）。
- 操作者授权的收尾口径：`bin/swarm complete` 后立即将产物 tar.gz 复制到
  `/home/z/my-project/`（沿 bug18 主轮授权，本轮按同口径执行）。
- 工具链交接：rustup 已装（TUNA 镜像 nightly 1.101.0 + rustfmt + clippy，PATH =
  ~/.cargo/bin 与 ~/.local/bin——后者为 lld 桥接符号链接 ld.lld → rust-lld，环境不跨
  会话持久时按 notes/rust.md 重建）；serde_yaml 0.9 新依赖已入 Cargo.toml/Cargo.lock。
