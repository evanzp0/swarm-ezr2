# QA 套件 · 01-throttle-proxy 全局限速（期号 01）

> 对应规格：`project/features/01-throttle-proxy.feature`（7 场景）。
> 端到端 UI 层验证：TUI 速度读数断言 + 配置文件/CLI 参数 + fixture 门控。
> （v1.6：原 QA-TP-08~11 代理用例随旧全局 proxy 键退役删除（FR-01-90），
> 代理语义由 02-named-proxy / QA-NP 套件承载。）

## 环境前置

- fixture 服务器慢速门控（保证限速下限可观测）。
- fixture 文件：`five-m.bin`、`streamy.bin`（无 Range）、12MB 等。
- 伪终端驱动（头部 ↓ 读数采样）；独立 HOME。
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

## 通过准则

- 全部用例 P/F/S；S 内嵌环境判据。QA-TP-01 对应 AC-7。
- （v1.6：AC-8 代理判据移至 02-named-proxy / QA-NP 套件。）
