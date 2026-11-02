# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   10-bt-anti-leech-01 吸血判定——只下载不上传（leech）自动封禁（下载中任务）
#   10-bt-anti-leech-02 做种任务豁免互惠判定（做种中不按比率封禁）
#   10-bt-anti-leech-03 连续坏块（bad_piece）自动封禁
#   10-bt-anti-leech-04 协议违规（protocol）自动封禁
#   10-bt-anti-leech-05 自动封禁可观察：断开 + toast + blacklist.yaml 即时落盘
#   10-bt-anti-leech-06 blacklist.yaml 启动加载与跨重启生效（含手工编辑解封）
#   10-bt-anti-leech-07 黑名单全面生效：拨出候选 / 入连 accept / tracker peers 吸纳三面过滤
#   10-bt-anti-leech-08 手动 B 键封禁与自动封禁共存且同文件持久化
# 期号 02（phase-02，操作者 bug18 轮指令五项功能之三「真正的自动反吸血」）。
# 需求依据 project/mission/phase-02.md FR-02-30 与 phase-02 D67。
# 策略参考：qBittorrent/libtorrent 无基于贡献比率的自动封禁（仅协议违规断开 + 手动 ban）
# → 主参考 BitComet 反吸血（PSR 互惠惩罚思路）并按 ezr 现状设计三判据（D67）：
#   A leech（只拿不给，下载中任务豁免做种）/ B bad_piece（连续坏块）/ C protocol（协议违规）。
# 黑名单持久化 = ~/.ezr/blacklist.yaml（EZR_HOME 可重定位，与 config 同目录；不放 config）。
Feature: 10-bt-anti-leech BT 自动反吸血与黑名单

  期号 02。引擎自动识别吸血 peer 并断开 + 封禁 IP，封禁写入独立 blacklist.yaml
  持久化；黑名单跨重启生效，拒绝一切入连/拨出/候选吸纳；B 键手动封禁共用同一名单。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 BT fixture 已就绪（HTTP/UDP tracker + seeder/leecher peer 可编程控制，握手与消息日志可观察）
    And 终端尺寸 120×44

  Scenario: 10-bt-anti-leech-01 吸血判定——只下载不上传（leech）自动封禁
    Given BT 任务 T 下载中（非做种）
    And leecher fixture L 挂入 tracker（L 连接本端后只发 request 接收数据，从不回传任何 piece，连接存活 <window> 秒以上）
    When 本端对 L 累计上传达到 <up_thresh>（L 对本端累计下载 <down_thresh> 以内）
    Then 本端断开 L 连接且 L 的 IP 进入黑名单（toast "⛔ 已封禁吸血对端 <ip>（只下载不上传）"）
    And blacklist.yaml 出现该条目（reason = leech）
    And 后续调度不再拨出 L（候选过滤）；L 重连（入连）被拒绝

    Examples:
      | up_thresh   | down_thresh | window |
      | 1 MiB       | 64 KiB      | 120    |

  Scenario: 10-bt-anti-leech-02 做种任务豁免互惠判定
    Given BT 任务 T 下载完成转做种中
    And leecher fixture L 只从本端拉取数据且从不回传（纯 leecher 合法形态）
    When 上传持续超过 leech 判定阈值（up ≥ 1 MiB 且存活 ≥ 120s）
    Then 不触发 leech 封禁（连接保持，上传继续；做种无私上传合法——BitComet 口径）
    And 黑名单不出现 L 条目、blacklist.yaml 不变

  Scenario: 10-bt-anti-leech-03 连续坏块（bad_piece）自动封禁
    Given BT 任务 T 下载中，seeder fixture S 对 request 返回与元数据 pieces 校验不符的 piece 数据
    When 同一连接 S 累计导致 <bad_count> 次 piece 校验失败（均归属 S 提供的数据）
    Then 本端断开 S 且封禁其 IP（toast "⛔ 已封禁对端 <ip>（连续提供坏块）"）
    And blacklist.yaml 出现该条目（reason = bad_piece）；坏块 piece 照常重新调度下载（既有重下口径不变）

    Examples:
      | bad_count |
      | 3         |

  Scenario: 10-bt-anti-leech-04 协议违规（protocol）自动封禁
    Given BT 任务 T 运行中，恶意 fixture M 与本端握手成功
    When M 发送 "<violation>"（<count> 形态）
    Then 本端断开 M 且封禁其 IP（toast "⛔ 已封禁对端 <ip>（协议违规）"）
    And blacklist.yaml 出现该条目（reason = protocol）

    Examples:
      | violation                                          | count      |
      | 未知消息 id 或帧长度与载荷不符的破坏性消息          | 首次即封   |
      | 非法区间 request（不存在的 piece/越界区间）          | 重复 3 次  |

  Scenario: 10-bt-anti-leech-05 自动封禁可观察：断开 + toast + blacklist.yaml 即时落盘
    Given 任一自动封禁判定命中（leech/bad_piece/protocol 之一）
    When 判定触发
    Then 对应连接有序关闭（沿 FR-02-27② 优雅关闭口径）且 toast 在本端可见（前缀 ⛔，3 秒自动消失）
    And blacklist.yaml 在封禁后即时可读出该条目（ip/reason/banned_at 三字段）
    And 并发面板该连接行约一进度帧内消失（B 键同款观察面）

  Scenario: 10-bt-anti-leech-06 blacklist.yaml 启动加载与跨重启生效（含手工编辑解封）
    Given blacklist.yaml 预置条目（ip = <ip>，reason = leech）
    When ezr 启动（或退出重启）
    Then <ip> 在黑名单内：候选拨出/入连 accept 均被拒绝且无相关连接
    And 手工从 blacklist.yaml 删除该条目并重启 ezr 后解封（该 IP 可正常连接）
    And 文件缺失/损坏（非法 YAML）时按空名单启动 + 启动警告（黑名单非致命，D67）

    Examples:
      | ip          |
      | 192.0.2.7   |

  Scenario: 10-bt-anti-leech-07 黑名单全面生效：拨出候选 / 入连 accept / tracker peers 吸纳三面过滤
    Given 黑名单含 <ip>
    When tracker 响应 peers 列表中包含 <ip> 且 <ip> 同时尝试入连
    Then 三面均拒绝：announce peers 吸纳候选时过滤、调度拨出时过滤、入连 accept 后立即拒绝（沿既有 is_blacklisted 口径 + 持久化名单）

    Examples:
      | ip          |
      | 192.0.2.7   |

  Scenario: 10-bt-anti-leech-08 手动 B 键封禁与自动封禁共存且同文件持久化
    Given BT 任务并发面板选中一连接
    When 按 B 断开（既有手动封禁）随后某 peer 触发自动封禁
    Then 两条目共存于 blacklist.yaml（reason 分别为 manual 与自动原因）且重启后均生效
    And B 键 toast 沿既有 "✂ 已断开 <ip> 并加入黑名单（剩余 <n> 条）" 口径（计数含自动条目）
