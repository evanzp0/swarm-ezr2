# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect　会话：ezr-v151-arch（2026-10-24）
> 上游交接：project/handoff.md（cleaner v150-clean 交接：mouse.rs 弹窗命中臂补测、
> rect_hit 单源化、CRAP/DRY/变异点三项度量基线重建）
> 当轮任务：v151-arch 架构轮——四阶段架构评审、模块边界收口、on_mouse_main
> 保持行为拆分、属性测试补齐、架构护栏扩充（规格面零改动）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.34，分期模式）+ `project/mission/phase-01.md`（v1.34）。
- 本轮后源码状态：v1.34 全条款落地 + 历轮 UI/加固/覆盖轮 + 输入框组件化 +
  退出路径鼠标残序根治（v150）+ 清理批次（v150-clean）+ **本轮架构轮**
  （保持行为，全量 **4532P/0F**；规格面不变，零条款改动）。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.34 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | v132 轮 3 份 + v149 轮 5 份 feature APS 机检沿袭未跑 |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F |
| architect（v145-arch） | supervisor 六文件拆分 + 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 | 已交付 | 3363P/0F |
| QA（ezr-qa-final / v148-qa 等） | 13+1 套件 e2e 可执行化 + llvm-cov 覆盖轮 | 已交付 | e2e 167P/0F（v148 基线态） |
| specifier（v129–v149） | FR-01-106…116 + v1.34 规格轮 | 已交付 | v149 轮逐字断言经 ezr-demo 核证 |
| coder（v129–v140 系列） | FR-01-106 实现 + v1.30/v1.31 UI 改版 + FR-01-112 内置 DNS | 已交付 | 4162P/0F |
| cleaner（v141）/ architect（v146） | dns 清理 + 四阶段评审 | 已交付 | 4200P/0F、arch 15/15 |
| coder（v134-demo-sync-impl） | v1.34 全条款实现（FR-01-113…116 + 修订十二条） | 已交付 | 4300P/0F |
| coder（v134-uifix 系列 ×3） | 三类 UI bug 修复 + 输入框原生光标 | 已交付 | 4388P/0F、clippy 0、fmt clean |
| coder（v135-fieldcomp-impl） | ①输入框组件化（src/textfield.rs）；②ezr-demo 同步 | 已交付 | ezr：4484P/0F；ezr-demo：18P/0F + 回归 8 套件全绿 |
| coder（v136/v137 中间轮） | 退出路径首轮防泄漏 | 已交付 | 操作者实测仍复现（v150 根因所据） |
| coder（v150-quitmouse-impl） | 退出路径鼠标残序根治 + ezr-demo 全量同步 + TimedWriter 时序锁定测试 | 已交付 | ezr：4486P/0F、clippy 0、fmt clean；ezr-demo：20P/0F、回归 8 套件全绿 |
| cleaner（v150-clean） | mouse.rs 弹窗命中臂 +4 测、CRAP 门禁 6→2、rect_hit 单源化、度量基线重建 | 已交付 | ezr：4518P/0F、clippy 0、fmt clean；覆盖率 95.1% 行 |
| **architect（v151-arch，本轮）** | **①模块边界收口：滚动条交互状态机自 app/mod.rs 收口至 `app/scrollbar.rs`（SbKey/SbHold/sb_* 方法族 + 4 单测随迁；门面 re-export 保持 `crate::app::SbKey` 消费路径，ui/keys/mouse 零改动）；②on_mouse_main 保持行为拆分：分发核 + Down 命中链（footer/tab/detail_hits/conns/list 五段）+ Up + 滚轮各自成函数，`copy_detail_value` 四字段复制尾段单源；新增主界面命中链优先级锁定测试；③属性测试 65→71（+6：取窗切片恒等/预算守恒/整值可见/点击映射封闭/走马灯恒宽周期/wrap 损耗与行宽界）；④arch_check 15→16 规则：规则 16 engine/model 渲染框架禁入 + 规则 14 白名单扩展（dlg_field_wins/dlg_cursor_screen/val_scroll——v1.34 轮已入库的合法回填），均阳性对照实测；⑤边界裁决：textfield.rs 与 ui/text.rs 内聚不拆（单一概念/纯文本原语族，变异点密度非拆分信号）** | **已交付** | **ezr：4532P/0F（15 目标，+14 = +1 命中链锁定测 ×(1 bin + 7 挂载 crate) + 6 属性）、clippy 0、fmt clean、arch 16/16、属性 71P/0F（独立命令独立计数）；变异点 3219→3232（+13 = 拆分函数边界点）** |

### 遗留受限项（当前有效）
- **APS 机检沿袭未执行**：v132 修订 3 份 + v149 修订 5 份 feature 未过
  `gherkin-ir-dry-checker`；本沙箱 GitHub 不可达，待环境可用时一并补跑。
- **qa/runners/ e2e 套件未随 v1.34 同步（移交 QA）**：可执行套件仍为 v1.30–v1.32 口径，
  对 v1.34 改版场景会 F，属预期——待 QA 轮同步。
- **demo 回归长脚本沙箱卡顿（沿袭）**：verify_ezr / test_focus_style 连续全量跑在
  沙箱内中后段随机挂起，按场景子集分半跑全绿（v135/v150 两轮复证）；本轮未改动
  ezr-demo，未重跑 demo 回归（无触碰面）。
- **真实 TCP stub e2e 时序脆弱（v147 沿袭）**：`--test-threads=2` 全绿即环境竞争非回归。
- **dns.rs 覆盖率残余 3 行 / 观察受限突变体（沿袭登记）**：状态不变。
- **ezr-proxy 测试基建豁免（沿袭登记）**：ezr-proxy.rs 覆盖率 82.0%、
  handle_socks5（CRAP 80.2）随此豁免登记。
- **CRAP>30 残余口径更新（移交 hardender）**：`on_mouse_main`（comp 37、CRAP 132.9）
  **本轮已完成保持行为拆分**（分发核 comp ≈6，五段命中链函数各自 comp ≤8）——
  comp/cov/CRAP 重测口径待 hardender 轮 llvm-cov + crap 解析器复测落账（本轮未重建
  覆盖率度量管线，沿袭 cleaner 口径）；`handle_socks5` 随 ezr-proxy 豁免不变。
- **孤儿 fixture `project/ezr/.tmp`（沿袭，移交裁决）**：hardender 轮误入库的
  sidecar 快照 JSON（512KB），src/tests 零消费点；删除动作影响归档内容，留操作者/下轮裁决。
- **D40 / PC-09-场景09 待操作者裁决（沿袭）**：状态不变。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D43、MOUSE_SETTLE=50ms / MOUSE_SETTLE_RETRY=20ms、
  drain poll 超时 5ms。本轮零新增（保持行为轮；属性论域收窄为测试口径非产品契约）。

## 二、当前产出情况（architect v151-arch）

### 本会话产物清单
- **`src/app/scrollbar.rs`（新增，边界收口）**：滚动条交互状态机单源——SbKey/SbHold
  类型 + sb_max/sb_scroll/sb_set/sb_page/sb_press/sb_drag_move/sb_release/sb_hold_tick/
  sb_arrow_pressed/sb_arrow_follow/sb_press_at 方法族 + sb_interaction_tests 4 测随迁
  （纯机械迁移：私有方法按消费面最小可见性 pub(super) 化，有效可见域不变）；app/mod.rs
  相应删减并以 `pub use scrollbar::{SbHold, SbKey}` 门面 re-export（消费路径零改动）。
- **`src/app/mouse.rs`（拆分 + 锁定测试）**：on_mouse_main 分发核保持行为拆分——
  `on_mouse_main_down`（页脚→页签→详情热区→滚动条→并发→列表命中链，命中即消费）、
  `on_mouse_main_up`、`on_mouse_main_scroll`、`main_down_footer/tab/detail_hits/conns/list`
  各段成函数；`copy_detail_value` 收口四个字段名复制臂同形尾段；新增
  `main_down_dispatch_chain_priority` 锁定测试（命中链优先级 + 页脚按压清除 + 无值聚焦臂）。
- **`src/property_tests.rs`（+6 属性）**：取窗切片恒等、预算守恒（窄字符超宽域取等）、
  整值可见域（CJK 全字母表）、点击映射封闭性、走马灯恒宽 + 周期性、wrap 损耗与行宽界；
  新增 `field_text_narrow`/`field_text_full` 论域策略（字母表卫生：排除 `…` 指示符歧义）。
- **`src/ui/text.rs` / `src/ui/mod.rs`**：wrap_text/marquee_bar 可见性 pub(super)→pub(crate)
  （crate 内属性测试经 ui 根 cfg(test) 门面消费，产品 API 面零增量）；门面清单扩展。
- **`scripts/arch_check.sh`**：规则 16（engine/model 渲染框架禁入）+ 规则 14 白名单扩展
  （dlg_field_wins/dlg_cursor_screen/val_scroll——v1.34 原生光标与字段值弹窗轮已入库的
  合法渲染回填，沿袭 help/pieces_scroll 双写形态判例）；两处均阳性对照实测（探针行红 →
  移除全绿）。
- **`packs/_common/engineering.md`（经验沉淀 +2 条）**：①不动点循环退出谓词须论证单调
  收敛否则设迭代上限；②边界护栏脚本属每轮最终验证序列固定一环，跳轮即漂移不知情。

### 验证证据

| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| ezr 全量测试 | **4532P / 0F**（15 目标；基线 4518 → +14） | `cargo test -- --test-threads=2`（project/ezr；超时分块时末两个 hardening 目标单独补跑，536P+542P） |
| ezr clippy | 0 警告（全目标） | `cargo clippy --all-targets` |
| ezr fmt | clean（nightly） | `cargo fmt --check` |
| 架构检查 | **16/16 规则全绿**（规则 16 新增） | `bash scripts/arch_check.sh` |
| 属性测试 | **71P / 0F**（基线 65 → +6；独立命令独立计数） | `cargo test --bin ezr property_tests` |
| 变异点扫描 | 全量 3232 点（开工 3219，+13） | `cargo mutants --list --line-col=true`（27.1.0） |
| 验收测试 | 不适用 | architect 不运行 QA 套件（SKILL 职责边界） |

### 对账口径（与 v150-clean 基线）
- 测试计数 4518 → 4532（**+14**）：新增 `main_down_dispatch_chain_priority` 1 测 ×
  （bin 1 + 挂载 app 树的 hardening crate 7）= 8（挂载重复计入为机制固有，
  notes/rust.md「挂载点测试计数对账法」）+ 属性 6 测（仅 bin，property_tests 不挂载）；
  无删改既有测试。
- 变异点 3219 → 3232（**+13**）：app/mod.rs 181 → 108（−73，滚动条状态机外移）+
  app/scrollbar.rs +73（同码异位）+ mouse.rs 138 → 151（+13，拆分新增函数边界点）；
  textfield.rs 125 / ui/text.rs 101 不变。文件级口径：scrollbar.rs（根，几何引擎）190、
  mouse.rs 151、keys.rs 137、textfield.rs 125、app/mod.rs 108、ui/text.rs 101。
- arch 规则 15 → 16：规则 16 新增（零发现 + 阳性对照生效）；规则 14 白名单扩展三处
  均为 v1.34 轮已入库存量回填的判例对齐（非新增违规放行），扩展前实测三行即红。

### 待办与待批（未决项）
- **【缺陷移交 coder｜product-reachable】textfield 超宽取窗不动点振荡（本轮属性测试发现）**：
  `TextInput::field_window` 两轮不动点循环（src/textfield.rs:290 起）在「锚点（光标）
  落宽字符 + 双侧窄字符边界 + 特定预算」组合下预留量 1↔2 **振荡不终止**（TUI 挂死）。
  最小复例：宽度序列 (1,2,1)、锚 1、预算 3；(1,2…₂ₖ…2,1) 族失败预算 = 2k+1——产品
  可达路径：URL 字段输入中文串（如 `a` + 29 个宽字符 + `a`）且光标置于宽字串内 +
  添加/修改对话框取窗预算 avail ≈ 59 = 2×29+1。**修法建议**（coder 裁决）：退出条件
  改窗口区间 [lo,hi] 稳定判定或预留量单调化（`reserve = max(reserve, need)` 不充分，
  需配窗口稳定判定）；或设迭代上限 + 保守收口。修复后扩属性论域：本轮
  prop_field_window_slice_identity / budget_conserved 的溢出路径论域收窄为
  「窄字符 + 零宽（ASCII + VS15）」（宽字符域穷举 L≤11 + 20 万随机实证振荡存在），
  整值可见/点击封闭两属性保留 CJK 全字母表（预算与值耦合，恒走整值路径不进溢出循环）。
  修复的验收 = 论域扩回全字母表后属性全绿 + 全量单测全绿。
- **沿袭项全部不变**（见第一节遗留受限项）；本轮无新增阻塞。
- **孤儿 fixture `project/ezr/.tmp` 处置待裁决**（沿袭，删/留；无消费点证据已列总账）。

### 移交建议
- 建议下一步运行 `six-pack/hardender`：本轮边界收口后，变异点批次文件口径更新为
  scrollbar.rs（根，190）、mouse.rs（151）、keys.rs（137）、textfield.rs（125）、
  app/mod.rs（108）、ui/text.rs（101）——hardender `--file` 分块兑现收益；app/scrollbar.rs
  （73 点，状态机内聚单元）与 on_mouse_main 拆分后的五段命中链函数（各 comp ≤8、
  cov 待复测）是优先加固面；`main_down_dispatch_chain_priority` 锁定测试可直接作
  分发序变异锚点。
- **field_window 振荡缺陷为 product-reachable 挂死级，建议在 hardender 轮之前或同轮
  先行 `six-pack/coder` 修复**（见待办第一项；修复属行为变更，architect 职责边界外）。
- v150 交接建议继续有效：rect_hit 单源函数与 settle/drain 纯逻辑可做变异加固。
