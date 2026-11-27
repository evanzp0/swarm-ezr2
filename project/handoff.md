# 交接（handoff）

> 流程：six-pack　当前节点：**six-pack/coder（20261127 优化轮：MDM 对齐的启动延迟与峰值速度优化 + D93 块请求面 4xx 缺口收口 + stale hardening 断言改写）——已完成**
> 路径：**six-pack/coder（本轮）** → cleaner → architect → (hardender) → QA
> 会话：six-pack-coder-opt1（上游交接 = D94 实现轮 handoff + 操作者 20261127 chat 指令：「EZR 比 MDM（anefzaoui/misterdownloadmanager）的 http 下载峰值小，并且 MDM 输入 URL 确定后就立刻下载了，EZR 输入 URL 确定后要等好几秒才下载，请参考 MDM 源码对 EZR 进行优化」；MDM 参考仓在位 = `.tools/misterdownloadmanager`）
> 环境：bb 1.13.230（`.tools/bb-bin/bb`）+ APS 仓 + MDM 参考仓沿袭在位；rust 工具链 stable 1.100.0 + nightly 双链在位（会话恢复按 notes/environment.md 判别法重建 PATH 注入）；cargo registry rsproxy sparse；`~/.cargo` 与 `.tools/` 非持久沿袭
> 受限项：①chat 选择题面板不可用（IM 通道）沿袭——Inversion 受限退化沿先例；②runner 不可执行沿袭（qa/runners 归 QA 轮同步）；③**MDM 性能基准 = 操作者线下验收口径**（本轮改动无法在本沙箱内与 MDM 实测对表，效果验收归操作者）；④P3 增益层沿规格排除沿袭

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，v1.64）；phase-05 详述 = `mission/phase-05.md`（FR-05-01–13 + D83–D94，v1.2）；ezr 不做旧版本兼容沿袭（v1.45/FR-02-35）。
- ezr-demo：仓库内独立子项目；历轮零接触，本轮零接触。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固、phase-05 规格轮 + 实现轮 + pieces 各修复轮 + 速度回退修复轮 + 族A/族B 裁决落实轮（specifier）+ D90–D94 落地轮（coder） | features 01–13 + qa 规程 + runners + 动态引擎/模板常量/断言面/信息列全量落地 | 已交付（沿上轮账） | 当时口径 bin 1264P/0F + hardening 四套件 1146/1014/1001/1169 全 0F + clippy/fmt/check 全零警告 |
| **20261127 优化轮（本轮，coder）** | **①启动延迟：dns.rs `race_with_grace` 首腿可用即返回（宽限只留首腿空集场景，测试面 3 改 1 增同步）；dynamic.rs probe 取消首 chunk 预读（响应头到达即 spawn 全部 worker，`PrimedResponse` 首块恒 None，rescue 路径同步）；②峰值：throttle.rs `unlimited: AtomicBool` 快路径 + acquire n==0 即返；dynamic.rs 限速只按新到字节支付（carry 不再重复限速）、commit+absorb_next 合并单锁、planner/failure/conns 全部 tokio Mutex → std::sync::Mutex、ConnView 更新 100ms 时间门控（CONN_VIEW_PERIOD）；writer.rs `pending` 增量计数（去 O(queue) 求和）+ push 返回封包 bool + worker step 仅在「封包 ∨ pending≥hold_cap」执行 + 监控 tick `step(u64::MAX)` 回收保 sidecar 新鲜；③D93 收口：error.rs `into_block_retry`（HTTP 前缀 Fatal → Transient「传输中断（HTTP xxx）」），仅接 dynamic worker `req_checked` Err 臂——worker 块请求面 4xx 原地重试按 D93 真实成立（此前靠 probe 预读时序碰巧通过）；④stale hardening 改写（QA v1.63 授权）：g_v147 `failure_freezes_all_worker_progress` 触发器改 D93 原地耗尽路径 + 新 stub_probe_then_trickle_then_403（保 stop_workers no-op 突变体杀伤），blocks 孪生用例与旧 stub 不动；⑤v115 `speed_window_tracks_downloading_task` 独立目录隔离修复（默认下载目录 sidecar 残留陷阱，~/Downloads 残留已清空）；⑥测试面净增：error.rs +1（into_block_retry 四臂）、dns.rs 净 +2（首腿可用/空集宽限两新语义 +1 改语义）、bin 1266P、v115 1171P；⑦notes 三文件 + INDEX 同步** | **已交付** | **fmt --check PASS；clippy --all-targets 0 警告；bin 1266P/0F（211s）；hardening_engine_backfill 1148P/0F；g_engine 1016P/0F；g_v147 1003P/0F；v115 1171P/0F** |

### 遗留受限项（当前有效）
- **①全仓 >100 变异点存量 / CRAP 100 / CPD 222**：历轮登记沿袭；归 architect/cleaner 轮。
- **②runner 断言同步面（归 QA 轮）**：QA-DE-04/05/06/07、QA-HT-12/13b、QA-PC-05 退役行；QA-HT-13、QA-PC-13 键集 41；suite_resume_sidecar.py:211 blocks 断言失效；`suite_config_template.py` 解析锚适配（沿 D94 账）。
- **③族C（gen 剥头副本漂移，1 例，归 cleaner 轮顺带）**：hardening_ui_backfill `gen_fixtures_match_product_sources`，比对面 = src/ui/{chart,detail,btn}.rs + src/app/clipboard.rs 四文件重新生成即可。
- **④qa/09-bt-engine-qa.md:70 字面真缺陷 1 处**（`ttp] 调优键` 应为 `ttp] 调优键`）：归 QA 轮顺带修复（沿袭登记）。
- **⑤MDM 性能基准线下口径**：本轮启动延迟/峰值优化的实测对表归操作者（建议场景：同一大文件 URL 分别用 EZR/MDM 下载，观察「URL 确认 → 首字节落盘」时延与峰值速率；弱网/高 RTT 场景收益更大）。
- **⑥blocks 引擎代码本体仍在编译面**：沿袭（config 面退役后生产不可达）；退役/瘦身归 cleaner/architect 轮裁决。
- **⑦e2e 时限断言负载抖动家族（新增登记）**：`assert!(completed, "…任务应完成")` 类 30s deadline 用例（如 g_engine dynamic_vacated_blocks）在全量并行负载下偶发超时（本轮四次全量两次触发；单测复跑 3/3 绿）——判别与处置纪律见 notes/testing.md；非不变量缺陷，复跑全绿即放行。
- **⑧MDM 未移植项（本轮登记，非缺陷）**：TCP 多地址竞速+赢家缓存（reqwest 无 connect 钩子）、每 worker 独享 Writer（共享 Writer 与 pieces 投影/sidecar 口径耦合，动面大）、BDP/RCVBUF 与慢连接剔除（P3 沿规格排除）——详见 notes/architecture.md 本轮条目。

### 实现定义值登记（累计覆盖为最新全量）
- 历轮登记（D81–D94）全部沿袭有效。
- **本轮新增登记（源裁决可回退，操作者否决 = 直接以裁决文本 begin six-pack/specifier）**：
  - **DNS 竞速语义**：首腿 Ok 非空 → 立即返回（不空等宽限）；首腿 Ok 空集 → 宽限窗内等另一腿（`dns_race_grace_ms` 配置键保留，语义变为「首腿空集时的对族等待上限」）；首腿 Err → 不设上限等另一腿（原语义保留）。
  - **probe 时序**：动态引擎 probe = 裸 GET（新任务）/开区间 Range（续传），响应头到达即 spawn；probe 响应体不预读，直接作为 worker #1 的 primed 数据流。
  - **ConnView 节流口径**：进度字段按 100ms 时间门控更新（CONN_VIEW_PERIOD），事件驱动字段（Connecting/Receiving/Retrying/段轮转）即时更新——per-conn 速度 EMA 采样节拍（SPEED_TICK/Progress 100ms）依赖此门控不得再收紧。
  - **块请求面 4xx 分类**：dynamic worker 块请求（req_checked）HTTP 状态 Fatal → `传输中断（HTTP xxx）` Transient 原地重试（0/1/2/4/8s、上限 5 次后任务级失败 Transient）；探测面/降级单流面（single_stream）不套用。

## 二、当前产出情况

### 本会话产物清单（20261127 优化轮）
1. `src/engine/dns.rs`：race_with_grace 首腿可用即返回 + 测试面同步（新增 usable 即返/空集宽限/超时弃置 3 用例，改写原「宽限拖慢腿」2 用例语义）。
2. `src/engine/throttle.rs`：unlimited AtomicBool 快路径（acquire 顶格即返；set_rate 同步维护）。
3. `src/engine/error.rs`：`into_block_retry` 构造器 + 四臂单测（4xx 重映射/非 HTTP Fatal 不动/Transient 原样/网络失败原样）。
4. `src/engine/supervisor/dynamic.rs`：probe 取消首 chunk 预读；worker 限速只按新到字节；planner/failure/conns std Mutex 化 + commit+absorb_next 单锁；ConnView 100ms 时间门控；writer step 封包/跑道条件执行；监控 tick step(u64::MAX) 回收；rescue 路径取消预读。
5. `src/engine/supervisor/writer.rs`：pending 增量计数；push 返回封包 bool；pending_bytes 转生产可见；absorb 签名携带 job.bytes。
6. `tests/hardening_g_v147.rs`：failure_freezes_all_worker_progress 触发器改写（D93 原地耗尽 + 新 stub_probe_then_trickle_then_403；blocks 孪生用例与旧 stub 保留）。
7. `tests/hardening_v115.rs`：speed_window_tracks_downloading_task 独立目录隔离（temp_dir+进程 id + 收尾清理）。
8. `project/notes/`：architecture.md（MDM 对齐热路径与启动时序全条目）+ testing.md（默认目录陷阱/e2e 抖动家族/D93 改写落实/probe 时序敏感面）+ INDEX 双行同步。

### 验证证据
| 验证项 | 结果 | 命令（cwd = project/ezr，前台串行，CARGO_INCREMENTAL=0） |
|---|---|---|
| cargo fmt --check | **PASS** | `cargo fmt --check` |
| clippy 全目标 | **0 警告** | `cargo clippy --all-targets` |
| bin 全量单测 | **1266P / 0F**（212s） | `cargo test --bin ezr` |
| hardening_engine_backfill | **1148P / 0F**（164s） | `cargo test --test hardening_engine_backfill` |
| hardening_g_engine | **1016P / 0F**（166s） | `cargo test --test hardening_g_engine` |
| hardening_g_v147 | **1003P / 0F**（215s） | `cargo test --test hardening_g_v147` |
| hardening_v115 | **1171P / 0F**（171s） | `cargo test --test hardening_v115` |

### 对账口径
- bin 1264 → 1266 = **+2**（error.rs into_block_retry +1；dns.rs 净 +1 = 新增 3 − 删除 1）；既有测试零删除（dns 2 例改语义、其余仅换行格式化）。
- v115 1169 → 1171 = +2（dns.rs 内嵌测试随源文件带入套件，同 bin 口径）；g_engine 1014 → 1016 = +2（同上）；engine_backfill 1146 → 1148 = +2（同上）；g_v147 1001 → 1003 = +2（同上）。五套件 +2 同源 = dns.rs 测试净增 2，零逻辑新增。
- hardening 改写面：g_v147 failure_freezes_all_worker_progress 断言文本与触发器按 D93 改写（QA v1.63 授权条款），用例数不变；v115 speed_window 仅隔离目录修复，断言不变。
- e2e 抖动登记见遗留受限项⑦；五套件最终口径均全绿复跑确认。

### 待办与待批（未决项）
- **本轮三项实现定义值（DNS 语义/probe 时序/ConnView 门控/4xx 分类）可回退窗口开放**：操作者否决 = 直接以裁决文本 `bin/swarm begin six-pack/specifier`。
- **QA 轮接手面**：遗留受限项② 全清单 + ④ qa/09:70 顺带修复；新增建议回归口径 = `cargo test --bin ezr` + hardening_g_engine/g_v147/v115 + 操作者线下 MDM 对表（受限项⑤）。
- **族C ui 剥头副本重新生成**：归 cleaner 轮顺带。**blocks 引擎代码本体退役/瘦身**：归 cleaner/architect 轮裁决。**变异/CRAP/DRY 度量**：沿袭，归 cleaner/architect 轮。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`（沿路径 coder → cleaner → architect → (hardender) → QA）：本轮无大审计需求，cleaner 顺带族C 剥头副本重生成即可。
- QA 轮接手时：QA-RB（retry-backoff）族判据与本轮 D93 收口对表（worker 块请求面 4xx 原地重试现按真实路径成立，建议补「stub 全 403 → 原地耗尽任务级 Transient」的端到端 QA 判据——本轮仅 hardening 面覆盖）；其余见待办节。
- 操作者对本轮任一实现定义值改判：直接以裁决文本 `bin/swarm begin six-pack/specifier`（三层同步口径已建立，改判成本低）。

By coder.
