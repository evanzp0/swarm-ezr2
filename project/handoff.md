# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect　会话：ezr-v146-arch（2026-10-21）
> 上游交接：project/handoff.md（cleaner v141-clean-dns 交接：4198P/0F + 两项移交裁决）
> + 操作者 chat 指令（运行 six-pack/architect）。
> 当轮任务：v141 产物的架构轮——四阶段架构评审、规则 14 白名单收窄与 CPD 产品区
> tile 两项移交裁决、属性测试面沿袭评审与 dns 模块属性补齐。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.33，分期模式）+ `project/mission/phase-01.md`（v1.33）。
- 源码状态 v146（全 15 目标 **4200P/0F**，对账见下）。本轮为架构复核轮：产品代码仅
  dns.rs 可见性提升（`pub(crate)`）+ engine 门面 cfg(test) re-export + 属性测试 +2，
  产品行为零改动。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.33 全量规格 + features/ 15 份 + qa/ 15 份 | 全部交付 | v134 起 3 份 UI feature 过 APS 机检（0 错误级）；v132 轮修订未过机检（见受限项） |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F（v128 口径，CPD 读数不可比见 v141 登记） |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 hardening_g_v147 | 已交付 | 3363P/0F、clippy 0、fmt 0 |
| QA（ezr-qa-final） | QA 规程可执行化 13 端到端套件（qa/runners/）+ 缺陷修复 | 已交付 | e2e 165P/0F/0S、3363P/0F、覆盖 94.37% |
| specifier（v129–v134/v132/v140） | FR-01-106…110 + v1.30/v1.31/v1.32 对齐规格 + v1.33 FR-01-112/D40 | 已交付 | v140 反向自检过（纯新增） |
| coder（v129–v137） | FR-01-106 实现 + 缺陷修复 + v1.30/v1.31 UI 改版 + q 退出鼠标泄漏修复 | 已交付 | 3889P/0F、clippy 0、fmt clean（v137 口径） |
| coder（v138-m3-impl） | m3 对齐全量实现（scrollbar 引擎 + scrollable_panel + About + 头部/对话框/帮助/模态口径） | 已交付 | 3963P/0F、clippy 0、fmt clean |
| coder（v139-m3-padding-fix） | 活跃计数标题 padding 对称；ezr-demo 同步 + demo 文档 §5.4 | 已交付 | 4009P/0F、clippy 0、fmt clean |
| coder（v140-builtin-dns-impl） | FR-01-112：src/engine/dns.rs 内置异步 DNS 解析器 + base_client_builder 挂接 + config hosts_file_path 缝 | 已交付 | 4162P/0F（全 15 目标）、clippy 0、fmt clean |
| cleaner（v141-clean-dns） | 保持行为清理：dns.rs E1–E3 去重 + load_from 测试缝 + 纯追加补测 4 例；arch 规则 14 白名单扩展裁决收口 17 行存量 | 已交付 | 4198P/0F、clippy 0、fmt clean、arch 15/15、dns.rs 覆盖 97.8%、CPD 125 tile（26 产品区口径见裁决①） |
| **architect（v146-arch，本轮）** | **四阶段评审（零违规）+ 两项移交裁决收口 + dns 属性面 +2（含策略 helper 收口）+ arch 阳性对照复测** | **已交付** | **4200P/0F、clippy 0、fmt clean、arch 15/15 + 阳性对照命中实测、CPD 125 tile（本轮新增区零 tile）** |

### 遗留受限项（当前有效）
- **APS 机检沿袭未执行**（v132 修订的 3 份 UI feature 未过 `gherkin-ir-dry-checker`）。
- **qa/runners/ 可执行化同步欠账（沿袭）**：旧断言残留（版本号/对话框标题/默认
  SHA-256 注释/滚动条字形/帮助行数/点外关闭类）；本轮无 UI 面改动，无新增欠账。
- **v147 时序 flake（沿袭）**：本轮全量跑 hardening_g_v147 全绿（未见 flake）；
  建议 QA 轮仍放宽 `wait_state` 预算或加重试。
- **Rust 工具链沙箱不持久（本轮再证）**：全新沙箱完整重装 nightly
  1.101.0-nightly (07e526db9 2026-10-19)（rustup-init USTC dist 直链 + minimal +
  rustfmt/clippy/llvm-tools-preview；crates 经 rsproxy-sparse；rust-lld →
  `~/.local/bin/ld.lld` 桥接）。冷启动配方见 rust.md §1。
- **dns.rs 覆盖率残余 3 行（沿袭 v141 登记）**：167（GaiFallback JoinError→BoxError
  管线，公开 API 不可达）、241/247（A/AAAA 查询成功映射臂，需真实 DNS 应答——
  沙箱无外网 DNS，mock DNS 服务器属 e2e 范畴未引入）。hardender 变异轮若在这三行
  产生存活突变体，按 v141 登记处置（mock DNS 或等价论证）。
- **CPD 口径（v146 复跑更新）**：`pmd cpd -l rust --minimum-tokens 50 --dir src`
  = **125 tile**（pmd 7.28.0，与 v141 总量一致）；产品区/测试区分界按「文件内首个
  `#[cfg(test)]` + property_tests.rs 整体计测试区」口径归一后，产品区形态与
  v141 登记一致（弹窗脚手架对、task_lines 行渲染对、ezr-proxy 请求处理对、
  mouse.rs 惯用样板对、main/ezr-proxy 独立 bin 姿态块对——bin-only crate 姿态
  不属冗余，rust.md 已载）；本轮新增区零 tile。裁决①见移交闭环注记。
- **观察受限突变体（沿袭 hardender 登记，仍开放）**：workers.rs 5 条 + 等价
  download.rs:85:27。
- **ezr-proxy.rs 测试基建（沿袭裁决）**：127 变异点不拆、CRAP/CPD 读数不追责。
- **退出竞态修复的可观测边界（沿袭 v137 登记）**：极端时序下毫秒级鼠标事件残留属
  业界共有限界。
- **沙箱后台进程回收（本轮再证，纪律见 engineering.md）**：detached 后台进程随
  工具调用结束/超时击杀连带清场——长验证一律前台分段（cargo 增量与 `curl -C -`
  跨调用续传），不用后台进程。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D40。
- **本轮零新增实现定义值**（架构复核轮无行为改动；GRACE=1s 等常量原样）。

## 二、当前产出情况（architect v146-arch）

### 本会话产物清单
- `project/ezr/src/property_tests.rs`（属性测试 +2，62→64）：
  - **prop_hosts_parse_contract**：dns `HostsFile::from_lines`/`lookup` 契约——
    任意文本全函数解析、别名大小写无关等价、逐别名命中列表精确特征化（首见序
    去重 oracle）、未声明探针 None、行尾注释免疫（对照文本逐点相等）。
  - **prop_merge_answers_ipv4_first_dedup**：dns `merge_answers` 契约——输出
    零重复（集合口径）、v4 家族先于 v6 家族、首见序全局去重独立 oracle 相等。
  - **arb_ipv4/arb_ipv6_ula 策略 helper 单源**：两条新属性的 IP 生成策略脚手架
    收口（生成分布不变；CPD 自配对消解，本轮新增区零 tile）。
- `project/ezr/src/engine/mod.rs`：cfg(test) 测试可达性门面 re-export
  `pub(crate) use dns::{merge_answers, HostsFile}`（与 throttle/error 既有形态
  逐字同构，机制固有 allow 注记同源；产品构建不产生该路径，窄接口零增量）。
- `project/ezr/src/engine/dns.rs`：`merge_answers` 升 `pub(crate)`（门面
  re-export 前置条件，notes/rust.md「门面 re-export」手法①；私有模块内不外泄）。
- `.work/tmp/arch_review.md`：四阶段评审三段式全文（现状评估 + 违规清单 +
  改进动作；全四阶段零违规；渲染聚合逐点检查 3 处命中均为纯几何）。
- `.work/tmp/cpd_v146*.txt/.xml`、`.work/tmp/prop_dns_draft.rs`：度量产物与
  属性草稿（会话级）。
- 工具侧经验沉淀（宪法第三章，3 条）：rust.md 显示层伪影条目修复自毁文本 +
  补 `cat -A` 非字节级证据细节；rust.md CPD tile 条目补 proptest 策略脚手架
  自配对形态与策略 helper 消解法；engineering.md 后台回收条目补「回收半径不限
  本调用」（超时击杀连带清场 detached 进程）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 基线复核（动工前） | 4198P/0F（与 v141 精确一致） | `cargo test --all-features` |
| 新属性 | 64P/0F（62 + 2） | `cargo test --bin ezr property_tests::` |
| ezr 全量测试 | **4200P / 0F**（全 15 目标） | `cargo test --all-features` |
| ezr clippy | 0 警告（all/pedantic/nursery 全开） | `cargo clippy --all-features --all-targets` |
| ezr fmt | clean | `cargo fmt --check` |
| arch_check | **15/15 全绿**；阳性对照：ui/header.rs 产品区注入 `app.scroll = 3;` → 规则 14 命中（header.rs:31），移除后复绿 | `bash scripts/arch_check.sh` |
| 测试名唯一性 | bin 目标零重复 | `cargo test --bin ezr -- --list \| sort \| uniq -d` |
| DRY（CPD） | **125 tile**，本轮新增区（property_tests.rs 行号 ≥ 新增起点）零 tile；产品区形态与 v141 登记一致 | `pmd cpd -l rust --minimum-tokens 50 --dir src`（pmd 7.28.0） |

### 对账口径
- 测试基线：上轮 handoff 记 4198P/0F。本轮全量 **4200P/0F = 4198 + 2**——
  2 = 本轮新增属性测试数；`property_tests.rs` 仅经 main.rs 的
  `#[cfg(test)] mod property_tests;` 挂载（main.rs 从未被任何 tests/ crate
  `#[path]` 挂载，仅注释提及），故 +2 即全量 +2，无跨 crate 倍增；口径闭合。
- 规格面对账：features/qa/mission 零改动（架构轮只读纪律）；qa/runners 无 DNS
  相关断言，影响面零触碰；产品行为零改动（改造前「既有测试会全绿吗」裁决：
  全量套件即行为锁，4200P/0F 闭环）。

### 待办与待批（未决项）
- **D40 微决策待操作者裁决（沿袭）**：宽限期 300ms / 加 `dns` 配置键等改判，
  改动面 = 引擎 dns 模块常量与构建函数各一处。
- 沿袭待办：qa/runners 同步欠账（QA 轮）、APS 机检补跑、v147 flake 预算放宽
  （本轮全量未见 flake，仍建议 QA 轮放宽）。

### 移交建议
- 本轮架构复核已闭环（四阶段零违规 + 两项移交裁决收口 + 属性面 +2 + 全量门禁
  绿）。**两项移交裁决闭环注记**：①CPD 产品区 tile——维持登记不立规拆样板
  （三族均惯用样板形态，拆样板风险高于收益，rust.md 既有纪律承载；产品区读数
  维持、本轮零新增）；②arch 规则 14 白名单收窄——维持 cleaner v141 裁决
  （精确词族 + 政策字段不豁免；阳性对照本轮复测命中，白名单外写面残余 = 空）。
  两项均已在 arch_check.sh 注释与本 handoff 落账，下游无需再议。
- 可径直运行 `six-pack/hardender`（以本 handoff 为上游交接内容）：dns.rs 变异
  面 = v141 登记的 21 点 + 本轮零新增；覆盖率残余 3 行处置按 v141 登记口径；
  其后按链路 QA。若操作者希望先收敛 D40 待批项，改判面极小（常量 + 构建函数
  各一处），由 specifier/coder 承接。
- 移交仅为建议，不构成自动流转（宪法第一章）。

By architect.
