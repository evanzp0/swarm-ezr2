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
