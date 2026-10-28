# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   02-named-proxy-01 多个命名代理（含认证）在添加对话框可选
#   02-named-proxy-02 任务级代理生效：不同任务走不同代理/直连
#   02-named-proxy-03 命名条目非法（重名/空名）忽略并 toast
#   02-named-proxy-04 认证代理经用户名密码认证可用；认证信息不写入界面与日志
#   02-named-proxy-05 引用失效（配置删除该命名代理）→ 直连 + toast
#   02-named-proxy-06 旧注册表（无代理字段 / 旧 "global" 值）按直连加载
#   02-named-proxy-07 代理地址无效（无法构成合法代理 url）→ toast + 直连回退
#   02-named-proxy-08 代理类型 type 三值显式生效（http / https / socks5；内部 url 由 type+ip+port 构造）
#   02-named-proxy-09 代理类型 type 缺失 → 条目作废 + 警告（v1.9 type 必填）
#   02-named-proxy-10 代理类型 type 未知值 → 条目作废 + 警告（三值枚举之外不处理）
#   02-named-proxy-11 socks5 型代理缺 username → 条目作废 + 警告（v1.8 FR-01-93 凭证必填）
#   02-named-proxy-12 socks5 型代理缺 password → 条目作废 + 警告（v1.8 FR-01-93 凭证必填）
#   02-named-proxy-13 无凭证代理 → 匿名代理正常加载（http 与 https 同级，v1.8/v1.9 FR-01-93）
#   02-named-proxy-14 代理凭证只填其一 → 条目作废 + 警告（http 与 https 同级成对原则，v1.8/v1.9）
#   02-named-proxy-15 带凭证代理 → 条目保留且经认证可用于下载（socks5 RFC 1929 / https basic auth）
#   02-named-proxy-16 ip 缺失或空白 → 条目作废 + 警告（v1.9）
#   02-named-proxy-17 port 缺失、非整数或越界 → 条目作废 + 警告（v1.9，合法域 1..=65535）
#   02-named-proxy-18 IPv6 字面量 ip 可用（内部 url 方括号构造，v1.9）
# （v1.6：原 03「默认选中随全局而定」随旧全局 proxy 键退役删除，序号重排；
#   v1.7：type 两值合并（http/socks5），矛盾不再作废——url scheme 唯一事实来源；
#   v1.8：FR-01-93 凭证按类型分级——socks5 必填 / http 可选（缺省匿名，半填作废），新增场景 11–15；
#   v1.9：条目字段重构（name/type/ip/port/username/password），url 键退役——08–10 按新语义重写
#   （三值枚举 / type 缺失作废 / type 未知作废；内部代理 url 由 type+ip+port 构造，其正确性以
#   「连接以 type 对应协议形态抵达 ip:port」为可观察行为），11–15 补 https 相位（https 匿名 /
#   成对 / 半填，凭证规则与 http 同级），新增 16–18（ip 校验 / port 校验 / IPv6 字面量）；
#   01–07 编号语义不变，仅条目字段形态由 url 机械改为 ip/port（v1.7 的「type 与 url 前缀矛盾
#   保留按 url 处理」「type 缺省从 url scheme 推断」「url 为空/url 无 scheme 作废」等 url 相关
#   场景语义随 url 键退役废止））
Feature: 02-named-proxy 命名代理（http/https/socks5）与任务级选择

  期号 01。依据 project/mission/phase-01.md FR-01-86（v1.5；v1.9 字段重构）+ FR-01-89
  （v1.6 建；v1.7 改判；v1.9 重写）+ FR-01-93（v1.8；v1.9 增补）：配置支持 [[proxies]]
  命名代理数组，每项字段 = name（非空且唯一）/ type（必填，http / https / socks5 三值）/
  ip（非空；允许 IP 字面量或主机名）/ port（整数 1..=65535）/ 可选 username/password；
  可配置多个。url 键已退役（v1.9）：配置不再读取，按未知键忽略（FR-01-71 口径，零警告、
  零迁移）。type 为唯一事实来源：内部代理 url 由 type+ip+port 构造——http →
  http://{ip}:{port}、https → https://{ip}:{port}（代理自身走 TLS，basic auth 在 TLS 会话内
  生效）、socks5 → socks5://{ip}:{port}；三类型均同时服务 http 与 https 下载目标（http/https
  型经 CONNECT 隧道、socks5 型经 SOCKS 隧道）。type 缺失或取值未知 → 条目作废 + 启动警告
  一次；ip 缺失或空白、port 缺失/非整数/越界（非 1..=65535）同样作废 + 警告（口径同空名/
  重名：忽略该条目、不阻塞启动、文案不回显凭证值）。凭证规则（FR-01-93）：socks5 型必填
  （trim 后均非空，SOCKS5 认证为 RFC 1929 user/pass 握手，不支持匿名）；http 与 https 型
  同级可选——都缺省 = 匿名代理（正常加载无警告）、成对 = HTTP basic auth（https 型在 TLS
  会话内生效）、只填其一 → 条目作废 + 警告。socks5 凭证由引擎经内部认证 url 的 userinfo
  传递（reqwest percent-decode 后走 RFC 1929 握手；Proxy::basic_auth 对 socks 代理无效，
  仅用于 http/https 型——v1.5 起 socks5 凭证静默失效，v1.8 一并修复）。IPv6 字面量 ip：
  内部 url 构造加方括号（可观察行为 = IPv6 字面量可作为 ip 配置且代理可用）。凭证不进
  日志/toast/界面文案；ProxyEndpoint.url 保持无凭证形态，认证 url 仅在引擎构建 client 时
  内部构造（FR-01-93）。任务持有任务级代理选择（两态：直连 / 命名引用）。旧全局 proxy
  配置键已退役（FR-01-90）。代理仅配置文件途径，不读取环境变量（FR-01-61 沿用）。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件
    And 本地 fixture 代理（可区分记录经手请求，支持 --name 多实例与 http/https/socks5 形态）已启动

  Scenario: 02-named-proxy-01 多个命名代理（含认证）在添加对话框可选
    Given 配置文件含 <count> 个 [[proxies]] 条目（name/type/ip/port 各异，含含 username/password 的条目）
    When 打开添加任务对话框并展开「代理」下拉
    Then 下拉选项 = 「直连」+ <count> 个命名代理（按配置顺序）
    And 每个命名代理按「名（type）」形态显示类型标注（http/https/socks5 三值，FR-01-89）
    And 认证信息（username/password）不出现在任何选项文案中

    Examples:
      | count |
      | 3     |

  Scenario: 02-named-proxy-02 任务级代理生效：不同任务走不同代理/直连
    Given 配置文件含两个命名代理 proxy-a/proxy-b（fixture 多实例）
    And 任务 A 添加时代理选 proxy-a、任务 B 选 proxy-b、任务 C 选直连（同服务器文件各一份）
    When 三任务并行下载至完成
    Then proxy-a 日志记录 A 的请求、proxy-b 日志记录 B 的请求、两日志均无 C 的请求
    And 三任务均正常完成且文件内容正确（代理不改变传输结果）

    Examples:
      | file-a     | file-b     | file-c     |
      | one-m.bin  | one-m.bin  | one-m.bin  |

  Scenario: 02-named-proxy-03 命名条目非法（重名/空名）忽略并 toast
    Given 配置文件含 [[proxies]] 条目：<bad_entries>（重名/空名）
    And 另含一个合法命名代理 valid
    When 启动 ezr 并打开添加任务对话框展开「代理」下拉
    Then 非法条目不出现于下拉；合法条目 valid 在位
    And 启动时 toast 提醒一次（说明忽略了非法代理条目，文案不含敏感信息）
    And 启动与下载行为不因非法条目受阻（容错口径同 FR-01-71）

    Examples:
      | bad_entries                |
      | 两个 name 均为 dup 的条目   |
      | name 为空字符串的条目       |

  Scenario: 02-named-proxy-04 认证代理经用户名密码认证可用；认证信息不写入界面与日志
    Given 配置文件含命名代理 auth-proxy（type = "http"，ip/port 指向需认证的 fixture 代理，username/password 正确）
    When 添加任务选 auth-proxy 并下载至完成
    Then 下载成功（代理侧认证通过）
    And 界面（列表/详情/代理下拉/toast）与 ezr 日志均不含 username/password 明文

    Examples:
      | file       |
      | one-m.bin  |

  Scenario: 02-named-proxy-05 引用失效（配置删除该命名代理）→ 直连 + toast
    Given 会话 1：配置含命名代理 gone-proxy，添加任务选 gone-proxy 后退出（注册表已存）
    When 会话 2：配置删除 gone-proxy 条目后启动并让该任务继续下载至完成
    Then 任务连接按直连执行并下载成功
    And 启动后首次使用该任务时 toast 提醒一次（引用的命名代理不存在，已按直连）

    Examples:
      | file       |
      | one-m.bin  |

  Scenario: 02-named-proxy-06 旧注册表（无代理字段 / 旧 "global" 值）按直连加载
    Given 注册表为 v1.5 形态（任务代理字段缺省或为 "global"），配置不再设全局 proxy（键已退役）
    When 启动 ezr 并让该任务继续下载至完成
    Then 任务连接按直连执行（旧 "global" 值映射为直连，FR-01-90 兼容口径）
    And 下载成功且文件内容正确

    Examples:
      | file       |
      | one-m.bin  |

  Scenario: 02-named-proxy-07 代理地址无效 → toast + 直连回退
    Given 配置文件含命名代理 bad-proxy（type = "http"，ip = "bad host"——含空格，无法与 port 构成合法代理地址）
    When 添加任务选 bad-proxy 并下载
    Then 该任务连接按直连执行并下载成功（运行期客户端构建失败回退，FR-01-86）
    And 使用该任务时 toast 提醒一次（代理地址无效，该连接已按直连）

    Examples:
      | file       |
      | one-m.bin  |

  Scenario: 02-named-proxy-08 代理类型 type 三值显式生效（http / https / socks5；内部 url 由 type+ip+port 构造）
    Given 配置文件含三个命名代理：<rows>（ip 均为 127.0.0.1，port 各指向对应协议形态的 fixture 代理）
    When 分别添加任务选用各代理并下载至完成
    Then 各任务连接以 type 对应的协议形态抵达 ip:port（内部代理 url 由 type+ip+port 构造的可观察面：
      http 型经普通 HTTP 代理请求与 CONNECT 隧道、https 型经代理自身 TLS 会话、socks5 型经 SOCKS
      握手——fixture 代理日志可证；https 型监听形态由 QA 会话扩展 ezr-proxy 后补证）
    And 三任务均下载成功且文件内容正确（三类型均同时服务 http 与 https 下载目标）
    And 添加/修改对话框代理下拉中各代理按「名（type）」显示类型标注

    Examples:
      | name     | type   | port  | file       |
      | p-http   | http   | 18766 | one-m.bin  |
      | p-https  | https  | 18767 | one-m.bin  |
      | p-socks5 | socks5 | 18768 | one-m.bin  |

  Scenario: 02-named-proxy-09 代理类型 type 缺失 → 条目作废 + 警告（v1.9 type 必填）
    Given 配置文件含命名代理 no-type（ip/port 指向 fixture 代理但未写 type 键）
    And 另含一个合法命名代理 valid
    When 启动 ezr 并打开添加任务对话框展开「代理」下拉
    Then 条目 no-type 不出现于下拉（type 必填，缺失即作废；url 键已退役无从推断）
    And 启动时 toast 警告一次（说明该代理条目因类型缺失被忽略）
    And 合法条目 valid 在位，选用 valid 下载至完成（启动与下载不因作废条目受阻）

    Examples:
      | file       |
      | one-m.bin  |

  Scenario: 02-named-proxy-10 代理类型 type 未知值 → 条目作废 + 警告（三值枚举之外不处理）
    Given 配置文件含命名代理 bad-type（type = "ftp"——非 http/https/socks5，ip/port 指向 fixture 代理）
    And 另含一个合法命名代理 valid
    When 启动 ezr 并打开添加任务对话框展开「代理」下拉
    Then 条目 bad-type 不出现于下拉（type 三值枚举之外即作废；v1.7「保留按 url 处理」口径随 url 键退役废止）
    And 启动时 toast 警告一次（说明该代理条目因类型未知被忽略）
    And 合法条目 valid 在位，选用 valid 下载至完成（启动与下载不因作废条目受阻）

    Examples:
      | file       |
      | one-m.bin  |

  Scenario: 02-named-proxy-11 socks5 型代理缺 username → 条目作废 + 警告（v1.8 FR-01-93 凭证必填）
    Given 配置文件含命名代理 s5-no-user（type = "socks5"，ip/port 指向 fixture socks5 代理，password 已填而 username 缺失）
    And 另含一个合法命名代理 valid
    When 启动 ezr 并打开添加任务对话框展开「代理」下拉
    Then 条目 s5-no-user 不出现于下拉（socks5 凭证必填，缺 username 即作废）
    And 启动时 toast 警告一次（说明该代理条目因凭证缺失被忽略，文案不含凭证值）
    And 合法条目 valid 在位，选用 valid 下载至完成（启动与下载不因作废条目受阻）

    Examples:
      | file       |
      | one-m.bin  |

  Scenario: 02-named-proxy-12 socks5 型代理缺 password → 条目作废 + 警告（v1.8 FR-01-93 凭证必填）
    Given 配置文件含命名代理 s5-no-pass（type = "socks5"，ip/port 指向 fixture socks5 代理，username 已填而 password 缺失）
    And 另含一个合法命名代理 valid
    When 启动 ezr 并打开添加任务对话框展开「代理」下拉
    Then 条目 s5-no-pass 不出现于下拉（socks5 凭证必填，缺 password 即作废）
    And 启动时 toast 警告一次（说明该代理条目因凭证缺失被忽略，文案不含凭证值）
    And 合法条目 valid 在位，选用 valid 下载至完成（启动与下载不因作废条目受阻）

    Examples:
      | file       |
      | one-m.bin  |

  Scenario: 02-named-proxy-13 无凭证代理 → 匿名代理正常加载（http 与 https 同级，v1.8/v1.9 FR-01-93）
    Given 配置文件含命名代理 anon-p（type = <ptype>，ip/port 指向对应 fixture 代理，未配置 username 与 password）
    When 添加任务选 anon-p 并下载至完成
    Then 启动无凭证相关警告（匿名代理为 http/https 型合法形态，条目正常加载）
    And 请求经对应 fixture 代理执行（代理日志可证）且任务下载成功
    And 代理侧无认证记录（匿名形态，未发送代理认证）

    Examples:
      | ptype |
      | http  |
      | https |

  Scenario: 02-named-proxy-14 代理凭证只填其一 → 条目作废 + 警告（http 与 https 同级成对原则，v1.8/v1.9）
    Given 配置文件含命名代理 half-p（type = <ptype>，ip/port 指向对应 fixture 代理，<half_cred>）
    And 另含一个合法命名代理 valid
    When 启动 ezr 并打开添加任务对话框展开「代理」下拉
    Then 条目 half-p 不出现于下拉（username 与 password 需成对配置，只填其一即作废）
    And 启动时 toast 警告一次（说明 username 与 password 需成对配置，文案不含凭证值）
    And 合法条目 valid 在位，选用 valid 下载至完成（启动与下载不因作废条目受阻）

    Examples:
      | ptype | half_cred                    |
      | http  | 只填 username 未填 password  |
      | http  | 只填 password 未填 username  |
      | https | 只填 username 未填 password  |
      | https | 只填 password 未填 username  |

  Scenario: 02-named-proxy-15 带凭证代理 → 条目保留且经认证可用于下载（socks5 RFC 1929 / https basic auth）
    Given 配置文件含命名代理 auth-p（type = <ptype>，ip/port 指向需认证的对应形态 fixture 代理，username/password 正确）
    When 添加任务选 auth-p 并下载至完成
    Then 条目 auth-p 在下拉正常显示（类型标注 <ptype>，选项文案不含凭证）
    And 下载成功且文件内容正确（认证按类型生效——socks5 经 RFC 1929 user/pass 握手、
      https 经 TLS 会话内 basic auth，代理侧认证日志可证）
    And 界面（列表/详情/代理下拉/toast）与 ezr 日志均不含 username/password 明文（socks5 凭证仅经内部认证 url userinfo 传递，FR-01-93）

    Examples:
      | ptype  |
      | socks5 |
      | https  |

  Scenario: 02-named-proxy-16 ip 缺失或空白 → 条目作废 + 警告（v1.9）
    Given 配置文件含命名代理 bad-ip（type = "http"，port 合法，<bad_ip>）
    And 另含一个合法命名代理 valid
    When 启动 ezr 并打开添加任务对话框展开「代理」下拉
    Then 条目 bad-ip 不出现于下拉（ip 必填非空，缺失或空白即作废）
    And 启动时 toast 警告一次（说明该代理条目被忽略，文案不含敏感信息）
    And 合法条目 valid 在位，选用 valid 下载至完成（启动与下载不因作废条目受阻）

    Examples:
      | bad_ip           |
      | ip 键缺失        |
      | ip 为空字符串    |
      | ip 仅空白字符    |

  Scenario: 02-named-proxy-17 port 缺失、非整数或越界 → 条目作废 + 警告（v1.9，合法域 1..=65535）
    Given 配置文件含命名代理 bad-port（type = "http"，ip 合法，<bad_port>）
    And 另含一个合法命名代理 valid
    When 启动 ezr 并打开添加任务对话框展开「代理」下拉
    Then 条目 bad-port 不出现于下拉（port 须为 1..=65535 内的整数，缺失/非整数/越界即作废）
    And 启动时 toast 警告一次（说明该代理条目被忽略）
    And 合法条目 valid 在位，选用 valid 下载至完成（启动与下载不因作废条目受阻）

    Examples:
      | bad_port         |
      | port 键缺失      |
      | port = "eighty"  |
      | port = 0         |
      | port = 65536     |
      | port = -1        |

  Scenario: 02-named-proxy-18 IPv6 字面量 ip 可用（内部 url 方括号构造，v1.9）
    Given 配置文件含命名代理 s6-proxy（type = "http"，ip 为 IPv6 字面量 "::1"，port 指向 IPv6 回环上的 fixture http 代理）
    When 添加任务选 s6-proxy 并下载至完成
    Then 条目 s6-proxy 正常加载并在下拉按「s6-proxy（http）」显示（IPv6 字面量可作为 ip 配置，FR-01-89）
    And 下载成功且文件内容正确（连接经该 IPv6 代理执行，代理日志可证——内部 url 对 IPv6 字面量加方括号构造，对配置与界面透明）
    And 界面与日志不因 ip 含冒号出现异常文案或崩溃

    Examples:
      | file       |
      | one-m.bin  |
