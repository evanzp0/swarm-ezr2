# acceptance-mutation-manifest-begin
# {"version":1,"tested_at":"2026-11-25T03:30:30.155446619Z","feature_name":"12-reload-config 配置热加载（Ctrl+R）","feature_path":"/home/z/swarm-ezr2/project/features/12-reload-config.feature","background_hash":"74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b","implementation_hash":"unknown","scenarios":[{"index":4,"name":"12-reload-config-05 A 类代理端点变更运行任务热切存量连接不打断","scenario_hash":"f19f2eed66830433893b5d209d89a341582af36116e3055e3f4bab29e062d8e0","mutation_count":2,"result":{"Total":2,"Killed":2,"Survived":0,"Errors":0},"tested_at":"2026-11-25T03:26:51.663221825Z"},{"index":9,"name":"12-reload-config-10 CLI max-speed 覆盖热加载后保持","scenario_hash":"c1ec1d3c11c2132bec280cb813837d5c72111947eebb2b6b47d51be57486a865","mutation_count":3,"result":{"Total":3,"Killed":3,"Survived":0,"Errors":0},"tested_at":"2026-11-25T03:26:51.663221825Z"}]}
# acceptance-mutation-manifest-end

# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   12-reload-config-01 Ctrl+R 全局可达且成功热加载即时生效
#   12-reload-config-02 A 类全局限速热切运行中任务即时重定目标
#   12-reload-config-03 A 类 download_slots 热调不抢占不补位
#   12-reload-config-04 A 类 auto_retry 热切下次失败判定生效
#   12-reload-config-05 A 类代理端点变更运行任务热切存量连接不打断
#   12-reload-config-06 命名代理删除引用任务落库转直连与添加修改下拉重建
#   12-reload-config-07 B 类快照键仅影响后续新任务
#   12-reload-config-08 C 类键需重启生效且监听端口维持启动绑定
#   12-reload-config-09 任一键非法整体拒绝旧配置原样保留
#   12-reload-config-10 CLI max-speed 覆盖热加载后保持
#   12-reload-config-11 配置文件被删除热加载回默认配置
#   12-reload-config-12 无主目录环境热加载提示无配置文件
#   12-reload-config-13 帮助窗口全局节 Ctrl+R 条目
#   12-reload-config-14 命名代理改名引用任务落库转直连且新名入池不迁移引用
# 期号 01（phase-01，操作者 20261123 修订轮指令「根据 ezr-reload-cfg.md 中的方案，实现
# ezr Ctrl+R 配置热加载改动方案」）。
# 需求依据 project/mission/phase-01.md FR-01-119 与 D81（操作者 ezr-reload-cfg.md 改动方案
# 为权威输入：交互定义 / 6 改动点 / A·B·C·D 四类键语义 / 明确不做清单）。
# Inversion 阶段 1 受限退化沿 v1.45–v1.57 轮先例：操作者指令文本 + ezr-reload-cfg.md 全文
# 即阶段 1 答案；残余微口径 5 项（缺失文件臂 / 成功 toast 组成 / C 类提示触发 / 失败
# toast 形态 / 代理切换计数口径）按源裁决登记 D81①–⑤ 可回退，handoff 待批登记。
# 操作者疑虑核查：ezr-reload-cfg.md 与全仓库 0 处 EZR_DIR，代码/文档统一 EZR_HOME——
# 本 feature 全部配置路径表述按 EZR_HOME 口径（D16）。
# ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）；本轮不改注册表与 sidecar 格式、
# 快捷键说明只进帮助窗口。
# v1.57 修订（操作者修订轮指令 20261124「检查并修改下：config 中任何配置项的被改动，热加载config
# 后都不需要单独的生成 toast 消息，统一用一个 '配置已重新加载' 的toast 消息就可以了」；操作者
# chat 指令即修订裁决，Inversion 受限退化沿 v1.45–v1.60 十轮先例）：FR-01-119 ③ 成功反馈改判——
# 成功 toast 恒为固定文案 `✓ 配置已重新加载`、不追加任何可选段（v1.56「主干 + N/K 可选段」口径
# 退役：N 代理切换段/K 需重启生效段退役——发命令口径与 C 类语义不变，仅提示面收窄；失败 toast
# 与「无配置文件」臂不变；D81② 改判、③ 退役、⑤ 收窄、D82② 同步）：全部场景成功 toast 断言改判
# + 场景 05 去 N 段 + 场景 08 去 K 段并改名去「提示」字样 + Feature 描述同步。
# v1.56 修订（操作者 D82 闸门裁定，specifier 闸门落实轮 20261124；v1.55 轮合成确认闸门 = B
# 基本准确需局部修改——操作者 chat 裁决「② M 口径：热加载改代理不需要单独的消息提示，统一用
# reload config 的 toast 就行了；① 切换落库 和 ③ 改名臂——保持」）：D82② 改判结案、D82①③
# 追认保持——引用消失（删除/改名）不追加专属提示段，成功反馈统一 reload config 标准 toast
# `✓ 配置已热加载`（可选段仅按 D81②③⑤：代理切换 N / C 类需重启 K，M 段退役）：场景 06/14
# toast 断言改判（无 M 段）+ 场景 06 改名去「汇总提示」+ Feature 描述可选段清单同步。
# v1.55 修订（操作者 bug 修复轮 20261124「执行 six-pack/specifier, 修复需求 bug：把一个引用的
# [[common.proxy]] 中定义的命名代理改名或删除，ezr中热重载 config 后，添加和修改任务时仍旧
# 出现原有命名代理。应该是原来使用该代理的任务，自动变成直连。添加和修改任务的代理中删除
# 该命名代理」；Inversion 受限退化沿 v1.45–v1.58 九轮先例，操作者指令文本即阶段 1 答案）：
# FR-01-119 ⑤ 引用消失臂（删除/改名统一，v1.54「沿用 missing 语义悬挂引用」口径退役）——
# 场景 06 扩面（修改对话框下拉不注入旧名、引用任务落库转直连、重启不复活旧名、重加同名不复挂）
# + 场景 14 新增（改名臂：旧名消失任务转直连、新名入池不迁移引用、浮层内同步重建）；
# 残余微口径 3 项登记 D82①–③ 可回退（切换落库 / M 全状态口径 / 改名不迁移；②已由操作者
# 20261124 闸门裁定改判结案、①③追认保持——见 v1.56 修订块）。
# 代码实证：dialogs.rs open_modify 旧名注入形态与 resolve_proxy missing 悬挂引用为 bug 机制，
# 修复语义以落库转直连为锚（specifier 20261124 源码核证，产品端实现归下一 coder 轮）。
Feature: 12-reload-config 配置热加载（Ctrl+R）
  v1.65 修订注（MDM 完全移植轮 20261128，phase-05 v1.3/D97①）：B/C 类示例键同步——
  block_size_http（B 类，场景 07）与 throttle_burst_secs（C 类，场景 08）随四键退役
  （D97①）分别改判 min_split_kib（B 类沿袭）与 upload_read_concurrency（C 类键集收窄为
  listen_port / upload_read_concurrency）；dns_race_grace_ms / throttle_sleep_ms 同批退役；
  热加载语义（任一键非法整体拒绝、固定文案 toast、快照族）不受本轮影响。

  期号 01。按 Ctrl+R 重新读取 config 文件（EZR_HOME/config.toml，D16）并热加载，无需重启
  ezr（FR-01-119）。主界面与全部浮层内均响应（与 Ctrl+C 同级处理）；任一键非法整体拒绝、
  旧配置原样保留（原子性）；成功后单条固定文案 toast「✓ 配置已重新加载」（任何配置项改动
  均不追加可选段，v1.57/D81②）。配置键按四类生效语义：A 类立即全局、B 类快照仅影响后续新任务、
  C 类需重启（不再 toast 提示，v1.57）、D 类失败保护；CLI --max-speed 覆盖贯穿热加载保持。
  命名代理引用消失（删除或改名，v1.55/D82）= 引用任务自动转直连并落库、添加与修改任务
  对话框代理下拉重建且旧名不再出现（场景 06/14）。

  Scenario: 12-reload-config-01 Ctrl+R 全局可达且成功热加载即时生效
    Given ezr 以独立 EZR_HOME 启动且 config.toml 预置 [common] download_slots = <slots_before> 与一条命名代理 p1（http 127.0.0.1:18081）
    And 添加 fixture 文件任务并开始下载
    When 修改 $EZR_HOME/config.toml 将 download_slots 改为 <slots_after> 并按 Ctrl+R
    Then toast 显示 "✓ 配置已重新加载"
    And 队列栏显示 0/<slots_after>（download_slots 即时生效，无需重启）
    And 运行中任务不中断（进度持续推进）
    When 依次在 添加对话框打开时、帮助窗口打开时、命令窗口打开时 各改写 download_slots 为递异值并按 Ctrl+R
    Then 各浮层内 Ctrl+R 均触发热加载（toast "✓ 配置已重新加载"、队列栏槽位数逐次变化）
    And 浮层不因热加载关闭（对话框/帮助/命令窗口保持打开，Esc 原语义不变）

    Examples:
      | slots_before | slots_after |
      | 5            | 3           |

  Scenario: 12-reload-config-02 A 类全局限速热切运行中任务即时重定目标
    Given ezr 以独立 EZR_HOME 启动且 config.toml 预置 [common] global_max_download_speed = <limit_before>
    And fixture 提供慢速文件（限速下速度可观察）
    And 添加该文件任务并开始下载
    When 修改 config.toml 将 global_max_download_speed 改为 <limit_after> 并按 Ctrl+R
    Then toast 显示 "✓ 配置已重新加载"
    And 运行中任务的下载速度立即按 <limit_after> 重定目标（限速粒度内速度显示随之变化）
    And 任务不中断（进度持续推进、并发连接保持、无失败无重试）

    Examples:
      | limit_before | limit_after |
      | 0            | 512 KiB/s   |

  Scenario: 12-reload-config-03 A 类 download_slots 热调不抢占不补位
    Given ezr 以独立 EZR_HOME 启动且 config.toml 预置 [common] download_slots = 1
    And fixture 提供三个慢速文件
    And 添加三个任务（第 1 个下载中，其余等待中，队列栏 0/1）
    When 修改 config.toml 将 download_slots 改为 3 并按 Ctrl+R
    Then toast 显示 "✓ 配置已重新加载"
    And 等待中任务立即按列表顺序依次拿到槽位启动（队列栏 0/3）
    When 修改 config.toml 将 download_slots 改为 1 并按 Ctrl+R
    Then toast 再次显示 "✓ 配置已重新加载"
    And 运行中的 3 个任务均不被抢占（继续下载，队列栏仍 0/3）
    And 任一任务结束后空出的槽位不再补位（等待中任务不自动启动，allocate 只授予不回收）

  Scenario: 12-reload-config-04 A 类 auto_retry 热切下次失败判定生效
    Given ezr 以独立 EZR_HOME 启动且 config.toml 预置 [http] auto_retry = <auto_retry_before>
    And fixture 对文件 "<file>" 在传输约 30% 时模拟断连（可重试形态失败，可重复触发）
    And 添加该文件任务并开始下载
    Then 首次失败判定按 <auto_retry_before> 语义呈现（true = 自动重试倒计时进入重试；false = 直接已失败且无自动重试倒计时）
    When 修改 config.toml 将 auto_retry 改为 <auto_retry_after> 并按 Ctrl+R
    Then toast 显示 "✓ 配置已重新加载"
    And 下一次失败判定采用新值（再次触发同形失败时按 <auto_retry_after> 语义呈现）

    Examples:
      | auto_retry_before | auto_retry_after |
      | true              | false            |
      | false             | true             |

  Scenario: 12-reload-config-05 A 类代理端点变更运行任务热切存量连接不打断
    Given ezr 以独立 EZR_HOME 启动且 config.toml 含命名代理 p1（http <proxy_before>）
    And 代理 fixture 记录每次连接的来源端点（直连 / p1 旧端点 / p1 新端点）
    And 添加 HTTP 任务（代理 = p1）并开始下载
    When 修改 config.toml 将 p1 的 ip/port 改为 <proxy_after> 并按 Ctrl+R
    Then toast 显示 "✓ 配置已重新加载"（代理端点变化仍发切换命令，N 段不再呈现，v1.57/D81②⑤）
    And 存量连接不打断（当前块跑完、进度不回退、无失败）
    And 后续新发起的连接经 <proxy_after>（fixture 代理日志出现新端点来源）
    And 任务最终下载完成且文件字节完整

    Examples:
      | proxy_before      | proxy_after       |
      | 127.0.0.1:18081   | 127.0.0.1:18082   |

  Scenario: 12-reload-config-06 命名代理删除引用任务落库转直连与添加修改下拉重建
    Given ezr 以独立 EZR_HOME 启动且 config.toml 含命名代理 p1 与 p2（各指向独立代理 fixture 端点）
    And 代理 fixture 记录每次连接的来源端点
    And 添加两个 HTTP 任务分别引用 p1 与 p2 并开始下载
    When 修改 config.toml 删除 p1（保留 p2）并按 Ctrl+R
    Then toast 显示 "✓ 配置已重新加载"（引用消失不追加专属提示段，统一固定文案 toast，D82②/v1.57）
    And 引用 p1 的任务按直连继续下载（fixture 直连来源记录出现该任务的新连接，不中断不失败）
    And 引用 p2 的任务不变（无切换命令、速度继续、代理日志无新端点）
    And 添加对话框代理下拉选项重建为 直连 + p2（p1 不再出现）
    And 打开引用 p1 任务的修改对话框：代理下拉预填 直连 且选项 = 直连 + p2（p1 不被重新注入，D82①）
    And 重启 ezr 后引用 p1 任务的代理仍为 直连（任务级代理选择已随注册表落库，旧名不复活）
    When 重加 p1 到 config.toml（同 type/ip/port）并按 Ctrl+R
    Then p1 重新出现在添加与修改对话框代理下拉（池条目恢复正常）
    And 引用 p1 的任务保持直连不复挂（其选择已落库直连，不随重加自动迁移）
    And 再次按 Ctrl+R toast 仍仅 "✓ 配置已重新加载"（任何配置项改动均不追加可选段，v1.57/D81②）

  Scenario: 12-reload-config-07 B 类快照键仅影响后续新任务
    # v1.65/D97① 修订：B 类示例键 block_size_http 随键退役（MDM 移植，D92）——改用
    # min_split_kib（[http] 表 B 类快照键沿袭）承载同一语义。
    Given ezr 以独立 EZR_HOME 启动且 config.toml 预置 [http] min_split_kib = 256、http_concurrency = 4、max_retries = 5
    And 添加 HTTP 任务 a 并开始下载
    When 修改 config.toml 将 min_split_kib 改为 512、http_concurrency 改为 8、max_retries 改为 9 并按 Ctrl+R
    Then toast 显示 "✓ 配置已重新加载"
    And 运行中任务 a 的切分粒度/并发/重试上限维持添加时烘焙值（分段行为与并发面板连接数不变）
    When 添加新 HTTP 任务 b（同 fixture 文件族）并开始下载
    Then 任务 b 按新值烘焙（切分粒度 512 KiB、并发 8、重试上限 9）
    And 添加对话框并发字段预填 8（http_concurrency 新默认进入对话框预填）

  Scenario: 12-reload-config-08 C 类键需重启生效且监听端口维持启动绑定
    Given ezr 以独立 EZR_HOME 启动且 config.toml 预置 [bt] upload_read_concurrency = 128 与 [bt] listen_port = <port>
    And BT fixture（tracker + seeder）已启动
    And 添加 BT 任务 t1 并开始下载（tracker fixture 记录 announce 来源端口 = <port>）
    When 修改 config.toml 将 upload_read_concurrency 改为 256（listen_port 不变）并按 Ctrl+R
    Then toast 显示 "✓ 配置已重新加载"（C 类变化不再追加重启提示段，v1.57/D81③）
    And 运行中任务 t1 传输行为无异常（上传读盘并发维持启动值、进度持续推进）
    When 修改 config.toml 将 listen_port 改为 <port2> 并按 Ctrl+R
    Then toast 显示 "✓ 配置已重新加载"（本次仅 listen_port 变化，重启提示段不再呈现，v1.57/D81③）
    And BT 监听端口仍为 <port>（新添加 BT 任务的 tracker announce 来源端口 = <port>，listen_port 引擎启动绑定一次）
    When config.toml 未再改动再次按 Ctrl+R
    Then toast 显示 "✓ 配置已重新加载"（无任何可选段——任何配置项改动统一固定文案，v1.57/D81②）

    Examples:
      | port  | port2 |
      | 51444 | 51555 |

  Scenario: 12-reload-config-09 任一键非法整体拒绝旧配置原样保留
    Given ezr 以独立 EZR_HOME 启动且 config.toml 预置 [common] download_slots = 5、global_max_download_speed = 1 MiB/s
    And 添加慢速 fixture 文件任务并开始下载（限速 1 MiB/s 生效中）
    When 修改 config.toml 写入非法值（[common] download_slots = "abc"）并按 Ctrl+R
    Then toast 显示 "⚠ 配置热加载失败：…"（含 try_load 逐键报错文案「键 common.download_slots」语义，单行显示无换行）
    And 旧配置原样保留（队列栏仍 0/5、运行中任务限速仍 1 MiB/s、无半新半旧状态）
    And 任务不中断（进度持续推进）
    When 修改 config.toml 同时写入一个合法新值与一个非法值（global_max_download_speed = 2 MiB/s 与 download_slots = "abc"）并按 Ctrl+R
    Then 整体拒绝（toast 同上报错，合法键 global_max_download_speed 也不生效，仍 1 MiB/s）
    When 修正全部非法值后按 Ctrl+R
    Then toast 显示 "✓ 配置已重新加载"（全部合法后热加载成功，新值生效）

  Scenario: 12-reload-config-10 CLI max-speed 覆盖热加载后保持
    Given ezr 以 --max-speed <cli_limit> 启动且 config.toml 预置 [common] global_max_download_speed = <file_limit>（与 <cli_limit> 不同值）
    And fixture 提供慢速文件
    And 添加该文件任务并开始下载（速度受 <cli_limit> 限制）
    When 修改 config.toml 将 global_max_download_speed 改为 <limit_after> 并按 Ctrl+R
    Then toast 显示 "✓ 配置已重新加载"
    And 生效全局限速仍为 <cli_limit>（CLI 覆盖贯穿热加载重应用，不被文件值冲掉）
    And 运行中任务速度与头部仪表按 <cli_limit> 呈现

    Examples:
      | cli_limit  | file_limit | limit_after |
      | 256 KiB/s  | 1 MiB/s    | 2 MiB/s     |

  Scenario: 12-reload-config-11 配置文件被删除热加载回默认配置
    Given ezr 以独立 EZR_HOME 启动且 config.toml 预置 [common] download_slots = 2 与一条命名代理 p1
    And 队列栏显示 0/2
    When 删除 $EZR_HOME/config.toml 并按 Ctrl+R
    Then toast 显示 "✓ 配置已重新加载"
    And 默认配置整体生效（队列栏 0/5——download_slots 回默认 5、全局限速回默认不限速）
    And $EZR_HOME/config.toml 未被重新生成（模板生成属启动逻辑 FR-01-85，热加载不重生成）
    And 添加对话框代理下拉仅剩 直连（命名代理池随默认配置清空）

  Scenario: 12-reload-config-12 无主目录环境热加载提示无配置文件
    Given 以 HOME、USERPROFILE 与 EZR_HOME 全部未设置的环境启动 ezr（实例锁回退 <系统临时目录>/ezr.lock，FR-01-72 v1.4 注记；配置按默认加载）
    And 应用正常进入主界面（任务列表可正常操作）
    When 按 Ctrl+R
    Then toast 显示 "无配置文件"
    And 应用不退出、配置保持默认（任务列表可继续操作）

  Scenario: 12-reload-config-13 帮助窗口全局节 Ctrl+R 条目
    Given ezr 以独立 EZR_HOME 启动
    When 按 H 打开帮助窗口
    Then 【全局】组含条目 "Ctrl+R 重新加载配置文件（热加载，无需重启）"
    And 帮助窗口可滚动容纳新增条目（窗口尺寸不变，内容行数增加走滚动）
    And 帮助内按 Ctrl+R 仍触发热加载且帮助窗口不关闭（全局可达，场景 01 同口径）

  Scenario: 12-reload-config-14 命名代理改名引用任务落库转直连且新名入池不迁移引用
    Given ezr 以独立 EZR_HOME 启动且 config.toml 含命名代理 p1 与 p2（各指向独立代理 fixture 端点）
    And 代理 fixture 记录每次连接的来源端点
    And 添加两个 HTTP 任务分别引用 p1 与 p2 并开始下载
    When 修改 config.toml 将 p1 改名为 p1x（type/ip/port/认证不变）并按 Ctrl+R
    Then toast 显示 "✓ 配置已重新加载"（旧名消失不追加专属提示段，统一固定文案 toast，D82②③/v1.57）
    And 引用 p1 的任务按直连继续下载（fixture 直连来源记录出现该任务的新连接，不中断不失败）
    And 引用 p2 的任务不变（无切换命令、速度继续、代理日志无新端点）
    And 添加对话框代理下拉选项重建为 直连 + p1x + p2（旧名 p1 不再出现，新名 p1x 按配置顺序出现）
    And 引用 p1 任务的修改对话框代理预填 直连 且选项 = 直连 + p1x + p2（无旧名，D82①）
    When 在引用 p1 任务的修改对话框保持打开时改写 config.toml 将 p1x 改名回 p1 并按 Ctrl+R
    Then toast 显示 "✓ 配置已重新加载"（无任务引用 p1x，不追加任何可选段）
    And 修改对话框不关闭且代理下拉选项即时重建为 直连 + p1 + p2（浮层内同步，场景 01 同口径）
    And 受影响任务代理预填仍为 直连（不自动复挂回 p1，引用不自动迁移，D82③）
    And 关闭浮层后重启 ezr：该任务代理仍为 直连（落库持久化，旧名不复活）
