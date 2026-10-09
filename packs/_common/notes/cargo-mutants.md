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
