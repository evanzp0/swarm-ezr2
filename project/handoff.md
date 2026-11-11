# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/coder（v1.47–v1.50 产品实现补偿轮已完成，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner 收敛轮 ⇒ architect 收敛轮 ⇒ QA 终验轮 ⇒ specifier 显示指标/调整轮 v1.48–v1.51（2026-11-10/11）⇒ coder 产品实现轮（2026-11-11，FR-01-117 ezr 产品端落地）⇒ **coder 产品实现补偿轮（本轮 2026-11-11，v1.47–v1.50 六项欠账 ezr 产品端落地）** ⇒（移交建议见第二节）
> 会话：six-pack-coder-20261111-legacy-metrics（任务 = fr-02-36-38-legacy-display-metrics；上游交接 = project/handoff.md 上一版 + 操作者 chat 指令「把没实现的 spec 都应该实现」——即 mission.md v1.47–v1.50 各轮注记中「产品端 ezr 实现归下一 coder 轮」的六项欠账；上轮 coder 会话任务单（fr-01-117）未含该六项，故滞留，本轮操作者显式扩单承接）
> 环境：stable 1.99.1 + nightly 1.102.0（rustup 1.29.3 前台装齐，镜像复测 rsproxy 18.3MB/s 最快）+ cargo registry rsproxy sparse + bb 1.13.229 / APS（.tools，工具缓存不进归档）

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.51**）+ `mission/` 两份 phase 详述（**phase-01 v1.50** 为当前页签交互与 FR-01-117 依据、**phase-02 v1.20** 为当前 BT 列表行布局与显示指标依据）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-10 QA 终验轮） | ezr 产品实现 + 15 套件 e2e + 引擎拆分 + 终验 | 已交付 | QA 终验全绿（上轮 handoff 口径在案） |
| specifier 显示指标轮（2026-11-10） | phase-02 v1.17 + mission.md v1.47 + features 2 份 + qa 2 份 + ezr-demo 同构修改 | 已交付 | APS 2/2 exit=0；ezr-demo cargo test 45P/0F；PTY 三项显示形态 |
| specifier 调整轮 v1.48（2026-11-11） | phase-02 v1.18 + mission.md v1.48 + features 2 份 + qa 2 份 + ezr-demo 修改（BT 尺寸对上移/协议列切片渲染） | 已交付 | APS dry-check 2/2；cargo test 51P/0F；PTY 90–190 列全宽协议列对齐 |
| specifier 调整轮 v1.49（2026-11-11） | phase-02 v1.19 + mission.md v1.49 + features 2 份 + qa 3 份 + ezr-demo 修改（全类型尺寸上移/切页签自愈/verify_size_move.py） | 已交付 | APS dry-check exit=0；cargo test 54P/0F；PTY 39 项全过 |
| specifier 调整轮 v1.50（2026-11-11） | phase-02 v1.20 + mission.md v1.50 + features/09 v1.50 + qa/09 v1.50 + ezr-demo 修改（做种臂去已上传对） | 已交付 | APS 2/2 exit=0；cargo test 54P/0F；PTY 40 项全过 |
| specifier 调整轮 v1.51（2026-11-11） | phase-01 v1.50（FR-01-117 立项）+ mission.md v1.51 + features/01-tui-display v1.50 + qa/01-tui-display-qa v1.50 + ezr-demo 修改（TabListState/set_filter 三路径收口/守卫三守卫/verify 场景 E） | 已交付 | ezr-demo cargo test 56P/0F；PTY 53 项全过 |
| coder 产品实现轮（2026-11-11，FR-01-117 ezr 落地） | project/ezr 源码 6 文件（TabListState/tab_states/set_filter 单入口/三路径收口/draw_list 自愈）+ 守卫三守卫 + notes v1.52 节 | 已交付 | cargo build --release 零警告；cargo test 全量 7930P/0F；clippy 0 警告；fmt clean；APS parser+dry-check 双 exit=0 |
| **coder 产品实现补偿轮（本轮 2026-11-11，v1.47–v1.50 六项欠账 ezr 落地）** | **project/ezr 源码 9 文件（引擎计算 2 + 数据链 4 + UI 3）+ 守卫 10 条** | 已交付 | **cargo build --release 零警告；cargo test 全量 7990P/0F；clippy 0 警告；fmt clean；APS parser+dry-check 三份双 exit=0** |

### 遗留受限项（当前有效）
- 历轮受限项沿 QA 终验轮口径不变（leech 专属 e2e、变异实跑、6 个 BT/命令窗口族无专属 runner、CRAP 观察项、ws(s) announce 无代理面、bt_dialog 变异点 119、CPD 残余克隆家族）。
- **ezr-demo verify_ezr.py 预存 FAIL 15 项维持（脚本锚点漂移，非产品回归）**；本轮 ezr-demo 零改动（demo 已由 specifier 五轮同构落地，本轮以 demo 源码为参照实现产品端）。
- QA-TD-07/08 的 PTY 帧对实证（qa/01-tui-display-qa.md v1.50 判据）与 v1.47–v1.50 显示形态的 PTY 实证（qa/09-bt-engine-qa v1.50 判据）属 QA 轮工作面，本轮以单元守卫落地（详见下文守卫清单），PTY 实证待下一 QA/specifier 轮按规程执行。

### 实现定义值登记（累计，当前有效口径）
- **D44–D74 沿历轮登记不变，本轮无新增 D 值**。
- 本轮两个补充裁决（沿 D74 既登记口径的落地方向细化，非新 D 值）：①distributed_copies 计数的「连接」= 已握手连接（位图未知不计入，与并发 panel 显示口径一致）；②availability 为瞬时显示指标不入 TaskSnapshot（恢复后首帧 BtConns 事件即刷新）。

## 二、当前产出情况

### 本会话产物清单（coder 产品实现补偿轮——v1.47–v1.50 六项欠账 ezr 产品端落地）
1. **src/engine/mod.rs**：`Evt::BtConns` 载荷新增 `availability: f64`（种子健康度，与并发明细同帧产生随 tick 同节拍刷新）；`BtConnView` 新增 `relevance: f64`（FR-02-36）。
2. **src/bt/engine/conn.rs**：`conn_views` 返回 `(Vec<BtConnView>, f64)`——逐连接计算 relevance =（对端有 ∧ 本端缺失 piece 数）÷（本端缺失 piece 总数），本端无缺失恒 0；新增 `distributed_copies`（FR-02-37/D74③：每 piece 持有者计数 = 连接的非种子 peer 持有记 1 + 本端已有记 1，健康度 = 计数最小值 + 连接种子数 +（计数超出最小值的 piece 数 ÷ 总 piece 数）；连接 = 已握手）。
3. **src/bt/engine/mod.rs**：6 处 `conn_views` 调用点解构元组并随 `Evt::BtConns` 发送 availability。
4. **src/app/mod.rs**：`BtConnRow` 新增 `relevance: f64`。
5. **src/model/task.rs / registry.rs**：`Task` 新增 `availability: f64`（new_queued/快照恢复均初始化 0；不入 TaskSnapshot——瞬时指标）。
6. **src/app/engine/evt.rs**：`Evt::BtConns` 消费处更新 `t.availability` + 映射 `BtConnRow.relevance`。
7. **src/ui/text.rs**：新增 `slice_by_width`（按显示宽度切片，跨界字符空格补齐）与 `slice_spans`（整行 spans 按滚动偏移切片、逐 span 保样式、可见宽度不足空格补齐——v1.48 ② 口径）+ 守卫 3 条（窗口宽度不变性/表头数据任意偏移对齐/跨界补齐）。
8. **src/ui/conns.rs**（FR-02-33 v1.17 + FR-02-36 + v1.48 ②）：BT 八列改**九列** `BT_COLW = [18,4,12,12,10,10,34,7,8]`、「客户端」与「协议」之间插入「相关度」列（数据宽 7、表头 `head_cell_r` 右对齐、数据 `pad_left` 右对齐、`{:.1}%` 1 位小数、前景 FG）；内容宽 115→**123**、客户端收缩基线 81→**89**（max(26, vw − 89)）；水平滚动渲染改为**表头行与数据行统一 `slice_spans` 预切片**（`Paragraph` 不再用 `.scroll` 逐行独立截断，任意偏移下逐列恒对齐）；吸附逻辑 D63④ 不变。
9. **src/ui/task_lines.rs**（FR-02-37/38 v1.18/19/20 + 01-tui-display v1.49 场景 02/28）：①任务行标题尺寸项——`title_size_txt`（size_known 即在「[状态]」徽标后单空格显示 `<总字节>`，未知不显示），名称预算改预留 2 列（demo 同构）；②做种臂 `stat_seeding` = `↑ <上传>  <s>/<p>/<健康度:.3> 分享 <r>  剩余做种`（三段 MAGENTA 同色），`已上传 <x>/<总>` 整项退役（v1.50）；③BT 其余状态臂 `stat_bt_rest` = 下载中三段 FG + 非下载中 `-/-/-`，两臂均接 ` 分享 <r>`（单空格衔接 D74④）；④`share_ratio_text`（qBt realRatio 同源：1% 兜底、分母 0 且上传>0 显 ∞、均 0 显 0.00）+ `share_spans`（「分享」DIM + 数字 MAGENTA）；⑤明细统计行尺寸项整项退役：已完成尾 ` · total`、失败尾 ` · x/y`、等待尾 ` · x/y|未知`、HTTP `  x/y` 与 `? B` 形态全部上移退役（`dl_total_txt` 删除；`fmt_size_pair` 退役为 cfg(test) 仅余 property_tests 不变量引用）；⑥HTTP 臂 = `↓ <速度>  剩余 <eta>`。
10. **守卫测试 10 条**（TDD）：引擎 4（relevance 交集比/做种恒 0/distributed_copies D74 公式/握手前连接排除+本端持有计入）+ UI 6（行 1 全类型全状态尺寸项/分享率三边界/分享 spans 配色/做种臂 v1.50 退役/slice 窗口不变性/slice 任意偏移对齐+跨界补齐）+ 既有断言同步（conns 7 个视口/列宽测试按 v1.17 基线 89/内容宽 123 平移；task_lines 6 个统计行测试按退役口径改写）。
11. **本 handoff.md** 重写为当前有效口径。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 构建 | release 零错误零警告（触碰文件强制重编核证） | `cd project/ezr && cargo build --release` |
| 单元测试（bin ezr） | **1016P / 0F**（1006 基线 + 10 新守卫净增：11 新增 − 1 改名 stat_seeding_speeds_and_peers→stat_seeding_speeds_peers_health_share，挂载对账精确） | `cargo test --bin ezr` |
| 其余 bin 目标 | ezr-proxy 15P / ezr-fixture 30P / ezr-mktorrent 1P（与基线持平） | `cargo test --bin <目标>` |
| 集成测试（12 目标） | **6928P / 0F**（hardening 247 + engine_backfill 908 + g_appcore 797 + g_engine 793 + g_main 2 + g_proxy 16 + g_slots 254 + g_supervisor 766 + g_throttle_err 496 + g_v147 780 + ui_backfill 938 + v115 931） | `cargo test --test <目标>` 逐目标（磁盘受限须顺序跑） |
| 集成增量对账 | 6878 → 6928（+50）：新守卫经集成目标镜像挂载——引擎守卫 4 × 5 目标（g_appcore/g_engine/g_supervisor/g_throttle_err/g_v147）= 20，UI 守卫 10 × 3 目标（engine_backfill/ui_backfill/v115）= 30；既有集成断言零删除零改名 | 分项目复算 |
| 全量合计 | **7990P / 0F** | 分项复算：1016+15+30+1+6928 |
| Lint | clippy 0 警告（触碰文件强制重编译核证）；cargo fmt --check clean | `cargo clippy --bin ezr`、`cargo fmt --check` |
| APS 复验 | gherkin-parser + gherkin-ir-dry-checker 对 features/09-bt-engine、01-tui-display、04-ui-conns 三份双 exit=0 | `.tools/Acceptance-Pipeline-Specification` 目录内 `../bin/bb gherkin-parser <feature> <out>` |

### 对账口径
- 产品端改动面 = **9 源文件**（engine/mod.rs、bt/engine/conn.rs、bt/engine/mod.rs、app/mod.rs、model/task.rs、model/registry.rs、app/engine/evt.rs、ui/text.rs、ui/conns.rs、ui/task_lines.rs——其中 ui/task_lines.rs 与 ui/conns.rs 为布局主体；另 cfg(test) 构造点补字段：app/engine/bt.rs、app/keys.rs、app/mouse.rs、model/mod.rs、ui/pieces.rs）；ezr-demo **零改动**（本轮以其源码为同构参照）。
- bin ezr 单元 1006 → 1016（+10 净增守卫，无既有守卫删除，改名 1 条）；集成 12 目标 6878 → 6928（+50 镜像挂载，见上表）。
- 需求对账（mission.md v1.47–v1.50 六项欠账逐项）：**①FR-02-36 相关度列**（九列/列宽 7/表头数据右对齐/1 位小数/内容宽 123/收缩基线 89/做种恒 0/握手前不显示）✅；**②FR-02-37 健康度**（三段 `<s>/<p>/<x.xxx>`/做种同形态 MAGENTA/非活跃 `-/-/-`/D74 公式）✅；**③FR-02-38 分享率**（单空格衔接健康度项/2 位小数/数字 MAGENTA 标签 DIM/∞ 与 0.00 与 1% 兜底/D74⑤ 其余两臂作用面/HTTP 不追加）✅；**④v1.48 ① 尺寸对上移 BT 臂**（被 ⑤ 扩面覆盖）✅；**⑤v1.49 全类型明细尺寸项上移**（任务行徽标后总字节全类型全状态/明细行四形态退役/名称预算 2 列预留）✅；**⑥v1.50 做种已上传对退役**（分享率衔接改健康度项）✅；**⑦v1.48 ② 协议列左对齐 + slice_spans 切片渲染**（产品端规避逐行独立截断）✅。与 ezr-demo v1.50/v1.51 实现同构（conns 九列与 slice、task_lines 各臂逐臂核证）。
- 上轮 FR-01-117（v1.51 页签状态保留）产物在本轮验证下全绿维持，无回归。

### 待办与待批
- 本轮无操作者待批事项；D74 微口径的落地方向细化两条已登记（见上）。
- 产物交付三件套（操作者 chat 指令）：`bin/swarm complete` 归档包 + ezr-demo 源码归档（不含 target/）+ ezr release 二进制 → 复制 /home/z/my-project/（归档即本轮任务终点，操作者执行）。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留），并按上文把三件产物复制到 /home/z/my-project/。
- 续跑建议：`six-pack/qa`（PTY 实证 QA-BE-10/13/21 v1.50 + QA-UC-08 v1.47 + QA-TD-02/28 v1.49 + QA-TD-07/08 v1.50，规格与产品端均已就绪）——由操作者显式决定。
