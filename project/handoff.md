# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect　会话：architect-20261006-v114（架构评审
> 四阶段全过 + 行为保持改进两处（WorkerDeps 参数对象 / 下拉浮层框架单源）+ arch_check
> 规则 11 + 属性测试 30→35 + CPD 48→45 + 覆盖率 93.42→93.63%）

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（ezr download）：TUI 高性能多协议下载器，phase-01（HTTP/HTTPS 真实下载
  内核与 TUI 正式版）已交付并通过 QA 终局独立验证；QA 后增量轮：coder 裁决落地轮、
  specifier v1.4–v1.12 + coder v14–v1.12、cleaner v113（度量收敛）、本轮 architect v114
  （架构评审与属性测试轮，上游 = cleaner-20261006-v113 交接）。
  需求权威来源 `project/mission.md`（v1.12），phase-01 详述见 `project/mission/phase-01.md`（v1.12），
  feature 规格在 `project/features/`（12 份），QA 规程在 `project/qa/`（12 份规程 / 9 份脚本）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（基线）+ coder（phase-01） | mission/features/qa 9 份 + 下载内核 + TUI + fixtures | 已交付 | 上游既定口径 |
| cleaner / architect / hardender / QA（历轮） | 加固 ~110 测试、度量收敛、架构复核、PTY e2e 终局验证 | 已交付 | QA 终局 966P/0F；行覆盖 90.5% |
| coder（20261005 裁决落地轮）~ coder v1.9 | 见上游各轮登记（on_evt 单测、锁回退收口、FR-01-85..93 全链） | 已交付 | 逐轮对账自洽（1276P/0F @ v1.8） |
| specifier（v1.10）/ coder（v1.10） | 操作者第七批指令：配置模板最小化（钦定文本逐字节权威 54 行） | 已交付 | 钦定文本 9/9 值一致 |
| specifier（v1.11）/ coder（v1.11） | 操作者第八批指令：详情面板移除「状态」「速度」行 + 死字段 Task.elapsed 清除 | 已交付 | 1312P/0F |
| specifier（v1.12）/ coder（v1.12） | 操作者第九批指令：详情「大小」行移除「（剩余 …）」后缀（TDD） | 已交付 | 1313P/0F；clippy 0；fmt 0 |
| cleaner（20261006 v113） | 全库度量收敛：覆盖率 93.42%、CRAP>6 收敛至 30、CPD 57→48、dialog_keys 拆分（49+69）、10 条补测、testutil 夹具单源化 | 已交付 | 1350P/0F；clippy 0；fmt 0；arch 10/10 |
| **architect（20261006 v114）** | **架构评审四阶段全过 + 行为保持改进两处 + arch_check 规则 11 + 属性测试 +5（详见第二节）**：1350→**1355P**/0F；CPD 48→**45**；行覆盖 93.42→**93.63%**；变异点 1785→1788（已更改文件全部 <100） | 已交付 | **1355P/0F（315/30/11/107/245/244/2/12/222/167）**；clippy 0；fmt 0；arch_check **11/11**（规则 11 阳性对照实测） |

### 遗留受限项（当前有效）
- **CRAP 为受限近似口径**（继承上游；操作者已裁决维持现状）：无标准 CRAP 工具；按
  「llvm-cov lcov BRDA 弧数/2 + 1 代入 comp，闭包并入外层取极值，测试符号排除」复算
  （脚本 `.tools/crap-approx-lcov.py`，本轮沙箱重建未复跑 CRAP——cleaner v113 的 30 点
  登记清单仍为当前口径，交 hardender 以变异为最终准绳）。
- **e2e 套件预登记 S 项（3 用例）**：RB-09；TP-02/03/06（既有口径不变）。
- **沙箱无 TTY**：TUI 进程级端到端不可自动化；pty E2E 由 QA 会话以 pyte 口径完成。
- **QA 规程就绪、套件脚本未落**：QA-CT-01..05、QA-NP-01..18、QA-MT-01..08 共三套
  （12 份规程 / 9 份脚本）；QA-NP 需 ezr-proxy 扩展 https / socks5+user/pass / IPv6
  回环三种监听形态（未就绪登记 S；NP-11/12/16/17 无连接尝试不依赖扩展）。
- **本轮沙箱为全新环境**：工具链与度量工具全部重建（rustup nightly
  1.101.0-nightly (ea137335b 2026-10-05) + clippy/rustfmt 组件 + llvm-tools-preview；
  cargo-llvm-cov 0.9.1、cargo-mutants 27.1.0（`~/.cargo/bin`）；PMD 7.28.0
  （`.tools/pmd-bin-7.28.0`））。`.tools/crap-approx-lcov.py` 未重建（CRAP 未复跑，
  见上）。`export PATH="$HOME/.cargo/bin:$PATH"` 注入后所有命令可复现。

### 实现定义值登记
- 无新增产品契约值（本轮为行为保持架构轮：两处改进均为等价变换，对外语义零变化；
  模板/警告/凭证运行期文案沿 v1.9 口径不变）。

## 二、当前产出情况（architect-20261006-v114）

### 架构评审结论（四阶段，结论 + 清单 + 改进动作）
1. **UI/核心分离：PASS（1 项登记 trade-off）**。实证：model 层零框架依赖（grep
   ratatui/crossterm/tokio/reqwest 零命中）、model 零 async；ui 层零 tokio/reqwest；
   ui 只经 app 消费引擎（arch_check 规则 3）；ui 纯文本工具（text.rs）与列表/详情/对话框
   渲染分离，核心行为可无 UI 测试（TestBackend 渲染断言锁像素）。**登记 trade-off**：
   app 层持 `ratatui::layout::Rect`（list_area / dlg_*_rects 命中测试状态）——Rect 为
   纯值类型（无 IO/后端），且 rects 是跨帧「ui 写 / mouse 读」的 App 交互状态，迁移到
   ui 层需引入跨帧共享通道，成本 > 收益；维持现状。
2. **依赖规则：PASS**。分层 `main → app → engine → model`、`ui → app + model`、
   `model → ∅`；无导入循环（编译器 + arch_check 1/2/3 锁定）；engine 窄接口（私有
   supervisor/error/throttle + 门面 re-export TaskSpec/VerifySpec）；低层不构造高层类型
   （规则 5：ConnView 纯数据视图，Connection 适配在 app 消费侧）；环境变量收口适配缝
   （规则 8）；sentinel 仅入口可见（规则 7）。
3. **信息隐藏与封装：PASS**。model pub 面 72 项均为领域概念；对外保留项
   （ProxyConfig 等）带 `#[allow(unused_imports)]` + 依据注释；cfg(test) 门控 re-export
   （throttle/error 测试门面、ui::text 属性测试门面、testutil 夹具）产品构建零 API 增量；
   Dialog 字段 pub 属 app↔ui 单向消费对（app 拥有、ui 渲染），不跨层泄漏。
4. **局部代码质量：2 项改进动作（均已闭环，见下）**，其余为登记口径（CPD 裁决）。

### cleaner 两处结构变更复核（上游移交第 1 项）
- **dialog_keys 拆分**：mod.rs（App 对话框处理器）+ routing.rs（纯键路由函数：下拉导航
  /文本字段编辑/十六进制封顶）职责边界清晰——routing 无 App 状态、无副作用，供 impl
  处理器与测试复用；依赖方向零变化。**确认符合架构意图**；本轮在 routing 基础上补
  属性测试 5 条（见下）。
- **testutil 夹具单源化**：cfg(test) 门控 `pub(crate) mod testutil`，Dialog 默认值口径
  （ck_type=3/SHA-256、下拉收起、代理直连）单一来源。**确认合格**；本轮属性测试直接
  复用 `testutil::dialog` 构造。

### CPD 残余五类裁决（上游移交第 3 项）
| 类别 | 内容 | 裁决 |
|---|---|---|
| a | 测试场景字面量（Evt::Probed / TestBackend setup / e2e 事件泵，约 17 项） | **维持**：字段与断言目的各异，提取即参数膨胀（abstraction 无净收益） |
| b | ui 渲染习语对（约 11 项） | **部分收敛**：算法/代理下拉浮层两大重复项（8+12 行）经 dropdown_frame/dropdown_item 单源消除（本轮改进②）；残余为字段行 "▾" 箭头等微习语（4–8 行），提取仅得间接层，**维持登记** |
| c | supervisor block_worker 12 参 spawn 表（2 调用点） | **立项并完成**（本轮改进①：WorkerDeps 参数对象） |
| d | 三二进制入口样板（main/ezr-proxy/ezr-fixture，约 6 项） | **维持登记，移交操作者裁决**：去重需 lib target 重构（模块树迁移 + 4 个 #[path] 挂载测试 crate 重验 + 1355 测试全量回归）；三入口相似属偶合（三个独立小程序各自解析参数/accept 循环），非本质共享抽象，收益/风险比不足。若操作者立项，建议独立会话执行并整树回归 |
| e | Task::new_queued 调用点参数表 | **维持**：10 参工厂调用点字面清晰，参数对象仅搬运膨胀 |

### CRAP 残余 30 点裁决（上游移交第 2 项）
- **维持登记口径移交 hardender**：分发核（on_evt/on_mouse/tick/draw_task_dialog）为
  同名规则路由表形态，已有契约单测锁定路由序，表驱动拆分会把路由矩阵分散且 comp
  受限近似口径对 `?` 早退低估——以 hardender 变异为最终准绳；100% 覆盖纯函数
  （spread_bytes/consistency::check/fmt_block_size 等 comp=7..9）已由属性测试锁定
  守恒性/往返/稳定性，CRAP 数字不再有测试杠杆可兑现。

### 本会话产物清单（行为保持架构轮，产品语义零变化）
**① supervisor WorkerDeps 参数对象（CPD 类别 c 改造）**
- `src/engine/supervisor.rs`：新增 `#[derive(Clone)] struct WorkerDeps`（url/file_path/
  blocks/lease_next/stop_rx/quota_rx/ep_rx/total_written/failure/conns 十字段依赖包）+
  `spawn(wid, shared)` 方法——初始 worker 池与 Reconfigure 扩容两个 12 参调用点单源；
  `block_worker` 签名 12 参 → 3 参（`(wid, shared, deps: WorkerDeps)`），函数体首解构
  （mut 绑定入模式），`#[allow(clippy::too_many_arguments)]` 删除。依赖清单唯一登记于
  结构体定义，构造点/消费点漂移由编译器锁定。
**② ui 下拉浮层框架单源（CPD 类别 b 部分收敛）**
- `src/ui/dialog.rs`：新增 `dropdown_frame()`（定位 + Clear + 宽字符边缘裁剪 + 圆角
  边框块，返回选项内框）与 `dropdown_item()`（选中高亮 + ▸ 指针 + 主体 + 可选后缀 +
  命中区域回填）两个渲染 helper；算法下拉与代理下拉两大渲染块（各 ~30 行）改经 helper，
  逐 widget/样式/矩形/顺序与原内联形态一致（TestBackend 像素锁定测试全绿佐证）。
**③ arch_check 规则 11（model 纯同步逻辑护栏）**
- `scripts/arch_check.sh`：新增规则 11「model 禁入 tokio/reqwest」——锁「model 为
  最内层纯同步逻辑、全部可单元测试」声明；扫描域仅 model/ 目录（外层合法定向使用，
  不扩面）。阳性对照实测：向 model/mod.rs 注入真实 `use tokio::sync::mpsc;` → 命中
  并非零退出；移除后全过（对照记录存脚本尾注）。
**④ 属性测试 +5（app 层对话框状态机不变量，proptest 覆盖面首次进入 app 层）**
- `src/property_tests.rs`：新增 dialog_keys/routing 纯函数属性——
  `prop_text_char_caps_and_consumption`（消费矩阵 + URL/目录 ≤300 + 并发 ≤2 位数字 +
  conns_edited 置位）、`prop_text_backspace_pops_at_most_one`（退格矩阵 + 至多弹一 +
  非命中字段零变化）、`prop_push_hex_capped_cap_and_charset`（≤128 + 仅十六进制）、
  `prop_dropdown_open_key_sync_and_bounds`（ck_sel 界内 + ck_type 同步 + Tab/BackTab
  跳焦）、`prop_proxy_dropdown_open_key_bounds_and_guard`（n=0 守卫 + 界内 + 按 kind
  跳焦）。键序列策略 `dialog_key_seq`（导航键 8 / 字符键 2 加权 Union）。
- 门面链：routing 五函数 `pub(super)` → `pub`（私有模块内不外泄）→ dialog_keys
  `pub(crate) use`（兼内部绑定，产品构建有内部消费者无 unused 警告）→ app/mod.rs
  `#[cfg(test)] pub(crate) use`（测试构建由 property_tests 消费）；产品 API 面零增量。
- proptest 属性测试总数 30 → **35**（独立模块/独立命令/独立计数纪律不变）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量单元/集成/属性测试 | **1355P/0F**（315/30/11/107/245/244/2/12/222/167）；对账：上游 1350 + 属性测试 5（均挂 main bin 单份，无倍增） | `cargo test`（crate 根） |
| 属性测试独立口径 | **35P/0F**（30 既有 + 5 新增） | `cargo test property_tests::` |
| clippy / fmt / arch | 0 警告 / 0 差异 / **11 条规则全过**（规则 11 新增，阳性对照实测） | `cargo clippy --all-targets`、`cargo fmt --check`、`bash scripts/arch_check.sh` |
| 行覆盖率 | **93.63%**（上游基线 93.42%；+0.21pp，routing 纯函数属性测试兑现）；函数执行率 94.63→94.77% | `cargo llvm-cov --summary-only` |
| DRY（CPD ≥50 tokens） | 48→**45** 项（spawn 表 1 项 + 下拉浮层 2 项消除；WorkerDeps 定义↔block_worker 解构的对仗为新单源形态，不再构成独立调用点重复） | `bash .tools/pmd-bin-7.28.0/bin/pmd cpd -l rust --dir src --minimum-tokens 50`（exit 4=发现重复） |
| 变异点扫描 | **1788** 点（+3：dropdown_item 等 helper 净增）；已更改文件全部 <100：supervisor 61→65、ui/dialog 62→61、routing 69 不变、dialog_keys/mod 49 降出前列；未跑变异测试（归 hardender） | `cargo mutants --list --line-col=true --exclude-re property_tests` |

### 待办与待批（未决项）
- **无操作者待批项**：本轮行为保持架构轮，产品语义零变化、无实现定义值新增。
- **操作者裁决项（CPD 类别 d）**：三二进制入口 lib target 重构是否立项——登记于上表
  裁决行，收益/风险比与执行建议已给出；不立项则该 6 项 CPD 残余为终态登记。
- **hardender 承接**：变异测试全量执行（按本轮 1788 点与每文件计数 `--file` 分块）；
  CRAP 残余 30 点以变异为准绳复核（cleaner v113 登记清单仍有效）。
- **QA 会话承接（累积）**：QA-CT-01..05、QA-NP-01..18、QA-MT-01..08 脚本化与
  ezr-proxy 三类监听扩展（https / socks5+认证 / IPv6 回环）。
- **遗留受限项**：沙箱无 TTY（pty E2E 归 QA 会话）；CRAP 受限近似口径（操作者已裁决
  维持；`.tools/crap-approx-lcov.py` 本轮未重建）。

## 三、移交建议
- 本轮为行为保持架构轮（six-pack 编排：specifier→coder→cleaner→**architect**→hardender→QA），
  建议操作者运行 `bin/swarm complete` 归档后链式启动 `six-pack/hardender`：
  1. **变异测试全量执行**：按本轮登记的每文件计数分块（`--file`），1788 点；app/engine.rs
     94 / lease.rs 88 / timefmt.rs 78 / fixture/respond.rs 69 / routing.rs 69 / config.rs 68
     为高点文件。
  2. **CRAP 残余 30 点复核**：cleaner v113 登记清单（draw_task_dialog 35.1 / download
     29.1 / on_mouse 25.5 / tick 25.2 / dlg_confirm_add 20.4 / on_evt 19.0 / block_worker
     15.3 / from_toml 12.0 / spread_bytes 9.0 等）以变异结果为最终准绳。
  3. **本轮两处改造的变异关注点**：WorkerDeps.spawn 与 block_worker 解构（字段遗漏/
     次序变化应被既有测试杀灭）；dropdown_frame/dropdown_item（渲染 helper 的选中臂/
     后缀臂/回填行为由 ui 像素测试与属性测试共同锁定）。
- 环境与工具安装位置见第一节「本轮沙箱为全新环境」条；度量命令均可按验证证据表复现。
