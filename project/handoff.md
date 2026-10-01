# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier　会话：specifier-20260928（R1 建档）+ specifier-R2（修订轮）

## 一、产物历史完成情况

### 项目与需求

- EZR Downloader（ezr download）：TUI 高性能多协议下载器。正式版一期（01）= 把 ezr-tui-demo 定稿
  的界面接到真实 HTTP/HTTPS 下载内核（AIR2 分块、断点续传、重试退避、校验、限速、代理、持久化）。
- 需求权威来源：`project/mission.md`（分期模式大纲，v1.1）+ `project/mission/phase-01.md`（01 期详述，v1.1）。
  分期模式：feature 与 QA 文档依据 phase-01.md 生成，文件名/头部标注期号 01。

### 产物总账

| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier R1 | `features/` 9 份 Gherkin 规格（115 场景）+ `qa/` 9 份端到端套件 | 已交付，操作者以「继续」确认（否决窗口保留） | gherkin-parser 9/9 通过；dry-checker 复核完毕 |
| specifier R2 | 上述产物的修订轮：5 项操作者变更三层一致落地（需求档案 + feature + QA） | 已交付 | APS 复跑 9/9 通过；唯一 high 为前轮已登记的有意重复 |

- 9 份 feature：01-add-task(17) / 01-download-engine(16) / 01-resume-sidecar(13) /
  01-slots-queue(13) / 01-retry-backoff(10) / 01-integrity-check(11) / 01-throttle-proxy(11) /
  01-persistence-config(10) / 01-tui-display(14)，括号为场景数（R2 未增减场景数）。
- 9 份 QA 套件与 feature 一一对应，用例级映射全部 115 场景，覆盖 AC-1~AC-11。

### 遗留受限项（当前有效）

- QA-IC-11（文件大小不符）依赖 fixture「Range 短响应」模态——AC-10 fixture 能力清单之外的
  新增需求，coder 实现 fixture 时需一并支持，否则该场景以受限方式验证。
- QA-RB-09（磁盘预检）依赖受限小容量保存目录（64MB tmpfs 或等效），QA 环境需提供。
- HTTPS 正向用例（QA-DE-12、QA-TP-09/10）需要 fixture 证书受系统信任（QA 环境预装 CA），
  自签形态仅用于证书错误负向用例。
- R2 后注：QA-TP-09/10 的代理途径改为配置文件（环境变量仅作负向验证），HTTPS 正向用例仍需受信 CA。

### 实现定义值登记（如规格含「由实现定义」项）

- 无。规格未引入「由实现定义」值；未逐字定义文案的新增 toast（URL 非法、校验码格式/位数、
  校验伴随文件无效、`-x` 参数非法等）以语义断言表达，具体文案由 coder 按 demo 风格定并保持稳定。

## 二、当前产出情况

### 本轮（R2 修订轮）产物清单

操作者行使否决窗口提出 5 项变更，已按「三层一致」（需求档案 → feature → QA）改齐：

1. **CLI `-x` 改为 `<算法>=<校验码>` 显式算法**（原按位数自动匹配取消）：
   phase-01.md FR-01-01/FR-01-50/D15；01-add-task-12/13、01-integrity-check-01；QA-01-12/13、QA-IC-01。
2. **等待下载槽位的任务不预取文件大小**（推翻 R1 澄清⑨）：01-download-engine-15（等待行显示
   「未知」，获槽探测后才显示大小）；QA-DE-15。
3. **默认下载目录 = 用户主目录下的下载目录**（Linux `~/Downloads`；macOS `~/Downloads`；
   Windows `%USERPROFILE%\Downloads`，推翻「当前工作目录」）：phase-01.md FR-01-03；
   01-persistence-config-03、01-add-task-11；QA-PC-03、QA-01-11。
4. **`Retry-After` 存在即优先采用，取消 60s 上限**：phase-01.md FR-01-43；01-retry-backoff-05/06
   （05 增加 90s 行、06 改为「120s 仍采用其值」）；QA-RB-05/06。
5. **代理仅配置文件，不读取环境变量**：phase-01.md FR-01-61/AC-8；01-throttle-proxy-08/09/10
   （09 由「环境变量生效」反转为「环境变量不读取」负向用例，10 改经配置代理走 CONNECT）；QA-TP-08/09/10。

- 需求档案同步：phase-01.md 升 v1.1（头部新增修订说明）；mission.md 升 v1.1（§3.1 校验口径、
  §4 槽位/块大小与 D13 对齐——补齐 R1 未落地的同步）。
- 顺带修正：01-integrity-check-01 示例中 SHA-384/SHA-512 样本值位数不足（72/96 → 96/128 位）。

### 派生定案（R2 修订引出的口径空缺，按最一致方式定案）

- `-x` 算法名集合 = 伴随文件后缀集（md5/sha1/sha224/sha256/sha384/sha512/adler32），**大小写不敏感**
  （与 D14 口径一致；示例含 `MD5=` 大写行）。
- `-x` 校验码位数与所指定算法不符 → **启动即报错退出**（与对话框确认时报错、原 D15「启动报错」精神一致；
  01-add-task-13 第三行示例覆盖）。
- `-x` 缺少 `=` 前缀 / 算法无法识别 / 位数不符三类非法统一报「`-x` 校验参数非法」语义断言（01-add-task-13）。

### R1 产物与本会话承继

- `project/features/01-*.feature` × 9、`project/qa/01-*-qa.md` × 9（R1 建档，R2 修订其中 6+6 份）。
- R1 的 9 项澄清决定（①~⑨）继续有效，唯 ⑨「等待任务添加即预取大小」已被本轮变更为「不预取」。
- 工具安装事实：Babashka v1.13.224 位于 `.tools/bin/bb`（会话内 PATH 需注入该目录）；
  APS 工具仓库位于 `.tools/Acceptance-Pipeline-Specification`；检查脚本 `scripts/aps-check.sh` 可复跑。
- 经验沉淀：engineering.md「提问与确认交互」新增「完成后修订轮的处置」条款（三层一致、
  总账追加不覆盖；R1 已有「继续/按推荐」条款）。

### 验证证据

| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| Gherkin 解析（R2 后全量） | 9/9 通过 | `sh scripts/aps-check.sh`（逐份 gherkin-parser + gherkin-ir-dry-checker，IR/报告在 `.work/tmp/`） |
| dry-checker 复核（R2 后全量） | 0 条新增高置信未决 | 唯一 high = 01-add-task-05「按 Enter 确认」两次出现，系场景两次对话框确认流程的有意重复（R1 已登记，处置不变）；其余均为 medium 中文分词假阳性（aps.md 口径人工复核） |
| 旧口径残留扫描 | 无残留 | 全库 grep「当前工作目录/位数自动匹配/Retry-After（≤60s）/配置优先/添加即预取」，仅命中决定历史引述（D13/D15 记录） |

### 对账口径（如存在基线）

- 无上游基线（本流程首会话）。产物数字自洽：场景 115 = 17+16+13+13+10+11+11+10+14（R2 未增减）；
  IR 展开行数以 `.work/tmp/*.ir.json` 实测为准。

### 待办与待批（未决项）

- **派生定案复核**：上述「派生定案」3 条由 specifier 按一致性原则定案，操作者如有异议请指出，随即修订。
- **规格确认**：R2 修订已按操作者 chat 指令逐项落地；确认无误后运行 `./bin/swarm complete` 结束修订轮。
- 否决窗口保持开放：任一修订不符意图时向 specifier 指出，相关 feature/QA/需求档案随之修订。

### 移交建议

- 建议下一角色 **six-pack/coder**：以 `features/` 九份规格（R2 修订后版本）为行为依据 TDD 实现；
  先搭 fixture 服务器基建（能力清单见 `qa/` 各套件环境前置节，含 Range/断连/5xx/Retry-After/
  慢速/Content-Disposition/ETag 变化/重定向链/代理/HTTPS 双证书/Range 短响应/稀疏大文件；
  代理能力需支持双代理区分记录），再按 feature 逐份推进。
  Rust 配置模板强制遵循 `packs/_common/notes/rust.md`（宪法与 mission §4 约束）。
