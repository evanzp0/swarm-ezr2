# QA 套件 · 01-config-template 配置模板自动生成（期号 01 · v1.4/FR-01-85）

> 对应规格：`project/features/01-config-template.feature`（5 场景）。
> 端到端 UI 层验证：CLI 启动/退出操作 + 磁盘产物观察（文件存在性/全注释/字节级对比）+
> TUI 默认值行为断言；不使用项目内部 API。
> v1.10（操作者第七批指令，模板最小化）：模板内容契约基线改为**操作者钦定文本整文本**
> （逐字节权威，规格内嵌原文行集）；QA-CT-01 判据同步为整文本逐字节比对。
> v1.11（操作者 bug11 轮指令，config 三大标准表重构）：钦定文本整体替换为 v1.11 三表版
> （168 行 = 标题行 + 3 表分隔头注释 + 36 键×3 行三行式 + [[common.proxy]] 一句头注释 +
> 双示例块各 7 行 + 41 空行，EOF 单换行收尾）；QA-CT-01 判据同步（36 键、三表头注释、
> [[common.proxy]] 双示例块）；QA-CT-02 前置配置改写为 [common] 表形态。
> v1.12（操作者 bug12 轮指令，模板注释格式修正）：钦定文本整体替换为 v1.12 版
> （174 行 = 标题行 + 3 组「分隔头注释 + 空行 + 表声明行」+ 36 键×3 行三行式（首行注释
> 无键名前缀）+ [[common.proxy]] 一句头注释 + 双示例块 + 44 空行，EOF 单换行收尾）；
> QA-CT-01 判据同步（两行式表头、无键名前缀、174 行）。

## 环境前置

- fixture 服务器（默认行为断言用简单下载即可）。
- 独立 HOME（每用例独立沙箱目录）；进程管理工具（pgrep/pkill）。
- `chmod`（目录只读场景）；字节级文件对比（`sha256sum`/`cmp`）。
- 模板内容契约基线（v1.12）：**操作者钦定模板文本整文本**（174 行，权威原文
  见规格 `01-config-template.feature` 内嵌原文，QA 会话脚本化时将其落盘为基线文件供
  `cmp` 逐字节比对）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-CT-01 | 01-config-template-01 | 干净 HOME 启动 ezr（无 config.toml）后退出 | `<HOME>/.ezr/config.toml` 已生成；文件内容与钦定文本**逐字节一致**（整文本 `cmp` 比对：标题行、3 组「分隔头注释 + 空行 + 表声明行」`# ===== 公共配置 =====`+`# [common]` / `# ===== HTTP 引擎配置 =====`+`# [http]` / `# ===== BT 引擎配置 =====`+`# [bt]`（v1.12：分隔头注释不含表名、表声明行独立成行）、36 键三行式**首行注释无键名前缀**（v1.12：如 `# 上传 choker 重排周期（秒）`，非 `# rechoke_interval_secs：…`）、`# 命名代理列表（可配置多个）` 一句头注释 + 双示例块 2 个 `# [[common.proxy]]`（各含 name/type/ip/port/username/password 6 键行）、空行位置与行序、EOF 单换行收尾全部一致）；文件内每条非空行以 `#` 开头；36 键示例值与代码常量默认逐一相符（http/bt 调优键含 progress_tick_ms=100、sidecar_flush_secs=2、throttle_burst_secs=0.5、throttle_sleep_ms=20、dns_race_grace_ms=1000、bt_concurrency=20、peer_pipeline=64、upload_slots=4、upload_read_concurrency=128、idle_eviction_secs=60（操作者已批）、connect_timeout_secs=3、handshake_timeout_secs=10、request_timeout_secs=20、requests_per_piece=4、rechoke_interval_secs=10、optimistic_interval_secs=30、progress_emit_min_interval_ms=250、tracker_numwant=200、peer_retry_base_secs=5、peer_retry_cap_secs=120、tracker_backoff_start_secs=8、tracker_backoff_cap_secs=60、udp_base_timeout_secs=15、udp_retries=3、seed_duration_secs=1800）；无凭证规则注释行（v1.10 最小注释集口径延续）；启动行为与缺失文件一致（并发 4/槽位 5/块 1 MB/重试 5/auto_retry 开/不限速/默认目录=用户下载目录） |
| QA-CT-02 | 01-config-template-02 | 预置 `[common]` 表 + `download_slots = 2` 的合法 config.toml → 记录字节与 mtime → 启动 ezr 添加 3 任务 | 文件字节级不变（cmp 一致）；槽位按 2 生效（队列栏「下载槽位 n/2」，第 3 个任务排队） |
| QA-CT-03 | 01-config-template-03 | 预置损坏 config.toml（非法 TOML）→ 记录字节 → 启动 ezr 正常下载 | 启动不报错；文件字节级不变；行为全默认（块 1 MB、槽位 5） |
| QA-CT-04 | 01-config-template-04 | 干净 HOME 预建 `~/.ezr` 目录并 `chmod 555` → 启动 ezr | 启动正常不报错；无 config.toml 生成（或生成失败不留半截文件）；行为全默认；退出后 `chmod 755` 清理 |
| QA-CT-05 | 01-config-template-05 | `EZR_HOME=$X`（独立目录）启动 ezr | `$X/config.toml` 已生成且与钦定文本逐字节一致；`$HOME/.ezr/` 下无 config.toml |

## 通过准则

- 全部用例 P/F/S；S 内嵌环境判据（如只读目录在特定沙箱不可模拟时登记 S 并说明受限原因）。
- QA-CT-01/05 的模板内容断言与钦定文本整文本逐字节对账（`cmp` 口径，v1.10 起模板逐字节
  锁定，含空行/行序/EOF；v1.12 文本整体替换）；模板行为等价性
  （QA-CT-01/03/04）与 `qa/01-persistence-config-qa.md` QA-PC-03 默认值口径一致。

## 套件脚本

- 可执行套件：`project/qa/runners/suite_config_template.py`（QA 会话落，端到端 UI 层；`python3 suite_config_template.py` 于 runners 目录执行，依赖 `harness.py` 共享基建与 pyte/wcwidth）。
- **v1.12 同步待办（coder/QA 会话执行）**：`suite_config_template.py` 的钦定原文解析断言
  `len(body) == 168` → `174`（块起点锚 `specifier v1.11）：` → `specifier v1.12）：`）；
  三表分隔头断言 `# ===== [common]`/`# ===== [http]`/`# ===== [bt]` → 两行式形态
  （`# ===== 公共配置 =====` + `# [common]` / `# ===== HTTP 引擎配置 =====` + `# [http]` /
  `# ===== BT 引擎配置 =====` + `# [bt]`，并断言无旧形态 `# ===== [common]`）；
  增断言无键名前缀（无 `# <key>：` 形态行）；36 键示例行断言不变（`# <key> = <值>` 行未改）。
