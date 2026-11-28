# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   13-http-turbo-01 探测即数据流与封闭区间并发
#   13-http-turbo-02 自适应分段切分维持并发
#   13-http-turbo-03 慢连接段抢占（重指派 + 最大段对半切）
#   13-http-turbo-04 瞬态断连重新认领（不转任务级失败）
#   13-http-turbo-05 连接重试耗尽升级任务级退避
#   13-http-turbo-06 限速贴住设定值（Limiter 令牌桶）
#   13-http-turbo-07 默认并发 8 与对话框回退
#   13-http-turbo-08 并发范围 1–128（CLI 与 config）
#   13-http-turbo-08b config http_concurrency 边界与非法拒绝
#   13-http-turbo-09 大文件端到端正确性（Writer 写流水线）
#   13-http-turbo-10 稀疏预分配磁盘占用随进度增长
#   13-http-turbo-11 kill -9 崩溃一致性续传
#   13-http-turbo-12 blocks 回退开关旧行为【退役，留空不重排——v1.63/族A：blocks 引擎退役】
#   13-http-turbo-13 config 键面严格校验（v1.3/D97：四退役键拒绝启动 + min_split_kib 默认 12）
#   13-http-turbo-13b 调度模式烘焙于任务启动（B 类快照）【退役，留空不重排——v1.63/族A：scheduler 键退役】
#   13-http-turbo-14 断点续传（封闭区间 + 64 B 覆叠校验 + 平铺台账）
#   13-http-turbo-15 服务器不支持 Range 降级单流（探测流吸收收口）
#   13-http-turbo-16 启动坡道（INITIAL_PIECE 64 KiB/连接，v1.3 新增）
#   13-http-turbo-17 引擎级 429 Retry-After 重试（v1.3 新增）
#   13-http-turbo-18 慢连接剔除重连（shed，v1.3 新增）
#   13-http-turbo-19 主机连接上限学习与退役（v1.3 新增）
#   13-http-turbo-20 网页应答拒收（GotWebPage，v1.3 新增）
Feature: 13-http-turbo HTTP 引擎 MDM 完全移植（phase-05 v1.3：planner 动态分段 · 探测即数据流 · MDM 重试节奏 · Writer 流水线 · Limiter/DNS/主机上限）

  期号 05。v1.3 依据 project/mission/phase-05.md（MDM 完全移植轮）与操作者 20261128
  chat 指令：废弃 ezr 原 HTTP 下载策略/算法/代码，完全移植 misterdownloadmanager
  （github.com/anefzaoui/misterdownloadmanager）的策略与算法；设计权威 = MDM 源码
  （D95）；待办与遗留不需审批（D95–D100 源裁决可回退）；ezr 不做旧版本兼容
  （沿 v1.45/FR-02-35）。v1.62 的 ezr-http-dw-tunning.md 设计权威退役。
  修订要点（相对 v1.63 口径）：①分段请求改封闭区间 bytes=<from>-<to-1>（原开区间退役，
  D99③）；②探测 = 不带 Range 的完整 GET 即数据流（沿袭 = MDM）；③重试节奏 = MDM
  （5 次、首次立即、1 s 翻倍封顶 30 s、Retry-After 优先、有进展重置、link_dead 不重试，
  D86/D93 退役）；④min_split 默认 12 KiB（D85/D97②）、启动坡道 64 KiB/连接、
  align 4096、preempt_after = 2×TTFB 下限 20 ms；⑤限速 = MDM Limiter（burst 常量 1.0 s、
  热改生效于在途等待；throttle_burst_secs/throttle_sleep_ms 键退役，D97①）；⑥慢连接剔除
  shed（2 s 宽限/4× 比率/2 s 剩余，限速时禁用）；⑦主机连接上限学习（Retired）；⑧sidecar
  = MDM 平铺段 (start,cursor,end) + validators（D98，旧格式一律按无台账处理）；⑨重定向
  上限 10 → 20（D97③）；⑩config 四退役键拒绝启动（block_size_http/throttle_burst_secs/
  throttle_sleep_ms/dns_race_grace_ms，D97①）；⑪pieces 网格 = 动态段（D99⑤）。
  「ttp]」字样均为终端显示伪象，磁盘字节完好（v1.2 核证沿袭）。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件

  Scenario: 13-http-turbo-01 探测即数据流与封闭区间并发
    Given fixture 文件 "<file>" 大小 <size> 且支持 Range 请求
    When 以默认配置添加并开始下载
    Then fixture 记录的首个请求为不带 Range 头的完整 GET（探测即数据流，MDM D-007）
    And 首请求响应体自字节 0 起持续落盘（fixture 首请求未中断至完成，FR-05-02）
    And 其余连接以封闭区间 Range（bytes=<from>-<to-1> 形态）并发加入（fixture 请求日志，D99③）
    And 详情分块行显示动态分段口径（"<size> · 动态分段" 形态，随 <已下载>/<总字节> 推进，D83）
    And 下载最终完成且文件字节完整

    Examples:
      | file         | size    |
      | ten-m.bin    | 10.0 MB |
      | big-100m.bin | 100 MB  |

  Scenario: 13-http-turbo-02 自适应分段切分维持并发
    Given fixture 文件 <file> 支持且可记录各连接请求区间
    When 以并发数 <concurrency> 添加并开始下载
    Then 实际活跃连接峰值 ≥ <min_conns> 且各连接持段互不重叠（并发连接面板与 fixture 日志，planner 不变量 FR-05-01）
    And 实际并发不超过 <concurrency>
    And 下载完成后详情分块行显示 "<size> · 动态分段"（D83）且文件字节完整

    Examples:
      | file         | concurrency | min_conns |
      | one-m.bin    | 8           | 2         |
      | twelve-m.bin | 8           | 8         |

  Scenario: 13-http-turbo-03 慢连接段抢占（重指派 + 最大段对半切）
    Given fixture 文件 "stall-20m.bin" 支持且可对指定连接延迟应答 ≥ <stall>
    When 以并发数 4 添加并开始下载
    Then 空闲连接按 planner 规则取得新工作：紧随其刚完成段的下一段在其 worker 未开始接收且等待 ≥ preempt_after 时被抢占（2×TTFB 下限 20 ms，FR-05-01②），或最大剩余段（含被延迟连接的未认领字节）被对半切分取得上半（FR-05-01③；fixture 出现段内新请求）
    And 被延迟连接未认领字节逐步被其他连接封闭区间请求接管
    And 任务保持「下载中」不转「已失败」
    And 下载最终完成且文件字节完整

    Examples:
      | stall |
      | 2 s   |

  Scenario: 13-http-turbo-04 瞬态断连重新认领（不转任务级失败）
    Given fixture 文件 "eight-m.bin" 且传输中对已建立连接断连 <drops> 次（重连后从剩余区间继续供数）
    When 以默认配置添加并开始下载
    Then 任务全程保持「下载中」且不出现「已失败」、重试倒计时与失败 toast（连接失败释放段后重新认领，FR-05-03；有进展重置沿 MDM at_last_failure）
    And fixture 记录到同字节区间的重新请求（段回归未分配后由原连接或其他连接认领，FR-05-01 release）
    And 下载最终完成且文件字节完整

    Examples:
      | drops |
      | 2     |

  Scenario: 13-http-turbo-05 连接重试耗尽升级任务级退避
    Given fixture 文件 "flappy-always.bin" 的连接一律在传输首段即断连（重试持续失败、无进展）
    When 以默认配置添加并开始下载（auto_retry = true）
    Then 累计 6 次连接尝试仍失败后任务转「已失败」（引擎重试 5 次：首次立即 + 1/2/4/8 s 翻倍封顶 30 s，D97④/FR-05-03）
    And 列表行按既有指数退避自动重试（8→16→32→60 s 封顶，应用层 auto_retry 族沿袭）且倒计时结束续传
    And auto_retry = false 时同 fixture 直接「已失败」停等且无倒计时

    Examples:
      | file              |
      | flappy-always.bin |

  Scenario: 13-http-turbo-06 限速贴住设定值（Limiter 令牌桶）
    Given 配置文件设 [common] 表 global_max_download_speed = "<speed>"
    When 下载文件 "<file>" 至传输稳定（≥ 3 秒滑动窗）
    Then 全局下载速度稳定在 <speed> ±10%（MDM Limiter 移植：burst 常量 1.0 s、收包侧计费 TCP 反压，FR-05-05；throttle_burst_secs/throttle_sleep_ms 键已退役 D97①）
    And 下载中修改任务级限速后速率按新限速收敛（热改生效于在途等待，MDM earned-mark）

    Examples:
      | speed    | file         |
      | 8 MiB/s  | big-100m.bin |
      | 20 MiB/s | big-100m.bin |

  Scenario: 13-http-turbo-07 默认并发 8 与对话框回退
    Given 默认配置（无 config 文件，http_concurrency 默认 8，FR-05-05 沿袭）
    When 按 A 打开「添加任务：HTTP(S)」对话框并检查并发字段
    Then 并发字段留空预填 8
    When 添加 "twelve-m.bin" 并开始下载
    Then 实际并发不超过 8 且并发连接面板出现 8 行活跃连接（12 MB 足够切满 8 段）
    And 下载最终完成且文件字节完整

  Scenario: 13-http-turbo-08 并发范围 1–128（CLI 与 config）
    Given fixture 文件 "five-m.bin" 且支持 Range 请求
    When 以命令行启动 "ezr <url> -c <cli_concurrency>"
    Then 启动行为为 "<behavior>"

    Examples:
      | url                                   | cli_concurrency | behavior                                    |
      | http://fixture.local/files/five-m.bin | 128             | 启动成功且实际并发不超过 128                  |
      | http://fixture.local/files/five-m.bin | 129             | 非零退出并提示并发数非法（应为 1–128 的整数） |
      | http://fixture.local/files/five-m.bin | 0               | 非零退出并提示并发数非法（应为 1–128 的整数） |

  Scenario: 13-http-turbo-08b config http_concurrency 边界与非法拒绝
    Given 配置文件设 [http] 表 http_concurrency = "<value>"
    When ezr 启动并添加 "five-m.bin" 下载
    Then 启动行为为 "<behavior>"（非法值拒绝启动沿 FR-02-35）

    Examples:
      | value | behavior                                                   |
      | 128   | 启动成功且实际并发不超过 128（fixture 连接数上限 128 生效） |
      | 129   | 启动失败并报错（FR-02-35 标量语义非法）                     |

  Scenario: 13-http-turbo-09 大文件端到端正确性（Writer 写流水线）
    Given fixture 文件 "big-100m.bin" 大小 100 MB（稀疏构造）且支持 Range 请求
    And 源文件的正确 SHA-256 摘要已预置为伴随文件 "big-100m.bin.sha256"
    When 以默认配置（MDM 移植引擎 · 并发 8）添加并下载该文件
    Then 完成后任务转「已完成」且显示「SHA-256 校验成功」（源与目标校验值一致）
    And 目标文件字节数为 104857600
    And fixture 请求日志显示多连接封闭区间分段交付（Writer 256 KiB batch/≤4 MiB 定位写/256 MiB 跑道不引入字节错位，FR-05-04）

  Scenario: 13-http-turbo-10 稀疏预分配磁盘占用随进度增长
    Given fixture 文件 "sparse-g.bin" 名义大小 1 GiB（流式生成、支持 Range）且下载器限速 global_max_download_speed = "64 MiB/s"
    When 以默认配置添加并下载至已接收约 <received> 后按 Space 暂停
    Then part 文件名义大小为 1073741824 字节且磁盘占用（du/st_blocks）显著小于名义大小（稀疏预分配生效：MDM MIN_SIZE 5 MiB 门槛、平台分支失败静默回退，FR-05-10/D88；磁盘占用随接收字节增长而非提前全额分配）
    And 按 Space 继续任务后下载最终完成且文件字节完整

    Examples:
      | received |
      | 64 MB    |

  Scenario: 13-http-turbo-11 kill -9 崩溃一致性续传
    Given fixture 文件 "eight-m.bin" 且源 SHA-256 摘要已预置为伴随文件
    When 以默认配置下载至已接收 ≥ 2 MB 后 kill -9 终止 ezr
    And 重新启动 ezr 并继续该任务
    Then sidecar 声称的已下载字节数 ≤ 磁盘 part 文件实际数据量（snapshot→flush→write 纪律，FR-05-09：flush 条件 = clamp(size/16,1 MiB,64 MiB) 或 30 s 且 fits 门控）
    And 任务从 sidecar 进度续传（不从头重下）且下载最终完成
    And 完成后「SHA-256 校验成功」且文件字节完整

  Scenario: 13-http-turbo-12 [退役，留空不重排]
    # 原「blocks 回退开关旧行为」场景随 blocks 引擎退役（v1.63/族A 操作者裁决 20261127，
    # phase-05 v1.1/D91：scheduler 键整键退役，blocks 模式不复存在；v1.3 MDM 完全移植后
    # HTTP 引擎仅存 MDM 移植单形态，D95）。
    # 场景序号保留追溯、不重排（修订纪律：删除类修订三层同步口径）。
    # 本场景无可执行步骤：QA 对应用例 QA-HT-12 同步退役（qa/13-http-turbo-qa.md 注记）。

  Scenario: 13-http-turbo-13 config 键面严格校验
    # v1.3/D97① 修订：scheduler 退役沿袭（v1.63/D91）；block_size_http/throttle_burst_secs/
    # throttle_sleep_ms/dns_race_grace_ms 四键整键退役——config 已知表含任一键（任意值）
    # → 拒绝启动并报错（未知键直接废弃，FR-02-35/D91 口径推广）；min_split_kib 默认 12
    # （D97②），合法域正整数不变；顶层平铺旧格式（无表头）整体忽略口径不变
    # （01-persistence-config-12）。
    Given 配置文件预置 <config_part>
    When ezr 启动
    Then 启动行为为 "<behavior>"

    Examples:
      | config_part                                                | behavior                                   |
      | [http] min_split_kib = 12                                  | 启动成功（默认口径）                       |
      | [http] min_split_kib = 64                                  | 启动成功（合法非默认值）                   |
      | [http] min_split_kib = 0                                   | 启动失败并报错（min_split_kib 非正数，FR-02-35） |
      | [http] min_split_kib = 12 与 block_size_http = "1 MB"      | 启动失败并报错（block_size_http 键退役 = 未知键，D97①/FR-02-35） |
      | [http] min_split_kib = 12 与 throttle_burst_secs = 1.0     | 启动失败并报错（throttle_burst_secs 键退役 = 未知键，D97①） |
      | [http] min_split_kib = 12 与 throttle_sleep_ms = 20        | 启动失败并报错（throttle_sleep_ms 键退役 = 未知键，D97①） |
      | [http] min_split_kib = 12 与 dns_race_grace_ms = 800       | 启动失败并报错（dns_race_grace_ms 键退役 = 未知键，D97①） |
      | [http] min_split_kib = 12 与 scheduler = "any"             | 启动失败并报错（scheduler 键退役沿袭，D91） |

  Scenario: 13-http-turbo-13b [退役，留空不重排]
    # 原「调度模式烘焙于任务启动（B 类快照）」场景随 scheduler 键退役失去 premise
    # （v1.63/族A 操作者裁决 20261127，phase-05 v1.1/D91）；v1.3 沿袭退役不重排。
    # 场景序号保留追溯、不重排（修订纪律：删除类修订三层同步口径）；
    # B 类快照键语义（任务启动烘焙、热加载只影响新任务）由 features/12-reload-config.feature
    # 场景 07（QA-RC-07：http_concurrency/min_split_kib/max_retries）继续承载
    # （v1.3：block_size_http 臂随键退役移出，D97①）。
    # 本场景无可执行步骤：QA 对应用例 QA-HT-13b 同步退役（qa/13-http-turbo-qa.md 注记）。

  Scenario: 13-http-turbo-14 断点续传（封闭区间 + 64 B 覆叠校验 + 平铺台账）
    Given fixture 文件 "eight-m.bin" 支持且请求区间可被记录
    When 以默认配置下载至进度 ≥ <progress> 后按 Space 暂停
    And 按 Space 继续任务
    Then 续传请求为封闭区间 Range bytes=<resume_from>-<end-1> 且 resume_from = 首个未提交字节 − 64（覆叠重取：前 64 B 与盘上字节逐字节比对通过后续传，MDM/IDM resume overlap check，FR-05-04/FR-05-08）
    And 已提交字节零重复写入（磁盘内容不回退；claim 只授予未认领部分）
    And 列表进度从 sidecar 声称值继续（平铺台账 (start,cursor,end) 恢复，D98）
    And 下载最终完成且文件字节完整

    Examples:
      | progress |
      | 30%      |

  Scenario: 13-http-turbo-15 服务器不支持 Range 降级单流（探测流吸收收口）
    Given fixture 文件 "norange-8m.bin" 忽略 Range 头（一律 200 全量应答）
    When 以默认配置添加并开始下载
    Then Range 学习态转 NO（200 应答）且其余连接的分段请求静默退出不写入（NoRanges 臂，FR-05-03/FR-05-12）
    And 探测连接经 absorb_next/make_single 以既有数据流单连接完成整文件（探测流不中断、不重发全量请求，MDM keep_reading/make_single）
    And toast 显示「⚠ 服务器不支持断点续传，单线程下载: <name>」（ezr 降级文案沿袭）
    And 任务完成且文件字节完整
    And 该任务暂停后继续时从头重新下载（无 Range 下载无台账可续，state 不保存，FR-05-12/D98；原「保留断点稍后重试」口径退役 D87②）

  Scenario: 13-http-turbo-16 启动坡道（INITIAL_PIECE 64 KiB/连接）
    Given fixture 文件 <file> 名义大小 <size> 且支持 Range 请求
    When 以并发数 8 添加并开始下载
    Then 初始并发连接数 = min(8, ⌈<size> ÷ 64 KiB⌉)（至少 1，INITIAL_PIECE 坡道，FR-05-01；fixture 记录的并发到达数吻合）
    And 后续经 planner 切分并发回升至上限 <conns_cap>（段仍可下探 min_split 12 KiB）
    And 下载最终完成且文件字节完整

    Examples:
      | file         | size    | conns_cap |
      | quarter-m.bin | 256 KB | 4         |
      | twelve-m.bin | 12 MB   | 8         |

  Scenario: 13-http-turbo-17 引擎级 429 Retry-After 重试
    Given fixture 文件 "retry-429.bin" 支持且可对指定请求返回 429 并携带 Retry-After: <retry_after>
    When 以默认配置添加并开始下载
    Then 收到 429 的连接按 Retry-After 等待后重试（Throttled 分类，FR-05-03；封顶 5 分钟）
    And 任务保持「下载中」不转「已失败」（429 = Transient 非 link_dead）
    And 下载最终完成且文件字节完整

    Examples:
      | retry_after |
      | 1 s         |

  Scenario: 13-http-turbo-18 慢连接剔除重连（shed）
    Given fixture 文件 "shed-20m.bin" 支持且可对指定连接限速至 <slow_rate>（其余连接全速）
    When 以并发数 4、不限速配置添加并开始下载
    Then 慢连接在应答运行 ≥ 2 s（SHED_GRACE）且净速 < 单连接可达速度 ÷ 4、剩余工作量 > 2 s 时被剔除重连（SlowConnection 非失败、不耗重试计数，FR-05-03/FR-05-05；fixture 记录慢连接段区间的新请求）
    And 并发面板该连接行回「正在连接...」后继续接收（D99②）
    And 下载最终完成且文件字节完整

    Examples:
      | slow_rate |
      | 64 KiB/s  |

  Scenario: 13-http-turbo-19 主机连接上限学习与退役
    Given fixture 文件 "hostcap-8m.bin" 支持且对第 <cap> 条之后的连接一律以 403 拒绝（其余连接正常供数）
    When 以并发数 <concurrency> 添加并开始下载
    And 任务完成后再以并发数 <concurrency> 添加同 host 第二个任务
    Then 第一任务下载期间被拒连接在学习表计数满 2 次后按上限 <cap> 收窄并发（NEEDED=2，FR-05-07）
    And 被拒连接转 Retired{host, cap} 退场且不再耗重试（并发面板活跃行口径下不显示，D99④）
    And 两任务下载均最终完成且文件字节完整

    Examples:
      | cap |
      | 2   |

  Scenario: 13-http-turbo-20 网页应答拒收（GotWebPage）
    Given fixture 提供地址 "<url>" 应答 Content-Type text/html 的 HTML 页面（非命名文件）
    When 添加该地址的下载任务并等待结果
    Then 任务转「已失败」且失败原因属 URL 失效类（GotWebPage：网页而非文件，不落盘，FR-05-02/FR-05-11/D100）
    And 目标位置不产生任何 part 文件或台账文件
    And 列表行显示「不自动重试」且任务不占用下载槽位

    Examples:
      | url                              |
      | http://fixture.local/page/app.html |
