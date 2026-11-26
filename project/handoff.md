# 交接（handoff）

> 流程：six-pack　当前节点：**six-pack/coder（phase-05 HTTP 引擎提速实现轮 P0–P2）——已完成，全部 FR-05-01–13 落地；追加轮修复操作者报告的 pieces 窗口显示 bug**
> 路径：**six-pack/coder（本轮）** → cleaner → architect → (hardender) → QA
> 会话：six-pack-coder-phase05（上游交接 = 上轮 handoff + 操作者 chat 指令「待办规格合成确认，选 1A」；追加轮 = 操作者 chat bug 报告「HTTP 任务 pieces 窗口分块显示不正确」）
> 环境：nightly 1.102.0（rustup USTC 装齐，rust-toolchain 钉 nightly + rustfmt/clippy）+ cargo registry rsproxy sparse + bb 1.13.230（.tools/bb）+ APS clone + mutator 扩展 patch（.tools/Acceptance-Pipeline-Specification）；工作区 /home/z/my-project/swarm-ezr2 持久区；**新增直接依赖 xattr 1.0**（Linux ntfs3 稀疏标记）；追加轮会话状态曾随沙箱重置丢失（.work/），按宪法第二章中断恢复条款自产物审计重建；rustup 工具链重装（~/.cargo 非持久，见 notes/environment.md）
> 受限项：①chat 选择题面板不可用（IM 通道）沿袭先例；②MDM 性能基准（同机对比/netem 高 RTT）= 操作者线下验收口径，不进自动化 QA 判据；③ext4 上 FR-05-10 的 fallocate(KEEP_SIZE) 回退会全额分配块——ntfs3 xattr 分支不可用时稀疏性让位于「免零填充卡顿」，QA-HT-10 的 du 判据按文件系统实际能力记 S（内嵌判据文案口径已内置）；④P3 增益层沿规格排除，未实现。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.62**）；phase-05 详述 = `mission/phase-05.md`（FR-05-01–13 + D83–D89）。**操作者裁决：phase-05 规格合成确认 = 1A（准确，无需修改）——待批闸门关闭，D83–D89 追认结案**；ezr 不做旧版本兼容沿袭（v1.45/FR-02-35）。
- ezr-demo：仓库内独立子项目；本轮零接触（phase-05 提速改造属产品引擎层，无 demo 镜像需求）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固 | features 01–12 + qa 规程 + runners + APS 四件套 | 已交付（沿上轮账） | 历轮全绿（上轮账口径：bin 1168P/0F + 集成 9016P/0F + demo 61P/0F + CRAP 100 + CPD 222） |
| phase-05 规格轮（20261125） | mission.md v1.62 + phase-05.md + features/13 + qa/13 + 六规格六 QA 同步修订 | 已交付 | APS 7/7 exit 0（上轮账口径） |
| phase-05 实现轮（20261125） | **FR-05-01–13 全量落地**：新增 supervisor/{planner,writer,sparse,dynamic}.rs 四模块 + workers/single/throttle/download/plan/config/sidecar/http/detail/task/registry/evt/engine(make_spec)/main/cli/dialogs 修订面 + 测试新增 60 项（明细见 notes/architecture.md 本轮追加节与 git diff） | 已交付 | 当时口径 bin 1219P/0F + clippy 零警告 + cargo build 干净；QA runner 可执行化归 QA 轮 |
| **追加轮（20261126，本轮）** | **操作者报告 bug 修复**：HTTP 任务 pieces 分块窗口只显示 🟦/⬜、已下载块全白——根因 = 动态调度器（默认）Progress 事件恒发空 done/partial 集合；修复 = Writer::flushed_ranges + model::chunk::project_ranges 投影 + dynamic.rs 发真实集合 + restore.rs 动态 sidecar 投影还原；新增测试 10 项 | 已交付 | **bin 1230P/0F + 集成 2444P/0F（合计 3674P/0F）+ clippy --tests 0 警告** |

### 遗留受限项（当前有效）
- **全仓 >100 变异点存量 / CRAP 100 / CPD 222**：历轮登记沿袭；实现轮新增代码（planner/writer/dynamic ≈ 1900 行）与追加轮新增投影代码未跑变异/度量（coder 职责边界），如需收敛由后续 architect 轮裁决后按既有口径拆分。
- **qa/09-bt-engine-qa.md:70 字面 `ttp]` 真缺陷 1 处**：登记待修，归下一 QA 轮顺带修复（沿上轮登记）。
- **既有 runner 断言的受影响面**：01-download-engine / 01-persistence-config 等既有 runner 涉及默认并发（4→8）、块下限（1MB→256 KiB）、限速贴附（比例等待后 ≤8 MiB/s 场景速率贴住设定值）的断言需 QA 轮同步——单元层对应断言已更新（全绿），端到端 runner 口径归 QA。
- **MDM 性能基准**：线下口径（受限项②）。

### 实现定义值登记
- 历轮登记（D81–D89、运行端点判定口径、ApplyGlobals 命令面、CLI 覆盖登记、进度条行尾恒空）全部沿袭有效；D83–D89 已实现落定（1A 追认），追加轮无新增裁决值。
- 默认值变更已实现：DEFAULT_HTTP_CONCURRENCY 4→8、HTTP 并发域 1–128 全链（CLI/config/引擎 clamp/添加与修改对话框/-c 帮助文案）、DEFAULT_THROTTLE_BURST_SECS 0.5→1.0、MIN_HTTP_BLOCK_SIZE 1 MB→256 KiB（DEFAULT_BLOCK_SIZE_HTTP 维持 1 MB）。
- 新键落位：[http] scheduler（{dynamic,blocks} 大小写不敏感、默认 dynamic、非法拒绝启动、B 类快照）+ [http] min_split_kib（默认 256、正整数）；模板 40→42 键（v1.62 口径，模板行数断言 214 同步）。
- 新实现常量：WBUF/BATCH 256 KiB、WRITE_MAX 4 MiB、SPARE_MAX 16、HOLD_MAX 256 MiB（跑道按连接数均摊）、SPLIT_ALIGN 4096、MIN_PREEMPT 20 ms、原地重试阶梯（0/1/2/4/8 s，上限 5 次，按块计数、块完成清除）。

## 二、当前产出情况

### 本会话产物清单（追加轮：pieces 窗口 bug 修复）
1. **`src/model/chunk/plan.rs`（修订）**：新增 `merge_byte_ranges`（升序合并重叠/相邻字节区间，防重复覆盖虚增）与 `project_ranges(total, piece, ranges)`（字节区间→分块网格投影：整块覆盖→Done、部分覆盖→Partial、零覆盖不入集合；piece 与 pieces 窗口同口径）；chunk/mod.rs 导出 project_ranges；单测 7 项。
2. **`src/engine/supervisor/writer.rs`（修订）**：新增 `flushed_ranges()`——全部已落盘字节区间（前缀 [0, flushed_prefix) ∪ 前沿后水洼 written，升序不相交）；单测 1 项；顺带清理 4 处测试未用变量警告。
3. **`src/engine/supervisor/dynamic.rs`（修订）**：download_dynamic 进度 tick 改发真实集合——`resume_ranges`（续传 sidecar 已收区间，循环前捕获）∪ `writer.flushed_ranges()` → project_ranges(total, spec.block_size, …) → Evt::Progress{chunk_done, done_blocks, partial_blocks}（原恒发 0/[]/[]）；e2e 1 项（限速 8 MiB 任务观察 done 增长与 partial 出现）。
4. **`src/app/restore.rs`（修订）**：动态 sidecar（scheduler=dynamic，无逐块台账）恢复时按任务块大小投影 resume_segments → done/partial/chunk_done——修复暂停/重启后分块窗口全白；测试 1 项。
5. **`src/engine/supervisor/planner.rs`（修订）**：cfg(test) 两方法补 #[allow(dead_code)]（hardening 测试以非 test cfg 包含本文件的既有误报）；proptest prop_map 冗余闭包清理。
6. **notes 沉淀**：architecture.md 追加「字节区间→块网格投影口径」节（投影数据源、求并语义、done=落盘口径）；INDEX.md 同步。

### 验证证据
| 验证项 | 结果 | 命令（cwd = project/ezr） |
|---|---|---|
| cargo build | exit 0 | `cargo build` |
| clippy | **0 警告**（含 --tests） | `cargo clippy --tests` |
| 全量单元 + proptest（bin ezr） | **1230P/0F**（追加轮净增 10 项：plan 7、writer 1、dynamic e2e 1、restore 1） | `cargo test --bin ezr`（前台 214s） |
| 集成测试 | **2444P/0F**（30+9+15+288+1112+990） | `cargo test`（全套合计 3674P/0F） |
| APS 就位 | bb test 11 测 51 断言 0F/0E | `(cd .tools/Acceptance-Pipeline-Specification && bb test)` |
| 产品端到端 QA | 未跑（coder 职责边界：runner 可执行化归 QA 轮） | — |

### 对账口径
- 追加轮净增测试 10 项（bin 断言数 1219 → 1230）；零删除既有断言。
- bug 根因一句话：动态调度器 Progress 事件的数据源缺失——引擎内部 planner 有完整状态，但事件层恒发空集合，UI 的 done_blocks 优先口径（非空则忽略 chunk_done 前缀）导致已下载块永不点亮。修复口径 = 「已落盘」为绿（flushed），received-but-unflushed 由连接行 🟦 表达，短暂滞后（≤256 KiB batch）可接受。
- RAG 索引随会话状态重建（185 文件/1665 分块）。

### 待办与待批（未决项）
- **runner 可执行化（归 QA 轮）**：suite_http_turbo.py（QA-HT 全用例）+ 六份既有 runner 受影响断言同步（默认并发 8、块下限 256 KiB、throttle 贴附、动态分块行、mock 并发语义）。
- **qa/09-bt-engine-qa.md:70 `ttp]`→`[http]`**：1 处历史真缺陷待修（沿上轮登记）。
- **变异/CRAP/DRY 度量（归 cleaner/architect 轮）**：实现轮 ≈1900 行 + 追加轮投影代码未度量，存量 >100 变异点沿袭。
- **QA-HT-10 du 判据**：ext4 + fallocate 回退的全额分配语义（受限项③），QA 按 S 兜底口径处理或操作者裁决 D88 回退序取舍。
- **性能基准**：操作者线下口径（同机 MDM 对比 / netem / 慢盘）。

### 移交建议
- 下一步 `bin/swarm begin six-pack/cleaner`：实现轮新增 planner/writer/dynamic/sparse 四模块建议做重复度与命名收敛审计（writer 与 workers 的 WBuf/flush 语义相近但职责不同——blocks 引擎用 WBuf 聚合、动态引擎用 Writer 流水线，合并需谨慎）；追加轮的 project_ranges 与 blocks 引擎 Blocks::done_indices/partial_indices 语义相近（字节账 vs 区间账），收敛时注意两套口径的适用面。
- QA 轮接手时先读 `qa/13-http-turbo-qa.md` 环境前置（fixture 新能力：Range 形态日志/延迟应答/断连注入/忽略 Range/1 GiB 流式/du 采样）与 `notes/architecture.md` 追加节（并发 mock 语义 + 完成判定三口径 + 投影口径）；pieces 分块窗口的回归口径以 `dynamic_progress_carries_done_and_partial_blocks` 与 `restore_dynamic_sidecar_projects_done_and_partial` 为准。
- 既有 runner 断言受影响面见上「遗留受限项」第 3 条；**runner 同步优先级最高**（默认并发已变 8，旧 runner 预填 4 的场景会按新默认走）。
- 沿袭受限项（>100 变异点存量 / CRAP 100 / CPD 222）如需收敛，由 architect 轮裁决后按既有口径拆分。

By coder.
