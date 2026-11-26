# ezr HTTP 引擎提速改造方案

## 0. 总体思路

ezr 与 MDM 底层栈相同（reqwest/hyper/rustls/tokio），差距全在调度层。因此不动技术栈，只改三层：

```
P0 补丁层：修掉四个"自伤"缺陷            → 预计拿到 40~60% 收益
P1 调度层：固定 1MB 块租约 → 动态分段调度   → 追平 MDM 的核心
P2 磁盘层：同步逐 chunk 写 → 流水线批量写   → 追平 MDM 的另一半
P3 增益层：BDP 自适应、并发预热、限速协同    → 超越 MDM
```

所有改动放在配置开关后面：`http.scheduler = "dynamic" | "blocks"`（默认 dynamic，保留旧引擎可一键回退）。

---

## P0 快赢补丁（1~2 天，收益立竿见影）

### P0.1 写合并——去掉每 chunk 同步写盘

**位置**：`src/engine/supervisor/workers.rs:221-235`、`single.rs:118-210`

现状是每个 hyper chunk（8~128 KB）直接 `file.write_all`，网络被磁盘串行卡死。最小改法：worker 手里加一个 256 KiB 聚合缓冲：

```rust
// workers.rs 内
const WBUF: usize = 256 * 1024;
struct WBuf { buf: Vec<u8>, pos: u64 }   // pos = 本缓冲起点（绝对偏移）

impl WBuf {
    fn push(&mut self, at: u64, data: &[u8]) { /* 顺序追加，不匹配则先 flush */ }
    async fn flush(&mut self, file: &mut tokio::fs::File) { /* 一次 write_all */ }
}
```

- 每个 chunk 从「1 次 seek + 1 次 syscall + 1 次阻塞池调度」变为「1 次内存拷贝」，syscall 次数下降 30~50 倍；
- `conns.lock()` 的 ConnView 更新从每 chunk 一次降为每 flush 一次（UI 100 ms tick 根本看不出差别）。

### P0.2 修 throttle 的 20 ms 睡眠下限

**位置**：`src/engine/throttle.rs:29-33`

```rust
// 现在：wait_ms = (left / rate * 1000).max(sleep_ms)   ← 桶空时每 chunk 强制睡 20ms
// 改为：wait_ms = (left / rate * 1000).round() as u64; // 按差额比例等待
//       wait_ms = wait_ms.clamp(1, 200).max(if wait_ms < 1 { sleep_ms } else { 0 })
```

等价地说：只有当计算出的等待 < 1 ms 时才用 `sleep_ms` 防忙轮询，其余按比例。修复后设了限速的场景实际速率能贴住设定值（现在 128 KB chunk 只能跑到 ~6.4 MB/s 单连接）。

### P0.3 probe 连接复用 + 块大小放宽

- `src/engine/supervisor/download.rs:153` 的 `drop(resp)` 改为：多流模式下 probe 响应体直接当作 worker #0 的工作，从字节 0 开始边收边等其余 worker 接力（第一阶段简化版：probe 收第 0 块，收完正常进入租约循环）。
- `src/model/chunk/plan.rs:3` 的 `MIN_HTTP_BLOCK_SIZE` 从 1 MB 放宽到 256 KB，`DEFAULT_BLOCK_SIZE_HTTP` 提到 4 MB——块越大，每 MB 摊到的 RTT 越少；配合 P1 后此参数交给调度器自适应。

### P0.4 失败粒度：单 worker 出错只重试该块

**位置**：`workers.rs:196-199, 238-241`（写 `*failure` 触发全任务停止）+ `app` 层 8→60 s 退避

改为：worker 网络错误不写全局 `failure`，而是把当前块 `requeue` 并立即原地重试（首次立即重试，之后指数退避到上限 5 次）；只有磁盘错误/URL 失效才升级为任务级失败。弱网下这一项的收益经常比所有其他项加起来还大。

### P0.5 默认参数

`config.rs`：`DEFAULT_HTTP_CONCURRENCY` 4 → 8；CLI 上限 64 → 128；`throttle_burst_secs` 0.5 → 1.0（与 MDM 的 BURST_SECS 对齐）。

---

## P1 核心改造：动态分段调度器（追平 MDM）

### P1.1 新增 `src/engine/supervisor/planner.rs`

完全对齐 MDM `planner.rs` 的算法（已被 IDM/MDM 双重验证），三个原语 `assign / claim / commit`：

```rust
pub struct Planner {
    segs: Vec<Segment>,
    min_split: u64,          // 初始 256 KiB，尾巴竞争激烈时自适应降
    align: u64,              // 4096
    preempt_after: Duration, // 2 × TTFB，下限 20 ms（TTFB 从 probe 首请求测得）
}

struct Segment {
    start: u64,                 // 固定
    end: u64,                   // 被切分时下移
    cursor: u64,                // 已提交（写盘确认）
    reserved: u64,              // 已认领（claim 拨出）
    worker: Option<WorkerId>,
    receiving: bool,
    given_at: Option<Instant>,
}

impl Planner {
    /// 空闲 worker 领活：1) 领未分配段 2) 抢占"分配了但 ≥preempt_after 还没收到
    /// 首字节"的段 3) 把剩余最大的段对半切（按 reserved 切，绝不切走别人已认领的字节）
    fn assign(&mut self, w: WorkerId, after: Option<u64>) -> Option<Assignment>;
    /// 写前认领：只授予本段剩余额度；段被切短后返回 Granted(较少的 n)
    fn claim(&mut self, w: WorkerId, pos: u64, len: u64) -> Claim; // Granted(n) | Lost
    /// 写后提交：段跑满则释放 worker，返回 Some(end) 让 worker 继续领下一单
    fn commit(&mut self, w: WorkerId, upto: u64) -> Option<u64>;
    fn release(&mut self, w: WorkerId);
    /// probe 的"从 0 开始的整文件响应"专用：吞并相邻无人认领的段
    fn absorb_next(&mut self, w: WorkerId) -> bool;
}
```

**与 MDM 相同的不变量**（后面用 proptest 钉死）：段区间互不重叠、`cursor ≤ reserved ≤ end`、Σ(段长) 恒等于文件大小、切分点按 `reserved` 计算所以窃取永不覆盖在途字节。

### P1.2 改写 worker 循环（`workers.rs`）

```rust
loop {
    let Some(a) = planner.assign(wid, after) else { break };
    let range = if a.whole { None } else { Some(format!("bytes={}-", a.from)) };
    let mut resp = req_checked(&client, &url, range).await?;   // 开区间，段尾由 claim 控制
    let mut at = a.from;
    loop {
        let Some(chunk) = resp.chunk().await else { break };
        // 先付限速再认领：限速等待发生在"字节还归属未定"时，等待完段可能已被
        // 切短，claim 只授予仍属于我们的部分，没认领的退款（学 MDM 的 pay-first+refund）
        limiter.acquire(chunk.len()).await;
        let n = match planner.claim(wid, at, chunk.len() as u64) {
            Claim::Granted(n) => n,
            Claim::Lost => break,                    // 整段被抢走
        };
        writer.push(at, &chunk[..n as usize]).await; // P2 的流水线写
        at += n;
        if let Some(end) = planner.commit(wid, at) { after = Some(end); break; }
    }
    drain_or_drop(resp).await;   // 体未到尾→直接丢弃（连接不回池）；到尾→回池复用
}
```

### P1.3 启动与收尾

- probe（`download.rs:30` 现在的 `Range: bytes=0-`）改为**裸 GET**，响应直接交给 worker #0 走 `absorb_next` 路径，字节 0 立刻开始落盘，其余 worker 并行领取切分段——消除现在“probe→丢弃→每人重新握手再请求”的启动空窗；
- 完成判定从“扫 blocks 计数”改为 `planner` 的聚合游标 `reserved_total == file_size`，100 ms tick 的三次 O(blocks) 扫描整个删掉；
- **sidecar 体积自动降一个数量级**：状态从 `Vec<u64> written`（每块 8 字节 × N 万块）变成 O(段数) 的 `(start, cursor)` 列表，2 s 一次的 JSON 全量重写问题随之消失，无需单独做增量化。

---

## P2 磁盘流水线（追平 MDM 的写盘侧）

### P2.1 新增 `src/engine/supervisor/writer.rs`

```rust
const BATCH: usize = 256 * 1024;      // 固定容量 batch，填满即封口
const WRITE_MAX: usize = 4 << 20;     // 合并后单次写上限
const SPARE_MAX: usize = 16;          // 回收池 = WRITE_MAX / BATCH
const HOLD_MAX: u64 = 256 << 20;      // 全下载在途内存跑道

struct Writer {
    file: std::fs::File,                              // 专用 handle，pwrite 定位写
    pos: u64,
    writing: Option<JoinHandle<io::Result<Vec<Vec<u8>>>>>,  // spawn_blocking 在写的一单
    queue: VecDeque<Vec<u8>>,                         // 排队的完整 batch
    buf: Vec<u8>,                                     // 正在填充
    spare: Vec<Vec<u8>>,                              // 写完回收，零分配器流量
}

impl Writer {
    fn push(&mut self, at: u64, data: &[u8]);   // 只做内存拷贝，永不等待
    async fn step(&mut self) -> Result<(), Error>; // 回收写完的 batch；writing==None 就发下一单；
                                                   // 仅当在途内存 ≥ hold_cap(=HOLD_MAX/连接数) 才 await
    async fn finish(&mut self) -> Result<(), Error>;
}
```

要点：
- 网络收包**永不**等磁盘，直到 256 MiB 跑道耗尽（≈1 Gbit 链路 2 秒缓冲）；
- 写盘任务内部把 queue 里的 batch 合并成 ≤4 MiB 的 `write_all_at`（unix `pwrite` / windows `seek_write`），无 seek、无 tokio::fs 每次调度；
- 对照测试（可选）：`tests/memory.rs` 式的分配审计，峰值内存 ≤ `HOLD_MAX + 20 MiB`。

### P2.2 稀疏文件

**位置**：`download.rs:154-177` 的 `set_len` 之前插入平台分支：

```rust
#[cfg(windows)] // FSCTL_SET_SPARSE
#[cfg(all(unix, target_os = "linux"))] // ntfs3: xattr system.ntfs_attrib |= 0x80000; 失败则尝试 fallocate(KEEP_SIZE)
```

否则 NTFS 上对远端偏移的首次写会被隐式零填充卡住数秒（MDM 实测 1 MiB 写@2GiB 处耗时 4.3 s）。ezr 动态调度第一条连接几十毫秒内就会写到文件中部，此项是硬前提。

### P2.3 fsync 策略

- 平时完全不 fsync（现状也如此，保持）；
- sidecar 保存（2 s tick）前对 part 文件做一次 `sync_data()`，且仅当距上次 sync 已写入 ≥ max(1 MiB, size/16) 或 ≥30 s——保证“断电后 sidecar 声称的进度真实可续”，同时把 sync 开销压到与 MDM 相同量级。

---

## P3 超越 MDM 的增量项（每项独立开关）

| # | 改动 | MDM 现状 | 预期收益 |
|---|---|---|---|
| 3.1 | **BDP 自适应接收缓冲**：连接层测得 RTT×单连接吞吐 > 默认窗口时，新连接用 socket2 设 `SO_RCVBUF`（上限 8 MB） | 明确不做（socket2 只用于绑定网卡） | 高延迟跨国/长肥管道场景显著超越 |
| 3.2 | **并发握手预热**：probe 确认 Range + size ≥ 64 MB 后，N-1 条连接**同时**发起，不等渐次 ramp | 按 64 KiB piece 渐进 ramp | 大文件冷启动快 1~2 个 RTT 周期 |
| 3.3 | **慢连接剔除**：响应 2 s 后速度 < 单连接标杆/4 且剩余 >2 s → 只掐这条连接立即重领（标杆须按“聚合链路速度的分摊份额”封顶，避免 MDM #896 之前的连锁雪崩教训） | 有（我们抄作业+保留其修正） | 真实网络追平 |
| 3.4 | **主机连接上限学习**：某连接被拒时记录“本主机最大成功并发”，2 次观察后生效，10 分钟衰减 | 有（hosts.rs） | 追平，少浪费重试 |
| 3.5 | **切分粒度自适应**：`min_split` 从 256 KiB 起，尾段竞争时自动降半；`preempt_after` 持续用实测 TTFB 校准 | 固定 12 KiB/2×TTFB | 小文件请求风暴与带宽利用率间更优 |
| 3.6 | （实验性）**学习到的并发上限 ≤2 时自动降级 h2 多路复用**：单 TCP 连接开多 stream 绕过服务端 HTTP 层连接数限制 | http1_only 固定 | 特定受限服务器上可超越，默认关 |

> P0+P1+P2 完成后即达到 MDM 同档；3.1+3.2 是拉开差距的主力，3.6 只在服务端限连接数的场景生效。

---

## 改动文件清单与工作量

| 阶段 | 文件 | 性质 | 估时 |
|---|---|---|---|
| P0 | workers.rs / single.rs / throttle.rs / download.rs / plan.rs / config.rs | 修改 | 1~2 天 |
| P1 | 新增 supervisor/planner.rs；改 download.rs、workers.rs、finalize.rs（段状态持久化）、blocks.rs（保留给旧引擎） | 新增+重构 | 3~5 天 |
| P2 | 新增 supervisor/writer.rs、sparse.rs；改 download.rs | 新增 | 2~3 天 |
| P3 | 新增 net.rs 扩展（socket2 连接器）、hosts.rs；改 planner.rs、download.rs | 新增 | 2~4 天 |

配置面只加一个节：

```toml
[http]
scheduler = "dynamic"        # dynamic | blocks（回退开关）
concurrency = 8
min_split_kib = 256
initial_piece_kib = 64
shed_slow = true
rcvbuf_adaptive = true
```

## 验收标准与测试方法

1. **正确性回归**：现有测试全绿；为 Planner 写 proptest（区间不重叠/认领不越界/Σ段=文件大小/并发 assign+claim+release 无死锁）；
2. **性能基准**（与 MDM 同机同文件对比）：
   - 本地 nginx：1 GiB、256 KiB、4 GiB 三档；
   - `tc netem` 模拟 RTT 20/100/200 ms + 1% 丢包（这条最容易拉开 ezr 旧版差距，也是 3.1 的主战场）；
   - 磁盘瓶颈场景：下载到机械盘/U 盘（验证流水线跑道）；
3. **通过线**：追平 = 各场景吞吐 ≥ MDM 的 95%；超越 = netem 高 RTT 档 ≥ MDM 的 110%，其余 ≥ 100%；
4. **断点续传**：kill -9 后重启，sidecar 声称的字节数与磁盘实际内容一致（sync 前置保证）。

## 风险控制

- 所有新路径在 `scheduler = "dynamic"` 后面，任何线上问题一键回退旧引擎（旧代码 P1 阶段不动，只停止调用）；
- 限速语义变化（P0.2）单独出 changelog，因为修复后“设了限速反而更快”属于用户可见的行为修正；
- h2 降级（3.6）默认关闭、只做实验特性。

