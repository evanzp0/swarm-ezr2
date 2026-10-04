# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   01-persistence-config-01 任务注册表跨会话保留
#   01-persistence-config-02 数据与配置目录布局
#   01-persistence-config-03 配置文件缺失时全部使用默认值
#   01-persistence-config-04 配置非法值回退默认或钳制边界
#   01-persistence-config-05 block_size_http 可配置生效
#   01-persistence-config-06 download_slots 可配置生效
#   01-persistence-config-07 单实例保护
#   01-persistence-config-08 Q 优雅退出断点保留重启恢复
#   01-persistence-config-09 Esc 与 Ctrl+C 优雅退出终端无花屏
#   01-persistence-config-10 EZR_HOME 环境变量重定位 .ezr 目录（v1.3/D16）
#   01-persistence-config-11 kill -9 异常终止终端自恢复（v1.3/FR-01-84）
Feature: 01-persistence-config 持久化 · 配置 · 单实例与退出语义

  期号 01。依据 project/mission/phase-01.md FR-01-70~73、FR-01-84 与本会话澄清决定
  （默认并发键 default_concurrency；v1.3：EZR_HOME 重定位 D16）。配置位于用户主目录
  .ezr/config.toml（或 $EZR_HOME/config.toml），注册表位于 .ezr/state/，均为原子写。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件

  Scenario: 01-persistence-config-01 任务注册表跨会话保留
    Given 任务列表含各状态任务（下载中、等待中、已暂停、已失败、已完成）
    When 优雅退出后重新启动 ezr
    Then 全部任务（含已完成与已失败历史）与列表顺序完整还原
    And 各任务恢复到与其 sidecar/状态一致的续传点或终态

  Scenario: 01-persistence-config-02 数据与配置目录布局
    When 运行 ezr 并添加任务
    Then 用户主目录下存在 .ezr/config.toml（或未改动时允许缺失）与 .ezr/state/ 任务注册表
    And sidecar 文件随各自目标文件存放（不在 .ezr/ 集中存放）

  Scenario: 01-persistence-config-03 配置文件缺失时全部使用默认值
    Given 用户主目录不存在 .ezr/config.toml
    When 按默认配置下载文件 "<file>"
    Then 行为与默认值一致：并发默认 <default_concurrency>、槽位 5、块大小 1 MB、
      max_retries 5、auto_retry 开启、max_speed 不限、download_dir 为用户主目录下的下载目录

    Examples:
      | file       | default_concurrency |
      | five-m.bin | 4                   |

  Scenario: 01-persistence-config-04 配置非法值回退默认或钳制边界
    Given 配置文件设 "<key>" = "<value>"（非法）
    When 启动 ezr 并正常使用
    Then 该配置项按回退/钳制值 "<default>" 生效且启动不报错

    Examples:
      | key                  | value | default |
      | block_size_http      | abc   | 1 MB    |
      | download_slots       | 0     | 5       |
      | max_retries          | -1    | 5       |
      | default_concurrency  | 99    | 64      |
      | max_speed            | xyz   | 0       |

  Scenario: 01-persistence-config-05 block_size_http 可配置生效
    Given 配置文件设 block_size_http = "<block_size>"
    When 下载文件 "<file>"
    Then 详情分块行显示 "x/<chunks> · <display>/块"

    Examples:
      | file       | block_size | chunks | display  |
      | five-m.bin | 256 KB     | 20     | 256 KB   |
      | five-m.bin | 2 MB       | 3      | 2 MB     |

  Scenario: 01-persistence-config-06 download_slots 可配置生效
    Given 配置文件设 download_slots = <slots>
    When 依次添加 <slots+2> 个下载任务
    Then 前 <slots> 个任务开始下载且任务队列栏显示「下载槽位 n/<slots>」
    And 其余任务排队等待

    Examples:
      | slots |
      | 2     |

  Scenario: 01-persistence-config-07 单实例保护
    Given ezr 实例已在运行
    When 再次启动 ezr
    Then 第二个实例在命令行提示「ezr 已在运行」并以非零退出码自动退出
    And 首个实例持续正常运行不受影响
    And 任意时刻进程中只有一个 ezr 实例

  Scenario: 01-persistence-config-08 Q 优雅退出断点保留重启恢复
    Given 任务下载至进度 ≥ <progress>
    When 按 Q 退出
    Then 传输停止且断点保留（sidecar 完整）
    And 终端恢复正常显示
    When 重新启动 ezr
    Then 任务从断点续传直至完成

    Examples:
      | progress |
      | 30%      |

  Scenario: 01-persistence-config-09 Esc 与 Ctrl+C 优雅退出终端无花屏
    Given 任务下载中
    When 以 "<exit_key>" 退出
    Then 退出语义与 Q 一致（停止传输、保留断点、保存注册表）
    And 终端无花屏残留（光标与配色恢复正常）

    Examples:
      | exit_key |
      | Esc      |
      | Ctrl+C   |

  Scenario: 01-persistence-config-10 EZR_HOME 环境变量重定位 .ezr 目录
    Given 环境变量 EZR_HOME 指向一个空目录
    When 运行 ezr 并添加任务（触发配置读取与注册表写入）
    Then 配置文件读取自 $EZR_HOME/config.toml 且任务注册表写入 $EZR_HOME/state/registry.json
    And 单实例锁文件位于 $EZR_HOME/state/ezr.lock
    And 用户主目录下不存在 .ezr/（默认路径未被创建）
    And EZR_HOME 为空串或未设置时回退缺省 ~/.ezr（D16 缺省口径）
    And 不同 EZR_HOME 的两组实例各自持有各自的锁与注册表（隔离语义，互不冲突）

  Scenario: 01-persistence-config-11 kill -9 异常终止终端自恢复
    Given ezr TUI 运行中（raw mode、鼠标捕获、bracketed paste 与备用屏幕已启用）
    When 从另一终端以 SIGKILL（kill -9）终止 ezr 进程
    Then 运行 ezr 的终端立即自恢复：不再出现 "32;64;10M" 形态的鼠标事件转义字符残影输出
    And 终端回到正常（cooked）模式：按键回显生效、CTRL+C 可正常产生 SIGINT 中断
    And 鼠标捕获、bracketed paste 与备用屏幕均已复位，光标与配色正常
    And 哨兵子进程随之退出，不留残留进程
