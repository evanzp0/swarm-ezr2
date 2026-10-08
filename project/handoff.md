# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner　会话：cleaner-20261008-crap-dry
> 版本注：清理 v1.15——coder v1.14 产出批次保持行为清理：CRAP>6 由 67 → 64
> （5 处表驱动/助手拆分：text::w 宽度表、clip_wide_at_edges 边缘助手、
> percent_decode 三元组、consistency::check 头对助手、find_companion probe/scan）；
> DRY：产品面 match 对重复收口 + ui 测试基建单源化（testfx）；覆盖率实测 100% 行
> 覆盖、0 真缺口；变异点扫描全仓 2074 点/50 文件（v1.14 触碰 6 文件全部 <100）。
> 全程零行为改动：1752P/0F 基线复验两轮（拆分后 + 去重后），clippy/fmt/arch 持续 0/0/过。

## 速览（先读这一页）

- **项目**：EZR Downloader——终端里的多协议下载工具（Rust / TUI）。需求权威来源
  `project/mission.md`（v1.14）；分期详述 `project/mission/phase-01.md`（v1.14）。
- **本轮（cleaner）做了什么**：覆盖率 + CRAP/DRY/变异点度量与保持行为收敛——
  llvm-cov（lcov+branch）函数级分析（自写解析器，logical-name 聚合/挂载树去重/
  闭包并入，阳性对照过）：产品函数 494 个、真覆盖缺口 0、行覆盖 100%；CRAP>6
  67→64（5 处拆分，全为表驱动/助手提取，逐处经全量测试复验）；PMD CPD 41 块
  重复（产品面修 1 处、测试面 testfx 单源化、bin 入口前导登记移交）；cargo-mutants
  scan 模式全仓 2074 变异点，v1.14 触碰 6 文件全部 <100 无强制拆分。
- **质量终态**：全目标 **1752P/0F**（11 目标，与 coder 基线逐数一致——零新增测试，
  纯行为保持）；clippy 0；fmt 0；arch 过；CRAP>6 残余 64 处全部 cov=100%（CRAP=comp
  固有复杂度，登记移交）。
- **移交**：建议 `bin/swarm begin six-pack/architect`（模块边界与残余 CRAP/变异点
  裁决），再 hardender → QA。

## 术语速查

| 词 | 意思 |
|---|---|
| P / F / S | 用例通过 / 失败 / 受限跳过 |
| CRAP | Change Risk Anti-Patterns：comp²(1−cov)³+comp；本项目 cov 全 100% → CRAP=comp |
| BRDA/DA | lcov 分支/行数据记录；BRDA 计数做 comp 近似（`&&` 链每项记 2，偏上界） |
| 变异点 | cargo-mutants `--list` 的可变异位置（scan/count 模式，不跑变异） |
| 挂载树合法副本 | `#[path]` 测试 crate 重复注册 cfg(test) 测试（notes/rust.md 条款） |
| 单态化噪音 | async fn/闭包多份编译实例 FNDA=0 但行覆盖 100%（聚合取极值后消失） |
| 落账 | 把操作者裁决结果以注记形式写入 mission 条款，关闭对应待批项 |

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（`ezr download`）：高性能多协议 TUI 下载器。phase-01 已交付；v1.13
  UI 对齐 + v1.14 详情面板批次（specifier + coder 两棒）已交付；本轮 cleaner 清理
  （six-pack 第三棒，实现后清理）。
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
| **cleaner v1.15（本轮）** | 覆盖率 100%/0 缺口实测 + CRAP 67→64（5 拆分）+ CPD 41 块去重 + 变异点全仓扫描 | **1752P/0F；clippy/fmt 0；arch 过；CRAP 残余 64 登记** |

### 遗留受限项（当前有效）
1. **ezr-proxy 三类监听扩展未做**（https / socks5+账号密码 / IPv6 回环）：QA-NP 部分
   S 相位沿旧口径登记（phase-02/后续承接）。
2. **e2e 预登记跳过 4 例**：RB-09（64MB 受限目录）、TP-02/03/06（测量方法学）。
3. **变异补跑待 hardender 轮**：v116 + QA 轮 3 处修复 + v1.13/v1.14 UI 层新代码 +
   本轮 5 处拆分后代码均未入变异集；另登记 header.rs（161 点）/ app/engine.rs
   （118 点）单文件 >100 变异点（architect 裁决模块边界后 hardender 兑现）。
4. **沙箱无 TTY**：TUI 进程级端到端无法自动化；PTY 端到端由 QA 会话以 pyte 完成
   （QA-UC 套件脚本化属 QA 轮职责）。
5. **环境（2026-10-08 本轮重建）**：rustup nightly 1.101.0-nightly（1d81eb4ad，
   `~/.cargo`，minimal + rustfmt/clippy/llvm-tools-preview）；cargo-llvm-cov 0.9.1；
   cargo-mutants 27.1.0（均 `~/.cargo/bin`）；PMD 7.28.0（`.tools/pmd-bin-7.28.0/`，
   bin zip 自 GitHub pmd_releases/7.28.0）。`export PATH="$HOME/.cargo/bin:$PATH"`
   后验证命令可复现。bb/Babashka 本轮未装（cleaner 不跑 APS 套件）。
6. **Ctrl+B 的 BT 断开实际执行**：phase-02 承接（D26；01 期守卫链 + HTTP 拒绝臂已实现，
   BT 臂为不可达注释锚点）。
7. **沙箱无图形会话**：arboard 系统剪贴板路径不可运行时验证（OSC 52 回退臂生效）。
8. **thread_count 产品面暂无调用**：#[allow(dead_code)] 注记保留（phase-02 BT 预期复用）。
9. **CRAP>6 残余 64 处（登记口径，非缺陷）**：全部 cov=100%（CRAP=comp 固有复杂度）
   ——分发核心（on_mouse 29 / draw_task_dialog 27 / tick 21 / add·modify_dialog_char 19 /
   dlg_confirm_add 15 / dropdown_item 15）、异步引擎核（download 19 / single_stream 13）、
   模型守卫（spread_bytes 13 / fmt_block_size 13〔钦定锚点保留〕 / consistency::check 13
   〔BRDA 对 `&&` 链虚高，源码 comp≈6〕）、绘制核（draw_header 13 等）。清单
   `.work/tmp/crap_residual_v114.txt`（会话级，数字已录本文件）；拆分裁决归 architect。
10. **bin 入口前导重复（CPD 登记）**：main.rs / bin/ezr-proxy.rs / bin/ezr-fixture/main.rs
    三处启动前导（哨兵/单实例锁/参数解析脚手架）重复——抽取需 crate 结构调整
    （bin-only crate 共享 lib），归 architect。

### 实现定义值登记（累计，当前有效全量）
1. **单实例锁回退扩展**（main.rs）：state 目录不可写时回退系统临时目录。
2. **D19 完成校验用最新值**（app/engine.rs）：完成判定以任务当前 checksum 为准。
3. **恢复路径提示顺序**（app/tasks.rs）：恢复提示先落、缺失引用提醒后发覆盖。
4. **D26 并发明细 toast 文案**：照录 demo 定稿原文。**v1.14 裁决维持**。
5. **连接级累计/速度口径（FR-01-99）**：App 消费侧计量、EMA α=1/5 平滑、清零触发
   与保留规则（详见 v1.13 登记，本轮未触碰）。
6. **并发明细选择序号口径**：conns_sel 为连接 id 升序展示序。
7. **流量图采样**：speed_hist 每秒 ≤1 点、双向 EMA、窗口缩放（本轮未触碰）。
8. **D27 URL 热区复制内容**：复制详情展示值（final_url 优先）；「校验」热区复制完整校验码值。
9. **复制反馈账本**：App::last_copied 记录最近复制内容。
10. **剪贴板实例生命周期**：arboard 懒初始化复用，失败回退 OSC 52。
11. **热区几何**：字段名整段 9 列热区（行号超出内框高度不设）。
12. **fmt_block_size 钦定锚点（本轮确认保留）**：`bytes == MB → "1 MB"`、
    `bytes == BT_CHUNK_SIZE → "256 KB"` 两臂与后续阶梯臂数学等价（常量可证），作为
    定稿口径的显式锚点保留，不做等价消除。

## 二、当前产出情况（cleaner-20261008-crap-dry）

### 本会话产物清单

**度量与分析（会话级，数字已录本文件）**
- `.work/tmp/coverage.lcov` / `coverage2.lcov`：llvm-cov lcov+branch 全量报告（拆分前/后）。
- `.work/tmp/cpd-src.txt`：PMD CPD 41 块重复清单。
- `.work/tmp/crap_residual_v114.txt`：CRAP>6 残余 64 处清单。
- `/tmp/mutants-list.txt`：cargo-mutants --list 全仓 2074 变异点清单。

**产品代码（5 处保持行为拆分 + 1 处 DRY 收口，零行为改动）**
- `src/ui/text.rs`：宽度函数表驱动化——CJK 全角区间收口 `WIDE_RANGES` 常量表 +
  `wide()` 助手；`w()` 收敛为查表映射（comp 13→2+2）。
- `src/ui/btn.rs`：`clip_wide_at_edges` 拆出 `clip_left_edge`/`clip_right_edge`
  （左右缘语义各自成函数，comp 13→4+3+3）。
- `src/model/namegen.rs`：`percent_decode` 拆出 `decode_triplet`（完整 %XX 三元组
  判定独立成纯函数，`unwrap_or(0)` 以 `?` 等价消除，comp 11→4+4）。
- `src/model/consistency.rs`：ETag/Last-Modified 双 match 重复收口 `header_unchanged`
  （产品面 DRY，comp 13→6+4）。
- `src/model/checksum.rs`：`find_companion` 拆出 `probe_companion`（读+解析单伴随文件）
  与 `scan_companion_variant`（大小写变体目录扫描；break 语义以"命中即停"保留，
  comp 13→7+5+2）。

**测试基建（DRY，仅 cfg(test) 代码）**
- `src/ui/mod.rs`：三测试模块重复基建单源化——新增 `#[cfg(test)] pub(crate) mod
  testfx`（make_app_in/task_in/render_full/row_strings 四助手），ui_tests/ui_v113_tests/
  ui_v114_tests 的重复体改为原地一行委托（零调用点改动）；随行清理由此产生的
  3 处 unused import，并修正 testfx 的模块路径（私有 `model::task` → 扁平 re-export）。

### 验证证据（crate 根 = project/ezr）
| # | 验证项 | 结果 | 复现命令 |
|---|---|---|---|
| 1 | 全量测试（11 目标，拆分后 + 去重后各一轮） | **1752P/0F ×2** | `cargo test --workspace` |
| 2 | 逐目标对账 | bin 344 / ezr-fixture 30 / ezr_proxy 11 / hardening 107 / g_appcore 265 / g_engine 263 / g_main 2 / g_proxy 12 / g_supervisor 241 / g_throttle_err 167 / v115 310 | `cargo test` 输出逐行 |
| 3 | 静态检查 / 格式 / 架构 | clippy 0 / fmt --check 0 / arch 过 | `cargo clippy --all-targets` / `cargo fmt --check` / `bash scripts/arch_check.sh` |
| 4 | 覆盖率（lcov+branch，--skip property_tests） | 产品函数 494 个、真缺口 0、行覆盖 100%（4372/4372） | `cargo llvm-cov --workspace --lcov --branch -- --skip property_tests` |
| 5 | CRAP（受限近似口径） | >6 由 67 → 64；残余全部 cov=100% | `.work/tmp/analyze_coverage.py`（会话级） |
| 6 | DRY（PMD CPD，--minimum-tokens 50） | 41 块 → 产品面 1 处修复 + 测试面 testfx 单源化 | `.tools/pmd-bin-7.28.0/bin/pmd cpd -l rust --minimum-tokens 50 --dir src` |
| 7 | 变异点（scan/count，未跑变异） | 全仓 2074 点/50 文件；v1.14 触碰 6 文件均 <100 | `cargo mutants --list --line-col=true` |

### 对账口径
- **1752 对账**：与 coder v1.14 基线逐数一致（零新增/删除/改名测试）——本轮纯
  行为保持清理，测试数即基线数；两轮复验（产品拆分后、测试去重后）结果相同。
- **CRAP 口径**：comp = BRDA 条目数 + 1（上界近似，`&&`/`||` 链每项记 2）；cov =
  函数行覆盖（DA 归属按起始行区间）；聚合按（文件，逻辑名）取极值（FNDA=max、
  comp=max、cov=min）。64 处残余的 comp 数字偏上界（如 consistency::check BRDA
  comp=13，源码决策点 ≈6），以源码口径复核后登记，未据此硬拆。
- **度量解析口径**：lcov FN 行全在 FNDA/BRDA/DA 之前（两段式解析：先收清单再按行
  区间归属）；async fn/闭包按逻辑名聚合剥单态化副本；测试函数按名含 `test`/文件在
  tests/ 排除。解析器与口径已沉淀 `packs/_common/notes/rust.md`。
- **行为基线**：FR-01-30/31/32 状态机、FR-01-17 速度、FR-01-94…99 定稿布局、
  FR-01-100/101 详情面板全部未动（既有 1752 测试零改动通过，仅实现内部结构变化）。

### 四要素③/④
- 无 feature 注释块变化（本轮未触碰规格与 QA 文档）。
- 无新增等价突变体登记（本轮未跑变异测试；fmt_block_size 钦定锚点两臂为可证等价
  分支，预登记为等价突变体高发位，hardender 轮注意）。

### 待办与待批（未决项）
1. **【移交 architect 轮】**CRAP>6 残余 64 处的模块边界裁决（分发核/引擎核/绘制核
   是否拆分、bin 入口前导是否抽共享 lib）；header.rs 161 / app/engine.rs 118 变异点
   的单文件拆分裁决。
2. **【移交 hardender 轮】**变异补跑基线更新（v1.13 + v1.14 + 本轮 5 处拆分后代码）；
   fmt_block_size 钦定锚点等价突变体预登记。
3. **【移交 QA 轮】**QA-UC-16…18 套件脚本化（pyte PTY 口径 + OSC 52 序列捕获断言）；
   既有套件旧断言无需改动（本轮未改既有布局）。
4. 无操作者待批项（本轮全部为保持行为清理，无口径变更）。

### 移交建议
- 建议下一步：`bin/swarm begin six-pack/architect`（残余 CRAP/变异点的边界裁决），
  随后 hardender → QA 走完 six-pack 链路；操作者如认可当前清理终态，直接
  `bin/swarm complete` 归档即可。
- 手工冒烟入口（沙箱外有 TTY 环境）：`cargo run --release`（project/ezr）——本轮
  零行为改动，冒烟预期与 coder v1.14 完全一致。
- 归档前无需手动清理（target/ 由 complete 自动清理；`.tools/` 按 .gitignore 不入包）。

By cleaner.
