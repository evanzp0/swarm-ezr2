# 测试经验

## proptest 随机种子敏感：f64 守恒类断言写相对容差
- **现象**：`prop_throttle_bucket_step_invariants` 在无任何相关代码改动的情况下突然失败
  （预算守恒 1-ULP 差：`487326933.1084516` vs `487326933.10845166`），复跑又时过时不过。
- **处置**：把 `taken + tokens_after == available` 改为相对容差断言
  `|taken + tokens_after - available| <= |available| * 1e-9`，复跑 3 次稳定绿。
- **根因**：proptest 每次运行随机种子，新增测试会改变进程内用例执行序与 RNG 流，命中
  此前从未抽到的极端输入（`remaining/available ≈ 0.448` 不满足 Sterbenz 引理适用域，
  `clamped - taken` 舍入一轮）；f64 精确相等守恒在极端量级下本就不可满足。回归基线 0F
  结论要意识到属性测试的种子敏感性——新增/删除测试可能"翻转"既有属性测试的命中面。

## BT 路由测试夹具：种子文件走 testenv 唯一目录
- 写 BT 启动路由类单测时，.torrent 夹具用 `crate::model::testenv::uniq_tmp_dir` 建唯一
  目录落盘（并发套件收编时避免路径碰撞），用例结束 `remove_dir_all` 清理；引擎 spy 用
  `EngineHandle::from_sender`（cfg(test) 构造器）替换 `app.engine` 断言收到的 Cmd。

## tokio::fs::File 写后即读必须 flush：write_all Ok ≠ 数据落 OS
- **现象**：BT 引擎 BtDone 即发后，外部立刻读产物文件偶发读到全零（约 1/30 概率），
  单元测试 30 连绿也无法排除。
- **处置**：piece 落盘路径 `seek → write_all → flush().await` 三步全查错，任一失败即
  中断不标记完成（顺带修掉原代码 seek 失败被静默吞掉的隐患）。
- **根因**：tokio 1.53 `File::poll_write` 把数据拷入内部缓冲（上限 2MB）后**立即**返回
  Ok，真正 write 系统调用由 `spawn_mandatory_blocking` 异步执行；官方文档明确 drop 不
  flush。任何"写完马上读/校验/发完成事件"的路径都必须显式 flush。

## tokio::select! 里放非取消安全读循环会丢字节：peer 连接必须读写分离
- **现象**：与真实客户端（libtorrent 流式连发 block）互通时连接必死，错误为
  「消息超长：<随机大数>」（帧错位）；自研一问一答 fixture 下永不复现。
- **处置**：peer_pump 拆 reader/writer 两个任务——reader 独占读半部裸循环 read_msg
  （永不取消），writer 只管出站；writer 退出 drop 写半部并限时 5s 收割 reader。
- **根因**：`read_msg` 内含多段 `read_exact`，被 select! 在"部分已消费"状态取消时，
  已从 socket 读走的字节随 future 一起丢失，下一帧把 piece 载荷当长度前缀。出站消息
  （request/have/interested）与入站流几乎持续并发，碰撞概率极高。教训：**同一条流的
  读循环绝不放进 select!**（channel recv/accept/定时器是取消安全的，read_exact 不是）。

## 同源 fixture 是互操作盲区：piece_len==单块长度的测试种子掩盖拼装缺陷
- **现象**：phase-02 交付时 5685P/0F 全绿，但真实 .torrent（256KB piece = 16 块/piece）
  下载零进度、死循环重取同一批块。
- **处置**：新增 `engine_downloads_multi_block_pieces`（64KB piece = 4 块/piece + 尾块
  部分）先红后绿；互操作夹具（project/harness/）用 torf 造真实规格种子常态化回归。
- **根因**：自研 fixture 的测试种子 piece length 恰等于 REQUEST_BLOCK_LEN（16KB），
  "整 piece 单块到达"的特例路径掩盖了多块拼装缺失。写协议层测试时，**夹具参数要覆盖
  真实世界的常见规格**（piece 256KB 是主流），不要只用等边特例。

## pty 驱动 TUI 的等待循环必须持续 pump，否则会人为冻死被测程序
- **现象**：夹具驱动 ezr 时 UI 永远停在首帧、事件"丢失"，疑似产品 bug；单独诊断时
  一切正常。
- **处置**：等待观察期间每轮 `pty.pump(0.3~0.4)` 再读屏。
- **根因**：ezr 把 `terminal.draw` 与 `app.tick()` 放同一 async 任务——pty 无人读时
  内核缓冲（约 64KB）写满，draw 阻塞即整个 TUI 冻结；引擎等后台任务不受影响（会呈现
  "引擎在跑、UI 不动"的假象）。另外 pty 缓冲写满属于普适问题：任何驱动 TUI 程序的
  脚本（QA harness、互操作夹具）都不得在观察窗内停止读 pty。

## 速度滑窗喂窗的单测写法：回拨 last_speed_tick + 两次 push
- App 速度采样受 `last_speed_tick` 节流（SPEED_TICK=1s）而 `Instant` 不可注入——测试直接
  回拨字段 `app.last_speed_tick -= Duration::from_secs(2)` 再调 `tick_speed_and_session`
  （App 私有字段对 app 模块子树内的测试可见；tick_speed_and_session 为可测性升
  pub(super)，与 sample_conn_speed_display 同例）。
- SpeedWindow 单样本 rate=0（front==back 同一样本），断言非 0 速度需两次 push 不同累计
  值；两次 push 近同时时 dt 被 `.max(0.05)` 钳制、rate 被放大——断言取 `> 0.0` 口径，
  不锚定具体数值。

## 挂载套件自动收编产品源测试：对账按「每挂载套件 +N」计
- 产品源（如 src/app/**）下的 cfg(test) 用例会被挂载同一源文件的套件各自重复运行：
  挂载 app/mod.rs 的 7 个套件（engine_backfill/g_appcore/g_engine/g_supervisor/g_v147/
  ui_backfill/v115）对 app 子树新用例各 +N。全量对账公式：bin ezr +N、7 挂载套件各 +N、
  不挂载该源的套件计数不变；反向纪律：改 app 子树共享源码后这 7 个套件必须重跑，
  只跑 bin ezr 不构成回归口径。

## 双测试二进制并发跑的假 FAIL 判据与重跑口径
- **现象**：一条命令并发跑两个挂载套件（--test A --test B，cargo 并行起两进程）时，
  g_v147 `conns_match_quota_with_unique_ids` 失败（fixture 任务在观察窗内转 Failed，
  错误详情未被断言消息捕获）；该套件单独全量跑 553P/0F，失败用例单测复跑亦绿。
  2026-10-28 复现同族一次：批量串行连跑刚结束重构建后，`engine_conns_quota_exact_and_unique_ids`
  两连败且签名变体（连接视图得 0 行而非预期 4 行），单测 ×3 绿 + 整套件单跑 553P/0F 绿。
- **处置**：按「失败用例单测复跑 + 全套件单独重跑」两步确认，三证齐（仅此一例 FAIL、
  单测复跑绿、全套件单跑绿）判环境性假 FAIL 并修正结果，不追产品根因；重跑一律
  一条命令只带一个 --test。
- **根因**：并发二进制/重构建刚结束的机器负载升高（fixture 均 127.0.0.1:0 临时端口，
  非端口冲突），真实网络栈时序被拉宽，任务在固定观察窗内走入失败路径——与分块验收
  块首竞态同族。断言失败信息宜附 t.error 便于归因（改进建议移交，不越权自行加）。

## pty 复现终端态残留 bug：oracle 用回显数而非退出后 FIONREAD，且 bash 必须当会话首进程
- **现象**：用 pty 复现「q 退出乱码」时，子进程（ezr）退出后测 tty 输入队列
  （FIONREAD）恒得 0——伪绿；而实际用户屏幕上鼠标事件字节被行规程 ECHO 上屏。
- **处置**：oracle 改为统计 master 输出中 `?1006l` 之后出现的鼠标事件字节数（回显数）；
  复现拓扑用 `pty.fork()` + `bash -c '<被测程序>; sleep 3'`（bash 当会话首进程持有
  tty，父进程另在子进程存活时经 /proc/<pid>/fd/0 解析 slave 路径开 fd 防 tty 解绑）。
- **根因**：pty slave 最后一个 fd 关闭（会话首进程退出）时内核冲刷输入队列——直接
  exec 被测程序时它自己就是会话首进程，退出即冲刷，事后 FIONREAD 测不到任何东西；
  真实场景 bash 才是会话首进程，被测程序退出后残留字节仍会被读/回显。pyte 渲染后
  再搜中文 needle（ratatui 逐 cell 绘制时 SGR 序列穿插在字间，原始字节流中连续中文
  未必存在）。
