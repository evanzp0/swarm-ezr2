# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-v136-partial-chunk-fix（2026-10-19）
> 上游交接：coder v135-impl-m2（交接内容 = 前轮产物总账 + 移交建议 cleaner）
> + 操作者 chat 指令（bug 报告：「开 4 并发下载，4 个分块都没完成下载时暂停，按 p 可以
> 看到 4 个黄色的分块；然后按 M 将并发数修改成 2 个，继续下载，按 p 发现只有 2 个下载中
> 的分块，还有两个部分下载的分块都变成白色的了」+ 基线指令「在
> swarm-20261019-075052-six-pack-coder 的基础上修改这个 bug」——工作区 = 该包解压，
> 未从上游仓库重建）。
> 当轮任务：v136 缺陷修复轮——下载中缩容后无租约部分块白屏（集合权威适用面收窄）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.31，分期模式）+ `project/mission/phase-01.md`（v1.31）。
- 本轮任务 = 操作者 bug 报告轮（v136）：修复「M 缩容后继续下载中，无租约部分块 ⬜ 白屏」，
  集合权威扩面到下载中状态；无规格文档改动。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.31 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | v134 起 3 份 UI feature 过 APS 机检（0 错误级） |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F；CPD 17→15 |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 hardening_g_v147 | 已交付 | 3363P/0F、clippy 0、fmt 0 |
| QA（ezr-qa-final） | QA 规程可执行化 13 端到端套件（qa/runners/）+ 缺陷修复 | 已交付 | e2e 165P/0F/0S、3363P/0F、覆盖 94.37% |
| specifier（v129/v130/v133/v134） | FR-01-106/107/108/109/110 + D35/D36/D38 规格 + v1.31 对齐 | 已交付 | 反向自检通过 |
| coder（v129–v133） | FR-01-106 实现 + 缺陷修复三件 + v1.30 全部 UI 改版 + D35 收窄 | 已交付 | bin 497P/0F + 集成 2993P/0F → 3843P/0F（v133 口径） |
| coder（v135-impl-m2） | v1.31 规格面四项落地：①页脚双行 chips（footer.rs 重写 + footer_cmd_rects/footer_press/footer_flash + 鼠标 Down/Up 臂）；②帮助 36 行（页脚命令条目）；③浮层翻转零清屏（退役 overlay_sig + terminal.clear，clip_wide_at_edges 缓冲区级根治）；④并发明细最后满窗起点钳制 | 已交付 | 3867P/0F、clippy 0、fmt 0、arch 规则级守恒、退役标识反向自检通过 |
| **coder（v136-partial-chunk-fix，本轮）** | **缺陷修复：`Task::pieces()` 部分块集合权威去 `!downloading` 前提（`partial_authoritative = !partial_blocks.is_empty()`）——M 缩容/恢复后下载中任务的无租约部分块恢复 🟨；+2 回归测试（主回归 `pieces_downloading_unleased_partial_blocks_show_yellow` + 🟩 防降级护栏 `pieces_downloading_unleased_partial_not_downgrading_done`）；注释同步 4 处（PieceState 文档/pieces() 文档/行内注释/既有测试文档）；规格零改动（FR-01-106③ 字面口径已覆盖）** | **已交付** | **3887P/0F、clippy 0、fmt 0、arch 与基线同形（规则级守恒，0 新增失败行）、gen 守卫面不含本轮触碰文件** |

### 遗留受限项（当前有效）
- **arch_check 规则 14（ui 回填白名单）沿袭未过**：8 行同形态（dlg_pos/help_pos/pieces_pos
  首帧回填 + scroll/follow 视图态钳制，视图态回填而非策略状态改写），规则级守恒不变
  （15 项 OK、仍仅规则 14 一条失败）。本轮零触碰 ui 层、0 新增失败行。**移交建议（不变）**：
  cleaner/architect 轮将白名单正则扩为含 `scroll|pos|follow` 并裁决存量 8 行形态。
- **v147 时序 flake（沿袭）**：`sidecar_not_written_within_first_500ms` / `hardening_g_v147`
  偶发（隔离 ×3 + 重跑绿）；本轮全量跑未复现。建议 QA 轮放宽 `wait_state` 预算或加重试。
- **Rust 工具链沙箱不持久（本轮重建登记）**：本轮完整重装——nightly
  1.98.0-nightly（cced03bfd 2026-05-28；rustup-init 经 rsproxy 下载 18.9MB/s，dist 走
  aliyun frozen 快照 manifest 200）+ rustfmt/clippy 组件 + `~/.local/bin/ld.lld` →
  rust-lld 符号链接桥接 + crates 走 rsproxy-sparse（`~/.cargo/config.toml`）。下轮会话
  先按 `packs/_common/notes/rust.md` §1 探针归因（丢注入 export 即恢复；本体缺失才重装）。
  注意：nohup 后台安装会被沙箱回收（engineering.md 既有条目形态），重装须前台执行。
- **观察受限突变体（沿袭 hardender 登记，仍开放）**：workers.rs 5 条 + 等价 download.rs:85:27。
- **ezr-proxy.rs 测试基建（沿袭裁决）**：127 变异点不拆、CRAP/CPD 读数不追责。
- **qa/runners/ 可执行化同步欠账（沿袭 + v1.30/v1.31 增量再扩大）**：13 个可执行套件与
  v1.30/v1.31 增量行为面（页脚双行 chips 点击、对话框拖拽场景 23、帮助 36 行、浮层
  零清屏、满窗钳制、Ctrl+C 直通等）未进 runner——属 six-pack/QA 职责，QA 轮一并承接。
- **gen 剥头副本守卫覆盖面（本轮核对登记）**：`tests/gen/*_stripped.rs` 覆盖
  btn/chart/clipboard/detail/update_conn_stats 五件，本轮触碰文件（model/task.rs）不在
  守卫面，无需再生成；gen-drift 守卫全绿随全量套件复核。
- **上游仓库滞后（本轮事实登记，供后续会话参考）**：github.com/evanzp0/swarm-ezr2 的
  clone 为数轮之前的代码状态（v134 前），不含 v135-impl-m2 与本轮增量；工作区一律以
  操作者指定的基线包为准（engineering.md 已沉淀对应通则）。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D37、v138/v140 口径（见归档 handoff 登记清单）。
- **D35 结案（操作者裁决 2026-10-18）+ 实现层收窄（`!probed && total == 0`）**：v1.31
  已落账 mission/FR-01-106①，实现已在位（v133），本轮无改动。
- **D38 登记（全部弹窗拖拽，产品契约口径）**：实现已在位（v133）；**遗留待裁决差异**
  （产品 = 非交互区起拖 + 字段/按钮 Down 即点击 vs demo m2 = 任意位置起拖 + Up 未移动
  = 点击）本轮操作者指令未改判 → 维持产品口径零改动（FR-01-110① 双向登记不变，
  是否对齐 demo 仍留操作者裁决）。
- **本轮口径注记（非新增实现定义值）**：pieces 🟨 集合权威自 v136 起不限任务状态
  （`partial_blocks` 非空即权威）——FR-01-106③ 字面口径「块内有已落盘零头但未分配
  并发下载」已覆盖，🟦 优先于 🟨、🟩 不降级语义不变。

## 二、当前产出情况（coder v136-partial-chunk-fix）

### 本会话产物清单
- `project/ezr/src/model/task.rs`（唯一生产文件，+81/−10）：
  - `Task::pieces()`：`partial_authoritative` 去 `!downloading` 前提 →
    `!self.partial_blocks.is_empty()`——下载中任务存在「有落盘零头但无租约」的块
    （M 缩容回收的租约、quota 中止块）时集合臂恢复判定，无租约部分块 🟨 不再 ⬜；
  - 注释同步 4 处：`pieces()` 文档 🟨 条目（v131 → v136 扩面口径）、行内注释
    （集合权威正交于对象状态 + 下游优先级保持论证）、`PieceState::Partial` 变体文档
    （补「下载中缩容回收的无租约块」形态）、既有测试 `pieces_downloading_partial_set_stays_active`
    文档（🟦 口径不变注记）；
  - `tests` 模块新增 2 回归测试：`pieces_downloading_unleased_partial_blocks_show_yellow`
    （操作者 bug 场景逐块断言：4 并发暂停 4 部分块 → M 缩容 2 → 块 0/1 🟦、块 2/3 🟨、
    其余 ⬜）、`pieces_downloading_unleased_partial_not_downgrading_done`（🟩 防降级
    护栏：done_blocks 与 partial_blocks 交叠时集合提升只作用于 ⬜）。
- `packs/_common/engineering.md`（工具侧经验沉淀，2 处）：
  - 「进度/状态类展示」条目扩充：集合权威适用面不得按对象状态收窄（门槛维持期载体
    变动产生的真值集合项失去判定路径；集合权威与对象状态正交）；
  - 「长任务的状态与续做」新增条目：操作者指定基线快照包时以包为工作区起点、不从
    上游仓库重建（上游仓库可能滞后于流水线产物）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量测试 | **3887P / 0F**（`cargo test --all-targets --no-fail-fast`，15 目标全绿） | `cd project/ezr && cargo test --all-targets --no-fail-fast` |
| TDD 红灯复现 | 修复前 2 新测试 FAILED（left: Empty / right: Partial = 白屏形态）；修复后全绿 | `cargo test --bin ezr pieces_downloading_unleased` |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| fmt | 0 偏差 | `cargo fmt --check` |
| arch_check | 与基线同形：15 项 OK，仅规则 14 沿袭 8 行（本轮零触碰 ui 层、0 新增失败行） | `bash scripts/arch_check.sh` |
| 规格核证 | FR-01-106③ 🟨 判据 = 「块内有已落盘零头但未分配并发下载」，状态枚举为举例非限定 → 修复后行为在规格字面口径内，零规格改动 | mission/phase-01.md FR-01-106 条精读 |

### 对账口径（如存在基线）
- 上游 v135-impl-m2 基线 = 3867P/0F。本轮测试计数 **3867 → 3887（+20）**，复算自洽
  = 2 新测试 × 10 个挂载 model 树的编译语境（bin 510→512、hardening_engine_backfill
  426→428、hardening_g_appcore 376→378、hardening_g_engine 371→373、hardening_g_slots
  161→163、hardening_g_supervisor 345→347、hardening_g_throttle_err 223→225、
  hardening_g_v147 359→361、hardening_ui_backfill 455→457、hardening_v115 450→452，
  各 +2；ezr-fixture 30、ezr-proxy 15、hardening 128、hardening_g_main 2、
  hardening_g_proxy 16 五个不挂载 model 树的目标不变）。
- arch 规则 14 失败行集合逐行比对 = 基线 8 行同集合（dlg_pos×2/help_pos/help_scroll/
  pieces_pos/pieces_follow/pieces_scroll×2），规则级守恒。

### 待办与待批（未决项）
- **待操作者裁决（沿袭开放）：对话框拖拽起拖面/点击时机**（FR-01-110①/D38）——本轮
  操作者指令未触及，维持产品口径零改动；如后续改判 B（对齐 demo），改动面 =
  mouse.rs 起拖分流与点击时机 + 测试。
- **qa/runners/ 同步欠账**（沿袭 + v1.30/v1.31 增量）：QA 轮承接。
- **arch 规则 14 白名单扩展建议**（沿袭）：cleaner/architect 裁决。
- **v147 时序 flake**（沿袭）：如复现，QA 轮放宽预算或加重试。
- **工具链重建**（本轮登记）：新会话先探针 PATH 注入（`command -v cargo`；失败则
  `export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"`），本体缺失才按 rust.md §1 重装
  （aliyun dist 源；nohup 后台安装会被沙箱回收，须前台执行）。

### 移交建议
- 建议下一步运行 `six-pack/cleaner`（以本 handoff 为上游交接内容）：v136 修复切片已全绿
  （修复收敛 1 个生产文件），按 cleaner 职责做保持行为清理、覆盖率提升、CRAP/DRY 审查
  与变异点扫描；顺带裁决 arch 规则 14 白名单扩展（scroll|pos|follow）与存量 8 行形态。
- 其后按链路 architect → hardender → QA；QA 轮顺带承接 qa/runners 可执行化同步
  （含 v1.30/v1.31 增量）。移交仅为建议，不构成自动流转（宪法第一章）。

By coder.
