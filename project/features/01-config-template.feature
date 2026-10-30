# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   01-config-template-01 首次运行生成全注释默认模板（FR-01-85）
#   01-config-template-02 已有配置文件不重写不覆盖
#   01-config-template-03 损坏配置文件不触发重写且不阻塞启动
#   01-config-template-04 生成失败静默跳过不阻塞启动
#   01-config-template-05 EZR_HOME 重定位协同生成
Feature: 01-config-template 配置模板自动生成

  期号 01。依据 project/mission/phase-01.md FR-01-85（v1.4 操作者指令）与 D17：
  配置文件不存在时按默认值生成全部行被注释的配置模板，每个参数注明用途与取值范围；
  已存在文件（含损坏）不重写，生成失败静默不阻塞启动。模板位置与解析语义沿 FR-01-71
  （含 EZR_HOME 重定位，D16）。v1.5 修订：键名 default_concurrency → http_concurrency
  （FR-01-88）；模板同步收录 [[proxies]] 命名代理注释示例块（FR-01-86，D18）。
  v1.9 修订（条目字段重构，FR-01-86/89）：proxies 段注释更新为 ip/port/type 字段说明 +
  三类型语义（https = 代理自身走 TLS）+ 凭证规则行（socks5 必填 / http 与 https 同级可选）；
  双示例块改为 ip/port 形态，不再含 url 行（url 键退役，按未知键忽略）。
  v1.10 修订（操作者第七批指令，模板最小化）：操作者钦定模板文本成为 default_template()
  输出的唯一权威（逐字节一致，EOF 以一个换行符收尾；v1.6 起的「契约锚点为键集与形态，
  空格/顺序可调」口径废止）。9 键保留「用途 + 取值范围 + 示例行」三行式。
  v1.11 修订（操作者 bug11 轮指令，config 三大标准表重构）：配置项划分 [common]/[http]/[bt]
  三大标准表——[common] = 公共配置（download_dir / download_slots / max_speed）+ 命名代理
  [[common.proxy]]（键名 proxies → proxy 改名迁入 [common]，字段 name/type/ip/port/username/
  password 不变）；[http] = HTTP 引擎全套（既有 block_size_http / http_concurrency /
  max_retries / auto_retry / backoff_initial / backoff_cap 六键迁入 + 新增调优键
  progress_tick_ms / sidecar_flush_secs / throttle_burst_secs / throttle_sleep_ms /
  dns_race_grace_ms——代码常量外置，缺省取代码常量）；[bt] = BT 引擎全套（既有
  http_trackers / udp_trackers 两键迁入 + 新增调优键 20 个：bt_concurrency / peer_pipeline /
  upload_slots / upload_read_concurrency / idle_eviction_secs（操作者已批默认 60）/
  connect_timeout_secs / handshake_timeout_secs / request_timeout_secs / requests_per_piece /
  rechoke_interval_secs / optimistic_interval_secs / progress_emit_min_interval_ms /
  tracker_numwant / peer_retry_base_secs / peer_retry_cap_secs / tracker_backoff_start_secs /
  tracker_backoff_cap_secs / udp_base_timeout_secs / udp_retries / seed_duration_secs——
  全部平铺表内不设子表，缺省取代码常量）。调优键统一回退口径：0/负值/非整数等非法值 →
  回退代码常量默认（与既有「0 或非法值回退默认」口径一致）；bt_concurrency 越界钳制 1–64
  （同 http_concurrency）。旧格式（顶层平铺键、[[proxies]] 表）不兼容：按未知键静默忽略、
  相关项回退默认值（操作者钦定「代码开发中没发布过，不需要兼容旧格式」，FR-01-71 未知键
  口径，零警告零迁移；场景见 01-persistence-config-12）。模板钦定文本整体替换为 v1.11
  三表版（36 键三行式 + [[common.proxy]] 双示例块 + 三表分隔头注释，权威机制沿 v1.10：
  default_template() 输出与钦定原文逐字节一致，EOF 单换行收尾）。

  Background:
    Given ezr 以独立 HOME 的干净环境启动

  Scenario: 01-config-template-01 首次运行生成全注释默认模板
    Given 不存在配置文件 <config_path>
    When 启动 ezr 并正常使用
    Then 配置文件 <config_path> 已生成
    And 文件内每条非空行均以 # 开头（全注释 ⇒ 解析结果与文件缺失一致，全部默认值生效）
    And 模板内容与操作者钦定文本逐字节一致（钦定原文行集内嵌于本场景下方注释块：标题行、三表分隔头注释、36 键三行式、[[common.proxy]] 一句头注释 + 双示例块、空行位置与行序、EOF 单换行收尾全部一致，v1.11）
    And 键 "<key>" 以注释形式给出默认值 "<default_line>"
    And 键 "<key>" 的注释说明含用途要点 "<purpose>" 与取值范围要点 "<range>"

    Examples:
      | key                          | default_line                              | purpose                                          | range                                       |
      | download_dir                 | # download_dir = ""                       | 默认保存目录                                     | 留空或缺失 = 用户主目录下的下载目录         |
      | download_slots               | # download_slots = 5                      | 全局下载槽位数                                   | 正整数（≥1）；0 或非法值回退默认            |
      | max_speed                    | # max_speed = 0                           | 全局下载限速                                     | 支持 "2 MB/s" / "500 KB/s" / 整数 B/s       |
      | block_size_http              | # block_size_http = 1048576               | HTTP 分块块大小（字节）                          | 正整数（≥1）；0 或非法值回退默认            |
      | http_concurrency             | # http_concurrency = 4                    | 默认并发数                                       | 1–64 的整数；越界钳制到边界                 |
      | max_retries                  | # max_retries = 5                         | 自动重试上限次数                                 | ≥1 的整数；0 或非法值回退默认               |
      | auto_retry                   | # auto_retry = true                       | 失败后是否自动重试                               | true / false                                |
      | backoff_initial              | # backoff_initial = 8.0                   | 自动重试退避初始秒                               | >0 的有限数；非法值回退默认                 |
      | backoff_cap                  | # backoff_cap = 60.0                      | 自动重试退避封顶秒                               | >0 的有限数；非法值回退默认                 |
      | progress_tick_ms             | # progress_tick_ms = 100                  | HTTP 下载进度事件节拍（毫秒）                    | 正整数（≥1）；0 或非法值回退默认            |
      | sidecar_flush_secs           | # sidecar_flush_secs = 2                  | 断点 sidecar 落盘间隔（秒）                      | 正整数（≥1）；0 或非法值回退默认            |
      | throttle_burst_secs          | # throttle_burst_secs = 0.5               | 限速令牌桶突发窗（秒）                           | >0 的有限数；非法值回退默认                 |
      | throttle_sleep_ms            | # throttle_sleep_ms = 20                  | 限速循环步进睡眠（毫秒）                         | 正整数（≥1）；0 或非法值回退默认            |
      | dns_race_grace_ms            | # dns_race_grace_ms = 1000                | DNS 双腿竞速宽限（毫秒）                         | 正整数（≥1）；0 或非法值回退默认            |
      | http_trackers                | # http_trackers = []                      | HTTP(S) tracker 列表（BT 任务默认池来源之一）    | 字符串数组；非法 URL 行启动时忽略并警告     |
      | udp_trackers                 | # udp_trackers = []                       | UDP tracker 列表（BT 任务默认池来源之一）        | 字符串数组；非法 URL 行启动时忽略并警告     |
      | bt_concurrency               | # bt_concurrency = 20                     | BT 任务默认并发数                                | 1–64 的整数；越界钳制到边界                 |
      | peer_pipeline                | # peer_pipeline = 64                      | 单 peer 在飞请求块数上限                         | 正整数（≥1）；0 或非法值回退默认            |
      | upload_slots                 | # upload_slots = 4                        | 同时获得上传槽位的 peer 数                       | 正整数（≥1）；0 或非法值回退默认            |
      | upload_read_concurrency      | # upload_read_concurrency = 128           | 上传读盘并发上限                                 | 正整数（≥1）；0 或非法值回退默认            |
      | idle_eviction_secs           | # idle_eviction_secs = 60                 | 空闲连接置换阈值（秒）                           | 正整数（≥1）；0 或非法值回退默认            |
      | connect_timeout_secs         | # connect_timeout_secs = 3                | 出连 TCP 拨号超时（秒）                          | 正整数（≥1）；0 或非法值回退默认            |
      | handshake_timeout_secs       | # handshake_timeout_secs = 10             | peer 握手超时（秒）                              | 正整数（≥1）；0 或非法值回退默认            |
      | request_timeout_secs         | # request_timeout_secs = 20               | 在飞块请求超时（秒）                             | 正整数（≥1）；0 或非法值回退默认            |
      | requests_per_piece           | # requests_per_piece = 4                  | 单 piece 请求块粒度                              | 正整数（≥1）；0 或非法值回退默认            |
      | rechoke_interval_secs        | # rechoke_interval_secs = 10              | 上传 choker 重排周期（秒）                       | 正整数（≥1）；0 或非法值回退默认            |
      | optimistic_interval_secs     | # optimistic_interval_secs = 30           | 乐观 unchoke 轮换周期（秒）                      | 正整数（≥1）；0 或非法值回退默认            |
      | progress_emit_min_interval_ms | # progress_emit_min_interval_ms = 250    | 进度事件最小发送间隔（毫秒）                     | 正整数（≥1）；0 或非法值回退默认            |
      | tracker_numwant              | # tracker_numwant = 200                   | announce 请求的 peer 数上限                      | 正整数（≥1）；0 或非法值回退默认            |
      | peer_retry_base_secs         | # peer_retry_base_secs = 5                | peer 连接失败重试退避起始（秒）                  | 正整数（≥1）；0 或非法值回退默认            |
      | peer_retry_cap_secs          | # peer_retry_cap_secs = 120               | peer 连接失败重试退避封顶（秒）                  | 正整数（≥1）；0 或非法值回退默认            |
      | tracker_backoff_start_secs   | # tracker_backoff_start_secs = 8          | tracker 失败退避起始（秒）                       | 正整数（≥1）；0 或非法值回退默认            |
      | tracker_backoff_cap_secs     | # tracker_backoff_cap_secs = 60           | tracker 失败退避封顶（秒）                       | 正整数（≥1）；0 或非法值回退默认            |
      | udp_base_timeout_secs        | # udp_base_timeout_secs = 15              | UDP tracker 单轮超时起始（秒）                   | 正整数（≥1）；0 或非法值回退默认            |
      | udp_retries                  | # udp_retries = 3                         | UDP tracker 超时重试次数                         | 正整数（≥1）；0 或非法值回退默认            |
      | seed_duration_secs           | # seed_duration_secs = 1800               | 做种时长默认值（秒）                             | >0 的数；0 或非法值回退默认                 |

  （v1.11 操作者钦定模板原文——default_template() 输出与其逐字节一致，coder 不得增删改
  任何字符/空行；钦定原文共 168 行 = 标题行 + 3 表分隔头注释 + 36 键×3 行三行式 +
  [[common.proxy]] 一句头注释 + 双示例块（各 7 行 = 1 表头行 + 6 字段行）+ 41 个空行分隔，EOF 以一个换行符收尾。
  下为权威原文逐字节内嵌（空行即真实空行；示例值与代码常量默认逐键比对 36/36 一致，
  specifier v1.11）：
  # EZR Downloader 配置模板

  # ===== [common] 公共配置（两引擎共用） =====

  # download_dir：默认保存目录（添加对话框目录留空时使用）
  # 取值范围：任意目录路径；留空或缺失 = 用户主目录下的下载目录
  # download_dir = ""

  # download_slots：全局下载槽位数（同时下载的任务数上限）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # download_slots = 5

  # max_speed：全局下载限速（0 = 不限）
  # 取值范围：≥0；支持 "2 MB/s" / "500 KB/s" / 整数 B/s（十进制口径 1 MB = 1000000 B/s）
  # max_speed = 0

  # proxy：命名代理列表（可配置多个）
  # [[common.proxy]]
  # name = "办公网代理"
  # type = "http"
  # ip = "proxy.corp.example"
  # port = 8080
  # username = "alice"
  # password = "secret"

  # [[common.proxy]]
  # name = "本地 SOCKS5"
  # type = "socks5"
  # ip = "127.0.0.1"
  # port = 1080
  # username = "ezr"
  # password = "secret"

  # ===== [http] HTTP 引擎配置 =====

  # block_size_http：HTTP 分块块大小（字节）
  # 取值范围：正整数（≥1）；0 或非法值回退默认（1 MB）
  # block_size_http = 1048576

  # http_concurrency：默认并发数
  # 取值范围：1–64 的整数；越界钳制到边界
  # http_concurrency = 4

  # max_retries：自动重试上限次数（达上限转停等，可按 R 手动重试）
  # 取值范围：≥1 的整数；0 或非法值回退默认
  # max_retries = 5

  # auto_retry：失败后是否自动重试
  # 取值范围：true / false
  # auto_retry = true

  # backoff_initial：自动重试退避初始秒（指数退避序列起点）
  # 取值范围：>0 的有限数；非法值回退默认
  # backoff_initial = 8.0

  # backoff_cap：自动重试退避封顶秒
  # 取值范围：>0 的有限数；非法值回退默认
  # backoff_cap = 60.0

  # progress_tick_ms：HTTP 下载进度事件节拍（毫秒）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # progress_tick_ms = 100

  # sidecar_flush_secs：断点 sidecar 落盘间隔（秒）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # sidecar_flush_secs = 2

  # throttle_burst_secs：限速令牌桶突发窗（秒）
  # 取值范围：>0 的有限数；非法值回退默认
  # throttle_burst_secs = 0.5

  # throttle_sleep_ms：限速循环步进睡眠（毫秒）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # throttle_sleep_ms = 20

  # dns_race_grace_ms：DNS 双腿竞速宽限（毫秒，双栈并行解析）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # dns_race_grace_ms = 1000

  # ===== [bt] BT 引擎配置 =====

  # http_trackers：HTTP(S) tracker 列表（BT 任务默认 tracker 池来源之一）
  # 取值范围：字符串数组，每项为 http:// 或 https:// 开头的 tracker announce URL；非法 URL 行启动时忽略并警告
  # http_trackers = []

  # udp_trackers：UDP tracker 列表（BT 任务默认 tracker 池来源之一）
  # 取值范围：字符串数组，每项为 udp:// 开头的 tracker announce URL（含端口）；非法 URL 行启动时忽略并警告
  # udp_trackers = []

  # bt_concurrency：BT 任务默认并发数（新任务预填）
  # 取值范围：1–64 的整数；越界钳制到边界
  # bt_concurrency = 20

  # peer_pipeline：单 peer 在飞请求块数上限（×16 KiB = 在飞窗口）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # peer_pipeline = 64

  # upload_slots：同时获得上传槽位的 peer 数（choker 周期分配）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # upload_slots = 4

  # upload_read_concurrency：上传读盘并发上限（进程级）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # upload_read_concurrency = 128

  # idle_eviction_secs：空闲连接置换阈值（秒，无新增进账超时即断开重拨）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # idle_eviction_secs = 60

  # connect_timeout_secs：出连 TCP 拨号超时（秒）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # connect_timeout_secs = 3

  # handshake_timeout_secs：peer 握手超时（秒）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # handshake_timeout_secs = 10

  # request_timeout_secs：在飞块请求超时（秒）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # request_timeout_secs = 20

  # requests_per_piece：单 piece 请求块粒度（piece 细分为 n 块请求）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # requests_per_piece = 4

  # rechoke_interval_secs：上传 choker 重排周期（秒）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # rechoke_interval_secs = 10

  # optimistic_interval_secs：乐观 unchoke 轮换周期（秒）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # optimistic_interval_secs = 30

  # progress_emit_min_interval_ms：进度事件最小发送间隔（毫秒，节流）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # progress_emit_min_interval_ms = 250

  # tracker_numwant：announce 请求的 peer 数上限
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # tracker_numwant = 200

  # peer_retry_base_secs：peer 连接失败重试退避起始（秒，倍增）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # peer_retry_base_secs = 5

  # peer_retry_cap_secs：peer 连接失败重试退避封顶（秒）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # peer_retry_cap_secs = 120

  # tracker_backoff_start_secs：tracker 失败退避起始（秒，倍增）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # tracker_backoff_start_secs = 8

  # tracker_backoff_cap_secs：tracker 失败退避封顶（秒）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # tracker_backoff_cap_secs = 60

  # udp_base_timeout_secs：UDP tracker 单轮超时起始（秒，倍增重试）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # udp_base_timeout_secs = 15

  # udp_retries：UDP tracker 超时重试次数
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # udp_retries = 3

  # seed_duration_secs：做种时长默认值（秒，任务做种倒计时起点）
  # 取值范围：>0 的数；0 或非法值回退默认
  # seed_duration_secs = 1800
  ）

  Scenario: 01-config-template-02 已有配置文件不重写不覆盖
    Given 配置文件已存在且设 [common] 表 download_slots = 2（合法生效值）
    When 启动 ezr 并正常使用
    Then 配置文件内容字节级不变（无重写、无模板追加、mtime 不因生成动作变化）
    And 下载槽位按已设值 2 生效（操作者手工配置优先于模板）

  Scenario: 01-config-template-03 损坏配置文件不触发重写且不阻塞启动
    Given 配置文件已存在但内容为非法 TOML（"not [valid toml ==="）
    When 启动 ezr 并正常使用
    Then 启动不报错且配置文件内容字节级不变（损坏文件不被模板覆盖）
    And 全部配置项按默认值生效（FR-01-71 非法回退口径）

  Scenario: 01-config-template-04 生成失败静默跳过不阻塞启动
    Given 不存在配置文件且配置所在目录不可写
    When 启动 ezr 并正常使用
    Then 启动不报错、不崩溃（生成失败静默跳过，D17）
    And 全部配置项按默认值生效（与文件缺失口径一致）

  Scenario: 01-config-template-05 EZR_HOME 重定位协同生成
    Given 环境变量 EZR_HOME 指向独立目录 <ezr_home> 且其中不存在 config.toml
    When 启动 ezr 并正常使用
    Then <ezr_home>/config.toml 已生成且为全注释默认模板（生成位置随 FR-01-71/D16 语义）
    And 用户主目录 ~/.ezr/ 下未生成 config.toml（重定位生效时缺省位置不落盘）
