# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（本轮已完成）→ 建议下一节点 six-pack/cleaner
> 会话：ezr-coder-bug16（2026-11-01，「bug16 BT 并发热调落地 + 任务窗口参数立即生效 +
> 全量验证 + ezr-demo 同步」实现轮）
> 上游交接：project/handoff.md（bug16 specifier 轮交接：FR-02-26 总规则 + FR-02-03 v1.7
> 注记 + 08-modify-bt v1.3，任务依据 = 已批规格 + 操作者 bug16 指令，含 ezr-demo 同步授权）。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器（单二进制、跨平台）。需求权威
  `project/mission.md`（分期模式）+ `project/mission/phase-01…04.md`。
- phase-01（HTTP/HTTPS 内核与 TUI）已收官；phase-02（BT 内核/做种/管理）已实现并经
  互操作夹具多轮收口。bug16 轮为操作者指令级需求修订落地轮（FR-02-26 总规则 + BT 并发
  热调），本轮 coder 全量交付（实现 + TDD 测试 + ezr-demo 同步 + 全量验证）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（phase-01/02 及历轮） | features/ 18 份 + qa/ 对应套件 | 已收官/已交付 | 见历轮登记 |
| coder（phase-02 及历次收口轮，含 bug15 落地） | `project/ezr/` 全量 | 已交付 | 见 bug15 轮登记（bin 852P 基线） |
| specifier（bug16 规格轮） | mission ×2 + feature ×2 + QA ×2 + notes ×3 | 已交付 | APS dry-check 08:29→28、03:8→8 |
| **coder（本轮 bug16 落地）** | 产品 ×4 文件（bt/mod.rs + bt/engine.rs + engine/mod.rs + app/bt_dialog.rs）+ 测试 ×7 新增 + ezr-demo 同步 ×2 文档 | **已交付** | **bin 859P/0F + 集成 12 目标 5240P/0F + clippy --all-targets 零警告 + fmt 干净（ezr/ezr-demo）；demo 30P/0F** |

### 遗留受限项（当前有效，沿袭 + 本轮登记）
- 沿袭（bug14/15 轮登记，全部不变）：BT 下载限速未接；DHT/BEP 10 完整扩展/uTP 属
  phase-03/04；cargo-mutants 缺位；死 tracker 约 4 分钟静默窗；TUI 恐慌自愈边界（bug6）；
  BT partial 进度不持久化（热调不引入新风险：partial 按 piece 键存储、随 sidecar tick 落盘
  的口径不变）；QA-SQ-13 竞态沿袭待办；QA-RS-06 沿袭；supervisor block_worker fatal 竞态
  观察项；`App.windows` 失去读者（cleaner 候选裁决）；ezr-demo verify_ezr.py 场景 N 四项
  预存假 FAIL（待 demo 脚本维护轮）；01-tui-display chips 措辞收编（待 specifier 收编轮）；
  ezr-demo 11 条预存 nightly lint（未触碰文件）。
- **Inversion 访谈受限项（沿袭）**：chat 选择题面板不可用，操作者 bug16 指令文本即阶段 1
  答案（沿 bug13/14/15/16 先例）。
- **工具通道文本改写（specifier 轮登记，本轮 coder 编辑同批文件未再触发——常规锚点不受
  影响，含 `[/]` 形态序列时仍按 notes/environment.md 处置）**。
- **QA 套件可执行化（QA-MB-05 修订/QA-MB-07 新增）未做**——属 six-pack/QA 职责
  （qa/runners 为 UI 层 pyte 套件，本轮未触碰）；QA 轮承接。

### 设计决定登记（沿袭 + 本轮实现口径）
- 沿袭 D1–D65 全部裁决（D65 = bug16 轮已批项，见 mission/phase-02.md）。
- **本轮实现定义值（coder 细化，规格框架内）**：
  - 热调命令面 = `BtTaskCmd::Reconfigure { concurrency, files, proxy_endpoint }` +
    `Cmd::ReconfigureBt`（bt_send 转发）；app 确认链仅对运行态（Downloading|Seeding）发送，
    非运行态仅模型更新（沿用既有口径：后续 StartBt 经 make_bt_spec 取新值）。
  - 下调收缩序 = 无在飞请求连接优先、其次累计下载最少（`shed_excess_conns` 纯函数，
    平序按 (inflight_n, cum_down) 升序）；在飞请求随连接拆除立即中断（D30 同构），
    `partial` 块内已写字节保留（按 piece 键存储，不随连接拆除丢失），缺口自动回归。
  - 文件勾选热重算 = pmap/order_cache 重建 + 新选文件句柄补开（复用启动路径
    `open_selected_file_handle`，Result 化）+ 逐连接撤销非覆盖 piece 在飞请求（向 peer 发
    Cancel）+ `recompute_downloaded`（= Σ done × selected_bytes_of_piece，消除 downloaded
    超越新 total 漂移）+ seeding 反转臂（改选使全部选中未完成时回「下载中」）。
  - 代理热切换 = spec.proxy_endpoint 更新 + tracker 池按新代理重建（UpdateTrackers 同路径，
    新 announce 生效；既有连接按旧代理完成——D19 口径）。
  - on_block 非覆盖守卫 = 非 wanted piece 数据早退（不写盘/不计数/不广播 Have）。
  - 引擎循环 `run()` 的 spec 与 pmap 改 mut（热可变字段仅 concurrency/files/proxy_endpoint）。
  - toast 口径：确认恒成功 toast（无种子重读，`⚠ 无法读取种子文件` 分支随重启路径退役）。

## 二、当前产出情况

### 本会话产物清单
- **产品（project/ezr/src/）×4 文件**：
  - `bt/mod.rs`：`BtTaskCmd::Reconfigure` 新增；
  - `bt/engine.rs`：cmd 臂热调（pmap 重建/在飞撤销/已下载重算/收缩-补建/tracker 池代理
    重建/BtConns 立即刷新）+ `shed_excess_conns`/`cancel_inflight_not_covered`/
    `recompute_downloaded` 纯函数 + `open_selected_file_handle` 抽取复用 + on_block 非覆盖
    守卫；测试 ×5 新增（纯函数 3 + 多 seeder 集成 2：grows/shrinks，夹具
    counting_seeder_any/serve_seeder_session_as/独立 peer_id）；
  - `engine/mod.rs`：`Cmd::ReconfigureBt` 变体 + 分发；
  - `app/bt_dialog.rs`：`dlg_confirm_modify_bt` 重写（热调，重启路径退役）；测试 ×2 新增
    （只改并发 = ReconfigureBt 无 CancelBt/StartBt；组合修改 = ReconfigureBt +
    BtUpdateTrackers）+ 既有 trackers-only 测试扩 skip 臂。
- **ezr-demo 同步 ×2 文件（文档面；行为面无需改代码——demo 模拟引擎确认即重建连接保留
  速度，本就满足立即生效）**：
  - `README.md`：变更记录第 25 条（bug16 同步，FR-02-26/D65 全量口径）；
  - `docs/ezr-demo UI界面及交互设计规范.md`：「本轮需求修订」新增 bug16 同步块。
- **notes**：architecture.md（热调落地图谱）+ testing.md（多 seeder 夹具三纪律 + 磁盘满
  处置）各 1 条；INDEX 同步。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| cargo test --bin ezr | 859P/0F（bug15 基线 852 + 本轮 +7） | `cargo test --bin ezr` |
| cargo test 集成 12 目标 | 5240P/0F | hardening 137 / engine_backfill 762 / g_appcore 666 / g_engine 661 / g_main 2 / g_proxy 16 / g_slots 190 / g_supervisor 635 / g_throttle_err 407 / v115 785 / ui_backfill 792 / g_v147 649（逐目标前台跑，CARGO_INCREMENTAL=0） |
| cargo clippy --all-targets | 零警告 | nightly 1.101.0-nightly（rsproxy 装机） |
| cargo fmt --check | 干净 | ezr + ezr-demo |
| ezr-demo cargo test | 30P/0F | 同步后回归；fmt 干净；11 条预存 lint 均在未触碰文件 |

### 对账口径
- 产品净改动 ×4 文件：新增 Reconfigure/ReconfigureBt 两命令 + 引擎热调臂 + 确认链重写；
  退役面 = CancelBt+StartBt 重启路径、`⚠ 无法读取种子文件` toast 分支、种子确认时重读重解析。
- 测试净增 7；计数 bin 852→859；集成各目标 +7 左右（HEAD 后置增量 + 本轮新增散布）。
- ezr-demo 净改动 ×2 文档文件；行为代码零改动（对比结论登记于 README 条目 25）。
- 规格符合性：08-modify-bt-05（热调口径）/07（组合立即生效）/03（文件勾选保速度）/06
  （trackers 热更新）全部由本轮实现覆盖；03-modify-task HTTP 口径未触碰。

### 待办与待批
- **待办（QA 轮）**：QA-MB-05/QA-MB-07 的 runner 可执行化与实跑（含 fixture 多 seeder
  组合用例，夹具纪律见 notes/testing.md bug16 条目）。
- **待办（沿袭，cleaner 轮）**：`windows` 退役裁决等 bug14 四项不变；ezr-demo 11 条预存
  nightly lint（未触碰文件）。
- **待批（操作者）**：D65 实现定义值（收缩序/重算口径/代理 tracker 池重建/on_block 守卫/
  toast 恒成功）；规格轮产物的规格措辞（specifier 轮待批项一并呈报）。
- 无阻塞。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：以本文件为上游交接。重点核对：
  ①`dlg_confirm_modify_bt` 与 `make_bt_spec` 的职责边界（前者不再重读种子）；②bt/engine.rs
  cmd 臂的参数数量与结构（可考虑提取 Reconfigure 应用函数）；③`engine/mod.rs` 中
  `Cmd::Reconfigure`（HTTP）与 `ReconfigureBt` 命名对偶的清晰度；④ezr-demo 11 条预存 lint。
- 给操作者的手测口径（修复后）：①下载中 BT 任务 M 键改并发（上调/下调各一轮）→ 确认后
  任务保持下载中、连接数按新并发增减、进度速度不闪断、无重校验窗（不可感知重启）；
  ②同一次确认同时改并发+文件勾选+代理+trackers → 全部立即生效且任务不中断；③HTTP 任务
  改并发/校验/代理回归不变；④暂停态 BT 任务改并发 → 开始后按新并发执行（沿袭口径）。
- 后续链：cleaner → architect → hardender → QA（宪法第一章：不自动流转）。
