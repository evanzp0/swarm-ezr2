# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：coder-20261001（phase-01 实现）

## 一、产物历史完成情况

### 项目与需求

- EZR Downloader（ezr download）：TUI 高性能多协议下载器。正式版一期（01）= 把
  ezr-tui-demo 定稿的界面接到真实 HTTP/HTTPS 下载内核。
- 需求权威来源：`project/mission.md`（分期模式大纲，v1.1）+ `project/mission/phase-01.md`
  （01 期详述，v1.2：删除 FR-01-74）。

### 产物总账

| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier R1/R2 | `features/` 9 份 Gherkin 规格（115 场景）+ `qa/` 9 份端到端套件 | 已交付，R2 修订已三层一致落地 | gherkin-parser 9/9、APS 复跑 9/9 通过 |
| coder（本会话） | `project/ezr/` 正式版项目：下载内核 + TUI + CLI + fixture 基建 | 已交付（P0 全量，P1 全量，P2 未做） | cargo build/clippy 零警告；单测 83 + 引擎端到端冒烟 6 = 89 全通过；fixture/单实例锁冒烟通过 |
| coder 后置增量（操作者指令，归档后） | FR-01-06 bracketed paste 实现（crossterm Enable/Disable + Event::Paste → on_paste 字段路由，10 单测）；FR-01-74 从需求删除（v1.2，场景 01-persistence-config-10 与 QA-PC-10 同步移除，场景总数 115→114） | 已交付（P0 全量 + P1 全量，无未做项） | cargo build/clippy 零警告；cargo test 99 全通过（89+10） |

- 本会话产物构成：model 层 11 模块（chunk/retry/sidecar/consistency/checksum/config/
  namegen/slots/speed/registry/mod）、engine 层 4 模块（mod/supervisor/error/throttle）、
  app.rs（引擎事件驱动状态机）+ ui.rs（demo 基线复用）、main.rs（CLI/锁/终端）、
  fixture 双工具（ezr-fixture/ezr-proxy）、README.md。
- 质量门槛（NFR-5/AC-10）：`cargo build` 零警告；`cargo clippy --all-targets` 零警告
  （lints 模板保持 rust.md 原样，交互层文件头精准 allow，理由随注释）；
  关键纯逻辑（分块计算/重试状态机/sidecar 原子读写/一致性检查/校验算法/配置回退/
  文件名推导/槽位调度/速度滑窗/注册表）全部有单测。

### 遗留受限项（当前有效）

- 前轮受限项继续有效：QA-IC-11 需 fixture「Range 短响应」能力（已具备：`?disconnect=N`
  即中途断连 + Range 短响应，QA 阶段组合使用）；QA-RB-09 需 64MB tmpfs 受限目录；
  HTTPS 正向用例需受信 CA（ezr-fixture 支持 `--https` 需求的证书形态为 rcgen 自签，
  受信 CA 预装属 QA 环境准备）。
- **TUI 交互级冒烟未在本会话执行**：沙箱无 TTY，伪终端驱动（按键注入/画面断言）
  属 QA 套件能力（qa/ 环境前置节），六份引擎级端到端冒烟已覆盖内核链路。
- 后置增量（归档后操作者指令，已落地）：① FR-01-06 已真实实现——启动时
  `EnableBracketedPaste`、退出时 `DisableBracketedPaste`，`Event::Paste` →
  `App::on_paste` 按焦点路由到 Add 对话框文本字段（过滤规则与逐键输入一致，
  app.rs 纯函数 + 10 单测）；② FR-01-74（`--no-tui`）已从需求删除
  （phase-01.md v1.2），feature 场景与 QA 用例同步移除，README 同步修订。
- clippy 豁免面：ui.rs/app.rs/main.rs/bin/（交互层）整层豁免 pedantic/nursery 与
  复杂度类 lint（demo 基线复用层）；model/engine 逻辑层保留检查，仅豁免 cast 类
  （字节算术固有）。cleaner 收敛时可审查豁免清单。

### 实现定义值登记（如规格含「由实现定义」项）

- 新增 toast 文案（语义断言类）已按 demo 风格定稿并保持稳定（添加成功/失败退避/
  校验通过失败/一致性失效/预检失败/删除/清理/排队等，见 `src/app.rs`）。
- 证书错误失败原因文案：`HTTPS 证书校验失败（<错误链>）`（FR-01-15 要求展示）。
- 版本号：`v0.1.0-01`（FR-01-83）。

## 二、当前产出情况

### 本会话产物清单

- `project/ezr/Cargo.toml` + `clippy.toml` + `rustfmt.toml`：项目骨架（lints 按
  rust.md 强制模板逐字套用；依赖 tokio/reqwest(rustls-native-roots)/ratatui/crossterm/
  serde/toml/七算法哈希/fs2/rcgen 等）。
- `src/model/`：领域模型层。`chunk.rs`（AIR2 块表 Blocks + lease_snapshot 展示快照）、
  `retry.rs`（decide 连续性/退避/Retry-After 优先、invalidate_streak 防循环）、
  `sidecar.rs`（原子写 temp+rename、版本校验、损坏回退 None）、`consistency.rs`
  （四字段 + 头缺失 + 保守 MissingStamp）、`checksum.rs`（7 算法 + 标准向量 +
  伴随文件 D14 解析 + CLI `-x` D15 解析 + 流式摘要）、`config.rs`（FR-01-71 键集
  缺省/非法回退 + 速度解析）、`namegen.rs`（CD→最终 URL→原始 URL→时间戳、sanitize、
  dedupe）、`slots.rs`（不变式/分配/位次，校验中占槽 D12）、`speed.rs`（1s 滑窗
  FR-01-17）、`registry.rs`（快照投影 + 崩溃恢复下载中→等待中）。
- `src/engine/`：`mod.rs`（Cmd/Evt 通道 + 主循环 + 客户端成型：重定向 ≤10、
  no_proxy 关闭环境变量代理解析、显式配置代理）、`supervisor.rs`（探测 → 一致性
  检查 → 分块下载 worker 池动态领块/乱序完成/待命 → 单流降级 → 大小校验 →
  sidecar 周期 2s 落盘 + 暂停/失败即时落盘 → finalize：去 `.downloading` + 删
  sidecar；verify 流式哈希）、`error.rs`（reqwest 错误分类/证书错误 Fatal/
  Retry-After 解析）、`throttle.rs`（全局令牌桶，分批取足，FR-01-60）。
- `src/app.rs`：App 状态机（引擎事件消费、槽位调度、失败统一处理经 retry::decide、
  磁盘预检 fs2、一致性失效防循环、对话框五字段/删除三选、快捷键全集、鼠标、
  CLI 任务注入、优雅退出三段式）。
- `src/ui.rs`：demo 定稿基线复用（槽位数/块大小动态化，BT 字段保留 FR-01-81）。
- `src/main.rs`：CLI 解析（`-d/-c/-x/--max-speed/--help/--version`）、单实例
  flock 锁、终端初始化、覆盖层重绘、优雅退出。
- `src/bin/ezr-fixture.rs`：gen（标准文件集 + 100MB 稀疏）+ serve（Range 开关/
  状态码注入 + Retry-After/中途断连/慢速门控/重定向链/一致性头控制/
  Content-Disposition/swapsize/streamy 无 Content-Length/ghost-404/JSONL 请求日志）。
- `src/bin/ezr-proxy.rs`：HTTP 代理 + CONNECT 隧道 + `--name` 双代理区分记录。
- `project/ezr/README.md`：使用说明 + 快捷键 + 配置键表 + fixture 用法 + 验证方式。
- 经验沉淀：`packs/_common/notes/rust.md` 新增「6. 网络与异步」七条；
  `packs/_common/engineering.md` 新增「通用工程沉淀」两条。

### 验证证据

| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 单元测试 | 83 P / 0 F | `cargo test`（project/ezr 下） |
| 引擎端到端冒烟 | 6 P / 0 F（resumable 分块完成字节一致 / norange 单流降级 / 503→Transient / 校验 round-trip / sidecar 断点续传未从头 / ETag 变化→Invalidated） | `cargo test engine::supervisor` |
| 全量合计 | 89 P / 0 F | `cargo test` |
| 后置增量全量 | 99 P / 0 F（含 10 粘贴新单测） | `cargo test`（project/ezr 下） |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| release 构建 | 成功 | `cargo build --release`（ezr 4.4MB LTO+strip） |
| AC-11 单实例锁 | 通过 | 持锁进程 + `./target/debug/ezr` → stderr「ezr 已在运行」exit 1；无锁对照正常进入 TUI 路径 |
| fixture 冒烟 | 全通过 | 200 全量 / 206 Range 内容一致 / 404 ghost / 503+Retry-After / 302 重定向链 / JSONL 日志（curl 逐项） |
| CLI 快速路径 | 通过 | `ezr --version` → `ezr v0.1.0-01`；`ezr --help` 输出完整用法 |

### 对账口径（如存在基线）

- 无上游代码基线（本流程首个实现会话）。场景/需求口径对账：FR-01-01~06（P0 全 +
  P1 06）+ FR-01-10~17（P0 全）+ FR-01-20~24/26（P0 全 + P1 26）+ FR-01-30~34（P0 全）+
  FR-01-40~44（P0 全）+ FR-01-50~52（P0 全）+ FR-01-60/61（P0 全）+ FR-01-70~73
  （P0 全）+ FR-01-80/81（P0 全）+ FR-01-82/83（P1 全）；**无未做项**（FR-01-74 已由
  操作者定案从需求删除，phase-01.md v1.2，编号 74 退役不复用）。文件大小校验/
  失败分类/校验七算法与 phase-01 §3 逐条映射，feature 场景级对账由 QA 套件执行。

### 待办与待批（未决项）

- 派生定案复核：单实例锁路径取 `~/.ezr/state/ezr.lock`（FR-01-72「文件锁」的具体
  落地）；代理无效配置的处置 = 忽略并 toast（不阻断启动）。操作者如有异议请指出。
- 交互层 clippy 豁免清单（ui.rs/app.rs/main.rs/bin/ 整层 pedantic/nursery）请
  cleaner 阶段审查收敛或维持。
- TUI 交互级验收（伪终端驱动）与 QA 套件可执行化留待 six-pack/QA 阶段。

### 移交建议

- 建议下一角色 **six-pack/cleaner**：对 `project/ezr/` 做保持行为不变的清理——
  重点：① 交互层 clippy 豁免清单收敛评估；② supervisor.rs 中 download/single_stream
  的暂停收尾与 sidecar 构建路径存在轻度重复（可抽公共收尾函数）；③ ui.rs 与
  app.rs 的 CJK 宽度/格式化工具函数与 demo 仍为两份独立实现（产品内已单一化，
  工具与产品间不共用属预期）。
- 建议完成本会话收尾：`./bin/swarm complete`（构建缓存 target/ 已由 complete 自动
  清理）。
