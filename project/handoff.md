# 交接（handoff）

> 流程：six-pack　当前节点：**six-pack/specifier（20261128 MDM 完全移植轮：废弃 ezr 原 HTTP 下载策略/算法/代码，完全移植 misterdownloadmanager 的策略与算法——phase-05 v1.3 规格轮）——已完成**
> 路径：**six-pack/specifier（本轮）** → coder → cleaner → architect → (hardender) → QA
> 会话：six-pack-specifier-mdm-port（上游交接 = 20261127 coder 优化轮 handoff + 操作者 20261128 chat 指令：「执行 six-pack/specifier, 废弃原有的 ezr 的 http 下载的策略、算法和代码，完全移植 mdm 的策略和算法。mdm：git clone --depth 1 https://github.com/anefzaoui/misterdownloadmanager。完全移植 mdm，你根据这原则设计，如有待办与遗留问题，不需要我审批」+ ① rustup TUNA 镜像前台装齐 stable 1.100.1 + ② ezr 不需要兼容旧的版本 + ④ 待办与遗留问题 CHAT 详述 + ⑤ complete 后产物复制 download/）
> 环境：bb 1.13.230（`.tools/bb-bin/bb`）+ APS 仓（`.tools/aps`）+ MDM 参考仓（`.tools/misterdownloadmanager` @ aeeb878）本轮重建在位；rust 工具链 stable 1.100.1（TUNA 镜像前台装齐，minimal profile；**nightly 未装**——本轮无编译验证需求，coder 轮开工按 notes/environment.md 判别法补装）；`~/.cargo` 与 `.tools/` 非持久沿袭
> 受限项：①chat 选择题面板不可用（IM 通道）沿袭——Inversion 受限退化沿 v1.45–v1.64 先例（操作者指令文本即阶段 1 答案）；②runner 不可执行沿袭（qa/runners 归 QA 轮同步）；③MDM 性能基准 = 操作者线下验收口径（本沙箱无 MDM 运行环境）；④nightly 工具链缺位（本轮 specifier 无编译面，不阻塞）

## 一、产物历史完成情况

### 项目与需求
- ezr：Rust TUI 下载器（HTTP + BT 双协议）。需求权威来源 = `mission.md`（分期模式，**v1.65**）；phase-05 详述 = `mission/phase-05.md`（**HTTP 引擎 MDM 完全移植 v1.3**：FR-05-01~14 + D95–D100 新增、D83–D94 沿袭/退役注记）；ezr 不做旧版本兼容沿袭（v1.45/FR-02-35）。
- ezr-demo：仓库内独立子项目；历轮零接触，本轮零接触。

### 产物总账
| 节点 | 产物 | 完成情况 | 验证状态 |
|---|---|---|---|
| （历轮）phase-01/02 全功能实现与加固、phase-05 规格轮 + 实现轮 + pieces 各修复轮 + 速度回退修复轮 + 族A/族B 裁决落实轮 + D94「信息」列轮 + 20261127 MDM 对齐优化轮 | features 01–13 + qa 规程 + runners + 动态引擎/信息列/启动延迟优化全量落地 | 已交付（沿上轮账） | 当时口径 bin 1266P/0F + hardening 四套件 1148/1016/1003/1171 全 0F + clippy/fmt/check 全零警告 |
| **20261128 MDM 完全移植轮（本轮，specifier）** | **①mission.md v1.65：v1.65 修订注（MDM 完全移植全貌 + 键面修订）+ §4 核心参数改判（AIR2 分块退役→MDM planner 动态分段；封闭区间；Limiter 常量 burst）+ 分期索引 phase-05 行改写；②phase-05.md v1.3 全文重写：FR-05-01~14（planner 完整移植/探测请求应答分类/worker 重试/receive+Writer/限速 DNS 主机上限/恢复保存磁盘/单流降级/UI 口径/配置面）+ D95–D100 新增裁决 + D84/D85/D86/D87②/D89/D92/D93 退役注记（编号保留追溯）；③features/13-http-turbo v2 全文重写（场景 01–15 修订 + 16–20 新增：启动坡道/429 Retry-After/慢连接剔除/主机上限学习/网页拒收 + 12/13b 沿袭退役留空 + 13 键面矩阵改判）；④qa/13-http-turbo-qa.md v2 全文重写（QA-HT-01–20：环境前置新增可控 429/限速/403/网页 fixture 能力 + 通过准则封闭区间口径）；⑤八份既有规格最小同步：01-download-engine（09 重定向 10→20/头注）、01-resume-sidecar（01 sidecar 平铺段/03 封闭区间+覆叠/07 一致性臂 per-node ETag/头注）、01-config-template（37 键/165 行钦定原文内嵌替换/Examples 退役行/头注）、01-persistence-config（04 四行改判退役键拒绝+min_split_kib=0 行/12 表内退役键拒绝注/13 键集 37=6+8+23/头注）、12-reload-config（07 block_size_http→min_split_kib/08 throttle_burst_secs→upload_read_concurrency/头注）、05-ui-pieces（03 块→段/头注）、04-ui-conns（头注）、09-bt-engine（调优键列表）；⑥notes 三文件 + INDEX 同步** | **已交付（规格面）** | **APS parser + gherkin-ir-dry-checker 9 文件全通过（bb 1.13.230；输出 `.work/tmp/aps-v165/`）；dry-check findings 全为中文分词假阳性（沿 aps.md 先例）+ placeholder-variant 1 例规范化修复 + 05-05 既有刻意重复保留；Examples 表完整性核证（config-template-01 38 行、退役键零残留；13-http-turbo 22 场景/30 执行行）** |

### 遗留受限项（当前有效）
- **①全仓 >100 变异点存量 / CRAP 100 / CPD 222**：历轮登记沿袭；归 architect/cleaner 轮（MDM 移植后代码面大改，数值待 coder 轮后重测）。
- **②runner 断言同步面（归 QA 轮，本轮扩面）**：原清单沿袭（QA-DE-04/05/06/07、QA-HT-12/13b、QA-PC-05 退役行等）+ **本轮新增**：QA-HT-01/02/03/14/15/16 的开区间断言 → 封闭区间 `bytes=N-M`；QA-HT-14 覆叠 64B 断言（fixture 需覆叠区间比对能力）；QA-PC-04 退役键四行改判 + min_split_kib=0 行；QA-RC-07/08 示例键替换；QA-CT-01 37 键/165 行；QA-BE-14 调优键列表；QA-HT 新增 16–20 用例可执行化。`suite_config_template.py` 解析锚适配（沿 D94 账）。
- **③族C（gen 剥头副本漂移，1 例，归 cleaner 轮顺带）**：hardening_ui_backfill `gen_fixtures_match_product_sources`（沿袭；MDM 移植后 src/ui 四文件若变动需重新生成）。
- **④qa/09-bt-engine-qa.md:70 字面「ttp]」残留**：沿袭登记（磁盘为 [http] 伪象判别已两轮实证，QA 轮顺带核对 runner 字节）。
- **⑤MDM 性能基准线下口径**：完全移植轮后建议场景 = 同一 fixture 服务器 + 同一大文件 URL，EZR/MDM 各默认配置下载，对表「URL 确认 → 首字节落盘」时延与峰值速率（启动延迟 + 吞吐双指标）；弱网/高 RTT 收益更大（操作者线下）。
- **⑥blocks 引擎代码本体**：随 MDM 移植一并删除（`supervisor/workers.rs` 等），遗留受限项⑥就此关闭（coder 轮执行）。
- **⑦e2e 时限断言负载抖动家族**：沿袭（非不变量缺陷，复跑全绿即放行）。
- **⑧nightly 工具链缺位**：本轮 specifier 无编译面；coder 轮开工按 rust.md/environment.md 判别法装齐（rust-toolchain.toml 钉 nightly）。

### 实现定义值登记（累计覆盖为最新全量）
- 历轮登记（D81–D94）沿袭有效；**D84/D85/D86/D87②/D89/D92/D93 随 v1.3 退役或被替代**（退役注记见 phase-05 §11，编号保留追溯）。
- **本轮新增登记（源裁决可回退，操作者否决 = 直接以裁决文本 begin six-pack/specifier）**：
  - **D95 移植总纲**：HTTP 引擎 = MDM crates/engine 策略与算法整体移植；ezr supervisor/throttle/dns/error 策略代码废弃；workers.rs blocks 本体删除。
  - **D96 不移植面**：browserlike/UA 回落、sites、unlock/debrid、FTP、POST、request_cap、naming/destination/replace/server_time、事件日志 UI、repair/keep_state、remotezip、html、PAC、Route、allowed_certs、slow_disk、endless keep（ezr 暂停语义保留）、自定义 DNS 服务器选择器。
  - **D97 config 键面**：四键退役（含键拒绝启动——孤儿键无替代判别准则）；min_split_kib 默认 12；重定向 20；引擎重试常量（5 次/1s 翻倍封顶 30s/Retry-After 5min）；verify_overlap 64B；http_concurrency 1–128 沿袭（无 32 默认上限开关）。
  - **D98 sidecar 平铺格式**：(start,cursor,end) 平铺 + validators（ETag/LM/摘要族）；URL 字段不入库（FR-01-14 承载）；全部旧格式台账 = 无台账处理。
  - **D99 UI 口径**：信息列三态文案不变/重试节奏随引擎；shed 重连行回「正在连接...」；封闭区间请求日志断言面；Retired/Failed/Finished 引擎态存在、UI 活跃行不显示；pieces 网格 = 动态段。
  - **D100 失败分类映射**：MDM Error 族 → ezr FailKind/UI reason（Fatal/URL 失效/Transient/SlowConnection 即时重连）；探测面 4xx → URL 失效停等沿袭。

## 二、当前产出情况

### 本会话产物清单（20261128 MDM 完全移植轮）
1. `project/mission.md`：v1.64 → v1.65（修订注 + §4 核心参数改判 + 分期索引行）。
2. `project/mission/phase-05.md`：v1.2 → v1.3 全文重写（FR-05-01~14、D95–D100、验收标准改判）。
3. `project/features/13-http-turbo.feature`：v2 全文重写（22 场景：01–15 修订 + 16–20 新增 + 12/13b 退役留空）。
4. `project/qa/13-http-turbo-qa.md`：v2 全文重写（QA-HT-01–20）。
5. `project/features/01-download-engine.feature`：场景 09 重定向 10 → 20（链长 21）+ v1.65 头注。
6. `project/features/01-resume-sidecar.feature`：场景 01/03/07 修订 + v1.65 头注。
7. `project/features/01-config-template.feature`：37 键/165 行钦定原文内嵌替换 + 场景 01 步骤/Examples + v1.65 头注。
8. `project/features/01-persistence-config.feature`：场景 04（四行改判退役键拒绝 + min_split_kib=0 行）/12（表内退役键拒绝注）/13（37=6+8+23 键集）+ 场景清单行 + v1.65 注。
9. `project/features/12-reload-config.feature`：场景 07/08 示例键替换 + v1.65 头注。
10. `project/features/05-ui-pieces.feature`：场景 03 块→段 + v1.65 头注。
11. `project/features/04-ui-conns.feature`：v1.65 头注（信息列/Retired 口径沿袭说明）。
12. `project/features/09-bt-engine.feature`：调优键同理行（退役键移出、min_split_kib 入列）。
13. `project/notes/`：architecture.md/testing.md/environment.md 各追加本轮条目 + INDEX.md 三行同步。
14. `.tools/`：bb 1.13.230 + APS 仓 + MDM 参考仓（aeeb878）重建在位（非持久，随会话）。

### 验证证据
| 验证项 | 结果 | 命令（cwd = .tools/aps，前台串行） |
|---|---|---|
| gherkin-parser ×9 | 全 0 退出 | `bb gherkin-parser <feature> <out>.json` |
| gherkin-ir-dry-checker ×9 | 全 0 退出；findings = 中文分词假阳性沿 aps.md 先例 | `bb gherkin-ir-dry-checker <ir> <out>.dry.json` |
| Examples 完整性 | config-template-01 38 行（退役键零残留）；13-http-turbo 22 场景 | python 读 IR 核验 |

### 对账口径
- 规格场景总数：13-http-turbo 15 → 22（+5 新增 16–20、+08b/13b 既有；执行行 30）。
- config 模板键数 41 → 37（−4：block_size_http/throttle_burst_secs/throttle_sleep_ms/dns_race_grace_ms），行数 177 → 165（估计值，以 coder 实测为准——engineering.md 估计数字条款）。
- 退役裁决注记：D84/D85/D86/D87②/D89/D92/D93（7 项）+ D95–D100 新增（6 项）；phase-05 FR 面 13 → 14 条（FR-05-14 配置面）。

### 待办与待批（未决项）
- **D95–D100 可回退窗口开放**（操作者已预授权不需审批）：任一裁决否决 = 以裁决文本 `bin/swarm begin six-pack/specifier`（三层同步口径已建立）。
- **coder 轮接手面**：MDM 算法移植全量实现（phase-05 v1.3 FR-05-01~14 为唯一依据；MDM 参考仓 `.tools/misterdownloadmanager`）；ezr 原 HTTP 引擎代码删除面（supervisor 现行策略代码/throttle/dns/error 分类族/workers.rs）；proptest 不变量（planner 五不变量）；既有单测/集成/hardening 按新口径改写（D86/D93 退役 → MDM 节奏；blocks 套件面随引擎删除收编对账）；nightly 工具链先装。
- **QA 轮接手面**：遗留受限项② 全清单（runner 同步 + QA-HT 16–20 可执行化 + fixture 新能力）。
- **Inversion 访谈记录（受限退化沿先例）**：阶段 1 = 操作者 20261128 指令文本即答案（①范围 = 废弃+完全移植；②设计权威 = MDM 源码；③待办遗留不需审批；④ezr 不兼容旧版本；⑤CHAT 详述待办）；阶段 2 未触发（残余微口径 6 项全部以 MDM 源码为权威裁决 → D95–D100）；选择题清单（若操作者要求人工复核）= 1 移植范围边界（A 引擎全量【预答】/B 仅调度层）2 旧 sidecar 处置（A 一律无台账【预答】/B 校验迁移）3 MDM Options 无 ezr 来源项（A 不移植登记【预答】/B 全移植）4 config 退役键处置（A 孤儿键拒绝启动【预答】/B 静默废弃）5 min_split 默认（A 12 KiB=MDM【预答】/B 保留 256）6 request_cap（A 不移植【预答】/B 新增 config 键）。

### 移交建议
- 下一步 `bin/swarm begin six-pack/coder`：MDM 移植是全量实现轮（估算为历轮最大动面），建议 coder 轮按 FR-05-01→14 顺序切片提交（planner/请求分类 → worker/receive/Writer → limiter/dns/hosts → 恢复/保存/sparse/错误分类 → config 键面 → UI 对接），每切片跑对应规格关联单测。
- cleaner 轮顺带：族C 剥头副本重生成（移植后 src/ui 四文件比对）。
- architect 轮关注：engine 模块边界重划后的依赖方向（MDM 移植代码与 app 适配层边界）。
- 操作者线下：EZR/MDM 同机对表基准（受限项⑤）。

By specifier.
