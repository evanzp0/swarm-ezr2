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
> v1.13（操作者 bug14 轮指令，D63；FR-01-85 v1.13/FR-01-71 v1.39）：钦定文本整体替换为
> v1.13 版（174 行，行集结构同 v1.12、仅 36 键值行与 3 表声明行取消注释成为活跃 TOML）；
> QA-CT-01 判据同步（活跃键值行 + 注释行保持 + 解析等价断言）；QA-CT-03 行为翻转
> （损坏配置 → 拒绝启动直接报错，FR-01-71 v1.39）。
> v1.12（操作者 bug12 轮指令，模板注释格式修正）：钦定文本整体替换为 v1.12 版
> （174 行 = 标题行 + 3 组「分隔头注释 + 空行 + 表声明行」+ 36 键×3 行三行式（首行注释
> 无键名前缀）+ [[common.proxy]] 一句头注释 + 双示例块 + 44 空行，EOF 单换行收尾）；
> QA-CT-01 判据同步（两行式表头、无键名前缀、174 行）。

## 环境前置

- fixture 服务器（默认行为断言用简单下载即可）。
- 独立 HOME（每用例独立沙箱目录）；进程管理工具（pgrep/pkill）。
- `chmod`（目录只读场景）；字节级文件对比（`sha256sum`/`cmp`）。
- 模板内容契约基线（v1.13）：**操作者钦定模板文本整文本**（174 行，权威原文
  见规格 `01-config-template.feature` 内嵌原文，QA 会话脚本化时将其落盘为基线文件供
  `cmp` 逐字节比对）；另备 TOML 解析器（python3 tomllib）作活跃键值解析等价断言用。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-CT-01 | 01-config-template-01 | 干净 HOME 启动 ezr（无 config.toml）后退出 | `<HOME>/.ezr/config.toml` 已生成；文件内容与钦定文本**逐字节一致**（整文本 `cmp` 比对，v1.13）；**v1.13 活跃模板判据**：3 个表声明行 `[common]`/`[http]`/`[bt]` 与 36 个键值行为活跃 TOML（无 `#` 前缀），注释行（用途/取值范围注释、3 条 `# ===== … =====` 分隔头、`# 命名代理列表（可配置多个）` 一句头、双 `# [[common.proxy]]` 示例块）保持 `#` 注释；`tomllib` 解析生成的 config.toml 成功且解析值 = 代码常量默认逐键一致（download_slots=5、bt_concurrency=20、http_concurrency=4、block_size_http=1048576、idle_eviction_secs=60 等全 36 键）⇒ 解析结果与文件缺失一致；proxy 表不存在（示例块未激活，D63②）；启动行为与缺失文件一致（并发 4/槽位 5/块 1 MB/重试 5/auto_retry 开/不限速/默认目录=用户下载目录） |
| QA-CT-02 | 01-config-template-02 | 预置 `[common]` 表 + `download_slots = 2` 的合法 config.toml → 记录字节与 mtime → 启动 ezr 添加 3 任务 | 文件字节级不变（cmp 一致）；槽位按 2 生效（队列栏「下载槽位 n/2」，第 3 个任务排队） |
| QA-CT-03 | 01-config-template-03 | 预置损坏 config.toml（非法 TOML）→ 记录字节 → 启动 ezr（v1.13 行为翻转，FR-01-71 v1.39/D63①） | **ezr 拒绝启动**：进程以非零退出码退出、TUI 不进入；报错输出含配置文件路径与解析失败原因（含 TOML 错误位置/描述）；文件字节级不变（cmp 一致，不被重写不被模板覆盖）；将文件修复为合法 TOML 后再次启动正常进入 |
| QA-CT-04 | 01-config-template-04 | 干净 HOME 预建 `~/.ezr` 目录并 `chmod 555` → 启动 ezr | 启动正常不报错；无 config.toml 生成（或生成失败不留半截文件）；行为全默认；退出后 `chmod 755` 清理 |
| QA-CT-05 | 01-config-template-05 | `EZR_HOME=$X`（独立目录）启动 ezr | `$X/config.toml` 已生成且与钦定文本逐字节一致；`$HOME/.ezr/` 下无 config.toml |

## 通过准则

- 全部用例 P/F/S；S 内嵌环境判据（如只读目录在特定沙箱不可模拟时登记 S 并说明受限原因）。
- QA-CT-01/05 的模板内容断言与钦定文本整文本逐字节对账（`cmp` 口径，v1.10 起模板逐字节
  锁定，含空行/行序/EOF；v1.13 文本整体替换——活跃键值行 + 注释行保持）；模板行为等价性
  （QA-CT-01/04）与 `qa/01-persistence-config-qa.md` QA-PC-03 默认值口径一致；QA-CT-03
  为 v1.13 翻转项（拒绝启动），与 QA-PC 系列损坏语义不再相同（PC 侧无损坏场景，无连带）。

## 套件脚本

- 可执行套件：`project/qa/runners/suite_config_template.py`（QA 会话落，端到端 UI 层；`python3 suite_config_template.py` 于 runners 目录执行，依赖 `harness.py` 共享基建与 pyte/wcwidth）。
- **v1.13 同步待办（coder/QA 会话执行）**：`suite_config_template.py` 的钦定原文解析断言
  块起点锚 `specifier v1.12）：` → `specifier v1.13）：`（`len(body) == 174` 不变）；
  表声明行断言 `# [common]`/`# [http]`/`# [bt]` → 活跃形态 `[common]`/`[http]`/`[bt]`；
  36 键示例行断言 `# <key> = <值>` → 活跃 `<key> = <值>`；「文件内每条非空行均以 # 开头」
  类全注释断言移除，改为：注释行保持 `#`（用途/范围/分隔头/proxy 示例块）+ 活跃键集 =
  36 键白名单 + `tomllib` 解析等价断言（解析值 = 代码默认逐键）；QA-CT-03 用例语义翻转
  （启动期望非零退出码 + 报错文案断言），runner 驱动方式从「启动并 pump 画面」改为
  「启动收退出码与 stderr 后收尸」。
