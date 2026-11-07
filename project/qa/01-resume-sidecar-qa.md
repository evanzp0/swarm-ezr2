# QA 套件 · 01-resume-sidecar 断点续传与 sidecar（期号 01）

> 对应规格：`project/features/01-resume-sidecar.feature`（15 场景）。
> 端到端 UI 层验证：TUI 状态 + 磁盘产物（目标文件/.ezr sidecar）+ kill/重启操作 + fixture 不变量控制。
> v1.45 同步（FR-01-26 守卫臂）：QA-RS-15 新增（原任务存活时守卫触发 → 序号新下）；占用判定与静默口径见场景 15。
> v1.46 同步（操作者审批改判）：QA-RS-14 改判（异 URL 死任务孤儿接续 → 探测不符序号改道 `<名>.1` + 孤儿保留手工删，原地作废重下臂退役）；QA-RS-13 补判别前提注（探测相符 → 原地接续）；QA-RS-15 不变。

## 环境前置

- fixture 能力：ETag/Last-Modified 可动态变更、可停止返回一致性头、同 URL 可换内容（大小变化）、异路径同推导名文件（如 files/alias.bin 与 other/alias.bin 内容不同，供路径基孤儿接续改判用例，v1.46）、断连注入、区间请求日志。
- 伪终端驱动；独立 HOME；每用例独立保存目录。
- kill -9 注入工具（对 ezr 进程）；重启脚本（保持注册表与保存目录）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-RS-01 | 01-resume-sidecar-01 | 下载 8MB 至中段 | 目标旁存在 `<file>.ezr`；含大小/ETag/LM/块状态/元信息且不含任何 URL 字段（文件级观察，v1.28/FR-01-20） |
| QA-RS-02 | 01-resume-sidecar-02 | 下载期间 3 次 kill -9，每次重启续传 | 每次 sidecar 完整可读；最终完成字节完整（NFR-2） |
| QA-RS-03 | 01-resume-sidecar-03 | 8MB 下到 40% Space 暂停→继续 | 续传请求无已下载块区间；字节完整 |
| QA-RS-04 | 01-resume-sidecar-04 | 8MB 传输中注入断连 | 自动重试续传；进度不回退；最终完整 |
| QA-RS-05 | 01-resume-sidecar-05 | 下到 30% kill -9→重启 | 任务回等待中排队；获槽断点续传；完成（AC-2） |
| QA-RS-06 | 01-resume-sidecar-06 | 预置 5 状态任务后 kill -9→重启 | 全部任务与历史按序恢复；下载中→等待中，其余保持 |
| QA-RS-07 | 01-resume-sidecar-07 | 暂停后分别变更 ETag/LM/大小/全部头缺失→继续 | 各行均 toast「服务器内容已更新…」；转等待不计失败；从头重下（v1.28：一致性三要素 ETag/LM/大小，无 URL 腿） |
| QA-RS-08 | 01-resume-sidecar-08 | 同一任务连续 3 次一致性失效 | 转已失败「服务器内容持续变化」；不自动重试；释放槽位 |
| QA-RS-09 | 01-resume-sidecar-09 | 停等后恢复一致再按 R | 计数清零重新探测续传；需再 3 次才停等 |
| QA-RS-10 | 01-resume-sidecar-10 | 无校验/有伴随校验两种任务 | 下载中仅 `<file>.downloading`；完成后裸名存在、sidecar 删除 |
| QA-RS-11 | 01-resume-sidecar-11 | 下载中 D→仅删除任务 | 记录消失；文件+sidecar 保留 |
| QA-RS-12 | 01-resume-sidecar-12 | 下载中/已完成任务 D→删除任务和文件 | 目标、sidecar（如存）、记录全删 |
| QA-RS-13 | 01-resume-sidecar-13 | 仅删任务后重加同 URL+目录（fixture 内容未变，探测相符） | toast 接续提示；不追加序号；进度断点合并；续传完成（v1.46 明示判别前提：相符 → 原地接续） |
| QA-RS-14 | 01-resume-sidecar-14 | 仅删任务后重加同推导名异 URL+目录（对话框；CLI 同口径抽测） | 探测三要素不符 → 新任务不沿用原名与 sidecar、无「服务器内容已更新」原地重下；目标落 `<名>.1` 全新下载且静默无提示；孤儿 `<名>.ezr` 与部分文件 `<名>` 原样保留不自动删除（由用户手工删除，v1.46 改判/FR-01-26+FR-01-22 路径基孤儿臂） |
| QA-RS-15 | 01-resume-sidecar-15 | 保留 URL <old_url> 的存活任务（下载中 ≥30%）不删除，重加同推导名异 URL 任务（对话框；CLI 同口径抽测） | 新任务不接续 sidecar、目标落 `<名>.1` 全新下载且静默无提示；既有任务文件/sidecar/断点不变且继续原下载（v1.45/FR-01-26 守卫臂） |

## 通过准则

- 全部用例 P/F/S；S 内嵌环境判据。QA-RS-05 对应 AC-2、QA-RS-07/08/09 对应 D11 与本会话决定 #5。
