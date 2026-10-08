# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：coder-20261008-detail-panel-alignment
> 版本注：实现 v1.14（FR-01-100/101）——任务详情面板对齐 ezr-demo 源码更新
> （提交 5492c7c，修订-3/5/6/7）：类型行去并发数；「校验」「URL」字段名淡蓝下划线
> 链接样式 + 点击复制剪贴板（arboard 优先 / OSC 52 回退）+ toast 反馈。
> specifier v1.14 交接内容已消化；D26/FR-01-98 裁决落账已确认（无需代码改动）。

## 速览（先读这一页）

- **项目**：EZR Downloader——终端里的多协议下载工具（Rust / TUI）。需求权威来源
  `project/mission.md`（v1.14）；分期详述 `project/mission/phase-01.md`（v1.14）。
- **本轮（coder）做了什么**：以 TDD 实现 FR-01-100 与 FR-01-101 全部行为切片——
  类型行收敛为「协议名 · 断点续传说明」（FR-01-100）；「校验」「URL」字段名链接样式
  （淡蓝 RGB(122,185,242) + 下划线仅覆盖文字）、热区每帧回填与三处清空门（无校验行 /
  无选中、G 收起、窄终端）、点击复制（URL = 详情展示值 D27 / 校验 = 完整校验码值）、
  toast 文案照录 demo 原文（「已复制 url」「已复制 校验码」）、剪贴板机制
  （arboard 懒初始化复用，无图形会话/失败回退 OSC 52，手写 b64 零新外部编码依赖）。
  新增 7 条测试（渲染断言 4 + 点击守卫 2 + OSC52 序列 1）。
- **质量终态**：全目标 **1752P/0F**（11 目标；对账 = 1729 基线 + 7 新测试 × 挂载
  （bin 本体 7 + g_appcore/g_engine/g_supervisor 各 +3 挂载副本（mouse 2 + clipboard 1）
  + v115 全量挂载 7）= +23）；clippy 0；fmt 0；arch 11/11。
- **移交**：建议 `bin/swarm begin six-pack/cleaner`（CRAP/DRY/清理），再 architect →
  hardender → QA（QA-UC-16…18 套件脚本化属 QA 轮职责）。

## 术语速查

| 词 | 意思 |
|---|---|
| P / F / S | 用例通过 / 失败 / 受限跳过 |
| 修订-3/5/6/7 | ezr-demo 源码（提交 5492c7c）修订编号：3=类型行去并发数、5=URL/校验点击复制、6=下划线仅覆盖文字、7=arboard+OSC52 剪贴板 |
| 热区（hot rect） | 详情字段名可点击区域，UI 层每帧回填到 App 字段，鼠标点击命中判定依据 |
| OSC 52 | 终端剪贴板转义序列（ESC ]52;c;<base64> BEL），无图形会话的复制回退通道 |
| 挂载树合法副本 | `#[path]` 测试 crate 重复注册 cfg(test) 测试（notes/rust.md 条款） |
| 落账 | 把操作者裁决结果以注记形式写入 mission 条款，关闭对应待批项 |

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（`ezr download`）：高性能多协议 TUI 下载器。phase-01 已交付；v1.13
  （第十批指令）UI 对齐 ezr-demo 定稿已交付；本轮 v1.14（操作者第十一批指令）详情面板
  增量批次（specifier + coder 两棒完成，实现交付）。
- 需求权威来源：`project/mission.md`（v1.14）；分期详述 `mission/phase-01.md`（v1.14，
  FR-01-94…101 + D26/D27）；功能规格 `project/features/`（13 份）；QA 规程 `project/qa/`
  （13 份规程 / 12 份套件脚本 + 共用 harness）。
- **外部参照**：`ezr-demo/docs/UI交互需求文档-定稿.md` v1.4 与 `ezr-demo/src/`
  （v1.14 权威 = ezr-demo 源码提交 5492c7c 修订-3/5/6/7）。

### 产物总账
| 阶段 | 做了什么 | 验证状态 |
|---|---|---|
| 基线交付 + 历轮加固 + 操作者第 7–9 批指令（至 v1.12） | 需求文档 + 下载内核 + TUI + 加固 + 度量收敛 | 已交付 |
| coder v116 / QA 终局（six-pack 收官轮） | 两缺陷修复；12 套件端到端 + 3 处缺陷 TDD 修复 | 1656P/0F；e2e 132P/0F/9S；clippy 0；arch 11/11 |
| specifier v1.13 | mission/phase-01 v1.13 增补 + features/04（15 场景）+ qa/04 套件规格 | APS parser + dry-check 0 错误 |
| coder v1.13 | FR-01-94…99 全量实现（TDD）+ 17 条新测试 + 01-tui-display 规格/QA 同步修订 | 1729P/0F；clippy 0；fmt 0；arch 11/11 |
| specifier v1.14 | phase-01 v1.14（FR-01-100/101 + D27 + D26/FR-01-98 裁决落账）+ features/04 场景 16–18 + qa/04 QA-UC-16…18 | APS parser + dry-check 0 错误级 |
| **coder v1.14（本轮）** | FR-01-100/101 全量实现（TDD）+ 7 条新测试 + arboard 依赖接入 | **1752P/0F；clippy 0；fmt 0；arch 11/11** |

### 遗留受限项（当前有效）
1. **ezr-proxy 三类监听扩展未做**（https / socks5+账号密码 / IPv6 回环）：QA-NP 部分
   S 相位沿旧口径登记（phase-02/后续承接）。
2. **e2e 预登记跳过 4 例**：RB-09（64MB 受限目录）、TP-02/03/06（测量方法学）。
3. **变异补跑待 hardender 轮**：v116 + QA 轮 3 处修复 + v1.13/v1.14 UI 层新代码均未入
   变异集。
4. **沙箱无 TTY**：TUI 进程级端到端无法自动化；PTY 端到端由 QA 会话以 pyte 完成
   （QA-UC 套件脚本化属 QA 轮职责，尚未实现 runner）。
5. **环境（2026-10-08 重建）**：rustup nightly 1.101.0-nightly（`~/.cargo`，含
   rustfmt/clippy）；Babashka 1.13.225（`.tools/bin/bb`）；APS 仓库
   （`.tools/Acceptance-Pipeline-Specification/`）。`export PATH="$HOME/.cargo/bin:$PATH"`
   后验证命令可复现。PMD（CPD）未装（cleaner/architect 轮按需安装）。
6. **Ctrl+B 的 BT 断开实际执行**：phase-02 承接（D26；01 期守卫链 + HTTP 拒绝臂已实现，
   BT 臂为不可达注释锚点）。
7. **沙箱无图形会话**：arboard 系统剪贴板路径在沙箱不可运行时验证（Clipboard::new()
   失败 → 测试与 QA 实际走 OSC 52 回退臂）；系统剪贴板写入成功路径由有图形会话的
   人工冒烟覆盖（QA-UC-18 的 S 登记规则已写入 qa/04 环境前置）。
8. **thread_count 产品面暂无调用**（FR-01-100 删除唯一调用点）：#[allow(dead_code)]
   注记保留（数据面语义测试有效；phase-02 BT 预期复用）。

### 实现定义值登记（累计，当前有效全量）
1. **单实例锁回退扩展**（main.rs）：state 目录不可写时回退系统临时目录。
2. **D19 完成校验用最新值**（app/engine.rs）：完成判定以任务当前 checksum 为准；
   引擎快照跳过的收尾由 App 幂等补齐。
3. **恢复路径提示顺序**（app/tasks.rs）：恢复提示先落、缺失引用提醒后发覆盖。
4. **D26 并发明细 toast 文案**：照录 demo 定稿原文（含「仅下载中/做种中」措辞）。
   **v1.14 裁决维持**。
5. **连接级累计/速度口径（FR-01-99，v1.13 实现）**：
   - 计量位置：App 消费侧 `update_conn_stats`（app/engine.rs），键 `(任务 id, 连接 id)`，
     观测三元组 (块号, 块内已写字节, 时刻)；不回写引擎、不参与会话已下载统计。
   - 增量：同块 = done 差值；换块/首次观测 = 新块内已写字节。
   - 速度：增量/时间差经 `SmoothedSpeed`（EMA α=1/5）平滑；待命（cap=0 或块满）
     `zero()` 零值速断显示 `-`；在传连接瞬时零增量不归零（防闪烁）。
   - 清零触发：Probed 事件 Queued→Downloading（首次启动/重试重排/失效重下）与
     tasks.rs 不可续传继续（从头下载）；任务删除/清理/完成收尾同步清账。
   - 保留：暂停后可续传继续、换块领新块（暂停期间账本冻结隐藏）。
6. **并发明细选择序号口径**：conns_sel 为「连接 id 升序」展示序（与面板渲染同序），
   非引擎 conns vec 的租约序。
7. **流量图采样**：数据源 `speed_hist`（每秒 ≤1 点，上限 180）；绘制取最近
   2×(列数+1) 样本、双向 EMA α=0.75、min–max 缩放（边距下限 = 窗口峰值 5%）、
   每列双子样本取大；图宽 = 内容区 20%（clamp 6..内容区-95，<6 不渲染）。
8. **D27 URL 热区复制内容（本轮实现）**：复制详情展示值（`final_url` 优先回退
   `url`，FR-01-14 口径）；「校验」热区复制完整校验码值（非 10 位截断展示值）。
9. **复制反馈账本（本轮新增）**：`App::last_copied` 记录最近一次复制内容
   （诊断与测试断言用；`copy_to_clipboard` 每次写入）。
10. **剪贴板实例生命周期（本轮新增）**：arboard `Clipboard` 懒初始化并常驻复用
    （首次点击建连后零开销）；写入失败置 None 下次点击重建、当次即回退 OSC 52；
    OSC 52 写 stdout 失败静默忽略（alternate screen 下终端仍解析并代写）。
11. **热区几何（本轮新增）**：字段名整段 9 列（x = 详情内框左 +1、y = 内框顶 + 行号、
    宽 9、高 1；与 label 对齐块一致便于点击）；行号超出内框高度不设（`hot_rect`）。

## 二、当前产出情况（coder-20261008-detail-panel-alignment）

### 本会话产物清单

**产品实现（FR-01-100/101 全量）**
- `Cargo.toml`：新增 `arboard = { version = "3", default-features = false,
  features = ["wayland-data-control"] }`（demo 同款；b64 为手写实现，零外部编码依赖）。
- `src/app/clipboard.rs`（新增）：`copy_to_clipboard`（arboard 优先 → 失败置 None 回退
  OSC 52；同时记 `last_copied`）+ `osc52_sequence`（纯函数）+ `b64_encode`
  （手写标准字母表 + '=' 填充）。
- `src/app/mod.rs`：App 新增 `detail_url_rect`/`detail_ck_rect`（Option<Rect>，ui 层每帧
  回填）、`clipboard`（私有，懒初始化）、`last_copied`（pub(crate)）；mod 声明与初始化。
- `src/ui/detail.rs`：类型行删除「 · N 并发 · 」段（FR-01-100，收敛为
  `协议名 · 支持断点续传/不支持断点续传`）；`link_label`（淡蓝 + 下划线仅覆盖文字，
  demo 修订-5/6）应用于「校验」「URL」行；签名 `&App` → `&mut App`；无选中任务分支
  清空热区；渲染后 `hot_rect` 回填两热区（校验行不存在 → ck 热区 None）。
- `src/ui/mod.rs`：G 收起分支与窄终端分支各加 `detail_url_rect`/`detail_ck_rect` 清空
  （FR-01-101 热区清空门）。
- `src/app/mouse.rs`：左键点击臂（对话框分支之后、列表选中之前）——URL 热区命中 →
  复制展示值 + toast「已复制 url」；校验热区命中 → 复制完整校验码 + toast
  「已复制 校验码」（D27/D26）；热区 None 点击无动作。
- `src/model/task.rs`：`thread_count` 加 `#[allow(dead_code)]` 注记（FR-01-100 后产品面
  暂无调用，语义测试保留，phase-02 复用）。

**测试（7 条新增，全部锁定行为）**
- `src/ui/mod.rs` ui_v114_tests（4）：类型行无并发（下载中 + 活跃连接，含去空格行断言
  与 to_string 定稿形态断言）；链接样式逐单元格断言（校验/URL 淡蓝 + 下划线、续格
  Reset 特性适配、填充空格无下划线、类型标签仍 DIM）+ 热区回填几何；无校验行/无选中
  热区清空；G 收起与窄终端整帧热区清空。
- `src/app/mouse.rs` mouse_tests（2）：点击复制全链（URL → final_url 展示值 + toast、
  校验 → 完整校验码 + toast、无校验任务无热区无动作）；守卫臂（热区 None 无动作、
  对话框打开时点击被对话框分支消费）。
- `src/app/clipboard.rs` clipboard_tests（1）：OSC 52 序列构造（RFC4648 已知向量：
  hello/空串/3 字节整除/2 字节填充/UTF-8 多字节）。

### 验证证据（crate 根 = project/ezr）
| # | 验证项 | 结果 | 复现命令 |
|---|---|---|---|
| 1 | 全量测试（11 目标） | **1752P/0F** | `cargo test` |
| 2 | 逐目标对账 | bin 344 / ezr-fixture 30 / ezr_proxy 11 / hardening 107 / g_appcore 265 / g_engine 263 / g_main 2 / g_proxy 12 / g_supervisor 241 / g_throttle_err 167 / v115 310 | `cargo test` 输出逐行 |
| 3 | 静态检查 / 格式 / 架构 | clippy 0 / fmt 0 / arch 11/11 | `cargo clippy --all-targets` / `cargo fmt --check` / `bash scripts/arch_check.sh` |

### 对账口径
- **1752 对账**：1729（v1.13 基线）+ 23 = 本轮 7 条新测试 × 挂载树合法副本——
  bin +7（ui 4 + mouse 2 + clipboard 1 本体）；g_appcore/g_engine/g_supervisor 各 +3
  （mouse.rs 2 + clipboard.rs 1 挂载副本）；v115 +7（全量挂载）。与 notes/rust.md
  「挂载树合法副本」条款口径一致。
- **行为基线**：FR-01-30/31/32 状态机与槽位、FR-01-17 速度节奏、FR-01-81 数据语义、
  FR-01-94…99 v1.13 定稿布局均未改动（本轮全部为详情面板展示与复制交互增量——
  既有 1729 测试零改动通过；仅详情渲染断言新增，无既有断言改名/删除）。

### 四要素③/④
- 无 feature 注释块变化（04-ui-conns 场景 16–18 已由 specifier v1.14 增补，本轮未触碰
  规格）；无新增等价突变体登记（本轮未跑变异）。

### 待办与待批（未决项）
1. **【移交 cleaner 轮】**CRAP/DRY 度量与清理：本轮触碰 ui/detail.rs、ui/mod.rs、
   app/mouse.rs、app/clipboard.rs（新）、app/mod.rs、model/task.rs；PMD 需按 packaging
   口径安装。
2. **【移交 hardender 轮】**变异补跑基线更新（含 v1.13 + 本轮新代码）。
3. **【移交 QA 轮】**QA-UC-16…18 套件脚本化（pyte PTY 口径 + OSC 52 序列捕获断言，
   参照 qa/04 环境前置）；QA-TD/QA-UC 既有套件旧断言无需改动（本轮未改既有布局）。
4. 无操作者待批项（D27 为技术定型已注记可改判，改动面仅热区点击臂一处）。

### 移交建议
- 建议下一步：`bin/swarm begin six-pack/cleaner`（保持行为清理 + CRAP/DRY），
  随后 architect → hardender → QA 走完 six-pack 链路；操作者如只要实现交付，直接
  `bin/swarm complete` 归档即可。
- 手工冒烟入口（沙箱外有 TTY 环境）：`cargo run --release`（project/ezr），
  选中提供校验码的任务：类型行无并发数；「校验/URL」淡蓝下划线；点击 → 剪贴板 +
  底部 toast（SSH 会话下经 OSC 52 由终端代写）。
- 归档前无需手动清理（target/ 由 complete 自动清理；`.tools/` 按 .gitignore 不入包）。

By coder.
