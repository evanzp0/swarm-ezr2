# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-v139-m3-padding-fix（2026-10-21）
> 上游交接：project/handoff.md（coder v138-m3-impl 交接）+ 操作者 chat 指令
> （「bug: 并发 panel 右上角的'活跃数'左边留出和右边相同的 padding；修改好后，
> ui 同步修改下 ezr-demo 并修改下它的文档」）。
> 当轮任务：v139 小步修复轮——并发连接面板右上角活跃计数标题左右 padding 对称，
> ezr 与 ezr-demo 同步，demo 设计文档同步修订。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.32，分期模式）+ `project/mission/phase-01.md`（v1.32）。
- 源码状态 v139（全 15 目标 4009P/0F）。上轮 v138 已交付 m3 对齐十项改动面，
  本轮在其上做操作者报告的 UI 细节缺陷修复（活跃计数标题 padding 对称）。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.32 全量规格 + features/ 15 份 + qa/ 15 份 | 全部交付 | v134 起 3 份 UI feature 过 APS 机检（0 错误级）；v132 轮修订未过机检（见受限项） |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F；CPD 17→15 |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 hardening_g_v147 | 已交付 | 3363P/0F、clippy 0、fmt 0 |
| QA（ezr-qa-final） | QA 规程可执行化 13 端到端套件（qa/runners/）+ 缺陷修复 | 已交付 | e2e 165P/0F/0S、3363P/0F、覆盖 94.37% |
| specifier（v129–v134/v132） | FR-01-106…110 + v1.30/v1.31/v1.32 对齐规格 | 已交付 | 反向自检通过 |
| coder（v129–v137） | FR-01-106 实现 + 缺陷修复 + v1.30/v1.31 UI 改版 + q 退出鼠标泄漏修复 | 已交付 | 3889P/0F、clippy 0、fmt clean（v137 口径） |
| coder（v138-m3-impl） | m3 对齐全量实现：scrollbar.rs 引擎 + scrollable_panel.rs 组件（FR-01-109 重写，4 实例替换 lazy_scrollbar）+ About 窗口（ui/about.rs，FR-01-111）+ 头部/对话框/校验 NONE/帮助 44 行/模态统一口径；新测试 +74 | 已交付 | 3963P/0F、clippy 0、fmt clean、arch_check 规则 1–13/15 过（规则 14 沿袭未过，见遗留项） |
| **coder（v139-m3-padding-fix，本轮）** | **并发面板活跃计数标题左右 padding 对称：ezr conns.rs `active_title()` 单源构造（` 活跃 N ` 前后各 1 空格）+ 形态守卫测试 +1；ezr-demo ui.rs 同款字面量 + 注记；demo 设计文档 §5.4 修订；rust.md 经验 +2** | **已交付** | **ezr 4009P/0F（全 15 目标）、clippy 0、fmt clean；ezr-demo cargo test 2P/0F** |

### 遗留受限项（当前有效）
- **arch_check 规则 14（ui 回填白名单）未过——沿袭 17 行同一集合，全部含 `scroll` 或 `pos`**：
  沿袭移交建议（cleaner/architect 轮把规则 14 白名单正则扩为含 `scroll|pos|follow` 并裁决
  存量）一次扩展即可全量收口；本轮零新增回填字段（无新形态）。
- **APS 机检沿袭未执行**（v132 修订的 3 份 UI feature 未过 `gherkin-ir-dry-checker`）：
  工具链可用环境补跑（进入 APS 仓库目录调用，输出写 `.work/tmp/`）。
- **qa/runners/ 可执行化同步欠账（沿袭）**：旧断言残留——`suite_tui_display.py`
  QA-TD-14 版本号断言、`suite_ui_conns.py` 头部版本号断言、四处 `添加下载任务`
  对话框标题断言、`suite_modify_task.py`「默认 SHA-256」注释头、滚动条 `▲│█▼`
  字形断言、帮助 36 行/七组类断言、点外关闭类断言；影响面 = runners 与 v1.32 规格
  及 v138 实现不符。**本轮核证：活跃数相关正则（suite_ui_conns.py `活跃 (\d+)` 等）
  为非锚定 re.search，v139 前导空格改动不影响匹配，无新增欠账**。移交建议：QA 轮一并承接。
- **v147 时序 flake（沿袭）**：`sidecar_not_written_within_first_500ms` / `hardening_g_v147`
  偶发（本轮全量跑中该目标绿，与登记行为一致）。建议 QA 轮放宽 `wait_state` 预算或加重试。
- **Rust 工具链沙箱不持久（沿袭登记）**：本轮已按 rust.md 配方重建——stable 1.99.1
  （USTC dist + rsproxy-sparse）+ **nightly 1.101.0-nightly (07e526db9 2026-10-19)**
  （项目 rust-toolchain.toml 钉 nightly；新沙箱冷启动必须先一次性装全 nightly 再跑
  cargo，防 shim 自动安装被超时打断成修复死循环——本轮已沉淀 rust.md）。
- **沙箱磁盘预算（本轮实证，移交知悉）**：ezr 全量构建 target 4.4G + demo 0.6G +
  registry 0.5G 可撑爆 10G 沙盘；本轮清 `target/debug/incremental` 释放 2G 后完成
  demo clippy（沿袭 rust.md「构建目录膨胀」条目口径，构建前查 `df -h /`）。
- **ezr-demo 参照仓库 fmt/clippy 历史漂移（本轮登记，建议 cleaner 轮裁决是否收编）**：
  demo 源码存在与当前 nightly rustfmt/clippy 不符的历史漂移（多行折叠/展开差异 +
  数条 lint），均不在本轮改动行内；demo 为设计参照、不进 ezr 验证门槛，本轮不扩面
  修复。如需对齐，建议独立清理轮承接（注意 demo 不受 rust-toolchain.toml 约束，
  需单独核其工具链姿态）。
- **观察受限突变体（沿袭 hardender 登记，仍开放）**：workers.rs 5 条 + 等价 download.rs:85:27。
- **ezr-proxy.rs 测试基建（沿袭裁决）**：127 变异点不拆、CRAP/CPD 读数不追责。
- **退出竞态修复的可观测边界（沿袭 v137 登记）**：极端时序下毫秒级鼠标事件残留属业界共有限界。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D38、v138/v140 口径、D35 结案收窄、D38 登记差异
  （维持产品口径，待操作者裁决）、D39（About 版本文案照录 demo）。
- **本轮零新增实现定义值**：活跃计数标题 padding 形态（前后各 1 空格）为操作者
  指令直接口径 + 与左标题 ` 并发连接 ` 既有形态对称，非自由裁量。
- **demo-only 差异登记（沿袭沿用）**：添加对话框预填演示数据（ADD_POOL 轮换）与
  并发面板「客户端/协议」两列演示数据生成为 demo 演示辅助；正式版空字段/数据源
  phase-02 定夺（本轮零改动）。

## 二、当前产出情况（coder v139-m3-padding-fix）

### 本会话产物清单
- `project/ezr/src/ui/conns.rs`：①新增 `active_title(active)` 模块级单源构造函数
  （`format!(" 活跃 {active} ")`——前导空格 + 尾随空格各 1 列，右对齐下右侧离边框
  1 列、左侧留白 1 列，与左标题 ` 并发连接 ` 前后各 1 空格对称）；②`draw_conns`
  右上角标题改经该函数（原 `format!("活跃 {} ")` 仅尾随空格、左侧贴内容区）；③
  测试模块新增 `active_title_padded_both_sides` 形态守卫（`assert_eq!` 字面量，
  防字面量漂移回归）。
- `ezr-demo/src/ui.rs`（操作者指令同步修改，保持 demo 与实现逐字一致）：
  `draw_conns` 活跃计数标题 `format!("活跃 {} ")` → `format!(" 活跃 {} ")`；
  修订-3 注释补 v139 padding 口径注记。
- `ezr-demo/docs/ezr-demo UI界面及交互设计规范.md` §5.4：右上角 `` `活跃 N ` ``
  → `` ` 活跃 N ` ``（DIM，前后各 1 空格：左右 padding 对称，v139 修订注记——
  原形态仅尾随空格致左侧贴边）。
- `packs/_common/notes/rust.md` 经验沉淀 2 条：①§1 工具链与环境——rust-toolchain.toml
  钉 nightly 的沙箱冷启动顺序（shim 自动安装被打断 → recovering 修复死循环，先前台
  一次性装全 nightly）；②§9 终端与 TTY——右对齐标题前导空格在渲染 buffer 上不可
  判定（显式空格与默认空白同形，contains 零判定力），形态守卫上移到单源构造函数。
- 工作区侧（随归档）：`.work/plan.md`（8 步全勾选）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| ezr 全量测试 | **4009P / 0F**（全 15 目标：bin ezr 532 + ezr-fixture 30 + ezr-proxy 15 + tests/ 12 套件 3432；基线对账见下） | `cargo test --bins` + `cargo test --tests`（分批等价于 `cargo test --all-features`；flaky 目标 hardening_g_v147 本轮 373P/0F 一次过） |
| ezr clippy | 0 警告（all/pedantic/nursery 全开） | `cargo clippy --all-features --all-targets` |
| ezr fmt | clean（fmt 归并 1 处换行后复核） | `cargo fmt --check` |
| ezr-demo 测试 | 2P / 0F | `cargo test`（demo 目录） |
| TDD 红绿证据 | 新守卫测试先红（E0425 `active_title` 未定义）→ 实现后绿 | `cargo test --bin ezr ui::conns`（6P/0F） |
| runner 影响面核证 | qa/runners 活跃数正则为非锚定 `re.search(r"活跃 (\d+)")`，v139 前导空格不影响匹配 | 人工核读 suite_ui_conns.py:512 |

### 对账口径（如存在基线）
- 测试基线：上轮 handoff 记 3963P/0F——该口径未含 ezr-fixture/ezr-proxy 两 bin 目标
  的 45 个测试（3963 + 45 = 4008）；本轮补齐全 15 目标口径 **4009P/0F** = 4008 + 1
  （本轮唯一新增测试 `active_title_padded_both_sides`；0F，无用例删除）。
- 规格面对账：features/qa 零改动（本轮纯 UI 细节修复轮，FR-01-96「标题右侧显示
  活跃 x」文案不受 padding 形态影响）；mission 零改动（coder 只读纪律）。

### 待办与待批（未决项）
- **沿袭待裁决（均见遗留受限项）**：arch 规则 14 白名单扩展（cleaner/architect 轮）、
  qa/runners 同步欠账（QA 轮）、APS 机检补跑、v147 flake 预算放宽。
- **待操作者裁决（本轮新增）**：ezr-demo 参照仓库的 fmt/clippy 历史漂移是否收编
  （登记如上；demo 非验证对象，默认不动）。
- **工具链注记（知悉）**：本轮重建 stable 1.99.1 + nightly 1.101.0-nightly
  (07e526db9 2026-10-19)；沙箱磁盘预算紧张时先清 `target/debug/incremental`。

### 移交建议
- 本轮为单点小步修复，UI 改动面已闭环（实现 + 守卫 + demo + 文档四处同步）。
  可径直运行 `six-pack/cleaner`（以本 handoff 为上游交接内容）承接沿袭清单：
  ①DRY/重复面检视（四面板 ScrollablePanel 接线、sb_press Head/Tail 对称臂、NONE
  拦截三入口）；②arch 规则 14 白名单扩展裁决（`scroll|pos|follow`，17 行存量）；
  ③（新增，待操作者批准）demo 参照仓库 fmt/clippy 漂移收编裁决。
- 其后按链路 architect → hardender → QA（QA 轮顺带承接 qa/runners 同步欠账与
  v147 flake 预算放宽）。移交仅为建议，不构成自动流转（宪法第一章）。

By coder.
