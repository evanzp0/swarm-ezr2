# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-p2-coder-bug8（2026-10-29，
> 「BT 下载慢 / 暂停继续进度清零 / 同 IP 两行复发 / 分块窗口并发口径」四 bug 修复轮）
> 上游交接：project/handoff.md（ezr-p2-coder-bug7 前轮交接：速度 EMA 不重置 / 同 peer_id
> 双通道去重 / BT 三集合分块显示 / HTTP blocks 还原，796P/0F）
> 当轮任务：操作者四项指令——①BT 下载速度极慢（同局域网其他客户端 2~3MB/s，ezr 只有
> 50KB/s）；②bt 任务暂停后继续下载，偶尔进度清零重新开始；③并发 panel 下载一段时间后
> 同一 peer 的 IP 仍出现两行（bug7 声称已修，复发）；④分块窗口「并发」显示了任务设置的
> 最高并发数，应显示实际并发连接数（操作者更正版）。
> 修复后用 project/harness 工具完整测试，bin/swarm complete 打包，产物复制
> /home/z/my-project/。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式）+ `project/mission/phase-01…04.md`。
- **phase-01（HTTP/HTTPS 内核与 TUI）已收官**；**phase-02（BT 内核/做种/管理）已实现**
  并经互操作夹具多轮收口，详见既有登记。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01/02） | features/ 01 十份 + 06…09 四份；qa/ 对应十四份 | 已收官/已交付 | 见前轮登记 |
| coder（phase-02 及历次收口轮） | `project/ezr/` 全量 | 已交付 | 前轮 5866P/0F；clippy 0 警告；互操作 e2e 三连 PASS |
| coder（bug5：去重/还原/peer 策略/策略移植/tracker 轮） | 产品改动 ×9 文件 + 回归测试 ×18 | 已交付 | 6030P/0F；clippy 0 警告；fmt 干净 |
| coder（bug6：版本回退/total order panic/花屏轮） | 产品改动 ×3 文件（model/bt.rs + bt/engine.rs + main.rs）；测试净 +1 | 已交付 | 788P/0F；clippy 0 警告；fmt 干净 |
| coder（本轮 bug7：速度闪烁/同 IP 两行/分块还原/连片轮） | 产品改动 ×8 文件（model/speed.rs + model/bt.rs + model/task.rs + model/registry.rs + model/mod.rs + bt/engine.rs + engine/mod.rs + app/{mod.rs, engine/evt.rs, engine/bt.rs}）；新增回归测试 ×8 | 已交付 | 全量 796P/0F；clippy 0 警告；fmt 干净；harness e2e 两连 PASS |
| coder（本轮 bug8：速度/恢复竞态/同 IP 两行复发/并发口径轮） | 产品改动 ×8 文件（bt/{engine.rs, mod.rs, pieces.rs} + app/{mod.rs, tasks.rs} + app/engine/{tick.rs, bt.rs} + ui/pieces.rs）；新增回归测试 ×8 | 已交付 | 全量 804P/0F；clippy 0 警告；fmt 干净；harness e2e 两连 PASS + 256MB 速度基准 ezr≈libtorrent |

### 遗留受限项（当前有效）
- **BT 下载限速未接**（沿袭：BtSpec.max_speed 仅对 HTTP 生效）；DHT/BEP 10 完整扩展/
  uTP 属 phase-03/04。
- **cargo-mutants 缺位**（加固轮职责）；demo 回归长脚本沙箱挂起（沿袭）；doc 测试不适用
  （bin-only）。
- **环境性抖动族**（既有登记项）：本轮全量未复现；处置依据 notes/testing.md 双测试二进
  制并发假 FAIL 判据，继续以「单跑复跑 + 整批复跑」口径处置。
- **死 tracker 的用户可见延迟**（沿袭待批项）：tracker 不可达时任务停留「下载中 0.0%」
  约 4 分钟才报「全部 tracker 失败」；是否加可视状态由操作者裁定。
- **TUI 恐慌自愈的边界**（bug6 登记）：被捕获恐慌后的重进 TUI 为循环粒度（≤100ms），
  钩子还原到自愈间一两帧可能瞬时花屏后恢复；零闪烁需恢复感知终端句柄（涉及 TUI 架构，
  未夹带）。
- **BT partial 进度不持久化**（本轮登记口径）：引擎 partial（已收块未验完）只存内存，
  暂停/重启后该部分退回未下载、由引擎磁盘 SHA-1 校验自愈重下——libtorrent 同口径
  （piece 粒度断点），如需块粒度断点请操作者裁定。
- 源码注释全量删除口径延续；挂载套件剥头副本随产品源同步（本轮改动文件无剥头对应项）。
- **规格偏离登记延续**：FR-02-17「单 peer 在飞窗口 5 块」放宽为 `PEER_PIPELINE = 32`；
  FR-02-02「trackers = 空（显示占位提示）」改为预填 config + .torrent trackers。feature
  文件未改（属 specifier 职责），QA 复核时以登记为准。
- **设计决定登记（沿袭 + 本轮新增）**：
  - **registry.json = 任务档案、.ezr = 数据断点**（bug5 定案）；sidecar 版本恒 1、同形
    v2 遗留文件放行（bug6/本轮定案，旧全量档嵌套形状仍拒绝）。
  - **BT 分块三集合**（本轮定案）：`done_blocks`（验完）/`partial_blocks`（引擎 partial
    表，黄）/`active_blocks`（在途请求，蓝，仅下载态）；BT 任务 `connections` 恒空、
    `pieces()` 走 BT 分支绝不用 HTTP lease；`BtProgress` 携带三集合 + uploaded +
    upload_speed；`BtStopped` 携带终态 done/partial 集合（暂停即还原，不靠重启回读）。
  - **单连接-per-peer**（本轮定案，对齐 libtorrent）：`fill_peer_slots` 按 IP 去重；
    握手 peer_id 去重（后到者 Drop 关闭、候选回退不计失败）；ConnStat 记 peer_id。
  - **peer 速度显示**（本轮定案）：EMA 不随窗口超时重置；`SpeedWindow::push` 超窗清
    基线（rate 不跨停顿期摊分母）；'-' 仅表示从未传数据，停止传输平滑衰减至 0 B/s。

### 实现定义值登记
- 沿袭 phase-01/02 全部口径（D1–D59）+ bug5/bug6 轮补充（`RECHOKE_INTERVAL_SECS` = 10、
  `OPTIMISTIC_INTERVAL_SECS` = 30、`UPLOAD_SLOTS` = 4、`PEER_RETRY_BASE_SECS` (5) ×
  2^fails 封顶 `PEER_RETRY_CAP_SECS` (120)、`PEER_PIPELINE` = 32、`REQUESTS_PER_PIECE`
  = 4、per-tracker completed Notify 立即发送、BT_SIDECAR_VERSION = 1 等）。
- 本轮新增：无新常量；行为口径变更见上「本轮定案」。

## 二、当前产出情况

### 本会话产物清单
- **Bug①（下载速度极慢）**：分层处置。**主根因与 Bug② 同源**——暂停→继续的双引擎竞态
  会造成重复下载与反复重校验，平均速度坍塌（见 Bug②）；另两处真实短板一并修复：
  ①出连 TCP 拨号超时 10s→3s（`CONNECT_TIMEOUT_SECS`）：公网 swarm 死 peer 占比高，
  10s 级拨号把并发槽位成片占满、拖垮连接建立（libtorrent 连接超时为秒级）；
  ②引擎每消息 CPU 热点削减：`handle_peer_msg` 原本对**每条**消息（含 KeepAlive/Request/
  Cancel）都全量重算 claimed+availability（O(conns×pieces)），改按需计算（仅 Have/
  Unchoke/Block 补请求路径）；Bitfield/Have 的 `covered_pieces().contains` 线性扫描改
  `PieceMap::covers` 二分（Bitfield 原为 O(pieces²)）。管线机制（pipeline 32×16KiB、
  nodelay、按 piece 落盘 flush、上传服务、tit-for-tat choker）经比对确认完好。
- **Bug②（暂停→继续偶尔进度清零）**：根因 = **乐观暂停 + 引擎异步消费 Pause 的竞态**。
  `toggle_pause` 同步置 Paused、`try_send` 异步发 Pause；旧引擎退出前（大种子启动重校验
  可达数十秒）用户恢复 → 调度器拉起第二个引擎：双引擎同写文件与 sidecar、进度集合互相
  覆盖、旧引擎 `BtStopped` 终态回写 `t.downloaded` —— 进度回退/清零重下。修复三件套：
  ①app 层 `bt_stopping` 登记门闸（HashMap<u32, Instant>）：暂停三分支（Downloading 暂停/
  Queued-held 退出/Failed 暂停）写入，BtStopped/BtFailed/BtDone 清除；`tick_grant_and_start`
  见登记在册的 BT 任务先归还槽位不启动；180s 兜底强制放行（防引擎挂死把任务永久卡死）；
  ②引擎启动重校验循环 `select!` 竞争 cmd：Pause/StopSeed 立即保存退出——**保存集 =
  已校验 ∪ 未校验预填余量**（只存已校验子集会让 sidecar 回退、余量误判丢失重下；余量
  下次载入本就重新校验，零进度损失、校验失败者自然排除），Cancel 立即退出不保存
  （与主循环同语义）；③`on_bt_stopped` 分态处理：Paused=正常暂停流落地终态集合与
  downloaded；Completed=停止做种流仅清连接/速度显示；Queued/Downloading/Seeding=旧引擎
  滞后回报整体忽略（状态不得回落 Paused、集合不得覆盖新引擎进度）。
- **Bug③（同 IP 两行复发）**：根因 = 去重发生在握手完成（Connected），但交叉拨入的入连
  从 **accept 起**就以同 IP 挂在 conns（peer_id=None），`conn_views` 照渲染——对方每次
  重拨都出现几秒同 IP 第二行，反复出现即「没修」。修复：`conn_views` 跳过
  `peer_id.is_none()`，并发 panel 只展示已建立连接；引擎去重语义不变（出连按 IP +
  握手按 peer_id）。同 IP 不同 peer_id（NAT 后多实例）视为不同 peer 保留两行（合法）。
- **Bug④（分块窗口「并发」口径）**：`conns_display_total` 抽出单任务版
  `conns_actual_of`（BT=bt_conns 行数即已建立 peer 数；HTTP=活跃 lease 数；非下载态 0），
  pieces 弹窗元信息行改用之——与并发面板徽标同口径；任务设置的最高并发只在编辑对话框。
- **测试（新增 ×8）**：引擎 ×1（conn_views 隐藏握手未完成连接）；app/engine/bt ×4
  （bt_stopped 竞态不覆盖新引擎进度 / 落终态改 Paused 态 / BtStopped 清登记 /
  BtFailed 清登记）；app/engine/tick ×2（bt_stopping 在册不启动、清登记后启动 /
  180s 兜底过期放行）；ui/pieces ×2 内含文案口径更新 ×3 处（HTTP 无连接 0、
  HTTP 活跃 2 非设置值 4、BT 已建立 2）。
- **`project/notes/`**：architecture.md 新增四条（同 IP 两行复发根因 / 「并发」口径 /
  暂停继续竞态与门闸三件套 / 速度短板两处）；INDEX.md 同步。

### 验证证据
| 验证项 | 结果 | 命令（可复现，均在 project/ezr 下） |
|---|---|---|
| bin ezr | **804P / 0F** | `cargo test --bin ezr --quiet` |
| 其余 bin 目标 | 15P / 0F | `cargo test --bin ezr-fixture --bin ezr-mktorrent --bin ezr-proxy --quiet` |
| clippy | 0 警告 | `cargo clippy --bin ezr` |
| fmt | 干净 | `cargo fmt`（rust-toolchain.toml 钉 nightly） |
| harness 互操作 e2e | **两连 PASS** | `project/harness`：`python3 swarm_e2e.py --ezr-bin ../ezr/target/release/ezr --payload-mb 4 --timeout 150`——libtorrent seeder+leecher 真实 swarm，HTTP(BEP 3)+UDP(BEP 15) 双 tracker，ezr 2.2s 完成转做种，三方 SHA-256 一致 |
| harness 速度基准 | **ezr≈libtorrent** | 同上 `--payload-mb 256 --timeout 420`：ezr 126.8s（2.02MB/s）、libtorrent leecher 128s（2.0MB/s）——同 swarm 同 seeder 双 leecher 同时下载，两者持平，管线无短板 |

### 对账口径
- 上轮基线 796（bin ezr）+ 15（其余 bin，上轮 46 为全量含挂载套件口径，本轮实测其三
  bin 目标 15 与上轮该三目标一致）。本轮 bin ezr 796→804（净 +8：新增 ×8，文案口径
  更新 ×3 断言并入原测试，实测吻合）。
- release 构建耗时 3m28s；256MB 基准为新增验证项（操作者速度质疑的对照证据）。

### 待办与待批
- **待批（沿袭）**：监听端口随机是否需固定范围/config 键；FR-02-17/FR-02-02 规格同步；
  死 tracker 约 4 分钟静默窗可视状态；做种任务重启不重挂引擎协程（FR-02-22⑥）；多协议
  喂窗公共化重构；BT partial 不持久化（引擎校验自愈，libtorrent 同口径）。
- **待批（本轮）**：①同 IP 不同 peer_id（NAT 后多客户端实例）保留两行——严格
  libtorrent `allow_multiple_connections_per_ip=false` 需按 IP 拒绝第二个已建立连接，
  但会破坏回环夹具（seeder/leecher 同 127.0.0.1）与 NAT 合法多实例场景，是否收紧由
  操作者裁定；②bt_stopping 兜底 180s 的取值；③「并发」列现在为实际连接数，设置值
  不再在 pieces 弹窗展示（如需「实际/设置」双口径请裁定）。
- 遗留观察项沿袭：黑名单仅按 IP、无 UI 展示；BT 多文件「删除并清理文件」对目录无效；
  endgame 重复请求无去重上限；choker 速率窗口 10s 判别粒度；availability 仍每事件
  全量重算（本轮已减频，增量维护可再优化）。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。建议顺序：
  ① `bt/engine.rs` ~3500 行持续膨胀（本轮 +~120 行），msg 处理/choker/fill_peer_slots/
  conn 建立路径拆分评估；②`conns_actual_of`/`conns_display_total`/并发面板徽标三处
  同口径来源可收敛为单一 helper；③`BtProgress` 事件字段增多，可考虑结构体化载荷。
- 给操作者的手测口径：①BT 下载中打开 pieces 窗口——「并发 N」应等于并发面板行数
  （实际连接数），暂停后为 0；②BT 下载中按空格暂停 → 立即按空格继续（连续多次）——
  进度不得清零/回退，pieces 窗口保留绿/黄块，任务短暂排队后恢复下载；③并发 panel
  下载一段时间观察——同 IP 仅一行（交叉拨入的握手期连接不再显示）；④真实公网 swarm
  下载速度与 qBittorrent 对照（本轮 loopback 基准 ezr≈libtorrent）。
- 后续链：cleaner → architect → hardender → QA（宪法第一章：不自动流转，操作者显式续跑）。
