# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：v129-impl（任务 ezr-v129-impl）
> 上游交接：specifier v129-spec（本轮 = 操作者指令两批新增需求的实现轮）：
> 实现 FR-01-106（P 键 pieces 分块弹窗 + 页脚 `P 分块`）与 FR-01-96 v1.29（并发面板
> 提示行随协议自适应），全部 UI 与交互逐字对齐 ezr-demo 定稿 §6.4 与 demo 源码。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.29，分期模式）+ `project/mission/phase-01.md`（v1.29）。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.29 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | 逐轮 parser/dry-check 通过（APS 不可用轮以人工规范化核对替代，见受限项） |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F；CPD 17→15 |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 hardening_g_v147 | 已交付 | 3363P/0F、clippy 0、fmt 0、arch 15 |
| QA（ezr-qa-final） | QA 规程可执行化 13 端到端套件（qa/runners/）+ 缺陷修复 + 双度量复测 | 已交付 | e2e 165P/0F/0S（13 套件）、3363P/0F、覆盖 94.37%、CRAP≥30=0、CPD 89/15 |
| specifier（v129-spec） | mission v1.29（FR-01-106 + FR-01-96 修订 + D35）+ features/05-ui-pieces.feature（8 场景）+ features/04-ui-conns.feature（场景 08 修订）+ qa/05-ui-pieces-qa.md（QA-UP-01..08）+ qa/04-ui-conns-qa.md（QA-UC-08 同步） | 已交付 | APS 不可用（受限项），人工规范化核对通过 |
| **coder（v129-impl，本轮）** | **FR-01-106 P 键 pieces 分块弹窗全量实现 + FR-01-96 v1.29 提示行随协议自适应**（详见当前节产物清单） | 已交付 | **cargo 全目标 3584P/0F（bin 475 + 集成 3109）、clippy 0、fmt 0；新增单测 40（435→475）** |

### 遗留受限项（当前有效）
- **APS 工具（gherkin-parser / gherkin-ir-dry-checker / gherkin-mutator）不可用**（沿袭开放）：
  环境为 fresh clone（.tools/ 空），Babashka 静态二进制经 GitHub release 下载停滞
  （网络受限）。影响面：v129-spec 新增/修订的 05-ui-pieces.feature 与 04-ui-conns
  场景 08 未过 parser/dry-checker 机检（specifier 轮已人工核对替代）；本轮 coder 不
  运行 Gherkin 机检（非 coder 职责），欠账不变。建议下游会话在网络可用时补装
  （TUNA 镜像对 GitHub release 无效，APS 资产仍在 GitHub）。
- **Rust 工具链本轮已重建（跨会话不持久，沿袭登记）**：rustup 1.101.0-nightly
  （minimal profile + rustfmt + clippy）装于 `~/.cargo` + `~/.rustup`；lld 桥接
  `~/.local/bin/ld.lld` → 工具链 rust-lld；crates 镜像 `~/.cargo/config.toml`
  （rsproxy-sparse）。新会话若环境重置需按 `packs/_common/notes/rust.md` §1 重建。
- **观察受限突变体（沿袭 hardender 登记，仍开放）**：workers.rs:27:5/51:25/57:14/168:25、
  download.rs:277:52 共 5 条 + 等价 download.rs:85:27——run 级回补基建已铺。
- **ezr-proxy.rs 测试基建（沿袭裁决）**：127 变异点不拆、CRAP/CPD 读数不追责。
- **磁盘/沙箱**：沿袭口径（throttle_proxy 慢速门控用例前台分批 `QA_TP_ONLY=1,3`）。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D35、v138/v140 口径（见归档 handoff 登记清单）。
- **本轮零新增**：FR-01-106 全部交互口径（64×21 窗口、四态色块、±2 行滚轮、8 行
  PgUp/PgDn、前沿定位公式、模态关闭集）与 FR-01-96 v1.29 文案均为 demo 定稿 §6.4
  与 demo 源码逐字照录，非实现定义值。

## 二、当前产出情况（coder v129-impl）

### 本会话产物清单
- `project/ezr/src/model/task.rs`：`PieceState` 四态枚举（Empty/Done/Partial/Active）+
  `Task::supports_pieces()`（BT 恒支持 / HTTP 需 resumable，FR-01-106 功能门槛）+
  `Task::pieces()`（Done 前缀 / Active 分配即算 / 🟦 优先于 🟨 / 待命连接不计 /
  越界租约防御 / 空表）。
- `project/ezr/src/model/mod.rs`：re-export `PieceState`。
- `project/ezr/src/app/pieces.rs`（新增）：`open_pieces()` 四拒绝臂（未选中 → 未探测
  D35 → 不支持分块 → 空内容，toast 文案照录）+ `close_pieces()` + `pieces_scroll_by()`。
- `project/ezr/src/app/mod.rs`：弹窗状态字段（pieces_open/scroll/follow/area/pos/drag）。
- `project/ezr/src/app/keys.rs`：`P` 键路由 + 弹窗模态分支（Esc/P/q/Q 关闭且 q/Q 不
  退出、↑↓/PgUp/PgDn/Home/End 滚动、其余键消费为无动作；Ctrl 分支先于弹窗，
  FR-01-97 口径回归保持）。
- `project/ezr/src/app/mouse.rs`：`on_mouse_pieces` 分支最上层（滚轮 ±2 行滚动网格、
  左键内按下记抓取偏移、Drag 随动、Up 结束、点外关闭不穿透任务列表）。
- `project/ezr/src/ui/pieces.rs`（新增）：`draw_pieces` 渲染——64×21 圆角浮层居中/
  拖拽位置钳制、标题「分块状态」加粗青色、四段（任务名白粗截尾 / 元信息暗灰
  `协议 · 块大小 N · x/y 块 · 并发 N` / 宽 2 列 run 合并色块网格 / 提示行暗灰）、
  打开定位下载前沿（首帧一次）、右缘滚动条（超一屏）、`clip_wide_at_edges` 同口径。
- `project/ezr/src/ui/mod.rs`：`mod pieces` + draw() 末层渲染弹窗；既有 conns 面板
  金样测试断言同步 v1.29 口径（HTTP 无 Ctrl+B 段、「（仅 BT）」退役）。
- `project/ezr/src/ui/footer.rs`：页脚新增 `P 分块`（位于「R 重试」与「A 添加」之间，
  式样同其他项；窄屏截断档位不变）。
- `project/ezr/src/ui/conns.rs`：**FR-01-96 v1.29**——底部提示行随任务协议自适应
  （BT = `Ctrl+↑↓ 选择 · Ctrl+B 断开`、非 BT = `Ctrl+↑↓ 选择`，「（仅 BT）」标注退役）。
- `project/ezr/src/main.rs`：`overlay_sig` 升 u32 并纳入 pieces 开关（位 2）与拖拽
  位置（位 8..23）——开/关/拖拽帧全量重绘（FR-01-106 ⑧）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| bin 单元测试（含 property_tests 62） | 475P / 0F | `cargo test --bin ezr` |
| 集成测试 12 目标 | 3109P / 0F | hardening 126、engine_backfill 391、g_appcore 356、g_engine 351、g_main 2、g_proxy 16、g_slots 153、g_supervisor 325、g_throttle_err 215、ui_backfill 420、v115 415、g_v147 339（`cargo test --test <名>` 逐目标） |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| rustfmt | 0 diff | `cargo fmt --check` |
| 新增单测 | 40（435→475） | model 12 + app/pieces 8 + keys 4 + mouse 4 + ui/pieces 7 + footer 2 + conns 3 |
| TDD 纪律 | 每切片先红后绿 | 模型层 E0433 红 → 绿；app 层 unimplemented 桩 15 红 → 绿；ui 层桩 7 红 → 绿 |

### 对账口径（如存在基线）
- 上游 QA ezr-qa-final 基线 3363P/0F（未留分项）→ 本轮实测全目标 3584P/0F：
  bin 435→475（+40，全为本轮新增）+ 集成目标实测 3109（上游无分项可逐目标对账，
  偏差 +181 归上游口径未留分项，以本轮逐目标实测为准）。
- 规格层基线（specifier v129-spec）：本轮为纯实现轮，features/ 与 qa/ 零改动；
  源码基线对账 = 全目标 0F + clippy 0 + fmt 0。

### 待办与待批（未决项）
- **D35 待操作者复核**（沿袭，本轮唯一相关决策点）：未探测任务按 `P` 拒绝臂文案
  `任务尚未探测，暂无分块信息` 与门槛序（未探测臂先于「不支持分块」「空内容」）。
  如改判：改动面 = FR-01-106①、05-ui-pieces 场景 06、QA-UP-06、`app/pieces.rs`
  一处臂文案与一处单测断言。
- **APS 机检欠账**（沿袭）：05-ui-pieces.feature（新增）与 04-ui-conns.feature（场景
  08 修订）待网络可用时补跑 parser/dry-checker。
- **沿袭移交（非本轮缺口）**：① 5+1 观察受限突变体回补；② 软 Gherkin 变异（APS
  安装 + runner 适配器）；③ qa/runners/ 13 个可执行套件与 v129 规格增量
  （QA-UP-01..08、QA-UC-08）的可执行化同步——属 six-pack/QA 职责范围，下一轮 QA
  会话承接；本轮 8 场景行为已由 40 项单测覆盖（含 TestBackend 伪终端金样）。

### 移交建议
- 建议下一步运行 `six-pack/cleaner`（以本 handoff 为上游交接内容）：对本轮触碰面
  （model/task.rs、app/pieces.rs、ui/pieces.rs 等）做重复度与清洁检查，完成后再经
  architect → hardender → QA 链路（QA 轮承接 qa/runners 可执行化同步）。移交仅为
  建议，不构成自动流转（宪法第一章）。
