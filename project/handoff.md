# 交接（handoff）

> 流程：six-pack　当前节点：six-pack/QA（本轮已完成）→ **six-pack 链走完，工作流完结**
> 路径：specifier → coder → cleaner → architect → hardender（操作者指示跳过并关闭，见移交建议）→ QA → Done
> 会话：ezr-QA-round（2026-11-06，six-pack 最终独立验证轮：规格对账 + 六层基线复验 +
> QA runners 14 套件实跑 + CRAP/DRY 分析 + 最小修复复验 + 交接收尾）
> 上游交接：project/handoff.md（architect 轮交接 = 四阶段架构评审 + 架构改进 2 项 +
> 属性测试 +3 + arch_check 16 规则全绿）
> 环境注记：沿用 architect 轮沙箱（rustup nightly 1.101.0，PATH=~/.cargo/bin）。

## 一、产物历史完成情况

### 项目与需求
- **EZR Downloader**：Rust + tokio + ratatui 终端下载器。需求权威 `project/mission.md`
  （分期模式 v1.42）+ `project/mission/phase-01…04.md`。
- 本轮为 **QA 最终轮**：six-pack 链最后一名角色，对全部既有产物做独立验证与度量。

### 产物总账
| 节点/切片 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| specifier/coder/cleaner（历轮） | bug25 三项规格 + 实现 + demo 同构 + 保持行为清理 + 覆盖率/CRAP/DRY/变异点度量 | 已交付 | 沿袭全绿（cleaner 轮 925P/0F） |
| architect（上轮） | 四阶段架构评审 + 架构改进 2 项（cmd.rs HOME 收口、arch_check 白名单收口）+ 属性测试 +3 + arch_check 复核 | 已交付 | 928P/0F + 集成 6201P + 16 规则全绿 |
| **QA（本轮）① 规格一致性核对** | 对账表 .work/tmp/spec-consistency.md + case-id 差集：跨轮 runner 同步欠账（v1.38 限速字段几何漂移 6 套件 + throttle 键名/单位/语义漂移 + 缺失用例 12 个）→ 会话前段已全部预同步（add_task +01-20/21、modify 几何+MT-10、throttle 翻转+TP-14/10/13、named_proxy/tui_display/integrity/persistence 几何与注记） | 已交付 | 预同步后各套件实跑佐证（见②） |
| **QA（本轮）② 六层基线复验** | ①静态 fmt+clippy 0；②单测 928P/0F 精确对账（925+3）；③集成 12 套件 6201P 逐组对账；④arch_check 16 规则；⑤QA runners **14 套件全绿**（throttle_proxy 经 QA_TP_ONLY 三批前台跑齐 1,2/3,6/4,5,7,8,9,10 + 复跑修复 3 处 runner 漂移）；⑥ezr-demo 44P/0F + clippy 0 | 已交付 | 全部前台实跑；修复 9 处 runner 侧问题（6+3）均复验绿 |
| **QA（本轮）③ CRAP 分析** | llvm-cov region 覆盖实测：dlg_finish_add 97.94%（97/2 缺失）、dlg_confirm_add_bt 93.86%（114/7 缺失）；CRAP 复算 14.1 / 14.7（comp=14 登记口径）远低于 CRAP4 阈值 30；clippy cognitive 实测 11 / 9（--force-warn 解除 crate 级 allow） | 已交付 | 登记项无恶化（两函数源码本轮零改动 + 单测集 928 精确对账 → 覆盖率与基线同构）；clippy cognitive 与登记 comp≈14 的口径差异见第二节对账 |
| **QA（本轮）④ DRY 分析** | jscpd min-tokens 40：cleaner 轮「7 文件=68」口径文件集未随归档留存、穷举重构不可复现 → 改立可复现新口径（全生产源码 ezr/src + demo/src = **1036 clones**）+ 无恶化判据：工作区 vs git HEAD 双扫描 clone 集逐位置恒等（1036=1036，A-only/B-only 均 0） | 已交付 | 本轮产品面仅 config.rs 3 行（peer_pipeline 常量回退，步骤①既定项），clone 群不变 |
| **QA（本轮）⑤ 收尾复验** | arch_check 16 规则收尾复跑 EXIT 0；产品零 bug 结论（四项发现全部 runner 侧漂移，已修复复验） | 已交付 | EXIT 0 |

### 遗留受限项（当前有效，沿袭 − 本轮解除）
- **本轮解除**：QA-TD-23 与 QA-UC-07/17/18 环境预存 FAIL——tui_display / ui_conns 套件
  本轮全量实跑全绿（无 F/S 登记），预存 FAIL 面复验通过，受限项关闭。
- 沿袭（历轮登记，全部不变）：死 tracker 约 4 分钟静默窗；TUI 恐慌自愈边界（bug6）；
  QA-SQ-13 竞态沿袭待办；QA-RS-06 沿袭；supervisor block_worker fatal 竞态观察项；
  leech 判据无 e2e 自动化；bad_piece 归属口径（D67 允许）；
  `periodic_flush_writes_sidecar_mid_download` HTTP 滴流 flake 单跑绿沿例；
  同 IP 拓扑残余启动段；leech 封禁按裸 IP 连坐面；ezr 不发 KeepAlive。
- 沿袭（cleaner 轮登记，全部不变）：g_v147 quota flake 间歇态沿例（本轮集成全量跑绿
  709P）；CRAP 登记口径 dlg_finish_add / dlg_confirm_add_bt comp≈14 拆分需重构轮授权
  （本轮复算无恶化，登记继续有效）。
- 沿袭（architect 轮登记，全部不变）：engine↔bt 模块级双向类型引用 = 既定协调枢纽
  模式，拆分属不安全类，移交 hardender 变异暴露后针对性裁决。
- **变异实跑（cargo-mutants 跑模式）未执行**：属 hardender/mutants 轮预算，本 QA 轮
  沿袭登记不越权代跑（scan 计数沿袭：ezr 全库 5710 / bug25 触碰 7 文件 437）。

### 实现定义值登记（累计口径，本轮无新增）
- 本轮未新增任何实现定义值；既有登记值全部不变（面板层速度格式化/判重目录解析/
  并发列宽/客户端收缩基线/重复任务错误窗口等，见历轮交接同节）。
- 度量口径变更 2 项（非实现定义值，见第二节对账）：DRY 基线口径重立（1036 clones
  全生产源码）；CRAP cognitive 实测口径补充（11/9）。

## 二、当前产出情况

### 本会话产物清单
- **QA runners（2 文件修复）**：`suite_throttle_proxy.py`（TP-10/13 对话框导航
  Tab×3→Tab×2 + 下载限速预填清场；TP-10 fixture cd=tp10→cd=tp10.bin 对齐落盘名）、
  `suite_add_task.py`（QA-01-20 补 Tab×3 导航至下载限速 + QA_AT_ONLY 用例过滤机制，
  v141 QA_TP_ONLY 同款）。
- **project/notes/**：testing.md / environment.md 追加 QA 轮条目 + INDEX.md 两行同步
  （对话框焦点几何探针实证法 / fixture cd= 落盘名覆盖 / llvm-cov 工件挥发手工管线 /
  DRY 口径失传 A/B 双扫描恒等判法 / 残留套件进程清理与磁盘分层清理口径）。
- **.work/tmp/**：spec-consistency.md、case-id-gap 对账、qa-run/ 各批日志与 screen
  dump、jscpd A/B 报告（.work 不进归档，结论已转录本文件与 notes）。
- **ezr 产品端**：本轮零改动（config.rs peer_pipeline 回退为会话前段预同步既定项）。

### 验证证据
| 验证项 | 结果 | 命令（可复现） |
|---|---|---|
| ezr bins 单元测试 | **928P/0F**（925 基线 +3 属性，精确对账） | `cd project/ezr && CARGO_INCREMENTAL=0 cargo test --bins` |
| ezr 集成测试（12 组） | **全绿 6201P** | `cargo test --test <套件名>` 逐组 |
| ezr 静态检查 | fmt 干净 + clippy 0 警告 | `cargo fmt --check` / `cargo clippy --all-targets` |
| arch_check.sh | **16 规则全绿 EXIT 0**（开工+收尾两次） | `bash scripts/arch_check.sh` |
| QA runners（14 套件） | **全绿，0F**（含 throttle_proxy 三批 + 2 轮复跑修复） | `python3 suite_*.py`（QA_TP_ONLY / QA_AT_ONLY 分批） |
| throttle_proxy 明细 | TP-01 P101.7s / TP-02 P202.0s / TP-03 P201.9s / TP-06 P101.8s / TP-04 P8.0s / TP-05 P43.0s / TP-07 P2.6s / TP-14 P2.9s / **TP-10 P22.3s（修复后）** / **TP-13 P4.5s（修复后）** | `QA_TP_ONLY=<批次> python3 suite_throttle_proxy.py` |
| ezr-demo 测试 / clippy | **44P/0F / 0 警告**（沿袭复核） | `cd ezr-demo && cargo test` / `cargo clippy --all-targets` |
| CRAP 登记项复算 | dlg_finish_add 97.94% region / CRAP≈14.1；dlg_confirm_add_bt 93.86% / CRAP≈14.7 | llvm-cov 手工管线（notes/testing.md QA 轮条目） |
| DRY 无恶化 | 工作区 vs HEAD clone 集恒等（1036=1036，diff=0） | `jscpd --min-tokens 40`（新基线口径）双扫描 |

### 对账口径
- 计数链：bins 928 = 925（cleaner 基线）+ 3（architect 属性测试）；其他 bin 不变；
  集成 12 组与 cleaner 轮口径一致（g_v147 实测 709P 沿例）。
- **CRAP 口径差异**：登记「comp≈14」为圈复杂度近似（cleaner 轮受限近似口径）；本轮
  clippy cognitive 实测 dlg_finish_add=11 / dlg_confirm_add_bt=9（cognitive≠cyclomatic，
  两者口径不同不可互换）；CRAP 复算按登记 comp=14 代入标准公式
  comp²(1−cov)²+comp。后续如做拆分裁决，建议先按统一口径重新登记。
- **DRY 口径差异**：cleaner 轮「7 变更面文件 = 68 clones (10.48%)」的文件集未随归档
  留存，本轮穷举重构不可复现；按 engineering.md 受限近似改立可复现新基线（全生产
  源码 1036 clones）并以「工作区 vs HEAD clone 集恒等」作为本轮无恶化的充要判据。
  后续轮沿新口径即可直接对账。

### 合并吸收决策（按 QA 职责逐项移交）
- **致 specifier/coder**：v1.39/v1.40 上传限速字段退役使添加对话框 Tab 几何前移一档
  （打开焦点 URL0 恒定；下载限速 = 从打开起 Tab×3）。后续任何对话框字段集变更，runner
  同步须以哨兵探针实证重推 Tab 几何，勿按文档注释转写（本轮 3 处漂移均源于旧注释）。
- **致 coder**：fixture `cd=` 查询参数 = Content-Disposition filename 覆盖（产品
  derive_name 优先级 1），新增用例的落盘观测路径必须与 cd= 值一致。
- **致 cleaner/architect**：度量口径（CRAP/DRY 基线）必须随归档留存**精确命令与文件
  集**（本轮 68 口径失信的根因）；架构轮如调整对话框字段集，同轮同步 runner 导航。
- **致 hardender（已跳过并关闭，预算项全部处置完毕）**：①变异实跑（cargo-mutants，
  scan 计数 5710/437）——操作者裁决移除（2026-11-06：跳过 hardender，不再追办，
  scan 计数留档备查）；②dlg_finish_add / dlg_confirm_add_bt 拆分裁决——复验轮判定
  为度量债务（非缺陷，cognitive 11/9 与基线一致无恶化），随登记保留；③engine↔bt
  协调枢纽拆分裁决——复验轮已裁决维持既定模式。无剩余 hardender 预算项。

### 待办与待批
- 无待批。待办全部随合并吸收决策移交（见上）；six-pack 链产物无遗留阻塞项。
- 操作者修订建议（工具侧 packs/，按宪法不自行改动）：六套件 QA SKILL 或 _common/
  notes 可补「runner 对话框导航断言须以哨兵探针实证 Tab 几何，勿信注释」与
  「fixture 查询参数语义（cd=）变更须 runner 同步清单」两条。

### 移交建议
- **six-pack 链已走完（specifier → coder → cleaner → architect → hardender（跳过）→
  QA → Done），工作流完结**：操作者运行 `bin/swarm complete` 归档本轮产物。
- hardender 预算项已全部处置完毕（变异实跑操作者裁决移除、不再追办；CRAP 拆分与
  engine↔bt 已由复验轮裁决），无需另起 hardender 会话。
- 工具链交接：rustup 1.29.3 + nightly 1.101.0（PATH=~/.cargo/bin，registry rsproxy
  sparse）；swarm 工作区 /home/z/swarm-ezr2。

## 三、遗留受限项复验轮追加（2026-11-06，操作者指令「再验证，有问题则修复，没问题则移除」）

> 环境：工作区 /home/z/swarm-ezr2（自 swarm-20261106-100546-six-pack-QA.tar.gz 重建）；
> 工具链 rustup 1.29.3 + nightly 1.101.0（镜像测速 TUNA 3.08MB/s 最快，前台安装；
> crates.io 走 rsproxy sparse——TUNA index config 的 dl 指向官方 static.crates.io，慢）。

### 复验结论总表（原 15 项 → 现行有效 1 项；变异实跑经操作者裁决移除）
| 原条目 | 复验结论 | 处置 |
|---|---|---|
| ezr 不发 KeepAlive | 规格沉默（FR-02-17 仅列消息集）；写泵加 90s 空闲保活 | **产品已修复**，移出 |
| 死 tracker ~4min 静默窗 | FR-02-15 明文（15/30/60/120s 倍增梯）+ FR-02-16 全败判败的复合后果，非缺陷 | 规格行为，移出（注记保留于 notes/environment.md） |
| TUI 恐慌自愈边界（bug6） | TUI_RESTORED_BY_PANIC + 帧间重进已实现（main.rs:107-117/361-368）；残余为钩子还原与捕获间固有窗口，不可再改进 | 已实现/固有，移出 |
| QA-SQ-13 竞态沿袭待办 | 产品正确；runner 断言锚在 <100ms 瞬态「等待中」→ 改锚长驻 R toast（`↻ 手动重试（等待中）`≈4s）+ 再探回「已失败」 | runner 已修复（slots_queue 3×15P/0F），移出 |
| QA-RS-06 沿袭 | **实锤三根因**（详见下节） | runner 已修复（resume_sidecar 5×14P/0F），移出；规格缺口移交 specifier |
| supervisor block_worker fatal 竞态观察项 | 现行 download.rs:148-171 prepare（create+set_len）先于全部 spawn/refill——登记笔记设想的根治形态即现行代码；本轮 RS-06 所见 ENOENT fatal 实为 FR-01-26 名字接续劫持（见下），非 supervisor 竞态 | 观察项失效，移出 |
| leech 判据无 e2e 自动化 | 仍缺：QA-AL-01..08 无 runner、引擎级无 leech 专属测试（LEECH_MIN_ALIVE_SECS=120 需 ≥120s 真实时钟夹具） | **维持登记** |
| bad_piece 归属口径（D67 允许） | 代码与登记一致（engine.rs:2146-2148）；D67 已批实现定义 | 已批实现定义，移出 |
| periodic_flush HTTP 滴流 flake | 固定 4.6s 单读 → 最长 20s 轮询（flush 周期 2s，≈10 倍余量）；实测一次 10.4s 晚落盘——旧口径必挂、新口径接住；3 连绿 | runner 已修复，移出 |
| g_v147 quota flake 间歇态 | **真根因实锤**：stub_206_trickle / stub_200_norange_slow 不读请求先写响应 → hyper「received unexpected message from connection」→ 探测秒败 → Failed(Transient) 全程 0 连接行（即登记的 left:0 形态）；此前「2 核压力敏感」只是表层 | **已修复（真根因）**，移出 |
| CRAP dlg_finish_add / dlg_confirm_add_bt | cognitive 复测 11 / 9，与 QA 轮基线精确一致，无恶化；拆分仍需重构轮授权 | 维持为度量债务（非缺陷） |
| engine↔bt 双向类型引用 | 既定协调枢纽模式（architect 轮裁决），本轮无变化 | 已裁决维持，移出 |
| 同 IP 拓扑残余启动段 | 第三方（lt）同 IP 纠缠固有行为，真实局域网无此段；属测试拓扑注记 | 环境注记，移出 |
| leech 封禁按裸 IP 连坐 | blacklist 键 = IpAddr（engine/mod.rs:212）系设计取舍（防端口轮换规避）；改动需规格轮 | 设计取舍，移出（注记保留） |
| 变异实跑（cargo-mutants）未执行 | 操作者裁决（2026-11-06）：跳过 hardender，不再追办（scan 计数 5710/437 留档备查） | **操作者裁决移除** |

### 现行有效遗留受限项（复验后）
1. **leech 判据无 e2e 自动化**（QA-AL-01..08 规格；需专项测试轮补 runner/引擎级用例，夹具须容纳 120s 判据窗）。

### 新增移交（specifier）
- **FR-01-26 v1.28 规格缺口**：重新添加任务的 sidecar 接续臂（dialogs.rs:244 `Sidecar::load(dir/推导名)`）按路径基匹配、无「活任务占用」守卫——异 URL 同推导名且原任务存活时，新任务复用同名（规格明文「不按 FR-01-02 追加序号」），两活任务共享同一 dl 文件互踩（RS-06 复验屏面实证：同名 ×2 → 一方 fatal「不自动重试 · 0 B」→ 槽位释放 → 队列前进）。FR-01-22 一致性兜底只覆盖死任务陈旧 sidecar，不覆盖**并发写 sidecar**。建议修订：接续臂加「无活任务占用该推导名」前提，否则按 FR-01-02 判重追加序号。runner 侧已以 cd= 专名隔离规避（见下），产品修订需规格轮授权。

### 本轮变更清单
- **产品（1 处）**：`src/bt/engine.rs` 写泵空闲 90s 发 keep-alive（`PEER_KEEPALIVE_SECS`，BEP 3 惯例；timeout+recv 取消安全；被置换连接先收 Drop，二者不竞争；!seeding 门控下做种长闲置连接为真实受益面）。
- **集成测试（hardening_g_v147.rs）**：①stub_206_trickle / stub_200_norange_slow 补读请求头循环（write-before-read 竞态根因，附根因注释）；②periodic_flush_writes_sidecar_mid_download 固定 4.6s 单读改 20s 轮询；③engine_conns_quota_exact_and_unique_ids 采样窗加宽（起步 150→300ms、单事件 400→800ms；25s deadline 与断言原样）。
- **QA runners（2 文件）**：suite_slots_queue.py QA-SQ-13 断言改长驻锚（toast + 再探回已失败）；suite_resume_sidecar.py RS-06 三层——①5 占槽任务 `cd=rs06-h{i}.bin` 专名隔离（绕 FR-01-26 接续边界）、②体量 3MB@100KB/s→5MB@60KB/s（≈83s 覆盖降速沙箱 setup 窗口；曾观测持有者在 setup 内提前完成 → 槽位空闲 → 第 6 任务无排队）、③End 注入 settle+有界重注（吞键免疫，QA-SQ-13 同源处方）。

### 验证证据（全部前台实跑，2 核沙箱）
| 验证项 | 结果 | 口径 |
|---|---|---|
| 单元测试（修复前基线 / 修复后） | 928P/0F 两次 | 精确对账（925+3） |
| 集成 12 套件 | **6201P/0F** | 与 QA 轮基线精确对账（142+721+695+202+451+16+2+726+828+858+851+709） |
| g_v147 全套件附加复跑 | 709P/0F ×2 | 含两处测试加固后 |
| quota 定向压测 | 修复前 ~1/6 失败率（复现 3 次，探针实证 Failed(Transient) 全程 0 行）→ 修复后 **15 连绿** | 三证齐口径升级为根因修复 |
| periodic_flush 定向 | 3 连绿（含一次 10.4s 晚落盘样本——旧口径必挂） | |
| QA runners slots_queue / resume_sidecar | 3×15P/0F / **5×14P/0F** | RS-06 修复后 5 连绿且耗时稳定 20.7s |
| fmt --check / arch_check / clippy | EXIT 0 / 16 规则全绿 EXIT 0 / 0 警告 | clippy 含 --all-targets |
| cognitive 复测（force-warn via RUSTFLAGS） | dlg_finish_add=11 / dlg_confirm_add_bt=9 | 与 QA 轮登记一致 |

### 工具链注记
- 镜像测速（channel-rust-nightly.toml）：TUNA 3.08MB/s > nju 2.19 > ustc 2.26 > sjtu 1.41 >> 官方 0.03；rsproxy index 异常（328B）。**crates 下载**与 rustup 分发镜像须分开选型：TUNA sparse index 的 `dl` 指向官方 static.crates.io（慢且曾停滞），cargo 注册表改 rsproxy sparse 后 285 crate 2min 拉齐。
- 沙箱环境后台进程随 Bash 调用结束被回收：cargo 构建须前台分段（timeout+增量缓存续跑）。

### 归档卫生（操作者指令，复验后追加 2026-11-06）
- 两归档（QA / 复验）剔除覆盖率管线遗留工件各 **19 件**：`project/ezr/default_*.profraw` ×18
  （llvm-cov 插桩运行的原始剖面，进程退出时逐个落盘）与 `project/ezr/.work-tmp-merge` ×1
  （llvm-profdata merge 中间产物，profdata 格式 638KB）。条目 374→355 / 462→443，
  体积 3.5M→1.7M（二进制剖面压缩率差，占归档近半）。
- **COMPLETE 前清理口径建议**（工具侧 notes 可补条）：覆盖率管线工件全族
  （`*.profraw` / `*.profdata` / `.work-tmp-merge` 类临时中间件）在归档前必须清除——
  本轮漏网根因：CRAP 实测走手工管线（notes/testing.md「llvm-cov 工件挥发手工管线」），
  工件不在 git 跟踪面内，归档打包按文件系统全量收集故而夹带。
