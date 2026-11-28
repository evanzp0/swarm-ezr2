# QA 套件 · 13-http-turbo HTTP 引擎 MDM 完全移植（期号 05，v1.3 重写）

> 对应规格：`project/features/13-http-turbo.feature`（15 既有场景修订 + 16–20 新增 + 12/13b 退役留空）。
> **v1.3 重写口径（MDM 完全移植轮 20261128，phase-05 v1.3/D95–D100；操作者指令「废弃 ezr 原
> HTTP 下载的策略、算法和代码，完全移植 mdm 的策略和算法」）**：本套件按 MDM 引擎行为重写——
> 探测 = 不带 Range 的完整 GET 即数据流；分段请求 = 封闭区间 `bytes=<from>-<to-1>`（原开区间
> 口径退役，D99③）；重试 = MDM 节奏（5 次、首次立即、1 s 翻倍封顶 30 s、Retry-After 优先、
> 有进展重置、link_dead 不重试——D86/D93 退役）；限速 = MDM Limiter（burst 常量 1.0 s、
> 热改生效于在途等待）；新增 启动坡道（16）/429 Retry-After（17）/慢连接剔除（18）/主机上限
> 学习（19）/网页拒收（20）五族判据；sidecar = MDM 平铺段 (start,cursor,end)（D98）；
> config 四退役键拒绝启动（D97①）。
> 需求依据：`project/mission/phase-05.md` v1.3（FR-05-01~14、D95–D100；设计权威 = MDM 源码）。
> 端到端 UI 层验证：TUI 可观察状态 + fixture 请求日志（请求形态断言的唯一事实来源）+ 磁盘产物
> （part 文件名义大小/磁盘占用/字节完整性）+ kill -9 驱动。运行器可执行化（suite runner）归
> six-pack/QA 轮。ezr 不做旧版本兼容（沿 v1.45/FR-02-35）；sidecar 平铺格式无迁移负担
> （旧格式一律按无台账处理，D98）。

## 环境前置

- 伪终端驱动：按键注入、画面捕获、toast/浮层断言；每用例独立 EZR_HOME（隔离 config 与
  state）；终端 120×44。
- HTTP fixture 服务器能力（在 01-download-engine-qa 既有能力上新增/修订）：
  - 请求日志完整记录 Range 头形态（无 Range 头 / **封闭区间** `bytes=N-M` / 其余）与连接
    时序（并发到达顺序），作请求形态断言的唯一事实来源（v1.3：开区间形态断言退役）；
  - 指定连接延迟应答（≥ 可配 stall 时长）与指定连接限速（慢连接剔除用）；
  - 指定连接传输中断连 N 次（重连后从剩余区间继续供数）与 100% 首段断连模式；
  - 指定请求返回 429 + Retry-After 头（引擎级重试用）；
  - 对第 N 条之后的连接一律 403（主机上限学习用）；
  - 忽略 Range 头模式（一律 200 全量应答）与 text/html 网页应答模式（GotWebPage 用）；
  - 1 GiB 流式生成文件（不预写盘、支持 Range）；
  - big-100m.bin（100 MB 稀疏）+ `.sha256` 伴随沿既有夹具。
- fixture 文件族：`ten-m.bin`(10 MB)、`one-m.bin`(1 MB)、`quarter-m.bin`(256 KB)、
  `twelve-m.bin`(12 MB)、`five-m.bin`(5 MB)、`eight-m.bin`(8 MB)、`stall-20m.bin`(20 MB 可延迟)、
  `flappy-always.bin`(首段即断连)、`sparse-g.bin`(1 GiB 流式)、`norange-8m.bin`(忽略 Range)、
  `retry-429.bin`(可控 429)、`shed-20m.bin`(可控限速)、`hostcap-8m.bin`(可控 403 拒绝)、
  `page/app.html`(网页应答)。
- 磁盘采样：`du`/`st_blocks` 读数采样口径固定（同文件系统、下载中暂停态采样避免抖动）。
- kill -9 驱动：进程组信号注入 + 重启续传（01-resume-sidecar-qa 既有口径）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-HT-01 | 13-http-turbo-01 | 默认配置下载 ten-m.bin 与 big-100m.bin（请求日志开） | 首请求无 Range 头且响应体流至完成（探测即数据流）；其余连接 `bytes=N-M` 封闭区间并发；详情分块行动态分段口径（`10.0 MB · 动态分段` 形态随进度推进）；两文件最终字节完整 |
| QA-HT-02 | 13-http-turbo-02 | one-m.bin 并发 8 / twelve-m.bin 并发 8 下载（日志开） | 活跃连接峰值 ≥ 2（小文件自适应切分）/ ≥ 8（12 MB）；持段互不重叠；并发不超 8；完成字节完整 |
| QA-HT-03 | 13-http-turbo-03 | stall-20m.bin 并发 4，对一条连接延迟应答 2 s | 空闲连接按 planner 规则接管（下一段未接收且 ≥ preempt_after 抢占，或最大剩余段对半切取上半）；fixture 见段内新封闭区间请求；任务不失败；最终完整 |
| QA-HT-04 | 13-http-turbo-04 | eight-m.bin 传输中断连 2 次 | 全程「下载中」，无「已失败」/倒计时/失败 toast；日志见同区间重请求（release 后重新认领）；最终字节完整 |
| QA-HT-05 | 13-http-turbo-05 | flappy-always.bin 默认下载（auto_retry=true）与 auto_retry=false 各一轮 | true：6 次连接尝试耗尽后「已失败」→ 应用层 8→16→32→60 s 退避倒计时；false：直接停等无倒计时 |
| QA-HT-06 | 13-http-turbo-06 | global_max_download_speed = 8 MiB/s / 20 MiB/s 下载 big-100m.bin；下载中改任务级限速一轮 | 稳态速度（≥3 s 滑动窗）落设定值 ±10%（±10% 口径沿 01-throttle-proxy）；改限速后速率按新值收敛（热改生效于在途等待） |
| QA-HT-07 | 13-http-turbo-07 | 无 config 启动；添加对话框检查并发字段；下载 twelve-m.bin | 并发字段留空预填 8；实际并发 ≤ 8 且峰值 8 行；完成完整 |
| QA-HT-08 | 13-http-turbo-08 | CLI `-c 128` / `-c 129` / `-c 0` 启动 | 128 启动成功；129/0 非零退出并提示并发数非法（应为 1–128 的整数） |
| QA-HT-08b | 13-http-turbo-08b | config http_concurrency = 128 / 129 启动并下载 five-m.bin | 128 启动成功（实际并发 ≤ 128）；129 启动失败并报错（FR-02-35） |
| QA-HT-09 | 13-http-turbo-09 | 默认配置下载 big-100m.bin（含 .sha256 伴随） | 「SHA-256 校验成功」；字节数 104857600；日志多连接封闭区间分段交付 |
| QA-HT-10 | 13-http-turbo-10 | sparse-g.bin（1 GiB）限速 64 MiB/s 下载至约 64 MB 后 Space 暂停，采样磁盘占用；Space 继续 | part 名义 1073741824 B 且 du/st_blocks ≪ 名义（随接收增长）；继续后完成且字节完整；若文件系统不支持稀疏则记 S（内嵌判据文案） |
| QA-HT-11 | 13-http-turbo-11 | eight-m.bin 下载至 ≥2 MB 后 kill -9；重启续传 | sidecar 声称字节 ≤ 磁盘实际数据量（snapshot→flush→write）；从 sidecar 进度续传不从头；完成「SHA-256 校验成功」 |
| QA-HT-12 | 13-http-turbo-12 | **（退役，不执行）** 原 blocks 回退开关用例随 blocks 引擎退役（v1.63/族A，D91）；v1.3 MDM 移植后引擎单形态（D95） | **（无判据）** 本用例退役留空不重排；执行时直接记 S 并注记「D91/D95 退役」 |
| QA-HT-13 | 13-http-turbo-13 | 键面矩阵：min_split_kib=12/64/0；各含 block_size_http、throttle_burst_secs、throttle_sleep_ms、dns_race_grace_ms、scheduler 键的 config 启动 | 12/64 启动成功；0 拒绝启动（FR-02-35）；五个退役键（任意值）均拒绝启动并报错（D97①/D91） |
| QA-HT-13b | 13-http-turbo-13b | **（退役，不执行）** 原调度模式烘焙用例随 scheduler 键退役失去 premise（v1.63/族A，D91） | **（无判据）** 本用例退役留空不重排；B 类快照键语义由 QA-RC-07（http_concurrency/min_split_kib/max_retries）继续承载，执行时直接记 S 并注记「D91 退役，行为归 QA-RC-07」 |
| QA-HT-14 | 13-http-turbo-14 | eight-m.bin 下载至 ≥30% → Space 暂停 → Space 继续（日志开） | 续传请求 = 封闭区间 bytes=(cursor−64)-(end−1)（64 B 覆叠重取）；已提交字节零重复写入；进度从 sidecar 平铺台账值继续；最终完整 |
| QA-HT-15 | 13-http-turbo-15 | norange-8m.bin 默认下载；同任务下载至有进度后暂停再继续 | 分段请求收 200 后静默退出不写入；toast「⚠ 服务器不支持断点续传，单线程下载: <name>」；探测流吸收收口（fixture 无对同文件的全量重发，首连接以既有数据流完成）；暂停后继续 = 从头重新下载（无台账） |
| QA-HT-16 | 13-http-turbo-16 | quarter-m.bin(256 KB) 与 twelve-m.bin 并发 8 下载（日志开） | 初始并发 = min(8, ⌈size/64 KiB⌉)（256 KB → 4 条；12 MB → 8 条）；后续经切分回升至上限；两文件最终字节完整 |
| QA-HT-17 | 13-http-turbo-17 | retry-429.bin：指定请求 429 + Retry-After: 1 s | 该连接等待 1 s 后重试；任务全程「下载中」；最终字节完整 |
| QA-HT-18 | 13-http-turbo-18 | shed-20m.bin 并发 4：指定连接限速 64 KiB/s（其余全速、全局不限速） | 慢连接 ≥2 s 后被剔除重连（fixture 见其段区间新请求）；面板行回「正在连接...」；最终字节完整 |
| QA-HT-19 | 13-http-turbo-19 | hostcap-8m.bin：第 3 条起连接一律 403；并发 2 下载；完成后同 host 第二任务 | 被拒连接 2 次观察后 Retired{host,2} 退场不耗重试；后续任务并发按学习上限收窄；两任务完成字节完整 |
| QA-HT-20 | 13-http-turbo-20 | 添加 page/app.html 地址任务 | 任务「已失败」（URL 失效类）；不产生 part/台账文件；「不自动重试」不占槽位 |

## 通过准则

- 全部用例结果记 P（通过）/ F（失败）/ S（受限跳过）；S 必须内嵌环境判据文案（如 QA-HT-10
  稀疏不支持、文件系统/内核限制），不得伪造 P。
- 请求形态断言（无 Range 头 / **封闭区间 `bytes=N-M`** / 段内接管 / 覆叠重取）以 fixture
  请求日志为唯一事实来源；画面断言（分块行/并发面板/toast）以伪终端捕获为准。
  开区间 `bytes=N-` 形态在任何日志中均不应出现（D99③）。
- 限速贴附判据（QA-HT-06）±10% 口径沿 01-throttle-proxy；本地 fixture 无外网抖动，不达
  ±10% 即判 F（不放宽）。
- 引擎重试口径（QA-HT-04/05/17）：瞬态断连/429 在 5 次引擎重试窗内被吸收 → 任务级零感知；
  无进展累计 6 次连接尝试 → 任务级失败（应用层 auto_retry 接管，D97④）；403/404 等
  link_dead 不重试（QA-HT-19 判据覆盖）。
- 慢连接剔除（QA-HT-18）：SHED_GRACE 2 s / 比率 4 / 剩余 2 s 判据由 fixture 时序承载；
  全局或任务级限速开启时剔除不触发（MDM should_shed limited 短路）——以 QA-HT-18 单独
  不限速轮承载，限速轮（QA-HT-06）不观察剔除。
- 主机上限学习（QA-HT-19）：NEEDED=2 观察 + DECAY 10 min；学习上限只在 fixture 可控 403
  模式下验证，真实网络不受控场景记 S 兜底。
- kill -9 一致性判据（QA-HT-11）：sidecar 声称字节 ≤ 磁盘实际；重启后从声称进度续传且最终
  校验通过；旧格式台账（无平铺段）按无台账从头（D98）——以预置旧格式台账负探针承载（QA 轮
  接手面）。
- 稀疏判据（QA-HT-10）按文件系统实际能力记 S 兜底，判据文案内嵌（du/名义比 + fs 类型）。
- 关联回归：QA-HT-14 与 01-resume-sidecar（一致性/孤儿分支）、QA-HT-05 与 01-retry-backoff
  （任务级退避序列）、QA-HT-13 与 01-persistence-config-04（退役键拒绝族）、QA-HT-13b 与
  12-reload-config（固定文案 toast + B 类快照族）、QA-HT-01/14/15 与 01-download-engine
  （探测/重定向/单流族）联合回归——MDM 移植不得破坏既有应用层语义。
- 性能基准（EZR/MDM 同机同文件对表）属操作者线下验收口径，不进自动化判据（受限项：沙箱
  无 MDM 运行环境，登记 handoff）。
