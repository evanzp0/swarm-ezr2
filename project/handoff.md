# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/hardender（v1.3 加固批次）　会话：hardender-20261004
> 上一节点：six-pack/architect（第二轮架构复核，258 测全绿基线）；其完整交接内容见
> git 历史与本文件「产物总账」新增行（本会话按宪法重写 handoff，四要素齐备）。

## 产物总账（新增行）

| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| **hardender（本轮）** | 变异加固批次：环境重建→基线对账→pristine→清单扫描→校准→放量 1210 点→存活处置（7 个加固套件 ~110 条新测试）→软 Gherkin 空清单登记→CRAP/DRY 复核→零改动证明 | 已交付 | 基线 258 测全绿对账一致；加固后全套测试全绿（见验证清单）；arch_check 8/8；clippy 0；fmt 0 |

## 一、本地验证命令清单（按序）

```bash
cd project/ezr
cargo test                                  # 产品全量（258 基线 + 加固套件）
cargo test --test hardening                 # 加固套件（model 层，批次 1）
cargo test --test hardening_g_throttle_err  # throttle/error/engine-mod（批次 2）
cargo test --test hardening_g_proxy         # ezr-proxy（含 include 自带单测）（批次 3）
cargo test --test hardening_g_appcore       # app 层 mod/keys/tasks/paste/cli（批次 4）
cargo test --test hardening_g_main          # main 进程级（批次 5）
cargo test --test hardening_g_engine        # app/engine（批次 6）
cargo test --test hardening_g_supervisor    # supervisor 暂停流（批次 7）
cargo clippy --all-targets                  # 0 警告
cargo fmt --check                           # 0 偏差
bash scripts/arch_check.sh                  # 8/8 EXIT=0
```

注意：加固测试在低配/高载环境偶发内部 tick 时序测试假失败（负载相关，重跑自愈）；
变异会话重试前先清理 `/tmp/ezr-*` 残留目录（PID 复用污染，见 notes §7）。

## 二、会话运行时产物位置与复现方法

会话运行时产物在 `.work/tmp/`（不入产品树，重放即复现）：

| 产物 | 位置 | 复现 |
|---|---|---|
| 基线证据 | `.work/tmp/baseline-{test,clippy,fmt,arch}.log` | 手跑第一节命令 |
| 清单扫描 | `.work/tmp/scan/scan.log` | `cargo mutants --list` |
| 放量轮（1210 点 / 51 片） | `.work/tmp/mut-full/`（ALL-MISSED/ALL-TIMEOUT/full-summary/queue.log） | `.work/tmp/mut-full/run-queue.sh`（前台分片，.done 断点） |
| 存活分组 | `.work/tmp/step7/g_*.missed.txt` | 按文件分组脚本（worklog 记录） |
| **存活处置复跑（23 片）** | `.work/tmp/verify7/`（*.missed.txt/*.timeout.txt/queue.log） | `RUST_TEST_THREADS=2 .work/tmp/verify7/run-queue.sh`；测试口径 = `--bin ezr` + 7 个加固套件 |
| 等价论证/超时型/环境型/偏差台账 | `.work/tmp/step7/EQUIVALENCE.md` | 静态文档 |
| CRAP 度量 | `.work/tmp/step9-crap.lcov` / `step9-crap.md` | `cargo llvm-cov --lcov --branch`（RUST_TEST_THREADS=1 防载噪） |
| DRY 复核 | `.work/tmp/step10/`（positive-control/real-scan-100/DRY-RESULT.md） | `pmd cpd --dir src --language rust --minimum-tokens 100` |
| 零改动证明 | `.work/tmp/step11-cmp.txt`（diff 0 / missing 0 / extra 16=tests/） | pristine 逐文件 cmp |

工具链：cargo-mutants 27.1.0（nightly）、cargo-llvm-cov 0.9.1、PMD 7.28.0
（`.tools/pmd-bin-7.28.0`，以 `bash bin/pmd` 运行）。复现变异参数：
`-j 2 -t 60 --cargo-test-arg=--bin --cargo-test-arg=ezr --cargo-test-arg=--test
--cargo-test-arg=<各加固套件> --cargo-test-arg=-- --cargo-test-arg=--skip
--cargo-test-arg=stale_pair`。

## 三、feature 注释块与差分复用注意事项

- 本会话**未执行**软 Gherkin 差分变异（空清单处置）：`project/features/` 9 项
  feature 均为已批准规格但**无步骤处理器**（`project/qa/runners/suite_*.py` 为
  Gherkin 转写实现，无 Gherkin↔代码绑定），按 notes §5 规则不入清单；feature
  文件零触碰，无工具 stamp/manifest 注释块，无差分复用失效面。
- gherkin-mutator 不存在于 npm registry（404）；未来启用 Gherkin 变异前需先落
  步骤处理器绑定，再选型可用的差分变异工具。

## 四、等价突变体清单及失效条件（回补触发）

全文见 `.work/tmp/step7/EQUIVALENCE.md`（含逐条论证与失效条件）。摘要：

- **等价（复跑 missed 证实）8 点**：throttle 70:25 / 81:21(≥,==) / 78:30(%)，
  error 195:18(≥)，proxy 194:45，supervisor 334:53，engine.rs 205:20，main 331:22。
  回补触发：可注入时钟/磁盘额度、read_line 语义变化、0 字节语料、位域重叠。
- **超时型 6 点**：throttle 81:21(<) / 72:31，proxy 265:11 / 192:5 / 68:5("")，
  supervisor 110:5。回补触发：multi-thread runtime、桩注入 EOF、参数步进注入。
- **环境条件型 13 点**：main 58:5 / 68:5 / 339:5 / 347:16，sentinel 9 点
  （arm 82:5 复跑 missed、115:8、140:27×2/140:36、163:9、308:5/310:8、324:5）。
  仅真实 TTY/产品二进制可观测；回补触发：pty E2E（建议随 six-pack/QA 回补）。

## 五、显式偏差（非等价未处置存活，164 点）与补测指引

1. **UI 渲染层 164 点**（btn 27 / dialog 26 / task_lines 21 / header 20 / detail 17 /
   dialogs 22 / delete 11 / dialog_keys 15 / mouse 13 / list 7 / text 5 / ui-mod 1，
   其中 dialogs 21 点经复跑 unviable 淘汰、87:35 为唯一 missed）：处置方案为按
   hardening_g_appcore 模式挂载 ui/app 模块，对 draw/confirm 函数做 ratatui Buffer
   精确单元断言（布局算术 `+→*` 一测多杀）；入口命名建议 tests/hardening_g_ui*.rs。
2. **supervisor 深水区 14 点**（download/block_worker：289:27、309:14/27/78、
   315:59、435:58、755:21、758:40、773:31、784:44、795:22、852:20）：需多连接
   Range 桩服务器（现 stub 仅整文件响应）+ 块级状态注。
3. 补测优先序：dialogs 87:35（对话框确认流，CRAP 951）→ btn/header（常驻帧）→
   block_worker（下载核心，CRAP 5857）。
4. 汇报口径警示：挂载式加固测试使结构性变异体 unviable（见 notes §7）——
   unviable 不计杀灭，上轮全量 missed 表已按处置台账逐项对账。

## 六、遗留受限项（继承 + 新增）

- 继承上游：CRAP 为受限近似口径（BRDA 分支计数+1 代入标准公式）；单实例锁无
  home 回退分支的 flock 缺失（待操作者裁决）。
- 新增：gherkin-mutator 工具不存在（404）——Gherkin 变异受限登记（空清单亦为
  主因）；LLM 会话沙箱无 TTY，sentinel/终端态类靶点只能论证不可测。

## 七、移交建议

- 建议操作者：`bin/swarm complete six-pack/hardender` 归档后，运行
  `six-pack/QA` 做端到端回归（其 pty E2E 正好回补本会话的环境条件型论证面）。
- 加固套件（7 个 tests/hardening_g_*.rs）随产品树移交，下游改动后按第一节命令
  全量回归。
