# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner　会话：ezr-v141-clean-dns（2026-10-21）
> 上游交接：project/handoff.md（coder v140-builtin-dns-impl 交接：FR-01-112 v140
> 4162P/0F）+ 操作者 chat 指令（运行 six-pack/cleaner）。
> 当轮任务：v140 产物的保持行为清理轮——DRY/重复面检视（dns.rs 自愈链、类型别名
> 扩面裁决）、覆盖率提升、CRAP/DRY/变异点三工具审查、arch 规则 14 白名单扩展裁决。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.33，分期模式）+ `project/mission/phase-01.md`（v1.33）。
- 源码状态 v141（全 15 目标 **4198P/0F**，对账见下）。上轮 v140 FR-01-112 内置 DNS
  解析器，本轮在其上做保持行为清理（产品代码仅 dns.rs 微调 + arch_check.sh 白名单扩展）。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.33 全量规格 + features/ 15 份 + qa/ 15 份 | 全部交付 | v134 起 3 份 UI feature 过 APS 机检（0 错误级）；v132 轮修订未过机检（见受限项） |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F；CPD 17→15（v128 口径，与本轮读数不可比，见受限项） |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 hardening_g_v147 | 已交付 | 3363P/0F、clippy 0、fmt 0 |
| QA（ezr-qa-final） | QA 规程可执行化 13 端到端套件（qa/runners/）+ 缺陷修复 | 已交付 | e2e 165P/0F/0S、3363P/0F、覆盖 94.37% |
| specifier（v129–v134/v132/v140） | FR-01-106…110 + v1.30/v1.31/v1.32 对齐规格 + v1.33 FR-01-112/D40 | 已交付 | v140 反向自检过（纯新增） |
| coder（v129–v137） | FR-01-106 实现 + 缺陷修复 + v1.30/v1.31 UI 改版 + q 退出鼠标泄漏修复 | 已交付 | 3889P/0F、clippy 0、fmt clean（v137 口径） |
| coder（v138-m3-impl） | m3 对齐全量实现（scrollbar 引擎 + scrollable_panel + About + 头部/对话框/帮助/模态口径） | 已交付 | 3963P/0F、clippy 0、fmt clean |
| coder（v139-m3-padding-fix） | 活跃计数标题 padding 对称；ezr-demo 同步 + demo 文档 §5.4 | 已交付 | 4009P/0F、clippy 0、fmt clean |
| coder（v140-builtin-dns-impl） | FR-01-112：src/engine/dns.rs 内置异步 DNS 解析器 + base_client_builder 挂接 + config hosts_file_path 缝；新测试 +17（挂载后 +153） | 已交付 | 4162P/0F（全 15 目标）、clippy 0、fmt clean、arch 规则 8 过/规则 14 沿袭未过 |
| **cleaner（v141-clean-dns，本轮）** | **保持行为清理：dns.rs E1–E3 去重 + load_from 测试缝 + 纯追加补测 4 例（挂载后 +36）；arch 规则 14 白名单扩展裁决收口 17 行存量** | **已交付** | **4198P/0F（全 15 目标）、clippy 0、fmt clean、arch 15/15 全绿、dns.rs 覆盖 97.8%、CRAP 达标（登记口径）、变异点 21 ≪ 100、CPD 产品区 26 tile（零新增）** |

### 遗留受限项（当前有效）
- **APS 机检沿袭未执行**（v132 修订的 3 份 UI feature 未过 `gherkin-ir-dry-checker`）。
- **qa/runners/ 可执行化同步欠账（沿袭）**：旧断言残留（版本号/对话框标题/默认
  SHA-256 注释/滚动条字形/帮助行数/点外关闭类）；本轮为 dns.rs 内部清理 + arch
  脚本白名单，无 UI 面改动，无新增欠账。
- **v147 时序 flake（沿袭）**：本轮全量两片跑中 hardening_g_v147 全绿；建议 QA 轮
  放宽 `wait_state` 预算或加重试。
- **Rust 工具链沙箱不持久（本轮再证）**：全新沙箱完整重装 nightly
  1.101.0-nightly (07e526db9 2026-10-19)（rustup-init USTC dist 镜像 + minimal +
  rustfmt/clippy/llvm-tools-preview；crates 经 rsproxy-sparse）。stable 未装
  （rust-toolchain.toml 钉 nightly 生效，本轮全程未需 stable）。冷启动配方见
  rust.md §1。
- **沙箱磁盘预算（本轮按归属管理）**：全量 target ≈ 4.8G、llvm-cov-target ≈ 1.3G；
  本轮顺序 = 基线验证 → cargo clean → llvm-cov 一次跑完提取度量（lcov 落
  `.work/tmp/`）→ 删 llvm-cov-target 还盘 → 终验走主 target 增量。构建/全量前查
  `df -h /`。
- **dns.rs 覆盖率残余 3 行（本轮登记）**：167（GaiFallback JoinError→BoxError 管线，
  公开 API 不可达）、241/247（A/AAAA 查询成功映射臂，需真实 DNS 应答——沙箱无外网
  DNS，mock DNS 服务器属 e2e 范畴未引入）。影响：hardender 变异轮若在这三行产生
  存活突变体，按本条登记处置（mock DNS 或等价论证）。
- **CPD 口径漂移（本轮登记）**：`pmd cpd -l rust --minimum-tokens 50 --dir src`
  = 125 tile（产品区 26 / 测试区 99，按文件内首个 `#[cfg(test)]` 分解）；v128 登记
  的 17→15 为彼时工具版本与树状态，不可直接对比。产品区 26 tile 均为历轮累积的
  既有惯用样板形态（about/help/delete 弹窗脚手架对、task_lines 行渲染对、
  ezr-proxy 请求处理等），dns.rs 零 tile、本轮零新增；移交 architect 轮裁量。
- **ezr-demo 参照仓库 fmt/clippy 历史漂移（v139 登记）**：维持不修（demo 非验证对象）。
- **观察受限突变体（沿袭 hardender 登记，仍开放）**：workers.rs 5 条 + 等价 download.rs:85:27。
- **ezr-proxy.rs 测试基建（沿袭裁决）**：127 变异点不拆、CRAP/CPD 读数不追责。
- **退出竞态修复的可观测边界（沿袭 v137 登记）**：极端时序下毫秒级鼠标事件残留属业界共有限界。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D40。
- **本轮零新增实现定义值**（清理轮无行为改动；GRACE=1s 等常量原样）。

## 二、当前产出情况（cleaner v141-clean-dns）

### 本会话产物清单
- `project/ezr/src/engine/dns.rs`（产品区微调 + 测试补齐，行为不变）：
  - **E1** `race_with_grace` 挂起腿绑定去重——if/else 两分支重复的
    `let pending_leg = if first_is_b {a} else {b}` 提升到分支外（单一绑定，
    两臂各自消费；编译器 + 既有 race 5 测试锁定等价）。
  - **P1** `HostsFile::load()` 提取 `load_from(path)` 测试缝（load 保持平台路径
    适配壳；SKILL「迁移到可测试模块」形态）。
  - **E2** 测试 `MockFallback::err()` 构造器（与 ok() 对称，替换一处结构体字面量）。
  - **E3** 两例 client 级测试脚手架（client 构建 + mini http 请求 + 收尾）提取
    `fetch_through_builtin` 单源（engineering.md「同文件多调用点重复体去重」形态）。
  - **纯追加 4 测试**（dns 模块 17→21）：`load_missing_path_yields_empty_table`
    （Err 臂）、`race_first_is_b_ok_abandons_slow_leg_after_grace`（b 先答交换臂序
    确定性覆盖，消除 both_err 用例的 50/50 非确定覆盖）、
    `parallel_lookup_unreachable_nameserver_yields_empty`（双腿接线：不可达
    nameserver 127.0.0.1:1 + 150ms/1 次 → 双腿败 → 空集）、
    `client_hosts_miss_falls_through_builtin_to_failure`（client 级 hosts 未命中 →
    内置臂 → 回落全链，.invalid 必败断言）。
- `project/ezr/scripts/arch_check.sh`：规则 14 白名单扩展裁决——扩展为
  `_pos`/`_scrollbar`/`max_hscroll|max_scroll`/`follow` + `(help|pieces)_scroll`
  精确词族（17 行渲染回填存量收口）；**裸 `scroll`/`conns_scroll`/`conns_hscroll`
  为键盘导航政策字段，不豁免**（收窄于交接建议的宽泛词族 `scroll|pos|follow`——
  其会静默豁免 3 个政策字段）；注释同步登记裁决依据；POSITIVE-CONTROL 尾注补记
  本轮对照。
- 工具侧经验沉淀（宪法第三章，4 条 rust.md + 1 条 engineering.md 扩展）：BRDA
  select! 不落行号口径；DNS 解析器双腿不可达 nameserver 测试法（含
  `ResolverOpts` `#[non_exhaustive]` 字面量禁令）；arch 白名单扩展前同词族字段
  枚举判定法；镜像大文件断点续传以 content-length 字节数为准。
- 工作区侧：`.work/plan.md`（13 步全勾选）、度量产物在 `.work/tmp/`
  （lcov_v141.out、cpd_v141.xml、mutants_list.txt、crap_dns.py）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 基线复核 | 4162P/0F（与 v140 精确一致） | `cargo test --all-features`（动工前） |
| dns 模块测试 | 21P/0F（17 既有 + 4 新增） | `cargo test --bin ezr engine::dns` |
| ezr 全量测试 | **4198P / 0F**（全 15 目标） | `cargo test --all-features` |
| ezr clippy | 0 警告（all/pedantic/nursery 全开） | `cargo clippy --all-features --all-targets` |
| ezr fmt | clean | `cargo fmt --check` |
| arch_check | **15/15 全绿**（规则 14 收口后）；阳性对照 `app.scroll = 3` 注入 header.rs 产品区 → 规则 14 命中（header.rs:231），移除后全绿 | `bash scripts/arch_check.sh` |
| dns.rs 覆盖率 | **97.8%**（136/139 产品行；残余 3 行见受限项）；bin harness 口径 | `cargo llvm-cov --branch --bin ezr --all-features --lcov --output-path <out>` |
| dns.rs CRAP | 全函数 ≤6（登记口径）——race_with_grace 源码 comp=5（select! 二臂 + 三处 if/else；BRDA 6 记录全部已命中，select! 不落行号为宏口径偏差），cov=100% 后 CRAP=5；其余函数 BRDA 口径即 ≤6 | 解析脚本 `.work/tmp/crap_dns.py`（口径见脚本头注） |
| DRY（CPD） | 125 tile = 产品区 26 + 测试区 99；dns.rs 零 tile、本轮零新增 | `pmd cpd -l rust --minimum-tokens 50 --dir src` |
| 变异点扫描 | dns.rs **21 点**（≤100，无需拆分） | `cargo mutants --list --line-col=true` |
| 测试名唯一性 | 零重复 | `cargo test --bin ezr -- --list \| sort \| uniq -d` |

### 对账口径
- 测试基线：上轮 handoff 记 4162P/0F。本轮全量 **4198P/0F = 4162 + 36**，
  36 = **4 × 9**——4 个新 dns 测试经 `#[path]` 挂载机制进入 9 个目标（bin ezr 1 +
  hardening 测试目标 8，v140 已逐一实测登记的同一挂载图），口径闭合无漂移。
- 规格面对账：features/qa/mission 零改动（清理轮只读纪律）；qa/runners 无 DNS
  相关断言，影响面零触碰。

### 待办与待批（未决项）
- **D40 微决策待操作者裁决（沿袭）**：宽限期 300ms / 加 `dns` 配置键等改判，
  改动面 = 引擎 dns 模块常量与构建函数各一处。
- **arch 规则 14 白名单收窄裁决请 architect 轮复核**：本轮按「政策同形字段不豁免」
  收窄于交接建议（裸 scroll/conns_scroll/conns_hscroll 保持在护栏内），扩展词族与
  依据已登记在 arch_check.sh 注释；如需再放宽/收窄由 architect 轮定夺。
- 沿袭待办：qa/runners 同步欠账（QA 轮）、APS 机检补跑、v147 flake 预算放宽。

### 移交建议
- 本轮清理已闭环（行为零改动 + 全量门槛绿 + 三工具读数登记 + arch 全绿）。可径直
  运行 `six-pack/architect`（以本 handoff 为上游交接内容）：①复核规则 14 白名单
  收窄裁决与 CPD 产品区 26 tile 既有形态（是否立规收口）；②属性测试面沿袭评审；
  其后按链路 hardender → QA（hardender 注意 dns.rs 变异点 21 与覆盖率残余 3 行登记）。
- 移交仅为建议，不构成自动流转（宪法第一章）。

By cleaner.
