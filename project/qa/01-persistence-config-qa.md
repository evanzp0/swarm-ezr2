# QA 套件 · 01-persistence-config 持久化配置与生命周期（期号 01）

> 对应规格：`project/features/01-persistence-config.feature`（12 场景）。
> 端到端 UI 层验证：CLI 启动/退出操作 + `~/.ezr` 磁盘产物观察 + TUI 状态断言 + 双实例进程操作。
> v1.17（操作者 bug11 轮指令，config 三大标准表重构）：配置键按「<表>.<键名>」点路径书写——
> QA 写入 = 对应表头（`[common]` / `[http]` / `[bt]`）+ 键值行；QA-PC-04 扩充新增调优键
> 非法值回退行（[http] 5 键 + [bt] 20 键代表性样例，统一回退口径 = 回退代码常量默认）；
> 新增 QA-PC-12（旧格式顶层键与旧 [[proxies]] 表静默忽略）。

## 环境前置

- fixture 服务器（慢速门控，保证退出/重启窗口内任务处于确定状态）。
- 伪终端驱动（终端恢复态断言：光标可见、无残留字符/花屏）；独立 HOME；进程管理工具（pgrep/pkill、kill -9）。
- config.toml 写入模板（三表表头 + 各键合法/非法样本值）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-PC-01 | 01-persistence-config-01 | 预置各状态任务→优雅退出→重启 | 任务/历史/顺序完整还原；续传点与状态一致 |
| QA-PC-02 | 01-persistence-config-02 | 添加任务后检查主目录 | 存在 `.ezr/state/`（注册表）；`.ezr/config.toml` 存在（v1.4/FR-01-85：缺失时启动自动生成全注释默认模板，细则见 `qa/01-config-template-qa.md`）；sidecar 随目标文件 |
| QA-PC-03 | 01-persistence-config-03 | 删除 config.toml 后下载 | 默认并发 4/槽位 5/块 1MB/重试 5/auto_retry 开/不限速/download_dir=用户主目录下载目录（Linux ~/Downloads）；BT 侧默认并发 20/做种 1800s（v1.17 口径注记） |
| QA-PC-04 | 01-persistence-config-04 | 分别写入非法值（v1.17 点路径）：`[http] block_size_http=abc`、`[http] block_size_http=256KB`（v1.16 钳制行）、`[common] download_slots=0`、`[http] max_retries=-1`、`[http] http_concurrency=99`、`[common] max_speed=xyz` + 新增调优键样例：`[http] progress_tick_ms=0`、`[http] sidecar_flush_secs=0`、`[http] throttle_burst_secs=0`、`[http] throttle_sleep_ms=0`、`[http] dns_race_grace_ms=0`、`[bt] bt_concurrency=0`、`[bt] bt_concurrency=99`、`[bt] peer_pipeline=0`、`[bt] upload_slots=0`、`[bt] upload_read_concurrency=0`、`[bt] idle_eviction_secs=-1`、`[bt] connect_timeout_secs=0`、`[bt] handshake_timeout_secs=0`、`[bt] request_timeout_secs=0`、`[bt] requests_per_piece=0`、`[bt] rechoke_interval_secs=0`、`[bt] optimistic_interval_secs=0`、`[bt] progress_emit_min_interval_ms=0`、`[bt] tracker_numwant=0`、`[bt] peer_retry_base_secs=0`、`[bt] peer_retry_cap_secs=0`、`[bt] tracker_backoff_start_secs=0`、`[bt] tracker_backoff_cap_secs=0`、`[bt] udp_base_timeout_secs=0`、`[bt] udp_retries=0`、`[bt] seed_duration_secs=0`、`[bt] udp_trackers=["ftp://x/ann"]` | 启动不报错；各按回退/钳制值 1MB/1MB（FR-01-104 下限）/5/5/64/0 + 100/2/0.5/20/1000/20/64（bt_concurrency=99 钳制上限）/64/4/128/60/3/10/20/4/10/30/250/200/5/120/8/60/15/3/1800 生效；`udp_trackers` 非法 URL 忽略 + 启动警告一次（v1.17 统一回退口径 = 代码常量默认，FR-01-71） |
| QA-PC-05 | 01-persistence-config-05 | `[http] block_size_http=4MB / 2MB` 下载 5MB（v1.16：原 256KB 行因低于 1 MB 下限改判 4MB） | 分块行 `x/2 · 4 MB/块` / `x/3 · 2 MB/块` |
| QA-PC-06 | 01-persistence-config-06 | `[common] download_slots=2` 后添加 4 任务 | 前 2 下载；队列栏「下载槽位 n/2」；其余排队 |
| QA-PC-07 | 01-persistence-config-07 | 运行中二次启动 ezr | 第二实例提示「ezr 已在运行」非零退出；首实例正常；全程单实例（AC-11） |
| QA-PC-08 | 01-persistence-config-08 | 下载至 30% 按 Q→重启 | 断点保留（sidecar 完整）；终端恢复；重启续传完成（AC-9） |
| QA-PC-09 | 01-persistence-config-09 | 下载中在主界面按 Esc；再以 Ctrl+C 退出 | Esc 主界面无任何动作（应用不退出、任务继续下载——退出仅 Q/Ctrl+C，01-tui-display-07 口径，操作者 20261027 裁决定稿）；Ctrl+C 优雅退出（语义同 Q：停传、保断点、存注册表；收尾亦可以 Q 优雅退出复验同口径）；终端无花屏（AC-9）。场景 09 文本已按裁决同步修订 |
| QA-PC-12 | 01-persistence-config-12 | 预置旧格式 config.toml（顶层平铺 `download_slots=2`、`block_size_http=4194304`、`http_concurrency=8`、`max_speed="1 MB/s"` + 顶层 `[[proxies]]` 合法条目，无任何新表头）→ 启动、添加任务、打开添加对话框代理下拉；随后同文件追加 `[common]` 表头与 `download_slots = 3` 再启动 | 启动不报错、无忽略类 toast/警告（零警告零迁移）；全部旧键忽略、行为全默认（槽位 5/块 1 MB/并发默认 4/不限速）；旧 [[proxies]] 条目不出现在代理下拉；追加新表头后 `[common] download_slots=3` 正常生效（新格式不受旧键干扰）（v1.17，操作者钦定不兼容旧格式） |

## 通过准则

- 全部用例 P/F/S；S 内嵌环境判据。QA-PC-07 对应 AC-11、QA-PC-08/09 对应 AC-9。
- QA-PC-04 的调优键「生效」断言以可观察面为准：`udp_trackers` 非法 URL 警告 + 数值键经
  09-bt-engine-14 引擎行为与单元级回退断言承载（套件内不逐键构造端到端可观察差异）。

## 套件脚本

- 可执行套件：`project/qa/runners/suite_persistence_config.py`（QA 会话落）。
- **v1.17 同步待办（coder/QA 会话执行）**：本套件 `env.write_config` 各处顶层键改三表形态
  （`[common] download_slots` / `[http] block_size_http` 等，约 11 处）；`suite_slots_queue.py`
  （9 处 download_slots → `[common]`）、`suite_throttle_proxy.py`（8 处 max_speed → `[common]`）、
  `suite_tui_display.py`（2 处 block_size_http → `[http]`）、`suite_ui_conns.py`（3 处 →
  `[common]`/`[http]`）、`suite_resume_sidecar.py`（1 处）、`suite_integrity_check.py`（1 处）、
  `suite_retry_backoff.py`（2 处 → `[http]`）、`suite_add_task.py`（1 处 download_dir →
  `[common]`）、`suite_modify_task.py`（2 处）、`suite_named_proxy.py`（`proxies_toml` 生成器
  `[[proxies]]` → `[[common.proxy]]`，15 处调用随之）、`suite_config_template.py`（见该套件
  v1.11 待办）。
