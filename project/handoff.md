# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（本轮已完成）→ 建议下一节点 six-pack/cleaner
> 会话：ezr-coder-bug17（2026-11-01，「bug17 BT 连接生命周期反吸血规避落地」实现轮；
> 本会话为沙箱重置后的重放轮，产物与首轮一致并全量验证补齐——首轮被中断的 g_appcore/
> g_supervisor 等集成目标本轮全部补跑）
> 上游交接：project/handoff.md（bug17 specifier 轮交接：FR-02-27 四规则 + D66 + 08-modify-bt
> v1.4 + 09-bt-engine v1.37 场景 15 + QA-BE-15，任务依据 = 已批规格 + 操作者 bug17 指令，
> 含「complete 后归档复制 /home/z/my-project/」授权）。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式）+ `project/mission/phase-01…04.md`。
- phase-01（HTTP/HTTPS 内核与 TUI）已收官；phase-02（BT 内核/做种/管理）已实现并经
  互操作夹具多轮收口。bug16 轮已交付 BT 参数热调（FR-02-26/D65）。bug17 轮为操作者
  指令级需求修订落地轮（FR-02-27 连接生命周期礼仪：修改并发 + 暂停/启用按 qBittorrent
  规则优化、不触发对端反吸血），本轮 coder 全量交付（实现 + TDD 测试 + 全量验证）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01/02 及历轮） | features/ 18 份 + qa/ 对应套件 | 已收官/已交付 | 见历轮登记 |
| coder（phase-02 及历次收口轮，含 bug15/16 落地） | `project/ezr/` 全量 | 已交付 | 见 bug16 轮登记（bin 859P 基线） |
| specifier（bug17 规格轮，重放） | mission/phase-02 v1.8 + feature ×2 + QA ×2 + notes ×3 | 已交付 | APS dry-check EXIT=0×2（09: 90 步全唯一/findings 52；08: 49 步全唯一/findings 28 与 bug16 持平） |
| **coder（本轮 bug17 落地，重放）** | 产品 ×2 文件（bt/peer_id.rs + bt/engine.rs）+ 测试 ×5 新增 + notes ×4 文件 | **已交付** | **bin 864P/0F（859 基线 +5）+ 集成 12 目标 5742P/0F + clippy --all-targets 零警告 + fmt 干净** |

### 遗留受限项（当前有效，沿袭 + 本轮登记）
- 沿袭（bug14/15/16 轮登记，全部不变）：BT 下载限速未接；DHT/BEP 10 完整扩展/uTP 属
  phase-03/04；cargo-mutants 缺位；死 tracker 约 4 分钟静默窗；TUI 恐慌自愈边界（bug6）；
  BT partial 进度不持久化（热调不引入新风险：partial 按 piece 键存储、随 sidecar tick 落盘
  的口径不变）；QA-SQ-13 竞态沿袭待办；QA-RS-06 沿袭；supervisor block_worker fatal 竞态
  观察项；`App.windows` 失去读者（cleaner 候选裁决）；ezr-demo verify_ezr.py 场景 N 四项
  预存假 FAIL（待 demo 脚本维护轮）；01-tui-display chips 措辞收编（待 specifier 收编轮）；
  ezr-demo 11 条预存 nightly lint（未触碰文件）；QA 套件可执行化（QA-MB-05/QA-MB-07/
  QA-BE-15 的 runner 化与实跑）属 six-pack/QA 职责。
- **Inversion 访谈受限项（沿袭）**：chat 选择题面板不可用，操作者 bug17 指令文本即阶段 1
  答案（沿 bug13/14/15/16 轮先例，登记 D66③）。
- **沙箱重置（沿袭，specifier 轮登记）**：本轮为重放轮——首轮实现已完成 TDD 红→绿
  （bin 864P/0F）后随沙箱回收丢失；本轮按同套补丁精确重放并补齐全部验证（含首轮被
  中断的集成目标）；纪律 = 每轮 complete 后归档即时复制 /home/z/my-project/。

### 设计决定登记（沿袭 + 本轮实现口径）
- 沿袭 D1–D66 全部裁决（D66 = bug17 轮已批项，见 mission/phase-02.md）。
- **本轮实现定义值（coder 细化，规格框架内）**：
  - 会话级 peer_id = `peer_id.rs::session_peer_id()`（`OnceLock<[u8; 20]>` 进程级单例，
    首次调用惰性生成 `-EZ2000-` 形态随机 ID）；`run()` 改调之；BtSpec 不加字段（21 处
    测试构造点零扰动，夹具 `-QX7777-` 前缀与自连守卫互不冲突）。
  - **连接滞留根治（超出规格框架的必要根因修复，登记待批）**：`run_peer_loop` 的
    `PeerIn::Connected { out }` 由 `out_tx.clone()` 改为 **move out_tx**——修复前引擎
    `run()` 返回（暂停/取消/失败）丢弃 conns 后，写循环 `recv()` 因 sender 计数仍 ≥1
    永不退出，连接整体滞留（fd + task 永久泄漏、FIN 永不发出、排水/收口全不发生）；
    move 后引擎侧 `ConnStat.peer_out` 为唯一 sender，引擎退出即触发 `None → 收口`，
    全部返回路径一处根治。TDD 红证：修复前暂停后 seeder 侧 10s 内观察不到任何收口。
  - `graceful_close` 原语（peer_pump 统一收尾）：`wr.shutdown()`（显式 FIN）→ 排水窗口
    `GRACE_CLOSE_SECS = 5`（timeout 等 reader 自然退出）→ `reader.abort()` 强收读半部；
    替代旧「drop(wr) + 5s detach」（reader 永久 detach 持 fd）。
  - 自关重拨冷却 = shed/evict 臂在 `conns.remove` 后刷新候选 `last_try`（复用既有
    `cooldown_ok`，下限沿 `peer_retry_base_secs = 5s`；fails 分支不动）——关闭记账侧
    实证无缺陷（自关臂先移除连接，Closed 处理器不触达候选，fails 未被污染，specifier
    轮机制描述已按此更正）；缺陷在调度侧 = 自关后候选 last_try 陈旧，回补调度零冷却
    （TDD 红证：回补仅 528µs）。
  - 测试夹具：`serve_seeder_session_as` 抽取 `serve_seeder_stream`（Result 化，终态读
    错误可观察）+ 新增 `probing_seeder`（记录对端握手 peer_ids 与收口错误串）。
  - toast/交互面零改动（本规则全部在引擎层与协议行为层，UI 无感知）。

## 二、当前产出情况

### 本会话产物清单
- **产品（project/ezr/src/）×2 文件**：
  - `bt/peer_id.rs`：`session_peer_id()` 新增（OnceLock 会话单例）+ 单测 ×1（两次获取全等
    + 前缀/字符集形态）；
  - `bt/engine.rs`：`run()` 改调 session_peer_id；`run_peer_loop` Connected 臂 move out_tx
    （连接滞留根治）；`graceful_close` 原语 + `GRACE_CLOSE_SECS` 常量 + peer_pump 收尾
    替换；shed/evict 臂候选 last_try 刷新；测试 ×4 新增（graceful_close 原语两阶段写判定 /
    暂停重启 peer_id 全等 / 暂停收口 EOF 非 RST / 收缩回补自关冷却 ×3 轮时序）+ 夹具重构
    （serve_seeder_stream 抽取 + probing_seeder）。
- **notes**：architecture.md（out_tx 滞留根因 + graceful_close 图谱 + 陷阱三则）+
  testing.md（会话单例与生命周期测试纪律四则）；INDEX 同步。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| cargo test --bin ezr | 864P/0F（bug16 基线 859 + 本轮 +5） | `cargo test --bin ezr` |
| cargo test 集成 12 目标 | 5742P/0F | hardening 137 / engine_backfill 767 / g_appcore 671 / g_engine 666 / g_main 2 / g_proxy 16 / g_slots 190 / g_supervisor 640 / g_throttle_err 412 / v115 790 / ui_backfill 797 / g_v147 654（逐目标前台跑，CARGO_INCREMENTAL=0） |
| cargo clippy --all-targets | 零警告 | nightly 1.101.0-nightly（TUNA 装机） |
| cargo fmt --check | 干净 | `cargo fmt && cargo fmt --check` |
| TDD 红证（重放前首轮实证，重放轮同码同判据） | 3 测试在旧实现下 FAIL | peer_id 跨重启不等 / 暂停后 10s 无收口（连接滞留）/ 收缩回补 528µs 立即重拨 |

### 对账口径
- 产品净改动 ×2 文件：+session_peer_id + Connected move + graceful_close + shed/evict
  last_try 刷新；测试净增 5；计数 bin 859→864；集成 12 目标全绿（逐目标数字见上表，
  与 bug16 轮同口径漂移属既有 HEAD 后置增量）。
- 规格符合性：FR-02-27①（peer_id 会话稳定）/②（自关优雅关闭 + 引擎退出连接真正关闭）/
  ③（自关冷却 + 失败记账不动）/④（单连接与 tracker 事件序列不变，零改动）全部覆盖；
  FR-02-26（bug16 热调立即生效）回归无损（reconfigure_grows/shrinks 集成测试通过）。
- ezr-demo：零改动（行为面修复在引擎层，demo 模拟引擎本就按确认即重建口径模拟；UI/交互
  无任何变化）——与 bug16 轮「行为面无需改代码」同判。
- 机制更正对账：specifier 轮 FR-02-27③/D66 的「误记 fails += 1」描述已由本轮实证更正为
  「自关后候选 last_try 不刷新」（mission 内已带更正注记，规格条款的可观察要求不变）。

### 待办与待批
- **待办（QA 轮）**：QA-BE-15 的 runner 可执行化与实跑（fixture seeder/tracker 留档口径，
  判据 = peer_id 全等 / EOF 非 RST / 事件序列一致 / 冷却内不重拨）；QA-MB-05 修订判据
  runner 化沿 QA-MB-05/07 待办合并处理。
- **待办（沿袭，cleaner 轮）**：`windows` 退役裁决等 bug14 四项不变；ezr-demo 11 条预存
  nightly lint（未触碰文件）。
- **待批（操作者）**：D66 实现定义值（Connected move out_tx 的滞留根治口径、GRACE_CLOSE_SECS
  = 5s 排水窗口、自关冷却 = last_try 刷新复用 cooldown_ok 下限 5s）；规格轮产物的规格措辞
  （specifier 轮待批项一并呈报，含 ③ 机制更正注记）。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。重点核对：
  ①`graceful_close` 与 `peer_pump` 的边界（收尾逻辑已集中，reader/writer 任务结构未动）；
  ②`GRACE_CLOSE_SECS` 常量是否应入 BtTunables/config（本轮定为引擎内常量，避免配置面
  扩散——cleaner/architect 可复核）；③shed/evict 两处 last_try 刷新的重复形态（可考虑提取
  小 helper，2 行 ×2 处，收益有限）；④`serve_seeder_stream` 抽取后 `serve_seeder_session_as`
  的薄壳形态。
- 给操作者的手测口径（修复后）：①对做种 BitComet 1.6（反吸血开启）反复「空格暂停 →
  空格开始」≥5 轮——每轮应快速重连并续传、BitComet 屏蔽列表始终为空、任务列表无僵死
  连接残留；②下载中 BT 任务 M 键 2→1→2 并发循环 ≥5 轮——不触发屏蔽、保留连接不掉线、
  收缩后回补不再瞬时重拨；③与 qBittorrent 同机对照：两者对同一 BitComet 反复暂停/启用
  均应正常。
- 后续链：cleaner → architect → hardender → QA（宪法第一章：不自动流转）。
- 操作者授权的收尾口径：`bin/swarm complete` 后**立即**将产物 tar.gz 复制到
  `/home/z/my-project/`。
