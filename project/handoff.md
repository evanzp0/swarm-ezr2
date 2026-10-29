# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-p2-coder-design1（2026-10-29，
> 「BT .ezr 控制文件 / IP 遮掩+seeder 粉色 / 并发速度平滑」三设计需求实现轮）
> 上游交接：project/handoff.md（ezr-p2-coder-bug3 前轮交接：trackers 编辑/全局黑名单/
> 下载速度四 bug 修复轮，5776P/0F）
> 当轮任务：操作者三项设计指令——①BT 下载任务补齐 `.ezr` 控制文件（对齐 HTTP 任务
> sidecar 生命周期）；②BT 并发 panel IP 显示照搬 ezr-demo 遮掩规则（IPv4 `首.*.尾` /
> IPv6 `首:*:尾`），另增规则：peer 是做种 seeder 时其 IP 用粉色显示；③BT 并发 panel
> 上传/下载速度变动太快、单连接时与任务列表速度不匹配，参照 HTTP 并发 panel 速度
> 算法（累计值 1s 滑窗 → EMA）修改。

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
| coder（phase-02 及注释删除/tilde/BT 启动收口轮） | `project/ezr/` 全量 | 已交付 | 前轮 5685P/0F；clippy 0 警告 |
| coder（BT 外部互操作夹具轮） | `project/harness/`；产品修复 ×4；新增回归测试 ×1 | 已交付 | 互操作 e2e 三连 PASS |
| coder（BT 速度归零 bug 修复轮） | 产品修复 ×1；新增回归测试 ×2 | 已交付 | 全量 5710P/0F |
| coder（q 退出乱码修复轮） | 产品修复 ×1（restore_into/drain_input_events）；测试删 1 增 1 | 已交付 | 全量 5710P/0F；clippy 0 警告 |
| coder（ezr-p2-coder-bug3：trackers 编辑/黑名单/速度轮） | 产品修复 ×8（3 文件键路由 + 3 文件 BT 对话框语义 + 引擎黑名单 + 引擎速度五件套）；新增回归测试 ×8 | 已交付 | 全量 5776P/0F；clippy 0 警告；fmt 干净；互操作 e2e PASS ×3 |
| coder（本轮 ezr-p2-coder-design1：BT sidecar/IP 遮掩/速度平滑轮） | 产品改动 ×9 文件（model/bt.rs BtSidecar + engine/mod.rs·bt/engine.rs sidecar 与 seeder + app 四文件速度平滑 + ui 两文件遮掩粉色）；新增回归测试 ×13 | 已交付 | 全量 5866P/0F（分项见下）；clippy 0 警告；fmt 干净；互操作 e2e 本轮未复跑（磁盘受限，沿袭上轮 PASS） |

### 遗留受限项（当前有效）
- **BT 下载限速未接**（沿袭：BtSpec.max_speed 仅对 HTTP 生效）；最小 choke 状态机
  （满足 D52）；DHT/BEP 10 完整扩展/uTP 属 phase-03/04。
- **cargo-mutants 缺位**（加固轮职责）；demo 回归长脚本沙箱挂起（沿袭）；doc 测试不适用
  （bin-only）。
- **环境性抖动族**（既有登记项，本轮两处按既有判据处置）：hardening_g_engine 批量连跑
  FAIL 一次（disconnect_blacklists_peer_and_refuses_after_restart），单跑 ×2 均 6s 绿、
  复跑整批 582P/0F 绿；hardening_g_v147 本轮 570P/0F 一次通过未复现。处置依据
  notes/testing.md 双测试二进制并发假 FAIL 判据。
- **死 tracker 的用户可见延迟**（沿袭待批项）：tracker 不可达时任务停留「下载中 0.0%」
  约 4 分钟才报「全部 tracker 失败」；是否加可视状态由操作者裁定。
- 源码注释全量删除口径延续；挂载套件剥头副本随产品源同步（对账口径见产物总账）。
- **上轮规格偏离登记延续**：FR-02-17「单 peer 在飞窗口 5 块」放宽为 `PEER_PIPELINE = 32`；
  FR-02-02「trackers = 空（显示占位提示）」改为预填 config + .torrent trackers。feature
  文件未改（属 specifier 职责），QA 复核时以登记为准。
- **本轮设计决定登记（操作者指令授权）**：BT sidecar 路径口径 = `save_dir/<task_name>.ezr`
  （复用 namegen::sidecar_path_of，与删除任务清理清单 `delete_task_files` 第三项
  `{name}.ezr` 天然对齐）；「删除任务但保留文件」时 sidecar 保留，供重加同种子自动接续
  （与 HTTP 添加时发现断点的体验对齐）；engine `Cancel` 分支不删 sidecar（任务可能仅
  退队保留，删除含文件场景由 pending_deletes 清单负责）。
- **沿袭观察项（本轮未改）**：BT 多文件任务「删除并清理文件」对目录无效
  （`delete_task_files` 仅 `remove_file`，多文件内容在 `{name}/` 目录下，删除失败静默），
  属既有局限，如需修复请下一轮明确授权。

### 实现定义值登记
- 沿袭 phase-01/02 全部口径（D1–D59）+ 前轮补充（引擎监听 127.0.0.1:0 随机端口、
  首轮 announce 延迟 2s、piece 落盘三步查错、peer 连接 5s 收割窗、互操作夹具端口、
  MOUSE_SETTLE 50ms / RESIDUAL_MIN_SPAN 80ms / RESIDUAL_QUIET 40ms / RESIDUAL_CAP
  250ms、`PEER_PIPELINE` = 32、`REQUEST_TIMEOUT_SECS` = 20、`REQUESTS_PER_PIECE` = 4、
  黑名单按 IP 记忆挂 EngineShared 内存态）。
- 本轮新增：`BT_SIDECAR_VERSION` = 1（BtSidecar 结构 version/id/name/save_dir/
  concurrency/data: BtTaskData，JSON 原子写）；BT sidecar 生命周期＝任务启动即写
  （磁盘校验后）→ done 增长时 1s tick 落盘 → Pause/StopSeed 终态落盘 → 全量完成即删
  → Cancel 不动（删除含文件走 pending_deletes 清单）；恢复口径＝
  spec.done_pieces（注册表）∪ sidecar.done_pieces 后逐 piece SHA-1 磁盘校验，
  坏 piece 自然淘汰（sidecar 可信度依赖校验，不设 infohash 字段）；seeder 判定＝
  peer has ⊇ covered_pieces（选集全量，`has.len() >= covered.len()` 预筛）；
  `PINK` = Rgb(255,153,170)（seeder IP 专用，与 MAGENTA 上传速度色区分）；
  BT 并发速度＝app 侧 `BtConnSpeedStat`（down/up 双 `SpeedWindow` 1s 滑窗 + 双
  `SmoothedSpeed` EMA α=1/5，与 HTTP conn_speed/conn_windows 同构），引擎侧
  conn_views 仍报区间瞬时值但 UI 不再消费（保留字段供 tick 汇总上传速度）。

## 二、当前产出情况

### 本会话产物清单
- **设计①（BT 任务 `.ezr` 控制文件）**：`model/bt.rs` 新增 `BtSidecar`（version=1 /
  id / name / save_dir / concurrency / data: `BtTaskData`，JSON 原子写，save/load/remove
  与 HTTP `Sidecar` 同构）；路径口径 `save_dir/<task_name>.ezr`（复用
  `namegen::sidecar_path_of`）。`bt/engine.rs::run` 生命周期：磁盘校验后启动即写 →
  done 增长时 1s tick 落盘（`done.len() != sidecar_saved_done` 单一脏判据）→
  Pause/StopSeed 终态落盘 → 全量完成（seeding 转换点）即删 → Cancel 不动；恢复口径 =
  `spec.done_pieces`（注册表）∪ sidecar.done_pieces 后逐 piece SHA-1 磁盘校验，坏
  piece 自然淘汰（不设 infohash 字段，校验即安全垫）。删除任务含文件场景由既有
  `delete_task_files` 清单第三项 `{name}.ezr` 自动覆盖；「删除任务保留文件」时
  sidecar 留存，重加同种子自动接续（与 HTTP 添加时「发现有效断点」体验对齐）。
- **设计②（IP 遮掩 + seeder 粉色）**：`ui/conns.rs` 新增 `mask_ip`（照搬 ezr-demo
  `app/model.rs` 规则：IPv4 `首.*.尾`、IPv6 `首:*:尾`），`ip_display` 重写为四分支
  （无 `:` 原样 / 含 `[` 剥括号取 v6 / 恰 1 个 `:` 按 v4:port / 多 `:` 裸 v6+port
  rsplit 剥 port），并发明细 IP 列不再泄露完整地址；`ConnStat.has` ⊇ covered_pieces
  （`has.len() >= covered.len()` 预筛）判 seeder，`BtConnView`/`BtConnRow` 各增
  `seeder: bool` 贯通到行渲染，seeder 行 IP 用新增 `PINK` = Rgb(255,153,170)
  （`ui/mod.rs`），非 seeder 维持 FG，选中行维持 White 优先。
- **设计③（BT 并发速度平滑，对齐 HTTP 算法）**：根因＝引擎侧 `conn_views` 每条
  peer 消息即重置窗口（区间瞬时速率 = 本区间字节 / 区间时长，块到达呈突发 → 抖动
  剧烈），而任务速度走 app 侧 1s 滑窗 → 单连接时两处数值天然不匹配。修法＝与 HTTP
  `conn_windows`/`conn_speed` 完全同构：app 新增 `BtConnSpeedStat`（down/up 双
  `SpeedWindow` + 双 `SmoothedSpeed`），`Evt::BtConns` 处理器喂累计值（陈窗重置 +
  离开连接 retain 回收），`tick.rs::sample_bt_conn_speed_display` 每 SPEED_TICK 把
  滑窗 rate 灌入 EMA（α=1/5，镜像 `sample_conn_speed_display`），UI 的 BT 行速度改读
  `bt_conn_speeds_of`；`on_bt_done`/`on_bt_stopped`/`remove_task`/`clear_completed`
  四处补 `clear_bt_conn_stats`（镜像 `clear_conn_stats` 口径）。引擎侧
  `down_speed/up_speed` 字段保留（tick 汇总上传速度仍消费），UI 不再读取。
- **测试（新增 ×13）**：model 层 ×3（`roundtrip_preserves_everything`、
  `load_rejects_unknown_version_and_broken_json`、`remove_is_idempotent`）；引擎层 ×4
  （`bt_sidecar_written_at_start_with_task_shape` 启动即写 + Cancel 不删、
  `bt_sidecar_prefill_resumes_and_removed_on_completion` 全量预填秒完成 + 完成即删、
  `bt_sidecar_pause_keeps_resume_state` 预填半量 + Pause 落盘数量断言、
  `bt_conns_mark_full_bitfield_peer_as_seeder` 全量 bitfield peer 标 seeder）；app 层
  ×2（`bt_conns_feed_sliding_windows_and_seeder_flag`、
  `bt_conn_stats_pruned_when_conn_leaves_and_cleared_on_stop`）；ui 层 ×4
  （`bt_seeder_ip_renders_pink` 缓冲区 fg 断言、`mask_ip_demo_rules`、
  `ip_display_masks_and_caps_width` 含裸 v6+port 形态、`bt_seven_columns_render_
  engine_rows` 改写为遮掩断言并新增完整 IP 不得出现反断言）。
- **`project/notes/`**：architecture.md 增 2 条（BT sidecar 生命周期与路径口径、
  BT 并发速度三段管线与 HTTP 同构）；testing.md 增 1 条（引擎级 sidecar 测试的
  确定性写法：预填 sidecar + 立即 Pause 避开下载竞态）；INDEX.md 同步。

### 验证证据
| 验证项 | 结果 | 命令（可复现，均在 project/ezr 下） |
|---|---|---|
| bin ezr | 769P / 0F | `cargo test --bin ezr --quiet` |
| 12 挂载套件 | 5051P / 0F | hardening 131 + engine_backfill 672 + g_appcore 587 + g_engine 582 + g_main 2 + g_proxy 16 + g_slots 179 + g_supervisor 556 + g_throttle_err 359 + g_v147 570 + ui_backfill 702 + v115 695 |
| 其余 bin 目标 | 46P / 0F | `cargo test --bin ezr-fixture --bin ezr-mktorrent --bin ezr-proxy --quiet` |
| 全量合计 | **5866P / 0F** | 上三行求和（769+5051+46） |
| clippy | 0 警告 | `cargo +nightly clippy --all-targets` |
| fmt | 干净 | `cargo +nightly fmt --check`（rust-toolchain.toml 钉版） |
| 互操作 e2e | 本轮未复跑 | 磁盘受限（release 构建需 ~3G，当前余 1.6G）；沿袭上轮 PASS ×3，本轮引擎改动为 additive（sidecar 写盘 + seeder 位计算），debug 引擎级测试 ×4 全绿覆盖同路径 |
| 设计① 回归 | 绿 | sidecar 三测试：启动即写/全量预填秒完成且完成即删/预填半量 Pause 落盘 |
| 设计② 回归 | 绿 | `bt_seeder_ip_renders_pink`（TestBackend 缓冲区 fg == PINK/FG 精确断言）+ `mask_ip_demo_rules` + 遮掩渲染反断言 |
| 设计③ 回归 | 绿 | `bt_conns_feed_sliding_windows_and_seeder_flag`（滑窗+EMA 非零、seeder 贯通）+ 回收/清零测试 |

### 对账口径
- 上轮基线 5776（bin ezr 758 + 12 挂载套件 4972 + 其余 bin 46）。本轮 bin ezr
  758→769（净增 11：model ×3 + 引擎 ×4 + app ×2 + ui ×2 新增，1 个既有测试改写
  不变计数）；挂载套件 4972→5051（剥头副本自动收编随产品源增长，与本轮改动文件集
  一致）；其余 bin 46 持平。合计 5866 与分套件实测逐项吻合。
- hardening_g_engine 首轮批量连跑 FAIL 一次（disconnect_blacklists_…），单跑 ×2 均
  6s 绿 + 复跑整批 582P/0F 绿，按 notes/testing.md 双测试二进制并发假 FAIL 判据处置，
  对账以复跑整批结果计。
- 磁盘事件登记：本轮磁盘 100% 满两次（clippy --all-targets 全套件构建 + 全量测试
  并发挤压），按 notes/environment.md 处置（`cargo clean` 一次、清 incremental 一次，
  回收 ~9.6G）；后续全量回归改用 `CARGO_INCREMENTAL=0` 分批跑。不涉及产品代码。

### 待办与待批
- **待批（设计①侧）**：sidecar 数据面当前只服务「engine 启动合并 + 注册表兜底」；
  如需「添加对话框打开时发现同种子 .ezr 显示可接续提示」（对齐 HTTP
  `resumed` toast）请在下一轮明确，本轮未加（不夹带未授权 UI）。
- **待批（沿袭）**：FR-02-17 在飞窗口 5→32、FR-02-02 trackers 预填规格同步；
  死 tracker 约 4 分钟静默窗的可视状态；多协议喂窗公共化重构（本轮
  `sample_bt_conn_speed_display` 与 `sample_conn_speed_display` 已同构，可一并评估）。
- **遗留观察项**：黑名单仅按 IP、无 UI 展示（沿袭）；BT 多文件「删除并清理文件」
  对目录无效（沿袭）；IP 遮掩为显示层（引擎事件流、日志仍持原始 addr，删除
  任务「保留文件」场景重加后续传不受影响）。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。建议顺序：
  ① `bt/engine.rs::run` 本轮新增 sidecar 三写点与既五处修改的重复面审查（save_
  bt_sidecar 单点封装已收敛，确认 tick/Pause/启动三调用点无第四处漏网）；
  ② `tick.rs` 两个 sample_* 函数同构归并（双协议喂窗公共化）；③ `ui/conns.rs`
  ip_display 四分支的形态覆盖是否需要收窄为「有 port 必剥」（沿袭裸 v6 形态歧义）。
- 给操作者的手测口径：①BT 任务启动后，下载目录出现 `<task_name>.ezr`，暂停/继续
  断点保持，完成后 `.ezr` 自动消失；②并发明细 IP 列显示 `1.*.4` / `2001:*:1` 形态，
  完整 IP 不再出现；对全量做种的 peer（如 transmission seeder）IP 呈粉色；③并发
  panel 下载/上传速度不再逐秒跳变，单连接任务的并发速度与任务列表速度量级一致
  （同为 1s 滑窗 + EMA 口径）。
- 后续链：cleaner → architect → hardender → QA（宪法第一章：不自动流转，操作者显式
  续跑）。
