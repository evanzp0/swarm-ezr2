# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/specifier（裁决落实修订轮，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner 收敛轮 ⇒ architect 收敛轮 ⇒ QA 终验轮 ⇒ specifier 显示指标/调整轮 v1.47–v1.51 ⇒ coder 产品实现轮 ⇒ cleaner/architect/QA 收敛链（2026-11-12/13）⇒ specifier 遗留项裁决/规程对齐/观察项清理轮 ⇒ specifier 新增需求轮（v1.53 五项需求 + ezr-demo 同构落地）⇒ **specifier 裁决落实修订轮（本轮 2026-11-13，v1.54 一项扩面 + 一项细化 + 两项追认 + complete 前指令 ezr-demo HTTP 臂 LT 同构同步清零；下一步建议 six-pack/coder 产品实现轮）**
> 会话：six-pack-specifier-20261113-r2（任务 = 操作者 chat 四条裁决；上游交接 = project/handoff.md 新增需求轮版）
> 环境：nightly（rustc 1.102.0-nightly）@ ~/.cargo/bin + cargo registry rsproxy sparse（沿前轮）；bb 1.13.229 + APS 仓 clone（.tools/）本轮复用跑通三份修订 feature 双检；PMD/llvm-cov 未装机（无度量需求，沿 architect 收敛轮先例）。

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.54**）+ `mission/` 两份 phase 详述（**phase-01 v1.53**、**phase-02 v1.22**）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-13 QA 观察项清理轮） | ezr 产品实现 + 15 套件 e2e + 收敛/终验链 | 已交付 | 历轮对账在案（8060P/0F、e2e 191P/0F 基线） |
| specifier 新增需求轮（2026-11-13） | mission v1.53 + phase-01 v1.52/phase-02 v1.21（FR-01-118、FR-02-37 v1.2/38 v1.21/39/40、D75–D79）+ features 六份修订 + QA 六份同步 + ezr-demo 同构落地（10 源文件 + docs + verify_v153.py）+ notes 三分类 | 已交付 | APS 双检 6/6 全过；demo 单测 58P/0F；PTY verify_v153.py 29 项全过 |
| **specifier 裁决落实修订轮（本轮 2026-11-13）** | **mission v1.54 + phase-01 v1.53 + phase-02 v1.22（FR-02-38 v1.22 LT 扩至 HTTP、FR-02-40/D79① 细化、D77/D78/D79 落定改写）+ features 三份修订（01-tui-display v1.52/09-bt-engine v1.52/01-config-template v1.18）+ QA 三份同步（QA-TD-02/19/28、QA-BE-21/23、QA-CT-01）+ ezr-demo HTTP 臂 LT 同构同步（complete 前指令：ui/list.rs 两态 + popups.rs 帮助 LT/SR 条目 + ui/mod.rs 守卫反转 + docs 设计规范 §5.2/§6.6 + verify_v153.py B6 反转/B7 新增）+ 本 handoff 重写** | 已交付 | **APS 双检 3/3 全过（parser + ir-dry-checker，IR/dry 落 .work/tmp/）；demo 单测 58P/0F、PTY 28 项全过、clippy 零警告** |

### 遗留受限项（当前有效，沿历轮口径）
- **产品端 ezr 未实现 v1.53 五项 + 本轮 v1.22 扩面**（FR-01-118/FR-02-37 v1.2/FR-02-38 v1.21+v1.22/FR-02-39/FR-02-40 + HTTP LT）——**移交下一 coder 轮**；产品源码零改动（历轮 specifier + demo 同构轮口径延续）。
- ~~ezr-demo HTTP 臂「剩余」标签漂移~~ **已清零（操作者 complete 前指令「这个 ezr-demo 也改一下」，本轮同构同步落地）**：ui/list.rs HTTP 信息行两态改 `LT`、帮助 LT 条目改 BT/HTTP 同口径（SR 条目补两臂同构）、ui/mod.rs 守卫测试反转、docs 设计规范 §5.2/§6.6 同步（含 v1.53 轮漏更的 BT 下载臂示例与做种行尾退役字样补齐）、verify_v153.py B6 反转 + B7 新增——单测 58P/0F、PTY 28 项全过、clippy 零警告。
- leech 专属 e2e、变异实跑（hardender 未跑）、6 个 BT/命令窗口族无专属 runner、ws(s) announce 无代理面（沿历轮）。
- ezr-demo verify_ezr.py 预存 FAIL 15 项（本环境缺 pyte 不重跑，沿登记口径）+ verify_phase2_ui.py 预存 FAIL 16 项本轮实测与基线一致（脚本锚点漂移，非产品回归）；demo clippy 本轮实测零警告（优于历轮「mut 警告 5 项」基线）。
- bb/APS 仓在 .tools/ 可复用；PMD/llvm-cov 未装机。

### 实现定义值登记（累计，当前有效口径）
- D44–D74 沿历轮登记不变；D75–D79 **全部落定（操作者 20261113 裁决，否决窗口关闭，不可回退）**：
  - **D75** 任务行 1 文件名折叠口径（32 显示列、三点 ASCII `...`、字符边界不劈半、仅行 1、min(动态预算,32)）——指令钦定为主，本轮未涉及；
  - **D76** 顺序下载切换语义（确认即生效不重置进度、自首个缺口 piece 索引升序、其余机制不变）——本轮未涉及；
  - **D77** SR/LT 作用面（**落定**：①SR 做种/下载两臂同构——操作者追认；②LT = BT 下载臂 + **HTTP 下载任务行**——操作者裁决扩面，v1.53「HTTP 行不变」口径退役；③帮助全称说明，82 行不变）；
  - **D78** seeds/peers 可见性（x2/y2 = tracker 最新成功应答 complete/incomplete、**②括号段省略操作者追认**、`-/-/-` 三段不变、做种臂同构）；
  - **D79** seed_exit_rule 边界（**①细化：share_ratio_gt 正浮点保留 1 位小数**（1.5 = 150%）、seeders_count_gt/seed_duration_m 正整数追认；AND 判定、exit rule 任一键 → default_seed_duration_m 失效、做种时长累计续走、tick 评估、seeders_count_gt 用 x1）。

## 二、当前产出情况

### 本会话产物清单（specifier 裁决落实修订轮）
1. **裁决受理与落实**：操作者 chat 四条裁决 = v1.53 轮合成确认闸门答案（**B 基本准确需局部修改**）+ D75–D79 否决窗口关闭——①LT 扩至 HTTP 下载任务（http 和 BT 下载任务都是「剩余时间→LT」同款改动）；②share_ratio_gt 允许小数保留 1 位小数、seeders_count_gt/seed_duration_m 正整数；③SR 作用于做种/下载两臂同构（追认）；④tracker 值未知时括号段省略（显示 x1/y1，qBt total == -1 同款，追认）。无遗留歧义，未触发 Inversion 二阶段。
2. **mission 三件**：`mission.md` v1.54 注记（裁决四条 + 闸门 B 关闭 + demo 同步补记）；`mission/phase-01.md` v1.53 注记（HTTP 信息行剩余→LT）+ FR-01-116① 行内注记（`LT -`）+ FR-01-80 行内注记（下载中 `↓ <speed> LT <eta>`、已暂停 `↓ - LT -`）；`mission/phase-02.md` v1.22 注记 + FR-02-38 v1.22 修订（LT 扩面）+ FR-02-40 行内细化（share_ratio_gt 一位小数）+ D77/D78/D79 三条落定改写（可回退字样退役）。
3. **features 三份修订**：`01-tui-display.feature` v1.52（v1.52 修订注；场景 02 示例表下载中/已暂停两行改 `LT <eta>`/`LT -`；场景 19 帮助 SR/LT 条目文本——SR 补两臂同构追认、LT 补 HTTP 同改说明，行数 82 不变；场景 28 未知尺寸臂 `LT -`）；`09-bt-engine.feature` v1.52（v1.52 修订注；场景 21 标题/正文——HTTP 臂改 LT 注记 + tracker 括号段省略追认；场景 23 Given share_ratio_gt 保留 1 位小数细化 + 场景注释；场景索引两行同步）；`01-config-template.feature` v1.18（v1.18 修订注；模板尾部 [bt.seed_exit_rule] 注释块 share_ratio_gt 两行注释补「保留 1 位小数」——**模板仍 207 行、键面/活跃行集不变**；场景 01 取值范围表行同步）。
4. **QA 三份同步**：`01-tui-display-qa.md`（v1.52 同步注；QA-TD-02 LT 两态判据、QA-TD-19 条目文本同步、QA-TD-28 `LT -`）；`09-bt-engine-qa.md`（v1.52 同步注；QA-BE-21 ①追认/④HTTP LT 判据 + ①内 LT 括注修正；QA-BE-23 share_ratio_gt 一位小数/正整数追认）；`01-config-template-qa.md`（v1.18 同步注；QA-CT-01 判据补「保留 1 位小数」注释锚点）。
5. **ezr-demo HTTP 臂 LT 同构同步（操作者 complete 前指令，漂移清零）**：`src/ui/list.rs`（HTTP 信息行两态「剩余」→ `LT`：下载中 `  LT <eta>`、其余 `  LT -`）；`src/ui/popups.rs`（帮助【任务列表】组 LT 条目改 BT/HTTP 同口径、SR 条目补做种/下载两臂同构，行数 83 不变）；`src/ui/mod.rs`（守卫测试 http_task_rows 反转：` LT ` 在位且无「剩余」）；`docs/ezr-demo UI界面及交互设计规范.md`（头部 v1.54 同步补记块 + §5.2 HTTP 臂版式/BT 下载臂示例 `8(14)/23(31)/1.675 SR 0.08  LT 17:00` 补齐 + 做种行尾「剩余做种」退役字样清理 + §6.6 帮助条目注记）；`scripts/verify_v153.py`（B 段标题补 v1.22、B6 判据反转为 HTTP 行 = LT、新增 B7 全屏无「剩余」标签）。
6. **本 handoff.md** 重写为裁决轮 + demo 同步口径。

### 验证证据（命令可复现）
| 验证项 | 结果 | 命令 |
|---|---|---|
| APS 双检（三份修订 feature） | **3/3 全过**（parser exit 0 + ir-dry-checker exit 0；IR/dry 落 .work/tmp/01-tui-display*、09-bt-engine*、01-config-template*） | `(cd .tools/Acceptance-Pipeline-Specification && ../../.tools/bb/bb gherkin-parser <feature> ../../.work/tmp/<out>.json)` + `gherkin-ir-dry-checker <json> <report>` |
| demo 单测 | **58P/0F**（含反转后 HTTP LT 守卫 http_task_rows_move_total_to_title_and_drop_pair） | `(cd ezr-demo && cargo test)` |
| demo PTY 验证 verify_v153.py | **28 项全过**（A5/C3/B7（B1–B7 含反转 B6 + 新增 B7 全屏无「剩余」）/D10/E3） | `(cd ezr-demo && cargo build && python3 scripts/verify_v153.py)` |
| demo clippy | **零警告**（优于历轮「mut 警告 5 项」基线） | `(cd ezr-demo && cargo clippy)` |
| verify_phase2_ui.py | 16 FAIL 与登记基线一致（脚本锚点漂移，非本轮回归）；verify_ezr.py 本环境缺 pyte 不重跑（沿登记口径） | `(cd ezr-demo && python3 scripts/verify_phase2_ui.py)` |
| 残留口径一致性核查 | 现行条款零「HTTP 行不变」残留（命中均为历史注记或退役声明） | `grep -rn "HTTP 行不变\|「剩余」标签不变" project/`（逐条人工判定归属） |
| notes 沉淀核查 | 本轮纯规格裁决落实 + demo 同构同步，无新踩坑；`project/notes/` 零新增（无去重风险） | `notes/INDEX.md` 四文件口径不变 |

### 对账口径
- 本轮改动面 = mission 三件 + features 三份 + qa 三份 + **ezr-demo 同构同步五文件（list.rs/popups.rs/mod.rs/docs 设计规范/verify_v153.py）** + handoff；产品端 project/ezr 零改动（cargo test 基线 8060P/0F 不重跑——源码未动，沿前轮口径）；demo 单测 58P/0F、PTY 28 项全过、clippy 零警告（本轮实测）。
- 帮助行数口径不变：规格锚点 82 行（v1.51 起）、demo 源码实测 83 行（既有 1 行漂移沿 v1.44 起「脚本锚点漂移」族登记）——本轮 SR/LT 条目仅文本修订、行数两端均不变。
- 做种统计行现行口径（产品端待实现）= `↑ <上传速度>  <x1(x2)/y1(y2)>/<健康度> SR <分享率>`；BT 下载臂 = `↓ <速度> ↑ <上传>  <x1(x2)/y1(y2)>/<健康度> SR <分享率>  LT <eta>`；**HTTP 行 = `↓ <速度> LT <eta>`（v1.22 扩面；demo 已同构落地，两端一致）**。
- config [bt] 表：`seed_duration_secs` 按未知键口径废弃；`default_seed_duration_m`（分钟，默认 30）+ 子表 `[bt.seed_exit_rule]`（三键可选 AND；share_ratio_gt 正浮点保留 1 位小数、另两键正整数）；模板 207 行（v1.18 注释文本级改动，行数不变）。

### 待办与待批
- 产品端实现移交下一 coder 轮：FR-01-118/FR-02-37 v1.2/FR-02-38 v1.21+v1.22/FR-02-39/FR-02-40 + **HTTP LT 扩面（v1.22）**——demo 已全量同构（含 HTTP 臂 LT），可作实现参照。
- 变异实跑维持移交 hardender（受限项）。
- 本轮无阻塞项、无待批项（裁决已全部落实，D75–D79 落定不可回退；demo 漂移已清零）。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留；归档包按操作者指令交付 `/home/z/my-project/download/`）。
- 下一节点 = `bin/swarm begin six-pack/coder`（产品端实现五项 + HTTP LT 扩面，以 project/handoff.md 为上游交接内容；demo 已全量同构可作参照）。
