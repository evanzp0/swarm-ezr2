# QA 套件 · 10-bt-anti-leech BT 自动反吸血与黑名单（期号 02，v1.9 新建）

> 对应规格：`project/features/10-bt-anti-leech.feature`（8 场景）。
> 需求依据：`project/mission/phase-02.md` FR-02-30、D67（bug18 轮设计定义值，待批）。
> 策略口径：qBittorrent/libtorrent 无自动封禁 → 主参考 BitComet PSR + 自设计三判据
> （leech / bad_piece / protocol）；阈值 = D67 登记实现定义值（leech：up ≥ 1 MiB 且
> down < 64 KiB 且存活 ≥ 120s；bad_piece：累计 3；protocol：破坏性消息首次即封、
> 非法区间 request 重复 3）；做种任务豁免 leech 判据。
> 端到端 UI 层验证：以用户可达入口（toast/并发面板/blacklist.yaml 文件观察）为主；
> 本地 fixture（tracker/seeder/leecher 恶意形态可编程）属暴露给 QA 的用户界面入口
> （SKILL 允许口径），用于驱动「吸血/坏块/违规」形态并做 fixture 侧可观察对账。

## 环境前置

- 伪终端驱动：按键注入、画面捕获、toast 断言；独立 HOME。
- 本地 BT fixture 四件套（同 09-bt-engine-qa）：HTTP tracker / UDP tracker / seeder /
  leecher，**可编程恶意形态**：
  - leecher 形态（握手后只发 request 接收、从不回传 piece、不 choke 不 interested 反向）；
  - 坏块形态（对 request 返回与 pieces 校验不符的伪 piece 数据）；
  - 协议违规形态（未知消息 id / 帧长度与载荷不符 / 非法区间 request 反复）。
- fixture 全部握手/消息/socket 关闭日志可观察（判据以 fixture 侧 + toast + 文件三面对账）。
- 分批复跑：`QA_AL_ONLY=<编号列表>` 过滤（缺省全量）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-AL-01 | 10-bt-anti-leech-01 | BT 任务下载中，leecher fixture L 挂入 tracker（只收不给）；维持连接至判据阈值 | L 连接被断开（fixture 观察有序关闭 EOF 非 RST）；toast `⛔ 已封禁吸血对端 <ip>（只下载不上传）`；blacklist.yaml 新增条目（ip/reason=leech/banned_at）；并发面板该行消失；L 后续重连被拒（入连拒绝）、不再被拨出 |
| QA-AL-02 | 10-bt-anti-leech-02 | 任务下载完成转做种中；纯 leecher L2 持续拉取（只收不给）超过 leech 阈值 | 不触发封禁（连接保持、上传继续、黑名单无 L2、文件不变）——做种无私上传合法（BitComet 口径） |
| QA-AL-03 | 10-bt-anti-leech-03 | 坏块 fixture S 对 request 返回伪 piece 数据，累计 3 次 | 第 3 次后断开 + toast `⛔ 已封禁对端 <ip>（连续提供坏块）`；blacklist.yaml 条目 reason=bad_piece；坏块 piece 照常重新调度（其他来源补完、产物最终完整） |
| QA-AL-04 | 10-bt-anti-leech-04 | 恶意 fixture M 发破坏性消息（未知 id/帧长不符）1 次；另一形态发非法区间 request 3 次 | 破坏性消息首次即封；非法区间第 3 次封；toast `⛔ 已封禁对端 <ip>（协议违规）`；条目 reason=protocol |
| QA-AL-05 | 10-bt-anti-leech-05 | 任一判据命中瞬间三面观察 | 连接有序关闭（EOF 非 RST，FR-02-27② 口径）+ toast 可见（3 秒自动消失）+ blacklist.yaml 即时可读出（ip/reason/banned_at 三字段）+ 并发面板行消失 |
| QA-AL-06 | 10-bt-anti-leech-06 | 预置 blacklist.yaml（ip=192.0.2.7）启动/重启；随后手工删除条目重启 | 该 IP 候选拨出/入连 accept 均被拒绝；手工删除并重启后解封（可正常连接）；文件损坏（非法 YAML）时空名单启动 + 启动警告、不阻塞 |
| QA-AL-07 | 10-bt-anti-leech-07 | 黑名单含 192.0.2.7；tracker 响应 peers 含该 IP 且该 IP 尝试入连 | 三面均拒绝：announce peers 吸纳过滤（不进候选）、拨出过滤、入连 accept 后立即拒绝（fixture 侧观察连接被关） |
| QA-AL-08 | 10-bt-anti-leech-08 | B 键手动断开 1 连接 + 随后自动封禁 1 peer | 两条目共存（reason=manual 与自动原因）；重启后均生效；B 键 toast `✂ 已断开 <ip> 并加入黑名单（剩余 <n> 条）` 计数含自动条目 |

## 通过准则

- 全部用例结果记 P（通过）/ F（失败）/ S（受限跳过）；S 必须内嵌环境判据文案。
- 黑名单文件断言 = 字节级读回 blacklist.yaml（yaml 解析或文本含条目均可）；封禁即时落盘
  判据以触发后轮询（≤1s 间隔）读到为准。
- 误封防护回归：QA-AL-02 为反向判据（做种豁免），FAIL 即策略设计缺陷，不归环境。
