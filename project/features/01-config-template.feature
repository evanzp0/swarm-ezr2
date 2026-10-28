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
  v1.8 修订：[[proxies]] 示例块补凭证规则注释行，socks5 示例块补 username/password
  示例行，http 示例注明凭证可选语义（FR-01-93）。
  v1.9 修订（条目字段重构，FR-01-86/89）：proxies 段注释更新为 ip/port/type 字段说明 +
  三类型语义（https = 代理自身走 TLS）+ 凭证规则行（socks5 必填 / http 与 https 同级可选）；
  双示例块改为 ip/port 形态，不再含 url 行（url 键退役，按未知键忽略）。
  v1.10 修订（操作者第七批指令，模板最小化）：操作者钦定模板文本成为 default_template()
  输出的唯一权威（逐字节一致，EOF 以一个换行符收尾；v1.6 起的「契约锚点为键集与形态，
  空格/顺序可调」口径废止）。9 键保留「用途 + 取值范围 + 示例行」三行式；proxies 段收敛为
  「一句头注释 + 双示例块」，三类型语义、凭证规则、IPv6、type/url 语义说明等解释性注释
  全部不再写入模板（FR-01-93⑤ 凭证注释行条款废止，凭证规则运行期语义不变——校验链/警告
  照旧；语义由运行期校验警告与 README/规格承载，不进配置文件）。

  Background:
    Given ezr 以独立 HOME 的干净环境启动

  Scenario: 01-config-template-01 首次运行生成全注释默认模板
    Given 不存在配置文件 <config_path>
    When 启动 ezr 并正常使用
    Then 配置文件 <config_path> 已生成
    And 文件内每条非空行均以 # 开头（全注释 ⇒ 解析结果与文件缺失一致，全部默认值生效）
    And 模板内容与操作者钦定文本逐字节一致（钦定原文行集内嵌于本场景下方注释块：标题行、9 键三行式、proxies 一句头注释 + 双示例块、空行位置与行序、EOF 单换行收尾全部一致，v1.10）
    And 键 "<key>" 以注释形式给出默认值 "<default_line>"
    And 键 "<key>" 的注释说明含用途要点 "<purpose>" 与取值范围要点 "<range>"

    Examples:
      | key              | default_line                | purpose                                          | range                                  |
      | download_dir     | # download_dir = ""         | 默认保存目录                                     | 留空或缺失 = 用户主目录下的下载目录    |
      | block_size_http  | # block_size_http = 1048576 | HTTP 分块块大小（字节）                          | 正整数（≥1）；0 或非法值回退默认       |
      | download_slots   | # download_slots = 5        | 全局下载槽位数（同时下载的任务数上限）           | 正整数（≥1）；0 或非法值回退默认       |
      | max_speed        | # max_speed = 0             | 全局下载限速（0 = 不限）                         | 支持 "2 MB/s" / "500 KB/s" / 整数 B/s  |
      | max_retries      | # max_retries = 5           | 自动重试上限次数（达上限转停等，可按 R 手动重试） | ≥1 的整数；0 或非法值回退默认         |
      | auto_retry       | # auto_retry = true         | 失败后是否自动重试                               | true / false                           |
      | backoff_initial  | # backoff_initial = 8.0     | 自动重试退避初始秒（指数退避序列起点）           | >0 的有限数；非法值回退默认            |
      | backoff_cap      | # backoff_cap = 60.0        | 自动重试退避封顶秒                               | >0 的有限数；非法值回退默认            |
      | http_concurrency | # http_concurrency = 4      | 默认并发数                                       | 1–64 的整数；越界钳制到边界            |

  （v1.10 操作者钦定模板原文——default_template() 输出与其逐字节一致，coder 不得增删改
  任何字符/空行；钦定原文共 54 行 = 标题行 + 9 键×3 行三行式 + proxies 一句头注释 +
  双示例块（各 7 行）+ 11 个空行分隔，EOF 以一个换行符收尾。下为权威原文逐字节内嵌
  （空行即真实空行；示例值已与 DEFAULT_*/Config::default() 比对 9/9 一致，specifier v1.10）：
  # EZR Downloader 配置模板

  # download_dir：默认保存目录（添加对话框目录留空时使用）
  # 取值范围：任意目录路径；留空或缺失 = 用户主目录下的下载目录
  # download_dir = ""

  # block_size_http：HTTP 分块块大小（字节）
  # 取值范围：正整数（≥1）；0 或非法值回退默认（1 MB）
  # block_size_http = 1048576

  # download_slots：全局下载槽位数（同时下载的任务数上限）
  # 取值范围：正整数（≥1）；0 或非法值回退默认
  # download_slots = 5

  # max_speed：全局下载限速（0 = 不限）
  # 取值范围：≥0；支持 "2 MB/s" / "500 KB/s" / 整数 B/s（十进制口径 1 MB = 1000000 B/s）
  # max_speed = 0

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

  # http_concurrency：默认并发数
  # 取值范围：1–64 的整数；越界钳制到边界
  # http_concurrency = 4

  # proxies：命名代理列表（可配置多个）
  # [[proxies]]
  # name = "办公网代理"
  # type = "http"
  # ip = "proxy.corp.example"
  # port = 8080
  # username = "alice"
  # password = "secret"

  # [[proxies]]
  # name = "本地 SOCKS5"
  # type = "socks5"
  # ip = "127.0.0.1"
  # port = 1080
  # username = "ezr"
  # password = "secret"
  ）

  Scenario: 01-config-template-02 已有配置文件不重写不覆盖
    Given 配置文件已存在且设 download_slots = 2（合法生效值）
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
