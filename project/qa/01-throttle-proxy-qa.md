# QA 套件 · 01-throttle-proxy 全局限速与代理（期号 01）

> 对应规格：`project/features/01-throttle-proxy.feature`（11 场景）。
> 端到端 UI 层验证：TUI 速度读数断言 + 配置文件/CLI 参数 + 环境变量（仅作代理不生效的负向验证）+ fixture 代理日志。

## 环境前置

- fixture 代理两个（proxy-a/proxy-b）+ 可记录经手请求与 CONNECT 隧道；fixture 服务器慢速门控（保证限速下限可观测）。
- fixture 文件：`five-m.bin`、`streamy.bin`（无 Range）、12MB 等。
- 伪终端驱动（头部 ↓ 读数采样）；独立 HOME；环境变量注入可控。
- 速度断言口径：传输稳定窗口（≥10s）采样均值，容差 ±10%（AC-7）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-TP-01 | 01-throttle-proxy-01 | 配置 `max_speed = "1 MB/s"` 单任务下载 | 稳定速度 1 MB/s ±10%（AC-7） |
| QA-TP-02 | 01-throttle-proxy-02 | 同配置 2 任务并发 | 合计速度 1 MB/s ±10% |
| QA-TP-03 | 01-throttle-proxy-03 | 配置 1 MB/s + CLI `--max-speed 512 KB/s` | 稳定速度 512 KB/s ±10%（参数优先） |
| QA-TP-04 | 01-throttle-proxy-04 | `max_speed = 0` / `"abc"` | 分别为不限速 / 回退默认不限速 |
| QA-TP-05 | 01-throttle-proxy-05 | 分别配置 2MB/s、2 mb/s、500KB/s、1GB/s、2000000 B/s | 各按十进制口径生效（1 MB=1000 KB） |
| QA-TP-06 | 01-throttle-proxy-06 | 限速 1 MB/s + 无 Range 文件单并发下载 | 速度同样受限 |
| QA-TP-07 | 01-throttle-proxy-07 | 检查全部界面区块与页脚 | 无限速控件；头部/图表布局不变 |
| QA-TP-08 | 01-throttle-proxy-08 | 配置 proxy-a + 环境变量 proxy-b 同时设 | 代理日志显示请求经 proxy-a（环境变量 proxy-b 不参与） |
| QA-TP-09 | 01-throttle-proxy-09 | 分别仅设 HTTP_PROXY(http 请求)/HTTPS_PROXY(https 请求)/ALL_PROXY | 均直连完成，代理日志无该请求（环境变量不读取） |
| QA-TP-10 | 01-throttle-proxy-10 | 配置文件 proxy 指向代理后下载 https URL | CONNECT 隧道记录；下载完整（AC-8 代理路径） |
| QA-TP-11 | 01-throttle-proxy-11 | proxy 含 `qa:s3cret-pw@` 凭据下载 | 界面/toast/日志无 `s3cret-pw` 出现 |

## 通过准则

- 全部用例 P/F/S；S 内嵌环境判据。QA-TP-01 对应 AC-7、QA-TP-08~10 对应 AC-8（配置文件途径）。
