# QA 套件 · 09-bt-engine BT 下载与做种引擎（期号 02）

> 对应规格：`project/features/09-bt-engine.feature`（14 场景）。
> 需求依据：`project/mission/phase-02.md` FR-02-06…25、D45/D46/D51/D52/D54/D57/D58/D60/D61。
> 协议基线：操作者钦定 BEP 3/15/23/7/12/9/20；tokio 异步（FR-02-10）。
> 上传真实统计（互惠/做种，D45/D58）、BEP 9 做种侧响应（D53）、下载完成转做种（D57）；
> tracker 池三来源（去磁力 tr=，D53）；v1.35（操作者 bug11 轮）：客户端列 全名/版本号、
> 地址列 ip缩写/port、[bt]/[http] 调优键入配置（FR-02-25，QA-BE-14 新增），D51 两字母缩写
> 口径随之改判。
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
| QA-BE-05 | 09-bt-engine-05 | 三来源合并任务（config + torrent + 手输）；停服主 tracker；全部停服 | 池去重（同 URL 单条）、http/udp 分型；主 tracker 失败其余接续（下载不中断）；全部失败 → 失败臂退避重试；tracker 列表持久化且修改对话框预填 |
| QA-BE-06 | 09-bt-engine-06 | 下载中观察 fixture 与并发面板；leecher 向本端请求数据 | seeder 观察标准握手（68 字节/protocol string/infohash/peer_id）；interested + request（≤16 KB 块、在飞 5 块）；piece SHA-1 校验、通过广播 have、失败重下；面板各 peer 速度/累计实时、上传两列 = 真实统计（D45/D58：leecher 从本端拉到已完成 piece 块、fixture 侧字节对账）、**客户端 = qBittorrent 源码口径两级识别（v1.36/FR-02-08 v1.3：①对端扩展握手携带 `v` 时显示其原样（如 libtorrent fixture 自报 `libtorrent/<版本号>`、BitComet 自报 `BitComet 1.64`——验收例：qBt 解析 1.64、旧 ezr 0.1.6.4，v1.36 起同源同值，fixture 侧对 v 串可观察对账）；②无 `v` 时 peer id 解码 = libtorrent identify_client 算法与映射表全量移植：fixture 可构造 -AR1001- → Arctic Torrent/1.0.0.1、-BC0164- → BitComet/0.1.6.4（peer id 路径）；映射表未命中 → 码本体；全零/全不匹配 → Unknown；旧 62 项子集表与逐位四段解码退役）**、明细客户端列左对齐（列宽 34）；协议 = TCP；**地址 = ip缩写/port（v1.35：斜杠分隔，如 1.*.4/51400，IPv6 ffff:*:ffff/65535 形态；旧冒号分隔退役）**；头部 ↓/↑ 与峰值真实计入 BT |
| QA-BE-07 | 09-bt-engine-07 | 做种中任务：leecher peer 发扩展握手 + ut_metadata request（合法分片 / 越界分片） | 本端回 extended handshake（m.ut_metadata id + metadata_size）；合法分片回 data（首片带 total_size）；越界分片回 reject；leecher 拼接 SHA-1 = infohash（fixture 侧观察）；完整 BEP 10 与拉取侧属 phase-03 |
| QA-BE-08 | 09-bt-engine-08 | 多文件任务只勾 3 文件下载；完成后补选文件 | 只落盘选中文件（未选路径不存在）；边界 piece 整 piece 下载、未选字节不写入；完成态选中文件字节 = seeder 原始字节；补选后缺口补下、已落盘 piece 不重下 |
| QA-BE-09 | 09-bt-engine-09 | 写入 `[bt]` 表 http_trackers/udp_trackers 配置（v1.17 三大表）；删配置重启动生成模板；配置非法 URL 行；无来源 tracker 任务 + 手输 trackers | 模板含两键注释示例（位于 [bt] 表内，FR-01-85 机制）；非法行忽略 + 警告；任务 tracker 池 = config 合并（去重分型）；手输每行一条并入持久 tracker 列表 |
| QA-BE-10 | 09-bt-engine-10 | 等待任务全部 piece 校验完成 | toast `✓ 下载完成: <name>` 且任务转「做种中」（D45/D57）；列表行切做种中形态 `↑ <速度>  <seeders>/<peers>  已上传 <x>/<总>  剩余做种 <时长>`；产物逐字节 = seeder 原始数据；引擎发 event=completed 且继续 announce（left=0、uploaded 真实） |
| QA-BE-11 | 09-bt-engine-11 | 下载中（及做种中）并发面板选中连接按 B；HTTP 任务按 B | 该连接移除（fixture 观察 socket 关闭、明细行消失）、块回归补完、下载持续不回退（toast ✂ 前缀）；做种中同样可断开（D45 改判）；HTTP 任务 toast 拒绝（文案 = phase-01 D26 维持口径 `仅 BT 任务支持断开并发连接（B）`） |
| QA-BE-12 | 09-bt-engine-12 | 下载中 BT 任务按 P | 弹窗元信息行 `BT · 块大小 256 KB · x/y 块 · 并发 N`（真实 piece 进度）；四态色块真实渲染（🟦 优先于 🟨）；下载前沿定位；滚动/关闭/鼠标同 FR-01-106 口径 |
| QA-BE-13 | 09-bt-engine-13 | 做种端到端：leecher 连接做种中任务拉数据 → 观察上传统计 → 倒计时归零；再跑一轮空格提前停止；重启恢复 | leecher 从本端拉到字节（fixture 对账）、上传统计真实增长（列表已上传/头部 ↑ 与峰值/面板上传两列/announce uploaded）；倒计时归零 → toast `✓ 做种结束: <name>`、转「已完成」、上传连接清空、tracker 收 stopped；空格 → toast `⏸ 已停止做种（进入已完成）: <name>`、转「已完成」、tracker 收 stopped；重启后做种恢复（uploaded 延续、seed_left 续走，FR-02-22） |
| QA-BE-14 | 09-bt-engine-14 | 配置 `[bt]` 表 bt_concurrency=8、seed_duration_secs=60、idle_eviction_secs=30 后添加 BT 任务观察至做种；另轮写非法值（0/负/非整数）观察回退；[http] 调优键同理抽验（throttle_burst_secs=0 回退 0.5） | 新 BT 任务并发预填/默认 = 8（v1.35：硬编码 20 退役，缺省回退代码常量）；做种列表行「剩余做种」自 60s 起倒计时；空闲置换按 30s 阈值执行（同 IP 重拨可观察）；其余 [bt] 调优键未配置时按代码常量默认生效、合法配置值引擎按值执行（下载/做种全链路行为不变）；非法值回退默认且启动不报错（统一口径 FR-01-71，与 QA-PC-04 回退行对账） |

## 通过准则

- 全部用例结果记 P（通过）/ F（失败）/ S（受限跳过）；S 必须内嵌环境判据文案。
- 本套件可基于本地 fixture 重复执行，不依赖外网。
