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
