# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   01-resume-sidecar-01 sidecar 伴随目标文件创建且内容齐全
#   01-resume-sidecar-02 sidecar 原子写任意时刻 kill 不损坏
#   01-resume-sidecar-03 暂停恢复从断点续传
#   01-resume-sidecar-04 失败重试从断点续传
#   01-resume-sidecar-05 kill 后重启从断点恢复直至完成
#   01-resume-sidecar-06 重启恢复全部任务列表与历史
#   01-resume-sidecar-07 续传一致性失效自动从头重下且不计失败
#   01-resume-sidecar-08 一致性连续 3 次失效转停等
#   01-resume-sidecar-09 一致性停等按 R 重置失效计数
#   01-resume-sidecar-10 .downloading 扩展名随状态生灭
#   01-resume-sidecar-11 仅删除任务保留文件与 sidecar
#   01-resume-sidecar-12 删除任务和文件全部删除
#   01-resume-sidecar-13 重加同 URL 同路径任务自动接续断点
Feature: 01-resume-sidecar 断点续传与 sidecar 元数据

  期号 01。依据 project/mission/phase-01.md FR-01-20~26 与 D10/D11 定案。sidecar 文件为
  伴随目标文件的 <目标文件>.ezr，原子写；续传一致性以最终 URL/ETag/Last-Modified/大小为准。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件

  Scenario: 01-resume-sidecar-01 sidecar 伴随目标文件创建且内容齐全
    When 添加支持续传的文件 "<file>" 并开始下载
    Then 目标文件旁存在 sidecar 文件 "<file>.ezr"
    And sidecar 内容涵盖：原始与最终 URL、文件大小、ETag、Last-Modified、块大小、总块数、
      各块状态（含部分下载块块内字节）、已下载字节、校验期望值（如有）与任务元信息

    Examples:
      | file       |
      | eight-m.bin |

  Scenario: 01-resume-sidecar-02 sidecar 原子写任意时刻 kill 不损坏
    When 下载文件 "<file>" 至进度 ≥ <progress>
    And 在进度推进期间连续发送 kill -9 共 <kills> 次且每次重启后续传
    Then 每次重启后 sidecar 均完整可读且从未出现损坏
    And 最终下载完成且文件字节完整

    Examples:
      | file       | progress | kills |
      | eight-m.bin | 30%      | 3     |

  Scenario: 01-resume-sidecar-03 暂停恢复从断点续传
    Given 文件 <file_size> 支持断点续传且请求区间可被记录
    When 下载至进度 ≥ <progress> 后按 Space 暂停
    And 按 Space 继续任务
    Then 任务从断点续传且 fixture 请求不含已下载块区间
    And 已下载块的字节保持有效（最终文件字节完整）

    Examples:
      | file_size | progress |
      | 8 MB      | 40%      |

  Scenario: 01-resume-sidecar-04 失败重试从断点续传
    Given fixture 在文件 "<file>" 传输至约 <progress> 时模拟断连
    When 下载触发失败并等待自动重试
    Then 重试从断点续传且列表行进度不回退
    And 下载最终完成且文件字节完整

    Examples:
      | file       | progress |
      | eight-m.bin | 50%      |

  Scenario: 01-resume-sidecar-05 kill 后重启从断点恢复直至完成
    When 下载文件至进度 ≥ <progress>
    And 发送 kill -9 终止进程
    And 重新启动 ezr
    Then 任务列表完整恢复且该任务回到「等待中」按序排队
    And 获得槽位后从断点续传（已下载块不重传）
    And 下载最终完成且文件字节完整

    Examples:
      | progress |
      | 30%      |

  Scenario: 01-resume-sidecar-06 重启恢复全部任务列表与历史
    Given 任务列表含状态各异的任务：下载中、等待中、已暂停、已失败、已完成
    And 其中下载中的任务进度 ≥ <progress>
    When 发送 kill -9 后重新启动 ezr
    Then 全部任务（含已完成与已失败历史）按原列表顺序恢复
    And 原下载中任务回到「等待中」排队、其余状态保持

    Examples:
      | progress |
      | 20%      |

  Scenario: 01-resume-sidecar-07 续传一致性失效自动从头重下且不计失败
    Given 任务已下载至进度 ≥ <progress> 后暂停
    And fixture 使 "<invariant>" 相比 sidecar 记录发生变化（或不再返回该头）
    When 按 Space 继续任务
    Then 出现「服务器内容已更新，已从头重新下载」toast
    And 任务转入「等待中」重新排队且不计失败、重试计数不变
    And 获得槽位后从头下载（原 sidecar 作废）

    Examples:
      | progress | invariant       |
      | 30%      | ETag            |
      | 30%      | Last-Modified   |
      | 30%      | 文件大小        |
      | 30%      | 最终 URL        |
      | 30%      | 全部一致性头（服务器不再返回）|

  Scenario: 01-resume-sidecar-08 一致性连续 3 次失效转停等
    Given 同一任务已连续 <strikes> 次发生续传一致性失效
    When 第 <strikes> 次失效发生
    Then 任务转「已失败」且失败原因为「服务器内容持续变化」
    And 不自动重试、释放槽位且可按 R 手动重试

    Examples:
      | strikes |
      | 3       |

  Scenario: 01-resume-sidecar-09 一致性停等按 R 重置失效计数
    Given 任务因「服务器内容持续变化」停等且失效计数为 3
    When fixture 恢复内容一致后按 R
    Then 失效计数重置且任务重新排队、重新探测后正常续传
    And 此后需再连续 3 次一致性失效才会再次停等

  Scenario: 01-resume-sidecar-10 .downloading 扩展名随状态生灭
    Given 下载项 "<file>" 的校验期望值来源为 "<checksum_source>"
    When 任务处于「下载中」
    Then 保存目录中目标文件名带 .downloading 扩展名
    And 不存在裸名目标文件
    When 任务转入「已完成」
    Then 目标文件以裸名 "<file>" 存在且 .downloading 扩展名已移除
    And 对应 sidecar 已删除

    Examples:
      | file       | checksum_source     |
      | five-m.bin | （无校验值）      |
      | five-m.bin | 正确的 .sha256 伴随 |

  Scenario: 01-resume-sidecar-11 仅删除任务保留文件与 sidecar
    Given 下载中的任务 "<file>" 已产生部分数据与 sidecar
    When 按 D 并选择「仅删除任务」
    Then 任务从列表消失
    And 已下载文件与 sidecar 文件均保留在保存目录

    Examples:
      | file       |
      | keep-me.bin |

  Scenario: 01-resume-sidecar-12 删除任务和文件全部删除
    Given 任务 "<file>"（可为下载中或已完成）
    When 按 D 并选择「删除任务和文件」
    Then 目标文件、sidecar（如仍在）与任务记录全部删除

    Examples:
      | file        |
      | purge-me.bin |

  Scenario: 01-resume-sidecar-13 重加同 URL 同路径任务自动接续断点
    Given 已存在 URL "<url>"、保存目录 "<dir>" 的任务且已下载至进度 ≥ <progress>
    And 该任务已被「仅删除任务」移除（文件与 sidecar 保留）
    When 按对话框重新添加相同 URL 与相同保存目录
    Then 出现断点接续的 toast 提示
    And 任务沿用既有目标文件与 sidecar（文件名不追加序号）且进度从断点合并显示
    And 获得槽位后从断点续传直至完成

    Examples:
      | url                                | dir       | progress |
      | http://fixture.local/files/resume.bin | /tmp/qadl | 30%      |
