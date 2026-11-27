# mutation-stamp: sha256=56eff044fad49dffc51962e637bbcd0fbe5d4b2df39b1167dbd822c7009e0673
# acceptance-mutation-manifest-begin
# {"version":1,"tested_at":"2026-11-25T03:26:01.449597454Z","feature_name":"01-download-engine HTTP 下载内核（探测 · 分块 · 重定向 · HTTPS · 速度）","feature_path":"/home/z/swarm-ezr2/project/features/01-download-engine.feature","background_hash":"ea68af8634f29da43ee298fb3cb4da8d212da2e7b56856c59005d5b86ffde2a8","implementation_hash":"unknown","scenarios":[{"index":0,"name":"01-download-engine-01 探测结果与详情如实显示（支持断点续传）","scenario_hash":"facf659fb5af1ef2d850f07d2eb75341efcd579a4ff6b2675812685acb85110a","mutation_count":3,"result":{"Total":3,"Killed":3,"Survived":0,"Errors":0},"tested_at":"2026-11-21T14:15:41.142892583Z"},{"index":1,"name":"01-download-engine-02 无 Content-Length 降级单并发","scenario_hash":"3192ec79af0a8f0e2ddc6474d50e5b2d07289107d4d98812780f855d1b5484a5","mutation_count":1,"result":{"Total":1,"Killed":1,"Survived":0,"Errors":0},"tested_at":"2026-11-21T14:15:41.142892583Z"},{"index":2,"name":"01-download-engine-03 服务器不响应 Range 降级且暂停后从头下载","scenario_hash":"9ba6ca8566e4187d925eadf2897d1aac3673e07238b79dc617401b87e173009d","mutation_count":2,"result":{"Total":2,"Killed":2,"Survived":0,"Errors":0},"tested_at":"2026-11-21T14:15:41.142892583Z"},{"index":3,"name":"01-download-engine-04 分块总数按 1 MB 块计算且末块吸收余数","scenario_hash":"624a6f1851d713a2b13843f03075453670704d114578de63b392dad754d4a8f8","mutation_count":8,"result":{"Total":8,"Killed":8,"Survived":0,"Errors":0},"tested_at":"2026-11-21T14:15:41.142892583Z"},{"index":4,"name":"01-download-engine-05 多连接动态领块乱序完成","scenario_hash":"da5ed3de5808f1766a9da4375993ed38eaf731996d1bb6a3f172c3ff70cfe7fb","mutation_count":6,"result":{"Total":6,"Killed":6,"Survived":0,"Errors":0},"tested_at":"2026-11-21T14:15:41.142892583Z"},{"index":6,"name":"01-download-engine-07 续传不重写已下载块","scenario_hash":"40cbda81ece26664538e46d6376f2e4a5104750e0694b4444d61a65106bbe42a","mutation_count":2,"result":{"Total":2,"Killed":2,"Survived":0,"Errors":0},"tested_at":"2026-11-21T14:15:41.142892583Z"},{"index":7,"name":"01-download-engine-08 重定向跟随完成下载（原始 URL 唯一权威，各连接独立跟跳转）","scenario_hash":"adac9b03f7828c3d495cea4338ab129b98da92527e580c1c7c2a56f28e701db1","mutation_count":3,"result":{"Total":3,"Killed":3,"Survived":0,"Errors":0},"tested_at":"2026-11-21T14:15:41.142892583Z"},{"index":8,"name":"01-download-engine-09 重定向超过 10 次停等","scenario_hash":"02859b5fd03587e17c63e1094647aa13e004c6c5049bb103dc6b1b24216ac35d","mutation_count":1,"result":{"Total":1,"Killed":1,"Survived":0,"Errors":0},"tested_at":"2026-11-21T14:15:41.142892583Z"},{"index":9,"name":"01-download-engine-10 重定向目标非 http(s) 停等","scenario_hash":"5951e0a9ae07922bd12fddf909da328c47639072a2f3b97b18876beadf0c590a","mutation_count":1,"result":{"Total":1,"Killed":1,"Survived":0,"Errors":0},"tested_at":"2026-11-21T14:15:41.142892583Z"},{"index":10,"name":"01-download-engine-11 HTTPS 证书错误按任务失败处理","scenario_hash":"577dd5c5985997d36ad9e8e915226f2d36dbd376e8a81e93c84f680d2f9baeef","mutation_count":1,"result":{"Total":1,"Killed":1,"Survived":0,"Errors":0},"tested_at":"2026-11-21T14:15:41.142892583Z"},{"index":11,"name":"01-download-engine-12 HTTPS 正常下载成功","scenario_hash":"f4ff7b0ba8e3ce09e9731e4d17cc70cccdd6b5d49ea3545a408e2502316695f1","mutation_count":1,"result":{"Total":1,"Killed":1,"Survived":0,"Errors":0},"tested_at":"2026-11-21T14:15:41.142892583Z"},{"index":12,"name":"01-download-engine-13 分块请求禁用内容压缩","scenario_hash":"cc65aaa76528052d5b6b19e8806838f48b9fd7c15518e1fb26f4e49ce4dba1ea","mutation_count":1,"result":{"Total":1,"Killed":1,"Survived":0,"Errors":0},"tested_at":"2026-11-21T14:15:41.142892583Z"},{"index":14,"name":"01-download-engine-15 等待下载槽位的任务不预取文件大小","scenario_hash":"91de009773ca663ccf0204cdc1334fd1f27dddaef635cb8a68bb56a80f4d3928","mutation_count":2,"result":{"Total":2,"Killed":2,"Survived":0,"Errors":0},"tested_at":"2026-11-21T14:15:41.142892583Z"}]}
# acceptance-mutation-manifest-end

# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   01-download-engine-01 探测结果与详情如实显示（支持断点续传）
#   01-download-engine-02 无 Content-Length 降级单并发
#   01-download-engine-03 服务器不响应 Range 降级且暂停后从头下载
#   01-download-engine-04 分块总数按 1 MB 块计算且末块吸收余数【退役，留空不重排——v1.63/族A：blocks 钉定臂随 blocks 引擎退役】
#   01-download-engine-05 多连接动态领块乱序完成【退役，留空不重排——v1.63/族A：同上】
#   01-download-engine-06 队列临近结束多余连接转待命【退役，留空不重排——v1.63/族A：同上】
#   01-download-engine-07 续传不重写已下载块【退役，留空不重排——v1.63/族A：同上；动态续传断言由 01-resume-sidecar-03/13-http-turbo-14 承载】
#   01-download-engine-08 重定向跟随完成下载（原始 URL 唯一权威，各连接独立跟跳转）
#   01-download-engine-09 重定向超过 10 次停等
#   01-download-engine-10 重定向目标非 http(s) 停等
#   01-download-engine-11 HTTPS 证书错误按任务失败处理
#   01-download-engine-12 HTTPS 正常下载成功
#   01-download-engine-13 分块请求禁用内容压缩
#   01-download-engine-14 速度按 1 秒滑动窗口驱动显示
#   01-download-engine-15 等待下载槽位的任务不预取文件大小
#   01-download-engine-16 默认配置下载 100 MB 文件端到端（AC-1）
Feature: 01-download-engine HTTP 下载内核（探测 · 分块 · 重定向 · HTTPS · 速度）

  期号 01。依据 project/mission/phase-01.md FR-01-10~17、FR-01-82 与本会话澄清决定
  （等待任务不预取、重定向边界停等）。块大小默认 1 MB（可配置），AIR2 分块队列模型。
  v1.28 修订（操作者 URL 权威模型改版，FR-01-14）：原始输入 URL 为任务级唯一权威，
  跳转后 URL 仅为每条连接内部的当前值；各连接（探测与分块）各自从原始 URL 独立跟跳转；
  详情 URL 显示原始 URL（场景 08 重写，原「详情 URL 显示最终 URL」退役）。
  v1.49 锚点对齐（specifier 遗留项处理轮 20261112，QA 终验轮移交待批项①）：场景 15
  THEN-1「等待行大小显示『未知』」为 v1.28 期观察锚，随 v1.49（FR-02-38 v1.19）明细行
  尺寸/未知项整项退役改「等待行不显示尺寸（尺寸未知不显示、无预取，以 fixture 请求
  日志为准）」——不预取语义自始未变，仅观察载体对齐。
  v1.51 修订（phase-05 引擎提速轮 20261125，FR-05-01~07/D83/D84）：默认引擎改动态分段调度
  （scheduler 默认 dynamic）——块计数/块大小断言（场景 04/05/06/07）钉定 blocks 回退模式
  （Given 显式 scheduler = "blocks"），动态分段行为由 features/13-http-turbo.feature 承载；
  场景 16（AC-1）随新默认改判：默认并发 4→8（FR-05-05）、分块行动态分段口径（D83）。
  probe 连接复用（探测即数据流）语义由 13-http-turbo-01 承载（FR-05-07/D87）。
  v1.63 修订注（族A/族B 裁决落实轮 20261127，phase-05 v1.1/D90–D93）：blocks 引擎整体退役
  （D91）——场景 04/05/06/07 的「scheduler = "blocks" 块模型钉定」premise 失效，四场景退役
  留空不重排；动态分段行为由 features/13-http-turbo.feature 承载（v1.51 注记的承接关系
  不变）。族B：worker 块请求面 4xx/断连按 FR-05-04 失败粒度原地重试 + Transient 分类、
  Range 全拒降级完成不失败、有进展反复断连不失败（D93，旧预期废弃）；探测面（URL 级）
  4xx → Fatal 停等口径不变（场景 09/10/11 不受影响）。

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

  Scenario: 01-download-engine-04 [退役，留空不重排]
    # 原「分块总数按 1 MB 块计算且末块吸收余数」场景钉定 blocks 回退模式（v1.51），随 blocks
    # 引擎退役失去 premise（v1.63/族A 操作者裁决 20261127，phase-05 v1.1/D91）。
    # 场景序号保留追溯、不重排（修订纪律：删除类修订三层同步口径）；动态分段显示口径由
    # 13-http-turbo-01/02（D83）与 05-ui-pieces 元信息行承载。
    # 本场景无可执行步骤：QA 对应用例 QA-DE-04 同步退役（qa/01-download-engine-qa.md 注记）。

  Scenario: 01-download-engine-05 [退役，留空不重排]
    # 原「多连接动态领块乱序完成」场景钉定 blocks 回退模式（v1.51），随 blocks 引擎退役失去
    # premise（v1.63/族A，D91）；动态多连接乱序形态由 13-http-turbo-01/02（D83/FR-05-06）
    # 与 01-download-engine-16 承载。
    # 场景序号保留追溯、不重排（修订纪律：删除类修订三层同步口径）。
    # 本场景无可执行步骤：QA 对应用例 QA-DE-05 同步退役（qa/01-download-engine-qa.md 注记）。

  Scenario: 01-download-engine-06 [退役，留空不重排]
    # 原「队列临近结束多余连接转待命」场景钉定 blocks 回退模式（v1.51），随 blocks 引擎退役
    # 失去 premise（v1.63/族A，D91）；并发上限与待命观察由 13-http-turbo-02/07 承载。
    # 场景序号保留追溯、不重排（修订纪律：删除类修订三层同步口径）。
    # 本场景无可执行步骤：QA 对应用例 QA-DE-06 同步退役（qa/01-download-engine-qa.md 注记）。

  Scenario: 01-download-engine-07 [退役，留空不重排]
    # 原「续传不重写已下载块」场景钉定 blocks 封闭区间续传断言（v1.51），随 blocks 引擎退役
    # 失去 premise（v1.63/族A，D91）；动态续传「已提交字节零重复写入」由 01-resume-sidecar-03
    # 与 13-http-turbo-14 承载（开区间认领不覆盖已提交字节，D87）。
    # 场景序号保留追溯、不重排（修订纪律：删除类修订三层同步口径）。
    # 本场景无可执行步骤：QA 对应用例 QA-DE-07 同步退役（qa/01-download-engine-qa.md 注记）。

  Scenario: 01-download-engine-08 重定向跟随完成下载（原始 URL 唯一权威，各连接独立跟跳转）
    Given fixture 提供从 "<origin_url>" 重定向（<redirects> 次）至 "<landing_url>" 的下载链
    When 添加 origin URL 的下载任务并等待开始
    Then 下载经重定向最终完成且文件字节完整
    And 详情 URL 显示原始 URL "<origin_url>"（任务级唯一权威；跳转落点不进任务详情、不持久化，v1.28/FR-01-14）
    And fixture 记录的每个分块请求均以原始 URL 为起点独立跟随重定向（非共享探测落点）

    Examples:
      | origin_url                              | redirects | landing_url                          |
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
    Then 列表行速度、头部全局 ↓ 速度与头部内嵌单色面积流量图随下载持续每秒更新且数值来自真实传输（v1.30 术语校准：Sparkline 面板已由 FR-01-95 流量图取代）
    And 下载完成后列表与详情速度归零或显示完成态

  Scenario: 01-download-engine-15 等待下载槽位的任务不预取文件大小
    Given 下载槽位已被其他任务占满
    When 添加 fixture 文件 "<file>" 的下载任务
    Then 任务处于「等待中」且等待行不显示尺寸（尺寸未知不显示、无预取，以 fixture 请求日志为准）
    When 该任务获得空闲槽位开始下载
    Then 详情显示大小 <display_size>（探测完成后显示）

    Examples:
      | file       | display_size |
      | five-m.bin | 5.0 MB       |

  Scenario: 01-download-engine-16 默认配置下载 100 MB 文件端到端（AC-1）
    Given fixture 文件 "big-100m.bin" 大小 100 MB（稀疏构造）且支持 Range 请求
    And 源文件的正确 SHA-256 摘要已预置为伴随文件 "big-100m.bin.sha256"
    When 以默认配置（动态分段调度 · 默认并发 8，v1.51/FR-05-05）添加并下载该文件
    Then 详情分块行显示动态分段口径（随 <已下载>/<总字节> 推进，v1.51/D83）且实际并发不超过 8（无连接级明细——并发分块明细表已随 FR-01-81 v1.34 修订二移除，活跃连接经并发连接面板观察）
    And 分段交付为多连接乱序形态
    And 完成后任务转「已完成」且显示「SHA-256 校验成功」（源与目标校验值一致）
    And 目标文件字节数为 104857600
