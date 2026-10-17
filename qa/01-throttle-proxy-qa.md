# QA 套件 · 01-throttle-proxy 全局限速（期号 01）

> 对应规格：`project/features/01-throttle-proxy.feature`（7 场景）。
> 端到端 UI 层验证：TUI 速度读数断言 + 配置文件/CLI 参数 + fixture 门控。
> （v1.6：原 QA-TP-08~11 代理用例随旧全局 proxy 键退役删除（FR-01-90），
> 代理语义由 02-named-proxy / QA-NP 套件承载。）
> （v1.7/v141：TP-02/03/06 实跑化——TP-01 同款 100MB 大文件总量口径，
> 见「速度断言口径」；本套件无常设 S 登记项。）

## 环境前置

- fixture 服务器慢速门控（保证限速下限可观测）。
- fixture 文件：`five-m.bin`、`streamy.bin`（无 Range）、`big-100m.bin`
  （104,857,600 B）、12MB 等。
- 伪终端驱动（详情「大小」读数采样）；独立 HOME。
- 速度断言口径（v1.7 权威量法，AC-7 同源）：100MB 大文件使启动瞬态（桶突发
  0.5s + socket 预读）摊薄至 <3%，跳过前 10MB 瞬态窗后按「总量/完成耗时」
  两端精确计（详情「大小」为字节级实时口径，FR-01-105），容差 ±10%；
  显示速度按块粒度跳变，不作断言依据。
- 分批复跑：套件全量实跑 >13min，支持 `QA_TP_ONLY=1,3` 环境变量过滤用例
  分批前台执行（缺省全量，结果仅含所跑用例）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-TP-01 | 01-throttle-proxy-01 | 配置 `max_speed = "1 MB/s"` 单任务下载 | 稳定速度 1 MB/s ±10%（AC-7） |
| QA-TP-02 | 01-throttle-proxy-02 | 同配置 2 任务并发下载 big-100m（`?cd=` 区分命名 + 独立子目录） | 合计速度（2×100MB − 组合瞬态窗 20MB）/（双完成时刻 − 窗时刻）∈ 1 MB/s ±10% |
| QA-TP-03 | 01-throttle-proxy-03 | 配置 1 MB/s + CLI `--max-speed 512 KB/s` | 稳定速度 512 KB/s ±10%（512K+10% 与 1M−10% 不相交，测得 512K 档即证参数优先，FR-01-60） |
| QA-TP-04 | 01-throttle-proxy-04 | `max_speed = 0` / `"abc"` | 分别为不限速 / 回退默认不限速 |
| QA-TP-05 | 01-throttle-proxy-05 | 分别配置 2MB/s、2 mb/s、500KB/s、1GB/s、2000000 B/s | 各按十进制口径生效（1 MB=1000 KB） |
| QA-TP-06 | 01-throttle-proxy-06 | 限速 1 MB/s + `big-100m.bin?norange=1`（200 全量 → 单流） | 速度同样受限 ±10%；详情「不支持断点续传」；除 bytes=0- 探测外无分块 Range 请求（FR-01-10，DE-02 同款证据面） |
| QA-TP-07 | 01-throttle-proxy-07 | 检查全部界面区块与页脚 | 无限速控件；头部/图表布局不变 |

## 通过准则

- 全部用例 P/F/S；QA-TP-01 对应 AC-7。（v1.7：TP-02/03/06 实跑化后本套件
  无常设 S 登记项；v138–v140 历史 S 登记与处置轨迹见各版交接。）
- （v1.6：AC-8 代理判据移至 02-named-proxy / QA-NP 套件。）
