# QA 套件 · 01-integrity-check 完整性校验（期号 01）

> 对应规格：`project/features/01-integrity-check.feature`（11 场景）。
> 端到端 UI 层验证：TUI 状态/校验文案断言 + 磁盘伴随文件预置 + CLI 参数 + fixture 区间日志。

## 环境前置

- fixture 能力：Range 日志（判定「无块重传」）、文件内容可控（正确/损坏内容两形态）。
- 预置工具：为各 fixture 文件计算 7 种算法摘要并按需写伴随文件（`<file>.md5/.sha1/.sha224/.sha256/.sha384/.sha512/.adler32`，含大写后缀、两种内容格式、位数错误内容、多伴随并存）。
- 伪终端驱动（列表/详情校验行文案断言）；独立 HOME；保存目录独立可清。
- 校验值均为正确/错误的确定性样本（预计算），不依赖外部网络。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-IC-01 | 01-integrity-check-01 | CLI `-x` 分别给 8/32/40/56/64/96/128 位码 | 详情算法依次 Adler-32/MD5/SHA-1/SHA-224/SHA-256/SHA-384/SHA-512 |
| QA-IC-02 | 01-integrity-check-02 | 预置正确 .sha256 + 显式提供正确 MD5 值 | 显示「MD5 校验成功」（显式优先，未用伴随） |
| QA-IC-03 | 01-integrity-check-03 | 分别预置 7 算法伴随（含 .SHA256 大写行） | 各显示对应「<算法> 校验成功」 |
| QA-IC-04 | 01-integrity-check-04 | 伴随内容分别为裸 hex / `hex  文件名` | 两种格式均识别并校验成功 |
| QA-IC-05 | 01-integrity-check-05 | .sha256 内容为 32 位 | toast 无效提醒；忽略校验直接完成显示「无校验」 |
| QA-IC-06 | 01-integrity-check-06 | 同时预置 .adler32 与 .md5（均正确） | 取 Adler-32 校验成功（算法表声明顺序） |
| QA-IC-07 | 01-integrity-check-07 | 正确 SHA-256 任务下载完成 | 校验中占槽→已完成；显示「SHA-256 校验成功」；.downloading 移除；sidecar 删除；槽位释放 |
| QA-IC-08 | 01-integrity-check-08 | 错误 SHA-256 期望值 | 已失败「SHA-256 校验失败：内容与校验值不符」；「不自动重试」；释放槽位（AC-6） |
| QA-IC-09 | 01-integrity-check-09 | 失败后 R→修正伴随→R | 首次 R 无 Range 请求且仍失败；修正后 R 显示校验成功转已完成（AC-6） |
| QA-IC-10 | 01-integrity-check-10 | 无显式值无伴随 | 直接已完成（不经校验中）；显示「无校验」；槽位释放 |
| QA-IC-11 | 01-integrity-check-11 | fixture Range 短响应 | 失败「文件大小不符」；自动重试续传补齐后完成 |

## 通过准则

- 全部用例 P/F/S；S 内嵌环境判据。QA-IC-08/09 对应 AC-6 全链路。
