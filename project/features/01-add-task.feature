# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   01-add-task-01 对话框五字段创建任务
#   01-add-task-02 校验算法下拉七选一
#   01-add-task-03 校验码含非十六进制字符确认报错
#   01-add-task-04 校验码位数与算法不符确认报错
#   01-add-task-05 校验码留空不校验；输入含大写确认统一小写
#   01-add-task-06 并发数留空或非法回退默认 4
#   01-add-task-07 URL 仅接受 http(s)（含无协议前缀拒绝）
#   01-add-task-08 文件名推导优先级
#   01-add-task-09 文件名无法推导回退 download-<时间戳>
#   01-add-task-10 目标文件已存在自动追加序号
#   01-add-task-11 保存目录缺省链与自动创建
#   01-add-task-12 CLI 参数创建任务且 -x <算法>=<校验码> 显式指定算法
#   01-add-task-13 CLI -x 非法（缺前缀/算法无法识别/位数不符）启动报错退出
#   01-add-task-14 CLI -c 非法值启动报错退出
#   01-add-task-15 CLI 多 URL 部分非法全部拒绝
#   01-add-task-16 重复任务拒绝并提示
#   01-add-task-17 对话框支持粘贴长文本
Feature: 01-add-task 任务添加（对话框与 CLI）

  期号 01。依据 project/mission/phase-01.md FR-01-01~06 与本会话澄清决定（重复任务拒绝、
  无协议前缀拒绝、CLI -c 非法报错退出、CLI 多 URL 部分非法全部拒绝、默认并发键
  default_concurrency）。UI 交互基线沿用 ezr-tui-demo 定稿。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件

  Scenario: 01-add-task-01 对话框五字段创建任务
    When 按 A 打开「添加下载任务」对话框
    And 在 URL 字段输入 "<url>"
    And 在保存目录字段输入 "<dir>"
    And 在并发数字段输入 "<concurrency>"
    And 校验算法保持默认 "SHA-256" 并在校验码字段输入 "<checksum>"
    And 按 Enter 确认
    Then 任务列表出现任务 "<filename>" 且状态为「等待中」或「下载中」
    And 详情显示类型 "<scheme>"、并发数 <concurrency>、校验 "SHA-256"
    And 目标文件按保存目录 "<dir>" 落盘

    Examples:
      | url                                          | dir            | concurrency | checksum                                                          | filename   | scheme |
      | http://fixture.local/files/big-one.bin       | /tmp/qadl      | 4           | d869db7fe62fb06d1c488a04d5e68e93a4f0c0e0e0c0a1b2c3d4e5f60718293a | big-one.bin | HTTP   |

  Scenario: 01-add-task-02 校验算法下拉七选一
    When 按 A 打开「添加下载任务」对话框
    And 在校验算法字段按 Enter 展开下拉列表
    Then 下拉列出 7 种算法：MD5 / SHA-1 / SHA-224 / SHA-256 / SHA-384 / SHA-512 / Adler-32
    When 按 ↓ 两次再按 Home 后按 End 选中 "SHA-512"
    Then 校验码字段占位提示显示期望位数 128
    When 按 Enter 确认选中
    Then 算法字段显示 "SHA-512"

  Scenario: 01-add-task-03 校验码含非十六进制字符确认报错
    When 按 A 打开「添加下载任务」对话框
    And 输入合法 URL 并在校验码字段输入 "<bad_checksum>"
    And 按 Enter 确认
    Then 出现校验码格式错误的 toast 报错
    And 焦点回到校验码字段且对话框未关闭

    Examples:
      | bad_checksum          |
      | zz649b77fe62fb06d1c48 |
      | d869db7f-e62f-b06d    |

  Scenario: 01-add-task-04 校验码位数与算法不符确认报错
    When 按 A 打开「添加下载任务」对话框
    And 输入合法 URL、选择算法 "<algo>" 并输入 "<digits>" 位十六进制校验码
    And 按 Enter 确认
    Then 出现校验码位数与算法不符的 toast 报错（期望 <expected> 位）
    And 焦点回到校验码字段且对话框未关闭

    Examples:
      | algo    | digits | expected |
      | SHA-256 | 32     | 64       |
      | MD5     | 64     | 32       |

  Scenario: 01-add-task-05 校验码留空不校验；输入含大写确认统一小写
    When 按 A 打开「添加下载任务」对话框
    And 输入合法 URL、选择算法 "MD5" 且校验码留空
    And 按 Enter 确认
    Then 任务创建成功且详情校验行显示「无校验」
    When 按 A 再次打开对话框
    And 输入合法 URL、选择算法 "MD5" 并输入校验码 "D869DB7FE62FB06D1C488A04D5E68E93"
    And 按 Enter 确认
    Then 任务创建成功且详情校验行显示校验码前缀 "d869db7f"

  Scenario: 01-add-task-06 并发数留空或非法回退默认 4
    When 按 A 打开「添加下载任务」对话框
    And 输入合法 URL 且并发数字段输入 "<concurrency_input>"
    And 按 Enter 确认
    Then 任务创建成功且详情显示并发数 4

    Examples:
      | concurrency_input |
      | （留空）           |
      | 0                 |
      | 65                |
      | abc               |

  Scenario: 01-add-task-07 URL 仅接受 http(s)（含无协议前缀拒绝）
    When 按 A 打开「添加下载任务」对话框
    And 在 URL 字段输入 "<url>"
    And 按 Enter 确认
    Then 出现 URL 非法的 toast 报错（仅支持 http/https）
    And 任务列表不新增任何任务

    Examples:
      | url                        |
      | www.fixture.local/file.bin |
      | ftp://fixture.local/file   |
      | /local/path/file.bin       |

  Scenario: 01-add-task-08 文件名推导优先级
    Given fixture 提供下载项 "<case>"：Content-Disposition "<disposition>"、最终 URL 路径末段 "<final_segment>"、原始 URL 路径末段 "<origin_segment>"
    When 按对话框添加该下载项并确认
    Then 目标文件名为 "<expected_filename>"

    Examples:
      | case | disposition                          | final_segment | origin_segment | expected_filename |
      | 1    | attachment; filename="report.pdf"    | data.bin      | origin.bin     | report.pdf        |
      | 2    | （无）                               | dataset.tar.gz| origin.zip     | dataset.tar.gz    |
      | 3    | （无）                               | （末段为空）  | archive.iso    | archive.iso       |

  Scenario: 01-add-task-09 文件名无法推导回退 download-<时间戳>
    Given fixture 提供下载项：Content-Disposition 缺失、最终与原始 URL 路径末段均为空
    When 按对话框添加该下载项并确认
    Then 目标文件名匹配 download-<数字时间戳> 格式

  Scenario: 01-add-task-10 目标文件已存在自动追加序号
    Given 保存目录 <dir> 已存在文件 "<filename>"
    When 按对话框添加一个将推导出同名目标文件的下载项并确认
    Then 新任务目标文件名为 "<filename>.1" 且既有文件内容未被改动

    Examples:
      | dir       | filename   |
      | /tmp/qadl | report.pdf |

  Scenario: 01-add-task-11 保存目录缺省链与自动创建
    Given 配置文件 download_dir 为 "<config_dir>"
    When 按对话框添加下载项且保存目录字段输入 "<input_dir>"
    And 按 Enter 确认
    Then 目标文件落在 "<resolved_dir>" 下（目录不存在时已自动创建）

    Examples:
      | config_dir      | input_dir   | resolved_dir    |
      | /tmp/qadl-home  | （留空）    | /tmp/qadl-home  |
      | （配置缺失）    | （留空）    | （用户主目录下载目录）|
      | /tmp/qadl-home  | /tmp/qadl-new | /tmp/qadl-new |

  Scenario: 01-add-task-12 CLI 参数创建任务且 -x <算法>=<校验码> 显式指定算法
    When 以命令行启动 "ezr <url> -d <dir> -c <concurrency> -x <x_arg>"
    Then 启动成功且任务列表出现对应任务
    And 详情显示校验算法 "<algo>"

    Examples:
      | url                                  | dir       | concurrency | x_arg                                                                  | algo     |
      | http://fixture.local/files/a.bin     | /tmp/qadl | 4           | sha256=d869db7fe62fb06d1c488a04d5e68e93a4f0c0e0e0c0a1b2c3d4e5f60718293a | SHA-256  |
      | http://fixture.local/files/b.bin     | /tmp/qadl | 2           | MD5=d869db7fe62fb06d1c488a04d5e68e93                                   | MD5      |
      | http://fixture.local/files/c.bin     | /tmp/qadl | 8           | adler32=d869db7f                                                       | Adler-32 |

  Scenario: 01-add-task-13 CLI -x 非法（缺前缀/算法无法识别/位数不符）启动报错退出
    When 以命令行启动 "ezr <url> -x <x_arg>"
    Then 进程以非零退出码退出并提示 "-x" 校验参数非法（须为 <算法>=<校验码> 形式）
    And 未创建任何任务

    Examples:
      | url                              | x_arg                                                                  |
      | http://fixture.local/files/a.bin | d869db7fe62fb06d1c488a04d5e68e93a4f0c0e0e0c0a1b2c3d4e5f60718293a       |
      | http://fixture.local/files/a.bin | sha3=d869db7f                                                          |
      | http://fixture.local/files/a.bin | md5=d869db7fe62fb06d1c488a04d5e68e93a4f0c0e0e0c0a1b2c3d4e5f60718293a   |

  Scenario: 01-add-task-14 CLI -c 非法值启动报错退出
    When 以命令行启动 "ezr <url> -c <concurrency>"
    Then 进程以非零退出码退出并提示并发数非法
    And 未创建任何任务

    Examples:
      | url                              | concurrency |
      | http://fixture.local/files/a.bin | 0           |
      | http://fixture.local/files/a.bin | 65          |
      | http://fixture.local/files/a.bin | abc         |

  Scenario: 01-add-task-15 CLI 多 URL 部分非法全部拒绝
    When 以命令行启动 "ezr <good_url> <bad_url>"
    Then 进程以非零退出码退出并提示存在非法 URL
    And 未创建任何任务（合法 URL 亦不添加）

    Examples:
      | good_url                         | bad_url                    |
      | http://fixture.local/files/a.bin | ftp://fixture.local/file   |
      | http://fixture.local/files/a.bin | not-a-url                  |

  Scenario: 01-add-task-16 重复任务拒绝并提示
    Given 任务列表已存在 URL "<url>" 且保存目录 "<dir>" 的任务
    When 按对话框再次添加相同 URL 与相同保存目录并确认
    Then 出现「任务已存在」toast 提示
    And 任务列表数量不变且未创建新任务

    Examples:
      | url                              | dir       |
      | http://fixture.local/files/a.bin | /tmp/qadl |

  Scenario: 01-add-task-17 对话框支持粘贴长文本
    When 按 A 打开「添加下载任务」对话框
    And 以终端粘贴（bracketed paste）向 URL 字段粘贴 <length> 字符的长 URL
    Then URL 字段完整显示粘贴内容
    And 按 Enter 确认后任务创建成功

    Examples:
      | length |
      | 200    |
