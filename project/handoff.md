# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（规格轮——新增需求轮）
> 路径：…QA 最终验证轮（上上轮，六包链走完归档）⇒ **specifier 新增需求轮（本轮，操作者 chat 指令「用 six-pack/specifier 处理新增需求：根据 ezr-reload-cfg.md 实现 Ctrl+R 配置热加载」；完成后 complete 归档交付 /home/z/my-project/download/）**
> 会话：six-pack-specifier-20261123（上游交接 = 上轮 handoff（QA 终验轮）+ 操作者 chat 指令 + `project/ezr-reload-cfg.md` 操作者钦定改动方案；ezr 不做旧版本兼容——操作者本轮重申）
> 环境：rustup stable 1.100.0（TUNA 当期最快，前台安装）、bb v1.13.230 + APS 仓库已重建于 `.tools/`（沙箱回收后 specifier 口径重建；ghfast.top 前缀镜像当期最快 ~480KB/s）——重建与镜像当期实测已沉淀 notes/environment.md 尾节。
> 受限项：chat 选择题面板不可用——Inversion 阶段 1 受限退化沿 v1.45–v1.57 八轮先例执行（操作者指令文本 + ezr-reload-cfg.md 即阶段 1 答案；残余微口径选择题清单见「待办与待批」，答案以 D81①–⑤ 裁决登记、可回退）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.58**）；phase-01/02 为现行详述（03/04 计划登记稿）。
- ezr-demo：仓库内独立子项目（演示口径与产品行为有意不同，见 11-trackers-window v1.14 注记）；本轮无 demo 改动（操作者指令未涉及，方案明确只做产品端规格）。
- **EZR_DIR 核查（操作者本轮疑虑项，已结案）**：`ezr-reload-cfg.md` 与全仓库 **0 处 `EZR_DIR`**，代码与文档统一 `EZR_HOME`（config.rs `ezr_dir()`、README:36/77、D16）——无需改字，结论登记 mission v1.58 / phase-01 v1.54 注记。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固 | features 01–11 + qa 规程 20 份 + runners 20 套件 + APS 四件套 | 已交付（沿上轮账） | bins 1152P + 集成 8482P + 属性 90P + fmt/clippy/arch_check 全过；端到端 20 套件 239P + 钦定 S 15 个 |
| QA 最终验证轮 | 最终独立验证全序列重放 + 6 个欠账用例补齐 + BE-11 runner 加固 + CRAP/DRY 复验 | 已交付（沿上轮账） | verdict Σ = 1107 killed + 17 filtered；端到端 20 套件全绿 |
| **specifier 新增需求轮（本轮）** | mission v1.58 + phase-01 v1.54（FR-01-119、D81①–⑤）+ `features/12-reload-config.feature`（新建 13 场景）+ `qa/12-reload-config-qa.md`（QA-RC-01–13）+ notes 尾节增补 | 已交付 | APS parser 13/13 解析成功 + ir-dry-checker high 置信发现 0；四层一致核对全过（场景名清单/QA 用例表/FR 引用/mission 注记） |

### 遗留受限项（当前有效，沿上轮账 + 本轮新增）
- 上轮验证面受限项全部沿袭有效：workers.rs 227-228/239-240 集成级错误臂、257-258 死防御臂保留、bin 口径 cov=0 五处登记、2 条 unviable 终态、bt/mod.rs 8 子模块 pub mod 面维持、DRY 几何再登记残留（CPD 221 vs 219 +2）、CRAP 行覆盖跨 nightly 口径漂移登记（详见上轮归档与 `qa/mutation-gap-registry.json`）。
- **本轮新增（登记状态与影响）**：①`features/12-reload-config.feature` 为**新建文件**，头部暂无 mutation-stamp/manifest 注释块（工具自管理注释，归 coder 后续轮次跑 mutator 时生成）；②QA-RC-12（无主目录臂）若沙箱无法构造 HOME/USERPROFILE/EZR_HOME 全未设置启动则记 S（判据已内嵌规程）；③`.work/tmp/` 下 APS 中间产物（12-reload-config.json/.dry.json）为会话运行时状态，不进归档。
- **工具状态**：bb v1.13.230 + APS 在 `.tools/`（跨会话复用）；llvm-cov/PMD 未重建（本轮未用到，coder/QA 轮按 notes/environment.md 恢复顺序按需重建）。

### 实现定义值登记
- 产品契约微口径 **D81①–⑤**（登记于 phase-01 D 表，**可回退**）：①缺失文件臂 = 默认配置整体热加载（不重生成模板）；②成功 toast = 单条拼接式（主干 + 代理切换/需重启生效/代理删除降直连可选段，段省略规则随 D81②）；③C 类「需重启生效」段仅 C 类键实际变化时出现（K = 实际变化键数）；④失败 toast = `⚠ 配置热加载失败：<明细单行化>`（⚠ 前缀，不扩展错误窗口）；⑤「代理切换：N 个任务」N = 运行中且端点实际变化任务数（= 实发 Reconfigure/ReconfigureBt 数）。其余历轮实现定义值沿历轮登记不变。

## 二、当前产出情况

### 本会话产物清单（specifier 新增需求轮）
1. **mission.md v1.58**：新增需求轮注记（Ctrl+R 配置热加载全貌 + EZR_DIR 核查结论 + Inversion 受限退化登记）。
2. **phase-01.md v1.54**：头部注记 + 3.8 节新增 **FR-01-119**（P0，九臂行为契约：全局可达/原子性/成功反馈/四类键语义/代理热切/CLI 覆盖保持/缺失臂/帮助窗口/不做清单）+ D 表新增 **D81①–⑤**。
3. **`features/12-reload-config.feature`（新建，13 场景）**：场景清单注释 + 期号/依据头注；覆盖 全局可达（01）/A 类三键（02–05）/代理删除降级（06）/B 类快照（07）/C 类重启提示（08）/原子拒绝（09）/CLI 覆盖（10）/缺失文件（11）/无主目录（12）/帮助条目（13）；参数仅保留变异相关列（恒值列剪除内联化）。
4. **`qa/12-reload-config-qa.md`（新建，QA-RC-01–13）**：与场景一一对应；环境前置（PTY/HTTP 慢速与断连 fixture/双端点代理 fixture/BT 四件套/config 运行期外部改写）/用例表（操作 + 逐字 toast 判据）/通过准则（原子性三通道交叉验证、fixture 日志为代理热切唯一事实来源、六项关联回归）。**runner 可执行化归 QA 轮**（沿 QA SKILL 职责边界）。
5. **notes 尾节增补（去重后追加）**：environment.md——镜像当期复测（GitHub 直连慢速可用/ghfast.top 480KB/s/mirror.ghproxy 与 moeyy 不通；rustup 四镜 TUNA 最快）+ .tools 重建口径（bb v1.13.230 + APS 两步式 dry-check 流水线）；testing.md——新建 feature 的 dry-check 基线对照口径（high 置信 duplicate-in-scenario/placeholder-variant 才是必改项）。INDEX.md 无需变更（两文件已在索引）。
6. **`.tools/` 重建**：bb v1.13.230（.tools/bin/bb）+ APS 仓库 clone——跨会话复用，不进归档。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| Gherkin 解析 | 13/13 场景解析成功 | `(cd .tools/Acceptance-Pipeline-Specification && PATH="$PWD/../bin:$PATH" bb gherkin-parser <abs>/project/features/12-reload-config.feature <abs>/.work/tmp/12-reload-config.json)` |
| IR dry-check | high 置信发现 0（修正 duplicate-in-scenario ×1、placeholder-variant ×1 后）；余项 near-duplicate 57 / possible-synonym 125 与基线 feature 同类型同量级 | `… bb gherkin-ir-dry-checker <abs>/.work/tmp/12-reload-config.json <abs>/.work/tmp/12-reload-config.dry.json` |
| 四层一致性 | 场景清单↔Scenario 行↔QA 用例表逐字一致（13=13=13、顺序一致、序号唯一）；FR-01-119/D81/v1.54/v1.58/QA-RC-01–13 交叉引用齐全；EZR_HOME 口径统一、无 EZR_DIR 使用 | 本轮 python 核对脚本（对账明细见 plan.md 步骤 9） |
| EZR_DIR 全仓核查 | 0 处 `EZR_DIR`；`EZR_HOME` 既有口径未动 | `rg -n "EZR_DIR\|EZR_HOME" <repo>` |
| 既有验证基线 | 本轮**零产品源码改动**（src/ 未触碰），上轮 bins 1152P + 集成 8482P + 属性 90P 基线继续有效 | `git -C project/ezr status --short src`（空输出即零改动） |

### 对账口径
- **产品源码零改动**：本轮为规格轮（specifier 职责边界），`project/ezr/src/` 零改动；唯一产品树内新增 = features/12-reload-config.feature 与 qa/12-reload-config-qa.md 两个新文件（mission/phase/notes 为需求与经验文档）。
- **场景计数**：feature 13 场景 = QA 规程 13 用例（QA-RC-01–13 一一对应，无 S 预登记——S 仅在 QA 轮实跑时按环境判据产生）。
- **数字口径**：场景数/用例数为估计值，QA 轮实跑如有出入以实测为准（engineering.md 通则）。

### 待办与待批（未决项）
- **D81①–⑤ 五项微口径待操作者批（可回退）**——本轮 Inversion 阶段 1 受限退化（chat 选择题面板不可用）沿八轮先例以操作者指令文本 + ezr-reload-cfg.md 为阶段 1 答案，残余需求微口径整理为选择题清单并按源裁决登记（选项与裁决依据全文见 phase-01 D 表 D81 行）：
  - **Q1 配置文件被删除**：A = 按默认配置整体热加载 + 成功 toast（裁决 A，依据 try_load 缺失臂语义）/ B = 拒绝热加载保持旧配置；
  - **Q2 成功 toast 组成**：A = 单条拼接式汇总（裁决 A，方案示例即拼接范式）/ B = 仅固定文案 / C = 多条 toast；
  - **Q3 C 类重启提示触发**：A = 仅实际变化时（裁决 A，防噪音）/ B = 每次成功都提示；
  - **Q4 失败 toast 形态**：A = `⚠` 前缀单行化明细（裁决 A，与操作被拒类 toast 同级）/ B = 错误窗口（D56 扩臂）/ C = `✘` 前缀；
  - **Q5 「代理切换 N」口径**：A = 运行中且端点实际变化任务数（裁决 A，方案钦定「有变化才发命令」）/ B = 全部引用代理任务数。
  - 操作者对任一项有异议时按 v1.54 轮先例出修订轮即可（改判写入 D81 行内注记，规格/QA 随改）。
- **本会话无阻塞项**——方案输入完备（ezr-reload-cfg.md 权威 + 操作者指令），APS 工具链重建成功。

### 移交建议
- **下一步运行 `six-pack/coder`**：按 FR-01-119 实现 Ctrl+R 配置热加载（6 处改动点：keys.rs CONTROL 分支 / App::reload_config / Cmd::ApplyGlobals / EngineShared burst·sleep 原子化 / Task.running_proxy / help.rs 条目——改动面以 ezr-reload-cfg.md 第二节为准，coder 以 FR-01-119 + 12-reload-config.feature 为验收契约）；feature 头部 stamp/manifest 由 mutator 首跑生成；D81 待批项如被改判，coder 开工前以最新 D81 口径为准。
- 归档：请操作者运行 `bin/swarm complete`（产物按操作者指令交付 /home/z/my-project/download/）。

By specifier.
