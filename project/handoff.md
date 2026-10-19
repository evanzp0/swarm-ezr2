# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder　会话：ezr-v137-mouse-leak-fix（2026-10-19）
> 上游交接：coder v136-partial-chunk-fix（交接内容 = 前轮产物总账 + 移交建议 cleaner）
> + 操作者 chat 指令（bug 报告：「swarm-20261019-104911-six-pack-coder 运行下载后，
> 按 q 退出，会有显示 `^[[<35;56;26M%   35;56;26M` 类似这样的乱码」+ 基线指令
> 「swarm-20261019-104911-six-pack-coder 的基础上修改」——工作区 = 该包解压）。
> 当轮任务：v137 缺陷修复轮——按 q 退出后 SGR 鼠标事件字节泄漏给 shell（zle 回显乱码）。

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（Rust / TUI 下载器，01 期：HTTP/HTTPS 真实下载内核与 TUI 正式版）。
  需求权威来源 `project/mission.md`（v1.31，分期模式）+ `project/mission/phase-01.md`（v1.31）。
- 本轮任务 = 操作者 bug 报告轮（v137）：修复「按 q 正常退出后 tty 输入缓冲中已排队的
  鼠标事件字节被 shell 回显」——退出竞态窗口治理；无规格文档改动。

### 产物总账（流程开工以来累计，当前有效口径）
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（历轮） | mission v1.1→v1.31 全量规格 + features/ 14 份 + qa/ 14 份 | 全部交付 | v134 起 3 份 UI feature 过 APS 机检（0 错误级） |
| coder（v128-impl 等） | v1.28 URL 权威模型等全量实现 | 已交付 | 2925P/0F（v128 轮口径） |
| cleaner（v128-clean） | supervisor 失败分类/完成收尾单源化 | 已交付 | 3040P/0F；CPD 17→15 |
| architect（v145-arch） | supervisor 六文件拆分（114 点守恒）+ 属性 62 + arch 15 | 已交付 | 3049P/0F、clippy 0、fmt 0、arch 15 |
| hardender（v147-hard） | 163 点差分变异 campaign + 加固套件 hardening_g_v147 | 已交付 | 3363P/0F、clippy 0、fmt 0 |
| QA（ezr-qa-final） | QA 规程可执行化 13 端到端套件（qa/runners/）+ 缺陷修复 | 已交付 | e2e 165P/0F/0S、3363P/0F、覆盖 94.37% |
| specifier（v129/v130/v133/v134） | FR-01-106/107/108/109/110 + D35/D36/D38 规格 + v1.31 对齐 | 已交付 | 反向自检通过 |
| coder（v129–v133） | FR-01-106 实现 + 缺陷修复三件 + v1.30 全部 UI 改版 + D35 收窄 | 已交付 | 3843P/0F（v133 口径） |
| coder（v135-impl-m2） | v1.31 规格面四项落地（页脚双行 chips / 帮助 36 行 / 浮层翻转零清屏 / 满窗起点钳制） | 已交付 | 3867P/0F、clippy 0、fmt 0、arch 规则级守恒 |
| coder（v136-partial-chunk-fix） | 缺陷修复：pieces 🟨 集合权威去 `!downloading` 前提（M 缩容后无租约部分块恢复 🟩 不白屏）+2 回归测试 | 已交付 | 3887P/0F、clippy 0、fmt 0、arch 守恒 |
| **coder（v137-mouse-leak-fix，本轮）** | **缺陷修复：①`restore_into<W: Write>` 可测内核——复原序列重排为 `DisableMouseCapture → DisableBracketedPaste → LeaveAlternateScreen`（鼠标组最先关闭，竞态窗口最小化）；②新增 `drain_input_events()`——raw mode 仍开时以非阻塞 poll/read 清空输入队列，消费按 q 瞬间已排队/在途的鼠标事件字节（进程退出后不再泄漏给 shell）；③哨兵 `RESTORE_SEQUENCE` 镜像新顺序（父死路径与正常收尾同形不变量）+ 镜像断言测试同步；+2 回归测试** | **已交付** | **3889P/0F、clippy 0、fmt clean、arch 规则级守恒（规则 14 沿袭 8 行同集合）、pty 行为级双冒烟 PASS** |

### 遗留受限项（当前有效）
- **arch_check 规则 14（ui 回填白名单）沿袭未过**：8 行同集合（dlg_pos×2/help_pos/
  help_scroll/pieces_pos/pieces_follow/pieces_scroll×2），规则级守恒不变。本轮零触碰
  ui 层、0 新增失败行。**移交建议（不变）**：cleaner/architect 轮将白名单正则扩为
  含 `scroll|pos|follow` 并裁决存量 8 行形态。
- **v147 时序 flake（沿袭）**：`sidecar_not_written_within_first_500ms` / `hardening_g_v147`
  偶发（隔离 ×3 + 重跑绿）；本轮全量跑未复现。建议 QA 轮放宽 `wait_state` 预算或加重试。
- **Rust 工具链沙箱不持久（本轮重建登记）**：nightly 1.98.0-nightly（cced03bfd
  2026-05-28）+ rustfmt/clippy + ld.lld 桥接，配方同前。**本轮新踩**：重装后
  `~/.cargo/config.toml` 缺失（随沙箱清理），cargo 走官方 crates.io 源假卡
  （`transfer too slow ... 0 bytes in 30s`，易误归因镜像抖动）——重装清单必须含
  cargo 镜像配置（已沉淀 rust.md）。
- **观察受限突变体（沿袭 hardender 登记，仍开放）**：workers.rs 5 条 + 等价 download.rs:85:27。
- **ezr-proxy.rs 测试基建（沿袭裁决）**：127 变异点不拆、CRAP/CPD 读数不追责。
- **qa/runners/ 可执行化同步欠账（沿袭 + v1.30/v1.31 增量再扩大）**：QA 轮一并承接。
- **gen 剥头副本守卫覆盖面（本轮核对登记）**：五件守卫面（btn/chart/clipboard/detail/
  update_conn_stats）不含本轮触碰文件（main.rs / sentinel.rs），无需再生成。
- **退出竞态修复的可观测边界（本轮登记）**：drain 消费的是 restore 时刻已到达的输入
  字节；disable 传播延迟窗口（毫秒级）内新产生且在 drain 之后到达的事件仍可能残留——
  已通过「鼠标组最先关闭」把窗口压到最小；pty 泄漏模拟（q + motion 同批入队）实测
  残留 0 字节。极端时序下的毫秒级残留属业界（crossterm/ratatui 模板）共有限界。

### 实现定义值登记（当前有效）
- 沿袭有效：v1.28 全部口径、D26–D37、v138/v140 口径、D35 结案收窄、D38 登记差异
  （维持产品口径，待操作者裁决）。
- **本轮口径注记（非新增实现定义值）**：复原序列顺序 = 鼠标组 → paste → 备用屏
  （v137 重排；crossterm 0.28.1 逐字序列不变，仅组间顺序调整）；哨兵 RESTORE_SEQUENCE
  镜像同序。FR-01-84 哨兵「父死还原 = 正常收尾镜像」设计不变量保持。

## 二、当前产出情况（coder v137-mouse-leak-fix）

### 本会话产物清单
- `project/ezr/src/main.rs`（+66/−9）：
  - `restore_into<W: Write>`：复原序列可测内核（接受任意 Write，测试以 `Vec<u8>`
    断言字节顺序）；顺序重排 `DisableMouseCapture → DisableBracketedPaste →
    LeaveAlternateScreen`（鼠标组最先关闭——disable 传播延迟窗口内不再产生新
    鼠标字节；各命令相互独立无栈约束，crossterm 0.28.1 逐字序列不变）；
  - `drain_input_events()`：退出前在 raw mode 仍开启时以非阻塞 `poll(0)`/`read()`
    循环清空输入队列（消费按 q 瞬间已排队/在途的鼠标事件字节；上限护栏 10_000
    次防异常积压阻塞退出；无 tty 环境报错即视为无事件）；
  - `restore_terminal()` 改为薄封装（`restore_into(&mut io::stdout())`），
    panic hook 路径自动获得同序 + drain 行为；
  - +2 回归测试：`restore_sequence_disables_mouse_before_leave_and_paste`
    （字节序守卫：?1006l/?1000l 先于 ?2004l/?1049l，且只含关闭不含启用）、
    `drain_input_events_returns_promptly_without_events`（非阻塞语义）。
- `project/ezr/src/sentinel.rs`（+12/−11）：
  - `RESTORE_SEQUENCE` 重排镜像新顺序（鼠标五连 `?1006l ?1015l ?1003l ?1002l
    ?1000l` 最先 → `?2004l` → `?1049l`），注释同步「父死还原 = 正常收尾镜像」；
  - 镜像断言测试 `restore_sequence_mirrors_crossterm_0_28_disable_set` 同步新序列。
- `packs/_common/notes/rust.md`（工具侧经验沉淀 2 处，见验证证据节）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量测试 | **3889P / 0F**（15 目标全绿） | `cd project/ezr && cargo test --all-targets --no-fail-fast` |
| 红灯回放 | 临时反转回旧顺序（Mouse 最后）→ 主回归 FAILED；恢复新顺序全绿 | 单测级灵敏度确认 |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| fmt | 0 偏差 | `cargo fmt --check` |
| arch_check | 规则级守恒：15 项 OK，仅规则 14 沿袭 8 行（同集合逐行比对） | `bash scripts/arch_check.sh` |
| **pty 行为冒烟 ①** | 复原序列完整性 + 顺序：退出时 `?1049l/?1006l/?1000l/?2004l` 全在位且 **1006l@606 < 2004l@646 < 1049l@654（鼠标组最先）** | `python3 .work/tmp/smoke_quit_restore.py`（pty fork 启动 ezr → q → 断言序列字节位置） |
| **pty 行为冒烟 ②（泄漏模拟）** | q + 2 个 SGR motion 字节同批入队 → ezr 退出 → `cat -v`（模拟 shell）读到残留 **0 字节**（修复前即用户报告的 `^[[<35;56;26M` 回显） | `python3 .work/tmp/smoke_quit_leak.py`（脚本在 .work/tmp，随会话清理；复现逻辑已沉淀 rust.md） |

### 对账口径（如存在基线）
- 上游 v136 基线 = 3887P/0F。本轮测试计数 **3887 → 3889（+2）**，复算自洽
  = 2 新测试均挂载 bin ezr（512→514）；sentinel 镜像断言为既有测试同步（净 0）；
  main.rs/sentinel.rs 不被其余 14 测试目标挂载（bin 专属模块），计数不变。
- arch 规则 14 失败行集合逐行比对 = 基线 8 行同集合，规则级守恒。

### 待办与待批（未决项）
- **待操作者裁决（沿袭开放）：对话框拖拽起拖面/点击时机**（FR-01-110①/D38）——本轮
  未触及，维持产品口径零改动。
- **qa/runners/ 同步欠账**（沿袭 + v1.30/v1.31 增量）：QA 轮承接。
- **arch 规则 14 白名单扩展建议**（沿袭）：cleaner/architect 裁决。
- **v147 时序 flake**（沿袭）：如复现，QA 轮放宽预算或加重试。
- **请操作者实机复测**：真实终端下运行下载 → 按 q 退出 → 确认鼠标乱码消失
  （pty 冒烟已模拟通过，真实终端模拟器行为以操作者实测为准）。

### 移交建议
- 建议下一步运行 `six-pack/cleaner`（以本 handoff 为上游交接内容）：v137 修复切片
  已全绿，按 cleaner 职责做保持行为清理、覆盖率提升、CRAP/DRY 审查与变异点扫描；
  顺带裁决 arch 规则 14 白名单扩展（scroll|pos|follow）与存量 8 行形态。
- 其后按链路 architect → hardender → QA。移交仅为建议，不构成自动流转（宪法第一章）。

By coder.
