# cargo-mutants 专项注意事项

> 用途分类：cargo-mutants（Rust 变异测试框架）的会话复盘沉淀。变异通用策略见
> `mutation-hardening.md`（语言无关）；本文件只收该工具的机制细节与陷阱。
> 版本口径：27.1.0。

## 断点续跑与状态文件

- **resume 的唯一输入是输出目录里的 `previously_caught.txt`**：`--iterate` 启动时读它跳过
  已杀灭突变体（"Iteration excludes N previously caught or unviable mutants" 的 N 含范围外
  条目，判定剩余量以 `Found N mutants to test` 为准）。`caught.txt`/`missed.txt`/
  `unviable.txt`/`timeout.txt`/`outcomes.json` 都是**每轮重置的当轮报告**，跨轮积累只看
  `previously_caught.txt`。
- **增量落盘发生在突变体之间**：SIGTERM/SIGKILL 落在某个突变体的 build/test 中段时，
  该突变体的当轮判定丢失（下轮重测，正确性无损、只费时间）；落盘间隙才是安全终止点。
  外层限时包 `timeout` 时要把这段损耗算进分片预算。
- **分片续跑前手动合并当轮结果防重测**：`cat caught.txt >> previously_caught.txt` 后
  `sort -u` 去重——工具自己在退出/迭代边界也会往 `previously_caught.txt` 追加，两边都写
  会产生重复行（重复行只浪费 excludes 计数，不影响正确性，但让 `wc -l` 对账失真；
  统计进度以 `sort | uniq` 后的行数为准）。
- **unviable/timeout 也会进 resume 账本**：`previously_caught.txt` 的行数 ≥ caught 数。
  对账公式：去重行数 = caught + unviable + timeout（+ 范围外文件的历史条目）。
- **iterate 长跑需配「等价豁免收敛」外部逻辑**：`--iterate` 对 missed 点会无限重测
  （账本只记 caught/unviable），而确定性等价登记点永远不会被杀灭 → 死循环。托管层
  加 EQUIV_ALLOWLIST：当轮 missed 全部命中书面登记清单 → 视为收敛停机（done+equivOk）
  ；非登记点出现在 missed 则继续重测。清单口径与 missed.txt 行逐字一致（登记与
  推导见 handoff）。
- **并行测试的 timing 波动会造成假 missed，重测即收**：-j 并行下异步/时序敏感测试
  在 mutant 场景的抖动会让部分点偶发存活；iterate 续跑实测 67→39→11 的收敛轨迹。
  连续两轮 missed 的才是确定性存活，才值得人工三分。

## 判定可信度与污染修复

- **盘满/资源故障制造批量假 unviable 与假 caught**：ENOSPC 下 scratch 构建链接失败
  （`error: linking with cc failed`、SIGBUS），突变体被判 unviable；若发生在收尾 flush
  边界，甚至可能进 caught 账本。采信批量判定前抽查逐突变体日志
  （`mutants.out/log/<name>.log`）：真 killed 的日志必有 `test result: FAILED` 类测试输出，
  只有链接错误即污染。修复按 `mutation-hardening.md`「作废重建」做**外科手术版**：
  只从 `previously_caught.txt` 剔除有污染嫌疑的具体行（逐条验日志），不必整轮作废——
  前提是能划清污染时间窗（首个盘满症状之前的结果可信）。
- **挂死突变体吃满超时上限**：循环条件类突变（如 `n == 0` → `!=`）会让测试进入死循环，
  单突变体耗时 = 测试超时上限。`--baseline skip` 下自动超时可能取到很大值（如 `-t` 全局
  上限），批量跑前显式钳位：`-t`（单 cargo 命令上限）与 `--build-timeout`（构建单独放宽，
  冷构建 200s+ 的项目别把构建挤死）、`--minimum-test-timeout`（正常测试时长下限之上留余量）。
  timeout 分类是有效的杀灭（行为差异实证为挂死），但每个挂死突变体都吃满超时预算，
  清单里有已知挂死点时预留时间。

## 资源与执行模型

- **版本探针勿用 `-V`/`--version`（27.1.0 实测）**：版本旗标在 `cargo locate-project`
  之后才解析——项目目录内 `cargo mutants -V` 不打印版本而是直接开扫（实测 30s 超时截断
  时已报 "Found 3247 mutants to test"），项目外则报 locate-project 错误。版本口径取
  发布 tag（`cargo search`/release 页），不要用版本探针触发一次全量扫描。

- **`CARGO_PROFILE_DEV_DEBUG=0` 是磁盘与提速双赢的等价开关**：去 DWARF 调试信息
  （debug_assertions 仍开，测试行为不变、判定不变），target 体积减半、冷构建/链接显著提速。
  变异跑对行为敏感、对调试信息不敏感，磁盘紧张时优先开它。
- **冷构建成本按"进程"计，不按"突变体"计**：每个新进程为每个 worker 复制新 scratch 树
  全量冷构建（首突变体 200s+），同一进程内后续突变体增量构建（10-30s）。分片越碎，
  冷构建税越重——优先长进程（托管父进程模式）少分片。
- **`-j` 的三重上限**：磁盘（worker 数 × 单 target 体积）、内存（并发 rustc/链接各 1-2G）、
  CPU（核数）。4G/2 核/10G 盘的沙箱实测 -j2 是上限；先测单 worker 冷构建耗时与体积
  再定并发。
- **`-f` 与 `-F` 是 AND 组合**：`-f` 按文件 glob 选文件，`-F` 按突变体名正则（名含文件路径
  前缀）再过滤——可表达"某文件的某函数"清单（如 `-F '.*fmt_block_size'`）。范围外已测
  条目留在 resume 账本不影响分片（excludes 只在范围内生效）。
- **`--shard k/n` 的 k 从 0 起**：传 `n/n` 报 `invalid value` 直接退出（`--sharding` 默认
  slice，round-robin 可选）；分片脚本循环时写对边界。
- **CRAP 复测（cargo-llvm-cov）是磁盘大户**：独立 target 目录（`target/llvm-cov-target`）
  带 debuginfo=2 全量重编，需 ~2-3G；跑前清空主 target、关增量（CARGO_INCREMENTAL=0），
  且勿与变异跑批同盘争食——曾出现 ENOSPC 半途爆盘。
- **被中断轮次的 caught 不会自动并入 resume 账本**：SIGTERM/timeout 中断时「退出/迭代边界
  并入」不会发生，已完成的 killed 判定只留在当轮 `caught.txt` 里——续跑前必须手动
  `cat caught.txt >> previously_caught.txt && sort -u`，否则已杀灭突变体整批重测（实测一轮
  损失 19 条判定）。与上条「手动合并」同一动作，此处补充的是**中断轮必然需要它**这一触发条件。

## test-command 参数边界（假杀灭高发）

- **`--` 之后全部参数逐字附加给 `cargo test`，工具自身选项放 `--` 之前**：`-o`/`-L`/`-t`
  等落在 `--` 后会被当成测试命令参数；给测试二进制传参（`--skip`、`--test` 等）必须**连同
  内层 `--` 一起**写入 CARGO_TEST_ARGS（如 `-- --bin ezr -- --skip property_tests::`——
  外层 `--` 是 cargo-mutants 的分隔符，内层 `--` 是 cargo test 的）。放错位置 → 测试命令
  usage error → 每个突变体都"测试失败" → **批量假 killed**（与 runner 聚合缺陷同效的另一
  形态）。
- **假杀灭的识别与外科处置**：抽查 killed 突变体日志（`mutants.out/log/*.log`）——真杀灭
  必有 `test result: FAILED` 测试输出；只有 `unexpected argument`/usage 类命令错误而无测试
  输出 = 假杀灭。处置按「作废重建」外科手术版：从 `previously_caught.txt` 与当轮
  `caught.txt` 剔除涉事行，用正确命令整体重跑（判定只有补跑后才可采信）。

## 双口径加速（phase-1 快杀 + phase-2 全量复核）

- **单包多测试目标项目的构建税**：改动一个产品文件触发全部 `#[path]` 挂载测试 crate 重编
  （构建 60-120s）+ 全量套件运行（~200-300s）= 每突变体 ~300s；而多数纯逻辑突变体死于
  主 bin 自带测试（首个测试目标，~60s）。
- **phase-1/phase-2 协议**：phase-1 用窄测试命令快杀（`-- --bin <主bin> -- --skip <属性>`，
  实测 ~30-60s/突变体）；phase-1 的 killed 是终局判定（bin 内测试真实失败即是证据），
  missed 只是"待复核"不是结论——幸存者名单再用默认全量命令 phase-2 复核（挂载测试 crate
  的加固测试常能杀掉 bin 杀不掉的点）。正确性保持：killed 恒真、missed 必复核，无双口径
  假阳性通道。

- **`--in-place` 与 `-j` 互斥**（27.1.0 实测）：同时传入报 `the argument '--in-place' cannot be used with '--jobs'` 直接退出——in-place 模式固定单 worker（默认 -j1），省略 `-j` 即可，不是需要绕开的缺陷。
- **重定向 stdout 是块缓冲的，进度判定以 mutants.out 台账为准**：输出重定向到文件时，逐突变体的 KILLED/MISSED 行积压在缓冲区、中断时只 flush 出最后几行（实测：stdout 仅见 1 条 MISSED，台账 caught.txt 已积累 20 条）——用 stdout tail 判断进度会系统性低估。每轮结束读 `caught.txt`/`missed.txt`/`outcomes.json`，中断轮按既定条款手工合并进 `previously_caught.txt`。
