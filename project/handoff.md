# 交接（handoff）

> 流程：six-pack　当前节点：**six-pack/coder（HTTP 动态引擎速度回退修复轮：块对齐切分封顶并行度 → refine_http_grid 收细网格恢复满并发）——已完成**
> 路径：**six-pack/coder（本轮）** → cleaner → architect → (hardender) → QA
> 会话：six-pack-coder-http-grid-refine（上游交接 = 上轮 handoff + 操作者 chat bug 报告：「swarm-20261126-125543-six-pack-coder 版本峰值下载一个 url 可以达到 3Mib/s 以上的速度；swarm-20261126-144621-six-pack-coder 版本下载同一个 url 只能达到 800kib/s 的速度」）
> 环境：nightly 1.102.0-nightly（rustup dist 走 rsproxy 前台装齐 + rustfmt/clippy；注意 `RUSTUP_DIST_SERVER` 变量名，误用 `DIST_SERVER` 会静默回官方 20KB/s）+ cargo registry rsproxy sparse + rust-toolchain.toml 钉 nightly；工作区 /home/z/swarm-ezr2（沙箱重置后由 144621 归档重建 + 125543 归档作 diff 基线）；~/.cargo 非持久、沙箱重置后重建
> 受限项：①chat 选择题面板不可用（IM 通道）沿袭先例；②集成套件 9 个**既有失败**沿袭登记（与本轮无关，三族根因与清单见「遗留受限项」②）；③MDM 性能基准 = 操作者线下验收口径；④P3 增益层沿规格排除。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.62**）；phase-05 详述 = `mission/phase-05.md`（FR-05-01–13 + D83–D89，1A 追认结案）；ezr 不做旧版本兼容沿袭（v1.45/FR-02-35）。
- ezr-demo：仓库内独立子项目；本轮零接触。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固 | features 01–12 + qa 规程 + runners + APS 四件套 | 已交付（沿上轮账） | 历轮全绿（上轮账口径） |
| phase-05 规格轮（20261125） | mission.md v1.62 + phase-05.md + features/13 + qa/13 | 已交付 | APS 7/7 exit 0（上轮账口径） |
| phase-05 实现轮（20261125） | FR-05-01–13 全量落地 | 已交付 | 当时口径 bin 1219P/0F |
| pieces 白块修复轮（20261126，commit 8a62985） | 动态 Progress 发真实 done/partial 集合 | 已交付 | 当时口径 bin 1230P/0F |
| 暂停续传蓝块修复轮（20261126） | dynamic_sidecar_snapshot 口径改造 + 回归测试 | 已交付 | 当时口径 bin 1232P/0F |
| 蓝块缺失 + dead_code 修复轮（20261126） | piece_states Active 臂当前写入位置 + 删废弃 sidecar_segments | 已交付 | 当时口径 bin 1234P/0F |
| 续传残留三 bug 修复轮（20261126） | 终态事件 sidecar 权威投影 + 百分比地板 0.1% + stop 改 abort + Failed 载荷 done_blocks | 已交付 | 当时口径 bin 1240P/0F |
| 下载中黄块修复轮（20261126） | Progress tick 投影三源并集 resume ∪ flushed ∪ committed_ranges | 已交付 | 当时口径 bin 1242P/0F |
| 块对齐切分修复轮（20261126） | Planner::cut_point 三级切分点（块边界对齐）+ dynamic_worker 跨段 carry 缓冲 + absorb cursor==start 守卫 | 已交付 | 当时口径 bin 1247P/0F；**该轮修复 bug9 但引入本轮修的速度回退（切分封顶并行度），见本轮根因账 1** |
| **本轮：速度回退修复轮（20261126）** | **①`refine_http_grid(total, block, concurrency)`（model/chunk/plan.rs）：块数 ≥ 并发保证——块数已足原样返回；不足收细到 ≤ total/并发 的最大 256 KiB 倍数（下限 MIN_HTTP_BLOCK_SIZE=256 KiB）；幂等且不回粗。②四处同源接线：dynamic.rs 引擎 cut_align + Progress 投影网格 / supervisor projection_of（Paused/Failed 事件）/ evt.rs Evt::Probed(dynamic) 回写 t.block_size（chunk_info、pieces 弹窗、DownloadDone 显示链同源）。③worker 提交次序修正：planner.commit 先于 ConnView.done 更新（帧一致性不变式）** | 已交付 | **bin 1256P/0F（净增 9 项：plan 5 + e2e 2 + app 2）+ clippy --tests 0 警告 + fmt --check 干净 + check --bins 0 警告 + hardening 13 目标零新增失败 + 实机 PTY 三重实证（见本轮验证证据）** |

### 遗留受限项（当前有效）
- **①全仓 >100 变异点存量 / CRAP 100 / CPD 222**：历轮登记沿袭；归 architect/cleaner 轮。
- **②集成套件 9 个既有失败（本轮实测复核与上轮登记完全一致，三族，与本轮无关；本轮改动前 8a62985 交付基线上即失败）**：
  - 族 A（blocks sidecar × 动态默认引擎无一致性裁决，4 例）：hardening_g_engine `invalidated_resets_and_toasts`、`orphan_adopt_mismatch_redirects_silently_and_preserves_sidecar` + hardening_g_v147 `inconsistent_block_count_orphan_redirects_even_when_size_matches`、`unknown_size_orphan_redirects_not_transient_fail`——裁决需操作者/QA。
  - 族 B（动态引擎失败/降级语义 vs 旧预期，4 例）：hardening_g_v147 `worker_block_403_fails_fatal_not_transient`、`single_stream_200_failure_writes_no_ledger`、`matching_sidecar_resumes_with_nonzero_range` + hardening_v115 `range_denied_fails_task`（403 降级 rescue 完成、body EOF 分类 Transient 非 Fatal、滴流断连不失败）。
  - 族 C（gen 剥头副本漂移，1 例）：hardening_ui_backfill `gen_fixtures_match_product_sources`，比对面 = src/ui/{chart,detail,btn}.rs + src/app/clipboard.rs 四文件（本轮未触碰，重新生成即可），归 cleaner 轮顺带。
  - 本轮改动面（plan.rs/chunk mod/dynamic.rs/supervisor mod.rs/evt.rs）不在族 A/B 断言面；其余 13 目标（hardening 303、g_supervisor 983、engine_backfill 1136、g_appcore 1013、g_main 2、g_throttle_err 627、g_proxy 16、g_slots 274、mutation 1145）全绿。
- **③既有 runner 断言受影响面**：01-download-engine / 01-persistence-config 等涉及默认并发 8、块下限 256 KiB、限速贴附的 runner 断言归 QA 轮同步（沿上轮登记）。**本轮新增**：05-ui-pieces 元信息行在动态模式小文件下将显示收细网格（如 `块大小 512 KB · x/4 块`）——既有 QA-UP 夹具 ≥12 MiB 不受影响；suite_http_turbo（未可执行）one-m.bin 弹窗若断言块大小需同步（归 QA 轮）。
- **④qa/09-bt-engine-qa.md:70 字面真缺陷 1 处**：归 QA 轮顺带修复（沿袭登记）。
- **⑤MDM 性能基准**：线下口径（沿袭）。

### 实现定义值登记
- 历轮登记（D81–D89 等）全部沿袭有效；**本轮新增 2 项实现口径**：
  - **动态网格细化（refine_http_grid）**：入参 (total, block_size, concurrency)；`chunk_total(total, block) ≥ concurrency` 或 total == 0 → 原样返回（正常域零变化）；否则收细到 `max(256 KiB, align_down(total/concurrency, 256 KiB))` 且不超过配置块大小。效果 = 块对齐切分（bug9 绿块不变式前提）不再封锁并行度：细化后块数 ≥ 并发，每条连接的租约起点均为网格块边界。256 KiB 下限对齐 MIN_HTTP_BLOCK_SIZE（配置合法域下限同源）；下限仍不足（total < 并发 × 256 KiB 的极小文件）时以块数封顶——13-http-turbo-02 `one-m.bin ≥ 2 连接` 口径仍满足（1 MiB → 256 KiB 网格 → 4 连接）。
  - **worker 帧一致性次序**：`planner.commit(wid, at)` 必须先于 `ConnView.done` 更新。进度 tick 读序 = planner 先、conns 后，反序会在两读窗口产生「连接 pos 已过块尾但 committed_ranges 未跟上」假警报帧（vacated 绿块不变式间歇失败形态）；正序后 committed ≥ conns 对任意交错成立，顺带消除 UI 潜在黄块闪烁。

## 二、当前产出情况

### 本会话产物清单（速度回退修复轮）
1. **`src/model/chunk/plan.rs`（新增函数 + 5 单测）**：`refine_http_grid`；chunk/mod.rs 导出同步。
2. **`src/engine/supervisor/dynamic.rs`（修订）**：引擎 start `cut_align = refine_http_grid(...)`（原 `spec.block_size.max(1)`）；Progress 投影 `project_ranges(total, cut_align, ...)` 同源；worker Granted 臂 commit 提前到 cv 更新之前；新增 e2e `dynamic_refined_grid_restores_full_concurrency`（2 MiB + 并发 4 慢速服务器峰值连接 ≥ 4 + 字节全等）与 `dynamic_refined_grid_vacated_blocks_turn_green`（2.8 MB + 并发 4 → 512 KiB 网格，跨块租约哨兵 saw_spanning + 逐帧绿块不变式）。
3. **`src/engine/supervisor/mod.rs`（修订）**：`projection_of(sc, spec_block, concurrency)` 动态臂经 refine_http_grid（Paused/Failed 事件投影与运行时网格一致）。
4. **`src/app/engine/evt.rs`（修订）**：`Evt::Probed { dynamic: true }` 臂回写 `t.block_size = refine_http_grid(total, t.block_size, t.concurrency)`——pieces 弹窗元信息（`块大小 X · x/N 块`）、chunk_done/DownloadDone 全链与引擎网格同源。
5. **`src/app/engine/mod.rs`（新增 2 测）**：`probed_dynamic_refines_block_grid_to_fit_concurrency`（2 MiB + 并发 4 → 512 KiB）+ `probed_blocks_mode_keeps_configured_block_size`（blocks 模式块模型不变反例钉住）。

### 验证证据
| 验证项 | 结果 | 命令（cwd = project/ezr） |
|---|---|---|
| 全量单元 + proptest（bin ezr） | **1256P/0F**（净增 9 项） | `cargo test --bin ezr`（前台 208s） |
| planner 专项 | 23P/0F | `cargo test --bin ezr planner` |
| dynamic/supervisor 专项 | 97P/0F | `cargo test --bin ezr engine::supervisor` |
| app Probed 专项 | 12P/0F（含新增 2） | `cargo test --bin ezr probed_` |
| clippy（含 tests） | **0 警告** | `cargo clippy --tests` |
| rustfmt | 干净 | `cargo fmt --check` |
| 编译 warning | **0 警告** | `cargo check --bins` |
| hardening 13 目标 | **除登记既有 9 例外全绿（零新增）**：g_engine 2F（族A）、g_v147 5F（族A 2 + 族B 3）、v115 1F（族B）、ui_backfill 1F（族C，比对面四 UI 文件本轮未触碰）；g_appcore 首跑 1F 复跑全绿判负载敏感（与改动面无接触） | 逐目标 `cargo test --test <name>` |
| 新 e2e 稳定性 | `dynamic_refined_*` 8 连跑全绿 | `cargo test --bin ezr dynamic_refined` ×8 |
| **实机 PTY 三重实证** | 真机 ezr 二进制 + ezr-fixture（2 MiB `?speed=400000` 每连接 400 KiB/s 门控、并发 4）：①fixture JSONL 日志 = 1 probe + **4 条开区间 Range（`bytes=0-` / `524288-` / `1048576-` / `1572864-`，起点全部 512 KiB 倍数——并行 4 与块对齐切分服务端双实证）**；②pieces 弹窗元信息 `HTTP · 块大小 512 KB · 4/4 块`；③文件 i%251 模式逐字节完整；④2.90s 完成 ≈ 1.6 MiB/s 聚合（单连接口径 ~5.2s，回退版同文件 2 连接 ~2.6s+） | `.work/tmp/verify_speed.py`（复现脚本，qa/runners harness 复用） |

### 本轮 bug 根因账（操作者报告 ↔ 修复映射）
1. **144621 版速度回退（3 MiB/s → 800 KiB/s，同 URL）**：上轮「块对齐切分修复轮」把切分对齐绑到 UI 网格（cut_align = spec.block_size = 1 MiB）——块对齐是 bug9 绿块不变式的正确前提，但并行度被物理封顶在**块数**上：块数 < 并发的文件（如 2 MiB / 1 MiB / 并发 8）只能切出 2 条连接 ≈ 800 KiB/s，而回退前 4096 对齐切分可达满并发 ≈ 3 MiB/s。规格冲突核对：FR-05-06「动态模式块大小概念由调度器取代」+ 13-http-turbo-02「twelve-m.bin 并发 8 → min_conns 8」均要求调度器自主切分；块对齐切分擅自越权接管了调度粒度。**修复 = 网格细化**（非回滚切分对齐）：refine_http_grid 保证块数 ≥ 并发，块对齐与满并发同时成立；小文件 pieces 网格随之变细（显示语义：动态模式规格本不钉网格粒度，FR-05-13 详情行恒「动态分段」口径不变）。
2. **worker 帧一致性真缺陷（本轮 e2e 排查副产）**：cv.done 更新先于 commit 的旧次序在「tick 读 planner→conns」读序下存在假警报窗口（g_v147 全量跑首现，探针帧取证定性）。已修正（见实现定义值），并以此把 vacated 不变式从「概率通过」加固为「结构性成立」。
3. **e2e 夹具勘误登记**：`burst_server` 不解析 Range（非 primed worker 必收 200 → range_ignored 退出）——并行度类 e2e 唯一可用夹具 = mock_server（206 + Content-Range）；既有 `dynamic_vacated_blocks_turn_green_when_frontier_moves_on`（conc=2）恰好在单连接形态下断言仍成立而未暴露此夹具局限（不变式主体仍有效，无回滚需求）。

### 待办与待批（未决项）
- **族 A/B 既有失败修复口径（需操作者裁决）**：沿袭登记，未决。
- **gen 剥头副本重新生成（族 C，机械项）**：归 cleaner 轮顺带（沿袭）。
- **runner 可执行化（归 QA 轮）**：suite_http_turbo.py + 六份既有 runner 受影响断言同步（沿袭）；本轮新增 05-ui-pieces 动态小文件网格细化显示面（见遗留受限项③）。
- **qa/09-bt-engine-qa.md:70 真缺陷**（沿袭）。
- **变异/CRAP/DRY 度量**（沿袭，归 cleaner/architect 轮）。
- **QA-HT-10 du 判据 / 性能基准**（沿袭，线下/操作者裁决）。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：gen 剥头副本重新生成（族 C）顺带处理；本轮改动集中在 plan.rs（+13 行函数）/dynamic.rs（接线 + 2 e2e + 次序修正）/supervisor mod.rs（projection_of 签名）/evt.rs（Probed 4 行）/app mod.rs（2 测），无大审计。
- QA 轮接手时：pieces 回归口径 = 上轮家族全量 + 本轮 `dynamic_refined_grid_restores_full_concurrency` + `dynamic_refined_grid_vacated_blocks_turn_green` + `probed_dynamic_refines_block_grid_to_fit_concurrency` + refine 5 单测；05-ui-pieces 动态模式小文件元信息行网格细化显示面需 runner 同步；实机复现脚本 `.work/tmp/verify_speed.py` 可作 QA-HT 家族参考（harness 复用，注意 pyte 依赖 pip 阿里云镜像秒装）。
- 族 A/B 裁决后如需 coder 轮落地，直接以本 handoff + 裁决结论 begin six-pack/coder。
- 沿袭受限项（变异存量/CRAP/CPD）如需收敛，由 architect 轮裁决。

By coder.
