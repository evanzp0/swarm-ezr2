# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   01-retry-backoff-01 失败原因分类展示
#   01-retry-backoff-02 有下载进展的失败计数重置为 1
#   01-retry-backoff-03 无进展失败计数累加至达上限
#   01-retry-backoff-04 指数退避 8s 到 16s 到 32s 到 60s 封顶
#   01-retry-backoff-05 Retry-After 不超过 60s 时优先采用
#   01-retry-backoff-06 Retry-After 超过 60s 时回退指数退避
#   01-retry-backoff-07 语义性 4xx 停等不自动重试
#   01-retry-backoff-08 408/429/5xx 与网络错误按退避自动重试
#   01-retry-backoff-09 磁盘空间预检不足停等
#   01-retry-backoff-10 auto_retry=false 时失败一律停等
Feature: 01-retry-backoff 失败分类 · 重试计数与退避

  期号 01。依据 project/mission/phase-01.md FR-01-40~44 与 D7 定案。计数连续性规则：
  有进展重置为 1、无进展累加；退避 8→16→32→60s 封顶；Retry-After ≤60s 优先。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件

  Scenario: 01-retry-backoff-01 失败原因分类展示
    Given fixture 使任务 "<file>" 以 "<fault>" 方式失败
    When 任务转「已失败」
    Then 详情失败原因行与 toast 显示 "<reason>"

    Examples:
      | file        | fault                     | reason                          |
      | neterr.bin  | 连接被拒绝                | 网络错误（连接失败）           |
      | neterr.bin  | 连接超时                  | 网络错误（超时）               |
      | neterr.bin  | 连接被重置                | 网络错误（连接被重置）         |
      | http404.bin | 返回 404                  | HTTP 404                       |
      | http503.bin | 返回 503                  | HTTP 503                       |
      | diskful.bin | 保存目录无写入权限        | 磁盘错误（无写权限）           |

  Scenario: 01-retry-backoff-02 有下载进展的失败计数重置为 1
    Given fixture 在文件 "<file>" 传输中先断连一次、重试后传输一段再断连
    When 任务经历第二次失败
    Then 列表行显示重试计数 "1/<max_retries>"（有进展故重置）

    Examples:
      | file        | max_retries |
      | flappy1.bin | 5           |

  Scenario: 01-retry-backoff-03 无进展失败计数累加至达上限
    Given fixture 对文件 "<file>" 的连接一律立即失败（每次尝试均无进展）
    When 任务连续自动重试
    Then 列表行计数依次累加（2/5、3/5、4/5、5/5）
    And 达到 5/5 后停止自动重试、释放槽位且列表行显示「已达上限」
    And 按 R 可重新排队（计数重置为 1）

    Examples:
      | file         |
      | deadport.bin |

  Scenario: 01-retry-backoff-04 指数退避 8s 到 16s 到 32s 到 60s 封顶
    Given fixture 对文件 "<file>" 持续产生无进展失败
    When 自动重试连续发生
    Then 列表行倒计时依次显示 <seq> 秒并在 60 封顶
    And 每次倒计时结束自动重试（回等待中→断点续传）

    Examples:
      | file         | seq              |
      | backoff1.bin | 8, 16, 32, 60, 60 |

  Scenario: 01-retry-backoff-05 Retry-After 不超过 60s 时优先采用
    Given fixture 以 503 与响应头 "Retry-After: <retry_after>" 应答文件 "<file>"
    When 任务失败进入自动重试
    Then 列表行倒计时显示 <retry_after> 秒（优先于指数退避）

    Examples:
      | file         | retry_after |
      | polite503.bin | 10          |
      | polite503.bin | 60          |

  Scenario: 01-retry-backoff-06 Retry-After 超过 60s 时回退指数退避
    Given fixture 以 503 与响应头 "Retry-After: <retry_after>" 应答文件 "<file>"
    When 任务失败进入自动重试
    Then 倒计时按指数退避序列进行（不采用 120 秒）

    Examples:
      | file          | retry_after |
      | distant503.bin | 120         |

  Scenario: 01-retry-backoff-07 语义性 4xx 停等不自动重试
    Given fixture 对文件 "<file>" 返回 <status>
    When 任务失败
    Then 任务转「已失败」且列表行显示「不自动重试」
    And 失败原因显示 "HTTP <status>" 且任务不占槽位、无自动重试倒计时
    And 按 R 可手动重试

    Examples:
      | file        | status |
      | forbidden.bin | 403  |
      | missing.bin   | 404  |

  Scenario: 01-retry-backoff-08 408/429/5xx 与网络错误按退避自动重试
    Given fixture 使任务以 "<fault>" 失败
    When 任务失败且未达重试上限
    Then 任务按指数退避自动重试（列表行显示倒计时）且继续占用槽位

    Examples:
      | fault        |
      | 返回 408     |
      | 返回 429     |
      | 返回 500     |
      | 连接被重置   |

  Scenario: 01-retry-backoff-09 磁盘空间预检不足停等
    Given 保存目录为受限小容量目录（如 64 MB tmpfs）且待下载文件 "<file>" 为 100 MB（剩余需下载量大于可用空间）
    When 任务获得槽位并开始预检
    Then 任务转「已失败」且失败原因显示「磁盘空间不足」
    And 不自动重试、释放槽位且按 R 可手动重试

    Examples:
      | file          |
      | big-100m.bin  |

  Scenario: 01-retry-backoff-10 auto_retry=false 时失败一律停等
    Given 配置文件设 auto_retry = false
    When 任务以网络错误（或 408/429/5xx）失败
    Then 任务转「已失败」且列表行显示「不自动重试」、无倒计时
    And 按 R 可手动重试（重试后再次失败仍停等）
