# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   01-slots-queue-01 六态状态机与页签口径
#   01-slots-queue-02 下载中与待自动重试失败任务占用槽位
#   01-slots-queue-03 校验中占槽不释放
#   01-slots-queue-04 达上限与停等失败释放槽位
#   01-slots-queue-05 七任务五槽位排队与递补
#   01-slots-queue-06 U/J 调整排队优先级
#   01-slots-queue-07 排队行与下载槽位计数显示
#   01-slots-queue-08 Space 暂停下载保存断点并释放槽位
#   01-slots-queue-09 Space 继续有空槽从断点续传
#   01-slots-queue-10 Space 继续遇满槽提示并进入等待队列
#   01-slots-queue-11 Space 暂停等待中任务退出队列
#   01-slots-queue-12 R 手动重试失败任务计数重置并续传
#   01-slots-queue-13 R 对已达上限与停等任务重新排队
#   01-slots-queue-14 Space 暂停失败任务挂起自动重试（v1.6/FR-01-92）
#   01-slots-queue-15 Space 恢复已暂停失败任务重新排队（v1.6/FR-01-92）
Feature: 01-slots-queue 状态机 · 下载槽位与排队调度

  期号 01。依据 project/mission/phase-01.md FR-01-30~34 与 D12 定案（校验中占槽）。
  槽位默认 5（可配置 download_slots）；占用 = 下载中 ∪ 校验中 ∪ 待自动重试失败 ∪ 已获槽等待。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件
    And 配置下载槽位数为默认 5

  Scenario: 01-slots-queue-01 六态状态机与页签口径
    Given 任务先后经历等待中、下载中、已暂停、校验中、已失败、已完成各状态
    Then 全程未出现「连接中」状态（下载中任务从等待中直接开始）
    And 「正在下载」页签含等待/下载/暂停/校验/失败任务
    And 「已完成」页签仅含已完成任务

  Scenario: 01-slots-queue-02 下载中与待自动重试失败任务占用槽位
    Given 已有 <active> 个任务处于下载中
    And 其中 1 个因网络错误失败且处于自动重试倒计时中
    When 查看任务队列栏
    Then 下载槽位显示 <active>/<active>（失败倒计时任务仍占槽）
    And 新添加任务处于等待中排队

    Examples:
      | active |
      | 3      |

  Scenario: 01-slots-queue-03 校验中占槽不释放
    Given 提供正确校验值的任务 "<file>" 即将下载完成且槽位配置为 <slots>
    And 此时另有 <others> 个任务在下载中且等待队列非空
    When 该任务下载完成转入「校验中」
    Then 下载槽位仍显示满载（校验中不释放槽位）
    And 等待任务未获递补
    When 校验成功转入「已完成」
    Then 槽位释放且队首等待任务开始下载

    Examples:
      | file        | slots | others |
      | verify1.bin | 3     | 2      |

  Scenario: 01-slots-queue-04 达上限与停等失败释放槽位
    Given 槽位配置为 <slots> 且全部被下载中任务占用、等待队列非空
    When 其中 1 个任务连续无进展失败达重试上限（或转入不自动重试停等）
    Then 该任务释放槽位且队首等待任务立即递补开始下载

    Examples:
      | slots |
      | 2     |

  Scenario: 01-slots-queue-05 七任务五槽位排队与递补
    When 依次添加 <total> 个下载任务（槽位 <slots>）
    Then 前 <slots> 个任务获得槽位开始下载
    And 第 6、7 个任务显示「排队第 1 位」「排队第 2 位 · 等待空闲下载槽位」
    And 下载槽位显示 <slots>/<slots>
    When 暂停队首下载任务
    Then 队首等待任务（原第 6 个）递补开始下载

    Examples:
      | total | slots |
      | 7     | 5     |

  Scenario: 01-slots-queue-06 U/J 调整排队优先级
    Given 槽位满载且等待队列含任务 T1（队首）、T2
    When 选中 T1 按 J 下移一位
    Then T2 变为队首显示「排队第 1 位」
    When 空出槽位
    Then T2 先于 T1 获得槽位开始下载

  Scenario: 01-slots-queue-07 排队行与下载槽位计数显示
    Given 槽位 <slots> 全部占用且存在等待任务
    Then 等待任务行显示「排队第 N 位 · 等待空闲下载槽位」
    And 任务队列栏右侧显示「下载槽位 n/<slots>」且满载时为黄色加粗
    And 未满载时显示实际占用数且为常规样式

    Examples:
      | slots |
      | 5     |

  Scenario: 01-slots-queue-08 Space 暂停下载保存断点并释放槽位
    Given 任务 "<file>" 处于下载中且进度 ≥ <progress>
    When 按 Space
    Then 任务转「已暂停」且断点已保存（sidecar 存在）
    And 槽位释放且队首等待任务递补

    Examples:
      | file       | progress |
      | pause1.bin | 20%      |

  Scenario: 01-slots-queue-09 Space 继续有空槽从断点续传
    Given 任务 "<file>" 处于已暂停且存在空闲槽位
    When 按 Space
    Then 任务转「下载中」并从断点续传（进度不回退）

    Examples:
      | file       |
      | pause1.bin |

  Scenario: 01-slots-queue-10 Space 继续遇满槽提示并进入等待队列
    Given 任务处于已暂停且下载槽位已满载
    When 按 Space
    Then 出现「无空闲下载槽位（<slots>/<slots>）」toast
    And 任务转入等待中并显示排队位次

    Examples:
      | slots |
      | 5     |

  Scenario: 01-slots-queue-11 Space 暂停等待中任务退出队列
    Given 任务处于等待中且显示排队位次（已获槽位或排队中）
    When 按 Space
    Then 任务转「已暂停」且退出等待队列（不再显示排队行）
    And 已获槽位（如有）被释放并递补给队首等待任务

  Scenario: 01-slots-queue-12 R 手动重试失败任务计数重置并续传
    Given 任务 "<file>" 因网络错误失败且重试计数为 <count>
    When 按 R
    Then 重试计数重置为 1 且任务回到等待中排队
    And 获得槽位后从断点续传

    Examples:
      | file       | count |
      | retry1.bin | 3     |

  Scenario: 01-slots-queue-13 R 对已达上限与停等任务重新排队
    Given 存在显示「已达上限」的任务与显示「不自动重试」停等的任务各一
    When 分别按 R
    Then 两个任务均重新回到等待中排队
    And 获得槽位后从断点续传（不停留在失败态）

  Scenario: 01-slots-queue-14 Space 暂停失败任务挂起自动重试（v1.6/FR-01-92）
    Given 任务 "<file>" 处于失败态且倒计时进行中（自动重试待发）
    When 按空格
    Then 任务转为「已暂停（失败）」：自动重试倒计时清除，不再到点重发
    And 错误信息保留可见（详情/列表错误行不因暂停丢失）
    And 任务不占用下载槽位，重启后仍保持该状态

    Examples:
      | file       |
      | retry1.bin |

  Scenario: 01-slots-queue-15 Space 恢复已暂停失败任务重新排队（v1.6/FR-01-92）
    Given 任务 "<file>" 处于「已暂停（失败）」态（01-slots-queue-14 之后）
    When 按空格（或按 R）
    Then 任务重新排队（计数重置、断点续传口径同 01-slots-queue-12）
    And 获得槽位后从断点续传

    Examples:
      | file       |
      | retry1.bin |
