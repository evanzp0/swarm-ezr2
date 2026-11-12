# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/cleaner（清理收敛轮已完成，待操作者 `bin/swarm complete` 归档）
> 路径：specifier → coder（ws/webrtc 轮）⇒ 诊断/复验/bug28 轮 ⇒ specifier 修订轮 ⇒ coder 历轮 ⇒ cleaner 收敛轮 ⇒ architect 收敛轮 ⇒ QA 终验轮 ⇒ specifier 显示指标/调整轮 v1.48–v1.51 ⇒ coder 产品实现轮（FR-01-117）⇒ coder 产品实现补偿轮（v1.47–v1.50 六项欠账）⇒ **cleaner 清理收敛轮（本轮 2026-11-12，批次=补偿轮 9 源文件）** ⇒（移交建议见第二节）
> 会话：six-pack-cleaner-20261112（任务 = 补偿轮产物保持行为清理：覆盖率、CRAP、DRY、变异点扫描、lint 清零；上游交接 = project/handoff.md 上一版）
> 环境：nightly 1.102.0（9c5648833 2026-11-10；NJU 镜像 11.2/10.5MB/s 断层第一、直链 rustup-init + DIST_SERVER/UPDATE_ROOT 双变量前台装齐）+ cargo registry rsproxy sparse + 度量四件（llvm-cov 0.9.1 / mutants 27.1.0 / PMD 7.29.0 / bb 1.13.229 + APS，均 .tools 或 ~/.local，工具缓存不进归档）

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
| coder 产品实现补偿轮（2026-11-11，v1.47–v1.50 六项欠账） | project/ezr 源码 9 文件（引擎计算 2 + 数据链 4 + UI 3）+ 守卫 10 条 | 已交付 | cargo test 7990P/0F；clippy 0；fmt clean；APS 三份双 exit=0 |
| **cleaner 清理收敛轮（本轮 2026-11-12）** | **lint 基线恢复 5 处 + conn_flags 表驱动 + join_panic_reason 提取 + 守卫 8 条纯追加 + 夹具 DRY 收编 2 通道 + notes 三分类** | 已交付 | **release 零警告；clippy 0；fmt clean；cargo test 8056P/0F；批次行覆盖 95.5%；CRAP 可修点收敛归零；CPD 双阈值；变异点扫描 777 点登记** |

### 遗留受限项（当前有效）
- 历轮受限项沿 QA 终验轮口径不变（leech 专属 e2e、变异实跑、6 个 BT/命令窗口族无专属 runner、ws(s) announce 无代理面）。
- **ezr-demo verify_ezr.py 预存 FAIL 15 项维持（脚本锚点漂移，非产品回归）**；本轮 ezr-demo 零改动。
- CRAP comp 残余登记（本轮复核口径，均属既定裁决的登记类）：bt/engine run comp=110（引擎分发/守护链核心）、App::on_evt comp=32（事件分发核心）、draw_conns comp=30（九列渲染核心）、Task::pieces comp=22（三集合上色核心，幻影行澄清后 cov=1.0）、App::restore comp=11（v1.46 双臂分叉还原链，幻影行澄清后 cov=1.0）、App::overlay_open comp=8（弹窗优先级守卫链）、批外观察 app/cmd.rs on_cmd_key CRAP 6.1（键分发表）；测试夹具误差臂（fake_udp_tracker 族）登记不硬凑。
- 变异点 >100 文件 4 个登记（rust.md §7 裁决：app/mod.rs 149 / conn.rs 125 / text.rs 122 / task.rs 101，密度 0.14–0.30 点/行线性带内；模块边界归 architect，hardender `--file` 分块兑现）。
- CPD（PMD 7.29，-l rust）min-tokens=200 共 9 家族登记：批内产品家族 1 个（link_break_spans 双胞胎，见下「待批」）；其余 8 个为批前既有测试克隆（bt/engine/mod.rs 同文件测试体/夹具变体 ×7）与独立二进制设计性重复（ezr-mktorrent ↔ bencode 编码器 ×1）。

### 实现定义值登记（累计，当前有效口径）
- D44–D74 沿历轮登记不变，本轮无新增 D 值；coder 补偿轮两条补充裁决（distributed_copies「连接」=已握手、availability 瞬时指标不入 TaskSnapshot）维持。

## 二、当前产出情况

### 本会话产物清单（cleaner 清理收敛轮）
1. **lint 基线恢复（nightly 同版本异 commit 9c5648833 漂移引入 5 条）**：src/bt/engine/mod.rs 测试段 4 处死 setup（files+pmap 构造后未用，relevance/distributed_copies 三测试改用手写 covered 向量）删除；src/ui/task_lines.rs 1 处多余 `mut` 删除——均属补偿轮批次文件，行为零改动。
2. **src/bt/engine/conn.rs conn_flags 表驱动化**：if 链 8 臂 → (bool,bool)→Option<char> match ×4 + then_some ×2 + 数组 flatten 收集；输出字符序（D/d,U/u,O,I,K,?）与空串 "-" 回退逐字节不变；comp 17→6、CRAP 17→≤6。
3. **src/engine/mod.rs join_panic_reason 纯函数提准**：start_bt_task 的 panic 消息提取链（downcast &str→String→回退文案）抽为独立纯函数，本体 if-let/if 结构保留；comp 5 保持、cov 0→0.62、CRAP 30→≤6。
4. **守卫 8 条（纯追加，不改产品代码、不改既有测试）**：
   - src/app/engine/evt.rs 新增 `evt_arm_tests` 6 条：BtPeers 臂 seeders/peers 更新；BtTrackers 臂新 URL 推入/同 URL 就地更新（next_at 刷新）/异 URL 追加；非活跃态与未知任务忽略臂；BtStopped 臂 Paused 落终态三集合（done/partial/chunk_done/downloaded/连接清空）；BtStopped 臂活跃态滞后回报忽略（不覆盖新引擎集合、状态不回落）；BtFailed 臂委派 handle_failure（Fatal→Failed + error 落账）。
   - src/engine/mod.rs 新增 `supervisor_tests` 2 条：join_panic_reason 三载荷（&str/String/未知回退）；start_bt_task 登记发送端 + 引擎起停 + Cancel 干净退出无 BtFailed（shared_for_tests + make_meta 门面 + 不可达 tracker + try_recv 排水）。
5. **夹具 DRY 收编 2 通道（零调用点改动）**：src/app/testutil.rs 新增共享 `bt_task` 夹具（src/app/engine/bt.rs 测试模块改一行委托）；src/bt/engine/mod.rs `make_meta` 升 pub(crate) + 父级 `#[cfg(test)] pub(crate) use tests::make_meta;` 门面 re-export（rust.md 门面模式，供 engine 侧测试复用）。
6. **notes 三分类追加 + INDEX.md 同步**（environment/testing/architecture 各一节，见「cleaner 收敛轮（2026-11-12）追加」）。
7. **本 handoff.md** 重写为当前有效口径。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| 构建 | release 零错误零编译器警告（最终源码 5m54s 全量重建核证） | `cd project/ezr && cargo build --release` |
| 单元测试（bin ezr） | **1024P / 0F**（1016 基线 + 8 新守卫，零删除零改名） | `cargo test --bin ezr` |
| 其余 bin 目标 | ezr-proxy 15P / ezr-fixture 30P / ezr-mktorrent 1P（基线持平） | `cargo test --bin <目标>` |
| 集成测试（12 目标） | **6986P / 0F**（hardening 247 + engine_backfill 916 + g_appcore 805 + g_engine 801 + g_main 2 + g_proxy 16 + g_slots 254 + g_supervisor 774 + g_throttle_err 498 + g_v147 788 + ui_backfill 946 + v115 939；磁盘受限须顺序跑） | `cargo test --test <目标>` 逐目标 |
| 集成增量对账 | 6928 → 6986（+58 挂载收编）：engine_backfill/g_appcore/g_engine/g_supervisor/g_v147/ui_backfill/v115 各 +8（evt_arm_tests+supervisor_tests 双挂载）、g_throttle_err +2（仅 supervisor_tests）、hardening/g_main/g_proxy/g_slots +0——挂载面按各套件 include 树逐套件实测 | 分套件复算 |
| 全量合计 | **8056P / 0F** | 分项复算：1024+15+30+1+6986（= 基线 7990 + bin 8 + 挂载 58） |
| Lint | clippy 0 警告（--all-targets，触碰文件重编译核证）；cargo fmt --check clean | `cargo clippy --all-targets`、`cargo fmt --check` |
| 覆盖率 | 批次行覆盖 **95.5%**（10 文件 10187/10670；基线 94.9%）；evt.rs 85.1→95.3%、engine/mod.rs 87.1→90.6%、task_lines.rs 100%（`--skip property_tests::` 口径，属性测试分离纪律） | `cargo llvm-cov --bin ezr --lcov -- --skip property_tests::` |
| CRAP | 受限近似（llvm-cov lcov + 语法决策点+1 代 comp、闭包并入外层取极值）：可修点收敛完成（conn_flags 17→6、start_bt_task 30→≤6）；残余全数登记（见第一节）；分析器阳性对照（cmd.rs 家族可见）通过 | .work/tmp/crap_analyze.py（会话级，不进归档） |
| DRY | PMD CPD 7.29 `-d src --language rust`：min200 = 9 家族 / min70 = 200 行（对账口径沿 cleaner 收敛轮双阈值）；批内产品家族 1 个登记待批（见下） | `.tools/pmd/pmd-bin-7.29.0/bin/pmd cpd …` |
| 变异点扫描 | count 模式（`cargo mutants --list`，不跑变异）：批次 777 点；>100 文件 4 个登记（第一节）；bt/engine/mod.rs 20 点（上轮拆分后再分布延续账） | `cargo mutants --list --file <批次文件×10>` |
| g_v147 稳定性 | 基线轮 1 例 pause_closes 时序 flake（FIN/RST 竞态）三证判环境性（单跑绿/整目标复跑绿/改动零接触面）；本轮复跑 788P 全绿无 flake | `.work/tmp` 各轮日志 |

### 对账口径
- 产品端改动面 = **3 文件**（bt/engine/conn.rs conn_flags 表驱动；engine/mod.rs join_panic_reason 提取；bt/engine/mod.rs 死 setup 删除 + make_meta 门面）；测试面 = evt.rs/bt.rs/testutil.rs/engine mod 测试段 + task_lines.rs mut 删除；ezr-demo 零改动。行为不变性由 8056P/0F 全绿与字符序逐字节守卫背书。
- 需求对账：本轮为清理轮，无 mission 需求变更；mission.md v1.51 与 phase-01 v1.50 / phase-02 v1.20 原样只读维持。
- 挂载增量逐套件见验证证据表；bin 1016→1024 与挂载 +58 合计 +66，与各套件实测精确自洽。

### 待办与待批
- **fg_shadow 语义统一裁决（移交 architect/操作者）**：CPD 批内产品家族 link_break_spans 双胞胎（ui/popups.rs:42 ↔ ui/text.rs:153，34 行同体）各自捕获语义分歧的本地 fg_shadow——popups 版 `saturating_sub(1)` 防下溢、text.rs 版裸 `b - 1`（b=0 角落 debug panic / release 回绕，当前调用链颜色常量未触达属潜在缺陷）。机械单点化会改变另一侧角落行为（违反清理期不引入新行为），故保留双份并登记；统一语义（text.rs 侧补 saturating 或钦定裸减口径）后即可单点化。
- 变异点 >100 四文件与 CRAP comp 残余的拆分收益在 hardender 兑现（`--file` 分块消费面就绪）；模块边界收口归 architect。
- 本轮无其他操作者待批事项。
- 产物交付（操作者 chat 指令）：`bin/swarm complete` 归档包 + ezr release 二进制（ezr/ezr-proxy，最终源码构建）→ 复制 /home/z/my-project/（归档即本轮任务终点，操作者执行）。

### 移交建议
- 请操作者运行 `bin/swarm complete` 归档（`project/` 工作区保留），并按上文把产物复制到 /home/z/my-project/。
- 续跑建议：`six-pack/architect`（cleaner 下游；fg_shadow 裁决 + 模块边界收口 + 属性测试覆盖面在其职责面）——由操作者显式决定。
