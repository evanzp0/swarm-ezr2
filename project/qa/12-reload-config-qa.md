# QA 套件 · 12-reload-config 配置热加载（期号 01，v1.54 新建；v1.55/v1.56 修订）

> 对应规格：`project/features/12-reload-config.feature`（14 场景）。
> 需求依据：`project/mission/phase-01.md` FR-01-119、D81（操作者 ezr-reload-cfg.md 改动方案
> 钦定 + 20261123 chat 指令；Inversion 阶段 1 受限退化沿 v1.45–v1.57 轮先例）。
> 行为口径：按 Ctrl+R 重新读取 config 文件（EZR_HOME/config.toml，D16）热加载无需重启——
> 全局可达（主界面/全部浮层，与 Ctrl+C 同级分派）、原子性（任一键非法整体拒绝、旧配置
> 原样保留）、成功单条汇总 toast（✓ 配置已热加载 + 代理切换/需重启生效可选段，D81②；
> v1.56 起「代理删除降直连」段退役——引用消失不单独提示，D82②）、CLI --max-speed 覆盖贯穿热加载（FR-01-119⑥）、配置文件删除 = 默认
> 配置整体热加载（D81①）、无主目录环境 toast「无配置文件」。
> 四类键生效语义判据来源：ezr-reload-cfg.md 第三节逐键影响分析（A 立即全局 / B 快照 /
> C 需重启 / D 失败保护），实现消费点已经 specifier 代码实证（slots::allocate 每 tick、
> Throttle::set_rate、resolve_proxy missing 臂、try_load 缺失臂、EngineShared burst/sleep
> 平值字段）。
> 端到端 UI 层验证：按键注入 + 画面断言 + config.toml 运行期外部改写 + fixture 日志对账
> （代理来源端点 / tracker announce 来源端口 / 失败重试形态）。运行器可执行化（suite
> runner）归 six-pack/QA 轮。
> ezr 不做旧版本兼容（操作者钦定沿 v1.45/FR-02-35）；本轮不改注册表与 sidecar 格式。
> v1.55 修订（操作者 bug 修复轮 20261124「命名代理改名或删除 + 热加载后添加和修改任务仍旧
> 出现原有命名代理；应为引用任务自动变直连、添加/修改代理下拉删除该命名代理」）：
> QA-RC-06 扩面（修改对话框下拉不注入旧名、落库转直连重启不复活、重加同名不复挂）+
> 新增 QA-RC-14（改名臂：旧名消失任务转直连、新名入池不迁移引用、浮层内同步重建、重启复查）；
> M 口径 = 引用已消失名字的任务总数（全状态计，D82②）；判据依据 phase-01 v1.55 注记与 D82。
> v1.56 修订（操作者 D82 闸门裁定 20261124「② M 口径：热加载改代理不需要单独的消息提示，
> 统一用 reload config 的 toast 就行了；① 切换落库 和 ③ 改名臂——保持」；v1.55 轮合成确认
> 闸门 = B）：D82② 改判结案（M 段退役）、D82①③ 追认保持——QA-RC-06/14 toast 判据改判
> （引用消失不追加专属提示段，成功 toast 仅 ✓ 配置已热加载）+ 通过准则 M 口径退役同步。
> 判据依据 phase-01 v1.56 注记与 D82（②提示口径结案版）。

## 环境前置

- 伪终端驱动：按键注入、画面捕获、toast/浮层断言；每用例独立 EZR_HOME（隔离 config 与
  state）；终端 120×44。
- HTTP fixture 服务器：慢速文件（限速可观察）、可重复触发的约 30% 断连模拟（同
  01-retry-backoff-qa 口径）、多文件族（同 01-slots-queue-qa）。
- 代理 fixture：双独立监听端点（p1/p2 可分别指向），记录每次连接来源端点（直连/经代理）
  ——同 02-named-proxy-qa/01-throttle-proxy-qa 既有 harness 口径。
- BT fixture 四件套（同 09-bt-engine-qa）：tracker 记录每次 announce 来源端口；seeder 供
  BT 任务真实传输。
- config.toml 运行期改写：用例内由外部进程按点路径（[common]/[http]/[bt].键名）改写文件，
  与运行中 ezr 无 IPC——热加载唯一入口 = Ctrl+R 键注入。
- toast 断言窗口：toast 3 秒自动消失（D36），注入 Ctrl+R 后须在 1 个采样帧内捕获页脚
  ◆ 行。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-RC-01 | 12-reload-config-01 | 预置 download_slots = 5 启动并添加任务下载；外部改 download_slots = 3 → 主界面按 Ctrl+R；随后依次在 添加对话框打开 / 帮助窗口打开 / 命令窗口打开 时各改写 download_slots 递异值并按 Ctrl+R | toast `✓ 配置已热加载`；队列栏 0/3（即时生效）；各浮层内 Ctrl+R 均触发热加载（toast 出现、队列栏槽位数逐次变化）且浮层不关闭；运行中任务不中断 |
| QA-RC-02 | 12-reload-config-02 | global_max_download_speed = 0（不限）启动，慢速文件下载中；外部改为 `512 KiB/s` → Ctrl+R | toast `✓ 配置已热加载`；运行中任务速度立即按 512 KiB/s 重定目标（限速粒度内速度显示变化）；任务不中断（进度推进、连接保持、无失败重试） |
| QA-RC-03 | 12-reload-config-03 | download_slots = 1 启动，添加 3 个慢速任务（1 下载 2 等待）；外部改 3 → Ctrl+R；再外部改 1 → Ctrl+R；等待某任务自然结束 | 第一次：toast `✓ 配置已热加载`、等待任务按列表顺序立即启动（0/3）；第二次：toast 再次 `✓ 配置已热加载`、3 个运行任务均不被抢占（仍 0/3）；任务结束后空槽不补位（等待任务不自动启动，allocate 只授予不回收） |
| QA-RC-04 | 12-reload-config-04 | auto_retry 两相位（true→false 与 false→true 各一轮）：fixture 约 30% 断连失败，外部改 auto_retry → Ctrl+R 后再次触发同形失败 | 首次失败判定按初值语义（true = 自动重试倒计时；false = 直接已失败无倒计时）；热加载后（toast `✓ 配置已热加载`）下一次失败判定按新值语义呈现 |
| QA-RC-05 | 12-reload-config-05 | 命名代理 p1 = http 127.0.0.1:18081 启动，HTTP 任务（代理 = p1）下载中；外部改 p1 为 127.0.0.1:18082 → Ctrl+R | toast `✓ 配置已热加载；代理切换：1 个任务`；存量连接不打断（当前块跑完、进度不回退、无失败）；代理 fixture 日志出现 18082 新连接来源；任务最终完成且文件字节完整 |
| QA-RC-06 | 12-reload-config-06 | p1/p2 双代理启动，两 HTTP 任务分别引用并下载中；外部删除 p1 → Ctrl+R；打开 p1 任务修改对话框；重启 ezr 后开修改对话框复查；外部重加 p1 → Ctrl+R；再次 Ctrl+R（配置未变） | toast 仅 `✓ 配置已热加载`（引用消失不追加专属提示段，统一 reload toast，D82②——M 段退役）；p1 任务按直连继续（fixture 直连来源出现、不中断不失败）；p2 任务无切换；添加对话框代理下拉 = 直连 + p2（p1 不再出现）；p1 任务修改对话框代理预填 直连 且选项 = 直连 + p2（p1 不被重新注入，D82①）；重启后该任务代理仍 直连（落库持久化、旧名不复活）；重加 p1 → Ctrl+R 后 p1 重现两对话框下拉、该任务保持直连不复挂（无代理切换段）；第二次 Ctrl+R toast 仅 `✓ 配置已热加载` |
| QA-RC-07 | 12-reload-config-07 | block_size_http = 1 MB / http_concurrency = 4 / max_retries = 5 启动，任务 a 下载中；外部改 2 MB / 8 / 9 → Ctrl+R；再添加任务 b | toast `✓ 配置已热加载`；任务 a 块大小/并发/重试上限维持烘焙值（分块口径与并发面板连接数不变）；任务 b 按新值烘焙（2 MB / 8 / 9）；添加对话框并发预填 8 |
| QA-RC-08 | 12-reload-config-08 | throttle_burst_secs = 0.5 + listen_port = 51444 启动，BT 任务 t1 下载中（announce 来源 = 51444）；外部改 burst = 1.5 → Ctrl+R；外部改 listen_port = 51555 → Ctrl+R；不改再按 Ctrl+R；新添加 BT 任务 t2 | 第一次 toast `✓ 配置已热加载；1 项需重启生效`（仅 burst 变化）；t1 传输无异常；第二次 toast `✓ 配置已热加载；1 项需重启生效`（仅 listen_port 相对运行配置变化）；BT 监听仍 51444（t2 announce 来源 = 51444——listen_port 启动绑定一次）；第三次 toast 仅 `✓ 配置已热加载`（C 类无变化不再提示） |
| QA-RC-09 | 12-reload-config-09 | download_slots = 5 + global_max_download_speed = 1 MiB/s 启动，慢速任务下载中；①外部写 download_slots = "abc" → Ctrl+R；②同时写 global_max_download_speed = 2 MiB/s 与非法 download_slots → Ctrl+R；③修正全部非法值 → Ctrl+R | ①toast `⚠ 配置热加载失败：…`（含「键 common.download_slots」逐键文案语义、单行无换行）；旧配置原样保留（0/5、限速仍 1 MiB/s、无半新半旧）；②整体拒绝（合法键也不生效，仍 1 MiB/s）；③toast `✓ 配置已热加载`（全部合法后成功生效） |
| QA-RC-10 | 12-reload-config-10 | ezr 以 --max-speed 256 KiB/s 启动（config 文件值 1 MiB/s），慢速任务下载中；外部改文件值 2 MiB/s → Ctrl+R | toast `✓ 配置已热加载`；生效全局限速仍 256 KiB/s（CLI 覆盖贯穿热加载重应用，不被文件值冲掉）；任务速度与头部仪表按 256 KiB/s 呈现 |
| QA-RC-11 | 12-reload-config-11 | download_slots = 2 + 命名代理 p1 启动（队列栏 0/2）；删除 $EZR_HOME/config.toml → Ctrl+R | toast `✓ 配置已热加载`；默认配置整体生效（队列栏 0/5、全局限速回默认不限）；$EZR_HOME/config.toml 未被重新生成（模板生成属启动逻辑，D81①）；代理下拉仅剩 直连 |
| QA-RC-12 | 12-reload-config-12 | HOME/USERPROFILE/EZR_HOME 全部未设置启动 ezr（回退锁 <系统临时目录>/ezr.lock）；主界面按 Ctrl+R | toast `无配置文件`；应用不退出、配置保持默认（任务列表可继续操作）；若沙箱环境无法构造无主目录启动则记 S（内嵌环境判据文案），不得伪造 P |
| QA-RC-13 | 12-reload-config-13 | 按 H 打开帮助窗口；检查【全局】组；帮助打开时按 Ctrl+R | 【全局】组含条目 `Ctrl+R 重新加载配置文件（热加载，无需重启）`；帮助窗口可滚动容纳新增条目（窗口尺寸不变）；Ctrl+R 触发热加载（toast 出现）且帮助窗口不关闭 |
| QA-RC-14 | 12-reload-config-14 | p1/p2 双代理启动，两 HTTP 任务分别引用并下载中；外部将 p1 改名 p1x（端点/认证不变）→ Ctrl+R；打开 p1 任务修改对话框（浮层保持）；浮层打开时外部将 p1x 改名回 p1 → Ctrl+R；关闭浮层重启 ezr 复查 | 改名 toast 仅 `✓ 配置已热加载`（旧名消失不追加专属提示段，统一 reload toast，D82②③——M 段退役）；p1 任务按直连继续（fixture 直连来源出现）；p2 任务无切换；添加对话框下拉 = 直连 + p1x + p2（p1 消失、p1x 按配置顺序出现）；p1 任务修改对话框预填 直连（无旧名，D82①）；浮层内改名回 p1 → Ctrl+R：toast 仅 `✓ 配置已热加载`（无任务引用 p1x，无任何可选段）、浮层不关、下拉即时重建 = 直连 + p1 + p2、预填仍 直连（不自动复挂，D82③）；重启后该任务仍 直连（落库持久化、旧名不复活） |

## 通过准则

- 全部用例结果记 P（通过）/ F（失败）/ S（受限跳过）；S 必须内嵌环境判据文案。
- toast 文案逐字断言主干与可选段（`✓ 配置已热加载` / `；代理切换：N 个任务` /
  `；K 项需重启生效` / `⚠ 配置热加载失败：` /
  `无配置文件`）；可选段触发条件按 D81②③⑤（N = 运行中且端点实际变化任务数、K = 本次
  实际变化的 C 类键数；引用消失臂无专属提示段——D82② 操作者改判，v1.55 M 口径退役；
  改名新名无专属段，D82③）。
- 原子性判据（QA-RC-09）：拒绝臂之后旧配置逐键与拒绝前一致（队列栏槽位数、生效限速、
  任务状态三通道交叉验证），不接受「部分生效」中间态。
- 代理热切对账口径（QA-RC-05/06/14）：fixture 连接来源日志为唯一事实来源；存量连接不打断 =
  进度无回退 + 无失败转态 + 连接行无消失重连形态。
- 关联回归：QA-RC-01 与 01-tui-display-07（页签与浮层关闭语义）、QA-RC-03 与
  01-slots-queue（槽位分配）、QA-RC-04 与 01-retry-backoff（重试语义）、QA-RC-05/06/14 与
  02-named-proxy（代理解析与缺失引用；启动期 missing 臂 02-named-proxy-05 语义不变，
  引用消失落库转直连仅属热加载路径 FR-01-119⑤）、QA-RC-08 与 09-bt-engine-16（listen_port 绑定）
  联合回归——热加载不得破坏既有语义。
- config 模板零干扰：QA-RC-11 断言热加载不触发生成逻辑（FR-01-85「启动时」语义边界）。
