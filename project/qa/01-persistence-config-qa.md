# QA 套件 · 01-persistence-config 持久化配置与生命周期（期号 01）

> 对应规格：`project/features/01-persistence-config.feature`（14 场景；**v1.26 修订场景 13——specifier QA-PC-13 组成算术重述轮 20261116，BT 判据 trackers 单键口径改标 + default_seed_duration_m 写入值口径；v1.25 修订场景 03/04/12——场景 04 max_speed 退役键拒绝行删除（待批① 裁决 20261116，FR-01-60 v1.42）+ 场景 03/12 退役键名标签清理；v1.24 修订场景 04/12——specifier FR-02-40② 退役键口径同步轮 20261116，FR-02-40②/FR-02-35 ③**）。
> **v1.28 裁决落录（操作者裁决「认可保持 ["tcp"]」，specifier peer_connection_protocol 裁决落实轮 20261116）**：QA-PC-13 非默认单值臂 peer_connection_protocol 取值钉为 **["tcp"]**——追认 QA 01-PC 套件可执行化扩面轮（v1.27）在 runner _case_13 的改判（["webrtc"] → ["tcp"]，沿 20261112 锚点对齐轮先例披露、待操作者追认的观察项收口）。判据：["tcp"] 合法（白名单 {tcp,webrtc} 内、去重）且非默认（≠ 模板默认 ["tcp","webrtc"] 双值）；WebRTC 单值臂下 ezr 无跨协议回退、无法与 libtorrent seeder（TCP only）建立连接致 BT 下载流程不可达成——TCP 臂下「[bt] 其余键 = 下载做种全程无错误」判据达成（runner 全量 P=14 F=0 S=0 实证）。需求口径不变（mission v1.56 FR-02-34 默认 ["tcp","webrtc"]、无跨协议回退——非默认单值臂属合法取值子集，本轮不涉 mission 修订）；feature 场景 13 通用「合法非默认值」口径不变（不钉值）；零行为语义变更，规程文字面补裁决注记与取值钉。
> **v1.27 补行（操作者裁决 handoff 待批② 执行，specifier QA-PC-10/11 规程行补齐轮 20261116）**：用例表补 QA-PC-10（EZR_HOME 环境变量重定位 .ezr 目录）与 QA-PC-11（kill -9 异常终止终端自恢复）两规程行——规程用例表自 v1.17 重构起即缺此两行（历史缺口沿 handoff 待批② 登记），本轮补齐后规程 14 用例与 feature 14 场景全对齐；QA-PC-12 规程行核对已在位、不变（runner 用例与可执行化归下一 QA 小轮）。零行为语义变更，规程文字面补行。
> **v1.26 重述（操作者批准 B 案「规程重述 + runner 键集补齐」，specifier QA-PC-13 组成算术重述轮 20261116；QA 同步轮按模板权威拨正分解）**：QA-PC-13 操作列组成算术按模板权威口径重述——「40 键 = [common] 6 键 + HTTP 表 11 键 + [bt] 23 键」，[[common.proxy]] 双条目为可选附加面不计入 40（模板权威 = 43 活跃行 = 40 键值 + 3 表声明，suite_config_template 逐键断言锚；[bt] 23 = trackers/peer_connection_protocol/bt_concurrency 3 核心键 + 20 调优键）。spec 面自 v1.20 沿用的「+ proxy 双条目 + [bt] 21」分解两项偏差恰好抵消合 40（proxy 误计入 +2、[bt] 少计 2），本轮拨正；v1.20 键集改组：max_speed 退役 + 4 限速键改入 + listen_port 新增，36→40。判据列 trackers 预填改单键口径（http_trackers/udp_trackers 退役不并入池，v1.21/FR-02-35）+ 补列 global_max_upload_speed 与 task_max_* 限速键、listen_port 判据 + default_seed_duration_m 改写入值臂。runner _case_13 键集补齐（+5 键、去 seed_duration_secs 残留、peer_connection_protocol 改非默认单值臂）与过时注记结案回填由 QA 同步轮落实（操作者 20261116 已批；注记回填沿 20261112 锚点对齐轮先例归 runner 面）。行为语义零变更，规程/feature 文字面修订。
> **v1.25 结案（操作者批准 QA 终验轮登记待批①，specifier max_speed 退役键拒绝行同步轮 20261116；FR-01-60 v1.42）**：QA-PC-04 拒绝样例集删除 `[common] max_speed=xyz` 行——max_speed 为 FR-01-60 v1.42 退役键，按未知键静默忽略（场景 12 同口径、单测 legacy_max_speed_key_retired_as_unknown 承载），退役键不入拒绝集；替代键 global_max_download_speed 拒绝行不新增（沿 runner QA-PC-04 行集 32 行锚——QA 终验轮已按三方一致口径裁定）。feature ↔ runner 行集对账 32=32 归零；runner QA-PC-04 偏差注记结案回填归 QA 面（沿 20261112 锚点对齐轮先例，本轮 runner 零改动）。同形全查另改判 QA-PC-13 判据限速标签「max_speed = 速度封顶」→「global_max_download_speed = 速度封顶」（FR-02-28 权威键名，与 runner _case_13 实写键与断言对账；行为语义零变更）。
> **v1.24 同步（操作者批准 QA 终验轮待批①，specifier FR-02-40② 退役键口径同步轮 20261116；FR-02-40②/FR-02-35 ③）**：QA-PC-04 调优键拒绝行改判 `bt.default_seed_duration_m = 0`（退役键 seed_duration_secs 不入拒绝集）+ 退役键静默忽略正探针（`[bt] seed_duration_secs=0` → 启动成功，与 runner QA-PC-04 v1.24 对账）；QA-PC-12 退役键判据补 seed_duration_secs；`[common] max_speed=xyz` 行与场景 12/单测互斥偏差沿 QA 终验轮 runner 注记登记待裁决（本轮不改行）。feature 同步 = 01-persistence-config v1.24（场景 04/12）。
> 端到端 UI 层验证：CLI 启动/退出操作 + `~/.ezr` 磁盘产物观察 + TUI 状态断言 + 双实例进程操作。
> **v1.51 同步（phase-05 引擎提速轮 20261125，FR-05-05/12、D84/D85）**：默认并发 4 → 8
> （QA-PC-03/12 判据同步）；config 新增 [http] scheduler（默认 dynamic）与 min_split_kib
> （默认 256）——QA-PC-13 键集 40 → 42（6+13+23，scheduler 写入非默认值 blocks 使块模型
> 判据可观察）；QA-PC-04 拒绝样例行集沿 runner 32 行锚不变（新键非法值拒绝由
> qa/13-http-turbo-qa QA-HT-13 独立承载，runner 行集扩充归 QA 轮）；QA-PC-05 钉定 blocks
> 回退模式 + 256 KiB 行回归（下限放宽）。
> **v1.19 同步（操作者 bug14 轮指令，D63②；FR-01-85 v1.13）**：新增 QA-PC-14（活跃模板
> 起步改值生效链——首启 v1.13 活跃模板直接改 bt_concurrency=10/http_concurrency=3/
> download_slots=2/download_dir → 重启逐项生效；复刻操作者「改了 bt_concurrency 添加任务
> 窗口仍是 20」的复现路径作为回归证据）。
> **v1.18 同步（操作者 bug13 轮指令，D62③）**：新增 QA-PC-13（全量配置键合法值生效冒烟：
> 36 键全量写入合法非默认值 → 启动零警告 + 可观察键逐项生效 + 引擎内部键不致病，回应
> 「所有 config.toml 中的配置项的都要测试验证是否可用」）。
> v1.17（操作者 bug11 轮指令，config 三大标准表重构）：配置键按「<表>.<键名>」点路径书写——
> QA 写入 = 对应表头（`[common]` / `[http]` / `[bt]`）+ 键值行；QA-PC-04 扩充新增调优键
> 非法值回退行（[http] 5 键 + [bt] 20 键代表性样例，统一回退口径 = 回退代码常量默认）；
> 新增 QA-PC-12（旧格式顶层键与旧 [[proxies]] 表静默忽略）。
> **v1.21 修订（操作者修订轮指令，FR-02-35/D70）**：QA-PC-04 行为翻转——全部非法值行改判
> 「启动失败并报错」（错误含配置文件路径、键点路径与非法原因，非零退出码；原「回退/钳制生效
> + udp_trackers 非法 URL 忽略警告」口径整体退役）；样例集改写：udp_trackers 行随键退役替换为
> `[bt] trackers=["ftp://x/ann"]`（未知 scheme）与 `[bt] peer_connection_protocol=["tcp","tcp"]`
> （重复项）两数组行；QA-PC-12 增退役键判据（http_trackers/udp_trackers 按未知键口径直接废弃，
> 不迁移不兼容不并入池——见 01-persistence-config.feature v1.21）。

## 环境前置

- fixture 服务器（慢速门控，保证退出/重启窗口内任务处于确定状态）。
- 伪终端驱动（终端恢复态断言：光标可见、无残留字符/花屏）；独立 HOME；进程管理工具（pgrep/pkill、kill -9）。
- config.toml 写入模板（三表表头 + 各键合法/非法样本值）。

## 用例

| 用例 | 场景 | 操作 | 通过判据 |
|---|---|---|---|
| QA-PC-01 | 01-persistence-config-01 | 预置各状态任务→优雅退出→重启 | 任务/历史/顺序完整还原；续传点与状态一致 |
| QA-PC-02 | 01-persistence-config-02 | 添加任务后检查主目录 | 存在 `.ezr/state/`（注册表）；`.ezr/config.toml` 存在（v1.4/FR-01-85：缺失时启动自动生成全注释默认模板，细则见 `qa/01-config-template-qa.md`）；sidecar 随目标文件 |
| QA-PC-03 | 01-persistence-config-03 | 删除 config.toml 后下载 | 默认并发 8（v1.51：4 → 8，FR-05-05）/槽位 5/动态分段调度（v1.51：scheduler 默认 dynamic，FR-05-12）/重试 5/auto_retry 开/不限速/download_dir=用户主目录下载目录（Linux ~/Downloads）；BT 侧默认并发 20/做种 1800s（v1.17 口径注记；「块 1MB」默认随动态分段引擎退役——块模型默认沿 blocks 回退模式口径 1 MB） |
| QA-PC-04 | 01-persistence-config-04 | 分别写入非法值（v1.17/v1.19 点路径全量：`[http] block_size_http=abc`、`[common] download_slots=0`、`[http] max_retries=-1`、`[http] http_concurrency=99` + 调优键 0/负值样例全集（同 feature Examples 表）+ `[bt] trackers=["ftp://x/ann"]`（未知 scheme）+ `[bt] peer_connection_protocol=["tcp","tcp"]`（重复项） | **v1.21 行为翻转（FR-02-35）**：每行均启动失败并报错——错误信息含配置文件路径、键点路径与非法原因，非零退出码退出，配置文件字节级不变（原「启动不报错 + 逐键回退/钳制 + udp_trackers 非法 URL 忽略警告」口径整体退役）；**v1.24 同步（FR-02-40②）**：调优键全集拒绝行改判 `bt.default_seed_duration_m = 0`（新键标量非法；退役键 seed_duration_secs 不入拒绝集——按未知键口径静默废弃，不报错不生效），另附退役键静默忽略正探针（`[bt] seed_duration_secs=0` → 启动成功，与 runner QA-PC-04 v1.24 对账）；**v1.25 结案（待批① 裁决 20261116，FR-01-60 v1.42）**：`[common] max_speed=xyz` 拒绝行删除——退役键不入拒绝集（按未知键静默忽略，场景 12/单测 legacy_max_speed_key_retired_as_unknown 承载），替代键拒绝行不新增（沿 runner 行集 32 行锚）；feature↔runner 行集对账 32=32 归零，runner QA-PC-04 注记结案回填归 QA 面 |
| QA-PC-05 | 01-persistence-config-05 | `[http] scheduler="blocks"` + `block_size_http=4MB / 2MB / 256KiB` 下载 5MB（v1.51：块模型断言钉定 blocks 回退模式；256 KiB 行随下限放宽回归——v1.16 原 256KB 行因当时 1 MB 下限改判 4MB） | 分块行 `x/2 · 4 MB/块` / `x/3 · 2 MB/块` / `x/20 · 256 KB/块` |
| QA-PC-06 | 01-persistence-config-06 | `[common] download_slots=2` 后添加 4 任务 | 前 2 下载；队列栏「下载槽位 n/2」；其余排队 |
| QA-PC-07 | 01-persistence-config-07 | 运行中二次启动 ezr | 第二实例提示「ezr 已在运行」非零退出；首实例正常；全程单实例（AC-11） |
| QA-PC-08 | 01-persistence-config-08 | 下载至 30% 按 Q→重启 | 断点保留（sidecar 完整）；终端恢复；重启续传完成（AC-9） |
| QA-PC-09 | 01-persistence-config-09 | 下载中在主界面按 Esc；再以 Ctrl+C 退出 | Esc 主界面无任何动作（应用不退出、任务继续下载——退出仅 Q/Ctrl+C，01-tui-display-07 口径，操作者 20261027 裁决定稿）；Ctrl+C 优雅退出（语义同 Q：停传、保断点、存注册表；收尾亦可以 Q 优雅退出复验同口径）；终端无花屏（AC-9）。场景 09 文本已按裁决同步修订 |
| QA-PC-10 | 01-persistence-config-10 | 环境变量 EZR_HOME 指向一个空目录 → 运行 ezr 并添加任务（触发配置读取与注册表写入）；另两轮探针：EZR_HOME 置空串/未设置启动、两组不同 EZR_HOME 实例并行运行 | 配置文件读取自 $EZR_HOME/config.toml 且任务注册表写入 $EZR_HOME/state/registry.json；单实例锁文件位于 $EZR_HOME/state/ezr.lock；用户主目录下不存在 .ezr/（默认路径未被创建）；EZR_HOME 为空串或未设置时回退缺省 ~/.ezr（D16 缺省口径）；不同 EZR_HOME 的两组实例各自持有各自的锁与注册表（隔离语义互不冲突）（v1.27 补行，待批② 裁决 20261116；场景 v1.3/D16） |
| QA-PC-11 | 01-persistence-config-11 | ezr TUI 运行中（raw mode、鼠标捕获、bracketed paste 与备用屏幕已启用）→ 从另一终端以 SIGKILL（kill -9）终止 ezr 进程 | 运行 ezr 的终端立即自恢复：不再出现 "32;64;10M" 形态的鼠标事件转义字符残影输出；终端回到正常（cooked）模式：按键回显生效、Ctrl+C 可正常产生 SIGINT 中断；鼠标捕获、bracketed paste 与备用屏幕均已复位，光标与配色正常；哨兵子进程随之退出，不留残留进程（v1.27 补行，待批② 裁决 20261116；场景 v1.3/FR-01-84） |
| QA-PC-12 | 01-persistence-config-12 | 预置旧格式 config.toml（顶层平铺 `download_slots=2`、`block_size_http=4194304`、`http_concurrency=8`、`max_speed="1 MB/s"` + 顶层 `[[proxies]]` 合法条目，无任何新表头）→ 启动、添加任务、打开添加对话框代理下拉；随后同文件追加 `[common]` 表头与 `download_slots = 3` 再启动 | 启动不报错、无忽略类 toast/警告（零警告零迁移）；全部旧键忽略、行为全默认（槽位 5/动态分段默认引擎/并发默认 8（v1.51）/不限速）；旧 [[proxies]] 条目不出现在代理下拉；追加新表头后 `[common] download_slots=3` 正常生效（新格式不受旧键干扰）（v1.17，操作者钦定不兼容旧格式）；**退役键判据（v1.21/v1.24，FR-02-35/FR-02-40②）**：http_trackers/udp_trackers 按未知键口径直接废弃（不迁移不兼容不并入池，v1.21 已登记）；[bt] seed_duration_secs 同口径静默废弃（不报错不生效、启动成功——v1.24 正探针，与 QA-PC-04 探针臂对账，runner 口径见 suite_persistence_config.py QA-PC-04 v1.24） |
| QA-PC-13 | 01-persistence-config-13 | 一次性写入全部 42 键合法非默认值（[common] 6 键 + [[common.proxy]] 双条目 + [http] 13 键含 v1.51 新增 scheduler="blocks"/min_split_kib=512 + [bt] 23 键——[bt] 23 = trackers/peer_connection_protocol/bt_concurrency 3 核心键 + 20 调优键（default_seed_duration_m/listen_port 在内）；proxy 双条目为命名代理生效面、不计入 42；v1.26 按模板权威对账：40 活跃键 = 6+11+23（v1.51：40 → 42 = 6+13+23）（spec 面自 v1.20 沿用「+ proxy 双条目 + [bt] 21」分解两项偏差恰好抵消合 40）；v1.21：seed_duration_secs 由 default_seed_duration_m 替代、键数不变；[bt.seed_exit_rule] 保持未配置默认形态）→ 启动 → 添加 HTTP 下载（观察预填/队列栏/分块行/限速）→ 打开添加对话框代理下拉 → 经命令窗口打开 BT 对话框（观察 trackers 预填/并发预填）→ 完成一次 BT 下载 | 启动零警告 toast；download_dir = 添加对话框预填值；download_slots = 队列栏「n/<slots>」；max_speed = 速度封顶（v1.25 改标 global_max_download_speed = 速度封顶，FR-01-60 v1.42/FR-02-28 权威键名，runner _case_13 实写 global_max_download_speed = "2 MiB/s" 已对账）；global_max_upload_speed = BT 上传限速生效（TP-08 实跑承载，v1.26 补列）；task_max_download_speed/task_max_upload_speed = HTTP 对话框下载限速与 BT 对话框下载/上传限速字段预填值（v1.26 随 runner 键集补齐补列，预填断言同 suite_add_task QA-01-20 口径）；命名代理 = 下拉按序出现可选；scheduler = blocks 引擎回退生效（分块行块模型口径，v1.51）；min_split_kib = 仅 dynamic 模式适用（blocks 任务运行期无异常，dynamic 生效面由 qa/13-http-turbo-qa 承载，v1.51）；block_size_http = 分块行自定义值；http_concurrency = 并发回填值；auto_retry/max_retries/backoff_* = 重试序列与上限一致；[http] 5 调优键 = 运行期无异常；trackers = BT 对话框 trackers 预填（配置 trackers 与种子 announce 合并；v1.26 改标——退役键 http_trackers/udp_trackers 不并入池，v1.21/FR-02-35）；bt_concurrency = BT 并发预填值；peer_connection_protocol = ["tcp"]（非默认单值臂，操作者裁决认可 v1.28——WebRTC 单值臂下 BT 下载流程不可达成、TCP 臂承载「下载完成产物完整」判据）；listen_port = 实际监听端口生效（生效断言由 suite_modify_bt announce 观察承载，套件内以合法解析零警告承载，v1.26）；default_seed_duration_m = 做种时长按写入值生效（exit rule 未配置时其生效、未配置默认 30 分钟；v1.26 随 runner 键集补齐由默认臂改写入值臂，生效断言沿通过准则调优键口径由引擎级承载；v1.21/FR-02-40）；[bt] 其余键 = 下载做种全程无错误；下载完成产物完整（v1.18 新增，D62③；v1.21 键替换；v1.26 组成算术重述 + 判据键集随 runner 补齐对齐全量口径） |
| QA-PC-14 | 01-persistence-config-14 | 干净 HOME 首启 ezr 生成 v1.13 活跃模板后退出 → 在生成的 config.toml 直接改值（[bt] bt_concurrency = 10、[http] http_concurrency = 3、[common] download_slots = 2、[common] download_dir = <tmp_dl>）→ 再次启动并使用（v1.19 新增，D63②） | 启动不报错零警告；下载槽位 = 2（队列栏「n/2」，第 3 个任务排队）；BT 对话框并发预填 = **10**（修 bug：改值后仍显示默认 20）；HTTP 对话框并发 = 3、保存目录预填 = <tmp_dl>；证据链逐环可验（模板字节级 → 改值落盘 → 重启解析 → 对话框预填） |

## 通过准则

- 全部用例 P/F/S；S 内嵌环境判据。QA-PC-07 对应 AC-11、QA-PC-08/09 对应 AC-9。
- QA-PC-04 的调优键「生效」断言以可观察面为准：`udp_trackers` 非法 URL 警告 + 数值键经
  09-bt-engine-14 引擎行为与单元级回退断言承载（套件内不逐键构造端到端可观察差异）。

## 套件脚本

- 可执行套件：`project/qa/runners/suite_persistence_config.py`（QA 会话落）。
- **v1.17 同步待办（coder/QA 会话执行）**：本套件 `env.write_config` 各处顶层键改三表形态
  （`[common] download_slots` / `[http] block_size_http` 等，约 11 处）；`suite_slots_queue.py`
  （9 处 download_slots → `[common]`）、`suite_throttle_proxy.py`（8 处 max_speed → `[common]`）、
  `suite_tui_display.py`（2 处 block_size_http → `[http]`）、`suite_ui_conns.py`（3 处 →
  `[common]`/`[http]`）、`suite_resume_sidecar.py`（1 处）、`suite_integrity_check.py`（1 处）、
  `suite_retry_backoff.py`（2 处 → `[http]`）、`suite_add_task.py`（1 处 download_dir →
  `[common]`）、`suite_modify_task.py`（2 处）、`suite_named_proxy.py`（`proxies_toml` 生成器
  `[[proxies]]` → `[[common.proxy]]`，15 处调用随之）、`suite_config_template.py`（见该套件
  v1.11 待办）。
