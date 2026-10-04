# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（操作者直启修订轮）　会话：coder-20261004

## 一、产物历史完成情况

### 项目与需求

- EZR Downloader（ezr download）：TUI 高性能多协议下载器，正式版一期。
- 需求权威来源：`project/mission.md`（v1.3 大纲）+ `project/mission/phase-01.md`（v1.3 详述）。
- **v1.3 修订（本轮，操作者定案）**：①`.ezr` 根目录可经环境变量 `EZR_HOME` 重定位
  （FR-01-70/71 修订，D16）；②新增 FR-01-84 终端哨兵——任意原因终止（含 kill -9）后
  运行 ezr 的终端必须自恢复；③AC-9 同步扩展。操作者指示：本轮流程走完 coder 即交付。

### 产物总账

| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier / coder / cleaner 组 / QA（首轮至四轮） | 详见历史归档 `swarm-20261002-062546` ~ `091047` | 已交付 | 208 测全绿基线 |
| coder（缺陷修复） | 明细表重复表头 + 闪烁两缺陷修复 | 已交付 | 210 测全绿 |
| specifier（三轮修订） | FR-01-81 修订二：并发分块明细表整体移除（三层同步） | 已交付 | 归档 specifier-20261002c |
| coder（三轮修订实现） | 明细表 UI 段落 + 连接级展示机制整体移除 | 已交付 | 206 测全绿、clippy 0、fmt 清零 |
| cleaner（清理批次） | 保持行为清理 3 处 + 覆盖率/CRAP/DRY/变异点四项度量与补测 15 条 | 已交付 | 221 测全绿、clippy 0、fmt 0 |
| architect | 四阶段架构评审 + 4 项保持行为改造 + 2 项裁决 + proptest 属性测试 12 条 + arch_check.sh | 已交付 | 233 测全绿、clippy 0、fmt 0、arch_check EXIT=0 |
| **coder（本轮，v1.3 修订）** | 需求文档三层同步（mission v1.3 / phase-01 v1.3 / feature 场景 10/11）+ `EZR_HOME` 重定位 + 终端哨兵（`src/sentinel.rs`）+ main.rs 接线 | 已交付 | 240 测全绿（详见验证证据）、clippy 0、fmt 0、arch_check 6/6、pty 端到端 A/B/C/D 四部分通过 |

### 遗留受限项（当前有效）

- CPD 余 3 块 `#![allow]` lint 配置头跨文件同形：Rust 逐文件豁免惯用法，登记接受，无需再报。
- CRAP 为受限近似口径（cleaner 会话，llvm-cov 行覆盖近似）；hardender 期以变异结果为准。
- cargo-mutants / cargo-llvm-cov / PMD 沙箱未安装（hardender/QA 期按 notes/rust.md 重装）。
- QA-TD-15 端到端 runner 实现与 9 套整体回归仍欠（QA 节点，承前）。
- features/01-persistence-config 场景 10/11（本轮新增）的 QA runner 实现待 QA 节点补齐
  （coder 按 SKILL 边界不实现 QA 套件）。
- 终端哨兵边界：以 `exec ezr` 方式令 ezr 自任会话首时，SIGKILL 会触发内核对控制终端的
  hangup，复原序列写入将 EIO（termios 仍由哨兵还原）；该场景终端恢复交由终端模拟器
  会话结束复位。常规「shell + 前台作业」场景（操作者报告的场景）已完整覆盖。
- 工具链（本沙箱重建）：rustup minimal + nightly 1.101.0（rustfmt/clippy），`~/.cargo/bin`；
  lld 桥接 `~/.local/bin/ld.lld → ~/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/
  rustlib/x86_64-unknown-linux-gnu/bin/rust-lld`（PATH 需含两目录）。

## 二、当前产出情况（coder → 交付）

### 本会话产物清单

- `project/mission.md`：v1.3 头注（与 phase-01 v1.3 同步；操作者明确指示修订）。
- `project/mission/phase-01.md`：v1.3 修订——FR-01-70/71 增加 `EZR_HOME` 重定位标注；
  新增 FR-01-84（终端哨兵）；AC-9 扩展；D16（EZR_HOME 语义与哨兵机制定案）。
- `project/features/01-persistence-config.feature`：场景清单与描述更新；新增
  场景 10「EZR_HOME 环境变量重定位 .ezr 目录」、场景 11「kill -9 异常终止终端自恢复」。
- `project/ezr/src/model/config.rs`：新增 `ezr_dir()`（`EZR_HOME` 非空生效，未设置/空串
  回退 `~/.ezr`）；`config_path()`/`state_dir()` 改经其派生；新增单测 2 条。
- `project/ezr/src/sentinel.rs`（新增）：终端哨兵（FR-01-84）——ready 握手 + wake 管道
  （字节=正常收尾/EOF=父死）+ `stty -g` 往返还原 termios + crossterm 0.28.1 复原序列
  （逐字核对 vendored 源码）；独立进程组；零 unsafe；`EZR_SENTINEL_TRACE=1` 诊断追踪
  （env 门控，未设零输出）；单测 5 条。
- `project/ezr/src/main.rs`：哨兵子进程模式在 tokio runtime 前短路；TUI 前 `arm()`、
  终端复原后 `release()`；`run_tui()` 抽取使 enable_raw_mode 后所有路径（含
  execute!/Terminal::new 错误分支）均复原终端（顺带修复既有早退不清终端的隐患）。
- `project/ezr/README.md`：EZR_HOME 用法、终端哨兵特性、验证命令与测试数（240）更新。

### 验证证据

| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量测试 | 240P / 0F（199+30+11） | `cargo test` |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| 格式化 | 通过 | `cargo fmt --check` |
| 架构边界 | 6/6 规则通过 | `bash scripts/arch_check.sh` |
| pty 端到端（kill -9 自恢复 / EZR_HOME / 优雅退出 / 隔离+单实例） | A/B/C/D 全部通过 | `python3 /home/z/swarm-ezr2/.work/tmp/e2e_v13.py`（脚本为会话临时件，不随归档保留） |

端到端要点：A——kill -9 后哨兵写出复原序列、termios 回 cooked（ISIG/ECHO/ICANON 恢复，
CTRL+C 可用）、单实例锁随死亡释放、无残留进程；B——registry.json/ezr.lock 落位
`$EZR_HOME/state/`、默认 `~/.ezr` 不创建、Q 退出码 0；C——优雅退出回归（哨兵随 wake
字节静默退场，不误判父死亡）；D——不同 EZR_HOME 两实例并行（隔离）、同 EZR_HOME 第二
实例「ezr 已在运行」非零退出（AC-11）。

### 对账口径

- 单测基线：233（architect 口径，180+30+11 → 192+30+11）**承前不变**；
  本轮净增 +7（config 2 + sentinel 5），全量 240 = 233 + 7，可由
  `cargo test | grep '^test result'` 三段分项复算自洽。
- 属性测试 12 条不变（对账隔离纪律，独立交付物不入单测公式）。

### 待办与待批（未决项）

1. QA-TD-15 端到端 runner + 9 套整体回归（QA 节点，承前）；场景 10/11 的 runner 一并补齐。
2. hardender 期工具重建：cargo-mutants / cargo-llvm-cov（notes/rust.md「本体丢失」条款）。
3. 无待批事项（需求修订经操作者本轮 chat 指令定案，无需再确认）。

### 移交建议

- 操作者已定案：本轮流程到 coder 为止即交付——运行 `bin/swarm complete` 打包归档
  （先 `git add -A`，packaging.md 口径；target/ 由 complete 自动清理）。
- 后续如继续演进，建议顺序：**six-pack/QA**（补 QA-TD-15 与场景 10/11 runner，端到端
  收口 v1.3）→ **six-pack/hardender**（变异加固，`scripts/arch_check.sh` 可纳入验证链）。
