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
