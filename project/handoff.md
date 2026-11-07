# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（规格审批改判轮，本轮已完成）
> 路径：specifier → coder → cleaner → architect → hardender（操作者指示跳过并关闭）→ QA → Done
> 　　→ 复验轮（2026-11-06）→ specifier 规格修订轮（v1.45 守卫）→ **specifier 审批改判轮（本轮：v1.46）**
> 会话：ezr-specifier-approval-overturn-round（2026-11-06，操作者审批「场景 14 的情形套用场景 15 的
> 处理，遗留的 sidecar 由用户手工删除」→ phase-01 v1.46 三层合成 + APS parser/dry-check + 交接收尾）
> 上游交接：project/handoff.md（v1.45 守卫轮交接）+ 操作者 chat 审批（改判指令，审批原文随注记登记）
> 环境注记：沙箱重置后自 my-project 归档重建工作区；rustup 1.29.3 + nightly 1.101.0（TUNA 前台装齐，
> PATH=~/.cargo/bin）；bb 1.13.229 + APS 在 `.tools/`（gh-proxy 前缀装，法见 notes/environment.md）。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器。需求权威 `project/mission.md`
  （分期模式 **v1.44**，本轮 +1）+ `project/mission/phase-01…04.md`（phase-01 现 **v1.46**，本轮 +1）。
- 本轮为 **specifier 审批改判轮**：消化操作者对 v1.45 真值表第三行（异 URL 同推导名 + 原任务已删除）
  的审批改判，产品代码零改动。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier/coder/cleaner（历轮） | bug25 三项规格 + 实现 + demo 同构 + 保持行为清理 + 覆盖率/CRAP/DRY/变异点度量 | 已交付 | 沿袭全绿（QA 轮六层基线复验佐证） |
| architect（2026-11-06） | 四阶段架构评审 + 架构改进 2 项 + 属性测试 +3 + arch_check 16 规则 | 已交付 | 928P/0F + 集成 6201P + 16 规则全绿 |
| QA + 复验（2026-11-06） | 六层基线复验 + CRAP/DRY 度量 + 遗留受限项复验（15→1 有效）+ KeepAlive 产品修复 + QA runners 修复（9 处） | 已交付 | 928P/0F + 6201P/0F + 14 套件全绿（明细见 QA/复验轮归档交接） |
| specifier 守卫轮（2026-11-06，v1.45） | mission v1.43 + phase-01 v1.45（FR-01-26「无活任务占用」守卫）+ 场景 15 + QA-RS-15 | 已交付 | parser 15 场景 + dry-check exit 0（101 findings 复核假阳性） |
| **specifier 审批改判轮（本轮，v1.46）** | mission v1.44 + phase-01 v1.46（FR-01-26/22/02 改判）+ 场景 14 改判 + QA-RS-14 改判 | 已交付 | parser 15 场景 + dry-check exit 0（108 findings 复核假阳性） |

### 遗留受限项（当前有效）
- **leech 判据无 e2e 自动化**（复验轮登记沿袭，six-pack 链唯一有效受限项）：QA-AL-01..08
  规格；需专项测试轮补 runner/引擎级用例，夹具须容纳 120s 判据窗。
- 本轮无新增受限项（bb/APS 经 gh-proxy 重装跑通；沙箱重置经归档重建闭环，不再登记）。

### 实现定义值登记
- 本轮未新增任何实现定义值；既有登记值全部不变（面板层速度格式化/判重目录解析/并发列宽/
  客户端收缩基线/bad_piece 归属 D67 等，见历轮归档交接同节）。
- 本轮**合成裁决点**（非实现定义值，操作者可否决）：**判别载体 = FR-01-22 三要素探测**
  （sidecar 无 URL、已删任务不在注册表 → 同/异 URL 无元数据可判）——探测相符（实为同一文件，
  含异 URL 同内容镜像边界）→ 原地接续（场景 13）；不符 → 序号改道（场景 14）。已随 phase-01
  v1.46 注记登记。

## 二、当前产出情况

### 本会话产物清单
- `project/mission.md`（v1.43→**v1.44**）：v1.44 审批改判轮注记行。
- `project/mission/phase-01.md`（v1.45→**v1.46**）：v1.46 注记块 + **FR-01-26 改判**
  （孤儿接续臂「原地作废从头」→「序号改道 + 孤儿保留」）+ **FR-01-22 双臂分叉**
  （自有任务恢复臂不变 / 路径基孤儿臂改判）+ **FR-01-02 序号触发源扩两源**。
- `project/features/01-resume-sidecar.feature`（15 场景不变）：v1.46 修订注 + Feature 描述补
  v1.46 句 + **场景 14 改判重写**（序号改道 + 孤儿保留 + 静默）+ 场景 13 补判别前提注；
  场景 15 不变。
- `project/qa/01-resume-sidecar-qa.md`：v1.46 同步注 + **QA-RS-14 改判**（序号改道预期 + 孤儿
  保留断言 + 静默）+ QA-RS-13 补前提注 + 环境前置措辞同步。
- `project/notes/environment.md` + `notes/INDEX.md`：本轮环境经验条目（沙箱重置归档重建、
  dist 清单架构无关路径与绝对 URL 重拼、TUNA 瞬态 404 复验判据、审批改判流程先例）。
- `.work/tmp/`：`01-resume-sidecar.json` / `.dry.json`（APS 产物，会话级不进归档）。
- 产品端（`ezr/`、`ezr-demo/`）：**零改动**（实现由 coder 轮承接）。

### v1.46 改判内容（操作者审批原文「场景 14 的情形套用场景 15 的处理，遗留的 sidecar 由用户手工删除」）
- **改判面**：真值表第三行（异 URL 同推导名 + 原任务已删除）——v1.45「路径基接续 + FR-01-22
  原地兜底」退役，改：**探测三要素不符 → 序号改道 `<名>.1` 全新下载 + 静默（套用场景 15 处理）
  + 孤儿 sidecar 与部分文件原样保留（不自动清理，由用户手工删除）**；原地作废从头臂退役。
- **判别载体（合成裁决，登记供否决）**：FR-01-22 三要素探测——相符 → 原地接续（场景 13 不变）；
  不符 → 序号改道（场景 14）。
- **FR-01-22 双臂分叉**：自有任务恢复臂（07/08/09）不符 → 原地作废从头不变（防循环 3 次停等
  不变）；路径基孤儿臂不符 → 序号改道（静默；改道后与孤儿路径解耦、无重复失效面，防循环条款
  不适用）。
- **FR-01-02**：序号触发源两源（① v1.45 任务列表占用判定 ② v1.46 孤儿匹配探测不符）。
- **不变面**：场景 13（同 URL + 已删接续）、场景 15（v1.45 守卫臂）、01-add-task-10/16 判重
  窗口、FR-01-25 删除联动语义、孤儿路径复用前提 = 用户先手工删除 sidecar 与部分文件。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| APS gherkin-parser | 15 场景解析通过（场景 14 改判文本入 IR） | `(cd .tools/Acceptance-Pipeline-Specification && ../../.tools/bin/bb gherkin-parser /home/z/swarm-ezr2/project/features/01-resume-sidecar.feature /home/z/swarm-ezr2/.work/tmp/01-resume-sidecar.json)` |
| APS gherkin-ir-dry-checker | exit 0；108 findings（near-dup 49 + synonym 59，涉场景 14 的 33 条）全人工复核 = 中文分词假阳性（相符/不符反义臂、保留/生成镜像臂、13/14/15 共享 sidecar 词汇碰撞），advisory 不改文本 | 同目录 `bb gherkin-ir-dry-checker …/01-resume-sidecar.json …/01-resume-sidecar.dry.json` |
| 规格三层同步核对 | mission v1.44 行 / phase-01 v1.46 注记+三条款 / feature+QA 同步；场景号 13/14/15 连续、QA-RS-13/14 对应、场景 15 未动 | 人工对账 |
| 工具链 | rustup 1.29.3 + nightly 1.101.0 (1c3a65069 2026-11-02) + rustfmt/clippy（TUNA 前台装齐，与历轮版本带一致） | `rustup show` / `cargo fmt --version` |

### 待办与待批
- 无待批。合成按操作者审批执行；判别载体为合成裁决（审批未覆盖的机制面），已随 v1.46 注记
  登记——如需再改判（如「相符也一律序号」「恢复 sidecar 存 URL」）可另开 specifier 修订轮。
- **致 coder（实现 v1.46 改判臂）**：①路径基孤儿臂的改道点在任务获得槽位、探测完成时（添加时
  目标名初判 `<推导名>`，探测不符后改判 `<名>.1` 落盘下载）；②孤儿 sidecar/部分文件零动作
  （不重命名、不删除、无提示）；③v1.45 守卫臂（任务列表占用）先于路径基匹配，勿合并；④同 URL
  判重（01-add-task-16 错误窗口）先于接续臂不变；⑤v1.45 守卫臂若未实现则 FR-01-26 v1.45+v1.46
  一并承接。
- **致 QA（QA-RS-14 落地）**：夹具沿用 files/alias.bin ↔ other/alias.bin（内容不同）+ 原任务
  「仅删除任务」留孤儿；新增断言孤儿 `<名>.ezr` 与部分文件 `<名>` 字节不变、新任务独立
  sidecar `<名>.1.ezr`、全程静默无 toast；CLI 同口径抽测。

### 移交建议
- 下一步运行 `six-pack/coder` 技能实现 v1.46 改判臂（唯一产品变更点，规格已就绪）。
- 工具链交接：rustup 1.29.3 + nightly 1.101.0（PATH=~/.cargo/bin，TUNA 前台装齐）；
  bb 1.13.229 + APS 仓库在 `.tools/`；swarm 工作区 `/home/z/swarm-ezr2`。
