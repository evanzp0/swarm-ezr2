# 05 · HTTP 引擎 MDM 完全移植（第五期详细需求）(v1.3)

> 期号：**05**（分期模式；需求大纲见 `../mission.md` v1.65 修订注）。
> 本文档是 05 的唯一需求详述依据：05 的 feature 与 QA 文档依本文生成，产出文件在文件名或头部标注期号 05（宪法第一章）。
> **v1.3 修订注（MDM 完全移植轮，specifier 规格轮 20261128；操作者 chat 指令「执行 six-pack/specifier,
> 废弃原有的 ezr 的 http 下载的策略、算法和代码，完全移植 mdm 的策略和算法；mdm =
> github.com/anefzaoui/misterdownloadmanager；完全移植 mdm，你根据这原则设计，如有待办与遗留问题，
> 不需要我审批」即 Inversion 阶段 1 答案；Inversion 受限退化沿 v1.45–v1.64 先例：chat 选择题面板
> 不可用（IM 通道），操作者指令文本即阶段 1 答案；阶段 2 未触发——残余微口径全部以 MDM 源码
> 为权威裁决，登记 D95–D100（可回退，操作者已预授权不需审批），选择题清单与受限项登记 handoff）**：
>
> **操作者裁决（20261128 chat 指令，本修订的唯一依据）**：
> ①范围 = 废弃 ezr 原 HTTP 下载的**策略、算法与代码**，**完全移植** MDM 的策略与算法；
> ②设计权威 = MDM 参考仓源码（`misterdownloadmanager` @ aeeb878，20261128 clone）；
> ③待办与遗留问题不需操作者审批，在 CHAT 中详细描述；
> ④ezr 不做旧版本兼容（沿 v1.45/FR-02-35）。
>
> **移植总纲（D95）**：EZR 的 HTTP 下载引擎整体废弃、MDM 引擎（crates/engine）的 HTTP 下载
> 策略与算法整体移植。移植面 = planner（动态分段全算法）、probe/请求与应答分类、worker 循环
> 与重试、receive/Writer 写流水线、Pace/慢连接剔除/链路计量、Limiter 限速、DNS 竞速、主机连接
> 上限学习、恢复状态与 validators、sync 保存策略、稀疏预分配、单流降级、磁盘错误分类与预检、
> 连接六态视图。v1.62 的「补丁层 + 动态调度器 + 磁盘流水线」三层叙事与设计权威
> `ezr-http-dw-tunning.md` 就此**整体退役**（该文档保留为历史方案，不再是任何条款的依据；
> 本文档按 MDM 源码逐条改写）。废弃的 ezr 代码面：`supervisor/`（download/dynamic/planner/
> single/workers/writer/http/finalize 的现行策略与算法）、`engine/throttle.rs`（Throttle 桶）、
> `engine/dns.rs`（race_with_grace）、`engine/error.rs`（分类族）——由 MDM 对应算法的移植实现
> 整体替换（`workers.rs` blocks 引擎代码本体一并删除，遗留受限项⑥就此关闭）。
> **不移植面（D96）**：BT 引擎零接触；ezr 应用层（TUI/任务管理/slots 队列/checksum 七算法/
> config 体系/URL 权威模型）零接触；MDM 的应用层与无 ezr 来源机制不移植（浏览器化连接/
> UA 回落/sites 站点规则/unlock 服务账号/FTP/POST 表单/request_cap/命名落盘/事件日志 UI/
> repair/PAC/网卡路由/allowed_certs 等——逐项见 D96）。
> **沿袭条款**：v1.1（族A/族B）与 v1.2（规格修复 + 「信息」列）的历史裁决以 D83–D94 登记
> 在案；本修订使其中的策略类裁决（D84/D86/D87②/D89/D92/D93 等）被 MDM 权威语义替代或
> 退役，逐项注记见 §7 裁决登记。`ttp]` 字样均为终端显示伪象，磁盘字节完好（v1.2 核证沿袭）。

## 1. 范围与目标

- **目标**：EZR 的 HTTP 下载行为 = MDM 引擎行为（同栈 reqwest/hyper/tokio，逐算法移植）——
  启动时延、峰值吞吐、分段调度、限速形态、重试节奏、恢复语义全部对齐 MDM 源码。
- **判断原则**：**完全移植 MDM**（操作者钦定）——凡 MDM 引擎有的 HTTP 下载策略/算法，
  一律按 MDM 源码语义移植；凡 ezr 独有且 MDM 没有的 HTTP 下载策略/算法/代码，一律废弃；
  ezr 应用层职责（任务管理/TUI/持久化位置/BT/校验）不在废弃面。
- **回退开关**：不存在（v1.1/族A 沿袭）——HTTP 引擎仅存 MDM 移植单形态。

## 2. 分段调度：planner 完整移植（FR-05-01）

移植 MDM `planner.rs` 全算法（纯逻辑，零 I/O；IDM 规则同源）：

- **段模型**：段 = 半开区间 `[start, end)`，含 `cursor`（`[start, cursor)` 已落盘）、
  `reserved`（`[cursor, reserved)` 已认领在写）、worker 归属、receiving 标记、given_at 时刻。
  分段表按 start 排序、连续、恰好平铺 `[0, size)`。
- **三原语 + 辅助**（语义逐条 = MDM）：
  - `assign(w, after)`：①领取未分配未完成段（优先接续自己刚完成的段 `after`）；
    ②IDM 重指派规则——抢占「紧随自己刚完成段之后、其 worker 尚未收到任何数据且已等待
    ≥ `preempt_after`」的段（被抢占 worker 的后续 claim 一律 `Lost`）；
    ③把**剩余最大的段**对半切分、取上半——切分点按 `reserved` 计（永不切走在写字节），
    对齐 `ALIGN = 4096`（对齐后不足 min_split 则不对齐），两半均 ≥ `min_split` 才切，
    否则返回 None（worker 收尾退出）。切分选段按 `(unclaimed, Reverse(index))` 取最大。
  - `claim(w, pos, len)`：写前认领——pos ≠ reserved 或段已易主 → `Lost`；
    授予 `min(len, unclaimed)`，段被切短则授予更少（退款语义由调用方执行）。
  - `commit(w, upto)`：写后提交——cursor 前进（钳制在 `[cursor, reserved]`）；
    段完成（cursor == end）则释放 worker 并返回段尾（触发 `assign(after = end)`）。
  - `release(w)`：worker 失败/被叫停——段回归未分配，未提交认领作废（reserved 回退到 cursor）。
  - `absorb_next(w)`：探测流（整文件应答）在段尾吞并紧邻的下一段——仅当下一段无 worker、
    无已认领字节；被吞并段的 worker 其 claim 回 `Lost`。
  - `make_single(w)`：服务器证实无 Range 时整文件归还给持 0 段的探测连接、永不再切分
    （min_split = u64::MAX）；w 不持 0 段或其余段已有数据 → 失败（下载按单流重启）。
  - `room(w, at)`：该 worker 在 `at` 之后还剩多少段内字节（段被切短后随之收缩）。
- **启动坡道（INITIAL_PIECE）**：初始连接数 = `min(最大并发, ⌈size / 64 KiB⌉)`（至少 1）——
  小文件不一次性开满并发（避免切碎与重复取数），后续切分仍可下探到 min_split。
  探测连接恒为 worker 0 且在其余连接生成前预占 0 段。
- **preempt_after** = 2 × probe 首字节时延（TTFB），下限 20 ms。
- **min_split**：初始 = config `http.min_split_kib`（KiB；**默认 12** = MDM 默认值/IDM ~12.6 KB，
  原 256 退役，D97②）；尾段竞争时不再自适应降半（MDM 无此机制，v1.62 D85 的「自适应降半」退役）。
- **不变量**（proptest 钉死，coder 职责）：段区间互不重叠；`cursor ≤ reserved ≤ end`；
  Σ(段长) 恒等于文件大小；切分点按 `reserved` 计算故窃取永不覆盖在途字节；
  并发 assign/claim/commit/release 无死锁；平铺校验（resume 时逐段衔接、首段 start=0、
  末段 end=size）失败即 InvalidState。
- **完成判定** = planner 聚合（全部段 `cursor == end`）；100 ms tick 的 O(blocks) 扫描不存在。
- 验收：13-http-turbo-01/02/03/16。

## 3. 探测、请求与应答分类（FR-05-02）

移植 MDM 首请求与 request() 应答分类语义：

- **探测即数据流**：新任务首个请求 = **不带 Range 的完整 GET**（MDM D-007/IDM 同源），
  其响应体即为 worker 0 的数据流，从字节 0 立刻开始落盘；其余连接并行领取切分段。
  （v1.62「probe 先探测后下载」两段式与 v1.63「裸 GET + 开区间」口径一并退役。）
- **Range 三态学习**（AtomicU8 语义 = MDM）：`UNKNOWN`（初始）/`YES`（任一请求得到 206）/
  `NO`（探测应答 200 且非同文件续流）。206 → YES 并唤醒等待中的其余连接；200 → NO。
- **续传/分段请求形态**：**封闭区间** `Range: bytes=<from>-<to-1>`（from 含 64 B 覆叠回退
  见 FR-05-08；to = 段尾 = `from + room(w, from)`）——MDM 全程封闭区间，
  ezr 现行「开区间 `bytes=<from>-`」口径**退役**（D99③）。
- **If-Range 校验器**：段请求携带 `If-Range`（优先 ETag，无则 Last-Modified）；校验器族
  （validators）= { ETag, Last-Modified, 全文件摘要族 }，摘要族 = MDM `digests_of` 同源
  （x-oss-hash-crc64ecma / x-cos-hash-crc64ecma / x-linked-etag(SHA-256) / x-goog-hash /
  Repr-Digest，忽略 Content-MD5/Digest 等分片摘要）。
- **应答分类**（逐臂 = MDM `request()`）：206 且摘要不符 → FileChanged；206 → 学习 YES；
  200 且 per-node ETag（同长 + 同 Last-Modified、ETag 异）→ 丢弃 ETag 改按日期校验、
  本请求按 RangesUnavailable 重试；200 且同校验器/同长 → RangesUnavailable（重试）；
  200 且摘要可证为另一版本 → FileChanged；200（无 Range 证据）→ 学习 NO → NoRanges；
  429/503 → Throttled（携带 Retry-After）；其余 4xx/5xx → Status(status)。
  收到应答后校验 Content-Range 起点 = 请求起点、总长 = 文件大小，否则 BadRange。
- **响应体校验**：段请求应答若为网页（Content-Type HTML 且非命名文件）→ GotWebPage
  （URL 失效类，不落盘）；另见 FR-05-12 错误分类。
- **重定向**：跟随上限 **20**（MDM MAX_REDIRECTS = 浏览器 20；ezr 现行 10 退役，D97③）；
  超限 = 重定向次数超限（既有失败文案不变）；ezr URL 权威模型沿袭（FR-01-14：原始 URL
  唯一权威，各连接独立跟跳转，final_url 不持久化）。
- **压缩**：分块请求禁用内容压缩沿袭（01-download-engine-13，MDM 同为裸 GET/Range 不解压
  语义——按 MDM client 构造对齐）。
- **单 Client**：每下载一个 reqwest Client，全部连接共享（keep-alive 连接池复用、
  TLS 会话恢复）；在飞请求数 = worker 数，天然有界。探测/重试共用该 Client。
- 验收：13-http-turbo-01/14/17/20、01-download-engine-08/09/10/13。

## 4. worker 循环与重试（FR-05-03）

移植 MDM `worker()`/`supervise()` 循环语义：

- **连接生命周期**：assign → Connecting → receive_all（首数据流或 Range 请求）→
  段完成 commit → `after = Some(end)` 继续下一 assign；被抢占（Ok(None)）→ release 后重试；
  段完成即续领，无事可做 → 收尾退出。
- **重试节奏**（D97④ = MDM）：连接失败计数上限 **5**（max_retries）；第 1 次重试**立即**，
  其后 `retry_delay`（1 s）按 2^(failures-2) 指数翻倍、**封顶 30 s**；
  Throttled 且服务器给了 Retry-After → 优先采用（封顶 5 分钟）。
  **有进展重置**：自上次失败以来该连接收到了新字节 → 计数清零（反复断连不失败沿袭，
  语义与 MDM `at_last_failure` 一致）。
- **计数耗尽**：该连接转连接级失败；仍有其他 worker 活跃 → 本连接静默退场（任务不受影响）；
  已无其他 worker → 升级任务级失败（错误文案 = MDM 错误描述映射，FR-05-12）。
- **link_dead 不重试**（MDM 语义，替代 v1.63/D93 的「4xx 原地重试」）：401/403/404/410/416、
  GotWebPage、NotTheFile、BotCheck、站点拒绝 → 链接失效类，该连接失败退场
  （others-working 退场规则同上）；唯余此连接 → 任务级失败。
  **探测面（首请求）失败 → 任务级失败**沿袭（401/403/404/410/416 → URL 失效停等，QA-RB-07）。
- **慢连接剔除（shed）**：`SlowConnection` 即时重连（非失败、不耗重试计数）：
  剔除判据 = 在其应答上运行 ≥ SHED_GRACE(2 s) 后，净网络速度（扣除等盘时间）
  < 单连接可达速度(shed_bar) / 4，且按此速度剩余工作量 > 2 s（SHED_LEFT）。
  shed_bar = min(单连接历史最优(≥256 KiB 且 ≥100 ms 的应答速度 yardstick)，
  链路 1 s 窗口聚合最优 ÷ 在收连接数)；等盘长 ≥ 50 ms（DISK_HELD）后 2 s 内不剔除
  （盘不背锅）；**限速开启或 request_cap 开启时不剔除**（后者 ezr 无来源恒关）。
- **主机连接上限学习（Retired）**：额外连接被拒（403/429/503/连接拒绝族）而其他连接
  在收数据 → 学习该主机连接上限（2 次观察后生效、10 分钟衰减、在收数上浮修正）；
  上限生效且活跃连接 ≥ 上限 → 被拒连接转 `Retired{host, cap}` 退场（不再耗重试）。
- **无 Range 臂**：非探测连接在 NO 态请求 → 退出（探测连接仍持段则 OK，否则 NoRanges 错误）；
  探测流跑完段尾经 absorb_next 继续吞并（MDM `keep_reading`）。
- 验收：13-http-turbo-04/05/17/18/19、01-retry-backoff（应用层族不变）。

## 5. receive 与写流水线（FR-05-04）

移植 MDM `receive()`/`Writer` 语义：

- **先付限速再认领**：每个网络 chunk 先 `acquire(len)`（任务级 + 全局两个桶），
  等待可被停止打断；claim 后未授予部分 `refund`（限速桶与计数同步退回）。
- **覆叠校验（verify_overlap）**：续传/接管段的请求从 `from - 64` 起始（重取最后 64 B
  与盘上字节逐字节比对，MDM/IDM resume overlap check），不符 → FileChanged；
  首 64 B 校验通过后正常写。verify_overlap = 64 B（MDM 默认，D97⑤，无 config 键）。
- **Writer 批量写**：每连接收包入 256 KiB 定容 batch（BATCH；缓冲从回收池取，
  绝不扩容），满或 1 s（FLUSH_EVERY）封口入队；写盘任务把队首若干 batch 合并成
  **≤ 4 MiB**（WRITE_MAX）的一次定位写（unix `pwrite`/windows `seek_write`，spawn_blocking），
  批次按序直写不拼接拷贝；写完的 batch 回收池复用（上限 WRITE_MAX/BATCH 个）。
- **内存跑道**：单连接在途内存（writing + queue + filling，按 batch 容量计）上限 =
  HOLD_MAX ÷ 连接数（HOLD_MAX = 256 MiB 全局跑道）；超限时连接 `step` 等待写盘回收；
  网络收包永不等磁盘（跑道未满时）。
- **提交**：写盘完成 → `commit(pos)` → 段完成返回段尾。速度采样每 500 ms 一次
  （conn speed 更新 + shed 检查）。
- **暂停/停止**：连接在 chunk 边界与限速等待处响应取消；已认领字节全部落盘后才返回
  （state 保存不悬空）。
- 验收：13-http-turbo-09（端到端字节正确性）、16（坡道）。

## 6. 限速、DNS 与主机上限（FR-05-05 ~ FR-05-07）

### FR-05-05 Limiter 令牌桶移植（替代 engine/throttle.rs）
- 每下载一个任务级桶 + 可选全局共享桶（`global_max_download_speed` / 任务级限速语义沿袭）。
- 桶语义 = MDM：令牌可负（债务）；**burst 窗 = 1.0 s**（BURST_SECS 常量）；
  **限速热改生效于在途等待**（earned-mark 机制：等待按「桶累计产出」记账，改速率后按
  新速率折算剩余债务，改无上限立即放行并豁免债务）；
  `refund` 归还未授予字节；不限速时 acquire 即返（快路径）。
- `throttle_burst_secs` / `throttle_sleep_ms` 配置键**整键退役**（D97⑥：MDM 无用户可调
  burst/sleep 旋钮——burst 为引擎常量 1.0 s、无睡眠下限）；config 含任一键 → 拒绝启动。
- 限速计费点 = 收包侧（socket 读节流，TCP 反压服务器，= MDM）。
- 验收：13-http-turbo-06、01-throttle-proxy 全族（语义不变面）。

### FR-05-06 DNS 竞速移植（替代 engine/dns.rs）
- 移植 MDM `dns.rs` 解析策略：多解析源并发竞速——起步 stagger **250 ms**、竞速预算 **3 s**
  （RACE_BUDGET）、**赢家缓存 300 s**（REMEMBER）；IP 候选经**可达性优先**
  （TCP 连接竞速 reachable_first）排序；解析超时 = min(connect_timeout 折算, 预算)。
- `dns_race_grace_ms` 配置键**整键退役**（D97⑦：MDM 竞速语义固定常量）；
  config 含该键 → 拒绝启动。
- 验收：13-http-turbo-16（解析不阻塞首请求（fixture 即解析成功））+ 01-download-engine 回归
  （ezr 沿用系统解析 + hosts 语义面；MDM 自定义 DNS 服务器选择器不移植，D96）。

### FR-05-07 主机连接上限学习（HostLimits 移植）
- 学习表（进程级共享）：被拒时记录「被拒时在收连接数」为上限候选，**2 次观察后生效**；
  上限随在收数上浮修正；**10 分钟**无观察即遗忘（DECAY）。
- 生效面：supervise 启动时按学习上限收窄初始连接数；worker 被拒不重试转 Retired（§4）。
- 事件文案：`<host> 允许 <n> 个连接`（Retired/学习事件，MDM 同源；「信息」列 Retired 文案
  见 FR-05-13——ezr 并发面板活跃行口径下通常不可见，登记 D99④）。
- 验收：13-http-turbo-19。

## 7. 恢复、保存与磁盘（FR-05-08 ~ FR-05-11）

### FR-05-08 恢复状态与 sidecar（MDM ResumeState 移植；D98）
- **sidecar 动态格式 = MDM 平铺段**：`segments = [(start, cursor, end), …]`（SavedSegment
  平铺、恰好覆盖 `[0, size)`）；恢复时平铺校验失败 → InvalidState → 按无台账处理。
  另含：文件大小、validators（ETag/Last-Modified/摘要族）、已下载字节、content_type、
  校验期望、任务元信息。**不含任何 URL 字段**（沿 FR-01-14/FR-01-20：URL 权威 = 任务注册表
  原始 URL，续传各连接从原始 URL 独立跟跳转；MDM final_url/refresh_final_url 等价面由
  该模型承载，D98①）。`scheduler` 字段退役（引擎单形态，键面消亡）。
- **旧格式台账处置**（v1.45 不兼容钦定 + D90 精神扩全量）：凡非上述平铺格式的磁盘台账
  （含 v1.62 动态格式、blocks 格式、无 scheduler 字段形态）**一律按无台账新任务处理**——
  原名静默从头下载、不改道、无提示，新台账同路径首次 flush 自然覆写，不特判不清理。
- **续传请求**：首连接 = 开机即从 `cursor - 64`（覆叠）到段尾的封闭 Range；
  校验器（If-Range）随请求携带；206 起点不符/BadRange/FileChanged 处置同 §3。
- **per-node ETag**：ETag 变化而 Last-Modified 与长度一致 → 丢弃 ETag 按日期继续（不失效）；
  服务器不再返回校验头 → 按长度继续 + 64 B 覆叠兜底（不符才 FileChanged）——
  一致性失效判定由「请求期 If-Range/覆叠校验」承载（01-resume-sidecar-07 同步修订）。
- 验收：13-http-turbo-11/14、01-resume-sidecar 全族。

### FR-05-09 保存与 sync 策略（MDM Synced 移植）
- **保存节律**：save_interval = 2 s（MDM 默认）；tick 到点时按需保存。
- **flush 条件**：距上次 flush 新增 ≥ `clamp(size/16, 1 MiB, 64 MiB)` 或距上次 ≥ 30 s，
  **且 fits 门控通过**（预计 flush 时长 ≤ 剩余内存跑道可撑时间的 3/4；预计时长按上次
  实测速率折算、无实测按 150 MiB/s 保守估）——盘慢时 flush 自动停摆、系统页缓存均匀
  背压（MDM #782 语义）；flush 仅在确有新字节时执行。
- **声称字节纪律**：快照 → flush → 写台账（snapshot→flush→write）；台账永不声称未 flush
  的字节（kill -9 后声称值 ≤ 盘上实际）。元数据变化（validators/URL）在 flush 间隙独立落盘。
- 停止/收尾强制保存（flush 全量）沿袭。
- 验收：13-http-turbo-11。

### FR-05-10 稀疏预分配（对齐 MDM sparse.rs）
- 平台分支沿袭（Windows FSCTL_SET_SPARSE / Linux ntfs3 xattr → fallocate KEEP_SIZE 回退），
  **新增 MDM 门槛**：仅名义大小 ≥ **5 MiB**（MIN_SIZE）的文件做稀疏标记；失败静默回退
  `set_len`（D88 沿袭）。
- 验收：13-http-turbo-10。

### FR-05-11 磁盘空间预检与错误分类（MDM disk.rs 移植）
- **预检**：开工前检查目标盘剩余空间 ≥ 文件大小 + 安全边际，不足 → 任务级失败
  「磁盘空间不足（需要 X，剩余 Y）」（NoSpace）。
- **错误分类族**（= MDM，映射 ezr FailKind/UI reason，D100）：磁盘写入失败（Io）、
  磁盘已满（DiskFull，含 StorageFull/QuotaExceeded 归类）、目标盘不可达
  （DestinationUnavailable）、目标盘被移除（DriveGone，ENXIO/ENODEV 等 os error 归类）、
  超出文件系统上限（TooBigForDrive，4 GB FAT 形态）——全部任务级 Fatal；
  网络族（连接失败/超时/传输中断/TLS）= Transient（TLS 握手失败除外：Fatal）；
  URL 失效族（401/403/404/410/416/GotWebPage/BotCheck/站点拒绝）停等；
  FileChanged/NoRanges(不可保)/BadState/Cancelled/ChecksumMismatch = 任务级。
  探测面 4xx → URL 失效停等沿袭（QA-RB-07）。
- 验收：01-retry-backoff-09（预检沿袭）、13-http-turbo-20。

## 8. 单流降级与 UI 口径（FR-05-12 ~ FR-05-13）

### FR-05-12 单流降级（MDM single_stream 移植）
- **无 Content-Length**（size 未知）→ 单流顺序下载（限速付款/可停；无台账不续传）；
  「不支持断点续传」详情语义沿袭。
- **NoRanges 收口**：探测流可保（absorb/make_single 成功）→ 探测连接单连接完成整文件
  （不再重发请求）；不可保 → 单流重启（新 GET）。
  ezr toast 文案沿袭（「⚠ 服务器不支持断点续传，单线程下载: <name>」）。
- **有进度续传任务遇 200**（曾 YES）：同校验器 → RangesUnavailable（重试）；
  校验器不符/文件变 → FileChanged（URL 失效/一致性臂按 D100 分类）——
  ezr v1.63「服务器本次未支持断点续传（Range），保留断点稍后重试」口径**退役**
  （MDM 无此臂；对应场景改按上述分类，13-http-turbo-15 同步修订）。
- **无长度流停止**（endless keep 等价）：无长度任务被暂停且已有字节 → 已收字节保留为
  最终产物（MDM `Download::keep` 语义映射 ezr 暂停臂——任务转「已完成（截断保留）」?
  **否**：ezr 无 keep 语义，登记遗留——无长度任务暂停 = 既有暂停语义（进度保留、
  不支持续传），不做 MDM keep（D96⑤））。

### FR-05-13 UI 口径（v1.3 修订：MDM 移植后的呈现）
- **「信息」列**（D94 v1.2 沿袭 + D99①）：三态映射不变
  （`正在连接...` / `正在接收数据...` / `正在重试：<原因>`）；重试节奏随 §4 改判
  （首次立即、其后 1/2/4/8/16 s 翻倍封顶 30 s、Retry-After 优先——显示的 `<原因>` =
  失败分类 reason，节奏不改文案）；shed 即时重连表现为连接行回「正在连接...」。
- **pieces 网格** = planner 动态段（SegmentView：🟩 已落盘 [start,cursor) / 🟦 在写
  [cursor,reserved) / ⬜ 未认领）；「块」语义退役（refine_http_grid/block_size 概念消亡）；
  分块行 `<已下载>/<总字节> · 动态分段`（D83）沿袭。
- **并发面板活跃行口径沿袭**（FR-01-96 v1.34）：已断开行（Finished/Failed/Retired）不显示
  ——引擎六态在 ConnView 完整存在（单元/QA 可断言），UI 呈现面不变（D99④ 登记遗留）。
- 其余 UI（任务行/头部仪表/速度统计/限速/代理/槽位）不变。

## 9. 配置面（FR-05-14，D97）

```toml
[http]
min_split_kib = 12            # 切分下限（KiB；默认 12 = MDM/IDM；D97②）
```

- **退役键（整键退役 = 未知键拒绝启动，沿 D91 先例）**：`block_size_http`（MDM 无块概念，
  分段全由 planner 决定；pieces/p分块行网格改动态段）、`throttle_burst_secs`、
  `throttle_sleep_ms`、`dns_race_grace_ms`（MDM 固定常量）。
  沿 FR-02-35：config 任意已知表含上述键（任意值）→ 拒绝启动并报错；
  顶层平铺旧格式（无表头）整体忽略口径不变（01-persistence-config-12）。
- **键值修订**：`min_split_kib` 默认 256 → **12**（合法域正整数不变，非法拒绝启动）。
- **不变**：`http_concurrency`（默认 8、合法域 1–128 = MDM 引擎 clamp
  1..=MAX_CONNECTIONS_UNLOCKED；MDM 的 32 默认上限开关 many_connections 为应用层概念，
  ezr 无此开关，登记 D97⑧）；`max_retries`/`backoff_*`（应用层任务级重试族沿袭，
  与引擎连接级 5 次重试叠加关系 = 引擎耗尽才升级任务级）；B 类快照族口径沿袭
  （任务启动烘焙，热加载只影响新任务）。
- **配置模板（FR-01-85）**：41 键 → **37 键**（退役 4 键各去三行式，行数 177 → 约 165，
  估计值以实测为准）；`min_split_kib` 注释默认值与取值说明同步（12）。
- 验收：13-http-turbo-13、01-config-template-01、01-persistence-config-04/12。

## 10. 验收标准

1. **正确性回归**：现有规格与 QA 全绿（本轮同步修订的 8 份规格按新口径执行）；
   planner proptest 不变量全绿（§2）。
2. **端到端 QA**：`qa/13-http-turbo-qa.md` 全用例通过（P/F/S 口径）。
3. **性能基准（操作者线下验收口径，不进自动化 QA）**：与 MDM 同机同文件对比（EZR/MDM
   同一 fixture 服务器、同文件、默认配置对表——「URL 确认 → 首字节落盘」时延与峰值速率；
   本沙箱无 MDM 运行环境，登记 handoff）。通过线 = 逐算法移植后的行为一致 + 操作者实测认可。
4. **断点续传**：kill -9 后重启，sidecar 声称字节数 ≤ 磁盘实际且续传完成字节完整。
5. **无回退开关**（沿袭）：HTTP 引擎单形态。

## 11. 裁决登记（D83–D94 沿袭 + D95–D100 本轮新增，可回退）

| # | 裁决 | 内容 | 依据 |
|---|---|---|---|
| D83 | 分块行动态口径 | `<已下载>/<总字节> · 动态分段`（沿袭有效） | v1.62 沿袭 |
| D84 | ~~scheduler 键口径~~ | （v1.1 已退役，沿袭） | v1.1/族A |
| D85 | ~~min_split 自适应降半~~（**v1.3 退役**） | 原默认 256 KiB + 尾段自适应降半退役；**默认 12 KiB、无自适应降半**（MDM min_split = 12 KiB、切分仅半分规则），键 min_split_kib 保留（D97②） | MDM planner.rs/Options::default + 操作者「完全移植」 |
| D86 | ~~原地重试节奏~~（**v1.3 退役，被 §4 MDM 语义替代**） | 原「首次立即 + 1/2/4/8 s 上限 5 次后任务级」退役——新语义 = MDM：首次立即 + retry_delay(1 s) 指数翻倍封顶 30 s、Retry-After 优先（5 min 封顶）、有进展重置、耗尽连接级退场（others-working 规则）；数值形态在 ≤5 次窗内与旧口径一致 | MDM retry_wait/worker + 操作者「完全移植」 |
| D87 | ~~probe 裸 GET + 开区间~~（**v1.3 部分退役**） | 裸 GET 探测即数据流沿袭成立（= MDM）；「开区间 Range」口径退役改封闭区间（D99③）；「Range 失效臂保留既有 toast/失败口径」改判（FR-05-12） | MDM download.rs/run_fresh + 操作者「完全移植」 |
| D88 | 稀疏回退 | 静默回退 set_len（沿袭有效；新增 MIN_SIZE 5 MiB 门槛 = MDM） | MDM sparse.rs |
| D89 | ~~sidecar 双格式~~（**v1.3 退役，被 D98 替代**） | 原 (start,cursor) 段列表 + scheduler 字段退役；改 MDM 平铺 (start,cursor,end) + validators；旧格式一律无台账处理 | MDM planner.rs SavedSegment/state.rs + 操作者「完全移植」 |
| D90 | blocks 旧台账处置 | 按无台账处理（沿袭有效，适用面扩至全部旧格式——D98） | v1.1/族A + 本轮扩面 |
| D91 | scheduler 键退役形态 | 拒绝启动（沿袭有效；退役键拒绝启动口径向 FR-05-14 四键推广） | v1.1/族A |
| D92 | ~~block_size_http 语义~~（**v1.3 退役**） | 原「网格细化上限」随键整键退役（FR-05-14）；pieces 网格改动态段 | MDM 无块概念 + 操作者「完全移植」 |
| D93 | ~~族B 失败/降级语义~~（**v1.3 退役**） | 「4xx 原地重试 + Transient」被 MDM 分类替代：link_dead（401/403/404/410/416/GotWebPage 等）不重试、transient（429/503/5xx/网络族）重试、有进展重置；探测面 4xx 停等沿袭 | MDM Error::link_dead/transient + 操作者「完全移植」 |
| D94 | 「信息」列 | 三态映射沿袭有效；Retired 文案引擎层存在、UI 活跃行口径不可见（D99④） | v1.2 沿袭 |
| **D95** | **移植总纲** | HTTP 引擎 = MDM crates/engine 策略与算法整体移植（面清单见 v1.3 修订注）；ezr 原 supervisor/throttle/dns/error 策略代码废弃；workers.rs blocks 本体删除 | 操作者 20261128 指令①「废弃…完全移植」 |
| **D96** | **不移植面** | MDM 应用层与无 ezr 来源机制不移植：browserlike/UA 回落、sites 站点规则与 per-node 域名特殊化、unlock/debrid、FTP、POST 表单、request_cap（无来源，恒 None——相关代码臂不移植）、naming/destination/replace_existing/server_time（ezr 应用层自有）、事件日志 UI、repair/keep_state、remotezip、html 页面解析、PAC、网卡路由（Route）、allowed_certs、slow_disk（测试钩子除外）、endless keep（ezr 暂停语义保留，D96⑤）、自定义 DNS 服务器选择器（ezr 沿系统解析 + hosts） | 操作者 20261128 指令①「策略与算法」边界 + ezr 应用层职责保留 |
| **D97** | **config 键面** | ①`block_size_http`/`throttle_burst_secs`/`throttle_sleep_ms`/`dns_race_grace_ms` 四键整键退役（含键 → 拒绝启动，D91 口径推广；顶层平铺旧格式忽略沿袭）；②`min_split_kib` 默认 12；③重定向上限 10 → 20（MDM）；④重试节奏常量（max_retries=5/retry_delay=1 s/封顶 30 s/Retry-After 5 min）为引擎值（ezr 应用层 auto_retry 退避族 8→60 s 不变、叠加在引擎耗尽之后）；⑤verify_overlap = 64 B 引擎值（无键）；⑥`http_concurrency` 1–128 沿袭（无 32 默认上限开关，D97⑧ 应用层差异登记） | MDM Options::default/MAX_REDIRECTS/limiter + D91 先例 + FR-05-05 沿袭 |
| **D98** | **sidecar MDM 平铺格式** | segments = (start,cursor,end) 平铺（tiling 校验失败 = 无台账）；validators（ETag/LM/摘要族）入库；**URL 字段不入库**（FR-01-14/20 URL 权威模型沿袭，续传各连接从原始 URL 独立跟跳转）；全部旧格式台账 = 无台账处理（D90 扩面） | MDM state.rs/planner.resume + ezr FR-01-14/FR-01-20 沿袭 |
| **D99** | **UI 呈现口径** | ①信息列三态文案不变、重试节奏随引擎（不改文案）；②shed 重连 = 行回「正在连接...」；③分段请求形态改封闭区间（请求日志断言面 `bytes=N-M`）；④Retired/Failed/Finished 引擎态存在、并发面板活跃行不显示（沿 FR-01-96 v1.34）；⑤pieces 网格 = 动态段（块语义退役） | MDM ConnStatus/SegmentView + ezr FR-01-96/D83/D94 沿袭 |
| **D100** | **失败分类映射** | MDM Error 族 → ezr FailKind/UI reason：Fatal（Io/DiskFull/DriveGone/DestinationUnavailable/TooBigForDrive/FileChanged/NoRanges 不可保/BadState/Cancelled/ChecksumMismatch/TLS）；URL 失效（401/403/404/410/416/GotWebPage/BotCheck/Refused/LinkExpired）；Transient（Throttled/500/502/504/Http connect·timeout·body/ShortBody/Net 非 TLS）；SlowConnection = 即时重连非失败；探测面 4xx → URL 失效停等（QA-RB-07 沿袭） | MDM Error::is_fatal/transient/link_dead + ezr 失败分类沿袭 |

> 注：D83/D88/D90/D91/D94 沿袭有效；D84/D85/D86/D87②/D89/D92/D93 就此退役或被替代
> （退役 = 条款不再约束实现，编号保留追溯）。本轮实现定义值（D95–D100）源裁决可回退，
> 操作者否决 = 以裁决文本 begin six-pack/specifier（沿 handoff 待批先例）。
