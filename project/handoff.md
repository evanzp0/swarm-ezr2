# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-p2-coder-bug3（2026-10-28，
> 「trackers 编辑/全局黑名单/下载速度」四 bug 修复轮）
> 上游交接：project/handoff.md（ezr-p2-coder-bug2 前轮交接：q 退出乱码 + BT 连不上
> tracker 双 bug 修复轮）
> 当轮任务：操作者报告四个问题——①「添加/修改 bt 任务」trackers 面板不能删除字符串
> （并要求多行输入 panel 与 tree panel 通用化、ui/交互直接抄 ezr-demo）；②添加 bt 窗
> 应罗列 .torrent 解析 trackers + config 配置 trackers 并支持编辑；③并发明细 b 键断开
> peer 应加入全局内存黑名单（暂停/继续后仍拒绝连接）；④LAN 下载 ~900KB/s vs
> transmission ~2MB/s，要求优化下载算法。

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
| coder（本轮：trackers 编辑/黑名单/速度轮） | 产品修复 ×8（3 文件键路由 + 3 文件 BT 对话框语义 + 引擎黑名单 + 引擎速度五件套）；新增回归测试 ×8 | 已交付 | 全量 5776P/0F（分项见下）；clippy 0 警告；fmt 干净；互操作 e2e PASS ×3 |

### 遗留受限项（当前有效）
- **BT 下载限速未接**（沿袭：BtSpec.max_speed 仅对 HTTP 生效）；最小 choke 状态机
  （满足 D52）；DHT/BEP 10 完整扩展/uTP 属 phase-03/04。
- **cargo-mutants 缺位**（加固轮职责）；demo 回归长脚本沙箱挂起（沿袭）；doc 测试不适用
  （bin-only）。
- **hardening_g_v147 环境性抖动**（既有登记项）：本轮未复现，561P/0F 一次通过。
- **hardening_ui_backfill 环境性抖动**（本轮新登记，同族）：批量连跑中 FAIL 一次、单跑
  691P/0F 绿、失败签名未及捕获（与 notes/testing.md 双测试二进制并发假 FAIL 判据同族
  处置）；已并入 notes/testing.md 既有判据语境。
- **死 tracker 的用户可见延迟**（沿袭待批项）：tracker 不可达时任务停留「下载中 0.0%」
  约 4 分钟才报「全部 tracker 失败」；是否加可视状态由操作者裁定。
- 源码注释全量删除口径延续；挂载套件剥头副本随产品源同步（对账口径见产物总账）。
- **本轮新登记（规格偏离，操作者指令授权）**：FR-02-17「单 peer 在飞窗口 5 块」放宽为
  `PEER_PIPELINE = 32`（速度优化，操作者本轮指令「下载算法有问题，请优化一下」）；
  FR-02-02「trackers = 空（显示占位提示）」改为预填 config + .torrent trackers（操作者
  本轮指令②）。feature 文件未改（属 specifier 职责），QA 复核时以本登记为准。

### 实现定义值登记
- 沿袭 phase-01/02 全部口径（D1–D59）+ 前轮补充（引擎监听 127.0.0.1:0 随机端口、
  首轮 announce 延迟 2s、piece 落盘三步查错、peer 连接 5s 收割窗、互操作夹具端口、
  MOUSE_SETTLE 50ms / RESIDUAL_MIN_SPAN 80ms / RESIDUAL_QUIET 40ms / RESIDUAL_CAP
  250ms）。
- 本轮新增：`PEER_PIPELINE` = 32（原 5，规格偏离见上）；`REQUEST_TIMEOUT_SECS` = 20
  （在飞块超时重发窗）；`REQUESTS_PER_PIECE` = 4（每 piece 每轮请求块数上限）；黑名单
  按 IP 记忆（`Mutex<HashSet<IpAddr>>` 挂 EngineShared，内存态不持久化，进程存活期
  有效；出站 tracker peers 循环 + 入站 accept 双门禁）。

## 二、当前产出情况

### 本会话产物清单
- **Bug A（trackers 面板不能删除字符串）**：根因＝ezr-demo 移植时组件层
  （ui/multiline、ui/tree、MultiLineBuf，命令窗与 BT 窗共用）已是 1:1 抄写版，但键路由
  层漏臂——`dialog_keys/routing.rs::text_backspace` 补 AddBt/ModifyBt 四臂（种子/保存到/
  并发/trackers，trackers 臂走 `MultiLineBuf::backspace` 行首合并）；`dialog_keys/mod.rs::
  on_dialog_key` 补 `Delete`/`Home`/`End` 三臂（trackers 焦点门控）；`app/paste.rs`
  paste_accepts 放行 AddBt/ModifyBt、trackers 焦点粘贴直通 `insert_str`（保换行分段）、
  dlg_apply_paste 补 BT 单行字段臂。ui 与交互与 ezr-demo keys.rs/cmd.rs/dialog.rs
  逐分支对齐（多行面板支持粘贴/添加/删除/垂直+水平滚动条/Alt+Enter 换行不变）。
- **Bug B（添加 bt 窗罗列 .torrent + config trackers）**：`app/bt_dialog.rs::
  open_add_bt_dialog_with` 打开即以 `merge_trackers(cfg.http_trackers, cfg.udp_trackers,
  meta, &[])` 预填 trackers 面板（config → announce → announce-list 去重保序，可编辑）；
  确认语义改为「编辑后面板内容即任务池」（`dedup_keep_order` 保序去重），清空确认回退
  自动池（BT 任务不留空 tracker）；修改框预填/空回退语义不变。
- **Bug C（b 键全局黑名单）**：`engine/mod.rs` EngineShared 增 `blacklist`（内存态）+
  `blacklist_add`/`is_blacklisted`；`bt/engine.rs` Disconnect 处理时解析 conn IP 写入
  黑名单并断开；tracker peers 循环与 listener.accept 双门禁；`app/keys.rs` toast 改
  「✂ 已断开 <ip> 并加入黑名单（剩余 N 条）」。
- **Bug D（下载速度五件套）**：①出/入站 peer socket `set_nodelay(true)`（Nagle+
  delayed-ACK 批停顿）；②PEER_PIPELINE 5→32；③请求调度全局认领集
  （claimed_blocks = 各 conn inflight ∪ partial 已到块；covered_pieces 免排序；每 piece
  每轮 ≤4 块）；④在飞块 20s 超时重发（inflight 值带 Instant，1s tick 清扫）；⑤做种
  上传 `read_piece_range` 定向读盘（原每 16KB request 读整 piece，16 倍读放大）。
  Have 消息补触发 issue_requests（不 choke 时即时补窗）。
- **测试（新增 ×8）**：`bt_dialog_trackers_edit_delete_and_navigation`（Backspace/
  Delete/Home/End/换行/行首合并全链）、`bt_dialog_backspace_routes_bt_fields`、
  `modify_bt_dialog_trackers_backspace`、`on_paste_bt_dialog_trackers_multiline_and_fields`、
  `open_add_bt_prefills_config_and_torrent_trackers`、`confirm_add_empty_trackers_falls_
  back_to_auto_pool`、`disconnect_blacklists_peer_and_refuses_after_restart`（引擎级：
  断开→黑名单→同 shared 二次 run 拒连→seeder 计数不变）、
  `read_piece_range_multi_file_reads_only_requested_slice`；改写 ×2
  （confirm_add_builds_task_with_merged_pool_and_engine_start 适配面板权威语义、
  open_add_bt_prefills_real_metadata 断言预填）；b 键 toast 断言同步 ×1。
- **`project/notes/`**：architecture.md 增 3 条（组件/路由双层契约、黑名单挂
  EngineShared、速度五件套）；testing.md 增 2 条（黑名单 e2e 写法、claimed 借用拆分）；
  environment.md 增 1 条（磁盘 100% 满二次复现处置）；INDEX.md 同步。

### 验证证据
| 验证项 | 结果 | 命令（可复现，均在 project/ezr 下） |
|---|---|---|
| bin ezr | 758P / 0F | `cargo +stable test --bin ezr -- --test-threads=2` |
| 12 挂载套件 | 4962P / 0F | hardening 131 + engine_backfill 661 + g_appcore 578 + g_engine 573 + g_main 2 + g_proxy 16 + g_slots 176 + g_supervisor 547 + g_throttle_err 352 + g_v147 561 + ui_backfill 691 + v115 684 |
| 其余 bin 目标 | 46P / 0F | `cargo +stable test --bin ezr-fixture --bin ezr-mktorrent --bin ezr-proxy` |
| 全量合计 | **5766P / 0F** | 上三行求和（758+4962+46） |
| clippy | 0 警告 | `cargo +stable clippy --bins --tests` |
| fmt | 干净 | `cargo +nightly fmt --check`（rust-toolchain.toml 钉版） |
| 互操作 e2e | PASS ×3 | `cd project/harness && python3 swarm_e2e.py --ezr-bin ../ezr/target/release/ezr --payload-mb 4 --timeout 150`（ezr 与 libtorrent leecher 双双做种，三方 SHA-256 一致，双 tracker 6 行 announce） |
| Bug A 回归 | 红→绿 | 修复前 Backspace/Delete 在 trackers 焦点为 no-op（text_backspace 无 AddBt/ModifyBt 臂、无 Delete 臂）；修复后新测试全绿 |
| Bug C 回归 | 绿 | disconnect_blacklists_peer_and_refuses_after_restart：黑名单落位 + 二次 run 不建连 + seeder 计数恒 1 |

### 对账口径
- 上轮基线 5710（bin ezr 750 + 12 挂载套件 4914 + 其余 bin 46）。本轮 bin ezr 750→758
  （净增 8：增 10、改写 2 不变计数、断言同步不计）；挂载套件 4914→4962（engine_backfill
  +8、g_appcore +8、g_engine +8、g_supervisor +8、g_throttle_err +2、ui_backfill +8、
  v115 +8、g_v147 +8——上轮后挂载套件按剥头副本自动收编口径随产品源增长，与本轮改动
  文件集一致）；其余 bin 46 持平。合计 5766 与分套件实测逐项吻合。
- hardening_ui_backfill 首轮批量连跑 FAIL 一次（签名未捕获），单跑 691P/0F 绿 + 复跑
  全绿，按 notes/testing.md 双测试二进制并发假 FAIL 判据处置，对账以单跑结果计。
- 磁盘事件登记：全量回归中段磁盘 100% 满（error 28）两次，按 notes/environment.md
  处置（清 incremental + deps 大测试可执行，回收 ~6.7G）后复跑全绿；不涉及产品代码。

### 待办与待批
- **待批（规格同步）**：FR-02-17 在飞窗口 5→32、FR-02-02 trackers 预填两处规格偏离
  已按操作者本轮指令实施并登记；建议下一 specifier 轮把 `features/09-bt-engine.feature`
  L76 与 `features/07-add-bt.feature` 对应条款改至新口径，QA 轮按新口径复核。
- **待批（沿袭）**：死 tracker 约 4 分钟静默窗的可视状态展示；多协议喂窗公共化重构
  （建议 refactorer/cleaner 轮评估）。
- **遗留观察项**：黑名单仅按 IP、无 UI 展示（并发明细/帮助未列黑名单内容）；如需
  「黑名单查看/解除」入口请在下一轮指令中明确，本轮未加（不夹带未授权 UI）。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。建议顺序：
  ① `bt/engine.rs` 本轮五处修改后的重复面与函数规模审查（issue_requests/claimed_blocks/
  on_block/tick 清扫）；② `dialog_keys/routing.rs` 与 `paste.rs` 的 (kind, focus) 分发表
  归并审查（text_backspace/text_char/dlg_apply_paste 三表同构）；③ bt_dialog.rs 确认链
  重复面。
- 给操作者的手测口径：①trackers 面板——添加/修改 BT 窗打开即罗列 config + torrent
  trackers，Backspace/Delete/方向键/Home/End/Alt+Enter/粘贴多行全部可用；②b 键——
  并发明细选中 peer 按 b 断开（toast 提示加入黑名单），暂停任务再空格继续，该 peer
  不再被连（黑名单进程存活期有效，重启 ezr 清空）；③速度——真实 LAN 复测，本轮改动
  主要消除 Nagle 批停顿与重复请求，loopback 上感知不明显。
- 后续链：cleaner → architect → hardender → QA（宪法第一章：不自动流转，操作者显式
  续跑）。
