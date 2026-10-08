# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect　会话：architect-20261008-v116
> 版本注：架构 v1.16——四阶段架构评审（结论：依赖方向/信息隐藏/封装零违规；
> 真缺口 = 渲染路径内联聚合）+ 保持行为边界改造：分发核拆分（on_mouse 两分支、
> tick 六段管线）、渲染区块拆分（header → header/chart/footer）、app/engine.rs
> 目录化（mod/tick/evt）、跨模块 DRY 单源五处（URL 校验/路径后缀/算法回查/
> 字段容量/排队位次）、死 API 清理两处；属性测试 35 → 47；arch 规则 11 → 12。
> 全程零行为改动：既有 1752 测试零改动全绿；全量 1770P/0F、clippy 0、fmt 0、arch 过。

## 速览（先读这一页）

- **项目**：EZR Downloader——终端里的多协议下载工具（Rust / TUI）。需求权威来源
  `project/mission.md`（v1.14）；分期详述 `project/mission/phase-01.md`（v1.14）。
- **本轮（architect）做了什么**：①四阶段评审（三路并行深读 + 主评核验）——分层
  依赖与窄接口零违规，Connection 消费侧适配等既有裁决完好；②改造落地——
  渲染聚合提取为 App 访问器（全局 ↓↑ 合计/并发 N/活跃 x/排队位次，可无头单测）、
  tick 九步单函数粗粒度拆六段、on_mouse 拆对话框/主界面两分支、下拉浮层对收口
  draw_dropdown、header 按区块拆 chart+footer（chart 数值核成纯函数）、
  app/engine.rs 目录化三文件（>100 变异点裁决兑现）、五处跨模块 DRY 单源、
  两处死 re-export 删除；③属性测试 35→47（Blocks 状态机/save_json_atomic 幂等/
  namegen 解析族/伴随文件解析/Retry-After 饱和口径/classify 全特征化/排队位次
  oracle 等 12 条）；④arch_check 规则 12（ui 层 IO 导入禁令，阳性对照实测）。
- **质量终态**：全量 **1770P/0F**（= 1752 基线 + 15 bin 内新增 + 3 挂载倍增，
  逐数对账见下）；其中属性测试 47 单列、单测基线 1752 零改动全绿；clippy 0；
  fmt 0；arch 12 规则全过。
- **移交**：建议 `bin/swarm begin six-pack/hardender`（变异补跑 + CRAP/变异点
  复测），随后 QA 走完 six-pack 链路。

## 术语速查

| 词 | 意思 |
|---|---|
| P / F / S | 用例通过 / 失败 / 受限跳过 |
| CRAP | Change Risk Anti-Patterns：comp²(1−cov)³+comp；本项目 cov 全 100% → CRAP=comp |
| 变异点 | cargo-mutants `--list` 的可变异位置（scan/count 模式，不跑变异） |
| 挂载树合法副本 | `#[path]` 测试 crate 重复注册 cfg(test) 测试（notes/rust.md 条款；本轮 chart 3 单测 ×2 上下文即此） |
| 对账隔离 | 属性测试独立计数不与单测基线对账（SKILL/notes/rust.md 纪律） |
| 访问器提取 | 渲染函数内联聚合改为应用层方法的保持行为变换（本轮产出纪律） |
| 落账 | 把操作者裁决结果以注记形式写入 mission 条款，关闭对应待批项 |

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（`ezr download`）：高性能多协议 TUI 下载器。phase-01 已交付；v1.13
  UI 对齐 + v1.14 详情面板批次（specifier + coder 两棒）已交付；cleaner 清理
  （第三棒）+ architect 架构轮（第四棒，本轮）已交付。
- 需求权威来源：`project/mission.md`（v1.14）；分期详述 `mission/phase-01.md`（v1.14，
  FR-01-94…101 + D26/D27）；功能规格 `project/features/`（13 份）；QA 规程 `project/qa/`
  （13 份规程 / 12 份套件脚本 + 共用 harness）。
- **外部参照**：`ezr-demo/docs/UI交互需求文档-定稿.md` v1.4 与 `ezr-demo/src/`
  （v1.14 权威 = ezr-demo 源码提交 5492c7c 修订-3/5/6/7）。

### 产物总账
| 阶段 | 做了什么 | 验证状态 |
|---|---|---|
| 基线交付 + 历轮加固 + 操作者第 7–9 批指令（至 v1.12） | 需求文档 + 下载内核 + TUI + 加固 + 度量收敛 | 已交付 |
| coder v116 / QA 终局（six-pack 收官轮） | 两缺陷修复；12 套件端到端 + 3 处缺陷 TDD 修复 | 1656P/0F；e2e 132P/0F/9S |
| specifier v1.13 + coder v1.13 | phase-01 v1.13 增补 + FR-01-94…99 全量实现 | 1729P/0F；clippy 0 |
| specifier v1.14 + coder v1.14 | FR-01-100/101 全量实现（TDD）+ 7 条新测试 | 1752P/0F；clippy/fmt 0 |
| cleaner v1.15 | 覆盖率 100%/0 缺口实测 + CRAP 67→64（5 拆分）+ CPD 41 块去重 + 变异点全仓扫描 | 1752P/0F；clippy/fmt 0；arch 过 |
| **architect v1.16（本轮）** | 四阶段评审 + 分发核/渲染核边界拆分 + 跨模块 DRY 单源五处 + 死 API 清理 + 属性测试 35→47 + arch 规则 12 | **1770P/0F；clippy/fmt 0；arch 12 规则过；单测基线 1752 零改动** |

### 遗留受限项（当前有效）
1. **ezr-proxy 三类监听扩展未做**（https / socks5+账号密码 / IPv6 回环）：QA-NP 部分
   S 相位沿旧口径登记（phase-02/后续承接）。
2. **e2e 预登记跳过 4 例**：RB-09（64MB 受限目录）、TP-02/03/06（测量方法学）。
3. **变异补跑待 hardender 轮**：v1.13 + v1.14 + v1.15 拆分 + v1.16 触碰文件均未入
   变异集。v1.16 触碰清单：mouse.rs、app/engine/{mod,tick,evt}.rs（原 app/engine.rs）、
   app/mod.rs、dialogs.rs、cli.rs、paste.rs、dialog_keys/{mod,routing}.rs、tasks.rs
   （间接）、main.rs、model/{mod,task,namegen,checksum,slots}.rs、engine/{mod,supervisor}.rs、
   ui/{mod,header,chart,footer,conns,list,dialog}.rs。**>100 变异点裁决结果**：
   app/engine.rs 已目录化（三文件估算各 <100，hardender `--list` 复测）；
   ui/header.rs 拆出 chart/footer 后余量下降（估算 ~121 点，实测为准）——若仍 >100，
   hardender 可对 chart.rs/footer.rs/header.rs 按 `--file` 分块跑，无需再拆。
4. **沙箱无 TTY**：TUI 进程级端到端无法自动化；PTY 端到端由 QA 会话以 pyte 完成
   （QA-UC 套件脚本化属 QA 轮职责）。
5. **环境（2026-10-08 本轮重建）**：全新沙箱重装 rustup nightly 1.101.0-nightly
   （1d81eb4ad，`~/.cargo`，minimal + rustfmt/clippy/llvm-tools-preview），
   `export PATH="$HOME/.cargo/bin:$PATH"` 后验证命令可复现。**度量工具本轮未装**
   （cargo-llvm-cov / cargo-mutants / PMD）——architect 验证序列以测试+静态检查
   为准未跑覆盖率/CPD/变异；hardender 轮按 cleaner v1.15 登记清单重建
   （cargo-llvm-cov 0.9.1、cargo-mutants 27.1.0、PMD 7.28.0 `.tools/pmd-bin-7.28.0/`）。
6. **Ctrl+B 的 BT 断开实际执行**：phase-02 承接（D26；01 期守卫链 + HTTP 拒绝臂已实现，
   BT 臂为不可达注释锚点）。
7. **沙箱无图形会话**：arboard 系统剪贴板路径不可运行时验证（OSC 52 回退臂生效）。
8. **thread_count 产品面暂无调用**：#[allow(dead_code)] 注记保留（phase-02 BT 预期复用）。
9. **CRAP>6 残余 64 处（cleaner v1.15 登记口径）的拆分裁决已部分兑现**：分发核
   （on_mouse 29 → 两分支；tick 21 → 六段）与绘制核（header 聚合提取、下拉对收口）
   已做安全级拆分；不安全项登记移交 hardender（见下「待办与待批」1）。拆分后
   CRAP 清单需以同口径（llvm-cov lcov+branch + BRDA 近似）复测刷新数字。
10. **bin 入口前导抽取裁决：不做（v1.16 改判）**——cleaner v1.15 登记的「三处启动
    前导重复」经实测修正：哨兵与单实例锁**仅产品 bin（main.rs）所有**，ezr-proxy/
    ezr-fixture 无此机制；真实重复仅为 lint 姿态块（3 份字面量）与产品↔QA-fixture
    的解析骨架/accept 循环形似（刻意对称）。lib 化抽取需 30 文件可见性 churn 换
    ~40 行去重且共享件仍单消费者，收益不成立——登记为接受项。
11. **join 语义双口径（既有，未改）**：Task 路径方法与引擎经 `namegen::join_path`
    （归一尾 `/` 与 `\`），对话框/CLI 的 sidecar 探测点与删除清理沿 `format!("{dir}/{name}")`
    （仅归一 `/`）——目录尾 `\` 输入下两口径产物字节不同（Linux 合法怪异路径）。
    统一属行为变化，不在架构轮权限内；后缀常量已单源（漂移主风险已消除），join
    语义留产品线裁决。

### 实现定义值登记（累计，当前有效全量）
1. **单实例锁回退扩展**（main.rs）：state 目录不可写时回退系统临时目录。
2. **D19 完成校验用最新值**（app/engine/evt.rs）：完成判定以任务当前 checksum 为准。
3. **恢复路径提示顺序**（app/tasks.rs）：恢复提示先落、缺失引用提醒后发覆盖。
4. **D26 并发明细 toast 文案**：照录 demo 定稿原文。**v1.14 裁决维持**。
5. **连接级累计/速度口径（FR-01-99）**：App 消费侧计量、EMA α=1/5 平滑、清零触发
   与保留规则（v1.13 登记未触碰）。
6. **并发明细选择序号口径**：conns_sel 为连接 id 升序展示序。
7. **流量图采样**：speed_hist 每秒 ≤1 点、双向 EMA、窗口缩放（本轮数值核提取为
   纯函数 `chart::chart_columns`，数学口径逐行等价）。
8. **D27 URL 热区复制内容**：复制详情展示值（final_url 优先）；「校验」热区复制完整校验码值。
9. **复制反馈账本**：App::last_copied 记录最近复制内容。
10. **剪贴板实例生命周期**：arboard 懒初始化复用，失败回退 OSC 52。
11. **热区几何**：字段名整段 9 列热区（行号超出内框高度不设）。
12. **fmt_block_size 钦定锚点（维持保留）**：`bytes == MB → "1 MB"`、
    `bytes == BT_CHUNK_SIZE → "256 KB"` 两臂为定稿口径显式锚点，不做等价消除。
13. **parse_retry_after_header 饱和口径（v1.16 属性锚定，非新裁决）**：`"inf"` →
    `Some(f64::INFINITY)`（f64 解析 + 非负过滤的既有行为），NaN/负数/垃圾 → None；
    属性测试锁定防漂移，hardender 注意 retry_in 对 inf 的既有计时语义。

## 二、当前产出情况（architect-20261008-v116）

### 本会话产物清单

**渲染边界与测试性（Phase 1 缺口修复，零行为变化）**
- `src/ui/chart.rs`（新增）：流量图自 header.rs 按区块拆出；数值核提取纯函数
  `chart_columns`（双向 EMA + min–max 缩放 + 双子采样，签名 `(hist,w,h)→Vec<usize>`），
  附值域/恒定输入/渲染冒烟 3 单测。
- `src/ui/footer.rs`（新增）：页脚快捷键行/toast 行自 header.rs 拆出。
- `src/ui/header.rs`：聚合改经 App 访问器（`global_dl_speed`/`global_ul_speed`/
  `conns_display_total`），绘制区余量下降（>100 变异点复测归 hardender）。
- `src/app/mod.rs`：新增 4 访问器（上述 3 + `active_conn_count(&Task)`）；死
  re-export `pub use model::Task` 降为私有导入（crate::app::Task 零消费者实测）。
- `src/ui/conns.rs`：`活跃 x` 计数改经 `App::active_conn_count`（渲染门槛语义随行）。
- `src/ui/list.rs`：排队位次改经 `model::slots::queue_positions`（渲染层重实现消除；
  与 `queue_pos` 数值等价有属性锁定）。

**分发核与适配器结构（Phase 4/裁决项，零行为变化）**
- `src/app/mouse.rs`：`on_mouse` 拆 `on_mouse_dialog`/`on_mouse_main`（对话框早退
  语义不变：对话框态一切鼠标事件仅作用对话框）。
- `src/app/engine.rs` → `src/app/engine/{mod,tick,evt}.rs`（>100 变异点目录化裁决）：
  mod.rs = 消费侧适配 + spec 管道 + save_registry/shutdown；tick.rs = 六段管线
  （事件消费/槽位授予与启动/重试倒计时/速度与会话/housekeeping/钳制）+ send_retry_start；
  evt.rs = on_evt（pub(super) 限 app::engine 子树）+ handle_failure。测试模块
  （tick_tests/evt_tests/conn_stats_tests）原样留在 mod.rs。
- `src/ui/dialog.rs`：`draw_dropdown` 收口校验算法/代理下拉对（框架定位 + 逐项渲染 +
  钳短裁剪单一函数，选项对由调用方传入）。
- 过期段注释两处删除（engine.rs「键盘交互」、dialogs.rs「鼠标交互」标题残留）。

**跨模块 DRY 单源五处（Phase 4 清单，逐字节等价替换）**
- `src/model/mod.rs`：`is_http_url` / `protocol_of_url` 新增——URL 校验三处
  （main.rs pos_url / dialogs.rs / cli.rs）与协议推导两处单源。
- `src/model/namegen.rs`：`DOWNLOADING_EXT` / `SIDECAR_EXT` 常量 + `sidecar_path_of`
  ——后缀手拼六处收口（task.rs 方法、supervisor、exists_on_disk、dialogs 探测与
  删除、cli 探测）；join 语义按消费点保留（登记受限项 11）。
- `src/model/checksum.rs`：`algo_index_exact_or_default`——规范名精确回查 + 默认
  SHA-256 两处内联（supervisor verify / 修改对话框预填）单源；与宽容匹配
  `algo_index_by_name` 刻意双语义，属性锚定差异（"sha1" 落默认不命中）。
- `src/app/dialog_keys/mod.rs`：`CAP_TEXT`/`CAP_CONNS_DIGITS`/`CAP_CK_HEX`——字段
  容量双入口（routing 逐键 / paste 粘贴）单源。
- `src/model/slots.rs`：`queue_positions(tasks, view)`——列表排队编号单一来源。
- 死 API：`model::ProxyConfig` re-export 移除（零消费者；ProxyChoice/ProxyEndpoint
  有消费者保留）。

**属性测试（35 → 47，独立计数）**
- `src/property_tests.rs` 新增 12 条：Blocks op 序列不变量（资源域有界生成）、
  save_json_atomic 幂等（fs 字节级 + tmp 无残留 + 漂移落盘）、from_url_path
  全函数性 + 残缺尾、from_content_disposition 形状、parse_companion_content 稳定性、
  parse_retry_after_header 稳定性（含 inf 饱和口径锚定）、classify_http_status
  全特征化、queue_positions 前缀计数 oracle + 恒等视图 queue_pos 等价、
  algo_index_exact_or_default 特征化、URL scheme 助手特征化、路径约定等价
  （Task 方法 ≡ 散点助手）、任务投影界（progress/eta/chunk_info）。
- `src/engine/mod.rs`：cfg(test) 门面增 `parse_retry_after_header` re-export
  （属性测试可达；窄接口不受影响）。

**自动化架构检查**
- `scripts/arch_check.sh`：规则 12——ui 层禁 `use tokio|reqwest|arboard|fs2`
  （展示层零 IO 依赖固化；crossterm/ratatui 为合法呈现依赖不在扫描域，
  `#[tokio::test]` 属性形态不命中）。阳性对照实测记录于脚本尾注。

### 验证证据（crate 根 = project/ezr）
| # | 验证项 | 结果 | 复现命令 |
|---|---|---|---|
| 1 | 全量测试（11 目标） | **1770P/0F** | `cargo test --workspace` |
| 2 | 逐目标对账 | bin 359（+12 属性 +3 chart）/ ezr-fixture 30 / ezr_proxy 11 / hardening 107 / g_appcore 265 / g_engine 263 / g_main 2 / g_proxy 12 / g_supervisor 241 / g_throttle_err 167 / v115 313（+3 chart 挂载） | `cargo test` 输出逐行 |
| 3 | 属性测试单列（对账隔离） | **47P/0F**（35 既有 + 12 新增） | `cargo test property_tests::` |
| 4 | 静态检查 / 格式 | clippy 0（--all-targets）/ fmt --check 0 | `cargo clippy --all-targets` / `cargo fmt --check` |
| 5 | 架构检查 | 12 规则全过（含新规则 12，阳性对照生效） | `bash scripts/arch_check.sh` |

### 对账口径
- **1770 对账**：单测基线 = 1770 − 12（属性，独立计数不入对账）= 1758；1758 −
  1752（cleaner v1.15 基线）= +6 = chart.rs 3 单测 × 2 编译上下文（bin ezr +
  hardening_v115 挂载，notes/rust.md 挂载树倍增机制）。既有 1752 测试**零改动
  全绿**——本轮纯结构等价变换，无新增/删除/改名既有测试。
- **行为基线**：FR-01-30/31/32 状态机、FR-01-17 速度、FR-01-94…99 定稿布局、
  FR-01-100/101 详情面板全部未动（1752 既有测试零改动通过仅实现内部结构变化）。
- **访问器数值等价**：`global_dl_speed`（过滤 Downloading 求和）与 tick 原全任务
  求和恒等——tick 同帧先行归零非下载态速度（tick_speed_and_session 前置语句），
  数值恒等有局部证明；`queue_positions` 与 `queue_pos` 等价有属性锁定。

### 四要素③/④
- 无 feature 注释块变化（本轮未触碰规格与 QA 文档）。
- 无新增等价突变体登记（本轮未跑变异测试）；沿 cleaner 预登记：fmt_block_size
  钦定锚点两臂为可证等价分支（hardender 轮注意）；本轮新增等价高发位提示——
  `algo_index_exact_or_default` 的 `unwrap_or(3)` 与默认臂（属性已锚定语义）。

### 待办与待批（未决项）
1. **【移交 hardender 轮】**变异补跑基线更新（v1.13 + v1.14 + v1.15 拆分 + v1.16
   触碰文件，清单见受限项 3）；app/engine 三文件与 header/chart/footer 的
   `--list` 实测复数（>100 判定以实测为准）；CRAP 残余清单同口径复测刷新；
   fmt_block_size 钦定锚点等价突变体预登记兑现。
2. **【移交 QA 轮】**QA-UC-16…18 套件脚本化（pyte PTY 口径 + OSC 52 序列捕获断言，
   沿 cleaner 登记）；本轮零布局/零文案改动，既有套件旧断言无需改动。
3. **【登记，无行动】**join 语义双口径（受限项 11）、bin 前导不抽取（受限项 10）、
   parse_retry_after 饱和口径（实现定义值 13）——均为已裁决/已锚定状态，无待批。
4. 无操作者待批项（本轮全部为保持行为结构改造，无口径变更）。

### 移交建议
- 建议下一步：`bin/swarm begin six-pack/hardender`（变异加固：补跑基线 +
  逐文件分块变异），随后 QA 走完 six-pack 链路；操作者如认可当前架构终态，
  直接 `bin/swarm complete` 归档即可。
- 手工冒烟入口（沙箱外有 TTY 环境）：`cargo run --release`（project/ezr）——本轮
  零行为改动，冒烟预期与 cleaner v1.15 完全一致。
- 归档前无需手动清理（target/ 由 complete 自动清理；`.tools/` 按 .gitignore 不入包）。

By architect.
