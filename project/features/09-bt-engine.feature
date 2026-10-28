# 场景清单（场景名 = feature 名称 + 稳定序号）：
#   09-bt-engine-01 .torrent 解析与任务元数据（announce-list / piece length / infohash / 单多文件）
#   09-bt-engine-02 BEP 3 HTTP tracker 获取 peers（compact=1 / BEP 23 紧凑解析 / interval / 事件）
#   09-bt-engine-03 BEP 15 UDP tracker（connect / announce / 紧凑 peers / 超时重试）
#   09-bt-engine-04 BEP 7 IPv6 peers（peers6 紧凑列表 / IPv6 连接）
#   09-bt-engine-05 BEP 12 多 tracker 冗余与 tracker 池三来源合并去重
#   09-bt-engine-06 peer 下载与 piece 校验（握手 / 分块请求 / SHA-1 / 多 peer 并发 / 上传统计）
#   09-bt-engine-07 BEP 9 元数据交换（做种侧响应：ut_metadata request → data/reject）
#   09-bt-engine-08 按文件下载语义（未选文件不落盘 / 边界 piece / 修改补下）
#   09-bt-engine-09 config trackers 配置项（http_trackers / udp_trackers / 模板示例 / 任务合并）
#   09-bt-engine-10 下载完成转做种（piece 全校验 → 做种中 / 产物字节一致 / tracker completed 事件）
#   09-bt-engine-11 B 断开连接（连接移除 / 分块补完 / 下载继续；做种中可用）
#   09-bt-engine-12 P pieces 弹窗 BT 臂（真实 piece 四态 / 块大小 = piece length）
#   09-bt-engine-13 做种与上传端到端（做种列表行 / 互惠上传 / 倒计时结束 / 空格停止做种 / 做种中交互）
# 期号 02（phase-02）。需求依据 project/mission/phase-02.md FR-02-06…23、D45/D46/D51/D52/D54/D57/D58。
# 协议基线 = 操作者钦定 BEP 3/15/23/7/12/9/20；tokio 异步（FR-02-10）。
# BEP 9 做种侧响应（D53/D45——磁力拉取 phase-03）；上传真实统计（D58）；
# 下载完成自动转做种（D57）；tracker 池三来源（去磁力 tr=，D53）。
# QA 经 UI 层观察 + fixture 侧可观察日志（本地 fixture 属暴露给 QA 的用户界面入口）。
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
    And 并发连接面板明细实时显示各 peer 下载速度/累计下载（口径同 HTTP 列）、上传速度/累计上传 = 真实统计（D45/D58：下载中互惠上传计入，fixture leecher 可从本端拉到已完成 piece 块）、客户端列 = 对端 peer id 前缀两字母缩写（无法识别 = --）、协议列 = TCP（D51）
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

  Scenario: 09-bt-engine-09 config trackers 配置项
    When 配置文件写入 http_trackers = [<http_url>] 与 udp_trackers = [<udp_url>]
    And 配置文件不存在时首次启动生成模板
    Then 模板包含两段注释示例行（含用途与取值范围说明，FR-01-85 机制）；非法 URL 行启动忽略 + 警告
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
