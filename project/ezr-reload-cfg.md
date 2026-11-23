所有事实我已核实完毕（含 qBt/libtorrent 对比、ezr 各配置项的消费点），以下是完整改动方案。

# ezr Ctrl+R 配置热加载 改动方案

## 一、目标与交互定义

按下 **Ctrl+R** → 重新读取 config 文件并热加载，无需重启 ezr。

- 全局可用（主界面、弹窗内均响应，与现有 Ctrl+C 同级处理）；
- **原子性**：任何一个键校验失败 → 整体拒绝热加载，旧配置原样保留，toast 报错（复用 `Config::try_load` 现有的逐键报错文案「键 xxx：…」）；
- 成功后 toast 汇总结果（如「✓ 配置已热加载；代理切换：2 个任务」）。

## 二、改动点（共 6 处，不含测试）

| # | 文件 | 改动 |
|---|---|---|
| 1 | `app/keys.rs` | `on_key` 的 `mods.contains(CONTROL)` 分支增加 `KeyCode::Char('r')` → `self.reload_config()`（当前该分支仅处理 Ctrl+C 退出，main.rs:382 已传入 modifiers） |
| 2 | `app/engine/mod.rs`（App 实现） | 新增 `fn reload_config(&mut self)`，流程见下 |
| 3 | `engine/mod.rs` | `Cmd` 枚举新增 `ApplyGlobals { dl, ul, burst_secs_bits, sleep_ms }`；`main_loop` 处理：`shared.throttle_dl.set_rate(dl)`、`throttle_ul.set_rate(ul)`，并更新 burst/sleep 原子字段 |
| 4 | `engine/mod.rs`（EngineShared） | `throttle_burst_secs: f64` / `throttle_sleep_ms: u64` 改为原子存储（burst 用 `AtomicU64` 存 f64 bits），使热加载后**新启动任务**读到新值 |
| 5 | `model/task.rs` | `Task` 新增 `running_proxy: Option<ProxyEndpoint>`（仅内存态；`Registry::from_tasks` 是显式字段映射，不受影响、不持久化）。在 `tick_grant_and_start` / `send_retry_start` / `start_bt_from_slot` 启动任务时写入 |
| 6 | `ui/help.rs` | `help_lines()` 全局节新增一行：`kd("Ctrl+R", "重新加载配置文件（热加载，无需重启）")`（窗口本身可滚动，无需改 HELP_H） |

`reload_config()` 流程：

1. `config_path()` 为 None → toast「无配置文件」，结束；
2. `Config::try_load(path)` 失败 → toast 错误明细，旧配置原样保留，结束；
3. 成功后：
   - 重应用启动时的 CLI `--max-speed` 覆盖（App 新增字段记住该覆盖值，否则热加载会把 CLI 覆盖冲掉）；
   - `self.cfg = new`；`self.max_slots = new.download_slots`（下一 tick 生效）；
   - 重建 `proxy_options` / `proxy_labels`（保持当前已选代理名不变；该代理被删则回退直连）；
   - `engine.send(Cmd::ApplyGlobals { ... })`；
   - 逐任务用新 cfg re-resolve 代理，与 `t.running_proxy` 比较：**有变化才发命令**——HTTP 任务发 `Cmd::Reconfigure { concurrency: t.concurrency, endpoint, max_download_speed: t.max_download_speed }`，BT 任务发 `Cmd::ReconfigureBt`（files/限速/sequential 原值不变，仅带新 endpoint）；更新 `running_proxy`；
   - Named 代理被删除的引用沿用现有 missing 语义：按直连 + toast，多条合并为一条汇总 toast 防刷屏。

## 三、配置项逐项影响分析（核心）

### A 类：立即全局生效

| 配置键 | 生效方式 | 对运行中下载任务的影响 |
|---|---|---|
| `common.download_slots` | `max_slots` 每 tick 被 `slots::allocate` 读取 | 调大：排队任务立即拿到槽位启动；调小：**不抢占**运行中任务，任务结束后不再补位（allocate 只授予不回收，与 qBt 行为一致） |
| `common.global_max_download_speed` | `Throttle::set_rate`（API 已存在，现用于任务级热调） | 所有运行中任务的限速桶立即重定目标 |
| `common.global_max_upload_speed` | 同上 | 同上 |
| `common.proxy` 代理池增/删/改 | 运行中任务逐个 re-resolve，变化才 Reconfigure | **HTTP**：worker 每次建新连接从 watch channel 取当前端点（`workers.rs:192`）——新连接立即走新代理，存量连接跑完当前块自然切换；**BT**：tracker pool 立即重建并 reannounce（`bt/engine/mod.rs:384`），存量 peer 连接不断、新 peer 连接走新代理。与 qBt/libtorrent 语义完全同源（已确认） |
| `http.auto_retry` | 失败判定时实时读取（`evt.rs:429`） | 下一次任务失败判定立即采用新值 |

### B 类：仅影响后续新添加 / 新启动任务（快照语义）

| 配置键 | 烘焙时机 |
|---|---|
| `http.block_size_http` / `http_concurrency` / `max_retries` | 添加任务时写入 Task 字段 |
| `bt.trackers` | 添加 BT 任务时合并进任务 |
| `bt.bt_concurrency` | 添加 BT 任务的默认并发 |
| `bt.peer_connection_protocol` | 任务启动时烘焙进 `BtTunables.transports`，运行中不切协议 |
| `bt` 其余全部 tunables（peer_pipeline、upload_slots、idle_eviction_secs、connect/handshake/request_timeout_secs、requests_per_piece、rechoke/optimistic_interval_secs、tracker_numwant、peer_retry_base/cap、tracker_backoff_start/cap、udp_base_timeout/udp_retries） | 任务启动时烘焙，运行中不重建（`ReconfigureBt` 不携带 tunables，保持现状） |
| `common.task_max_download_speed` / `task_max_upload_speed` | 仅作添加/修改对话框的预填默认值（限速实际存于任务字段，热改走任务级 M 对话框） |
| `common.download_dir` | 新增任务对话框的目录预填默认值 |
| `http.progress_tick_ms` / `sidecar_flush_secs` | `make_spec` 启动时烘焙，下次启动/重试生效 |
| `http.backoff_initial` / `backoff_cap` | **现状即死键**：已解析校验但无任何运行时消费（HTTP 自动重试退避由 `model/retry.rs::backoff_secs` 硬编码），热加载后同样无效。可顺手删除或留待后续接线，建议本次不动 |

### C 类：需重启应用（热加载后 toast 提示“重启生效”）

| 配置键 | 原因 |
|---|---|
| `bt.listen_port` | `bind_bt_listener` 在引擎启动时绑定一次 |
| `http.dns_race_grace_ms` | reqwest 客户端（含代理客户端池）构建时烘焙 grace |
| `http.throttle_burst_secs` / `throttle_sleep_ms` | 本方案改为原子读取：**新启动任务**即生效；运行中任务保持当前桶参数（彻底热切需替换运行中的 Throttle 实例，收益低不做） |
| `bt.upload_read_concurrency` | Semaphore 引擎启动时创建；v1 标注需重启，可选升级为 RwLock 交换 |

### D 类：失败保护

- 任一键类型/取值非法 → `try_load` 整体拒绝 → 热加载保持旧配置并 toast 报错，不会出现半新半旧状态。

## 四、与 qBittorrent 的对齐关系（已确认采用）

- 共同语义：**存量连接不打断、新连接/新 announce 立即用新代理**；
- 差异仅在粒度：qBt 全局一份代理一次 `apply_settings` 下发；ezr 是 per-task 引用具名代理，热加载时需逐任务重解析并按需 Reconfigure；
- 全局限速热调（`set_rate`）与 qBt 的 `applyBandwidthLimits` 同机制。

## 五、明确不做的事

- 不改注册表（registry.json）与边车文件格式；
- 不新增其他路由/文档，快捷键说明只进 help 窗口（按你的要求）；
- 不处理旧版本兼容。

方案完毕。你审核后若无需调整，我可以按此实施；若有想改的点（比如 C 类里想顺手把 `upload_read_concurrency` 也做成热生效，或删掉 `backoff_initial/backoff_cap` 死键），告诉我即可。
