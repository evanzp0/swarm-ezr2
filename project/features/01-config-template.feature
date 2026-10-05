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
  （含 EZR_HOME 重定位，D16）。

  Background:
    Given ezr 以独立 HOME 的干净环境启动

  Scenario: 01-config-template-01 首次运行生成全注释默认模板
    Given 不存在配置文件 <config_path>
    When 启动 ezr 并正常使用
    Then 配置文件 <config_path> 已生成
    And 文件内每条非空行均以 # 开头（全注释 ⇒ 解析结果与文件缺失一致，全部默认值生效）
    And 键 "<key>" 以注释形式给出默认值 "<default_line>"
    And 键 "<key>" 的注释说明含用途要点 "<purpose>" 与取值范围要点 "<range>"

    Examples:
      | key                 | default_line                  | purpose                   | range                          |
      | download_dir        | # download_dir = ""           | 默认保存目录              | 空或缺失 = 用户下载目录        |
      | block_size_http     | # block_size_http = 1048576   | HTTP 分块块大小（字节）   | 正整数；0 或非法回退默认 1 MB  |
      | download_slots      | # download_slots = 5          | 全局下载槽位数            | 正整数；0 或非法回退默认 5     |
      | max_speed           | # max_speed = 0               | 全局限速                  | 0 = 不限；支持 "2 MB/s" 形态   |
      | max_retries         | # max_retries = 5             | 自动重试上限次数          | ≥1 整数；0 或非法回退默认      |
      | auto_retry          | # auto_retry = true           | 是否自动重试              | true 或 false                  |
      | backoff_initial     | # backoff_initial = 8.0       | 重试退避初始秒            | >0 的有限数                    |
      | backoff_cap         | # backoff_cap = 60.0          | 重试退避封顶秒            | >0 的有限数                    |
      | proxy               | # proxy = ""                  | HTTP(S) 代理地址          | 空 = 不用代理；仅配置文件途径  |
      | default_concurrency | # default_concurrency = 4     | 默认并发数                | 1–64；越界钳制                 |

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
