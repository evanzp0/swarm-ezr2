# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner　会话：ezr-v150-clean（2026-10-24）
> 上游交接：project/handoff.md（coder v150-quitmouse-impl 交接：退出路径鼠标残序根治，
> ezr 与 ezr-demo 同步交付）
> 当轮任务：v150-quitmouse-impl 清理批次——保持行为不变的清理、覆盖率提升、
> CRAP/DRY 审查与变异点扫描（规格面零改动）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.34，分期模式）+ `project/mission/phase-01.md`（v1.34）。
- 本轮后源码状态：v1.34 全条款落地 + 历轮 UI/加固/覆盖轮 + 输入框组件化 +
  退出路径鼠标残序根治（v150）+ **本轮清理批次**（保持行为，测试 +4，
  全量 **4518P/0F**；规格面不变，零条款改动）。

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
| coder（v150-quitmouse-impl） | 退出路径鼠标残序根治：restore_into 重排 + settle/drain 窗口 + bool 契约 + ezr-demo 全量同步 + TimedWriter 时序锁定测试 | 已交付 | ezr：4486P/0F、clippy 0、fmt clean；ezr-demo：20P/0F、回归 8 套件全绿 |
| **cleaner（v150-clean，本轮）** | **①覆盖率提升：mouse.rs 弹窗命中臂 +4 测（about/val/help/dialog-[✗] 全臂，纯追加 mouse_tests）；②CRAP 门禁 6→2（on_mouse_about/val/help/dialog 4 条经补测退出）；③hit 命中闭包单源化 rect_hit（dialog/main/val_rects 三处，行为不变）；④lint 警告清零复核（clippy 0、fmt clean）；⑤CRAP/DRY/变异点三项度量基线重建（口径见当前节）** | **已交付** | **ezr：4518P/0F（15 目标，+32=+4×(1+7 挂载 crate)）、clippy 0、fmt clean；覆盖率 95.1% 行（开工 94.4%）、mouse.rs 80.4%→90.8%** |

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
- **ezr-proxy 测试基建豁免（沿袭登记）**：本轮延续——ezr-proxy.rs 覆盖率 82.0%、
  CRAP 门禁残留 handle_socks5（CRAP 80.2）随此豁免登记。
- **CRAP>30 残余 2 处（本轮口径，移交 hardender）**：`on_mouse_main`
  （app/mouse.rs:405，comp 37、cov 58.8%、CRAP 132.9——分发核心，comp=分支数，
  保持行为拆分依赖模块边界归 architect 裁决，登记口径不硬凑）+ `handle_socks5`
  （ezr-proxy.rs:342，随 ezr-proxy 豁免）。
- **孤儿 fixture `project/ezr/.tmp`（本轮发现，移交裁决）**：hardender 轮
  `.gitignore` 删除时误入库的 sidecar 快照 JSON（512KB mock 元数据），src/tests
  零消费点（rg 全量核查）；删除动作影响归档内容，留操作者/下轮裁决。
- **D40 / PC-09-场景09 待操作者裁决（沿袭）**：状态不变。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D43、MOUSE_SETTLE=50ms / MOUSE_SETTLE_RETRY=20ms、
  drain poll 超时 5ms（v150 轮登记）。本轮零新增（保持行为轮）。

## 二、当前产出情况（cleaner v150-clean）

### 本会话产物清单
- **测试补齐（纯追加，`src/app/mouse.rs` mouse_tests 模块末尾 +4）**：
  `about_window_mouse_paths`（关于窗口命中臂：拖拽/[✗] 关闭/点外无操作/页脚按压清除）、
  `val_popup_mouse_paths`（字段值弹窗命中臂：拖拽/滚轮钳制/[✗] 关闭位置即清）、
  `help_window_mouse_paths`（帮助窗口命中臂：滚轮 ±1/拖拽/[✗] 关闭）、
  `dialog_close_btn_cancel_paths`（对话框 [✗] 取消臂 + 下拉展开守卫臂）。
- **重复消解（产品区，行为不变）**：`rect_hit<T: Copy>` 单源函数
  （src/app/mouse.rs:625）收口 dialog/main 两处同形命中闭包 + detail_val_rects
  内联同谓词 find，共 3 处调用点原地一行委托。
- **测试夹具单源**：`open_about_with_rects` 助手收口 about 测试首开/重开
  两处同形脚手架（避免新增测试自配对 CPD tile）。
- **度量基线重建（口径与产物落 `.work/tmp/`，会话终结随 complete 清除，口径登记如下）**：
  - 覆盖率/CRAP：`cargo llvm-cov --no-clean --branch` 分块跑全量（lcov-final.out）；
    CRAP 受限近似解析器（crap.py）沿上游口径（BRDA→comp=1+边数/2、DA→cov、
    函数级同行归并、测试模块双形态排除），本轮补齐三处解析器边界（同行闭包/具名
    FN 集合存储、泛型实例化副本按行归并、测试模块函数保留为跨距边界）。
  - DRY：PMD CPD 7.28.0（`-l rust --minimum-tokens 50`，src 口径，文本渲染器），
    134 tiles = 产品区 34 + 测试区 99 + 混合 1（产品/测试区按文件内首个
    `#[cfg(test)] mod` 行号分解；property_tests.rs 整文件按声明点归测试区）。
  - 变异点：`cargo mutants --list --line-col=true` 全量 3219 点（27.1.0）。
- **经验沉淀（packs/_common/）**：notes/rust.md +3 条（rustup component add 须带
  镜像 env、PMD CPD XML 渲染器 xmlresolver 依赖、CRAP 解析器 FN 记录三处边界）、
  engineering.md +1 条（同形≠同序：合并重复块前核对检查顺序契约）。

### 验证证据

| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| ezr 全量测试 | **4518P / 0F**（15 目标；基线 4486 → +32） | `cargo test -- --test-threads=2`（project/ezr；v115 目标单独补跑，535P） |
| ezr clippy | 0 警告（全目标） | `cargo clippy --all-targets` |
| ezr fmt | clean（nightly） | `cargo fmt --check` |
| 覆盖率（插桩全量） | 4518P/0F、全仓行覆盖 **95.1%**（开工 94.4%） | `cargo llvm-cov --no-clean --branch …`（15 目标分块）+ `cargo llvm-cov report --lcov` |
| CRAP 门禁 | **>30 共 2 条**（开工 6 条） | `python3 .work/tmp/crap.py .work/tmp/lcov-final.out`（会话内可复现） |
| DRY | src 口径 134 tiles（开工 138，-4） | `.tools/pmd-cpd/cpd.sh cpd -l rust --minimum-tokens 50 --dir src` |
| 变异点扫描 | 全量 3219 点（开工 3239，-20） | `cargo mutants --list --line-col=true` |
| 验收测试 | 不适用 | cleaner 不运行 QA 套件（SKILL 职责边界） |

### 对账口径（与 v150 coder 基线）
- 测试计数 4486 → 4518（**+32**）：本轮新增 4 测 ×（bin 1 + 挂载 app 树的
  hardening crate 7）= 32——挂载重复计入为机制固有（notes/rust.md「挂载点测试计数
  对账法」），无删改既有测试。
- CPD 138 → 134（-4）：hit 闭包单源化消解 3 tiles（94tok×1 + 69tok×2）+
  自新增测试夹具 tile 1；变异点 3239 → 3219（-20）：重复分支点随 rect_hit
  单源化收敛（mouse.rs 158→138，其余文件不变）。
- CRAP 门禁 6 → 2：on_mouse_about（56→7）、on_mouse_val（110→10）、
  on_mouse_help（42→≤30）、on_mouse_dialog（38→≤30）经补测退出；
  on_mouse_main comp 43→37（分支点随单源化外移）。

### 待办与待批（未决项）
- **沿袭项全部不变**（见第一节遗留受限项）；本轮无新增阻塞。
- **孤儿 fixture `project/ezr/.tmp` 处置待裁决**（删/留；无消费点证据已列总账）。
- **ezr-demo 同形移植待裁决**：本轮 rect_hit 单源化与 +4 测属 ezr 内部清理；
  ezr-demo 在 `project/` 工作区外（宪法第三章工作区边界），未同形移植——
  demo 侧 app.rs 保留自身 3 处 hit 闭包形态（行为不受影响）；如需保持两仓
  形态一致，移交 coder 轮移植（移植面：rect_hit 等价助手 + 调用点）。

### 移交建议
- 建议下一步运行 `six-pack/architect`：on_mouse_main（CRAP 132.9，comp=37）的
  保持行为拆分需先收模块边界（architect 职责）；变异点 >100 的批次文件
  （app/mod.rs 181、mouse.rs 138、textfield.rs 125、ui/text.rs 101）同属
  模块边界裁决范畴，architect 定案后 hardender `--file` 分块兑现收益。
- `six-pack/hardender` 可对本轮 rect_hit 单源函数与 settle/drain 纯逻辑做
  变异加固（v150 交接建议继续有效）。
