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
