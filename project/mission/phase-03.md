# 03 · BT 效率与发现扩展（第三期详细需求）(v0.1 计划登记稿)

> 期号：**03**（分期模式；需求大纲见 `../mission.md`）。
> **状态：计划登记稿**——操作者 20261027 指令提供的 phase3 实现计划，仅作分期登记；
> 详述条款（FR-03-xx / D 值 / 验收口径）由首角色在 phase-03 开工会话中依操作者输入补写定稿
> （宪法第一章），**phase-02 不处理本文件任何条款**。
> 承接：phase-02 已交付 BEP 3/15/23/7/12/9/20 下载基线（`mission/phase-02.md`）。

## phase3 实现计划（操作者钦定，20261027）

- **「命令窗口」增强**：增加可以输入 magnet link、以及 40 位的字符串；
  40 位的字符串补齐 magnet link 的头后进行解析
  （phase-02 D44 口径下磁力已可经「其余」臂进入 BT 对话框；本期将磁力输入升级为
  命令窗口一等公民：btih 解析、dn/tr 提取展示、40 位裸 infohash 自动补
  `magnet:?xt=urn:btih:` 头）。
- **BEP 5（BitTorrent DHT Protocol）**：DHT 核心规范——客户端在 DHT 网络中发送
  `get_peers` 查询寻找拥有特定 infohash 的 peer；同时使用 `announce_peer` 查询
  宣告自己拥有该 infohash，使其他 peer 能找到自己。
- **BEP 10（Extension Protocol）**：peer 间扩展协议的完整框架
  （phase-02 仅实现了 BEP 9 所需最小子集，本期补全扩展握手与扩展消息注册）。
- **BEP 6（Fast Extension）**：优化 peer 间通信效率（高延迟/高带宽环境减少等待、
  提升下载速度）；现代客户端基础功能集的一部分。
- **BEP 11（Peer Exchange / ut_pex）**：经 BEP 10 扩展协议交换 `added`（新增 peer）与
  `dropped`（断开 peer）列表。
- **BEP 14（Local Service/Peer Discovery）**：类 SSDP 机制，UDP 多播（默认端口 6771）
  在本地网络宣告存在，发现同一局域网内其他 peer。
- **BEP 51（DHT Infohash Indexing）**：`sample_infohashes` 查询，从 DHT 节点采样其
  存储的 infohash 列表。
- **BEP 32（BitTorrent DHT Extensions for IPv6）**：支持 IPv6 的 DHT。
- **config 新增 `dht_entry` 配置项**：默认值 `"dht.transmissionbt.com:6881"` 与
  `"router.bittorrent.com:6881"`，用作 DHT 网络引导入口。
