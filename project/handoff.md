# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-v135-impl-fieldwin（2026-10-25）
> 上游交接：project/handoff.md（specifier v135-pieces-sync 交接：v1.35 规格修订待实现 +
> v151-arch 移交 field_window 振荡缺陷）
> 当轮任务：①v1.35 pieces 布局实现（68×20 八段、31 块/行、341/屏）；
> ②field_window 超宽取窗不动点振荡缺陷修复（v151-arch 移交，product-reachable）+
> 属性论域扩回全字母表。两项均已交付。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.35，分期模式）+ `project/mission/phase-01.md`（v1.35）。
- 本轮后源码状态：v1.34 全条款落地 + 历轮 UI/加固/覆盖轮 + 输入框组件化 +
  退出路径鼠标残序根治（v150）+ 清理批次（v150-clean）+ 架构轮（v151-arch）+
  v1.35 规格修订（specifier）+ **本轮 v1.35 实现 + field_window 振荡缺陷修复（coder）**
  ——全量 **4548P/0F**（`--test-threads=2`，15 测试目标）。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.35 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | v132 轮 3 份 + v149 轮 5 份 feature APS 机检沿袭未跑 |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F |
| architect（v145-arch） | supervisor 六文件拆分 + 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 | 已交付 | 3363P/0F |
| QA（ezr-qa-final / v148-qa 等） | 13+1 套件 e2e 可执行化 + llvm-cov 覆盖轮 | 已交付 | e2e 167P/0F（v148 基线态） |
| coder（v129–v140 系列） | FR-01-106 实现 + v1.30/v1.31 UI 改版 + FR-01-112 内置 DNS | 已交付 | 4162P/0F |
| cleaner（v141）/ architect（v146） | dns 清理 + 四阶段评审 | 已交付 | 4200P/0F、arch 15/15 |
| coder（v134-demo-sync-impl） | v1.34 全条款实现（FR-01-113…116 + 修订十二条） | 已交付 | 4300P/0F |
| coder（v134-uifix 系列 ×3） | 三类 UI bug 修复 + 输入框原生光标 | 已交付 | 4388P/0F、clippy 0、fmt clean |
| coder（v135-fieldcomp-impl） | ①输入框组件化（src/textfield.rs）；②ezr-demo 同步 | 已交付 | ezr：4484P/0F；ezr-demo：18P/0F + 回归 8 套件全绿 |
| coder（v136/v137 中间轮） | 退出路径首轮防泄漏 | 已交付 | 操作者实测仍复现（v150 根因所据） |
| coder（v150-quitmouse-impl） | 退出路径鼠标残序根治 + ezr-demo 全量同步 + TimedWriter 时序锁定测试 | 已交付 | ezr：4486P/0F、clippy 0、fmt clean；ezr-demo：20P/0F、回归 8 套件全绿 |
| cleaner（v150-clean） | mouse.rs 弹窗命中臂 +4 测、CRAP 门禁 6→2、rect_hit 单源化、度量基线重建 | 已交付 | ezr：4518P/0F、clippy 0、fmt clean；覆盖率 95.1% 行 |
| architect（v151-arch） | 模块边界收口（scrollbar.rs）+ on_mouse_main 拆分 + 属性 71 + arch 16 规则 | 已交付 | ezr：4532P/0F（15 目标）、clippy 0、fmt clean、arch 16/16、属性 71P/0F；变异点 3232 |
| specifier（v135-pieces-sync） | v1.35 规格修订：pieces 弹窗布局对齐 ezr-demo README 记录 21+22（68×20/八段/31 块/341，逐字断言经 demo 源码核证） | 已交付 | 文档与源码一致无差异登记；沿袭测试基线 4532P/0F 不变 |
| **coder（v135-impl-fieldwin，本轮）** | **①v1.35 pieces 布局实现：PIECES 64×21→68×20（内框 66×18）、六段→八段（网格上下呼吸行）、网格 14→11 行、每行 29→31 块（视口 62 列整除）、一屏 406→341——实现面集中 ui/pieces.rs（常量 + Layout 八段 + rows[4]/rows[6] 索引 + 文档），cols/前沿/钳制公式天然跟随；②field_window 振荡缺陷修复：textfield.rs 超宽取窗循环改「原退出条件优先（对既有收敛输入行为零变化）+ 窗口重现破振荡（配输出不变量守卫）+ 硬上限双指示符保守收口」，终止性论证落实现注释；属性论域扩回全字母表（field_text_narrow 退役，slice_identity/budget_conserved 切 field_text_full，恰满预算等式保留窄字符门槛）+ 确定性回归 2 测（最小复例 + 振荡族三锚位 + 产品可达 29 宽字符预算 59）** | **已交付** | **ezr：4548P/0F（15 目标，--test-threads=2）、clippy 0（-D warnings）、fmt clean、arch 16/16；属性 3 测全字母表 5 万例复跑全绿；TDD 红证据留存（最小复例修复前挂死 timeout 击杀、v1.35 断言修复前 5F）** |

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
  drain poll 超时 5ms。本轮零新增（pieces 几何数字 68×20/31/341/11 均为规格钦定值
  经 demo 源码核证，非实现定义值；field_window 修法为缺陷修复、不涉产品契约值）。

## 二、当前产出情况（coder v135-impl-fieldwin）

### 本会话产物清单
- **`ezr/src/textfield.rs`（field_window 振荡缺陷修复）**：超宽取窗不动点循环重构——
  取窗收敛为 `window_for` 闭包单源；退出两路：①`need == reserve`（原不动点口径，
  对既有收敛输入行为零变化）；②窗口 `(lo,hi)` 重现且 `内容宽 + 指示符数 ≤ 预算`
  （振荡破除 + 不变量守卫的保守收口）；另设 8 步硬上限兜底（超限按双指示符预留
  重取，不变量无条件成立）；终止性论证（need ∈ {0,1,2} ⇒ 预算至多 3 取值 ⇒ 窗口
  至多 3 取值 ⇒ 必现重现）落实现注释。新增确定性回归 2 测：最小复例（宽度序列
  (1,2,1) 锚 1 预算 3，修复前挂死）+ 振荡族（k=1..15 三锚位 + 产品可达 29 宽字符
  预算 59）。
- **`ezr/src/property_tests.rs`（论域扩回）**：v151-arch 收窄登记退役；`field_text_narrow`
  退役删除；`prop_field_window_slice_identity` / `prop_field_window_budget_conserved`
  论域切全字母表（ASCII + CJK + VS15）；「恰满预算」等式断言保留窄字符域门槛
  （宽字原子损耗下取等不成立，不等式覆盖全字母表）。
- **`ezr/src/ui/pieces.rs`（v1.35 布局实现）**：`PIECES_W/H` 64×21→68×20；Layout
  六段→八段（网格上下各插 1 条 `Length(1)` 呼吸行）；`grid = rows[4]`、图例
  `rows[6]`；模块/行内注释同步 v1.35 口径（内框 66×18、视口 62 列、31 块/行、
  341/屏、两步演进链）。cols/前沿定位/滚动钳制公式无改动（由内框尺寸自然跟随）；
  7 个渲染单测断言全面对齐 v1.35（居中 x=26/y=7、网格屏行 12、小终端网格屏行 5、
  前沿 scroll=4、End 钳制 9、滚动条右边框列 x=93、padding/呼吸行逐格空行核验）。
- **`packs/_common/engineering.md`（经验沉淀）**：「不动点退出谓词」条追加——
  状态/窗口重现破振荡必须配输出不变量守卫（同周期内各重现点并非都安全）+
  「周期内必存在安全点」一句话论证要求 + 缺陷修复后收窄论域扩回复跑成对闭环。
- **`packs/_common/notes/rust.md`（经验沉淀）**：新增 `#[path]` 源包含收编条——
  src cfg(test) 单测跨收编目标重复注册，全量计数 = 各目标之和、新增 1 测增量 =
  收编目标数（对账防误判，基线随收编目标清单漂移）。
- **环境（沙箱重建，随交接）**：rustup 1.29.2 + nightly 1.101.0-nightly
  （2026-10-24，rustfmt/clippy 组件齐）经 rsproxy.cn 镜像安装
  （`RUSTUP_DIST_SERVER`/`RUSTUP_UPDATE_ROOT`）；`~/.cargo/config.toml` 已配
  rsproxy-sparse crates 镜像。新 shell 需 `export PATH="$HOME/.cargo/bin:$PATH"`
  补注入（notes/rust.md「PATH 注入不跨会话持久」）。

### 验证证据

| 验证项 | 结果 | 命令/方法（可复现） |
|---|---|---|
| TDD 红证据（振荡缺陷） | 最小复例修复前挂死（timeout 150s 击杀，EXIT=124）；v1.35 断言修复前 5F/2P | `cargo test --bin ezr field_window_oscillation -- --test-threads=1`（修复前）；`cargo test --bin ezr ui::pieces::`（修复前） |
| 全量测试 | **4548P / 0F**（15 目标） | `cargo test -- --test-threads=2`（project/ezr） |
| 增量对账 | 4532 → 4548 = +16 = 新增 2 测 × 8 个 `#[path]` 收编 textfield.rs 的测试目标（逐目标 delta 核对自洽） | `grep "^test result:" <log>` 逐目标比对（baseline.log / full1.log） |
| 属性论域扩回 | 3 属性（slice_identity / budget_conserved / fits_shows_all）全字母表 **5 万例 ×3** 全绿 | `PROPTEST_CASES=50000 cargo test --bin ezr prop_field_window -- --test-threads=2` |
| clippy 零警告 | 0（-D warnings 门禁） | `cargo clippy --all-targets --all-features -- -D warnings` |
| fmt | clean | `cargo fmt -- --check` |
| 架构护栏 | 16/16 规则通过 | `bash scripts/arch_check.sh`（project/ezr） |
| v1.35 数字对账 | 68×20（内框 66×18）、八段、31 块/行（62 列 ÷2 整除无余列）、一屏 341 = 11×31、前沿 scroll 4、End 末页 9 | ui/pieces.rs 7 渲染单测（TestBackend 逐格断言） |
| 退役标识反向自检 | 旧口径（64×21 现行态/406/每行 29/网格 14 行）全 src 检索零漏改点（命中均为历史修订注记或无关模块同名局部） | `grep -rn "64×21\|406\|每行 29\|网格 14 行" src/` |

### 对账口径（与 v151-arch / v135-pieces-sync 基线）
- 测试计数：4532（15 目标）→ 4548 = +2 新测 × 8 收编目标（textfield.rs）；
  属性测试数不变（3 个，论域扩回非新增）；arch 规则 16/16 不变。
- 源码触碰面：`src/textfield.rs`、`src/property_tests.rs`、`src/ui/pieces.rs`
  三文件；ezr-demo 零改动；features/qa/mission 零改动（纯实现轮）。

### 待办与待批（未决项）
- **沿袭项全部不变**（见第一节遗留受限项）；本轮无新增阻塞、无新增待批。
- field_window 振荡缺陷已闭环（v151-arch 待办第一项销账）：修复 + 论域扩回 +
  全量绿，hardender 轮可按 v151-arch 移交建议兑现 `--file` 分块加固
  （textfield.rs 振荡回归 2 测可作为加固基线的一部分）。

### 移交建议
- 建议下一步运行 `six-pack/cleaner`：本轮触碰面（textfield.rs 取窗重构、
  ui/pieces.rs 布局）尚未过保持行为清理/覆盖率/CRAP/DRY 审查；cleaner 轮可顺带
  复核 `window_for` 闭包与 `fill_cols` 的职责划分是否需上提。
- 或按 v151-arch 原计划跳 `six-pack/hardender`：变异点批次文件口径（scrollbar.rs
  190 / mouse.rs 151 / keys.rs 137 / textfield.rs 125 / app/mod.rs 108 / ui/text.rs 101）
  继续有效；本轮 textfield.rs 净增约 40 行（新增 2 测 + 循环重构），变异点计数
  变化待 hardender 扫描落账。
- v151-arch 移交建议中「field_window 振荡缺陷先行修复」已兑现，其余建议不变。
