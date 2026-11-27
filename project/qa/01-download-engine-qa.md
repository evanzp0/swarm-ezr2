# QA 套件 · 01-download-engine HTTP 下载内核（期号 01）

> 对应规格：`project/features/01-download-engine.feature`（16 场景）。
> 端到端 UI 层验证：TUI 可观察状态 + fixture 请求记录（fixture 属 QA 工具入口，允许）+ 磁盘产物。
> **v1.49 锚点对齐（specifier 遗留项处理轮 20261112，QA 终验轮移交待批项②）**：QA-DE-15 判据修订——原「等待行显示『未知』」为 v1.28 期观察锚，v1.49（FR-02-38 v1.19）尾随尺寸/未知项整项退役后改「等待行不显示尺寸（尺寸未知不显示、无预取，以 fixture 请求日志为准）」，与 runner suite_download_engine.py QA-DE-15 现行断言一致。
> **v1.51 同步（phase-05 引擎提速轮 20261125，FR-05-01~07、D83/D84）**：默认引擎改动态分段
> 调度——QA-DE-04/05/06/07 块模型断言钉定 blocks 回退模式（config scheduler="blocks"）；
> **v1.63 同步（族A/族B 裁决落实轮 20261127，phase-05 v1.1/D90–D93）**：blocks 引擎整体退役
> （D91）——QA-DE-04/05/06/07 退役留空（见用例表）；QA-DE-16 动态口径不变。族B：worker 块
> 请求面 4xx/断连按 FR-05-04 原地重试 + Transient 分类、Range 全拒降级完成不失败、有进展
> 反复断连不失败（D93，旧预期废弃）。
> QA-DE-16（AC-1）随新默认改判（默认并发 8、动态分段口径 D83）；动态分段/裸 GET/抢占/
> 原地重试等新行为由 `qa/13-http-turbo-qa.md` 承载。

## 环境前置

- 本地 fixture 服务器能力：Range 支持/关闭开关、隐藏 Content-Length 模式、重定向链（可配长度与目标协议）、HTTPS（自签证书与受信证书双形态）、请求头记录（Accept-Encoding、Range 区间）、请求区间日志。
- fixture 文件集：`five-m.bin`(5MB)、`three-m.bin`(3MB)、`eight-m.bin`(8MB)、12MB、3MB、2MB、1MB、512KB、10MB 等尺寸文件、`big-100m.bin`(100MB 稀疏) 及其 `.sha256` 伴随、`ghost-404.bin`（探测必失败）、`streamy.bin`（无 Content-Length）。
- 伪终端驱动（画面断言：详情面板、分块明细表、Sparkline）；独立 HOME；保存目录独立。
- 网络隔离：除 fixture 本机地址外不可达（保证走 fixture）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-DE-01 | 01-download-engine-01 | 添加 5MB 支持续传文件并开始 | 详情显示大小、HTTP、「支持断点续传」 |
| QA-DE-02 | 01-download-engine-02 | 添加无 Content-Length 下载项 | 详情「不支持断点续传」；仅 1 活跃连接；最终字节完整 |
| QA-DE-03 | 01-download-engine-03 | Range 关闭文件下到 30%→Space 暂停→Space 继续 | fixture 新请求为从头全量（无 Range）；详情「不支持断点续传」；最终完整（AC-4） |
| QA-DE-04 | 01-download-engine-04 | **（退役，不执行）** 原块模型分块计数用例随 blocks 引擎退役（v1.63/族A，D91——scheduler 键退役，blocks 模式不复存在） | **（无判据）** 本用例退役留空不重排（修订纪律三层同步口径）；动态分段显示口径由 QA-HT-01/02（D83）与 QA-UP-01 承载，执行时直接记 S 并注记「D91 退役，行为归 QA-HT-01/02」 |
| QA-DE-05 | 01-download-engine-05 | **（退役，不执行）** 原块模型动态领块用例随 blocks 引擎退役（v1.63/族A，D91） | **（无判据）** 本用例退役留空不重排；动态多连接乱序形态由 QA-HT-01/02 与 QA-DE-16 承载，执行时直接记 S 并注记「D91 退役，行为归 QA-HT-01/02」 |
| QA-DE-06 | 01-download-engine-06 | **（退役，不执行）** 原队列临近结束待命用例随 blocks 引擎退役（v1.63/族A，D91） | **（无判据）** 本用例退役留空不重排；并发上限与待命观察由 QA-HT-02/07 承载，执行时直接记 S 并注记「D91 退役，行为归 QA-HT-02/07」 |
| QA-DE-07 | 01-download-engine-07 | **（退役，不执行）** 原块模型封闭区间续传用例随 blocks 引擎退役（v1.63/族A，D91） | **（无判据）** 本用例退役留空不重排；动态续传「已提交字节零重复写入」由 QA-RS-03 与 QA-HT-14 承载（D87），执行时直接记 S 并注记「D91 退役，行为归 QA-RS-03/QA-HT-14」 |
| QA-DE-08 | 01-download-engine-08 | 添加 2 跳重定向 URL（请求日志开） | 经重定向完成；详情 URL 为原始 URL（v1.28/FR-01-14）；fixture 日志显示每个分块请求均从原始 URL 独立发起并跟跳转（无共享探测落点） |
| QA-DE-09 | 01-download-engine-09 | 添加 11 跳重定向 URL | 失败「重定向次数超限」；「不自动重试」；不占槽 |
| QA-DE-10 | 01-download-engine-10 | 添加重定向至 ftp:// 的 URL | 失败「不支持的重定向协议」；「不自动重试」 |
| QA-DE-11 | 01-download-engine-11 | 自签 HTTPS URL | 失败原因含 TLS 证书错误；无跳过校验开关 |
| QA-DE-12 | 01-download-engine-12 | 受信证书 HTTPS URL | 下载完成字节完整；类型 HTTPS |
| QA-DE-13 | 01-download-engine-13 | 下载 5MB（请求头记录） | 全部分块请求 `Accept-Encoding: identity` |
| QA-DE-14 | 01-download-engine-14 | 下载中观察列表/头部/头部右侧内嵌流量图（v1.13 布局） | 三处速度每秒真实更新；完成后归零/完成态 |
| QA-DE-15 | 01-download-engine-15 | 槽位占满时添加 5MB 观察等待行（fixture 请求日志无探测）；获槽开始后再观察详情 | 等待行不显示尺寸（尺寸未知不显示、无预取，以 fixture 请求日志为准）；获槽后详情显示 5.0 MB |
| QA-DE-16 | 01-download-engine-16 | 默认配置（动态分段 · 并发 8，v1.51）下载 big-100m.bin（含 .sha256） | 分块行动态分段口径（随 <已下载>/<总字节> 推进，v1.51/D83）；实际并发 ≤ 8（并发面板观察）；多连接乱序交付；「SHA-256 校验成功」；字节数 104857600（AC-1） |

## 通过准则

- 全部用例 P/F/S；S 内嵌环境判据。QA-DE-16 为 AC-1 直接对应用例，必须实跑。
