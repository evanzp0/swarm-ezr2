# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   09-bt-engine-01 .torrent 解析与任务元数据（announce-list / piece length / infohash / 单多文件）
#   09-bt-engine-02 BEP 3 HTTP tracker 获取 peers（compact=1 / BEP 23 紧凑解析 / interval / 事件）
#   09-bt-engine-03 BEP 15 UDP tracker（connect / announce / 紧凑 peers / 超时重试）
#   09-bt-engine-04 BEP 7 IPv6 peers（peers6 紧凑列表 / IPv6 连接）
#   09-bt-engine-05 BEP 12 多 tracker 冗余与 tracker 池三来源合并去重（v1.41：分型 = http(s)/ws(s)/udp 三族，FR-02-21 v1.15）
#   09-bt-engine-06 peer 下载与 piece 校验（握手 / 分块请求 / SHA-1 / 多 peer 并发 / 上传统计；v1.36 修订：客户端列识别改 qBittorrent 源码口径，FR-02-08 v1.3）
#   09-bt-engine-07 BEP 9 元数据交换（做种侧响应：ut_metadata request → data/reject）
#   09-bt-engine-08 按文件下载语义（未选文件不落盘 / 边界 piece / 修改补下）
#   09-bt-engine-09 config trackers 配置项（v1.41：统一单键 [bt] trackers，http_trackers/udp_trackers 退役废弃，FR-02-21 v1.15/FR-02-35）
#   09-bt-engine-10 下载完成转做种（piece 全校验 → 做种中 / 产物字节一致 / tracker completed 事件）
#   09-bt-engine-11 B 断开连接（连接移除 / 分块补完 / 下载继续；做种中可用）
#   09-bt-engine-12 P pieces 弹窗 BT 臂（真实 piece 四态 / 块大小 = piece length）
#   09-bt-engine-13 做种与上传端到端（做种列表行 / 互惠上传 / 倒计时结束 / 空格停止做种 / 做种中交互）
#   09-bt-engine-14 [bt] 调优键配置生效与回退（三大表重构：bt_concurrency 预填 / default_seed_duration_m（v1.51：seed_duration_secs 退役）/ 其余键引擎生效与缺省回退）
#   09-bt-engine-15 peer 连接生命周期礼仪（v1.37，FR-02-27：会话级 peer_id 稳定 / 优雅关闭 / 自关不计失败+冷却）
#   09-bt-engine-16 BT 监听端口 [bt] listen_port（v1.38，FR-02-32：配置生效 / 未配置随机 / 占用回退+警告 / 进程级共享）
#   09-bt-engine-17 partial 进度持久化与恢复不重下（v1.39，FR-02-22/D68：暂停重启不重发已收块 / 篡改自愈 / 重校验窗二次暂停允许边界）
#   09-bt-engine-18 并发 panel 标志列数据语义（v1.40，FR-02-33/D69：qBt Peers Flags 子集 D/d/U/u/O/I/K/? 判定表 / 连写顺序 / 实时刷新）
#   09-bt-engine-19 [bt] peer_connection_protocol 全局开关与 WebRTC 数据传输（v1.41 新增，FR-02-34/D70：无序数组 / 大小写不敏感 / 无跨协议回退；协议列 TCP·WebRTC）
#   09-bt-engine-20 ws(s) tracker announce 与 peer_connection_protocol 严格校验（v1.41 新增，FR-02-34/35/D70：ws(s) tracker harness e2e；未知值/重复项启动失败）
#   09-bt-engine-21 BT 下载列表行布局与种子健康度/分享率（v1.47 新增，v1.48 布局修订，v1.49 全类型扩面，v1.51 x1(x2)/y1(y2)+SR/LT+做种剩余做种退役，v1.52 D77② 落定/D78② 追认，v1.53 y1 取值改排除种子（FR-02-37 v1.3/D80），FR-02-37/38：任务行 [状态] 后 <总字节>（全类型尺寸已知） / 明细行 <x1(x2)/y1(y2)>/<h> / SR 2 位小数粉红 / LT 缩写（HTTP 同改 v1.52） / ∞ 边界 / 非活跃 dash）
#   09-bt-engine-22 并发 panel 相关度列数据语义（v1.47 新增，FR-02-36/D74：qBt calcRelevance 口径——对端有∧本端缺 ÷ 本端缺）
#   09-bt-engine-23 [bt.seed_exit_rule] 做种退出规则（v1.51 新增，v1.52 share_ratio_gt 保留 1 位小数细化，FR-02-40/D79：三键 AND 满足停止做种 / 未配置项不参与 / default_seed_duration_m 生效关系 / 非法值拒绝启动）
#   09-bt-engine-24 BT 顺序下载引擎语义（v1.51 新增，FR-02-39/D76：piece 索引升序线性领用 / 取消恢复 rarest-first / 随任务持久化）
# 期号 02（phase-02）。需求依据 project/mission/phase-02.md FR-02-06…23、D45/D46/D51/D52/D54/D57/D58。
# 协议基线 = 操作者钦定 BEP 3/15/23/7/12/9/20；tokio 异步（FR-02-10）。
# BEP 9 做种侧响应（D53/D45——磁力拉取 phase-03）；上传真实统计（D58）；
# 下载完成自动转做种（D57）；tracker 池三来源（去磁力 tr=，D53）。
# v1.41 修订（操作者修订轮指令「tracker 统一 + ws(s) 支持 + BT WebRTC + config 严格校验」；
# phase-02 v1.15，FR-02-21/31/33/08 v1.15、FR-02-34/35 新增、D70；Inversion 阶段 1 受限退化
# chat 输入选择题答案 1a/2b/3a/4a/5a，题目录重建登记见 phase-02 v1.15 注记与 handoff 待批）：
# ①场景 05/09：tracker scheme 白名单扩为 {http, https, ws, wss, udp}（大小写不敏感），
#   ws(s) 三来源同权入池（原 ws(s) 拒绝臂退役）；config [bt] 统一单键 trackers，
#   http_trackers/udp_trackers 退役废弃（不迁移不兼容，未知键忽略口径）；config trackers
#   白名单外 scheme = 启动失败并报错（FR-02-35，原「非法 URL 行忽略 + 警告」仅 announce-list 来源保留）；
# ②场景 19 新增：[bt] peer_connection_protocol 全局开关（无序数组、默认 ["tcp","webrtc"]、
#   大小写不敏感、未知值/重复项启动失败）与 WebRTC DataChannel peer 数据传输（无跨协议回退）；
# ③场景 20 新增：ws(s) tracker harness（mini_ws_tracker.py）e2e——ws(s) tracker announce +
#   peers 获取 + WebRTC 数据传输全链路验收。
# v1.36 修订（操作者 bug12 轮指令，客户端列识别修正）：客户端列解析法改 qBittorrent 源码口径
# （FR-02-08 v1.3：扩展握手 v 自报名优先 + peer id 解码 = libtorrent identify_client 算法与
# 映射表全量移植；验收例：BitComet 1.64 对端显示 BitComet 1.64——旧 62 项子集表与逐位四段
# 解码（0.1.6.4 形态）退役）。
# QA 经 UI 层观察 + fixture 侧可观察日志（本地 fixture 属暴露给 QA 的用户界面入口）。
# v1.37 修订（操作者 bug17 轮指令，FR-02-27 连接生命周期礼仪 + D66）：修复前缺陷 = peer_id 每
# 引擎启动随机生成（暂停/重启后同 IP 换 peer_id 立即重连，触发 BitComet 等反吸血屏蔽，qBittorrent
# 因会话级 peer_id + 优雅关闭而不触发）+ 自关路径引擎返回即整体丢弃连接（读半部滞留/RST 面）+
# 「引擎断开」关闭误记对端失败；修订后 = 会话级 peer_id 稳定（进程内全部 BT 任务共用，跨暂停/
# 重启/重试/热调不变，仅进程重启更换）+ 全部自发起关闭路径优雅关闭（先关写 FIN、排水窗口、
# 无 RST）+ 自关后候选进重拨冷却（不递增失败、不立即重拨；真实失败退避不变）。QA 严格检查 = QA-BE-15。
# v1.38 修订（操作者 bug18 轮指令五项功能之一/二/五：全局与任务级上传下载限速 + [bt] listen_port；
# Inversion 阶段 1 答案 = 操作者 chat 指令文本，受限退化沿 bug13–17 轮先例；FR-02-28/29/32 新增，
# 裁决登记 phase-02 D67）：BT 引擎接入任务级下载/上传限速（原 BtSpec.max_speed dead_code 字段
# 退役，改任务级两字段；做种与互惠上传合计受上传限速约束；Reconfigure 载体扩展限速参数热调，
# 引擎不重启）与全局限速（全局两键合计配额跨协议共享，合成 = min(全局, 任务级)，见
# 01-throttle-proxy v1.9）；[bt] 表新增 listen_port 键（20 键 → 21 键，场景 16 专管）；
# 监听由每任务独立随机端口改为进程级共享单端口（未配置随机、配置生效、占用回退随机 + 警告）。
# v1.40 修订（操作者「并发 panel 标志列」轮指令，FR-02-33 新增/D69 裁决已批（操作者 20261104 chat 批准七项全项）；Inversion 受限退化
# 沿 bug13–18 轮先例，操作者 chat 指令文本即阶段 1 答案）：新增场景 18——并发行标志数据语义：
# 标志集 = qBt 官方 FAQ Flags 图例的 ezr 引擎可产生子集 {D,d,U,u,O,I,K,?}（P/S/X/H/E/e/L 对应
# μTP/snubbed/PEX/DHT/协议加密/本地发现未实现不出现）；参考源判定 = qBt Peers（用户）面板
# Flags 列（Trackers 面板无 Flags 列，官方 FAQ 核证）。QA 严格检查 = QA-BE-18（fixture 侧
# choke/interest/入连/乐观槽可观察驱动）。
# v1.39 修订（操作者 bug23 轮批准归档，specifier 轮；FR-02-22 v1.11，D68）：新增场景 17
# ——BT sidecar partial 持久化与恢复不重下：暂停/停止做种全量保存（BtSidecar 可选字段
# partial_pieces，b64 块数据，版本恒 1 不 bump）；重启恢复验证链（覆盖内 ∧ 非 done ∧
# 16 KiB 对齐 ∧ 单调不重叠 ∧ 界内）通过后已收块不重发请求；完整 piece 组装校验通过直接
# 落盘转 done；非法记录逐块丢弃自愈；endgame 跳过已收块；允许边界 = 启动重校验窗内
# 再次暂停丢弃上一轮 partial（该部分重下，操作者批准登记）。修复前缺陷 = Pause 终结引擎
# 丢 partial → 恢复后向同一 peer 重发同区间请求（BitComet 反吸血触发源，操作者 bug23 报告）。
# QA 严格检查 = QA-BE-17。
# v1.42 修订（操作者 bug28 轮指令 20261109「多文件 b/b 双层 / 多文件删除目录树 / WebRTC 地址列 /
# 扩容重连调优」；chat 指令文本即答案，受限退化沿 bug13–25 轮先例；D72 登记 handoff）：
# ①落盘布局钦定——多文件 torrent 落盘 = save_dir/<info.name>/…（torrent 内相对路径，首段即
# info.name），单文件 = save_dir/<info.name> 文件；任务详情保存路径展示（target_path =
# save_dir/name）与之一致；修复前缺陷 = 多段 rel 再叠任务名（save_dir/name/name/… 双层，bug28①）。
# ②「删除任务和文件」语义澄清（01-resume-sidecar-12 v1.47 注）：多文件 BT 任务「目标文件」=
# save_dir/name/ 目录树整删（原 remove_file 对目录无效且静默失败，bug28②）。
# ③并发面板 WebRTC 连接地址列 = 对端 peer_id 前 5 字节 hex 裸值（D71⑤ 改判：协议列已显
# WebRTC，"WebRTC:" 前缀退役）。
# ④扩容重连调优（bug28③）：修改任务并发扩容/仍有缺口时全 tracker 提前 re-announce（ws 持久
# 会话立即携新 offers 重连 shed 的 peer；http/udp 刷新候选池；尊重 tracker min_interval）+
# 本端主动收口（缩容 shed/空闲置换）的 WebRTC peer 不记失败冷却（mark_peer_idle，区别于异常
# 断开的惩罚性退避）——peer 重连不再等 tracker interval 周期。QA 严格检查 = QA-BE-19（观察面注记）。
# v1.53 修订（操作者修订轮指令 20261114「bt 下载任务：确认下 QTORRENT 源码中这个任务列表中的
# PEERS 和 SEEDS 连接数的取值是不是这个逻辑，如果 EZR 的任务列表中进度条下方的 PEERS 和 SEEDS
# 连接数不是相同的逻辑，则修改 EZR 的 PEERS 和 SEEDS 连接数取值逻辑」+ 引文「括号外 PEERS/SEEDS =
# 直接连接数而非 tracker 返回数、PEERS 列不包含 SEEDS（= 正在下载的 Leecher 数）、做种态 SEEDS 显 0
# 而 PEERS 有数正常、未完成态 SEEDS 有数 = 正在从种子获取数据」——phase-02 v1.23，FR-02-37 v1.3/D80；
# Inversion 阶段 1 受限退化沿 v1.45–v1.54 八轮先例，操作者指令文本即阶段 1 答案）：qBt master 取源核证
# （transferlistmodel.cpp L386-388 TR_SEEDS = amountString(seedsCount(), totalSeedsCount()) /
# TR_PEERS = amountString(leechsCount(), totalLeechersCount())；torrentimpl.cpp L1488-1517
# seedsCount = num_seeds、leechsCount = num_peers − num_seeds、totalSeedsCount/totalLeechersCount =
# tracker 报告 complete/incomplete）——操作者引文与源完全一致；EZR 对判：x1（连接种子数）已同源维持，
# y1（D78③ conns.len() 含种子）不同源 → 修订 = 实际连接的非种子 peers 数（BtConns 快照 seeder==false
# 连接计数，等价 conns.len() − x1）；场景 21 y1 判据修订、场景 10 做种臂注记同步；x2/y2 括号段、
# tracker 未知括号段省略（D78②）、非活跃 -/-/-、SR/LT 形态均不变；ezr 不做旧版本兼容（沿 v1.45 钦定）。
# v1.52 修订（操作者裁决落实 20261113「LT: http和 BT 下载任务都是做 '剩余时间→LT' 这样的改动；
# share_ratio_gt 允许小数保留1位小数；SR 作用于做种/下载两臂同构；tracker 值未知时括号段省略」——
# v1.51 轮合成闸门 B + D75–D79 否决窗口关闭，phase-02 v1.22，FR-02-38 v1.22/D77②、FR-02-40/D79①）：
# ①场景 21 HTTP 臂注记修订——HTTP 任务行「剩余」标签同改 LT（v1.51「HTTP 行不变」口径退役；
# HTTP 下载/暂停两态信息行归 01-tui-display v1.52 场景 02 承载），SR 两臂同构追认（D77①）、
# tracker 未知括号段省略追认（D78②）；②场景 23 share_ratio_gt 取值细化——正浮点保留 1 位小数
# （D79①，原「无小数位」裁读注记退役）；本文件场景行形态零改动（判据面注记级）。
# v1.51 修订（操作者修订轮指令 20261113「BT 顺序下载 / 做种退出规则 / 显示折叠与缩写」——phase-02 v1.21，FR-02-37 v1.2/FR-02-38 v1.21/FR-02-39/40 新增、D75–D79；Inversion 受限退化沿 v1.45–v1.52 先例，操作者指令文本即阶段 1 答案）：
# ①场景 10/21 明细行格式修订——seeders/peers 改 x1(x2)/y1(y2)（x1 连接种子/x2 tracker complete/y1 连接 peers/y2 tracker incomplete，tracker 值未知括号段省略，FR-02-37 v1.2/D78）；「分享」标签改「SR」、BT 下载臂「剩余」标签改「LT」（HTTP 行不变，FR-02-38 v1.21/D77）；做种臂「剩余做种 <时长>」项整项退役（退出规则归 FR-02-40）；②场景 14 seed_duration_secs 键退役改 default_seed_duration_m（分钟，默认 30；exit rule 任一键配置时失效）；③场景 13 退出触发改「退出条件满足」（时长臂 = default_seed_duration_m 倒计时归零，rule 臂 = 已配置条件 AND 满足）；④场景 23 新增（[bt.seed_exit_rule] 退出规则）、场景 24 新增（顺序下载引擎语义）。
# v1.50 修订（操作者调整轮指令 20261111「bt 下载任务：做种的 bt 任务，去除 '已上传 <上传字节数>/<总字节数>'」——phase-02 v1.20，FR-02-38 v1.20；操作者 chat 指令即修订裁决）：
# 做种统计行已上传对退役——场景 10/21 做种臂形态去 `已上传 <x>/<总大小>`，分享率改接
# <seeders>/<peers>/<健康度> 项（单空格衔接不变，与 BT 下载臂同构）；场景 13 上传统计观察面
# 改头部/并发明细/announce（列表行不再含已上传）；上传量仅经分享率体现，健康度/剩余做种不变。
# v1.49 修订（操作者调整轮指令 20261111「所有下载任务（http/bt 等）明细尺寸项上移 + 删除已下载字节只留总字节」——v1.48 作用面扩判，phase-02 v1.19，FR-02-38 v1.19；操作者 chat 指令即修订裁决）：
# 场景 21 作用面扩为**全类型/全状态**——尺寸已知（FR-01-116 size_known 语义）任务行「[状态]」徽标后
# 均追加 <总字节>（原「做种中/已完成/已失败/等待中不追加、HTTP 不追加」口径退役）；明细统计行
# 尺寸项整项退役（含已完成尾 ` · total`、未知尺寸 <已下载>/? B）；做种中已上传对保留；
# 已完成/已失败/等待中明细瘦身后只剩自身语义项；HTTP 行不含健康度/分享率项（FR-02-37/38 仍限 BT）。
# 通用任务行（含 HTTP）载体的规格同步 = 01-tui-display v1.49（场景 02/28）。
# v1.48 修订（操作者调整轮指令 20261111「bt 下载明细布局调整 + 并发协议列对齐修正」，
# phase-02 v1.18，FR-02-38 v1.18；操作者 chat 指令即修订裁决）：场景 21 修订——明细统计行
# 尺寸对 <已下载字节>/<总字节> 整项上移至任务行「[状态]」徽标后（单空格间隔、只留
# <总字节>），明细行不再显示尺寸对；分享率项改接 <seeders>/<peers>/<健康度> 项后
# （单空格衔接不变）；作用面 = BT 分支状态（下载中/校验中/后期处理/已暂停），
# 做种中/已完成/已失败/等待中与 HTTP 行形态不变（v1.49 扩面后上述「不追加」口径退役）。
# v1.47 修订（操作者修订轮指令 20261110「bt 下载任务显示指标三项」，phase-02 v1.17，
# FR-02-33 v1.17/FR-02-36/37/38 新增、D74；操作者 chat 指令即修订裁决，Inversion 受限退化
# 沿 bug13–25 轮先例）：①场景 10 做种中列表行形态修订——<做种>/<peer> 扩为
# <做种>/<peer>/<健康度>（3 位小数）、已上传项后追加「 分享 <分享率>」（2 位小数，数字 MAGENTA）；
# ②场景 21 新增：BT 下载列表明细行种子健康度与分享率（下载中/做种中/非活跃 dash 三臂 + ∞ 边界，
# qBt Availability/realRatio 同源口径）；③场景 22 新增：并发 panel 相关度列数据语义
# （qBt PeerInfo::calcRelevance 同源口径；显示形态见 04-ui-conns-08 v1.47）。
# v1.43 修订（操作者修订轮指令 20261109「BT 多文件任务修改对话框文件勾选行为修订」三需求；
# phase-02 v1.16，FR-02-03/FR-02-19 v1.16，D73；Inversion 阶段 1 受限退化 chat 输入选择题答案
# 1A 2B 3B 4A 5A）：场景 08 修订——①取消选中 = 已落盘该文件数据删除钦定落实（文件粒度 +
# 变空目录自底向上整树清理，4A；目标不存在静默跳过，5A；删除 IO 失败不阻塞确认与勾选生效，
# D73④；勾回已删文件从头补下，原数据保留断点语义退役）；②新增做种回退臂：做种中任务补选
# 新文件 → 任务回下载态、上传暂停至再次全部完成（2B；可观察面 = 上传速度 0 / uploaded 与
# announce uploaded 不增长 / leecher 拉不到数据）、回做种时 seed_left 重置 config 默认（3B）。
# QA 严格检查 = QA-BE-08（修订）+ QA-MB-10/11/12（UI 层）。
Feature: 09-bt-engine BT 下载与做种引擎（tracker / peer / 上传 / 文件选择）

  期号 02。真实 BT 链路：.torrent 解析 → tracker announce（HTTP/UDP、紧凑 peer、
  IPv6、多 tracker 冗余）→ peer wire 协议并发分块下载（SHA-1 piece 校验）→ 按文件落盘 →
  完成自动做种上传（D45：互惠/全量 unchoke、BEP 9 做种侧响应、tracker left=0）。

  Background:
    Given ezr 以干净环境启动
    And 本地 BT fixture 已启动（HTTP tracker、UDP tracker、seeder peer、leecher peer 各至少一台，日志可观察）
    And 预置种子文件（单文件 / 多文件两种；piece length <piece_len>）

  Scenario: 09-bt-engine-01 .torrent 解析与任务元数据
    When 经命令窗口以本地路径添加 "<torrent>" 任务
    Then 任务元数据解析正确：任务名 = info.name、分块行显示 "x/y · <piece_len>/块"（y = ⌈大小 ÷ piece_len⌉）
    And announce-list 全部层级 tracker 纳入任务 tracker 池（BEP 12；多文件种子文件树路径 = 元数据相对路径）
    And infohash = info 字典字节 SHA-1（fixture 侧可观察对同一 infohash announce）
    And 解析失败种子（非法 bencode / 缺 info）添加被拒绝且 toast "⚠ 无法读取种子文件"

    Examples:
      | torrent              | piece_len |
      | multi-file.torrent   | 256 KB    |
      | big-file.torrent     | 256 KB    |

  Scenario: 09-bt-engine-02 BEP 3 HTTP tracker 获取 peers
    When 添加仅含 HTTP tracker 的任务并开始下载
    Then fixture HTTP tracker 收到 announce：info_hash / peer_id（-EZ2000- 形态，BEP 20）/ port / uploaded（真实累计上传字节，D58）/ downloaded / left / compact=1 / event=started
    And 响应的 BEP 23 紧凑 peer 列表（每 peer 6 字节）被正确解析并建立连接（并发连接面板出现明细行）
    And 后续按响应 interval 周期重 announce（event 空）；全部 piece 校验完成后下一轮 event=completed 且不停止轮询（做种继续 announce，D57）
    And tracker 响应 complete/incomplete 计入列表做种/peer 展示（<seeders>/<peers>）

  Scenario: 09-bt-engine-03 BEP 15 UDP tracker
    When 添加含 UDP tracker（udp://fixture.local:6969/announce）的任务并开始下载
    Then fixture UDP tracker 收到 connect（magic 0x41727101980、action=0）并回 connection_id
    And 随后收到 announce（action=1、info_hash / peer_id / downloaded / left / event）并回紧凑 peers
    And peers 被解析并建立连接、下载推进
    When UDP tracker 无响应（fixture 停服）
    Then 该 tracker 按协议重试（倍增间隔）后判失败、任务经其他 tracker 继续下载（FR-02-16 冗余）

  Scenario: 09-bt-engine-04 BEP 7 IPv6 peers
    When fixture HTTP tracker 响应含 peers6（IPv6 紧凑列表、每 peer 18 字节）
    Then IPv6 peer 被解析并建立连接（并发面板 IP 列显示 IPv6 地址）
    And 下载经 IPv6 peer 推进（字节可观察落盘）

  Scenario: 09-bt-engine-05 BEP 12 多 tracker 冗余与 tracker 池合并（v1.41：分型 = http(s)/ws(s)/udp 三族，FR-02-21 v1.15）
    When 任务 tracker 池 = config 默认（<cfg_tracker>）+ torrent announce-list + 手输 tracker 合并（三来源，去磁力 tr=；ws(s) 与 http(s)/udp 同权入池，v1.41）
    Then 池内 URL 去重（同 URL 只保留一条）、scheme 大小写不敏感归一分型为 http(s)/ws(s)/udp 三族（WS:// 与 ws:// 同族）
    And 主 tracker（fixture 停服）失败后其余 tracker 接续提供 peers、下载不中断（任一 tracker 可推进）
    And 全部 tracker 失败 → 任务走失败臂（重试退避同 FR-01-41 口径）；tracker 列表随任务持久化并在修改对话框 trackers 预填

    Examples:
      | cfg_tracker                              |
      | https://tracker.fixture.local/announce   |
      | wss://tracker.fixture.local/ws           |

  Scenario: 09-bt-engine-06 peer 下载与 piece 校验
    When 下载进行中
    Then fixture seeder 侧观察到标准 peer wire 握手（19 + "BitTorrent protocol" + reserved + infohash + peer_id）
    And 本端发送 interested 并按缺口对已选文件覆盖的 piece 发送 request（块长 ≤ 16 KB、单 peer 在飞窗口 5 块）
    And piece 全块落盘后 SHA-1 校验（对 metadata pieces 对应段）、通过后广播 have；校验失败 piece 重下
    And 并发连接面板明细实时显示各 peer 下载速度/累计下载（口径同 HTTP 列）、上传速度/累计上传 = 真实统计（D45/D58：下载中互惠上传计入，fixture leecher 可从本端拉到已完成 piece 块）、客户端列 = 客户端识别显示（v1.36 修订：解析法 = qBittorrent 源码口径两级——①**扩展握手 `v` 自报名优先**：对端扩展握手携带 `v` 字符串时显示其原样（空白归并 + 不可打印字符剔除后；qBittorrent 同源行为——libtorrent bt_peer_connection.cpp 扩展握手 `v` 覆盖 peer id 解码结果，如 BitComet 自报 `BitComet 1.64`、libtorrent 自报 `libtorrent/<版本号>`）；②**无 `v` 时按 peer id 解码 = libtorrent identify_client 算法与映射表全量移植**（qBittorrent 捆绑同源）：非标准形态优先（generic_mappings 37 项按 offset+前缀子串匹配、仅名无版本，如 exbc→BitComet；`-BOW`+尾`-`→`Bits on Wheels <3 字符>`；`eX` 前缀→`eXeem ('<12 字符>')`；前 12 字节全零+第 13 字节 \x97/\0 → Experimental 两态）→ Transmission legacy（`-TRxYYR-`，x∈0–3、YY 两位 minor、R 为数字或 A–Z 字母 tag）→ Azureus 形态（`-XX????-`：首 `−`、码后首字符可打印、第 3–7 字符均 ≥'0'、尾 `−`；4 位版本逐位 decode_digit = 数字原值/大写字母→10–35）→ Shadow 形态（首字符字母或数字；`--` 结尾臂 = 后 3 字符逐位 decode，否则 = 后 3 字节原值且第 9 字节为 0）→ Mainline 形态（`<字符><1-3 位数>-<1-3 位数>-<1-3 位数>--`）；版本格式 = `M.m.r`（tag≠0 时追加 `.tag`）；全名映射表 = name_map 102 项全量按码排序查找（如 AR→Arctic Torrent、BC→BitComet、qB→qBittorrent、QD→QQDownload、SD/XL→Xunlei、DP→Propagate Data Client），未命中 → 以 1–2 字符码本体为名；全零 peer id 与全部形态均不匹配 → Unknown（不采用 lt 的 `Unknown [原始串]` 形态，v1.35 口径维持）。**验收例（操作者钦定）：同一 BitComet 1.64 对端，qBittorrent 解析版本号 = 1.64、ezr v1.35 旧实现 = 0.1.6.4——v1.36 起与 qBittorrent 同源同值**；旧 62 项子集表与逐位四段点分解码退役；D51 相应改判）、协议列 = TCP；地址列 = ip缩写/port（v1.35 修订：斜杠分隔，IPv4 首尾组遮掩与 IPv6 首尾组遮掩三形态同口径，如 1.*.4/51400、ffff:*:ffff/65535；旧冒号分隔形态退役，bug10 轮 coder 偏离登记收编清零）
    And 头部全局 ↓/↑ 与峰值 ↓/↑ 计入 BT 任务真实速度（MAGENTA 上传位，header.rs 形态）

  Scenario: 09-bt-engine-07 BEP 9 元数据交换（做种侧响应）
    When 本端任务做种中（全部 piece 已完成，元数据完整可用）
    And fixture leecher peer 以磁力方式连接本端并发起扩展握手（reserved 元数据位）与 ut_metadata request（msg_type=0，分片每片 16 KiB）
    Then 本端回 extended handshake（上报 m.ut_metadata 消息 id + metadata_size）
    And 对每个合法分片请求回 data（msg_type=1，携带对应元数据分片；首片携带 total_size）
    And 对超出元数据范围的分片请求回 reject（msg_type=2）
    And leecher 拼接元数据后 SHA-1 = infohash 校验通过（fixture 侧观察；完整 BEP 10 与拉取侧属 phase-03）

  # v1.43 修订（操作者修订轮 20261109，FR-02-19 v1.16/D73）：取消选中 = 落盘数据删除；
  # 新增做种回退臂（上传暂停 + seed_left 重置）。
  Scenario: 09-bt-engine-08 按文件下载语义
    When 添加多文件任务且仅勾选 <selected> 个文件后确认下载
    Then 只下载选中文件覆盖的 piece、未选文件不建立数据不落盘（保存目录无未选文件路径）
    And 跨选中/未选边界的 piece 整 piece 下载（未选部分字节不写入文件、进度按 piece 记账）
    And 下载完成后选中文件字节与 seeder 原始字节一致（fixture 侧校验）
    When 经修改对话框补选先前未选文件
    Then 新增文件覆盖 piece 进入缺口补下（已落盘数据不重下）
    When 经修改对话框取消已下载文件的勾选后确认
    Then 该文件落盘数据删除（保存目录无该文件路径；变空目录自底向上整树清理；目标不存在静默跳过）
    And 对应 piece 退出缺口与可服务集合、已下载量按新 total 钳制
    When 重新勾选该文件后确认
    Then 该文件从头补下（数据已删无断点；v1.43 反转，原断点保留语义退役）
    When 做种中任务经修改对话框补选新文件确认
    Then 任务回下载态且上传暂停（该任务上传速度 0、uploaded 与 announce uploaded 不增长、
      leecher 拉不到数据；D73②）直至再次全部完成
    And 再次全部完成后回做种态、seed_left 重置为 config seed_duration_secs 当前值（D73③）、上传恢复

    Examples:
      | selected |
      | 3        |

  Scenario: 09-bt-engine-09 config trackers 配置项（v1.41：统一单键 [bt] trackers，FR-02-21 v1.15/FR-02-35）
    When 配置文件写入 [bt] 表 trackers = [<t1>, <t2>]（http(s)/ws(s)/udp 混排，scheme 大小写不敏感）
    And 配置文件不存在时首次启动生成模板
    Then 模板包含 trackers 单键注释示例行（位于 [bt] 表内，含用途与取值范围说明——元素限 http(s)/ws(s)/udp scheme 白名单，FR-01-85 机制）；白名单外 scheme 元素 = 启动失败并报错（FR-02-35，原「非法 URL 行启动忽略 + 警告」对 config 来源退役）
    And 旧键 http_trackers / udp_trackers 出现时按未知键口径直接废弃（不迁移、不兼容、不并入池，FR-02-35 退役规则；池只来自 trackers 键）
    And 新建 BT 任务（未提供来源 tracker 时）tracker 池 = trackers 键合并结果（ws(s) 同权）；有来源 tracker 时并入池内去重
    And 手输 trackers（添加对话框 trackers 输入框每行一条）同样并入任务持久 tracker 列表

    Examples:
      | t1                                      | t2                                        |
      | https://tracker.fixture.local/announce  | wss://tracker.fixture.local/ws            |
      | ws://tracker.fixture.local:8080/announce | udp://tracker.fixture.local:6969/announce |

  Scenario: 09-bt-engine-10 下载完成转做种
    When 全部选中 piece 下载并通过 SHA-1 校验
    Then toast 显示 "✓ 下载完成: <name>"（demo 无校验完成文案）且任务状态转「做种中」（D45/D57）
    And 列表信息行切做种中形态 "↑ <上传速度>  <x1(x2)/y1(y2)>/<健康度> SR <分享率>"（MAGENTA 做种/peer/健康度段，SR 数字 MAGENTA 粉红；v1.47 修订：原 "<做种>/<peer>" 两段与无分享率形态退役，FR-02-37/38/D74，细则见场景 21；v1.50 修订：原 `已上传 <x>/<总大小>` 项退役，分享率改接健康度项，FR-02-38 v1.20；v1.51 修订：seeders/peers 改 x1(x2)/y1(y2)（FR-02-37 v1.2/D78）、"分享" 标签改 "SR"（FR-02-38 v1.21/D77）、"剩余做种 <时长>" 项整项退役——v1.50"健康度/剩余做种不变"口径中剩余做种臂退役，做种退出规则归 FR-02-40，剩余时长不再展示；v1.53 修订：y1 = 实际连接的非种子 peers 数（排除种子，FR-02-37 v1.3/D80，细则见场景 21））
    And 产物文件字节与 seeder 原始数据逐字节一致（fixture 侧校验）
    And 引擎已对 tracker 发送 event=completed 且继续周期 announce（left=0、uploaded 真实）

  Scenario: 09-bt-engine-11 B 断开连接
    When 任务下载中（或做种中，D45）并发面板选中一条连接按 B
    Then 该连接被移除（fixture 侧可观察 socket 关闭）、并发明细行消失
    And 其持有的未完成块回归待分配并由其他/新建连接分块补完、下载持续推进不回退（toast ✂ 前缀）
    And HTTP 任务按 B 仍 toast 拒绝（文案同 phase-01 D26 维持口径）

  Scenario: 09-bt-engine-12 P pieces 弹窗 BT 臂
    When 选中下载中的 BT 任务按 P
    Then 打开分块状态弹窗、元信息行显示 "BT · 块大小 <piece_len> · x/y 块 · 并发 N"（x/y = 真实 piece 进度）
    And 四态色块按真实 piece 状态渲染（🟩 已下载 / 🟦 正在下载 / 🟨 部分下载 / ⬜ 未下载、🟦 优先于 🟨）
    And 打开定位下载前沿、滚动/关闭/鼠标交互同 FR-01-106 既有口径

    Examples:
      | piece_len |
      | 256 KB    |

  Scenario: 09-bt-engine-13 做种与上传端到端（D45，FR-02-23）
    When 下载完成任务进入做种中（fixture leecher peer 连接本端）
    Then 本端对 leecher 全量 unchoke 并响应其 request（发送已完成 piece 块、块 ≤ 16 KB）——leecher 从本端拉到数据字节（fixture 侧对账）
    And 任务上传统计真实增长（头部 ↑ 与峰值 ↑、并发明细上传两列、tracker announce uploaded 参数；v1.50：列表行不再含已上传项，上传量仅经列表分享率体现，FR-02-38 v1.20）
    And 做种中任务可查看并发明细、可按 B 断开、可按 M 修改（FR-02-03/08）、删除走既有删除对话框
    When 做种退出条件满足（v1.51/FR-02-40：[bt.seed_exit_rule] 未配置 → default_seed_duration_m 时长倒计时归零（未配置默认 30 分钟，D57/D79）；已配置 → 已配置条件全部满足（AND）即时收尾）
    Then toast 显示 "✓ 做种结束: <name>" 且任务转「已完成」、上传连接清空、tracker 收到 stopped
    When 做种中按空格提前停止
    Then toast 显示 "⏸ 已停止做种（进入已完成）: <name>" 且任务转「已完成」、tracker 收到 stopped
    And 重启 ezr 后做种中任务恢复继续做种（uploaded 延续、default_seed_duration_m 臂 seed_left 续走 / [bt.seed_exit_rule] 臂做种时长累计续走，FR-02-22）

  Scenario: 09-bt-engine-14 [bt] 调优键配置生效与回退（v1.35，三大表重构；v1.51：seed_duration_secs → default_seed_duration_m）
    Given 配置文件 [bt] 表设 bt_concurrency = <conns>、default_seed_duration_m = <seed_m>、
      idle_eviction_secs = <evict>（其余调优键未配置；v1.51：seed_duration_secs 键退役废弃——按未知键口径直接废弃（FR-02-35 ⑤/FR-02-40：零迁移零警告不报错），改 default_seed_duration_m 单位分钟）
    When 添加 BT 任务（不经修改对话框改并发）并观察至下载完成转做种
    Then 新任务并发预填/默认 = <conns>（v1.35：BT_DEFAULT_CONCURRENCY 硬编码退役，缺省回退代码常量 20）
    And 做种时长默认 = <seed_m> 分钟（v1.35：SEED_DURATION_SECS 硬编码退役；v1.51：seed_duration_secs 键退役改 default_seed_duration_m，未配置默认 30 分钟（原 1800s 等值）；[bt.seed_exit_rule] 任一键配置时本键失效（FR-02-40/D79）；做种列表行不再显示剩余做种倒计时（FR-02-38 v1.21），退出时点经 fixture 侧观察）
    And 空闲连接置换按 <evict> 秒阈值执行（无新增进账超时即断开重拨；缺省回退 60，操作者已批默认值）
    And 其余 [bt] 调优键（peer_pipeline / upload_slots / upload_read_concurrency /
      connect_timeout_secs / handshake_timeout_secs / request_timeout_secs /
      requests_per_piece / rechoke_interval_secs / optimistic_interval_secs /
      progress_emit_min_interval_ms / tracker_numwant / peer_retry_base_secs /
      peer_retry_cap_secs / tracker_backoff_start_secs / tracker_backoff_cap_secs /
      udp_base_timeout_secs / udp_retries）未配置时按代码常量默认值生效、配置合法值时引擎按
      配置值执行（下载/做种全链路行为不变，仅节律/窗口随配置伸缩）
    And [http] 调优键同理（progress_tick_ms / sidecar_flush_secs / throttle_burst_secs /
      throttle_sleep_ms / dns_race_grace_ms；HTTP 任务链路按配置值执行，语义非法值拒绝启动（FR-02-35 v1.41，样例行见 01-persistence-config-04 v1.21）
    When [bt] 表写入 0/负值/非法值后启动
    Then 启动失败并报错（v1.41 反转：错误含配置文件路径、键点路径与非法原因，非零退出码退出；原「对应键回退代码常量默认值且启动不报错」口径随 FR-01-71 回退臂全局退役，FR-02-35）

    Examples:
      | conns | seed_m | evict |
      | 8     | 1      | 30    |


  # v1.37 新增（操作者 bug17 轮指令，FR-02-27 连接生命周期礼仪 + D66）：修改并发与暂停/启用
  # 任务按 qBittorrent 规则优化、不触发对端反吸血——四判据全部 fixture 侧可观察（peer_id 全等、
  # EOF 非 RST、自关冷却、事件序列一致），QA 套件按本场景严格检查（QA-BE-15）。
  Scenario: 09-bt-engine-15 peer 连接生命周期礼仪（v1.37，FR-02-27）
    Given 任务下载中且与 fixture seeder 保持连接（fixture 可观察握手与 socket 关闭）
    When 对该任务按空格暂停、再次按空格开始（重复 3 轮）
    Then 每轮开始后 fixture seeder 收到的新握手 peer_id 与首轮完全一致（会话级稳定：
      进程内全部 BT 任务共用、跨暂停/重启/热调不变，-EZ2000- 形态维持）
    And 每轮暂停时 fixture seeder 侧观察到有序关闭：本端先关写、seeder 读到 EOF，
      全程无 ConnectionReset（优雅关闭而非 RST 强拆）
    And fixture tracker 每轮收到的事件序列 = started（首轮）→ stopped（每轮暂停）→
      started（重启）且全程同 peer_id（对端视角为一致客户端生命周期，无「换 ID 重启」形态）
    And 重启后对同 peer 无并行双连（同 peer_id 单连接既有口径不变）
    When 下载中任务修改并发下调收缩一条连接、随即改回原并发
    Then 被收缩连接以同口径优雅关闭（seeder 侧读到 EOF，无 ConnectionReset）
    And 被收缩候选不因自关计入失败退避（fails 不被污染），且自关冷却内不向该候选重拨
      （fixture seeder 观察同候选两次入连间隔 ≥ 冷却下限）
    And 任务全程保持下载中、进度单调不减（收缩/回补判据同 08-modify-bt-05）


  # v1.38 新增（操作者 bug18 轮指令五项功能之五，FR-02-32 + D67）：config [bt] listen_port——
  # 未明确配置则随机绑定端口（现状 0.0.0.0:0 口径转正为回退语义）；配置生效则监听该端口；
  # 绑定失败（占用）回退随机 + 启动警告；监听为进程级共享单端口（同 qBittorrent：全部 BT 任务
  # 共用，announce port 参数与入连端口一致）。
  Scenario: 09-bt-engine-16 BT 监听端口 [bt] listen_port（v1.38，FR-02-32）
    Given 配置文件 [bt] 表 listen_port = <port>（未配置 / <port> 两分支）
    When 添加 BT 任务至引擎运行（fixture tracker 记录 announce 参数）
    Then 监听端口为 <observed>（fixture 侧实际拨入端口与 announce port 参数一致）
    And 第二个 BT 任务并发运行时 announce port 与首个任务相同（进程级共享单端口，非每任务随机）
    And 配置 0/负值/非法值时按未配置处理（随机绑定）；配置但绑定失败（端口被占用）时回退随机绑定
      + 启动警告（沿 config warnings 口径），不阻塞启动

    Examples:
      | port     | observed          |
      | 未配置   | 随机端口（非固定值）   |
      | 51413    | 51413              |


  # v1.39 新增（操作者 bug23 轮批准归档，FR-02-22 v1.11 + D68）：BT sidecar partial 持久化与
  # 恢复不重下——修复「暂停→恢复向同一 peer 重发同区间请求」的重复下载特征（BitComet 反吸血
  # 触发源）；反吸血联动回归归口 10-bt-anti-leech。
  Scenario: 09-bt-engine-17 partial 进度持久化与恢复不重下（v1.39，FR-02-22/D68）
    Given 任务下载中且已收部分块（含未满 piece 的 partial 块，fixture seeder 请求日志可观察已收区间）
    When 按空格暂停后退出 ezr 并重新启动、恢复该任务
    Then 启动恢复载入 sidecar partial 块（BtSidecar partial_pieces）：已收块不向 seeder 重发请求（fixture 请求日志：恢复后请求仅覆盖缺失块区间，已收 (piece, offset) 区间零重请求）
    And 已收块字节保真（sidecar 载入与盘上字节一致；恢复凑齐的完整 piece 组装校验通过后直接落盘转 done，不等重下）
    And 任务最终下载完成且产物与 seeder 原始数据逐字节一致
    And 暂停→恢复全程对同一 peer 无同区间重复请求（BitComet 反吸血特征消除，与 10-bt-anti-leech 联合回归）
    When sidecar partial 记录被篡改（越界 / 区间重叠 / 坏 b64 / 指向已完成 piece）后重启恢复
    Then 非法记录逐块丢弃自愈（该部分重下、其余合法块保留不重下、任务不卡死且最终完成）
    And 允许边界（操作者批准登记，D68⑥）：启动重校验窗内再次暂停时上一轮 partial 丢弃（该部分重下）；正常路径（校验窗完成后暂停/退出）partial 完整持久化

  # v1.40 新增（FR-02-33/D69）：并发 panel BT 行「标志」列的数据语义层——显示形态（列序/列宽/
  # 连写/空值/对齐/配色）属 04-ui-conns-08 v1.41，本场景只定义每连接的标志判定与数据源。
  # 引擎状态源 = ConnStat 既有 peer_choking/am_choking/peer_interested 三字段 + choker 乐观槽
  # （apply_choker optimistic_slot）+ 入连/出连方向（listen accept vs 外连 dial）；「我方对其
  # 无需求」= 本端未对该 peer 声明 interested（无未完成 piece 需求，既有 Bitfield/Have want
  # 判定同口径可复算）。实现映射方式（新增 am_interested 跟踪或按 has∩缺口复算）属实现细节，
  # 规格只锁可观察判定。
  Scenario: 09-bt-engine-18 并发 panel 标志列数据语义（v1.40，FR-02-33/D69）
    Given BT 任务下载中或做种中且并发明细存在至少一条已建立连接（fixture seeder/leecher 可观察）
    Then 每条并发行标志按下列判定表逐连接独立计算（每 tick 随 BtConns 事件同帧刷新，与速度列同节拍）：
      | 标志 | 判定（qBt 官方 FAQ 图例语义）                                | 引擎状态源 |
      | D    | 我方已声明 interested 且对方未 choke（Currently downloading） | !peer_choking ∧ 我方有未完成需求 |
      | d    | 我方已声明 interested 但对方 choke（interested and choked）   | peer_choking ∧ 我方有未完成需求 |
      | U    | 对方 interested 且我方未 choke（Currently uploading）         | !am_choking ∧ peer_interested |
      | u    | 对方 interested 但我方 choke（interested and choked）         | am_choking ∧ peer_interested |
      | O    | 本轮乐观槽选中该连接（Optimistic unchoke）                    | apply_choker optimistic_slot 命中 |
      | I    | 对方主动拨入、本端 accept（incoming connection）              | 入连方向标记（listen accept） |
      | K    | 对方未 choke 我方且我方对其无需求（qBt K 图例）               | !peer_choking ∧ 我方无未完成需求 |
      | ?    | 我方未 choke 对方且对方对我方无兴趣（qBt ? 图例）             | !am_choking ∧ !peer_interested |
    And 多标志按 qBt 图例序 D d U u O I K ? 无空格连写（如下载互惠连接 = DU、被 choke 的候选源 = d、
      乐观槽互惠连接 = DUO、做种入连上传中 = UI）；无任何标志（如刚建立双方均 choke 且无 interest 的新连接）显示 `-`
    And 标志随状态变化实时翻转：被对方 choke（Choke 消息）D→d、解除 choke（Unchoke 消息）d→D、
      对方 interested（Interested 消息）出现 U/u、我方 choke（choker 轮换）U→u、乐观槽轮换 O 换位、
      下一明细帧可见（fixture 侧发送对应 wire 消息驱动，可观察判定）
    And 仅 BT 任务并发行显示标志（HTTP 三列形态不变）；握手未完成连接不显示（既有 peer_id=None 过滤口径）
    And qBt 图例其余旗标 P/S/X/H/E/e/L 不出现（μTP/snubbed/PEX/DHT/协议加密/本地发现为 phase-02 未实现能力，
      phase-03/04 落地时再评估扩充；fixture 侧无法触发即 QA 无判据，不作为缺陷）


  # v1.41 新增（操作者修订轮指令：BT 任务支持 WebRTC——WebRTC DataChannel 承载 peer 数据传输；
  # FR-02-34/D70；无跨协议回退；并发面板「协议」列按连接实际传输显示 TCP/WebRTC，见 04-ui-conns-08 v1.43；
  # QA 经 ws(s) tracker harness 端到端验证，见场景 20 与 QA-BE-19/20）。
  Scenario: 09-bt-engine-19 [bt] peer_connection_protocol 全局开关与 WebRTC 数据传输（v1.41 新增，FR-02-34/D70）
    Given 配置文件 [bt] 表 peer_connection_protocol = <modes>（无序字符串数组；大小写不敏感；全局开关——决定 BT 任务 peer 连接与数据传输可用的传输协议集合；未配置时默认 ["tcp","webrtc"]，与模板示例值逐键一致）
    When BT 任务经 tracker（含 ws(s) tracker harness）获取 peers 并下载至完成
    Then 允许集 <expect>（Webrtc 臂 = WebRTC DataChannel 承载 peer wire 全消息族：握手后 bitfield/have/request/piece/cancel 收发与 TCP 臂同语义；piece SHA-1 校验、在飞窗口、上传统计、下载完成转做种口径不变）
    And 无跨协议回退：peer 在允许集内某传输上失败时，不对该 peer 换用另一允许传输重拨（按 FR-02-27 同传输重试礼仪退避；允许集之外的传输一律不发起、不回退、不接受）
    And 未配置时默认 ["tcp","webrtc"]：TCP 连接行为与既有基线一致、WebRTC 连接同为合法通道（缺省不改变既有 TCP 链路可用性）
    And 并发面板「协议」列 = 该连接实际传输（"TCP" / "WebRTC"，列宽 8，见 04-ui-conns-08 v1.43）

    Examples:
      | modes                 | expect                                 |
      | ["webrtc"]            | 仅 WebRTC（零 TCP peer 连接）          |
      | ["tcp"]               | 仅 TCP（现行基线）                     |
      | ["tcp", "webrtc"]     | TCP 与 WebRTC 均可（按 peer 能力择一） |
      | ["WEBRTC", "Tcp"]     | 大小写不敏感，等价 ["webrtc","tcp"]    |


  # v1.41 新增（操作者修订轮指令：ws(s) tracker harness 用于测试 + WebRTC 数据传输经 harness 验收；
  # FR-02-34/35/D70）。harness 行为规格见 qa/09-bt-engine-qa.md QA-BE-20 与 harness/README 登记口径。
  Scenario: 09-bt-engine-20 ws(s) tracker announce 与 peer_connection_protocol 严格校验（v1.41 新增，FR-02-34/35/D70）
    Given ws(s) tracker harness（harness/tracker/mini_ws_tracker.py，行为规格见 QA-BE-20）已就绪（ws://127.0.0.1:17905/announce 与 wss://127.0.0.1:17906/announce）
    When 配置 [bt] trackers = ["<ws_url>"]、peer_connection_protocol = ["webrtc"]，添加 harness 做种任务的 .torrent 下载至完成
    Then announce 经 ws(s) tracker 完成（trackers 弹窗该行状态 = 工作中、Peers/Seeds 真实计数；peer 列表经 ws(s) tracker 下发并被引擎接续拨号）
    And 下载数据经 WebRTC DataChannel 完成（SHA-1 全部通过、产物字节与做种侧一致、完成后转做种口径不变；并发面板该连接「协议」列 = "WebRTC"）
    And [bt] peer_connection_protocol = <bad>（未知值或重复项，FR-02-35）时启动失败并报错（错误含配置文件路径、键点路径与非法元素；合法集合 = {tcp, webrtc} 去重后 1–2 项）

    Examples:
      | ws_url                           | bad            |
      | ws://127.0.0.1:17905/announce    | ["tcp","tcp"]  |
      | wss://127.0.0.1:17906/announce   | ["quic"]       |


  # v1.47 新增（操作者修订轮指令「bt 下载任务显示指标三项」②③，FR-02-37/38/D74）：
  # 列表行统计行的显示形态与数据语义层——种子健康度 = qBt General 面板 Availability
  # （libtorrent distributed_copies 同源口径），分享率 = qBt General 面板 Share Ratio
  # （realRatio 同源口径）。并发 panel 相关度列显示形态归 04-ui-conns-08 v1.47，数据语义归场景 22。
  Scenario: 09-bt-engine-21 BT 下载列表行布局与种子健康度/分享率（v1.47 新增，v1.48 布局修订，v1.49 全类型扩面，v1.50 做种已上传对退役，v1.51 x1(x2)/y1(y2) + SR/LT + 做种剩余做种退役，v1.52 D77② 落定/D78② 追认，v1.53 y1 取值改排除种子，FR-02-37 v1.2/v1.3/38 v1.21/D74/D77/D78/D80）
    Given BT 任务下载中且 tracker 已有成功 announce 应答（complete/incomplete 计入 x2/y2）
    Then 任务行「[状态]」徽标后以单空格间隔显示 "<总字节> <单位>"（fmt_size 口径、普通前景色；
      如 "[下载中] 4.19 GB"；v1.48：原明细统计行尺寸对 <已下载>/<总大小> 整项上移至此、
      删除 <已下载字节> 只保留 <总字节>；v1.49 扩面：作用面 = 全部任务类型与状态（尺寸已知即显示，
      FR-01-116 size_known 语义；尺寸未知任务行 1 不显示尺寸、无伪造总字节）——BT 全状态含
      做种中/已完成/已失败/等待中均追加，HTTP/HTTPS 任务行同样追加）
    And 列表统计行 seeders/peers 项显示 "<x1(x2)/y1(y2)>/<健康度>"（健康度保留 3 位小数，如 12(45)/35(120)/1.675；
      v1.51：x1 = 实际连接种子数、x2 = tracker 最新成功应答 complete 值、
      y2 = tracker 最新成功应答 incomplete 值（qBt 任务列表 SEEDS/PEERS 列取值逻辑同源，FR-02-37 v1.2/D78；
      原 <seeders>/<peers> 两段口径退役）；v1.53：y1 = 实际连接的非种子 peers 数（leecher，排除种子；
      FR-02-37 v1.3/D80③——操作者修订轮指令「PEERS 和 SEEDS 双列核证，不同源则修改」：BtConns 快照中
      seeder==false 的连接计数，等价连接 peers 总数 − x1；qBt leechsCount() = num_peers − num_seeds 同源
      （transferlistmodel.cpp TR_PEERS = amountString(leechsCount(), totalLeechersCount())，torrentimpl.cpp）；
      v1.2「y1 = 实际连接 peers 数（含种子）」口径退役；qBt 源核证：SEEDS 括号外 = seedsCount（连接种子）、
      PEERS 括号外 = leechsCount（排除种子）、括号内 = tracker 报告 complete/incomplete，操作者引文
      「括号外 = 直接连接数、PEERS 不含 SEEDS、做种态 SEEDS≈0 而 PEERS 有数、未完成态 SEEDS 有数」与源一致；
      x1 已同源维持不变）；tracker 值未知（无 tracker/本轮未成功应答/未带字段）→ 括号段省略
      显示 "x1/y1" 形态（qBt total == -1 只显计数同款，D78②；操作者 20261113 追认——v1.52）；健康度计算 = 连接 peer piece 位图估算的分布式完整副本数
      ——每 piece 持有者计数（连接的非种子 peer 持有该 piece 记 1、本端已有该 piece 记 1）取全部 piece 最小值 +
      连接种子数 + 持有者计数超出该最小值的 piece 数 ÷ 总 piece 数，qBt Availability / libtorrent distributed_copies
      同源，随 tick 同节拍刷新）
    And 列表统计行不再含 <已下载>/<总大小> 尺寸对（v1.18：整项上移任务行，原位退役）
    And 列表统计行 <x1(x2)/y1(y2)>/<健康度> 项后以单空格显示 " SR <分享率>"（保留 2 位小数；分享率 = 累计上传 ÷ 累计下载，
      下载量 < total_done 1% 时按 total_done 兜底、分母 0 且上传 > 0 显示 "∞"、两者均 0 显示 "0.00"，qBt realRatio 同源；
      分享率数字 MAGENTA 粉红、"SR" 标签 DIM——v1.51：标签 "分享" 改缩写 "SR"（FR-02-38 v1.21/D77，具体名称见帮助窗口
      【任务列表】组 SR 条目；操作者钦定示例 "0.00/3.34 GB 分享 0.07" 的标签字面随此退役）；v1.18 起衔接对象为健康度项）
    And BT 下载臂剩余时间标签显 " LT <eta>"（v1.51：标签 "剩余" 改缩写 "LT"（FR-02-38 v1.21/D77，具体名称见帮助窗口
      【任务列表】组 LT 条目）；HTTP 任务行 "剩余" 标签同改 "LT"（v1.52：操作者裁决 D77② 扩面——http 与 BT
      下载任务都做「剩余时间→LT」同款改动，HTTP 两态信息行归 01-tui-display v1.52 场景 02 承载）；非下载中臂显 " LT " + dash 位；ETA 值与 DIM 标签族口径不变）
    When 任务下载完成转做种中
    Then 做种统计行同口径——"↑ <上传速度>  <x1(x2)/y1(y2)>/<健康度> SR <分享率>"（品红段整体同色；分享率数字 MAGENTA；
      uploaded 增长时分享率随之增长；v1.50：`已上传 <x>/<总大小>` 项整项退役——分享率改接健康度项
      （单空格衔接，与 BT 下载臂同构），做种上传量仅经分享率与头部/并发明细体现（FR-02-38 v1.20，
      原 v1.47 形态 `已上传 <x>/<总大小> 分享 <分享率>` 与 v1.49「已上传对保留」口径退役）；
      v1.51：`剩余做种 <时长>` 项整项退役（FR-02-38 v1.21/D77——v1.50「健康度/剩余做种……均不变」口径中
      剩余做种臂退役，做种退出规则归 FR-02-40，剩余时长不再展示）；v1.49：做种中任务行标题同样追加 <总字节>）
    When 任务暂停（非活跃 BT 状态）
    Then 速度段逐位 dash 照旧、seeders/peers 项显 "-/-/-"（dash 三段形态不变，D78③——不展开为六值括号形态）、
      任务行标题尺寸项照常、分享率按真实统计显示（uploaded/downloaded；全 0 = "0.00"）、剩余时间标签 " LT " + dash
    And 已完成 BT 任务行明细只剩校验结论（v1.49：尾部 ` · <总大小>` 上移退役，行 1 已含总字节）与
      HTTP 任务行不含健康度与分享率项（FR-02-37/38 仍限 BT；已完成 BT 行无 <s>/<p> 形态）

    Examples:
      | seeders/peers  | 健康度 |
      | 12(45)/35(120) | 2.418  |
      | 8(14)/23(31)   | 1.675  |


  # v1.47 新增（操作者修订轮指令「bt 下载任务显示指标三项」①，FR-02-36/D74）：并发 panel
  # 「相关度」列的数据语义层——显示形态（列位/列宽/对齐/格式）属 04-ui-conns-08 v1.47，
  # 本场景只定义每连接的相关度判定与数据源（qBt Peers 面板 Relevance 列同源口径）。
  Scenario: 09-bt-engine-22 并发 panel 相关度列数据语义（v1.47 新增，FR-02-36/D74）
    Given BT 任务下载中或做种中且并发明细存在至少一条已建立连接（fixture seeder/leecher 可观察）
    Then 每条并发行相关度 = （对端拥有 ∧ 本端缺失的 piece 数）÷（本端缺失 piece 总数）
      （qBt PeerInfo::calcRelevance 同源：本端缺失为 0（做种中/已完成下载）时恒 0；逐连接独立计算）
    And 显示为百分数保留 1 位小数 + "%"（如 "100.0%"、"37.5%"、"0.0%"；qBt
      fromDouble(relevance*100, 1) + '%' 同款；"100.0%" 6 显示列在列宽 7 内恒完整无截断）
    And 相关度随并发明细同帧刷新（每 tick BtConns 事件与速度列同节拍）：对端发 Have/本端收块后
      下一明细帧重算（本端收块 → 本端缺失下降 → 相关度上升；对端新持有本端缺失 piece → 相关度上升；
      本端收齐全部 piece → 相关度恒 0.0%）
    And 仅 BT 任务并发行显示（HTTP 三列形态不变）；握手未完成连接不显示（既有 peer_id=None 过滤口径）

    Examples:
      | 本端缺失 | 对端有∧本端缺 | 显示   |
      | 100      | 100           | 100.0% |
      | 40       | 15            | 37.5%  |
      | 0        | 0             | 0.0%   |


  # v1.51 新增（操作者修订轮指令「config.toml [bt] 表添加子表 [bt.seed_exit_rule]（做种退出
  # 条件），当满足所有条件后任务停止做种；[bt] seed_duration_secs 废弃改 default_seed_duration_m
  # （单位分钟）」；FR-02-40/D79）：做种退出规则数据语义层；config 键面 = 01-persistence-config-13
  # v1.21 与 01-config-template-01 v1.17（v1.18：share_ratio_gt 取值范围注记补「保留 1 位小数」）。
  # v1.52：share_ratio_gt 取值细化（正浮点保留 1 位小数，D79① 操作者裁决）；seeders_count_gt/
  # seed_duration_m 正整数追认。
  Scenario: 09-bt-engine-23 [bt.seed_exit_rule] 做种退出规则（v1.51 新增，v1.52 share_ratio_gt 保留 1 位小数细化，FR-02-40/D79）
    Given 配置文件 [bt] 子表 [bt.seed_exit_rule] 设 <rule>（三键可选——share_ratio_gt：正浮点比率语义、
      保留 1 位小数，1.5 = 150%（操作者钦定示例 + 20261113 裁决细化/D79①）；seeders_count_gt：正整数，按 x1 实际连接种子数计算（操作者钦定）；
      seed_duration_m：正整数，做种状态累计时长、跨暂停/重启续走（FR-02-22 同源机制））
    When BT 任务下载完成进入做种中并持续观察至退出
    Then 退出判定 = 全部已配置条件同时满足（AND）即停止做种——任务转「已完成」、toast "✓ 做种结束: <name>"、
      上传连接清空、tracker 收到 stopped（与 default_seed_duration_m 倒计时收尾同口径，FR-02-13）
    And 未配置的条件不参与判定（只配 share_ratio_gt = 1.5 即纯分享率退出、可能无限做种；
      只配 seed_duration_m = 60 即纯时长退出；三者全配即三条件同时满足才退出）
    And [bt.seed_exit_rule] 任一键配置时 [bt] default_seed_duration_m 失效（不再默认 30 分钟倒计时退出）；
      子表未配置（不存在或三键皆无）时 default_seed_duration_m 生效（未配置默认 30 分钟）
    And 条件随做种 tick 评估（秒级），满足即时收尾；seeders_count_gt 实时跟随 x1（实际连接种子数）变化；
      share_ratio_gt 实时跟随分享率（累计上传 ÷ 累计下载，FR-02-38 口径）
    And 做种中空格提前停止做种（FR-02-13）不变；D73③ 回做种重置在 exit rule 生效臂 = 做种时长累计清零
      （若 seed_duration_m 为已配置条件之一）
    And 非法值拒绝启动（FR-02-35 ③：任一键 0/负 → 启动失败；seeders_count_gt/seed_duration_m 非整数 →
      启动失败；错误含配置文件路径、键点路径与非法原因）

    Examples:
      | rule                                        |
      | share_ratio_gt = 1.5                        |
      | seeders_count_gt = 3                        |
      | seed_duration_m = 60                        |
      | share_ratio_gt = 1.5 + seed_duration_m = 60 |

  # v1.51 新增（操作者修订轮指令「添加和修改任务窗口，增加'按顺序下载' 选项，默认不勾选，
  # 随任务持久化，策略参考 qtorrent 的 'Download in sequential order'，确认后应立即生效」；
  # FR-02-39/D76）：顺序下载引擎数据语义层（对话框交互面 = 07-add-bt-12 / 08-modify-bt-13）。
  Scenario: 09-bt-engine-24 BT 顺序下载引擎语义（v1.51 新增，FR-02-39/D76）
    Given BT 任务开启「按顺序下载」（添加对话框勾选创建或修改对话框勾选确认；默认不勾选）
    When 任务下载中观察 piece 领用与请求顺序（fixture seeder 侧 request 日志可观察）
    Then piece 请求顺序 = 按 piece 索引升序线性推进（自当前首个缺口 piece 起依序领用，
      qBt/libtorrent sequential_download 同源；中途开启不重扫已收 piece、不从 0 重下）
    And 修改对话框取消勾选确认 → 立即恢复 rarest-first 领用（任务不中断、进度不重置、
      已建立连接保持——FR-02-26 立即生效总规则兜底）
    And 修改补下臂（FR-02-03/08 新勾选文件补下）在顺序开启时按 piece 索引升序
    And 顺序臂只改领用顺序：在飞窗口、SHA-1 校验、choke 礼仪（FR-02-27）、限速、
      做种上传/互惠上传（D58）全部不变
    And 勾选状态随任务持久化（重启恢复保持，FR-02-22 机制）；既有任务（无该字段）默认不勾选
