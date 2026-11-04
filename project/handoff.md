# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（本轮已完成）→ 建议下一节点 six-pack/cleaner
> 会话：ezr-coder-bt-speed-endpoint-ema（2026-11-06，coder 轮：BT 下载速度追查第二轮——
> 操作者反馈「实测 ezr 仍比 qtorrent 慢」，怀疑反吸血触发面追加面不合理；落点 =
> 连接去重端点化（废除 per-IP 阻断/60s 空闲置换逃生口）+ 管线速率源引擎级 EMA 与
> libtorrent slow-start 移植）
> 上游交接：project/handoff.md（上轮 coder 排队时间动态管线轮交接）

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式 v1.41）+ `project/mission/phase-01…04.md`。
- phase-01（HTTP/HTTPS 内核与 TUI）收官；phase-02（BT 内核/做种/管理）已实现并经
  互操作夹具多轮收口。历轮 bug11–24、标志列（FR-02-33/D69）全部落地；上上轮「BT 下载
  速度优化」（排队时间动态管线 + 零连接紧急重拨）与上轮 QA runner 同步轮均交付；
  本轮为「BT 下载速度追查第二轮」（纯性能轮，无 features/qa/mission 文件变更）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| **coder（本轮，速度追查二轮）** | `project/ezr/`：`bt/engine.rs` 连接去重端点化（`inbound_admitted` 改 (ip,port) 判定 + `fill_peer_slots` 的 connected_ips 集改 connected_eps，per-IP 阻断与 60s 空闲置换逃生口退役，peer_id 重复裁决保留为单连接收口）+ 管线速率源改造（ConnStat 新增 pipe_target/pipe_slow_start/pipe_ema/pipe_sample/tick_prev_down/pipe_last_second 六字段，`tick_pipeline` 仅在引擎 1s tick 更新：整数 EMA α=1/5 + slow-start 每收一块 +1 + 平台退出 + `ema_target` 钳 [2, peer_pipeline]，`issue_requests`/`request_blocks_of_piece` 改读 pipe_target，on_block 顶部 slow-start 增量）+ 4 个新单测（ema_target 口径/slow-start 平台退出/UI 快照隔离/同 IP 异端口候选与同端点去重/端点级入连改写）+ 4 个旧单测改写（rate_conn 助手/fresh 连接爬坡/inbound 断言/fill 两例）；`project/notes/` 三文件 + INDEX 同步 | 已交付 | **bins 919P/0F + 集成 12 组全绿**（hardening 142、engine_backfill 822、g_appcore 721、g_engine 716、g_main 2、g_proxy 16、g_slots 202、g_supervisor 690、g_throttle_err 451、g_v147 704（HTTP 滴流 flake 单跑绿沿例）、ui_backfill 852、v115 845）+ clippy 0 警告 + fmt 干净 + swarm_e2e PASS 三方 SHA-256 一致 |
| coder（上上轮，BT 速度优化一轮） | 排队时间动态管线（当时速率源=down_window）+ 零连接紧急重拨 + peer_pipeline 64→512 | 已交付（速率源缺陷本轮修正） | 本轮 s1l0 复测 2.39MB/s 无回归 |
| coder（上轮，QA runner 执行层同步） | suite_tui_display.py 等 runner 同步 | 已交付 | 沿袭 |

### 遗留受限项（当前有效，沿袭 + 本轮修订）
- 沿袭（历轮登记，除注明外全部不变）：cargo-mutants 缺位；死 tracker 约 4 分钟静默窗；
  TUI 恐慌自愈边界（bug6）；QA-SQ-13 竞态沿袭待办；QA-RS-06 沿袭；supervisor
  block_worker fatal 竞态观察项；`App.windows` 失去读者（cleaner 候选裁决）；leech 判据
  无 e2e 自动化（阈值常量固定）；bad_piece 归属口径 = 交付完成块 peer 计 1 次（D67
  允许）；QA-TD-23 与 QA-UC-07/17/18 环境预存 FAIL 待 QA 轮复验。
- **本轮修订**：上轮登记的 v147 `engine_conns_quota_exact_and_unique_ids` flake 本轮
  未复现（该用例复验绿）；新增沿袭一条 = `periodic_flush_writes_sidecar_mid_download`
  （HTTP 206 滴流，BT 改动零接触面）full-run 单次 FAIL，单跑即刻绿 + 全量复跑绿，
  沿 notes/testing.md 非回归口径继续观察。
- **本轮新增受限项**：①同 IP 拓扑的残余启动段（~10-14s）= 第三方 seeder 对第二个同 IP
  入连的 duplicate_peer_id 拒绝 + 一次 5s 退避，属 libtorrent 自身行为，ezr 不可修；
  ②leech 封禁/黑名单仍按裸 IP（D67 口径）：同 IP 多 peer 场景封一个 peer 会连坐同 IP
  其他 peer（含 seeder），真实局域网各机独立 IP 无此问题，同机多客户端压测场景需注意；
  ③ezr 不发 KeepAlive（libtorrent keep_alive 按超时半程补发）：默认 inactivity_timeout
  600s 下无影响，未移植。
- ezr-demo clippy 9 警告为未触碰文件既有基线（本轮未触碰 ezr-demo）。
- 工具链：rustup nightly 1.101.0（PATH = ~/.cargo/bin）+ cargo registry rsproxy-sparse
  + libtorrent 2.1.1/torf/pyte（pip 阿里云镜像）本轮可用；libtorrent C++ 源码浅克隆在
  会话 `.work/tmp/lt-src/`（RC_2_0，gh-proxy 镜像，complete 后随会话清除，语义结论已
  沉淀 notes/architecture.md）。

### 实现定义值登记（累计口径，本轮落地处加粗）
| 值 | 取值 | 位置 |
|---|---|---|
| **连接去重口径（本轮落地）** | **端点级 (ip:port)**：候选拨出与入连接受按端点判重；同一客户端实例的出连+入连双通道由握手后 peer_id 重复裁决收口（保留） | bt/engine.rs `fill_peer_slots` / `inbound_admitted` |
| **管线深度目标（本轮落地）** | **pipe_target：slow-start 初值 4（libtorrent 成员初始化）+ 每收一块 +1；平台退出（未 choke ∧ 上秒进账>0 ∧ 上秒+5KiB≥本秒）后 = 3s × EMA ÷ 16KiB，钳 [2, peer_pipeline]**；EMA = 整数 avg×4/5 + sample/5（α=1/5），仅引擎 1s tick 采样，与 UI 窗口解耦 | bt/engine.rs `tick_pipeline` / `ema_target` |
| **[bt] peer_pipeline 缺省** | 512（不变；语义 = EMA 深度上限；libtorrent max_out_request_queue=500 临近，>500 在飞会被对端丢弃） | config.rs DEFAULT_PEER_PIPELINE |
| **零连接紧急重拨** | conns 空时重试退避基数与上限均降 1s（不变） | bt/engine.rs fill_peer_slots |
| 标志列参考源（D69①，已批） | qBittorrent Peers（用户）面板 Flags 列 | phase-02 FR-02-33 |
| 标志集（D69②，已批） | qBt 图例 ezr 可产生子集 {D,d,U,u,O,I,K,?} | bt/engine.rs `conn_flags` |
| 标志列结构（D69③④⑤，已批） | 地址(18)/标志(5)/下载速度(8)/上传速度(8)/累计下载(8)/累计上传(8)/客户端(34)/协议(4)，内容宽 99 | ui/conns.rs BT_COLW/BT_CONTENT_W |
| 客户端收缩基线（D69⑥，已批） | max(26, 视口宽 − 65) | ui/conns.rs BT_CLIENT_SHRINK_BASE |
| leech 阈值 | up ≥ 1 MiB 且 down < 64 KiB 且存活 ≥ 120s | bt/engine.rs LEECH_* 常量 |
| bad_piece 阈值 | 同连接累计 3 次 | BAD_PIECE_BAN_THRESHOLD |
| protocol 阈值 | 未知 id/帧错首次即封；非法区间 request 重复 3 次 | INVALID_REQUEST_BAN_THRESHOLD |
| 限速输入上限 | 32 列（FR-01-06 形态对齐） | routing.rs CAP_LIMIT |
| 限速单位口径（config 文件键） | KiB/MiB/GiB 二进制 + 裸整数与 B/s = B/s；KB/MB/GB 十进制废弃 | config.rs SPEED_UNITS / parse_speed |
| BT sidecar partial 持久化（D68） | BtSidecar 可选 partial_pieces（版本恒 1）；恢复验证链五条件 | model/bt.rs + bt/engine.rs |
| endgame 已收块豁免（D68⑤） | endgame 臂跳过 partial 已收块 | engine.rs request_blocks_of_piece |
| trackers 弹窗几何（D67⑤） | 94×22；末列「消息」起点 76、预算 12 列 | ui/popups.rs |
| 修改窗口清空限速确认（D67②） | 回填 config.toml 任务级默认 | dialogs.rs / bt_dialog.rs |

## 二、当前产出情况

### 本会话产物清单
- **产品端 `project/ezr/`（仅 `src/bt/engine.rs` 一个文件）**：
  - 连接去重端点化：`inbound_admitted(conns, (IpAddr, u16), cap)`（原 (conns, ip, cap)
    同 IP 拒绝改同端点拒绝）；`fill_peer_slots` 的 `connected_ips: HashSet<IpAddr>` →
    `connected_eps: HashSet<(IpAddr, u16)>`（候选过滤 `!connected_eps.contains(k)`）；
    注释改写为 libtorrent peer_list 真实语义口径。peer_id 重复裁决（Connected 臂）原样
    保留，成为「同一客户端实例单连接」的唯一收口。
  - 管线速率源：ConnStat 新增 6 字段（pipe_target=4 起步/pipe_slow_start=true 起步/
    pipe_ema=0/pipe_sample=(0,now)/tick_prev_down=0/pipe_last_second=0）；
    `tick_pipeline(st, now, cap)`（整数 EMA α=1/5 采样 + slow-start 平台退出 + EMA 目标
    重算）挂入引擎 1s tick 的 conns 遍历（与 inflight 超时 retain 同循环）；
    `on_block` 顶部 slow-start 每块 +1（min(peer_pipeline)）；`issue_requests` 的
    `pipeline_target(speed(down_window))` → `st.pipe_target`；常量 PIPELINE_MIN 64→2、
    PIPELINE_QUEUE_TIME_SECS f64 3.0 → u64 3；`pipeline_target` 纯函数退役 →
    `ema_target(ema_rate, cap)`。
  - 测试：新增 `ema_target_floor_cap_and_rate_scaling`、
    `tick_pipeline_slow_start_grows_and_exits_at_plateau`、
    `tick_pipeline_ema_stable_under_ui_snapshot_resets`、
    `fill_peer_slots_dials_same_ip_different_endpoint`、
    `fill_peer_slots_skips_connected_endpoint`；改写
    `issue_requests_scales_queue_with_rate`（直接设 pipe_target 480/64）、
    `issue_requests_fresh_connection_ramps_per_block_in_slow_start`（4→20 爬坡）、
    `inbound_admitted_blocks_same_endpoint_and_cap`（同端点拒/同 IP 异端口放行）、
    pause_persists…（假 peer serve 集 = 全块减 (0,1)(0,2)，partial 断言按 piece 定位，
    适配 slow-start 起步深度 4 下的请求可达性）。
- **`project/notes/`**：architecture.md 新增「速度追查第二轮」节 5 条（per-IP 误移植/
  速率源污染/libtorrent 请求队列全参数/choker 反吸血疑云排除/第三方同 IP 纠缠不可修）；
  testing.md 新增 3 条（slow-start 下假 peer serve 集可达性前提/负载敏感 flake 沿例/
  bench s1l1 判读口径）；INDEX.md 两行同步。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 产品端 bins 单元测试 | **919P/0F**（915 基线 + 5 新 − 1 合并改写） | `cd project/ezr && CARGO_INCREMENTAL=0 cargo test --bins` |
| 产品端集成测试（12 组） | **全绿**（142+822+721+716+2+16+202+690+451+704+852+845） | `CARGO_INCREMENTAL=0 cargo test --test <套件名>` 逐组 |
| clippy / fmt | 0 警告 / 干净 | `cargo clippy --all-targets` / `cargo fmt --check` |
| BT 互操作回归 | PASS（双 tracker + libtorrent 对端 + 三方 SHA-256 一致） | `cd project/harness && python3 swarm_e2e.py --ezr-bin ../ezr/target/release/ezr --payload-mb 4` |
| **竞享 seeder 基准（用户场景复现，核心证据）** | **修复前 ezr 115.9s / 均速 1.16MB/s（前 60s 死区：只连 leecher 17920，seeder 17903 被同 IP 规则挡死，t≈64s 空闲置换才满速）；修复后 64.9s / 均速 2.07MB/s（死区→~14s 第三方拒绝+一次退避；稳态 ezr 吃满 seeder 2560KiB/s 上限并中继给 lt leecher）**，sha256_match | `python3 bench_multipeer.py --ezr-bin ../ezr/target/release/ezr --seeders 1 --leechers 1 --payload-mb 128 --seeder-upload-kbps 2560 --workdir <abs>` |
| 单 leecher 基准（无回归对照） | 56.1s / 2.39MB/s（基线 ~2.44，理论上限 2.5 = seeder 限速），sha256_match | `python3 bench_multipeer.py --seeders 1 --leechers 0 --payload-mb 128 --seeder-upload-kbps 2560 --workdir <abs>` |

### 对账口径
- 规格基线 = 上轮交接；本轮**未触碰任何 features/qa/mission 文件**（coder 职责边界），
  实现全部落在 `project/ezr/src/bt/engine.rs` 与 `project/notes/`；本轮为性能追查轮，
  mission 版本不变。
- 配置兼容性：`peer_pipeline` 键名/键位/取值范围不变（语义 = EMA 深度上限，与上轮
  「动态深度上限」口径一致）；无新增配置键（EMA/slow-start 参数为代码常量，登记见上表）。
- **操作者问题「反吸血触发面追加面是否不合理」的结论**：bug17/bug23 的追加面（会话
  peer_id/优雅 FIN/last_try 刷新/恢复去重）本身合理且保留；真正不合理的是更早轮次的
  **per-IP 连接去重**（候选阻断 + 入连拒绝 + 60s 空闲置换）——它把「防同一客户端双连」
  错做成「防同 IP 一切连接」，在 seeder 与其他 peer 同 IP 的拓扑（同机多客户端压测、
  NAT 共享出口）里制造 60s 级死区，是「实测仍慢」的主因；次要因 = 动态管线速率源被
  UI 快照窗口污染，突发-间隙流量下目标塌向下限并拒绝补管线（夹具平滑流量测不出）。
  两处均已按 libtorrent 源码（RC_2_0 peer_list/choker/peer_connection/stat）口径收口。
- 速度归因链更新（接上轮口径）：启动段死时 = 握手碰撞吃满超时 + 深退避（上轮紧急重拨
  收口）+ **同 IP 拓扑候选阻断死区（本轮收口）**；稳态段 = **速率源污染导致的管线塌陷
  （本轮收口）**；实测 1.5 vs 2.4 的量级差由两段叠加呈现。

### 待办与待批
- **待批（如操作者认为需要规格化）**：本轮为纯性能轮，未登记新 FR；若操作者希望把
  「端点级连接去重 + 管线深度引擎级 EMA/slow-start」写入 phase-02 需求（含 [bt]
  peer_pipeline 语义口径微调），建议开 specifier 小轮归档。
- **待办（cleaner 轮候选，沿袭）**：`App.windows` 失去读者裁决；ezr-demo 既有 9 处
  clippy 警告清理。
- **待办（QA 轮沿袭）**：QA-TD-23 与 QA-UC-07/17/18 环境预存 FAIL 复验。
- **待办（观察项，本轮新增）**：leech 封禁按裸 IP 在同 IP 多 peer 场景的连坐面（见
  遗留受限项②）；ezr 无 KeepAlive 在超长静默窗的断连面（见③）。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接（本轮改动面收敛于
  bt/engine.rs 单文件 + notes，cleaner 重点沿袭候选：`App.windows` 裁决、demo clippy
  基线）。
- 后续链：cleaner → architect → hardender → QA（宪法第一章：不自动流转）。
- 操作者授权的收尾口径：`bin/swarm complete` 后立即将产物 tar.gz 复制到
  `/home/z/my-project/`（沿历轮授权口径执行）。
- 工具链交接：rustup nightly 1.101.0 已装（PATH = ~/.cargo/bin）；harness Python 依赖
  （libtorrent 2.1.1/torf/pyte）本会话可用；libtorrent C++ 源码浅克隆在
  `.work/tmp/lt-src/`（complete 后清除，语义结论已沉淀 notes/architecture.md，需要
  复查时 gh-proxy 镜像浅克隆 RC_2_0 即可）；本轮基准产物在 `.work/tmp/bench-compete|
  compete2|solo2/`（随会话清除）。
