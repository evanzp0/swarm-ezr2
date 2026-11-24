# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（闸门 B 答复落实轮——D82② 改判结案）
> 路径：…six-pack/specifier 引用消失 bug 修复轮（上轮）⇒ **six-pack/specifier（本轮，操作者合成确认闸门 B 答复「1、② M 口径 -- 热加载改代理不需要单独的消息提示，统一用 reload config 的toast 就行了 2、① 切换落库 和 ③ 改名臂 -- 保持」；完成后 complete 归档）**
> 会话：six-pack-specifier-20261124-gate（上游交接 = 上轮 handoff（引用消失 bug 修复轮）+ 操作者闸门 B 答复；ezr 不做旧版本兼容——操作者钦定沿 v1.45，本轮纯规格修订无持久化格式变化）
> 环境：rustup stable 1.96.0（aliyun 镜像前台重装，上轮沿用）；bb v1.13.230（.tools/bin）+ APS 仓库 .tools/Acceptance-Pipeline-Specification（gherkin-parser / gherkin-ir-dry-checker 两步流水线复证可用）。
> 受限项：chat 选择题面板不可用（IM 通道）——合成确认闸门按受限退化以 chat 消息答复执行（本轮已答复：B + D82② 改判，闸门关闭）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.60**）；phase-01/02 为现行详述（03/04 计划登记稿）。specifier 为首角色，本轮按操作者闸门 B 答复修订 mission（FR-01-119 ⑤ v1.56 修订 + D82② 改判结案），保持原始意图不增删曲解。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同）；本轮零改动（操作者指令未涉 demo）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固 | features 01–12 + qa 规程 20 份 + runners + APS 四件套 | 已交付（沿上轮账） | bins 1152P + 集成 8482P + 属性 90P + fmt/clippy/arch_check 全过 |
| specifier 引用消失 bug 修复轮（20261124，v1.59/v1.55） | mission v1.59 + phase-01 v1.55（FR-01-119 ⑤ 修订 + D82①–③）+ features/12-reload-config.feature v1.55（06 扩面 + 14 新增，14 场景）+ qa/12-reload-config-qa.md v1.55 | 已交付（D82①–③ 待批已由本轮闸门裁定结案） | APS parser exit 0 + dry-check 假阳性豁免 + rg 五处核验 |
| **specifier 闸门 B 落实轮（20261124，v1.60/v1.56）** | mission v1.60 + phase-01 v1.56（FR-01-119 ⑤ 修订 + D81② 同步 + D82② 改判）+ features/12-reload-config.feature v1.56（场景 06/14 断言改判 + 场景 06 改名，14 场景）+ qa/12-reload-config-qa.md v1.56（QA-RC-06/14 判据 + 通过准则同步） | 已交付 | APS parser exit 0（IR 14 场景齐序）+ dry-checker exit 0 + rg 落盘核验（M 段字样仅存历史注记原文与退役声明） |

### 遗留受限项（当前有效）
- 上轮验证面受限项全部沿袭有效（workers.rs 错误臂保留、bin 口径 cov=0 五处、2 条 unviable、bt/mod.rs pub mod 面、CPD 残留、CRAP nightly 口径漂移，详见上轮归档与 `qa/mutation-gap-registry.json`）。
- D81①–⑤（phase-01 D 表，可回退）沿袭有效（其中 ② 可选段清单已随 D82② 改判同步去 M 段）；**D82①③ 操作者追认保持、D82② 改判结案（不可回退）——D82 裁决项全部关闭**。
- chat 选择题面板不可用（IM 通道）——Inversion 访谈与合成确认闸门均按 engineering.md 受限退化执行；本轮闸门已以 chat 消息答复关闭。
- ezr-demo 树 rustfmt 存量漂移沿袭（demo 轮遗留，本轮未触碰 demo）。

### 实现定义值登记
- **D82②（v1.56 操作者改判结案）**：引用消失（删除/改名）不追加专属 toast 段，成功反馈统一 reload config 标准 toast `✓ 配置已热加载`（可选段仅按 D81②③⑤：N/K）；原 v1.55「M 段 = 引用已消失名字的任务总数全状态计」口径退役。
- D82①③ 追认保持（切换落库 / 改名新名入池不迁移引用）；D81①–⑤ 沿上轮登记不变（② 可选段清单去 M 段）。
- 帮助条目/toast 等钦定文案零改动；无其他新增 D 值。

## 二、当前产出情况

### 本会话产物清单（specifier 闸门 B 落实轮）
1. **`project/mission.md` → v1.60**：标题版本同步 + v1.60 头注（操作者闸门 B 答复登记、D82② 改判结案 + D82①③ 追认保持、FR-01-119 ⑤ toast 修订总述、规格/QA 指针）。
2. **`project/mission/phase-01.md` → v1.56**：①v1.56 注记（D82② 改判结案——M 段退役、统一 reload 标准 toast、D81② 可选段清单同步、引用消失不计入 N 口径澄清）；②FR-01-119 行 ③（M 段退役注记）⑤（无专属 toast 段）修订；③D81 行 ②（「代理删除降级 M>0」触发臂退役）与 D82 行（② 改判结案、行首状态声明、依据列补闸门答复）修订。
3. **`project/features/12-reload-config.feature` → v1.56**：头注 v1.56 修订块（闸门裁定 + D82② 结案声明）+ Feature 描述可选段清单去 M 段 + 场景 06 改名（「…落库转直连与添加修改下拉重建」，去「汇总提示」）+ 场景 06/14 toast 断言改判（「toast 显示 "✓ 配置已热加载"（引用消失不追加专属提示段，统一 reload toast，D82②）」形态；场景 06「再次按 Ctrl+R」句与场景 14「无删除段」句同步改判）。
4. **`project/qa/12-reload-config-qa.md` → v1.56**：标题版本 + 头注 v1.56 修订块 + 行为口径可选段清单去 M 段 + QA-RC-06 判据（toast 仅 `✓ 配置已热加载`）+ QA-RC-14 判据（改名 toast 仅 `✓ 配置已热加载`、浮层内句同步）+ 通过准则（toast 文案清单出 M 段、触发条件按 D81②③⑤、M 口径退役注记）。

### 验证证据
| 验证项 | 结果 | 命令（可复现，cwd = /home/z/swarm-ezr2） |
|---|---|---|
| APS parser | exit 0，IR 14 场景齐序（06/14 步数 14/14，场景 06 新名落 IR） | `(cd .tools/Acceptance-Pipeline-Specification && ../bin/bb gherkin-parser /home/z/swarm-ezr2/project/features/12-reload-config.feature /home/z/swarm-ezr2/.work/tmp/12-reload-config.json)` |
| APS dry-checker | exit 0 | `(cd .tools/Acceptance-Pipeline-Specification && ../bin/bb gherkin-ir-dry-checker /home/z/swarm-ezr2/.work/tmp/12-reload-config.json /home/z/swarm-ezr2/.work/tmp/12-reload-config.dry.json)` |
| 修订落盘核验 | mission v1.60 / phase-01 v1.56 四处 / feature 五处（场景 06 新名 + 断言改判）/ QA 五处；rg 计数核验通过 | `rg -c 'v1\.56 修订|v1\.56 起|D82②' project/mission.md project/mission/phase-01.md project/features/12-reload-config.feature project/qa/12-reload-config-qa.md` 等 |
| M 段残留核验 | feature 与 qa 正文零残留；全改动面 M 段字样仅存历史注记原文（mission v1.59 行 / phase-01 v1.55 注记）与退役声明（mission v1.60 行 / phase-01 v1.56 注记 / D81② / FR-01-119③）——历史注记不改写惯例 | `rg -n '已删除，按直连' project/mission.md project/mission/phase-01.md project/features/12-reload-config.feature project/qa/12-reload-config-qa.md` |
| 产品源码零改动 | 本轮为规格轮，`project/ezr/` 未触碰；ezr-demo 未触碰 | `git -C project/ezr status --short src`（空输出） |

### 对账口径
- **改动面**：仅需求/规格/QA 四文件（mission + phase-01 + feature + qa）；ezr 与 ezr-demo 源码零改动（产品端实现归下一 coder 轮）；notes 零改动（既有「命名代理引用消失三层约束」条目与改判后口径一致，无需去重合并）。
- **场景计数**：14 → 14（场景 06 原地改判 + 改名，序号不重排；场景 14 不变）；QA 用例 14 → 14（QA-RC-06/14 判据改判）；IR 步数 06=14 步、14=14 步（parser 实测）。
- **行为变化面**：仅 toast 反馈面收窄（引用消失不追加 M 段，成功 toast = `✓ 配置已热加载` + N/K 既有可选段）；落库转直连、双对话框重建、改名新名入池等引用消失臂其余语义全部不变（D82①③ 保持）。

### 待办与待批（未决项）
- 无待批项——D82①③ 追认保持、D82② 改判结案，合成确认闸门关闭；本会话无阻塞项，验证全绿。

### 移交建议
- **下一步运行 `six-pack/coder`**：按 FR-01-119（含 v1.56 引用消失臂）实现产品端 Ctrl+R 配置热加载。实现要点：keys.rs CONTROL 分支 / App::reload_config / Cmd::ApplyGlobals / EngineShared burst·sleep 原子化 / Task.running_proxy / help.rs 条目 + **本轮口径**：引用消失落库转直连（Task.proxy 改写 + 注册表持久化）、proxy_options/proxy_labels 重建双对话框面（含浮层内同步与已选名回退直连）、**引用消失无专属 toast 段（统一 reload 标准 toast `✓ 配置已热加载` + N/K 既有可选段，D82② 结案）**。验收契约 = FR-01-119 + features/12-reload-config.feature（14 场景）。
- 归档：请操作者运行 `bin/swarm complete`。

By specifier.
