# QA 套件 · 01-persistence-config 持久化配置与生命周期（期号 01）

> 对应规格：`project/features/01-persistence-config.feature`（9 场景）。
> 端到端 UI 层验证：CLI 启动/退出操作 + `~/.ezr` 磁盘产物观察 + TUI 状态断言 + 双实例进程操作。

## 环境前置

- fixture 服务器（慢速门控，保证退出/重启窗口内任务处于确定状态）。
- 伪终端驱动（终端恢复态断言：光标可见、无残留字符/花屏）；独立 HOME；进程管理工具（pgrep/pkill、kill -9）。
- config.toml 写入模板（各键合法/非法样本值）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-PC-01 | 01-persistence-config-01 | 预置各状态任务→优雅退出→重启 | 任务/历史/顺序完整还原；续传点与状态一致 |
| QA-PC-02 | 01-persistence-config-02 | 添加任务后检查主目录 | 存在 `.ezr/state/`（注册表）；`.ezr/config.toml` 缺失或合法；sidecar 随目标文件 |
| QA-PC-03 | 01-persistence-config-03 | 删除 config.toml 后下载 | 默认并发 4/槽位 5/块 1MB/重试 5/auto_retry 开/不限速/download_dir=用户主目录下载目录（Linux ~/Downloads） |
| QA-PC-04 | 01-persistence-config-04 | 分别写入非法值：block_size_http=abc、download_slots=0、max_retries=-1、default_concurrency=99、max_speed=xyz | 启动不报错；各按默认 1MB/5/5/4/0 生效 |
| QA-PC-05 | 01-persistence-config-05 | block_size_http=256KB / 2MB 下载 5MB | 分块行 `x/20 · 256 KB/块` / `x/3 · 2 MB/块` |
| QA-PC-06 | 01-persistence-config-06 | download_slots=2 后添加 4 任务 | 前 2 下载；队列栏「下载槽位 n/2」；其余排队 |
| QA-PC-07 | 01-persistence-config-07 | 运行中二次启动 ezr | 第二实例提示「ezr 已在运行」非零退出；首实例正常；全程单实例（AC-11） |
| QA-PC-08 | 01-persistence-config-08 | 下载至 30% 按 Q→重启 | 断点保留（sidecar 完整）；终端恢复；重启续传完成（AC-9） |
| QA-PC-09 | 01-persistence-config-09 | 下载中分别以 Esc / Ctrl+C 退出 | 语义同 Q（停传、保断点、存注册表）；终端无花屏（AC-9） |

## 通过准则

- 全部用例 P/F/S；S 内嵌环境判据。QA-PC-07 对应 AC-11、QA-PC-08/09 对应 AC-9。
