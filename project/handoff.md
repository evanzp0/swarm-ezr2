# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner（竞态销项轮后续清理轮——coder SKILL 标准移交路径）
> 路径：…遗留受限项销项轮 ⇒ 竞态销项轮（上轮）⇒ **清理轮（本轮，操作者 chat 指令「用 six-pack/cleaner」）**
> 会话：six-pack-cleaner-20261122-cleanup（上游交接 = 上轮 handoff（竞态销项轮）+ 操作者 chat 指令；ezr 不做旧版本兼容）
> 环境：rustup stable 1.100.0 + nightly 1.102.0（bdf712ee8 2026-11-21；rust-toolchain.toml 钉 nightly 全项目统一）、cargo registry rsproxy sparse、llvm-cov 0.9.2 + cargo-mutants 27.1.0 + PMD 7.29.0（.tools/pmd-bin-7.29.0，与历轮基线同版本）、CARGO_INCREMENTAL=0 + CARGO_PROFILE_DEV_DEBUG=0 + CARGO_PROFILE_TEST_DEBUG=0 三件套全程前台分块。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.57**）。ezr 不做旧版本兼容。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同，见 11-trackers-window v1.14 注记）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| QA runtime 重建轮（上上上轮） | APS 四件套重建 + verdict 级全量 + 07-add-bt 哈希销项 | 已交付 | verdict Σ = 1107 killed + 17 filtered + 0S/0E |
| 遗留受限项销项轮 | ③644 改判杀灭 + ①缺口账目 ID 权威清单 + ④LO_ONLY 敞口消灭 + ②Go 版 mutator filter 移植 | 已交付 | verdict Σ = 1107 killed + 0 survived + 17 filtered；bin 基线 1091P/0F 全绿 |
| 竞态销项轮（上轮） | architecture:389 观察项销项：block_worker 打开失败有界重试窗（workers.rs）+ 两行为锁定单测（supervisor/mod.rs tests） | 已交付 | bin 1147P/0F + 集成 8440P/0F；fmt/clippy clean |
| **清理轮（本轮）** | 竞态销项改动面度量收口 + 3 个纯追加 worker 级测试（错误臂/requeue 臂覆盖） | 已交付 | bins 1150P/0F + 集成 8467P/0F 全绿；fmt clean；clippy 零警告；CRAP/DRY/变异点三度量全过（见二节） |

### 遗留受限项（当前有效）
- architecture:389「supervisor block_worker 打开目标文件失败即单发 fatal 与 resume 作废重建/重试路径理论竞态」——竞态销项轮结清（条目已改写为销项定案，notes/architecture.md）。
- **workers.rs 227-228（磁盘写入失败 fatal）与 239-240（resp.chunk 流错误 classify）为集成级错误臂**：bin 口径纯追加测试不可确定性触发（需 fs/流层注入基建），由集成链覆盖；bin 口径不追测，非缺陷。
- **workers.rs 257-258 死防御臂保留**（`if failure.is_some() { return; }` 于 `if aborted` 返回之后）：控制流证明不可达（循环内所有 failure 赋值臂同迭代必置 aborted；循环外唯一 failure 赋值点立即 return），属防御性不变式护栏，按「删除死代码必须给出证据 + 竞态销项面保持最小」裁决登记保留，不删。
- bin 口径 cov=0 五处登记：property_tests.rs:2223（失败闭包 by design，覆盖它=令测试失败）、main.rs:303（入口壳 env 兜底）、webrtc.rs:213（webrtc 覆盖非确定性受限项）、mouse.rs:516（交互层）、metainfo.rs:52（批次外积压）。
- 2 条 unviable 终态（mutation 账目人工实证定案）+ trackpool.rs:412 CRAP 竞速对账口径（已接受，非可执行任务）沿历轮登记不变。
- 上轮 QA 登记的 schema 口径演进（open_int 三分退役为二分）维持交付态，操作者否决路径仍留档（notes/testing.md 锚回补轮先例）。

## 二、当前产出情况

### 本会话产物清单（清理轮）
1. **3 个纯追加 worker 级测试（supervisor/mod.rs tests 末尾，产品代码零改动）**：
   - `worker_open_non_notfound_absence_is_immediate_fatal`：目录目标（EISDIR，非 NotFound）打开失败 → 即时 fatal（时长断言 <500ms，窗耗尽路径 ≥1s 必假红）——覆盖 open_target_file 非 NotFound 即时错误臂（workers.rs:94），锁定「不吞永久错、不误入重试窗」不变式；
   - `worker_request_error_sets_failure_without_writes`：预建文件 + 死端口 URL → req_checked 错误臂 failure 落账即返、零写入、零完成块——覆盖 workers.rs:197-198；
   - `worker_requeue_scan_skips_done_blocks_and_resumes`：requeue=[已完成块 0, 未完成块 1] → 扫描跳过已完成块、续传未完成块、仅未完成块字节计入本轮写入、断点零字节区不被重写——覆盖 workers.rs:137-143 + 148-150。
2. **度量收口（三工具，全前台）**：CRAP 受限近似直采管线（插桩 → profraw → merge → export lcov → 段级过滤 → crap_calc 两参）；PMD CPD 7.29.0 -l rust --minimum-tokens 70（阳性对照先行）；cargo mutants --list scan 模式逐文件。
3. **经验沉淀**：notes/testing.md 清理轮 2 条（#[path] 双载体测试计数传导对账公式 / worker 级测试目标文件预建纪律）；notes/environment.md 镜像复测与 PMD 版本对齐条目；packs/_common/notes/rust.md E0603 可见性条目；本 handoff.md 重写。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 新增单测 | 3P/0F | `cargo test --bin ezr worker_` |
| bin 单测全量 | 1096P/0F（ezr）+ 30/9/15P（fixture/mktorrent/proxy）= 1150P/0F | `cargo test --bins`（DEBUG 三件套） |
| 集成全量 | 13 目标 8467P/0F（hardening 247 / engine_backfill 982 / ui_backfill 1012 / g_appcore 866 / g_engine 862 / g_main 2 / g_proxy 16 / g_slots 254 / g_supervisor 835 / g_throttle_err 546 / g_v147 849 / mutation 991 / v115 1005） | `cargo test --test <目标>`（逐块前台） |
| fmt / clippy | clean / 零警告 | `cargo fmt --check && cargo clippy --all-targets` |
| 覆盖率 + CRAP | 行覆盖 39667/41911 = 94.6%；ACT（comp≥7∧cov<1）= 101；显示面 ACTIONABLE 146→145（open_target_file 组 + 早期 worker 组全覆盖出榜）；锚 mouse.rs:178 = 77.09 恒等 | 直采管线（notes/testing.md:856/895 口径）+ `qa/runners/crap_calc.py <lcov> <src_root>` |
| DRY（PMD CPD 7.29.0） | 219 组 = 历轮基线精确相等；workers.rs（本轮唯一产品面）零克隆；追加测试区零克隆 | `pmd cpd --dir src --language rust --minimum-tokens 70` |
| 变异点扫描 | workers.rs = 41 点、supervisor/mod.rs = 23 点（cfg(test) 默认排除），均 <100 无需拆分 | `cargo mutants --list --file <文件>` |

### 对账口径
- **测试增量传导（双载体）**：bin +3（1093→1096）→ 集成 +27（8440→8467 = 9 个含 supervisor 模块的集成目标 × 3；hardening/g_main/g_proxy/g_slots 不含该模块零变化）——差值 ÷ bin 差值 = 9，公式对账自洽（notes/testing.md 清理轮条目）。
- **行为不变**：产品代码零改动（本轮唯一改动面 = supervisor/mod.rs 测试模块纯追加）→ 集成中不含新增测试的 4 目标计数逐个恒等（247/2/16/254），行为保持的编译级实证。
- **CRAP 对账三证**：ACT 精确相等（101）+ 稳定函数锚恒等（mouse.rs:178 = 77.09）+ 方向非回归（行覆盖 94.6% 持平、覆盖行数 +103、显示面可修点 146→145）——与 notes/testing.md:895 基线口径一致（webrtc 覆盖非确定性按既有登记不逐点对账）。
- **工具口径**：CRAP = 项目既有 crap_calc.py（bug27 口径内嵌）；DRY = PMD 7.29.0（与历轮基线同版本，初装 7.16.0 已按「度量口径与上游可比」对齐重扫）；变异点 = cargo-mutants --list（scan，未跑变异）。

### 交接四要素
1. **本地验证命令清单（按序）**：`cargo test --bin ezr worker_`（5P）→ `cargo test --bins`（1150P）→ 逐集成目标（8467P）→ `cargo fmt --check` + `cargo clippy --all-targets` → CRAP/DRY/变异点三度量（命令见验证证据表）。
2. **环境**：rust-toolchain.toml 钉 nightly（fmt/clippy 需其 rustfmt unstable 选项），测试运行兼容 stable/nightly 双通道；cargo registry = rsproxy sparse（~/.cargo/config.toml）；llvm-cov 直采管线需 llvm-tools-preview 组件（已装）。
3. **行为不变式（下游注意）**：worker 打开失败重试窗只认 ENOENT（非 NotFound 即时 fatal 由本轮测试锁定）；「无法写入目标文件」fatal 文案被 UI/QA 语境引用，改文案先对账消费点；workers.rs 257-258 为保留的死防御臂（见遗留受限项），若未来新增 failure 赋值臂未同迭代置 aborted 则该臂转活，须重新对账。
4. **夹具纪律**：worker 级测试目标文件必须先预建（`write+create` + `set_len`）再 spawn——漏建会被 ENOENT 重试窗演成 1s fatal 假象（notes/testing.md 清理轮条目）；stop watch sender 保活纪律沿上轮（notes/testing.md 竞态销项轮条目）。

### 待办与待批
- **本会话无待批无待办**——操作者指令的清理轮已落地结清。

### 移交建议
- 下一步建议运行 `six-pack/architect` 技能（cleaner SKILL 标准移交路径；变异点计数 workers.rs=41 / supervisor/mod.rs=23 均远低于 100，无需模块拆分移交）。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。

By cleaner.
