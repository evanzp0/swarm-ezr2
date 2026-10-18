# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-v133-opfix（2026-10-18）
> 上游交接：coder v132-ui（交接内容 = 前轮产物总账）+ 操作者 chat 指令五条：
> （① D35 裁决「保留现在方案」；② APS 机检欠账清偿；③ 对话框拖拽缺陷；④
> 并发下调面板残留缺陷；⑤ 部分块暂停后 P 打不开弹窗缺陷）。
> 当轮任务：缺陷修复轮（3 缺陷 + 1 裁决登记 + 1 欠账清偿）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.30，分期模式）+ `project/mission/phase-01.md`（v1.30）。
- 本轮任务 = 操作者指令轮（v133）：D35 结案登记、APS 机检欠账清偿、三缺陷修复。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.30 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | v133 起 features 已过 APS 机检（0 错误级），人工核对口径退役 |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F；CPD 17→15 |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 hardening_g_v147 | 已交付 | 3363P/0F、clippy 0、fmt 0 |
| QA（ezr-qa-final） | QA 规程可执行化 13 端到端套件（qa/runners/）+ 缺陷修复 | 已交付 | e2e 165P/0F/0S、3363P/0F、覆盖 94.37% |
| specifier（v129-spec / v130-spec） | FR-01-106 实现 + v1.30 规格修订 | 已交付 | 反向自检 grep 通过 |
| coder（v129-impl / v130-fix / v131-fix） | FR-01-106 实现 + 两轮缺陷修复 | 已交付 | bin 485P/0F + 集成 3727P/0F（v131-fix 口径） |
| coder（v132-ui） | v1.30 全部 UI 改版实现 | 已交付 | bin 497P/0F + 集成 2993P/0F、clippy 0、fmt 0、arch 规则级守恒 |
| **coder（v133-opfix，本轮）** | **三缺陷修复（对话框拖拽 D38、缩容连接回收、未探测臂收窄）+ D35 结案登记 + APS 机检欠账清偿（bb v1.13.226 + 14 features 全量机检 0 错误级）+ 测试域修正（ipv6 property 端口归一化）** | **已交付** | **3843P/0F（bin 504 + 集成 12 套件 3294）、clippy --all-targets 0 警告、fmt --check 0 diff、arch 15 OK 守恒（规则 14 基线同败，见受限项）** |

### 遗留受限项（当前有效）
- **arch_check 规则 14（ui 回填白名单）沿袭未过**：基线（v132 前即败）6 行（pieces 渲染
  路径 `pieces_pos/pieces_follow/pieces_scroll` + help `help_pos/help_scroll`——视图态
  回填而非策略状态改写）。本轮新增 **2 行同形态违规**（`dlg_pos` 首帧居中回填，`ui/
  dialog.rs` / `ui/delete.rs` 各 1 处）：违规行 6→8，**规则级守恒不变**（15 项 OK、仍仅
  规则 14 一条失败）。**移交建议（不变 + 扩面）**：cleaner/architect 轮将白名单正则扩为
  含 `scroll|pos|follow`（与规则注释「回填字段」意图一致，一并合法化 8 行），属架构裁决，
  coder 不越权改 scripts/arch_check.sh。
- **v147 时序 flake（沿袭 + 本轮再观测 ×2）**：`sidecar_not_written_within_first_500ms`
  全量第 1 遍偶发 1 次（隔离 ×3 + v147 全套重跑绿）；`hardening_g_v147` 全量另 1 遍偶发
  1 次同款（隔离重跑 359P/0F 绿）。滴流桩真连接 + `wait_state`（≈800ms 预算）在宿主
  高负载下可超时，非本轮改动引入（本轮未触碰引擎 flush 路径）。**移交建议（不变）**：
  QA 轮放宽 `wait_state` 预算或加重试。
- **Rust 工具链沙箱不持久（沿袭登记）**：本轮重装 nightly 1.101.0（2026-10-18，rsproxy
  rustup-init 直链 + USTC dist + rsproxy-sparse crates 源 + rust-lld 桥接；clippy 基线
  0 警告复核）。新会话先按 `packs/_common/notes/rust.md` §1 探针归因。
- **观察受限突变体（沿袭 hardender 登记，仍开放）**：workers.rs 5 条 + 等价 download.rs:85:27。
- **ezr-proxy.rs 测试基建（沿袭裁决）**：127 变异点不拆、CRAP/CPD 读数不追责。
- **qa/runners/ 可执行化同步欠账（沿袭 + 本轮再扩大）**：13 个可执行套件与 v1.30 增量的
  同步欠账（旧断言 `Ctrl+↑↓`/`Ctrl+B`/旧页脚/64×21）沿用；本轮缺陷修复再新增行为面
  （对话框拖拽 D38、未探测臂收窄、缩容连接行即时回收）未进 runner——属 six-pack/QA
  职责，QA 轮一并承接。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D37、v138/v140 口径（见归档 handoff 登记清单）。
- **D35 结案（操作者裁决 2026-10-18：保留现在方案）**：未探测任务按 `P` 拒绝臂的文案
  与门槛序维持 v1.29 定型，复核待办关闭。
- **D35 实现层收窄（v133，语义澄清不改臂）**：未探测臂命中条件由 `!probed` 收窄为
  `!probed && total == 0`。依据 = D35 立臂理由原文「探测前大小未知态」：恢复（继续）
  后的重探窗口（获槽清 probed、Probed 事件未至）任务大小已知，不属未知态，落入正常
  三臂可开窗（操作者 v133 实证：此时按 P 被误拒）。臂序与文案一字未动，D35 裁决
  不受影响。**规格同步移交 specifier**：phase-01.md FR-01-106 ①未探测臂条件与 D38
  （下条）随下一 spec 轮补录。
- **D38 新增（v133 操作者指令：全部弹窗支持鼠标拖拽移动）**：添加/修改/删除对话框
  支持左键拖拽移动（此前仅帮助/pieces 两浮层支持）。口径：
  - 起拖面 = 窗口内**非交互区**（边框/标题/空白，未命中字段行与按钮）；字段/按钮/
    下拉项点击语义不变（拖拽与点击互斥分流）；下拉浮层展开期间不拖拽（点按先消费）；
  - 点击窗口外无动作（对话框仅经按钮/`Esc`/`Q` 关闭——与帮助/pieces 的点外关闭
    口径**不同**：数据录入模态防误触丢输入）；
  - 位置记忆**不保留**：每次打开重新居中（与 pieces 同口径，与帮助跨次记忆不同）；
  - 拖拽帧全量重绘：`overlay_sig` 重构为「模态互斥 + 当前浮层左上角编码」（对话框
    ⇄ pieces ⇄ 帮助任一打开时其余不可达，位 8..63 按「当前打开浮层」编码位置，
    对话框拖拽并入；原对话框/下拉断言值 0/1/3 兼容不变）。

## 二、当前产出情况（coder v133-opfix）

### 本会话产物清单
- `ezr/src/app/pieces.rs`：open_pieces 未探测臂收窄（`!probed && total == 0`）+ 文档
  登记 D35 收窄依据；新增 2 测试（重探窗口可续传开窗 / 重探窗口不可续传落支持臂）。
- `ezr/src/app/mod.rs`：App 新字段 `dlg_area`/`dlg_pos`/`dlg_drag`（对话框区域热区与
  拖拽状态，ui 层回填/mouse 层消费）。
- `ezr/src/app/dialogs.rs`：`set_dialog` 单源入口（置入对话框 + 清位置/拖拽/区域——
  每次打开重新居中）；open_add/modify/delete 三入口切换。
- `ezr/src/app/mouse.rs`：on_mouse_dialog 重写为全事件形态——Down（下拉优先 → 字段 →
  按钮 → 非交互区起拖）+ Drag（dlg_pos = 指针 − 抓取偏移）+ Up（结束）；新增拖拽
  全链路测试（起拖偏移/随动/结束/字段行不起拖/窗外无动作/重开居中）。
- `ezr/src/ui/btn.rs`：`overlay_rect` 共享助手（拖拽位置优先钳制、None 居中——与
  pieces/help 渲染同口径）。
- `ezr/src/ui/dialog.rs` / `ezr/src/ui/delete.rs`：绘制矩形改 overlay_rect + 首帧居中
  记忆回填 + dlg_area 热区回填；新增 2 渲染测试（首帧居中回填 13,9 / 越界拖拽点钳制
  26,19 且记忆保留原始值）。
- `ezr/src/main.rs`：overlay_sig 重构（模态互斥 + 当前浮层位置编码，D38 拖拽帧全量
  重绘）；对话框/下拉既有断言值兼容。
- `ezr/src/engine/supervisor/workers.rs`：block_worker 块边界配额退出清自身连接行
  （缩容后超额槽位僵死行根治点之一）。
- `ezr/src/engine/supervisor/download.rs`：Reconfigure 下调即回收 id > q 连接行
  （待命行 + 块边界退出遗留行统一收敛，不等下一进度帧）。
- `ezr/src/app/engine/mod.rs`（测试）：新增 2 集成测试——暂停（在飞块带零头）后 P
  开窗全流程（Range 滴流桩 206）；并发 4→2 缩容后连接行全部 id ≤ 2（走真实
  Reconfigure 命令 + 20 帧泵观察）。
- `ezr/src/property_tests.rs`：ipv6 endpoint property 端口保真断言改
  `port_or_known_default()`（url crate 默认端口归一化，测试域修正，产品无错）。
- `ezr/tests/gen/btn_stripped.rs`：剥头副本再生成（v133 源码口径，btn.rs 含
  overlay_rect）。
- `.tools/`：bb v1.13.226（linux-amd64-static）+ Acceptance-Pipeline-Specification
  仓（codeload main tarball）——APS 机检环境就位。
- 工具侧经验沉淀：`packs/_common/notes/aps.md`（bb 安装配方 + duplicate-in-scenario
  两类合法重复形态）、`packs/_common/notes/rust.md`（rustup-init 直链 ELF 执行方式 +
  url crate 默认端口归一化）、`packs/_common/engineering.md`（codeload tarball 替代
  git clone）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 单元 + 集成全量 | **3843P/0F**（bin 504 + fixture 30 + proxy 15 + 12 套件 3294；v147 flake 两次偶发经隔离 ×3 + 全套重跑绿） | `cargo test`（逐套件明细见下对账） |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| fmt | 0 diff | `cargo fmt --check` |
| 架构守卫 | 15 OK 守恒；规则 14 基线同败 + 本轮 dlg_pos 2 行同形态（见受限项） | `bash scripts/arch_check.sh` |
| 剥头副本一致性 | 4 副本逐字一致（btn 再生成后） | `cargo test --test hardening_ui_backfill` |
| 逐字契约核证 | 对话框拖拽语义与帮助/pieces 拖拽实现逐字对照（抓取偏移/随动/钳制公式同源） | mouse.rs 单测 + TestBackend 渲染金样 |
| **APS 机检（欠账清偿）** | **14/14 features 解析 0 错；dry-checker 770 findings 全部 advisory（near-duplicate 390 / possible-synonym 378 / duplicate-in-scenario 2），错误级 0 = 验收口径达成**；2 条 duplicate-in-scenario 人工复核为合法重复（两段式确认流 / 逐臂同断言），不改规格 | `(cd .tools/Acceptance-Pipeline-Specification && <swarm>/.tools/bb gherkin-parser <feature> <out>.json && bb gherkin-ir-dry-checker <ir> <out>.dry.json)`；产物在 `.work/tmp/aps/`（会话级，复跑即得） |

### 对账口径（如存在基线）
- 上游 v132-ui 基线：bin 497 + 集成 2993（= 3490）。本轮 bin 504（+7：pieces 收窄 ×2、
  暂停开窗集成 ×1、缩容回收集成 ×1、拖拽链路 ×1、对话框渲染 ×2）+ 集成 3294。
  集成计数口径：本轮逐套件实跑合计（hardening 128 / engine_backfill 420 / g_appcore
  376 / g_engine 371 / g_main 2 / g_proxy 16 / g_slots 161 / g_supervisor 345 /
  g_throttle_err 223 / g_v147 359 / ui_backfill 449 / v115 444）；v132 报告的 2993
  与本轮逐套件合计的差额属上轮汇总口径（本轮以 `cargo test` 逐套件输出为准）。
- 全量运行中两次偶发失败均为既有登记的时序 flake 家族（`sidecar_not_written_
  within_first_500ms` ×1、`hardening_g_v147` 全量 ×1），隔离 ×3 + 全套重跑绿，
  非本轮引入。
- 测试域修正 1 处（ipv6 property 端口归一化）：proptest 随机种子命中 port=80 暴露
  `url::Url` 默认端口归一化行为，产品 endpoint_url 无错，断言域修正后反例回放通过。

### 待办与待批（未决项）
- **arch_check 规则 14 白名单扩展建议**（沿袭移交，本轮扩面）：cleaner/architect 裁决。
- **v147 时序 flake**（沿袭，本轮再观测 ×2）：如复现，建议 QA 轮将 `wait_state` 预算
  放宽或加重试。
- **qa/runners/ 同步欠账**（沿袭 + 本轮再扩大）：QA 轮承接（含 v1.30 增量 + 本轮
  缺陷修复行为面）。
- **规格同步**（本轮移交 specifier）：FR-01-106 ①未探测臂条件收窄（`!probed &&
  total == 0`）与 D38（对话框拖拽）补录 phase-01.md；qa/ 05-ui-pieces 对应场景断言
  同步随 QA 轮。

### 移交建议
- 建议下一步运行 `six-pack/specifier`（以本 handoff 为上游交接内容）：把 D35 结案、
  D38 对话框拖拽、未探测臂收窄补录 mission/phase-01.md（本轮三缺陷中另两项为纯
  缺陷修复无规格面）。
- 其后按链路 specifier → cleaner → architect → hardender → QA；QA 轮顺带承接
  qa/runners 可执行化同步（含本轮增量）。移交仅为建议，不构成自动流转（宪法第一章）。

By coder.
