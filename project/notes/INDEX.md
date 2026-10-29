# 项目经验索引（notes/）

- `architecture.md` — 任务启动必须经槽位单点路径，协议分流集中在 tick 层（BT 误走 HTTP 管道的根因与收口模式）；速度统计的喂窗义务在协议事件路径、采样点只读不喂（BT 速度恒 0 的根因与不变式）；TUI 退出路径输入排空窗保持 raw/ECHO-off 到 termios 恢复前最后一刻（含 crossterm EventStream 持锁假阴性陷阱）；抄写 ezr-demo 组件时键路由层必须成对接入（text_backspace/on_dialog_key/on_paste 三张分发表）；BT 全局黑名单挂 EngineShared（per-run 状态活不过暂停）；BT 下载速度优化五件套（nodelay/pipeline/全局认领集/在飞超时/上传定向读）
- `testing.md` — proptest 随机种子敏感与 f64 守恒断言容差；BT 路由测试夹具与引擎 spy 用法；tokio File flush 可见性、select! 取消安全与读写分离、同源 fixture 盲区、pty 驱动 pump 纪律；速度滑窗单测写法（回拨 last_speed_tick + 两次 push）；挂载套件自动收编对账口径；双测试二进制并发假 FAIL 判据；pty 复现终端态残留 bug 的 oracle 口径（回显数 + bash 会话首进程拓扑）；引擎级黑名单 e2e 测试（shared 复用 + 轮询断言 + 二次 run 模拟续跑）；全局认领集的借用拆分（先算 claimed 再 get_mut）
- `environment.md` — 沙箱构建环境：rsproxy(registry)/TUNA(rustup dist) 镜像选型、后台进程回收与分块构建、小盘清理（含 deps 测试可执行文件第二层空间 + 本轮二次复现处置）、终端显示伪影；pip 阿里云镜像；nightly 装成与 fmt 补跑口径；harness tracker/seeder 会话级生命周期与死 tracker 判定路径
