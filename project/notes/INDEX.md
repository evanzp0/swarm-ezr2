# 项目经验索引（notes/）

- `architecture.md` — 任务启动必须经槽位单点路径，协议分流集中在 tick 层（BT 误走 HTTP 管道的根因与收口模式）；速度统计的喂窗义务在协议事件路径、采样点只读不喂（BT 速度恒 0 的根因与不变式）；TUI 退出路径输入排空窗保持 raw/ECHO-off 到 termios 恢复前最后一刻（含 crossterm EventStream 持锁假阴性陷阱）
- `testing.md` — proptest 随机种子敏感与 f64 守恒断言容差；BT 路由测试夹具与引擎 spy 用法；tokio File flush 可见性、select! 取消安全与读写分离、同源 fixture 盲区、pty 驱动 pump 纪律；速度滑窗单测写法（回拨 last_speed_tick + 两次 push）；挂载套件自动收编对账口径；双测试二进制并发假 FAIL 判据；pty 复现终端态残留 bug 的 oracle 口径（回显数 + bash 会话首进程拓扑）
- `environment.md` — 沙箱构建环境：rsproxy(registry)/TUNA(rustup dist) 镜像选型、后台进程回收与分块构建、小盘清理（含 deps 测试可执行文件第二层空间）、终端显示伪影；pip 阿里云镜像；nightly 装成与 fmt 补跑口径；harness tracker/seeder 会话级生命周期与死 tracker 判定路径
