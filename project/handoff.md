# 交接（handoff）

> 流程：six-pack　当前节点：**six-pack/specifier（v1.64 裁决落实轮：phase-05 §5 ttp] 修复 + D92 矛盾落地 + HTTP 并发列表「信息」列 D94 登记）——已完成**
> 路径：**six-pack/specifier（本轮）** → coder → cleaner → architect → (hardender) → QA
> 会话：six-pack-specifier-v164（上游交接 = coder D90–D93 落地轮 handoff（规格层缺陷 2 处上报 + coder 侧同步面）+ 操作者 20261127 chat 裁决：「①mission/phase-05.md §5 示例 toml 块表头行损坏成 ttp] 修复应为 [http]；②D92 修订注与钦定原文自相矛盾——删除 config.toml 中关于『仅 dynamic 模式适用』和『仅 blocks 回退模式适用』的注释，因为 EZR 只有 dynamic 模式；③增加需求——http 下载任务的并发 panel 中的每个并发列表最后一列增加『信息』列，内容参考 misterdownloadmanager（github.com/anefzaoui/misterdownloadmanager）http 下载明细窗口并发列表『信息』列」）
> 环境：bb 1.13.230（`.tools/bb-bin/bb`，PATH 注入）+ APS 仓 `.tools/Acceptance-Pipeline-Specification` 本轮重建；MDM 参考实现 `.tools/misterdownloadmanager` 本轮克隆（语义核证源，工作区纪律内只读）；nightly 工具链未重建（本轮零代码编辑）；~/.cargo 与 .tools/ 非持久沿袭（bb/APS/MDM 重建口径见 notes/environment.md）
> 受限项：①chat 选择题面板不可用（IM 通道）沿袭——Inversion 受限退化沿 v1.45–v1.63 先例，操作者裁决文本即阶段 1 答案；②runner 不可执行沿袭（qa/runners 归 QA 轮同步）；③MDM 性能基准 = 操作者线下验收口径；④P3 增益层沿规格排除；⑤族C hardening_ui_backfill gen 剥头副本 1 例沿袭（归 cleaner）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，v1.64）；phase-05 详述 = `mission/phase-05.md`（FR-05-01–13 + D83–D94，v1.2）；ezr 不做旧版本兼容沿袭（v1.45/FR-02-35）。
- ezr-demo：仓库内独立子项目；本轮零接触。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固、phase-05 规格轮 + 实现轮 + pieces 各修复轮 + 速度回退修复轮 + 族A/族B 裁决落实轮（specifier）+ D90–D93 落地轮（coder） | features 01–13 + qa 规程 + runners + 动态引擎/模板常量/断言面全量落地 | 已交付（沿上轮账） | 当时口径 bin 1256P/0F + hardening 13 目标仅剩族C 1 例 + clippy/fmt/check 全零警告 |
| **本轮：v1.64 裁决落实轮（20261127，specifier）** | **①mission/phase-05.md v1.2：§5 示例 toml 表头 `ttp]` → `[http]`（194 行）+ D91 条款内联同源损坏一并修复（201 行，specifier 字节级扫描补充发现）+ D92 矛盾落地条款化 + FR-05-13 v1.2「信息」列全条款 + D94 登记；②mission.md v1.64（标题版本对齐 + v1.64 注记 + §4 核心参数 block_size_http 行 D92 口径 + 分期索引 phase-05 行退役表述同步）；③features/01-config-template v1.64：钦定模板两注释行整短语删除（行数/键数不变）+ 场景 01 取值范围表同步；④features/04-ui-conns v1.64：场景 08 HTTP 三列改四列（信息列全判据）；⑤QA 三份同步（04-ui-conns/01-config-template/01-persistence-config）；⑥notes 三文件 + INDEX 追加** | 已交付 | **APS parser 2/2 + ir-dry-checker 2/2（findings 与 HEAD 基线持平 12/12、92/92，零新增）；字节级终审（`[h` 计数 + realbad `ttp]` 扫描编辑前后比对）零附带损伤；钦定模板 git diff = 纯 2 行文本替换** |

### 遗留受限项（当前有效）
- **①全仓 >100 变异点存量 / CRAP 100 / CPD 222**：历轮登记沿袭；归 architect/cleaner 轮。
- **②runner 断言同步面（归 QA 轮）**：沿上轮清单（QA-DE-04/05/06/07、QA-HT-12/13b、QA-PC-05 退役行；QA-HT-13、QA-PC-13 键集 41；suite_resume_sidecar.py:211 blocks 断言失效）**本轮新增**：`suite_config_template.py` 解析锚需对 v1.63/v1.64 一并适配（起点锚版本行 + 两注释行新文本——`# 动态分段调度初始切分粒度下限（KiB）` 与 `# HTTP 分块块大小（字节）` + 非空行计数口径：钦定文本 177 = 非空行计数，代码常量 lines() = 211，内嵌块物理行 212）。
- **③族C（gen 剥头副本漂移，1 例）**：hardening_ui_backfill `gen_fixtures_match_product_sources`，比对面 = src/ui/{chart,detail,btn}.rs + src/app/clipboard.rs 四文件，重新生成即可，归 cleaner 轮顺带。
- **④qa/09-bt-engine-qa.md:70 字面真缺陷 1 处**（`ttp] 调优键` 应为 `[http] 调优键`）：归 QA 轮顺带修复（沿袭登记；本轮字节扫描复核确认该处为全仓唯一余留真损坏）。
- **⑤MDM 性能基准**：线下口径（沿袭）。
- **⑥blocks 引擎代码本体仍在编译面**：沿袭（config 面退役后生产不可达）；引擎代码退役/瘦身归 cleaner/architect 轮裁决。
- **⑦D94 可回退窗口**：信息列三项微口径（列宽 18 / 超宽横滚 / MDM 三态映射）沿 D83–D93 先例登记可回退；操作者否决 = 直接以裁决文本 begin six-pack/specifier 改判（feature/QA 判据行 + coder 实现面改判成本低）。

### 实现定义值登记（累计覆盖为最新全量）
- 历轮登记（D81–D93）全部沿袭有效（scheduler 键拒绝语义、钦定模板常量 211 行/41 键按 v1.63 钦定原文——**本轮 v1.64 两注释行文本修订后 coder 侧 AUTHORIZED_TEMPLATE 尚未同步，见待办**、hardening 8 例更名映射）。
- **本轮新增（D94 三项微口径，源裁决可回退）**：
  - **信息列宽 = 18**：极值算法 = 「正在接收数据...」17 显示列 + 1 空白（沿标志列 5/相关度 7 口径）；HTTP 内容宽 28 → 47（4+12+10+18+3 分隔）。
  - **超宽横滚**：120×34 视口 44 < 47 → 水平滚动条 + ←/→ ±2 列（既有通用机制适用 HTTP；上限 = 内容宽 − 视口宽 + 1）；原「HTTP 28 列无横滚」口径退役。
  - **MDM 三态映射**：信息列取值 ezr 三态 = `正在连接...` / `正在接收数据...` / `正在重试：<原因>`（原因 = 既有失败分类 reason 文案族）；MDM Finished（空）/Failed（错误：…）/Retired（<host> 允许 <n> 个连接）无 ezr 对应臂（活跃口径行不显示 / 失败升级任务级 / 上限学习属 P3）。

## 二、当前产出情况

### 本会话产物清单（v1.64 裁决落实轮）
1. **`mission/phase-05.md` v1.2**：①§5 示例 toml 表头行 194 `ttp]` → `[http]`（操作者裁决①）；②D91 条款内联 201 `` `ttp]` `` → `` `[http]` ``（同源损坏，specifier 字节级扫描补充发现并一并修复）；③头部 v1.2 修订注；④FR-05-13 v1.2 =「信息」列全条款（四列形态/三态取值/MDM 映射/超宽横滚）；⑤D92 行追记矛盾落地、D94 行登记。
2. **`mission.md` v1.64**：标题版本 (v1.62) → (v1.64)（v1.63 轮漏更顺带对齐）+ v1.64 注记 + §4 核心参数 block_size_http 行 D92 口径修订（「仅 blocks 回退模式适用…由调度器取代」表述退役）+ 分期索引 phase-05 行退役表述同步（scheduler 键退役 D91 + 信息列 D94）。
3. **`features/01-config-template.feature` v1.64**：钦定模板两注释行整短语删除（`# 动态分段调度初始切分粒度下限（KiB）`、`# HTTP 分块块大小（字节）`；行内文本修订、行数/键数不变）+ 场景 01 取值范围表 min_split_kib/block_size_http 行同步 + v1.64 修订注（历史注记 v1.51/v1.63 原样保留不改写）。
4. **`features/04-ui-conns.feature` v1.64**：场景 08 标题与清单行更新（HTTP 四列结构）+ 表头/信息列取值/超宽横滚三条判据 + 收缩条款尾部修订 + v1.64 修订注。
5. **QA 三份同步**：`qa/04-ui-conns-qa.md`（v1.64 同步注 + QA-UC-08 增判据⑫⑬）；`qa/01-config-template-qa.md`（v1.64 同步注 + 环境前置基线 180 → 177 行口径修正）；`qa/01-persistence-config-qa.md`（v1.64 同步注 + QA-PC-13 min_split_kib 判据项改判）。
6. **`project/notes/`**：environment.md（GitHub release 断点续传 / 通道面逐通道探针 / 194+201 修复登记 / bb 落位）+ architecture.md（MDM 信息列语义源核证路径）+ testing.md（dry-checker findings 提示级与 HEAD 基线对比法）+ INDEX.md 三行同步。
7. **`.tools/`（运行时资产，不进归档）**：bb-bin（bb 1.13.230）+ Acceptance-Pipeline-Specification 克隆 + misterdownloadmanager 克隆。

### 验证证据
| 验证项 | 结果 | 命令（cwd = swarm 根，前台串行） |
|---|---|---|
| APS gherkin-parser（两份修订 feature） | **2/2 OK** | `(cd .tools/Acceptance-Pipeline-Specification && PATH=$PWD/../bb-bin:$PATH bb gherkin-parser <abs>.feature .work/tmp/<stem>.json)` |
| APS gherkin-ir-dry-checker（两份 IR） | **2/2 OK**（exit 0；findings 12/12、92/92 与 HEAD 基线持平零新增） | `(cd .tools/Acceptance-Pipeline-Specification && … bb gherkin-ir-dry-checker <ir> <out>.dry.json)`；基线对比 = `git show HEAD:<feature>` 同管道复跑 |
| ttp] 字节级终审 | **phase-05 realbad 归零（194/201 已修复）**；全仓 realbad 增量 = 本轮注记刻意描述（mission 3 + phase-05 3，与文本一一对应）；qa/09-bt-engine-qa.md:70 为唯一余留真缺陷（登记归 QA 轮） | `python3 .work/tmp/ttp_baseline.py`（编辑前后各一次，落 .work/tmp/ttp_baseline_{before,after}.txt） |
| 钦定模板完整性 | **纯 2 行文本替换**（git diff 确认，行数/键数/空行零变化） | `git diff -U1 -- project/features/01-config-template.feature` |
| Write 通道探针 | `[h` 序数 `91 104 116 116 112 93` 完整落盘（Write/Edit 不吞，仅 Bash 输出层吞） | `.work/tmp/probe_h.txt` 序数回读 |

### 对账口径
- ttp] 真损坏磁盘面 = **2 处**（phase-05.md:194 独立行 + :201 内联），均修复；coder 上报的「194 行」准确，201 为本轮同源扫描补充发现（同一 v1.1 编辑引入）。
- 其余各文档所见 `ttp]` = 终端显示伪象或 notes 刻意描述（判别法 notes/environment.md 已载），字节面完好，未改动——「别凭显示断定损坏，更别顺手修复把好文件改坏」条款全程生效。
- 版本链：mission.md v1.63 → v1.64；phase-05.md v1.1 → v1.2；两 feature/QA 文档头注对齐 v1.64；mission.md 标题 (v1.62) → (v1.64)（v1.63 轮漏更顺带对齐，D92 落地属其裁决延伸）。
- dry-checker findings 为提示级（near-duplicate/possible-synonym，exit 0），本轮零新增；既有 92 项（04-ui-conns）为历轮存量。

### 待办与待批（未决项）
- **D94 可回退窗口**：三项微口径（列宽 18 / 超宽横滚 / MDM 三态映射）沿 D83–D93 先例登记，操作者否决窗口开放；否决 = 直接以裁决文本 `bin/swarm begin six-pack/specifier`（移交建议沿袭）。
- **coder 轮接手面（产品端实现归下一 coder 轮）**：①信息列 UI 四列形态（src/ui/conns.rs HTTP 臂 + 引擎 per-conn 状态快照——Connecting/在传/原地重试三态暴露，参照 MDM ConnView {id, downloaded, status} 同构形态，语义源核证路径见 notes/architecture.md v1.64 节）；②AUTHORIZED_TEMPLATE 两注释行同步（v1.64 钦定原文逐字节，1 行代码面 ×2）；③config 测试面如有模板字节断言同步。
- **QA 轮接手面**：遗留受限项② 全清单（+ suite_config_template.py 解析锚 v1.63/v1.64 一并适配）+ ④。
- **族C gen 剥头副本重新生成**：归 cleaner 轮顺带。**blocks 引擎代码本体退役/瘦身**：归 cleaner/architect 轮裁决。**变异/CRAP/DRY 度量**：沿袭，归 cleaner/architect 轮。**QA-HT-10 du 判据 / 性能基准**：沿袭，线下/操作者裁决。

### 移交建议
- 下一步 `bin/swarm begin six-pack/coder`：D94 实现轮（信息列四列形态 + 模板常量两行同步 + 测试面）——改动面小（conns.rs HTTP 臂 + 引擎状态快照 + config.rs 2 行），无大审计需求。
- QA 轮接手时：QA-UC-08 v1.64 增判据⑫⑬ 纳入回归口径（fixture 慢速门控观察 `正在接收数据...`、指定连接注入断连观察 `正在重试：传输中断（…）`）；`suite_config_template.py` 解析锚适配；qa/09-bt-engine-qa.md:70 顺带修复；最小回归集 = `cargo test --bin ezr` + hardening_g_engine/g_v147/v115。
- 操作者对 D94 任一微口径改判：直接以裁决文本 `bin/swarm begin six-pack/specifier`（三层同步口径已建立，改判成本低）。

By specifier.
