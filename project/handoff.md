# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect 终态　会话：v126-arch（任务 v126-arch）
> 本轮（架构批次）：对 cleaner v125-clean 交付做四阶段架构评审（UI/核心分离、依赖规则、
> 信息隐藏、局部质量+渲染聚合逐点检查）与保持行为改造；全量 **2523P/0F**（单测基线
> 2506 + tab_counts 1×7 挂载上下文 + 属性 +10）、clippy 0、fmt 0、**arch 14 规则过**
> （新增 13/14）、属性测试独立口径 **57P/0F**（+10）。
> 行为保持实证：本轮唯一产品改造为页签计数访问器提取（机械等价），渲染断言测试零改动全绿。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.1，分期模式）+ `project/mission/phase-01.md`（**v1.18**）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（v115 前历轮） | mission/phase-01.md 全量 FR/D 裁决表 + features/ 13 份 + qa/ 13 份 | 全部交付 | parser/dry-check 全过 |
| coder（v120-code-two-batch） | FR-01-102 连接速度同管线 + FR-01-103 仅添加/立即下载三钮 | 已交付 | 2438P/0F、clippy 0 |
| coder（v121-code-batch3） | FR-01-104 1MB 块下限 + FR-01-105 实时字节口径 + D30 下调即时收缩 | 已交付 | 2470P/0F、clippy 0 |
| coder（v122-code-batch4） | FR-01-06 v1.17 双对话框粘贴 + FR-01-51 v1.17 清码三分支 + mock hold 门控根治 flake | 已交付 | 2503P/0F、clippy 0 |
| specifier（v124-spec-batch5） | FR-01-51 v1.18 直判规格 + D34 v1.18 注记 + 场景 12 注/QA-IC-12 v1.18 | 已交付 | parser + dry-check 过 |
| coder（v124-code-batch5） | 修订 A 终态 sidecar 即时落盘 + 修订 B 清码 retry 文件完整性直判 + 测试 3 条 | 已交付 | 2506P/0F、clippy 0、fmt 0、arch 12 过 |
| cleaner（v125-clean） | 保持行为清理 8 处 + 四度量复核（覆盖率/CRAP/DRY/变异点 scan） | 已交付 | 2506P/0F、clippy 0、fmt 0、arch 12 过、CPD tiles 55 |
| architect（v126-arch，本轮） | 四阶段架构评审 + 页签计数访问器提取 + 属性测试 +10 + arch 规则 13/14 + 经验沉淀 2 条（见二） | 已交付 | 2523P/0F、clippy 0、fmt 0、arch 14 过、属性 57P/0F |

### 遗留受限项（当前有效）
- e2e 六套件滞后于规程（累计）：`suite_add_task.py`、`suite_ui_conns.py`、
  `suite_modify_task.py`、`suite_persistence_config.py`、`suite_tui_display.py`、
  `suite_integrity_check.py`——QA 会话须先同步全部受影响套件再跑 e2e（见二）。
- **CRAP 为受限近似口径**（Rust 无标准 CRAP 工具；llvm-cov lcov BRDA comp=分支记录数+1
  上界近似 + 源码决策点复核，近似口径偏差方向见 `packs/_common/notes/rust.md`）：
  存量 **72 条近似 CRAP>6** 全部为既有分发核心/UI 绘制/解析函数（on_evt、draw、block_worker、
  from_toml 等，comp≈分支数属「拆不动残余高 comp 点」登记口径），非历轮新增；cleaner
  v125-clean 改动面函数经源码复核 comp 全部 ≤6，本轮（architect）改动面仅 1 访问器
  + 测试/脚本，无新增高 comp 点。
- **CPD 剩余 55 tiles**（PMD 7.28，-l rust --minimum-tokens 50，cleaner v125-clean 口径）：
  18 个 supervisor 测试区 mock 样板 + 37 个分发臂/UI 绘制/既有夹具形态，均为历轮登记既有项。
  本轮未复跑 CPD（architect 职责度量不含 DRY；本轮新增面为唯一代码——新增访问器/属性/
  脚本规则均无拷贝形态，不引入新 tile），沿用 cleaner 口径交付 hardender。
- **本轮沙箱为全新环境**：工具链已重装——rustup nightly（rustc 1.101.0-nightly
  32dba69d6 2026-10-09，minimal + rustfmt + clippy + llvm-tools-preview），安装位置
  `~/.cargo` + `~/.rustup`，新 shell 需 `export PATH="$HOME/.cargo/bin:$PATH"` 补注入
  （rust.md「PATH 注入不跨会话持久」）；首轮全量编译+测试实证环境完整。`.tools/` 为空
  （PMD/APS 等按需重建，hardender 跑变异前需装 cargo-mutants）。
- 无其他受限项。

### 实现定义值登记（当前有效）
- 本轮无新增实现定义值。沿袭有效：D28/D29（仅添加落态与三钮布局）、D30（下调即时收缩）、
  D31（1MB 下限）、D32（Enter 语义）、D33（1s 归零时延）、D34（清码 R = 不校验直接收尾，
  v1.18 实现精化见 coder v124-code-batch5 轮）。修订 B fallback（文件不完整 → 作废断点
  从头重下）为 FR-01-22 作废语义同源的实现定义值，操作者可改判（见待办）。

## 二、当前产出情况（architect v126-arch）

### 本会话产物清单
- `src/app/mod.rs`：新增 `App::tab_counts()` 访问器——页签行「正在下载 (n) │ 已完成 (n)」
  计数口径自渲染路径提取（architect v116 提取序列的后续批次；下标对齐 FILTERS，两项之和
  恒等于任务总数）；新增 cfg(test) 无头单测 `tab_counts_split_by_done_state`。
- `src/ui/header.rs`：页签行内联聚合（`doing` filter-count + `done` len 减法）改为消费
  `app.tab_counts()`（机械等价，渲染输出逐字符不变）。
- `src/property_tests.rs`：属性测试 +10（4 缺口批次）——① checksum 摘要管线：
  `prop_digest_bytes_output_domain`（7 算法输出域：小写 hex/位数恰为表值/被 validate_value
  原样接受）、`prop_digest_file_matches_bytes`（流式 digest_file == 内存 digest_bytes
  守恒，1MB 内随机内容）；② 代理端点 url：`prop_endpoint_url_bracket_iff_colon`（括号
  ⟺ ip 含冒号，任意域）、`prop_endpoint_url_ipv6_parseable`（全 8 段可解析 + 端口保真）、
  `prop_endpoint_url_plain_ip_parseable`（点分形态可解析）；③ Task 纯函数：
  `prop_task_progress_bounded`（值域 [0,1] 含 total=0/downloaded 越界）、
  `prop_task_eta_none_gates`（非下载态/零速恒 None 门语义）、
  `prop_task_chunk_info_consistent`（y=chunk_total 口径 + x≤y 钳制）、
  `prop_task_paths_suffix_invariants`（路径三件套后缀不变量，namegen 常量单源）；
  ④ 速度滑窗：`prop_speed_window_rate_bounded`（任意推进序含累计回退下 rate≥0、
  极差/0.05s 上界、last_push 单调）。
- `scripts/arch_check.sh`：12 规则 → **14 规则**——规则 13【组装根】app 层零 ui 依赖
  （`use crate::ui` 扫描）；规则 14【回填白名单】ui 渲染路径对 App 的写操作只允许
  rect/area/visible 回填字段（写形态含赋值+容器方法；扫描域 = 各 ui 文件首个
  `#[cfg(test)]` 前产品区，awk 截断——测试夹具播种状态属合法豁免）。两规则阳性对照
  本轮实测（探针注入 → FAIL → 移除 → 全过），对照记录在脚本尾注。
- `packs/_common/notes/rust.md`：经验沉淀 2 条——「复合结构的属性生成：逐段合法 ≠
  整体合法（可解析断言锁合法形态、纯格式契约才用任意域；proptest 属性体内早退禁裸
  return）」、「grep 级白名单规则的文件内区域扫描域切分（awk 截 cfg(test) 前）+
  容器方法写形态枚举」。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量单测 | 2523P / 0F（13 目标） | `cargo test --workspace` |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| fmt | 0 偏差 | `cargo fmt --check` |
| 架构边界 | 14 规则全过（13/14 阳性对照实测） | `bash scripts/arch_check.sh` |
| 属性测试（独立口径） | 57P / 0F（47 → 57，+10） | `cargo test --bin ezr property_tests::` |

### 对账口径（基线 → 终态）
- 基线 2506P/0F（cleaner v125-clean 终态；本轮全新沙箱重装工具链后全新编译实测复现一致）
  → 终态 **2523P/0F**，**Δ = +17**，分项可复算：单测 +7（`tab_counts_split_by_done_state`
  1 条 × 7 个编译上下文——bin ezr + 6 个挂载 app/mod.rs 的 hardening 测试 crate，
  rust.md「挂载树 cfg(test) 重复注册合法」机制）+ 属性 +10（property_tests 仅 bin ezr
  1 个上下文）。
- **对账隔离**：属性测试独立计数（47 → 57），不混入单测基线对账（SKILL「属性测试与
  常规单测对账隔离」）；单测基线对账只计 tab_counts 单测。
- 行为保持的机械等价性依据：唯一产品面改动为 tab_counts 访问器提取（函数提取/调用替换，
  无控制流与渲染输出变化）；2523 全绿（含 ui 渲染断言与页签行文本断言）+ clippy 0 +
  fmt 0 为回归实证。

### QA 会话须知（e2e 前必读）
- 套件同步清单（累计，不变）：六套件（见一·遗留受限项）先同步再跑 e2e。
- 本轮不改变任何可观测行为（页签计数口径与渲染输出逐字符不变），套件断言无需调整。

### 待办与待批（未决项）
- 无新增裁决待批。沿袭待批：修订 B fallback（文件不完整 → 作废断点从头重下）为
  实现定义值，操作者可改判（如改判「保留台账从缺失处补下」，代价：稀疏文件空洞风险，
  不建议）。

### 移交建议
- 下一角色：six-pack/hardender（SKILL 链序 architect → hardender → QA）。架构批次已
  收口：模块边界 14 规则全过（新增 app→ui 依赖禁令与 ui 回填白名单两条防漂移护栏）、
  渲染聚合口径提取完成、属性测试独立面 57 条就绪（hardender 变异后属性命令可作独立
  回归口径）。变异范围决策可沿用 cleaner v125-clean 登记的变异点 scan 数字
  （supervisor.rs=86、tasks.rs=30，均 <100 门槛）；`.tools/` 全新，跑变异前需先安装
  cargo-mutants。
