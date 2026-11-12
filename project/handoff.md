# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/architect（架构收敛轮已完成，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner 收敛轮 ⇒ architect 收敛轮 ⇒ QA 终验轮 ⇒ specifier 显示指标/调整轮 v1.48–v1.51 ⇒ coder 产品实现轮（FR-01-117）⇒ coder 补偿轮（v1.47–v1.50 六项欠账）⇒ cleaner 清理收敛轮 ⇒ **architect 架构收敛轮（本轮 2026-11-12）** ⇒（移交建议见第二节）
> 会话：six-pack-architect-20261112（任务 = 四阶段架构评审 + 保持行为改进 + 属性测试 + 自动化检查收口；上游交接 = project/handoff.md 上一版）
> 环境：nightly 1.102.0（9c5648833 2026-11-10，与交接基线同 commit）@ ~/.cargo/bin + cargo registry rsproxy sparse；度量四件（llvm-cov/mutants/PMD/bb+APS）本轮未装机（architect 验证面零外部依赖），hardender/QA 开工先按 engineering.md「工具启动」重建 .tools

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议，ratatui + tokio）。需求权威来源 = `mission.md`（分期模式，**v1.51**）+ `mission/` 两份 phase 详述（**phase-01 v1.50** 为当前页签交互与 FR-01-117 依据、**phase-02 v1.20** 为当前 BT 列表行布局与显示指标依据）。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier 历轮 | features/ 全套 Gherkin + qa/ 全套规程 | 已交付 | 历轮对账在案 |
| coder/cleaner/architect/QA 历轮（至 2026-11-10 QA 终验轮） | ezr 产品实现 + 15 套件 e2e + 引擎拆分 + 终验 | 已交付 | QA 终验全绿（历轮 handoff 口径在案） |
| specifier 显示指标/调整轮 v1.48–v1.51（2026-11-10/11） | phase-01 v1.50 / phase-02 v1.20 + mission.md v1.47–v1.51 + features/qa 配套 + ezr-demo 同构修改 | 已交付 | 历轮 APS/cargo test/PTY 在案 |
| coder 产品实现轮（2026-11-11，FR-01-117） | project/ezr 源码 6 文件 + 守卫三守卫 | 已交付 | cargo test 7930P/0F；clippy 0；APS 双 exit=0 |
| coder 补偿轮（2026-11-11，v1.47–v1.50 六项欠账） | project/ezr 源码 9 文件 + 守卫 10 条 | 已交付 | cargo test 7990P/0F；clippy 0；fmt clean；APS 三份双 exit=0 |
| cleaner 清理收敛轮（2026-11-12） | lint 基线恢复 5 处 + conn_flags 表驱动 + join_panic_reason 提取 + 守卫 8 条 + 夹具 DRY 收编 2 通道 + notes 三分类 | 已交付 | release 零警告；clippy 0；fmt clean；cargo test 8056P/0F；批次行覆盖 95.5%；CRAP 可修点收敛归零；CPD 双阈值；变异点扫描 777 点登记 |
| **architect 架构收敛轮（本轮 2026-11-12）** | **四阶段评审（结论+清单+动作）+ 反链接 VS15 工具族单源化 + arch_check 17→18 + 属性测试 +4 + notes 三分类** | 已交付 | **fmt clean；clippy --all-targets 0W；release 0W（5m13s）；cargo test 8060P/0F（=基线 8056+4 属性）；arch_check 18 条全过；属性独立命令 86P/0F** |

### 遗留受限项（当前有效）
- 历轮受限项沿 QA 终验轮口径不变（leech 专属 e2e、变异实跑、6 个 BT/命令窗口族无专属 runner、ws(s) announce 无代理面）。
- **ezr-demo verify_ezr.py 预存 FAIL 15 项维持（脚本锚点漂移，非产品回归）**；本轮 ezr-demo 零改动。
- CRAP comp 残余登记（沿 cleaner 复核口径，均属既定裁决的登记类）：bt/engine run comp=110、App::on_evt comp=32、draw_conns comp=30、Task::pieces comp=22（cov=1.0）、App::restore comp=11（cov=1.0）、App::overlay_open comp=8、批外观察 app/cmd.rs on_cmd_key CRAP 6.1；测试夹具误差臂（fake_udp_tracker 族）登记不硬凑。
- 变异点 >100 文件 4 个登记（rust.md §7 裁决，architect 本轮复核维持不拆）：app/mod.rs 149 / bt/engine/conn.rs 125 / ui/text.rs 122 / model/task.rs 101，密度 0.10–0.30 点/行线性带内；hardender `--file` 分块兑现（本轮改动面 ui/text.rs 计数或有 ±1 内漂移，hardender 以自扫为准）。
- CPD（PMD 7.29，-l rust）min-tokens=200：批内产品家族 1 个（link_break_spans 双胞胎）**已本轮单源化消除**；其余 8 个为批前既有测试克隆（bt/engine/mod.rs 测试体/夹具变体 ×7）与独立二进制设计性重复（ezr-mktorrent ↔ bencode ×1）维持登记；PMD 本轮未装机未复扫，下轮 DRY 复扫沿 cleaner 双阈值口径（min200/min70）重跑对账。

### 实现定义值登记（累计，当前有效口径）
- D44–D74 沿历轮登记不变，本轮无新增 D 值。

## 二、当前产出情况

### 本会话产物清单（architect 架构收敛轮）
1. **四阶段架构评审**（结论 + 违规清单 + 改进动作三段，全树覆盖）：
   - ①UI/核心分离：合规（ui 纯渲染零 IO 框架导入、app 零 ui 依赖、机制层零渲染导入；ui 层唯一 crate::bt 导入在 cfg(test) 夹具内，合规形态）。
   - ②依赖规则：顶层依赖图单向无环（model→model；engine→model；bt→model+engine；app→model+engine+bt+textfield+scrollbar；ui→app+model+textfield+scrollbar+multiline；main→app+model）；engine↔bt 双向引用实测已收敛为 bt→engine 单向（比上轮记录更收敛）；**唯一违规 = 规则 14 命中 ui/list.rs:81/86**（v1.49/v1.52 视口容量自愈写未按「响亮失败→显式扩白名单」登记，cleaner 轮验证表未含 arch_check）。
   - ③信息隐藏：门面/窄接口纪律完好；**违规 = 反链接 VS15 工具族双实现副本**（popups.rs/text.rs 各一份 break_links+fg_shadow+link_break_spans）。
   - ④局部代码质量：cleaner 批次文件结构清晰、clippy/fmt 零漂移；属性覆盖缺口 = 反链接工具族零属性 + conn_flags 表驱动重写仅有固定值单测。
2. **反链接 VS15 工具族单源化（保持行为不变）**：break_links/fg_shadow/link_break_spans 收编 `src/ui/text.rs`（共享文本工具既有归属），`src/ui/popups.rs` 三函数退役（-49 行）、`src/ui/multiline.rs:110` 改指 super::text::*。**裁决修正**：cleaner 移交的「fg_shadow 语义分歧（b=0 角落）」判读有误——两版首臂 `Rgb(r,g,0)→Rgb(r,g+1,1)` 已捕获 b==0，第二臂只收 b≥1，`b-1` 不可能下溢，**两函数对所有输入行为恒等**，单点化零行为差；统一取 saturating 形态（防御式，免疫未来臂序重排）。消费点样式实测（detail.rs:247/popups.rs:269 用 FG=Rgb(198,208,212)、multiline.rs:110 用 Color::White）连算术臂都未触达。
3. **arch_check 17→18 条**：
   - 规则 14 白名单扩展 2 项精确形态（`app.selected = idxs.len() - 1;` / `app.scroll = max_scroll;`，list.rs 视口自愈）——裁决依据：当帧 rows 自愈与 tick 层夹紧不可互替（notes/testing.md v1.52 轮在案），移出渲染路径即行为等价改写不成立；精确字面豁免下政策写探针仍红（阳性对照实测）。
   - 新增规则 18【单源】：反链接 VS15 工具族定义点只允许 src/ui/text.rs（防 CPD 双胞胎回归）；阳性对照（popups.rs 临时第二定义）命中实测后移除。
   - 双阳性对照记录入册 scripts/arch_check.sh 尾注。
4. **属性测试 +4**（src/property_tests.rs，独立模块独立命令纪律）：prop_break_links_strip_vs15_identity（strip 还原 + 裸前缀隔断）、prop_break_links_idempotent（二次应用零增量）、prop_link_break_spans_lossless_and_alternation（拼接还原 + span 数 ∈{k,k+1} + shadow 交替）、prop_conn_flags_domain_invariants（字符域/唯一性/规范序严格递增 + 8 臂语义逐臂 oracle）。配套 `flags_case` 升 pub(crate) + 门面 re-export（bt/engine/mod.rs，零产品 API 增量）。
5. **notes 三分类追加 + INDEX.md 同步**（architecture/testing/environment 各一节，含对 cleaner 轮 fg_shadow 判读的修正指针）。
6. **本 handoff.md** 重写为当前有效口径。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 格式化 | cargo fmt --check clean（本沙箱工具链同 commit 复核，零漂移） | `cargo fmt --check` |
| Lint | clippy 0 警告（--all-targets） | `cargo clippy --all-targets` |
| 构建 | release 零错误零编译器警告 | `cargo build --release` |
| 单元测试（bin ezr） | **1028P / 0F**（942 单测基线不动 + 86 属性 = +4 新属性） | `cargo test --bin ezr` |
| 其余 bin 目标 | ezr-proxy 15P / ezr-fixture 30P / ezr-mktorrent 1P（基线持平） | `cargo test --bin <目标>` |
| 集成测试（12 目标） | **6986P / 0F** 逐目标与基线持平（hardening 247 + engine_backfill 916 + g_appcore 805 + g_engine 801 + g_main 2 + g_proxy 16 + g_slots 254 + g_supervisor 774 + g_throttle_err 498 + g_v147 788 + ui_backfill 946 + v115 939；磁盘受限顺序跑） | `cargo test --test <目标>` 逐目标 |
| 全量合计 | **8060P / 0F**（= 交接基线 8056 + 新属性 4；分项复算 1028+15+30+1+6986 自洽） | 分项复算 |
| 属性独立命令 | 86P / 0F（property_tests:: 独立口径，不与单测基线对账） | `cargo test --bin ezr property_tests::` |
| 自动化架构检查 | **18 条规则全过**（含新规则 18；规则 14 政策写探针红、规则 18 第二定义点探针红，对照后移除，记录入册尾注） | `bash scripts/arch_check.sh` |
| 行为零改动背书 | 工具族全输入恒等论证（fg_shadow 首臂 b==0 捕获）+ 全量测试零删除零改名全绿 + 产品面 3 文件均测试消费者零改动（multiline.rs 改指为等价换名） | `git diff --stat` |

### 对账口径
- 产品端改动面 = **3 文件**（ui/text.rs 工具族收编 + saturating 统一；ui/popups.rs 副本退役；ui/multiline.rs 改指）；测试面 = property_tests.rs +4、bt/engine/mod.rs 门面 re-export（`#[cfg(test)]` 门控）；检查面 = scripts/arch_check.sh；ezr-demo 零改动、mission 零变更（mission.md v1.51 与 phase-01 v1.50 / phase-02 v1.20 只读维持）。
- 测试增量对账：bin 1024→1028（+4 全落 property 层，单测基线 942 不动）；集成 12 目标逐数与基线持平；全量 8056→8060。
- 行为不变性：单源化三函数与原实现全输入恒等（fg_shadow 首臂捕获论证 + link_break_spans/break_links 逐字同体迁移）；全量绿 + clippy 0 + fmt clean 同 commit 工具链背书。

### 待办与待批
- 变异点 >100 四文件与 CRAP comp 残余的拆分收益在 hardender 兑现（`--file` 分块消费面就绪；本轮裁决=不拆维持，收口完成）。
- cleaner 轮「fg_shadow 语义统一裁决」待批项**已本轮裁决关闭**（语义恒等 + 单源化落地，登记于第二节产物 2）。
- 本轮无其他操作者待批事项。
- 产物交付（操作者 chat 指令）：`bin/swarm complete` 归档包 + ezr release 二进制（ezr/ezr-proxy，最终源码构建）→ 复制 /home/z/my-project/（归档即本轮任务终点，操作者执行）。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留），并按上文把产物复制到 /home/z/my-project/。
- 续跑建议：`six-pack/hardender`（SKILL 移交链下一节点：变异实跑按 --file 分块消费变异点登记面，CRAP comp 残余在其职责面）——由操作者显式决定。
