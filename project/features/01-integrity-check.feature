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
    Given 保存目录同时存在 "<file>.adler32"（正确）与 "<file>.md5"（正确）
    And 任务未显式提供校验值
    When 下载完成进入校验
    Then 按算法表声明顺序取 Adler-32 执行校验
    And 显示「Adler-32 校验成功」

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

  Scenario: 01-integrity-check-11 文件大小不符失败并可续传补齐
    Given fixture 对文件 "<file>" 的 Range 响应返回短数据（实际写入字节数与 Content-Length 不符）
    When 任务分块全部结束触发大小校验
    Then 任务转「已失败」且失败原因显示「文件大小不符」
    And 任务按自动重试类处理（退避倒计时后续传补齐缺失字节）
    And 重试成功后下载完成且文件字节完整

    Examples:
      | file        |
      | shorty.bin  |
