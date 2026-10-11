# mutation-stamp: sha256=57e757f1a8f5f65d5d101e6e025b24614b265310cd4af69788aed8f7b17ef6c4
# acceptance-mutation-manifest-begin
# {"version":1,"tested_at":"2026-10-06T13:47:36.316254687Z","feature_name":"01-persistence-config 持久化 · 配置 · 单实例与退出语义","feature_path":"/home/z/swarm-ezr2/project/features/01-persistence-config.feature","background_hash":"ea68af8634f29da43ee298fb3cb4da8d212da2e7b56856c59005d5b86ffde2a8","implementation_hash":"c9d3c78e0bdfc9eb45d3eee4b19e306fbb06fa8e558dcda870cbe627283cdea1","scenarios":[{"index":2,"name":"01-persistence-config-03 配置文件缺失时全部使用默认值","scenario_hash":"76e9f6a256c2a71861b4463d7b95a08194c218d13dd834e371bd2eee1d9f4da1","mutation_count":2,"result":{"Total":2,"Killed":2,"Survived":0,"Errors":0},"tested_at":"2026-10-06T13:47:36.316254687Z"},{"index":3,"name":"01-persistence-config-04 配置非法值回退默认或钳制边界","scenario_hash":"9dd7081e2518d69aa498d3fca3401dafa8e9dfc23cae1ac4f0b16c4ad0c4e6b2","mutation_count":15,"result":{"Total":15,"Killed":15,"Survived":0,"Errors":0},"tested_at":"2026-10-06T13:47:36.316254687Z"},{"index":4,"name":"01-persistence-config-05 block_size_http 可配置生效","scenario_hash":"bb4f6bf1a2afd0728cf11d8025a15daaa6eb2780cd5ee4019dd8815473b5fc95","mutation_count":8,"result":{"Total":8,"Killed":8,"Survived":0,"Errors":0},"tested_at":"2026-10-06T13:47:36.316254687Z"},{"index":5,"name":"01-persistence-config-06 download_slots 可配置生效","scenario_hash":"f9a26f636e2c5d507916daf95ea2ad87173745317c4c7b401b1a6fe4e43268c7","mutation_count":1,"result":{"Total":1,"Killed":1,"Survived":0,"Errors":0},"tested_at":"2026-10-06T13:47:36.316254687Z"},{"index":7,"name":"01-persistence-config-08 Q 优雅退出断点保留重启恢复","scenario_hash":"0c1f36fa7534c122c169d217373eddf5145d9a6935fdf45d51b212329d889b0f","mutation_count":1,"result":{"Total":1,"Killed":1,"Survived":0,"Errors":0},"tested_at":"2026-10-06T13:47:36.316254687Z"},{"index":8,"name":"01-persistence-config-09 Esc 与 Ctrl+C 优雅退出终端无花屏","scenario_hash":"09746c68d73c74212c1d6d28e4cf7ff36862214a15014a4377d3d48d18d2ba9b","mutation_count":2,"result":{"Total":2,"Killed":2,"Survived":0,"Errors":0},"tested_at":"2026-10-06T13:47:36.316254687Z"}]}
# acceptance-mutation-manifest-end

# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   01-persistence-config-01 任务注册表跨会话保留
#   01-persistence-config-02 数据与配置目录布局
#   （v1.4 注：config.toml 缺失时启动自动生成全注释默认模板，FR-01-85 ——
#    判据由「允许缺失」改为「缺失时自动生成」，生成细则见 01-config-template.feature）
#   01-persistence-config-03 配置文件缺失时全部使用默认值
#   01-persistence-config-04 配置非法值回退默认或钳制边界（v1.16：block_size_http 增 <1MB 钳制行）
#   01-persistence-config-05 block_size_http 可配置生效（v1.16：下限 1 MB）
#   01-persistence-config-06 download_slots 可配置生效
#   01-persistence-config-07 单实例保护
#   01-persistence-config-08 Q 优雅退出断点保留重启恢复
#   01-persistence-config-09 Esc 与 Ctrl+C 优雅退出终端无花屏
#   01-persistence-config-10 EZR_HOME 环境变量重定位 .ezr 目录（v1.3/D16）
#   01-persistence-config-11 kill -9 异常终止终端自恢复（v1.3/FR-01-84）
Feature: 01-persistence-config 持久化 · 配置 · 单实例与退出语义

  期号 01。依据 project/mission/phase-01.md FR-01-70~73、FR-01-84 与本会话澄清决定
  （默认并发键 http_concurrency，v1.5 自 default_concurrency 改名，FR-01-88；v1.3：EZR_HOME
  重定位 D16）。配置位于用户主目录
  .ezr/config.toml（或 $EZR_HOME/config.toml），注册表位于 .ezr/state/，均为原子写。
  v1.16 修订（FR-01-104/D31，操作者钦定块大小下限）：`block_size_http` 正值 < 1 MB →
  一律钳制为 1 MB 生效（场景 04 增钳制行；场景 05 原子 1 MB 下界示例行同步替换）；
  0/缺失/非法 → 默认 1 MB 口径不变；续传以 sidecar 块大小为准（一致性优先）。

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
    Then 用户主目录下存在 .ezr/config.toml（缺失时启动自动生成全注释默认模板，v1.4/FR-01-85，
      细则见 01-config-template.feature）与 .ezr/state/ 任务注册表
    And sidecar 文件随各自目标文件存放（不在 .ezr/ 集中存放）

  Scenario: 01-persistence-config-03 配置文件缺失时全部使用默认值
    Given 用户主目录不存在 .ezr/config.toml
    When 按默认配置下载文件 "<file>"
    Then 行为与默认值一致：并发默认 <http_concurrency>、槽位 5、块大小 1 MB、
      max_retries 5、auto_retry 开启、max_speed 不限、download_dir 为用户主目录下的下载目录

    Examples:
      | file       | http_concurrency |
      | five-m.bin | 4                   |

  Scenario: 01-persistence-config-04 配置非法值回退默认或钳制边界
    Given 配置文件设 "<key>" = "<value>"（非法）
    When 启动 ezr 并正常使用
    Then 该配置项按回退/钳制值 "<default>" 生效且启动不报错

    Examples:
      | key                  | value | default |
      | block_size_http      | abc   | 1 MB    |
      | block_size_http      | 256 KB | 1 MB   |
      | download_slots       | 0     | 5       |
      | max_retries          | -1    | 5       |
      | http_concurrency     | 99    | 64      |
      | max_speed            | xyz   | 0       |

  Scenario: 01-persistence-config-05 block_size_http 可配置生效（v1.16：下限 1 MB）
    Given 配置文件设 block_size_http = "<block_size>"
    When 下载文件 "<file>"
    Then 详情分块行显示 "x/<chunks> · <display>/块"

    Examples:
      | file       | block_size | chunks | display  |
      | five-m.bin | 4 MB       | 2      | 4 MB     |
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
