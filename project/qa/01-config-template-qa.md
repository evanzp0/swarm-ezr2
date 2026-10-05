# QA 套件 · 01-config-template 配置模板自动生成（期号 01 · v1.4/FR-01-85）

> 对应规格：`project/features/01-config-template.feature`（5 场景）。
> 端到端 UI 层验证：CLI 启动/退出操作 + 磁盘产物观察（文件存在性/全注释/字节级对比）+
> TUI 默认值行为断言；不使用项目内部 API。
> v1.10（操作者第七批指令，模板最小化）：模板内容契约基线改为**操作者钦定文本整文本**
> （逐字节权威，规格内嵌原文行集）；QA-CT-01 判据同步为整文本逐字节比对 + proxies 段
> 最小形态断言（一句头注释 + 双示例块）；凭证规则注释行相关断言随 FR-01-93⑤ 废止删除
> （凭证运行期语义不变，经 02-named-proxy 套件覆盖）。

## 环境前置

- fixture 服务器（默认行为断言用简单下载即可）。
- 独立 HOME（每用例独立沙箱目录）；进程管理工具（pgrep/pkill）。
- `chmod`（目录只读场景）；字节级文件对比（`sha256sum`/`cmp`）。
- 模板内容契约基线（v1.10）：**操作者钦定模板文本整文本**（54 行 = 标题行 + 9 键×3 行
  三行式 + proxies 一句头注释 + 双示例块各 7 行 + 11 个空行，EOF 单换行收尾；权威原文
  见规格 `01-config-template.feature` 内嵌原文，QA 会话脚本化时将其落盘为基线文件供
  `cmp` 逐字节比对）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-CT-01 | 01-config-template-01 | 干净 HOME 启动 ezr（无 config.toml）后退出 | `<HOME>/.ezr/config.toml` 已生成；文件内容与钦定文本**逐字节一致**（整文本 `cmp` 比对：标题行、9 键三行式、proxies 一句头注释 `# proxies：命名代理列表（可配置多个）` + 双示例块 2 个 `# [[proxies]]`（各含 name/type/ip/port/username/password 6 键行）、空行位置与行序、EOF 单换行收尾全部一致）；文件内每条非空行以 `#` 开头；9 键示例值与默认值口径逐一相符（http_concurrency=4/槽位 5/块 1048576/重试 5/auto_retry 开/max_speed 0/退避 8.0/60.0）；无凭证规则注释行、无三类型语义/IPv6/type-url 语义等解释性注释行（v1.10 最小注释集，FR-01-93⑤ 废止承载）；启动行为与缺失文件一致（并发 4/槽位 5/块 1 MB/重试 5/auto_retry 开/不限速/默认目录=用户下载目录） |
| QA-CT-02 | 01-config-template-02 | 预置 `download_slots = 2` 的合法 config.toml → 记录字节与 mtime → 启动 ezr 添加 3 任务 | 文件字节级不变（cmp 一致）；槽位按 2 生效（队列栏「下载槽位 n/2」，第 3 个任务排队） |
| QA-CT-03 | 01-config-template-03 | 预置损坏 config.toml（非法 TOML）→ 记录字节 → 启动 ezr 正常下载 | 启动不报错；文件字节级不变；行为全默认（块 1 MB、槽位 5） |
| QA-CT-04 | 01-config-template-04 | 干净 HOME 预建 `~/.ezr` 目录并 `chmod 555` → 启动 ezr | 启动正常不报错；无 config.toml 生成（或生成失败不留半截文件）；行为全默认；退出后 `chmod 755` 清理 |
| QA-CT-05 | 01-config-template-05 | `EZR_HOME=$X`（独立目录）启动 ezr | `$X/config.toml` 已生成且与钦定文本逐字节一致；`$HOME/.ezr/` 下无 config.toml |

## 通过准则

- 全部用例 P/F/S；S 内嵌环境判据（如只读目录在特定沙箱不可模拟时登记 S 并说明受限原因）。
- QA-CT-01/05 的模板内容断言与钦定文本整文本逐字节对账（`cmp` 口径，不再按逐行 contains
  宽容比对——v1.10 起模板逐字节锁定，含空行/行序/EOF）；模板行为等价性
  （QA-CT-01/03/04）与 `qa/01-persistence-config-qa.md` QA-PC-03 默认值口径一致。
