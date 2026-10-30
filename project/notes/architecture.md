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
