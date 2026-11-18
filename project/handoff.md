# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（规格分歧两处裁决落实轮）
> 路径：…hardender 修订轮 ⇒ QA 轮 ⇒ **specifier 裁决落实轮（本轮，操作者指令：用 six-pack/specifier 完成 handoff.md 待办——规格分歧登记两处的裁决落地）**
> 会话：six-pack-specifier-20261118（SESSION_TASK = 规格分歧两处裁决落地；上游交接 = 上轮 handoff）
> 环境：rustup stable 1.100（rsproxy 镜像前台安装）、pyte/wcwidth/libtorrent 2.1.1/bencodepy（pip）、
> Java 21 + PMD 7.29.0（.tools/）；ezr 二进制 target/debug 沿 QA 轮全量重建（本轮零代码改动）。
> APS 工具未装（.tools/ 无 Acceptance-Pipeline-Specification）——本轮为文档修订，dry-check 未跑（受限项）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.57**）。ezr 不做旧版本兼容。
- ezr-demo：仓库内独立子项目（演示口径保留「新增 BT 任务未手输 trackers 预置 4 条默认 tracker」，与产品行为有意不同，见 11-trackers-window v1.14 注记澄清）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| hardender 修订轮 | 裁决落地（② 产品修正 + ①③ 规格修订 + ④/mktorrent 了结） | 已交付 | 修订重验 2 feature FULL：77 = 70 killed + 7 等价类 + 0 errors；bin 1090P |
| QA 轮 | 五规程缺口可执行化：06/07/09/10/11 五套件新增（58 用例登记）+ harness 退出礼仪修复 + 产品一处 bug 修复（normalize_save_dir_with `./` 分量归一） | 已交付 | 06 = 8P/0F；07 = 11P/1S（AB-07 退役钦定）；11 = 6P/0F；09 = 17P/7S；10 = 1P/7S（S 均内嵌 fixture 能力判据）；产品单测 1090P/0F、fmt/clippy clean、CRAP/DRY 历轮口径 |
| **specifier 裁决落实轮（本轮）** | **规格分歧两处操作者裁决落地（下节详列）**：① QA-AB-11④ 文案改产品实际行为（规程 + feature 三层同步）；② FR-02-21 v1.25 补「udp 必须带端口」 | 已交付 | rg 残留检索清零（现行口径）；三层口径对齐核对（feature/QA/mission）；产品零代码改动、1090P/0F 基线不受影响 |

### 待办承接（历轮待办 → 本轮处置）
- **规格分歧登记两处（QA 轮登记，待操作者裁决）**：✅ **两处均已裁决并落地（本轮主体）**——
  1. 「留空 trackers 默认 4 条入库」：操作者裁决**修订规程该臂** → QA-AB-11④ 文案改产品实际行为（qa/07-add-bt-qa.md v1.52 + features/07-add-bt.feature 场景 11 留空臂 v1.52 同步 + 11-trackers-window v1.14 注记澄清）；
  2. udp URL 无端口拒绝启动：操作者裁决**补完备规格** → FR-02-21 v1.25 补「udp 必须带端口」（mission/phase-02.md + features/09-bt-engine v1.55 注记追踪 + mission.md v1.57）。
- **mutator 等价突变过滤器扩展（上游字节冻结，操作者裁决面）**：沿历轮登记不变——上游 bb/脚本字节冻结未解，QA 角色不可动，维持登记。

### 遗留受限项（当前有效）
- 缺口账目 ID 不可精确复现 / trackpool.rs:644 等价论证 / 2 条 unviable / 软 Gherkin 等价类 17 格 / RB-09+UC-10 / mktorrent bencode：**沿历轮登记不变**。
- QA 轮 S 登记（fixture 能力判据）：QA-BE-04/07/17/18/19/20/24、QA-AL-01–05/07/08——全部内嵌环境条件，沿「S 必须内嵌环境判据」条款，非预算性弃测。
- **本轮新增（文档面，非行为面）**：07-add-bt.feature 场景 11 内容变更致 mutation manifest/stamp 场景哈希滞后——下次 gherkin 变异运行自刷新（工程规则：清单由变异工具更新，不手工改）；APS 未装，本轮 feature dry-check 未跑。

## 二、当前产出情况

### 本会话产物清单（specifier 裁决落实轮）
1. **分歧① 落地（三层同步，QA-AB-11④ 文案改产品实际行为）**：
   - `project/qa/07-add-bt-qa.md` **v1.52**：QA-AB-11④ 改为「留空 trackers 确认任务创建成功，tracker 池 = 三来源（config trackers/announce-list/手输）合并去重（FR-02-21），留空时池内容 = 种子元数据自带 announce（announce-list 来源），T 弹窗与修改面板预填一致」；原「留空默认 4 条（http/https/udp）入库」为 ezr-demo 演示口径退役（头部 v1.52 同步注登记）。
   - `project/features/07-add-bt.feature` **v1.52**：场景 11 留空臂步骤同步改产品实际行为；场景名「trackers scheme 校验与默认 trackers」→「…与留空 trackers」（稳定序号不变）；场景清单行 + v1.52 修订注。
   - `project/features/11-trackers-window.feature` **v1.14 注记**：v1.10 注记后补澄清——「预置 4 条默认 tracker」仅属 ezr-demo 演示口径，产品无默认注入（注释澄清，场景内容零改动）。
2. **分歧② 落地（FR-02-21 补完备）**：
   - `project/mission/phase-02.md` **v1.25**：头部 v1.25 注记 + FR-02-21 行内修订——**udp 必须带端口**（`udp://<host>:<port>`，port 1–65535；缺端口/端口非法含 0 = 非法 URL）：config 来源 = 启动失败并报错（FR-02-35，config 层逐条校验即拒）；announce-list/手输来源 = 照常进任务 tracker 列表（T 弹窗显示/持久化/修改预填）但不进引擎 announce 池（TrackerUrl::parse 失败丢弃，不 announce、状态列「待 announce」）；对话框 scheme 白名单层不拦截。语义按产品源码核证（tracker.rs `udp_host_port` / config.rs `trackers_val` / bt_dialog.rs scheme 校验 / engine/mod.rs 建池 filter_map / ui/popups.rs 状态臂）。
   - `project/features/09-bt-engine.feature` **v1.55 注记**：FR-02-21 v1.25 追踪注（场景 05/09 内容零改动，Examples udp 行均带端口）。
   - `project/mission.md` **v1.57**：大纲注记（两处裁决落地摘要）+ 标题版本号。
3. **经验沉淀**：`project/notes/architecture.md` 追加 1 条——tracker URL 合法性三层分界（config 层 fail-fast / 引擎建池丢弃 / 显示层原始串入列），规格表述「入池」必须指明层。
4. **本 handoff.md** 重写为本轮口径。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 「默认 4 条」现行口径残留 | 现行口径清零；残留均为历史注记（v1.9.1/v1.43/D70⑥，按惯例保留被新注记覆盖）与本轮退役引述 | `rg -n "默认 4 条|4 条默认|预置 4 条" project/` |
| udp 用例端口形态 | 规格与 QA 文档 udp URL 均带端口（prose 描述除外）；runners 构造 `udp://127.0.0.1:<port>/announce` | `rg -n 'udp://' project/qa project/features` |
| 产品代码 | 零改动（git diff 仅 mission/features/qa/notes 文档面） | `git -C project/ezr status --short`（空）；`git status --short`（列文档面） |
| 产品单测基线 | 沿 QA 轮 1090P/0F（本轮零代码改动，不重跑） | `cd project/ezr && cargo test --bin ezr` |

### 对账口径
- 修订面 6 文件 + notes 1 条与两处裁决一一对应：① = qa/07-add-bt-qa.md + 07-add-bt.feature + 11-trackers-window.feature；② = phase-02.md + 09-bt-engine.feature + mission.md。
- QA-AB-11④ 与 07-add-bt 场景 11 留空臂同口径（三层同步，修订纪律）；suite_add_bt.py QA-AB-11 断言本按产品行为（announce 入池），零改动即合规。
- 语义细化为两层口径（config 层拒启 / 引擎建池丢弃 / 任务 tracker 列表照常显示）系按产品源码核证后的准确化，QA-TW 长 URL 无端口 udp 行显示实证（QA-TW-02 断言行显示 + 状态列无引擎回报口径）与之一致。

### 待办与待批
- **mutator 等价突变过滤器扩展**：沿历轮登记（上游字节冻结，操作者裁决面）——本会话唯一未决项。
- 本会话无新增待批：两处规格分歧已裁决落地结案。
- 说明（非待批，角色边界登记）：qa/runners/suite_add_bt.py QA-AB-11 文档串与断言消息中的旧叙述句（「规程『留空默认 4 条』为 demo 演示口径…已登记 handoff 待批」）随规程修订已滞后——断言行为不受影响；runners 属 QA 触点文件，specifier 不越权修改，QA 下次触点时随手刷新。

### 移交建议
- **six-pack 流转路径完结**：specifier → coder → cleaner → architect → hardender → QA → **Done**（本轮为裁决落实轮，规格与规程已对齐产品实际行为）。
- 如需回归验证规格-套件一致性，可复跑 `python3 project/qa/runners/suite_add_bt.py`（QA-AB-11 判据零改动）。
- 请操作者运行 `bin/swarm complete` 归档（归档包按操作者指令交付 `/home/z/my-project/download/`）。

By specifier.
