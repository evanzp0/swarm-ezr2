# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（最终独立验证与 QA 收尾——hardender SKILL 标准移交路径）
> 路径：…竞态销项轮 ⇒ 清理轮 ⇒ 架构评审轮 ⇒ 变异加固轮 ⇒ **QA 最终验证轮（本轮，操作者 chat 指令「用 six-pack/QA」；完成后 complete 归档交付 /home/z/my-project/download/）**
> 会话：six-pack-QA-20261123（上游交接 = 上轮 handoff（变异加固轮）+ 操作者 chat 指令；ezr 不做旧版本兼容）
> 环境：rustup stable 1.96.0 + nightly 1.102.0-nightly（c7d5ba59d 2026-11-20，TUNA 2026-11-21 快照，与上轮基线同快照日期；rust-toolchain.toml 钉 nightly 全项目统一）、cargo registry rsproxy sparse、llvm-cov 0.9.2 + PMD 7.29.0（.tools/pmd-bin-7.29.0）全部就位；CARGO_INCREMENTAL=0 + CARGO_PROFILE_DEV_DEBUG=0 + CARGO_PROFILE_TEST_DEBUG=0 三件套全程前台分块。
> **会话中断与恢复**：首轮 QA 会话（20261122）完成开工门复验 + 规程一致性核查 + 6 个欠账用例实现与单测 + 端到端 13 套件后遭遇**沙箱整机回收**（/home/z 非持久挂载全丢）；本轮在持久区（/home/z/my-project/swarm-ezr2）重建环境与工作区、从会话历史精确重建 6 个用例、全量重放全部验证。恢复纪律与镜像实测已沉淀 notes/environment.md 尾节。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.57**）。ezr 不做旧版本兼容。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同，见 11-trackers-window v1.14 注记）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| QA runtime 重建轮 | APS 四件套重建 + verdict 级全量 + 07-add-bt 哈希销项 | 已交付 | verdict Σ = 1107 killed + 17 filtered + 0S/0E |
| 遗留受限项销项轮 | ③644 改判杀灭 + ①缺口账目 ID 权威清单 + ④LO_ONLY 敞口消灭 + ②Go 版 mutator filter 移植；open_int 三分退役二分交付（经操作者裁定结案） | 已交付 | verdict Σ = 1107 killed + 0 survived + 17 filtered |
| 竞态销项轮 | block_worker 打开失败有界重试窗 + 两行为锁定单测 | 已交付 | bins 1147P/0F + 集成 8440P/0F 全绿 |
| 清理轮 | 竞态销项改动面度量收口 + 3 个纯追加 worker 级测试 | 已交付 | bins 1150P/0F + 集成 8467P/0F；CRAP/DRY/变异点三度量全过 |
| 架构评审轮 | 四阶段架构评审 + model↔bt 模块环销项 + arch_check 规则 1 扩域 + PeerTransport 属性测试补缺 | 已交付 | bins 1152P/0F + 集成 8479P/0F + 属性 90P/0F；arch_check 18 规则全过 |
| 变异加固轮 | architect 批次变异加固（82 变异点全判定）+ 加固测试 3 条 + schema 口径演进结案 + APS patch 复用回归修复 + 最终验证序列四步全过 | 已交付 | 82 = 75 killed + 6 unviable + 1 等价；soft 全复用守恒 1124；CRAP 94.6% 恒等；DRY 双扫描恒等 |
| **QA 最终验证轮（本轮）** | 最终独立验证全序列重放 + 规程↔套件一致性核查 + **6 个可执行化欠账用例补齐**（QA-01-16a、QA-RS-15、QA-TD-25/26/27/28）+ BE-11 runner 时序加固 + CRAP/DRY 复验 + 端到端 20 套件全绿 | 已交付 | bins 1152P + 集成 8482P + 属性 90P + fmt/clippy/arch_check 全过；端到端 20 套件全绿（见当前产出②）；CRAP ACTIONABLE 101 恒等 + DRY 221 组逐位置恒等 |

### 遗留受限项（当前有效）
- **workers.rs 227-228（磁盘写入失败 fatal）与 239-240（resp.chunk 流错误 classify）为集成级错误臂**：bin 口径纯追加测试不可确定性触发（需 fs/流层注入基建），由集成链覆盖；bin 口径不追测，非缺陷。
- **workers.rs 257-258 死防御臂保留**（`if failure.is_some() { return; }` 于 `if aborted` 返回之后）：控制流证明不可达，属防御性不变式护栏，按历轮裁决登记保留；若未来新增 failure 赋值臂未同迭代置 aborted 则该臂转活，须重新对账。
- bin 口径 cov=0 五处登记（本轮 CRAP 复验全部在列）：property_tests.rs（失败闭包 by design）、main.rs:303（入口壳 env 兜底）、bt/webrtc.rs（webrtc 覆盖非确定性受限项）、mouse.rs:516（交互层）、metainfo.rs:52（批次外积压）。
- 2 条 unviable 终态（mutation 账目人工实证定案）+ trackpool.rs CRAP 竞速对账口径（已接受，非可执行任务；现存 FN 行号 413）沿历轮登记不变。
- bt/mod.rs 8 子模块 pub mod 面维持现状（hardender 轮裁决收口，观察项关闭）。
- **DRY 几何再登记残留**：CPD 组数 221 vs cleaner 轮基线 219 的 +2 为 architect 纯移动轮几何再登记残留（本轮双扫描逐位置恒等再证，无克隆组涉批次文件），登记为观察项非缺陷。
- **CRAP 行覆盖口径漂移（本轮登记）**：行覆盖 93.69%（41169/43940）vs 上轮基线 94.64%（39828/42084）——分母（LF 插桩行数）随 nightly build 的 coverage mapping 版本漂移（+1856 行/+4.4%；本轮 c7d5ba59d-20261120 vs 上轮 bdf712ee8-20261121），行覆盖百分比不可跨 nightly build 直接对账；CRAP 回归判定锚 = ACTIONABLE_COUNT 与具体函数 CRAP 锚恒等（本轮两者恒等），口径差异随本条登记。

## 二、当前产出情况

### 本会话产物清单（QA 最终验证轮）
1. **开工门基线复验（重放 + 首轮各一次，两次恒等）**：bins 1152P/0F（1098+30+9+15）→ 13 集成目标 8482P/0F（hardening 262 + engine_backfill 982 + g_appcore 866 + g_engine 862 + g_main 2 + g_proxy 16 + g_slots 254 + g_supervisor 835 + g_throttle_err 546 + g_v147 849 + mutation 991 + ui_backfill 1012 + v115 1005）→ 属性 90P/0F → `cargo fmt --check` CLEAN → `cargo clippy --all-targets` 零警告 → `bash scripts/arch_check.sh` 18 规则全过——与上轮 handoff 交接四要素①逐项恒等。
2. **端到端 20 套件全绿（UI 层，qa/runners/ 全量）**：add_task 22P / resume_sidecar 15P / tui_display 25P / config_template 5P / download_engine 16P / integrity_check 12P（IC-07 块首竞速假 FAIL，三证齐复跑绿） / persistence_config 14P / slots_queue 15P / retry_backoff 9P / throttle_proxy TP-01…10、13、14 实跑全 P（TP-09/11/12 规程钦定等价覆盖：≡ QA-01-20 实跑 / ≡ QA-MT-10 实跑 / hardening_g_engine 单测锁） / named_proxy 18P / modify_task 10P / ui_conns 18P / ui_pieces 7P / cmd_window 8P / add_bt 11P+1S（AB-07 钦定 S 登记） / modify_bt 4P / bt_engine 17P+7S（BE-04/07/17/18/19/20/24 钦定 fixture 受限登记） / bt_anti_leech 1P+7S（AL-01…05 钦定登记：RawPeer 交互不可驱动，AL-06 实跑 + BE-11 承载） / trackers_window 6P。
3. **6 个可执行化欠账用例补齐（md 已定义、无钦定归属注记、suite 未实现）**：
   - `QA-01-16a`（bug27 CLI 判重，suite_add_task.py `_case_16a` + run 挂接）：CLI 同 URL 二次添加 toast `⚠ 该链接的任务已存在`、同/异目录判重与列表不变断言（产品行为核证 src/app/cli.rs add_cli_task）。
   - `QA-RS-15`（v1.45 守卫臂，suite_resume_sidecar.py `_case_15` + run 挂接）：存活任务占用守卫——异 URL 同推导名序号改道 `<名>.1` 静默 + CLI 抽测（单实例锁约束下串行会话，.2 改道）；.1 sidecar 观察用慢速 100000 避开 RS-14 已登记的「快路径首刷前即完成不可观察形态」。
   - `QA-TD-25/26/27/28`（v1.34 新增，suite_tui_display.py `_case_25/26/27/28` + 共享辅助 + run 挂接 + QA_TD_ONLY 过滤）：失败任务复制扩展（redirect=20 链 Fatal 秒级失败路径）/ 字段值弹窗 48×12 全矩阵（内容锚定位 + 42/20 wrap + Y 复制 + 拖拽 + 四通道关闭）/ 七弹窗统一 [✗]（几何/配色/按压/拖出/padding/下拉展开期收起）/ 未知尺寸走马灯（streamy fixture 自足 + 15 帧循环采样 + LT - + ? B 退役）。
   - 套件头部「N 用例」声明同步（add_task 18→22、resume_sidecar 14→15、tui_display 14→25）+ suite_add_bt QA-AB-11 docstring 旧措辞（v1.52 口径）修正——断言零改动（v1.52 已注记「断言本按已实现行为断言」）。
4. **BE-11 runner 时序加固（QA SKILL 修复职责，产品源码零改动）**：HTTP 任务 b 拒绝臂原 pump(0.6) 固定等待在「等待中」（探测未完成）时命中更早的「当前状态无可断开」臂——改 wait_for 下载中（30s 有界）后再断言专用臂；复现单跑稳定 F → 修复后单跑 9.8s P → 全量 17P/0F/7S。
5. **规程↔套件一致性核查**：20 对（qa/*.md ↔ runners/suite_*.py）用例集 diff（md 表格用例集 vs py 引用集）——全对应（6 欠账补齐后）+ 3 处钦定等价覆盖注记（TP-09/11/12）+ 2 处 S 登记（AB-07、AL 系列）+ 套件头部声明数全部与注册数对齐。
6. **CRAP 复验**：`cargo llvm-cov --lcov --bin ezr -- --skip property_tests` → awk 过滤 ezr src → `crap_calc.py`——ACTIONABLE_COUNT=101 与历轮既定账逐数恒等（本轮新增 0）；mouse.rs:178=77.09 稳定锚恒等；行覆盖口径漂移见遗留受限项末条。
7. **DRY 复验**：PMD 7.29.0 CPD min-tokens 70 双扫描（worktree vs git archive HEAD pristine）——221 组 = 221 组，**逐位置恒等**（clone 集按（行,文件）序列 diff=0），本轮零漂移（src 零改动再证）。
8. **经验沉淀**：notes/testing.md 尾 7 条（BE-11 型等待中先臂遮蔽 / hover 残留 vs 按压粘滞判定 / 字段值弹窗内容锚与列范围裁剪 / CPD 对账 strip 前缀陷阱 / llvm-cov 行覆盖跨 nightly 不可直比 / QA 覆盖欠账检测口径 / IC-07 三证再证）；notes/environment.md 尾 4 条（沙箱整机回收恢复纪律 + 镜像当期实测 + E0658 nightly 过旧信号 + libtorrent 套件依赖）；INDEX.md 同步。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 开工门基线复验 | bins 1152P/0F + 集成 8482P/0F + 属性 90P/0F + fmt clean + clippy 零警告 + arch_check 18 规则全过（两次重放恒等） | `cargo test --bins` / 逐 `cargo test --test <目标>` / `cargo test --bin ezr property_tests::` / `cargo fmt --check && cargo clippy --all-targets` / `bash scripts/arch_check.sh` |
| 端到端 20 套件 | 全绿（各套件计数见当前产出②；S 登记全部为规程钦定） | `cd project/qa/runners && python3 suite_<名>.py`（TP/RB 支持环境变量 ONLY 分块） |
| 新增 6 用例 | 16a 3.4s P / RS-15 17.2s P / TD-25 14.4s P / TD-26 19.2s P / TD-27 33.5s P / TD-28 39.9s P；全量复放无回归 | `QA_AT_ONLY=16a python3 suite_add_task.py` / `QA_RS_ONLY=15 python3 suite_resume_sidecar.py` / `QA_TD_ONLY=25..28 python3 suite_tui_display.py` |
| CRAP | ACTIONABLE_COUNT=101 恒等（新增 0）+ mouse:178=77.09 锚恒等；行覆盖 93.69%（口径漂移登记） | `cargo llvm-cov --lcov --bin ezr --output-path <lcov> -- --skip property_tests` → `awk '/^SF:/{keep=index($0,"/project/ezr/src/")>0} keep'` → `python3 qa/runners/crap_calc.py <lcov> project/ezr` |
| DRY | 221 组，worktree vs git archive HEAD 双扫描逐位置恒等（本轮零漂移） | `pmd cpd -l rust --minimum-tokens 70 --dir <src> --format text`（两棵树同参数） |
| 产品源码零改动 | src/ 逐文件 cmp 恒等（本轮唯一产品树外改动 = runners 用例与套件头部声明） | `git -C project/ezr status --short src` |

### 对账口径
- **产品源码零改动**：本轮 QA 无产品代码修复；唯一产品树内新增 = 无（BE-11 为 runner 修复；src/ 与 features/ 均零改动，git status 实证）。上轮唯一产品树新增 = 加固测试 bt_vocab_hard.rs（沿上轮账）。
- **端到端计数守恒**：20 套件 P 数总和 = 各套件头部声明数之和（欠账补齐后 22+15+25+5+16+12+14+15+9+12+18+10+18+7+8+11+4+17+1+6 = 239P）+ 钦定 S 15 个（AB-07 1 + BE 7 + AL 7）。throttle_proxy 计数口径：_case_01..10 对应 TP-01…08/13/14（方法重排后映射随头部注记），TP-09/11/12 走钦定等价覆盖。
- **CRAP 锚守恒**：ACTIONABLE_COUNT=101（历轮恒等）+ 函数锚恒等 = 无回归；行覆盖分母漂移为 nightly build 差异（登记口径），非代码漂移（src 零改动 cmp 证明）。
- **工具口径**：llvm-cov 0.9.2 / PMD 7.29.0 / nightly c7d5ba59d-20261120（TUNA 2026-11-21 快照）——与上轮基线同快照日期、rustc hash 相邻 build；nightly 移动目标以测试全绿为准（全套重放绿）。

### 交接四要素
1. **本地验证命令清单（按序）**：`cargo test --bins`（1152P）→ 逐集成目标（8482P，hardening 262）→ `cargo test --bin ezr property_tests::`（90P）→ `cargo fmt --check` + `cargo clippy --all-targets` → `bash scripts/arch_check.sh` → 端到端 20 套件（`python3 suite_<名>.py`，TP 分块 QA_TP_ONLY=1,2 / 3,4,5 / 6,7,8,9,10）→ CRAP/DRY 按上表。
2. **会话运行时产物与复现**：`.work/`（session/plan/tmp/lcov/CPD 双扫描）不进工作区产物；复现 = 按 notes/environment.md 开工顺序重建工具链（持久区 clone + rsproxy 镜像 + TUNA nightly 钉定 + llvm-cov + PMD + pyte/libtorrent）后按上表命令重跑。
3. **feature 注释块与差分复用**：20 份 feature 的 stamp/manifest 为 mutator 自管理注释（上轮软跑刷新，本轮未触碰 mutator——QA 不跑变异，沿上轮 stamp 有效性口径）；差分复用按 scenario_hash + mutation_count（滤选后）+ filter-hash 判定。
4. **等价突变体清单及失效条件**：唯一等价体 = src/model/bt.rs:79:19 `lo < hi`→`lo <= hi`（BtTaskData::span_sum_over_selected_files 边界空转；失效条件：累加引入副作用 / 饱和或 panic 算术 / hi==lo 边界成为规格区分点——任一成立须回补边界测试）。

### 待办与待批
- **本会话无待批无待办**——QA 轮全部验证通过，未发现产品缺陷（BE-11 为 runner 时序问题已修复）；6 个欠账用例补齐属 QA SKILL 职责域内的规程可执行化，无需操作者裁决。

### 移交建议
- **six-pack 流转路径（specifier → coder → cleaner → architect → hardender → QA → Done）已走完，工作流完结**——QA 为最后一名角色，最终验证与 CRAP/DRY 复验全过，产物与清单相互一致。
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 /home/z/my-project/download/）。

By QA.
