# QA 套件 · 01-throttle-proxy 全局与任务级限速（期号 01，本轮 v1.9 修订属 phase-02 bug18 轮）

> 对应规格：`project/features/01-throttle-proxy.feature`（14 场景）。
> （**v1.10 修订（操作者 bug21 修订轮批准，FR-02-29 v1.10/D67②）**：config 键解析口径**不变仍十进制**（TP-01..08/10..14 的 config 键示例不变）；对话框限速字段二进制单位 + 键入/粘贴即规范化显示 + 占位退役（TP-09/11/12/13 示例随二进制口径调整）；HTTP 添加/修改对话框「上传限速」字段退役（BT 框保留）。）
> 端到端 UI 层验证：TUI 速度读数断言 + 配置文件/CLI 参数 + fixture 门控 + 对话框字段观察。
> （v1.6：原 QA-TP-08~11 代理用例随旧全局 proxy 键退役删除（FR-01-90），
> 代理语义由 02-named-proxy / QA-NP 套件承载。）
> （v1.7/v141：TP-02/03/06 实跑化——TP-01 同款 100MB 大文件总量口径，
> 见「速度断言口径」；本套件无常设 S 登记项。）
> （v1.9 修订（操作者 bug18 轮指令；FR-02-28/29，D67）：全局键 max_speed 改名
> global_max_download_speed 并新增 global_max_upload_speed（[common]）；生效范围扩展 =
> HTTP+BT 跨协议合计（BT 引擎真实接入，修复「BT 下载限速未接」遗留）；任务级两键
> task_max_download_speed / task_max_upload_speed（默认 0）+ 四对话框限速字段联动 +
> min(全局, 任务级) 合成 + 注册表持久化 + 旧键退役。TP-01..07 键名/口径修订、TP-08..14 新增。）

## 环境前置

- fixture 服务器慢速门控（保证限速下限可观测）；BT fixture 四件套
  （HTTP/UDP tracker + seeder/leecher，见 09-bt-engine-qa 环境前置）。
- fixture 文件：`five-m.bin`、`streamy.bin`（无 Range）、`big-100m.bin`（104,857,600 B）、12MB 等。
- 伪终端驱动（详情「大小」读数采样）；独立 HOME。
- 速度断言口径（v1.7 权威量法，AC-7 同源）：100MB 大文件使启动瞬态摊薄至 <3%，
  跳过前 10MB 瞬态窗后按「总量/完成耗时」两端精确计（详情「大小」为字节级实时口径），
  容差 ±10%；显示速度按块粒度跳变，不作断言依据；BT 上传侧以 fixture leecher 拉取
  字节与本端头部 ↑ 读数对账。
- 分批复跑：套件支持 `QA_TP_ONLY=<编号列表>` 环境变量过滤用例分批前台执行
  （缺省全量，结果仅含所跑用例）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-TP-01 | 01-throttle-proxy-01 | 配置 `global_max_download_speed = "1 MB/s"` 单任务（HTTP）下载 | 稳定速度 1 MB/s ±10%（AC-7） |
| QA-TP-02 | 01-throttle-proxy-02 | 同配置 2 任务并发下载 big-100m（HTTP×2 与 HTTP+BT 两轮，`?cd=` 区分命名 + 独立子目录） | 合计速度 ∈ 1 MB/s ±10%（两轮均达标；HTTP+BT 轮以 BT 任务真实进度字节 + HTTP 详情字节合计对账——跨协议共享配额，FR-02-28） |
| QA-TP-03 | 01-throttle-proxy-03 | 配置 1 MB/s + CLI `--max-speed 512 KB/s` | 稳定速度 512 KB/s ±10%（512K+10% 与 1M−10% 不相交，测得 512K 档即证参数优先，FR-01-60/FR-02-28） |
| QA-TP-04 | 01-throttle-proxy-04 | `global_max_download_speed = 0` / `"abc"` | 分别为不限速 / 回退默认 0（不限速） |
| QA-TP-05 | 01-throttle-proxy-05 | 分别配置 2MB/s、2 mb/s、500KB/s、2000000（下载键）；1 MB/s（上传键 + BT leecher 拉取） | 各按十进制口径生效（1 MB=1000 KB）；上传轮全部任务上传合计 ∈ 1 MB/s ±10% |
| QA-TP-06 | 01-throttle-proxy-06 | 限速 1 MB/s + `big-100m.bin?norange=1`（200 全量 → 单流） | 速度同样受限 ±10%；详情「不支持断点续传」；除 bytes=0- 探测外无分块 Range 请求 |
| QA-TP-07 | 01-throttle-proxy-07 | 检查全部界面区块与页脚 | 头部/图表布局不变且无全局限速控件（添加/修改对话框任务级限速字段不算） |
| QA-TP-08 | 01-throttle-proxy-08 | 配置 `global_max_upload_speed = "1 MB/s"`，本端 BT 任务做种中，2 个 leecher fixture 同时拉取 | 全部任务上传合计 ∈ 1 MB/s ±10%（做种 + 互惠上传均计入；头部 ↑ 与 fixture 拉取字节对账）；HTTP 任务不受上传限速影响 |
| QA-TP-09 | 01-throttle-proxy-09 | 配置 task 两键（2 MB/s / 0 等 4 行 Examples）分别打开 HTTP 添加对话框与 BT 添加对话框 | HTTP 框**仅下载限速字段**（上传限速字段退役，v1.10）、BT 框两字段均在；预填 = config 解析整数 B/s（0 显示 0）、无占位提示；键入/粘贴即规范化显示（`500k` → `500 KiB/s`、`1 M` → `1 MiB/s`，前导零剥离；退格不回写）；确认后任务级限速落库 = Examples 预期（二进制：512000/1048576；留空或非法回退 config 默认） |
| QA-TP-10 | 01-throttle-proxy-10 | (全局, 任务级) ∈ {(1M,500K), (1M,0), (0,2M)} 下载观察 | 任务速度 ∈ min(全局, 任务级) ±10%（0 视为不限） |
| QA-TP-11 | 01-throttle-proxy-11 | HTTP 任务下载中按 M 修改下载限速 0→1 MiB/s→500 KiB/s→0（键入 1M 观察规范化显示；toast/注册表/回读核对） | 键入/粘贴即规范化显示（1M → 1 MiB/s）；任务保持下载中（不重启不重新排队），速度约一进度帧内向新值收敛；重开对话框预填新值；重启后保留 |
| QA-TP-12 | 01-throttle-proxy-12 | BT 任务下载中/做种中按 M 修改限速（fixture 观察；键入/规范化显示同 TP-11） | 引擎不重启（无重新排队/无启动重校验窗/既有连接不整体拆除），速度向新值收敛且与全局 min 合成；toast `✓ BT 任务已更新…（立即生效）` |
| QA-TP-13 | 01-throttle-proxy-13 | HTTP 与 BT 任务各设任务级限速后退出重启 | 预填与生效限速与重启前一致（注册表持久化） |
| QA-TP-14 | 01-throttle-proxy-14 | 配置文件仅含旧键 `[common] max_speed = "1 MB/s"` 启动 | 启动不报错、全局下载限速 0（旧键未知键静默忽略）；日志/警告无致命项 |

## 通过准则

- 全部用例 P/F/S；QA-TP-01 对应 AC-7（限速 `1 MB/s` 全局速度稳定 ±10%）。
- （v1.6：AC-8 代理判据移至 02-named-proxy / QA-NP 套件。）
- v1.9 起 TP-08/09/12 需 BT fixture（四件套见 09-bt-engine-qa）；BT 侧速度断言以
  fixture 字节对账为主、TUI 读数为辅（块粒度跳变不作断言依据）。
