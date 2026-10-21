# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-v140-builtin-dns-impl（2026-10-21）
> 上游交接：project/handoff.md（specifier v140-spec-builtin-dns 交接：phase-01 v1.33
> FR-01-112/D40）+ 操作者 chat 指令（Chrome 内建 async DNS resolver 同机制给 ezr；
> 「ezr 是要支持 linux/mac/windows 的」跨平台澄清）。
> 当轮任务：FR-01-112 实现——引擎 HTTP 客户端内置异步 DNS 解析器（hosts → A/AAAA
> 并行独立查询 + 1s 宽限 → 回落系统解析），TDD + 全量门槛。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.33，分期模式）+ `project/mission/phase-01.md`（v1.33）。
- 源码状态 v140（全 15 目标 **4162P/0F**，对账见下）。上轮 v139 活跃计数 padding 修复，
  本轮在其上实现 v1.33 新增 FR-01-112。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.33 全量规格 + features/ 15 份 + qa/ 15 份 | 全部交付 | v134 起 3 份 UI feature 过 APS 机检（0 错误级）；v132 轮修订未过机检（见受限项） |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F；CPD 17→15 |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 hardening_g_v147 | 已交付 | 3363P/0F、clippy 0、fmt 0 |
| QA（ezr-qa-final） | QA 规程可执行化 13 端到端套件（qa/runners/）+ 缺陷修复 | 已交付 | e2e 165P/0F/0S、3363P/0F、覆盖 94.37% |
| specifier（v129–v134/v132/v140） | FR-01-106…110 + v1.30/v1.31/v1.32 对齐规格 + **v1.33 FR-01-112/D40** | 已交付 | v140 反向自检过（纯新增） |
| coder（v129–v137） | FR-01-106 实现 + 缺陷修复 + v1.30/v1.31 UI 改版 + q 退出鼠标泄漏修复 | 已交付 | 3889P/0F、clippy 0、fmt clean（v137 口径） |
| coder（v138-m3-impl） | m3 对齐全量实现：scrollbar.rs 引擎 + scrollable_panel.rs 组件 + About 窗口 + 头部/对话框/校验 NONE/帮助 44 行/模态统一口径；新测试 +74 | 已交付 | 3963P/0F、clippy 0、fmt clean |
| coder（v139-m3-padding-fix） | 活跃计数标题 padding 对称（active_title 单源 + 守卫）；ezr-demo 同步 + demo 文档 §5.4 | 已交付 | 4009P/0F、clippy 0、fmt clean |
| **coder（v140-builtin-dns-impl，本轮）** | **FR-01-112 实现：src/engine/dns.rs 内置异步 DNS 解析器（BuiltinResolver + HostsFile + race_with_grace + merge_answers + resolve_flow + SystemFallback/GaiFallback；GRACE=1s）+ base_client_builder 挂接（构造失败降级）+ model/config.rs `hosts_file_path()` 平台适配缝；新测试 +17（挂载机制下全量 +153）** | **已交付** | **ezr 4162P/0F（全 15 目标）、clippy 0、fmt clean、arch_check 规则 8 过（本轮命中后收口 config 缝修复）/规则 14 沿袭未过** |

### 遗留受限项（当前有效）
- **arch_check 规则 14（ui 回填白名单）未过——沿袭 17 行同一集合，全部含 `scroll` 或 `pos`**：
  本轮零新增回填字段；移交建议不变（cleaner/architect 轮扩展白名单正则 `scroll|pos|follow`）。
- **APS 机检沿袭未执行**（v132 修订的 3 份 UI feature 未过 `gherkin-ir-dry-checker`）。
- **qa/runners/ 可执行化同步欠账（沿袭）**：旧断言残留（版本号/对话框标题/默认 SHA-256
  注释/滚动条字形/帮助行数/点外关闭类）；本轮为引擎传输层改动、无 UI 面，**无新增欠账**
  （qa/runners 无 DNS 相关断言，影响面核证零触碰）。
- **v147 时序 flake（沿袭）**：本轮全量跑中 hardening_g_v147 376P/0F 一次过，与登记的偶发
  行为一致；建议 QA 轮放宽 `wait_state` 预算或加重试。
- **Rust 工具链沙箱不持久（沿袭登记）**：本轮沙箱 nightly 1.101.0-nightly (3960fa69b
  2026-10-12) 已在位（rust-toolchain.toml 钉定生效），stable 1.99.1 共存；冷启动配方见
  rust.md §1。
- **沙箱磁盘预算（沿袭，本轮再命中）**：全量跑中曾报 `StorageFull`（ezr-fixture 2 用例
  env 失败）——清 `target/debug/incremental`（2.2G）后复跑全绿；**该 2F 为环境性误报，
  与本轮改动无关**。构建/全量前查 `df -h /`。
- **ezr-demo 参照仓库 fmt/clippy 历史漂移（v139 登记）**：维持不修（demo 非验证对象）。
- **观察受限突变体（沿袭 hardender 登记，仍开放）**：workers.rs 5 条 + 等价 download.rs:85:27。
- **ezr-proxy.rs 测试基建（沿袭裁决）**：127 变异点不拆、CRAP/CPD 读数不追责。
- **退出竞态修复的可观测边界（沿袭 v137 登记）**：极端时序下毫秒级鼠标事件残留属业界共有限界。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D40。
- **本轮零新增实现定义值**：D40 四项微决策（宽限 1s/IPv4 前置/回落链/无开关）按 specifier
  既有登记逐字落地——`GRACE = 1s` 常量单源、`merge_answers` v4 前置去重、
  hosts→内置→系统解析链、无配置开关（构造失败 + 运行时失败双重自动降级）。

## 二、当前产出情况（coder v140-builtin-dns-impl）

### 本会话产物清单
- `project/ezr/Cargo.toml`：①依赖新增 `hickory-resolver = "0.24"`（`default-features =
  false`，`features = ["system-config", "tokio-runtime"]`——FR-01-112② 跨平台系统 DNS
  配置读取单源；cargo tree 核证全树单副本无重复）；②dev-dependencies 新增
  `tokio = { version = "1", features = ["test-util"] }`（仅测试侧：`start_paused` 前置，
  产品构建不增面）。
- `project/ezr/src/engine/dns.rs`（**新增**，FR-01-112 全量实现 + 17 测试）：
  - `BuiltinResolver`（impl `reqwest::dns::Resolve`）：解析次序自愈链单源
    `resolve_flow`——①hosts 命中即返（不触内置/回落）→ ②内置并行双腿命中即返
    （不触回落）→ ③回落系统解析（`GaiFallback`，std `ToSocketAddrs` 经
    `spawn_blocking`；失败走既有连接失败臂，零新增失败形态）。
  - `HostsFile`：hosts 表解析（`#` 注释、IP 字面量行首、别名大小写无关归一、
    同别名同 IP 去重/异 IP 保留、畸形行跳过不中止、读取失败空表非致命）；路径经
    `model::config::hosts_file_path()` 适配缝（arch 规则 8）。
  - `race_with_grace`（D40①）：双腿并行起跑；任一腿**成功**后另一腿至多再等
    `GRACE = 1s`（超时弃置 = None）；先答腿**失败**则等另一腿到其自身超时（不因宽限
    提前放弃唯一可用答案）；双腿皆败原样返回进回落。
  - `merge_answers`（D40②）：IPv4 前置、跨腿去重、保序（双栈均健康时连接层
    happy-eyeballs 兜底，序不影响正确性）。
  - `BuiltinResolver::try_system()`：`TokioAsyncResolver::tokio_from_system_conf()`
    单源构造（unix = `/etc/resolv.conf`、windows = 适配器配置）；失败上抛由
    `base_client_builder` 整体降级（D40④）。
- `project/ezr/src/engine/mod.rs`：①`mod dns;` 挂载；②`base_client_builder()` 挂接
  内置解析器（`dns_resolver(Arc::new(...))`，直连基线与端点 client 共用单一事实来源；
  构造失败整体降级系统解析路径，行为与既有版本一致——FR-01-112④/D40④）。
- `project/ezr/src/model/config.rs`：新增 `hosts_file_path()` 平台适配缝（Linux/macOS =
  `/etc/hosts`、Windows = `%SystemRoot%\System32\drivers\etc\hosts`（SystemRoot 缺失回退
  `C:\Windows`））——env 读取收口 config 缝（arch 规则 8，与 `home_dir`/`ezr_dir` 同缝）。
- `packs/_common/notes/rust.md` 经验沉淀 3 条：①§6——reqwest 自定义 DNS 解析器实现形态
  （dns_resolver + tokio_from_system_conf；hickory rdata 新类型 `.0`；跨 await 持有
  `Pin<&mut dyn Future>` 必须 `+ Send` 擦除否则 resolve future 非 Send）；②§6——
  `start_paused` 两前置（test-util dev feature + `tokio::time::Instant` 而非 std Instant）；
  ③§8——新增模块平台 env 读取预先收口适配缝（Windows hosts 路径案例）。
- 工作区侧（随归档）：`.work/plan.md`（8 步全勾选）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| TDD 红步 | E0432 ×8（`merge_answers`/`race_with_grace`/`resolve_flow`/`BuiltinResolver`/`GaiFallback`/`HostsFile`/`SystemFallback`/`GRACE` 未定义）+ E0425 `BoxError` | `cargo test --bin ezr engine::dns`（实现前） |
| TDD 绿步 | dns 模块 **17P/0F** | `cargo test --bin ezr engine::dns` |
| ezr 全量测试 | **4162P / 0F**（全 15 目标，对账见下） | `cargo test --all-features` |
| ezr clippy | 0 警告（all/pedantic/nursery 全开） | `cargo clippy --all-features --all-targets` |
| ezr fmt | clean | `cargo fmt --check` |
| arch_check | 规则 1–13/15 过；规则 8 过（本轮曾命中 dns.rs 内 `SystemRoot` env 读取 → 收口 config 缝后复跑过）；**规则 14 沿袭未过**（17 行同一集合，零新增） | `bash scripts/arch_check.sh` |
| 行为接管证明 | `client_resolves_fake_host_via_builtin_hosts_interception`：ezr.test（系统解析必败的假域名）经注入 hosts 表命中 → 全链 client 请求成功——证明解析已被内置器接管（若未接管，reqwest 默认 GaiResolver 必败） | dns 模块测试 |

### 对账口径（如存在基线）
- 测试基线：上轮 handoff 记 4009P/0F。本轮全量 **4162P/0F = 4009 + 153**，153 = **17 × 9**
  ——17 个 dns 守卫测试经 `#[path]` 挂载机制进入 9 个目标（bin ezr 1 + hardening 测试目标
  8：engine_backfill/g_appcore/g_engine/g_supervisor/g_throttle_err/g_v147/ui_backfill/
  v115；per-target 挂载计数已逐一实测登记）。v139 口径 4009 中同机制已含既有单元测试的
  挂载副本，本轮新增沿同机制，口径闭合无漂移。
- bin ezr 分目标：532 → **549**（+17）。
- 规格面对账：features/qa 零改动（FR-01-112 引擎传输层策略无界面形态，v1.33 注记口径；
  FR-01-61 同例）；mission 零改动（coder 只读纪律）。qa/runners 无 DNS 相关断言，影响面
  零触碰。

### 待办与待批（未决项）
- **D40 微决策待操作者裁决**（宽限期 300ms/加 `dns` 配置键等改判，改动面 = 引擎 dns
  模块常量与构建函数各一处）。
- 沿袭待裁决（均见遗留受限项）：arch 规则 14 白名单扩展（cleaner/architect 轮）、
  qa/runners 同步欠账（QA 轮）、APS 机检补跑、v147 flake 预算放宽。

### 移交建议
- 本轮行为切片已闭环（规格 v1.33 + 实现 + 17 守卫测试 + 全量门槛 0F/0 告警/arch 除沿袭
  规则 14 全过）。可径直运行 `six-pack/cleaner`（以本 handoff 为上游交接内容）：
  ①DRY/重复面检视（dns.rs 自愈链臂序与既有失败臂衔接、`LegResult`/`PendingLeg` 类型
  别名化是否扩面）；②arch 规则 14 白名单扩展裁决（`scroll|pos|follow`，17 行存量）。
- 其后按链路 architect → hardender → QA（QA 轮顺带承接 qa/runners 同步欠账与 v147 flake
  预算放宽）。移交仅为建议，不构成自动流转（宪法第一章）。

By coder.
