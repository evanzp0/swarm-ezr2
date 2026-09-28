# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier　会话：specifier-20260928

## 一、产物历史完成情况

### 项目与需求

- EZR Downloader（ezr download）：TUI 高性能多协议下载器。正式版一期（01）= 把 ezr-tui-demo 定稿
  的界面接到真实 HTTP/HTTPS 下载内核（AIR2 分块、断点续传、重试退避、校验、限速、代理、持久化）。
- 需求权威来源：`project/mission.md`（分期模式大纲）+ `project/mission/phase-01.md`（01 期详述）。
  分期模式：feature 与 QA 文档依据 phase-01.md 生成，文件名/头部标注期号 01。

### 产物总账

| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier | `features/` 9 份 Gherkin 规格（115 场景 / IR 展开 188 行）+ `qa/` 9 份端到端套件 | 全部交付，待操作者确认 | gherkin-parser 9/9 通过；dry-checker 复核完毕（见当前节验证证据） |

- 9 份 feature：01-add-task(17) / 01-download-engine(16) / 01-resume-sidecar(13) /
  01-slots-queue(13) / 01-retry-backoff(10) / 01-integrity-check(11) / 01-throttle-proxy(11) /
  01-persistence-config(10) / 01-tui-display(14)，括号为场景数。
- 9 份 QA 套件与 feature 一一对应，用例级映射全部场景，覆盖 AC-1~AC-11。

### 遗留受限项（当前有效）

- QA-IC-11（文件大小不符）依赖 fixture「Range 短响应」模态——AC-10 fixture 能力清单之外的
  新增需求，coder 实现 fixture 时需一并支持，否则该场景以受限方式验证。
- QA-RB-09（磁盘预检）依赖受限小容量保存目录（64MB tmpfs 或等效），QA 环境需提供。
- HTTPS 正向用例（QA-DE-12、QA-TP-09/10）需要 fixture 证书受系统信任（QA 环境预装 CA），
  自签形态仅用于证书错误负向用例。

### 实现定义值登记（如规格含「由实现定义」项）

- 无。规格未引入「由实现定义」值；未逐字定义文案的新增 toast（URL 非法、校验码格式/位数、
  校验伴随文件无效等）以语义断言表达，具体文案由 coder 按 demo 风格定并保持稳定。

## 二、当前产出情况

### 本会话产物清单

- `project/features/01-*.feature` × 9：phase-01 行为规格（期号 01 标注于文件名与头部；
  场景名 = feature 名 + 稳定序号；公共前置已提炼 Background）。
- `project/qa/01-*-qa.md` × 9：端到端 QA 套件（UI 层：TUI 按键/鼠标/CLI/磁盘产物观察，
  不用项目内部 API；各套件含环境前置、用例表、通过准则）。
- 本会话 9 项需求澄清决定（Inversion 阶段 1，操作者以「继续」采纳全部推荐项）：
  ① 重复任务（同 URL+同路径）拒绝创建并 toast「任务已存在」；② CLI 多 URL 部分非法全部拒绝退出；
  ③ 无协议前缀 URL 拒绝；④ CLI `-c` 非法值启动报错退出；⑤ 一致性失效计数按 `R` 重置；
  ⑥ 重定向超 10 次与目标非 http(s) 均停等（不自动重试，`R` 可手动）；⑦ 默认并发键名
  `default_concurrency`；⑧ 限速十进制口径（1 MB=1000 KB，接受 B/s~GB/s，大小写/空格不敏感）；
  ⑨ 等待任务添加即预取大小（失败静默显示「未知」）。
- 另有两处自然语义编码：`Retry-After` >60s 回退指数退避（规格 01-retry-backoff-06）；
  失败 toast 未定文案以语义断言表达。
- 工具安装事实：Babashka v1.13.224 位于 `.tools/bin/bb`（会话内 PATH 需注入该目录）；
  APS 工具仓库位于 `.tools/Acceptance-Pipeline-Specification`（bb 任务调用约定见
  `packs/_common/engineering.md`「验收流水线（APS）」）；检查脚本 `scripts/aps-check.sh` 可复跑。
- 经验沉淀：engineering.md「提问与确认交互」新增「继续/按推荐」类整体推进指令处置条款。

### 验证证据

| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| Gherkin 解析 | 9/9 通过 | `sh scripts/aps-check.sh`（逐份 gherkin-parser + gherkin-ir-dry-checker，IR/报告在 `.work/tmp/`） |
| dry-checker 复核 | 0 条高置信未决 | 340 条 findings：338 条 medium 为中文分词假阳性（aps.md 口径逐类人工复核）；1 条 duplicate-in-scenario 为 01-add-task-05 两次确认流程的有意重复（保留）；1 条 placeholder-variant 已修复（throttle-proxy 占位符统一 `<speed>`，复跑清零） |

### 对账口径（如存在基线）

- 无上游基线（本流程首会话）。产物数字自洽：场景 115 = 17+16+13+13+10+11+11+10+14；IR 展开 188 行以 `.work/tmp/*.ir.json` 实测为准。

### 待办与待批（未决项）

- **否决窗口**：上述 ①~⑨ 澄清决定按操作者「继续」指令以推荐项采纳并已逐项明示；任一项不符意图时
  向 specifier 指出，相关 feature/QA 随之修订。
- **规格确认**：会话恢复后完成交付前终验（APS 复跑 9/9 通过、交付物抽查合格）并再次呈现确认摘要；
  操作者以「继续」推进（受限退化协议），否决窗口保持开放。specifier 节点工作全部完结，等待操作者
  运行 `./bin/swarm complete` 结束会话。

### 移交建议

- 建议下一角色 **six-pack/coder**：以 `features/` 九份规格为行为依据 TDD 实现；先搭 fixture 服务器
  基建（能力清单见 `qa/` 各套件环境前置节，含 Range/断连/5xx/Retry-After/慢速/Content-Disposition/
  ETag 变化/重定向链/代理/HTTPS 双证书/Range 短响应/稀疏大文件），再按 feature 逐份推进。
  Rust 配置模板强制遵循 `packs/_common/notes/rust.md`（宪法与 mission §4 约束）。
