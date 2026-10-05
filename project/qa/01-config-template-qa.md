# QA 套件 · 01-config-template 配置模板自动生成（期号 01 · v1.4/FR-01-85）

> 对应规格：`project/features/01-config-template.feature`（5 场景）。
> 端到端 UI 层验证：CLI 启动/退出操作 + 磁盘产物观察（文件存在性/全注释/字节级对比）+
> TUI 默认值行为断言；不使用项目内部 API。

## 环境前置

- fixture 服务器（默认行为断言用简单下载即可）。
- 独立 HOME（每用例独立沙箱目录）；进程管理工具（pgrep/pkill）。
- `chmod`（目录只读场景）；字节级文件对比（`sha256sum`/`cmp`）。
- 模板内容契约基线：FR-01-71 全部 10 键及其默认值行（见规格 Examples 表）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-CT-01 | 01-config-template-01 | 干净 HOME 启动 ezr（无 config.toml）后退出 | `<HOME>/.ezr/config.toml` 已生成；文件内每条非空行以 `#` 开头；10 键逐一在位且为注释形态 `# <键> = <默认值>`；各键注释含用途与取值范围说明；启动行为与缺失文件一致（并发 4/槽位 5/块 1 MB/重试 5/auto_retry 开/不限速/默认目录=用户下载目录） |
| QA-CT-02 | 01-config-template-02 | 预置 `download_slots = 2` 的合法 config.toml → 记录字节与 mtime → 启动 ezr 添加 3 任务 | 文件字节级不变（cmp 一致）；槽位按 2 生效（队列栏「下载槽位 n/2」，第 3 个任务排队） |
| QA-CT-03 | 01-config-template-03 | 预置损坏 config.toml（非法 TOML）→ 记录字节 → 启动 ezr 正常下载 | 启动不报错；文件字节级不变；行为全默认（块 1 MB、槽位 5） |
| QA-CT-04 | 01-config-template-04 | 干净 HOME 预建 `~/.ezr` 目录并 `chmod 555` → 启动 ezr | 启动正常不报错；无 config.toml 生成（或生成失败不留半截文件）；行为全默认；退出后 `chmod 755` 清理 |
| QA-CT-05 | 01-config-template-05 | `EZR_HOME=$X`（独立目录）启动 ezr | `$X/config.toml` 已生成且为全注释默认模板；`$HOME/.ezr/` 下无 config.toml |

## 通过准则

- 全部用例 P/F/S；S 内嵌环境判据（如只读目录在特定沙箱不可模拟时登记 S 并说明受限原因）。
- QA-CT-01 的模板内容断言与规格 Examples 表逐键对账（10/10 键）；模板行为等价性
  （QA-CT-01/03/04）与 `qa/01-persistence-config-qa.md` QA-PC-03 默认值口径一致。
