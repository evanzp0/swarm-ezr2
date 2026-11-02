# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   01-throttle-proxy-01 配置全局下载限速生效于全部任务合计速度（v1.9 修订：键改名 + BT 并入）
#   01-throttle-proxy-02 多任务并发时限速作用于合计速度（v1.9 修订：HTTP+BT 混合合计）
#   01-throttle-proxy-03 --max-speed 参数优先于配置（v1.9 修订：键名更新）
#   01-throttle-proxy-04 global_max_download_speed 为 0 不限速且非法值回退默认（v1.9 修订：键名更新）
#   01-throttle-proxy-05 限速值接受十进制单位格式（v1.9 修订：全局两键同口径）
#   01-throttle-proxy-06 限速对不支持续传的任务同样生效
#   01-throttle-proxy-07 头部与图表不新增全局限速控件（v1.9 修订：措辞收窄——任务对话框限速字段属 FR-02-29）
#   01-throttle-proxy-08 全局上传限速生效于 BT 上传合计（v1.9 新增，FR-02-28）
#   01-throttle-proxy-09 任务级限速默认值来自 config 且添加对话框预填可改（v1.9 新增，FR-02-29）
#   01-throttle-proxy-10 任务级与全局限速取 min 合成（v1.9 新增，FR-02-29）
#   01-throttle-proxy-11 修改 HTTP 任务限速确认后立即生效（v1.9 新增，FR-02-29/FR-02-26）
#   01-throttle-proxy-12 修改 BT 任务限速确认后立即生效（v1.9 新增，FR-02-29/FR-02-26）
#   01-throttle-proxy-13 任务级限速随注册表持久化（v1.9 新增，FR-02-29）
#   01-throttle-proxy-14 旧 max_speed 键退役按未知键忽略（v1.9 新增，FR-02-28）
# （v1.6：原 08–11 代理场景随旧全局 proxy 键退役删除（FR-01-90），代理语义由
#   features/02-named-proxy.feature 承载；文件名保留不改，限速场景序号不变。）
# （v1.9 修订注（操作者 bug18 轮指令五项功能之全局/任务级限速；Inversion 阶段 1 答案 =
#   操作者 chat 指令文本，受限退化沿 bug13–17 轮先例；FR-01-60 v1.42 改名注记 +
#   FR-02-28/29 新增，裁决登记 phase-02 D67）：全局限速键 max_speed 改名
#   global_max_download_speed 并新增 global_max_upload_speed（[common] 表，0 = 不限，
#   格式 = 整数 B/s 或十进制单位字符串）；生效范围 = 全部任务合计（HTTP 下载 + BT 下载
#   共享下载配额；上传 = BT 互惠+做种合计；HTTP 无上传语义不消耗上传配额）；
#   CLI --max-speed 保留且语义 = 全局下载限速、参数优先不变。任务级限速两键
#   task_max_download_speed / task_max_upload_speed（[common]，默认 0 = 不限）为新建任务
#   的默认限速值：添加任务对话框（HTTP/BT）新增下载限速/上传限速两字段（预填 config
#   解析值整数 B/s，可修改；留空或非法确认时回退 config 默认）；修改任务对话框（HTTP/BT）
#   新增两字段（预填任务当前值，存量任务缺失按 0），确认后立即生效（FR-02-26）并落注册表；
#   有效限制 = min(全局, 任务级)（上传/下载两方向独立合成，0 视为不限）。
#   场景 01–07 键名与合计口径修订，08–14 新增。）
Feature: 01-throttle-proxy 全局与任务级限速

  期号 01（本轮 v1.9 修订属操作者 bug18 轮指令，需求落 phase-02 FR-02-28/29 并
  回写 phase-01 FR-01-60）。全局下载/上传限速覆盖全部任务（HTTP(S)、BT 及将来
  其他类型）合计速度；任务级限速为任务定义字段（对话框预填 config 默认值）。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件

  Scenario: 01-throttle-proxy-01 配置全局下载限速生效于全部任务合计速度
    Given 配置文件设 [common] 表 global_max_download_speed = "<speed>"
    When 下载文件 "<file>" 至传输稳定
    Then 全局下载速度稳定在 <speed> ±10%（头部 ↓ 与列表行速度一致）

    Examples:
      | speed   | file        |
      | 1 MB/s  | five-m.bin  |

  Scenario: 01-throttle-proxy-02 多任务并发时限速作用于合计速度
    Given 配置文件设 [common] 表 global_max_download_speed = "<speed>"
    And 同时有 <tasks> 个任务在下载中（<mix>）
    Then 全部任务合计速度稳定在 <speed> ±10%（单任务速度为合计的均摊）

    Examples:
      | speed   | tasks | mix        |
      | 1 MB/s  | 2     | HTTP×2     |
      | 1 MB/s  | 2     | HTTP+BT    |

  Scenario: 01-throttle-proxy-03 --max-speed 参数优先于配置
    Given 配置文件设 [common] 表 global_max_download_speed = "1 MB/s"
    When 以命令行启动 "ezr <url> --max-speed <param_speed>"
    Then 下载稳定后全局速度按 <param_speed> ±10% 生效（参数覆盖配置）

    Examples:
      | url                              | param_speed |
      | http://fixture.local/files/a.bin | 512 KB/s    |

  Scenario: 01-throttle-proxy-04 global_max_download_speed 为 0 不限速且非法值回退默认
    Given 配置文件设 [common] 表 global_max_download_speed = "<speed>"
    When 下载文件至传输稳定
    Then 全局速度行为为 "<behavior>"

    Examples:
      | speed   | behavior                |
      | 0       | 不限速（满速下载）     |
      | abc     | 回退默认 0（不限速）   |

  Scenario: 01-throttle-proxy-05 限速值接受十进制单位格式
    Given 配置文件设 [common] 表 <key> = "<value>"
    When 下载至传输稳定（global_max_upload_speed 场景以 BT 上传观察）
    Then 限速按 <value> 十进制口径生效（1 MB=1000 KB）

    Examples:
      | key                          | value      |
      | global_max_download_speed    | 2 MB/s     |
      | global_max_download_speed    | 2 mb/s     |
      | global_max_download_speed    | 500KB/s    |
      | global_max_download_speed    | 2000000    |
      | global_max_upload_speed      | 1 MB/s     |

  Scenario: 01-throttle-proxy-06 限速对不支持续传的任务同样生效
    Given 配置文件设 [common] 表 global_max_download_speed = "1 MB/s"
    When 下载不支持续传的文件（<file>）至传输稳定
    Then 速度同样受限 ±10%

    Examples:
      | file           |
      | streamy.bin    |

  Scenario: 01-throttle-proxy-07 头部与图表不新增全局限速控件
    When 检查全部界面区块与页脚
    Then 头部/图表布局不变且无全局限速控件（添加/修改对话框的任务级限速字段属 FR-02-29，不算全局限速控件）

  Scenario: 01-throttle-proxy-08 全局上传限速生效于 BT 上传合计
    Given 配置文件设 [common] 表 global_max_upload_speed = "1 MB/s"
    And 本地 BT fixture 已就绪（leecher 对本端做种任务拉取数据）
    When leecher 从本端上传合计稳定
    Then 全部任务上传合计稳定在 1 MB/s ±10%（头部 ↑ 与列表行上传速度一致；做种与互惠上传均计入）

  Scenario: 01-throttle-proxy-09 任务级限速默认值来自 config 且添加对话框预填可改
    Given 配置文件设 [common] 表 task_max_download_speed = "<down>" 与 task_max_upload_speed = "<up>"
    When 按 A 打开「添加任务：HTTP(S)」对话框（或命令窗口分流打开「添加任务：BT」）
    Then 下载限速字段预填 "<down_parsed>"、上传限速字段预填 "<up_parsed>"（config 解析后整数 B/s；0 显示 0）
    And 两字段均可编辑（接受整数 B/s 或十进制单位字符串如 "2 MB/s"）
    And 输入 "<input>" 确认后任务任务级限速落为 "<result>"（留空或非法回退 config 默认 <down>/<up>）

    Examples:
      | down      | up        | down_parsed | up_parsed | input     | result    |
      | 2 MB/s    | 0         | 2000000     | 0         | 留空      | 2000000   |
      | 2 MB/s    | 0         | 2000000     | 0         | 500 KB/s  | 500000    |
      | 0         | 512 KB/s  | 0           | 512000    | abc       | 0         |

  Scenario: 01-throttle-proxy-10 任务级与全局限速取 min 合成
    Given 配置文件设 [common] 表 global_max_download_speed = "<g_down>" 与 task_max_download_speed = "<t_down>"
    When 任务（任务级下载限速 <t_down>）下载至传输稳定
    Then 任务速度稳定在 <expected> ±10%（有效限制 = min(全局, 任务级)；0 视为不限）

    Examples:
      | g_down   | t_down   | expected |
      | 1 MB/s   | 500 KB/s | 500 KB/s |
      | 1 MB/s   | 0        | 1 MB/s   |
      | 0        | 2 MB/s   | 2 MB/s   |

  Scenario: 01-throttle-proxy-11 修改 HTTP 任务限速确认后立即生效
    Given 任务 A（HTTP）下载中且任务级下载限速为 "<before>"
    When 按 M 打开「修改任务：HTTP(S)」对话框将下载限速改为 "<after>" 并确认
    Then 任务保持下载中（不重启、不重新排队），约一进度帧内速度向 <after> 收敛（上调/下调同口径）
    And toast 报告修改成功且注册表立即落盘（重开对话框预填 <after>）

    Examples:
      | before   | after    |
      | 0        | 1 MB/s   |
      | 1 MB/s   | 500 KB/s |
      | 1 MB/s   | 0        |

  Scenario: 01-throttle-proxy-12 修改 BT 任务限速确认后立即生效
    Given 任务 B（BT）下载中（或做种中）且任务级下载/上传限速为 "<before>"
    When 按 M 打开「修改任务：BT」对话框将下载限速/上传限速改为 "<after>" 并确认
    Then 引擎不重启（无 CancelBt+StartBt、无重新排队、无启动重校验窗），任务保持运行
    And 下载速度/上传速度（做种与互惠上传合计）约一进度帧内向 <after> 收敛（与全局取 min 合成）
    And toast 沿既有 "✓ BT 任务已更新…（立即生效）" 口径且注册表落盘

    Examples:
      | before     | after      |
      | 0 / 0      | 1 MB/s / 0 |
      | 1 MB/s / 0 | 0 / 1 MB/s |

  Scenario: 01-throttle-proxy-13 任务级限速随注册表持久化
    Given 任务（<proto>）任务级下载/上传限速为 "<down>"/"<up>"
    When 退出 ezr 并重新启动
    Then 任务预填与生效限速仍为 "<down>"/"<up>"（注册表保留；修改对话框回读一致）

    Examples:
      | proto     | down     | up       |
      | HTTP      | 1 MB/s   | 0        |
      | BT        | 0        | 1 MB/s   |

  Scenario: 01-throttle-proxy-14 旧 max_speed 键退役按未知键忽略
    Given 配置文件仅含 [common] 表 max_speed = "1 MB/s"（旧键名）
    When ezr 启动
    Then 启动不报错且全局下载限速为 0（旧键按未知键静默忽略回退默认，FR-01-71 口径；迁移 = 改用 global_max_download_speed）
