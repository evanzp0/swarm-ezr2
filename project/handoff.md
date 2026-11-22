# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（**竞态销项轮**——操作者 chat 指令销 notes/architecture.md:389 观察项）
> 路径：…遗留受限项销项轮（上轮）⇒ **竞态销项轮（本轮，操作者 chat 指令）**
> 会话：six-pack-coder-20261122-racefix（SESSION_TASK = block_worker 打开失败单发 fatal 与 resume 作废重建/重试路径理论竞态销项；上游交接 = 上轮 handoff + 操作者 chat 指令；ezr 不做旧版本兼容）
> 环境：rustup stable 1.100.0 + nightly 1.102.0（TUNA 镜像 0.08s 最快、minimal profile；rust-toolchain.toml 钉 nightly 全项目统一）、cargo registry rsproxy sparse、CARGO_INCREMENTAL=0 + CARGO_PROFILE_DEV_DEBUG=0 + CARGO_PROFILE_TEST_DEBUG=0 三件套全程前台分块。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.57**）。ezr 不做旧版本兼容。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同，见 11-trackers-window v1.14 注记）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| QA runtime 重建轮（上上轮） | APS 四件套重建 + verdict 级全量 + 07-add-bt 哈希销项 | 已交付 | verdict Σ = 1107 killed + 17 filtered + 0S/0E；20/20 基线 0F/0E |
| 遗留受限项销项轮（上轮） | ③644 改判杀灭 + ①缺口账目 ID 权威清单 + ④LO_ONLY 敞口消灭（schema 口径演进）+ ②Go 版 mutator filter 移植 | 已交付 | verdict Σ = 1107 killed + 0 survived + 0 errors + 17 filtered（历轮口径精确复现）；bin 基线 1091P/0F 全绿；fmt/clippy clean；go test 全绿 + 17/17 |
| **竞态销项轮（本轮）** | architecture:389 观察项销项：block_worker 打开失败有界重试窗（workers.rs）+ 两行为锁定单测（supervisor/mod.rs tests） | 已交付 | 4 bin 单测 1147P/0F（ezr 1093 = 基线 1091 + 本轮 2）+ 13 集成目标 8440P/0F = **9587P/0F 全绿**；fmt clean；clippy --all-targets 零警告 |

### 遗留受限项（当前有效）
- architecture:389「supervisor block_worker 打开目标文件失败即单发 fatal 与 resume 作废重建/重试路径理论竞态」——**本轮销项结清**（见二节产物 1；条目已改写为销项定案，notes/architecture.md）。
- 2 条 unviable 终态（mutation 账目人工实证定案）+ trackpool.rs:412 CRAP 竞速对账口径（已接受，非可执行任务）沿历轮登记不变。
- 上轮 QA 登记的 schema 口径演进（open_int 三分退役为二分）维持交付态，操作者否决路径仍留档（notes/testing.md 锚回补轮先例）。

## 二、当前产出情况

### 本会话产物清单（竞态销项轮）
1. **engine 修复（唯一行为改动面 = workers.rs）**：block_worker 打开目标文件由一次性 open→fatal 改为 `open_target_file` 有界重试窗——仅 `ErrorKind::NotFound`（ENOENT，即登记竞态的交错重建/重试缺位形态）在 1s 窗（OPEN_RETRY_WINDOW）内 25ms（OPEN_RETRY_POLL）轮询重开；非 NotFound（权限等永久错误）立即原样返回致命；窗耗尽返回最后一次 ENOENT，调用点保持原 fatal 语义与文案「无法写入目标文件」（不吞永久错、失败面口径不变）。登记修法中的「supervisor 先 touch 再派 worker」核对为既有事实（download.rs create+set_len 先于 spawn），无需新增；重试窗补的是文件短暂缺位窗的残余时序（含 fill_workers 运行中 re-spawn 臂）。
2. **行为锁定双测（supervisor/mod.rs tests，TDD 先红后绿）**：`worker_open_retry_recovers_from_interleaved_rebuild`（目标文件派生后 100ms 才出现 → 窗内恢复、完成唯一块、无 fatal、落盘字节与内容模式断言）+ `worker_open_permanent_absence_still_fatal_after_window`（恒缺位 → 窗耗尽仍 Fatal「无法写入目标文件」、零写入）。红灯实证：修复前恢复测 panic「交错重建窗内 ENOENT 不得误 Fatal」。
3. **经验沉淀**：`notes/architecture.md` 389 条目改写为销项定案（保留竞态描述 + 修复口径 + 行为锁定双测指针）；`notes/testing.md` 追加竞态销项轮 2 条（worker 级 WorkerDeps 夹具 stop watch sender 保活纪律——drop 即 changed() Err 被当停止信号无声中止；TDD 红灯归因 panic 文本逐行比对与恢复测/守卫测配对）；`notes/INDEX.md` 同步 + 本 handoff.md 重写。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 新增单测 | 2P/0F（含 ~1s 守卫测窗耗尽路径） | `cargo test --bin ezr worker_open` |
| bin 单测全量 | 1093P/0F（ezr）+ 30/9/15P（fixture/mktorrent/proxy） | `cargo test --bins`（DEBUG 三件套） |
| 集成全量 | 13 目标 8440P/0F（hardening 247 / engine_backfill 979 / ui_backfill 1009 / g_appcore 863 / g_engine 859 / g_main 2 / g_proxy 16 / g_slots 254 / g_supervisor 832 / g_throttle_err 543 / g_v147 846 / mutation 988 / v115 1002） | `cargo test --test <目标>`（逐块前台） |
| fmt / clippy | clean / 零警告 | `cargo fmt --check && cargo clippy --all-targets` |
| 产品基线对账 | ezr 1091 基线全数保持 + 本轮 +2 | 上两行合计口径 |

### 对账口径
- **行为改动面最小化**：产品源码仅 workers.rs 一处（常量对 + open_target_file + 调用点替换），single.rs（自建文件臂）与 download.rs（预建/预分配臂）零改动——登记项原文即「workers.rs:93-103 一次性 open→fatal」，销项以该面为界。
- **测试基建同步**：supervisor/mod.rs tests 新增 worker_deps_for 夹具（返回 WorkerDeps + stop_tx）；既有 1091 单测零触碰零翻转（全量 0F 为证）。
- **工作区零残留**：临时探针（.work/tmp/probe）随 complete 清理；本轮无 features/qa 产物改动（登记项为引擎观察项，非规格面）。

### 待办与待批
- **本会话无待批无待办**——操作者指令的销项已落地结清。

### 交接四要素
1. **本地验证命令清单（按序）**：`cargo test --bin ezr worker_open`（2P）→ `cargo test --bins`（1147P）→ 逐集成目标（8440P）→ `cargo fmt --check` + `cargo clippy --all-targets`。
2. **环境**：rust-toolchain.toml 钉 nightly（fmt/clippy 需其 rustfmt unstable 选项），测试运行兼容 stable/nightly 双通道（本轮 bin 全量在 nightly 跑绿）；cargo registry = rsproxy sparse（~/.cargo/config.toml）。
3. **行为不变式（下游注意）**：worker 打开失败的重试窗只认 ENOENT——若未来目标文件缺位需区分其他可恢复错误类别，扩 open_target_file 的错误判定面而非放宽窗口；fatal 文案「无法写入目标文件」被 UI/QA 语境引用，改文案先对账消费点。
4. **夹具纪律**：worker 级测试构造 WorkerDeps 时 stop watch sender 必须保活到断言之后（见 notes/testing.md 竞态销项轮条目）。

### 移交建议
- 下一步建议运行 `six-pack/cleaner` 技能（coder SKILL 标准移交路径）。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。

By coder.
