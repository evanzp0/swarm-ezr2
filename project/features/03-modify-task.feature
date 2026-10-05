# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   03-modify-task-01 按 m 弹出修改对话框且四字段预填当前值
#   03-modify-task-02 修改并发立即生效（上调连接数上升 / 下调块后收缩）
#   03-modify-task-03 修改代理立即对新连接生效
#   03-modify-task-04 修改校验算法与校验码，完成校验按新值判定
#   03-modify-task-05 清空校验码 = 清除校验
#   03-modify-task-06 非法输入不生效并聚焦提示（不关闭对话框）
#   03-modify-task-07 已完成任务按 m 拒绝（toast 提示）
#   03-modify-task-08 修改后注册表立即落盘（重启保留新参数）
Feature: 03-modify-task 任务修改对话框（并发/校验/代理热生效）

  期号 01。依据 project/mission/phase-01.md FR-01-87（v1.5 操作者指令）与 D19/D20：
  列表选中任务按 m 弹出「修改任务」对话框，可改并发/校验算法/校验码/代理，确定后立即生效；
  立即生效口径（D19）：并发 = 下一调度周期（上调新 worker 立即加入，下调多余 worker 完成当前
  块后退出）；代理 = 之后新发起的连接（在途请求按旧代理完成）；校验 = 完成校验时采用最新值。
  适用范围（D20，待操作者复核）：非「已完成」任务均可按 m；已完成任务 toast 拒绝。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件
    And 本地 fixture 代理（可区分记录经手请求，支持 --name 多实例）已启动

  Scenario: 03-modify-task-01 按 m 弹出修改对话框且四字段预填当前值
    Given 添加任务时设并发 <concurrency>、校验 <algo>/<value>、代理 <proxy>
    When 列表选中该任务按 m
    Then 弹出「修改任务」对话框：并发输入框 = <concurrency>、算法下拉 = <algo>、
      校验码输入框 = <value>、代理下拉 = <proxy>
    And 字段焦点次序 = 并发 → 算法 → 校验码 → 代理 → 确定 → 取消（键盘可完整操作）

    Examples:
      | concurrency | algo    | value                            | proxy    |
      | 3           | SHA-256 | <fixture 摘要>                    | 直连     |

  Scenario: 03-modify-task-02 修改并发立即生效（上调连接数上升 / 下调块后收缩）
    Given 任务以并发 <before> 下载中（文件足够大以维持多块在途）
    When 按 m 将并发改为 <after> 并确定
    Then 上调（<after> > <before>）：详情活跃连接数在下一调度周期升至 ≤ <after>
    And 下调（<after> < <before>）：多余 worker 完成当前块后退出，活跃连接数降至 ≤ <after>
    And 全程任务不暂停、已下载字节单调不减（无在途块被打断重取）

    Examples:
      | before | after |
      | 1      | 4     |
      | 4      | 1     |

  Scenario: 03-modify-task-03 修改代理立即对新连接生效
    Given 任务以 <before_proxy> 下载中（多块文件，传输持续）
    When 按 m 将代理改为 <after_proxy> 并确定
    Then 之后新发起的连接经 <after_proxy> 执行（fixture 代理日志出现该任务后续请求；
      切换直连时两代理日志均不再增长）
    And 在途请求按旧代理完成；任务最终完成且文件正确

    Examples:
      | before_proxy | after_proxy             |
      | 直连         | http://127.0.0.1:18766 |
      | proxy-a      | 直连                    |

  Scenario: 03-modify-task-04 修改校验算法与校验码，完成校验按新值判定
    Given 任务下载中且原校验为 <orig_algo>/<orig_value>（或无校验）
    When 按 m 将校验改为 <new_algo>/<new_value> 并确定
    Then 任务完成时按新算法/校验码校验：正确值 → 状态「已完成」+ 校验通过标识；
      错误值 → 校验失败提示（错误摘要入详情）

    Examples:
      | orig_algo | orig_value      | new_algo | new_value        | verdict |
      | 无        |                 | SHA-256  | <正确摘要>        | 通过    |
      | SHA-256   | <正确摘要>       | MD5      | <错误摘要>        | 失败    |

  Scenario: 03-modify-task-05 清空校验码 = 清除校验
    Given 任务原设校验 SHA-256/<某摘要>
    When 按 m 清空校验码并确定
    Then 详情显示无校验期望；任务完成时不校验直接「已完成」（无校验通过/失败标识）

    Examples:
      | file       |
      | one-m.bin  |

  Scenario: 03-modify-task-06 非法输入不生效并聚焦提示（不关闭对话框）
    Given 修改对话框已打开
    When 校验码输入与所选算法位数不符的十六进制（如 SHA-256 配 32 位 hex）并确定
    Then 对话框不关闭、参数不生效（任务无变化）；校验码字段聚焦并提示位数不符
    And 修正为合法值后确定生效（正向路径恢复）

    Examples:
      | algo    | bad_value                        |
      | SHA-256 | deadbeef（8 位）                  |

  Scenario: 03-modify-task-07 已完成任务按 m 拒绝（toast 提示）
    Given 列表存在一个「已完成」任务
    When 选中它按 m
    Then 不弹对话框；toast 提示已完成任务不可修改

    Examples:
      | file       |
      | one-m.bin  |

  Scenario: 03-modify-task-08 修改后注册表立即落盘（重启保留新参数）
    Given 任务下载中（或暂停），按 m 修改并发/校验/代理并确定
    When 立即退出 ezr（Q）后重新启动
    Then 该任务的并发/校验算法/校验码/代理与修改后一致（详情与修改对话框预填可证）
    And 续传/重试按新参数执行

    Examples:
      | file       | concurrency | proxy    |
      | big-m.bin  | 2           | proxy-a  |
