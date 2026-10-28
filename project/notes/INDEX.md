# 项目经验索引（notes/）

- `architecture.md` — 任务启动必须经槽位单点路径，协议分流集中在 tick 层（BT 误走 HTTP 管道的根因与收口模式）
- `testing.md` — proptest 随机种子敏感与 f64 守恒断言容差；BT 路由测试夹具与引擎 spy 用法；tokio File flush 可见性、select! 取消安全与读写分离、同源 fixture 盲区、pty 驱动 pump 纪律
- `environment.md` — 沙箱构建环境：rsproxy(registry)/TUNA(rustup dist) 镜像选型、后台进程回收与分块构建、小盘清理、终端显示伪影；pip 阿里云镜像；nightly 装成与 fmt 补跑口径
