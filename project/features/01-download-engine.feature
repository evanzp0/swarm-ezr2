# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   01-download-engine-01 探测结果与详情如实显示（支持断点续传）
#   01-download-engine-02 无 Content-Length 降级单并发
#   01-download-engine-03 服务器不响应 Range 降级且暂停后从头下载
#   01-download-engine-04 分块总数按 1 MB 块计算且末块吸收余数
#   01-download-engine-05 多连接动态领块乱序完成
#   01-download-engine-06 队列临近结束多余连接转待命
#   01-download-engine-07 续传不重写已下载块
#   01-download-engine-08 重定向跟随至最终 URL 完成下载
#   01-download-engine-09 重定向超过 10 次停等
#   01-download-engine-10 重定向目标非 http(s) 停等
#   01-download-engine-11 HTTPS 证书错误按任务失败处理
#   01-download-engine-12 HTTPS 正常下载成功
#   01-download-engine-13 分块请求禁用内容压缩
#   01-download-engine-14 速度按 1 秒滑动窗口驱动显示
#   01-download-engine-15 等待任务添加即预取大小（失败显示未知）
#   01-download-engine-16 默认配置下载 100 MB 文件端到端（AC-1）
Feature: 01-download-engine HTTP 下载内核（探测 · 分块 · 重定向 · HTTPS · 速度）

  期号 01。依据 project/mission/phase-01.md FR-01-10~17、FR-01-82 与本会话澄清决定
  （等待任务添加即预取、重定向边界停等）。块大小默认 1 MB（可配置），AIR2 分块队列模型。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件

  Scenario: 01-download-engine-01 探测结果与详情如实显示（支持断点续传）
    Given fixture 文件 "<file>" 大小 <size> 且支持 Range 请求
    When 添加该文件的下载任务
    And 任务获得空闲槽位开始下载
    Then 详情显示大小 <size>、类型 "<scheme>" 且断点续传为「支持断点续传」

    Examples:
      | file       | size    | scheme |
      | five-m.bin | 5.0 MB  | HTTP   |

  Scenario: 01-download-engine-02 无 Content-Length 降级单并发
    Given fixture 提供不返回 Content-Length 的下载项 "<file>"
    When 添加该下载项并等待开始
    Then 详情显示「不支持断点续传」且分块明细仅 1 个活跃连接
    And 下载最终完成且文件字节完整

    Examples:
      | file        |
      | streamy.bin |

  Scenario: 01-download-engine-03 服务器不响应 Range 降级且暂停后从头下载
    Given fixture 对文件 "<file>" 不响应 Range 请求（忽略 Range 头返回 200 全量）
    When 添加该文件并开始下载至进度 ≥ <progress>
    And 按 Space 暂停任务
    And 按 Space 继续任务
    Then fixture 收到的新请求为从头全量下载（无 Range 断点）
    And 详情显示「不支持断点续传」
    And 下载最终完成且文件字节完整

    Examples:
      | file       | progress |
      | three-m.bin | 30%      |

  Scenario: 01-download-engine-04 分块总数按 1 MB 块计算且末块吸收余数
    Given fixture 文件大小 <file_size>
    When 添加该文件并等待开始
    Then 详情分块行显示 "<expected_chunks> · 1 MB/块"

    Examples:
      | file_size | expected_chunks |
      | 2.5 MB    | 0/3             |
      | 1 MB      | 0/1             |
      | 512 KB    | 0/1             |
      | 10 MB     | 0/10            |

  Scenario: 01-download-engine-05 多连接动态领块乱序完成
    Given fixture 文件 <file_size> 且支持 Range 请求
    When 以并发数 <concurrency> 添加并开始下载
    Then 分块明细表同时显示最多 <concurrency> 个连接各自持有块号
    And 各连接完成一块立即领取下一块且块完成顺序为乱序
    And 实际并发不超过 <concurrency> 且下载完成后分块行显示 "<done_chunks> · 1 MB/块"

    Examples:
      | file_size | concurrency | done_chunks |
      | 12 MB     | 4           | 12/12       |
      | 3 MB      | 8           | 3/3         |

  Scenario: 01-download-engine-06 队列临近结束多余连接转待命
    Given fixture 文件大小 2 MB（共 2 块）
    When 以并发数 4 添加并开始下载
    Then 分块明细表显示 2 个连接传输中、2 个连接「待命」
    And 下载最终完成

  Scenario: 01-download-engine-07 续传不重写已下载块
    Given fixture 文件 <file_size> 支持断点续传且请求区间可被记录
    When 下载至进度 ≥ <progress> 后按 Space 暂停
    And 按 Space 继续任务
    Then fixture 记录的续传请求仅覆盖未完成块的区间（已下载块区间零重复请求）
    And 下载最终完成且文件字节完整

    Examples:
      | file_size | progress |
      | 8 MB      | 40%      |

  Scenario: 01-download-engine-08 重定向跟随至最终 URL 完成下载
    Given fixture 提供从 "<origin_url>" 重定向（<redirects> 次）至 "<final_url>" 的下载链
    When 添加 origin URL 的下载任务并等待开始
    Then 下载经重定向最终完成且文件字节完整
    And 详情 URL 显示最终 URL

    Examples:
      | origin_url                              | redirects | final_url                             |
      | http://fixture.local/r/jump/a.bin       | 2         | http://fixture.local/files/a.bin      |

  Scenario: 01-download-engine-09 重定向超过 10 次停等
    Given fixture 提供重定向链长度 <chain_length> 的下载项
    When 添加该下载项并等待开始
    Then 任务转「已失败」且失败原因显示「重定向次数超限」
    And 列表行显示「不自动重试」且任务不占用下载槽位

    Examples:
      | chain_length |
      | 11           |

  Scenario: 01-download-engine-10 重定向目标非 http(s) 停等
    Given fixture 提供重定向至 "<target>" 的下载项
    When 添加该下载项并等待开始
    Then 任务转「已失败」且失败原因显示「不支持的重定向协议」
    And 列表行显示「不自动重试」且任务不占用下载槽位

    Examples:
      | target               |
      | ftp://fixture.local/f |

  Scenario: 01-download-engine-11 HTTPS 证书错误按任务失败处理
    Given fixture 以自签名证书提供 "<https_url>"（未加入系统信任）
    When 添加该下载任务并等待开始
    Then 任务转「已失败」且失败原因显示 TLS 证书错误
    And 不存在跳过证书校验的开关或提示

    Examples:
      | https_url                          |
      | https://fixture.local/files/a.bin  |

  Scenario: 01-download-engine-12 HTTPS 正常下载成功
    Given fixture 以受系统信任的证书提供 "<https_url>"
    When 添加该下载任务并等待开始
    Then 下载完成且文件字节完整且详情类型显示 "HTTPS"

    Examples:
      | https_url                          |
      | https://fixture.local/files/tls.bin |

  Scenario: 01-download-engine-13 分块请求禁用内容压缩
    Given fixture 可记录各请求的 Accept-Encoding 头
    When 以默认配置下载支持 Range 的文件 <file>
    Then fixture 记录的全部分块下载请求头 Accept-Encoding 为 "identity"

    Examples:
      | file       |
      | five-m.bin |

  Scenario: 01-download-engine-14 速度按 1 秒滑动窗口驱动显示
    When 下载任务处于「下载中」
    Then 列表行速度、头部全局 ↓ 速度与 Sparkline 面板随下载持续每秒更新且数值来自真实传输
    And 下载完成后列表与详情速度归零或显示完成态

  Scenario: 01-download-engine-15 等待任务添加即预取大小（失败显示未知）
    Given 下载槽位已被其他任务占满
    When 添加 fixture 文件 "<file>" 的下载任务
    Then 任务处于「等待中」且列表等待行显示总大小 <display_size>（未开始下载即预取）
    When 添加一个探测必然失败的下载项 "<bad_file>"
    Then 其等待行大小显示「未知」且不阻塞等待队列

    Examples:
      | file       | display_size | bad_file     |
      | five-m.bin | 0 B/5.0 MB   | ghost-404.bin |

  Scenario: 01-download-engine-16 默认配置下载 100 MB 文件端到端（AC-1）
    Given fixture 文件 "big-100m.bin" 大小 100 MB（稀疏构造）且支持 Range 请求
    And 源文件的正确 SHA-256 摘要已预置为伴随文件 "big-100m.bin.sha256"
    When 以默认并发 4 与默认块大小添加并下载该文件
    Then 详情分块行显示 "x/100 · 1 MB/块"（x 随进度推进至 100）且分块明细表活跃连接数为 4
    And 分块完成顺序为乱序
    And 完成后任务转「已完成」且显示「SHA-256 校验成功」（源与目标校验值一致）
    And 目标文件字节数为 104857600
