# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier　会话：ezr-v135-pieces-sync（2026-10-25）
> 上游交接：project/handoff.md（architect v151-arch 交接：模块边界收口 scrollbar.rs、
> on_mouse_main 保持行为拆分、属性测试 65→71、arch 16/16、全量 4532P/0F）
> 当轮任务：v1.35 规格轮——pieces 分块弹窗布局对齐 ezr-demo README 变更记录 21+22
> （提交 817bdb5「demo:adjust pieces window w*h」），逐条经 ui.rs draw_pieces 源码核证；
> 纯规格修订轮，ezr 源码零改动。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.35，分期模式）+ `project/mission/phase-01.md`（v1.35）。
- 本轮后源码状态：v1.34 全条款落地 + 历轮 UI/加固/覆盖轮 + 输入框组件化 +
  退出路径鼠标残序根治（v150）+ 清理批次（v150-clean）+ 架构轮（v151-arch，保持行为，
  全量 **4532P/0F**）+ **本轮 v1.35 规格修订**（ezr 源码零改动，实现待 coder 轮落地）。

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
| architect（v151-arch） | 模块边界收口（scrollbar.rs）+ on_mouse_main 拆分 + 属性 71 + arch 16 规则 | 已交付 | ezr：4532P/0F（15 目标）、clippy 0、fmt clean、arch 16/16、属性 71P/0F；变异点 3232 |
| **specifier（v135-pieces-sync，本轮）** | **v1.35 规格修订：pieces 弹窗布局对齐 ezr-demo README 记录 21+22——窗口 64×21→68×20（呼吸行+加宽+高度收缩，内框 66×18）、布局六段→八段、网格 14→11 行、每行 29→31 块（视口 62 列整除）、一屏 406→341；features/05-ui-pieces.feature（01/04）+ qa/05-ui-pieces-qa.md（QA-UP-01/04 + 环境前置滚程 54 行）+ mission/phase-01 v1.35 建档 + FR-01-106②/FR-01-115④ 修订** | **已交付（规格待 coder 实现落地）** | **逐字断言经 ezr-demo ui.rs 源码核证（文档与源码一致，无差异登记）；退役旧口径全仓反向自检零漏改点；沿袭测试基线 4532P/0F 不变（源码零改动）** |

### 遗留受限项（当前有效）
- **APS 机检沿袭未执行**：v132 修订 3 份 + v149 修订 5 份 feature 未过
  `gherkin-ir-dry-checker`；本沙箱 GitHub 不可达，待环境可用时一并补跑。
- **qa/runners/ e2e 套件欠账（移交 QA）**：`suite_ui_pieces.py` 仍为 v1.30 口径
  （断言 64×19），05 规程已修订至 v1.35（68×20/31 块/341）——待 QA 轮随其余
  套件（v1.30–v1.32 口径对 v1.34/35 场景会 F，属预期）一并同步。
- **demo 回归长脚本沙箱卡顿（沿袭）**：verify_ezr / test_focus_style 连续全量跑在
  沙箱内中后段随机挂起，按场景子集分半跑全绿（v135/v150 两轮复证）；本轮未改动
  ezr-demo，未重跑 demo 回归（无触碰面）。
- **真实 TCP stub e2e 时序脆弱（v147 沿袭）**：`--test-threads=2` 全绿即环境竞争非回归。
- **dns.rs 覆盖率残余 3 行 / 观察受限突变体（沿袭登记）**：状态不变。
- **ezr-proxy 测试基建豁免（沿袭登记）**：ezr-proxy.rs 覆盖率 82.0%、
  handle_socks5（CRAP 80.2）随此豁免登记。
- **CRAP>30 残余口径（移交 hardender）**：`on_mouse_main` 拆分后 comp/cov/CRAP 重测
  口径待 hardender 轮 llvm-cov + crap 解析器复测落账（v151-arch 未重建覆盖率度量
  管线）；`handle_socks5` 随 ezr-proxy 豁免不变。
- **孤儿 fixture `project/ezr/.tmp`（沿袭，移交裁决）**：hardender 轮误入库的
  sidecar 快照 JSON（512KB），src/tests 零消费点；删除动作影响归档内容，留操作者/下轮裁决。
- **D40 / PC-09-场景09 待操作者裁决（沿袭）**：状态不变。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D43、MOUSE_SETTLE=50ms / MOUSE_SETTLE_RETRY=20ms、
  drain poll 超时 5ms。本轮零新增（纯规格同步轮；pieces 几何数字均经 demo 源码核证，
  非实现定义值）。

## 二、当前产出情况（specifier v135-pieces-sync）

### 本会话产物清单
- **`features/05-ui-pieces.feature`（01/04 修订 + v1.35 同步注）**：场景 01 窗口
  64×21→**68×20**、内容六段→**八段**（网格上下各空 1 行呼吸行）、Examples w/h 参数
  68/20；场景 04 每行 **31 块**（视口 62 列整除无余列）、一屏最多 **341 块**（网格
  **11 行**）；稳定序号不重排，历史修订注记保留。
- **`qa/05-ui-pieces-qa.md`（QA-UP-01/04 同步 + 环境前置 + v1.35 同步注）**：QA-UP-01
  弹窗 68×20/八段齐全；QA-UP-04 每行 31 块/一屏 341（11 行）；环境前置 piece-big.bin
  （2000 块）滚程按算式收敛 `总行数 = ⌈2000 ÷ 31⌉ = 65`、`滚程 = 65 − 11 = 54 行`
  （v1.34 环境前置行残留 v1.30 口径「一屏 14 行、滚程 51」一并按源码终态纠正）。
- **`mission.md`（追加 v1.35 注记）**：操作者指令建档，与 phase-01 v1.35 同步。
- **`mission/phase-01.md`（追加 v1.35 修订注记 + FR 现行条款修订）**：FR-01-106②
  窗口形态改 68×20（两步演进链 64×21→67×23→68×20 与核证出处落档）、内容八段、
  呼吸行条款；FR-01-115④ 补 pieces 尺寸链 v1.35 注记（现行口径指针回 FR-01-106）。
- **`packs/six-pack/specifier/SKILL.md`（经验沉淀 +1 条）**：修订纪律新增「修订牵动
  派生数字时同轮同步重算并写出算式」（跨项目纪律，含本轮 v1.34 环境前置残留旧口径
  的教训归并）。
- **环境（沙箱重建，随交接）**：rustup 1.29.2 + stable 1.99.1 + nightly
  1.101.0-nightly（2026-10-24，rustfmt/clippy 组件齐）经 rsproxy.cn 镜像安装
  （`RUSTUP_DIST_SERVER`/`RUSTUP_UPDATE_ROOT`）；`~/.cargo/config.toml` 已配
  rsproxy-sparse crates 镜像。新 shell 需 `export PATH="$HOME/.cargo/bin:$PATH"`
  补注入（notes/rust.md「PATH 注入不跨会话持久」）。

### 验证证据

| 验证项 | 结果 | 命令/方法（可复现） |
|---|---|---|
| 参照实现源码核证 | 全部字面断言与 `ezr-demo/src/ui.rs` draw_pieces 终态一致 | `PIECES_W=68 / PIECES_H=20`（ui.rs:2445-2446）；八行 Layout（ui.rs:2521-2531，grid=rows[4]、图例=rows[6]）；视口 66−4=62 列、cols=31（ui.rs:2534-2544）；一屏 11×31=341；README 记录 21/22 与规范 §4.2.6/§4.3/§6.5 逐条对源码复核一致 |
| 退役标识反向自检 | 旧口径（64×21/406/434/每行 29/网格 14 行/六段/滚程 51）全仓检索，全部命中均为历史修订注记或本轮演进链描述，零漏改点 | `grep -rn "64×21\|406 块\|434\|每行 29\|网格 14 行\|内容四段\|内容六段\|滚程 51" project/` |
| 规格面完整性 | 05-ui-pieces.feature 7 场景（08 退役留空）结构不变、稳定序号未重排；QA-UP-01..07 编号不变 | 逐文件 Read 复核 |
| 沿袭测试基线 | 4532P/0F 不变（本轮 ezr 源码零改动，未重跑） | 沿袭 v151-arch 口径 `cargo test -- --test-threads=2`（project/ezr） |

### 对账口径（与 v151-arch 基线）
- 纯规格轮：ezr 源码、测试计数（4532P/0F）、变异点（3232）、arch 规则（16/16）
  全部零变化；产物面仅 mission/features/qa/SKILL 四类文档 + 环境重建。

### 待办与待批（未决项）
- **【缺陷移交 coder｜product-reachable，v151-arch 沿袭】textfield 超宽取窗不动点振荡**：
  `TextInput::field_window` 两轮不动点循环在「锚点落宽字符 + 双侧窄字符边界 + 特定
  预算」下 1↔2 振荡不终止（TUI 挂死），product-reachable（URL 字段输中文串）。
  修法建议与扩属性论域验收口径见 v151-arch 待办原文（arch_check/属性测试
  归属不动）；**建议在 v1.35 实现轮之前或同轮先行修复**。
- **【实现移交 coder，本轮新增】v1.35 pieces 规格落地**：68×20（呼吸行八段布局、
  31 块/行、341/屏）尚待实现——ezr 源码当前仍为 64×21 v1.34 形态，coder 轮按
  FR-01-106② v1.35 修订条款实现 + 对应单测/快照调整。
- **沿袭项全部不变**（见第一节遗留受限项）；本轮无新增阻塞。

### 移交建议
- 建议下一步运行 `six-pack/coder`：①v1.35 pieces 布局实现（窗口 68×20、八行
  Layout 插入两条 Length(1) 呼吸行、视口宽随内框 66−4、cols 推导 31/341 自然跟随——
  实现面集中 `project/ezr/src/app/pieces.rs` 或 ui 侧 draw_pieces 对应物，含快照/
  单测数字对账）；②field_window 振荡缺陷修复（v151-arch 待办，product-reachable），
  可同轮或先行。
- v151-arch 移交建议（hardender 优先加固面、`--file` 分块口径）继续有效，待
  coder 轮完成后兑现。
