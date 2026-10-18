# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：v131-fix（任务 ezr-partial-yellow-fix）
> 上游交接：coder v130-fix（交接内容 = 前轮产物总账）+ 操作者
> 当轮 bug 指令：修复多并发下载「失败/暂停后，分块窗口中部分下载的分块可能显示
> ⬜（部分此类块显示 🟨 正确），应一律 🟨」。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.29，分期模式）+ `project/mission/phase-01.md`（v1.29）。
- 本轮任务 = 操作者 bug 报告（非新规格切片）：FR-01-106 pieces 分块弹窗在
  失败/暂停收尾场景的 🟨 展示缺陷，验收口径按操作者指令「部分下载的分块
  应该显示为黄色的」。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.29 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | 逐轮 parser/dry-check 通过（APS 不可用轮以人工规范化核对替代，见受限项） |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F；CPD 17→15 |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 hardening_g_v147 | 已交付 | 3363P/0F、clippy 0、fmt 0、arch 15 |
| QA（ezr-qa-final） | QA 规程可执行化 13 端到端套件（qa/runners/）+ 缺陷修复 + 双度量复测 | 已交付 | e2e 165P/0F/0S（13 套件）、3363P/0F、覆盖 94.37%、CRAP≥30=0、CPD 89/15 |
| specifier（v129-spec） | mission v1.29（FR-01-106 + FR-01-96 修订 + D35）+ features/05-ui-pieces.feature（8 场景）+ features/04-ui-conns.feature（场景 08 修订）+ qa/05-ui-pieces-qa.md + qa/04-ui-conns-qa.md（QA-UC-08 同步） | 已交付 | APS 不可用（受限项），人工规范化核对通过 |
| coder（v129-impl） | FR-01-106 P 键 pieces 分块弹窗全量实现 + FR-01-96 v1.29 提示行随协议自适应 | 已交付 | cargo 全目标 3584P/0F、clippy 0、fmt 0；新增单测 40 |
| coder（v130-fix） | FR-01-106 缺陷修复：分块弹窗多并发乱序完成白屏——Progress 事件携带逐块完成集合 done_blocks，pieces() 🟩 以集合为准（前缀口径仅作空集回退）；已完成块不被冻结租约降级 | 已交付 | cargo 全目标 4011P/0F、clippy 0、fmt 0；新增单测 4（475→479） |
| **coder（v131-fix，本轮）** | **FR-01-106 缺陷修复：失败/暂停后部分下载块白屏**——Progress/PausedDone/Failed 三事件携带逐块部分集合 partial_blocks（引擎 Blocks 逐块真值；收尾事件为终态快照），pieces() 🟨 集合权威（投影租约零头口径仅作空集回退）；下载中 🟦 与 🟩 防降级口径不变 | 已交付 | **bin 485P/0F（479→485）+ `cargo test --tests` 15 目标合计 3727P/0F、clippy 0、fmt 0；新增单测 6** |

### 遗留受限项（当前有效）
- **APS 工具（gherkin-parser / gherkin-ir-dry-checker / gherkin-mutator）不可用**（沿袭开放）：
  环境为 fresh clone（.tools/ 空），Babashka 静态二进制经 GitHub release 下载停滞
  （网络受限）。影响面：05-ui-pieces.feature 与 04-ui-conns 场景 08 未过 parser/
  dry-checker 机检（specifier 轮已人工核对替代）；本轮 coder 不运行 Gherkin 机检
  （非 coder 职责），欠账不变。
- **Rust 工具链沙箱不持久（沿袭登记）**：本轮开工探测 `~/.cargo/bin` 本体在位
  （nightly 1.101.0 + clippy + rustfmt）、仅丢 PATH 注入——`export
  PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"` 即恢复（较 v130 轮的完整重建
  轻）；crates 镜像（rsproxy-sparse）与 ld.lld 桥接在位。新会话先按
  `packs/_common/notes/rust.md` §1 探针归因（本体丢失才重装）。
- **hardening_g_v147 首轮全量运行出现 1F（本轮新登记，复跑自愈）**：`cargo
  test --tests` 首轮该目标 348P/1F（日志过滤截断未捕获失败用例名），随后
  单目标复跑与第二次全量复跑均 349P/0F。本轮改动不触碰引擎运行时时序
  （纯展示数据通道 + 事件字段扩展），初判沙箱负载时序 flaky；后续会话若
  复现，建议保留完整测试输出定位用例名后向 hardender 移交。
- **观察受限突变体（沿袭 hardender 登记，仍开放）**：workers.rs:27:5/51:25/57:14/168:25、
  download.rs:277:52 共 5 条 + 等价 download.rs:85:27——run 级回补基建已铺。
- **ezr-proxy.rs 测试基建（沿袭裁决）**：127 变异点不拆、CRAP/CPD 读数不追责。
- **磁盘/沙箱**：沿袭口径（throttle_proxy 慢速门控用例前台分批 `QA_TP_ONLY=1,3`）。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D35、v138/v140 口径（见归档 handoff 登记清单）。
- **本轮零新增**：FR-01-106 全部交互口径（64×21 窗口、四态色块、±2 行滚轮、8 行
  PgUp/PgDn、前沿定位公式、模态关闭集）与 FR-01-96 v1.29 文案均为 demo 定稿 §6.4
  与 demo 源码逐字照录；本轮修复不引入新契约值——🟨 判定改为事件携带的逐块
  部分真值集合（实现内部数据流，非产品契约面）；「集合权威时投影租约零头退出
  🟨 判定（假 🟨 一并消除）」为 v129 定稿口径「部分下载 → 🟨」在失败/暂停态的
  正确化，非口径变更。

## 二、当前产出情况（coder v131-fix）

### 本会话产物清单
- `project/ezr/src/model/chunk/blocks.rs`：`Blocks::partial_indices()`——单次扫描
  产出升序部分块号（0 < written < 块长），与 `done_indices` 互斥同口径；
  + 单测 `partial_indices_sorted_excludes_done_and_empty`。
- `project/ezr/src/engine/mod.rs`：`Evt::Progress` / `Evt::PausedDone` /
  `Evt::Failed` 三变体新增 `partial_blocks: Vec<u32>`（升序部分块号；Failed 为
  失败台账真值，无台账失败臂为空集）。
- `project/ezr/src/engine/supervisor/download.rs`：进度 tick 同帧产出
  `partial_blocks = b.partial_indices()` 随 Progress 事件上报。
- `project/ezr/src/engine/supervisor/single.rs`：单流路径 Progress 携带空集合
  （单流无逐块记账，FR-01-12 口径不变）。
- `project/ezr/src/engine/supervisor/mod.rs`：`run()` 终态产出——PausedDone 从
  `state.blocks.partial_indices()`（stop_workers 后终态快照）；Failed 从失败台账
  sidecar 经新 helper `partial_of()`（与 `completed_of` 同款扫描）；无台账空集。
- `project/ezr/src/model/task.rs`：`Task` 新增 `partial_blocks` 字段（会话内，
  不持久化）；**`pieces()` 🟨 判定重写**——非「下载中」且集合非空 → 集合权威
  标 🟨（投影租约零头退出 🟨 判定：投影外真部分块不再 ⬜、投影内假零头不再
  虚标 🟨，双向消除），空集回退投影租约零头口径（v129 行为，重启后未收到
  逐块事件的存量面）；下载中 🟦（Active）口径不变、🟩 防降级保持；+ 行为
  回归测试 4 项（失败集合 🟨 / 零头租约不降级 / 下载中不标黄 / 空集回退）。
- `project/ezr/src/model/mod.rs`、`project/ezr/src/model/registry.rs`：`sample_task`
  与注册表重建点补字段初值（注册表不持久化该字段，重启后空集走租约回退）。
- `project/ezr/src/app/engine/evt.rs`：Progress 臂全量重铺 `t.partial_blocks`；
  PausedDone 臂终态快照重铺；Failed 臂经 `handle_failure` 新参重铺（Paused/Failed
  迟到事件守卫语义不变，重铺位于守卫之后）；Invalidated 臂（失效重下）同步
  清空——断点作废后不得残留旧集合虚标 🟨。
- `project/ezr/src/app/engine/mod.rs`：既有事件构造测试点补字段；+
  `partial_blocks_relaid_by_progress_paused_done_and_failed`（evt wiring 回归：
  三事件重铺 + 失效清空）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| bin 单元测试（含 property_tests 62） | 485P / 0F | `cargo test --bin ezr` |
| 集成测试 `cargo test --tests` | 3727P / 0F（15 个测试目标合计，含 lib/bin 重复计入 485） | 30、15、128、401、366、361、2、16、161、335、223、349、430、425 + lib/bin 485 |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| rustfmt | 0 diff | `cargo fmt --check` |
| 新增单测 | 6（479→485） | model 4（失败集合 🟨 / 零头租约不降级 / 下载中不标黄 / 空集回退）+ blocks 1（partial_indices）+ evt 1（wiring 三事件 + Invalidated 清空） |
| TDD 纪律 | 先红后绿 | 新测试在旧 `pieces()` 下实红（真部分块 `left: Empty`——白屏 bug 实证），修复后全绿；既有 29 项 pieces 测试零改动通过（口径兼容） |
| 全目标合计 | 3727P / 0F | `cargo test --bin ezr && cargo test --tests`（分项见上两行） |

### 对账口径（如存在基线）
- 上游 coder v130-fix 基线 4011P/0F（bin 479 + 集成 3532，含 #[path] 重复计入）→
  本轮实测 bin 485P/0F（479→485，+6 = 本轮新增 6 项）；`cargo test --tests`
  3727P/0F（本轮 6 项新测试经 `#[path]` 挂载在各 g_* 目标的重复计入 + 15 目标
  合并口径与上轮 12 目标分项口径的统计差异；0 失败为硬口径）。
- 规格层基线（specifier v129-spec）：本轮为缺陷修复轮，features/ 与 qa/ 零改动；
  源码基线对账 = 全目标 0F + clippy 0 + fmt 0。

### 待办与待批（未决项）
- **D35 待操作者复核**（沿袭，仍开放）：未探测任务按 `P` 拒绝臂文案与门槛序。
  如改判：改动面 = FR-01-106①、05-ui-pieces 场景 06、QA-UP-06、`app/pieces.rs`
  一处臂文案与一处单测断言。
- **APS 机检欠账**（沿袭）：05-ui-pieces.feature（新增）与 04-ui-conns.feature（场景
  08 修订）待网络可用时补跑 parser/dry-checker。
- **hardening_g_v147 flaky 复现排查**（本轮新增，见遗留受限项）：若后续会话
  复现 1F，保留完整输出定位用例名，移交 hardender。
- **沿袭移交（非本轮缺口）**：① 5+1 观察受限突变体回补；② 软 Gherkin 变异（APS
  安装 + runner 适配器）；③ qa/runners/ 13 个可执行套件与 v129 规格增量
  （QA-UP-01..08、QA-UC-08）的可执行化同步——属 six-pack/QA 职责范围，下一轮 QA
  会话承接。
- **本轮修复的口径说明（供下游/操作者知悉，非待批）**：暂停/失败态的断点视图
  （详情分块表）仍为 v139 前缀同参投影口径（事件只带计数，展示面投影不变）；
  pieces 弹窗在该两态下已按逐块真值标 🟩/🟨 且不被投影租约降级。若操作者希望
  详情分块表也按逐块真值展示，属 v139 口径修订（需 specifier 改规格）。

### 移交建议
- 建议下一步运行 `six-pack/cleaner`（以本 handoff 为上游交接内容）：对本轮触碰面
  （model/task.rs、model/chunk/blocks.rs、app/engine/evt.rs、supervisor/mod.rs 等）
  做重复度与清洁检查（`partial_indices`/`partial_of`/`done_indices`/`completed_of`
  四个扫描器已刻意保持同构扇出，如需收敛可评估泛型化单扫描器），完成后再经
  architect → hardender → QA 链路（QA 轮承接 qa/runners 可执行化同步，可顺带把
  05-ui-pieces 的多并发乱序/失败暂停场景补进 e2e）。移交仅为建议，不构成自动流转
  （宪法第一章）。

By coder.
