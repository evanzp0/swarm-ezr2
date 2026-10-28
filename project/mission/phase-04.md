# 04 · 传输优化与种子规范补全（第四期详细需求）(v0.1 计划登记稿)

> 期号：**04**（分期模式；需求大纲见 `../mission.md`）。
> **状态：计划登记稿**——操作者 20261027 指令提供的 phase4 实现计划，仅作分期登记；
> 详述条款（FR-04-xx / D 值 / 验收口径）由首角色在 phase-04 开工会话中依操作者输入补写定稿
> （宪法第一章），**phase-02 不处理本文件任何条款**。
> 承接：phase-02 已交付 TCP 下载基线，phase-03 已交付 DHT/扩展协议基线。

## phase4 实现计划（操作者钦定，20261027）

- **BEP 29（uTorrent Transport Protocol / uTP）**：在连接建立逻辑中加入
  「**先试 uTP（UDP），失败则回退 TCP**」（操作者钦定的实际实现口径）。
  （并发连接面板「协议」列的 `uTP` 形态随本期落地——phase-02 D51 预留承接。）
- **BEP 47（Padding files and extended file attributes）**：为多文件种子定义
  padding 文件与扩展文件属性机制。
  （核证注记：操作者指令原文将本条标注为「BEP 29」；padding files/extended file
  attributes 的现行 BEP 编号为 **47**，uTorrent Transport Protocol 为 BEP 29——
  两条均为操作者钦定实现计划，编号勘误不影响条款内容，登记备考。）
