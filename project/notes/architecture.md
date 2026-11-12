# 架构经验

## 任务启动必须经槽位单点路径，协议分流集中在 tick 层
- **现象**：BT 任务获槽后被发 HTTP `Cmd::Start`（url=本地 .torrent 路径），HTTP 引擎报
  「builder error: relative URL without a base」并被归类为「网络错误」；确认添加 BT 还有
  一条绕过槽位的立即 `StartBt`，与槽位路径构成双重启动。
- **处置**：启动收口到 `tick_grant_and_start`/`send_retry_start` 两处，按协议分流——
  BT 走 `start_bt_from_slot`（读种子 + 解析 + `make_bt_spec` + `Cmd::StartBt` +
  Queued→Downloading；读取/解析失败 → Fatal「无法读取种子文件」并释放槽位），HTTP 保持
  原路径；`dlg_confirm_add_bt` 移除立即 `StartBt`，非暂停添加一律入队等槽。
- **根因**：槽位路径是 BT/HTTP 统一的单点启动入口，任何绕过它直接给引擎发启动命令的
  路径（对话框确认、未来的右键菜单等）都会与槽位路径双重启动，且槽位路径不知道该任务
  的真实协议就会误用 HTTP 管道。新增任务入口时只创建 Queued 任务，启动一律等槽。

## 速度统计的喂窗义务在协议事件路径，采样点只读不喂
- **现象**：BT 任务下载进度正常推进（任务行已下载字节在涨）但速度恒显示 0；HTTP 任务
  同版代码速度正常。
- **处置**：协议进度事件处理器必须喂 `self.windows` SpeedWindow（镜像 `Evt::Progress`
  的口径：push (now, 累计字节数)，缺窗则建）；`on_bt_progress` 补喂窗且仅下载态喂
  （做种态不重建窗——on_bt_done 已清窗，滑窗生命周期只属下载态）。
- **根因**：速度采样单点在 `tick_speed_and_session`，只从 `self.windows` 读速率
  （get_mut miss 即采样 0.0）；喂窗点却分散在各协议事件路径——HTTP 在 `Evt::Progress`
  喂窗，BT 的 `Evt::BtProgress` 只更新任务字段不喂窗。不变式自查口径：**「进度字段
  更新 ≠ 速度统计生效」**，新协议/新事件路径的两条义务（更新任务字段、喂速度滑窗）
  必须成对出现；终态路径（done/stopped/failed/pause/delete）成对清窗。

## TUI 退出路径：输入排空窗必须保持 raw/ECHO-off 到 termios 恢复前最后一刻
- **现象**：按 q 退出后终端出现 `35;62;12M` 乱码（SGR 鼠标移动事件；`ESC[<` 被终端
  模拟器当未识别 CSI 吞掉，余下部分直接上屏）。
- **处置**：`restore_into` 固定顺序＝关鼠标捕获 → settle(50ms) → 排空循环 → 恢复
  termios → 关 paste/离备屏；排空循环用「截止时间 + 静默判定」双条件
  （RESIDUAL_MIN_SPAN 80ms / QUIET 40ms / CAP 250ms），不得用「首个 5ms 静默即
  break」。项目 `unsafe_code = deny`，字节级 ioctl 排空不可用，用 crossterm 公开
  poll/read 即可（排空循环对「锁被占用时 poll 假阴性」免疫：循环重试而非 break）。
- **根因**：web 终端经中继有在途延迟，`?1006l` 写出后仍有鼠标字节晚到；termios 恢复
  （ECHO on）后到达的字节被 tty 行规程**立即回显上屏**（这才是乱码的直接机制，
  比残留队列被 shell 读到更早发生）；旧排空在首个静默间隙就退出，在途字节必然漏。
  另外 crossterm `EventStream` 后台线程以无超时 `poll_internal(None)` 长期持有全局
  reader 锁，`crossterm::event::poll(timeout)` 拿不到锁时直接返回 `Ok(false)`
  （假装无事件）——任何依赖 poll 做排空的逻辑都不能把单个 false 当「队列已空」。

## BT peer 池（候选 × 退避重试 × 断开即补位）替代一次性 tried 集合
- **现象**：tracker 一次返回的 peer 全部进 `tried` 集合且永不清除——连接失败/断开的
  peer 不再重试，tracker 再返回同批 peer 也无动作；swarm 内其他 peer 上线后 ezr 永远
  不连它。叠加监听只绑 127.0.0.1，局域网 peer 连不进来，表现为「只连一个可达 peer、
  断开后不重连、不上传」。
- **处置**：①监听 `0.0.0.0:0`（announce 端口 = 真实监听端口，LAN 可入连）；②tracker
  响应只喂**候选池** `HashMap<(ip,port), PeerCandidate{fails,last_try}>`，2s sweep 按
  「未连过/未拉黑/冷却到期」补满到 concurrency（冷却 = 5s×2^fails 封顶 120s，连接成功
  清零 fails）；③PeerIn::Closed 立即触发补位扫描（断开即重连，不等 sweep）；④握手响应
  peer_id 与本机相同 → 判定连到自身，关连接并把该候选移出池。
- **根因**：「tracker 一次响应 = 一次拨号机会」的模型下，任何瞬时失败都被放大成永久
  失败；正确模型是候选池常驻、连接机会周期性再生成（libtorrent policy 的 connect
  candidates 等价物）。

## libtorrent 策略移植三件套：选块 / ENDGAME / 固定槽 choker
- **选块**（piece_picker）：`order_wanted` = 部分到达的 piece 优先（已到块数降序）→
  稀有度升序（可用副本数）→ 同稀有度随机抖动（xorshift jitter，per-call 变化）。全局
  认领集（claimed = 各 conn inflight ∪ partial 已到块）防重复下载。
- **ENDGAME**：正常轮（跳过被认领块）无块可发且管线未满时进入：允许重复请求他人已认领
  的块；任一块到达即对其他 conn 的同块在飞请求发 Cancel（`on_block` 入口统一处理，非
  endgame 时天然无重复、无副作用）。判定 `endgame_active` = 所有未完成 piece 的所有块
  都被认领。
- **反吸血 choker**（fixed slots choker）：每 10s（unchoke_interval）重算——仅
  interested peer 参与，按「对端给我们的下载速率」降序取 4 个固定槽（tit-for-tat），
  外加 1 个乐观槽每 30s 在剩余池轮转；状态翻转才发 Unchoke/Choke。做种态全量 unchoke
  （D58 口径不变）。`Request` 服务闸门 = 我方未 choke 它（旧实现误用对端 choke 本端的
  状态做判据，等于永远不上传）。NotInterested 立即回收槽位。
- **tracker 交互**：每 tracker 独立 completed_sent 标志（修复共享标志导致多 tracker 只
  有一个发 completed）；下载完成经 per-tracker `Notify` 立即唤醒 announce 循环发
  completed（不等 interval）；Pause/StopSeed/Cancel 三终态都发 stopped 且携带真实
  uploaded/downloaded/left（旧实现 stopped 全 0 且 Cancel 不发）；live.uploaded 每秒
  tick 回填（uploaded=0 的 announce 属失真上报）。

## 抄写 ezr-demo 组件时键路由层是交互契约的另一半，必须成对接入
- **现象**：BT 对话框 trackers 多行面板能输入/换行但 Backspace/Delete 全部失效（不能
  删除字符串）；BT 对话框种子行/并发/保存到退格无效；粘贴进 BT 对话框被整体丢弃。
- **处置**：组件层（`ui/multiline.rs::draw_multi_line_input`、`ui/tree.rs::
  draw_tree_panel`、`multiline.rs::MultiLineBuf`）本就是 ezr-demo 1:1 抄写版且已被命令
  窗与 BT 对话框共用，无需重抄；缺的是应用层路由——`dialog_keys/routing.rs::
  text_backspace` 补 (AddBt,0)/(AddBt,1)/(AddBt,3|ModifyBt,1)/(AddBt,5|ModifyBt,3) 四臂
  （trackers 臂转 `MultiLineBuf::backspace` 行首合并）；`on_dialog_key` 补
  Delete/Home/End 臂（trackers 焦点门控，抄 demo keys.rs）；`app/paste.rs` 的
  paste_accepts 放行 AddBt/ModifyBt、trackers 焦点直通 `insert_str`（保换行分段）、
  dlg_apply_paste 补 BT 单行字段臂。
- **根因**：ezr-demo 的交互契约 = 渲染组件 + 键路由两层；抄写移植时只对齐了组件层，
  路由层（`text_backspace` 的 (kind, focus) 分发表、`on_dialog_key` 的 KeyCode 臂、
  `on_paste` 的门禁与分流）没有同步。新增对话框/字段时这三处分发表是唯一登记点，
  「渲染出来但按了没反应」先查路由表漏臂，不要先怀疑组件。

## BT 全局黑名单必须挂在跨任务的 EngineShared 上
- **现象**：并发明细按 b 断开 peer 后，同轮内 tracker 不会再连它（`tried` 集合挡同
  (ip,port)），但暂停任务再继续（引擎 run() 整个重启）peer 又被连上。
- **处置**：黑名单放 `EngineShared`（`Mutex<HashSet<IpAddr>>`，内存态、不持久化、进程
  存活期有效）：`BtTaskCmd::Disconnect` 处理时从 `conn.addr` 解析 IP（`addr_ip`，兼容
  v6 方括号）写入；出站门禁在 tracker Peers 循环（`is_blacklisted(p.ip)` 跳过）、入站
  门禁在 `listener.accept()`（黑名单 IP 直接 drop 流）。按 IP 而非 ip:port 记忆——对端
  换端口仍拒绝，与「屏蔽这个 peer」的操作者意图一致。
- **根因**：BT 引擎每次暂停/继续/修改重建都是全新 `run()`（新 conns/新 tried），任何
  per-run 状态都活不过暂停；进程级记忆必须放在跨任务共享的 `Arc<EngineShared>`。

## BT 下载速度优化五件套（LAN 900KB/s 量级对齐 transmission）
- **现象**：局域网实测 ezr ~900KB/s，同 swarm transmission ~2MB/s。
- **处置**（按收益排序）：①peer socket 全部 `set_nodelay(true)`——请求路径全是 17B
  小报文，Nagle 只放行一个未 ACK 小段，后续 pipeline 请求被压到对端 delayed-ACK
  （~40ms）之后，形成整批停顿；②`PEER_PIPELINE` 5→32（FR-02-17 的「单 peer 在飞
  5 块」经操作者指令放宽，登记为规格偏离）；③请求调度加**全局认领集**（claimed =
  全部 conn 的 inflight ∪ partial 已到块），同块不再被多 peer 重复请求，covered_pieces
  出自 BTreeSet 天然有序（旧实现每块到达后全量 sort 是白费），每 piece 每轮至多 4 块
  平铺推进；④在飞块 20s 超时重发（inflight 值带 Instant，1s tick 清扫后重填）——
  peer 吞请求不应答不再把块永久卡死；⑤做种上传按 request 区间**定向读盘**
  （`read_piece_range` 只读与 [begin,begin+len) 相交的文件段）——旧实现每个 16KB
  request 读整个 256KB piece，16 倍读放大。
- **根因**：吞吐 = 在飞字节/RTT，五处分别对应「报文被 Nagle 压住」「在飞窗口太小」
  「带宽被重复请求浪费」「坏 peer 卡死调度」「做种侧磁盘放大」；单改任何一处都不够，
  组合后才对齐主流客户端量级。loopback e2e 感知不到 ①③（delayed-ACK 不触发），性能
  类修复要在真实网络（或至少真实 NIC）上复核。

## 任务档案与断点分离：registry.json 存任务定义，.ezr 只存 piece 进度
- **动机**：BT 任务注册表快照（BtTaskData）与 .ezr sidecar 双写 source/trackers/piece_
  length/files/done_pieces 全集——任务定义（不改）与下载进度（秒级增长）耦合存放，.ezr
  每秒重写全量任务 JSON，重启还原又依赖两处并集。
- **口径**：**registry.json = 任务档案**（任务是什么、从哪下、怎么下：source/trackers/
  piece_length/files + UI 态，不含任何进度字段）；**.ezr = 数据断点**（version + done_
  pieces，进度唯一归属，与数据文件同生命周期：删除含文件即删、保留文件即留、重加同种子
  自动接续）。恢复 = .ezr 逐 piece SHA-1 磁盘校验（校验即身份验证，无需 infohash 字段）；
  引擎保存用 `load_done`（bug6 轮定案：内部开发阶段**无兼容性需求**——版本恒为 1，
  v1 全量档兼容读取代码已删除，`load_done` 只认 `{version:1, done_pieces}` 结构体形态，
  形状/版本不符一律 None；操作者明示「有 version 1 的代码直接删除，不需要保留」）。
- **不变式**：任务定义字段进 `BtTaskData` 的前提是「重加同种子重建任务所需」；进度数据
  进 sidecar 的前提是「仅服务于断点续传」。UI 还原（重启后 pieces 窗口）由 app restore
  读 .ezr 填 done_blocks/chunk_done/downloaded（`selected_bytes_of_pieces` 与引擎
  PieceMap.write_plan 同一套覆盖计算），引擎启动校验后以 BtProgress 自愈收敛。

## BT 并发 panel 速度三段管线：与 HTTP 完全同构（消抖动的正解在消费侧）
- **根因**：引擎 `conn_views` 在每条 peer 消息后都调用并重置窗口——「区间瞬时速率 =
  本区间字节/区间时长」，块到达呈突发，逐秒跳变且与任务速度（app 侧 1s 滑窗）口径
  不同，单连接时两处数值明显不匹配。
- **管线**（与 HTTP conn_windows/conn_speed 同构）：①`Evt::BtConns` 处理器把每连接
  累计值喂 `BtConnSpeedStat.down_win/up_win`（1s SpeedWindow，陈窗重置，离开连接
  retain 回收）；②`sample_bt_conn_speed_display` 每 SPEED_TICK 把滑窗 rate 灌入
  down_ema/up_ema（SmoothedSpeed α=1/5）；③UI 只读 EMA 值（`bt_conn_speeds_of`）。
- **任务速度同源化（bug9 定案）**：BT 任务 `t.speed`/`t.upload_speed` = `Σ
  per-conn EMA`（`sample_bt_conn_speed_display` 采样时顺带汇总写任务字段）——单连接
  时任务速度与该行速度构造上相等，多连接等于各行之和。引擎侧上传速度三处旧口径
  全废：逐块 BtProgress 硬编码 upload_speed=0、tick 用 conn_views 碎片窗口求和、
  任务下载速度走 verified-piece 粒度进度窗（低速时锯齿与行速度对不上）——
  `Evt::BtProgress` 已删 upload_speed 字段，BT 任务不再喂进度窗（windows 只属 HTTP）。
- **不变式**：喂窗义务在事件路径、采样点只读不喂、清理口径四点对齐
  （on_bt_done/on_bt_stopped/remove_task/clear_completed 均调 clear_bt_conn_stats）。

## 逐块 O(P) 热路径是大种子速度塌缩的根因：增量 availability + 缓存选片序 + 事件节流三件套
- **现象**：loopback 基准（256KiB piece、千级 pieces）ezr≈libtorrent，但真实局域网
  大种子（数万 pieces）速度塌到 100KB/s 以下且多 peer 时更糟；引擎单任务 CPU 被打满。
- **根因**：引擎对**每条 peer 消息**（尤其每个 Block）做 O(P)~O(P log P) 全量重算——
  `availability_map` 遍历全部连接的 has 集合重建 O(Σ|has|)、`issue_requests` 重建
  wanted_all + 全排序 O(P log P)、逐块 `conn_views`（seeder 检查 O(P)/conn）、逐块
  BtProgress/BtConns 事件携带 done/partial/active 全集克隆 O(P)×3 并由 app 全量替换。
  成本 = 块率 × P，块率高（16KiB 块）+ P 大（真实种子）时引擎任务饿死 socket 读取，
  TCP 窗口塌缩 → 速度崩。loopback 基准小 payload（千级 pieces）测不出——**速度回归
  必须用小 piece 压测**（harness `--piece-kib 16` → 16k pieces）。
- **处置三件套**：①`PieceOrderCache` 增量维护「多少 peer 报有」（Bitfield/Have
  peer_gained、Closed/Disconnect peer_lost，只计 covers 的 piece），按（稀有度,
  jitter-seed-per-rebuild）缓存排序，dirty 失效；②逐块 refill 只走
  `refill_visit_order`（partial 按已收块数降序前置 + 缓存序，O(已收块数+跳过数)），
  done/无报有靠 walk 跳过不触发重建；③BtProgress/BtConns 逐块事件 250ms 节流
  （`PROGRESS_EMIT_MIN_INTERVAL`，tick 1s 强制发），进度/速度显示粒度足够
  （qBittorrent 同为秒级刷新）。配套 `PEER_PIPELINE` 32→64（对齐 libtorrent
  request_queue_time=3s 动态深窗精神，吸收校验/落盘的 refill 尖峰）、tracker
  numwant 50→200（libtorrent num_want 默认）。16k pieces 压测对照：旧 162.3s vs
  新 129.8s（256MB loopback，新代码贴 seeder 上限）。

## 排序键必须轮内恒定：有状态 jitter 触发 sort 的 total order panic（引擎花屏退出根因）
- **现象**：BT 下载任务启动后片刻报「引擎内部错误: user-provided comparison function does
  not correctly implement a total order」，随后 TUI 花屏、看似退出。
- **根因**：`order_wanted` 的 sort_by_key 键里含 `jitter(piece)`，而 jitter 内部用全局
  自增 AtomicU64 计数器做随机源——**每次取键值都不同**。Rust 新排序实现（driftsort/
  ipnsort）会在排序过程中多次取键并主动校验比较器一致性，键漂移 → 比较器不满足全序
  （传递性/一致性被破坏）→ panic。小输入可能侥幸不触发（比较次数少），「停一会」才炸
  是典型的输入规模相关 panic。
- **修复口径**：随机化必须**在单次排序内恒定**——种子在 `order_wanted` 入口取一次
  （AtomicU64 fetch_add，每轮选片一次），`jitter(piece, seed)` 纯函数化（同
  (piece,seed) 恒定、异种子异值）；轮间换种保留「同优先级 piece 次序多样化」的
  libtorrent 随机选片意图。同族隐患：浮点排序一律 `f64::total_cmp`（而非
  `partial_cmp().unwrap_or(Equal)`——NaN 时 Equal 自反性成立但等价不传递，同样不是全序）。
- **回归测试**：8192 piece 大输入 `order_wanted` 无 panic 且结果为排列（不丢不重）+
  jitter 同种子恒定断言；验证方法：临时回退旧 jitter 跑回归测试，必须精确复现原 panic
  消息（smallsort.rs total order panic），证明测试真的罩得住。
- **花屏放大器（panic 钩子与被捕获恐慌的相互作用）**：`start_bt_task` 包装层把引擎任务
  panic 捕获转 `Evt::BtFailed(Fatal)`（app 不退出），但 panic 钩子在**恐慌发起时**（还
  未被捕获前）就执行 `restore_terminal()` 并让默认钩子打印 panic 报文——终端已退出
  raw/alt screen，而 TUI 循环还在向已还原的终端 draw → 花屏。修复：钩子还原时置
  `TUI_RESTORED_BY_PANIC` 原子标记；TUI 循环每轮先 swap 检查，命中则重进
  enable_raw_mode + alt screen 并 `terminal.clear()`（重置 ratatui 双缓冲，下一帧全量
  重绘），Fatal 原因正常以任务错误行展示。教训：**只要存在「panic 会被捕获并继续运行」
  的路径，钩子的终端还原就必须配套循环内自愈**，否则钩子反而成为花屏源头。

## pieces 窗口数据流：BT 用引擎真实三集合，HTTP 用 lease；fabricated lease 让在途分块连成一片
- **现象**：BT 下载中 pieces 窗口「正在下载」分块从前沿连成一片（而非 libtorrent 随机选块
  的离散分布）；已收块未验完的 piece 与已验完 piece 一度显示成未下载。
- **根因**：`on_bt_progress` 对 BT 任务也调 `apply_chunk_snapshot`——它按 HTTP 语义用
  （downloaded, total）算**连续 lease** 塞进 `t.connections`，`pieces()` 再把 lease 覆盖
  成 Active/Empty → 蓝块必然连片且与引擎真实在途集合无关；引擎侧 `partial_pieces` 只报
  在途请求的 piece，已收块未验完的 piece（引擎 partial 内存表）从未上报 → 黄色缺失。
- **口径**：BT 分块三集合全部由引擎上报——`done_blocks`（验完落盘）/ `partial_blocks`
  （partial 表有块未验完，黄）/ `active_blocks`（有在途请求，蓝，仅下载态上色）；BT 任务
  `connections` 恒空，`pieces()` 对 `bt.is_some()` 走 active 集合分支、绝不再走 lease 覆盖；
  HTTP 维持 lease 口径不变。`conns_display_total` 对 BT 数 `bt_conns` 行数（真实 peer 数）。
- **重启还原**：HTTP sidecar `blocks`（逐块字节进度）映射满块→Done、零头→Partial（旧实现
  只回读 downloaded，分块全白）；BT sidecar 版本恒 1，但 bug6 前遗留的同形 v2 文件放行
  （形状相同仅版本号不同），旧全量档嵌套形状仍拒绝。

## 同 peer_id / 同 IP 只连一条：入连+出连双通道是同 IP 两行与速度分裂的根因
- **根因**：0.0.0.0 监听后，同一 LAN peer 既被我们出连（tracker 给 ip:listen）又会拨入
  （ip:ephemeral）——入连 addr 端口不同，按 ip:port 去重挡不住；两条连接并存导致并发
  panel 同 IP 两行、choker/速度统计分裂。
- **口径**（对齐 libtorrent）：①`fill_peer_slots` 按 **IP** 去重（同 IP 不再拨第二条）；
  ②握手完成以 **peer_id** 去重（同一客户端实例只留一条，后到者发 Drop 关闭并回退候选，
  不计失败不惩罚）；连接建立即记 `peer_id` 到 ConnStat。测试用 fake tracker 解析 announce
  包内 engine 监听端口（BEP 15 第 96..98 字节）让 fake peer 拨入复现双通道。

## BT peer 速度列 '-' 一跳一跳：EMA 被窗口超时整体重置
- **根因**：BtConns 处理器里「上次进样 >1s 就整体 default() 重置速度统计」——引擎 1s tick
  与 WINDOW=1s 贴边，事件送达抖动即触发偶发重置 → EMA 清零 → 显示 '-' → 下一事件重建
  → 反复横跳。
- **口径**：处理器绝不重置 EMA；`SpeedWindow::push` 在上次进样超窗后仅清滑窗重新起算
  （rate 不跨停顿期摊分母）。EMA 只随任务失活 zero()，传输停止后平滑衰减到 0 B/s，
  状态切换单调无闪烁；'-' 语义 = 从未传过数据（EMA 恒 0）。

## 并发 panel 同 IP 两行（复发）：握手未完成的连接也渲染成行
- **现象**：bug7 修了 peer_id 双通道去重后，用户仍报「下载一段时间后同一 peer IP 两行」。
- **根因**：去重发生在握手完成（Connected）时，但交叉拨入的入连从 **accept 起**就以
  同 IP 挂在 conns 表里（peer_id=None），`conn_views` 照渲染——每次对方重拨都出现
  几秒的同 IP 第二行，反复出现看起来就是没修。
- **口径**：并发 panel 只展示**已建立**（握手完成、peer_id 已知）的连接；`conn_views`
  跳过 `peer_id.is_none()`。引擎去重语义不变（IP 出连去重 + peer_id 握手去重）。
  同 IP 不同 peer_id（NAT 后多实例）视为不同 peer，保留两行是 libtorrent
  `allow_multiple_connections_per_ip=false` 之外的合法可见情形。

## 分块窗口「并发」语义：显示实际连接数，不是任务设置
- **口径**：`conns_display_total` 抽出单任务版 `conns_actual_of`（BT=bt_conns 行数即已
  建立 peer 数、HTTP=活跃 lease 数、非下载态恒 0），pieces 弹窗元信息行与并发面板徽标
  同口径；任务设置的最高并发只在编辑对话框里体现。

## 暂停→继续进度清零：乐观暂停 + try_send 与旧引擎滞后退出的竞态
- **现象**：BT 暂停后继续，偶尔进度清零重新下载。
- **根因**：暂停是乐观置 Paused（引擎 select 循环异步消费 Pause），恢复若早于旧引擎
  真正退出（重校验大种子可达数十秒），会拉起**第二个引擎**：双引擎同写文件与 sidecar、
  互相覆盖进度集合、`t.downloaded` 被旧引擎终态回写——表现为进度回退/清零重下。
- **口径**：①app 层 `bt_stopping` 登记（暂停三分支写入；BtStopped/BtFailed/BtDone 清除；
  槽位调度见登记在册的 BT 任务先归还槽位不启动；180s 兜底强制放行防引擎挂死卡死任务）；
  ②引擎重校验循环内 `select!` 竞争 cmd——Pause/StopSeed 立即保存退出，**保存集 =
  已校验 ∪ 未校验预填余量**（只存已校验子集会让 sidecar 回退、余量被重下；余量下次
  载入本就重校验，零进度损失），Cancel 立即退出不保存（主循环同语义）；③`on_bt_stopped`
  分态处理：Paused=正常暂停流落地终态集合；Completed=停止做种流仅清显示；
  Queued/Downloading/Seeding=旧引擎滞后回报，整体忽略（状态不得回落、集合不得覆盖）。
- **配套**：出连 TCP 拨号超时 10s→3s（死 peer 不应占槽 10 秒，公网 swarm 死 peer 占比
  高会成片拖垮连接建立）；`handle_peer_msg` 的 claimed/availability 从「每条消息全量
  重算」改为按需计算（只有 Have/Unchoke/Block 补请求路径才构建）；Bitfield/Have 的
  `covered_pieces().contains` 线性扫描改 `PieceMap::covers` 二分（O(pieces²)→O(log n)）。
  harness 256MB 基准：ezr 2.02MB/s ≈ libtorrent leecher 2.0MB/s（同 swarm 同 seeder）。

## 上传服务必须在引擎循环外：inline 读盘+发送是「加 peer 反而降速」的自饿根因
- **现象**：1 seeder（限速 2.5MB/s）+ 1 并行 leecher 场景，ezr 下载均速 0.77MB/s
  （公平份额 1.25MB/s）；leecher 停止索要后立即恢复满速。单 seeder/多 seeder 场景
  全部正常，故循环内 refill/claimed/热路径均非根因。
- **根因**：`PeerMsg::Request` 的服务路径（`read_piece_range` 读盘 + `send` 16KiB
  Block）内联在引擎任务里。libtorrent leecher 管线可达 500 个在飞请求，每个请求的
  读盘 await 把整个引擎（全部连接的消息处理、refill、tick）串行阻塞，自身下载被
  Request 洪流饿死。libtorrent/qBittorrent 的对应解法是 aio 线程池。
- **处置**：Request 分支改为「预计算读盘计划（upload_read_plan）+ tokio::spawn 任务
  （read_upload_blocks + 发送）」。三个配套纪律：①上传读必须**独立 open**
  （`tokio::fs::File::open(path)`），`try_clone` 是 dup——共享 open file description
  （偏移），与引擎写句柄并发 seek 会互踩（回归测试：serve 前后引擎句柄
  `stream_position` 不得变化）；②进程级 `Semaphore(128)` 限并发读盘防 fd 洪泛；③
  发送阶段不持读盘资源，慢对端由 out 通道（cap 64）背压天然限流。cum_up 改为
  serve 决策时计入（读盘失败才可能多计，量级可忽略）。

## 同 IP 候选去重有两个盲区：入连不查同 IP、无用连接永不重估
- **现象**：三方同机（seeder/leecher/ezr 全在 127.0.0.1）时 ezr 首连随机落在空
  leecher，`fill_peer_slots` 的同 IP 去重把 seeder 候选**永久**挡住（0 字节永远等
  不到），直到 leecher 自己从 seeder 下完转做种才解锁。且入连 accept 无同 IP 检查，
  一条入连就能占住该 IP 的出连名额。
- **根因**：同 IP 只连一条本身是 libtorrent `allow_multiple_connections_per_ip=false`
  的正确口径，但 libtorrent 有配套的 peer 置换机制（inactivity_timeout 等），ezr
  只有去重没有置换——去重从「防重复」变成「一选定终身」。
- **处置**：①tick 内空闲置换：`!seeding` 且连接 cum_down 60s（`IDLE_EVICTION_SECS`，
  对齐 libtorrent inactivity_timeout=60）无新增字节 → 断开（conns.remove 先行，
  Closed 路径即不计 fails、不触发退避）→ fill_peer_slots 重拨同 IP 下一个候选；
  ②入连 accept 走 `inbound_admitted`（同 IP 已有连接即拒绝 + 容量上限），入连不再
  能占住同 IP 出连名额。地址不可解析的既有连接不参与同 IP 判定（容忍手测假地址）。

## config 三大标准表键位总账：[common] 公共 / [http] HTTP 引擎 / [bt] BT 引擎，旧格式不兼容
- bug11 定案（操作者钦定）：配置键按「<表>.<键名>」归属——`[common]` = download_dir /
  download_slots / max_speed + `[[common.proxy]]`（proxies 改名迁入，字段不变）；`[http]` =
  block_size_http / http_concurrency / max_retries / auto_retry / backoff_initial / backoff_cap +
  调优键 progress_tick_ms(100) / sidecar_flush_secs(2) / throttle_burst_secs(0.5) /
  throttle_sleep_ms(20) / dns_race_grace_ms(1000)；`[bt]` = http_trackers / udp_trackers +
  调优键 20 个（bt_concurrency(20) / peer_pipeline(64) / upload_slots(4) /
  upload_read_concurrency(128) / idle_eviction_secs(60) / connect_timeout_secs(3) /
  handshake_timeout_secs(10) / request_timeout_secs(20) / requests_per_piece(4) /
  rechoke_interval_secs(10) / optimistic_interval_secs(30) / progress_emit_min_interval_ms(250) /
  tracker_numwant(200) / peer_retry_base_secs(5) / peer_retry_cap_secs(120) /
  tracker_backoff_start_secs(8) / tracker_backoff_cap_secs(60) / udp_base_timeout_secs(15) /
  udp_retries(3) / seed_duration_secs(1800)），全部平铺表内不设子表。调优键 = 代码常量外置
  （缺省取常量；常量表见 FR-02-25），统一回退口径 0/负/非法 → 代码常量默认，
  bt_concurrency/http_concurrency 越界钳制 1–64。**旧顶层平铺键与 [[proxies]] 按未知键静默
  忽略（零警告零迁移，内部开发无兼容需求——与 sidecar v2 同一口径家族）**；写 QA/测试配置
  时必须带表头，否则键被忽略、用例假绿或假红。

## 并发 panel 列宽按极值样例 +1：地址 18 = ffff:*:ffff/65535，客户端 34 = Propagate Data Client/35.35.35.35
- 列宽演进三笔账：地址 11→15（bug10，`255.*.255:65535` 恰满）→ **18**（bug11，格式改
  `ip缩写/port` 斜杠分隔，IPv6 极值 `ffff:*:ffff/65535` = 17 列 + 1 空白）；客户端 6→26
  （bug11，值改 `<全名>/<版本号>`，旧 62 项表极值 25 列 + 1 空白）→ **34**（bug12，
  映射表换 102 项全量后极值重推：全名最长 Propagate Data Client 21 列 + 版本极值
  35.35.35.35 11 列 = 33 列 + 1 空白；旧极值样例 ArcticTorrent/100.100.100 基于 62 项旧表，
  换表必须重推极值——漏推即极值溢出列宽破坏对齐）；内容宽 BT_COLW 求和 + 列数 − 1 =
  94（content_w_matches_colw 断言守护）。对齐口径：**表头（列头）BT/HTTP 全部左对齐**
  （pad_right），明细行**数据右对齐、地址列与客户端列数据左对齐**（bug12：客户端值视作
  字符串）——表头跟随数据对齐是旧实现的顺手写法，不是规格。客户端全名/版本解析 =
  qBittorrent 源码口径两级（见下条）。

## 客户端识别 qBittorrent 口径的两级源码链：扩展握手 v 优先，peer id 解码 = lt identify_client 全量移植
- **源码链核证**（qBt f6cf56c + lt 1.1/1.2/2.0/master 四分支比对）：qBittorrent 的
  `PeerInfo::client()` 直接返回 `lt::peer_info::client`，而 lt 侧 `m_client_version` 有两条
  写入路径——①peer 发 BEP 10 扩展握手时 `v` 字符串**覆盖** identify_client 结果
  （bt_peer_connection.cpp）；②无 `v` 时握手后 `identify_client(pid)` 解码。**没有任何
  libtorrent 分支对 BitComet 做版本特判**——qBt 显示 BitComet 1.64 全靠 BitComet 自报
  `v`="BitComet 1.64"；只抄 peer id 解码路径必然复现 0.1.6.4（旧 62 项子集表的缺陷）。
- **移植要点**：identify_client 匹配序 = generic_mappings 37 项（offset+前缀子串、仅名无
  版本，先于一切 style）→ -BOW/eXeem/Experimental 特判 → Transmission legacy（-TRxYYR-，
  YY=两位 minor 合一数字）→ Azureus（首 -、码后首字符可打印、第 3–7 字符 ≥'0'、尾 -；
  版本逐位 decode_digit = 数字原值/大写字母→10–35，**不看小写字母**——小写经 c-'A'+10
  得 ≥42 的大数是 lt 原语义，照抄）→ Shadow（`--` 结尾臂逐位 decode / 否则原始字节臂且
  第 9 字节须 0）→ Mainline（sscanf %1c%3d-%3d-%3d--）；lookup = name_map 102 项按码
  字典序查找、未命中用码本体为名、版本 `M.m.r`（tag=0 省略第 4 段——qB2500 → 2.5.0 不是
  2.5.0.0）；全零 → Unknown；全不匹配 → Unknown（ezr 不采用 lt 的 `Unknown [原始串]`
  展示形态——列宽约束，v1.35 既有口径）。**别再把「映射表外码」判 Unknown**——lt 显示
  码本体 + 版本。
- 判定「qBt 到底怎么显示」要看 lt 的 `m_client_version` 全部写入路径，不能只看
  identify_client.cpp；扩展握手解析在本项目属 BEP 9 框架既有路径（parse_ext_handshake
  同一 dict），读 `v` 字段不越 phase-03「完整 BEP 10」边界。

## tracker 修改不生效的根因：确认路径只改模型，引擎 tracker 池无热更新命令
- `dlg_confirm_modify_bt` 确认后仅 `Cmd::Reconfigure`（并发/代理）+ 文件/并发变化才
  CancelBt+StartBt 重挂；只改 trackers 时 `changed == false` → 引擎沿用启动时 spawn 的
  tracker announce 循环（`spec.trackers` 解析后逐个 spawn_tracker，任务级 Vec 固定），
  模型里改的 `b.trackers` 引擎永远看不到。修复口径（FR-02-24）：确认后向引擎发 tracker 池
  热更新命令——新增 tracker 立即 announce、移除的不再轮询、连接/进度不动、下载中与做种中
  同口径；判断依据是 fixture tracker 日志（announce 到达与停止），不是界面状态。

## 引擎调优常量外置的注入模式：spec 携带 BtTunables / EngineShared 承载进程级量

- 模式：per-task 调优键（BT 17 键）经 `BtTunables`（supervisor::BtTunables，Default =
  config::DEFAULT_*）随 `BtSpec` 进引擎，引擎内 `let tun = spec.tunables.clone()` 后
  各 helper 以参数下传（自由函数不读全局常量）；进程级量（DNS grace / 上传读盘信号量 /
  限速 burst+sleep）挂 `EngineShared`（start(cfg) 时构造）。模块级常量全部退役到
  `config::DEFAULT_*` 单点，模板示例值与代码默认共用一源。
- 陷阱：①测试 BtSpec 字面量 29 处需补 `tunables: Default::default()`；②自由函数
  （cooldown_ok/idle_evict_due/progress_emit_due/apply_choker/udp_roundtrip/handshake）
  全部要加参数，漏一处即 E0425；③EngineShared 测试构造点（shared_for_tests/supervisor
  tests）要补 grace + upload_read_permits 两字段。

## tracker 池热更新：AbortHandle 池重建 + tr_tx 保活 + 失败计数重置

- 08-modify-bt-06 口径：M 改 trackers 确认后立即生效、任务不中断。实现 = `BtTaskCmd::
  UpdateTrackers(Vec<String>)` → 引擎 abort 旧池句柄（spawn_tracker 返回
  JoinHandle::abort_handle，池句柄随 trackers/completed_notifies 平行维护）→ 重新
  parse+去重建池（`start_tracker_pool` 统一初建与重建两路）→ tracker_fail_count=0、
  got_peers 保真。`tr_tx` 初建后**不能 drop**（重建还要用它），stopped announce 用当下
  池快照，行为自然正确。
- app 侧接线：`dlg_confirm_modify_bt` 记 old/new trackers，`trackers_changed &&
  !pool_updated && state∈{Downloading,Seeding}` 才发 Cmd::BtUpdateTrackers（纯 tracker
  修改不重挂引擎；total/并发变化走既有重建路径，新 spec 自带新池，双发会重复 announce）。
- spy 测试注意：确认路径先发既有 `Cmd::Reconfigure`（并发/代理热调），断言要跳过它再
  找 BtUpdateTrackers，且断言无 CancelBt/StartBt（不重挂）。
- 并发 panel 选择钳制必须与明细行数同源：`tick_clamp_selection` 的 `conns_sel` 钳制此前误用
  HTTP 连接快照长度（`t.connections.len()`），而 BT 任务 connections 恒空（peer 行在
  `bt_conns` map）——每次 tick（约 100ms）把 `conns_sel >= 1` 强制复位为 0，表象即「BT 明细
  永远只能选中第一个 peer，↑↓ 与鼠标看似无效」。根因模式：同一索引被两处数据源引用（渲染
  行集 vs 钳制基线）时，钳制基线必须与渲染行集同一来源（BT = bt_conns.len、HTTP = 活跃连接
  行数），与 `conn_count()`/`conns_actual_of` 同源；修复落在 tick 钳制一处，渲染/键位/鼠标
  均不动。同类隐患：任何「选择索引 + 周期钳制」结构都要求钳制计数与面板实际行数同源。
- 保存目录归一层位 = 任务创建确认时，不在配置解析层：`download_dir`（config）与对话框/CLI
  输入统一在任务创建确认时做 `~` 展开 + 相对→绝对（按创建时进程 cwd）+ 末尾 `/` 去除；配置
  解析层保留原文（同一 config 在不同 cwd 启动语义一致，归一后果随创建时上下文）。既有
  `expand_tilde`（种子路径/cmd 窗口已在用）直接复用，缺的只是四条创建路径各自调用——修此类
  bug 先找「既有的归一函数 + 未接线的路径」，不要新造归一实现。
- supervisor block_worker 的「无法写入目标文件」fatal（`OpenOptions::write(true).open` 失败）
  与重试路径存在理论竞态：resume 作废重建目标文件与 worker（重）开文件交错时 ENOENT →
  任务 Fatal。QA-PC-06 全量套件语境下低频复现（隔离 11 连跑零复现；慢速逐键清场把任务添加
  窗口拉长 ~0.5s 后暴露率上升，清场提速后 3 连跑稳定）。本轮未改引擎，登记为 QA/引擎轮
  观察项：worker 打开失败可考虑有界重试窗（文件由 supervisor 先 touch 再派 worker 即根治）。
## bug14 轮（specifier）
- 「协议」表头只显示「协」的根因：视口右缘恰好落在末列宽字（议）的中缝，ratatui 渲染层对跨界宽字整字丢弃（不渲染半个字），视觉上=表头被截断；v1.34 的「+1 放宽」只保证滚到最右完整，不保证默认视图/中间偏移。探针实证矩阵（TestBackend buffer 直读）：面板宽 98（视口 92）默认视图、面板宽 50 + hscroll 48/49 均复现「协」；hscroll≥50 或视口≥94 完整。修法=客户端列自适应收缩（内容>视口时客户端 34→max(12, 视口宽−60)，整表无滚动可见）+ 末列完整可见吸附（右缘 ∈ (内容宽−4, 内容宽) 时偏移吸附到最大）。ezr-demo 同构同现（同列宽同 head_cell），属设计基线级现象非 ezr 实现独有。
- 并发速度合计不一致的协议差异：BT 任务速度=Σ per-conn EMA（bug12 已同源）；HTTP 任务速度走独立任务级窗口 EMA 链（tick_speed_and_session 的 speed_display），与并发行 SmoothedSpeed 结构性不同源。v1.39 统一为两协议 Σ per-conn EMA；源不变式严格成立、显示层各自 fmt 的亚 0.1 KB/s 舍入差登记为实现定义值。
- config 解析失败静默回退是「改配置不生效」的根因载体：from_toml 用 toml::from_str(text).unwrap_or_default()，非法 TOML → 空 ConfigRaw → 全默认 + 零警告，用户无任何感知。v1.39 翻转为启动即报错非零退出（仅「解析失败」层致命；逐键非法值回退与未知键静默忽略口径不变——三层语义要分清）。
- 模板「示例行活跃」改版（v1.13）的连带语义：首启 config.toml 直接可改值（消除「只改键值不动 # 表头导致键落根表被未知键忽略」的坑）；proxy 双示例块必须保持注释（激活=注入两个占位死代理），36 键白名单取消注释要精确到键名（proxy 块的 name=/type= 行同形态但不得动）。

## bug14 轮（coder 实现）
- 解析失败分层的最终口径（D63① 落地版）：toml 反序列化层错误（语法错 + 键类型不匹配）= try_from_toml Err = 启动致命；反序列化成功后的语义非法值（0/负/越界/非法字符串值）= 逐键回退默认。规格 v1.39 措辞已同步此分层（原「类型错逐键回退」表述与 serde 类型化结构体不可实现，正确口径 = 结构层致命、语义层回退）。
- 浮点键整数写法容忍（num_f64）：backoff_initial/backoff_cap/throttle_burst_secs/seed_duration_secs 的 ConfigRaw 字段改 Option<toml::Value>，Integer/Float 通吃、其余回退——否则用户写 `throttle_burst_secs = 1`（整数）会整体解析失败误伤致命；模板 v1.13 的 seed_duration_secs = 1800（整数）也依赖它。
- HTTP 任务速度 Σ 接线：sample_conn_speed_display 聚合 per-conn EMA 值写 t.speed（totals map 两段式避开借用）；speed_display EMA 链整链退役（字段/zero/entry/evt 清理）；任务级 windows 滑窗保留（evt/bt 仍喂、生命周期测试锁定——仅失去 rate() 读者，cleaner 轮可退役登记）。
- conns 自适应收缩/吸附实现：client_w = 视口<94 时 max(12, vw−60)（BT_CLIENT_SHRINK_BASE=60 = 除客户端外六列宽+分隔）；内容宽动态 = 94−34+client_w；吸附条件 edge+4>content_w && edge<content_w（edge=hs+vw，落入 (c−4,c) 即吸到 max），渲染与滚动条位置同用吸附后偏移。
- BT 种子路径归一接线点 = dlg_confirm_add_bt 确认链（normalize_save_dir 复用，~ 展开+相对转 cwd 绝对），fs::read 直读归一后路径（expand_tilde 冗余化），判重与 Task.url 同用归一值；预填层不改写。

- D64 收缩下限：client_w 下限 12→26 = 极值样例 ArcticTorrent/100.100.100（25 列+1 空白）所需列宽，BT_CLIENT_MIN 单常量点（ui/conns.rs）；视口 <86 后内容宽 86 > 视口走既有水平滚动条（吸附臂 edge+4>c && edge<c 不变、hmax=内容宽−视口+1 不变），≥94 维持 34 列钦定形态；列位置在客户端列之前者不随 client_w 移动（窄视口 N5 类断言的可见性只取决于列起点不取决于 client_w）。
- ezr-demo 同构同步点 = ui/conns.rs 收缩式 max(12→26)（demo 无 BT_CLIENT_MIN 常量，colw[5] 内联）+ README 变更记录 + 设计规范「本轮需求修订」块；demo 无引擎故 ezr 独有文案（黑名单 toast/校验 computed=expected/重试计数）不属同步面；chips `A 命令` 两端实现一致（规格/QA 文档写 `A 添加` 为 phase-02 命令窗口改版时的文档滞后，非两端漂移）。

## bug16 轮（specifier）：BT 引擎无并发热调命令——改并发的现行实现是全量重启（bug16 根因事实）
- `BtTaskCmd` 全集 = Pause/StopSeed/Cancel/Disconnect/UpdateTrackers，**无并发/Reconfigure
  命令**；`Cmd::Reconfigure` 只查 HTTP 任务表（`reconfigure_task(tasks, …)`），对 BT 任务是
  no-op（BT 引擎句柄在独立的 bt_tasks 表）。
- 修改任务：BT 确认链（`app/bt_dialog.rs::dlg_confirm_modify_bt`）：模型更新 + 发
  Reconfigure（BT 侧无效）+ 若 total/并发变化且状态 ∈ {Downloading, Seeding} → **CancelBt +
  StartBt 全量重启**（新 spec 带新并发）。探针实证（spy 引擎命令序列）：只改并发 →
  Reconfigure + CancelBt + StartBt 三连。
- 全量重启的可感知代价 = 连接整体拆除 + 启动重校验（bt/engine.rs 启动注释：大种子逐块
  SHA-1 可达数十秒）→ 用户感知「改并发未生效、需暂停重启才生效」。app 侧旧引擎滞后退出
  的 BtStopped 回报在 Downloading/Seeding 态被忽略（on_bt_stopped `_` 臂），状态机无回落。
- FR-02-24 的 tracker 池热更新先例 = 引擎内 AbortHandle 池重建 + 不重挂引擎；并发热调
  同理应在 BtTaskCmd 新增命令 + 引擎内重排连接（上调补建、下调即时收缩），杜绝 CancelBt+StartBt。

## bug16 轮（coder）：BT 并发热调落地版实现图谱（D65① 落地）
- 命令面：`BtTaskCmd::Reconfigure { concurrency, files, proxy_endpoint }`（bt/mod.rs，BtFile 需 use crate::model）+ `Cmd::ReconfigureBt` 转发（engine/mod.rs bt_send）；app 确认链（dlg_confirm_modify_bt）running 态（Downloading|Seeding）发 ReconfigureBt，trackers_changed 另发 BtUpdateTrackers，**CancelBt+StartBt 路径整体退役**（torrent 重读/重解析不再需要，toast 恒成功）。
- 引擎 cmd 臂顺序：目标并发 clamp(1,64) → files_changed 则重建 pmap+order_cache、补开新选文件句柄（open_selected_file_handle 复用启动路径，Result 化）、逐连接 cancel_inflight_not_covered（try_send Cancel，非覆盖 piece 块退还缺口池、partial 内存保留）、recompute_downloaded（= Σ done × selected_bytes，消除 downloaded 超越新 total 的漂移）、seeding 反转臂（改选使 all_selected_done 不成立时回 downloading）→ shed_excess_conns（先无在飞后最少累计，Drop）→ 补建 fill_peer_slots（上调立即，非 seeding 才拨）→ proxy_changed 重建 tracker 池（同 UpdateTrackers 路径）→ BtConns 立即刷新（连接行一帧内消失）。
- on_block 新守卫：非覆盖 piece 数据早退（不写盘不计字节不发 Have，退 conn 并续 issue_requests）——撤销在飞后对端仍会发数据的兜底；run() 的 spec/pmap 改 mut（spec.files/concurrency/proxy_endpoint 热可变）。
- 下调保字节语义 = D30 同构：超额连接的在飞请求随连接拆除立即中断，`partial` 按 piece 键存储不受连接拆除影响，块内已写字节保留、claimed_blocks 自动回归缺口池。

## bug17 轮（specifier）：BT 连接生命周期反吸血触发面与 qBt 对等口径
- 反吸血触发面三处（代码实证）：①peer_id 每引擎随机（bt/engine.rs run() 调 peer_id.rs make_peer_id）——暂停/重启后同 IP 换 peer_id 立即重连，正对 BitComet「换 ID 重置统计」吸血特征，qBt/libtorrent 每 session 一枚全 torrent 共用故不触发；②引擎 Pause/StopSeed/Cancel 臂直接 return，连接随 HashMap 丢弃（读半部滞留 + 强制收口存在 RST 面），qBt 为有序 FIN 关闭；③自关后候选 last_try 不刷新（shed/evict 臂先 conns.remove 再发 Drop，候选保留旧 last_try）——回补调度对刚关候选零冷却，「断开即刻重拨」。
- 修法口径（FR-02-27/D66）：peer_id 会话化（引擎侧 OnceLock，进程内全任务共用、跨重启不变——对 21 处测试 spec 构造点零扰动，夹具固定 -QX 前缀与自连守卫互不干扰）；自关一律「先关写 FIN → 排水窗口 → 超时强收」；自关刷新候选 last_try（复用既有 cooldown_ok，fails 不动）；失败退避与同 peer/同 IP 单连接、tracker started/stopped/completed 事件不变。
- 判据全部 fixture 侧可观察：握手 peer_id 全等、seeder 读到 EOF 非 ConnectionReset、同候选两次入连间隔 ≥ 冷却、tracker 事件序列一致。

## bug17 轮（coder）：引擎退出连接滞留根因（out_tx 残留）与生命周期落地版图谱
- **引擎退出连接滞留根因（本 round 最重要发现）**：run_peer_loop 把 `out: out_tx.clone()` 交给引擎、**原件留驻自己栈帧**直到函数返回——而 peer_pump 正是该函数的 await 体，永不返回。引擎 run() 返回丢弃 conns 后，写循环 `recv()` 永不 None（sender 计数仍 ≥1）→ **暂停后连接整体滞留（fd+task 永久泄漏、FIN 永不发出）**；对端视角 = 旧连接挂着 + 同 IP 新连接（换 peer_id）并行 = 反吸血双杀。修法 = Connected 消息改为 **move out_tx**（引擎侧 ConnStat.peer_out 成为唯一 sender），引擎退出即触发写循环 None→收口，一处改动根治全部返回路径。
- graceful_close 原语（peer_pump 收尾统一）：`wr.shutdown()`（显式 FIN）→ 排水窗口（GRACE_CLOSE_SECS=5，timeout 等 reader 自然退出：有消息即转发失败自灭，静默则等超时）→ `reader.abort()` 强收读半部。替代旧「drop(wr) + 5s detach」——旧路径 reader 永久 detach 持 fd。
- 自关冷却落地：shed/evict 臂在 conns.remove 后**刷新候选 last_try**（回补/置换调度复用既有 cooldown_ok，fails 不动）——原缺陷 = 自关不刷新 last_try，回补调度对刚关候选零冷却（TDD 红证：回补仅 528µs）。首轮规格「Closed 处理器误记 fails += 1」经实证更正：各自关臂先 remove 再发 Drop，处理器 `conns.remove` 返回 None 整段跳过，fails 从未被污染——**缺陷在调度侧不在记账侧**。
- TDD 夹具增补：probing_seeder（记录对端握手 peer_ids + 终态读错误串，serve_seeder_stream 抽取复用并 Result 化）——peer_id 会话稳定性（两轮 run 全等）、暂停收口 EOF 判据、自关冷却三测试共用。
- 陷阱登记：tokio read_exact EOF 文案是 **"early eof"**（非 std 的 "failed to fill whole buffer"）；OwnedWriteHalf 无 Clone（两阶段写测试用 oneshot 门闸单任务内完成）；TCP 半关闭后对端**第一次**写仍成功（RST 由第二次写触发）——强制收口断言必须写两次。

## bug18 轮（specifier，五项功能规格轮）

- 限速现状证据链：全局限速 = EngineShared.throttle 单点（engine/mod.rs start() 构造，仅 HTTP 管道接入）；BtSpec.max_speed 字段带 `#[allow(dead_code)]`（BT 引擎从未消费）——「BT 下载限速未接」遗留项的机制落点；cfg.max_speed 同时被 bt_dialog 预填进 BtSpec.max_speed（形同任务级预填但全链路无效）。限速全域化落点 = 协议无关层共享配额（EngineShared 单点扩上传方向）+ BtSpec 任务级两字段 + Reconfigure 载体扩展（bug16 热调命令面已有）。
- 反吸血设计口径（D67）：qBittorrent/libtorrent 无贡献比率自动封禁（仅协议违规断开+手动 ban）→ 主参考 BitComet PSR；三判据 leech（up≥1MiB 且 down<64KiB 且存活≥120s，仅下载侧任务，做种豁免——做种无私上传合法，纯 leecher 拉做种不罚）/ bad_piece（3 次）/ protocol（破坏性消息首封、非法区间 request 3 次）；黑名单 = EngineShared 既有名单（is_blacklisted 三过滤面：拨出候选/入连 accept/tracker peers 吸纳）+ blacklist.yaml 持久化层（损坏=空名单+警告非致命——与 config 致命口径分界）。
- trackers 窗口数据源 = 引擎 tracker 池真实状态（interval 倒计时/失败退避/未运行三态）；t 键退役 = keys.rs cycle_filter 的 t/T 臂删除，页签循环仅 ]/[ 承担（帮助【页签】组条目替换不改总行数）。
- listen_port：现状每任务 `bind("0.0.0.0:0")` 随机 + `unwrap_or(6881)` 展示兜底；目标 = 进程级共享单端口（qBt 会话口径；每任务独立 listener 的分散形态退役）。

## bug19 追轮（specifier，操作者三条 UI 反馈修复 + trackers 弹窗改版轮）

- 「面板有值、弹窗空」缺陷的机制模式：演示预填数据只进对话框缓冲、不进任务档案（demo_bt_trackers 仅修改面板空任务时预填、确认链不落库）——同源数据的两处消费点（T 弹窗读 t.trackers / 面板预填读 t.trackers）必须同库；演示口径的预填必须走创建链入库。修法 = 新增 BT 任务 trackers 留空确认时按「种子元数据自带」口径预置默认 trackers 入库。
- wss 泄漏双源修法：源 1 = 演示预填数据本身含 wss（改 4 条 http/https/udp）；源 2 = 用户输入不经校验直落任务档案——确认链逐行 scheme 白名单校验（http/https/udp，大小写不敏感，首违即拒），toast + 焦点回 trackers + 整单不落库（qBt 确认语义），与引擎侧拒入池构成双过滤面。
- trackers 弹窗改版落点：qBt 列口径 = #/URL/状态/Peers/Seeds/消息（类型并入状态，状态三态不变）；URL 换行 = wrap_by_width 按显示宽贪心折行 + 续行仅回填 URL 列（首行携带全部数据列）；折行后显示行为滚动单位（flat 行集，滚动条 max = 总显示行 − 可视行）；列排版 push_col 按绝对列偏移铺 span（cur 游标推进，CJK 宽度走 w()）；SbKey::Trackers 五件套（enum/bar/max/scroll/set）与 pieces 完全同构。

## bug20 追轮（specifier，操作者六条 UI 反馈：细线分隔/续行同色/反终端自动链接/对称 padding/删底部提示）

- 渲染层反终端自动链接注入纪律：VS15（U+FE0E，零宽、GCB=Extend 不独立成格）+ 相邻 span ±1 微差色（fg_shadow RGB±1）双保险打断终端 URL 模式识别；注入点必须在 disp_window（水平视口切窗）/折行分段**之后**——直接改缓冲会污染数据源并使光标/点击映射失准，注入在切窗前则视口边缘切片可能漏 scheme；共享组件经 `no_link` 参数收口（命令窗口 false、BT trackers 面板 true），udp 行不注入（终端不识别非 http 形态，无需注入）。
- 浮层删底部提示行的两处联动：可视行公式随动（vis = 内框高 − 3）；滚动条 total 口径把分隔细线计入 flat 行集。删显示类 UI 反馈必须同步翻转验证脚本断言方向（原「有提示」断言改「无提示」），否则旧断言假绿掩盖新口径。
- 表格右 padding 对称推导：左 padding 2 + 末列文案宽恰好用满 content_w ⇒ 右视觉 padding = 左 padding。本轮 URL 列 36→44 承接增量、状态/Peers/Seeds/消息 右移（49/62/69/76），消息列预算 = content_w − 76 = 8（最长文案「重试 29s」= 8 列）恰好齐平——改列宽的核平公式：最长列文案宽 = content_w − 列偏移。
- 折行同色的实现口径：续行与首行同一 Style（FG）即可，颜色一致性由 span 构造点保证；验证脚本对纯色差断言无 PTY 手段（文本捕获不带样式），以「续行无 # 前缀回填 URL 列」结构断言 + 代码构造点评审双保险登记。

## bug18-coder 轮（2026-11-02，五项功能实现落地图谱）

- 全局/任务级限速的 min 合成实现口径 = 每 chunk 对全局桶与任务桶**各 acquire 一次**（两桶串联取币，紧桶自然成为瓶颈）——不能预合成 min 值建单桶：全局桶是跨任务共享配额，任务桶归任务私有，合成语义必须在消费点保持两桶独立。EngineShared 拆 throttle_dl/throttle_ul 双桶（dl = HTTP+BT 下载共享，ul = BT 上传），Throttle 新增 set_rate 支持运行时热调（rate=0 = INFINITY 不限）。
- BT 下载限速挂点 = peer_pump 读循环（read_msg 后、转发 PeerIn::Msg 前，仅 Block 消息 acquire）：per-conn 并行取币不阻塞引擎主循环——若挂 on_block（主循环）会让 accept/tick/cmd 全部被限速等待拖住。上传挂点 = Request 臂 spawn 任务内、send(Block) 前（ul 全局桶 + 任务桶），不阻塞 select 主循环。DlFlow{global,task} 经 spawn_outgoing/incoming_peer → run_peer_loop → peer_pump 透传。
- 进程级共享监听端口的落地形态 = std::net::TcpListener 同步 bind（EngineHandle::start，占用回退随机 + 启动警告收集后 toast）+ EngineShared 持 std listener 与 `OnceLock<Arc<tokio TcpListener>>`：tokio from_std 只能注册一次，首个 BT 任务在 run() 内 get_or_init 转换，后续任务 clone Arc；announce port = listener.local_addr().port() 全任务同值。bind_bt_listener 必须在 cfg 解析后、main_loop spawn 前执行。
- 反吸血三判据落点：leech 判据在 1s tick 臂遍历（!seeding && cum_up≥1MiB && cum_down<64KiB && connected_at.elapsed≥120s，做种豁免 = seeding 布尔）；bad_piece 在 on_block 校验失败分支计 st.bad_pieces（≥3 封禁，归属交付完成块的 peer——piece 可能多 peer 拼块，归属近似为完成者，D67 允许实现定义）；protocol 三源 = Unknown 消息 id 首封 + pump 读循环 BtError::Peer(_) 帧错（区别于 BtError::Io 的网络错，不改协议枚举加 PeerIn::ProtocolViolation 变体分类）+ 非法区间 request 计 st.invalid_requests（≥3 封；choking 期间的 request 不计数——合法行为）。统一 ban_and_drop：黑名单落 EngineShared + blacklist.yaml record + ⛔ toast + PeerOut::Drop（graceful_close 自动满足 FR-02-27②）+ 候选池摘除。
- blacklist.yaml 持久化的测试隔离门 = EngineShared.blacklist_file: Option<PathBuf>（生产 Some(ezr_dir/blacklist.yaml)，shared_for_tests 恒 None）——封禁副作用挂 record 调用点，测试零磁盘副作用；同 IP 重复封禁不重复记条目；损坏=空名单+警告文本返回（load 返回 (entries, Option<warn>)，警告并入口径沿 cfg.warnings 的 ⚠ toast 通道）。
- trackers 弹窗真实数据源 = TrackerReport 增加 url + interval/retry_secs（失败分支先算退避后 send），引擎 tr_rx 臂转 Evt::BtTrackers{id,url,working,peers,seeds,next_in_secs}；app 侧 TrackerRow{url,working,peers,seeds,next_at} 存 HashMap<task_id, Vec>，渲染按 t.bt.trackers 权威序 + url 查状态；任务未运行（state.shows_conns() false）或无引擎回报 = 待 announce；倒计时存 Instant 在 draw 期换算剩余秒（不存秒数避免静止倒计时）。

## bug23 轮：限速单位口径三层归一与 BT 暂停→恢复的重复下载特征（反吸血修复）
- 限速单位三层口径（bug23 定案）：config 文件键解析 = **二进制 KiB/MiB/GiB（1024^n）
  为新规范 + 旧写法 KB/MB/GB 仍按十进制兼容**（parse_speed 的 SPEED_UNITS 表按后缀
  最长优先排列：gib/mib/kib 先于 gb/mb/kb 先于 b——旧表只有 mb/kb/b，「512 MiB/s」
  会被 b 后缀截成「512mi」解析为 0 = 静默不限速，这是旧口径真缺陷）；对话框限速
  字段 = 二进制（speed_unit_mult，bug21 既有）；**裸数字恒为 B/s**（parse_speed 与
  parse_speed_limit 均然，测试钉死）。
- 限速预填显示规范 = fmt_speed_limit（GiB→MiB→KiB→B 整除链，非整倍数显裸 B/s，
  0 显「0」）：5 处预填点全接（Add/Modify HTTP、AddBt/ModifyBt、cmd open_add_dialog_with），
  fmt→parse 恒等（round-trip 测试钉死）——「修改窗口显示裸整数 51200」类困惑的
  根治 = 显示层永远带明确单位，而不是改解析。
- 修改窗口限速**清空确认 = 回填 config.toml 任务级默认**（task_max_download_speed/
  task_max_upload_speed，HTTP 无上传字段），不再保留旧值；添加窗口空输入本就回退
  down_prefill（=config 默认）无需改——「空输入」语义两窗口统一为「跟随配置默认」。
- **BT 暂停→恢复触发 BitComet 反吸血的机制链**：Pause 直接终结引擎任务（连接随
  引擎退出），partial（已收块、未验完 piece 的内存缓冲）全丢；Resume 拉起新引擎
  重新向同一 peer 请求**完全相同的 (index, begin, length) 区间**——对端可观测的
  「同区间重复下载」正是反吸血规则的触发特征（下载量涨、上传恒 0、进度零增益）。
- 修复三件：①BtSidecar 增可选 partial_pieces（serde default + skip_serializing_if
  → **版本号不 bump**：旧构建 serde 忽略未知字段照常读 done，新构建读旧文件得空
  partial——字段级前后向兼容比版本 bump 平滑）；块数据 b64 存 String（字段级
  decode 失败只丢块不拖垮整档，done 进度免受池鱼）。②restore_sidecar_partial 启动
  恢复：验证链 = 覆盖内 ∧ 非 done ∧ begin 对齐 REQUEST_BLOCK_LEN ∧ 单调不重叠
  （next_begin 游标）∧ 界内 → 进 partial；**完整恢复的 piece 直接组装校验落盘转
  done**（留全 claimed 的未验 piece 会掉进 endgame 陷阱）；校验失败/写盘失败整块
  丢弃走重下（自愈）。③endgame 必须跳过 partial 已收块——endgame 语义是「找回
  丢失的请求」，已收块不是丢失；不加此跳过，恢复后的 endgame 窗口照样重发恢复块
  （实测复现后才定位到：claimed 检查只挡非 endgame 臂）。
- 启动序列次序陷阱：先 load_full 恢复、再 save_bt_sidecar_full 回写——反过来
  （先 done-only 保存再加载）会把文件里的 partial 抹掉；启动回写同时完成归一化
  （滤掉恢复中验证失败/写失败的残条）。
- 中断点语义边界：启动重校验窗口内的 Pause 仍存「已校验∪未校验余量 + 空 partial」
  （partial 此刻必空），**恢复后又在重校验窗内再暂停**会丢上一轮 partial——极边缘
  （重校验窗内连按两次暂停），登记为可接受边界。
- 弹窗末列宽度口径（bug24）：**末列预算 = content_w − 末列起点偏移**，不是独立
  常量——放宽窗口只涨 content_w，末列起点（TRACKERS_COL_MSG=76 等）不动即自动
  变宽；反向陷阱：右移列起点（如 bug20 把消息列推到 76）会**吃掉**末列预算。
  trackers 弹窗 94×22 下 content_w = 88、末列预算 12（容「下轮 NNNNNNs」6 位秒数）。
  定宽 CJK 列布局改动时先算一遍「文案宽 ≤ 末列预算」再收工（下轮 5 列 + 秒数 N 列
  + s 1 列）。

标志列轮（v1.41/D69）：qBt「Flags 列」取证锚点 = 官方 FAQ（Trackers 面板无 Flags 列、
  Peers（用户）面板 Flags 中文列名「标志」——操作者口中的「trackers panel Flags」按 Peers 面板判定，
  先取证再落地防臆造列）；qBt 全集 15 旗标裁剪法 = 只收引擎已具备状态源对应的旗标（P/S/X/H/E/e/L
  无状态源不收，防死旗标列）；判定表直接映射 ConnStat 既有 peer_choking/am_choking/peer_interested
  三字段 + choker 乐观槽 + 入连方向，am_interested 不必新增跟踪（Bitfield/Have want 判定同口径复算）；
  标志极值列宽推演 = 组合互斥分析（D/d 互斥 K 于我方 interest、U/u 互斥 ? 于对方 interest∧choke 配对、
  O 互斥 u 于乐观必 unchoke ⇒ 极值 DUOI 4 枚 + 1 空白 = 列宽 5）；列插入位置 = 地址后第二列
  （状态列首屏可见性优先于 qBt GUI 的列尾位置——TUI 视口窄横向滚动常态化， bury 在 91 列外的状态列不可用；
  附带收益 = 末列「协议」吸附语义零改动）；**收缩基线随列插入平移纪律 = client_w = max(下限, vw_pre − 基线)，
  基线 = 内容宽 − 客户端列宽，插列后基线 + 列宽（60→65）、整表完整可见视口下限同步平移（86→91）——
  改 BT 列集必须同步重推收缩基线与视口下限两数，漏推 = 收缩臂失效假完整**；帮助窗口缩写图例 = 每条 kd 描述
  ≤ 内框余宽 46 显示列（70 宽 − 4 缩进 − 16 键名列 − 4 余量），超宽被 Paragraph 无 wrap 截尾，
  一行放不下拆多条目（8 缩写 → 5 条目）。

- **BT 下载排队时间动态管线（libtorrent update_desired_queue_size 移植）**：现象：固定 64 深
  请求管线在带 RTT/抖动的真实链路上给 seeder 的排队缓冲只有 ~400ms（64×16KiB），线级实测
  qBt 内核 libtorrent 维持 330–460 在飞请求（≈3s 数据量），延迟尖峰期间 ezr 队列先耗尽、
  seeder 断粮、管道空转；处置：`issue_requests` 目标深度 = `clamp(3s × per-conn 速率EMA ÷ 16KiB,
  64, [bt] peer_pipeline)`，速率源 = `ConnStat::speed(&down_window, cum_down)`（conn_views
  周期性翻转窗口），默认上限 64→512（3s×2.5MiB/s=480 与 libtorrent 实测吻合；<
  libtorrent `max_allowed_in_request_queue` 2000 丢弃线，安全）；根因：libtorrent 的
  `request_queue_time=3s` 语义——在飞请求数应正比于下载速率而非固定值，起步阶段速率 0
  自然回落到下限形成事实上的 slow-start。
- **BT 零连接紧急重拨**：现象：握手挂起（双向互拨碰撞/丢包/半开）时 ezr 要吃满
  handshake_timeout 10s + `peer_retry_base << fails` 退避（5s×2^n）才重试，单候选场景一次
  碰撞 = 白等 20s+（基准里首 piece 前 14s）；处置：`fill_peer_slots` 在 `conns.is_empty()`
  时退避基数与上限都降 1s（有连接时维持原节奏防拨号风暴，失败连接本身还有
  connect_timeout 3s 兜底节流）；根因：BT swarm 里双方同时互拨是常态，libtorrent 靠
  并行拨多候选 + 快速重复试掩盖单点挂起，ezr 单候选时没有并行兜底，只能靠快重拨。
- **用户态延迟代理 + BT 帧感知观测法**：现象：沙箱无 netem 权限（`tc qdisc` RTNETLINK
  EPERM），局域网 RTT/抖动场景无法直接复现，限速 seeder 下 ezr 线速已达 cap，瓶颈
  （队列深度、启动死时）在传统速度曲线上不可见；处置：asyncio TCP 代理（监听
  127.0.0.1:17905 → 转发 127.0.0.2:17903，双向 read→sleep→write 注入延迟）+ 帧级 tap
  （68B 握手偏移 + 4B 长度前缀流式解析，按消息 id 统计 request(6)/block(7)/cancel(8)，
  每秒采样 outstanding = req − block − cancel 落 JSONL）+ mini_http_tracker 加
  `--static-peer/--only-static-peers` 把代理地址注入 announce 应答；根因：wire 级帧统计
  能无侵入量化「seeder 侧请求在飞深度」这个速度问题的直接因变量，配合速度曲线把
  差距拆成「稳态 vs 启动」两个可分别归因的成分。

## 速度追查第二轮（2026-11-06，连接去重端点化 + 管线速率源引擎级 EMA）
- **per-IP 连接去重是 libtorrent 语义的误移植（本轮最重要发现）**：现象：bench s1l1
  （1 seeder + 1 并行 lt leecher，同 127.0.0.1）ezr 前 60s 只连到 leecher（标志 K 互瞪），
  seeder 候选被既有连接按裸 IP 挡死，60s 空闲置换才解套，均速 1.16MB/s；处置：候选拨出
  与入连接受都改端点级（ip:port）判定，同一客户端实例的出连+入连双通道交握手后 peer_id
  重复裁决收口；根因：libtorrent `peer_list` 的 allow_multiple_connections_per_ip=false
  真实语义 = peer 表按地址单条目 + add_peer 同地址只更新端口 + new_connection 仅在
  同地址且有活连接时按方向（双向出/双向入拒新、一出一入按端口比较裁决）处置，连接一死
  条目即刻可复用——从不因「同 IP 已有连接」拒绝不同端口的 peer；ezr 的 per-IP 规则把
  「防双连」错做成了「防同 IP 一切连接」，且以 60s 空闲置换为唯一逃生口。
- **管线速率源曾被 UI 快照窗口污染**：现象：动态管线目标（3s×速率）在真实突发-间隙
  流量下于 64↔512 间振荡，低目标期间 `issue_requests` 直接拒绝补管线（inflight ≥ target
  即 return），seeder 断粮空转——夹具回环/代理基准因流量平滑测不出；处置：速率源改
  ConnStat 独立字段（pipe_ema/pipe_sample），仅引擎 1s tick 更新，UI 的 conn_views 继续
  翻转 down_window 但不再影响管线；根因：`conn_views`（250ms 进度节流 + 1s tick + 事件
  路径多处调用）每调用即重置 down_window，issue_requests 逐块读取的「速率」实为距上次
  UI 快照的瞬时值，突发后第一个块把速率估成 ~0，目标塌向下限并拒绝补管线，形成
  「浅管线→间隙→更低估计」的自锁；libtorrent 对照 = stat_channel 整数 EMA
  （avg×4/5 + sample/5，每秒一次）喂 update_desired_queue_size，与 UI 完全解耦。
- **libtorrent 请求队列全参数口径（移植对齐基准）**：desired_queue_size 成员初始化 4；
  slow-start 期间每收一块 +1 且 EMA 不覆盖，退出口径 = 未被 choke ∧ 上一秒有进账 ∧
  上一秒+5KiB ≥ 本秒进账（速率平台）；非 slow-start 每秒重算 3s×EMA÷block，钳
  [min_request_queue=2, max_out_request_queue=500]；snub（块超时）把 desired 压 1；
  发请求守卫 = download_queue ≥ desired 即停（低目标同样阻断补管线，故速率源质量是
  生死线）；seeder 侧 max_allowed_in_request_queue=2000 之外，在飞请求超
  max_out_request_queue(500) 会被对端丢弃——ezr peer_pipeline=512 略高于 500，注意。
- **seeder 端 choker 不构成 ezr 劣势（反吸血疑云排除）**：choker.cpp 三算法
  （round_robin/fastest_upload/anti_leech，qBt 默认 round_robin）首键都是
  downloaded_in_last_round（纯 seeder 恒 0）、次键 upload 优先级；round_robin 配额
  = piece_length×seeding_piece_quota(20) 且 unchoke ≥1 分钟才轮替，多 leecher 公平
  分摊——差距只能来自连接生命周期与请求管线，不在 choke 策略。
- **第三方同 IP 纠缠不可修只可绕**：lt leecher 在 ezr 拨入时按端口比较裁决误杀自己的
  seeder 连接（libtorrent 自身在多客户端同 IP 拓扑下的已知纠缠），修复后 ezr 的残余
  14s 启动段 = seeder 对第二个同 IP 入连的 duplicate_peer_id 拒绝 + 一次 5s<<1 退避，
  属第三方行为；真实局域网（各机独立 IP）无此段。

## bug25 轮：面板速度单位二进制化 + 并发列宽改版联动 + hscroll 宽字边界 artifact
- **现象/处置**：①面板层活速显示（头部全局/峰值、任务行 ↓/↑、并发速度列）三面板同源
  `ui/text.rs::fmt_speed` 单点十进制（KB 1000/MB 1e6），与对话框层 fmt_speed_limit
  （KiB/MiB/GiB 二进制）及 config 键新写法口径割裂——单点改判四档 1024 进位即三面板
  全量生效，显示精度阶梯沿用尺寸类 fmt_size 惯例（KB {:.0}/MB {:.1}/GB {:.2} →
  KiB {:.0}/MiB {:.1}/GiB {:.2}），属性测试后缀封闭域同轮翻转；尺寸类 fmt_size/
  fmt_size_pair 与限速输入层 kb/mb 二进制速记不在速度显示范围、不触碰。
  ②并发面板列宽改版（速度 8→12/累计 8→10，内容宽 99→111、HTTP 22→28）必须四点联动：
  列宽表、内容宽常量、客户端收缩基线（65→77，= 内容宽 − 客户端列 34）、整表可见视口
  阈值（≥91→≥103、≥99→≥111）——沿标志列轮「基线随列插入平移」纪律第二次实证，漏推
  即收缩臂失效；速度/累计四列表头右对齐 = head_cell_r（pad_left）仅四处调用点，
  数据单元格对齐零改动。
  ③ratatui `Paragraph::scroll((0, hscroll))` 左边界宽字截断 artifact：右端吸附带外
  （edge == content_w，如 hs=59/内容宽 103）仍可现末列半字（左切断宽字被丢弃整体
  左移一位）——吸附测试取值必须 0..=max 探针实测校准，不能纯公式推演；带内
  （右缘 ∈ (内容宽−4, 内容宽)）吸附语义不受影响。
- **根因**：显示单位口径分层（面板活速 vs 对话框输入 vs config 键）历史割裂；列宽
  派生量（内容宽/收缩基线/视口阈值）是列宽表的隐式函数；ratatui 逐 cell 跳过在宽字
  中缝的可见窗收缩一格。

## cleaner 轮（2026-11-05）：App.windows 死状态裁决与判重口径单点化
- **只写不读字段的识别与手术**：`App.windows: HashMap<u32, SpeedWindow>` 全部引用 =
  insert/get_mut+push/remove/contains_key（测试），零 get/iter/values 消费者——数据
  只进不出。语义换代注释即废弃信号（"BT 任务速度不走进度窗口：任务速度与并发 panel
  行同源"，BT 分支已排除喂窗；HTTP 的 t.speed 亦另源）。手术 = 删字段+init+5 文件
  喂窗/清窗点（bt.rs on_progress 整块、evt.rs Progress/PausedDone/Failed/DownloadDone/
  Invalidated/ghost 臂、tasks.rs 4 处、dialogs.rs remove_task）+ SpeedWindow/WINDOW
  import 按残余使用保留（conn_windows 活跃）。识别判据：rg 全引用按读写分类，写面
  覆盖读面缺失即死状态；"失去读者"类 cleaner 候选先查 UI 消费链再裁决。
- **判重口径与目录口径耦合单点化**：HTTP/BT 两条添加确认链的"空输入回退默认目录 +
  normalize"重复（2 处 9 行）且判重比较（t.save_dir == dir）直接消费该解析结果——
  提取 `App::resolve_confirm_dir` 单点化，防判重口径与目录口径漂移。jscpd min-tokens
  40 扫 7 变更面文件 68 clones（10.48%），bug25 区间相关 19 个：生产代码 2 处收敛、
  既有测试 fixture 群（追加纪律不动）、新增测试小段预置保持可读性，逐个裁决登记。

architect 轮（2026-11-06）：arch_check.sh 规则 14 白名单漂移是「响亮失败」设计而非误报——后续轮新增面板（命令窗 cmd_sb/cmd_cursor_screen、trackers 弹窗 trackers_scroll/trackers_sb、BT 对话框 dlg_bt_sb/dlg_trackers_sb/dlg_trackers_h_sb）的渲染回填字段未登记白名单即红；修复路径 = 逐一核实消费点确证同类回填模式（SbKey 映射/鼠标命中测试/ui::draw 光标绘制消费/键盘写+渲染钳制双写）后**显式扩白名单 + 注释登记判例**，不做字段重命名迁就检查器；白名单扩展必须配阳性对照（注入 app.selected 写 → 红还原 → 绿）。engine↔bt 模块级双向类型引用（bt/engine.rs use crate::engine + engine/mod.rs use crate::bt）= 既定协调枢纽模式而非依赖环：EngineShared/BtTaskCmd/Evt/BtTunables/BtConnView/TorrentMeta 是契约类型，Rust 可见性已强制窄接口（bt 只能见 engine 的 pub 门面，impl 子模块 pub(super) 对 crate 树外不可见）——拆分属「依赖共享可变状态的长分派拆分」不安全类，登记移交 hardender 变异暴露后针对性拆。PieceMap 写盘计划的真不变量是**全局守恒**（Σ 覆盖 piece 的计划字节 == selected_total）：逐 piece 守恒与「未选文件零区间」都是假不变量——未选文件贡献 piece 网格（num_pieces 按 total_all 含未选算）但无计划条目、piece 与选中区边界相交时计划只含交集字节、pieces_of_file 按 size>0 返回 span 与 selected 无关（selected=false size>0 返回真实区间，仅 size==0 返回 (0,0)）。

## v1.45+v1.46 路径基孤儿接续：resume_orphan 标记生命周期与双臂分叉落点
- **实现口径**：FR-01-26 接续判定的占用守卫（v1.45）= 任务列表内任何未完成任务（state != Completed）同保存目录同目标名即占用，先于 sidecar 匹配评估（`resumed = !occupied && sidecar 存在`）；对话框与 CLI 添加同口径。同 URL+同目录添加仍由重复判定错误窗口（01-add-task-16）先行拦截，不经接续臂。
- **判别载体落点**：Task 新增 `resume_orphan` 标记——添加时路径基匹配成功即置位（=孤儿候选、未探测确认）；**探测相符（Evt::Probed）即清位收编**（此后一致性失效按自有臂 07 原地作废）；**探测不符（Flow::OrphanRedirect）即消费改道**。标记随注册表 serde(default) 持久化——探测前崩溃重启后仍按孤儿臂裁决，孤儿数据不丢。
- **改道点 = 获槽 + 探测完成时**：supervisor 一致性不符按 `spec.resume_orphan` 分流 `Flow::OrphanRedirect`（不删 sidecar）vs `Flow::Invalidated`（删 sidecar，自有臂 07 口径不变）；App 收 `Evt::OrphanRedirect` 后改名重排：**去重谓词含自身**（改道语义=无条件让出当前名，链起 `<名>.1`）+ exists_on_disk（后缀碰撞不覆盖既有文件），download.rs 单流台账/文件不符点同分流保孤儿零动作。
- **根因**：孤儿 sidecar（原任务已删）与自有 sidecar（暂停/失败/重启续传）在磁盘路径形态完全一致（`{dir}/{name}.ezr`），且 alloc_task_id 随机分配可能撞上孤儿 sidecar 内嵌的旧 task.id——唯一可靠判别 = 添加时点的语义标记而非运行时探测；arm 选择必须在 supervisor 内完成（它掌握 sidecar 文件删除权），App 只负责改名重排与静默。
- **bug26 轮**：任务创建默认值的接线不变式——「config 键 = 新建任务默认值」类需求必须覆盖**全部**任务创建路径（HTTP 添加对话框 / BT 添加对话框 / CLI add_cli_task / 命令窗口分流），任一路径漏接 = 该入口下功能整体失效且对话框路径测试全绿掩盖（本轮 CLI 路径漏接任务级限速默认，致 CLI 添加的 HTTP 任务恒不限速）。限速生效链复查基线：引擎双桶（全局+任务）串联取币、Reconfigure 热调、注册表往返四条链路均有测试锚点后，新入口只需验证「值落位」。

**bug27 轮（2026-11-07，coder）**：任务唯一性判重谓词单点语义——「同 URL 即任务身份」（操作者钦定），与保存目录解耦；旧口径「URL+保存目录」源自 bug13 轮归一判重的措辞惯性，属口径漂移而非实现缺陷（三入口 HTTP 对话框/BT 对话框/CLI 的判重状态：前两者谓词多了一个 save_dir 条件、CLI 根本无判重）。修复不变式：判重谓词改动必须 rg 全部任务创建入口（dlg_finish_add / dlg_confirm_add_bt / add_cli_task）同步落地，CLI 入口无对话框载体、提示走 toast（⚠ 前缀）——载体差异不构成豁免判重的理由。判重与 FR-01-26 接续守卫是两层独立拦截（判重按 URL 先行、守卫按目标路径后置）：同 URL 异目录在判重层即被拒，不会流到接续链；异 URL 同名才进守卫/序号臂——两层谓词不同维度，合并不得混淆。残留影响面核查：判重收紧只减不加（更多输入被拒），-name dedupe（异 URL 同推导名 .1 递增）与断点接续（注册表无同 URL 时 sidecar 路径基匹配）链路不受影响，restore/孤儿接续场景实测全绿。

## bug27 清理轮：添加确认链三单点化（判重谓词 / 任务名推导 / 任务注册尾部）
- cleaner 轮把 bug27 引入的三入口判重谓词复制（HTTP/BT/CLI 各一份 `tasks.iter().any(t.url == url)`）
  收敛为 `App::url_task_exists`（dialogs.rs，与 resolve_confirm_dir 同族同位）；dialogs/cli 的
  occupied→sidecar→dedupe 15 行名推导双胞胎收敛为 `App::derive_add_task_name(&url, &dir) -> (String, bool)`；
  HTTP 注册尾部（push+关窗+filter 归零+选中末行）提取 `App::register_new_task` 后 BT 入口第三处
  手写尾部一并复用。延续 resolve_confirm_dir 的「口径单点化」先例：唯一性判重的语义演进点
  （bug27 同 URL 即身份）从此只有一处实现面。
- **BT 入口内联序号循环不并入 namegen::dedupe**（裁决保留）：内联 while 无 dedupe 的 >10_000
  时间戳回退臂，病态边界行为严格不同——保持行为纪律下不并网；如未来统一需先改 dedupe 回退
  语义或内联臂补回退（属行为变更，移交裁决）。
- 对话框入口链现状：validate_add_url（空/协议守卫）→ resolve_confirm_dir（目录口径）→
  url_task_exists（唯一性）→ derive_add_task_name（占用/接续/去重）→ conns_from_input（并发钳制，
  HTTP 拒绝式与 BT 钳制式语义不同各自保留）→ Task 构造 → register_new_task（注册尾部）→
  announce_*（载体分众 toast/错误窗口留在入口）。

## architect 轮（2026-11-07）：交互层分派复杂度定性、bt 层护栏收口与神结构拆分裁决
- 四阶段评审零违规：分层基线实测合规（main→app→engine→model；ui→app/model；app→bt 仅
  TorrentMeta 纯解析消费；engine↔bt 协调枢纽沿 2026-11-06 裁决）；app 层 ratatui 触面仅
  `layout::Rect` 几何类型（ui 回填既定架构，规则 14 白名单守写侧）；核心无 UI/IO 可测性有
  appcore 747 / engine 743 双套件实证。
- **高 comp 函数定性法：先辨「本质路由复杂度」再决定拆不拆**。焦点路由分派
  （add_bt_dialog_char 13 臂 / modify_bt_dialog_char 10）与多臂守卫链（parse_speed_limit 9、
  overlay_open 8 臂 || 链）的 comp 来自路由决策本身，各臂已委托单一职责助手
  （insert_conns_digit/insert_limit/bt_list_char/dlg_activate_*）——再拆即「长分派拆分
  （参数列表膨胀、状态跨模块漂移）」的不安全类；共享 &mut App 的工作树 + hardender 关闭期
  裁决**不拆**，移交登记。判据：把函数改成 match 后臂数不变 ≈ 本质复杂度；臂体超过一屏
  或混杂无关行为才属可拆。
- **App 神结构（~140 字段）拆分裁决：不拆**。>100 变异点三文件（app/mod 142 / dialogs 101 /
  bt_dialog 100）点数与行数 ≈0.06–0.08 点/行属交互层常态；拆分收益（hardender `--file`
  分块跑变异）无消费方；字段聚类（几何回填/滚动/弹窗开关/连接统计/引擎通道）清晰且模块
  边界由规则 12/13/14 守护——拆子结构触及全模块签名，风险高于收益。
- arch_check.sh 16→17 规则：规则 16（机制层禁渲染框架）扫描域扩至 src/bt（原扫描域漏盖
  bt 层——评审实测 bt 零渲染导入但无规则固化）；新增规则 17（bt 层禁 app/ui use）——
  bt 分层箭头白名单 = bt→model/engine、app→bt 纯解析、engine↔bt 枢纽。两条阳性对照实测
  变红后移除复绿（尾注登记）。
- Dialog 测试字面量克隆家族（179/165/142tok ×8 处）收编 `testutil::dialog` 工厂（测试基建
  微调，零行为变更）；判重错误窗口测试对提取 `dup_hit_app` 共享 setup（差分点=对话框目录
  参数化，两测断言意图文案保留）。PMD CPD 全仓 200→181。;

ws/webrtc 轮（2026-11-09）：TrackerUrl 三族分型（Ws 变体与 Http/Udp 同级、parse 白名单五族大小写不敏感、udp 仍走 host:port 校验）+ announce_ws JSON 文本帧契约（info_hash/peer_id 小写 hex——原始 20 字节非 UTF8 不可直入 JSON；response peers 带 peer_id 供信令定向）+ WebRTC DataChannel 流适配器（RTCDataChannel→AsyncRead/AsyncWrite：读=on_message 推 VecDeque<(chunk,offset)>+reader waker，写=后台发送任务消化有界 mpsc（webrtc-rs send 是 async 不能在 poll_write 直调）+try_reserve 满时 overflow waker 由发送任务逐帧唤醒）+ 信令设计三决策（①offer 角色按 peer_id 字节序确定分裂防 glare——只对「大于自身」的 swarm peer 发 offer；②offer 定向 to_peer_id 由 tracker 中继到该 peer 下次 announce 响应，offer_id 去重；③AnnounceCtx 携 signals Arc<SignalState> 任务级共享跨 tracker 去重 busy）+ webrtc-rs 0.14 API 五坑（send/close/local_description 均 async；on_data_channel 收到的 channel 已 open 不触发 on_open——发起侧 on_open/应答侧直接用；on_open 是 FnOnce 只能捕获 stream 一次；RTCPeerConnection 在 peer_connection 模块根、RTCIceGatheringState 在 ice_transport::ice_gathering_state；ClientConfig 加危险验证器必须 builder().dangerous().with_custom_certificate_verifier 链而非 with_dangerous_verifier）+ wss 自签信任口径（QA-BE-20 明示 harness 自签——accept-any ServerCertVerifier 四方法全 assertion+provider supported_verify_schemes）+ std MutexGuard 过 await 的 Send 假阳性规避（if/else 内 drop(guard) 后 await 仍报 held——锁临界区整体收进 { } 块让 guard 在 await 前作用域结束，布尔结果外提）+ WS announce 无代理面登记（spec 各按自身协议，ws 臂 proxy 未接 reqwest 等价物=遗留受限；操作者 20261109 裁决不补齐，边界关闭）

- **真实 WebTorrent ws tracker 契约三则（20261109 与 webtorrent-cli 6.0.1 实测定案）**：①**响应必须回带 `action:'announce'` 与 `info_hash`**——官方 bittorrent-tracker 客户端 `_onSocketData` 按 `data.action` 分派，缺省判 `invalid action` 走 `_onSocketError` 销毁全部 pending offers 与 socket；补 action 后 `_onAnnounceResponse` 对 `data.info_hash` 做 `bin2hex`，undefined 直接 TypeError 进程崩溃。ezr 自身解析按 interval/顶层 offer/answer 宽容分派故未暴露。②**offer/answer 中继是服务器主动顶层推送**（独立 WS 消息 `{action:'announce', info_hash, peer_id:<from>, offer|answer:{type,sdp}, offer_id}`），响应内捎带数组（offers_to_answer/answers）是 ezr/harness 扩展契约官方客户端不认——mini_ws_tracker 双投递（推送+捎带）幂等兼容两侧。③**id 编码线上形态是 20 字符 latin-1 二进制串**（JSON 序列化时控制字符转义、≥0x80 走 UTF-8 多字节），40 hex 为 harness 旧契约——`bin_string`/`id20` 双形态兼容（tracker.rs）。定位方法论：TCP 记录代理抓双向帧 + Node ESM 依赖打桩须直接 patch node_modules 文件（`--require` preload 对 ESM 不生效），streamx `updateNonPrimary→_destroy` 栈即销毁源；`code=1005`（无 close frame FIN）不等于网络异常，应用层 destroy 亦可呈现。沙箱 NAT 对称型（srflx 端口≠rport）下与公网浏览器 peer 的 ICE 打不通属环境限制，同机受控实验即可证数据面（dc-open→rtc-ready→bt-hs-ok→分片）。

## 文件勾选修订轮（2026-11-09，coder，FR-02-03/19 v1.16 + D73 落地）

- **Reconfigure 文件面四件套联动**（本轮四缺口收敛后的机制图谱）：①删除单一实现路径 = `model::bt::delete_deselected_files`（App 非运行态确认与引擎 Reconfigure 共用一函数——文件粒度 remove_file + 祖先链自底向上 remove_dir 非空即停 + NotFound/IO 失败双静默，D73④⑤⑦ 一处收口；路径基 = metainfo 解析层已含 name 首段，join save_dir 即 v1.42 布局）；②句柄先关后删（files_handles 按索引 remove——不关句柄则 Linux unlink 后陈旧句柄写孤儿 inode，勾回重下文件永不落盘，句柄关闭的「资源泄漏」表象下藏着数据正确性炸弹）；③**done 集合跨 Reconfigure 持久**：落盘删除后必须对「selected 且磁盘文件缺失」文件覆盖的 piece 做 done/partial 失效（按文件字节区间→piece 区间推导；跨界 piece 整块重下与「整 piece 下载」既有语义同构）——否则引擎视已完成、勾回补下永不发生（本轮集成测试 20s 超时的真根因，失效后 downloaded 即刻回落是 App/引擎两侧对账的关键信号）；④PieceOrderCache.avail 是 Bitfield/Have 事件流的累计值，重建 pmap 时**不得** `PieceOrderCache::new()`（avail 清空后无事件源、rebuild 出空 order → 泵空转），必须从既有连接 `st.has` ∩ 新 pmap 覆盖集全量重建 avail + `dirty=true`——新覆盖 piece 的 availability 只存在于连接握手期的 bitfield 里，事件流不会重放。
- **请求泵是反应式的**：issue_requests 仅由 Have/Unchoke/Block/在飞超时清扫驱动，无周期调度器；Reconfigure 后既有连接 inflight 为空且 seeder 不再发事件 → 泵永不自启（老语义无「勾回补下」缺口故从未暴露）。files_changed 分支末尾对全部连接主动各泵一次（`if !seeding` 门——做种态无缺口无需泵）。
- **上传暂停（D73②）单点闸门** = `handle_peer_msg` Request 臂 `allowed = !am_choking && !upload_paused`（闸门静默忽略 request：cum_up 不增长 → 并发行速度/uploaded/tracker announce uploaded 三观察面全零，无需动 choker）；语义生命周期 = BtSpec 启动注入（跨暂停/恢复/**应用重启**，载体 `BtTaskData.upload_paused` serde default 随注册表持久化，字段级兼容旧档）+ Reconfigure 回退臂置位（seeding && 重算有缺口）+ 双完成臂（block 校验路径 + tick 路径各一处）清除。正常下载互惠上传（D58）与做种态上传零影响。
- **App 层做种回退（D73①）判据与载体**：确认时 `selected_bytes_of_pieces(done_blocks) < new_total`（piece 记账重算，与引擎 recompute_downloaded 同源同值——跨界 piece 的已收字节计入）→ 状态即时翻「下载中」不等进度帧 + downloaded=recalc + 置 upload_paused；纯删减重算仍 100% → 保持「做种中」（场景 11 与回退臂互斥的判据分界）。`on_bt_done` 无条件重置 seed_left（D73③ 原剩余值作废；原 `if <= 0` 条件让做种期旧剩余值穿越重完成存活）+ 清 upload_paused + toast 照常。

## 做种倒计时补缺口轮（2026-11-10，coder）追加

- **「赋值≠倒计时」缺口模式**：seed_duration_secs（配置总时长）与 seed_left（任务剩余量）的衔接原只有完成时一次性赋值与 UI 显示两环——规格「自 <seed_secs> 起**倒计时**」的后半句（递减与归零）全仓无实现，字段是死数。配置键已实现极易造成「功能已存在」误判：缺口审计须按规格逐 When/Then 对账，按键名 grep 只能证明赋值点存在。
- **递减载体归 App tick 层**：seed_left 是 App 模型显示/持久化字段，`tick_seed_countdown(dt)` 与 tick_retry_countdown 同款口径（逐帧 dt 累计、仅 Seeding 态、归零清底后转臂）。不进 BT 引擎：引擎只认 Cmd::StopSeed，归零的 tracker stopped / 上传连接清空 / 引擎退出全复用既有停止臂，App 侧零新增引擎协议面。
- **归零臂与空格停止臂共用 finish_seeding 单点**（tasks.rs 提取，pub(super)）：转「已完成」+ upload_speed 归零 + 释放槽位 + seed_left 清底 + spawn 发 StopSeed + 即时 save_registry；入口差异仅 toast 文案（`✓ 做种结束: <name>` vs `⏸ 已停止做种（进入已完成）: <name>`）。空格臂原内联实现随即退役——双臂单点 = 行为漂移免疫。

## cleaner 收敛轮（2026-11-10）追加

- **CRAP 收敛的两类拆分落点**（comp=语法决策点+1、cov=1 时 CRAP=comp，门槛 ≤6）：①纯函数提纯——delete_deselected_files 提 `push_ancestor_dirs`（祖先目录收集）、BtTaskData::selected_bytes_of_pieces 提 `span_sum_over_selected_files`（单 piece×选中文件交叠求和），主函数保留数据编排、子函数单一职责，comp 8→6 与 8→4/5；②拆不动的登记而非硬凑——on_bt_done（事件臂守卫链 comp=7）、handle_peer_msg/Reconfigure 臂（分发核心）、dlg_confirm_modify_bt（对话框路由）按 architect 定性法（路由/守卫链=本质复杂度）登记口径。
- **三处 DRY 单点化**：①测试基建收编规范位——tick.rs 两个兄弟测试模块的 recv_cmd/assert_no_cmd 重复对收编 `crate::app::testutil`（pub(crate)，cfg(test) 门控，与 make_app_in 工厂同位）；②NotFound 容忍删除单点化——BtSidecar::remove 与 BT 侧 sidecar::remove 同体实现收编 `model::remove_missing_ok`；③引擎取消命令块单点化——暂停/失败两臂的 bt_stopping 登记 + spawn CancelBt/Cancel 分派收编 `App::request_engine_cancel`（is_bt 布尔参，入口差异留在调用侧）。

## 架构评审轮（2026-11-10，architect）追加

- **8477 行单文件目录化拆分的「私有子模块 + glob 收编」模式**：bt/engine.rs → bt/engine/{mod 6260/pieceio 754/trackpool 738/peerwire 468/conn 358}。保行为的关键 = 子模块私有声明（`mod conn;` 无 pub）+ `use conn::*;` 收编 mod.rs 作用域——engine 内既有项的路径与可见性（pub(super)/pub(crate)）零改动，下游 use 面（crate::bt::engine::X）与 12 个集成测试目标零触碰。职责分片法：mod=编排核心与共享类型（6260 行大头是 cfg(test) 测试主体）、conn=连接状态与生命周期、peerwire=wire 协议编解码、pieceio=piece 读写与校验、trackpool=tracker 池。拆分完整性的机械验证 = `cargo mutants --list` 点数对账：engine.rs 465 → 五文件 20/106/78/209/52 总和精确 465、全仓 5994 不变（搬运不增减）；mod.rs 仅 20 点因变异扫描跳过 cfg(test) 测试主体。
- **对账类规则不入 arch_check 的裁决判据（第 18 条=否）**：coder 轮提请「规格逐 When/Then 全量对账固化为 arch_check 第 18 条」裁决否——When/Then ↔ 规则映射是语义级对应（grep 表达不了「该 Then 被此断言覆盖」），硬编码映射表维护面无界（规格每轮修订即漂移），且与 QA 套件的场景问责职责重复；arch_check 定位 = 结构性边界（依赖方向/可见性/禁入）的 grep 可表达面，arch_check.sh 保持 17 规则零改动。
- bt_dialog.rs（119 变异点）不强拆维持登记：可见性耦合 + 阈值边缘，拆分触及全模块签名（rust.md 口径）；engine 拆分后 hardender `--file` 分块消费面就绪（五文件 20–209 点/文件，全部 <250）。

## v1.47 显示指标轮（2026-11-10，specifier）追加
- qBt 三口径源码锚点（新增）：相关度 = qBt PeerInfo::calcRelevance（peerinfo.cpp）——（对端拥有 ∧ 本端缺失 piece）÷ 本端缺失 piece 总数、本端无缺失恒 0（不是「÷ 对端拥有数」）；显示 ×100 一位小数 + %（peerlistwidget.cpp `fromDouble(relevance*100, 1) + '%'`）；种子健康度 = qBt General「Availability」= libtorrent `torrent_status.distributed_copies`（piece_picker.cpp distributed_copies：每 piece 持有者计数【非种子 peer 持有记 1 + 本端已有记 1】取最小值 + 连接种子数 m_seeds + 超出最小值的 piece 数 ÷ 总 piece 数；qBt 显示 fromDouble(...,3) 三位小数、且 finished torrent 隐藏该行——ezr 因 <s>/<p> 项两态共用选择做种中照常显示，D74⑥ 登记可回退）；分享率 = qBt realRatio（torrentimpl.cpp）——uploaded/downloaded、下载量 < 1% total_done 按 total_done 兜底（防 seeder 丢统计）、分母 0 且上传 >0 显示 ∞（MAX_RATIO → C_INFINITY）、两位小数（propertieswidget.cpp fromDouble(ratio,2)）。
- ezr-demo 同构实现点（新增）：并发 panel 九列 = colw [18,4,12,12,10,10,34,7,8]、客户端收缩基线 81→89（base 123 − 客户端 34）、相关度单元格 pad_left 右对齐 + `{:.1}%`；列表行健康度三段 `{}/{}/{:.3}`（dash 臂 `-/-/-`）与分享项 `share_spans`（标签「 分享」DIM + 数字 MAGENTA、单空格衔接尺寸项=操作者钦定示例字面口径）；数据面 `Connection.relevance`（pseudo_relevance 确定性哈希 0..1、done≥total 臂恒 0）与 `Task.availability`（demo 钦定值 2.418/1.675 + 对话框建单伪随机 0.350–2.749）——单构造点（make_chunk_conns_lease/mk）注入即全路径覆盖，是 demo 数据面加字段的最小改动位。

## ratatui Paragraph 水平滚动遇 CJK 表头跨列截断会整行错位，宽字表格水平窗口必须自行切片（v1.48 轮）
- **现象**：并发 panel「协议」列数据值在特定滚动偏移下越过表头左边界 1 列（操作者屏面实证；
  PTY 复现条件 = 终端宽度使 hmax = 内容宽 − 视口宽 + 1 恰落在表头 CJK 字符跨列位 p+1，
  如 150 列终端 hmax=59 切入「累计下载」末字「载」右半格、160 列同型）。
- **处置**：不再用 `Paragraph::new(lines).scroll((0, hscroll))` 的逐行独立截断，改为按显示
  宽度自行切片渲染水平窗口（`ui::slice_spans` + `slice_by_width`）：按单元格累计显示宽走查，
  完全在窗口左侧的单元格丢弃、跨界单元格按显示宽精确切片（跨界 CJK 以可见余量**前导空格**
  占位、不渲染半字）、其余原样保留，行渲染不再带 scroll 偏移。
- **根因**：ratatui 0.29 `LineTruncator` 对 horizontal_offset 逐 grapheme 消耗显示宽，遇
  `w > offset` 的宽字调 `trim_offset`——宽字不可切分只能整字返回，该行其余内容整体右移
  1 列；数据行纯 ASCII 不触发该臂、不位移，于是表头（含 CJK）与数据在特定偏移下错位。
  陷阱细节：切片时跨界宽字的可见余量必须占位在**原字符位置**（前导空格），若放行尾补齐
  （drop 后 pad-after），同 span 内跨界字后有后继字符时后继内容仍 +1 错位——仅在「跨界字
  恰为 span 末字」时两种写法等价，单点验证会漏。
- **自查口径**：含 CJK 表头的可水平滚动表格，「表头/数据任意偏移同列」必须作为不变式
  守卫（对每个 cut 断言两行单元格起点恒等），且跨界位覆盖须含「跨界字非 span 末字」的样本。

## v1.49 全类型扩面轮（2026-11-11，specifier）
- **作用面扩判的代码落点形态**：任务行标题尺寸项从「协议×状态白名单」（上一轮
  `bt_title_size_visible`：BT 且状态 ∈ 下载中/校验中/后期处理/已暂停）收为单点
  `title_size_txt(t)`（`size_known()` 门控 + 前导空格承载间隔）——作用面扩判时优先
  收单点谓词而非扩散 if 分支，协议/状态枚举再增员也不需要回改；尺寸未知（不可续传
  HTTP 且未入后期）返回空串即「不伪造 0 B」。
- **明细行瘦身成对删除纪律**：删除尾缀数据项必须连同其前置分隔符（`" · "` /
  `"  "`）同删，遗留孤立分隔符会让明细行以悬挂 ` · ` 开头（等待中队列位 0 且无槽位
  提示时 l3 只有分隔符）；四个臂（Failed/Queued/Completed/HTTP fallback）各删一处，
  做种中「已上传 up/total」对不在迁移范围（操作者口径：迁移对象 = 已下载/总字节对）。
- **切页签残留 selection 越界 + scroll 搁浅根因**：`cycle_filter` 只切 filter 不夹紧
  `selected`（另一页签长度不同），`scroll_to_selection` 用旧 selected 把 scroll 推到
  `selected+1-vis`；切到短页签后 `max_items = min(rows, flen - scroll)` 只渲染尾部
  残行（PTY 实测：11 项选中尾项切 4 项页签仅剩 1 行）。修法两层：`cycle_filter`
  夹紧 selected 到新页签范围（光标与动作面立即正确）+ `draw_list` 按视口容量自愈
  夹紧 scroll = len−rows、selected 越界夹紧——自愈点放 draw 层可覆盖模拟状态迁移/
  清理任务等一切集合收缩源，不限于切页签；`scroll_to_selection` 需 `pub(crate)`
  才能从 ui 层调用（可见性是接线第一坑）。
- **窄面板贴合预防**：名称预算公式多预留 1 列（`pct_w + 2`），否则预算恰好的长名行
  `pad = 0`、尺寸与百分比贴合（`96.4 MB100.0%`）；该形态此前就存在于徽标贴合
  百分比，尺寸项加入后视觉恶化显著，属「预留量随行内新增元素同步」纪律。

## v1.50 做种已上传对退役轮（2026-11-11，specifier）
- **删除类修订的「保留判据」翻转链**：历史上明文登记「保留/不在迁移范围」的显示项被后续
  指令退役时，三层判据须同步翻转——守卫测试（断言极性反转 + 测试名 keeps→drops，否则旧守卫
  反向拦截正确实现）、PTY 脚本（该项自「保留项完好」清单移入缺席判据）、规格/QA 做种臂
  （形态串重写 + 历史注记保留退役痕）；本轮 v1.49 的「做种已上传对保留」口径即被 v1.50
  指令退役，v1.49 节该句为历史记录不再回改。
- **动态值缺席判据的形态选择**：fmt_size_pair 产物为无空格 `x/y 单位` 形（`1.71/4.32 GB`），
  与详情面板带空格 `x / y` 形天然可区分；做种上传量随模拟每帧增长（全字面断言必漏），
  缺席判据用「标签缺席（已上传）+ 形态正则 `\d+\.\d+/\d+\.\d+ (GB|MB)`」双判据任意帧稳定命中。
- **孤儿辅助函数随唯一调用点退役**：fmt_size_pair 唯一非测试调用点删除后立即移入 tests 模块
  （pub(crate) fn 留在主代码 cargo build 即报 dead_code 警告，破坏零警告验收线）；
  产品端 ezr task_lines.rs 同名函数归下一 coder 轮同处置。
- **衔接对象改接的同构校验**：分享率项改接健康度项后做种臂与 BT 下载臂 span 序列完全同构
  （share_spans 首跨度自带单空格「 分享」），衔接格式零新增逻辑——改接类修订优先复用既有
  臂的 span 构造顺序而非新写衔接串。

## v1.51 页签状态快照还原轮（2026-11-11，specifier）
- **共享 selected/scroll 是页签状态漂移根因**：两页签共用一对 selected/scroll，切走再切回
  即漂移；修法 = 每页签独立 TabListState 快照（selected_task id + selected + scroll），
  切走时存、切回时还原——快照存「任务身份」而非仅索引：页签内集合位移/收缩（任务完成
  移页签、新增插队）时按 id 重新定位选回原任务，纯索引还原会选中错位邻居（操作者报告
  的缺陷形态正是索引漂移）。
- **还原后必须抑制 tick 视口跟随**：set_filter 还原 (selected, scroll) 后须同步
  `last_selected = selected`——sim tick 守卫「last_selected ≠ selected → scroll_to_selection」
  会把还原的 scroll 吸附回选中窗口，滚轮「只移视口不动选中」留下的选中离屏态被破坏
  （这是「滚动条不在原先位置」的第二根因，不抑制则快照还原只对选中可见项成立）。
- **三条 filter 变更路径收口单入口**：键盘 ]/[（cycle_filter）、鼠标点击页签（原直改
  self.filter + scroll_to_selection）、添加任务跳回（原直改 filter=0）三处同走 set_filter——
  漏一路则该路径行为分叉（如鼠标切页签不还原）；cycle_filter 退化为 set_filter(next)。
- **还原语义与自愈夹紧的分层**：还原 = 忠实恢复快照（含选中离屏态）；夹紧 = 越界兜底
  （selected ≥ len 夹紧 + scroll 越界夹紧，tick 守卫与 draw_list 双层保留）——「尽可能还原」
  的落点是快照优先、夹紧只兜集合收缩/视口变化，不把夹紧当切换语义本体。

## v1.52 页签快照还原产品实现轮（2026-11-11，coder）
- **同构移植的守卫几何不可照抄参考实现**：demo 渲染守卫 110×34 在 demo ITEM_HEIGHT=2 下
  rows=16 ≥ flen'=9 成立，直搬产品（ITEM_HEIGHT=4 → rows=8 < flen'=9）会令「全列表渲染」
  断言在正确实现下也红（行 0 恒滚出视口）——守卫断言依赖视口容量时，容量必须从**被测端**
  常量重推（产品 110×48 → inner 46/4=11 ≥ flen'），不从参考端复制面积。
- **三路径收口的完成判据 = 写点普查归零**：rg `\.filter = ` 全 src，生产代码直写 filter
  必须归零（唯一写点 set_filter），测试直设 filter 属合法非产品路径（bt_dialog/tasks 等
  测试夹具不受限）——「主要路径已接」不算收口，残留直写点即行为分叉源。
- **draw_list 自愈与 tick_clamp_selection 双层兜底不可互替**：tick 层用 visible_rows
  （上一帧视口，App::new 默认 6），draw 层用当帧 rows（真容量）——终端改尺寸/首帧前的
  快照还原只有 draw 层能按真容量兜住；draw 层自愈落位在 empty 早退之后、max_items 计算
  之前（先行夹紧保 qpos/渲染循环下标合法），顺序错置 = 越界下标 panic。

## cleaner 收敛轮（2026-11-12）追加
- conn_flags 表驱动化先例（互斥标志对收 comp）：if 链 8 臂（D/d、U/u、O、I、K、? 互斥对 + 单条件旗）→ (bool,bool)→Option<char> match ×4 + then_some ×2 + 数组 into_iter().flatten().collect()——输出字符序不变（D/d,U/u,O,I,K,?）、comp 17→6、CRAP 17→≤6（cov=1.0 下 CRAP=comp）；判据：多臂互斥布尔对映射到固定字符序列，表驱动不损可读性。
- link_break_spans 双胞胎克隆裁决（CPD min200 唯一批内产品家族）：popups.rs/text.rs 两份完全同体 34 行，但各自捕获**语义分歧的本地 fg_shadow**（popups=saturating_sub(1) 防下溢，text.rs=裸 b-1，b=0 角落 debug panic/release 回绕）——「同体克隆」要读齐其传递闭包捕获链再判可否合并，机械单点化=改变另一侧 b=0 角落行为（违反不引入新行为）；已登记 handoff 移交 fg_shadow 语义统一裁决。
- spawn+watch 函数纯函数提准先例：start_bt_task 的 panic 消息提取链（downcast &str→String→回退）抽 join_panic_reason 纯函数（Box<dyn Any + Send> 三载荷直测），本体 comp 降、CRAP 达标；提取前后行为逐字等价（format! 参数化仅形态变化）。
- 变异点扫描本轮口径：cargo mutants --list 按文件计数（--file 多值一次扫全批），批次 10 文件 777 点、>100 文件 4 个（app/mod.rs 149/conn.rs 125/text.rs 122/task.rs 101，密度 0.14-0.30/行）——沿 rust.md §7 登记 hardender --file 分块消费，模块边界裁决权在 architect（conn.rs 本身即上一轮 8477 行拆分的产物，变异点 465 精确再分布的延续账）。
