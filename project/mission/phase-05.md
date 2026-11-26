# 05 · HTTP 引擎提速改造 P0–P2（第五期详细需求）(v1.0)

> 期号：**05**（分期模式；需求大纲见 `../mission.md` v1.62 修订注）。
> 本文档是 05 的唯一需求详述依据：05 的 feature 与 QA 文档依本文生成，产出文件在文件名或头部标注期号 05（宪法第一章）。
> **设计权威**：操作者提供的 `project/ezr-http-dw-tunning.md`（ezr HTTP 引擎提速改造方案）——
> 本文档把该方案中 **P0（补丁层）+ P1（调度层）+ P2（磁盘层）** 转写为可验收需求条款；
> **P3（增益层：BDP 自适应 / 并发预热 / 慢连接剔除 / 主机连接上限学习 / 切分粒度持续校准 / h2 降级）本轮不实现**，
> 其条款不进入任何规格与 QA 文档。
> **操作者裁决（20261125 chat 指令，Inversion 阶段 1 答案）**：
> ①范围 = P0、P1、P2；②P0.3 中「`DEFAULT_BLOCK_SIZE_HTTP` 提到 4 MB」**去除**，直接采用
> **下限 256 KiB**（`DEFAULT_BLOCK_SIZE_HTTP` 维持 1 MB 不变）；③其余一切以方案为准；
> ④判断原则 = **能提高下载速度**；⑤ezr 不做旧版本兼容（沿 v1.45/FR-02-35）。
> Inversion 受限退化沿 v1.45–v1.61 十一轮先例：chat 选择题面板不可用（IM 通道），操作者
> 指令文本 + 设计文档全文即阶段 1 答案；残余微口径 7 项按源裁决登记 D83–D89（可回退）。
> **代码实证基线（specifier 本轮核实）**：`src/engine/supervisor/workers.rs`（每 chunk `file.write_all` +
> `*failure` 全任务停止）、`single.rs`（单流逐 chunk 写盘）、`engine/throttle.rs`（`wait_ms = max(left/rate*1000, sleep_ms)` 20 ms 下限）、
> `download.rs:30`（probe `Range: bytes=0-` 后 `drop(resp)` 丢弃探测体）、`model/chunk/plan.rs`
>（`MIN_HTTP_BLOCK_SIZE = 1 MB`）、`model/config.rs`（`DEFAULT_HTTP_CONCURRENCY = 4`、
> `DEFAULT_THROTTLE_BURST_SECS = 0.5`）、CLI/dialog 并发范围 `1..=64`——与方案中的现状描述一致。

## 1. 范围与目标

- **目标**：ezr 与 MDM 底层栈相同（reqwest/hyper/rustls/tokio），差距在调度层——不动技术栈，
  通过补丁层修复 + 动态分段调度 + 磁盘写流水线追平 MDM 档位的下载吞吐。
- **回退开关**：所有新路径放在配置开关 `http.scheduler = "dynamic" | "blocks"` 之后（默认
  `dynamic`）；旧 blocks 引擎完整保留、随时一键回退（方案「风险控制」条款）。
- **判断原则**：以「提高下载速度」为功能取舍准绳；凡与提速无关的重构不进本期。

## 2. P0 补丁层（FR-05-01 ~ FR-05-05）

### FR-05-01 写合并（P0.1）
blocks 引擎 worker 与单流路径的逐 chunk 同步写盘改为 **256 KiB 聚合缓冲**（方案 WBuf）：
- worker 收到的每个网络 chunk 先做内存拷贝入聚合缓冲（缓冲起点 = 绝对偏移；写入位置不连续
  时先 flush 再开新缓冲）；缓冲满 256 KiB 或任务收尾/中止时一次性 `write_all` 落盘。
- 网络收包从「每 chunk 1 次 seek + 1 次 syscall + 1 次阻塞池调度」变为「1 次内存拷贝」。
- 并发连接面板 ConnView 更新从每 chunk 一次降为每 flush 一次（UI 100 ms tick 无观感差异）。
- 适用面：blocks 引擎 `workers.rs` 与 `single.rs` 单流路径；动态模式（FR-05-09 Writer）自带
  批量写，不使用 WBuf。
- 验收：默认配置下载任意大文件字节完整（与 FR-05-09 共用端到端正确性断言）。

### FR-05-02 throttle 睡眠下限修正（P0.2）
限速等待从「空桶每 chunk 强制 `sleep_ms`（20 ms）」改为**按差额比例等待**：
- `wait_ms = round(left / rate * 1000)`；仅当计算出的等待 < 1 ms 时才使用 `sleep_ms` 防忙轮询
  （`wait_ms = max(1, wait_ms)` 后按比例；原「一律 max(sleep_ms)」退役）。
- 修复后设置了限速的场景实际速率能**贴住设定值**（现状 128 KB chunk 被钳在单连接 ~6.4 MB/s）。
- `throttle_sleep_ms` 配置键保留，语义收窄为「防忙轮询下限」。
- 验收：全局限速 ≥ 8 MiB/s 时稳态速率落在设定值 ±10%（既有 01-throttle-proxy ±10% 口径）。

### FR-05-03 probe 连接复用 + 块大小下限（P0.3）
- **probe 连接复用（多流模式）**：blocks 引擎多流下载的 probe 响应体不再丢弃——作为 worker #0
  的第一个块从字节 0 边收边写（简化版：probe 收第 0 块，收完正常进入租约循环）；消除
  「probe→丢弃→重新握手」启动空窗。
- **块大小下限放宽**：`MIN_HTTP_BLOCK_SIZE` 从 1 MB 放宽到 **256 KiB**（`block_size_http`
  合法下限随之放宽；01-persistence-config-05 下限口径同步修订）。
- **默认块大小**：`DEFAULT_BLOCK_SIZE_HTTP` **维持 1 MB**（操作者裁决②：方案中「提到 4 MB」
  去除；blocks 模式块模型语义不变）。
- `block_size_http` 仅对 blocks 模式有意义；动态模式下块大小概念由调度器取代（FR-05-06）。

### FR-05-04 失败粒度：单 worker 出错只重试该块（P0.4）
- worker 网络错误**不再写全局 `failure` 触发全任务停止**：把当前块 requeue 并**原地重试**——
  第 1 次立即重试，之后指数退避（D86），上限 **5 次**；期间任务状态保持「下载中」（无失败
  转态、无重试倒计时、无失败 toast）。
- 仅**磁盘错误**（「磁盘写入失败」「无法写入目标文件」）与**任务级失败类错误**（URL 失效、
  语义性 4xx 等 Fatal 分类）直接升级为任务级失败（既有失败分类与 FR-01-40~44 口径不变）。
- 原地重试耗尽 5 次 → 升级为任务级瞬态失败，走**既有** app 层自动重试口径（8→16→32→60 s
  退避封顶、auto_retry 开关、重试计数；有进展重置为 1 的语义不变）。
- 动态模式（FR-05-06/07）天然达成同语义：单连接错误释放其段、由该连接或其他连接重新认领，
  不触发任务级失败；原地重试耗尽口径一致（D86）。
- 弱网收益：单连接抖动被引擎层吸收，任务级重试只在持续故障时触发。

### FR-05-05 默认参数（P0.5）
- `DEFAULT_HTTP_CONCURRENCY`：**4 → 8**（新任务默认并发；添加对话框并发字段留空/非法回退
  默认 8）。
- HTTP 并发上限：**64 → 128**，全链同步——CLI `-c` 合法域 1–128（非法报错文案同步）、
  config `http.http_concurrency` 合法域 1–128（FR-02-35 非法拒绝启动）、引擎 clamp、添加
  对话框并发字段合法域。
- `DEFAULT_THROTTLE_BURST_SECS`：**0.5 → 1.0**（与 MDM BURST_SECS 对齐；令牌桶突发窗语义
  不变）。
- BT 并发（默认 20、1–64）**不在本期范围**，一律不动。

## 3. P1 动态分段调度器（FR-05-06 ~ FR-05-08）

### FR-05-06 planner 动态分段调度（P1.1）
新增 HTTP 引擎调度层，完全对齐方案中 MDM `planner.rs` 算法（IDM/MDM 双重验证）：
- 三个原语 `assign / claim / commit`（+ `release`、`absorb_next`）：
  - `assign`：空闲 worker 领活——①领未分配段 ②抢占「分配后 ≥ preempt_after 仍未收到首字节」
    的段 ③把剩余最大的段对半切（按 `reserved` 切，绝不切走他人已认领字节）；
  - `claim`：写前认领——只授予本段剩余额度；段被切短后返回较少的 n（退款语义）；
  - `commit`：写后提交——段跑满释放 worker，返回续领起点；
  - `absorb_next`：probe「从 0 开始的整文件响应」专用——吞并相邻无人认领的段。
- 参数（D85）：`min_split` 初始 = config `http.min_split_kib`（默认 **256**，单位 KiB），
  尾段竞争激烈时自适应降半；`align` = 4096（切分点对齐）；`preempt_after` = 2 × TTFB
  （TTFB 从 probe 首请求实测，下限 20 ms）。
- **不变量**（proptest 钉死，coder 职责）：段区间互不重叠；`cursor ≤ reserved ≤ end`；
  Σ(段长) 恒等于文件大小；切分点按 `reserved` 计算故窃取永不覆盖在途字节；并发
  assign/claim/commit/release 无死锁。
- worker 循环（P1.2）：开区间 Range 请求 `bytes=<from>-`（段尾由 claim 控制）；**先付限速再
  认领**（限速等待发生在字节归属未定时，等待完 claim 只授予仍归属部分，未认领部分退款）；
  `Claim::Lost` 立即结束本段；响应体未到尾即丢弃（连接不回池）、到尾回池复用。

### FR-05-07 启动与收尾（P1.3）
- **probe 改裸 GET（新任务）**：动态模式新任务首个请求不带 Range 头；响应体直接交给
  worker #0 走 `absorb_next` 路径，**字节 0 立刻开始落盘**，其余 worker 并行领取切分段——
  消除「probe→丢弃→每人重新握手再请求」的启动空窗（D87）。
- **续传任务**：首个请求为从首个未提交字节起始的**开区间 Range**（`bytes=<resume_from>-`），
  响应头（206 + Content-Range 总长 + ETag/Last-Modified）完成一致性校验后，响应体同样直接
  作为数据流消费（D87）。
- **Range 失效臂语义保留**：续传/分段请求被服务器以 200 全量应答（忽略 Range）时——
  新任务 = 其余连接静默退出、仅首连接单流完成 + 既有「不支持断点续传」toast 语义；
  续传任务 = 既有「服务器本次未支持断点续传（Range），保留断点稍后重试」失败口径（D87）。
- **完成判定**：从「扫 blocks 计数」改为 planner 聚合游标（`reserved_total == file_size`）；
  100 ms tick 的三次 O(blocks) 扫描删除；「文件大小不符（已接收 X B ≠ Content-Length Y B）」
  守卫口径保留。
- **sidecar 体积**：动态模式状态从 `Vec<u64> written`（每块 8 字节 × N 万块）变为 O(段数)
  `(start, cursor)` 段列表（D89），2 s 一次的 JSON 全量重写不再随块数膨胀。

### FR-05-08 sidecar 双格式与动态续传（D89）
- **动态模式 sidecar**：新增 `scheduler = "dynamic"` 字段与 `(start, cursor)` 分段列表；
  保留 文件大小 / ETag / Last-Modified / 已下载字节 / 校验期望 / 任务元信息 等既有字段。
- **blocks 模式 sidecar**：现行格式（块大小 / 总块数 / 各块状态）原样保留。
- **格式即引擎**：sidecar 含 `scheduler = "dynamic"` → 动态引擎接续；无该字段（blocks 格式）
  → blocks 引擎接续。任务调度模式**烘焙于启动时**（D84），运行中任务与 sidecar 均不随
  config 热加载切换（B 类快照语义，FR-01-119 快照族口径）。
- 一致性校验口径不变（ETag/Last-Modified/Size 三要素；不符走既有 孤儿/失效 分支）。
- ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）：格式变更无迁移负担。

## 4. P2 磁盘流水线（FR-05-09 ~ FR-05-11）

### FR-05-09 writer 写流水线（P2.1）
动态模式新增磁盘写流水线（方案 Writer）：
- 固定容量 batch（256 KiB）填满封口；写盘任务把排队 batch 合并成 **≤ 4 MiB** 的定位写
  （unix `pwrite` / windows `seek_write`，无 seek 往返、无逐 chunk 调度）。
- 网络收包**永不**等磁盘，直到全下载在途内存 ≥ `HOLD_MAX`（**256 MiB** 跑道，≈1 Gbit 链路
  2 秒缓冲；跑道容量按连接数均摊）；超限时 `step` 才等待写盘回收。
- 缓冲回收池（spare = WRITE_MAX/BATCH）：写完的 batch 缓冲回收复用，零分配器流量。
- 对照审计（可选，coder 职责）：峰值内存 ≤ HOLD_MAX + 20 MiB。
- 验收：磁盘正确性（字节完整 + 校验）由 13-http-turbo 端到端断言覆盖；慢盘收益属操作者
  线下验收口径（见 §6）。

### FR-05-10 稀疏文件预分配（P2.2）
- 动态模式多流下载在 `set_len` 预分配处插入平台分支：Windows `FSCTL_SET_SPARSE`；Linux
  ntfs3 `system.ntfs_attrib |= 0x80000`，失败则尝试 `fallocate(KEEP_SIZE)`（D88）。
- **背景**：NTFS 对远端偏移首次写会被隐式零填充卡住数秒（MDM 实测 1 MiB 写 @2 GiB 处
  4.3 s）；动态调度第一条连接几十毫秒内即写文件中部，此项是硬前提。
- 预分配方式失败时**静默回退**普通 `set_len`（不产生任务失败；D88）。
- 验收：Linux 下 part 文件磁盘占用（du/st_blocks）随接收字节增长而非提前全额分配
  （13-http-turbo QA 断言）。

### FR-05-11 fsync 策略（P2.3）
- 平时**完全不 fsync**（现状如此，保持）。
- sidecar 保存（2 s tick）前对 part 文件做一次 `sync_data()`，且仅当：距上次 sync 已写入
  ≥ max(1 MiB, size/16) **或** 距上次 sync ≥ 30 s。
- 目的：保证「断电/kill -9 后 sidecar 声称的进度真实可续」，同时把 sync 开销压至 MDM 量级。
- 验收：kill -9 中断后重启续传，sidecar 声称字节数 ≤ 磁盘实际内容且续传完成字节完整
  （13-http-turbo QA 断言）。

## 5. 配置面（FR-05-12）与 UI 口径（FR-05-13）

### FR-05-12 config 新增键与严格校验
```toml
[http]
scheduler = "dynamic"        # dynamic | blocks（回退开关；默认 dynamic；D84）
min_split_kib = 256          # 动态模式初始切分粒度下限（KiB；默认 256；D85）
```
- `http.scheduler`：取值 {dynamic, blocks}（大小写不敏感，D84）；非法值/未知值 → **拒绝启动**
  （FR-02-35 严格校验口径）；**B 类快照**——任务启动时烘焙，运行中任务与 sidecar 不随热加载
  切换（FR-01-119 快照族口径；热加载只影响后续新任务，成功 toast 沿 v1.57 固定文案）。
- `http.min_split_kib`：正整数（KiB），默认 256；非法值 → 拒绝启动（FR-02-35）。
- `block_size_http` 保留，语义限定 blocks 模式（动态模式不读取；合法域下限 256 KiB）。
- 配置模板（FR-01-85）同步收录两新键（用途与取值范围注释；模板活跃键 40 → 42）。

### FR-05-13 UI 口径（最小变化）
- **详情分块行（D83）**：动态模式显示 `<已下载>/<总字节> · 动态分段`（如 `3.2 MB/10 MB ·
  动态分段`）；blocks 模式保持 `x/N · <块大小>/块` 既有口径。
- 并发连接面板：动态模式每行 = 一条连接当前持段（start/end/done 语义沿用 ConnView），
  列结构不变。
- 任务行/头部仪表/速度统计/限速/代理/槽位/重试 UI 全部不变（本期零 UI 结构改动）。

## 6. 验收标准

1. **正确性回归**：现有规格与 QA 全绿（本轮已同步修订的 6 份既有规格按新口径执行）；
   Planner proptest 不变量全绿（§3 FR-05-06）。
2. **端到端 QA**：`qa/13-http-turbo-qa.md` 全用例通过（P/F/S 口径）。
3. **性能基准（操作者线下验收口径，不进自动化 QA）**：与 MDM 同机同文件对比——本地 nginx
   1 GiB / 256 KiB / 4 GiB 三档；`tc netem` RTT 20/100/200 ms + 1% 丢包；磁盘瓶颈场景
   （机械盘/U 盘验证流水线跑道）。通过线：追平 = 各场景吞吐 ≥ MDM 95%（P3 超越线不在本期）。
4. **断点续传**：kill -9 后重启，sidecar 声称字节数与磁盘实际内容一致（FR-05-11 前置保证）。
5. **回退开关**：`scheduler = "blocks"` 时旧行为逐条可复现（13-http-turbo-11）。

## 7. 裁决登记（D83–D89，可回退）

| # | 裁决 | 内容 | 依据 |
|---|---|---|---|
| D83 | 分块行动态口径 | 动态模式详情分块行 = `<已下载>/<总字节> · 动态分段`；blocks 口径不变 | 设计文档未定义动态模式 UI 呈现；最小变化原则 + v1.49 尺寸口径延续 |
| D84 | scheduler 键口径 | 取值 {dynamic, blocks} 大小写不敏感；默认 dynamic；非法拒绝启动（FR-02-35）；B 类快照（任务启动烘焙，热加载不影响运行中任务与 sidecar） | 方案「所有改动放在配置开关后面」+ FR-01-119 B 类快照先例（QA-RC-07 烘焙口径） |
| D85 | min_split_kib 语义 | 默认 256 KiB；仅动态模式；尾段竞争自适应降半、切分点 align 4096；不适用于 blocks 模式 | 方案 P1.1（min_split 初始 256 KiB / align 4096 / 自适应降）+ 操作者裁决②（下限 256 KiB） |
| D86 | 原地重试节奏 | 第 1 次立即，之后指数退避（1s、2s、4s、8s），上限 5 次；耗尽升级任务级瞬态失败走既有 8→60 s 退避 | 方案 P0.4（首次立即重试，之后指数退避到上限 5 次）+ 既有 retry.rs 退避族 |
| D87 | probe 裸 GET 语义 | 新任务裸 GET（无 Range）+ worker #0 absorb；续传任务开区间 Range 从首个未提交字节；Range 失效臂保留既有 toast/失败口径 | 方案 P1.3 + 既有 single.rs 降级臂（「不支持断点续传」「保留断点稍后重试」） |
| D88 | 稀疏回退 | 平台稀疏分支失败静默回退普通 set_len，不产生任务失败 | 方案 P2.2（平台分支）+ 「不能因预分配增强而新增任务失败」最小风险原则 |
| D89 | sidecar 双格式 | 动态模式 = scheduler 字段 + (start,cursor) 段列表；blocks 格式原样；格式即引擎（无 scheduler 字段 → blocks 引擎接续）；调度模式烘焙于任务启动 | 方案 P1.3（O(段数) 状态）+ FR-02-35 不兼容废弃口径 + B 类快照 |
