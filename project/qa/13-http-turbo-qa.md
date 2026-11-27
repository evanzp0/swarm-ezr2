# QA 套件 · 13-http-turbo HTTP 引擎提速 P0–P2（期号 05，v1.62 新建）

> 对应规格：`project/features/13-http-turbo.feature`（15 场景 + 08b/13b；v1.63/族A：场景 12/13b 退役留空不重排）。
> **v1.63 同步（族A/族B 裁决落实轮 20261127，phase-05 v1.1/D90–D93）**：blocks 引擎整体退役——QA-HT-12/13b 退役留空；QA-HT-13 scheduler 行改判「含 scheduler 键任意值拒绝启动（未知键，FR-02-35/D91）」；QA-HT-15 的「服务器不支持 Range」族判据不变（D93：Range 全拒降级完成不失败，旧预期废弃）。
> 需求依据：`project/mission/phase-05.md` FR-05-01~13、D83–D89（操作者 20261125 指令 + 设计
> 权威 `project/ezr-http-dw-tunning.md`；Inversion 阶段 1 受限退化沿 v1.45–v1.62 轮先例）。
> 行为口径：动态分段调度为默认引擎（`http.scheduler` 默认 dynamic，blocks 一键回退）；
> probe 裸 GET 即数据流（新任务无 Range 头、续传开区间从首个未提交字节）；分段认领/
> 抢占/自适应切分；单连接瞬态错误原地重试吸收（5 次耗尽升级任务级既有退避）；throttle
> 比例等待（限速贴住设定值）；默认并发 8、范围 1–128；磁盘写流水线 + 稀疏预分配 +
> 条件 sync_data；调度模式烘焙于任务启动（B 类快照，热加载不影响运行中任务）；动态
> sidecar = scheduler 字段 + (start, cursor) 段列表（D89，格式即引擎）。
> 端到端 UI 层验证：TUI 可观察状态 + fixture 请求日志（请求形态断言的唯一事实来源）+
> 磁盘产物（part 文件名义大小/磁盘占用/字节完整性）+ kill -9 驱动。运行器可执行化
> （suite runner）归 six-pack/QA 轮。
> ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）；sidecar 动态格式无迁移负担。

## 环境前置

- 伪终端驱动：按键注入、画面捕获、toast/浮层断言；每用例独立 EZR_HOME（隔离 config 与
  state）；终端 120×44。
- HTTP fixture 服务器能力（在 01-download-engine-qa 既有能力上新增）：
  - 请求日志完整记录 Range 头形态（无 Range 头 / 开区间 `bytes=N-` / 封闭区间 `bytes=N-M`）
    与连接时序（并发到达顺序），作请求形态断言的唯一事实来源；
  - 指定连接延迟应答（≥ 可配 stall 时长后再发响应/续体）；
  - 指定连接传输中断连 N 次（重连后从剩余区间继续供数）与 100% 首段断连模式；
  - 忽略 Range 头模式（一律 200 全量应答）；
  - 1 GiB 流式生成文件（不预写盘、支持 Range）；
  - big-100m.bin（100 MB 稀疏）+ `.sha256` 伴随沿既有夹具。
- fixture 文件族：`ten-m.bin`(10 MB)、`one-m.bin`(1 MB)、`twelve-m.bin`(12 MB)、
  `five-m.bin`(5 MB)、`eight-m.bin`(8 MB)、`stall-20m.bin`(20 MB 可延迟)、
  `flappy-always.bin`(首段即断连)、`sparse-g.bin`(1 GiB 流式)、`norange-8m.bin`(忽略 Range)。
- 磁盘采样：`du`/`st_blocks` 读数采样口径固定（同文件系统、下载中暂停态采样避免抖动）。
- kill -9 驱动：进程组信号注入 + 重启续传（01-resume-sidecar-qa 既有口径）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-HT-01 | 13-http-turbo-01 | 默认配置下载 ten-m.bin 与 big-100m.bin（请求日志开） | 首请求无 Range 头；其余连接 `bytes=N-` 开区间并发；详情分块行动态分段口径（`10.0 MB · 动态分段` 形态随进度推进）；两文件最终字节完整 |
| QA-HT-02 | 13-http-turbo-02 | one-m.bin 并发 8 / twelve-m.bin 并发 8 下载（日志开） | 活跃连接峰值 ≥ 2（小文件自适应切分）/ ≥ 8（12 MB）；持段互不重叠；并发不超 8；完成字节完整 |
| QA-HT-03 | 13-http-turbo-03 | stall-20m.bin 并发 4，对一条连接延迟应答 2 s | 延迟窗内其未认领字节被其他连接开区间接管（段内新请求）；任务不失败；最终完整 |
| QA-HT-04 | 13-http-turbo-04 | eight-m.bin 传输中断连 2 次 | 全程「下载中」，无「已失败」/倒计时/失败 toast；日志见同区间重请求；最终字节完整 |
| QA-HT-05 | 13-http-turbo-05 | flappy-always.bin 默认下载（auto_retry=true）与 auto_retry=false 各一轮 | true：6 次连接尝试耗尽后「已失败」→ 既有 8→16→32→60 s 退避倒计时；false：直接停等无倒计时 |
| QA-HT-06 | 13-http-turbo-06 | global_max_download_speed = 8 MiB/s / 20 MiB/s 下载 big-100m.bin | 稳态速度（≥3 s 滑动窗）落设定值 ±10%（头部 ↓ 与列表行一致；±10% 口径沿 01-throttle-proxy） |
| QA-HT-07 | 13-http-turbo-07 | 无 config 启动；添加对话框检查并发字段；下载 twelve-m.bin | 并发字段留空预填 8；实际并发 ≤ 8 且峰值 8 行；完成完整 |
| QA-HT-08 | 13-http-turbo-08 | CLI `-c 128` / `-c 129` / `-c 0` 启动 | 128 启动成功；129/0 非零退出并提示并发数非法（应为 1–128 的整数） |
| QA-HT-08b | 13-http-turbo-08b | config http_concurrency = 128 / 129 启动并下载 five-m.bin | 128 启动成功（实际并发 ≤ 128）；129 启动失败并报错（FR-02-35） |
| QA-HT-09 | 13-http-turbo-09 | 默认配置下载 big-100m.bin（含 .sha256 伴随） | 「SHA-256 校验成功」；字节数 104857600；日志多连接分段交付 |
| QA-HT-10 | 13-http-turbo-10 | sparse-g.bin（1 GiB）限速 64 MiB/s 下载至约 64 MB 后 Space 暂停，采样磁盘占用；Space 继续 | part 名义 1073741824 B 且 du/st_blocks ≪ 名义（随接收增长）；继续后完成且字节完整；若文件系统不支持稀疏则记 S（内嵌判据文案） |
| QA-HT-11 | 13-http-turbo-11 | eight-m.bin 下载至 ≥2 MB 后 kill -9；重启续传 | sidecar 声称字节 ≤ 磁盘实际数据量；从 sidecar 进度续传不从头；完成「SHA-256 校验成功」 |
| QA-HT-12 | 13-http-turbo-12 | **（退役，不执行）** 原 blocks 回退开关用例随 blocks 引擎退役（v1.63/族A，D91——scheduler 键整键退役，blocks 模式不复存在） | **（无判据）** 本用例退役留空不重排（修订纪律三层同步口径）；动态引擎行为由 QA-HT-01–08/09/14/15 全族承载，执行时直接记 S 并注记「D91 退役」 |
| QA-HT-13 | 13-http-turbo-13 | min_split_kib 合法/非法启动 + scheduler 退役键拒绝启动（v1.63/族A 改判：无 scheduler 键+256 启动成功；含 scheduler 键任意值拒绝启动；512 成功；0 拒绝） | 无 scheduler 键启动成功（默认口径）；含 scheduler 键（任意值）启动失败并报错（未知键直接废弃，FR-02-35/D91）；min_split_kib=0 启动失败并报错（FR-02-35）；原 DYNAMIC 大小写不敏感臂随键退役失去载体 |
| QA-HT-13b | 13-http-turbo-13b | **（退役，不执行）** 原调度模式烘焙用例随 scheduler 键退役失去 premise（v1.63/族A，D91——不存在「改写 scheduler 热加载」操作面） | **（无判据）** 本用例退役留空不重排；B 类快照键语义由 QA-RC-07（block_size_http/http_concurrency/max_retries）继续承载，执行时直接记 S 并注记「D91 退役，行为归 QA-RC-07」 |
| QA-HT-14 | 13-http-turbo-14 | eight-m.bin 下载至 ≥30% → Space 暂停 → Space 继续 | 续传请求从首个未提交字节开区间；已提交字节零重复写入（磁盘不回退）；进度从 sidecar 值继续；最终完整 |
| QA-HT-15 | 13-http-turbo-15 | norange-8m.bin 默认下载；同任务下载至有进度后暂停再继续 | 分段请求收 200 后静默退出不写入；toast「⚠ 服务器不支持断点续传，单线程下载: <name>」；首连接单流完成完整；有进度续传臂转「服务器本次未支持断点续传（Range），保留断点稍后重试」 |

## 通过准则

- 全部用例结果记 P（通过）/ F（失败）/ S（受限跳过）；S 必须内嵌环境判据文案（如 QA-HT-10
  稀疏不支持、文件系统/内核限制），不得伪造 P。
- 请求形态断言（无 Range 头 / 开区间 / 封闭区间 / 段内接管）以 fixture 请求日志为唯一
  事实来源；画面断言（分块行/并发面板/toast）以伪终端捕获为准。
- 限速贴附判据（QA-HT-06）±10% 口径沿 01-throttle-proxy；本地 fixture 无外网抖动，不达
  ±10% 即判 F（不放宽）。
- 原地重试口径（QA-HT-04/05）：瞬态断连 < 5 次原地重试上限 → 任务级零感知；累计 6 次连接
  尝试失败 → 任务级失败（D86）。
- kill -9 一致性判据（QA-HT-11）：sidecar 声称字节 ≤ 磁盘实际（sync_data 条件前置）；重启
  后从声称进度续传且最终校验通过。
- 稀疏判据（QA-HT-10）按文件系统实际能力记 S 兜底，判据文案内嵌（du/名义比 + fs 类型）。
- 关联回归：QA-HT-14 与
  01-resume-sidecar（sidecar 一致性/孤儿分支）、QA-HT-05 与 01-retry-backoff（任务级退避
  序列）、QA-HT-13b 与 12-reload-config（固定文案 toast + B 类快照族）联合回归——提速改造
  不得破坏既有语义。
- 性能基准（与 MDM 同机对比、netem 高 RTT 档）属操作者线下验收口径，不进自动化判据
  （受限项：沙箱无 MDM 与 tc netem 环境，登记 handoff）。
