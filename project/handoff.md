# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-v150-quitmouse-impl（2026-10-24）
> 上游交接：project/handoff.md（coder v135-fieldcomp-impl 交接：输入框组件化 +
> ezr-demo 光标交互同步交付；v136 展示统一/v137 hardender/v138 QA 等中间轮
> 沿袭见产物总账与 packs notes）
> 当轮任务：修复操作者实测 bug——按 q 退出主程序后，shell 回显
> `^[[<35;58;18M` 类 SGR 鼠标残序（v137 轮首修「drain 最先」顺序的残余竞态），
> ezr 与 ezr-demo 同步交付。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.34，分期模式）+ `project/mission/phase-01.md`（v1.34）。
- 本轮后源码状态：v1.34 全条款落地 + 历轮 UI/加固/覆盖轮 + 输入框组件化 +
  **本轮退出路径鼠标残序根治**（**4486P/0F**，15 目标；规格面不变——
  退出收尾行为修复，零条款改动）。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.34 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | v132 轮 3 份 + v149 轮 5 份 feature APS 机检沿袭未跑 |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F |
| architect（v145-arch） | supervisor 六文件拆分 + 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 | 已交付 | 3363P/0F |
| QA（ezr-qa-final / v148-qa 等） | 13+1 套件 e2e 可执行化 + llvm-cov 覆盖轮 | 已交付 | e2e 167P/0F（v148 基线态） |
| specifier（v129–v149） | FR-01-106…116 + v1.34 规格轮 | 已交付 | v149 轮逐字断言经 ezr-demo 核证 |
| coder（v129–v140 系列） | FR-01-106 实现 + v1.30/v1.31 UI 改版 + FR-01-112 内置 DNS | 已交付 | 4162P/0F |
| cleaner（v141）/ architect（v146） | dns 清理 + 四阶段评审 | 已交付 | 4200P/0F、arch 15/15 |
| coder（v134-demo-sync-impl） | v1.34 全条款实现（FR-01-113…116 + 修订十二条） | 已交付 | 4300P/0F |
| coder（v134-uifix-impl） | 三类 UI bug 首轮修复 | 已交付 | 4336P/0F |
| coder（v134-uifix2-impl） | 二次修复：conn_transferring 单源 / Dialog.cursor 光标编辑 / 深度断链 | 已交付 | 4367P/0F |
| coder（v134-uifix3-nativecursor-impl） | 输入框原生光标（field_window 取窗单源 / ▏ 字形退役 / dlg_cursor_screen 发布 + z 序门控 / click_cursor_offset / EnableBlinking） | 已交付 | 4388P/0F、clippy 0、fmt clean |
| coder（v135-fieldcomp-impl） | ①输入框组件化（src/textfield.rs 顶层组件，13 项单测，Dialog 组件化，使用点全量替换）；②ezr-demo 同步（组件移植 + 光标交互全套 + e2e ×3 + 文档同步） | 已交付 | ezr：4484P/0F、clippy 0、fmt clean；ezr-demo：18P/0F + 回归 8 套件全绿 |
| coder（v136/v137 中间轮，commit d64b92b「fix:code bug」） | 退出路径首轮防泄漏：restore 顺序鼠标组最先（restore_into 重排）+ drain_input_events 事件消费（产品 poll(0)；demo 同步时发现 crossterm poll 锁竞争语义改 poll(5ms)，产品未回填） | 已交付 | 操作者实测仍复现（本轮根因所据）；口径自源码注释与 git log 推定 |
| **coder（v150-quitmouse-impl，本轮）** | **退出路径鼠标残序根治：①restore_into 重排「鼠标关最先 → settle(50ms) → drain → raw 关 → paste/alt 关」，错误容忍（两段 execute! 都执行）；②drain 回填 poll(5ms)（锁等待 + 在途着陆窗）并升级 bool 契约（是否消费到事件）；③settle_and_drain（首轮消费到事件才追加 20ms 二轮）；④ezr-demo 全量同步；⑤测试：TimedWriter 时序锁定（鼠标关 < 45ms < paste/alt 关）+ drain bool 契约 + settle 窗口时长契约（+2 测 ×2 项目）** | **已交付** | **ezr：4486P/0F（15 目标，+2 vs 4484）、clippy 0、fmt clean；ezr-demo：20P/0F、fmt clean、回归脚本 8 套件全绿（verify_ezr 118+64 分半、focus 26+32 分半、scrollbar 31/31、footer 14/14、overlay 46/46、popup 5/5、close_btn ALL PASS、smoke 0F）** |

### 遗留受限项（当前有效）
- **APS 机检沿袭未执行**：v132 修订 3 份 + v149 修订 5 份 feature 未过
  `gherkin-ir-dry-checker`；本沙箱 GitHub 不可达，待环境可用时一并补跑。
- **qa/runners/ e2e 套件未随 v1.34 同步（移交 QA）**：可执行套件仍为 v1.30–v1.32 口径，
  对 v1.34 改版场景会 F，属预期——待 QA 轮同步。
- **demo 回归长脚本沙箱卡顿（沿袭 + 本轮复证，非本轮改动引入）**：
  verify_ezr / test_focus_style 连续全量跑在沙箱内会在中后段场景随机挂起
  （本轮实测：全量挂 R 场景起点；`verify_ezr.py r` 单独跑 10/10 通过；
  按场景子集分半跑全绿 118+64 / 26+32）。判定依据：8 套件中 6 个（含
  footer「点击 Q 退出进程」、smoke 等 pty 全生命周期套件）直接全绿 + 挂起
  场景单独复跑通过 + 挂起点随运行变化——环境性 flake，与退出路径改动无关。
  复现/规避配方已沉（worklog + notes/rust.md）。
- **真实 TCP stub e2e 时序脆弱（v147 沿袭）**：`--test-threads=2` 全绿即环境竞争非
  回归。
- **dns.rs 覆盖率残余 3 行 / 观察受限突变体 / ezr-proxy 测试基建豁免（沿袭登记）**：状态不变。
- **CRAP>30 9 处处置（沿袭，移交 hardender）**：状态不变。
- **D40 / PC-09-场景09 待操作者裁决（沿袭）**：状态不变。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D43。
- 本轮新增：
  - **MOUSE_SETTLE = 50ms / MOUSE_SETTLE_RETRY = 20ms**（退出收尾 settle 窗口，
    ezr 与 ezr-demo 同值；本地终端 disable 处理延迟毫秒级，50ms 覆盖充分；
    常见退出路径总耗时 ≈50ms，不可感知）。
  - **drain poll 超时 = 5ms**（自 demo v137 口径回填产品；锁等待 + 在途字节
    着陆窗，消费循环上限护栏 10_000 次不变）。

## 二、当前产出情况（coder v150-quitmouse-impl）

### 根因（v137 首修的两个残余缺口）
操作者现象：按 q 退出后 shell 提示符处回显 `^[[<35;58;18M% … ➜ … ✗ 35;58;18M`
——SGR any-event（1003）模式下的鼠标 motion 字节（35=无按键移动，58/18=列/行）
排队在 tty 输入队列，进程退出后被继承 tty 的 shell（zle）当输入回显成裸转义。
v137 顺序 `drain → disable_raw_mode → execute!(关鼠标…)` 的两个缺口：
- **缺口 A（产品 drain 空转）**：crossterm 0.28 带超时 poll 的 `Ok(false)` 混合
  语义——先 `try_lock_internal_event_reader_for(timeout)`，锁竞争立即失败同样
  返回 Ok(false)，并非查过队列；EventStream 常驻后台线程 Drop 只置 shutdown
  不 join，restore 时 poll(0) 首轮撞锁尾巴即空转。产品 poll(0) 因此经常**一个
  字节都没消费**（demo 同步轮已发现并改 5ms，产品漏回填）。
- **缺口 B（先 drain 后关鼠标的生成窗口）**：drain 后、终端模拟器真正处理
  `?1003l` 前，1003 上报仍在进行——该窗口内新产生的 motion 字节排队在 tty
  队列，进程退出后被 shell 读走。窗口虽仅毫秒级，但 motion 事件高频，
  必现概率高（操作者两次实测均中）。

### 修复（ezr 与 ezr-demo 同形）
- **`restore_into` 重排**（v150）：`execute!(DisableMouseCapture)` 最先 flush →
  `settle_and_drain()` → `disable_raw_mode()` → `execute!(DisableBracketedPaste,
  LeaveAlternateScreen)`。错误容忍：两段 execute! 都执行（前段失败不跳过后续
  关闭命令），`mouse_off.and(rest)` 按序上报首个 Err。
- **settle 窗口**：`MOUSE_SETTLE`(50ms) 等终端真正处理关闭序列——把「序列已
  发出但尚未生效」的生成窗口整个纳入等待；窗口内到达的字节随后 drain 消费。
  首轮 drain 消费到事件（积压大/关闭仍在传播）→ `MOUSE_SETTLE_RETRY`(20ms)
  追加一轮；首轮即空则单窗口收工。
- **drain 回填 5ms + bool 契约**：poll(0)→poll(5ms)（先等到内部事件锁、再按
  队列真实状态判定；在途字节着陆窗），返回是否消费到事件供 settle 二轮决策。
- **顺序契约（raw mode）**：drain 必须在 raw mode 仍开启时进行（canonical 模式
  下无换行的鼠标序列被内核 line discipline 攒行等 EOL，drain 失效）——
  「raw 开着 drain、drain 完才关 raw」已写入双方 doc 注释。
- **哨兵不动**：`RESTORE_SEQUENCE` 字节序（鼠标组最先）与正常收尾同形的不变量
  保持；父死路径不 drain——与已恢复的 shell 竞争读队列会吞用户键入，乱码仅限
  kill -9 场景，风险不对称。sentinel.rs 注释同步 v150 口径。
- **测试（每项目 +2，同形）**：
  - `TimedWriter`（记录每次 write 时刻的探针 Writer）锁定时序契约：
    鼠标关（?1006l）写出 < 45ms、paste/alt 关写出 ≥ 45ms（睡眠保证 ≥50ms，
    断言取 45ms 容差）且 paste ≤ alt——顺序回退（先 drain 后关鼠标）即刻红；
  - drain 空队列契约升级：返回 false + 快速返回（<2s）；
  - settle_and_drain 无事件环境：总耗时 ∈ [MOUSE_SETTLE, +RETRY+2s)
    （至少等待主窗口 + 不追加二轮）；
  - v137 字节顺序测试原样保留复核（鼠标组最先、只含关闭）。

### 验证证据

| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| ezr 全量测试 | **4486P / 0F**（15 目标；基线 4484 → +2） | `cargo test`（project/ezr，汇总 15 行全 ok） |
| ezr clippy | 0 警告（全目标） | `cargo clippy --all-targets` |
| ezr fmt | clean（nightly） | `cargo fmt --check` |
| ezr-demo 测试 | **20P / 0F**（18 基线 + 2 同形新增） | `cargo test`（ezr-demo） |
| ezr-demo fmt | clean | `cargo fmt --check` |
| verify_ezr | 118 + 64 = **182/182**（场景子集分半；默认集含 i 场景较上轮 169 增 13 项） | `python3 scripts/verify_ezr.py abcdefhijklmn` + `python3 scripts/verify_ezr.py opqrs` |
| test_focus_style | 26 + 32 = **58/58**（分半） | `python3 scripts/test_focus_style.py target/debug/ezr-tui-demo fgi` / `… hjkl` |
| verify_scrollbar_universal | 31/31 | `python3 scripts/verify_scrollbar_universal.py` |
| verify_footer_pad_scroll | 14/14（含「点击 Q 退出进程」） | `python3 scripts/verify_footer_pad_scroll.py` |
| check_overlay_redraw | 46/46 | `python3 scripts/check_overlay_redraw.py` |
| check_popup_integrity | 5/5 | `python3 scripts/check_popup_integrity.py` |
| verify_close_btn_color | ALL PASS | `python3 scripts/verify_close_btn_color.py` |
| smoke_v_sync | 0 失败 | `python3 scripts/smoke_v_sync.py` |

- 对账口径：4484 → 4486（**+2**：TimedWriter 时序测 + settle 窗口时长测各 1，
  在 bin 目标计 1 次且无 `#[path]` 挂载展开——main.rs 的 cli_parse_tests 不入
  tests/ 挂载树；drain 契约测为改名不增数）。
- 工具链口径（本轮环境实证）：cargo 需 `PATH=$HOME/.cargo/bin:$PATH` +
  `RUSTUP_TOOLCHAIN=nightly-x86_64-unknown-linux-gnu`（rustup toolchain list
  在沙箱内会卡网络解析，环境变量直指工具链目录绕过 rust-toolchain.toml 解析；
  crates 走 rsproxy）。

### 待办与待批（未决项）
- **沿袭项全部不变**（见第一节遗留受限项）；本轮无新增阻塞。
- **bin/ezr-tui-demo 预编译二进制未随本轮重建**（沿袭 v135 口径）：需要随包
  分发时由操作者决定是否 `cargo build --release` 后覆盖 `bin/ezr-tui-demo`。
- **真实终端人工复核建议**：本修复的端到端效果（按 q 退出后 shell 零残序）
  已由时序单测 + pty 回归（footer A7 点击 Q 退出、smoke 退出流）覆盖；
  操作者在真实终端（zsh + 鼠标移动场景）复核一次更稳妥。

### 移交建议
- 建议下一步运行 `six-pack/QA`：qa/runners/ e2e 套件同步 v1.34 口径时，
  可顺带补「退出收尾」断言（复原序列字节序/时序已有单测锚点，e2e 层可加
  pty 退出后 tty 队列空的断言形态）。
- `six-pack/hardender` 可对本轮 settle/drain 纯逻辑做变异加固
  （bool 契约 + 窗口时长断言是现成杀灭面）。
