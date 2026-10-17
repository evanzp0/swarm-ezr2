# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   01-integrity-check-01 CLI -x <算法>=<校验码> 显式覆盖七种算法
#   01-integrity-check-02 显式校验值优先于伴随文件
#   01-integrity-check-03 伴随文件按算法后缀识别（大小写不敏感）
#   01-integrity-check-04 伴随文件接受裸 hex 与 hex 文件名两种格式
#   01-integrity-check-05 伴随文件摘要位数不符视为无效
#   01-integrity-check-06 多伴随文件并存按算法表声明顺序取最先
#   01-integrity-check-07 校验成功流转已完成并收尾
#   01-integrity-check-08 校验失败停等不自动重试
#   01-integrity-check-09 校验失败 R 重新校验且修正来源后通过
#   01-integrity-check-10 无期望值直接完成显示无校验
#   01-integrity-check-11 文件大小不符失败并可续传补齐
#   01-integrity-check-12 校验失败后清空校验码按 R 不校验直接完成（v1.17 新增）
Feature: 01-integrity-check 完整性校验（七算法 · 来源优先级 · 校验行为）

  期号 01。依据 project/mission/phase-01.md FR-01-50~52、D14 定案与 D15 R2 修订
  （CLI -x 以 <算法>=<校验码> 显式提供算法，不按位数匹配）。算法表：
  Adler-32=8、MD5=32、SHA-1=40、SHA-224=56、SHA-256=64、SHA-384=96、SHA-512=128 位。

  Background:
    Given ezr 以干净环境启动（独立 HOME，无历史注册表与配置文件）
    And 本地 fixture 服务器已启动且根目录含测试文件
    And 保存目录为 <save_dir>

  Scenario: 01-integrity-check-01 CLI -x <算法>=<校验码> 显式覆盖七种算法
    When 以命令行启动 "ezr <url> -x <x_arg>"
    Then 启动成功且详情校验行显示算法 "<algo>"

    Examples:
      | url                                | x_arg                                                                                    | algo     |
      | http://fixture.local/files/a0.bin  | adler32=d869db7f                                                                          | Adler-32 |
      | http://fixture.local/files/a1.bin  | md5=d869db7fe62fb06d1c488a04d5e68e93                                                      | MD5      |
      | http://fixture.local/files/a2.bin  | sha1=d869db7fe62fb06d1c488a04d5e68e93cb4d5f2a                                              | SHA-1    |
      | http://fixture.local/files/a3.bin  | sha224=d869db7fe62fb06d1c488a04d5e68e93cb4d5f2a3e4f5a6b                                    | SHA-224  |
      | http://fixture.local/files/a4.bin  | sha256=d869db7fe62fb06d1c488a04d5e68e93a4f0c0e0e0c0a1b2c3d4e5f60718293a                    | SHA-256  |
      | http://fixture.local/files/a5.bin  | sha384=d869db7fe62fb06d1c488a04d5e68e93a4f0c0e0e0c0a1b2c3d4e5f60718293a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e | SHA-384 |
      | http://fixture.local/files/a6.bin  | sha512=d869db7fe62fb06d1c488a04d5e68e93a4f0c0e0e0c0a1b2c3d4e5f60718293a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6a7b8c9d | SHA-512 |

  Scenario: 01-integrity-check-02 显式校验值优先于伴随文件
    Given 保存目录已存在内容正确 "<algo>" 摘要的伴随文件 "<file>.<ext>"
    When 按对话框添加该文件并显式提供与文件内容一致的 "<algo2>" 校验值
    And 下载完成进入校验
    Then 校验按显式 "<algo2>" 期望值进行并显示「<algo2> 校验成功」（未采用伴随文件）

    Examples:
      | file       | algo    | ext     | algo2   |
      | verify2.bin | SHA-256 | sha256  | MD5     |

  Scenario: 01-integrity-check-03 伴随文件按算法后缀识别（大小写不敏感）
    Given 保存目录已预置 "<file>.<ext>"，内容为该文件的正确 <algo> 十六进制摘要
    And 任务未显式提供校验值
    When 下载完成进入校验
    Then 列表与详情显示「<algo> 校验成功」

    Examples:
      | file        | ext     | algo     |
      | v-md5.bin   | md5     | MD5      |
      | v-sha1.bin  | sha1    | SHA-1    |
      | v-sha224.bin| sha224  | SHA-224  |
      | v-sha256.bin| sha256  | SHA-256  |
      | v-sha256.bin| SHA256  | SHA-256  |
      | v-sha384.bin| sha384  | SHA-384  |
      | v-sha512.bin| sha512  | SHA-512  |
      | v-adler.bin | adler32 | Adler-32 |

  Scenario: 01-integrity-check-04 伴随文件接受裸 hex 与 hex 文件名两种格式
    Given 保存目录已预置 "<file>.<ext>"，内容格式为 "<format>"
    When 下载完成进入校验
    Then 校验正常执行并显示「<algo> 校验成功」

    Examples:
      | file        | ext    | format              | algo |
      | fmt-a.bin   | md5    | 裸 hex 摘要         | MD5  |
      | fmt-b.bin   | md5    | hex 空格 文件名     | MD5  |

  Scenario: 01-integrity-check-05 伴随文件摘要位数不符视为无效
    Given 保存目录已预置 "<file>.<ext>"，其摘要为 <digits> 位（与 <algo> 期望 <expected> 位不符）
    When 下载完成
    Then 出现「校验伴随文件无效」类 toast 提醒
    And 忽略校验直接转「已完成」且显示「无校验」

    Examples:
      | file       | ext    | digits | algo    | expected |
      | badlen.bin | sha256 | 32     | SHA-256 | 64       |

  Scenario: 01-integrity-check-06 多伴随文件并存按算法表声明顺序取最先
    Given 保存目录同时存在 "<file>.md5"（正确）与 "<file>.adler32"（正确）
    And 任务未显式提供校验值
    When 下载完成进入校验
    Then 按算法表声明顺序取 MD5 执行校验
    And 显示「MD5 校验成功」

    Examples:
      | file       |
      | multi.bin  |

  Scenario: 01-integrity-check-07 校验成功流转已完成并收尾
    Given 任务 "<file>" 的校验期望值为正确的 "<algo>" 摘要
    When 下载完成
    Then 任务转「校验中」且期间继续占用下载槽位
    And 校验通过后转「已完成」
    And 列表与详情显示「<algo> 校验成功」
    And 目标文件 .downloading 扩展名已移除且 sidecar 已删除
    And 下载槽位已释放

    Examples:
      | file        | algo    |
      | oksha.bin   | SHA-256 |

  Scenario: 01-integrity-check-08 校验失败停等不自动重试
    Given 任务 "<file>" 的校验期望值与文件实际内容不符
    When 下载完成进入校验
    Then 任务转「已失败」且原因显示「<algo> 校验失败：内容与校验值不符」
    And 列表行显示「不自动重试」且释放槽位

    Examples:
      | file        | algo    |
      | badsha.bin  | SHA-256 |

  Scenario: 01-integrity-check-09 校验失败 R 重新校验且修正来源后通过
    Given 任务 "<file>" 已因「<algo> 校验失败」停等
    When 按 R
    Then 不发生任何块重传（fixture 无新 Range 请求）且重新校验后仍失败
    When 将伴随文件 "<file>.<ext>" 修正为正确摘要后再次按 R
    Then 重新校验通过并显示「<algo> 校验成功」转「已完成」

    Examples:
      | file        | algo    | ext    |
      | fixsha.bin  | SHA-256 | sha256 |

  Scenario: 01-integrity-check-10 无期望值直接完成显示无校验
    Given 任务 "<file>" 无显式校验值且保存目录无任何校验伴随文件
    When 下载完成
    Then 任务直接转「已完成」且不经过「校验中」
    And 列表与详情显示「无校验」
    And 下载槽位随下载结束释放

    Examples:
      | file        |
      | plain.bin   |

  # 块记账不变式：块须全部标完成才触发大小校验，而标完成的洞无法续传补齐——
  # 故「文件大小不符」终态与可补齐互斥；Range 短响应实测走网络类瞬态 + 续传补齐主链。
  Scenario: 01-integrity-check-11 Range 短响应按网络类瞬态续传补齐
    Given fixture 对文件 "<file>" 的 Range 响应返回短数据（实际写入字节数与 Content-Length 不符）
    When 任务分块全部结束
    Then 短响应按网络类瞬态失败处理并进入自动重试
    And 退避倒计时后续传补齐缺失字节
    And 重试成功后下载完成且文件字节完整
    And 「文件大小不符」失败文案与可补齐互斥（块记账不变式下正常路径不可达）

    Examples:
      | file        |
      | shorty.bin  |

  # v1.17 新增（FR-01-51 v1.17 修订③ + D34；操作者 20261009 第十四批指令缺陷 B）：
  # 校验失败型任务的 R 重校验期望值解析三分支之③——伴随缺失且任务显式校验值
  # 已清空（03-modify-task-05）→ 不校验：不重传、不进入「校验中」，走正常重新
  # 排队，引擎按全块完成 sidecar + 无期望值直接收尾「已完成」（显示「无校验」）。
  # 原实现在此分支仍发空期望值校验指令（空串比对恒败），「已失败」R 后仍显示
  # 校验失败，废止。
  # v1.18 注（FR-01-51 v1.18 修订 + D34 注记；操作者 20261009 第十六批指令缺陷）：
  # ③的收尾路径精化为**文件完整性直判**——文件在且大小一致时不经引擎直接收尾
  # （v1.17 实现经「正常重新排队 → 引擎恢复盘上 sidecar」，而完成臂无 sidecar
  # 终态落盘，恢复的是最后一次 2s 周期落盘的过期台账 → 进度回退 + 尾巴重传，
  # 快速下载无 sidecar 时整体重下——均违反本场景「不发生任何块重传」行，废止）；
  # 文件缺失/大小不符（外部改动）→ 作废断点从头重下。配套引擎修订：全块完成 +
  # 有校验值完成时即时落盘终态 sidecar（「校验中」退出重启的续传入口同根修复）。
  Scenario: 01-integrity-check-12 校验失败后清空校验码按 R 不校验直接完成
    Given 任务 "<file>" 已因「<algo> 校验失败」停等且保存目录无任何校验伴随文件
    When 按 m 清空校验码并确定修改
    And 按 R
    Then 不发送校验指令、不发生任何块重传（fixture 无新 Range 请求）
    And 任务不经过「校验中」直接转「已完成」且显示「无校验」
    And 已下载文件保留在保存目录（.downloading 后缀已去除）

    Examples:
      | file        | algo    |
      | badsha.bin  | SHA-256 |
