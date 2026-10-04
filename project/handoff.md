# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner（操作者直启，coder v1.3 交付后的清理批次）　会话：cleaner-20261004

## 一、产物历史完成情况

### 项目与需求

- EZR Downloader（ezr download）：TUI 高性能多协议下载器，正式版一期。
- 需求权威来源：`project/mission.md`（v1.3 大纲）+ `project/mission/phase-01.md`（v1.3 详述）。
- **v1.3 修订（coder 轮，操作者定案）**：①`.ezr` 根目录可经环境变量 `EZR_HOME` 重定位
  （FR-01-70/71 修订，D16）；②新增 FR-01-84 终端哨兵——任意原因终止（含 kill -9）后
  运行 ezr 的终端必须自恢复；③AC-9 同步扩展。

### 产物总账

| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier / coder / cleaner 组 / QA（首轮至四轮） | 详见历史归档 `swarm-20261002-062546` ~ `091047` | 已交付 | 208 测全绿基线 |
| coder（缺陷修复） | 明细表重复表头 + 闪烁两缺陷修复 | 已交付 | 210 测全绿 |
| specifier（三轮修订） | FR-01-81 修订二：并发分块明细表整体移除（三层同步） | 已交付 | 归档 specifier-20261002c |
| coder（三轮修订实现） | 明细表 UI 段落 + 连接级展示机制整体移除 | 已交付 | 206 测全绿、clippy 0、fmt 清零 |
| cleaner（清理批次） | 保持行为清理 3 处 + 覆盖率/CRAP/DRY/变异点四项度量与补测 15 条 | 已交付 | 221 测全绿、clippy 0、fmt 0 |
| architect | 四阶段架构评审 + 4 项保持行为改造 + 2 项裁决 + proptest 属性测试 12 条 + arch_check.sh | 已交付 | 233 测全绿、clippy 0、fmt 0、arch_check EXIT=0 |
| coder（v1.3 修订） | 需求文档三层同步 + `EZR_HOME` 重定位 + 终端哨兵（`src/sentinel.rs`）+ main.rs 接线 | 已交付 | 240 测全绿、clippy 0、fmt 0、arch_check 6/6、pty 端到端 A/B/C/D 通过 |
| **cleaner（本轮，v1.3 后清理批次）** | 保持行为清理 2 处 + `parse_speed` 表驱动改造 + sentinel 补测 6 条 + 覆盖率/CRAP/DRY/变异点四项度量 | 已交付 | 246 测全绿（详见验证证据）、clippy 0、fmt 0、arch_check 6/6 |

### 遗留受限项（当前有效）

- CPD 余 3 块 `#![allow]` lint 配置头跨文件同形（本轮复核与既往登记一致，无新增）：
  model/engine 层两个子集家族 + app/mod.rs 与 ezr-proxy.rs 交互层豁免头。Rust 逐文件
  豁免惯用法，登记接受，无需再报。
- CRAP 为受限近似口径（llvm-cov 行覆盖 + BRDA 分支计数代入标准公式；偏差方向：`?` 操作符
  无 BRDA → 入口胶水 comp 低估，测试代码 assert! 宏展开 → 测试函数虚增，门禁排除测试模块）；
  hardender 期以变异结果为准。**早期批次遗留 CRAP>6 共 54 项**（最高：
  `app/dialog_keys.rs` add_dialog_char 136.1 / `app/mouse.rs` on_mouse 131.7 /
  `bin/ezr-fixture` emit_throttle 72.0 / `ui/dialog.rs` draw_add_dialog 36.0，多为
  分发核心与交互层，comp=分支数属既有登记形态）；本轮批次文件（config/sentinel/main）
  全部函数 CRAP ≤ 6。
- 工具链（本沙箱本轮重建）：rustup minimal + nightly 1.101.0（rustfmt/clippy），
  `~/.cargo/bin`；lld 桥接 `~/.local/bin/ld.lld`（PATH 需含两目录）。
  度量工具：cargo-mutants 27.1.0、cargo-llvm-cov 0.9.1（llvm-tools-preview 已装），
  `~/.cargo/bin`；PMD 7.28.0 在 `.tools/pmd-bin-7.28.0/`（`pmd cpd` 子命令，`-l rust` 必带）。
- QA-TD-15 端到端 runner 实现与 9 套整体回归仍欠（QA 节点，承前）；
  features/01-persistence-config 场景 10/11 的 QA runner 一并待 QA 节点补齐。
- 终端哨兵边界（承前）：以 `exec ezr` 方式令 ezr 自任会话首时，SIGKILL 会触发内核对控制
  终端的 hangup，复原序列写入将 EIO（termios 仍由哨兵还原）；常规「shell + 前台作业」
  场景已完整覆盖。
- pty 端到端脚本（e2e_v13.py）为 coder 会话临时件未随归档保留：本轮未复跑（本轮产品行为
  零变更，单测 246 全绿 + 上轮 pty A/B/C/D 证据承前有效）。

## 二、当前产出情况（cleaner → 交付）

### 本会话产物清单

- `project/ezr/src/model/config.rs`：
  - `Config::load` 过期 doc 重写（移除指向不存在函数的 `sanitize` 引用与不成立的
    `# Errors`/`Result` 段落，git 历史证实 `load` 自始即 `Self` 签名）；
  - `parse_speed` 表驱动改造（else-if 链 → `SPEED_UNITS` 常量表 find_map，匹配顺序与
    乘数逐项保持，行为不变，既有 `speed_parsing` 等测试钉住）。
- `project/ezr/src/sentinel.rs`：
  - 删除 `run_child` 中 `let _ = outcome;` 死语句（通配符丢弃不移动值，纯无操作）；
  - 测试模块**纯追加** 6 条：`wait_ready` 握手字节/EOF 双路径、`restore_tty` 无 tty
    no-op 分支、`restore_tty` 非 tty 文件（序列可写 + termios 必败）、`stty_save`/
    `stty_restore` 非 tty 失败容错、`TerminalSentinel` release/Drop 通知语义（构造
    child=None 句柄，不涉真实子进程）、`trace` env 门控全路径+静默路径（单测试双相位，
    顺序执行防 env 键竞态）。
- `project/ezr/README.md`：自动化验证测试数 240 → 246。
- `packs/_common/notes/rust.md`：+3 条经验（llvm-cov `--branch` 与 BRDA 阳性对照、
  BRDA comp 近似两个偏差方向、PMD 7.x 获取与调用）。
- `packs/_common/engineering.md`：+1 条经验（同一 env 键只归一个测试触碰，双分支合并
  顺序相位）。

### 验证证据

| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 全量测试 | 246P / 0F（205+30+11） | `cargo test` |
| clippy | 0 警告 | `cargo clippy --all-targets` |
| 格式化 | 通过 | `cargo fmt --check` |
| 架构边界 | 6/6 规则通过 | `bash scripts/arch_check.sh` |
| 覆盖率（llvm-cov 行覆盖） | 总 89.40% → 89.93%；sentinel.rs 38.60% → 72.51%；config.rs 97.11% | `cargo llvm-cov --summary-only` |
| CRAP（受限近似） | 批次文件全部函数 ≤ 6（config 最高 4.0 / sentinel 最高 5.5） | 口径见 notes/rust.md（--branch + BRDA 分支计数） |
| DRY（PMD CPD，70 tokens） | 3 块，均为既往登记的 allow 头，无新增 | `pmd cpd --dir src -l rust --minimum-tokens 70` |
| 变异点扫描（scan/count） | 全 crate 1657 点/47 文件，无文件 >100；批次 config 34 / sentinel 30 / main 30 | `cargo mutants --list` |

### 对账口径

- 单测基线：240（coder v1.3 口径，199+30+11）→ 本轮净增 **+6**（sentinel 补测），
  全量 246 = 205+30+11，三段分项可由 `cargo test | grep '^test result'` 复算自洽。
- 属性测试 12 条不变（对账隔离纪律，独立交付物不入单测公式）。
- 覆盖率/CRAP/DRY/变异点四项数字均为实测值；CRAP 口径偏差方向见「遗留受限项」。

### 待办与待批（未决项）

1. QA-TD-15 端到端 runner + 9 套整体回归 + 场景 10/11 runner（QA 节点，承前）。
2. hardender 期：变异加固全量执行（工具本轮已在位）；早期批次 CRAP>6 的 54 项以变异
   结果为准复核，分发核心/交互层拆分裁决归 architect。
3. 无待批事项。

### 移交建议

- 运行 `bin/swarm complete` 打包归档（先 `git add -A`，packaging.md 口径；target/ 由
  complete 自动清理）。
- 后续如继续演进，按 six-pack 角色链顺序建议：**six-pack/architect**（架构评审，复核
  本轮 parse_speed 表驱动改造与四项度量的结构面）→ **six-pack/hardender**（变异加固）
  → **six-pack/QA**（补 QA-TD-15 与场景 10/11 runner，端到端收口 v1.3）。
