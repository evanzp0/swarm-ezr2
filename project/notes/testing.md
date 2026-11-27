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

## 引擎级黑名单 e2e 测试：shared 复用 + 轮询断言 + 二次 run 模拟续跑
- 引擎行为测试三件套：fake UDP tracker 返回 `interval=1`（真实 tracker 给 60s，测试内
  等不起二轮 announce）；seeder 用 loop-accept + `Arc<AtomicUsize>` 计数（连接数是黑
  名单门禁的唯一外部可观察量，seeder 会话体独立成 `serve_seeder_session` 供逐连接
  spawn——内层 spawn 捕获引用不 'static，数据要 `Arc<Vec<u8>>` clone 进每个连接）。
- `BtTaskCmd::Disconnect` 由引擎异步消费，发送后**不能立即**断言
  `shared.is_blacklisted`——轮询（20ms 步进 + 截止）等黑名单落位；「暂停→继续仍拒绝」
  用同一 `Arc<EngineShared>` 二次 `spawn(run(...))` 模拟，断言计数不涨 + 收到
  `Evt::BtPeers`（tracker 正常）但无 `BtConns` 非空（连接被拒）。

## issue_requests 需要「全局认领集」时的借用拆分：先算 claimed 再 get_mut
- `issue_requests` 要看全部连接的在飞块（防多 peer 重复请求），又要 `&mut` 目标连接的
  stat——同一 HashMap 不能同时可变/不可变借。拆两阶段：`claimed_blocks(conns, partial)`
  先产出 `HashSet<(u32,u32)>`（借用即还），再 `conns.get_mut(&cid)` 传 `&claimed` 进
  issue。调用点（handle_peer_msg 顶部、on_block 尾部、tick 清扫后逐 cid）都遵循
  「先算集、再取连接」；tick 清扫循环内每个 cid 重算一次（前一 conn 的补发会改变
  claimed）。

## 引擎级 sidecar 生命周期测试的确定性写法：预填 sidecar + 立即 Pause
- 引擎下载速度在 loopback 上不可控（1.5MB 可能亚秒完成），「先等进度再 Pause」会与
  BtDone 竞态。确定性做法：磁盘预写完整内容 + sidecar 预填一半 piece → 引擎启动时
  磁盘校验通过半量 → **立即**发 Pause → 断言 BtStopped 后 sidecar.done_pieces 恰为
  预填量。同型可测「启动即写」（轮询文件存在）与「完成即删」（全量预填秒完成）。
- 断言 sidecar 内容时逐字段核对（version/id/name/save_dir/concurrency/data.trackers/
  piece_length/files/source），防止未来结构漂移静默破坏续传。

## fake seeder 会话必须容忍本端任意协议消息（`Ok(_) => {}` 而非 `_ => return`）
- **现象**：`serve_seeder_session` 用 `_ => return` 兜底——本端连接后先发 bitfield，会话
  立即退出；引擎按候选池退避重连 → 再退出的死循环，e2e 下载测试 30s 超时（旧用例未暴露
  是因为它们只断言连接数不断言下载完成）。
- **处置**：fixture 消息循环只对「关心的请求」响应（Request → 回块），其余 `Ok(_) => {}`
  吞掉，仅 `Err(_) => return`；与 `fake_seeder` 的既有口径一致。
- **根因**：BT 握手后本端必然发 bitfield（有数据时）——「对端只该收 Request」的假设
  不成立；fixture 的消息循环必须按协议现实（对端消息集不可枚举）设计兜底分支。

## 引擎 e2e 的 peer 状态断言用轮询计数（AtomicUsize）而非事件推断
- seeder 连接次数是连接行为的唯一外部可观察量：`counting_seeder`（每 accept 计数 +
  逐连接 spawn 会话）+ `fetch_add` 返回值区分首个/后续会话（flaky fixture：首个会话
  直接 drop 模拟掉线）。断言「重连」= 计数恰为 2 + BtDone；断言「多 peer 并发连接」=
  多 seeder 计数各 ≥1 的轮询等待（deadline 内 sleep 50ms 步进）。
- 退避时序入账：候选重试冷却 5s×2^fails →「断开后重连」e2e 至少 ~12s（首轮 2s announce
  + 连接失败 + 10s 冷却），deadline 给 30s；不要用 5s 级 deadline 测带退避的行为。

## 事件节流后，小负载 e2e 的进度断言必须等到完成后的 tick
- **现象**：引擎逐块事件加 250ms 节流后，`engine_downloads_via_udp_tracker_and_seeder`
  偶发 saw_progress=false——首 announce 的 2s 延迟与 1s tick 节拍对齐，tick 刚刷新
  last_emit，小负载（<250ms 下完）落在节流窗内，BtDone 先于任何 BtProgress 到达；
  负载放大或加探针后时序漂移又通过——典型竞速 flaky。
- **处置**：事件循环不在 BtDone 处 break，继续收到「完成后的 1s tick 必发」的
  BtProgress/BtConns（downloaded=total>0、连接仍在）再退出断言；节流闸门本身用
  纯函数单测钉住（`progress_emit_due` 249ms 拒 / 250ms 放），e2e 不测节流时序。
- **根因**：断言依赖「完成前必有中间事件」的隐含时序假设，而该假设被节流设计
  合法地打破；e2e 断言只锚定设计保证（tick 必发 + 完成事件必达），不锚定实现细节。

## 新旧二进制同工况对照：备份源码 → checkout HEAD → 独立 target-dir 构建 → 恢复
- 速度优化的证据链要有「同 harness 同参数新旧对照」：①先 `cp` 改动文件到
  `.work/tmp/new-src/`（分目录防同名覆盖）；②`git checkout HEAD -- <改动文件>`
  还原旧实现；③`cargo build --release --target-dir .work/tmp/old-target`
  （独立 target-dir，不动现行构建缓存）；④跑同一基准；⑤从备份恢复文件重编。
- 注意：工作树含未提交改动时不要用 `git stash`（会把 harness 等配套改动一起卷走，
  旧二进制跑不了新参数）；只 checkout 明确列出的改动文件。

- libtorrent seeder 限速必须 `handle.set_upload_limit(bytes/s)`：对已建 session
  `apply_settings({"upload_rate_limit": …})` 实测不生效（场景 A 仍跑满 49MB/s），
  两个都设才稳。回环不限速时页缓存全开 ~50MB/s，任何「多 peer 速度」结论都必须在
  seeder 限速口径下取数。
- 多 peer 对照实验的方法纪律：①同机三方（seeder/leecher/ezr 全 127.0.0.1）会触发
  客户端同 IP 去重交互，late seeder 必须绑 127.0.0.2+ 区分 IP；②ezr 进度采样用
  `.ezr` sidecar（引擎 1s tick 落盘的 done_pieces JSON，完成后即删，删除即完成），
  不能用文件大小（BT 预分配 set_len）；③终帧只有完成后状态，定位引擎级卡点要在
  下载中途每 5s 落盘屏幕快照（screens.log）对照 leecher 实时状态
  （handle.status() 的 payload rate）——「0 进度 + 并发 1 指向空 peer」这类结论只能
  从中途快照得出。
- 小盘跑 12 个 hardening 套件必爆盘（每测试二进制 ~200MB × 12 ≈ 2.5G）：全量套件
  编译运行加 `CARGO_PROFILE_TEST_DEBUG=0`（二进制缩到 ~25MB，断言与运行时行为不变，
  仅丢回溯符号），配合 `CARGO_INCREMENTAL=0`；编译用 `--no-run` 分块（crate 级产物
  跨块累计），运行按二进制逐个前台跑、只取 `test result` 行。
- 规格钉定列宽的变更必须同步 feature 文件引用：`04-ui-conns.feature` L128 钉死
  「IP(11)/…内容宽 59 列」，bug10 按操作者指令改为「地址(15)/内容宽 63」——feature
  文件属 specifier 职责未改，QA 复核时以 handoff 登记的偏离为准（操作者指令级偏离）。

## 钦定模板文本的 runner 解析锚点：改模板必须同步三处断言
- `suite_config_template.py` 不内嵌模板——运行时从 `features/01-config-template.feature`
  注释块解析钦定原文：块起点 = 以「specifier v<N.N>）：」结尾的说明行，块终点 = 独立「）」行，
  内容行统一剥 2 空格缩进。改模板（v1.10→v1.11 54→168 行）必须同步：①起点锚版本号
  `specifier v1.10）：` → 新版本；②`len(body)` 断言数字；③键行抽验清单（新增键行加入）。
  陷阱：示例块行数计入块体（`# [[common.proxy]]` 表头行算 1 行，双示例块各 7 行不是 6），
  行数手工推算容易少 2。

- **runner 不可执行期间，钦定原文 ↔ 代码常量会静默漂移**（D91 落地轮实测三重漂移）：
  代码侧模板单测只钉 counts/substrings（键数/行数/取值范围计数/键行存在），不做逐字节；
  cmp 逐字节只在 runner（QA-CT-01）里——runner 未可执行的沿袭期里，v1.51 起的漂移无人
  发现：[http] 键序不同（钦定 = scheduler/min_split 前置，代码 = block_size 前置 + 两键
  后置）、block_size_http 注释停留在 v1.16 文案（下限（1 MB）未随 v1.51 256 KiB 同步）。
  重建口径：以规格内嵌钦定原文为唯一字节权威，**双路径构建互证**（v1.62 原文减三行 vs
  v1.63 原文减注记行，diff 为空才写入）+ tomllib 解析断言（41 键/默认值/键序）+ 尾部单
  换行。钦定文本内嵌的退役注记行（「（v1.63/族A 退役：…）」）是规格编辑器留痕，不是
  模板行——构建时剔除；QA 轮执行 suite_config_template.py 时注意其解析锚点（起点锚
  「specifier v<N.N>）：」行 + 终点「）」行）对 v1.63 修订注的适配。

## QA 套件 write_config 的键路径随三大表重构整体迁移（13 套件 ~57 处）
- runner 写配置一律顶层平铺（`download_slots = 2`），三大表后这些键全部变成「未知键静默
  忽略」→ 用例假绿（回退默认恰好等于期望值时）或假红。迁移清单（v1.17 登记）：
  suite_slots_queue ×9（→[common]）、suite_throttle_proxy ×8（→[common]）、
  suite_persistence_config ×11、suite_named_proxy ×15（proxies_toml 生成器 [[proxies]] →
  [[common.proxy]]）、suite_ui_conns ×3、suite_tui_display ×2、suite_retry_backoff ×2、
  suite_modify_task ×2、suite_config_template ×2、suite_add_task ×1（→[common]）、
  suite_resume_sidecar ×1、suite_integrity_check ×1、harness.py ×1。机械规则：
  标量键写入 = 对应表头行 + 键值行（同一 [表] 段内多键合并）。

## phase-02 交互变更必须同步 QA runner 帮助层：添加任务唯一入口已改命令窗口

- 现象：QA 套件全线假红「添加对话框未打开」——`add_task_via_dialog` 仍按 phase-01
  口径发 A 直接等「添加任务：HTTP(S)」，而 06-cmd-window（phase-02）后 A = 命令窗口
  （标题「请输入命令或链接」），http URL 经 Enter 分流才开 HTTP 框；页脚 chip 同步改名
  「A 添加」→「A 命令」，空态引导文案改「按 A 打开命令窗口添加任务」。
- 处置：harness 新增 `open_add_dialog_via_cmd(app, url)`（A → 命令窗口 → 输 URL →
  Enter → 等对话框），`add_task_via_dialog` 与 23 处直开点全部改走该 helper（ftp 等
  对话框级校验用例 = 先 DUMMY_URL 开框再 bs 清空预填注入）；chip/空态文案断言同步。
- 根因：phase-02 改交互时只改了产品与 feature，runner 帮助层未随动——**交互类需求
  落地时必须把 QA runner 的入口/文案锚点一并过一遍**，否则全套件假红掩盖真回归。
- 注意：tui_display 有两处特殊入口（页脚 chip 点击、Ctrl+C 浮层循环）需按命令窗口
  口径单独改写；`A 添加`→`A 命令` 的全文替换会误伤「按 A 添加下载任务」类空态
  断言（该文案本身也已改），替换后须逐点核对。

## 状态转场窗口注入按键存在丢键/瞬时窗口：断言要落在稳定可观察物上

- 现象（QA-SQ-13）：`wait_for("已失败")` 后立即 `send("r")`，偶发整套重试流程无响应
  （无 toast 无重排队）；加 0.5s settle pump 后 toast 稳定出现，但「等待中」行仍可能
  抓不到——重排队 → 1s tick 授槽 → probe 404 再败，整链 <1.5s，等待中窗口可能短于
  0.1s 轮询粒度（授槽 tick 对齐时 <100ms）。
- 处置：转场后先 `pump(0.4~0.5)` 再注入按键；断言优先落在长驻可观察物（toast 约 4s、
  终态行），瞬时中间态（等待中）断言天然竞态，判读按 testing.md 假 FAIL 口径复跑。
- 根因：pty 驱动与被测端 1s tick 异步，事件循环在状态切换批次内可能吞掉恰逢其时的
  键事件；产品侧行为经单测与手工复验均正确（toast + 重排队语义在）。

## 配置解析语义变更时，单元测试与 runner 同在键路径迁移清单内
- **现象**：bug11 三大表重构后，`tests/hardening_v115.rs` 的 `config_keeps_positive_block_size_http`
  仍断言顶层平铺键 `block_size_http = 2097152` 被保留——旧格式按未知键静默忽略后该键回退
  默认，断言失败（750P/1F）。
- **处置**：改 `[http]` 表形态断言 + 补旧格式回退负对照（顶层键 → DEFAULT，与
  01-persistence-config-12 口径对齐）。
- **根因**：迁移清单只覆盖了 QA runner 的 write_config（~57 处），单元测试里硬编码的
  `Config::from_toml("裸键 = 值")` 形态漏排。改配置解析语义时必须 grep 全部 `tests/` 的
  from_toml 裸键形态（顶层键断言 = 旧格式残留）。
- **runner 对话框预填的清场辅助（v1.38/D62 起）**：HTTP 添加对话框保存目录/URL 字段打开即预填配置链值——runner 一律经 `clear_dir_prefill`/`clear_url_prefill`（harness.py，一次写 64 个 `\x7f` + 单次 0.06s pump，空字段上 bs 为无害 no-op）清空后再输入显式值；留空确认 = 走预填（恰为原「留空走默认」语义）。**清场节奏是时序敏感面**：逐键 0.008s pump 的 64 连发会把对话框停留时间拉长 ~0.5s，曾放大 QA-PC-06 的「worker 重开文件失败」竞态窗口（详见 architecture 同日条目）；一次写整段 + 单 pump 后 3 连跑稳定。**预填字段断言按尾部可见段**：字段超宽左截断显示（`…`+尾部），`assert_in(完整路径)` 必假红——用 `cfg_dir[-16:]` 这类尾部锚。**D44 迁移残留修复**：QA-01-07（坏 URL 现被命令窗口预过滤，对话框级校验可达路径 = 对话框内改写 URL 字段）与 QA-01-17（粘贴前先清 URL 预填，防 300 上限截断拼接）——凡「直开对话框」时代的用例迁移到命令窗口分流后，都要重推一遍「字段预填 + 光标在尾」前提。
- **A/B 归因先核对 runner 与二进制的组合矩阵**：本轮曾把「旧 runner + 新二进制」的失败（预填拼接 → 下载落错目录 → sidecar 找不到）误读为产品回归——stash/pop 序列中 runner 与 binary 的版本组合极易错位；归因前先确认（binary mtime vs 源码 stash 状态、runner 是否含本轮 helper）。对照法同 testing.md 既有「新旧二进制同工况对照」，但必须连同 runner 一起锁版本。
## bug14 轮（specifier）
- 模板改版（v1.13 活跃键值行）的 runner 锚点三处同步：块起点锚 `specifier v1.12）：` → v1.13（len(body)==174 不变）；表声明行断言 `# [common]` → `[common]`；键行抽验 `# <key> = <值>` → `<key> = <值>`；「每条非空行以 # 开头」全注释断言退役，新增 tomllib 解析等价断言（解析值=代码默认逐键）。
- QA-CT-03 行为翻转的 runner 驱动方式变化：从「启动并 pump 画面」改为「启动收退出码 + stderr 后收尸」（损坏配置现期望非零退出）；负向探测单发，不重试。
- TestBackend 探针直读 buffer 比字符串比对可靠：terminal.backend().buffer() 按 (x,y) 取 symbol，可精确定位宽字边界处的渲染结果；别依赖 to_string() 的行形态（含坐标注记，断言锚易错）。

## bug14 轮（coder 实现）
- TestBackend 断言两形态差异：backend().to_string() 会压缩宽字符跳格（「协议」可直接 contains），裸 buffer 遍历（cell by cell push symbol）则宽字后带跳格单元（「协 议」空格形态）——断言宽字完整性用 to_string() 行匹配（套件既有模式），别混用两种形态的期望串。
- QA-PC-04① 语义翻转的 runner 改法：类型不匹配（字符串进数字键）改断言「进程退出 + exit_code!=0 + 报错含『配置文件解析失败』」，用 app.exit_code 轮询（EZR 致命臂在 TUI 之前退出，stderr 直写 PTY 可断言）；语义非法值腿（0/256KB/-1）维持启动+逐键回退断言。
- throttle 套件单调用装不下（TP-02/03 各 ~210s）：`python3 -c` 外部驱动 import 套件类、只调 `_case_05..07` 补跑尾部（原文件零改动），与整跑日志拼对账。
- MT-03（重启应续传）本轮 2 连败与 bug13 登记一致（基线同败、A/B 已证与二进制无关、本轮续传路径零改动）——沿袭已知项，QA 轮 refinement 归属不变。
- demo pty 验证脚本（verify_ezr.py）的表头查找断言隐含列宽假设：N5 查找「下载速度+累计下载」同行在 44 列视口下依赖累计下载单元格（37..45 列）完整可见——bug11 列宽改版（内容 59→86/94）后该单元格被视口右缘裁成「累计下」，查找 not found 连锁 N5b/N10/N11 四项假 FAIL（A/B 对照 max(12)/max(26) 同败已证与 D64 无关）；列宽改版轮必须同步核对 demo 脚本的视口可见性断言（查找锚应选列起点不受 client_w 影响的列，如「地址/下载速度/上传速度」，或放宽滚动次数到 hmax）。
- pyte 是 demo pty 脚本依赖（本沙箱 pip 阿里云镜像装 0.8.2 即可）；跑单场景 `python3 scripts/verify_ezr.py n`（参数=场景字母过滤器），release 二进制先 cargo build --release（增量 18s）。

## bug16 轮（specifier）：binary-only crate 的 spy 探针法与临时测试插桩还原纪律
- 本仓无 lib.rs（binary-only），tests/ 集成测试访问不了内部——行为探针只能做 crate 内
  测试模块（cfg(test)）。
- 探针插桩纪律：改前 `cp` 备份原文件到 `.work/tmp/`；探针测试以 `panic!` 携带诊断载荷
  （命令序列 fmt 列表）主动失败输出证据；跑完 `cp` 还原并 `git diff` 复核零残留。
- spy 引擎命令序列断言模式：`EngineHandle::from_sender(spy_tx)` 替换 app.engine，逐条
  `timeout(recv)` 收集 `Cmd` 的 Debug 串——比逐命令断言更适合「发生了什么」类归因。
- APS dry-check 增量口径：改 feature 前先对 `git show HEAD:` 版本跑 parser+dry-checker
  存基线，改后 diff findings（同类配对增减），套件级增减才是本轮净效应（本轮 08：
  29→28，03：8→8）。

## bug16 轮（coder）：BT 引擎多 seeder 夹具三纪律 + 磁盘满复现
- **互异 peer_id**：多 seeder 夹具每个实例必须用不同 peer_id（serve_seeder_session_as 支持传入）——同 peer_id 会被引擎「单 peer 单连接」去重臂正确断开，tracker 周期 announce 又把候选加回，形成「拨号→dup 断开→重拨」死循环（探针 Closed reason=引擎断开 连锁复现）。
- **0.0.0.0 绑定**：多回环地址（127.0.0.2–4）拨号测多连接时，seeder 必须 bind 0.0.0.0:0（counting_seeder_any）——绑 127.0.0.1 的 listener 不接受 127.0.0.2 目的连接（连接拒绝假象）；沙箱 127/8 全段可连（python 探针实测）。
- **piece 数学**：done 集 × selected_bytes_of_piece 的重算断言要按 piece_length 跨文件边界推演（a=100+b=100、块 64：done{0..3} 全选=200、b 取消后=100=64+36）——别想当然按文件大小直加。
- 磁盘 100% 二次复现（集成目标链接期 Bus error + No space left）：清 target/debug/incremental（3.2G）+ 全程 CARGO_INCREMENTAL=0 再跑 13 目标（累计 ~10 分钟逐目标前台跑，禁 --tests 一把梭）。

## bug17 轮（coder）：会话级单例与生命周期测试纪律
- 会话级 peer_id（OnceLock）使同进程所有引擎共享 peer_id——既有夹具固定 -QX7777- 前缀与自连守卫（peer_id 全等才拒）互不冲突，21 处测试 spec 构造点零改动；新测试断言「跨 run 全等」而非「跨 run 互异」。
- 暂停收口判据轮询 seeder 探针的 close_kind（deadline 10s），断言 `contains("eof") && !contains("reset")`——不要断言具体 io 文案（tokio/std 文案不同且版本可漂移）。
- 自关冷却测试的时序锚：冷却锚定在 **shed 时刻**（last_try 刷新点）而非 grow 时刻——grow 在 shed 后 ~300ms 才发送，断言阈值按 shed→established ≥ 700ms（base=1s tunable）留 300ms 偏斜余量；被收缩候选是唯一非连接候选（同 IP 去重过滤另一在连候选），重拨路径确定。
- 先让连接成熟（sleep 1.5s > base）再收缩：否则旧 last_try 未过期，旧实现也天然带冷却，红测失效。；bug19 轮：回归归因用 git worktree add /tmp <HEAD> + 独立构建跑同一脚本对照（失败集合 HEAD 与工作区完全同形 = 环境预存非本轮回归；比 stash 快且不动工作区，用后 git worktree remove --force）；PTY 验证脚本几何断言随对话框加字段整体平移（trackers 行号 tr_y 每新增一字段行 +1）；焦点表膨胀后 Tab 步进数不稳，改鼠标点击面板聚焦（click → focus_field）更稳

## bug18-coder 轮

- 对话框焦点矩阵变更（新增字段/nfocus 变化）的测试同步面实测清单：①dialog_keys 走焦类测试的 Tab 步进数与终态 focus 断言（每测试单独数步，勿批量替换）；②text_field_at/text_backspace/text_char 的 (kind,focus) 矩阵断言与 proptest 的 is_text/is_bs_field 矩阵（两处独立硬编码，漏一处假绿）；③paste 分发矩阵（dlg_apply_paste 焦点入参）；④mouse 对话框热区测试的 dlg_field_rects/dlg_btn_rects 登记下标；⑤确认链按钮序号（dlg_activate_* match 臂）；⑥挂载套件（hardening_v115 等几何 exact 断言）的 y 平移 = dh 增量/2（居中对话框 y = (H−dh)/2，行号与按钮行 y 同步平移、下拉框锚点 y 随所属行平移）。
- TestBackend 宽字残影新形态：对话框 y 平移后若标题行与底层列表宽字行重叠，buffer 内出现「标题宽字与底层宽字逐格交错」（两套宽字各占奇偶格），to_string 与 buffer 直读都呈现交错串——文本 contains 断言在这种行上不可靠，改用 ASCII 连续子串锚点（如标题里的 "HTTP(S)"）或 ASCII 邻域探针。幂因：ratatui diff 对空符号（宽字右半格）跳过回写，后端缓冲残留前帧字符。
- 压缩口径分工再确认：to_string()（压缩宽字、保文字序列）适合 contains 文本断言；裸 buffer 直读适合几何/单格字符断言，但对宽字第二半格返回空格而非空串——跨宽字的子串匹配只能走 to_string。
- 引擎封禁类 e2e 夹具：探测 peer 常驻 accept 循环 + 行为闭包（Arc<dyn Fn(TcpStream)->Pin<Box<Future>>>），观察面用「轮询 evt_rx 收 Evt::Toast(⛔) + shared.is_blacklisted(ip)」双证据；fake peer 等 Unchoke 的陷阱 = 引擎对同一连接只发一次 Unchoke（am_choking 置位），等多次 Unchoke 的夹具永远凑不齐触发次数——首个 Unchoke 后立即发齐全部违规帧。bad_piece 夹具 = corrupt_seeder（bitfield 全有 + request 应答纯垃圾数据），依赖引擎坏片重下机制自然凑满 3 次。
- 挂载套件对账：src 内 #[cfg(test)] 新增测试会被全部 hardening_* 挂载套件自动收编（每套件 +N 重复计数），总账对账按「bin 增量 × 套件数 + 套件自有增量」复算，勿把收编增量误记为独立新测试。
- 时序 flake 复跑口径沿 bug17 轮：单套件单点 FAIL（v115/v147 各 1 次）原样复跑全绿即判环境性时序，非回归；不要为 flake 放宽断言。

## bug21 轮（coder）：输入字段三类交互的测试口径
- **满容量重启录入（数字封顶字段）**：容量封顶的数字字段（并发 2 位）语义 = 光标在字段末尾且已满时键入数字 → clear+insert（"键入即替换"）；光标在中间位置不重启不插入。单测须同时覆盖：满容量末尾重启、满容量中位不动、非数字拒收；预填值（BT 默认 20 恒满容量）就是触发面——旧实现 insert 容量满静默返回 0，HTTP 默认 4（1 位）未满故仅 BT 显现「并发无法修改」。
- **限速输入规范化三件套缺一不可**：①insert 后全缓冲 canonical 化（二进制单位 + 前导零剥离）；②canonical 完成态对续入 `b/i/s/` 吸收（否则 `512k`→`512 KiB/s` 后补键 `b` 变残渣 `512 KiB/sb`）；③退格不回写规范化（否则删 `/s` 尾缀被立刻补回，永远删不掉）。测试锚点：`512k`→`512 KiB/s`、`512 kb` 逐字符、`1 M`、纯整数原样、退格后不回写。
- **multiline 接缝光标的行为耦合**：合并行光标从「合并行行尾」改「接缝（前一行长度处）」后，行中插入内容的右侧残留字符退格不可达（光标左侧语义）——任何依赖「退格连发清空缓冲」的 PTY 脚本计数语义随之改变；脚本清屏须先 End（`\x1b[F`）归位缓冲末尾再退格（verify_phase2_ui B-TR 实证：旧行尾光标 10 次清空，接缝光标下 `tr1⏎trY2` 行中 Y 右侧的 2 残留 → 先 End 再 12 次退格）。改光标语义 = 全量排查 PTY 脚本的退格清屏流。
- 焦点矩阵变更（HTTP Add 11→10 / Modify 8→7）同步面 = 六处登记点（text_field_at/text_backspace/text_char/mouse ck_i+proxy_i/下拉 Tab 跳焦/nfocus+enter/char 处理器）+ 单测断言 + TestBackend 几何断言（行 y 坐标、按钮 y、下拉锚点 y）+ property 测试字段矩阵；几何断言按新布局重推（对话框高度变 1 行时居中 y 与内部行全变，部分断言因居中取整恰好不变——逐条重推勿抄旧值）。

## bug21 修订轮（specifier）：多轮几何变更后的 QA 用例行对账口径
- 现象：QA-AB-01（78×26/九段布局无限速字段）与 QA-MB-01（78×24/九段）滞留在 v1.38/v1.5
  之前的形态——bug18 轮只改了套件头部「对应规格」行，漏改用例表行；07-add-bt 场景清单
  缺 09/10 两条目（v1.37/v1.38 新增场景未回填清单）。
- 处置：规格几何/字段面变更轮的对账面 = ①feature 场景清单注释逐条与 Scenario 行数核对；
  ②QA 套件「对应规格」行 + 每条涉及几何/字段/焦点的用例行逐一核对（头部同步 ≠ 用例同步）；
  ③APS dry-check 基线 diff 只验步骤形态，验不出「用例行陈述旧几何」——几何数字必须
  人工/rg 扫旧值（74×12/78×26 这类字符串直搜）。
- 附：APS 装机复证——bb v1.13.229 静态包 + APS clone 均 gh-proxy.com 一次过
  （与 bug14/18 轮口径一致）；dry-check findings 增减判读：本轮 6 文件
  （158→163/9→9/86→85/45→45/44→38/29→29），仅 possible-synonym/near-duplicate
  类增减（步骤结构变化净效应），无错误类新增。

## bug21 修订轮（coder）：引擎事件观察环的「先落库后发事件」竞态
- 现象：hardening_v147 三处封禁测试（unknown_message_id / illegal_requests /
  corrupt_seeder）满负载并行跑偶发 FAIL（⛔ toast 可观察），单测过滤恒绿、产品代码未动。
- 根因：引擎 ban 路径 blacklist_add_with 先于 evt_tx.send(Evt::Toast) —— 测试环
  「banned 命中即 break」在负载抖动下先翻转 banned 就退出，toast 事件还在通道里没被
  观察。断言的两个可观察物有先后依赖时，break 条件必须等齐全部条件。
- 处置：观察环改为 `if banned && saw_toast { break; }`（deadline 不变，失败时依然
  跑满 10s 才红）；同类「断言多事件 + 中途 break」的测试环写法一律套用双条件。
- 判读口径：单测过滤绿 + 全套件偶发红 + 产品零改动 = 先查测试观察环与事件顺序，
  不要急着归因产品回归（与既有「挂载套件自动收编对账」条目同族）。

## bug22 轮（coder）：字段行光标列/点击 value_x 公式的「标签宽硬编码」缺陷族
- 现象：BT 添加/修改对话框「下载限速/上传限速」聚焦时终端光标压在最后字符上
  （并发框正常）——用户报「无法用退格删最后字符」。
- 根因：`ui/bt_dialog.rs bt_text_row` 终端光标 x 与点击 value_x 硬编码 `+9`
  （= 标签 7 列 + "> " 2 列），但「下载限速/上传限速」标签 4 个 CJK = **8 显示列**
  （`pad_right(s, 7)` 超宽不截断），实际文本起点 = 行首 + 10 → 光标/点击基准左偏
  1 列。**标签宽 ≤ 7 的行（种子/保存到/并发）恰好不触发**——同函数同公式，缺陷只
  在宽标签行显形，属「公式对参数域假设过窄」族。
- 修法：`label_cols = w(label).max(7)` 动态化（对齐 ui/dialog.rs HTTP 对话框与
  ezr-demo 同位点公式）；回归测试用「自校准矩形」写法——渲染后从 `dlg_field_rects`
  反查聚焦行 rect，断言 `dlg_cursor_screen.x == rect.x + label_cols + 2 + w(value)`，
  勿硬编码整屏坐标（居中取整一变全崩）；点击映射补 value_x + 首字符格/末字符格/
  越尾三判据 + 末尾光标退格删最后字符的行为语义断言。
- 对账纪律：修「产品端 vs demo」同构位点时**先审 demo 是否同款**（本轮 demo
  `bt_text_row` 素来动态计算、零改动）——对账结论与守卫测试同样要落 demo 侧
  （cursor_tests + README 变更记录 + 规范文档规则 bullet），防止「修了 A 侧、
  B 侧将来重引入」。
- 集成收编面按**测试所在模块**分流：bin 测试收编进挂载套件不是全集——
  `app::bt_dialog` 测试收编 7 套件（bug21 修订轮口径），`ui::bt_dialog` 测试
  收编 3 套件（engine_backfill/ui_backfill/v115）；对账算术用「基线 + Σ收编」
  核总数，别套用上一轮的收编套件清单。

## bug23 轮：假 peer 请求日志（反吸血断言）与恢复单测的确定性写法
- **wire 级「没发过某请求」断言 = 假 peer 收集 (index,begin) 到共享 BTreeSet**：
  fake_logging_peer 循环 accept（跨阶段重建连接）+ Request 全量入账 + serve 集合
  控制服务行为；「未重请求恢复块」的断言放在**任务完成之后**（sidecar 移除 = 全部
  请求已发生），不用时间窗内轮询（无法证明「将来也不会发」）。
- partial 形成的确定性信号不存在（BtProgress 只在整 piece 完成时发、partial 块
  无事件）：用「假 peer 服务了 (0,0)」的 served 通道信号 + 1.5s settle 再 Pause；
  断言 sidecar partial 非空本身就是竞态探测器（若 flaky 说明 settle 不足）。
- restore/save 纯函数单测不用网络：restore_sidecar_partial 直接喂
  BtPartialPiece 列表 + 真 PieceMap + open_selected_file_handle；**读盘断言用
  std::fs::read 而不是句柄 read_to_end**（写后句柄 seek 停在 EOF，读回恒空——
  曾据此误判「没写盘」）；「未落盘」断言注意句柄打开即 set_len 预分配（稀疏
  全量零填充），应断言内容全零而非文件为空。
- 私有字段构造绕法：BtPartialBlock.data_b64 私有，引擎测试里构造坏 b64 条目走
  serde_json::from_str 反序列化（derive 的 Deserialize 不受字段私有影响）。
- ezr-demo 的 sel_task() 走过滤视图（selected 下标 ≠ tasks 下标）：demo 交互测试
  打开对话框后用 **d.task_id 回查 tasks 位置**再断言，别假设 selected 与 tasks
  同序（本次 idx=2 打开的是 id=4 的任务）。
- 测试内文件读断言与句柄生命周期：handles.remove(&0) 拿回句柄后原 read_to_end
  方案已废，统一 std::fs::read(path)（写路径 = save_dir/name 平铺）。
- 基于 Instant 的 UI 倒计时断言要**抗秒级漂移**（bug24）：测试进程插入 next_at 与
  渲染函数内部两次 `Instant::now()` 之间可能流逝 ≥1s（test 二进制启动/调度延迟），
  钉具体文案（`下轮 120s`）会随机翻车（实测渲染出 `下轮 119s`）。正确姿势：解析
  「下轮 」后取连续 ASCII 数字段断言**位数 ≥3** + 后随 's' 收尾——既验证「不被
  截断成 '…'」（截断时数字段只有 2 位且无 s），又对任意合法剩余秒数恒真。
- 负载敏感 e2e（真 TCP 桩 + 截止时间）在高负载时段会 2 连败（v147
  engine_conns_quota_exact_and_unique_ids，25s 截止没跑满即断言失败；单跑 0.31s
  即绿）：沿 bug17 轮口径复跑判环境性，但**要先把用例单独跑绿 + 确认改动无接触面**
  再下「非回归」结论；连续复跑仍红时优先怀疑环境负载而非代码。config 键废弃轮
  再证（连续两轮四次 full-run FAIL、单跑绿 ×5 且 0.3s 级）：判法稳定，同用例
  负载敏感实录直接引用，无需每次重查接触面（本轮改动面 parse_speed/CLI 测试
  与连接视图 quota 零交集）。
- **废弃类口径变更的测试翻转面三件套**：删单位/改语义类变更（如 SPEED_UNITS
  移除旧写法）不能只翻单元断言——①rg 全 src 找旧形态引用（**property_tests.rs
  在 bin 目标内**，全量 bin 跑才暴露）；②proptest 生成器里的单位枚举表与派生
  断言（`{n} KB == n*1000` 型）同翻；③CLI 路径补实机探针（构建后直接跑
  `--max-speed "2 MB/s"` 验报错退出 + 文案）。实测：单元翻转后 proptest 2 例
  立即红，漏翻即交付缺口；兼容旧行为的 fixture 字符串（如已退役键的无关值）
  不在翻转面内，键已废弃时值串不参与解析。
- **标志列轮 coder：ezr-demo 文档计数登记以源码实测为准**：demo 帮助行数在
  规范文档 §6.6 登记 68 行，历轮条目增量（v1.35–bug24 各轮加条目）从未同步
  计数，源码 `help_lines().len()` 实测基线实为 76——计数类登记（行数/条目数/
  列宽合计）直接以源码常量或运行时实测回填，测试断言同步钉实测值；同步轮
  顺序 = 先实测 → 再写 docs/README → 断言钉同一实测值，避免「文档钉旧登记、
  测试钉猜测」双源漂移。另：demo 渲染测试选任务必须走 `filtered()` 视图
  （`selected` 是过滤后下标，raw `tasks.position` 与其错位——bug23 轮既有
  条目「selected 下标 ≠ tasks 下标」的渲染测试面同型，复证实例 = position
  选中的 Seeding BT 任务不在「正在下载」页签渲染出 0/y 空态）。

## 标志列轮 coder 续：QA runner 执行层同步（2026-11-04）

- **runner 同步欠账的三层形态盘点**：①断言落后于规格版本（TD-19 帮助 44 行旧口径，实际内容
  经 v1.34/v1.36/v1.42/v1.44 四轮涨到 80 行、八组→十二组）；②语义退役未翻转（TD-07 仍断言
  T 循环页签，而 phase-02 v1.9/FR-02-31 已把 T/t 改 trackers 弹窗门控键——runner 假红）；
  ③既定形态注记缺失（UC-08 的「BT 判据由单测锁 + QA 会话覆盖、runner 维持 HTTP 驱动」注记
  先例见 v1.36/37/38 同步注，新判据轮照此登记即可，HTTP 断言零改动）。
- **滚动收集步数公式**：内容 L 行、可视 V 行、PgDn 步长 8 → 收集步数 = ⌈(L−V)/8⌉ + 冗余
  （80−20=60 → 8 步，取 10 冗余）。内容行数每轮上涨时旧步数静默漏读——但组标题断言落在
  文本尾部会假 FAIL 而非假绿，恰是暴露点；改步数时同步核对新组标题列表（本次 8→12 组，
  补【字段值弹窗】【命令窗口（A）】【BT 对话框（添加/修改：BT）】【trackers 弹窗（T）】，
  逐字取 help.rs sec() 调用清单，勿凭 QA 文档转写）。
- **环境 FAIL 归因三连口径**：复跑×3 判 flake → HEAD worktree 对照同形判环境预存 → 不放宽
  断言只登记。本轮实测：TD-23（下拉展开期点按时序）工作区 F 3/3 + HEAD F 2/2；UC-07/17/18
  （校验码填入链：下拉 ↓×4 选 SHA-256 + Tab + 输码在本沙箱 pty 时序下不生效，详情无校验行）
  UC-07 工作区 F 3/3 + HEAD F 1/1——四例全部环境预存，与本轮 runner 注记/断言改动零交集。
- **HEAD worktree 对照操作细节**：`git worktree add /tmp/<name> HEAD` + 独立 target 构建
  （~2min）+ 同一 runner 单用例直调（`s._case_XX()`）；`worktree remove --force` 必须在
  主仓库目录执行（在 worktree 内执行报 not a working tree）；worktree 内 harness 的
  EZR_BIN 相对路径自解析到 worktree 内二进制，无需改代码。
- **pyte 环境重建**：runner 依赖 pyte（pty 画面捕获），沙箱重置后 `pip3 install pyte`
  即可（无 __version__ 属性，验证用 Screen 实例化探针）。
- **延迟代理基准实验口径（.work/tmp/lan/lan_bench.py + proxy_delay.py）**：`--workdir` 必须绝对
  路径（相对路径经 pty chdir 后 torrent 失效 →「无法读取种子文件」错误弹窗，与
  bench_multipeer `--ezr-bin` 同坑族）；only-static tracker 模式下 seeder 也会收到代理 peer
  地址并经代理自拨自己，产生「c2s 握手已发、s2c 永不回复」的假 hung 连接——用握手载荷
  peer_id 前缀区分（-EZ2000- = ezr，-LT2110- = libtorrent seeder 自连），勿误判 ezr 握手
  缺陷；outstanding 计数必须合并两方向（c2s.req − s2c.block − c2s.cancel），单方向流上
  恒为负值；libtorrent 对照组在飞深度 ~440（3s 排队时间语义），ezr 优化前后 64 → 514+ 是
  管线优化生效的直接证据；EMA 速率驱动的动态深度单测断言用区间（如 470..=480）不钉值
  （Instant 计时漂移 ±1）；管线目标 > visits×requests_per_piece 时首拍不满、靠逐块
  refill 1:1 补满（每拍每 piece 再发 4 个未请求块），小种子几次到达内收敛，属预期。
- **负载敏感 flake 复验沿例**：v147 `engine_conns_quota_exact_and_unique_ids` 本轮 full-run
  FAIL（left: 0 = 连接窗口内未建立）单跑即刻绿，改动无接触面（失败形态是少连接非多连接，
  与管线/重拨改动方向相反），沿 notes 既有非回归口径登记。

## 速度追查第二轮（2026-11-06）
- **slow-start 起步深度改变了假 peer 测试的可达性前提**：管线目标初值 64→4
  （libtorrent 成员初始化口径）后，「假 peer 只服务单一指定块」的旧形态会死锁——首访
  piece 由 jitter 随机决定，单块 serve 集大概率永远等不到请求；改法 = serve 集 = 全块
  减去目标 piece 的其余块（目标 piece 永不完整 → done 不含它是确定性断言），partial
  断言按 piece 定位（`iter().find(|p| p.piece == 0)`）而非下标/全集计数；推论：任何
  「假 peer 部分服务」测试在 slow-start 语义下都必须保证目标块之前的请求链能被喂饱。
- **负载敏感 flake 两例沿例（失败形态与改动方向相反）**：`pause_persists_partial…`
  全量首跑 FAIL（假 peer 15s 窗口未等到块）实为初值 64→4 的真回归，serve 集改造后
  连续全绿；`periodic_flush_writes_sidecar_mid_download`（HTTP 206 滴流，v147 套件）
  FAIL 属已登记负载敏感家族——BT 轮改动不可能影响 HTTP 下载状态迁移，单跑即刻绿、
  全量复跑绿，沿非回归口径继续观察。
- **bench_multipeer 结果判读口径**：s1l1（竞享 seeder）修复后 ezr 稳态吃满 seeder
  上限且为中继（lt leecher 显示 >2.5MB/s 的瞬时值 = 从 ezr 拉取），均速包含 ~14s
  第三方拓扑启动段；判定修复有效看三件：死区消失（curve 无 60s 零增长段）、
  均速接近 payload/(seeder 上限理论时停 + 启动段)、s1l0 对照无回归。

## bug25 轮：EMA 收敛显示断言取值、载体改判断言翻转面、demo PTY 基线对照归因
- **EMA 收敛显示断言**：SmoothedSpeed push 30 次收敛 99.9% 仍可能跨档回退显示
  （1024 seed → 1023.998 → `{:.0}` = "1024 B/s"，KiB 档断言假红）——显示档位断言
  seed 取档位中段值（1100 → 稳落 KiB 档 "1 KiB/s"）+ push ≥60 次（0.8^60 ≈ 1.5e-6）。
- **载体改判（toast→错误窗口）断言翻转面**：err_open/err_msg 正断言 + toast 不再承载
  该文案；「toast 为空」断言须用 assert_ne（同轮早前的成功 toast 仍在槽位，is_none
  假红）；对话框保持打开断言保留（错误窗口 z 序在对话框之上，keys/mouse err_open
  先于 dialog 消费）。
- **demo PTY 场景预存 FAIL 归因**：`git stash push -- <目录> → 跑基线 → pop` 对照——
  基线同败 = 环境预存非回归（verify_ezr.py 场景 R1-1 基本添加亦败，同
  verify_phase2_ui.py A 段沙箱预存同形）；stash 带 pathspec 不触 .work/ 未跟踪状态。

## cleaner 轮（2026-11-05）：挂载套件 ±N 对账与 quota flake 间歇态确认
- **挂载套件对账双向适用**：src `#[cfg(test)]` 测试的增删在包含该模块的每个集成套件
  同步反映——本轮 bt.rs 删 2 个窗口守护测试 → engine_backfill/g_appcore/g_engine/
  g_supervisor/ui_backfill/v115 六套件各 **-2**；bt_dialog.rs 追加 5 + app/mod.rs
  追加 2 → 同六套件各 **+7**。对账 = 每套件前后计数差必须与改动面精确吻合，差值即
  隐性丢失/意外收编信号。
- **g_v147 quota flake 间歇态**：`engine_conns_quota_exact_and_unique_ids` 本轮 4 次
  全量跑 2 次 FAIL（701P/1F、708P/1F）+ 1 次全量绿 + 单跑恒绿（3 连）——比上轮
  "1 次复现"更频但仍符合单跑绿沿例口径；与改动面（UI 展示层/对话框报错/App.windows
  死状态）零接触面，2 核调度压力敏感。沿袭登记不阻断。
- **只写不读字段的守护测试随守护对象退役**：删除死状态字段时，专测"不该发生行为"
  的守护测试（`bt_progress_does_not_feed_speed_window` 等）随字段一并删除，改名类
  测试（`progress_seeds_window_and_updates_task_view` → `progress_updates_task_view`）
  同步去掉名称与断言中的窗口语义——改名不改变断言覆盖面。

architect 轮（2026-11-06）：proptest 断言消息**内联捕获 {var} 在 prop_assert! 与 prop_assert_eq! 全系编译失败**（error: there is no argument named）——两宏都经 concat 展开格式串，rust.md「prop_assert_eq! 不能隐式捕获」条款实证扩展到 prop_assert!；一律显式位置参数（"msg {} {}", var）。属性测试揭穿测试侧错误断言的判别法：proptest 自动 shrink 给出最小失败输入直接指认过界断言（逐 piece 守恒假不变量 88B 未选文件/pl=1、selected 零区间假不变量 362B 未选文件），先读实现源码核对真实契约再改断言——本轮 2 轮揭穿全是测试错、实现对，**实现零改动**；属性红 ≠ 产品回归，先归因断言侧。属性测试收敛后并入 cargo test --bins 的计数对账（925→928，+3 属性随主 bin 计数），独立命令口径不变（cargo test property_tests::）。clippy::needless_range_loop 在「下标要传参 + 元素要用」双需求场景（pieces_of_file(i)/files[i]）改 `for (i, f) in files.iter().enumerate()`，不要回退索引循环。
QA 轮（2026-11-06）：对话框焦点几何漂移的探针实证法——添加对话框打开焦点恒 URL0、Tab 序 URL0→保存到1→并发2→下载限速3→校验4→校验码5→代理6→按钮行，v1.39/v1.40 上传限速字段退役后下载限速前移一档（从保存到起 Tab×2、从打开起 Tab×3），runner 仍按旧几何 tab×3 即越位到校验字段，断言落错字段（「键入 500k 应规范化」假红）与提交失败（enter 未落立即下载）是同一根因的两种显形；判定用哨兵字符串探针（每 tab 后键入唯一标记读屏落位）比静态读源码/数注释可靠，修复=重推现行几何+落位后 clear 预填再键入；fixture `cd=` 查询参数 = Content-Disposition filename 覆盖（namegen::derive_name 优先级 1），落盘名 ≠ URL 末段——runner 的完成观测路径必须与 cd= 值严格一致（tp10.bin ≠ cd=tp10 落盘的 tp10，任务早已完成但观测路径永不出现 → 「90s 未完成」假红）。前台 10min 上限的套件内过滤机制推广：QA_TP_ONLY 先例复制为 QA_AT_ONLY（Suite.run() 读环境变量过滤 _case_NN，缺省全量、结果仅含所跑用例、汇总由调用方合并各批），修复后只需复跑失败用例（QA_TP_ONLY=9 / QA_AT_ONLY=20 单用例秒级-分钟级）。llvm-cov 沙箱工件挥发对策：cargo-llvm-cov 跑完 deps/.fingerprint 可能被清（profraw 残留）→ 手工管线替代：RUSTFLAGS=-C instrument-coverage cargo test --bin ezr --no-run --target-dir <临时> → 测试二进制在 build/<hash>/out/ 下 → LLVM_PROFILE_FILE=... 跑二进制 --skip property_tests（engineering:53 属性测试不进覆盖率命令）→ llvm-profdata merge -sparse → llvm-cov report <bin> --instr-profile=... --show-functions --sources <src>；每函数多实例化行，取无泛型后缀的主实例化行（泛型变体 100% 是噪声）；CRAP 复算 cov 用 region 列（covered/total），comp 沿登记口径。DRY 基线口径失传的 A/B 恒等判法：历史「N 文件 M clones」文件集未随归档留存时，穷举重构不可靠 → 改扫全生产源码并做工作区 vs git archive HEAD 双扫描，clone 集逐位置 diff=0 即证本轮改动不增不减（无恶化无需复现历史口径），新口径数字与口径差异记 handoff。

复验轮（2026-11-06，操作者指令「遗留受限项再验证，有问题则修复，没问题则移除」）：

## 测试 stub 不读请求先写响应 = 「unexpected message」假 FAIL 家族真根因
- **现象**：hardening_g_v147 quota 双用例间歇 left:0（连接视图全程 0 行、烧满 deadline）；
  此前历轮按「2 核调度压力敏感」登记沿例，复跑判据三证齐。
- **定位**：采样循环临时探针（打印全部 Evt）——失败运行全程**仅一个 Evt::Failed**
  （`received unexpected message from connection`）随后死寂：探测秒败 → Transient →
  无 worker 行。stub_206_trickle / stub_200_norange_slow（同文件 8 stub 中仅此 2 个）
  accept 后**不读请求头立即写响应**；loopback 通常侥幸，调度抖动时 hyper 在请求发送
  中途收到响应即协议态错乱。负载越重抖动越大——「压力敏感」只是表层。
- **处置**：两 stub 补 BufReader 读头循环（同文件 stub_nocl_trickle/range_recorder 本就
  正确——同批手笔漏网两处）；quota 采样窗加宽（起步 150→300ms、单事件 400→800ms）。
  修复前定向压测 ~1/6 失败率（复现 3 次全抓到 Failed 实证）→ 修复后 15 连绿 +
  全套件 709P ×2。
- **教训**：write-before-read 是 HTTP fixture 的协议级违约，任何手写 stub 必须先读完
  请求头再响应；「间歇性环境 FAIL」家族在归因环境前值得先用事件探针抓全程事件流，
  单事件 + 死寂形态直指任务早夭而非调度延迟。

## 定宽单读断言改轮询判据（sidecar 中途落盘）
- periodic_flush_writes_sidecar_mid_download 固定 4.6s 睡眠后单读 sidecar：滴流节奏 ×
  2 核负载下首次 flush 可晚至 10s+（复验实测 10.4s 样本——旧口径必挂）。
- 改最长 20s 轮询（flush 周期 2s，≈10 倍余量），判据语义不变（中途落盘本身即判据），
  flush_sidecar→() 突变体仍被超时杀伤；3 连绿。

## 同路径多实例任务必须 cd= 专名隔离（FR-01-26 接续臂边界）
- QA-RS-06 三重根因实锤（屏面转储定位）：①5 个占槽任务同 URL 路径仅查询参数互异 →
  前一任务周期 sidecar（2s）落盘后，后续添加命中 dialogs.rs:244 接续臂
  （Sidecar::load(dir/推导名) 按路径基、无「活任务占用」守卫）复用同名 → 两活任务
  共享 dl 文件互踩 → 一方 fatal「不自动重试 · 0 B」→ 槽位释放 → 排队前进 →
  「排队第」判据失真；②3MB@100KB/s=30s < 降速沙箱 6 次对话框添加的 setup 窗口 →
  持有者提前完成、槽位全空、第 6 任务直接下载无排队（屏面实证「已完成 (7)」）；
  ③恢复初期 End 注入吞键（QA-SQ-13 同源）。
- **处置**：①持有者 `cd=rs06-h{i}.bin`（derive_name 优先级 1 = Content-Disposition）
  专名隔离，彻底绕开接续边界——产品侧缺口（FR-01-22 兜底不覆盖并发写 sidecar）移交
  specifier，不越权热修（FR-01-26 v1.28 明文「不按 FR-01-02 追加序号」）；②体量改
  5MB@60KB/s ≈ 83s 覆盖 setup 窗口（kill9 即止无收尾成本）；③End settle + 有界重注。
  修复前 ~1/3 失败率 → 修复后 5 连绿且耗时稳定 20.7s。
- **教训**：e2e 多任务编排的夹具体量/速度必须按「降速沙箱 setup 耗时」预算，不是按
  正常机速；同路径多实例是 FR-01-26 接续臂的显式危险区，runner 一律 cd= 专名。

## 断言锚从瞬时中间态迁到长驻可观察物（SQ-13 复验落地）
- QA-SQ-13 原断言 `wait_for("等待中") or wait_for("下载中")`——「等待中」窗口可 <100ms
  （授槽 tick 对齐），天然竞态。改锚 requeue_failed 专有 toast「↻ 手动重试（等待中）」
  （≈4s 长驻）+ 再探回「已失败」（全链走通证据）；3×15P/0F。

## 工具链补记（复验轮）
- rustup 分发镜像与 crates 下载镜像**分开选型**：rustup 走 TUNA（manifest 实测
  3.08MB/s 最快），cargo registry 走 rsproxy sparse——TUNA sparse index 的 config.json
  `dl` 仍指向官方 static.crates.io（慢且可停滞，195/285 crate 后零进展）；rsproxy
  index+dl 一体，285 crate 2min 拉齐。
- 沙箱后台进程随 Bash 调用结束被回收（setsid/nohup 皆不保）：cargo 构建一律前台
  timeout 分块 + 增量缓存续跑；clippy `--force-warn` 不是 cargo 侧参数，crate 级 allow
  需 `RUSTFLAGS="--force-warn clippy::cognitive_complexity"` 压制。

## 规格改判轮的测试前提翻转面（v1.45+v1.46 实录）
- **现象**：实现 v1.46 改道臂后三个既有测试转红——`invalidated_resets_and_toasts`（g_engine）、`unknown_size_resume_invalidates_not_transient_fail` 与 `inconsistent_block_count_invalidates_even_when_size_matches`（g_v147）。共同前提 = 「添加时点带异名 sidecar → 一致性不符 → 原地作废 + streak +1」，该前提恰是 v1.46 废弃的路径基孤儿臂旧行为。
- **处置**：①g_engine 拆双测——原测改「自有臂」口径（add 后手动 `resume_orphan = false` 模拟已收编/注册表恢复态，断言作废 toast 不变）；新增 `orphan_adopt_mismatch_redirects_silently_and_preserves_sidecar`（断言改名 f.bin.1 + Queued + 无作废 toast + sidecar 仍在盘）。②v147 两测试保留突变体守卫价值但断言翻转：突变体（>→>=、&&→||）失效形态从「误接续」变「不致改道/暂态失败」，断言改 `name == "f.bin.1"` + `state != Failed`。
- **根因**：载体/行为改判轮（toast→错误窗口、原地作废→序号改道一类）的测试翻转面判定法沿 bug25 轮口径扩展——先 rg 全部以「旧行为可观察物」为断言的测试（streak>=1、作废 toast、is_none 类），按「该测试场景在新规格下属哪一臂」逐个改写；突变体守卫的断言方向随行为翻转，守卫点本身（被守护的代码行）不变。
- **bug26 轮（限速不生效）验证法四件**：①pty 真机 e2e 三坑——ezr 预分配目标文件 set_len 后 size 恒等于 total（文件大小不能当进度判据，用 TUI 屏百分比/做种态）；`wait_for("命令窗口")` 只匹配空列表占位符文案，列表非空时改锚「EZR Downloader」标题；EzrPty.send_key 的 "Backspace" 会把字面 8 字符敲进字段，真退格须 `os.write(fd, b"\x7f")` 且退格次数按预填长度清场（单次退格清不空 "2 MiB/s" → 残串拼新输入 → 非法 toast 假象）。②fast mock + 限速下事件间隔不可钉死：回环缓冲聚合使 chunk 从 8KB 长到百 KB 级，单次 acquire 可 >2s 无 Progress 事件——限速断言的事件窗按「限速值 × 最大 chunk」预算（30s）+ 总 deadline 兜底，别用固定 2s（会误判挂起）。③Throttle 隔离单测过 ≠ 集成通：集成挂起先二分「acquire 前 vs 完成」插桩定位，别急着改桶实现（本轮隔离测 4.74s 精确限速、集成"卡死"实为测试窗太窄）。④断言下限取 ≥ 理论时长 × 0.85（如 512KiB@100KB/s 应 ≥4.5s），上限靠总 deadline；速度峰值采样 ≈ 限速值 ± 突发窗为生效证据（512 限速实测峰值 487-497）。
- **bug26 轮根因与修复**：任务级限速"不起作用"的实锚 = CLI 添加路径漏接 config 任务级默认（add_cli_task 不设 t.max_download_speed → 0=不限）——「新建任务的默认值」类规格（FR-02-29）的接线核对法 = rg 全部任务创建路径（dlg_finish_add/dlg_confirm_add_bt/add_cli_task）逐个验证默认值落位，对话框路径全对 ≠ 全路径对。附带 harness 修复：bench_multipeer.py config 模板 `ttp]` 缺 `[`（TOML 语法损坏，try_load 报错退出——上一轮终端伪影经验此处真损坏，boolean 探针判别法再次必要）。

**bug27 轮（2026-11-07，coder）**：判重口径翻转的测试翻转面三件——①旧口径测试按名翻转而非删除（finish_add_different_dir_is_not_duplicate → _is_duplicate，断言整体反向 + 消息注明新规格依据），保留同 URL 异目录这条边界线的测试覆盖连续性；②同 URL 二次添加类测试（cli_add_dedupes_duplicate_names 同 URL 两连加）在新口径下语义变化，改异 URL 同推导名保全「名字去重」语义、另立同 URL 拒绝测试——翻转时先问「这条测试原本守的是哪个不变式」，不变式仍成立的换数据、不成立的才翻转断言；③盘点 blast radius 的机械法：rg add_cli_task 调用点 → 按所属测试函数聚合 → 只有多次调用同测试的才可能受判重收紧影响（本轮 30+ 调用点仅 1 个多调用函数且异 URL，零集成翻转）。小盘跑法复用：逐目标 --no-run + touch 强制重链（cargo 判 fresh 不重建被手删的二进制）+ 测试输出落 /tmp 日志文件再 rg（管道 tail 在 timeout 截断下丢输出的假象坑）+ 磁盘 99% 链接 Bus error 先删 target/debug/incremental（2.6G，CARGO_INCREMENTAL=0 下纯死重）。

## bug27 清理轮（2026-11-07）：llvm-cov 收尾花括号幻影行与判重错误臂补测
- **llvm-cov lcov 的 if-let 收尾 `}` 行是幻影未覆盖行**：if-let 臂体为单条赋值/表达式时，
  LLVM 区域计数器终止于臂体行，臂的收尾 `}` 行 DA=0——臂实测已执行（toast 断言通过）但该行
  恒报未覆盖。判据：`}` 行且其所在 if-let 的臂体行已覆盖 → 幻影，不计入真实缺口；覆盖率
  口径里这类行不可归零，CRAP 受限近似按「可达臂全绿 + 幻影行登记」收口，不为凑 cov=1.0
  硬造测试。
- **CRAP 受限近似口径（bin 套件 llvm-cov + 语法决策点代 comp）**：comp=if/loop/match/&&/||/? 计数+1，
  cov=函数行覆盖；comp≤6 是拆分门槛（comp≥7 覆盖率再高也过不了 6）。函数级归属沿 rust.md
  闭包共享行区间取极值纪律。本轮批次函数实测：dlg_finish_add 拆分后 comp 12→5、
  dlg_confirm_add_bt comp=6 cov=0.947（残余 = 3 行幻影 `}`）→ CRAP≈6.005 属幻影噪音级，
  登记不硬凑。
- **纯追加 4 测（bug27 判重错误臂与 BT 边界）**：confirm_add_rejects_corrupt_torrent_file
  （可读非 bencode → 解析失败 toast 臂）、confirm_add_rejects_unsupported_tracker_protocol
  （ftp:// tracker 拒绝臂）、confirm_add_bt_paused_announces_paused_toast（BT 暂停添加 toast 臂 +
  无对话框确认空操作）、confirm_add_bt_empty_selection_rejected（空选守卫臂）。挂载对账：
  bin 945→949、7 挂载套件各 +4（845→849 等），差值即对账信号。
- ui_backfill `graceful_close_sends_fin_then_force_closes_after_grace` 全量中 FAIL、单测复跑
  0.20s 绿、全套件单跑 879P/0F 绿——三证齐判时序 flake（第 4 例，同族判据沿用）；本轮改动
  面与 bt::engine 零接触佐证非回归。

## architect 轮（2026-11-07）：Dialog 字面量收编等价口径与限速往返属性的 2^53 域
- **Dialog 测试字面量 → testutil::dialog 收编的逐字段等价口径**：`TextInput::set(x)` ≡
  `with_value(x)`（value + cursor_end 同语义）；工厂默认 `ck_type/ck_sel = 4` 与测试口径
  `CK_NONE = 0` 不同，收编时**必须显式覆写这两个字段**；bt_follow/conns_edited 等其余默认
  与字面量逐字段核对一致。收编形态：本地 `add_dlg(url, dir)` 助手（差分点参数化）+
  `d.url.set(...)`，8 处字面量全收编（判据：字面量与工厂差集 ≤ 2 个语义字段即值得收编，
  差集更大时字面量自文档更优）。
- **fmt→parse 往返属性的域必须限 f64 整数精确带（2^53）**：`parse_speed_limit(fmt_speed_limit(v))
  == Some(v)` 对全域 u64 不成立——fmt 走十进制裸整数（B/s 臂）、parse 走 f64，v ≥ 2^53 时
  f64 舍入可把 parse 顶到 u64::MAX clamp 臂（实测 u64::MAX−1 → Some(u64::MAX)）；全域属性
  断言必假红。属性域限 `0..(1<<53)`；固定值单测（含 u64::MAX clamp 边界值，恰好自洽）探不出
  舍入带——**固定值单测绿 ≠ 全域属性成立，属性域约束不可省**。
- 属性缺口定位法：`grep` 属性清单中被测函数名——单测有固定值往返（fmt_speed_limit_round_trips_
  through_parse 10 值）≠ 属性覆盖存在；本轮补 2 条（fmt→parse 往返 + 单位全别名二进制乘表
  域表征 `n × mult` 对 select 全别名 × 空格 × /s 全组合），proptest 23→25，独立命令
  `cargo test property_tests::` 76P/0F；bin 949→951、7 挂载套件 +2 各计（对账隔离：集成
  6357 不变，属性增量单独报数）。
- 限速解析双口径在案再确认：config 侧 `parse_speed` 旧写法 KB/MB/GB 拒识（v1.40 废弃）、
  对话框侧 `parse_speed_limit` 全别名（含 KB/MB/GB）按二进制乘表（bug21 口径）——两口径
  各有属性/单测，混同即假绿。

## QA 终验轮（2026-11-07）：判重翻转面、清场自适应与三竞态锚定
- **清场必须自适应判空，不可假设预填长度**：v1.38/D62 保存目录字段预填配置链值，长度随
  工作区路径深度变化（/home/z/my-project/swarm-ezr2 下 75 字符 > 旧固定 64 退格）→ 残留
  与键入路径拼接 → save_dir 畸变（registry 实录 `/home/z/my` + 全路径）→ 伴随校验全家
  假红。修 = 单字节退格 ×16 一轮 → 读屏判空（空态含占位符形态 `/srv/downloads`，dialog.rs
  逐字段定义）→ 未空再来一轮；单字节写入同时规避 PTY \x7f 连发投递上限。
- **pyte display 对超宽字段做「…头截断」**：全路径包含断言在深工作区必败——预填断言锚
  可见尾部两段（如 `<home basename>/cfgdl`），或对字段值断言改走落盘证据。
- **完成 toast 同刻顶替竞态**（TD-02）：并发慢任务恰同刻完成时，新完成 toast 可见窗
  ~0.2s，wait_for 轮询粒度必失——toast 观测与落盘 wait_file_size 双臂锚定。
- **v1.43/v1.47 判重的套件翻转面（同 URL 即任务身份、与保存目录解耦）**：同一 env.home
  内多相位/多迭代复用同 URL 的用例（子目录隔离 = 旧「同 URL+同目录」口径假设）全族改查询
  参数差异化（fixture 忽略未知键）；本轮实录 IC-01 / PC-05 / PC-09 / NP-08 / NP-14/16/17
  / TP-05（含循环尾变量复用形态）/ MT-02 / MT-04 / AT-12。判别法：空 note F =
  无消息 `wait_for("下载中")` 断言，即判重拒绝后无任务。
- **重排队类 toast 被再探失败 toast 顶替**（SQ-13）：回环「R→重排队→授槽→404 再探」全链
  87ms（20ms 细粒度连拍取证：toast 22ms 出现、87ms 被顶替）——改锚 fixture 访问日志
  新增请求（磁盘持久证据）+ 已失败回环断言。
- **满容量中位忽略的前提 = 字段已满**（feature When「且字段已满 2 位」+ 单测锚「满容量非
  行尾不重启不插入」）：runner 顺序须先补满再验中位；无 End 键见 environment.md 本轮条目。
- **大任务完成时点须预算出用例序列**（UC-13/TD-06）：100MB@16 连接×150KB/s ≈ 42s 与
  加任务序列同长 → 完成迁移污染列表锚（列表塌缩/视口顶变化）；12 任务排队场景须让任务
  活过加序列（rate 压至 8KB/s）。门控速度按「每连接限速 × 并发数」折算总耗时代价。
- **CRAP 本轮实测**：`cmd_complete_path`（命令窗口 Tab 补全）comp 24 × bin 套件 0 覆盖
  （CRAP 600 全仓最差、无任何单测）→ 补 3 测（唯一目录 / 多候选 lcp / 无匹配）→ cov 0.824、
  CRAP 27.17；对账 bin 951→954、7 个挂载套件各 +3（6357→6378，8×3 自洽）。`ezr_main` /
  `on_mouse_trackers` bin 套件 0 覆盖属承载面差异（e2e/集成承载），登记观察项不硬凑。
- **QA 规程↔runner 版本欠账三层形态再证**：规程行已随 v1.46 改判同步而 runner 未改
  （RS-14 全套断言仍是 v1.28 原地作废口径）→ 以 feature 场景 Then 链逐条重写 runner，
  断言锚全部改持久物（改道名、孤儿保留、.1 独立 sidecar 生成/删除）。

- specifier ws(s)/WebRTC 修订轮（2026-11-09）：钦定文案全角括号通道改写陷阱第二例（bug18
  `ttp]` 写入侧同族）——toast 钦定文案 `（仅 http(s)/ws(s)/udp）：` 的**外层全角闭括号经
  编辑通道落盘成半角 `)`**（内层 `(s)` 保持半角属正常混排，肉眼难辨）；写入后必须
  `grep -c "udp）：" / cat -A` 字节级终审外层全角括号，python bytes 级 replace 修复；
  本轮 features 2 处 + qa 3 处同修。mini_ws_tracker harness 端口段 17905（ws）/17906
  （wss 自签）随夹具段 17901–17904 顺延（登记 harness/README.md 与 D70⑦）。;

ws/webrtc 轮（2026-11-09）：ws announce 单测用 tokio_tungstenite::accept_async 同 crate 起本地服务（无需外部 harness 即可验 JSON 往返+信令字段）+ 沙箱重置后重建的「每里程碑 git commit」纪律（上一轮全部未提交改动一次丢失）+ hardening 垫片第三形态（include config.rs 的测试根需最小 bt 垫片：BtError 五变体+Display+From<io> 与 bencode/tracker/webrtc 三个 #[path] 子模块——tracker.rs 内嵌测试引用 crate::model::config 时垫片根再挂 mod model{pub use crate::config_fallback 的 pub config}）+ webrtc 测试目标挂载面（凡 #[path] 引 src/model/mod.rs 或 config.rs 的测试根都要有 crate::bt 可达，g_slots 系漏挂=E0433 crate 根无 bt）+ #[path] 在内联 mod 里相对基目录会追一层 mod 名目录（tests/hardening/bt/…）改独立文件 tests/hardening/bt.rs 基目录才是 tests/hardening + 协议列对齐翻转的测试断言面（cell→cell_left 后 right-aligned pad_left 期望串逐处改 16/17 空格差，显示层吞字符时用 python 结构化 repl（'x'+' '*N）勿手抄字面量）+ 全量回归以「统一环境变量先 --no-run、再逐二进制 timeout 前台跑」最稳（不同 CARGO_PROFILE_* 环境混跑 = 指纹失效双倍构建撑爆小盘；nohup 后台脚本仍会被会话回收）

裁决落账轮（2026-11-09，操作者 chat 十一项）：D71 八项（①③④「与参考实现同策略」核证落账 = qBt/libtorrent 无 WebRTC、参考实为 WebTorrent 家族线协议、peer_id 角色分裂为防 glare 确定性补充且线协议兼容；⑥ DataChannel label 改 "data_channel"——发起侧 create_data_channel 单点、应答侧 on_data_channel 随对端故仅一处）+ QA 终验轮未决三项全结案：判重张力（url_task_exists 本无状态过滤=钦定口径，规格删「非完成」限定零码变）、hardender 定可选节点（CONSTITUTION/chains/INDEX/SKILL 同步）、QA-01-11 双锚断言（pyte「…头截断」从「必败障碍」转正为「有意深路径渲染行为锚」+ 短路径整值逐字符断言恢复全路径口径——教训：屏幕捕获断言要先锁「被测值在可视宽内」这一前提，短路径整值锚与深路径尾部锚双管齐下才同时覆盖准确性与正确性）；harness 新增 QA_ONLY 环境变量单用例复跑入口（默认不设=全量行为不变，长套件复跑不再整跑）。另：沙箱第二次重置（仓库/rustup 全失），从 my-project 归档 081517 恢复重建 git 基线 839b8e4 后续做——归档含已提交全态，恢复成本 ≈ 解包+rustup 前台重装，「每里程碑 commit + complete 后归档同步 my-project」纪律再次验证必要。;

bug28 轮（2026-11-09）：tokio::spawn 借用参数必 'static——ws_tracker_session(&url,&ctx,&tx) 全引用签名无法直接 spawn，测试内改「spawn 的 async move 块里构造 ctx + channel 再调用」（url/reannounce Arc move 进去）；AnnounceCtx 字段扩展时 start_tracker_pool 三处调用点（初始/UpdateTrackers/proxy 重建）同改易漏，编译器兜底即可但测试夹具构造体也要同步（ws_test_ctx 集中构造防漂移）；tokio::time::Instant 与 std::time::Instant 不可互赋（next_announce 混用 E0308，统一 tokio 版）；WsAnnounceResponse 此前漏解析 "min interval" 字段——补字段时 Default 派生让结构体扩展零破坏（..Default::default() 尾随）；clippy unnecessary_get_then_check 新 lint（HashMap get().is_none() → !contains_key）；删除文件类函数「先 is_dir 分支再删」对不存在路径天然幂等（remove_file 对目录无效且静默——多文件 BT 目录树删除缺陷的教训是删除函数要按落盘形态分支，不是加错误处理）。

文件勾选修订轮（2026-11-09，coder）测试经验：

- **引擎 evt 通道容量 256 是测试死线**：磁盘轮询循环不消费 evt_rx → 引擎 `evt_tx.send().await` 满阻塞 → 引擎整体冻结（表象 = 连接活着、零事件、零进度，与「下载停滞」同形极易误诊为业务 bug）。轮询必须 `while evt_rx.try_recv().is_ok() {}` 排水。
- **handle 的 set_len 立即创建稀疏零字节文件**：「文件存在」≠「补下完成」——勾回补下轮询若先判存在即断言，会在 set_len 瞬间读到全零假绿（或字节不符假红）。轮询条件必须字节 == 期望（bug23 轮「预分配 set_len 未落盘断言全零」的轮询形态姊妹坑）。
- 结构体加字段的全仓同步面：BtSpec/BtTaskData 字面量遍布 src 测试 + `tests/gen/*_stripped.rs` 副本（rg 全 tests/）。python 大括号匹配批量补丁的两个假阳性：`-> BtTaskData {`（返回类型+函数体括号被当字面量，字段插进函数体）与 `impl BtTaskData {`（impl 块被当字面量）——补丁后必须 brace-match 复扫 missing + 抽样布尔探针，肉眼比对不可靠。
- 做种任务在 `filter=0`（进行中视图）不可见（`is_done` 含 Seeding）——M 键修改对话框链路测试须 `app.filter = 1`（已完成视图），否则 `modify_dialog_entry` 走「没有选中的任务」臂、dialog 恒 None（表象 = `dialog.unwrap()` panic 于 None）。
- 引擎级集成范式：`reconfigure_deselect_deletes_then_reselect_redownloads`（真 seeder + 首下载 BtDone → 删除+目录清理断言 → 勾回字节断言），多文件 meta 构建器 `make_meta_files`（files 列表 + name 首段路径）可复用。

## 做种倒计时补缺口轮（2026-11-10，coder）追加
- **tick 类逻辑直调 + 合成 dt 测法**：倒计时勿依赖真实 Instant（dt 被 clamp 0.001~0.5 且随机器抖动）——测试直调 `tick_seed_countdown(1.0)` 得确定性断言（1800→1799.0）；主 tick 接线另立独立回归锁，断言用开区间 `seed_left < 1800.0`（钉值会被首帧 dt 漂移随机打破，bug24 轮「不钉具体数值」纪律同源）。
- **「接线缺失」类缺口的回归锁形态**：本缺口本质是 tick() 漏挂递减臂——直调私有函数的测试即便全绿也防不住漏挂回归，必须有一条走 `app.tick()` 全链路的接线测试；再配 `assert_no_cmd` 锁「未归零不发命令」与「归零后二次递减不重复触发」。
- 挂载收编对账：app::engine::tick 新 3 测 → g_appcore 790→793 / g_engine 786→789 各 +3，与 bin 994→997 同源三方一致。

## cleaner 收敛轮（2026-11-10）追加
- **ezr-demo 守卫 FAIL 的三步归因与修否裁决法**：①先验 demo 树状态（git status 于 HEAD clean + 从源码全新构建再跑——缺二进制时守卫子进程秒退，全屏空白产生成片假 FAIL，A5/A6b/B0/T3 即此类，显式传参 target/debug 二进制后消散）；②A 族（multiline 清场链）根因 = 守卫清场依赖退格连发，沙箱 PTY 对 \x7f 投递上限（bug19 登记限制）使 400×3/100×10 连发大量丢失，残留 'xxx' 贯穿后续断言（FAIL 消息携带残留字符即铁证），bug19 的缓解方案（合法输入直接确认）只回移植了 QA runner、未回移植 demo 守卫；③主守卫（verify_ezr.py）75 FAIL 为场景时间线/锚点钉历史轮次同步口径的基线漂移。裁决 = 否（不修）：demo 侧守卫重定基线需操作者权威裁决哪一版行为口径为准，超出 cleaner 本地可验证规模；产品面守卫（bin/集成套件）全绿不受影响。
- **CPD 对账口径**：双阈值读数（200 = 结构级克隆、70 = 全量）+ 阳性对照先行（同内容双 .rs 除函数名外逐字节相同，PMD 7.29 在 ≥93 token 对照稳定命中，阈值 70 下小对照不命中属正常非失灵）；EXIT=4=有克隆、0=无克隆（经管道 tail 时 $? 是 tail 的，探针要直跑或 PIPESTATUS）。
- **变异点扫描口径再证**：cargo-mutants 27.1 `--list` 全仓 5994 点，交互层千行文件 200+ 点（mouse 220/textfield 216/multiline 204）——变更文件 >100 点按 rust.md 登记 handoff 移交（hardender --file 分块兑现），cleaner 不强拆。

## 架构评审轮（2026-11-10，architect）追加
- **中断恢复的 mtime 时序校验法**：恢复被中断会话时，以「产物文件 mtime vs 验证日志时间戳」决定哪些验证仍有效——源文件 mtime 晚于验证时刻 → 该验证作废重跑（本轮 property_tests.rs 06:50 > t_bin_final 06:46 → bin/clippy/fmt 重跑）；源文件 mtime 早于验证时刻且被改文件不在该验证的编译域 → 验证结果沿用（engine/ 05:48 < 集成 05:55–06:25 → 集成 6869P 沿用）。前提核验 = 被改文件是否在验证目标编译域（`rg 'path = "\.\./src' tests/` 白名单比对，本轮集成目标不含 property_tests.rs）。
- **过滤运行的双重收益**：`cargo test property_tests::` 单命令同时拿到①属性测试独立报告（82P/0F，对账隔离口径）与②全仓 13 目标编译通行证（其余目标 `0 passed; N filtered out` 的 N 即各目标测试总数，编译过 = 当前源码态全目标可构建）——中断后重验先跑过滤运行最省。
- **bin 对账的属性测试分层**：bin 计数天然含 proptest（同 bin 目标编译），对账按分层口径书写：1003 = 921 非属性 + 82 属性（基线 997 = 921 + 76 同式）——非属性面零变动 + 属性测试独立报告，+6 全归属属性测试交付物，不与单测基线混对。

## QA 终验轮（2026-11-10）追加：上传暂停协议化修复与 BT e2e 门控三课
- **上传暂停必须协议级 Choke，静默丢请求会让恢复永久失效**：现象 = MB-10 做种回退后回做种，leecher total_done 永久冻结（对端视角 dlq=9 请求悬挂 + snubbed 置位、flags 0x3413 恒定），上传速度 0——回做种后上传从未恢复。根因 = 原实现暂停期 `allowed=false` 静默忽略 Request 不回 Choke（BT 语义违规），libtorrent 侧 9 个请求进入悬挂队列且 snubbed 后不再重发，恢复后门控放行也无请求到达。修复 = 暂停进入统一 Choke（am_choking=true + 发帧）+ 暂停期跳过 apply_choker（否则 rechoke 周期会把 choke 再解开）；恢复时既有 apply_choker(seeding=true) 因状态迁移自动发 Unchoke；peerwire allowed 门控保留为第二防线。判别工具 = lt python `get_peer_info()` 的 flags/download_queue_length 双侧观测（对端视角一眼定性「谁 choke 谁、请求挂在哪」）。
- **BT e2e 门控三课（MB-10/MB-03 轮实证）**：①「文件存在/大小」均非完成证据——set_len 全尺寸稀疏预分配使新勾选文件确认瞬间即存在，等待类门控（`all(_disk_file(...))`）0 秒塌缩（勾回补下轮「轮询须字节比对」条目的门控形态姊妹坑）；等待循环与断言一律字节级比对。②piece 完成序由调度决定非索引序（16KB/s 实测 b→c→a→d），相邻完成间隔 ≈ piece_size/限速（16s）远大于轮询间隔但相邻件交叉使固定序门控（a∧b∧¬c）不可达——顺序无关门控取「恰 N 文件完成」态并动态选取消目标。③lt seeder **不按 file_priority 限制服务**（MB-12「seeder1 只供 a/b」是窗口短未暴露的假象）——制造未完成窗口唯一可靠手段 = upload_limit 限速，窗宽按完成间隔 ≈ piece_size/rate 预算（限速值每降一档窗口线性拉长）。
- **断言数值先验**：进度显示 = done_pieces×piece_size/1e6 保留 1 位（2 文件 = 4×256KiB = 1MiB → "1.0/1.6 MB"·66.7%，勿按文件数心算成 0.5）；toast 尺寸同 sizefmt 口径。

## v1.47 显示指标轮（2026-11-10，specifier）追加
- ezr-demo PTY capture 验证三坑（新增）：①capture.py 的 `j` 是列表滚动不是选中（任务选择用 `down` 命名键，j×N 只滚视口、详情面板不换任务）；②发键节奏 = 45%×duration 起每 0.18s 一键——长串 right×40 需 duration ≥ 键数×0.18/0.55，否则横滚半途而废貌似「吸附失灵」（cap_conns3 半程实录）；③并发表横滚验证优先 unit TestBackend 全宽渲染（render_conns 130 宽一次看全九列），PTY 只做主屏文案级抽查（健康度/分享率行 + 表尾相关度/协议）。
- demo 守卫预存漂移不新增破坏的确认法（新增）：N5b/P20b 断言算术停留在 m3 时代（「表宽 59 − 44 + 1 = 16」），v1.43 内容宽 115 后 hmax=72 早已 >16——本轮改列前先用几何推演确认该两案在 HEAD 上同败（基线漂移类、cleaner 轮裁决=否），新增列只是让未来重定基线时的目标算式再 +7+1；N5/P20「客户端/协议不在首屏」类断言对列插入不敏感（新列偏移 99+ 更在视口外）——改列前先扫 guards 的列偏移假设，只登记不修。

## v1.48 轮（specifier）：宽字表格水平窗口对齐的测试口径
- TestBackend 裸 buffer 的 cell 下标 = 显示列（render_text 对宽字 x += width，占位格不写），
  表头「协」与数据 "TCP" 的同列断言可直读 buffer 坐标；但把 buffer 符号逐格 join 成字符串时
  宽字会多出占位格偏移——几何断言一律按 (x,y) 直读，不要 join 后 find。
- pyte 侧同理：cursor 按字符显示宽推进、宽字后一格为空串占位——cell 下标即终端列；但把
  各格 data join 成串后 `find("相关度")` 的命中下标 ≠ cell 下标（占位格不贡献字符但坐标已
  前移），按 join 串下标换算显示列必错。
- 复现条件探查法：错位只在「hscroll = 表头某 CJK 起点 p+1」时出现（ratatui 整字回退臂），
  hmax = 内容宽 − 视口宽 + 1 随终端宽度连续变化——扫描多档终端宽度（90–190 列）的最大
  滚动位即可撞到复现点（本轮 150/160 列）；键盘步进 2 的偶数偏移也有跨界位（如 64/66/68）。
- 守卫覆盖面：slice_spans 对每个 cut 断言「切片后可见宽度 = 总宽 − cut」恒等 + 表头/数据
  单元格起点恒等，cut 全域扫描（0..=总宽）；另加 TestBackend 渲染级对账（直接置
  conns_hscroll=59、61 列终端）——纯函数守卫防不住渲染接线回归。

## v1.49 全类型扩面轮（2026-11-11，specifier）
- **pyte 宽字符占位格双形态重建法**：占位格 `data == ''` 跳过 → 逻辑行（子串断言用）、
  补空格 → 显示行（快照/列位用）——单一形态两用必错：占位格当空格会让 CJK 子串断言
  全假阴性（"[做种中]" 重建为 "[做 种 中 ]"）；`screen.display` 直接塌缩宽字少一列，
  列位敏感检查不可用。
- **ratatui Buffer 占位格符号 = 空格（非空串）**：TestBackend 渲染对账后逐格 join 的
  字符串含插入空格（同上 "[做 种 中 ]" 形态），CJK 徽标子串断言须先 `replace(' ', "")`
  归一化；与 pyte `data=''` 行为不同——两库两口径，别把一方的处理经验直接搬给另一方。
- **模拟动态窗口的键序设计法**：demo 状态迁移有仿真时限（PostProcessing 3400ms、
  flaky fail_after ≈6s、重试回等待），验证脚本键序按模拟时钟排布：快速键序（0.12s
  间隔）在窗口内走到目标行、慢速等待状态翻转后再切页签；帧并集（union of frames）
  覆盖滚动路径全貌，但页签后覆盖依赖 scroll 自愈正确性——本轮验证脚本先暴露了
  产品缺陷本身（PTY 实证兼做缺陷探测，两个价值一次交付）。
- **尺寸不变量正则断言**：对每个已知总尺寸断言 `\[(HTTP|HTTPS|BT)\] \[(状态枚举)\] <size>`
  命中，比逐状态点查覆盖仿真动态（失败/重试/转做种随仿真漂移，状态不可预先钉死）；
  缺席断言用「无空格 x/y 特征串」（`GB/6.21 GB`）与详情面板「带空格 x / y」形态区分，
  同屏两种形态不误伤；未知尺寸任务的缺席断言锚 `MB/? B`（无空格）而非裸 `? B`。
- **跨行拼接陷阱**：终端整行 = 列表面板 + 详情面板同行拼接，子串断言在整行上做时
  须确认特征串跨面板无歧义（如 ` · 96.4 MB` 只会出现在列表行 3，详情面板为带空格
  形态）。

## v1.51 页签状态快照还原轮（2026-11-11，specifier）追加
- **PTY 帧对断言法（还原类实证）**：还原行为断言需「切走前帧 vs 切回后帧」逐项对账——
  脚本按时间窗缓存全部显示行帧（latest_in(lo,hi) 取窗内末帧），对比列表计数行（k/y 项）、
  光标 ▌ 行号、首可见任务行的协议+状态+尺寸 token 三元组；速度/进度/spinner 随仿真漂移，
  禁做整屏文本等值断言。
- **pyte 显示行 CJK 带字间空格**：标题「下载任务」在 pyte 显示行呈「下 载 任 务」形态
  （CJK 渲染逐字空格间隔），标题行定位不能按连续 CJK 串匹配；改锚面板标题行的计数徽标
  `\d+/\d+ 项`（首个命中 = 列表标题行，位于并发面板标题行之前）。
- **还原实证的确定性键序设计**：还原断言需要可预知的目标态——Home 回顶 + 定数 ↓ 构造
  「计数 3/10 项、▌ 行号固定、scroll=0」基准态；差异制造臂（切走期间对目标页签
  End/Home/↓）保证「还原值 ≠ 目标页签自然态」，防还原断言被「恰好没变」假阳性通过；
  状态 A/B 两次往返覆盖重复快照与不同还原值；时窗避开任务状态迁移（~2.2s neovim 移页签
  后集合成员稳定，win11 重试/再失败均不换页签，计数 k/y 可硬断言）。

## v1.52 页签快照还原产品实现轮（2026-11-11，coder）追加
- **TDD 红阶段失败消息即缺陷指纹**：三守卫红点与三缺陷形态一一对应——共享 selection
  选错任务（Some(5)≠Some(3)）/ 纯索引选中错位邻居（Some(4)≠Some(3)，操作者报告形态）/
  越界无夹紧（selected=9/len=9）；红形态核对无误即覆盖成立，绿后无须人工推演守卫覆盖度。
- **防视口吸附断言用「选中离屏态」构造**：scroll 原样还原守卫取 scroll=4 > selected=2
  （滚轮只移视口不动选中的合法态，合法性前提 flen−vis ≥ scroll）替代「scroll=0 还原」
  ——scroll_to_selection 类吸附回归会把 scroll 吸回选中窗（≤selected），离屏态断言
  才能抓住吸附回归；scroll=0 形态对吸附回归天然免疫（吸回后仍 0）。
- **bin-only crate 测试目标运行口径**：cargo test --lib 报 no library targets（单元测试
  挂在 bin 目标）；cargo test --bin a --bin b 多值混跑遇无测试目标（如 ezr-proxy-certs）
  整体 exit 1——逐目标跑各自收数；全量 cargo test 管道超 600s 会话超时丢输出，落盘
  .work/tmp + --tests --no-run 预编译 + --test 分块（3 块 ≤150s/块）是唯一稳态。

## cleaner 收敛轮（2026-11-12）追加
- CRAP 受限近似分析器两坑（自制分析器首次投产实录）：① lcov FN 行区间切割必须按「非闭包 FN 起始行」定界且切片用源码行列表（先 "\n".join 再按行号切字符位是错位 bug，comp 全错、on_evt 报 comp=2 假象）；② 闭包 FN 记录（mangled _RNC*）不得切分外层区间——否则大分发表被切碎成 comp 1-2 的假低值（815 函数 0 超 6 假阴性），闭包并入外层后 run comp=110/on_evt comp=32 现形；零发现结论先跑阳性对照（cmd.rs 历史 cmd_complete_path 家族可见）再采信。
- llvm-cov 幻影未覆盖行第三形态：**普通 if 块的收尾 `}`**（body 已覆盖）同幻影（app/mod.rs:528、task.rs:279/292/293 复核均幻影——restore/pieces 实际 cov=1.0）；幻影判据从「if-let 单行臂」放宽为「行文本恰为 } 且向上 5 行内存在已覆盖的块开启行（{ 或 => 收尾）」，不为幻影行造测试。
- 挂载收编增量逐套件实测口径：8 条新守卫（evt_arm_tests 6 + supervisor_tests 2）→ engine_backfill/g_appcore/g_engine/g_supervisor/g_v147/ui_backfill/v115 各 +8、g_throttle_err 仅 +2（只挂 supervisor_tests）、hardening/g_main/g_proxy/g_slots +0——挂载面按各套件 include 树差异，±N 对账必须逐套件实测不套「×N 套件」公式。
- spawn+watch 型函数（start_bt_task）可达覆盖上界与收敛组合拳：快乐路径（登记发送端/引擎起停/Cancel 干净退出无 BtFailed）单测可达；join Err panic 臂无产品面确定性触发（run() 全程优雅报错无 unwrap 面）——「纯函数提准收 comp」（join_panic_reason 三载荷 &str/String/未知直测 cov=1）+ 快乐路径单测把 CRAP 30→≤6，panic 臂留白属登记不属硬凑；引擎级单测构造用 shared_for_tests + make_meta 门面 re-export + 不可达 tracker（udp://127.0.0.1:1）+ Cancel 收尾 + try_recv 排水（evt 通道 256 死线纪律）。
- 夹具 DRY 收编两通道：跨测试模块重复夹具升 testutil 共享（bt_task：bt.rs 改委托一行、调用点零改动）；跨 crate 模块测试助手用「门面 re-export」（make_meta pub(crate) 于 tests 模块 + 父级 #[cfg(test)] pub(crate) use，零外部 API 增量）；收编后旧导入即死代码，clippy unused import 清单跟改。
- lint 警告清零的 nightly 漂移形态本轮实录：同版本号 nightly（1.102.0）不同 commit（9c5648833）引入 5 条 unused/mut 需要 lint（4 处测试死 setup files+pmap 构造后未用、1 处多余 mut）——「上轮 clippy 0」复核必须在本沙箱重跑才算数；unused 类是死代码免费信号，先查死因（本例：上一轮 coder 补字段时构造点成死 setup）再删。

## architect 收敛轮（2026-11-12）追加
- 判定表全域属性先例（conn_flags 8 臂）：逐臂语义 oracle（we_want 单行重推 `covered.iter().any(|&p| !done.contains(&p) && has.contains(&p))`）+ 结构断言（字符域 ⊆ 判定表字符集或单 "-"、旗标不重复、ORDER 表 windows(2) 严格递增——一箭双雕锁顺序与互斥臂唯一）；表驱动重写的属性锁定形态，比固定值单测覆盖全域组合。
- 跨模块属性测试可达性选 fn 门面而非类型门面：flags_case 升 pub(crate) + `#[cfg(test)] pub(crate) use tests::{flags_case, make_meta}` —— ConnStat 字段是 pub(super)，跨子树 re-export 类型名不给字段访问权（构造仍不可见），暴露「测试包装 fn」才是正确粒度（零产品 API 增量不变）。
- 同一工具族两属性域不同，域按属性语义选不按工具选：strip-identity 属性域排除 VS15（输入自身含 VS15 时 strip 后 ≠ 输入，变换只保证「自己插入的不丢」）；幂等属性全域成立（插入位被 VS15 隔断二次零增量）——break_links 一族两属性两种域。
- prop_assert_eq! 按值移动 E0382 再实录：同值多断言场景比较断言改 prop_assert!(a == b, "…: len={} len={}", x.len(), y.len()) 借用形态，或先借用断言后消费断言排序；rust.md 既有条款第三次踩中，断言消息统一显式位置参数。
- 属性分层对账口径续：bin 1028 = 942 单测 + 86 属性（独立命令 `cargo test --bin ezr property_tests::` 86P/0F；+4 全部落在 property 层，单测基线 942 不动）。

## QA 终验轮（2026-11-12）追加
- **显示退役类修订的观察锚漂移是 QA 套件假红高发位**：v1.49 把明细行尺寸对/「未知」项整项退役后，规程行与 runner 断言未同步的锚三例连发（DE-15 等待行「未知」/MB-03 `1.0/1.6 MB` 尺寸对/TD-17 明细行 `↓ 速度 <dl>/` 锚）——修复模式 = 用例按实现口径断言 + docstring 偏差注记 + handoff 待批（待 specifier 修订 feature/规程行），产品零改动；新锚选取 = 同语义的存活可观察物（无预取 → fixture 请求日志；重算 → 任务行[状态]徽标后总字节+进度百分数；块内推进 → 头部「会话已下载」，同源 Evt::Progress 的 t.downloaded）。
- **proptest 随机探索中签「全量绿」探不到的臂 + 回归文件常驻守卫**：属性测试每轮随机熵，普通运行绿 ≠ 全域成立——llvm-cov 轮中签 prop_scrollbar_bar_geometry_monotone 最小失败输入（`page==0 && total>page` 时属性期待 thumb_pos=0，实现按几何 max=total>0 算出 2）；裁决法 = 实现+守卫层+单测三方对读（scrollbar.rs thumb_pos 几何合法、render/hit 的 page==0 守卫与 TD-20 显示条件一致、单测 stat_queued 同口径）→ 属性过宽非产品 bug，最小修复 = 拆臂（`total<=page` 保归零断言、`page==0` 只断 hit None）；proptest 自动落盘回归文件（proptest-regressions/）在修复后成为该输入的永久守卫，不删。
- **llvm-cov 全目标覆盖超单调用时限的逐目标跑+并集合并**：全目标单次 `cargo llvm-cov --lcov` 插桩测试运行 ~40min 超 10min 上限 → 逐目标 `--bin/--test <t> --output-path` 各产出 lcov（插桩编译一次后缓存，每次调用只跑该目标）+ 合并脚本同行取 max（=覆盖并集语义）；mangled 符号名跨编译单元不同 → FN 记录按 (start,name) 不去重不影响行级 cov；CRAP 受限近似计算脚本随产物流转 `qa/runners/crap_calc.py`（口径头注内嵌）。
- **属性文件改动的 mtime 编译域核验实例**：property_tests.rs 是 bin 模块（main.rs `mod` 声明、无 lib.rs、tests/ 零引用）→ 修改后 12 集成目标结果沿用有效（中断恢复 mtime 校验法前提核验的实证）。

## specifier 锚点对齐轮（2026-11-12）追加
- **移交待批清单是 QA 视角的最小集，接手角色必须做同形全查**：QA 终验轮移交「观察锚漂移 4 处」，specifier 接手后按同一退役形态（尺寸对/「未知」显示锚）grep 全量扫描 features/ + qa/，发现第 5 处同类漂移（05-ui-pieces 场景 06 未探测臂描述性括号「列表大小显示『未知』」——runner 断言行为锚不受影响，QA e2e 全绿故未入清单）——漂移清单的完备性判据 = 形态学全查而非移交清单照抄；超出清单的修订走「头部注记登记供否决」通道（审批改判轮先例），不扩权不改 runner。
- **锚点对齐轮的闭环形态与遗留尾注**：移交目标角色按移交口径落文档后，修订措辞与 runner 现行断言逐字对齐（断言先例 = QA 轮偏差注记），runner docstring 的「待 specifier 修订后对齐」尾注随即转为过时叙述——断言已一致、仅叙述陈旧，清理属 qa/runners 改动面（specifier 不改 runner），登记移交不阻塞——移交指针注释（「已移交 X 待办」形态）同此（规程措辞对齐轮 20261113 结案 QA 收敛轮 T/t 门控移交指针再证）；「语义零变更、仅观察载体对齐」的修订不递增 feature 版本号，头部注记锚定语义版本（v1.49）而非新版本。
- **mission 只读约束下的口径张力处理**：发现 mission 条款措辞（FR-01-116① 统计行 `<已下载>/? B`）与已批准退役口径（v1.49/QA-TD-28 列表统计行退役、详情保留）张力时——不移改 mission，登记 handoff 待批由操作者裁决；流程角色对 mission 的边界 = 只读 + 观察项上报，与「缺失时报告不得臆造」同族。**裁决闭环（操作者裁决轮实证）**：操作者 chat 显式指令「修改 mission」即宪法第一章「除非用户明确要求修订」例外成立，首角色出修订注记结案（mission.md v1.52 总注记 + phase-01 v1.51 注记 + FR-01-116① 行内注记三层落点，规格与 QA 套件零改动）；同轮对上轮「登记供否决」类注记做结案回填（保留 → 头部注记追加「操作者裁决保留，结案」；否决 → 回退注记与正文两处）——登记类注记不是终态，裁决后必须回填，否则下游读到悬挂审批态。

## QA 收敛轮（2026-11-13）追加
- harness 级 `QA_ONLY`（case_id 全名逗分隔，如 `QA_ONLY=QA-TP-05,QA-TP-06`）与套件级 `QA_TP_ONLY`/`QA_RB_ONLY`（_case_NN 号）双机制并存；全套墙钟 >600s 单调用上限时（throttle_proxy 10 案实测：TP-03 单案 202s、全套 ~750s）用 QA_ONLY 分段对账——整跑段（TP-01..04）+ 点名段（05,06,07,14,10,13）各自 `== 汇总 ==` 全绿后拼接即全套对账，两段日志分落独立文件防互覆，点名段结果计数按段口径计入不冒充单套整跑。
- CRAP 受限近似复跑口径简化：cargo-llvm-cov 0.9.1 标准管线本轮零工件挥发（沙箱对策手工管线无需启用）；`--skip property_tests` 可省时，但「与不跳过完全同数」经 QA 观察项清理轮（2026-11-13）同工具链复测证伪——带 skip 翻转一条 ACT↔REG 并 -10 行覆盖（详见该轮条目），精确对账须不带 skip。

## QA 观察项清理轮（2026-11-13）追加
- **CRAP 登记口径 = bin ezr 单目标全量 1028 单测（无 skip）**：多目标 lcov 并集（12 集成测试 `#[path]` 收编 src 模块 + 3 辅助 bin 自带 cfg(test)）会把行清单膨胀至 41446（+1887）、ACTIONABLE 漂至 106——那是另一口径，不是登记基线；跨轮对账只在 bin ezr 单目标口径内进行。本轮 37251/39559/103/36 与基线精确一致：分母清单逐行恒等，覆盖分子 ±2 行漂移 = proptest 随机探索噪声，REG 家族与登记完全对账。
- **`--skip property_tests` 省时可行但非覆盖恒等（修正 QA 收敛轮「完全同数」条目）**：带 skip=37239 覆盖/104 ACT/35 REG，不带 skip=37251/103/36——被跳过属性函数自身 cov<1 翻转一条 ACT↔REG 并 -10 行覆盖；精确对账必须不带 skip 复跑（属性测试运行仅 +55s，省时收益有限）。
- **harness `QA_ONLY` 过滤键是完整用例号**（`QA_ONLY=QA-TD-01,QA-TD-02,…`），传裸数字 = 过滤器全不命中 → 零用例静默通过（EXIT=0、P=0 F=0 S=0）——分段复跑先验首段 P>0 再续跑，勿把空跑当绿。
- **过时移交指针的清理面 = 移交目标角色**：「已移交 X 待办」形态注释随移交事项结案转为过时叙述，清理归被移交角色的改动面（本 QA 轮清 QA-TD-07 指针）；同形全查中「历史缘由注释」（修订已落地）与「协议登记注释」（待操作者裁决偏差）不属过时态，维持不动。

## BT 顺序下载/做种退出规则/显示缩写轮（2026-11-13，specifier）追加
- demo 做种演示任务的 seed_left=0 会被 sim 首 tick 立即收割转「已完成」（`seed_left -= dt; if <= 0 → Completed`）——做种形态的 PTY/演示验证必须给 fixture 正 seed_left（本轮 v[2].seed_left=1800.0，与 docs 示例 29:56 同源）；单元测试不受影响（fresh App 未跑 tick）。
- 做种任务在「已完成」页签（`is_done` 含 Seeding）——「正在下载」页签里上下滚动找不到做种行属页签归属、不是渲染缺陷；PTY 断言序 = 先按 `]` 切页签再扫描，C/B 段之间用 `[` 切回（页签快照还原保证往返无损）。
- 对话框焦点环插入式扩展（新增字段行）的同步面清单：nfocus / trackers_focus / proxy 下拉 Tab·BackTab 跳转 / enter 焦点表注释 / 按钮焦点索引 / reject_bad_trackers 焦点回跳 / Dialog 构造点 ×N（漏一处 = E0560 编译错或默认值假绿）/ mouse 字段点击映射 / 键层早退分支（复选项 Space·Enter = 切换语义必须在 Enter 分发前短路，否则 enter 表 else 臂兜底成确认）。struct literal 内引用 `t.field` 与同函数 `&mut self.xxx` 冲突（E0502/E0506）——标量提前拷贝再入 literal。
- PTY 断言「其余行不受影响」类检查要考虑添加任务后 set_filter 跳转 + 选中跟随会把视口拉走：先 scroll_until 回目标锚再断言，别在跳转后的视口里做缺席断言。
## v1.53/v1.54 产品实现轮（2026-11-13，coder）追加
- 计时型 e2e 断言依赖 RST 异步回达（graceful_close 对端写失败）时，单发写入判定间歇假红——改为有界重试窗（≤2s @10ms 持续写直到首次报错），断言「最终失败」性质不与内核回包竞速（沙箱负载抖动实测一绿一红）。
- MultiEdit 部分应用实录：old_str 不匹配报错时先前编辑已落盘（非原子）——每步编辑后 grep 核状态再续，不信「失败=零变更」。
- e2e 套件 fixture 漂移归因：suite 内嵌旧焦点链（树到按钮 6×Tab）与旧行数基线（模板 190 行）撞 v1.21+ 规格 = 套件面欠账非产品回归；coder 不维护 QA 套件（SKILL 边界），登记 handoff 移交 QA；判据 = 套件 docstring 内嵌的 UI 结构事实 vs 现行规格焦点表逐项对照。
- 帮助/模板类「行数+计数」断言群（lines().count/取值范围 matches/拒绝启动 matches/End 钳制值）是模板与 help 修订的必查翻转面；lines() 与 split 计数差 1（EOF 单换行），钦定文本「207 行」= split 口径，lines() 断言写 206。

## v1.54 复核重验轮（2026-11-14，coder）追加
- 剥头副本漂移实测归因：`gen_fixtures_match_product_sources` 红 = fixture 侧手改 `sequential: false` 少尾逗号（src 侧经 cargo fmt 有逗号）——cargo fmt 不覆盖 include! 的 tests/gen/*.rs（根因条目见 packs/_common/notes/rust.md），双侧手改必漂；修复 = 按契约测试的剥头变换（跳过起始 `//!` 行后逐行相等）机械重生成 4 份副本，diff 预验零差异后复跑目标即绿（958P/0F）。重生成单行模板：`awk 'BEGIN{h=1} h && /^\/\/!/ {next} {h=0; print}' src.rs > fixture.rs`。
- 总账对 ≠ 分项对：上轮 handoff 分项目标计数 6 处归属错位（928/951/797/500/16/2 六个数字在 v115/g_v147/engine_backfill/g_throttle_err/g_proxy/g_main 间错挂）而总数 8100 恒等——复核轮以逐目标实测为分项权威口径，实测对账按「总数先对、分项再核、错位修正入账不改总数」处理，分项数字必须能从逐目标日志复算。

## v1.55 PEERS/SEEDS 核证修订轮（2026-11-14，specifier）追加
- demo clippy 基线口径：历轮「clippy 零警告」疑似 bin-only 口径（--all-targets 下测试模块 needless_mut 5 处为既有）——本轮顺手修为不可变绑定后 nightly --all-targets 零警告；后续轮一律 --all-targets 口径声明。
- verify_trackers_popup.py T2/T3 共 3F 为既有失败（git stash 前后同 3F 归因，方法沿 bug25 轮先例）：「T2 弹窗打开」「T3 修改对话框打开」，疑 v1.51 焦点环扩位/沙箱 PTY 时序漂移，与本轮 diff 无关；移交后续轮修复（判据面 verify_v153.py 28 项全过不受影响）。
- 显示层取值减法（y1 = peers − seeders 饱和减法）的字段语义保真判据：显示面减法与数据面重定义观测等价时，选不动字段的方案（trackers 弹窗等共享字段读者零波及）——D80④ demo 承接口径实录。

## v1.56 D80 裁决落实轮（2026-11-14，specifier）追加
- 字段语义对齐替代显示层换算（D80④ 修订实录）：操作者裁定「demo SEEDS/PEERS 定义与产品一致」——上轮「显示层减法保字段语义」判据被否决回退，demo seeders=x1/peers=y1 直存直读；经验：显示面换算层是需要向每个读者解释的额外间接层，操作者偏好「字段语义即显示语义」的单层定义（D80④ 新旧口径对照存 handoff/phase-02 v1.24）。
- 字段语义变更的共享显示面联动排查：改 seeders/peers 字段语义后逐个排查消费方——Trackers 弹窗 per-tracker 假值原锚连接值，做种态 x1=0 会退化弹窗数据，改锚 tracker 报告值（tracker_seeds/tracker_peers，fallback 连接值）；守卫测试与 verify 判据同帧同步，PTY C1 实屏复验（0(45)/35(120)/2.418）。
- 样例数据语义化：做种臂 fixture 从「渲染样本」（任意可显示值）升级为「语义样本」（seeders=0 呈现做种态 SEEDS=0 而 PEERS 有数）——演示数据应贴合其声称模拟的真实场景语义，避免观者误读口径。

## FR-02-37 v1.3 产品实现轮（2026-11-14，coder）追加
- 测试目标口径对账：全目标 16 = 4 bin（ezr 1040 + fixture 30 + mktorrent 1 + proxy 15）+ 12 hardening 集成；coder 单测基线口径 13 目标 = ezr bin + 12 集成（本轮 8100 与历轮基线总数精确吻合），3 工具 bin 46P 不入基线——总账对账先分清口径再比数字。
- nightly 版本带漂移 lint 新实录：2026-11-14 构建 unnecessary_to_owned 新覆盖 `.contains(&"字面".to_string())` 形态（产品仓两处既有测试代码报警，昨轮 0 警告基线的漂移源）——先归因「工具链漂移非回归」再动手，`iter().any(|i| i == "…")` 等价改写；注意 `Vec<String>::contains` 需要 `&String`，直接传 `&str` 是编译错。
- ENOSPC 处置新面：集成测试二进制落 `target/debug/build/<pkg>/<hash>/out/`（两层通配删除已完成目标的 out 目录 + incremental，保留 deps 编译产物免全量重编，单次释放 4G+）；bash 层级 glob 少一层会静默删不到东西，删后必 `du`/`df` 复核。

## FR-02-37 v1.3 收敛轮（2026-11-14，cleaner）追加
- 覆盖率直采管线（cargo-llvm-cov 自动合并不可信时）：`LLVM_PROFILE_FILE=<dir>/%p-%m.profraw` 直驱每个单测二进制 → llvm-profdata merge -sparse → llvm-cov export <bin> --instr-profile --format=lcov（2026 LLVM 已弃 --lcov 单旗标）；逐二进制 lcov 交 crap_calc.py 多文件并集。cargo-llvm-cov 自动模式三坑实录：四个不同 --test 目标产出 md5 相同的 lcov（g_* 变体共享覆盖映射、profraw 名以包名前缀碰撞）、profraw 跨调用累积（--no-clean 语义）、单测二进制落在 OUT_DIR 会被后续调用清理——直采法全部规避。
- profraw 归属与组合矩阵污染：多次直采复用同一 profraw 目录时，旧二进制（重构前）的计数与新二进制按函数名哈希碰撞合并 → llvm-cov export 对新映射产出「已覆盖臂显示未覆盖」的假缺口（本轮 L363/L275 假未覆盖实录）——复采前必须清空 profraw 目录，A/B 归因先锁 binary×profraw 组合矩阵。
- 死臂删除的验证闭环：删后先跑目标过滤单测（行为不变），再以干净 profraw 重出 lcov 核对缺口消失；crap_calc 的 ph=N 幻影列（未覆盖 `}` 收尾行）随死臂删除自动归零，勿把幻影行当真实缺口追测。
- 挂载套件增量对账不均匀形态：src 新增 10 测在 12 集成套件的挂载增量 = +10/+8/+8/+8/+0/+0/+0/+8/+3/+8/+10/+10（按模块归属分布，非历轮「各 +N」均匀形态）——对账以「每套件恰好收编其含模块的测试」为准，总数复算 8146+10+65=8221 自洽即过，勿强求各套件等增。

## FR-02-37 v1.3 架构评审轮（2026-11-14，architect）追加
- wrap_segments 属性的「分区连续」断言过界实录：首版断言"段起点连续无空洞"被 shrink 最小输入 `"一aAA0A一一A  a 一aa", width=2` 揭穿——真实契约 = 断点空格与其后空格串作为分隔符**丢弃不渲染**（cleaner「断点+空格跳过」单测正是此语义），非分区覆盖全字符域。正确形态 = 双侧空格校验：段间跳过域必为空格 + 尾部未覆盖域必为空格（`for (idx, c) in chars.iter().enumerate().take(start).skip(cursor)` / `.skip(cursor)`），配合段长 ≥1、段界不越域、段宽 ≤width（单字豁免）。写属性前先核对被测函数的「有意丢弃」语义——覆盖类断言只对"零损耗"函数（如 wrap_text）成立。
- scrollbar hit 全域分类属性模板：面板下限保证 track≥2（垂直 height≥6 / 水平 width≥10）+ prop_assume!(page<total) 锁可滚动域 → 全线扫格 [a,b] 分类对账：Head/Tail 仅在箭头起点/终点、Thumb(i) i<th、Track(i) i<tl、计数恒等式 seen_thumb==th 与 seen_track==tl−th（ Thumb 格数=thumb_len）+ 三向越界静默（偏轴线/箭头外侧前后）——「分类器全域」属性用计数恒等式锁分类完备性，比逐点断言覆盖更紧。thumb_len 域补 th≥1 与 th<tl 两条几何界（可滚动域）。
- prop_assert 环境的 clippy 机械修两形态：int_plus_one（`th <= tl - 1` → `th < tl`）与 needless_range_loop（`for idx in range { vec[idx] }` → `for (idx, c) in vec.iter().enumerate().take(end).skip(start)`，skip/take 链保 idx 供断言消息）——属性测试代码同样受 --all-targets 0 警告门槛约束。

## FR-02-37 v1.3 变异加固轮（2026-11-15，hardender）追加
- cargo-mutants 27.1 契约要点：无 --test-command（`--cargo-test-arg` = cargo 侧参数、`--` 之后 = libtest 侧过滤器，`-j` 类 cargo 参数放 `--` 后会传给测试二进制报 Unrecognized option——environment.md 同款陷阱在本工具再现）；`--baseline skip` + `--copy-target true` 组合下依赖指纹全复用（副本冷构建 6-9 分钟 → 零）；`--build-timeout` 默认值（约 120s）小于本项目 ezr crate 单文件重建时间时每突变体被 SIGKILL 误分类为 unviable——表象是「全部 unviable + 日志尾部 signal: 9」，必须显式加大；`-j 2` 双 rustc 并行编译超内存（4G 沙箱）同样 SIGKILL 全体 unviable——单 worker 是唯一稳态。
- 单 crate 大项目变异提速正解 = 持久 scratch 副本（.work/tmp）+ CARGO_INCREMENTAL=1 + 每突变体 git apply diff → cargo test 过滤器 → 从 pristine 还原单文件；冷构建一次（~2 分钟含依赖复用）后续每突变体 50-110s；对比每 shard 冷 scratch 方案（每调用重付冷构建）吞吐差 3-5 倍。窄过滤器（模块名前缀）先杀 + 存活体全量单测复验 + 加固测试杀灭验证三段式，杀灭证据闭环（known-answer 探针：杀灭日志须有真实断言失败）。
- 集成测试目标（tests/*.rs 经 #[path] 内联产品模块）编译时 cfg(test) 处于激活态：产品模块的 #[cfg(test)] 单测在集成目标内重复运行（hardening_g_engine 的 818 含产品单测）——新增集成目标的计数含重复面，对账时按「每目标实测值」口径不要按自测数推断。
- pub(super)/私有方法不可从集成目标直调（pub(super) 锚定在 app 模块内）；对话框按钮臂（dlg_activate_add_bt 等）正解走真实事件路径：键盘 = `App::on_key`（pub）+ 设 d.focus；鼠标 = ratatui TestBackend `ui::draw` 渲染登记热区（dlg_btn_rects）→ 合成 MouseEvent 点热区中心——不修改产品源码也能覆盖 pub(super) 变异臂。App::new 自带「就绪」欢迎 toast：断言 toast 内容用 contains 特定文案，不要断言 is_none。
- 变异存活体的诚实分类三分：等价类（m077 ignored 去重无观察消费方/m097/m098 边界防御钳制不可达）、覆盖缺口（btmod 7 个：私有常量钉扎不可达、时序/e2e 时域观察点缺失——登记失效条件与回补触发，不冒充等价）、待复验队列（trackpool ws 面 26 个）。「非等价存活率必须为 0」在本轮以「覆盖缺口显式登记 + 加固测试杀灭验证」方式兑现，不降格为等价论证。
- gherkin-mutator（APS bb 版）runner 适配器最小实现：NDJSON 持久 worker（python）即可全量跑通 20 feature / 1123 突变；`--level soft` 差分复用会合并历史 manifest 条目（implementation_hash 变化不阻断 soft 级复用）——01-persistence-config 的 2026-10-06 旧条目因此被部分保留；强制全量重跑 = 删除 feature 头部 stamp/manifest 注释块（SKILL 既定口径，本轮实测确认）。

## FR-02-37 v1.3 变异加固轮续（2026-11-15，hardender 补跑段）追加
- 双面过滤器路由法（308 突变体吞吐从 ~144s/条 压到 ~25-50s/条的关键）：bt::engine 73 单测实测轻面 27 组名过滤器（52 测 pristine 14.74s）+ 重面 17 组名（21 重 e2e 测 pristine 35.24s），全集 = 轻∪重；先按突变体函数面路由（纯函数/池管理→轻面，协议消息/e2e 下载→重面），轻面存活体再补重面——补杀收益实测 pieceio 60 存活→16 补杀、peerwire 27→2 补杀、trackpool 8→0。多过滤器语法坑：cargo 侧只收一个位置 TESTNAME，多过滤器必须放 `--` 之后（`cargo test --bin ezr -- f1 f2 …`，libtest OR 语义）；`cargo test --bins` 会跑 3 个 bin（ezr/ezr-fixture/ezr-mktorrent），`--bin ezr` 单 bin 口径免 fixture/mktorrent 噪音。
- 超时窗口与「caught-by-timeout」判定纪律：MUT_TIMEOUT 必须大于「被杀突变体失败测试烧内部超时」的最坏耗时（重面 killed 实测 177s > pristine 50s——失败测试各自等满内部 deadline）；backfill+supervisor 双套件纯耗时 ~253s 贴 260s 窗口边缘时，timeout 判定不可信（本轮 046/047 首判 timeout、560s 重跑翻案 survived）——timeout 与套件裸耗时可比时必须放大窗口重跑；轻面（pristine 14.7s）下 260s timeout 且日志有 "running for over 60 seconds" 才是真挂起=caught-by-timeout 杀灭（pieceio_043/072 判例）。
- 存活体跨 pass 去重口径：同一突变体经多 pass（轻面→重面→集成面）复验时，汇总账必须按「最新判定」取值（naive 并集会把「早期存活、后被补杀」的 18 条误计入存活，97 vs 79 实录）——判定权威 = 结果 CSV 的最后一次出现。
- pub(super) 引擎内部件的全量单测复验替代口径：trackpool/peerwire/pieceio 三文件自身零测试、全部测试在 mod.rs 测试 mod（bt::engine::*），非 engine 单测（app/ui/http/config）经 fake/测试替身不触真实引擎内部——「存活体全量单测复验」以「bt::engine 全 73 测两阶段跑满」替代，集成链补 hardening_g_engine；非 engine 单测面豁免理由 = pub(super)/private 可达性边界（与 bt_dialog 轮「真实事件路径覆盖法」互补）。
- 杀灭证据的 panic 位点多样性：trackpool announce_url 突变体被 conn_views 测试杀（mod.rs:2326）、peerwire handle_peer_msg 被 corrupt/illegal/unknown 三 ban 测试杀（mod.rs:4566/6330/6334）、pieceio write_piece 被 sidecar/下载测试杀（mod.rs:1243/1234/2805）——窄面过滤器的杀灭位点落在邻接模块测试是常态，过滤器组名不必与被测函数同前缀。

## FR-02-37 v1.3 变异加固续轮二（2026-11-16，hardender）追加
- runner 简化 verdict 标签的假分类陷阱与账目改判：unit_one 类脚本的三分逻辑（exit=0→存活 / 无摘要行→unviable / 其他→dead）会把 timeout（exit=124、被杀时无 libtest 摘要）误标成 UNVIABLE——账目复算必须以 exit 码 + 日志挂起证据改判，不信 runner 标签（pieceio_072 判例：300s 窗内 bt_conns_mark_full_bitfield/engine_connects_all_tracker_peers 双测试 >60s running 后被杀，与上轮 caught-by-timeout 同签名 → 改判超时杀，真 unviable 的判据是日志尾部「error: could not compile」）；另：日志头部「Blocking waiting for file lock on package cache」是并发噪音非失败信号，判读一律看尾部证据。
- 自适应混合流的「直接 BS 结案」账目口径：登记函数内突变体可跳过单测面重推导、直接经 BS 批次实跑 record ALIVE_BS 结案（本会话 79 存活 = 56 单测面重推导 + 23 直接 BS）——缺项风险靠「重推导集覆盖登记函数全集（非 N_A 全集）+ 函数级账目闭合（登记函数 235 = 149+6+1+79）」兜底；漂移守卫只报「超」不报「缺」，账目闭合检查不可省。
- 判定权威与 runner CSV 缺行：adaptive_resolved.csv 最新行是判定权威（同 id 多行时后者覆盖）；runner CSV（unit_runs.csv）允许缺行（同会话内另一 runner 直录或行丢失均可能，本会话 2 条 UNIT_DEAD 仅有 unit 日志无 CSV 行）——账目复算以 resolved 最新行为准、以逐突变体日志为证据底座，二者交叉核验。
- BS 批次打包收益口径：bs_run.sh 批量打补丁（冲突检查 ±4 行 span）后单次双套件覆盖整批（23 批次 × ~260s 结案 79 突变体 ≈ 11s/条，对比单条实跑 ~270s/条），批内零杀灭 = 整批 ALIVE_BS；批失败 fail_split 整批回 POOL 重推导，不做批内二分（预期全绿场景下二分是纯浪费）。

## FR-02-37 v1.3 QA 终验轮（2026-11-16，QA）追加
- 焦点环插入的「五表同步」缺陷模式与 PTY 交叉归因法：对话框新增可聚焦节点（v1.53 按顺序下载复选）时，同步面是五张表——ui 渲染 indices（field_rects/buttons 元组）、dialog_keys nfocus、Enter 分发 arms、Char 分发 arms、路由单测的焦点常量；本轮实录 render+nfocus 已更新而 add_bt_dialog_enter/modify_bt_dialog_enter 停留旧 indices（立即下载 Enter 落暂停添加、确定 Enter 关闭不保存、取消 Enter 反而确认），单测全绿的漏网机制 = 路由单测同样编码旧 indices、行为级 walk 测试只断言「任务已建」（暂停臂同样建任务）。归因探针 = ACCENT 高亮读数与 toast 分支交叉验证（_focused_btn 显示与实际激活臂可分离，终态归因只认 toast 全文/registry state）。
- cargo test 不刷新 target/debug/ezr：cargo test --bins/--all-targets 只构建测试 harness 二进制，套件消费的 target/debug/ezr 可执行文件保持旧版——产品源码修改后跑 PTY 套件前必须 cargo build 显式刷新，否则「修复未生效」假象（本轮实录：修复后套件原样 4F，build 后 4P）。
- crap_calc.py argv 契约：单 lcov 输入时 sys.argv[1:-1] 为空、路径被吞作 src_root 判断位，静默 ZeroDivision——必须带 src_root 尾参（crap_calc.py <lcov...> <src_root>，单 lcov 时共 2 个路径参数）。
- PMD CPD 产品/测试划界的多测试区形态：同文件多个 #[cfg(test)] mod（engine/mod.rs 905 与 6191 双测试区）必须收集全部 tstart 划界，只取首个会把中段（4454-5006）测试区大克隆误判为产品侧——修正实录 53/145 → 23/175（阈值 70 全量组数口径）。
- 退役键/退役显示的 runner 翻转面三件套：①拒绝行改判（seed_duration_secs 退役后拒绝目标改 default_seed_duration_m=0，FR-02-40②）；②退役键正探针（退役键写入 → 启动成功断言，补 FR-02-35④ 静默忽略语义的观察面）；③退役展示断言翻转（剩余做种 → assert_not_in，数值断言随载体退役，机制面移交单测/fixture 观察）。规程/feature 残留旧行（08-MB-10 seed_duration_secs 表述、feature 场景 04 残留拒绝行）属 specifier 同步欠账，QA 记偏差不越权改。
- throttle_proxy QA_ONLY 分段实录：10 案 3-202s/案，500s 单窗只容 2-3 慢案（TP-01/02/03 合计 ~506s）；四段切法 3+3+3+1 全绿，段间 pkill mini_http_tracker/ezr-fixture 清场。
- 退役键口径同步轮（specifier 视角，FR-02-40② 待批① 落实 20261116）：QA 登记的同步欠账是 QA 视角最小集，接手轮必须同形全查（沿锚点对齐轮先例）——grep 退役键名全库后按排除词清单（退役/废弃/替代/正探针/原口径/改判/历史/注记/runner 已）滤掉退役语义行，余项逐条按 mission 权威条款改判（本轮 QA 登记 2 处、同形扫出 3 处滞留：08-MB 场景 10 Given/步骤倒计时表述、09-BE 场景 08 seed_left 步骤、QA-BE-08）；历史版本注记不回改（v1.10/v1.17/v1.43 注记保留原样，撤销痕 = 清单行加新版本痕 + 头部新版本注记）；feature Examples ↔ runner rows 元组正则对账（唯一分歧 = 已登记偏差 common.max_speed 沿 QA runner 注记待裁决，不越权改行）；doc-runner 组成差异只登记不越权补（QA-PC-12 有规程行无 _case_12、QA-PC-10/11 规程行与 runner 用例双缺——补规程属 QA 可执行化职责面）；清单缺场景行补登 + 文档场景计数校正属卫生面可顺手修（01-PC 清单漏场景 14、QA 文档 13→14）。
- 退役键拒绝行结案轮（specifier 视角，max_speed 待批① 裁决 20261116）：登记类偏差的处置选型锚 = QA 已同步 runner 行集（行集不含退役键亦不含替代键拒绝行 → 删除而非改判——改判 = 新增覆盖超出批准范围需另批；行集已含改判行则沿 runner，FR-02-40② seed_duration_secs 先例）；登记类注记结案回填按面归属分流（规程/feature 注记本轮回填结案态，runner .py 注记沿 20261112 先例归 QA 面）；同形全查滤网后余项按「语义 vs 标签」分层：退役语义行（判据断言退役口径）= 口径改判，纯键名标签（默认行为描述/判据键名，语义零变更）= 标签清理顺手改；[/] 改写敏感序列 new_str 全程避开 + 编辑前后 ttp]/[http] 字节计数比对（零附带损伤终审）。

## QA-PC-13 组成算术重述轮（2026-11-16，specifier）追加
- 组成算术类登记的锚选型：「runner 实写键集为权威」的前提是 runner 自身已同步现行口径——重述前必须先逐键盘点 runner 键集残留（本轮 _case_13 实写 38 项 = 缺 5 现行键 global_max_upload_speed/task_max_download_speed/task_max_upload_speed/listen_port/default_seed_duration_m、含 1 退役键 seed_duration_secs），runner 自带残留时不可字面照抄（照抄会把退役键写进「全量合法值」清单，与 v1.21/v1.24 退役口径矛盾）；同理，重述锚的终点也不是 feature 算术（其分解同样可漂移：v1.20 起「+ proxy 双条目 + [bt] 21」两项偏差恰好抵消合 40）——最终权威 = 模板活跃键集（43 活跃行 = 40 键值 + 3 表声明，suite_config_template 逐键断言锚）→ 40 = [common] 6 + [http] 11 + [bt] 23、proxy 双条目为可选附加面不计入；runner 覆盖差登记移交 QA 面补齐（「规格随实现」与「登记不越权」两先例的合成用法）。
- 判据列随 runner 键集补齐的联动写法：补列新实写键的判据一律引用既有同款断言先例与承载面（预填断言引 suite_add_task QA-01-20 正则口径、生效断言引承载套件 suite_modify_bt announce 面/TP-08——沿规程通过准则「调优键承载面」写法），不发明新断言语义；default_seed_duration_m 判据随实写非默认值由「默认臂」改「写入值臂」（判据臂必须跟随 runner 实写值形态，否则文字与断言脱钩）；规程操作列与 runner 实跑流程的既有差（「完成一次 BT 下载」vs 实跑仅 HTTP 下载完成 + BT 对话框预填观察）不顺手改规格也不顺手扩 runner，登记观察待操作者裁决。
- 全量冒烟 cfg 的逐键审读纪律（QA 同步轮实录两项追加发现）：①cfg 键集对账必须逐键比对模板 40 键清单而非数总数（总数相等都可能分解错，QA 按模板拨正 spec 面分解 6+11+23）；②既存 cfg 行也可能写默认值违反「非默认」口径（peer_connection_protocol ["tcp","webrtc"] 即模板默认，修正为合法单值 ["webrtc"]——单值合法/大小写规范化/重复项拒绝边界以单测 peer_connection_protocol_validation 为锚），逐键默认值比对是全量冒烟用例的固定审计步。

## 01-PC 可执行化扩面轮（2026-11-16，QA）追加
- peer_connection_protocol 单值臂与 BT 下载流程的互斥：["webrtc"] 臂下 ezr 无跨协议回退，与 libtorrent seeder（TCP only）永远握手失败——BT 端到端下载流程（tracker driver 基建）必须用 ["tcp"] 臂（合法单值且非默认，≠ 模板双值）；全量冒烟用例补真实下载流程时先审「传输协议臂 vs seeder 协议能力」相容性，不是任意合法非默认组合都可达成端到端。
- kill -9 哨兵恢复序列的断言窗口：EzrApp.kill9() 的 waitpid+close 时序不 pump，哨兵 RESTORE_SEQUENCE 写入后随 master close 丢弃——恢复序列断言必须自驱 pump 循环等 reap（exit_code 就位）再读 raw_text；且全流 raw tail 含启动时 crossterm 开启序列（?1000h 等），「无残影」断言须锚定 kill 后新增尾流段（base 长度切片），不能对全流断言无开启序列。
- 秒传文件的中间态断言失效：small.bin 本地下载 <1s，「下载中」徽标一闪而过不可稳定观察（历轮「断言锚定最终态常驻文本」条款的用例内实例）——小文件用例一律等「已完成」终态；要观察下载中态须 fixture ?speed 门控（TP-01 口径）。
- 跨套件基建复用的最小改动面选型：suite_modify_bt tracker driver（_Tracker/LtPeer/mktorrent/_tab_to_button/_read_driver_announces）供 01-PC 复用时直接 from suite_modify_bt import（模块级纯常量+函数、无副作用）——比上提 harness 改两个文件小一个量级，suite_modify_bt 本身零改动零重验；DRY 上提（共享基建入 harness）登记归后续 cleaner/QA 收敛轮。
- swarm complete 清 target/ 的跨会话连锁：归档即删构建缓存——下一会话跑 PTY 套件前必须先 cargo build（rsproxy + nightly 钉扎口径 2.5 分钟分块前台），套件绿基线不跨会话成立（QA-PC 基线 P=11 在 complete 后直接跑 = ezr-fixture FileNotFound）。

## ezr-demo verify 脚本陈旧化与 PTY 换行不可靠（2026-11-16，coder demo 专项轮）追加
- **ezr-demo verify 脚本陈旧化三形态**：demo 经 41 轮变更记录演进，PTY 级 verify 脚本（verify_trackers_popup.py / verify_size_move.py）相对当前源码陈旧，失败面分三类：①硬编码几何断言（ trackers 弹窗宽 90→94，源码 TRACKERS_W 常量已改）；②标签文案换代（l3 明细行「分享」→「SR」、「剩余」→「LT」缩写化 v1.21/FR-02-38）；③数据值口径变更（健康度三段 D80④ 修订后带 tracker 括号形态「0(45)/35(120)/2.418」与「8(14)/15(31)/1.675」，旧脚本仍断言无括号「12/35/2.418」「8/23/1.675」）。修订时先跑脚本复现失败面，再对照源码（ui/popups.rs 常量、ui/list.rs l3 构造、app/demo.rs 任务数据）逐项对齐，勿凭脚本失败信息猜口径。
- **PTY 环境下 Alt+Enter / kitty Shift+Enter 不被 crossterm 解析为 trackers 换行**：verify_trackers_popup.py T3 原经 `\x1b\r`（Alt+Enter）在 trackers MultiLineBuf 追加 wss 新行触发拒绝，当前 PTY 环境下 `\x1b\r` 与 kitty `\x1b[13;2u`（Shift+Enter）均不产生 trackers.newline() 调用（crossterm event-stream 在 PTY 下的 escape 解析时序与真终端不同）。变通：清空 trackers 既有内容（退格×80）后键入 `wss://bad.example` 作唯一行，再 Tab×2 焦点转「确定」后 Enter 确认——tracker_reject 命中 wss 行即触发拒绝 toast，绕开换行键不可达问题。

## ezr-demo trackers 焦点修复后 T3 测法改键盘导航（2026-11-16e，coder 续轮）追加
- **补记（T3 测法升级）**：焦点模型修复后（见 architecture.md 同轮补记），T3 从「鼠标点击 trackers 高列位 + Tab×2 至确定」（依赖 sequential 复选框命中区与 trackers 区的重叠、坐标脆弱且修复后点击落 sequential 不再送 trackers）改为「Tab×6 键盘导航至 trackers（focus 0→6）+ End 归末行尾 + 退格×80 清空 + 键入 wss 单行 + Tab×1 至确定（6→7）+ Enter」。键盘导航不依赖鼠标坐标、不受布局漂移影响，且焦点修复后 Tab 落 trackers 时 End/Home/退格/char 全部正确响应。PTY 下 Alt+Enter 换行键不可达问题仍在（crossterm escape 解析时序），故仍用「清空+单行」绕开换行，但焦点层面的死锁已消除。

## ezr-demo 焦点修复根因更正与鼠标点击验证（2026-11-16f，coder 三轮）追加
- **根因更正**：上轮 T3 改 Tab 键盘导航后两连绿，但鼠标点击 trackers 仍不能输入（用户实测反馈「焦点停上面复选框、tracker panel 无法输入」）。调试输出 dump `dlg_field_rects` 实证：trackers rect 带 idx=5（应 6），sequential rect 丢失——根因是 UI 层 L623 局部变量 `trackers_focus`（7/5）与 Dialog 函数（8/6）不一致（见 architecture.md 同轮条目）。修 L623 后鼠标点击 trackers 正确落 focus=6，char 进 trackers ✓（ModifyBt + AddBt 双对话框探针验证：点击 trackers 内容行 + 键入字符 → 字入 trackers 缓冲）。
- **调试输出写文件避 PTY 污染**：PTY 下 stderr 混进 master 端会污染屏幕渲染，临时调试用 `std::fs::write("/tmp/dbg.log, prev + &new)`（read+write append 模拟，非原子但测试够用）dump 内部状态到文件，探针跑完读文件分析；调试代码验证后即回退。eprintln 在 PTY 测试里不可用（会被 MiniTerm 当终端序列喂回污染屏幕）。

## PTY Alt+Enter 不可达定性收口（2026-11-16h，操作者实测确认）追加
- **定性收口**：操作者真终端实测 ezr-demo 添加/修改 BT 对话框 Alt+Enter 换行键可用——证实「PTY 下 Alt+Enter / kitty Shift+Enter 不被 crossterm 解析为 trackers 换行」是 **PTY 测试环境限制**（crossterm event-stream 在 PTY 下的 escape 解析时序与真终端不同），非 demo/产品缺陷。verify T3 保持 Tab×6 键盘导航 + 清空 + 单行 wss 测法（PTY 环境限制仍在，Alt+Enter 测法在 PTY 里跑不了），但不再作为待修受限项登记——移出 handoff 遗留受限项列表。
- **区分「测试环境限制」与「产品缺陷」的判据**：PTY 不可达 + 真终端可用 = 环境限制（verify 用变通测法绕开，产品不动）；PTY 不可达 + 真终端也不可达 = 产品缺陷（必须修产品）。本轮操作者真终端实测是判定关键——「PTY 测试失败」单独不构成产品缺陷结论，需真终端对照。

## 缺口 112 回补轮（2026-11-16，hardender）追加
- **杀灭证据闭环账目**：scope 清单 339（pieceio 209/peerwire 78/trackpool 52，对历轮 308 为口径漂移以实测为准）；覆盖函数（24+13 gap112 测试）140 突变体 = **137 杀灭 + 1 等价（ws 644）+ 2 unviable（count_invalid_request→Some(Default) 编译错、spawn_outgoing 删 Ok(Ok(s)) 臂工具判定）**；未覆盖函数 199 沿历轮登记（本轮纯增量测试不改其杀灭状态）。挂载对账三方精确：bin 1052→1089、backfill 938→975、supervisor 791→828（各 +37）。产品源码零改动证明：pieceio/peerwire/trackpool 逐文件 cmp 与 pristine 字节恒等，mod.rs diff = 1618 行纯测试模块追加。
- **engine 内部件回补测试的挂载法**：pub(super) 内部件的加固测试只能挂 `src/bt/engine/mod.rs` 的 `#[cfg(test)]` 兄弟测试模块（宪法「产品树新增只允许加固测试」口径；tests/ 集成目标触不到 engine 内部件）——新模块复用 `use super::*`（含 engine 对 conn/peerwire/pieceio/trackpool 的 glob 再导出），但 trackpool 私有 use 的 tracker 类型（PeerAddr/WsSession/WsAnnounceResponse/WsSignal）不随 glob 再导出，须显式 `use crate::bt::tracker::{…}`；mod tests 的私有 helper（ws_test_ctx/test_ul/shared_for_fill_test）兄弟模块不可见，自建 gap_* 系列。
- **一测一靶 + gap112_ 前缀 = 批量杀灭归因**：cargo-mutants `-F --re` 按函数名过滤 + face 过滤 `cargo test --bin ezr -- gap112_`（新测试名前缀）→ 单突变体循环 ~7.5s（CARGO_INCREMENTAL=1 + CARGO_PROFILE_DEV_DEBUG=0：增量 CGU 重编 + 零调试信息），339 清单中覆盖函数 140 突变体全程 ~10 段前台分段跑完。
- **cargo-mutants 27.1 三契约**：`--in-place` 与 `-j` 互斥（单 worker 稳态）；构建阶段固定 `cargo test --no-run --package=ezr` 编译全部 14 目标 → 4G 沙箱并行 rustc 链接 OOM（signal 9 → 全体 unviable）——**大目标隔离正解 = 持久 scratch 副本删 tests/ 目录**（bin ezr 自带全部单测含 #[path] 内联不可用；bin face 足以承载 gap112 杀灭验证）+ `-o` 把 mutants.out 外置 .work/tmp；`-F` 正则匹配 --list 全名（含 file:line:col 前缀，可按行号段切块）；**无断点续跑**（同 -o 目录重跑从头重测，分段必须单段跑完整块，块尺寸按 ~7.5s/突变体 × 预算反推）。
- **timeout 分段杀 runner 的清场次序**：timeout SIGTERM 后 cargo 子进程可能仍持锁/占 scratch → 先 pkill mut-scratch 进程组、再 cmp+cp 还原 pristine（cmp 防无谓 mtime 触发整文件重编）。
- **未覆盖臂的挖掘靠变异实证不靠猜测**：handle_peer_msg 50 突变体首轮 45 missed——Bitfield 臂（位掩码/索引算术/peer_gained 覆盖过滤/want 判定）、Request 服务臂（allowed/have/valid_len 三条件表 + cum_up 计账 + else-if 违规计数）、ext handshake v 字段、ut_metadata 切片算术（start/end 窗口）全无直测；回补 10 测后 47/47 杀灭。誤判教训：assemble_contiguous e7 用例的 cursor 手算错（buf 累加后 cursor=3 非 5），plen=5 双 None 假绿，plen=3 才使 mutant 落 Some——**杀灭用例必须先手推 mutant 路径的终值再落断言**。
- **ut_metadata 边界块（start==len）的构造法**：`<` → `<=` 突变体仅在 start 恰等于 info_bytes.len() 时可观察——固定 pieces 哈希（避免每迭代重算 SHA-1）+ 名字字段逐字节填充扫描（info_len 随 pad 单字节步进，16384 步内必命中整除点，bencode 编解码 ~200µs/步）。
- ** Have 臂语义双面**：`st.has.insert(p)` 无条件登记（covers 与否都进 has），order.peer_gained 才按 covers 过滤——测试断言「has 含未覆盖 piece + avail 不含」两段缺一不可（`&&`→`||` 突变体靠后者杀灭）；want 判定的 `!done` 删除靠「唯一覆盖 piece 已完成 → 不得发 Interested」的静默断言杀灭。
- **WS announce 计数服务器复用形态**：spawn_stopped_announce 的 Ws 臂杀灭 = tokio_tungstenite::accept_async 本地服务器计 action==announce 次数（wait 5s 轮询）；spawn_outgoing/incoming peer 的体替换（`replace fn with ()`）杀灭须正证据（完成握手 → Connected 事件），「无事件」类负断言对体替换恒绿（体替换=什么都不做=无事件）。
- **ws_process_response 644 `delete !` 等价论证（本面内）**：transports.webrtc=false 时 ws_handle_answer 的 pending_offers.remove(未知 id) → None → 早退，无任何可观察差异；构造可观察面需真实 WebRTC pc（new_peer_connection 沙箱未验证）——失效条件 = webrtc 栈可用的测试环境（届时 pre-insert PendingOffer + 假 sdp answer 走 mark_peer_fail 臂即杀灭）。

## 软 Gherkin 执行层基建轮（2026-11-16，hardender）追加
- **APS 执行层四件套落位（.work/tmp/aps/，会话运行时产物不入归档）**：runtime.py（IR 展开：Background 前置 + Outline 按行展开 + 步骤路由）、generator.py（acceptance-entrypoint-generator：入口脚本 + metadata/<slug>.json，implementation_hash 覆盖 generated_files）、adapter.py（NDJSON 持久 worker，mutator stdin/stdout 协议：job{id,feature_json,generated_dir,work_dir,timeout} → resp{id,outcome,output,error,duration}）、drive.py（20 feature 全量驱动）。执行语义双层：词汇面 = 步骤文本占位符解析 + 示例值语义校验（URL scheme 白名单/二进制单位尺寸限速/config 三表键集/数值界），不支持即 FAIL；产品面 = 步骤 handler 直驱真实 ezr 二进制（PTY harness 复用）。
- **gherkin-mutator 实跑契约**：`bb gherkin-mutator --feature <abs> --work-dir <dir> --generated-dir <dir> --level soft --runner-worker "python3 adapter.py" --status-interval 30s`；soft 级 20 feature / 1121 突变体全量执行 ~15s；差分复用会把 stamp/manifest 注释块写入 feature 文件（工具自管理注释，宪法允许）——二轮跑同 feature 显示 skipped_mutations 即复用首轮判定，强制重跑删 feature 头部注释块；metadata feature_path 必须与 --feature 路径一致否则复用判定漂移。
- **IR examples 双形态**：gherkin-parser 的 Outline Examples 既有 `{examples:[{rows:[…]}]}` 块形态也有 `{examples:[{行字典},…]}` 平铺形态——展开器只认一种会把另一形态展开成 0 执行 → 全部假存活；首轮实录 slots-queue 16/16 假存活、修复后 1121 全量执行 903 killed / 218 survived。**执行层的分类结果必须先对账执行数（执行数>0 且 = 突变体覆盖的执行行数）再采信 verdict**。
- **词汇面幸存体的诚实口径**：218 survived = 突变后的示例值仍通过当前校验器集（数值 ±δ / 布尔翻转 / 自由文本 dither 在规格语义内合法）——不是等价论证而是执行层覆盖面声明；压缩幸存体的途径 = 按场景域逐步骤实现产品面 handler（回补触发登记于 handoff）。
- **产品冒烟三坑**：CLI `-x` 算法前缀要求可识别名（sha256/md5/adler32 小写可识、显示名 SHA-256 不可作前缀）；秒传小文件的 toast 断言失效（<1s 完成，等「已完成 (n)」终态）；详情算法显示需选中任务（] 切页签 + down 选中后再断言）。
- **CRAP 直采管线（无 cargo-llvm-cov 环境）**：rustup component add llvm-tools-preview（TUNA 组件包可能静默解包失败——组件登记 installed 但 llvm-profdata 缺失，强制 uninstall+add 无效，**重装整个工具链**才落盘）→ `RUSTFLAGS="-C instrument-coverage" cargo test --bin ezr --no-run` → `LLVM_PROFILE_FILE=<dir>/%p-%m.profraw` 跑 bin 全量 → llvm-profdata merge -sparse → llvm-cov export <bin> --instr-profile --format=lcov → lcov 过滤 SF: 前缀到项目 src/（不过滤会把 registry crate 计入，ACT 虚高 2044 vs 101）→ crap_calc.py <lcov> <src_root>。实测 41151/43932 行、ACT=101（基线 103，方向向下非回归）。

## 产品面 handler 回补轮（2026-11-17，hardender）追加
- **执行层双层架构落位**：词汇面（按场景列域封闭性校验，schema 落 column_schema.json：非整数值=规格枚举封闭域、整数列界开放 ±15）+ 产品面（59 个 bespoke PTY 场景 handler 落 .work/tmp/aps/handlers/，复用 qa/runners/harness.py）。执行数对账锚：差分定位（adapter 深比较 base/变异 IR 找唯一变更格 → APS_AFFECTED 只跑受影响行）+ 基线运行全场景全执行必须全绿（基线不绿不开工）。变异 job 收窄为受影响行是 adapter 登记的协议偏差，全量覆盖由基线运行承担。
- **变异杀灭锚的六类设计**（产品面把示例值变成可观察载荷）：①跨格锚（校验中占槽 others+1 vs slots 容量、down_parsed vs down 回显、conns_expected vs 粘贴结果）；②步骤文本字面锚（「第 6、7 个任务显示排队第 1 位/第 2 位」——容量突变后第 6/7 个获槽即整行消失）；③config 越界拒启动锚（slots=-4 → ezr 拒启动，无画面即杀）；④输入过滤锚（并发/限速字段负值剥负号 → 回显与突变值失配）；⑤产品固定产物锚（粘贴过滤 12/96、退避表 8/16/32/60、失败分类表）；⑥fixture 块数截断锚（conns=13 > 4 块文件 → 活跃连接截断为 4 ≠ 13）。
- **时序归约替代窗口竞速**：「校验中」显示窗实测 ≈109ms（1 tick），wait_for 轮询竞速不可靠——改断言事件顺序（「SHA-256 校验通过」必须先于排队行消失，30ms 粒度监视器）；同理 Retry-After/退避倒计时只断言显示值镜像。
- **QA 蓝图口径是产品面 handler 的权威来源**：本域 handler 逐场景对齐 qa/runners/suite_*.py 已验证用例（QA-SQ-02/03/04/05/07、QA-MT-01~03、QA-UC-08~10、QA-RS-02/07/08）。QA 已按规程 S 口径删除的用例（RB-09 磁盘预检、UC-10 校验中窗口）handler 走 runtime.SkipExecution（基线记 SKIP、变异 job 记 infrastructure_error），不虚报 killed。
- **断言 needle 三陷阱**：①页签标签子串陷阱——「正在下载」不含「下载中」（差一个中字），「已完成 (0)」恒在场——完成判定必须用「下载完成」toast 或「[已完成]」行徽标；②pyte 头截断——超长字段显示「…尾部」，断言用 endswith；③行值提取截到对话框右边框，防右栏串扰。完成类 toast 会被后续 toast 覆盖（3 秒窗），失败现场转储要在断言内携带。
- **规格-产品形态分歧登记（本轮产品面执行发现，待操作者裁决）**：①路径补全规格期望展开形态 /home/qa/downloads/，产品保持 tilde 形态 ~/downloads/；②URL token 规格期望不响应补全，产品 path_like 含 contains('/') → 无匹配 toast；③失败分类规格「网络错误（子类）」，产品直接形态（连接失败/连接超时/传输中断/无法创建目标文件）；④下载限速字符串写 config 必须带引号（裸 2 MiB/s 是 TOML 解析错误拒启动）。
- **等价类突变体的诚实处置**：17 格镜像等价（retry_after/status/slots/concurrency/secs/conns/port/extra_lines——产品对这些输入值泛化，任何断言均从突变值派生，无锚可杀）。非等价存活率=0（hardender 零目标达成）；等价论证逐格登记于 handoff，回补触发=产品引入对这些输入的校验边界或规格新增区分性断言。压缩它们需 mutator 扩展项目等价突变过滤器（上游字节冻结，属操作者裁决面）。

## 分歧裁决落地修订轮（2026-11-17，hardender 续轮）追加
- **显示通道改写与磁盘真值判别**：工具回读文本里 `[http]`→`ttp]`、`#[must_use]`→`#ust_use]` 等方括号序列残缺是**显示通道改写**，磁盘字节往往完好——判真值用字节计数（`open(f,'rb').read().count(b'[http]')`），别凭显示断定文件损坏，更别"顺手修复"把好文件改坏。
- **描述形态格的断言子串陷阱（全角括号）**：feature 描述性期望「（无 toast——…）」括号是全角，handler 用半角 `startswith("(` 判定恒假；描述形态判定用不含括号的子串（`"无 toast" in toast`）。
- **双层统一：产品面格同样先过规格封闭校验**：「无 toast」描述格被 dither（变异落在判定子串之外的"件→x"）时，子串判定继续为真 → survived 假阴性——产品面 handler 分支必须**先跑 schema closed 校验（域外先拒）再实跑**；handler=基线产品行为验证与第二道防线，规格枚举封闭是第一道。
- **APS 运行时重建口径**：schema 键 = `<stem>|<sidx>|<rowidx>|<col>` 格级；closed=值集封闭（含产品面格）、open_int=lo0/hi base+15（extra_lines 0 格 lo=0 使负 δ 被杀、0→2 界内幸存=等价类）；mutator δ 确定性复现上轮逐格值（10→14/60→68/90→99/120→112/403→399/404→401）。
- **soft differential 跳过语义**：修订后未变场景（scenario_hash 同）复用 manifest killed——对账先分「执行 N + 复用 M」再合总账；硬证据用 `--level full` 全执行（忽略 stamp/manifest，两 feature 77 突变体零跳过）。
- **CRAP 采集三坑（本轮实测）**：①llvm-cov export 后**按行 grep -v SF 会破坏 lcov 段结构**（孤儿 DA 行归属错文件，覆盖值全错位）——过滤必须按 `SF..end_of_record` 段级（`awk '/^SF:/{keep=index($0,"/project/ezr/src/")>0} keep'`）；②crap_calc 依赖预过滤的产品 lcov，全量 export 直接喂会混入 registry 依赖（32.5% 假覆盖）；③**tokio spawn 的 webrtc 任务覆盖竞速**：ws_prepare_offers 同一二进制两遍 9/105→3/79——webrtc 受限函数的 CRAP/ACT 名义值非确定，非回归对账用「行覆盖恒等（94.6%）+ 稳定函数集恒等（mouse:178=77.09 等）」，ACT 名义差异逐条归因（与遗留受限项 webrtc 同源）。
- **CRAP 段级过滤的文件名匹配**：SF 路径含子目录（`src/bt/engine/trackpool.rs`），匹配串别写 `/src/trackpool.rs`（永假）；按目录前缀或 endswith 判。

## QA 五规程可执行化轮（2026-11-17，six-pack/QA）追加
- **graceful_quit 退出礼仪**：模态文本焦点下 q 是普通字符会插入缓冲（快照假尾迹三例归因）——先 Esc 关模态再 Ctrl+C 直退（cmd 窗口 CONTROL+Char('c') → quit 同口径），q 仅兜底。
- **harness 按键名速查**：NAMED_KEYS 无 "backspace"（误用 = 字面文本注入字段导致校验拒绝假红）；退格键名 = "bs"。
- **鼠标点击成对事件**：部分交互（页脚 chip、T 弹窗 [✗]、并发行选中）只认 release——mclick 后必须跟 mup 同坐标。
- **VS15 吞字双口径**：pyte 对 break_links 注入的 U+FE0E 截断绘制 chunk——T 弹窗/对话框 http URL 断言只能走 raw 尾流（re.sub 剥 ANSI + rfind 标题区段化 + strip \uFE0E）；raw_text() 自身已剥离 VS15，VS15 存在性断言必须直读 _raw_tail 字节。
- **折行续行拼接**：列表行折行续行带前导空格且跨行夹其他列内容——整行 join 后子串必断，须逐行 strip 后拼接或两段式（前缀 + 续行）断言。
- **toast 3s 生存期污染**：跨用例段的 toast 断言前必须 wait_gone（stale toast 假阳性实录：补全目录/无匹配两例）。
- **LtPeer upload_rate_limit 不限本端下载**：seeder 侧限速对 driver 下载无效（秒完），下载速率门控用 driver config task_max_download_speed（裸 B/s）。
- **注册表落盘点**：`.ezr/state/registry.json`（非 .ezr/registry.json）；BT 条目 bt.trackers 为创建时入库值（默认池引擎侧另算），任务级限速/归一路径断言以注册表为准。
- **做种行翻页**：做种行在「已完成」页签——断言前必须 ] 循环（沿用 suite_modify_bt._cycle_to_visible）。
- **判重谓词与 infohash**：BT 判重按来源路径归一（v1.42），同 infohash 不同路径的种子可并存；重启用同 config 时注册表恢复旧任务，再添同源即判重——重启臂断言前先清判重面（改 config 或异源种子）。

## mutator 等价过滤器扩展轮（2026-11-18，hardender）追加
- **mutator 项目等价突变过滤器落地口径**：`--equivalence-filter <file>`（version 1；entry = feature stem + 突变路径 `$.scenarios[i].examples[j].<col>` + original + mutated 四元全匹配，附 class/justification/invalidation）——过滤发生在执行前：不执行、不进 total/结果列表（mutator-spec §Equivalent Mutation Filters 口径），单独 `filtered_equivalents=<n>` 汇总 + 报告尾 `filtered equivalents (not executed)` 区 + JSON `filtered` 数组与 `filter-hash`；exit 码只看幸存/错误，等价格不触发 exit 1。17 格登记于 `project/qa/equivalence-filter.json`（权威逐格登记，承接 r3 handoff 等价类清单）。
- **manifest/stamp 过滤器感知语义**：manifest 顶层 `equivalence_filter_hash` + 场景 result.FilteredEquivalents；场景 mutation_count = 执行数（全量 − 过滤）；复用条件 = mutation_count 等于当前可执行数 且（旧条目带过滤哈希时）哈希相等——过滤器变更/移除/路径漂移 → 场景如实重执行（fail-loud）；stamp 哈希掺入过滤哈希（同过滤器可整文件跳过，跨过滤器失效）。feature 编辑致 IR 索引漂移 → 过滤失配 → 突变体回现为幸存，属诚实回退，须重建过滤器条目（discover 派生路径重对账）。
- **机制验证四对照口径**：bb test（上游 8 测不动 + 新增 3 测 = 51 assertions 全绿）+ 合成探针（基线无过滤器行为不变 / 阳性命中过滤 / 阴性 original 错配零过滤 / killed+全过滤 exit 0）+ 同过滤器复用（skipped_scenarios=1）+ 真实数据 6 feature 副本 17/17 精确命中零过度匹配。**副本纪律**：stub/机制验证运行绝不指向真实 features（metadata 自管理写回会污染 manifest），一律 `.work/tmp/` 副本；verdict 运行须接 APS 执行层 runtime（本轮未重建，沿 QA 轮 1090P 基线登记）。

## mktorrent bencode 合规修订轮（2026-11-18，hardender）追加
- **错误类型重构的行为等价论证法**（「零逻辑变化」改动的验证口径）：decode 族 `BtError::Bencode(msg)` → `BencodeError(msg)` + From 升格——消息串逐条保留（Display 输出字节级不变）、消费点三类零改动（`?` 经 From 自动转换 / `.ok()?` 类型擦除 / `.is_ok()` 类型无关）、测试断言面零触碰（is_err/unwrap 与错误具体形态无关）；产品基线 1090P/0F 精确复现 = 等价实证。
- **CRAP 直采管线全链复现**（crap_calc.py 随工作区流转于 qa/runners/，会话级丢失假设不成立）：插桩 `--no-run` 2m27s → 直跑测试二进制采 profraw → merge/export/awk 段级过滤 → 两参调用出账。本轮实测 **ACT=101 与基线精确相等**（零新增可修点）、稳定函数锚 mouse.rs:178=77.09 恒等、行覆盖 94.6%（DA 总量 42121 vs 历轮 43932——nightly 1.101→1.102 插桩行数漂移 + webrtc 覆盖非确定，按「方向非回归 + ACT 相等 + 锚恒等」三证对账）。

## PMD 销项轻量轮（2026-11-18，hardender）追加
- **cpd 销项的双扫描对账法**：受控删除克隆后，复扫不能只看总数下降——用 `git archive HEAD` 提取删除前源码态做同参数对照扫描，前态须与历轮基线组数恒等（本轮 220 恒等，同工具 7.29.0 同 min-tokens 70），再对前后报告做路径规范化（sed 归一 `project/ezr/` 前缀）后 diff：差集应恰为被删克隆组且其余组字节级恒等。本轮差集 = 唯一组 35 行/232 tokens（ezr-mktorrent.rs ↔ bencode.rs encode/encode_into），220→219 的 −1 全额归因，无新增组——比单点 rg 唯一定义点实证强一档（rg 证"定义唯一"，双扫描证"克隆账目零残余且零漂移"）。mktorrent bencode 受限项凭此销项结清。
- **unviable 判定的人工补证法（全臂发散型编译错）**：工具判 unviable 的「删 match 非兜底臂」突变体可 scratch 副本 sed 删臂 + `cargo check` 人工复现裁决——spawn_outgoing_peer 删 `Ok(Ok(s)) => s` 臂实测 **E0599**（match 仅剩 `_` 兜底臂、全臂发散 → 表达式类型推断为 `!`，报错不落在 match 处而落在后续首个对绑定值的方法调用 `sock.set_nodelay`，"method not found in `!`"）。两条教训：①全臂发散型编译错不报 E0004 非穷尽而报 E0599 于远离变异位点的使用点——看到远处报错不要怀疑删错了臂，先看类型流；②unviable 账目可按此法从「工具判定」升格「人工实证」，报错码入账后即为终态，无需再跑测试段。

## QA runtime 重建轮（2026-11-19，six-pack/QA）追加
- **双层统一在调度器落地才是闭环**：「handler 分支先过 schema 域外先拒」若只在通用分支实现、handler 分支绕过，closed 格 dither 会直落 handler 且「判定子串仍在」假阳性幸存（本轮实录 s2.file verify1.bin→Verify1.bin 幸存）——修复 = run_feature 在 handler 分派前对受影响格强制 check_cell；verdict 随即 1107/17 精确闭合。教训：**防线落点必须在「所有执行路径的公共上游」，而不是某一分支内部**。
- **schema 格级 closed 的缺省语义**：场景数据格（夹具名/枚举/期望显示/动作计数/非法输入向量/fixture 参数）缺省 closed = 该格原值单元素集——「该格规格值唯一确定」，任何突变即域外；open_int 三分登记：FILTERED（等价过滤器 17 格镜像语义）/ ANCHORED（handler 锚杀灭，界内突变放行产品面）/ LO_ONLY（纯输入域登记，本轮突变全负 δ 由 lo=0 杀灭，缺 handler 时界内突变如实幸存）。**宽松方向赋 open_int 前必须先有锚，否则幸存体回现**。
- **产品面锚六类在本轮的实例化**：①徽标收敛锚（fixture 块数截断：badge_wait(x,y) 等活跃==并发==格值，文件块数 < 突变值即永不收敛）；②步骤字面锚（「第 6、7 个任务显示排队第 1 位」——total/slots 突变后排队行不存在）；③固定夹具锚（9 文件种子 + 统计行「已选 x 个文件」== 格值）；④产品常量锚（并发回退默认 4、重试 1/5 分母 = max_retries 格值）；⑤速度观察锚（修改限速前后 plateau 采样对照 min(limit, 聚合)）；⑥期望报错锚（CLI 非法值报错退出，突变变合法时 TUI 驻留 → 超时 killed）。
- **非 PTY 直跑 CLI 全体假报错**：subprocess 无伪终端跑 ezr → TUI 初始化 `Os { code: 6 }`（ENXIO）exit 1 → 「期望报错」锚对任意值恒真 → 幸存体假阴性。CLI 臂探针必须 PTY 起动（EzrApp 同款 pty.fork），判「窗内退出码非零 vs TUI 驻留」。
- **fixture ?speed 是每连接门控**：verify1.bin 8MB@500KB/s 实测 ~2.3MB/s（默认并发 4 连接聚合 ×4）——「下载 16s 窗」算成 4s 导致排队任务来不及落位。聚合速率 = rate × min(并发, 块数)，给时序窗算账必须按聚合口径。
- **校验中窗口（~1 tick）的时序归约**：排队任务先落位（大文件+门控下载留窗）→ wait_for(校验中) 抓帧 → 同帧断言满载「下载槽位 n/n」即「占槽不释放+未递补」的可观察面（递补会破坏满载），不做跨帧竞速。
- **限速速度观察用 plateau 采样**：EMA 爬坡期首样本假低（实测 220KB/s vs 期望 1MiB/s）——采样窗取尾段均值（后 40–50% 样本），前置 pump 2.5s。
- **BT 树交互形态（本轮回补）**：ezr-mktorrent 恒包 root/ 目录壳、payload 共同父目录嵌套（root/root/ 双层）→ 对话框树默认收拢只剩「[x] ▸ root/」；展开序列 = tab×2 聚焦文件列表 → right（展开 L1）→ down → right（展开 L2）→ down 落首个文件行；**目录行原始名以 / 结尾**（判目录要在 rstrip("/") 之前判）；修改对话框同一统计行「已选 x 个文件」可直接当格值锚（无需展开数文件行）。
- **按钮聚焦段文本匹配**：ACCENT（青底 00cdcd）段文本 == 目标按钮才算聚焦——整行含三个按钮文本（立即下载/仅添加/取消），整行匹配必假阳（本轮实录：误按立即下载，任务变下载中）。
- **SkipExecution 口径在本轮的落点**：RB-09（磁盘预检）与 04-ui-conns 校验中行的格全为 closed → 突变在词汇面先拒、无 mutation job 到达 handler，SkipExecution 仅保留为 handler 侧语义（本轮未触发，errors=0 与 r3 账目相容）；若未来这些格转 open_int，须在 handler 内恢复 SkipExecution（基线 SKIP / 变异 infrastructure_error）。
- **verdict 级对账锚**：Δ复刻（Python 复刻 mutator fnv64a/mutate-value/discover）应先与 mutator 全量对账（1107+17=1124 逐数恒等）再用于 schema 设计；verdict 后 stamp/manifest 核验用 hash-json 同算法独立复算（sha256(cheshire compact JSON)）——本轮 07-add-bt 9/9 场景哈希一致即「哈希滞后」销项的独立实证。

## 遗留受限项销项轮（2026-11-21，six-pack/QA）追加
- **schema open_int 三分收敛为二分（口径演进定案）**：历轮 ANCHORED/LO_ONLY 的"界内放行产品面"形态在 verdict 实跑数据下收敛——第一轮无 handler 全量跑（954K+153S+17F=1124 恒等）收集全部界内幸存格 153 个，逐格裁决（场景名+占位符步骤上下文全量导出）全部符合 closed 缺省语义（期望显示/夹具参数/规格钦定输入组合/非法向量/装置参数——「该格规格值唯一确定」），closed 化后 verdict 精确复现历轮账目（1107+17+0+0）且 LO_ONLY 18 格「未来界内突变如实幸存」敞口永久消灭。判据：产品输入格若无非派生区分锚（回显/≤ 派生/计数同源/否定断言恒真），其界内突变在场景规格面本就等价——历轮六类锚产品面的价值面由 qa/runners suite_*.py 独立承载，schema 层 closed 先拒等效且更严。**宽判据先跑再裁决**：整数格全赋 open_int 的机械分类只作候选池，真分类必须实跑收集幸存后逐格语义裁决。
- **mutator δ 确定性与 LO_ONLY 敞口的时点语义**：mutator 对格值的突变方向/步长由 fnv64a(path,value) 确定定向——LO_ONLY 格（负 δ 界外杀灭）的"未来 +δ 幸存"敞口只在格值/路径变化时暴露，同一 feature 内容下逐轮复现同一突变集；因此 LO_ONLY 的敞口消灭不必等未来轮，当前轮 closed 化即可永久封口。整数格 δ=±(1..9)、限速字符串格 dither=单字符大小写翻转或 'x' 替换（均非法）——限速格当前突变恒被词汇面拒。
- **APS 执行层四件套重建的接口契约清单（随 .work 清空需按此重建）**：runtime.py（IR 展开 + APS_AFFECTED 格键 `<stem>|<sidx>|<rowidx>|<col>` 单行执行 + closed 域外先拒 + open_int 界校验；exit 0/1/2 = survived/killed/infra）；generator.py（`<ir> <dir>` 两参数 + entry.py 嵌 IR + metadata/<slug>.json——**slug = 全路径逐字符字母数字保留其余转连字符（对齐 mutator feature-metadata-slug，非 basename）**，feature_path 必须与 --feature 一致否则 implementation_hash 落 unknown 复用漂移）；adapter.py（NDJSON job/resp 协议 + base 推导 `<mutations 同级>/../../base/feature.json`（mutations/mN/feature.json 上溯三级）+ 差分定位单格（多格=协议偏差 infra）+ jsonpath 无 $ 前缀）；drive.py（bb gherkin-parser → generator → APS_BASELINE=1 基线先行 → mutator --level full --equivalence-filter --json → reports/<stem>.json + verdict-summary.json；**mutator JSON summary 键为大驼峰 Total/Killed/Survived/Errors/FilteredEquivalents**）；bb 子命令必须 APS 目录内调用（--feature 用绝对路径）。
- **bb/Go 双版 APS 互证口径**：bb 与 Go 的 Discover/MutateValue 逐值恒等（16/16 条对照 diffs=0，fnv64a + delta=±(1..9) 同构）——filter 条目（bb 版实跑登记）两版通用；Go 版 --equivalence-filter 移植后真实数据 20 feature 副本 17/17 精确命中零过度匹配。**四元匹配的 feature 字段 = feature 文件 stem（basename 去 .feature）**——副本纪律探针副本必须保持同名（probe.feature 副本 stem=probe ≠ 登记 stem 零匹配是正确行为非缺陷）。mutator-spec 的 adapter 协议：job{id,feature_json,generated_dir,work_dir,timeout} → resp{id,outcome,output,error,duration(ns)}；outcome→分类映射 test_failure→killed/test_success→survived/infrastructure_error→error。
- **CLI 探针的 runner-worker 不能用 /bin/false**：不读 stdin 不写协议直接退出 → 全部 infrastructure_error（Stream closed）——探针 runner 必须是协议 worker（python3 adapter.py 或最小 NDJSON echo）；"/bin/false" 只适合「全过滤后零执行」的 exit 码探针（D 对照）。
- **工具调用双向文本改写对 `[m` 形态序列的三通道幻象**（bug16 轮登记的复发与扩展）：rg 回读、python -c 源码参数、python repr 输出三处同步显示 `currentFilteredutation`（`[m` 吞失）——磁盘真值用 chr(91) 探针（`src.count('x'+chr(91)+'y')` 纯数字输出）判别：good=3/bad=0 完好 + go build exit=0 双证。判别纪律升级：凡rg/python 回读见 `[X` 形态残缺，先用 chr(91) 拼串探针 + 编译探针双证磁盘，再决定是否重写文件——盲目"修复"会把好文件改坏。
- **trackpool.rs:644 `delete !` 等价体杀灭实证（历轮失效条件兑现）**：杀灭测试 gap644_ws_answer_gate_webrtc_off_answers_arm_inert 落 mod.rs cfg(test)（trackpool.rs 零改动 → --list 不漂移）——构造 = gap_announce_ctx（webrtc:false）+ gap_ws_announce_server 本地 accept_async 会话 + pending_offers pre-insert 匹配 offer_id 的 PendingOffer（**new_peer_connection 沙箱可用性本轮实证**：RTCPeerConnection 本地创建成功，历轮"沙箱未验证"解除）+ 假 sdp answer → 突变体下 accept_answer 失败臂 mark_peer_fail → candidates 含 peer → 断言 killed；原始下守卫 return 断言过。cargo-mutants 单突变体实测 caught（-F 过滤 + face 过滤 gap644_ + --baseline skip）。杀灭证据闭环账目改判：140 覆盖函数突变体 = **138 killed + 0 等价 + 2 unviable 终态**。**ws_handle_offer 自带第二道 webrtc 守卫**（`if !ctx.transports.webrtc return` 于函数头部）——delete ! 突变的观察面只在 ws_process_response 的 answers 臂（ws_handle_answer 无第二守卫），offers_to_answer 臂双层守卫无观察差异。
- **cargo-mutants -F 必须配 --re**：`-F 'pattern'` 默认非正则语义（实跑 18 突变体未过滤），`-F --re` 才按全名正则过滤——历轮"一测一靶"口径的 `-F --re` 两旗标缺一不可。
- **缺口账目 ID 权威清单落档（mutation-gap-registry.json）**：339 entry（pieceio 209/peerwire 78/trackpool 52 实测对账恒等）+ 644 改判 + 2 unviable 终态注记——历轮"未覆盖函数 199 沿登记"从数字升级为逐 ID 权威清单（qa/ 目录随工作区流转，会话级丢失假设不成立）。cargo-mutants --list 三文件 ~1 分钟纯源解析（CARGO_TARGET_DIR 隔离用后即删）。

## 竞态销项轮（2026-11-22，six-pack/coder）追加
- **worker 级 WorkerDeps 夹具的 stop watch sender 必须保活到断言之后**：`watch::channel(false).1` 只取接收端、sender 当场 drop → block_worker 的 `stop_rx.changed()` 返回 Err → select! 臂 `changed.is_err()` 判 aborted → worker 无声中止下载（failure 恒空、块未完成）——表象是「open 已恢复却完不成块」的断言错位。生产路径 stop_tx 由 download.rs 持有活到任务收尾，夹具必须同构：worker_deps_for 返回 (deps, stop_tx)，测试用 `let (deps, _stop_tx)` 绑定保活（`let _ =` 会立即 drop，下划线前缀绑定才保活到作用域尾）。
- **TDD 红灯归因要 panic 文本逐行比对，夹具修正与行为修复两步分账**：本轮红灯→绿灯经历两步——真正的行为红灯文本是「交错重建窗内 ENOENT 不得误 Fatal」（open 路径单发 fatal）；夹具 sender 修正后红灯文本变为「应完成唯一块」（断言链后移、open 修复已生效）。只看失败测试名或截断的 tail 输出会把两步混为一谈。恢复测与守卫测必须配对：恢复测（窗内文件出现 → 恢复并完成下载、字节数与内容模式断言）防「误 Fatal」，守卫测（恒缺位 → 窗耗尽仍 fatal 原文案）防「有界重试吞掉永久错误」——缺一即测试网有洞。

## 竞态销项后续清理轮（2026-11-22，six-pack/cleaner）追加
- **#[path] 双载体的测试计数传导对账公式**：集成目标经 `#[path = "../src/..."]` 再纳入 src 模块时，bin 测试模块（cfg(test)）的增删会按「含该模块的集成目标数」放大进集成计数——本轮 bins +3 → 集成 +27（engine_backfill/ui_backfill/g_appcore/g_engine/g_supervisor/g_throttle_err/g_v147/mutation/v115 九个目标各 +3；hardening/g_main/g_proxy/g_slots 四目标不含 supervisor 模块零变化）。对账公式 = 集成差值 ÷ bin 差值 = 携带目标数（9），对不上即另有改动面未清点。编译域白名单核验用 `rg 'path = "\.\./src' tests/<目标>.rs`（沿用 mtime 时序校验法条目的白名单口径）。
- **worker 级测试的目标文件必须先预建**（`OpenOptions::write+create` + `set_len` 先于 spawn）：漏建文件时 open_target_file 的 ENOENT 有界重试窗会把「漏建」演成 1s 窗耗尽 fatal（failure=无法写入目标文件），与被测行为（requeue 跳过已完成块）完全无关——表象是断言"不得误报失败"反而收到 fatal。归因探针 = 临时 eprintln failure Debug（产物删除），读 kind/reason 即定位；产物探针必须随修复删除，不留调试残渣。

## 变异加固轮 + schema 口径结案（2026-11-22，six-pack/hardender）追加
- **schema 口径演进二分口径结案（操作者裁定不否决，否决路径关闭）**：open_int 三分（FILTERED/ANCHORED/LO_ONLY）退役为二分（FILTERED + closed）的 QA 交付态经操作者 20261122 裁定不否决，正式定案——FILTERED 17（equivalence-filter.json：retry-backoff 6/modify-task 4/ui-conns 3/slots-queue 2/named-proxy 1/cmd-window 1）+ closed 1107，账目守恒 1124=1107+17 与 153 格逐格裁决一并落定。守恒在本轮以修复后的 mutator 软差分复用全程复现：193 场景全复用、executed=0、Σ=1124 恒等；20 个 feature 的 stamp/manifest 经本轮软跑刷新（fh 键仅 6 个 filter 命中 feature 持有，与 QA 轮口径一致）。
- **APS mutator patch 的复用路径回归（本轮发现并修复）**：等价过滤器扩展 patch 把 manifest 复用判据的「本场景可执行数」写成了 remove 语义（count(非本场景且未滤选)）——与 stamped mutation_count（本场景滤选后计数）永失配，soft/hard 级差分复用整体失效（--level full 不受影响，QA 轮全量跑未暴露）。修复 = filter+not contains 的本场景滤选后计数；修复后单 feature 探针（SkippedScenarios=全部）验证再放量；patch 交付件已重生成（qa/aps-mutator-equivalence-extension.patch）。判据校准法：bb 内直调 accepted-skips/manifest-entry-reusable? 逐条件打印（#' 私有函数语法），比读码猜快一个量级。
- **本轮 cargo-mutants 账目（batch = architect 轮新增产品逻辑 model/bt.rs + model/persist.rs，82 变异点）**：killed 75 + unviable 6（Default 替换型：BtPartialBlock/BtSidecar/TrackerUrl/PeerTransport 均无 Default 派生，E0277 活体探针实证，逐类型 derive 核验在案）+ 幸存 1（bt.rs:79:19 `lo < hi`→`lo <= hi` 边界空转型无条件等价：lo==hi 时 sum += hi-lo = 0，任何语料无可观察面，非等价存活率=0）。两个覆盖缺口（bt.rs:44:45 `o.path == f.path` 的 ==→!=、persist.rs:4:19 NotFound 守卫→true）由 tests/hardening/bt_vocab_hard.rs 三测杀灭（一测一靶：先前选中才删 + 从未选中必不删 + 非 NotFound 错误必须传播，kill 输入 = remove_file(目录) 的 EISDIR）。测试命令口径：`cargo test --bin ezr --test hardening -- model:: bt::tracker:: bt::webrtc:: bt_vocab_hard::`（属性测试不并入变异被测命令）。
- **等价体的失效条件（回补触发）**：span_sum_over_selected_files 的 lo==hi 分支加 0 等价依赖「sum += 为纯数值累加」——若未来该累加引入副作用（日志/计数）或改用饱和/panic 算术使 lo==hi 可观察，bt.rs:79:19 等价论证失效，须回补边界测试（新增文件使 hi==lo 边界可观察即触发回补）。
- **DRY 几何再登记残留**：CPD 双扫描（worktree vs git archive HEAD，min-tokens 70，同 PMD 7.29.0）逐字节恒等 221 组；vs cleaner 轮基线 219 的 +2 为 architect 纯移动轮的几何再登记残留（无克隆组涉 model/bt.rs/model/persist.rs，克隆面集中在 bt/engine/mod.rs 148 组与 supervisor 22 组的既有账），本轮零漂移。
- **CRAP 无回归判定按历轮口径复验**：行覆盖 39828/42084=94.6% 与 QA 轮登记恒等；mouse.rs:178=77.09 稳定锚恒等；ACTIONABLE_COUNT=101 全部为历轮既定账（受限项 5 处 cov=0 登记在列：mouse:516/main:303/metainfo:52 等；trackpool 竞速对账条目现存行号 413，历轮登记 412 为 FN 行漂移）；本轮零新增 ACTIONABLE（src 零改动 + 覆盖集不变）。

## QA 轮（20261123）：runner 时序加固 / hover 残留 / 弹窗内容锚 / CPD 对账 / llvm-cov 漂移
- **「等待中」状态的先臂遮蔽（BE-11 型 runner 时序）**：B 键断开的 HTTP 拒绝臂断言前，任务若
  处于等待中（探测未完成）会命中更早的「当前状态无可断开的并发连接」臂——多臂 toast 断言
  必须先等前置状态到位（wait_for 下载中/占槽）再断言专用臂，pump 固定等待撑不过负载抖动。
- **✗ hot 渲染的 hover 残留不是按压粘滞**：SGR 点击后 pointer 停位若落在 [✗] 热区，✗ 呈
  红+粗是悬停渲染（btn.rs hot = x_press || hovered，pressed 在 Up 分支无条件清除、无粘滞）——
  套件断言常态配色前先点外部移出 pointer 再抓帧；「按压态跨弹窗实例残留」的误判要先排除
  hover 语义再怀疑状态机。
- **字段值弹窗的套件定位锚**：弹窗标题与详情面板字段名同串冲突（panel_span 首
  命中落在详情行）——用内容锚（完整值的前 32 位等单行内唯一子串）向上找 ╭ 提取几何；
  wrap 断点 = 内容区 42 列（46 内框 − 2×2 阅读内边距），64 位码 42/20 分断；
  弹窗体文本提取必须裁剪列范围（l..r），右侧重叠区的详情面板 tick 刷新会污染全文 diff。
- **CPD A/B 对账的 strip 前缀陷阱**：PMD CPD 输出绝对路径——双扫描逐位置对账时
  strip_prefix 必须与 PMD 实际输出的绝对路径根逐字对齐（相对前缀静默不截取 → 路径残留
  差异 → 恒等误判 False）；对账脚本先用一个已知组验证截取非空再采信结论。
- **llvm-cov 行覆盖随 nightly build 漂移**：分母（LF 插桩行数）随 rustc coverage mapping
  微版本变化（本轮 42084→43940，+4.4%，nightly c7d5ba59d-20261120 vs 基线 bdf712ee8-20261121），
  行覆盖百分比不可跨 nightly build 直接对账；CRAP 回归判定锚 = ACTIONABLE_COUNT 与
  具体函数 CRAP 锚（mouse.rs:178=77.09）恒等，行覆盖仅在口径注记中登记。
- **QA 套件覆盖欠账的检测与补齐口径**：md 表格用例集（`^\| QA-XX-NN \|`）与 suite 用例集
  diff——md 独有 = 可执行化欠账（补实现）；py 独有 = 跨文档引用噪声（甄别后放行）；
  规程钦定等价覆盖（头部注记「≡ 其他 suite 实跑 / 单测锁」）不算欠账。补齐时 suite 头部
  「N 用例」声明同步改写，防止声明数与注册数继续漂移。
- **IC-07 块首竞速假 FAIL 三证复判再证**：完成瞬态「校验中占槽」观察窗单点 FAIL，原样复跑
  全量即绿——块首竞速三证判别法（engineering.md）在沙箱回收重建后的首次全量再证有效。
- **新建 feature 的 dry-check 基线对照口径**：新文件无 `git show HEAD:` 基线可 diff 时，
  以既有同风格 feature 的 findings 类型分布为对照（01-resize 量级：near-duplicate ~49/
  possible-synonym ~59）——near-duplicate/possible-synonym 属中文步骤风格的顾问性发现
  （medium/low 置信），不是必改项；**high 置信的 duplicate-in-scenario（同场景逐字重复步）
  与 placeholder-variant（同动作仅占位符名不同）才是本轮必改项**（措辞区分/占位符统一即可
  清零）。
- **demo PTY toast 验证须逐帧捕获（capture.py 末帧假阴性）**：ezr-demo 模拟器周期性产生
  事件 toast（如「后期处理完成」），"一次只显示最新一条"口径下按键触发的 toast 会被覆盖——
  capture.py 只报最终帧，toast 落在窗口中段即假阴性 MISS。正解 = 逐帧捕获：每次
  stream.feed 后立即断言当前 screen.display（帧级 HIT 任一即过），时长取 键注入时刻 +
  toast 存活窗（3s）内即可；另注意 needle 逐字匹配无正则（传 `Ctrl\+R` 反斜杠即字面量
  必 MISS）。（two-pack/coder Ctrl+R toast 轮实证：末帧法 MISS、逐帧法 66 帧 HIT）

## Ctrl+R 配置热加载实现轮（2026-11-24，coder）追加
- **reload_config_from(Option<&Path>) 参数化注入口避 env 竞态**：App 级 reload 测试全部走显式路径注入，`config_path()` 读 env 的 glue 保持零测试——不重蹈 config.rs 既有 set_var/remove_var 跨测试并行竞态面（EZR_HOME 是进程级全局，并行测试互相改写）。App::new 的测试实例用 testutil::make_app_in（隔离 registry），reload 路径用 uniq_tmp_dir 手写 config.toml。
- **引擎 spy 下 reload 命令序固定可断言**：ApplyGlobals 恒为首条 + reconfigs 按任务序——`recv_all(n)` 收满计数 + `timeout(150ms).expect_err` 断言「不应有第二条命令」（无变化臂/暂停臂的负向单发断言）。timeout abort 的 acquire/receive 取消安全，无状态残留。
- **帮助行数守卫是双断言面**：help_window_geometry 的 End 钳制值（62→63）与 help_scroll_clamps 的总行数守卫（82→83）两处独立断言同源同步——加一条目漏改一处 = 单跑绿全量红（本轮实录第二处即全量跑出）。改帮助内容先 `rg -n "82|62" src/ui/help.rs` 找齐守卫点。
- **注册表落库断言用 pretty JSON 形态**：save_json_atomic 走 to_string_pretty，`"proxy": "direct"` 带空格——字面 contains 断言按实际序列化形态写，不看 serde 默认假设。
- **网关取消调用 ≠ 测试进程死亡**：集成套件串行轮询中工具调用被 context canceled，timeout 包装的 cargo test 进程可能存活继续跑完——恢复时先 pgrep 探针 + 日志尾部核对，进程在则继续轮询采信落盘结果（g_engine 实录：canceled 后进程跑完 882P/0F 落盘）。

## 进度条行尾恒空 bug 修复轮（2026-11-24，coder）追加
- **行尾恒空的守卫测试形态**：两条断言正交缺一不可——①内容断言：五状态（Verifying/PostProcessing/Downloading/Failed/Completed）× 携带 SHA-512 checksum 的 progress_row 输出不含「分块校验/后期处理中/SH」任何残字（直接钉住操作者报告的观测形态）；②预算断言：行 spans 显示宽度恒 = 2 + bar(width−4)（`w()` 累加求和），任何未来「往行尾再加点什么」的改动立即被预算断言击穿；③窄宽度参数化（20/24/31/40）防「只在典型宽度测过」盲区。
- **旧断言反转而非删除**：progress_row_activity_hints（断言提示在场）改判为 progress_row_tail_always_empty（断言提示缺席）——测试名与断言同步反转保留测试点位，git 历史可溯「同一显示面的语义翻转」；全部 progress_row 调用点同步去 spinner 参数（编译器兜底找全）。
- **变异测试随宿主代码退役**：spinner 表的变异测试（hardening_g_appcore）在宿主方法删除后必须同步退役——变异测试的生命周期绑定其宿主实现，宿主亡则测试亡；集成套件计数因此 -1（886→885）属预期位移，非回归。

## 清理轮（2026-11-24，cleaner）追加
- **bin 侧新增测试经 include! 树复制进集成套件，对账按每套件差值登记**：本轮新增 16 个 bin 侧测试（app/keys/engine/list），集成合计 8646→8768（+122）——增量 = 各套件 include 树所含新测之和（含 app/mod.rs+keys.rs+engine/mod.rs 的套件各 +14、仅含 engine 的 +2、含 list 的 ui/engine_backfill/v115/mutation 各 +16），逐套件差值精确自洽；对账不做「全局 +16」单值推断，以每套件实测差值入账。
- **一切构造 App 的测试必须 #[tokio::test]**：App::new 内嵌引擎 main_loop 的 tokio::spawn（engine/mod.rs），同步 #[test] 下构造即 panic「there is no reactor running」——TestBackend 纯渲染断言测试同样中招（draw_list 不触引擎也无济于事），收尾统一 shutdown().await。
- **cfg(test) re-export 的 #[allow(unused_imports)] 是 include! 集成测试服务路径，删前先查引用面**：bin 编译口径看不到 include! 树里的使用（engine::classify_reqwest 被 hardening_g_throttle_err 直接调用），删 allow 即 bin 口径报 unused——但同组其余 3 个 re-export（HostsFile/parse_retry_after_header/bucket_step 组）确属死抑制器，删除后 clippy --all-targets 0 警告裁决。判据：rg tests/ 里 include! 树对该符号的调用，有调用 = allow 必需，无调用 = 删。
- **clippy field_reassign_with_default 在测试代码同样算数**：Config::default() 构造后字段赋值改 struct update 语法（`Config { warnings: vec![…], ..Config::default() }`），与 lint 清零纪律同口径。

## 变异加固轮（2026-11-25，hardender）追加
- **cargo-mutants 真实仓库 = sourcefrog/cargo-mutants**（sourcegraph 路径 404；crates.io 直连被沙箱拦、sparse index 路径探测亦不可靠——GitHub 搜索 API 找仓最稳）。`--in-place` 与 `-j` 互斥复证（单 worker 稳态）。
- **CARGO_TEST_ARGS 是 cargo test 的直接参数**：工具把 `--` 后参数原样追加到 cargo test 命令（无分隔符），cargo test 只收一个 TESTNAME 位置参数——多滤选串必须写成 `-- -- f1 f2`（第一个 `--` 归 mutants、第二个归 cargo，libtest 多滤选 = OR 子串、无匹配滤串为空操作）。libtest 滤选语义实证：`-- --list -- f1 f2` 列出的是 OR 并集。
- **基线闸门的超时假死形态**：`--timeout` 低于滤选面裸耗时（宽面 480 测 ≈62s < timeout 60）→ 工具把基线判 Timeout 直接拒跑（表象与 OOM/编译错同形）。宽面跑变异一律 timeout ≥ 3× 裸耗时（180s）。
- **三角复判协议（窄面快跑 + 补面定真伪）**：发现面按击杀密度裁剪（省 40-50% 墙钟）→ 每个幸存体用「被裁模块 + ui::」补面复判——补面被杀 = 伪幸存（既有测试他面击杀，记 killed）；补面仍 MISSED = 真缺口 → 一测一靶补杀。本轮 97 幸存体 → 56 真缺口 + 12 伪幸存（keys/mod 12 + stats 13 + app/mod 7）+ 12 等价 + 17 直接击杀面外。
- **--iterate 只跳 caught，MISSED 每轮重测**：幸存体 >9 个时每块全预算被重测吃光 → 用 `-E 'file\.rs:(行号列表):'` 排除已判行（行粒度无法分选同行多突变体，同行含等价体时整行重测）。`-F '^path:行号:'` 行号段推进绕开重测区。
- **追加式加固测试的收敛捷径**：测试只追加在文件尾/测试模块尾 → 产品码突变体位置零位移，`cargo mutants --list` 计数恒等（87/46/27/68/59/20 实测恒等）→ 既有 killed/unviable 判定保持有效，仅需定向复验幸存体（无需删会话状态全量重跑）。--list 计数恒等 = 零新增突变体证明。
- **等价体高频模式新增三类**（承接 mutation-hardening.md 模式库）：①**mod-2 环同余**：FILTERS.len()==2 时 cycle_filter 的 ±delta rem_euclid(2) 恒等（失效条件：FILTERS 扩至 >2）；②**双层卫语句嵌套冗余**：外层 `if n > 0` 与被调函数内层 `n==0` 早退嵌套，外层比较符翻转不可观察（失效条件：内层守卫移除/改动）；③**死函数**：#[allow(dead_code)] 全仓零调用点函数的任何突变不可观察（失效条件：出现任何调用点）。死钳位（卫语句先行子类）：早退卫语句保证 min(x, len-1) 恒等 x。
- **小盘磁盘时序（变异+插桩同轮）**：mutants 场景 INC=1 缓存把 scratch target 推到 3.3G——变异收尾先删 scratch 再进插桩段；插桩 cov-target ~1.3G 用后即删；中途爆盘先删可再生大头再跑（cp -a 项目树务必显式排除 target/，整树 cp 会把 1.6G target 拷爆盘）。

**reload 套件轮（2026-11-25，QA）**：12-reload-config 可执行化十四坑沉淀——①**reload/完成 toast 单槽竞态**：BT 任务的完成 toast 会顶掉 reload toast（3s 窗），reload 断言必须锚定在无完成事件的观察窗；BT 用 `task_max_download_speed = 200000` 钉任务生命周期（bt 套件 _driver_config(rate) 同款），HTTP 用 fixture 门控拉长（seeder upload_rate_limit 实测不收敛，勿依赖）。②**长周期与块周转双约束**：reload 后「重拨直连/新端点」观察依赖块翻转会重拨（每块一次请求），1MB 块下限 `MIN_HTTP_BLOCK_SIZE` 钳制使小块方案不可行——组合公式 = 大文件（big-100m）× 中速门控（300K/s×4 连接 ≈83s 生命周期、~3.4s 块周转），20K/s×1MB 块 = 51s/块（重拨窗 30s 恒空）。③**auto_retry 两相位的失败确定性**：disconnect ≥ 残余片长时截断落在响应体末尾，请求完成与对端重置竞速、失败判定非确定（disconnect=1000000 重试轮静默完成）——两相位用块内中截 `disconnect=65536`（< 残余片长恒为网络错误，有进展重置计数不触上限）。④**修改框时序约束**：'m' 对已完成任务不开修改框（实测 20261125）——规程的修改框检查须在任务下载中打开；完成任务从「正在下载」移入「已完成」页签，选任务助手需两页签回退（harness.select_task_by_needle）。⑤**窄终端长 toast 渲染截断**：120 列页脚只呈现失败 toast 前 ~95 字符（含路径后「键 common」即断），完整键点路径判据（逐键文案）须 160 列启动（产品支持 90–190 列，其余用例仍 120×44）。⑥**BT announce 落账 ~2s**：announce 断言用带泵轮询窗（20s），不即时断言；driver peer_id = -EZ2000- 前缀、announce port 字段即 listen_port（t2 新增 started 行 + port 断言 = listen_port 启动绑定一次的持久证据）。

**phase-05 规格轮（2026-11-25，specifier）**：gherkin-ir-dry-checker findings 为**建议性口径**——
未改动 feature 基线 117–215 条（near-duplicate/possible-synonym medium 为主，09-bt-engine 117/
12-reload-config 206/01-tui-display 215 对照实测）；判读 = 基线对照 + 高置信度
（duplicate-in-scenario/high）逐条人工核对（本轮 01-add-task 唯一 high = 场景 05 有意重复
「按 Enter 确认」两臂，既有非本轮引入）；bb v1.13.230 静态包 gh-proxy 直连快（GitHub 直连
120s 超时再证）+ APS clone 直连可用，7 个涉改 feature parser/dry-check 全绿。

## 动态引擎测试（2026-11-25，coder phase-05）追加
- e2e 竞速与暂停触发：全速 mock 下 2 MiB 秒完，「进度>256 KiB 即 Pause」的暂停触发会输给下载完成——暂停类测试用 slow mock（8 KiB/15ms）把窗口拉到秒级；测字节完整性用确定性 fill(pos) 内容（跨服务器实例续传也成立）。
- cargo test 超时误判：大 crate 增量编译 60-90s，杀掉编译后再跑会反复触发重编——长时间编译单独跑一次（build --tests），测试运行与编译分开限时；被杀的编译留下脏状态是「测试挂死」假象的主因之一。

## 集成套件既有失败的基线对照纪律（2026-11-26，coder 蓝块轮）
- **现象**：全量集成实跑发现 10 个失败（hardening_g_engine 2、g_v147 5、v115 2、ui_backfill 1），其中失败断言信息直指"突变体被放行"等历史加固语义，与当轮改动面（dynamic.rs sidecar 快照）无文件交集。
- **处置**：`git stash push` → 同命令复跑失败用例 → `git stash pop`，三条家族代表用例在基线上逐一复现失败 → 判定既有失败，登记 handoff 遗留项并按族归因，不顺手越权修复；对照必须 stash 前后同语境（同命令、同环境）。
- **根因（三族）**：①phase-05 D84 把默认 scheduler 改 dynamic 后，blocks sidecar 也被路由进 download_dynamic，其一致性/孤儿裁决只包在 `if resume`（dynamic sidecar）臂内 → 作废/改道臂失效；②动态引擎 D87 降级 rescue（Range 失效改单流）与旧失败语义预期冲突（403 块请求被 rescue 完成、body EOF 分类 Transient）；③gen 剥头副本在 phase-05 改 src/ui/detail.rs 后未重新生成。教训：默认开关翻转轮（mode-flip）必须全量实跑集成套件而不是子集，"上轮账全绿"只对其实跑过的子集有效。

## HTTP 续传 e2e 桩服务两条纪律：206 必带 Content-Range + 逐连接线程
- **现象**：app 层暂停→继续 e2e 两族假象——①桩 206 不带 Content-Range → 续传探测
  `head.total` 落到 Content-Length（剩余长度）→ consistency 失效作废断点 → 静默从头
  重下，断言全绿但根本没测续传；②桩 accept 循环单线程逐个服务连接 → 多 worker 只有
  一个在传，其余饿死并触发 preempt 抖动（60s 不完成假红）。
- **处置**：①stub 206 响应必带 `Content-Range: bytes {start}-{end}/{total}`（产品
  `probe_head` 的 content_range_total 优先于 Content-Length，缺头时 total=剩余长度）；
  ②桩 accept 循环逐连接 `thread::spawn`；e2e 加「downloaded 不得回退」断言锁死作废
  重下路径（回退即作废，静默污染无所遁形）。
- **根因**：桩服务的协议完备度决定被测路径的真实性——产品按 RFC 把「206 缺
  Content-Range」当无法核验总长处理是正确行为；测试污染的表象是「测试通过」而非报错，
  判别探针 = 引擎 Progress 的 conns 数与 done 增长（饿死 = 多数 conn done 恒 0）。

## coder 轮复现操作者 TUI 报告：harness.EzrApp + Fixture 一次性探针（.work/tmp）
- **处置**：PTY 驱动真实 TUI 的诊断探针脚本放 `.work/tmp/`（会话级，不入 project/）：
  `sys.path.insert` 引 `qa/runners/harness.py`，EzrApp（按键注入 + pyte 画面）+
  Fixture（`?speed=` 门控稳定观察窗）+ screen buffer 逐格计数四色块；引擎侧探针用
  「feature 门控 + 文件落盘」而非 eprintln——PTY 会把 stderr 混进画面污染 pyte；
  交付前删除探针代码与 feature。
- **根因**：操作者报告的 TUI 显示缺陷在单元/app e2e 层可能根本不复现（本轮 app 层
  四态 e2e 全绿、真实 TUI 却 🟦=0 全程）——端到端 PTY 逐帧采样是 TUI 投影类报告的
  唯一可信复现层，也是修复效果的最终验收层。

## 四态 e2e 断言必须以暂停形状为条件（2026-11-26，coder 续传残留轮）
- **现象**：续传 e2e「续传过程应存在 ⬜」断言偶发失败——暂停时 4 个 worker 各自跨过块
  边界，全部块已被触碰（done∪partial=全块），续传期间合法地没有 ⬜；旧代码却长期
  通过，因为旧暂停显示滞后最后一次 Progress（白块残影），断言靠假象通过。
- **处置**：先在暂停采样点记录 `untouched_at_pause`（pieces 为 Empty 的块），续传断言
  改为「暂停时存在未触碰块 ⇒ 续传过程必须出现 ⬜」；并把暂停触发提前到首块绿
  （pre_done ≥1）使白块在续传窗口内稳定存在。
- **根因**：暂停快照的块形状由 worker 写入位置决定（对齐分段下每 worker 跨 2 块 =
  全触碰），显示修复（陈旧→精确）会把旧断言的运气翻转成失败——**四态 e2e 的状态
  存在性断言必须以被测时刻的真实集合形状为前提，不能用上一版渲染缺陷当期望值**。

## 分配顺序的外部证据法：fixture JSONL Range 日志（2026-11-26，coder 续传残留轮）
- **处置**：验证「续传是否优先续下部分块」无需引擎插桩——Fixture `--log` 落 JSONL
  （ts/method/path/range），续传批次的 Range 起点即 worker 分配点；全部命中部分段
  终点（如 bytes=246000- / 1294576-）即证明 planner 从黄块残留字节续下。配套单元
  特征测试：`from_gaps_resume_starts_inside_partial_block_residues`（注意 from_gaps
  入参是缺口=已下载区间的补集，不是已下载区间本身）。
- **根因**：分配顺序属引擎内部时序，UI 断言只能看结果色块；请求日志是无侵入的外部
  真值源，把「分配语义」验证从推理变成逐字节对账。

## 速度回退修复轮（20261126）追加
- burst_server 夹具不解析 Range 头：非 primed worker 的 `bytes=N-` 请求收 200 → range_ignored
  退出，只剩 primed 单连接——并行度类 e2e 必须用 mock_server（206 + Content-Range + slow 模式），
  服务器侧并行实证走 fixture JSONL 日志（Range 请求条数 + 起点 = 并行数 + 对齐双证据）。
- 细化网格 vacated 不变式 e2e 的哨兵选型：跨块租约会被空闲 worker 在首个内部块边界切走
  （限速桶 burst=rate×burst_secs 让首 worker 提前完成是触发器），前沿越界瞬间恰逢租约旋转时
  50ms 进度帧采样抓不到越界帧 → saw_vacated「必须观测到越界」哨兵间歇假失败；换 saw_spanning
  （鸽笼：块数 > 并发数 ⟹ 必存在跨块租约）+ frames ≥ 10 采样充足哨兵，不变式断言本体保持
  逐帧全检。哨兵断言必须区分「性质验证」（可确定性保证）与「事件观测」（依赖时序窗口）。
- 负载敏感 e2e 三连判据沿用：单跑绿 + 全量复跑绿 + 探针帧模式取证（临时 EZR_PROBE eprintln
  注入 → 跑 6 次 → 对比失败样本帧序列）定性根因后撤探针。
- app 侧 Probed 细化接线测试注意：全部既有 app Probed 测试均 dynamic:false——新增 dynamic:true
  用例才覆盖 t.block_size 回写链；blocks 模式不变式需独立反例用例钉住。
- spec 三层同步的 examples 行退役操作细节（20261127 族A 轮）：退役 Examples 表中的行不能写
  「单格占位行」——Gherkin 要求 example row 列数 = header 列数，占位行 parse 即报
  "example row has N cells, header has M"（APS gherkin-parser line 146 实证）；正确形态 =
  整行删除 + 撤销痕写进场景清单注释/头部注记（修订纪律的留空只适用于「场景」整体退役，
  不适用于表行）。
- APS dry-check 是两步流水线：`gherkin-parser <feature> <ir.json>` 产出 IR 后才能
  `gherkin-ir-dry-checker <ir.json> <dry.json>`——直接把 feature 原文喂 dry-checker 报
  "Unexpected character '#'"（输入不是 JSON）；bb 任务必须在 .tools/Acceptance-Pipeline-Specification
  目录内调用（engineering.md 调用约定）。

## v1.64 轮追加
- APS `gherkin-ir-dry-checker` 的 findings（near-duplicate/possible-synonym）是提示级输出
  （exit 0），不阻断；轮内回归判据 = 与 HEAD 基线 findings 计数对比（本轮两 feature
  12→12、92→92 持平 = 零新增）。基线对比法：`git show HEAD:<feature>` 落 .work/tmp 后
  同管道跑 parser + dry-checker 对计数。

## D94 实现轮追加
- update_conn_stats 签名变更的剥头副本三触点联动：tests/gen/update_conn_stats_stripped.rs
  （逐字节重生成 = type 别名两行 + fn 全文）+ ucs_fix.call 形参 + pure_targets 构造点/新
  map 断言三处同步；凡改 product fn 签名，收尾前先 grep tests/gen/ 剥头副本面。gen_ucs_
  fixture 对比锚 = `fn update_conn_stats(` 起至列 0 `}` 止，剥头副本含别名两行。
- clippy --all-targets 是测试目标编译断裂的第一道网：`cargo test --bin ezr` 不编译
  tests/ 目标，构造点新增字段这类签名断裂只有 --all-targets（或显式 --test <名>）先红——
  全量验证序列里 clippy 必须先于 test 跑，红灯定位比 test 全量后置发现便宜得多。
