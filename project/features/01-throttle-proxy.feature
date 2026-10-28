# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   01-throttle-proxy-01 配置全局限速生效于合计速度
#   01-throttle-proxy-02 多任务并发时限速作用于合计速度
#   01-throttle-proxy-03 --max-speed 参数优先于配置
#   01-throttle-proxy-04 max_speed 为 0 不限速且非法值回退默认
#   01-throttle-proxy-05 限速值接受十进制单位格式
#   01-throttle-proxy-06 限速对不支持续传的任务同样生效
#   01-throttle-proxy-07 TUI 不新增限速控件
# （v1.6：原 08–11 代理场景随旧全局 proxy 键退役删除（FR-01-90），代理语义由
#   features/02-named-proxy.feature 承载；文件名保留不改，限速场景序号不变。）
Feature: 01-throttle-proxy 全局限速

  期号 01。依据 project/mission/phase-01.md FR-01-60、NFR-3 与本会话澄清决定
  （十进制单位口径、接受 B/s~GB/s）。每任务限速为分期 03 范围外。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件

  Scenario: 01-throttle-proxy-01 配置全局限速生效于合计速度
    Given 配置文件设 max_speed = "<speed>"
    When 下载文件 "<file>" 至传输稳定
    Then 全局下载速度稳定在 <speed> ±10%（头部 ↓ 与列表行速度一致）

    Examples:
      | speed   | file        |
      | 1 MB/s  | five-m.bin  |

  Scenario: 01-throttle-proxy-02 多任务并发时限速作用于合计速度
    Given 配置文件设 max_speed = "<speed>"
    And 同时有 <tasks> 个任务在下载中
    Then 全部任务合计速度稳定在 <speed> ±10%（单任务速度为合计的均摊）

    Examples:
      | speed   | tasks |
      | 1 MB/s  | 2     |

  Scenario: 01-throttle-proxy-03 --max-speed 参数优先于配置
    Given 配置文件设 max_speed = "1 MB/s"
    When 以命令行启动 "ezr <url> --max-speed <param_speed>"
    Then 下载稳定后全局速度按 <param_speed> ±10% 生效（参数覆盖配置）

    Examples:
      | url                              | param_speed |
      | http://fixture.local/files/a.bin | 512 KB/s    |

  Scenario: 01-throttle-proxy-04 max_speed 为 0 不限速且非法值回退默认
    Given 配置文件设 max_speed = "<speed>"
    When 下载文件至传输稳定
    Then 全局速度行为为 "<behavior>"

    Examples:
      | speed   | behavior                |
      | 0       | 不限速（满速下载）     |
      | abc     | 回退默认 0（不限速）   |

  Scenario: 01-throttle-proxy-05 限速值接受十进制单位格式
    Given 配置文件设 max_speed = "<speed>"
    When 下载文件至传输稳定
    Then 全局速度稳定在 <limit> ±10%（1 MB = 1000 KB，十进制口径）

    Examples:
      | speed      | limit     |
      | 2MB/s      | 2 MB/s    |
      | 2 mb/s     | 2 MB/s    |
      | 500KB/s    | 500 KB/s  |
      | 1GB/s      | 1 GB/s    |
      | 2000000 B/s | 约 2 MB/s |

  Scenario: 01-throttle-proxy-06 限速对不支持续传的任务同样生效
    Given 配置文件设 max_speed = "<speed>"
    And fixture 对文件 "<file>" 不支持 Range 请求
    When 该任务以单并发降级模式下载
    Then 其速度同样被限制在 <speed> ±10%

    Examples:
      | speed   | file        |
      | 1 MB/s  | streamy.bin |

  Scenario: 01-throttle-proxy-07 TUI 不新增限速控件
    When 检查界面全部区块与页脚快捷键
    Then 不存在限速设置控件（头部统计与图表布局不变）
    And 限速仅经配置文件与启动参数设置

