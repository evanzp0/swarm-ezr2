# QA 套件 · 01-download-engine HTTP 下载内核（期号 01）

> 对应规格：`project/features/01-download-engine.feature`（16 场景）。
> 端到端 UI 层验证：TUI 可观察状态 + fixture 请求记录（fixture 属 QA 工具入口，允许）+ 磁盘产物。

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
| QA-DE-04 | 01-download-engine-04 | 分别添加 2.5MB/1MB/512KB/10MB | 分块行 `0/3`·`0/1`·`0/1`·`0/10` · 1 MB/块 |
| QA-DE-05 | 01-download-engine-05 | 12MB 并发 4 / 3MB 并发 8 下载 | 明细表同持块连接数 ≤设置值；乱序完成；完成行 `12/12`·`3/3` |
| QA-DE-06 | 01-download-engine-06 | 2MB（2 块）并发 4 | 明细表 2 传输 + 2「待命」；最终完成 |
| QA-DE-07 | 01-download-engine-07 | 8MB 下到 40% 暂停→继续（区间日志） | 续传请求区间与已完成块零交集；最终字节完整 |
| QA-DE-08 | 01-download-engine-08 | 添加 2 跳重定向 URL | 经重定向完成；详情 URL 为最终 URL |
| QA-DE-09 | 01-download-engine-09 | 添加 11 跳重定向 URL | 失败「重定向次数超限」；「不自动重试」；不占槽 |
| QA-DE-10 | 01-download-engine-10 | 添加重定向至 ftp:// 的 URL | 失败「不支持的重定向协议」；「不自动重试」 |
| QA-DE-11 | 01-download-engine-11 | 自签 HTTPS URL | 失败原因含 TLS 证书错误；无跳过校验开关 |
| QA-DE-12 | 01-download-engine-12 | 受信证书 HTTPS URL | 下载完成字节完整；类型 HTTPS |
| QA-DE-13 | 01-download-engine-13 | 下载 5MB（请求头记录） | 全部分块请求 `Accept-Encoding: identity` |
| QA-DE-14 | 01-download-engine-14 | 下载中观察列表/头部/Sparkline | 三处速度每秒真实更新；完成后归零/完成态 |
| QA-DE-15 | 01-download-engine-15 | 槽位占满时添加 5MB（预取）+ 探测必失败项 | 等待行分别显示 `0 B/5.0 MB` 与「未知」 |
| QA-DE-16 | 01-download-engine-16 | 默认并发 4 下载 big-100m.bin（含 .sha256） | 分块行 x/100 · 1 MB/块；活跃连接 4；乱序完成；「SHA-256 校验成功」；字节数 104857600（AC-1） |

## 通过准则

- 全部用例 P/F/S；S 内嵌环境判据。QA-DE-16 为 AC-1 直接对应用例，必须实跑。
