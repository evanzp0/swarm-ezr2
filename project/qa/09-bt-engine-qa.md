# QA 套件 · 09-bt-engine BT 下载与做种引擎（期号 02）

> 对应规格：`project/features/09-bt-engine.feature`（20 场景；v1.38 新增场景 16；v1.39 新增场景 17；v1.40 新增场景 18；v1.41 新增场景 19/20；**v1.43 修订场景 08**）。
> **v1.43 修订（操作者修订轮 20261109「BT 多文件任务修改对话框文件勾选行为修订」；phase-02 v1.16，FR-02-03/19 v1.16，D73）**：QA-BE-08 修订——取消选中 = 已落盘该文件数据删除（文件粒度 + 变空目录自底向上整树清理，4A；目标不存在静默跳过，5A；删除 IO 失败不阻塞确认，D73④）+ 勾回已删文件从头补下（原断点保留语义退役）+ 做种回退臂（做种中补选新文件 → 任务回下载态、上传暂停至再次全部完成（2B）、回做种时 seed_left 重置 config 默认（3B））；UI 层严格检查 = QA-MB-03/10/11/12。
> **v1.42 注记（bug28 轮 20261109，D72；判据未新增，观察面登记）**：QA-BE-19 观察面新增三项可观察行为——①修改任务并发扩容/仍有缺口时全 tracker 提前 re-announce（ws 持久会话立即携新 offers；尊重 tracker min_interval）；②本端主动收口（缩容 shed/空闲置换）的 WebRTC peer 不记失败冷却、下一轮 offer 即可重连（mark_peer_idle）；③并发面板 WebRTC 连接地址列 = 对端 peer_id 前 5 字节 hex 裸值（"WebRTC:" 前缀退役，D71⑤ 改判）。QA-BE-08 关联面：多文件 torrent 落盘布局钦定 = save_dir/<info.name>/…（双层 /a/b/b 缺陷修复，bug28①）；QA-RS-12 关联面：「删除任务和文件」对多文件 BT = 目录树整删（bug28②，01-resume-sidecar v1.47 注）。建议 QA 轮补上述两面 e2e 判据。
> **v1.41 修订（操作者修订轮指令「tracker 统一 + ws(s) 支持 + BT WebRTC + config 严格校验」；phase-02 v1.15，FR-02-21/31/33/08 v1.15、FR-02-34/35 新增、D70）**：QA-BE-05 修订（分型 = http(s)/ws(s)/udp 三族、ws(s) 同权入池）、QA-BE-09 修订（[bt] 统一单键 trackers；旧两键 http_trackers/udp_trackers 退役废弃；白名单外 scheme 启动失败，FR-02-35）、**QA-BE-19 新增（[bt] peer_connection_protocol 全局开关四臂 + WebRTC DataChannel 数据传输 + 无跨协议回退）、QA-BE-20 新增（ws(s) tracker harness e2e 全链路 + peer_connection_protocol 未知值/重复项启动失败）**。
> **v1.40 修订（标志列轮，FR-02-33/D69）**：QA-BE-18 新增（并发 panel 标志列数据语义：qBt Peers Flags
> 子集 D/d/U/u/O/I/K/? 判定表——choke/interest 翻转、乐观槽轮换、入连方向逐项 fixture 驱动；
> 无标志 `-`、连写顺序、HTTP 无标志列；与 QA-UC-08 v1.41 显示形态判据配对）。
> **v1.39 修订（bug23 归档轮，FR-02-22 v1.11 + D68）**：QA-BE-17 新增（partial 持久化与恢复
> 不重下：暂停重启已收块零重请求 / 完整 piece 直接落盘转 done / 篡改自愈 / 重校验窗二次暂停
> 允许边界 D68⑥；与 QA-AL 反吸血联合回归）。
> 需求依据：`project/mission/phase-02.md` FR-02-06…25、D45/D46/D51/D52/D54/D57/D58/D60/D61。
> 协议基线：操作者钦定 BEP 3/15/23/7/12/9/20；tokio 异步（FR-02-10）。
> 上传真实统计（互惠/做种，D45/D58）、BEP 9 做种侧响应（D53）、下载完成转做种（D57）；
> tracker 池三来源（去磁力 tr=，D53）；v1.35（操作者 bug11 轮）：客户端列 全名/版本号、
> 地址列 ip缩写/port、[bt]/[http] 调优键入配置（FR-02-25，QA-BE-14 新增），D51 两字母缩写
> 口径随之改判。
> v1.38 修订（操作者 bug18 轮指令五项功能之一/二/五；FR-02-28/29/32，D67）：QA-BE-16 新增
> （[bt] listen_port：配置生效/未配置随机/占用回退随机+警告/进程级共享单端口/announce port =
> 实际监听端口）；BT 引擎接入任务级下载/上传限速（FR-02-29，速度判据归口 QA-TP-12）与全局限速
> （FR-02-28，归口 QA-TP-02/08）；[bt] 表 20 → 21 键（场景 14 描述含 listen_port 除外键）。
> v1.36（操作者 bug12 轮指令，客户端列识别修正）：QA-BE-06 客户端列改 qBittorrent 源码口径
> 两级识别（FR-02-08 v1.3——扩展握手 `v` 自报名优先 + peer id 解码 = libtorrent identify_client
> 算法与映射表全量移植；BitComet 验收例 1.64；明细客户端列左对齐、列宽 34，见 04-ui-conns
> v1.36），旧 62 项子集表与逐位四段解码退役。
> 端到端 UI 层验证：以用户可达入口（TUI 按键/命令窗口、磁盘产物、并发面板/pieces/详情观察）
> 为主；本地 fixture（tracker/seeder/leecher）与其控制脚本属于暴露给 QA 智能体的用户界面入口
> （SKILL 允许口径），用于驱动「正常/失败/冗余」场景并做字节级对账。

## 环境前置

- 伪终端驱动：按键注入、画面捕获、toast/浮层断言。
- **本地 BT fixture 四件套**（可编程控制，日志可观察）：
  - HTTP tracker（BEP 3：compact=1 / peers6 / interval / failure reason 可控，
    announce 参数含 uploaded/left 可记录）；
  - UDP tracker（BEP 15：connect/announce，可停服模拟超时）；
  - seeder peer（peer wire：bitfield/request/piece；可观察握手/请求/socket 关闭；
    保留原始数据副本供字节对账）；
  - **leecher peer**：可向本端发 interested + request（拉取数据字节供上传
    对账）；可发扩展握手 + ut_metadata request（供做种侧元数据响应用例）。
- 预置种子：多文件（9 文件含子目录、piece length 256 KB）、单文件、损坏种子（非法 bencode）。
- 独立 HOME（干净 `~/.ezr`）；独立保存目录 `/tmp/qadl`（用例间清空复位）。

## 用例

| 用例 | 场景 | 操作（用户可达入口） | 通过判据（可观察状态） |
|---|---|---|---|
| QA-BE-01 | 09-bt-engine-01 | 分别添加多文件/单文件 .torrent 任务；再添加损坏种子 | 任务名 = info.name；详情分块行 `x/y · 256 KB/块`（y = ⌈大小 ÷ piece_len⌉）；tracker 池含 announce-list 全层（fixture 侧同一 infohash 可观察）；损坏种子 toast `⚠ 无法读取种子文件` 无任务创建 |
| QA-BE-02 | 09-bt-engine-02 | 添加仅 HTTP tracker 任务并下载 | fixture 收到 announce（info_hash/peer_id `-EZ2000-` 形态/port/uploaded = 真实累计上传（D58）/downloaded/left/compact=1/event=started）；BEP 23 6 字节紧凑 peer 正确解析建连（并发面板明细出现）；interval 周期重 announce；完成后 event=completed 且不停止轮询（做种继续，D57）；做种/peer 计数展示 = tracker complete/incomplete |
| QA-BE-03 | 09-bt-engine-03 | 添加含 UDP tracker 任务下载；随后停服 UDP tracker | fixture 收到 connect（magic 0x41727101980/action=0）与 announce（action=1）；peers 建连下载推进；停服后该 tracker 协议重试（倍增）判失败、任务经其他 tracker 继续下载 |
| QA-BE-04 | 09-bt-engine-04 | fixture HTTP tracker 返回 peers6（IPv6 紧凑） | IPv6 peer 建连（面板 IP 列 IPv6 地址）、经 IPv6 peer 下载推进（字节落盘对账） |
| QA-BE-05 | 09-bt-engine-05 | 三来源合并任务（config + torrent + 手输，config 默认 tracker 分别取 https:// 与 wss:// 两例行）；停服主 tracker；全部停服 | 池去重（同 URL 单条）、scheme 大小写不敏感归一分型 = http(s)/ws(s)/udp 三族（v1.41；ws(s) 与 http/udp 同权入池，WS:// 与 ws:// 同族）；主 tracker 失败其余接续（下载不中断）；全部失败 → 失败臂退避重试；tracker 列表持久化且修改对话框预填 |
| QA-BE-06 | 09-bt-engine-06 | 下载中观察 fixture 与并发面板；leecher 向本端请求数据 | seeder 观察标准握手（68 字节/protocol string/infohash/peer_id）；interested + request（≤16 KB 块、在飞 5 块）；piece SHA-1 校验、通过广播 have、失败重下；面板各 peer 速度/累计实时、上传两列 = 真实统计（D45/D58：leecher 从本端拉到已完成 piece 块、fixture 侧字节对账）、**客户端 = qBittorrent 源码口径两级识别（v1.36/FR-02-08 v1.3：①对端扩展握手携带 `v` 时显示其原样（如 libtorrent fixture 自报 `libtorrent/<版本号>`、BitComet 自报 `BitComet 1.64`——验收例：qBt 解析 1.64、旧 ezr 0.1.6.4，v1.36 起同源同值，fixture 侧对 v 串可观察对账）；②无 `v` 时 peer id 解码 = libtorrent identify_client 算法与映射表全量移植：fixture 可构造 -AR1001- → Arctic Torrent/1.0.0.1、-BC0164- → BitComet/0.1.6.4（peer id 路径）；映射表未命中 → 码本体；全零/全不匹配 → Unknown；旧 62 项子集表与逐位四段解码退役）**、明细客户端列左对齐（列宽 34）；协议 = TCP；**地址 = ip缩写/port（v1.35：斜杠分隔，如 1.*.4/51400，IPv6 ffff:*:ffff/65535 形态；旧冒号分隔退役）**；头部 ↓/↑ 与峰值真实计入 BT |
| QA-BE-07 | 09-bt-engine-07 | 做种中任务：leecher peer 发扩展握手 + ut_metadata request（合法分片 / 越界分片） | 本端回 extended handshake（m.ut_metadata id + metadata_size）；合法分片回 data（首片带 total_size）；越界分片回 reject；leecher 拼接 SHA-1 = infohash（fixture 侧观察）；完整 BEP 10 与拉取侧属 phase-03 |
| QA-BE-08 | 09-bt-engine-08 | 多文件任务只勾 3 文件下载；完成后补选文件；补选完成后取消已下载文件勾选；重勾回；做种中补选新文件至完成（v1.43） | 只落盘选中文件（未选路径不存在）；边界 piece 整 piece 下载、未选字节不写入；完成态选中文件字节 = seeder 原始字节；补选后缺口补下、已落盘 piece 不重下；**取消勾选 → 该文件从保存目录删除**（变空目录整树清理；不存在静默跳过）、对应 piece 退出缺口与可服务集合、已下载钳制 ≤ 新 total；**重勾回 → 从头补下**（数据已删无断点，v1.43 反转）；**做种中补选新文件 → 任务回下载态且上传暂停**（上传速度 0、uploaded/announce uploaded 不增长、leecher 拉不到数据；D73②）直至再次全部完成；**完成后回做种态、seed_left 重置 config seed_duration_secs 当前值、上传恢复**（D73③） |
| QA-BE-09 | 09-bt-engine-09 | 写入 `[bt]` 表 trackers = [https 与 wss 混排两例行]（v1.41 统一单键）；删配置重启动生成模板；配置白名单外 scheme 行（`["ftp://x/ann"]`）；配置含旧键 http_trackers/udp_trackers；无来源 tracker 任务 + 手输 trackers（含 ws:// 行） | 模板含 trackers 单键注释示例（位于 [bt] 表内，FR-01-85 机制）；白名单外 scheme 行 = 启动失败并报错（v1.41，FR-02-35——原「非法行忽略 + 警告」对 config 来源退役）；旧两键按未知键口径直接废弃（不迁移、不并入池）；任务 tracker 池 = trackers 键合并（去重、三族分型、ws(s) 同权）；手输每行一条并入持久 tracker 列表 |
| QA-BE-10 | 09-bt-engine-10 | 等待任务全部 piece 校验完成 | toast `✓ 下载完成: <name>` 且任务转「做种中」（D45/D57）；列表行切做种中形态 `↑ <速度>  <seeders>/<peers>  已上传 <x>/<总>  剩余做种 <时长>`；产物逐字节 = seeder 原始数据；引擎发 event=completed 且继续 announce（left=0、uploaded 真实） |
| QA-BE-11 | 09-bt-engine-11 | 下载中（及做种中）并发面板选中连接按 B；HTTP 任务按 B | 该连接移除（fixture 观察 socket 关闭、明细行消失）、块回归补完、下载持续不回退（toast ✂ 前缀）；做种中同样可断开（D45 改判）；HTTP 任务 toast 拒绝（文案 = phase-01 D26 维持口径 `仅 BT 任务支持断开并发连接（B）`） |
| QA-BE-12 | 09-bt-engine-12 | 下载中 BT 任务按 P | 弹窗元信息行 `BT · 块大小 256 KB · x/y 块 · 并发 N`（真实 piece 进度）；四态色块真实渲染（🟦 优先于 🟨）；下载前沿定位；滚动/关闭/鼠标同 FR-01-106 口径 |
| QA-BE-13 | 09-bt-engine-13 | 做种端到端：leecher 连接做种中任务拉数据 → 观察上传统计 → 倒计时归零；再跑一轮空格提前停止；重启恢复 | leecher 从本端拉到字节（fixture 对账）、上传统计真实增长（列表已上传/头部 ↑ 与峰值/面板上传两列/announce uploaded）；倒计时归零 → toast `✓ 做种结束: <name>`、转「已完成」、上传连接清空、tracker 收 stopped；空格 → toast `⏸ 已停止做种（进入已完成）: <name>`、转「已完成」、tracker 收 stopped；重启后做种恢复（uploaded 延续、seed_left 续走，FR-02-22） |
| QA-BE-14 | 09-bt-engine-14 | 配置 `[bt]` 表 bt_concurrency=8、seed_duration_secs=60、idle_eviction_secs=30 后添加 BT 任务观察至做种；另轮写非法值（0/负/非整数）观察回退；[http] 调优键同理抽验（throttle_burst_secs=0 回退 0.5） | 新 BT 任务并发预填/默认 = 8（v1.35：硬编码 20 退役，缺省回退代码常量）；做种列表行「剩余做种」自 60s 起倒计时；空闲置换按 30s 阈值执行（同 IP 重拨可观察）；其余 [bt] 调优键未配置时按代码常量默认生效、合法配置值引擎按值执行（下载/做种全链路行为不变）；非法值回退默认且启动不报错（统一口径 FR-01-71，与 QA-PC-04 回退行对账） |
| QA-BE-15 | 09-bt-engine-15 | 下载中任务空格暂停 → 空格开始重复 3 轮（fixture seeder/tracker 全程留档）；另轮修改并发下调收缩一条连接后随即改回 | **生命周期礼仪严格检查（v1.37/FR-02-27）**：每轮新握手 peer_id = 首轮（会话级稳定：进程内全部 BT 任务共用、跨暂停/重启/热调不变，`-EZ2000-` 形态）；每轮暂停 seeder 侧读到 EOF（本端先关写）且全程无 ConnectionReset（优雅关闭非 RST 强拆）；tracker 事件序列 = started → stopped → started 且全程同 peer_id（无「换 ID 重启」形态）；重启后无同 peer 双连（peer_id 去重既有口径）；下调收缩连接 seeder 侧读到 EOF 无 ConnectionReset；被收缩候选不计失败退避（fails 不被污染）且自关冷却内不向该候选重拨（同候选两次入连间隔 ≥ 冷却下限，fixture 观察入连时间戳）；任务全程下载中、进度单调不减 |

| QA-BE-16 | 09-bt-engine-16 | ①[bt] 表 listen_port = 51413 添加任务；②删配置重启两任务；③listen_port 占用（fixture 先占）启动 | ①fixture tracker 记录 announce port = 51413 且 fixture 拨入命中该端口；②两任务 announce port 相同（进程级共享单端口）且为随机值（非 51413）；③回退随机绑定 + 启动警告、不阻塞启动；0/负/非法值同②随机臂 |
| QA-BE-17 | 09-bt-engine-17 | ①下载至含 partial 块（fixture 限速门控）→ 空格暂停 → 退出重启 → 空格恢复，全程留档 seeder 请求日志；②手工篡改 sidecar partial 记录（越界/重叠/坏 b64/指向已完成 piece 各一次）重启恢复；③启动重校验窗内立即再暂停一轮 | ①恢复后 seeder 请求日志中已收 (piece, offset) 区间**零重请求**（仅缺失块区间被请求）；已收块字节保真（盘上对账）；恢复凑齐的完整 piece 校验通过直接落盘转 done（不等重下）；任务最终完成且产物 = seeder 原始字节；全程无同区间重复请求（BitComet 反吸血特征消除，与 QA-AL 联合回归）；②非法记录逐块丢弃自愈：该部分重下、其余合法块保留不重下、任务不卡死且最终完成；③允许边界（D68⑥）：重校验窗内二次暂停丢弃上一轮 partial（该部分重下）——本臂记录实测口径，不判 F |
| QA-BE-18 | 09-bt-engine-18 | ①BT 下载任务多 peer 观察（fixture seeder/leecher：一个正常供块 seeder、一个 choke 后恢复的 seeder、一个 interested 的 leecher、一个入连 leecher、一个刚建立无 interest 的新连接）逐帧读并发行标志；②wire 驱动翻转：fixture seeder 发 Choke→Unchoke、fixture leecher 发 Interested、choker 轮换（乐观槽周期）观察下一明细帧；③做种任务观察 | ①判定表逐项命中：供块 seeder 行 = D（+对方 interested 时 U/DU）、被 choke 的 seeder 行 = d、我方未 choke 的 interested leecher 行 = U、我方 choke 的 interested leecher 行 = u、乐观槽连接行含 O、入连行含 I、刚建立无 interest 连接行 = `-`；连写按 qBt 图例序无空格（如 DU/DUO/UI）；标志列内容与 04-ui-conns-08 v1.41 显示形态判据（QA-UC-08 v1.41）一致（左对齐、DIM、`-` 空值、连写）；②翻转逐项下一明细帧可见（D→d、d→D、U/u 出现与消失、O 轮换换位）；③做种行 = K/UI/`-` 等（我方无需求臂；对方无 interest 未 choke 臂 = ? 可出现）；全程 P/S/X/H/E/e/L 零出现（fixture 无对应能力，无判据不判 F）；HTTP 任务并发行无标志列（三列形态不变） |
| QA-BE-19 | 09-bt-engine-19 | ①[bt] peer_connection_protocol = ["webrtc"] 跑 BT 下载（fixture seeder）至完成；②= ["tcp"] 跑基线；③= ["tcp","webrtc"] 混跑并人为断开一条 WebRTC 连接观察重拨；④= ["WEBRTC","Tcp"]（大写）跑 | ①全部 peer 连接经 WebRTC DataChannel 承载 peer wire 全消息族（握手/bitfield/request/piece/cancel；SHA-1 全通过、产物字节一致、完成转做种口径不变），并发面板「协议」列全 = "WebRTC"、零 TCP peer 连接；②全部 = "TCP"（现行基线不变）；③两值均可按 peer 能力出现；被断连接按 FR-02-27 同传输退避重拨、**不产生同 peer 跨协议（WebRTC 失败改 TCP）重拨**（无跨协议回退，v1.41）；④大小写不敏感等价小写混合（全局开关解析归一）；列值显示形态判据 = QA-UC-08 v1.43 |
| QA-BE-20 | 09-bt-engine-20 | ①启动 ws(s) tracker harness `harness/tracker/mini_ws_tracker.py`（ws://127.0.0.1:17905/announce + wss://127.0.0.1:17906/announce；**harness 行为要求**：WebSocket 文本帧 JSON 消息——announce 请求含 action/info_hash/peer_id/port/uploaded/downloaded/left/event 字段，响应含 action:'announce' 与 info_hash（真实 tracker 契约：官方 bittorrent-tracker 客户端按 action 分派、对 info_hash 做 bin2hex，缺任一即判 invalid/崩溃并销毁全部 offers——20261109 与 webtorrent-cli 6.0.1 实测定位）、interval/complete/incomplete/peers 列表（peer 含 ip/port/peer_id），同 swarm 生态 ws tracker 消息形态；注册多 peer 并互换 peers 下发（offer/answer 以顶层独立消息即时推送对齐真实服务器语义，另保留响应内捎带数组兼容 ezr/harness 旧契约）；接受 started/completed 事件；wss 以自签证书提供 TLS；客户端异常断开不影响服务）；②配置 [bt] trackers=["ws://127.0.0.1:17905/announce"]、peer_connection_protocol=["webrtc"]，下载 harness 做种任务至完成；③换 wss://127.0.0.1:17906 同口径重跑；④分别写 peer_connection_protocol=["tcp","tcp"]（重复项）与 ["quic"]（未知值）后启动 | ①harness 正常收 ezr announce（ezr trackers 弹窗该行 = 工作中、Peers/Seeds 真实计数；harness 侧 peers 列表下发被引擎接续拨号）；②下载经 WebRTC DataChannel 完成（SHA-1 全通过；产物与做种侧字节一致；并发行「协议」列 = "WebRTC"）；③wss 臂同判据（TLS 按 harness 自签信任口径）；④两臂均启动失败并报错（错误含配置文件路径、键点路径与非法元素说明，非零退出码；FR-02-35） |

## 通过准则

- 全部用例结果记 P（通过）/ F（失败）/ S（受限跳过）；S 必须内嵌环境判据文案。
- 本套件可基于本地 fixture 重复执行，不依赖外网。
