# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（终验轮已完成）——**工作流已走完（QA 为末位角色），待操作者 `bin/swarm complete` 归档**
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 修订实现轮 ⇒ coder 补缺口轮（2026-11-10）⇒ cleaner 收敛轮（2026-11-10）⇒ architect 收敛轮（2026-11-10）⇒ **QA 终验轮（本轮，2026-11-10）** ⇒ Done
> 会话：six-pack-QA-20261110（任务 = architect 收敛轮交接的最终独立验证；上游交接 = project/handoff.md architect 收敛轮版；会话因上下文中断按宪法中断恢复口径凭 plan.md 续跑完成）
> 环境：nightly 1.102.0（rustup 1.29.3 前台装齐）+ cargo registry rsproxy sparse + bb 1.13.229 / APS / PMD 7.29.0 / llvm-cov 0.9.1 / mutants 27.1.0（工具缓存不进归档）

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，v1.46）+ `mission/` 4 份 phase 详述（phase-02 v1.16 为当前实现依据）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder 历轮（ws/webrtc + bug28 + 修订 + 补缺口轮） | ezr 产品实现 + 12 集成测试目标 + bin 单测；bug6–bug28 修复；FR-02 全量实现 + D71/D72/D73 落地；ezr-demo 同构同步 | 已交付 | bin + 集成全绿（历轮口径在案） |
| cleaner 历轮 | 保持行为清理 + 度量四件 | 已交付 | 全绿；CPD 198 |
| architect（2026-11-07） | 四阶段评审 + arch_check 17 规则 + 属性测试 76P | 已交付 | 历轮口径在案 |
| hardender | —（操作者 20261109 裁定为可选节点，默认链不含） | 不适用 | — |
| QA + 终验（2026-11-06/07） | 14 套件 187P 实跑 + harness 自适应化 | 已交付 | 全绿 |
| 诊断轮（2026-11-09 下午） | mini_ws_tracker 修复 + Sintel 真实 swarm 129.3MB ×2 全成 + 三方冒烟 | 已交付 | ezr 源码零改动 |
| specifier 修订轮（2026-11-09） | phase-02 v1.16（FR-02-03/19 v1.16 + D73）+ mission.md v1.46 + features 2 份 + qa 2 份同步 | 已交付 | APS parser+dry-check 2/2 exit=0 |
| cleaner 收敛轮（2026-11-10） | CRAP 收敛拆分 ×2 + DRY 单点化 ×3 + demo 守卫修否裁决=否 | 已交付 | bin 997P/0F + 集成 6869P/0F + clippy 0 警告 + fmt 干净 + APS 20/20 |
| architect 收敛轮（2026-11-10） | engine.rs 8477 行目录化拆分（5 文件零下游改动）+ 属性测试 +6（76→82）+ arch_check 第 18 条裁决=否 | 已交付 | bin 1003P/0F + 集成 6869P/0F + clippy 0 警告 + fmt 干净 + arch_check 17 规则 + 变异点 5994 再分布 |
| **QA 终验轮（本轮 2026-11-10）** | 全 15 套件注册用例对账实跑 + QA-MB-03 补齐可执行化 + MB-10 门控修复 + 1 项产品缺陷修复（D73② 上传暂停协议化） | 已交付 | 本轮验证证据见第二节 |

### 遗留受限项（当前有效）
- ~~**leech 判据无 e2e 自动化**~~ → **本轮部分结案**：D73② 上传暂停/恢复的 leecher 端到端拉取面已入 `suite_modify_bt.py`（QA-MB-10 上传恢复臂 leecher 拉完断言，本轮修复后实证）；「leech 判据」（BanKind::Leech 触发面）仍无专属 e2e，维持历史受限口径。
- **变异实跑未执行**（hardender 可选节点；`--file` 分块口径悬置转政策性关闭）。全仓 5994 点在案；engine 拆分后 `--file` 分块消费面就绪（五文件 20/106/78/209/52）。
- **QA 规程 6 个 BT/命令窗口族无专属 runner**（harness/命令链路承载，历轮既定形态）。
- **CRAP 观察项**：`ezr_main` 与 `on_mouse_trackers` bin 套件 0 覆盖（承载面差异登记，历轮维持）。
- **ws(s) tracker announce 无代理面**——操作者 20261109 chat 裁决不补齐，长期已知边界关闭。
- **ezr-demo 守卫 FAIL（cleaner 轮裁决=否，architect 轮维持）**：影响面 = `ezr-demo/scripts/` 10 个 PTY 守卫（环境预存退格投递上限 + 场景基线漂移两类）；行为口径权威裁决归操作者，产品面验证不受影响。
- **CRAP 残余高 comp 登记**：`on_bt_done` comp=7、`handle_peer_msg`/Reconfigure 臂、`dlg_confirm_modify_bt`/`make_bt_spec`（拆不动的本质复杂度，architect 定性维持）。
- **CPD 残余克隆家族**：198 群组（70 阈值）/9 群组（200 阈值），主体 = engine 测试字面量家族 + supervisor 测试夹具（历轮维持）。
- **bt_dialog.rs 变异点 119（>100 登记维持）**；arch_check 第 18 条裁决=否（备查）。

### 实现定义值登记（累计，当前有效口径）
- **D44–D73 沿历轮登记不变**。
- **D73② 载体修订登记（本轮 QA 修复，规格可观察面不变、载体升级为协议级）**：
  - 闸门：`handle_peer_msg` Request 臂 `allowed = !am_choking && !upload_paused`（保留，第二防线）。
  - **新增协议通知**：Reconfigure 做种回退臂置位 `upload_paused=true` 时统一对全部连接 `am_choking=true` + 发送 Choke（暂停即协议级 choke）；暂停期 tick 跳过 apply_choker（防 rechoke 周期解 choke）；恢复（双完成臂）既有 `apply_choker(seeding=true)` 因状态迁移自动发 Unchoke。
  - 修复依据：原「静默忽略请求」使 libtorrent 侧请求悬挂 + snubbed 后永不重发，回做种后上传无法恢复（QA-MB-10 e2e 双侧观测实证，见 notes/testing.md QA 终验轮条目）。
- D73①③④⑤⑥⑦ 载体沿 architect 轮登记不变。

## 二、当前产出情况

### 本会话产物清单（QA 终验轮）
1. **产品缺陷修复 ×1（最小改动，与已接受规格一致）**：`src/bt/engine/mod.rs` 两处——①Reconfigure 做种回退臂新增统一 Choke 循环（6 行）；②tick rechoke 增加 `!upload_paused` 门控（1 处条件）。修复「做种回退→回做种后上传不恢复」（D73③「上传恢复 leecher 可拉到数据」此前不可达）。
2. **e2e 套件修复与补齐**：`project/qa/runners/suite_modify_bt.py`——①MB-10 暂停窗等待条件重写（占位文件全尺寸稀疏预分配使旧门控 0 秒塌缩；改为回做种翻转等待 + 下载文件与种子原件字节比对 + 有界冻结断言）；②新增 `_case_3`（QA-MB-03 v1.8 可执行化：取消未完成文件=落盘删除+total 重算+勾回从头补下字节级验证；限速门控 + 顺序无关完成态 + 动态取消目标）；③run() 纳入 03。
3. **e2e 全量对账实跑**：15 套件全部注册用例与实跑记录差集归零（含本会话早段中断前的 14 批日志合并对账 + 本轮补跑）；TP-08 确认为 v1.6 已退役（非缺口）。
4. **`project/notes/testing.md`**：追加 QA 终验轮三条经验（上传暂停协议化修复判别法 / BT e2e 门控三课 / 断言数值先验）+ INDEX.md 索引既有行覆盖。
5. **`project/handoff.md`**：重写为当前有效口径（本文件）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| bin 全部单元测试（修复后重跑） | 1049P/0F（--bins 全部二进制口径；含 82 属性） | `cargo test --bins`（CARGO_INCREMENTAL=0 + CARGO_PROFILE_*_DEBUG=0） |
| 集成回归（修复触及面） | hardening 247P + hardening_g_engine 789P + hardening_g_supervisor 762P + hardening_engine_backfill 895P，全 0F | 逐目标 `cargo test --test <t>` |
| e2e modify_bt（修复后） | QA-MB-03/10/11/12 = 4/4 P（161.8s/60.3s/13.0s/16.4s） | `python3 suite_modify_bt.py` |
| e2e throttle_proxy 补口 | TP-07/14/10/13 = 4/4 P（缺口语合并后该套件注册用例全覆盖） | `QA_TP_ONLY=7,8,9,10 python3 suite_throttle_proxy.py` |
| e2e 其余 13 套件 | 本会话早段实跑日志合并对账：add_task 21P、config_template 5P、download_engine 16P、integrity_check 12P、modify_task 10P、named_proxy 18P、persistence_config 11P、resume_sidecar 14P、retry_backoff 9P、slots_queue 15P、tui_display 21P、ui_conns 18P、ui_pieces 7P，全 0F | `.work/tmp/e2e-suite_*.log`（complete 前请勿清） |
| 用例对账 | 15 套件源码注册 ID × 实跑 ID 差集 = 0（退役/移交用例凭规程注记豁免） | 对账脚本见会话记录 |

### 对账口径
- 产品改动面：仅 `bt/engine/mod.rs` 2 处（做种回退臂 + rechoke 门控）——正常下载/做种流程 `upload_paused=false` 下 rechoke 行为逐位不变；仅做种回退臂行为升级为协议级。
- bins 口径说明：本轮 `--bins` 1049P 覆盖全部二进制目标（ezr + proxy/mktorrent/fixture），较 architect 轮 `--bin ezr` 1003P 口径宽——同源测试零改动，差异 = 计入子命令二进制的 46P。
- mission/features/qa 规程文件：零改动（QA 只动 runners 与产品缺陷，规格面无修订请求）。

### 待办与待批
- 无待批项。QA 未发现规格矛盾（Gherkin/单测/实现三方一致面内无冲突需裁决）。
- 历轮受限项（leech 专属 e2e、变异实跑、ezr-demo 守卫重定基线）维持登记，如需立项请操作者显式指令。

### 移交建议
- **six-pack 流转路径已走完（specifier → coder → cleaner → architect → QA → Done）**：请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留，下一轮可直接续跑或开新项目）。
- 如需继续迭代（如 leech 专属 e2e 立项、hardender 变异实跑），建议以本文件为上游交接内容显式 `bin/swarm begin` 对应角色。
