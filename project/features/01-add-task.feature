# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   01-add-task-01 对话框五字段创建任务（v1.32 修订：标题更名、校验默认 NONE——选 SHA-256 输码创建）
#   01-add-task-02 校验算法下拉八项含 NONE（v1.32 修订：七选一改八项、NONE 首行默认、占位提示删除）
#   01-add-task-03 校验码含非十六进制字符确认报错（v1.32 修订：空提交拒绝臂入本场景）
#   01-add-task-04 校验码位数与算法不符确认报错
#   01-add-task-05 校验码必填与 NONE 臂（v1.32 重写：非 NONE 算法留空提交拒绝；NONE = 不校验；大写统一小写）
#   01-add-task-06 并发数留空或非法回退默认 4
#   01-add-task-07 URL 仅接受 http(s)（含无协议前缀拒绝）
#   01-add-task-08 文件名推导优先级
#   01-add-task-09 文件名无法推导回退 download-<时间戳>
#   01-add-task-10 目标文件已存在自动追加序号
#   01-add-task-11 保存目录缺省链、打开预填与归一（v1.35 重写：~ 展开 + 相对→绝对）
#   01-add-task-12 CLI 参数创建任务且 -x <算法>=<校验码> 显式指定算法
#   01-add-task-13 CLI -x 非法（缺前缀/算法无法识别/位数不符）启动报错退出
#   01-add-task-14 CLI -c 非法值启动报错退出
#   01-add-task-15 CLI 多 URL 部分非法全部拒绝
#   01-add-task-16 重复任务拒绝并提示
#   01-add-task-17 对话框支持粘贴长文本
#   01-add-task-18 仅添加按钮：只添加任务不下载（v1.30 修订：对话框无提示行，P3/FR-01-107）
#   01-add-task-19 URL 输入自动回填默认并发与 Esc 取消（v1.30 新增，对齐 ezr-demo 重构版 §6.1）
# v1.32 同步注（操作者 20261021 指令，对齐 ezr-demo m3 §6.1 与 demo 源码，逐条核证）：①对话框标题
# 「 添加下载任务 」→「 添加任务：HTTP(S) 」（74×11 不变）；②校验下拉 8 项 = NONE（首行，默认，
# 右列 — 占位）+ 7 算法；NONE 选中即校验码实时清空且不可输入（逐键/粘贴同拦截）；其余算法校验码
# 必填——空提交拒绝 toast `⚠ {算法} 校验码不可为空（需 N 位十六进制）` 并聚焦校验码字段；
# ③URL/校验码字段占位提示与校验行「Enter 选择算法」尾注删除（P3）。
# v1.34 同步注（操作者 20261023 指令，对齐 ezr-demo 重构版 padding 统一 README 变更记录 17，
# 逐条经 ui.rs draw_task_dialog 源码核证）：①对话框尺寸 74×11 → 74×12（底部补齐 1 行 padding——
# 内框 = 顶 1 + 字段 6 + 空 1 + 按钮 1 + 底 1，FR-01-115/FR-01-103 v1.34；字段行/按钮行 y 不变、
# 字段行 x = 内框 + 2、行宽 = 内框 − 4、热区同步）；②下拉选项行左 2 列前缀缩进（前缀 2 空格，
# 高度「项数 + 2」紧凑特例不变，FR-01-115）；③顶边框行右端 [✗] 关闭按钮（FR-01-114：原地松开 =
# 取消，几何/配色细则见 01-tui-display-27）。
# v1.35 修订注（操作者 bug13 轮指令，D62；FR-01-03 v1.38）：①场景 11 重写——保存目录字段打开时
# 预填 = 配置 download_dir（未设/空白 → 代码默认 ~/Downloads 口径，与 BT 对话框 D48 同口径），
# 确认时按归一口径处理：~ 与 ~/ 前缀展开为用户主目录（主目录不可得时保持原样）、相对路径
# （非 / 开头且非 ~ 前缀）按创建时进程工作目录转绝对路径、末尾 / 去除；任务落注册表与详情
# 「路径」显示均为归一后绝对路径；②目录不存在时自动创建口径不变。
Feature: 01-add-task 任务添加（对话框与 CLI）

  期号 01。依据 project/mission/phase-01.md FR-01-01~06 与本会话澄清决定（重复任务拒绝、
  无协议前缀拒绝、CLI -c 非法报错退出、CLI 多 URL 部分非法全部拒绝、默认并发键
  http_concurrency，v1.5 自 default_concurrency 改名，FR-01-88）。UI 交互基线沿用
  ezr-tui-demo 定稿。
  v1.15 增补：场景 18（仅添加按钮，FR-01-103/D28/D29，操作者第十二批指令）。
  v1.30 修订（操作者指令，对齐 ezr-demo 重构版）：对话框 74×11、无底部操作提示行
  （P3 原则/FR-01-107，交互说明收敛帮助窗口）；按钮激活 = Enter/空格/鼠标点击；
  URL 输入自动回填默认并发（未手动改过并发时，HTTP 4 / BT 20 预留）。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件

  Scenario: 01-add-task-01 对话框五字段创建任务
    When 按 A 打开「添加任务：HTTP(S)」对话框（74×12 圆角边框浮层、终端内居中；v1.32 标题更名，原「添加下载任务」退役；v1.34/FR-01-115：74×11 → 74×12 底部补齐 1 行 padding，字段行 x = 内框 + 2、行宽 = 内框 − 4）
    And 在 URL 字段输入 "<url>"
    And 在保存目录字段输入 "<dir>"
    And 在并发数字段输入 "<concurrency>"
    And 校验算法选 "SHA-256" 并在校验码字段输入 "<checksum>"（打开时默认 NONE，校验码禁输入）
    And 按 Enter 确认
    Then 任务列表出现任务 "<filename>" 且状态为「等待中」或「下载中」
    And 详情显示类型 "<scheme>"、并发数 <concurrency>、校验 "SHA-256"
    And 目标文件按保存目录 "<dir>" 落盘

    Examples:
      | url                                          | dir            | concurrency | checksum                                                          | filename   | scheme |
      | http://fixture.local/files/big-one.bin       | /tmp/qadl      | 4           | d869db7fe62fb06d1c488a04d5e68e93a4f0c0e0e0c0a1b2c3d4e5f60718293a | big-one.bin | HTTP   |

  Scenario: 01-add-task-19 URL 输入自动回填默认并发与 Esc 取消（v1.30 新增）
    When 按 A 打开「添加任务：HTTP(S)」对话框且并发字段留空
    And 在 URL 字段逐字输入 http(s) URL
    Then 并发字段自动回填该协议默认并发 4（HTTP；BT 20 为 phase-02 预留；v1.30/FR-01-01 注记）
    When 手动将并发改为 "6" 后继续修改 URL
    Then 并发字段保持 "6" 不再被回填（仅未手动改过时自动同步）
    When 按 Esc 取消
    Then 对话框关闭且无任务创建（Esc 取消语义，v1.30）

  Scenario: 01-add-task-02 校验算法下拉八项含 NONE（v1.32 重写）
    When 按 A 打开「添加任务：HTTP(S)」对话框
    And 观察校验算法字段
    Then 默认显示 "NONE"（不校验，右列 — 占位；校验码字段清空且不可输入——逐键/粘贴同拦截，v1.32）
    When 在校验算法字段按 Enter 展开下拉列表
    Then 下拉列出 8 项：首行 NONE（右列显示 — 占位）+ MD5 / SHA-1 / SHA-224 / SHA-256 / SHA-384 / SHA-512 / Adler-32（各右列显示期望位数；选项行左 2 列前缀缩进——前缀 2 空格，v1.34/FR-01-115；高度「项数 + 2」紧凑特例不变）
    And URL 字段与校验码字段无占位提示、校验行无「Enter 选择算法」尾注（提示删除，v1.32/P3）
    When 按 ↓ 选中 "SHA-512"
    Then 选中即时生效（算法字段显示 "SHA-512"，校验码可输入）——校验下拉 ↑↓ 移动选中即改值
    When 按 ↑ 回到 "NONE"
    Then 校验码字段实时清空且此后不可输入（逐键/粘贴同拦截，v1.32）
    When 按 ↓ 选中 "SHA-512" 后按 Enter 确认选中
    Then 算法字段显示 "SHA-512" 且下拉收起

  Scenario: 01-add-task-03 校验码非法字符逐键拒绝（输入即过滤；空提交拒绝臂 v1.32 增补）
    When 按 A 打开「添加任务：HTTP(S)」对话框
    And 输入合法 URL、校验算法选 "MD5"，在校验码字段输入 "<bad_input>"（含非十六进制字符）
    Then 非十六进制字符被逐键拒绝、不进入校验码字段（纯非法输入则字段保持空）
    And 按 Enter 确认
    Then 提交被拒绝（校验码为空——非 NONE 算法必填，v1.32）：toast 逐字显示 "⚠ MD5 校验码不可为空（需 32 位十六进制）"、焦点回校验码字段、对话框未关闭

    Examples:
      | bad_input  |
      | zz         |
      | ---------- |

  Scenario: 01-add-task-04 校验码位数与算法不符确认报错
    When 按 A 打开「添加任务：HTTP(S)」对话框
    And 输入合法 URL、选择算法 "<algo>" 并输入 "<digits>" 位十六进制校验码
    And 按 Enter 确认
    Then 出现校验码位数与算法不符的 toast 报错（期望 <expected> 位）
    And 焦点回到校验码字段且对话框未关闭

    Examples:
      | algo    | digits | expected |
      | SHA-256 | 32     | 64       |
      | MD5     | 64     | 32       |

  Scenario: 01-add-task-05 校验码必填与 NONE 臂；输入含大写确认统一小写（v1.32 重写）
    When 按 A 打开「添加任务：HTTP(S)」对话框
    And 输入合法 URL、校验算法保持默认 "NONE"（校验码清空且不可输入）
    And 按 Enter 确认
    Then 任务创建成功且详情校验行显示「无校验」（NONE = 不校验，v1.32——原「MD5 留空创建」路径退役）
    When 按 A 再次打开对话框
    And 输入合法 URL、校验算法选 "MD5" 且校验码留空
    And 按 Enter 确认
    Then 提交被拒绝且 toast 显示 "⚠ MD5 校验码不可为空（需 32 位十六进制）"、焦点回校验码字段（v1.32 必填）
    When 在校验码字段输入 "D869DB7FE62FB06D1C488A04D5E68E93" 后按 Enter 确认
    Then 任务创建成功且详情校验行显示校验码前缀 "d869db7f"（确认时统一小写）

  Scenario: 01-add-task-06 并发数留空或非法回退默认 4
    When 按 A 打开「添加任务：HTTP(S)」对话框
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
    When 按 A 打开「添加任务：HTTP(S)」对话框
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
    Given fixture 提供下载项 "<case>"：Content-Disposition "<disposition>"、探测请求跳转落点 URL 路径末段 "<final_segment>"（该次连接的当前值，不持久化，v1.28/FR-01-02）、原始 URL 路径末段 "<origin_segment>"
    When 按对话框添加该下载项并确认
    Then 目标文件名为 "<expected_filename>"

    Examples:
      | case | disposition                          | final_segment | origin_segment | expected_filename |
      | 1    | attachment; filename="report.pdf"    | data.bin      | origin.bin     | report.pdf        |
      | 2    | （无）                               | dataset.tar.gz| origin.zip     | dataset.tar.gz    |
      | 3    | （无）                               | （末段为空）  | archive.iso    | archive.iso       |

  Scenario: 01-add-task-09 文件名无法推导回退 download-<时间戳>
    Given fixture 提供下载项：Content-Disposition 缺失、探测跳转落点与原始 URL 路径末段均为空
    When 按对话框添加该下载项并确认
    Then 目标文件名匹配 download-<数字时间戳> 格式

  Scenario: 01-add-task-10 目标文件已存在自动追加序号
    Given 保存目录 <dir> 已存在文件 "<filename>"
    When 按对话框添加一个将推导出同名目标文件的下载项并确认
    Then 新任务目标文件名为 "<filename>.1" 且既有文件内容未被改动

    Examples:
      | dir       | filename   |
      | /tmp/qadl | report.pdf |

  Scenario: 01-add-task-11 保存目录缺省链、打开预填与归一（v1.35 重写：~ 展开 + 相对→绝对，FR-01-03 v1.38/D62）
    Given 配置文件 download_dir 为 "<config_dir>"
    When 按 A 打开「添加任务：HTTP(S)」对话框且未触碰保存目录字段
    Then 保存目录字段预填 "<prefilled>"（配置未设/空白 → 代码默认；FR-01-01 v1.38/D48 同口径）
    When 按对话框添加下载项且保存目录字段输入 "<input_dir>"
    And 按 Enter 确认
    Then 目标文件落在 "<resolved_dir>" 下（目录不存在时已自动创建）
    And 详情「路径」与注册表保存目录均为归一后绝对路径 "<resolved_dir>"（~ 展开用户主目录、相对路径按创建时工作目录转绝对、末尾 / 去除）

    Examples:
      | config_dir      | input_dir      | prefilled       | resolved_dir             |
      | /tmp/qadl-home  | （留空）       | /tmp/qadl-home  | /tmp/qadl-home           |
      | （配置缺失）    | （留空）       | （用户主目录下载目录）| （用户主目录下载目录） |
      | /tmp/qadl-home  | /tmp/qadl-new  | /tmp/qadl-home  | /tmp/qadl-new            |
      | （配置缺失）    | ~/qadl-docs    | （用户主目录下载目录）| （用户主目录）/qadl-docs |
      | （配置缺失）    | qadl-rel/sub   | （用户主目录下载目录）| （启动时工作目录）/qadl-rel/sub |

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
    When 按 A 打开「添加任务：HTTP(S)」对话框
    And 以终端粘贴（bracketed paste）向 URL 字段粘贴 <length> 字符的长 URL
    Then URL 字段完整显示粘贴内容
    And 按 Enter 确认后任务创建成功

    Examples:
      | length |
      | 200    |

  Scenario: 01-add-task-18 仅添加按钮：只添加任务不下载
    When 按 A 打开「添加任务：HTTP(S)」对话框
    Then 按钮行显示 [ 立即下载 ] [ 仅添加 ] [ 取消 ]（原「确认」钮已改名「立即下载」，FR-01-103；对话框 74×12、顶边框行右端 [✗] 关闭按钮——v1.34/FR-01-114/115，原地松开 = 取消）
    And 焦点经 Tab/↑↓ 环绕可达三钮（序 6/7/8）
    And 对话框无底部操作提示行（v1.30/FR-01-107 P3：交互说明收敛帮助窗口【弹窗】组；按钮激活 = Enter/空格/鼠标点击）
    When 输入合法 URL "<url>" 后激活「仅添加」（Enter 或空格或鼠标点击）
    Then 对话框关闭且任务列表出现任务 "<filename>" 且状态为「已暂停」
    And 下载槽位未被占用（下载槽位 x/5 中 x 不含该任务）且任务不开始下载
    And toast 显示 "已添加任务" 且文案含「已暂停，按空格开始下载」（D29 钦定文案）
    And 目标文件与 .downloading 未落盘（未发起传输）
    When 选中该任务按空格继续
    Then 任务转为「等待中」或「下载中」并开始传输（FR-01-33 既有链路）
    When 按 A 重新打开对话框、输入非法 URL "<bad_url>" 后激活「仅添加」
    Then 出现 URL 非法 toast 报错且对话框未关闭且未创建任务（校验链与立即下载一致）

    Examples:
      | url                                       | filename    | bad_url                    |
      | http://fixture.local/files/paused-one.bin | paused-one.bin | ftp://fixture.local/file   |
