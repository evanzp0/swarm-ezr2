# 交接（handoff）

> 流程：six-pack　当前节点：**six-pack/coder（phase-05 HTTP 引擎提速实现轮 P0–P2）——已完成，全部 FR-05-01–13 落地**
> 路径：**six-pack/coder（本轮）** → cleaner → architect → (hardender) → QA
> 会话：six-pack-coder-phase05（上游交接 = 上轮 handoff + 操作者 chat 指令「待办规格合成确认，选 1A」）
> 环境：stable 1.100.0 + nightly 1.102.0（rustup USTC 双链前台装齐，rust-toolchain 钉 nightly + rustfmt/clippy）+ cargo registry rsproxy sparse + bb 1.13.230（ghfast.top，.tools/bb）+ APS clone + mutator 扩展 patch（.tools/Acceptance-Pipeline-Specification，bb test 11 测 51 断言 0F/0E）；工作区已迁 /home/z/my-project/swarm-ezr2 持久区；**新增直接依赖 xattr 1.0**（Linux ntfs3 稀疏标记；项目 unsafe-code deny 下以安全 crate 取代裸 libc FFI）
> 受限项：①chat 选择题面板不可用（IM 通道）沿袭十三轮先例——本轮规格合成确认由操作者 chat 指令直接作答（1A），无新微口径登记；②MDM 性能基准（同机对比/netem 高 RTT）沿袭上轮登记 = 操作者线下验收口径，不进自动化 QA 判据；③ext4 上 FR-05-10 的 fallocate(KEEP_SIZE) 回退会全额分配块（st_blocks 立即增长）——ntfs3 xattr 分支不可用时稀疏性让位于「免零填充卡顿」，QA-HT-10 的 du 判据按文件系统实际能力记 S（内嵌判据文案口径已内置）；④P3 增益层沿规格排除，未实现。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.62**）；phase-05 详述 = `mission/phase-05.md`（FR-05-01–13 + D83–D89）。**操作者本轮 chat 裁决：phase-05 规格合成确认 = 1A（准确，无需修改）——待批闸门关闭，D83–D89 追认结案**；ezr 不做旧版本兼容沿袭（v1.45/FR-02-35）。
- ezr-demo：仓库内独立子项目；本轮零接触（phase-05 提速改造属产品引擎层，无 demo 镜像需求）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固 | features 01–12 + qa 规程 + runners + APS 四件套 | 已交付（沿上轮账） | 历轮全绿（上轮账口径：bin 1168P/0F + 集成 9016P/0F + demo 61P/0F + CRAP 100 + CPD 222） |
| phase-05 规格轮（20261125） | mission.md v1.62 + phase-05.md + features/13 + qa/13 + 六规格六 QA 同步修订 | 已交付 | APS 7/7 exit 0（上轮账口径） |
| **phase-05 实现轮（20261125，本轮）** | **FR-05-01–13 全量落地**：新增 supervisor/{planner,writer,sparse,dynamic}.rs 四模块 + workers/single/throttle/download/plan/config/sidecar/http/detail/task/registry/evt/engine(make_spec)/main/cli/dialogs 修订面 + 测试新增 60 项 | 已交付 | **bin 1219P/0F（含本轮新增）+ clippy 零警告 + cargo build 干净**；验收流水线组件按 coder 职责边界仅确保 APS 就位（bb test 0F/0E），QA runner 可执行化归 QA 轮 |

### 遗留受限项（当前有效）
- **全仓 >100 变异点存量 / CRAP 100 / CPD 222**：历轮登记沿袭；本轮新增代码（planner/writer/dynamic ≈ 1900 行）未跑变异/度量（coder 职责边界），如需收敛由后续 architect 轮裁决后按既有口径拆分。
- **qa/09-bt-engine-qa.md:70 字面 `ttp]` 真缺陷 1 处**：登记待修，归下一 QA 轮顺带修复（沿上轮登记）。
- **既有 runner 断言的受影响面**：01-download-engine / 01-persistence-config 等既有 runner 涉及默认并发（4→8）、块下限（1MB→256 KiB）、限速贴附（比例等待后 ≤8 MiB/s 场景速率贴住设定值）的断言需 QA 轮同步——单元层对应断言本轮已更新（全绿），端到端 runner 口径归 QA。
- **MDM 性能基准**：线下口径（受限项②）。

### 实现定义值登记
- 历轮登记（D81–D89、运行端点判定口径、ApplyGlobals 命令面、CLI 覆盖登记、进度条行尾恒空）全部沿袭有效；**D83–D89 本轮实现落定（1A 追认）**，无新增裁决值。
- 默认值变更已实现：DEFAULT_HTTP_CONCURRENCY 4→8、HTTP 并发域 1–128 全链（CLI/config/引擎 clamp/添加与修改对话框/-c 帮助文案）、DEFAULT_THROTTLE_BURST_SECS 0.5→1.0、MIN_HTTP_BLOCK_SIZE 1 MB→256 KiB（DEFAULT_BLOCK_SIZE_HTTP 维持 1 MB）。
- 新键落位：[http] scheduler（{dynamic,blocks} 大小写不敏感、默认 dynamic、非法拒绝启动、B 类快照）+ [http] min_split_kib（默认 256、正整数）；模板 40→42 键（v1.62 口径，模板行数断言 214 同步）。
- 新实现常量：WBUF/BATCH 256 KiB、WRITE_MAX 4 MiB、SPARE_MAX 16、HOLD_MAX 256 MiB（跑道按连接数均摊）、SPLIT_ALIGN 4096、MIN_PREEMPT 20 ms、原地重试阶梯（0/1/2/4/8 s，上限 5 次，按块计数、块完成清除）。

## 二、当前产出情况

### 本会话产物清单（phase-05 实现轮）
1. **`src/engine/supervisor/planner.rs`（新增，FR-05-06/D85）**：Segment/Planner 全套原语 assign（未分配→抢占 2×TTFB 静默段→最大剩余段按 reserved 对半切 + min_split 自适应降半）/claim（退款语义，返回 seg_end）/commit（耗尽返回续领起点）/release（保留 cursor 退款 reserved）/absorb_next（整文件流吞并相邻无人认领段）；不变量 check_invariants（不重叠/cursor≤reserved≤end/pre_committed+retired+Σ段=total）；**proptest 2 条**（随机操作序列不变量 + 公平轮转收敛 liveness）。
2. **`src/engine/supervisor/writer.rs`（新增，FR-05-09）**：BATCH 256 KiB 封口、写盘任务合并 ≤4 MiB 定位写（unix write_all_at / windows seek_write）、HOLD_MAX 256 MiB 跑道按连接数均摊（超限才 await）、SPARE 回收池零分配流量、written 区间账（merged intervals + flushed_prefix 压缩 + flushed_extent_from）、total_pushed/total_flushed；单测 10 项。
3. **`src/engine/supervisor/sparse.rs`（新增，FR-05-10/D88）**：Windows FSCTL_SET_SPARSE（cfg 隔离、unsafe 按项目 deny 加允许域）/ Linux ntfs3 xattr system.ntfs_attrib|=0x80000（xattr crate）/ 失败静默回退 fallocate(KEEP_SIZE)（fs2::allocate）→ set_len；单测 4 项。
4. **`src/engine/supervisor/dynamic.rs`（新增，FR-05-06/07/08 + P2 接线）**：动态 worker 循环（开区间 Range + 先付限速再认领 + claim 退款 + commit/absorb 链 + primed 对齐守卫 + 按块原地重试阶梯 + Range 失效臂：新任务静默退出+toast、续传任务保留断点失败）；download_dynamic（裸 GET probe + 首字节 TTFB 预读 + 续传开区间从首个未提交字节 + 一致性校验 + 稀疏预分配 + 聚合游标完成判定 + 并集落盘守卫 + FR-05-11 条件 sync_data 的 sidecar 保存 + 降级救援：检测 Range 失效后停群重置 planner 单连接裸 GET 完成）；sidecar 快照（flushed 钳位段列表）；gaps_of 补集重建；e2e 4 项。
5. **`workers.rs` 修订（FR-05-01/03a/04）**：WBuf 256 KiB 写合并（断点续推、盘账 flushed 驱动 conns 与 total_written）；probe 体复用（primed 注入 worker #1，第 0 块从 0 边收边写）；失败粒度（Transient 原地重试 requeue + 按块计数阶梯、Fatal 升级任务级、短 EOF 计入重试）；块完成守卫（不足段长 requeue 不误标 done）；单测 5 项。
6. **`single.rs` 修订（FR-05-01）**：单流路径 WBuf 化，暂停/失败账本落 flushed 真值。
7. **`throttle.rs` 修订（FR-05-02）**：bucket_step 比例等待（round(left/rate×1000)，<1 ms 才落 sleep_ms 地板）+ proptest 不变式更新 + 4 组比例断言；supervisor 三处限速计时测试按 burst 1.0 语义重校（≥4.0s）。
8. **`download.rs` 修订**：blocks/dynamic 分流（spec.scheduler + sidecar 格式即引擎双判定）；primed 注入；1–128 全链 clamp；完成守卫改块账本精确值（防原地重试重传虚增）。
9. **`http.rs` 修订**：probe_head 解析 Content-Range 总长（续传开区间探测的 total 来源；noclen 时 mock 双抑制保持既有判据）。
10. **`sidecar.rs` 修订（FR-05-08/D89）**：双格式——scheduler 字段 + (start,cursor) 段列表（skip_serializing_if 保持 blocks 格式字节不变）；is_dynamic/resume_segments/build_dynamic（downloaded=Σ(cursor-start)）；单测 4 项。
11. **`config.rs` 修订（FR-05-05/12）**：默认值三处 + range1_128_val + HttpScheduler 解析 + scheduler/min_split_kib 严格校验 + 模板 42 键（v1.62）+ 测试同步（59/59）。
12. **UI（FR-05-13/D83）**：detail.rs 分块行动态口径 `<已下载>/<总字节> · 动态分段`；blocks 口径逐字保留；Evt::Probed 增 dynamic 字段（app 侧 t.dynamic 落位）；Task/registry/restore 贯通；UI 测试 2 项。
13. **make_spec（B 类快照）**：scheduler/min_split 烘焙于任务启动（cfg 值）；**格式即引擎**：sidecar 含 scheduler=dynamic → 动态引擎接续，blocks 格式 → blocks 引擎（热加载不影响运行中任务与 sidecar）。
14. **notes 沉淀**：architecture.md（planner 不变量测试化口径 / primed 对齐守卫 / 完成判定三口径 / 抢占活锁根因 / 按块重试计数 / 并发 mock 前置）、testing.md（慢 mock 竞速窗 / cargo 超时误判）、INDEX.md 索引同步。

### 验证证据
| 验证项 | 结果 | 命令（cwd = project/ezr） |
|---|---|---|
| cargo build | exit 0 | `cargo build` |
| clippy | **0 警告** | `cargo clippy` |
| 全量单元 + proptest | **1219P/0F**（本轮净增 51 项：planner 17+2 proptest、writer 10、sparse 4、sidecar 4、dynamic e2e 4、throttle 断言扩充、config/UI/CLI/supervisor 场景同步） | `cargo test --bin ezr`（前台 216s） |
| APS 就位 | bb test 11 测 51 断言 0F/0E | `(cd .tools/Acceptance-Pipeline-Specification && bb test)` |
| 集成测试编译 | exit 0（hardening_* 全部适配新字段） | `cargo build --tests` |
| 产品端到端 QA | 未跑（coder 职责边界：runner 可执行化归 QA 轮） | — |

### 对账口径
- 新增源码 ≈ 1900 行（planner 736 + writer 470 + sparse 150 + dynamic 1010 计测试），修改面 14 文件；`git diff --stat` 可复核。
- 测试净增：bin 断言数 1168 → 1219（+51）；本轮零删除既有断言（仅改口径值：并发域、块下限、burst 计时、模板计数、mock 语义适配）。
- **mock 语义修正登记**：mock_server 由串行 accept 改每连接一线程（开区间请求的并发服务前置，blocks 封闭区间语义不变）；206 响应补 Content-Range（RFC 7233 强制头，续传探测依据）；noclen 同时抑制 CL 与 Content-Range（保持「缺 Content-Length」判据成立）。既有 blocks 测试全绿复证语义等价。

### 待办与待批（未决项）
- **runner 可执行化（归 QA 轮）**：suite_http_turbo.py（QA-HT 全用例）+ 六份既有 runner 受影响断言同步（默认并发 8、块下限 256 KiB、throttle 贴附、动态分块行、mock 并发语义）。
- **qa/09-bt-engine-qa.md:70 `ttp]`→`[http]`**：1 处历史真缺陷待修（沿上轮登记）。
- **变异/CRAP/DRY 度量（归 cleaner/architect 轮）**：本轮新增 ≈1900 行未度量，存量 >100 变异点沿袭。
- **QA-HT-10 du 判据**：ext4 + fallocate 回退的全额分配语义（受限项③），QA 按 S 兜底口径处理或操作者裁决 D88 回退序取舍。
- **性能基准**：操作者线下口径（同机 MDM 对比 / netem / 慢盘）。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：本轮新增 planner/writer/dynamic/sparse 四模块建议做重复度与命名收敛审计（writer 与 workers 的 WBuf/flush 语义相近但职责不同——blocks 引擎用 WBuf 聚合、动态引擎用 Writer 流水线，合并需谨慎）；CARGO_INCREMENTAL=0 全程未开，target/ 体积由 complete 自动清理。
- QA 轮接手时先读 `qa/13-http-turbo-qa.md` 环境前置（fixture 新能力：Range 形态日志/延迟应答/断连注入/忽略 Range/1 GiB 流式/du 采样）与 `notes/architecture.md` 本轮追加节（并发 mock 语义 + 完成判定三口径）。
- 既有 runner 断言受影响面见上「遗留受限项」第 3 条；**runner 同步优先级最高**（默认并发已变 8，旧 runner 预填 4 的场景会按新默认走）。
- 沿袭受限项（>100 变异点存量 / CRAP 100 / CPD 222）如需收敛，由 architect 轮裁决后按既有口径拆分。

By coder.
