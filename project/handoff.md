# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（操作者修订轮——reload-toast-unify）
> 路径：…six-pack/specifier 闸门 B 落实轮（上轮）⇒ **six-pack/specifier（本轮，操作者修订轮指令 20261124「检查并修改下：config 中任何配置项的被改动，热加载config 后都不需要单独的生成 toast 消息，统一用一个 '配置已重新加载' 的toast 消息就可以了」；完成后 complete 归档）**
> 会话：six-pack-specifier-20261124-reload-toast-unify（上游交接 = 上轮 handoff（闸门 B 落实轮）+ 操作者 chat 指令；ezr 不做旧版本兼容——操作者钦定沿 v1.45，本轮纯规格修订 + demo 同构无持久化格式变化）
> 环境：rustup stable 1.96.0（aliyun 镜像，上轮沿用）；bb v1.13.230（.tools/bin）+ APS 仓库 .tools/Acceptance-Pipeline-Specification（gherkin-parser / gherkin-ir-dry-checker 两步流水线复证可用）；本轮新增 `~/.cargo/config.toml` aliyun sparse registry（cargo 依赖下载镜像，官方源 30s 超时定性 → aliyun fetch exit 0，详见 notes/environment.md）。
> 受限项：chat 选择题面板不可用（IM 通道）——Inversion 受限退化沿 v1.45–v1.60 十轮先例，操作者指令文本即阶段 1 答案，本轮无残余歧义选择题（✓ 前缀子项登记 D81② 可改判）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.61**）；phase-01/02 为现行详述（03/04 计划登记稿）。specifier 为首角色，本轮按操作者修订轮指令改判 FR-01-119 ③ 成功反馈口径（mission v1.61 + phase-01 v1.57），保持原始意图不增删曲解。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同）；本轮**四面同步**（操作者指令改判 D81② 主干文案，demo 演示 toast 镜像文案随之失效——沿 v1.47–v1.56 九轮「操作者显式指令直接落地」先例）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固 | features 01–12 + qa 规程 20 份 + runners + APS 四件套 | 已交付（沿上轮账） | bins 1152P + 集成 8482P + 属性 90P + fmt/clippy/arch_check 全过 |
| specifier 引用消失 bug 修复轮（20261124，v1.59/v1.55） | mission v1.59 + phase-01 v1.55 + features/12-reload-config.feature v1.55 + qa v1.55 | 已交付（D82①–③ 已由闸门轮裁定结案） | APS parser exit 0 + dry-check 假阳性豁免 + rg 五处核验 |
| specifier 闸门 B 落实轮（20261124，v1.60/v1.56） | mission v1.60 + phase-01 v1.56 + features v1.56 + qa v1.56 | 已交付 | APS parser exit 0 + dry-checker exit 0 + rg 落盘核验 |
| **specifier 修订轮 reload-toast-unify（20261124，v1.61/v1.57）** | mission v1.61 + phase-01 v1.57（FR-01-119 ③ 改判 + D81②③⑤ 原位改判 + D82② 同步）+ features/12-reload-config.feature v1.57（14 场景成功 toast 断言全改判 + 场景 05 去 N 段 + 场景 08 去 K 段并改名去「提示」字样）+ qa/12-reload-config-qa.md v1.57（QA-RC-01–14 判据 + 通过准则同步）+ ezr-demo 四面同步（keys.rs / ui/mod.rs / docs 三表 + v1.61 补记 / README item 43） | 已交付 | APS parser exit 0（IR 14 场景齐序）+ dry-checker exit 0 + rg 残留核验（旧文案仅存历史注记原文与退役声明）+ demo cargo test 61P/0F |

### 遗留受限项（当前有效）
- 上轮验证面受限项全部沿袭有效（workers.rs 错误臂保留、bin 口径 cov=0 五处、2 条 unviable、bt/mod.rs pub mod 面、CPD 残留、CRAP nightly 口径漂移，详见上轮归档与 `qa/mutation-gap-registry.json`）。
- **D81①④ 沿袭有效；D81②③⑤ v1.57 操作者改判结案**（②成功 toast 组成改判固定文案——其中 ✓ 前缀子项为 specifier 解释裁决、可改判；③K 段退役；⑤N 仅存于发命令口径）；**D82①③ 追认保持、D82② 改判结案不可回退并经 v1.57 扩面收窄——D82 裁决项全部关闭**。
- chat 选择题面板不可用（IM 通道）——Inversion 访谈与合成确认闸门均按 engineering.md 受限退化执行；本轮操作者指令即修订裁决，无待答闸门。
- ezr-demo 树 rustfmt 存量漂移沿袭（demo 轮遗留，本轮未新增漂移面）；`bin/ezr-tui-demo` 预编译二进制不属 git 树（打包轮产物），本轮免重建，打包轮按 README「重新构建」口径处理。

### 实现定义值登记
- **D81②（v1.57 操作者改判结案）**：config 中**任何**配置项被改动，热加载成功后 toast **恒为单条固定文案 `✓ 配置已重新加载`**、不追加任何可选段（操作者钦定主干文案 '配置已重新加载'；✓ = ezr 成功类 toast 既有前缀惯例「✓ 下载完成/✓ 做种结束」——如要求去 ✓ 前缀可改判）；v1.54 拼接式汇总「主干 + N/K/M 可选段」口径退役。
- **D81③（v1.57 退役）**：C 类「需重启生效」K 段整段退役——C 类键生效语义不变仍需重启，仅不再 toast 提示。
- **D81⑤（v1.57 提示面退役）**：N 计数仅存于发命令口径（「有变化才发命令」判定基准不变），不再呈现于 toast。
- **D82②（v1.56 结案 + v1.57 扩面收窄）**：引用消失不追加专属提示段，v1.57 起统一固定文案 toast、可选段引用面清零。
- D81①④ 沿袭不变（缺失文件臂 / 失败 toast `⚠ 配置热加载失败：<明细>`）；「无配置文件」臂不变——失败/拒绝非「config 被改动成功热加载」语义，不受本裁决影响。

## 二、当前产出情况

### 本会话产物清单（specifier 修订轮 reload-toast-unify）
1. **`project/mission.md` → v1.61**：标题版本同步 + v1.61 头注（操作者指令登记、FR-01-119 ③ 固定文案改判总述、N/K/M 段退役、D81/D82 同步指针、demo 同构登记）。
2. **`project/mission/phase-01.md` → v1.57**：①v1.57 注记（③ 改判——固定文案 toast、N/K 段退役、失败/无配置文件臂不变声明、D81②③⑤ 与 D82② 同步总述）；②FR-01-119 行 ③（成功反馈改判）④（C 类「需重启生效」toast 段退役）⑤（无专属 toast 段口径 v1.57 更新）行内句替换；③D81 行原位改判（②固定文案 + ✓ 前缀子项登记、③K 段退役、⑤提示面退役、依据列补操作者 20261124 chat 指令）；④D82 行② 同步（可选段引用面清零）。
3. **`project/features/12-reload-config.feature` → v1.57**：头注 v1.57 修订块（指令登记 + 改判总述）+ Feature 描述（成功反馈句 + C 类句）+ 全部场景成功 toast 断言改 `✓ 配置已重新加载`（场景 01/02/03/04/07/09/10/11 计 12 处裸断言）+ 场景 05 去 N 段（代理端点变化仍发切换命令注记）+ 场景 08 三处断言去 K 段 + 场景 06/14 断言注记同步 + 场景 08 改名「C 类键需重启生效且监听端口维持启动绑定」（去「变化提示」退役字样，序号不重排）。
4. **`project/qa/12-reload-config-qa.md` → v1.57**：标题版本 + 头注 v1.57 修订块 + 行为口径（成功固定文案 toast）+ QA-RC-01–14 判据全改判（QA-RC-05 去 N 段、QA-RC-08 三处去 K 段、QA-RC-06/14 注记同步）+ 通过准则（toast 文案清单改固定文案口径、退役注记）。
5. **ezr-demo 四面同步（操作者指令直接涉及其镜像文案）**：`src/app/keys.rs` 演示 toast 改 `✓ 配置已重新加载（演示）`；`src/ui/mod.rs` 守卫测试断言 5 处 + 断言消息（D81② 固定文案）；`docs/ezr-demo UI界面及交互设计规范.md` v1.61 同步补记块（新增，v1.58 块历史原文保留）+ §4.2.4/§6.6/§7 三表现行文案同步；`README.md` 变更记录新增 item 43（item 42 历史原文保留）。
6. **notes 沉淀**：architecture.md（统一口径改判第七同步面 = demo 镜像引用面四处形态）+ environment.md（cargo 官方源下载超时定性与 aliyun sparse registry 配置）+ INDEX.md 轮次追加段。

### 验证证据
| 验证项 | 结果 | 命令（可复现，cwd = /home/z/swarm-ezr2） |
|---|---|---|
| APS parser | exit 0，IR 14 场景齐序（场景 08 新名落 IR、步数 11；06/14 步数 14/14） | `(cd .tools/Acceptance-Pipeline-Specification && ../bin/bb gherkin-parser /home/z/swarm-ezr2/project/features/12-reload-config.feature /home/z/swarm-ezr2/.work/tmp/12-reload-config.json)` |
| APS dry-checker | exit 0 | `(cd .tools/Acceptance-Pipeline-Specification && ../bin/bb gherkin-ir-dry-checker /home/z/swarm-ezr2/.work/tmp/12-reload-config.json /home/z/swarm-ezr2/.work/tmp/12-reload-config.dry.json)` |
| 退役文案残留核验 | 规格树旧文案字样 8 处命中全属历史注记原文（mission v1.58/v1.60 行、phase-01 v1.56 注记、feature v1.56 头注块、qa v1.56 头注块）与退役声明（phase-01 v1.57 注记/D81⑤/FR-01-119⑤、qa v1.57 头注块）——历史注记不改写惯例 | `rg -n '配置已热加载\|代理切换：[N0-9１]\|；K 项需重启生效' project/mission.md project/mission/phase-01.md project/features/12-reload-config.feature project/qa/12-reload-config-qa.md` |
| ezr-demo 测试 | 61P/0F（含 Ctrl+R toast 断言 5 处改判后全绿；编译 42s） | `(cd ezr-demo && source ~/.cargo/env && cargo test)` |
| 产品源码零改动 | 本轮为规格轮 + demo 同构，`project/ezr/` 未触碰 | `git -C project/ezr status --short src`（空输出） |

### 对账口径
- **改动面**：需求/规格/QA 四文件（mission + phase-01 + feature + qa）+ ezr-demo 四文件（src/app/keys.rs、src/ui/mod.rs、docs 设计规范、README）+ notes 三文件（architecture/environment/INDEX）；产品端 ezr 源码零改动（产品端实现归下一 coder 轮）。
- **场景计数**：14 → 14（场景 08 原地改名去退役字样，序号不重排）；QA 用例 14 → 14（QA-RC-01–14 判据改判）；IR 步数核验 parser 实测（08 = 11 步）。
- **行为变化面**：仅 toast 提示面——成功 toast 恒为固定文案 `✓ 配置已重新加载`（主干文案改判 + N/K/M 全部可选段退役）；发命令口径（re-resolve 有变化才发 Reconfigure/ReconfigureBt）、C 类键需重启语义、引用消失落库转直连（D82①）、双对话框重建（D82③）、CLI --max-speed 覆盖保持、缺失文件臂、失败/无配置文件臂全部不变。

### 待办与待批（未决项）
- 无待批项——操作者修订轮指令即裁决（D81②③⑤ 改判落定；仅 ✓ 前缀子项登记 D81② 可改判供操作者否决）；本会话无阻塞项，验证全绿。

### 移交建议
- **下一步运行 `six-pack/coder`**：按 FR-01-119（含 v1.57 固定文案口径）实现产品端 Ctrl+R 配置热加载。实现要点：keys.rs CONTROL 分支 / App::reload_config / Cmd::ApplyGlobals / EngineShared burst·sleep 原子化 / Task.running_proxy / help.rs 条目 + **本轮口径**：成功 toast **恒为固定文案 `✓ 配置已重新加载`**（不拼接任何可选段——代理切换计数仅用于发命令判定不入 toast、C 类变化不再 toast 提示）、引用消失落库转直连（Task.proxy 改写 + 注册表持久化）、proxy_options/proxy_labels 重建双对话框面（含浮层内同步与已选名回退直连）。验收契约 = FR-01-119 + features/12-reload-config.feature（14 场景）。
- 归档：请操作者运行 `bin/swarm complete`（产物复制 /home/z/my-project/download/ 由操作者执行）。

By specifier.
