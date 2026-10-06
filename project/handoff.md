# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：coder-20261007-v116
> 版本注：操作者裁决「两缺陷不移交、直接修复」落地轮——缺陷①（v115 待办①）产品源码
> 修复 + 缺陷②（v115 待办②）测试隔离全类收口；上游 hardender v115 交接内容已消化。

## 速览（先读这一页）

- **项目**：EZR Downloader——终端里的多协议下载工具（Rust / TUI）。需求权威来源
  `project/mission.md`（v1.12）。
- **流水线走到哪**：six-pack 六角色顺序流水线
  specifier→coder→cleaner→architect→hardender→QA；前五棒已完成，本轮为操作者钦点的
  缺陷修复轮（coder），下一棒 QA。
- **本轮（coder v116）做了什么**：①修复 26 列窄终端代理下拉越界渲染 panic——
  `dropdown_rect` 浮层宽/高一律钳制在终端 area 内（横向为报告触发点，纵向同缺陷
  分支一并钳制），选项行渲染按钳后内框截断；②修复测试隔离缺陷——v115 待办②的
  make_app 固定 pid 式目录在现场**跨目标复现**（dialog_keys 收编测试真实失败），遂从
  单点修复扩为全类收口：新增 `src/model/testenv.rs` 单源 helper（pid+序号+毫秒，
  sidecar 序号法同款），src 内全部 40 处固定 pid/时间戳式测试临时目录一次转换。
  新增回归测试 4 条，全部失败先行（TDD）。
- **质量现状**：全目标 1644P/0F（11 目标，分目标见验证证据）；clippy 0；fmt 0；
  arch 11/11。**本轮产品源码有一处行为修复**（dialog.rs `dropdown_rect`），其余全部
  为 cfg(test) 测试代码与测试基建。
- **两个移交缺陷均已闭环**：v115 待办①②由操作者裁决「直接修复」，本轮完成并附
  回归测试；其余遗留项（CRAP 三项未变异裁定、QA 三套规程脚本化等）口径不变。

## 术语速查（本文件用到的黑话）

| 词 | 意思 |
|---|---|
| P / F | 测试通过 / 失败数。1644P/0F = 1644 通过 0 失败 |
| 失败先行（TDD） | 先写测试并确认在旧实现上失败（锁住缺陷指纹），再实现使其通过 |
| #[path] 收编 | 集成测试以 `#[path = "..."]` 把产品源文件挂载进独立测试 crate（mutants 口径：产品树零改动） |
| 挂载壳 | 测试侧为收编源提供 `crate::xxx` 路径解析点的同名模块（如 hardening/model/mod.rs） |
| testenv | 本轮新增的 cfg(test) 单源工具模块：进程内唯一临时目录（pid+序号+毫秒） |
| clippy / fmt | Rust 官方静态检查 / 代码格式检查；0 = 无任何问题 |
| arch_check | 本项目架构自检脚本，11 条规则 |

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（`ezr download`）：高性能多协议 TUI 下载器。phase-01（HTTP/HTTPS 真实
  下载内核与 TUI 正式版）已交付并通过 QA 终局独立验证；其后为多轮增量（coder 裁决落地、
  specifier/coder 第 7–9 批操作者指令 v1.10–v1.12、cleaner v113 度量收敛、architect v114
  架构评审、hardender v115 变异加固、本轮 coder v116 缺陷修复）。
- 需求权威来源：`project/mission.md`（v1.12）；分期详述 `project/mission/phase-01.md`；
  功能规格 `project/features/`（12 份）；QA 规程 `project/qa/`（12 份规程 / 9 份套件脚本
  + 共用 harness）。

### 产物总账
| 阶段 | 做了什么 | 验证状态 |
|---|---|---|
| 基线交付（specifier + coder） | 需求文档 + 下载内核 + TUI + 测试夹具 | 已交付；QA 终局 966P/0F |
| 历轮加固与 QA | 历次加固约 110 条测试、度量收敛、架构复核、PTY 端到端 | 已交付 |
| coder 裁决落地轮（至 v1.9） | 上游裁决落地（on_evt 单测、锁回退收口、FR-01-85..93） | 已交付 |
| 操作者第 7–9 批指令（v1.10–v1.12） | 配置模板最小化、详情面板删 2 行、大小行去后缀 | 已交付 |
| cleaner v113（度量收敛） | 行覆盖 93.42%、CRAP>6 收敛 30 点、CPD 57→48、键盘模块拆分 | 1350P/0F |
| architect v114（架构评审） | 四阶段评审全过；WorkerDeps 收拢、下拉渲染单源；arch 规则 11；属性测试 +5 | 1355P/0F；arch 11/11；CPD 45 |
| hardender v115（变异加固） | 4 高点文件 288 点全量变异 + 3 杀灭测试 + 6 等价论证；产品源码零改动 | 1594P/0F（其口径未计 ezr-fixture 30 / ezr_proxy 11 两 bin 单测目标）；非等价存活 0 |
| **coder v116（本轮）** | 两移交缺陷直接修复：下拉浮层 Rect 钳制（产品源码 1 处）+ 测试临时目录全类唯一化收口（testenv 单源，src 40 处转换）；回归测试 +4（全部失败先行） | **1644P/0F；clippy 0；fmt 0；arch 11/11** |

### 遗留受限项（当前有效）
1. **变异覆盖为时间盒口径**：全量变异仅覆盖 4 个高点文件（288 点）+ 校准 + 定向重跑；
   其余文件约 1500 点未跑。**本轮新增注意**：dialog.rs `dropdown_rect` 新增钳制逻辑
   （约 8 个变异位点）与 testenv/helper 尚未入变异集，交 QA 后的下一轮 hardender。
2. **CRAP 三项未获变异裁定**：on_mouse（mouse.rs:25）、dlg_confirm_add（dialogs.rs:175）、
   spread_bytes（lease.rs:60）维持 v113 登记口径（时间盒外）。
3. **e2e 预登记跳过 3 例**：RB-09；TP-02/03/06（既有口径不变）。
4. **沙箱无 TTY**：TUI 进程级端到端无法自动化；PTY 端到端由 QA 会话以 pyte 完成。
5. **QA 三套规程有文档、脚本未落**：QA-CT-01..05、QA-NP-01..18、QA-MT-01..08；
   其中 QA-NP 需 ezr-proxy 先扩展三种监听形态（https / socks5+账号密码 / IPv6 回环）。
6. **Gherkin 变异清单仅为 01-persistence-config**：其余 11 份 feature 无步骤处理器，
   按 mutation-hardening §5 不入清单。
7. **tests/ 内部重复不入 CPD 账**：历史口径为 src-only（45 块）；本轮 src 净变化为
   dialog.rs 钳制 2 行 + testenv.rs 新模块（单源，无新增重复），CPD 口径预期不变
   （未在本轮复跑 PMD，交 QA/后续度量轮复核）。
8. **环境**：沙箱 2 核 / 4G / 磁盘 10G；工具链 rustup nightly 1.101.0-nightly、
   cargo-mutants 27.1.0（`~/.cargo/bin`）、PMD 7.28.0（`.tools/`）、bb 1.13.225 + APS
   仓库（`.tools/`）。`export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"` 后验证
   命令可复现。

### 实现定义值登记
- 无新增产品契约值。产品行为变化仅一处且为缺陷修复：**下拉浮层在终端不足以容纳时
  截断显示（宽或高方向），不再越界 panic**——26 列宽终端浮层右缘钳在末列（末列
  原为对话框右边框处，浮层覆盖之属预期遮挡）；浮层条目多于终端可容纳行数时只显示
  前若干项（无滚动，超出部分不可见）。此为窄端/多代理场景的可接受视觉口径，
  QA 复验时按「不 panic + 不越界」为契约、完整可见性不设契约。

## 二、当前产出情况（coder-20261007-v116）

### 本会话产物清单
**缺陷①修复（产品源码，1 处）**
- `project/ezr/src/ui/dialog.rs`：`dropdown_rect` 浮层 Rect 宽/高钳制在 area 内
  （`width: pw.min(area 右缘 - px)`、`height: ph.min(area 底缘 - y)`）；同函数
  `draw_task_dialog` 内算法/代理两处下拉选项行循环加 `.take(pinner.height)`
  （钳后内框截断）。触发点：26 列终端 + 代理下拉（pw=26，旧几何右缘 27>26 →
  ratatui 0.29 Clear 渲染 `index outside of buffer` panic）；纵向分支：浮层条目数
  +2 超过终端高（如 12 个命名代理 + 12 行终端）同样越界，一并钳制。

**缺陷②修复（测试隔离，全类收口）**
- `project/ezr/src/model/testenv.rs`（**新增**）：cfg(test) 单源
  `uniq_tmp_dir(prefix)` → `<前缀>-<pid>-<进程内原子序号>-<毫秒>`，建目录失败即
  panic。收口于 model 层（最内层，被全部测试 crate 收编，app/ui/engine 依赖合规，
  arch 规则 1/2/3/8/11 逐条核对通过）。
- 转换 40 处固定 pid/时间戳式临时目录（`std::env::temp_dir().join(format!(...pid...))`
  → `uniq_tmp_dir(...)`）：app/dialog_keys（2）、app/dialogs（3）、app/cli（1）、
  app/mouse（1）、app/paste（1）、app/engine tick/evt（7）、app/testutil（1，先建后
  并入 testenv）、ui/mod（2）、main.rs（2）、model/config（4）、model/checksum（4）、
  model/registry（6）、engine/supervisor（11）、engine/mod（1）、sentinel（2）、
  bin/ezr-fixture（6，独立 crate 经 `#[path]` 挂载同一源文件）。
- `project/ezr/src/app/testutil.rs`：uniq_tmp_dir 并入 testenv 后移除（保留 dialog 夹具）。
- `project/ezr/tests/hardening/model/mod.rs`：挂载壳补挂 testenv（收编 config.rs 的
  cfg(test) 代码引用 `crate::model::testenv` 所需）。

**回归测试（+4，全部失败先行确认后转绿）**
- `project/ezr/src/ui/dialog.rs` `dropdown_rect_clamps_within_area`：钳制几何单元锁定
  （26 列横向钳宽 25 / 纵向钳高 12 / 常态不钳）。
- `project/ezr/tests/hardening_v115.rs` `proxy_dropdown_no_panic_at_26_cols`：26 列渲染
  不 panic + 左上角 (1,22) ╭ + 右缘 (25,22) ╮ + 对话框左边框未被覆盖；并更新该文件
  头部 v115 备注（26 列由「另行登记」改为「已修复，见下方回归」）。
- `project/ezr/tests/hardening_v115.rs` `proxy_dropdown_no_panic_when_taller_than_terminal`：
  12 代理 + 80×12 终端渲染不 panic + 浮层顶 (12,0) ╭ + 底 (12,11) ╰。
- `project/ezr/src/app/engine.rs` `make_app_same_tag_never_restores_previous_registry`：
  同 tag 两次 make_app 后者不得恢复前者的 registry（旧实现下此测试真实失败——
  污染机制的进程内精确复现）。

**工具侧经验沉淀（packs/_common/，宪法第三章）**
- engineering.md：「pid 命名的测试临时目录」既有条目合并新内核——**根治靠收口**：
  唯一化命名收敛为单一 cfg(test) 工具函数、全仓禁手拼 pid 公式；审计优先级按
  「路径是否被夹具构造函数读取以恢复状态」排。
- notes/rust.md +2：`#[path]` 收编源的 cfg(test) 代码按测试 crate 解析 crate 路径
  （挂载壳须提供解析点，cargo fmt 解析失败是首个信号）；嵌套 mod 文件内 `#[path]`
  相对该文件所在目录解析（层级按 mod 文件位置数）。

### 验证证据（四要素①：本地验证命令清单，按序；crate 根 = project/ezr）
| # | 验证项 | 结果 | 复现命令 |
|---|---|---|---|
| 1 | 缺陷①指纹复现（修复前） | 两测试 panic：`index outside of buffer ... index is (26, 22)` / `(12, 12)` | 修复前 `cargo test --test hardening_v115 -- proxy_dropdown_no_panic`（已修复，指纹留档本表） |
| 2 | 缺陷②指纹复现（修复前） | 隔离测试失败：同 tag 二次构造恢复出前次任务（assert tasks.is_empty() 失败） | 修复前 `cargo test --bin ezr make_app_same_tag`（已修复） |
| 3 | 全目标测试 | **1644P/0F**（11 目标：bin 317 + ezr-fixture 30 + ezr_proxy 11 + hardening 107 + g_appcore 246 + g_engine 245 + g_main 2 + g_proxy 12 + g_supervisor 223 + g_throttle_err 167 + v115 284） | `cargo test` |
| 4 | 下拉回归组 | 5P/0F（既有 3 + 新增 2） | `cargo test --test hardening_v115 -- proxy_dropdown` |
| 5 | 静态检查 / 格式 | clippy 0 警告 / fmt 0 差异 | `cargo clippy --all-targets` / `cargo fmt --check` |
| 6 | 架构自检 | 11/11 | `bash scripts/arch_check.sh` |
| 7 | 缺陷②现场修复确认 | confirm_add_validates_then_creates 转绿（v115 全量轮曾真实失败于此） | `cargo test --test hardening_v115 -- confirm_add_validates` |

### 对账口径
- 与 v115 交接的 1594 口径差：1644 − 1594 = +50 = +41（ezr-fixture 30 + ezr_proxy 11
  两个 bin 单测目标本轮计入总账，上游口径未单列）+ +9（本轮新增 4 条测试在含收编源
  目标中的计次：bin +2、g_appcore +1、g_engine +1、g_supervisor +1、v115 +4）。
  每目标数字均可在 `cargo test` 输出逐行复算。
- 本轮改动面：产品源码 1 文件（dialog.rs，行为修复 1 处）；产品内 cfg(test) 代码
  15 文件；测试基建 2 文件（testutil.rs、hardening 挂载壳）；新增 1 模块 + 4 测试；
  测试目录（tests/）仅 hardening_v115.rs（新增 2 测试 + 备注更新）。

### 四要素②：会话运行时产物位置与复现方法
- 本轮为缺陷修复轮，无变异/覆盖率/CPD 运行时产物；`.work/tmp/` 仅会话临时文件。
- 上游 v115 的复现资产（变异、Gherkin、pristine）随会话终结已不保留，复现方法见
  归档包 v115 交接（要素①第 4/5 条命令仍有效）。**注意**：v115 的 pristine 基线
  已不含本轮 dialog.rs 修复，下一轮变异/对账以本轮后的工作区为新基线。

### 四要素③/④
- 无 feature 注释块变化；无等价突变体新增（本轮未跑变异）。

### 待办与待批（未决项）
1. **【已闭环】v115 待办①（26 列下拉越界 panic）**：操作者裁决直接修复，本轮完成，
   回归测试 3 条锁定（单元几何 + 26 列集成 + 高于终端集成）。
2. **【已闭环】v115 待办②（make_app 临时目录跨目标污染）**：操作者裁决直接修复，
   本轮完成并扩为全类收口（40 处转换 + 隔离回归测试 + engineering.md 根治条款）。
3. **【移交 QA / 后续轮】** CRAP 三项未变异裁定（on_mouse / dlg_confirm_add /
   spread_bytes）与其余 ~1500 变异点 + 本轮新增钳制逻辑与 testenv 的变异补跑：
   按时间盒维持登记，是否扩大变异范围由操作者定。
4. **【移交 QA（累积，继承自上游）】** QA-CT-01..05、QA-NP-01..18、QA-MT-01..08
   脚本化；ezr-proxy 三类监听扩展（https / socks5+账号密码 / IPv6 回环）。
5. **【移交 QA 复验清单（本轮新增）】** ①窄端下拉：26 列添加对话框展开代理下拉不
   panic、末列钳制符合「实现定义值登记」口径；②浮层高于终端只显示前若干项（无
   滚动）的视觉可接受性；③测试套件在连续多轮 `cargo test`（pid 复用场景）下全绿。

### 移交建议
- 建议操作者运行 `bin/swarm complete` 归档后，链式启动 `six-pack/QA`：
  1. 独立复验两缺陷修复（上复验清单①②③ + 要素① #4/#7）；
  2. 既定 QA 规程执行（累积移交第 4 条），PTY 端到端按 pyte 口径；
  3. 变异补跑（移交第 3 条）建议下一 hardender 轮以本轮后工作区为新基线。

By coder.
