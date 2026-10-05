# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier　会话：specifier-20261005-v110（操作者第七批指令建册：配置模板最小化——操作者钦定文本成为 `default_template()` 输出的逐字节唯一权威）

## 一、产物历史完成情况

### 项目与需求
- EZR Downloader（ezr download）：TUI 高性能多协议下载器，phase-01（HTTP/HTTPS 真实下载
  内核与 TUI 正式版）已交付并通过 QA 终局独立验证；QA 后增量轮：coder 裁决落地轮
  （on_evt 单测 24 条 / 锁回退收口）、specifier v1.4 + coder v14（FR-01-85 配置模板）、
  coder r17（D17 裁决落账）、specifier v1.5（FR-01-86/87/88 建档）+ coder v1.5（实现）、
  specifier v1.6（第三批指令：旧 proxy 键退役/代理 type/UI 文案清理/失败任务空格暂停，
  FR-01-89..92 建档）+ coder v1.6（实现）、specifier v1.7（D24 改判：type 两值合并/
  矛盾不作废 + D20 维持落账，AIR2/aria2 实证）+ coder v1.7（D24 改判实现）、
  specifier v1.8 + coder v1.8（FR-01-93 代理凭证按类型收紧 + socks5 userinfo 认证修复）、
  specifier v1.9 + coder v1.9（操作者第六批指令：`[[proxies]]` 条目字段重构——ip/port/
  type/username/password，type 必填三值且为唯一事实来源，url 键退役），
  本轮 specifier v1.10（操作者第七批指令：配置模板最小化——钦定文本逐字节权威，
  9 键三行式保留、proxies 段收敛、解释性注释全部退出模板）。
  需求权威来源 `project/mission.md`（v1.10），phase-01 详述见 `project/mission/phase-01.md`（v1.10），
  feature 规格在 `project/features/`（12 份），QA 规程在 `project/qa/`（12 份规程 / 9 份脚本）。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier（基线）+ coder（phase-01） | mission/features/qa 9 份 + 下载内核 + TUI + fixtures | 已交付 | 上游既定口径 |
| cleaner / architect / hardender / QA（历轮） | 加固 ~110 测试、度量收敛、架构复核、PTY e2e 终局验证 | 已交付 | QA 终局 966P/0F；行覆盖 90.5% |
| coder（20261005 裁决落地轮） | ①CRAP 登记 ②on_evt arm 单测 24 条 ③锁回退分支收口 | 已交付 | 1063P/0F |
| specifier（v1.4）/ coder（v14）/ coder（r17） | FR-01-85 配置模板（建档/实现/裁决落账） | 已交付 | 1105P/0F |
| specifier（v1.5）/ coder（v1.5） | FR-01-86/87/88：命名代理任务级选择 + client 缓存池 + Reconfigure + m 修改对话框 + http_concurrency 改名 | 已交付 | 1152P/0F |
| specifier（v1.6）/ coder（v1.6） | FR-01-89..92：旧 proxy 键退役 + type 键（D24 待批）+ UI 文案清理 + FailedPaused | 已交付 | 1206P/0F（v1.7 去重后真实计数 1205） |
| specifier（v1.7）/ coder（v1.7） | D24 改判落地：type 两值、url scheme 唯一事实来源、矛盾/未知保留按 url 处理 + 警告；D20 维持落账 | 已交付 | 1205P/0F（10 二进制）；clippy 0；fmt 0；arch_check 通过 |
| specifier（v1.8）/ coder（v1.8） | FR-01-93 代理凭证按类型（socks5 必填/http 可选）+ socks5 认证经 url userinfo RFC 1929（修复 v1.5 起静默失效缺陷）+ 模板凭证契约 | 已交付 | 1276P/0F（10 二进制）；clippy 0；fmt 0；arch_check 10/10 |
| specifier（v1.9）/ coder（v1.9） | 操作者第六批指令全链：`[[proxies]]` 字段重构 name/type/ip/port/username/password，type 必填三值唯一事实来源（url 键退役按未知键忽略零迁移）；内部 url = {type}://{ip}:{port}（IPv6 方括号）；非法清单更新；UI「名（type）」三值标注 | 已交付 | **1305P/0F（10 二进制 297/30/11/106/235/234/2/12/212/166）**；clippy 0；fmt 0；arch_check 全通过 |
| **specifier（20261005 v1.10）** | 操作者第七批指令建档：**配置模板最小化**——操作者钦定模板文本成为 `default_template()` 输出的唯一权威（逐字节一致，54 行：标题行 + 9 键×3 行三行式 + proxies 一句头注释 + 双示例块各 7 行 + 11 空行，EOF 单换行收尾）；9 键「用途 + 取值范围 + 示例行」三行式保留（FR-01-85 双注释契约对 9 键仍成立）；proxies 段豁免解释性注释——三类型语义、凭证规则、IPv6、type/url 语义说明全部不进模板（语义由运行期校验警告与 README/规格承载）；FR-01-93⑤ 凭证注释行条款废止（运行期凭证语义不变）；v1.6「空格/顺序可调」口径废止；钦定文本示例值与 DEFAULT_* 常量比对 **9/9 一致（无冲突）**。phase-01 v1.10 + mission.md v1.10 + 01-config-template.feature 重写（钦定原文内嵌）+ qa/01-config-template-qa.md 整文本比对口径 | 已交付 | APS parser 12 份全通过（01-config-template IR=5 景、Examples 9 组；02-named-proxy IR=18 景阳性对照） |

### 遗留受限项（当前有效）
- **CRAP 为受限近似口径**（继承上游；操作者已裁决维持现状）：无标准 CRAP 工具；新增代码为
  纯函数 + 单点接线风格，未复算。
- **e2e 套件预登记 S 项（3 用例）**：RB-09；TP-02/03/06（既有口径不变）。
- **沙箱无 TTY**：TUI 进程级端到端不可自动化；pty E2E 由 QA 会话以 pyte 口径完成。
- **QA 规程就绪、套件脚本未落**：QA-CT-01..05、QA-NP-01..18、QA-MT-01..08 共三套
  （12 份规程 / 9 份脚本，v1.6 起 suite_throttle_proxy.py 收敛为 7 限速用例）；
  脚本化按 six-pack 编排属 QA 角色会话产物。v1.10 起 QA-CT-01/05 的模板内容断言改为
  **与钦定文本整文本 `cmp` 逐字节比对**（基线文件从规格内嵌原文落盘）。v1.9 起 QA-NP
  另需 ezr-proxy 扩展 **https 代理监听**（NP-08 p-https）、**socks5+user/pass 认证监听**
  （NP-15）、**IPv6 回环监听**（NP-18）三种形态（均沿 ezr-proxy 扩展遗留受限项口径，
  未就绪登记 S；NP-11/12/16/17 的作废条目用例无连接尝试，不依赖扩展）。

### 实现定义值登记
- 无新增产品契约值（v1.10 钦定文本本身即操作者权威文本，示例值与代码常量 9/9 一致——
  见下节比对表；模板警告/凭证运行期文案沿 v1.9 口径不变）。

## 二、当前产出情况（specifier-20261005-v110）

### 本会话产物清单（纯规格轮，零代码改动，指令直落不触发 Inversion）
- **`mission/phase-01.md` v1.10**：头部第七批指令修订注记；FR-01-85 增 v1.10 修订
  （钦定文本唯一权威/逐字节/EOF 单换行、54 行结构、最小注释集、9 键三行式契约保留、
  proxies 段豁免解释性注释、v1.6「空格/顺序可调」口径废止、值比对 9/9 登记）；
  FR-01-93⑤ 废止改写（模板凭证规则注释行废止，①②③④ 运行期语义照旧）；FR-01-89
  模板句同步（type 行仍在双示例块内，语义注释退出模板）。
- **`mission.md` v1.10**：头部 v1.10 注记（一行变更摘要，操作者第七批指令）。
- **`features/01-config-template.feature`**：Feature 头补 v1.10 修订段；场景 01 新增
  「模板内容与操作者钦定文本逐字节一致」步 + **钦定原文 54 行逐字节内嵌**（权威行集，
  coder 不得增删改任何字符/空行；空行即真实空行）；Examples 表 9 键的 purpose/range
  要点全部改为钦定文本的子串（含用途与取值范围要点对齐）；旧 proxies 段解释性注释
  期望行（字段说明/三类型语义/IPv6/凭证规则/示例块凭证语义注记）全部删除，替换为
  一句头注释 + 双示例块钦定行集；场景 02–05 零改动（存在不覆写/损坏不重写/生成失败
  静默/EZR_HOME 重定位语义不变）。
- **`qa/01-config-template-qa.md`**：头部 v1.10 注记；环境前置基线改「操作者钦定模板
  文本整文本」（54 行，QA 脚本化时落盘为基线文件供 `cmp`）；QA-CT-01 判据改整文本
  逐字节比对 + proxies 段最小形态断言（一句头注释 `# proxies：命名代理列表（可配置
  多个）` + 双示例块 2 个 `# [[proxies]]`，无凭证规则/三类型语义/IPv6 等解释性注释行）；
  QA-CT-05 同步整文本口径；通过准则改「与钦定文本整文本逐字节对账（cmp 口径，
  不再逐行 contains 宽容比对）」。
- **`packs/_common/engineering.md`**：经验沉淀——「生成默认文件」三条纪律增补第 ④ 点：
  操作者钦定权威文本时改用整文本逐字节锁定（整文本比对而非逐行 contains 的理由）+
  钦定值 ↔ 产品常量逐项比对要在规格层完成（两处事实来源漂移的调和）。

### 值一致性比对结果（钦定文本示例值 ↔ 代码常量，specifier 现场核对——重点登记）
结论：**9/9 一致，无冲突**。无需「示例值以钦定文本为准」标注；无需任何常量对齐任务。

| 键 | 钦定文本示例值 | 代码现状（v1.9 实现） | 比对 |
|---|---|---|---|
| download_dir | `""` | `Config::default().download_dir = None`（≡ 留空/缺失语义） | 一致 |
| block_size_http | `1048576` | `DEFAULT_BLOCK_SIZE_HTTP` = `chunk::HTTP_CHUNK_SIZE` = 1024×1024 | 一致 |
| download_slots | `5` | `DEFAULT_DOWNLOAD_SLOTS = 5` | 一致 |
| max_speed | `0` | `Config::default().max_speed = 0` | 一致 |
| max_retries | `5` | `DEFAULT_MAX_RETRIES = 5` | 一致 |
| auto_retry | `true` | `Config::default().auto_retry = true` | 一致 |
| backoff_initial | `8.0` | `Config::default().backoff_initial = 8.0` | 一致 |
| backoff_cap | `60.0` | `Config::default().backoff_cap = 60.0` | 一致 |
| http_concurrency | `4` | `DEFAULT_HTTP_CONCURRENCY = 4` | 一致 |

（代码位置 `project/ezr/src/model/config.rs`：常量 L9–15、`impl Default` L155–171、
`default_template()` L437–514。后续任何 DEFAULT_* 常量变更须回到操作者钦定文本对齐，
不得单方改模板或单方改常量——见 phase-01 FR-01-85 v1.10 修订句。）

### 解释口径（操作者指令的解释与落地，均按指令直接落地，非待批项）
- **钦定文本 = 唯一权威**：`default_template()` 输出与钦定原文逐字节一致（含空行、行序、
  EOF 单换行收尾）；模板内容不再是「从 Config::default() 拼装」的派生物——钦定文本为
  第一事实来源，既有「往返契约测试」（模板解析 = 全默认）保留作为行为护栏，模板内容
  契约改为整文本精确比对（engineering.md 第 ④ 条纪律）。
- **最小注释集的边界**：9 键的「用途 + 取值范围」双注释契约保留；proxies 段豁免解释性
  注释——仅一句头注释 + 双示例块（示例值 = 办公网代理 http 与本地 SOCKS5，与既有
  示例同源）。凭证规则/三类型语义/IPv6/type-url 语义由运行期校验警告（FR-01-86/89/93
  既有链）与 README/规格承载，运行期行为零变化。
- **v1.6「空格/顺序可调」口径废止**：v1.10 逐字节锁定取代之（空行、行序、EOF 收尾均
  在契约内）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| Gherkin 规格 | 12 份全通过（01-config-template IR = 5 场景、Examples 9 组解析正确；02-named-proxy IR = 18 场景阳性对照，本轮未动） | `(cd .tools/Acceptance-Pipeline-Specification && bb gherkin-parser <feature> <out>.json)`，输出在 `.work/tmp/aps-v110/*.json` |
| 钦定文本逐字节自查 | feature 内嵌原文 ≡ 钦定文本（54 行，diff 为空，EOF 单换行 `od -c` 实证）；Examples 表 9 键 purpose/range 要点全部为钦定文本子串（python `in` 精确比对） | `.work/tmp/authorized-template-v110.txt`（钦定基准）vs `.work/tmp/embedded-extract.txt`（从 feature 反提取）`diff` |
| 口径自查 | FR-01-85/93⑤/89 三处互洽无残留矛盾（「空格/顺序可调」仅存废止语境）；QA-CT 与 feature 契约一致；01-persistence-config feature/qa 的指针引用不受影响 | `rg -n "空格/顺序可调" project/`（仅 phase-01 废止注记 + feature 头修订段命中） |

### 待办与待批（未决项）
- **无操作者待批项**：本轮为指令直落（操作者第七批指令），钦定文本全盘按指令落地；
  值一致性比对无冲突，无需操作者裁决面。
- **coder 实现（下一轮）**：见「四、移交建议」改动面。
- **QA 套件脚本化**（累积）：QA-CT-01..05、QA-NP-01..18、QA-MT-01..08、QA-SQ-14/15 待 QA
  会话落成 `qa/runners/suite_*.py` 并入回归；QA-CT-01/05 需先把钦定文本落盘为基线文件
  （源：`features/01-config-template.feature` 内嵌原文）；QA-NP-08/15/18 前置需扩展
  ezr-proxy（沿既有移交口径）。
- **遗留受限项**：沙箱无 TTY（pty E2E 归 QA 会话）；CRAP 受限近似口径（操作者已裁决维持）。

## 三、上一轮产出存档（coder-20261005-v19，本轮基线）

### 产物与验证（本轮 specifier 零代码，基线不变）
- v1.9 全链落地：`ProxyRaw` 增 ip/port 删 url、`ProxyKind` 三值（Http/Https/Socks5）
  + scheme()/label() 同源、`from_toml` 校验链重写（type 缺失/未知作废、ip 空白作废、
  port 越界作废、凭证规则三类型）、`ProxyConfig` 字段 ip/port 化 + `endpoint_url()`
  （IPv6 方括号，单一构造点）；engine `build_endpoint_client` 三值分支（Https 与 Http
  同路 basic_auth；Socks5 userinfo RFC 1929）；dialogs 夹具 ip/port/type 化 + 下拉
  三值标注；README 代理节同步。
- 验证基线：**1305P/0F（10 二进制 297/30/11/106/235/234/2/12/212/166）**、clippy 0、
  fmt 0、arch_check 全通过、测试名唯一性 0 重复；对账删除 3 / 改写 12 / 新增 8
  （+29 与 1305−1276 精确自洽）。
- **v1.10 实现相关代码现状**：`Config::default_template()`（config.rs L437–514）现按
  v1.9 口径逐行 `push_str` 拼装（含 3 行文件头说明、proxies 段 10+ 行解释性注释、
  示例块凭证语义注记）——v1.10 需整段替换为钦定文本；模板契约测试现有
  `template_parses_to_defaults`（往返，保留）与逐行 contains 式断言（需改整文本比对）。

## 四、移交建议
- 本轮为纯规格轮（零代码改动），建议操作者运行 `bin/swarm complete` 归档后链式启动
  `six-pack/coder` 实现 v1.10（TDD）。改动面集中且小：
  1. **`project/ezr/src/model/config.rs`**：`default_template()` 整段替换——输出与
     `features/01-config-template.feature` 内嵌钦定原文**逐字节一致**（54 行、11 空行、
     EOF 单换行收尾；不得增删改任何字符/空行）。建议以整文本字面量（`concat!` 或
     裸字符串常量）替代逐行 `push_str` 拼装；`ensure_default_config`（create-new 语义、
     D17 不覆写/静默）零改动。
  2. **模板契约单测改写**：新增/改写为**整文本精确比对**断言（生成输出 ≡ 钦定原文，
     含空行/行序/EOF——不要逐行 contains，防多余行/空行漂移；基线可内嵌同一字面量
     或 `include_str!` 对照文件）；`template_parses_to_defaults` 往返契约保留；
     旧「凭证规则注释行/三类型语义注释/示例块凭证注记」相关断言删除（FR-01-93⑤
     废止承载）；9 键三行式与双示例块断言并入整文本比对即可。
  3. **README**：配置节 v1.9 凭证/三类型解释**保留**（语义承载地，正是 v1.10 的去向）；
     仅 L79–81「每个参数注明用途与取值范围」一句宜微调（9 键注明、proxies 段最小化），
     属可选措辞同步。
  4. **无需常量改动**：值一致性 9/9 一致（见上表），DEFAULT_* 全部原样。
- QA 会话后续承接：QA-CT-01..05 脚本化（整文本 cmp 基线落盘）+ QA-NP-01..18 脚本化
  + ezr-proxy 三类监听扩展（https / socks5+认证 / IPv6 回环）。
