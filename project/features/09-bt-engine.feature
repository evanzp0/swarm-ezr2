# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   09-bt-engine-01 .torrent 解析与任务元数据（announce-list / piece length / infohash / 单多文件）
#   09-bt-engine-02 BEP 3 HTTP tracker 获取 peers（compact=1 / BEP 23 紧凑解析 / interval / 事件）
#   09-bt-engine-03 BEP 15 UDP tracker（connect / announce / 紧凑 peers / 超时重试）
#   09-bt-engine-04 BEP 7 IPv6 peers（peers6 紧凑列表 / IPv6 连接）
#   09-bt-engine-05 BEP 12 多 tracker 冗余与 tracker 池三来源合并去重
#   09-bt-engine-06 peer 下载与 piece 校验（握手 / 分块请求 / SHA-1 / 多 peer 并发 / 上传统计；v1.36 修订：客户端列识别改 qBittorrent 源码口径，FR-02-08 v1.3）
#   09-bt-engine-07 BEP 9 元数据交换（做种侧响应：ut_metadata request → data/reject）
#   09-bt-engine-08 按文件下载语义（未选文件不落盘 / 边界 piece / 修改补下）
#   09-bt-engine-09 config trackers 配置项（[bt] http_trackers / udp_trackers / 模板示例 / 任务合并）
#   09-bt-engine-10 下载完成转做种（piece 全校验 → 做种中 / 产物字节一致 / tracker completed 事件）
#   09-bt-engine-11 B 断开连接（连接移除 / 分块补完 / 下载继续；做种中可用）
#   09-bt-engine-12 P pieces 弹窗 BT 臂（真实 piece 四态 / 块大小 = piece length）
#   09-bt-engine-13 做种与上传端到端（做种列表行 / 互惠上传 / 倒计时结束 / 空格停止做种 / 做种中交互）
#   09-bt-engine-14 [bt] 调优键配置生效与回退（三大表重构：bt_concurrency 预填 / seed_duration_secs / 其余键引擎生效与缺省回退）
#   09-bt-engine-15 peer 连接生命周期礼仪（v1.37，FR-02-27：会话级 peer_id 稳定 / 优雅关闭 / 自关不计失败+冷却）
#   09-bt-engine-16 BT 监听端口 [bt] listen_port（v1.38，FR-02-32：配置生效 / 未配置随机 / 占用回退+警告 / 进程级共享）
#   09-bt-engine-17 partial 进度持久化与恢复不重下（v1.39，FR-02-22/D68：暂停重启不重发已收块 / 篡改自愈 / 重校验窗二次暂停允许边界）
# 期号 02（phase-02）。需求依据 project/mission/phase-02.md FR-02-06…23、D45/D46/D51/D52/D54/D57/D58。
# 协议基线 = 操作者钦定 BEP 3/15/23/7/12/9/20；tokio 异步（FR-02-10）。
# BEP 9 做种侧响应（D53/D45——磁力拉取 phase-03）；上传真实统计（D58）；
# 下载完成自动转做种（D57）；tracker 池三来源（去磁力 tr=，D53）。
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
# v1.39 修订（操作者 bug23 轮批准归档，specifier 轮；FR-02-22 v1.11，D68）：新增场景 17
# ——BT sidecar partial 持久化与恢复不重下：暂停/停止做种全量保存（BtSidecar 可选字段
# partial_pieces，b64 块数据，版本恒 1 不 bump）；重启恢复验证链（覆盖内 ∧ 非 done ∧
# 16 KiB 对齐 ∧ 单调不重叠 ∧ 界内）通过后已收块不重发请求；完整 piece 组装校验通过直接
# 落盘转 done；非法记录逐块丢弃自愈；endgame 跳过已收块；允许边界 = 启动重校验窗内
# 再次暂停丢弃上一轮 partial（该部分重下，操作者批准登记）。修复前缺陷 = Pause 终结引擎
# 丢 partial → 恢复后向同一 peer 重发同区间请求（BitComet 反吸血触发源，操作者 bug23 报告）。
# QA 严格检查 = QA-BE-17。
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

  Scenario: 09-bt-engine-05 BEP 12 多 tracker 冗余与 tracker 池合并
    When 任务 tracker 池 = config 默认（<cfg_tracker>）+ torrent announce-list + 手输 tracker 合并（三来源，去磁力 tr=）
    Then 池内 URL 去重（同 URL 只保留一条）、http(s) 与 udp 分型
    And 主 tracker（fixture 停服）失败后其余 tracker 接续提供 peers、下载不中断（任一 tracker 可推进）
    And 全部 tracker 失败 → 任务走失败臂（重试退避同 FR-01-41 口径）；tracker 列表随任务持久化并在修改对话框 trackers 预填

    Examples:
      | cfg_tracker                              |
      | https://tracker.fixture.local/announce   |

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

  Scenario: 09-bt-engine-08 按文件下载语义
    When 添加多文件任务且仅勾选 <selected> 个文件后确认下载
    Then 只下载选中文件覆盖的 piece、未选文件不建立数据不落盘（保存目录无未选文件路径）
    And 跨选中/未选边界的 piece 整 piece 下载（未选部分字节不写入文件、进度按 piece 记账）
    And 下载完成后选中文件字节与 seeder 原始字节一致（fixture 侧校验）
    When 经修改对话框补选先前未选文件
    Then 新增文件覆盖 piece 进入缺口补下（已落盘数据不重下）

    Examples:
      | selected |
      | 3        |

  Scenario: 09-bt-engine-09 config trackers 配置项（v1.35：迁入 [bt]）
    When 配置文件写入 [bt] 表 http_trackers = [<http_url>] 与 udp_trackers = [<udp_url>]
    And 配置文件不存在时首次启动生成模板
    Then 模板包含两段注释示例行（位于 [bt] 表内，含用途与取值范围说明，FR-01-85 机制）；非法 URL 行启动忽略 + 警告
    And 新建 BT 任务（未提供来源 tracker 时）tracker 池 = 两键合并结果；有来源 tracker 时并入池内去重
    And 手输 trackers（添加对话框 trackers 输入框每行一条）同样并入任务持久 tracker 列表

    Examples:
      | http_url                                | udp_url                                  |
      | https://tracker.fixture.local/announce  | udp://tracker.fixture.local:6969/announce |

  Scenario: 09-bt-engine-10 下载完成转做种
    When 全部选中 piece 下载并通过 SHA-1 校验
    Then toast 显示 "✓ 下载完成: <name>"（demo 无校验完成文案）且任务状态转「做种中」（D45/D57）
    And 列表信息行切做种中形态 "↑ <上传速度>  <做种>/<peer>  已上传 <x>/<总大小>  剩余做种 <时长>"（MAGENTA，剩余时长向上取整）
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
    And 任务上传统计真实增长（列表行 "已上传 <x>/<总>"、头部 ↑ 与峰值 ↑、并发明细上传两列、tracker announce uploaded 参数）
    And 做种中任务可查看并发明细、可按 B 断开、可按 M 修改（FR-02-03/08）、删除走既有删除对话框
    When 做种倒计时归零（seed_left 默认 1800s，D57）
    Then toast 显示 "✓ 做种结束: <name>" 且任务转「已完成」、上传连接清空、tracker 收到 stopped
    When 做种中按空格提前停止
    Then toast 显示 "⏸ 已停止做种（进入已完成）: <name>" 且任务转「已完成」、tracker 收到 stopped
    And 重启 ezr 后做种中任务恢复继续做种（uploaded 延续、seed_left 续走，FR-02-22）

  Scenario: 09-bt-engine-14 [bt] 调优键配置生效与回退（v1.35，三大表重构）
    Given 配置文件 [bt] 表设 bt_concurrency = <conns>、seed_duration_secs = <seed_secs>、
      idle_eviction_secs = <evict>（其余调优键未配置）
    When 添加 BT 任务（不经修改对话框改并发）并观察至下载完成转做种
    Then 新任务并发预填/默认 = <conns>（v1.35：BT_DEFAULT_CONCURRENCY 硬编码退役，缺省回退代码常量 20）
    And 做种列表行「剩余做种」自 <seed_secs> 起倒计时（v1.35：SEED_DURATION_SECS 硬编码退役，缺省回退代码常量 1800）
    And 空闲连接置换按 <evict> 秒阈值执行（无新增进账超时即断开重拨；缺省回退 60，操作者已批默认值）
    And 其余 [bt] 调优键（peer_pipeline / upload_slots / upload_read_concurrency /
      connect_timeout_secs / handshake_timeout_secs / request_timeout_secs /
      requests_per_piece / rechoke_interval_secs / optimistic_interval_secs /
      progress_emit_min_interval_ms / tracker_numwant / peer_retry_base_secs /
      peer_retry_cap_secs / tracker_backoff_start_secs / tracker_backoff_cap_secs /
      udp_base_timeout_secs / udp_retries）未配置时按代码常量默认值生效、配置合法值时引擎按
      配置值执行（下载/做种全链路行为不变，仅节律/窗口随配置伸缩）
    And [http] 调优键同理（progress_tick_ms / sidecar_flush_secs / throttle_burst_secs /
      throttle_sleep_ms / dns_race_grace_ms；HTTP 任务链路按配置值执行，非法值回退默认——
      回退样例行见 01-persistence-config-04）
    When [bt] 表写入 0/负值/非法值后启动
    Then 对应键回退代码常量默认值且启动不报错（统一回退口径，FR-01-71）

    Examples:
      | conns | seed_secs | evict |
      | 8     | 60        | 30    |


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
