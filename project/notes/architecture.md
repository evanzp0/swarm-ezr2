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
