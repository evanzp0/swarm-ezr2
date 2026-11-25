# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   13-http-turbo-01 默认动态分段引擎（裸 GET + 开区间并发 + 动态分段行）
#   13-http-turbo-02 自适应分段切分维持并发
#   13-http-turbo-03 慢连接段抢占
#   13-http-turbo-04 瞬态断连原地吸收（不转任务级失败）
#   13-http-turbo-05 原地重试耗尽升级任务级退避
#   13-http-turbo-06 限速贴住设定值（throttle 比例等待）
#   13-http-turbo-07 默认并发 8 与对话框回退
#   13-http-turbo-08 并发范围 1–128（CLI 与 config）
#   13-http-turbo-08b config http_concurrency 边界与非法拒绝
#   13-http-turbo-09 大文件端到端正确性（写合并 + 写流水线）
#   13-http-turbo-10 稀疏预分配磁盘占用随进度增长
#   13-http-turbo-11 kill -9 崩溃一致性续传
#   13-http-turbo-12 blocks 回退开关旧行为
#   13-http-turbo-13 config 新键严格校验
#   13-http-turbo-13b 调度模式烘焙于任务启动（B 类快照）
#   13-http-turbo-14 动态模式断点续传
#   13-http-turbo-15 服务器不支持 Range 降级单流
Feature: 13-http-turbo HTTP 引擎提速（P0–P2：动态分段调度 · 磁盘流水线 · 补丁层修复）

  期号 05。依据 project/mission/phase-05.md FR-05-01~13、D83–D89 与操作者 20261125
  裁决（实现 ezr-http-dw-tunning.md 的 P0/P1/P2；「DEFAULT_BLOCK_SIZE_HTTP 提到 4 MB」
  去除、直接下限 256 KiB；其余按方案；提速为判断依据；ezr 不做旧版本兼容沿 v1.45/FR-02-35）。
  设计权威 = project/ezr-http-dw-tunning.md。P3 增益层（BDP 自适应/并发预热/慢连接剔除/
  主机连接上限/切分粒度持续校准/h2 降级）不在本期。
  回退开关：http.scheduler = "dynamic"（默认）| "blocks"（旧引擎一键回退）。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件

  Scenario: 13-http-turbo-01 默认动态分段引擎（裸 GET + 开区间并发 + 动态分段行）
    Given fixture 文件 "<file>" 大小 <size> 且支持 Range 请求
    When 以默认配置添加并开始下载
    Then fixture 记录的首个请求为不带 Range 头的完整 GET（探测即数据流，FR-05-07/D87）
    And 其余连接以开区间 Range（bytes=<from>- 形态，无结束偏移）并发加入（fixture 请求日志）
    And 详情分块行显示动态分段口径（"<size> · 动态分段" 形态，随 <已下载>/<总字节> 推进，FR-05-13/D83）
    And 下载最终完成且文件字节完整

    Examples:
      | file        | size    |
      | ten-m.bin   | 10.0 MB |
      | big-100m.bin | 100 MB |

  Scenario: 13-http-turbo-02 自适应分段切分维持并发
    Given fixture 文件 <file> 支持且可记录各连接请求区间
    When 以并发数 <concurrency> 添加并开始下载
    Then 实际活跃连接峰值 ≥ <min_conns> 且各连接持段互不重叠（并发连接面板与 fixture 日志）
    And 实际并发不超过 <concurrency>
    And 下载完成后详情分块行显示 "<size> · 动态分段"（D83）且文件字节完整

    Examples:
      | file        | concurrency | min_conns |
      | one-m.bin   | 8           | 2         |
      | twelve-m.bin | 8          | 8         |

  Scenario: 13-http-turbo-03 慢连接段抢占
    Given fixture 文件 "stall-20m.bin" 支持且可对指定连接延迟应答 ≥ <stall>
    When 以并发数 4 添加并开始下载
    Then 被延迟连接分配段中未认领的字节在 <stall> 内被其他连接以开区间 Range 接管（fixture 出现段内新请求，preempt_after = 2×TTFB 下限 20 ms，FR-05-06）
    And 任务保持「下载中」不转「已失败」
    And 下载最终完成且文件字节完整

    Examples:
      | stall |
      | 2 s   |

  Scenario: 13-http-turbo-04 瞬态断连原地吸收（不转任务级失败）
    Given fixture 文件 "eight-m.bin" 且传输中对已建立连接断连 <drops> 次（重连后从剩余区间继续供数）
    When 以默认配置添加并开始下载
    Then 任务全程保持「下载中」且不出现「已失败」、重试倒计时与失败 toast（原地重试吸收，FR-05-04）
    And fixture 记录到同字节区间的重新请求（原地重试）
    And 下载最终完成且文件字节完整

    Examples:
      | drops |
      | 2     |

  Scenario: 13-http-turbo-05 原地重试耗尽升级任务级退避
    Given fixture 文件 "flappy-always.bin" 的连接一律在传输首段即断连（原地重试持续失败）
    When 以默认配置添加并开始下载（auto_retry = true）
    Then 累计 6 次连接尝试仍失败后任务转「已失败」（原地重试 5 次耗尽：首次立即 + 1/2/4/8 s 退避，D86）
    And 列表行按既有指数退避自动重试（8→16→32→60 s 封顶）且倒计时结束原地续传
    And auto_retry = false 时同 fixture 直接「已失败」停等且无倒计时

    Examples:
      | file               |
      | flappy-always.bin |

  Scenario: 13-http-turbo-06 限速贴住设定值（throttle 比例等待）
    Given 配置文件设 [common] 表 global_max_download_speed = "<speed>"
    When 下载文件 "<file>" 至传输稳定（≥ 3 秒滑动窗）
    Then 全局下载速度稳定在 <speed> ±10%（20 ms/chunk 睡眠下限退役，FR-05-02；此前 >6.4 MB/s 设定值不可达）

    Examples:
      | speed    | file         |
      | 8 MiB/s  | big-100m.bin |
      | 20 MiB/s | big-100m.bin |

  Scenario: 13-http-turbo-07 默认并发 8 与对话框回退
    Given 默认配置（无 config 文件，DEFAULT_HTTP_CONCURRENCY = 8，FR-05-05）
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
      | url                              | cli_concurrency | behavior                                       |
      | http://fixture.local/files/five-m.bin | 128        | 启动成功且实际并发不超过 128                     |
      | http://fixture.local/files/five-m.bin | 129        | 非零退出并提示并发数非法（应为 1–128 的整数）    |
      | http://fixture.local/files/five-m.bin | 0          | 非零退出并提示并发数非法（应为 1–128 的整数）    |

  Scenario: 13-http-turbo-08b config http_concurrency 边界与非法拒绝
    Given 配置文件设 [http] 表 http_concurrency = "<value>"
    When ezr 启动并添加 "five-m.bin" 下载
    Then 启动行为为 "<behavior>"（非法值拒绝启动沿 FR-02-35）

    Examples:
      | value | behavior                                    |
      | 128   | 启动成功且实际并发不超过 128（fixture 连接数上限 128 生效） |
      | 129   | 启动失败并报错（FR-02-35 标量语义非法）       |

  Scenario: 13-http-turbo-09 大文件端到端正确性（写合并 + 写流水线）
    Given fixture 文件 "big-100m.bin" 大小 100 MB（稀疏构造）且支持 Range 请求
    And 源文件的正确 SHA-256 摘要已预置为伴随文件 "big-100m.bin.sha256"
    When 以默认配置（动态分段 · 并发 8）添加并下载该文件
    Then 完成后任务转「已完成」且显示「SHA-256 校验成功」（源与目标校验值一致）
    And 目标文件字节数为 104857600
    And fixture 请求日志显示多连接分段交付（写合并/写流水线不引入字节错位，FR-05-01/09）

  Scenario: 13-http-turbo-10 稀疏预分配磁盘占用随进度增长
    Given fixture 文件 "sparse-g.bin" 名义大小 1 GiB（流式生成、支持 Range）且下载器限速 global_max_download_speed = "64 MiB/s"
    When 以默认配置添加并下载至已接收约 <received> 后按 Space 暂停
    Then part 文件名义大小为 1073741824 字节且磁盘占用（du/st_blocks）显著小于名义大小（稀疏预分配生效，FR-05-10；磁盘占用随接收字节增长而非提前全额分配）
    And 按 Space 继续任务后下载最终完成且文件字节完整

    Examples:
      | received |
      | 64 MB    |

  Scenario: 13-http-turbo-11 kill -9 崩溃一致性续传
    Given fixture 文件 "eight-m.bin" 且源 SHA-256 摘要已预置为伴随文件
    When 以默认配置下载至已接收 ≥ 2 MB 后 kill -9 终止 ezr
    And 重新启动 ezr 并继续该任务
    Then sidecar 声称的已下载字节数 ≤ 磁盘 part 文件实际数据量（条件 sync_data 前置保证，FR-05-11）
    And 任务从 sidecar 进度续传（不从头重下）且下载最终完成
    And 完成后「SHA-256 校验成功」且文件字节完整

  Scenario: 13-http-turbo-12 blocks 回退开关旧行为
    Given 配置文件设 [http] 表 scheduler = "blocks"
    And fixture 文件 "<file>" 支持且可记录各请求 Range 头
    When 以默认其余配置添加并开始下载
    Then fixture 记录的首个请求携带 Range 头 "bytes=0-" 且分块请求为封闭区间（bytes=<start>-<end> 形态）
    And 详情分块行按 "x/<blocks> · 1 MB/块" 既有口径显示（blocks 模式块模型不变，FR-05-03/D83）
    And 下载最终完成且文件字节完整

    Examples:
      | file      | blocks |
      | ten-m.bin | 10     |

  Scenario: 13-http-turbo-13 config 新键严格校验
    Given 配置文件设 [http] 表 scheduler = "<sched>" 与 min_split_kib = "<split>"
    When ezr 启动
    Then 启动行为为 "<behavior>"

    Examples:
      | sched   | split | behavior                                        |
      | dynamic | 256   | 启动成功（默认口径）                             |
      | blocks  | 256   | 启动成功（blocks 引擎回退）                      |
      | DYNAMIC | 512   | 启动成功且动态调度生效（大小写不敏感，D84）      |
      | turbo   | 256   | 启动失败并报错（scheduler 非法值，FR-02-35）     |
      | dynamic | 0     | 启动失败并报错（min_split_kib 非正数，FR-02-35） |

  Scenario: 13-http-turbo-13b 调度模式烘焙于任务启动（B 类快照）
    Given 配置文件设 [http] 表 scheduler = "blocks" 且任务 "ten-m.bin" 下载中
    When 外部改写 config scheduler = "dynamic" 并按 Ctrl+R 热加载
    Then 运行中任务保持 blocks 请求形态（封闭区间分块，不切动态，FR-05-12/D84）
    And 热加载后新添加任务按 dynamic 形态（首个请求无 Range 头）
    And 两任务均最终完成且字节完整（成功 toast 沿 v1.57 固定文案 ✓ 配置已重新加载）

  Scenario: 13-http-turbo-14 动态模式断点续传
    Given fixture 文件 "eight-m.bin" 支持且请求区间可被记录
    When 以默认配置下载至进度 ≥ <progress> 后按 Space 暂停
    And 按 Space 继续任务
    Then 续传请求从首个未提交字节起始（开区间 Range；已提交字节零重复写入，磁盘内容不回退，FR-05-07/D87）
    And 列表进度从 sidecar 声称值继续（非从 0）
    And 下载最终完成且文件字节完整

    Examples:
      | progress |
      | 30%      |

  Scenario: 13-http-turbo-15 服务器不支持 Range 降级单流
    Given fixture 文件 "norange-8m.bin" 忽略 Range 头（一律 200 全量应答）
    When 以默认配置添加并开始下载
    Then 其余连接的分段请求收到 200 后静默退出且不写入（不产生错位字节）
    And toast 显示「⚠ 服务器不支持断点续传，单线程下载: <name>」（既有降级语义，D87）
    And 任务以首连接单流完成且文件字节完整
    And 该任务暂停后继续时转「服务器本次未支持断点续传（Range），保留断点稍后重试」失败口径（有进度续传任务）
